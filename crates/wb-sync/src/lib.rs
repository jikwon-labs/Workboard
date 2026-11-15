// Working together. Without a server, every person's app writes its own copy
// of the workspace into one shared folder (kept in step by iCloud Drive,
// Dropbox, Google Drive, Syncthing...) and reads everyone else's copies. With
// a server (workboard-server), the server keeps one copy for the team and
// every app sends its copy there and gets the merged one back; the merging
// below is the same in both cases.
//
// Copies are merged item by item, and inside an item field by field. Every
// item with an `id` (task, list, doc, channel, message, reply, member...)
// remembers when it last changed on the device that changed it, and when each
// of its own fields did (`SyncMeta::fields`): two people changing different
// fields of one task both keep their change, and for the same field the
// newest change wins. The items inside it (messages in a channel, lists in a
// space) are merged one by one, so two people chatting or editing different
// tasks never overwrite each other. Reactions are merged per (emoji, person)
// and running timers per person, so reacting or starting a timer at the same
// moment as someone else keeps both; whiteboard strokes and notes have ids
// (`fill_board_ids`) and merge like any other item. Deleted items leave a tombstone so they don't come back; restoring
// one from the trash revives it. Task comments and activity lines have ids
// too, so edits, deletes and reactions carry over; lines saved before they
// had ids get one worked out from their contents (`activity_id`).

pub mod mentions;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Collections that belong to the whole team (everything else stays personal).
pub const SHARED: &[&str] = &[
    "spaces",
    "tasks",
    "members",
    "docs",
    "channels",
    "extra.goals",
    "extra.forms",
    "extra.whiteboards",
    "extra.meetings",
    "extra.fields",
    "extra.automations",
    "extra.templates",
    "extra.dashboards",
    "extra.teams",
    "extra.bookmarks",
];

/// Single settings that belong to the whole team (not lists of items): the
/// newest change wins, by the stamps in `SyncMeta::values`.
pub const SHARED_VALUES: &[&str] = &["workspace", "extra.workspace_color", "extra.apps"];

/// Fields that are each person's own even inside shared items.
const LOCAL_KEYS: &[&str] = &["last_read", "saved", "expanded"];

/// Fields the app leaves out of the file when they are at their default and
/// that can go back to it (a stopped timer, an unresolved comment, the last
/// watcher removed). When the newest copy of an item comes from an app that
/// knows them (`SyncMeta.v` 2 and later) and lacks one, it is at its default
/// there and is not filled back in from an older copy. Any other missing
/// field is kept from the other copies, so an app that doesn't know a field
/// can't erase it. A new field like these goes here with a `META_V` bump.
const DEFAULT_WHEN_MISSING: &[&str] = &[
    "archived",
    "assignee",
    "reactions",
    "resolved",
    "timer_by",
    "timers",
    "milestone",
    "watchers",
    "waiting_on",
    "linked",
    "tags",
    "private",
    "required",
    "options",
    "cards",
    "mentions",
];

/// Lists that only ever grow; every side's entries are kept.
const APPEND_KEYS: &[&str] = &["activity"];

pub const FILE_SUFFIX: &str = ".workboard.json";

/// Where this person's workspace is shared: a server when `server` is set,
/// else a folder.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SharedCfg {
    /// The shared folder (empty when connected to a server).
    #[serde(default)]
    pub folder: String,
    /// This computer's file in the folder: `<device>.workboard.json`.
    pub device: String,
    /// Every new id made here ends in this number (mod 1024), so ids made on
    /// different computers never collide. A server hands this out.
    pub tag: u16,
    /// The workboard-server address, as typed ("macmini", "100.1.2.3:8787").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
}

/// Change bookkeeping, saved with the workspace.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct SyncMeta {
    /// id -> when its own fields last changed (milliseconds; copies saved
    /// before the server used seconds, which simply compare as older).
    pub stamps: BTreeMap<u64, i64>,
    /// id -> when it was deleted.
    pub tombs: BTreeMap<u64, i64>,
    /// id -> when it was restored after a delete.
    pub revived: BTreeMap<u64, i64>,
    /// id -> fingerprint of its own fields when last saved (this computer only).
    pub hashes: BTreeMap<u64, u64>,
    /// How `hashes` were made: 0 = before an item's place (the item it sits
    /// in) counted as one of its fields, 1 = since; 2 = also `moved` is kept,
    /// and fields left at their default are left out of the file (see the
    /// app's `legacy_shape`); 3 = also `fields` is kept.
    pub v: u32,
    /// id -> when it last moved into another item (a list into another space
    /// or folder). Where an item sits follows this, not `stamps`, so someone
    /// renaming a list they never saw move doesn't move it back. Only copies
    /// with `v` 2 or later keep it; for older ones `stamps` decides.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub moved: BTreeMap<u64, i64>,
    /// id -> the item it sat in when last saved (this computer only).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parents: BTreeMap<u64, u64>,
    /// Shared setting (see `SHARED_VALUES`) -> when it last changed.
    pub values: BTreeMap<String, i64>,
    /// Shared setting -> fingerprint when last saved (this computer only).
    pub value_hashes: BTreeMap<String, u64>,
    /// id -> when each of its own fields last changed ("field stamps"; see
    /// `read_part`). A part is a field name, or for fields merged element
    /// by element (reactions, running timers) "reactions/<emoji>/<member>"
    /// and "timers/<member>". The entry "" is the item's base: when every
    /// field not listed last changed (missing: the item's stamp). A field
    /// that is there and not listed changed at the base; a field that was
    /// taken away is listed with when that happened. Items without an entry
    /// (copies from apps before field stamps) count every field as changed
    /// at the item's stamp.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<u64, BTreeMap<String, i64>>,
    /// id -> fingerprint of each part when last saved (this computer only).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub field_hashes: BTreeMap<u64, BTreeMap<String, u32>>,
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

fn id_of(v: &Value) -> Option<u64> {
    v.get("id")?.as_u64()
}

/// A non-empty array whose entries are all objects with an id.
fn is_id_array(v: &Value) -> bool {
    match v.as_array() {
        Some(a) => !a.is_empty() && a.iter().all(|x| x.is_object() && id_of(x).is_some()),
        None => false,
    }
}

fn get_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

fn set_path(v: &mut Value, path: &str, new: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    let mut cur = v;
    for part in &parts[..parts.len() - 1] {
        if cur.get(*part).is_none() {
            cur[*part] = Value::Object(Map::new());
        }
        cur = cur.get_mut(*part).unwrap();
    }
    cur[parts[parts.len() - 1]] = new;
}

/// Fingerprint of an item's own fields (not the items inside it, not personal fields).
fn own_hash(obj: &Map<String, Value>) -> u64 {
    let mut own = Map::new();
    for (k, v) in obj {
        if LOCAL_KEYS.contains(&k.as_str()) {
            continue;
        }
        if v.as_array().map(|a| a.is_empty()).unwrap_or(false) || is_id_array(v) {
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
const TIMER_KEYS: &[&str] = &["timer_started", "timer_by", "timers"];

fn is_empty_arr(v: &Value) -> bool {
    v.as_array().is_some_and(|a| a.is_empty())
}

/// The (emoji, member) pairs of a `reactions` field, in order. None when it
/// isn't a list of `{emoji, users}`.
fn reaction_pairs(v: &Value) -> Option<Vec<(String, u64)>> {
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
fn timer_list(obj: &Map<String, Value>) -> Vec<(u64, i64, bool)> {
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
/// ones and the items inside it, with reactions and running timers split
/// into one part per element. An empty list counts as no field.
fn parts_of(obj: &Map<String, Value>) -> Vec<(String, Value)> {
    let mut out = vec![];
    for (k, v) in obj {
        let k = k.as_str();
        if LOCAL_KEYS.contains(&k)
            || APPEND_KEYS.contains(&k)
            || TIMER_KEYS.contains(&k)
            || is_empty_arr(v)
            || is_id_array(v)
        {
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
fn part_field(part: &str) -> &str {
    part.split_once('/').map_or(part, |(f, _)| f)
}

fn part_hash(v: &Value) -> u32 {
    fnv(serde_json::to_string(v).unwrap_or_default().as_bytes()) as u32
}

/// Every shared item: its id, the item it sits in, its fields.
fn visit<'a>(data: &'a Value, f: &mut dyn FnMut(u64, Option<u64>, &'a Map<String, Value>)) {
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
fn walk(v: &Value, parent: Option<u64>, placed: bool, out: &mut BTreeMap<u64, u64>) {
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

fn fingerprints_with(data: &Value, placed: bool) -> BTreeMap<u64, u64> {
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
fn parents_of(data: &Value) -> BTreeMap<u64, u64> {
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

/// The `SyncMeta::v` this version writes.
pub const META_V: u32 = 3;

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

/// An item's own fields changed at `next` (it last changed at `before`):
/// note which ones in its field stamps. Without fingerprints of its fields
/// from the last save there's no telling which, so all of them did.
fn note_fields(
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

/// Leave out what an entry of field stamps doesn't need: fields that are
/// there and changed at the base, and the base when it is the item's stamp.
fn tidy_fields(
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

fn value_hashes(data: &Value) -> BTreeMap<String, u64> {
    SHARED_VALUES
        .iter()
        .map(|p| {
            (
                p.to_string(),
                fnv(
                    serde_json::to_string(get_path(data, p).unwrap_or(&Value::Null))
                        .unwrap_or_default()
                        .as_bytes(),
                ),
            )
        })
        .collect()
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
    // time keeps none, so it wins only once someone changes it
    for (path, h) in value_hashes(data) {
        if meta.value_hashes.get(&path).is_some_and(|old| *old != h) {
            let next = meta.values.get(&path).map_or(now, |s| now.max(s + 1));
            meta.values.insert(path.clone(), next);
        }
        meta.value_hashes.insert(path, h);
    }
}

type Cands<'a> = HashMap<u64, Vec<(usize, Option<u64>, &'a Value)>>;

struct Ctx<'a> {
    stamps: Vec<&'a BTreeMap<u64, i64>>,
    tombs: BTreeMap<u64, i64>,
    revived: BTreeMap<u64, i64>,
    out: BTreeMap<u64, i64>,
    /// id -> every copy of the item, in every source and every place.
    cands: HashMap<u64, Vec<(usize, &'a Value)>>,
    /// id -> the item it sits in, in the newest copy (None at the top).
    home: HashMap<u64, Option<u64>>,
    /// id -> when it moved there (0: not known)
    home_moved: HashMap<u64, i64>,
    out_moved: BTreeMap<u64, i64>,
    /// source -> whether that copy knows the `DEFAULT_WHEN_MISSING` fields
    /// (`SyncMeta.v` 2 and later).
    omits_defaults: Vec<bool>,
    /// source -> its field stamps
    fields: Vec<&'a BTreeMap<u64, BTreeMap<String, i64>>>,
    out_fields: BTreeMap<u64, BTreeMap<String, i64>>,
}

/// Every item with an id in one source, with the item it sits in.
fn collect<'a>(src: usize, arr: &'a [Value], parent: Option<u64>, out: &mut Cands<'a>) {
    for item in arr {
        if let (Some(id), Some(obj)) = (id_of(item), item.as_object()) {
            out.entry(id).or_default().push((src, parent, item));
            for (_, child) in obj {
                if is_id_array(child) {
                    collect(src, child.as_array().unwrap(), Some(id), out);
                }
            }
        }
    }
}

impl<'a> Ctx<'a> {
    fn new(
        stamps: Vec<&'a BTreeMap<u64, i64>>,
        fields: Vec<&'a BTreeMap<u64, BTreeMap<String, i64>>>,
        moved: Vec<Option<&'a BTreeMap<u64, i64>>>,
        tombs: BTreeMap<u64, i64>,
        revived: BTreeMap<u64, i64>,
        all: Cands<'a>,
    ) -> Self {
        let omits_defaults: Vec<bool> = moved.iter().map(|m| m.is_some()).collect();
        let mut cands = HashMap::new();
        let mut home = HashMap::new();
        let mut home_moved = HashMap::new();
        // the copy that moved it last decides where the item is (copies that
        // don't keep move times count their item's last change as one), then
        // the newest copy; on a tie this computer's
        let key = |src: usize, id: u64| {
            let s = stamps[src].get(&id).copied().unwrap_or(0);
            let m = match moved[src] {
                Some(m) => m.get(&id).copied().unwrap_or(0),
                None => s,
            };
            (m, s)
        };
        for (id, list) in all {
            let mut best = list[0];
            let mut best_key = key(best.0, id);
            for c in &list[1..] {
                let k = key(c.0, id);
                if k > best_key {
                    best = *c;
                    best_key = k;
                }
            }
            home.insert(id, best.1);
            // a move time worth keeping: one the deciding copy keeps, or a
            // real move (the copies disagree where it is). Copies that don't
            // keep move times otherwise record nothing, or every item they
            // win would get one.
            let recorded = if moved[best.0].is_some() || list.iter().any(|c| c.1 != best.1) {
                best_key.0
            } else {
                list.iter()
                    .filter_map(|c| moved[c.0].and_then(|m| m.get(&id)).copied())
                    .max()
                    .unwrap_or(0)
            };
            home_moved.insert(id, recorded);
            cands.insert(id, list.into_iter().map(|(s, _, v)| (s, v)).collect());
        }
        Ctx {
            stamps,
            tombs,
            revived,
            out: BTreeMap::new(),
            cands,
            home,
            home_moved,
            out_moved: BTreeMap::new(),
            omits_defaults,
            fields,
            out_fields: BTreeMap::new(),
        }
    }

    fn alive(&self, id: u64) -> bool {
        match self.tombs.get(&id) {
            None => true,
            Some(t) => self.revived.get(&id).map_or(false, |r| r > t),
        }
    }
}

/// One copy of an item, as `merge_fields` sees it.
struct Side<'a> {
    obj: &'a Map<String, Value>,
    parts: Vec<(String, Value)>,
    /// when its fields not listed in `listed` changed
    base: i64,
    listed: Option<&'a BTreeMap<String, i64>>,
    omits: bool,
}

/// When a copy's version of `part` was set, or None when the copy can't
/// tell (it doesn't have the part, and may simply not know the field).
/// A part that is there changed at its stamp in the field stamps, or else
/// at the base. A missing part is taken away as of its stamp, or as of the
/// base when the copy knows it would be there: a field the app leaves out
/// at its default (`DEFAULT_WHEN_MISSING`, from apps that do), an element
/// missing from a list the copy has.
fn read_part(
    sd: &Side,
    part: &str,
    present: bool,
    in_vec: &std::collections::HashSet<u64>,
) -> Option<i64> {
    if let Some(t) = sd.listed.and_then(|e| e.get(part)) {
        return Some(*t);
    }
    let counts = present
        || match part.split_once('/') {
            Some(("reactions", _)) => {
                sd.omits || sd.obj.get("reactions").is_some_and(|v| !is_empty_arr(v))
            }
            Some(("timers", m)) => {
                let m: u64 = m.parse().unwrap_or(0);
                // an app that doesn't know `timers` can't have stopped one there
                sd.omits
                    || sd.obj.contains_key("timers")
                    || (sd.obj.contains_key("timer_started") && !in_vec.contains(&m))
            }
            _ => DEFAULT_WHEN_MISSING.contains(&part) && sd.omits,
        };
    counts.then_some(sd.base)
}

/// Merge the own fields of one item's copies part by part (see
/// `SyncMeta::fields`): for each part the copy that set it last wins.
/// `stamp` is the item's merged stamp, `newest` the newest copy's fields.
/// Returns the fields and the field names it decided (ones it left out
/// stay out).
fn merge_fields<'a>(
    cx: &mut Ctx<'a>,
    id: u64,
    list: &[(usize, &'a Value)],
    stamp: i64,
    newest: &Map<String, Value>,
) -> (Map<String, Value>, std::collections::HashSet<String>) {
    use std::collections::HashSet;
    // fields holding items in any copy merge as items
    let child: HashSet<&str> = list
        .iter()
        .filter_map(|c| c.1.as_object())
        .flat_map(|o| {
            o.iter()
                .filter(|(_, v)| is_id_array(v))
                .map(|(k, _)| k.as_str())
        })
        .collect();
    let sides: Vec<Side> = list
        .iter()
        .filter_map(|c| {
            let obj = c.1.as_object()?;
            let s = cx.stamps[c.0].get(&id).copied().unwrap_or(0);
            let listed = cx.fields[c.0].get(&id);
            Some(Side {
                obj,
                parts: parts_of(obj)
                    .into_iter()
                    .filter(|(p, _)| !child.contains(part_field(p)))
                    .collect(),
                base: listed.and_then(|e| e.get("")).copied().unwrap_or(s),
                listed,
                omits: cx.omits_defaults.get(c.0).copied().unwrap_or(false),
            })
        })
        .collect();
    let any_entry = sides.iter().any(|s| s.listed.is_some());
    // parts in the order the newest copy has them, so every side ends up
    // with the same order
    let mut order: Vec<&str> = vec![];
    let mut seen = HashSet::new();
    let first = sides
        .iter()
        .position(|sd| std::ptr::eq(sd.obj, newest))
        .unwrap_or(0);
    for sd in sides[first..].iter().chain(&sides[..first]) {
        for (p, _) in &sd.parts {
            if seen.insert(p.as_str()) {
                order.push(p);
            }
        }
        for p in sd.listed.into_iter().flat_map(|e| e.keys()) {
            if !p.is_empty() && !child.contains(part_field(p)) && seen.insert(p.as_str()) {
                order.push(p);
            }
        }
    }
    let in_vec: HashSet<u64> = sides
        .iter()
        .flat_map(|sd| timer_list(sd.obj).into_iter().filter(|t| t.2).map(|t| t.0))
        .collect();
    let base_out = sides.iter().map(|s| s.base).min().unwrap_or(stamp);
    let mut entry = BTreeMap::new();
    let mut obj = Map::new();
    let mut decided: HashSet<String> = HashSet::new();
    let mut reactions: Vec<(String, u64)> = vec![];
    let mut timers: Vec<(u64, i64)> = vec![];
    for p in order {
        let mut win: Option<(i64, &Side, Option<&Value>)> = None;
        for sd in &sides {
            let val = sd.parts.iter().find(|(q, _)| q == p).map(|(_, v)| v);
            let Some(t) = read_part(sd, p, val.is_some(), &in_vec) else {
                continue;
            };
            if win.as_ref().map_or(true, |w| t > w.0) {
                win = Some((t, sd, val));
            }
        }
        let Some((t, sd, val)) = win else { continue };
        if any_entry && (val.is_none() || t != base_out) {
            entry.insert(p.to_string(), t);
        }
        match p.split_once('/') {
            Some(("reactions", rest)) => {
                decided.insert("reactions".into());
                if let (Some(_), Some((e, u))) = (val, rest.rsplit_once('/')) {
                    if let Ok(u) = u.parse() {
                        reactions.push((e.to_string(), u));
                    }
                }
            }
            Some(("timers", _)) => {
                for k in TIMER_KEYS {
                    decided.insert(k.to_string());
                }
                if let Some(v) = val {
                    timers.push((
                        v["member"].as_u64().unwrap_or(0),
                        v["start"].as_i64().unwrap_or(0),
                    ));
                }
            }
            _ => {
                decided.insert(p.to_string());
                match val {
                    Some(v) => {
                        obj.insert(p.to_string(), v.clone());
                    }
                    // taken away there; keep how that copy writes it (an empty list)
                    None => {
                        if let Some(v) = sd.obj.get(p) {
                            obj.insert(p.to_string(), v.clone());
                        }
                    }
                }
            }
        }
    }
    if decided.contains("reactions") {
        let mut out: Vec<(String, Vec<u64>)> = vec![];
        for (e, u) in reactions {
            match out.iter_mut().find(|(x, _)| *x == e) {
                Some((_, users)) => users.push(u),
                None => out.push((e, vec![u])),
            }
        }
        if !out.is_empty() || newest.contains_key("reactions") {
            obj.insert(
                "reactions".into(),
                Value::Array(
                    out.into_iter()
                        .map(|(e, users)| serde_json::json!({"emoji": e, "users": users}))
                        .collect(),
                ),
            );
        }
    }
    if decided.contains("timers") {
        // the same order everywhere: an old timer nobody owns first (only
        // the field older apps read can hold it), then by start
        timers.sort_by_key(|(m, st)| (*m != 0, *st, *m));
        let mut rest = timers.into_iter();
        match rest.next() {
            Some((m, st)) => {
                obj.insert("timer_started".into(), Value::from(st));
                if m != 0 {
                    obj.insert("timer_by".into(), Value::from(m));
                }
            }
            None => {
                if sides.iter().any(|sd| sd.obj.contains_key("timer_started")) {
                    obj.insert("timer_started".into(), Value::Null);
                }
            }
        }
        let rest: Vec<Value> = rest
            .map(|(m, st)| serde_json::json!({"member": m, "start": st}))
            .collect();
        if !rest.is_empty() {
            obj.insert("timers".into(), Value::Array(rest));
        }
    }
    if any_entry {
        if base_out != stamp {
            entry.insert(String::new(), base_out);
        }
        cx.out_fields.insert(id, entry);
    }
    (obj, decided)
}

/// Merge one array of items. `parent` is the item it sits in (None at the
/// top). An item that moved sits only where its newest copy puts it, and its
/// copies from every place are merged, so nothing added inside it is lost.
fn merge_arr<'a>(
    cx: &mut Ctx<'a>,
    sources: &[(usize, &'a Vec<Value>)],
    parent: Option<u64>,
) -> Vec<Value> {
    let mut order: Vec<u64> = vec![];
    let mut seen = std::collections::HashSet::new();
    for (_, arr) in sources {
        for v in arr.iter() {
            let Some(id) = id_of(v) else { continue };
            if seen.insert(id) {
                order.push(id);
            }
        }
    }
    let mut out = vec![];
    for id in order {
        if !cx.alive(id) {
            continue;
        }
        if parent.is_some() && cx.home.get(&id).is_some_and(|h| *h != parent) {
            continue;
        }
        let Some(list) = cx.cands.get(&id).cloned() else {
            continue;
        };
        let list = &list;
        // newest own fields win; on a tie this computer's version stays
        let mut best = list[0];
        let mut best_stamp = cx.stamps[best.0].get(&id).copied().unwrap_or(0);
        for c in &list[1..] {
            let s = cx.stamps[c.0].get(&id).copied().unwrap_or(0);
            if s > best_stamp {
                best = *c;
                best_stamp = s;
            }
        }
        cx.out.insert(id, best_stamp);
        if let Some(m) = cx.home_moved.get(&id).filter(|m| **m > 0) {
            cx.out_moved.insert(id, *m);
        }
        let Some(wobj) = best.1.as_object() else {
            continue;
        };
        let local = list.iter().find(|c| c.0 == 0).and_then(|c| c.1.as_object());
        let mut keys: Vec<String> = wobj.keys().cloned().collect();
        for c in list {
            if let Some(o) = c.1.as_object() {
                for k in o.keys() {
                    if !keys.contains(k) {
                        keys.push(k.clone());
                    }
                }
            }
        }
        // own fields first, field by field (see `merge_fields`)
        let (mut obj, decided) = merge_fields(cx, id, list, best_stamp, wobj);
        for k in keys {
            if decided.contains(k.as_str()) || obj.contains_key(&k) {
                continue;
            }
            let vals: Vec<(usize, &'a Value)> = list
                .iter()
                .filter_map(|c| c.1.get(&k).map(|v| (c.0, v)))
                .collect();
            let value = if vals.iter().any(|(_, v)| is_id_array(v)) {
                let subs: Vec<(usize, &'a Vec<Value>)> = vals
                    .iter()
                    .filter_map(|(s, v)| v.as_array().map(|a| (*s, a)))
                    .collect();
                Value::Array(merge_arr(cx, &subs, Some(id)))
            } else if APPEND_KEYS.contains(&k.as_str()) {
                let mut all: Vec<Value> = vec![];
                for (_, v) in &vals {
                    for x in v.as_array().into_iter().flatten() {
                        if !all.contains(x) {
                            all.push(x.clone());
                        }
                    }
                }
                all.sort_by_key(|x| x.get("ts").and_then(|t| t.as_i64()).unwrap_or(0));
                Value::Array(all)
            } else if LOCAL_KEYS.contains(&k.as_str())
                && local.map_or(false, |o| o.contains_key(&k))
            {
                local.unwrap()[&k].clone()
            } else if let Some(v) = wobj.get(&k) {
                v.clone()
            } else if DEFAULT_WHEN_MISSING.contains(&k.as_str())
                && cx.omits_defaults.get(best.0).copied().unwrap_or(false)
            {
                // the newest copy has it at its default (see `omits_defaults`)
                continue;
            } else {
                vals[0].1.clone()
            };
            obj.insert(k, value);
        }
        out.push(Value::Object(obj));
    }
    // messages and replies read in time order
    if !out.is_empty()
        && out
            .iter()
            .all(|x| x.get("ts").map_or(false, |t| t.is_i64()))
    {
        out.sort_by_key(|x| x["ts"].as_i64().unwrap_or(0));
    }
    out
}

fn meta_of(v: &Value) -> SyncMeta {
    v.get("extra")
        .and_then(|e| e.get("sync"))
        .and_then(|s| serde_json::from_value(s.clone()).ok())
        .unwrap_or_default()
}

/// Merge other people's copies into this one. Returns true when anything changed.
pub fn merge(local: &mut Value, others: &[Value]) -> bool {
    // copies from before activity had ids: give their entries the same ids
    // here as everywhere else, or their comments would not be matched up
    fill_activity_ids(local);
    fill_board_ids(local);
    let filled: Vec<Value> = others
        .iter()
        .map(|o| {
            let mut o = o.clone();
            fill_activity_ids(&mut o);
            fill_board_ids(&mut o);
            o
        })
        .collect();
    let others = &filled[..];
    let before = local.clone();
    let mut lmeta = meta_of(local);
    let metas: Vec<SyncMeta> = others.iter().map(meta_of).collect();
    let mut tombs = lmeta.tombs.clone();
    let mut revived = lmeta.revived.clone();
    for m in &metas {
        for (k, v) in &m.tombs {
            let e = tombs.entry(*k).or_insert(*v);
            *e = (*e).max(*v);
        }
        for (k, v) in &m.revived {
            let e = revived.entry(*k).or_insert(*v);
            *e = (*e).max(*v);
        }
    }
    let empty = vec![];
    let merged: Vec<(String, Value)>;
    let out_stamps;
    let out_moved;
    let out_fields;
    {
        let mut stamps: Vec<&BTreeMap<u64, i64>> = vec![&lmeta.stamps];
        stamps.extend(metas.iter().map(|m| &m.stamps));
        let mut fields: Vec<&BTreeMap<u64, BTreeMap<String, i64>>> = vec![&lmeta.fields];
        fields.extend(metas.iter().map(|m| &m.fields));
        let keeps = |m: &SyncMeta| m.v >= 2;
        let mut moved: Vec<Option<&BTreeMap<u64, i64>>> =
            vec![keeps(&lmeta).then_some(&lmeta.moved)];
        moved.extend(metas.iter().map(|m| keeps(m).then_some(&m.moved)));
        let mut all: Vec<Vec<(usize, &Vec<Value>)>> = vec![];
        let mut cands = HashMap::new();
        for path in SHARED {
            let mut sources: Vec<(usize, &Vec<Value>)> = vec![(
                0,
                get_path(&before, path)
                    .and_then(|v| v.as_array())
                    .unwrap_or(&empty),
            )];
            for (i, o) in others.iter().enumerate() {
                if let Some(a) = get_path(o, path).and_then(|v| v.as_array()) {
                    sources.push((i + 1, a));
                }
            }
            for (src, arr) in &sources {
                collect(*src, arr, None, &mut cands);
            }
            all.push(sources);
        }
        let mut cx = Ctx::new(stamps, fields, moved, tombs.clone(), revived.clone(), cands);
        let mut res = vec![];
        for (path, sources) in SHARED.iter().zip(&all) {
            res.push((
                path.to_string(),
                Value::Array(merge_arr(&mut cx, sources, None)),
            ));
        }
        merged = res;
        out_stamps = cx.out;
        out_moved = cx.out_moved;
        out_fields = cx.out_fields;
    }
    for (path, v) in merged {
        set_path(local, &path, v);
    }
    // shared settings: the newest change wins; on a tie this computer's stays
    for path in SHARED_VALUES {
        let mut best: Option<&Value> = None;
        let mut best_stamp = lmeta.values.get(*path).copied().unwrap_or(0);
        for (o, m) in others.iter().zip(&metas) {
            let s = m.values.get(*path).copied().unwrap_or(0);
            if s > best_stamp {
                if let Some(v) = get_path(o, path) {
                    best = Some(v);
                    best_stamp = s;
                }
            }
        }
        if let Some(v) = best {
            set_path(local, path, v.clone());
        }
        if best_stamp > 0 {
            lmeta.values.insert(path.to_string(), best_stamp);
        }
    }
    lmeta.tombs = tombs;
    lmeta.revived = revived;
    lmeta.stamps = out_stamps;
    lmeta.moved = out_moved;
    shrink_moved(&mut lmeta.moved, local);
    lmeta.fields = out_fields;
    lmeta.hashes = fingerprints(local);
    lmeta.field_hashes = part_hashes(local);
    lmeta.parents = parents_of(local);
    lmeta.value_hashes = value_hashes(local);
    lmeta.v = META_V;
    set_path(
        local,
        "extra.sync",
        serde_json::to_value(&lmeta).unwrap_or_default(),
    );
    // compare without the bookkeeping, which always moves a little
    let strip = |v: &Value| {
        let mut v = v.clone();
        if let Some(e) = v.get_mut("extra").and_then(|e| e.as_object_mut()) {
            e.remove("sync");
        }
        v
    };
    strip(&before) != strip(local)
}

/// The app version a copy in a shared folder was written by (`"app"`,
/// written by `write_mine` from this version on). None for copies from
/// before.
pub fn app_version_of(copy: &Value) -> Option<&str> {
    copy.get("app").and_then(|a| a.as_str())
}

/// "0.1.7" -> (0, 1, 7); missing parts count as 0.
fn version_key(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// A copy written by an app older than `mine` (or by one from before apps
/// wrote their version, or with an older sync format): such an app erases
/// fields it doesn't know when its copy wins. Returns its version when it
/// wrote one ("" when it didn't).
pub fn older_app<'a>(copy: &'a Value, mine: &str) -> Option<&'a str> {
    let v = meta_of(copy).v;
    match app_version_of(copy) {
        None => Some(""),
        Some(a) if version_key(a) < version_key(mine) || v < META_V => Some(a),
        Some(_) => None,
    }
}

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
        let m = meta_of(data);
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
        let m = meta_of(part);
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
        for (path, h) in value_hashes(part) {
            if get_path(part, &path).is_some() {
                self.value_hashes.insert(path.clone(), h);
                if let Some(t) = m.values.get(&path) {
                    self.values.insert(path, *t);
                }
            }
        }
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
    let meta = meta_of(data);
    let changed: std::collections::HashSet<u64> = cur
        .iter()
        .filter(|(id, h)| {
            since.hashes.get(id) != Some(h)
                || since.stamps.get(id) != meta.stamps.get(id)
                || since.fields.get(id) != meta.fields.get(id)
                || since.moved.get(id) != meta.moved.get(id)
        })
        .map(|(id, _)| *id)
        .collect();
    let newer = |a: &BTreeMap<u64, i64>, b: &BTreeMap<u64, i64>| -> BTreeMap<u64, i64> {
        a.iter()
            .filter(|(k, v)| b.get(k).is_none_or(|o| o < v))
            .map(|(k, v)| (*k, *v))
            .collect()
    };
    let tombs = newer(&meta.tombs, &since.tombs);
    let revived = newer(&meta.revived, &since.revived);
    let vh = value_hashes(data);
    let values: Vec<&str> = SHARED_VALUES
        .iter()
        .copied()
        .filter(|p| {
            since.value_hashes.get(*p) != vh.get(*p) || since.values.get(*p) != meta.values.get(*p)
        })
        .collect();
    if changed.is_empty() && tombs.is_empty() && revived.is_empty() && values.is_empty() {
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
            let mut o = Map::new();
            let mut inner = false;
            for (k, v) in obj {
                if is_id_array(v) {
                    let sub = prune(v.as_array().unwrap(), changed, kept);
                    if !sub.is_empty() {
                        inner = true;
                        o.insert(k.clone(), Value::Array(sub));
                    }
                } else {
                    o.insert(k.clone(), v.clone());
                }
            }
            if inner || changed.contains(&id) {
                kept.push(id);
                out.push(Value::Object(o));
            }
        }
        out
    }
    let mut out = serde_json::json!({});
    let mut kept = vec![];
    for path in SHARED {
        if let Some(a) = get_path(data, path).and_then(|v| v.as_array()) {
            let sub = prune(a, &changed, &mut kept);
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

/// Put the top-level items `ids` of `data` into `part` (a `delta`) whole,
/// with their bookkeeping: for someone who may see them again after they
/// were hidden from them.
pub fn include_whole(part: &mut Value, data: &Value, ids: &[u64]) {
    if ids.is_empty() {
        return;
    }
    let meta = meta_of(data);
    let mut pm = meta_of(part);
    let mut added = vec![];
    for path in SHARED {
        let Some(a) = get_path(data, path).and_then(|v| v.as_array()) else {
            continue;
        };
        for item in a
            .iter()
            .filter(|x| id_of(x).is_some_and(|i| ids.contains(&i)))
        {
            if get_path(part, path).is_none_or(|v| !v.is_array()) {
                set_path(part, path, Value::Array(vec![]));
            }
            let arr = get_path_mut(part, path)
                .and_then(|v| v.as_array_mut())
                .unwrap();
            arr.retain(|x| id_of(x) != id_of(item));
            arr.push(item.clone());
            added.push(item.clone());
        }
    }
    let mut inner = BTreeMap::new();
    walk(&Value::Array(added), None, false, &mut inner);
    for id in inner.keys() {
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

/// The other people's files in a shared folder, with their change times.
pub fn other_files(folder: &Path, device: &str) -> Vec<(PathBuf, std::time::SystemTime)> {
    let mine = format!("{}{}", device, FILE_SUFFIX);
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(folder) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(FILE_SUFFIX) && name != mine && !name.starts_with('.') {
                if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                    out.push((e.path(), m));
                }
            }
        }
    }
    out.sort();
    out
}

pub fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// Each person's own settings and reading state in `extra`, left out of the
/// copy other people get: what they've read and closed in chat, starred and
/// recent items, notification and sidebar settings, their timesheet rows and
/// where their copy is shared. None of it is merged from other copies, and
/// the app fills in its default when one is missing.
const PERSONAL_EXTRA: &[&str] = &[
    "chat_seen",
    "closed_dm_seen",
    "closed_dms",
    "replies_seen",
    "replies_read",
    "starred",
    "recent",
    "notify",
    "muted_until",
    "pinned_tools",
    "rail_items",
    "hidden_nav",
    "hidden_views",
    "hidden_cards",
    "accent",
    "auto_theme",
    "labs",
    "shared",
    "sheet_rows",
    "sheet_week_rows",
];

/// The copy other people get: personal notes, reminders, settings and
/// reading state (`PERSONAL_EXTRA`), items marked `personal` and this
/// computer's fingerprints left out.
pub fn shareable(data: &Value) -> Value {
    let mut v = data.clone();
    // items marked personal (bookmarks a person had before bookmarks were
    // shared) stay on this computer
    for path in SHARED {
        if let Some(a) = get_path_mut(&mut v, path).and_then(|a| a.as_array_mut()) {
            a.retain(|x| x.get("personal").and_then(|p| p.as_bool()) != Some(true));
        }
    }
    if let Some(e) = v.get_mut("extra").and_then(|e| e.as_object_mut()) {
        for k in PERSONAL_EXTRA {
            e.remove(*k);
        }
        e.insert("notepad".into(), Value::String(String::new()));
        e.insert("reminders".into(), Value::Array(vec![]));
        e.insert("inbox_cleared".into(), Value::Array(vec![]));
        e.insert("inbox_later".into(), Value::Array(vec![]));
        e.insert("trash".into(), Value::Array(vec![]));
        // filters saved under a name are each person's own and never merged
        e.remove("saved_filters");
        if let Some(s) = e.get_mut("sync").and_then(|s| s.as_object_mut()) {
            s.insert("hashes".into(), Value::Object(Map::new()));
            s.insert("value_hashes".into(), Value::Object(Map::new()));
            s.remove("parents");
            s.remove("field_hashes");
        }
    }
    v
}

/// A DM or a private channel with a member list that doesn't include
/// `member`: they don't get it and can't change it.
pub fn closed_to(ch: &Value, member: u64) -> bool {
    let restricted = ch.get("kind").and_then(|k| k.as_str()) == Some("Direct")
        || ch.get("private").and_then(|p| p.as_bool()) == Some(true);
    let members: Vec<u64> = ch
        .get("members")
        .and_then(|m| m.as_array())
        .map(|a| a.iter().filter_map(|m| m.as_u64()).collect())
        .unwrap_or_default();
    restricted && !members.is_empty() && !members.contains(&member)
}

/// How much someone may do in a space shared with chosen people: read it,
/// also comment on its tasks, also change what is in it (lists, folders,
/// tasks, docs), or also change the space itself and who it's shared with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    View,
    Comment,
    Edit,
    Full,
}

impl Level {
    pub const ALL: [Level; 4] = [Level::Full, Level::Edit, Level::Comment, Level::View];

    /// A level as the file keeps it. A word this version doesn't know (from
    /// a newer app) reads as View, so it never gives more than meant.
    pub fn parse(s: &str) -> Level {
        match s {
            "full" => Level::Full,
            "edit" => Level::Edit,
            "comment" => Level::Comment,
            _ => Level::View,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Level::Full => "full",
            Level::Edit => "edit",
            Level::Comment => "comment",
            Level::View => "view",
        }
    }
}

/// A space shared with chosen people only: it has `sharing` and that
/// doesn't say `"everyone": true`. Spaces from before sharing existed have
/// no `sharing` and are everyone's.
pub fn chosen_only(space: &Value) -> bool {
    space
        .get("sharing")
        .and_then(|s| s.as_object())
        .is_some_and(|s| s.get("everyone").and_then(|e| e.as_bool()) != Some(true))
}

/// What `member` may do in a space; None when it is closed to them.
/// Someone's Personal space (`private_owner`) is only theirs. A space for
/// everyone gives everyone Full, as before sharing existed. A space shared
/// with chosen people gives each the level in `sharing.people`, and owners
/// and admins (`manages`) always Full.
pub fn space_level(space: &Value, member: u64, manages: bool) -> Option<Level> {
    if let Some(owner) = space.get("private_owner").and_then(|o| o.as_u64()) {
        return (owner == member).then_some(Level::Full);
    }
    if manages || !chosen_only(space) {
        return Some(Level::Full);
    }
    space["sharing"]
        .get("people")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
        .find(|p| p.get("member").and_then(|m| m.as_u64()) == Some(member))
        .map(|p| Level::parse(p.get("level").and_then(|l| l.as_str()).unwrap_or("")))
}

/// The id of an item and of every item inside it (a space's folders and lists).
pub fn ids_within(item: &Value) -> Vec<u64> {
    let mut inner = BTreeMap::new();
    walk(&Value::Array(vec![item.clone()]), None, false, &mut inner);
    inner.into_keys().collect()
}

/// An item's own fields: without the items inside it and without each
/// person's own fields (what a change stamp covers).
pub fn own_fields(item: &Value) -> Map<String, Value> {
    let mut own = Map::new();
    for (k, v) in item.as_object().into_iter().flatten() {
        if LOCAL_KEYS.contains(&k.as_str())
            || v.as_array().is_some_and(|a| a.is_empty())
            || is_id_array(v)
        {
            continue;
        }
        own.insert(k.clone(), v.clone());
    }
    own
}

/// The top-level items of a team's copy that `member` doesn't get (and
/// can't change): DMs and private channels they aren't in, spaces closed to
/// them (other people's Personal spaces, spaces shared with chosen people
/// that leave them out; see `space_level`, `manages` for owners and admins)
/// with the tasks, docs, dashboards and forms in them, and other people's
/// private docs (with their sub pages) and dashboards.
pub fn hidden_from(data: &Value, member: u64, manages: bool) -> Vec<u64> {
    let arr = |path: &str| {
        get_path(data, path)
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[])
    };
    let mut out: Vec<u64> = arr("channels")
        .iter()
        .filter(|c| closed_to(c, member))
        .filter_map(id_of)
        .collect();
    let mut spaces = vec![];
    let mut lists = std::collections::HashSet::new();
    for s in arr("spaces") {
        if space_level(s, member, manages).is_some() {
            continue;
        }
        let Some(id) = id_of(s) else { continue };
        spaces.push(id);
        let mut inner = BTreeMap::new();
        walk(&Value::Array(vec![s.clone()]), None, false, &mut inner);
        lists.extend(inner.into_keys());
    }
    out.extend(&spaces);
    out.extend(
        arr("tasks")
            .iter()
            .filter(|t| {
                t.get("list_id")
                    .and_then(|l| l.as_u64())
                    .is_some_and(|l| lists.contains(&l))
            })
            .filter_map(id_of),
    );
    // other people's private dashboards (older ones without a maker are
    // everyone's), and dashboards and forms in their Personal spaces
    out.extend(
        arr("extra.dashboards")
            .iter()
            .filter(|d| {
                let owner = d.get("owner").and_then(|o| o.as_u64()).unwrap_or(0);
                (d.get("private").and_then(|p| p.as_bool()) == Some(true)
                    && owner != 0
                    && owner != member)
                    || d.get("space")
                        .and_then(|s| s.as_u64())
                        .is_some_and(|s| spaces.contains(&s))
            })
            .filter_map(id_of),
    );
    out.extend(
        arr("extra.forms")
            .iter()
            .filter(|f| {
                f.get("list_id")
                    .and_then(|l| l.as_u64())
                    .is_some_and(|l| lists.contains(&l))
            })
            .filter_map(id_of),
    );
    let docs = arr("docs");
    let mut hid: std::collections::HashSet<u64> = docs
        .iter()
        .filter(|d| {
            let owner = d.get("owner").and_then(|o| o.as_u64()).unwrap_or(0);
            let private = d.get("private").and_then(|p| p.as_bool()) == Some(true)
                && owner != 0
                && owner != member;
            private
                || d.get("space")
                    .and_then(|s| s.as_u64())
                    .is_some_and(|s| spaces.contains(&s))
        })
        .filter_map(id_of)
        .collect();
    // sub pages of a hidden page
    loop {
        let more: Vec<u64> = docs
            .iter()
            .filter(|d| {
                d.get("parent")
                    .and_then(|p| p.as_u64())
                    .is_some_and(|p| hid.contains(&p))
            })
            .filter_map(id_of)
            .filter(|i| !hid.contains(i))
            .collect();
        if more.is_empty() {
            break;
        }
        hid.extend(more);
    }
    out.extend(docs.iter().filter_map(id_of).filter(|i| hid.contains(i)));
    out
}

/// Take the top-level items `ids` out of a copy without deleting them: no
/// tombstone, and their change stamps and this computer's fingerprints for
/// them (and everything inside them) go too, so the next save doesn't count
/// them as deleted. What someone may no longer see leaves their copy this
/// way and comes back if they may see it again. Returns what was taken out.
pub fn hide(data: &mut Value, ids: &[u64]) -> Vec<Value> {
    if ids.is_empty() {
        return vec![];
    }
    let mut gone = vec![];
    for path in SHARED {
        let Some(a) = get_path_mut(data, path).and_then(|v| v.as_array_mut()) else {
            continue;
        };
        let (out, keep): (Vec<Value>, Vec<Value>) = std::mem::take(a)
            .into_iter()
            .partition(|x| id_of(x).is_some_and(|i| ids.contains(&i)));
        *a = keep;
        gone.extend(out);
    }
    if gone.is_empty() {
        return gone;
    }
    let mut inner = BTreeMap::new();
    walk(&Value::Array(gone.clone()), None, false, &mut inner);
    if let Some(sync) = get_path_mut(data, "extra.sync").and_then(|s| s.as_object_mut()) {
        for key in [
            "stamps",
            "hashes",
            "moved",
            "parents",
            "fields",
            "field_hashes",
        ] {
            if let Some(m) = sync.get_mut(key).and_then(|m| m.as_object_mut()) {
                for id in inner.keys() {
                    m.remove(&id.to_string());
                }
            }
        }
    }
    gone
}

fn get_path_mut<'a>(v: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut cur = v;
    for part in path.split('.') {
        cur = cur.get_mut(part)?;
    }
    Some(cur)
}

/// Write this computer's copy (personal notes and reminders left out),
/// saying which app version wrote it (`app`, see `older_app`).
pub fn write_mine(folder: &Path, device: &str, data: &Value, app: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let mut v = shareable(data);
    if let Some(o) = v.as_object_mut() {
        o.insert("app".into(), Value::from(app));
    }
    let tmp = folder.join(format!(".{}.tmp", device));
    std::fs::write(&tmp, serde_json::to_string(&v).unwrap_or_default())?;
    std::fs::rename(&tmp, folder.join(format!("{}{}", device, FILE_SUFFIX)))
}

/// Folders of sync services found on this computer, as places to share from.
pub fn suggested_folders() -> Vec<(String, String)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let h = PathBuf::from(&home);
    let mut out = vec![];
    let icloud = h.join("Library/Mobile Documents/com~apple~CloudDocs");
    if icloud.is_dir() {
        out.push((
            "iCloud Drive".to_string(),
            icloud.join("Workboard").to_string_lossy().to_string(),
        ));
    }
    if let Ok(rd) = std::fs::read_dir(h.join("Library/CloudStorage")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("GoogleDrive") {
                let my = e.path().join("My Drive");
                let base = if my.is_dir() { my } else { e.path() };
                out.push((
                    "Google Drive".to_string(),
                    base.join("Workboard").to_string_lossy().to_string(),
                ));
            } else if name.starts_with("Dropbox") {
                out.push((
                    "Dropbox".to_string(),
                    e.path().join("Workboard").to_string_lossy().to_string(),
                ));
            } else if name.starts_with("OneDrive") {
                out.push((
                    "OneDrive".to_string(),
                    e.path().join("Workboard").to_string_lossy().to_string(),
                ));
            }
        }
    }
    let dropbox = h.join("Dropbox");
    if dropbox.is_dir() && !out.iter().any(|(n, _)| n == "Dropbox") {
        out.push((
            "Dropbox".to_string(),
            dropbox.join("Workboard").to_string_lossy().to_string(),
        ));
    }
    out
}

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

fn board_sig(kind: &str, item: &Value) -> String {
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

/// A workspace made of `team`'s shared things (items, shared settings and
/// their change bookkeeping, the id counter) and `personal`'s own settings
/// for everything else: what joining a team workspace opens.
pub fn with_team_things(personal: &Value, team: &Value) -> Value {
    let mut v = personal.clone();
    for path in SHARED
        .iter()
        .chain(SHARED_VALUES)
        .chain(&["extra.sync", "next_id"])
    {
        if let Some(x) = get_path(team, path) {
            set_path(&mut v, path, x.clone());
        }
    }
    v
}

#[cfg(test)]
mod tests {
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
        a["tasks"][0]["activity"] =
            json!([{"ts": 20, "author": 1, "text": "A says", "comment": true}]);
        stamp(&a.clone(), &mut ma, 20);
        a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
        b["tasks"][0]["activity"] =
            json!([{"ts": 25, "author": 2, "text": "B says", "comment": true}]);
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
        let stroke = |x: f64| json!({"color": [0, 0, 0], "width": 2.0, "points": [[x, 1.25], [x + 1.0, 2.0]]});
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
        let mut bloated: BTreeMap<u64, i64> =
            [(7, 5), (100, 5), (101, 30), (102, 5), (999, 5)].into();
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
                "sheet_rows": [[1, 7]], "accent": "pink", "labs": ["x"],
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
}
