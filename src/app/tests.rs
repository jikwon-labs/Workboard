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
    a.apply_chat(vec![ChatAction::Send { ch: cid, text: "Hello team".into(), parent: None }]);
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
    a.apply_chat(vec![ChatAction::Send { ch: cid, text: "Reply".into(), parent: Some(mid) }]);
    assert_eq!(a.data.channels.iter().find(|c| c.id == cid).unwrap().messages.last().unwrap().replies.len(), 1);
    // post
    a.apply_chat(vec![ChatAction::SendPost { ch: cid, title: "Launch".into(), text: "We ship Friday".into() }]);
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
    b.apply_chat(vec![ChatAction::Send { ch, text: "hi from Bora".into(), parent: None }]);
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
