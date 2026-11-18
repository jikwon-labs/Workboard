// What is shared and how a copy remembers its changes: the shared
// collections, settings and maps, the shared-folder settings and SyncMeta.

use super::*;

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

/// Shared maps whose entries each belong to the whole team on their own:
/// `SyncMeta::values` keeps a stamp per entry ("<path>/<key>", see
/// `map_part`) and for each key the newest change wins, so two people
/// setting different keys at once both keep theirs, and the later of two
/// settings of one key wins everywhere. An entry is taken away by leaving
/// it out (its stamp stays as when that happened). `extra.slack_map`: Slack
/// user id -> the member an Import from Slack brings them in as.
///
/// No `META_V` bump: no fingerprint or field stamp of anything older apps
/// know changes. Older apps don't know these maps; they never merge or send
/// them (the Mac app drops the field when it reads a file), and a copy of
/// theirs that kept an entry's stamp but not the entry loses the tie to a
/// copy with the entry, so they can't take entries away or overwrite them.
pub const SHARED_MAPS: &[&str] = &["extra.slack_map"];

/// The name of one entry of a shared map in `SyncMeta::values` and
/// `value_hashes` ("extra.slack_map/U123").
pub fn map_part(path: &str, key: &str) -> String {
    format!("{}/{}", path, key)
}

/// The shared map and key an entry name stands for (see `map_part`).
pub(crate) fn map_entry(part: &str) -> Option<(&'static str, &str)> {
    SHARED_MAPS
        .iter()
        .find_map(|p| Some((*p, part.strip_prefix(p)?.strip_prefix('/')?)))
}

pub(crate) fn json_hash(v: &Value) -> u64 {
    fnv(serde_json::to_string(v).unwrap_or_default().as_bytes())
}

/// Fields that are each person's own even inside shared items.
pub(crate) const LOCAL_KEYS: &[&str] = &["last_read", "saved", "expanded"];

/// Fields the app leaves out of the file when they are at their default and
/// that can go back to it (a stopped timer, an unresolved comment, the last
/// watcher removed). When the newest copy of an item comes from an app that
/// knows them (`SyncMeta.v` 2 and later) and lacks one, it is at its default
/// there and is not filled back in from an older copy. Any other missing
/// field is kept from the other copies, so an app that doesn't know a field
/// can't erase it. A new field like these goes here with a `META_V` bump.
pub(crate) const DEFAULT_WHEN_MISSING: &[&str] = &[
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
pub(crate) const APPEND_KEYS: &[&str] = &["activity"];

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
    /// app's `legacy_shape`); 3 = also `fields` is kept; 4 = time entries
    /// and custom field values are each a field of their own in `fields`
    /// ("time_entries/<member>/<start>", "custom/<field id>"), not one for
    /// the whole list or map.
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
    /// Shared setting (see `SHARED_VALUES`), or entry of a shared map
    /// (`SHARED_MAPS`, "<path>/<key>") -> when it last changed.
    pub values: BTreeMap<String, i64>,
    /// Shared setting or map entry -> fingerprint when last saved (this
    /// computer only).
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
    /// Set in a saved file that left out this computer's fingerprints
    /// (`hashes`, `field_hashes`, `parents`, `value_hashes`) because they
    /// were exactly what the file's content gives: opening it works them
    /// out again (see `refill_hashes`). Never set in memory.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rehash: bool,
}
