// Template Center: one dialog to browse, preview and use templates, the
// built-in ones and the team's own (`Extra::templates`, shared through
// wb-sync), and to rename or delete the team's. A template makes a List with
// its Space statuses and starting tasks (in a new Space or an existing one),
// a task, or a Doc. Built-in templates are written here in English and made
// in the interface language, so a Korean team gets Korean task names; team
// templates are made exactly as they were saved.

use super::*;

/// `TaskTemplate::kind` values ("" is a task).
pub const KIND_LIST: &str = "list";
pub const KIND_DOC: &str = "doc";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    All,
    Lists,
    Tasks,
    Docs,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::All,
        Category::Lists,
        Category::Tasks,
        Category::Docs,
    ];

    /// English label (also the accessibility name after "Template category ").
    pub fn label(self) -> &'static str {
        match self {
            Category::All => "All templates",
            Category::Lists => "Lists & Spaces",
            Category::Tasks => "Tasks",
            Category::Docs => "Docs",
        }
    }

    fn fits(self, kind: &str) -> bool {
        match self {
            Category::All => true,
            Category::Lists => kind == KIND_LIST,
            Category::Tasks => kind != KIND_LIST && kind != KIND_DOC,
            Category::Docs => kind == KIND_DOC,
        }
    }
}

/// The template picked in the center.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Pick {
    Builtin(&'static str),
    Team(u64),
    /// The form for a new task template.
    NewTask,
}

/// A built-in template.
pub struct Builtin {
    pub key: &'static str,
    /// The English name (accessibility names stay English).
    pub en: &'static str,
    pub glyph: &'static str,
    pub color: [u8; 3],
    /// One line for the card, in the interface language.
    pub about: &'static str,
    /// What it makes, in the interface language.
    pub tpl: TaskTemplate,
}

/// Where a List template's List goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListDest {
    NewSpace,
    /// An existing Space (or a Folder in it); `add_statuses` adds the
    /// template's statuses that the Space doesn't have yet.
    Space {
        space: u64,
        folder: Option<u64>,
        add_statuses: bool,
    },
}

/// The Template Center's state (`Modal::TemplateCenter`).
pub struct TemplateCenter {
    pub query: String,
    pub cat: Category,
    pub pick: Option<Pick>,
    /// What the new item is called; filled in from the template when picked.
    pub name: String,
    /// List templates: make a new Space for the List.
    pub new_space: bool,
    /// List templates in an existing Space: the Space and Folder.
    pub space: u64,
    pub folder: Option<u64>,
    pub add_statuses: bool,
    /// Task templates: the List the task goes in.
    pub list: Option<u64>,
    /// Doc templates: the Space (None: the workspace).
    pub doc_space: Option<u64>,
    /// Renaming the picked team template: the name typed so far.
    pub renaming: Option<String>,
    pub confirm_delete: bool,
    // the New task template form
    pub new_name: String,
    pub new_desc: String,
    pub new_checklist: String,
    pub new_priority: u8,
}

/// One row in the center's list.
struct Entry {
    pick: Pick,
    /// For the accessibility name.
    en: String,
    name: String,
    about: String,
    glyph: &'static str,
    color: [u8; 3],
    tpl: TaskTemplate,
}

fn st(name: &str, color: [u8; 3], kind: StatusKind) -> Status {
    Status::new(name, color, kind)
}

fn task(name: &str, status: &str, priority: u8) -> TemplateTask {
    TemplateTask {
        name: name.into(),
        status: status.into(),
        priority,
        ..Default::default()
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// The built-in templates, in the interface language.
pub fn builtins() -> Vec<Builtin> {
    let grey = STATUS_PALETTE[0];
    let review = [0xb6, 0x60, 0xe0];
    let mut v = vec![];

    // Lists & Spaces
    let mut brief = task(tr("Write the project brief"), "to do", 2);
    brief.description = tr("Goals, scope, and what done looks like.").into();
    brief.checklist = strings(&[tr("Goals"), tr("Scope"), tr("Success measures")]);
    let mut kickoff = task(tr("Hold the kickoff meeting"), "to do", 3);
    kickoff.checklist = strings(&[
        tr("Book a time"),
        tr("Share the brief"),
        tr("Agree on roles"),
    ]);
    v.push(Builtin {
        key: "project-tracker",
        en: "Project tracker",
        glyph: "\u{1F4CB}",
        color: [0xe1, 0x6b, 0x16],
        about: tr("Plan a project from kickoff to wrap-up, with a review step"),
        tpl: TaskTemplate {
            name: tr("Project tracker").into(),
            task_name: tr("Project tracker").into(),
            kind: KIND_LIST.into(),
            statuses: vec![
                st("to do", grey, StatusKind::Open),
                st("in progress", IN_PROGRESS_COLOR, StatusKind::Custom),
                st(tr("in review"), review, StatusKind::Custom),
                st("complete", COMPLETE_COLOR, StatusKind::Closed),
            ],
            tasks: vec![
                brief,
                task(tr("Plan milestones and dates"), "to do", 3),
                kickoff,
                task(tr("Weekly status update"), "to do", 0),
                task(tr("Wrap up and note lessons learned"), "to do", 4),
            ],
            ..Default::default()
        },
    });

    let mut sign_in = task(
        tr("Example: The sign-in button does nothing"),
        tr("reported"),
        2,
    );
    sign_in.description =
        tr("Steps to reproduce:\n1. \n2. \n\nWhat happened:\n\nWhat should happen:\n").into();
    sign_in.checklist = strings(&[tr("Reproduce"), tr("Find the cause"), tr("Fix and test")]);
    sign_in.tags = vec![tr("bug").into()];
    let mut typo = task(tr("Example: A typo on the settings page"), tr("triaged"), 4);
    typo.tags = vec![tr("bug").into()];
    v.push(Builtin {
        key: "bug-tracker",
        en: "Bug tracker",
        glyph: "\u{1F41B}",
        color: [0xd3, 0x3d, 0x44],
        about: tr("Report, sort and fix bugs until they are closed"),
        tpl: TaskTemplate {
            name: tr("Bug tracker").into(),
            task_name: tr("Bug tracker").into(),
            kind: KIND_LIST.into(),
            statuses: vec![
                st(tr("reported"), grey, StatusKind::Open),
                st(tr("triaged"), [0xf8, 0xae, 0x00], StatusKind::Custom),
                st("in progress", IN_PROGRESS_COLOR, StatusKind::Custom),
                st(tr("fixed"), [0x3d, 0xb8, 0x8b], StatusKind::Done),
                st(tr("closed"), COMPLETE_COLOR, StatusKind::Closed),
            ],
            tasks: vec![sign_in, typo],
            ..Default::default()
        },
    });

    let mut blog = task(tr("Blog post: product update"), tr("writing"), 3);
    blog.checklist = strings(&[tr("Outline"), tr("Draft"), tr("Edit"), tr("Pick an image")]);
    blog.tags = vec![tr("blog").into()];
    let mut letter = task(tr("Newsletter for this month"), tr("idea"), 0);
    letter.tags = vec![tr("newsletter").into()];
    let mut social = task(tr("Social posts for the launch"), tr("idea"), 0);
    social.tags = vec![tr("social").into()];
    v.push(Builtin {
        key: "content-calendar",
        en: "Content calendar",
        glyph: "\u{1F4C5}",
        color: [0x44, 0x66, 0xff],
        about: tr("Take posts from idea to published on a schedule"),
        tpl: TaskTemplate {
            name: tr("Content calendar").into(),
            task_name: tr("Content calendar").into(),
            kind: KIND_LIST.into(),
            statuses: vec![
                st(tr("idea"), grey, StatusKind::Open),
                st(tr("writing"), [0x44, 0x66, 0xff], StatusKind::Custom),
                st(tr("editing"), [0xe1, 0x6b, 0x16], StatusKind::Custom),
                st(tr("scheduled"), review, StatusKind::Custom),
                st(tr("published"), COMPLETE_COLOR, StatusKind::Closed),
            ],
            tasks: vec![
                blog,
                letter,
                social,
                task(tr("Customer story interview"), tr("idea"), 0),
            ],
            ..Default::default()
        },
    });

    let mut accounts = task(tr("Set up accounts and tools"), "to do", 2);
    accounts.checklist = strings(&[tr("Email"), tr("Chat"), "Workboard", tr("Work tools")]);
    let mut meet = task(tr("Meet the team"), "to do", 3);
    meet.checklist = strings(&[
        tr("One-on-one with your manager"),
        tr("Team lunch"),
        tr("Meet your buddy"),
    ]);
    v.push(Builtin {
        key: "onboarding",
        en: "Onboarding checklist",
        glyph: "\u{1F44B}",
        color: [0x3d, 0xb8, 0x8b],
        about: tr("Help a new teammate settle in during the first month"),
        tpl: TaskTemplate {
            name: tr("Onboarding checklist").into(),
            task_name: tr("Onboarding checklist").into(),
            kind: KIND_LIST.into(),
            statuses: default_statuses(),
            tasks: vec![
                accounts,
                meet,
                task(tr("Read how we work"), "to do", 0),
                task(tr("Finish a first small task"), "to do", 0),
                task(tr("30-day check-in"), "to do", 3),
            ],
            ..Default::default()
        },
    });

    // Tasks
    v.push(Builtin {
        key: "bug-report",
        en: "Bug report",
        glyph: "\u{26A0}",
        color: [0xd3, 0x3d, 0x44],
        about: tr("One bug with steps to reproduce and a fix checklist"),
        tpl: TaskTemplate {
            name: tr("Bug report").into(),
            task_name: tr("New bug").into(),
            description: tr(
                "Steps to reproduce:\n1. \n2. \n\nWhat happened:\n\nWhat should happen:\n",
            )
            .into(),
            checklist: strings(&[
                tr("Reproduce"),
                tr("Find the cause"),
                tr("Fix"),
                tr("Test the fix"),
            ]),
            tags: vec![tr("bug").into()],
            priority: 2,
            ..Default::default()
        },
    });
    v.push(Builtin {
        key: "feature-request",
        en: "Feature request",
        glyph: "\u{1F4A1}",
        color: [0xf8, 0xae, 0x00],
        about: tr("Describe a problem and the change that would solve it"),
        tpl: TaskTemplate {
            name: tr("Feature request").into(),
            task_name: tr("New feature request").into(),
            description: tr("Problem:\n\nProposed change:\n\nWho it helps:\n").into(),
            checklist: strings(&[
                tr("Talk to the people it helps"),
                tr("Agree on the scope"),
                tr("Build it"),
                tr("Announce it"),
            ]),
            tags: vec![tr("feature").into()],
            priority: 3,
            ..Default::default()
        },
    });

    // Docs (also the cards on the Docs page)
    for (key, en, glyph, color, about, body) in [
        (
            "project-overview",
            "Project Overview",
            "\u{1F4CB}",
            [0xe1, 0x6b, 0x16],
            tr("Summarize goals, scope, and milestones"),
            tr("Goals\n- \n\nScope\n- \n\nMilestones\n- \n\nTeam\n- "),
        ),
        (
            "meeting-notes",
            "Meeting Notes",
            "\u{1F4DD}",
            [0xf8, 0xae, 0x00],
            tr("Capture an agenda, notes, and action items"),
            tr("Agenda\n- \n\nNotes\n- \n\nAction items\n- [ ] "),
        ),
        (
            "wiki",
            "Wiki",
            "\u{1F4D3}",
            [0x44, 0x66, 0xff],
            tr("Organize information in one place"),
            tr("Overview\n\nHow we work\n\nLinks\n"),
        ),
    ] {
        v.push(Builtin {
            key,
            en,
            glyph,
            color,
            about,
            tpl: TaskTemplate {
                name: tr(en).into(),
                task_name: tr(en).into(),
                description: body.into(),
                kind: KIND_DOC.into(),
                ..Default::default()
            },
        });
    }
    v
}

pub fn builtin(key: &str) -> Option<Builtin> {
    builtins().into_iter().find(|b| b.key == key)
}

/// Adds the statuses in `add` that `to` lacks (by name): open and custom
/// ones before the first done or closed one, done and closed ones before the
/// closed ones. The Space keeps its own first (open) status.
pub fn merge_statuses(to: &mut Vec<Status>, add: &[Status]) {
    for s in add {
        if to.iter().any(|x| x.name == s.name) {
            continue;
        }
        let mut s = s.clone();
        let at = match s.kind {
            StatusKind::Open | StatusKind::Custom => {
                s.kind = StatusKind::Custom;
                to.iter()
                    .position(|x| matches!(x.kind, StatusKind::Done | StatusKind::Closed))
                    .unwrap_or(to.len())
            }
            StatusKind::Done | StatusKind::Closed => {
                if to.iter().any(|x| x.kind == StatusKind::Closed) {
                    s.kind = StatusKind::Done;
                }
                to.iter()
                    .position(|x| x.kind == StatusKind::Closed)
                    .unwrap_or(to.len())
            }
        };
        to.insert(at.max(1).min(to.len()), s);
    }
}

fn kind_glyph(kind: &str) -> (&'static str, [u8; 3]) {
    match kind {
        KIND_LIST => ("\u{2630}", [0x5f, 0x55, 0xee]),
        KIND_DOC => ("\u{1F4C4}", [0x10, 0x90, 0xe0]),
        _ => ("\u{2611}", [0x0f, 0x9d, 0x9f]),
    }
}

fn count(n: usize, one: &str, many: &'static str) -> String {
    if n == 1 {
        tr(one).to_string()
    } else {
        crate::i18n::fill(tr(many), &[n.to_string()])
    }
}

/// A line about a team template for its row.
fn about_team(tp: &TaskTemplate) -> String {
    match tp.kind.as_str() {
        KIND_LIST => format!(
            "{} \u{2022} {}",
            tr("List"),
            count(tp.tasks.len(), "1 task", "{} tasks")
        ),
        KIND_DOC => format!(
            "{} \u{2022} {}",
            tr("Doc"),
            truncate(tp.description.lines().next().unwrap_or(""), 30)
        ),
        _ => {
            let mut s = tr("Task").to_string();
            if !tp.checklist.is_empty() {
                s.push_str(" \u{2022} ");
                s.push_str(&count(
                    tp.checklist.len(),
                    "1 checklist item",
                    "{} checklist items",
                ));
            }
            s
        }
    }
}

/// A template row: round coloured icon, name and a line under it.
#[allow(clippy::too_many_arguments)]
fn template_row(
    ui: &mut Ui,
    t: &Theme,
    w: f32,
    color: [u8; 3],
    glyph: &str,
    title: &str,
    sub: &str,
    selected: bool,
    a11y: &str,
) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
    name_it(&resp, a11y);
    if selected {
        ui.painter().rect_filled(r, 8.0, t.selected_bg);
    } else {
        hover_bg(ui, &resp, r, 8.0);
    }
    let c = r.left_center() + vec2(20.0, 0.0);
    ui.painter()
        .circle_filled(c, 14.0, rgb(color).gamma_multiply(0.35));
    ui.painter().text(
        c,
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(13.0),
        rgb(color),
    );
    let clip = ui.painter().with_clip_rect(r.shrink(2.0));
    clip.text(
        r.left_center() + vec2(42.0, -8.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(13.5),
        if selected { t.selected_text } else { t.text },
    );
    clip.text(
        r.left_center() + vec2(42.0, 9.0),
        Align2::LEFT_CENTER,
        sub,
        FontId::proportional(11.5),
        t.text3,
    );
    resp
}

impl App {
    /// Opens the Template Center on `cat`; `space` is the Space whose "+"
    /// menu it was opened from (List templates then go there).
    pub(super) fn open_templates(&mut self, cat: Category, space: Option<u64>) {
        self.modal = Some(self.template_center(cat, space));
    }

    pub(super) fn template_center(&self, cat: Category, space: Option<u64>) -> Modal {
        let open: Vec<u64> = self.open_spaces().iter().map(|s| s.id).collect();
        let here = space.or_else(|| self.current_space());
        let doc_space = here.filter(|s| open.contains(s));
        Modal::TemplateCenter(Box::new(TemplateCenter {
            query: String::new(),
            cat,
            pick: None,
            name: String::new(),
            new_space: space.is_none(),
            space: doc_space.or_else(|| open.first().copied()).unwrap_or(0),
            folder: None,
            add_statuses: true,
            list: self.default_list(),
            doc_space,
            renaming: None,
            confirm_delete: false,
            new_name: String::new(),
            new_desc: String::new(),
            new_checklist: String::new(),
            new_priority: 0,
        }))
    }

    /// Team templates first, then the built-in ones.
    fn template_entries(&self) -> Vec<Entry> {
        let mut v: Vec<Entry> = self
            .data
            .extra
            .templates
            .iter()
            .map(|tp| {
                let (glyph, color) = kind_glyph(&tp.kind);
                Entry {
                    pick: Pick::Team(tp.id),
                    en: tp.name.clone(),
                    name: tp.name.clone(),
                    about: about_team(tp),
                    glyph,
                    color,
                    tpl: tp.clone(),
                }
            })
            .collect();
        v.extend(builtins().into_iter().map(|b| Entry {
            pick: Pick::Builtin(b.key),
            en: b.en.to_string(),
            name: b.tpl.name.clone(),
            about: b.about.to_string(),
            glyph: b.glyph,
            color: b.color,
            tpl: b.tpl,
        }));
        v
    }

    fn picked_template(&self, pick: &Pick) -> Option<TaskTemplate> {
        match pick {
            Pick::Builtin(k) => builtin(k).map(|b| b.tpl),
            Pick::Team(id) => self
                .data
                .extra
                .templates
                .iter()
                .find(|x| x.id == *id)
                .cloned(),
            Pick::NewTask => None,
        }
    }

    // ------------------------------------------------------------ making things

    /// Makes a List (and a Space for it when asked) from a List template,
    /// with the template's statuses and tasks. Returns the List's id.
    pub(super) fn make_list_from_template(
        &mut self,
        tp: &TaskTemplate,
        name: &str,
        dest: ListDest,
    ) -> Option<u64> {
        let name = match name.trim() {
            "" => tp.task_name.clone(),
            n => n.to_string(),
        };
        let statuses = if tp.statuses.is_empty() {
            default_statuses()
        } else {
            tp.statuses.clone()
        };
        let lid = match dest {
            ListDest::NewSpace => {
                let id = self.data.new_id();
                let lid = self.data.new_id();
                let color = STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13];
                self.data.spaces.push(Space {
                    id,
                    name: name.clone(),
                    color,
                    statuses,
                    folders: vec![],
                    lists: vec![ListItem {
                        id: lid,
                        name: name.clone(),
                        ..Default::default()
                    }],
                    expanded: true,
                    ..Default::default()
                });
                lid
            }
            ListDest::Space {
                space,
                folder,
                add_statuses,
            } => {
                if add_statuses {
                    let s = self.data.spaces.iter_mut().find(|s| s.id == space)?;
                    merge_statuses(&mut s.statuses, &statuses);
                }
                self.create_list(space, folder, &name)?
            }
        };
        let have: Vec<String> = self
            .data
            .statuses_for_list(lid)
            .into_iter()
            .map(|s| s.name)
            .collect();
        for x in &tp.tasks {
            self.pending_template = Some(TaskTemplate {
                description: x.description.clone(),
                checklist: x.checklist.clone(),
                tags: x.tags.clone(),
                ..Default::default()
            });
            self.apply(Action::Add {
                list: lid,
                name: x.name.clone(),
                status: Some(x.status.clone()).filter(|s| have.contains(s)),
                priority: x.priority,
                assignees: vec![],
                due: None,
                parent: None,
                open: false,
            });
        }
        self.pending_template = None;
        self.section = Section::Home;
        self.loc = Location::List(lid);
        self.view = ViewKind::List;
        self.dirty = true;
        Some(lid)
    }

    /// Makes one task from a task template in `list`. Returns its id.
    pub(super) fn make_task_from_template(
        &mut self,
        tp: &TaskTemplate,
        list: u64,
        name: &str,
    ) -> Option<u64> {
        self.data.space_of_list(list)?;
        let name = match name.trim() {
            "" => tp.task_name.clone(),
            n => n.to_string(),
        };
        self.pending_template = Some(tp.clone());
        self.apply(Action::Add {
            list,
            name,
            status: None,
            priority: tp.priority,
            assignees: vec![],
            due: None,
            parent: None,
            open: false,
        });
        self.pending_template = None;
        let id = self.data.tasks.last().map(|t| t.id)?;
        self.section = Section::Home;
        self.loc = Location::List(list);
        self.apply(Action::Open(id));
        Some(id)
    }

    /// Makes a Doc from a Doc template and opens it. Returns its id.
    pub(super) fn make_doc_from_template(
        &mut self,
        tp: &TaskTemplate,
        space: Option<u64>,
        title: &str,
        meeting_note: bool,
    ) -> u64 {
        self.new_doc(space);
        let id = self.doc_open.unwrap_or(0);
        if let Some(d) = self.data.docs.iter_mut().find(|d| d.id == id) {
            d.title = match title.trim() {
                "" => tp.task_name.clone(),
                n => n.to_string(),
            };
            d.body = tp.description.clone();
            d.tags = tp.tags.clone();
            d.meeting_note = meeting_note;
        }
        id
    }

    /// Makes what a built-in Doc template makes (the Docs page cards).
    pub(super) fn use_builtin_doc(&mut self, key: &str, space: Option<u64>) {
        if let Some(b) = builtin(key) {
            let title = b.tpl.task_name.clone();
            self.make_doc_from_template(&b.tpl, space, &title, key == "meeting-notes");
        }
    }

    // ------------------------------------------------------------ saving

    /// Saves a List (its Space's statuses and its open top-level tasks) as a
    /// team template. Returns the template's id.
    pub(super) fn save_list_as_template(&mut self, lid: u64) -> Option<u64> {
        let item = self.data.list_item(lid)?.clone();
        let statuses = self.data.statuses_for_list(lid);
        let tasks: Vec<TemplateTask> = self
            .data
            .tasks
            .iter()
            .filter(|t| t.list_id == lid && t.parent.is_none() && self.data.task_live(t))
            .map(|t| TemplateTask {
                name: t.name.clone(),
                status: t.status.clone(),
                description: t.description.clone(),
                checklist: t.checklist.iter().map(|c| c.text.clone()).collect(),
                tags: t.tags.clone(),
                priority: t.priority,
            })
            .collect();
        let id = self.data.new_id();
        self.data.extra.templates.push(TaskTemplate {
            id,
            name: item.name.clone(),
            task_name: item.name.clone(),
            description: item.description.clone(),
            kind: KIND_LIST.into(),
            statuses,
            tasks,
            ..Default::default()
        });
        self.dirty = true;
        Some(id)
    }

    /// Saves a Doc's title and text as a team template.
    pub(super) fn save_doc_as_template(&mut self, doc: u64) -> Option<u64> {
        let d = self.data.docs.iter().find(|d| d.id == doc)?.clone();
        let id = self.data.new_id();
        self.data.extra.templates.push(TaskTemplate {
            id,
            name: d.title.clone(),
            task_name: d.title.clone(),
            description: d.body.clone(),
            tags: d.tags.clone(),
            kind: KIND_DOC.into(),
            ..Default::default()
        });
        self.dirty = true;
        Some(id)
    }

    // ------------------------------------------------------------ the dialog

    /// Returns true when the dialog should close.
    pub(super) fn template_center_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        c: &mut TemplateCenter,
    ) -> bool {
        let mut close = false;
        ui.label(
            RichText::new(tr(
                "Start Lists, tasks and Docs from a template: built-in ones or your team's own.",
            ))
            .color(t.text2),
        );
        ui.add_space(8.0);
        let r = ui.add(
            egui::TextEdit::singleline(&mut c.query)
                .hint_text(tr("Search templates"))
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(10, 6)),
        );
        name_field(&r, "Search templates");
        ui.add_space(6.0);
        let mut pick = None;
        ui.horizontal(|ui| {
            for cat in Category::ALL {
                let r = ui.selectable_label(c.cat == cat, tr(cat.label()));
                name_it(&r, &format!("Template category {}", cat.label()));
                if r.clicked() {
                    c.cat = cat;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let r = ui.add(ghost(egui::Button::new(
                    RichText::new(tr("+ New task template")).color(t.text2),
                )));
                name_it(&r, "New task template");
                if r.clicked() {
                    pick = Some(Pick::NewTask);
                }
            });
        });
        ui.add_space(6.0);
        ui.separator();

        let q = c.query.trim().to_lowercase();
        let entries: Vec<Entry> = self
            .template_entries()
            .into_iter()
            .filter(|e| c.cat.fits(&e.tpl.kind))
            .filter(|e| {
                q.is_empty()
                    || e.name.to_lowercase().contains(&q)
                    || e.en.to_lowercase().contains(&q)
                    || e.about.to_lowercase().contains(&q)
            })
            .collect();
        let h = (ui.ctx().screen_rect().height() - 330.0).clamp(230.0, 470.0);
        let left_w = 250.0;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(left_w, h), Layout::top_down(Align::Min), |ui| {
                ui.set_min_size(vec2(left_w, h));
                egui::ScrollArea::vertical()
                    .id_salt("tc_list")
                    .max_height(h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let w = ui.available_width();
                        let team = entries
                            .iter()
                            .filter(|e| matches!(e.pick, Pick::Team(_)))
                            .count();
                        for (i, e) in entries.iter().enumerate() {
                            if i == 0 && team > 0 {
                                ui.label(RichText::new(tr("Your team")).small().color(t.text3));
                            }
                            if i == team {
                                if team > 0 {
                                    ui.add_space(6.0);
                                }
                                ui.label(RichText::new(tr("Built-in")).small().color(t.text3));
                            }
                            let sel = c.pick.as_ref() == Some(&e.pick);
                            if template_row(
                                ui,
                                t,
                                w,
                                e.color,
                                e.glyph,
                                &e.name,
                                &e.about,
                                sel,
                                &format!("Pick template {}", e.en),
                            )
                            .clicked()
                            {
                                pick = Some(e.pick.clone());
                            }
                        }
                        if entries.is_empty() {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(tr("No templates match your search.")).color(t.text3),
                            );
                        }
                    });
            });
            ui.separator();
            ui.vertical(|ui| {
                egui::ScrollArea::vertical()
                    .id_salt("tc_detail")
                    .max_height(h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        close = self.template_detail(ui, t, c);
                    });
            });
        });
        if let Some(p) = pick {
            if let Some(tp) = self.picked_template(&p) {
                c.name = tp.task_name.clone();
            }
            c.pick = Some(p);
            c.renaming = None;
            c.confirm_delete = false;
        }
        close
    }

    /// The right side: preview, where to make it and Use; Rename and Delete
    /// for team templates; or the New task template form.
    fn template_detail(&mut self, ui: &mut Ui, t: &Theme, c: &mut TemplateCenter) -> bool {
        let Some(pick) = c.pick.clone() else {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(tr("Pick a template to see what it makes.")).color(t.text3));
            });
            return false;
        };
        if pick == Pick::NewTask {
            self.new_task_template_form(ui, t, c);
            return false;
        }
        let Some(tp) = self.picked_template(&pick) else {
            c.pick = None;
            return false;
        };
        let b = match &pick {
            Pick::Builtin(k) => builtin(k),
            _ => None,
        };
        let (glyph, color) = match &b {
            Some(b) => (b.glyph, b.color),
            None => kind_glyph(&tp.kind),
        };
        // heading
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
            ui.painter()
                .circle_filled(r.center(), 17.0, rgb(color).gamma_multiply(0.35));
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                glyph,
                FontId::proportional(15.0),
                rgb(color),
            );
            ui.vertical(|ui| {
                ui.label(RichText::new(&tp.name).size(16.0).strong());
                let about = match &b {
                    Some(b) => b.about.to_string(),
                    None => format!("{} \u{2022} {}", tr("Your team"), about_team(&tp)),
                };
                ui.label(RichText::new(about).small().color(t.text2));
            });
        });
        if let Pick::Team(id) = pick {
            if self.team_template_manage(ui, t, c, id) {
                return false;
            }
        }
        ui.add_space(8.0);
        ui.label(RichText::new(tr("What it makes")).small().color(t.text3));
        egui::Frame::new()
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // kept short so where-to and Use stay in view
                let h = (ui.ctx().screen_rect().height() - 600.0).clamp(80.0, 200.0);
                egui::ScrollArea::vertical()
                    .id_salt("tc_preview")
                    .max_height(h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| template_preview(ui, t, &tp));
            });
        ui.add_space(10.0);
        self.template_use(ui, t, c, &tp, b.as_ref().map(|b| b.key))
    }

    /// Rename and Delete for a team template. True when it was just deleted.
    fn team_template_manage(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        c: &mut TemplateCenter,
        id: u64,
    ) -> bool {
        ui.add_space(6.0);
        if let Some(name) = c.renaming.as_mut() {
            let mut save = false;
            let mut cancel = false;
            ui.horizontal(|ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(name)
                        .hint_text(tr("Template name"))
                        .desired_width(220.0),
                );
                name_field(&r, "Template name");
                focus_once(ui, &r);
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                save = primary_button(ui, t, tr("Save")).clicked() || enter;
                cancel = ui.button(tr("Cancel")).clicked();
            });
            if save && !name.trim().is_empty() {
                let new = name.trim().to_string();
                if let Some(x) = self.data.extra.templates.iter_mut().find(|x| x.id == id) {
                    x.name = new;
                    self.dirty = true;
                }
                c.renaming = None;
            } else if cancel {
                c.renaming = None;
            }
        } else if c.confirm_delete {
            ui.label(
                RichText::new(tr("Delete this template for everyone on the team?")).color(t.danger),
            );
            let mut gone = false;
            ui.horizontal(|ui| {
                let r = ui.add(Tint(
                    egui::Button::new(RichText::new(tr("Delete")).color(Color32::WHITE))
                        .fill(t.danger)
                        .corner_radius(6),
                ));
                name_it(&r, "Confirm delete template");
                if r.clicked() {
                    self.data.extra.templates.retain(|x| x.id != id);
                    self.dirty = true;
                    self.toast = Some((tr("Template deleted").into(), now_ts()));
                    gone = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    c.confirm_delete = false;
                }
            });
            if gone {
                c.pick = None;
                c.confirm_delete = false;
                return true;
            }
        } else {
            ui.horizontal(|ui| {
                let r = ui.add(ghost(egui::Button::new(tr("Rename"))));
                name_it(&r, "Rename template");
                if r.clicked() {
                    c.renaming = self
                        .data
                        .extra
                        .templates
                        .iter()
                        .find(|x| x.id == id)
                        .map(|x| x.name.clone());
                }
                let r = ui.add(ghost(egui::Button::new(
                    RichText::new(tr("Delete")).color(t.danger),
                )));
                name_it(&r, "Delete template");
                if r.clicked() {
                    c.confirm_delete = true;
                }
            });
        }
        false
    }

    /// Name, destination and the Use button. True when something was made.
    fn template_use(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        c: &mut TemplateCenter,
        tp: &TaskTemplate,
        builtin_key: Option<&str>,
    ) -> bool {
        let kind = tp.kind.as_str();
        ui.label(
            RichText::new(match kind {
                KIND_LIST => tr("List name"),
                KIND_DOC => tr("Doc title"),
                _ => tr("Task name"),
            })
            .color(t.text2),
        );
        let r = ui.add(
            egui::TextEdit::singleline(&mut c.name)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(8, 6)),
        );
        name_field(&r, "Template item name");
        ui.add_space(6.0);
        ui.label(
            RichText::new(match kind {
                KIND_LIST => tr("Where the List goes"),
                KIND_DOC => tr("Space (location)"),
                _ => tr("Add the task to"),
            })
            .color(t.text2),
        );
        let spaces: Vec<(u64, String, Vec<(u64, String)>)> = self
            .open_spaces()
            .iter()
            .map(|s| {
                (
                    s.id,
                    s.name.clone(),
                    s.folders
                        .iter()
                        .filter(|f| !f.archived)
                        .map(|f| (f.id, f.name.clone()))
                        .collect(),
                )
            })
            .collect();
        // (label, what picking it sets) for the destination menu
        let current: String;
        let mut options: Vec<(String, bool)> = vec![];
        match kind {
            KIND_LIST => {
                current = if c.new_space {
                    tr("New Space").to_string()
                } else {
                    spaces
                        .iter()
                        .find(|s| s.0 == c.space)
                        .map(
                            |s| match c.folder.and_then(|f| s.2.iter().find(|x| x.0 == f)) {
                                Some(f) => format!("{} / {}", s.1, f.1),
                                None => s.1.clone(),
                            },
                        )
                        .unwrap_or_else(|| tr("Choose a Space").into())
                };
                options.push((tr("New Space").into(), c.new_space));
                for (sid, sname, folders) in &spaces {
                    options.push((
                        sname.clone(),
                        !c.new_space && c.space == *sid && c.folder.is_none(),
                    ));
                    for (fid, fname) in folders {
                        options.push((
                            format!("{} / {}", sname, fname),
                            !c.new_space && c.folder == Some(*fid),
                        ));
                    }
                }
            }
            KIND_DOC => {
                current = c
                    .doc_space
                    .and_then(|d| spaces.iter().find(|s| s.0 == d))
                    .map(|s| s.1.clone())
                    .unwrap_or_else(|| tr("Workspace (no Space)").into());
                options.push((tr("Workspace (no Space)").into(), c.doc_space.is_none()));
                for (sid, sname, _) in &spaces {
                    options.push((sname.clone(), c.doc_space == Some(*sid)));
                }
            }
            _ => {
                current = c
                    .list
                    .filter(|l| self.data.space_of_list(*l).is_some())
                    .map(|l| self.data.list_path(l).join(" / "))
                    .unwrap_or_else(|| tr("Choose a List").into());
                for l in self.template_lists() {
                    options.push((self.data.list_path(l).join(" / "), c.list == Some(l)));
                }
            }
        }
        let r = ui.add(
            egui::Button::new(format!("{}  \u{25BE}", current))
                .min_size(vec2(ui.available_width().min(360.0), 28.0)),
        );
        name_it(&r, "Template destination");
        let mut chosen = None;
        popup(ui, &r, ui.id().with("tc_dest"), 300.0, |ui| {
            for (i, (label, sel)) in options.iter().enumerate() {
                if ui.selectable_label(*sel, label).clicked() {
                    chosen = Some(i);
                    close_popup(ui);
                }
            }
        });
        if let Some(i) = chosen {
            match kind {
                KIND_LIST => {
                    // the same order the options were listed in
                    let mut n = 1;
                    if i == 0 {
                        c.new_space = true;
                    }
                    for (sid, _, folders) in &spaces {
                        if i == n {
                            c.new_space = false;
                            c.space = *sid;
                            c.folder = None;
                        }
                        n += 1;
                        for (fid, _) in folders {
                            if i == n {
                                c.new_space = false;
                                c.space = *sid;
                                c.folder = Some(*fid);
                            }
                            n += 1;
                        }
                    }
                }
                KIND_DOC => c.doc_space = i.checked_sub(1).and_then(|j| spaces.get(j)).map(|s| s.0),
                _ => c.list = self.template_lists().get(i).copied(),
            }
        }
        if kind == KIND_LIST && !c.new_space {
            let r = ui.checkbox(
                &mut c.add_statuses,
                tr("Add the template's statuses to this Space"),
            );
            name_it(&r, "Add the template's statuses");
        }
        ui.add_space(10.0);
        let ok = match kind {
            KIND_LIST => c.new_space || spaces.iter().any(|s| s.0 == c.space),
            KIND_DOC => true,
            _ => c.list.is_some_and(|l| self.data.space_of_list(l).is_some()),
        };
        let mut made = false;
        ui.horizontal(|ui| {
            let r = ui.add_enabled(
                ok,
                Tint(
                    egui::Button::new(RichText::new(tr("Use template")).color(Color32::WHITE))
                        .fill(t.primary)
                        .corner_radius(7)
                        .min_size(vec2(110.0, 32.0)),
                ),
            );
            if r.clicked() {
                made = match kind {
                    KIND_LIST => {
                        let dest = if c.new_space {
                            ListDest::NewSpace
                        } else {
                            ListDest::Space {
                                space: c.space,
                                folder: c.folder,
                                add_statuses: c.add_statuses,
                            }
                        };
                        self.make_list_from_template(tp, &c.name, dest).is_some()
                    }
                    KIND_DOC => {
                        self.make_doc_from_template(
                            tp,
                            c.doc_space,
                            &c.name,
                            builtin_key == Some("meeting-notes"),
                        );
                        true
                    }
                    _ => c
                        .list
                        .and_then(|l| self.make_task_from_template(tp, l, &c.name))
                        .is_some(),
                };
                if made {
                    self.toast = Some((tr("Made from the template").into(), now_ts()));
                }
            }
        });
        made
    }

    /// The Lists a task template can go in: every open List in open Spaces.
    fn template_lists(&self) -> Vec<u64> {
        let mut v = vec![];
        for s in self.open_spaces() {
            v.extend(s.lists.iter().filter(|l| !l.archived).map(|l| l.id));
            for f in s.folders.iter().filter(|f| !f.archived) {
                v.extend(f.lists.iter().filter(|l| !l.archived).map(|l| l.id));
            }
        }
        v
    }

    /// A task template from scratch: name, description, checklist, priority.
    fn new_task_template_form(&mut self, ui: &mut Ui, t: &Theme, c: &mut TemplateCenter) {
        ui.label(RichText::new(tr("New task template")).size(16.0).strong());
        ui.label(
            RichText::new(tr(
                "Start new tasks from a saved shape: name, description, checklist and priority.",
            ))
            .small()
            .color(t.text2),
        );
        ui.add_space(6.0);
        let r = ui.add(
            egui::TextEdit::singleline(&mut c.new_name)
                .hint_text(tr("Template name, e.g. Bug report"))
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(8, 6)),
        );
        name_field(&r, "Template name, e.g. Bug report");
        focus_once(ui, &r);
        let r = ui.add(
            egui::TextEdit::multiline(&mut c.new_desc)
                .hint_text(tr("Description (optional)"))
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(8, 6)),
        );
        name_field(&r, "Description (optional)");
        let r = ui.add(
            egui::TextEdit::multiline(&mut c.new_checklist)
                .hint_text(tr("Checklist, one item per line (optional)"))
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(8, 6)),
        );
        name_field(&r, "Checklist, one item per line (optional)");
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Priority")).color(t.text2));
            let cur = if c.new_priority > 0 {
                tr(PRIORITIES[c.new_priority as usize].label)
            } else {
                tr("None")
            };
            egui::ComboBox::from_id_salt("tp_pr")
                .selected_text(cur)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut c.new_priority, 0, tr("None"));
                    for p in 1..PRIORITIES.len() {
                        ui.selectable_value(&mut c.new_priority, p as u8, tr(PRIORITIES[p].label));
                    }
                });
        });
        ui.add_space(8.0);
        let ok = !c.new_name.trim().is_empty();
        let b = ui.add_enabled(
            ok,
            Tint(
                egui::Button::new(RichText::new(tr("Create template")).color(Color32::WHITE))
                    .fill(t.primary)
                    .corner_radius(7)
                    .min_size(vec2(0.0, 30.0)),
            ),
        );
        if b.clicked() {
            let id = self.data.new_id();
            let items: Vec<String> = c
                .new_checklist
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            let name = c.new_name.trim().to_string();
            self.data.extra.templates.push(TaskTemplate {
                id,
                name: name.clone(),
                task_name: name.clone(),
                description: c.new_desc.trim().to_string(),
                checklist: items,
                priority: c.new_priority,
                ..Default::default()
            });
            c.new_name.clear();
            c.new_desc.clear();
            c.new_checklist.clear();
            c.new_priority = 0;
            c.pick = Some(Pick::Team(id));
            c.name = name;
            self.dirty = true;
        }
    }
}

/// What a template makes, drawn small.
fn template_preview(ui: &mut Ui, t: &Theme, tp: &TaskTemplate) {
    match tp.kind.as_str() {
        KIND_LIST => {
            ui.label(RichText::new(tr("Statuses")).small().color(t.text3));
            ui.horizontal_wrapped(|ui| {
                for s in &tp.statuses {
                    status_pill(ui, s, Sense::hover());
                }
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(count(tp.tasks.len(), "1 task", "{} tasks"))
                    .small()
                    .color(t.text3),
            );
            for x in &tp.tasks {
                ui.horizontal(|ui| {
                    let st = tp
                        .statuses
                        .iter()
                        .find(|s| s.name == x.status)
                        .or(tp.statuses.first());
                    if let Some(s) = st {
                        let (r, _) = ui.allocate_exact_size(vec2(12.0, 16.0), Sense::hover());
                        paint_status_circle(ui.painter(), r.center(), 5.0, s);
                    }
                    ui.label(&x.name);
                    if x.priority > 0 {
                        ui.label(
                            RichText::new(tr(PRIORITIES[x.priority as usize].label))
                                .small()
                                .color(rgb(PRIORITIES[x.priority as usize].color)),
                        );
                    }
                    if !x.checklist.is_empty() {
                        ui.label(
                            RichText::new(format!("\u{2611} {}", x.checklist.len()))
                                .small()
                                .color(t.text3),
                        );
                    }
                    for tag in &x.tags {
                        chip(ui, tag, tag_color(tag));
                    }
                });
            }
        }
        KIND_DOC => {
            ui.label(RichText::new(&tp.task_name).strong());
            ui.add_space(4.0);
            let lines: Vec<&str> = tp.description.lines().take(16).collect();
            ui.label(RichText::new(lines.join("\n")).color(t.text2));
        }
        _ => {
            ui.label(RichText::new(&tp.task_name).strong());
            if tp.priority > 0 || !tp.tags.is_empty() {
                ui.horizontal(|ui| {
                    if tp.priority > 0 {
                        ui.label(
                            RichText::new(crate::trf!(
                                "{} priority",
                                tr(PRIORITIES[tp.priority as usize].label)
                            ))
                            .small()
                            .color(rgb(PRIORITIES[tp.priority as usize].color)),
                        );
                    }
                    for tag in &tp.tags {
                        chip(ui, tag, tag_color(tag));
                    }
                });
            }
            if !tp.description.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(&tp.description).color(t.text2));
            }
            if !tp.checklist.is_empty() {
                ui.add_space(4.0);
                for item in &tp.checklist {
                    ui.label(format!("\u{2610}  {}", item));
                }
            }
        }
    }
}
