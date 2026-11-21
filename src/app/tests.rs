// Behaviour tests: each one drives the same code the buttons and menus call
// and checks the result. Run with `cargo test`.

use super::chat::ChatAction;
use super::comments::{parse_link, segments, toggle_comment_check, toggle_reaction, Seg};
use super::nav::Link as LinkTo;
use super::*;
use workboard_server::testing::{temp_home, wait_until, TestServer};

mod attachments;
mod chat;
mod comments;
mod filters;
mod server_sync;
mod slack;

pub(super) fn app() -> App {
    App::build(None, Some(Data::seed()))
}

/// A new workspace named `workspace` with only `name` (member `id`) in it.
pub(super) fn blank_as(workspace: &str, id: u64, name: &str) -> Data {
    Data::blank(
        workspace,
        Member {
            id,
            name: name.into(),
            ..Default::default()
        },
    )
}

fn list_ids(a: &App) -> Vec<u64> {
    a.data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .map(|l| l.id)
                .chain(s.folders.iter().flat_map(|f| f.lists.iter().map(|l| l.id)))
        })
        .collect()
}

fn add_task(a: &mut App, list: u64, name: &str) -> u64 {
    a.apply(Action::Add {
        list,
        name: name.into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    a.data
        .tasks
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.id)
        .expect("task was added")
}

/// Tasks are looked up by id and by parent through an index made once per
/// change; it gives the right answer right after tasks are added, deleted,
/// moved under another and changed by hand, before and after a frame.
#[test]
fn task_lookups_stay_right_as_tasks_change() {
    let mut a = app();
    let ctx = egui::Context::default();
    let frame = |a: &mut App| {
        let _ = ctx.run(egui::RawInput::default(), |ctx| a.ui_frame(ctx));
    };
    frame(&mut a);
    let list = list_ids(&a)[0];
    let p = add_task(&mut a, list, "Parent");
    let c = add_task(&mut a, list, "Child");
    assert_eq!(a.task(c).unwrap().name, "Child", "before a frame");
    frame(&mut a);
    assert_eq!(a.task(p).unwrap().name, "Parent");
    assert!(a.subtasks(p).is_empty());

    // made a subtask (an edit, taken in on the next frame)
    let mut child = a.task(c).unwrap().clone();
    child.parent = Some(p);
    a.apply(Action::Replace(child));
    assert_eq!(a.subtasks(p).len(), 1, "right away");
    frame(&mut a);
    assert_eq!(a.subtasks(p)[0].id, c);

    // tasks shuffled by hand: a stale place is never taken for another task
    a.data.tasks.reverse();
    assert_eq!(a.task(c).unwrap().id, c);
    assert_eq!(a.task(p).unwrap().id, p);

    // deleted
    a.apply(Action::DeleteTask(c));
    frame(&mut a);
    assert!(a.task(c).is_none());
    assert!(a.subtasks(p).is_empty());
    assert_eq!(a.task(p).unwrap().name, "Parent");

    // a status is lent, not copied, and is the list's own
    let t = a.task(p).unwrap();
    assert_eq!(a.data.status_ref(t).name, a.data.status_of(t).name);
    assert!(std::ptr::eq(
        a.data.status_ref(t),
        &a.data.statuses_ref(list)[0]
    ));
}

#[test]
fn create_task_gets_first_status_and_activity() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Write report");
    let t = a.task(id).unwrap();
    assert_eq!(t.status, "to do");
    assert_eq!(t.list_id, list);
    assert!(t.activity.iter().any(|x| x.text.contains("created")));
}

#[test]
fn empty_name_becomes_untitled() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let before = a.data.tasks.len();
    a.apply(Action::Add {
        list,
        name: "   ".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    assert_eq!(a.data.tasks.len(), before + 1);
    assert_eq!(a.data.tasks.last().unwrap().name, "Untitled task");
}

#[test]
fn editing_status_priority_assignee_dates_is_logged() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Edit me");
    let mut t = a.task(id).unwrap().clone();
    t.status = "in progress".into();
    t.priority = 1;
    t.assignees = vec![2];
    t.due = Some(today());
    a.apply(Action::Replace(t));
    let t = a.task(id).unwrap();
    assert_eq!(t.status, "in progress");
    assert_eq!(t.priority, 1);
    assert_eq!(t.assignees, vec![2]);
    let log: Vec<&str> = t.activity.iter().map(|x| x.text.as_str()).collect();
    assert!(log.iter().any(|l| l.contains("changed status")));
    assert!(log.iter().any(|l| l.contains("set priority to Urgent")));
    assert!(log.iter().any(|l| l.contains("assigned to Alex Kim")));
    assert!(log.iter().any(|l| l.contains("due date")));
}

#[test]
fn closing_a_task_hides_it_unless_closed_is_shown() {
    let mut a = app();
    let list = list_ids(&a)[0];
    a.loc = Location::List(list);
    let id = add_task(&mut a, list, "Finish");
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    assert!(a.data.is_closed(a.task(id).unwrap()));
    a.show_closed = false;
    assert!(!a.filtered().iter().any(|t| t.id == id));
    a.show_closed = true;
    assert!(a.filtered().iter().any(|t| t.id == id));
}

#[test]
fn subtasks_are_created_under_the_parent() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let parent = add_task(&mut a, list, "Parent");
    a.apply(Action::Add {
        list,
        name: "Child".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: Some(parent),
        open: false,
    });
    assert_eq!(a.subtasks(parent).len(), 1);
    a.loc = Location::List(list);
    assert!(
        !a.filtered().iter().any(|t| t.name == "Child"),
        "subtasks stay out of the top level"
    );
}

#[test]
fn delete_task_goes_to_trash_and_restores_with_subtasks() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let parent = add_task(&mut a, list, "Doomed");
    a.apply(Action::Add {
        list,
        name: "Doomed child".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: Some(parent),
        open: false,
    });
    a.apply(Action::DeleteTask(parent));
    assert!(a.task(parent).is_none());
    assert!(!a.data.tasks.iter().any(|t| t.name == "Doomed child"));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.title == "Doomed")
        .expect("in trash");
    a.restore_trash(i);
    assert!(a.task(parent).is_some());
    assert!(a.data.tasks.iter().any(|t| t.name == "Doomed child"));
}

#[test]
fn delete_list_goes_to_trash_and_restores() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let n = a.data.tasks.iter().filter(|t| t.list_id == list).count();
    a.do_delete(Target::List(list));
    assert!(!list_ids(&a).contains(&list));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List")
        .expect("list in trash");
    a.restore_trash(i);
    assert!(list_ids(&a).contains(&list));
    assert_eq!(a.data.tasks.iter().filter(|t| t.list_id == list).count(), n);
}

#[test]
fn spaces_folders_lists_create_rename_delete() {
    let mut a = app();
    let spaces = a.data.spaces.len();
    a.apply(Action::NewSpace);
    assert_eq!(a.data.spaces.len(), spaces + 1);
    let sid = a.data.spaces.last().unwrap().id;
    a.apply(Action::NewFolder(sid));
    assert_eq!(a.data.spaces.last().unwrap().folders.len(), 1);
    a.apply(Action::NewList {
        space: sid,
        folder: None,
    });
    let lid = match a.loc {
        Location::List(id) => id,
        _ => panic!("new list should open"),
    };
    a.finish_rename(Target::List(lid), "Roadmap".into());
    assert_eq!(a.data.list_name(lid), "Roadmap");
    a.finish_rename(Target::Space(sid), "Design".into());
    assert_eq!(a.data.location_name(Location::Space(sid)), "Design");
    a.do_delete(Target::Space(sid));
    assert_eq!(a.data.spaces.len(), spaces);
}

#[test]
fn favorites_toggle() {
    let mut a = app();
    let loc = Location::List(list_ids(&a)[1]);
    let had = a.data.favorites.contains(&loc);
    a.apply(Action::ToggleFavorite(loc));
    assert_ne!(had, a.data.favorites.contains(&loc));
    a.apply(Action::ToggleFavorite(loc));
    assert_eq!(had, a.data.favorites.contains(&loc));
}

#[test]
fn timer_starts_stops_and_only_one_runs() {
    let mut a = app();
    let ids: Vec<u64> = a.data.tasks.iter().take(2).map(|t| t.id).collect();
    a.apply(Action::ToggleTimer(ids[0]));
    assert!(a.task(ids[0]).unwrap().timer_started.is_some());
    a.apply(Action::ToggleTimer(ids[1]));
    assert!(
        a.task(ids[0]).unwrap().timer_started.is_none(),
        "starting another timer stops the first"
    );
    assert!(a.task(ids[1]).unwrap().timer_started.is_some());
    let entries = a.task(ids[0]).unwrap().time_entries.len();
    assert!(entries >= 1);
    a.apply(Action::ToggleTimer(ids[1]));
    assert!(a.task(ids[1]).unwrap().timer_started.is_none());
}

#[test]
fn timesheet_hours_replace_the_day() {
    let mut a = app();
    let id = a.data.tasks[1].id;
    let day = today();
    a.set_day_time(id, day, 2.5);
    a.set_day_time(id, day, 1.25);
    let secs: i64 = a
        .task(id)
        .unwrap()
        .time_entries
        .iter()
        .filter(|e| crate::timefmt::local_date(e.start) == day)
        .map(|e| e.end - e.start)
        .sum();
    assert_eq!(secs, 4500);
    a.set_day_time(id, day, 0.0);
    assert!(!a
        .task(id)
        .unwrap()
        .time_entries
        .iter()
        .any(|e| crate::timefmt::local_date(e.start) == day));
}

#[test]
fn automations_run_on_status_change() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let other = list_ids(&a)[1];
    a.data.extra.automations.push(Automation {
        id: 1,
        enabled: true,
        trigger_status: "complete".into(),
        action: "assign".into(),
        value: "3".into(),
        list: None,
        ..Default::default()
    });
    a.data.extra.automations.push(Automation {
        id: 2,
        enabled: true,
        trigger_status: "complete".into(),
        action: "priority".into(),
        value: "4".into(),
        list: None,
        ..Default::default()
    });
    a.data.extra.automations.push(Automation {
        id: 3,
        enabled: true,
        trigger_status: "complete".into(),
        action: "comment".into(),
        value: "Nice work".into(),
        list: None,
        ..Default::default()
    });
    a.data.extra.automations.push(Automation {
        id: 4,
        enabled: true,
        trigger_status: "complete".into(),
        action: "move".into(),
        value: other.to_string(),
        list: None,
        ..Default::default()
    });
    a.data.extra.automations.push(Automation {
        id: 5,
        enabled: false,
        trigger_status: "complete".into(),
        action: "priority".into(),
        value: "1".into(),
        list: None,
        ..Default::default()
    });
    let id = add_task(&mut a, list, "Automate me");
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    let t = a.task(id).unwrap();
    assert!(t.assignees.contains(&3));
    assert_eq!(t.priority, 4, "the disabled rule must not run");
    assert!(t
        .activity
        .iter()
        .any(|x| x.comment && x.text == "Nice work"));
    assert_eq!(t.list_id, other);
}

#[test]
fn templates_fill_the_new_task() {
    let mut a = app();
    let list = list_ids(&a)[0];
    a.pending.template = Some(TaskTemplate {
        id: 9,
        name: "Bug".into(),
        task_name: "Bug report".into(),
        description: "Steps".into(),
        checklist: vec!["Repro".into(), "Fix".into()],
        tags: vec!["bug".into()],
        priority: 2,
        ..Default::default()
    });
    a.apply(Action::Add {
        list,
        name: "Bug report".into(),
        status: None,
        priority: 2,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    let t = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Bug report")
        .unwrap();
    assert_eq!(t.description, "Steps");
    assert_eq!(t.checklist.len(), 2);
    assert_eq!(t.tags, vec!["bug".to_string()]);
    assert_eq!(t.priority, 2);
}

#[test]
fn invite_adds_people_once_and_joins_general() {
    let mut a = app();
    let n = a.data.members.len();
    let added = a.invite_people(
        &[
            "kim.minji@company.com".into(),
            "not-an-email".into(),
            "kim.minji@company.com".into(),
        ],
        "Guest",
    );
    assert_eq!(added, 1);
    assert_eq!(a.data.members.len(), n + 1);
    let m = a.data.members.last().unwrap();
    assert_eq!(m.name, "Kim Minji");
    assert_eq!(m.role, "Guest");
    assert!(m.invited);
    let general = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap();
    assert!(general.members.contains(&m.id));
}

#[test]
fn csv_import_and_export() {
    let mut a = app();
    let dir = temp_home("test");
    std::fs::create_dir_all(&dir).unwrap();
    let csv = dir.join("in.csv");
    std::fs::write(&csv, "name,status,priority,due,assignee\nShip v2,in progress,High,2026-12-01,Sam Lee\nPlan party,,,,\n").unwrap();
    let list = list_ids(&a)[0];
    a.loc = Location::List(list);
    let msg = a.import_csv(csv.to_str().unwrap());
    assert!(msg.contains("Imported 2"), "{}", msg);
    a.apply_actions();
    let t = a.data.tasks.iter().find(|t| t.name == "Ship v2").unwrap();
    assert_eq!(t.status, "in progress");
    assert_eq!(t.priority, 2);
    assert_eq!(t.due, NaiveDate::from_ymd_opt(2026, 12, 1));
    assert_eq!(t.assignees, vec![3]);
    assert!(a.data.tasks.iter().any(|t| t.name == "Plan party"));
}

#[test]
fn personal_list_is_private() {
    let mut a = app();
    let lid = a.create_personal_list();
    assert_eq!(a.personal_list_id(), Some(lid));
    let id = add_task(&mut a, lid, "My secret errand");
    a.loc = Location::Everything;
    a.show_closed = true;
    assert!(a.filtered().iter().any(|t| t.id == id));
    a.data.me = 2; // someone else on this computer
    assert!(
        !a.filtered().iter().any(|t| t.id == id),
        "others can't see my Personal List"
    );
}

#[test]
fn reminders_count_when_due() {
    let mut a = app();
    let me = a.data.me;
    a.data.extra.reminders.push(Reminder {
        id: 1,
        text: "Call".into(),
        due_ts: now_ts() - 60,
        done: false,
        owner: me,
        ..Default::default()
    });
    a.data.extra.reminders.push(Reminder {
        id: 2,
        text: "Later".into(),
        due_ts: now_ts() + 3600,
        done: false,
        owner: me,
        ..Default::default()
    });
    a.data.extra.reminders.push(Reminder {
        id: 3,
        text: "Done".into(),
        due_ts: now_ts() - 60,
        done: true,
        owner: me,
        ..Default::default()
    });
    assert_eq!(a.due_reminders(), 1);
}

#[test]
fn back_and_forward_history() {
    let mut a = app();
    a.track_history();
    let start = a.loc;
    a.apply(Action::Go(Location::Inbox));
    a.track_history();
    a.apply(Action::Go(Location::MyTasks));
    a.track_history();
    a.go_back();
    a.track_history();
    assert_eq!(a.loc, Location::Inbox);
    a.go_back();
    a.track_history();
    assert_eq!(a.loc, start);
    a.go_forward();
    a.track_history();
    assert_eq!(a.loc, Location::Inbox);
}

#[test]
fn everything_survives_save_and_load() {
    let mut a = app();
    a.invite_people(&["new.person@x.io".into()], "Member");
    a.data.extra.goals.push(Goal {
        id: 77,
        name: "Grow".into(),
        color: [1, 2, 3],
        due: None,
        owner: 1,
        targets: vec![GoalTarget {
            name: "Users".into(),
            kind: TargetKind::Number {
                start: 0.0,
                target: 10.0,
                current: 5.0,
                unit: String::new(),
            },
        }],
        ..Default::default()
    });
    let json = serde_json::to_string(&a.data).unwrap();
    let back: Data = serde_json::from_str(&json).unwrap();
    assert_eq!(back.members.len(), a.data.members.len());
    assert_eq!(back.extra.goals.len(), 1);
    assert_eq!(back.channels.len(), a.data.channels.len());
    assert_eq!(back.docs.len(), a.data.docs.len());
    // a file from the first version (no extra fields) still loads
    let old = r#"{"workspace":"W","spaces":[],"tasks":[],"members":[{"id":1,"name":"Me","color":[1,2,3]}],"me":1,"next_id":5,"favorites":[]}"#;
    let d: Data = serde_json::from_str(old).unwrap();
    assert_eq!(d.extra.accent, "black");
}

#[test]
fn goal_progress_mixes_target_kinds() {
    let a = app();
    let done_task = a
        .data
        .tasks
        .iter()
        .find(|t| a.data.is_closed(t))
        .unwrap()
        .id;
    let open_task = a
        .data
        .tasks
        .iter()
        .find(|t| !a.data.is_closed(t))
        .unwrap()
        .id;
    let g = Goal {
        id: 1,
        name: "G".into(),
        color: [0, 0, 0],
        due: None,
        owner: 1,
        targets: vec![
            GoalTarget {
                name: "n".into(),
                kind: TargetKind::Number {
                    start: 0.0,
                    target: 10.0,
                    current: 5.0,
                    unit: String::new(),
                },
            },
            GoalTarget {
                name: "b".into(),
                kind: TargetKind::TrueFalse { done: true },
            },
            GoalTarget {
                name: "t".into(),
                kind: TargetKind::Tasks {
                    ids: vec![done_task, open_task],
                },
            },
        ],
        ..Default::default()
    };
    let p = a.goal_progress(&g);
    assert!((p - (0.5 + 1.0 + 0.5) / 3.0).abs() < 1e-4, "{}", p);
}

#[test]
fn labs_confetti_celebrates_completion() {
    let mut a = app();
    a.data.extra.labs.push("confetti".into());
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Party");
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    assert!(a
        .toast
        .as_ref()
        .map(|x| x.0.contains("complete"))
        .unwrap_or(false));
}

#[test]
fn custom_field_values_are_kept_per_task() {
    let mut a = app();
    a.data.extra.fields.push(CustomField {
        id: 500,
        name: "Budget".into(),
        kind: "number".into(),
        options: vec![],
    });
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Costed");
    let mut t = a.task(id).unwrap().clone();
    t.custom.insert(500, "1200".into());
    assert!(
        task_changed(a.task(id).unwrap(), &t),
        "a custom field edit counts as a change"
    );
    a.apply(Action::Replace(t));
    assert_eq!(
        a.task(id).unwrap().custom.get(&500).map(|s| s.as_str()),
        Some("1200")
    );
}

/// Tests that point HOME at a folder of their own (to save workspace files)
/// take turns: HOME is one for the whole test run.
pub(super) static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn edits_are_saved_once_they_settle_off_the_screens_thread() {
    use std::time::{Duration, Instant};
    let _home = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home-save");
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let mut a = app();
    a.persist = true;
    a.save_now();
    let ctx = egui::Context::default();
    let frame = |a: &mut App| {
        let _ = ctx.run(egui::RawInput::default(), |ctx| a.ui_frame(ctx));
    };
    let on_disk = || {
        Data::finish_saving();
        std::fs::read_to_string(Data::path()).unwrap_or_default()
    };
    let list = list_ids(&a)[0];
    // the first frames tidy the workspace up (a change of their own), and
    // after that a frame with nothing done leaves nothing to save
    for _ in 0..3 {
        frame(&mut a);
    }
    a.save_now();
    frame(&mut a);
    assert!(a.save_due.get().is_none(), "an idle frame changes nothing");

    add_task(&mut a, list, "Saved later");
    frame(&mut a);
    assert!(
        !on_disk().contains("Saved later"),
        "not written on the change itself"
    );
    assert!(a.save_due.get().is_some(), "but waiting to be");
    let quiet = Instant::now() - SAVE_QUIET - Duration::from_millis(10);
    a.save_due.set(Some((quiet, quiet)));
    frame(&mut a);
    assert!(a.save_due.get().is_none());
    let text = on_disk();
    assert!(text.contains("Saved later"), "written once changes stopped");
    assert!(!text.contains("\n "), "as compact text");

    // steady changes (one every frame) are still written after a while
    add_task(&mut a, list, "Typed steadily");
    frame(&mut a);
    let first = Instant::now() - SAVE_AT_MOST - Duration::from_millis(10);
    a.save_due.set(Some((first, Instant::now())));
    frame(&mut a);
    assert!(on_disk().contains("Typed steadily"));

    // quitting doesn't wait
    add_task(&mut a, list, "Saved on quit");
    frame(&mut a);
    eframe::App::on_exit(&mut a, None);
    assert!(on_disk().contains("Saved on quit"));
    assert!(a.save_due.get().is_none());

    // an older copy still queued is written before a save made after it
    let mut older = a.data.clone();
    older.workspace = "An older copy".into();
    older.save_later(Data::path());
    a.data.save();
    assert!(!on_disk().contains("An older copy"));
    assert!(on_disk().contains("Saved on quit"));
}

#[test]
fn workspaces_can_be_created_and_switched() {
    let _home = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home");
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let mut a = app();
    a.persist = true;
    a.data.save();
    let owner = a.data.member(a.data.me).cloned().unwrap();
    let d = Data::blank("Side Biz", owner);
    d.save_file("workspace-test.json");
    Data::save_workspaces(&[
        WsEntry {
            name: "My Workspace".into(),
            file: "data.json".into(),
        },
        WsEntry {
            name: "Side Biz".into(),
            file: "workspace-test.json".into(),
        },
    ]);
    a.switch_workspace("workspace-test.json");
    assert_eq!(a.data.workspace, "Side Biz");
    // the switch menu says where each workspace lives, since names repeat
    assert_eq!(a.workspace_place("data.json"), "This computer");
    let mut team = Data::blank("My Workspace", a.data.member(a.data.me).cloned().unwrap());
    team.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "d1".into(),
        tag: 3,
        server: Some("100.1.2.3".into()),
    });
    team.save_file("workspace-team.json");
    assert_eq!(a.workspace_place("workspace-team.json"), "Team server");
    assert_eq!(
        a.workspace_place("workspace-test.json"),
        "This computer",
        "the open one is read from memory"
    );
    // the next start opens the workspace that was open last
    let reopened = App::build(None, None);
    assert_eq!(reopened.data.workspace, "Side Biz");
    assert_eq!(reopened.ws_file, "workspace-test.json");
    a.switch_workspace("data.json");
    assert_eq!(a.data.workspace, "My Workspace");
}

#[test]
fn archiving_a_doc_saves_a_markdown_copy() {
    let dir = temp_home("arch");
    let mut a = app();
    a.share_here(dir.to_str().unwrap()).unwrap();
    let id = a.data.docs[0].id;
    a.toggle_archive(id);
    assert!(a.data.docs[0].archived);
    let files: Vec<_> = std::fs::read_dir(dir.join("Docs archive"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(files[0].path()).unwrap();
    assert!(text.starts_with(&format!("# {}", a.data.docs[0].title)));
    a.toggle_archive(id);
    assert!(!a.data.docs[0].archived, "Unarchive brings it back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn example_content_is_removed_but_own_work_stays() {
    let mut d = Data::seed();
    d.extra.demo_removed = false;
    // things the person made themselves
    let web = d.spaces[0].folders[0].lists[0].id;
    let mine = d.new_id() + 5000;
    d.tasks.push(Task {
        id: mine,
        name: "My real task".into(),
        list_id: web,
        assignees: vec![1, 2],
        ..d.tasks[0].clone()
    });
    let gen = d.channels.iter().position(|c| c.name == "general").unwrap();
    let mut m = d.channels[gen].messages[0].clone();
    m.id = mine + 2;
    m.author = 1;
    m.text = "real message".into();
    m.replies.clear();
    d.channels[gen].messages.push(m);
    d.remove_demo();

    assert_eq!(d.members.len(), 1, "example people are gone");
    assert_eq!(d.members[0].id, d.me);
    assert_eq!(d.tasks.len(), 1);
    assert_eq!(d.tasks[0].name, "My real task");
    assert_eq!(
        d.tasks[0].assignees,
        vec![1],
        "removed people are unassigned"
    );
    assert!(
        d.list_name(web) == "Website Redesign",
        "a list holding my task stays"
    );
    assert_eq!(d.spaces.len(), 1, "example spaces without my work are gone");
    assert!(d.docs.is_empty());
    let ch: Vec<_> = d
        .channels
        .iter()
        .map(|c| (c.name.clone(), c.messages.len()))
        .collect();
    assert_eq!(
        ch,
        vec![("general".to_string(), 1)],
        "only my message is left"
    );
    assert!(
        d.extra.goals.is_empty()
            && d.extra.whiteboards.is_empty()
            && d.extra.dashboards.is_empty()
            && d.extra.teams.is_empty()
    );
    assert!(d.extra.demo_removed);
}

#[test]
fn pure_example_workspace_becomes_empty() {
    let mut d = Data::seed();
    d.extra.demo_removed = false;
    d.remove_demo();
    assert!(d.tasks.is_empty() && d.docs.is_empty());
    assert_eq!(d.members.len(), 1);
    assert_eq!(d.spaces.len(), 1, "a fresh Team Space to start in");
    assert!(d.first_list().is_some());
    let old: Data = serde_json::from_str(r#"{"workspace":"W","spaces":[],"tasks":[],"members":[],"me":1,"next_id":1,"favorites":[]}"#).unwrap();
    assert!(
        !old.extra.demo_removed,
        "files from before the cleanup get cleaned"
    );
    assert!(Data::fresh().tasks.is_empty());
}

fn other_list(a: &App, not: u64) -> u64 {
    list_ids(a).into_iter().find(|l| *l != not).unwrap()
}

#[test]
fn move_task_takes_subtasks_and_keeps_a_valid_status() {
    let mut a = app();
    let from = list_ids(&a)[0];
    let to = other_list(&a, from);
    let id = add_task(&mut a, from, "Movable");
    a.apply(Action::Add {
        list: from,
        name: "Child".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: Some(id),
        open: false,
    });
    // a status the target list does not have falls back to its first status
    a.data.tasks.iter_mut().find(|t| t.id == id).unwrap().status = "made up".into();
    a.move_tasks(&[id], to);
    let t = a.task(id).unwrap();
    assert_eq!(t.list_id, to);
    assert_eq!(t.status, "to do");
    assert!(t
        .activity
        .iter()
        .any(|x| x.text.starts_with("moved this task to")));
    let child = a.data.tasks.iter().find(|t| t.name == "Child").unwrap();
    assert_eq!(child.list_id, to, "subtasks move with their task");
}

#[test]
fn duplicate_task_copies_subtasks_under_the_copy() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Original");
    a.apply(Action::Add {
        list,
        name: "Step".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: Some(id),
        open: false,
    });
    let copy = a.duplicate_task(id).unwrap();
    assert_eq!(a.task(copy).unwrap().name, "Original (copy)");
    assert_eq!(a.subtasks(copy).len(), 1);
    assert_eq!(a.subtasks(id).len(), 1, "the original keeps its subtask");
}

#[test]
fn closing_a_recurring_task_reopens_it_with_the_next_date() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Weekly report");
    let due = today();
    let mut t = a.task(id).unwrap().clone();
    t.due = Some(due);
    t.recur = Some("weekly".into());
    t.checklist = vec![CheckItem {
        text: "send".into(),
        done: true,
        assignee: None,
    }];
    a.apply(Action::Replace(t));
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    let t = a.task(id).unwrap();
    assert_eq!(t.status, "to do");
    assert_eq!(t.due, Some(due + Duration::days(7)));
    assert!(!t.checklist[0].done);
    assert_eq!(
        next_recur(NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(), "monthly"),
        NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
    );
}

#[test]
fn lists_move_and_duplicate_with_their_tasks() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let n = a
        .data
        .tasks
        .iter()
        .filter(|t| t.list_id == list && t.parent.is_none())
        .count();
    let target = a
        .data
        .spaces
        .iter()
        .find(|s| a.data.space_of_list(list).map(|x| x.id) != Some(s.id))
        .unwrap()
        .id;
    a.move_list(list, target, None);
    assert_eq!(a.data.space_of_list(list).unwrap().id, target);
    let copy = a.duplicate_list(list).unwrap();
    assert_eq!(
        a.data.list_name(copy),
        format!("{} (copy)", a.data.list_name(list))
    );
    assert_eq!(
        a.data
            .tasks
            .iter()
            .filter(|t| t.list_id == copy && t.parent.is_none())
            .count(),
        n
    );
}

#[test]
fn new_list_from_the_personal_list_goes_to_a_visible_space() {
    let mut a = app();
    let pl = a.create_personal_list();
    a.loc = Location::List(pl);
    a.open_new_list(a.current_space(), None);
    let Some(Modal::NewList { space, .. }) = a.modal.take() else {
        panic!("Create List dialog")
    };
    assert!(a
        .data
        .spaces
        .iter()
        .any(|s| s.id == space && s.private_owner.is_none()));
    let id = a.create_list(space, None, "Launch").unwrap();
    assert_eq!(a.data.list_name(id), "Launch");
    assert_eq!(a.loc, Location::List(id));
}

#[test]
fn doc_pages_nest_and_stay_out_of_the_doc_list() {
    let mut a = app();
    a.new_doc(None);
    let root = a.doc_open.unwrap();
    let page = a.add_page(root);
    let sub = a.add_page(page);
    assert_eq!(a.doc_root(sub), root);
    assert_eq!(
        a.data
            .docs
            .iter()
            .filter(|d| d.parent.is_none() && (d.id == page || d.id == sub))
            .count(),
        0
    );
}

#[test]
fn doc_formatting_adds_markdown_at_the_cursor() {
    let mut s = String::from("Hello\nworld");
    super::docs::apply_block(&mut s, 7, 7, "Heading 2");
    assert_eq!(s, "Hello\n## world");
    super::docs::apply_block(&mut s, 7, 7, "Checklist");
    assert_eq!(
        s, "Hello\n- [ ] world",
        "a new block replaces the old prefix"
    );
    let mut b = String::from("make bold");
    super::docs::apply_block(&mut b, 5, 9, "Bold");
    assert_eq!(b, "make **bold**");
}

#[test]
fn recent_items_remember_tasks_lists_and_docs() {
    let mut a = app();
    let list = list_ids(&a)[0];
    a.loc = Location::List(list);
    a.note_recent();
    let id = add_task(&mut a, list, "Seen");
    a.apply(Action::Open(id));
    a.note_recent();
    let items = a.recent_items(None);
    assert_eq!(items[0].1, id);
    assert!(items.iter().any(|i| i.0 == "list" && i.1 == list));
}

/// A real workboard-server on a free local port, data in a temporary folder.
/// Keep taking in what the server sends until `cond` holds.
pub(super) fn net_until(a: &mut App, what: &str, cond: impl Fn(&App) -> bool) {
    let held = wait_until(
        std::time::Duration::from_secs(15),
        std::time::Duration::from_millis(20),
        || {
            a.net_tick(None);
            cond(a)
        },
    );
    if held {
        return;
    }
    panic!(
        "timed out waiting for: {} ({:?}, toast {:?})",
        what, a.sync_rt.net_status, a.toast
    );
}

fn tid(a: &App, name: &str) -> u64 {
    a.data
        .tasks
        .iter()
        .find(|t| t.name == name)
        .map(|t| t.id)
        .unwrap_or_else(|| panic!("no task {:?}", name))
}

#[test]
fn deleting_offers_undo() {
    let mut a = app();
    let n = a.data.tasks.len();
    let id = tid(&a, "Design new homepage layout");
    a.actions.push(Action::DeleteTask(id));
    a.apply_actions();
    assert_eq!(a.data.tasks.len(), n - 3, "the task and its two subtasks");
    let (trash, _) = a.undo_trash.clone().expect("undo offered");
    assert_eq!(trash.len(), 1);
}

#[test]
fn estimates_read_hours_and_minutes() {
    use super::taskfields::{fmt_estimate, parse_estimate};
    assert_eq!(parse_estimate("2h 30m"), Some(150));
    assert_eq!(parse_estimate("2h30m"), Some(150));
    assert_eq!(parse_estimate("45m"), Some(45));
    assert_eq!(parse_estimate("1.5h"), Some(90));
    assert_eq!(parse_estimate("2"), Some(120), "a bare number is hours");
    assert_eq!(parse_estimate("2h 15"), Some(135));
    assert_eq!(parse_estimate("1d"), Some(480));
    assert_eq!(parse_estimate("1:45"), Some(105));
    assert_eq!(parse_estimate("soon"), None);
    assert_eq!(parse_estimate(""), None);
    assert_eq!(fmt_estimate(150), "2h 30m");
    assert_eq!(fmt_estimate(120), "2h");
    assert_eq!(fmt_estimate(45), "45m");
}

#[test]
fn relations_show_on_both_sides_and_come_off() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let x = add_task(&mut a, list, "X");
    let y = add_task(&mut a, list, "Y");
    let z = add_task(&mut a, list, "Z");
    a.set_relation(x, TaskPick::WaitingOn, y);
    a.set_relation(x, TaskPick::Blocking, z);
    a.set_relation(x, TaskPick::Linked, y);
    assert_eq!(a.relations(x), (vec![y], vec![z], vec![y]));
    assert_eq!(a.relations(y), (vec![], vec![x], vec![x]));
    assert_eq!(a.relations(z), (vec![x], vec![], vec![]));
    assert!(
        !a.pick_candidates(x, TaskPick::WaitingOn).contains(&y),
        "already waiting on it"
    );
    a.remove_relation(y, TaskPick::Linked, x);
    a.remove_relation(z, TaskPick::WaitingOn, x);
    a.remove_relation(x, TaskPick::WaitingOn, y);
    assert_eq!(a.relations(x), (vec![], vec![], vec![]));
    // a deleted task drops out
    a.set_relation(x, TaskPick::WaitingOn, y);
    a.apply(Action::DeleteTask(y));
    assert!(a.relations(x).0.is_empty());
}

#[test]
fn parent_picker_skips_loops_and_top_level_undoes_it() {
    let mut a = app();
    let lists = list_ids(&a);
    let p = add_task(&mut a, lists[0], "Parent");
    let c = add_task(&mut a, lists[1], "Child");
    a.set_relation(c, TaskPick::Parent, p);
    let t = a.task(c).unwrap();
    assert_eq!((t.parent, t.list_id), (Some(p), lists[0]));
    assert!(
        !a.pick_candidates(p, TaskPick::Parent).contains(&c),
        "a task can't go under its own subtask"
    );
    assert!(
        !a.pick_candidates(c, TaskPick::Child).contains(&p),
        "nor take its parent as a subtask"
    );
    a.make_top_level(c);
    assert_eq!(a.task(c).unwrap().parent, None);
    assert!(a
        .task(c)
        .unwrap()
        .activity
        .iter()
        .any(|x| x.text.contains("top-level")));
}

#[test]
fn new_task_fields_load_from_old_files_and_stay_out_when_empty() {
    let a = app();
    let t = a.data.tasks[0].clone();
    let json = serde_json::to_string(&t).unwrap();
    for k in [
        "milestone",
        "watchers",
        "waiting_on",
        "linked",
        "attachments",
        "assignee",
    ] {
        assert!(
            !json.contains(&format!("\"{}\"", k)),
            "{} is left out when empty",
            k
        );
    }
    let back: Task = serde_json::from_str(&json).unwrap();
    assert!(
        !back.milestone
            && back.watchers.is_empty()
            && back.checklist.iter().all(|c| c.assignee.is_none())
    );
    let mut w = t.clone();
    w.watchers.push(2);
    assert!(task_changed(&t, &w));
    let mut e = t.clone();
    e.time_entries.push(TimeEntry {
        start: 0,
        end: 60,
        member: 0,
    });
    assert!(task_changed(&t, &e));
}

#[test]
fn deleted_list_comes_back_into_its_folder_with_its_details() {
    let mut a = app();
    let fid = a.data.spaces[0].folders[0].id;
    let lid = a.data.spaces[0].folders[0].lists[0].id;
    {
        let l = a.data.list_item_mut(lid).unwrap();
        l.color = Some([1, 2, 3]);
        l.description = "About the site".into();
    }
    a.do_delete(Target::List(lid));
    assert!(a.data.list_item(lid).is_none());
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List")
        .unwrap();
    a.restore_trash(i);
    let f = a.data.spaces[0]
        .folders
        .iter()
        .find(|f| f.id == fid)
        .unwrap();
    let l = f
        .lists
        .iter()
        .find(|l| l.id == lid)
        .expect("back in its folder");
    assert_eq!(l.color, Some([1, 2, 3]));
    assert_eq!(l.description, "About the site");
    // a List trashed by an older version (no folder) still restores
    let sid = a.data.spaces[0].id;
    let old = serde_json::to_string(&(
        sid,
        ListItem {
            id: 77,
            name: "Old".into(),
            ..Default::default()
        },
        Vec::<Task>::new(),
    ))
    .unwrap();
    a.data.extra.trash.push(TrashItem {
        id: 1,
        kind: "List".into(),
        title: "Old".into(),
        deleted: 0,
        payload: old,
    });
    let i = a.data.extra.trash.len() - 1;
    a.restore_trash(i);
    assert!(a.data.spaces[0].lists.iter().any(|l| l.id == 77));
}

#[test]
fn app_items_go_to_trash_and_back() {
    let mut a = app();
    let id = a.data.new_id();
    a.data.extra.goals.push(Goal {
        id,
        name: "Ship".into(),
        color: [1, 1, 1],
        due: None,
        owner: a.data.me,
        targets: vec![],
        ..Default::default()
    });
    a.remove_item("goal", id);
    a.data.extra.teams.push(Team {
        id: id + 1,
        name: "Core".into(),
        members: vec![],
        color: [2, 2, 2],
    });
    a.remove_item("team", id + 1);
    a.new_dashboard("Sales");
    let d = a.dash_open.unwrap();
    a.remove_item("dashboard", d);
    assert!(a.data.extra.goals.is_empty() || !a.data.extra.goals.iter().any(|x| x.name == "Ship"));
    assert!(!a.data.extra.dashboards.iter().any(|x| x.id == d));
    while !a.data.extra.trash.is_empty() {
        a.restore_trash(0);
    }
    assert!(a.data.extra.goals.iter().any(|x| x.name == "Ship"));
    assert!(a.data.extra.teams.iter().any(|x| x.name == "Core"));
    assert!(a
        .data
        .extra
        .dashboards
        .iter()
        .any(|x| x.name == "Sales" && x.owner == a.data.me));
}

#[test]
fn form_answers_check_required_number_and_email() {
    use super::more::form_answers;
    let f = Form {
        id: 1,
        name: "Intake".into(),
        list_id: 0,
        description: String::new(),
        fields: vec![
            FormField {
                label: "Title".into(),
                map: "name".into(),
                required: true,
                ..Default::default()
            },
            FormField {
                label: "Seats".into(),
                map: "number".into(),
                ..Default::default()
            },
            FormField {
                label: "Mail".into(),
                map: "email".into(),
                ..Default::default()
            },
            FormField {
                label: "Who".into(),
                map: "assignee".into(),
                ..Default::default()
            },
            FormField {
                label: "Size".into(),
                map: "dropdown".into(),
                options: vec!["S".into(), "M".into()],
                ..Default::default()
            },
        ],
        owner: 1,
        ..Default::default()
    };
    let mut fill: HashMap<usize, String> = HashMap::new();
    assert!(form_answers(&f, &fill).unwrap_err().contains("Title"));
    fill.insert(0, "New desk".into());
    fill.insert(1, "lots".into());
    assert!(form_answers(&f, &fill).unwrap_err().contains("number"));
    fill.insert(1, "4".into());
    fill.insert(2, "nope".into());
    assert!(form_answers(&f, &fill).unwrap_err().contains("email"));
    fill.insert(2, "a@b.co".into());
    fill.insert(3, "2".into());
    fill.insert(4, "M".into());
    let ans = form_answers(&f, &fill).unwrap();
    assert_eq!(ans.name, "New desk");
    assert_eq!(ans.assignees, vec![2]);
    assert!(ans.description.contains("Seats: 4") && ans.description.contains("Size: M"));
}

#[test]
fn notepad_lines() {
    use super::menus::notepad_line;
    assert_eq!(notepad_line("- [ ] Buy milk"), (Some(false), "Buy milk"));
    assert_eq!(notepad_line("  - [x] Done"), (Some(true), "Done"));
    assert_eq!(notepad_line("plain"), (None, "plain"));
}

#[test]
fn whiteboard_undo_brings_back_a_deleted_note() {
    use super::more::WbUndo;
    let mut a = app();
    a.new_whiteboard();
    let wid = a.wb_open.unwrap();
    let wb = a
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    assert_eq!(wb.owner, 1);
    let n = Sticky {
        id: 0,
        pos: [0.0, 0.0],
        text: "Idea".into(),
        color: [1, 1, 1],
    };
    a.sweep.wb_undo.push((wid, WbUndo::NoteGone(0, n)));
    a.wb_undo(wid);
    assert_eq!(
        a.data
            .extra
            .whiteboards
            .iter()
            .find(|w| w.id == wid)
            .unwrap()
            .notes[0]
            .text,
        "Idea"
    );
}

#[test]
fn deleting_a_folder_or_space_sends_its_lists_to_trash() {
    let mut a = app();
    let sid = a.data.spaces[0].id;
    let fid = a.data.spaces[0].folders[0].id;
    let web = a.data.spaces[0].folders[0].lists[0].id;
    let n = a.data.tasks.iter().filter(|t| t.list_id == web).count();
    assert!(n > 0);
    a.do_delete(Target::Folder(fid));
    assert!(!list_ids(&a).contains(&web));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List" && x.title == "Website Redesign")
        .expect("folder's list in trash");
    // the folder is gone, so it comes back in its space and says so
    a.restore_trash(i);
    assert!(a.data.spaces[0].lists.iter().any(|l| l.id == web));
    assert_eq!(a.data.tasks.iter().filter(|t| t.list_id == web).count(), n);
    assert!(a.toast.is_some());
    // a whole space: each list goes to Trash and lands in the first space
    let ideas_space = a.data.spaces[1].id;
    let ideas = a.data.spaces[1].lists[0].id;
    a.do_delete(Target::Space(ideas_space));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.title == "Ideas")
        .expect("space's list in trash");
    a.restore_trash(i);
    assert!(a
        .data
        .spaces
        .iter()
        .find(|s| s.id == sid)
        .unwrap()
        .lists
        .iter()
        .any(|l| l.id == ideas));
}

#[test]
fn a_restored_list_keeps_its_settings_and_folder() {
    let mut a = app();
    let fid = a.data.spaces[0].folders[0].id;
    let mkt = a.data.spaces[0].folders[0].lists[1].id;
    {
        let l = a.data.list_item_mut(mkt).unwrap();
        l.color = Some([1, 2, 3]);
        l.description = "about".into();
        l.priority = 2;
    }
    a.do_delete(Target::List(mkt));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List")
        .unwrap();
    a.restore_trash(i);
    let f = a.data.spaces[0]
        .folders
        .iter()
        .find(|f| f.id == fid)
        .unwrap();
    let l = f
        .lists
        .iter()
        .find(|l| l.id == mkt)
        .expect("back in its folder");
    assert_eq!(
        (l.color, l.description.as_str(), l.priority),
        (Some([1, 2, 3]), "about", 2)
    );
    // an entry saved by an older version (space id, list, tasks) still restores
    let sid = a.data.spaces[1].id;
    let old = ListItem {
        id: 9999,
        name: "Old one".into(),
        ..Default::default()
    };
    let payload = serde_json::to_string(&(sid, old, Vec::<Task>::new())).unwrap();
    a.data.extra.trash.push(TrashItem {
        id: 1,
        kind: "List".into(),
        title: "Old one".into(),
        deleted: 0,
        payload,
    });
    let i = a.data.extra.trash.len() - 1;
    a.restore_trash(i);
    assert!(a.data.spaces[1].lists.iter().any(|l| l.id == 9999));
}

#[test]
fn a_restored_task_whose_list_is_gone_finds_a_home() {
    let mut a = app();
    let ideas = a.data.spaces[1].lists[0].id;
    let id = add_task(&mut a, ideas, "Lonely");
    a.apply(Action::DeleteTask(id));
    a.do_delete(Target::List(ideas));
    // the list is in Trash: it comes back with the task
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.title == "Lonely")
        .unwrap();
    a.restore_trash(i);
    assert!(list_ids(&a).contains(&ideas));
    assert_eq!(a.task(id).unwrap().list_id, ideas);
    // the list is gone for good: the task goes to the first list
    a.apply(Action::DeleteTask(id));
    a.do_delete(Target::List(ideas));
    a.data.extra.trash.retain(|x| x.kind != "List");
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.title == "Lonely")
        .unwrap();
    a.restore_trash(i);
    assert_eq!(a.task(id).unwrap().list_id, a.data.first_list().unwrap());
    assert!(a
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("list is gone")));
}

#[test]
fn renaming_or_deleting_a_status_carries_its_tasks() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Moving");
    let mut t = a.task(id).unwrap().clone();
    t.status = "in progress".into();
    a.apply(Action::Replace(t));
    a.data.extra.automations.push(Automation {
        id: 5,
        enabled: true,
        trigger_status: "in progress".into(),
        action: "priority".into(),
        value: "1".into(),
        list: None,
        ..Default::default()
    });
    let si = 0;
    let mut orig: Vec<Option<String>> = a.data.spaces[si]
        .statuses
        .iter()
        .map(|s| Some(s.name.clone()))
        .collect();
    let i = a.data.spaces[si]
        .statuses
        .iter()
        .position(|s| s.name == "in progress")
        .unwrap();
    let mut rules = orig.clone();
    // a rename that matches another status waits, so tasks never merge
    a.data.spaces[si].statuses[i].name = "to do".into();
    a.commit_status_renames(si, &mut orig, &mut rules);
    assert_eq!(a.task(id).unwrap().status, "in progress");
    a.data.spaces[si].statuses[i].name = "doing".into();
    a.commit_status_renames(si, &mut orig, &mut rules);
    assert_eq!(a.task(id).unwrap().status, "doing");
    // the other space still has "in progress", so the automation stays
    assert_eq!(a.data.extra.automations[0].trigger_status, "in progress");
    // deleting it moves the task to a status of the same kind (or the first)
    let st = a.data.spaces[si].statuses.remove(i);
    let old = orig.remove(i).unwrap();
    a.move_off_status(si, &old, st.kind, &orig);
    let t = a.task(id).unwrap();
    let kind_match = a.data.spaces[si]
        .statuses
        .iter()
        .find(|s| s.kind == st.kind)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| a.data.spaces[si].statuses[0].name.clone());
    assert_eq!(t.status, kind_match);
    assert!(t
        .activity
        .iter()
        .any(|x| x.text.contains("changed status from DOING")));
}

#[test]
fn archived_subtasks_hide_and_my_subtasks_show_on_personal_views() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let parent = add_task(&mut a, list, "Parent");
    let me = a.data.me;
    a.apply(Action::Add {
        list,
        name: "Mine sub".into(),
        status: None,
        priority: 0,
        assignees: vec![me],
        due: Some(today()),
        parent: Some(parent),
        open: false,
    });
    let sub = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Mine sub")
        .unwrap()
        .id;
    a.loc = Location::MyTasks;
    assert!(
        a.filtered().iter().any(|t| t.id == sub),
        "my subtask on Assigned to me"
    );
    a.loc = Location::TodayOverdue;
    assert!(
        a.filtered().iter().any(|t| t.id == sub),
        "my subtask on Today & Overdue"
    );
    a.set_archived(&[sub], true);
    assert!(a.subtasks(parent).is_empty(), "archived subtask hidden");
    assert!(!a.filtered().iter().any(|t| t.id == sub));
    // the archived view still reaches it
    a.loc = Location::List(list);
    a.filter.archived = true;
    assert!(a.filtered().iter().any(|t| t.id == sub));
    assert!(!a.task_visible(a.task(sub).unwrap()));
}

#[test]
fn tasks_made_on_today_and_overdue_stay_there() {
    let mut a = app();
    a.loc = Location::TodayOverdue;
    let g = Group {
        key: "all".into(),
        label: String::new(),
        color: Color32::WHITE,
        status: None,
        priority: None,
        assignee: None,
        due: None,
        tasks: vec![],
    };
    a.add_from_group(&g, "Due now".into(), false);
    a.apply_actions();
    let t = a.data.tasks.iter().find(|t| t.name == "Due now").unwrap();
    assert!(t.assignees.contains(&a.data.me));
    assert_eq!(t.due, Some(today()));
    assert!(a.filtered().iter().any(|x| x.name == "Due now"));
    a.open_quick_add(None);
    let qa = a.quick_add.as_ref().unwrap();
    assert_eq!(
        (qa.assignees.clone(), qa.due),
        (vec![a.data.me], Some(today()))
    );
}

#[test]
fn first_list_skips_archived_spaces() {
    let mut a = app();
    a.data.spaces[0].archived = true;
    let ideas = a.data.spaces[1].lists[0].id;
    assert_eq!(a.data.first_list(), Some(ideas));
    a.loc = Location::Everything;
    assert_eq!(a.default_list(), Some(ideas));
}

#[test]
fn going_somewhere_clears_the_selection_and_overview() {
    let mut a = app();
    a.selected.insert(1);
    let sid = a.data.spaces[0].id;
    a.apply(Action::Go(Location::Space(sid)));
    assert!(a.selected.is_empty());
    a.view = ViewKind::Overview;
    a.apply(Action::Go(Location::Everything));
    assert!(a.view == ViewKind::List);
}

#[test]
fn a_recurring_task_reopens_without_a_date_done() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Repeat me");
    let mut t = a.task(id).unwrap().clone();
    t.due = Some(today());
    t.recur = Some("daily".into());
    a.apply(Action::Replace(t));
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    let t = a.task(id).unwrap();
    assert_eq!(t.status, "to do");
    assert!(t.done_at.is_none() && t.closed_at.is_none());
}

#[test]
fn csv_export_reads_back_with_commas_quotes_and_line_breaks() {
    let mut a = app();
    let list = a.data.spaces[1].lists[0].id;
    a.loc = Location::List(list);
    let names = ["Plain", "Commas, and more", "Say \"hi\"", "Two\nlines"];
    for n in names {
        add_task(&mut a, list, n);
    }
    let text = a.csv_text();
    let dir = temp_home("csv");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("round.csv");
    std::fs::write(&f, &text).unwrap();
    let before = a.data.tasks.len();
    let msg = a.import_csv(f.to_str().unwrap());
    a.apply_actions();
    assert!(
        msg.contains(&format!("Imported {}", a.data.tasks.len() - before)),
        "{}",
        msg
    );
    assert!(a.data.tasks.len() - before >= names.len());
    let new: Vec<String> = a.data.tasks[before..]
        .iter()
        .map(|t| t.name.clone())
        .collect();
    for n in names {
        assert!(new.iter().any(|x| x == n), "{:?} missing from {:?}", n, new);
    }
    assert_eq!(
        super::menus::parse_csv("a, \"b,\"\"c\"\"\" ,d\n"),
        vec![vec!["a".to_string(), "b,\"c\"".into(), "d".into()]]
    );
}

#[test]
fn a_login_name_starting_with_a_non_ascii_letter() {
    assert_eq!(Data::display_user("\u{e9}va"), "\u{c9}va");
    assert_eq!(Data::display_user("\u{c9c0}\u{c6d0}"), "\u{c9c0}\u{c6d0}");
    assert_eq!(Data::display_user(""), "Me");
}

#[test]
fn my_timer_leaves_a_teammates_running() {
    let mut a = app();
    let ids: Vec<u64> = a.data.tasks.iter().take(2).map(|t| t.id).collect();
    let started = now_ts() - 600;
    if let Some(t) = a.data.tasks.iter_mut().find(|t| t.id == ids[0]) {
        t.timer_started = Some(started);
        t.timer_by = Some(2);
    }
    // starting mine stops only mine
    a.apply(Action::ToggleTimer(ids[1]));
    assert_eq!(
        a.task(ids[0]).unwrap().timer_started,
        Some(started),
        "Alex's timer keeps running"
    );
    assert_eq!(a.task(ids[1]).unwrap().timer_by, Some(1));
    // on the task Alex is timing I run my own timer beside his (and, timing
    // one task at a time, mine on the other task stops)
    a.apply(Action::ToggleTimer(ids[0]));
    assert_eq!(
        a.task(ids[0]).unwrap().timer_started,
        Some(started),
        "Alex's timer keeps running"
    );
    assert!(a.data.is_my_timer(a.task(ids[0]).unwrap()));
    assert!(a.task(ids[1]).unwrap().timer_started.is_none());
    // closing the app (on_exit) stops mine and keeps Alex's
    a.data.stop_my_timers(now_ts());
    assert!(a.task(ids[0]).unwrap().timers.is_empty());
    assert_eq!(
        a.task(ids[0]).unwrap().time_entries.last().unwrap().member,
        1,
        "the stopped time is mine"
    );
    assert_eq!(a.task(ids[0]).unwrap().timer_started, Some(started));
    // an old timer without an owner is never stopped behind anyone's back
    if let Some(t) = a.data.tasks.iter_mut().find(|t| t.id == ids[0]) {
        t.timer_by = None;
    }
    a.data.stop_my_timers(now_ts());
    assert!(a.task(ids[0]).unwrap().timer_started.is_some());
}

#[test]
fn timesheet_edits_only_my_entries() {
    let mut a = app();
    let id = a.data.tasks[1].id;
    let day = today();
    let nine = day
        .and_hms_opt(9, 0, 0)
        .and_then(|x| x.and_local_timezone(chrono::Local).single())
        .unwrap()
        .timestamp();
    let task = a.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
    task.assignees = vec![3];
    task.time_entries.push(TimeEntry {
        start: nine,
        end: nine + 3600,
        member: 2,
    });
    // an old entry with no owner belongs to the first assignee (Sam), not me
    task.time_entries.push(TimeEntry {
        start: nine,
        end: nine + 1800,
        member: 0,
    });
    a.set_day_time(id, day, 2.0);
    let t = a.task(id).unwrap();
    let mine: i64 = t
        .time_entries
        .iter()
        .filter(|e| a.data.is_my_entry(t, e))
        .map(|e| e.end - e.start)
        .sum();
    assert_eq!(mine, 7200);
    assert!(
        t.time_entries
            .iter()
            .any(|e| e.member == 2 && e.end - e.start == 3600),
        "Alex's hour stays"
    );
    assert!(
        t.time_entries
            .iter()
            .any(|e| e.member == 0 && e.end - e.start == 1800),
        "the old entry stays"
    );
    a.set_day_time(id, day, 0.5);
    let t = a.task(id).unwrap();
    assert_eq!(
        t.time_entries
            .iter()
            .filter(|e| e.member == 1)
            .map(|e| e.end - e.start)
            .sum::<i64>(),
        1800
    );
    a.set_day_time(id, day, 0.0);
    let t = a.task(id).unwrap();
    assert!(!t.time_entries.iter().any(|e| e.member == 1));
    assert_eq!(t.time_entries.len(), 2);
    assert_eq!(t.entry_owner(&t.time_entries[1]), 3);
}

#[test]
fn scoped_automation_runs_only_in_its_list_and_moves_subtasks() {
    let mut a = app();
    let lists = list_ids(&a);
    let (list, other) = (lists[0], lists[1]);
    a.data.extra.automations.push(Automation {
        id: 1,
        enabled: true,
        trigger_status: "complete".into(),
        action: "priority".into(),
        value: "4".into(),
        list: Some(other),
        ..Default::default()
    });
    a.data.extra.automations.push(Automation {
        id: 2,
        enabled: true,
        trigger_status: "complete".into(),
        action: "move".into(),
        value: other.to_string(),
        list: Some(list),
        ..Default::default()
    });
    let id = add_task(&mut a, list, "Scoped");
    a.apply(Action::Add {
        list,
        name: "Scoped child".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: Some(id),
        open: false,
    });
    let child = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Scoped child")
        .unwrap()
        .id;
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    let t = a.task(id).unwrap();
    assert_eq!(
        t.priority, 0,
        "the rule for the other list must not run here"
    );
    assert_eq!(t.list_id, other);
    assert_eq!(a.task(child).unwrap().list_id, other, "subtasks move along");
    let statuses = a.data.statuses_for_list(other);
    assert!(statuses
        .iter()
        .any(|s| s.name == a.task(child).unwrap().status));
}

#[test]
fn removing_a_person_drops_them_from_teams_and_meetings() {
    let mut a = app();
    a.data.extra.teams.push(Team {
        id: 900,
        name: "T".into(),
        members: vec![1, 3],
        color: [0, 0, 0],
    });
    a.data.extra.meetings.push(Meeting {
        id: 901,
        title: "M".into(),
        date: today(),
        time: "10:00".into(),
        attendees: vec![1, 3],
        agenda: String::new(),
        notes: String::new(),
    });
    a.remove_item("person", 3);
    assert!(a.data.member(3).is_none());
    assert_eq!(a.data.extra.teams[0].members, vec![1]);
    assert_eq!(a.data.extra.meetings[0].attendees, vec![1]);
    a.remove_item("meeting", 901);
    assert!(a.data.extra.meetings.is_empty());
}

#[test]
fn goal_tasks_ignore_deleted_ones() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Goal task");
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    let g = Goal {
        id: 902,
        name: "G".into(),
        color: [0, 0, 0],
        due: None,
        owner: 1,
        targets: vec![GoalTarget {
            name: "x".into(),
            kind: TargetKind::Tasks {
                ids: vec![id, 123456789],
            },
        }],
        ..Default::default()
    };
    assert_eq!(a.goal_progress(&g), 1.0);
}

#[test]
fn workboard_links_are_read_back() {
    use super::nav::{location_link, parse_link, Link};
    assert_eq!(parse_link("workboard://task/42"), Some(Link::Task(42)));
    assert_eq!(
        parse_link("  workboard://list/7/  "),
        Some(Link::Loc(Location::List(7)))
    );
    assert_eq!(
        parse_link("workboard://space/3."),
        Some(Link::Loc(Location::Space(3)))
    );
    assert_eq!(
        parse_link("workboard://folder/9"),
        Some(Link::Loc(Location::Folder(9)))
    );
    assert_eq!(
        parse_link("workboard://inbox"),
        Some(Link::Loc(Location::Inbox))
    );
    assert_eq!(parse_link("workboard://task/abc"), None);
    assert_eq!(parse_link("workboard://task"), None);
    assert_eq!(parse_link("workboard://task/1/2"), None);
    assert_eq!(parse_link("https://task/1"), None);
    assert_eq!(parse_link("workboard://My-Workspace/Team-Space"), None);
    for l in [
        Location::Home,
        Location::Inbox,
        Location::Replies,
        Location::Assigned,
        Location::MyTasks,
        Location::TodayOverdue,
        Location::Everything,
        Location::Space(1),
        Location::Folder(2),
        Location::List(3),
    ] {
        assert_eq!(parse_link(&location_link(l)), Some(Link::Loc(l)));
    }
    // links inside a message, by character (Korean text before one)
    let found = super::nav::find_links(
        "보세요 workboard://task/5, and (workboard://list/6) not workboard://x",
    );
    assert_eq!(found.len(), 2);
    assert_eq!(found[0], (4, 22, "workboard://task/5".to_string()));
    assert_eq!(found[1].2, "workboard://list/6");
}

#[test]
fn opening_a_link_goes_there() {
    let mut a = app();
    let list = list_ids(&a)[1];
    let tid = add_task(&mut a, list, "Linked");
    assert!(a.open_link(&format!("workboard://task/{}", tid)));
    a.apply_actions();
    assert_eq!(a.open_task, Some(tid));
    assert!(a.open_link(&format!("workboard://list/{}", list)));
    a.apply_actions();
    assert_eq!(a.loc, Location::List(list));
    // a task that isn't here: a notice, nothing opens
    a.open_task = None;
    assert!(!a.open_link("workboard://task/999999"));
    a.apply_actions();
    assert_eq!(a.open_task, None);
    assert!(a.toast.is_some());
}

#[test]
fn back_returns_to_the_previous_channel() {
    let mut a = app();
    let chans: Vec<u64> = a.data.channels.iter().map(|c| c.id).collect();
    a.section = Section::Chat;
    a.chat.open = Some(chans[0]);
    a.track_history();
    a.chat.open = Some(chans[1]);
    a.track_history();
    a.go_back();
    a.track_history();
    assert_eq!((a.section, a.chat.open), (Section::Chat, Some(chans[0])));
    a.go_forward();
    a.track_history();
    assert_eq!(a.chat.open, Some(chans[1]));
}

#[test]
fn back_skips_deleted_pages_and_new_workspace_forgets_history() {
    let mut a = app();
    let lists = list_ids(&a);
    a.apply(Action::Go(Location::List(lists[0])));
    a.track_history();
    a.apply(Action::Go(Location::List(lists[1])));
    a.track_history();
    a.apply(Action::Go(Location::Inbox));
    a.track_history();
    // the list before goes away: Back lands on the one before it
    for s in &mut a.data.spaces {
        s.lists.retain(|l| l.id != lists[1]);
        for f in &mut s.folders {
            f.lists.retain(|l| l.id != lists[1]);
        }
    }
    a.go_back();
    a.track_history();
    assert_eq!(a.loc, Location::List(lists[0]));
    a.clear_history();
    assert!(a.nav.back.is_empty() && a.nav.fwd.is_empty());
}

#[test]
fn a_deleted_space_or_folder_comes_back_whole() {
    let mut a = app();
    // a space with its own look and statuses
    let si = 1;
    let sid = a.data.spaces[si].id;
    let ideas = a.data.spaces[si].lists[0].id;
    a.data.spaces[si].color = [9, 8, 7];
    a.data.spaces[si].icon = "*".into();
    a.data.spaces[si].statuses = vec![
        Status::new("idea", [1, 1, 1], StatusKind::Open),
        Status::new("shipped", [2, 2, 2], StatusKind::Closed),
    ];
    let n = add_task(&mut a, ideas, "Big idea");
    a.data.tasks.iter_mut().find(|t| t.id == n).unwrap().status = "shipped".into();
    a.do_delete(Target::Space(sid));
    assert!(!a.data.spaces.iter().any(|s| s.id == sid));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "Space")
        .expect("the space is in Trash");
    a.restore_trash(i);
    let sp = a
        .data
        .spaces
        .iter()
        .find(|s| s.id == sid)
        .expect("the space is back");
    assert_eq!(
        (sp.color, sp.icon.as_str(), sp.statuses.len()),
        ([9, 8, 7], "*", 2)
    );
    assert!(sp.lists.iter().any(|l| l.id == ideas), "with its list");
    assert_eq!(a.task(n).unwrap().status, "shipped");
    assert!(!a
        .data
        .extra
        .trash
        .iter()
        .any(|x| x.kind == "List" && x.title == "Ideas"));

    // a folder: one list restored on its own first goes to the space; the
    // folder then comes back with the other one in it
    let fid = a.data.spaces[0].folders[0].id;
    let (web, mkt) = (
        a.data.spaces[0].folders[0].lists[0].id,
        a.data.spaces[0].folders[0].lists[1].id,
    );
    a.do_delete(Target::Folder(fid));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List" && x.title == "Website Redesign")
        .unwrap();
    a.restore_trash(i);
    assert!(a.data.spaces[0].lists.iter().any(|l| l.id == web));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "Folder")
        .expect("the folder is in Trash");
    a.restore_trash(i);
    let f = a.data.spaces[0]
        .folders
        .iter()
        .find(|f| f.id == fid)
        .expect("the folder is back");
    assert_eq!(f.lists.iter().map(|l| l.id).collect::<Vec<_>>(), vec![mkt]);
}

#[test]
fn a_list_restored_into_another_space_takes_that_spaces_statuses() {
    let mut a = app();
    let sid = a.data.spaces[1].id;
    let ideas = a.data.spaces[1].lists[0].id;
    a.data.spaces[1].statuses = vec![
        Status::new("idea", [1, 1, 1], StatusKind::Open),
        Status::new("doing", [2, 2, 2], StatusKind::Custom),
        Status::new("shipped", [3, 3, 3], StatusKind::Closed),
        Status::new("to do", [4, 4, 4], StatusKind::Custom),
    ];
    let mut ids = vec![];
    for (name, status) in [
        ("one", "idea"),
        ("two", "doing"),
        ("three", "shipped"),
        ("four", "to do"),
    ] {
        let t = add_task(&mut a, ideas, name);
        a.data.tasks.iter_mut().find(|x| x.id == t).unwrap().status = status.into();
        ids.push(t);
    }
    a.do_delete(Target::List(ideas));
    a.data.spaces.retain(|s| s.id != sid);
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List")
        .unwrap();
    a.restore_trash(i);
    assert!(
        a.data.spaces[0].lists.iter().any(|l| l.id == ideas),
        "into the team space"
    );
    let st: Vec<String> = ids
        .iter()
        .map(|t| a.task(*t).unwrap().status.clone())
        .collect();
    assert_eq!(
        st,
        vec!["to do", "in progress", "complete", "to do"],
        "same name, else the same kind"
    );
    assert!(a
        .task(ids[1])
        .unwrap()
        .activity
        .iter()
        .any(|x| x.text.contains("changed status from DOING to IN PROGRESS")));
}

#[test]
fn a_restored_list_skips_archived_and_other_peoples_spaces() {
    let mut a = app();
    let ideas = a.data.spaces[1].lists[0].id;
    let sid = a.data.spaces[1].id;
    a.do_delete(Target::List(ideas));
    a.data.spaces.retain(|s| s.id != sid);
    a.data.spaces[0].archived = true;
    a.data.spaces.insert(
        0,
        Space {
            id: 7100,
            name: "Alex's".into(),
            private_owner: Some(2),
            statuses: default_statuses(),
            ..Default::default()
        },
    );
    a.data.spaces.push(Space {
        id: 7200,
        name: "Fresh".into(),
        statuses: default_statuses(),
        ..Default::default()
    });
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List")
        .unwrap();
    a.restore_trash(i);
    assert!(a
        .data
        .spaces
        .iter()
        .find(|s| s.id == 7200)
        .unwrap()
        .lists
        .iter()
        .any(|l| l.id == ideas));
}

#[test]
fn timesheet_typing_or_dragging_keeps_one_entry_per_day() {
    let mut a = app();
    let id = a.data.tasks[1].id;
    let day = today();
    a.data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .time_entries
        .clear();
    // a drag sends many small steps
    for k in 1..=20 {
        a.set_day_time(id, day, 0.05 * k as f64);
    }
    let t = a.task(id).unwrap();
    assert_eq!(t.time_entries.len(), 1, "one entry grows instead of twenty");
    assert_eq!(t.time_entries[0].end - t.time_entries[0].start, 3600);
}

#[test]
fn timesheet_rows_belong_to_a_week_and_old_rows_to_every_week() {
    let mut a = app();
    let me = a.data.me;
    let ids: Vec<u64> = a.data.tasks.iter().take(2).map(|t| t.id).collect();
    let ws = today() - Duration::days(today().weekday().num_days_from_sunday() as i64);
    a.data.extra.sheet_rows = vec![[me, ids[0]]];
    a.add_sheet_rows(ws, vec![ids[1]]);
    assert_eq!(a.my_sheet_rows(ws), vec![ids[0], ids[1]]);
    assert_eq!(
        a.my_sheet_rows(ws + Duration::days(7)),
        vec![ids[0]],
        "an old row shows in every week"
    );
    a.drop_sheet_row(ws + Duration::days(7), ids[0]);
    assert_eq!(
        a.my_sheet_rows(ws),
        vec![ids[1]],
        "removing an old row takes it off every week"
    );
    // saved files without week rows stay the same
    let mut e = a.data.extra.clone();
    e.sheet_week_rows.clear();
    assert!(!serde_json::to_string(&e)
        .unwrap()
        .contains("sheet_week_rows"));
}

#[test]
fn each_person_runs_a_timer_on_the_same_task() {
    let mut a = app();
    let id = a.data.tasks[0].id;
    let started = now_ts() - 600;
    let t = a.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
    t.timer_started = Some(started);
    t.timer_by = Some(2);
    assert!(
        !serde_json::to_string(&a.data.tasks[0])
            .unwrap()
            .contains("\"timers\""),
        "no new field when one timer runs"
    );
    a.apply(Action::ToggleTimer(id));
    let t = a.task(id).unwrap();
    assert!(a.data.is_my_timer(t));
    assert_eq!(
        (t.timer_started, t.timer_by),
        (Some(started), Some(2)),
        "Alex's timer stays where older apps read it"
    );
    assert!(t.tracked_secs(now_ts()) >= 600);
    // Alex stops his: mine moves into the field older apps read
    a.data.me = 2;
    a.apply(Action::ToggleTimer(id));
    let t = a.task(id).unwrap();
    assert_eq!(t.timer_by, Some(1));
    assert!(t.timers.is_empty());
    assert!(t
        .time_entries
        .iter()
        .any(|e| e.member == 2 && e.start == started));
    a.data.me = 1;
    assert!(a.data.is_my_timer(a.task(id).unwrap()));
    a.apply(Action::ToggleTimer(id));
    assert!(a.task(id).unwrap().timer_started.is_none());
}

#[test]
fn status_renames_and_deletes_carry_automations_of_that_space() {
    let mut a = app();
    let si = 0;
    let sid = a.data.spaces[si].id;
    let list = list_ids(&a)[0];
    let other = a
        .data
        .spaces
        .iter()
        .find(|s| s.id != sid && s.statuses.iter().any(|x| x.name == "in progress"))
        .map(|s| s.id)
        .expect("another space shares the name");
    let rule = |id, list| Automation {
        id,
        enabled: true,
        trigger_status: "in progress".into(),
        action: "priority".into(),
        value: "1".into(),
        list,
        ..Default::default()
    };
    a.data.extra.automations = vec![
        rule(1, Some(list)),
        rule(2, Some(sid)),
        rule(3, None),
        rule(4, Some(other)),
    ];
    let mut orig: Vec<Option<String>> = a.data.spaces[si]
        .statuses
        .iter()
        .map(|s| Some(s.name.clone()))
        .collect();
    let i = a.data.spaces[si]
        .statuses
        .iter()
        .position(|s| s.name == "in progress")
        .unwrap();
    let mut rules = orig.clone();
    a.data.spaces[si].statuses[i].name = "doing".into();
    a.commit_status_renames(si, &mut orig, &mut rules);
    let trig: Vec<String> = a
        .data
        .extra
        .automations
        .iter()
        .map(|x| x.trigger_status.clone())
        .collect();
    assert_eq!(
        trig,
        ["doing", "doing", "in progress", "in progress"],
        "this space's rules follow; workspace-wide and other spaces' stay"
    );
    let st = a.data.spaces[si].statuses.remove(i);
    let old = orig.remove(i).unwrap();
    a.move_off_status(si, &old, st.kind, &orig);
    let to = a.data.extra.automations[0].trigger_status.clone();
    assert_ne!(to, "doing", "a deleted status's rules move with its tasks");
    assert!(a.data.spaces[si].statuses.iter().any(|s| s.name == to));
    assert_eq!(a.data.extra.automations[3].trigger_status, "in progress");
}

#[test]
fn single_assignee_setting_replaces_instead_of_adding() {
    let mut a = app();
    a.data.extra.apps.multiple_assignees = false;
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "One person");
    let mut t = a.task(id).unwrap().clone();
    t.assignees = vec![3];
    a.apply(Action::Replace(t));
    a.data.extra.automations.push(Automation {
        id: 9,
        enabled: true,
        trigger_status: "in progress".into(),
        action: "assign".into(),
        value: "2".into(),
        list: None,
        ..Default::default()
    });
    let mut t = a.task(id).unwrap().clone();
    t.status = "in progress".into();
    a.apply(Action::Replace(t));
    assert_eq!(a.task(id).unwrap().assignees, vec![2]);
    // My Tasks grouped by assignee: adding in Alex's group makes it mine only
    a.loc = Location::MyTasks;
    let g = Group {
        key: "a:2".into(),
        label: String::new(),
        color: Color32::WHITE,
        status: None,
        priority: None,
        assignee: Some(2),
        due: None,
        tasks: vec![],
    };
    a.add_from_group(&g, "From Alex's group".into(), false);
    a.apply_actions();
    let t = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "From Alex's group")
        .unwrap();
    assert_eq!(t.assignees, vec![a.data.me]);
}

#[test]
fn back_returns_to_the_workspace_overview() {
    let mut a = app();
    a.section = Section::Dashboards;
    a.dash_open = Some(super::hubs::WORKSPACE_OVERVIEW);
    let spot = a.nav_spot();
    assert!(a.nav_spot_exists(&spot));
    a.track_history();
    a.section = Section::Home;
    a.dash_open = None;
    a.track_history();
    a.go_back();
    a.track_history();
    assert_eq!(
        (a.section, a.dash_open),
        (Section::Dashboards, Some(super::hubs::WORKSPACE_OVERVIEW))
    );
}

#[test]
fn links_to_archived_or_private_places_do_not_open() {
    let mut a = app();
    let me = a.data.me;
    let lists = list_ids(&a);
    let tid = add_task(&mut a, lists[0], "Behind a link");
    // someone else's Personal space
    let sid = a.data.space_of_list(lists[0]).unwrap().id;
    a.data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .private_owner = Some(me + 100);
    a.toast = None;
    assert!(!a.open_link(&format!("workboard://space/{}", sid)));
    assert!(!a.open_link(&format!("workboard://list/{}", lists[0])));
    assert!(!a.open_link(&format!("workboard://task/{}", tid)));
    a.apply_actions();
    assert_eq!(a.open_task, None);
    assert!(a
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("archived or private")));
    assert!(a
        .search_rows(&format!("workboard://task/{}", tid), 0)
        .iter()
        .all(|r| r.4 != super::more::Hit::Task(tid)));
    // my own Personal space opens
    a.data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .private_owner = Some(me);
    assert!(a.open_link(&format!("workboard://list/{}", lists[0])));
    // an archived space doesn't
    a.data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .archived = true;
    assert!(!a.open_link(&format!("workboard://space/{}", sid)));
    assert!(!a.open_link(&format!("workboard://list/{}", lists[0])));
}

#[test]
fn back_and_forward_to_another_place_clear_the_selection() {
    let mut a = app();
    let lists = list_ids(&a);
    a.apply(Action::Go(Location::List(lists[0])));
    a.track_history();
    a.apply(Action::Go(Location::List(lists[1])));
    a.track_history();
    let t = a
        .data
        .tasks
        .iter()
        .find(|t| t.list_id == lists[1])
        .map(|t| t.id);
    if let Some(t) = t {
        a.selected.insert(t);
        a.open_task = Some(t);
    }
    a.go_back();
    a.track_history();
    assert_eq!(a.loc, Location::List(lists[0]));
    assert!(a.selected.is_empty() && a.open_task.is_none());
    let t0 = a
        .data
        .tasks
        .iter()
        .find(|t| t.list_id == lists[0])
        .unwrap()
        .id;
    a.selected.insert(t0);
    a.go_forward();
    a.track_history();
    assert_eq!(a.loc, Location::List(lists[1]));
    assert!(a.selected.is_empty());
}

#[test]
fn my_timer_counts_only_my_time() {
    let mut a = app();
    let me = a.data.me;
    let other = a.data.members.iter().find(|m| m.id != me).unwrap().id;
    let now = now_ts();
    let t = &mut a.data.tasks[0];
    t.time_entries = vec![
        TimeEntry {
            start: 0,
            end: 100,
            member: me,
        },
        TimeEntry {
            start: 0,
            end: 1000,
            member: other,
        },
    ];
    t.timer_started = None;
    t.timer_by = None;
    t.timers = vec![
        RunningTimer {
            member: me,
            start: now - 10,
        },
        RunningTimer {
            member: other,
            start: now - 500,
        },
    ];
    let t = a.data.tasks[0].clone();
    assert_eq!(a.data.my_tracked_secs(&t, now), 110);
    assert_eq!(t.tracked_secs(now), 1610);
}

#[test]
fn a_status_name_another_status_still_carries_waits_and_closing_settles_names() {
    let mut a = app();
    let si = 0;
    let sid = a.data.spaces[si].id;
    let list = a.data.lists_in(Location::Space(sid)).unwrap()[0];
    let t1 = add_task(&mut a, list, "On to do");
    let t2 = add_task(&mut a, list, "On doing");
    let mut t = a.task(t2).unwrap().clone();
    t.status = "in progress".into();
    a.apply(Action::Replace(t));
    let mut orig: Vec<Option<String>> = a.data.spaces[si]
        .statuses
        .iter()
        .map(|s| Some(s.name.clone()))
        .collect();
    let mut rules = orig.clone();
    // "to do" emptied, then "in progress" renamed to "to do": the name is
    // still carried by the first status's tasks, so the rename waits
    a.data.spaces[si].statuses[0].name = String::new();
    a.data.spaces[si].statuses[1].name = "to do".into();
    a.commit_status_renames(si, &mut orig, &mut rules);
    assert_eq!(a.task(t1).unwrap().status, "to do");
    assert_eq!(a.task(t2).unwrap().status, "in progress");
    // swapping two names at once works
    a.data.spaces[si].statuses[0].name = "in progress".into();
    a.commit_status_renames(si, &mut orig, &mut rules);
    assert_eq!(a.task(t1).unwrap().status, "in progress");
    assert_eq!(a.task(t2).unwrap().status, "to do");
    // an added status with an empty or taken name gets a free one on close
    a.data.spaces[si]
        .statuses
        .push(Status::new("", [1, 1, 1], StatusKind::Custom));
    orig.push(None);
    a.data.spaces[si].statuses[1].name = "".into();
    assert!(a.settle_status_names(si, &orig));
    let names: Vec<String> = a.data.spaces[si]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names[1], "to do");
    assert_eq!(names.last().unwrap(), "new status");
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len());
}

#[test]
fn a_list_deleted_while_archived_comes_back_not_archived() {
    let mut a = app();
    let ideas = a.data.spaces[1].lists[0].id;
    a.data.list_item_mut(ideas).unwrap().archived = true;
    a.do_delete(Target::List(ideas));
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List")
        .unwrap();
    a.restore_trash(i);
    assert!(a.data.list_item(ideas).is_some());
    assert!(
        !a.data.list_archived(ideas),
        "restored where it can be seen"
    );
    // back in an archived Space: it says so
    let web = a.data.spaces[0].folders[0].lists[0].id;
    a.do_delete(Target::List(web));
    a.data.spaces[0].archived = true;
    let i = a
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "List")
        .unwrap();
    a.toast = None;
    a.restore_trash(i);
    assert!(a
        .toast
        .as_ref()
        .is_some_and(|t| t.0.contains("archived Folder or Space")));
}

#[test]
fn goal_pace_compares_progress_with_time_used() {
    let day = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    let g = Goal {
        start: Some(day - Duration::days(50)),
        due: Some(day + Duration::days(50)),
        ..Default::default()
    };
    assert_eq!(g.time_used(day), Some(0.5));
    assert_eq!(g.pace(0.5, day), Some(Pace::OnTrack));
    assert_eq!(g.pace(0.46, day), Some(Pace::OnTrack), "within 5 points");
    assert_eq!(g.pace(0.30, day), Some(Pace::Behind));
    assert_eq!(g.pace(0.10, day), Some(Pace::AtRisk));
    assert_eq!(g.pace(1.0, day), Some(Pace::Done));
    // the first tenth of the time is too early to call anything late
    assert_eq!(g.pace(0.0, day - Duration::days(45)), Some(Pace::OnTrack));
    assert_eq!(Goal::default().pace(0.3, day), None, "no dates, no pace");
    assert_eq!(Goal::default().pace(1.0, day), Some(Pace::Done));
}

#[test]
fn goal_milestones_follow_progress_and_old_goals_start_quietly() {
    let mut a = app();
    let me = a.data.me;
    let set = |a: &mut App, v: f64| {
        if let TargetKind::Number { current, .. } = &mut a.data.extra.goals[0].targets[0].kind {
            *current = v;
        }
    };
    // a goal from before history was kept, already at 80%
    a.data.extra.goals = vec![Goal {
        id: 1,
        name: "Old".into(),
        owner: me,
        targets: vec![GoalTarget {
            name: "n".into(),
            kind: TargetKind::Number {
                start: 0.0,
                target: 10.0,
                current: 8.0,
                unit: String::new(),
            },
        }],
        ..Default::default()
    }];
    a.goal_watch(now_ts());
    let g = &a.data.extra.goals[0];
    assert_eq!(g.reached, 75, "what it passed already is taken as it is");
    assert_eq!(g.log.len(), 1);
    assert_eq!(g.log[0].what, GoalChange::Tracked, "without announcing it");
    assert_eq!(g.days.last().map(|d| d.1), Some(80));
    // reaching 100% records the milestone and the day
    set(&mut a, 10.0);
    a.goal_watch(now_ts());
    let g = &a.data.extra.goals[0];
    assert_eq!(g.reached, 100);
    assert_eq!(g.done_on, Some(today()));
    assert_eq!(g.days.len(), 1, "one entry per day");
    assert!(g
        .log
        .iter()
        .any(|e| e.what == GoalChange::Milestone(100) && e.who == 0));
    // dropping back clears the day and takes the milestones above off
    set(&mut a, 5.0);
    a.goal_watch(now_ts());
    let g = &a.data.extra.goals[0];
    assert_eq!(g.done_on, None);
    assert_eq!(g.reached, 50);
    assert!(!g
        .log
        .iter()
        .any(|e| matches!(e.what, GoalChange::Milestone(_))));
    // crossing again records it anew, once
    set(&mut a, 10.0);
    a.goal_watch(now_ts());
    let g = &a.data.extra.goals[0];
    assert_eq!(g.done_on, Some(today()));
    let times = g
        .log
        .iter()
        .filter(|e| e.what == GoalChange::Milestone(100))
        .count();
    assert_eq!(times, 1);
}

#[test]
fn goal_history_survives_save_and_load_and_old_files_load() {
    let g = Goal {
        id: 5,
        name: "G".into(),
        start: Some(today()),
        reached: 50,
        log: vec![GoalEvent {
            ts: 1,
            who: 1,
            what: GoalChange::Number {
                target: "n".into(),
                from: 1.0,
                to: 2.0,
                unit: "u".into(),
            },
            pct: 50,
        }],
        days: vec![(today(), 50)],
        ..Default::default()
    };
    let json = serde_json::to_string(&g).unwrap();
    assert!(
        json.contains("\"done_on\":null"),
        "written even when empty: {json}"
    );
    let back: Goal = serde_json::from_str(&json).unwrap();
    assert_eq!(back.log, g.log);
    assert_eq!(back.days, g.days);
    assert_eq!(back.reached, 50);
    // a goal saved before these were kept
    let old = r#"{"id":1,"name":"Old","color":[1,2,3],"due":null,"owner":1,"targets":[]}"#;
    let g: Goal = serde_json::from_str(old).unwrap();
    assert!(g.start.is_none() && g.log.is_empty() && g.reached == 0);
}
