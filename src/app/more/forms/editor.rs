// The form editor: the form's tile, name and description, a card per
// question (dragged by its handle to reorder, its kind picked from a menu
// that says where the answer goes, choices typed one by one), and the panel
// on the right with the List, the task an answer makes and who is told.

use super::*;

/// What a question's menu asked for, done after the cards are drawn.
enum QuestionAct {
    Up(usize),
    Down(usize),
    Copy(usize),
    Delete(usize),
}

/// The kinds a menu offers, in two groups: those that fill a task field and
/// those added to the description.
const KIND_GROUPS: [(&str, [&str; 5]); 2] = [
    (
        "Task info",
        ["name", "description", "priority", "due", "assignee"],
    ),
    (
        "More info",
        ["text", "dropdown", "number", "email", "checkbox"],
    ),
];

/// The menu of question kinds. `taken` are kinds another question already
/// has that a form has only one of (shown, not pickable); `current` is the
/// question's own kind. Returns the kind picked.
fn kind_menu(ui: &mut Ui, t: &Theme, taken: &[String], current: &str) -> Option<String> {
    let mut out = None;
    ui.set_width(372.0);
    for (gi, (group, kinds)) in KIND_GROUPS.iter().enumerate() {
        if gi > 0 {
            ui.separator();
        }
        ui.label(RichText::new(tr(group)).size(12.0).strong().color(t.text));
        for pair in kinds.chunks(2) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for k in pair {
                    let used = *k != current && taken.iter().any(|x| x == k);
                    let (r, resp) = ui.allocate_exact_size(
                        vec2(182.0, 36.0),
                        if used { Sense::hover() } else { Sense::click() },
                    );
                    name_it(&resp, &format!("Kind {}", form_kind_label(k)));
                    let p = ui.painter();
                    if *k == current {
                        p.rect_filled(r, 8.0, t.selected_bg);
                    } else if resp.hovered() && !used {
                        p.rect_filled(r, 8.0, t.hover);
                    }
                    let tile =
                        Rect::from_center_size(r.left_center() + vec2(18.0, 0.0), vec2(26.0, 26.0));
                    let (fg, bg) = if used {
                        (t.text3, t.hover)
                    } else if fills_task_field(k) {
                        (t.accent_text, t.accent_bg)
                    } else {
                        (t.text2, t.hover)
                    };
                    p.rect_filled(tile, 7.0, bg);
                    line_icon_scaled(p, kind_icon(k), tile.center(), fg, 0.85);
                    p.text(
                        pos2(tile.right() + 10.0, r.center().y),
                        Align2::LEFT_CENTER,
                        tr(form_kind_label(k)),
                        FontId::proportional(13.0),
                        if used { t.text3 } else { t.text },
                    );
                    if used {
                        p.text(
                            r.right_center() - vec2(8.0, 0.0),
                            Align2::RIGHT_CENTER,
                            tr("In use"),
                            FontId::proportional(11.0),
                            t.text3,
                        );
                    }
                    if resp.clicked() {
                        out = Some(k.to_string());
                        close_popup(ui);
                    }
                }
            });
        }
    }
    out
}

impl App {
    pub(super) fn form_editor(&mut self, ui: &mut Ui, fi: usize) {
        let t = self.t();
        // the settings beside the questions when both fit, else under them
        // (beside them in a small window the questions ran under the panel)
        let beside = ui.available_width() >= 760.0;
        if beside {
            egui::SidePanel::right("form_settings")
                .resizable(false)
                .exact_width(300.0)
                .frame(
                    egui::Frame::new()
                        .fill(t.bg)
                        .inner_margin(egui::Margin::symmetric(20, 20)),
                )
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("form_settings_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.form_settings(ui, fi));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.column_bg))
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("form_edit")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let w = ui.available_width();
                        let col = (w - 48.0).clamp(280.0, 720.0);
                        let pad = ((w - col) / 2.0).max(16.0);
                        ui.add_space(24.0);
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_width(col);
                                self.form_questions(ui, &t, fi);
                                if !beside {
                                    ui.add_space(24.0);
                                    egui::Frame::new()
                                        .fill(t.card)
                                        .stroke(Stroke::new(1.0_f32, t.border))
                                        .corner_radius(12)
                                        .inner_margin(egui::Margin::same(20))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            self.form_settings(ui, fi);
                                        });
                                }
                                ui.add_space(48.0);
                            });
                        });
                    });
            });
    }

    /// The editor's column: the title card, the question cards and Add question.
    fn form_questions(&mut self, ui: &mut Ui, t: &Theme, fi: usize) {
        let fid = self.data.extra.forms[fi].id;
        let mut ch = false;
        // the form's tile, name and description
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(20, 18))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::click());
                    name_it(&resp, "Change icon");
                    hover_ring(ui, &resp, r, 11.0);
                    paint_tile(ui.painter(), r, form_look(&self.data.extra.forms[fi], t));
                    let resp = resp.on_hover_text(tr("Change icon"));
                    if popup(ui, &resp, ui.id().with(("form_icon", fid)), 292.0, |ui| {
                        icon_picker(ui, t, &mut self.data.extra.forms[fi])
                    }) == Some(true)
                    {
                        ch = true;
                    }
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        let old = self.data.extra.forms[fi].name.clone();
                        let f = &mut self.data.extra.forms[fi];
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut f.name)
                                .font(FontId::proportional(22.0))
                                .frame(false)
                                .hint_text(tr("Form name"))
                                .desired_width(f32::INFINITY),
                        );
                        name_field(&r, "Form name");
                        if r.changed() {
                            // answers older apps sent, found by name, stay with
                            // their form: this one's under its old name, and
                            // another form's that has the new name
                            let new = std::mem::replace(&mut self.data.extra.forms[fi].name, old);
                            let old = self.data.extra.forms[fi].name.clone();
                            self.tie_old_answers(&old);
                            self.tie_old_answers(&new);
                            self.data.extra.forms[fi].name = new;
                            ch = true;
                        }
                        let f = &mut self.data.extra.forms[fi];
                        let r = ui.add(
                            egui::TextEdit::multiline(&mut f.description)
                                .frame(false)
                                .desired_rows(1)
                                .hint_text(tr("Add a description"))
                                .text_color(t.text2)
                                .desired_width(f32::INFINITY),
                        );
                        name_field(&r, "Form description");
                        ch |= r.changed();
                    });
                });
            });
        ui.add_space(18.0);
        let n = self.data.extra.forms[fi].fields.len();
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Questions")).size(14.0).color(t.text));
            ui.label(RichText::new(n.to_string()).size(14.0).color(t.text3));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(tr("Drag the handle to reorder"))
                        .size(12.0)
                        .color(t.text3),
                );
            });
        });
        ui.add_space(8.0);
        let mut rects = vec![];
        let mut act = None;
        for i in 0..n {
            let picked = self.sweep.forms.picked == Some(i);
            let out = egui::Frame::new()
                .fill(t.card)
                .stroke(if picked {
                    Stroke::new(1.5_f32, t.accent_text)
                } else {
                    Stroke::new(1.0_f32, t.border)
                })
                .corner_radius(12)
                .inner_margin(egui::Margin {
                    left: 6,
                    right: 12,
                    top: 12,
                    bottom: 10,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    self.question_card(ui, t, fi, i, &mut act)
                });
            ch |= out.inner;
            let rect = out.response.rect;
            if ui.input(|x| x.pointer.primary_pressed())
                && ui
                    .input(|x| x.pointer.interact_pos())
                    .is_some_and(|p| rect.contains(p))
            {
                self.sweep.forms.picked = Some(i);
            }
            rects.push(rect);
            ui.add_space(10.0);
        }
        // a question dragged by its handle goes where it is dropped
        if let Some(from) = self.sweep.forms.drag {
            let at = ui.input(|x| x.pointer.interact_pos());
            let to = at.map_or(from, |p| {
                rects.iter().filter(|r| p.y > r.center().y).count()
            });
            if to != from && to != from + 1 && !rects.is_empty() {
                let y = if to == 0 {
                    rects[0].top() - 5.0
                } else {
                    rects[to - 1].bottom() + 5.0
                };
                ui.painter()
                    .hline(rects[0].x_range(), y, Stroke::new(2.0_f32, t.accent_text));
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            if !ui.input(|x| x.pointer.primary_down()) {
                self.sweep.forms.drag = None;
                if to != from && to != from + 1 && from < n {
                    let f = &mut self.data.extra.forms[fi];
                    let q = f.fields.remove(from);
                    let to = if to > from { to - 1 } else { to };
                    f.fields.insert(to.min(f.fields.len()), q);
                    self.sweep.forms.picked = Some(to);
                    self.sweep.forms.choice = None;
                    ch = true;
                }
            }
        }
        if let Some(a) = act {
            let f = &mut self.data.extra.forms[fi];
            match a {
                QuestionAct::Up(i) if i > 0 => f.fields.swap(i, i - 1),
                QuestionAct::Down(i) if i + 1 < f.fields.len() => f.fields.swap(i, i + 1),
                QuestionAct::Copy(i) => {
                    let mut q = f.fields[i].clone();
                    // a copy of the one-per-form kinds would replace the
                    // original's answer, so it starts as short text
                    if one_per_form(&q.map) {
                        q.map = "text".into();
                    }
                    f.fields.insert(i + 1, q);
                }
                QuestionAct::Delete(i) => {
                    f.fields.remove(i);
                }
                _ => {}
            }
            self.sweep.forms.picked = None;
            self.sweep.forms.choice = None;
            ch = true;
        }
        ui.add_space(2.0);
        let add = dashed_button(ui, t, tr("Add question"), "Add question");
        let taken = self.taken_kinds(fi, None);
        if let Some(Some(k)) = popup(ui, &add, ui.id().with(("form_add_q", fid)), 372.0, |ui| {
            kind_menu(ui, t, &taken, "")
        }) {
            let f = &mut self.data.extra.forms[fi];
            f.fields.push(FormField {
                label: tr(form_kind_label(&k)).into(),
                map: k,
                ..Default::default()
            });
            self.sweep.forms.picked = Some(f.fields.len() - 1);
            ch = true;
        }
        if ch {
            self.form_touched(fid);
        }
    }

    /// The one-per-form kinds questions other than `except` already have.
    fn taken_kinds(&self, fi: usize, except: Option<usize>) -> Vec<String> {
        self.data.extra.forms[fi]
            .fields
            .iter()
            .enumerate()
            .filter(|(i, q)| Some(*i) != except && one_per_form(&q.map))
            .map(|(_, q)| q.map.clone())
            .collect()
    }

    /// One question's card: handle, number, text and kind; where the answer
    /// goes, Required and the menu; a dropdown's choices. True on a change.
    fn question_card(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        fi: usize,
        i: usize,
        act: &mut Option<QuestionAct>,
    ) -> bool {
        let fid = self.data.extra.forms[fi].id;
        let n = self.data.extra.forms[fi].fields.len();
        let picked = self.sweep.forms.picked == Some(i);
        let mut ch = false;
        let taken = self.taken_kinds(fi, Some(i));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // the handle
            let (r, grip) = ui.allocate_exact_size(vec2(20.0, 32.0), Sense::drag());
            name_it(&grip, &format!("Drag question {}", i + 1));
            if grip.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                ui.painter().rect_filled(r, 5.0, t.hover);
            }
            for dy in [-5.0, 0.0, 5.0] {
                for dx in [-2.5, 2.5] {
                    ui.painter()
                        .circle_filled(r.center() + vec2(dx, dy), 1.3, t.text3);
                }
            }
            if grip.drag_started() {
                self.sweep.forms.drag = Some(i);
            }
            // the number
            let (nr, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            ui.painter().circle_filled(
                nr.center(),
                11.0,
                if picked { t.accent_text } else { t.hover },
            );
            ui.painter().text(
                nr.center(),
                Align2::CENTER_CENTER,
                (i + 1).to_string(),
                FontId::proportional(12.0),
                if picked { t.on_primary } else { t.text2 },
            );
            let kind_w = 172.0;
            let fld = &mut self.data.extra.forms[fi].fields[i];
            let r = ui.add(
                field(&mut fld.label)
                    .min_size(vec2(0.0, 32.0))
                    .hint_text(tr("Question"))
                    // the width is the text area's: the field adds its margin (16) and
                    // the row a gap (6) before the kind
                    .desired_width((ui.available_width() - kind_w - 22.0).max(80.0)),
            );
            name_field(&r, &format!("Question {}", i + 1));
            ch |= r.changed();
            let k = fld.map.clone();
            let icon_color = if fills_task_field(&k) {
                t.accent_text
            } else {
                t.text2
            };
            let kind = select_button(
                ui,
                t,
                kind_icon(&k),
                icon_color,
                tr(form_kind_label(&k)),
                t.text,
                vec2(kind_w, 32.0),
                &format!("Question {} kind", i + 1),
            );
            if let Some(Some(new)) = popup(
                ui,
                &kind,
                ui.id().with(("form_kind", fid, i)),
                372.0,
                |ui| kind_menu(ui, t, &taken, &k),
            ) {
                if new != k {
                    self.data.extra.forms[fi].fields[i].map = new;
                    ch = true;
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(52.0);
            let hint = kind_hint(&self.data.extra.forms[fi].fields[i]);
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
            line_icon_scaled(ui.painter(), "arrow_right", r.center(), t.text3, 0.75);
            // wrapped before Required and the menu, so it never runs under them
            let room = (ui.available_width() - 150.0).max(60.0);
            ui.scope(|ui| {
                ui.set_max_width(room);
                ui.add(egui::Label::new(RichText::new(hint).size(12.0).color(t.text2)).wrap());
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let more = icon_button(
                    ui,
                    t,
                    "dots",
                    "",
                    &format!("Question {} menu", i + 1),
                    false,
                );
                popup(
                    ui,
                    &more,
                    ui.id().with(("form_q_menu", fid, i)),
                    190.0,
                    |ui| {
                        let rows: [(&str, String, &str, bool, QuestionAct); 4] = [
                            (
                                "arrow_up_left",
                                format!("Move question {} up", i + 1),
                                "Move up",
                                i > 0,
                                QuestionAct::Up(i),
                            ),
                            (
                                "arrow_right",
                                format!("Move question {} down", i + 1),
                                "Move down",
                                i + 1 < n,
                                QuestionAct::Down(i),
                            ),
                            (
                                "templates",
                                format!("Duplicate question {}", i + 1),
                                "Duplicate",
                                true,
                                QuestionAct::Copy(i),
                            ),
                            (
                                "trash",
                                format!("Delete question {}", i + 1),
                                "Delete question",
                                true,
                                QuestionAct::Delete(i),
                            ),
                        ];
                        for (icon, name, label, ok, a) in rows {
                            let danger = icon == "trash";
                            let r = ui
                                .add_enabled_ui(ok, |ui| {
                                    menu_row_named(ui, t, &name, tr(label), false, |p, c| {
                                        line_icon(
                                            p,
                                            icon,
                                            c,
                                            if danger { t.danger } else { t.text2 },
                                        );
                                    })
                                })
                                .inner;
                            if r.clicked() && ok {
                                *act = Some(a);
                                close_popup(ui);
                            }
                        }
                    },
                );
                ui.add_space(6.0);
                let fld = &mut self.data.extra.forms[fi].fields[i];
                let req = ui.add(
                    egui::Label::new(
                        RichText::new(tr("Required"))
                            .size(12.0)
                            .color(if fld.required { t.text } else { t.text2 }),
                    )
                    .sense(Sense::click()),
                );
                let sw = switch(ui, t, &mut fld.required, &format!("Required {}", i + 1));
                if req.clicked() {
                    fld.required = !fld.required;
                }
                ch |= sw.clicked() || req.clicked();
            });
        });
        if self.data.extra.forms[fi].fields[i].map == "dropdown" {
            ui.add_space(4.0);
            ch |= self.question_choices(ui, t, fi, i);
        }
        ch
    }

    /// A dropdown question's choices: a chip each with its ×, and a field
    /// where Enter adds what was typed (several at once, split at commas).
    fn question_choices(&mut self, ui: &mut Ui, t: &Theme, fi: usize, i: usize) -> bool {
        let mut ch = false;
        let editing = self.sweep.forms.choice.as_ref().is_some_and(|c| c.0 == i);
        let opts: Vec<String> = self.data.extra.forms[fi].fields[i]
            .options
            .iter()
            .filter(|o| !o.trim().is_empty())
            .cloned()
            .collect();
        let mut remove = None;
        let mut add_now = false;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            ui.add_space(52.0);
            ui.label(RichText::new(tr("Choices")).size(12.0).color(t.text2));
            for o in &opts {
                egui::Frame::new()
                    .fill(t.column_bg)
                    .stroke(Stroke::new(1.0_f32, t.border))
                    .corner_radius(14)
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 3,
                        top: 2,
                        bottom: 2,
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(o).size(13.0).color(t.text));
                            let x = ui.add(ghost(egui::Button::new(
                                RichText::new("\u{00D7}").size(13.0).color(t.text3),
                            )));
                            name_it(&x, &format!("Remove choice {}", o));
                            if x.clicked() {
                                remove = Some(o.clone());
                            }
                        });
                    });
            }
            if editing {
                let mut text = self
                    .sweep
                    .forms
                    .choice
                    .as_ref()
                    .map(|c| c.1.clone())
                    .unwrap_or_default();
                let r = ui.add(
                    field(&mut text)
                        .hint_text(tr("Type a choice"))
                        .desired_width(150.0),
                );
                name_field(&r, &format!("New choice {}", i + 1));
                focus_once(ui, &r);
                if r.changed() {
                    if let Some(c) = self.sweep.forms.choice.as_mut() {
                        c.1 = text.clone();
                        c.2 = None;
                    }
                }
                if r.lost_focus() && ui.input(|x| x.key_pressed(egui::Key::Enter)) {
                    add_now = true;
                    r.request_focus();
                }
                if ui.input(|x| x.key_pressed(egui::Key::Escape)) {
                    self.sweep.forms.choice = None;
                }
                if row_button(ui, tr("Add")).clicked() {
                    add_now = true;
                }
                let x = icon_button(
                    ui,
                    t,
                    "close",
                    "",
                    &format!("Stop adding choices {}", i + 1),
                    false,
                );
                if x.clicked() {
                    self.sweep.forms.choice = None;
                }
            } else {
                let b = icon_button(
                    ui,
                    t,
                    "plus",
                    tr("Add choice"),
                    &format!("Add choice {}", i + 1),
                    false,
                );
                if b.clicked() {
                    self.sweep.forms.choice = Some((i, String::new(), None));
                }
            }
        });
        if let Some(o) = remove {
            self.data.extra.forms[fi].fields[i].options =
                opts.iter().filter(|x| **x != o).cloned().collect();
            ch = true;
        }
        if add_now {
            let text = self
                .sweep
                .forms
                .choice
                .as_ref()
                .map(|c| c.1.clone())
                .unwrap_or_default();
            let parts: Vec<String> = text
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
            let mut seen = opts.clone();
            let mut dup = None;
            for p in &parts {
                if seen.contains(p) {
                    dup = Some(p.clone());
                    break;
                }
                seen.push(p.clone());
            }
            if let Some(d) = dup {
                if let Some(c) = self.sweep.forms.choice.as_mut() {
                    c.2 = Some(trf!("\"{}\" is already a choice.", d));
                }
            } else if !parts.is_empty() {
                let left: Vec<String> = self.data.extra.forms[fi].fields[i]
                    .options
                    .iter()
                    .filter(|o| !o.trim().is_empty())
                    .cloned()
                    .collect();
                self.data.extra.forms[fi].fields[i].options =
                    left.into_iter().chain(parts).collect();
                if let Some(c) = self.sweep.forms.choice.as_mut() {
                    c.1.clear();
                    c.2 = None;
                }
                ch = true;
            }
        }
        if opts.is_empty() && !editing && !add_now {
            ui.horizontal(|ui| {
                ui.add_space(52.0);
                ui.label(
                    RichText::new(tr(
                        "No choices yet. People filling the form out can't pick anything.",
                    ))
                    .size(12.0)
                    .color(WARN_ORANGE),
                );
            });
        }
        if let Some((_, _, problem)) = self.sweep.forms.choice.as_ref().filter(|c| c.0 == i) {
            ui.horizontal(|ui| {
                ui.add_space(52.0);
                match problem {
                    Some(p) => {
                        ui.label(RichText::new(p).size(12.0).color(t.danger));
                    }
                    None => {
                        ui.label(
                            RichText::new(tr(
                                "Press Enter to add. Separate with commas to add several at once.",
                            ))
                            .size(12.0)
                            .color(t.text3),
                        );
                    }
                }
            });
        }
        ch
    }

    /// The editor's settings, beside the questions or under them: the List
    /// new tasks go to, the task an answer makes, who is told, who made the
    /// form, and Delete.
    fn form_settings(&mut self, ui: &mut Ui, fi: usize) {
        let t = self.t();
        let f = self.data.extra.forms[fi].clone();
        // Lists answers can go to (not archived); an archived one already
        // picked stays, marked, so the form says where answers go now
        let mut lists: Vec<(u64, String)> = self.all_list_paths();
        if self.data.list_archived(f.list_id) && self.data.space_of_list(f.list_id).is_some() {
            lists.push((
                f.list_id,
                trf!("{} (archived)", self.data.list_name(f.list_id)),
            ));
        }
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            panel_heading(ui, &t, tr("List for new tasks"));
            let cur = lists.iter().find(|l| l.0 == f.list_id).map(|l| l.1.clone());
            let (text, color) = match &cur {
                Some(c) => (c.clone(), t.text),
                None => (tr("Choose a List").to_string(), WARN_ORANGE),
            };
            let w = ui.available_width();
            let r = select_button(
                ui,
                &t,
                "list",
                t.text2,
                &text,
                color,
                vec2(w, 34.0),
                "Pick the List for new tasks",
            );
            let picked = popup(ui, &r, ui.id().with(("form_list", f.id)), w, |ui| {
                let mut pick = None;
                for (id, n) in &lists {
                    if menu_row_plain(ui, &t, n, *id == f.list_id).clicked() {
                        pick = Some(*id);
                        close_popup(ui);
                    }
                }
                pick
            });
            if let Some(Some(id)) = picked {
                self.data.extra.forms[fi].list_id = id;
                self.form_touched(f.id);
            }
            ui.label(
                RichText::new(tr("Each answer adds one task to this List."))
                    .size(12.0)
                    .color(t.text2),
            );
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            panel_heading(ui, &t, tr("Will be created"));
            egui::Frame::new()
                .stroke(Stroke::new(1.0_f32, t.border))
                .corner_radius(10)
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for (k, label) in [
                        ("name", "Task name"),
                        ("priority", "Priority"),
                        ("description", "Description"),
                        ("due", "Due date"),
                        ("assignee", "Assignee"),
                    ] {
                        ui.horizontal_top(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(72.0, 20.0), Sense::hover());
                            ui.painter().text(
                                r.left_center(),
                                Align2::LEFT_CENTER,
                                tr(label),
                                FontId::proportional(12.0),
                                t.text2,
                            );
                            let qs: Vec<(usize, &FormField)> = f
                                .fields
                                .iter()
                                .enumerate()
                                .filter(|(_, q)| {
                                    q.map == k || (k == "description" && !fills_task_field(&q.map))
                                })
                                .collect();
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                                if qs.is_empty() {
                                    ui.label(
                                        RichText::new(tr("No question")).size(12.0).color(t.text3),
                                    );
                                }
                                for (qi, q) in qs {
                                    let own = q.map == k;
                                    let label = format!(
                                        "{} \u{00B7} {}",
                                        qi + 1,
                                        crate::widgets::one_line(&q.label)
                                    );
                                    egui::Frame::new()
                                        .fill(if own { t.accent_bg } else { t.hover })
                                        .corner_radius(6)
                                        .inner_margin(egui::Margin::symmetric(7, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(label).size(12.0).color(if own {
                                                    t.accent_text
                                                } else {
                                                    t.text2
                                                }),
                                            );
                                        });
                                }
                            });
                        });
                    }
                });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            panel_heading(ui, &t, tr("Notifications"));
            let owner = self.data.member(f.owner);
            let text = if f.owner == self.data.me {
                tr("New answers notify you, the form's maker.").to_string()
            } else if let Some(m) = owner {
                trf!("New answers notify {}, who made this form.", m.shown())
            } else {
                tr("New answers notify the form's maker.").to_string()
            };
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                line_icon(ui.painter(), "bell", r.center(), t.text2);
                ui.add(egui::Label::new(RichText::new(text).size(13.0).color(t.text)).wrap());
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);
            panel_heading(ui, &t, tr("Made by"));
            ui.horizontal(|ui| match owner {
                Some(m) => {
                    let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                    paint_avatar(ui.painter(), r.center(), 10.0, m, t.bg);
                    let name = if f.owner == self.data.me {
                        trf!("{} (you)", m.shown())
                    } else {
                        m.shown().to_string()
                    };
                    ui.label(RichText::new(name).size(13.0).color(t.text));
                }
                None => {
                    ui.label(RichText::new("\u{2014}").size(13.0).color(t.text3));
                }
            });
            ui.add_space(20.0);
            if icon_button_ex(
                ui,
                &t,
                "trash",
                tr("Delete form"),
                "Delete form",
                false,
                0.0,
                t.danger,
                t.danger,
            )
            .clicked()
            {
                self.modal = Some(Modal::ConfirmRemove {
                    kind: "form",
                    id: f.id,
                    name: form_title(&f),
                });
            }
            ui.label(
                RichText::new(tr("It goes to Trash. The tasks it made stay."))
                    .size(12.0)
                    .color(t.text3),
            );
        });
    }
}
