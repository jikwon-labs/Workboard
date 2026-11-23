// Reading and guarding the team's copy: members, roles shown in People,
// the copy each member gets, and what a member may change.

use super::*;

pub(crate) fn invite_alive(i: &InviteRec, now: i64) -> bool {
    now - i.created < INVITE_DAYS * 86_400_000
}

pub(crate) fn member_ids(canon: Option<&Value>) -> Vec<u64> {
    canon
        .and_then(|c| c.get("members"))
        .and_then(|m| m.as_array())
        .map(|a| a.iter().filter_map(|m| m.get("id")?.as_u64()).collect())
        .unwrap_or_default()
}

/// A member's entry in the team's copy.
pub(crate) fn member_record(canon: Option<&Value>, member: u64) -> Option<&Value> {
    canon
        .and_then(|c| c.get("members"))
        .and_then(|m| m.as_array())
        .and_then(|a| {
            a.iter()
                .find(|m| m.get("id").and_then(|i| i.as_u64()) == Some(member))
        })
}

/// A text field of a member in the team's copy ("name", "email"), trimmed.
pub(crate) fn member_field(canon: Option<&Value>, member: u64, field: &str) -> String {
    member_record(canon, member)
        .and_then(|m| m.get(field))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The name others see a member by, as the apps show it: their display
/// name when they set one, else their full name.
pub(crate) fn shown_name(canon: Option<&Value>, member: u64) -> String {
    member_record(canon, member)
        .map(wb_sync::json::shown_name)
        .unwrap_or_default()
}

pub(crate) fn member_exists(canon: Option<&Value>, member: u64) -> bool {
    member_record(canon, member).is_some()
}

/// Reserve even orphaned references and sync tombstones: reusing an old
/// identity must not attach the new teammate to somebody else's data.
/// Numeric values/keys are conservatively reserved, including unknown fields.
pub(crate) fn fresh_member_id(canon: &Value, saved: &Saved, start: u64) -> u64 {
    pub(crate) fn reserve(value: &Value, ids: &mut std::collections::BTreeSet<u64>) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    if let Ok(id) = key.parse::<u64>() {
                        ids.insert(id);
                    }
                    reserve(value, ids);
                }
            }
            Value::Array(array) => {
                for value in array {
                    reserve(value, ids);
                }
            }
            Value::Number(n) => {
                if let Some(id) = n.as_u64() {
                    ids.insert(id);
                }
            }
            _ => {}
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    reserve(canon, &mut ids);
    ids.extend(saved.roles.keys().copied());
    ids.extend(saved.accounts.keys().copied());
    ids.extend(saved.devices.values().map(|d| d.member));
    ids.extend(saved.invites.values().map(|i| i.member));
    const MASK: u64 = (1u64 << 62) - 1;
    let mut id = start & MASK;
    while id == 0 || ids.contains(&id) {
        id = (id + 1) & MASK;
    }
    id
}

/// Make every item in a restored copy win over what the apps have, and bring
/// back the items deleted since: their change stamps become `now` and each
/// one counts as restored.
pub fn mark_restored(data: &mut Value, now: i64) {
    if !copy_shape_ok(data) {
        return;
    }
    let ids: Vec<u64> = wb_sync::fingerprints(data).into_keys().collect();
    let mut meta = wb_sync::json::meta_of(data);
    for id in ids {
        meta.stamps.insert(id, now);
        meta.revived.insert(id, now);
    }
    // every field of every item counts as changed now
    meta.fields.clear();
    meta.hashes.clear();
    meta.field_hashes.clear();
    wb_sync::json::set_meta(data, &meta);
}

/// Write a member's role into the team's copy so everyone's app shows it.
pub(crate) fn label_role(canon: &mut Value, member: u64, role: Role) -> bool {
    set_member_field(canon, member, "role", json!(role.label()))
}

/// Someone invited signed in: they're no longer shown as invited, wherever
/// they joined from (the iPhone app doesn't take it off their entry itself).
pub(crate) fn still_invited(canon: Option<&Value>, member: u64) -> bool {
    member_record(canon, member).and_then(|m| m.get("invited")) == Some(&Value::Bool(true))
}

/// Take `invited` off a member's entry in the team's copy.
pub(crate) fn mark_joined(canon: &mut Value, member: u64) -> bool {
    still_invited(Some(canon), member) && set_member_field(canon, member, "invited", json!(false))
}

/// Set one field of a member's entry in the team's copy, stamped so it wins
/// over the apps' older copies of it.
fn set_member_field(canon: &mut Value, member: u64, field: &str, value: Value) -> bool {
    if !copy_shape_ok(canon) {
        return false;
    }
    let Some(m) = canon
        .get_mut("members")
        .and_then(|m| m.as_array_mut())
        .and_then(|a| {
            a.iter_mut()
                .find(|m| m.get("id").and_then(|i| i.as_u64()) == Some(member))
        })
    else {
        return false;
    };
    if m.get(field) == Some(&value) {
        return false;
    }
    m[field] = value;
    let sync = canon.get("extra").and_then(|e| e.get("sync"));
    let meta: Option<wb_sync::SyncMeta> = match sync {
        None | Some(Value::Null) => Some(Default::default()),
        Some(s) => serde_json::from_value(s.clone()).ok(),
    };
    if let Some(mut meta) = meta {
        // only this field changed, so a rename made meanwhile still counts
        wb_sync::touch_fields(&mut meta, member, &[field], now_ms());
        wb_sync::json::set_meta(canon, &meta);
        return true;
    }
    let stamps = &mut canon["extra"]["sync"]["stamps"];
    if !stamps.is_object() {
        *stamps = json!({});
    }
    let prev = stamps
        .get(member.to_string())
        .and_then(|s| s.as_i64())
        .unwrap_or(0);
    stamps[member.to_string()] = json!(now_ms().max(prev + 1));
    true
}

/// A workspace copy the server can work with: an object whose `extra`,
/// `extra.sync` and `extra.sync.stamps` are objects (or missing), so writing
/// into them can't fail.
pub(crate) fn copy_shape_ok(v: &Value) -> bool {
    let fine = |x: Option<&Value>| x.is_none_or(|x| x.is_null() || x.is_object());
    let extra = v.get("extra");
    let sync = extra.and_then(|e| e.get("sync"));
    v.is_object() && fine(extra) && fine(sync) && fine(sync.and_then(|s| s.get("stamps")))
}

pub(crate) fn drop_ids(meta: Option<&mut Value>, keys: &[&str], ids: &[u64]) {
    let Some(meta) = meta else { return };
    for k in keys {
        if let Some(m) = meta.get_mut(*k).and_then(|m| m.as_object_mut()) {
            for id in ids {
                m.remove(&id.to_string());
            }
        }
    }
}

/// What `member` (an owner or admin when `manages`) gets of the team's copy,
/// and the ids of the items left out of it: DMs and private channels they
/// aren't in, spaces closed to them (other people's Personal spaces, spaces
/// shared with chosen people without them) with what's in them, and other
/// people's private docs (see `wb_sync::hidden_from`),
/// with their change stamps. None when nothing is left out. Tombstones stay,
/// so deletes still reach everyone; an app never treats an item missing here
/// as deleted (only a tombstone deletes), and the ids tell it to take its
/// own copy of them away without deleting them (`wb_sync::hide`).
pub(crate) fn copy_for(canon: &Value, member: u64, manages: bool) -> Option<(Value, Vec<u64>)> {
    let hidden = wb_sync::hidden_from(canon, member, manages);
    if hidden.is_empty() {
        return None;
    }
    let mut out = canon.clone();
    wb_sync::hide(&mut out, &hidden);
    Some((out, hidden))
}

/// What a connection remembers was left out of the copy it last sent: the
/// hidden top-level items and everything inside them (a folder in a space
/// closed to its person, with its lists), so whatever of it they may see
/// later reaches them whole (see `Canon::delta_for`).
/// (`wb_sync::ids_inside` without copying or fingerprinting what it walks:
/// a hidden channel holds all its messages.)
pub(crate) fn left_out(canon: &Value, hidden: &[u64]) -> Vec<u64> {
    fn items(v: &Value) -> Option<&Vec<Value>> {
        // as wb_sync::is_id_array: objects, at least one with an id
        let a = v.as_array()?;
        (!a.is_empty()
            && a.iter().all(|x| x.is_object())
            && a.iter().any(|x| wb_sync::json::id_of(x).is_some()))
        .then_some(a)
    }
    fn walk(a: &[Value], out: &mut std::collections::BTreeSet<u64>) {
        for item in a {
            let (Some(id), Some(obj)) = (wb_sync::json::id_of(item), item.as_object()) else {
                continue;
            };
            out.insert(id);
            for child in obj.values() {
                if let Some(inner) = items(child) {
                    walk(inner, out);
                }
            }
        }
    }
    if hidden.is_empty() {
        return vec![];
    }
    let wanted: std::collections::HashSet<u64> = hidden.iter().copied().collect();
    let mut out = std::collections::BTreeSet::new();
    for path in wb_sync::SHARED {
        let top = wb_sync::json::array_at(canon, path);
        for item in top
            .iter()
            .filter(|x| wb_sync::json::id_of(x).is_some_and(|i| wanted.contains(&i)))
        {
            walk(std::slice::from_ref(item), &mut out);
        }
    }
    out.into_iter().collect()
}

/// Whether `member` may delete a channel and change who else is in it: the
/// app's rule (`may_manage_channel`). Whoever made it, an owner or admin, or
/// for one made before makers were kept (creator 0) anyone in it.
pub fn may_manage_channel(ch: &Value, member: u64, role: Role) -> bool {
    let creator = ch.get("creator").and_then(|c| c.as_u64()).unwrap_or(0);
    let in_it = ch
        .get("members")
        .and_then(|m| m.as_array())
        .is_some_and(|a| a.iter().any(|m| m.as_u64() == Some(member)));
    role.manages() || creator == member || (creator == 0 && in_it)
}

/// Before merging a copy from `member`, for the team's channels: who made
/// one stays (an older app or the iPhone may drop the field); and unless
/// they may manage it (`may_manage_channel`), they can't delete it (their
/// tombstone for it is dropped), make it private or a DM, turn its
/// `default_channel` on or off, or add or remove anyone but themselves (its
/// member list stays the team's, with only their own place changed).
pub(crate) fn guard_channels(canon: &Value, data: &mut Value, member: u64, role: Role) {
    let Some(team) = canon.get("channels").and_then(|c| c.as_array()) else {
        return;
    };
    // (a push of only what changed may hold a delete without the channel)
    let no_delete: Vec<u64> = team
        .iter()
        .filter(|t| !may_manage_channel(t, member, role))
        .filter_map(|t| t.get("id").and_then(|i| i.as_u64()))
        .collect();
    if let Some(mine) = data.get_mut("channels").and_then(|c| c.as_array_mut()) {
        for t in team {
            let Some(id) = t.get("id").and_then(|i| i.as_u64()) else {
                continue;
            };
            let manages = !no_delete.contains(&id);
            let Some(c) = mine
                .iter_mut()
                .find(|c| c.get("id").and_then(|i| i.as_u64()) == Some(id))
            else {
                continue;
            };
            let Some(c) = c.as_object_mut() else { continue };
            match t.get("creator") {
                Some(k) if k.as_u64().is_some_and(|k| k != 0) => {
                    c.insert("creator".into(), k.clone());
                }
                _ => {}
            }
            // a DM or private channel whose members all left would be open
            // to everyone (an empty list means no list: see closed_to), with
            // all its history: its last member stays in it
            let restricted = t.get("kind").and_then(|k| k.as_str()) == Some("Direct")
                || t.get("private").and_then(|p| p.as_bool()) == Some(true);
            let had = t
                .get("members")
                .and_then(|m| m.as_array())
                .is_some_and(|a| !a.is_empty());
            let emptied = c
                .get("members")
                .and_then(|m| m.as_array())
                .is_some_and(|a| a.is_empty());
            if restricted && had && emptied {
                if let Some(m) = t.get("members") {
                    c.insert("members".into(), m.clone());
                }
            }
            if manages {
                continue;
            }
            // whether it's private (or a DM) decides who sees it at all:
            // the app makes a channel private only when it is made. And
            // whether someone joining is announced in it (`joins.rs`) is
            // for whoever manages it, as who else is in it
            for k in ["private", "kind", "default_channel"] {
                match t.get(k) {
                    Some(v) => {
                        c.insert(k.into(), v.clone());
                    }
                    None => {
                        c.remove(k);
                    }
                }
            }
            let ids = |v: Option<&Value>| -> Vec<u64> {
                v.and_then(|m| m.as_array())
                    .map(|a| a.iter().filter_map(|m| m.as_u64()).collect())
                    .unwrap_or_default()
            };
            let theirs = ids(c.get("members"));
            let mut allowed: Vec<u64> = ids(t.get("members"))
                .into_iter()
                .filter(|m| *m != member)
                .collect();
            if theirs.contains(&member) || (restricted && had && allowed.is_empty()) {
                allowed.push(member);
            }
            let (mut a, mut b) = (allowed.clone(), theirs);
            a.sort_unstable();
            b.sort_unstable();
            if a != b {
                c.insert("members".into(), json!(allowed));
            }
        }
    }
    drop_ids(
        data.get_mut("extra").and_then(|e| e.get_mut("sync")),
        &["tombs"],
        &no_delete,
    );
}

/// Before merging a copy from `member`: the items hidden from them stay as
/// the team has them. Their version of one (say, a private channel with
/// themselves added to its members) is swapped for the team's, and their
/// stamps, tombstones, revivals and moves for anything in it are dropped,
/// so they can't change, join or delete it. Before that, what they may not
/// change in spaces (shared with them, or closed to them) is put back (see
/// spaces.rs); the result says what to do to the team's copy after the merge.
pub(crate) fn guard_push(
    canon: &Value,
    data: &mut Value,
    member: u64,
    saved: &Saved,
) -> spaces::PutBack {
    let role = saved.role_of(member);
    guard_channels(canon, data, member, role);
    // an admin manages people, but the owner's entry stays the owner's
    let owners: Vec<u64> = match role {
        Role::Admin => saved
            .roles
            .iter()
            .filter(|(m, r)| **r == Role::Owner && **m != member)
            .map(|(m, _)| *m)
            .collect(),
        _ => vec![],
    };
    let fix = spaces::guard(
        canon,
        data,
        member,
        role.manages(),
        &owners,
        now_ms(),
        &saved.graves,
    );
    swap_hidden(canon, data, member, role);
    fix
}

pub(crate) fn swap_hidden(canon: &Value, data: &mut Value, member: u64, role: Role) {
    let hidden = wb_sync::hidden_from(canon, member, role.manages());
    if hidden.is_empty() {
        return;
    }
    let items: Vec<Value> = wb_sync::SHARED
        .iter()
        .flat_map(|path| wb_sync::json::array_at(canon, path))
        .filter(|x| {
            x.get("id")
                .and_then(|i| i.as_u64())
                .is_some_and(|i| hidden.contains(&i))
        })
        .cloned()
        .collect();
    let ids: Vec<u64> = wb_sync::fingerprints(&json!({ "channels": items }))
        .into_keys()
        .collect();
    for item in &items {
        let id = item.get("id").and_then(|i| i.as_u64());
        for path in wb_sync::SHARED {
            if let Some(x) = wb_sync::json::array_at_mut(data, path).and_then(|a| {
                a.iter_mut()
                    .find(|c| c.get("id").and_then(|i| i.as_u64()) == id)
            }) {
                *x = item.clone();
            }
        }
    }
    drop_ids(
        data.get_mut("extra").and_then(|e| e.get_mut("sync")),
        &["stamps", "tombs", "revived", "moved", "fields"],
        &ids,
    );
}
