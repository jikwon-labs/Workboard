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

/// A message's text worked out for showing (`App::message_text`): its
/// formatting, where the mentions and links are, and where the text is cut
/// into differently drawn pieces (byte offsets, with their char offsets).
pub(super) struct ShownText {
    pub r: super::chatfmt::Rich,
    pub mentions: Vec<(usize, usize)>,
    pub links: Vec<(usize, usize, String)>,
    pub cuts: Vec<usize>,
    pub cut_chars: HashMap<usize, usize>,
    /// When workboard:// links in it show the names of what they point to:
    /// the workspace's change counts (`data_gen`, `docs_gen`) those names
    /// were read at, so a rename shows once it happens.
    pub named_at: Option<(u64, u64)>,
}

/// Texts kept at most; past this they're all worked out again.
const TEXTS_KEPT: usize = 4000;

/// What `App::filtered` depends on besides the tasks themselves.
#[derive(Clone, PartialEq)]
pub(super) struct FilteredKey {
    /// The pass it was worked out in: tasks are changed in place in many
    /// ways, so this is only kept within one pass (Board, the sidebar and
    /// the List's counts ask several times in one).
    pub pass: u64,
    pub gen: u64,
    pub tasks: usize,
    pub me: u64,
    pub loc: Location,
    pub search: String,
    pub me_mode: bool,
    pub assignee: Option<u64>,
    pub show_closed: bool,
    pub filter: parity::TaskFilter,
    pub sort: (SortBy, bool),
    /// Today, and the minute: what's overdue changes with the time.
    pub minute: i64,
}

#[derive(Default)]
pub(super) struct Kept {
    /// Inside a pass of `ui_frame`: its number plus one; 0 outside, when
    /// nothing is remembered from one call to the next.
    pass: Cell<u64>,
    seen: Cell<Option<(u64, u64, SeenKey)>>,
    chat: RefCell<Option<(nav::InboxKey, SeenKey, Rc<ChatIndex>)>>,
    /// The names an @ can stand for (`App::mention_names`).
    names: RefCell<Option<(nav::InboxKey, Rc<Vec<String>>)>>,
    /// Message texts as shown, for the names they were worked out with.
    texts: RefCell<(Rc<Vec<String>>, HashMap<String, Rc<ShownText>>)>,
    /// The open conversation's messages in time order: (key, the copies'
    /// places in `data.channels`, (copy, message) places).
    order: RefCell<Option<(nav::InboxKey, Vec<usize>, Rc<Vec<(usize, usize)>>)>>,
    /// The open conversation's messages its find box and panel let through:
    /// (key, conversation, what's in the find box, panel, places).
    #[allow(clippy::type_complexity)]
    /// The search palette's results and archived count for a query:
    /// (key, query, kind, rows, archived).
    #[allow(clippy::type_complexity)]
    search: RefCell<
        Option<(
            (nav::InboxKey, u64),
            String,
            u8,
            Rc<Vec<more::SearchRow>>,
            usize,
        )>,
    >,
    pub(super) filtered: RefCell<Option<(FilteredKey, Rc<Vec<u64>>)>>,
    /// Open top-level tasks by list, for the sidebar: (pass, change,
    /// tasks, counts). Only kept within one pass, like `filtered`.
    #[allow(clippy::type_complexity)]
    /// Every tag in use (`App::all_tags`): (change, tasks, tags, them).
    #[allow(clippy::type_complexity)]
    pub(super) tags: RefCell<Option<((u64, usize, usize), Vec<String>)>>,
    pub(super) open_counts: RefCell<Option<((u64, u64, usize), Rc<HashMap<u64, usize>>)>>,
    pub(super) matching: RefCell<
        Option<(
            nav::InboxKey,
            u64,
            String,
            chat::ChannelPanel,
            Rc<chat::Found>,
        )>,
    >,
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

    /// `mention_names`, kept until the workspace changes.
    pub(super) fn kept_mention_names(&self) -> Rc<Vec<String>> {
        let key = self.inbox_key();
        if let (Some(k), Some((kept, names))) = (key, &*self.kept.names.borrow()) {
            if k == *kept {
                return names.clone();
            }
        }
        let names = Rc::new(self.mention_names());
        if let Some(k) = key {
            *self.kept.names.borrow_mut() = Some((k, names.clone()));
        }
        names
    }

    /// The workspace's change counts the names links show depend on.
    fn names_gen(&self) -> (u64, u64) {
        (self.data_gen.get(), self.docs_gen.get())
    }

    /// A message's text as shown (see `message_text`), worked out once for
    /// the names an @ can stand for.
    pub(super) fn shown_text(&self, src: &str) -> Rc<ShownText> {
        let names = self.kept_mention_names();
        {
            let mut kept = self.kept.texts.borrow_mut();
            if *kept.0 != *names || kept.1.len() > TEXTS_KEPT {
                *kept = (names.clone(), HashMap::new());
            }
            if let Some(s) = kept.1.get(src) {
                if s.named_at.is_none_or(|g| g == self.names_gen()) {
                    return s.clone();
                }
            }
        }
        let mut r = super::chatfmt::rich(src);
        let mut links = r.links.clone();
        // a workboard:// link typed as is shows the name of what it points to
        let mut named = vec![];
        for l in super::nav::find_links(&r.text) {
            if !links.iter().any(|x| x.0 < l.1 && l.0 < x.1) {
                if let Some(name) = super::nav::parse_link(&l.2).and_then(|k| self.link_title(k)) {
                    named.push((l.0, l.1, name));
                }
                links.push(l);
            }
        }
        links.sort_by_key(|l| l.0);
        named.sort_by_key(|l| l.0);
        let named_at = (!named.is_empty()).then(|| self.names_gen());
        swap_text(&mut r, &mut links, &named);
        let mentions = chat::mention_spans(&r.text, &names);
        let mut cuts: Vec<usize> = vec![0, r.text.len()];
        for (a, b, _) in &r.runs {
            cuts.extend([*a, *b]);
        }
        for (a, b) in &mentions {
            cuts.extend([*a, *b]);
        }
        cuts.sort_unstable();
        cuts.dedup();
        let mut cut_chars = HashMap::new();
        let (mut byte, mut chars) = (0, 0);
        for &c in &cuts {
            chars += r.text.get(byte..c).map_or(0, |s| s.chars().count());
            byte = c;
            cut_chars.insert(c, chars);
        }
        let s = Rc::new(ShownText {
            r,
            mentions,
            links,
            cuts,
            cut_chars,
            named_at,
        });
        self.kept
            .texts
            .borrow_mut()
            .1
            .insert(src.to_string(), s.clone());
        s
    }

    /// Where the open conversation's messages are (`copies` are their
    /// channels' places), in time order.
    pub(super) fn message_order(&self, copies: &[usize]) -> Rc<Vec<(usize, usize)>> {
        let key = self.inbox_key();
        if let (Some(k), Some((kept, c, at))) = (key, &*self.kept.order.borrow()) {
            if k == *kept && c == copies {
                return at.clone();
            }
        }
        let chans = &self.data.channels;
        let mut at: Vec<(usize, usize)> = copies
            .iter()
            .flat_map(|&c| (0..chans[c].messages.len()).map(move |m| (c, m)))
            .collect();
        at.sort_by_key(|&(c, m)| chans[c].messages[m].ts);
        let at = Rc::new(at);
        if let Some(k) = key {
            *self.kept.order.borrow_mut() = Some((k, copies.to_vec(), at.clone()));
        }
        at
    }

    /// The search palette's results (`search_rows`) and how many archived
    /// matches it leaves out (`archived_matches`), worked out again only
    /// when the query, its kind or the workspace change: the palette asks
    /// on every frame it is open. An empty query lists recent things, which
    /// aren't kept.
    pub(super) fn kept_search(&self, q: &str, kind: u8) -> (Rc<Vec<more::SearchRow>>, usize) {
        // (docs are searched too: typing in one changes only `docs_gen`)
        let key = self
            .inbox_key()
            .filter(|_| !q.trim().is_empty())
            .map(|k| (k, self.docs_gen.get()));
        if let (Some(k), Some((kk, kq, kind2, rows, arch))) = (key, &*self.kept.search.borrow()) {
            if k == *kk && kq == q && *kind2 == kind {
                return (rows.clone(), *arch);
            }
        }
        let rows = Rc::new(self.search_rows(q, kind));
        let arch = self.archived_matches(q, kind);
        if let Some(k) = key {
            *self.kept.search.borrow_mut() = Some((k, q.to_string(), kind, rows.clone(), arch));
        }
        (rows, arch)
    }
}

/// Puts the words of `swap` (character ranges of `r`'s text, in order, not
/// overlapping) in place of what those ranges hold, moving `r`'s runs (byte
/// ranges) and its links and `links` (character ranges) to match.
pub(super) fn swap_text(
    r: &mut super::chatfmt::Rich,
    links: &mut [(usize, usize, String)],
    swap: &[(usize, usize, String)],
) {
    // the last first, so the places before it stay where they are
    for (cs, ce, new) in swap.iter().rev() {
        let byte = |c: usize| r.text.char_indices().nth(c).map_or(r.text.len(), |x| x.0);
        let (bs, be) = (byte(*cs), byte(*ce));
        let (nb, nc) = (new.len(), new.chars().count());
        r.text.replace_range(bs..be, new);
        // a place before the swapped words stays, one after moves by the
        // change in length, one inside goes to the near end of the new words
        let at = |p: usize, a: usize, b: usize, n: usize, end: bool| {
            if p <= a {
                p
            } else if p >= b {
                p - (b - a) + n
            } else if end {
                a + n
            } else {
                a
            }
        };
        for run in r.runs.iter_mut() {
            run.0 = at(run.0, bs, be, nb, false);
            run.1 = at(run.1, bs, be, nb, true);
        }
        for l in r.links.iter_mut().chain(links.iter_mut()) {
            l.0 = at(l.0, *cs, *ce, nc, false);
            l.1 = at(l.1, *cs, *ce, nc, true);
        }
    }
}
