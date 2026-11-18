// Notifications. When the team's changes come in, `notify_scan` works out
// what is new for me (messages, mentions, replies, huddles, reactions,
// assignments, comments and changes on my tasks, new teammates, form
// answers), and `notify_clock` watches the clock (tasks due today, meetings
// about to start, goals coming due, the server connection). Each thing has a
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

/// A message's text cut to banner length (a picture or video says so).
fn preview(m: &Message) -> String {
    let shown = super::chat::shown_text(m);
    let text = shown.trim();
    let text = if text.is_empty() && !m.attachments.is_empty() {
        tr("Sent a picture or video").to_string()
    } else {
        text.to_string()
    };
    cut(&text, 140)
}

fn cut(s: &str, n: usize) -> String {
    truncate_chars(&s.replace('\n', " "), n)
}

impl App {
    /// Everything that is news for me in the data as it is, with when it
    /// happened (i64::MAX: not known). Turned-off kinds are left out.
    pub(super) fn notify_candidates(&self, n: &NotifySettings) -> Vec<(i64, Note)> {
        let me = self.data.me;
        let mut out = vec![];
        let mut keys = HashSet::new();
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
                if m.author != me && m.imported.is_none() {
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
                    }
                }
                if dm && n.direct {
                    for r in m
                        .replies
                        .iter()
                        .filter(|r| r.author != me && r.imported.is_none())
                    {
                        let key = format!("chat:{}:{}", c.id, r.id);
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
            .map(|m| m.name.clone())
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
            let who = self.who(it.author);
            let task = match it.src {
                InboxSrc::Task(id) => self.task(id),
                _ => None,
            };
            let mine = task.is_some_and(|t| self.my_task(t));
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
                let title = trf!("{} joined {}", m.name, self.data.workspace);
                add(
                    &mut out,
                    &mut keys,
                    i64::MAX,
                    Note {
                        key: format!("member:{}", m.id),
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
                        if mine.iter().any(|f| f.name == name) {
                            let title = trf!("New answer to {}", name);
                            let body = trf!("{} answered: {}", self.who(a.author), t.name);
                            add(
                                &mut out,
                                &mut keys,
                                a.ts,
                                Note {
                                    key: format!("form:{}", t.id),
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
    pub(super) fn notify_scan(&mut self) {
        let now = now_ts();
        let mine = self.data.extra.notify.clone();
        let found = self.notify_candidates(&mine);
        // every key, whatever the settings: turning a kind on later must not
        // bring back old things as news
        let all: Vec<String> = self
            .notify_candidates(&NotifySettings::everything())
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
            return;
        }
        self.notify_rt.pending = false;
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
                let body = rest
                    .iter()
                    .take(3)
                    .map(|n| n.title.clone())
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
                self.goal_open = Some(id);
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
        let f = &mut self.data.extra.notify.fired;
        f.push(key);
        if f.len() > FIRED_KEEP {
            let extra = f.len() - FIRED_KEEP;
            f.drain(..extra);
        }
        self.dirty = true;
    }

    /// Notices on the clock: my tasks due today, meetings about to start,
    /// goals coming due. `now` is passed in so tests can move the clock.
    pub(super) fn notify_clock(&mut self, now: i64) {
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
                let body = trf!("{}% done", (self.goal_progress(&g) * 100.0).round() as i64);
                self.deliver(Note {
                    key,
                    kind: Kind::Goal,
                    title,
                    body,
                    target: format!("goal:{}", g.id),
                });
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
            self.notify_rt.away = !ctx.input(|i| i.viewport().focused.unwrap_or(true));
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
