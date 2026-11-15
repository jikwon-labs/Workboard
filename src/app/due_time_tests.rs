// Times of day on task start and due dates: reading and writing them (and
// files without them), overdue, sorting, repeats, the history and the
// due-time notice.

use super::*;
use chrono::{NaiveDateTime, NaiveTime};

fn hm(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn at(d: NaiveDate, h: u32, m: u32) -> NaiveDateTime {
    d.and_time(hm(h, m))
}

fn seed_task() -> Task {
    Data::seed().tasks[0].clone()
}

fn app() -> App {
    App::build(None, Some(Data::seed()))
}

#[test]
fn times_are_read_in_the_usual_ways() {
    assert_eq!(parse_time_of_day("14:30"), Some(hm(14, 30)));
    assert_eq!(parse_time_of_day("14:30:00"), Some(hm(14, 30)));
    assert_eq!(parse_time_of_day(" 2:30 pm "), Some(hm(14, 30)));
    assert_eq!(parse_time_of_day("2pm"), Some(hm(14, 0)));
    assert_eq!(parse_time_of_day("12 am"), Some(hm(0, 0)));
    assert_eq!(parse_time_of_day("12:15 PM"), Some(hm(12, 15)));
    assert_eq!(parse_time_of_day("9"), Some(hm(9, 0)));
    for bad in ["", "25:00", "9:75", "13 pm", "noon", "1:2:3:4"] {
        assert_eq!(parse_time_of_day(bad), None, "{:?}", bad);
    }
}

#[test]
fn a_file_without_times_loads_and_is_written_the_same() {
    // a workspace saved by an app from before times: no field at all
    let v = serde_json::to_value(Data::seed()).unwrap();
    assert!(v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t.get("due_time").is_none() && t.get("start_time").is_none()));
    let d: Data = serde_json::from_value(v.clone()).unwrap();
    assert!(d
        .tasks
        .iter()
        .all(|t| t.due_time.is_none() && t.start_time.is_none()));
    // and written back unchanged, so wb-sync sees no edit
    assert_eq!(serde_json::to_value(&d).unwrap(), v);
}

#[test]
fn a_file_with_times_loads_and_keeps_them() {
    let mut v = serde_json::to_value(Data::seed()).unwrap();
    let t = &mut v["tasks"][0];
    t["due"] = serde_json::json!("2026-11-20");
    t["due_time"] = serde_json::json!("14:30");
    t["start"] = serde_json::json!("2026-11-18");
    t["start_time"] = serde_json::json!("09:15:00");
    v["tasks"][1]["due_time"] = serde_json::Value::Null; // removed
    v["tasks"][2]["due_time"] = serde_json::json!("not a time"); // unreadable
    let d: Data = serde_json::from_value(v).unwrap();
    assert_eq!(d.tasks[0].due_at(), Some(hm(14, 30)));
    assert_eq!(d.tasks[0].start_at(), Some(hm(9, 15)));
    assert_eq!(d.tasks[1].due_time, Some(None));
    assert_eq!(
        d.tasks[2].due_time,
        Some(None),
        "unreadable counts as removed"
    );
    let back = serde_json::to_value(&d).unwrap();
    assert_eq!(back["tasks"][0]["due_time"], "14:30");
    assert_eq!(back["tasks"][0]["start_time"], "09:15");
    assert!(back["tasks"][1]["due_time"].is_null());
    assert!(back["tasks"][1].get("due_time").is_some(), "null stays");
}

#[test]
fn a_time_needs_its_date() {
    let mut t = seed_task();
    t.due = None;
    t.due_time = Some(Some(hm(10, 0)));
    assert_eq!(t.due_at(), None);
    assert_eq!(fmt_due(&t), None);
    assert!(!t.is_overdue_at(at(today() + Duration::days(400), 0, 0)));
    // removing a time that was never set leaves it out of the file
    let mut t = seed_task();
    t.set_due_time(None);
    assert_eq!(t.due_time, None);
    t.set_due_time(Some(hm(8, 0)));
    t.set_due_time(None);
    assert_eq!(t.due_time, Some(None));
}

#[test]
fn due_today_is_overdue_only_after_its_time() {
    let d = NaiveDate::from_ymd_opt(2026, 11, 16).unwrap();
    let mut t = seed_task();
    t.due = Some(d);
    t.due_time = None;
    // no time: due by the end of the day
    assert!(!t.is_overdue_at(at(d, 23, 59)));
    assert!(t.is_overdue_at(at(d + Duration::days(1), 0, 0)));
    t.set_due_time(Some(hm(14, 0)));
    assert!(!t.is_overdue_at(at(d, 13, 59)));
    assert!(t.is_overdue_at(at(d, 14, 0)));
    assert!(t.is_overdue_at(at(d, 18, 0)));
    assert!(!t.is_overdue_at(at(d - Duration::days(1), 23, 0)));
}

#[test]
fn sorting_by_due_date_orders_a_day_by_time() {
    let mut a = app();
    let list = a.data.first_list().unwrap();
    let d = today() + Duration::days(3);
    let base = seed_task();
    for (id, name, time) in [
        (9301, "No time", None),
        (9302, "Afternoon", Some(hm(15, 0))),
        (9303, "Morning", Some(hm(8, 30))),
    ] {
        let mut x = base.clone();
        x.id = id;
        x.name = name.into();
        x.list_id = list;
        x.parent = None;
        x.due = Some(d);
        x.due_time = time.map(Some);
        a.data.tasks.push(x);
    }
    let mut early = base.clone();
    early.id = 9304;
    early.name = "Day before".into();
    early.list_id = list;
    early.parent = None;
    early.due = Some(d - Duration::days(1));
    early.due_time = Some(Some(hm(23, 0)));
    a.data.tasks.push(early);
    a.sort_by = SortBy::DueDate;
    let names: Vec<&str> = a
        .filtered()
        .into_iter()
        .filter(|t| t.id > 9300)
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(names, ["Day before", "Morning", "Afternoon", "No time"]);
}

#[test]
fn a_repeating_task_keeps_its_time_on_the_next_date() {
    let mut a = app();
    let mut t = a
        .data
        .tasks
        .iter()
        .find(|t| t.parent.is_none())
        .unwrap()
        .clone();
    let d = today();
    t.due = Some(d);
    t.set_due_time(Some(hm(14, 30)));
    t.recur = Some("weekly".into());
    a.apply(Action::Replace(t.clone()));
    let done = a
        .data
        .statuses_for_list(t.list_id)
        .iter()
        .find(|s| matches!(s.kind, StatusKind::Closed | StatusKind::Done))
        .unwrap()
        .name
        .clone();
    let mut closed = a.task(t.id).unwrap().clone();
    closed.status = done;
    a.apply(Action::Replace(closed));
    let now = a.task(t.id).unwrap();
    assert_eq!(now.due, Some(d + Duration::days(7)));
    assert_eq!(now.due_at(), Some(hm(14, 30)));
    assert!(
        now.activity.iter().any(
            |l| l.text.starts_with("completed a weekly repeat; next due ")
                && l.text.ends_with(" at 2:30 PM")
        ),
        "{:?}",
        now.activity.iter().map(|l| &l.text).collect::<Vec<_>>()
    );
}

#[test]
fn history_mentions_the_time_and_reads_in_korean() {
    let mut a = app();
    let mut t = a.data.tasks[0].clone();
    t.due = Some(today());
    t.due_time = None;
    a.apply(Action::Replace(t.clone()));
    // only the time changes: still a line
    let mut t = a.task(t.id).unwrap().clone();
    t.set_due_time(Some(hm(14, 30)));
    a.apply(Action::Replace(t.clone()));
    let line = a.task(t.id).unwrap().activity.last().unwrap().text.clone();
    assert_eq!(line, "changed due date to Today at 2:30 PM");
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let ko = crate::i18n::history_line("Alex", &line);
    let ko_date = crate::i18n::history_line("Alex", "changed due date to Nov 16 at 9:05 AM");
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(ko, "Alex님이 마감일을 오늘 오후 2:30으로 변경했습니다");
    assert_eq!(
        ko_date,
        "Alex님이 마감일을 11월 16일 오전 9:05로 변경했습니다"
    );
    // the start date too
    let mut t = a.task(t.id).unwrap().clone();
    t.start = Some(today());
    t.set_start_time(Some(hm(9, 0)));
    a.apply(Action::Replace(t.clone()));
    let line = a.task(t.id).unwrap().activity.last().unwrap().text.clone();
    assert_eq!(line, "changed start date to Today at 9:00 AM");
}

#[test]
fn clearing_a_date_anywhere_drops_its_time() {
    let mut a = app();
    let mut t = a.data.tasks[0].clone();
    t.due = Some(today());
    t.set_due_time(Some(hm(11, 0)));
    t.start = Some(today());
    t.set_start_time(Some(hm(8, 0)));
    a.apply(Action::Replace(t.clone()));
    // a change that clears the dates directly (bulk edit, planner)
    let mut t = a.task(t.id).unwrap().clone();
    t.due = None;
    t.start = None;
    a.apply(Action::Replace(t.clone()));
    let now = a.task(t.id).unwrap().clone();
    assert_eq!((now.due_time, now.start_time), (Some(None), Some(None)));
    assert_eq!(
        now.activity.last().unwrap().text,
        "changed start date to none"
    );
    // a time left without its date (two people's edits merged) doesn't come
    // back when a date is set again
    let mut t = now.clone();
    t.due_time = Some(Some(hm(7, 0)));
    a.data
        .tasks
        .iter_mut()
        .find(|x| x.id == t.id)
        .unwrap()
        .due_time = t.due_time;
    t.due = Some(today());
    a.apply(Action::Replace(t.clone()));
    assert_eq!(a.task(t.id).unwrap().due_at(), None);
}

#[test]
fn a_removed_time_stays_removed_when_copies_merge() {
    let task = |extra: serde_json::Value| {
        let mut t = serde_json::json!({"id": 7, "list_id": 1, "name": "T", "due": "2026-11-20"});
        for (k, v) in extra.as_object().unwrap() {
            t[k] = v.clone();
        }
        t
    };
    let copy = |t: serde_json::Value, stamp: i64| serde_json::json!({"tasks": [t], "extra": {"sync": {"v": 2, "stamps": {"7": stamp}}}});
    // I removed the time after a teammate's copy had it
    let mut mine = copy(task(serde_json::json!({"due_time": null})), 200);
    let theirs = copy(task(serde_json::json!({"due_time": "14:30"})), 100);
    crate::sync::merge(&mut mine, &[theirs.clone()]);
    assert!(mine["tasks"][0]["due_time"].is_null());
    // an older app (no field) changed the task later: the time is kept
    let mut mine = copy(task(serde_json::json!({"due_time": "14:30"})), 100);
    let old_app = copy(task(serde_json::json!({"name": "Renamed"})), 300);
    crate::sync::merge(&mut mine, &[old_app]);
    assert_eq!(mine["tasks"][0]["name"], "Renamed");
    assert_eq!(mine["tasks"][0]["due_time"], "14:30");
}

#[test]
fn a_task_due_at_a_time_is_announced_then() {
    let mut a = app();
    a.notify_scan();
    a.notify_rt.sent.clear();
    a.data.extra.notify.due = true;
    let d = today();
    let mut t = a
        .data
        .tasks
        .iter()
        .find(|t| t.parent.is_none())
        .unwrap()
        .clone();
    t.assignees = vec![a.data.me];
    t.due = Some(d);
    t.set_due_time(Some(hm(10, 0)));
    let id = t.id;
    let name = t.name.clone();
    a.apply(Action::Replace(t));
    let ts = |h: u32, m: u32| {
        at(d, h, m)
            .and_local_timezone(chrono::Local)
            .earliest()
            .unwrap()
            .timestamp()
    };
    let due_at = |a: &App| {
        a.notify_rt
            .sent
            .iter()
            .filter(|n| n.key.starts_with("due-at:"))
            .cloned()
            .collect::<Vec<_>>()
    };
    a.notify_clock(ts(9, 55));
    assert!(due_at(&a).is_empty(), "not before its time");
    a.notify_clock(ts(10, 1));
    let sent = due_at(&a);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].title, format!("{} is due now", name));
    assert_eq!(sent[0].target, format!("task:{}", id));
    a.notify_clock(ts(10, 20));
    assert_eq!(due_at(&a).len(), 1, "once");
}
