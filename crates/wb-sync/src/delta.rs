// What changed since a summary: the part of a copy to send to the server.

use super::*;

/// What one side of a server connection knows the other has of the shared
/// workspace: each item's fingerprint and bookkeeping, deletes, restores
/// and shared settings. `delta` sends only what differs from it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summary {
    hashes: BTreeMap<u64, u64>,
    stamps: BTreeMap<u64, i64>,
    fields: BTreeMap<u64, BTreeMap<String, i64>>,
    moved: BTreeMap<u64, i64>,
    tombs: BTreeMap<u64, i64>,
    revived: BTreeMap<u64, i64>,
    values: BTreeMap<String, i64>,
    value_hashes: BTreeMap<String, u64>,
}

impl Summary {
    /// Everything in a whole copy.
    pub fn of(data: &Value) -> Summary {
        let m = stamps_of(data);
        Summary {
            hashes: fingerprints(data),
            stamps: m.stamps,
            fields: m.fields,
            moved: m.moved,
            tombs: m.tombs,
            revived: m.revived,
            values: m.values,
            value_hashes: value_hashes(data),
        }
    }

    /// The other side has this copy (a whole one or a `delta`) too now.
    pub fn absorb(&mut self, part: &Value) {
        let m = stamps_of(part);
        for (id, h) in fingerprints(part) {
            self.hashes.insert(id, h);
            let set = |map: &mut BTreeMap<u64, i64>, from: &BTreeMap<u64, i64>| match from.get(&id)
            {
                Some(t) => {
                    map.insert(id, *t);
                }
                None => {
                    map.remove(&id);
                }
            };
            set(&mut self.stamps, &m.stamps);
            set(&mut self.moved, &m.moved);
            match m.fields.get(&id) {
                Some(e) => {
                    self.fields.insert(id, e.clone());
                }
                None => {
                    self.fields.remove(&id);
                }
            }
        }
        for (k, v) in &m.tombs {
            let e = self.tombs.entry(*k).or_insert(*v);
            *e = (*e).max(*v);
        }
        for (k, v) in &m.revived {
            let e = self.revived.entry(*k).or_insert(*v);
            *e = (*e).max(*v);
        }
        let vh = value_hashes(part);
        for (path, h) in &vh {
            // a shared map's entries are there when they have a fingerprint
            if map_entry(path).is_some() || get_path(part, path).is_some() {
                self.value_hashes.insert(path.clone(), *h);
                if let Some(t) = m.values.get(path) {
                    self.values.insert(path.clone(), *t);
                }
            }
        }
        // shared map entries it took away
        for (p, t) in &m.values {
            if map_entry(p).is_some() && !vh.contains_key(p) {
                self.value_hashes.remove(p);
                self.values.insert(p.clone(), *t);
            }
        }
    }
}

/// What a `delta` carries, by name: the items that changed, the deletes
/// and restores, and the shared settings and map entries ("<path>/<key>").
/// Those of several deltas added together (`add`) say what changed over
/// all of them: `delta_of` makes one part with all of it, as it is now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    ids: std::collections::HashSet<u64>,
    tombs: std::collections::BTreeSet<u64>,
    revived: std::collections::BTreeSet<u64>,
    values: std::collections::BTreeSet<String>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
            && self.tombs.is_empty()
            && self.revived.is_empty()
            && self.values.is_empty()
    }

    /// How many things it names (for keeping a list of them small).
    pub fn len(&self) -> usize {
        self.ids.len() + self.tombs.len() + self.revived.len() + self.values.len()
    }

    /// What `other` changed too.
    pub fn add(&mut self, other: &Changes) {
        self.ids.extend(other.ids.iter().copied());
        self.tombs.extend(other.tombs.iter().copied());
        self.revived.extend(other.revived.iter().copied());
        self.values.extend(other.values.iter().cloned());
    }
}

/// The part of `data` (a whole copy) that differs from what `since` says
/// the other side has: the items that changed, each inside the items it
/// sits in (with their own fields but only the changed items inside them),
/// with their bookkeeping, plus new deletes and restores and changed
/// shared settings. Merging it is the same as merging the whole copy,
/// since a missing item is never taken as deleted. None when nothing
/// changed.
pub fn delta(since: &Summary, data: &Value) -> Option<Value> {
    let cur = fingerprints(data);
    let meta = stamps_of(data);
    let vh = value_hashes(data);
    let changes = diff(since, &cur, &meta, &vh);
    build(data, &meta, &changes)
}

/// `delta`, and `since` becomes what `data` holds (`Summary::of`), from
/// the same fingerprints and bookkeeping instead of working them out a
/// second time. Also says what the delta carries (`Changes`).
pub fn delta_advance(since: &mut Summary, data: &Value) -> (Option<Value>, Changes) {
    let cur = fingerprints(data);
    let m = stamps_of(data);
    let vh = value_hashes(data);
    let changes = diff(since, &cur, &m, &vh);
    let part = build(data, &m, &changes);
    *since = Summary {
        hashes: cur,
        stamps: m.stamps,
        fields: m.fields,
        moved: m.moved,
        tombs: m.tombs,
        revived: m.revived,
        values: m.values,
        value_hashes: vh,
    };
    (part, changes)
}

/// One part with everything `changes` names, as `data` (a whole copy) has
/// it now: what the deltas the changes came from bring, taken together.
/// Merging it is the same as merging those deltas one after the other,
/// since each brought its items as they were then and the later ones as
/// they are now. None when it names nothing that is there.
pub fn delta_of(data: &Value, changes: &Changes) -> Option<Value> {
    build(data, &stamps_of(data), changes)
}

/// What differs between `since` and a copy with fingerprints `cur`,
/// bookkeeping `meta` and shared setting fingerprints `vh`.
fn diff(
    since: &Summary,
    cur: &BTreeMap<u64, u64>,
    meta: &SyncMeta,
    vh: &BTreeMap<String, u64>,
) -> Changes {
    let ids: std::collections::HashSet<u64> = cur
        .iter()
        .filter(|(id, h)| {
            since.hashes.get(id) != Some(h)
                || since.stamps.get(id) != meta.stamps.get(id)
                || since.fields.get(id) != meta.fields.get(id)
                || since.moved.get(id) != meta.moved.get(id)
        })
        .map(|(id, _)| *id)
        .collect();
    let newer =
        |a: &BTreeMap<u64, i64>, b: &BTreeMap<u64, i64>| -> std::collections::BTreeSet<u64> {
            a.iter()
                .filter(|(k, v)| b.get(k).is_none_or(|o| o < v))
                .map(|(k, _)| *k)
                .collect()
        };
    let mut values: std::collections::BTreeSet<String> = SHARED_VALUES
        .iter()
        .filter(|p| {
            since.value_hashes.get(**p) != vh.get(**p)
                || since.values.get(**p) != meta.values.get(**p)
        })
        .map(|p| p.to_string())
        .collect();
    // shared map entries set or taken away since
    values.extend(
        vh.keys()
            .chain(meta.values.keys())
            .filter(|p| map_entry(p).is_some())
            .filter(|p| {
                since.value_hashes.get(*p) != vh.get(*p)
                    || since.values.get(*p) != meta.values.get(*p)
            })
            .cloned(),
    );
    Changes {
        ids,
        tombs: newer(&meta.tombs, &since.tombs),
        revived: newer(&meta.revived, &since.revived),
        values,
    }
}

/// The part of `data` (bookkeeping `meta`) with what `changes` names.
fn build(data: &Value, meta: &SyncMeta, changes: &Changes) -> Option<Value> {
    let pick_at =
        |m: &BTreeMap<u64, i64>, keys: &std::collections::BTreeSet<u64>| -> BTreeMap<u64, i64> {
            keys.iter()
                .filter_map(|k| m.get(k).map(|v| (*k, *v)))
                .collect()
        };
    let changed = &changes.ids;
    let tombs = pick_at(&meta.tombs, &changes.tombs);
    let revived = pick_at(&meta.revived, &changes.revived);
    let values: Vec<&str> = SHARED_VALUES
        .iter()
        .copied()
        .filter(|p| changes.values.contains(*p))
        .collect();
    let entries: std::collections::BTreeSet<&str> = changes
        .values
        .iter()
        .filter(|p| map_entry(p).is_some())
        .map(|p| p.as_str())
        .collect();
    if changed.is_empty()
        && tombs.is_empty()
        && revived.is_empty()
        && values.is_empty()
        && entries.is_empty()
    {
        return None;
    }
    // the changed items with the items they sit in
    fn prune(
        arr: &[Value],
        changed: &std::collections::HashSet<u64>,
        kept: &mut Vec<u64>,
    ) -> Vec<Value> {
        let mut out = vec![];
        for item in arr {
            let (Some(id), Some(obj)) = (id_of(item), item.as_object()) else {
                continue;
            };
            // the items inside first: the item's own fields are copied
            // only when it goes in
            let mut subs: Vec<(&String, Vec<Value>)> = vec![];
            let mut inner = false;
            for (k, v) in obj {
                if is_id_array(v) {
                    let sub = prune(v.as_array().unwrap(), changed, kept);
                    inner |= !sub.is_empty();
                    subs.push((k, sub));
                }
            }
            if !(inner || changed.contains(&id)) {
                continue;
            }
            let mut subs = subs.into_iter();
            let mut o = Map::new();
            for (k, v) in obj {
                if is_id_array(v) {
                    let Some((_, mut sub)) = subs.next() else {
                        continue;
                    };
                    // entries without an id are the item's own
                    sub.extend(loose_entries(v));
                    if !sub.is_empty() {
                        o.insert(k.clone(), Value::Array(sub));
                    }
                } else {
                    o.insert(k.clone(), v.clone());
                }
            }
            kept.push(id);
            out.push(Value::Object(o));
        }
        out
    }
    let mut out = serde_json::json!({});
    let mut kept = vec![];
    for path in SHARED {
        if let Some(a) = get_path(data, path).and_then(|v| v.as_array()) {
            let sub = prune(a, changed, &mut kept);
            if !sub.is_empty() {
                set_path(&mut out, path, Value::Array(sub));
            }
        }
    }
    for p in &values {
        if let Some(v) = get_path(data, p) {
            set_path(&mut out, p, v.clone());
        }
    }
    for p in &entries {
        let Some((path, key)) = map_entry(p) else {
            continue;
        };
        // one taken away is only its stamp
        let Some(v) = get_path(data, path).and_then(|m| m.get(key)) else {
            continue;
        };
        if get_path(&out, path).is_none() {
            set_path(&mut out, path, Value::Object(Map::new()));
        }
        if let Some(m) = get_path_mut(&mut out, path).and_then(|m| m.as_object_mut()) {
            m.insert(key.to_string(), v.clone());
        }
    }
    let pick = |m: &BTreeMap<u64, i64>| -> BTreeMap<u64, i64> {
        kept.iter()
            .filter_map(|id| m.get(id).map(|t| (*id, *t)))
            .collect()
    };
    let part = SyncMeta {
        stamps: pick(&meta.stamps),
        moved: pick(&meta.moved),
        fields: kept
            .iter()
            .filter_map(|id| meta.fields.get(id).map(|e| (*id, e.clone())))
            .collect(),
        tombs,
        revived,
        values: values
            .iter()
            .chain(&entries)
            .filter_map(|p| meta.values.get(*p).map(|t| (p.to_string(), *t)))
            .collect(),
        v: meta.v,
        ..Default::default()
    };
    set_path(
        &mut out,
        "extra.sync",
        serde_json::to_value(&part).unwrap_or_default(),
    );
    Some(out)
}

/// Put the items `ids` of `data` into `part` (a `delta`) whole, with
/// everything inside them and their bookkeeping, each inside the items it
/// sits in: for someone who may see them again after they were hidden from
/// them. An inner item can be one of them too: a folder moved out of a
/// space closed to them into one they see comes with the lists in it,
/// which didn't change and so aren't in the delta otherwise.
pub fn include_whole(part: &mut Value, data: &Value, ids: &[u64]) {
    use std::collections::HashSet;
    if ids.is_empty() {
        return;
    }
    let want: HashSet<u64> = ids.iter().copied().collect();
    let holds = |v: &Value| {
        let mut inner = BTreeMap::new();
        walk(v, None, false, &mut inner);
        inner.keys().any(|i| want.contains(i))
    };
    // an item's own fields, without the items inside it (as `delta` has it)
    fn shell(item: &Value) -> Value {
        let mut o = Map::new();
        for (k, v) in item.as_object().into_iter().flatten() {
            if is_id_array(v) {
                let loose = loose_entries(v);
                if !loose.is_empty() {
                    o.insert(k.clone(), Value::Array(loose));
                }
            } else {
                o.insert(k.clone(), v.clone());
            }
        }
        Value::Object(o)
    }
    fn put(
        into: &mut Vec<Value>,
        arr: &[Value],
        want: &HashSet<u64>,
        holds: &dyn Fn(&Value) -> bool,
        added: &mut Vec<u64>,
    ) {
        for item in arr {
            let (Some(id), Some(obj)) = (id_of(item), item.as_object()) else {
                continue;
            };
            if want.contains(&id) {
                into.retain(|x| id_of(x) != Some(id));
                into.push(item.clone());
                added.extend(ids_within(item));
                continue;
            }
            for (k, v) in obj {
                if !is_id_array(v) || !holds(v) {
                    continue;
                }
                let at = match into.iter().position(|x| id_of(x) == Some(id)) {
                    Some(i) => i,
                    None => {
                        into.push(shell(item));
                        into.len() - 1
                    }
                };
                added.push(id);
                let host = &mut into[at];
                if !host.get(k).is_some_and(|x| x.is_array()) {
                    host[k.as_str()] = Value::Array(loose_entries(v));
                }
                if let (Some(dst), Some(src)) = (host[k.as_str()].as_array_mut(), v.as_array()) {
                    put(dst, src, want, holds, added);
                }
            }
        }
    }
    let meta = stamps_of(data);
    let mut pm = meta_of(part);
    let mut added = vec![];
    for path in SHARED {
        let Some(a) = get_path(data, path).and_then(|v| v.as_array()) else {
            continue;
        };
        if !holds(&Value::Array(a.clone())) {
            continue;
        }
        if get_path(part, path).is_none_or(|v| !v.is_array()) {
            set_path(part, path, Value::Array(vec![]));
        }
        if let Some(into) = get_path_mut(part, path).and_then(|v| v.as_array_mut()) {
            put(into, a, &want, &holds, &mut added);
        }
    }
    for id in &added {
        if let Some(t) = meta.stamps.get(id) {
            pm.stamps.insert(*id, *t);
        }
        if let Some(t) = meta.moved.get(id) {
            pm.moved.insert(*id, *t);
        }
        if let Some(e) = meta.fields.get(id) {
            pm.fields.insert(*id, e.clone());
        }
    }
    pm.v = meta.v;
    set_path(
        part,
        "extra.sync",
        serde_json::to_value(&pm).unwrap_or_default(),
    );
}

/// Every id in the top-level items `ids` of `data`: theirs and those of
/// everything inside them.
pub fn ids_inside(data: &Value, ids: &[u64]) -> Vec<u64> {
    let ids: std::collections::HashSet<u64> = ids.iter().copied().collect();
    let mut out = BTreeMap::new();
    for path in SHARED {
        for item in get_path(data, path)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter(|x| id_of(x).is_some_and(|i| ids.contains(&i)))
        {
            walk(&Value::Array(vec![item.clone()]), None, false, &mut out);
        }
    }
    out.into_keys().collect()
}
