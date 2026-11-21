// Forms: questions whose answers each make a task in the form's List.
// Here: the question kinds, checking answers, the templates, making a form,
// and the open form's header with its tabs. In forms/: the look the pages
// share (look.rs), the hub with its sidebar list, table and templates
// (home.rs), the question editor (editor.rs), filling a form out (fill.rs)
// and the Answers tab (answers.rs).

use super::*;
use wb_net::email_ok;

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
    /// The open form's questions as (text, kind) when `App::form.fill` and
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

/// A question's text as the form shows it: "Untitled question" when it
/// has none, as an unnamed form shows "Untitled form".
fn question_title(fld: &FormField) -> String {
    if fld.label.trim().is_empty() {
        tr("Untitled question").into()
    } else {
        fld.label.trim().to_string()
    }
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
            question_title(fld)
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

/// The form name in a task's history line "submitted the form <name>": how
/// apps that don't record `Task::form` (0.1.21 and older) mark an answer.
fn old_answer_name(t: &Task) -> Option<&str> {
    t.activity
        .iter()
        .filter(|a| !a.comment)
        .find_map(|a| a.text.strip_prefix("submitted the form "))
}

/// The form such an answer went to: the one form named so, or, when
/// several are, the one whose List the task is in.
pub(crate) fn old_answer_form(forms: &[Form], name: &str, list: u64) -> Option<u64> {
    let named: Vec<&Form> = forms.iter().filter(|f| f.name == name).collect();
    let one = |v: Vec<&Form>| match v.as_slice() {
        [f] => Some(f.id),
        _ => None,
    };
    one(named.clone()).or_else(|| one(named.into_iter().filter(|f| f.list_id == list).collect()))
}

/// Checks the answers (required, number, email) and turns them into task
/// fields; a problem comes with the question it is about.
fn form_check(f: &Form, fill: &HashMap<usize, String>) -> Result<FormAnswers, (usize, String)> {
    // a form that doesn't ask for a priority makes a Normal task, like any other new task
    let mut a = FormAnswers {
        priority: DEFAULT_PRIORITY,
        ..Default::default()
    };
    for (i, fld) in f.fields.iter().enumerate() {
        let label = question_title(fld);
        let mut v = fill
            .get(&i)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        // a choice removed since it was picked is no answer
        if fld.map == "dropdown" && !fld.options.iter().any(|o| o.trim() == v) {
            v.clear();
        }
        if v.is_empty() {
            // a dropdown with no choices can't be answered, so it can't hold
            // the form up
            let answerable =
                fld.map != "dropdown" || fld.options.iter().any(|o| !o.trim().is_empty());
            if fld.required && answerable {
                return Err((i, trf!("Please answer \"{}\".", label)));
            }
            continue;
        }
        match fld.map.as_str() {
            "name" => a.name = v,
            "description" => {
                // a second one starts its own paragraph
                if !a.description.is_empty() {
                    a.description.push_str("\n\n");
                }
                a.description.push_str(&v)
            }
            "priority" => {
                a.priority = PRIORITIES.iter().position(|p| p.label == v).unwrap_or(0) as u8
            }
            "due" => match NaiveDate::parse_from_str(&v, "%Y-%m-%d") {
                Ok(d) => a.due = Some(d),
                Err(_) => {
                    return Err((
                        i,
                        trf!("\"{}\" isn't a date. Write it as YYYY-MM-DD.", label),
                    ))
                }
            },
            "assignee" => a.assignees.extend(v.parse::<u64>().ok()),
            k => {
                // read as goal numbers are: thousands commas allowed, finite
                if k == "number" && super::goals::parse_number(&v).is_none() {
                    return Err((i, trf!("\"{}\" needs a number.", label)));
                }
                if k == "email" && !email_ok(&v) {
                    return Err((i, trf!("\"{}\" needs an email address.", label)));
                }
                // a ticked box ("Yes" here) as it read on the screen
                let shown = if k == "checkbox" { tr("Yes") } else { &v };
                a.description.push_str(&format!("\n{}: {}", label, shown));
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
        // answers older apps sent a form of this name stay with it
        self.tie_old_answers(&name);
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
        self.form.open = Some(id);
        self.form.tab = FormTab::Edit;
        self.sweep.forms.made.push(id);
        self.section = Section::Forms;
        self.dirty = true;
        self.toast(trf!(
            "Made the form \"{}\". It's in the list on the left.",
            name
        ));
    }

    /// A blank form made from a space's + menu: its answers go to the List
    /// being looked at when it is in that space, else to the first List of
    /// that space that isn't archived, or to none yet (it asks for one) when
    /// the space has no List; never to a List of another space.
    pub(crate) fn new_form_in_space(&mut self, sid: u64) {
        let here = self
            .default_list()
            .filter(|l| self.data.space_of_list(*l).is_some_and(|s| s.id == sid))
            .filter(|l| !self.data.list_archived(*l));
        self.new_form_from(None);
        let list = here
            .or_else(|| {
                self.data
                    .lists_in(Location::Space(sid))
                    .and_then(|v| v.into_iter().find(|l| !self.data.list_archived(*l)))
            })
            .unwrap_or(0);
        if let Some(f) = self
            .form
            .open
            .and_then(|id| self.data.extra.forms.iter_mut().find(|f| f.id == id))
        {
            f.list_id = list;
        }
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
        self.form.open = Some(fid);
        self.form.tab = match tab {
            "fill" => FormTab::Fill,
            "answers" => FormTab::Answers,
            _ => FormTab::Edit,
        };
        if tab == "home" {
            self.form.open = None;
        }
    }

    /// Ties the answers older apps sent to a form named `name` to that form
    /// (`Task::form`), before something makes the name stop finding them:
    /// the form being renamed, or another form taking the same name.
    pub(crate) fn tie_old_answers(&mut self, name: &str) {
        let forms = &self.data.extra.forms;
        let mut tied = false;
        for t in self.data.tasks.iter_mut().filter(|t| t.form.is_none()) {
            let form = old_answer_name(t)
                .filter(|n| *n == name)
                .and_then(|n| old_answer_form(forms, n, t.list_id));
            if form.is_some() {
                t.form = form;
                tied = true;
            }
        }
        if tied {
            self.dirty = true;
        }
    }

    /// Opens a form on one of its tabs.
    fn open_form(&mut self, id: u64, tab: FormTab) {
        self.form.open = Some(id);
        self.form.tab = tab;
    }

    /// The tasks each form's answers made, newest first: those that say so
    /// (`Task::form`), and for answers from apps that don't keep that, tasks
    /// whose history says "submitted the form <name>" (`old_answer_form`).
    /// Counted again when the workspace changes.
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
        let forms = &self.data.extra.forms;
        let mut tasks: Vec<&Task> = self.data.tasks.iter().filter(|t| !t.archived).collect();
        tasks.sort_by(|a, b| b.created.cmp(&a.created));
        let mut out: HashMap<u64, Vec<u64>> = HashMap::new();
        for t in tasks {
            let form = t
                .form
                .or_else(|| old_answer_name(t).and_then(|n| old_answer_form(forms, n, t.list_id)));
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
            .form
            .open
            .and_then(|id| self.data.extra.forms.iter().position(|f| f.id == id))
        else {
            // deleted meanwhile (here or by a teammate): back to the list
            self.form.open = None;
            self.forms_home(ui);
            return;
        };
        // what was typed, flagged or picked on one form doesn't stay on another
        if self.sweep.form_for != self.form.open {
            self.sweep.form_error = None;
            self.sweep.form_for = self.form.open;
            self.sweep.forms.error_at = None;
            self.sweep.forms.choice = None;
            self.sweep.forms.picked = None;
            self.sweep.forms.drag = None;
            self.sweep.forms.answers_filter = 0;
            self.form.fill.clear();
        }
        self.fit_form_fill(fi);
        self.form_header(ui, fi);
        match self.form.tab {
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
            // first the questions still where they were, then the moved ones
            for (i, q) in was.iter().enumerate() {
                if now.get(i) == Some(q) {
                    used[i] = true;
                    to[i] = Some(i);
                }
            }
            for (i, q) in was.iter().enumerate() {
                if to[i].is_some() {
                    continue;
                }
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
            for (i, v) in std::mem::take(&mut self.form.fill) {
                if let Some(Some(j)) = to.get(i) {
                    self.form.fill.insert(*j, v);
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
                // right: the star first, so the name and the tabs know where it starts
                let mut right = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(row)
                        .layout(Layout::right_to_left(Align::Center)),
                );
                self.star_toggle(&mut right, format!("form:{}", f.id));
                let tabs_end = right.min_rect().left() - 8.0;
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
                    self.form.open = None;
                }
                let crumb = left.add(ghost(egui::Button::new(
                    RichText::new(tr("All forms")).size(13.0).color(t.text2),
                )));
                name_it(&crumb, "Forms crumb");
                if crumb.clicked() {
                    self.form.open = None;
                }
                left.label(RichText::new("/").color(t.text3));
                let (tile, _) = left.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                paint_tile(left.painter(), tile, form_look(&f, &t));
                left.add_space(2.0);
                // the tabs, measured before the name, which is shortened to
                // leave them room before the star; where even that isn't
                // enough (a small window, a wide sidebar) they drop their
                // icons, and the name goes
                let name_left = left.cursor().left() + 4.0;
                let tab = |icon: &'static str, wide: bool| if wide { icon } else { "" };
                let tabs = |wide: bool| -> [SegItem; 3] {
                    [
                        (tab("edit", wide), tr("Edit").into(), None, "Edit"),
                        (tab("eye", wide), tr("Fill out").into(), None, "Fill out"),
                        (
                            tab("inbox", wide),
                            tr("Answers").into(),
                            Some(answers),
                            "Answers",
                        ),
                    ]
                };
                let mut items = tabs(true);
                let mut seg_w = seg_width(ui.painter(), &t, &items);
                if tabs_end - seg_w - 16.0 - name_left < 40.0 {
                    items = tabs(false);
                    seg_w = seg_width(ui.painter(), &t, &items);
                }
                let room = (row.width() * 0.3).min(tabs_end - seg_w - 16.0 - name_left);
                if room >= 16.0 {
                    let title = form_title(&f);
                    let shown = fit_text(left.painter(), &title, &FontId::proportional(14.0), room);
                    left.label(RichText::new(shown).size(14.0).color(t.text));
                }
                let left_end = left.min_rect().right();
                // the tabs: in the middle when there is room, else after the
                // name and before the star
                let sel = match self.form.tab {
                    FormTab::Edit => 0,
                    FormTab::Fill => 1,
                    FormTab::Answers => 2,
                };
                let x = (row.center().x - seg_w / 2.0)
                    .min(tabs_end - seg_w)
                    .max(left_end + 16.0);
                let mut mid = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_max(pos2(x, row.top()), row.max))
                        .layout(Layout::left_to_right(Align::Center)),
                );
                if let Some(i) = seg(&mut mid, &t, &items, sel) {
                    self.form.tab = [FormTab::Edit, FormTab::Fill, FormTab::Answers][i];
                }
                let mid_end = mid.min_rect().right();
                // "Saved automatically" before the star when it fits
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
