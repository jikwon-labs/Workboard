// Moving, duplicating, archiving and reordering tasks, Lists, Folders and
// Spaces, with the Move dialogs.

use super::*;

// ---------------------------------------------------------------- move / duplicate

fn shell_checklist_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    let r = Rect::from_center_size(c, vec2(12.0, 12.0));
    p.rect_stroke(
        r,
        2.0,
        Stroke::new(1.2_f32, color),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [
            r.left_center() + vec2(3.0, 0.0),
            r.right_center() - vec2(3.0, 0.0),
        ],
        Stroke::new(1.0_f32, color),
    );
}

impl App {
    /// Moves tasks (and their subtasks) to another list; statuses that list
    /// does not have fall back to its first status.
    pub(crate) fn move_tasks(&mut self, ids: &[u64], list: u64) {
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
            all.extend(
                self.data
                    .tasks
                    .iter()
                    .filter(|t| t.parent == Some(k) && !all.contains(&t.id))
                    .map(|t| t.id)
                    .collect::<Vec<_>>(),
            );
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
                t.activity.push(Activity {
                    ts: now_ts(),
                    author: me,
                    text: format!("moved this task to {}", lname),
                    comment: false,
                    ..Default::default()
                });
                // a moved subtask becomes a task of its own in the new list
                if t.parent.map_or(false, |p| !all.contains(&p)) {
                    t.parent = None;
                }
            }
            t.updated = now_ts();
        }
        self.dirty = true;
        self.toast(if ids.len() == 1 {
            trf!("Moved to {}", lname)
        } else {
            trf!("Moved {} tasks to {}", ids.len(), lname)
        });
    }

    /// Copies a task with its subtasks; returns the copy's id.
    pub(crate) fn duplicate_task(&mut self, id: u64) -> Option<u64> {
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
            t.activity = vec![Activity {
                ts: now,
                author: app.data.me,
                text: format!("duplicated this task from {}", src.name),
                comment: false,
                ..Default::default()
            }];
            // the copy's assignees hear about it like a new assignment
            for &a in src.assignees.iter().filter(|a| **a != app.data.me) {
                let line = app.data.about_line(app.data.me, "assigned to", a);
                t.activity.push(line);
            }
            t.order = src.order;
            app.data.tasks.push(t);
            let subs: Vec<u64> = app
                .data
                .tasks
                .iter()
                .filter(|x| x.parent == Some(id))
                .map(|x| x.id)
                .collect();
            for s in subs {
                copy(app, s, Some(nid), false);
            }
            Some(nid)
        }
        let nid = copy(self, id, self.task(id)?.parent, true);
        self.dirty = true;
        nid
    }

    pub(crate) fn set_archived(&mut self, ids: &[u64], archived: bool) {
        let me = self.data.me;
        for t in self.data.tasks.iter_mut().filter(|t| ids.contains(&t.id)) {
            t.archived = archived;
            t.activity.push(Activity {
                ts: now_ts(),
                author: me,
                text: if archived {
                    "archived this task".into()
                } else {
                    "unarchived this task".into()
                },
                comment: false,
                ..Default::default()
            });
        }
        if archived && self.open_task.map_or(false, |o| ids.contains(&o)) {
            self.open_task = None;
        }
        self.selected.retain(|s| !ids.contains(s));
        self.dirty = true;
        self.toast(if !archived {
            tr("Unarchived").into()
        } else if ids.len() == 1 {
            trf!("Archived {} task", ids.len())
        } else {
            trf!("Archived {} tasks", ids.len())
        });
    }

    /// Moves a list to another space or folder.
    pub(crate) fn move_list(&mut self, list: u64, space: u64, folder: Option<u64>) {
        let Some(item) = self
            .data
            .spaces
            .iter()
            .flat_map(|s| {
                s.lists
                    .iter()
                    .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
            })
            .find(|l| l.id == list)
            .cloned()
        else {
            return;
        };
        if !self.data.spaces.iter().any(|s| s.id == space) {
            return;
        }
        for s in &mut self.data.spaces {
            s.lists.retain(|l| l.id != list);
            for f in &mut s.folders {
                f.lists.retain(|l| l.id != list);
            }
        }
        let statuses = self
            .data
            .spaces
            .iter()
            .find(|s| s.id == space)
            .map(|s| s.statuses.clone())
            .unwrap_or_else(default_statuses);
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
        self.toast(trf!(
            "Moved {} to {}",
            item.name,
            self.data.location_name(
                folder
                    .map(Location::Folder)
                    .unwrap_or(Location::Space(space))
            )
        ));
        self.dirty = true;
    }

    /// Copies a list and its tasks next to the original.
    pub(crate) fn duplicate_list(&mut self, list: u64) -> Option<u64> {
        let name = format!("{} (copy)", self.data.list_name(list));
        let nid = self.data.new_id();
        // the copy keeps the list's details (colour, dates, priority, owner, description)
        let src = self.data.list_item(list).cloned().unwrap_or_default();
        let mut placed = false;
        for s in &mut self.data.spaces {
            if let Some(i) = s.lists.iter().position(|l| l.id == list) {
                s.lists.insert(
                    i + 1,
                    ListItem {
                        id: nid,
                        name: name.clone(),
                        archived: false,
                        ..src.clone()
                    },
                );
                placed = true;
            }
            for f in &mut s.folders {
                if let Some(i) = f.lists.iter().position(|l| l.id == list) {
                    f.lists.insert(
                        i + 1,
                        ListItem {
                            id: nid,
                            name: name.clone(),
                            archived: false,
                            ..src.clone()
                        },
                    );
                    placed = true;
                }
            }
        }
        if !placed {
            return None;
        }
        let tops: Vec<u64> = self
            .data
            .tasks
            .iter()
            .filter(|t| t.list_id == list && t.parent.is_none() && !t.archived)
            .map(|t| t.id)
            .collect();
        for id in tops {
            if let Some(c) = self.duplicate_task(id) {
                let name = self.task(id).map(|t| t.name.clone()).unwrap_or_default();
                let mut kids = vec![c];
                let mut i = 0;
                while i < kids.len() {
                    let k = kids[i];
                    kids.extend(
                        self.data
                            .tasks
                            .iter()
                            .filter(|t| t.parent == Some(k))
                            .map(|t| t.id)
                            .collect::<Vec<_>>(),
                    );
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

    /// Every list as (id, "Space / Folder / List"), personal space included;
    /// archived ones left out (tasks moved there would drop out of sight).
    pub(crate) fn all_list_paths(&self) -> Vec<(u64, String)> {
        let me = self.data.me;
        self.data
            .list_places()
            .filter(|p| !p.others_private(me) && !p.archived())
            .map(|p| (p.list.id, p.path()))
            .collect()
    }

    pub(crate) fn move_tasks_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        ids: &[u64],
        query: &mut String,
    ) -> bool {
        let mut close = false;
        let here: Vec<u64> = ids
            .iter()
            .filter_map(|i| self.task(*i))
            .map(|t| t.list_id)
            .collect();
        ui.label(
            RichText::new(if ids.len() == 1 {
                tr("Move this task to another List.").to_string()
            } else {
                trf!("Move {} tasks to another List.", ids.len())
            })
            .color(t.text2),
        );
        let r = ui.add(
            field(query)
                .hint_text(tr("Search Lists..."))
                .desired_width(f32::INFINITY),
        );
        focus_once(ui, &r);
        ui.add_space(4.0);
        let q = query.to_lowercase();
        let paths = self.all_list_paths();
        // only where the tasks already are: say there is nowhere else yet
        if paths
            .iter()
            .all(|(l, _)| !here.is_empty() && here.iter().all(|h| h == l))
        {
            ui.label(
                RichText::new(tr("There is no other List to move to yet. Make one first."))
                    .color(t.text3),
            );
        }
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (lid, path) in paths {
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

    pub(crate) fn move_list_body(&mut self, ui: &mut Ui, t: &Theme, list: u64) -> bool {
        let mut close = false;
        ui.label(RichText::new(trf!("Move \"{}\" to:", self.data.list_name(list))).color(t.text2));
        ui.add_space(4.0);
        let spaces: Vec<(u64, String, Vec<(u64, String)>)> = self
            .open_spaces()
            .iter()
            .map(|s| {
                (
                    s.id,
                    s.name.clone(),
                    s.folders
                        .iter()
                        .filter(|f| !f.archived)
                        .map(|f| (f.id, f.name.clone()))
                        .collect(),
                )
            })
            .collect();
        let (cur_space, cur_folder) = {
            let sp = self.data.space_of_list(list).map(|s| s.id).unwrap_or(0);
            let f = self
                .data
                .spaces
                .iter()
                .flat_map(|s| s.folders.iter())
                .find(|f| f.lists.iter().any(|l| l.id == list))
                .map(|f| f.id);
            (sp, f)
        };
        // only where the List already is: say there is nowhere else yet
        let elsewhere = spaces.iter().any(|(sid, _, folders)| {
            !(*sid == cur_space && cur_folder.is_none())
                || folders.iter().any(|(fid, _)| cur_folder != Some(*fid))
        });
        if !elsewhere {
            ui.label(
                RichText::new(tr(
                    "There is no other Space or Folder to move it to yet. Make one first.",
                ))
                .color(t.text3),
            );
        }
        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                for (sid, sname, folders) in &spaces {
                    let cur = *sid == cur_space && cur_folder.is_none();
                    // a Space by its colour and letter, its Folders under it,
                    // as in the sidebar
                    let color = self
                        .data
                        .spaces
                        .iter()
                        .find(|s| s.id == *sid)
                        .map(|s| rgb(s.color))
                        .unwrap_or(t.text3);
                    let letter = sname
                        .chars()
                        .next()
                        .unwrap_or('S')
                        .to_uppercase()
                        .to_string();
                    let space_lead = |p: &egui::Painter, c: Pos2| {
                        let r = Rect::from_center_size(c, vec2(16.0, 16.0));
                        p.rect_filled(r, 4.0, color);
                        p.text(
                            c,
                            Align2::CENTER_CENTER,
                            &letter,
                            FontId::proportional(10.0),
                            Color32::WHITE,
                        );
                    };
                    if menu_row(ui, t, sname, cur, space_lead).clicked() && !cur {
                        self.move_list(list, *sid, None);
                        close = true;
                    }
                    for (fid, fname) in folders {
                        let cur = cur_folder == Some(*fid);
                        if menu_row_named(ui, t, fname, &format!("      {}", fname), cur, |p, c| {
                            super::icons::line_icon(p, "folder", c + vec2(18.0, 0.0), t.text2);
                        })
                        .clicked()
                            && !cur
                        {
                            self.move_list(list, *sid, Some(*fid));
                            close = true;
                        }
                    }
                }
            });
        close
    }

    pub(crate) fn set_folder_archived(&mut self, fid: u64, archived: bool) {
        let mut name = String::new();
        let mut lists = vec![];
        for f in self
            .data
            .spaces
            .iter_mut()
            .flat_map(|s| s.folders.iter_mut())
            .filter(|f| f.id == fid)
        {
            f.archived = archived;
            name = f.name.clone();
            lists = f.lists.iter().map(|l| l.id).collect();
            self.dirty = true;
        }
        let here = self.loc == Location::Folder(fid)
            || matches!(self.loc, Location::List(l) if lists.contains(&l));
        if archived && here {
            self.loc = Location::Home;
        }
        let msg = if archived {
            trf!("Archived \"{}\". Restore it from Settings > Spaces.", name)
        } else {
            trf!("Restored \"{}\".", name)
        };
        self.toast(msg);
    }

    /// Moves a folder, with its lists, to the end of another space.
    pub(crate) fn move_folder(&mut self, fid: u64, space: u64) {
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
        let statuses = self
            .data
            .spaces
            .iter()
            .find(|s| s.id == space)
            .map(|s| s.statuses.clone())
            .unwrap_or_else(default_statuses);
        if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == space) {
            s.expanded = true;
            s.folders.push(f);
        }
        // tasks take the new space's statuses
        for t in self
            .data
            .tasks
            .iter_mut()
            .filter(|t| lists.contains(&t.list_id))
        {
            if !statuses.iter().any(|s| s.name == t.status) {
                t.status = statuses[0].name.clone();
            }
        }
        self.toast(trf!(
            "Moved {} to {}",
            name,
            self.data.location_name(Location::Space(space))
        ));
        self.dirty = true;
    }

    /// A copy of a folder next to it, with copies of its lists and their tasks.
    pub(crate) fn duplicate_folder(&mut self, fid: u64) -> Option<u64> {
        let (sid, src) = self.data.spaces.iter().find_map(|s| {
            s.folders
                .iter()
                .find(|f| f.id == fid)
                .map(|f| (s.id, f.clone()))
        })?;
        let nid = self.data.new_id();
        let mut copy = Folder {
            id: nid,
            name: format!("{} (copy)", src.name),
            lists: vec![],
            expanded: true,
            archived: false,
        };
        for l in src.lists.iter().filter(|l| !l.archived) {
            if let Some(n) = self.duplicate_list(l.id) {
                if let Some(item) = self.take_list(n) {
                    copy.lists.push(ListItem {
                        name: l.name.clone(),
                        ..item
                    });
                }
            }
        }
        let s = self.data.spaces.iter_mut().find(|s| s.id == sid)?;
        let pos = s
            .folders
            .iter()
            .position(|f| f.id == fid)
            .map(|i| i + 1)
            .unwrap_or(s.folders.len());
        s.folders.insert(pos, copy);
        self.dirty = true;
        Some(nid)
    }

    /// Puts a list just before another one (sidebar drag and drop), in the
    /// other one's space or folder.
    pub(crate) fn place_list_before(&mut self, list: u64, before: u64) {
        if list == before || self.data.list_item(before).is_none() {
            return;
        }
        let Some(item) = self.take_list(list) else {
            return;
        };
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
        let statuses = space
            .and_then(|sid| self.data.spaces.iter().find(|s| s.id == sid))
            .map(|s| s.statuses.clone())
            .unwrap_or_else(default_statuses);
        for t in self.data.tasks.iter_mut().filter(|t| t.list_id == list) {
            if !statuses.iter().any(|s| s.name == t.status) {
                t.status = statuses[0].name.clone();
            }
        }
        self.dirty = true;
    }

    /// Puts a space just before another one (sidebar drag and drop).
    pub(crate) fn place_space_before(&mut self, space: u64, before: u64) {
        if space == before || !self.data.spaces.iter().any(|s| s.id == before) {
            return;
        }
        let Some(i) = self.data.spaces.iter().position(|s| s.id == space) else {
            return;
        };
        let s = self.data.spaces.remove(i);
        let j = self
            .data
            .spaces
            .iter()
            .position(|s| s.id == before)
            .unwrap_or(self.data.spaces.len());
        self.data.spaces.insert(j, s);
        self.dirty = true;
    }

    pub(crate) fn set_list_archived(&mut self, lid: u64, archived: bool) {
        let name = self.data.list_name(lid);
        if let Some(l) = self.data.list_item_mut(lid) {
            l.archived = archived;
            self.dirty = true;
        }
        if archived && self.loc == Location::List(lid) {
            self.loc = Location::Home;
        }
        let msg = if archived {
            trf!("Archived \"{}\". Restore it from Settings > Spaces.", name)
        } else {
            trf!("Restored \"{}\".", name)
        };
        self.toast(msg);
    }

    pub(crate) fn set_space_archived(&mut self, sid: u64, archived: bool) {
        let lists = self.data.lists_in(Location::Space(sid)).unwrap_or_default();
        if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
            s.archived = archived;
            self.dirty = true;
        }
        let here = self.loc == Location::Space(sid)
            || matches!(self.loc, Location::List(l) if lists.contains(&l))
            || matches!(self.loc, Location::Folder(f) if self.data.spaces.iter().any(|s| s.id == sid && s.folders.iter().any(|x| x.id == f)));
        if archived && here {
            self.loc = Location::Home;
        }
    }

    /// A copy of the space: statuses, folders and lists, with their tasks.
    pub(crate) fn duplicate_space(&mut self, sid: u64) -> Option<u64> {
        let src = self.data.spaces.iter().find(|s| s.id == sid)?.clone();
        let nid = self.data.new_id();
        let mut copy = Space {
            id: nid,
            name: format!("{} (copy)", src.name),
            color: src.color,
            statuses: src.statuses.clone(),
            folders: vec![],
            lists: vec![],
            expanded: true,
            private_owner: None,
            icon: src.icon.clone(),
            ..Default::default()
        };
        let pos = self
            .data
            .spaces
            .iter()
            .position(|s| s.id == sid)
            .map(|i| i + 1)
            .unwrap_or(self.data.spaces.len());
        self.data.spaces.insert(pos, copy.clone());
        // copy each list with duplicate_list (it lands next to the original), then move it over
        let mut moved: Vec<(Option<usize>, ListItem)> = vec![];
        let plain: Vec<u64> = src
            .lists
            .iter()
            .filter(|l| !l.archived)
            .map(|l| l.id)
            .collect();
        for l in plain {
            if let Some(n) = self.duplicate_list(l) {
                if let Some(item) = self.take_list(n) {
                    moved.push((
                        None,
                        ListItem {
                            name: self.data.list_name(l),
                            ..item
                        },
                    ));
                }
            }
        }
        for (fi, f) in src.folders.iter().enumerate() {
            copy.folders.push(Folder {
                id: self.data.new_id(),
                name: f.name.clone(),
                lists: vec![],
                expanded: true,
                archived: false,
            });
            for l in f.lists.iter().filter(|l| !l.archived) {
                if let Some(n) = self.duplicate_list(l.id) {
                    if let Some(item) = self.take_list(n) {
                        moved.push((
                            Some(fi),
                            ListItem {
                                name: l.name.clone(),
                                ..item
                            },
                        ));
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
}
