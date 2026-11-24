use super::*;
use serde_json::json;

fn ws(tasks: Value, channels: Value, meta: &SyncMeta) -> Value {
    json!({"workspace": "W", "spaces": [], "tasks": tasks, "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": channels, "docs": [], "extra": {"sync": meta}})
}

#[test]
fn two_people_chatting_keep_both_messages() {
    let mut ma = SyncMeta::default();
    let mut mb = SyncMeta::default();
    let ch = |msgs: Value, read: i64| json!([{"id": 10, "name": "general", "kind": "Channel", "topic": "", "members": [], "messages": msgs, "last_read": read}]);
    let base = ch(json!([{"id": 11, "author": 1, "text": "hi", "ts": 100}]), 0);
    let mut a = ws(json!([]), base.clone(), &ma);
    stamp(&a, &mut ma, 100);
    let mut b = ws(json!([]), base, &mb);
    stamp(&b, &mut mb, 100);
    a = ws(
        json!([]),
        ch(
            json!([{"id": 11, "author": 1, "text": "hi", "ts": 100}, {"id": 21, "author": 1, "text": "from A", "ts": 200}]),
            500,
        ),
        &ma,
    );
    stamp(&a.clone(), &mut ma, 200);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    b = ws(
        json!([]),
        ch(
            json!([{"id": 11, "author": 1, "text": "hi", "ts": 100}, {"id": 31, "author": 2, "text": "from B", "ts": 150}]),
            9,
        ),
        &mb,
    );
    stamp(&b.clone(), &mut mb, 150);
    b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    assert!(merge(&mut a, &[b]));
    let msgs: Vec<&str> = a["channels"][0]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["text"].as_str().unwrap())
        .collect();
    assert_eq!(msgs, vec!["hi", "from B", "from A"]);
    assert_eq!(
        a["channels"][0]["last_read"], 500,
        "read position stays personal"
    );
}

#[test]
fn newer_edit_wins_and_deletes_stick() {
    let mut ma = SyncMeta::default();
    let mut mb = SyncMeta::default();
    let t = |name: &str, id: u64| json!({"id": id, "name": name, "activity": []});
    let mut a = ws(json!([t("one", 1), t("two", 2)]), json!([]), &ma);
    stamp(&a.clone(), &mut ma, 10);
    let mut b = a.clone();
    mb.hashes = ma.hashes.clone();
    mb.stamps = ma.stamps.clone();
    // A renames task 1 later than B does; B deletes task 2
    a["tasks"] = json!([t("one by A", 1), t("two", 2)]);
    stamp(&a.clone(), &mut ma, 30);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    b["tasks"] = json!([t("one by B", 1)]);
    stamp(&b.clone(), &mut mb, 20);
    b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    merge(&mut a, &[b.clone()]);
    let names: Vec<&str> = a["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["one by A"]);
    // and B sees the same after reading A
    merge(&mut b, &[a.clone()]);
    let names: Vec<&str> = b["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["one by A"]);
}

#[test]
fn comments_from_both_sides_are_kept() {
    let mut ma = SyncMeta::default();
    let mut a = ws(
        json!([{"id": 1, "name": "t", "activity": []}]),
        json!([]),
        &ma,
    );
    stamp(&a.clone(), &mut ma, 10);
    let mut mb = ma.clone();
    let mut b = a.clone();
    a["tasks"][0]["activity"] = json!([{"ts": 20, "author": 1, "text": "A says", "comment": true}]);
    stamp(&a.clone(), &mut ma, 20);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    b["tasks"][0]["activity"] = json!([{"ts": 25, "author": 2, "text": "B says", "comment": true}]);
    stamp(&b.clone(), &mut mb, 25);
    b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    merge(&mut a, &[b]);
    assert_eq!(a["tasks"][0]["activity"].as_array().unwrap().len(), 2);
}

#[test]
fn a_second_edit_in_the_same_moment_still_wins() {
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([{"id": 1, "name": "t"}]), json!([]), &ma);
    stamp(&a.clone(), &mut ma, 10);
    a["tasks"][0]["name"] = json!("first");
    stamp(&a.clone(), &mut ma, 50);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    // someone reads the first edit
    let mut reader = a.clone();
    a["tasks"][0]["name"] = json!("second");
    stamp(&a.clone(), &mut ma, 50);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    merge(&mut reader, &[a]);
    assert_eq!(reader["tasks"][0]["name"], "second");
}

#[test]
fn restored_item_comes_back_for_everyone() {
    let mut m = SyncMeta::default();
    let a = ws(json!([{"id": 1, "name": "t"}]), json!([]), &m);
    stamp(&a, &mut m, 10);
    let gone = ws(json!([]), json!([]), &m);
    stamp(&gone, &mut m, 20);
    assert!(m.tombs.contains_key(&1));
    stamp(&a, &mut m, 30);
    assert!(m.revived[&1] > m.tombs[&1]);
    let mut other = ws(
        json!([]),
        json!([]),
        &SyncMeta {
            tombs: [(1, 20)].into(),
            ..Default::default()
        },
    );
    let mut mine = a.clone();
    mine["extra"]["sync"] = serde_json::to_value(&m).unwrap();
    merge(&mut other, &[mine]);
    assert_eq!(other["tasks"].as_array().unwrap().len(), 1);
}

fn sp(spaces: Value, meta: &SyncMeta) -> Value {
    json!({"workspace": "W", "spaces": spaces, "tasks": [], "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": [], "docs": [], "extra": {"sync": meta}})
}

fn list(id: u64) -> Value {
    json!({"id": id, "name": format!("L{}", id)})
}

fn space(id: u64, folders: Value, lists: Value) -> Value {
    json!({"id": id, "name": format!("S{}", id), "folders": folders, "lists": lists})
}

fn folder(id: u64, lists: Value) -> Value {
    json!({"id": id, "name": format!("F{}", id), "lists": lists})
}

/// Two computers that start from the same synced copy.
fn pair(spaces: Value) -> (Value, SyncMeta, Value, SyncMeta) {
    let mut ma = SyncMeta::default();
    let mut a = sp(spaces, &ma);
    stamp(&a.clone(), &mut ma, 10);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    (a.clone(), ma.clone(), a, ma)
}

fn change(v: &mut Value, m: &mut SyncMeta, spaces: Value, now: i64) {
    v["spaces"] = spaces;
    stamp(&v.clone(), m, now);
    v["extra"]["sync"] = serde_json::to_value(&*m).unwrap();
}

/// Where every list is: (parent id, list id), in order.
fn places(v: &Value) -> Vec<(u64, u64)> {
    let mut out = vec![];
    for s in v["spaces"].as_array().unwrap() {
        for l in s["lists"].as_array().unwrap() {
            out.push((s["id"].as_u64().unwrap(), l["id"].as_u64().unwrap()));
        }
        for f in s["folders"].as_array().unwrap() {
            for l in f["lists"].as_array().unwrap() {
                out.push((f["id"].as_u64().unwrap(), l["id"].as_u64().unwrap()));
            }
        }
    }
    out
}

#[test]
fn a_moved_list_appears_once_for_everyone() {
    // space to space
    let (mut a, mut ma, mut b, _) = pair(json!([
        space(100, json!([]), json!([list(101), list(102)])),
        space(200, json!([]), json!([]))
    ]));
    change(
        &mut a,
        &mut ma,
        json!([
            space(100, json!([]), json!([list(102)])),
            space(200, json!([]), json!([list(101)]))
        ]),
        30,
    );
    merge(&mut a, &[b.clone()]);
    assert_eq!(places(&a), vec![(100, 102), (200, 101)]);
    merge(&mut b, &[a.clone()]);
    assert_eq!(places(&b), vec![(100, 102), (200, 101)]);

    // folder to another space, and back from a space into a folder
    let (mut a, mut ma, mut b, _) = pair(json!([
        space(100, json!([folder(300, json!([list(101)]))]), json!([])),
        space(200, json!([]), json!([]))
    ]));
    change(
        &mut a,
        &mut ma,
        json!([
            space(100, json!([folder(300, json!([]))]), json!([])),
            space(200, json!([]), json!([list(101)]))
        ]),
        30,
    );
    let stale = b.clone();
    merge(&mut b, &[a.clone()]);
    assert_eq!(places(&b), vec![(200, 101)]);
    merge(&mut a, &[stale]);
    assert_eq!(places(&a), vec![(200, 101)]);
    let mut mb = meta_of(&b);
    change(
        &mut b,
        &mut mb,
        json!([
            space(100, json!([folder(300, json!([list(101)]))]), json!([])),
            space(200, json!([]), json!([]))
        ]),
        40,
    );
    let stale = a.clone();
    merge(&mut a, &[b.clone()]);
    assert_eq!(places(&a), vec![(300, 101)]);
    merge(&mut b, &[stale]);
    assert_eq!(places(&b), vec![(300, 101)]);
}

#[test]
fn a_list_added_while_another_moved_is_kept() {
    let (mut a, mut ma, mut b, mut mb) = pair(json!([
        space(100, json!([]), json!([list(101), list(102)])),
        space(200, json!([]), json!([]))
    ]));
    // A moves 101 away; B, not knowing, adds 103 to the old space
    change(
        &mut a,
        &mut ma,
        json!([
            space(100, json!([]), json!([list(102)])),
            space(200, json!([]), json!([list(101)]))
        ]),
        30,
    );
    change(
        &mut b,
        &mut mb,
        json!([
            space(100, json!([]), json!([list(101), list(102), list(103)])),
            space(200, json!([]), json!([]))
        ]),
        25,
    );
    let (a0, b0) = (a.clone(), b.clone());
    merge(&mut a, &[b0]);
    merge(&mut b, &[a0]);
    assert_eq!(places(&a), vec![(100, 102), (100, 103), (200, 101)]);
    assert_eq!(places(&b), places(&a));

    // a folder moves while someone adds a list inside it
    let (mut a, mut ma, mut b, mut mb) = pair(json!([
        space(100, json!([folder(300, json!([list(101)]))]), json!([])),
        space(200, json!([]), json!([]))
    ]));
    change(
        &mut a,
        &mut ma,
        json!([
            space(100, json!([]), json!([])),
            space(200, json!([folder(300, json!([list(101)]))]), json!([]))
        ]),
        30,
    );
    change(
        &mut b,
        &mut mb,
        json!([
            space(
                100,
                json!([folder(300, json!([list(101), list(104)]))]),
                json!([])
            ),
            space(200, json!([]), json!([]))
        ]),
        25,
    );
    let (a0, b0) = (a.clone(), b.clone());
    merge(&mut a, &[b0]);
    merge(&mut b, &[a0]);
    assert_eq!(places(&a), vec![(300, 101), (300, 104)]);
    assert_eq!(a["spaces"][1]["folders"][0]["id"], 300);
    assert_eq!(a["spaces"][0]["folders"].as_array().unwrap().len(), 0);
    assert_eq!(places(&b), places(&a));
}

#[test]
fn updating_the_app_does_not_mark_inner_items_as_edited() {
    let d = sp(
        json!([space(100, json!([]), json!([list(101)]))]),
        &SyncMeta::default(),
    );
    let mut m = SyncMeta {
        hashes: fingerprints_with(&d, false),
        stamps: [(100, 10), (101, 10)].into(),
        ..Default::default()
    };
    stamp(&d, &mut m, 99);
    assert_eq!(m.stamps[&101], 10);
    assert_eq!(m.v, META_V);
}

#[test]
fn a_delete_wins_even_with_a_clock_behind() {
    let mut m = SyncMeta::default();
    let a = ws(json!([{"id": 1, "name": "t"}]), json!([]), &m);
    stamp(&a, &mut m, 1000);
    // restored on a computer whose clock is ahead
    m.revived.insert(1, 2000);
    let gone = ws(json!([]), json!([]), &m);
    stamp(&gone, &mut m, 20);
    assert!(m.tombs[&1] > 2000);
    let mut other = a.clone();
    let mut mine = gone.clone();
    mine["extra"]["sync"] = serde_json::to_value(&m).unwrap();
    merge(&mut other, &[mine]);
    assert_eq!(other["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn workspace_settings_are_shared_newest_wins() {
    let (mut a, mut ma, mut b, mut mb) = pair(json!([]));
    a["extra"]["apps"] = json!({"priorities": true});
    b["extra"]["apps"] = json!({"priorities": true});
    stamp(&a.clone(), &mut ma, 11);
    stamp(&b.clone(), &mut mb, 11);
    // A renames the workspace, B turns off priorities later
    a["workspace"] = json!("Team");
    stamp(&a.clone(), &mut ma, 20);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    b["extra"]["apps"] = json!({"priorities": false});
    stamp(&b.clone(), &mut mb, 30);
    b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    let (a0, b0) = (a.clone(), b.clone());
    assert!(merge(&mut a, &[b0]));
    merge(&mut b, &[a0]);
    for v in [&a, &b] {
        assert_eq!(v["workspace"], "Team");
        assert_eq!(v["extra"]["apps"]["priorities"], false);
    }
    // a setting nobody changed yet stays as each computer has it
    let (mut c, _, mut d, _) = pair(json!([]));
    d["extra"]["workspace_color"] = json!([1, 2, 3]);
    merge(&mut c, &[d.clone()]);
    assert!(c["extra"].get("workspace_color").is_none());
}

#[test]
fn joining_keeps_your_own_settings() {
    let team = json!({"workspace": "Team", "spaces": [space(1, json!([]), json!([]))], "tasks": [], "members": [{"id": 5}], "next_id": 77, "dark": true, "extra": {"accent": "pink", "teams": [{"id": 9}], "sync": {"stamps": {"1": 3}}}});
    let mine = json!({"workspace": "Mine", "spaces": [], "tasks": [], "members": [], "next_id": 2, "dark": false, "extra": {"accent": "blue", "teams": []}});
    let v = with_team_things(&mine, &team);
    assert_eq!(v["workspace"], "Team");
    assert_eq!(v["spaces"].as_array().unwrap().len(), 1);
    assert_eq!(v["extra"]["teams"][0]["id"], 9);
    assert_eq!(v["extra"]["sync"]["stamps"]["1"], 3);
    assert_eq!(v["next_id"], 77);
    assert_eq!(v["dark"], false);
    assert_eq!(v["extra"]["accent"], "blue");
}

#[test]
fn renaming_a_list_you_never_saw_move_keeps_the_move() {
    let (mut a, mut ma, mut b, mut mb) = pair(json!([
        space(100, json!([]), json!([list(101)])),
        space(200, json!([]), json!([]))
    ]));
    // A moves the list; later B, not knowing, renames it where B has it
    change(
        &mut a,
        &mut ma,
        json!([
            space(100, json!([]), json!([])),
            space(200, json!([]), json!([list(101)]))
        ]),
        30,
    );
    let mut renamed = list(101);
    renamed["name"] = json!("Renamed by B");
    change(
        &mut b,
        &mut mb,
        json!([
            space(100, json!([]), json!([renamed])),
            space(200, json!([]), json!([]))
        ]),
        40,
    );
    let (a0, b0) = (a.clone(), b.clone());
    merge(&mut a, &[b0]);
    merge(&mut b, &[a0]);
    for v in [&a, &b] {
        assert_eq!(places(v), vec![(200, 101)], "the move stays");
        assert_eq!(
            v["spaces"][1]["lists"][0]["name"], "Renamed by B",
            "and so does the rename"
        );
    }
    // a copy from an app that doesn't keep move times still moves it by its stamp
    let (mut a, _, mut old, mut mo) = pair(json!([
        space(100, json!([]), json!([list(101)])),
        space(200, json!([]), json!([]))
    ]));
    change(
        &mut old,
        &mut mo,
        json!([
            space(100, json!([]), json!([])),
            space(200, json!([]), json!([list(101)]))
        ]),
        30,
    );
    old["extra"]["sync"]["v"] = json!(1);
    old["extra"]["sync"]
        .as_object_mut()
        .unwrap()
        .remove("moved");
    merge(&mut a, &[old]);
    assert_eq!(places(&a), vec![(200, 101)]);
}

#[test]
fn hidden_items_leave_a_copy_without_a_tombstone() {
    let mut m = SyncMeta::default();
    let chans = json!([
        {"id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [{"id": 11, "text": "hi", "ts": 1}]},
        {"id": 20, "name": "", "kind": "Direct", "members": [1, 3], "messages": [{"id": 21, "text": "secret", "ts": 1}]},
        {"id": 30, "name": "plans", "kind": "Channel", "private": true, "members": [1, 3], "messages": []},
        {"id": 40, "name": "open", "kind": "Channel", "private": true, "members": [1, 2], "messages": []}
    ]);
    let mut d = json!({"workspace": "W", "members": [], "me": 2, "next_id": 1, "favorites": [], "channels": chans,
        "spaces": [{"id": 50, "name": "Personal", "private_owner": 1, "folders": [], "lists": [{"id": 51, "name": "Mine"}]}, {"id": 60, "name": "Bo's", "private_owner": 2, "folders": [], "lists": [{"id": 61, "name": "Bo"}]}, space(70, json!([]), json!([list(71)]))],
        "tasks": [{"id": 52, "list_id": 51, "name": "secret task"}, {"id": 62, "list_id": 61, "name": "own"}, {"id": 72, "list_id": 71, "name": "team"}],
        "docs": [{"id": 80, "owner": 1, "private": true}, {"id": 81, "owner": 1, "private": false, "parent": 80}, {"id": 82, "owner": 2, "private": true}, {"id": 83, "owner": 0, "private": true}, {"id": 84, "owner": 3, "private": false}, {"id": 85, "owner": 3, "space": 50}],
        "extra": {"sync": {}}});
    let mut hidden = hidden_from(&d, 2, false);
    hidden.sort();
    assert_eq!(hidden, vec![20, 30, 50, 52, 80, 81, 85]);
    stamp(&d.clone(), &mut m, 10);
    d["extra"]["sync"] = serde_json::to_value(&m).unwrap();
    let gone = hide(&mut d, &hidden);
    assert_eq!(gone.len(), 7);
    let mut m = meta_of(&d);
    for id in [20, 21, 30, 50, 51, 52, 80, 81, 85] {
        assert!(
            !m.stamps.contains_key(&id) && !m.hashes.contains_key(&id),
            "no bookkeeping left for {}",
            id
        );
    }
    stamp(&d.clone(), &mut m, 20);
    assert!(
        m.tombs.is_empty(),
        "nothing counts as deleted: {:?}",
        m.tombs
    );
    assert!(d.to_string().contains("\"open\"") && !d.to_string().contains("secret"));
    // when they come back from the team's copy, they are simply there again
    let mut team = json!({"workspace": "W", "spaces": [], "tasks": [], "members": [], "docs": [], "channels": [{"id": 30, "name": "plans", "kind": "Channel", "private": true, "members": [1, 2, 3], "messages": []}], "extra": {"sync": {"stamps": {"30": 5}, "v": 2}}});
    d["extra"]["sync"] = serde_json::to_value(&m).unwrap();
    merge(&mut d, &[team.take()]);
    assert!(d["channels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == 30));
}

#[test]
fn bookmarks_are_shared() {
    let (mut a, mut ma, mut b, _) = pair(json!([]));
    a["extra"]["bookmarks"] = json!([{"id": 5, "title": "Docs", "url": "https://example.com"}]);
    stamp(&a.clone(), &mut ma, 20);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    merge(&mut b, &[a.clone()]);
    assert_eq!(b["extra"]["bookmarks"][0]["title"], "Docs");
    let joined = with_team_things(&json!({"extra": {"bookmarks": []}}), &a);
    assert_eq!(joined["extra"]["bookmarks"][0]["id"], 5);
}

#[test]
fn saved_filters_stay_home_and_survive_a_merge() {
    let saved = json!([{"id": 4, "name": "Urgent", "join": "Or",
        "rows": [{"field": "Priority", "not": false, "values": ["1"]}]}]);
    let mut mine = ws(json!([]), json!([]), &SyncMeta::default());
    mine["extra"]["saved_filters"] = saved.clone();
    // left out of the copy teammates and the server get
    assert!(shareable(&mine)["extra"].get("saved_filters").is_none());
    // a teammate's copy with their own saved filters doesn't replace mine
    let mut theirs = ws(
        json!([{"id": 7, "name": "T"}]),
        json!([]),
        &SyncMeta::default(),
    );
    theirs["extra"]["saved_filters"] = json!([{"id": 9, "name": "Theirs"}]);
    merge(&mut mine, &[theirs]);
    assert_eq!(mine["extra"]["saved_filters"], saved);
    assert_eq!(mine["tasks"][0]["id"], 7);
}

#[test]
fn a_field_set_back_to_its_default_stays_there() {
    // a task with a running timer (fields left out at their default)
    let (mut a, mut ma, mut b, mut mb) = pair(json!([]));
    a["tasks"] = json!([{"id": 7, "name": "T", "timer_started": 4, "timer_by": 2, "timers": [{"member": 3, "start": 5}]}]);
    stamp(&a.clone(), &mut ma, 20);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    merge(&mut b, &[a.clone()]);
    stamp(&b.clone(), &mut mb, 21);
    b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    assert_eq!(b["tasks"][0]["timer_by"], 2);
    // B stops both timers: the fields go back to their default and leave the file
    b["tasks"] = json!([{"id": 7, "name": "T"}]);
    stamp(&b.clone(), &mut mb, 30);
    b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    merge(&mut a, &[b.clone()]);
    assert!(
        a["tasks"][0].get("timer_by").is_none(),
        "a stopped timer doesn't come back: {}",
        a["tasks"][0]
    );
    assert!(a["tasks"][0].get("timers").is_none());
    // a copy from an app that doesn't leave defaults out (v below 2) still
    // keeps fields it doesn't know about
    let mut old = b.clone();
    let mut mo = mb.clone();
    mo.v = 1;
    mo.fields.clear();
    old["tasks"] = json!([{"id": 7, "name": "T renamed"}]);
    mo.stamps.insert(7, 40);
    old["extra"]["sync"] = serde_json::to_value(&mo).unwrap();
    let mut c = b.clone();
    c["tasks"] = json!([{"id": 7, "name": "T", "watchers": [4]}]);
    merge(&mut c, &[old]);
    assert_eq!(c["tasks"][0]["name"], "T renamed");
    assert_eq!(
        c["tasks"][0]["watchers"],
        json!([4]),
        "an older app's copy doesn't drop what it can't know"
    );
    // and a field outside DEFAULT_WHEN_MISSING is kept even from a new copy
    let mut newer = b.clone();
    let mut mn = mb.clone();
    newer["tasks"] = json!([{"id": 7, "name": "T again"}]);
    mn.stamps.insert(7, 50);
    newer["extra"]["sync"] = serde_json::to_value(&mn).unwrap();
    let mut d = b.clone();
    d["tasks"] = json!([{"id": 7, "name": "T", "attachments": [{"file": "x"}]}]);
    merge(&mut d, &[newer]);
    assert_eq!(d["tasks"][0]["attachments"], json!([{"file": "x"}]));
}

/// Record `v`'s changes at `now` into `m` and write the bookkeeping into `v`.
fn save(v: &mut Value, m: &mut SyncMeta, now: i64) {
    fill_activity_ids(v);
    fill_board_ids(v);
    fill_goal_ids(v);
    stamp(&v.clone(), m, now);
    v["extra"]["sync"] = serde_json::to_value(&*m).unwrap();
}

/// Both merge the other's copy as it was before.
fn swap(a: &mut Value, b: &mut Value) {
    let (a0, b0) = (a.clone(), b.clone());
    merge(a, &[b0]);
    merge(b, &[a0]);
}

#[test]
fn two_people_changing_different_fields_of_one_task_keep_both() {
    let mut ma = SyncMeta::default();
    let mut a = ws(
        json!([{"id": 1, "name": "t", "status": "to do", "priority": 0}]),
        json!([]),
        &ma,
    );
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    a["tasks"][0]["name"] = json!("renamed by A");
    save(&mut a, &mut ma, 20);
    b["tasks"][0]["status"] = json!("done");
    save(&mut b, &mut mb, 30);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(v["tasks"][0]["name"], "renamed by A", "{}", v["tasks"][0]);
        assert_eq!(v["tasks"][0]["status"], "done");
    }
    assert_eq!(meta_of(&a).fields, meta_of(&b).fields, "both sides agree");
    // the same field: the newest change wins on both sides
    let (mut ma, mut mb) = (meta_of(&a), meta_of(&b));
    b["tasks"][0]["name"] = json!("B early");
    save(&mut b, &mut mb, 40);
    a["tasks"][0]["name"] = json!("A late");
    a["tasks"][0]["priority"] = json!(2);
    save(&mut a, &mut ma, 50);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(v["tasks"][0]["name"], "A late");
        assert_eq!(v["tasks"][0]["priority"], 2);
        assert_eq!(v["tasks"][0]["status"], "done");
    }
    // nothing left to merge
    assert!(!merge(&mut a.clone(), &[b.clone()]));
    // a field taken away (at its default) stays away while another
    // field changes elsewhere
    let mut ma = meta_of(&a);
    a["tasks"][0]["watchers"] = json!([7]);
    save(&mut a, &mut ma, 60);
    merge(&mut b, &[a.clone()]);
    let mut mb = meta_of(&b);
    mb.field_hashes = part_hashes(&b);
    b["tasks"][0].as_object_mut().unwrap().remove("watchers");
    save(&mut b, &mut mb, 70);
    a["tasks"][0]["name"] = json!("A again");
    save(&mut a, &mut ma, 80);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(v["tasks"][0]["name"], "A again");
        assert!(v["tasks"][0].get("watchers").is_none(), "{}", v["tasks"][0]);
    }
}

#[test]
fn a_copy_without_field_stamps_counts_every_field_at_its_stamp() {
    let mut ma = SyncMeta::default();
    let mut a = ws(
        json!([{"id": 1, "name": "t", "status": "to do"}]),
        json!([]),
        &ma,
    );
    save(&mut a, &mut ma, 10);
    let mut old = a.clone();
    a["tasks"][0]["status"] = json!("done");
    save(&mut a, &mut ma, 20);
    // an app from before field stamps renames it later: its whole copy wins
    old["tasks"][0]["name"] = json!("old app");
    old["extra"]["sync"] = json!({"stamps": {"1": 30}, "v": 2});
    merge(&mut a, &[old]);
    assert_eq!(a["tasks"][0]["name"], "old app");
    assert_eq!(a["tasks"][0]["status"], "to do");
}

fn chat(reactions: Value) -> Value {
    json!([{"id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [{"id": 11, "author": 1, "text": "hi", "ts": 5, "reactions": reactions}]}])
}

fn reactions_of(v: &Value) -> Vec<(String, u64)> {
    reaction_pairs(&v["channels"][0]["messages"][0]["reactions"]).unwrap()
}

#[test]
fn reactions_made_at_once_are_all_kept_and_removals_stick() {
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([]), chat(json!([])), &ma);
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    a["channels"] = chat(json!([{"emoji": "👍", "users": [1]}]));
    save(&mut a, &mut ma, 20);
    b["channels"] = chat(json!([{"emoji": "❤️", "users": [2]}, {"emoji": "👍", "users": [2]}]));
    save(&mut b, &mut mb, 30);
    swap(&mut a, &mut b);
    let want = vec![
        ("👍".to_string(), 1),
        ("👍".to_string(), 2),
        ("❤️".to_string(), 2),
    ];
    let mut got = reactions_of(&a);
    got.sort();
    let mut w = want.clone();
    w.sort();
    assert_eq!(got, w);
    assert_eq!(reactions_of(&a), reactions_of(&b));
    assert_eq!(
        a["channels"][0]["messages"][0]["reactions"],
        b["channels"][0]["messages"][0]["reactions"]
    );
    // A takes back their 👍 while B, not knowing, adds 🎉
    let (mut ma, mut mb) = (meta_of(&a), meta_of(&b));
    ma.field_hashes = part_hashes(&a);
    mb.field_hashes = part_hashes(&b);
    a["channels"][0]["messages"][0]["reactions"] =
        json!([{"emoji": "👍", "users": [2]}, {"emoji": "❤️", "users": [2]}]);
    save(&mut a, &mut ma, 40);
    b["channels"][0]["messages"][0]["reactions"] = json!([{"emoji": "👍", "users": [1, 2]}, {"emoji": "❤️", "users": [2]}, {"emoji": "🎉", "users": [2]}]);
    save(&mut b, &mut mb, 50);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        let mut got = reactions_of(v);
        got.sort();
        assert_eq!(
            got,
            vec![
                ("❤️".to_string(), 2),
                ("🎉".to_string(), 2),
                ("👍".to_string(), 2)
            ],
            "the removed 👍 stays removed: {}",
            v["channels"][0]["messages"][0]
        );
    }
}

#[test]
fn reactions_stay_in_the_order_they_were_added() {
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([]), chat(json!([])), &ma);
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    // 👍 first, then 🎉 (which sorts before 👍 by character), then a second 👍
    a["channels"] = chat(json!([{"emoji": "👍", "users": [1]}]));
    save(&mut a, &mut ma, 20);
    b["channels"] = chat(json!([{"emoji": "🎉", "users": [2]}]));
    save(&mut b, &mut mb, 30);
    swap(&mut a, &mut b);
    let mut ma = meta_of(&a);
    ma.field_hashes = part_hashes(&a);
    a["channels"][0]["messages"][0]["reactions"] =
        json!([{"emoji": "👍", "users": [1, 3]}, {"emoji": "🎉", "users": [2]}]);
    save(&mut a, &mut ma, 40);
    swap(&mut a, &mut b);
    let want = json!([{"emoji": "👍", "users": [1, 3]}, {"emoji": "🎉", "users": [2]}]);
    for v in [&a, &b] {
        assert_eq!(v["channels"][0]["messages"][0]["reactions"], want);
    }
}

#[test]
fn timers_started_at_once_both_keep_running() {
    let mut ma = SyncMeta::default();
    let task = |extra: Value| {
        let mut t = json!({"id": 1, "name": "t", "timer_started": null});
        for (k, v) in extra.as_object().unwrap() {
            t[k] = v.clone();
        }
        json!([t])
    };
    let mut a = ws(task(json!({})), json!([]), &ma);
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    // both start their timer in the field older apps read
    a["tasks"] = task(json!({"timer_started": 100, "timer_by": 1}));
    save(&mut a, &mut ma, 20);
    b["tasks"] = task(json!({"timer_started": 105, "timer_by": 2}));
    save(&mut b, &mut mb, 21);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        let t = &v["tasks"][0];
        assert_eq!(t["timer_started"], 100, "{}", t);
        assert_eq!(t["timer_by"], 1);
        assert_eq!(t["timers"], json!([{"member": 2, "start": 105}]));
    }
    // A stops theirs (B's moves into the old field) while B renames the task
    let (mut ma, mut mb) = (meta_of(&a), meta_of(&b));
    ma.field_hashes = part_hashes(&a);
    mb.field_hashes = part_hashes(&b);
    a["tasks"] = task(json!({"timer_started": 105, "timer_by": 2}));
    save(&mut a, &mut ma, 30);
    b["tasks"][0]["name"] = json!("renamed");
    save(&mut b, &mut mb, 40);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        let t = &v["tasks"][0];
        assert_eq!(t["name"], "renamed");
        assert_eq!(t["timer_started"], 105, "A's stop sticks: {}", t);
        assert_eq!(t["timer_by"], 2);
        assert!(t.get("timers").is_none());
    }
}

#[test]
fn strokes_drawn_at_once_are_all_kept_and_erasing_sticks() {
    let stroke =
        |x: f64| json!({"color": [0, 0, 0], "width": 2.0, "points": [[x, 1.25], [x + 1.0, 2.0]]});
    let board = |strokes: Value| json!([{"id": 50, "name": "B", "strokes": strokes, "notes": [{"pos": [1.0, 2.0], "text": "old", "color": [1, 2, 3]}]}]);
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &ma);
    a["extra"]["whiteboards"] = board(json!([stroke(0.1)]));
    // a copy saved before strokes had ids works out the same ids
    let mut old = a.clone();
    save(&mut a, &mut ma, 10);
    fill_board_ids(&mut old);
    let x_id = a["extra"]["whiteboards"][0]["strokes"][0]["id"].clone();
    assert!(x_id.as_u64().unwrap() >= 1 << 62);
    assert_eq!(old["extra"]["whiteboards"][0]["strokes"][0]["id"], x_id);
    assert_eq!(
        old["extra"]["whiteboards"][0]["notes"][0]["id"],
        a["extra"]["whiteboards"][0]["notes"][0]["id"]
    );
    let (mut b, mut mb) = (a.clone(), ma.clone());
    // A erases the old stroke and draws one; B draws another
    let wa = &mut a["extra"]["whiteboards"][0]["strokes"];
    *wa = json!([stroke(5.0)]);
    save(&mut a, &mut ma, 20);
    b["extra"]["whiteboards"][0]["strokes"]
        .as_array_mut()
        .unwrap()
        .push(stroke(9.0));
    save(&mut b, &mut mb, 30);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        let xs: Vec<f64> = v["extra"]["whiteboards"][0]["strokes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["points"][0][0].as_f64().unwrap())
            .collect();
        let mut xs = xs;
        xs.sort_by(|p, q| p.partial_cmp(q).unwrap());
        assert_eq!(xs, vec![5.0, 9.0], "{}", v["extra"]["whiteboards"]);
    }
}

#[test]
fn touching_a_field_by_hand_wins_for_that_field_only() {
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &ma);
    a["members"] = json!([{"id": 3, "name": "Kim", "role": "Member"}]);
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    b["members"][0]["name"] = json!("Kim Lee");
    save(&mut b, &mut mb, 20);
    // the server labels the role later, not having seen the rename
    a["members"][0]["role"] = json!("Admin");
    let mut m = meta_of(&a);
    touch_fields(&mut m, 3, &["role"], 30);
    a["extra"]["sync"] = serde_json::to_value(&m).unwrap();
    merge(&mut a, &[b]);
    assert_eq!(a["members"][0]["name"], "Kim Lee");
    assert_eq!(a["members"][0]["role"], "Admin");
}

#[test]
fn board_ids_are_the_same_on_the_phone() {
    // the iPhone test (testBoardIdsMatchTheRustOnes) checks the same values
    assert_eq!(
        board_item_id(50, "strokes", "0,0,0|1,13;11,20", 0),
        6148156056053566450
    );
    assert_eq!(
        board_sig(
            "strokes",
            &json!({"color": [0, 0, 0], "points": [[0.1, 1.25], [1.1, 2.0]]})
        ),
        "0,0,0|1,13;11,20"
    );
    assert_eq!(
        board_sig(
            "notes",
            &json!({"color": [1, 2, 3], "text": "old", "pos": [1.0, 2.0]})
        ),
        "1,2,3|old|10|20"
    );
}

#[test]
fn an_older_copy_records_move_times_only_for_real_moves() {
    let (mut a, _, mut old, mut mo) = pair(json!([
        space(100, json!([]), json!([list(101), list(102)])),
        space(200, json!([]), json!([]))
    ]));
    a["tasks"] = json!([{"id": 7, "name": "t"}]);
    let mut ma = meta_of(&a);
    save(&mut a, &mut ma, 15);
    // an app that doesn't keep move times moves 101 and edits everything
    change(
        &mut old,
        &mut mo,
        json!([
            space(100, json!([]), json!([list(102)])),
            space(200, json!([]), json!([list(101)]))
        ]),
        30,
    );
    old["extra"]["sync"]["v"] = json!(1);
    old["extra"]["sync"]
        .as_object_mut()
        .unwrap()
        .remove("moved");
    merge(&mut a, &[old]);
    assert_eq!(places(&a), vec![(100, 102), (200, 101)]);
    let m = meta_of(&a);
    assert_eq!(
        m.moved.keys().copied().collect::<Vec<_>>(),
        vec![101],
        "only the list that moved: {:?}",
        m.moved
    );
    // a bloated map from before shrinks to the items that can move
    let mut bloated: BTreeMap<u64, i64> = [(7, 5), (100, 5), (101, 30), (102, 5), (999, 5)].into();
    shrink_moved(&mut bloated, &a);
    assert_eq!(bloated, [(101, 30), (102, 5)].into());
}

#[test]
fn personal_items_stay_on_this_computer() {
    let (mut a, _, _, _) = pair(json!([]));
    a["extra"]["bookmarks"] = json!([{"id": 5, "title": "Mine", "url": "u", "personal": true}, {"id": 6, "title": "Team", "url": "v"}]);
    let out = shareable(&a);
    assert_eq!(
        out["extra"]["bookmarks"],
        json!([{"id": 6, "title": "Team", "url": "v"}])
    );
    // and merging the team's copy (without it) keeps it here
    let mut ma = meta_of(&a);
    save(&mut a, &mut ma, 20);
    let team = shareable(&a);
    merge(&mut a, &[team]);
    assert_eq!(a["extra"]["bookmarks"].as_array().unwrap().len(), 2);
}

#[test]
fn copies_from_older_apps_are_told_apart() {
    let new = json!({"app": "0.1.7", "extra": {"sync": {"v": META_V}}});
    assert_eq!(older_app(&new, "0.1.7"), None);
    assert_eq!(
        older_app(&new, "0.1.10"),
        Some("0.1.7"),
        "compared as numbers"
    );
    assert_eq!(
        older_app(&json!({"extra": {"sync": {"v": 2}}}), "0.1.7"),
        Some("")
    );
    assert_eq!(
        older_app(
            &json!({"app": "0.1.7", "extra": {"sync": {"v": 2}}}),
            "0.1.7"
        ),
        Some("0.1.7"),
        "an older sync format"
    );
    let dir = std::env::temp_dir().join(format!("wb-sync-app-{:x}", random_u64()));
    write_mine(&dir, "d1", &new, "0.1.7").unwrap();
    let back = read_json(&dir.join(format!("d1{}", FILE_SUFFIX))).unwrap();
    assert_eq!(app_version_of(&back), Some("0.1.7"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Strip what a merge result doesn't need for comparing.
fn items(v: &Value) -> Value {
    let mut v = v.clone();
    v["extra"].as_object_mut().map(|e| e.remove("sync"));
    v
}

#[test]
fn a_delta_holds_only_what_changed_and_merges_like_the_whole_copy() {
    let mut m = SyncMeta::default();
    let mut a = ws(
        json!([{"id": 1, "name": "one"}, {"id": 2, "name": "two"}]),
        chat(json!([])),
        &m,
    );
    a["spaces"] = json!([
        space(100, json!([]), json!([list(101)])),
        space(200, json!([]), json!([]))
    ]);
    save(&mut a, &mut m, 10);
    let other = a.clone();
    let mut sum = Summary::of(&shareable(&a));
    assert_eq!(delta(&sum, &shareable(&a)), None, "nothing changed");
    // a new message, a renamed task, a deleted task, a moved list, a setting
    a["channels"][0]["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": 12, "author": 1, "text": "new", "ts": 6}));
    a["tasks"] = json!([{"id": 1, "name": "one!"}]);
    a["spaces"] = json!([
        space(100, json!([]), json!([])),
        space(200, json!([]), json!([list(101)]))
    ]);
    a["workspace"] = json!("Renamed");
    save(&mut a, &mut m, 20);
    let d = delta(&sum, &shareable(&a)).unwrap();
    assert_eq!(d["tasks"], json!([{"id": 1, "name": "one!"}]));
    let msgs = d["channels"][0]["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1, "only the new message: {}", d["channels"]);
    assert_eq!(msgs[0]["text"], "new");
    assert_eq!(d["workspace"], "Renamed");
    assert!(d["extra"]["sync"]["tombs"].get("2").is_some());
    assert!(d.get("members").is_none() && d.get("docs").is_none());
    // merging the delta is the same as merging the whole copy
    let (mut x, mut y) = (other.clone(), other.clone());
    merge(&mut x, &[d.clone()]);
    merge(&mut y, &[shareable(&a)]);
    assert_eq!(items(&x), items(&y));
    assert_eq!(places(&x), vec![(200, 101)]);
    assert_eq!(meta_of(&x).stamps, meta_of(&y).stamps);
    // once the other side has it, nothing is left to send
    sum.absorb(&d);
    assert_eq!(delta(&sum, &shareable(&a)), None);
}

#[test]
fn private_dashboards_and_personal_forms_are_hidden() {
    let d = json!({"workspace": "W", "members": [], "channels": [], "docs": [], "tasks": [],
        "spaces": [{"id": 50, "name": "Personal", "private_owner": 1, "folders": [{"id": 53, "name": "F", "lists": [{"id": 54, "name": "In folder"}]}], "lists": [{"id": 51, "name": "Mine"}]},
                   {"id": 60, "name": "Bo's", "private_owner": 2, "folders": [], "lists": [{"id": 61, "name": "Bo"}]},
                   space(70, json!([]), json!([list(71)]))],
        "extra": {"sync": {},
            "dashboards": [{"id": 90, "name": "a", "owner": 1, "private": true},
                           {"id": 91, "name": "b", "owner": 2, "private": true},
                           {"id": 92, "name": "c", "owner": 0, "private": true},
                           {"id": 93, "name": "d", "owner": 1},
                           {"id": 94, "name": "e", "owner": 3, "space": 50},
                           {"id": 95, "name": "f", "owner": 3, "space": 70}],
            "forms": [{"id": 100, "name": "x", "list_id": 51},
                      {"id": 101, "name": "y", "list_id": 54},
                      {"id": 102, "name": "z", "list_id": 61},
                      {"id": 103, "name": "w", "list_id": 71}]}});
    let mut hidden = hidden_from(&d, 2, false);
    hidden.sort();
    assert_eq!(hidden, vec![50, 90, 94, 100, 101]);
    // the owner of the Personal space and maker of the dashboard sees them
    let mut theirs = hidden_from(&d, 1, false);
    theirs.sort();
    assert_eq!(theirs, vec![60, 91, 102]);
    let mut out = d.clone();
    hide(&mut out, &hidden);
    let names: Vec<&str> = out["extra"]["dashboards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["b", "c", "d", "f"]);
    assert_eq!(out["extra"]["forms"].as_array().unwrap().len(), 2);
}

#[test]
fn personal_settings_and_reading_state_stay_home() {
    let mut m = SyncMeta::default();
    m.hashes.insert(1, 2);
    let d = json!({"workspace": "W", "me": 1, "tasks": [{"id": 7, "name": "T"}],
        "extra": {"sync": m, "notepad": "secret note", "reminders": [{"id": 1}],
            "chat_seen": {"10": {"floor": 5, "ids": [11]}}, "closed_dm_seen": {"20": {"floor": 1, "ids": []}},
            "closed_dms": [[20, 5]], "starred": ["doc:1"], "recent": ["task:7"], "replies_seen": 9,
            "replies_read": ["chat:1:2"], "notify": {"comments": false}, "muted_until": 99,
            "shared": {"folder": "/Users/ann/Dropbox/team", "device": "d1", "tag": 1},
            "sheet_rows": [[1, 7]], "accent": "pink", "labs": ["x"], "slack_under_me": ["U1"],
            "dashboards": [{"id": 8, "name": "D"}], "workspace_color": [1, 2, 3]}});
    let out = shareable(&d);
    let e = out["extra"].as_object().unwrap();
    for k in PERSONAL_EXTRA {
        assert!(!e.contains_key(*k), "{} left out", k);
    }
    assert!(!out.to_string().contains("secret note"));
    assert!(!out.to_string().contains("Dropbox"));
    assert_eq!(out["extra"]["sync"]["hashes"], json!({}));
    // what the team shares is all there
    assert_eq!(out["extra"]["dashboards"][0]["name"], "D");
    assert_eq!(out["extra"]["workspace_color"], json!([1, 2, 3]));
    assert_eq!(out["tasks"], d["tasks"]);
}

#[test]
fn spaces_shared_with_chosen_people_are_hidden_from_everyone_else() {
    let shared = |people: Value, everyone: bool| {
        json!({"id": 70, "name": "Plans", "folders": [{"id": 73, "name": "F", "lists": [{"id": 74, "name": "In folder"}]}], "lists": [{"id": 71, "name": "L"}],
               "sharing": {"everyone": everyone, "people": people}})
    };
    let people = json!([{"member": 2, "level": "view"}, {"member": 3, "level": "comment"}, {"member": 4, "level": "edit"}, {"member": 5, "level": "full"}, {"member": 6, "level": "someday"}]);
    let s = shared(people.clone(), false);
    assert!(chosen_only(&s));
    let lv = |m: u64, manages: bool| space_level(&s, m, manages);
    assert_eq!(lv(2, false), Some(Level::View));
    assert_eq!(lv(3, false), Some(Level::Comment));
    assert_eq!(lv(4, false), Some(Level::Edit));
    assert_eq!(lv(5, false), Some(Level::Full));
    assert_eq!(
        lv(6, false),
        Some(Level::View),
        "an unknown level gives the least"
    );
    assert_eq!(lv(7, false), None);
    assert_eq!(
        lv(7, true),
        Some(Level::Full),
        "owners and admins see every shared space"
    );
    // back to everyone (the people stay for next time), and spaces from before sharing
    let open = shared(people, true);
    assert!(!chosen_only(&open));
    assert_eq!(space_level(&open, 7, false), Some(Level::Full));
    assert_eq!(
        space_level(&json!({"id": 1, "name": "Old"}), 7, false),
        Some(Level::Full)
    );
    // a Personal space stays its owner's, even for admins
    let personal = json!({"id": 1, "private_owner": 2});
    assert_eq!(space_level(&personal, 1, true), None);
    assert_eq!(space_level(&personal, 2, false), Some(Level::Full));

    let d = json!({"workspace": "W", "members": [], "channels": [],
        "spaces": [shared(json!([{"member": 2, "level": "view"}]), false), space(80, json!([]), json!([list(81)]))],
        "tasks": [{"id": 72, "list_id": 71, "name": "secret"}, {"id": 75, "list_id": 74, "name": "in folder"}, {"id": 82, "list_id": 81, "name": "open"}],
        "docs": [{"id": 90, "owner": 2, "space": 70}, {"id": 91, "owner": 1, "parent": 90}, {"id": 92, "owner": 1}],
        "extra": {"sync": {}, "dashboards": [{"id": 93, "name": "d", "space": 70}], "forms": [{"id": 94, "name": "f", "list_id": 74}]}});
    let mut hidden = hidden_from(&d, 3, false);
    hidden.sort();
    assert_eq!(hidden, vec![70, 72, 75, 90, 91, 93, 94]);
    assert!(
        hidden_from(&d, 2, false).is_empty(),
        "a chosen person sees it"
    );
    assert!(hidden_from(&d, 3, true).is_empty(), "so does an admin");
    assert_eq!(ids_within(&d["spaces"][0]), vec![70, 71, 73, 74]);
    let own = own_fields(&d["spaces"][0]);
    assert!(own.contains_key("sharing") && !own.contains_key("lists"));
}

#[test]
fn a_stroke_without_an_id_leaves_the_known_strokes_alone() {
    let stroke = |id: u64, x: f64| {
        let mut s = json!({"color": [0, 0, 0], "width": 2.0, "points": [[x, 1.0]]});
        if id != 0 {
            s["id"] = json!(id);
        }
        s
    };
    let mut m = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &m);
    a["extra"]["whiteboards"] = json!([{"id": 50, "name": "B", "strokes": [stroke(51, 1.0), stroke(52, 2.0)], "notes": []}]);
    stamp(&a.clone(), &mut m, 10);
    // a stroke just drawn, before it gets its id (left out, or written as 0)
    for fresh in [
        stroke(0, 3.0),
        json!({"id": 0, "color": [0, 0, 0], "width": 2.0, "points": [[4.0, 1.0]]}),
    ] {
        let mut m2 = m.clone();
        let mut b = a.clone();
        b["extra"]["whiteboards"][0]["strokes"]
            .as_array_mut()
            .unwrap()
            .push(fresh.clone());
        stamp(&b.clone(), &mut m2, 20);
        assert!(m2.tombs.is_empty(), "nothing deleted: {:?}", m2.tombs);
        assert!(m2.stamps.contains_key(&51) && m2.stamps.contains_key(&52));
        assert!(m2.stamps[&50] > m.stamps[&50], "the board changed");
    }
}

#[test]
fn entries_without_an_id_stay_beside_the_items_they_sit_with() {
    let mut ma = SyncMeta::default();
    let mut a = ws(
        json!([{"id": 1, "name": "t", "things": [{"id": 5, "name": "five"}, {"id": 6, "name": "six"}]}]),
        json!([]),
        &ma,
    );
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    // A adds an entry without an id; B renames item 6
    a["tasks"][0]["things"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "loose"}));
    save(&mut a, &mut ma, 20);
    assert!(ma.tombs.is_empty());
    b["tasks"][0]["things"][1]["name"] = json!("six by B");
    b["tasks"][0]["name"] = json!("renamed by B");
    save(&mut b, &mut mb, 30);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(
            v["tasks"][0]["things"],
            json!([{"id": 5, "name": "five"}, {"id": 6, "name": "six by B"}, {"name": "loose"}]),
            "{}",
            v["tasks"][0]
        );
        assert_eq!(v["tasks"][0]["name"], "renamed by B");
    }
    // a delta carries the entry without an id along with its item
    let mut since = Summary::of(&b);
    let mut mb = meta_of(&b);
    mb.field_hashes = part_hashes(&b);
    b["tasks"][0]["name"] = json!("again");
    save(&mut b, &mut mb, 40);
    let d = delta(&since, &b).unwrap();
    assert_eq!(d["tasks"][0]["things"], json!([{"name": "loose"}]));
    since.absorb(&d);
    assert!(delta(&since, &b).is_none());
}

fn entries_of(v: &Value) -> Vec<(u64, i64, i64)> {
    v["tasks"][0]["time_entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["member"].as_u64().unwrap_or(0),
                e["start"].as_i64().unwrap(),
                e["end"].as_i64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn two_people_stopping_their_timers_at_once_both_keep_their_time() {
    let mut ma = SyncMeta::default();
    let mut a = ws(
        json!([{"id": 1, "name": "t", "time_entries": [{"start": 1, "end": 5}]}]),
        json!([]),
        &ma,
    );
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    a["tasks"][0]["time_entries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"start": 100, "end": 200, "member": 1}));
    save(&mut a, &mut ma, 20);
    b["tasks"][0]["time_entries"]
        .as_array_mut()
        .unwrap()
        .push(json!({"start": 90, "end": 201, "member": 2}));
    save(&mut b, &mut mb, 21);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(
            entries_of(v),
            vec![(0, 1, 5), (2, 90, 201), (1, 100, 200)],
            "{}",
            v["tasks"][0]
        );
    }
    assert_eq!(a["tasks"], b["tasks"]);
    // A deletes the old entry and fixes B's end while B renames the task
    let (mut ma, mut mb) = (meta_of(&a), meta_of(&b));
    ma.field_hashes = part_hashes(&a);
    mb.field_hashes = part_hashes(&b);
    a["tasks"][0]["time_entries"] =
        json!([{"start": 90, "end": 150, "member": 2}, {"start": 100, "end": 200, "member": 1}]);
    save(&mut a, &mut ma, 30);
    b["tasks"][0]["name"] = json!("renamed");
    save(&mut b, &mut mb, 40);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(v["tasks"][0]["name"], "renamed");
        assert_eq!(
            entries_of(v),
            vec![(2, 90, 150), (1, 100, 200)],
            "the deleted entry stays deleted: {}",
            v["tasks"][0]
        );
    }
    // the last one deleted: an empty list, not the old entries back
    let (mut ma, mut mb) = (meta_of(&a), meta_of(&b));
    ma.field_hashes = part_hashes(&a);
    mb.field_hashes = part_hashes(&b);
    a["tasks"][0]["time_entries"] = json!([]);
    save(&mut a, &mut ma, 50);
    b["tasks"][0]["name"] = json!("again");
    save(&mut b, &mut mb, 60);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(
            v["tasks"][0]["time_entries"],
            json!([]),
            "{}",
            v["tasks"][0]
        );
    }
}

#[test]
fn a_whole_field_stamp_from_an_older_app_counts_for_each_entry() {
    // an app from before entries were split stamped "time_entries" whole
    let mut ma = SyncMeta::default();
    let mut a = ws(
        json!([{"id": 1, "name": "t", "time_entries": [{"start": 1, "end": 5}], "custom": {"7": "x"}}]),
        json!([]),
        &ma,
    );
    save(&mut a, &mut ma, 10);
    let mut old = a.clone();
    old["tasks"][0]["time_entries"] = json!([{"start": 1, "end": 9}]);
    old["tasks"][0]["custom"] = json!({"7": "y"});
    old["extra"]["sync"] = json!({"v": 3, "stamps": {"1": 30}, "fields": {"1": {"": 10, "time_entries": 30, "custom": 30}}});
    // A edits something else in between
    a["tasks"][0]["name"] = json!("A");
    save(&mut a, &mut ma, 20);
    merge(&mut a, &[old]);
    assert_eq!(entries_of(&a), vec![(0, 1, 9)]);
    assert_eq!(a["tasks"][0]["custom"], json!({"7": "y"}));
    assert_eq!(a["tasks"][0]["name"], "A");
    // fingerprints kept from before: the new split alone is no change
    let mut m = SyncMeta::default();
    let t = json!([{"id": 1, "name": "t", "time_entries": [{"start": 1, "end": 5}], "custom": {"7": "x"}}]);
    let v = ws(t, json!([]), &m);
    stamp(&v, &mut m, 10);
    let h = &mut m.field_hashes.get_mut(&1).unwrap();
    h.retain(|p, _| part_field(p) != "time_entries" && part_field(p) != "custom");
    h.insert(
        "time_entries".into(),
        part_hash(&json!([{"start": 1, "end": 5}])),
    );
    h.insert("custom".into(), part_hash(&json!({"7": "x"})));
    let mut v2 = v.clone();
    v2["tasks"][0]["name"] = json!("renamed");
    stamp(&v2, &mut m, 20);
    let e = &m.fields[&1];
    assert!(
        e.keys().all(|p| p == "name" || p.is_empty()),
        "only the name changed: {:?}",
        e
    );
}

#[test]
fn different_custom_fields_changed_at_once_both_stay() {
    let mut ma = SyncMeta::default();
    let mut a = ws(
        json!([{"id": 1, "name": "t", "custom": {"7": "x", "8": "y"}}]),
        json!([]),
        &ma,
    );
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    a["tasks"][0]["custom"]["7"] = json!("x by A");
    save(&mut a, &mut ma, 20);
    b["tasks"][0]["custom"]["9"] = json!("z by B");
    save(&mut b, &mut mb, 30);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(
            v["tasks"][0]["custom"],
            json!({"7": "x by A", "8": "y", "9": "z by B"})
        );
    }
    // one taken away while the other side changes another
    let (mut ma, mut mb) = (meta_of(&a), meta_of(&b));
    ma.field_hashes = part_hashes(&a);
    mb.field_hashes = part_hashes(&b);
    a["tasks"][0]["custom"].as_object_mut().unwrap().remove("8");
    save(&mut a, &mut ma, 40);
    b["tasks"][0]["custom"]["9"] = json!("z again");
    save(&mut b, &mut mb, 50);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(
            v["tasks"][0]["custom"],
            json!({"7": "x by A", "9": "z again"})
        );
    }
}

#[test]
fn reactions_end_up_in_the_same_order_everywhere() {
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([]), chat(json!([])), &ma);
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    a["channels"] = chat(json!([{"emoji": "👍", "users": [2, 1]}, {"emoji": "❤️", "users": [3]}]));
    save(&mut a, &mut ma, 20);
    b["channels"] = chat(json!([{"emoji": "❤️", "users": [3]}, {"emoji": "👍", "users": [1, 2]}]));
    save(&mut b, &mut mb, 20);
    swap(&mut a, &mut b);
    assert_eq!(
        a["channels"][0]["messages"][0]["reactions"],
        b["channels"][0]["messages"][0]["reactions"]
    );
    assert_eq!(fingerprints(&a), fingerprints(&b));
    assert_eq!(
        a["channels"][0]["messages"][0]["reactions"],
        json!([{"emoji": "❤️", "users": [3]}, {"emoji": "👍", "users": [1, 2]}])
    );
}

#[test]
fn field_fingerprints_filled_for_an_older_file_make_an_emptied_field_stick() {
    // a copy saved before field fingerprints were kept
    let base = json!([{"id": 1, "name": "t", "attachments": [{"file": "x"}]}]);
    let mut m = SyncMeta::default();
    let v0 = ws(base.clone(), json!([]), &m);
    stamp(&v0, &mut m, 10);
    m.field_hashes.clear();
    m.fields.clear();
    m.v = 2;
    let team = {
        let mut t = v0.clone();
        t["extra"]["sync"] = serde_json::to_value(&m).unwrap();
        t
    };
    for fill in [false, true] {
        let mut mine = m.clone();
        let mut v = ws(base.clone(), json!([]), &mine);
        if fill {
            assert!(fill_field_hashes(&mut mine, &v));
            assert!(mine.field_hashes.contains_key(&1));
        }
        // the last attachment removed before the first sync
        v["tasks"][0].as_object_mut().unwrap().remove("attachments");
        stamp(&v.clone(), &mut mine, 20);
        v["extra"]["sync"] = serde_json::to_value(&mine).unwrap();
        merge(&mut v, &[team.clone()]);
        assert_eq!(
            v["tasks"][0].get("attachments").is_none(),
            fill,
            "filled: {} {}",
            fill,
            v["tasks"][0]
        );
    }
    // an item changed since the last save is left alone
    let mut mine = m.clone();
    let mut v = ws(base, json!([]), &mine);
    v["tasks"][0]["name"] = json!("changed");
    assert!(!fill_field_hashes(&mut mine, &v));
}

#[test]
fn a_folder_brought_back_whole_comes_with_its_lists() {
    let mut m = SyncMeta::default();
    let mut d = json!({"workspace": "W", "members": [], "channels": [], "tasks": [], "docs": [],
        "spaces": [space(80, json!([folder(73, json!([list(74)]))]), json!([list(81)]))], "extra": {}});
    stamp(&d.clone(), &mut m, 10);
    d["extra"]["sync"] = serde_json::to_value(&m).unwrap();
    // a delta holding the space and the folder without its list
    let mut part = json!({"spaces": [{"id": 80, "name": "s", "folders": [{"id": 73, "name": "f"}]}], "extra": {"sync": {}}});
    include_whole(&mut part, &d, &[73, 74]);
    assert_eq!(part["spaces"][0]["folders"][0]["lists"][0]["id"], 74);
    assert_eq!(part["spaces"][0]["folders"].as_array().unwrap().len(), 1);
    let pm = meta_of(&part);
    assert!(pm.stamps.contains_key(&73) && pm.stamps.contains_key(&74));
    // not in the delta at all: it comes inside its space, without the rest
    let mut part = json!({"extra": {"sync": {}}});
    include_whole(&mut part, &d, &[74]);
    assert_eq!(
        part["spaces"][0]["folders"][0]["lists"],
        json!([d["spaces"][0]["folders"][0]["lists"][0]])
    );
    assert!(part["spaces"][0].get("lists").is_none());
    assert_eq!(ids_inside(&d, &[80]), vec![73, 74, 80, 81]);
}

#[test]
fn a_format_3_copy_becomes_format_4_without_counting_as_an_edit() {
    let t = json!([{"id": 1, "name": "t", "time_entries": [{"start": 1, "end": 5, "member": 2}], "custom": {"7": "x"}}]);
    // saved by a round-3 app: one fingerprint for each whole field
    let mut old = SyncMeta::default();
    let v = ws(t.clone(), json!([]), &old);
    stamp(&v, &mut old, 10);
    let h = old.field_hashes.get_mut(&1).unwrap();
    h.retain(|p, _| part_field(p) != "time_entries" && part_field(p) != "custom");
    h.insert(
        "time_entries".into(),
        part_hash(&json!([{"start": 1, "end": 5, "member": 2}])),
    );
    h.insert("custom".into(), part_hash(&json!({"7": "x"})));
    old.v = 3;
    // and by a round-4 app, which split them already but wrote 3
    let mut split = SyncMeta::default();
    stamp(&v, &mut split, 10);
    split.v = 3;
    for mut m in [old, split] {
        let before = (m.stamps.clone(), m.fields.clone());
        stamp(&v, &mut m, 20);
        assert_eq!(m.v, META_V);
        assert_eq!(META_V, 5);
        assert_eq!(
            (m.stamps.clone(), m.fields.clone()),
            before,
            "the new format alone is no change"
        );
        let parts = &m.field_hashes[&1];
        assert!(parts.contains_key("time_entries/2/1"), "{:?}", parts);
        assert!(parts.contains_key("custom/7"));
        assert!(!parts.contains_key("time_entries") && !parts.contains_key("custom"));
    }
}

#[test]
fn a_slack_id_taken_off_a_member_stays_off() {
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &ma);
    a["members"] = json!([{"id": 4, "name": "A", "color": [1, 2, 3], "slack": ["U1"]}, {"id": 5, "name": "B", "color": [1, 2, 3]}]);
    save(&mut a, &mut ma, 10);
    let mut b = a.clone();
    let mut mb = meta_of(&b);
    mb.field_hashes = part_hashes(&b);
    // B's computer moves U1 from A to B (A's list is now empty: left out)
    b["members"][0].as_object_mut().unwrap().remove("slack");
    b["members"][1]["slack"] = json!(["U1"]);
    save(&mut b, &mut mb, 20);
    // meanwhile A's computer renames A
    a["members"][0]["name"] = json!("A renamed");
    save(&mut a, &mut ma, 30);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(v["members"][0]["name"], "A renamed");
        assert!(
            v["members"][0].get("slack").is_none(),
            "{}",
            v["members"][0]
        );
        assert_eq!(v["members"][1]["slack"], json!(["U1"]));
    }
}

fn slack_map(v: &Value) -> Value {
    v["extra"].get("slack_map").cloned().unwrap_or(Value::Null)
}

/// The same steps and results as the Swift test of the same name
/// (`testSharedMapEntriesMergeOneByOne`), so the phone merges a shared
/// map exactly like a Mac.
#[test]
fn shared_map_entries_merge_one_by_one() {
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &ma);
    a["extra"]["slack_map"] = json!({"U0": 7});
    save(&mut a, &mut ma, 10);
    assert!(
        ma.values.is_empty(),
        "a copy stamped for the first time keeps no stamp"
    );
    let (mut b, mut mb) = (a.clone(), ma.clone());
    // A points UA at 2, then B points UA at 3 and UB at 2
    a["extra"]["slack_map"]["UA"] = json!(2);
    save(&mut a, &mut ma, 20);
    assert_eq!(ma.values.get("extra.slack_map/UA"), Some(&20));
    b["extra"]["slack_map"]["UA"] = json!(3);
    b["extra"]["slack_map"]["UB"] = json!(2);
    save(&mut b, &mut mb, 30);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(slack_map(v), json!({"U0": 7, "UA": 3, "UB": 2}));
        assert_eq!(meta_of(v).values.get("extra.slack_map/UA"), Some(&30));
    }
    // at once: A moves U0, B moves UB; both stay
    let (mut ma, mut mb) = (meta_of(&a), meta_of(&b));
    ma.value_hashes = value_hashes(&a);
    mb.value_hashes = value_hashes(&b);
    a["extra"]["slack_map"]["U0"] = json!(9);
    save(&mut a, &mut ma, 40);
    b["extra"]["slack_map"]["UB"] = json!(4);
    save(&mut b, &mut mb, 41);
    // and later B takes UA away
    b["extra"]["slack_map"]
        .as_object_mut()
        .unwrap()
        .remove("UA");
    save(&mut b, &mut mb, 50);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(slack_map(v), json!({"U0": 9, "UB": 4}));
        assert_eq!(meta_of(v).values.get("extra.slack_map/UA"), Some(&50));
    }
    // a copy that never had UA set (stamp 0) doesn't bring it back, and
    // one set again later does come back
    let mut old = ws(json!([]), json!([]), &SyncMeta::default());
    old["extra"]["slack_map"] = json!({"UA": 2, "UC": 5});
    let mut x = a.clone();
    merge(&mut x, &[old.clone()]);
    assert_eq!(slack_map(&x), json!({"U0": 9, "UB": 4, "UC": 5}));
    // two copies that each worked out an entry nobody decided (stamp 0)
    // end up with the same one
    let mut p = ws(json!([]), json!([]), &SyncMeta::default());
    p["extra"]["slack_map"] = json!({"UD": 2});
    let mut q = ws(json!([]), json!([]), &SyncMeta::default());
    q["extra"]["slack_map"] = json!({"UD": 3});
    swap(&mut p, &mut q);
    assert_eq!(slack_map(&p), json!({"UD": 3}));
    assert_eq!(slack_map(&q), json!({"UD": 3}));
    // a copy without the map gets none made up
    let mut e = ws(json!([]), json!([]), &SyncMeta::default());
    merge(&mut e, &[ws(json!([]), json!([]), &SyncMeta::default())]);
    assert!(e["extra"].get("slack_map").is_none());
}

#[test]
fn a_carried_map_entry_is_not_a_decision() {
    let mut m = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &m);
    save(&mut a, &mut m, 10);
    // an upgrade works one out of older data
    a["extra"]["slack_map"] = json!({"UA": 2});
    carry_map_entry(&mut m, "extra.slack_map", "UA", &json!(2));
    save(&mut a, &mut m, 20);
    assert!(m.values.get("extra.slack_map/UA").is_none(), "not set now");
    // a decision on another copy wins over it
    let mut b = a.clone();
    let mut mb = m.clone();
    b["extra"]["slack_map"]["UA"] = json!(3);
    save(&mut b, &mut mb, 15);
    merge(&mut a, &[b]);
    assert_eq!(slack_map(&a), json!({"UA": 3}));
    // without the carry it would have counted as set now
    let mut n = SyncMeta::default();
    let mut c = ws(json!([]), json!([]), &n);
    save(&mut c, &mut n, 10);
    c["extra"]["slack_map"] = json!({"UA": 2});
    save(&mut c, &mut n, 20);
    assert_eq!(n.values.get("extra.slack_map/UA"), Some(&20));
}

#[test]
fn a_delta_carries_only_the_changed_map_entries() {
    let mut m = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &m);
    a["extra"]["slack_map"] = json!({"UA": 2, "UB": 3});
    save(&mut a, &mut m, 10);
    let other = a.clone();
    let mut sum = Summary::of(&shareable(&a));
    assert_eq!(delta(&sum, &shareable(&a)), None);
    a["extra"]["slack_map"]["UA"] = json!(4);
    a["extra"]["slack_map"]["UC"] = json!(5);
    a["extra"]["slack_map"]
        .as_object_mut()
        .unwrap()
        .remove("UB");
    save(&mut a, &mut m, 20);
    let d = delta(&sum, &shareable(&a)).unwrap();
    assert_eq!(d["extra"]["slack_map"], json!({"UA": 4, "UC": 5}));
    assert_eq!(
        d["extra"]["sync"]["values"]["extra.slack_map/UB"],
        json!(20)
    );
    let (mut x, mut y) = (other.clone(), other.clone());
    merge(&mut x, &[d.clone()]);
    merge(&mut y, &[shareable(&a)]);
    assert_eq!(slack_map(&x), json!({"UA": 4, "UC": 5}));
    assert_eq!(slack_map(&x), slack_map(&y));
    assert_eq!(meta_of(&x).values, meta_of(&y).values);
    sum.absorb(&d);
    assert_eq!(delta(&sum, &shareable(&a)), None);
    // joining takes the team's map
    let joined = with_team_things(&json!({"extra": {"slack_map": {"UZ": 1}}}), &a);
    assert_eq!(joined["extra"]["slack_map"], json!({"UA": 4, "UC": 5}));
}

#[test]
fn an_older_app_that_drops_the_map_takes_no_entry_away() {
    let mut m = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &m);
    save(&mut a, &mut m, 10);
    a["extra"]["slack_map"] = json!({"UA": 2});
    save(&mut a, &mut m, 20);
    // an older app read the team's copy: it kept the stamps (all of
    // `values`) but not the map it doesn't know, and its fingerprints
    // are of the shared settings only
    let mut old = a.clone();
    old["extra"].as_object_mut().unwrap().remove("slack_map");
    let mut om = meta_of(&old);
    om.value_hashes
        .retain(|p, _| SHARED_VALUES.contains(&p.as_str()));
    assert_eq!(om.values.get("extra.slack_map/UA"), Some(&20));
    old["extra"]["sync"] = serde_json::to_value(&om).unwrap();
    let mut x = a.clone();
    merge(&mut x, &[old.clone()]);
    assert_eq!(slack_map(&x), json!({"UA": 2}));
    let mut y = old.clone();
    merge(&mut y, &[a.clone()]);
    assert_eq!(slack_map(&y), json!({"UA": 2}));
}

#[test]
fn map_entries_an_older_copy_kept_are_not_edits_after_opening() {
    // an older app kept the map (from the team's copy) but fingerprinted
    // the shared settings only; UA's last decision was at 100
    let mut m = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &m);
    save(&mut a, &mut m, 10);
    a["extra"]["slack_map"] = json!({"UA": 2});
    m.value_hashes
        .retain(|p, _| SHARED_VALUES.contains(&p.as_str()));
    m.values.insert("extra.slack_map/UA".into(), 100);
    // without the baseline the next change takes UA as set now
    let mut n = m.clone();
    stamp(&a, &mut n, 1000);
    assert_eq!(n.values.get("extra.slack_map/UA"), Some(&1000));
    // opening the copy records it as it is
    fill_value_hashes(&mut m, &a);
    a["workspace"] = json!("W2");
    save(&mut a, &mut m, 1000);
    assert_eq!(m.values.get("extra.slack_map/UA"), Some(&100));
    assert_eq!(m.values.get("workspace"), Some(&1000));
    // so a teammate's later decision wins
    let mut b = a.clone();
    b["extra"]["slack_map"]["UA"] = json!(3);
    let mut mb = meta_of(&b);
    mb.values.insert("extra.slack_map/UA".into(), 200);
    b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    merge(&mut a, &[b]);
    assert_eq!(slack_map(&a), json!({"UA": 3}));
    // an entry added after opening is still a decision, and a recorded
    // fingerprint is never replaced
    a["extra"]["slack_map"]["UB"] = json!(4);
    let mut ma = meta_of(&a);
    let kept = ma.value_hashes.clone();
    fill_value_hashes(&mut ma, &json!({"workspace": "other"}));
    assert_eq!(ma.value_hashes, kept);
    save(&mut a, &mut ma, 2000);
    assert_eq!(ma.values.get("extra.slack_map/UB"), Some(&2000));
}

#[test]
fn two_people_changing_different_targets_of_a_goal_both_keep_theirs() {
    let num = |current: f64| json!({"Number": {"start": 0.0, "target": 10.0, "current": current, "unit": ""}});
    let line =
        |ts: i64, who: u64, pct: u64| json!({"ts": ts, "who": who, "what": "Created", "pct": pct});
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([]), json!([]), &ma);
    a["extra"]["goals"] = json!([{"id": 70, "name": "G", "color": [0, 0, 0], "due": null, "owner": 1,
        "targets": [
            {"name": "Signups", "kind": num(0.0)},
            {"name": "Review", "kind": {"TrueFalse": {"done": false}}}
        ],
        "log": [line(100, 1, 0)]}]);
    // a copy saved before targets had ids works out the same ids
    let mut old = a.clone();
    save(&mut a, &mut ma, 10);
    fill_goal_ids(&mut old);
    let ids = |v: &Value, k: &str| -> Vec<u64> {
        v["extra"]["goals"][0][k]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_u64().unwrap())
            .collect()
    };
    assert_eq!(ids(&a, "targets"), ids(&old, "targets"));
    assert_eq!(ids(&a, "log"), ids(&old, "log"));
    assert!(ids(&a, "targets").iter().all(|i| *i >= 1 << 62));
    let (mut b, mut mb) = (a.clone(), ma.clone());

    // at once: A records 6 signups, B ticks the review; each adds a line
    a["extra"]["goals"][0]["targets"][0]["kind"] = num(6.0);
    a["extra"]["goals"][0]["log"].as_array_mut().unwrap().push(json!({"ts": 200, "who": 1, "what": {"Number": {"target": "Signups", "from": 0.0, "to": 6.0, "unit": ""}}, "pct": 30}));
    save(&mut a, &mut ma, 20);
    b["extra"]["goals"][0]["targets"][1]["kind"] = json!({"TrueFalse": {"done": true}});
    b["extra"]["goals"][0]["log"].as_array_mut().unwrap().push(json!({"ts": 201, "who": 2, "what": {"Done": {"target": "Review", "done": true}}, "pct": 50}));
    save(&mut b, &mut mb, 30);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        let g = &v["extra"]["goals"][0];
        assert_eq!(g["targets"][0]["kind"], num(6.0), "A's number: {}", g);
        assert_eq!(
            g["targets"][1]["kind"]["TrueFalse"]["done"], true,
            "B's tick"
        );
        let whos: Vec<u64> = g["log"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["who"].as_u64().unwrap())
            .collect();
        assert_eq!(
            whos,
            vec![1, 1, 2],
            "both lines, in time order: {}",
            g["log"]
        );
    }

    // A removes the review; B, not having seen it, records another number
    let (mut ma, mut mb) = (meta_of(&a), meta_of(&b));
    a["extra"]["goals"][0]["targets"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    save(&mut a, &mut ma, 40);
    b["extra"]["goals"][0]["targets"][0]["kind"] = num(7.0);
    save(&mut b, &mut mb, 50);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        let t = v["extra"]["goals"][0]["targets"].as_array().unwrap();
        assert_eq!(t.len(), 1, "the removal sticks: {:?}", t);
        assert_eq!(t[0]["kind"], num(7.0));
    }

    // a target like the removed one added again isn't taken for it
    let mut ma = meta_of(&a);
    a["extra"]["goals"][0]["targets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "Review", "kind": {"TrueFalse": {"done": false}}}));
    save(&mut a, &mut ma, 60);
    let mut b = b.clone();
    merge(&mut b, &[a.clone()]);
    let names: Vec<&str> = b["extra"]["goals"][0]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["Signups", "Review"],
        "it arrives: {}",
        b["extra"]["goals"][0]
    );
}

#[test]
fn goal_ids_are_the_same_on_the_phone() {
    // the iPhone test (testGoalIdsMatchTheRustOnes) checks the same values
    assert_eq!(
        goal_sig(
            "targets",
            &json!({"name": "Signups", "kind": {"Number": {"start": 0.0}}})
        ),
        "Signups|Number"
    );
    assert_eq!(
        goal_sig(
            "log",
            &json!({"ts": 1763000000, "who": 2, "pct": 50, "what": {"Milestone": 50}})
        ),
        "1763000000|2|50|Milestone:50"
    );
    assert_eq!(
        goal_sig(
            "log",
            &json!({"ts": 5, "who": 0, "pct": 0, "what": "Tracked"})
        ),
        "5|0|0|Tracked"
    );
    assert_eq!(
        goal_item_id(70, "targets", "Signups|Number", 0),
        GOAL_ID_SAMPLE
    );
}

const GOAL_ID_SAMPLE: u64 = 5095967436815532327;

#[test]
fn turning_task_updates_back_on_reaches_every_copy_and_older_apps_keep_it() {
    // `unwatched` is not in DEFAULT_WHEN_MISSING: the apps keep it in the
    // file, empty, once written, so emptying it is a change like any other
    let mut ma = SyncMeta::default();
    let mut a = ws(json!([{"id": 1, "name": "t"}]), json!([]), &ma);
    save(&mut a, &mut ma, 10);
    let (mut b, mut mb) = (a.clone(), ma.clone());
    a["tasks"][0]["unwatched"] = json!([1]);
    save(&mut a, &mut ma, 20);
    swap(&mut a, &mut b);
    let (mut ma, mut mb2) = (meta_of(&a), meta_of(&b));
    a["tasks"][0]["unwatched"] = json!([]);
    save(&mut a, &mut ma, 30);
    b["tasks"][0]["name"] = json!("renamed");
    save(&mut b, &mut mb2, 40);
    swap(&mut a, &mut b);
    for v in [&a, &b] {
        assert_eq!(v["tasks"][0]["name"], "renamed");
        assert_eq!(v["tasks"][0]["unwatched"], json!([]), "{}", v["tasks"][0]);
    }
    // an app from before the field drops it from its copy: that is not a removal
    let mut old = b.clone();
    mb = meta_of(&old);
    a["tasks"][0]["unwatched"] = json!([1]);
    save(&mut a, &mut ma, 50);
    merge(&mut old, &[a.clone()]);
    old["tasks"][0].as_object_mut().unwrap().remove("unwatched");
    old["tasks"][0]["name"] = json!("renamed again");
    save(&mut old, &mut mb, 60);
    merge(&mut a, &[old]);
    assert_eq!(a["tasks"][0]["name"], "renamed again");
    assert_eq!(a["tasks"][0]["unwatched"], json!([1]));
}

/// A small xorshift: the same copies on every run.
struct Rng(u64);

impl Rng {
    fn n(&mut self, k: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % k.max(1) as u64) as usize
    }
}

/// A workspace with what merging tidies: reactions, timers, time entries,
/// custom fields, replies, comments with and without ids, strokes without
/// ids among ones with, messages out of time order.
fn busy_workspace(r: &mut Rng) -> Value {
    let mut tasks = vec![];
    for i in 0..12u64 {
        let mut t = json!({"id": 100 + i, "list_id": 11, "name": format!("T{i}"), "tags": [], "watchers": [2]});
        match r.n(5) {
            0 => t["activity"] = json!([{"ts": 5, "author": 2, "text": "made", "comment": false}]),
            1 => t["custom"] = json!({"1": "x"}),
            2 => t["time_entries"] = json!([{"member": 2, "start": 1, "end": 2}]),
            3 => {
                t["timer_started"] = json!(9);
                t["timer_by"] = json!(3);
            }
            _ => {}
        }
        tasks.push(t);
    }
    let mut channels = vec![];
    for c in 0..3u64 {
        let msgs: Vec<Value> = (0..10u64)
            .map(|k| {
                let mut m = json!({"id": 1000 + c * 100 + k, "author": 2, "ts": 50 + k * 2 + r.n(3) as u64, "text": "m"});
                if r.n(3) == 0 {
                    let emoji = ["👍", "x"][r.n(2)];
                    m["reactions"] = json!([{"emoji": emoji, "users": [3, 2]}]);
                }
                if r.n(4) == 0 {
                    m["replies"] = json!([{"id": 5000 + c * 100 + k, "author": 3, "ts": 90, "text": "r"}]);
                }
                m
            })
            .collect();
        channels.push(json!({"id": 500 + c, "name": "c", "messages": msgs}));
    }
    json!({"workspace": "W", "members": [{"id": 2}, {"id": 3}],
        "spaces": [{"id": 10, "lists": [{"id": 11}, {"id": 12}]}, {"id": 20, "lists": []}],
        "tasks": tasks, "channels": channels, "docs": [{"id": 900, "title": "d"}],
        "extra": {"whiteboards": [{"id": 950, "strokes": [{"points": [1]}, {"id": 951, "points": [2]}]}],
            "slack_map": {"U1": 2}}})
}

/// One random edit to a copy, as an app would make it.
fn busy_edit(r: &mut Rng, v: &mut Value, id: &mut u64) {
    *id += 1;
    let pick = |r: &mut Rng, a: &Value| r.n(a.as_array().map_or(1, |a| a.len()));
    match r.n(9) {
        0 => {
            let i = pick(r, &v["tasks"]);
            v["tasks"][i]["name"] = json!(format!("n{}", id));
        }
        1 => {
            let c = pick(r, &v["channels"]);
            v["channels"][c]["messages"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id": *id, "author": 3, "ts": *id, "text": "new"}));
        }
        2 => {
            let c = pick(r, &v["channels"]);
            let m = pick(r, &v["channels"][c]["messages"]);
            let msg = &mut v["channels"][c]["messages"][m];
            let mut rs = msg["reactions"].as_array().cloned().unwrap_or_default();
            if rs.is_empty() || r.n(2) == 0 {
                rs.push(json!({"emoji": "🎉", "users": [r.n(3) + 2]}));
            } else {
                rs.pop();
            }
            msg["reactions"] = json!(rs);
        }
        3 => {
            let i = pick(r, &v["tasks"]);
            if v["tasks"].as_array().unwrap().len() > 3 {
                v["tasks"].as_array_mut().unwrap().remove(i);
            }
        }
        4 => {
            // a list moves to the other space
            if let Some(l) = v["spaces"][0]["lists"].as_array_mut().unwrap().pop() {
                v["spaces"][1]["lists"].as_array_mut().unwrap().push(l);
            }
        }
        5 => {
            let i = pick(r, &v["tasks"]);
            v["tasks"][i]["custom"] = json!({"2": *id});
        }
        6 => {
            let s = v["extra"]["whiteboards"][0]["strokes"]
                .as_array_mut()
                .unwrap();
            if r.n(2) == 0 {
                s.push(json!({"points": [*id]}));
            } else {
                s.push(json!({"id": *id, "points": [1]}));
            }
        }
        7 => {
            let i = pick(r, &v["tasks"]);
            v["tasks"][i]["timer_started"] = json!(*id);
            v["tasks"][i]["timer_by"] = json!(2);
        }
        _ => v["extra"]["slack_map"][format!("U{}", r.n(3))] = json!(r.n(3) + 2),
    }
}

/// Merging with the quick way for untouched items (and with the
/// bookkeeping kept apart, fingerprints reused) gives exactly what merging
/// every item the long way gives, bookkeeping included.
#[test]
fn untouched_items_merge_the_same_as_the_long_way() {
    let merge_long = |local: &mut Value, others: &[Value]| {
        merge::LONG_WAY.with(|l| l.set(true));
        let c = merge(local, others);
        merge::LONG_WAY.with(|l| l.set(false));
        c
    };
    let mut changed = 0;
    for seed in 1..=25u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1);
        let mut id = 10_000;
        let mut start = busy_workspace(&mut r);
        let mut m = SyncMeta::default();
        stamp(&start, &mut m, 1);
        json::set_meta(&mut start, &m);
        let mut devs = vec![start.clone(), start.clone(), start];
        for step in 0..30i64 {
            let now = 100 + step * 10;
            let d = r.n(3);
            if r.n(2) == 0 {
                for _ in 0..1 + r.n(2) {
                    busy_edit(&mut r, &mut devs[d], &mut id);
                }
                let mut m = meta_of(&devs[d]);
                stamp(&devs[d], &mut m, now);
                json::set_meta(&mut devs[d], &m);
                continue;
            }
            let o = (d + 1 + r.n(2)) % 3;
            // the other copy whole, or only what changed since this copy's
            let other = if r.n(2) == 0 {
                shareable(&devs[o])
            } else {
                match delta(&Summary::of(&devs[d]), &devs[o]) {
                    Some(p) => p,
                    None => continue,
                }
            };
            let mut long = devs[d].clone();
            let c_long = merge_long(&mut long, std::slice::from_ref(&other));
            let mut quick = devs[d].clone();
            let c_quick = merge(&mut quick, std::slice::from_ref(&other));
            assert_eq!(c_quick, c_long, "seed {seed} step {step}");
            assert_eq!(quick, long, "seed {seed} step {step}");
            // the app's way
            let mut apart = devs[d].clone();
            let mut meta = meta_of(&apart);
            apart["extra"].as_object_mut().unwrap().remove("sync");
            let c_apart = merge_apart(&mut apart, &mut meta, &[other], true);
            json::set_meta(&mut apart, &meta);
            assert_eq!(c_apart, c_long, "seed {seed} step {step}");
            assert_eq!(apart, long, "seed {seed} step {step}");
            if quick != devs[d] {
                changed += 1;
            }
            devs[d] = long;
        }
    }
    // both ways were taken, many times
    assert!(changed > 50, "{changed}");
    assert!(merge::QUICK.with(|q| q.get()) > 1000);
}

/// The server's way: merging without fingerprints gives what merging and
/// then `make_shareable` gives; `delta_advance` gives what `delta` and
/// `Summary::of` give; and an app that missed some revisions, merging one
/// part made with `delta_of` from what they changed, ends with what merging
/// each revision's part one after the other gives.
#[test]
fn the_server_merges_and_catches_up_like_the_long_way() {
    let mut caught = 0;
    for seed in 1..=20u64 {
        let mut r = Rng(seed.wrapping_mul(0xD1B54A32D192ED03) | 1);
        let mut id = 10_000;
        let mut start = busy_workspace(&mut r);
        let mut m = SyncMeta::default();
        stamp(&start, &mut m, 1);
        json::set_meta(&mut start, &m);
        let mut devs = vec![start.clone(), start.clone()];
        let mut canon = shareable(&start);
        let mut summary = Summary::of(&canon);
        // what each revision's part carried, and an app's copy at each one
        let mut changes: Vec<Changes> = vec![];
        let mut parts: Vec<Value> = vec![];
        let mut apps: Vec<Value> = vec![start.clone()];
        for step in 0..30i64 {
            let now = 100 + step * 10;
            let d = r.n(2);
            for _ in 0..1 + r.n(2) {
                busy_edit(&mut r, &mut devs[d], &mut id);
            }
            let mut m = meta_of(&devs[d]);
            stamp(&devs[d], &mut m, now);
            json::set_meta(&mut devs[d], &m);
            let push = if r.n(2) == 0 {
                shareable(&devs[d])
            } else {
                match delta(&Summary::of(&canon), &shareable(&devs[d])) {
                    Some(p) => p,
                    None => continue,
                }
            };
            let mut long = canon.clone();
            let c_long = merge(&mut long, std::slice::from_ref(&push));
            make_shareable(&mut long);
            let c = merge_unhashed(&mut canon, std::slice::from_ref(&push));
            make_shareable(&mut canon);
            assert_eq!(c, c_long, "seed {seed} step {step}");
            assert_eq!(canon, long, "seed {seed} step {step}");
            if !c {
                continue;
            }
            let old = summary.clone();
            let (part, ch) = delta_advance(&mut summary, &canon);
            assert_eq!(part, delta(&old, &canon), "seed {seed} step {step}");
            assert_eq!(summary, Summary::of(&canon), "seed {seed} step {step}");
            let part = part.unwrap_or_else(|| json!({}));
            // the device merges what came back, as a connected app does
            merge(&mut devs[d], std::slice::from_ref(&part));
            let mut app = apps.last().unwrap().clone();
            merge(&mut app, std::slice::from_ref(&part));
            changes.push(ch);
            parts.push(part);
            apps.push(app);
        }
        // a merge can move bookkeeping alone, which goes with the next
        // revision: one more, so the last revision has all of it
        let (part, ch) = delta_advance(&mut summary, &canon);
        if let Some(part) = part {
            let mut app = apps.last().unwrap().clone();
            merge(&mut app, std::slice::from_ref(&part));
            changes.push(ch);
            parts.push(part);
            apps.push(app);
        }
        // an app that has revision `from` catches up to the newest at once
        for from in 0..apps.len() {
            let mut all = Changes::default();
            for ch in &changes[from..] {
                all.add(ch);
            }
            let mut at_once = apps[from].clone();
            if let Some(p) = delta_of(&canon, &all) {
                merge(&mut at_once, &[p]);
            }
            let mut one_by_one = apps[from].clone();
            for p in &parts[from..] {
                merge(&mut one_by_one, std::slice::from_ref(p));
            }
            assert_eq!(at_once, one_by_one, "seed {seed} from {from}");
            assert_eq!(&at_once, apps.last().unwrap(), "seed {seed} from {from}");
            caught += 1;
        }
    }
    assert!(caught > 200, "{caught}");
}

fn docs_ws(docs: Value, meta: &SyncMeta) -> Value {
    json!({"workspace": "W", "spaces": [], "tasks": [], "members": [], "me": 1, "next_id": 1, "favorites": [], "channels": [], "docs": docs, "extra": {"sync": meta}})
}

fn version(id: u64, by: u64, at: i64, body: &str) -> Value {
    json!({"id": id, "by": by, "at": at, "title": "Plan", "body": body})
}

fn doc(body: &str, versions: Value) -> Value {
    json!({"id": 5, "owner": 1, "title": "Plan", "body": body, "created": 1, "updated": 1, "versions": versions})
}

fn bodies(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = v["docs"][0]["versions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|x| x["body"].as_str().unwrap_or_default().to_string())
        .collect();
    out.sort();
    out
}

/// Two people write the same doc at once: the newer body wins, as for any
/// field, and each one's version stays, so the other text can be brought back.
#[test]
fn both_peoples_doc_versions_stay_when_one_body_wins() {
    let mut ma = SyncMeta::default();
    let mut a = docs_ws(json!([doc("start", json!([]))]), &ma);
    stamp(&a.clone(), &mut ma, 100);
    let mut mb = ma.clone();
    a = docs_ws(
        json!([doc("A text", json!([version(11, 1, 200, "A text")]))]),
        &ma,
    );
    stamp(&a.clone(), &mut ma, 200);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    let mut b = docs_ws(
        json!([doc("B text", json!([version(12, 2, 300, "B text")]))]),
        &mb,
    );
    stamp(&b.clone(), &mut mb, 300);
    b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    let mut a2 = a.clone();
    assert!(merge(&mut a2, &[b.clone()]));
    assert_eq!(a2["docs"][0]["body"], "B text", "the newer body wins");
    assert_eq!(bodies(&a2), vec!["A text", "B text"]);
    merge(&mut b, &[a]);
    assert_eq!(b["docs"][0]["body"], "B text");
    assert_eq!(
        bodies(&b),
        vec!["A text", "B text"],
        "the same on both sides"
    );
}

/// An app that doesn't know versions drops them from what it sends; a body
/// it changed still comes in, and no version goes.
#[test]
fn a_copy_without_versions_takes_none_away() {
    let mut mt = SyncMeta::default();
    let mut team = docs_ws(
        json!([doc("kept", json!([version(11, 1, 100, "kept")]))]),
        &mt,
    );
    stamp(&team.clone(), &mut mt, 100);
    team["extra"]["sync"] = serde_json::to_value(&mt).unwrap();
    // the older app's own copy and bookkeeping never had them
    let plain = |body: &str| {
        let mut d = doc(body, json!([]));
        d.as_object_mut().unwrap().remove("versions");
        json!([d])
    };
    let mut mo = SyncMeta::default();
    stamp(&docs_ws(plain("kept"), &mo), &mut mo, 100);
    let mut old = docs_ws(plain("from an older app"), &mo);
    stamp(&old.clone(), &mut mo, 500);
    assert!(mo.tombs.is_empty());
    old["extra"]["sync"] = serde_json::to_value(&mo).unwrap();
    merge(&mut team, &[old]);
    assert_eq!(team["docs"][0]["body"], "from an older app");
    assert_eq!(bodies(&team), vec!["kept"]);
    let m = meta_of(&team);
    assert!(!m.tombs.contains_key(&11));
}

/// A version taken out (past the number or days kept) stays out.
#[test]
fn a_dropped_version_stays_dropped() {
    let mut ma = SyncMeta::default();
    let both = json!([version(11, 1, 100, "old"), version(12, 1, 200, "new")]);
    let mut a = docs_ws(json!([doc("new", both)]), &ma);
    stamp(&a.clone(), &mut ma, 100);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    let b = a.clone();
    let mut a = docs_ws(
        json!([doc("new", json!([version(12, 1, 200, "new")]))]),
        &ma,
    );
    stamp(&a.clone(), &mut ma, 300);
    a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    merge(&mut a, &[b]);
    assert_eq!(bodies(&a), vec!["new"]);
}

/// Versions sit inside their doc: someone who may not see a private doc
/// gets none of its versions either.
#[test]
fn a_private_docs_versions_are_hidden_with_it() {
    let mut m = SyncMeta::default();
    let mut d = docs_ws(
        json!([
            {"id": 80, "owner": 1, "private": true, "title": "Secret", "body": "secret now", "created": 1, "updated": 1,
             "versions": [{"id": 801, "by": 1, "at": 5, "title": "Secret", "body": "secret before"}]},
            {"id": 90, "owner": 1, "title": "Open", "body": "open", "created": 1, "updated": 1,
             "versions": [{"id": 901, "by": 1, "at": 5, "title": "Open", "body": "open before"}]}
        ]),
        &m,
    );
    assert!(ids_within(&d["docs"][0]).contains(&801));
    let hidden = hidden_from(&d, 2, false);
    assert_eq!(hidden, vec![80]);
    stamp(&d.clone(), &mut m, 10);
    d["extra"]["sync"] = serde_json::to_value(&m).unwrap();
    hide(&mut d, &hidden);
    let text = d["docs"].to_string();
    assert!(!text.contains("secret") && text.contains("open before"));
    let m = meta_of(&d);
    assert!(!m.stamps.contains_key(&801) && m.stamps.contains_key(&901));
}
