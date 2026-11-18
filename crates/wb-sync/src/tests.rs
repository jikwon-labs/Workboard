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
        assert_eq!(META_V, 4);
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
