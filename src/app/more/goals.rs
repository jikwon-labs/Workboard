// Goals: their progress, the Goals page and the goal editor.

use super::*;

// ---------------------------------------------------------------- goals

impl App {
    pub(crate) fn goal_progress(&self, g: &Goal) -> f32 {
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

    pub(crate) fn goals_page(&mut self, ui: &mut Ui) {
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
                                .stroke(Stroke::new(1.0_f32, if open { t.text3 } else { t.border }))
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
                                                        RichText::new(m.shown())
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
            .map(|m| (m.id, m.shown().to_string()))
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
                            ui.painter().circle_stroke(
                                r.center(),
                                9.0,
                                Stroke::new(1.5_f32, t.text),
                            );
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
}
