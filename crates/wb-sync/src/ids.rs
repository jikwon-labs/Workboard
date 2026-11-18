// Ids worked out from contents, for activity lines and whiteboard items
// saved before they had ids, so every copy gives them the same one.

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
