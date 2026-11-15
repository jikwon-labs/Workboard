// Features that bring the app closer to the original: create dialogs for
// lists and spaces, moving / duplicating / archiving tasks and lists, the
// full task menu, selecting several tasks at once, condition filters,
// choosing columns, recurring tasks and single-key shortcuts.

use super::*;
use egui::Pos2;

/// Space icons offered in Color & Icon ("" = the first letter of the name).
pub const SPACE_ICONS: [&str; 12] = ["", "\u{2605}", "\u{25C6}", "\u{25CF}", "\u{25B2}", "\u{2665}", "\u{2691}", "\u{2714}", "\u{2600}", "\u{2699}", "\u{263A}", "\u{25A0}"];

pub const DUE_FILTERS: [&str; 6] = ["Any time", "Overdue", "Today", "This week", "Next week", "No due date"];
pub const COLUMNS: [&str; 6] = ["Assignee", "Due date", "Priority", "Status", "Comments", "Date created"];
/// Properties that can be shown as extra columns (hidden at first), drawn
/// from values every task already has.
pub const PROPERTIES: [&str; 10] = ["Created by", "Date closed", "Date done", "Date updated", "Start date", "Time estimate", "Time tracked", "Tags", "Latest comment", "Subtasks"];

/// Width of an extra property column.
pub fn prop_width(name: &str) -> f32 {
    match name {
        "Latest comment" => 180.0,
        "Tags" => 140.0,
        "Created by" => 120.0,
        _ => 105.0,
    }
}

/// A field a filter row can test.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum FilterField {
    Status,
    Priority,
    Assignee,
    DueDate,
    Tags,
    StartDate,
    Created,
    Updated,
    CreatedBy,
    HasSubtasks,
}

/// Choices for the start date filter (index = stored value).
pub const START_FILTERS: [&str; 6] = ["Any time", "Started", "Today", "This week", "Next week", "No start date"];
/// Choices for the date created / updated filters (index = stored value).
pub const AGE_FILTERS: [&str; 5] = ["Any time", "Today", "Yesterday", "Last 7 days", "Last 30 days"];

impl FilterField {
    pub const ALL: [FilterField; 10] = [FilterField::Status, FilterField::Priority, FilterField::Assignee, FilterField::DueDate, FilterField::Tags, FilterField::StartDate, FilterField::Created, FilterField::Updated, FilterField::CreatedBy, FilterField::HasSubtasks];

    pub fn label(self) -> &'static str {
        match self {
            FilterField::Status => "Status",
            FilterField::Priority => "Priority",
            FilterField::Assignee => "Assignee",
            FilterField::DueDate => "Due date",
            FilterField::Tags => "Tags",
            FilterField::StartDate => "Start date",
            FilterField::Created => "Date created",
            FilterField::Updated => "Date updated",
            FilterField::CreatedBy => "Created by",
            FilterField::HasSubtasks => "Subtasks",
        }
    }

    /// Fields whose values exclude each other (one pick, not several).
    pub fn single(self) -> bool {
        matches!(self, FilterField::DueDate | FilterField::StartDate | FilterField::Created | FilterField::Updated | FilterField::HasSubtasks)
    }
}

#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct FilterRow {
    pub field: Option<FilterField>,
    pub not: bool,
    pub values: Vec<String>,
}

impl FilterRow {
    pub fn active(&self) -> bool {
        self.field.is_some() && !self.values.is_empty()
    }

    pub fn matches(&self, t: &Task, data: &Data) -> bool {
        let Some(f) = self.field else { return true };
        if self.values.is_empty() {
            return true;
        }
        let hit = match f {
            FilterField::Status => self.values.iter().any(|v| *v == data.status_of(t).name),
            FilterField::Priority => self.values.iter().any(|v| v.parse::<u8>().ok() == Some(t.priority)),
            FilterField::Assignee => self.values.iter().any(|v| if v == "none" { t.assignees.is_empty() } else { v.parse::<u64>().is_ok_and(|id| t.assignees.contains(&id)) }),
            FilterField::DueDate => self.values.iter().any(|v| due_matches(v.parse().unwrap_or(0), t, data)),
            FilterField::Tags => self.values.iter().any(|v| t.tags.contains(v)),
            FilterField::StartDate => self.values.iter().any(|v| start_matches(v.parse().unwrap_or(0), t.start)),
            FilterField::Created => self.values.iter().any(|v| age_matches(v.parse().unwrap_or(0), t.created)),
            FilterField::Updated => self.values.iter().any(|v| age_matches(v.parse().unwrap_or(0), t.updated)),
            // the creator is whoever wrote the first activity line ("created this task")
            FilterField::CreatedBy => self.values.iter().any(|v| v.parse::<u64>().ok() == t.activity.first().map(|a| a.author)),
            FilterField::HasSubtasks => {
                let has = data.tasks.iter().any(|x| x.parent == Some(t.id) && !x.archived);
                self.values.iter().any(|v| (v == "yes") == has)
            }
        };
        hit != self.not
    }
}

fn start_matches(which: u8, start: Option<NaiveDate>) -> bool {
    let td = today();
    let week_start = td - Duration::days(td.weekday().num_days_from_monday() as i64);
    match which {
        1 => start.is_some_and(|d| d <= td),
        2 => start == Some(td),
        3 => start.is_some_and(|d| d >= week_start && d < week_start + Duration::days(7)),
        4 => start.is_some_and(|d| d >= week_start + Duration::days(7) && d < week_start + Duration::days(14)),
        5 => start.is_none(),
        _ => true,
    }
}

fn age_matches(which: u8, ts: i64) -> bool {
    let day = chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&chrono::Local).date_naive()).unwrap_or_else(today);
    let age = (today() - day).num_days();
    match which {
        1 => age == 0,
        2 => age == 1,
        3 => age < 7,
        4 => age < 30,
        _ => true,
    }
}

fn due_matches(due: u8, t: &Task, data: &Data) -> bool {
    let td = today();
    // weeks start on Sunday as in the calendar, or Monday with the Labs option
    let monday = data.extra.labs.iter().any(|l| l == "week_monday");
    let back = if monday { td.weekday().num_days_from_monday() } else { td.weekday().num_days_from_sunday() };
    let week_start = td - Duration::days(back as i64);
    match due {
        1 => t.due.map_or(false, |d| d < td) && !data.is_closed(t),
        2 => t.due == Some(td),
        3 => t.due.map_or(false, |d| d >= week_start && d < week_start + Duration::days(7)),
        4 => t.due.map_or(false, |d| d >= week_start + Duration::days(7) && d < week_start + Duration::days(14)),
        5 => t.due.is_none(),
        _ => true,
    }
}

/// Field conditions from the Filter panel (all must match): the rows of the
/// condition builder, plus fixed fields kept for code that sets them directly.
#[derive(Clone, Default, PartialEq)]
pub struct TaskFilter {
    pub rows: Vec<FilterRow>,
    pub statuses: Vec<String>,
    pub priorities: Vec<u8>,
    pub assignees: Vec<u64>,
    pub unassigned: bool,
    pub tags: Vec<String>,
    pub due: u8,
    pub archived: bool,
}

impl TaskFilter {
    pub fn count(&self) -> usize {
        self.rows.iter().filter(|r| r.active()).count() + [!self.statuses.is_empty(), !self.priorities.is_empty(), !self.assignees.is_empty() || self.unassigned, !self.tags.is_empty(), self.due != 0, self.archived].iter().filter(|x| **x).count()
    }

    pub fn matches(&self, t: &Task, data: &Data) -> bool {
        if t.archived != self.archived {
            return false;
        }
        if !self.rows.iter().all(|r| r.matches(t, data)) {
            return false;
        }
        if !self.statuses.is_empty() && !self.statuses.iter().any(|s| *s == data.status_of(t).name) {
            return false;
        }
        if !self.priorities.is_empty() && !self.priorities.contains(&t.priority) {
            return false;
        }
        if (!self.assignees.is_empty() || self.unassigned) && !(self.assignees.iter().any(|a| t.assignees.contains(a)) || (self.unassigned && t.assignees.is_empty())) {
            return false;
        }
        if !self.tags.is_empty() && !self.tags.iter().any(|g| t.tags.contains(g)) {
            return false;
        }
        due_matches(self.due, t, data)
    }
}

/// Closes whichever menu (popup or context menu) is showing.
pub fn close_any(ui: &mut Ui) {
    close_popup(ui);
    ui.close_menu();
}

impl App {
    // ---------------------------------------------------------------- create

    /// Spaces a new list can go in (not the hidden personal space).
    pub(super) fn open_spaces(&self) -> Vec<&Space> {
        self.data.spaces.iter().filter(|s| s.private_owner.is_none() && !s.archived).collect()
    }

    pub(super) fn open_new_list(&mut self, space: Option<u64>, folder: Option<u64>) {
        let visible = |id: u64| self.data.spaces.iter().any(|s| s.id == id && s.private_owner.is_none());
        let space = space.filter(|s| visible(*s)).or_else(|| self.open_spaces().first().map(|s| s.id)).unwrap_or(0);
        self.modal = Some(Modal::NewList { name: String::new(), space, folder, private: false });
    }

    pub(super) fn create_list(&mut self, space: u64, folder: Option<u64>, name: &str) -> Option<u64> {
        let name = if name.trim().is_empty() { "List".to_string() } else { name.trim().to_string() };
        let id = self.data.new_id();
        let s = self.data.spaces.iter_mut().find(|s| s.id == space)?;
        s.expanded = true;
        let item = ListItem { id, name, ..Default::default() };
        match folder.and_then(|fid| s.folders.iter_mut().find(|f| f.id == fid)) {
            Some(f) => {
                f.expanded = true;
                f.lists.push(item);
            }
            None => s.lists.push(item),
        }
        self.section = Section::Home;
        self.loc = Location::List(id);
        self.view = ViewKind::List;
        self.dirty = true;
        Some(id)
    }

    pub(super) fn create_space(&mut self, name: &str, color: [u8; 3]) -> u64 {
        let id = self.data.new_id();
        let lid = self.data.new_id();
        let name = if name.trim().is_empty() { "Space".to_string() } else { name.trim().to_string() };
        self.data.spaces.push(Space { id, name, color, statuses: default_statuses(), folders: vec![], lists: vec![ListItem { id: lid, name: "List".into(), ..Default::default() }], expanded: true, private_owner: None, ..Default::default() });
        self.section = Section::Home;
        self.loc = Location::Space(id);
        self.view = ViewKind::Overview;
        self.dirty = true;
        id
    }

    pub(super) fn new_list_body(&mut self, ui: &mut Ui, t: &Theme, name: &mut String, space: &mut u64, folder: &mut Option<u64>) -> bool {
        let mut close = false;
        ui.label(RichText::new(tr("All Lists are located within a Space. Lists can house any type of task.")).color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Name")).color(t.text2));
        let r = ui.add(egui::TextEdit::singleline(name).hint_text(tr("e.g. Project, List of items, Campaign")).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
        focus_once(ui, &r);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Space (location)")).color(t.text2));
        let spaces: Vec<(u64, String, Vec<(u64, String)>)> = self.open_spaces().iter().map(|s| (s.id, s.name.clone(), s.folders.iter().map(|f| (f.id, f.name.clone())).collect())).collect();
        let cur = spaces.iter().find(|s| s.0 == *space).map(|s| match folder.and_then(|f| s.2.iter().find(|x| x.0 == f)) {
            Some(f) => format!("{} / {}", s.1, f.1),
            None => s.1.clone(),
        });
        egui::ComboBox::from_id_salt("new_list_space").width(ui.available_width() - 10.0).selected_text(cur.unwrap_or_else(|| tr("Choose a Space").into())).show_ui(ui, |ui| {
            for (sid, sname, folders) in &spaces {
                if ui.selectable_label(*space == *sid && folder.is_none(), sname).clicked() {
                    *space = *sid;
                    *folder = None;
                }
                for (fid, fname) in folders {
                    if ui.selectable_label(*folder == Some(*fid), format!("    {} / {}", sname, fname)).clicked() {
                        *space = *sid;
                        *folder = Some(*fid);
                    }
                }
            }
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ok = spaces.iter().any(|s| s.0 == *space);
                if (ui.add_enabled(ok, egui::Button::new(RichText::new(tr("Create")).color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(90.0, 32.0))).clicked() || (enter && ok)) && self.create_list(*space, *folder, name).is_some() {
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    pub(super) fn new_space_body(&mut self, ui: &mut Ui, t: &Theme, name: &mut String, color: &mut [u8; 3]) -> bool {
        let mut close = false;
        ui.label(RichText::new(tr("A Space represents teams, departments, or groups, each with its own Lists, workflows, and settings.")).color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Icon & name")).color(t.text2));
        let mut enter = false;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
            ui.painter().rect_filled(r, 7.0, rgb(*color));
            let letter = name.chars().next().unwrap_or('S').to_uppercase().to_string();
            ui.painter().text(r.center(), Align2::CENTER_CENTER, letter, FontId::proportional(15.0), Color32::WHITE);
            let te = ui.add(egui::TextEdit::singleline(name).hint_text(tr("e.g. Marketing, Engineering, HR")).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
            focus_once(ui, &te);
            enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Color")).color(t.text2));
        ui.horizontal_wrapped(|ui| {
            for c in STATUS_PALETTE.iter().skip(1) {
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                ui.painter().circle_filled(r.center(), 9.0, rgb(*c));
                if *color == *c {
                    ui.painter().circle_stroke(r.center(), 11.0, Stroke::new(1.5, t.text));
                }
                if resp.clicked() {
                    *color = *c;
                }
            }
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, t, tr("Continue")).clicked() || enter {
                    self.create_space(name, *color);
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// "Create Channel" (+ Create > Channel, the sidebar's Channels "+").
    pub(super) fn new_channel_body(&mut self, ui: &mut Ui, t: &Theme, name: &mut String, private: &mut bool, add_list: &mut bool) -> bool {
        let mut close = false;
        ui.label(RichText::new(tr("Channels are where your team communicates. They're best when organized around a topic, like #marketing.")).color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Name")).color(t.text2));
        let r = ui.add(egui::TextEdit::singleline(name).hint_text(tr("e.g. design-team")).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
        name_field(&r, "Channel name");
        focus_once(ui, &r);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(10.0);
        for (on, label, help) in [(&mut *private, "Make Private", "Only invited people can find this channel."), (&mut *add_list, "Add a List", "Make a List with the same name, tied to this channel.")] {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr(label)).color(t.text));
                    ui.label(RichText::new(tr(help)).small().color(t.text3));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    switch(ui, t, on, label);
                });
            });
            ui.add_space(6.0);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ok = !name.trim().is_empty();
                let b = ui.add_enabled(ok, egui::Button::new(RichText::new(tr("Create")).color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(90.0, 32.0)));
                if (b.clicked() || (enter && ok)) && ok {
                    self.create_channel(name, *private, *add_list);
                    close = true;
                }
            });
        });
        close
    }

    pub(super) fn create_channel(&mut self, name: &str, private: bool, add_list: bool) -> u64 {
        let id = self.data.new_id();
        let clean = name.trim().to_lowercase().replace(' ', "-");
        let me = self.data.me;
        let members = if private { vec![me] } else { self.data.members.iter().map(|m| m.id).collect() };
        let list = if add_list {
            let space = self.current_space().filter(|s| self.open_spaces().iter().any(|x| x.id == *s)).or_else(|| self.open_spaces().first().map(|s| s.id));
            space.and_then(|s| self.create_list(s, None, name.trim()))
        } else {
            None
        };
        self.data.channels.push(Channel { id, name: clean, kind: ChannelKind::Channel, topic: String::new(), members, messages: vec![], last_read: now_ts(), private, list, creator: me });
        self.section = Section::Chat;
        self.chat_open = Some(id);
        self.thread_open = None;
        self.dirty = true;
        id
    }

    // ---------------------------------------------------------------- move / duplicate

    /// Moves tasks (and their subtasks) to another list; statuses that list
    /// does not have fall back to its first status.
    pub(super) fn move_tasks(&mut self, ids: &[u64], list: u64) {
        if self.data.space_of_list(list).is_none() {
            return;
        }
        let statuses = self.data.statuses_for_list(list);
        let lname = self.data.list_name(list);
        let me = self.data.me;
        let mut all: Vec<u64> = ids.to_vec();
        let mut i = 0;
        while i < all.len() {
            let k = all[i];
            all.extend(self.data.tasks.iter().filter(|t| t.parent == Some(k) && !all.contains(&t.id)).map(|t| t.id).collect::<Vec<_>>());
            i += 1;
        }
        for t in self.data.tasks.iter_mut().filter(|t| all.contains(&t.id)) {
            if t.list_id == list {
                continue;
            }
            t.list_id = list;
            if !statuses.iter().any(|s| s.name == t.status) {
                t.status = statuses[0].name.clone();
            }
            if ids.contains(&t.id) {
                t.activity.push(Activity { ts: now_ts(), author: me, text: format!("moved this task to {}", lname), comment: false, ..Default::default() });
                // a moved subtask becomes a task of its own in the new list
                if t.parent.map_or(false, |p| !all.contains(&p)) {
                    t.parent = None;
                }
            }
            t.updated = now_ts();
        }
        self.dirty = true;
        self.toast = Some((if ids.len() == 1 { trf!("Moved to {}", lname) } else { trf!("Moved {} tasks to {}", ids.len(), lname) }, now_ts()));
    }

    /// Copies a task with its subtasks; returns the copy's id.
    pub(super) fn duplicate_task(&mut self, id: u64) -> Option<u64> {
        fn copy(app: &mut App, id: u64, parent: Option<u64>, top: bool) -> Option<u64> {
            let src = app.task(id)?.clone();
            let nid = app.data.new_id();
            let now = now_ts();
            let mut t = src.clone();
            t.id = nid;
            t.parent = parent;
            if top {
                t.name = format!("{} (copy)", src.name);
            }
            t.created = now;
            t.updated = now;
            t.timer_started = None;
            t.timer_by = None;
            t.timers.clear();
            t.time_entries.clear();
            t.activity = vec![Activity { ts: now, author: app.data.me, text: format!("duplicated this task from {}", src.name), comment: false, ..Default::default() }];
            t.order = src.order;
            app.data.tasks.push(t);
            let subs: Vec<u64> = app.data.tasks.iter().filter(|x| x.parent == Some(id)).map(|x| x.id).collect();
            for s in subs {
                copy(app, s, Some(nid), false);
            }
            Some(nid)
        }
        let nid = copy(self, id, self.task(id)?.parent, true);
        self.dirty = true;
        nid
    }

    pub(super) fn set_archived(&mut self, ids: &[u64], archived: bool) {
        let me = self.data.me;
        for t in self.data.tasks.iter_mut().filter(|t| ids.contains(&t.id)) {
            t.archived = archived;
            t.activity.push(Activity { ts: now_ts(), author: me, text: if archived { "archived this task".into() } else { "unarchived this task".into() }, comment: false, ..Default::default() });
        }
        if archived && self.open_task.map_or(false, |o| ids.contains(&o)) {
            self.open_task = None;
        }
        self.selected.retain(|s| !ids.contains(s));
        self.dirty = true;
        self.toast = Some((if !archived { tr("Unarchived").into() } else if ids.len() == 1 { trf!("Archived {} task", ids.len()) } else { trf!("Archived {} tasks", ids.len()) }, now_ts()));
    }

    /// Moves a list to another space or folder.
    pub(super) fn move_list(&mut self, list: u64, space: u64, folder: Option<u64>) {
        let Some(item) = self.data.spaces.iter().flat_map(|s| s.lists.iter().chain(s.folders.iter().flat_map(|f| f.lists.iter()))).find(|l| l.id == list).cloned() else { return };
        if !self.data.spaces.iter().any(|s| s.id == space) {
            return;
        }
        for s in &mut self.data.spaces {
            s.lists.retain(|l| l.id != list);
            for f in &mut s.folders {
                f.lists.retain(|l| l.id != list);
            }
        }
        let statuses = self.data.spaces.iter().find(|s| s.id == space).map(|s| s.statuses.clone()).unwrap_or_else(default_statuses);
        if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == space) {
            s.expanded = true;
            match folder.and_then(|fid| s.folders.iter_mut().find(|f| f.id == fid)) {
                Some(f) => {
                    f.expanded = true;
                    f.lists.push(item.clone());
                }
                None => s.lists.push(item.clone()),
            }
        }
        for t in self.data.tasks.iter_mut().filter(|t| t.list_id == list) {
            if !statuses.iter().any(|s| s.name == t.status) {
                t.status = statuses[0].name.clone();
            }
        }
        self.toast = Some((trf!("Moved {} to {}", item.name, self.data.location_name(folder.map(Location::Folder).unwrap_or(Location::Space(space)))), now_ts()));
        self.dirty = true;
    }

    /// Copies a list and its tasks next to the original.
    pub(super) fn duplicate_list(&mut self, list: u64) -> Option<u64> {
        let name = format!("{} (copy)", self.data.list_name(list));
        let nid = self.data.new_id();
        // the copy keeps the list's details (colour, dates, priority, owner, description)
        let src = self.data.list_item(list).cloned().unwrap_or_default();
        let mut placed = false;
        for s in &mut self.data.spaces {
            if let Some(i) = s.lists.iter().position(|l| l.id == list) {
                s.lists.insert(i + 1, ListItem { id: nid, name: name.clone(), archived: false, ..src.clone() });
                placed = true;
            }
            for f in &mut s.folders {
                if let Some(i) = f.lists.iter().position(|l| l.id == list) {
                    f.lists.insert(i + 1, ListItem { id: nid, name: name.clone(), archived: false, ..src.clone() });
                    placed = true;
                }
            }
        }
        if !placed {
            return None;
        }
        let tops: Vec<u64> = self.data.tasks.iter().filter(|t| t.list_id == list && t.parent.is_none() && !t.archived).map(|t| t.id).collect();
        for id in tops {
            if let Some(c) = self.duplicate_task(id) {
                let name = self.task(id).map(|t| t.name.clone()).unwrap_or_default();
                let mut kids = vec![c];
                let mut i = 0;
                while i < kids.len() {
                    let k = kids[i];
                    kids.extend(self.data.tasks.iter().filter(|t| t.parent == Some(k)).map(|t| t.id).collect::<Vec<_>>());
                    i += 1;
                }
                for t in self.data.tasks.iter_mut().filter(|t| kids.contains(&t.id)) {
                    t.list_id = nid;
                }
                if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == c) {
                    t.name = name;
                }
            }
        }
        self.dirty = true;
        Some(nid)
    }

    /// Every list as (id, "Space / Folder / List"), personal space included.
    pub(super) fn all_list_paths(&self) -> Vec<(u64, String)> {
        let mut out = vec![];
        for s in &self.data.spaces {
            if s.private_owner.map_or(false, |o| o != self.data.me) {
                continue;
            }
            for l in &s.lists {
                out.push((l.id, format!("{} / {}", s.name, l.name)));
            }
            for f in &s.folders {
                for l in &f.lists {
                    out.push((l.id, format!("{} / {} / {}", s.name, f.name, l.name)));
                }
            }
        }
        out
    }

    pub(super) fn move_tasks_body(&mut self, ui: &mut Ui, t: &Theme, ids: &[u64], query: &mut String) -> bool {
        let mut close = false;
        let here: Vec<u64> = ids.iter().filter_map(|i| self.task(*i)).map(|t| t.list_id).collect();
        ui.label(RichText::new(if ids.len() == 1 { tr("Move this task to another List.").to_string() } else { trf!("Move {} tasks to another List.", ids.len()) }).color(t.text2));
        let r = ui.add(egui::TextEdit::singleline(query).hint_text(tr("Search Lists...")).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
        focus_once(ui, &r);
        ui.add_space(4.0);
        let q = query.to_lowercase();
        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for (lid, path) in self.all_list_paths() {
                if !q.is_empty() && !path.to_lowercase().contains(&q) {
                    continue;
                }
                let cur = !here.is_empty() && here.iter().all(|h| *h == lid);
                let tc = t.text2;
                let r = menu_row(ui, t, &path, cur, |p, c| shell_checklist_icon(p, c, tc));
                if r.clicked() && !cur {
                    self.move_tasks(ids, lid);
                    self.selected.clear();
                    close = true;
                }
            }
        });
        close
    }

    pub(super) fn move_list_body(&mut self, ui: &mut Ui, t: &Theme, list: u64) -> bool {
        let mut close = false;
        ui.label(RichText::new(trf!("Move \"{}\" to:", self.data.list_name(list))).color(t.text2));
        ui.add_space(4.0);
        let spaces: Vec<(u64, String, Vec<(u64, String)>)> = self.open_spaces().iter().map(|s| (s.id, s.name.clone(), s.folders.iter().map(|f| (f.id, f.name.clone())).collect())).collect();
        let (cur_space, cur_folder) = {
            let sp = self.data.space_of_list(list).map(|s| s.id).unwrap_or(0);
            let f = self.data.spaces.iter().flat_map(|s| s.folders.iter()).find(|f| f.lists.iter().any(|l| l.id == list)).map(|f| f.id);
            (sp, f)
        };
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for (sid, sname, folders) in &spaces {
                let cur = *sid == cur_space && cur_folder.is_none();
                if menu_row(ui, t, sname, cur, |_, _| {}).clicked() && !cur {
                    self.move_list(list, *sid, None);
                    close = true;
                }
                for (fid, fname) in folders {
                    let cur = cur_folder == Some(*fid);
                    if menu_row(ui, t, &format!("    {}", fname), cur, |_, _| {}).clicked() && !cur {
                        self.move_list(list, *sid, Some(*fid));
                        close = true;
                    }
                }
            }
        });
        close
    }

    // ---------------------------------------------------------------- menus

    /// The task menu (right click on a row or card, and the row's "..." button).
    pub(super) fn task_menu(&mut self, ui: &mut Ui, tid: u64) {
        let t = self.t();
        let Some(task) = self.task(tid).cloned() else { return };
        ui.set_min_width(220.0);
        ui.horizontal(|ui| {
            if ui.button(tr("Copy link")).clicked() {
                ui.ctx().copy_text(format!("workboard://task/{}", tid));
                self.toast = Some((tr("Link copied").into(), now_ts()));
                close_any(ui);
            }
            if ui.button(tr("Copy ID")).clicked() {
                ui.ctx().copy_text(format!("{}", tid));
                self.toast = Some((tr("Task ID copied").into(), now_ts()));
                close_any(ui);
            }
        });
        ui.separator();
        if ui.button(tr("Open")).clicked() {
            self.actions.push(Action::Open(tid));
            close_any(ui);
        }
        if ui.button(tr("Rename")).clicked() {
            if self.view == ViewKind::List && self.open_task.is_none() && self.row_order.contains(&tid) {
                // a row on screen is renamed in place
                self.row_rename = Some((tid, task.name.clone()));
            } else {
                self.actions.push(Action::StartRename(Target::Task(tid)));
            }
            close_any(ui);
        }
        let star = format!("task:{}", tid);
        let fav = self.data.extra.starred.contains(&star);
        if ui.button(tr(if fav { "Remove from Favorites" } else { "Add to Favorites" })).clicked() {
            if fav {
                self.data.extra.starred.retain(|s| *s != star);
            } else {
                self.data.extra.starred.push(star);
            }
            self.dirty = true;
            close_any(ui);
        }
        let selected = self.selected.contains(&tid);
        if ui.button(if selected { tr("Deselect") } else { tr("Select") }).clicked() {
            if selected {
                self.selected.remove(&tid);
            } else {
                self.selected.insert(tid);
            }
            close_any(ui);
        }
        ui.separator();
        let statuses = self.data.statuses_for_list(task.list_id);
        ui.menu_button(tr("Status"), |ui| {
            ui.set_max_width(200.0);
            for st in &statuses {
                if menu_row(ui, &t, &crate::i18n::status_text(&st.name).to_uppercase(), st.name == task.status, |p, c| paint_status_circle(p, c, 6.0, st)).clicked() {
                    let mut e = task.clone();
                    e.status = st.name.clone();
                    self.actions.push(Action::Replace(e));
                    close_any(ui);
                }
            }
        });
        if self.data.extra.apps.priorities {
            ui.menu_button(tr("Priority"), |ui| {
                ui.set_max_width(180.0);
                for pr in [1u8, 2, 3, 4, 0] {
                    let col = priority_color(pr, &t);
                    let label = if pr == 0 { "Clear" } else { PRIORITIES[pr as usize].label };
                    if menu_row(ui, &t, tr(label), task.priority == pr, |p, c| paint_flag(p, c + vec2(-4.0, 0.0), col, 13.0)).clicked() {
                        let mut e = task.clone();
                        e.priority = pr;
                        self.actions.push(Action::Replace(e));
                        close_any(ui);
                    }
                }
            });
        }
        ui.menu_button(tr("Assignees"), |ui| {
            ui.set_max_width(220.0);
            let members = self.data.members.clone();
            for m in &members {
                let on = task.assignees.contains(&m.id);
                let ring = t.card;
                if menu_row(ui, &t, &m.name, on, |p, c| paint_avatar(p, c, 9.0, m, ring)).clicked() {
                    let mut e = task.clone();
                    if on {
                        e.assignees.retain(|a| *a != m.id);
                    } else if self.data.extra.apps.multiple_assignees {
                        e.assignees.push(m.id);
                    } else {
                        e.assignees = vec![m.id];
                    }
                    self.actions.push(Action::Replace(e));
                    close_any(ui);
                }
            }
        });
        ui.menu_button(tr("Due date"), |ui| {
            let td = today();
            let wd = td.weekday().num_days_from_monday() as i64;
            for (label, d) in [("Today", Some(td)), ("Tomorrow", Some(td + Duration::days(1))), ("Next week", Some(td + Duration::days(7 - wd))), ("2 weeks", Some(td + Duration::days(14))), ("Clear", None)] {
                if ui.button(tr(label)).clicked() {
                    let mut e = task.clone();
                    e.due = d;
                    self.actions.push(Action::Replace(e));
                    close_any(ui);
                }
            }
        });
        ui.menu_button(tr("Repeat"), |ui| {
            for (v, label) in RECUR {
                let sel = task.recur.as_deref().unwrap_or("") == v;
                if ui.selectable_label(sel, tr(label)).clicked() {
                    let mut e = task.clone();
                    e.recur = if v.is_empty() { None } else { Some(v.to_string()) };
                    if e.recur.is_some() && e.due.is_none() {
                        e.due = Some(today());
                    }
                    self.actions.push(Action::Replace(e));
                    close_any(ui);
                }
            }
        });
        ui.separator();
        if ui.button(tr("Add subtask")).clicked() {
            self.actions.push(Action::Add { list: task.list_id, name: String::new(), status: None, priority: 0, assignees: vec![], due: None, parent: Some(tid), open: true });
            close_any(ui);
        }
        if ui.button(tr("Move to...")).clicked() {
            self.modal = Some(Modal::MoveTasks { ids: vec![tid], query: String::new() });
            close_any(ui);
        }
        if ui.button(tr("Make subtask of...")).clicked() {
            self.modal = Some(Modal::PickTask { task: tid, mode: TaskPick::Parent, query: String::new() });
            close_any(ui);
        }
        if task.parent.is_some() && ui.button(tr("Make top-level task")).clicked() {
            self.make_top_level(tid);
            close_any(ui);
        }
        if ui.button(tr("Duplicate")).clicked() {
            if self.duplicate_task(tid).is_some() {
                self.toast = Some((tr("Task duplicated").into(), now_ts()));
            }
            close_any(ui);
        }
        if ui.button(tr("Remind me")).clicked() {
            self.modal = Some(Modal::Reminder { text: task.name.clone(), date: Some(today() + Duration::days(1)), time: "09:00".into() });
            self.reminder_about = (Some(task.id), None);
            close_any(ui);
        }
        if ui.button(if self.data.is_my_timer(&task) { tr("Stop timer") } else { tr("Start timer") }).clicked() {
            self.actions.push(Action::ToggleTimer(tid));
            close_any(ui);
        }
        if ui.button(tr("Save as template")).clicked() {
            let id = self.data.new_id();
            self.data.extra.templates.push(TaskTemplate { id, name: task.name.clone(), task_name: task.name.clone(), description: task.description.clone(), checklist: task.checklist.iter().map(|c| c.text.clone()).collect(), tags: task.tags.clone(), priority: task.priority });
            self.toast = Some((tr("Saved as a template").into(), now_ts()));
            self.dirty = true;
            close_any(ui);
        }
        ui.separator();
        if ui.button(if task.archived { tr("Unarchive") } else { tr("Archive") }).clicked() {
            self.set_archived(&[tid], !task.archived);
            close_any(ui);
        }
        if ui.button(RichText::new(tr("Delete")).color(t.danger)).clicked() {
            self.actions.push(Action::DeleteTask(tid));
            close_any(ui);
        }
    }

    /// The list menu (right click, or the "..." button on a sidebar list).
    pub(super) fn list_menu(&mut self, ui: &mut Ui, lid: u64) {
        let t = self.t();
        let loc = Location::List(lid);
        ui.set_min_width(200.0);
        let fav = if self.data.favorites.contains(&loc) { "Remove from Favorites" } else { "Add to Favorites" };
        if ui.button(tr(fav)).clicked() {
            self.actions.push(Action::ToggleFavorite(loc));
            close_any(ui);
        }
        if ui.button(tr("Rename")).clicked() {
            self.actions.push(Action::StartRename(Target::List(lid)));
            close_any(ui);
        }
        if ui.button(tr("Copy link")).clicked() {
            ui.ctx().copy_text(format!("workboard://list/{}", lid));
            self.toast = Some((tr("Link copied").into(), now_ts()));
            close_any(ui);
        }
        if ui.button(tr("List Info")).clicked() {
            self.modal = Some(Modal::ListInfo { list: lid });
            close_any(ui);
        }
        let cur = self.data.list_item(lid).and_then(|l| l.color);
        ui.menu_button(tr("Color"), |ui| {
            ui.label(RichText::new(tr("List color")).small().color(t.text3));
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(190.0);
                for c in STATUS_PALETTE.iter().skip(1) {
                    let (rr, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    name_it(&resp, &format!("List color {:02x}{:02x}{:02x}", c[0], c[1], c[2]));
                    ui.painter().circle_filled(rr.center(), 8.0, rgb(*c));
                    if cur == Some(*c) {
                        ui.painter().circle_stroke(rr.center(), 9.5, Stroke::new(1.5_f32, t.text));
                    }
                    if resp.clicked() {
                        if let Some(l) = self.data.list_item_mut(lid) {
                            l.color = Some(*c);
                            self.dirty = true;
                        }
                        close_any(ui);
                    }
                }
            });
            if cur.is_some() && ui.button(tr("No color")).clicked() {
                if let Some(l) = self.data.list_item_mut(lid) {
                    l.color = None;
                    self.dirty = true;
                }
                close_any(ui);
            }
        });
        ui.separator();
        ui.menu_button(tr("Create new"), |ui| self.list_create_menu(ui, lid));
        if let Some(sid) = self.data.space_of_list(lid).map(|s| s.id) {
            if ui.button(tr("Task statuses")).clicked() {
                self.actions.push(Action::EditStatuses(sid));
                close_any(ui);
            }
        }
        if ui.button(tr("Custom Fields")).clicked() {
            self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
            close_any(ui);
        }
        if ui.button(tr("Automations")).clicked() {
            self.modal = Some(Modal::Automations { status: "complete".into(), action: "comment".into(), value: String::new(), scope: Some(Location::List(lid)) });
            close_any(ui);
        }
        ui.separator();
        if ui.button(tr("Move...")).clicked() {
            self.modal = Some(Modal::MoveList { list: lid });
            close_any(ui);
        }
        if ui.button(tr("Duplicate")).clicked() {
            if let Some(n) = self.duplicate_list(lid) {
                self.section = Section::Home;
                self.loc = Location::List(n);
                self.toast = Some((tr("List duplicated").into(), now_ts()));
            }
            close_any(ui);
        }
        if ui.button(tr("Archive")).clicked() {
            self.set_list_archived(lid, true);
            close_any(ui);
        }
        if ui.button(RichText::new(tr("Delete")).color(t.danger)).clicked() {
            self.actions.push(Action::AskDelete(Target::List(lid)));
            close_any(ui);
        }
    }

    /// The folder menu (right click, or the "..." button on a sidebar folder).
    pub(super) fn folder_menu(&mut self, ui: &mut Ui, fid: u64) {
        let t = self.t();
        let floc = Location::Folder(fid);
        let Some(sid) = self.data.spaces.iter().find(|s| s.folders.iter().any(|f| f.id == fid)).map(|s| s.id) else { return };
        ui.set_min_width(200.0);
        let fav = if self.data.favorites.contains(&floc) { "Remove from Favorites" } else { "Add to Favorites" };
        if ui.button(tr(fav)).clicked() {
            self.actions.push(Action::ToggleFavorite(floc));
            close_any(ui);
        }
        if ui.button(tr("Rename")).clicked() {
            self.actions.push(Action::StartRename(Target::Folder(fid)));
            close_any(ui);
        }
        if ui.button(tr("Copy link")).clicked() {
            ui.ctx().copy_text(format!("workboard://folder/{}", fid));
            self.toast = Some((tr("Link copied").into(), now_ts()));
            close_any(ui);
        }
        ui.separator();
        ui.menu_button(tr("Create new"), |ui| {
            if ui.button(tr("List")).clicked() {
                self.open_new_list(Some(sid), Some(fid));
                close_any(ui);
            }
            if ui.button(tr("Doc")).clicked() {
                self.new_doc(Some(sid));
                close_any(ui);
            }
        });
        let others: Vec<(u64, String)> = self.open_spaces().iter().filter(|s| s.id != sid).map(|s| (s.id, s.name.clone())).collect();
        ui.menu_button(tr("Move to Space"), |ui| {
            if others.is_empty() {
                ui.label(RichText::new(tr("No other Space")).color(t.text3));
            }
            for (id, name) in &others {
                if ui.button(name).clicked() {
                    self.move_folder(fid, *id);
                    close_any(ui);
                }
            }
        });
        if ui.button(tr("Duplicate")).clicked() {
            if let Some(n) = self.duplicate_folder(fid) {
                self.actions.push(Action::Go(Location::Folder(n)));
                self.toast = Some((tr("Folder duplicated").into(), now_ts()));
            }
            close_any(ui);
        }
        if ui.button(tr("Archive")).clicked() {
            self.set_folder_archived(fid, true);
            close_any(ui);
        }
        ui.separator();
        if ui.button(RichText::new(tr("Delete")).color(t.danger)).clicked() {
            self.actions.push(Action::AskDelete(Target::Folder(fid)));
            close_any(ui);
        }
    }

    pub(super) fn set_folder_archived(&mut self, fid: u64, archived: bool) {
        let mut name = String::new();
        let mut lists = vec![];
        for f in self.data.spaces.iter_mut().flat_map(|s| s.folders.iter_mut()).filter(|f| f.id == fid) {
            f.archived = archived;
            name = f.name.clone();
            lists = f.lists.iter().map(|l| l.id).collect();
            self.dirty = true;
        }
        let here = self.loc == Location::Folder(fid) || matches!(self.loc, Location::List(l) if lists.contains(&l));
        if archived && here {
            self.loc = Location::Home;
        }
        let msg = if archived { trf!("Archived \"{}\". Restore it from Settings > Spaces.", name) } else { trf!("Restored \"{}\".", name) };
        self.toast = Some((msg, now_ts()));
    }

    /// Moves a folder, with its lists, to the end of another space.
    pub(super) fn move_folder(&mut self, fid: u64, space: u64) {
        if !self.data.spaces.iter().any(|s| s.id == space) {
            return;
        }
        let mut taken = None;
        for s in &mut self.data.spaces {
            if let Some(i) = s.folders.iter().position(|f| f.id == fid) {
                taken = Some(s.folders.remove(i));
            }
        }
        let Some(f) = taken else { return };
        let lists: Vec<u64> = f.lists.iter().map(|l| l.id).collect();
        let name = f.name.clone();
        let statuses = self.data.spaces.iter().find(|s| s.id == space).map(|s| s.statuses.clone()).unwrap_or_else(default_statuses);
        if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == space) {
            s.expanded = true;
            s.folders.push(f);
        }
        // tasks take the new space's statuses
        for t in self.data.tasks.iter_mut().filter(|t| lists.contains(&t.list_id)) {
            if !statuses.iter().any(|s| s.name == t.status) {
                t.status = statuses[0].name.clone();
            }
        }
        self.toast = Some((trf!("Moved {} to {}", name, self.data.location_name(Location::Space(space))), now_ts()));
        self.dirty = true;
    }

    /// A copy of a folder next to it, with copies of its lists and their tasks.
    pub(super) fn duplicate_folder(&mut self, fid: u64) -> Option<u64> {
        let (sid, src) = self.data.spaces.iter().find_map(|s| s.folders.iter().find(|f| f.id == fid).map(|f| (s.id, f.clone())))?;
        let nid = self.data.new_id();
        let mut copy = Folder { id: nid, name: format!("{} (copy)", src.name), lists: vec![], expanded: true, archived: false };
        for l in src.lists.iter().filter(|l| !l.archived) {
            if let Some(n) = self.duplicate_list(l.id) {
                if let Some(item) = self.take_list(n) {
                    copy.lists.push(ListItem { name: l.name.clone(), ..item });
                }
            }
        }
        let s = self.data.spaces.iter_mut().find(|s| s.id == sid)?;
        let pos = s.folders.iter().position(|f| f.id == fid).map(|i| i + 1).unwrap_or(s.folders.len());
        s.folders.insert(pos, copy);
        self.dirty = true;
        Some(nid)
    }

    /// Puts a list just before another one (sidebar drag and drop), in the
    /// other one's space or folder.
    pub(super) fn place_list_before(&mut self, list: u64, before: u64) {
        if list == before || self.data.list_item(before).is_none() {
            return;
        }
        let Some(item) = self.take_list(list) else { return };
        let mut space = None;
        for s in &mut self.data.spaces {
            if let Some(i) = s.lists.iter().position(|l| l.id == before) {
                s.lists.insert(i, item.clone());
                space = Some(s.id);
            }
            for f in &mut s.folders {
                if let Some(i) = f.lists.iter().position(|l| l.id == before) {
                    f.lists.insert(i, item.clone());
                    space = Some(s.id);
                }
            }
        }
        let statuses = space.and_then(|sid| self.data.spaces.iter().find(|s| s.id == sid)).map(|s| s.statuses.clone()).unwrap_or_else(default_statuses);
        for t in self.data.tasks.iter_mut().filter(|t| t.list_id == list) {
            if !statuses.iter().any(|s| s.name == t.status) {
                t.status = statuses[0].name.clone();
            }
        }
        self.dirty = true;
    }

    /// Puts a space just before another one (sidebar drag and drop).
    pub(super) fn place_space_before(&mut self, space: u64, before: u64) {
        if space == before || !self.data.spaces.iter().any(|s| s.id == before) {
            return;
        }
        let Some(i) = self.data.spaces.iter().position(|s| s.id == space) else { return };
        let s = self.data.spaces.remove(i);
        let j = self.data.spaces.iter().position(|s| s.id == before).unwrap_or(self.data.spaces.len());
        self.data.spaces.insert(j, s);
        self.dirty = true;
    }

    /// The "+" on a sidebar list, and the list menu's "Create new".
    pub(super) fn list_create_menu(&mut self, ui: &mut Ui, lid: u64) {
        ui.set_min_width(200.0);
        let sid = self.data.space_of_list(lid).map(|s| s.id);
        let folder = self.data.spaces.iter().flat_map(|s| s.folders.iter()).find(|f| f.lists.iter().any(|l| l.id == lid)).map(|f| f.id);
        ui.label(RichText::new(tr("Create")).small().color(self.t().text3));
        for (icon, label) in [("\u{2611}", "Task"), ("\u{2630}", "List"), ("\u{1F4C4}", "Doc"), ("\u{1F4CA}", "Dashboard"), ("\u{1F3A8}", "Whiteboard"), ("\u{1F4CB}", "Form")] {
            let b = ui.button(format!("{}  {}", icon, tr(label)));
            name_it(&b, &format!("Create {}", label));
            if b.clicked() {
                match label {
                    "Task" => {
                        self.section = Section::Home;
                        self.loc = Location::List(lid);
                        self.open_quick_add(None);
                    }
                    "List" => self.open_new_list(sid, folder),
                    "Doc" => self.new_doc(sid),
                    "Dashboard" => self.modal = Some(Modal::NewDashboard { name: "New Dashboard".into() }),
                    "Whiteboard" => self.new_whiteboard(),
                    _ => {
                        self.new_form();
                        if let Some(f) = self.data.extra.forms.last_mut() {
                            f.list_id = lid;
                        }
                    }
                }
                close_any(ui);
            }
        }
    }

    pub(super) fn set_list_archived(&mut self, lid: u64, archived: bool) {
        let name = self.data.list_name(lid);
        if let Some(l) = self.data.list_item_mut(lid) {
            l.archived = archived;
            self.dirty = true;
        }
        if archived && self.loc == Location::List(lid) {
            self.loc = Location::Home;
        }
        let msg = if archived { trf!("Archived \"{}\". Restore it from Settings > Spaces.", name) } else { trf!("Restored \"{}\".", name) };
        self.toast = Some((msg, now_ts()));
    }

    pub(super) fn set_space_archived(&mut self, sid: u64, archived: bool) {
        let lists = self.data.lists_in(Location::Space(sid)).unwrap_or_default();
        if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
            s.archived = archived;
            self.dirty = true;
        }
        let here = self.loc == Location::Space(sid) || matches!(self.loc, Location::List(l) if lists.contains(&l)) || matches!(self.loc, Location::Folder(f) if self.data.spaces.iter().any(|s| s.id == sid && s.folders.iter().any(|x| x.id == f)));
        if archived && here {
            self.loc = Location::Home;
        }
    }

    /// A copy of the space: statuses, folders and lists, with their tasks.
    pub(super) fn duplicate_space(&mut self, sid: u64) -> Option<u64> {
        let src = self.data.spaces.iter().find(|s| s.id == sid)?.clone();
        let nid = self.data.new_id();
        let mut copy = Space { id: nid, name: format!("{} (copy)", src.name), color: src.color, statuses: src.statuses.clone(), folders: vec![], lists: vec![], expanded: true, private_owner: None, icon: src.icon.clone(), ..Default::default() };
        let pos = self.data.spaces.iter().position(|s| s.id == sid).map(|i| i + 1).unwrap_or(self.data.spaces.len());
        self.data.spaces.insert(pos, copy.clone());
        // copy each list with duplicate_list (it lands next to the original), then move it over
        let mut moved: Vec<(Option<usize>, ListItem)> = vec![];
        let plain: Vec<u64> = src.lists.iter().filter(|l| !l.archived).map(|l| l.id).collect();
        for l in plain {
            if let Some(n) = self.duplicate_list(l) {
                if let Some(item) = self.take_list(n) {
                    moved.push((None, ListItem { name: self.data.list_name(l), ..item }));
                }
            }
        }
        for (fi, f) in src.folders.iter().enumerate() {
            copy.folders.push(Folder { id: self.data.new_id(), name: f.name.clone(), lists: vec![], expanded: true, archived: false });
            for l in f.lists.iter().filter(|l| !l.archived) {
                if let Some(n) = self.duplicate_list(l.id) {
                    if let Some(item) = self.take_list(n) {
                        moved.push((Some(fi), ListItem { name: l.name.clone(), ..item }));
                    }
                }
            }
        }
        for (fi, item) in moved {
            match fi {
                Some(i) => copy.folders[i].lists.push(item),
                None => copy.lists.push(item),
            }
        }
        if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == nid) {
            *s = copy;
        }
        self.dirty = true;
        Some(nid)
    }

    /// List Info: where the list is, its task counts and a description.
    pub(super) fn list_info_body(&mut self, ui: &mut Ui, t: &Theme, lid: u64) -> bool {
        let Some(item) = self.data.list_item(lid).cloned() else { return true };
        let path = self.data.list_path(lid).join(" / ");
        let tops: Vec<&Task> = self.data.tasks.iter().filter(|x| x.list_id == lid && x.parent.is_none() && !x.archived).collect();
        let closed = tops.iter().filter(|x| self.data.is_closed(x)).count();
        let subs = self.data.tasks.iter().filter(|x| x.list_id == lid && x.parent.is_some() && !x.archived).count();
        let archived = self.data.tasks.iter().filter(|x| x.list_id == lid && x.archived).count();
        egui::Grid::new("list_info").num_columns(2).spacing(vec2(24.0, 8.0)).show(ui, |ui| {
            for (k, v) in [
                ("Name", item.name.clone()),
                ("Location", path),
                ("Tasks", trf!("{} ({} open, {} closed)", tops.len(), tops.len() - closed, closed)),
                ("Subtasks", subs.to_string()),
                ("Archived tasks", archived.to_string()),
            ] {
                ui.label(RichText::new(tr(k)).color(t.text2));
                ui.label(RichText::new(v).color(t.text));
                ui.end_row();
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Description")).color(t.text2));
        let mut desc = item.description.clone();
        let r = ui.add(egui::TextEdit::multiline(&mut desc).hint_text(tr("What is this List for?")).desired_rows(3).desired_width(f32::INFINITY));
        name_field(&r, "List description");
        if desc != item.description {
            if let Some(l) = self.data.list_item_mut(lid) {
                l.description = desc;
            }
            self.dirty = true;
        }
        ui.add_space(10.0);
        let mut close = false;
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, t, tr("Done")).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// Removes a list from wherever it sits and returns it.
    fn take_list(&mut self, lid: u64) -> Option<ListItem> {
        for s in &mut self.data.spaces {
            if let Some(i) = s.lists.iter().position(|l| l.id == lid) {
                return Some(s.lists.remove(i));
            }
            for f in &mut s.folders {
                if let Some(i) = f.lists.iter().position(|l| l.id == lid) {
                    return Some(f.lists.remove(i));
                }
            }
        }
        None
    }

    /// The space menu (right click, or the "..." button on a sidebar space).
    pub(super) fn space_menu(&mut self, ui: &mut Ui, sid: u64) {
        let t = self.t();
        let loc = Location::Space(sid);
        ui.set_min_width(200.0);
        let fav = if self.data.favorites.contains(&loc) { "Remove from Favorites" } else { "Add to Favorites" };
        if ui.button(tr(fav)).clicked() {
            self.actions.push(Action::ToggleFavorite(loc));
            close_any(ui);
        }
        if ui.button(tr("Rename")).clicked() {
            self.actions.push(Action::StartRename(Target::Space(sid)));
            close_any(ui);
        }
        if ui.button(tr("Copy link")).clicked() {
            ui.ctx().copy_text(format!("workboard://space/{}", sid));
            self.toast = Some((tr("Link copied").into(), now_ts()));
            close_any(ui);
        }
        ui.menu_button(tr("Create new"), |ui| {
            if ui.button(tr("List")).clicked() {
                self.open_new_list(Some(sid), None);
                close_any(ui);
            }
            if ui.button(tr("Folder")).clicked() {
                self.actions.push(Action::NewFolder(sid));
                close_any(ui);
            }
            if ui.button(tr("Doc")).clicked() {
                self.new_doc(Some(sid));
                close_any(ui);
            }
        });
        ui.menu_button(tr("Color & Icon"), |ui| {
            ui.label(RichText::new(tr("Space color")).small().color(t.text3));
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(190.0);
                for c in STATUS_PALETTE.iter().skip(1) {
                    let (rr, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    name_it(&resp, &format!("Space color {:02x}{:02x}{:02x}", c[0], c[1], c[2]));
                    ui.painter().circle_filled(rr.center(), 8.0, rgb(*c));
                    if resp.clicked() {
                        self.actions.push(Action::SetSpaceColor(sid, *c));
                        close_any(ui);
                    }
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new(tr("Icon")).small().color(t.text3));
            let color = self.data.spaces.iter().find(|s| s.id == sid).map(|s| s.color).unwrap_or([96, 73, 231]);
            let letter = self.data.spaces.iter().find(|s| s.id == sid).map(|s| s.name.chars().next().unwrap_or('S').to_uppercase().to_string()).unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(190.0);
                for ic in SPACE_ICONS {
                    let (rr, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                    name_it(&resp, &format!("Space icon {}", if ic.is_empty() { "letter" } else { ic }));
                    ui.painter().rect_filled(rr.shrink(2.0), 5.0, rgb(color));
                    ui.painter().text(rr.center(), Align2::CENTER_CENTER, if ic.is_empty() { letter.as_str() } else { ic }, FontId::proportional(11.0), Color32::WHITE);
                    if resp.clicked() {
                        if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
                            s.icon = ic.to_string();
                            self.dirty = true;
                        }
                        close_any(ui);
                    }
                }
            });
        });
        if ui.button(tr("Task statuses")).clicked() {
            self.actions.push(Action::EditStatuses(sid));
            close_any(ui);
        }
        if ui.button(tr("Custom Fields")).clicked() {
            self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
            close_any(ui);
        }
        if ui.button(tr("Automations")).clicked() {
            self.modal = Some(Modal::Automations { status: "complete".into(), action: "comment".into(), value: String::new(), scope: Some(Location::Space(sid)) });
            close_any(ui);
        }
        ui.separator();
        if ui.button(tr("Duplicate")).clicked() {
            if let Some(n) = self.duplicate_space(sid) {
                self.actions.push(Action::Go(Location::Space(n)));
                self.toast = Some((tr("Space duplicated").into(), now_ts()));
            }
            close_any(ui);
        }
        let hidden = self.data.spaces.iter().find(|s| s.id == sid).is_some_and(|s| s.hidden);
        if ui.button(if hidden { tr("Show Space") } else { tr("Hide Space") }).clicked() {
            if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
                s.hidden = !hidden;
                self.dirty = true;
            }
            if !hidden {
                self.toast = Some((tr("Space hidden from the sidebar. Show it again from Settings > Spaces.").into(), now_ts()));
            }
            close_any(ui);
        }
        let archived = self.data.spaces.iter().find(|s| s.id == sid).is_some_and(|s| s.archived);
        if ui.button(if archived { tr("Restore") } else { tr("Archive") }).clicked() {
            self.set_space_archived(sid, !archived);
            close_any(ui);
        }
        if ui.button(RichText::new(tr("Delete")).color(t.danger)).clicked() {
            self.actions.push(Action::AskDelete(Target::Space(sid)));
            close_any(ui);
        }
    }

    // ---------------------------------------------------------------- toolbar panels

    pub(super) fn col_on(&self, c: &str) -> bool {
        !self.hidden_cols.contains(c)
    }

    /// The text a property column shows for a task.
    pub(super) fn prop_text(&self, t: &Task, name: &str) -> String {
        let day = |ts: i64| chrono::DateTime::from_timestamp(ts, 0).map(|d| fmt_date(d.with_timezone(&chrono::Local).date_naive())).unwrap_or_default();
        match name {
            "Created by" => t.activity.first().and_then(|a| self.data.member(a.author)).map(|m| m.name.clone()).unwrap_or_default(),
            "Date closed" => t.closed_at.map(day).unwrap_or_default(),
            "Date done" => t.done_at.map(day).unwrap_or_default(),
            "Date updated" => day(t.updated),
            "Start date" => t.start.map(fmt_date).unwrap_or_default(),
            "Time estimate" => t.estimate_min.map(|m| fmt_duration(m as i64 * 60)).unwrap_or_default(),
            "Time tracked" => {
                let s = t.tracked_secs(now_ts());
                if s > 0 { fmt_duration(s) } else { String::new() }
            }
            "Tags" => t.tags.join(", "),
            "Latest comment" => t.activity.iter().rev().find(|a| a.comment).map(|a| a.text.replace('\n', " ")).unwrap_or_default(),
            "Subtasks" => {
                let n = self.data.tasks.iter().filter(|x| x.parent == Some(t.id) && !x.archived).count();
                if n > 0 { n.to_string() } else { String::new() }
            }
            _ => String::new(),
        }
    }

    /// The "Fields" panel on the right of a List: search, the fields shown,
    /// and the properties that can be added.
    pub(super) fn fields_panel(&mut self, ctx: &egui::Context, top: f32) {
        let t = self.t();
        let screen = ctx.screen_rect();
        let rect = Rect::from_min_max(pos2(screen.right() - 320.0, top), pos2(screen.right() - 8.0, screen.bottom() - 8.0));
        let q_id = egui::Id::new("fields_query");
        let mut q: String = ctx.data(|d| d.get_temp(q_id)).unwrap_or_default();
        let mut close = false;
        egui::Area::new(egui::Id::new("fields_panel")).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(14)).shadow(egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }).show(ui, |ui| {
                ui.set_width(rect.width() - 28.0);
                ui.set_min_height(rect.height() - 28.0);
                if dialog_header(ui, &t, tr("Fields")) {
                    close = true;
                }
                ui.add_space(6.0);
                let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text(tr("Search for new or existing fields")).desired_width(f32::INFINITY));
                name_field(&r, "Search fields");
                ui.add_space(6.0);
                let ql = q.trim().to_lowercase();
                let hit = |n: &str| ql.is_empty() || n.to_lowercase().contains(&ql);
                let all: Vec<&str> = COLUMNS.iter().chain(PROPERTIES.iter()).copied().collect();
                // shown properties that the List has no room to draw
                let fits: Vec<&str> = if self.list_width > 0.0 {
                    super::shell::col_layout(Rect::from_min_size(pos2(0.0, 0.0), vec2(self.list_width, 30.0)), &self.hidden_cols, self.data.extra.apps.priorities).extra.iter().map(|c| c.0).collect()
                } else {
                    PROPERTIES.to_vec()
                };
                egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(rect.height() - 160.0).show(ui, |ui| {
                    for (title, shown) in [("Shown", true), ("Properties", false)] {
                        let names: Vec<&str> = all.iter().copied().filter(|n| self.col_on(n) == shown && hit(n)).collect();
                        ui.label(RichText::new(tr(title)).small().color(t.text3));
                        if names.is_empty() {
                            ui.label(RichText::new(if shown { tr("No fields shown") } else { tr("No more fields") }).small().color(t.text3));
                        }
                        for n in names {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(tr(n)).color(t.text));
                                if shown && PROPERTIES.contains(&n) && !fits.contains(&n) {
                                    ui.label(RichText::new(tr("no room")).small().color(t.text3)).on_hover_text(tr("The List is too narrow to show this column; widen the window or turn another field off."));
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let mut on = shown;
                                    if switch(ui, &t, &mut on, &format!("Field {}", n)).clicked() {
                                        if on {
                                            self.hidden_cols.remove(n);
                                        } else {
                                            self.hidden_cols.insert(n.to_string());
                                        }
                                    }
                                });
                            });
                        }
                        ui.add_space(8.0);
                    }
                });
                ui.separator();
                if ui.button(tr("+ Create new field")).clicked() {
                    self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
                    close = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.menu_was_open && self.modal.is_none() {
            close = true;
        }
        if close {
            self.fields_open = false;
            ctx.data_mut(|d| d.remove::<String>(q_id));
        } else {
            ctx.data_mut(|d| d.insert_temp(q_id, q));
        }
    }

    /// Choices for a filter row's values: (stored value, label).
    fn filter_choices(&self, f: FilterField) -> Vec<(String, String)> {
        match f {
            FilterField::Status => self.location_statuses().into_iter().map(|s| (s.name.clone(), crate::i18n::status_text(&s.name).to_uppercase())).collect(),
            FilterField::Priority => [1u8, 2, 3, 4, 0].iter().map(|p| (p.to_string(), if *p == 0 { tr("No priority").to_string() } else { tr(PRIORITIES[*p as usize].label).to_string() })).collect(),
            FilterField::Assignee => {
                let mut v: Vec<(String, String)> = self.data.members.iter().map(|m| (m.id.to_string(), m.name.clone())).collect();
                v.push(("none".into(), tr("Unassigned").into()));
                v
            }
            FilterField::DueDate => DUE_FILTERS.iter().enumerate().skip(1).map(|(i, l)| (i.to_string(), tr(l).to_string())).collect(),
            FilterField::Tags => {
                let mut tags: Vec<String> = self.data.tasks.iter().flat_map(|t| t.tags.iter().cloned()).collect();
                tags.sort();
                tags.dedup();
                tags.into_iter().map(|g| (g.clone(), g)).collect()
            }
            FilterField::StartDate => START_FILTERS.iter().enumerate().skip(1).map(|(i, l)| (i.to_string(), tr(l).to_string())).collect(),
            FilterField::Created | FilterField::Updated => AGE_FILTERS.iter().enumerate().skip(1).map(|(i, l)| (i.to_string(), tr(l).to_string())).collect(),
            FilterField::CreatedBy => self.data.members.iter().map(|m| (m.id.to_string(), m.name.clone())).collect(),
            FilterField::HasSubtasks => vec![("yes".into(), tr("Has subtasks").into()), ("no".into(), tr("No subtasks").into())],
        }
    }

    /// The Filter panel: a search field and condition rows ("Select filter",
    /// is / is not, values) joined with AND. Drawn as a panel, not a popup,
    /// so the drop-downs in its rows can open.
    pub(super) fn filter_panel(&mut self, ctx: &egui::Context, at: Pos2, pill: Rect) {
        let t = self.t();
        let mut close = false;
        let area = egui::Area::new(egui::Id::new("filter_panel")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.set_width(560.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Filters")).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(tr("Clear all")).clicked() {
                            self.filter = Default::default();
                            self.search.clear();
                            self.me_mode = false;
                            self.assignee_filter = None;
                        }
                    });
                });
                let r = ui.add(egui::TextEdit::singleline(&mut self.search).hint_text(tr("Task name or tag...")).desired_width(f32::INFINITY));
                name_field(&r, "Filter search");
                ui.add_space(6.0);
                let mut drop: Option<usize> = None;
                for i in 0..self.filter.rows.len() {
                    let row = self.filter.rows[i].clone();
                    let mut new = row.clone();
                    ui.horizontal(|ui| {
                        // a fixed-width lead so every row's drop-downs line up
                        let (lr, _) = ui.allocate_exact_size(vec2(48.0, 20.0), Sense::hover());
                        ui.painter().text(lr.left_center(), Align2::LEFT_CENTER, if i == 0 { tr("Where") } else { tr("And") }, FontId::proportional(14.0), t.text3);
                        let fr = egui::ComboBox::from_id_salt(("filter_field", i)).width(130.0).selected_text(row.field.map(|f| tr(f.label())).unwrap_or(tr("Select filter"))).show_ui(ui, |ui| {
                            for f in FilterField::ALL {
                                if ui.selectable_label(row.field == Some(f), tr(f.label())).clicked() && row.field != Some(f) {
                                    new.field = Some(f);
                                    new.values.clear();
                                }
                            }
                        });
                        name_it(&fr.response, &format!("Filter {} field", i + 1));
                        ui.add_enabled_ui(row.field.is_some(), |ui| {
                            let or = egui::ComboBox::from_id_salt(("filter_op", i)).width(70.0).selected_text(if row.not { tr("is not") } else { tr("is") }).show_ui(ui, |ui| {
                                if ui.selectable_label(!row.not, tr("is")).clicked() {
                                    new.not = false;
                                }
                                if ui.selectable_label(row.not, tr("is not")).clicked() {
                                    new.not = true;
                                }
                            });
                            name_it(&or.response, &format!("Filter {} condition", i + 1));
                            let choices = row.field.map(|f| self.filter_choices(f)).unwrap_or_default();
                            let summary = if row.values.is_empty() {
                                tr("Select option").to_string()
                            } else {
                                let names: Vec<String> = row.values.iter().filter_map(|v| choices.iter().find(|c| c.0 == *v).map(|c| c.1.clone())).collect();
                                truncate(&names.join(", "), 24)
                            };
                            let single = row.field.is_some_and(|f| f.single());
                            let vr = egui::ComboBox::from_id_salt(("filter_vals", i))
                                .width(200.0)
                                .selected_text(summary)
                                .close_behavior(if single { egui::PopupCloseBehavior::CloseOnClick } else { egui::PopupCloseBehavior::CloseOnClickOutside })
                                .show_ui(ui, |ui| {
                                    for (v, label) in &choices {
                                        let on = row.values.contains(v);
                                        if ui.selectable_label(on, label).clicked() {
                                            if single {
                                                new.values = vec![v.clone()];
                                            } else if on {
                                                new.values.retain(|x| x != v);
                                            } else {
                                                new.values.push(v.clone());
                                            }
                                        }
                                    }
                                    if choices.is_empty() {
                                        ui.label(RichText::new(tr("Nothing to pick")).color(t.text3));
                                    }
                                });
                            name_it(&vr.response, &format!("Filter {} values", i + 1));
                        });
                        let x = ui.add(egui::Button::new("\u{2716}").frame(false));
                        name_it(&x, &format!("Remove filter {}", i + 1));
                        if x.clicked() {
                            drop = Some(i);
                        }
                    });
                    if new != row {
                        self.filter.rows[i] = new;
                    }
                }
                if let Some(i) = drop {
                    self.filter.rows.remove(i);
                }
                if ui.add(egui::Button::new(RichText::new(tr("+ Add filter")).color(t.text2)).frame(false)).clicked() {
                    self.filter.rows.push(FilterRow::default());
                }
                ui.separator();
                ui.checkbox(&mut self.filter.archived, tr("Show archived tasks only"));
                ui.checkbox(&mut self.me_mode, tr("Only my tasks"));
            });
        });
        // close on Escape, or a press outside the panel and its drop-downs
        let pressed_outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p) && !pill.contains(p)) && !self.menu_was_open && !ctx.memory(|m| m.any_popup_open());
        if pressed_outside || (ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.menu_was_open) {
            close = true;
        }
        if close {
            self.filter_open = false;
        }
    }

    // ---------------------------------------------------------------- bulk edit

    /// The bar along the bottom while tasks are selected.
    pub(super) fn bulk_bar(&mut self, ctx: &egui::Context) {
        self.undo_bar(ctx);
        self.selected.retain(|id| self.data.tasks.iter().any(|t| t.id == *id));
        self.bulk_delete_confirm(ctx);
        if self.selected.is_empty() || self.section != Section::Home {
            return;
        }
        let t = self.t();
        let ids: Vec<u64> = self.selected.iter().copied().collect();
        let first = self.task(ids[0]).cloned();
        egui::Area::new(egui::Id::new("bulk_bar")).order(egui::Order::Foreground).anchor(Align2::CENTER_BOTTOM, [0.0, -22.0]).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgb(32, 32, 36)).corner_radius(10).inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(52, 52, 58);
                    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(72, 72, 80);
                    ui.label(RichText::new(if ids.len() == 1 { trf!("{} task selected", ids.len()) } else { trf!("{} tasks selected", ids.len()) }).strong().color(Color32::WHITE));
                    if ui.small_button("\u{2716}").on_hover_text(tr("Clear selection")).clicked() {
                        self.selected.clear();
                    }
                    ui.separator();
                    let pid = ui.id();
                    let statuses = first.as_ref().map(|f| self.data.statuses_for_list(f.list_id)).unwrap_or_else(default_statuses);
                    let r = ui.button(tr("Status"));
                    let mut pick_status: Option<String> = None;
                    popup(ui, &r, pid.with("bs"), 180.0, |ui| {
                        for st in &statuses {
                            if menu_row(ui, &t, &crate::i18n::status_text(&st.name).to_uppercase(), false, |p, c| paint_status_circle(p, c, 6.0, st)).clicked() {
                                pick_status = Some(st.name.clone());
                                close_popup(ui);
                            }
                        }
                    });
                    let r = ui.button(tr("Assignees"));
                    let mut pick_asg: Option<u64> = None;
                    let mut drop_asg: Option<u64> = None;
                    let mut unassign = false;
                    let members = self.data.members.clone();
                    let sel_tasks: Vec<Task> = ids.iter().filter_map(|id| self.task(*id).cloned()).collect();
                    popup(ui, &r, pid.with("ba"), 200.0, |ui| {
                        // ticked when every selected task has them: a click then removes
                        for m in &members {
                            let ring = t.card;
                            let all = sel_tasks.iter().all(|x| x.assignees.contains(&m.id));
                            if menu_row(ui, &t, &m.name, all, |p, c| paint_avatar(p, c, 9.0, m, ring)).clicked() {
                                if all {
                                    drop_asg = Some(m.id);
                                } else {
                                    pick_asg = Some(m.id);
                                }
                                close_popup(ui);
                            }
                        }
                        ui.separator();
                        if menu_row(ui, &t, tr("Unassign all"), false, |_, _| {}).clicked() {
                            unassign = true;
                            close_popup(ui);
                        }
                    });
                    let r = ui.button(tr("Tags"));
                    let mut add_tags: Option<Vec<String>> = None;
                    let all_tags = self.all_tags();
                    let tag_before: Vec<String> = all_tags.iter().filter(|g| sel_tasks.iter().all(|x| x.tags.contains(g))).cloned().collect();
                    popup(ui, &r, pid.with("bt"), 220.0, |ui| {
                        // tags every selected task has show ticked
                        let mut common = tag_before.clone();
                        if super::listview::tag_menu(ui, &t, &all_tags, &mut common) {
                            add_tags = Some(common);
                        }
                    });
                    let r = ui.button(tr("Due date"));
                    let mut due: Option<NaiveDate> = None;
                    let mut due_set = false;
                    popup(ui, &r, pid.with("bd"), 380.0, |ui| {
                        if date_menu(ui, &t, pid.with("bdc"), &mut due) {
                            due_set = true;
                        }
                    });
                    let r = ui.button(tr("Start date"));
                    let mut start: Option<NaiveDate> = None;
                    let mut start_set = false;
                    popup(ui, &r, pid.with("bsd"), 380.0, |ui| {
                        if date_menu(ui, &t, pid.with("bsdc"), &mut start) {
                            start_set = true;
                        }
                    });
                    let r = ui.button(tr("Repeat"));
                    let mut recur: Option<Option<String>> = None;
                    popup(ui, &r, pid.with("brc"), 160.0, |ui| {
                        for (v, label) in RECUR {
                            if menu_row(ui, &t, tr(label), false, |_, _| {}).clicked() {
                                recur = Some(if v.is_empty() { None } else { Some(v.to_string()) });
                                close_popup(ui);
                            }
                        }
                    });
                    let mut pick_pr: Option<u8> = None;
                    if self.data.extra.apps.priorities {
                        let r = ui.button(tr("Priority"));
                        popup(ui, &r, pid.with("bp"), 160.0, |ui| {
                            pick_pr = priority_pick(ui, &t, 0);
                        });
                    }
                    if ui.button(tr("Move")).clicked() {
                        self.modal = Some(Modal::MoveTasks { ids: ids.clone(), query: String::new() });
                    }
                    if ui.button(tr("Duplicate")).clicked() {
                        for id in &ids {
                            self.duplicate_task(*id);
                        }
                        self.toast = Some((trf!("Duplicated {} tasks", ids.len()), now_ts()));
                        self.selected.clear();
                    }
                    // archived tasks (Show archived tasks only) can be brought back
                    let all_archived = sel_tasks.iter().all(|x| x.archived);
                    if ui.button(tr(if all_archived { "Unarchive" } else { "Archive" })).clicked() {
                        self.set_archived(&ids, !all_archived);
                        if all_archived {
                            self.selected.clear();
                        }
                    }
                    if ui.button(RichText::new(tr("Delete")).color(Color32::from_rgb(255, 120, 120))).clicked() {
                        self.confirm_bulk_delete = true;
                    }
                    // apply picks
                    let mut edits: Vec<Task> = vec![];
                    for id in &ids {
                        let Some(mut e) = self.task(*id).cloned() else { continue };
                        let mut ch = false;
                        if let Some(s) = &pick_status {
                            if self.data.statuses_for_list(e.list_id).iter().any(|x| x.name == *s) {
                                e.status = s.clone();
                                ch = true;
                            }
                        }
                        if let Some(a) = pick_asg {
                            if !self.data.extra.apps.multiple_assignees {
                                if e.assignees != [a] {
                                    e.assignees = vec![a];
                                    ch = true;
                                }
                            } else if !e.assignees.contains(&a) {
                                e.assignees.push(a);
                                ch = true;
                            }
                        }
                        if let Some(a) = drop_asg {
                            if e.assignees.contains(&a) {
                                e.assignees.retain(|x| *x != a);
                                ch = true;
                            }
                        }
                        if unassign && !e.assignees.is_empty() {
                            e.assignees.clear();
                            ch = true;
                        }
                        if let Some(common) = &add_tags {
                            // tags ticked on are added to all, tags ticked off removed from all
                            for g in common {
                                if !e.tags.contains(g) {
                                    e.tags.push(g.clone());
                                    ch = true;
                                }
                            }
                            for g in tag_before.iter().filter(|g| !common.contains(g)) {
                                if e.tags.contains(g) {
                                    e.tags.retain(|x| x != g);
                                    ch = true;
                                }
                            }
                        }
                        if start_set {
                            e.start = start;
                            ch = true;
                        }
                        if let Some(r) = &recur {
                            e.recur = r.clone();
                            if e.recur.is_some() && e.due.is_none() {
                                e.due = Some(today());
                            }
                            ch = true;
                        }
                        if due_set {
                            e.due = due;
                            ch = true;
                        }
                        if let Some(p) = pick_pr {
                            e.priority = p;
                            ch = true;
                        }
                        if ch {
                            edits.push(e);
                        }
                    }
                    for e in edits {
                        self.actions.push(Action::Replace(e));
                    }
                });
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.modal.is_none() && self.open_task.is_none() && !self.menu_was_open {
            self.selected.clear();
        }
    }

    /// The selection box at the left of a list row; returns true when clicked.
    pub(super) fn row_checkbox(&mut self, ui: &mut Ui, row: Rect, tid: u64, hovered: bool) -> bool {
        let t = self.t();
        let on = self.selected.contains(&tid);
        let br = Rect::from_center_size(pos2(row.left() + 2.0, row.center().y), vec2(14.0, 14.0));
        let resp = ui.interact(br.expand(3.0), ui.id().with(("sel", tid)), Sense::click());
        let tname = self.task(tid).map(|t| t.name.clone()).unwrap_or_default();
        name_it(&resp, &format!("Select {}", tname));
        if on || hovered || resp.hovered() || !self.selected.is_empty() {
            if on {
                ui.painter().rect_filled(br, 3.0, t.primary);
                ui.painter().line_segment([br.left_center() + vec2(3.0, 0.0), br.center_bottom() + vec2(-1.0, -3.5)], Stroke::new(1.6, Color32::WHITE));
                ui.painter().line_segment([br.center_bottom() + vec2(-1.0, -3.5), br.right_top() + vec2(-3.0, 3.5)], Stroke::new(1.6, Color32::WHITE));
            } else {
                ui.painter().rect_stroke(br, 3.0, Stroke::new(1.2, t.text3), egui::StrokeKind::Inside);
            }
        }
        if resp.clicked() {
            if ui.input(|i| i.modifiers.shift) {
                self.select_click(tid, true);
            } else {
                if on {
                    self.selected.remove(&tid);
                } else {
                    self.selected.insert(tid);
                }
                self.select_anchor = Some(tid);
            }
            return true;
        }
        false
    }

    // ---------------------------------------------------------------- recurring

    /// When a repeating task is closed it opens again with its dates moved on.
    pub(super) fn roll_recurring(&self, old: &Task, new: &mut Task) {
        let Some(r) = new.recur.clone() else { return };
        let closes = |t: &Task| self.data.statuses_for_list(t.list_id).iter().any(|s| s.name == t.status && matches!(s.kind, StatusKind::Closed | StatusKind::Done));
        if closes(old) || !closes(new) {
            return;
        }
        let first = self.data.statuses_for_list(new.list_id)[0].name.clone();
        let base = new.due.unwrap_or_else(today);
        let next = next_recur(base, &r);
        let shift = next - base;
        new.due = Some(next);
        if let Some(s) = new.start {
            new.start = Some(s + shift);
        }
        new.status = first;
        // it's open again: no Date done / Date closed until it next closes
        new.done_at = None;
        new.closed_at = None;
        new.activity.push(Activity { ts: now_ts(), author: self.data.me, text: format!("completed a {} repeat; next due {}", r, crate::i18n::in_english(|| fmt_date(next))), comment: false, ..Default::default() });
        for c in &mut new.checklist {
            c.done = false;
        }
    }

    // ---------------------------------------------------------------- recent

    /// Remembers what was opened (tasks, lists, docs) for the Recent card.
    pub(super) fn note_recent(&mut self) {
        let key = if let Some(id) = self.open_task {
            format!("task:{}", id)
        } else if self.section == Section::Docs && self.doc_open.is_some() {
            format!("doc:{}", self.doc_open.unwrap_or(0))
        } else if let (Section::Home, Location::List(id)) = (self.section, self.loc) {
            format!("list:{}", id)
        } else {
            return;
        };
        if self.data.extra.recent.first() == Some(&key) {
            return;
        }
        self.data.extra.recent.retain(|k| *k != key);
        self.data.extra.recent.insert(0, key);
        self.data.extra.recent.truncate(20);
        self.dirty = true;
    }

    /// Recent items that still exist (and sit inside `lists` when given).
    pub(super) fn recent_items(&self, lists: Option<&[u64]>) -> Vec<(String, u64, String, String)> {
        let mut out = vec![];
        for k in &self.data.extra.recent {
            let Some((kind, id)) = k.split_once(':') else { continue };
            let Ok(id) = id.parse::<u64>() else { continue };
            match kind {
                "task" => {
                    if let Some(t) = self.task(id) {
                        if lists.map_or(true, |l| l.contains(&t.list_id)) {
                            out.push(("task".into(), id, t.name.clone(), self.data.list_name(t.list_id)));
                        }
                    }
                }
                "list" => {
                    if self.data.location_exists(Location::List(id)) && lists.map_or(true, |l| l.contains(&id)) {
                        let sp = self.data.space_of_list(id).map(|s| s.name.clone()).unwrap_or_default();
                        out.push(("list".into(), id, self.data.list_name(id), sp));
                    }
                }
                "doc" => {
                    if let Some(d) = self.data.docs.iter().find(|d| d.id == id) {
                        let space_ok = match (lists, d.space) {
                            (None, _) => true,
                            (Some(l), Some(sp)) => self.data.lists_in(Location::Space(sp)).map_or(false, |sl| sl.iter().any(|x| l.contains(x))),
                            _ => false,
                        };
                        if space_ok {
                            out.push(("doc".into(), id, d.title.clone(), tr("Doc").into()));

                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    // ---------------------------------------------------------------- shortcuts

    /// Single-key shortcuts (only while no text field has focus):
    /// T new task, R reminder, N or P notepad, Cmd+\ sidebar, Cmd+[ / ] history.
    pub(super) fn single_key_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Backslash)) {
            self.sidebar_collapsed = !self.sidebar_collapsed;
        }
        // Cmd+, Settings
        if self.modal.is_none() && self.palette.is_none() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.open_settings("My Settings");
        }
        let typing = ctx.memory(|m| m.focused().is_some()) || ctx.wants_keyboard_input();
        if typing || self.modal.is_some() || self.quick_add.is_some() || self.palette.is_some() || self.rename.is_some() {
            return;
        }
        // Cmd+[ / Cmd+]: back / forward, like the top bar arrows
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::OpenBracket)) {
            self.go_back();
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::CloseBracket)) {
            self.go_forward();
            return;
        }
        let plain = ctx.input(|i| !i.modifiers.command && !i.modifiers.alt && !i.modifiers.ctrl);
        if !plain {
            return;
        }
        if self.open_task.is_none() {
            if ctx.input(|i| i.key_pressed(egui::Key::T)) {
                self.open_quick_add(None);
            } else if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.modal = Some(Modal::Reminder { text: String::new(), date: Some(today()), time: "09:00".into() });
            } else if ctx.input(|i| i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::P)) {
                self.notepad_open = !self.notepad_open;
            }
        }
    }

    /// The tasks around the open one, in the order the view shows them.
    pub(super) fn neighbours(&self, tid: u64) -> (Option<u64>, Option<u64>) {
        let ids: Vec<u64> = self.filtered().iter().map(|t| t.id).collect();
        let Some(i) = ids.iter().position(|x| *x == tid) else { return (None, None) };
        (if i > 0 { Some(ids[i - 1]) } else { None }, ids.get(i + 1).copied())
    }
}

fn shell_checklist_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    let r = Rect::from_center_size(c, vec2(12.0, 12.0));
    p.rect_stroke(r, 2.0, Stroke::new(1.2, color), egui::StrokeKind::Inside);
    p.line_segment([r.left_center() + vec2(3.0, 0.0), r.right_center() - vec2(3.0, 0.0)], Stroke::new(1.0, color));
}
