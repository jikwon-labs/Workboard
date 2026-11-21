// Selecting several tasks at once: the bar of bulk actions shown while tasks
// are selected, and the selection box at the start of a row.

use super::*;

// ---------------------------------------------------------------- bulk edit

impl App {
    /// The bar along the bottom while tasks are selected.
    pub(crate) fn bulk_bar(&mut self, ctx: &egui::Context) {
        self.undo_bar(ctx);
        self.selected
            .retain(|id| self.data.tasks.iter().any(|t| t.id == *id));
        self.bulk_delete_confirm(ctx);
        if self.selected.is_empty() || self.section != Section::Home {
            return;
        }
        let t = self.t();
        let ids: Vec<u64> = self.selected.iter().copied().collect();
        let first = self.task(ids[0]).cloned();
        egui::Area::new(egui::Id::new("bulk_bar"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, [0.0, -22.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(32, 32, 36))
                    .corner_radius(10)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.style_mut().visuals.override_text_color = Some(Color32::WHITE);
                            ui.style_mut().visuals.widgets.inactive.weak_bg_fill =
                                Color32::from_rgb(52, 52, 58);
                            ui.style_mut().visuals.widgets.hovered.weak_bg_fill =
                                Color32::from_rgb(72, 72, 80);
                            ui.label(
                                RichText::new(if ids.len() == 1 {
                                    trf!("{} task selected", ids.len())
                                } else {
                                    trf!("{} tasks selected", ids.len())
                                })
                                .strong()
                                .color(Color32::WHITE),
                            );
                            let clear = crate::app::icons::square_button(
                                ui,
                                "\u{00D7}",
                                crate::widgets::CONTROL_H,
                                "Clear selection",
                            );
                            if clear.on_hover_text(tr("Clear selection")).clicked() {
                                self.selected.clear();
                            }
                            ui.separator();
                            let pid = ui.id();
                            let statuses = first
                                .as_ref()
                                .map(|f| self.data.statuses_for_list(f.list_id))
                                .unwrap_or_else(default_statuses);
                            let r = ui.button(tr("Status"));
                            let mut pick_status: Option<String> = None;
                            popup(ui, &r, pid.with("bs"), 180.0, |ui| {
                                for st in &statuses {
                                    if menu_row(
                                        ui,
                                        &t,
                                        &crate::i18n::status_text(&st.name).to_uppercase(),
                                        false,
                                        |p, c| paint_status_circle(p, c, 6.0, st),
                                    )
                                    .clicked()
                                    {
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
                            let sel_tasks: Vec<Task> = ids
                                .iter()
                                .filter_map(|id| self.task(*id).cloned())
                                .collect();
                            popup(ui, &r, pid.with("ba"), 200.0, |ui| {
                                // ticked when every selected task has them: a click then removes
                                for m in &members {
                                    let ring = t.card;
                                    let all = sel_tasks.iter().all(|x| x.assignees.contains(&m.id));
                                    if menu_row(ui, &t, m.shown(), all, |p, c| {
                                        paint_avatar(p, c, 9.0, m, ring)
                                    })
                                    .clicked()
                                    {
                                        if all {
                                            drop_asg = Some(m.id);
                                        } else {
                                            pick_asg = Some(m.id);
                                        }
                                        close_popup(ui);
                                    }
                                }
                                ui.separator();
                                if menu_row_plain(ui, &t, tr("Unassign all"), false).clicked() {
                                    unassign = true;
                                    close_popup(ui);
                                }
                            });
                            let r = ui.button(tr("Tags"));
                            let mut add_tags: Option<Vec<String>> = None;
                            let all_tags = self.all_tags();
                            let tag_before: Vec<String> = all_tags
                                .iter()
                                .filter(|g| sel_tasks.iter().all(|x| x.tags.contains(g)))
                                .cloned()
                                .collect();
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
                            popup(ui, &r, pid.with("bd"), DATE_MENU_W, |ui| {
                                if date_menu(ui, &t, pid.with("bdc"), &mut due) {
                                    due_set = true;
                                }
                            });
                            let r = ui.button(tr("Start date"));
                            let mut start: Option<NaiveDate> = None;
                            let mut start_set = false;
                            popup(ui, &r, pid.with("bsd"), DATE_MENU_W, |ui| {
                                if date_menu(ui, &t, pid.with("bsdc"), &mut start) {
                                    start_set = true;
                                }
                            });
                            let r = ui.button(tr("Repeat"));
                            let mut recur: Option<Option<String>> = None;
                            popup(ui, &r, pid.with("brc"), 160.0, |ui| {
                                for (v, label) in RECUR {
                                    if menu_row_plain(ui, &t, tr(label), false).clicked() {
                                        recur = Some(if v.is_empty() {
                                            None
                                        } else {
                                            Some(v.to_string())
                                        });
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
                                self.modal = Some(Modal::MoveTasks {
                                    ids: ids.clone(),
                                    query: String::new(),
                                });
                            }
                            if ui.button(tr("Duplicate")).clicked() {
                                for id in &ids {
                                    self.duplicate_task(*id);
                                }
                                self.toast(if ids.len() == 1 {
                                    tr("Duplicated 1 task").to_string()
                                } else {
                                    trf!("Duplicated {} tasks", ids.len())
                                });
                                self.selected.clear();
                            }
                            // archived tasks (Show archived tasks only) can be brought back
                            let all_archived = sel_tasks.iter().all(|x| x.archived);
                            if ui
                                .button(tr(if all_archived { "Unarchive" } else { "Archive" }))
                                .clicked()
                            {
                                self.set_archived(&ids, !all_archived);
                                if all_archived {
                                    self.selected.clear();
                                }
                            }
                            if ui
                                .button(
                                    RichText::new(tr("Delete"))
                                        .color(Color32::from_rgb(255, 120, 120)),
                                )
                                .clicked()
                            {
                                self.confirm_bulk_delete = true;
                            }
                            // apply picks
                            let mut edits: Vec<Task> = vec![];
                            for id in &ids {
                                let Some(mut e) = self.task(*id).cloned() else {
                                    continue;
                                };
                                let mut ch = false;
                                if let Some(s) = &pick_status {
                                    if self
                                        .data
                                        .statuses_for_list(e.list_id)
                                        .iter()
                                        .any(|x| x.name == *s)
                                    {
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
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && self.modal.is_none()
            && self.open_task.is_none()
            && !self.menu_was_open
        {
            self.selected.clear();
        }
    }

    /// The selection box at the left of a list row; returns true when clicked.
    pub(crate) fn row_checkbox(&mut self, ui: &mut Ui, row: Rect, tid: u64, hovered: bool) -> bool {
        let t = self.t();
        let on = self.selected.contains(&tid);
        let br = Rect::from_center_size(pos2(row.left() + 2.0, row.center().y), vec2(14.0, 14.0));
        let resp = ui.interact(br.expand(3.0), ui.id().with(("sel", tid)), Sense::click());
        let tname = self.task(tid).map(|t| t.name.clone()).unwrap_or_default();
        name_it(&resp, &format!("Select {}", tname));
        if on || hovered || resp.hovered() || !self.selected.is_empty() {
            if on {
                ui.painter().rect_filled(br, 3.0, t.primary);
                ui.painter().line_segment(
                    [
                        br.left_center() + vec2(3.0, 0.0),
                        br.center_bottom() + vec2(-1.0, -3.5),
                    ],
                    Stroke::new(1.6_f32, t.on_primary),
                );
                ui.painter().line_segment(
                    [
                        br.center_bottom() + vec2(-1.0, -3.5),
                        br.right_top() + vec2(-3.0, 3.5),
                    ],
                    Stroke::new(1.6_f32, t.on_primary),
                );
            } else {
                ui.painter().rect_stroke(
                    br,
                    3.0,
                    Stroke::new(1.2_f32, t.text3),
                    egui::StrokeKind::Inside,
                );
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
}
