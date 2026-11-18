// Fingerprints: of each shared item, of each part of an item (the fields
// that merge on their own) and the JSON path helpers they read with. The
// iPhone app works out the same values (tests/golden).

use super::*;

/// Leave this computer's fingerprints out of a workspace about to be saved
/// (`data`, with its bookkeeping at "extra.sync") when they are exactly what
/// its content gives, and mark it to have them worked out again on opening.
/// They are about half of a synced workspace's file. Fingerprints that
/// differ from the content (a change not recorded yet, or made the way an
/// older version made them) say something the content can't, so they stay.
pub fn leave_out_hashes(data: &mut Value) {
    let Some(meta) = data.pointer("/extra/sync").cloned() else {
        return;
    };
    let meta: SyncMeta = serde_json::from_value(meta).unwrap_or_default();
    // no fingerprints is itself news: nothing recorded yet
    if meta.hashes.is_empty()
        || meta.hashes != fingerprints(data)
        || meta.field_hashes != part_hashes(data)
        || meta.parents != parents_of(data)
        || meta.value_hashes != value_hashes(data)
    {
        return;
    }
    if let Some(s) = data
        .pointer_mut("/extra/sync")
        .and_then(|s| s.as_object_mut())
    {
        for k in ["hashes", "field_hashes", "parents", "value_hashes"] {
            s.remove(k);
        }
        s.insert("rehash".into(), Value::Bool(true));
    }
}

/// Opening a file saved by `leave_out_hashes`: its fingerprints again, from
/// its content as it was saved (before anything else changes it).
pub fn refill_hashes(meta: &mut SyncMeta, data: &Value) {
    if !meta.rehash {
        return;
    }
    meta.hashes = fingerprints(data);
    meta.field_hashes = part_hashes(data);
    meta.parents = parents_of(data);
    meta.value_hashes = value_hashes(data);
    meta.rehash = false;
}

pub fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// A random-enough number without extra dependencies.
pub fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.finish()
}

/// A non-empty array of objects of which at least one has an id: its
/// entries with ids are items, merged one by one. Entries without one (a
/// stroke drawn before it got its id) don't make the others any less items;
/// they stay as they are (`loose_entries`), part of the item the array sits
/// in.
pub(crate) fn is_id_array(v: &Value) -> bool {
    match v.as_array() {
        Some(a) => {
            !a.is_empty() && a.iter().all(|x| x.is_object()) && a.iter().any(|x| id_of(x).is_some())
        }
        None => false,
    }
}

/// The entries without an id in an array of items (see `is_id_array`).
pub(crate) fn loose_entries(v: &Value) -> Vec<Value> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter(|x| id_of(x).is_none())
        .cloned()
        .collect()
}

/// Fingerprint of an item's own fields (not the items inside it, not personal fields).
pub(crate) fn own_hash(obj: &Map<String, Value>) -> u64 {
    let mut own = Map::new();
    for (k, v) in obj {
        if LOCAL_KEYS.contains(&k.as_str()) {
            continue;
        }
        if v.as_array().map(|a| a.is_empty()).unwrap_or(false) {
            continue;
        }
        if is_id_array(v) {
            // entries without an id are the item's own
            let loose = loose_entries(v);
            if !loose.is_empty() {
                own.insert(k.clone(), Value::Array(loose));
            }
            continue;
        }
        own.insert(k.clone(), v.clone());
    }
    fnv(serde_json::to_string(&Value::Object(own))
        .unwrap_or_default()
        .as_bytes())
}

/// The fields of a task that together hold its running timers: the one
/// older apps read (`timer_started`, with `timer_by`) and everyone else's
/// (`timers`). They merge as one set of timers, one per person.
pub(crate) const TIMER_KEYS: &[&str] = &["timer_started", "timer_by", "timers"];

pub(crate) fn is_empty_arr(v: &Value) -> bool {
    v.as_array().is_some_and(|a| a.is_empty())
}

/// The parts of a `time_entries` field: one per entry, named by whose it is
/// and when it started ("time_entries/<member>/<start>", with "/<n>" for
/// the n-th repeat), so two people stopping their timers at once both keep
/// their entry and a deleted one stays deleted. None when it isn't a list
/// of entries.
pub(crate) fn entry_parts(v: &Value) -> Option<Vec<(String, Value)>> {
    let mut out = vec![];
    let mut seen: HashMap<(u64, i64), usize> = HashMap::new();
    for e in v.as_array()? {
        let start = e.as_object()?.get("start")?.as_i64()?;
        let member = e.get("member").and_then(|m| m.as_u64()).unwrap_or(0);
        let n = seen.entry((member, start)).or_insert(0);
        let name = match *n {
            0 => format!("time_entries/{}/{}", member, start),
            k => format!("time_entries/{}/{}/{}", member, start, k),
        };
        *n += 1;
        out.push((name, e.clone()));
    }
    Some(out)
}

/// The (emoji, member) pairs of a `reactions` field, in order. None when it
/// isn't a list of `{emoji, users}`.
pub(crate) fn reaction_pairs(v: &Value) -> Option<Vec<(String, u64)>> {
    let mut out = vec![];
    for r in v.as_array()? {
        let emoji = r.get("emoji")?.as_str()?;
        for u in r.get("users")?.as_array()? {
            let u = u.as_u64()?;
            if !out.iter().any(|(e, x)| e == emoji && *x == u) {
                out.push((emoji.to_string(), u));
            }
        }
    }
    Some(out)
}

/// The running timers of a task as (member, start), member 0 for an old
/// timer that didn't record who started it; whether each came from
/// `timers` (true) or the field older apps read (false).
pub(crate) fn timer_list(obj: &Map<String, Value>) -> Vec<(u64, i64, bool)> {
    let mut out: Vec<(u64, i64, bool)> = vec![];
    if let Some(st) = obj.get("timer_started").and_then(|t| t.as_i64()) {
        let by = obj.get("timer_by").and_then(|b| b.as_u64()).unwrap_or(0);
        out.push((by, st, false));
    }
    for r in obj
        .get("timers")
        .and_then(|t| t.as_array())
        .into_iter()
        .flatten()
    {
        if let (Some(m), Some(st)) = (
            r.get("member").and_then(|m| m.as_u64()),
            r.get("start").and_then(|s| s.as_i64()),
        ) {
            if !out.iter().any(|x| x.0 == m) {
                out.push((m, st, true));
            }
        }
    }
    out
}

/// The parts of an item's own fields with their values (see
/// `SyncMeta::fields`): every field except personal ones, ever-growing
/// ones and the items inside it, with reactions (per emoji and person),
/// time entries (per person and start), custom field values (per field)
/// and running timers (per person) split into one part per element, so
/// each merges on its own. An empty list counts as no field. For a list of items, its
/// entries without an id are a part named after the field.
pub(crate) fn parts_of(obj: &Map<String, Value>) -> Vec<(String, Value)> {
    let mut out = vec![];
    for (k, v) in obj {
        let k = k.as_str();
        if LOCAL_KEYS.contains(&k)
            || APPEND_KEYS.contains(&k)
            || TIMER_KEYS.contains(&k)
            || is_empty_arr(v)
        {
            continue;
        }
        if is_id_array(v) {
            let loose = loose_entries(v);
            if !loose.is_empty() {
                out.push((k.to_string(), Value::Array(loose)));
            }
            continue;
        }
        if k == "reactions" {
            if let Some(pairs) = reaction_pairs(v) {
                for (e, u) in pairs {
                    out.push((format!("reactions/{}/{}", e, u), Value::Bool(true)));
                }
                continue;
            }
        }
        if k == "time_entries" {
            if let Some(parts) = entry_parts(v) {
                out.extend(parts);
                continue;
            }
        }
        if k == "custom" {
            if let Some(o) = v.as_object() {
                for (key, x) in o {
                    out.push((format!("custom/{}", key), x.clone()));
                }
                continue;
            }
        }
        out.push((k.to_string(), v.clone()));
    }
    for (m, st, _) in timer_list(obj) {
        out.push((
            format!("timers/{}", m),
            serde_json::json!({"member": m, "start": st}),
        ));
    }
    out
}

/// The field a part belongs to ("reactions/👍/3" -> "reactions").
pub(crate) fn part_field(part: &str) -> &str {
    part.split_once('/').map_or(part, |(f, _)| f)
}

pub(crate) fn part_hash(v: &Value) -> u32 {
    fnv(serde_json::to_string(v).unwrap_or_default().as_bytes()) as u32
}

/// Every shared item: its id, the item it sits in, its fields.
pub(crate) fn visit<'a>(
    data: &'a Value,
    f: &mut dyn FnMut(u64, Option<u64>, &'a Map<String, Value>),
) {
    fn go<'a>(
        v: &'a Value,
        parent: Option<u64>,
        f: &mut dyn FnMut(u64, Option<u64>, &'a Map<String, Value>),
    ) {
        for item in v.as_array().into_iter().flatten() {
            if let (Some(id), Some(obj)) = (id_of(item), item.as_object()) {
                f(id, parent, obj);
                for (_, child) in obj {
                    if is_id_array(child) {
                        go(child, Some(id), f);
                    }
                }
            }
        }
    }
    for path in SHARED {
        if let Some(v) = get_path(data, path) {
            go(v, None, f);
        }
    }
}

/// Fingerprints of every part of every shared item (`SyncMeta::field_hashes`).
pub fn part_hashes(data: &Value) -> BTreeMap<u64, BTreeMap<String, u32>> {
    let mut out = BTreeMap::new();
    visit(data, &mut |id, _, obj| {
        out.insert(
            id,
            parts_of(obj)
                .into_iter()
                .map(|(p, v)| (p, part_hash(&v)))
                .collect(),
        );
    });
    out
}

/// `parent` is the item this array sits in (None at the top). With `placed`,
/// an inner item's fingerprint includes its parent, so moving a list to
/// another space or folder counts as a change to the list.
pub(crate) fn walk(v: &Value, parent: Option<u64>, placed: bool, out: &mut BTreeMap<u64, u64>) {
    if let Some(a) = v.as_array() {
        for item in a {
            if let (Some(id), Some(obj)) = (id_of(item), item.as_object()) {
                let h = own_hash(obj);
                let h = match parent {
                    Some(p) if placed => fnv(format!("{}/{}", h, p).as_bytes()),
                    _ => h,
                };
                out.insert(id, h);
                for (_, child) in obj {
                    if is_id_array(child) {
                        walk(child, Some(id), placed, out);
                    }
                }
            }
        }
    }
}

pub(crate) fn fingerprints_with(data: &Value, placed: bool) -> BTreeMap<u64, u64> {
    let mut out = BTreeMap::new();
    for path in SHARED {
        if let Some(v) = get_path(data, path) {
            walk(v, None, placed, &mut out);
        }
    }
    out
}

/// Fingerprints of every shared item in a workspace.
pub fn fingerprints(data: &Value) -> BTreeMap<u64, u64> {
    fingerprints_with(data, true)
}

/// id -> the item it sits in, for every shared item inside another one.
pub fn parents_of(data: &Value) -> BTreeMap<u64, u64> {
    fn go(v: &Value, parent: Option<u64>, out: &mut BTreeMap<u64, u64>) {
        for item in v.as_array().into_iter().flatten() {
            if let (Some(id), Some(obj)) = (id_of(item), item.as_object()) {
                if let Some(p) = parent {
                    out.insert(id, p);
                }
                for (_, child) in obj {
                    if is_id_array(child) {
                        go(child, Some(id), out);
                    }
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for path in SHARED {
        if let Some(v) = get_path(data, path) {
            go(v, None, &mut out);
        }
    }
    out
}
