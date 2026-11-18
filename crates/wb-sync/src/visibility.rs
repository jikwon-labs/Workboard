// What each person may see: personal settings left out of shared copies,
// private channels and spaces, and hiding what a member may not see.

use super::*;

/// Each person's own settings and reading state in `extra`, left out of the
/// copy other people get: what they've read and closed in chat, starred and
/// recent items, notification and sidebar settings, their timesheet rows,
/// the Slack people they import under their own name and where their copy
/// is shared. None of it is merged from other copies, and
/// the app fills in its default when one is missing.
pub(crate) const PERSONAL_EXTRA: &[&str] = &[
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
    "slack_under_me",
    "slack_not_under_me",
    "slack_carried",
];

/// The copy other people get: personal notes, reminders, settings and
/// reading state (`PERSONAL_EXTRA`), items marked `personal` and this
/// computer's fingerprints left out.
pub fn shareable(data: &Value) -> Value {
    let mut v = data.clone();
    make_shareable(&mut v);
    v
}

/// `shareable` in place, for a copy that is changed anyway (the server's
/// copy after a merge): no second copy of the whole workspace.
pub fn make_shareable(v: &mut Value) {
    // items marked personal (bookmarks a person had before bookmarks were
    // shared) stay on this computer
    for path in SHARED {
        if let Some(a) = get_path_mut(v, path).and_then(|a| a.as_array_mut()) {
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
        if LOCAL_KEYS.contains(&k.as_str()) || v.as_array().is_some_and(|a| a.is_empty()) {
            continue;
        }
        if is_id_array(v) {
            let loose = loose_entries(v);
            if !loose.is_empty() {
                own.insert(k.clone(), Value::Array(loose));
            }
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

/// A workspace made of `team`'s shared things (items, shared settings and
/// their change bookkeeping, the id counter) and `personal`'s own settings
/// for everything else: what joining a team workspace opens.
pub fn with_team_things(personal: &Value, team: &Value) -> Value {
    let mut v = personal.clone();
    for path in SHARED
        .iter()
        .chain(SHARED_VALUES)
        .chain(SHARED_MAPS)
        .chain(&["extra.sync", "next_id"])
    {
        if let Some(x) = get_path(team, path) {
            set_path(&mut v, path, x.clone());
        }
    }
    v
}
