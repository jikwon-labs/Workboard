// Spaces shared with chosen people, on the server: what a copy someone sends
// may change in each space, by the level the space gives them
// (`wb_sync::space_level`).
//
//   View     nothing (opening a doc still notes their own view of it)
//   Comment  also add comments to tasks, and edit or delete their own
//   Edit     anything inside: lists, folders, tasks, docs, dashboards, forms
//   Full     also the space itself: name, color, statuses, sharing, delete
//
// A space for everyone gives everyone Full, as before sharing existed, and
// owners and admins have Full everywhere except other people's Personal
// spaces (which stay closed to them, as before).
//
// Apps send whole copies, so a copy is fixed up before it is merged: what
// its sender may not change is put back to the team's version and their
// change stamps for it are dropped, so none of it wins; a delete they may
// not make is undone (the item counts as restored right after it); an item
// they made (or moved in) where they may not is left out. After the merge,
// `put_back` moves the team's stamps past theirs and gives the items they
// made a tombstone, so their own app takes the team's version back with the
// next update instead of keeping (and sending) its own forever. Nobody
// else's data changes: the team's version is what everyone already has.
//
// The same goes for what changes the workspace for everyone in a copy from
// a plain member (`guard_people`): only the owner and admins add or remove
// people, change anyone else's entry or any role, rename or recolour the
// workspace, turn its features on or off, and make, change or delete custom
// fields, automations, templates and teams. And chat messages and task
// comments stay their authors' (`guard_authors`): a plain member can't write
// as someone else or change or delete what someone else wrote.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use wb_sync::{Level, SyncMeta};

/// What to do to the team's copy after merging a fixed-up copy.
#[derive(Default, Debug, PartialEq)]
pub struct PutBack {
    /// id -> (their change stamp, their move stamp): the team's go past
    /// these (the change stamp is the newest of their field stamps too).
    pub bump: BTreeMap<u64, (i64, i64)>,
    /// id -> tombstone time: items they made where they may not.
    pub bury: BTreeMap<u64, i64>,
    /// id -> their tombstone: deletes they may not make. The team's copy
    /// keeps the tombstone (so their app sees it was taken in) and counts
    /// the item as restored right after it.
    pub revive: BTreeMap<u64, i64>,
    /// Shared setting -> their stamp for it: a change of the workspace's
    /// name or colour they may not make. The team's stamp goes past it.
    pub values: BTreeMap<String, i64>,
    /// Something they changed was turned down (worth telling them).
    pub refused: bool,
}

impl PutBack {
    pub fn is_empty(&self) -> bool {
        self.bump.is_empty()
            && self.bury.is_empty()
            && self.revive.is_empty()
            && self.values.is_empty()
            && !self.refused
    }
}

/// Shared settings only the owner and admins change: the workspace's name
/// and colour (Settings > General) and its features (Settings > Apps).
pub const ADMIN_VALUES: &[&str] = &["workspace", "extra.workspace_color", "extra.apps"];

/// Team-wide collections only the owner and admins add to, change or
/// delete from: custom fields, automations, templates and teams. Everyone
/// uses them (a field's value on a task, a template to make a task).
pub const ADMIN_ITEMS: &[&str] = &[
    "extra.fields",
    "extra.automations",
    "extra.templates",
    "extra.teams",
];

fn value_at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, part| v.get(part))
}

fn set_value_at(v: &mut Value, path: &str, new: Option<Value>) {
    let mut parts: Vec<&str> = path.split('.').collect();
    let Some(last) = parts.pop() else { return };
    let mut cur = v;
    for part in parts {
        if !cur.get(part).is_some_and(|x| x.is_object()) {
            if new.is_none() || !cur.is_object() {
                return;
            }
            cur[part] = Value::Object(Map::new());
        }
        cur = &mut cur[part];
    }
    let Some(obj) = cur.as_object_mut() else {
        return;
    };
    match new {
        Some(n) => {
            obj.insert(last.into(), n);
        }
        None => {
            obj.remove(last);
        }
    }
}

fn id_of(v: &Value) -> Option<u64> {
    v.get("id")?.as_u64()
}

fn meta_of(v: &Value) -> SyncMeta {
    v.get("extra")
        .and_then(|e| e.get("sync"))
        .and_then(|s| serde_json::from_value(s.clone()).ok())
        .unwrap_or_default()
}

fn set_meta(v: &mut Value, meta: &SyncMeta) {
    if v.is_object() && !v.get("extra").is_some_and(|e| e.is_object()) {
        v["extra"] = Value::Object(Map::new());
    }
    if let Some(extra) = v.get_mut("extra").and_then(|e| e.as_object_mut()) {
        extra.insert(
            "sync".into(),
            serde_json::to_value(meta).unwrap_or_default(),
        );
    }
}

fn array<'a>(v: &'a Value, path: &str) -> &'a [Value] {
    path.split('.')
        .try_fold(v, |v, part| v.get(part))
        .and_then(|a| a.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

fn array_mut<'a>(v: &'a mut Value, path: &str) -> Option<&'a mut Vec<Value>> {
    let mut cur = Some(v);
    for part in path.split('.') {
        cur = cur.and_then(|v| v.get_mut(part));
    }
    cur.and_then(|a| a.as_array_mut())
}

fn u64_at(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|x| x.as_u64())
}

/// Arrays of items with ids inside an item (a task's comments, a doc's views).
fn is_items(v: &Value) -> bool {
    v.as_array()
        .is_some_and(|a| !a.is_empty() && a.iter().all(|x| id_of(x).is_some()))
}

/// Own fields without `ignore`: the ones a change worth telling about is in.
fn telling_fields(v: &Value, ignore: &[&str]) -> Map<String, Value> {
    let mut m = wb_sync::own_fields(v);
    for k in ignore {
        m.remove(*k);
    }
    m
}

struct Cx {
    cmeta: SyncMeta,
    dmeta: SyncMeta,
    /// fingerprints of every item in the team's copy (what is there)
    cfp: BTreeMap<u64, u64>,
    /// and in theirs, from before it was fixed up
    dfp: BTreeMap<u64, u64>,
    out: PutBack,
    now: i64,
}

impl Cx {
    fn in_team(&self, id: u64) -> bool {
        self.cfp.contains_key(&id)
    }

    fn their(&self, m: &BTreeMap<u64, i64>, id: u64) -> i64 {
        m.get(&id).copied().unwrap_or(0)
    }

    /// When their copy last changed an item's own fields: its stamp or the
    /// newest of its field stamps.
    fn their_change(&self, id: u64) -> i64 {
        let fields = self
            .dmeta
            .fields
            .get(&id)
            .and_then(|f| f.values().max().copied())
            .unwrap_or(0);
        self.their(&self.dmeta.stamps, id).max(fields)
    }

    /// Their change bookkeeping for an item goes, so none of it wins.
    fn forget(&mut self, id: u64) {
        self.dmeta.stamps.remove(&id);
        self.dmeta.fields.remove(&id);
        self.dmeta.moved.remove(&id);
        self.dmeta.revived.remove(&id);
    }

    /// An item they may not change stays the team's: their stamps for it go,
    /// and when they did change it, the team's stamps are moved past theirs
    /// afterwards. `telling`: the change is worth telling them about.
    fn keep(&mut self, id: u64, telling: bool) {
        if !self.in_team(id) {
            self.leave_out(&[id]);
            return;
        }
        let (ds, cs) = (self.their_change(id), self.their(&self.cmeta.stamps, id));
        let (dm, cm) = (
            self.their(&self.dmeta.moved, id),
            self.their(&self.cmeta.moved, id),
        );
        let differs = self
            .dfp
            .get(&id)
            .is_some_and(|h| self.cfp.get(&id) != Some(h));
        if differs && (ds > cs || dm > cm) {
            self.out.bump.insert(id, (ds, dm));
            self.out.refused |= telling;
        }
        self.forget(id);
        self.undelete(id);
    }

    /// A delete they may not make: it doesn't take part in the merge, and
    /// afterwards the item counts as restored right after it (`revive`).
    fn undelete(&mut self, id: u64) {
        let Some(t) = self.dmeta.tombs.get(&id).copied() else {
            return;
        };
        if self.in_team(id) && t > self.their(&self.cmeta.tombs, id) {
            self.dmeta.tombs.remove(&id);
            self.out.revive.insert(id, t);
            self.out.refused = true;
        }
    }

    /// Items in their copy that sit where they may not make them. One the
    /// team has elsewhere moved there and goes back; a new one (or one they
    /// brought back from the trash) gets a tombstone after the merge.
    fn leave_out(&mut self, ids: &[u64]) {
        for &id in ids {
            if self.in_team(id) {
                self.keep(id, true);
                continue;
            }
            let ctomb = self.their(&self.cmeta.tombs, id);
            let dead = ctomb > 0 && self.their(&self.cmeta.revived, id) < ctomb;
            let (ds, dr) = (self.their_change(id), self.their(&self.dmeta.revived, id));
            // a deleted item still in a copy that hasn't heard is no news
            if !dead || dr > ctomb {
                let t = (ds.max(dr).max(ctomb) + 1).max(self.now);
                self.out.bury.insert(id, t);
                self.out.refused = true;
            }
            self.forget(id);
        }
    }

    /// Their copy of a team item they may not change: the team's version,
    /// with the inner items `own` says are theirs to change (their own
    /// comments, their own doc views) as they have them.
    fn keep_item(
        &mut self,
        team: &Value,
        theirs: &Value,
        own: Option<&dyn Fn(&Value) -> bool>,
        ignore: &[&str],
    ) -> Value {
        let id = id_of(team).unwrap_or(0);
        let telling = telling_fields(team, ignore) != telling_fields(theirs, ignore);
        self.keep(id, telling);
        let mut out = team.clone();
        let mine = |x: &Value| own.is_some_and(|f| f(x));
        for (k, v) in theirs.as_object().into_iter().flatten() {
            if !is_items(v) {
                continue;
            }
            for child in v.as_array().into_iter().flatten() {
                let cid = id_of(child).unwrap_or(0);
                let team_child = array(team, k).iter().find(|c| id_of(c) == Some(cid));
                if mine(child) && team_child.is_none_or(mine) {
                    // theirs to change: their version goes in
                    let Some(arr) = out.get_mut(k.as_str()) else {
                        out[k.as_str()] = Value::Array(vec![child.clone()]);
                        continue;
                    };
                    let Some(arr) = arr.as_array_mut() else {
                        continue;
                    };
                    match arr.iter_mut().find(|c| id_of(c) == Some(cid)) {
                        Some(c) => *c = child.clone(),
                        None => arr.push(child.clone()),
                    }
                    continue;
                }
                for inner in wb_sync::ids_within(child) {
                    self.keep(inner, true);
                }
            }
        }
        // inner items they deleted: only their own may go
        for (k, v) in team.as_object().into_iter().flatten() {
            for child in v.as_array().into_iter().flatten() {
                let Some(cid) = id_of(child) else { continue };
                let kept = array(theirs, k).iter().any(|c| id_of(c) == Some(cid));
                if !kept && !mine(child) {
                    for inner in wb_sync::ids_within(child) {
                        self.undelete(inner);
                    }
                }
            }
        }
        out
    }

    /// One kind of top-level item (tasks, docs...) in a space they may not
    /// change: `in_team` says which of the team's sit in it, `in_theirs`
    /// which of theirs claim to.
    fn guard_items(
        &mut self,
        canon: &Value,
        data: &mut Value,
        path: &str,
        in_team: &dyn Fn(&Value) -> bool,
        in_theirs: &dyn Fn(&Value) -> bool,
        own: Option<&dyn Fn(&Value) -> bool>,
        ignore: &[&str],
    ) {
        let all = array(canon, path);
        let team: HashMap<u64, &Value> = all
            .iter()
            .filter(|x| in_team(x))
            .filter_map(|x| id_of(x).map(|i| (i, x)))
            .collect();
        let mut seen = HashSet::new();
        if let Some(arr) = array_mut(data, path) {
            let mut out = Vec::with_capacity(arr.len());
            for item in std::mem::take(arr) {
                let Some(id) = id_of(&item) else {
                    out.push(item);
                    continue;
                };
                seen.insert(id);
                if let Some(t) = team.get(&id) {
                    out.push(self.keep_item(t, &item, own, ignore));
                } else if in_theirs(&item) {
                    match all.iter().find(|x| id_of(x) == Some(id)) {
                        // moved in from where the team has it: it stays there
                        Some(t) => {
                            for inner in wb_sync::ids_within(&item) {
                                self.keep(inner, true);
                            }
                            out.push(t.clone());
                        }
                        None => self.leave_out(&wb_sync::ids_within(&item)),
                    }
                } else {
                    out.push(item);
                }
            }
            *arr = out;
        }
        // the team's items missing from their copy: deletes are undone
        for (id, t) in &team {
            if !seen.contains(id) {
                for inner in wb_sync::ids_within(t) {
                    let own_inner = own.is_some_and(|f| {
                        inner != *id
                            && t.as_object().into_iter().flatten().any(|(_, v)| {
                                v.as_array()
                                    .into_iter()
                                    .flatten()
                                    .any(|c| id_of(c) == Some(inner) && f(c))
                            })
                    });
                    if !own_inner {
                        self.undelete(inner);
                    }
                }
            }
        }
    }
}

impl Cx {
    /// A plain member (not an owner or admin): only the owner and admins
    /// bring people in, take them out or change anyone else's entry, change
    /// the workspace's name, colour and features, and make, change or delete
    /// custom fields, automations, templates and teams. Their own entry
    /// they may change (name, colour, status), but not its role.
    fn guard_people(&mut self, canon: &Value, data: &mut Value, member: u64) {
        let team = array(canon, "members");
        let own = |x: &Value| id_of(x) == Some(member);
        let mut seen = HashSet::new();
        if let Some(arr) = array_mut(data, "members") {
            let mut out = Vec::with_capacity(arr.len());
            for item in std::mem::take(arr) {
                let Some(id) = id_of(&item) else {
                    out.push(item);
                    continue;
                };
                seen.insert(id);
                match team.iter().find(|t| id_of(t) == Some(id)) {
                    Some(t) if own(&item) => {
                        let mut mine = item;
                        // (a copy that leaves the field out changes nothing)
                        if mine.get("role").is_some_and(|r| Some(r) != t.get("role")) {
                            match t.get("role") {
                                Some(r) => mine["role"] = r.clone(),
                                None => {
                                    if let Some(o) = mine.as_object_mut() {
                                        o.remove("role");
                                    }
                                }
                            }
                            // their stamp for it goes; the item may still be
                            // newer (a rename), and then carries the team's
                            // role. Afterwards the team's copy is the newer,
                            // so their app takes the role back.
                            let change = self.their_change(id);
                            if let Some(f) = self.dmeta.fields.get_mut(&id) {
                                f.remove("role");
                            }
                            self.out.bump.insert(id, (change, 0));
                            self.out.refused = true;
                        }
                        out.push(mine);
                    }
                    Some(t) => out.push(self.keep_item(t, &item, None, &[])),
                    // someone new: only the owner and admins invite
                    None => self.leave_out(&[id]),
                }
            }
            *arr = out;
        }
        // the team's people missing from their copy: removing them is undone
        for t in team {
            if let Some(id) = id_of(t) {
                if !seen.contains(&id) {
                    self.undelete(id);
                }
            }
        }
        for path in ADMIN_ITEMS {
            self.guard_items(canon, data, path, &|_| true, &|_| true, None, &[]);
        }
        self.guard_authors(canon, data, member);
        for path in ADMIN_VALUES {
            let theirs = self.dmeta.values.get(*path).copied().unwrap_or(0);
            let team_stamp = self.cmeta.values.get(*path).copied().unwrap_or(0);
            let team_value = value_at(canon, path).cloned();
            if theirs > team_stamp && value_at(data, path).cloned() != team_value {
                self.out.values.insert(path.to_string(), theirs);
                self.out.refused = true;
            }
            if theirs > team_stamp {
                self.dmeta.values.remove(*path);
                set_value_at(data, path, team_value);
            }
        }
    }
}

/// What only a message's author changes: what it says and who said it.
/// Anyone may react to it, pin it or make a task from it.
pub const MESSAGE_KEYS: &[&str] = &[
    "author",
    "text",
    "ts",
    "edited",
    "attachments",
    "post_title",
    "post_type",
    "call",
    "mentions",
    "imported",
];
/// What only a task comment's author changes. Anyone may react to it,
/// resolve it or assign it.
pub const COMMENT_KEYS: &[&str] = &[
    "author", "text", "ts", "edited", "comment", "post", "parent",
];

fn author_of(v: &Value) -> Option<u64> {
    u64_at(v, "author")
}

fn is_comment(v: &Value) -> bool {
    v.get("comment").and_then(|c| c.as_bool()) == Some(true)
}

/// Chat messages, with their thread replies, by id.
fn collect_messages<'a>(msgs: &'a [Value], out: &mut HashMap<u64, &'a Value>) {
    for m in msgs {
        if let Some(id) = id_of(m) {
            out.insert(id, m);
        }
        collect_messages(array(m, "replies"), out);
    }
}

impl Cx {
    /// A plain member: chat messages (with their thread replies) and task
    /// comments belong to whoever wrote them, as the app has it (Edit and
    /// Delete only on one's own). They may react to anyone's, pin it, make
    /// a task from it, resolve or assign a comment, but not write as someone
    /// else, change what someone else wrote, or delete it. History brought
    /// in by an import comes only from an owner or admin (Import from Slack).
    fn guard_authors(&mut self, canon: &Value, data: &mut Value, member: u64) {
        let mut messages = HashMap::new();
        for ch in array(canon, "channels") {
            collect_messages(array(ch, "messages"), &mut messages);
        }
        let comments: HashMap<u64, &Value> = array(canon, "tasks")
            .iter()
            .flat_map(|t| array(t, "activity"))
            .filter(|a| is_comment(a))
            .filter_map(|a| id_of(a).map(|i| (i, a)))
            .collect();
        for ch in array_mut(data, "channels").into_iter().flatten() {
            if let Some(m) = ch.get_mut("messages") {
                self.fix_written(m, &messages, member, true);
            }
        }
        for t in array_mut(data, "tasks").into_iter().flatten() {
            if let Some(a) = t.get_mut("activity") {
                self.fix_written(a, &comments, member, false);
            }
        }
        // deletes of what someone else wrote are undone, unless what it is
        // in goes (a channel or task they may delete, their own message
        // with its thread)
        for ch in array(canon, "channels") {
            if id_of(ch).is_some_and(|c| !self.dmeta.tombs.contains_key(&c)) {
                self.keep_messages(array(ch, "messages"), member);
            }
        }
        for t in array(canon, "tasks") {
            if id_of(t).is_none_or(|i| self.dmeta.tombs.contains_key(&i)) {
                continue;
            }
            let acts: Vec<&Value> = array(t, "activity")
                .iter()
                .filter(|a| is_comment(a))
                .collect();
            // replies after the comments they answer
            for replies in [false, true] {
                for a in &acts {
                    let parent = u64_at(a, "parent");
                    if parent.is_some() != replies || author_of(a) == Some(member) {
                        continue;
                    }
                    if parent.is_some_and(|p| self.dmeta.tombs.contains_key(&p)) {
                        continue;
                    }
                    if let Some(id) = id_of(a) {
                        self.undelete(id);
                    }
                }
            }
        }
    }

    fn keep_messages(&mut self, msgs: &[Value], member: u64) {
        for m in msgs {
            let Some(id) = id_of(m) else { continue };
            if author_of(m) != Some(member) {
                self.undelete(id);
            }
            if !self.dmeta.tombs.contains_key(&id) {
                self.keep_messages(array(m, "replies"), member);
            }
        }
    }

    /// Their messages (`chat`, with replies) or a task's activity: what
    /// someone else wrote keeps its author and words; who wrote one stays
    /// who wrote it; a new one is theirs or it is left out.
    fn fix_written(
        &mut self,
        list: &mut Value,
        team: &HashMap<u64, &Value>,
        member: u64,
        chat: bool,
    ) {
        let Some(arr) = list.as_array_mut() else {
            return;
        };
        let keys = if chat { MESSAGE_KEYS } else { COMMENT_KEYS };
        let mut out = Vec::with_capacity(arr.len());
        for mut e in std::mem::take(arr) {
            let Some(id) = id_of(&e) else {
                // every message has an id: one without can only be made up
                // (task activity got ids from fill_activity_ids)
                if !chat {
                    out.push(e);
                }
                continue;
            };
            match team.get(&id) {
                Some(t) => {
                    let protect: &[&str] = if author_of(t) == Some(member) {
                        &["author"]
                    } else {
                        keys
                    };
                    let change = self.their_change(id);
                    let mut changed = false;
                    for k in protect {
                        if e.get(*k) == t.get(*k) {
                            continue;
                        }
                        // a copy that leaves a field out changes nothing
                        changed |= e.get(*k).is_some();
                        match (t.get(*k), e.as_object_mut()) {
                            (Some(v), Some(o)) => {
                                o.insert(k.to_string(), v.clone());
                            }
                            (None, Some(o)) => {
                                o.remove(*k);
                            }
                            _ => {}
                        }
                        if let Some(f) = self.dmeta.fields.get_mut(&id) {
                            f.remove(*k);
                        }
                    }
                    if changed {
                        self.out.bump.insert(id, (change, 0));
                        self.out.refused = true;
                    }
                }
                None => {
                    let written = chat || is_comment(&e);
                    let imported = e.get("imported").is_some_and(|i| !i.is_null());
                    let forged = written && (author_of(&e) != Some(member) || imported);
                    // (one the team deleted coming back from the Trash is no forgery)
                    if forged && !self.cmeta.tombs.contains_key(&id) {
                        self.leave_out(&wb_sync::ids_within(&e));
                        continue;
                    }
                }
            }
            if chat {
                if let Some(r) = e.get_mut("replies") {
                    self.fix_written(r, team, member, true);
                }
            }
            out.push(e);
        }
        *arr = out;
    }
}

/// Docs in space `sid`, with their sub pages, among `docs`; `seed` are
/// pages already known to be in it.
fn docs_in(docs: &[Value], sid: u64, seed: &HashSet<u64>) -> HashSet<u64> {
    let mut out: HashSet<u64> = seed.clone();
    out.extend(
        docs.iter()
            .filter(|d| u64_at(d, "space") == Some(sid))
            .filter_map(id_of),
    );
    loop {
        let more: Vec<u64> = docs
            .iter()
            .filter(|d| u64_at(d, "parent").is_some_and(|p| out.contains(&p)))
            .filter_map(id_of)
            .filter(|i| !out.contains(i))
            .collect();
        if more.is_empty() {
            return out;
        }
        out.extend(more);
    }
}

/// Fix up a copy from `member` (an owner or admin when `manages`) before it
/// is merged into the team's copy `canon`: see the top of this file.
pub fn guard(canon: &Value, data: &mut Value, member: u64, manages: bool, now: i64) -> PutBack {
    let all_theirs = array(canon, "spaces")
        .iter()
        .all(|s| wb_sync::space_level(s, member, manages) == Some(Level::Full));
    if all_theirs && manages {
        return PutBack::default();
    }
    // (a plain member's comments are checked one by one: see guard_authors)
    if !all_theirs || !manages {
        wb_sync::fill_activity_ids(data);
    }
    let mut cx = Cx {
        cmeta: meta_of(canon),
        dmeta: meta_of(data),
        cfp: wb_sync::fingerprints(canon),
        dfp: wb_sync::fingerprints(data),
        out: PutBack::default(),
        now,
    };
    for s in array(canon, "spaces") {
        let Some(sid) = id_of(s) else { continue };
        let level = wb_sync::space_level(s, member, manages);
        if level == Some(Level::Full) {
            continue;
        }
        let inner = wb_sync::ids_within(s);
        let lists: HashSet<u64> = inner.iter().copied().filter(|i| *i != sid).collect();
        let team_docs = docs_in(array(canon, "docs"), sid, &HashSet::new());
        let their_docs = docs_in(array(data, "docs"), sid, &team_docs);
        let in_lists = |x: &Value| u64_at(x, "list_id").is_some_and(|l| lists.contains(&l));
        let in_space = |x: &Value| u64_at(x, "space") == Some(sid);
        let canon_has = |x: &Value, set: &HashSet<u64>| id_of(x).is_some_and(|i| set.contains(&i));
        if level.is_some_and(|l| l >= Level::Edit) {
            // Edit: everything inside, but not the space's own settings or
            // sharing, and not deleting it
            match array_mut(data, "spaces")
                .and_then(|a| a.iter_mut().find(|x| id_of(x) == Some(sid)))
            {
                Some(ds) => {
                    let telling = wb_sync::own_fields(ds) != wb_sync::own_fields(s);
                    let mut fixed = s.clone();
                    for (k, v) in ds.as_object().into_iter().flatten() {
                        if is_items(v) || v.as_array().is_some_and(|a| a.is_empty()) {
                            fixed[k.as_str()] = v.clone();
                        }
                    }
                    *ds = fixed;
                    cx.keep(sid, telling);
                }
                None if cx.dmeta.tombs.contains_key(&sid) => {
                    // deleting the space takes what's in it along: all of it comes back
                    for id in &inner {
                        cx.undelete(*id);
                    }
                    for (path, inside) in [
                        ("tasks", &in_lists as &dyn Fn(&Value) -> bool),
                        ("docs", &|x: &Value| canon_has(x, &team_docs)),
                        ("extra.dashboards", &in_space),
                        ("extra.forms", &in_lists),
                    ] {
                        for x in array(canon, path).iter().filter(|x| inside(x)) {
                            for id in wb_sync::ids_within(x) {
                                cx.undelete(id);
                            }
                        }
                    }
                }
                None => {}
            }
            continue;
        }
        // View, Comment or closed: the space and everything in it stay the team's
        if let Some(ds) =
            array_mut(data, "spaces").and_then(|a| a.iter_mut().find(|x| id_of(x) == Some(sid)))
        {
            let theirs = std::mem::replace(ds, s.clone());
            let extra: Vec<u64> = wb_sync::ids_within(&theirs)
                .into_iter()
                .filter(|i| !inner.contains(i))
                .collect();
            cx.leave_out(&extra);
        }
        for id in &inner {
            cx.keep(*id, true);
        }
        let comments = level == Some(Level::Comment);
        let reads = level.is_some();
        let own_comment = move |a: &Value| {
            comments
                && a.get("comment").and_then(|c| c.as_bool()) == Some(true)
                && u64_at(a, "author") == Some(member)
        };
        let own_view = move |v: &Value| reads && u64_at(v, "member") == Some(member);
        // a task's `updated` moves with a comment: put back without a word
        cx.guard_items(
            canon,
            data,
            "tasks",
            &in_lists,
            &in_lists,
            Some(&own_comment),
            &["updated"],
        );
        cx.guard_items(
            canon,
            data,
            "docs",
            &|x| canon_has(x, &team_docs),
            &|x| canon_has(x, &their_docs),
            Some(&own_view),
            &[],
        );
        cx.guard_items(
            canon,
            data,
            "extra.dashboards",
            &in_space,
            &in_space,
            None,
            &[],
        );
        cx.guard_items(canon, data, "extra.forms", &in_lists, &in_lists, None, &[]);
    }
    if !manages {
        cx.guard_people(canon, data, member);
    }
    let meta = std::mem::take(&mut cx.dmeta);
    set_meta(data, &meta);
    cx.out
}

/// After merging a copy fixed up by `guard`: the team's stamps for what its
/// sender changed go past theirs, and what they made where they may not
/// gets a tombstone, so their app takes the team's version back.
pub fn put_back(canon: &mut Value, fix: &PutBack) {
    if fix.bump.is_empty() && fix.bury.is_empty() && fix.revive.is_empty() && fix.values.is_empty()
    {
        return;
    }
    let present = wb_sync::fingerprints(canon);
    let mut meta = meta_of(canon);
    for (id, (stamp, moved)) in &fix.bump {
        if !present.contains_key(id) {
            continue;
        }
        // without field stamps every field counts as changed at the item's
        // stamp, so all of the team's version wins over theirs
        let base = meta
            .fields
            .remove(id)
            .and_then(|f| f.values().max().copied())
            .unwrap_or(0);
        let s = meta.stamps.entry(*id).or_insert(0);
        *s = (*s).max(stamp + 1).max(base);
        if *moved > 0 {
            let m = meta.moved.entry(*id).or_insert(0);
            *m = (*m).max(moved + 1);
        }
    }
    for (id, t) in &fix.bury {
        let e = meta.tombs.entry(*id).or_insert(0);
        *e = (*e).max(*t);
    }
    for (id, t) in &fix.revive {
        if !present.contains_key(id) {
            continue;
        }
        let tomb = meta.tombs.entry(*id).or_insert(0);
        *tomb = (*tomb).max(*t);
        let tomb = *tomb;
        let r = meta.revived.entry(*id).or_insert(0);
        *r = (*r).max(tomb + 1);
        let r = *r;
        // a newer stamp too, so an update that carries only what changed
        // (a delta) brings the item back whole to the app that deleted it
        meta.fields.remove(id);
        let s = meta.stamps.entry(*id).or_insert(0);
        *s = (*s).max(r);
    }
    for (path, stamp) in &fix.values {
        let s = meta.values.entry(path.clone()).or_insert(0);
        *s = (*s).max(stamp + 1);
    }
    set_meta(canon, &meta);
}
