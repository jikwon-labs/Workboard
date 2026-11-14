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

fn walk(v: &Value, out: &mut BTreeMap<u64, u64>) {
    if let Some(a) = v.as_array() {
        for item in a {
            if let (Some(id), Some(obj)) = (id_of(item), item.as_object()) {
                out.insert(id, own_hash(obj));
                for (_, child) in obj {
                    if is_id_array(child) {
                        walk(child, out);
                    }
                }
            }
        }
    }
}

/// Fingerprints of every shared item in a workspace.
pub fn fingerprints(data: &Value) -> BTreeMap<u64, u64> {
    let mut out = BTreeMap::new();
    for path in SHARED {
        if let Some(v) = get_path(data, path) {
            walk(v, &mut out);
        }
    }
    out
}

/// Record what changed since the last save: new stamps for edited items,
/// tombstones for removed ones, revivals for restored ones. `now` is in
/// milliseconds. A new stamp is always later than the item's last one, so a
/// second edit made within the same moment still wins over the first.
pub fn stamp(data: &Value, meta: &mut SyncMeta, now: i64) {
    let cur = fingerprints(data);
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
            meta.tombs.insert(*id, now);
            meta.stamps.remove(id);
        }
    }
    meta.hashes = cur;
}

struct Ctx<'a> {
    stamps: Vec<&'a BTreeMap<u64, i64>>,
    tombs: BTreeMap<u64, i64>,
    revived: BTreeMap<u64, i64>,
    out: BTreeMap<u64, i64>,
}

impl Ctx<'_> {
    fn alive(&self, id: u64) -> bool {
        match self.tombs.get(&id) {
            None => true,
            Some(t) => self.revived.get(&id).map_or(false, |r| r > t),
        }
    }
}

fn merge_arr(cx: &mut Ctx, sources: &[(usize, &Vec<Value>)]) -> Vec<Value> {
    let mut order: Vec<u64> = vec![];
    let mut cands: HashMap<u64, Vec<(usize, &Value)>> = HashMap::new();
    for (src, arr) in sources {
        for v in arr.iter() {
            let Some(id) = id_of(v) else { continue };
            let e = cands.entry(id).or_default();
            if e.is_empty() {
                order.push(id);
            }
            e.push((*src, v));
        }
    }
    let mut out = vec![];
    for id in order {
        if !cx.alive(id) {
            continue;
        }
        let list = &cands[&id];
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
            let vals: Vec<(usize, &Value)> = list.iter().filter_map(|c| c.1.get(&k).map(|v| (c.0, v))).collect();
            let value = if vals.iter().any(|(_, v)| is_id_array(v)) {
                let subs: Vec<(usize, &Vec<Value>)> = vals.iter().filter_map(|(s, v)| v.as_array().map(|a| (*s, a))).collect();
                Value::Array(merge_arr(cx, &subs))
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
        let mut cx = Ctx { stamps, tombs: tombs.clone(), revived: revived.clone(), out: BTreeMap::new() };
        let mut res = vec![];
        for path in SHARED {
            let mut sources: Vec<(usize, &Vec<Value>)> = vec![(0, get_path(&before, path).and_then(|v| v.as_array()).unwrap_or(&empty))];
            for (i, o) in others.iter().enumerate() {
                if let Some(a) = get_path(o, path).and_then(|v| v.as_array()) {
                    sources.push((i + 1, a));
                }
            }
            res.push((path.to_string(), Value::Array(merge_arr(&mut cx, &sources))));
        }
        merged = res;
        out_stamps = cx.out;
    }
    for (path, v) in merged {
        set_path(local, &path, v);
    }
    lmeta.tombs = tombs;
    lmeta.revived = revived;
    lmeta.stamps = out_stamps;
    lmeta.hashes = fingerprints(local);
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
}
