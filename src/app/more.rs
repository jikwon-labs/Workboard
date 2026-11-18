// Pages reached from the sidebar, the rail and the menus: People, Meetings,
// Reminders, AI, dashboards and removing items. Posts, Inbox, Goals, Forms,
// Whiteboards, Search and the Overview view have their own files in more/.

use super::icons::{icon_button, icon_button_ex, line_icon, line_icon_scaled};
use super::menus::ROLES;
use super::*;

mod forms;
mod goals;
mod inbox;
mod overview;
mod posts;
mod search;
mod whiteboard;

// for tests that name them `more::form_answers` and `more::Hit`
#[cfg(test)]
pub use forms::form_answers;
#[cfg(test)]
pub use search::Hit;
pub use whiteboard::{WbUndo, WbView};

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
                let server = self.on_server();
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
                ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0_f32, t.border));
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
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5_f32, t.border));
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
                    let (st, col) = if m.invited { ("Invited", WARN_ORANGE) } else { ("Active", Color32::from_rgb(41, 151, 100)) };
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

    // ---------------------------------------------------------------- meetings

    pub(super) fn meetings_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::SidePanel::left("meet_list")
            .exact_width(260.0)
            .resizable(false)
            .frame(side_panel_frame(&t, egui::Margin::symmetric(10, 12)))
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
                    self.toast(trf!("Tasks created from the meeting notes in {}", self.data.list_name(list)));
                } else {
                    self.toast(tr("Make a List first: action items become tasks in a List"));
                }
            }
            if delete {
                let m = &self.data.extra.meetings[idx];
                self.modal = Some(Modal::ConfirmRemove { kind: "meeting", id: m.id, name: m.title.clone() });
            }
        });
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
                let when = local(r.due_ts)
                    .map(|d| d.text("%b %-d, %-I:%M %p"))
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
                    let local = local(r.due_ts);
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
                        let when = local(r.due_ts)
                            .map(|d| d.text("%b %-d, %-I:%M %p"))
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
                let server = self.on_server();
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
