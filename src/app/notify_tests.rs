// Notification tests: a teammate's change lands in the data (as it does when
// the team's changes are merged in), then `notify_scan` / `notify_clock`
// decide what to announce. `notify_rt.sent` records every announcement.

use super::notify::Kind;
use super::tests::channel_id;
use super::*;
use crate::app::nav::InboxTab;

fn app() -> App {
    let mut a = super::tests::app();
    a.notify_scan(); // the first look only remembers what is there
    a.notify_rt.sent.clear();
    a
}

fn msg(id: u64, author: u64, text: &str, ts: i64) -> Message {
    serde_json::from_value(serde_json::json!({"id": id, "author": author, "text": text, "ts": ts}))
        .unwrap()
}

fn dm_with_alex(a: &App) -> u64 {
    a.data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id
}

fn post(a: &mut App, ch: u64, author: u64, text: &str) -> u64 {
    let id = a.data.new_id();
    let m = msg(id, author, text, now_ts());
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == ch)
        .unwrap()
        .messages
        .push(m);
    id
}

fn kinds(a: &App) -> Vec<Kind> {
    a.notify_rt.sent.iter().map(|n| n.kind).collect()
}

fn my_task(a: &mut App, name: &str) -> u64 {
    let list = a.data.first_list().unwrap();
    a.apply(Action::Add {
        list,
        name: name.into(),
        status: None,
        priority: 0,
        assignees: vec![1],
        due: None,
        parent: None,
        open: false,
    });
    a.data.tasks.iter().find(|t| t.name == name).unwrap().id
}

fn teammate_line(a: &mut App, task: u64, author: u64, text: &str, comment: bool) {
    let t = a.data.tasks.iter_mut().find(|t| t.id == task).unwrap();
    t.activity.push(Activity {
        ts: now_ts(),
        author,
        text: text.into(),
        comment,
        ..Default::default()
    });
}

#[test]
fn what_was_there_at_the_start_is_not_news() {
    let a = app();
    assert!(a.notify_rt.sent.is_empty());
    assert!(a.notify_rt.seeded);
}

#[test]
fn a_direct_message_is_announced_once_and_opens_the_conversation() {
    let mut a = app();
    let dm = dm_with_alex(&a);
    post(&mut a, dm, 2, "Can you look at the draft?");
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Direct]);
    let n = a.notify_rt.sent[0].clone();
    assert_eq!(n.title, "Alex Kim");
    assert_eq!(n.body, "Can you look at the draft?");
    assert_eq!(n.target, format!("chat:{}", dm));
    a.notify_scan();
    assert_eq!(a.notify_rt.sent.len(), 1, "announced once");
    // the toast opens it
    assert!(a
        .toast
        .as_ref()
        .unwrap()
        .0
        .contains("Can you look at the draft?"));
    let target = n.target.clone();
    a.open_target(&target);
    assert_eq!(a.section, Section::Chat);
    assert_eq!(a.chat.open, Some(dm));
}

#[test]
fn my_own_messages_are_never_news() {
    let mut a = app();
    let dm = dm_with_alex(&a);
    post(&mut a, dm, 1, "Note to self");
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty());
}

#[test]
fn channels_notify_mentions_by_default_and_follow_their_bell() {
    let mut a = app();
    let g = channel_id(&a, "general");
    post(&mut a, g, 3, "Lunch at noon");
    a.notify_scan();
    assert!(
        a.notify_rt.sent.is_empty(),
        "plain channel messages are quiet by default"
    );
    post(&mut a, g, 3, "@Me can you check this?");
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Mention]);
    // every message
    a.data.extra.notify.set_channel_mode(g, "all");
    post(&mut a, g, 4, "New build is up");
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Mention, Kind::Channel]);
    assert_eq!(a.notify_rt.sent[1].title, "Jordan Park in # general");
    // nothing at all, not even mentions
    a.data.extra.notify.set_channel_mode(g, "nothing");
    post(&mut a, g, 4, "@Me ping");
    a.notify_scan();
    assert_eq!(a.notify_rt.sent.len(), 2);
    // back to the default: the channel no longer stands apart
    a.data.extra.notify.set_channel_mode(g, "mentions");
    assert!(a.data.extra.notify.channel_modes.is_empty());
}

#[test]
fn a_reply_in_my_thread_is_announced_with_the_thread() {
    let mut a = app();
    let g = channel_id(&a, "general");
    let mine = post(&mut a, g, 1, "Who has the slides?");
    a.notify_scan();
    let rid = a.data.new_id();
    let r = msg(rid, 2, "I do", now_ts());
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == g)
        .unwrap()
        .messages
        .iter_mut()
        .find(|m| m.id == mine)
        .unwrap()
        .replies
        .push(r);
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Reply]);
    assert_eq!(a.notify_rt.sent[0].target, format!("chat:{}:{}", g, mine));
}

#[test]
fn a_huddle_rings_but_an_old_one_does_not() {
    let mut a = app();
    let dm = dm_with_alex(&a);
    let id = post(&mut a, dm, 2, "Started a huddle");
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == dm)
        .unwrap()
        .messages
        .iter_mut()
        .find(|m| m.id == id)
        .unwrap()
        .call = Some(format!("workboard-huddle:{}", dm));
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Huddle]);
    assert_eq!(a.notify_rt.sent[0].title, "Alex Kim is calling you");
    // one from ten minutes ago is probably over
    let old = a.data.new_id();
    let mut m = msg(old, 2, "Started a huddle", now_ts() - 600);
    m.call = Some(format!("workboard-huddle:{}", dm));
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == dm)
        .unwrap()
        .messages
        .push(m);
    a.notify_scan();
    assert_eq!(a.notify_rt.sent.len(), 1);
}

#[test]
fn assignments_comments_and_changes_on_my_tasks() {
    let mut a = app();
    let id = my_task(&mut a, "Ship the site");
    a.notify_scan();
    a.notify_rt.sent.clear();
    // Alex assigns, comments, changes the status
    let list = a.data.first_list().unwrap();
    a.apply(Action::Add {
        list,
        name: "For me".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    let other = a.data.tasks.iter().find(|t| t.name == "For me").unwrap().id;
    teammate_line(&mut a, other, 2, "assigned to Me", false);
    teammate_line(&mut a, id, 2, "Looks good to me", true);
    teammate_line(
        &mut a,
        id,
        2,
        "changed status from TO DO to IN PROGRESS",
        false,
    );
    teammate_line(&mut a, id, 2, "added tag web", false);
    a.notify_scan();
    let k = kinds(&a);
    assert!(k.contains(&Kind::Assigned), "{:?}", k);
    assert!(k.contains(&Kind::Comment), "{:?}", k);
    assert!(k.contains(&Kind::TaskChange), "{:?}", k);
    assert_eq!(k.len(), 3, "a new tag is not worth a notice: {:?}", k);
    let change = a
        .notify_rt
        .sent
        .iter()
        .find(|n| n.kind == Kind::TaskChange)
        .unwrap();
    assert_eq!(change.title, "Ship the site");
    assert_eq!(
        change.body,
        "Alex Kim changed status from TO DO to IN PROGRESS"
    );
    assert_eq!(change.target, format!("task:{}", id));
    // and the kinds can be turned off
    a.notify_rt.sent.clear();
    a.data.extra.notify.task_changes = false;
    teammate_line(&mut a, id, 3, "changed due date to Dec 1", false);
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty());
}

#[test]
fn a_task_whose_updates_i_turned_off_stays_quiet() {
    let mut a = app();
    let id = my_task(&mut a, "Ship the site");
    a.data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .set_watching(1, false);
    a.notify_scan();
    a.notify_rt.sent.clear();
    teammate_line(&mut a, id, 2, "Looks good to me", true);
    teammate_line(&mut a, id, 2, "changed due date to Dec 1", false);
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty(), "{:?}", kinds(&a));
    // a mention still reaches me
    teammate_line(&mut a, id, 2, "@Me can you check?", true);
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Mention]);
    // a teammate's task I chose to watch tells me
    a.notify_rt.sent.clear();
    let theirs = a.data.tasks.iter().find(|t| !t.watched_by(1)).unwrap().id;
    a.data
        .tasks
        .iter_mut()
        .find(|t| t.id == theirs)
        .unwrap()
        .set_watching(1, true);
    teammate_line(&mut a, theirs, 2, "Started on it", true);
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Comment]);
}

#[test]
fn paused_notifications_announce_nothing() {
    let mut a = app();
    a.data.extra.muted_until = now_ts() + 3600;
    let dm = dm_with_alex(&a);
    post(&mut a, dm, 2, "hello?");
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty());
}

#[test]
fn what_i_am_looking_at_is_not_announced() {
    let mut a = app();
    let dm = dm_with_alex(&a);
    a.open_chat_at(dm, None);
    post(&mut a, dm, 2, "you're here");
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty());
    // with the window in the background it is
    a.notify_rt.away = true;
    post(&mut a, dm, 2, "still there?");
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Direct]);
}

#[test]
fn a_big_batch_becomes_one_summary() {
    let mut a = app();
    let dm = dm_with_alex(&a);
    for i in 0..6 {
        post(&mut a, dm, 2, &format!("message {}", i));
    }
    a.notify_scan();
    assert_eq!(a.notify_rt.sent.len(), 1);
    assert_eq!(a.notify_rt.sent[0].title, "6 new notifications");
    assert_eq!(a.notify_rt.sent[0].target, "inbox");
}

#[test]
fn stale_news_from_long_ago_is_skipped() {
    let mut a = app();
    let dm = dm_with_alex(&a);
    let id = a.data.new_id();
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == dm)
        .unwrap()
        .messages
        .push(msg(id, 2, "from last week", now_ts() - 7 * 86400));
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty());
}

fn at(d: NaiveDate, hh: u32, mm: u32) -> i64 {
    d.and_hms_opt(hh, mm, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .earliest()
        .unwrap()
        .timestamp()
}

#[test]
fn tasks_due_today_are_announced_once_a_day_at_the_chosen_time() {
    let mut a = app();
    let day = today();
    // only the tasks made here are mine
    a.data
        .tasks
        .iter_mut()
        .for_each(|t| t.assignees.retain(|m| *m != 1));
    let id = my_task(&mut a, "File taxes");
    a.data.tasks.iter_mut().find(|t| t.id == id).unwrap().due = Some(day);
    let late = my_task(&mut a, "Send invoice");
    a.data.tasks.iter_mut().find(|t| t.id == late).unwrap().due = Some(day - Duration::days(2));
    a.notify_clock(at(day, 8, 30));
    assert!(a.notify_rt.sent.is_empty(), "not before 9:00");
    a.notify_clock(at(day, 9, 1));
    assert_eq!(kinds(&a), vec![Kind::Due]);
    assert_eq!(a.notify_rt.sent[0].title, "1 due today, 1 overdue");
    assert_eq!(a.notify_rt.sent[0].target, "today");
    a.notify_clock(at(day, 15, 0));
    assert_eq!(a.notify_rt.sent.len(), 1, "once a day");
    // another time of day
    let mut b = app();
    b.data
        .tasks
        .iter_mut()
        .for_each(|t| t.assignees.retain(|m| *m != 1));
    b.data.extra.notify.due_time = "07:30".into();
    let id = my_task(&mut b, "Early");
    b.data.tasks.iter_mut().find(|t| t.id == id).unwrap().due = Some(day);
    b.notify_clock(at(day, 7, 45));
    assert_eq!(kinds(&b), vec![Kind::Due]);
}

#[test]
fn a_meeting_is_announced_shortly_before_it_starts() {
    let mut a = app();
    let day = today();
    a.data.extra.meetings.clear();
    a.data.extra.notify.due = false;
    let id = a.data.new_id();
    a.data.extra.meetings.push(Meeting {
        id,
        title: "Standup".into(),
        date: day,
        time: "14:00".into(),
        attendees: vec![1, 2],
        agenda: String::new(),
        notes: String::new(),
    });
    a.notify_clock(at(day, 13, 40));
    assert!(a.notify_rt.sent.is_empty());
    a.notify_clock(at(day, 13, 52));
    assert_eq!(kinds(&a), vec![Kind::Meeting]);
    assert_eq!(a.notify_rt.sent[0].title, "Standup starts in 8 minutes");
    assert_eq!(a.notify_rt.sent[0].target, format!("meeting:{}", id));
    a.notify_clock(at(day, 13, 55));
    assert_eq!(a.notify_rt.sent.len(), 1);
    // a meeting I'm not in is not mine
    let other = a.data.new_id();
    a.data.extra.meetings.push(Meeting {
        id: other,
        title: "Sales".into(),
        date: day,
        time: "14:05".into(),
        attendees: vec![3],
        agenda: String::new(),
        notes: String::new(),
    });
    a.notify_clock(at(day, 14, 0));
    assert_eq!(a.notify_rt.sent.len(), 1);
}

#[test]
fn a_goal_due_tomorrow_is_announced() {
    let mut a = app();
    let day = today();
    a.data.extra.goals.clear();
    a.data.extra.notify.due = false;
    let id = a.data.new_id();
    a.data.extra.goals.push(Goal {
        id,
        name: "Launch".into(),
        color: [0, 0, 0],
        due: Some(day + Duration::days(1)),
        owner: 1,
        targets: vec![GoalTarget {
            id: 0,
            name: "Done".into(),
            kind: TargetKind::TrueFalse { done: false },
        }],
        ..Default::default()
    });
    a.notify_clock(at(day, 10, 0));
    assert_eq!(kinds(&a), vec![Kind::Goal]);
    assert_eq!(a.notify_rt.sent[0].title, "Goal due tomorrow: Launch");
    a.notify_clock(at(day, 11, 0));
    assert_eq!(a.notify_rt.sent.len(), 1);
}

#[test]
fn owners_hear_when_someone_joins() {
    let mut a = app();
    let id = a.data.new_id();
    a.data.members.push(Member {
        id,
        name: "Riley".into(),
        invited: true,
        ..Default::default()
    });
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty(), "invited, not joined yet");
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == id)
        .unwrap()
        .invited = false;
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Member]);
    // a plain member doesn't
    let mut b = app();
    b.data.members.iter_mut().find(|m| m.id == 1).unwrap().role = "Member".into();
    let id = b.data.new_id();
    b.data.members.push(Member {
        id,
        name: "Kai".into(),
        ..Default::default()
    });
    b.notify_scan();
    assert!(b.notify_rt.sent.is_empty());
}

#[test]
fn a_form_answer_is_logged_and_tells_the_forms_owner() {
    let mut a = app();
    let list = a.data.first_list().unwrap();
    let fid = a.data.new_id();
    a.data.extra.forms.push(Form {
        id: fid,
        name: "Bug report".into(),
        list_id: list,
        description: String::new(),
        fields: vec![],
        owner: 1,
        ..Default::default()
    });
    // the submit button leaves this line on the new task
    a.pending.log = Some("submitted the form Bug report".into());
    a.apply(Action::Add {
        list,
        name: "Crash on start".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    let t = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Crash on start")
        .unwrap();
    assert!(t
        .activity
        .iter()
        .any(|x| x.text == "submitted the form Bug report"));
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty(), "my own answer");
    // a teammate's answer
    a.apply(Action::Add {
        list,
        name: "Login fails".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    let tid = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Login fails")
        .unwrap()
        .id;
    teammate_line(&mut a, tid, 3, "submitted the form Bug report", false);
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Form]);
    assert_eq!(a.notify_rt.sent[0].title, "New answer to Bug report");
    assert_eq!(a.notify_rt.sent[0].body, "Sam Lee answered: Login fails");
}

#[test]
fn a_dropped_server_connection_is_announced_after_a_while_and_when_back() {
    let mut a = app();
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "d".into(),
        tag: 1,
        server: Some("macmini".into()),
    });
    let t0 = now_ts();
    a.sync_rt.net_status = Some(wb_net::Status::Connected);
    a.notify_connection(t0);
    a.sync_rt.net_status = Some(wb_net::Status::Reconnecting("timed out".into()));
    a.notify_connection(t0 + 1);
    a.notify_connection(t0 + 10);
    assert!(
        a.notify_rt.sent.is_empty(),
        "a short drop is not worth a notice"
    );
    a.notify_connection(t0 + 40);
    assert_eq!(kinds(&a), vec![Kind::Connection]);
    a.sync_rt.net_status = Some(wb_net::Status::Connected);
    a.notify_connection(t0 + 50);
    assert_eq!(a.notify_rt.sent.len(), 2);
    assert_eq!(
        a.notify_rt.sent[1].title,
        "Connected to the team server again"
    );
    // a short drop and back says nothing
    a.sync_rt.net_status = Some(wb_net::Status::Reconnecting("x".into()));
    a.notify_connection(t0 + 60);
    a.sync_rt.net_status = Some(wb_net::Status::Connected);
    a.notify_connection(t0 + 65);
    assert_eq!(a.notify_rt.sent.len(), 2);
}

#[test]
fn reactions_are_off_until_turned_on() {
    let mut a = app();
    let g = channel_id(&a, "general");
    let mine = post(&mut a, g, 1, "Shipped!");
    a.notify_scan();
    let react = |a: &mut App, user: u64, emoji: &str| {
        let m = a
            .data
            .channels
            .iter_mut()
            .find(|c| c.id == g)
            .unwrap()
            .messages
            .iter_mut()
            .find(|m| m.id == mine)
            .unwrap();
        m.reactions.push(Reaction {
            emoji: emoji.into(),
            users: vec![user],
        });
    };
    react(&mut a, 2, "\u{1F389}");
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty());
    a.data.extra.notify.reactions = true;
    a.notify_scan(); // the earlier reaction was already seen
    assert!(a.notify_rt.sent.is_empty());
    react(&mut a, 3, "\u{1F44D}");
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Reaction]);
    assert_eq!(
        a.notify_rt.sent[0].title,
        "Sam Lee reacted \u{1F44D} to your message"
    );
}

#[test]
fn reminders_go_through_notifications_and_open_their_task() {
    let mut a = app();
    let id = my_task(&mut a, "Call the bank");
    let rid = a.data.new_id();
    a.data.extra.reminders.push(Reminder {
        id: rid,
        text: "Call the bank".into(),
        due_ts: now_ts() - 5,
        owner: 1,
        task: Some(id),
        ..Default::default()
    });
    a.reminder_tick(None);
    assert_eq!(kinds(&a), vec![Kind::Reminder]);
    assert_eq!(a.toast.as_ref().unwrap().0, "Reminder: Call the bank");
    let target = a.notify_rt.sent[0].target.clone();
    a.open_target(&target);
    a.apply_actions();
    assert_eq!(a.open_task, Some(id));
}

#[test]
fn reminders_missed_while_closed_say_overdue_with_names_apart() {
    let mut a = app();
    for (text, ago) in [("File taxes", 2 * 86400), ("Dentist", 86400)] {
        let id = a.data.new_id();
        a.data.extra.reminders.push(Reminder {
            id,
            text: text.into(),
            due_ts: now_ts() - ago,
            owner: 1,
            ..Default::default()
        });
    }
    a.reminder_tick(None);
    assert_eq!(
        a.toast.as_ref().unwrap().0,
        "2 reminders are overdue: File taxes, Dentist"
    );
    // one that just came due is still "due"
    let id = a.data.new_id();
    a.data.extra.reminders.push(Reminder {
        id,
        text: "Stretch".into(),
        due_ts: now_ts() - 5,
        owner: 1,
        ..Default::default()
    });
    let id = a.data.new_id();
    a.data.extra.reminders.push(Reminder {
        id,
        text: "Water".into(),
        due_ts: now_ts() - 5,
        owner: 1,
        ..Default::default()
    });
    a.reminder_tick(None);
    assert_eq!(
        a.toast.as_ref().unwrap().0,
        "2 reminders are due: Stretch, Water"
    );
}

#[test]
fn the_dock_count_adds_unread_direct_messages_to_the_inbox() {
    let mut a = app();
    let base = a.dock_count();
    let dm = dm_with_alex(&a);
    post(&mut a, dm, 2, "one");
    post(&mut a, dm, 2, "two");
    assert_eq!(a.dock_count(), base + 2);
    a.data.extra.notify.dock_badge = false;
    assert_eq!(a.dock_count(), 0);
}

#[test]
fn notify_settings_from_older_files_get_the_new_defaults() {
    let n: NotifySettings = serde_json::from_str(r#"{"comments": false, "mentions": true, "reminders": true, "chat": true, "assigned": true}"#).unwrap();
    assert!(!n.comments);
    assert!(n.desktop && n.direct && n.due && !n.reactions);
    assert_eq!(n.channel_default, "mentions");
    assert_eq!(n.due_time, "09:00");
}

#[test]
fn notify_settings_stay_on_this_computer() {
    // only wb_sync::SHARED paths go to teammates
    assert!(!crate::sync::SHARED.iter().any(|p| p.contains("notify")));
}

#[test]
fn a_mention_picked_from_the_list_is_announced_by_member_not_name() {
    let mut a = app();
    let g = channel_id(&a, "general");
    // picked from the mention list: kept by id, whatever the text says
    let id = a.data.new_id();
    let m: Message = serde_json::from_value(serde_json::json!({"id": id, "author": 2, "text": "@Boss please review", "ts": now_ts(), "mentions": [1]})).unwrap();
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == g)
        .unwrap()
        .messages
        .push(m);
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Mention]);
    // someone else picked: not me, even with my name in the text
    let id = a.data.new_id();
    let m: Message = serde_json::from_value(serde_json::json!({"id": id, "author": 2, "text": "@Me (the other one)", "ts": now_ts(), "mentions": [3]})).unwrap();
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == g)
        .unwrap()
        .messages
        .push(m);
    a.notify_scan();
    assert_eq!(a.notify_rt.sent.len(), 1);
}

#[test]
fn comments_on_a_task_i_duplicated_notify_like_the_inbox_shows_them() {
    let mut a = app();
    // a teammate's task, not assigned to me: I make a copy
    let theirs = a
        .data
        .tasks
        .iter()
        .find(|t| t.parent.is_none() && !t.assignees.contains(&1))
        .unwrap()
        .id;
    let copy = a.duplicate_task(theirs).unwrap();
    if let Some(t) = a.data.tasks.iter_mut().find(|t| t.id == copy) {
        t.assignees.clear();
    }
    a.notify_scan();
    a.notify_rt.sent.clear();
    teammate_line(&mut a, copy, 2, "Thanks for copying it", true);
    assert!(a
        .inbox_items()
        .iter()
        .any(|i| i.tab == InboxTab::Primary && i.text == "Thanks for copying it"));
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Comment]);
}

#[test]
fn the_dock_count_follows_the_sidebar_seen_rule() {
    let mut a = app();
    let dm = dm_with_alex(&a);
    a.mark_read(dm);
    let base = a.dock_count();
    post(&mut a, dm, 2, "read already");
    a.mark_read(dm);
    assert_eq!(a.dock_count(), base);
    // a message whose author's clock is behind arrives after I read: it is
    // older than last_read but not seen, so it counts (as in the sidebar)
    let id = a.data.new_id();
    let late = msg(id, 2, "late", now_ts() - 3600);
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == dm)
        .unwrap()
        .messages
        .push(late);
    let c = a.data.channels.iter().find(|c| c.id == dm).unwrap().clone();
    assert_eq!(a.unread(&c), 1);
    assert_eq!(a.dock_count(), base + 1);
    a.mark_read(dm);
    assert_eq!(a.dock_count(), base);
}

#[test]
fn keywords_notify_like_a_mention_but_not_in_quiet_channels() {
    let mut a = app();
    let g = channel_id(&a, "general");
    a.data.extra.notify.keywords = "deploy, 배포 ,".into();
    post(&mut a, g, 3, "Lunch at noon");
    a.notify_scan();
    assert!(a.notify_rt.sent.is_empty());
    post(&mut a, g, 3, "The Deploy is done");
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Mention]);
    assert_eq!(
        a.notify_rt.sent[0].title,
        "Sam Lee said \"deploy\" in # general"
    );
    // in a thread too
    let top = a.data.channels.iter().find(|c| c.id == g).unwrap().messages[0].id;
    let rid = a.data.new_id();
    let r = msg(rid, 2, "배포 언제 해요?", now_ts());
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == g)
        .unwrap()
        .messages
        .iter_mut()
        .find(|m| m.id == top)
        .unwrap()
        .replies
        .push(r);
    a.notify_scan();
    assert_eq!(kinds(&a), vec![Kind::Mention, Kind::Mention]);
    // a message that also mentions me is told once, as the mention
    post(&mut a, g, 3, "@Me deploy please");
    a.notify_scan();
    assert_eq!(a.notify_rt.sent.len(), 3);
    assert!(a.notify_rt.sent[2].title.contains("mentioned you"));
    // a channel set to nothing stays quiet
    a.data.extra.notify.set_channel_mode(g, "nothing");
    post(&mut a, g, 3, "deploy again");
    a.notify_scan();
    assert_eq!(a.notify_rt.sent.len(), 3);
}

#[test]
fn my_goals_passing_a_milestone_are_announced_once() {
    let mut a = app();
    let day = today();
    a.data.extra.goals.clear();
    a.data.extra.notify.due = false;
    let id = a.data.new_id();
    let me = a.data.me;
    a.data.extra.goals.push(Goal {
        id,
        name: "Launch".into(),
        owner: me,
        targets: vec![GoalTarget {
            id: 0,
            name: "Users".into(),
            kind: TargetKind::Number {
                start: 0.0,
                target: 10.0,
                current: 2.0,
                unit: String::new(),
            },
        }],
        ..Default::default()
    });
    // first seen: its history starts quietly
    a.notify_clock(at(day, 10, 0));
    assert!(a.notify_rt.sent.is_empty(), "{:?}", a.notify_rt.sent);
    // a teammate's change (or a task closed elsewhere) moves it past 50%
    if let TargetKind::Number { current, .. } = &mut a.data.extra.goals[0].targets[0].kind {
        *current = 6.0;
    }
    a.notify_clock(at(day, 10, 5));
    assert_eq!(kinds(&a), vec![Kind::Goal]);
    assert_eq!(a.notify_rt.sent[0].title, "Launch reached 50%");
    assert_eq!(a.notify_rt.sent[0].target, format!("goal:{}", id));
    a.notify_clock(at(day, 10, 10));
    assert_eq!(a.notify_rt.sent.len(), 1, "once");
    // turned off in Notifications: nothing
    a.data.extra.notify.goal_milestones = false;
    if let TargetKind::Number { current, .. } = &mut a.data.extra.goals[0].targets[0].kind {
        *current = 10.0;
    }
    a.notify_clock(at(day, 10, 15));
    assert_eq!(a.notify_rt.sent.len(), 1);
}

#[test]
fn a_milestone_i_recorded_myself_is_not_announced() {
    let mut a = app();
    let day = today();
    a.data.extra.goals.clear();
    a.data.extra.notify.due = false;
    let id = a.data.new_id();
    let me = a.data.me;
    a.data.extra.goals.push(Goal {
        id,
        name: "Launch".into(),
        owner: me,
        targets: vec![GoalTarget {
            id: 0,
            name: "Ship".into(),
            kind: TargetKind::TrueFalse { done: false },
        }],
        ..Default::default()
    });
    a.notify_clock(at(day, 10, 0));
    a.goal_edit(id, None, |g| {
        g.targets[0].kind = TargetKind::TrueFalse { done: true };
    });
    a.notify_clock(at(day, 10, 5));
    assert!(a.notify_rt.sent.is_empty(), "the page already showed it");
}

#[test]
fn a_goal_far_behind_is_announced_once_after_the_morning_time() {
    let mut a = app();
    let day = today();
    a.data.extra.goals.clear();
    a.data.extra.notify.due = false;
    let id = a.data.new_id();
    let me = a.data.me;
    a.data.extra.goals.push(Goal {
        id,
        name: "Docs".into(),
        owner: me,
        start: Some(day - Duration::days(30)),
        due: Some(day + Duration::days(7)),
        targets: vec![GoalTarget {
            id: 0,
            name: "Write".into(),
            kind: TargetKind::TrueFalse { done: false },
        }],
        ..Default::default()
    });
    a.notify_clock(at(day, 8, 0));
    assert!(a.notify_rt.sent.is_empty(), "not before 9:00");
    a.notify_clock(at(day, 9, 30));
    assert_eq!(kinds(&a), vec![Kind::Goal]);
    assert_eq!(a.notify_rt.sent[0].title, "Goal at risk: Docs");
    assert_eq!(a.notify_rt.sent[0].body, "0% done, 7 days left");
    a.notify_clock(at(day, 15, 0));
    assert_eq!(a.notify_rt.sent.len(), 1, "once per due date");
}

#[test]
fn a_goal_due_today_is_announced_once_unless_turned_off_and_opens_the_goal() {
    let mut a = app();
    let day = today();
    a.data.extra.goals.clear();
    a.data.extra.notify.due = false;
    let id = a.data.new_id();
    let me = a.data.me;
    let goal = |id: u64, name: &str| Goal {
        id,
        name: name.into(),
        due: Some(day),
        owner: me,
        start: Some(day - Duration::days(10)),
        targets: vec![GoalTarget {
            id: 0,
            name: "Ship".into(),
            kind: TargetKind::TrueFalse { done: false },
        }],
        ..Default::default()
    };
    a.data.extra.goals.push(goal(id, "Launch"));
    a.data.extra.notify.goal_risk = false;
    a.notify_clock(at(day, 10, 0));
    assert_eq!(kinds(&a), vec![Kind::Goal]);
    assert_eq!(a.notify_rt.sent[0].title, "Goal due today: Launch");
    assert_eq!(a.notify_rt.sent[0].body, "0% done");
    a.notify_clock(at(day, 16, 0));
    assert_eq!(a.notify_rt.sent.len(), 1, "once");
    // clicking it opens the goal
    a.open_target(&a.notify_rt.sent[0].target.clone());
    assert_eq!(a.section, Section::Goals);
    assert_eq!(a.goal_open, Some(id));
    // another due today with "Goal deadlines" turned off: nothing
    a.data.extra.notify.goals = false;
    let other = a.data.new_id();
    a.data.extra.goals.push(goal(other, "Second"));
    a.notify_clock(at(day, 17, 0));
    assert_eq!(a.notify_rt.sent.len(), 1);
    // a goal already achieved isn't announced as due
    a.data.extra.notify.goals = true;
    for g in a.data.extra.goals.iter_mut().filter(|g| g.id == other) {
        g.targets[0].kind = TargetKind::TrueFalse { done: true };
    }
    // (it reaching 100% elsewhere is news of its own)
    a.notify_clock(at(day, 18, 0));
    assert!(
        a.notify_rt
            .sent
            .iter()
            .all(|n| n.title != "Goal due today: Second"),
        "{:?}",
        a.notify_rt.sent
    );
}

/// Two forms can have the same name (two made from one template): an
/// answer tells the owner of the form it was given to, which the task
/// keeps, not everyone with a form of that name. An answer from an app
/// that didn't keep the form is still found by the name.
#[test]
fn a_form_answer_tells_only_that_forms_owner() {
    let mut a = app();
    let list = a.data.first_list().unwrap();
    let mine = a.data.new_id();
    let theirs = a.data.new_id();
    for (id, owner) in [(mine, 1), (theirs, 3)] {
        a.data.extra.forms.push(Form {
            id,
            name: "Bug report".into(),
            list_id: list,
            owner,
            ..Default::default()
        });
    }
    let answer = |a: &mut App, name: &str, form: Option<u64>| {
        a.pending.form = form;
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
        let tid = a.data.tasks.iter().find(|t| t.name == name).unwrap().id;
        teammate_line(a, tid, 2, "submitted the form Bug report", false);
        a.notify_scan();
    };
    answer(&mut a, "Login fails", Some(theirs));
    assert!(
        a.notify_rt.sent.is_empty(),
        "an answer to someone else's form"
    );
    answer(&mut a, "Crash on save", Some(mine));
    assert_eq!(kinds(&a), vec![Kind::Form]);
    assert_eq!(a.notify_rt.sent[0].body, "Alex Kim answered: Crash on save");
    answer(&mut a, "Old app answer", None);
    assert_eq!(a.notify_rt.sent.len(), 2);
    assert_eq!(
        a.notify_rt.sent[1].body,
        "Alex Kim answered: Old app answer"
    );
    // with the two forms in different Lists, an older app's answer is told
    // to the owner of the form whose List it went to
    let other = a
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .map(|l| l.id)
        .find(|l| *l != list)
        .unwrap();
    a.data
        .extra
        .forms
        .iter_mut()
        .find(|f| f.id == theirs)
        .unwrap()
        .list_id = other;
    a.pending.log = None;
    a.apply(Action::Add {
        list: other,
        name: "To their List".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    let tid = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "To their List")
        .unwrap()
        .id;
    teammate_line(&mut a, tid, 2, "submitted the form Bug report", false);
    a.notify_scan();
    assert_eq!(a.notify_rt.sent.len(), 2, "not told about theirs");
}

/// An answer to a form whose name was cleared says "Untitled form", as the
/// form shows in the app, not "New answer to ".
#[test]
fn an_answer_to_an_unnamed_form_says_untitled() {
    let mut a = app();
    let list = a.data.first_list().unwrap();
    let fid = a.data.new_id();
    a.data.extra.forms.push(Form {
        id: fid,
        list_id: list,
        owner: 1,
        ..Default::default()
    });
    a.pending.form = Some(fid);
    a.apply(Action::Add {
        list,
        name: "Printer".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    let tid = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Printer")
        .unwrap()
        .id;
    teammate_line(&mut a, tid, 2, "submitted the form ", false);
    a.notify_scan();
    assert_eq!(a.notify_rt.sent[0].title, "New answer to Untitled form");
}

#[test]
fn a_goal_far_behind_isnt_announced_again_after_many_other_notices() {
    let mut a = app();
    let day = today();
    a.data.extra.goals.clear();
    a.data.extra.notify.due = false;
    let id = a.data.new_id();
    let me = a.data.me;
    let behind = |id: u64, name: &str, due: i64| Goal {
        id,
        name: name.into(),
        owner: me,
        start: Some(day - Duration::days(30)),
        due: Some(day + Duration::days(due)),
        targets: vec![GoalTarget {
            id: 0,
            name: "Write".into(),
            kind: TargetKind::TrueFalse { done: false },
        }],
        ..Default::default()
    };
    a.data.extra.goals.push(behind(id, "Docs", 7));
    a.notify_clock(at(day, 9, 30));
    assert_eq!(a.notify_rt.sent.len(), 1);
    // months of other notices after it
    for n in 0..400 {
        a.data.extra.notify.fired.push(format!("due:old-{}", n));
    }
    // the next notice trims the list
    let other = a.data.new_id();
    a.data.extra.goals.push(behind(other, "Other", 1));
    a.notify_clock(at(day, 10, 0));
    a.notify_clock(at(day, 10, 30));
    let docs = a
        .notify_rt
        .sent
        .iter()
        .filter(|n| n.title == "Goal at risk: Docs")
        .count();
    assert_eq!(docs, 1, "once per due date: {:?}", a.notify_rt.sent);
    assert!(a.data.extra.notify.fired.len() <= 310, "still trimmed");
}
