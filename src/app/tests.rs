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
    a.data.extra.reminders.push(Reminder { id: 1, text: "Call".into(), due_ts: now_ts() - 60, done: false, owner: me, ..Default::default() });
    a.data.extra.reminders.push(Reminder { id: 2, text: "Later".into(), due_ts: now_ts() + 3600, done: false, owner: me, ..Default::default() });
    a.data.extra.reminders.push(Reminder { id: 3, text: "Done".into(), due_ts: now_ts() - 60, done: true, owner: me, ..Default::default() });
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
    t.checklist = vec![CheckItem { text: "send".into(), done: true, assignee: None }];
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

// ---- task comments and activity (sf-comments)

use super::comments::{parse_link, segments, toggle_comment_check, toggle_reaction, LinkTo, Seg};

#[test]
fn comment_lines_split_into_mentions_and_links() {
    let names = vec!["Alex Kim".to_string(), "Alex".to_string()];
    let segs = segments("hi @Alex Kim see workboard://task/42, ok", &names);
    assert_eq!(segs, vec![Seg::Text("hi "), Seg::Mention("@Alex Kim"), Seg::Text(" see "), Seg::Link("workboard://task/42", LinkTo::Task(42)), Seg::Text(", ok")]);
    // an @ inside a word is no mention
    assert_eq!(segments("mail@Alex", &names), vec![Seg::Text("mail@Alex")]);
    assert_eq!(parse_link("workboard://list/7").map(|x| x.0), Some(LinkTo::List(7)));
    assert_eq!(parse_link("workboard://space/9 more").map(|x| x.0), Some(LinkTo::Space(9)));
    assert_eq!(parse_link("workboard://task/"), None);
}

#[test]
fn comment_checklist_items_tick_and_untick() {
    let text = "Todo:\n\u{2610} one\n\u{2610} two";
    let once = toggle_comment_check(text, 2);
    assert_eq!(once, "Todo:\n\u{2610} one\n\u{2611} two");
    assert_eq!(toggle_comment_check(&once, 2), text);
    // a line that is no checklist item stays as it is
    assert_eq!(toggle_comment_check(text, 0), text);
}

#[test]
fn comment_reactions_toggle_per_person() {
    let mut rs = vec![];
    toggle_reaction(&mut rs, "\u{1F44D}", 1);
    toggle_reaction(&mut rs, "\u{1F44D}", 2);
    assert_eq!(rs[0].users, vec![1, 2]);
    toggle_reaction(&mut rs, "\u{1F44D}", 1);
    toggle_reaction(&mut rs, "\u{1F44D}", 2);
    assert!(rs.is_empty());
}

#[test]
fn mention_word_reads_the_name_being_typed() {
    assert_eq!(mention_word("hi @"), Some(""));
    assert_eq!(mention_word("hi @Al"), Some("Al"));
    assert_eq!(mention_word("@Al"), Some("Al"));
    assert_eq!(mention_word("hi @Al and"), None);
    assert_eq!(mention_word("mail@x"), None);
}

#[test]
fn old_activity_gets_the_same_ids_on_every_computer() {
    // a file from before activity had ids
    let json = r#"{"ts": 5, "author": 1, "text": "hi", "comment": true}"#;
    let a: Activity = serde_json::from_str(json).unwrap();
    assert_eq!(a.id, 0);
    let mut d1 = Data::seed();
    let mut d2 = Data::seed();
    for d in [&mut d1, &mut d2] {
        let t = &mut d.tasks[0];
        t.activity = vec![a.clone(), a.clone(), Activity { ts: 6, author: 2, text: "x".into(), ..Default::default() }];
        d.fix_activity_ids();
    }
    let ids1: Vec<u64> = d1.tasks[0].activity.iter().map(|a| a.id).collect();
    let ids2: Vec<u64> = d2.tasks[0].activity.iter().map(|a| a.id).collect();
    assert_eq!(ids1, ids2);
    assert!(ids1.iter().all(|i| *i != 0));
    // two identical old lines still get different ids
    assert_ne!(ids1[0], ids1[1]);
    // and the app works out the same ids wb-sync does
    let mut v = serde_json::json!({"tasks": [{"id": d1.tasks[0].id, "activity": [json_val(&a), json_val(&a)]}]});
    wb_sync::fill_activity_ids(&mut v);
    assert_eq!(v["tasks"][0]["activity"][1]["id"].as_u64(), Some(ids1[1]));
    // id 0 is never written, so an entry without an id never looks like id 0
    assert!(!serde_json::to_string(&a).unwrap().contains("\"id\""));
}

fn json_val(a: &Activity) -> serde_json::Value {
    serde_json::to_value(a).unwrap()
}

#[test]
fn edited_and_deleted_comments_carry_over_between_copies() {
    use wb_sync::{merge, stamp, SyncMeta};
    let mut base = Data::seed();
    base.tasks.truncate(1);
    base.tasks[0].activity = vec![
        Activity { ts: 1, author: 1, text: "first".into(), comment: true, id: 90_001, ..Default::default() },
        Activity { ts: 2, author: 2, text: "second".into(), comment: true, id: 90_002, ..Default::default() },
    ];
    let mut ma = SyncMeta::default();
    let mut mb = SyncMeta::default();
    let va = serde_json::to_value(&base).unwrap();
    stamp(&va, &mut ma, 1000);
    stamp(&va, &mut mb, 1000);
    // A edits the first comment, B deletes the second and adds a third
    let mut a = base.clone();
    a.tasks[0].activity[0].text = "first (fixed)".into();
    a.tasks[0].activity[0].edited = Some(3);
    let mut b = base.clone();
    b.tasks[0].activity.remove(1);
    b.tasks[0].activity.push(Activity { ts: 4, author: 3, text: "third".into(), comment: true, id: 90_003, ..Default::default() });
    let mut va = serde_json::to_value(&a).unwrap();
    stamp(&va, &mut ma, 2000);
    va["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    let mut vb = serde_json::to_value(&b).unwrap();
    stamp(&vb, &mut mb, 2000);
    vb["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    merge(&mut va, &[vb]);
    let merged: Data = serde_json::from_value(va).unwrap();
    let texts: Vec<&str> = merged.tasks[0].activity.iter().map(|a| a.text.as_str()).collect();
    assert_eq!(texts, vec!["first (fixed)", "third"]);
}

#[test]
fn typing_a_name_or_description_logs_one_line() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Plan");
    let n = a.task(id).unwrap().activity.len();
    // the name box sends a change on every key press
    for name in ["Plan t", "Plan tr", "Plan trip"] {
        let mut t = a.task(id).unwrap().clone();
        t.name = name.into();
        a.apply(Action::Replace(t));
    }
    let t = a.task(id).unwrap().clone();
    assert_eq!(t.activity.len(), n + 1);
    assert_eq!(t.activity.last().unwrap().text, "renamed this task to \"Plan trip\"");
    let mut t2 = t.clone();
    t2.description = "Notes".into();
    t2.tags.push("travel".into());
    t2.estimate_min = Some(90);
    a.apply(Action::Replace(t2));
    let texts: Vec<String> = a.task(id).unwrap().activity.iter().map(|x| x.text.clone()).collect();
    assert!(texts.contains(&"updated the description".to_string()));
    assert!(texts.contains(&"added tag travel".to_string()));
    assert!(texts.contains(&"set the time estimate to 1h 30m".to_string()), "{:?}", texts);
}

#[test]
fn each_task_keeps_its_own_comment_draft() {
    let mut a = app();
    let (t1, t2) = (a.data.tasks[0].id, a.data.tasks[1].id);
    a.apply(Action::Open(t1));
    a.comment_text = "half a thought".into();
    a.apply(Action::Open(t2));
    assert!(a.comment_text.is_empty());
    a.comment_text = "other".into();
    a.apply(Action::Open(t1));
    assert_eq!(a.comment_text, "half a thought");
    a.apply(Action::Open(t2));
    assert_eq!(a.comment_text, "other");
}

#[test]
fn a_copy_from_before_comment_ids_merges_without_doubles() {
    let mut d = Data::seed();
    d.tasks.truncate(1);
    let tid = d.tasks[0].id;
    d.tasks[0].activity = vec![Activity { ts: 1, author: 1, text: "old".into(), comment: true, ..Default::default() }];
    // the other computer still runs an app without ids and added a comment
    let mut old = serde_json::to_value(&d).unwrap();
    old["tasks"][0]["activity"].as_array_mut().unwrap().push(serde_json::json!({"ts": 2, "author": 2, "text": "from the old app", "comment": true}));
    d.fix_activity_ids();
    let mut mine = serde_json::to_value(&d).unwrap();
    wb_sync::merge(&mut mine, &[old]);
    let merged: Data = serde_json::from_value(mine).unwrap();
    let t = merged.tasks.iter().find(|t| t.id == tid).unwrap();
    let texts: Vec<&str> = t.activity.iter().map(|a| a.text.as_str()).collect();
    assert_eq!(texts, vec!["old", "from the old app"]);
    assert!(t.activity.iter().all(|a| a.id != 0));
}

// ---------------------------------------------------------------- list view slice (sf-listview)

fn tid(a: &App, name: &str) -> u64 {
    a.data.tasks.iter().find(|t| t.name == name).map(|t| t.id).unwrap_or_else(|| panic!("no task {:?}", name))
}

fn names(a: &App) -> Vec<String> {
    a.filtered().iter().map(|t| t.name.clone()).collect()
}

#[test]
fn manual_order_moves_a_task_among_its_siblings() {
    let mut a = app();
    let web = a.data.tasks.iter().find(|t| t.name == "Write copy for landing page").unwrap().list_id;
    a.loc = Location::List(web);
    a.show_closed = true;
    let before = names(&a);
    let (drag, target) = (tid(&a, "Collect feedback from beta users"), tid(&a, "Write copy for landing page"));
    a.reorder_task(drag, target, false);
    let after = names(&a);
    let pos = |v: &[String], n: &str| v.iter().position(|x| x == n).unwrap();
    assert!(pos(&after, "Collect feedback from beta users") + 1 == pos(&after, "Write copy for landing page"), "{:?}", after);
    assert_eq!(after.len(), before.len());
    // dropped after a task
    a.reorder_task(drag, tid(&a, "Choose color palette"), true);
    let v = names(&a);
    assert!(pos(&v, "Collect feedback from beta users") > pos(&v, "Choose color palette"));
    // subtasks move only under their own parent
    let (s1, s2) = (tid(&a, "Wireframe hero section"), tid(&a, "Pick hero image"));
    let parent = a.task(s1).unwrap().parent.unwrap();
    a.reorder_task(s2, s1, false);
    let subs: Vec<u64> = a.subtasks(parent).iter().map(|t| t.id).collect();
    assert_eq!(subs, vec![s2, s1]);
    let top = tid(&a, "Write copy for landing page");
    let order = a.task(s1).unwrap().order;
    a.reorder_task(s1, top, false);
    assert_eq!(a.task(s1).unwrap().parent, Some(parent));
    assert_eq!(a.task(s1).unwrap().order, order, "a subtask is not moved among top-level tasks");
}

#[test]
fn new_sorts_and_groups() {
    let mut a = app();
    a.loc = Location::Everything;
    a.show_closed = true;
    a.sort_by = SortBy::Status;
    let sts: Vec<StatusKind> = a.filtered().iter().map(|t| a.data.status_of(t).kind).collect();
    let rank = |k: &StatusKind| match k {
        StatusKind::Open => 0,
        StatusKind::Custom => 1,
        StatusKind::Done => 2,
        StatusKind::Closed => 3,
    };
    assert!(sts.windows(2).all(|w| rank(&w[0]) <= rank(&w[1])), "open statuses first");
    a.sort_by = SortBy::StartDate;
    let v = a.filtered();
    let firsts: Vec<bool> = v.iter().map(|t| t.start.is_none()).collect();
    assert!(firsts.windows(2).all(|w| !w[0] || w[1]), "no start date last");
    a.sort_by = SortBy::Assignee;
    let v = a.filtered();
    assert!(v.last().unwrap().assignees.is_empty() || v.iter().all(|t| !t.assignees.is_empty()));
    let id = tid(&a, "Newsletter draft");
    a.data.tasks.iter_mut().find(|t| t.id == id).unwrap().estimate_min = Some(5);
    a.sort_by = SortBy::Estimate;
    assert_eq!(a.filtered()[0].id, id);
    a.data.tasks.iter_mut().find(|t| t.id == id).unwrap().updated = now_ts() + 100;
    a.sort_by = SortBy::Updated;
    assert_eq!(a.filtered()[0].id, id, "most recently updated first");
    // Group: Tags (a task under each of its tags) and Group: List
    a.sort_by = SortBy::Manual;
    a.group_by = GroupBy::Tags;
    let tasks = a.filtered();
    let groups = a.groups(&tasks, false);
    let design = groups.iter().find(|g| g.key == "t:design").expect("a design group");
    assert_eq!(design.tasks.len(), 2);
    assert!(groups.iter().any(|g| g.label == "NO TAGS"));
    a.group_by = GroupBy::List;
    let tasks = a.filtered();
    let groups = a.groups(&tasks, false);
    assert!(groups.iter().any(|g| g.label == "MARKETING"));
    // adding from a tag group gives the task that tag; from a list group, that list
    a.group_by = GroupBy::Tags;
    let tasks = a.filtered();
    let g = a.groups(&tasks, false).into_iter().find(|g| g.key == "t:bug").unwrap();
    a.add_from_group(&g, "Tagged".into(), false);
    a.apply_actions();
    assert_eq!(a.task(tid(&a, "Tagged")).unwrap().tags, vec!["bug".to_string()]);
    a.group_by = GroupBy::List;
    let tasks = a.filtered();
    let g = a.groups(&tasks, false).into_iter().find(|g| g.label == "MARKETING").unwrap();
    a.add_from_group(&g, "In marketing".into(), false);
    a.apply_actions();
    assert_eq!(a.data.list_name(a.task(tid(&a, "In marketing")).unwrap().list_id), "Marketing");
}

#[test]
fn new_filter_fields() {
    use parity::{FilterField, FilterRow};
    let mut a = app();
    a.loc = Location::Everything;
    let row = |f: FilterField, v: &str| FilterRow { field: Some(f), not: false, values: vec![v.to_string()] };
    a.filter.rows = vec![row(FilterField::HasSubtasks, "yes")];
    assert_eq!(names(&a), vec!["Design new homepage layout".to_string()]);
    a.filter.rows = vec![row(FilterField::StartDate, "5")];
    assert!(!a.filtered().is_empty() && a.filtered().iter().all(|t| t.start.is_none()));
    a.filter.rows = vec![row(FilterField::CreatedBy, "2")];
    assert!(a.filtered().is_empty(), "the seed tasks were all made by Me");
    a.filter.rows = vec![row(FilterField::CreatedBy, "1")];
    assert!(!a.filtered().is_empty());
    a.filter.rows = vec![row(FilterField::Created, "1")];
    assert!(a.filtered().is_empty(), "nothing was made today");
    a.filter.rows = vec![row(FilterField::Created, "3")];
    assert!(!a.filtered().is_empty());
    a.filter.rows = vec![row(FilterField::Updated, "1")];
    assert!(!a.filtered().is_empty());
}

#[test]
fn search_finds_subtasks() {
    let mut a = app();
    a.loc = Location::Everything;
    a.show_closed = true;
    a.search = "hero".into();
    let v = names(&a);
    assert!(v.contains(&"Pick hero image".to_string()) && v.contains(&"Wireframe hero section".to_string()), "{:?}", v);
    a.search.clear();
    assert!(!names(&a).contains(&"Pick hero image".to_string()), "subtasks stay under their parent otherwise");
}

#[test]
fn view_settings_are_kept_per_location() {
    let mut a = app();
    let lists = list_ids(&a);
    let (l1, l2) = (lists[0], lists[1]);
    a.loc = Location::List(l1);
    a.sync_view_prefs();
    a.group_by = GroupBy::Priority;
    a.sort_by = SortBy::Name;
    a.sync_view_prefs();
    a.loc = Location::List(l2);
    a.sync_view_prefs();
    assert!(a.group_by == GroupBy::Priority, "a list never changed keeps what was showing");
    a.group_by = GroupBy::Assignee;
    a.show_closed = true;
    a.sync_view_prefs();
    a.loc = Location::List(l1);
    a.sync_view_prefs();
    assert!(a.group_by == GroupBy::Priority && a.sort_by == SortBy::Name && !a.show_closed);
    a.loc = Location::List(l2);
    a.sync_view_prefs();
    assert!(a.group_by == GroupBy::Assignee && a.show_closed);
    // and they survive a round trip through the saved file format
    let s = serde_json::to_string(&a.view_prefs).unwrap();
    let back: HashMap<String, listview::ViewPrefs> = serde_json::from_str(&s).unwrap();
    assert_eq!(back, a.view_prefs);
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

// ---------------------------------------------------------------- task window fields (sf-taskfields)

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
    assert!(!a.pick_candidates(x, TaskPick::WaitingOn).contains(&y), "already waiting on it");
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
    assert!(!a.pick_candidates(p, TaskPick::Parent).contains(&c), "a task can't go under its own subtask");
    assert!(!a.pick_candidates(c, TaskPick::Child).contains(&p), "nor take its parent as a subtask");
    a.make_top_level(c);
    assert_eq!(a.task(c).unwrap().parent, None);
    assert!(a.task(c).unwrap().activity.iter().any(|x| x.text.contains("top-level")));
}

#[test]
fn new_task_fields_load_from_old_files_and_stay_out_when_empty() {
    let a = app();
    let t = a.data.tasks[0].clone();
    let json = serde_json::to_string(&t).unwrap();
    for k in ["milestone", "watchers", "waiting_on", "linked", "attachments", "assignee"] {
        assert!(!json.contains(&format!("\"{}\"", k)), "{} is left out when empty", k);
    }
    let back: Task = serde_json::from_str(&json).unwrap();
    assert!(!back.milestone && back.watchers.is_empty() && back.checklist.iter().all(|c| c.assignee.is_none()));
    let mut w = t.clone();
    w.watchers.push(2);
    assert!(task_changed(&t, &w));
    let mut e = t.clone();
    e.time_entries.push(TimeEntry { start: 0, end: 60, member: 0 });
    assert!(task_changed(&t, &e));
}

// ---------------------------------------------------------------- apps sweep (inbox, trash, forms, search, timesheets)

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
    let i = a.data.extra.trash.iter().position(|x| x.kind == "List").unwrap();
    a.restore_trash(i);
    let f = a.data.spaces[0].folders.iter().find(|f| f.id == fid).unwrap();
    let l = f.lists.iter().find(|l| l.id == lid).expect("back in its folder");
    assert_eq!(l.color, Some([1, 2, 3]));
    assert_eq!(l.description, "About the site");
    // a List trashed by an older version (no folder) still restores
    let sid = a.data.spaces[0].id;
    let old = serde_json::to_string(&(sid, ListItem { id: 77, name: "Old".into(), ..Default::default() }, Vec::<Task>::new())).unwrap();
    a.data.extra.trash.push(TrashItem { id: 1, kind: "List".into(), title: "Old".into(), deleted: 0, payload: old });
    let i = a.data.extra.trash.len() - 1;
    a.restore_trash(i);
    assert!(a.data.spaces[0].lists.iter().any(|l| l.id == 77));
}

#[test]
fn app_items_go_to_trash_and_back() {
    let mut a = app();
    let id = a.data.new_id();
    a.data.extra.goals.push(Goal { id, name: "Ship".into(), color: [1, 1, 1], due: None, owner: a.data.me, targets: vec![] });
    a.remove_item("goal", id);
    a.data.extra.teams.push(Team { id: id + 1, name: "Core".into(), members: vec![], color: [2, 2, 2] });
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
    assert!(a.data.extra.dashboards.iter().any(|x| x.name == "Sales" && x.owner == a.data.me));
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
            FormField { label: "Title".into(), map: "name".into(), required: true, ..Default::default() },
            FormField { label: "Seats".into(), map: "number".into(), ..Default::default() },
            FormField { label: "Mail".into(), map: "email".into(), ..Default::default() },
            FormField { label: "Who".into(), map: "assignee".into(), ..Default::default() },
            FormField { label: "Size".into(), map: "dropdown".into(), options: vec!["S".into(), "M".into()], ..Default::default() },
        ],
        owner: 1,
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
fn search_reads_copied_links_and_finds_apps_and_people() {
    use super::more::Hit;
    let mut a = app();
    let tid = a.data.tasks[2].id;
    let rows = a.search_rows(&format!("workboard://task/{}", tid), 0);
    assert_eq!(rows[0].4, Hit::Task(tid));
    let lid = a.data.first_list().unwrap();
    assert_eq!(a.search_rows(&format!("workboard://list/{}", lid), 0)[0].4, Hit::Go(Location::List(lid)));
    let sid = a.data.spaces[1].id;
    assert_eq!(a.search_rows(&format!("workboard://space/{}/", sid), 0)[0].4, Hit::Go(Location::Space(sid)));
    a.new_dashboard("Quarterly numbers");
    let did = a.dash_open.unwrap();
    assert!(a.search_rows("quarterly", 0).iter().any(|r| r.4 == Hit::Dashboard(did)));
    assert!(a.search_rows("alex", 0).iter().any(|r| r.4 == Hit::Person(2)));
    // a message found by its text opens the channel
    assert!(a.search_rows("newsletter draft is due", 5).iter().any(|r| matches!(r.4, Hit::Chat(_))));
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
    let wb = a.data.extra.whiteboards.iter_mut().find(|w| w.id == wid).unwrap();
    assert_eq!(wb.owner, 1);
    let n = Sticky { pos: [0.0, 0.0], text: "Idea".into(), color: [1, 1, 1] };
    a.sweep.wb_undo.push((wid, WbUndo::NoteGone(0, n)));
    a.wb_undo(wid);
    assert_eq!(a.data.extra.whiteboards.iter().find(|w| w.id == wid).unwrap().notes[0].text, "Idea");
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
    Message { mentions: vec![], id, author, text: text.into(), ts, reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: None, attachments: vec![], edited: None }
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
    // on the task Alex is timing I run my own timer beside his (and, timing
    // one task at a time, mine on the other task stops)
    a.apply(Action::ToggleTimer(ids[0]));
    assert_eq!(a.task(ids[0]).unwrap().timer_started, Some(started), "Alex's timer keeps running");
    assert!(a.data.is_my_timer(a.task(ids[0]).unwrap()));
    assert!(a.task(ids[1]).unwrap().timer_started.is_none());
    // closing the app (on_exit) stops mine and keeps Alex's
    a.data.stop_my_timers(now_ts());
    assert!(a.task(ids[0]).unwrap().timers.is_empty());
    assert_eq!(a.task(ids[0]).unwrap().time_entries.last().unwrap().member, 1, "the stopped time is mine");
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


// ---------------------------------------------------------------- getting around (nav.rs)

#[test]
fn workboard_links_are_read_back() {
    use super::nav::{location_link, parse_link, Link};
    assert_eq!(parse_link("workboard://task/42"), Some(Link::Task(42)));
    assert_eq!(parse_link("  workboard://list/7/  "), Some(Link::Loc(Location::List(7))));
    assert_eq!(parse_link("workboard://space/3."), Some(Link::Loc(Location::Space(3))));
    assert_eq!(parse_link("workboard://folder/9"), Some(Link::Loc(Location::Folder(9))));
    assert_eq!(parse_link("workboard://inbox"), Some(Link::Loc(Location::Inbox)));
    assert_eq!(parse_link("workboard://task/abc"), None);
    assert_eq!(parse_link("workboard://task"), None);
    assert_eq!(parse_link("workboard://task/1/2"), None);
    assert_eq!(parse_link("https://task/1"), None);
    assert_eq!(parse_link("workboard://My-Workspace/Team-Space"), None);
    for l in [Location::Home, Location::Inbox, Location::Replies, Location::Assigned, Location::MyTasks, Location::TodayOverdue, Location::Everything, Location::Space(1), Location::Folder(2), Location::List(3)] {
        assert_eq!(parse_link(&location_link(l)), Some(Link::Loc(l)));
    }
    // links inside a message, by character (Korean text before one)
    let found = super::nav::find_links("보세요 workboard://task/5, and (workboard://list/6) not workboard://x");
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
    a.chat_open = Some(chans[0]);
    a.track_history();
    a.chat_open = Some(chans[1]);
    a.track_history();
    a.go_back();
    a.track_history();
    assert_eq!((a.section, a.chat_open), (Section::Chat, Some(chans[0])));
    a.go_forward();
    a.track_history();
    assert_eq!(a.chat_open, Some(chans[1]));
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
    assert!(a.nav_back.is_empty() && a.nav_fwd.is_empty());
}

fn chat_msg(id: u64, author: u64, text: &str, ts: i64) -> Message {
    Message { mentions: vec![], id, author, text: text.into(), ts, reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: None, attachments: vec![], edited: None }
}

#[test]
fn inbox_primary_and_badge_follow_the_settings() {
    let mut a = app();
    a.data.tasks.clear();
    a.data.channels.clear();
    let list = list_ids(&a)[0];
    let mine = add_task(&mut a, list, "Mine");
    let theirs = add_task(&mut a, list, "Theirs");
    // someone else made "Theirs" and I'm not on it
    a.data.tasks.iter_mut().find(|t| t.id == theirs).unwrap().activity.iter_mut().for_each(|x| x.author = 2);
    let now = now_ts();
    let push = |a: &mut App, id: u64, ts: i64, author: u64, text: &str, comment: bool| {
        a.data.tasks.iter_mut().find(|t| t.id == id).unwrap().activity.push(Activity { ts, author, text: text.into(), comment, ..Default::default() });
    };
    push(&mut a, mine, now - 10, 2, "Looks good", true);
    push(&mut a, theirs, now - 9, 3, "Not for you", true);
    push(&mut a, theirs, now - 8, 3, "@Me can you check?", true);
    push(&mut a, theirs, now - 7, 3, "assigned to Me", false);
    push(&mut a, theirs, now - 6, 3, "assigned to Alex Kim", false);
    push(&mut a, mine, now - 5, 1, "my own note", true);
    let primary = |a: &App| a.inbox_items().into_iter().filter(|i| i.tab == 0).map(|i| i.text).collect::<Vec<_>>();
    let mut p = primary(&a);
    p.sort();
    assert_eq!(p, vec!["@Me can you check?", "Looks good", "assigned to Me"]);
    // my own actions are nowhere; the rest is Other
    assert!(!a.inbox_items().iter().any(|i| i.text == "my own note"));
    assert!(a.inbox_items().iter().any(|i| i.text == "Not for you" && i.tab == 1));
    assert!(a.inbox_items().iter().any(|i| i.text == "assigned to Alex Kim" && i.tab == 1 && !i.assigned));
    assert_eq!(a.inbox_badge(), 3);
    // clearing and Later take rows off the badge
    let keys: Vec<String> = a.inbox_items().into_iter().filter(|i| i.tab == 0).map(|i| i.key).collect();
    a.data.extra.inbox_cleared.push(keys[0].clone());
    a.data.extra.inbox_later.push(keys[1].clone());
    assert_eq!(a.inbox_badge(), 1);
    // turning comments off takes "Looks good" out of Primary
    a.data.extra.inbox_cleared.clear();
    a.data.extra.inbox_later.clear();
    a.data.extra.notify.comments = false;
    assert!(!primary(&a).contains(&"Looks good".to_string()));
    a.data.extra.notify.assigned = false;
    a.data.extra.notify.mentions = false;
    assert_eq!(a.inbox_badge(), 0);
}

#[test]
fn chat_mentions_and_thread_replies_reach_the_inbox() {
    let mut a = app();
    a.data.tasks.clear();
    let now = now_ts();
    let cid = a.data.channels.iter().find(|c| c.kind == ChannelKind::Channel).unwrap().id;
    let c = a.data.channels.iter_mut().find(|c| c.id == cid).unwrap();
    c.messages.clear();
    let mut root = chat_msg(900, 1, "Plan for Friday?", now - 100);
    root.replies.push(chat_msg(901, 2, "Sounds good", now - 50));
    c.messages.push(root);
    c.messages.push(chat_msg(902, 3, "@Me please look", now - 40));
    c.messages.push(chat_msg(903, 3, "nothing for me", now - 30));
    let items = a.inbox_items();
    assert!(items.iter().any(|i| i.text == "Sounds good" && i.reply && i.tab == 0));
    assert!(items.iter().any(|i| i.text == "@Me please look" && i.mention && i.tab == 0));
    assert!(!items.iter().any(|i| i.text == "nothing for me"));
    assert_eq!(a.inbox_badge(), 2);
    assert_eq!(a.chat_replies().len(), 1);
    // opening the reply shows its thread
    let src = items.iter().find(|i| i.reply).unwrap().src;
    a.open_inbox_src(src);
    assert_eq!((a.section, a.chat_open, a.thread_open), (Section::Chat, Some(cid), Some(900)));
}

#[test]
fn reminders_alert_once_when_due() {
    let mut a = app();
    let me = a.data.me;
    let list = list_ids(&a)[0];
    let tid = add_task(&mut a, list, "Call back");
    a.data.extra.reminders.push(Reminder { id: 50, text: "Call back".into(), due_ts: now_ts() - 1, owner: me, task: Some(tid), ..Default::default() });
    a.data.extra.reminders.push(Reminder { id: 51, text: "Later on".into(), due_ts: now_ts() + 3600, owner: me, ..Default::default() });
    a.toast = None;
    a.reminder_tick(None);
    assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains("Call back")));
    assert!(a.data.extra.reminders.iter().find(|r| r.id == 50).unwrap().alerted);
    assert!(!a.data.extra.reminders.iter().find(|r| r.id == 51).unwrap().alerted);
    // only once
    a.toast = None;
    a.reminder_tick(None);
    assert!(a.toast.is_none());
    // the reminder opens its task
    let r = a.data.extra.reminders.iter().find(|r| r.id == 50).unwrap().clone();
    a.open_reminder(&r);
    a.apply_actions();
    assert_eq!(a.open_task, Some(tid));
    // with "A reminder is due" off there is no notice
    a.data.extra.notify.reminders = false;
    a.data.extra.reminders.push(Reminder { id: 52, text: "Quiet".into(), due_ts: now_ts() - 1, owner: me, ..Default::default() });
    a.reminder_tick(None);
    assert!(a.toast.is_none());
}

#[test]
fn mentions_in_private_channels_i_am_not_in_stay_out() {
    let mut a = app();
    a.data.tasks.clear();
    let now = now_ts();
    for c in &mut a.data.channels {
        c.messages.clear();
    }
    let c = a.data.channels.iter_mut().find(|c| c.kind == ChannelKind::Channel).unwrap();
    c.private = true;
    c.members = vec![2, 3];
    c.messages.push(chat_msg(950, 3, "@Me secret", now - 10));
    assert!(a.inbox_items().is_empty());
    a.data.channels.iter_mut().find(|c| c.private).unwrap().members.push(1);
    assert_eq!(a.inbox_badge(), 1);
}

#[test]
fn quitting_sends_my_stopped_timer_to_the_team() {
    let dir = temp_folder("quit");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(Data::blank("Theirs", Member { id: 1, name: "B".into(), ..Default::default() })));
    b.join_shared(&folder).unwrap();
    b.become_member(None, "Bora", "");
    let tid = a.data.tasks[0].id;
    let entries = a.data.tasks[0].time_entries.len();
    a.data.tasks[0].timer_started = Some(now_ts() - 60);
    a.data.tasks[0].timer_by = Some(a.data.me);
    a.sync_note_change();
    a.write_shared();
    b.sync_pull();
    assert!(b.task(tid).unwrap().timer_started.is_some());
    // quitting right away, before the next frame would have written it
    a.finish_before_quit();
    assert!(a.task(tid).unwrap().timer_started.is_none());
    b.sync_pull();
    let t = b.task(tid).unwrap();
    assert!(t.timer_started.is_none(), "the stopped timer reached the team");
    assert_eq!(t.time_entries.len(), entries + 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_not_recorded_before_a_merge_still_reaches_the_team() {
    let dir = temp_folder("unrecorded");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(Data::blank("Theirs", Member { id: 1, name: "B".into(), ..Default::default() })));
    b.join_shared(&folder).unwrap();
    b.become_member(None, "Bora", "");
    b.sync_note_change();
    b.write_shared();
    // A changes a task, and the team's copy is merged in before that change was recorded
    let tid = a.data.tasks[0].id;
    a.data.tasks[0].name = "Renamed just before".into();
    a.sync_pull();
    a.sync_note_change();
    a.write_shared();
    b.sync_pull();
    assert_eq!(b.task(tid).unwrap().name, "Renamed just before");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn joining_a_folder_does_not_add_you_to_private_channels() {
    let mut a = app();
    let private = a.data.channels.iter().find(|c| c.kind == ChannelKind::Channel && c.name != "general").unwrap().id;
    a.data.channels.iter_mut().find(|c| c.id == private).unwrap().private = true;
    a.become_member(None, "New", "");
    let me = a.data.me;
    let ch = |id: u64| a.data.channels.iter().find(|c| c.id == id).unwrap();
    assert!(!ch(private).members.contains(&me));
    assert!(a.data.channels.iter().find(|c| c.name == "general").unwrap().members.contains(&me));
    assert!(a.data.channels.iter().filter(|c| c.kind == ChannelKind::Direct).all(|c| !c.members.contains(&me)));
}

#[test]
fn fields_added_later_read_the_same_to_sync_when_left_unset() {
    let fp = |extra: serde_json::Value| crate::sync::fingerprints(&serde_json::json!({ "extra": extra }));
    let dash = Dashboard { id: 1, name: "D".into(), ..Default::default() };
    let form = Form { id: 2, name: "F".into(), list_id: 3, description: String::new(), fields: vec![], owner: 0 };
    let wb = Whiteboard { id: 4, name: "W".into(), strokes: vec![], notes: vec![], owner: 0 };
    let new = fp(serde_json::json!({"dashboards": [dash], "forms": [form], "whiteboards": [wb]}));
    // as an app from before these fields wrote them
    let old = fp(serde_json::json!({"dashboards": [{"id": 1, "name": "D", "space": null}], "forms": [{"id": 2, "name": "F", "list_id": 3, "description": "", "fields": []}], "whiteboards": [{"id": 4, "name": "W", "strokes": [], "notes": []}]}));
    assert_eq!(new, old);
    let e = serde_json::to_value(TimeEntry { start: 1, end: 2, member: 0 }).unwrap();
    assert_eq!(e, serde_json::json!({"start": 1, "end": 2}));

    // fingerprints saved while these fields were always written don't make
    // everything look edited after updating
    let dir = temp_folder("shape");
    let mut a = app();
    a.data.extra.dashboards.push(Dashboard { id: 7001, name: "Old".into(), ..Default::default() });
    a.data.tasks[0].time_entries.push(TimeEntry { start: 1, end: 2, member: 0 });
    a.share_here(dir.to_str().unwrap()).unwrap();
    let v = serde_json::to_value(&a.data).unwrap();
    a.data.extra.sync.hashes = crate::sync::fingerprints(&together::legacy_shape(&v));
    a.data.extra.sync.v = 1;
    let stamps = a.data.extra.sync.stamps.clone();
    a.sync_note_change();
    assert_eq!(a.data.extra.sync.stamps, stamps, "nothing counts as edited");
    let _ = std::fs::remove_dir_all(&dir);
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
    a.data.spaces[si].statuses = vec![Status::new("idea", [1, 1, 1], StatusKind::Open), Status::new("shipped", [2, 2, 2], StatusKind::Closed)];
    let n = add_task(&mut a, ideas, "Big idea");
    a.data.tasks.iter_mut().find(|t| t.id == n).unwrap().status = "shipped".into();
    a.do_delete(Target::Space(sid));
    assert!(!a.data.spaces.iter().any(|s| s.id == sid));
    let i = a.data.extra.trash.iter().position(|x| x.kind == "Space").expect("the space is in Trash");
    a.restore_trash(i);
    let sp = a.data.spaces.iter().find(|s| s.id == sid).expect("the space is back");
    assert_eq!((sp.color, sp.icon.as_str(), sp.statuses.len()), ([9, 8, 7], "*", 2));
    assert!(sp.lists.iter().any(|l| l.id == ideas), "with its list");
    assert_eq!(a.task(n).unwrap().status, "shipped");
    assert!(!a.data.extra.trash.iter().any(|x| x.kind == "List" && x.title == "Ideas"));

    // a folder: one list restored on its own first goes to the space; the
    // folder then comes back with the other one in it
    let fid = a.data.spaces[0].folders[0].id;
    let (web, mkt) = (a.data.spaces[0].folders[0].lists[0].id, a.data.spaces[0].folders[0].lists[1].id);
    a.do_delete(Target::Folder(fid));
    let i = a.data.extra.trash.iter().position(|x| x.kind == "List" && x.title == "Website Redesign").unwrap();
    a.restore_trash(i);
    assert!(a.data.spaces[0].lists.iter().any(|l| l.id == web));
    let i = a.data.extra.trash.iter().position(|x| x.kind == "Folder").expect("the folder is in Trash");
    a.restore_trash(i);
    let f = a.data.spaces[0].folders.iter().find(|f| f.id == fid).expect("the folder is back");
    assert_eq!(f.lists.iter().map(|l| l.id).collect::<Vec<_>>(), vec![mkt]);
}

#[test]
fn a_list_restored_into_another_space_takes_that_spaces_statuses() {
    let mut a = app();
    let sid = a.data.spaces[1].id;
    let ideas = a.data.spaces[1].lists[0].id;
    a.data.spaces[1].statuses = vec![Status::new("idea", [1, 1, 1], StatusKind::Open), Status::new("doing", [2, 2, 2], StatusKind::Custom), Status::new("shipped", [3, 3, 3], StatusKind::Closed), Status::new("to do", [4, 4, 4], StatusKind::Custom)];
    let mut ids = vec![];
    for (name, status) in [("one", "idea"), ("two", "doing"), ("three", "shipped"), ("four", "to do")] {
        let t = add_task(&mut a, ideas, name);
        a.data.tasks.iter_mut().find(|x| x.id == t).unwrap().status = status.into();
        ids.push(t);
    }
    a.do_delete(Target::List(ideas));
    a.data.spaces.retain(|s| s.id != sid);
    let i = a.data.extra.trash.iter().position(|x| x.kind == "List").unwrap();
    a.restore_trash(i);
    assert!(a.data.spaces[0].lists.iter().any(|l| l.id == ideas), "into the team space");
    let st: Vec<String> = ids.iter().map(|t| a.task(*t).unwrap().status.clone()).collect();
    assert_eq!(st, vec!["to do", "in progress", "complete", "to do"], "same name, else the same kind");
    assert!(a.task(ids[1]).unwrap().activity.iter().any(|x| x.text.contains("changed status from DOING to IN PROGRESS")));
}

#[test]
fn a_restored_list_skips_archived_and_other_peoples_spaces() {
    let mut a = app();
    let ideas = a.data.spaces[1].lists[0].id;
    let sid = a.data.spaces[1].id;
    a.do_delete(Target::List(ideas));
    a.data.spaces.retain(|s| s.id != sid);
    a.data.spaces[0].archived = true;
    a.data.spaces.insert(0, Space { id: 7100, name: "Alex's".into(), private_owner: Some(2), statuses: default_statuses(), ..Default::default() });
    a.data.spaces.push(Space { id: 7200, name: "Fresh".into(), statuses: default_statuses(), ..Default::default() });
    let i = a.data.extra.trash.iter().position(|x| x.kind == "List").unwrap();
    a.restore_trash(i);
    assert!(a.data.spaces.iter().find(|s| s.id == 7200).unwrap().lists.iter().any(|l| l.id == ideas));
}

#[test]
fn on_a_server_what_i_may_no_longer_see_leaves_my_copy_without_deleting_it() {
    let mut a = app();
    a.data.extra.shared = Some(crate::sync::SharedCfg { folder: String::new(), device: "d".into(), tag: 1, server: Some("127.0.0.1:1".into()) });
    let me = a.data.me;
    let ch = a.data.channels.iter().find(|c| c.kind == ChannelKind::Channel && c.name != "general").unwrap().id;
    // a DM between two other people, kept from an older version
    a.data.channels.push(Channel { id: 7300, kind: ChannelKind::Direct, members: vec![2, 3], ..Default::default() });
    a.sync_note_change();
    // I write in the channel just as I'm taken out of it
    a.data.channels.iter_mut().find(|c| c.id == ch).unwrap().messages.push(Message { id: 7301, author: me, text: "my last words".into(), ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: None, attachments: vec![], edited: None });
    let mut team = serde_json::to_value(&a.data).unwrap();
    crate::sync::hide(&mut team, &[ch, 7300]);
    a.merge_in(&[team], &[ch]);
    assert!(!a.data.channels.iter().any(|c| c.id == ch || c.id == 7300));
    assert!(a.data.extra.notepad.contains("my last words"), "what I wrote is kept: {:?}", a.data.extra.notepad);
    assert!(a.toast.is_some());
    a.sync_note_change();
    let m = &a.data.extra.sync;
    assert!(!m.tombs.contains_key(&ch) && !m.tombs.contains_key(&7300) && !m.tombs.contains_key(&7301), "nothing deleted for the team");
}


#[test]
fn an_older_servers_unnamed_refusal_ends_a_waiting_huddle() {
    let mut a = app();
    let ch = a.data.channels[0].id;
    let now = std::time::Instant::now();
    let waiting = |asked: std::time::Instant| {
        let mut h = huddle::HuddleRt::default();
        (h.channel, h.waiting, h.asked) = (Some(ch), true, Some(asked));
        h
    };
    // only the call_join went out: its refusal ends the huddle at once
    a.huddle = waiting(now);
    a.sync_rt.other_asked = None;
    a.huddle_denied_unnamed();
    assert!(a.huddle.channel.is_none() && !a.huddle.waiting);
    // another request went just before it: the refusal may be that one's, keep waiting
    a.huddle = waiting(now);
    a.sync_rt.other_asked = Some(now - std::time::Duration::from_secs(1));
    a.huddle_denied_unnamed();
    assert!(a.huddle.waiting && a.huddle.channel == Some(ch));
    // one sent after the call_join is answered after it, and one long before is done
    a.sync_rt.other_asked = Some(now + std::time::Duration::from_millis(5));
    a.huddle_denied_unnamed();
    assert!(a.huddle.channel.is_none());
    a.huddle = waiting(now);
    a.sync_rt.other_asked = Some(now - std::time::Duration::from_secs(huddle::HUDDLE_JOIN_SECS + 1));
    a.huddle_denied_unnamed();
    assert!(a.huddle.channel.is_none());
    // not waiting for a huddle: nothing to end
    a.huddle = huddle::HuddleRt::default();
    a.huddle_denied_unnamed();
    assert!(a.huddle.channel.is_none());
}


#[test]
fn seen_ids_keep_the_newest_and_cover_from_the_oldest_of_them() {
    let few = SeenIds::of((1..=5).map(|i| (i, i as i64 * 10)));
    assert_eq!(few.ids, vec![1, 2, 3, 4, 5]);
    assert!(few.is_new(99, -1_000_000), "with few messages everything is covered");
    let many = SeenIds::of((1..=400u64).map(|i| (i, i as i64)));
    assert_eq!(many.ids.len(), SeenIds::MAX);
    assert_eq!(many.floor, 101);
    assert!(many.has(400) && !many.has(100));
    assert!(many.is_new(999, 150) && !many.is_new(999, 50), "only inside what it covers");
}

#[test]
fn a_late_message_or_one_from_a_clock_behind_counts_as_unread() {
    let mut a = app();
    let cid = a.data.channels.iter().find(|c| c.name == "general").unwrap().id;
    a.mark_read(cid);
    let get = |a: &App| a.data.channels.iter().find(|c| c.id == cid).unwrap().clone();
    assert_eq!(a.unread(&get(&a)), 0);
    // written an hour ago by its author's clock, arriving only now
    let id = a.data.new_id();
    let behind = get(&a).last_read - 3600;
    a.data.channels.iter_mut().find(|c| c.id == cid).unwrap().messages.push(test_msg(id, 2, "late", behind));
    assert_eq!(a.unread(&get(&a)), 1, "a message not seen yet is new whatever its time");
    a.mark_read(cid);
    assert_eq!(a.unread(&get(&a)), 0);
    // "Mark unread" from a message on brings it and the later ones back
    let ts = get(&a).messages.iter().filter(|m| m.author != 1).map(|m| m.ts).min().unwrap();
    a.apply_chat(vec![ChatAction::MarkUnread { ch: cid, ts }]);
    let others = get(&a).messages.iter().filter(|m| m.author != 1 && m.ts >= ts).count();
    assert_eq!(a.unread(&get(&a)), others);
}

#[test]
fn a_closed_dm_comes_back_for_a_new_message_from_a_clock_behind() {
    let mut a = app();
    let dm = a.data.channels.iter().find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2]).unwrap().id;
    let get = |a: &App| a.data.channels.iter().find(|c| c.id == dm).unwrap().clone();
    a.close_dm(dm);
    assert!(!a.listed(&get(&a)));
    let id = a.data.new_id();
    a.data.channels.iter_mut().find(|c| c.id == dm).unwrap().messages.push(test_msg(id, 2, "earlier by my clock", now_ts() - 86_400));
    assert!(a.listed(&get(&a)), "a message that wasn't there when it was closed brings it back");
}

#[test]
fn two_dms_that_both_have_messages_show_as_one_conversation() {
    let mut a = app();
    let older = a.data.channels.iter().find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2]).unwrap().id;
    let newer = a.data.new_id();
    let mid = a.data.new_id();
    a.data.channels.push(Channel { id: newer, kind: ChannelKind::Direct, members: vec![2, 1], messages: vec![test_msg(mid, 2, "in the other copy", now_ts())], ..Default::default() });
    let get = |a: &App, id: u64| a.data.channels.iter().find(|c| c.id == id).unwrap().clone();
    assert!(a.listed(&get(&a, older)));
    assert!(!a.listed(&get(&a, newer)));
    assert_eq!(a.conversation(&get(&a, older)).len(), 2);
    assert!(a.unread(&get(&a, older)) >= 1, "the other copy's messages count in the one shown");
    // acting on a message from the other copy changes it where it is
    a.apply_chat(vec![ChatAction::React { ch: older, msg: mid, emoji: "\u{1F44D}".into() }]);
    assert_eq!(get(&a, newer).messages[0].reactions.len(), 1);
    // new messages go to the older one; reading it reads both
    a.apply_chat(vec![ChatAction::Send { ch: older, text: "hello".into(), parent: None, files: vec![] }]);
    assert!(get(&a, older).messages.iter().any(|m| m.text == "hello"));
    a.mark_read(older);
    assert_eq!(a.unread(&get(&a, older)), 0);
    assert!(a.data.extra.chat_seen.get(&newer).is_some_and(|s| s.has(mid)));
}

#[test]
fn only_a_channels_maker_or_an_admin_may_delete_it() {
    let mut a = app();
    a.apply_chat(vec![ChatAction::NewChannel("mine".into())]);
    let c = a.data.channels.iter().find(|c| c.name == "mine").unwrap().clone();
    assert_eq!(c.creator, a.data.me);
    assert!(a.may_manage_channel(&c));
    let mut theirs = c.clone();
    theirs.creator = 2;
    assert!(!a.may_manage_channel(&theirs), "a member who didn't make it");
    a.sync_rt.role = Some(wb_net::Role::Admin);
    assert!(a.may_manage_channel(&theirs), "the server's admins may");
    a.sync_rt.role = Some(wb_net::Role::Member);
    let mut old = theirs.clone();
    old.creator = 0;
    assert!(a.may_manage_channel(&old), "from before makers were kept: any member");
    old.members.retain(|m| *m != a.data.me);
    assert!(!a.may_manage_channel(&old));
    // the maker isn't saved when there is none, so older copies stay the same
    assert!(!serde_json::to_string(&old).unwrap().contains("creator"));
}

#[test]
fn a_mention_needs_the_whole_name() {
    use super::chat::mentions;
    assert!(mentions("hi @Bob", "Bob"));
    assert!(mentions("@Bob, look", "Bob"));
    assert!(mentions("(cc @Bob)", "Bob"));
    assert!(!mentions("hi @Bobby", "Bob"));
    assert!(!mentions("hi @Bob", "Bo"), "@Bo is not the start of @Bob");
    assert!(mentions("hi @Alex Kim!", "Alex Kim"));
    assert!(!mentions("hi @Bob", ""));
}

#[test]
fn a_failed_upload_for_a_sent_message_is_tried_again_and_not_cancelled() {
    let mut a = app();
    a.add_media(false, Some(picture_file("again.png", 16, 16)), None);
    let att = until_ready(&mut a, false).remove(0);
    let sha = att.file.clone();
    let remote = super::files::Remote::Server { addr: "127.0.0.1:9".into(), auth: wb_net::Auth { device: "d".into(), token: "t".into() } };
    a.files.upload_soon(&att, &remote);
    // removing the same file from a message box doesn't stop it
    a.files.cancel_upload(&sha);
    a.files.settle(&remote, |f| matches!(f.state(&sha), Some(super::files::Transfer::Failed(_))) && f.upload_tries(&sha) == Some((1, true)));
    a.files.retry_due(&sha);
    a.files.settle(&remote, |f| f.upload_tries(&sha).is_some_and(|(n, _)| n == 2));
}

#[test]
fn a_picture_added_before_sharing_goes_up_when_the_message_is_sent() {
    let mut a = app();
    a.add_media(false, Some(picture_file("early.png", 16, 16)), None);
    let files = until_ready(&mut a, false);
    let sha = files[0].file.clone();
    // now the workspace is on a server this computer is signed in to
    a.data.extra.shared = Some(crate::sync::SharedCfg { folder: String::new(), device: "dev-early".into(), tag: 1, server: Some("127.0.0.1:9".into()) });
    super::together::save_token("127.0.0.1:9", "dev-early", "tok");
    let ch = a.data.channels[0].id;
    a.apply_chat(vec![ChatAction::Send { ch, text: "made before sharing".into(), parent: None, files }]);
    super::together::forget_token("127.0.0.1:9", "dev-early");
    assert!(a.files.upload_tries(&sha).is_some(), "it is sent to the server (and tried again until it's there)");
}

#[test]
fn file_errors_are_shown_in_the_interface_language() {
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let shown = App::error_text("cat.gif isn't a picture or a video.");
    let net = App::error_text("Couldn't reach the server: refused");
    let plain = App::error_text("The server doesn't have this file.");
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(shown, "cat.gif은(는) 사진이나 동영상이 아닙니다.");
    assert_eq!(net, "서버에 연결하지 못했습니다: refused");
    assert_eq!(plain, "서버에 이 파일이 없습니다.");
    assert_eq!(App::error_text("Couldn't put a.png in the shared folder: full"), "Couldn't put a.png in the shared folder: full");
}


#[test]
fn timesheet_typing_or_dragging_keeps_one_entry_per_day() {
    let mut a = app();
    let id = a.data.tasks[1].id;
    let day = today();
    a.data.tasks.iter_mut().find(|t| t.id == id).unwrap().time_entries.clear();
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
    assert_eq!(a.my_sheet_rows(ws + Duration::days(7)), vec![ids[0]], "an old row shows in every week");
    a.drop_sheet_row(ws + Duration::days(7), ids[0]);
    assert_eq!(a.my_sheet_rows(ws), vec![ids[1]], "removing an old row takes it off every week");
    // saved files without week rows stay the same
    let mut e = a.data.extra.clone();
    e.sheet_week_rows.clear();
    assert!(!serde_json::to_string(&e).unwrap().contains("sheet_week_rows"));
}

#[test]
fn each_person_runs_a_timer_on_the_same_task() {
    let mut a = app();
    let id = a.data.tasks[0].id;
    let started = now_ts() - 600;
    let t = a.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
    t.timer_started = Some(started);
    t.timer_by = Some(2);
    assert!(!serde_json::to_string(&a.data.tasks[0]).unwrap().contains("\"timers\""), "no new field when one timer runs");
    a.apply(Action::ToggleTimer(id));
    let t = a.task(id).unwrap();
    assert!(a.data.is_my_timer(t));
    assert_eq!((t.timer_started, t.timer_by), (Some(started), Some(2)), "Alex's timer stays where older apps read it");
    assert!(t.tracked_secs(now_ts()) >= 600);
    // Alex stops his: mine moves into the field older apps read
    a.data.me = 2;
    a.apply(Action::ToggleTimer(id));
    let t = a.task(id).unwrap();
    assert_eq!(t.timer_by, Some(1));
    assert!(t.timers.is_empty());
    assert!(t.time_entries.iter().any(|e| e.member == 2 && e.start == started));
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
    let other = a.data.spaces.iter().find(|s| s.id != sid && s.statuses.iter().any(|x| x.name == "in progress")).map(|s| s.id).expect("another space shares the name");
    let rule = |id, list| Automation { id, enabled: true, trigger_status: "in progress".into(), action: "priority".into(), value: "1".into(), list };
    a.data.extra.automations = vec![rule(1, Some(list)), rule(2, Some(sid)), rule(3, None), rule(4, Some(other))];
    let mut orig: Vec<Option<String>> = a.data.spaces[si].statuses.iter().map(|s| Some(s.name.clone())).collect();
    let i = a.data.spaces[si].statuses.iter().position(|s| s.name == "in progress").unwrap();
    a.data.spaces[si].statuses[i].name = "doing".into();
    a.commit_status_renames(si, &mut orig);
    let trig: Vec<String> = a.data.extra.automations.iter().map(|x| x.trigger_status.clone()).collect();
    assert_eq!(trig, ["doing", "doing", "in progress", "in progress"], "this space's rules follow; workspace-wide and other spaces' stay");
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
    a.data.extra.automations.push(Automation { id: 9, enabled: true, trigger_status: "in progress".into(), action: "assign".into(), value: "2".into(), list: None });
    let mut t = a.task(id).unwrap().clone();
    t.status = "in progress".into();
    a.apply(Action::Replace(t));
    assert_eq!(a.task(id).unwrap().assignees, vec![2]);
    // My Tasks grouped by assignee: adding in Alex's group makes it mine only
    a.loc = Location::MyTasks;
    let g = Group { key: "a:2".into(), label: String::new(), color: Color32::WHITE, status: None, priority: None, assignee: Some(2), due: None, tasks: vec![] };
    a.add_from_group(&g, "From Alex's group".into(), false);
    a.apply_actions();
    let t = a.data.tasks.iter().find(|t| t.name == "From Alex's group").unwrap();
    assert_eq!(t.assignees, vec![a.data.me]);
}


#[test]
fn joining_a_shared_workspace_forgets_history() {
    let dir = temp_folder("join-history");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(Data::blank("Theirs", Member { id: 1, name: "B".into(), ..Default::default() })));
    b.apply(Action::Go(Location::Inbox));
    b.track_history();
    b.apply(Action::Go(Location::Home));
    b.track_history();
    assert!(!b.nav_back.is_empty());
    b.join_shared(&folder).unwrap();
    assert!(b.nav_back.is_empty() && b.nav_fwd.is_empty(), "Back doesn't lead into the old workspace");
    let _ = std::fs::remove_dir_all(&dir);
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
    assert_eq!((a.section, a.dash_open), (Section::Dashboards, Some(super::hubs::WORKSPACE_OVERVIEW)));
}

#[test]
fn links_to_archived_or_private_places_do_not_open() {
    let mut a = app();
    let me = a.data.me;
    let lists = list_ids(&a);
    let tid = add_task(&mut a, lists[0], "Behind a link");
    // someone else's Personal space
    let sid = a.data.space_of_list(lists[0]).unwrap().id;
    a.data.spaces.iter_mut().find(|s| s.id == sid).unwrap().private_owner = Some(me + 100);
    a.toast = None;
    assert!(!a.open_link(&format!("workboard://space/{}", sid)));
    assert!(!a.open_link(&format!("workboard://list/{}", lists[0])));
    assert!(!a.open_link(&format!("workboard://task/{}", tid)));
    a.apply_actions();
    assert_eq!(a.open_task, None);
    assert!(a.toast.as_ref().is_some_and(|(t, _)| t.contains("archived or private")));
    assert!(a.search_rows(&format!("workboard://task/{}", tid), 0).iter().all(|r| r.4 != super::more::Hit::Task(tid)));
    // my own Personal space opens
    a.data.spaces.iter_mut().find(|s| s.id == sid).unwrap().private_owner = Some(me);
    assert!(a.open_link(&format!("workboard://list/{}", lists[0])));
    // an archived space doesn't
    a.data.spaces.iter_mut().find(|s| s.id == sid).unwrap().archived = true;
    assert!(!a.open_link(&format!("workboard://space/{}", sid)));
    assert!(!a.open_link(&format!("workboard://list/{}", lists[0])));
}

#[test]
fn search_leaves_out_archived_lists_and_other_peoples_private_docs() {
    let mut a = app();
    let me = a.data.me;
    let lists = list_ids(&a);
    add_task(&mut a, lists[0], "Zebra crossing");
    let found = |a: &App, q: &str| a.search_rows(q, 0).iter().filter(|r| r.0 != "Recent searches").map(|r| r.2.clone()).collect::<Vec<_>>();
    assert!(found(&a, "zebra").iter().any(|t| t == "Zebra crossing"));
    a.data.list_item_mut(lists[0]).unwrap().archived = true;
    assert!(!found(&a, "zebra").iter().any(|t| t == "Zebra crossing"), "a task in an archived list stays out");
    let now = now_ts();
    let space = a.data.spaces.iter().find(|s| s.private_owner.is_none() && !s.archived).map(|s| s.id);
    for (id, owner, private, title) in [(9101, me + 100, true, "Quokka secret"), (9102, me, true, "Quokka mine"), (9103, me + 100, false, "Quokka shared")] {
        a.data.docs.push(Doc { id, space, owner, private, archived: false, meeting_note: false, parent: None, title: title.into(), body: String::new(), created: now, updated: now, tags: vec![] });
    }
    let docs = found(&a, "quokka");
    assert!(!docs.iter().any(|t| t == "Quokka secret"));
    assert!(docs.iter().any(|t| t == "Quokka mine") && docs.iter().any(|t| t == "Quokka shared"));
    let d = a.data.docs.iter().find(|d| d.id == 9101).unwrap();
    assert!(!a.doc_shown(d));
}

#[test]
fn comments_on_a_task_i_duplicated_go_to_primary() {
    let mut a = app();
    let me = a.data.me;
    let lists = list_ids(&a);
    let src = add_task(&mut a, lists[0], "Original");
    let copy = a.duplicate_task(src).unwrap();
    let other = a.data.members.iter().map(|m| m.id).find(|id| *id != me).unwrap();
    let t = a.data.tasks.iter_mut().find(|t| t.id == copy).unwrap();
    t.assignees.clear();
    t.activity.push(Activity { ts: now_ts() + 5, author: other, text: "looks good".into(), comment: true, ..Default::default() });
    let row = a.inbox_items().into_iter().find(|i| i.src == super::nav::InboxSrc::Task(copy) && i.text == "looks good").unwrap();
    assert_eq!(row.tab, 0);
}


/// Runs `f` as another member, then switches back.
fn as_member<R>(a: &mut App, who: u64, f: impl FnOnce(&mut App) -> R) -> R {
    let me = a.data.me;
    a.data.me = who;
    let r = f(a);
    a.data.me = me;
    r
}

/// The tasks with an "assigned" row in Primary for the person using the app.
fn assigned_in_primary(a: &App) -> Vec<u64> {
    a.inbox_items()
        .into_iter()
        .filter(|i| i.assigned && i.tab == 0)
        .filter_map(|i| match i.src {
            nav::InboxSrc::Task(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn mentioned_in_primary(a: &App) -> Vec<String> {
    a.inbox_items().into_iter().filter(|i| i.mention && i.tab == 0).map(|i| i.text).collect()
}

fn quiet_app() -> App {
    let mut a = app();
    a.data.tasks.clear();
    for c in &mut a.data.channels {
        c.messages.clear();
    }
    a
}

#[test]
fn tasks_made_or_copied_with_me_on_them_reach_my_inbox() {
    let mut a = quiet_app();
    let list = list_ids(&a)[0];
    // Alex Kim (2) makes a task with me (1) on it, and copies another one
    let made = as_member(&mut a, 2, |a| {
        a.apply(Action::Add { list, name: "Made for you".into(), status: None, priority: 0, assignees: vec![1, 2], due: None, parent: None, open: false });
        a.data.tasks.iter().find(|t| t.name == "Made for you").unwrap().id
    });
    let copy = as_member(&mut a, 2, |a| a.duplicate_task(made).unwrap());
    let rows = assigned_in_primary(&a);
    assert!(rows.contains(&made), "a task made with me on it: {:?}", rows);
    assert!(rows.contains(&copy), "a copy of a task with me on it: {:?}", rows);
    // the maker putting themselves on it says nothing to them
    as_member(&mut a, 2, |a| assert!(assigned_in_primary(a).is_empty()));
    // a task I make for myself is not news to me
    a.apply(Action::Add { list, name: "Mine".into(), status: None, priority: 0, assignees: vec![1], due: None, parent: None, open: false });
    assert_eq!(assigned_in_primary(&a).len(), 2);
}

#[test]
fn automation_assignments_reach_the_inbox() {
    let mut a = quiet_app();
    let list = list_ids(&a)[0];
    a.data.extra.automations.push(Automation { id: 1, enabled: true, trigger_status: "complete".into(), action: "assign".into(), value: "1".into(), list: None });
    let id = add_task(&mut a, list, "Finish me");
    a.data.tasks.iter_mut().find(|t| t.id == id).unwrap().activity.iter_mut().for_each(|x| x.author = 2);
    as_member(&mut a, 2, |a| {
        let mut t = a.task(id).unwrap().clone();
        t.status = "complete".into();
        a.apply(Action::Replace(t));
    });
    assert!(a.task(id).unwrap().assignees.contains(&1));
    assert_eq!(assigned_in_primary(&a), vec![id]);
    // the automation's own line stays as it was
    assert!(a.task(id).unwrap().activity.iter().any(|x| x.text.starts_with("Automation ran: ")));
}

#[test]
fn a_mention_of_alex_kim_is_not_a_mention_of_alex() {
    let mut a = quiet_app();
    a.data.members.push(Member { id: 5, name: "Alex".into(), ..Default::default() });
    let cid = a.data.channels.iter().find(|c| c.kind == ChannelKind::Channel).unwrap().id;
    // sent from the app, and one from a file written before ids were kept
    as_member(&mut a, 3, |a| a.apply_chat(vec![ChatAction::Send { ch: cid, text: "@Alex Kim new one".into(), parent: None, files: vec![] }]));
    a.data.channels.iter_mut().find(|c| c.id == cid).unwrap().messages.push(chat_msg(960, 3, "@Alex Kim old one", now_ts() - 5));
    let list = list_ids(&a)[0];
    let tid = add_task(&mut a, list, "Talk");
    a.data.tasks.iter_mut().find(|t| t.id == tid).unwrap().activity.push(Activity { ts: now_ts() - 4, author: 3, text: "@Alex Kim on a task".into(), comment: true, ..Default::default() });
    as_member(&mut a, 5, |a| assert!(mentioned_in_primary(a).is_empty(), "Alex: {:?}", mentioned_in_primary(a)));
    as_member(&mut a, 2, |a| {
        let mut got = mentioned_in_primary(a);
        got.sort();
        assert_eq!(got, vec!["@Alex Kim new one", "@Alex Kim old one", "@Alex Kim on a task"]);
    });
    // "@Alex" alone still reaches Alex, and a Korean ending may follow a name
    as_member(&mut a, 3, |a| a.apply_chat(vec![ChatAction::Send { ch: cid, text: "@Alex, and @Alex님".into(), parent: None, files: vec![] }]));
    as_member(&mut a, 5, |a| assert_eq!(mentioned_in_primary(a), vec!["@Alex, and @Alex님"]));
    as_member(&mut a, 2, |a| assert_eq!(mentioned_in_primary(a).len(), 3));
}

#[test]
fn same_names_and_renames_do_not_mix_people_up() {
    let mut a = quiet_app();
    a.data.members.push(Member { id: 6, name: "Sam Lee".into(), ..Default::default() });
    let list = list_ids(&a)[0];
    let tid = add_task(&mut a, list, "Shared name");
    // Alex Kim puts the second Sam Lee (6) on the task
    as_member(&mut a, 2, |a| {
        let mut t = a.task(tid).unwrap().clone();
        t.assignees.push(6);
        a.apply(Action::Replace(t));
    });
    as_member(&mut a, 6, |a| assert_eq!(assigned_in_primary(a), vec![tid]));
    as_member(&mut a, 3, |a| assert!(assigned_in_primary(a).is_empty(), "the other Sam Lee"));
    // Jordan Park (4) is assigned and mentioned, then renamed
    let cid = a.data.channels.iter().find(|c| c.kind == ChannelKind::Channel).unwrap().id;
    let tid2 = add_task(&mut a, list, "For Jordan");
    as_member(&mut a, 2, |a| {
        let mut t = a.task(tid2).unwrap().clone();
        t.assignees.push(4);
        a.apply(Action::Replace(t));
        a.apply_chat(vec![ChatAction::Send { ch: cid, text: "@Jordan Park please look".into(), parent: None, files: vec![] }]);
    });
    a.data.members.iter_mut().find(|m| m.id == 4).unwrap().name = "Jordan P".into();
    as_member(&mut a, 4, |a| {
        assert_eq!(assigned_in_primary(a), vec![tid2]);
        assert_eq!(mentioned_in_primary(a), vec!["@Jordan Park please look"]);
    });
}

#[test]
fn mentioned_ids_take_the_longest_name_and_honour_picks() {
    let m = |id: u64, name: &str| Member { id, name: name.into(), ..Default::default() };
    let people = vec![m(1, "Alex"), m(2, "Alex Kim"), m(3, "지원"), m(4, "Sam"), m(5, "Sam")];
    assert_eq!(mentioned_ids("@Alex Kim hi", &people, &[]), vec![2]);
    assert_eq!(mentioned_ids("@Alex hi", &people, &[]), vec![1]);
    assert!(mentioned_ids("@Alexander hi", &people, &[]).is_empty(), "a longer word is someone else");
    assert!(mentioned_ids("mail@Alex", &people, &[]).is_empty(), "@ inside a word");
    assert_eq!(mentioned_ids("@지원님 확인 부탁", &people, &[]), vec![3]);
    assert_eq!(mentioned_ids("@Sam", &people, &[]), vec![4, 5], "a shared name with no pick");
    assert_eq!(mentioned_ids("@Sam", &people, &[5]), vec![5], "the picked one");
    assert_eq!(mentioned_ids("@Alex and @Alex Kim and @Alex", &people, &[]), vec![1, 2]);
}

#[test]
fn files_from_before_ids_were_kept_still_load_and_new_fields_stay_out_when_empty() {
    // an activity line and a chat message as an older app writes them
    let act: Activity = serde_json::from_str(r#"{"ts":5,"author":2,"text":"assigned to Me","comment":false}"#).unwrap();
    assert_eq!((act.target, act.mentions.is_empty()), (None, true));
    let msg: Message = serde_json::from_str(r#"{"id":9,"author":2,"text":"@Me hi","ts":5}"#).unwrap();
    assert!(msg.mentions.is_empty());
    // empty ones are not written, so files only change where ids are known
    let out = serde_json::to_value(&act).unwrap();
    assert!(out.get("target").is_none() && out.get("mentions").is_none());
    assert!(serde_json::to_value(&msg).unwrap().get("mentions").is_none());
    // and they round-trip when set
    let a2 = Activity { target: Some(4), mentions: vec![2, 3], ..act.clone() };
    let back: Activity = serde_json::from_value(serde_json::to_value(&a2).unwrap()).unwrap();
    assert_eq!((back.target, back.mentions), (Some(4), vec![2, 3]));
    // a whole workspace saved by an older app reads, and its inbox still works by name
    let mut old = serde_json::to_value(Data::seed()).unwrap();
    let t0 = &mut old["tasks"][0];
    t0["assignees"] = serde_json::json!([]);
    t0["activity"] = serde_json::json!([{"ts": 5, "author": 2, "text": "assigned to Me", "comment": false}]);
    let d: Data = serde_json::from_value(old).unwrap();
    let a = App::build(None, Some(d));
    assert!(a.inbox_items().iter().any(|i| i.assigned && i.text == "assigned to Me"));
}
