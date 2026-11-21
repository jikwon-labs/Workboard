// Data model for the workspace: spaces, folders, lists, tasks and members.
// Default statuses and priorities follow the values recorded in MECHANICS.md.
//
// Here: the workspace (`Data`) and the thread that writes it to disk. In
// model/: the shapes (types.rs, task.rs, extra.rs), a task's history lines
// (history.rs), loading and saving
// (persist.rs), lookups (query.rs) and the example workspace (demo.rs). All of
// it is reachable as `crate::model::*`.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod demo;
mod extra;
mod history;
mod persist;
mod query;
mod task;
mod types;

pub use extra::*;
pub use task::*;
pub use types::*;

/// Where `Extra::slack_map` sits in the saved workspace (one of
/// `sync::SHARED_MAPS`).
pub const SLACK_MAP: &str = "extra.slack_map";

/// See `Data::take_unreadable`.
static UNREADABLE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

/// Writes workspace files off the screen's thread. Saving a large workspace
/// (turning it into text and writing it) takes long enough to be felt when
/// it happens on every key press, so the app hands a copy to this writer
/// instead and goes on drawing. Writes to one file keep their order: a newer
/// copy waiting for the same file replaces the older one.
mod writer {
    use super::Data;
    use std::path::{Path, PathBuf};
    use std::sync::{Condvar, Mutex, OnceLock};

    #[derive(Default)]
    struct State {
        waiting: Vec<(PathBuf, Data)>,
        busy: bool,
    }

    struct Writer {
        state: Mutex<State>,
        changed: Condvar,
    }

    static WRITER: OnceLock<&'static Writer> = OnceLock::new();

    fn writer() -> &'static Writer {
        WRITER.get_or_init(|| {
            let w: &'static Writer = Box::leak(Box::new(Writer {
                state: Mutex::new(State::default()),
                changed: Condvar::new(),
            }));
            std::thread::Builder::new()
                .name("workboard-save".into())
                .spawn(move || run(w))
                .expect("start the save thread");
            w
        })
    }

    fn run(w: &'static Writer) {
        loop {
            let batch = {
                let mut g = w.state.lock().unwrap_or_else(|e| e.into_inner());
                while g.waiting.is_empty() {
                    g = w.changed.wait(g).unwrap_or_else(|e| e.into_inner());
                }
                g.busy = true;
                std::mem::take(&mut g.waiting)
            };
            for (p, d) in batch {
                write(&d, &p);
            }
            w.state.lock().unwrap_or_else(|e| e.into_inner()).busy = false;
            w.changed.notify_all();
        }
    }

    pub(super) fn queue(p: PathBuf, d: Data) {
        let w = writer();
        let mut g = w.state.lock().unwrap_or_else(|e| e.into_inner());
        match g.waiting.iter_mut().find(|(q, _)| *q == p) {
            Some(slot) => slot.1 = d,
            None => g.waiting.push((p, d)),
        }
        drop(g);
        w.changed.notify_all();
    }

    /// Wait until nothing is queued or being written (without starting the
    /// writer when nothing ever was).
    pub(super) fn flush() {
        let Some(w) = WRITER.get() else {
            return;
        };
        let mut g = w.state.lock().unwrap_or_else(|e| e.into_inner());
        while !g.waiting.is_empty() || g.busy {
            g = w.changed.wait(g).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// The whole file at once: written beside it, then moved over it, so a
    /// crash midway leaves the last whole copy. Compact text (no
    /// indentation) is smaller and quicker to write and to read back, and
    /// this computer's sync fingerprints are left out when opening the file
    /// can work them out again (`leave_out_hashes`).
    pub(super) fn write(d: &Data, p: &Path) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(mut v) = serde_json::to_value(d) else {
            return;
        };
        crate::sync::leave_out_hashes(&mut v);
        if let Ok(s) = serde_json::to_vec(&v) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, p);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Data {
    pub workspace: String,
    pub spaces: Vec<Space>,
    pub tasks: Vec<Task>,
    pub members: Vec<Member>,
    pub me: u64,
    pub next_id: u64,
    pub favorites: Vec<Location>,
    #[serde(default)]
    pub dark: bool,
    #[serde(default)]
    pub docs: Vec<Doc>,
    #[serde(default)]
    pub channels: Vec<Channel>,
    #[serde(default)]
    pub extra: Extra,
}

pub fn now_ts() -> i64 {
    Local::now().timestamp()
}

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

impl Data {
    pub fn new_id(&mut self) -> u64 {
        if let Some(tag) = self.extra.shared.as_ref().map(|s| s.tag as u64 % 1024) {
            // shared: ids end in this computer's tag, so nobody else makes the same one
            let ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
            let mut id = ms * 1024 + tag;
            if id <= self.next_id {
                id = (self.next_id / 1024 + 1) * 1024 + tag;
            }
            self.next_id = id;
            return id;
        }
        self.next_id += 1;
        self.next_id
    }
}
