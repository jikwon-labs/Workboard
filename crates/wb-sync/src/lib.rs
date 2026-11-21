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
// tasks never overwrite each other. Reactions are merged per (emoji, person),
// running timers per person, time entries per (person, start) and custom
// field values per field, so reacting, starting or stopping a timer or
// setting a custom field at the same moment as someone else keeps both;
// entries without an id in a list of items stay as they are beside the
// items; whiteboard strokes and notes have ids
// (`fill_board_ids`) and merge like any other item, and so do a goal's
// targets and history lines (`fill_goal_ids`): two people changing
// different targets of a goal at once both keep theirs. Deleted items leave a tombstone so they don't come back; restoring
// one from the trash revives it. Task comments and activity lines have ids
// too, so edits, deletes and reactions carry over; lines saved before they
// had ids get one worked out from their contents (`activity_id`). A few
// single settings are shared too (`SHARED_VALUES`, newest change wins), and
// a few maps entry by entry (`SHARED_MAPS`: newest change per key wins).

pub mod json;
pub mod mentions;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

mod copies;
mod delta;
mod hash;
mod ids;
mod merge;
mod meta;
pub mod mine;
mod stamp;
mod visibility;

pub use copies::*;
pub use delta::*;
pub use hash::*;
pub use ids::*;
pub use merge::*;
pub use meta::*;
pub use stamp::*;
pub use visibility::*;

use json::*;

#[cfg(test)]
mod tests;
