// Ids worked out from contents, for activity lines and whiteboard items
// saved before they had ids and for goal targets and history lines, so
// every copy gives them the same one.

use super::*;

/// The id a task comment or activity line gets when it was written before
/// activity had ids. It is worked out from what the entry says, so every
/// computer gives the same old entry the same id and copies still match up.
/// `nth` counts earlier entries in the same task that read exactly the same.
/// Ids made this way have bit 62 set: far above the ids `new_id` makes from
/// the clock, and still a positive 64-bit signed number.
pub fn activity_id(task: u64, ts: i64, author: u64, comment: bool, text: &str, nth: usize) -> u64 {
    let h = fnv(format!("{}|{}|{}|{}|{}|{}", task, ts, author, comment, nth, text).as_bytes());
    (h & ((1u64 << 62) - 1)) | (1u64 << 62)
}

/// Gives every task activity entry without an id one (see `activity_id`),
/// and drops repeats of the same id. True when anything changed.
pub fn fill_activity_ids(data: &mut Value) -> bool {
    let mut changed = false;
    let Some(tasks) = data.get_mut("tasks").and_then(|t| t.as_array_mut()) else {
        return false;
    };
    for task in tasks {
        let tid = id_of(task).unwrap_or(0);
        let Some(acts) = task.get_mut("activity").and_then(|a| a.as_array_mut()) else {
            continue;
        };
        if acts.iter().all(|a| id_of(a).is_some_and(|i| i != 0)) {
            let mut seen = std::collections::HashSet::new();
            if acts.iter().all(|a| seen.insert(id_of(a))) {
                continue;
            }
        }
        let mut counts: HashMap<(i64, u64, bool, String), usize> = HashMap::new();
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::with_capacity(acts.len());
        for mut a in std::mem::take(acts) {
            let Some(obj) = a.as_object_mut() else {
                continue;
            };
            let ts = obj.get("ts").and_then(|v| v.as_i64()).unwrap_or(0);
            let author = obj.get("author").and_then(|v| v.as_u64()).unwrap_or(0);
            let comment = obj
                .get("comment")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let text = obj
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let n = counts
                .entry((ts, author, comment, text.clone()))
                .or_insert(0);
            let nth = *n;
            *n += 1;
            let id = match obj.get("id").and_then(|v| v.as_u64()) {
                Some(i) if i != 0 => i,
                _ => {
                    let i = activity_id(tid, ts, author, comment, &text, nth);
                    obj.insert("id".into(), Value::from(i));
                    changed = true;
                    i
                }
            };
            if seen.insert(id) {
                out.push(a);
            } else {
                changed = true;
            }
        }
        *acts = out;
    }
    changed
}

/// The id a whiteboard stroke or sticky note gets when it was drawn before
/// they had ids, worked out from what it looks like (rounded, so every app
/// reads the same numbers) so every computer gives it the same id. `nth`
/// counts earlier entries on the board that look the same. Bit 62 set, as
/// in `activity_id`.
pub fn board_item_id(board: u64, kind: &str, sig: &str, nth: usize) -> u64 {
    let h = fnv(format!("wb|{}|{}|{}|{}", board, kind, nth, sig).as_bytes());
    (h & ((1u64 << 62) - 1)) | (1u64 << 62)
}

pub(crate) fn board_sig(kind: &str, item: &Value) -> String {
    let r10 = |v: Option<&Value>| (v.and_then(|x| x.as_f64()).unwrap_or(0.0) * 10.0).round() as i64;
    let color: Vec<String> = item
        .get("color")
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .map(|c| c.as_u64().unwrap_or(0).to_string())
        .collect();
    if kind == "strokes" {
        let pts: Vec<String> = item
            .get("points")
            .and_then(|p| p.as_array())
            .into_iter()
            .flatten()
            .map(|p| format!("{},{}", r10(p.get(0)), r10(p.get(1))))
            .collect();
        format!("{}|{}", color.join(","), pts.join(";"))
    } else {
        let pos = item.get("pos");
        format!(
            "{}|{}|{}|{}",
            color.join(","),
            item.get("text").and_then(|t| t.as_str()).unwrap_or(""),
            r10(pos.and_then(|p| p.get(0))),
            r10(pos.and_then(|p| p.get(1)))
        )
    }
}

/// Gives every whiteboard stroke and sticky note without an id one (see
/// `board_item_id`), so they merge one by one: strokes two people draw at
/// once are both kept and an erased one stays erased. True when anything
/// changed.
pub fn fill_board_ids(data: &mut Value) -> bool {
    let mut changed = false;
    let Some(boards) = get_path_mut(data, "extra.whiteboards").and_then(|b| b.as_array_mut())
    else {
        return false;
    };
    for b in boards {
        let bid = id_of(b).unwrap_or(0);
        for kind in ["strokes", "notes"] {
            let Some(items) = b.get_mut(kind).and_then(|a| a.as_array_mut()) else {
                continue;
            };
            let mut counts: HashMap<String, usize> = HashMap::new();
            for it in items.iter_mut() {
                if !it.is_object() || id_of(it).is_some_and(|i| i != 0) {
                    continue;
                }
                let sig = board_sig(kind, it);
                let n = counts.entry(sig.clone()).or_insert(0);
                it["id"] = Value::from(board_item_id(bid, kind, &sig, *n));
                *n += 1;
                changed = true;
            }
        }
    }
    changed
}

/// The id a goal's target or history line gets when it has none (saved
/// before they had ids, made by an app that doesn't keep them, or just
/// added): worked out from what it is, so every computer gives it the same
/// one and they merge one by one, like whiteboard strokes. `kind` is
/// "targets" or "log"; `nth` counts on past ids already taken (see
/// `fill_goal_ids`). Bit 62 set, as in `activity_id`.
pub fn goal_item_id(goal: u64, kind: &str, sig: &str, nth: usize) -> u64 {
    let h = fnv(format!("goal|{}|{}|{}|{}", goal, kind, nth, sig).as_bytes());
    (h & ((1u64 << 62) - 1)) | (1u64 << 62)
}

/// What a goal's target or history line is, for `goal_item_id`: a target by
/// its name and kind ("Signups|Number"), which stay put while its numbers
/// move; a history line by when, who, the progress after and what happened
/// ("1763000000|2|50|Milestone:50"), which never change.
pub fn goal_sig(kind: &str, item: &Value) -> String {
    let variant = |v: Option<&Value>| -> String {
        match v {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Object(o)) => match o.iter().next() {
                Some((k, Value::Number(n))) => format!("{}:{}", k, n),
                Some((k, _)) => k.clone(),
                None => String::new(),
            },
            _ => String::new(),
        }
    };
    if kind == "targets" {
        format!(
            "{}|{}",
            item.get("name").and_then(|n| n.as_str()).unwrap_or(""),
            variant(item.get("kind"))
        )
    } else {
        let n = |k: &str| item.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
        format!(
            "{}|{}|{}|{}",
            n("ts"),
            n("who"),
            n("pct"),
            variant(item.get("what"))
        )
    }
}

/// Gives every goal target and history line without an id one (see
/// `goal_item_id`), so two people changing different targets of a goal at
/// once both keep their change and both history lines stay. An id already
/// in the list, or deleted (a tombstone), is passed over for the next one,
/// so a target added again after one like it was removed isn't taken for
/// the removed one. True when anything changed.
pub fn fill_goal_ids(data: &mut Value) -> bool {
    let tombs: std::collections::HashSet<u64> = get_path(data, "extra.sync.tombs")
        .and_then(|t| t.as_object())
        .map(|o| o.keys().filter_map(|k| k.parse().ok()).collect())
        .unwrap_or_default();
    let Some(goals) = get_path_mut(data, "extra.goals").and_then(|g| g.as_array_mut()) else {
        return false;
    };
    let mut changed = false;
    for g in goals {
        let gid = id_of(g).unwrap_or(0);
        for kind in ["targets", "log"] {
            let Some(items) = g.get_mut(kind).and_then(|a| a.as_array_mut()) else {
                continue;
            };
            let missing = |it: &Value| it.is_object() && !id_of(it).is_some_and(|i| i != 0);
            if !items.iter().any(missing) {
                continue;
            }
            let mut used: std::collections::HashSet<u64> =
                items.iter().filter_map(id_of).filter(|i| *i != 0).collect();
            let mut counts: HashMap<String, usize> = HashMap::new();
            for it in items.iter_mut() {
                if !missing(it) {
                    continue;
                }
                let sig = goal_sig(kind, it);
                let n = counts.entry(sig.clone()).or_insert(0);
                let id = loop {
                    let id = goal_item_id(gid, kind, &sig, *n);
                    *n += 1;
                    if !used.contains(&id) && !tombs.contains(&id) {
                        break id;
                    }
                };
                used.insert(id);
                it["id"] = Value::from(id);
                changed = true;
            }
        }
    }
    changed
}
