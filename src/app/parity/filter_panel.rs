// The toolbar panels: Columns (which properties and custom fields show in
// the List) and Filter (condition rows, groups and saved filters).

use super::*;

// ---------------------------------------------------------------- toolbar panels

impl App {
    pub(crate) fn col_on(&self, c: &str) -> bool {
        !self.hidden_cols.contains(c)
    }

    /// The text a property column shows for a task.
    pub(crate) fn prop_text(&self, t: &Task, name: &str) -> String {
        let day = |ts: i64| {
            local(ts)
                .map(|d| fmt_date(d.date_naive()))
                .unwrap_or_default()
        };
        match name {
            "Created by" => t
                .activity
                .first()
                .and_then(|a| self.data.member(a.author))
                .map(|m| m.name.clone())
                .unwrap_or_default(),
            "Date closed" => t.closed_at.map(day).unwrap_or_default(),
            "Date done" => t.done_at.map(day).unwrap_or_default(),
            "Date updated" => day(t.updated),
            "Start date" => fmt_start(t).unwrap_or_default(),
            "Time estimate" => t
                .estimate_min
                .map(|m| fmt_duration(m as i64 * 60))
                .unwrap_or_default(),
            "Time tracked" => {
                let s = t.tracked_secs(now_ts());
                if s > 0 {
                    fmt_duration(s)
                } else {
                    String::new()
                }
            }
            "Tags" => t.tags.join(", "),
            "Latest comment" => t
                .activity
                .iter()
                .rev()
                .find(|a| a.comment)
                .map(|a| a.text.replace('\n', " "))
                .unwrap_or_default(),
            "Subtasks" => {
                let n = self
                    .data
                    .tasks
                    .iter()
                    .filter(|x| x.parent == Some(t.id) && !x.archived)
                    .count();
                if n > 0 {
                    n.to_string()
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    }

    /// The "Fields" panel on the right of a List: search, the fields shown,
    /// and the properties that can be added.
    pub(crate) fn fields_panel(&mut self, ctx: &egui::Context, top: f32) {
        let t = self.t();
        let screen = ctx.screen_rect();
        let rect = Rect::from_min_max(
            pos2(screen.right() - 320.0, top),
            pos2(screen.right() - 8.0, screen.bottom() - 8.0),
        );
        let q_id = egui::Id::new("fields_query");
        let mut q: String = ctx.data(|d| d.get_temp(q_id)).unwrap_or_default();
        let mut close = false;
        egui::Area::new(egui::Id::new("fields_panel")).order(egui::Order::Foreground).fixed_pos(rect.min).show(ctx, |ui| {
            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0_f32, t.border)).corner_radius(10).inner_margin(egui::Margin::same(14)).shadow(egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }).show(ui, |ui| {
                ui.set_width(rect.width() - 28.0);
                ui.set_min_height(rect.height() - 28.0);
                if dialog_header(ui, &t, tr("Fields")) {
                    close = true;
                }
                ui.add_space(6.0);
                let r = ui.add(field(&mut q).hint_text(tr("Search for new or existing fields")).desired_width(f32::INFINITY));
                name_field(&r, "Search fields");
                ui.add_space(6.0);
                let ql = q.trim().to_lowercase();
                let hit = |n: &str| ql.is_empty() || n.to_lowercase().contains(&ql);
                let all: Vec<&str> = COLUMNS.iter().chain(PROPERTIES.iter()).copied().collect();
                // shown properties and fields that the List has no room to
                // draw, laid out the way the List does (custom field columns
                // come before the properties and take their room)
                let list_cols = self.list_cols(Rect::from_min_size(pos2(0.0, 0.0), vec2(self.list_width.max(1.0), 30.0)));
                let fits: Vec<&str> = if self.list_width > 0.0 { list_cols.extra.iter().map(|c| c.0).collect() } else { PROPERTIES.to_vec() };
                let field_fits: Vec<u64> = if self.list_width > 0.0 { list_cols.fields.iter().map(|c| c.0).collect() } else { self.list_field_ids() };
                let fields: Vec<(u64, String)> = self.list_field_ids().into_iter().filter_map(|id| self.data.extra.fields.iter().find(|f| f.id == id).map(|f| (id, f.name.clone()))).collect();
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
                    // custom fields: shown at first, each can be turned off here
                    let shown_fields: Vec<&(u64, String)> = fields.iter().filter(|(_, n)| hit(n)).collect();
                    if !shown_fields.is_empty() {
                        ui.label(RichText::new(tr("Custom fields")).small().color(t.text3));
                        for (fid, name) in shown_fields {
                            let key = super::listview::field_col_key(*fid);
                            ui.horizontal(|ui| {
                                super::icons::icon_label(ui, "fields", RichText::new(name).color(t.text), t.text2);
                                let on_now = !self.hidden_cols.contains(&key);
                                if on_now && !field_fits.contains(fid) {
                                    ui.label(RichText::new(tr("no room")).small().color(t.text3)).on_hover_text(tr("The List is too narrow to show this column; widen the window or turn another field off."));
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let mut on = on_now;
                                    if switch(ui, &t, &mut on, &format!("Field {}", name)).clicked() {
                                        if on {
                                            self.hidden_cols.remove(&key);
                                        } else {
                                            self.hidden_cols.insert(key.clone());
                                        }
                                    }
                                });
                            });
                        }
                        ui.add_space(8.0);
                    }
                });
                ui.separator();
                if self.may_manage() && ui.button(tr("+ Create new field")).clicked() {
                    self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
                    close = true;
                }
            });
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && !self.menu_was_open
            && self.modal.is_none()
        {
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
            FilterField::Status => self
                .location_statuses()
                .into_iter()
                .map(|s| {
                    (
                        s.name.clone(),
                        crate::i18n::status_text(&s.name).to_uppercase(),
                    )
                })
                .collect(),
            FilterField::Priority => [1u8, 2, 3, 4, 0]
                .iter()
                .map(|p| {
                    (
                        p.to_string(),
                        if *p == 0 {
                            tr("No priority").to_string()
                        } else {
                            tr(PRIORITIES[*p as usize].label).to_string()
                        },
                    )
                })
                .collect(),
            FilterField::Assignee => {
                let mut v: Vec<(String, String)> = self
                    .data
                    .members
                    .iter()
                    .map(|m| (m.id.to_string(), m.name.clone()))
                    .collect();
                v.push(("none".into(), tr("Unassigned").into()));
                v
            }
            FilterField::DueDate => DUE_FILTERS
                .iter()
                .enumerate()
                .skip(1)
                .map(|(i, l)| (i.to_string(), tr(l).to_string()))
                .collect(),
            FilterField::Tags => {
                let mut tags: Vec<String> = self
                    .data
                    .tasks
                    .iter()
                    .flat_map(|t| t.tags.iter().cloned())
                    .collect();
                tags.sort();
                tags.dedup();
                tags.into_iter().map(|g| (g.clone(), g)).collect()
            }
            FilterField::StartDate => START_FILTERS
                .iter()
                .enumerate()
                .skip(1)
                .map(|(i, l)| (i.to_string(), tr(l).to_string()))
                .collect(),
            FilterField::Created | FilterField::Updated => AGE_FILTERS
                .iter()
                .enumerate()
                .skip(1)
                .map(|(i, l)| (i.to_string(), tr(l).to_string()))
                .collect(),
            FilterField::CreatedBy => self
                .data
                .members
                .iter()
                .map(|m| (m.id.to_string(), m.name.clone()))
                .collect(),
            FilterField::HasSubtasks => vec![
                ("yes".into(), tr("Has subtasks").into()),
                ("no".into(), tr("No subtasks").into()),
            ],
        }
    }

    /// Condition rows (field, is / is not, values, remove) of one level of
    /// the Filter panel, edited in place. `names` is (id salt, the start of
    /// each control's accessibility name such as "Filter" or "Group 1
    /// filter", the name of the level's And / Or). Returns true when the
    /// And / Or was clicked.
    fn filter_rows_ui(
        &self,
        ui: &mut Ui,
        t: &Theme,
        rows: &mut Vec<FilterRow>,
        join: Join,
        names: (&str, &str, &str),
        values_width: f32,
    ) -> bool {
        let (salt, prefix, join_name) = names;
        let mut flip = false;
        let mut drop: Option<usize> = None;
        for i in 0..rows.len() {
            let row = rows[i].clone();
            let mut new = row.clone();
            let n = i + 1;
            ui.horizontal(|ui| {
                // a fixed-width lead so every row's drop-downs line up
                flip |= join_lead(ui, t, i, join, join_name);
                let fr = egui::ComboBox::from_id_salt(("filter_field", salt, i))
                    .width(130.0)
                    .selected_text(
                        row.field
                            .map(|f| tr(f.label()))
                            .unwrap_or(tr("Select filter")),
                    )
                    .show_ui(ui, |ui| {
                        for f in FilterField::ALL {
                            if ui
                                .selectable_label(row.field == Some(f), tr(f.label()))
                                .clicked()
                                && row.field != Some(f)
                            {
                                new.field = Some(f);
                                new.values.clear();
                            }
                        }
                    });
                name_it(&fr.response, &format!("{} {} field", prefix, n));
                ui.add_enabled_ui(row.field.is_some(), |ui| {
                    let or = egui::ComboBox::from_id_salt(("filter_op", salt, i))
                        .width(70.0)
                        .selected_text(if row.not { tr("is not") } else { tr("is") })
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(!row.not, tr("is")).clicked() {
                                new.not = false;
                            }
                            if ui.selectable_label(row.not, tr("is not")).clicked() {
                                new.not = true;
                            }
                        });
                    name_it(&or.response, &format!("{} {} condition", prefix, n));
                    let choices = row
                        .field
                        .map(|f| self.filter_choices(f))
                        .unwrap_or_default();
                    let summary = if row.values.is_empty() {
                        tr("Select option").to_string()
                    } else {
                        let names: Vec<String> = row
                            .values
                            .iter()
                            .filter_map(|v| choices.iter().find(|c| c.0 == *v).map(|c| c.1.clone()))
                            .collect();
                        truncate(&names.join(", "), 24)
                    };
                    let single = row.field.is_some_and(|f| f.single());
                    let vr = egui::ComboBox::from_id_salt(("filter_vals", salt, i))
                        .width(values_width)
                        .selected_text(summary)
                        .close_behavior(if single {
                            egui::PopupCloseBehavior::CloseOnClick
                        } else {
                            egui::PopupCloseBehavior::CloseOnClickOutside
                        })
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
                    name_it(&vr.response, &format!("{} {} values", prefix, n));
                });
                let x = ui.add(ghost(egui::Button::new("\u{00D7}")));
                name_it(&x, &format!("Remove {} {}", prefix.to_lowercase(), n));
                if x.clicked() {
                    drop = Some(i);
                }
            });
            if new != row {
                rows[i] = new;
            }
        }
        if let Some(i) = drop {
            rows.remove(i);
        }
        flip
    }

    /// Saved filters, below the conditions: the conditions saved under a
    /// name, and the saved ones to bring back, rename or delete. They are
    /// the person's own (`Extra::saved_filters`).
    fn saved_filters_ui(&mut self, ui: &mut Ui, t: &Theme) {
        let name_id = egui::Id::new("saved_filter_name");
        let edit_id = egui::Id::new("saved_filter_rename");
        let mut name: String = ui.data(|d| d.get_temp(name_id)).unwrap_or_default();
        let mut editing: Option<(u64, String)> = ui.data(|d| d.get_temp(edit_id));
        ui.label(RichText::new(tr("Saved filters")).strong());
        let mut apply: Option<u64> = None;
        let mut delete: Option<u64> = None;
        let mut renamed: Option<(u64, String)> = None;
        let mut start_rename: Option<(u64, String)> = None;
        for s in &self.data.extra.saved_filters {
            ui.horizontal(|ui| match &mut editing {
                Some((id, text)) if *id == s.id => {
                    let r = ui.add(field(text).desired_width(260.0));
                    name_field(&r, "Saved filter new name");
                    focus_once(ui, &r);
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let ok = ui.button(tr("Done"));
                    name_it(&ok, "Done renaming");
                    if enter || ok.clicked() {
                        renamed = Some((s.id, text.clone()));
                    }
                }
                _ => {
                    // the name is as the person typed it
                    if ui
                        .selectable_label(s.is_shown(&self.filter), &s.name)
                        .on_hover_text(tr("Show this filter"))
                        .clicked()
                    {
                        apply = Some(s.id);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let d = ui.add(ghost(egui::Button::new(
                            RichText::new(tr("Delete")).color(t.text3),
                        )));
                        name_it(&d, &format!("Delete saved filter {}", s.name));
                        if d.clicked() {
                            delete = Some(s.id);
                        }
                        let r = ui.add(ghost(egui::Button::new(
                            RichText::new(tr("Rename")).color(t.text3),
                        )));
                        name_it(&r, &format!("Rename saved filter {}", s.name));
                        if r.clicked() {
                            start_rename = Some((s.id, s.name.clone()));
                        }
                    });
                }
            });
        }
        if let Some(id) = apply {
            if let Some(s) = self.data.extra.saved_filters.iter().find(|s| s.id == id) {
                s.apply_to(&mut self.filter);
            }
        }
        if let Some(id) = delete {
            self.data.extra.saved_filters.retain(|s| s.id != id);
            self.dirty = true;
        }
        if let Some((id, text)) = renamed {
            let text = text.trim();
            if let Some(s) = self
                .data
                .extra
                .saved_filters
                .iter_mut()
                .find(|s| s.id == id)
            {
                if !text.is_empty() && s.name != text {
                    s.name = text.to_string();
                    self.dirty = true;
                }
            }
            editing = None;
        }
        if start_rename.is_some() {
            editing = start_rename;
        }
        ui.horizontal(|ui| {
            let r = ui.add(
                field(&mut name)
                    .hint_text(tr("Name this filter"))
                    .desired_width(260.0),
            );
            name_field(&r, "Saved filter name");
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let can = !name.trim().is_empty() && self.filter.conditions() > 0;
            let b = ui
                .add_enabled(can, egui::Button::new(tr("Save filter")))
                .on_disabled_hover_text(tr("Pick a condition and type a name first"));
            name_it(&b, "Save filter");
            if can && (b.clicked() || enter) {
                self.save_filter(&name);
                name.clear();
            }
        });
        ui.data_mut(|d| {
            d.insert_temp(name_id, name);
            match editing {
                Some(e) => d.insert_temp(edit_id, e),
                None => d.remove::<(u64, String)>(edit_id),
            }
        });
    }

    /// Saves the builder's conditions under `name`; a saved filter with the
    /// same name takes the new conditions.
    pub(crate) fn save_filter(&mut self, name: &str) {
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }
        let f = &self.filter;
        let mut saved = SavedFilter {
            id: 0,
            name,
            join: f.join,
            rows: f.rows.clone(),
            groups: f.groups.clone(),
        };
        match self
            .data
            .extra
            .saved_filters
            .iter_mut()
            .find(|s| s.name == saved.name)
        {
            Some(s) => {
                saved.id = s.id;
                *s = saved;
            }
            None => {
                saved.id = self.data.new_id();
                self.data.extra.saved_filters.push(saved);
            }
        }
        self.dirty = true;
    }

    /// The Filter panel: a search field, condition rows ("Select filter",
    /// is / is not, values) and groups of rows, combined with And or Or, and
    /// saved filters. Drawn as a panel, not a popup, so the drop-downs in its
    /// rows can open.
    pub(crate) fn filter_panel(&mut self, ctx: &egui::Context, at: Pos2, pill: Rect) {
        let t = self.t();
        let mut close = false;
        let area = egui::Area::new(egui::Id::new("filter_panel"))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
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
                        let r = ui.add(
                            field(&mut self.search)
                                .hint_text(tr("Task name or tag..."))
                                .desired_width(f32::INFINITY),
                        );
                        name_field(&r, "Filter search");
                        ui.add_space(6.0);
                        // edit a copy, then put it back if anything changed
                        let mut f = self.filter.clone();
                        let top_join = f.join;
                        let mut flip = self.filter_rows_ui(
                            ui,
                            &t,
                            &mut f.rows,
                            top_join,
                            ("top", "Filter", "Filter join"),
                            200.0,
                        );
                        let lead = f.rows.len();
                        let mut drop_group: Option<usize> = None;
                        for (g, group) in f.groups.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                // level with the group's first row
                                ui.vertical(|ui| {
                                    ui.add_space(8.0);
                                    flip |= join_lead(ui, &t, lead + g, top_join, "Filter join");
                                });
                                egui::Frame::group(ui.style())
                                    .inner_margin(egui::Margin::same(6))
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            let n = g + 1;
                                            if self.filter_rows_ui(
                                                ui,
                                                &t,
                                                &mut group.rows,
                                                group.join,
                                                (
                                                    &format!("group{}", n),
                                                    &format!("Group {} filter", n),
                                                    &format!("Group {} join", n),
                                                ),
                                                150.0,
                                            ) {
                                                group.join = group.join.flipped();
                                            }
                                            ui.horizontal(|ui| {
                                                let a = ui.add(ghost(egui::Button::new(
                                                    RichText::new(tr("+ Add filter"))
                                                        .color(t.text2),
                                                )));
                                                name_it(&a, &format!("Add filter to group {}", n));
                                                if a.clicked() {
                                                    group.rows.push(FilterRow::default());
                                                }
                                                let x = ui.add(ghost(egui::Button::new(
                                                    RichText::new(tr("Remove group"))
                                                        .color(t.text3),
                                                )));
                                                name_it(&x, &format!("Remove group {}", n));
                                                if x.clicked() {
                                                    drop_group = Some(g);
                                                }
                                            });
                                        });
                                    });
                            });
                        }
                        if let Some(g) = drop_group {
                            f.groups.remove(g);
                        }
                        if flip {
                            f.join = f.join.flipped();
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add(ghost(egui::Button::new(
                                    RichText::new(tr("+ Add filter")).color(t.text2),
                                )))
                                .clicked()
                            {
                                f.rows.push(FilterRow::default());
                            }
                            let g = ui
                                .add(ghost(egui::Button::new(
                                    RichText::new(tr("+ Add group")).color(t.text2),
                                )))
                                .on_hover_text(tr(
                                    "A group has its own And / Or and counts as one condition",
                                ));
                            if g.clicked() {
                                f.groups.push(FilterGroup {
                                    join: Join::And,
                                    rows: vec![FilterRow::default()],
                                });
                            }
                        });
                        if f != self.filter {
                            self.filter = f;
                        }
                        ui.separator();
                        check(
                            ui,
                            &mut self.filter.archived,
                            tr("Show archived tasks only"),
                        );
                        check(ui, &mut self.me_mode, tr("Only my tasks"));
                        ui.separator();
                        self.saved_filters_ui(ui, &t);
                    });
            });
        // close on Escape, or a press outside the panel and its drop-downs
        let pressed_outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| !area.response.rect.contains(p) && !pill.contains(p))
            && !self.menu_was_open
            && !ctx.memory(|m| m.any_popup_open());
        if pressed_outside
            || (ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.menu_was_open)
        {
            close = true;
        }
        if close {
            self.filter_open = false;
            ctx.data_mut(|d| d.remove::<(u64, String)>(egui::Id::new("saved_filter_rename")));
        }
    }
}

/// The lead before a condition: "Where" for the first, then the level's
/// And / Or, which flips when clicked (returns true then).
fn join_lead(ui: &mut Ui, t: &Theme, i: usize, join: Join, name: &str) -> bool {
    if i == 0 {
        let (lr, _) = ui.allocate_exact_size(vec2(48.0, 20.0), Sense::hover());
        ui.painter().text(
            lr.left_center(),
            Align2::LEFT_CENTER,
            crate::i18n::tr_ctx("Where (filter)", "Where"),
            FontId::proportional(14.0),
            t.text3,
        );
        return false;
    }
    let r = ui
        .add(ghost(
            egui::Button::new(RichText::new(tr(join.label())).color(t.text2))
                .min_size(vec2(48.0, 20.0)),
        ))
        .on_hover_text(tr("Switch between And and Or"));
    name_it(&r, name);
    r.clicked()
}
