// One person's own reading state and notification choices, kept in step
// between that person's devices (a Mac and an iPhone) through the team
// server, and never shown to anyone else.
//
// Each app turns what it has into a small "view" (`MineView`): where it has
// read up to in each conversation, its Inbox rows moved to Later or Cleared,
// replies marked read, and its notification settings. `collect` compares the
// view with the last state both sides agreed on (`known`) and stamps what
// changed here since; `merge` puts two such states together; `resolve` turns
// one back into a view for the app to take in. The server keeps the merged
// state of each member and sends it to their other devices (`mine` messages).
//
// How two changes are put together:
// - read positions (`read`, per conversation): the newest change wins, so
//   "Mark unread" (a lower position) holds; at the same moment the later
//   position;
// - `replies_seen`: the later one;
// - Inbox rows (`inbox`) and replies marked read (`replies_read`): row by row,
//   the newest change wins, so clearing one row on the Mac and another on the
//   iPhone keeps both;
// - settings (`set`, each whole): the newest change wins.
// A tie goes to the side that already had it (the server's copy on the
// server, the server's copy in the app), so the first device to bring a
// value keeps it. What a device has before it ever agreed with the server
// (`known` empty: just updated, or just joined) is stamped 0, so it never
// undoes a change made on another device since. Parts a newer app adds are
// passed along untouched.
//
// The iPhone app has the same in Swift (WorkboardKit/Mine.swift).

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The settings kept in step, each as one value (`set`).
pub const MINE_SETTINGS: &[&str] = &["notify", "muted_until"];

/// Fields of `notify` that stay on each device: the clock notices it has
/// already shown (each device shows its own) and those it handed to the
/// system to show later.
pub const NOTIFY_LOCAL: &[&str] = &["fired", "scheduled"];

/// What one device has, in the shape both apps build.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MineView {
    /// conversation id -> where I've read up to (message time, seconds)
    pub read: BTreeMap<u64, i64>,
    pub replies_seen: i64,
    pub inbox_cleared: BTreeSet<String>,
    pub inbox_later: BTreeSet<String>,
    pub replies_read: BTreeSet<String>,
    /// setting -> value; a setting this device doesn't have is left out
    pub settings: BTreeMap<String, Value>,
}

impl MineView {
    /// The view of a whole workspace file (`data.json`, the iPhone's copy).
    pub fn of_data(data: &Value) -> MineView {
        let mut v = MineView::default();
        for c in data
            .get("channels")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
        {
            let id = c.get("id").and_then(|i| i.as_u64());
            let read = c.get("last_read").and_then(|r| r.as_i64()).unwrap_or(0);
            if let (Some(id), true) = (id, read > 0) {
                v.read.insert(id, read);
            }
        }
        let extra = data.get("extra");
        let list = |k: &str| -> BTreeSet<String> {
            extra
                .and_then(|e| e.get(k))
                .and_then(|l| l.as_array())
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        };
        v.inbox_cleared = list("inbox_cleared");
        v.inbox_later = list("inbox_later");
        v.replies_read = list("replies_read");
        v.replies_seen = extra
            .and_then(|e| e.get("replies_seen"))
            .and_then(|r| r.as_i64())
            .unwrap_or(0);
        for k in MINE_SETTINGS {
            if let Some(s) = extra.and_then(|e| e.get(*k)).filter(|s| !s.is_null()) {
                v.settings.insert(k.to_string(), setting_value(k, s));
            }
        }
        v
    }
}

/// A setting as it is kept in step: `notify` without what stays on each
/// device.
pub fn setting_value(key: &str, v: &Value) -> Value {
    let mut v = v.clone();
    if key == "notify" {
        if let Some(o) = v.as_object_mut() {
            for k in NOTIFY_LOCAL {
                o.remove(*k);
            }
        }
    }
    v
}

fn at(e: &Value) -> i64 {
    e.get("at").and_then(|a| a.as_i64()).unwrap_or(0)
}

fn ts(e: &Value) -> i64 {
    e.get("ts").and_then(|a| a.as_i64()).unwrap_or(0)
}

/// The parts this version knows; others (from a newer app) pass along.
const PARTS: &[&str] = &["read", "replies_seen", "inbox", "replies_read", "set"];

/// Nothing agreed with the server yet.
fn fresh(known: &Value) -> bool {
    known.as_object().is_none_or(|o| o.is_empty())
}

fn obj<'a>(v: &'a Value, k: &str) -> Option<&'a Map<String, Value>> {
    v.get(k).and_then(|o| o.as_object())
}

/// An Inbox row's place in a view: "cleared", "later" or "" (where it belongs).
fn tab_in(view: &MineView, key: &str) -> &'static str {
    if view.inbox_cleared.contains(key) {
        "cleared"
    } else if view.inbox_later.contains(key) {
        "later"
    } else {
        ""
    }
}

/// The state to send: `known` (the last state agreed with the server, `{}`
/// at first) with what differs in `view` stamped `now` (milliseconds; 0
/// while nothing is agreed yet). Nothing this device lacks is taken as a
/// change.
pub fn collect(view: &MineView, known: &Value, now: i64) -> Value {
    let now = if fresh(known) { 0 } else { now };
    let mut read = obj(known, "read").cloned().unwrap_or_default();
    for (id, t) in &view.read {
        let key = id.to_string();
        if read.get(&key).map(ts) != Some(*t) {
            read.insert(key, json!({"ts": t, "at": now}));
        }
    }
    let replies_seen = known
        .get("replies_seen")
        .and_then(|r| r.as_i64())
        .unwrap_or(0)
        .max(view.replies_seen);

    // Inbox rows: every row either side knows
    let known_inbox = obj(known, "inbox").cloned().unwrap_or_default();
    let mut inbox = known_inbox.clone();
    let keys: BTreeSet<&str> = known_inbox
        .keys()
        .map(|k| k.as_str())
        .chain(view.inbox_cleared.iter().map(|k| k.as_str()))
        .chain(view.inbox_later.iter().map(|k| k.as_str()))
        .collect();
    for k in keys {
        let here = tab_in(view, k);
        let there = known_inbox
            .get(k)
            .and_then(|e| e.get("tab"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        if here != there {
            inbox.insert(k.to_string(), json!({"tab": here, "at": now}));
        }
    }

    let known_rr = obj(known, "replies_read").cloned().unwrap_or_default();
    let mut replies_read = known_rr.clone();
    let keys: BTreeSet<&str> = known_rr
        .keys()
        .map(|k| k.as_str())
        .chain(view.replies_read.iter().map(|k| k.as_str()))
        .collect();
    for k in keys {
        let here = view.replies_read.contains(k);
        let there = known_rr
            .get(k)
            .and_then(|e| e.get("on"))
            .and_then(|o| o.as_bool())
            .unwrap_or(false);
        if here != there {
            replies_read.insert(k.to_string(), json!({"on": here, "at": now}));
        }
    }

    let mut set = obj(known, "set").cloned().unwrap_or_default();
    for (k, v) in &view.settings {
        if set.get(k).and_then(|e| e.get("value")) != Some(v) {
            set.insert(k.clone(), json!({"value": v, "at": now}));
        }
    }

    let mut out = known.as_object().cloned().unwrap_or_default();
    out.insert("read".into(), Value::Object(read));
    out.insert("replies_seen".into(), json!(replies_seen));
    out.insert("inbox".into(), Value::Object(inbox));
    out.insert("replies_read".into(), Value::Object(replies_read));
    out.insert("set".into(), Value::Object(set));
    Value::Object(out)
}

/// The part of `state` that `base` doesn't have: the read positions, rows
/// and settings whose entry differs, `replies_seen` when it differs, and
/// parts from a newer app that differ. Merging it into anything is the
/// same as merging all of `state`, as far as `base` goes there already (a
/// missing entry is never taken as a change), so a device sends only this
/// instead of its whole state each time something changed.
pub fn changes(state: &Value, base: &Value) -> Value {
    let mut out = Map::new();
    for (k, v) in state.as_object().into_iter().flatten() {
        let had = base.get(k);
        match (v.as_object(), had.and_then(|h| h.as_object())) {
            (Some(entries), Some(old)) if k != "replies_seen" => {
                let diff: Map<String, Value> = entries
                    .iter()
                    .filter(|(e, val)| old.get(*e) != Some(*val))
                    .map(|(e, val)| (e.clone(), val.clone()))
                    .collect();
                if !diff.is_empty() {
                    out.insert(k.clone(), Value::Object(diff));
                }
            }
            _ => {
                if had != Some(v) {
                    out.insert(k.clone(), v.clone());
                }
            }
        }
    }
    Value::Object(out)
}

/// Put `other` into `base`. On a tie `base` keeps its own. True when
/// `base` changed.
pub fn merge(base: &mut Value, other: &Value) -> bool {
    let before = base.clone();
    if !base.is_object() {
        *base = json!({});
    }
    let b = base.as_object_mut().expect("made an object above");
    // read positions: the newest change, at the same moment the later one
    {
        let read = b.entry("read").or_insert_with(|| json!({})).as_object_mut();
        if let (Some(read), Some(o)) = (read, obj(other, "read")) {
            for (k, v) in o {
                if !v.is_object() {
                    continue;
                }
                let wins = read.get(k).is_none_or(|m| (at(v), ts(v)) > (at(m), ts(m)));
                if wins {
                    read.insert(k.clone(), v.clone());
                }
            }
        }
    }
    // parts from a newer app pass along
    for (k, v) in other.as_object().into_iter().flatten() {
        if !PARTS.contains(&k.as_str()) && !b.contains_key(k) {
            b.insert(k.clone(), v.clone());
        }
    }
    let seen = b
        .get("replies_seen")
        .and_then(|r| r.as_i64())
        .unwrap_or(0)
        .max(
            other
                .get("replies_seen")
                .and_then(|r| r.as_i64())
                .unwrap_or(0),
        );
    b.insert("replies_seen".into(), json!(seen));
    // the newest change per row or setting
    for part in ["inbox", "replies_read", "set"] {
        let mine = b.entry(part).or_insert_with(|| json!({})).as_object_mut();
        if let (Some(mine), Some(o)) = (mine, obj(other, part)) {
            for (k, v) in o {
                if !v.is_object() {
                    continue;
                }
                if mine.get(k).is_none_or(|m| at(v) > at(m)) {
                    mine.insert(k.clone(), v.clone());
                }
            }
        }
    }
    *base != before
}

/// The view a merged state stands for. Settings it doesn't have are left
/// out (the app keeps its own).
pub fn resolve(mine: &Value) -> MineView {
    let mut v = MineView::default();
    for (k, r) in obj(mine, "read").into_iter().flatten() {
        if let Ok(id) = k.parse::<u64>() {
            v.read.insert(id, ts(r));
        }
    }
    v.replies_seen = mine
        .get("replies_seen")
        .and_then(|r| r.as_i64())
        .unwrap_or(0);
    for (k, e) in obj(mine, "inbox").into_iter().flatten() {
        match e.get("tab").and_then(|t| t.as_str()) {
            Some("cleared") => {
                v.inbox_cleared.insert(k.clone());
            }
            Some("later") => {
                v.inbox_later.insert(k.clone());
            }
            _ => {}
        }
    }
    for (k, e) in obj(mine, "replies_read").into_iter().flatten() {
        if e.get("on").and_then(|o| o.as_bool()) == Some(true) {
            v.replies_read.insert(k.clone());
        }
    }
    for (k, e) in obj(mine, "set").into_iter().flatten() {
        if let Some(val) = e.get("value") {
            v.settings.insert(k.clone(), val.clone());
        }
    }
    v
}

/// A list in `order`, with what `want` has: the ones still in it in their
/// order, then the new ones. Used to take a resolved set into an app's list
/// without reordering it.
pub fn keep_order(order: &[String], want: &BTreeSet<String>) -> Vec<String> {
    let mut out: Vec<String> = order
        .iter()
        .filter(|k| want.contains(*k))
        .cloned()
        .collect();
    let had: BTreeSet<&String> = out.iter().collect();
    let new: Vec<String> = want.iter().filter(|k| !had.contains(k)).cloned().collect();
    out.extend(new);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> MineView {
        MineView::default()
    }

    /// A device that already agreed with the server once (its changes are
    /// stamped with their time).
    fn agreed() -> Value {
        json!({"replies_seen": 0})
    }

    #[test]
    fn a_change_here_is_stamped_and_reaches_the_other_device() {
        // the Mac clears a row and reads #general
        let mut mac = view();
        mac.inbox_cleared.insert("1:100:2".into());
        mac.read.insert(10, 500);
        let sent = collect(&mac, &agreed(), 1000);
        assert_eq!(sent["inbox"]["1:100:2"]["at"], json!(1000));
        let mut server = json!({});
        assert!(merge(&mut server, &sent));
        // the iPhone, which had nothing, takes it
        let phone = view();
        let mut mine = collect(&phone, &json!({}), 2000);
        merge(&mut mine, &server);
        let got = resolve(&mine);
        assert!(got.inbox_cleared.contains("1:100:2"));
        assert_eq!(got.read.get(&10), Some(&500));
    }

    /// Sending only what changed since the last agreed state leaves the
    /// server with what sending everything would.
    #[test]
    fn only_what_changed_merges_like_the_whole_state() {
        let mut mac = view();
        mac.inbox_cleared.insert("a".into());
        mac.inbox_later.insert("b".into());
        mac.replies_read.insert("r1".into());
        mac.read.insert(10, 500);
        mac.read.insert(11, 600);
        mac.settings.insert("muted_until".into(), json!(0));
        let known = collect(&mac, &agreed(), 1000);
        let mut server = json!({});
        merge(&mut server, &known);
        // the phone changed some of it meanwhile, later
        let mut phone = view();
        phone.read.insert(11, 650);
        phone.inbox_cleared.insert("b".into());
        merge(&mut server, &collect(&phone, &known, 1500));
        // then the Mac changes other things (and one the phone changed)
        mac.inbox_cleared.remove("a");
        mac.replies_read.insert("r2".into());
        mac.read.insert(10, 550);
        mac.settings.insert("muted_until".into(), json!(99));
        mac.replies_seen = 7;
        let state = collect(&mac, &known, 2000);
        let part = changes(&state, &known);
        assert!(part["read"].get("11").is_none(), "unchanged: {}", part);
        assert!(part["inbox"].get("b").is_none());
        assert_eq!(part["read"]["10"]["ts"], json!(550));
        assert_eq!(part["replies_seen"], json!(7));
        let (mut whole, mut partly) = (server.clone(), server.clone());
        assert!(merge(&mut whole, &state));
        assert!(merge(&mut partly, &part));
        assert_eq!(whole, partly);
        // nothing changed: nothing to send
        assert_eq!(changes(&state, &state), json!({}));
        // nothing agreed yet: everything
        assert_eq!(changes(&state, &json!({})), state);
    }

    #[test]
    fn moving_a_row_back_is_a_change_too() {
        let mut mac = view();
        mac.inbox_cleared.insert("k".into());
        let known = collect(&mac, &agreed(), 1000);
        mac.inbox_cleared.clear();
        let sent = collect(&mac, &known, 2000);
        let mut server = known.clone();
        merge(&mut server, &sent);
        assert!(resolve(&server).inbox_cleared.is_empty());
        assert_eq!(server["inbox"]["k"], json!({"tab": "", "at": 2000}));
    }

    #[test]
    fn rows_changed_on_both_devices_both_stay() {
        let mut mac = view();
        mac.inbox_cleared.insert("a".into());
        let mut phone = view();
        phone.inbox_later.insert("b".into());
        let mut server = json!({});
        merge(&mut server, &collect(&mac, &agreed(), 1000));
        merge(&mut server, &collect(&phone, &agreed(), 1001));
        let got = resolve(&server);
        assert!(got.inbox_cleared.contains("a"));
        assert!(got.inbox_later.contains("b"));
    }

    #[test]
    fn the_same_row_goes_by_the_newest_change() {
        let mut server = json!({});
        let mut a = view();
        a.inbox_cleared.insert("k".into());
        let mut b = view();
        b.inbox_later.insert("k".into());
        merge(&mut server, &collect(&b, &agreed(), 2000));
        merge(&mut server, &collect(&a, &agreed(), 1000));
        assert!(resolve(&server).inbox_later.contains("k"));
        assert!(!resolve(&server).inbox_cleared.contains("k"));
    }

    #[test]
    fn read_positions_go_by_the_newest_change_so_mark_unread_holds() {
        let mut mac = view();
        mac.read.insert(10, 900);
        let mut server = json!({});
        let known = collect(&mac, &agreed(), 1000);
        merge(&mut server, &known);
        // "Mark unread" on the Mac: a lower position, changed later
        mac.read.insert(10, 700);
        merge(&mut server, &collect(&mac, &known, 2000));
        assert_eq!(resolve(&server).read.get(&10), Some(&700));
        // the phone, which still has the old position, doesn't undo it
        let mut phone = view();
        phone.read.insert(10, 900);
        let phone_known = known.clone();
        merge(&mut server, &collect(&phone, &phone_known, 3000));
        assert_eq!(resolve(&server).read.get(&10), Some(&700));
        // at the same moment the later position wins, whichever came first
        let mut a = json!({"read": {"11": {"ts": 5, "at": 0}}});
        merge(&mut a, &json!({"read": {"11": {"ts": 8, "at": 0}}}));
        assert_eq!(resolve(&a).read.get(&11), Some(&8));
        // replies_seen: the later one
        let mut s = json!({"replies_seen": 80});
        merge(&mut s, &json!({"replies_seen": 50}));
        assert_eq!(resolve(&s).replies_seen, 80);
    }

    #[test]
    fn settings_go_by_the_newest_change_and_ties_keep_the_first() {
        let mut a = view();
        a.settings.insert("muted_until".into(), json!(0));
        let mut b = view();
        b.settings.insert("muted_until".into(), json!(99));
        let mut server = json!({});
        merge(&mut server, &collect(&a, &agreed(), 1000));
        merge(&mut server, &collect(&b, &agreed(), 2000));
        assert_eq!(resolve(&server).settings["muted_until"], json!(99));
        // the same moment: the one already there stays
        let mut tie = json!({});
        merge(&mut tie, &collect(&a, &agreed(), 1000));
        merge(&mut tie, &collect(&b, &agreed(), 1000));
        assert_eq!(resolve(&tie).settings["muted_until"], json!(0));
    }

    #[test]
    fn what_a_device_had_before_it_ever_agreed_never_undoes_a_change() {
        // the phone mutes on Tuesday
        let mut phone = view();
        phone.settings.insert("muted_until".into(), json!(99));
        let mut server = json!({});
        merge(&mut server, &collect(&phone, &agreed(), 1000));
        // the Mac is updated on Wednesday: what it has counts as old
        let mut mac = view();
        mac.settings.insert("muted_until".into(), json!(0));
        mac.inbox_cleared.insert("x".into());
        let first = collect(&mac, &json!({}), 5000);
        assert_eq!(first["set"]["muted_until"]["at"], json!(0));
        merge(&mut server, &first);
        let got = resolve(&server);
        assert_eq!(
            got.settings["muted_until"],
            json!(99),
            "the phone's change stays"
        );
        assert!(
            got.inbox_cleared.contains("x"),
            "what only the Mac had comes along"
        );
    }

    #[test]
    fn a_setting_this_device_lacks_is_not_a_change() {
        let mut mac = view();
        mac.settings
            .insert("notify".into(), json!({"direct": false}));
        let server = collect(&mac, &agreed(), 1000);
        // the iPhone never had notify: it doesn't send one
        let phone = view();
        let sent = collect(&phone, &server, 2000);
        assert_eq!(sent, server);
    }

    #[test]
    fn nothing_new_sends_the_same_state() {
        let mut v = view();
        v.read.insert(1, 10);
        v.inbox_cleared.insert("x".into());
        v.replies_read.insert("chat:1:2".into());
        v.settings.insert("notify".into(), json!({"direct": true}));
        let known = collect(&v, &agreed(), 1000);
        assert_eq!(collect(&resolve(&known), &known, 5000), known);
        assert_eq!(collect(&v, &known, 5000), known);
    }

    #[test]
    fn parts_from_a_newer_app_pass_along() {
        let newer = json!({"read": {}, "pins": {"a": 1}});
        let mut server = json!({});
        merge(&mut server, &newer);
        assert_eq!(server["pins"], json!({"a": 1}));
        // an older app sends them back as they came, so nothing goes round
        let back = collect(&resolve(&server), &server, 9000);
        assert_eq!(back, server);
        assert!(!merge(&mut server, &back));
    }

    #[test]
    fn notify_leaves_out_what_stays_on_each_device() {
        let data = json!({"extra": {"notify": {"direct": true, "fired": ["x"]}, "muted_until": 0}});
        let v = MineView::of_data(&data);
        assert_eq!(v.settings["notify"], json!({"direct": true}));
        assert_eq!(v.settings["muted_until"], json!(0));
    }

    /// The same case as testTheSameStateAsTheMac in the iPhone's
    /// MineTests.swift: both apps must give exactly this.
    #[test]
    fn the_shape_both_apps_give() {
        let mut v = view();
        v.read.insert(10, 500);
        v.replies_seen = 7;
        v.inbox_cleared.insert("a".into());
        v.inbox_later.insert("b".into());
        v.replies_read.insert("r".into());
        v.settings.insert("muted_until".into(), json!(9));
        let mut got = collect(&v, &agreed(), 1000);
        assert_eq!(
            got,
            json!({
                "read": {"10": {"ts": 500, "at": 1000}},
                "replies_seen": 7,
                "inbox": {"a": {"tab": "cleared", "at": 1000}, "b": {"tab": "later", "at": 1000}},
                "replies_read": {"r": {"on": true, "at": 1000}},
                "set": {"muted_until": {"value": 9, "at": 1000}},
            })
        );
        merge(
            &mut got,
            &json!({"inbox": {"a": {"tab": "", "at": 2000}}, "read": {"10": {"ts": 400, "at": 900}, "11": {"ts": 3, "at": 900}}}),
        );
        let r = resolve(&got);
        assert!(r.inbox_cleared.is_empty());
        assert_eq!(r.inbox_later.iter().collect::<Vec<_>>(), ["b"]);
        assert_eq!(r.read.get(&10), Some(&500));
        assert_eq!(r.read.get(&11), Some(&3));
        // nothing agreed yet: stamped 0
        assert_eq!(
            collect(&v, &json!({}), 1000)["set"]["muted_until"]["at"],
            json!(0)
        );
    }

    #[test]
    fn keep_order_keeps_the_list_order() {
        let want: BTreeSet<String> = ["b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let order: Vec<String> = ["c", "a", "b"].iter().map(|s| s.to_string()).collect();
        assert_eq!(keep_order(&order, &want), vec!["c", "b", "d"]);
    }
}
