// Pages reached from the sidebar, the rail and the menus: People, Posts,
// Meetings, Goals, Forms, Whiteboards, Reminders, AI and the Overview view.

use super::*;
use super::menus::ROLES;

fn title(ui: &mut Ui, t: &Theme, text: &str, sub: &str) {
    ui.label(RichText::new(text).size(20.0));
    if !sub.is_empty() {
        ui.label(RichText::new(sub).color(t.text2));
    }
    ui.add_space(10.0);
}

fn list_panel_frame(t: &Theme) -> egui::Frame {
    egui::Frame::new().fill(t.sidebar).corner_radius(egui::CornerRadius { nw: 8, ne: 0, sw: 8, se: 0 }).inner_margin(egui::Margin::symmetric(10, 12))
}

/// Space Overview cards that Customize can turn off.
pub const OV_CARDS: [&str; 5] = ["Recent", "Docs", "Bookmarks", "Folders", "Lists"];

/// What the whiteboard's Undo takes back, newest last.
pub enum WbUndo {
    Stroke,
    Note,
    NoteGone(usize, Sticky),
    StrokeGone(usize, WbStroke),
}

/// Screen state of the app pages (not saved).
#[derive(Default)]
pub struct SweepUi {
    /// Reminders: show the done ones too.
    pub reminders_done: bool,
    /// The reminder being changed: (id, text, date, time).
    pub reminder_edit: Option<(u64, String, Option<NaiveDate>, String)>,
    /// Due reminders already announced this session.
    pub reminders_alerted: HashSet<u64>,
    /// Planner: the day a task is being typed into, and its name.
    pub planner_add: Option<(NaiveDate, String)>,
    /// Search: the highlighted result, and the query it belongs to.
    pub search_sel: usize,
    pub search_q: String,
    /// Searches run with Enter, newest first.
    pub search_recent: Vec<String>,
    /// A destructive button waiting for "Yes" (e.g. "wb_clear", "goal").
    pub confirm: Option<String>,
    /// Whiteboard pen width (0 = the standard 2.5).
    pub wb_width: f32,
    pub wb_undo: Vec<(u64, WbUndo)>,
    /// The form `form_fill` holds answers for.
    pub form_for: Option<u64>,
    pub form_error: Option<String>,
    /// Dashboard card being renamed: (index, text).
    pub card_rename: Option<(usize, String)>,
    /// Time entry being changed: (task, index, start "HH:MM", hours).
    pub entry_edit: Option<(u64, usize, String, f64)>,
}

/// Pen and note colours on a whiteboard.
pub const WB_COLORS: [[u8; 3]; 6] = [[32, 32, 32], [0xd3, 0x3d, 0x44], [8, 128, 234], [41, 151, 100], [0xf8, 0xae, 0x00], [0x5f, 0x55, 0xee]];

/// Question kinds of a form: what the answer fills in.
pub const FORM_KINDS: [(&str, &str); 10] = [
    ("name", "Task name"),
    ("description", "Description"),
    ("priority", "Priority"),
    ("due", "Due date"),
    ("assignee", "Assignee"),
    ("text", "Short text"),
    ("dropdown", "Dropdown"),
    ("number", "Number"),
    ("email", "Email"),
    ("checkbox", "Checkbox"),
];

fn form_kind_label(k: &str) -> &'static str {
    FORM_KINDS.iter().find(|x| x.0 == k).map(|x| x.1).unwrap_or("Short text")
}

/// What a filled-in form makes: the new task's fields.
#[derive(Debug, Default)]
pub struct FormAnswers {
    pub name: String,
    pub description: String,
    pub priority: u8,
    pub due: Option<NaiveDate>,
    pub assignees: Vec<u64>,
}

/// Checks the answers (required, number, email) and turns them into task fields.
pub fn form_answers(f: &Form, fill: &HashMap<usize, String>) -> Result<FormAnswers, String> {
    let mut a = FormAnswers::default();
    for (i, fld) in f.fields.iter().enumerate() {
        let v = fill.get(&i).map(|s| s.trim().to_string()).unwrap_or_default();
        if v.is_empty() {
            if fld.required {
                return Err(format!("Please answer \"{}\".", fld.label));
            }
            continue;
        }
        match fld.map.as_str() {
            "name" => a.name = v,
            "description" => a.description.push_str(&v),
            "priority" => a.priority = PRIORITIES.iter().position(|p| p.label == v).unwrap_or(0) as u8,
            "due" => a.due = NaiveDate::parse_from_str(&v, "%Y-%m-%d").ok(),
            "assignee" => a.assignees.extend(v.parse::<u64>().ok()),
            k => {
                if k == "number" && v.parse::<f64>().is_err() {
                    return Err(format!("\"{}\" needs a number.", fld.label));
                }
                if k == "email" && !(v.contains('@') && v.contains('.')) {
                    return Err(format!("\"{}\" needs an email address.", fld.label));
                }
                a.description.push_str(&format!("\n{}: {}", fld.label, v));
            }
        }
    }
    Ok(a)
}

/// The standard dashboard cards, in order, with their titles.
pub const DASH_CARDS: [(&str, &str); 6] = [
    ("stats", "Task totals"),
    ("status", "Tasks by status"),
    ("priority", "Tasks by priority"),
    ("workload", "Workload by assignee"),
    ("time", "Time tracked this week"),
    ("overdue", "Overdue tasks"),
];

pub fn dash_card_title(c: &DashCard) -> String {
    if !c.title.is_empty() {
        return c.title.clone();
    }
    DASH_CARDS.iter().find(|k| k.0 == c.kind).map(|k| k.1.to_string()).unwrap_or_else(|| c.kind.clone())
}

pub fn default_dash_cards() -> Vec<DashCard> {
    DASH_CARDS.iter().map(|k| DashCard { kind: k.0.into(), title: String::new() }).collect()
}

/// Reads a workboard:// link (from Copy link): ("task" | "list" | "space" | "folder", id).
pub fn parse_wb_link(q: &str) -> Option<(&str, u64)> {
    let rest = q.trim().strip_prefix("workboard://")?;
    let (kind, id) = rest.split_once('/')?;
    let id = id.trim_end_matches('/').parse().ok()?;
    matches!(kind, "task" | "list" | "space" | "folder").then_some((kind, id))
}

impl App {
    // ---------------------------------------------------------------- people

    pub(super) fn people_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("People").size(20.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, &t, "Invite people").clicked() {
                            self.modal = Some(Modal::Invite { emails: String::new(), role: "Member".into() });
                        }
                    });
                });
                ui.label(RichText::new(format!("Manage who can access {}", self.data.workspace)).color(t.text2));
                ui.add_space(12.0);
                let w = ui.available_width();
                let (hr, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
                for (x, h) in [(0.0, "Name"), (0.32, "Email"), (0.6, "Role"), (0.76, "Status")] {
                    ui.painter().text(pos2(hr.left() + w * x + 8.0, hr.center().y), Align2::LEFT_CENTER, h, FontId::proportional(13.0), t.text2);
                }
                ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
                let members = self.data.members.clone();
                let me = self.data.me;
                let mut remove = None;
                for m in &members {
                    let (r, _) = ui.allocate_exact_size(vec2(w, 46.0), Sense::hover());
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                    paint_avatar(ui.painter(), pos2(r.left() + 22.0, r.center().y), 13.0, m, t.bg);
                    let name = if m.id == me { format!("{} (you)", m.name) } else { m.name.clone() };
                    ui.painter().text(pos2(r.left() + 44.0, r.center().y - (if m.status_text.is_empty() { 0.0 } else { 8.0 })), Align2::LEFT_CENTER, name, FontId::proportional(14.0), t.text);
                    if !m.status_text.is_empty() {
                        ui.painter().text(pos2(r.left() + 44.0, r.center().y + 9.0), Align2::LEFT_CENTER, &m.status_text, FontId::proportional(12.0), t.text3);
                    }
                    ui.painter().text(pos2(r.left() + w * 0.32 + 8.0, r.center().y), Align2::LEFT_CENTER, if m.email.is_empty() { "-" } else { &m.email }, FontId::proportional(13.5), t.text2);
                    let role_rect = Rect::from_min_size(pos2(r.left() + w * 0.6, r.center().y - 13.0), vec2(w * 0.15, 26.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(role_rect).layout(Layout::left_to_right(Align::Center)));
                    let mut role = if m.role.is_empty() { "Member".to_string() } else { m.role.clone() };
                    if m.role == "Owner" {
                        child.label("Owner");
                    } else {
                        egui::ComboBox::from_id_salt(("role", m.id)).selected_text(role.clone()).width(100.0).show_ui(&mut child, |ui| {
                            for (n, _) in ROLES.iter().take(3) {
                                ui.selectable_value(&mut role, n.to_string(), *n);
                            }
                        });
                        if role != m.role && !(m.role.is_empty() && role == "Member") {
                            if let Some(mm) = self.data.members.iter_mut().find(|x| x.id == m.id) {
                                mm.role = role;
                                self.dirty = true;
                            }
                        }
                    }
                    let (st, col) = if m.invited { ("Invited", Color32::from_rgb(0xe1, 0x6b, 0x16)) } else { ("Active", Color32::from_rgb(41, 151, 100)) };
                    ui.painter().text(pos2(r.left() + w * 0.76 + 8.0, r.center().y), Align2::LEFT_CENTER, st, FontId::proportional(13.0), col);
                    if m.id != me && m.role != "Owner" {
                        let br = Rect::from_min_size(pos2(r.right() - 80.0, r.center().y - 12.0), vec2(72.0, 24.0));
                        if put_over(ui, br, egui::Button::new(RichText::new("Remove").color(t.danger)).frame(false)).clicked() {
                            remove = Some(m.id);
                        }
                    }
                }
                if let Some(id) = remove {
                    self.data.members.retain(|m| m.id != id);
                    for task in &mut self.data.tasks {
                        task.assignees.retain(|a| *a != id);
                    }
                    for c in &mut self.data.channels {
                        c.members.retain(|a| *a != id);
                    }
                    self.dirty = true;
                }
                ui.add_space(12.0);
                ui.label(RichText::new("Invited people get access when the shared server connects them. Until then they can use this computer through Log out \u{2192} Switch person.").small().color(t.text3));
            });
        });
    }

    // ---------------------------------------------------------------- inbox (Primary / Other / Later / Cleared)

    pub(super) fn inbox_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let my_name = self.data.member(me).map(|m| format!("@{}", m.name)).unwrap_or_default();
        // tabs
        let w = ui.available_width();
        let (tr, _) = ui.allocate_exact_size(vec2(w, 50.0), Sense::hover());
        let tw = (w - 20.0) / 4.0;
        for (i, (icon, label)) in [("\u{1F4E5}", "Primary"), ("\u{26A1}", "Other"), ("\u{23F0}", "Later"), ("\u{2714}", "Cleared")].iter().enumerate() {
            let r = Rect::from_min_size(pos2(tr.left() + 10.0 + tw * i as f32, tr.top()), vec2(tw, 50.0));
            let resp = ui.interact(r, ui.id().with(("itab", i)), Sense::click());
            name_it(&resp, &format!("Inbox {}", label));
            let sel = self.inbox_tab == i as u8;
            if resp.hovered() && !sel {
                ui.painter().rect_filled(r.shrink(4.0), 6.0, t.hover);
            }
            ui.painter().text(r.left_center() + vec2(26.0, 0.0), Align2::CENTER_CENTER, *icon, FontId::proportional(14.0), if sel { t.text } else { t.text2 });
            ui.painter().text(r.left_center() + vec2(44.0, 0.0), Align2::LEFT_CENTER, *label, FontId::proportional(15.0), if sel { t.text } else { t.text2 });
            if sel {
                ui.painter().rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 2.0), pos2(r.right() - 4.0, r.bottom())), 1.0, t.text);
            }
            if i > 0 {
                ui.painter().vline(r.left(), r.top() + 14.0..=r.bottom() - 14.0, Stroke::new(1.0, t.border));
            }
            if resp.clicked() {
                self.inbox_tab = i as u8;
            }
        }
        ui.painter().hline(tr.x_range(), tr.bottom(), Stroke::new(1.0, t.border));
        // collect items
        let _ = me;
        let mut items: Vec<(String, i64, u64, String, Activity)> = vec![];
        for task in &self.data.tasks {
            for a in &task.activity {
                let key = format!("{}:{}:{}", task.id, a.ts, a.author);
                if self.inbox_tab_of(&key, a) == self.inbox_tab {
                    items.push((key, a.ts, task.id, task.name.clone(), a.clone()));
                }
            }
        }
        match self.inbox_filter {
            1 => items.retain(|x| x.4.comment),
            2 => items.retain(|x| x.4.comment && x.4.text.contains(&my_name)),
            3 => items.retain(|x| self.assigned_to_me(&x.4)),
            _ => {}
        }
        items.sort_by_key(|x| std::cmp::Reverse(x.1));
        let mut clear_all = false;
        egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 10)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = ["\u{2261}  Filter", "\u{2261}  Comments", "\u{2261}  Mentions", "\u{2261}  Assigned"][self.inbox_filter as usize % 4];
                let fr = ui.add(egui::Button::new(label).corner_radius(13));
                popup(ui, &fr, ui.id().with("inbox_filter"), 180.0, |ui| {
                    for (i, l) in ["Everything", "Comments", "Mentions", "Assigned to me"].iter().enumerate() {
                        if ui.selectable_label(self.inbox_filter == i as u8, *l).clicked() {
                            self.inbox_filter = i as u8;
                            close_popup(ui);
                        }
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add_enabled(!items.is_empty() && self.inbox_tab != 3, egui::Button::new("\u{2714}  Clear all").corner_radius(6)).clicked() {
                        clear_all = true;
                    }
                    if ui.add(egui::Button::new("\u{2699}").corner_radius(6)).on_hover_text("Notification settings").clicked() {
                        self.modal = Some(Modal::Notifications);
                    }
                    if ui.add(egui::Button::new("\u{23F0}  Reminder").corner_radius(6)).clicked() {
                        self.modal = Some(Modal::Reminder { text: String::new(), date: Some(today()), time: "09:00".into() });
                    }
                });
            });
        });
        if clear_all {
            for (k, ..) in &items {
                self.data.extra.inbox_later.retain(|x| x != k);
                self.data.extra.inbox_cleared.push(k.clone());
            }
            self.dirty = true;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 0)).show(ui, |ui| {
                if self.inbox_tab == 0 {
                    self.reminders_block(ui);
                }
                if items.is_empty() {
                    ui.add_space(ui.available_height() * 0.3);
                    ui.vertical_centered(|ui| {
                        let alone = self.data.members.len() <= 1;
                        let (title, sub) = match self.inbox_tab {
                            0 if alone => ("Looking to collaborate?", "Collaboration is one invite away."),
                            0 => ("You're all caught up", "New comments, mentions and assignments show up here."),
                            1 => ("Nothing in Other", "Updates that don't need you show up here."),
                            2 => ("Nothing saved for later", "Use Later on an item to come back to it."),
                            _ => ("Nothing cleared yet", "Cleared items show up here."),
                        };
                        let (r, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                        ui.painter().rect_stroke(r.shrink(2.0), 14.0, Stroke::new(1.5, t.border), egui::StrokeKind::Inside);
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "\u{1F465}", FontId::proportional(22.0), Color32::from_rgb(0x5f, 0x55, 0xee));
                        ui.add_space(10.0);
                        ui.label(RichText::new(title).size(18.0).color(t.text));
                        ui.label(RichText::new(sub).size(14.0).color(t.text2));
                        if self.inbox_tab == 0 {
                            ui.add_space(10.0);
                            if ui.add(egui::Button::new(RichText::new("Invite people").size(15.0).color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(0.0, 38.0))).clicked() {
                                self.modal = Some(Modal::Invite { emails: String::new(), role: "Member".into() });
                            }
                        }
                    });
                }
                for (key, ts, tid, tname, a) in items {
                    let w = ui.available_width();
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click());
                    name_it(&resp, &tname);
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 8.0, t.hover);
                    }
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                    if let Some(m) = self.data.member(a.author) {
                        paint_avatar(ui.painter(), r.left_center() + vec2(22.0, 0.0), 13.0, m, t.bg);
                    }
                    ui.painter().text(r.left_top() + vec2(48.0, 18.0), Align2::LEFT_CENTER, &tname, FontId::proportional(14.0), t.text);
                    let who = self.data.member(a.author).map(|m| m.name.clone()).unwrap_or_default();
                    let line = if a.comment { format!("{} commented: {}", who, a.text) } else { format!("{} {}", who, a.text) };
                    ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.right() - 190.0, r.bottom()))).text(r.left_top() + vec2(48.0, 40.0), Align2::LEFT_CENTER, line, FontId::proportional(13.0), t.text2);
                    ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, fmt_ago(ts), FontId::proportional(12.0), t.text3);
                    let mut handled = false;
                    if resp.hovered() || ui.rect_contains_pointer(r) {
                        let lr = Rect::from_min_size(pos2(r.right() - 180.0, r.center().y - 13.0), vec2(56.0, 26.0));
                        let cr = Rect::from_min_size(pos2(r.right() - 120.0, r.center().y - 13.0), vec2(56.0, 26.0));
                        if self.inbox_tab != 2 && put_over(ui, lr, egui::Button::new("Later").corner_radius(6)).clicked() {
                            self.data.extra.inbox_later.push(key.clone());
                            self.dirty = true;
                            handled = true;
                        }
                        // Later items can go back to the inbox
                        if self.inbox_tab == 2 && put_over(ui, lr, egui::Button::new("Primary").corner_radius(6)).on_hover_text("Back to the inbox").clicked() {
                            self.data.extra.inbox_later.retain(|x| x != &key);
                            self.dirty = true;
                            handled = true;
                        }
                        let ctext = if self.inbox_tab == 3 { "Restore" } else { "Clear" };
                        if put_over(ui, cr, egui::Button::new(ctext).corner_radius(6)).clicked() {
                            if self.inbox_tab == 3 {
                                self.data.extra.inbox_cleared.retain(|x| x != &key);
                            } else {
                                self.data.extra.inbox_later.retain(|x| x != &key);
                                self.data.extra.inbox_cleared.push(key.clone());
                            }
                            self.dirty = true;
                            handled = true;
                        }
                    }
                    if resp.clicked() && !handled {
                        self.actions.push(Action::Open(tid));
                    }
                }
            });
        });
    }

    /// True for "assigned to <my name>" activity (what assigning logs).
    pub(super) fn assigned_to_me(&self, a: &Activity) -> bool {
        let Some(m) = self.data.member(self.data.me) else { return false };
        !a.comment && !m.name.is_empty() && a.text == format!("assigned to {}", m.name)
    }

    /// Which Inbox tab an activity belongs in: 0 Primary, 1 Other, 2 Later, 3 Cleared.
    pub(super) fn inbox_tab_of(&self, key: &str, a: &Activity) -> u8 {
        let x = &self.data.extra;
        if x.inbox_cleared.iter().any(|k| k == key) {
            return 3;
        }
        if x.inbox_later.iter().any(|k| k == key) {
            return 2;
        }
        let n = &x.notify;
        let my_name = self.data.member(self.data.me).map(|m| format!("@{}", m.name)).unwrap_or_default();
        let mention = !my_name.is_empty() && a.text.contains(&my_name);
        let primary = a.author != self.data.me && ((a.comment && (n.comments || (n.mentions && mention))) || (n.assigned && self.assigned_to_me(a)));
        if primary {
            0
        } else {
            1
        }
    }

    /// The Inbox badge: Primary items of the last day not cleared or put off.
    pub(super) fn inbox_unseen(&self) -> usize {
        let day = now_ts() - 86400;
        let mut n = 0;
        for task in &self.data.tasks {
            for a in task.activity.iter().filter(|a| a.ts > day) {
                if self.inbox_tab_of(&format!("{}:{}:{}", task.id, a.ts, a.author), a) == 0 {
                    n += 1;
                }
            }
        }
        n
    }

    // ---------------------------------------------------------------- replies

    /// Replies: others' comments on tasks and replies to my chat messages,
    /// each one readable on its own or all at once.
    pub(super) fn replies_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
            ui.label(RichText::new("Replies").size(20.0).strong());
            ui.label(RichText::new("Comments from others on tasks and replies to your chat messages").color(t.text2));
            ui.add_space(8.0);
            let tab_id = egui::Id::new(("inbox_tab", 1u8));
            let mut tab: u8 = ui.data(|d| d.get_temp(tab_id)).unwrap_or(0);
            ui.horizontal(|ui| {
                for (i, l) in ["Unread", "Read"].iter().enumerate() {
                    if ui.selectable_label(tab == i as u8, *l).clicked() {
                        tab = i as u8;
                    }
                }
                if tab == 0 {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Mark all as read").clicked() {
                            self.data.extra.replies_seen = now_ts();
                            self.dirty = true;
                        }
                    });
                }
            });
            ui.data_mut(|d| d.insert_temp(tab_id, tab));
            ui.add_space(10.0);
            // (key, ts, author, title, line, open: task or (channel, message))
            let mut items: Vec<(String, i64, u64, String, String, Result<u64, (u64, u64)>)> = vec![];
            for task in &self.data.tasks {
                for a in task.activity.iter().filter(|a| a.comment && a.author != me) {
                    items.push((format!("{}:{}:{}", task.id, a.ts, a.author), a.ts, a.author, task.name.clone(), a.text.clone(), Ok(task.id)));
                }
            }
            for c in &self.data.channels {
                let place = if c.kind == ChannelKind::Direct { "Direct message".to_string() } else { format!("#{}", c.name) };
                for m in c.messages.iter().filter(|m| m.author == me) {
                    for r in m.replies.iter().filter(|r| r.author != me) {
                        items.push((format!("msg:{}", r.id), r.ts, r.author, format!("Reply in {}", place), r.text.clone(), Err((c.id, m.id))));
                    }
                }
            }
            let seen = self.data.extra.replies_seen;
            let read = |k: &str, ts: i64| ts <= seen || self.data.extra.replies_read.iter().any(|x| x == k);
            items.retain(|x| read(&x.0, x.1) == (tab == 1));
            items.sort_by_key(|x| std::cmp::Reverse(x.1));
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if items.is_empty() {
                    ui.label(RichText::new("You're all caught up").color(t.text3));
                }
                for (key, ts, author, title, text, open) in items.into_iter().take(200) {
                    let w = ui.available_width();
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 56.0), Sense::click());
                    name_it(&resp, &title);
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 8.0, t.hover);
                    }
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                    if let Some(m) = self.data.member(author) {
                        paint_avatar(ui.painter(), r.left_center() + vec2(22.0, 0.0), 13.0, m, t.bg);
                    }
                    ui.painter().text(r.left_top() + vec2(48.0, 16.0), Align2::LEFT_CENTER, &title, FontId::proportional(13.5), t.text);
                    let who = self.data.member(author).map(|m| m.name.clone()).unwrap_or_default();
                    let verb = if open.is_ok() { "commented" } else { "replied" };
                    ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.right() - 200.0, r.bottom()))).text(r.left_top() + vec2(48.0, 38.0), Align2::LEFT_CENTER, format!("{} {}: {}", who, verb, text), FontId::proportional(12.5), t.text2);
                    ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, fmt_ago(ts), FontId::proportional(11.5), t.text3);
                    let mut handled = false;
                    if tab == 0 {
                        let br = Rect::from_min_size(pos2(r.right() - 190.0, r.center().y - 13.0), vec2(90.0, 26.0));
                        let b = put_over(ui, br, egui::Button::new("Mark read").corner_radius(6));
                        name_it(&b, &format!("Mark read {}", text));
                        if b.clicked() {
                            self.data.extra.replies_read.push(key.clone());
                            self.dirty = true;
                            handled = true;
                        }
                    }
                    if resp.clicked() && !handled {
                        match open {
                            Ok(tid) => self.actions.push(Action::Open(tid)),
                            Err((cid, mid)) => {
                                self.section = Section::Chat;
                                self.chat_open = Some(cid);
                                self.thread_open = Some(mid);
                            }
                        }
                        if !self.data.extra.replies_read.contains(&key) {
                            self.data.extra.replies_read.push(key);
                            self.dirty = true;
                        }
                    }
                }
            });
        });
    }

    // ---------------------------------------------------------------- posts

    pub(super) fn posts_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.page_bar(ui, "Posts", |s, ui| {
            let tt = s.t();
            if primary_button(ui, &tt, "New Post").clicked() {
                s.post_title.clear();
                s.post_body.clear();
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let w = ui.available_width();
            let cw = 960.0f32.min(w - 60.0);
            let pad = ((w - cw) / 2.0).max(0.0);
            ui.add_space(20.0);
            ui.horizontal_top(|ui| {
                ui.add_space((pad - 50.0).max(0.0));
                if let Some(m) = self.data.member(self.data.me).cloned() {
                    let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                    paint_avatar(ui.painter(), r.center(), 15.0, &m, t.bg);
                }
                egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::symmetric(16, 10)).show(ui, |ui| {
                    ui.set_width(cw - 34.0);
                    // the frame sits in a horizontal row; stack its contents top to bottom
                    ui.vertical(|ui| {
                    let chans: Vec<(u64, String)> = self.data.channels.iter().filter(|c| c.kind == ChannelKind::Channel).map(|c| (c.id, c.name.clone())).collect();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Channel:").color(t.text2));
                        let cur = chans.iter().find(|c| c.0 == self.post_channel).map(|c| format!("# {}", c.1)).unwrap_or_else(|| "Select Channel".into());
                        egui::ComboBox::from_id_salt("post_ch").selected_text(cur).show_ui(ui, |ui| {
                            for (id, n) in &chans {
                                ui.selectable_value(&mut self.post_channel, *id, format!("# {}", n));
                            }
                        });
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Type:").color(t.text2));
                        egui::ComboBox::from_id_salt("post_type").selected_text(format!("\u{1F4E2} {}", self.post_type)).show_ui(ui, |ui| {
                            for k in ["Announcement", "Update", "Idea", "Question"] {
                                ui.selectable_value(&mut self.post_type, k.to_string(), k);
                            }
                        });
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Title:").color(t.text2));
                        ui.add(egui::TextEdit::singleline(&mut self.post_title).frame(false).desired_width(f32::INFINITY));
                    });
                    ui.separator();
                    ui.add(egui::TextEdit::multiline(&mut self.post_body).hint_text("Share an announcement...").frame(false).desired_rows(3).desired_width(f32::INFINITY));
                    ui.horizontal(|ui| {
                        for (g, tip) in [("+", "Create task"), ("@", "Mention someone"), ("\u{263A}", "Emoji"), ("\u{2611}", "Create task")] {
                            if ui.add(egui::Button::new(RichText::new(g).color(t.text2)).frame(false)).on_hover_text(tip).clicked() {
                                match g {
                                    "@" => self.post_body.push('@'),
                                    "\u{263A}" => self.post_body.push_str("\u{1F44D}"),
                                    _ => self.open_quick_add(None),
                                }
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let ready = !self.post_title.trim().is_empty() && self.post_channel != 0;
                            let b = egui::Button::new(RichText::new("Post").color(if ready { Color32::WHITE } else { t.text3 })).fill(if ready { t.primary } else { t.hover }).corner_radius(6).min_size(vec2(56.0, 30.0));
                            if ui.add(b).on_hover_text(if self.post_channel == 0 { "Select a channel first" } else { "" }).clicked() && ready {
                                let id = self.data.new_id();
                                let me = self.data.me;
                                let m = Message { edited: None, attachments: vec![], id, author: me, text: self.post_body.trim().to_string(), ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: Some(self.post_title.trim().to_string()), post_type: self.post_type.clone(), pinned: false, call: None };
                                if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == self.post_channel) {
                                    c.messages.push(m);
                                    c.last_read = now_ts();
                                }
                                self.post_title.clear();
                                self.post_body.clear();
                                self.dirty = true;
                            }
                        });
                    });
                    });
                });
            });
            ui.add_space(20.0);
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let fol = egui::Button::new(RichText::new("\u{1F514}  Channels I'm Following").color(if self.post_following { t.accent_text } else { t.text2 })).fill(if self.post_following { t.accent_bg } else { t.bg }).corner_radius(13);
                if ui.add(fol).clicked() {
                    self.post_following = !self.post_following;
                }
                let chans: Vec<(u64, String)> = self.data.channels.iter().filter(|c| c.kind == ChannelKind::Channel).map(|c| (c.id, c.name.clone())).collect();
                let cl = self.post_f_channel.and_then(|id| chans.iter().find(|c| c.0 == id)).map(|c| format!("#  {}", c.1)).unwrap_or_else(|| "#  Channel".into());
                let cr = ui.add(egui::Button::new(cl).corner_radius(13));
                popup(ui, &cr, ui.id().with("pf_ch"), 180.0, |ui| {
                    if ui.selectable_label(self.post_f_channel.is_none(), "All channels").clicked() {
                        self.post_f_channel = None;
                        close_popup(ui);
                    }
                    for (id, n) in &chans {
                        if ui.selectable_label(self.post_f_channel == Some(*id), format!("# {}", n)).clicked() {
                            self.post_f_channel = Some(*id);
                            close_popup(ui);
                        }
                    }
                });
                let members: Vec<(u64, String)> = self.data.members.iter().map(|m| (m.id, m.name.clone())).collect();
                let al = self.post_f_author.and_then(|id| members.iter().find(|m| m.0 == id)).map(|m| format!("@  {}", m.1)).unwrap_or_else(|| "@  Author".into());
                let ar = ui.add(egui::Button::new(al).corner_radius(13));
                popup(ui, &ar, ui.id().with("pf_au"), 180.0, |ui| {
                    if ui.selectable_label(self.post_f_author.is_none(), "Anyone").clicked() {
                        self.post_f_author = None;
                        close_popup(ui);
                    }
                    for (id, n) in &members {
                        if ui.selectable_label(self.post_f_author == Some(*id), n).clicked() {
                            self.post_f_author = Some(*id);
                            close_popup(ui);
                        }
                    }
                });
                let tl = self.post_f_type.clone().map(|t| format!("\u{25A6}  {}", t)).unwrap_or_else(|| "\u{25A6}  Type".into());
                let tr = ui.add(egui::Button::new(tl).corner_radius(13));
                popup(ui, &tr, ui.id().with("pf_ty"), 180.0, |ui| {
                    if ui.selectable_label(self.post_f_type.is_none(), "Any type").clicked() {
                        self.post_f_type = None;
                        close_popup(ui);
                    }
                    for k in ["Announcement", "Update", "Idea", "Question"] {
                        if ui.selectable_label(self.post_f_type.as_deref() == Some(k), k).clicked() {
                            self.post_f_type = Some(k.to_string());
                            close_popup(ui);
                        }
                    }
                });
                ui.add_space((cw - 520.0).max(10.0));
                ui.label(RichText::new("Sort:").color(t.text2));
                egui::ComboBox::from_id_salt("post_sort").selected_text(if self.post_sort_old { "Oldest" } else { "Newest" }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.post_sort_old, false, "Newest");
                    ui.selectable_value(&mut self.post_sort_old, true, "Oldest");
                });
            });
            ui.add_space(12.0);
            let mut posts: Vec<(String, u64, Message)> = vec![];
            for c in &self.data.channels {
                for m in &c.messages {
                    if m.post_title.is_some() {
                        posts.push((c.name.clone(), c.id, m.clone()));
                    }
                }
            }
            let me = self.data.me;
            let following: Vec<u64> = self.data.channels.iter().filter(|c| c.members.contains(&me)).map(|c| c.id).collect();
            posts.retain(|p| {
                (!self.post_following || following.contains(&p.1))
                    && self.post_f_channel.map(|c| c == p.1).unwrap_or(true)
                    && self.post_f_author.map(|a| a == p.2.author).unwrap_or(true)
                    && self.post_f_type.as_ref().map(|ty| (if p.2.post_type.is_empty() { "Announcement" } else { p.2.post_type.as_str() }) == ty).unwrap_or(true)
            });
            posts.sort_by_key(|p| p.2.ts);
            if !self.post_sort_old {
                posts.reverse();
            }
            if posts.is_empty() {
                ui.add_space(60.0);
                ui.vertical_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                    ui.painter().rect_stroke(r.shrink(2.0), 14.0, Stroke::new(1.5, t.border), egui::StrokeKind::Inside);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "\u{1F4E2}", FontId::proportional(22.0), t.text2);
                    ui.add_space(10.0);
                    ui.label(RichText::new("There are no Posts yet!").size(18.0).color(t.text));
                });
            }
            for (cname, cid, m) in posts {
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                        ui.set_width(cw - 28.0);
                        ui.horizontal(|ui| {
                            if let Some(a) = self.data.member(m.author) {
                                let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                                paint_avatar(ui.painter(), r.center(), 12.0, a, t.card);
                                ui.label(RichText::new(&a.name).strong());
                            }
                            ui.label(RichText::new(format!("{} in # {} \u{2022} {}", if m.post_type.is_empty() { "Announcement" } else { &m.post_type }, cname, fmt_ago(m.ts))).small().color(t.text3));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.link("Open in channel").clicked() {
                                    self.section = Section::Chat;
                                    self.chat_open = Some(cid);
                                }
                            });
                        });
                        ui.label(RichText::new(m.post_title.clone().unwrap_or_default()).size(17.0));
                        if !m.text.is_empty() {
                            ui.add(egui::Label::new(&m.text).wrap());
                        }
                    });
                });
                ui.add_space(8.0);
            }
        });
    }

    // ---------------------------------------------------------------- meetings

    pub(super) fn meetings_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::SidePanel::left("meet_list").exact_width(260.0).resizable(false).frame(list_panel_frame(&t)).show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Meetings").size(16.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, &t, "+ Meeting").clicked() {
                        let id = self.data.new_id();
                        self.data.extra.meetings.push(Meeting { id, title: "New meeting".into(), date: today(), time: "10:00".into(), attendees: vec![self.data.me], agenda: String::new(), notes: String::new() });
                        self.meeting_open = Some(id);
                        self.dirty = true;
                    }
                });
            });
            ui.add_space(8.0);
            let mut ms = self.data.extra.meetings.clone();
            ms.sort_by_key(|m| (m.date, m.time.clone()));
            for m in &ms {
                let sel = self.meeting_open == Some(m.id);
                let w = ui.available_width();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 44.0), Sense::click());
                if sel {
                    ui.painter().rect_filled(r, 6.0, t.row_selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                ui.painter().with_clip_rect(r).text(r.left_top() + vec2(10.0, 14.0), Align2::LEFT_CENTER, &m.title, FontId::proportional(14.0), t.text);
                ui.painter().text(r.left_top() + vec2(10.0, 31.0), Align2::LEFT_CENTER, format!("{} \u{2022} {}", fmt_date(m.date), m.time), FontId::proportional(11.5), if m.date < today() { t.text3 } else { t.text2 });
                if resp.clicked() {
                    self.meeting_open = Some(m.id);
                }
            }
            if ms.is_empty() {
                ui.label(RichText::new("No meetings yet").color(t.text3));
            }
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::symmetric(32, 20))).show_inside(ui, |ui| {
            let Some(idx) = self.meeting_open.and_then(|id| self.data.extra.meetings.iter().position(|m| m.id == id)) else {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| ui.label(RichText::new("Pick a meeting or create one to keep the agenda and notes together.").color(t.text2)));
                return;
            };
            let members = self.data.members.clone();
            let mut delete = false;
            let mut make_tasks: Vec<String> = vec![];
            {
                let m = &mut self.data.extra.meetings[idx];
                let mut ch = false;
                ch |= ui.add(egui::TextEdit::singleline(&mut m.title).font(FontId::proportional(26.0)).frame(false).desired_width(f32::INFINITY)).changed();
                ui.horizontal(|ui| {
                    let mut d = Some(m.date);
                    let r = ui.button(format!("\u{1F4C5} {}", fmt_date(m.date)));
                    if let Some(true) = popup(ui, &r, ui.id().with("meet_date"), 380.0, |ui| date_menu(ui, &t, ui.id().with("meet_cal"), &mut d)) {
                        if let Some(d) = d {
                            m.date = d;
                            ch = true;
                        }
                    }
                    ui.label("at");
                    ch |= ui.add(egui::TextEdit::singleline(&mut m.time).desired_width(60.0)).changed();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("Delete").color(t.danger)).frame(false)).clicked() {
                            delete = true;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.label(RichText::new("Attendees").color(t.text2));
                ui.horizontal_wrapped(|ui| {
                    for mem in &members {
                        let mut on = m.attendees.contains(&mem.id);
                        if ui.checkbox(&mut on, &mem.name).changed() {
                            if on {
                                m.attendees.push(mem.id);
                            } else {
                                m.attendees.retain(|a| *a != mem.id);
                            }
                            ch = true;
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label(RichText::new("Agenda").color(t.text2));
                ch |= ui.add(egui::TextEdit::multiline(&mut m.agenda).hint_text("What will you discuss?").desired_rows(4).desired_width(f32::INFINITY)).changed();
                ui.add_space(8.0);
                ui.label(RichText::new("Notes").color(t.text2));
                ch |= ui.add(egui::TextEdit::multiline(&mut m.notes).hint_text("Write notes. Lines starting with [ ] become action items.").desired_rows(10).desired_width(f32::INFINITY)).changed();
                let actions: Vec<String> = m.notes.lines().filter_map(|l| l.trim().strip_prefix("[ ]").map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect();
                if !actions.is_empty() && ui.button(format!("Create {} tasks from action items", actions.len())).clicked() {
                    make_tasks = actions;
                    m.notes = m.notes.replace("[ ]", "[x]");
                    ch = true;
                }
                if ch {
                    self.dirty = true;
                }
            }
            if !make_tasks.is_empty() {
                if let Some(list) = self.data.first_list() {
                    let who = self.data.extra.meetings[idx].attendees.clone();
                    for n in make_tasks {
                        self.actions.push(Action::Add { list, name: n, status: None, priority: 0, assignees: who.iter().take(1).copied().collect(), due: None, parent: None, open: false });
                    }
                    self.toast = Some(("Tasks created from the meeting notes".into(), now_ts()));
                }
            }
            if delete {
                self.data.extra.meetings.remove(idx);
                self.meeting_open = None;
                self.dirty = true;
            }
        });
    }

    // ---------------------------------------------------------------- goals

    pub(super) fn goal_progress(&self, g: &Goal) -> f32 {
        if g.targets.is_empty() {
            return 0.0;
        }
        let sum: f32 = g
            .targets
            .iter()
            .map(|tg| match &tg.kind {
                TargetKind::Number { start, target, current, .. } => {
                    if (target - start).abs() < f64::EPSILON {
                        1.0
                    } else {
                        (((current - start) / (target - start)) as f32).clamp(0.0, 1.0)
                    }
                }
                TargetKind::TrueFalse { done } => {
                    if *done {
                        1.0
                    } else {
                        0.0
                    }
                }
                TargetKind::Tasks { ids } => {
                    if ids.is_empty() {
                        0.0
                    } else {
                        ids.iter().filter(|id| self.task(**id).map(|x| self.data.is_closed(x)).unwrap_or(false)).count() as f32 / ids.len() as f32
                    }
                }
            })
            .sum();
        sum / g.targets.len() as f32
    }

    pub(super) fn goals_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Goals").size(20.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, &t, "+ New Goal").clicked() {
                            let id = self.data.new_id();
                            let color = STATUS_PALETTE[1 + self.data.extra.goals.len() % 13];
                            self.data.extra.goals.push(Goal { id, name: "New goal".into(), color, due: None, owner: self.data.me, targets: vec![] });
                            self.goal_open = Some(id);
                            self.dirty = true;
                        }
                    });
                });
                ui.label(RichText::new("Track progress with targets: numbers, done / not done, or tasks").color(t.text2));
                ui.add_space(12.0);
                let goals = self.data.extra.goals.clone();
                if goals.is_empty() {
                    ui.label(RichText::new("No goals yet.").color(t.text3));
                }
                for g in &goals {
                    let p = self.goal_progress(g);
                    let open = self.goal_open == Some(g.id);
                    let resp = egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, if open { t.text3 } else { t.border })).corner_radius(10).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 6.0, rgb(g.color));
                            ui.label(RichText::new(&g.name).size(16.0));
                            if let Some(d) = g.due {
                                ui.label(RichText::new(format!("Due {}", fmt_date(d))).small().color(date_color(d, p >= 1.0, &t)));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(format!("{:.0}%", p * 100.0)).strong());
                                if let Some(m) = self.data.member(g.owner) {
                                    let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                                    paint_avatar(ui.painter(), r.center(), 10.0, m, t.card);
                                    ui.label(RichText::new(&m.name).small().color(t.text2));
                                }
                            });
                        });
                        ui.add(egui::ProgressBar::new(p).desired_height(6.0).fill(rgb(g.color)));
                    });
                    if ui.interact(resp.response.rect, ui.id().with(("goal", g.id)), Sense::click()).clicked() {
                        self.goal_open = if open { None } else { Some(g.id) };
                        self.sweep.confirm = None;
                    }
                    if open {
                        self.goal_editor(ui, g.id);
                    }
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn goal_editor(&mut self, ui: &mut Ui, gid: u64) {
        let t = self.t();
        let Some(gi) = self.data.extra.goals.iter().position(|g| g.id == gid) else { return };
        let tasks: Vec<(u64, String)> = self.data.tasks.iter().filter(|x| x.parent.is_none()).map(|x| (x.id, x.name.clone())).collect();
        let closed: Vec<u64> = self.data.tasks.iter().filter(|x| self.data.is_closed(x)).map(|x| x.id).collect();
        let members: Vec<(u64, String)> = self.data.members.iter().map(|m| (m.id, m.name.clone())).collect();
        let asking = self.sweep.confirm.as_deref() == Some("goal");
        let mut ask = None;
        let mut delete = false;
        egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 0, top: 6, bottom: 6 }).show(ui, |ui| {
            let g = &mut self.data.extra.goals[gi];
            let mut ch = false;
            ui.horizontal(|ui| {
                ui.label("Name");
                ch |= ui.add(egui::TextEdit::singleline(&mut g.name).desired_width(260.0)).changed();
                let mut d = g.due;
                let r = ui.button(g.due.map(|d| format!("Due {}", fmt_date(d))).unwrap_or_else(|| "\u{1F4C5} Due date".into()));
                if let Some(true) = popup(ui, &r, ui.id().with(("goal_due", gid)), 380.0, |ui| date_menu(ui, &t, ui.id().with(("goal_cal", gid)), &mut d)) {
                    g.due = d;
                    ch = true;
                }
                if asking {
                    ui.label(RichText::new("Move this goal to Trash?").color(t.danger));
                    delete = ui.button("Yes").clicked();
                    if ui.button("No").clicked() {
                        ask = Some(false);
                    }
                } else if ui.add(egui::Button::new(RichText::new("Delete goal").color(t.danger)).frame(false)).clicked() {
                    ask = Some(true);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Owner");
                let cur = members.iter().find(|m| m.0 == g.owner).map(|m| m.1.clone()).unwrap_or_else(|| "Nobody".into());
                egui::ComboBox::from_id_salt(("goal_owner", gid)).selected_text(cur).show_ui(ui, |ui| {
                    for (id, n) in &members {
                        ch |= ui.selectable_value(&mut g.owner, *id, n).changed();
                    }
                });
                ui.label("Color");
                for c in STATUS_PALETTE.iter().skip(1) {
                    let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    ui.painter().circle_filled(r.center(), 7.0, rgb(*c));
                    if g.color == *c {
                        ui.painter().circle_stroke(r.center(), 9.0, Stroke::new(1.5, t.text));
                    }
                    if resp.clicked() {
                        g.color = *c;
                        ch = true;
                    }
                }
            });
            let mut remove = None;
            for (i, tg) in g.targets.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ch |= ui.add(egui::TextEdit::singleline(&mut tg.name).desired_width(180.0)).changed();
                    match &mut tg.kind {
                        TargetKind::Number { start, target, current, unit } => {
                            ui.label("now");
                            ch |= ui.add(egui::DragValue::new(current).speed(1.0)).changed();
                            ui.label("of");
                            ch |= ui.add(egui::DragValue::new(target).speed(1.0)).changed();
                            ui.label("from");
                            ch |= ui.add(egui::DragValue::new(start).speed(1.0)).changed();
                            ch |= ui.add(egui::TextEdit::singleline(unit).hint_text("unit").desired_width(50.0)).changed();
                        }
                        TargetKind::TrueFalse { done } => {
                            ch |= ui.checkbox(done, "Done").changed();
                        }
                        TargetKind::Tasks { ids } => {
                            let n = ids.iter().filter(|id| closed.contains(id)).count();
                            ui.label(format!("{}/{} tasks done", n, ids.len()));
                            ui.menu_button("Pick tasks", |ui| {
                                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                    for (id, name) in &tasks {
                                        let mut on = ids.contains(id);
                                        if ui.checkbox(&mut on, name).changed() {
                                            if on {
                                                ids.push(*id);
                                            } else {
                                                ids.retain(|x| x != id);
                                            }
                                            ch = true;
                                        }
                                    }
                                });
                            });
                        }
                    }
                    if ui.add(egui::Button::new(RichText::new("\u{2716}").color(t.text3)).frame(false)).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                g.targets.remove(i);
                ch = true;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Add target:").color(t.text2));
                if ui.button("Number").clicked() {
                    g.targets.push(GoalTarget { name: "Target".into(), kind: TargetKind::Number { start: 0.0, target: 100.0, current: 0.0, unit: String::new() } });
                    ch = true;
                }
                if ui.button("Done / not done").clicked() {
                    g.targets.push(GoalTarget { name: "Target".into(), kind: TargetKind::TrueFalse { done: false } });
                    ch = true;
                }
                if ui.button("Tasks").clicked() {
                    g.targets.push(GoalTarget { name: "Finish tasks".into(), kind: TargetKind::Tasks { ids: vec![] } });
                    ch = true;
                }
            });
            if ch {
                self.dirty = true;
            }
        });
        match ask {
            Some(true) => self.sweep.confirm = Some("goal".into()),
            Some(false) => self.sweep.confirm = None,
            None => {}
        }
        if delete {
            let g = self.data.extra.goals.remove(gi);
            self.to_trash("Goal", &g.name, &g);
            self.sweep.confirm = None;
            self.goal_open = None;
            self.dirty = true;
        }
    }

    // ---------------------------------------------------------------- forms

    pub(super) fn form_detail(&mut self, ui: &mut Ui) {
        let t = self.t();
        // answers belong to one form; another form starts empty
        if self.sweep.form_for != self.form_open {
            self.form_fill.clear();
            self.sweep.form_error = None;
            self.sweep.confirm = None;
            self.sweep.form_for = self.form_open;
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::symmetric(32, 20))).show_inside(ui, |ui| {
            let Some(fi) = self.form_open.and_then(|id| self.data.extra.forms.iter().position(|f| f.id == id)) else {
                ui.label(RichText::new("Create a form to collect requests; each answer becomes a task.").color(t.text2));
                return;
            };
            let fid = self.data.extra.forms[fi].id;
            let lists: Vec<(u64, String)> = self.data.spaces.iter().flat_map(|s| s.lists.iter().chain(s.folders.iter().flat_map(|f| f.lists.iter())).map(move |l| (l.id, format!("{} / {}", s.name, l.name)))).collect();
            let members: Vec<(u64, String)> = self.data.members.iter().map(|m| (m.id, m.name.clone())).collect();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.form_mode_fill, false, "\u{1F4DD} Edit");
                ui.selectable_value(&mut self.form_mode_fill, true, "\u{1F441} Fill out");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| self.star_button(ui, &format!("form:{}", fid)));
            });
            ui.separator();
            if !self.form_mode_fill {
                let mut delete = false;
                let asking = self.sweep.confirm.as_deref() == Some("form_delete");
                let f = &mut self.data.extra.forms[fi];
                let mut ch = false;
                ch |= ui.add(egui::TextEdit::singleline(&mut f.name).font(FontId::proportional(24.0)).frame(false).desired_width(f32::INFINITY)).changed();
                ch |= ui.add(egui::TextEdit::multiline(&mut f.description).desired_rows(2).desired_width(f32::INFINITY)).changed();
                ui.horizontal(|ui| {
                    ui.label("Answers create tasks in");
                    let cur = lists.iter().find(|l| l.0 == f.list_id).map(|l| l.1.clone()).unwrap_or_else(|| "choose a list".into());
                    egui::ComboBox::from_id_salt("form_list_sel").selected_text(cur).show_ui(ui, |ui| {
                        for (id, n) in &lists {
                            ch |= ui.selectable_value(&mut f.list_id, *id, n).changed();
                        }
                    });
                });
                ui.add_space(8.0);
                ui.label(RichText::new("Questions").strong());
                let mut remove = None;
                let mut swap = None;
                let count = f.fields.len();
                for (i, fld) in f.fields.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ch |= ui.add(egui::TextEdit::singleline(&mut fld.label).desired_width(220.0)).changed();
                        egui::ComboBox::from_id_salt(("map", i)).selected_text(form_kind_label(&fld.map)).show_ui(ui, |ui| {
                            for (k, n) in FORM_KINDS {
                                ch |= ui.selectable_value(&mut fld.map, k.to_string(), n).changed();
                            }
                        });
                        let rq = ui.checkbox(&mut fld.required, "Required");
                        name_it(&rq, &format!("Required {}", i + 1));
                        ch |= rq.changed();
                        let up = ui.add_enabled(i > 0, egui::Button::new("\u{2191}").frame(false));
                        name_it(&up, &format!("Move question {} up", i + 1));
                        if up.clicked() {
                            swap = Some((i, i - 1));
                        }
                        let down = ui.add_enabled(i + 1 < count, egui::Button::new("\u{2193}").frame(false));
                        name_it(&down, &format!("Move question {} down", i + 1));
                        if down.clicked() {
                            swap = Some((i, i + 1));
                        }
                        if ui.add(egui::Button::new(RichText::new("\u{2716}").color(t.text3)).frame(false)).clicked() {
                            remove = Some(i);
                        }
                    });
                    if fld.map == "dropdown" {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(RichText::new("Choices").small().color(t.text2));
                            // empty pieces are kept while typing so "a, " can grow into "a, b"
                            let mut text = fld.options.join(", ");
                            let r = ui.add(egui::TextEdit::singleline(&mut text).hint_text("Red, Green, Blue").desired_width(300.0));
                            name_it(&r, &format!("Choices {}", i + 1));
                            if r.changed() {
                                fld.options = text.split(',').map(|x| x.trim().to_string()).collect();
                                ch = true;
                            }
                        });
                    }
                }
                if let Some(i) = remove {
                    f.fields.remove(i);
                    ch = true;
                }
                if let Some((a, b)) = swap {
                    f.fields.swap(a, b);
                    ch = true;
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Add question").clicked() {
                        f.fields.push(FormField { label: "Question".into(), map: "text".into(), ..Default::default() });
                        ch = true;
                    }
                    if asking {
                        ui.label(RichText::new("Move this form to Trash?").color(t.danger));
                        if ui.button("Yes").clicked() {
                            delete = true;
                        }
                        if ui.button("No").clicked() {
                            self.sweep.confirm = None;
                        }
                    } else if ui.add(egui::Button::new(RichText::new("Delete form").color(t.danger)).frame(false)).clicked() {
                        self.sweep.confirm = Some("form_delete".into());
                    }
                });
                if ch {
                    self.dirty = true;
                }
                if delete {
                    let f = self.data.extra.forms[fi].clone();
                    self.to_trash("Form", &f.name, &f);
                    self.data.extra.forms.remove(fi);
                    self.data.extra.starred.retain(|s| s != &format!("form:{}", fid));
                    self.sweep.confirm = None;
                    self.form_open = None;
                    self.dirty = true;
                }
            } else {
                let f = self.data.extra.forms[fi].clone();
                egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::same(20)).show(ui, |ui| {
                    ui.set_width(ui.available_width().min(640.0));
                    ui.label(RichText::new(&f.name).size(22.0));
                    ui.label(RichText::new(&f.description).color(t.text2));
                    ui.add_space(10.0);
                    for (i, fld) in f.fields.iter().enumerate() {
                        ui.label(RichText::new(if fld.required { format!("{} *", fld.label) } else { fld.label.clone() }).strong());
                        let v = self.form_fill.entry(i).or_default();
                        match fld.map.as_str() {
                            "priority" => {
                                egui::ComboBox::from_id_salt(("fp", i)).selected_text(if v.is_empty() { "Choose".to_string() } else { v.clone() }).show_ui(ui, |ui| {
                                    for p in ["Urgent", "High", "Normal", "Low"] {
                                        ui.selectable_value(v, p.to_string(), p);
                                    }
                                });
                            }
                            "due" => {
                                let mut d = NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
                                let b = ui.button(d.map(fmt_date).unwrap_or_else(|| "\u{1F4C5} Pick a date".into()));
                                name_it(&b, &format!("Answer {}", i + 1));
                                if let Some(true) = popup(ui, &b, ui.id().with(("form_due", fid, i)), 380.0, |ui| date_menu(ui, &t, ui.id().with(("form_due_cal", fid, i)), &mut d)) {
                                    *v = d.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
                                }
                            }
                            "dropdown" => {
                                egui::ComboBox::from_id_salt(("fd", i)).selected_text(if v.is_empty() { "Choose".to_string() } else { v.clone() }).show_ui(ui, |ui| {
                                    for o in fld.options.iter().filter(|o| !o.is_empty()) {
                                        ui.selectable_value(v, o.clone(), o);
                                    }
                                });
                            }
                            "assignee" => {
                                let cur = v.parse::<u64>().ok().and_then(|id| members.iter().find(|m| m.0 == id)).map(|m| m.1.clone()).unwrap_or_else(|| "Choose someone".into());
                                egui::ComboBox::from_id_salt(("fa", i)).selected_text(cur).show_ui(ui, |ui| {
                                    for (id, n) in &members {
                                        ui.selectable_value(v, id.to_string(), n);
                                    }
                                });
                            }
                            "checkbox" => {
                                let mut on = v == "Yes";
                                let cb = ui.checkbox(&mut on, "Yes");
                                name_it(&cb, &format!("Answer {}", i + 1));
                                if cb.changed() {
                                    *v = if on { "Yes".into() } else { String::new() };
                                }
                            }
                            "description" | "text" => {
                                let r = ui.add(egui::TextEdit::multiline(v).desired_rows(3).desired_width(f32::INFINITY));
                                name_it(&r, &format!("Answer {}", i + 1));
                            }
                            k => {
                                let hint = match k {
                                    "number" => "0",
                                    "email" => "name@example.com",
                                    _ => "",
                                };
                                let r = ui.add(egui::TextEdit::singleline(v).hint_text(hint).desired_width(f32::INFINITY));
                                name_it(&r, &format!("Answer {}", i + 1));
                            }
                        }
                        ui.add_space(6.0);
                    }
                    if let Some(e) = &self.sweep.form_error {
                        ui.label(RichText::new(e).color(t.danger));
                    }
                    if primary_button(ui, &t, "Submit").clicked() {
                        match form_answers(&f, &self.form_fill) {
                            Err(e) => self.sweep.form_error = Some(e),
                            Ok(_) if self.data.space_of_list(f.list_id).is_none() => {
                                self.sweep.form_error = Some("This form has no List to put answers in. Pick one under Edit.".into());
                            }
                            Ok(a) => {
                                let name = if a.name.trim().is_empty() { format!("{} submission", f.name) } else { a.name };
                                self.actions.push(Action::Add { list: f.list_id, name, status: None, priority: a.priority, assignees: a.assignees, due: a.due, parent: None, open: false });
                                self.pending_description = Some(a.description.trim().to_string());
                                self.form_fill.clear();
                                self.sweep.form_error = None;
                                self.toast = Some((format!("Submitted. A task was created in {}", self.data.list_name(f.list_id)), now_ts()));
                            }
                        }
                    }
                });
            }
        });
    }

    fn side_like_row(&self, ui: &mut Ui, sel: bool, icon: &str, label: &str) -> egui::Response {
        let t = self.t();
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
        name_it(&resp, label);
        if sel {
            ui.painter().rect_filled(r, 7.0, t.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 7.0, t.hover);
        }
        ui.painter().text(r.left_center() + vec2(14.0, 0.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
        ui.painter().with_clip_rect(r).text(r.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), t.text);
        resp
    }

    // ---------------------------------------------------------------- whiteboards

    pub(super) fn whiteboard_canvas(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::same(12))).show_inside(ui, |ui| {
            let Some(wi) = self.wb_open.and_then(|id| self.data.extra.whiteboards.iter().position(|w| w.id == id)) else {
                ui.label(RichText::new("Create a whiteboard to sketch ideas and add sticky notes.").color(t.text2));
                return;
            };
            let wid = self.data.extra.whiteboards[wi].id;
            let mut delete = false;
            if self.sweep.wb_width <= 0.0 {
                self.sweep.wb_width = 2.5;
            }
            // wraps onto a second row in a narrow window
            ui.horizontal_wrapped(|ui| {
                if ui.add(egui::TextEdit::singleline(&mut self.data.extra.whiteboards[wi].name).font(FontId::proportional(18.0)).frame(false).desired_width(200.0)).changed() {
                    self.dirty = true;
                }
                self.star_button(ui, &format!("wb:{}", wid));
                ui.separator();
                ui.selectable_value(&mut self.wb_tool, 0, "\u{1F4DD} Pen");
                ui.selectable_value(&mut self.wb_tool, 3, "\u{232B} Eraser");
                ui.selectable_value(&mut self.wb_tool, 1, "\u{1F4DD} Sticky note");
                ui.selectable_value(&mut self.wb_tool, 2, "\u{270B} Move notes");
                for c in WB_COLORS {
                    let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                    ui.painter().circle_filled(r.center(), 8.0, rgb(c));
                    if self.wb_color == c {
                        ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(1.5, t.text));
                    }
                    if resp.clicked() {
                        self.wb_color = c;
                    }
                }
                let wr = ui.add(egui::DragValue::new(&mut self.sweep.wb_width).range(1.0..=16.0).speed(0.2).max_decimals(1).prefix("width "));
                name_it(&wr, "Pen width");
                ui.separator();
                let can_undo = self.sweep.wb_undo.iter().any(|u| u.0 == wid);
                if ui.add_enabled(can_undo, egui::Button::new("Undo")).clicked() {
                    self.wb_undo(wid);
                }
                // the question belongs to this board only
                let asking = self.sweep.confirm.clone().and_then(|c| c.strip_suffix(&format!(":{}", wid)).map(String::from));
                match asking.as_deref() {
                    Some("wb_clear") | Some("wb_delete") => {
                        let clear = asking.as_deref() == Some("wb_clear");
                        ui.label(RichText::new(if clear { "Clear the whole board?" } else { "Move this whiteboard to Trash?" }).color(t.danger));
                        if ui.button("Yes").clicked() {
                            self.sweep.confirm = None;
                            if clear {
                                let wb = &mut self.data.extra.whiteboards[wi];
                                wb.strokes.clear();
                                wb.notes.clear();
                                self.sweep.wb_undo.retain(|u| u.0 != wid);
                                self.dirty = true;
                            } else {
                                delete = true;
                            }
                        }
                        if ui.button("No").clicked() {
                            self.sweep.confirm = None;
                        }
                    }
                    _ => {
                        if ui.button("Clear").clicked() {
                            self.sweep.confirm = Some(format!("wb_clear:{}", wid));
                        }
                        if ui.add(egui::Button::new(RichText::new("Delete").color(t.danger)).frame(false)).clicked() {
                            self.sweep.confirm = Some(format!("wb_delete:{}", wid));
                        }
                    }
                }
            });
            let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 8.0, t.column_bg);
            // dot grid
            let step = 24.0;
            let mut y = rect.top() + step;
            while y < rect.bottom() {
                let mut x = rect.left() + step;
                while x < rect.right() {
                    p.circle_filled(pos2(x, y), 1.0, t.border);
                    x += step;
                }
                y += step;
            }
            let origin = rect.min;
            let wb = &mut self.data.extra.whiteboards[wi];
            for s in &wb.strokes {
                let pts: Vec<egui::Pos2> = s.points.iter().map(|q| origin + vec2(q[0], q[1])).collect();
                p.add(egui::Shape::line(pts, Stroke::new(s.width, rgb(s.color))));
            }
            let mut drag_note = None;
            let mut note_gone = None;
            let mut recolor = None;
            for (i, n) in wb.notes.iter_mut().enumerate() {
                let r = Rect::from_min_size(origin + vec2(n.pos[0], n.pos[1]), vec2(160.0, 120.0));
                p.rect_filled(r.translate(vec2(2.0, 3.0)), 4.0, Color32::from_black_alpha(25));
                p.rect_filled(r, 4.0, rgb(n.color).gamma_multiply(0.35).to_opaque());
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(r.min + vec2(8.0, 18.0), r.max - vec2(8.0, 8.0))));
                if child.add(egui::TextEdit::multiline(&mut n.text).frame(false).desired_width(144.0).desired_rows(4)).changed() {
                    self.dirty = true;
                }
                if self.wb_tool == 2 {
                    let h = ui.interact(r, ui.id().with(("note", i)), Sense::drag());
                    if h.dragged() {
                        drag_note = Some((i, h.drag_delta()));
                    }
                }
                // a colour dot and a close mark along the note's top edge
                let dot = Rect::from_center_size(r.left_top() + vec2(12.0, 10.0), vec2(14.0, 14.0));
                let dr = ui.interact(dot, ui.id().with(("note_color", wid, i)), Sense::click());
                name_it(&dr, &format!("Note colour {}", i + 1));
                p.circle_filled(dot.center(), 5.0, rgb(n.color));
                p.circle_stroke(dot.center(), 5.0, Stroke::new(1.0, Color32::from_black_alpha(60)));
                popup(ui, &dr, ui.id().with(("note_color_pop", wid, i)), 150.0, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for c in WB_COLORS.iter().skip(1).chain(STATUS_PALETTE.iter().skip(5).step_by(2)) {
                            let (cr, cresp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                            ui.painter().circle_filled(cr.center(), 8.0, rgb(*c));
                            if *c == n.color {
                                ui.painter().circle_stroke(cr.center(), 10.0, Stroke::new(1.5, t.text));
                            }
                            if cresp.clicked() {
                                recolor = Some((i, *c));
                                close_popup(ui);
                            }
                        }
                    });
                });
                let x = Rect::from_center_size(r.right_top() + vec2(-12.0, 10.0), vec2(14.0, 14.0));
                let xr = ui.interact(x, ui.id().with(("note_x", wid, i)), Sense::click());
                name_it(&xr, &format!("Delete note {}", i + 1));
                p.text(x.center(), Align2::CENTER_CENTER, "\u{2716}", FontId::proportional(10.0), if xr.hovered() { t.danger } else { t.text3 });
                if xr.clicked() {
                    note_gone = Some(i);
                }
            }
            if let Some((i, c)) = recolor {
                wb.notes[i].color = c;
                self.dirty = true;
            }
            if let Some((i, d)) = drag_note {
                wb.notes[i].pos[0] += d.x;
                wb.notes[i].pos[1] += d.y;
                self.dirty = true;
            }
            if let Some(i) = note_gone {
                let n = wb.notes.remove(i);
                self.sweep.wb_undo.push((wid, WbUndo::NoteGone(i, n)));
                self.dirty = true;
            }
            if let Some(pos) = resp.interact_pointer_pos() {
                let local = pos - origin;
                match self.wb_tool {
                    0 => {
                        if resp.drag_started() {
                            wb.strokes.push(WbStroke { color: self.wb_color, width: self.sweep.wb_width, points: vec![[local.x, local.y]] });
                            self.sweep.wb_undo.push((wid, WbUndo::Stroke));
                        } else if resp.dragged() {
                            if let Some(s) = wb.strokes.last_mut() {
                                s.points.push([local.x, local.y]);
                            }
                        }
                        if resp.drag_stopped() {
                            self.dirty = true;
                        }
                    }
                    1 => {
                        if resp.clicked() {
                            wb.notes.push(Sticky { pos: [local.x - 80.0, local.y - 60.0], text: String::new(), color: if self.wb_color == [32, 32, 32] { [0xf8, 0xae, 0x00] } else { self.wb_color } });
                            self.sweep.wb_undo.push((wid, WbUndo::Note));
                            self.dirty = true;
                        }
                    }
                    3 => {
                        // the eraser takes away whole strokes it touches
                        if resp.dragged() || resp.clicked() {
                            let hit = |s: &WbStroke| s.points.iter().any(|q| vec2(q[0] - local.x, q[1] - local.y).length() < s.width + 6.0);
                            while let Some(i) = wb.strokes.iter().position(hit) {
                                let s = wb.strokes.remove(i);
                                self.sweep.wb_undo.push((wid, WbUndo::StrokeGone(i, s)));
                                self.dirty = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if delete {
                let w = self.data.extra.whiteboards[wi].clone();
                self.to_trash("Whiteboard", &w.name, &w);
                self.data.extra.whiteboards.remove(wi);
                self.data.extra.starred.retain(|s| s != &format!("wb:{}", wid));
                self.sweep.wb_undo.retain(|u| u.0 != wid);
                self.wb_open = None;
                self.dirty = true;
            }
        });
    }

    /// Takes back the last pen stroke, note, note removal or erasing on a board.
    pub(super) fn wb_undo(&mut self, wid: u64) {
        let Some(k) = self.sweep.wb_undo.iter().rposition(|u| u.0 == wid) else { return };
        let (_, op) = self.sweep.wb_undo.remove(k);
        let Some(wb) = self.data.extra.whiteboards.iter_mut().find(|w| w.id == wid) else { return };
        match op {
            WbUndo::Stroke => {
                wb.strokes.pop();
            }
            WbUndo::Note => {
                wb.notes.pop();
            }
            WbUndo::NoteGone(i, n) => wb.notes.insert(i.min(wb.notes.len()), n),
            WbUndo::StrokeGone(i, s) => wb.strokes.insert(i.min(wb.strokes.len()), s),
        }
        self.dirty = true;
    }

    // ---------------------------------------------------------------- reminders

    pub(super) fn reminders_block(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let now = now_ts();
        let show_done = self.sweep.reminders_done;
        let mine: Vec<Reminder> = self.data.extra.reminders.iter().filter(|r| r.owner == me).cloned().collect();
        let n_done = mine.iter().filter(|r| r.done).count();
        let mut rs: Vec<Reminder> = mine.into_iter().filter(|r| show_done || !r.done).collect();
        if rs.is_empty() && n_done == 0 {
            return;
        }
        rs.sort_by_key(|r| (r.done, r.due_ts));
        ui.horizontal(|ui| {
            ui.label(RichText::new("Reminders").strong());
            if n_done > 0 {
                let b = ui.add(egui::Button::new(RichText::new(if show_done { "Hide done".to_string() } else { format!("Show done ({})", n_done) }).small().color(t.text2)).frame(false));
                name_it(&b, "Show done reminders");
                if b.clicked() {
                    self.sweep.reminders_done = !show_done;
                }
            }
        });
        let mut delete = None;
        for r in rs {
            if let Some((id, text, date, time)) = self.sweep.reminder_edit.as_mut().filter(|e| e.0 == r.id) {
                let id = *id;
                let mut save = false;
                let mut cancel = false;
                ui.horizontal(|ui| {
                    let te = ui.add(egui::TextEdit::singleline(text).desired_width(220.0));
                    name_it(&te, "Reminder text");
                    let b = ui.button(date.map(fmt_date).unwrap_or_else(|| "\u{1F4C5} Date".into()));
                    popup(ui, &b, ui.id().with(("rem_edit_date", id)), 380.0, |ui| date_menu(ui, &t, ui.id().with(("rem_edit_cal", id)), date));
                    ui.add(egui::TextEdit::singleline(time).hint_text("09:00").desired_width(54.0));
                    save = ui.button("Save").clicked() || (te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                    cancel = ui.button("Cancel").clicked();
                });
                if save && !text.trim().is_empty() {
                    let ts = reminder_ts(date.unwrap_or_else(today), time);
                    let text = text.trim().to_string();
                    if let Some(x) = self.data.extra.reminders.iter_mut().find(|x| x.id == id) {
                        x.text = text;
                        x.due_ts = ts;
                        self.dirty = true;
                    }
                    self.sweep.reminders_alerted.remove(&id);
                    self.sweep.reminder_edit = None;
                } else if cancel {
                    self.sweep.reminder_edit = None;
                }
                continue;
            }
            ui.horizontal(|ui| {
                let mut done = r.done;
                let cb = ui.checkbox(&mut done, "");
                name_it(&cb, &format!("Done {}", r.text));
                if cb.changed() {
                    if let Some(x) = self.data.extra.reminders.iter_mut().find(|x| x.id == r.id) {
                        x.done = done;
                        self.dirty = true;
                    }
                }
                ui.label(if r.done { RichText::new(&r.text).strikethrough().color(t.text3) } else { RichText::new(&r.text) });
                let when = chrono::DateTime::from_timestamp(r.due_ts, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %-I:%M %p").to_string()).unwrap_or_default();
                ui.label(RichText::new(when).small().color(if r.due_ts < now && !r.done { t.danger } else { t.text3 }));
                if !r.done {
                    let sz = ui.add(egui::Button::new(RichText::new("Snooze").small()).frame(false));
                    name_it(&sz, &format!("Snooze {}", r.text));
                    popup(ui, &sz, ui.id().with(("rem_snooze", r.id)), 170.0, |ui| {
                        let tomorrow = reminder_ts(today() + Duration::days(1), "09:00");
                        let week = reminder_ts(today() + Duration::days(7), "09:00");
                        for (label, ts) in [("In 1 hour", now + 3600), ("Tomorrow 9 AM", tomorrow), ("Next week", week)] {
                            if ui.button(label).clicked() {
                                if let Some(x) = self.data.extra.reminders.iter_mut().find(|x| x.id == r.id) {
                                    x.due_ts = ts;
                                    self.dirty = true;
                                }
                                self.sweep.reminders_alerted.remove(&r.id);
                                close_popup(ui);
                            }
                        }
                    });
                }
                let ed = ui.add(egui::Button::new(RichText::new("Edit").small()).frame(false));
                name_it(&ed, &format!("Edit {}", r.text));
                if ed.clicked() {
                    let local = chrono::DateTime::from_timestamp(r.due_ts, 0).map(|d| d.with_timezone(&chrono::Local));
                    let date = local.map(|d| d.date_naive());
                    let time = local.map(|d| d.format("%H:%M").to_string()).unwrap_or_else(|| "09:00".into());
                    self.sweep.reminder_edit = Some((r.id, r.text.clone(), date, time));
                }
                let del = ui.add(egui::Button::new(RichText::new("\u{2716}").small().color(t.text3)).frame(false));
                name_it(&del, &format!("Delete {}", r.text));
                if del.clicked() {
                    delete = Some(r.id);
                }
            });
        }
        if let Some(id) = delete {
            self.data.extra.reminders.retain(|x| x.id != id);
            self.dirty = true;
        }
        ui.add_space(10.0);
    }

    pub(super) fn due_reminders(&self) -> usize {
        let now = now_ts();
        self.data.extra.reminders.iter().filter(|r| r.owner == self.data.me && !r.done && r.due_ts <= now).count()
    }

    // ---------------------------------------------------------------- AI

    pub(super) fn ai_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(40, 40)).show(ui, |ui| {
            ui.label(RichText::new("\u{2728} AI").size(24.0));
            ui.add_space(8.0);
            ui.label(RichText::new("Ask, Build, Create, Skills, Artifacts, Super Agents, AI Chats and the AI Notetaker need an AI service.").color(t.text2));
            ui.label(RichText::new("They will be connected together with the shared server. Everything else works now.").color(t.text2));
            ui.add_space(16.0);
            // read-only rather than disabled: a disabled field fades its hint
            // to an unreadable grey on the dark theme
            ui.add(egui::TextEdit::multiline(&mut String::new()).interactive(false).hint_text(RichText::new("Ask anything... (available with the server)").color(t.text3)).desired_rows(3).desired_width(600.0));
        });
    }

    // ---------------------------------------------------------------- overview view

    pub(super) fn overview_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let lists = self.data.lists_in(self.loc).unwrap_or_default();
        // top row: Customize (turn cards on and off) and Add card
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let hidden: Vec<String> = self.data.extra.hidden_cards.clone();
                let add = ui.add_enabled(!hidden.is_empty(), egui::Button::new("+ Add card"));
                popup(ui, &add, egui::Id::new("ov_add_card"), 180.0, |ui| {
                    for c in &hidden {
                        let b = ui.button(c);
                        name_it(&b, &format!("Add card {}", c));
                        if b.clicked() {
                            self.data.extra.hidden_cards.retain(|x| x != c);
                            self.dirty = true;
                            close_popup(ui);
                        }
                    }
                });
                let cz = ui.button("\u{2699}  Customize");
                popup(ui, &cz, egui::Id::new("ov_customize"), 200.0, |ui| {
                    ui.label(RichText::new("Cards").small().color(t.text3));
                    for c in OV_CARDS {
                        let mut on = !self.data.extra.hidden_cards.iter().any(|x| x == c);
                        let cb = ui.checkbox(&mut on, c);
                        name_it(&cb, &format!("Card {}", c));
                        if cb.changed() {
                            if on {
                                self.data.extra.hidden_cards.retain(|x| x != c);
                            } else {
                                self.data.extra.hidden_cards.push(c.to_string());
                            }
                            self.dirty = true;
                        }
                    }
                });
            });
        });
        let card_on = |s: &Self, c: &str| !s.data.extra.hidden_cards.iter().any(|x| x == c);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            let w = ui.available_width();
            let top: Vec<&str> = ["Recent", "Docs", "Bookmarks"].into_iter().filter(|c| card_on(self, c)).collect();
            let cw = (w - 12.0 * (top.len().max(1) as f32 - 1.0)) / top.len().max(1) as f32;
            ui.horizontal_top(|ui| {
                // Recent
                if card_on(self, "Recent") {
                self.ov_card(ui, cw, "Recent", |s, ui| {
                    let tt = s.t();
                    let items = s.recent_items(Some(&lists));
                    if items.is_empty() {
                        ui.label(RichText::new("Your recently opened items will appear here.").color(tt.text2));
                    }
                    for (kind, id, name, place) in items.into_iter().take(8) {
                        ui.horizontal(|ui| {
                            let glyph = match kind.as_str() {
                                "task" => "\u{2611}",
                                "doc" => "\u{1F4C4}",
                                _ => "\u{2630}",
                            };
                            ui.label(RichText::new(glyph).color(tt.text3));
                            if ui.link(&name).clicked() {
                                match kind.as_str() {
                                    "task" => s.actions.push(Action::Open(id)),
                                    "doc" => {
                                        s.doc_open = Some(id);
                                        s.section = Section::Docs;
                                    }
                                    _ => s.actions.push(Action::Go(Location::List(id))),
                                }
                            }
                            ui.label(RichText::new(format!("\u{2022} in {}", place)).small().color(tt.text3));
                        });
                    }
                });
                }
                // Docs
                let space = self.current_space();
                if card_on(self, "Docs") {
                self.ov_card(ui, cw, "Docs", |s, ui| {
                    let docs: Vec<(u64, String)> = s.data.docs.iter().filter(|d| (space.is_none() || d.space == space) && d.parent.is_none() && !d.archived).map(|d| (d.id, d.title.clone())).collect();
                    if docs.is_empty() {
                        ui.label(RichText::new("There are no Docs in this location yet.").color(s.t().text2));
                    }
                    for (id, n) in docs {
                        if ui.link(format!("\u{1F4C4} {}", n)).clicked() {
                            s.doc_open = Some(id);
                            s.section = Section::Docs;
                        }
                    }
                    ui.add_space(6.0);
                    if primary_button(ui, &s.t(), "Add a Doc").clicked() {
                        s.new_doc(space);
                    }
                });
                }
                // Bookmarks
                if card_on(self, "Bookmarks") {
                self.ov_card(ui, cw, "Bookmarks", |s, ui| {
                    let bms = s.data.extra.bookmarks.clone();
                    if bms.is_empty() {
                        ui.label(RichText::new("Bookmarks make it easy to save items or any URL from around the web.").color(s.t().text2));
                    }
                    let mut remove = None;
                    for b in &bms {
                        ui.horizontal(|ui| {
                            if ui.link(format!("\u{1F516} {}", b.title)).on_hover_text(&b.url).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(&b.url));
                            }
                            if ui.add(egui::Button::new(RichText::new("\u{2716}").size(10.0).color(s.t().text3)).frame(false)).clicked() {
                                remove = Some(b.id);
                            }
                        });
                    }
                    if let Some(id) = remove {
                        s.data.extra.bookmarks.retain(|b| b.id != id);
                        s.dirty = true;
                    }
                    ui.add_space(6.0);
                    if primary_button(ui, &s.t(), "Add Bookmark").clicked() {
                        s.modal = Some(Modal::Bookmark { title: String::new(), url: String::new() });
                    }
                });
                }
            });
            ui.add_space(12.0);
            // Folders
            let folders: Vec<(u64, String, usize)> = match self.loc {
                Location::Space(sid) => self.data.spaces.iter().filter(|s| s.id == sid).flat_map(|s| s.folders.iter().map(|f| (f.id, f.name.clone(), f.lists.len()))).collect(),
                _ => vec![],
            };
            if matches!(self.loc, Location::Space(_)) && card_on(self, "Folders") {
                self.ov_card(ui, w, "Folders", |s, ui| {
                    if folders.is_empty() {
                        ui.label(RichText::new("Add new Folder to your Space").color(s.t().text2));
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (id, n, c) in &folders {
                            if ui.add(egui::Button::new(format!("\u{1F4C1} {}  ({} lists)", n, c)).min_size(vec2(200.0, 40.0)).stroke(Stroke::new(1.0, s.t().border))).clicked() {
                                s.actions.push(Action::Go(Location::Folder(*id)));
                            }
                        }
                    });
                    if let Location::Space(sid) = s.loc {
                        ui.add_space(6.0);
                        if primary_button(ui, &s.t(), "Add Folder").clicked() {
                            s.actions.push(Action::NewFolder(sid));
                        }
                    }
                });
                ui.add_space(12.0);
            }
            // Lists: Name, Color, Progress, Start, End, Priority, Owner
            if card_on(self, "Lists") {
                self.ov_card(ui, w, "Lists", |s, ui| s.ov_lists(ui, w - 34.0, &lists));
            }
        });
    }

    fn ov_lists(&mut self, ui: &mut Ui, w: f32, lists: &[u64]) {
        let tt = self.t();
        const COLS: [(f32, &str); 7] = [(0.0, "Name"), (0.28, "Color"), (0.36, "Progress"), (0.55, "Start"), (0.66, "End"), (0.77, "Priority"), (0.88, "Owner")];
        let (hr, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
        for (x, h) in COLS {
            ui.painter().text(pos2(hr.left() + w * x, hr.center().y), Align2::LEFT_CENTER, h, FontId::proportional(12.5), tt.text2);
        }
        let cell = |r: Rect, i: usize| {
            let x0 = r.left() + w * COLS[i].0;
            let x1 = if i + 1 < COLS.len() { r.left() + w * COLS[i + 1].0 } else { r.right() };
            Rect::from_min_max(pos2(x0, r.top()), pos2(x1 - 6.0, r.bottom()))
        };
        for lid in lists {
            let Some(item) = self.data.list_item(*lid).cloned().filter(|l| !l.archived) else { continue };
            let total = self.data.tasks.iter().filter(|x| x.list_id == *lid && x.parent.is_none() && !x.archived).count();
            let done = self.data.tasks.iter().filter(|x| x.list_id == *lid && x.parent.is_none() && !x.archived && self.data.is_closed(x)).count();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
            name_it(&resp, &format!("Overview list {}", item.name));
            if resp.hovered() {
                ui.painter().rect_filled(r, 5.0, tt.hover);
            }
            let p = ui.painter().clone();
            p.with_clip_rect(cell(r, 0)).text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, &item.name, FontId::proportional(14.0), tt.text);
            let mut edited = item.clone();
            // color
            let c0 = cell(r, 1);
            let cr = ui.interact(c0, ui.id().with(("ov_color", lid)), Sense::click());
            name_it(&cr, &format!("Color of {}", item.name));
            match item.color {
                Some(c) => {
                    p.circle_filled(pos2(c0.left() + 8.0, c0.center().y), 6.0, rgb(c));
                }
                None => {
                    p.circle_stroke(pos2(c0.left() + 8.0, c0.center().y), 6.0, Stroke::new(1.0, tt.text3));
                }
            }
            popup(ui, &cr, ui.id().with(("ov_color_p", lid)), 160.0, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for c in STATUS_PALETTE.iter().skip(1) {
                        let (b, br) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                        ui.painter().circle_filled(b.center(), 8.0, rgb(*c));
                        if br.clicked() {
                            edited.color = Some(*c);
                            close_popup(ui);
                        }
                    }
                });
                if ui.small_button("No color").clicked() {
                    edited.color = None;
                    close_popup(ui);
                }
            });
            // progress
            let c2 = cell(r, 2);
            let bar = Rect::from_min_size(pos2(c2.left(), c2.center().y - 3.0), vec2((c2.width() - 40.0).max(20.0), 6.0));
            p.rect_filled(bar, 3.0, tt.hover);
            if total > 0 {
                p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * done as f32 / total as f32, 6.0)), 3.0, rgb(COMPLETE_COLOR));
            }
            p.text(pos2(bar.right() + 6.0, c2.center().y), Align2::LEFT_CENTER, format!("{}/{}", done, total), FontId::proportional(12.0), tt.text2);
            // start / end
            for (i, which) in [(3usize, 0u8), (4, 1)] {
                let c = cell(r, i);
                let dr = ui.interact(c, ui.id().with(("ov_date", lid, which)), Sense::click());
                name_it(&dr, &format!("{} of {}", if which == 0 { "Start" } else { "End" }, item.name));
                let v = if which == 0 { item.start } else { item.end };
                match v {
                    Some(d) => {
                        p.text(c.left_center(), Align2::LEFT_CENTER, fmt_date(d), FontId::proportional(13.0), tt.text);
                    }
                    None => {
                        p.text(c.left_center(), Align2::LEFT_CENTER, "\u{2013}", FontId::proportional(13.0), tt.text3);
                    }
                }
                let mut val = v;
                if let Some(true) = popup(ui, &dr, ui.id().with(("ov_date_p", lid, which)), 300.0, |ui| date_menu(ui, &tt, ui.id().with(("ov_cal", lid, which)), &mut val)) {
                    if which == 0 {
                        edited.start = val;
                    } else {
                        edited.end = val;
                    }
                }
            }
            // priority
            let c5 = cell(r, 5);
            let prr = ui.interact(c5, ui.id().with(("ov_pr", lid)), Sense::click());
            name_it(&prr, &format!("Priority of {}", item.name));
            if item.priority > 0 {
                paint_flag(&p, c5.left_center() + vec2(4.0, 0.0), priority_color(item.priority, &tt), 12.0);
                p.text(c5.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, PRIORITIES[item.priority as usize].label, FontId::proportional(13.0), tt.text);
            } else {
                p.text(c5.left_center(), Align2::LEFT_CENTER, "\u{2013}", FontId::proportional(13.0), tt.text3);
            }
            let mut pv = item.priority;
            if let Some(true) = popup(ui, &prr, ui.id().with(("ov_pr_p", lid)), 180.0, |ui| priority_menu(ui, &tt, &mut pv)) {
                edited.priority = pv;
            }
            // owner
            let c6 = cell(r, 6);
            let orr = ui.interact(c6, ui.id().with(("ov_owner", lid)), Sense::click());
            name_it(&orr, &format!("Owner of {}", item.name));
            match item.owner.and_then(|o| self.data.member(o)) {
                Some(m) => paint_avatar(&p, c6.left_center() + vec2(10.0, 0.0), 10.0, m, tt.card),
                None => paint_empty_avatar(&p, c6.left_center() + vec2(10.0, 0.0), 10.0, tt.text3),
            }
            let members = self.data.members.clone();
            popup(ui, &orr, ui.id().with(("ov_owner_p", lid)), 200.0, |ui| {
                for m in &members {
                    let ring = tt.card;
                    if menu_row(ui, &tt, &m.name, item.owner == Some(m.id), |p, c| paint_avatar(p, c, 8.0, m, ring)).clicked() {
                        edited.owner = if item.owner == Some(m.id) { None } else { Some(m.id) };
                        close_popup(ui);
                    }
                }
            });
            let any_cell = [cr.hovered(), prr.hovered(), orr.hovered()].iter().any(|x| *x) || [c0, cell(r, 3), cell(r, 4), c5, c6].iter().any(|c| ui.rect_contains_pointer(*c));
            let changed = edited.color != item.color || edited.start != item.start || edited.end != item.end || edited.priority != item.priority || edited.owner != item.owner;
            if changed {
                if let Some(l) = self.data.list_item_mut(*lid) {
                    *l = edited;
                }
                self.dirty = true;
            } else if resp.clicked() && !any_cell {
                self.actions.push(Action::Go(Location::List(*lid)));
            }
        }
        ui.add_space(6.0);
        if ui.add(egui::Button::new(RichText::new("+ New List").color(tt.text2)).frame(false)).clicked() {
            let space = self.current_space();
            let folder = if let Location::Folder(f) = self.loc { Some(f) } else { None };
            self.open_new_list(space, folder);
        }
    }

    fn ov_card(&mut self, ui: &mut Ui, w: f32, title: &str, add: impl FnOnce(&mut Self, &mut Ui)) {
        let t = self.t();
        egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(w - 34.0);
                ui.set_min_height(150.0);
                ui.label(RichText::new(title).size(15.0));
                ui.add_space(8.0);
                add(self, ui);
            });
        });
    }
}

/// A local date and "HH:MM" as a timestamp (bad times fall back to 9:00).
pub fn reminder_ts(d: NaiveDate, time: &str) -> i64 {
    let (h, m) = time.split_once(':').map(|(h, m)| (h.trim().parse().unwrap_or(9), m.trim().parse().unwrap_or(0))).unwrap_or((9, 0));
    d.and_hms_opt(u32::min(h, 23), u32::min(m, 59), 0).and_then(|x| x.and_local_timezone(chrono::Local).single()).map(|x| x.timestamp()).unwrap_or_else(now_ts)
}

/// A notification from the system (macOS), next to the in-app toast.
fn os_notify(title: &str, body: &str) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let q = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("display notification \"{}\" with title \"{}\"", q(body), q(title));
    let _ = std::process::Command::new("osascript").arg("-e").arg(script).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
}

// Apps sweep: Home, Planner, Reminders, Notepad, Trash and the hubs.
impl App {
    /// A star that adds the item ("wb:ID", "form:ID", "dash:ID") to Favorites.
    pub(super) fn star_button(&mut self, ui: &mut Ui, key: &str) {
        let t = self.t();
        let starred = self.data.extra.starred.iter().any(|s| s == key);
        let star = RichText::new(if starred { "\u{2605}" } else { "\u{2606}" }).size(16.0).color(if starred { Color32::from_rgb(0xf8, 0xae, 0x00) } else { t.text2 });
        let b = ui.add(egui::Button::new(star).frame(false)).on_hover_text(if starred { "Remove from Favorites" } else { "Add to Favorites" });
        name_it(&b, if starred { "Unstar" } else { "Star" });
        if b.clicked() {
            if starred {
                self.data.extra.starred.retain(|s| s != key);
            } else {
                self.data.extra.starred.push(key.to_string());
            }
            self.dirty = true;
        }
    }

    /// Keeps a deleted item in Trash so it can be restored.
    pub(super) fn to_trash<T: serde::Serialize>(&mut self, kind: &str, title: &str, item: &T) {
        let id = self.data.new_id();
        self.data.extra.trash.push(TrashItem { id, kind: kind.into(), title: title.into(), deleted: now_ts(), payload: serde_json::to_string(item).unwrap_or_default() });
    }

    /// Adds a dashboard owned by me and opens it.
    pub(super) fn new_dashboard(&mut self, name: &str) {
        let id = self.data.new_id();
        self.data.extra.dashboards.push(Dashboard { id, name: name.to_string(), owner: self.data.me, ..Default::default() });
        self.dash_open = Some(id);
        self.dirty = true;
    }

    /// Ticks a task done (its list's closed status) or opens it again.
    pub(super) fn toggle_done(&mut self, id: u64) {
        let Some(mut task) = self.task(id).cloned() else { return };
        let sts = self.data.statuses_for_list(task.list_id);
        if self.data.is_closed(&task) {
            task.status = sts[0].name.clone();
        } else if let Some(done) = sts.iter().find(|s| s.kind == StatusKind::Closed).or_else(|| sts.iter().find(|s| s.kind == StatusKind::Done)) {
            task.status = done.name.clone();
        }
        self.actions.push(Action::Replace(task));
    }

    /// "+ Add task" at the bottom of a Planner day: a task for me due that day.
    pub(super) fn planner_add_row(&mut self, ui: &mut Ui, d: NaiveDate, w: f32) {
        let t = self.t();
        let Some((_, text)) = self.sweep.planner_add.as_mut().filter(|p| p.0 == d) else {
            let b = ui.add(egui::Button::new(RichText::new("+ Add task").color(t.text3)).frame(false));
            name_it(&b, &format!("Add task on {}", d.format("%a %-d")));
            if b.clicked() {
                self.sweep.planner_add = Some((d, String::new()));
            }
            return;
        };
        let r = ui.add(egui::TextEdit::singleline(text).hint_text("Task name").desired_width(w - 8.0));
        if r.lost_focus() {
            let name = text.trim().to_string();
            self.sweep.planner_add = None;
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.is_empty() {
                if let Some(list) = self.default_list() {
                    let me = self.data.me;
                    self.actions.push(Action::Add { list, name, status: None, priority: 0, assignees: vec![me], due: Some(d), parent: None, open: false });
                }
            }
        } else {
            r.request_focus();
        }
    }

    /// Announces reminders as they fall due (once each), if the person
    /// wants that (Notifications > A reminder is due) and isn't muted.
    pub(super) fn reminder_tick(&mut self, ctx: &egui::Context) {
        let now = now_ts();
        let me = self.data.me;
        let mine = || self.data.extra.reminders.iter().filter(|r| r.owner == me && !r.done);
        if let Some(next) = mine().filter(|r| r.due_ts > now).map(|r| r.due_ts).min() {
            ctx.request_repaint_after(std::time::Duration::from_secs((next - now).clamp(1, 3600) as u64));
        }
        let due: Vec<(u64, String)> = mine().filter(|r| r.due_ts <= now && !self.sweep.reminders_alerted.contains(&r.id)).map(|r| (r.id, r.text.clone())).collect();
        if due.is_empty() {
            return;
        }
        self.sweep.reminders_alerted.extend(due.iter().map(|d| d.0));
        if !self.data.extra.notify.reminders || self.data.extra.muted_until > now {
            return;
        }
        let msg = if due.len() == 1 { format!("Reminder: {}", due[0].1) } else { format!("{} reminders are due", due.len()) };
        self.toast = Some((msg.clone(), now));
        if self.persist {
            os_notify("Workboard", &msg);
        }
    }
}

/// Where a search result leads.
#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    Go(Location),
    Task(u64),
    Doc(u64),
    Chat(u64),
    Whiteboard(u64),
    Form(u64),
    Dashboard(u64),
    Goal(u64),
    Person(u64),
    /// A recent search: put it back in the box.
    Query(String),
}

/// One search result row: (group, glyph, title, place, where it leads).
pub type SearchRow = (&'static str, &'static str, String, String, Hit);

/// Kind chips over the results.
pub const SEARCH_KINDS: [&str; 8] = ["All", "Tasks", "Lists", "Docs", "Channels", "Messages", "Apps", "People"];

impl App {
    /// Everything the search box finds, grouped, best first. A pasted
    /// workboard:// link (Copy link) or a task number finds that item.
    pub(super) fn search_rows(&self, q: &str, kind: u8) -> Vec<SearchRow> {
        let ql = q.trim().to_lowercase();
        let show = |k: u8| kind == 0 || kind == k;
        let many = |k: u8| if kind == k { 60 } else { 8 };
        let mut rows: Vec<SearchRow> = vec![];
        if let Some((k, id)) = parse_wb_link(q) {
            let hit = match k {
                "task" => self.task(id).map(|t| ("\u{2611}", t.name.clone(), self.data.list_name(t.list_id), Hit::Task(id))),
                "list" => self.data.location_exists(Location::List(id)).then(|| ("\u{2630}", self.data.list_name(id), self.data.space_of_list(id).map(|s| s.name.clone()).unwrap_or_default(), Hit::Go(Location::List(id)))),
                "folder" => self.data.location_exists(Location::Folder(id)).then(|| ("\u{1F4C1}", self.data.location_name(Location::Folder(id)), "Folder".into(), Hit::Go(Location::Folder(id)))),
                _ => self.data.location_exists(Location::Space(id)).then(|| ("\u{25A0}", self.data.location_name(Location::Space(id)), "Space".into(), Hit::Go(Location::Space(id)))),
            };
            match hit {
                Some((g, title, place, hit)) => rows.push(("Link", g, title, place, hit)),
                None => rows.push(("Link", "\u{26A0}", "Nothing with this link here (it may have been deleted)".into(), String::new(), Hit::Query(String::new()))),
            }
            return rows;
        }
        if ql.is_empty() && kind == 0 {
            for (k, id, name, place) in self.recent_items(None).into_iter().take(8) {
                let (g, hit) = match k.as_str() {
                    "task" => ("\u{2611}", Hit::Task(id)),
                    "doc" => ("\u{1F4C4}", Hit::Doc(id)),
                    _ => ("\u{2630}", Hit::Go(Location::List(id))),
                };
                rows.push(("Recent", g, name, place, hit));
            }
            for r in self.sweep.search_recent.iter().take(5) {
                rows.push(("Recent searches", "\u{1F50D}", r.clone(), String::new(), Hit::Query(r.clone())));
            }
        }
        let has = |s: &str| ql.is_empty() || s.to_lowercase().contains(&ql);
        if show(2) {
            let mut locs: Vec<SearchRow> = vec![];
            for s in self.data.spaces.iter().filter(|s| !s.archived) {
                locs.push(("Locations", "\u{25A0}", s.name.clone(), "Space".into(), Hit::Go(Location::Space(s.id))));
                for f in &s.folders {
                    locs.push(("Locations", "\u{1F4C1}", f.name.clone(), s.name.clone(), Hit::Go(Location::Folder(f.id))));
                    for l in f.lists.iter().filter(|l| !l.archived) {
                        locs.push(("Locations", "\u{2630}", l.name.clone(), format!("{} / {}", s.name, f.name), Hit::Go(Location::List(l.id))));
                    }
                }
                for l in s.lists.iter().filter(|l| !l.archived) {
                    locs.push(("Locations", "\u{2630}", l.name.clone(), s.name.clone(), Hit::Go(Location::List(l.id))));
                }
            }
            rows.extend(locs.into_iter().filter(|r| has(&r.2) || has(&format!("{} / {}", r.3, r.2))).take(many(2)));
        }
        if show(1) {
            let tasks = self.data.tasks.iter().filter(|x| !x.archived && (has(&x.name) || x.id.to_string() == ql)).take(if kind == 1 { 60 } else { 20 });
            rows.extend(tasks.map(|x| ("Tasks", "\u{2611}", x.name.clone(), self.data.list_name(x.list_id), Hit::Task(x.id))));
        }
        if show(3) {
            let docs = self.data.docs.iter().filter(|d| !d.archived && (has(&d.title) || (ql.len() > 2 && d.body.to_lowercase().contains(&ql)))).take(many(3));
            rows.extend(docs.map(|d| ("Docs", "\u{1F4C4}", d.title.clone(), "Doc".into(), Hit::Doc(d.id))));
        }
        if show(4) {
            let chans = self.data.channels.iter().filter(|c| c.kind == ChannelKind::Channel && has(&c.name)).take(many(4));
            rows.extend(chans.map(|c| ("Channels", "#", c.name.clone(), "Channel".into(), Hit::Chat(c.id))));
        }
        if show(5) && ql.len() > 1 {
            let mut n = 0;
            'outer: for c in &self.data.channels {
                let place = if c.kind == ChannelKind::Direct { "Direct message".to_string() } else { format!("#{}", c.name) };
                for m in c.messages.iter().chain(c.messages.iter().flat_map(|m| m.replies.iter())) {
                    if m.text.to_lowercase().contains(&ql) {
                        rows.push(("Messages", "\u{1F4AC}", truncate(&m.text, 60), place.clone(), Hit::Chat(c.id)));
                        n += 1;
                        if n >= many(5) {
                            break 'outer;
                        }
                    }
                }
            }
        }
        if show(6) {
            let x = &self.data.extra;
            let mut apps: Vec<SearchRow> = vec![];
            apps.extend(x.whiteboards.iter().filter(|w| has(&w.name)).map(|w| ("Apps", "\u{1F3A8}", w.name.clone(), "Whiteboard".into(), Hit::Whiteboard(w.id))));
            apps.extend(x.forms.iter().filter(|f| has(&f.name)).map(|f| ("Apps", "\u{1F4CB}", f.name.clone(), "Form".into(), Hit::Form(f.id))));
            apps.extend(x.dashboards.iter().filter(|d| has(&d.name)).map(|d| ("Apps", "\u{1F4CA}", d.name.clone(), "Dashboard".into(), Hit::Dashboard(d.id))));
            apps.extend(x.goals.iter().filter(|g| has(&g.name)).map(|g| ("Apps", "\u{25CE}", g.name.clone(), "Goal".into(), Hit::Goal(g.id))));
            if !(ql.is_empty() && kind == 0) {
                rows.extend(apps.into_iter().take(many(6)));
            }
        }
        if show(7) && !(ql.is_empty() && kind == 0) {
            let people = self.data.members.iter().filter(|m| has(&m.name) || (!ql.is_empty() && m.email.to_lowercase().contains(&ql))).take(many(7));
            rows.extend(people.map(|m| ("People", "\u{1F464}", m.name.clone(), if m.email.is_empty() { m.role.clone() } else { m.email.clone() }, Hit::Person(m.id))));
        }
        rows
    }

    /// Goes where a search result leads.
    pub(super) fn open_hit(&mut self, hit: Hit) {
        match hit {
            Hit::Go(l) => self.actions.push(Action::Go(l)),
            Hit::Task(id) => self.actions.push(Action::Open(id)),
            Hit::Doc(id) => {
                self.doc_open = Some(id);
                self.section = Section::Docs;
            }
            Hit::Chat(id) => {
                self.section = Section::Chat;
                self.chat_open = Some(id);
                self.thread_open = None;
            }
            Hit::Whiteboard(id) => {
                self.wb_open = Some(id);
                self.section = Section::Whiteboards;
            }
            Hit::Form(id) => {
                self.form_open = Some(id);
                self.section = Section::Forms;
            }
            Hit::Dashboard(id) => {
                self.dash_open = Some(id);
                self.section = Section::Dashboards;
            }
            Hit::Goal(id) => {
                self.goal_open = Some(id);
                self.section = Section::Goals;
            }
            Hit::Person(_) => self.section = Section::People,
            Hit::Query(_) => {}
        }
    }

    /// Cmd+K / Cmd+J search: arrow keys pick a result, Enter opens it.
    pub(super) fn search_palette(&mut self, ctx: &egui::Context) {
        let Some(mut q) = self.palette.take() else { return };
        let t = self.t();
        let mut open = true;
        egui::Window::new("search_palette").title_bar(false).collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 60.0]).fixed_size([620.0, 420.0]).show(ctx, |ui| {
            // the arrows move the highlight, not the text cursor
            let (down, up) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown), i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)));
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search, or paste a link to a task, list or space...").font(FontId::proportional(16.0)).desired_width(f32::INFINITY));
            r.request_focus();
            let kid = egui::Id::new("palette_kind");
            let mut kind: u8 = ui.data(|d| d.get_temp(kid)).unwrap_or(0);
            ui.horizontal(|ui| {
                for (i, label) in SEARCH_KINDS.iter().enumerate() {
                    if ui.selectable_label(kind == i as u8, *label).clicked() {
                        kind = i as u8;
                    }
                }
            });
            ui.data_mut(|d| d.insert_temp(kid, kind));
            ui.separator();
            let rows = self.search_rows(&q, kind);
            let key = format!("{}\u{1}{}", kind, q);
            if self.sweep.search_q != key {
                self.sweep.search_q = key;
                self.sweep.search_sel = 0;
            }
            let pickable: Vec<usize> = (0..rows.len()).filter(|i| rows[*i].4 != Hit::Query(String::new())).collect();
            if !pickable.is_empty() {
                let pos = pickable.iter().position(|i| *i == self.sweep.search_sel).unwrap_or(0);
                let pos = if down { (pos + 1).min(pickable.len() - 1) } else if up { pos.saturating_sub(1) } else { pos };
                self.sweep.search_sel = pickable[pos];
            }
            let mut chosen: Option<Hit> = None;
            egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                let mut group = "";
                for (i, (g, glyph, title, place, hit)) in rows.iter().enumerate() {
                    if *g != group {
                        if !group.is_empty() {
                            ui.add_space(4.0);
                        }
                        group = g;
                        ui.label(RichText::new(*g).small().color(t.text3));
                    }
                    let w = ui.available_width();
                    let (rr, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
                    name_it(&resp, title);
                    let sel = i == self.sweep.search_sel && pickable.contains(&i);
                    if sel || resp.hovered() {
                        ui.painter().rect_filled(rr, 6.0, if sel { t.row_selected } else { t.hover });
                    }
                    if sel && (down || up) {
                        resp.scroll_to_me(None);
                    }
                    ui.painter().text(rr.left_center() + vec2(12.0, 0.0), Align2::CENTER_CENTER, *glyph, FontId::proportional(12.5), t.text3);
                    let tg = ui.painter().layout_no_wrap(title.clone(), FontId::proportional(14.0), t.text);
                    let tw = tg.size().x.min(w - 200.0);
                    ui.painter().with_clip_rect(Rect::from_min_max(rr.min, pos2(rr.left() + 28.0 + tw, rr.bottom()))).galley(pos2(rr.left() + 28.0, rr.center().y - tg.size().y / 2.0), tg, t.text);
                    ui.painter().with_clip_rect(rr).text(pos2(rr.left() + 40.0 + tw, rr.center().y), Align2::LEFT_CENTER, place, FontId::proportional(12.0), t.text3);
                    if resp.clicked() {
                        chosen = Some(hit.clone());
                    }
                }
                if rows.is_empty() && !q.trim().is_empty() {
                    ui.label(RichText::new("No results").color(t.text3));
                }
            });
            if enter && chosen.is_none() {
                if let Some(i) = pickable.iter().find(|i| **i == self.sweep.search_sel) {
                    chosen = Some(rows[*i].4.clone());
                }
                if chosen.is_none() {
                    open = false;
                }
            }
            match chosen {
                Some(Hit::Query(text)) => {
                    q = text;
                }
                Some(hit) => {
                    let typed = q.trim().to_string();
                    if !typed.is_empty() && parse_wb_link(&typed).is_none() {
                        self.sweep.search_recent.retain(|x| x != &typed);
                        self.sweep.search_recent.insert(0, typed);
                        self.sweep.search_recent.truncate(8);
                    }
                    self.open_hit(hit);
                    open = false;
                }
                None => {}
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                open = false;
            }
        });
        if open {
            self.palette = Some(q);
        }
    }
}
