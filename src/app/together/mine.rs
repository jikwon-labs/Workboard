// My reading state and notification choices kept in step with my other
// devices (an iPhone, another Mac) through the team server: where I've read
// up to in each conversation, Inbox rows moved to Later or Cleared, replies
// marked read, and my notification settings (`sync::mine`).

use super::*;
use crate::sync::mine::{self, MineView};

/// How often (seconds) my reading state is looked at for changes to send.
const MINE_EVERY: f64 = 1.0;

impl App {
    /// What this computer has, as `sync::mine` takes it.
    pub(crate) fn mine_view(&self) -> MineView {
        let e = &self.data.extra;
        let mut v = MineView {
            replies_seen: e.replies_seen,
            inbox_cleared: e.inbox_cleared.iter().cloned().collect(),
            inbox_later: e.inbox_later.iter().cloned().collect(),
            replies_read: e.replies_read.iter().cloned().collect(),
            ..Default::default()
        };
        for c in &self.data.channels {
            if c.last_read > 0 {
                v.read.insert(c.id, c.last_read);
            }
        }
        let notify = serde_json::to_value(&e.notify).unwrap_or_default();
        v.settings
            .insert("notify".into(), mine::setting_value("notify", &notify));
        v.settings
            .insert("muted_until".into(), serde_json::json!(e.muted_until));
        v
    }

    /// A fingerprint of what `mine_view` is made of, worked out without
    /// copying any of it: when it is the same as last time, nothing changed
    /// to send.
    fn mine_fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        /// Feeds JSON text straight into the hasher.
        struct Feed<'a>(&'a mut std::collections::hash_map::DefaultHasher);
        impl std::io::Write for Feed<'_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.write(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let e = &self.data.extra;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for c in &self.data.channels {
            (c.id, c.last_read).hash(&mut h);
        }
        e.replies_seen.hash(&mut h);
        e.inbox_cleared.hash(&mut h);
        e.inbox_later.hash(&mut h);
        e.replies_read.hash(&mut h);
        e.muted_until.hash(&mut h);
        // what my devices agreed on, started over (a workspace joined)
        e.mine_sync.is_null().hash(&mut h);
        let _ = serde_json::to_writer(Feed(&mut h), &e.notify);
        h.finish()
    }

    /// Once a second while connected: send my reading state when it
    /// changed here.
    pub(crate) fn mine_tick(&mut self, now: f64) {
        if !self.sync_rt.mine_ok || now - self.sync_rt.mine_checked < MINE_EVERY {
            return;
        }
        self.sync_rt.mine_checked = now;
        let print = self.mine_fingerprint();
        if self.sync_rt.mine_print == Some(print) {
            return;
        }
        self.send_mine();
        self.sync_rt.mine_print = Some(print);
    }

    /// Send what changed here since my devices last agreed.
    pub(crate) fn send_mine(&mut self) {
        let Some(net) = &self.sync_rt.net else {
            return;
        };
        let known = &self.data.extra.mine_sync;
        let out = mine::collect(
            &self.mine_view(),
            known,
            chrono::Utc::now().timestamp_millis(),
        );
        if &out == known {
            return;
        }
        // only what changed: the server merges entry by entry (and has the
        // rest, or gets it back after the next welcome; see take_mine)
        net.send(NetMsg::Mine {
            data: mine::changes(&out, known),
        });
        self.data.extra.mine_sync = out;
        self.save_soon();
    }

    /// The server sent what changed in my reading state (`mine_part`): the
    /// rest is as my devices last agreed, so the whole of it is that with
    /// this in it.
    pub(crate) fn take_mine_part(&mut self, part: serde_json::Value) {
        // on a tie the server's stays
        let mut theirs = part;
        mine::merge(&mut theirs, &self.data.extra.mine_sync);
        self.take_mine(theirs);
    }

    /// The server sent my reading state (after welcome, and when another
    /// device of mine changed it): take it in, and send back what this
    /// computer has that is newer.
    pub(crate) fn take_mine(&mut self, theirs: serde_json::Value) {
        self.sync_rt.mine_ok = true;
        let ours = mine::collect(
            &self.mine_view(),
            &self.data.extra.mine_sync,
            chrono::Utc::now().timestamp_millis(),
        );
        // on a tie the server's stays
        let mut merged = theirs.clone();
        mine::merge(&mut merged, &ours);
        let changed = self.apply_mine(&mine::resolve(&merged));
        let newer_here = merged != theirs;
        self.data.extra.mine_sync = merged.clone();
        if newer_here {
            if let Some(net) = &self.sync_rt.net {
                // what the server lacks: also what a send that never
                // arrived (the connection broke) had
                net.send(NetMsg::Mine {
                    data: mine::changes(&merged, &theirs),
                });
            }
        }
        // what the next look compares against moved: look again
        self.sync_rt.mine_print = None;
        if changed {
            // what is worked out from it (the Inbox rows) is worked out
            // again: rows can move without any count changing
            self.data_changed();
            self.notify_rt.pending = true;
        }
        self.save_soon();
    }

    /// Take a merged view into this workspace. True when anything changed.
    pub(crate) fn apply_mine(&mut self, v: &MineView) -> bool {
        let mut changed = false;
        for c in self.data.channels.iter_mut() {
            let Some(&read) = v.read.get(&c.id) else {
                continue;
            };
            if read == c.last_read {
                continue;
            }
            let lower = read < c.last_read;
            c.last_read = read;
            // read on another device: what's up to there is seen here too;
            // marked unread there: what's after it is new here again
            if let Some(seen) = self.data.extra.chat_seen.get(&c.id) {
                let now_seen = SeenIds::of(
                    c.messages
                        .iter()
                        .filter(|m| m.ts <= read || (!lower && seen.has(m.id)))
                        .map(|m| (m.id, m.ts)),
                );
                self.data.extra.chat_seen.insert(c.id, now_seen);
            }
            changed = true;
        }
        let e = &mut self.data.extra;
        if v.replies_seen > e.replies_seen {
            e.replies_seen = v.replies_seen;
            changed = true;
        }
        for (list, want) in [
            (&mut e.inbox_cleared, &v.inbox_cleared),
            (&mut e.inbox_later, &v.inbox_later),
            (&mut e.replies_read, &v.replies_read),
        ] {
            let have: std::collections::BTreeSet<String> = list.iter().cloned().collect();
            if &have != want {
                *list = mine::keep_order(list, want);
                changed = true;
            }
        }
        if let Some(n) = v.settings.get("notify") {
            let mut n = n.clone();
            if let Some(o) = n.as_object_mut() {
                // what stays on this computer
                for k in mine::NOTIFY_LOCAL {
                    let here = serde_json::to_value(&e.notify)
                        .ok()
                        .and_then(|h| h.get(*k).cloned());
                    if let Some(h) = here {
                        o.insert(k.to_string(), h);
                    }
                }
            }
            if let Ok(n) = serde_json::from_value::<crate::model::NotifySettings>(n) {
                if serde_json::to_value(&n).ok() != serde_json::to_value(&e.notify).ok() {
                    e.notify = n;
                    changed = true;
                }
            }
        }
        if let Some(m) = v.settings.get("muted_until").and_then(|m| m.as_i64()) {
            if m != e.muted_until {
                e.muted_until = m;
                changed = true;
            }
        }
        changed
    }
}
