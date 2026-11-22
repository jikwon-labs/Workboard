// What views work out from the workspace on every frame (unread counts,
// which direct messages are copies of each other), kept until the workspace
// changes. Kept under the Inbox's key (`nav::InboxKey`: `data_changed`, plus
// counts as a check for a change that didn't say so) and, for what depends on
// reading, a fingerprint of the read marks.

use super::*;
use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// The read marks unread counts depend on: every channel's `last_read` and
/// the ids kept as seen (`chat_seen`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct SeenKey(u64);

/// Direct messages with the same people and the unread messages of each
/// channel, worked out once for all channels.
#[derive(Default)]
pub(super) struct ChatIndex {
    /// Where each channel is in `data.channels`, by id.
    at: HashMap<u64, usize>,
    /// A direct message's copies (see `App::conversation`), as places in
    /// `data.channels`: itself first. Only for direct messages that show.
    copies: HashMap<u64, Vec<usize>>,
    /// Second copies of a direct message (`App::dm_duplicate`).
    duplicate: HashSet<u64>,
    /// The copy that shows, for each direct message (`App::dm_primary`).
    primary: HashMap<u64, u64>,
    /// Messages from others not seen, by channel (`App::unread_here`).
    unread: HashMap<u64, usize>,
}

#[derive(Default)]
pub(super) struct Kept {
    /// Inside a pass of `ui_frame`: its number plus one; 0 outside, when
    /// nothing is remembered from one call to the next.
    pass: Cell<u64>,
    seen: Cell<Option<(u64, u64, SeenKey)>>,
    chat: RefCell<Option<(nav::InboxKey, SeenKey, Rc<ChatIndex>)>>,
}

impl Kept {
    pub(super) fn start_pass(&self, nr: u64) {
        self.pass.set(nr.wrapping_add(1));
    }

    pub(super) fn end_pass(&self) {
        self.pass.set(0);
    }

    /// The read marks changed (`App::mark_read`): worked out again even in
    /// the same pass.
    pub(super) fn forget_seen(&self) {
        self.seen.set(None);
    }

    /// The pass this is in, if it's in one.
    pub(super) fn pass(&self) -> Option<u64> {
        Some(self.pass.get()).filter(|p| *p != 0)
    }
}

/// Members of a direct message, sorted, as `chat::dm_people` has them.
fn people(c: &Channel) -> Vec<u64> {
    let mut v = c.members.clone();
    v.sort_unstable();
    v.dedup();
    v
}

impl App {
    fn seen_key(&self) -> SeenKey {
        let pass = self.kept.pass();
        let gen = self.data_gen.get();
        if let (Some(p), Some((kp, kg, k))) = (pass, self.kept.seen.get()) {
            if p == kp && gen == kg {
                return k;
            }
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for c in &self.data.channels {
            (c.id, c.last_read).hash(&mut h);
        }
        let mut seen: Vec<_> = self.data.extra.chat_seen.iter().collect();
        seen.sort_unstable_by_key(|(id, _)| **id);
        for (id, s) in seen {
            (id, s.floor, &s.ids).hash(&mut h);
        }
        self.data.me.hash(&mut h);
        let k = SeenKey(h.finish());
        if let Some(p) = pass {
            self.kept.seen.set(Some((p, gen, k)));
        }
        k
    }

    /// The chat index for the workspace as it is; none while a change is
    /// waiting to be taken in (`dirty`), when everything is worked out the
    /// slow way.
    pub(super) fn chat_index(&self) -> Option<Rc<ChatIndex>> {
        let key = self.inbox_key()?;
        let seen = self.seen_key();
        if let Some((k, s, ix)) = &*self.kept.chat.borrow() {
            if *k == key && *s == seen {
                return Some(ix.clone());
            }
        }
        let ix = Rc::new(self.build_chat_index());
        *self.kept.chat.borrow_mut() = Some((key, seen, ix.clone()));
        Some(ix)
    }

    fn build_chat_index(&self) -> ChatIndex {
        let chans = &self.data.channels;
        let mut ix = ChatIndex::default();
        let mut groups: HashMap<Vec<u64>, Vec<usize>> = HashMap::new();
        for (i, c) in chans.iter().enumerate() {
            ix.at.insert(c.id, i);
            ix.unread.insert(
                c.id,
                c.messages.iter().filter(|m| self.is_unread(c, m)).count(),
            );
            if c.kind == ChannelKind::Direct {
                groups.entry(people(c)).or_default().push(i);
            }
        }
        for g in groups.values() {
            // the one that shows: one with messages, then the older
            let shown = *g
                .iter()
                .min_by_key(|&&i| (chans[i].messages.is_empty(), chans[i].id))
                .expect("a group has a channel");
            for &i in g {
                let c = &chans[i];
                ix.primary.insert(c.id, chans[shown].id);
                // a duplicate: some other copy ranks before it
                let dup = g.iter().any(|&j| {
                    j != i
                        && (chans[j].messages.is_empty(), chans[j].id)
                            < (c.messages.is_empty(), c.id)
                });
                if dup {
                    ix.duplicate.insert(c.id);
                } else {
                    let mut v = vec![i];
                    v.extend(g.iter().copied().filter(|&j| j != i));
                    ix.copies.insert(c.id, v);
                }
            }
        }
        ix
    }

    /// `c` is the channel at its place in `data.channels` (not a copy),
    /// with the index that says so.
    fn indexed(&self, c: &Channel) -> Option<Rc<ChatIndex>> {
        let ix = self.chat_index()?;
        let i = *ix.at.get(&c.id)?;
        std::ptr::eq(self.data.channels.get(i)?, c).then_some(ix)
    }

    /// `unread_here` from the index; none when it can't say.
    pub(super) fn kept_unread_here(&self, c: &Channel) -> Option<usize> {
        self.indexed(c)?.unread.get(&c.id).copied()
    }

    /// `dm_duplicate` from the index; none when it can't say.
    pub(super) fn kept_dm_duplicate(&self, c: &Channel) -> Option<bool> {
        Some(self.indexed(c)?.duplicate.contains(&c.id))
    }

    /// `dm_primary` from the index; none when it can't say.
    pub(super) fn kept_dm_primary(&self, c: &Channel) -> Option<u64> {
        self.indexed(c)?.primary.get(&c.id).copied()
    }

    /// `conversation` from the index, for a direct message that shows;
    /// none when it can't say.
    pub(super) fn kept_conversation<'a>(&'a self, c: &'a Channel) -> Option<Vec<&'a Channel>> {
        let ix = self.indexed(c)?;
        let v = ix.copies.get(&c.id)?;
        v.iter()
            .map(|&i| self.data.channels.get(i))
            .collect::<Option<Vec<_>>>()
    }
}
