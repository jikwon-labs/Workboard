// Click-through tests for tasks, Lists, Spaces and folders: creating,
// editing, moving, the task window and its fields, bulk actions, the
// status editor and trash.

use super::*;

fn pri(h: &Ui1, name: &str) -> u8 {
    task_named(h, name).priority
}

/// A workspace with no List a task could go in: no Spaces at all, or
/// (with `keep_space`) a Space whose Lists are gone.
fn no_list_workspace(keep_space: bool) -> Ui1 {
    let mut h = Ui1::new();
    if keep_space {
        for s in h.app.data.spaces.iter_mut() {
            s.lists.clear();
            s.folders.clear();
        }
    } else {
        h.app.data.spaces.retain(|s| s.private_owner.is_some());
        for s in h.app.data.spaces.iter_mut() {
            s.lists.clear();
            s.folders.clear();
        }
    }
    assert_eq!(h.app.data.first_list(), None);
    h.settle();
    h
}

/// Opens the first space's status editor and, mid-edit, renames "to do" to
/// "backlog" and empties "in progress". Returns (space, a task on "to do",
/// a task on "in progress").
fn status_edit_half_done(h: &mut Ui1) -> (u64, u64, u64) {
    let sid = h.app.data.spaces[0].id;
    let list = h.app.data.lists_in(Location::Space(sid)).unwrap()[0];
    let ids: Vec<u64> = h
        .app
        .data
        .tasks
        .iter()
        .filter(|t| t.list_id == list)
        .map(|t| t.id)
        .collect();
    let (a, b) = (ids[0], ids[1]);
    for (t, st) in [(a, "to do"), (b, "in progress")] {
        h.app
            .data
            .tasks
            .iter_mut()
            .find(|x| x.id == t)
            .unwrap()
            .status = st.into();
    }
    h.app.apply(Action::EditStatuses(sid));
    h.settle();
    h.click("to do");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("backlog");
    h.click("in progress");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.settle();
    assert!(h.app.status_edit.is_some());
    (sid, a, b)
}

#[test]
fn click_create_menu_task_and_type_it() {
    let mut h = Ui1::new();
    let n = h.app.data.tasks.len();
    h.click("Create");
    h.click("Task");
    assert!(h.app.quick_add.is_some(), "the Create Task window opens");
    h.type_text("Clicked into existence");
    h.key(egui::Key::Enter);
    assert_eq!(h.app.data.tasks.len(), n + 1);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.name == "Clicked into existence"));
}

#[test]
fn click_add_task_row_in_list() {
    let mut h = Ui1::new();
    h.click("Add Task in TO DO");
    h.type_text("Inline task");
    h.key(egui::Key::Enter);
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Inline task")
        .expect("added from the list row");
    assert_eq!(t.status, "to do");
}

#[test]
fn click_status_circle_and_pick_status() {
    let mut h = Ui1::new();
    h.click("Status of Set up analytics dashboard");
    h.click("IN PROGRESS");
    let t = task_named(&h, "Set up analytics dashboard");
    assert_eq!(t.status, "in progress");
}

#[test]
fn click_task_row_opens_it_then_complete_and_timer() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.expect("task window opens");
    let play = h.rect("Start timer");
    assert!(
        (play.width() - play.height()).abs() < 0.5,
        "the play button is a circle, not taller than wide: {play:?}"
    );
    h.click("Start timer");
    assert!(
        h.app.task(id).unwrap().timer_started.is_some(),
        "the play button starts the timer"
    );
    h.click("Mark complete");
    assert_eq!(
        h.app.task(id).unwrap().status,
        "complete",
        "the check button completes the task"
    );
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none(), "Esc closes the task");
}

#[test]
fn right_click_delete_then_restore_from_trash() {
    let mut h = Ui1::new();
    h.right_click("Task Collect feedback from beta users", 120.0);
    h.click("Delete");
    assert!(!h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.name == "Collect feedback from beta users"));
    h.click("Profile menu");
    h.click("Trash");
    assert!(matches!(h.app.modal, Some(Modal::Trash)));
    h.click("Restore");
    assert!(
        h.app
            .data
            .tasks
            .iter()
            .any(|t| t.name == "Collect feedback from beta users"),
        "restored from the trash"
    );
    h.key(egui::Key::Escape);
    assert!(h.app.modal.is_none());
}

#[test]
fn create_list_opens_a_dialog_and_lands_in_a_visible_space() {
    let mut h = Ui1::new();
    h.click("My Tasks");
    h.click("Personal List");
    h.click("Create");
    h.click("List");
    assert!(
        matches!(h.app.modal, Some(Modal::NewList { .. })),
        "Create List dialog"
    );
    h.type_text("Roadmap");
    h.key(egui::Key::Enter);
    let Location::List(id) = h.app.loc else {
        panic!("opened the new list")
    };
    assert_eq!(h.app.data.list_name(id), "Roadmap");
    assert!(h
        .app
        .data
        .space_of_list(id)
        .unwrap()
        .private_owner
        .is_none());
}

#[test]
fn right_click_task_move_to_another_list() {
    let mut h = Ui1::new();
    h.right_click("Task Set up analytics dashboard", 60.0);
    h.click("Move to...");
    assert!(matches!(h.app.modal, Some(Modal::MoveTasks { .. })));
    h.click("Team Space / Projects / Marketing");
    let t = task_named(&h, "Set up analytics dashboard");
    assert_eq!(h.app.data.list_name(t.list_id), "Marketing");
}

#[test]
fn tick_two_rows_and_archive_them_from_the_bar() {
    let mut h = Ui1::new();
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Select Write copy for landing page");
    h.click("Select Set up analytics dashboard");
    assert_eq!(h.app.selected.len(), 2);
    assert!(h.has("2 tasks selected"));
    h.click("Archive");
    let archived = h.app.data.tasks.iter().filter(|t| t.archived).count();
    assert_eq!(archived, 2);
    assert!(h.app.selected.is_empty());
}

#[test]
fn row_highlight_covers_the_selection_box_and_quick_buttons_are_drawn() {
    let mut h = Ui1::new();
    let name = "Write copy for landing page";
    h.hover(h.find(&format!("Task {}", name)).unwrap());
    // the box drawn inside its click area
    let boxr = h.rect(&format!("Select {}", name)).shrink(3.0);
    let sel = boxr.left_center() + egui::vec2(1.0, 0.0);
    // pointer on the left of the selection box: the row stays highlighted and
    // the grey band covers the whole box (it used to start halfway through it)
    h.hover(sel);
    let hover = h.app.t().hover;
    let row = h.rect(&format!("Task {}", name));
    let band = h.shapes.iter().find_map(|c| match &c.shape {
        egui::Shape::Rect(r)
            if r.fill == hover && r.rect.contains_rect(boxr) && r.rect.contains(row.center()) =>
        {
            Some((r.rect, c.clip_rect))
        }
        _ => None,
    });
    let (band, clip) = band.expect("the row highlight covers the selection box");
    assert!(
        clip.contains(band.left_center()),
        "the highlight is not clipped"
    );
    // the quick buttons are painted icons, not a pencil glyph a font may lack
    assert!(h.has(&format!("Rename {}", name)));
    assert!(h.painted_text("\u{270E}").is_empty());
}

#[test]
fn row_dots_menu_duplicates_a_task() {
    let mut h = Ui1::new();
    let n = h.app.data.tasks.len();
    h.hover(h.find("Task Set up analytics dashboard").unwrap());
    h.click("Task options Set up analytics dashboard");
    h.click("Duplicate");
    assert_eq!(h.app.data.tasks.len(), n + 1);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.name == "Set up analytics dashboard (copy)"));
}

#[test]
fn list_dots_menu_moves_a_list() {
    let mut h = Ui1::new();
    h.hover(h.find("Marketing").unwrap());
    h.click("List options Marketing");
    h.click("Move...");
    assert!(matches!(h.app.modal, Some(Modal::MoveList { .. })));
    h.click_side("Side Projects", true);
    let id = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.folders
                .iter()
                .flat_map(|f| f.lists.iter())
                .chain(s.lists.iter())
        })
        .find(|l| l.name == "Marketing")
        .unwrap()
        .id;
    assert_eq!(h.app.data.space_of_list(id).unwrap().name, "Side Projects");
}

#[test]
fn bulk_priority_opening_the_menu_changes_nothing() {
    let mut h = Ui1::new();
    let a = "Write copy for landing page";
    let b = "Set up analytics dashboard";
    for t in h
        .app
        .data
        .tasks
        .iter_mut()
        .filter(|t| t.name == a || t.name == b)
    {
        t.priority = 2;
    }
    h.hover(h.find(&format!("Task {}", a)).unwrap());
    h.click(&format!("Select {}", a));
    h.click(&format!("Select {}", b));
    assert!(h.has("2 tasks selected"));
    h.click("Priority");
    assert!(h.has("Clear"), "the priority menu is open");
    h.settle();
    assert_eq!(
        (pri(&h, a), pri(&h, b)),
        (2, 2),
        "just opening the menu keeps the priority"
    );
    h.click("Urgent");
    assert_eq!((pri(&h, a), pri(&h, b)), (1, 1));
    h.click("Priority");
    h.click("Clear");
    assert_eq!((pri(&h, a), pri(&h, b)), (0, 0), "Clear still clears");
}

#[test]
fn date_picker_fits_its_content_and_shows_set_recurring() {
    let mut h = Ui1::new();
    let mut rows: Vec<(String, Rect)> = h
        .tree
        .iter()
        .filter(|(n, _)| n.starts_with("Due date of "))
        .cloned()
        .collect();
    rows.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    assert!(rows.len() >= 2);
    for (name, _) in [rows.first().unwrap().clone(), rows.last().unwrap().clone()] {
        h.click(&name);
        let today = h.rect("Today");
        let weekly = h.rect("Weekly");
        assert!(
            weekly.bottom() <= 900.0 && weekly.top() >= 0.0,
            "{}: Set Recurring is on screen ({:?})",
            name,
            weekly
        );
        assert!(
            weekly.top() - today.top() < 380.0,
            "{}: the menu is only as tall as its content ({:?} .. {:?})",
            name,
            today,
            weekly
        );
        h.key(egui::Key::Escape);
    }
}

/// The task window's "..." menu draws its rows like the right-click menu:
/// plain lines, not a stack of outlined buttons. The header has no separate
/// "ID" button (the menu has Copy ID) and no second eye beside Watch.
#[test]
fn task_window_menu_rows_have_no_frame() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    assert!(
        h.painted_text("ID").is_empty(),
        "no ID button in the header"
    );
    h.click("Task actions");
    assert!(h.has("Duplicate"), "the menu is open");
    fn boxes(s: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match s {
            egui::Shape::Rect(r) => out.push(r.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| boxes(s, out)),
            _ => {}
        }
    }
    let mut found = vec![];
    for c in &h.shapes {
        boxes(&c.shape, &mut found);
    }
    for row in ["Duplicate", "Copy ID", "Copy link", "Delete task"] {
        let r = h.rect(row);
        let framed = found.iter().any(|b| {
            b.rect.expand(1.0).contains_rect(r)
                && b.rect.height() < r.height() + 8.0
                && (b.stroke.width > 0.0 || b.fill.a() > 0)
        });
        assert!(!framed, "{row} is drawn without a box");
    }
}

#[test]
fn escape_closes_the_task_menu_first_then_the_task() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    assert!(h.app.open_task.is_some());
    h.click("Task actions");
    assert!(h.has("Duplicate"), "the task window menu is open");
    h.key(egui::Key::Escape);
    assert!(!h.has("Duplicate"), "Escape closes the menu");
    assert!(h.app.open_task.is_some(), "and leaves the task open");
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none(), "a second Escape closes the task");

    // same for a row "..." menu with ticked rows: the selection stays
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Select Write copy for landing page");
    h.click("Task options Write copy for landing page");
    assert_eq!(h.app.selected.len(), 1);
    assert!(h.has("Add subtask"), "the row menu is open");
    h.key(egui::Key::Escape);
    assert_eq!(
        h.app.selected.len(),
        1,
        "Escape on the menu keeps the selection"
    );
}

#[test]
fn create_task_window_has_description_status_and_tags() {
    let mut h = Ui1::new();
    h.click("Create");
    h.click("Task");
    assert!(h.has("Close"), "header with a close button");
    assert!(h.has("Task Name"));
    h.type_text("Full task");
    h.click("Add description");
    h.type_text("Some details");
    h.click("Task status");
    h.click("IN PROGRESS");
    h.click("Tags");
    h.type_text("urgent-fix");
    h.key(egui::Key::Enter);
    h.key(egui::Key::Escape);
    assert!(
        h.app.quick_add.is_some(),
        "Escape closes the tag popup first"
    );
    h.click_side("Create Task", true); // the button, right of the heading
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Full task")
        .expect("created");
    assert_eq!(t.description, "Some details");
    assert_eq!(t.status, "in progress");
    assert_eq!(t.tags, vec!["urgent-fix".to_string()]);
    assert!(h.app.quick_add.is_none());
}

#[test]
fn create_task_window_closes_from_the_header_x() {
    let mut h = Ui1::new();
    h.click("Create");
    h.click("Task");
    h.click("Close");
    assert!(h.app.quick_add.is_none());
}

#[test]
fn task_window_subtask_and_checklist_rows_open_their_fields() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.unwrap();
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .checklist
        .clear();
    h.settle();
    assert!(
        !h.has("Subtask name"),
        "no subtask field until the row is clicked"
    );
    h.click("+  Add subtask");
    h.type_text("Child one");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    h.settle();
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.parent == Some(id) && t.name == "Child one"));
    assert!(!h.has("Checklist item"));
    h.click("+  Create checklist");
    h.type_text("Step A");
    h.key(egui::Key::Enter);
    h.settle();
    assert_eq!(h.app.task(id).unwrap().checklist.len(), 1);
    assert!(h.app.open_task.is_some());
}

#[test]
fn task_window_assign_this_task_button() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.unwrap();
    {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.activity.retain(|a| !a.comment);
        t.assignees.clear();
    }
    h.settle();
    h.click("Assign this task");
    h.click_side("Sam Lee", true);
    h.settle();
    assert!(h.app.task(id).unwrap().assignees.contains(&3));
}

#[test]
fn closing_a_task_records_date_done_and_closed() {
    let mut h = Ui1::new();
    let mut t = task_named(&h, "Write copy for landing page").clone();
    assert!(t.closed_at.is_none());
    t.status = "complete".into();
    h.app.actions.push(Action::Replace(t.clone()));
    h.app.apply_actions();
    let after = h.app.task(t.id).unwrap().clone();
    assert!(after.closed_at.is_some() && after.done_at.is_some());
    assert!(!h.app.prop_text(&after, "Date closed").is_empty());
    t.status = "to do".into();
    h.app.actions.push(Action::Replace(t.clone()));
    h.app.apply_actions();
    assert!(
        h.app.task(t.id).unwrap().closed_at.is_none(),
        "reopening clears it"
    );
    // old data without the fields still loads
    let mut v = serde_json::to_value(h.app.task(t.id).unwrap()).unwrap();
    v.as_object_mut().unwrap().remove("closed_at");
    v.as_object_mut().unwrap().remove("done_at");
    let back: Task = serde_json::from_value(v).unwrap();
    assert!(back.closed_at.is_none());
}

#[test]
fn list_plus_opens_a_create_menu() {
    let mut h = Ui1::new();
    h.click("Create in Marketing");
    assert!(
        h.app.quick_add.is_none(),
        "the + opens a menu, not the Create Task window"
    );
    for item in [
        "Create Task",
        "Create List",
        "Create Doc",
        "Create Dashboard",
        "Create Whiteboard",
        "Create Form",
    ] {
        assert!(h.has(item), "{} in the menu", item);
    }
    h.click("Create Task");
    assert!(h.app.quick_add.is_some());
    let mk = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .find(|l| l.name == "Marketing")
        .unwrap()
        .id;
    assert_eq!(
        h.app.quick_add.as_ref().unwrap().list,
        mk,
        "the task goes to that List"
    );
}

#[test]
fn list_menu_info_archive_and_restore_from_settings() {
    let mut h = Ui1::new();
    let mk = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .find(|l| l.name == "Marketing")
        .unwrap()
        .id;
    h.click("List options Marketing");
    h.click("List Info");
    assert!(matches!(h.app.modal, Some(Modal::ListInfo { .. })));
    h.click("List description");
    h.type_text("Campaign work");
    assert_eq!(
        h.app.data.list_item(mk).unwrap().description,
        "Campaign work"
    );
    h.key(egui::Key::Escape);
    // the "..." leaves creating to the "+" beside it
    h.click("List options Marketing");
    assert!(!h.has("Create new"));
    h.key(egui::Key::Escape);
    // Archive: gone from the sidebar and from All Tasks
    h.click("List options Marketing");
    h.click("Archive");
    assert!(h.app.data.list_item(mk).unwrap().archived);
    assert!(
        !h.has("List options Marketing"),
        "an archived List leaves the sidebar"
    );
    h.app.loc = Location::Everything;
    assert!(h.app.filtered().iter().all(|t| t.list_id != mk));
    // Settings > Spaces lists it with Restore
    h.app.section = Section::Settings;
    h.settle();
    h.click("Settings Spaces");
    h.click("Restore Team Space / Projects / Marketing");
    assert!(!h.app.data.list_item(mk).unwrap().archived);
}

#[test]
fn space_menu_duplicate_hide_archive_and_icon() {
    let mut h = Ui1::new();
    let sid = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.name == "Side Projects")
        .unwrap()
        .id;
    let n = h.app.data.spaces.len();
    // Color & Icon
    h.click("Space options Side Projects");
    h.click("Color & Icon");
    h.click("Space icon \u{2605}");
    assert_eq!(
        h.app.data.spaces.iter().find(|s| s.id == sid).unwrap().icon,
        "\u{2605}"
    );
    // Duplicate copies lists and tasks
    h.click("Space options Side Projects");
    h.click("Duplicate");
    h.app.apply_actions();
    assert_eq!(h.app.data.spaces.len(), n + 1);
    let copy = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.name == "Side Projects (copy)")
        .unwrap()
        .clone();
    let orig_lists = h.app.data.lists_in(Location::Space(sid)).unwrap().len();
    let copy_lists = h.app.data.lists_in(Location::Space(copy.id)).unwrap();
    assert_eq!(copy_lists.len(), orig_lists);
    assert!(
        h.app
            .data
            .tasks
            .iter()
            .any(|t| copy_lists.contains(&t.list_id)),
        "tasks come along"
    );
    h.settle();
    // Hide Space: off the sidebar, back from Settings
    h.click("Space options Side Projects (copy)");
    h.click("Hide Space");
    assert!(!h.has("Space options Side Projects (copy)"));
    h.app.section = Section::Settings;
    h.settle();
    h.click("Settings Spaces");
    h.click("Show Side Projects (copy)");
    assert!(
        !h.app
            .data
            .spaces
            .iter()
            .find(|s| s.id == copy.id)
            .unwrap()
            .hidden
    );
    // Archive, then Restore from Settings
    h.click("\u{2190}  Back to Workspace");
    h.click("Space options Side Projects (copy)");
    h.click("Archive");
    assert!(
        h.app
            .data
            .spaces
            .iter()
            .find(|s| s.id == copy.id)
            .unwrap()
            .archived
    );
    h.app.section = Section::Settings;
    h.settle();
    h.click("Settings Spaces");
    h.click("Restore Side Projects (copy)");
    assert!(
        !h.app
            .data
            .spaces
            .iter()
            .find(|s| s.id == copy.id)
            .unwrap()
            .archived
    );
}

#[test]
fn old_space_and_list_files_still_load() {
    let s: Space = serde_json::from_str(r#"{"id":1,"name":"S","color":[1,2,3],"statuses":[],"folders":[],"lists":[{"id":2,"name":"L"}]}"#).unwrap();
    assert!(!s.archived && !s.hidden && s.icon.is_empty());
    assert!(!s.lists[0].archived && s.lists[0].owner.is_none());
}

#[test]
fn escape_in_the_subtask_field_keeps_the_task_open() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    h.click("+  Add subtask");
    h.type_text("Child");
    h.key(egui::Key::Enter);
    h.key(egui::Key::Escape);
    assert!(
        h.app.open_task.is_some(),
        "Escape leaves the field, not the task"
    );
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none(), "a second Escape closes the task");
}

#[test]
fn narrow_task_window_fields_stay_left_of_the_comments() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.click_left("Task Write copy for landing page", 60.0);
    assert!(h.app.open_task.is_some());
    let panel = h.rect("Comments").left() - 12.0;
    for name in [
        "Time entries",
        "Time estimate value",
        "Track time",
        "Priority",
        "Assignees",
        "Time estimate",
    ] {
        let r = h.rect(name);
        assert!(
            r.right() <= panel,
            "{:?} {:?} runs into the comments column (from x {})",
            name,
            r,
            panel
        );
    }
    // the wide window keeps two fields per row
    let mut w = Ui1::new();
    w.click_left("Task Write copy for landing page", 120.0);
    let (st, asg) = (w.rect("Status"), w.rect("Assignees"));
    assert!(
        (st.center().y - asg.center().y).abs() < 4.0,
        "Status and Assignees share a row: {:?} {:?}",
        st,
        asg
    );
}

#[test]
fn narrow_task_window_has_no_blank_rows_when_fields_are_off() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.app.data.extra.apps.priorities = false;
    h.app.data.extra.apps.time_tracking = false;
    h.settle();
    h.click_left("Task Write copy for landing page", 60.0);
    let dates = h.rect("Dates");
    let est = h.rect("Time estimate");
    let tags = h.rect("Tags");
    let step = h.rect("Assignees").center().y - h.rect("Status").center().y;
    assert!(
        est.center().y - dates.center().y < step * 1.5,
        "no gap after Dates: {:?} {:?} (row step {})",
        dates,
        est,
        step
    );
    assert!(
        tags.center().y - est.center().y < step * 1.5,
        "no gap after Time estimate: {:?} {:?}",
        est,
        tags
    );
}

#[test]
fn drag_a_row_into_a_new_place() {
    let mut h = Ui1::new();
    let row_y = |h: &Ui1, n: &str| h.rect(&format!("Task {}", n)).center().y;
    assert!(
        row_y(&h, "Collect feedback from beta users") > row_y(&h, "Write copy for landing page")
    );
    h.drag(
        "Task Collect feedback from beta users",
        "Task Write copy for landing page",
    );
    assert!(h.app.open_task.is_none(), "a drag does not open the task");
    assert!(
        row_y(&h, "Collect feedback from beta users") < row_y(&h, "Write copy for landing page"),
        "the dragged row now comes first"
    );
    // dropped into another status group, it takes that status
    h.drag(
        "Task Set up analytics dashboard",
        "Task Fix navigation on mobile",
    );
    let t = h.app.task(h.task_id("Set up analytics dashboard")).unwrap();
    assert_eq!(t.status, "in progress");
}

#[test]
fn shift_click_selects_a_range_and_cmd_a_selects_all() {
    let mut h = Ui1::new();
    h.hover(h.find("Task Design new homepage layout").unwrap());
    h.click("Select Design new homepage layout");
    h.click_with(
        "Task Set up analytics dashboard",
        egui::Modifiers::SHIFT,
        Some(60.0),
    );
    // to do: Write copy, Set up analytics, Collect feedback; in progress: Design, Fix navigation
    assert_eq!(h.app.selected.len(), 3, "{:?}", h.app.selected);
    assert!(!h
        .app
        .selected
        .contains(&h.task_id("Write copy for landing page")));
    assert!(h
        .app
        .selected
        .contains(&h.task_id("Collect feedback from beta users")));
    h.key(egui::Key::Escape);
    assert!(h.app.selected.is_empty());
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    assert_eq!(h.app.selected.len(), 5, "every row shown");
}

#[test]
fn delete_key_asks_then_undo_brings_tasks_back() {
    let mut h = Ui1::new();
    let n = h.app.data.tasks.len();
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Select Write copy for landing page");
    h.click("Select Set up analytics dashboard");
    h.key(egui::Key::Backspace);
    assert!(h.app.confirm_bulk_delete, "Delete asks first");
    assert_eq!(h.app.data.tasks.len(), n);
    h.click("Confirm delete");
    assert_eq!(h.app.data.tasks.len(), n - 2);
    assert!(h.has("2 tasks deleted"));
    h.click("Undo delete");
    assert_eq!(h.app.data.tasks.len(), n);
    assert!(h.app.data.extra.trash.is_empty());
    // the row menu deletes at once, with the same Undo
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Task options Write copy for landing page");
    h.click("Delete");
    assert_eq!(h.app.data.tasks.len(), n - 1);
    h.click("Undo delete");
    assert_eq!(h.app.data.tasks.len(), n);
}

#[test]
fn group_menu_selects_and_collapses() {
    let mut h = Ui1::new();
    h.hover(h.find("Group options TO DO").unwrap());
    h.click("Group options TO DO");
    h.click("Select all in group");
    assert_eq!(h.app.selected.len(), 3);
    h.key(egui::Key::Escape);
    h.hover(h.find("Group options TO DO").unwrap());
    h.click("Group options TO DO");
    h.click("Collapse all groups");
    assert!(
        !h.has("Task Write copy for landing page") && !h.has("Task Design new homepage layout")
    );
    h.hover(h.find("Group options TO DO").unwrap());
    h.click("Group options TO DO");
    h.click("Expand all groups");
    assert!(h.has("Task Write copy for landing page"));
}

#[test]
fn row_quick_actions_rename_subtask_and_tag() {
    let mut h = Ui1::new();
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Rename Write copy for landing page");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("Landing page copy");
    h.key(egui::Key::Enter);
    let id = h.task_id("Landing page copy");
    assert!(h.app.open_task.is_none());
    h.hover(h.find("Task Landing page copy").unwrap());
    h.click("Add subtask Landing page copy");
    h.type_text("Headline");
    h.key(egui::Key::Enter);
    let sub = h.app.task(h.task_id("Headline")).unwrap();
    assert_eq!(sub.parent, Some(id));
    h.key(egui::Key::Escape);
    h.hover(h.find("Task Landing page copy").unwrap());
    h.click("Add tag Landing page copy");
    h.click("design");
    assert!(h.app.task(id).unwrap().tags.contains(&"design".to_string()));
    // the row menu's Rename also renames in place
    h.key(egui::Key::Escape);
    h.right_click("Task Set up analytics dashboard", 60.0);
    h.click("Rename");
    assert!(h.app.row.rename.is_some() && h.app.rename.is_none());
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("Analytics");
    h.key(egui::Key::Enter);
    h.task_id("Analytics");
}

#[test]
fn arrow_keys_and_enter_open_a_row() {
    let mut h = Ui1::new();
    h.key(egui::Key::ArrowDown);
    h.key(egui::Key::ArrowDown);
    h.key(egui::Key::Enter);
    // the second row: TO DO comes first
    assert_eq!(
        h.app.open_task,
        Some(h.task_id("Set up analytics dashboard"))
    );
}

#[test]
fn bulk_bar_tags_unassign_start_date_and_unarchive() {
    let mut h = Ui1::new();
    let a = "Write copy for landing page";
    let b = "Design new homepage layout";
    h.hover(h.find(&format!("Task {}", a)).unwrap());
    h.click(&format!("Select {}", a));
    h.click(&format!("Select {}", b));
    h.click("Tags");
    h.type_text("launch");
    h.key(egui::Key::Enter);
    for n in [a, b] {
        assert!(
            h.app
                .task(h.task_id(n))
                .unwrap()
                .tags
                .contains(&"launch".to_string()),
            "{}",
            n
        );
    }
    h.key(egui::Key::Escape);
    h.click("Assignees");
    h.click("Unassign all");
    for n in [a, b] {
        assert!(h.app.task(h.task_id(n)).unwrap().assignees.is_empty());
    }
    h.click("Start date");
    h.click("Today");
    for n in [a, b] {
        assert_eq!(h.app.task(h.task_id(n)).unwrap().start, Some(today()));
    }
    h.click("Archive");
    h.app.filter.archived = true;
    h.settle();
    h.hover(h.find(&format!("Task {}", a)).unwrap());
    h.click(&format!("Select {}", a));
    h.click(&format!("Select {}", b));
    h.click("Unarchive");
    assert!(!h.app.task(h.task_id(a)).unwrap().archived);
}

#[test]
fn task_type_pill_switches_to_milestone() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("Task type");
    h.click("Milestone");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().milestone);
    h.click("Task type");
    h.click_side("Task", true);
    h.app.apply_actions();
    assert!(!h.app.task(id).unwrap().milestone);
}

#[test]
fn time_estimate_takes_hours_and_minutes_as_text() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("Time estimate value");
    h.type_text("2h 30m");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    assert_eq!(h.app.task(id).unwrap().estimate_min, Some(150));
    assert!(h.has("2h 30m"), "shown back as text");
}

#[test]
fn time_entries_add_by_hand_and_remove() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .time_entries
        .clear();
    h.settle();
    h.click("Time entries");
    h.click("Time to add");
    h.type_text("1h 30m");
    h.click("Add time");
    h.app.apply_actions();
    let t = h.app.task(id).unwrap();
    assert_eq!(t.time_entries.len(), 1);
    assert_eq!(t.time_entries[0].end - t.time_entries[0].start, 5400);
    h.click("Remove entry 1");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().time_entries.is_empty());
}

#[test]
fn tag_field_suggests_tags_in_use() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("Add tag");
    h.type_text("bu");
    assert!(h.has("Tag bug"), "the tag used on another task is offered");
    assert!(!h.has("Tag content"), "narrowed by what is typed");
    h.click("Tag bug");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().tags.contains(&"bug".to_string()));
}

#[test]
fn watch_toggle_and_watchers_menu() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    let me = h.app.data.me;
    h.click("Watch task");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().watchers.contains(&me));
    assert!(h.has("Stop watching"));
    h.click("Watchers");
    h.click("Watcher Sam Lee");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().watchers.contains(&3));
    h.key(egui::Key::Escape);
    h.click("Stop watching");
    h.app.apply_actions();
    assert_eq!(h.app.task(id).unwrap().watchers, vec![3]);
}

#[test]
fn checklist_items_rename_reorder_assign_and_become_subtasks() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    // rename by clicking the text
    h.click("Sketch three options");
    assert!(h.has("Checklist item name"));
    h.type_text(" now");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    assert_eq!(
        h.app.task(id).unwrap().checklist[1].text,
        "Sketch three options now"
    );
    // on hover: move up
    let p = h.find("Sketch three options now").unwrap();
    h.hover(p);
    h.click("Move up Sketch three options now");
    h.app.apply_actions();
    assert_eq!(
        h.app.task(id).unwrap().checklist[0].text,
        "Sketch three options now"
    );
    // assign
    let p = h.find("Share with the team").unwrap();
    h.hover(p);
    h.click("Assign item Share with the team");
    h.click_side("Sam Lee", true);
    h.app.apply_actions();
    assert_eq!(h.app.task(id).unwrap().checklist[2].assignee, Some(3));
    // turn into a subtask (it keeps the assignee)
    let p = h.find("Share with the team").unwrap();
    h.hover(p);
    h.click("Make subtask Share with the team");
    h.app.apply_actions();
    let t = h.app.task(id).unwrap();
    assert_eq!(t.checklist.len(), 2);
    let sub = h
        .app
        .data
        .tasks
        .iter()
        .find(|x| x.parent == Some(id) && x.name == "Share with the team")
        .expect("a subtask");
    assert_eq!(sub.assignees, vec![3]);
}

#[test]
fn subtask_rows_set_due_make_top_level_and_delete() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    let sub = |h: &Ui1, n: &str| {
        h.app
            .data
            .tasks
            .iter()
            .find(|t| t.name == n)
            .map(|t| (t.id, t.parent, t.due))
    };
    h.click("Subtask due Pick hero image");
    h.click("Today");
    h.app.apply_actions();
    assert_eq!(sub(&h, "Pick hero image").unwrap().2, Some(today()));
    let p = h.find("Subtask due Wireframe hero section").unwrap();
    h.hover(p);
    h.click("Make top-level Wireframe hero section");
    h.app.apply_actions();
    assert_eq!(sub(&h, "Wireframe hero section").unwrap().1, None);
    let p = h.find("Subtask due Pick hero image").unwrap();
    h.hover(p);
    h.click("Delete subtask Pick hero image");
    h.app.apply_actions();
    assert!(sub(&h, "Pick hero image").is_none());
    assert!(h.app.subtasks(id).is_empty());
}

#[test]
fn list_due_date_takes_a_time_removes_it_and_clearing_the_date_drops_it() {
    let mut h = Ui1::new();
    let name = "Write copy for landing page";
    let cell = format!("Due date of {}", name);
    // Add time: 9:00 on the date (today when there was none)
    h.click(&cell);
    h.click("Add time");
    h.app.apply_actions();
    let t = task_named(&h, name);
    assert!(t.due.is_some());
    assert_eq!(t.due_at(), chrono::NaiveTime::from_hms_opt(9, 0, 0));
    // type another time and press Enter
    h.click("Time of day");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("2:30 pm");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    let t = task_named(&h, name).clone();
    assert_eq!(t.due_at(), chrono::NaiveTime::from_hms_opt(14, 30, 0));
    // shown next to the date in the row, and in the history
    h.key(egui::Key::Escape);
    let shown = fmt_due(&t).unwrap();
    assert!(shown.ends_with("2:30 PM"), "{}", shown);
    assert!(
        !h.painted_text(&shown).is_empty(),
        "the row shows {:?}",
        shown
    );
    let line = &t.activity.last().unwrap().text;
    assert!(
        line.starts_with("changed due date to ") && line.ends_with(" at 2:30 PM"),
        "{}",
        line
    );
    // Remove time keeps the date
    h.click(&cell);
    h.click("Remove time");
    h.app.apply_actions();
    let t = task_named(&h, name);
    assert!(t.due.is_some());
    assert_eq!(t.due_at(), None);
    assert_eq!(t.due_time, Some(None), "a removed time is written as null");
    // with a time again, clearing the date drops the time too
    h.click("Add time");
    h.app.apply_actions();
    assert!(task_named(&h, name).due_at().is_some());
    h.click("Clear");
    h.app.apply_actions();
    let t = task_named(&h, name);
    assert_eq!((t.due, t.due_time), (None, Some(None)));
    // a new date afterwards starts without a time
    h.click(&cell);
    h.click("Tomorrow");
    h.app.apply_actions();
    let t = task_named(&h, name);
    assert_eq!(t.due, Some(today() + Duration::days(1)));
    assert_eq!(t.due_at(), None);
}

#[test]
fn subtask_due_date_takes_a_time() {
    let mut h = Ui1::new();
    open_design_task(&mut h);
    h.click("Subtask due Pick hero image");
    h.click("Add time");
    h.app.apply_actions();
    let t = task_named(&h, "Pick hero image");
    assert_eq!(t.due, Some(today()), "a time on no date sets today");
    assert_eq!(t.due_at(), chrono::NaiveTime::from_hms_opt(9, 0, 0));
    h.click("Remove time");
    h.app.apply_actions();
    assert_eq!(task_named(&h, "Pick hero image").due_at(), None);
}

#[test]
fn task_window_picker_sets_parent_and_adds_existing_subtask() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("Task actions");
    h.click("Make subtask of...");
    assert!(matches!(
        h.app.modal,
        Some(Modal::PickTask {
            mode: TaskPick::Parent,
            ..
        })
    ));
    assert!(
        !h.has("Pick Wireframe hero section"),
        "its own subtasks can't be its parent"
    );
    h.click("Search tasks to pick");
    h.type_text("q1");
    h.click("Pick Plan Q1 campaign");
    let q1 = task_named(&h, "Plan Q1 campaign").clone();
    let t = h.app.task(id).unwrap();
    assert_eq!(t.parent, Some(q1.id));
    assert_eq!(
        t.list_id, q1.list_id,
        "a subtask moves into its parent's List"
    );
    assert!(h.app.modal.is_none());
    // the reverse: put an existing task under this one
    h.click("+  Add existing task");
    h.click("Search tasks to pick");
    h.type_text("passport");
    h.click("Pick Renew passport");
    let p = task_named(&h, "Renew passport");
    assert_eq!(p.parent, Some(id));
}

#[test]
fn relationships_waiting_on_blocking_and_linked() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("+  Add relationship");
    h.click("Waiting on");
    h.click("Search tasks to pick");
    h.type_text("analytics");
    h.click("Pick Set up analytics dashboard");
    let other = h.task_id("Set up analytics dashboard");
    assert_eq!(h.app.task(id).unwrap().waiting_on, vec![other]);
    assert!(h.has("Waiting on Set up analytics dashboard"));
    // the other task shows it as Blocking
    assert_eq!(h.app.relations(other).1, vec![id]);
    h.click("+  Add relationship");
    h.click("Link a task");
    h.click("Search tasks to pick");
    h.type_text("newsletter");
    h.click("Pick Newsletter draft");
    assert!(h.has("Link a task Newsletter draft"));
    h.click("Remove link Set up analytics dashboard");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().waiting_on.is_empty());
    // a row opens the other task
    h.click("Link a task Newsletter draft");
    h.app.apply_actions();
    assert_ne!(h.app.open_task, Some(id));
}

#[test]
fn description_keeps_markdown_and_ticks_checkboxes() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .description = "- [ ] step".into();
    h.settle();
    // the "- [ ] " marker is hidden and a box is painted instead
    let r = h.rect("Add description");
    h.click_at(pos2(r.left() + 10.0, r.top() + 10.0));
    h.app.apply_actions();
    assert_eq!(h.app.task(id).unwrap().description, "- [x] step");
}

#[test]
fn folder_row_menu_duplicate_archive_and_move() {
    let mut h = Ui1::new();
    h.click("Folder options Projects");
    h.click("Duplicate");
    assert_eq!(h.app.data.spaces[0].folders.len(), 2);
    let copy = h.app.data.spaces[0].folders[1].id;
    assert_eq!(
        h.app.data.spaces[0].folders[1].lists.len(),
        2,
        "the lists are copied"
    );
    h.key(egui::Key::Escape);
    h.click("Folder options Projects (copy)");
    h.click("Archive");
    assert!(h.app.data.spaces[0].folders[1].archived);
    assert!(
        !h.has("Projects (copy)"),
        "archived folders leave the sidebar"
    );
    let lid = h.app.data.spaces[0].folders[1].lists[0].id;
    assert!(h.app.data.list_archived(lid));
    h.app.set_folder_archived(copy, false);
    let side = h.app.data.spaces[1].id;
    h.app.move_folder(copy, side);
    assert!(h.app.data.spaces[1].folders.iter().any(|f| f.id == copy));
    assert_eq!(h.app.data.spaces[0].folders.len(), 1);
}

#[test]
fn list_color_from_the_list_menu() {
    let mut h = Ui1::new();
    h.click("List options Personal");
    h.click("Color");
    let c = STATUS_PALETTE[2];
    h.click(&format!("List color {:02x}{:02x}{:02x}", c[0], c[1], c[2]));
    assert_eq!(h.app.data.spaces[0].lists[0].color, Some(c));
}

#[test]
fn trash_empty_asks_then_clears() {
    let mut h = Ui1::new();
    let lid = h.app.data.spaces[1].lists[0].id;
    h.app.do_delete(Target::List(lid));
    h.app.modal = Some(Modal::Trash);
    h.settle();
    h.click("Empty trash");
    assert!(!h.app.data.extra.trash.is_empty());
    h.click("Yes");
    assert!(h.app.data.extra.trash.is_empty());
}

#[test]
fn ownerless_old_timer_can_be_stopped_from_the_task() {
    let mut h = Ui1::new();
    let id = h.task_id("Write copy for landing page");
    let started = now_ts() - 600;
    let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
    t.timer_started = Some(started);
    t.timer_by = None;
    h.app.apply(Action::Open(id));
    h.settle();
    // it isn't mine, so my play button starts my own timer beside it
    h.click("Start timer");
    let t = task_named(&h, "Write copy for landing page");
    assert_eq!(t.timer_started, Some(started));
    assert!(h.app.data.is_my_timer(t));
    h.click("Stop (started by an older app)");
    let t = task_named(&h, "Write copy for landing page");
    assert!(
        t.time_entries
            .iter()
            .any(|e| e.start == started && e.member == 0),
        "its time is kept"
    );
    assert!(h.app.data.is_my_timer(t), "my timer keeps running");
    assert!(!h.has("Stop (started by an older app)"));
}

#[test]
fn my_subtask_shows_once_on_my_tasks() {
    let mut h = Ui1::new();
    let list = h.app.data.first_list().unwrap();
    let me = h.app.data.me;
    h.app.apply(Action::Add {
        list,
        name: "Parent of mine".into(),
        status: None,
        priority: 0,
        assignees: vec![me],
        due: None,
        parent: None,
        open: false,
    });
    let parent = h.task_id("Parent of mine");
    h.app.apply(Action::Add {
        list,
        name: "Sub of mine".into(),
        status: None,
        priority: 0,
        assignees: vec![me],
        due: None,
        parent: Some(parent),
        open: false,
    });
    h.app.loc = Location::MyTasks;
    h.app.subtasks_open = true;
    h.settle();
    h.scroll_into_view("Task Sub of mine");
    assert_eq!(
        h.tree
            .iter()
            .filter(|(n, _)| n == "Task Sub of mine")
            .count(),
        1
    );
}

#[test]
fn status_editor_retargets_automations_only_when_a_rename_is_done() {
    let mut h = Ui1::new();
    let sid = h.app.data.spaces[0].id;
    h.app.data.spaces[0]
        .statuses
        .insert(1, Status::new("triage", [1, 2, 3], StatusKind::Custom));
    h.app.data.spaces[1]
        .statuses
        .insert(1, Status::new("review", [1, 2, 3], StatusKind::Custom));
    let list = h.app.data.lists_in(Location::Space(sid)).unwrap()[0];
    let tid = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.list_id == list)
        .unwrap()
        .id;
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == tid)
        .unwrap()
        .status = "triage".into();
    let rule = |id, s: &str| Automation {
        id,
        enabled: true,
        trigger_status: s.into(),
        action: "priority".into(),
        value: "1".into(),
        list: None,
        ..Default::default()
    };
    h.app.data.extra.automations = vec![rule(1, "review"), rule(2, "triage")];
    h.app.edit_statuses = Some(sid);
    h.settle();
    let status = |h: &Ui1| h.app.task(tid).unwrap().status.clone();
    let triggers = |h: &Ui1| {
        h.app
            .data
            .extra
            .automations
            .iter()
            .map(|a| a.trigger_status.clone())
            .collect::<Vec<_>>()
    };
    // type a name through the other space's "review": tasks follow live
    h.click("triage");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("review");
    assert_eq!(status(&h), "review");
    assert_eq!(
        triggers(&h),
        ["review", "triage"],
        "rules wait for the rename"
    );
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("reviewed");
    assert_eq!(status(&h), "reviewed");
    h.key(egui::Key::Enter);
    assert_eq!(
        triggers(&h),
        ["review", "reviewed"],
        "the other space's rule stays; this status's rule follows to the final name"
    );
}

#[test]
fn closing_the_status_editor_puts_back_an_empty_or_taken_name() {
    let mut h = Ui1::new();
    let sid = h.app.data.spaces[0].id;
    let list = h.app.data.lists_in(Location::Space(sid)).unwrap()[0];
    let tid = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.list_id == list)
        .unwrap()
        .id;
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == tid)
        .unwrap()
        .status = "in progress".into();
    h.app.edit_statuses = Some(sid);
    h.settle();
    // "in progress" typed into "to do": taken, so its tasks wait
    h.click("in progress");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("to do");
    assert_eq!(h.app.task(tid).unwrap().status, "in progress");
    h.click("Close window");
    assert!(h.app.edit_statuses.is_none());
    let names: Vec<String> = h.app.data.spaces[0]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert!(names.contains(&"in progress".to_string()), "{:?}", names);
    let st = h.app.task(tid).unwrap().status.clone();
    assert!(names.contains(&st), "the task's status exists: {}", st);
    assert!(h.app.toast.is_some());
    // an empty name goes back too
    h.app.edit_statuses = Some(sid);
    h.settle();
    h.click("in progress");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.click("Close window");
    assert!(h.app.data.spaces[0]
        .statuses
        .iter()
        .any(|s| s.name == "in progress"));
    assert_eq!(h.app.task(tid).unwrap().status, "in progress");
}

#[test]
fn add_task_without_a_list_says_why_and_offers_one() {
    // the header "Add Task" on My Tasks: no Space yet, so New Space opens
    let mut h = no_list_workspace(false);
    h.click("My Tasks");
    h.click("Add Task");
    assert!(h.app.quick_add.is_none());
    let toast = h.app.toast.clone().expect("a toast says why").0;
    assert!(toast.contains("Make a Space first"), "{}", toast);
    assert!(matches!(h.app.modal, Some(Modal::NewSpace { .. })));
    // with a Space, the New List dialog opens instead (Everything, + Create > Task)
    let mut h = no_list_workspace(true);
    h.click("All Tasks");
    h.click("Create");
    h.click("Task");
    assert!(h.app.quick_add.is_none());
    assert!(h.app.toast.clone().unwrap().0.contains("Make one first"));
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })));
}

#[test]
fn inline_add_without_a_list_keeps_the_name() {
    let mut h = no_list_workspace(true);
    h.click("Today & Overdue");
    let row = h
        .tree
        .iter()
        .map(|(n, _)| n.clone())
        .find(|n| n.starts_with("Add Task in "))
        .expect("an inline add row");
    h.click(&row);
    h.type_text("Call the printer");
    h.key(egui::Key::Enter);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .all(|t| t.name != "Call the printer"));
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })));
    assert_eq!(
        h.app.adding.as_ref().map(|a| a.1.as_str()),
        Some("Call the printer")
    );
}

#[test]
fn planner_add_without_a_list_keeps_the_name_and_says_why() {
    let mut h = no_list_workspace(false);
    h.app.section = Section::Planner;
    h.settle();
    let day = h.app.planner_week + Duration::days(2);
    h.click(&format!("Add task on {}", day.format("%a %-d")));
    h.type_text("Plan the trip");
    h.key(egui::Key::Enter);
    assert!(h.app.data.tasks.iter().all(|t| t.name != "Plan the trip"));
    assert_eq!(
        h.app.sweep.planner_add,
        Some((day, "Plan the trip".to_string()))
    );
    assert!(h.app.toast.is_some());
    assert!(matches!(h.app.modal, Some(Modal::NewSpace { .. })));
}

#[test]
fn create_list_or_channel_list_without_a_space_says_so() {
    let mut h = no_list_workspace(false);
    h.click("Create");
    // the Create menu's List, not a sidebar List row of that name
    let item = h.rect_in(
        "List",
        Rect::from_min_max(pos2(200.0, 0.0), pos2(f32::INFINITY, f32::INFINITY)),
    );
    h.click_at(item.center());
    assert!(
        matches!(h.app.modal, Some(Modal::NewSpace { .. })),
        "no Space to pick: New Space instead of a List dialog with nothing to pick"
    );
    assert!(h
        .app
        .toast
        .clone()
        .unwrap()
        .0
        .contains("Make a Space first"));
    // the channel's "Add a List" switch is off and says why
    h.app.modal = Some(Modal::NewChannel {
        name: String::new(),
        private: false,
        add_list: true,
    });
    h.settle();
    assert!(h.has("Needs a Space to put the List in. Make one from + Create > Space."));
    h.click("Add a List");
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewChannel {
            add_list: false,
            ..
        })
    ));
}

#[test]
fn task_picker_with_nothing_typed_does_not_say_no_match() {
    let mut h = archived_workspace();
    let tid = h.app.data.tasks[0].id;
    h.app.data.tasks[0].archived = false;
    h.app.modal = Some(Modal::PickTask {
        task: tid,
        mode: TaskPick::Linked,
        query: String::new(),
    });
    h.settle();
    assert!(!h.has("No tasks match"));
    assert!(h.has("No other tasks to pick. Archived tasks are not offered here."));
}

#[test]
fn switching_the_status_editor_to_another_space_or_quitting_finishes_the_edits() {
    let mut h = Ui1::new();
    let sid = h.app.data.spaces[0].id;
    let other = h.app.data.spaces[1].id;
    let list = h.app.data.lists_in(Location::Space(sid)).unwrap()[0];
    let other_list = h.app.data.lists_in(Location::Space(other)).unwrap()[0];
    let in_list = |h: &Ui1, l: u64| {
        h.app
            .data
            .tasks
            .iter()
            .filter(|t| t.list_id == l)
            .map(|t| t.id)
            .collect::<Vec<_>>()
    };
    let (a, b) = (in_list(&h, list)[0], in_list(&h, list)[1]);
    let c = in_list(&h, other_list)[0];
    for (t, st) in [(a, "in progress"), (b, "to do"), (c, "in progress")] {
        h.app
            .data
            .tasks
            .iter_mut()
            .find(|x| x.id == t)
            .unwrap()
            .status = st.into();
    }
    h.app.data.extra.automations = vec![Automation {
        id: 1,
        enabled: true,
        trigger_status: "to do".into(),
        action: "priority".into(),
        value: "1".into(),
        list: Some(list),
        ..Default::default()
    }];
    let status_ok = |h: &Ui1, si: usize, t: u64| {
        let st = h.app.task(t).unwrap().status.clone();
        h.app.data.spaces[si].statuses.iter().any(|s| s.name == st)
    };
    // in the first space: "to do" renamed, "in progress" emptied
    h.app.apply(Action::EditStatuses(sid));
    h.settle();
    h.click("to do");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("backlog");
    h.click("in progress");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.settle();
    assert!(!status_ok(&h, 0, a), "mid-edit the name is empty");
    // the other space's "Edit statuses" while the editor is open
    h.app.toast = None;
    h.app.apply(Action::EditStatuses(other));
    h.settle();
    assert_eq!(h.app.edit_statuses, Some(other));
    assert!(status_ok(&h, 0, a), "the emptied name went back");
    assert!(status_ok(&h, 0, b));
    assert_eq!(h.app.task(b).unwrap().status, "backlog");
    assert_eq!(
        h.app.data.extra.automations[0].trigger_status, "backlog",
        "the rename reached the automation"
    );
    assert!(h.app.toast.is_some());
    // quitting with a name taken in the other space's editor
    h.click("in progress");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("complete");
    h.settle();
    assert!(
        !status_ok(&h, 1, c),
        "mid-edit the task waits on a name no status has"
    );
    h.app.finish_before_quit();
    assert!(h.app.edit_statuses.is_none());
    let names: Vec<String> = h.app.data.spaces[1]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert!(
        names.contains(&"in progress".to_string()),
        "the taken name went back: {:?}",
        names
    );
    assert!(status_ok(&h, 1, c));
    assert_eq!(
        names.iter().filter(|n| *n == "complete").count(),
        1,
        "no two statuses share a name"
    );
}

#[test]
fn switching_workspace_finishes_the_status_edits_first() {
    let mut h = Ui1::new();
    let (_, a, b) = status_edit_half_done(&mut h);
    // a workspace whose file can't be read: the switch goes no further, so
    // what the switch did first shows in this workspace
    let file = format!("workspace-r5-gone-{:x}.json", crate::sync::random_u64());
    h.app.switch_workspace(&file);
    assert!(h.app.edit_statuses.is_none(), "the editor closed");
    assert!(h.app.status_edit.is_none());
    assert_eq!(
        h.app.task(a).unwrap().status,
        "backlog",
        "the rename reached its tasks"
    );
    let names: Vec<String> = h.app.data.spaces[0]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert!(
        names.contains(&"in progress".to_string()),
        "the emptied name went back: {:?}",
        names
    );
    assert_eq!(h.app.task(b).unwrap().status, "in progress");
}

#[test]
fn joining_a_team_finishes_the_status_edits_and_leaves_the_teams_statuses_alone() {
    let mut h = Ui1::new();
    // the team's copy: the same spaces, with "to do" renamed to "queued"
    let mut team = App::build(None, Some(h.app.data.clone()));
    let st = team.data.spaces[0]
        .statuses
        .iter_mut()
        .find(|s| s.name == "to do")
        .unwrap();
    st.name = "queued".into();
    let folder = temp_home("r5-join");
    team.share_here(folder.to_str().unwrap()).unwrap();
    let team_names: Vec<String> = team.data.spaces[0]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let (sid, _, _) = status_edit_half_done(&mut h);
    h.app.join_shared(folder.to_str().unwrap()).unwrap();
    assert!(h.app.edit_statuses.is_none(), "the editor closed");
    assert!(h.app.status_edit.is_none(), "and its bookkeeping is gone");
    h.settle();
    // closing anything status-related later puts no old names on the team's
    h.app.finish_status_edit();
    let sp = h.app.data.spaces.iter().find(|s| s.id == sid).unwrap();
    let names: Vec<String> = sp.statuses.iter().map(|s| s.name.clone()).collect();
    assert_eq!(names, team_names, "the team's statuses as they were");
    let _ = std::fs::remove_dir_all(&folder);
}

/// Deleting selected tasks asks with Delete first, then Cancel, as every
/// other confirm dialog does.
#[test]
fn delete_tasks_dialog_puts_delete_before_cancel() {
    let mut h = Ui1::new();
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Select Write copy for landing page");
    h.key(egui::Key::Backspace);
    assert!(h.app.confirm_bulk_delete);
    let del = h.rect("Confirm delete");
    let cancel = h.rect("Cancel");
    assert!(del.right() <= cancel.left(), "{:?} then {:?}", del, cancel);
}
