// Pages reached from the sidebar, the rail and the menus: People, Posts,
// Meetings, Goals, Forms, Whiteboards, Reminders, AI and the Overview view.

use super::icons::{icon_button, icon_button_ex, line_icon, line_icon_scaled};
use super::menus::ROLES;
use super::*;

fn title(ui: &mut Ui, t: &Theme, text: &str, sub: &str) {
    ui.label(RichText::new(text).size(20.0));
    if !sub.is_empty() {
        ui.label(RichText::new(sub).color(t.text2));
    }
    ui.add_space(10.0);
}

fn list_panel_frame(t: &Theme) -> egui::Frame {
    egui::Frame::new()
        .fill(t.sidebar)
        .corner_radius(egui::CornerRadius {
            nw: 8,
            ne: 0,
            sw: 8,
            se: 0,
        })
        .inner_margin(egui::Margin::symmetric(10, 12))
}

/// Space Overview cards that Customize can turn off.
pub const OV_CARDS: [&str; 5] = ["Recent", "Docs", "Bookmarks", "Folders", "Lists"];

/// What the whiteboard's Undo takes back, newest last. Only my own actions
/// are kept here, by the id of the stroke or note, so a teammate's stroke
/// or note merged in meanwhile is never the one taken back.
pub enum WbUndo {
    /// I drew this stroke.
    Stroke(u64),
    /// I added this note.
    Note(u64),
    /// I deleted this note; it sat at this place in the list.
    NoteGone(usize, Sticky),
    /// I erased this stroke; it sat at this place in the list.
    StrokeGone(usize, WbStroke),
    /// I dragged this note away from this spot.
    NoteMoved(u64, [f32; 2]),
}

/// How a whiteboard is shown: board coordinates (where strokes and notes
/// are stored) map to the screen as `origin + pan + board * zoom`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WbView {
    pub pan: egui::Vec2,
    pub zoom: f32,
}

impl Default for WbView {
    fn default() -> Self {
        WbView {
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl WbView {
    pub const MIN_ZOOM: f32 = 0.2;
    pub const MAX_ZOOM: f32 = 4.0;

    pub fn to_screen(&self, origin: egui::Pos2, b: [f32; 2]) -> egui::Pos2 {
        origin + self.pan + vec2(b[0], b[1]) * self.zoom
    }

    pub fn to_board(&self, origin: egui::Pos2, p: egui::Pos2) -> [f32; 2] {
        let v = (p - origin - self.pan) / self.zoom;
        [v.x, v.y]
    }

    /// Zooms by `factor`, keeping the board point under `anchor` in place.
    pub fn zoom_at(&mut self, origin: egui::Pos2, anchor: egui::Pos2, factor: f32) {
        let before = self.to_board(origin, anchor);
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        let after = self.to_screen(origin, before);
        self.pan += anchor - after;
    }
}

/// Screen state of the app pages (not saved).
#[derive(Default)]
pub struct SweepUi {
    /// Reminders: show the done ones too.
    pub reminders_done: bool,
    /// The reminder being changed: (id, text, date, time).
    pub reminder_edit: Option<(u64, String, Option<NaiveDate>, String)>,
    /// Due reminders already announced this session.
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
    /// The pen stroke being drawn: (whiteboard, stroke id).
    pub wb_drawing: Option<(u64, u64)>,
    /// Each whiteboard's pan and zoom on screen (not saved).
    pub wb_view: HashMap<u64, WbView>,
    /// The form `form_fill` holds answers for.
    pub form_for: Option<u64>,
    pub form_error: Option<String>,
    /// Dashboard card being renamed: (index, text).
    pub card_rename: Option<(usize, String)>,
    /// Time entry being changed.
    pub entry_edit: Option<EntryEdit>,
}

/// A time entry open for changing in the timesheet list. The entry is found
/// again on every frame by whose it is and when it starts (what the team
/// merge knows an entry by), not by its place in the list: a merge can put a
/// teammate's earlier entry in front of it.
#[derive(Clone, Debug, Default)]
pub struct EntryEdit {
    pub task: u64,
    /// The entry's `member` as saved (0 on entries older apps wrote).
    pub member: u64,
    pub start: i64,
    /// The start time being typed, "HH:MM".
    pub at: String,
    pub hours: f64,
}

/// Pen and note colours on a whiteboard.
pub const WB_COLORS: [[u8; 3]; 6] = [
    [32, 32, 32],
    [0xd3, 0x3d, 0x44],
    [8, 128, 234],
    [41, 151, 100],
    [0xf8, 0xae, 0x00],
    [0x5f, 0x55, 0xee],
];

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
    FORM_KINDS
        .iter()
        .find(|x| x.0 == k)
        .map(|x| x.1)
        .unwrap_or("Short text")
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
        let v = fill
            .get(&i)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if v.is_empty() {
            if fld.required {
                return Err(trf!("Please answer \"{}\".", fld.label));
            }
            continue;
        }
        match fld.map.as_str() {
            "name" => a.name = v,
            "description" => a.description.push_str(&v),
            "priority" => {
                a.priority = PRIORITIES.iter().position(|p| p.label == v).unwrap_or(0) as u8
            }
            "due" => match NaiveDate::parse_from_str(&v, "%Y-%m-%d") {
                Ok(d) => a.due = Some(d),
                Err(_) => {
                    return Err(trf!(
                        "\"{}\" isn't a date. Write it as YYYY-MM-DD.",
                        fld.label
                    ))
                }
            },
            "assignee" => a.assignees.extend(v.parse::<u64>().ok()),
            k => {
                if k == "number" && v.parse::<f64>().is_err() {
                    return Err(trf!("\"{}\" needs a number.", fld.label));
                }
                if k == "email" && !(v.contains('@') && v.contains('.')) {
                    return Err(trf!("\"{}\" needs an email address.", fld.label));
                }
                a.description.push_str(&format!("\n{}: {}", fld.label, v));
            }
        }
    }
    Ok(a)
}

/// The standard dashboard cards, in order, with their titles.
pub const DASH_CARDS: [(&str, &str); 7] = [
    ("stats", "Task totals"),
    ("status", "Tasks by status"),
    ("priority", "Tasks by priority"),
    ("workload", "Workload by assignee"),
    ("time", "Time tracked this week"),
    ("time_people", "Time tracked this week by person"),
    ("overdue", "Overdue tasks"),
];

pub fn dash_card_title(c: &DashCard) -> String {
    if !c.title.is_empty() {
        return c.title.clone();
    }
    DASH_CARDS
        .iter()
        .find(|k| k.0 == c.kind)
        .map(|k| k.1.to_string())
        .unwrap_or_else(|| c.kind.clone())
}

/// The cards a dashboard shows until its cards are customised: those of its
/// template (`Dashboard::kind`; "" shows all but the per-person time card).
pub fn default_dash_cards(kind: &str) -> Vec<DashCard> {
    let on = |card: &str| match kind {
        "simple" => matches!(card, "stats" | "status" | "priority" | "overdue"),
        "time" => matches!(card, "time" | "time_people"),
        "project" => matches!(card, "stats" | "status" | "workload" | "overdue"),
        _ => card != "time_people",
    };
    DASH_CARDS
        .iter()
        .filter(|k| on(k.0))
        .map(|k| DashCard {
            kind: k.0.into(),
            title: String::new(),
        })
        .collect()
}

impl App {
    // ---------------------------------------------------------------- people

    pub(super) fn people_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        // in Settings the section title above already says "People", and the
        // body lines up with the other sections
        let in_settings = self.section == Section::Settings;
        let margin = if in_settings {
            egui::Margin::ZERO
        } else {
            egui::Margin::symmetric(24, 16)
        };
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if !in_settings {
                        ui.label(RichText::new(tr("People")).size(20.0));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.may_manage() && primary_button(ui, &t, tr("Invite people")).clicked() {
                            self.modal = Some(Modal::Invite { emails: String::new(), role: "Member".into() });
                        }
                    });
                });
                ui.label(RichText::new(trf!("Manage who can access {}", self.data.workspace)).color(t.text2));
                let server = self.data.extra.shared.as_ref().is_some_and(|c| c.server.is_some());
                if server && self.sync_rt.role.is_none() {
                    // the server says who may manage people, once it's reached
                    ui.label(RichText::new(tr("Changing roles and removing people need the team's server. They come back once it's connected.")).small().color(super::together::WARN));
                } else if server && self.sync_rt.role.is_some_and(|r| !r.manages()) {
                    // a member sees no Remove or Cancel invite buttons: say why
                    ui.label(RichText::new(tr("Only the owner or an admin can change roles, cancel invites and remove people.")).small().color(t.text3));
                }
                ui.add_space(12.0);
                let w = ui.available_width();
                let (hr, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
                // column starts as parts of the width; the last 108 px are the
                // Remove or Cancel invite button, so a small window narrows the columns instead
                let (ex, rx, sx) = (0.3, 0.52, 0.68);
                let col_w = |from: f32, to: f32| w * (to - from) - 16.0;
                for (x, h) in [(0.0, "Name"), (ex, "Email"), (rx, "Role"), (sx, "Status")] {
                    ui.painter().text(pos2(hr.left() + w * x + 8.0, hr.center().y), Align2::LEFT_CENTER, tr(h), FontId::proportional(13.0), t.text2);
                }
                ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
                let members = self.data.members.clone();
                let me = self.data.me;
                // on a team server only its owner and admins manage people, and
                // the server keeps the roles; alone or in a shared folder it's local
                let manages = !server || self.sync_rt.role.is_some_and(|r| r.manages());
                let owner = !server || self.sync_rt.role == Some(wb_net::Role::Owner);
                let mut remove = None;
                for m in &members {
                    // another admin is the owner's to change or remove (as the server)
                    let manages = manages && (owner || m.role != "Admin" || m.id == me);
                    let (r, _) = ui.allocate_exact_size(vec2(w, 46.0), Sense::hover());
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                    paint_avatar(ui.painter(), pos2(r.left() + 22.0, r.center().y), 13.0, m, t.bg);
                    let name = if m.id == me { trf!("{} (you)", m.name) } else { m.name.clone() };
                    let name_w = w * ex - 44.0 - 8.0;
                    paint_fit(ui, pos2(r.left() + 44.0, r.center().y - (if m.status_text.is_empty() { 0.0 } else { 8.0 })), Align2::LEFT_CENTER, &name, FontId::proportional(14.0), t.text, name_w);
                    if !m.status_text.is_empty() {
                        paint_fit(ui, pos2(r.left() + 44.0, r.center().y + 9.0), Align2::LEFT_CENTER, &m.status_text, FontId::proportional(12.0), t.text3, name_w);
                    }
                    paint_fit(ui, pos2(r.left() + w * ex + 8.0, r.center().y), Align2::LEFT_CENTER, if m.email.is_empty() { "-" } else { &m.email }, FontId::proportional(13.5), t.text2, col_w(ex, rx));
                    let role_w = w * (sx - rx) - 8.0;
                    let role_rect = Rect::from_min_size(pos2(r.left() + w * rx, r.center().y - 13.0), vec2(role_w, 26.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(role_rect).layout(Layout::left_to_right(Align::Center)));
                    let mut role = if m.role.is_empty() { "Member".to_string() } else { m.role.clone() };
                    if m.role == "Owner" || !manages || (server && m.id == me) {
                        child.label(super::menus::role_text(&role));
                    } else {
                        // Guest isn't offered: nothing limits what a guest can see yet
                        egui::ComboBox::from_id_salt(("role", m.id)).selected_text(super::menus::role_text(&role).to_string()).width(role_w.min(100.0)).show_ui(&mut child, |ui| {
                            for (n, _) in ROLES.iter().take(2) {
                                ui.selectable_value(&mut role, n.to_string(), super::menus::role_text(n));
                            }
                        });
                        if role != m.role && !(m.role.is_empty() && role == "Member") {
                            if server {
                                // the server changes the role and everyone's copy follows
                                let r = if role == "Admin" { wb_net::Role::Admin } else { wb_net::Role::Member };
                                self.access_request(wb_net::Msg::SetRole { member: m.id, role: r });
                            } else if let Some(mm) = self.data.members.iter_mut().find(|x| x.id == m.id) {
                                mm.role = role;
                                self.dirty = true;
                            }
                        }
                    }
                    let (st, col) = if m.invited { ("Invited", Color32::from_rgb(0xe1, 0x6b, 0x16)) } else { ("Active", Color32::from_rgb(41, 151, 100)) };
                    let st = if st == "Active" { crate::i18n::tr_ctx("Active (member)", st) } else { tr(st) };
                    paint_fit(ui, pos2(r.left() + w * sx + 8.0, r.center().y), Align2::LEFT_CENTER, st, FontId::proportional(13.0), col, w * (1.0 - sx) - 8.0 - 108.0);
                    if m.id != me && m.role != "Owner" && manages {
                        let br = Rect::from_min_size(pos2(r.right() - 80.0, r.center().y - 12.0), vec2(72.0, 24.0));
                        // someone who hasn't joined yet has an invite to take back
                        let (label, kind) = if m.invited { ("Cancel invite", "invite") } else { ("Remove", "person") };
                        let br = if m.invited { Rect::from_min_size(pos2(r.right() - 104.0, br.top()), vec2(96.0, 24.0)) } else { br };
                        if put_over(ui, br, ghost(egui::Button::new(RichText::new(tr(label)).color(t.danger)))).clicked() {
                            remove = Some((kind, m.id, m.name.clone()));
                        }
                    }
                }
                if let Some((kind, id, name)) = remove {
                    self.modal = Some(Modal::ConfirmRemove { kind, id, name });
                }
                // how invited people get in, for where this workspace is kept
                if members.iter().any(|m| m.invited) {
                    let how = match &self.data.extra.shared {
                        None => tr("Invited people can join once you share this workspace (Work together). Until then they can use this computer through Switch person in the profile menu."),
                        Some(c) if c.server.is_some() => tr("Invited people sign in with the invite code the server emails them. Owners and admins also find the codes in Work together > People while connected."),
                        Some(_) => tr("Invited people join from the shared folder: on their computer they open Work together, paste the folder's path, choose Join this workspace and pick their name."),
                    };
                    ui.add_space(12.0);
                    ui.label(RichText::new(how).small().color(t.text3));
                }
            });
        });
    }

    // ---------------------------------------------------------------- inbox (Primary / Other / Later / Cleared)

    pub(super) fn inbox_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        // tabs
        let w = ui.available_width();
        let (tabs_r, _) = ui.allocate_exact_size(vec2(w, 50.0), Sense::hover());
        let tw = (w - 20.0) / 4.0;
        for (i, (icon, label)) in [
            ("inbox", "Primary"),
            ("bolt", "Other"),
            ("alarm", "Later"),
            ("check", "Cleared"),
        ]
        .iter()
        .enumerate()
        {
            let r = Rect::from_min_size(
                pos2(tabs_r.left() + 10.0 + tw * i as f32, tabs_r.top()),
                vec2(tw, 50.0),
            );
            let resp = ui.interact(r, ui.id().with(("itab", i)), Sense::click());
            name_it(&resp, &format!("Inbox {}", label));
            let sel = self.inbox_tab == i as u8;
            if resp.hovered() && !sel {
                ui.painter().rect_filled(r.shrink(4.0), 6.0, t.hover);
            }
            line_icon(
                ui.painter(),
                icon,
                r.left_center() + vec2(26.0, 0.0),
                if sel { t.text } else { t.text2 },
            );
            ui.painter().text(
                r.left_center() + vec2(44.0, 0.0),
                Align2::LEFT_CENTER,
                tr(label),
                FontId::proportional(15.0),
                if sel { t.text } else { t.text2 },
            );
            if sel {
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        pos2(r.left(), r.bottom() - 2.0),
                        pos2(r.right() - 4.0, r.bottom()),
                    ),
                    1.0,
                    t.text,
                );
            }
            if i > 0 {
                ui.painter().vline(
                    r.left(),
                    r.top() + 14.0..=r.bottom() - 14.0,
                    Stroke::new(1.0, t.border),
                );
            }
            if resp.clicked() {
                self.inbox_tab = i as u8;
            }
        }
        ui.painter().hline(
            tabs_r.x_range(),
            tabs_r.bottom(),
            Stroke::new(1.0, t.border),
        );
        // collect items (what is on which tab: inbox_items)
        let mut items: Vec<super::nav::InboxItem> = self
            .inbox_items()
            .iter()
            .cloned()
            .filter(|x| x.tab == self.inbox_tab)
            .collect();
        match self.inbox_filter {
            1 => items.retain(|x| x.comment),
            2 => items.retain(|x| x.mention),
            3 => items.retain(|x| x.assigned),
            _ => {}
        }
        items.sort_by_key(|x| std::cmp::Reverse(x.ts));
        let mut clear_all = false;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(18, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let label = ["Filter", "Comments", "Mentions", "Assigned"]
                        [self.inbox_filter as usize % 4];
                    // framed like the buttons on the right of this row
                    let fr = icon_button(ui, &t, "filter", tr(label), label, true);
                    popup(ui, &fr, ui.id().with("inbox_filter"), 180.0, |ui| {
                        for (i, l) in ["Everything", "Comments", "Mentions", "Assigned to me"]
                            .iter()
                            .enumerate()
                        {
                            if ui
                                .selectable_label(self.inbox_filter == i as u8, tr(l))
                                .clicked()
                            {
                                self.inbox_filter = i as u8;
                                close_popup(ui);
                            }
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_clear = !items.is_empty() && self.inbox_tab != 3;
                        if ui
                            .add_enabled_ui(can_clear, |ui| {
                                icon_button(
                                    ui,
                                    &t,
                                    "check",
                                    crate::i18n::tr_ctx("Clear all (inbox)", "Clear all"),
                                    "Clear all",
                                    true,
                                )
                            })
                            .inner
                            .clicked()
                            && can_clear
                        {
                            clear_all = true;
                        }
                        if icon_button(ui, &t, "settings", "", "Notification settings", true)
                            .on_hover_text(tr("Notification settings"))
                            .clicked()
                        {
                            self.modal = Some(Modal::Notifications);
                        }
                        if icon_button(ui, &t, "alarm", tr("Reminder"), "Reminder", true).clicked()
                        {
                            self.modal = Some(Modal::Reminder {
                                text: String::new(),
                                date: Some(today()),
                                time: "09:00".into(),
                            });
                        }
                    });
                });
            });
        if clear_all {
            for k in items.iter().map(|x| &x.key) {
                self.data.extra.inbox_later.retain(|x| x != k);
                self.data.extra.inbox_cleared.push(k.clone());
            }
            self.dirty = true;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(18, 0))
                    .show(ui, |ui| {
                        if self.inbox_tab == 0 {
                            self.reminders_block(ui);
                        }
                        // reminders are waiting above: not "all caught up"
                        let reminders_shown = self.inbox_tab == 0
                            && self
                                .data
                                .extra
                                .reminders
                                .iter()
                                .any(|r| r.owner == self.data.me && !r.done);
                        if items.is_empty() && reminders_shown {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(tr("No new comments, mentions or assignments."))
                                    .color(t.text3),
                            );
                        } else if items.is_empty() {
                            ui.add_space(ui.available_height() * 0.3);
                            ui.vertical_centered(|ui| {
                                let alone = self.data.members.len() <= 1;
                                // only someone who may invite is pointed to inviting
                                let invite = alone && self.may_manage();
                                // alone, the Inbox stays empty: point at what is due
                                let me = self.data.me;
                                let due_work = self
                                    .data
                                    .tasks
                                    .iter()
                                    .filter(|x| {
                                        x.assignees.contains(&me)
                                            && x.due.is_some_and(|d| d <= today())
                                            && !self.data.is_closed(x)
                                            && self.data.task_live(x)
                                    })
                                    .count();
                                let point_to_due = self.inbox_tab == 0 && alone && due_work > 0;
                                // the people icon only where it invites people
                                let (title, sub, icon) = match self.inbox_tab {
                                    0 if point_to_due => (
                                        "Nothing from other people",
                                        "The Inbox collects comments, mentions and assignments from others.",
                                        "check",
                                    ),
                                    0 if invite => (
                                        "Looking to collaborate?",
                                        "Collaboration is one invite away.",
                                        "teams",
                                    ),
                                    0 => (
                                        "You're all caught up",
                                        "New comments, mentions and assignments show up here.",
                                        "check",
                                    ),
                                    1 => (
                                        "Nothing in Other",
                                        "Updates that don't need you show up here.",
                                        "inbox",
                                    ),
                                    2 => (
                                        "Nothing saved for later",
                                        "Use Later on an item to come back to it.",
                                        "alarm",
                                    ),
                                    _ => (
                                        "Nothing cleared yet",
                                        "Cleared items show up here.",
                                        "check",
                                    ),
                                };
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                                ui.painter().rect_stroke(
                                    r.shrink(2.0),
                                    14.0,
                                    Stroke::new(1.5, t.border),
                                    egui::StrokeKind::Inside,
                                );
                                line_icon_scaled(
                                    ui.painter(),
                                    icon,
                                    r.center(),
                                    Color32::from_rgb(0x5f, 0x55, 0xee),
                                    1.6,
                                );
                                ui.add_space(10.0);
                                ui.label(RichText::new(tr(title)).size(18.0).color(t.text));
                                ui.label(RichText::new(tr(sub)).size(14.0).color(t.text2));
                                if point_to_due {
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(trf!(
                                            "Tasks due today or overdue: {}",
                                            due_work
                                        ))
                                        .size(14.0)
                                        .color(t.text),
                                    );
                                    ui.add_space(6.0);
                                    if ui
                                        .add(Tint(
                                            egui::Button::new(
                                                RichText::new(tr("Open Today & Overdue"))
                                                    .size(15.0)
                                                    .color(t.on_primary),
                                            )
                                            .fill(t.primary)
                                            .corner_radius(7)
                                            .min_size(vec2(0.0, 38.0)),
                                        ))
                                        .clicked()
                                    {
                                        self.actions.push(Action::Go(Location::TodayOverdue));
                                    }
                                    ui.add_space(4.0);
                                    if invite && ui.button(tr("Invite people")).clicked() {
                                        self.modal = Some(Modal::Invite {
                                            emails: String::new(),
                                            role: "Member".into(),
                                        });
                                    }
                                } else if self.inbox_tab == 0 && invite {
                                    ui.add_space(10.0);
                                    if ui
                                        .add(Tint(
                                            egui::Button::new(
                                                RichText::new(tr("Invite people"))
                                                    .size(15.0)
                                                    .color(t.on_primary),
                                            )
                                            .fill(t.primary)
                                            .corner_radius(7)
                                            .min_size(vec2(0.0, 38.0)),
                                        ))
                                        .clicked()
                                    {
                                        self.modal = Some(Modal::Invite {
                                            emails: String::new(),
                                            role: "Member".into(),
                                        });
                                    }
                                }
                            });
                        }
                        for it in items {
                            let (key, ts, tname) = (it.key.clone(), it.ts, it.title.clone());
                            let w = ui.available_width();
                            let (r, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click());
                            name_it(&resp, &tname);
                            if resp.hovered() {
                                ui.painter().rect_filled(r, 8.0, t.hover);
                            }
                            ui.painter()
                                .hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                            if let Some(m) = self.data.member(it.author) {
                                paint_avatar(
                                    ui.painter(),
                                    r.left_center() + vec2(22.0, 0.0),
                                    13.0,
                                    m,
                                    t.bg,
                                );
                            }
                            // a comment on an archived task: say the task is archived
                            let archived = match it.src {
                                super::nav::InboxSrc::Task(id) => {
                                    self.task(id).is_some_and(|x| !self.data.task_live(x))
                                }
                                _ => false,
                            };
                            ui.painter().text(
                                r.left_top() + vec2(48.0, 18.0),
                                Align2::LEFT_CENTER,
                                if archived {
                                    trf!("{} (archived)", tname)
                                } else {
                                    tname.clone()
                                },
                                FontId::proportional(14.0),
                                if archived { t.text2 } else { t.text },
                            );
                            let line = self
                                .inbox_line(it.author, &it.text, it.comment, it.src, it.mention);
                            ui.painter()
                                .with_clip_rect(Rect::from_min_max(
                                    r.min,
                                    pos2(r.right() - 190.0, r.bottom()),
                                ))
                                .text(
                                    r.left_top() + vec2(48.0, 40.0),
                                    Align2::LEFT_CENTER,
                                    line,
                                    FontId::proportional(13.0),
                                    t.text2,
                                );
                            ui.painter().text(
                                r.right_center() - vec2(12.0, 0.0),
                                Align2::RIGHT_CENTER,
                                fmt_ago(ts),
                                FontId::proportional(12.0),
                                t.text3,
                            );
                            let mut handled = false;
                            if resp.hovered() || ui.rect_contains_pointer(r) {
                                let lr = Rect::from_min_size(
                                    pos2(r.right() - 180.0, r.center().y - 13.0),
                                    vec2(56.0, 26.0),
                                );
                                let cr = Rect::from_min_size(
                                    pos2(r.right() - 120.0, r.center().y - 13.0),
                                    vec2(56.0, 26.0),
                                );
                                if self.inbox_tab != 2
                                    && put_over(
                                        ui,
                                        lr,
                                        egui::Button::new(tr("Later")).corner_radius(6),
                                    )
                                    .clicked()
                                {
                                    self.data.extra.inbox_later.push(key.clone());
                                    self.dirty = true;
                                    handled = true;
                                }
                                // Later items can go back to the inbox
                                if self.inbox_tab == 2
                                    && put_over(
                                        ui,
                                        lr,
                                        egui::Button::new(tr("Primary")).corner_radius(6),
                                    )
                                    .on_hover_text(tr("Back to the inbox"))
                                    .clicked()
                                {
                                    self.data.extra.inbox_later.retain(|x| x != &key);
                                    self.dirty = true;
                                    handled = true;
                                }
                                let ctext = if self.inbox_tab == 3 {
                                    "Restore"
                                } else {
                                    "Clear"
                                };
                                if put_over(ui, cr, egui::Button::new(tr(ctext)).corner_radius(6))
                                    .clicked()
                                {
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
                                self.open_inbox_src(it.src);
                            }
                        }
                    });
            });
    }

    // ---------------------------------------------------------------- posts

    pub(super) fn posts_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.page_bar(ui, tr("Posts"), |s, ui| {
            let tt = s.t();
            if primary_button(ui, &tt, tr("New Post")).clicked() {
                s.post_title.clear();
                s.post_body.clear();
            }
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
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
                    egui::Frame::new()
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(12)
                        .inner_margin(egui::Margin::symmetric(16, 10))
                        .show(ui, |ui| {
                            ui.set_width(cw - 34.0);
                            // the frame sits in a horizontal row; stack its contents top to bottom
                            ui.vertical(|ui| {
                                let chans: Vec<(u64, String)> = self
                                    .data
                                    .channels
                                    .iter()
                                    .filter(|c| c.kind == ChannelKind::Channel && self.can_see(c))
                                    .map(|c| (c.id, c.name.clone()))
                                    .collect();
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(tr("Channel:")).color(t.text2));
                                    let cur = chans
                                        .iter()
                                        .find(|c| c.0 == self.post_channel)
                                        .map(|c| format!("# {}", c.1))
                                        .unwrap_or_else(|| tr("Select Channel").into());
                                    egui::ComboBox::from_id_salt("post_ch")
                                        .selected_text(cur)
                                        .show_ui(ui, |ui| {
                                            for (id, n) in &chans {
                                                ui.selectable_value(
                                                    &mut self.post_channel,
                                                    *id,
                                                    format!("# {}", n),
                                                );
                                            }
                                        });
                                });
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(tr("Type:")).color(t.text2));
                                    egui::ComboBox::from_id_salt("post_type")
                                        .selected_text(tr(&self.post_type))
                                        .show_ui(ui, |ui| {
                                            for k in ["Announcement", "Update", "Idea", "Question"]
                                            {
                                                ui.selectable_value(
                                                    &mut self.post_type,
                                                    k.to_string(),
                                                    tr(k),
                                                );
                                            }
                                        });
                                });
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(tr("Title:")).color(t.text2));
                                    // frameless like the body below, so a hint shows where to type
                                    let r = ui.add(
                                        egui::TextEdit::singleline(&mut self.post_title)
                                            .hint_text(tr("Add a title"))
                                            .frame(false)
                                            .desired_width(f32::INFINITY),
                                    );
                                    name_field(&r, "Post title");
                                });
                                ui.separator();
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.post_body)
                                        .hint_text(tr("Share an announcement..."))
                                        .frame(false)
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.horizontal(|ui| {
                                    // "+" and "@" are characters; the others line icons
                                    for (g, tip) in [
                                        ("+", "Create task"),
                                        ("@", "Mention someone"),
                                        ("smile", "Emoji"),
                                        ("task", "Create task"),
                                    ] {
                                        let b = if g.len() == 1 {
                                            ui.add(ghost(egui::Button::new(
                                                RichText::new(g).color(t.text2),
                                            )))
                                        } else {
                                            icon_button(ui, &t, g, "", tip, false)
                                        };
                                        if b.on_hover_text(tr(tip)).clicked() {
                                            match g {
                                                "@" => self.post_body.push('@'),
                                                "smile" => self.post_body.push_str("\u{1F44D}"),
                                                _ => self.open_quick_add(None),
                                            }
                                        }
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let ready = !self.post_title.trim().is_empty()
                                            && self.post_channel != 0;
                                        let b =
                                            Tint(
                                                egui::Button::new(RichText::new(tr("Post")).color(
                                                    if ready { t.on_primary } else { t.text3 },
                                                ))
                                                .fill(if ready { t.primary } else { t.hover })
                                                .corner_radius(6)
                                                .min_size(vec2(56.0, 30.0)),
                                            );
                                        if ui
                                            .add(b)
                                            .on_hover_text(if self.post_channel == 0 {
                                                tr("Select a channel first")
                                            } else {
                                                ""
                                            })
                                            .clicked()
                                            && ready
                                        {
                                            let id = self.data.new_id();
                                            let me = self.data.me;
                                            let m = Message {
                                                edited: None,
                                                imported: None,
                                                mentions: self
                                                    .data
                                                    .mentions_in(self.post_body.trim()),
                                                attachments: vec![],
                                                id,
                                                author: me,
                                                text: self.post_body.trim().to_string(),
                                                ts: now_ts(),
                                                reactions: vec![],
                                                replies: vec![],
                                                saved: false,
                                                task: None,
                                                post_title: Some(
                                                    self.post_title.trim().to_string(),
                                                ),
                                                post_type: self.post_type.clone(),
                                                pinned: false,
                                                call: None,
                                            };
                                            if let Some(c) = self
                                                .data
                                                .channels
                                                .iter_mut()
                                                .find(|c| c.id == self.post_channel)
                                            {
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
                ui.painter()
                    .hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    let fol = Tint(
                        egui::Button::new(RichText::new(tr("Channels I'm Following")).color(
                            if self.post_following {
                                t.accent_text
                            } else {
                                t.text2
                            },
                        ))
                        .fill(if self.post_following {
                            t.accent_bg
                        } else {
                            t.bg
                        })
                        .corner_radius(13),
                    );
                    if ui.add(fol).clicked() {
                        self.post_following = !self.post_following;
                    }
                    let chans: Vec<(u64, String)> = self
                        .data
                        .channels
                        .iter()
                        .filter(|c| c.kind == ChannelKind::Channel && self.can_see(c))
                        .map(|c| (c.id, c.name.clone()))
                        .collect();
                    let cl = self
                        .post_f_channel
                        .and_then(|id| chans.iter().find(|c| c.0 == id))
                        .map(|c| format!("#  {}", c.1))
                        .unwrap_or_else(|| tr("#  Channel").into());
                    let cr = ui.add(egui::Button::new(cl).corner_radius(13));
                    popup(ui, &cr, ui.id().with("pf_ch"), 180.0, |ui| {
                        if ui
                            .selectable_label(self.post_f_channel.is_none(), tr("All channels"))
                            .clicked()
                        {
                            self.post_f_channel = None;
                            close_popup(ui);
                        }
                        for (id, n) in &chans {
                            if ui
                                .selectable_label(
                                    self.post_f_channel == Some(*id),
                                    format!("# {}", n),
                                )
                                .clicked()
                            {
                                self.post_f_channel = Some(*id);
                                close_popup(ui);
                            }
                        }
                    });
                    let members: Vec<(u64, String)> = self
                        .data
                        .members
                        .iter()
                        .map(|m| (m.id, m.name.clone()))
                        .collect();
                    let al = self
                        .post_f_author
                        .and_then(|id| members.iter().find(|m| m.0 == id))
                        .map(|m| format!("@  {}", m.1))
                        .unwrap_or_else(|| tr("@  Author").into());
                    let ar = ui.add(egui::Button::new(al).corner_radius(13));
                    popup(ui, &ar, ui.id().with("pf_au"), 180.0, |ui| {
                        if ui
                            .selectable_label(self.post_f_author.is_none(), tr("Anyone"))
                            .clicked()
                        {
                            self.post_f_author = None;
                            close_popup(ui);
                        }
                        for (id, n) in &members {
                            if ui
                                .selectable_label(self.post_f_author == Some(*id), n)
                                .clicked()
                            {
                                self.post_f_author = Some(*id);
                                close_popup(ui);
                            }
                        }
                    });
                    let tl = self
                        .post_f_type
                        .clone()
                        .map(|t| tr(&t).to_string())
                        .unwrap_or_else(|| tr("Type").into());
                    let tyr = icon_button(ui, &t, "shapes", &tl, "Post type filter", true);
                    popup(ui, &tyr, ui.id().with("pf_ty"), 180.0, |ui| {
                        if ui
                            .selectable_label(self.post_f_type.is_none(), tr("Any type"))
                            .clicked()
                        {
                            self.post_f_type = None;
                            close_popup(ui);
                        }
                        for k in ["Announcement", "Update", "Idea", "Question"] {
                            if ui
                                .selectable_label(self.post_f_type.as_deref() == Some(k), tr(k))
                                .clicked()
                            {
                                self.post_f_type = Some(k.to_string());
                                close_popup(ui);
                            }
                        }
                    });
                    ui.add_space((cw - 520.0).max(10.0));
                    ui.label(RichText::new(tr("Sort:")).color(t.text2));
                    egui::ComboBox::from_id_salt("post_sort")
                        .selected_text(if self.post_sort_old {
                            tr("Oldest")
                        } else {
                            tr("Newest")
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.post_sort_old, false, tr("Newest"));
                            ui.selectable_value(&mut self.post_sort_old, true, tr("Oldest"));
                        });
                });
                ui.add_space(12.0);
                let mut posts: Vec<(String, u64, Message)> = vec![];
                for c in self.data.channels.iter().filter(|c| self.can_see(c)) {
                    for m in &c.messages {
                        if m.post_title.is_some() {
                            posts.push((c.name.clone(), c.id, m.clone()));
                        }
                    }
                }
                let me = self.data.me;
                let following: Vec<u64> = self
                    .data
                    .channels
                    .iter()
                    .filter(|c| c.members.contains(&me))
                    .map(|c| c.id)
                    .collect();
                posts.retain(|p| {
                    (!self.post_following || following.contains(&p.1))
                        && self.post_f_channel.map(|c| c == p.1).unwrap_or(true)
                        && self.post_f_author.map(|a| a == p.2.author).unwrap_or(true)
                        && self
                            .post_f_type
                            .as_ref()
                            .map(|ty| {
                                (if p.2.post_type.is_empty() {
                                    "Announcement"
                                } else {
                                    p.2.post_type.as_str()
                                }) == ty
                            })
                            .unwrap_or(true)
                });
                posts.sort_by_key(|p| p.2.ts);
                if !self.post_sort_old {
                    posts.reverse();
                }
                if posts.is_empty() {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                        ui.painter().rect_stroke(
                            r.shrink(2.0),
                            14.0,
                            Stroke::new(1.5, t.border),
                            egui::StrokeKind::Inside,
                        );
                        line_icon_scaled(ui.painter(), "megaphone", r.center(), t.text2, 1.6);
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(tr("There are no Posts yet!"))
                                .size(18.0)
                                .color(t.text),
                        );
                    });
                }
                for (cname, cid, m) in posts {
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        egui::Frame::new()
                            .fill(t.card)
                            .stroke(Stroke::new(1.0, t.border))
                            .corner_radius(10)
                            .inner_margin(egui::Margin::same(14))
                            .show(ui, |ui| {
                                ui.set_width(cw - 28.0);
                                ui.horizontal(|ui| {
                                    if let Some(a) = self.data.member(m.author) {
                                        let (r, _) = ui
                                            .allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                                        paint_avatar(ui.painter(), r.center(), 12.0, a, t.card);
                                        ui.label(RichText::new(&a.name).strong());
                                    }
                                    ui.label(
                                        RichText::new(trf!(
                                            "{} in # {} \u{2022} {}",
                                            tr(if m.post_type.is_empty() {
                                                "Announcement"
                                            } else {
                                                &m.post_type
                                            }),
                                            cname,
                                            fmt_ago(m.ts)
                                        ))
                                        .small()
                                        .color(t.text3),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.link(tr("Open in channel")).clicked() {
                                            self.section = Section::Chat;
                                            self.chat_open = Some(cid);
                                        }
                                    });
                                });
                                ui.label(
                                    RichText::new(m.post_title.clone().unwrap_or_default())
                                        .size(17.0),
                                );
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
        egui::SidePanel::left("meet_list")
            .exact_width(260.0)
            .resizable(false)
            .frame(list_panel_frame(&t))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Meetings")).size(16.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, &t, tr("+ Meeting")).clicked() {
                            let id = self.data.new_id();
                            self.data.extra.meetings.push(Meeting {
                                id,
                                title: tr("New meeting").into(),
                                date: today(),
                                time: "10:00".into(),
                                attendees: vec![self.data.me],
                                agenda: String::new(),
                                notes: String::new(),
                            });
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
                    ui.painter().with_clip_rect(r).text(
                        r.left_top() + vec2(10.0, 14.0),
                        Align2::LEFT_CENTER,
                        &m.title,
                        FontId::proportional(14.0),
                        t.text,
                    );
                    ui.painter().text(
                        r.left_top() + vec2(10.0, 31.0),
                        Align2::LEFT_CENTER,
                        format!("{} \u{2022} {}", fmt_date(m.date), m.time),
                        FontId::proportional(11.5),
                        if m.date < today() { t.text3 } else { t.text2 },
                    );
                    if resp.clicked() {
                        self.meeting_open = Some(m.id);
                    }
                }
                if ms.is_empty() {
                    ui.label(RichText::new(tr("No meetings yet")).color(t.text3));
                }
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::symmetric(32, 20))).show_inside(ui, |ui| {
            let Some(idx) = self.meeting_open.and_then(|id| self.data.extra.meetings.iter().position(|m| m.id == id)) else {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| ui.label(RichText::new(tr("Pick a meeting or create one to keep the agenda and notes together.")).color(t.text2)));
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
                    let r = icon_button(ui, &t, "calendar", &fmt_date(m.date), "Meeting date", true);
                    if let Some(true) = popup(ui, &r, ui.id().with("meet_date"), 380.0, |ui| date_menu(ui, &t, ui.id().with("meet_cal"), &mut d)) {
                        if let Some(d) = d {
                            m.date = d;
                            ch = true;
                        }
                    }
                    ui.label(tr("at"));
                    ch |= ui.add(field(&mut m.time).desired_width(60.0)).changed();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(ghost(egui::Button::new(RichText::new(tr("Delete")).color(t.danger)))).clicked() {
                            delete = true;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Attendees")).color(t.text2));
                ui.horizontal_wrapped(|ui| {
                    for mem in &members {
                        let mut on = m.attendees.contains(&mem.id);
                        if check(ui, &mut on, &mem.name).changed() {
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
                ui.label(RichText::new(tr("Agenda")).color(t.text2));
                ch |= ui.add(field_multi(&mut m.agenda).hint_text(tr("What will you discuss?")).desired_rows(4).desired_width(f32::INFINITY)).changed();
                ui.add_space(8.0);
                ui.label(RichText::new(tr("Notes")).color(t.text2));
                ch |= ui.add(field_multi(&mut m.notes).hint_text(tr("Write notes. Lines starting with [ ] become action items.")).desired_rows(10).desired_width(f32::INFINITY)).changed();
                let actions: Vec<String> = m.notes.lines().filter_map(|l| l.trim().strip_prefix("[ ]").map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect();
                if !actions.is_empty() && ui.button(trf!("Create {} tasks from action items", actions.len())).clicked() {
                    make_tasks = actions;
                    m.notes = m.notes.replace("[ ]", "[x]");
                    ch = true;
                }
                if ch {
                    self.dirty = true;
                }
            }
            if !make_tasks.is_empty() {
                // the List in view, else the first one that isn't archived
                let list = self.default_list().filter(|l| !self.data.list_archived(*l)).or_else(|| {
                    self.data.spaces.iter().filter(|s| !s.archived && s.private_owner.is_none_or(|o| o == self.data.me)).flat_map(|s| s.folders.iter().flat_map(|f| f.lists.iter()).chain(s.lists.iter())).find(|l| !l.archived).map(|l| l.id)
                });
                if let Some(list) = list {
                    // assigned to the attendees ticked above (none ticked: nobody)
                    let who: Vec<u64> = self.data.extra.meetings[idx].attendees.iter().copied().filter(|a| self.data.member(*a).is_some()).collect();
                    for n in make_tasks {
                        self.actions.push(Action::Add { list, name: n, status: None, priority: 0, assignees: who.clone(), due: None, parent: None, open: false });
                    }
                    self.toast = Some((trf!("Tasks created from the meeting notes in {}", self.data.list_name(list)), now_ts()));
                } else {
                    self.toast = Some((tr("Make a List first: action items become tasks in a List").into(), now_ts()));
                }
            }
            if delete {
                let m = &self.data.extra.meetings[idx];
                self.modal = Some(Modal::ConfirmRemove { kind: "meeting", id: m.id, name: m.title.clone() });
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
                TargetKind::Number {
                    start,
                    target,
                    current,
                    ..
                } => {
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
                    // tasks deleted since they were picked don't count either way
                    let live: Vec<&Task> = ids.iter().filter_map(|id| self.task(*id)).collect();
                    if live.is_empty() {
                        0.0
                    } else {
                        live.iter().filter(|x| self.data.is_closed(x)).count() as f32
                            / live.len() as f32
                    }
                }
            })
            .sum();
        sum / g.targets.len() as f32
    }

    pub(super) fn goals_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(24, 16))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("Goals")).size(20.0));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if primary_button(ui, &t, tr("+ New Goal")).clicked() {
                                    let id = self.data.new_id();
                                    let color =
                                        STATUS_PALETTE[1 + self.data.extra.goals.len() % 13];
                                    self.data.extra.goals.push(Goal {
                                        id,
                                        name: tr("New goal").into(),
                                        color,
                                        due: None,
                                        owner: self.data.me,
                                        targets: vec![],
                                    });
                                    self.goal_open = Some(id);
                                    self.dirty = true;
                                }
                            });
                        });
                        ui.label(
                            RichText::new(tr(
                                "Track progress with targets: numbers, done / not done, or tasks",
                            ))
                            .color(t.text2),
                        );
                        ui.add_space(12.0);
                        let goals = self.data.extra.goals.clone();
                        if goals.is_empty() {
                            ui.label(RichText::new(tr("No goals yet.")).color(t.text3));
                        }
                        for g in &goals {
                            let p = self.goal_progress(g);
                            let open = self.goal_open == Some(g.id);
                            let resp = egui::Frame::new()
                                .fill(t.card)
                                .stroke(Stroke::new(1.0, if open { t.text3 } else { t.border }))
                                .corner_radius(10)
                                .inner_margin(egui::Margin::same(14))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui
                                            .allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                                        ui.painter().circle_filled(r.center(), 6.0, rgb(g.color));
                                        ui.label(RichText::new(&g.name).size(16.0));
                                        if let Some(d) = g.due {
                                            ui.label(
                                                RichText::new(trf!("Due {}", fmt_date(d)))
                                                    .small()
                                                    .color(date_color(d, p >= 1.0, &t)),
                                            );
                                        }
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(format!("{:.0}%", p * 100.0))
                                                        .strong(),
                                                );
                                                if let Some(m) = self.data.member(g.owner) {
                                                    let (r, _) = ui.allocate_exact_size(
                                                        vec2(22.0, 22.0),
                                                        Sense::hover(),
                                                    );
                                                    paint_avatar(
                                                        ui.painter(),
                                                        r.center(),
                                                        10.0,
                                                        m,
                                                        t.card,
                                                    );
                                                    ui.label(
                                                        RichText::new(&m.name)
                                                            .small()
                                                            .color(t.text2),
                                                    );
                                                }
                                            },
                                        );
                                    });
                                    ui.add(
                                        egui::ProgressBar::new(p)
                                            .desired_height(6.0)
                                            .fill(rgb(g.color)),
                                    );
                                });
                            let card = ui.interact(
                                resp.response.rect,
                                ui.id().with(("goal", g.id)),
                                Sense::click(),
                            );
                            hover_bg(ui, &card, card.rect, 8.0);
                            if card.clicked() {
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
        let Some(gi) = self.data.extra.goals.iter().position(|g| g.id == gid) else {
            return;
        };
        let tasks: Vec<(u64, String)> = self
            .data
            .tasks
            .iter()
            .filter(|x| x.parent.is_none())
            .map(|x| (x.id, x.name.clone()))
            .collect();
        let closed: Vec<u64> = self
            .data
            .tasks
            .iter()
            .filter(|x| self.data.is_closed(x))
            .map(|x| x.id)
            .collect();
        let members: Vec<(u64, String)> = self
            .data
            .members
            .iter()
            .map(|m| (m.id, m.name.clone()))
            .collect();
        let exists: HashSet<u64> = self.data.tasks.iter().map(|x| x.id).collect();
        let mut delete = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 20,
                right: 0,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                let g = &mut self.data.extra.goals[gi];
                let mut ch = false;
                ui.horizontal(|ui| {
                    ui.label(tr("Name"));
                    ch |= ui.add(field(&mut g.name).desired_width(260.0)).changed();
                    let mut d = g.due;
                    let due_txt = g
                        .due
                        .map(|d| trf!("Due {}", fmt_date(d)))
                        .unwrap_or_else(|| tr("Due date").into());
                    let r = icon_button(ui, &t, "calendar", &due_txt, "Goal due date", true);
                    if let Some(true) =
                        popup(ui, &r, ui.id().with(("goal_due", gid)), 380.0, |ui| {
                            date_menu(ui, &t, ui.id().with(("goal_cal", gid)), &mut d)
                        })
                    {
                        g.due = d;
                        ch = true;
                    }
                    if ui
                        .add(ghost(egui::Button::new(
                            RichText::new(tr("Delete goal")).color(t.danger),
                        )))
                        .clicked()
                    {
                        delete = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Owner"));
                    let cur = members
                        .iter()
                        .find(|m| m.0 == g.owner)
                        .map(|m| m.1.clone())
                        .unwrap_or_else(|| tr("Nobody").into());
                    egui::ComboBox::from_id_salt(("goal_owner", gid))
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            for (id, n) in &members {
                                ch |= ui.selectable_value(&mut g.owner, *id, n).changed();
                            }
                        });
                    ui.label(tr("Color"));
                    for c in STATUS_PALETTE.iter().skip(1) {
                        let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 7.0, rgb(*c));
                        hover_ring(ui, &resp, r.shrink(1.0), 8.0);
                        if g.color == *c {
                            ui.painter()
                                .circle_stroke(r.center(), 9.0, Stroke::new(1.5, t.text));
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
                        ch |= ui.add(field(&mut tg.name).desired_width(180.0)).changed();
                        match &mut tg.kind {
                            TargetKind::Number {
                                start,
                                target,
                                current,
                                unit,
                            } => {
                                ui.label(tr("now"));
                                ch |= ui.add(egui::DragValue::new(current).speed(1.0)).changed();
                                ui.label(tr("of"));
                                ch |= ui.add(egui::DragValue::new(target).speed(1.0)).changed();
                                ui.label(tr("from"));
                                ch |= ui.add(egui::DragValue::new(start).speed(1.0)).changed();
                                ch |= ui
                                    .add(field(unit).hint_text(tr("unit")).desired_width(50.0))
                                    .changed();
                            }
                            TargetKind::TrueFalse { done } => {
                                ch |= check(ui, done, tr("Done")).changed();
                            }
                            TargetKind::Tasks { ids } => {
                                let n = ids.iter().filter(|id| closed.contains(id)).count();
                                ui.label(trf!(
                                    "{}/{} tasks done",
                                    n,
                                    ids.iter().filter(|id| exists.contains(id)).count()
                                ));
                                ui.menu_button(tr("Pick tasks"), |ui| {
                                    egui::ScrollArea::vertical()
                                        .max_height(300.0)
                                        .show(ui, |ui| {
                                            for (id, name) in &tasks {
                                                let mut on = ids.contains(id);
                                                if check(ui, &mut on, name).changed() {
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
                        if ui
                            .add(ghost(egui::Button::new(
                                RichText::new("\u{00D7}").color(t.text3),
                            )))
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    g.targets.remove(i);
                    ch = true;
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Add target:")).color(t.text2));
                    if ui.button(tr("Number")).clicked() {
                        g.targets.push(GoalTarget {
                            name: tr("Target").into(),
                            kind: TargetKind::Number {
                                start: 0.0,
                                target: 100.0,
                                current: 0.0,
                                unit: String::new(),
                            },
                        });
                        ch = true;
                    }
                    if ui.button(tr("Done / not done")).clicked() {
                        g.targets.push(GoalTarget {
                            name: tr("Target").into(),
                            kind: TargetKind::TrueFalse { done: false },
                        });
                        ch = true;
                    }
                    if ui.button(tr("Tasks")).clicked() {
                        g.targets.push(GoalTarget {
                            name: tr("Finish tasks").into(),
                            kind: TargetKind::Tasks { ids: vec![] },
                        });
                        ch = true;
                    }
                });
                if ch {
                    self.dirty = true;
                }
            });
        if delete {
            let g = &self.data.extra.goals[gi];
            self.modal = Some(Modal::ConfirmRemove {
                kind: "goal",
                id: g.id,
                name: g.name.clone(),
            });
        }
    }

    // ---------------------------------------------------------------- forms

    pub(super) fn form_detail(&mut self, ui: &mut Ui) {
        let t = self.t();
        // a message about one form doesn't stay on another
        if self.sweep.form_for != self.form_open {
            self.sweep.form_error = None;
            self.sweep.form_for = self.form_open;
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::symmetric(32, 20))).show_inside(ui, |ui| {
            let Some(fi) = self.form_open.and_then(|id| self.data.extra.forms.iter().position(|f| f.id == id)) else {
                ui.label(RichText::new(tr("Create a form to collect requests; each answer becomes a task.")).color(t.text2));
                return;
            };
            // Lists answers can go to (not archived); an archived one already
            // picked stays, marked, so the form says where answers go now
            let mut lists: Vec<(u64, String)> = self.all_list_paths();
            let picked = self.data.extra.forms[fi].list_id;
            if self.data.list_archived(picked) && self.data.space_of_list(picked).is_some() {
                lists.push((picked, trf!("{} (archived)", self.data.list_name(picked))));
            }
            let members: Vec<(u64, String)> = self.data.members.iter().map(|m| (m.id, m.name.clone())).collect();
            // answers typed into one form don't carry over to another
            let fid = self.data.extra.forms[fi].id;
            let for_id = egui::Id::new("form_fill_for");
            if ui.data(|d| d.get_temp::<u64>(for_id)) != Some(fid) {
                self.form_fill.clear();
                ui.data_mut(|d| d.insert_temp(for_id, fid));
            }
            ui.horizontal(|ui| {
                // the mode shown is outlined
                for (fill, kind, label) in [(false, "edit", "Edit"), (true, "eye", "Fill out")] {
                    let sel = self.form_mode_fill == fill;
                    if icon_button_ex(ui, &t, kind, tr(label), label, sel, 0.0, if sel { t.text } else { t.text2 }, t.text2).clicked() {
                        self.form_mode_fill = fill;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| self.star_toggle(ui, format!("form:{}", fid)));
            });
            ui.separator();
            if !self.form_mode_fill {
                let mut delete = false;
                let f = &mut self.data.extra.forms[fi];
                let mut ch = false;
                ch |= ui.add(egui::TextEdit::singleline(&mut f.name).font(FontId::proportional(24.0)).frame(false).desired_width(f32::INFINITY)).changed();
                ch |= ui.add(field_multi(&mut f.description).desired_rows(2).desired_width(f32::INFINITY)).changed();
                ui.horizontal(|ui| {
                    ui.label(tr("Answers create tasks in"));
                    let cur = lists.iter().find(|l| l.0 == f.list_id).map(|l| l.1.clone()).unwrap_or_else(|| tr("choose a list").into());
                    egui::ComboBox::from_id_salt("form_list_sel").selected_text(cur).show_ui(ui, |ui| {
                        for (id, n) in &lists {
                            ch |= ui.selectable_value(&mut f.list_id, *id, n).changed();
                        }
                    });
                });
                ui.add_space(8.0);
                ui.label(RichText::new(tr("Questions")).strong());
                let mut remove = None;
                let mut swap = None;
                let count = f.fields.len();
                for (i, fld) in f.fields.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ch |= ui.add(field(&mut fld.label).desired_width(220.0)).changed();
                        egui::ComboBox::from_id_salt(("map", i)).selected_text(tr(form_kind_label(&fld.map))).show_ui(ui, |ui| {
                            for (k, n) in FORM_KINDS {
                                ch |= ui.selectable_value(&mut fld.map, k.to_string(), tr(n)).changed();
                            }
                        });
                        let rq = check(ui, &mut fld.required, tr("Required"));
                        name_it(&rq, &format!("Required {}", i + 1));
                        ch |= rq.changed();
                        let up = ui.add_enabled(i > 0, ghost(egui::Button::new("\u{2191}")));
                        name_it(&up, &format!("Move question {} up", i + 1));
                        if up.clicked() {
                            swap = Some((i, i - 1));
                        }
                        let down = ui.add_enabled(i + 1 < count, ghost(egui::Button::new("\u{2193}")));
                        name_it(&down, &format!("Move question {} down", i + 1));
                        if down.clicked() {
                            swap = Some((i, i + 1));
                        }
                        if ui.add(ghost(egui::Button::new(RichText::new("\u{00D7}").color(t.text3)))).clicked() {
                            remove = Some(i);
                        }
                    });
                    if fld.map == "dropdown" {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(RichText::new(tr("Choices")).small().color(t.text2));
                            // empty pieces are kept while typing so "a, " can grow into "a, b"
                            let mut text = fld.options.join(", ");
                            let r = ui.add(field(&mut text).hint_text(tr("Red, Green, Blue")).desired_width(300.0));
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
                    if ui.button(tr("+ Add question")).clicked() {
                        f.fields.push(FormField { label: tr("Question").into(), map: "text".into(), ..Default::default() });
                        ch = true;
                    }
                    if ui.add(ghost(egui::Button::new(RichText::new(tr("Delete form")).color(t.danger)))).clicked() {
                        delete = true;
                    }
                });
                if ch {
                    self.dirty = true;
                }
                if delete {
                    let f = &self.data.extra.forms[fi];
                    self.modal = Some(Modal::ConfirmRemove { kind: "form", id: f.id, name: f.name.clone() });
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
                                egui::ComboBox::from_id_salt(("fp", i)).selected_text(if v.is_empty() { tr("Choose").to_string() } else { tr(v).to_string() }).show_ui(ui, |ui| {
                                    for p in ["Urgent", "High", "Normal", "Low"] {
                                        ui.selectable_value(v, p.to_string(), tr(p));
                                    }
                                });
                            }
                            "due" => {
                                let mut d = NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
                                let b = icon_button(ui, &t, "calendar", &d.map(fmt_date).unwrap_or_else(|| tr("Pick a date").into()), "", true);
                                name_it(&b, &format!("Answer {}", i + 1));
                                if let Some(true) = popup(ui, &b, ui.id().with(("form_due", fid, i)), 380.0, |ui| date_menu(ui, &t, ui.id().with(("form_due_cal", fid, i)), &mut d)) {
                                    *v = d.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
                                }
                            }
                            "dropdown" => {
                                egui::ComboBox::from_id_salt(("fd", i)).selected_text(if v.is_empty() { tr("Choose").to_string() } else { v.clone() }).show_ui(ui, |ui| {
                                    for o in fld.options.iter().filter(|o| !o.is_empty()) {
                                        ui.selectable_value(v, o.clone(), o);
                                    }
                                });
                            }
                            "assignee" => {
                                let cur = v.parse::<u64>().ok().and_then(|id| members.iter().find(|m| m.0 == id)).map(|m| m.1.clone()).unwrap_or_else(|| tr("Choose someone").into());
                                egui::ComboBox::from_id_salt(("fa", i)).selected_text(cur).show_ui(ui, |ui| {
                                    for (id, n) in &members {
                                        ui.selectable_value(v, id.to_string(), n);
                                    }
                                });
                            }
                            "checkbox" => {
                                let mut on = v == "Yes";
                                let cb = check(ui, &mut on, tr("Yes"));
                                name_it(&cb, &format!("Answer {}", i + 1));
                                if cb.changed() {
                                    *v = if on { "Yes".into() } else { String::new() };
                                }
                            }
                            "description" | "text" => {
                                let r = ui.add(field_multi(v).desired_rows(3).desired_width(f32::INFINITY));
                                name_it(&r, &format!("Answer {}", i + 1));
                            }
                            k => {
                                let hint = match k {
                                    "number" => "0",
                                    "email" => "name@example.com",
                                    _ => "",
                                };
                                let r = ui.add(field(v).hint_text(hint).desired_width(f32::INFINITY));
                                name_it(&r, &format!("Answer {}", i + 1));
                            }
                        }
                        ui.add_space(6.0);
                    }
                    if let Some(e) = &self.sweep.form_error {
                        ui.label(RichText::new(tr(e)).color(t.danger));
                    }
                    if primary_button(ui, &t, tr("Submit")).clicked() {
                        match form_answers(&f, &self.form_fill) {
                            Err(e) => self.sweep.form_error = Some(e),
                            Ok(_) if self.data.space_of_list(f.list_id).is_none() => {
                                // the List was never chosen or has been deleted
                                self.toast = Some((tr("This form has no List for its answers. Choose one under Edit.").into(), now_ts()));
                                self.sweep.form_error = None;
                                self.form_mode_fill = false;
                            }
                            Ok(a) => {
                                let name = if a.name.trim().is_empty() { format!("{} submission", f.name) } else { a.name };
                                self.actions.push(Action::Add { list: f.list_id, name, status: None, priority: a.priority, assignees: a.assignees, due: a.due, parent: None, open: false });
                                self.pending_description = Some(a.description.trim().to_string());
                                // tells the form's owner (notify.rs); shared history, so in English
                                self.pending_log = Some(format!("submitted the form {}", f.name));
                                self.form_fill.clear();
                                self.sweep.form_error = None;
                                self.toast = Some((trf!("Submitted. A task was created in {}", self.data.list_name(f.list_id)), now_ts()));
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
        ui.painter().text(
            r.left_center() + vec2(14.0, 0.0),
            Align2::CENTER_CENTER,
            icon,
            FontId::proportional(13.0),
            t.text2,
        );
        ui.painter().with_clip_rect(r).text(
            r.left_center() + vec2(30.0, 0.0),
            Align2::LEFT_CENTER,
            tr(label),
            FontId::proportional(14.0),
            t.text,
        );
        resp
    }

    // ---------------------------------------------------------------- whiteboards

    pub(super) fn whiteboard_canvas(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(t.bg)
                    .inner_margin(egui::Margin::same(12)),
            )
            .show_inside(ui, |ui| {
                let Some(wi) = self
                    .wb_open
                    .and_then(|id| self.data.extra.whiteboards.iter().position(|w| w.id == id))
                else {
                    ui.label(
                        RichText::new(tr(
                            "Create a whiteboard to sketch ideas and add sticky notes.",
                        ))
                        .color(t.text2),
                    );
                    return;
                };
                let wid = self.data.extra.whiteboards[wi].id;
                let mut delete = false;
                let mut btn_zoom: Option<f32> = None;
                let mut reset_view = false;
                if self.sweep.wb_width <= 0.0 {
                    self.sweep.wb_width = 2.5;
                }
                // wraps onto a second row in a narrow window
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.data.extra.whiteboards[wi].name)
                                .font(FontId::proportional(18.0))
                                .frame(false)
                                .desired_width(200.0),
                        )
                        .changed()
                    {
                        self.dirty = true;
                    }
                    ui.separator();
                    // the tool in use is outlined; each tool has its own icon
                    // (the pen and the sticky note shared one emoji)
                    for (tool, kind, label) in [
                        (0u8, "pen", "Pen"),
                        (3, "eraser", "Eraser"),
                        (1, "sticky", "Sticky note"),
                        (2, "move", "Move notes"),
                    ] {
                        let sel = self.wb_tool == tool;
                        let text_color = if sel { t.text } else { t.text2 };
                        if icon_button_ex(ui, &t, kind, tr(label), label, sel, 0.0, text_color, t.text2)
                            .clicked()
                        {
                            self.wb_tool = tool;
                        }
                    }
                    for c in WB_COLORS {
                        let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 8.0, rgb(c));
                        hover_ring(ui, &resp, r.shrink(1.0), 9.0);
                        if self.wb_color == c {
                            ui.painter()
                                .circle_stroke(r.center(), 10.0, Stroke::new(1.5, t.text));
                        }
                        if resp.clicked() {
                            self.wb_color = c;
                        }
                    }
                    let wr = ui.add(
                        egui::DragValue::new(&mut self.sweep.wb_width)
                            .range(1.0..=16.0)
                            .speed(0.2)
                            .max_decimals(1)
                            .prefix(tr("width ")),
                    );
                    name_it(&wr, "Pen width");
                    ui.separator();
                    let can_undo = self.sweep.wb_undo.iter().any(|u| u.0 == wid);
                    if ui
                        .add_enabled(can_undo, egui::Button::new(tr("Undo")))
                        .clicked()
                    {
                        self.wb_undo(wid);
                    }
                    ui.separator();
                    let zr = ui.button("\u{2212}").on_hover_text(tr("Zoom out"));
                    name_it(&zr, "Zoom out");
                    if zr.clicked() {
                        btn_zoom = Some(1.0 / 1.25);
                    }
                    let zoom_now = self.sweep.wb_view.get(&wid).map_or(1.0, |v| v.zoom);
                    ui.label(
                        RichText::new(format!("{:.0}%", zoom_now * 100.0))
                            .color(t.text2)
                            .small(),
                    );
                    let zr = ui.button("+").on_hover_text(tr("Zoom in"));
                    name_it(&zr, "Zoom in");
                    if zr.clicked() {
                        btn_zoom = Some(1.25);
                    }
                    let rr = ui.button(tr("Reset view")).on_hover_text(tr(
                        "Back to 100% at the board's top left corner. Drag with Space or the middle button, or scroll, to move around; pinch or \u{2318}+scroll to zoom.",
                    ));
                    name_it(&rr, "Reset view");
                    if rr.clicked() {
                        reset_view = true;
                    }
                    // the question belongs to this board only
                    let asking = self
                        .sweep
                        .confirm
                        .clone()
                        .and_then(|c| c.strip_suffix(&format!(":{}", wid)).map(String::from));
                    match asking.as_deref() {
                        Some("wb_clear") => {
                            ui.label(RichText::new(tr("Clear the whole board?")).color(t.danger));
                            if ui.button(tr("Yes")).clicked() {
                                self.sweep.confirm = None;
                                let wb = &mut self.data.extra.whiteboards[wi];
                                wb.strokes.clear();
                                wb.notes.clear();
                                self.sweep.wb_undo.retain(|u| u.0 != wid);
                                self.dirty = true;
                            }
                            if ui.button(tr("No")).clicked() {
                                self.sweep.confirm = None;
                            }
                        }
                        _ => {
                            if ui.button(tr("Clear")).clicked() {
                                self.sweep.confirm = Some(format!("wb_clear:{}", wid));
                            }
                            // framed like the other buttons in this bar
                            if ui
                                .button(RichText::new(tr("Delete")).color(t.danger))
                                .clicked()
                            {
                                delete = true;
                            }
                        }
                    }
                    let wid = self.data.extra.whiteboards[wi].id;
                    self.star_toggle(ui, format!("wb:{}", wid));
                });
                let (rect, resp) =
                    ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                name_it(&resp, "Whiteboard canvas");
                crate::widgets::no_pointer(&resp);
                let origin = rect.min;
                // pan and zoom: Space or middle-button drag and two-finger
                // scroll move the view; pinch, Cmd+scroll and the buttons zoom
                let mut view = self.sweep.wb_view.get(&wid).copied().unwrap_or_default();
                if reset_view {
                    view = WbView::default();
                }
                if let Some(f) = btn_zoom {
                    view.zoom_at(origin, rect.center(), f);
                }
                let space = !ui.ctx().wants_keyboard_input()
                    && ui.input(|i| i.key_down(egui::Key::Space));
                let panning = resp.dragged_by(egui::PointerButton::Middle)
                    || (space && resp.dragged_by(egui::PointerButton::Primary));
                if panning {
                    let started = resp.drag_started_by(egui::PointerButton::Middle)
                        || resp.drag_started_by(egui::PointerButton::Primary);
                    // the first step counts from where the button went down
                    let from_press = ui.input(|i| {
                        i.pointer
                            .press_origin()
                            .zip(i.pointer.interact_pos())
                            .map(|(o, p)| p - o)
                    });
                    view.pan += match from_press {
                        Some(d) if started => d,
                        _ => resp.drag_delta(),
                    };
                }
                if ui.rect_contains_pointer(rect) {
                    let (zoom, scroll, at) = ui.input(|i| {
                        (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos())
                    });
                    if zoom != 1.0 {
                        if let Some(at) = at {
                            view.zoom_at(origin, at, zoom);
                        }
                    } else if scroll != egui::Vec2::ZERO {
                        view.pan += scroll;
                    }
                }
                if space && ui.rect_contains_pointer(rect) {
                    ui.ctx().set_cursor_icon(if panning {
                        egui::CursorIcon::Grabbing
                    } else {
                        egui::CursorIcon::Grab
                    });
                }
                if view == WbView::default() {
                    self.sweep.wb_view.remove(&wid);
                } else {
                    self.sweep.wb_view.insert(wid, view);
                }
                let z = view.zoom;
                let p = ui.painter_at(rect);
                p.rect_filled(rect, 8.0, t.column_bg);
                // dot grid, moving with the board
                let step = 24.0 * z;
                if step >= 6.0 {
                    let off = view.pan;
                    let first = |lo: f32, o: f32| lo + (o.rem_euclid(step));
                    let mut y = first(rect.top(), off.y);
                    while y < rect.bottom() {
                        let mut x = first(rect.left(), off.x);
                        while x < rect.right() {
                            p.circle_filled(pos2(x, y), 1.0, t.border);
                            x += step;
                        }
                        y += step;
                    }
                }
                // strokes and notes put in without an id (an older copy)
                // get theirs before anything goes by id
                self.data.fix_board_ids();
                let wb = &mut self.data.extra.whiteboards[wi];
                for s in &wb.strokes {
                    let pts: Vec<egui::Pos2> =
                        s.points.iter().map(|q| view.to_screen(origin, *q)).collect();
                    p.add(egui::Shape::line(
                        pts,
                        Stroke::new(s.width * z, rgb(s.color)),
                    ));
                }
                let mut drag_note = None;
                let mut move_from = None;
                let mut drop_note = None;
                let mut recolor = None;
                for (i, n) in wb.notes.iter_mut().enumerate() {
                    // every widget of a note takes its id from the note, so
                    // notes skipped off screen don't hand their ids (and the
                    // typing focus) to another note
                    let key = if n.id != 0 { n.id } else { (1 << 63) | i as u64 };
                    let r = Rect::from_min_size(view.to_screen(origin, n.pos), vec2(160.0, 120.0) * z);
                    // notes off the visible part of the board are not drawn
                    if !r.intersects(rect) {
                        continue;
                    }
                    p.rect_filled(
                        r.translate(vec2(2.0, 3.0) * z),
                        4.0 * z,
                        Color32::from_black_alpha(25),
                    );
                    p.rect_filled(r, 4.0 * z, rgb(n.color).gamma_multiply(0.35).to_opaque());
                    let inner =
                        Rect::from_min_max(r.min + vec2(8.0, 18.0) * z, r.max - vec2(8.0, 8.0) * z);
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt(("wb_note", wid, key))
                            .max_rect(inner),
                    );
                    child.set_clip_rect(inner.intersect(rect));
                    if z >= 0.45
                        && child
                            .add(
                                egui::TextEdit::multiline(&mut n.text)
                                    .id(egui::Id::new(("wb_note_text", wid, key)))
                                    .frame(false)
                                    .font(FontId::proportional(13.0 * z))
                                    .desired_width(144.0 * z)
                                    .desired_rows(4),
                            )
                            .changed()
                    {
                        self.dirty = true;
                    }
                    if z < 0.45 {
                        // too small to type in: the text is only drawn
                        p.with_clip_rect(inner.intersect(rect)).text(
                            inner.left_top(),
                            Align2::LEFT_TOP,
                            &n.text,
                            FontId::proportional(13.0 * z),
                            t.text,
                        );
                    }
                    if self.wb_tool == 2 && !space {
                        let h = ui.interact(r.intersect(rect), ui.id().with(("note", wid, key)), Sense::drag());
                        if h.drag_started() {
                            move_from = Some((n.id, n.pos));
                        }
                        if h.dragged() {
                            drag_note = Some((i, h.drag_delta() / z));
                        }
                    }
                    // a colour dot along the note's top edge recolours it
                    let dot =
                        Rect::from_center_size(r.left_top() + vec2(12.0, 10.0) * z, vec2(14.0, 14.0));
                    if rect.contains_rect(dot) {
                        let dr =
                            ui.interact(dot, ui.id().with(("note_color", wid, key)), Sense::click());
                        name_it(&dr, &format!("Note colour {}", i + 1));
                        p.circle_filled(dot.center(), 5.0, rgb(n.color));
                        hover_ring(ui, &dr, dot.shrink(1.0), 6.0);
                        p.circle_stroke(
                            dot.center(),
                            5.0,
                            Stroke::new(1.0, Color32::from_black_alpha(60)),
                        );
                        popup(
                            ui,
                            &dr,
                            ui.id().with(("note_color_pop", wid, key)),
                            150.0,
                            |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    for c in WB_COLORS
                                        .iter()
                                        .skip(1)
                                        .chain(STATUS_PALETTE.iter().skip(5).step_by(2))
                                    {
                                        let (cr, cresp) = ui
                                            .allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                                        ui.painter().circle_filled(cr.center(), 8.0, rgb(*c));
                                        hover_ring(ui, &cresp, cr.shrink(1.0), 9.0);
                                        if *c == n.color {
                                            ui.painter().circle_stroke(
                                                cr.center(),
                                                10.0,
                                                Stroke::new(1.5, t.text),
                                            );
                                        }
                                        if cresp.clicked() {
                                            recolor = Some((i, *c));
                                            close_popup(ui);
                                        }
                                    }
                                });
                            },
                        );
                    }
                    // a small x on the corner deletes just this note
                    let xr =
                        Rect::from_center_size(r.right_top() + vec2(-9.0, 9.0) * z, vec2(16.0, 16.0));
                    if rect.contains_rect(xr) {
                        let x = ui.interact(xr, ui.id().with(("note_x", wid, key)), Sense::click());
                        hover_bg(ui, &x, xr, 4.0);
                        p.text(
                            xr.center(),
                            Align2::CENTER_CENTER,
                            "\u{00D7}",
                            FontId::proportional(9.0),
                            if x.hovered() { t.text } else { t.text3 },
                        );
                        if x.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let x = x.on_hover_text(tr("Delete note"));
                        name_it(&x, &format!("Delete note {}", i));
                        if x.clicked() {
                            drop_note = Some(i);
                        }
                    }
                }
                if let Some((i, c)) = recolor {
                    wb.notes[i].color = c;
                    self.dirty = true;
                }
                if let Some(i) = drop_note {
                    // Undo brings it back
                    let n = wb.notes.remove(i);
                    self.sweep.wb_undo.push((wid, WbUndo::NoteGone(i, n)));
                    self.dirty = true;
                }
                if let Some((id, at)) = move_from {
                    self.sweep.wb_undo.push((wid, WbUndo::NoteMoved(id, at)));
                }
                if let Some((i, d)) = drag_note {
                    wb.notes[i].pos[0] += d.x;
                    wb.notes[i].pos[1] += d.y;
                    self.dirty = true;
                }
                // the tools draw with the left button, in board coordinates;
                // a pan (Space or the middle button) never draws
                let primary = resp.dragged_by(egui::PointerButton::Primary)
                    || resp.drag_started_by(egui::PointerButton::Primary)
                    || resp.drag_stopped_by(egui::PointerButton::Primary)
                    || resp.clicked();
                if let Some(pos) = resp.interact_pointer_pos().filter(|_| primary && !space) {
                    let b = view.to_board(origin, pos);
                    let local = vec2(b[0], b[1]);
                    match self.wb_tool {
                        0 => {
                            if resp.drag_started() {
                                // the line starts where the button went down
                                let start = ui
                                    .input(|i| i.pointer.press_origin())
                                    .map(|o| view.to_board(origin, o));
                                let mut points: Vec<[f32; 2]> = start.into_iter().collect();
                                points.push([local.x, local.y]);
                                // a real id from the first point on: a merge
                                // while the button is still down must see a
                                // board whose every item has one (see
                                // wb_sync::is_id_array)
                                let id = self.data.new_id();
                                self.data.extra.whiteboards[wi].strokes.push(WbStroke {
                                    id,
                                    color: self.wb_color,
                                    width: self.sweep.wb_width,
                                    points,
                                });
                                self.sweep.wb_drawing = Some((wid, id));
                                self.sweep.wb_undo.push((wid, WbUndo::Stroke(id)));
                                self.dirty = true;
                            } else if resp.dragged() {
                                // the stroke being drawn, wherever a merge put
                                // it (a teammate's strokes can come after it)
                                let drawing = self.sweep.wb_drawing.filter(|d| d.0 == wid);
                                if let Some(s) = drawing.and_then(|(_, sid)| {
                                    self.data.extra.whiteboards[wi]
                                        .strokes
                                        .iter_mut()
                                        .find(|s| s.id == sid)
                                }) {
                                    s.points.push([local.x, local.y]);
                                }
                            }
                            if resp.drag_stopped() {
                                self.sweep.wb_drawing = None;
                                self.dirty = true;
                            }
                        }
                        1 => {
                            if resp.clicked() {
                                let id = self.data.new_id();
                                self.data.extra.whiteboards[wi].notes.push(Sticky {
                                    id,
                                    pos: [local.x - 80.0, local.y - 60.0],
                                    text: String::new(),
                                    color: if self.wb_color == [32, 32, 32] {
                                        [0xf8, 0xae, 0x00]
                                    } else {
                                        self.wb_color
                                    },
                                });
                                self.sweep.wb_undo.push((wid, WbUndo::Note(id)));
                                self.dirty = true;
                            }
                        }
                        3 => {
                            // the eraser takes away whole strokes it touches
                            if resp.dragged() || resp.clicked() {
                                let hit = |s: &WbStroke| {
                                    s.points.iter().any(|q| {
                                        vec2(q[0] - local.x, q[1] - local.y).length()
                                            < s.width + 6.0 / z
                                    })
                                };
                                let wb = &mut self.data.extra.whiteboards[wi];
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
                    let w = &self.data.extra.whiteboards[wi];
                    self.modal = Some(Modal::ConfirmRemove {
                        kind: "whiteboard",
                        id: w.id,
                        name: w.name.clone(),
                    });
                }
            });
    }

    /// Takes back the last pen stroke, note, note removal, note move or erasing on a board.
    pub(super) fn wb_undo(&mut self, wid: u64) {
        let Some(k) = self.sweep.wb_undo.iter().rposition(|u| u.0 == wid) else {
            return;
        };
        let (_, op) = self.sweep.wb_undo.remove(k);
        let Some(wb) = self.data.extra.whiteboards.iter_mut().find(|w| w.id == wid) else {
            return;
        };
        // by id: a stroke or note a teammate took away meanwhile is left
        // alone (nothing happens), and theirs are never touched
        let done = match op {
            WbUndo::Stroke(id) => match wb.strokes.iter().position(|s| s.id == id) {
                Some(i) => {
                    wb.strokes.remove(i);
                    true
                }
                None => false,
            },
            WbUndo::Note(id) => match wb.notes.iter().position(|n| n.id == id) {
                Some(i) => {
                    wb.notes.remove(i);
                    true
                }
                None => false,
            },
            WbUndo::NoteGone(i, n) => {
                let back = !wb.notes.iter().any(|x| x.id == n.id);
                if back {
                    wb.notes.insert(i.min(wb.notes.len()), n);
                }
                back
            }
            WbUndo::StrokeGone(i, s) => {
                let back = !wb.strokes.iter().any(|x| x.id == s.id);
                if back {
                    wb.strokes.insert(i.min(wb.strokes.len()), s);
                }
                back
            }
            WbUndo::NoteMoved(id, at) => match wb.notes.iter_mut().find(|n| n.id == id) {
                Some(n) => {
                    n.pos = at;
                    true
                }
                None => false,
            },
        };
        if done {
            self.dirty = true;
        }
    }

    // ---------------------------------------------------------------- reminders

    pub(super) fn reminders_block(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let now = now_ts();
        let show_done = self.sweep.reminders_done;
        let mine: Vec<Reminder> = self
            .data
            .extra
            .reminders
            .iter()
            .filter(|r| r.owner == me)
            .cloned()
            .collect();
        let n_done = mine.iter().filter(|r| r.done).count();
        let mut rs: Vec<Reminder> = mine.into_iter().filter(|r| show_done || !r.done).collect();
        if rs.is_empty() && n_done == 0 {
            return;
        }
        rs.sort_by_key(|r| (r.done, r.due_ts));
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Reminders")).strong());
            if n_done > 0 {
                let b = ui.add(ghost(egui::Button::new(
                    RichText::new(if show_done {
                        tr("Hide done").to_string()
                    } else {
                        trf!("Show done ({})", n_done)
                    })
                    .small()
                    .color(t.text2),
                )));
                name_it(&b, "Show done reminders");
                if b.clicked() {
                    self.sweep.reminders_done = !show_done;
                }
            }
        });
        let mut delete = None;
        for r in rs {
            if let Some((id, text, date, time)) =
                self.sweep.reminder_edit.as_mut().filter(|e| e.0 == r.id)
            {
                let id = *id;
                let mut save = false;
                let mut cancel = false;
                ui.horizontal(|ui| {
                    let te = ui.add(field(text).desired_width(220.0));
                    name_it(&te, "Reminder text");
                    let b = icon_button(
                        ui,
                        &t,
                        "calendar",
                        &date.map(fmt_date).unwrap_or_else(|| tr("Date").into()),
                        "Reminder date",
                        true,
                    );
                    popup(ui, &b, ui.id().with(("rem_edit_date", id)), 380.0, |ui| {
                        date_menu(ui, &t, ui.id().with(("rem_edit_cal", id)), date)
                    });
                    ui.add(field(time).hint_text("09:00").desired_width(54.0));
                    save = ui.button(tr("Save")).clicked()
                        || (te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                    cancel = ui.button(tr("Cancel")).clicked();
                });
                if save && !text.trim().is_empty() {
                    let ts = reminder_ts(date.unwrap_or_else(today), time);
                    let text = text.trim().to_string();
                    if let Some(x) = self.data.extra.reminders.iter_mut().find(|x| x.id == id) {
                        x.text = text;
                        x.due_ts = ts;
                        // a new time alerts again
                        x.alerted = false;
                        self.dirty = true;
                    }
                    self.sweep.reminder_edit = None;
                } else if cancel {
                    self.sweep.reminder_edit = None;
                }
                continue;
            }
            ui.horizontal(|ui| {
                let mut done = r.done;
                let cb = check(ui, &mut done, "");
                name_it(&cb, &format!("Done {}", r.text));
                if cb.changed() {
                    if let Some(x) = self.data.extra.reminders.iter_mut().find(|x| x.id == r.id) {
                        x.done = done;
                        self.dirty = true;
                    }
                }
                let text = if r.done {
                    RichText::new(&r.text).strikethrough().color(t.text3)
                } else {
                    RichText::new(&r.text)
                };
                if r.task.is_some() || r.message.is_some() {
                    // opens the task or message it is about
                    if ui.link(text).clicked() {
                        self.open_reminder(&r);
                    }
                } else {
                    ui.label(text);
                }
                let when = chrono::DateTime::from_timestamp(r.due_ts, 0)
                    .map(|d| d.with_timezone(&chrono::Local).text("%b %-d, %-I:%M %p"))
                    .unwrap_or_default();
                ui.label(
                    RichText::new(when)
                        .small()
                        .color(if r.due_ts < now && !r.done {
                            t.danger
                        } else {
                            t.text3
                        }),
                );
                // snoozing only ever moves a reminder later
                let later: Vec<(&str, i64)> = [
                    ("In 1 hour", now + 3600),
                    (
                        "Tomorrow 9 AM",
                        reminder_ts(today() + Duration::days(1), "09:00"),
                    ),
                    (
                        "Next week",
                        reminder_ts(today() + Duration::days(7), "09:00"),
                    ),
                ]
                .into_iter()
                .filter(|(_, ts)| *ts > r.due_ts)
                .collect();
                if !r.done && !later.is_empty() {
                    let sz = ui.add(ghost(egui::Button::new(
                        RichText::new(tr("Snooze")).small(),
                    )));
                    name_it(&sz, &format!("Snooze {}", r.text));
                    popup(ui, &sz, ui.id().with(("rem_snooze", r.id)), 170.0, |ui| {
                        for (label, ts) in later {
                            if ui.button(tr(label)).clicked() {
                                if let Some(x) =
                                    self.data.extra.reminders.iter_mut().find(|x| x.id == r.id)
                                {
                                    x.due_ts = ts;
                                    x.alerted = false;
                                    self.dirty = true;
                                }
                                close_popup(ui);
                            }
                        }
                    });
                }
                let ed = ui.add(ghost(egui::Button::new(RichText::new(tr("Edit")).small())));
                name_it(&ed, &format!("Edit {}", r.text));
                if ed.clicked() {
                    let local = chrono::DateTime::from_timestamp(r.due_ts, 0)
                        .map(|d| d.with_timezone(&chrono::Local));
                    let date = local.map(|d| d.date_naive());
                    let time = local
                        .map(|d| d.format("%H:%M").to_string())
                        .unwrap_or_else(|| "09:00".into());
                    self.sweep.reminder_edit = Some((r.id, r.text.clone(), date, time));
                }
                let del = ui.add(ghost(egui::Button::new(
                    RichText::new("\u{00D7}").small().color(t.text3),
                )));
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
        self.data
            .extra
            .reminders
            .iter()
            .filter(|r| r.owner == self.data.me && !r.done && r.due_ts <= now)
            .count()
    }

    /// My reminders that are due and my meetings today and tomorrow, for
    /// Home: (reminders, meetings), soonest first.
    pub(super) fn home_due_extras(&self) -> (Vec<Reminder>, Vec<Meeting>) {
        let (me, now, td) = (self.data.me, now_ts(), today());
        let mut rs: Vec<Reminder> = self
            .data
            .extra
            .reminders
            .iter()
            .filter(|r| r.owner == me && !r.done && r.due_ts <= now)
            .cloned()
            .collect();
        rs.sort_by_key(|r| r.due_ts);
        let mut ms: Vec<Meeting> = self
            .data
            .extra
            .meetings
            .iter()
            .filter(|m| m.attendees.contains(&me) && (0..=1).contains(&(m.date - td).num_days()))
            .cloned()
            .collect();
        ms.sort_by(|a, b| (a.date, &a.time).cmp(&(b.date, &b.time)));
        (rs, ms)
    }

    /// The Reminders and Meetings rows under My Work on Home.
    pub(super) fn home_extras_rows(&mut self, ui: &mut Ui, w: f32) {
        let t = self.t();
        let (rs, ms) = self.home_due_extras();
        let td = today();
        let mut go = None;
        let rows: [(&str, Vec<(String, String, String)>); 2] = [
            (
                "Reminders",
                rs.iter()
                    .map(|r| {
                        let when = chrono::DateTime::from_timestamp(r.due_ts, 0)
                            .map(|d| d.with_timezone(&chrono::Local).text("%b %-d, %-I:%M %p"))
                            .unwrap_or_default();
                        (r.text.clone(), when, format!("reminder:{}", r.id))
                    })
                    .collect(),
            ),
            (
                "Meetings",
                ms.iter()
                    .map(|m| {
                        let day = if m.date == td { "Today" } else { "Tomorrow" };
                        (
                            m.title.clone(),
                            format!("{} {}", tr(day), m.time),
                            format!("meeting:{}", m.id),
                        )
                    })
                    .collect(),
            ),
        ];
        for (name, items) in rows {
            if items.is_empty() {
                continue;
            }
            let (r, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
            ui.painter().text(
                r.left_center() + vec2(18.0, 0.0),
                Align2::LEFT_CENTER,
                tr(name),
                FontId::proportional(13.5),
                t.text,
            );
            ui.painter().text(
                r.left_center() + vec2(110.0, 0.0),
                Align2::LEFT_CENTER,
                items.len().to_string(),
                FontId::proportional(12.0),
                t.text3,
            );
            for (text, when, target) in items {
                let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                name_it(&resp, &text);
                if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                ui.painter()
                    .with_clip_rect(Rect::from_min_max(
                        r.min,
                        pos2(r.right() - 140.0, r.bottom()),
                    ))
                    .text(
                        r.left_center() + vec2(40.0, 0.0),
                        Align2::LEFT_CENTER,
                        &text,
                        FontId::proportional(13.0),
                        t.text,
                    );
                ui.painter().text(
                    r.right_center() - vec2(6.0, 0.0),
                    Align2::RIGHT_CENTER,
                    when,
                    FontId::proportional(12.0),
                    if name == "Reminders" {
                        t.danger
                    } else {
                        t.text2
                    },
                );
                if resp.clicked() {
                    go = Some(target);
                }
            }
        }
        if let Some(target) = go {
            self.open_target(&target);
        }
    }

    /// My Work on Home with nothing assigned to me and nothing due: what
    /// shows up here, and a way to add the first task.
    pub(super) fn home_nothing_yet(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.label(RichText::new(tr("Nothing assigned to you yet.")).color(t.text2));
        ui.label(
            RichText::new(tr(
                "Tasks assigned to you show up here by due date, with your due reminders and upcoming meetings.",
            ))
            .small()
            .color(t.text3),
        );
        ui.add_space(6.0);
        if self.default_list().is_none() {
            ui.label(
                RichText::new(tr("Tasks live in Lists: make one in the Lists card first."))
                    .small()
                    .color(t.text3),
            );
        } else if ui.button(tr("+ Add a task for me")).clicked() {
            self.open_quick_add(None);
            if let Some(qa) = self.quick_add.as_mut() {
                if !qa.assignees.contains(&self.data.me) {
                    qa.assignees.push(self.data.me);
                }
            }
        }
    }

    // ---------------------------------------------------------------- AI

    pub(super) fn ai_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(40, 40)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(26.0, 28.0), Sense::hover());
                line_icon_scaled(ui.painter(), "sparkle", r.center(), t.text, 1.4);
                ui.label(RichText::new(tr("AI")).size(24.0));
            });
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Ask, Build, Create, Skills, Artifacts, Super Agents, AI Chats and the AI Notetaker need an AI service.")).color(t.text2));
            ui.label(RichText::new(tr("They will be connected together with the shared server. Everything else works now.")).color(t.text2));
            ui.add_space(16.0);
            // read-only rather than disabled: a disabled field fades its hint
            // to an unreadable grey on the dark theme
            ui.add(egui::TextEdit::multiline(&mut String::new()).interactive(false).hint_text(RichText::new(tr("Ask anything... (available with the server)")).color(t.text3)).desired_rows(3).desired_width(600.0));
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
                let add = ui.add_enabled(!hidden.is_empty(), egui::Button::new(tr("+ Add card")));
                popup(ui, &add, egui::Id::new("ov_add_card"), 180.0, |ui| {
                    for c in &hidden {
                        let b = ui.button(tr(c));
                        name_it(&b, &format!("Add card {}", c));
                        if b.clicked() {
                            self.data.extra.hidden_cards.retain(|x| x != c);
                            self.dirty = true;
                            close_popup(ui);
                        }
                    }
                });
                let cz = icon_button(ui, &t, "settings", tr("Customize"), "Customize", true);
                popup(ui, &cz, egui::Id::new("ov_customize"), 200.0, |ui| {
                    ui.label(RichText::new(tr("Cards")).small().color(t.text3));
                    for c in OV_CARDS {
                        let mut on = !self.data.extra.hidden_cards.iter().any(|x| x == c);
                        let cb = check(ui, &mut on, tr(c));
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
                        ui.label(RichText::new(tr("Your recently opened items will appear here.")).color(tt.text2));
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
                            ui.label(RichText::new(trf!("\u{2022} in {}", place)).small().color(tt.text3));
                        });
                    }
                });
                }
                // Docs
                let space = self.current_space();
                if card_on(self, "Docs") {
                self.ov_card(ui, cw, "Docs", |s, ui| {
                    let me = s.data.me;
                    let docs: Vec<(u64, String)> = s.data.docs.iter().filter(|d| (space.is_none() || d.space == space) && d.parent.is_none() && !d.archived && (!d.private || d.owner == me)).map(|d| (d.id, d.title.clone())).collect();
                    if docs.is_empty() {
                        ui.label(RichText::new(tr("There are no Docs in this location yet.")).color(s.t().text2));
                    }
                    for (id, n) in docs {
                        if ui.link(format!("\u{1F4C4} {}", n)).clicked() {
                            s.doc_open = Some(id);
                            s.section = Section::Docs;
                        }
                    }
                    ui.add_space(6.0);
                    if primary_button(ui, &s.t(), tr("Add a Doc")).clicked() {
                        s.new_doc(space);
                    }
                });
                }
                // Bookmarks
                if card_on(self, "Bookmarks") {
                self.ov_card(ui, cw, "Bookmarks", |s, ui| {
                    let bms = s.data.extra.bookmarks.clone();
                    if bms.is_empty() {
                        ui.label(RichText::new(tr("Bookmarks make it easy to save items or any URL from around the web.")).color(s.t().text2));
                    }
                    let mut remove = None;
                    for b in &bms {
                        ui.horizontal(|ui| {
                            if ui.link(format!("\u{1F516} {}", b.title)).on_hover_text(&b.url).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(&b.url));
                            }
                            if ui.add(ghost(egui::Button::new(RichText::new("\u{00D7}").size(10.0).color(s.t().text3)))).clicked() {
                                remove = Some(b.id);
                            }
                        });
                    }
                    if let Some(id) = remove {
                        s.data.extra.bookmarks.retain(|b| b.id != id);
                        s.dirty = true;
                    }
                    ui.add_space(6.0);
                    if primary_button(ui, &s.t(), tr("Add bookmark")).clicked() {
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
                        ui.label(RichText::new(tr("Add new Folder to your Space")).color(s.t().text2));
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (id, n, c) in &folders {
                            let label = if *c == 1 { trf!("\u{1F4C1} {}  (1 list)", n) } else { trf!("\u{1F4C1} {}  ({} lists)", n, c) };
                            if ui.add(egui::Button::new(label).min_size(vec2(200.0, 40.0)).stroke(Stroke::new(1.0, s.t().border))).clicked() {
                                s.actions.push(Action::Go(Location::Folder(*id)));
                            }
                        }
                    });
                    if let Location::Space(sid) = s.loc {
                        ui.add_space(6.0);
                        if primary_button(ui, &s.t(), tr("Add Folder")).clicked() {
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
        const COLS: [(f32, &str); 7] = [
            (0.0, "Name"),
            (0.28, "Color"),
            (0.36, "Progress"),
            (0.55, "Start"),
            (0.66, "End"),
            (0.77, "Priority"),
            (0.88, "Owner"),
        ];
        let (hr, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
        for (x, h) in COLS {
            ui.painter().text(
                pos2(hr.left() + w * x, hr.center().y),
                Align2::LEFT_CENTER,
                tr(h),
                FontId::proportional(12.5),
                tt.text2,
            );
        }
        let cell = |r: Rect, i: usize| {
            let x0 = r.left() + w * COLS[i].0;
            let x1 = if i + 1 < COLS.len() {
                r.left() + w * COLS[i + 1].0
            } else {
                r.right()
            };
            Rect::from_min_max(pos2(x0, r.top()), pos2(x1 - 6.0, r.bottom()))
        };
        for lid in lists {
            let Some(item) = self.data.list_item(*lid).cloned().filter(|l| !l.archived) else {
                continue;
            };
            let total = self
                .data
                .tasks
                .iter()
                .filter(|x| x.list_id == *lid && x.parent.is_none() && !x.archived)
                .count();
            let done = self
                .data
                .tasks
                .iter()
                .filter(|x| {
                    x.list_id == *lid && x.parent.is_none() && !x.archived && self.data.is_closed(x)
                })
                .count();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
            name_it(&resp, &format!("Overview list {}", item.name));
            if resp.hovered() {
                ui.painter().rect_filled(r, 5.0, tt.hover);
            }
            let p = ui.painter().clone();
            p.with_clip_rect(cell(r, 0)).text(
                r.left_center() + vec2(4.0, 0.0),
                Align2::LEFT_CENTER,
                &item.name,
                FontId::proportional(14.0),
                tt.text,
            );
            let mut edited = item.clone();
            // color
            let c0 = cell(r, 1);
            let cr = ui.interact(c0, ui.id().with(("ov_color", lid)), Sense::click());
            hover_bg(ui, &cr, c0.shrink(2.0), 4.0);
            name_it(&cr, &format!("Color of {}", item.name));
            match item.color {
                Some(c) => {
                    p.circle_filled(pos2(c0.left() + 8.0, c0.center().y), 6.0, rgb(c));
                }
                None => {
                    p.circle_stroke(
                        pos2(c0.left() + 8.0, c0.center().y),
                        6.0,
                        Stroke::new(1.0, tt.text3),
                    );
                }
            }
            popup(ui, &cr, ui.id().with(("ov_color_p", lid)), 160.0, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for c in STATUS_PALETTE.iter().skip(1) {
                        let (b, br) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                        ui.painter().circle_filled(b.center(), 8.0, rgb(*c));
                        hover_ring(ui, &br, b.shrink(1.0), 9.0);
                        if br.clicked() {
                            edited.color = Some(*c);
                            close_popup(ui);
                        }
                    }
                });
                if ui.small_button(tr("No color")).clicked() {
                    edited.color = None;
                    close_popup(ui);
                }
            });
            // progress
            let c2 = cell(r, 2);
            let bar = Rect::from_min_size(
                pos2(c2.left(), c2.center().y - 3.0),
                vec2((c2.width() - 40.0).max(20.0), 6.0),
            );
            p.rect_filled(bar, 3.0, tt.hover);
            if total > 0 {
                p.rect_filled(
                    Rect::from_min_size(
                        bar.min,
                        vec2(bar.width() * done as f32 / total as f32, 6.0),
                    ),
                    3.0,
                    rgb(COMPLETE_COLOR),
                );
            }
            p.text(
                pos2(bar.right() + 6.0, c2.center().y),
                Align2::LEFT_CENTER,
                format!("{}/{}", done, total),
                FontId::proportional(12.0),
                tt.text2,
            );
            // start / end
            for (i, which) in [(3usize, 0u8), (4, 1)] {
                let c = cell(r, i);
                let dr = ui.interact(c, ui.id().with(("ov_date", lid, which)), Sense::click());
                hover_bg(ui, &dr, c.shrink(2.0), 4.0);
                name_it(
                    &dr,
                    &format!(
                        "{} of {}",
                        if which == 0 { "Start" } else { "End" },
                        item.name
                    ),
                );
                let v = if which == 0 { item.start } else { item.end };
                match v {
                    Some(d) => {
                        p.text(
                            c.left_center(),
                            Align2::LEFT_CENTER,
                            fmt_date(d),
                            FontId::proportional(13.0),
                            tt.text,
                        );
                    }
                    None => {
                        p.text(
                            c.left_center(),
                            Align2::LEFT_CENTER,
                            "\u{2013}",
                            FontId::proportional(13.0),
                            tt.text3,
                        );
                    }
                }
                let mut val = v;
                if let Some(true) = popup(
                    ui,
                    &dr,
                    ui.id().with(("ov_date_p", lid, which)),
                    300.0,
                    |ui| date_menu(ui, &tt, ui.id().with(("ov_cal", lid, which)), &mut val),
                ) {
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
            hover_bg(ui, &prr, c5.shrink(2.0), 4.0);
            name_it(&prr, &format!("Priority of {}", item.name));
            if item.priority > 0 {
                paint_flag(
                    &p,
                    c5.left_center() + vec2(4.0, 0.0),
                    priority_color(item.priority, &tt),
                    12.0,
                );
                p.text(
                    c5.left_center() + vec2(16.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr(PRIORITIES[item.priority as usize].label),
                    FontId::proportional(13.0),
                    tt.text,
                );
            } else {
                p.text(
                    c5.left_center(),
                    Align2::LEFT_CENTER,
                    "\u{2013}",
                    FontId::proportional(13.0),
                    tt.text3,
                );
            }
            let mut pv = item.priority;
            if let Some(true) = popup(ui, &prr, ui.id().with(("ov_pr_p", lid)), 180.0, |ui| {
                priority_menu(ui, &tt, &mut pv)
            }) {
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
                    if menu_row(ui, &tt, &m.name, item.owner == Some(m.id), |p, c| {
                        paint_avatar(p, c, 8.0, m, ring)
                    })
                    .clicked()
                    {
                        edited.owner = if item.owner == Some(m.id) {
                            None
                        } else {
                            Some(m.id)
                        };
                        close_popup(ui);
                    }
                }
            });
            let any_cell = [cr.hovered(), prr.hovered(), orr.hovered()]
                .iter()
                .any(|x| *x)
                || [c0, cell(r, 3), cell(r, 4), c5, c6]
                    .iter()
                    .any(|c| ui.rect_contains_pointer(*c));
            let changed = edited.color != item.color
                || edited.start != item.start
                || edited.end != item.end
                || edited.priority != item.priority
                || edited.owner != item.owner;
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
        if ui
            .add(ghost(egui::Button::new(
                RichText::new(tr("+ New List")).color(tt.text2),
            )))
            .clicked()
        {
            let space = self.current_space();
            let folder = if let Location::Folder(f) = self.loc {
                Some(f)
            } else {
                None
            };
            self.open_new_list(space, folder);
        }
    }

    fn ov_card(&mut self, ui: &mut Ui, w: f32, title: &str, add: impl FnOnce(&mut Self, &mut Ui)) {
        let t = self.t();
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::same(16))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_width(w - 34.0);
                    ui.set_min_height(150.0);
                    ui.label(RichText::new(tr(title)).size(15.0));

                    ui.add_space(8.0);
                    add(self, ui);
                });
            });
    }
}

/// A local date and "HH:MM" as a timestamp (bad times fall back to 9:00).
pub fn reminder_ts(d: NaiveDate, time: &str) -> i64 {
    let (h, m) = time
        .split_once(':')
        .map(|(h, m)| (h.trim().parse().unwrap_or(9), m.trim().parse().unwrap_or(0)))
        .unwrap_or((9, 0));
    d.and_hms_opt(u32::min(h, 23), u32::min(m, 59), 0)
        .and_then(|x| x.and_local_timezone(chrono::Local).single())
        .map(|x| x.timestamp())
        .unwrap_or_else(now_ts)
}

// Apps sweep: Home, Planner, Reminders, Notepad, Trash and the hubs.
impl App {
    /// Adds a dashboard owned by me and opens it.
    pub(super) fn new_dashboard(&mut self, name: &str) {
        let id = self.data.new_id();
        self.data.extra.dashboards.push(Dashboard {
            id,
            name: name.to_string(),
            owner: self.data.me,
            ..Default::default()
        });
        self.dash_open = Some(id);
        self.dirty = true;
    }

    /// Ticks a task done (its list's closed status) or opens it again.
    pub(super) fn toggle_done(&mut self, id: u64) {
        let Some(mut task) = self.task(id).cloned() else {
            return;
        };
        let sts = self.data.statuses_for_list(task.list_id);
        if self.data.is_closed(&task) {
            task.status = sts[0].name.clone();
        } else if let Some(done) = sts
            .iter()
            .find(|s| s.kind == StatusKind::Closed)
            .or_else(|| sts.iter().find(|s| s.kind == StatusKind::Done))
        {
            task.status = done.name.clone();
        }
        self.actions.push(Action::Replace(task));
    }

    /// "+ Add task" at the bottom of a Planner day: a task for me due that day.
    pub(super) fn planner_add_row(&mut self, ui: &mut Ui, d: NaiveDate, w: f32) {
        let t = self.t();
        let Some((_, text)) = self.sweep.planner_add.as_mut().filter(|p| p.0 == d) else {
            let b = ui.add(ghost(
                egui::Button::new(RichText::new(tr("+ Add task")).color(t.text3)).truncate(),
            ));
            name_it(&b, &format!("Add task on {}", d.format("%a %-d")));
            if b.clicked() {
                self.sweep.planner_add = Some((d, String::new()));
            }
            return;
        };
        let r = ui.add(
            field(text)
                .hint_text(tr("Task name"))
                .desired_width(w - 8.0),
        );
        if r.lost_focus() {
            let name = text.trim().to_string();
            self.sweep.planner_add = None;
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.is_empty() {
                let Some(list) = self.default_list() else {
                    // no List to put it in: say why and keep the name typed
                    self.sweep.planner_add = Some((d, name));
                    self.need_a_list();
                    return;
                };
                let me = self.data.me;
                self.actions.push(Action::Add {
                    list,
                    name,
                    status: None,
                    priority: 0,
                    assignees: vec![me],
                    due: Some(d),
                    parent: None,
                    open: false,
                });
            }
        } else if self.modal.is_none() {
            // (not over the dialog the next step opens)
            r.request_focus();
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
pub const SEARCH_KINDS: [&str; 8] = [
    "All", "Tasks", "Lists", "Docs", "Channels", "Messages", "Apps", "People",
];

impl App {
    /// Everything the search box finds, grouped, best first. A pasted
    /// workboard:// link (Copy link) or a task number finds that item.
    /// Archived tasks and Lists (Spaces, Folders) whose names match: search
    /// leaves them out, so it says how many. Kind as in SEARCH_KINDS.
    pub(super) fn archived_matches(&self, q: &str, kind: u8) -> usize {
        let ql = q.trim().to_lowercase();
        if ql.is_empty() || super::nav::parse_link(q).is_some() {
            return 0;
        }
        let mine = |sp: &Space| sp.private_owner.is_none_or(|o| o == self.data.me);
        let tasks = if kind == 0 || kind == 1 {
            self.data
                .tasks
                .iter()
                .filter(|t| !self.data.task_live(t))
                .filter(|t| self.data.space_of_list(t.list_id).is_some_and(mine))
                .filter(|t| t.name.to_lowercase().contains(&ql))
                .count()
        } else {
            0
        };
        let lists = if kind == 0 || kind == 2 {
            self.data
                .spaces
                .iter()
                .filter(|sp| mine(sp))
                .flat_map(|sp| {
                    sp.lists
                        .iter()
                        .chain(sp.folders.iter().flat_map(|f| f.lists.iter()))
                })
                .filter(|l| self.data.list_archived(l.id) && l.name.to_lowercase().contains(&ql))
                .count()
        } else {
            0
        };
        tasks + lists
    }

    pub(super) fn search_rows(&self, q: &str, kind: u8) -> Vec<SearchRow> {
        let ql = q.trim().to_lowercase();
        let show = |k: u8| kind == 0 || kind == k;
        let many = |k: u8| if kind == k { 60 } else { 8 };
        let mut rows: Vec<SearchRow> = vec![];
        // a pasted workboard:// link (from Copy link)
        if let Some(link) = super::nav::parse_link(q) {
            let hit = match link {
                super::nav::Link::Task(id) => self.task(id).map(|t| {
                    (
                        "task",
                        t.name.clone(),
                        self.data.list_name(t.list_id),
                        Hit::Task(id),
                    )
                }),
                super::nav::Link::Loc(l @ Location::List(id)) => {
                    self.data.location_exists(l).then(|| {
                        (
                            "list",
                            self.data.list_name(id),
                            self.data
                                .space_of_list(id)
                                .map(|s| s.name.clone())
                                .unwrap_or_default(),
                            Hit::Go(l),
                        )
                    })
                }
                super::nav::Link::Loc(l @ Location::Folder(_)) => {
                    self.data.location_exists(l).then(|| {
                        (
                            "folder",
                            self.data.location_name(l),
                            tr("Folder").into(),
                            Hit::Go(l),
                        )
                    })
                }
                super::nav::Link::Loc(l @ Location::Space(_)) => {
                    self.data.location_exists(l).then(|| {
                        (
                            "space",
                            self.data.location_name(l),
                            tr("Space").into(),
                            Hit::Go(l),
                        )
                    })
                }
                super::nav::Link::Loc(l) => Some((
                    "arrow_right",
                    self.data.location_name(l),
                    String::new(),
                    Hit::Go(l),
                )),
                super::nav::Link::Doc(id) => self
                    .data
                    .docs
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| ("docs", d.title.clone(), tr("Doc").into(), Hit::Doc(id))),
            };
            match hit {
                // archived, or in someone else's Personal space
                Some(_) if self.link_shown(link) == Some(false) => rows.push((
                    "Link",
                    "warning",
                    tr("This link points to something archived or private").into(),
                    String::new(),
                    Hit::Query(String::new()),
                )),
                Some((g, title, place, hit)) => rows.push(("Link", g, title, place, hit)),
                None => rows.push((
                    "Link",
                    "warning",
                    tr("Nothing with this link here (it may have been deleted)").into(),
                    String::new(),
                    Hit::Query(String::new()),
                )),
            }
            return rows;
        }
        if ql.is_empty() && kind == 0 {
            for (k, id, name, place) in self.recent_items(None).into_iter().take(8) {
                let (g, hit) = match k.as_str() {
                    "task" => ("task", Hit::Task(id)),
                    "doc" => ("docs", Hit::Doc(id)),
                    _ => ("list", Hit::Go(Location::List(id))),
                };
                rows.push(("Recent", g, name, place, hit));
            }
            for r in self.sweep.search_recent.iter().take(5) {
                rows.push((
                    "Recent searches",
                    "search",
                    r.clone(),
                    String::new(),
                    Hit::Query(r.clone()),
                ));
            }
        }
        let has = |s: &str| ql.is_empty() || s.to_lowercase().contains(&ql);
        let me = self.data.me;
        // like the sidebar: no other people's Personal spaces, nothing archived
        let space_shown = |s: &Space| !s.archived && s.private_owner.map_or(true, |o| o == me);
        if show(2) {
            let mut locs: Vec<SearchRow> = vec![];
            for s in self.data.spaces.iter().filter(|s| space_shown(s)) {
                locs.push((
                    "Locations",
                    "space",
                    s.name.clone(),
                    tr("Space").into(),
                    Hit::Go(Location::Space(s.id)),
                ));
                // folders and lists by their path, as in the sidebar
                for f in &s.folders {
                    locs.push((
                        "Locations",
                        "folder",
                        format!("{} / {}", s.name, f.name),
                        tr("Folder").into(),
                        Hit::Go(Location::Folder(f.id)),
                    ));
                    for l in f.lists.iter().filter(|l| !l.archived) {
                        locs.push((
                            "Locations",
                            "list",
                            format!("{} / {} / {}", s.name, f.name, l.name),
                            tr("List").into(),
                            Hit::Go(Location::List(l.id)),
                        ));
                    }
                }
                for l in s.lists.iter().filter(|l| !l.archived) {
                    locs.push((
                        "Locations",
                        "list",
                        format!("{} / {}", s.name, l.name),
                        tr("List").into(),
                        Hit::Go(Location::List(l.id)),
                    ));
                }
            }
            rows.extend(locs.into_iter().filter(|r| has(&r.2)).take(many(2)));
        }
        if show(1) {
            // not in archived lists, folders or spaces, nor other people's Personal spaces
            let tasks = self
                .data
                .tasks
                .iter()
                .filter(|x| self.task_visible(x) && (has(&x.name) || x.id.to_string() == ql))
                .take(if kind == 1 { 60 } else { 20 });
            rows.extend(tasks.map(|x| {
                (
                    "Tasks",
                    "task",
                    x.name.clone(),
                    self.data.list_name(x.list_id),
                    Hit::Task(x.id),
                )
            }));
        }
        if show(3) {
            let docs = self
                .data
                .docs
                .iter()
                .filter(|d| {
                    self.doc_shown(d)
                        && (has(&d.title) || (ql.len() > 2 && d.body.to_lowercase().contains(&ql)))
                })
                .take(many(3));
            rows.extend(docs.map(|d| {
                (
                    "Docs",
                    "docs",
                    d.title.clone(),
                    tr("Doc").into(),
                    Hit::Doc(d.id),
                )
            }));
        }
        if show(4) {
            let chans = self
                .data
                .channels
                .iter()
                .filter(|c| c.kind == ChannelKind::Channel && self.can_see(c) && has(&c.name))
                .take(many(4));
            rows.extend(chans.map(|c| {
                (
                    "Channels",
                    "#",
                    c.name.clone(),
                    tr("Channel").into(),
                    Hit::Chat(c.id),
                )
            }));
        }
        if show(5) && ql.len() > 1 {
            let mut n = 0;
            'outer: for c in self.data.channels.iter().filter(|c| self.can_see(c)) {
                let place = if c.kind == ChannelKind::Direct {
                    tr("Direct message").to_string()
                } else {
                    format!("#{}", c.name)
                };
                for m in c
                    .messages
                    .iter()
                    .chain(c.messages.iter().flat_map(|m| m.replies.iter()))
                {
                    if m.text.to_lowercase().contains(&ql) {
                        rows.push((
                            "Messages",
                            "chat",
                            truncate(&super::chat::shown_text(m), 60),
                            place.clone(),
                            Hit::Chat(c.id),
                        ));
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
            apps.extend(x.whiteboards.iter().filter(|w| has(&w.name)).map(|w| {
                (
                    "Apps",
                    "whiteboards",
                    w.name.clone(),
                    tr("Whiteboard").into(),
                    Hit::Whiteboard(w.id),
                )
            }));
            apps.extend(
                x.forms
                    .iter()
                    .filter(|f| self.form_shown(f) && has(&f.name))
                    .map(|f| {
                        (
                            "Apps",
                            "forms",
                            f.name.clone(),
                            tr("Form").into(),
                            Hit::Form(f.id),
                        )
                    }),
            );
            apps.extend(
                x.dashboards
                    .iter()
                    .filter(|d| self.dash_shown(d) && has(&d.name))
                    .map(|d| {
                        (
                            "Apps",
                            "dashboards",
                            d.name.clone(),
                            tr("Dashboard").into(),
                            Hit::Dashboard(d.id),
                        )
                    }),
            );
            apps.extend(x.goals.iter().filter(|g| has(&g.name)).map(|g| {
                (
                    "Apps",
                    "goals",
                    g.name.clone(),
                    tr("Goal").into(),
                    Hit::Goal(g.id),
                )
            }));
            if !(ql.is_empty() && kind == 0) {
                rows.extend(apps.into_iter().take(many(6)));
            }
        }
        if show(7) && !(ql.is_empty() && kind == 0) {
            let people = self
                .data
                .members
                .iter()
                .filter(|m| {
                    has(&m.name) || (!ql.is_empty() && m.email.to_lowercase().contains(&ql))
                })
                .take(many(7));
            rows.extend(people.map(|m| {
                (
                    "People",
                    "person",
                    m.name.clone(),
                    if m.email.is_empty() {
                        super::menus::role_text(&m.role).to_string()
                    } else {
                        m.email.clone()
                    },
                    Hit::Person(m.id),
                )
            }));
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
        let Some(mut q) = self.palette.take() else {
            return;
        };
        let t = self.t();
        let mut open = true;
        egui::Window::new("search_palette")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 60.0])
            .fixed_size([620.0, 420.0])
            .show(ctx, |ui| {
                // the arrows move the highlight, not the text cursor
                let (down, up) = ui.input_mut(|i| {
                    (
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    )
                });
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let r = ui.add(
                    field(&mut q)
                        .hint_text(tr("Search, or paste a link to a task, list or space..."))
                        .font(FontId::proportional(16.0))
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                let kid = egui::Id::new("palette_kind");
                let mut kind: u8 = ui.data(|d| d.get_temp(kid)).unwrap_or(0);
                ui.horizontal(|ui| {
                    for (i, label) in SEARCH_KINDS.iter().enumerate() {
                        if ui.selectable_label(kind == i as u8, tr(label)).clicked() {
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
                let pickable: Vec<usize> = (0..rows.len())
                    .filter(|i| rows[*i].4 != Hit::Query(String::new()))
                    .collect();
                if !pickable.is_empty() {
                    let pos = pickable
                        .iter()
                        .position(|i| *i == self.sweep.search_sel)
                        .unwrap_or(0);
                    let pos = if down {
                        (pos + 1).min(pickable.len() - 1)
                    } else if up {
                        pos.saturating_sub(1)
                    } else {
                        pos
                    };
                    self.sweep.search_sel = pickable[pos];
                }
                let mut chosen: Option<Hit> = None;
                egui::ScrollArea::vertical()
                    .max_height(340.0)
                    .show(ui, |ui| {
                        let mut group = "";
                        for (i, (g, glyph, title, place, hit)) in rows.iter().enumerate() {
                            if *g != group {
                                if !group.is_empty() {
                                    ui.add_space(4.0);
                                }
                                group = g;
                                ui.label(RichText::new(tr(g)).small().color(t.text3));
                            }
                            let w = ui.available_width();
                            let (rr, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
                            name_it(&resp, title);
                            let sel = i == self.sweep.search_sel && pickable.contains(&i);
                            if sel || resp.hovered() {
                                ui.painter().rect_filled(
                                    rr,
                                    6.0,
                                    if sel { t.row_selected } else { t.hover },
                                );
                            }
                            if sel && (down || up) {
                                resp.scroll_to_me(None);
                            }
                            // a Space shows its colour and first letter as in the
                            // sidebar; everything else its kind's line icon
                            let ic = rr.left_center() + vec2(14.0, 0.0);
                            let space = match hit {
                                Hit::Go(Location::Space(id)) => {
                                    self.data.spaces.iter().find(|s| s.id == *id)
                                }
                                _ => None,
                            };
                            if let Some(sp) = space {
                                let r = Rect::from_center_size(ic, vec2(16.0, 16.0));
                                ui.painter().rect_filled(r, 4.0, rgb(sp.color));
                                ui.painter().text(
                                    r.center(),
                                    Align2::CENTER_CENTER,
                                    sp.name
                                        .chars()
                                        .next()
                                        .unwrap_or('S')
                                        .to_uppercase()
                                        .to_string(),
                                    FontId::proportional(10.0),
                                    Color32::WHITE,
                                );
                            } else {
                                line_icon(ui.painter(), glyph, ic, t.text2);
                            }
                            // the title keeps all but 200 for the grey place after
                            // it; both are shortened with an ellipsis
                            let title_r = paint_fit(
                                ui,
                                pos2(rr.left() + 28.0, rr.center().y),
                                Align2::LEFT_CENTER,
                                title,
                                FontId::proportional(14.0),
                                t.text,
                                w - 200.0,
                            );
                            paint_fit(
                                ui,
                                pos2(title_r.right() + 12.0, rr.center().y),
                                Align2::LEFT_CENTER,
                                place,
                                FontId::proportional(12.0),
                                t.text3,
                                rr.right() - 8.0 - (title_r.right() + 12.0),
                            );
                            if resp.clicked() {
                                chosen = Some(hit.clone());
                            }
                        }
                        if rows.is_empty() && !q.trim().is_empty() {
                            ui.label(RichText::new(tr("No results")).color(t.text3));
                        }
                        // archived ones are left out: say so rather than drop them silently
                        let arch = self.archived_matches(&q, kind);
                        if arch > 0 {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(if arch == 1 {
                                    tr("1 archived match is not shown").to_string()
                                } else {
                                    trf!("{} archived matches are not shown", arch)
                                })
                                .small()
                                .color(t.text3),
                            );
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
                        if !typed.is_empty() && super::nav::parse_link(&typed).is_none() {
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

impl App {
    /// The "Delete this?" dialog for `Modal::ConfirmRemove`; true closes it.
    pub(super) fn confirm_remove_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        kind: &str,
        id: u64,
        name: &str,
    ) -> bool {
        let mut close = false;
        let text = match kind {
            "dashboard" => trf!("Delete the dashboard \"{}\"? It goes to Trash.", name),
            "whiteboard" => trf!("Delete the whiteboard \"{}\"? It goes to Trash.", name),
            "form" => trf!("Delete the form \"{}\"? It goes to Trash.", name),
            "goal" => trf!("Delete the goal \"{}\"? It goes to Trash.", name),
            "meeting" => trf!("Delete the meeting \"{}\"? This can't be undone.", name),
            "team" => trf!("Delete the team \"{}\"? It goes to Trash.", name),
            "invite" => trf!(
                "Cancel the invite for {}? Their invite code stops working and they leave the list.",
                name
            ),
            _ => trf!(
                "Remove {} from the Workspace? Their tasks and conversations stay, without them.",
                name
            ),
        };
        ui.label(text);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let label = if kind == "person" {
                tr("Remove")
            } else if kind == "invite" {
                tr("Cancel invite")
            } else {
                tr("Delete")
            };
            if ui
                .add(Tint(
                    egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(t.danger),
                ))
                .clicked()
            {
                self.remove_item(kind, id);
                close = true;
            }
            if ui.button(tr("Cancel")).clicked() {
                close = true;
            }
        });
        close
    }

    /// Deletes a dashboard, whiteboard, form, goal, meeting or team, or
    /// removes a person, once confirmed. All but meetings and people go to
    /// Trash, where Restore brings them back.
    pub(super) fn remove_item(&mut self, kind: &str, id: u64) {
        let x = &self.data.extra;
        let trashed = match kind {
            "dashboard" => x
                .dashboards
                .iter()
                .find(|d| d.id == id)
                .map(|d| ("Dashboard", d.name.clone(), serde_json::to_string(d))),
            "whiteboard" => x
                .whiteboards
                .iter()
                .find(|w| w.id == id)
                .map(|w| ("Whiteboard", w.name.clone(), serde_json::to_string(w))),
            "form" => x
                .forms
                .iter()
                .find(|f| f.id == id)
                .map(|f| ("Form", f.name.clone(), serde_json::to_string(f))),
            "goal" => x
                .goals
                .iter()
                .find(|g| g.id == id)
                .map(|g| ("Goal", g.name.clone(), serde_json::to_string(g))),
            "team" => x
                .teams
                .iter()
                .find(|tm| tm.id == id)
                .map(|tm| ("Team", tm.name.clone(), serde_json::to_string(tm))),
            _ => None,
        };
        if let Some((k, title, Ok(payload))) = trashed {
            let tid = self.data.new_id();
            self.data.extra.trash.push(TrashItem {
                id: tid,
                kind: k.into(),
                title,
                deleted: now_ts(),
                payload,
            });
        }
        if kind == "whiteboard" {
            self.sweep.wb_undo.retain(|u| u.0 != id);
        }
        let x = &mut self.data.extra;
        match kind {
            "dashboard" => {
                x.dashboards.retain(|d| d.id != id);
                x.starred.retain(|s| s != &format!("dash:{}", id));
                if self.dash_open == Some(id) {
                    self.dash_open = None;
                }
            }
            "whiteboard" => {
                x.whiteboards.retain(|w| w.id != id);
                x.starred.retain(|s| s != &format!("wb:{}", id));
                if self.wb_open == Some(id) {
                    self.wb_open = None;
                }
            }
            "form" => {
                x.forms.retain(|f| f.id != id);
                x.starred.retain(|s| s != &format!("form:{}", id));
                if self.form_open == Some(id) {
                    self.form_open = None;
                }
            }
            "goal" => {
                x.goals.retain(|g| g.id != id);
                if self.goal_open == Some(id) {
                    self.goal_open = None;
                }
            }
            "meeting" => {
                x.meetings.retain(|m| m.id != id);
                if self.meeting_open == Some(id) {
                    self.meeting_open = None;
                }
            }
            "team" => {
                x.teams.retain(|tm| tm.id != id);
                if self.team_open == Some(id) {
                    self.team_open = None;
                    self.hub_filter = 0;
                }
            }
            // cancelling an invite takes the not-yet-joined person out the
            // same way, and the server drops their code
            "person" | "invite" => {
                let server = self
                    .data
                    .extra
                    .shared
                    .as_ref()
                    .is_some_and(|c| c.server.is_some());
                if server {
                    // their computers are signed out by the server; without it,
                    // nothing changes here either
                    if self.sync_rt.net_status != Some(wb_net::Status::Connected) {
                        self.access_request(wb_net::Msg::Revoke { member: id });
                        return;
                    }
                    self.access_request(wb_net::Msg::Revoke { member: id });
                }
                self.data.members.retain(|m| m.id != id);
                for task in &mut self.data.tasks {
                    task.assignees.retain(|a| *a != id);
                }
                for c in &mut self.data.channels {
                    c.members.retain(|a| *a != id);
                }
                for tm in &mut self.data.extra.teams {
                    tm.members.retain(|a| *a != id);
                }
                for m in &mut self.data.extra.meetings {
                    m.attendees.retain(|a| *a != id);
                }
            }
            _ => return,
        }
        self.dirty = true;
    }
}
