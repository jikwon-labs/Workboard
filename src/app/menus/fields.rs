// Dialogs that shape tasks across the workspace: custom fields, automations
// and the tag manager.

use super::*;

impl App {
    /// The CustomFields dialog. Returns true when it should close.
    pub(super) fn custom_fields_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        name: &mut String,
        kind: &mut String,
        opts: &mut String,
    ) -> bool {
        let mut remove = None;
        for (i, f) in self.data.extra.fields.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&f.name).strong());
                let mut k = f.kind.clone();
                if let Some(c) = k.get_mut(0..1) {
                    c.make_ascii_uppercase();
                }
                ui.label(RichText::new(tr(&k)).color(t.text2));
                if !f.options.is_empty() {
                    ui.label(RichText::new(f.options.join(", ")).small().color(t.text3));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(ghost(egui::Button::new(
                            RichText::new(tr("Delete")).color(t.danger),
                        )))
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            let id = self.data.extra.fields[i].id;
            self.data.extra.fields.remove(i);
            for task in &mut self.data.tasks {
                task.custom.remove(&id);
            }
            self.dirty = true;
        }
        if self.data.extra.fields.is_empty() {
            ui.label(RichText::new(tr("Add your own columns to tasks, like Budget, Client or Sprint. They show on every task and in the Table view.")).color(t.text2));
        }
        ui.add_space(4.0);
        ui.separator();
        ui.label(RichText::new(tr("New field")).strong());
        const KINDS: [(&str, &str, &str); 5] = [
            ("text", "Text", "Any words"),
            ("number", "Number", "Amounts, counts, points"),
            ("dropdown", "Dropdown", "Pick one of your options"),
            ("checkbox", "Checkbox", "Done or not"),
            ("date", "Date", "A day on the calendar"),
        ];
        let kind_label = KINDS
            .iter()
            .find(|k| k.0 == kind.as_str())
            .map(|k| k.1)
            .unwrap_or("Text");
        ui.horizontal(|ui| {
            let r = ui.add(
                field(name)
                    .hint_text(tr("Field name, e.g. Budget"))
                    .desired_width(240.0),
            );
            name_field(&r, "Field name");
            let k = egui::ComboBox::from_id_salt("cf_kind")
                .selected_text(tr(kind_label))
                .width(150.0)
                .show_ui(ui, |ui| {
                    for (k, l, d) in KINDS {
                        ui.selectable_value(kind, k.to_string(), tr(l))
                            .on_hover_text(tr(d));
                    }
                })
                .response;
            name_it(&k, "Field type");
        });
        if let Some(d) = KINDS.iter().find(|k| k.0 == kind.as_str()).map(|k| k.2) {
            ui.label(RichText::new(tr(d)).small().color(t.text3));
        }
        if kind == "dropdown" {
            ui.add(
                field(opts)
                    .hint_text(tr("Options, comma separated: Low, Medium, High"))
                    .desired_width(f32::INFINITY),
            );
        }
        if primary_button(ui, t, tr("Add field")).clicked() && !name.trim().is_empty() {
            let id = self.data.new_id();
            let options = if kind == "dropdown" {
                opts.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            } else {
                vec![]
            };
            self.data.extra.fields.push(CustomField {
                id,
                name: name.trim().to_string(),
                kind: kind.clone(),
                options,
            });
            name.clear();
            opts.clear();
            self.dirty = true;
        }
        false
    }

    /// The Automations dialog. Returns true when it should close.
    pub(super) fn automations_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        status: &mut String,
        action: &mut String,
        value: &mut String,
        scope: &mut Option<Location>,
    ) -> bool {
        let mut remove = None;
        let rules = self.data.extra.automations.clone();
        for (i, a) in rules.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut en = a.enabled;
                if check(ui, &mut en, "").changed() {
                    self.data.extra.automations[i].enabled = en;
                    self.dirty = true;
                }
                ui.label(crate::trf!(
                    "When status changes to {}, {}",
                    crate::i18n::status_text(&a.trigger_status).to_uppercase(),
                    self.action_text(a)
                ));
                if let Some(l) = a.list {
                    ui.label(
                        RichText::new(crate::trf!("in {}", self.scope_name(l))).color(t.text3),
                    );
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(ghost(egui::Button::new(
                            RichText::new(tr("Delete")).color(t.danger),
                        )))
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            self.data.extra.automations.remove(i);
            self.dirty = true;
        }
        if rules.is_empty() {
            ui.label(RichText::new(tr("No automations yet.")).color(t.text3));
        }
        ui.separator();
        ui.label(RichText::new(tr("New automation")).strong());
        // opened from a List or Space menu: the new rule runs only there
        let scope_id = match *scope {
            Some(Location::List(id)) | Some(Location::Space(id)) => Some(id),
            _ => None,
        };
        ui.label(
            RichText::new(match scope_id {
                Some(id) => crate::trf!("Runs in {}", self.scope_name(id)),
                None => tr("Runs in the whole Workspace").to_string(),
            })
            .small()
            .color(t.text2),
        );
        let mut names: Vec<String> = vec![];
        for s in &self.data.spaces {
            for st in &s.statuses {
                if !names.contains(&st.name) {
                    names.push(st.name.clone());
                }
            }
        }
        ui.horizontal(|ui| {
            ui.label(tr("When status changes to"));
            egui::ComboBox::from_id_salt("au_status")
                .selected_text(crate::i18n::status_text(status).to_uppercase())
                .show_ui(ui, |ui| {
                    for n in &names {
                        ui.selectable_value(
                            status,
                            n.clone(),
                            crate::i18n::status_text(n).to_uppercase(),
                        );
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label(tr("then"));
            egui::ComboBox::from_id_salt("au_action")
                .selected_text(match action.as_str() {
                    "assign" => tr("assign to"),
                    "priority" => tr("set priority"),
                    "move" => tr("move to list"),
                    _ => tr("add a comment"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(action, "assign".to_string(), tr("assign to"));
                    ui.selectable_value(action, "priority".to_string(), tr("set priority"));
                    ui.selectable_value(action, "comment".to_string(), tr("add a comment"));
                    ui.selectable_value(action, "move".to_string(), tr("move to list"));
                });
            match action.as_str() {
                "assign" => {
                    let cur = value
                        .parse::<u64>()
                        .ok()
                        .and_then(|id| self.data.member(id))
                        .map(|m| m.name.clone())
                        .unwrap_or_else(|| tr("choose").into());
                    egui::ComboBox::from_id_salt("au_val")
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            for m in &self.data.members {
                                ui.selectable_value(value, m.id.to_string(), &m.name);
                            }
                        });
                }
                "priority" => {
                    let cur = value
                        .parse::<usize>()
                        .ok()
                        .filter(|p| *p < 5)
                        .map(|p| tr(PRIORITIES[p].label))
                        .unwrap_or(tr("choose"));
                    egui::ComboBox::from_id_salt("au_val")
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            for p in 0..5usize {
                                ui.selectable_value(value, p.to_string(), tr(PRIORITIES[p].label));
                            }
                        });
                }
                "move" => {
                    let cur = value
                        .parse::<u64>()
                        .ok()
                        .map(|id| self.data.list_name(id))
                        .unwrap_or_else(|| tr("choose").into());
                    egui::ComboBox::from_id_salt("au_val")
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            for s in &self.data.spaces {
                                for l in s
                                    .lists
                                    .iter()
                                    .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
                                {
                                    ui.selectable_value(
                                        value,
                                        l.id.to_string(),
                                        format!("{} / {}", s.name, l.name),
                                    );
                                }
                            }
                        });
                }
                _ => {
                    ui.add(
                        field(value)
                            .hint_text(tr("Comment text"))
                            .desired_width(200.0),
                    );
                }
            }
        });
        if primary_button(ui, t, tr("Add automation")).clicked() && !value.trim().is_empty() {
            let id = self.data.new_id();
            self.data.extra.automations.push(Automation {
                id,
                enabled: true,
                trigger_status: status.clone(),
                action: action.clone(),
                value: value.trim().to_string(),
                list: scope_id,
            });
            value.clear();
            self.dirty = true;
        }
        false
    }

    /// The TagManager dialog. Returns true when it should close.
    pub(super) fn tag_manager_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        from: &mut String,
        to: &mut String,
    ) -> bool {
        let mut counts: Vec<(String, usize)> = vec![];
        for task in &self.data.tasks {
            for tag in &task.tags {
                match counts.iter_mut().find(|c| &c.0 == tag) {
                    Some(c) => c.1 += 1,
                    None => counts.push((tag.clone(), 1)),
                }
            }
        }
        counts.sort();
        if counts.is_empty() {
            ui.label(RichText::new(tr("No tags yet. Add tags from a task.")).color(t.text3));
        }
        let mut delete = None;
        for (tag, n) in &counts {
            ui.horizontal(|ui| {
                chip(ui, tag, tag_color(tag));
                ui.label(
                    RichText::new(if *n == 1 {
                        tr("1 task").to_string()
                    } else {
                        crate::trf!("{} tasks", n)
                    })
                    .color(t.text2),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(ghost(egui::Button::new(
                            RichText::new(tr("Delete")).color(t.danger),
                        )))
                        .clicked()
                    {
                        delete = Some(tag.clone());
                    }
                    if ui.add(ghost(egui::Button::new(tr("Rename")))).clicked() {
                        *from = tag.clone();
                        *to = tag.clone();
                    }
                });
            });
        }
        if let Some(tag) = delete {
            for task in &mut self.data.tasks {
                task.tags.retain(|x| x != &tag);
            }
            self.dirty = true;
        }
        if !from.is_empty() {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(crate::trf!("Rename \"{}\" to", from));
                ui.add(field(to).desired_width(140.0));
                if primary_button(ui, t, tr("Save")).clicked() && !to.trim().is_empty() {
                    let nt = to.trim().to_lowercase();
                    for task in &mut self.data.tasks {
                        for tag in task.tags.iter_mut() {
                            if tag == from {
                                *tag = nt.clone();
                            }
                        }
                        // drop repeats anywhere in the list, keeping the first
                        let mut seen = std::collections::HashSet::new();
                        task.tags.retain(|g| seen.insert(g.clone()));
                    }
                    from.clear();
                    self.dirty = true;
                }
            });
        }
        false
    }
}
