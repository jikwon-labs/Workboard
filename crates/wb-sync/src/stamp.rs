// Stamping: noting, before a copy is saved, which items and fields changed
// since the last save and when.

use super::*;

/// The `SyncMeta::v` this version writes. A copy of an older one is
/// upgraded when stamped: fingerprints that still match are carried over
/// (see `upgrade_split_hashes`), so the new format alone isn't an edit.
pub const META_V: u32 = 5;

/// After the app changed how items are written (`legacy` is the workspace as
/// the old way wrote it, `current` the new way): fingerprints that still
/// match the old way are carried over, so the change in shape alone doesn't
/// count as an edit.
pub fn carry_hashes(meta: &mut SyncMeta, legacy: &Value, current: &Value) {
    let cur = fingerprints(current);
    let parts = part_hashes(current);
    for (id, old) in fingerprints(legacy) {
        if meta.hashes.get(&id) == Some(&old) {
            if let Some(h) = cur.get(&id) {
                meta.hashes.insert(id, *h);
                if let Some(p) = parts.get(&id) {
                    meta.field_hashes.insert(id, p.clone());
                }
            }
        }
    }
}

/// For a copy saved before field fingerprints were kept (`SyncMeta::v`
/// below 3): the fingerprints of each field of every item that hasn't
/// changed since the last save, so the next change to one of them is noted
/// field by field (a field emptied, like the last attachment removed,
/// then stays empty) instead of as a change to the whole item. Items
/// changed since stay as they were. True when anything was filled in.
pub fn fill_field_hashes(meta: &mut SyncMeta, data: &Value) -> bool {
    if meta.hashes.is_empty() {
        return false;
    }
    let cur = fingerprints(data);
    // fingerprints from before an item's place counted (see `stamp`)
    let flat = (meta.v < 1).then(|| fingerprints_with(data, false));
    let mut filled = false;
    for (id, parts) in part_hashes(data) {
        if meta.field_hashes.contains_key(&id) {
            continue;
        }
        let Some(old) = meta.hashes.get(&id) else {
            continue;
        };
        if cur.get(&id) == Some(old) || flat.as_ref().is_some_and(|f| f.get(&id) == Some(old)) {
            meta.field_hashes.insert(id, parts);
            filled = true;
        }
    }
    filled
}

/// An item's own fields changed at `next` (it last changed at `before`):
/// note which ones in its field stamps. Without fingerprints of its fields
/// from the last save there's no telling which, so all of them did.
pub(crate) fn note_fields(
    meta: &mut SyncMeta,
    id: u64,
    before: i64,
    now_parts: Option<&BTreeMap<String, u32>>,
    next: i64,
) {
    let empty = BTreeMap::new();
    let now_parts = now_parts.unwrap_or(&empty);
    let Some(old_parts) = meta.field_hashes.get(&id) else {
        if let Some(e) = meta.fields.get_mut(&id) {
            e.retain(|p, _| !p.is_empty() && !now_parts.contains_key(p));
        }
        return;
    };
    let mut changed: Vec<String> = now_parts
        .iter()
        .filter(|(p, h)| old_parts.get(*p) != Some(h))
        .map(|(p, _)| p.clone())
        .collect();
    changed.extend(
        old_parts
            .keys()
            .filter(|p| !now_parts.contains_key(*p))
            .cloned(),
    );
    let e = meta.fields.entry(id).or_default();
    let base = e.get("").copied().unwrap_or(before);
    for p in changed {
        e.insert(p, next);
    }
    tidy_fields(e, |p| now_parts.contains_key(p), base, next);
}

/// Fields split into parts since field stamps began (all but reactions):
/// fingerprints from before have one part for the whole field.
pub(crate) const SPLIT_LATER: &[&str] = &["time_entries", "custom"];

/// Fingerprints saved before time entries and custom field values were
/// split into parts have one for the whole field: when it still matches,
/// it is swapped for today's parts, so the new way of splitting alone
/// doesn't count as a change to every element.
pub(crate) fn upgrade_split_hashes(
    meta: &mut SyncMeta,
    data: &Value,
    parts: &BTreeMap<u64, BTreeMap<String, u32>>,
) {
    if !meta
        .field_hashes
        .values()
        .any(|e| SPLIT_LATER.iter().any(|k| e.contains_key(*k)))
    {
        return;
    }
    let field_hashes = &mut meta.field_hashes;
    visit(data, &mut |id, _, obj| {
        let Some(old) = field_hashes.get_mut(&id) else {
            return;
        };
        let now = parts.get(&id);
        for k in SPLIT_LATER {
            let Some(h) = old.get(*k).copied() else {
                continue;
            };
            // a value that doesn't split is still one part
            if now.is_some_and(|p| p.contains_key(*k)) {
                continue;
            }
            if obj.get(*k).is_some_and(|v| part_hash(v) == h) {
                old.remove(*k);
                for (p, ph) in now.into_iter().flatten() {
                    if part_field(p) == *k && p.len() > k.len() {
                        old.insert(p.clone(), *ph);
                    }
                }
            }
        }
    });
}

/// Leave out what an entry of field stamps doesn't need: fields that are
/// there and changed at the base, and the base when it is the item's stamp.
pub(crate) fn tidy_fields(
    e: &mut BTreeMap<String, i64>,
    present: impl Fn(&str) -> bool,
    base: i64,
    stamp: i64,
) {
    e.retain(|p, t| p.is_empty() || !(present(p) && *t == base));
    if base == stamp {
        e.remove("");
    } else {
        e.insert(String::new(), base);
    }
}

/// Note that `fields` of the item `id` changed at `now` (for code that
/// edits a workspace's JSON directly, like the server labelling roles).
pub fn touch_fields(meta: &mut SyncMeta, id: u64, fields: &[&str], now: i64) -> i64 {
    let before = meta.stamps.get(&id).copied().unwrap_or(0);
    let next = now.max(before + 1);
    let e = meta.fields.entry(id).or_default();
    let base = e.get("").copied().unwrap_or(before);
    for f in fields {
        e.insert(f.to_string(), next);
    }
    if base == next {
        e.remove("");
    } else {
        e.insert(String::new(), base);
    }
    meta.stamps.insert(id, next);
    next
}

/// Fingerprints of the shared settings, and of each entry of the shared
/// maps there is (`map_part`): `SyncMeta::value_hashes` for a copy as it
/// is now.
pub fn value_hashes(data: &Value) -> BTreeMap<String, u64> {
    let mut out: BTreeMap<String, u64> = SHARED_VALUES
        .iter()
        .map(|p| {
            (
                p.to_string(),
                json_hash(get_path(data, p).unwrap_or(&Value::Null)),
            )
        })
        .collect();
    for path in SHARED_MAPS {
        for (k, v) in get_path(data, path)
            .and_then(|m| m.as_object())
            .into_iter()
            .flatten()
        {
            out.insert(map_part(path, k), json_hash(v));
        }
    }
    out
}

/// An entry an upgrade put into a shared map (worked out from what an
/// older version kept somewhere else) rather than anyone's decision: noted
/// as saved already and without a stamp, so `stamp` doesn't take it as set
/// now and any real decision, here or on another copy, wins over it.
pub fn carry_map_entry(meta: &mut SyncMeta, path: &str, key: &str, value: &Value) {
    meta.value_hashes
        .insert(map_part(path, key), json_hash(value));
}

/// After opening a saved copy: a shared setting or shared map entry it has
/// no fingerprint for gets today's, without a stamp. Every save since
/// shared maps came in fingerprints every entry, so one missing here was
/// saved by a version that didn't know the map (the iPhone keeps fields it
/// doesn't know, a map the team's copy brought in included). Without this
/// the next change would take each such entry as set just now and
/// overwrite the team's newer decisions with it. An entry that turns up
/// without a fingerprint after this was added since the copy was opened.
pub fn fill_value_hashes(meta: &mut SyncMeta, data: &Value) {
    for (part, h) in value_hashes(data) {
        meta.value_hashes.entry(part).or_insert(h);
    }
}

/// Only items inside a space (folders, lists) ever move; move times kept
/// for anything else are left out. Copies merged with an older app's copy
/// used to get one for every item.
pub fn shrink_moved(moved: &mut BTreeMap<u64, i64>, data: &Value) {
    if moved.is_empty() {
        return;
    }
    let mut movable = std::collections::HashSet::new();
    if let Some(spaces) = data.get("spaces") {
        let mut inner = BTreeMap::new();
        walk(spaces, None, false, &mut inner);
        for s in spaces.as_array().into_iter().flatten().filter_map(id_of) {
            inner.remove(&s);
        }
        movable.extend(inner.into_keys());
    }
    moved.retain(|id, _| movable.contains(id));
}

/// Record what changed since the last save: new stamps for edited items,
/// tombstones for removed ones, revivals for restored ones. `now` is in
/// milliseconds. A new stamp is always later than the item's last one, so a
/// second edit made within the same moment still wins over the first.
pub fn stamp(data: &Value, meta: &mut SyncMeta, now: i64) {
    let cur = fingerprints(data);
    if meta.v < 1 {
        // fingerprints saved before an item's place counted: carry over the
        // unchanged ones so updating the app doesn't mark everything as edited
        for (id, old) in fingerprints_with(data, false) {
            if meta.hashes.get(&id) == Some(&old) {
                if let Some(h) = cur.get(&id) {
                    meta.hashes.insert(id, *h);
                }
            }
        }
        meta.v = 1;
    }
    // places: an item now in another item than when last saved moved there
    let places = parents_of(data);
    for (id, p) in &places {
        if meta.parents.get(id).is_some_and(|old| old != p) {
            let next = meta.moved.get(id).map_or(now, |s| now.max(s + 1));
            meta.moved.insert(*id, next);
        }
    }
    meta.parents = places;
    shrink_moved(&mut meta.moved, data);
    meta.v = META_V;
    let parts = part_hashes(data);
    upgrade_split_hashes(meta, data, &parts);
    for (id, h) in &cur {
        let next = meta.stamps.get(id).map_or(now, |s| now.max(s + 1));
        match meta.hashes.get(id) {
            Some(old) if old == h => {}
            Some(_) => {
                let before = meta.stamps.get(id).copied().unwrap_or(0);
                note_fields(meta, *id, before, parts.get(id), next);
                meta.stamps.insert(*id, next);
            }
            None => {
                meta.stamps.insert(*id, next);
                // new (or back): every field changed now
                meta.fields.insert(*id, BTreeMap::new());
                if let Some(t) = meta.tombs.get(id) {
                    if meta.revived.get(id).map_or(true, |r| r < t) {
                        meta.revived.insert(*id, now.max(*t + 1));
                    }
                }
            }
        }
    }
    for id in meta.hashes.keys() {
        if !cur.contains_key(id) {
            // a delete always wins over the item's last known change or
            // restore, even when this computer's clock is behind
            let mut t = now;
            if let Some(s) = meta.stamps.get(id) {
                t = t.max(s + 1);
            }
            if let Some(r) = meta.revived.get(id) {
                t = t.max(r + 1);
            }
            meta.tombs.insert(*id, t);
            meta.stamps.remove(id);
            meta.moved.remove(id);
            meta.fields.remove(id);
        }
    }
    meta.hashes = cur;
    meta.field_hashes = parts;
    // shared settings: a changed one gets a new stamp; one seen for the first
    // time keeps none, so it wins only once someone changes it. An entry of
    // a shared map that is new since the last save was just set (but on a
    // copy never stamped before, like a setting, it keeps none), and one
    // gone since was just taken away.
    let vh = value_hashes(data);
    let stamped_before = SHARED_VALUES
        .iter()
        .any(|p| meta.value_hashes.contains_key(*p));
    let bump = |values: &mut BTreeMap<String, i64>, part: &str| {
        let next = values.get(part).map_or(now, |s| now.max(s + 1));
        values.insert(part.to_string(), next);
    };
    for (part, h) in &vh {
        let changed = match meta.value_hashes.get(part) {
            Some(old) => old != h,
            None => stamped_before && map_entry(part).is_some(),
        };
        if changed {
            bump(&mut meta.values, part);
        }
    }
    let gone: Vec<String> = meta
        .value_hashes
        .keys()
        .filter(|p| map_entry(p).is_some() && !vh.contains_key(*p))
        .cloned()
        .collect();
    for part in gone {
        bump(&mut meta.values, &part);
    }
    meta.value_hashes.retain(|p, _| map_entry(p).is_none());
    meta.value_hashes.extend(vh);
}
