// The right-click and "..." menus of a task, a List, a Folder and a Space,
// and the menu for adding to a List.

use super::*;

// ---------------------------------------------------------------- menus

impl App {
    /// The task menu (right click on a row or card, and the row's "..." button).
    pub(crate) fn task_menu(&mut self, ui: &mut Ui, tid: u64) {
        let t = self.t();
        let Some(task) = self.task(tid).cloned() else {
            return;
        };
        ui.set_min_width(220.0);
        ui.horizontal(|ui| {
            if ui.button(tr("Copy link")).clicked() {
                self.copy_link(ui, nav::task_link(tid));
                close_any(ui);
            }
            if ui.button(tr("Copy ID")).clicked() {
                ui.ctx().copy_text(format!("{}", tid));
                self.toast(tr("Task ID copied"));
                close_any(ui);
            }
        });
        ui.separator();
        if ui.button(tr("Open")).clicked() {
            self.actions.push(Action::Open(tid));
            close_any(ui);
        }
        if ui.button(tr("Rename")).clicked() {
            if self.view == ViewKind::List
                && self.open_task.is_none()
                && self.row.order.contains(&tid)
            {
                // a row on screen is renamed in place
                self.row.rename = Some((tid, task.name.clone()));
            } else {
                self.actions.push(Action::StartRename(Target::Task(tid)));
            }
            close_any(ui);
        }
        let star = format!("task:{}", tid);
        let fav = self.data.extra.starred.contains(&star);
        if ui
            .button(tr(if fav {
                "Remove from Favorites"
            } else {
                "Add to Favorites"
            }))
            .clicked()
        {
            if fav {
                self.data.extra.starred.retain(|s| *s != star);
            } else {
                self.data.extra.starred.push(star);
            }
            self.dirty = true;
            close_any(ui);
        }
        let selected = self.selected.contains(&tid);
        if ui
            .button(if selected {
                tr("Deselect")
            } else {
                tr("Select")
            })
            .clicked()
        {
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
                if menu_row(
                    ui,
                    &t,
                    &crate::i18n::status_text(&st.name).to_uppercase(),
                    st.name == task.status,
                    |p, c| paint_status_circle(p, c, 6.0, st),
                )
                .clicked()
                {
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
                    let label = if pr == 0 {
                        "Clear"
                    } else {
                        PRIORITIES[pr as usize].label
                    };
                    if menu_row(ui, &t, tr(label), task.priority == pr, |p, c| {
                        paint_flag(p, c + vec2(-4.0, 0.0), col, 13.0)
                    })
                    .clicked()
                    {
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
                if menu_row(ui, &t, &m.name, on, |p, c| paint_avatar(p, c, 9.0, m, ring)).clicked()
                {
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
            for (label, d) in [
                ("Today", Some(td)),
                ("Tomorrow", Some(td + Duration::days(1))),
                ("Next week", Some(td + Duration::days(7 - wd))),
                ("2 weeks", Some(td + Duration::days(14))),
                ("Clear", None),
            ] {
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
                    e.recur = if v.is_empty() {
                        None
                    } else {
                        Some(v.to_string())
                    };
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
            self.actions.push(Action::Add {
                list: task.list_id,
                name: String::new(),
                status: None,
                priority: 0,
                assignees: vec![],
                due: None,
                parent: Some(tid),
                open: true,
            });
            close_any(ui);
        }
        if ui.button(tr("Move to...")).clicked() {
            self.modal = Some(Modal::MoveTasks {
                ids: vec![tid],
                query: String::new(),
            });
            close_any(ui);
        }
        if ui.button(tr("Make subtask of...")).clicked() {
            self.modal = Some(Modal::PickTask {
                task: tid,
                mode: TaskPick::Parent,
                query: String::new(),
            });
            close_any(ui);
        }
        if task.parent.is_some() && ui.button(tr("Make top-level task")).clicked() {
            self.make_top_level(tid);
            close_any(ui);
        }
        if ui.button(tr("Duplicate")).clicked() {
            if self.duplicate_task(tid).is_some() {
                self.toast(tr("Task duplicated"));
            }
            close_any(ui);
        }
        if ui.button(tr("Remind me")).clicked() {
            self.modal = Some(Modal::reminder(
                task.name.clone(),
                today() + Duration::days(1),
            ));
            self.reminder_about = (Some(task.id), None);
            close_any(ui);
        }
        if ui
            .button(if self.data.is_my_timer(&task) {
                tr("Stop timer")
            } else {
                tr("Start timer")
            })
            .clicked()
        {
            self.actions.push(Action::ToggleTimer(tid));
            close_any(ui);
        }
        if self.may_manage() && ui.button(tr("Save as template")).clicked() {
            let id = self.data.new_id();
            self.data.extra.templates.push(TaskTemplate {
                id,
                name: task.name.clone(),
                task_name: task.name.clone(),
                description: task.description.clone(),
                checklist: task.checklist.iter().map(|c| c.text.clone()).collect(),
                tags: task.tags.clone(),
                priority: task.priority,
                ..Default::default()
            });
            self.toast(tr("Saved as a template"));
            self.dirty = true;
            close_any(ui);
        }
        ui.separator();
        if ui
            .button(if task.archived {
                tr("Unarchive")
            } else {
                tr("Archive")
            })
            .clicked()
        {
            self.set_archived(&[tid], !task.archived);
            close_any(ui);
        }
        if ui
            .button(RichText::new(tr("Delete")).color(t.danger))
            .clicked()
        {
            self.actions.push(Action::DeleteTask(tid));
            close_any(ui);
        }
    }

    /// The list menu (right click, or the "..." button on a sidebar list).
    pub(crate) fn list_menu(&mut self, ui: &mut Ui, lid: u64) {
        let t = self.t();
        let loc = Location::List(lid);
        ui.set_min_width(200.0);
        let fav = if self.data.favorites.contains(&loc) {
            "Remove from Favorites"
        } else {
            "Add to Favorites"
        };
        if ui.button(tr(fav)).clicked() {
            self.actions.push(Action::ToggleFavorite(loc));
            close_any(ui);
        }
        if ui.button(tr("Rename")).clicked() {
            self.actions.push(Action::StartRename(Target::List(lid)));
            close_any(ui);
        }
        if ui.button(tr("Copy link")).clicked() {
            self.copy_link(ui, nav::location_link(Location::List(lid)));
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
                    name_it(
                        &resp,
                        &format!("List color {:02x}{:02x}{:02x}", c[0], c[1], c[2]),
                    );
                    ui.painter().circle_filled(rr.center(), 8.0, rgb(*c));
                    hover_ring(ui, &resp, rr.shrink(0.5), 8.5);
                    if cur == Some(*c) {
                        ui.painter()
                            .circle_stroke(rr.center(), 9.5, Stroke::new(1.5_f32, t.text));
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
        if self.may_manage() && ui.button(tr("Custom fields")).clicked() {
            self.modal = Some(Modal::custom_fields());
            close_any(ui);
        }
        if self.may_manage() && ui.button(tr("Automations")).clicked() {
            self.modal = Some(Modal::automations(Some(Location::List(lid))));
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
                self.toast(tr("List duplicated"));
            }
            close_any(ui);
        }
        if self.may_manage() && ui.button(tr("Save as template")).clicked() {
            if self.save_list_as_template(lid).is_some() {
                self.toast(tr("Saved as a template"));
            }
            close_any(ui);
        }
        if ui.button(tr("Archive")).clicked() {
            self.set_list_archived(lid, true);
            close_any(ui);
        }
        if ui
            .button(RichText::new(tr("Delete")).color(t.danger))
            .clicked()
        {
            self.actions.push(Action::AskDelete(Target::List(lid)));
            close_any(ui);
        }
    }

    /// The folder menu (right click, or the "..." button on a sidebar folder).
    pub(crate) fn folder_menu(&mut self, ui: &mut Ui, fid: u64) {
        let t = self.t();
        let floc = Location::Folder(fid);
        let Some(sid) = self
            .data
            .spaces
            .iter()
            .find(|s| s.folders.iter().any(|f| f.id == fid))
            .map(|s| s.id)
        else {
            return;
        };
        ui.set_min_width(200.0);
        let fav = if self.data.favorites.contains(&floc) {
            "Remove from Favorites"
        } else {
            "Add to Favorites"
        };
        if ui.button(tr(fav)).clicked() {
            self.actions.push(Action::ToggleFavorite(floc));
            close_any(ui);
        }
        if ui.button(tr("Rename")).clicked() {
            self.actions.push(Action::StartRename(Target::Folder(fid)));
            close_any(ui);
        }
        if ui.button(tr("Copy link")).clicked() {
            self.copy_link(ui, nav::location_link(Location::Folder(fid)));
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
        let others: Vec<(u64, String)> = self
            .open_spaces()
            .iter()
            .filter(|s| s.id != sid)
            .map(|s| (s.id, s.name.clone()))
            .collect();
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
                self.toast(tr("Folder duplicated"));
            }
            close_any(ui);
        }
        if ui.button(tr("Archive")).clicked() {
            self.set_folder_archived(fid, true);
            close_any(ui);
        }
        ui.separator();
        if ui
            .button(RichText::new(tr("Delete")).color(t.danger))
            .clicked()
        {
            self.actions.push(Action::AskDelete(Target::Folder(fid)));
            close_any(ui);
        }
    }

    /// The "+" on a sidebar list, and the list menu's "Create new".
    pub(crate) fn list_create_menu(&mut self, ui: &mut Ui, lid: u64) {
        ui.set_min_width(200.0);
        let sid = self.data.space_of_list(lid).map(|s| s.id);
        let folder = self
            .data
            .spaces
            .iter()
            .flat_map(|s| s.folders.iter())
            .find(|f| f.lists.iter().any(|l| l.id == lid))
            .map(|f| f.id);
        ui.label(RichText::new(tr("Create")).small().color(self.t().text3));
        for (icon, label) in [
            ("task", "Task"),
            ("list", "List"),
            ("docs", "Doc"),
            ("dashboards", "Dashboard"),
            ("whiteboards", "Whiteboard"),
            ("forms", "Form"),
        ] {
            let t = self.t();
            let b = menu_row_named(
                ui,
                &t,
                &format!("Create {}", label),
                tr(label),
                false,
                |p, c| {
                    super::icons::line_icon(p, icon, c, t.text2);
                },
            );
            if b.clicked() {
                match label {
                    "Task" => {
                        self.section = Section::Home;
                        self.loc = Location::List(lid);
                        self.open_quick_add(None);
                    }
                    "List" => self.open_new_list(sid, folder),
                    "Doc" => self.new_doc(sid),
                    "Dashboard" => {
                        self.modal = Some(Modal::NewDashboard {
                            name: tr("New Dashboard").into(),
                        })
                    }
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

    /// The space menu (right click, or the "..." button on a sidebar space).
    pub(crate) fn space_menu(&mut self, ui: &mut Ui, sid: u64) {
        let level = self.space_level(sid);
        if level < crate::sync::Level::Full {
            self.limited_space_menu(ui, sid, level);
            return;
        }
        let t = self.t();
        let loc = Location::Space(sid);
        ui.set_min_width(200.0);
        let fav = if self.data.favorites.contains(&loc) {
            "Remove from Favorites"
        } else {
            "Add to Favorites"
        };
        if ui.button(tr(fav)).clicked() {
            self.actions.push(Action::ToggleFavorite(loc));
            close_any(ui);
        }
        if ui.button(tr("Rename")).clicked() {
            self.actions.push(Action::StartRename(Target::Space(sid)));
            close_any(ui);
        }
        if ui.button(tr("Copy link")).clicked() {
            self.copy_link(ui, nav::location_link(Location::Space(sid)));
            close_any(ui);
        }
        let personal = self
            .data
            .spaces
            .iter()
            .any(|s| s.id == sid && s.private_owner.is_some());
        if !personal && ui.button(tr("Share & permissions")).clicked() {
            self.open_sharing(sid);
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
            if ui.button(tr("From a template")).clicked() {
                self.open_templates(super::templates::Category::Lists, Some(sid));
                close_any(ui);
            }
        });
        ui.menu_button(tr("Color & Icon"), |ui| {
            ui.label(RichText::new(tr("Space color")).small().color(t.text3));
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(190.0);
                for c in STATUS_PALETTE.iter().skip(1) {
                    let (rr, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    name_it(
                        &resp,
                        &format!("Space color {:02x}{:02x}{:02x}", c[0], c[1], c[2]),
                    );
                    ui.painter().circle_filled(rr.center(), 8.0, rgb(*c));
                    hover_ring(ui, &resp, rr.shrink(0.5), 8.5);
                    if resp.clicked() {
                        self.actions.push(Action::SetSpaceColor(sid, *c));
                        close_any(ui);
                    }
                }
            });
            ui.add_space(4.0);
            ui.label(RichText::new(tr("Icon")).small().color(t.text3));
            let color = self
                .data
                .spaces
                .iter()
                .find(|s| s.id == sid)
                .map(|s| s.color)
                .unwrap_or([96, 73, 231]);
            let letter = self
                .data
                .spaces
                .iter()
                .find(|s| s.id == sid)
                .map(|s| {
                    s.name
                        .chars()
                        .next()
                        .unwrap_or('S')
                        .to_uppercase()
                        .to_string()
                })
                .unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(190.0);
                for ic in SPACE_ICONS {
                    let (rr, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                    name_it(
                        &resp,
                        &format!("Space icon {}", if ic.is_empty() { "letter" } else { ic }),
                    );
                    ui.painter().rect_filled(rr.shrink(2.0), 5.0, rgb(color));
                    hover_ring(ui, &resp, rr.shrink(1.0), 6.0);
                    ui.painter().text(
                        rr.center(),
                        Align2::CENTER_CENTER,
                        if ic.is_empty() { letter.as_str() } else { ic },
                        FontId::proportional(11.0),
                        Color32::WHITE,
                    );
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
        if self.may_manage() && ui.button(tr("Custom fields")).clicked() {
            self.modal = Some(Modal::custom_fields());
            close_any(ui);
        }
        if self.may_manage() && ui.button(tr("Automations")).clicked() {
            self.modal = Some(Modal::automations(Some(Location::Space(sid))));
            close_any(ui);
        }
        ui.separator();
        if ui.button(tr("Duplicate")).clicked() {
            if let Some(n) = self.duplicate_space(sid) {
                self.actions.push(Action::Go(Location::Space(n)));
                self.toast(tr("Space duplicated"));
            }
            close_any(ui);
        }
        let hidden = self
            .data
            .spaces
            .iter()
            .find(|s| s.id == sid)
            .is_some_and(|s| s.hidden);
        if ui
            .button(if hidden {
                tr("Show Space")
            } else {
                tr("Hide Space")
            })
            .clicked()
        {
            if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
                s.hidden = !hidden;
                self.dirty = true;
            }
            if !hidden {
                self.toast(tr(
                    "Space hidden from the sidebar. Show it again from Settings > Spaces.",
                ));
            }
            close_any(ui);
        }
        let archived = self
            .data
            .spaces
            .iter()
            .find(|s| s.id == sid)
            .is_some_and(|s| s.archived);
        if ui
            .button(if archived {
                tr("Restore")
            } else {
                tr("Archive")
            })
            .clicked()
        {
            self.set_space_archived(sid, !archived);
            close_any(ui);
        }
        if ui
            .button(RichText::new(tr("Delete")).color(t.danger))
            .clicked()
        {
            self.actions.push(Action::AskDelete(Target::Space(sid)));
            close_any(ui);
        }
    }
}
