// Filling out a form: the form as the person answering sees it, each
// question with a control for its kind, a problem shown under the question
// it is about, and Submit, which makes the task.

use super::*;

impl App {
    pub(super) fn form_fill_page(&mut self, ui: &mut Ui, fi: usize) {
        let t = self.t();
        let f = self.data.extra.forms[fi].clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.column_bg))
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("form_fill")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let w = ui.available_width();
                        let col = (w - 48.0).clamp(280.0, 640.0);
                        let pad = ((w - col) / 2.0).max(16.0);
                        ui.add_space(24.0);
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_width(col);
                                self.form_fill_body(ui, &t, &f);
                                ui.add_space(48.0);
                            });
                        });
                    });
            });
    }

    fn form_fill_body(&mut self, ui: &mut Ui, t: &Theme, f: &Form) {
        let path = self
            .all_list_paths()
            .into_iter()
            .find(|l| l.0 == f.list_id)
            .map(|l| l.1)
            .or_else(|| {
                self.data
                    .space_of_list(f.list_id)
                    .map(|_| self.data.list_name(f.list_id))
            });
        // what Submit does, said before the form
        let (text, fg, bg) = match &path {
            Some(p) => (
                trf!(
                    "This is the real form, not a preview: submitting makes a task in {}.",
                    p
                ),
                t.accent_text,
                t.accent_bg,
            ),
            None => (
                tr("This form has no List for its answers. Choose one under Edit.").to_string(),
                WARN_ORANGE,
                WARN_ORANGE.gamma_multiply(0.12),
            ),
        };
        egui::Frame::new()
            .fill(bg)
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                    line_icon_scaled(ui.painter(), "help", r.center(), fg, 0.9);
                    ui.add(egui::Label::new(RichText::new(text).size(13.0).color(fg)).wrap());
                });
            });
        ui.add_space(14.0);
        let members: Vec<(u64, String)> = self
            .data
            .members
            .iter()
            .map(|m| (m.id, m.shown().to_string()))
            .collect();
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(14)
            .inner_margin(egui::Margin::same(28))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.label(RichText::new(&f.name).size(22.0).color(t.text));
                if !f.description.trim().is_empty() {
                    ui.label(RichText::new(&f.description).size(14.0).color(t.text2));
                }
                if f.fields.iter().any(|q| q.required) {
                    ui.label(
                        RichText::new(tr("* marks a question that needs an answer."))
                            .size(12.0)
                            .color(t.text3),
                    );
                }
                for (i, fld) in f.fields.iter().enumerate() {
                    ui.add_space(14.0);
                    self.form_fill_question(ui, t, f.id, i, fld, &members);
                }
                ui.add_space(18.0);
                ui.separator();
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(tr("Submitting makes one task and tells the form's maker."))
                            .size(12.0)
                            .color(t.text2),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let submit = ui.add(Tint(
                            primary_btn(t, tr("Submit")).min_size(vec2(88.0, 36.0)),
                        ));
                        name_it(&submit, "Submit");
                        let clear = ui.add(
                            egui::Button::new(tr("Clear"))
                                .corner_radius(6)
                                .min_size(vec2(0.0, 36.0)),
                        );
                        name_it(&clear, "Clear answers");
                        if clear.clicked() {
                            self.form_fill.clear();
                            self.sweep.form_error = None;
                            self.sweep.forms.error_at = None;
                        }
                        if submit.clicked() {
                            self.form_submit(f);
                        }
                    });
                });
            });
    }

    /// One question: its text (with * when required), the control for its
    /// kind, and the problem with its answer when Submit found one.
    fn form_fill_question(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        fid: u64,
        i: usize,
        fld: &FormField,
        members: &[(u64, String)],
    ) {
        let bad = self.sweep.forms.error_at == Some(i);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new(&fld.label).size(14.0).strong().color(t.text));
            if fld.required {
                ui.label(RichText::new("*").size(14.0).color(t.danger));
            }
        });
        let name = format!("Answer {}", i + 1);
        let v = self.form_fill.entry(i).or_default();
        let w = ui.available_width();
        ui.scope(|ui| {
            if bad {
                let s = Stroke::new(1.5_f32, t.danger);
                let wv = &mut ui.visuals_mut().widgets;
                wv.inactive.bg_stroke = s;
                wv.hovered.bg_stroke = s;
                wv.active.bg_stroke = s;
                ui.visuals_mut().selection.stroke = s;
            }
            match fld.map.as_str() {
                "priority" => {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let bw = ((w - 24.0) / 4.0).max(60.0);
                        for (pi, label) in [(1u8, "Urgent"), (2, "High"), (3, "Normal"), (4, "Low")]
                        {
                            let (r, resp) = ui.allocate_exact_size(vec2(bw, 38.0), Sense::click());
                            name_it(&resp, &format!("{} {}", name, label));
                            let on = v.as_str() == label;
                            let p = ui.painter();
                            p.rect_filled(
                                r,
                                8.0,
                                if on {
                                    t.hover
                                } else if resp.hovered() {
                                    t.column_bg
                                } else {
                                    t.bg
                                },
                            );
                            p.rect_stroke(
                                r,
                                8.0,
                                if on {
                                    Stroke::new(1.5_f32, t.text)
                                } else if bad {
                                    Stroke::new(1.5_f32, t.danger)
                                } else {
                                    Stroke::new(1.0_f32, t.border)
                                },
                                egui::StrokeKind::Inside,
                            );
                            let g = p.layout_no_wrap(
                                tr(label).to_string(),
                                FontId::proportional(14.0),
                                t.text,
                            );
                            let content = 12.0 + 8.0 + g.size().x;
                            let left = r.center().x - content / 2.0;
                            paint_flag(
                                p,
                                pos2(left + 3.0, r.center().y),
                                priority_color(pi, t),
                                12.0,
                            );
                            p.galley(
                                pos2(left + 20.0, r.center().y - g.size().y / 2.0),
                                g,
                                t.text,
                            );
                            if resp.clicked() {
                                *v = if on { String::new() } else { label.to_string() };
                            }
                        }
                    });
                }
                "due" => {
                    let mut d = NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
                    let text = d.map(fmt_date).unwrap_or_else(|| tr("Pick a date").into());
                    let color = if d.is_some() { t.text } else { t.text3 };
                    let b = select_button(
                        ui,
                        t,
                        "calendar",
                        t.text2,
                        &text,
                        color,
                        vec2(w, 38.0),
                        &name,
                    );
                    if let Some(true) = popup(
                        ui,
                        &b,
                        ui.id().with(("form_due", fid, i)),
                        DATE_MENU_W,
                        |ui| date_menu(ui, t, ui.id().with(("form_due_cal", fid, i)), &mut d),
                    ) {
                        *v = d
                            .map(|d| d.format("%Y-%m-%d").to_string())
                            .unwrap_or_default();
                    }
                }
                "dropdown" => {
                    // a choice removed on Edit since it was picked: ask again
                    if !fld.options.iter().any(|o| o == v) {
                        v.clear();
                    }
                    let (text, color) = if v.is_empty() {
                        (tr("Choose").to_string(), t.text3)
                    } else {
                        (v.clone(), t.text)
                    };
                    let b = select_button(ui, t, "", t.text2, &text, color, vec2(w, 38.0), &name);
                    popup(ui, &b, ui.id().with(("form_dd", fid, i)), w, |ui| {
                        for o in fld.options.iter().filter(|o| !o.trim().is_empty()) {
                            let on = v == o;
                            if menu_row_named(ui, t, &format!("Choice {}", o), o, on, |_, _| {})
                                .clicked()
                            {
                                *v = o.clone();
                                close_popup(ui);
                            }
                        }
                    });
                }
                "assignee" => {
                    let cur = v
                        .parse::<u64>()
                        .ok()
                        .and_then(|id| members.iter().find(|m| m.0 == id))
                        .map(|m| m.1.clone());
                    let (text, color) = match cur {
                        Some(n) => (n, t.text),
                        None => (tr("Choose someone").to_string(), t.text3),
                    };
                    let b =
                        select_button(ui, t, "person", t.text2, &text, color, vec2(w, 38.0), &name);
                    popup(ui, &b, ui.id().with(("form_who", fid, i)), w, |ui| {
                        for (id, n) in members {
                            let on = *v == id.to_string();
                            let m = self.data.member(*id).cloned();
                            let r = menu_row_named(ui, t, &format!("Pick {}", n), n, on, |p, c| {
                                if let Some(m) = &m {
                                    paint_avatar(p, c, 8.0, m, t.bg);
                                }
                            });
                            if r.clicked() {
                                *v = id.to_string();
                                close_popup(ui);
                            }
                        }
                    });
                }
                "checkbox" => {
                    let mut on = v == "Yes";
                    let cb = check(ui, &mut on, tr("Yes"));
                    name_it(&cb, &name);
                    if cb.changed() {
                        *v = if on { "Yes".into() } else { String::new() };
                    }
                }
                "description" => {
                    let r = ui.add(field_multi(v).desired_rows(4).desired_width(f32::INFINITY));
                    name_it(&r, &name);
                }
                k => {
                    let hint = match k {
                        "number" => "0",
                        "email" => "name@example.com",
                        _ => "",
                    };
                    let r = ui.add(
                        field(v)
                            .min_size(vec2(0.0, 38.0))
                            .hint_text(hint)
                            .desired_width(f32::INFINITY),
                    );
                    name_it(&r, &name);
                }
            }
        });
        if bad {
            if let Some(e) = &self.sweep.form_error {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    line_icon_scaled(ui.painter(), "help", r.center(), t.danger, 0.75);
                    ui.label(RichText::new(tr(e)).size(12.0).color(t.danger));
                });
            }
        }
    }

    /// Submit: checks the answers, then makes the task in the form's List,
    /// or says what stopped it.
    fn form_submit(&mut self, f: &Form) {
        match form_check(f, &self.form_fill) {
            Err((i, e)) => {
                self.sweep.form_error = Some(e);
                self.sweep.forms.error_at = Some(i);
            }
            Ok(_) if self.data.space_of_list(f.list_id).is_none() => {
                // the List was never chosen or has been deleted
                self.toast(tr(
                    "This form has no List for its answers. Choose one under Edit.",
                ));
                self.sweep.form_error = None;
                self.sweep.forms.error_at = None;
                self.form_tab = FormTab::Edit;
            }
            Ok(a) => {
                let name = if a.name.trim().is_empty() {
                    format!("{} submission", f.name)
                } else {
                    a.name
                };
                self.actions.push(Action::Add {
                    list: f.list_id,
                    name,
                    status: None,
                    priority: a.priority,
                    assignees: a.assignees,
                    due: a.due,
                    parent: None,
                    open: false,
                });
                self.pending.description = Some(a.description.trim().to_string());
                // tells the form's owner (notify.rs); shared history, so in English
                self.pending.log = Some(format!("submitted the form {}", f.name));
                self.pending.form = Some(f.id);
                self.form_fill.clear();
                self.sweep.form_error = None;
                self.sweep.forms.error_at = None;
                self.toast(trf!(
                    "Submitted. A task was created in {}",
                    self.data.list_name(f.list_id)
                ));
            }
        }
    }
}
