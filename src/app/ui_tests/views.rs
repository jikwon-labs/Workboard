// Click-through tests for the views of a List and the apps that show tasks:
// List columns and filters, Board, Calendar, Gantt, Timeline, Table,
// Planner, forms, dashboards and timesheets.

use super::*;

fn list_of_other(h: &Ui1, not: u64) -> u64 {
    h.app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .map(|l| l.id)
                .chain(s.folders.iter().flat_map(|f| f.lists.iter().map(|l| l.id)))
        })
        .find(|l| *l != not)
        .unwrap()
}

/// A shared workspace with one 30-minute entry of mine at 10:00 today on
/// "Renew passport", the timesheet's entry list open. Returns the task's
/// index and the entry's start.
fn shared_timesheet(h: &mut Ui1) -> (usize, i64) {
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    let td = today();
    let ws = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
    let start = td
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .single()
        .unwrap()
        .timestamp();
    let i = h
        .app
        .data
        .tasks
        .iter()
        .position(|t| t.name == "Renew passport")
        .unwrap();
    let me = h.app.data.me;
    h.app.data.tasks[i].time_entries = vec![TimeEntry {
        start,
        end: start + 1800,
        member: me,
    }];
    h.app.section = Section::Timesheets;
    h.app.sheet_week = ws;
    h.app.sheet_entries = true;
    h.app.dirty = true;
    h.settle();
    h.app.sync_note_change();
    (i, start)
}

#[test]
fn click_view_tabs() {
    let mut h = Ui1::new();
    for (name, v) in [
        ("View Board", ViewKind::Board),
        ("View Calendar", ViewKind::Calendar),
        ("View Gantt", ViewKind::Gantt),
        ("View Timeline", ViewKind::Timeline),
        ("View Table", ViewKind::Table),
        ("View List", ViewKind::List),
    ] {
        h.click(name);
        assert!(h.app.view == v, "{} selects the view", name);
    }
}

#[test]
fn click_form_template_and_submit() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Forms");
    h.click("IT Requests");
    let fid = h.app.form.open.expect("form opens");
    assert_eq!(
        h.app
            .data
            .extra
            .forms
            .iter()
            .find(|f| f.id == fid)
            .unwrap()
            .fields
            .len(),
        3
    );
    h.click("Fill out");
    let n = h.app.data.tasks.len();
    h.click("Submit");
    assert_eq!(h.app.data.tasks.len(), n + 1, "submitting creates a task");
}

#[test]
fn drag_board_card_to_another_column() {
    let mut h = Ui1::new();
    h.click("View Board");
    h.drag(
        "Card Write copy for landing page",
        "Card Fix navigation on mobile",
    );
    let t = task_named(&h, "Write copy for landing page");
    assert_eq!(
        t.status, "in progress",
        "dropping a card in another column moves it"
    );
}

/// A long List draws only the rows in view: a row far down is still there
/// by name (and for the arrow keys), but its controls are drawn only once
/// it's scrolled to, and then they work. The Board does the same with the
/// cards in a long column.
#[test]
fn a_long_list_draws_the_rows_in_view_and_the_rest_on_scrolling() {
    let mut h = Ui1::new();
    let list = h.app.data.tasks[0].list_id;
    for i in 0..300 {
        h.app.apply(Action::Add {
            list,
            name: format!("Bulk task {:03}", i),
            status: None,
            priority: 0,
            assignees: vec![],
            due: None,
            parent: None,
            open: false,
        });
    }
    h.app.actions.push(Action::Go(Location::List(list)));
    h.settle();
    let far = "Bulk task 299";
    assert!(
        h.has(&format!("Task {}", far)),
        "a row out of view keeps its name"
    );
    assert!(
        !h.has(&format!("Status of {}", far)),
        "but its controls aren't drawn"
    );
    assert!(
        h.app.row.order.len() > 300,
        "and it keeps its place for the arrow keys"
    );
    // down to it with the mouse wheel
    h.frame(vec![egui::Event::PointerMoved(pos2(700.0, 500.0))]);
    for _ in 0..4 {
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -20_000.0),
            modifiers: Default::default(),
        }]);
    }
    h.settle();
    h.scroll_into_view(&format!("Task {}", far));
    assert!(h.has(&format!("Status of {}", far)), "drawn once in view");
    assert!(
        !h.has("Status of Bulk task 000"),
        "and the top rows aren't any more"
    );
    h.click(&format!("Status of {}", far));
    h.click("IN PROGRESS");
    let t = h.app.data.tasks.iter().find(|t| t.name == far).unwrap();
    assert_eq!(t.status, "in progress");

    h.click("View Board");
    h.settle();
    let card = "Card Bulk task 000";
    assert!(h.has(card));
    h.click(card);
    assert!(h.app.open_task.is_some(), "a card far down opens");
}

#[test]
fn click_goals_dashboards_teams_create() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Goals");
    let g = h.app.data.extra.goals.len();
    h.click("New goal");
    h.type_text("Grow");
    h.click("Create goal");
    assert_eq!(h.app.data.extra.goals.len(), g + 1);

    h.click("Rail More");
    h.click("Dashboards");
    let d = h.app.data.extra.dashboards.len();
    h.click("Simple Dashboard");
    assert_eq!(h.app.data.extra.dashboards.len(), d + 1);

    h.click("Rail Teams");
    assert_eq!(h.app.section, Section::Teams);
    let n = h.app.data.extra.teams.len();
    h.click("Create team");
    assert_eq!(h.app.data.extra.teams.len(), n + 1);
}

#[test]
fn fields_panel_searches_and_adds_property_columns() {
    let mut h = Ui1::new();
    assert!(!h.app.col_on("Created by"), "properties start hidden");
    h.click("Columns");
    assert!(h.app.fields_open);
    assert!(h.has("Shown") && h.has("Properties"));
    h.click("Search fields");
    h.type_text("created");
    assert!(h.has("Field Created by") && h.has("Field Date created"));
    assert!(!h.has("Field Tags"), "search narrows the list");
    h.click("Field Created by");
    assert!(h.app.col_on("Created by"));
    h.key(egui::Key::Escape);
    assert!(!h.app.fields_open, "Escape closes the panel");
    // the column is laid out, with a value ("Me" made the seed tasks)
    let cols = crate::app::shell::col_layout(
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 30.0)),
        &h.app.prefs.hidden_cols,
        true,
    );
    assert_eq!(
        cols.extra.iter().map(|c| c.0).collect::<Vec<_>>(),
        vec!["Created by"]
    );
    let t = task_named(&h, "Write copy for landing page");
    assert_eq!(
        h.app.prop_text(t, "Created by"),
        h.app.data.member(t.activity[0].author).unwrap().name
    );
}

#[test]
fn filter_builder_rows_are_joined_with_and() {
    let mut h = Ui1::new();
    h.app.loc = Location::Everything;
    h.settle();
    h.click("Filter");
    assert!(h.app.filter_open);
    h.click("+ Add filter");
    h.click("Filter 1 field");
    h.click("Priority");
    h.click("Filter 1 values");
    h.click("Urgent");
    h.key(egui::Key::Escape); // closes the drop-down, not the panel
    assert!(
        h.app.filter_open,
        "the panel stays while a drop-down closes"
    );
    assert_eq!(h.app.prefs.filter.rows.len(), 1);
    assert!(h.app.filtered().iter().all(|t| t.priority == 1));
    let urgent = h.app.filtered().len();
    // a second row: Status is not "to do"
    h.click("+ Add filter");
    h.click("Filter 2 field");
    h.click("Status");
    h.click("Filter 2 condition");
    h.click("is not");
    h.click("Filter 2 values");
    h.click("TO DO");
    h.key(egui::Key::Escape);
    assert_eq!(h.app.prefs.filter.count(), 2);
    assert!(h
        .app
        .filtered()
        .iter()
        .all(|t| t.priority == 1 && t.status != "to do"));
    assert!(h.app.filtered().len() <= urgent);
    h.click("Remove filter 2");
    assert_eq!(h.app.prefs.filter.rows.len(), 1);
    h.click("Clear all");
    assert_eq!(h.app.prefs.filter.count(), 0);
    h.key(egui::Key::Escape);
    assert!(!h.app.filter_open);
}

#[test]
fn filter_groups_with_their_own_or_and_the_badge_counts_them() {
    use parity::Join;
    let mut h = Ui1::new();
    h.app.loc = Location::Everything;
    h.settle();
    h.click("Filter");
    // Priority is Urgent
    h.click("+ Add filter");
    h.click("Filter 1 field");
    h.click("Priority");
    h.click("Filter 1 values");
    h.click("Urgent");
    h.key(egui::Key::Escape);
    // or a group: Status is TO DO or Priority is High
    h.click("+ Add group");
    assert_eq!(h.app.prefs.filter.groups.len(), 1);
    h.click("Group 1 filter 1 field");
    h.click("Status");
    h.click("Group 1 filter 1 values");
    h.click("TO DO");
    h.key(egui::Key::Escape);
    h.click("Add filter to group 1");
    h.click("Group 1 filter 2 field");
    h.click("Priority");
    h.click("Group 1 filter 2 values");
    h.click("High");
    h.key(egui::Key::Escape);
    assert!(h.app.filter_open, "the panel stays open");
    assert_eq!(h.app.prefs.filter.groups[0].rows.len(), 2);
    // And inside the group: TO DO and High
    let and_inside = h.app.filtered().len();
    h.click("Group 1 join");
    assert_eq!(h.app.prefs.filter.groups[0].join, Join::Or);
    h.click("Filter join");
    assert_eq!(h.app.prefs.filter.join, Join::Or);
    let shown: Vec<(u8, String)> = h
        .app
        .filtered()
        .iter()
        .map(|t| (t.priority, t.status.clone()))
        .collect();
    assert!(shown.len() > and_inside, "Or lets more through");
    assert!(shown
        .iter()
        .all(|(p, s)| *p == 1 || s == "to do" || *p == 2));
    assert!(shown.iter().any(|(p, _)| *p != 1), "not only Urgent");
    // the badge counts every condition, the ones in the group too
    assert_eq!(h.app.prefs.filter.count(), 3);
    assert!(h.has("Filter \u{00B7} 3"));
    // removing a row in the group, then the group
    h.click("Remove group 1 filter 2");
    assert_eq!(h.app.prefs.filter.count(), 2);
    h.click("Remove group 1");
    assert!(h.app.prefs.filter.groups.is_empty());
    assert_eq!(h.app.prefs.filter.count(), 1);
    assert!(h.has("Filter \u{00B7} 1"));
}

#[test]
fn save_a_filter_then_bring_it_back_rename_and_delete_it() {
    use parity::{FilterField, FilterGroup, FilterRow, Join};
    let mut h = Ui1::new();
    h.app.loc = Location::Everything;
    h.settle();
    h.click("Filter");
    // nothing to save yet
    h.click("Saved filter name");
    h.type_text("Urgent or to do");
    h.click("Save filter");
    assert!(h.app.data.extra.saved_filters.is_empty(), "no conditions");
    // Urgent, or (TO DO or High)
    let row = |field, v: &str| FilterRow {
        field: Some(field),
        not: false,
        values: vec![v.to_string()],
    };
    h.app.prefs.filter.rows = vec![row(FilterField::Priority, "1")];
    h.app.prefs.filter.join = Join::Or;
    h.app.prefs.filter.groups = vec![FilterGroup {
        join: Join::Or,
        rows: vec![
            row(FilterField::Status, "to do"),
            row(FilterField::Priority, "2"),
        ],
    }];
    h.settle();
    let shown = h.app.filtered().len();
    h.click("Save filter");
    assert_eq!(h.app.data.extra.saved_filters.len(), 1);
    let saved = h.app.data.extra.saved_filters[0].clone();
    assert_eq!(saved.name, "Urgent or to do");
    assert_eq!(saved.groups[0].rows.len(), 2);
    assert_eq!((saved.join, saved.groups[0].join), (Join::Or, Join::Or));
    assert!(h.has("Urgent or to do"));
    // clear, then bring it back by its name
    h.click("Clear all");
    assert_eq!(h.app.prefs.filter.count(), 0);
    h.click("Urgent or to do");
    assert_eq!(h.app.prefs.filter.count(), 3);
    assert_eq!(h.app.prefs.filter.groups, saved.groups);
    assert_eq!(h.app.filtered().len(), shown);
    assert!(h.has("Filter \u{00B7} 3"));
    // rename
    h.click("Rename saved filter Urgent or to do");
    h.type_text(" soon");
    h.click("Done renaming");
    assert_eq!(
        h.app.data.extra.saved_filters[0].name,
        "Urgent or to do soon"
    );
    assert!(h.has("Urgent or to do soon"));
    // changing the builder leaves the saved filter as it was
    h.click("Remove group 1");
    assert_eq!(h.app.prefs.filter.count(), 1);
    assert_eq!(h.app.data.extra.saved_filters[0].groups.len(), 1);
    // delete
    h.click("Delete saved filter Urgent or to do soon");
    assert!(h.app.data.extra.saved_filters.is_empty());
    assert!(!h.has("Urgent or to do soon"));
}

#[test]
fn space_overview_customize_and_lists_card_columns() {
    let mut h = Ui1::new();
    let sid = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.private_owner.is_none())
        .unwrap()
        .id;
    h.app.actions.push(Action::Go(Location::Space(sid)));
    h.app.apply_actions();
    h.app.view = ViewKind::Overview;
    h.settle();
    let lid = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.id == sid)
        .unwrap()
        .lists[0]
        .id;
    let lname = h.app.data.list_name(lid);
    assert!(h.has(&format!("Overview list {}", lname)));
    // Priority and Owner cells edit the List
    h.click(&format!("Priority of {}", lname));
    h.click("Urgent");
    assert_eq!(h.app.data.list_item(lid).unwrap().priority, 1);
    h.click(&format!("Owner of {}", lname));
    h.click_side("Sam Lee", true);
    assert_eq!(h.app.data.list_item(lid).unwrap().owner, Some(3));
    assert!(
        matches!(h.app.loc, Location::Space(_)),
        "editing a cell does not open the List"
    );
    // Customize turns the Docs card off; Add card brings it back
    h.click("Customize");
    h.click("Card Docs");
    assert!(h.app.data.extra.hidden_cards.contains(&"Docs".to_string()));
    h.key(egui::Key::Escape);
    assert!(!h.has("Add a Doc"));
    h.click("+ Add card");
    h.click("Add card Docs");
    assert!(h.app.data.extra.hidden_cards.is_empty());
    // New List opens the Create List window
    h.click("+ New List");
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })));
}

#[test]
fn personal_list_looks_like_the_original_and_my_tasks_stays_open() {
    let mut h = Ui1::new();
    // the My Tasks items show without clicking My Tasks; My Tasks itself
    // is "Assigned to me", so that has no row of its own
    assert!(h.has("Today & Overdue") && h.has("Personal List"));
    assert!(!h.has("Assigned to me"));
    assert!(h.app.prefs.group_by == GroupBy::Status);
    h.click("Personal List");
    let pl = h.app.personal_list_id().unwrap();
    assert_eq!(h.app.loc, Location::List(pl));
    assert!(h.app.prefs.group_by == GroupBy::None, "Group: None");
    assert!(h.app.col_on("Date created"));
    assert!(h.has("View Board") && h.has("View List"));
    assert!(
        !h.has("View Calendar") && !h.has("View Gantt") && !h.has("View Table"),
        "only Board and List"
    );
    assert!(
        !h.has("Add Channel")
            || h.tree
                .iter()
                .filter(|(n, _)| n == "Add Channel")
                .all(|(_, r)| r.left() < 320.0),
        "no Add Channel button on the page"
    );
    // empty: an inline Add Task row
    h.click("Add Task in ALL TASKS");
    h.type_text("Buy milk");
    h.key(egui::Key::Enter);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.list_id == pl && t.name == "Buy milk"));
    // leaving brings the earlier settings back
    h.click("My Tasks");
    assert_eq!(h.app.loc, Location::MyTasks);
    assert!(h.app.prefs.group_by == GroupBy::Status);
    assert!(!h.app.col_on("Date created"));
    assert!(h.has("Personal List"), "still open after moving away");
}

#[test]
fn property_columns_never_squeeze_the_name_away() {
    let mut hidden = std::collections::BTreeSet::new();
    hidden.insert("Date created".to_string());
    let row = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 30.0));
    let cols = crate::app::shell::col_layout(row, &hidden, true);
    assert!(
        cols.extra.len() < parity::PROPERTIES.len(),
        "not every column fits in 1100px"
    );
    assert!(
        cols.left >= 300.0,
        "the name keeps at least 300px ({})",
        cols.left
    );
}

#[test]
fn narrow_window_list_header_toolbar_and_columns_do_not_overlap() {
    let mut h = Ui1::sized(960.0, 560.0);
    let page = Rect::from_min_max(pos2(320.0, 0.0), pos2(960.0, 560.0));
    // columns: the name keeps its room, the property cells start after it
    let row = h.rect("Task Write copy for landing page");
    let due = h.rect("Due date of Write copy for landing page");
    let cols = crate::app::shell::col_layout(row, &h.app.prefs.hidden_cols, true);
    assert!(
        cols.left - row.left() >= 220.0,
        "name column {} px wide",
        cols.left - row.left()
    );
    assert!(
        due.left() >= cols.left && cols.assignee.left() >= cols.left,
        "cells sit right of the name: due {:?}, left {}",
        due,
        cols.left
    );
    for r in [
        cols.assignee,
        cols.due,
        cols.priority,
        cols.status,
        cols.comments,
        cols.created,
    ] {
        assert!(
            r.width() == 0.0 || r.left() >= row.left() + 220.0,
            "a shown column {:?} covers the name",
            r
        );
    }
    // header: the breadcrumb (and its star) end before the buttons on the right
    let star = h.rect_in("\u{2605}", page).union(
        h.tree
            .iter()
            .find(|(n, _)| n == "\u{2606}")
            .map(|(_, r)| *r)
            .unwrap_or(Rect::NOTHING),
    );
    let crumb = h.rect_in("Website Redesign", page);
    let huddle = h.rect("Huddle");
    assert!(
        crumb.right() <= huddle.left() && star.right() <= huddle.left(),
        "crumb {:?} star {:?} huddle {:?}",
        crumb,
        star,
        huddle
    );
    // toolbar: left chips end before the right ones start, with search closed and open
    for open in [false, true] {
        if open {
            h.click("Search tasks");
        }
        let cols_chip = h.rect("Columns");
        let sort = h.rect("Sort: Manual");
        assert!(
            cols_chip.right() <= sort.left(),
            "search open {}: Columns {:?} Sort {:?}",
            open,
            cols_chip,
            sort
        );
        if open {
            let field = h.rect("Search tasks...");
            assert!(
                field.left() >= cols_chip.right() && field.right() <= h.rect("Add Task").left(),
                "the search field fits: {:?}",
                field
            );
        }
    }
    // the wide window still shows every label in full
    let w = Ui1::new();
    assert!(w.rect("Columns").width() > 60.0 && w.has("Team Space") && w.has("Projects"));
}

#[test]
fn task_names_use_the_room_up_to_the_columns() {
    let mut h = Ui1::sized(960.0, 560.0);
    let row = h.rect("Task Write copy for landing page");
    let cols = crate::app::shell::col_layout(row, &h.app.prefs.hidden_cols, true);
    assert!(
        h.painted_text("Write copy for landing page")
            .iter()
            .any(|(r, _)| row.contains(r.center())),
        "the whole name fits at 900 px"
    );
    // a long name is cut with an ellipsis just before the first column
    let long = "Prepare the quarterly marketing report with charts for every channel and region";
    let id = h.task_id("Write copy for landing page");
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .name = long.into();
    h.settle();
    let row = h.rect(&format!("Task {}", long));
    let mut found = None;
    for s in &h.shapes {
        if let egui::Shape::Text(ts) = &s.shape {
            let txt = ts.galley.text();
            if txt.ends_with('\u{2026}')
                && long.starts_with(txt.trim_end_matches('\u{2026}'))
                && row.contains(ts.visual_bounding_rect().center())
            {
                found = Some(ts.visual_bounding_rect());
            }
        }
    }
    let r = found.expect("the long name is shown cut with an ellipsis");
    assert!(
        r.right() <= cols.left,
        "the cut name {:?} ends before the columns at {}",
        r,
        cols.left
    );
    assert!(
        r.right() >= cols.left - 60.0,
        "the cut name {:?} uses the room up to the columns at {}",
        r,
        cols.left
    );
}

#[test]
fn board_card_boxes_open_the_pickers_and_the_card_opens() {
    let mut h = Ui1::new();
    h.click("View Board");
    h.click("Card priority Set up analytics dashboard");
    h.click("Urgent");
    assert_eq!(task_named(&h, "Set up analytics dashboard").priority, 1);
    assert!(
        h.app.open_task.is_none(),
        "the box changes the field, it doesn't open the task"
    );
    h.click("Card assignees Set up analytics dashboard");
    h.click_side("Sam Lee", true);
    assert_eq!(
        task_named(&h, "Set up analytics dashboard").assignees,
        vec![3]
    );
    h.key(egui::Key::Escape);
    h.click("Card due date Collect feedback from beta users");
    h.click("Today");
    assert_eq!(
        task_named(&h, "Collect feedback from beta users").due,
        Some(today())
    );
    h.key(egui::Key::Escape);
    h.click("Card Write copy for landing page");
    assert_eq!(
        h.app.open_task,
        Some(task_named(&h, "Write copy for landing page").id)
    );
}

#[test]
fn board_card_options_button_opens_the_task_menu() {
    let mut h = Ui1::new();
    h.click("View Board");
    let n = h.app.data.tasks.len();
    h.click("Card options Set up analytics dashboard");
    h.click("Duplicate");
    assert!(
        h.app.data.tasks.len() > n,
        "Duplicate from the card's ... menu"
    );
}

#[test]
fn board_add_task_asks_for_a_name_first() {
    let mut h = Ui1::new();
    h.click("View Board");
    let n = h.app.data.tasks.len();
    h.click("Add Task in TO DO");
    assert_eq!(
        h.app.data.tasks.len(),
        n,
        "no Untitled task is made by the click"
    );
    h.type_text("Board task");
    h.key(egui::Key::Enter);
    assert_eq!(task_named(&h, "Board task").status, "to do");
    // Esc on an empty field leaves nothing behind
    h.key(egui::Key::Escape);
    assert_eq!(h.app.data.tasks.len(), n + 1);
    assert!(h.app.open_task.is_none());
}

#[test]
fn board_columns_collapse_and_expand() {
    let mut h = Ui1::new();
    h.click("View Board");
    h.click("Collapse group TO DO");
    assert!(
        !h.has("Card Set up analytics dashboard"),
        "the column's cards are hidden"
    );
    h.click("Expand group TO DO");
    assert!(h.has("Card Set up analytics dashboard"));
}

#[test]
fn board_card_dropped_on_a_sidebar_list_moves_the_task() {
    // tall enough for the whole sidebar
    let mut h = Ui1::sized(1440.0, 1200.0);
    h.click("View Board");
    let personal = h.app.data.spaces[0].lists[0].id;
    h.drag("Card Set up analytics dashboard", "Personal");
    assert_eq!(
        task_named(&h, "Set up analytics dashboard").list_id,
        personal
    );
}

#[test]
fn calendar_more_opens_the_days_tasks_and_spans_show_every_day() {
    let mut h = Ui1::new();
    let td = today();
    let base = task_named(&h, "Set up analytics dashboard").clone();
    // only these on the calendar, so the sample tasks don't take the lines
    for x in h.app.data.tasks.iter_mut() {
        x.start = None;
        x.due = None;
    }
    for k in 0..8 {
        let mut x = base.clone();
        x.id = 9000 + k;
        x.name = format!("Busy day {}", k);
        x.start = None;
        x.due = Some(td);
        h.app.data.tasks.push(x);
    }
    // three days inside today's week (weeks start on Sunday): on a Friday or
    // Saturday a span from today would break onto the next row, and its
    // one-day piece today would sit under "+N more"
    let back = (td.weekday().num_days_from_sunday() as i64 - 4).max(0);
    let first = td - Duration::days(back);
    let mut span = base.clone();
    span.id = 9100;
    span.name = "Three day task".into();
    span.start = Some(first);
    span.due = Some(first + Duration::days(2));
    h.app.data.tasks.push(span);
    h.click("View Calendar");
    for d in 0..3 {
        assert!(
            h.has(&format!(
                "Calendar Three day task {}",
                first + Duration::days(d)
            )),
            "drawn on day {}",
            d
        );
    }
    h.click(&format!("More tasks on {}", td));
    h.click("Busy day 7");
    assert_eq!(h.app.open_task, Some(9007));
}

#[test]
fn calendar_shows_due_times_and_orders_a_day_by_them() {
    let mut h = Ui1::new();
    let td = today();
    let base = task_named(&h, "Set up analytics dashboard").clone();
    // only these on the calendar, so none is hidden under "+N more"
    for x in h.app.data.tasks.iter_mut() {
        x.start = None;
        x.due = None;
    }
    for (k, name, at) in [
        (0, "Late call", Some(15)),
        (1, "Early call", Some(9)),
        (2, "All day", None),
    ] {
        let mut x = base.clone();
        x.id = 9200 + k;
        x.name = name.into();
        x.start = None;
        x.due = Some(td);
        x.set_due_time(at.and_then(|hr| chrono::NaiveTime::from_hms_opt(hr, 0, 0)));
        h.app.data.tasks.push(x);
    }
    h.click("View Calendar");
    let y = |h: &Ui1, n: &str| h.rect(&format!("Calendar {} {}", n, td)).center().y;
    assert!(
        y(&h, "All day") < y(&h, "Early call"),
        "a day without a time first"
    );
    assert!(y(&h, "Early call") < y(&h, "Late call"), "then by time");
    assert!(!h.painted_text("9:00 AM").is_empty());
    assert!(!h.painted_text("3:00 PM").is_empty());
}

#[test]
fn calendar_unscheduled_items_have_the_task_menu() {
    let mut h = Ui1::new();
    h.click("View Calendar");
    let n = h.app.data.tasks.len();
    h.right_click("Unscheduled Collect feedback from beta users", 20.0);
    h.click("Duplicate");
    assert_eq!(h.app.data.tasks.len(), n + 1);
}

#[test]
fn gantt_bar_ends_resize_and_empty_rows_schedule() {
    let mut h = Ui1::new();
    h.click("View Gantt");
    let before = task_named(&h, "Write copy for landing page").clone();
    let r = h.rect("End of Write copy for landing page");
    // a day is 30 px wide at the Day scale
    h.drag_by(r.center(), r.center() + vec2(90.0, 0.0));
    let after = task_named(&h, "Write copy for landing page").clone();
    assert_eq!(after.due, before.due.map(|d| d + Duration::days(3)));
    assert_eq!(after.start, before.start, "only the end moved");
    let r = h.rect("Start of Write copy for landing page");
    h.drag_by(r.center(), r.center() - vec2(60.0, 0.0));
    assert_eq!(
        task_named(&h, "Write copy for landing page").start,
        before.start.map(|d| d - Duration::days(2))
    );
    // a task without dates gets them from a click in its row
    let r = h.rect("Schedule Collect feedback from beta users");
    // (the row runs under the names column too; click where it shows)
    h.click_at(pos2(1000.0, r.center().y));
    assert!(task_named(&h, "Collect feedback from beta users")
        .due
        .is_some());
}

#[test]
fn gantt_shows_subtasks_when_expanded() {
    let mut h = Ui1::new();
    h.click("View Gantt");
    assert!(!h.has("Schedule Wireframe hero section"));
    h.click("Subtasks of Design new homepage layout");
    assert!(
        h.has("Schedule Wireframe hero section"),
        "the subtask gets its own row"
    );
}

#[test]
fn timeline_lanes_switch_grouping() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    assert!(h.app.view == ViewKind::Timeline);
    // by assignee: the task Me and Alex share has a bar in both lanes
    for lane in [
        "Lane ME",
        "Lane ALEX KIM",
        "Lane SAM LEE",
        "Lane UNASSIGNED",
    ] {
        assert!(h.has(lane), "{} is shown", lane);
    }
    assert_eq!(h.count_named("Bar Design new homepage layout"), 2);
    // a task with only a due date gets a one-day bar in the Unassigned lane
    let bar = h.rect("Bar Set up analytics dashboard");
    assert!(bar.width() < 30.0, "one day wide: {}", bar.width());
    let lane = h.rect("Lane UNASSIGNED");
    assert!(bar.center().y > lane.top() && bar.center().y < lane.bottom());
    h.click("Lanes by Status");
    assert_eq!(h.app.timeline.by, GroupBy::Status);
    assert!(h.has("Lane IN PROGRESS") && h.has("Lane TO DO"));
    assert!(!h.has("Lane ALEX KIM"));
    assert_eq!(h.count_named("Bar Design new homepage layout"), 1);
    h.click("Lanes by Priority");
    assert_eq!(h.app.timeline.by, GroupBy::Priority);
    assert!(h.has("Lane URGENT"));
    h.click("Lanes by List");
    assert_eq!(h.app.timeline.by, GroupBy::List);
    assert!(h.has("Lane WEBSITE REDESIGN"));
    h.click("Lanes by Assignee");
    assert!(h.has("Lane UNASSIGNED"));
}

#[test]
fn timeline_bar_opens_and_drags_dates() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    let id = task_named(&h, "Write copy for landing page").id;
    h.click("Bar Write copy for landing page");
    assert_eq!(h.app.open_task, Some(id), "a click opens the task");
    h.app.open_task = None;
    h.settle();
    let before = task_named(&h, "Write copy for landing page").clone();
    // a day is 30 px wide at the Day scale, as in the Gantt
    let r = h.rect("Bar Write copy for landing page");
    h.drag_by(r.center(), r.center() + vec2(60.0, 0.0));
    let moved = task_named(&h, "Write copy for landing page").clone();
    assert_eq!(moved.start, before.start.map(|d| d + Duration::days(2)));
    assert_eq!(moved.due, before.due.map(|d| d + Duration::days(2)));
    assert!(h.app.open_task.is_none(), "a drag doesn't open the task");
    let r = h.rect("End of Write copy for landing page");
    h.drag_by(r.center(), r.center() + vec2(90.0, 0.0));
    let after = task_named(&h, "Write copy for landing page").clone();
    assert_eq!(after.due, moved.due.map(|d| d + Duration::days(3)));
    assert_eq!(after.start, moved.start, "only the end moved");
}

#[test]
fn timeline_keeps_a_name_beside_a_bar_off_the_next_bar() {
    let mut h = Ui1::new();
    // two one-day tasks on neighbouring days in the Unassigned lane: the
    // first bar is too narrow for its name, which is written to its right
    let a = task_named(&h, "Write copy for landing page").id;
    let b = task_named(&h, "Set up analytics dashboard").id;
    let today = today();
    for (n, id) in [a, b].into_iter().enumerate() {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.assignees.clear();
        t.start = None;
        t.due = Some(today + Duration::days(n as i64));
    }
    h.click("View Timeline");
    let first = h.rect("Bar Write copy for landing page");
    let second = h.rect("Bar Set up analytics dashboard");
    assert!(first.width() < 60.0, "one day wide: {}", first.width());
    assert!(
        second.left() - first.right() < 60.0,
        "the bars sit on neighbouring days"
    );
    assert!(
        (first.center().y - second.center().y).abs() > 20.0,
        "the second bar gets its own row: {:?} {:?}",
        first,
        second
    );
    let lane = h.rect("Lane UNASSIGNED");
    for r in [first, second] {
        assert!(r.center().y > lane.top() && r.center().y < lane.bottom());
    }
}

#[test]
fn timeline_tray_lists_tasks_without_dates() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    assert!(!h.has("Undated Collect feedback from beta users"));
    h.click("Tasks without dates");
    let id = task_named(&h, "Collect feedback from beta users").id;
    h.click("Undated Collect feedback from beta users");
    assert_eq!(h.app.open_task, Some(id));
}

#[test]
fn timeline_follows_the_filter_and_zoom() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    assert!(h.has("Bar Design new homepage layout"));
    h.app.search = "copy".into();
    h.settle();
    assert!(h.has("Bar Write copy for landing page"));
    assert!(!h.has("Bar Design new homepage layout"), "filtered out");
    h.app.search.clear();
    h.app.me_mode = true;
    h.settle();
    assert!(h.has("Bar Fix navigation on mobile"));
    assert!(!h.has("Bar Write copy for landing page"), "Me mode");
    h.app.me_mode = false;
    h.click("Month");
    assert_eq!(h.app.timeline.zoom, crate::app::timebar::Zoom::Month);
    h.click("Today");
    let r = h.rect("Bar Write copy for landing page");
    assert!(r.left() > 0.0 && r.right() < 1440.0, "today is in view");
    assert!(r.width() < 30.0, "narrow at the Month scale");
}

#[test]
fn timeline_zoom_buttons_are_square() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    for name in ["Zoom out", "Zoom in"] {
        let r = h.rect(name);
        assert!(
            (r.width() - r.height()).abs() < 0.5,
            "{name}: {:?}",
            r.size()
        );
    }
    h.assert_plus_centered("Zoom in");
    assert_eq!(h.app.timeline.zoom, crate::app::timebar::Zoom::Day);
    h.click("Zoom out");
    assert_eq!(h.app.timeline.zoom, crate::app::timebar::Zoom::Week);
    h.click("Zoom in");
    assert_eq!(h.app.timeline.zoom, crate::app::timebar::Zoom::Day);
}

#[test]
fn table_sorts_renames_and_chooses_columns() {
    // wide enough for every column and the "+" after them
    let mut h = Ui1::sized(1900.0, 900.0);
    h.click("View Table");
    h.click("Sort by Name");
    let y = |h: &Ui1, n: &str| h.rect(&format!("Rename {}", n)).center().y;
    assert!(
        y(&h, "Collect feedback from beta users") < y(&h, "Write copy for landing page"),
        "A to Z"
    );
    h.click("Sort by Name");
    assert!(
        y(&h, "Collect feedback from beta users") > y(&h, "Write copy for landing page"),
        "Z to A"
    );
    h.click("Rename Collect feedback from beta users");
    h.frame(vec![egui::Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    h.type_text("Pick colours");
    h.key(egui::Key::Enter);
    assert!(h.app.data.tasks.iter().any(|t| t.name == "Pick colours"));
    assert!(h.app.open_task.is_none(), "renaming doesn't open the task");
    // custom fields get columns; any column but # and Name can be hidden
    h.app.data.extra.fields.push(CustomField {
        id: 777,
        name: "Budget".into(),
        kind: "number".into(),
        options: vec![],
    });
    h.settle();
    assert!(h.has("Sort by Budget"));
    h.assert_plus_centered("Table columns");
    h.click("Table columns");
    h.click("Column Tags");
    assert!(!h.has("Sort by Tags"));
    h.click("Column Budget");
    assert!(!h.has("Sort by Budget"));
}

#[test]
fn planner_with_monday_weeks_shows_this_week_on_sundays_too() {
    // with weeks starting on Monday, a Sunday is the last day of its week:
    // the Planner opened (or Today pressed) on a Sunday showed next week
    let mut d = Data::seed();
    d.set_lab(Lab::WeekMonday, true);
    let mut h = Ui1::new();
    h.app = App::build(None, Some(d));
    h.app.notify_rt.last_clock = i64::MAX;
    h.app.section = Section::Planner;
    h.settle();
    let td = today();
    let day = |d: NaiveDate| format!("Add task on {}", d.format("%a %-d"));
    let week = h.app.planner_week;
    assert_eq!(week.weekday(), chrono::Weekday::Mon);
    assert!(
        (0..7).contains(&(td - week).num_days()),
        "{week} holds {td}"
    );
    assert!(h.has(&day(td)));
    h.click("Next period");
    assert!(!h.has(&day(td)));
    h.click("Today");
    assert_eq!(h.app.planner_week, week);
    assert!(h.has(&day(td)));
    // the Labs option off: the week that overlaps most, from Sunday
    h.app.data.set_lab(Lab::WeekMonday, false);
    h.settle();
    assert_eq!(
        h.app.planner_week,
        sunday_week_start(week + Duration::days(3))
    );
}

#[test]
fn planner_day_adds_a_task_for_me() {
    let mut h = Ui1::new();
    h.app.section = Section::Planner;
    h.settle();
    let day = h.app.planner_week + Duration::days(3);
    h.click(&format!("Add task on {}", day.format("%a %-d")));
    h.type_text("Planned from the day");
    h.key(egui::Key::Enter);
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Planned from the day")
        .expect("task added");
    assert_eq!(t.due, Some(day));
    assert_eq!(t.assignees, vec![h.app.data.me]);
}

#[test]
fn form_questions_reorder_require_and_block_bad_submit() {
    let mut h = Ui1::new();
    h.app.new_form();
    h.settle();
    let fid = h.app.form.open.unwrap();
    let form = |h: &Ui1| {
        h.app
            .data
            .extra
            .forms
            .iter()
            .find(|f| f.id == fid)
            .cloned()
            .unwrap()
    };
    h.click("Question 2 menu");
    h.click("Move question 2 up");
    assert_eq!(form(&h).fields[0].label, "Details");
    h.click("Required 1");
    assert!(form(&h).fields[0].required);
    h.app.form.tab = super::more::FormTab::Fill;
    h.settle();
    let n = h.app.data.tasks.len();
    h.click("Submit");
    assert_eq!(
        h.app.data.tasks.len(),
        n,
        "a required question stops the submit"
    );
    assert!(h
        .app
        .sweep
        .form_error
        .as_deref()
        .unwrap_or("")
        .contains("Details"));
    h.click("Answer 1");
    h.type_text("Needs a desk");
    h.click("Submit");
    assert_eq!(h.app.data.tasks.len(), n + 1);
    // answers don't follow you to another form
    h.app.form.fill.insert(0, "left over".into());
    h.app.new_form();
    h.settle();
    assert!(!h.app.form.fill.values().any(|v| v == "left over"));
}

#[test]
fn dashboard_cards_move_rename_remove_and_add() {
    let mut h = Ui1::new();
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Team board");
    h.settle();
    let did = h.app.dash_open.unwrap();
    let cards = |h: &Ui1| {
        h.app
            .data
            .extra
            .dashboards
            .iter()
            .find(|d| d.id == did)
            .unwrap()
            .cards
            .clone()
    };
    h.click("Remove card Tasks by priority");
    assert!(!cards(&h).unwrap().iter().any(|c| c.kind == "priority"));
    h.click("Move card up Workload by assignee");
    assert_eq!(cards(&h).unwrap()[1].kind, "workload");
    h.click("Rename card Task totals");
    h.type_text(" now");
    h.key(egui::Key::Enter);
    assert_eq!(cards(&h).unwrap()[0].title, "Task totals now");
    h.click("+ Add card");
    h.click("Add card Tasks by priority");
    assert_eq!(cards(&h).unwrap().last().unwrap().kind, "priority");
    // the dashboard can be renamed and starred
    h.click(&format!("Star dash:{}", did));
    assert!(h.app.data.extra.starred.contains(&format!("dash:{}", did)));
}

#[test]
fn timesheet_entry_edits_and_week_picker() {
    let mut h = Ui1::new();
    let td = today();
    let ws = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
    let start = td
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .single()
        .unwrap()
        .timestamp();
    let i = h
        .app
        .data
        .tasks
        .iter()
        .position(|t| t.name == "Renew passport")
        .unwrap();
    h.app.data.tasks[i].time_entries = vec![TimeEntry {
        start,
        end: start + 1800,
        member: 1,
    }];
    h.app.section = Section::Timesheets;
    h.app.sheet_week = ws;
    h.app.sheet_entries = true;
    h.settle();
    h.click("Edit entry Renew passport");
    h.app.sweep.entry_edit.as_mut().unwrap().hours = 2.0;
    h.click("Save");
    let e = &h.app.data.tasks[i].time_entries[0];
    assert_eq!(e.end - e.start, 7200);
    h.app.sheet_entries = false;
    h.settle();
    h.click("Pick a week");
    assert!(
        h.has("Today") || h.has("Tomorrow"),
        "the week picker opens a calendar"
    );
}

#[test]
fn timesheet_drawing_a_long_day_keeps_its_entries() {
    let mut h = Ui1::new();
    let td = today();
    let ws = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
    let start = td
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .single()
        .unwrap()
        .timestamp();
    let i = h
        .app
        .data
        .tasks
        .iter()
        .position(|t| t.name == "Renew passport")
        .unwrap();
    // 30 hours on one day (e.g. typed on another computer): more than a cell's 24h
    let long = vec![TimeEntry {
        start,
        end: start + 30 * 3600,
        member: 1,
    }];
    h.app.data.tasks[i].time_entries = long.clone();
    h.app.section = Section::Timesheets;
    h.app.hub.sheets = SheetsTab::Mine;
    h.app.sheet_week = ws;
    h.app.sheet_entries = false;
    h.app.dirty = false;
    h.settle();
    assert!(h.has("Remove row Renew passport"), "the grid is drawn");
    let e = &h.app.data.tasks[i].time_entries;
    assert_eq!(
        (e.len(), e[0].start, e[0].end),
        (1, long[0].start, long[0].end),
        "drawing the grid changed nothing"
    );
    assert!(!h.app.dirty);
    // no Approvals tab: approving weeks isn't built
    assert!(h.has("All timesheets") && !h.has("Approvals"));
}

#[test]
fn timesheet_rows_added_by_hand_stay_in_their_week() {
    let mut h = Ui1::new();
    let td = today();
    let ws = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
    for t in &mut h.app.data.tasks {
        t.time_entries.clear();
    }
    h.app.section = Section::Timesheets;
    h.app.hub.sheets = SheetsTab::Mine;
    h.app.sheet_week = ws;
    h.app.sheet_entries = false;
    h.app.sheet_force = true;
    h.settle();
    h.click("+ Add task");
    h.click("Renew passport");
    assert!(h.has("Remove row Renew passport"));
    h.click("Next period");
    assert_eq!(h.app.sheet_week, ws + Duration::days(7));
    assert!(!h.has("Remove row Renew passport"), "not in the next week");
    h.click("Previous period");
    assert!(h.has("Remove row Renew passport"));
    h.click("Remove row Renew passport");
    assert!(!h.has("Remove row Renew passport"), "the x takes it off");
}

#[test]
fn list_view_shows_custom_field_columns_edits_them_in_place_and_remembers_them() {
    let mut h = Ui1::new();
    h.app.data.extra.apps.custom_fields = true;
    let field = |id, name: &str, kind: &str, options: &[&str]| CustomField {
        id,
        name: name.into(),
        kind: kind.into(),
        options: options.iter().map(|s| s.to_string()).collect(),
    };
    h.app.data.extra.fields = vec![
        field(9001, "Billable", "checkbox", &[]),
        field(9002, "Size", "dropdown", &["S", "M", "L"]),
    ];
    let name = "Write copy for landing page";
    let tid = task_named(&h, name).id;
    let list = task_named(&h, name).list_id;
    h.app.apply(Action::Go(Location::List(list)));
    h.app.view = ViewKind::List;
    h.settle();
    // make room for two field columns next to the name
    h.app.prefs.hidden_cols.insert("Comments".into());
    h.app.prefs.hidden_cols.insert("Assignee".into());
    h.settle();
    let custom = |h: &Ui1, f: u64| h.app.task(tid).unwrap().custom.get(&f).cloned();
    // a checkbox ticks in place, without opening the task
    h.click(&format!("Billable of {}", name));
    assert_eq!(custom(&h, 9001).as_deref(), Some("true"));
    assert!(h.app.open_task.is_none());
    // a dropdown opens its options
    h.click(&format!("Size of {}", name));
    h.click("M");
    assert_eq!(custom(&h, 9002).as_deref(), Some("M"));
    // the Columns panel turns a field column off; the list remembers it
    h.click("Columns");
    assert!(h.has("Custom fields"));
    h.click("Field Billable");
    assert!(h.app.prefs.hidden_cols.contains("field:9001"));
    h.key(egui::Key::Escape);
    assert!(!h.has(&format!("Billable of {}", name)));
    let other = list_of_other(&h, list);
    h.app.apply(Action::Go(Location::List(other)));
    h.settle();
    h.app.prefs.hidden_cols.remove("field:9001");
    h.settle();
    h.app.apply(Action::Go(Location::List(list)));
    h.settle();
    assert!(
        h.app.prefs.hidden_cols.contains("field:9001"),
        "the first list still hides it"
    );
    // text and number fields open a box; a number takes only numbers
    h.app.data.extra.fields = vec![
        field(9003, "Notes", "text", &[]),
        field(9004, "Points", "number", &[]),
    ];
    h.settle();
    h.click(&format!("Notes of {}", name));
    h.type_text("hello");
    assert_eq!(custom(&h, 9003).as_deref(), Some("hello"));
    h.key(egui::Key::Enter);
    h.click(&format!("Points of {}", name));
    h.type_text("abc");
    assert_eq!(custom(&h, 9004), None);
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("5");
    assert_eq!(custom(&h, 9004).as_deref(), Some("5"));
    // with the Custom Fields ClickApp off the columns go away
    h.key(egui::Key::Enter);
    h.app.data.extra.apps.custom_fields = false;
    h.settle();
    assert!(!h.has(&format!("Notes of {}", name)));
}

#[test]
fn columns_panel_marks_a_property_custom_fields_leave_no_room_for() {
    let mut h = Ui1::new();
    h.app.data.extra.apps.custom_fields = true;
    let name = "Write copy for landing page";
    let list = task_named(&h, name).list_id;
    h.app.apply(Action::Go(Location::List(list)));
    h.app.view = ViewKind::List;
    h.app.prefs.hidden_cols.remove("Created by");
    h.settle();
    let w = h.app.list_width;
    assert!(w > 0.0);
    let row = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, 30.0));
    let prio = h.app.data.extra.apps.priorities;
    // as many field columns as it takes to push "Created by" out
    let mut n = 0;
    while h
        .app
        .list_cols(row)
        .extra
        .iter()
        .any(|c| c.0 == "Created by")
    {
        n += 1;
        h.app.data.extra.fields.push(CustomField {
            id: 9100 + n,
            name: format!("F{}", n),
            kind: "text".into(),
            options: vec![],
        });
        assert!(n < 10);
    }
    assert!(
        crate::app::shell::col_layout(row, &h.app.prefs.hidden_cols, prio)
            .extra
            .iter()
            .any(|c| c.0 == "Created by"),
        "without the fields it would fit"
    );
    h.settle();
    h.click("Columns");
    let created = h.rect("Created by");
    let marks: Vec<Rect> = h
        .tree
        .iter()
        .filter(|(n, _)| n == "no room")
        .map(|(_, r)| *r)
        .collect();
    assert!(
        marks
            .iter()
            .any(|r| (r.center().y - created.center().y).abs() < 6.0),
        "\"no room\" beside Created by: {:?} {:?}",
        created,
        marks
    );
}

#[test]
fn a_teammates_earlier_entry_merging_in_mid_edit_does_not_move_my_change() {
    let mut h = Ui1::new();
    let (i, early) = shared_timesheet(&mut h);
    // a second entry of mine at 13:00 the same day
    let late = early + 3 * 3600;
    h.app.data.tasks[i].time_entries.push(TimeEntry {
        start: late,
        end: late + 1800,
        member: 1,
    });
    h.app.dirty = true;
    h.settle();
    h.app.sync_note_change();
    let before = h.app.data.clone();
    // the newest entry is listed first: change the 13:00 one to 2 hours
    let top = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Edit entry Renew passport")
        .map(|(_, r)| *r)
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap();
    h.click_at(top.center());
    assert_eq!(h.app.sweep.entry_edit.as_ref().unwrap().start, late);
    h.app.sweep.entry_edit.as_mut().unwrap().hours = 2.0;
    h.settle();
    // a teammate's entry from earlier that day merges in: the team merge
    // sorts entries by start, so every entry of mine moves one place down
    let theirs = early - 3600;
    let team = teammate_change(&before, |d| {
        d.tasks[i].time_entries.push(TimeEntry {
            start: theirs,
            end: theirs + 900,
            member: 2,
        });
    });
    h.app.merge_in(&[team], &[]);
    assert_eq!(
        h.app.data.tasks[i].time_entries[0].member, 2,
        "the merge put their entry first"
    );
    h.settle();
    h.click("Save");
    let es = &h.app.data.tasks[i].time_entries;
    assert_eq!(es.len(), 3);
    let t = es.iter().find(|e| e.member == 2).unwrap();
    assert_eq!(
        (t.start, t.end),
        (theirs, theirs + 900),
        "the teammate's entry is untouched"
    );
    let mut mine: Vec<(i64, i64)> = es
        .iter()
        .filter(|e| e.member == 1)
        .map(|e| (e.start, e.end - e.start))
        .collect();
    mine.sort();
    assert_eq!(
        mine,
        vec![(early, 1800), (late, 7200)],
        "the 13:00 entry got the change and the 10:00 one is kept"
    );
    assert!(h.app.sweep.entry_edit.is_none());
}

#[test]
fn an_entry_taken_away_elsewhere_mid_edit_closes_the_editor_with_a_note() {
    let mut h = Ui1::new();
    let (i, _) = shared_timesheet(&mut h);
    // the week's only entry: once it's gone the page shows no list (it
    // stayed open then, on Sundays, when the sample's other entries fall in
    // the week before)
    for (k, x) in h.app.data.tasks.iter_mut().enumerate() {
        if k != i {
            x.time_entries.clear();
        }
    }
    h.settle();
    let before = h.app.data.clone();
    h.click("Edit entry Renew passport");
    assert!(h.has("Entry hours"));
    // my other device took the entry away
    let team = teammate_change(&before, |d| d.tasks[i].time_entries.clear());
    h.app.merge_in(&[team], &[]);
    assert!(h.app.data.tasks[i].time_entries.is_empty());
    h.settle();
    assert!(h.app.sweep.entry_edit.is_none(), "the editor closed");
    assert!(!h.has("Entry hours"));
    let note = h
        .app
        .toast
        .as_ref()
        .map(|t| t.0.clone())
        .unwrap_or_default();
    assert!(note.contains("closed without saving"), "{}", note);
    assert!(h.app.data.tasks[i].time_entries.is_empty(), "nothing saved");
}

/// A task of mine without a date dragged from "To schedule" onto a day takes
/// that day; dragged from the day back onto "To schedule" it loses it again.
#[test]
fn planner_drag_schedules_and_unschedules_a_task() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let id = h.app.data.new_id();
    let mut x = task_named(&h, "Write copy for landing page").clone();
    x.id = id;
    x.name = "Plan me".into();
    x.assignees = vec![me];
    x.due = None;
    x.start = None;
    x.parent = None;
    h.app.data.tasks.push(x);
    h.app.section = Section::Planner;
    h.settle();
    let day = h.app.planner_week + Duration::days(3);
    let from = h.painted_text("Plan me")[0].0.center();
    let add = h.rect(&format!("Add task on {}", day.format("%a %-d")));
    let to = add.center() + vec2(0.0, 80.0);
    h.drag_by(from, to);
    h.settle();
    let due = |h: &Ui1| h.app.data.tasks.iter().find(|t| t.id == id).unwrap().due;
    assert_eq!(due(&h), Some(day), "dropped on the day");
    // and back onto To schedule, under its heading
    // the day's narrow column may shorten the name; another task starting
    // "Plan" may be on another day of the week
    let mut from = None;
    each_text(&h.shapes, |ts, clip| {
        let at = ts.visual_bounding_rect().intersect(clip).center();
        if ts.galley.text().starts_with("Plan") && add.x_range().contains(at.x) {
            from = Some(at);
        }
    });
    let from = from.expect("the task on its day");
    let sched = h.painted_text(tr("To schedule"))[0].0;
    h.drag_by(from, sched.center() + vec2(40.0, 120.0));
    h.settle();
    assert_eq!(due(&h), None, "its date taken off");
}

/// In a small window the Gantt toolbar wraps: the zoom buttons used to be
/// drawn over "1 without dates", and at the smallest size Week, Month,
/// "without dates" and "Export CSV" piled on top of each other.
#[test]
fn gantt_toolbar_wraps_in_a_small_window() {
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Ko] {
        for (w, hh) in [(460.0, 640.0), (360.0, 640.0), (960.0, 560.0)] {
            let mut h = Ui1::sized(w, hh);
            let list = list_id(&h, "Website Redesign");
            h.app.loc = Location::List(list);
            h.app.view = ViewKind::Gantt;
            h.settle();
            crate::i18n::set_lang(lang);
            h.settle();
            let undated = trf!("{} without dates", 1);
            let (chip, _) = h.painted_text(&undated)[0];
            let mut boxes = vec![("without dates".to_string(), chip)];
            for name in ["Zoom out", "Zoom in"] {
                boxes.push((name.to_string(), h.rect(name)));
            }
            for name in ["Today", "Day", "Week", "Month", "Export CSV"] {
                boxes.push((name.to_string(), h.rect(tr(name))));
            }
            for (i, (a, ra)) in boxes.iter().enumerate() {
                assert!(
                    ra.right() <= w,
                    "{lang:?} {w}x{hh}: {a} reaches {}",
                    ra.right()
                );
                for (b, rb) in &boxes[i + 1..] {
                    let o = ra.intersect(*rb);
                    assert!(
                        !(o.width() > 0.5 && o.height() > 0.5),
                        "{lang:?} {w}x{hh}: {a} {ra:?} and {b} {rb:?} overlap"
                    );
                }
            }
            crate::i18n::set_lang(crate::i18n::Lang::En);
        }
    }
}

/// A teammate whose name is longer than the Timeline's lane column: the
/// lane's name is shortened with "…" and keeps clear of its count. It used
/// to be cut mid-letter by the column and drawn over the count.
#[test]
fn timeline_lane_names_are_shortened_clear_of_the_count() {
    let mut h = Ui1::sized(1440.0, 900.0);
    let long = "Bartholomew Montgomery-Fitzgerald Worthington";
    let mut p = h.app.data.members[0].clone();
    p.id = 9_001;
    p.name = long.into();
    p.display_name = String::new();
    h.app.data.members.push(p);
    let id = h.task_id("Design new homepage layout");
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|x| x.id == id)
        .unwrap()
        .assignees = vec![9_001];
    let list = list_id(&h, "Website Redesign");
    h.app.loc = Location::List(list);
    h.app.view = ViewKind::Timeline;
    h.settle();
    let lane = h.rect(&format!("Lane {}", long.to_uppercase()));
    let texts = h.texts_in(lane, Seen::Visible);
    let name = texts
        .iter()
        .find(|(s, _)| s.starts_with("BARTHOLOMEW"))
        .expect("the lane's name is drawn");
    assert!(name.0.ends_with('\u{2026}'), "shortened: {:?}", name.0);
    assert!(
        name.1.right() <= lane.right(),
        "inside the column: {name:?}"
    );
    assert!(h.cut_texts(lane).is_empty(), "{:?}", h.cut_texts(lane));
    let over = layout_tests::overlapping(&texts);
    assert!(over.is_empty(), "{over:?}");
    assert!(
        texts.iter().any(|(s, _)| s == "1"),
        "the count shows: {texts:?}"
    );
}

/// The Calendar's week view in the smallest window: the week's dates keep
/// clear of the Week / Month toggle, which moves to the next row when they
/// don't fit. The dates used to sit under the toggle.
#[test]
fn calendar_week_dates_keep_clear_of_the_toggle() {
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Ko] {
        for (w, hh) in [(360.0, 640.0), (460.0, 640.0), (960.0, 560.0)] {
            for week in [true, false] {
                let mut h = Ui1::sized(w, hh);
                let list = list_id(&h, "Website Redesign");
                h.app.loc = Location::List(list);
                h.app.view = ViewKind::Calendar;
                h.app.cal.week = week;
                h.settle();
                crate::i18n::set_lang(lang);
                h.settle();
                h.settle();
                let toggles = [h.rect(tr("Week")), h.rect(tr("Month"))];
                let today = h.rect(tr("Today"));
                let band = Rect::from_x_y_ranges(
                    0.0..=w,
                    today.top() - 2.0..=toggles[0].bottom().max(today.bottom()) + 2.0,
                );
                for (s, r) in h.texts_in(band, Seen::Visible) {
                    assert!(r.right() <= w, "{lang:?} {w}x{hh}: {s:?} reaches {r:?}");
                    for tg in toggles {
                        if tg.contains(r.center()) {
                            continue;
                        }
                        let o = r.intersect(tg);
                        assert!(
                            !(o.width() > 0.5 && o.height() > 0.5),
                            "{lang:?} {w}x{hh} week {week}: {s:?} {r:?} under {tg:?}"
                        );
                    }
                }
                crate::i18n::set_lang(crate::i18n::Lang::En);
            }
        }
    }
}

/// A List grouped by priority in the light theme: the group pills' text is
/// dark on the light yellow (High) and light blue (Normal), readable at
/// least 3:1. It used to be white on them, hard to read.
#[test]
fn priority_group_pills_are_readable() {
    let mut h = Ui1::sized(1440.0, 900.0);
    let list = list_id(&h, "Website Redesign");
    h.app.loc = Location::List(list);
    h.app.view = ViewKind::List;
    h.app.prefs.group_by = GroupBy::Priority;
    h.settle();
    let mut seen = 0;
    for p in [2usize, 3, 4] {
        let label = tr(crate::model::PRIORITIES[p].label).to_uppercase();
        let fill = rgb(crate::model::PRIORITIES[p].color);
        for (r, c) in h.painted_text(&label) {
            // the pill's text, not the same word in a row's Priority column
            if r.height() > 16.0 {
                continue;
            }
            seen += 1;
            let k = contrast(
                c,
                if p == 4 {
                    Color32::from_rgb(0xb0, 0xb0, 0xb0)
                } else {
                    fill
                },
            );
            assert!(k >= 3.0, "{label}: {c:?} on {fill:?} is {k:.2}:1");
        }
    }
    assert!(seen >= 2, "the High and Normal groups are drawn");
}

/// The inline "+ Add task" row in the smallest window: Save fits inside it.
/// It used to be cut by the right edge, the field being at least 200 wide.
#[test]
fn inline_add_task_row_fits_a_small_window() {
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Ko] {
        for (w, hh) in [(360.0, 640.0), (460.0, 640.0), (960.0, 560.0)] {
            let mut h = Ui1::sized(w, hh);
            let list = list_id(&h, "Website Redesign");
            h.app.loc = Location::List(list);
            h.app.view = ViewKind::List;
            h.settle();
            crate::i18n::set_lang(lang);
            h.settle();
            let add = h
                .tree
                .iter()
                .find(|(n, _)| n.starts_with("Add Task in "))
                .map(|(n, _)| n.clone())
                .unwrap();
            h.scroll_into_view(&add);
            h.click(&add);
            h.settle();
            for name in [tr("Save"), tr("Cancel")] {
                let r = h.rect(name);
                assert!(r.right() <= w - 8.0, "{lang:?} {w}x{hh}: {name} at {r:?}");
            }
            crate::i18n::set_lang(crate::i18n::Lang::En);
        }
    }
}
