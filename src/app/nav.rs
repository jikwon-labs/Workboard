// Getting around: back / forward history entries, workboard:// links (what
// "Copy link" puts on the clipboard), what the Inbox and its badge count, and
// reminders coming due.

use super::*;

// ---------------------------------------------------------------- history

/// One page in the back / forward history: the section, the place in the
/// tree, and the item open in that section (channel, doc, whiteboard, form or
/// dashboard), so #a -> #b -> Back returns to #a.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct NavSpot {
    pub section: Section,
    pub loc: Location,
    pub item: Option<u64>,
}

// ---------------------------------------------------------------- links

/// Where a workboard:// link points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Link {
    Task(u64),
    Loc(Location),
    Doc(u64),
}

/// Reads a workboard:// link: workboard://task/ID, doc/ID, list/ID,
/// folder/ID, space/ID, and the fixed pages (home, inbox, replies, assigned, my-tasks,
/// today, everything). Spaces around the link and a trailing slash or
/// punctuation are ignored.
pub(crate) fn parse_link(s: &str) -> Option<Link> {
    let s = s
        .trim()
        .trim_end_matches(|c: char| matches!(c, '/' | '.' | ',' | ')' | '>' | '"' | '\''));
    let rest = s.strip_prefix("workboard://")?;
    let mut parts = rest.split('/');
    let kind = parts.next()?.to_ascii_lowercase();
    let id = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let num = || id.and_then(|x| x.parse::<u64>().ok());
    Some(match (kind.as_str(), id) {
        ("task", _) => Link::Task(num()?),
        ("doc", _) => Link::Doc(num()?),
        ("list", _) => Link::Loc(Location::List(num()?)),
        ("folder", _) => Link::Loc(Location::Folder(num()?)),
        ("space", _) => Link::Loc(Location::Space(num()?)),
        ("home", None) => Link::Loc(Location::Home),
        ("inbox", None) => Link::Loc(Location::Inbox),
        ("replies", None) => Link::Loc(Location::Replies),
        ("assigned", None) => Link::Loc(Location::Assigned),
        ("my-tasks", None) => Link::Loc(Location::MyTasks),
        ("today", None) => Link::Loc(Location::TodayOverdue),
        ("everything", None) => Link::Loc(Location::Everything),
        _ => return None,
    })
}

/// The workboard:// link of a place (read back by `parse_link`).
pub(crate) fn location_link(loc: Location) -> String {
    match loc {
        Location::Home => "workboard://home".into(),
        Location::Inbox => "workboard://inbox".into(),
        Location::Replies => "workboard://replies".into(),
        Location::Assigned => "workboard://assigned".into(),
        Location::MyTasks => "workboard://my-tasks".into(),
        Location::TodayOverdue => "workboard://today".into(),
        Location::Everything => "workboard://everything".into(),
        Location::Space(id) => format!("workboard://space/{}", id),
        Location::Folder(id) => format!("workboard://folder/{}", id),
        Location::List(id) => format!("workboard://list/{}", id),
    }
}

/// The workboard:// links in a text that `parse_link` can read, with the
/// character range each one takes (start, end), in order.
pub(crate) fn find_links(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = vec![];
    let mut start = 0;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i <= chars.len() {
        if i == chars.len() || chars[i].is_whitespace() {
            if i > start {
                let word: String = chars[start..i].iter().collect();
                if let Some(at) = word.find("workboard://") {
                    let lead = word[..at].chars().count();
                    let link: String = word[at..].to_string();
                    if parse_link(&link).is_some() {
                        let clean = link.trim_end_matches(|c: char| {
                            matches!(c, '/' | '.' | ',' | ')' | '>' | '"' | '\'')
                        });
                        out.push((
                            start + lead,
                            start + lead + clean.chars().count(),
                            clean.to_string(),
                        ));
                    }
                }
            }
            start = i + 1;
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------- inbox

/// What an Inbox row opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum InboxSrc {
    Task(u64),
    /// A chat message; `thread` is the message whose thread holds it.
    Chat {
        ch: u64,
        msg: u64,
        thread: Option<u64>,
    },
}

#[derive(Clone, Debug)]
pub(super) struct InboxItem {
    pub key: String,
    pub ts: i64,
    pub author: u64,
    pub title: String,
    pub text: String,
    pub src: InboxSrc,
    /// A task comment or a chat message (not a task history line).
    pub comment: bool,
    pub mention: bool,
    pub assigned: bool,
    /// A chat reply in a thread I started or wrote in.
    pub reply: bool,
    /// 0 Primary, 1 Other, 2 Later, 3 Cleared.
    pub tab: u8,
    /// From an imported message (old history): starts out Cleared and is
    /// never announced.
    pub imported: bool,
}

/// What the Inbox rows were last built from. The sidebar and the Dock badge
/// ask for the rows on every frame, and building them walks every task's
/// history and every message, so they're kept until something here changes:
/// the workspace was changed (`App::data_changed`), or, as a check for a
/// change that didn't say so, a count below.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct InboxKey {
    gen: u64,
    me: u64,
    tasks: usize,
    activity: usize,
    channels: usize,
    messages: usize,
    replies: usize,
    channel_members: usize,
    private_channels: usize,
    members: usize,
    cleared: usize,
    later: usize,
    replies_read: usize,
    replies_seen: i64,
    settings: [bool; 4],
}

/// The Inbox rows and the Replies count, with what they were built from.
#[derive(Default)]
pub(super) struct InboxCache {
    rows: Option<(InboxKey, std::rc::Rc<Vec<InboxItem>>)>,
    unread_replies: Option<(InboxKey, usize)>,
}

impl App {
    /// Something in the workspace changed: what was worked out from it (the
    /// Inbox rows) is worked out again when next asked for.
    pub(super) fn data_changed(&self) {
        self.data_gen.set(self.data_gen.get().wrapping_add(1));
    }

    /// The key the Inbox rows are kept under; none while a change is
    /// waiting to be taken in (`dirty`), when they're built fresh.
    fn inbox_key(&self) -> Option<InboxKey> {
        if self.dirty {
            return None;
        }
        let d = &self.data;
        let n = &d.extra.notify;
        Some(InboxKey {
            gen: self.data_gen.get(),
            me: d.me,
            tasks: d.tasks.len(),
            activity: d.tasks.iter().map(|t| t.activity.len()).sum(),
            channels: d.channels.len(),
            messages: d.channels.iter().map(|c| c.messages.len()).sum(),
            replies: d
                .channels
                .iter()
                .flat_map(|c| &c.messages)
                .map(|m| m.replies.len())
                .sum(),
            channel_members: d.channels.iter().map(|c| c.members.len()).sum(),
            private_channels: d.channels.iter().filter(|c| c.private).count(),
            members: d.members.len(),
            cleared: d.extra.inbox_cleared.len(),
            later: d.extra.inbox_later.len(),
            replies_read: d.extra.replies_read.len(),
            replies_seen: d.extra.replies_seen,
            settings: [n.comments, n.mentions, n.assigned, n.replies],
        })
    }

    /// The Inbox rows (all four tabs, see `build_inbox_items`), built again
    /// only after the workspace changed.
    pub(super) fn inbox_items(&self) -> std::rc::Rc<Vec<InboxItem>> {
        let key = self.inbox_key();
        if let (Some(k), Some((kept, rows))) = (key, &self.inbox_cache.borrow().rows) {
            if k == *kept {
                return rows.clone();
            }
        }
        let rows = std::rc::Rc::new(self.build_inbox_items());
        self.inbox_cache.borrow_mut().rows = key.map(|k| (k, rows.clone()));
        rows
    }

    /// The count `unread_replies` works out, kept like the Inbox rows.
    pub(super) fn kept_unread_replies(&self, count: impl FnOnce() -> usize) -> usize {
        let key = self.inbox_key();
        if let (Some(k), Some((kept, n))) = (key, self.inbox_cache.borrow().unread_replies) {
            if k == kept {
                return n;
            }
        }
        let n = count();
        self.inbox_cache.borrow_mut().unread_replies = key.map(|k| (k, n));
        n
    }

    /// The Inbox rows (all four tabs). Primary holds what the notification
    /// settings ask for: comments on tasks I'm assigned to or created,
    /// @mentions of me (task comments and chat), tasks assigned to me and
    /// chat replies in threads I'm in. My own actions are left out.
    /// Mentions and assignments are matched by member id; entries written
    /// before ids were kept are matched by name.
    fn build_inbox_items(&self) -> Vec<InboxItem> {
        let me = self.data.me;
        let n = &self.data.extra.notify;
        let my_name = self
            .data
            .member(me)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        let cleared = &self.data.extra.inbox_cleared;
        let later = &self.data.extra.inbox_later;
        let tab_of = |key: &str, primary: bool| {
            if cleared.iter().any(|k| k == key) {
                3
            } else if later.iter().any(|k| k == key) {
                2
            } else if primary {
                0
            } else {
                1
            }
        };
        // imported history is already cleared for everyone, unless saved
        // for later
        let imported_tab = |key: &str| {
            if later.iter().any(|k| k == key) {
                2
            } else {
                3
            }
        };
        let members = &self.data.members;
        let mentions = |text: &str, ids: &[u64]| {
            if ids.is_empty() {
                crate::model::mentioned_ids(text, members, &[]).contains(&me)
            } else {
                ids.contains(&me)
            }
        };
        let mut out = vec![];
        for task in &self.data.tasks {
            let mine = self.my_task(task);
            for a in &task.activity {
                if a.author == me {
                    continue;
                }
                let mention = a.comment && mentions(&a.text, &a.mentions);
                let assigned = !a.comment
                    && match a.target {
                        Some(who) => who == me && a.text.starts_with("assigned to "),
                        None => !my_name.is_empty() && a.text == format!("assigned to {}", my_name),
                    };
                let primary = (a.comment && n.comments && mine)
                    || (mention && n.mentions)
                    || (assigned && n.assigned);
                let key = format!("{}:{}:{}", task.id, a.ts, a.author);
                let tab = tab_of(&key, primary);
                out.push(InboxItem {
                    key,
                    ts: a.ts,
                    author: a.author,
                    title: task.name.clone(),
                    text: a.text.clone(),
                    src: InboxSrc::Task(task.id),
                    comment: a.comment,
                    mention,
                    assigned,
                    reply: false,
                    tab,
                    imported: false,
                });
            }
        }
        for c in &self.data.channels {
            // only conversations I can see: private channels and DMs are for their members
            if (c.kind == ChannelKind::Direct || c.private) && !c.members.contains(&me) {
                continue;
            }
            let title = if c.kind == ChannelKind::Channel {
                format!("#{}", c.name)
            } else {
                self.data
                    .member(c.members.iter().copied().find(|m| *m != me).unwrap_or(me))
                    .map(|m| m.name.clone())
                    .unwrap_or_default()
            };
            for m in &c.messages {
                if m.author != me && mentions(&m.text, &m.mentions) {
                    let key = format!("chat:{}:{}", c.id, m.id);
                    let tab = if m.imported.is_some() {
                        imported_tab(&key)
                    } else {
                        tab_of(&key, n.mentions)
                    };
                    out.push(InboxItem {
                        key,
                        ts: m.ts,
                        author: m.author,
                        title: title.clone(),
                        text: m.text.clone(),
                        src: InboxSrc::Chat {
                            ch: c.id,
                            msg: m.id,
                            thread: None,
                        },
                        comment: true,
                        mention: true,
                        assigned: false,
                        reply: false,
                        tab,
                        imported: m.imported.is_some(),
                    });
                }
                for (i, r) in m.replies.iter().enumerate() {
                    if r.author == me {
                        continue;
                    }
                    let mention = mentions(&r.text, &r.mentions);
                    // a reply counts once I started the thread or wrote in it before
                    let reply = m.author == me || m.replies[..i].iter().any(|x| x.author == me);
                    if !(mention || reply) {
                        continue;
                    }
                    let key = format!("chat:{}:{}", c.id, r.id);
                    let tab = if r.imported.is_some() {
                        imported_tab(&key)
                    } else {
                        tab_of(&key, (mention && n.mentions) || reply)
                    };
                    out.push(InboxItem {
                        key,
                        ts: r.ts,
                        author: r.author,
                        title: title.clone(),
                        text: r.text.clone(),
                        src: InboxSrc::Chat {
                            ch: c.id,
                            msg: r.id,
                            thread: Some(m.id),
                        },
                        comment: true,
                        mention,
                        assigned: false,
                        reply,
                        tab,
                        imported: r.imported.is_some(),
                    });
                }
            }
        }
        out
    }

    /// A task whose comments are for me (the Inbox's Primary tab and
    /// desktop notifications): one I'm assigned to, or one I made, as new
    /// or as a copy of another.
    pub(super) fn my_task(&self, t: &Task) -> bool {
        let me = self.data.me;
        t.assignees.contains(&me)
            || t.activity.iter().any(|a| {
                a.author == me
                    && !a.comment
                    && (a.text == "created this task"
                        || a.text.starts_with("duplicated this task from "))
            })
    }

    /// The Inbox badge: the Primary rows (not cleared, not saved for later)
    /// plus reminders that are due, when reminders are turned on.
    pub(super) fn inbox_badge(&self) -> usize {
        let rows = self.inbox_items().iter().filter(|i| i.tab == 0).count();
        rows + if self.data.extra.notify.reminders {
            self.due_reminders()
        } else {
            0
        }
    }

    /// The text of an Inbox row under its title.
    pub(super) fn inbox_line(
        &self,
        author: u64,
        text: &str,
        comment: bool,
        src: InboxSrc,
        mention: bool,
    ) -> String {
        let who = self
            .data
            .member(author)
            .map(|m| m.name.clone())
            .unwrap_or_default();
        match src {
            InboxSrc::Chat {
                thread: Some(_), ..
            } if !mention => trf!("{} replied: {}", who, text),
            InboxSrc::Chat { .. } => trf!("{} mentioned you: {}", who, text),
            InboxSrc::Task(_) if comment => trf!("{} commented: {}", who, text),
            InboxSrc::Task(_) => crate::i18n::history_line(&who, text),
        }
    }

    /// Opens what an Inbox row points at.
    pub(super) fn open_inbox_src(&mut self, src: InboxSrc) {
        match src {
            InboxSrc::Task(id) => self.actions.push(Action::Open(id)),
            InboxSrc::Chat { ch, thread, .. } => self.open_chat_at(ch, thread),
        }
    }

    /// Shows a channel, with a thread open when given.
    pub(super) fn open_chat_at(&mut self, ch: u64, thread: Option<u64>) {
        if !self.data.channels.iter().any(|c| c.id == ch) {
            return;
        }
        self.section = Section::Chat;
        self.chat_open = Some(ch);
        self.thread_open = thread;
    }

    /// Chat replies for the Replies page: replies by others in threads I
    /// started or wrote in.
    pub(super) fn chat_replies(&self) -> Vec<InboxItem> {
        self.inbox_items()
            .iter()
            .filter(|i| i.reply)
            .cloned()
            .collect()
    }

    // ---------------------------------------------------------------- history

    /// The page shown now, for the back / forward history.
    pub(super) fn nav_spot(&self) -> NavSpot {
        let item = match self.section {
            Section::Chat => self.chat_open,
            Section::Docs => self.doc_open,
            Section::Whiteboards => self.wb_open,
            Section::Forms => self.form_open,
            Section::Dashboards => self.dash_open,
            _ => None,
        };
        NavSpot {
            section: self.section,
            loc: self.loc,
            item,
        }
    }

    /// The page of a history entry still exists (its place, and its channel,
    /// doc, whiteboard, form or dashboard).
    pub(super) fn nav_spot_exists(&self, s: &NavSpot) -> bool {
        if !self.data.location_exists(s.loc) {
            return false;
        }
        let Some(id) = s.item else { return true };
        let x = &self.data.extra;
        match s.section {
            Section::Chat => self.data.channels.iter().any(|c| c.id == id),
            Section::Docs => self.data.docs.iter().any(|d| d.id == id),
            Section::Whiteboards => x.whiteboards.iter().any(|w| w.id == id),
            Section::Forms => x.forms.iter().any(|f| f.id == id),
            // the Workspace overview is built in, not one of the dashboards
            Section::Dashboards => {
                id == super::hubs::WORKSPACE_OVERVIEW || x.dashboards.iter().any(|d| d.id == id)
            }
            _ => true,
        }
    }

    /// Shows a history entry.
    pub(super) fn show_nav_spot(&mut self, s: NavSpot) {
        self.section = s.section;
        if self.loc != s.loc {
            // moving to another place resets what Action::Go resets: the
            // selection, the open task and the collapsed groups belong to
            // the place left behind
            self.selected.clear();
            self.open_task = None;
            self.collapsed.clear();
            if self.view == ViewKind::Overview
                && !matches!(s.loc, Location::Space(_) | Location::Folder(_))
            {
                self.view = ViewKind::List;
            }
        }
        self.loc = s.loc;
        match s.section {
            Section::Chat => {
                if self.chat_open != s.item {
                    self.thread_open = None;
                }
                self.chat_open = s.item;
            }
            Section::Docs => self.doc_open = s.item,
            Section::Whiteboards => self.wb_open = s.item,
            Section::Forms => self.form_open = s.item,
            Section::Dashboards => self.dash_open = s.item,
            _ => {}
        }
    }

    /// Forgets the back / forward history (another workspace was opened).
    pub(super) fn clear_history(&mut self) {
        self.nav_back.clear();
        self.nav_fwd.clear();
        self.nav_last = None;
        self.nav_skip = false;
    }

    // ---------------------------------------------------------------- links

    /// A place I can go to, as the sidebar shows them: it exists, and isn't
    /// archived (nor its folder or space) or in someone else's Personal space.
    pub(super) fn location_shown(&self, loc: Location) -> bool {
        let me = self.data.me;
        let space_ok = |s: &Space| !s.archived && s.private_owner.is_none_or(|o| o == me);
        self.data.location_exists(loc)
            && match loc {
                Location::Space(id) => self
                    .data
                    .spaces
                    .iter()
                    .find(|s| s.id == id)
                    .is_some_and(space_ok),
                Location::Folder(id) => self
                    .data
                    .spaces
                    .iter()
                    .any(|s| space_ok(s) && s.folders.iter().any(|f| f.id == id && !f.archived)),
                Location::List(id) => {
                    !self.data.list_archived(id)
                        && self.data.space_of_list(id).is_some_and(space_ok)
                }
                _ => true,
            }
    }

    /// A doc shown in lists, search and the sidebar: not archived, not
    /// someone else's Private doc and not in a space I can't go to.
    pub(super) fn doc_shown(&self, d: &Doc) -> bool {
        !d.archived && self.doc_reachable(d)
    }

    /// `doc_shown` without the archived check (the Docs hub's Archived tab
    /// lists archived docs, still only the ones I could otherwise see).
    pub(super) fn doc_reachable(&self, d: &Doc) -> bool {
        (!d.private || d.owner == self.data.me)
            && d.space.is_none_or(|s| {
                !self.data.location_exists(Location::Space(s))
                    || self.location_shown(Location::Space(s))
            })
    }

    /// A dashboard shown in lists and search: not someone else's private
    /// one (older ones without a maker are everyone's) and not in a space I
    /// can't go to.
    pub(super) fn dash_shown(&self, d: &Dashboard) -> bool {
        (!d.private || d.owner == self.data.me || d.owner == 0)
            && d.space.is_none_or(|s| {
                !self.data.location_exists(Location::Space(s))
                    || self.location_shown(Location::Space(s))
            })
    }

    /// A form shown in lists and search: not one for a list in someone
    /// else's Personal space.
    pub(super) fn form_shown(&self, f: &Form) -> bool {
        let me = self.data.me;
        self.data
            .space_of_list(f.list_id)
            .is_none_or(|s| s.private_owner.is_none_or(|o| o == me))
    }

    /// What a workboard:// link opens: `None` when this workspace doesn't
    /// have it, `Some(false)` when it is there but archived or someone
    /// else's private place.
    pub(super) fn link_shown(&self, link: Link) -> Option<bool> {
        match link {
            Link::Task(id) => self.task(id).map(|t| {
                let l = Location::List(t.list_id);
                !self.data.location_exists(l) || self.location_shown(l)
            }),
            Link::Loc(l) => self.data.location_exists(l).then(|| self.location_shown(l)),
            Link::Doc(id) => self
                .data
                .docs
                .iter()
                .find(|d| d.id == id)
                .map(|d| self.doc_shown(d)),
        }
    }

    /// Opens a workboard:// link; false when it isn't one this workspace has
    /// (or it is archived or in someone else's Personal space).
    pub(super) fn open_link(&mut self, url: &str) -> bool {
        self.open_link_to(parse_link(url))
    }

    /// Opens what a read workboard:// link points to (see `open_link`).
    pub(super) fn open_link_to(&mut self, link: Option<Link>) -> bool {
        let shown = link.and_then(|l| self.link_shown(l));
        match (link, shown) {
            (Some(Link::Task(id)), Some(true)) => self.actions.push(Action::Open(id)),
            (Some(Link::Loc(l)), Some(true)) => self.actions.push(Action::Go(l)),
            (Some(Link::Doc(id)), Some(true)) => {
                self.open_task = None;
                self.section = Section::Docs;
                self.doc_open = Some(id);
            }
            (_, Some(false)) => {
                self.toast = Some((
                    tr("This link points to something archived or private").into(),
                    now_ts(),
                ))
            }
            _ => {
                self.toast = Some((
                    tr("This link can't be opened in this Workspace").into(),
                    now_ts(),
                ))
            }
        }
        shown == Some(true)
    }

    /// A message text with its workboard:// links clickable; returns the
    /// link clicked.
    pub(super) fn text_with_links(
        &self,
        ui: &mut Ui,
        job: egui::text::LayoutJob,
        text: &str,
    ) -> Option<String> {
        let links = find_links(text);
        if links.is_empty() {
            ui.add(egui::Label::new(job).wrap());
            return None;
        }
        let mut job = job;
        // colour the links (the job's text is the message text, word for word)
        let t = self.t();
        let sections: Vec<_> = job.sections.drain(..).collect();
        let full = job.text.clone();
        let char_at = |byte: usize| full[..byte].chars().count();
        for mut s in sections {
            let (a, b) = (char_at(s.byte_range.start), char_at(s.byte_range.end));
            if links.iter().any(|(ls, le, _)| a < *le && b > *ls) {
                s.format.color = t.primary;
                s.format.underline = Stroke::new(1.0, t.primary);
            }
            job.sections.push(s);
        }
        let (pos, galley, resp) = egui::Label::new(job)
            .wrap()
            .sense(Sense::click())
            .layout_in_ui(ui);
        ui.painter().galley(pos, galley.clone(), t.text);
        let at = |p: egui::Pos2| galley.cursor_from_pos(p - pos).ccursor.index;
        let over = resp
            .hover_pos()
            .map(at)
            .and_then(|c| links.iter().find(|(s, e, _)| c >= *s && c < *e).cloned());
        if over.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            let c = resp.interact_pointer_pos().map(at);
            return c
                .and_then(|c| links.iter().find(|(s, e, _)| c >= *s && c < *e))
                .map(|l| l.2.clone());
        }
        None
    }

    /// Links waiting to be opened: clicked in a message, or sent by macOS.
    pub(super) fn open_pending_links(&mut self) {
        let urls: Vec<String> = self.link_pending.take().into_iter().collect();
        for u in urls {
            self.open_link(&u);
        }
    }

    // ---------------------------------------------------------------- reminders

    /// Alerts reminders as they come due (once each) and wakes the app up
    /// for the next one.
    pub(super) fn reminder_tick(&mut self, ctx: Option<&egui::Context>) {
        let me = self.data.me;
        let now = now_ts();
        let mut fired: Vec<Reminder> = vec![];
        for r in self
            .data
            .extra
            .reminders
            .iter_mut()
            .filter(|r| r.owner == me && !r.done && !r.alerted && r.due_ts <= now)
        {
            r.alerted = true;
            fired.push(r.clone());
        }
        if !fired.is_empty() {
            self.dirty = true;
            if self.data.extra.notify.reminders {
                let key = format!("reminder:{}:{}", fired[0].id, now);
                // came due a while ago (the app was closed): not "due now"
                let late = fired.iter().any(|r| now - r.due_ts > 10 * 60);
                let note = if fired.len() == 1 {
                    super::notify::Note {
                        key,
                        kind: super::notify::Kind::Reminder,
                        title: tr(if late { "Overdue reminder" } else { "Reminder" }).into(),
                        body: fired[0].text.clone(),
                        target: format!("reminder:{}", fired[0].id),
                    }
                } else {
                    // a toast shows the body on one line, so keep the names apart
                    let body = fired
                        .iter()
                        .map(|r| r.text.trim().replace('\n', " "))
                        .collect::<Vec<_>>()
                        .join(", ");
                    super::notify::Note {
                        key,
                        kind: super::notify::Kind::Reminder,
                        title: if late {
                            trf!("{} reminders are overdue", fired.len())
                        } else {
                            trf!("{} reminders are due", fired.len())
                        },
                        body,
                        target: "inbox".into(),
                    }
                };
                self.deliver(note);
            }
        }
        if let Some(ctx) = ctx {
            let next = self
                .data
                .extra
                .reminders
                .iter()
                .filter(|r| r.owner == me && !r.done && !r.alerted && r.due_ts > now)
                .map(|r| r.due_ts)
                .min();
            if let Some(next) = next {
                ctx.request_repaint_after(std::time::Duration::from_secs(
                    (next - now).max(1) as u64
                ));
            }
        }
    }

    /// Opens what a reminder is about (a task, or the message in its channel).
    pub(super) fn open_reminder(&mut self, r: &Reminder) {
        if let Some(id) = r
            .task
            .filter(|id| self.data.tasks.iter().any(|t| t.id == *id))
        {
            self.actions.push(Action::Open(id));
        } else if let Some((ch, msg)) = r.message {
            let thread = self
                .data
                .channels
                .iter()
                .find(|c| c.id == ch)
                .and_then(|c| {
                    c.messages
                        .iter()
                        .find(|m| m.replies.iter().any(|x| x.id == msg))
                })
                .map(|m| m.id);
            self.open_chat_at(ch, thread);
        }
    }
}
