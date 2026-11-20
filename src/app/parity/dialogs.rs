// The New List, New Space and New Channel dialogs and what they create, and
// the List Info dialog.

use super::*;

// ---------------------------------------------------------------- create

impl App {
    /// Spaces a new list can go in (not the hidden personal space).
    pub(crate) fn open_spaces(&self) -> Vec<&Space> {
        self.data
            .spaces
            .iter()
            .filter(|s| s.private_owner.is_none() && !s.archived)
            .collect()
    }

    pub(crate) fn open_new_list(&mut self, space: Option<u64>, folder: Option<u64>) {
        let visible = |id: u64| {
            self.data
                .spaces
                .iter()
                .any(|s| s.id == id && s.private_owner.is_none())
        };
        let space = space
            .filter(|s| visible(*s))
            .or_else(|| self.open_spaces().first().map(|s| s.id))
            .unwrap_or(0);
        self.modal = Some(Modal::NewList {
            name: String::new(),
            space,
            folder,
        });
    }

    pub(crate) fn create_list(
        &mut self,
        space: u64,
        folder: Option<u64>,
        name: &str,
    ) -> Option<u64> {
        let name = if name.trim().is_empty() {
            "List".to_string()
        } else {
            name.trim().to_string()
        };
        let id = self.data.new_id();
        let s = self.data.spaces.iter_mut().find(|s| s.id == space)?;
        s.expanded = true;
        let item = ListItem {
            id,
            name,
            ..Default::default()
        };
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
        let name = if name.trim().is_empty() {
            "Space".to_string()
        } else {
            name.trim().to_string()
        };
        self.data.spaces.push(Space {
            id,
            name,
            color,
            statuses: default_statuses(),
            folders: vec![],
            lists: vec![ListItem {
                id: lid,
                name: "List".into(),
                ..Default::default()
            }],
            expanded: true,
            private_owner: None,
            ..Default::default()
        });
        self.section = Section::Home;
        self.loc = Location::Space(id);
        self.view = ViewKind::Overview;
        self.dirty = true;
        id
    }

    pub(crate) fn new_list_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        name: &mut String,
        space: &mut u64,
        folder: &mut Option<u64>,
    ) -> bool {
        let mut close = false;
        ui.label(
            RichText::new(tr(
                "All Lists are located within a Space. Lists can house any type of task.",
            ))
            .color(t.text2),
        );
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Name")).color(t.text2));
        let r = ui.add(
            field(name)
                .hint_text(tr("e.g. Project, List of items, Campaign"))
                .desired_width(f32::INFINITY),
        );
        focus_once(ui, &r);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Space (location)")).color(t.text2));
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
        let cur = spaces.iter().find(|s| s.0 == *space).map(|s| {
            match folder.and_then(|f| s.2.iter().find(|x| x.0 == f)) {
                Some(f) => format!("{} / {}", s.1, f.1),
                None => s.1.clone(),
            }
        });
        // as wide as the name field above it
        egui::ComboBox::from_id_salt("new_list_space")
            .width(ui.available_width())
            .selected_text(cur.unwrap_or_else(|| tr("Choose a Space").into()))
            .show_ui(ui, |ui| {
                for (sid, sname, folders) in &spaces {
                    if ui
                        .selectable_label(*space == *sid && folder.is_none(), sname)
                        .clicked()
                    {
                        *space = *sid;
                        *folder = None;
                    }
                    for (fid, fname) in folders {
                        if ui
                            .selectable_label(
                                *folder == Some(*fid),
                                format!("    {} / {}", sname, fname),
                            )
                            .clicked()
                        {
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
                if (ui
                    .add_enabled(
                        ok,
                        Tint(primary_btn(t, tr("Create")).min_size(vec2(90.0, CONTROL_H))),
                    )
                    .clicked()
                    || (enter && ok))
                    && self.create_list(*space, *folder, name).is_some()
                {
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    pub(crate) fn new_space_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        name: &mut String,
        color: &mut [u8; 3],
    ) -> bool {
        let mut close = false;
        ui.label(RichText::new(tr("A Space represents teams, departments, or groups, each with its own Lists, workflows, and settings.")).color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Icon & name")).color(t.text2));
        let mut enter = false;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
            ui.painter().rect_filled(r, 7.0, rgb(*color));
            let letter = name
                .chars()
                .next()
                .unwrap_or('S')
                .to_uppercase()
                .to_string();
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                letter,
                FontId::proportional(15.0),
                Color32::WHITE,
            );
            let te = ui.add(
                field(name)
                    .hint_text(tr("e.g. Marketing, Engineering, HR"))
                    .desired_width(f32::INFINITY),
            );
            focus_once(ui, &te);
            enter = te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        });
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Color")).color(t.text2));
        ui.horizontal_wrapped(|ui| {
            for c in STATUS_PALETTE.iter().skip(1) {
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                ui.painter().circle_filled(r.center(), 9.0, rgb(*c));
                hover_ring(ui, &resp, r.shrink(1.0), 10.0);
                if *color == *c {
                    ui.painter()
                        .circle_stroke(r.center(), 11.0, Stroke::new(1.5_f32, t.text));
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
    pub(crate) fn new_channel_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        name: &mut String,
        private: &mut bool,
        add_list: &mut bool,
    ) -> bool {
        let mut close = false;
        ui.label(RichText::new(tr("Channels are where your team communicates. They're best when organized around a topic, like #marketing.")).color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Name")).color(t.text2));
        let r = ui.add(
            field(name)
                .hint_text(tr("e.g. design-team"))
                .desired_width(f32::INFINITY),
        );
        name_field(&r, "Channel name");
        focus_once(ui, &r);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.add_space(10.0);
        // a List needs a Space to go in: without one the switch is off and says so
        let can_list = !self.open_spaces().is_empty();
        if !can_list {
            *add_list = false;
        }
        for (on, label, help, enabled) in [
            (
                &mut *private,
                "Make Private",
                "Only invited people can find this channel.",
                true,
            ),
            (
                &mut *add_list,
                "Add a List",
                if can_list {
                    "Make a List with the same name, tied to this channel."
                } else {
                    "Needs a Space to put the List in. Make one from + Create > Space."
                },
                can_list,
            ),
        ] {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(tr(label)).color(t.text));
                    ui.label(RichText::new(tr(help)).small().color(t.text3));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_enabled_ui(enabled, |ui| switch(ui, t, on, label));
                });
            });
            ui.add_space(6.0);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ok = !name.trim().is_empty();
                let b = ui.add_enabled(
                    ok,
                    Tint(primary_btn(t, tr("Create")).min_size(vec2(90.0, CONTROL_H))),
                );
                if (b.clicked() || (enter && ok)) && ok {
                    self.create_channel(name, *private, *add_list);
                    close = true;
                }
            });
        });
        close
    }

    pub(crate) fn create_channel(&mut self, name: &str, private: bool, add_list: bool) -> u64 {
        let id = self.data.new_id();
        let clean = name.trim().to_lowercase().replace(' ', "-");
        let me = self.data.me;
        let members = if private {
            vec![me]
        } else {
            self.data.members.iter().map(|m| m.id).collect()
        };
        let list = if add_list {
            let space = self
                .current_space()
                .filter(|s| self.open_spaces().iter().any(|x| x.id == *s))
                .or_else(|| self.open_spaces().first().map(|s| s.id));
            space.and_then(|s| self.create_list(s, None, name.trim()))
        } else {
            None
        };
        self.data.channels.push(Channel {
            id,
            name: clean,
            kind: ChannelKind::Channel,
            topic: String::new(),
            members,
            messages: vec![],
            last_read: now_ts(),
            private,
            list,
            creator: me,
        });
        self.section = Section::Chat;
        self.chat.open = Some(id);
        self.leave_thread();
        self.dirty = true;
        id
    }

    /// List Info: where the list is, its task counts and a description.
    pub(crate) fn list_info_body(&mut self, ui: &mut Ui, t: &Theme, lid: u64) -> bool {
        let Some(item) = self.data.list_item(lid).cloned() else {
            return true;
        };
        let path = self.data.list_path(lid).join(" / ");
        let tops: Vec<&Task> = self
            .data
            .tasks
            .iter()
            .filter(|x| x.list_id == lid && x.parent.is_none() && !x.archived)
            .collect();
        let closed = tops.iter().filter(|x| self.data.is_closed(x)).count();
        let subs = self
            .data
            .tasks
            .iter()
            .filter(|x| x.list_id == lid && x.parent.is_some() && !x.archived)
            .count();
        let archived = self
            .data
            .tasks
            .iter()
            .filter(|x| x.list_id == lid && x.archived)
            .count();
        egui::Grid::new("list_info")
            .num_columns(2)
            .spacing(vec2(24.0, 8.0))
            .show(ui, |ui| {
                for (k, v) in [
                    ("Name", item.name.clone()),
                    ("Location", path),
                    (
                        "Tasks",
                        trf!(
                            "{} ({} open, {} closed)",
                            tops.len(),
                            tops.len() - closed,
                            closed
                        ),
                    ),
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
        let r = ui.add(
            field_multi(&mut desc)
                .hint_text(tr("What is this List for?"))
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
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
}
