// Behaviour tests: each one drives the same code the buttons and menus call
// and checks the result. Run with `cargo test`.

use super::chat::ChatAction;
use super::*;

fn app() -> App {
    App::build(None, Some(Data::seed()))
}

fn list_ids(a: &App) -> Vec<u64> {
    a.data.spaces.iter().flat_map(|s| s.lists.iter().map(|l| l.id).chain(s.folders.iter().flat_map(|f| f.lists.iter().map(|l| l.id)))).collect()
}

fn add_task(a: &mut App, list: u64, name: &str) -> u64 {
    a.apply(Action::Add { list, name: name.into(), status: None, priority: 0, assignees: vec![], due: None, parent: None, open: false });
    a.data.tasks.iter().find(|t| t.name == name).map(|t| t.id).expect("task was added")
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
    a.apply(Action::Add { list, name: "   ".into(), status: None, priority: 0, assignees: vec![], due: None, parent: None, open: false });
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
fn filters_search_me_mode_and_assignee() {
    let mut a = app();
    a.loc = Location::Everything;
    a.show_closed = true;
    a.search = "homepage".into();
    let r: Vec<String> = a.filtered().iter().map(|t| t.name.clone()).collect();
    assert!(!r.is_empty() && r.iter().all(|n| n.to_lowercase().contains("homepage")));
    a.search.clear();
    a.me_mode = true;
    assert!(a.filtered().iter().all(|t| t.assignees.contains(&a.data.me)));
    a.me_mode = false;
    a.assignee_filter = Some(3);
    assert!(a.filtered().iter().all(|t| t.assignees.contains(&3)));
}

#[test]
fn subtasks_are_created_under_the_parent() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let parent = add_task(&mut a, list, "Parent");
    a.apply(Action::Add { list, name: "Child".into(), status: None, priority: 0, assignees: vec![], due: None, parent: Some(parent), open: false });
    assert_eq!(a.subtasks(parent).len(), 1);
    a.loc = Location::List(list);
    assert!(!a.filtered().iter().any(|t| t.name == "Child"), "subtasks stay out of the top level");
}

#[test]
fn delete_task_goes_to_trash_and_restores_with_subtasks() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let parent = add_task(&mut a, list, "Doomed");
    a.apply(Action::Add { list, name: "Doomed child".into(), status: None, priority: 0, assignees: vec![], due: None, parent: Some(parent), open: false });
    a.apply(Action::DeleteTask(parent));
    assert!(a.task(parent).is_none());
    assert!(!a.data.tasks.iter().any(|t| t.name == "Doomed child"));
    let i = a.data.extra.trash.iter().position(|x| x.title == "Doomed").expect("in trash");
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
    let i = a.data.extra.trash.iter().position(|x| x.kind == "List").expect("list in trash");
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
    a.apply(Action::NewList { space: sid, folder: None });
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
    assert!(a.task(ids[0]).unwrap().timer_started.is_none(), "starting another timer stops the first");
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
    let secs: i64 = a.task(id).unwrap().time_entries.iter().filter(|e| super::hubs::ts_day(e.start) == day).map(|e| e.end - e.start).sum();
    assert_eq!(secs, 4500);
    a.set_day_time(id, day, 0.0);
    assert!(!a.task(id).unwrap().time_entries.iter().any(|e| super::hubs::ts_day(e.start) == day));
}

#[test]
fn automations_run_on_status_change() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let other = list_ids(&a)[1];
    a.data.extra.automations.push(Automation { id: 1, enabled: true, trigger_status: "complete".into(), action: "assign".into(), value: "3".into(), list: None });
    a.data.extra.automations.push(Automation { id: 2, enabled: true, trigger_status: "complete".into(), action: "priority".into(), value: "4".into(), list: None });
    a.data.extra.automations.push(Automation { id: 3, enabled: true, trigger_status: "complete".into(), action: "comment".into(), value: "Nice work".into(), list: None });
    a.data.extra.automations.push(Automation { id: 4, enabled: true, trigger_status: "complete".into(), action: "move".into(), value: other.to_string(), list: None });
    a.data.extra.automations.push(Automation { id: 5, enabled: false, trigger_status: "complete".into(), action: "priority".into(), value: "1".into(), list: None });
    let id = add_task(&mut a, list, "Automate me");
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    let t = a.task(id).unwrap();
    assert!(t.assignees.contains(&3));
    assert_eq!(t.priority, 4, "the disabled rule must not run");
    assert!(t.activity.iter().any(|x| x.comment && x.text == "Nice work"));
    assert_eq!(t.list_id, other);
}

#[test]
fn templates_fill_the_new_task() {
    let mut a = app();
    let list = list_ids(&a)[0];
    a.pending_template = Some(TaskTemplate { id: 9, name: "Bug".into(), task_name: "Bug report".into(), description: "Steps".into(), checklist: vec!["Repro".into(), "Fix".into()], tags: vec!["bug".into()], priority: 2 });
    a.apply(Action::Add { list, name: "Bug report".into(), status: None, priority: 2, assignees: vec![], due: None, parent: None, open: false });
    let t = a.data.tasks.iter().find(|t| t.name == "Bug report").unwrap();
    assert_eq!(t.description, "Steps");
    assert_eq!(t.checklist.len(), 2);
    assert_eq!(t.tags, vec!["bug".to_string()]);
    assert_eq!(t.priority, 2);
}

#[test]
fn invite_adds_people_once_and_joins_general() {
    let mut a = app();
    let n = a.data.members.len();
    let added = a.invite_people(&["kim.minji@company.com".into(), "not-an-email".into(), "kim.minji@company.com".into()], "Guest");
    assert_eq!(added, 1);
    assert_eq!(a.data.members.len(), n + 1);
    let m = a.data.members.last().unwrap();
    assert_eq!(m.name, "Kim Minji");
    assert_eq!(m.role, "Guest");
    assert!(m.invited);
    let general = a.data.channels.iter().find(|c| c.name == "general").unwrap();
    assert!(general.members.contains(&m.id));
}

#[test]
fn chat_send_react_pin_thread_post_and_task() {
    let mut a = app();
    let cid = a.data.channels.iter().find(|c| c.name == "general").unwrap().id;
    let before = a.data.channels.iter().find(|c| c.id == cid).unwrap().messages.len();
    a.apply_chat(vec![ChatAction::Send { ch: cid, text: "Hello team".into(), parent: None, files: vec![] }]);
    let ch = a.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(ch.messages.len(), before + 1);
    let mid = ch.messages.last().unwrap().id;
    // reactions toggle
    a.apply_chat(vec![ChatAction::React { ch: cid, msg: mid, emoji: "\u{1F44D}".into() }]);
    let m = a.data.channels.iter().find(|c| c.id == cid).unwrap().messages.last().unwrap().clone();
    assert_eq!(m.reactions.len(), 1);
    a.apply_chat(vec![ChatAction::React { ch: cid, msg: mid, emoji: "\u{1F44D}".into() }]);
    assert!(a.data.channels.iter().find(|c| c.id == cid).unwrap().messages.last().unwrap().reactions.is_empty());
    // pin
    a.apply_chat(vec![ChatAction::Pin { ch: cid, msg: mid }]);
    assert!(a.data.channels.iter().find(|c| c.id == cid).unwrap().messages.last().unwrap().pinned);
    // thread reply
    a.apply_chat(vec![ChatAction::Send { ch: cid, text: "Reply".into(), parent: Some(mid), files: vec![] }]);
    assert_eq!(a.data.channels.iter().find(|c| c.id == cid).unwrap().messages.last().unwrap().replies.len(), 1);
    // post
    a.apply_chat(vec![ChatAction::SendPost { ch: cid, title: "Launch".into(), text: "We ship Friday".into(), files: vec![] }]);
    assert_eq!(a.data.channels.iter().find(|c| c.id == cid).unwrap().messages.last().unwrap().post_title.as_deref(), Some("Launch"));
    // message -> task
    let tasks = a.data.tasks.len();
    a.apply_chat(vec![ChatAction::Task { ch: cid, msg: mid }]);
    assert_eq!(a.data.tasks.len(), tasks + 1);
    let m = a.data.channels.iter().find(|c| c.id == cid).unwrap().messages.iter().find(|m| m.id == mid).unwrap().clone();
    assert!(m.task.is_some());
    assert_eq!(a.task(m.task.unwrap()).unwrap().name, "Hello team");
}

#[test]
fn chat_new_channel_and_dm_reuse() {
    let mut a = app();
    let n = a.data.channels.len();
    a.apply_chat(vec![ChatAction::NewChannel("Design Crit".into())]);
    assert_eq!(a.data.channels.len(), n + 1);
    assert_eq!(a.data.channels.last().unwrap().name, "design-crit");
    a.apply_chat(vec![ChatAction::Dm(4)]);
    let after_first = a.data.channels.len();
    a.apply_chat(vec![ChatAction::Dm(4)]);
    assert_eq!(a.data.channels.len(), after_first, "opening the same DM twice reuses it");
}

#[test]
fn csv_import_and_export() {
    let mut a = app();
    let dir = std::env::temp_dir().join(format!("wb-test-{}", now_ts()));
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
    assert!(!a.filtered().iter().any(|t| t.id == id), "others can't see my Personal List");
}

#[test]
fn today_and_overdue_filter() {
    let mut a = app();
    a.loc = Location::TodayOverdue;
    let r = a.filtered();
    assert!(!r.is_empty());
    assert!(r.iter().all(|t| t.due.map(|d| d <= today()).unwrap_or(false) && t.assignees.contains(&a.data.me)));
}

#[test]
fn due_date_groups_follow_the_rule() {
    let td = today();
    assert_eq!(due_group_key(None, false), "null");
    assert_eq!(due_group_key(Some(td), true), "done");
    assert_eq!(due_group_key(Some(td), false), "0");
    assert_eq!(due_group_key(Some(td - Duration::days(30)), false), "-1");
    assert_eq!(due_group_key(Some(td + Duration::days(3)), false), "3");
    assert_eq!(due_group_key(Some(td + Duration::days(40)), false), "+7");
}

#[test]
fn reminders_count_when_due() {
    let mut a = app();
    let me = a.data.me;
    a.data.extra.reminders.push(Reminder { id: 1, text: "Call".into(), due_ts: now_ts() - 60, done: false, owner: me });
    a.data.extra.reminders.push(Reminder { id: 2, text: "Later".into(), due_ts: now_ts() + 3600, done: false, owner: me });
    a.data.extra.reminders.push(Reminder { id: 3, text: "Done".into(), due_ts: now_ts() - 60, done: true, owner: me });
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
    a.data.extra.goals.push(Goal { id: 77, name: "Grow".into(), color: [1, 2, 3], due: None, owner: 1, targets: vec![GoalTarget { name: "Users".into(), kind: TargetKind::Number { start: 0.0, target: 10.0, current: 5.0, unit: String::new() } }] });
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
    let done_task = a.data.tasks.iter().find(|t| a.data.is_closed(t)).unwrap().id;
    let open_task = a.data.tasks.iter().find(|t| !a.data.is_closed(t)).unwrap().id;
    let g = Goal {
        id: 1,
        name: "G".into(),
        color: [0, 0, 0],
        due: None,
        owner: 1,
        targets: vec![
            GoalTarget { name: "n".into(), kind: TargetKind::Number { start: 0.0, target: 10.0, current: 5.0, unit: String::new() } },
            GoalTarget { name: "b".into(), kind: TargetKind::TrueFalse { done: true } },
            GoalTarget { name: "t".into(), kind: TargetKind::Tasks { ids: vec![done_task, open_task] } },
        ],
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
    assert!(a.toast.as_ref().map(|x| x.0.contains("complete")).unwrap_or(false));
}

#[test]
fn custom_field_values_are_kept_per_task() {
    let mut a = app();
    a.data.extra.fields.push(CustomField { id: 500, name: "Budget".into(), kind: "number".into(), options: vec![] });
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Costed");
    let mut t = a.task(id).unwrap().clone();
    t.custom.insert(500, "1200".into());
    assert!(task_changed(a.task(id).unwrap(), &t), "a custom field edit counts as a change");
    a.apply(Action::Replace(t));
    assert_eq!(a.task(id).unwrap().custom.get(&500).map(|s| s.as_str()), Some("1200"));
}

#[test]
fn workspaces_can_be_created_and_switched() {
    let home = std::env::temp_dir().join(format!("wb-home-{}", now_ts()));
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let mut a = app();
    a.persist = true;
    a.data.save();
    let owner = a.data.member(a.data.me).cloned().unwrap();
    let d = Data::blank("Side Biz", owner);
    d.save_file("workspace-test.json");
    Data::save_workspaces(&[WsEntry { name: "My Workspace".into(), file: "data.json".into() }, WsEntry { name: "Side Biz".into(), file: "workspace-test.json".into() }]);
    a.switch_workspace("workspace-test.json");
    assert_eq!(a.data.workspace, "Side Biz");
    // the switch menu says where each workspace lives, since names repeat
    assert_eq!(a.workspace_place("data.json"), "This computer");
    let mut team = Data::blank("My Workspace", a.data.member(a.data.me).cloned().unwrap());
    team.extra.shared = Some(crate::sync::SharedCfg { folder: String::new(), device: "d1".into(), tag: 3, server: Some("100.1.2.3".into()) });
    team.save_file("workspace-team.json");
    assert_eq!(a.workspace_place("workspace-team.json"), "Team server");
    assert_eq!(a.workspace_place("workspace-test.json"), "This computer", "the open one is read from memory");
    // the next start opens the workspace that was open last
    let reopened = App::build(None, None);
    assert_eq!(reopened.data.workspace, "Side Biz");
    assert_eq!(reopened.ws_file, "workspace-test.json");
    a.switch_workspace("data.json");
    assert_eq!(a.data.workspace, "My Workspace");
}

fn temp_folder(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("wb-{}-{:x}", tag, crate::sync::random_u64()))
}

#[test]
fn two_computers_work_in_one_shared_folder() {
    let dir = temp_folder("share");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();

    // someone else joins from their own computer
    let mut b = App::build(None, Some(Data::blank("Theirs", Member { id: 1, name: "B".into(), ..Default::default() })));
    b.join_shared(&folder).unwrap();
    assert_eq!(b.data.tasks.len(), a.data.tasks.len(), "the joiner gets the team's tasks");
    assert!(b.data.member(b.data.me).is_none(), "the joiner still has to pick who they are");
    b.become_member(None, "Bora", "bora@team.kr");
    let bora = b.data.me;

    // both work at the same time
    let ch = b.data.channels.iter().find(|c| c.name == "general").unwrap().id;
    b.apply_chat(vec![ChatAction::Send { ch, text: "hi from Bora".into(), parent: None, files: vec![] }]);
    b.sync_note_change();
    b.write_shared();
    let list = a.data.first_list().unwrap();
    a.apply(Action::Add { list, name: "Made by A".into(), status: None, priority: 0, assignees: vec![], due: None, parent: None, open: false });
    a.sync_note_change();
    a.write_shared();

    assert!(a.sync_pull(), "A gets B's changes");
    assert!(a.data.member(bora).is_some(), "the new member shows up for A");
    let msgs = &a.data.channels.iter().find(|c| c.id == ch).unwrap().messages;
    assert_eq!(msgs.last().unwrap().text, "hi from Bora");
    assert!(b.sync_pull(), "B gets A's task");
    assert!(b.data.tasks.iter().any(|t| t.name == "Made by A"));

    // a delete on one computer reaches the other and doesn't come back
    let tid = b.data.tasks.iter().find(|t| t.name == "Made by A").unwrap().id;
    b.apply(Action::DeleteTask(tid));
    b.sync_note_change();
    b.write_shared();
    a.sync_pull();
    assert!(a.task(tid).is_none(), "deleted for everyone");
    a.sync_note_change();
    a.write_shared();
    b.sync_pull();
    assert!(b.task(tid).is_none(), "and it stays deleted");

    // ids made on the two computers never clash
    assert_ne!(a.data.new_id() % 1024, b.data.new_id() % 1024);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn starting_a_call_posts_a_join_link() {
    let mut a = app();
    let ch = a.data.channels[0].id;
    let link = a.start_call(None, ch, "");
    assert!(link.starts_with("https://meet.jit.si/Workboard-"));
    let m = a.data.channels[0].messages.last().unwrap();
    assert_eq!(m.call.as_deref(), Some(link.as_str()));
    assert_eq!(a.section, Section::Chat);
    let own = a.start_call(None, ch, "https://zoom.us/j/123");
    assert_eq!(own, "https://zoom.us/j/123", "your own meeting link is used as given");
}

#[test]
fn archiving_a_doc_saves_a_markdown_copy() {
    let dir = temp_folder("arch");
    let mut a = app();
    a.share_here(dir.to_str().unwrap()).unwrap();
    let id = a.data.docs[0].id;
    a.toggle_archive(id);
    assert!(a.data.docs[0].archived);
    let files: Vec<_> = std::fs::read_dir(dir.join("Docs archive")).unwrap().flatten().collect();
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
    d.tasks.push(Task { id: mine, name: "My real task".into(), list_id: web, assignees: vec![1, 2], ..d.tasks[0].clone() });
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
    assert_eq!(d.tasks[0].assignees, vec![1], "removed people are unassigned");
    assert!(d.list_name(web) == "Website Redesign", "a list holding my task stays");
    assert_eq!(d.spaces.len(), 1, "example spaces without my work are gone");
    assert!(d.docs.is_empty());
    let ch: Vec<_> = d.channels.iter().map(|c| (c.name.clone(), c.messages.len())).collect();
    assert_eq!(ch, vec![("general".to_string(), 1)], "only my message is left");
    assert!(d.extra.goals.is_empty() && d.extra.whiteboards.is_empty() && d.extra.dashboards.is_empty() && d.extra.teams.is_empty());
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
    assert!(!old.extra.demo_removed, "files from before the cleanup get cleaned");
    assert!(Data::fresh().tasks.is_empty());
}

// ---------------------------------------------------------------- parity features

fn other_list(a: &App, not: u64) -> u64 {
    list_ids(a).into_iter().find(|l| *l != not).unwrap()
}

#[test]
fn move_task_takes_subtasks_and_keeps_a_valid_status() {
    let mut a = app();
    let from = list_ids(&a)[0];
    let to = other_list(&a, from);
    let id = add_task(&mut a, from, "Movable");
    a.apply(Action::Add { list: from, name: "Child".into(), status: None, priority: 0, assignees: vec![], due: None, parent: Some(id), open: false });
    // a status the target list does not have falls back to its first status
    a.data.tasks.iter_mut().find(|t| t.id == id).unwrap().status = "made up".into();
    a.move_tasks(&[id], to);
    let t = a.task(id).unwrap();
    assert_eq!(t.list_id, to);
    assert_eq!(t.status, "to do");
    assert!(t.activity.iter().any(|x| x.text.starts_with("moved this task to")));
    let child = a.data.tasks.iter().find(|t| t.name == "Child").unwrap();
    assert_eq!(child.list_id, to, "subtasks move with their task");
}

#[test]
fn duplicate_task_copies_subtasks_under_the_copy() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Original");
    a.apply(Action::Add { list, name: "Step".into(), status: None, priority: 0, assignees: vec![], due: None, parent: Some(id), open: false });
    let copy = a.duplicate_task(id).unwrap();
    assert_eq!(a.task(copy).unwrap().name, "Original (copy)");
    assert_eq!(a.subtasks(copy).len(), 1);
    assert_eq!(a.subtasks(id).len(), 1, "the original keeps its subtask");
}

#[test]
fn archived_tasks_leave_views_until_the_archived_filter() {
    let mut a = app();
    let list = list_ids(&a)[0];
    a.loc = Location::List(list);
    let id = add_task(&mut a, list, "Old work");
    a.set_archived(&[id], true);
    assert!(!a.filtered().iter().any(|t| t.id == id));
    a.filter.archived = true;
    assert!(a.filtered().iter().any(|t| t.id == id));
    a.filter.archived = false;
    a.set_archived(&[id], false);
    assert!(a.filtered().iter().any(|t| t.id == id));
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
    t.checklist = vec![CheckItem { text: "send".into(), done: true }];
    a.apply(Action::Replace(t));
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    let t = a.task(id).unwrap();
    assert_eq!(t.status, "to do");
    assert_eq!(t.due, Some(due + Duration::days(7)));
    assert!(!t.checklist[0].done);
    assert_eq!(next_recur(NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(), "monthly"), NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
}

#[test]
fn filter_conditions_status_priority_due_and_sort_direction() {
    let mut a = app();
    a.loc = Location::Everything;
    let all = a.filtered().len();
    a.filter.priorities = vec![1];
    assert!(a.filtered().iter().all(|t| t.priority == 1));
    a.filter = Default::default();
    a.filter.statuses = vec!["in progress".into()];
    assert!(!a.filtered().is_empty());
    assert!(a.filtered().iter().all(|t| t.status == "in progress"));
    a.filter = Default::default();
    a.filter.due = 1;
    assert!(a.filtered().iter().all(|t| t.due.map_or(false, |d| d < today())));
    a.filter.due = 5;
    assert!(a.filtered().iter().all(|t| t.due.is_none()));
    a.filter = Default::default();
    assert_eq!(a.filtered().len(), all);
    a.sort_by = SortBy::Name;
    let asc: Vec<u64> = a.filtered().iter().map(|t| t.id).collect();
    a.sort_desc = true;
    let mut desc: Vec<u64> = a.filtered().iter().map(|t| t.id).collect();
    desc.reverse();
    assert_eq!(asc, desc);
}

#[test]
fn lists_move_and_duplicate_with_their_tasks() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let n = a.data.tasks.iter().filter(|t| t.list_id == list && t.parent.is_none()).count();
    let target = a.data.spaces.iter().find(|s| a.data.space_of_list(list).map(|x| x.id) != Some(s.id)).unwrap().id;
    a.move_list(list, target, None);
    assert_eq!(a.data.space_of_list(list).unwrap().id, target);
    let copy = a.duplicate_list(list).unwrap();
    assert_eq!(a.data.list_name(copy), format!("{} (copy)", a.data.list_name(list)));
    assert_eq!(a.data.tasks.iter().filter(|t| t.list_id == copy && t.parent.is_none()).count(), n);
}

#[test]
fn new_list_from_the_personal_list_goes_to_a_visible_space() {
    let mut a = app();
    let pl = a.create_personal_list();
    a.loc = Location::List(pl);
    a.open_new_list(a.current_space(), None);
    let Some(Modal::NewList { space, .. }) = a.modal.take() else { panic!("Create List dialog") };
    assert!(a.data.spaces.iter().any(|s| s.id == space && s.private_owner.is_none()));
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
    assert_eq!(a.data.docs.iter().filter(|d| d.parent.is_none() && (d.id == page || d.id == sub)).count(), 0);
}

#[test]
fn doc_formatting_adds_markdown_at_the_cursor() {
    let mut s = String::from("Hello\nworld");
    super::docs::apply_block(&mut s, 7, 7, "Heading 2");
    assert_eq!(s, "Hello\n## world");
    super::docs::apply_block(&mut s, 7, 7, "Checklist");
    assert_eq!(s, "Hello\n- [ ] world", "a new block replaces the old prefix");
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
struct TestServer {
    port: u16,
    home: std::path::PathBuf,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    fn start() -> TestServer {
        let home = temp_folder("server");
        let h = home.clone();
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                port_tx.send(listener.local_addr().unwrap().port()).unwrap();
                workboard_server::serve(listener, h, async {
                    let _ = stop_rx.await;
                })
                .await
                .unwrap();
            });
        });
        let port = port_rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        TestServer { port, home, stop: Some(stop_tx), thread: Some(thread) }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

/// Keep taking in what the server sends until `cond` holds.
fn net_until(a: &mut App, what: &str, cond: impl Fn(&App) -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < until {
        a.net_tick(None);
        if cond(a) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("timed out waiting for: {} ({:?})", what, a.sync_rt.net_status);
}

#[test]
fn two_computers_work_through_a_server() {
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);

    // the first app to connect brings its workspace to the empty server
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A shared on the server", |a| a.data.extra.shared.is_some());
    let cfg = a.data.extra.shared.clone().unwrap();
    assert_eq!(cfg.server.as_deref(), Some(addr.as_str()));
    assert!(cfg.folder.is_empty());
    // wait until the server has saved A's copy, so B finds it there
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !server.home.join(workboard_server::WORKSPACE_FILE).is_file() && std::time::Instant::now() < until {
        a.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // A is the owner and makes an invite code for Sam (already a member)
    assert_eq!(a.sync_rt.role, Some(wb_net::Role::Owner));
    net_until(&mut a, "A sees the people list", |a| a.sync_rt.people.iter().any(|p| p.member == 3));
    a.access_request(wb_net::Msg::CreateInvite { member: 3, role: wb_net::Role::Member });
    net_until(&mut a, "invite code", |a| a.sync_rt.people.iter().any(|p| p.member == 3 && p.invite.is_some()));
    let code = a.sync_rt.people.iter().find(|p| p.member == 3).unwrap().invite.clone().unwrap();

    // without a code nobody gets in
    let mut x = App::build(None, Some(Data::blank("Nope", Member { id: 1, name: "X".into(), ..Default::default() })));
    x.connect_server(&addr, "").unwrap();
    net_until(&mut x, "refused", |x| matches!(x.sync_rt.net_status, Some(wb_net::Status::Refused(_))));
    assert!(x.data.extra.shared.is_none(), "nothing changes for them");

    // Sam joins from their own computer with the code
    let mut b = App::build(None, Some(Data::blank("Theirs", Member { id: 1, name: "B".into(), ..Default::default() })));
    b.connect_server(&addr, &code.to_lowercase()).unwrap();
    net_until(&mut b, "B joined", |b| b.data.extra.shared.is_some());
    assert_eq!(b.data.tasks.len(), a.data.tasks.len(), "the joiner gets the team's tasks");
    assert_eq!(b.data.me, 3, "the code says who they are");
    assert!(!matches!(b.modal, Some(Modal::WhoAmI { .. })), "no picking who you are on a server");
    assert_eq!(b.sync_rt.role, Some(wb_net::Role::Member));
    assert_ne!(b.data.extra.shared.as_ref().unwrap().tag, cfg.tag, "the server hands out different id tags");
    let cfg_b = b.data.extra.shared.clone().unwrap();
    let token = together::load_token(&addr, &cfg_b.device).expect("the token is kept");
    assert!(!serde_json::to_string(&b.data).unwrap().contains(&token), "but not in data.json");
    let bora = b.data.me;

    // chat from B reaches A
    let ch = b.data.channels.iter().find(|c| c.name == "general").unwrap().id;
    b.apply_chat(vec![ChatAction::Send { ch, text: "hi through the server".into(), parent: None, files: vec![] }]);
    b.sync_note_change();
    b.write_shared();
    net_until(&mut a, "B's message at A", |a| a.data.channels.iter().find(|c| c.id == ch).is_some_and(|c| c.messages.last().is_some_and(|m| m.text == "hi through the server")));
    assert!(a.data.member(bora).is_some(), "the new member shows up for A");

    // a task from A reaches B
    let list = a.data.first_list().unwrap();
    a.apply(Action::Add { list, name: "Made by A".into(), status: None, priority: 0, assignees: vec![], due: None, parent: None, open: false });
    a.sync_note_change();
    a.write_shared();
    net_until(&mut b, "A's task at B", |b| b.data.tasks.iter().any(|t| t.name == "Made by A"));

    // the owner removes Sam's access: Sam is signed out and can sign in again with a new code
    a.access_request(wb_net::Msg::Revoke { member: 3 });
    net_until(&mut b, "B signed out", |b| matches!(b.sync_rt.net_status, Some(wb_net::Status::Refused(_))));
    a.access_request(wb_net::Msg::CreateInvite { member: 3, role: wb_net::Role::Member });
    net_until(&mut a, "new code", |a| a.sync_rt.people.iter().any(|p| p.member == 3 && p.invite.is_some()));
    let code = a.sync_rt.people.iter().find(|p| p.member == 3).unwrap().invite.clone().unwrap();
    b.sign_in_again(&code);
    net_until(&mut b, "B back in", |b| b.sync_rt.net_status == Some(wb_net::Status::Connected));

    // stopping sharing ends the connection and forgets the token
    let dev = a.data.extra.shared.as_ref().unwrap().device.clone();
    a.stop_sharing();
    assert!(a.sync_rt.net.is_none() && a.data.extra.shared.is_none());
    assert!(together::load_token(&addr, &dev).is_none());
}

#[test]
fn a_bad_server_address_is_explained() {
    let mut a = app();
    assert!(a.connect_server("", "").is_err());
    assert!(a.connect_server("wss://example", "").is_err());
    assert!(a.sync_rt.joining.is_none());
}

#[test]
fn old_shared_settings_still_load() {
    let old = r#"{"folder":"/tmp/x","device":"abc","tag":7}"#;
    let cfg: crate::sync::SharedCfg = serde_json::from_str(old).unwrap();
    assert_eq!(cfg.server, None);
    assert_eq!(cfg.folder, "/tmp/x");
    assert!(!serde_json::to_string(&cfg).unwrap().contains("server"), "folder settings are saved as before");
}

/// A local livekit-server for huddle tests (None when it isn't installed).
struct TestLiveKit(std::process::Child, u16);

impl Drop for TestLiveKit {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

const LK_SECRET: &str = "app-test-secret-app-test-secret-0123456";

fn test_livekit() -> Option<TestLiveKit> {
    use std::process::{Command, Stdio};
    let bin = ["livekit-server", "/opt/homebrew/bin/livekit-server", "/usr/local/bin/livekit-server"].into_iter().find(|b| Command::new(b).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success()))?;
    let free = || std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let (port, rtc) = (free(), free());
    let config = format!("port: {port}\nbind_addresses: [\"127.0.0.1\"]\nrtc:\n  tcp_port: {rtc}\n  udp_port: {rtc}\n  use_external_ip: false\n  node_ip: 127.0.0.1\nkeys:\n  appkey: {LK_SECRET}\nlogging:\n  level: warn\n");
    let child = Command::new(bin).arg("--config-body").arg(config).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok()?;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(std::time::Instant::now() < until, "livekit-server didn't start");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Some(TestLiveKit(child, port))
}

/// Keep both apps' connections and calls going until `cond` holds.
fn huddle_until(a: &mut App, b: &mut App, what: &str, cond: impl Fn(&App, &App) -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(25);
    while std::time::Instant::now() < until {
        for x in [&mut *a, &mut *b] {
            x.net_tick(None);
            x.huddle_tick(None);
        }
        if cond(a, b) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    panic!("timed out waiting for: {} (A {:?} / B {:?})", what, a.huddle.people, b.huddle.people);
}

// WebRTC opens UDP on every network, so macOS asks about incoming connections
// for each freshly built test binary. Run it on purpose: cargo test -- --ignored
#[test]
#[ignore = "opens WebRTC sockets; run with --ignored"]
fn a_huddle_on_the_server_connects_two_computers() {
    let Some(lk) = test_livekit() else {
        eprintln!("skipped: livekit-server isn't installed");
        return;
    };
    let server = TestServer::start();
    // the server makes its folder when it starts serving; don't race it
    std::fs::create_dir_all(&server.home).unwrap();
    std::fs::write(server.home.join(workboard_server::LIVEKIT_FILE), serde_json::json!({"url": format!("ws://127.0.0.1:{}", lk.1), "key": "appkey", "secret": LK_SECRET}).to_string()).unwrap();
    let addr = format!("127.0.0.1:{}", server.port);

    // A sets the server up, Sam (member 3) joins with a code
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A on the server", |a| a.data.extra.shared.is_some() && a.sync_rt.people.iter().any(|p| p.member == 3));
    a.access_request(wb_net::Msg::CreateInvite { member: 3, role: wb_net::Role::Member });
    net_until(&mut a, "code", |a| a.sync_rt.people.iter().any(|p| p.member == 3 && p.invite.is_some()));
    let code = a.sync_rt.people.iter().find(|p| p.member == 3).unwrap().invite.clone().unwrap();
    let mut b = App::build(None, Some(Data::blank("Theirs", Member { id: 1, name: "B".into(), ..Default::default() })));
    b.connect_server(&addr, &code).unwrap();
    net_until(&mut b, "B joined", |b| b.data.extra.shared.is_some());
    b.sync_note_change();
    b.write_shared();
    let ch = a.data.channels.iter().find(|c| c.name == "general").unwrap().id;
    net_until(&mut a, "Sam is in #general on the server", |a| a.data.channels.iter().any(|c| c.id == ch && c.members.contains(&3)));

    // A presses Huddle in #general: a huddle message goes out and A joins
    a.section = Section::Chat;
    a.chat_open = Some(ch);
    a.open_call_dialog();
    assert!(a.modal.is_none(), "no browser-call dialog on the server");
    assert_eq!(a.huddle.channel, Some(ch));
    let msg_is_huddle = |x: &App| x.data.channels.iter().find(|c| c.id == ch).and_then(|c| c.messages.last()).and_then(|m| m.call.as_deref()).and_then(huddle::huddle_channel) == Some(ch);
    // the card goes out once the server hands A the call token
    net_until(&mut a, "A's huddle card", msg_is_huddle);
    a.sync_note_change();
    a.write_shared();
    net_until(&mut b, "B sees the huddle message", |b| msg_is_huddle(b));

    // B presses Join on it; both hear each other (a tone in tests)
    b.apply_chat(vec![ChatAction::JoinHuddle(ch)]);
    huddle_until(&mut a, &mut b, "both in the huddle and heard", |a, b| {
        let heard = |x: &App| x.huddle.people.len() == 2 && x.huddle.people.iter().any(|p| !p.me && p.heard > 0.02);
        heard(a) && heard(b)
    });
    let sam = a.huddle.people.iter().find(|p| !p.me).unwrap();
    assert_eq!(sam.member(), Some(3), "A sees Sam");

    // A turns on video (a red test picture): B sees it, and A sees itself
    a.set_huddle_camera(true);
    let a_identity = b.huddle.people.iter().find(|p| !p.me).unwrap().identity.clone();
    huddle_until(&mut a, &mut b, "B sees A's video", |a, b| {
        let red = |p: &wb_voice::Picture| p.average()[0] > 150 && p.average()[1] < 100;
        b.huddle.pictures.get(&a_identity).is_some_and(|(p, _)| red(p)) && a.huddle.pictures.get(wb_voice::ME).is_some_and(|(p, _)| red(p))
    });
    a.set_huddle_camera(false);
    huddle_until(&mut a, &mut b, "A's video is gone at B", |a, b| !b.huddle.pictures.contains_key(&a_identity) && !a.huddle.pictures.contains_key(wb_voice::ME));

    // B mutes, then leaves
    b.set_huddle_muted(true);
    huddle_until(&mut a, &mut b, "A sees B muted", |a, _| a.huddle.people.iter().any(|p| !p.me && p.muted));
    b.leave_huddle();
    assert!(b.huddle.channel.is_none() && b.huddle.call.is_none());
    huddle_until(&mut a, &mut b, "A alone", |a, _| a.huddle.people.len() == 1);
    a.stop_sharing();
    assert!(a.huddle.channel.is_none(), "stopping sharing ends the huddle");
}

#[test]
fn without_a_server_the_huddle_button_still_opens_a_browser_call() {
    let mut a = app();
    let ch = a.data.channels[0].id;
    a.section = Section::Chat;
    a.chat_open = Some(ch);
    a.open_call_dialog();
    assert!(matches!(a.modal, Some(Modal::Call { .. })));
    assert!(a.huddle.channel.is_none());
    a.join_huddle(ch);
    assert!(a.huddle.channel.is_none(), "can't join an in-app huddle without the server");
}

fn picture_file(name: &str, w: u32, h: u32) -> std::path::PathBuf {
    let dir = temp_folder("pic");
    std::fs::create_dir_all(&dir).unwrap();
    let mut img = image::RgbaImage::new(w, h);
    for (x, _, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([if x < w / 2 { 200 } else { 30 }, 90, 40, 255]);
    }
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

/// Tick until the message box's attachments are ready (or fail the test).
fn until_ready(a: &mut App, thread: bool) -> Vec<Attachment> {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while std::time::Instant::now() < end {
        a.files_tick(None);
        a.net_tick(None);
        let (ok, files) = a.pending_ready(thread);
        let list = if thread { &a.thread_files } else { &a.chat_files };
        if let Some(e) = list.iter().find_map(|p| p.error.clone()) {
            panic!("attachment refused: {}", e);
        }
        if ok && !files.is_empty() {
            return files;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("attachments never got ready: {:?}", a.chat_files);
}

#[test]
fn a_picture_goes_through_the_server_to_the_other_computer() {
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A shared on the server", |a| a.data.extra.shared.is_some());
    net_until(&mut a, "A sees the people list", |a| a.sync_rt.people.iter().any(|p| p.member == 3));
    a.access_request(wb_net::Msg::CreateInvite { member: 3, role: wb_net::Role::Member });
    net_until(&mut a, "invite code", |a| a.sync_rt.people.iter().any(|p| p.member == 3 && p.invite.is_some()));
    let code = a.sync_rt.people.iter().find(|p| p.member == 3).unwrap().invite.clone().unwrap();
    let mut b = App::build(None, Some(Data::blank("Theirs", Member { id: 1, name: "B".into(), ..Default::default() })));
    b.connect_server(&addr, &code).unwrap();
    net_until(&mut b, "B joined", |b| b.data.extra.shared.is_some());

    // A puts a picture in the message box: it's checked, then sent to the server
    let ch = a.data.channels.iter().find(|c| c.name == "general").unwrap().id;
    let pic = picture_file("sunset.png", 320, 200);
    a.add_media(false, Some(pic.clone()), None);
    let files = until_ready(&mut a, false);
    assert_eq!(files.len(), 1);
    let att = files[0].clone();
    assert_eq!((att.name.as_str(), att.kind, att.w, att.h), ("sunset.png", MediaKind::Image, 320, 200));
    assert_eq!(att.bytes, std::fs::metadata(&pic).unwrap().len());
    assert_eq!(std::fs::read(server.home.join(workboard_server::FILES_DIR).join(&att.file)).unwrap(), std::fs::read(&pic).unwrap(), "on the server under its hash");
    a.apply_chat(vec![ChatAction::Send { ch, text: "look at this".into(), parent: None, files }]);
    a.sync_note_change();
    a.write_shared();

    // B gets the message, and the picture when it comes on screen
    net_until(&mut b, "A's picture message at B", |b| b.data.channels.iter().any(|c| c.messages.iter().any(|m| m.text == "look at this" && m.attachments.len() == 1)));
    let m = b.data.channels.iter().flat_map(|c| c.messages.iter()).find(|m| m.text == "look at this").unwrap().clone();
    assert_eq!(m.attachments[0], att, "the same attachment details");
    assert!(b.files.local(&att.file).is_none(), "not fetched before it's shown");
    let remote = b.media_remote();
    assert!(matches!(remote, super::files::Remote::Server { .. }));
    b.files.want(&att, super::files::THUMB);
    let sha = att.file.clone();
    // the screen asks again every frame while the message is visible
    b.files.settle(&remote, |f| {
        f.want(&att, super::files::THUMB);
        f.local(&sha).is_some() && f.decoded(&sha, super::files::THUMB)
    });
    assert_eq!(std::fs::read(b.files.local(&att.file).unwrap()).unwrap(), std::fs::read(&pic).unwrap());
}

#[test]
fn attachments_wait_with_their_channel_and_only_media_goes() {
    let mut a = app();
    let chans: Vec<u64> = a.data.channels.iter().map(|c| c.id).take(2).collect();
    a.section = Section::Chat;
    a.chat_open = Some(chans[0]);
    a.drafts.chat_files.swap(a.chat_open, &mut a.chat_files);
    // no server, no folder: the picture is ready once it's in the cache
    assert_eq!(a.media_remote(), super::files::Remote::None);
    a.add_media(false, Some(picture_file("one.png", 50, 40)), None);
    let files = until_ready(&mut a, false);
    assert!(a.files.local(&files[0].file).is_some());
    // another channel has its own (empty) box; coming back brings the picture back
    a.chat_open = Some(chans[1]);
    a.drafts.chat_files.swap(a.chat_open, &mut a.chat_files);
    assert!(a.chat_files.is_empty());
    a.chat_open = Some(chans[0]);
    a.drafts.chat_files.swap(a.chat_open, &mut a.chat_files);
    assert_eq!(a.chat_files.len(), 1);
    assert_eq!(a.chat_files[0].att.as_ref().map(|x| x.file.clone()), Some(files[0].file.clone()));
    // a pasted screenshot arrives as PNG bytes
    let png = std::fs::read(picture_file("shot.png", 30, 20)).unwrap();
    a.add_media(false, None, Some(png));
    let files = until_ready(&mut a, false);
    assert_eq!(files.len(), 2);
    assert_eq!((files[1].name.as_str(), files[1].w, files[1].h), ("Pasted image.png", 30, 20));
    // a thread reply can carry a picture too
    a.apply_chat(vec![ChatAction::Send { ch: chans[0], text: String::new(), parent: None, files: files.clone() }]);
    let mid = a.data.channels.iter().find(|c| c.id == chans[0]).unwrap().messages.last().unwrap().id;
    a.apply_chat(vec![ChatAction::Send { ch: chans[0], text: "reply".into(), parent: Some(mid), files: files[..1].to_vec() }]);
    let m = a.data.channels.iter().find(|c| c.id == chans[0]).unwrap().messages.last().unwrap().clone();
    assert_eq!(m.attachments.len(), 2);
    assert_eq!(m.replies[0].attachments.len(), 1);
    // messages without attachments don't grow a field in data.json
    let json = serde_json::to_value(&a.data).unwrap();
    let plain = json["channels"].as_array().unwrap().iter().flat_map(|c| c["messages"].as_array().unwrap().iter()).find(|m| m["attachments"].is_null());
    assert!(plain.is_some());
    // things that aren't pictures or videos are refused
    let doc = temp_folder("doc").join("plan.pdf");
    std::fs::create_dir_all(doc.parent().unwrap()).unwrap();
    std::fs::write(&doc, b"%PDF").unwrap();
    a.chat_files.clear();
    a.add_media(false, Some(doc), None);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while a.chat_files[0].error.is_none() && std::time::Instant::now() < end {
        a.files_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(a.chat_files[0].error.as_deref().unwrap().contains("isn't a picture or a video"));
}

#[test]
fn in_a_shared_folder_pictures_go_into_its_files_folder() {
    let folder = temp_folder("shared-files");
    let mut a = app();
    a.share_here(&folder.to_string_lossy()).unwrap();
    assert_eq!(a.media_remote(), super::files::Remote::Folder(folder.clone()));
    a.add_media(false, Some(picture_file("team.png", 40, 40)), None);
    let files = until_ready(&mut a, false);
    assert!(folder.join("files").join(&files[0].file).is_file(), "copied into the shared folder");
    // the folder sync ignores it (only *.workboard.json files are read)
    assert!(crate::sync::other_files(&folder, "nobody").iter().all(|(p, _)| p.extension().is_some_and(|e| e == "json")));
    // another computer copies it from there into its own cache
    let b = App::build(None, Some(Data::seed()));
    let mut bf = super::files::Files::new(temp_folder("b-cache"));
    let remote = super::files::Remote::Folder(folder.clone());
    bf.want(&files[0], super::files::THUMB);
    let sha = files[0].file.clone();
    bf.settle(&remote, |f| f.local(&sha).is_some());
    drop(b);
}

#[test]
fn a_video_shows_a_still_from_the_cache() {
    let Some(ffmpeg) = ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"].into_iter().find(|p| std::path::Path::new(p).exists()) else {
        eprintln!("skipped: ffmpeg isn't installed");
        return;
    };
    let dir = temp_folder("video");
    std::fs::create_dir_all(&dir).unwrap();
    let clip = dir.join("Holiday.MP4");
    let ok = std::process::Command::new(ffmpeg).args(["-loglevel", "error", "-f", "lavfi", "-i", "testsrc=duration=2:size=640x360:rate=30", "-pix_fmt", "yuv420p", "-c:v", "libx264"]).arg(&clip).output().unwrap().status.success();
    assert!(ok);
    let mut a = app();
    a.add_media(false, Some(clip), None);
    let files = until_ready(&mut a, false);
    let v = files[0].clone();
    assert_eq!((v.kind, v.w, v.h), (MediaKind::Video, 640, 360));
    assert!((v.secs - 2.0).abs() < 0.1, "{}", v.secs);
    // the cache names files by hash alone; the still must come out anyway
    let remote = a.media_remote();
    let sha = v.file.clone();
    a.files.settle(&remote, |f| {
        f.want(&v, super::files::THUMB);
        f.decoded(&sha, super::files::THUMB) || f.broken(&sha, super::files::THUMB)
    });
    assert!(a.files.decoded(&sha, super::files::THUMB), "a still from the video");
    assert!(a.files.playable(&v).unwrap().to_string_lossy().ends_with(".mp4"));
}

#[test]
fn one_message_takes_at_most_ten_pictures() {
    let mut a = app();
    let pic = picture_file("many.png", 20, 20);
    // twelve chosen at once: ten go in, the rest are left out with a note
    let added: Vec<bool> = (0..12).map(|_| a.add_media(false, Some(pic.clone()), None)).collect();
    assert_eq!(added.iter().filter(|x| **x).count(), 10);
    assert_eq!(a.chat_files.len(), super::files::MAX_PER_MESSAGE);
    assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains("up to 10")));
    // the thread's box counts on its own
    assert!(a.add_media(true, Some(pic.clone()), None));
    // removing one makes room again
    a.chat_files.pop();
    assert!(a.add_media(false, Some(pic), None));
    assert_eq!(a.chat_files.len(), 10);
}

#[test]
fn a_huddle_join_gives_up_after_a_while_and_only_its_own_denial_cancels_it() {
    let mut a = app();
    let ch = a.data.channels[0].id;
    let other = a.data.channels.get(1).map(|c| c.id).unwrap_or(ch + 1);
    let waiting = |asked: std::time::Instant| {
        let mut h = huddle::HuddleRt::default();
        (h.channel, h.waiting, h.asked, h.announce) = (Some(ch), true, Some(asked), true);
        h
    };
    let cards = |a: &App| a.data.channels[0].messages.iter().filter(|m| m.call.as_deref().and_then(huddle::huddle_channel) == Some(ch)).count();

    // an unrelated refusal (a backup, an invite, another conversation) leaves it waiting
    a.huddle = waiting(std::time::Instant::now());
    a.huddle_denied(other);
    assert!(a.huddle.waiting && a.huddle.channel == Some(ch));
    // its own refusal ends it, and no "Started a huddle" card was posted
    a.huddle_denied(ch);
    assert!(a.huddle.channel.is_none());
    assert_eq!(cards(&a), 0);

    // no answer for HUDDLE_JOIN_SECS: give up and say so
    a.huddle = waiting(std::time::Instant::now() - std::time::Duration::from_secs(huddle::HUDDLE_JOIN_SECS + 1));
    a.toast = None;
    a.huddle_tick(None);
    assert!(a.huddle.channel.is_none() && !a.huddle.waiting);
    assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains("didn't answer")), "{:?}", a.toast);
    assert_eq!(cards(&a), 0);

    // the token came: now the card goes out, once
    a.huddle = waiting(std::time::Instant::now());
    a.huddle_token(ch, "ws://x".into(), "t".into());
    assert_eq!(cards(&a), 1);
    assert!(!a.huddle.waiting && a.huddle.token.is_some());
    a.huddle_token(ch, "ws://x".into(), "t".into());
    assert_eq!(cards(&a), 1);
}

#[test]
fn team_settings_and_moved_lists_reach_everyone_once() {
    let dir = temp_folder("move");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.data.extra.accent = "pink".into();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(Data::blank("Theirs", Member { id: 1, name: "B".into(), ..Default::default() })));
    b.data.extra.accent = "teal".into();
    b.join_shared(&folder).unwrap();
    assert_eq!(b.data.extra.accent, "teal", "the joiner keeps their own theme");
    assert_eq!(b.data.workspace, a.data.workspace);
    b.become_member(None, "Bora", "bora@team.kr");
    b.sync_note_change();
    b.write_shared();
    a.sync_pull();

    // A adds a second space, moves a list there and renames the workspace
    let sid = a.data.new_id();
    a.data.spaces.push(Space { id: sid, name: "Other".into(), statuses: default_statuses(), ..Default::default() });
    a.dirty = true;
    a.sync_note_change();
    a.write_shared();
    b.sync_pull();
    let list = a.data.first_list().unwrap();
    a.move_list(list, sid, None);
    a.data.workspace = "Renamed".into();
    a.data.extra.apps.priorities = false;
    a.sync_note_change();
    a.write_shared();
    // B, not having seen that yet, writes its stale copy
    b.sync_note_change();
    b.write_shared();
    b.sync_pull();
    a.sync_pull();
    for d in [&a.data, &b.data] {
        let homes = d.spaces.iter().flat_map(|s| s.lists.iter().chain(s.folders.iter().flat_map(|f| f.lists.iter()))).filter(|l| l.id == list).count();
        assert_eq!(homes, 1, "the moved list is in one place");
        assert!(d.spaces.iter().find(|s| s.id == sid).unwrap().lists.iter().any(|l| l.id == list));
        assert_eq!(d.workspace, "Renamed");
        assert!(!d.extra.apps.priorities);
    }
    assert_eq!(b.data.extra.accent, "teal", "theme stays personal");

    // a copy this app can't read is reported once, not on every update
    let mut bad = serde_json::to_value(&b.data).unwrap();
    // a newer app changed what a setting looks like
    bad["workspace"] = serde_json::json!({"name": "Renamed"});
    bad["extra"]["sync"]["values"]["workspace"] = serde_json::json!(i64::MAX / 2);
    std::fs::write(dir.join(format!("zz{}", crate::sync::FILE_SUFFIX)), bad.to_string()).unwrap();
    a.toast = None;
    assert!(!a.sync_pull());
    assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains("newer version")));
    a.toast = None;
    a.sync_pull();
    assert!(a.toast.is_none());
    let _ = std::fs::remove_dir_all(&dir);
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
    let i = a.data.extra.trash.iter().position(|x| x.kind == "List" && x.title == "Website Redesign").expect("folder's list in trash");
    // the folder is gone, so it comes back in its space and says so
    a.restore_trash(i);
    assert!(a.data.spaces[0].lists.iter().any(|l| l.id == web));
    assert_eq!(a.data.tasks.iter().filter(|t| t.list_id == web).count(), n);
    assert!(a.toast.is_some());
    // a whole space: each list goes to Trash and lands in the first space
    let ideas_space = a.data.spaces[1].id;
    let ideas = a.data.spaces[1].lists[0].id;
    a.do_delete(Target::Space(ideas_space));
    let i = a.data.extra.trash.iter().position(|x| x.title == "Ideas").expect("space's list in trash");
    a.restore_trash(i);
    assert!(a.data.spaces.iter().find(|s| s.id == sid).unwrap().lists.iter().any(|l| l.id == ideas));
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
    let i = a.data.extra.trash.iter().position(|x| x.kind == "List").unwrap();
    a.restore_trash(i);
    let f = a.data.spaces[0].folders.iter().find(|f| f.id == fid).unwrap();
    let l = f.lists.iter().find(|l| l.id == mkt).expect("back in its folder");
    assert_eq!((l.color, l.description.as_str(), l.priority), (Some([1, 2, 3]), "about", 2));
    // an entry saved by an older version (space id, list, tasks) still restores
    let sid = a.data.spaces[1].id;
    let old = ListItem { id: 9999, name: "Old one".into(), ..Default::default() };
    let payload = serde_json::to_string(&(sid, old, Vec::<Task>::new())).unwrap();
    a.data.extra.trash.push(TrashItem { id: 1, kind: "List".into(), title: "Old one".into(), deleted: 0, payload });
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
    let i = a.data.extra.trash.iter().position(|x| x.title == "Lonely").unwrap();
    a.restore_trash(i);
    assert!(list_ids(&a).contains(&ideas));
    assert_eq!(a.task(id).unwrap().list_id, ideas);
    // the list is gone for good: the task goes to the first list
    a.apply(Action::DeleteTask(id));
    a.do_delete(Target::List(ideas));
    a.data.extra.trash.retain(|x| x.kind != "List");
    let i = a.data.extra.trash.iter().position(|x| x.title == "Lonely").unwrap();
    a.restore_trash(i);
    assert_eq!(a.task(id).unwrap().list_id, a.data.first_list().unwrap());
    assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains("list is gone")));
}

#[test]
fn renaming_or_deleting_a_status_carries_its_tasks() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Moving");
    let mut t = a.task(id).unwrap().clone();
    t.status = "in progress".into();
    a.apply(Action::Replace(t));
    a.data.extra.automations.push(Automation { id: 5, enabled: true, trigger_status: "in progress".into(), action: "priority".into(), value: "1".into(), list: None });
    let si = 0;
    let mut orig: Vec<Option<String>> = a.data.spaces[si].statuses.iter().map(|s| Some(s.name.clone())).collect();
    let i = a.data.spaces[si].statuses.iter().position(|s| s.name == "in progress").unwrap();
    // a rename that matches another status waits, so tasks never merge
    a.data.spaces[si].statuses[i].name = "to do".into();
    a.commit_status_renames(si, &mut orig);
    assert_eq!(a.task(id).unwrap().status, "in progress");
    a.data.spaces[si].statuses[i].name = "doing".into();
    a.commit_status_renames(si, &mut orig);
    assert_eq!(a.task(id).unwrap().status, "doing");
    // the other space still has "in progress", so the automation stays
    assert_eq!(a.data.extra.automations[0].trigger_status, "in progress");
    // deleting it moves the task to a status of the same kind (or the first)
    let st = a.data.spaces[si].statuses.remove(i);
    let old = orig.remove(i).unwrap();
    a.move_off_status(si, &old, st.kind, &orig);
    let t = a.task(id).unwrap();
    let kind_match = a.data.spaces[si].statuses.iter().find(|s| s.kind == st.kind).map(|s| s.name.clone()).unwrap_or_else(|| a.data.spaces[si].statuses[0].name.clone());
    assert_eq!(t.status, kind_match);
    assert!(t.activity.iter().any(|x| x.text.contains("changed status from DOING")));
}

#[test]
fn archived_subtasks_hide_and_my_subtasks_show_on_personal_views() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let parent = add_task(&mut a, list, "Parent");
    let me = a.data.me;
    a.apply(Action::Add { list, name: "Mine sub".into(), status: None, priority: 0, assignees: vec![me], due: Some(today()), parent: Some(parent), open: false });
    let sub = a.data.tasks.iter().find(|t| t.name == "Mine sub").unwrap().id;
    a.loc = Location::MyTasks;
    assert!(a.filtered().iter().any(|t| t.id == sub), "my subtask on Assigned to me");
    a.loc = Location::TodayOverdue;
    assert!(a.filtered().iter().any(|t| t.id == sub), "my subtask on Today & Overdue");
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
    let g = Group { key: "all".into(), label: String::new(), color: Color32::WHITE, status: None, priority: None, assignee: None, due: None, tasks: vec![] };
    a.add_from_group(&g, "Due now".into(), false);
    a.apply_actions();
    let t = a.data.tasks.iter().find(|t| t.name == "Due now").unwrap();
    assert!(t.assignees.contains(&a.data.me));
    assert_eq!(t.due, Some(today()));
    assert!(a.filtered().iter().any(|x| x.name == "Due now"));
    a.open_quick_add(None);
    let qa = a.quick_add.as_ref().unwrap();
    assert_eq!((qa.assignees.clone(), qa.due), (vec![a.data.me], Some(today())));
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
    let dir = std::env::temp_dir().join(format!("wb-csv-{}", now_ts()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("round.csv");
    std::fs::write(&f, &text).unwrap();
    let before = a.data.tasks.len();
    let msg = a.import_csv(f.to_str().unwrap());
    a.apply_actions();
    assert!(msg.contains(&format!("Imported {}", a.data.tasks.len() - before)), "{}", msg);
    assert!(a.data.tasks.len() - before >= names.len());
    let new: Vec<String> = a.data.tasks[before..].iter().map(|t| t.name.clone()).collect();
    for n in names {
        assert!(new.iter().any(|x| x == n), "{:?} missing from {:?}", n, new);
    }
    assert_eq!(super::menus::parse_csv("a, \"b,\"\"c\"\"\" ,d\n"), vec![vec!["a".to_string(), "b,\"c\"".into(), "d".into()]]);
}

#[test]
fn a_login_name_starting_with_a_non_ascii_letter() {
    assert_eq!(Data::display_user("\u{e9}va"), "\u{c9}va");
    assert_eq!(Data::display_user("\u{c9c0}\u{c6d0}"), "\u{c9c0}\u{c6d0}");
    assert_eq!(Data::display_user(""), "Me");
}

fn test_msg(id: u64, author: u64, text: &str, ts: i64) -> Message {
    Message { id, author, text: text.into(), ts, reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: None, attachments: vec![], edited: None }
}

#[test]
fn private_channels_and_dms_show_only_to_their_members() {
    let mut a = app();
    let secret = a.create_channel("secret plans", true, false);
    let public = a.data.channels.iter().find(|c| c.name == "general").unwrap().id;
    let dm_me_alex = a.data.channels.iter().find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2]).unwrap().id;
    let ch = |a: &App, id: u64| a.data.channels.iter().find(|c| c.id == id).unwrap().clone();
    assert!(a.can_see(&ch(&a, secret)), "the person who made it is in it");
    // Sam (3) is in neither the private channel nor my DM with Alex
    a.data.me = 3;
    assert!(!a.can_see(&ch(&a, secret)));
    assert!(!a.can_see(&ch(&a, dm_me_alex)));
    assert!(a.can_see(&ch(&a, public)), "public channels stay open to all");
    // added as a member, Sam sees it
    a.data.channels.iter_mut().find(|c| c.id == secret).unwrap().members.push(3);
    assert!(a.can_see(&ch(&a, secret)));
}

#[test]
fn closing_a_dm_hides_it_for_me_until_a_new_message() {
    let mut a = app();
    let dm = a.data.channels.iter().find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2]).unwrap().id;
    let n = a.data.channels.len();
    a.chat_open = Some(dm);
    a.close_dm(dm);
    assert_eq!(a.data.channels.len(), n, "nothing is deleted");
    assert_eq!(a.chat_open, None);
    let listed = |a: &App| a.listed(a.data.channels.iter().find(|c| c.id == dm).unwrap());
    assert!(!listed(&a));
    // only this computer remembers it: it's not in what the team shares
    let v = serde_json::to_value(&a.data).unwrap();
    assert!(v["channels"].as_array().unwrap().iter().all(|c| c.get("closed").is_none()));
    assert!(!crate::sync::SHARED.iter().any(|p| p.contains("closed_dms")));
    // Alex writes again: it's back
    let id = a.data.new_id();
    let c = a.data.channels.iter_mut().find(|c| c.id == dm).unwrap();
    c.messages.push(test_msg(id, 2, "still there?", now_ts() + 5));
    assert!(listed(&a));
    // opening it from New message also brings it back
    a.close_dm(dm);
    assert!(!listed(&a));
    a.apply_chat(vec![ChatAction::Dm(2)]);
    assert_eq!(a.chat_open, Some(dm));
}

#[test]
fn two_dms_with_the_same_people_show_as_one() {
    let mut a = app();
    let older = a.data.channels.iter().find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 3]).unwrap().id;
    let newer = a.data.new_id();
    a.data.channels.push(Channel { id: newer, name: String::new(), kind: ChannelKind::Direct, topic: String::new(), members: vec![3, 1], messages: vec![], last_read: now_ts(), ..Default::default() });
    let get = |a: &App, id: u64| a.data.channels.iter().find(|c| c.id == id).unwrap().clone();
    assert!(a.listed(&get(&a, older)));
    assert!(!a.listed(&get(&a, newer)), "the newer empty copy gives way");
    a.apply_chat(vec![ChatAction::Dm(3)]);
    assert_eq!(a.chat_open, Some(older));
    // a copy with messages wins over an empty one
    let id = a.data.new_id();
    a.data.channels.iter_mut().find(|c| c.id == newer).unwrap().messages.push(test_msg(id, 3, "hi", now_ts()));
    assert!(!a.listed(&get(&a, older)));
    assert!(a.listed(&get(&a, newer)));
}

#[test]
fn a_missing_file_is_fetched_again_later() {
    let folder = temp_folder("retry-files");
    std::fs::create_dir_all(folder.join("files")).unwrap();
    let pic = picture_file("later.png", 30, 20);
    let (sha, bytes) = wb_net::sha256_file(&pic).unwrap();
    let att = Attachment { file: sha.clone(), name: "later.png".into(), kind: MediaKind::Image, bytes, w: 30, h: 20, secs: 0.0 };
    let mut f = super::files::Files::new(temp_folder("retry-cache"));
    let remote = super::files::Remote::Folder(folder.clone());
    // not in the shared folder yet: the first try fails and the next one waits
    f.settle(&remote, |f| {
        f.want(&att, super::files::THUMB);
        matches!(f.state(&sha), Some(super::files::Transfer::Failed(_))) && f.fetch_tries(&sha) == Some((1, true))
    });
    std::fs::copy(&pic, folder.join("files").join(&sha)).unwrap();
    f.retry_due(&sha);
    f.settle(&remote, |f| {
        f.want(&att, super::files::THUMB);
        f.local(&sha).is_some()
    });
}

#[test]
fn without_a_sign_in_attachments_wait_and_the_token_is_kept() {
    let mut a = app();
    a.data.extra.shared = Some(crate::sync::SharedCfg { folder: String::new(), device: "dev1".into(), tag: 1, server: Some("127.0.0.1:9".into()) });
    assert_eq!(a.media_remote(), super::files::Remote::SignedOut);
    a.add_media(false, Some(picture_file("wait.png", 20, 20)), None);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while a.chat_files[0].att.is_none() && std::time::Instant::now() < end {
        a.files_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let att = a.chat_files[0].att.clone().unwrap();
    assert_eq!(a.files.state(&att.file), Some(super::files::Transfer::Failed(super::files::NOT_SIGNED_IN.into())), "not silently done");
    assert!(!a.pending_ready(false).0);
    // a sign-in is seen at once (the cached token is dropped when one is saved)
    super::together::save_token("127.0.0.1:9", "dev1", "tok");
    assert!(matches!(a.media_remote(), super::files::Remote::Server { auth, .. } if auth.token == "tok"));
    super::together::forget_token("127.0.0.1:9", "dev1");
    assert_eq!(a.media_remote(), super::files::Remote::SignedOut);
}

#[test]
fn sharing_a_folder_brings_earlier_pictures_along() {
    let mut a = app();
    a.add_media(false, Some(picture_file("before.png", 24, 24)), None);
    let files = until_ready(&mut a, false);
    let ch = a.data.channels[0].id;
    a.apply_chat(vec![ChatAction::Send { ch, text: "from before sharing".into(), parent: None, files: files.clone() }]);
    let folder = temp_folder("share-later");
    a.share_here(&folder.to_string_lossy()).unwrap();
    let dest = folder.join("files").join(&files[0].file);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !dest.is_file() && std::time::Instant::now() < end {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(dest.is_file(), "the picture sent earlier is in the shared folder");
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
    assert_eq!(a.task(ids[0]).unwrap().timer_started, Some(started), "Alex's timer keeps running");
    assert_eq!(a.task(ids[1]).unwrap().timer_by, Some(1));
    // clicking Alex's timer doesn't stop it
    a.apply(Action::ToggleTimer(ids[0]));
    assert_eq!(a.task(ids[0]).unwrap().timer_started, Some(started));
    assert!(a.task(ids[1]).unwrap().timer_started.is_some());
    // closing the app (on_exit) stops mine and keeps Alex's
    a.data.stop_my_timers(now_ts());
    assert!(a.task(ids[1]).unwrap().timer_started.is_none());
    assert_eq!(a.task(ids[1]).unwrap().time_entries.last().unwrap().member, 1, "the stopped time is mine");
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
    let nine = day.and_hms_opt(9, 0, 0).and_then(|x| x.and_local_timezone(chrono::Local).single()).unwrap().timestamp();
    let task = a.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
    task.assignees = vec![3];
    task.time_entries.push(TimeEntry { start: nine, end: nine + 3600, member: 2 });
    // an old entry with no owner belongs to the first assignee (Sam), not me
    task.time_entries.push(TimeEntry { start: nine, end: nine + 1800, member: 0 });
    a.set_day_time(id, day, 2.0);
    let t = a.task(id).unwrap();
    let mine: i64 = t.time_entries.iter().filter(|e| a.data.is_my_entry(t, e)).map(|e| e.end - e.start).sum();
    assert_eq!(mine, 7200);
    assert!(t.time_entries.iter().any(|e| e.member == 2 && e.end - e.start == 3600), "Alex's hour stays");
    assert!(t.time_entries.iter().any(|e| e.member == 0 && e.end - e.start == 1800), "the old entry stays");
    a.set_day_time(id, day, 0.5);
    let t = a.task(id).unwrap();
    assert_eq!(t.time_entries.iter().filter(|e| e.member == 1).map(|e| e.end - e.start).sum::<i64>(), 1800);
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
    a.data.extra.automations.push(Automation { id: 1, enabled: true, trigger_status: "complete".into(), action: "priority".into(), value: "4".into(), list: Some(other) });
    a.data.extra.automations.push(Automation { id: 2, enabled: true, trigger_status: "complete".into(), action: "move".into(), value: other.to_string(), list: Some(list) });
    let id = add_task(&mut a, list, "Scoped");
    a.apply(Action::Add { list, name: "Scoped child".into(), status: None, priority: 0, assignees: vec![], due: None, parent: Some(id), open: false });
    let child = a.data.tasks.iter().find(|t| t.name == "Scoped child").unwrap().id;
    let mut t = a.task(id).unwrap().clone();
    t.status = "complete".into();
    a.apply(Action::Replace(t));
    let t = a.task(id).unwrap();
    assert_eq!(t.priority, 0, "the rule for the other list must not run here");
    assert_eq!(t.list_id, other);
    assert_eq!(a.task(child).unwrap().list_id, other, "subtasks move along");
    let statuses = a.data.statuses_for_list(other);
    assert!(statuses.iter().any(|s| s.name == a.task(child).unwrap().status));
}

#[test]
fn removing_a_person_drops_them_from_teams_and_meetings() {
    let mut a = app();
    a.data.extra.teams.push(Team { id: 900, name: "T".into(), members: vec![1, 3], color: [0, 0, 0] });
    a.data.extra.meetings.push(Meeting { id: 901, title: "M".into(), date: today(), time: "10:00".into(), attendees: vec![1, 3], agenda: String::new(), notes: String::new() });
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
    let g = Goal { id: 902, name: "G".into(), color: [0, 0, 0], due: None, owner: 1, targets: vec![GoalTarget { name: "x".into(), kind: TargetKind::Tasks { ids: vec![id, 123456789] } }] };
    assert_eq!(a.goal_progress(&g), 1.0);
}
