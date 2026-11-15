// Working together. Without a server, every person's app writes its own copy
// of the workspace into one shared folder (kept in step by iCloud Drive,
// Dropbox, Google Drive, Syncthing...) and reads everyone else's copies. With
// a server (workboard-server), the server keeps one copy for the team and
// every app sends its copy there and gets the merged one back; the merging
// below is the same in both cases.
//
// Copies are merged item by item. Every item with an `id` (task, list, doc,
// channel, message, reply, member...) remembers when its own fields last
// changed on the device that changed it; the newest version wins, while the
// items inside it (messages in a channel, lists in a space) are merged one by
// one, so two people chatting or editing different tasks never overwrite each
// other. Deleted items leave a tombstone so they don't come back; restoring
// one from the trash revives it. Task comments are only ever added, so both
// sides' comments are kept.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Collections that belong to the whole team (everything else stays personal).
pub const SHARED: &[&str] = &[
    "spaces", "tasks", "members", "docs", "channels", "extra.goals", "extra.forms", "extra.whiteboards", "extra.meetings", "extra.fields", "extra.automations", "extra.templates", "extra.dashboards", "extra.teams",
];

/// Single settings that belong to the whole team (not lists of items): the
/// newest change wins, by the stamps in `SyncMeta::values`.
pub const SHARED_VALUES: &[&str] = &["workspace", "extra.workspace_color", "extra.apps"];

/// Fields that are each person's own even inside shared items.
const LOCAL_KEYS: &[&str] = &["last_read", "saved", "expanded"];

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
    /// in) counted as one of its fields, 1 = since.
    pub v: u32,
    /// Shared setting (see `SHARED_VALUES`) -> when it last changed.
    pub values: BTreeMap<String, i64>,
    /// Shared setting -> fingerprint when last saved (this computer only).
    pub value_hashes: BTreeMap<String, u64>,
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
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
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
    fnv(serde_json::to_string(&Value::Object(own)).unwrap_or_default().as_bytes())
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

fn value_hashes(data: &Value) -> BTreeMap<String, u64> {
    SHARED_VALUES.iter().map(|p| (p.to_string(), fnv(serde_json::to_string(get_path(data, p).unwrap_or(&Value::Null)).unwrap_or_default().as_bytes()))).collect()
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
    for (id, h) in &cur {
        let next = meta.stamps.get(id).map_or(now, |s| now.max(s + 1));
        match meta.hashes.get(id) {
            Some(old) if old == h => {}
            Some(_) => {
                meta.stamps.insert(*id, next);
            }
            None => {
                meta.stamps.insert(*id, next);
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
        }
    }
    meta.hashes = cur;
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
    fn new(stamps: Vec<&'a BTreeMap<u64, i64>>, tombs: BTreeMap<u64, i64>, revived: BTreeMap<u64, i64>, all: Cands<'a>) -> Self {
        let mut cands = HashMap::new();
        let mut home = HashMap::new();
        for (id, list) in all {
            // the newest copy decides where the item is; on a tie this computer's
            let mut best = list[0];
            let mut best_stamp = stamps[best.0].get(&id).copied().unwrap_or(0);
            for c in &list[1..] {
                let s = stamps[c.0].get(&id).copied().unwrap_or(0);
                if s > best_stamp {
                    best = *c;
                    best_stamp = s;
                }
            }
            home.insert(id, best.1);
            cands.insert(id, list.into_iter().map(|(s, _, v)| (s, v)).collect());
        }
        Ctx { stamps, tombs, revived, out: BTreeMap::new(), cands, home }
    }

    fn alive(&self, id: u64) -> bool {
        match self.tombs.get(&id) {
            None => true,
            Some(t) => self.revived.get(&id).map_or(false, |r| r > t),
        }
    }
}

/// Merge one array of items. `parent` is the item it sits in (None at the
/// top). An item that moved sits only where its newest copy puts it, and its
/// copies from every place are merged, so nothing added inside it is lost.
fn merge_arr<'a>(cx: &mut Ctx<'a>, sources: &[(usize, &'a Vec<Value>)], parent: Option<u64>) -> Vec<Value> {
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
        let Some(list) = cx.cands.get(&id).cloned() else { continue };
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
        let Some(wobj) = best.1.as_object() else { continue };
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
        let mut obj = Map::new();
        for k in keys {
            let vals: Vec<(usize, &'a Value)> = list.iter().filter_map(|c| c.1.get(&k).map(|v| (c.0, v))).collect();
            let value = if vals.iter().any(|(_, v)| is_id_array(v)) {
                let subs: Vec<(usize, &'a Vec<Value>)> = vals.iter().filter_map(|(s, v)| v.as_array().map(|a| (*s, a))).collect();
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
            } else if LOCAL_KEYS.contains(&k.as_str()) && local.map_or(false, |o| o.contains_key(&k)) {
                local.unwrap()[&k].clone()
            } else if let Some(v) = wobj.get(&k) {
                v.clone()
            } else {
                vals[0].1.clone()
            };
            obj.insert(k, value);
        }
        out.push(Value::Object(obj));
    }
    // messages and replies read in time order
    if !out.is_empty() && out.iter().all(|x| x.get("ts").map_or(false, |t| t.is_i64())) {
        out.sort_by_key(|x| x["ts"].as_i64().unwrap_or(0));
    }
    out
}

fn meta_of(v: &Value) -> SyncMeta {
    v.get("extra").and_then(|e| e.get("sync")).and_then(|s| serde_json::from_value(s.clone()).ok()).unwrap_or_default()
}

/// Merge other people's copies into this one. Returns true when anything changed.
pub fn merge(local: &mut Value, others: &[Value]) -> bool {
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
    {
        let mut stamps: Vec<&BTreeMap<u64, i64>> = vec![&lmeta.stamps];
        stamps.extend(metas.iter().map(|m| &m.stamps));
        let mut all: Vec<Vec<(usize, &Vec<Value>)>> = vec![];
        let mut cands = HashMap::new();
        for path in SHARED {
            let mut sources: Vec<(usize, &Vec<Value>)> = vec![(0, get_path(&before, path).and_then(|v| v.as_array()).unwrap_or(&empty))];
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
        let mut cx = Ctx::new(stamps, tombs.clone(), revived.clone(), cands);
        let mut res = vec![];
        for (path, sources) in SHARED.iter().zip(&all) {
            res.push((path.to_string(), Value::Array(merge_arr(&mut cx, sources, None))));
        }
        merged = res;
        out_stamps = cx.out;
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
    lmeta.hashes = fingerprints(local);
    lmeta.value_hashes = value_hashes(local);
    lmeta.v = 1;
    set_path(local, "extra.sync", serde_json::to_value(&lmeta).unwrap_or_default());
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

/// The copy other people get: personal notes, reminders and this computer's
/// fingerprints left out.
pub fn shareable(data: &Value) -> Value {
    let mut v = data.clone();
    if let Some(e) = v.get_mut("extra").and_then(|e| e.as_object_mut()) {
        e.insert("notepad".into(), Value::String(String::new()));
        e.insert("reminders".into(), Value::Array(vec![]));
        e.insert("inbox_cleared".into(), Value::Array(vec![]));
        e.insert("inbox_later".into(), Value::Array(vec![]));
        e.insert("trash".into(), Value::Array(vec![]));
        if let Some(s) = e.get_mut("sync").and_then(|s| s.as_object_mut()) {
            s.insert("hashes".into(), Value::Object(Map::new()));
            s.insert("value_hashes".into(), Value::Object(Map::new()));
        }
    }
    v
}

/// Write this computer's copy (personal notes and reminders left out).
pub fn write_mine(folder: &Path, device: &str, data: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let v = shareable(data);
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
        out.push(("iCloud Drive".to_string(), icloud.join("Workboard").to_string_lossy().to_string()));
    }
    if let Ok(rd) = std::fs::read_dir(h.join("Library/CloudStorage")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("GoogleDrive") {
                let my = e.path().join("My Drive");
                let base = if my.is_dir() { my } else { e.path() };
                out.push(("Google Drive".to_string(), base.join("Workboard").to_string_lossy().to_string()));
            } else if name.starts_with("Dropbox") {
                out.push(("Dropbox".to_string(), e.path().join("Workboard").to_string_lossy().to_string()));
            } else if name.starts_with("OneDrive") {
                out.push(("OneDrive".to_string(), e.path().join("Workboard").to_string_lossy().to_string()));
            }
        }
    }
    let dropbox = h.join("Dropbox");
    if dropbox.is_dir() && !out.iter().any(|(n, _)| n == "Dropbox") {
        out.push(("Dropbox".to_string(), dropbox.join("Workboard").to_string_lossy().to_string()));
    }
    out
}

/// A workspace made of `team`'s shared things (items, shared settings and
/// their change bookkeeping, the id counter) and `personal`'s own settings
/// for everything else: what joining a team workspace opens.
pub fn with_team_things(personal: &Value, team: &Value) -> Value {
    let mut v = personal.clone();
    for path in SHARED.iter().chain(SHARED_VALUES).chain(&["extra.sync", "next_id"]) {
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
        a = ws(json!([]), ch(json!([{"id": 11, "author": 1, "text": "hi", "ts": 100}, {"id": 21, "author": 1, "text": "from A", "ts": 200}]), 500), &ma);
        stamp(&a.clone(), &mut ma, 200);
        a["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
        b = ws(json!([]), ch(json!([{"id": 11, "author": 1, "text": "hi", "ts": 100}, {"id": 31, "author": 2, "text": "from B", "ts": 150}]), 9), &mb);
        stamp(&b.clone(), &mut mb, 150);
        b["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
        assert!(merge(&mut a, &[b]));
        let msgs: Vec<&str> = a["channels"][0]["messages"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap()).collect();
        assert_eq!(msgs, vec!["hi", "from B", "from A"]);
        assert_eq!(a["channels"][0]["last_read"], 500, "read position stays personal");
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
        let names: Vec<&str> = a["tasks"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["one by A"]);
        // and B sees the same after reading A
        merge(&mut b, &[a.clone()]);
        let names: Vec<&str> = b["tasks"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["one by A"]);
    }

    #[test]
    fn comments_from_both_sides_are_kept() {
        let mut ma = SyncMeta::default();
        let mut a = ws(json!([{"id": 1, "name": "t", "activity": []}]), json!([]), &ma);
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
        let mut other = ws(json!([]), json!([]), &SyncMeta { tombs: [(1, 20)].into(), ..Default::default() });
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
        let (mut a, mut ma, mut b, _) = pair(json!([space(100, json!([]), json!([list(101), list(102)])), space(200, json!([]), json!([]))]));
        change(&mut a, &mut ma, json!([space(100, json!([]), json!([list(102)])), space(200, json!([]), json!([list(101)]))]), 30);
        merge(&mut a, &[b.clone()]);
        assert_eq!(places(&a), vec![(100, 102), (200, 101)]);
        merge(&mut b, &[a.clone()]);
        assert_eq!(places(&b), vec![(100, 102), (200, 101)]);

        // folder to another space, and back from a space into a folder
        let (mut a, mut ma, mut b, _) = pair(json!([space(100, json!([folder(300, json!([list(101)]))]), json!([])), space(200, json!([]), json!([]))]));
        change(&mut a, &mut ma, json!([space(100, json!([folder(300, json!([]))]), json!([])), space(200, json!([]), json!([list(101)]))]), 30);
        let stale = b.clone();
        merge(&mut b, &[a.clone()]);
        assert_eq!(places(&b), vec![(200, 101)]);
        merge(&mut a, &[stale]);
        assert_eq!(places(&a), vec![(200, 101)]);
        let mut mb = meta_of(&b);
        change(&mut b, &mut mb, json!([space(100, json!([folder(300, json!([list(101)]))]), json!([])), space(200, json!([]), json!([]))]), 40);
        let stale = a.clone();
        merge(&mut a, &[b.clone()]);
        assert_eq!(places(&a), vec![(300, 101)]);
        merge(&mut b, &[stale]);
        assert_eq!(places(&b), vec![(300, 101)]);
    }

    #[test]
    fn a_list_added_while_another_moved_is_kept() {
        let (mut a, mut ma, mut b, mut mb) = pair(json!([space(100, json!([]), json!([list(101), list(102)])), space(200, json!([]), json!([]))]));
        // A moves 101 away; B, not knowing, adds 103 to the old space
        change(&mut a, &mut ma, json!([space(100, json!([]), json!([list(102)])), space(200, json!([]), json!([list(101)]))]), 30);
        change(&mut b, &mut mb, json!([space(100, json!([]), json!([list(101), list(102), list(103)])), space(200, json!([]), json!([]))]), 25);
        let (a0, b0) = (a.clone(), b.clone());
        merge(&mut a, &[b0]);
        merge(&mut b, &[a0]);
        assert_eq!(places(&a), vec![(100, 102), (100, 103), (200, 101)]);
        assert_eq!(places(&b), places(&a));

        // a folder moves while someone adds a list inside it
        let (mut a, mut ma, mut b, mut mb) = pair(json!([space(100, json!([folder(300, json!([list(101)]))]), json!([])), space(200, json!([]), json!([]))]));
        change(&mut a, &mut ma, json!([space(100, json!([]), json!([])), space(200, json!([folder(300, json!([list(101)]))]), json!([]))]), 30);
        change(&mut b, &mut mb, json!([space(100, json!([folder(300, json!([list(101), list(104)]))]), json!([])), space(200, json!([]), json!([]))]), 25);
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
        let d = sp(json!([space(100, json!([]), json!([list(101)]))]), &SyncMeta::default());
        let mut m = SyncMeta { hashes: fingerprints_with(&d, false), stamps: [(100, 10), (101, 10)].into(), ..Default::default() };
        stamp(&d, &mut m, 99);
        assert_eq!(m.stamps[&101], 10);
        assert_eq!(m.v, 1);
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
}
