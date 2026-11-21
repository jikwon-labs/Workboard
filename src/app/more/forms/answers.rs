// A form's Answers tab: the tasks its answers made, with who sent each, its
// priority, status and assignees, filtered by done or not. A row opens
// its task.

use super::*;

/// The table's columns after the task's name: (heading, width).
const COLS: [(&str, f32); 5] = [
    ("Submitted by", 100.0),
    ("Priority", 100.0),
    ("Status", 130.0),
    ("Assignee", 90.0),
    ("Submitted", 120.0),
];

impl App {
    pub(super) fn form_answers_page(&mut self, ui: &mut Ui, fi: usize) {
        let t = self.t();
        let f = self.data.extra.forms[fi].clone();
        let ids = self.form_answer_ids().remove(&f.id).unwrap_or_default();
        let tasks: Vec<Task> = ids
            .iter()
            .filter_map(|id| self.task(*id).cloned())
            .collect();
        let open = tasks.iter().filter(|x| !self.data.is_closed(x)).count();
        let week = tasks
            .iter()
            .filter(|x| now_ts() - x.created < 7 * 86_400)
            .count();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.bg))
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("form_answers")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin {
                                left: 32,
                                right: 32,
                                top: 24,
                                bottom: 40,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(if tasks.len() == 1 {
                                                tr("1 answer").to_string()
                                            } else {
                                                trf!("{} answers", tasks.len())
                                            })
                                                .size(20.0)
                                                .color(t.text),
                                        );
                                        let list = self.data.list_name(f.list_id);
                                        let sub = if self.data.space_of_list(f.list_id).is_some() {
                                            trf!("Each answer is a task in {}. {} this week.", list, week)
                                        } else {
                                            tr("This form has no List for its answers. Choose one under Edit.").to_string()
                                        };
                                        ui.label(RichText::new(sub).size(13.0).color(t.text2));
                                    });
                                    if self.data.space_of_list(f.list_id).is_some() {
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            let label = trf!("Open {}", self.data.list_name(f.list_id));
                                            if icon_button(ui, &t, "list", &label, "Open the form's List", true).clicked() {
                                                self.actions.push(Action::Go(Location::List(f.list_id)));
                                            }
                                        });
                                    }
                                });
                                ui.add_space(14.0);
                                if tasks.is_empty() {
                                    self.form_no_answers(ui, &t);
                                    return;
                                }
                                let items: [SegItem; 3] = [
                                    ("", tr("All").into(), Some(tasks.len()), "All answers"),
                                    ("", tr("Not done").into(), Some(open), "Not done answers"),
                                    ("", tr("Done").into(), Some(tasks.len() - open), "Done answers"),
                                ];
                                if let Some(i) = seg(ui, &t, &items, self.sweep.forms.answers_filter.min(2) as usize) {
                                    self.sweep.forms.answers_filter = i as u8;
                                }
                                ui.add_space(12.0);
                                let shown: Vec<&Task> = tasks
                                    .iter()
                                    .filter(|x| match self.sweep.forms.answers_filter {
                                        1 => !self.data.is_closed(x),
                                        2 => self.data.is_closed(x),
                                        _ => true,
                                    })
                                    .collect();
                                self.form_answers_table(ui, &t, &shown);
                            });
                    });
            });
    }

    /// The Answers tab before anyone has answered.
    fn form_no_answers(&mut self, ui: &mut Ui, t: &Theme) {
        egui::Frame::new()
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(16, 32))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(tr("No answers yet")).size(15.0).color(t.text));
                    ui.label(
                        RichText::new(tr("Answers come in when someone fills out the form."))
                            .size(13.0)
                            .color(t.text2),
                    );
                    ui.add_space(8.0);
                    if icon_button(
                        ui,
                        t,
                        "eye",
                        tr("Fill out the form"),
                        "Fill out the form",
                        true,
                    )
                    .clicked()
                    {
                        self.form.tab = FormTab::Fill;
                    }
                });
            });
    }

    fn form_answers_table(&mut self, ui: &mut Ui, t: &Theme, tasks: &[&Task]) {
        let fixed: f32 = COLS.iter().map(|c| c.1).sum();
        let w = ui.available_width().max(fixed + 220.0);
        let name_w = w - fixed;
        egui::ScrollArea::horizontal()
            .id_salt("form_answers_table")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let (head, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
                ui.painter().rect_filled(
                    head,
                    egui::CornerRadius {
                        nw: 12,
                        ne: 12,
                        sw: 0,
                        se: 0,
                    },
                    t.column_bg,
                );
                let font = FontId::proportional(12.0);
                ui.painter().text(
                    pos2(head.left() + 16.0, head.center().y),
                    Align2::LEFT_CENTER,
                    tr("Task"),
                    font.clone(),
                    t.text2,
                );
                let mut x = head.left() + name_w;
                for (label, cw) in COLS {
                    ui.painter().text(
                        pos2(x + cw / 2.0, head.center().y),
                        Align2::CENTER_CENTER,
                        tr(label),
                        font.clone(),
                        t.text2,
                    );
                    x += cw;
                }
                let mut bottom = head.bottom();
                for task in tasks {
                    let (r, row) = ui.allocate_exact_size(vec2(w, 48.0), Sense::click());
                    name_it(&row, &format!("Answer task {}", task.name));
                    bottom = r.bottom();
                    let p = ui.painter().clone();
                    if row.hovered() {
                        p.rect_filled(r.shrink(1.0), 0.0, t.column_bg);
                    }
                    p.hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
                    let cy = r.center().y;
                    let font = FontId::proportional(14.0);
                    let name = fit_text(&p, &task.name, &font, name_w - 32.0);
                    p.text(
                        pos2(r.left() + 16.0, cy),
                        Align2::LEFT_CENTER,
                        name,
                        font,
                        t.text,
                    );
                    let mut x = r.left() + name_w;
                    // who sent it: the one who logged "submitted the form"
                    let by = task
                        .activity
                        .iter()
                        .find(|a| !a.comment && a.text.starts_with("submitted the form "))
                        .map(|a| a.author);
                    let c = pos2(x + COLS[0].1 / 2.0, cy);
                    match by.and_then(|id| self.data.member(id)) {
                        Some(m) => {
                            paint_avatar(&p, c, 11.0, m, t.bg);
                            ui.interact(
                                Rect::from_center_size(c, vec2(24.0, 24.0)),
                                ui.id().with(("answer_by", task.id)),
                                Sense::hover(),
                            )
                            .on_hover_text(m.shown().to_string());
                        }
                        None => {
                            p.text(
                                c,
                                Align2::CENTER_CENTER,
                                "\u{2014}",
                                FontId::proportional(13.0),
                                t.text3,
                            );
                        }
                    }
                    x += COLS[0].1;
                    // priority
                    let c = pos2(x + COLS[1].1 / 2.0, cy);
                    if task.priority > 0 {
                        let label = tr(PRIORITIES[task.priority.min(4) as usize].label);
                        let g =
                            p.layout_no_wrap(label.to_string(), FontId::proportional(13.0), t.text);
                        let left = c.x - (14.0 + g.size().x) / 2.0;
                        paint_flag(
                            &p,
                            pos2(left + 3.0, cy),
                            priority_color(task.priority, t),
                            11.0,
                        );
                        p.galley(pos2(left + 14.0, cy - g.size().y / 2.0), g, t.text);
                    } else {
                        p.text(
                            c,
                            Align2::CENTER_CENTER,
                            "\u{2014}",
                            FontId::proportional(13.0),
                            t.text3,
                        );
                    }
                    x += COLS[1].1;
                    // status
                    let st = self.data.status_of(task);
                    let cell = Rect::from_center_size(
                        pos2(x + COLS[2].1 / 2.0, cy),
                        vec2(COLS[2].1 - 12.0, 22.0),
                    );
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(cell)
                            .layout(Layout::top_down(Align::Center)),
                    );
                    status_pill(&mut child, &st, Sense::hover());
                    x += COLS[2].1;
                    // assignees
                    let aw = avatars_width(10.0, &task.assignees, &self.data);
                    if aw > 0.0 {
                        paint_avatars(
                            &p,
                            pos2(x + COLS[3].1 / 2.0 - aw / 2.0, cy),
                            10.0,
                            &task.assignees,
                            &self.data,
                            t.bg,
                        );
                    } else {
                        p.text(
                            pos2(x + COLS[3].1 / 2.0, cy),
                            Align2::CENTER_CENTER,
                            "\u{2014}",
                            FontId::proportional(13.0),
                            t.text3,
                        );
                    }
                    x += COLS[3].1;
                    p.text(
                        pos2(x + COLS[4].1 / 2.0, cy),
                        Align2::CENTER_CENTER,
                        edited_text(task.created),
                        FontId::proportional(13.0),
                        t.text2,
                    );
                    if row.clicked() {
                        self.actions.push(Action::Open(task.id));
                    }
                }
                ui.painter().rect_stroke(
                    Rect::from_min_max(head.min, pos2(head.left() + w, bottom)),
                    12.0,
                    Stroke::new(1.0_f32, t.border),
                    egui::StrokeKind::Inside,
                );
            });
    }
}
