// Forms: the field kinds, collecting the answers, and the form page.

use super::*;

/// Question kinds of a form: what the answer fills in.
pub const FORM_KINDS: [(&str, &str); 10] = [
    ("name", "Task name"),
    ("description", "Description"),
    ("priority", "Priority"),
    ("due", "Due date"),
    ("assignee", "Assignee"),
    ("text", "Short text"),
    ("dropdown", "Dropdown"),
    ("number", "Number"),
    ("email", "Email"),
    ("checkbox", "Checkbox"),
];

fn form_kind_label(k: &str) -> &'static str {
    FORM_KINDS
        .iter()
        .find(|x| x.0 == k)
        .map(|x| x.1)
        .unwrap_or("Short text")
}

/// What a filled-in form makes: the new task's fields.
#[derive(Debug, Default)]
pub struct FormAnswers {
    pub name: String,
    pub description: String,
    pub priority: u8,
    pub due: Option<NaiveDate>,
    pub assignees: Vec<u64>,
}

/// Checks the answers (required, number, email) and turns them into task fields.
pub fn form_answers(f: &Form, fill: &HashMap<usize, String>) -> Result<FormAnswers, String> {
    let mut a = FormAnswers::default();
    for (i, fld) in f.fields.iter().enumerate() {
        let v = fill
            .get(&i)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if v.is_empty() {
            if fld.required {
                return Err(trf!("Please answer \"{}\".", fld.label));
            }
            continue;
        }
        match fld.map.as_str() {
            "name" => a.name = v,
            "description" => a.description.push_str(&v),
            "priority" => {
                a.priority = PRIORITIES.iter().position(|p| p.label == v).unwrap_or(0) as u8
            }
            "due" => match NaiveDate::parse_from_str(&v, "%Y-%m-%d") {
                Ok(d) => a.due = Some(d),
                Err(_) => {
                    return Err(trf!(
                        "\"{}\" isn't a date. Write it as YYYY-MM-DD.",
                        fld.label
                    ))
                }
            },
            "assignee" => a.assignees.extend(v.parse::<u64>().ok()),
            k => {
                if k == "number" && v.parse::<f64>().is_err() {
                    return Err(trf!("\"{}\" needs a number.", fld.label));
                }
                if k == "email" && !(v.contains('@') && v.contains('.')) {
                    return Err(trf!("\"{}\" needs an email address.", fld.label));
                }
                a.description.push_str(&format!("\n{}: {}", fld.label, v));
            }
        }
    }
    Ok(a)
}

// ---------------------------------------------------------------- forms

impl App {
    pub(crate) fn form_detail(&mut self, ui: &mut Ui) {
        let t = self.t();
        // a message about one form doesn't stay on another
        if self.sweep.form_for != self.form_open {
            self.sweep.form_error = None;
            self.sweep.form_for = self.form_open;
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::symmetric(32, 20))).show_inside(ui, |ui| {
            let Some(fi) = self.form_open.and_then(|id| self.data.extra.forms.iter().position(|f| f.id == id)) else {
                ui.label(RichText::new(tr("Create a form to collect requests; each answer becomes a task.")).color(t.text2));
                return;
            };
            // Lists answers can go to (not archived); an archived one already
            // picked stays, marked, so the form says where answers go now
            let mut lists: Vec<(u64, String)> = self.all_list_paths();
            let picked = self.data.extra.forms[fi].list_id;
            if self.data.list_archived(picked) && self.data.space_of_list(picked).is_some() {
                lists.push((picked, trf!("{} (archived)", self.data.list_name(picked))));
            }
            let members: Vec<(u64, String)> = self.data.members.iter().map(|m| (m.id, m.shown().to_string())).collect();
            // answers typed into one form don't carry over to another
            let fid = self.data.extra.forms[fi].id;
            let for_id = egui::Id::new("form_fill_for");
            if ui.data(|d| d.get_temp::<u64>(for_id)) != Some(fid) {
                self.form_fill.clear();
                ui.data_mut(|d| d.insert_temp(for_id, fid));
            }
            ui.horizontal(|ui| {
                // the mode shown is outlined
                for (fill, kind, label) in [(false, "edit", "Edit"), (true, "eye", "Fill out")] {
                    let sel = self.form_mode_fill == fill;
                    if icon_button_ex(ui, &t, kind, tr(label), label, sel, 0.0, if sel { t.text } else { t.text2 }, t.text2).clicked() {
                        self.form_mode_fill = fill;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| self.star_toggle(ui, format!("form:{}", fid)));
            });
            ui.separator();
            if !self.form_mode_fill {
                let mut delete = false;
                let f = &mut self.data.extra.forms[fi];
                let mut ch = false;
                ch |= ui.add(egui::TextEdit::singleline(&mut f.name).font(FontId::proportional(24.0)).frame(false).desired_width(f32::INFINITY)).changed();
                ch |= ui.add(field_multi(&mut f.description).desired_rows(2).desired_width(f32::INFINITY)).changed();
                ui.horizontal(|ui| {
                    ui.label(tr("Answers create tasks in"));
                    let cur = lists.iter().find(|l| l.0 == f.list_id).map(|l| l.1.clone()).unwrap_or_else(|| tr("choose a list").into());
                    egui::ComboBox::from_id_salt("form_list_sel").selected_text(cur).show_ui(ui, |ui| {
                        for (id, n) in &lists {
                            ch |= ui.selectable_value(&mut f.list_id, *id, n).changed();
                        }
                    });
                });
                ui.add_space(8.0);
                ui.label(RichText::new(tr("Questions")).strong());
                let mut remove = None;
                let mut swap = None;
                let count = f.fields.len();
                for (i, fld) in f.fields.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ch |= ui.add(field(&mut fld.label).desired_width(220.0)).changed();
                        egui::ComboBox::from_id_salt(("map", i)).selected_text(tr(form_kind_label(&fld.map))).show_ui(ui, |ui| {
                            for (k, n) in FORM_KINDS {
                                ch |= ui.selectable_value(&mut fld.map, k.to_string(), tr(n)).changed();
                            }
                        });
                        let rq = check(ui, &mut fld.required, tr("Required"));
                        name_it(&rq, &format!("Required {}", i + 1));
                        ch |= rq.changed();
                        let up = ui.add_enabled(i > 0, ghost(egui::Button::new("\u{2191}")));
                        name_it(&up, &format!("Move question {} up", i + 1));
                        if up.clicked() {
                            swap = Some((i, i - 1));
                        }
                        let down = ui.add_enabled(i + 1 < count, ghost(egui::Button::new("\u{2193}")));
                        name_it(&down, &format!("Move question {} down", i + 1));
                        if down.clicked() {
                            swap = Some((i, i + 1));
                        }
                        if ui.add(ghost(egui::Button::new(RichText::new("\u{00D7}").color(t.text3)))).clicked() {
                            remove = Some(i);
                        }
                    });
                    if fld.map == "dropdown" {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(RichText::new(tr("Choices")).small().color(t.text2));
                            // empty pieces are kept while typing so "a, " can grow into "a, b"
                            let mut text = fld.options.join(", ");
                            let r = ui.add(field(&mut text).hint_text(tr("Red, Green, Blue")).desired_width(300.0));
                            name_it(&r, &format!("Choices {}", i + 1));
                            if r.changed() {
                                fld.options = text.split(',').map(|x| x.trim().to_string()).collect();
                                ch = true;
                            }
                        });
                    }
                }
                if let Some(i) = remove {
                    f.fields.remove(i);
                    ch = true;
                }
                if let Some((a, b)) = swap {
                    f.fields.swap(a, b);
                    ch = true;
                }
                ui.horizontal(|ui| {
                    if ui.button(tr("+ Add question")).clicked() {
                        f.fields.push(FormField { label: tr("Question").into(), map: "text".into(), ..Default::default() });
                        ch = true;
                    }
                    if ui.add(ghost(egui::Button::new(RichText::new(tr("Delete form")).color(t.danger)))).clicked() {
                        delete = true;
                    }
                });
                if ch {
                    self.dirty = true;
                }
                if delete {
                    let f = &self.data.extra.forms[fi];
                    self.modal = Some(Modal::ConfirmRemove { kind: "form", id: f.id, name: f.name.clone() });
                }
            } else {
                let f = self.data.extra.forms[fi].clone();
                egui::Frame::new().stroke(Stroke::new(1.0_f32, t.border)).corner_radius(12).inner_margin(egui::Margin::same(20)).show(ui, |ui| {
                    ui.set_width(ui.available_width().min(640.0));
                    ui.label(RichText::new(&f.name).size(22.0));
                    ui.label(RichText::new(&f.description).color(t.text2));
                    ui.add_space(10.0);
                    for (i, fld) in f.fields.iter().enumerate() {
                        ui.label(RichText::new(if fld.required { format!("{} *", fld.label) } else { fld.label.clone() }).strong());
                        let v = self.form_fill.entry(i).or_default();
                        match fld.map.as_str() {
                            "priority" => {
                                egui::ComboBox::from_id_salt(("fp", i)).selected_text(if v.is_empty() { tr("Choose").to_string() } else { tr(v).to_string() }).show_ui(ui, |ui| {
                                    for p in ["Urgent", "High", "Normal", "Low"] {
                                        ui.selectable_value(v, p.to_string(), tr(p));
                                    }
                                });
                            }
                            "due" => {
                                let mut d = NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
                                let b = icon_button(ui, &t, "calendar", &d.map(fmt_date).unwrap_or_else(|| tr("Pick a date").into()), "", true);
                                name_it(&b, &format!("Answer {}", i + 1));
                                if let Some(true) = popup(ui, &b, ui.id().with(("form_due", fid, i)), 380.0, |ui| date_menu(ui, &t, ui.id().with(("form_due_cal", fid, i)), &mut d)) {
                                    *v = d.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
                                }
                            }
                            "dropdown" => {
                                egui::ComboBox::from_id_salt(("fd", i)).selected_text(if v.is_empty() { tr("Choose").to_string() } else { v.clone() }).show_ui(ui, |ui| {
                                    for o in fld.options.iter().filter(|o| !o.is_empty()) {
                                        ui.selectable_value(v, o.clone(), o);
                                    }
                                });
                            }
                            "assignee" => {
                                let cur = v.parse::<u64>().ok().and_then(|id| members.iter().find(|m| m.0 == id)).map(|m| m.1.clone()).unwrap_or_else(|| tr("Choose someone").into());
                                egui::ComboBox::from_id_salt(("fa", i)).selected_text(cur).show_ui(ui, |ui| {
                                    for (id, n) in &members {
                                        ui.selectable_value(v, id.to_string(), n);
                                    }
                                });
                            }
                            "checkbox" => {
                                let mut on = v == "Yes";
                                let cb = check(ui, &mut on, tr("Yes"));
                                name_it(&cb, &format!("Answer {}", i + 1));
                                if cb.changed() {
                                    *v = if on { "Yes".into() } else { String::new() };
                                }
                            }
                            "description" | "text" => {
                                let r = ui.add(field_multi(v).desired_rows(3).desired_width(f32::INFINITY));
                                name_it(&r, &format!("Answer {}", i + 1));
                            }
                            k => {
                                let hint = match k {
                                    "number" => "0",
                                    "email" => "name@example.com",
                                    _ => "",
                                };
                                let r = ui.add(field(v).hint_text(hint).desired_width(f32::INFINITY));
                                name_it(&r, &format!("Answer {}", i + 1));
                            }
                        }
                        ui.add_space(6.0);
                    }
                    if let Some(e) = &self.sweep.form_error {
                        ui.label(RichText::new(tr(e)).color(t.danger));
                    }
                    if primary_button(ui, &t, tr("Submit")).clicked() {
                        match form_answers(&f, &self.form_fill) {
                            Err(e) => self.sweep.form_error = Some(e),
                            Ok(_) if self.data.space_of_list(f.list_id).is_none() => {
                                // the List was never chosen or has been deleted
                                self.toast(tr("This form has no List for its answers. Choose one under Edit."));
                                self.sweep.form_error = None;
                                self.form_mode_fill = false;
                            }
                            Ok(a) => {
                                let name = if a.name.trim().is_empty() { format!("{} submission", f.name) } else { a.name };
                                self.actions.push(Action::Add { list: f.list_id, name, status: None, priority: a.priority, assignees: a.assignees, due: a.due, parent: None, open: false });
                                self.pending.description = Some(a.description.trim().to_string());
                                // tells the form's owner (notify.rs); shared history, so in English
                                self.pending.log = Some(format!("submitted the form {}", f.name));
                                self.form_fill.clear();
                                self.sweep.form_error = None;
                                self.toast(trf!("Submitted. A task was created in {}", self.data.list_name(f.list_id)));
                            }
                        }
                    }
                });
            }
        });
    }
}
