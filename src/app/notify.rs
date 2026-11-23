// Notifications. When the team's changes come in, `notify_scan` works out
// what is new for me (messages, mentions, replies, huddles, reactions,
// assignments, comments and changes on my tasks, new teammates, form
// answers), and `notify_clock` watches the clock (tasks due today, meetings
// about to start, goals coming due, passing milestones or falling far
// behind, the server connection). Each thing has a
// key, and a key is announced once.
//
// How it is announced (`deliver`): nothing when I'm already looking at it; a
// toast and a short sound when the app is in front; otherwise a banner in
// Notification Center (src/notifier.rs), and a Dock bounce for direct
// messages, mentions and huddles. The Dock badge counts the Inbox and unread
// direct messages. "Pause notifications" (`muted_until`) silences all of it.
//
// What each person wants is `Data.extra.notify` (NotifySettings), which stays
// on their own computer.

use super::nav::InboxSrc;
use super::*;
use wb_net::Status as NetStatus;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Direct,
    Mention,
    Reply,
    Channel,
    Huddle,
    Reaction,
    Assigned,
    Comment,
    TaskChange,
    Due,
    Reminder,
    Meeting,
    Goal,
    Member,
    Form,
    Connection,
}

#[derive(Clone, Debug)]
pub(super) struct Note {
    pub key: String,
    pub kind: Kind,
    pub title: String,
    pub body: String,
    /// What a click opens: "task:ID", "chat:CH", "chat:CH:THREAD",
    /// "meeting:ID", "goal:ID", "today", "inbox", "people", "together".
    pub target: String,
}

/// News older than this (a teammate's copy from days offline) is not announced.
const FRESH_SECS: i64 = 24 * 3600;
/// A huddle older than this is probably over.
const HUDDLE_FRESH_SECS: i64 = 5 * 60;
/// More new things at once than this become one summary.
const BURST: usize = 3;
/// How long the server connection may be down before it is announced.
const LOST_SECS: i64 = 30;
/// How often the clock is looked at.
const CLOCK_SECS: i64 = 20;
/// How many clock notices `fired` remembers.
const FIRED_KEEP: usize = 300;

#[derive(Default)]
pub struct NotifyRt {
    seen: HashSet<String>,
    /// `seen` holds what was there at the start: only later things are news.
    pub(super) seeded: bool,
    /// The team's changes came in: look for news on the next frame.
    pub(super) pending: bool,
    /// What came in is a (re)connection's catch-up: what happened while
    /// this computer wasn't connected (the app closed, asleep, offline).
    /// It shows as unread and in the Inbox, so only a call still going on
    /// is announced; the rest is taken as seen.
    pub(super) catch_up: bool,
    /// When the clock was last looked at (`i64::MAX`: never again, for UI
    /// tests that must not depend on the time of day).
    pub(super) last_clock: i64,
    /// The window is not in front (as of the last frame).
    pub(super) away: bool,
    /// Everything announced, newest last (kept short; tests read it).
    pub(super) sent: Vec<Note>,
    /// What a click on the toast opens.
    /// (the toast text, the target): only while that toast shows.
    pub(super) toast_target: Option<(String, String)>,
    lost_since: Option<i64>,
    told_lost: bool,
}

fn add(out: &mut Vec<(i64, Note)>, keys: &mut HashSet<String>, ts: i64, note: Note) {
    if keys.insert(note.key.clone()) {
        out.push((ts, note));
    }
}

/// Whether a notice with this key is still worth making: not one already
/// made (`keys`), nor one known already (`skip`), which would be thrown
/// away. Its text (names, previews) is only made when it is.
fn wants(keys: &HashSet<String>, skip: &HashSet<String>, key: &str) -> bool {
    !keys.contains(key) && !skip.contains(key)
}

/// A message's text cut to banner length (a picture, video or file says
/// which it is).
fn preview(m: &Message) -> String {
    // without its formatting marks
    let shown = super::chatfmt::plain(&super::chat::shown_text(m));
    let text = shown.trim();
    let text = if text.is_empty() && !m.attachments.is_empty() {
        sent_what(&m.attachments).to_string()
    } else {
        text.to_string()
    };
    cut(&text, 140)
}

/// What a message with only attachments says in a banner: a picture, a
/// video, both, or a file.
pub(super) fn sent_what(a: &[crate::model::Attachment]) -> &'static str {
    let has = |k: MediaKind| a.iter().any(|x| x.kind == k);
    if has(MediaKind::File) {
        tr("Sent a file")
    } else if has(MediaKind::Video) && has(MediaKind::Image) {
        tr("Sent pictures and videos")
    } else if has(MediaKind::Video) {
        tr("Sent a video")
    } else {
        tr("Sent a picture")
    }
}

fn cut(s: &str, n: usize) -> String {
    truncate(&s.replace('\n', " "), n + 1)
}

/// The goal a notice's key is about ("goal:ID:...", "goalm:ID:...",
/// "goalrisk:ID:...").
fn goal_key(k: &str) -> Option<u64> {
    ["goal:", "goalm:", "goalrisk:"]
        .iter()
        .find_map(|p| k.strip_prefix(p))
        .and_then(|rest| rest.split(':').next())
        .and_then(|id| id.parse().ok())
}

impl App {
    /// Everything that is news for me in the data as it is, with when it
    /// happened (i64::MAX: not known). Turned-off kinds are left out, and
    /// comments and changes on tasks I don't watch unless `every_task`.
    pub(super) fn notify_candidates(
        &self,
        n: &NotifySettings,
        every_task: bool,
    ) -> Vec<(i64, Note)> {
        self.notify_candidates_but(n, every_task, &HashSet::new())
    }

    /// `notify_candidates` without the ones whose key is in `skip` (known
    /// already): after the first look most things are, and making their
    /// text for every message on every change was most of the work.
    pub(super) fn notify_candidates_but(
        &self,
        n: &NotifySettings,
        every_task: bool,
        skip: &HashSet<String>,
    ) -> Vec<(i64, Note)> {
        let me = self.data.me;
        let mut out = vec![];
        let mut keys = HashSet::new();
        let keywords = if n.mentions { n.keyword_list() } else { vec![] };
        // ---- chat: direct messages, channels set to everything, huddles, reactions
        for c in &self.data.channels {
            if !self.can_see(c) {
                continue;
            }
            let dm = c.kind == ChannelKind::Direct;
            let mode = if dm { "all" } else { n.channel_mode(c.id) };
            if mode == "nothing" {
                continue;
            }
            let place = self.channel_label(c);
            for m in &c.messages {
                let key = format!("chat:{}:{}", c.id, m.id);
                // the channel's copy of a thread reply: the reply itself is told
                if m.author != me
                    && m.imported.is_none()
                    && m.thread_of.is_none()
                    && wants(&keys, skip, &key)
                {
                    let who = self.who(m.author);
                    let target = format!("chat:{}", c.id);
                    if m.call.is_some() {
                        if n.huddles {
                            let title = if dm {
                                trf!("{} is calling you", who)
                            } else {
                                trf!("{} started a huddle in {}", who, place)
                            };
                            add(
                                &mut out,
                                &mut keys,
                                m.ts,
                                Note {
                                    key,
                                    kind: Kind::Huddle,
                                    title,
                                    body: tr("Join from the conversation").into(),
                                    target,
                                },
                            );
                        }
                    } else if dm {
                        if n.direct {
                            add(
                                &mut out,
                                &mut keys,
                                m.ts,
                                Note {
                                    key,
                                    kind: Kind::Direct,
                                    title: who,
                                    body: preview(m),
                                    target,
                                },
                            );
                        }
                    } else if mode == "all" {
                        add(
                            &mut out,
                            &mut keys,
                            m.ts,
                            Note {
                                key,
                                kind: Kind::Channel,
                                title: trf!("{} in {}", who, place),
                                body: preview(m),
                                target,
                            },
                        );
                    } else if let Some(word) = self.keyword_hit(m, &keywords) {
                        add(
                            &mut out,
                            &mut keys,
                            m.ts,
                            Note {
                                key,
                                kind: Kind::Mention,
                                title: trf!("{} said \"{}\" in {}", who, word, place),
                                body: preview(m),
                                target,
                            },
                        );
                    }
                }
                // keywords in a channel's threads
                if !dm && mode != "nothing" {
                    for r in m
                        .replies
                        .iter()
                        .filter(|r| r.author != me && r.imported.is_none())
                    {
                        let key = format!("chat:{}:{}", c.id, r.id);
                        if !wants(&keys, skip, &key) {
                            continue;
                        }
                        if let Some(word) = self.keyword_hit(r, &keywords) {
                            add(
                                &mut out,
                                &mut keys,
                                r.ts,
                                Note {
                                    key,
                                    kind: Kind::Mention,
                                    title: trf!(
                                        "{} said \"{}\" in {}",
                                        self.who(r.author),
                                        word,
                                        place
                                    ),
                                    body: preview(r),
                                    target: format!("chat:{}:{}", c.id, m.id),
                                },
                            );
                        }
                    }
                }
                if dm && n.direct {
                    for r in m
                        .replies
                        .iter()
                        .filter(|r| r.author != me && r.imported.is_none())
                    {
                        let key = format!("chat:{}:{}", c.id, r.id);
                        if !wants(&keys, skip, &key) {
                            continue;
                        }
                        add(
                            &mut out,
                            &mut keys,
                            r.ts,
                            Note {
                                key,
                                kind: Kind::Direct,
                                title: self.who(r.author),
                                body: preview(r),
                                target: format!("chat:{}:{}", c.id, m.id),
                            },
                        );
                    }
                }
                if n.reactions {
                    for (msg, thread) in
                        std::iter::once((m, None)).chain(m.replies.iter().map(|r| (r, Some(m.id))))
                    {
                        if msg.author != me {
                            continue;
                        }
                        for re in &msg.reactions {
                            for u in re.users.iter().filter(|u| **u != me) {
                                let key = format!("react:{}:{}:{}:{}", c.id, msg.id, re.emoji, u);
                                if !wants(&keys, skip, &key) {
                                    continue;
                                }
                                let target = thread
                                    .map(|t| format!("chat:{}:{}", c.id, t))
                                    .unwrap_or_else(|| format!("chat:{}", c.id));
                                let title =
                                    trf!("{} reacted {} to your message", self.who(*u), re.emoji);
                                add(
                                    &mut out,
                                    &mut keys,
                                    i64::MAX,
                                    Note {
                                        key,
                                        kind: Kind::Reaction,
                                        title,
                                        body: preview(msg),
                                        target,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
        // ---- the Inbox: mentions, thread replies, assignments, comments and changes on my tasks
        let my_name = self
            .data
            .member(me)
            .map(|m| m.shown().to_string())
            .unwrap_or_default();
        for it in self.inbox_items().iter() {
            if it.author == me || it.imported {
                continue;
            }
            let (key, target) = match it.src {
                // two lines in the same second by the same person share an Inbox key
                InboxSrc::Task(id) => (
                    format!("task:{}:{}", it.key, it.text),
                    format!("task:{}", id),
                ),
                InboxSrc::Chat { ch, thread, .. } => {
                    if self.data.channels.iter().any(|c| {
                        c.id == ch
                            && c.kind == ChannelKind::Channel
                            && n.channel_mode(ch) == "nothing"
                    }) {
                        continue;
                    }
                    (
                        it.key.clone(),
                        thread
                            .map(|t| format!("chat:{}:{}", ch, t))
                            .unwrap_or_else(|| format!("chat:{}", ch)),
                    )
                }
            };
            if !wants(&keys, skip, &key) {
                continue;
            }
            let who = self.who(it.author);
            let task = match it.src {
                InboxSrc::Task(id) => self.task(id),
                _ => None,
            };
            let mine = every_task || task.is_some_and(|t| self.my_task(t));
            let (kind, title, body) = if it.mention {
                if !n.mentions {
                    continue;
                }
                (
                    Kind::Mention,
                    trf!("{} mentioned you in {}", who, it.title),
                    cut(&it.text, 140),
                )
            } else if it.reply {
                if !n.replies {
                    continue;
                }
                (
                    Kind::Reply,
                    trf!("{} replied in {}", who, it.title),
                    cut(&it.text, 140),
                )
            } else if it.assigned {
                if !n.assigned {
                    continue;
                }
                (
                    Kind::Assigned,
                    trf!("{} assigned you a task", who),
                    it.title.clone(),
                )
            } else if it.comment && task.is_some() {
                if !(n.comments && mine) {
                    continue;
                }
                (
                    Kind::Comment,
                    trf!("{} commented on {}", who, it.title),
                    cut(&it.text, 140),
                )
            } else if task.is_some() {
                let t = &it.text;
                let change = t.starts_with("changed status")
                    || t.starts_with("changed due date")
                    || t.starts_with("set priority")
                    || (!my_name.is_empty() && *t == format!("removed assignee {}", my_name));
                if !(n.task_changes && mine && change) {
                    continue;
                }
                (
                    Kind::TaskChange,
                    it.title.clone(),
                    crate::i18n::history_line(&who, t),
                )
            } else {
                continue;
            };
            add(
                &mut out,
                &mut keys,
                it.ts,
                Note {
                    key,
                    kind,
                    title,
                    body,
                    target,
                },
            );
        }
        // ---- people joining (for owners and admins)
        if n.team && self.i_manage() {
            for m in self
                .data
                .members
                .iter()
                .filter(|m| m.id != me && !m.invited)
            {
                let key = format!("member:{}", m.id);
                if !wants(&keys, skip, &key) {
                    continue;
                }
                let title = trf!("{} joined {}", m.shown(), self.data.workspace);
                add(
                    &mut out,
                    &mut keys,
                    i64::MAX,
                    Note {
                        key,
                        kind: Kind::Member,
                        title,
                        body: tr("Say hello in a direct message").into(),
                        target: "people".into(),
                    },
                );
            }
        }
        // ---- answers to my forms
        if n.forms {
            let mine: Vec<&Form> = self
                .data
                .extra
                .forms
                .iter()
                .filter(|f| f.owner == me)
                .collect();
            if !mine.is_empty() {
                for t in &self.data.tasks {
                    for a in t.activity.iter().filter(|a| a.author != me && !a.comment) {
                        let Some(name) = a.text.strip_prefix("submitted the form ") else {
                            continue;
                        };
                        // the form the task says it answers; by its name for
                        // answers from apps that didn't keep that
                        let to_me = match t.form.or_else(|| {
                            crate::app::more::old_answer_form(
                                &self.data.extra.forms,
                                name,
                                t.list_id,
                            )
                        }) {
                            Some(id) => mine.iter().any(|f| f.id == id),
                            // several of that name and the List doesn't tell
                            // which (as before forms were told apart): any of mine
                            None => mine.iter().any(|f| f.name == name),
                        };
                        let key = format!("form:{}", t.id);
                        if to_me && wants(&keys, skip, &key) {
                            let name = if name.trim().is_empty() {
                                tr("Untitled form")
                            } else {
                                name
                            };
                            let title = trf!("New answer to {}", name);
                            let body = trf!("{} answered: {}", self.who(a.author), t.name);
                            add(
                                &mut out,
                                &mut keys,
                                a.ts,
                                Note {
                                    key,
                                    kind: Kind::Form,
                                    title,
                                    body,
                                    target: format!("task:{}", t.id),
                                },
                            );
                        }
                    }
                }
            }
        }
        out
    }

    /// Looks for news after the team's changes came in. The first look only
    /// remembers what is there.
    /// The keyword (of mine, `NotifySettings::keywords`) a message has when
    /// it doesn't already mention me (that is told as a mention).
    fn keyword_hit(&self, m: &Message, keywords: &[String]) -> Option<String> {
        if keywords.is_empty() {
            return None;
        }
        let me = self.data.me;
        let mentions_me = if m.mentions.is_empty() {
            crate::model::mentioned_ids(&m.text, &self.data.members, &[]).contains(&me)
        } else {
            m.mentions.contains(&me)
        };
        if mentions_me {
            return None;
        }
        wb_sync::mentions::has_keyword(&m.text, keywords)
    }

    pub(super) fn notify_scan(&mut self) {
        let now = now_ts();
        let mine = self.data.extra.notify.clone();
        // what is known already would be passed over below: not made at all
        // (the first look remembers everything there is)
        let empty = HashSet::new();
        let known = if self.notify_rt.seeded {
            &self.notify_rt.seen
        } else {
            &empty
        };
        let found = self.notify_candidates_but(&mine, false, known);
        // every key, whatever the settings and watched tasks: turning a kind
        // or a task's updates on later must not bring back old things as news
        let all: Vec<String> = self
            .notify_candidates_but(&NotifySettings::everything(), true, known)
            .into_iter()
            .map(|(_, n)| n.key)
            .collect();
        if !self.notify_rt.seeded {
            self.notify_rt.seen = all
                .into_iter()
                .chain(found.into_iter().map(|(_, n)| n.key))
                .collect();
            self.notify_rt.seeded = true;
            self.notify_rt.pending = false;
            self.notify_rt.catch_up = false;
            return;
        }
        self.notify_rt.pending = false;
        let catch_up = std::mem::take(&mut self.notify_rt.catch_up);
        let mut fresh = vec![];
        for (ts, note) in found {
            if !self.notify_rt.seen.insert(note.key.clone()) {
                continue;
            }
            let limit = if note.kind == Kind::Huddle {
                HUDDLE_FRESH_SECS
            } else {
                FRESH_SECS
            };
            if ts != i64::MAX && now - ts > limit {
                continue;
            }
            fresh.push(note);
        }
        self.notify_rt.seen.extend(all);
        if catch_up {
            fresh.retain(|n| n.kind == Kind::Huddle);
        }
        // what I'm looking at right now isn't news
        let focused = !self.notify_rt.away;
        fresh.retain(|n| !(focused && self.target_visible(&n.target)));
        if fresh.len() > BURST {
            // a big batch (back from offline): one summary, and calls on their own
            let (calls, rest): (Vec<Note>, Vec<Note>) =
                fresh.into_iter().partition(|n| n.kind == Kind::Huddle);
            for c in calls {
                self.deliver(c);
            }
            if !rest.is_empty() {
                let urgent = rest
                    .iter()
                    .any(|n| matches!(n.kind, Kind::Direct | Kind::Mention));
                let title = trf!("{} new notifications", rest.len());
                // who and what they said, not just who
                let body = rest
                    .iter()
                    .take(3)
                    .map(|n| {
                        if n.body.is_empty() {
                            n.title.clone()
                        } else {
                            format!("{}: {}", n.title, cut(&n.body, 60))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                self.deliver(Note {
                    key: format!("burst:{}", now),
                    kind: if urgent { Kind::Mention } else { Kind::Comment },
                    title,
                    body,
                    target: "inbox".into(),
                });
            }
        } else {
            for n in fresh {
                self.deliver(n);
            }
        }
    }

    /// Whether what a notice opens is on screen now.
    pub(super) fn target_visible(&self, target: &str) -> bool {
        let mut p = target.split(':');
        match (p.next(), p.next().and_then(|x| x.parse::<u64>().ok())) {
            (Some("chat"), Some(ch)) => self.section == Section::Chat && self.chat.open == Some(ch),
            (Some("task"), Some(id)) => self.open_task == Some(id),
            _ => false,
        }
    }

    /// Announces one thing.
    pub(super) fn deliver(&mut self, note: Note) {
        let now = now_ts();
        let n = self.data.extra.notify.clone();
        if self.data.extra.muted_until > now {
            return;
        }
        self.notify_rt.sent.push(note.clone());
        if self.notify_rt.sent.len() > 100 {
            self.notify_rt.sent.remove(0);
        }
        if !self.notify_rt.away {
            let text = if note.body.is_empty() {
                note.title.clone()
            } else {
                format!("{}: {}", note.title, cut(&note.body, 80))
            };
            self.notify_rt.toast_target = Some((text.clone(), note.target.clone()));
            self.toast = Some((text, now));
            if n.sound && self.persist && !matches!(note.kind, Kind::Connection) {
                crate::notifier::play_sound();
            }
            return;
        }
        if !self.persist {
            return;
        }
        if n.desktop {
            crate::notifier::post(
                &note.title,
                &note.body,
                &note.target,
                n.sound && note.kind != Kind::Connection,
            );
        }
        match note.kind {
            // a call in a direct message rings until seen
            Kind::Huddle if self.dm_target(&note.target) => crate::notifier::bounce(true),
            Kind::Direct | Kind::Mention | Kind::Huddle => crate::notifier::bounce(false),
            _ => {}
        }
    }

    fn dm_target(&self, target: &str) -> bool {
        let ch = target.split(':').nth(1).and_then(|x| x.parse::<u64>().ok());
        target.starts_with("chat:")
            && self
                .data
                .channels
                .iter()
                .any(|c| Some(c.id) == ch && c.kind == ChannelKind::Direct)
    }

    /// Opens what a notice is about (a banner click, a toast click).
    pub(super) fn open_target(&mut self, target: &str) {
        // from another workspace (watch.rs)
        if let Some((file, inner)) = super::watch::split_target(target) {
            let (file, inner) = (file.to_string(), inner.to_string());
            return self.open_in_workspace(&file, &inner);
        }
        let mut p = target.split(':');
        let what = p.next().unwrap_or("");
        let a = p.next().and_then(|x| x.parse::<u64>().ok());
        let b = p.next().and_then(|x| x.parse::<u64>().ok());
        match (what, a) {
            ("task", Some(id)) if self.task(id).is_some() => self.actions.push(Action::Open(id)),
            ("chat", Some(ch)) => self.open_chat_at(ch, b),
            ("meeting", Some(id)) => {
                self.section = Section::Meetings;
                self.meeting_open = Some(id);
            }
            ("goal", Some(id)) => {
                self.section = Section::Goals;
                self.goal.open = Some(id);
            }
            ("reminder", Some(id)) => match self
                .data
                .extra
                .reminders
                .iter()
                .find(|r| r.id == id)
                .cloned()
            {
                Some(r) if r.task.is_some() || r.message.is_some() => self.open_reminder(&r),
                _ => self.actions.push(Action::Go(Location::Inbox)),
            },
            ("today", _) => self.actions.push(Action::Go(Location::TodayOverdue)),
            ("people", _) => self.section = Section::People,
            ("together", _) => self.modal = Some(self.together_modal()),
            _ => self.actions.push(Action::Go(Location::Inbox)),
        }
    }

    fn fired(&self, key: &str) -> bool {
        self.data.extra.notify.fired.iter().any(|k| k == key)
    }

    fn mark_fired(&mut self, key: String) {
        let goals: std::collections::HashSet<u64> =
            self.data.extra.goals.iter().map(|g| g.id).collect();
        let f = &mut self.data.extra.notify.fired;
        f.push(key);
        if f.len() > FIRED_KEEP {
            // the oldest go, but not a goal's while the goal is there:
            // "far behind" (once per due date) and a milestone (once ever)
            // would otherwise be told again months later
            let mut extra = f.len() - FIRED_KEEP;
            f.retain(|k| {
                let keep = goal_key(k).is_some_and(|id| goals.contains(&id));
                if extra > 0 && !keep {
                    extra -= 1;
                    false
                } else {
                    true
                }
            });
        }
        self.dirty = true;
    }

    /// Notices on the clock: my tasks due today, meetings about to start,
    /// goals coming due. `now` is passed in so tests can move the clock.
    pub(super) fn notify_clock(&mut self, now: i64) {
        self.goal_watch(now);
        let n = self.data.extra.notify.clone();
        let me = self.data.me;
        let local = local(now).unwrap_or_else(chrono::Local::now);
        let day = local.date_naive();
        let after_due_time = parse_time_of_day(&n.due_time).is_some_and(|at| local.time() >= at);
        // ---- tasks due today and overdue, once a day
        let key = format!("due:{}", day);
        if n.due && after_due_time && !self.fired(&key) {
            self.mark_fired(key.clone());
            let mine: Vec<&Task> = self
                .data
                .tasks
                .iter()
                .filter(|t| {
                    t.assignees.contains(&me)
                        && !t.archived
                        && !self.data.is_closed(t)
                        && t.due.is_some_and(|d| d <= day)
                })
                .collect();
            if !mine.is_empty() {
                // due today with a time that has passed counts as overdue
                let now_dt = local.naive_local();
                let today_n = mine
                    .iter()
                    .filter(|t| t.due == Some(day) && !t.is_overdue_at(now_dt))
                    .count();
                let late = mine.len() - today_n;
                let title = match (today_n, late) {
                    (1, 0) => tr("1 task is due today").to_string(),
                    (d, 0) => trf!("{} tasks are due today", d),
                    (0, 1) => tr("1 overdue task").to_string(),
                    (0, l) => trf!("{} overdue tasks", l),
                    (d, l) => trf!("{} due today, {} overdue", d, l),
                };
                let body = mine
                    .iter()
                    .take(3)
                    .map(|t| t.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.deliver(Note {
                    key,
                    kind: Kind::Due,
                    title,
                    body,
                    target: "today".into(),
                });
            }
        }
        // ---- my tasks with a due time, when that time comes (one missed
        // by more than an hour, the app being closed, is left to the daily
        // notice above)
        if n.due {
            let now_dt = local.naive_local();
            let due_now: Vec<(u64, String, String, String)> = self
                .data
                .tasks
                .iter()
                .filter(|t| t.assignees.contains(&me) && !t.archived && !self.data.is_closed(t))
                .filter_map(|t| {
                    let (d, at) = (t.due?, t.due_at()?);
                    let when = d.and_time(at);
                    (when <= now_dt && now_dt - when < chrono::Duration::hours(1)).then(|| {
                        (
                            t.id,
                            t.name.clone(),
                            fmt_when(d, Some(at)),
                            when.format("%Y-%m-%dT%H:%M").to_string(),
                        )
                    })
                })
                .collect();
            for (id, name, when, stamp) in due_now {
                let key = format!("due-at:{}:{}", id, stamp);
                if self.fired(&key) {
                    continue;
                }
                self.mark_fired(key.clone());
                self.deliver(Note {
                    key,
                    kind: Kind::Due,
                    title: trf!("{} is due now", name),
                    body: trf!("Due {}", when),
                    target: format!("task:{}", id),
                });
            }
        }
        // ---- meetings about to start
        if n.meetings {
            let lead = n.meeting_minutes.max(1) as i64 * 60;
            let soon: Vec<Meeting> = self
                .data
                .extra
                .meetings
                .iter()
                .filter(|m| m.attendees.contains(&me))
                .cloned()
                .collect();
            for m in soon {
                let Some(start) = parse_time_of_day(&m.time).and_then(|at| local_ts(m.date, at))
                else {
                    continue;
                };
                let key = format!("meeting:{}:{}T{}", m.id, m.date, m.time);
                if now >= start - lead && now < start && !self.fired(&key) {
                    self.mark_fired(key.clone());
                    let mins = ((start - now + 59) / 60).max(1);
                    let title = if mins == 1 {
                        trf!("{} starts in 1 minute", m.title)
                    } else {
                        trf!("{} starts in {} minutes", m.title, mins)
                    };
                    let body = if m.agenda.trim().is_empty() {
                        m.time.clone()
                    } else {
                        cut(m.agenda.trim(), 140)
                    };
                    self.deliver(Note {
                        key,
                        kind: Kind::Meeting,
                        title,
                        body,
                        target: format!("meeting:{}", m.id),
                    });
                }
            }
        }
        // ---- my goals due tomorrow and today
        if n.goals && after_due_time {
            let goals: Vec<Goal> = self
                .data
                .extra
                .goals
                .iter()
                .filter(|g| g.owner == me && g.due.is_some())
                .cloned()
                .collect();
            for g in goals {
                let due = g.due.unwrap();
                let days = (due - day).num_days();
                if !(0..=1).contains(&days) || self.goal_progress(&g) >= 1.0 {
                    continue;
                }
                let key = format!("goal:{}:{}:{}", g.id, due, days);
                if self.fired(&key) {
                    continue;
                }
                self.mark_fired(key.clone());
                let title = if days == 0 {
                    trf!("Goal due today: {}", g.name)
                } else {
                    trf!("Goal due tomorrow: {}", g.name)
                };
                let body = trf!("{}% done", super::more::pct_of(self.goal_progress(&g)));
                self.deliver(Note {
                    key,
                    kind: Kind::Goal,
                    title,
                    body,
                    target: format!("goal:{}", g.id),
                });
            }
        }
        // ---- my goals passing a milestone (one I didn't record myself:
        // those show on the goal's page), and falling far behind
        if n.goal_milestones || (n.goal_risk && after_due_time) {
            let goals: Vec<Goal> = self
                .data
                .extra
                .goals
                .iter()
                .filter(|g| g.owner == me)
                .cloned()
                .collect();
            for g in goals {
                let p = self.goal_progress(&g);
                let pct = super::more::pct_of(p);
                if n.goal_milestones {
                    for e in &g.log {
                        let GoalChange::Milestone(m) = e.what else {
                            continue;
                        };
                        // news, not history: a day old at most
                        if e.who == me || now - e.ts > 86_400 {
                            continue;
                        }
                        let key = format!("goalm:{}:{}", g.id, m);
                        if self.fired(&key) {
                            continue;
                        }
                        self.mark_fired(key.clone());
                        let (title, body) = if m >= 100 {
                            (
                                trf!("Goal achieved: {}", g.name),
                                tr("All targets met").to_string(),
                            )
                        } else {
                            (trf!("{} reached {}%", g.name, m), trf!("{}% done", pct))
                        };
                        self.deliver(Note {
                            key,
                            kind: Kind::Goal,
                            title,
                            body,
                            target: format!("goal:{}", g.id),
                        });
                    }
                }
                if n.goal_risk && after_due_time && g.pace(p, day) == Some(Pace::AtRisk) {
                    let Some(due) = g.due else {
                        continue;
                    };
                    let key = format!("goalrisk:{}:{}", g.id, due);
                    if self.fired(&key) {
                        continue;
                    }
                    self.mark_fired(key.clone());
                    let left = (due - day).num_days().max(0);
                    self.deliver(Note {
                        key,
                        kind: Kind::Goal,
                        title: trf!("Goal at risk: {}", g.name),
                        body: trf!("{}% done, {} days left", pct, left),
                        target: format!("goal:{}", g.id),
                    });
                }
            }
        }
    }

    /// The team server connection: announced when it has been down a while,
    /// and when it comes back after that.
    pub(super) fn notify_connection(&mut self, now: i64) {
        let on_server = self.on_server();
        let status = self.sync_rt.net_status.clone();
        match status {
            Some(NetStatus::Connected) => {
                self.notify_rt.lost_since = None;
                if std::mem::take(&mut self.notify_rt.told_lost)
                    && self.data.extra.notify.connection
                {
                    self.deliver(Note {
                        key: format!("net:back:{}", now),
                        kind: Kind::Connection,
                        title: tr("Connected to the team server again").into(),
                        body: tr("Your changes are being shared again.").into(),
                        target: "together".into(),
                    });
                }
            }
            Some(NetStatus::Reconnecting(_)) if on_server => {
                let since = *self.notify_rt.lost_since.get_or_insert(now);
                if now - since >= LOST_SECS && !self.notify_rt.told_lost {
                    self.notify_rt.told_lost = true;
                    if self.data.extra.notify.connection {
                        self.deliver(Note {
                            key: format!("net:lost:{}", now),
                            kind: Kind::Connection,
                            title: tr("Lost the connection to the team server").into(),
                            body: tr("Keep working: your changes are shared when it comes back.")
                                .into(),
                            target: "together".into(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// The number on the Dock icon: the Inbox badge plus unread direct messages.
    pub(super) fn dock_count(&self) -> usize {
        if !self.data.extra.notify.dock_badge || self.data.extra.muted_until > now_ts() {
            return 0;
        }
        let me = self.data.me;
        let dms: usize = self
            .data
            .channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Direct && c.members.contains(&me))
            .filter(|c| !self.reading_now(c))
            // the sidebar's rule: messages not seen by id, so a late one
            // (its author's clock behind) still counts and a read one doesn't
            .map(|c| self.unread_here(c))
            .sum();
        self.inbox_badge() + dms
    }

    /// Once a frame: banner clicks, news after changes came in, the clock,
    /// the Dock badge.
    pub(super) fn notify_tick(&mut self, ctx: Option<&egui::Context>) {
        if let Some(ctx) = ctx {
            // the thread's own window or the small window in front counts
            // as the app's
            self.notify_rt.away = !ctx.input(|i| i.viewport().focused.unwrap_or(true))
                && !self.chat.thread_window_focused
                && !self.mini.focused;
        }
        for target in crate::notifier::take_clicked() {
            self.open_target(&target);
        }
        if !self.notify_rt.seeded || self.notify_rt.pending {
            self.notify_scan();
        }
        let now = now_ts();
        self.notify_connection(now);
        if now - self.notify_rt.last_clock >= CLOCK_SECS {
            self.notify_rt.last_clock = now;
            self.notify_clock(now);
        }
        if self.persist {
            let c = self.dock_count();
            crate::notifier::set_badge(&if c == 0 {
                String::new()
            } else if c > 99 {
                "99+".into()
            } else {
                c.to_string()
            });
        }
        if let Some(ctx) = ctx {
            ctx.request_repaint_after(std::time::Duration::from_secs(CLOCK_SECS as u64));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{set_lang, Lang};

    #[test]
    fn attachment_only_messages_say_what_was_sent() {
        let one = |kind| crate::model::Attachment {
            kind,
            ..Default::default()
        };
        set_lang(Lang::Ko);
        assert_eq!(sent_what(&[one(MediaKind::Image)]), "사진을 보냈습니다");
        assert_eq!(sent_what(&[one(MediaKind::Video)]), "영상을 보냈습니다");
        assert_eq!(
            sent_what(&[one(MediaKind::Image), one(MediaKind::Video)]),
            "사진과 영상을 보냈습니다"
        );
        assert_eq!(
            sent_what(&[one(MediaKind::Video), one(MediaKind::File)]),
            "파일을 보냈습니다"
        );
        set_lang(Lang::En);
        assert_eq!(sent_what(&[one(MediaKind::Video)]), "Sent a video");
    }
}
