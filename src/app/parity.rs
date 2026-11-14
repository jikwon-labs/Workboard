// Features that bring the app closer to the original: create dialogs for
// lists and spaces, moving / duplicating / archiving tasks and lists, the
// full task menu, selecting several tasks at once, condition filters,
// choosing columns, recurring tasks and single-key shortcuts.

use super::*;
use egui::Pos2;

pub const DUE_FILTERS: [&str; 6] = ["Any time", "Overdue", "Today", "This week", "Next week", "No due date"];
pub const COLUMNS: [&str; 6] = ["Assignee", "Due date", "Priority", "Status", "Comments", "Date created"];

/// Field conditions from the Filter panel (all must match).
#[derive(Clone, Default, PartialEq)]
pub struct TaskFilter {
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
        [!self.statuses.is_empty(), !self.priorities.is_empty(), !self.assignees.is_empty() || self.unassigned, !self.tags.is_empty(), self.due != 0, self.archived].iter().filter(|x| **x).count()
    }

    pub fn matches(&self, t: &Task, data: &Data) -> bool {
        if t.archived != self.archived {
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
        let td = today();
        let week_start = td - Duration::days(td.weekday().num_days_from_monday() as i64);
        match self.due {
            1 => t.due.map_or(false, |d| d < td) && !data.is_closed(t),
            2 => t.due == Some(td),
            3 => t.due.map_or(false, |d| d >= week_start && d < week_start + Duration::days(7)),
            4 => t.due.map_or(false, |d| d >= week_start + Duration::days(7) && d < week_start + Duration::days(14)),
            5 => t.due.is_none(),
            _ => true,
        }
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
        self.data.spaces.iter().filter(|s| s.private_owner.is_none()).collect()
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
        let item = ListItem { id, name };
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
        self.data.spaces.push(Space { id, name, color, statuses: default_statuses(), folders: vec![], lists: vec![ListItem { id: lid, name: "List".into() }], expanded: true, private_owner: None });
        self.section = Section::Home;
        self.loc = Location::Space(id);
        self.view = ViewKind::Overview;
        self.dirty = true;
        id
    }

    pub(super) fn new_list_body(&mut self, ui: &mut Ui, t: &Theme, name: &mut String, space: &mut u64, folder: &mut Option<u64>) -> bool {
        let mut close = false;
        ui.label(RichText::new("All Lists are located within a Space. Lists can house any type of task.").color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new("Name").color(t.text2));
        let r = ui.add(egui::TextEdit::singleline(name).hint_text("e.g. Project, List of items, Campaign").desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
        focus_once(ui, &r);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(8.0);
        ui.label(RichText::new("Space (location)").color(t.text2));
        let spaces: Vec<(u64, String, Vec<(u64, String)>)> = self.open_spaces().iter().map(|s| (s.id, s.name.clone(), s.folders.iter().map(|f| (f.id, f.name.clone())).collect())).collect();
        let cur = spaces.iter().find(|s| s.0 == *space).map(|s| match folder.and_then(|f| s.2.iter().find(|x| x.0 == f)) {
            Some(f) => format!("{} / {}", s.1, f.1),
            None => s.1.clone(),
        });
        egui::ComboBox::from_id_salt("new_list_space").width(ui.available_width() - 10.0).selected_text(cur.unwrap_or_else(|| "Choose a Space".into())).show_ui(ui, |ui| {
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
                if (ui.add_enabled(ok, egui::Button::new(RichText::new("Create").color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(90.0, 32.0))).clicked() || (enter && ok)) && self.create_list(*space, *folder, name).is_some() {
                    close = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        close
    }

    pub(super) fn new_space_body(&mut self, ui: &mut Ui, t: &Theme, name: &mut String, color: &mut [u8; 3]) -> bool {
        let mut close = false;
        ui.label(RichText::new("A Space represents teams, departments, or groups, each with its own Lists, workflows, and settings.").color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new("Icon & name").color(t.text2));
        let mut enter = false;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
            ui.painter().rect_filled(r, 7.0, rgb(*color));
            let letter = name.chars().next().unwrap_or('S').to_uppercase().to_string();
            ui.painter().text(r.center(), Align2::CENTER_CENTER, letter, FontId::proportional(15.0), Color32::WHITE);
            let te = ui.add(egui::TextEdit::singleline(name).hint_text("e.g. Marketing, Engineering, HR").desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
            focus_once(ui, &te);
            enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Color").color(t.text2));
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
                if primary_button(ui, t, "Continue").clicked() || enter {
                    self.create_space(name, *color);
                    close = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        close
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
                t.activity.push(Activity { ts: now_ts(), author: me, text: format!("moved this task to {}", lname), comment: false });
                // a moved subtask becomes a task of its own in the new list
                if t.parent.map_or(false, |p| !all.contains(&p)) {
                    t.parent = None;
                }
            }
            t.updated = now_ts();
        }
        self.dirty = true;
        self.toast = Some((if ids.len() == 1 { format!("Moved to {}", lname) } else { format!("Moved {} tasks to {}", ids.len(), lname) }, now_ts()));
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
            t.time_entries.clear();
            t.activity = vec![Activity { ts: now, author: app.data.me, text: format!("duplicated this task from {}", src.name), comment: false }];
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
            t.activity.push(Activity { ts: now_ts(), author: me, text: if archived { "archived this task".into() } else { "unarchived this task".into() }, comment: false });
        }
        if archived && self.open_task.map_or(false, |o| ids.contains(&o)) {
            self.open_task = None;
        }
        self.selected.retain(|s| !ids.contains(s));
        self.dirty = true;
        self.toast = Some((if archived { format!("Archived {} task{}", ids.len(), if ids.len() == 1 { "" } else { "s" }) } else { "Unarchived".into() }, now_ts()));
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
        self.toast = Some((format!("Moved {} to {}", item.name, self.data.location_name(folder.map(Location::Folder).unwrap_or(Location::Space(space)))), now_ts()));
        self.dirty = true;
    }

    /// Copies a list and its tasks next to the original.
    pub(super) fn duplicate_list(&mut self, list: u64) -> Option<u64> {
        let name = format!("{} (copy)", self.data.list_name(list));
        let nid = self.data.new_id();
        let mut placed = false;
        for s in &mut self.data.spaces {
            if let Some(i) = s.lists.iter().position(|l| l.id == list) {
                s.lists.insert(i + 1, ListItem { id: nid, name: name.clone() });
                placed = true;
            }
            for f in &mut s.folders {
                if let Some(i) = f.lists.iter().position(|l| l.id == list) {
                    f.lists.insert(i + 1, ListItem { id: nid, name: name.clone() });
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
        ui.label(RichText::new(if ids.len() == 1 { "Move this task to another List.".to_string() } else { format!("Move {} tasks to another List.", ids.len()) }).color(t.text2));
        let r = ui.add(egui::TextEdit::singleline(query).hint_text("Search Lists...").desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
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
        ui.label(RichText::new(format!("Move \"{}\" to:", self.data.list_name(list))).color(t.text2));
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
            if ui.button("Copy link").clicked() {
                ui.ctx().copy_text(format!("workboard://task/{}", tid));
                self.toast = Some(("Link copied".into(), now_ts()));
                close_any(ui);
            }
            if ui.button("Copy ID").clicked() {
                ui.ctx().copy_text(format!("{}", tid));
                self.toast = Some(("Task ID copied".into(), now_ts()));
                close_any(ui);
            }
        });
        ui.separator();
        if ui.button("Open").clicked() {
            self.actions.push(Action::Open(tid));
            close_any(ui);
        }
        if ui.button("Rename").clicked() {
            self.actions.push(Action::StartRename(Target::Task(tid)));
            close_any(ui);
        }
        let selected = self.selected.contains(&tid);
        if ui.button(if selected { "Deselect" } else { "Select" }).clicked() {
            if selected {
                self.selected.remove(&tid);
            } else {
                self.selected.insert(tid);
            }
            close_any(ui);
        }
        ui.separator();
        let statuses = self.data.statuses_for_list(task.list_id);
        ui.menu_button("Status", |ui| {
            ui.set_max_width(200.0);
            for st in &statuses {
                if menu_row(ui, &t, &st.name.to_uppercase(), st.name == task.status, |p, c| paint_status_circle(p, c, 6.0, st)).clicked() {
                    let mut e = task.clone();
                    e.status = st.name.clone();
                    self.actions.push(Action::Replace(e));
                    close_any(ui);
                }
            }
        });
        if self.data.extra.apps.priorities {
            ui.menu_button("Priority", |ui| {
                ui.set_max_width(180.0);
                for pr in [1u8, 2, 3, 4, 0] {
                    let col = priority_color(pr, &t);
                    let label = if pr == 0 { "Clear" } else { PRIORITIES[pr as usize].label };
                    if menu_row(ui, &t, label, task.priority == pr, |p, c| paint_flag(p, c + vec2(-4.0, 0.0), col, 13.0)).clicked() {
                        let mut e = task.clone();
                        e.priority = pr;
                        self.actions.push(Action::Replace(e));
                        close_any(ui);
                    }
                }
            });
        }
        ui.menu_button("Assignees", |ui| {
            ui.set_max_width(220.0);
            let members = self.data.members.clone();
            for m in &members {
                let on = task.assignees.contains(&m.id);
                let ring = t.card;
                if menu_row(ui, &t, &m.name, on, |p, c| paint_avatar(p, c, 9.0, m, ring)).clicked() {
                    let mut e = task.clone();
                    if on {
                        e.assignees.retain(|a| *a != m.id);
                    } else {
                        e.assignees.push(m.id);
                    }
                    self.actions.push(Action::Replace(e));
                    close_any(ui);
                }
            }
        });
        ui.menu_button("Due date", |ui| {
            let td = today();
            let wd = td.weekday().num_days_from_monday() as i64;
            for (label, d) in [("Today", Some(td)), ("Tomorrow", Some(td + Duration::days(1))), ("Next week", Some(td + Duration::days(7 - wd))), ("2 weeks", Some(td + Duration::days(14))), ("Clear", None)] {
                if ui.button(label).clicked() {
                    let mut e = task.clone();
                    e.due = d;
                    self.actions.push(Action::Replace(e));
                    close_any(ui);
                }
            }
        });
        ui.menu_button("Repeat", |ui| {
            for (v, label) in RECUR {
                let sel = task.recur.as_deref().unwrap_or("") == v;
                if ui.selectable_label(sel, label).clicked() {
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
        if ui.button("Add subtask").clicked() {
            self.actions.push(Action::Add { list: task.list_id, name: String::new(), status: None, priority: 0, assignees: vec![], due: None, parent: Some(tid), open: true });
            close_any(ui);
        }
        if ui.button("Move to...").clicked() {
            self.modal = Some(Modal::MoveTasks { ids: vec![tid], query: String::new() });
            close_any(ui);
        }
        if ui.button("Duplicate").clicked() {
            if self.duplicate_task(tid).is_some() {
                self.toast = Some(("Task duplicated".into(), now_ts()));
            }
            close_any(ui);
        }
        if ui.button("Remind me").clicked() {
            self.modal = Some(Modal::Reminder { text: task.name.clone(), date: Some(today() + Duration::days(1)), time: "09:00".into() });
            close_any(ui);
        }
        if ui.button(if task.timer_started.is_some() { "Stop timer" } else { "Start timer" }).clicked() {
            self.actions.push(Action::ToggleTimer(tid));
            close_any(ui);
        }
        if ui.button("Save as template").clicked() {
            let id = self.data.new_id();
            self.data.extra.templates.push(TaskTemplate { id, name: task.name.clone(), task_name: task.name.clone(), description: task.description.clone(), checklist: task.checklist.iter().map(|c| c.text.clone()).collect(), tags: task.tags.clone(), priority: task.priority });
            self.toast = Some(("Saved as a template".into(), now_ts()));
            self.dirty = true;
            close_any(ui);
        }
        ui.separator();
        if ui.button(if task.archived { "Unarchive" } else { "Archive" }).clicked() {
            self.set_archived(&[tid], !task.archived);
            close_any(ui);
        }
        if ui.button(RichText::new("Delete").color(t.danger)).clicked() {
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
        if ui.button(fav).clicked() {
            self.actions.push(Action::ToggleFavorite(loc));
            close_any(ui);
        }
        if ui.button("Rename").clicked() {
            self.actions.push(Action::StartRename(Target::List(lid)));
            close_any(ui);
        }
        if ui.button("Copy link").clicked() {
            ui.ctx().copy_text(format!("workboard://list/{}", lid));
            self.toast = Some(("Link copied".into(), now_ts()));
            close_any(ui);
        }
        ui.separator();
        if ui.button("Create task").clicked() {
            self.loc = loc;
            self.section = Section::Home;
            self.open_quick_add(None);
            close_any(ui);
        }
        if let Some(sid) = self.data.space_of_list(lid).map(|s| s.id) {
            if ui.button("Task statuses").clicked() {
                self.actions.push(Action::EditStatuses(sid));
                close_any(ui);
            }
        }
        if ui.button("Custom Fields").clicked() {
            self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
            close_any(ui);
        }
        if ui.button("Automations").clicked() {
            self.modal = Some(Modal::Automations { status: "complete".into(), action: "comment".into(), value: String::new() });
            close_any(ui);
        }
        ui.separator();
        if ui.button("Move...").clicked() {
            self.modal = Some(Modal::MoveList { list: lid });
            close_any(ui);
        }
        if ui.button("Duplicate").clicked() {
            if let Some(n) = self.duplicate_list(lid) {
                self.section = Section::Home;
                self.loc = Location::List(n);
                self.toast = Some(("List duplicated".into(), now_ts()));
            }
            close_any(ui);
        }
        if ui.button(RichText::new("Delete").color(t.danger)).clicked() {
            self.actions.push(Action::AskDelete(Target::List(lid)));
            close_any(ui);
        }
    }

    /// The space menu (right click, or the "..." button on a sidebar space).
    pub(super) fn space_menu(&mut self, ui: &mut Ui, sid: u64) {
        let t = self.t();
        let loc = Location::Space(sid);
        ui.set_min_width(200.0);
        let fav = if self.data.favorites.contains(&loc) { "Remove from Favorites" } else { "Add to Favorites" };
        if ui.button(fav).clicked() {
            self.actions.push(Action::ToggleFavorite(loc));
            close_any(ui);
        }
        if ui.button("Rename").clicked() {
            self.actions.push(Action::StartRename(Target::Space(sid)));
            close_any(ui);
        }
        if ui.button("Copy link").clicked() {
            ui.ctx().copy_text(format!("workboard://space/{}", sid));
            self.toast = Some(("Link copied".into(), now_ts()));
            close_any(ui);
        }
        ui.menu_button("Create new", |ui| {
            if ui.button("List").clicked() {
                self.open_new_list(Some(sid), None);
                close_any(ui);
            }
            if ui.button("Folder").clicked() {
                self.actions.push(Action::NewFolder(sid));
                close_any(ui);
            }
            if ui.button("Doc").clicked() {
                self.new_doc(Some(sid));
                close_any(ui);
            }
        });
        ui.menu_button("Color", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(170.0);
                for c in STATUS_PALETTE.iter().skip(1) {
                    let (rr, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    ui.painter().circle_filled(rr.center(), 8.0, rgb(*c));
                    if resp.clicked() {
                        self.actions.push(Action::SetSpaceColor(sid, *c));
                        close_any(ui);
                    }
                }
            });
        });
        if ui.button("Task statuses").clicked() {
            self.actions.push(Action::EditStatuses(sid));
            close_any(ui);
        }
        if ui.button("Custom Fields").clicked() {
            self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
            close_any(ui);
        }
        if ui.button("Automations").clicked() {
            self.modal = Some(Modal::Automations { status: "complete".into(), action: "comment".into(), value: String::new() });
            close_any(ui);
        }
        ui.separator();
        if ui.button(RichText::new("Delete").color(t.danger)).clicked() {
            self.actions.push(Action::AskDelete(Target::Space(sid)));
            close_any(ui);
        }
    }

    // ---------------------------------------------------------------- toolbar panels

    pub(super) fn col_on(&self, c: &str) -> bool {
        !self.hidden_cols.contains(c)
    }

    pub(super) fn columns_panel(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.label(RichText::new("Shown in this view").small().color(t.text3));
        for c in COLUMNS {
            let mut on = self.col_on(c);
            if ui.checkbox(&mut on, c).changed() {
                if on {
                    self.hidden_cols.remove(c);
                } else {
                    self.hidden_cols.insert(c.to_string());
                }
            }
        }
        ui.separator();
        if ui.button("+ Create new field").clicked() {
            self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
            close_popup(ui);
        }
    }

    pub(super) fn filter_panel(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.set_width(300.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Filters").strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("Clear all").clicked() {
                    self.filter = Default::default();
                    self.search.clear();
                    self.me_mode = false;
                    self.assignee_filter = None;
                }
            });
        });
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Task name or tag...").desired_width(f32::INFINITY));
        ui.add_space(4.0);
        ui.label(RichText::new("Status").small().color(t.text3));
        ui.horizontal_wrapped(|ui| {
            for st in self.location_statuses() {
                let on = self.filter.statuses.contains(&st.name);
                if ui.selectable_label(on, RichText::new(st.name.to_uppercase()).color(rgb(st.color))).clicked() {
                    if on {
                        self.filter.statuses.retain(|s| *s != st.name);
                    } else {
                        self.filter.statuses.push(st.name.clone());
                    }
                }
            }
        });
        ui.label(RichText::new("Priority").small().color(t.text3));
        ui.horizontal_wrapped(|ui| {
            for pr in [1u8, 2, 3, 4, 0] {
                let on = self.filter.priorities.contains(&pr);
                let label = if pr == 0 { "None" } else { PRIORITIES[pr as usize].label };
                if ui.selectable_label(on, label).clicked() {
                    if on {
                        self.filter.priorities.retain(|p| *p != pr);
                    } else {
                        self.filter.priorities.push(pr);
                    }
                }
            }
        });
        ui.label(RichText::new("Assignee").small().color(t.text3));
        ui.horizontal_wrapped(|ui| {
            for m in self.data.members.clone() {
                let on = self.filter.assignees.contains(&m.id);
                if ui.selectable_label(on, &m.name).clicked() {
                    if on {
                        self.filter.assignees.retain(|a| *a != m.id);
                    } else {
                        self.filter.assignees.push(m.id);
                    }
                }
            }
            let on = self.filter.unassigned;
            if ui.selectable_label(on, "Unassigned").clicked() {
                self.filter.unassigned = !on;
            }
        });
        ui.label(RichText::new("Due date").small().color(t.text3));
        ui.horizontal_wrapped(|ui| {
            for (i, label) in DUE_FILTERS.iter().enumerate() {
                if ui.selectable_label(self.filter.due == i as u8, *label).clicked() {
                    self.filter.due = i as u8;
                }
            }
        });
        let mut tags: Vec<String> = self.data.tasks.iter().flat_map(|t| t.tags.iter().cloned()).collect();
        tags.sort();
        tags.dedup();
        if !tags.is_empty() {
            ui.label(RichText::new("Tags").small().color(t.text3));
            ui.horizontal_wrapped(|ui| {
                for g in tags {
                    let on = self.filter.tags.contains(&g);
                    if ui.selectable_label(on, RichText::new(&g).color(tag_color(&g))).clicked() {
                        if on {
                            self.filter.tags.retain(|x| *x != g);
                        } else {
                            self.filter.tags.push(g.clone());
                        }
                    }
                }
            });
        }
        ui.separator();
        ui.checkbox(&mut self.filter.archived, "Show archived tasks only");
        ui.checkbox(&mut self.me_mode, "Only my tasks");
    }

    // ---------------------------------------------------------------- bulk edit

    /// The bar along the bottom while tasks are selected.
    pub(super) fn bulk_bar(&mut self, ctx: &egui::Context) {
        self.selected.retain(|id| self.data.tasks.iter().any(|t| t.id == *id));
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
                    ui.label(RichText::new(format!("{} task{} selected", ids.len(), if ids.len() == 1 { "" } else { "s" })).strong().color(Color32::WHITE));
                    if ui.small_button("\u{2716}").on_hover_text("Clear selection").clicked() {
                        self.selected.clear();
                    }
                    ui.separator();
                    let pid = ui.id();
                    let statuses = first.as_ref().map(|f| self.data.statuses_for_list(f.list_id)).unwrap_or_else(default_statuses);
                    let r = ui.button("Status");
                    let mut pick_status: Option<String> = None;
                    popup(ui, &r, pid.with("bs"), 180.0, |ui| {
                        for st in &statuses {
                            if menu_row(ui, &t, &st.name.to_uppercase(), false, |p, c| paint_status_circle(p, c, 6.0, st)).clicked() {
                                pick_status = Some(st.name.clone());
                                close_popup(ui);
                            }
                        }
                    });
                    let r = ui.button("Assignees");
                    let mut pick_asg: Option<u64> = None;
                    let members = self.data.members.clone();
                    popup(ui, &r, pid.with("ba"), 200.0, |ui| {
                        for m in &members {
                            let ring = t.card;
                            if menu_row(ui, &t, &m.name, false, |p, c| paint_avatar(p, c, 9.0, m, ring)).clicked() {
                                pick_asg = Some(m.id);
                                close_popup(ui);
                            }
                        }
                    });
                    let r = ui.button("Due date");
                    let mut due: Option<NaiveDate> = None;
                    let mut due_set = false;
                    popup(ui, &r, pid.with("bd"), 380.0, |ui| {
                        if date_menu(ui, &t, pid.with("bdc"), &mut due) {
                            due_set = true;
                        }
                    });
                    let mut pick_pr: Option<u8> = None;
                    if self.data.extra.apps.priorities {
                        let r = ui.button("Priority");
                        popup(ui, &r, pid.with("bp"), 160.0, |ui| {
                            let mut p = 0u8;
                            if priority_menu(ui, &t, &mut p) || p == 0 {
                                pick_pr = Some(p);
                            }
                        });
                    }
                    if ui.button("Move").clicked() {
                        self.modal = Some(Modal::MoveTasks { ids: ids.clone(), query: String::new() });
                    }
                    if ui.button("Duplicate").clicked() {
                        for id in &ids {
                            self.duplicate_task(*id);
                        }
                        self.toast = Some((format!("Duplicated {} tasks", ids.len()), now_ts()));
                        self.selected.clear();
                    }
                    if ui.button("Archive").clicked() {
                        self.set_archived(&ids, true);
                    }
                    if ui.button(RichText::new("Delete").color(Color32::from_rgb(255, 120, 120))).clicked() {
                        for id in &ids {
                            self.actions.push(Action::DeleteTask(*id));
                        }
                        self.selected.clear();
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
                            if !e.assignees.contains(&a) {
                                e.assignees.push(a);
                                ch = true;
                            }
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
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.modal.is_none() && self.open_task.is_none() && !ctx.memory(|m| m.any_popup_open()) {
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
            if on {
                self.selected.remove(&tid);
            } else {
                self.selected.insert(tid);
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
        new.activity.push(Activity { ts: now_ts(), author: self.data.me, text: format!("completed a {} repeat; next due {}", r, fmt_date(next)), comment: false });
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
                            out.push(("doc".into(), id, d.title.clone(), "Doc".into()));
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
    /// T new task, R reminder, N or P notepad, Cmd+\ sidebar.
    pub(super) fn single_key_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Backslash)) {
            self.sidebar_collapsed = !self.sidebar_collapsed;
        }
        let typing = ctx.memory(|m| m.focused().is_some()) || ctx.wants_keyboard_input();
        if typing || self.modal.is_some() || self.quick_add.is_some() || self.palette.is_some() || self.rename.is_some() {
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
