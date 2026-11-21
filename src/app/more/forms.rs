// Forms: questions whose answers each make a task in the form's List.
// Here: the question kinds, checking answers, the templates, making a form,
// and the open form's header with its tabs. In forms/: the look the pages
// share (look.rs), the hub with its sidebar list, table and templates
// (home.rs), the question editor (editor.rs), filling a form out (fill.rs)
// and the Answers tab (answers.rs).

use super::*;

mod answers;
mod editor;
mod fill;
mod home;
mod look;

use look::*;

/// The open form's tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FormTab {
    #[default]
    Edit,
    Fill,
    Answers,
}

/// Screen state of the form pages (not saved).
#[derive(Default)]
pub struct FormsUi {
    /// Forms made this session: "New" beside them in the list.
    pub made: Vec<u64>,
    /// The sidebar's search, which also narrows the table.
    pub search: String,
    /// The table in name order instead of last edited first.
    pub by_name: bool,
    /// The question whose answer stopped the last submit.
    pub error_at: Option<usize>,
    /// A choice being typed into a dropdown question: (question, text, problem).
    pub choice: Option<(usize, String, Option<String>)>,
    /// The question last clicked in the editor (outlined).
    pub picked: Option<usize>,
    /// The question whose handle is being dragged.
    pub drag: Option<usize>,
    /// The Answers tab's filter: 0 all, 1 not done, 2 done.
    pub answers_filter: u8,
    /// The open form's questions as (text, kind) when `form_fill` and
    /// `error_at` were last fitted to them (`App::fit_form_fill`).
    fill_shape: Vec<(String, String)>,
    /// Answers per form, and the workspace state they were counted for.
    answers: (u64, usize, usize, HashMap<u64, Vec<u64>>),
}

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

/// The line icon of a question kind.
fn kind_icon(k: &str) -> &'static str {
    match k {
        "name" => "task",
        "description" => "notebook",
        "priority" => "flag",
        "due" => "calendar",
        "assignee" => "person",
        "dropdown" => "list",
        "number" => "hash",
        "email" => "send",
        "checkbox" => "check",
        _ => "pen",
    }
}

/// Kinds whose answer fills a field of the task; the others are added to
/// its description.
fn fills_task_field(k: &str) -> bool {
    matches!(k, "name" | "description" | "priority" | "due" | "assignee")
}

/// Kinds a form has at most one of: a second one's answer would replace the
/// first's.
fn one_per_form(k: &str) -> bool {
    matches!(k, "name" | "priority" | "due")
}

/// Where a question's answer goes, as the editor says under it.
fn kind_hint(fld: &FormField) -> String {
    match fld.map.as_str() {
        "name" => tr("Saved as the task name").into(),
        "description" => tr("Saved in the task description").into(),
        "priority" => tr("Saved as the priority: Urgent, High, Normal or Low").into(),
        "due" => tr("Saved as the due date").into(),
        "assignee" => tr("Saved as an assignee").into(),
        _ => trf!(
            "Added to the task description as \"{}: answer\"",
            fld.label.trim()
        ),
    }
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

/// Checks the answers (required, number, email) and turns them into task
/// fields; a problem comes with the question it is about.
fn form_check(f: &Form, fill: &HashMap<usize, String>) -> Result<FormAnswers, (usize, String)> {
    let mut a = FormAnswers::default();
    for (i, fld) in f.fields.iter().enumerate() {
        let mut v = fill
            .get(&i)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        // a choice removed since it was picked is no answer
        if fld.map == "dropdown" && !fld.options.iter().any(|o| o.trim() == v) {
            v.clear();
        }
        if v.is_empty() {
            if fld.required {
                return Err((i, trf!("Please answer \"{}\".", fld.label)));
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
                    return Err((
                        i,
                        trf!("\"{}\" isn't a date. Write it as YYYY-MM-DD.", fld.label),
                    ))
                }
            },
            "assignee" => a.assignees.extend(v.parse::<u64>().ok()),
            k => {
                if k == "number" && v.parse::<f64>().is_err() {
                    return Err((i, trf!("\"{}\" needs a number.", fld.label)));
                }
                if k == "email" && !(v.contains('@') && v.contains('.')) {
                    return Err((i, trf!("\"{}\" needs an email address.", fld.label)));
                }
                a.description.push_str(&format!("\n{}: {}", fld.label, v));
            }
        }
    }
    Ok(a)
}

/// Checks the answers (required, number, email) and turns them into task fields.
#[cfg(test)]
pub fn form_answers(f: &Form, fill: &HashMap<usize, String>) -> Result<FormAnswers, String> {
    form_check(f, fill).map_err(|(_, e)| e)
}

/// The templates on the Forms page: name, tile icon and colour, and the
/// questions as (text, kind).
type FormTemplate = (
    &'static str,
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);
const FORM_TEMPLATES: [FormTemplate; 5] = [
    (
        "Feedback Form",
        "smile",
        "teal",
        &[
            ("Your name", "name"),
            ("How was your experience?", "text"),
            ("Comments", "description"),
        ],
    ),
    (
        "Project Intake",
        "forms",
        "pink",
        &[
            ("Project name", "name"),
            ("Goal", "description"),
            ("Deadline", "due"),
            ("Priority", "priority"),
        ],
    ),
    (
        "Order Form",
        "briefcase",
        "purple",
        &[
            ("Customer", "name"),
            ("Items", "description"),
            ("Quantity", "text"),
            ("Needed by", "due"),
        ],
    ),
    (
        "Job Application",
        "person",
        "orange",
        &[
            ("Full name", "name"),
            ("Role", "text"),
            ("Experience", "description"),
            ("Links", "text"),
        ],
    ),
    (
        "IT Requests",
        "settings",
        "blue",
        &[
            ("Issue", "name"),
            ("Urgency", "priority"),
            ("Details", "description"),
        ],
    ),
];

/// A form template's question in the interface language: it is saved as the
/// question's text, so a Korean screen starts with Korean questions. Some
/// read differently in a form than elsewhere ("Comments" is 의견 here, not 댓글).
fn form_question(label: &str) -> &str {
    match label {
        "Your name" => crate::i18n::tr_ctx("Your name (form question)", label),
        "Comments" => crate::i18n::tr_ctx("Comments (form question)", label),
        "Goal" => crate::i18n::tr_ctx("Goal (form question)", label),
        "Role" => crate::i18n::tr_ctx("Role (form question)", label),
        "Full name" => crate::i18n::tr_ctx("Full name (form question)", label),
        "How was your experience?" => tr("How was your experience?"),
        "Project name" => tr("Project name"),
        "Deadline" => tr("Deadline"),
        "Customer" => tr("Customer"),
        "Items" => tr("Items"),
        "Quantity" => tr("Quantity"),
        "Needed by" => tr("Needed by"),
        "Experience" => tr("Experience"),
        "Links" => tr("Links"),
        "Issue" => tr("Issue"),
        "Urgency" => tr("Urgency"),
        _ => tr(label),
    }
}

impl App {
    /// Makes a form, from template `tpl` (a place in `FORM_TEMPLATES`) or a
    /// blank one, opens it on its Edit tab and says where it went.
    pub(crate) fn new_form_from(&mut self, tpl: Option<usize>) {
        let id = self.data.new_id();
        let list = self.default_list().unwrap_or(0);
        // made in the language of the person making it, like a new task's name
        let (name, icon, color, fields) = match tpl.and_then(|i| FORM_TEMPLATES.get(i)) {
            Some((n, icon, color, qs)) => (
                tr(n).to_string(),
                icon.to_string(),
                color.to_string(),
                qs.iter()
                    .map(|(l, m)| FormField {
                        label: form_question(l).to_string(),
                        map: m.to_string(),
                        ..Default::default()
                    })
                    .collect(),
            ),
            None => (
                tr("New Form").to_string(),
                "forms".to_string(),
                FORM_COLORS[(id % FORM_COLORS.len() as u64) as usize]
                    .0
                    .to_string(),
                vec![
                    FormField {
                        label: tr("Title").into(),
                        map: "name".into(),
                        ..Default::default()
                    },
                    FormField {
                        label: tr("Details").into(),
                        map: "description".into(),
                        ..Default::default()
                    },
                ],
            ),
        };
        self.data.extra.forms.push(Form {
            id,
            name: name.clone(),
            list_id: list,
            description: tr("Fill out this form to create a task.").into(),
            fields,
            owner: self.data.me,
            icon,
            color,
            updated: now_ts(),
        });
        self.form_open = Some(id);
        self.form_tab = FormTab::Edit;
        self.sweep.forms.made.push(id);
        self.section = Section::Forms;
        self.dirty = true;
        self.toast(trf!(
            "Made the form \"{}\". It's in the list on the left.",
            name
        ));
    }

    /// For screenshots (`--form home|edit|fill|answers`): an IT Requests
    /// form with a dropdown question and two answers, a feedback form and
    /// one with no List, and that page shown.
    pub(crate) fn screenshot_forms(&mut self, tab: &str) {
        self.new_form_from(Some(2));
        self.new_form_from(Some(0));
        if let Some(f) = self.data.extra.forms.last_mut() {
            f.list_id = 0;
            f.name = tr("Bug report").into();
            f.icon = "flag".into();
            f.color = "orange".into();
        }
        self.new_form_from(Some(4));
        let fi = self.data.extra.forms.len() - 1;
        let fid = self.data.extra.forms[fi].id;
        let list = self.data.extra.forms[fi].list_id;
        self.data.extra.forms[fi].fields.push(FormField {
            label: tr("Device").into(),
            map: "dropdown".into(),
            options: vec!["MacBook".into(), "Windows PC".into(), "iPhone".into()],
            ..Default::default()
        });
        for (name, pr) in [
            ("Meeting room screen shows no signal", 2u8),
            ("VPN keeps dropping", 1),
        ] {
            self.pending.form = Some(fid);
            self.pending.log = Some(format!(
                "submitted the form {}",
                self.data.extra.forms[fi].name
            ));
            self.apply(Action::Add {
                list,
                name: name.into(),
                status: None,
                priority: pr,
                assignees: vec![],
                due: None,
                parent: None,
                open: false,
            });
        }
        self.section = Section::Forms;
        self.form_open = Some(fid);
        self.form_tab = match tab {
            "fill" => FormTab::Fill,
            "answers" => FormTab::Answers,
            _ => FormTab::Edit,
        };
        if tab == "home" {
            self.form_open = None;
        }
    }

    /// Opens a form on one of its tabs.
    fn open_form(&mut self, id: u64, tab: FormTab) {
        self.form_open = Some(id);
        self.form_tab = tab;
    }

    /// The tasks each form's answers made, newest first: those that say so
    /// (`Task::form`), and for answers from before that was kept, tasks whose
    /// history says "submitted the form <name>" (when only one form has that
    /// name). Counted again when the workspace changes.
    fn form_answer_ids(&mut self) -> HashMap<u64, Vec<u64>> {
        let key = (
            self.data_gen.get(),
            self.data.tasks.len(),
            self.data.extra.forms.len(),
        );
        let c = &self.sweep.forms.answers;
        // (built fresh while a change waits to be taken in, like the Inbox rows)
        if !self.dirty && (c.0, c.1, c.2) == key {
            return c.3.clone();
        }
        let mut by_name: HashMap<&str, Option<u64>> = HashMap::new();
        for f in &self.data.extra.forms {
            by_name
                .entry(f.name.as_str())
                .and_modify(|v| *v = None)
                .or_insert(Some(f.id));
        }
        let mut tasks: Vec<&Task> = self.data.tasks.iter().filter(|t| !t.archived).collect();
        tasks.sort_by(|a, b| b.created.cmp(&a.created));
        let mut out: HashMap<u64, Vec<u64>> = HashMap::new();
        for t in tasks {
            let form = t.form.or_else(|| {
                t.activity
                    .iter()
                    .filter(|a| !a.comment)
                    .find_map(|a| a.text.strip_prefix("submitted the form "))
                    .and_then(|n| by_name.get(n).copied().flatten())
            });
            if let Some(f) = form {
                out.entry(f).or_default().push(t.id);
            }
        }
        self.sweep.forms.answers = (key.0, key.1, key.2, out.clone());
        out
    }

    /// The open form: its header with the tabs, then the tab.
    pub(crate) fn form_detail(&mut self, ui: &mut Ui) {
        let Some(fi) = self
            .form_open
            .and_then(|id| self.data.extra.forms.iter().position(|f| f.id == id))
        else {
            // deleted meanwhile (here or by a teammate): back to the list
            self.form_open = None;
            self.forms_home(ui);
            return;
        };
        // what was typed or flagged on one form doesn't stay on another
        if self.sweep.form_for != self.form_open {
            self.sweep.form_error = None;
            self.sweep.form_for = self.form_open;
            self.sweep.forms.error_at = None;
            self.sweep.forms.choice = None;
            self.sweep.forms.picked = None;
            self.sweep.forms.drag = None;
            self.form_fill.clear();
        }
        self.fit_form_fill(fi);
        self.form_header(ui, fi);
        match self.form_tab {
            FormTab::Edit => self.form_editor(ui, fi),
            FormTab::Fill => self.form_fill_page(ui, fi),
            FormTab::Answers => self.form_answers_page(ui, fi),
        }
    }

    /// Keeps what was typed on Fill out, and the problem Submit found, with
    /// their questions when the questions change (moved, copied, deleted or
    /// given another kind on Edit, or by a teammate): answers are kept by
    /// the question's place, so they would land on another question. A
    /// question is found again by its text and kind, or at the same place
    /// with the same kind when only its text changed; an answer whose
    /// question is gone or asks for another kind now is dropped.
    fn fit_form_fill(&mut self, fi: usize) {
        let now: Vec<(String, String)> = self.data.extra.forms[fi]
            .fields
            .iter()
            .map(|q| (q.label.clone(), q.map.clone()))
            .collect();
        let was = std::mem::take(&mut self.sweep.forms.fill_shape);
        if was != now {
            let mut used = vec![false; now.len()];
            let mut to: Vec<Option<usize>> = vec![None; was.len()];
            for (i, q) in was.iter().enumerate() {
                if let Some(j) = (0..now.len()).find(|&j| !used[j] && now[j] == *q) {
                    used[j] = true;
                    to[i] = Some(j);
                }
            }
            // the problem says the question's text: kept only when that is the same
            let error_at = self
                .sweep
                .forms
                .error_at
                .and_then(|i| to.get(i).copied().flatten());
            for (i, q) in was.iter().enumerate() {
                if to[i].is_none() && i < now.len() && !used[i] && now[i].1 == q.1 {
                    used[i] = true;
                    to[i] = Some(i);
                }
            }
            for (i, v) in std::mem::take(&mut self.form_fill) {
                if let Some(Some(j)) = to.get(i) {
                    self.form_fill.insert(*j, v);
                }
            }
            self.sweep.forms.error_at = error_at;
            if error_at.is_none() {
                self.sweep.form_error = None;
            }
        }
        self.sweep.forms.fill_shape = now;
    }

    /// The bar over an open form: back to the list and the form's name on the
    /// left, the tabs in the middle, saving and the star on the right.
    fn form_header(&mut self, ui: &mut Ui, fi: usize) {
        let t = self.t();
        let f = self.data.extra.forms[fi].clone();
        let answers = self.form_answer_ids().get(&f.id).map_or(0, |v| v.len());
        egui::TopBottomPanel::top("form_head")
            .frame(
                egui::Frame::new()
                    .fill(t.bg)
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show_inside(ui, |ui| {
                let row = ui.available_rect_before_wrap();
                let row = Rect::from_min_size(row.min, vec2(row.width(), CONTROL_H + 4.0));
                ui.allocate_rect(row, Sense::hover());
                // left: back, "All forms", the form
                let mut left = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(row)
                        .layout(Layout::left_to_right(Align::Center)),
                );
                left.spacing_mut().item_spacing.x = 4.0;
                if icon_button(
                    &mut left,
                    &t,
                    "chevron_left",
                    "",
                    "Back to all forms",
                    false,
                )
                .on_hover_text(tr("All forms"))
                .clicked()
                {
                    self.form_open = None;
                }
                let crumb = left.add(ghost(egui::Button::new(
                    RichText::new(tr("All forms")).size(13.0).color(t.text2),
                )));
                name_it(&crumb, "Forms crumb");
                if crumb.clicked() {
                    self.form_open = None;
                }
                left.label(RichText::new("/").color(t.text3));
                let (tile, _) = left.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                paint_tile(left.painter(), tile, form_look(&f, &t));
                left.add_space(2.0);
                let room = (row.width() * 0.3).max(80.0);
                let shown = fit_text(left.painter(), &f.name, &FontId::proportional(14.0), room);
                left.label(RichText::new(shown).size(14.0).color(t.text));
                let left_end = left.min_rect().right();
                // the tabs, in the middle when there is room
                let items: [SegItem; 3] = [
                    ("edit", tr("Edit").into(), None, "Edit"),
                    ("eye", tr("Fill out").into(), None, "Fill out"),
                    ("inbox", tr("Answers").into(), Some(answers), "Answers"),
                ];
                let sel = match self.form_tab {
                    FormTab::Edit => 0,
                    FormTab::Fill => 1,
                    FormTab::Answers => 2,
                };
                let seg_w = 330.0;
                let x = (row.center().x - seg_w / 2.0).max(left_end + 16.0);
                let mut mid = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_max(pos2(x, row.top()), row.max))
                        .layout(Layout::left_to_right(Align::Center)),
                );
                if let Some(i) = seg(&mut mid, &t, &items, sel) {
                    self.form_tab = [FormTab::Edit, FormTab::Fill, FormTab::Answers][i];
                }
                let mid_end = mid.min_rect().right();
                // right: the star, and "Saved automatically" when it fits
                let mut right = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(row)
                        .layout(Layout::right_to_left(Align::Center)),
                );
                self.star_toggle(&mut right, format!("form:{}", f.id));
                let saved = tr("Saved automatically");
                let w = right
                    .painter()
                    .layout_no_wrap(saved.to_string(), FontId::proportional(12.0), t.text3)
                    .size()
                    .x;
                if right.min_rect().left() - w - 40.0 > mid_end {
                    right.add_space(8.0);
                    right.label(RichText::new(saved).size(12.0).color(t.text3));
                    let (r, _) = right.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    line_icon_scaled(right.painter(), "check", r.center(), t.text3, 0.8);
                }
            });
    }
}
