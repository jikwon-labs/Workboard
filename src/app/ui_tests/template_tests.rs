// Template Center: opened from the menus, searched, previewed and used with
// real clicks, and each kind of template making the right items.

use super::*;
use crate::app::templates::{self, Category, ListDest};

fn center(h: &Ui1) -> &templates::TemplateCenter {
    match &h.app.modal {
        Some(Modal::TemplateCenter(c)) => c,
        _ => panic!("the Template Center is open"),
    }
}

fn space_id(h: &Ui1, name: &str) -> u64 {
    h.app
        .data
        .spaces
        .iter()
        .find(|s| s.name == name)
        .unwrap()
        .id
}

fn tasks_in(h: &Ui1, list: u64) -> Vec<Task> {
    h.app
        .data
        .tasks
        .iter()
        .filter(|t| t.list_id == list)
        .cloned()
        .collect()
}

fn status_names(s: &Space) -> Vec<&str> {
    s.statuses.iter().map(|x| x.name.as_str()).collect()
}

fn open_center(h: &mut Ui1) {
    h.click("Create");
    h.click("Templates");
    center(h);
}

#[test]
fn template_center_opens_from_the_menus_and_searches() {
    let mut h = Ui1::new();
    open_center(&mut h);
    for name in [
        "Project tracker",
        "Bug tracker",
        "Content calendar",
        "Onboarding checklist",
        "Bug report",
        "Feature request",
        "Project Overview",
        "Meeting Notes",
        "Wiki",
    ] {
        assert!(
            h.has(&format!("Pick template {}", name)),
            "{} is listed",
            name
        );
    }
    h.click("Search templates");
    h.type_text("bug");
    assert!(h.has("Pick template Bug tracker") && h.has("Pick template Bug report"));
    assert!(
        !h.has("Pick template Wiki"),
        "the search leaves out the rest"
    );
    h.click("Template category Docs");
    assert!(h.has("No templates match your search."));
    h.click("Search templates");
    for _ in 0..3 {
        h.key(egui::Key::Backspace);
    }
    assert!(h.has("Pick template Meeting Notes"));
    assert!(!h.has("Pick template Bug report"), "Docs shows only Docs");

    // Workspace menu > Templates
    h.app.modal = None;
    h.settle();
    h.click("Workspace menu");
    h.click("Templates");
    assert_eq!(center(&h).cat, Category::All);

    // a Space's "+" aims List templates at that Space
    h.app.modal = None;
    h.settle();
    let side = space_id(&h, "Side Projects");
    h.click("Create in Side Projects");
    h.click("From a template");
    let c = center(&h);
    assert_eq!(c.cat, Category::Lists);
    assert!(!c.new_space);
    assert_eq!(c.space, side);

    // the Docs page's template row links here
    h.app.modal = None;
    h.open_docs_list();
    h.click("Browse all templates");
    assert_eq!(center(&h).cat, Category::Docs);

    // and so does Settings > Templates
    h.app.modal = None;
    h.app.open_settings("Templates");
    h.settle();
    assert!(h.has("Pick template Project tracker"));
}

#[test]
fn project_tracker_makes_a_space_with_its_statuses_and_tasks() {
    let mut h = Ui1::new();
    let spaces = h.app.data.spaces.len();
    let tasks = h.app.data.tasks.len();
    open_center(&mut h);
    assert!(h.has("Pick a template to see what it makes."));
    h.click("Pick template Project tracker");
    assert!(h.has("What it makes"));
    assert!(
        h.has("Write the project brief"),
        "the preview lists the tasks"
    );
    assert!(
        center(&h).new_space,
        "from the Create menu it makes a Space"
    );
    // the list sits beside the preview, and Use is in view without scrolling
    let rect = |name: &str| {
        h.tree
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .unwrap()
    };
    let row = rect("Pick template Project tracker");
    let dest = rect("Template destination");
    let use_b = rect("Use template");
    assert!(row.right() < dest.left(), "{:?} {:?}", row, dest);
    assert!(
        use_b.bottom() < 880.0 && use_b.top() > row.top(),
        "{:?}",
        use_b
    );
    h.click("Use template");
    assert!(h.app.modal.is_none(), "using it closes the center");
    assert_eq!(h.app.data.spaces.len(), spaces + 1);
    let s = h.app.data.spaces.last().unwrap().clone();
    assert_eq!(s.name, "Project tracker");
    assert_eq!(
        status_names(&s),
        ["to do", "in progress", "in review", "complete"]
    );
    assert_eq!(s.lists.len(), 1);
    let lid = s.lists[0].id;
    assert_eq!(h.app.loc, Location::List(lid), "the new List opens");
    let made = tasks_in(&h, lid);
    assert_eq!(made.len(), 5);
    assert_eq!(h.app.data.tasks.len(), tasks + 5);
    let brief = made
        .iter()
        .find(|t| t.name == "Write the project brief")
        .unwrap();
    assert_eq!(brief.priority, 2);
    assert_eq!(brief.checklist.len(), 3);
    assert!(brief.description.starts_with("Goals, scope"));
    assert!(made.iter().all(|t| t.status == "to do"));
}

#[test]
fn bug_tracker_in_an_existing_space_adds_the_statuses_it_needs() {
    let mut h = Ui1::new();
    let side = space_id(&h, "Side Projects");
    h.click("Create in Side Projects");
    h.click("From a template");
    assert_eq!(center(&h).space, side);
    h.click("Pick template Bug tracker");
    assert!(h.has("Add the template's statuses"));
    h.click("Use template");
    let s = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.id == side)
        .unwrap()
        .clone();
    assert_eq!(
        status_names(&s),
        [
            "to do",
            "in progress",
            "reported",
            "triaged",
            "fixed",
            "closed",
            "complete"
        ],
        "the Space keeps its own statuses and gains the missing ones"
    );
    let lid = list_id(&h, "Bug tracker");
    assert_eq!(h.app.data.space_of_list(lid).unwrap().id, side);
    let made = tasks_in(&h, lid);
    assert_eq!(made.len(), 2);
    let sign_in = made
        .iter()
        .find(|t| t.name == "Example: The sign-in button does nothing")
        .unwrap();
    assert_eq!(sign_in.status, "reported");
    assert_eq!(sign_in.tags, vec!["bug".to_string()]);
    assert_eq!(sign_in.checklist.len(), 3);
    assert!(made
        .iter()
        .any(|t| t.status == "triaged" && t.priority == 4));
}

#[test]
fn task_template_goes_into_the_chosen_list() {
    let mut h = Ui1::new();
    open_center(&mut h);
    h.click("Template category Tasks");
    assert!(!h.has("Pick template Project tracker"));
    h.click("Pick template Bug report");
    h.click("Template destination");
    h.click("Team Space / Projects / Marketing");
    // let the last click age so this one is not read as a double click
    for _ in 0..3 {
        h.settle();
    }
    h.click("Template item name");
    h.type_text(": sign-in fails");
    h.click("Use template");
    let mk = list_id(&h, "Marketing");
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "New bug: sign-in fails")
        .unwrap_or_else(|| {
            panic!(
                "the task is made with the name typed: {:?}",
                h.app
                    .data
                    .tasks
                    .iter()
                    .rev()
                    .take(2)
                    .map(|t| &t.name)
                    .collect::<Vec<_>>()
            )
        })
        .clone();
    assert_eq!(t.list_id, mk);
    assert_eq!(t.priority, 2);
    assert_eq!(t.tags, vec!["bug".to_string()]);
    assert_eq!(
        t.checklist
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>(),
        ["Reproduce", "Find the cause", "Fix", "Test the fix"]
    );
    assert!(t.description.starts_with("Steps to reproduce"));
    assert_eq!(h.app.open_task, Some(t.id), "the new task opens");
}

#[test]
fn doc_template_makes_a_doc_in_the_chosen_space() {
    let mut h = Ui1::new();
    let side = space_id(&h, "Side Projects");
    h.open_docs_list();
    h.click("Browse all templates");
    h.click("Pick template Meeting Notes");
    assert!(h.has("Doc title"));
    h.click("Template destination");
    // the drop-down's item, not the Space's own row in the sidebar
    let pick = h.rect_in(
        "Side Projects",
        Rect::from_min_max(pos2(320.0, 0.0), pos2(f32::INFINITY, f32::INFINITY)),
    );
    h.click_at(pick.center());
    h.click("Use template");
    let id = h.app.doc_open.expect("the new Doc opens");
    let d = h.app.data.docs.iter().find(|d| d.id == id).unwrap();
    assert_eq!(d.title, "Meeting Notes");
    assert_eq!(d.space, Some(side));
    assert!(d.meeting_note);
    assert!(d.body.ends_with("Action items\n- [ ] "));
    assert_eq!(h.app.section, Section::Docs);
}

#[test]
fn saved_list_template_is_used_renamed_and_deleted() {
    let mut h = Ui1::new();
    let mk = list_id(&h, "Marketing");
    let originals: Vec<String> = tasks_in(&h, mk)
        .iter()
        .filter(|t| t.parent.is_none() && !t.archived)
        .map(|t| t.name.clone())
        .collect();
    assert!(!originals.is_empty());
    h.click("List options Marketing");
    h.click("Save as template");
    assert_eq!(h.app.data.extra.templates.len(), 1);
    let tp = h.app.data.extra.templates[0].clone();
    assert_eq!(tp.kind, templates::KIND_LIST);
    assert_eq!(tp.tasks.len(), originals.len());
    assert_eq!(tp.statuses.len(), 3);

    open_center(&mut h);
    assert!(h.has("Your team"));
    h.click("Pick template Marketing");
    h.click("Rename template");
    h.type_text(" plan");
    h.key(egui::Key::Enter);
    assert_eq!(h.app.data.extra.templates[0].name, "Marketing plan");

    // use it in another Space
    h.click("Template destination");
    // the drop-down's item, not the Space's own row in the sidebar
    let pick = h.rect_in(
        "Side Projects",
        Rect::from_min_max(pos2(320.0, 0.0), pos2(f32::INFINITY, f32::INFINITY)),
    );
    h.click_at(pick.center());
    h.click("Use template");
    let side = space_id(&h, "Side Projects");
    let Location::List(made) = h.app.loc else {
        panic!("the new List opens")
    };
    assert_ne!(made, mk);
    assert_eq!(h.app.data.space_of_list(made).unwrap().id, side);
    assert_eq!(h.app.data.list_name(made), "Marketing");
    let mut names: Vec<String> = tasks_in(&h, made).into_iter().map(|t| t.name).collect();
    let mut want = originals.clone();
    names.sort();
    want.sort();
    assert_eq!(names, want, "the saved tasks come back");

    // delete asks first
    open_center(&mut h);
    h.click("Pick template Marketing plan");
    h.click("Delete template");
    assert_eq!(h.app.data.extra.templates.len(), 1);
    h.click("Confirm delete template");
    assert!(h.app.data.extra.templates.is_empty());
    assert!(!h.has("Pick template Marketing plan"));
}

#[test]
fn new_task_template_form_saves_a_team_template() {
    let mut h = Ui1::new();
    open_center(&mut h);
    h.click("New task template");
    h.click("Template name, e.g. Bug report");
    h.type_text("Release check");
    h.click("Checklist, one item per line (optional)");
    h.type_text("Tag the build\nWrite notes");
    h.click("Create template");
    let tp = h.app.data.extra.templates.last().unwrap().clone();
    assert_eq!(tp.name, "Release check");
    assert_eq!(tp.kind, "");
    assert_eq!(tp.checklist, ["Tag the build", "Write notes"]);
    assert!(
        h.has("Pick template Release check"),
        "it shows under Your team, picked"
    );
    assert!(h.has("Use template"));
}

#[test]
fn saved_doc_template_makes_the_same_doc() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let id = h.app.doc_open.unwrap();
    if let Some(d) = h.app.data.docs.iter_mut().find(|d| d.id == id) {
        d.title = "Weekly report".into();
        d.body = "Done\n- \n\nNext\n- ".into();
        d.tags = vec!["report".into()];
    }
    let tid = h.app.save_doc_as_template(id).unwrap();
    let tp = h
        .app
        .data
        .extra
        .templates
        .iter()
        .find(|x| x.id == tid)
        .unwrap()
        .clone();
    assert_eq!(tp.kind, templates::KIND_DOC);
    let side = space_id(&h, "Side Projects");
    let made = h.app.make_doc_from_template(&tp, Some(side), "", false);
    assert_ne!(made, id);
    let d = h.app.data.docs.iter().find(|d| d.id == made).unwrap();
    assert_eq!(d.title, "Weekly report");
    assert_eq!(d.body, "Done\n- \n\nNext\n- ");
    assert_eq!(d.tags, vec!["report".to_string()]);
    assert_eq!(d.space, Some(side));
}

#[test]
fn list_template_without_its_statuses_uses_the_space_ones() {
    let mut h = Ui1::new();
    let side = space_id(&h, "Side Projects");
    let before = status_names(h.app.data.spaces.iter().find(|s| s.id == side).unwrap())
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let b = templates::builtin("content-calendar").unwrap();
    let lid = h
        .app
        .make_list_from_template(
            &b.tpl,
            "",
            ListDest::Space {
                space: side,
                folder: None,
                add_statuses: false,
            },
        )
        .unwrap();
    let s = h.app.data.spaces.iter().find(|s| s.id == side).unwrap();
    assert_eq!(status_names(s), before);
    assert_eq!(h.app.data.list_name(lid), "Content calendar");
    let made = tasks_in(&h, lid);
    assert_eq!(made.len(), 4);
    assert!(
        made.iter().all(|t| t.status == "to do"),
        "statuses the Space lacks become its first"
    );
    assert!(made
        .iter()
        .any(|t| t.tags == ["blog"] && t.checklist.len() == 4));
}

#[test]
fn every_built_in_template_makes_its_items() {
    let mut h = Ui1::new();
    let all = templates::builtins();
    assert!((6..=10).contains(&all.len()));
    let mut keys: Vec<&str> = all.iter().map(|b| b.key).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), all.len(), "keys are unique");
    let list = list_id(&h, "Marketing");
    for b in &all {
        let tasks = h.app.data.tasks.len();
        let docs = h.app.data.docs.len();
        match b.tpl.kind.as_str() {
            templates::KIND_LIST => {
                let lid = h
                    .app
                    .make_list_from_template(&b.tpl, "", ListDest::NewSpace)
                    .unwrap();
                assert_eq!(h.app.data.list_name(lid), b.tpl.task_name);
                assert_eq!(tasks_in(&h, lid).len(), b.tpl.tasks.len(), "{}", b.key);
                assert!(!b.tpl.tasks.is_empty(), "{} has sample tasks", b.key);
                // every sample task's status is one of the template's
                for x in &b.tpl.tasks {
                    assert!(b.tpl.statuses.iter().any(|s| s.name == x.status));
                }
            }
            templates::KIND_DOC => {
                let id = h.app.make_doc_from_template(&b.tpl, None, "", false);
                assert_eq!(h.app.data.docs.len(), docs + 1);
                let d = h.app.data.docs.iter().find(|d| d.id == id).unwrap();
                assert_eq!(d.body, b.tpl.description);
                assert!(!d.body.is_empty());
            }
            _ => {
                h.app
                    .make_task_from_template(&b.tpl, list, "")
                    .expect("a task");
                assert_eq!(h.app.data.tasks.len(), tasks + 1);
                let t = h.app.data.tasks.last().unwrap();
                assert_eq!(t.name, b.tpl.task_name);
                assert_eq!(t.checklist.len(), b.tpl.checklist.len());
            }
        }
    }
}

#[test]
fn built_in_templates_are_made_in_the_interface_language() {
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let b = templates::builtin("bug-tracker").unwrap();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(b.tpl.name, "\u{bc84}\u{adf8} \u{ad00}\u{b9ac}");
    assert!(b
        .tpl
        .statuses
        .iter()
        .any(|s| s.name == "\u{bcf4}\u{ace0}\u{b428}"));
    // the default statuses keep their English names (shown translated)
    assert!(b.tpl.statuses.iter().any(|s| s.name == "in progress"));
}

#[test]
fn merge_statuses_keeps_the_space_first_status() {
    let mut to = default_statuses();
    let add = vec![
        Status::new("backlog", [1, 2, 3], StatusKind::Open),
        Status::new("complete", [0, 0, 0], StatusKind::Closed),
        Status::new("shipped", [4, 5, 6], StatusKind::Closed),
    ];
    templates::merge_statuses(&mut to, &add);
    let names: Vec<&str> = to.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        ["to do", "in progress", "backlog", "shipped", "complete"]
    );
    assert_eq!(to[0].kind, StatusKind::Open);
    assert_eq!(to[2].kind, StatusKind::Custom);
    assert_eq!(to[3].kind, StatusKind::Done, "one closed status stays");
}

#[test]
fn templates_keep_the_file_shape_older_apps_read() {
    // a template saved by an older app reads as a task template
    let old = r#"{"id":5,"name":"Bug","task_name":"Bug","description":"","checklist":[],"tags":[],"priority":1}"#;
    let tp: TaskTemplate = serde_json::from_str(old).unwrap();
    assert_eq!(tp.kind, "");
    assert!(tp.tasks.is_empty() && tp.statuses.is_empty());
    // and a task template is written the way older apps wrote it
    let v = serde_json::to_value(&tp).unwrap();
    assert!(v.get("kind").is_none() && v.get("tasks").is_none() && v.get("statuses").is_none());
    // a List template keeps everything, and its tasks have no ids, so
    // wb-sync treats the template as one item
    let b = templates::builtin("bug-tracker").unwrap().tpl;
    let v = serde_json::to_value(&b).unwrap();
    assert_eq!(v["kind"], "list");
    assert!(v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x.get("id").is_none()));
    let back: TaskTemplate = serde_json::from_value(v).unwrap();
    assert_eq!(back.tasks, b.tasks);
    assert_eq!(back.statuses.len(), b.statuses.len());
    // the old fields an older app needs are all there
    for k in [
        "id",
        "name",
        "task_name",
        "description",
        "checklist",
        "tags",
        "priority",
    ] {
        assert!(
            serde_json::to_value(&back).unwrap().get(k).is_some(),
            "{}",
            k
        );
    }
}

#[test]
fn long_template_lines_end_in_an_ellipsis_and_show_in_full_on_hover() {
    let mut h = Ui1::new();
    open_center(&mut h);
    let row = h.rect("Pick template Project tracker");
    let about = "Plan a project from kickoff to wrap-up, with a review step";
    let texts: Vec<(String, Rect)> = h
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(ts) => {
                let r = ts.visual_bounding_rect();
                row.contains(r.center())
                    .then(|| (ts.galley.text().to_string(), r))
            }
            _ => None,
        })
        .collect();
    let line = texts
        .iter()
        .find(|(s, _)| about.starts_with(s.trim_end_matches('\u{2026}')) && s.len() > 4)
        .unwrap_or_else(|| panic!("the description is drawn: {:?}", texts));
    assert!(
        line.0.ends_with('\u{2026}'),
        "a description too long for the row ends in an ellipsis: {:?}",
        line.0
    );
    for (s, r) in &texts {
        assert!(r.right() <= row.right(), "{:?} runs past the row", s);
    }
    // the tooltip comes up after the pointer rests a moment
    h.hover(row.center());
    for _ in 0..12 {
        h.frame(vec![]);
    }
    let shown = h.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Text(ts) => ts.galley.text().contains(about),
        _ => false,
    });
    assert!(shown, "hovering the row shows the whole description");
}
