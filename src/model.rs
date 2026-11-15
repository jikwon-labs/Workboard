// Data model for the workspace: spaces, folders, lists, tasks and members.
// Default statuses and priorities follow the values recorded in MECHANICS.md.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusKind {
    Open,
    Custom,
    Done,
    Closed,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Status {
    pub name: String,
    pub color: [u8; 3],
    pub kind: StatusKind,
}

impl Status {
    pub fn new(name: &str, color: [u8; 3], kind: StatusKind) -> Self {
        Self {
            name: name.to_string(),
            color,
            kind,
        }
    }
}

/// Status palette (open grey first, then the coloured choices).
pub const STATUS_PALETTE: [[u8; 3]; 14] = [
    [0x87, 0x90, 0x9e],
    [0x5f, 0x55, 0xee],
    [0x44, 0x66, 0xff],
    [0x10, 0x90, 0xe0],
    [0x0f, 0x9d, 0x9f],
    [0x3d, 0xb8, 0x8b],
    [0x00, 0x88, 0x44],
    [0xf8, 0xae, 0x00],
    [0xe1, 0x6b, 0x16],
    [0xd3, 0x3d, 0x44],
    [0xee, 0x5e, 0x99],
    [0xb6, 0x60, 0xe0],
    [0xaa, 0x8d, 0x80],
    [0x65, 0x6f, 0x7d],
];

/// Default status colours as the person's own ClickUp workspace shows them
/// (measured from the running app): in progress rgb(8,128,234), complete rgb(41,151,100).
pub const IN_PROGRESS_COLOR: [u8; 3] = [8, 128, 234];
pub const COMPLETE_COLOR: [u8; 3] = [41, 151, 100];

pub fn default_statuses() -> Vec<Status> {
    vec![
        Status::new("to do", [0x87, 0x90, 0x9e], StatusKind::Open),
        Status::new("in progress", IN_PROGRESS_COLOR, StatusKind::Custom),
        Status::new("complete", COMPLETE_COLOR, StatusKind::Closed),
    ]
}

/// Priority levels: 0 = none, 1 = urgent, 2 = high, 3 = normal, 4 = low.
pub struct PriorityDef {
    pub label: &'static str,
    pub color: [u8; 3],
}

pub const PRIORITIES: [PriorityDef; 5] = [
    PriorityDef {
        label: "No Priority",
        color: [0x87, 0x90, 0x9e],
    },
    PriorityDef {
        label: "Urgent",
        color: [0xf5, 0x00, 0x00],
    },
    PriorityDef {
        label: "High",
        color: [0xff, 0xcc, 0x00],
    },
    PriorityDef {
        label: "Normal",
        color: [0x6f, 0xdd, 0xff],
    },
    PriorityDef {
        label: "Low",
        color: [0xd8, 0xd8, 0xd8],
    },
];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ListItem {
    pub id: u64,
    pub name: String,
    /// List details shown on the Space Overview Lists card.
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default)]
    pub start: Option<NaiveDate>,
    #[serde(default)]
    pub end: Option<NaiveDate>,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub owner: Option<u64>,
    /// Shown with List Info.
    #[serde(default)]
    pub description: String,
    /// Archived lists leave the sidebar until restored (Settings > Spaces).
    #[serde(default)]
    pub archived: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Folder {
    pub id: u64,
    pub name: String,
    pub lists: Vec<ListItem>,
    #[serde(default = "yes")]
    pub expanded: bool,
    /// Archived folders leave the sidebar (with their lists) until restored.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Space {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    pub statuses: Vec<Status>,
    pub folders: Vec<Folder>,
    pub lists: Vec<ListItem>,
    #[serde(default = "yes")]
    pub expanded: bool,
    /// A private space that holds one person's Personal List (hidden from Spaces).
    #[serde(default)]
    pub private_owner: Option<u64>,
    /// Archived spaces leave the sidebar and task views until restored.
    #[serde(default)]
    pub archived: bool,
    /// Hidden from the sidebar only (Hide Space); shown again from Settings > Spaces.
    #[serde(default)]
    pub hidden: bool,
    /// A symbol drawn in place of the first letter (Color & Icon); empty = letter.
    #[serde(default)]
    pub icon: String,
    /// Who the space is shared with (Share & permissions). Left out of the
    /// file until first shared; spaces without it are everyone's. Going
    /// back to everyone keeps it with `everyone: true`, so it never goes
    /// back to missing (an older app's copy, which drops it, can't open a
    /// shared space up; see `wb_sync::space_level`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sharing: Option<SpaceSharing>,
}

/// Who a space is shared with (see `Space::sharing`).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SpaceSharing {
    /// Everyone in the workspace; the people below are kept for next time.
    #[serde(default)]
    pub everyone: bool,
    #[serde(default)]
    pub people: Vec<SpaceGrant>,
}

/// One person a space is shared with and what they may do there.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SpaceGrant {
    pub member: u64,
    /// "full", "edit", "comment" or "view" (`wb_sync::Level`); kept as
    /// written, so a level from a newer app survives a save here.
    pub level: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Member {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    #[serde(default)]
    pub email: String,
    /// Owner, Admin, Member or Guest.
    #[serde(default)]
    pub role: String,
    /// Invited by email and not joined yet.
    #[serde(default)]
    pub invited: bool,
    #[serde(default)]
    pub status_text: String,
    /// The Slack user ids whose messages an Import from Slack brought in as
    /// this member, so importing again (from any teammate's computer) finds
    /// them here even after a rename or with no email to match by.
    /// A Slack user id is on one member at most: pointing them at another
    /// member takes it off this one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slack: Vec<String>,
    /// Only read, from files of an earlier version that kept "Import under
    /// my name" here, on the shared member: my own member's list moves to
    /// my personal `Extra::slack_under_me` (`Data::keep_slack_under_me_personal`)
    /// and nothing writes it any more.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slack_mine: Vec<String>,
}

/// The people a text @mentions, by the rule the team server uses too
/// (`wb_sync::mentions`): at each "@" that starts a word the longest
/// matching name wins, so "@Alex Kim" is Alex Kim and not Alex; "@Alexander"
/// is not Alex but "@지원님" is 지원. When several people share the name, the
/// ones in `picked` (chosen from the mention list) are meant, else all.
pub fn mentioned_ids(text: &str, members: &[Member], picked: &[u64]) -> Vec<u64> {
    let people: Vec<(u64, &str)> = members.iter().map(|m| (m.id, m.name.as_str())).collect();
    wb_sync::mentions::mentioned_ids(text, &people, picked)
}

impl Member {
    pub fn initials(&self) -> String {
        let mut s: String = self
            .name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect();
        s = s.to_uppercase();
        if s.is_empty() {
            "?".into()
        } else {
            s
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CheckItem {
    pub text: String,
    pub done: bool,
    /// The one person this item is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Activity {
    pub ts: i64,
    pub author: u64,
    pub text: String,
    pub comment: bool,
    /// Written from the comment box's Post tab (shown as a post).
    #[serde(default)]
    pub post: bool,
    /// So a comment can be edited, deleted, reacted to and replied to, and
    /// so wb-sync merges comments one by one. 0 until `Data::fix_activity_ids`
    /// gives it one (and then left out of the file, so an entry without an
    /// id never looks like one with id 0 to wb-sync).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub id: u64,
    /// When its author last changed the text (shown as "(edited)").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<Reaction>,
    /// The comment this one replies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
    /// Someone asked to act on this comment (Assigned Comments).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<u64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resolved: bool,
    /// The person a history line is about ("assigned to ..."), so the Inbox
    /// finds them by id even after a rename or when two share a name. Lines
    /// from before this was kept have none and are matched by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<u64>,
    /// Who a comment @mentions, worked out when it was written (see
    /// `mentioned_ids`). Empty for comments from before this was kept; those
    /// are matched by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<u64>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// A time of day typed or saved as "14:30", "14:30:00", "2:30 pm", "2pm"
/// or "14". None when it isn't one.
pub fn parse_time_of_day(s: &str) -> Option<NaiveTime> {
    let s = s.trim().to_lowercase().replace(' ', "");
    let (s, half) = if let Some(x) = s.strip_suffix("am") {
        (x, Some(false))
    } else if let Some(x) = s.strip_suffix("pm") {
        (x, Some(true))
    } else {
        (s.as_str(), None)
    };
    let mut parts = s.split(':');
    let h: u32 = parts.next()?.parse().ok()?;
    let m: u32 = match parts.next() {
        Some(m) => m.parse().ok()?,
        None => 0,
    };
    let sec: u32 = match parts.next() {
        Some(x) => x.split('.').next()?.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    let h = match half {
        Some(pm) if (1..=12).contains(&h) => h % 12 + if pm { 12 } else { 0 },
        Some(_) => return None,
        None => h,
    };
    NaiveTime::from_hms_opt(h, m, sec)
}

/// `Task::start_time` / `due_time` in the file: there (a time, or null once
/// removed) or missing. A value this app can't read counts as removed
/// rather than failing the whole file.
fn time_slot<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<NaiveTime>>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(Some(v.and_then(|v| v.as_str().and_then(parse_time_of_day))))
}

/// Writes a time of day as "14:30".
fn write_time_slot<S: serde::Serializer>(
    t: &Option<Option<NaiveTime>>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match t {
        Some(Some(t)) => s.serialize_str(&t.format("%H:%M").to_string()),
        _ => s.serialize_none(),
    }
}

/// Sets a time slot: removing one that was never set leaves it missing.
pub fn set_slot(slot: &mut Option<Option<NaiveTime>>, t: Option<NaiveTime>) {
    *slot = match t {
        Some(t) => Some(Some(
            t.with_second(0)
                .unwrap_or(t)
                .with_nanosecond(0)
                .unwrap_or(t),
        )),
        None => slot.map(|_| None),
    };
}

/// The local date and time now.
pub fn now_local() -> NaiveDateTime {
    Local::now().naive_local()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TimeEntry {
    pub start: i64,
    pub end: i64,
    /// Whose time this is; 0 for entries from before entries had an owner
    /// (see `Task::entry_owner`). Left out of the file when 0, so entries
    /// read the same to wb-sync as before the field existed.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub member: u64,
}

/// A timer someone runs on a task (see `Task::timers`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct RunningTimer {
    pub member: u64,
    pub start: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Task {
    pub id: u64,
    pub list_id: u64,
    pub parent: Option<u64>,
    pub name: String,
    pub description: String,
    pub status: String,
    pub priority: u8,
    pub assignees: Vec<u64>,
    pub start: Option<NaiveDate>,
    pub due: Option<NaiveDate>,
    /// Time of day on `start` / `due`, when one was picked ("14:30"). Only
    /// means something with its date: read them through `start_at` /
    /// `due_at`. Missing until first set; removing a time writes null rather
    /// than leaving it out, so it never goes back to missing and wb-sync
    /// needs no default rule for it (like `Doc::wiki`). Older apps don't
    /// read them and keep the date alone.
    #[serde(
        default,
        deserialize_with = "time_slot",
        serialize_with = "write_time_slot",
        skip_serializing_if = "Option::is_none"
    )]
    pub start_time: Option<Option<NaiveTime>>,
    #[serde(
        default,
        deserialize_with = "time_slot",
        serialize_with = "write_time_slot",
        skip_serializing_if = "Option::is_none"
    )]
    pub due_time: Option<Option<NaiveTime>>,
    pub tags: Vec<String>,
    pub checklist: Vec<CheckItem>,
    pub activity: Vec<Activity>,
    pub time_entries: Vec<TimeEntry>,
    pub timer_started: Option<i64>,
    /// Who started the running timer; None for timers started before this
    /// was recorded (see `Data::is_my_timer`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_by: Option<u64>,
    /// Timers more people run on the task at once, one per person, besides
    /// the one in `timer_started` (which older apps still read).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timers: Vec<RunningTimer>,
    pub estimate_min: Option<u32>,
    pub created: i64,
    pub updated: i64,
    pub order: i64,
    #[serde(default)]
    pub custom: std::collections::BTreeMap<u64, String>,
    /// Repeats when closed: daily, weekly, monthly or yearly.
    #[serde(default)]
    pub recur: Option<String>,
    /// Archived tasks stay out of every view until unarchived.
    #[serde(default)]
    pub archived: bool,
    /// When the task last moved into a done or closed status (Date done).
    #[serde(default)]
    pub done_at: Option<i64>,
    /// When the task last moved into a closed status (Date closed).
    #[serde(default)]
    pub closed_at: Option<i64>,
    /// Task type: a milestone instead of a plain task.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub milestone: bool,
    /// People following the task besides its assignees.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub watchers: Vec<u64>,
    /// Tasks this one waits on. The other side shows it as Blocking, so
    /// only this side is stored.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waiting_on: Vec<u64>,
    /// Related tasks; shown on both sides but stored on one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linked: Vec<u64>,
    /// Pictures and videos attached to the task (see Attachment on ids).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
}

/// `Data::fix_activity_ids` for one task.
pub fn fix_task_activity_ids(t: &mut Task) {
    let mut seen = std::collections::HashSet::new();
    if t.activity.iter().all(|a| a.id != 0 && seen.insert(a.id)) {
        return;
    }
    let mut counts: std::collections::HashMap<(i64, u64, bool, String), usize> = Default::default();
    let mut seen = std::collections::HashSet::new();
    let tid = t.id;
    t.activity.retain_mut(|a| {
        let n = counts
            .entry((a.ts, a.author, a.comment, a.text.clone()))
            .or_insert(0);
        if a.id == 0 {
            a.id = wb_sync::activity_id(tid, a.ts, a.author, a.comment, &a.text, *n);
        }
        *n += 1;
        seen.insert(a.id)
    });
}

impl Task {
    /// The time of day on the start date, if it has both.
    pub fn start_at(&self) -> Option<NaiveTime> {
        self.start.and(self.start_time.flatten())
    }

    /// The time of day on the due date, if it has both.
    pub fn due_at(&self) -> Option<NaiveTime> {
        self.due.and(self.due_time.flatten())
    }

    pub fn set_start_time(&mut self, t: Option<NaiveTime>) {
        set_slot(&mut self.start_time, t);
    }

    pub fn set_due_time(&mut self, t: Option<NaiveTime>) {
        set_slot(&mut self.due_time, t);
    }

    /// After an edit (`old` is the task before it): a date that was cleared
    /// takes its time with it, and a date set where there was none doesn't
    /// bring back a time left over from before.
    pub fn tidy_times(&mut self, old: &Task) {
        if self.due.is_none() || (old.due.is_none() && self.due_time == old.due_time) {
            self.set_due_time(None);
        }
        if self.start.is_none() || (old.start.is_none() && self.start_time == old.start_time) {
            self.set_start_time(None);
        }
    }

    /// When the task is due: its due date at its due time, or without a time
    /// the end of that day (the next midnight). Orders tasks by due date.
    pub fn due_moment(&self) -> Option<NaiveDateTime> {
        let d = self.due?;
        Some(match self.due_at() {
            Some(t) => d.and_time(t),
            None => (d + Duration::days(1)).and_time(NaiveTime::MIN),
        })
    }

    /// Past due at `now`: a day that has gone by, or today after its due time.
    /// Says nothing about whether the task is closed.
    pub fn is_overdue_at(&self, now: NaiveDateTime) -> bool {
        self.due_moment().is_some_and(|m| m <= now)
    }

    pub fn is_overdue(&self) -> bool {
        self.is_overdue_at(now_local())
    }

    pub fn tracked_secs(&self, now: i64) -> i64 {
        let mut s: i64 = self.time_entries.iter().map(|e| e.end - e.start).sum();
        if let Some(st) = self.timer_started {
            s += now - st;
        }
        s += self.timers.iter().map(|r| now - r.start).sum::<i64>();
        s
    }

    /// Every running timer: who started it (None for an old timer that
    /// didn't record it) and when.
    pub fn running_timers(&self) -> Vec<(Option<u64>, i64)> {
        self.timer_started
            .map(|st| (self.timer_by, st))
            .into_iter()
            .chain(self.timers.iter().map(|r| (Some(r.member), r.start)))
            .collect()
    }

    /// Who a time entry belongs to: its owner, or for an entry from before
    /// entries had one, the task's first assignee (0 when there is none).
    pub fn entry_owner(&self, e: &TimeEntry) -> u64 {
        if e.member != 0 {
            e.member
        } else {
            self.assignees.first().copied().unwrap_or(0)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Doc {
    pub id: u64,
    #[serde(default)]
    pub space: Option<u64>,
    #[serde(default)]
    pub owner: u64,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub meeting_note: bool,
    /// The page this one sits under (sub pages), if any.
    #[serde(default)]
    pub parent: Option<u64>,
    pub title: String,
    pub body: String,
    pub created: i64,
    pub updated: i64,
    /// Labels shown in the Docs list's Tags column and used by its filter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Marked as a Wiki (Popular Wikis, the Wikis filter). Left out until
    /// first marked; unmarking writes `false` rather than leaving it out, so
    /// it never goes back to missing and needs no wb-sync default rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wiki: Option<bool>,
    /// Who opened the doc and when: one entry per person (and device), each
    /// changed only by its owner, so wb-sync merges them one by one and no
    /// view is lost when two people read the doc at once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<DocViews>,
}

/// One person's recent opens of a doc (see `Doc::views`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DocViews {
    pub id: u64,
    pub member: u64,
    /// When the doc was opened, oldest first; opens older than
    /// `DOC_VIEW_DAYS` are dropped the next time this person opens it.
    #[serde(default)]
    pub at: Vec<i64>,
}

/// How far back Popular Wikis counts views.
pub const DOC_VIEW_DAYS: i64 = 30;

impl Doc {
    pub fn is_wiki(&self) -> bool {
        self.wiki == Some(true)
    }

    /// Opens by everyone since `since`.
    pub fn views_since(&self, since: i64) -> usize {
        self.views
            .iter()
            .map(|v| v.at.iter().filter(|t| **t >= since).count())
            .sum()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Reaction {
    pub emoji: String,
    pub users: Vec<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub id: u64,
    pub author: u64,
    pub text: String,
    pub ts: i64,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    #[serde(default)]
    pub replies: Vec<Message>,
    #[serde(default)]
    pub saved: bool,
    #[serde(default)]
    pub task: Option<u64>,
    /// A post (announcement with a title) instead of a plain message.
    #[serde(default)]
    pub post_title: Option<String>,
    #[serde(default)]
    pub post_type: String,
    #[serde(default)]
    pub pinned: bool,
    /// A call someone started; others join from this link.
    #[serde(default)]
    pub call: Option<String>,
    /// Pictures and videos. The files live on the team server (or in the
    /// shared folder); the message only says which ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    /// When its author last changed the text (shown as "(edited)").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<i64>,
    /// Who the message @mentions, worked out when it was written (see
    /// `mentioned_ids`). Empty for messages from before this was kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<u64>,
    /// Set on a message brought in by an import: where it came from
    /// ("slack:<channel>:<ts>", unique per message). Imported messages are
    /// old history, not news: they don't fill the Inbox, notifications,
    /// unread counts or badges. Set once and never cleared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported: Option<String>,
}

pub use crate::media::MediaKind;

/// One picture or video in a message. There is deliberately no `id` key:
/// wb-sync tracks arrays of objects with ids item by item, so an app that
/// doesn't know attachments would mark them deleted for everyone. Without
/// ids the array is a plain field and survives such an app.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Attachment {
    /// sha256 of the contents (hex); also the file's name on the server.
    pub file: String,
    pub name: String,
    #[serde(default)]
    pub kind: MediaKind,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub w: u32,
    #[serde(default)]
    pub h: u32,
    /// Length of a video in seconds; 0 for a picture.
    #[serde(default)]
    pub secs: f32,
    /// A video's still (a small JPEG, its own file by sha256 like `file`),
    /// so others can show the video without fetching all of it. None for
    /// pictures and for videos sent before stills were made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChannelKind {
    #[default]
    Channel,
    Direct,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Channel {
    pub id: u64,
    pub name: String,
    pub kind: ChannelKind,
    pub topic: String,
    pub members: Vec<u64>,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub last_read: i64,
    /// Made with "Make Private": only its members see it (a direct message is
    /// always seen only by the people in it).
    #[serde(default)]
    pub private: bool,
    /// The List made with "Add a List" and tied to this channel.
    #[serde(default)]
    pub list: Option<u64>,
    /// Who made it (0: made before this was kept). Only they (or, for 0,
    /// any member) and the server's owner and admins may delete it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub creator: u64,
}

/// The messages I've seen in a conversation, to tell new ones by id rather
/// than by their clock: the newest `SeenIds::MAX` ids (sorted) and the time
/// of the oldest of them. A message from `floor` on that isn't among `ids`
/// is new, even when its author's clock was behind. Only on this computer.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SeenIds {
    pub ids: Vec<u64>,
    pub floor: i64,
}

impl SeenIds {
    pub const MAX: usize = 300;

    /// The newest MAX of these (id, time) pairs (all when there are fewer;
    /// then everything counts as covered).
    pub fn of(msgs: impl Iterator<Item = (u64, i64)>) -> SeenIds {
        let mut v: Vec<(u64, i64)> = msgs.collect();
        v.sort_by_key(|x| std::cmp::Reverse(x.1));
        let floor = if v.len() > Self::MAX {
            v[Self::MAX - 1].1
        } else {
            i64::MIN
        };
        // (every message at the floor's time is kept, so none of them looks new)
        let mut ids: Vec<u64> = v
            .into_iter()
            .filter(|x| x.1 >= floor)
            .map(|x| x.0)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        SeenIds { ids, floor }
    }

    pub fn has(&self, id: u64) -> bool {
        self.ids.binary_search(&id).is_ok()
    }

    /// A message not seen, inside the time this covers.
    pub fn is_new(&self, id: u64, ts: i64) -> bool {
        ts >= self.floor && !self.has(id)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Location {
    Home,
    Inbox,
    Replies,
    Assigned,
    TodayOverdue,
    MyTasks,
    Everything,
    Space(u64),
    Folder(u64),
    List(u64),
}

/// See `Data::take_unreadable`.
static UNREADABLE: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Reminder {
    pub id: u64,
    pub text: String,
    pub due_ts: i64,
    pub done: bool,
    pub owner: u64,
    /// The task it is about ("Remind me" on a task).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<u64>,
    /// The message it is about: (channel, message).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<(u64, u64)>,
    /// The due notice was shown.
    #[serde(default)]
    pub alerted: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TargetKind {
    Number {
        start: f64,
        target: f64,
        current: f64,
        unit: String,
    },
    TrueFalse {
        done: bool,
    },
    Tasks {
        ids: Vec<u64>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GoalTarget {
    pub name: String,
    pub kind: TargetKind,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Goal {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    pub due: Option<NaiveDate>,
    pub owner: u64,
    pub targets: Vec<GoalTarget>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FormField {
    pub label: String,
    /// name, description, priority, due, assignee or text; dropdown, number,
    /// email and checkbox answers are added to the description too
    pub map: String,
    /// The form can't be submitted without an answer.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    /// Choices of a dropdown question.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Form {
    pub id: u64,
    pub name: String,
    pub list_id: u64,
    pub description: String,
    pub fields: Vec<FormField>,
    /// Who made it; 0 for forms from before this was recorded (left out of
    /// the file then, like the other fields added later).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub owner: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WbStroke {
    /// So strokes merge one by one (`wb_sync::fill_board_ids`); 0 until
    /// `Data::fix_activity_ids` gives it one.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub id: u64,
    pub color: [u8; 3],
    pub width: f32,
    pub points: Vec<[f32; 2]>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sticky {
    /// As `WbStroke::id`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub id: u64,
    pub pos: [f32; 2],
    pub text: String,
    pub color: [u8; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Whiteboard {
    pub id: u64,
    pub name: String,
    pub strokes: Vec<WbStroke>,
    pub notes: Vec<Sticky>,
    /// Who made it; 0 for whiteboards from before this was recorded.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub owner: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Meeting {
    pub id: u64,
    pub title: String,
    pub date: NaiveDate,
    pub time: String,
    pub attendees: Vec<u64>,
    pub agenda: String,
    pub notes: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrashItem {
    pub id: u64,
    pub kind: String,
    pub title: String,
    pub deleted: i64,
    pub payload: String,
}

/// What a deleted list keeps in Trash: the whole list with its settings, where
/// it sat (space and folder) and its tasks. Older entries were saved as a
/// `(space id, list, tasks)` tuple; `parse` reads both.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ListTrash {
    pub space: u64,
    #[serde(default)]
    pub folder: Option<u64>,
    pub list: ListItem,
    #[serde(default)]
    pub tasks: Vec<Task>,
    /// The statuses of the space it was in, so its tasks keep theirs (or the
    /// nearest one) when it comes back into another space. Empty in older entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<Status>,
}

impl ListTrash {
    pub fn parse(payload: &str) -> Option<ListTrash> {
        if let Ok(t) = serde_json::from_str::<ListTrash>(payload) {
            return Some(t);
        }
        let (space, list, tasks) =
            serde_json::from_str::<(u64, ListItem, Vec<Task>)>(payload).ok()?;
        Some(ListTrash {
            space,
            folder: None,
            list,
            tasks,
            statuses: vec![],
        })
    }
}

/// What a deleted space keeps in Trash (kind "Space"): the space itself with
/// its name, colour, icon, statuses and folders, its lists taken out. Each
/// list went to Trash on its own; `lists` are those entries' ids, restored
/// with the space while they are still there.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SpaceTrash {
    pub space: Space,
    #[serde(default)]
    pub lists: Vec<u64>,
}

/// What a deleted folder keeps in Trash (kind "Folder"), like `SpaceTrash`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FolderTrash {
    pub space: u64,
    pub folder: Folder,
    #[serde(default)]
    pub lists: Vec<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CustomField {
    pub id: u64,
    pub name: String,
    /// text, number, dropdown, checkbox or date
    pub kind: String,
    pub options: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Automation {
    pub id: u64,
    pub enabled: bool,
    /// When a task's status changes to this status...
    pub trigger_status: String,
    /// ...then: assign, priority, comment or move
    pub action: String,
    pub value: String,
    /// The List or Space (by id; ids are unique across both) it runs in;
    /// None runs it everywhere.
    pub list: Option<u64>,
}

/// A team template (Template Center). The fields from before `kind` was
/// kept describe a task; older apps read every template as one, so a List or
/// Doc template there makes a task named `task_name` with `description`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct TaskTemplate {
    pub id: u64,
    pub name: String,
    /// The name of what it makes (task name, List name or Doc title).
    pub task_name: String,
    /// The task's description, or a Doc template's text.
    pub description: String,
    pub checklist: Vec<String>,
    pub tags: Vec<String>,
    pub priority: u8,
    /// What it makes: "" a task, "list" a List with statuses and tasks, "doc" a Doc.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// List templates: the statuses its Space uses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub statuses: Vec<Status>,
    /// List templates: the tasks it starts with. No ids on purpose (see
    /// `Attachment`): the template is one item to wb-sync.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TemplateTask>,
}

/// A task a List template starts with.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct TemplateTask {
    pub name: String,
    /// The status by name; one the Space lacks becomes its first status.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checklist: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub priority: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bookmark {
    pub id: u64,
    pub title: String,
    pub url: String,
    /// Only on this computer: bookmarks someone had before bookmarks were
    /// shared with the team stay theirs (`wb_sync::shareable` leaves them
    /// out). New ones are shared.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub personal: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Dashboard {
    pub id: u64,
    pub name: String,
    pub space: Option<u64>,
    /// Who made it; 0 for dashboards from before this was recorded.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub owner: u64,
    /// Which cards it shows: "simple", "time", "project", or "" for all of them.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub private: bool,
    /// The cards in order, once customised; None shows the set `kind` picks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cards: Option<Vec<DashCard>>,
}

/// One card on a dashboard. No `id` on purpose (see `Attachment`): the
/// whole list is one field, so reordering merges as a single change.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DashCard {
    /// stats, status, priority, workload, time or overdue
    pub kind: String,
    /// Shown instead of the kind's own title when set.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ClickApps {
    pub priorities: bool,
    pub tags: bool,
    pub time_tracking: bool,
    pub time_estimates: bool,
    pub custom_fields: bool,
    pub multiple_assignees: bool,
}

impl Default for ClickApps {
    fn default() -> Self {
        Self {
            priorities: true,
            tags: true,
            time_tracking: true,
            time_estimates: true,
            custom_fields: true,
            multiple_assignees: true,
        }
    }
}

/// Each person's own notification choices (not shared with the team: only
/// `wb_sync::SHARED` paths are). The first five decide what shows in the
/// Inbox; mentions, assignments, comments and reminders also get a desktop
/// notification when that is on. The rest are desktop notifications only.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct NotifySettings {
    pub comments: bool,
    pub mentions: bool,
    pub reminders: bool,
    pub chat: bool,
    pub assigned: bool,
    /// Banners in Notification Center (when the app is not in front).
    pub desktop: bool,
    pub sound: bool,
    /// The number on the Dock icon: Inbox plus unread direct messages.
    pub dock_badge: bool,
    pub direct: bool,
    /// Replies in threads I started or wrote in.
    pub replies: bool,
    pub huddles: bool,
    /// Reactions to my messages.
    pub reactions: bool,
    /// Someone else changes the status, due date or priority of my tasks,
    /// or takes me off one.
    pub task_changes: bool,
    /// Once a day at `due_time`: my tasks due today and overdue.
    pub due: bool,
    /// "HH:MM"
    pub due_time: String,
    pub meetings: bool,
    pub meeting_minutes: u32,
    /// My goals due tomorrow and today.
    pub goals: bool,
    /// Someone joins the workspace (owners and admins).
    pub team: bool,
    /// A form I made gets an answer.
    pub forms: bool,
    /// The team server connection drops for a while, and comes back.
    pub connection: bool,
    /// Channel messages: "all", "mentions" or "nothing".
    pub channel_default: String,
    /// Channels set apart from `channel_default`.
    pub channel_modes: Vec<(u64, String)>,
    /// Notices on the clock (due tasks, meetings, goals) already given, so a
    /// restart doesn't repeat them. Newest last, kept short.
    pub fired: Vec<String>,
}

impl Default for NotifySettings {
    fn default() -> Self {
        Self {
            comments: true,
            mentions: true,
            reminders: true,
            chat: true,
            assigned: true,
            desktop: true,
            sound: true,
            dock_badge: true,
            direct: true,
            replies: true,
            huddles: true,
            reactions: false,
            task_changes: true,
            due: true,
            due_time: "09:00".into(),
            meetings: true,
            meeting_minutes: 10,
            goals: true,
            team: true,
            forms: true,
            connection: true,
            channel_default: "mentions".into(),
            channel_modes: vec![],
            fired: vec![],
        }
    }
}

impl NotifySettings {
    /// Every kind on and every channel loud: for telling what exists at all.
    pub fn everything() -> Self {
        Self {
            comments: true,
            mentions: true,
            reminders: true,
            chat: true,
            assigned: true,
            direct: true,
            replies: true,
            huddles: true,
            reactions: true,
            task_changes: true,
            team: true,
            forms: true,
            channel_default: "all".into(),
            ..Self::default()
        }
    }

    /// "all", "mentions" or "nothing" for a channel.
    pub fn channel_mode(&self, id: u64) -> &str {
        self.channel_modes
            .iter()
            .find(|(c, _)| *c == id)
            .map(|(_, m)| m.as_str())
            .unwrap_or(&self.channel_default)
    }

    pub fn set_channel_mode(&mut self, id: u64, mode: &str) {
        self.channel_modes.retain(|(c, _)| *c != id);
        if mode != self.channel_default {
            self.channel_modes.push((id, mode.to_string()));
        }
    }
}

/// Everything added after the first version, kept together so older saved
/// files still load.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Extra {
    pub reminders: Vec<Reminder>,
    pub goals: Vec<Goal>,
    pub forms: Vec<Form>,
    pub whiteboards: Vec<Whiteboard>,
    pub meetings: Vec<Meeting>,
    pub trash: Vec<TrashItem>,
    pub fields: Vec<CustomField>,
    pub automations: Vec<Automation>,
    pub templates: Vec<TaskTemplate>,
    pub bookmarks: Vec<Bookmark>,
    /// Bookmarks here are shared with the team; false in files from before
    /// they were, whose bookmarks then stay personal
    /// (`Data::keep_old_bookmarks_personal`).
    #[serde(default)]
    pub bookmarks_shared: bool,
    pub dashboards: Vec<Dashboard>,
    pub notepad: String,
    pub apps: ClickApps,
    pub notify: NotifySettings,
    /// Personal tools pinned to the top bar.
    pub pinned_tools: Vec<String>,
    /// Apps pinned to the left rail.
    pub rail_items: Vec<String>,
    /// Sidebar items the person hid.
    pub hidden_nav: Vec<String>,
    pub hidden_views: Vec<String>,
    /// Theme colour: black, purple, blue, pink, violet, indigo, orange, teal, bronze, mint.
    pub accent: String,
    /// Follow the computer's light / dark setting.
    pub auto_theme: bool,
    pub muted_until: i64,
    pub labs: Vec<String>,
    pub workspace_color: [u8; 3],
    pub teams: Vec<Team>,
    pub inbox_cleared: Vec<String>,
    pub inbox_later: Vec<String>,
    /// Starred items: "doc:ID", "wb:ID", "form:ID", "dash:ID".
    pub starred: Vec<String>,
    /// Recently opened items, newest first: "task:ID", "list:ID", "doc:ID".
    pub recent: Vec<String>,
    /// Replies older than this were read.
    pub replies_seen: i64,
    /// The shared folder this workspace is kept in step through, if any.
    pub shared: Option<crate::sync::SharedCfg>,
    /// What changed when, for merging with teammates' copies.
    pub sync: crate::sync::SyncMeta,
    /// The example content earlier versions started with has been taken out
    /// (files from before this field existed read as false).
    #[serde(default)]
    pub demo_removed: bool,
    /// Space Overview cards turned off with Customize.
    #[serde(default)]
    pub hidden_cards: Vec<String>,
    /// Filters I saved under a name (my own: left out of the copy
    /// teammates get).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub saved_filters: Vec<crate::filters::SavedFilter>,
    /// Replies marked read one by one: "taskid:ts:author" or "chat:CHANNEL:MESSAGE".
    #[serde(default)]
    pub replies_read: Vec<String>,
    /// A first start: show the welcome (files from before read as false).
    #[serde(default)]
    pub welcome: bool,
    /// Direct messages I closed, with when: each stays out of my sidebar
    /// until a message newer than that arrives (only on this computer).
    #[serde(default)]
    pub closed_dms: Vec<(u64, i64)>,
    /// The messages each closed direct message had when I closed it: a new
    /// one brings it back even when its author's clock is behind.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub closed_dm_seen: std::collections::BTreeMap<u64, SeenIds>,
    /// The messages I've seen in each channel and direct message (by
    /// channel id), so a late message still counts as unread.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub chat_seen: std::collections::BTreeMap<u64, SeenIds>,
    /// Tasks people added to their own timesheet with "+ Add task":
    /// [member, task] pairs.
    #[serde(default)]
    pub sheet_rows: Vec<[u64; 2]>,
    /// Rows added to one week's timesheet: [member, task, week] (the week's
    /// first day in days from the common era).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sheet_week_rows: Vec<[u64; 3]>,
    /// The Slack user ids I chose to bring in under my name ("Import under
    /// my name"): my next import starts from that for them. My own choice,
    /// left out of the copy teammates get, so it never changes their
    /// matching (`Member::slack`) and their imports never change it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slack_under_me: Vec<String>,
}

impl Default for Extra {
    fn default() -> Self {
        Self {
            reminders: vec![],
            goals: vec![],
            forms: vec![],
            whiteboards: vec![],
            meetings: vec![],
            trash: vec![],
            fields: vec![],
            automations: vec![],
            templates: vec![],
            bookmarks: vec![],
            bookmarks_shared: true,
            dashboards: vec![],
            notepad: String::new(),
            apps: ClickApps::default(),
            notify: NotifySettings::default(),
            pinned_tools: vec!["create_task".into(), "clip".into(), "stt".into()],
            rail_items: vec!["Planner".into(), "AI".into(), "Teams".into()],
            hidden_nav: vec![],
            hidden_views: vec![],
            accent: "black".into(),
            auto_theme: false,
            muted_until: 0,
            labs: vec![],
            workspace_color: [48, 164, 108],
            teams: vec![],
            inbox_cleared: vec![],
            inbox_later: vec![],
            starred: vec![],
            recent: vec![],
            replies_seen: 0,
            shared: None,
            sync: Default::default(),
            demo_removed: false,
            hidden_cards: vec![],
            saved_filters: vec![],
            replies_read: vec![],
            welcome: false,
            closed_dms: vec![],
            closed_dm_seen: Default::default(),
            chat_seen: Default::default(),
            sheet_rows: vec![],
            sheet_week_rows: vec![],
            slack_under_me: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Team {
    pub id: u64,
    pub name: String,
    pub members: Vec<u64>,
    pub color: [u8; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WsEntry {
    pub name: String,
    pub file: String,
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

    /// Gives every activity entry without an id one: entries from files saved
    /// before activity had ids, and lines just logged with `..Default::default()`.
    /// The id comes from what the entry says (`wb_sync::activity_id`), so every
    /// computer works out the same one. Repeats of an id are dropped.
    pub fn fix_activity_ids(&mut self) {
        for t in &mut self.tasks {
            fix_task_activity_ids(t);
        }
        self.fix_board_ids();
    }

    /// Gives whiteboard strokes and notes without an id one, the same one
    /// every computer works out (`wb_sync::fill_board_ids`).
    pub fn fix_board_ids(&mut self) {
        let boards = &mut self.extra.whiteboards;
        if boards
            .iter()
            .all(|w| w.strokes.iter().all(|s| s.id != 0) && w.notes.iter().all(|n| n.id != 0))
        {
            return;
        }
        let mut v = serde_json::json!({"extra": {"whiteboards": serde_json::to_value(&*boards).unwrap_or_default()}});
        if wb_sync::fill_board_ids(&mut v) {
            if let Ok(b) = serde_json::from_value(v["extra"]["whiteboards"].take()) {
                *boards = b;
            }
        }
    }

    pub fn dir() -> PathBuf {
        let mut p = Self::path();
        p.pop();
        p
    }

    pub fn workspaces() -> Vec<WsEntry> {
        let p = Self::dir().join("workspaces.json");
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// The workspace file open when the app last ran.
    pub fn last_workspace() -> Option<String> {
        let f = std::fs::read_to_string(Self::dir().join("last-workspace"))
            .ok()?
            .trim()
            .to_string();
        // only a plain file name in the data folder
        (!f.is_empty() && !f.contains('/') && !f.contains("..")).then_some(f)
    }

    /// The interface language picked on this computer.
    pub fn language() -> crate::i18n::Choice {
        let s = std::fs::read_to_string(Self::dir().join("language")).unwrap_or_default();
        crate::i18n::choice_from_key(&s)
    }

    pub fn set_language(c: crate::i18n::Choice) {
        let _ = std::fs::create_dir_all(Self::dir());
        let _ = std::fs::write(Self::dir().join("language"), crate::i18n::choice_key(c));
    }

    pub fn set_last_workspace(file: &str) {
        let _ = std::fs::create_dir_all(Self::dir());
        let _ = std::fs::write(Self::dir().join("last-workspace"), file);
    }

    pub fn save_workspaces(list: &[WsEntry]) {
        let _ = std::fs::create_dir_all(Self::dir());
        if let Ok(s) = serde_json::to_string_pretty(list) {
            let _ = std::fs::write(Self::dir().join("workspaces.json"), s);
        }
    }

    pub fn load_file(file: &str) -> Option<Self> {
        let s = std::fs::read_to_string(Self::dir().join(file)).ok()?;
        let mut d: Option<Data> = serde_json::from_str(&s).ok();
        match d.as_mut() {
            Some(d) => {
                d.keep_old_bookmarks_personal();
                d.keep_slack_under_me_personal();
            }
            None => Self::keep_unreadable(file, &s),
        }
        d
    }

    /// A file from before bookmarks were shared: the bookmarks in it are
    /// this person's own, so they stay on this computer instead of going to
    /// the whole team. True when that happened.
    pub fn keep_old_bookmarks_personal(&mut self) -> bool {
        if self.extra.bookmarks_shared {
            return false;
        }
        for b in &mut self.extra.bookmarks {
            b.personal = true;
        }
        self.extra.bookmarks_shared = true;
        true
    }

    /// A file from the version that kept "Import under my name" on my
    /// shared member (`Member::slack_mine`): the choices move to my personal
    /// `Extra::slack_under_me` and leave the member. Only my own member's;
    /// a teammate's are theirs to move on their computer. True when that
    /// happened.
    pub fn keep_slack_under_me_personal(&mut self) -> bool {
        let me = self.me;
        let Some(m) = self.members.iter_mut().find(|m| m.id == me) else {
            return false;
        };
        if m.slack_mine.is_empty() {
            return false;
        }
        for sid in std::mem::take(&mut m.slack_mine) {
            if !self.extra.slack_under_me.contains(&sid) {
                self.extra.slack_under_me.push(sid);
            }
        }
        true
    }

    /// The Slack people I bring in under my name: my personal choices, and
    /// any still on my member from an earlier version.
    pub fn slack_under_me(&self) -> Vec<String> {
        let mut out = self.extra.slack_under_me.clone();
        for sid in self
            .member(self.me)
            .map(|m| &m.slack_mine[..])
            .unwrap_or(&[])
        {
            if !out.contains(sid) {
                out.push(sid.clone());
            }
        }
        out
    }

    /// A saved workspace that couldn't be read is copied aside before
    /// anything can save over it; the app tells the person where it went.
    fn keep_unreadable(file: &str, text: &str) {
        let stem = file.strip_suffix(".json").unwrap_or(file);
        let p = Self::dir().join(format!("{}-unreadable-{}.json", stem, now_ts()));
        eprintln!(
            "workboard: couldn't read {}; a copy is kept at {}",
            file,
            p.display()
        );
        if std::fs::write(&p, text).is_ok() {
            *UNREADABLE.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
        }
    }

    /// The copy of a workspace file that couldn't be read, once.
    pub fn take_unreadable() -> Option<PathBuf> {
        UNREADABLE.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn save_file(&self, file: &str) {
        let p = Self::dir().join(file);
        let _ = std::fs::create_dir_all(Self::dir());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    /// A fresh workspace with one space and list, owned by `owner`.
    pub fn blank(name: &str, owner: Member) -> Self {
        let mut d = Data {
            workspace: name.to_string(),
            spaces: vec![],
            tasks: vec![],
            members: vec![owner.clone()],
            me: owner.id,
            next_id: 1000,
            favorites: vec![],
            dark: false,
            docs: vec![],
            channels: vec![],
            extra: Extra::default(),
        };
        let sid = d.new_id();
        let lid = d.new_id();
        // named in the language the screen is in; a new workspace has one
        // person, so the space isn't called a team's
        d.spaces.push(Space {
            id: sid,
            name: crate::i18n::tr("Main space").into(),
            color: [0x5f, 0x55, 0xee],
            statuses: default_statuses(),
            folders: vec![],
            lists: vec![ListItem {
                id: lid,
                name: crate::i18n::tr("List").into(),
                ..Default::default()
            }],
            expanded: true,
            private_owner: None,
            ..Default::default()
        });
        let cid = d.new_id();
        d.channels.push(Channel {
            id: cid,
            name: "general".into(),
            kind: ChannelKind::Channel,
            topic: String::new(),
            members: vec![owner.id],
            messages: vec![],
            last_read: now_ts(),
            ..Default::default()
        });
        d.extra.demo_removed = true;
        // a new workspace leaves out what needs an online service Workboard
        // doesn't have (Customize brings them back); files saved before keep
        // their own choices, and files without these fields keep the old
        // defaults (`Extra::default`)
        d.extra.pinned_tools = vec!["create_task".into()];
        d.extra.hidden_nav = vec!["Skills".into(), "Artifacts".into()];
        d
    }

    pub fn path() -> PathBuf {
        // a separate data folder (for trying things without touching real work)
        if let Ok(dir) = std::env::var("WORKBOARD_HOME") {
            if !dir.is_empty() {
                return PathBuf::from(dir).join("data.json");
            }
        }
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".into());
        let mut p = PathBuf::from(home);
        if cfg!(target_os = "macos") {
            p.push("Library/Application Support/Workboard");
        } else {
            p.push(".workboard");
        }
        p.push("data.json");
        p
    }

    pub fn load() -> Self {
        if let Ok(s) = std::fs::read_to_string(Self::path()) {
            if let Ok(mut d) = serde_json::from_str::<Data>(&s) {
                d.keep_old_bookmarks_personal();
                d.keep_slack_under_me_personal();
                // move the earlier default colours to the measured ones
                for sp in &mut d.spaces {
                    for st in &mut sp.statuses {
                        if st.name == "in progress" && st.color == [0x5f, 0x55, 0xee] {
                            st.color = IN_PROGRESS_COLOR;
                        }
                        if st.name == "complete" && st.color == [0x00, 0x88, 0x44] {
                            st.color = COMPLETE_COLOR;
                        }
                    }
                }
                if !d.extra.demo_removed {
                    let _ = std::fs::write(
                        Self::dir().join(format!("data-before-cleanup-{}.json", now_ts())),
                        &s,
                    );
                    d.remove_demo();
                    d.save();
                }
                // every workspace has a #general channel to talk in
                if !d.channels.iter().any(|c| c.kind == ChannelKind::Channel) {
                    let id = d.new_id();
                    let members = d.members.iter().map(|m| m.id).collect();
                    d.channels.insert(
                        0,
                        Channel {
                            id,
                            name: "general".into(),
                            kind: ChannelKind::Channel,
                            topic: String::new(),
                            members,
                            messages: vec![],
                            last_read: now_ts(),
                            ..Default::default()
                        },
                    );
                }
                return d;
            }
            if !s.trim().is_empty() {
                Self::keep_unreadable("data.json", &s);
            }
        }
        Self::fresh()
    }

    /// The empty workspace a first start opens with.
    /// The login name with its first letter in capitals ("Me" when empty);
    /// works on any first character, not only ASCII.
    pub fn display_user(user: &str) -> String {
        let mut cs = user.chars();
        match cs.next() {
            None => "Me".to_string(),
            Some(c) => c.to_uppercase().collect::<String>() + cs.as_str(),
        }
    }

    pub fn fresh() -> Self {
        let user = std::env::var("USER").unwrap_or_default();
        let name = Self::display_user(&user);
        let owner = Member {
            id: 1,
            name,
            color: [0x5f, 0x55, 0xee],
            email: String::new(),
            role: "Owner".into(),
            ..Default::default()
        };
        let mut d = Self::blank(crate::i18n::tr("My Workspace"), owner);
        d.extra.welcome = true;
        d
    }

    /// Take out the example content (people, spaces, tasks, docs, chat...)
    /// that earlier versions started with, keeping everything the person made.
    pub fn remove_demo(&mut self) {
        use serde_json::Value;
        use std::collections::HashSet;
        fn label(v: &Value) -> Option<String> {
            ["name", "title", "text"]
                .iter()
                .find_map(|k| v.get(*k).and_then(|x| x.as_str()).map(|x| x.to_string()))
        }
        fn collect(v: &Value, key: &str, out: &mut HashSet<(String, String)>) {
            match v {
                Value::Array(a) => {
                    for x in a {
                        if x.get("id").is_some() {
                            if let Some(l) = label(x) {
                                out.insert((key.to_string(), l));
                            }
                        }
                        collect(x, key, out);
                    }
                }
                Value::Object(o) => {
                    for (k, x) in o {
                        collect(x, k, out);
                    }
                }
                _ => {}
            }
        }
        // true when the item (or something inside it) must stay
        fn filter(
            v: &mut Value,
            key: &str,
            demo: &HashSet<(String, String)>,
            keep: &HashSet<u64>,
        ) -> bool {
            let mut stays = false;
            match v {
                Value::Array(a) => {
                    a.retain_mut(|x| {
                        let inner = filter(x, key, demo, keep);
                        let Some(id) = x.get("id").and_then(|i| i.as_u64()) else {
                            return true;
                        };
                        let is_demo = id < 1000
                            && label(x).map_or(false, |l| demo.contains(&(key.to_string(), l)));
                        let keep_it = !is_demo || inner || keep.contains(&id);
                        stays |= keep_it;
                        keep_it
                    });
                }
                Value::Object(o) => {
                    for (k, x) in o.iter_mut() {
                        if x.is_array() {
                            let had = x
                                .as_array()
                                .map_or(false, |a| a.iter().any(|i| i.get("id").is_some()));
                            let any = filter(x, k, demo, keep);
                            stays |= had && any;
                        }
                    }
                }
                _ => {}
            }
            stays
        }
        let mut seed = serde_json::to_value(Data::seed()).unwrap_or_default();
        if let Some(e) = seed.get_mut("extra").and_then(|e| e.as_object_mut()) {
            e.remove("sync");
        }
        let mut demo = HashSet::new();
        collect(&seed, "", &mut demo);
        let mut v = serde_json::to_value(&*self).unwrap_or_default();
        // lists still holding the person's own tasks, and the person themself, stay
        let mut keep: HashSet<u64> = HashSet::new();
        keep.insert(self.me);
        let demo_task =
            |t: &Task| t.id < 1000 && demo.contains(&("tasks".to_string(), t.name.clone()));
        for t in &self.tasks {
            if !demo_task(t) {
                keep.insert(t.list_id);
                for s in &self.spaces {
                    if s.lists.iter().any(|l| l.id == t.list_id) {
                        keep.insert(s.id);
                    }
                    for f in &s.folders {
                        if f.lists.iter().any(|l| l.id == t.list_id) {
                            keep.insert(f.id);
                            keep.insert(s.id);
                        }
                    }
                }
            }
        }
        for d in &self.docs {
            if !(d.id < 1000 && demo.contains(&("docs".to_string(), d.title.clone()))) {
                if let Some(sp) = d.space {
                    keep.insert(sp);
                }
            }
        }
        if let Some(o) = v.as_object_mut() {
            for (k, x) in o.iter_mut() {
                if k == "extra" {
                    if let Some(e) = x.as_object_mut() {
                        for (ek, ex) in e.iter_mut() {
                            if ek != "sync" && ex.is_array() {
                                filter(ex, ek, &demo, &keep);
                            }
                        }
                    }
                } else if x.is_array() {
                    filter(x, k, &demo, &keep);
                }
            }
        }
        if let Ok(mut d) = serde_json::from_value::<Data>(v) {
            let people: HashSet<u64> = d.members.iter().map(|m| m.id).collect();
            for t in &mut d.tasks {
                t.assignees.retain(|a| people.contains(a));
            }
            for c in &mut d.channels {
                c.members.retain(|m| people.contains(m));
            }
            for m in &mut d.members {
                if m.email.ends_with("@example.com") {
                    m.email.clear();
                }
            }
            let lists: HashSet<u64> = d
                .spaces
                .iter()
                .flat_map(|s| {
                    s.lists
                        .iter()
                        .map(|l| l.id)
                        .chain(s.folders.iter().flat_map(|f| f.lists.iter().map(|l| l.id)))
                })
                .collect();
            let spaces: HashSet<u64> = d.spaces.iter().map(|s| s.id).collect();
            let folders: HashSet<u64> = d
                .spaces
                .iter()
                .flat_map(|s| s.folders.iter().map(|f| f.id))
                .collect();
            d.favorites.retain(|f| match f {
                Location::List(id) => lists.contains(id),
                Location::Space(id) => spaces.contains(id),
                Location::Folder(id) => folders.contains(id),
                _ => true,
            });
            if d.spaces.is_empty() {
                let sid = d.new_id();
                let lid = d.new_id();
                d.spaces.push(Space {
                    id: sid,
                    name: "Team Space".into(),
                    color: [0x5f, 0x55, 0xee],
                    statuses: default_statuses(),
                    folders: vec![],
                    lists: vec![ListItem {
                        id: lid,
                        name: "List".into(),
                        ..Default::default()
                    }],
                    expanded: true,
                    private_owner: None,
                    ..Default::default()
                });
            }
            d.extra.demo_removed = true;
            *self = d;
        }
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    pub fn member(&self, id: u64) -> Option<&Member> {
        self.members.iter().find(|m| m.id == id)
    }

    /// Who a text @mentions (see `mentioned_ids`).
    pub fn mentions_in(&self, text: &str) -> Vec<u64> {
        mentioned_ids(text, &self.members, &[])
    }

    pub fn space_of_list(&self, list_id: u64) -> Option<&Space> {
        self.spaces.iter().find(|s| {
            s.lists.iter().any(|l| l.id == list_id)
                || s.folders
                    .iter()
                    .any(|f| f.lists.iter().any(|l| l.id == list_id))
        })
    }

    /// True when the list, or the folder or space it sits in, is archived.
    pub fn list_archived(&self, id: u64) -> bool {
        self.list_item(id).is_some_and(|l| l.archived)
            || self.space_of_list(id).is_some_and(|s| {
                s.archived
                    || s.folders
                        .iter()
                        .any(|f| f.archived && f.lists.iter().any(|l| l.id == id))
            })
    }

    /// True when the Space, Folder or List, or what it sits in, is archived.
    pub fn location_archived(&self, loc: Location) -> bool {
        match loc {
            Location::Space(id) => self.spaces.iter().any(|s| s.id == id && s.archived),
            Location::Folder(id) => self.spaces.iter().any(|s| {
                s.folders
                    .iter()
                    .any(|f| f.id == id && (f.archived || s.archived))
            }),
            Location::List(id) => self.list_archived(id),
            _ => false,
        }
    }

    /// Not archived, and not in an archived list or space.
    pub fn task_live(&self, t: &Task) -> bool {
        !t.archived && !self.list_archived(t.list_id)
    }

    /// Only one person in the workspace: everything unowned is theirs.
    pub fn solo(&self) -> bool {
        self.members.len() <= 1
    }

    /// The task's timer runs and it is mine. A timer from before timers
    /// recorded who started it counts as mine only when I'm alone in the
    /// workspace, so it is never stopped behind a teammate's back.
    pub fn is_my_timer(&self, t: &Task) -> bool {
        self.my_timer_start(t).is_some()
    }

    /// When my timer on the task started, if one runs.
    pub fn my_timer_start(&self, t: &Task) -> Option<i64> {
        let legacy = t.timer_started.filter(|_| match t.timer_by {
            Some(m) => m == self.me,
            None => self.solo(),
        });
        legacy.or_else(|| {
            t.timers
                .iter()
                .find(|r| r.member == self.me)
                .map(|r| r.start)
        })
    }

    /// My time on a task: my time entries plus my running timer (not my
    /// teammates' time, which `Task::tracked_secs` adds in).
    pub fn my_tracked_secs(&self, t: &Task, now: i64) -> i64 {
        let solo = self.solo();
        let done: i64 = t
            .time_entries
            .iter()
            .filter(|e| {
                let owner = t.entry_owner(e);
                owner == self.me || (owner == 0 && solo)
            })
            .map(|e| e.end - e.start)
            .sum();
        done + self.my_timer_start(t).map(|st| now - st).unwrap_or(0)
    }

    /// A timer from an older app that didn't record who started it, in a
    /// workspace with more people: nobody's to stop, so it is offered to
    /// stop by hand.
    pub fn ownerless_timer(&self, t: &Task) -> bool {
        t.timer_started.is_some() && t.timer_by.is_none() && !self.solo()
    }

    /// Starts my timer on a task (other people's timers keep running).
    pub fn start_my_timer(&mut self, task_id: u64, now: i64) {
        let me = self.me;
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == task_id) else {
            return;
        };
        if t.timer_started.is_none() {
            t.timer_started = Some(now);
            t.timer_by = Some(me);
        } else if !t.timers.iter().any(|r| r.member == me) {
            t.timers.push(RunningTimer {
                member: me,
                start: now,
            });
        }
    }

    /// Stops my timer on one task, keeping the time as my entry.
    pub fn stop_my_timer(&mut self, task_id: u64, now: i64) {
        let me = self.me;
        let Some(i) = self.tasks.iter().position(|t| t.id == task_id) else {
            return;
        };
        let legacy_mine = self.tasks[i].timer_started.is_some()
            && match self.tasks[i].timer_by {
                Some(m) => m == me,
                None => self.solo(),
            };
        let t = &mut self.tasks[i];
        if legacy_mine {
            if let Some(st) = t.timer_started.take() {
                t.time_entries.push(TimeEntry {
                    start: st,
                    end: now,
                    member: me,
                });
            }
            t.timer_by = None;
            // the next person's timer moves into the field older apps read
            if !t.timers.is_empty() {
                let r = t.timers.remove(0);
                t.timer_started = Some(r.start);
                t.timer_by = Some(r.member);
            }
        } else if let Some(k) = t.timers.iter().position(|r| r.member == me) {
            let r = t.timers.remove(k);
            t.time_entries.push(TimeEntry {
                start: r.start,
                end: now,
                member: me,
            });
        }
    }

    /// Stops a timer an older app started without an owner, keeping the
    /// time without an owner too.
    pub fn stop_ownerless_timer(&mut self, task_id: u64, now: i64) {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == task_id) else {
            return;
        };
        if t.timer_by.is_none() {
            if let Some(st) = t.timer_started.take() {
                t.time_entries.push(TimeEntry {
                    start: st,
                    end: now,
                    member: 0,
                });
            }
            if !t.timers.is_empty() {
                let r = t.timers.remove(0);
                t.timer_started = Some(r.start);
                t.timer_by = Some(r.member);
            }
        }
    }

    /// A time entry counts as mine (old entries: see `Task::entry_owner`;
    /// with nobody to give it to, it is mine when I'm alone).
    pub fn is_my_entry(&self, t: &Task, e: &TimeEntry) -> bool {
        match t.entry_owner(e) {
            0 => self.solo(),
            o => o == self.me,
        }
    }

    /// Stops every timer of mine, keeping the time as my entries.
    pub fn stop_my_timers(&mut self, now: i64) {
        let mine: Vec<u64> = self
            .tasks
            .iter()
            .filter(|t| self.is_my_timer(t))
            .map(|t| t.id)
            .collect();
        for id in mine {
            self.stop_my_timer(id, now);
        }
    }

    /// The letter or symbol drawn on a space's square.
    pub fn space_glyph(s: &Space) -> String {
        if s.icon.is_empty() {
            s.name
                .chars()
                .next()
                .unwrap_or('S')
                .to_uppercase()
                .to_string()
        } else {
            s.icon.clone()
        }
    }

    /// The list with this id, wherever it sits (space or folder).
    pub fn list_item(&self, id: u64) -> Option<&ListItem> {
        self.spaces
            .iter()
            .flat_map(|s| {
                s.lists
                    .iter()
                    .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
            })
            .find(|l| l.id == id)
    }

    pub fn list_item_mut(&mut self, id: u64) -> Option<&mut ListItem> {
        self.spaces
            .iter_mut()
            .flat_map(|s| {
                s.lists
                    .iter_mut()
                    .chain(s.folders.iter_mut().flat_map(|f| f.lists.iter_mut()))
            })
            .find(|l| l.id == id)
    }

    pub fn list_name(&self, list_id: u64) -> String {
        for s in &self.spaces {
            for l in &s.lists {
                if l.id == list_id {
                    return l.name.clone();
                }
            }
            for f in &s.folders {
                for l in &f.lists {
                    if l.id == list_id {
                        return l.name.clone();
                    }
                }
            }
        }
        "List".into()
    }

    /// Breadcrumb names for a list: space / folder / list.
    pub fn list_path(&self, list_id: u64) -> Vec<String> {
        for s in &self.spaces {
            for l in &s.lists {
                if l.id == list_id {
                    return vec![s.name.clone(), l.name.clone()];
                }
            }
            for f in &s.folders {
                for l in &f.lists {
                    if l.id == list_id {
                        return vec![s.name.clone(), f.name.clone(), l.name.clone()];
                    }
                }
            }
        }
        vec![]
    }

    pub fn statuses_for_list(&self, list_id: u64) -> Vec<Status> {
        self.space_of_list(list_id)
            .map(|s| s.statuses.clone())
            .unwrap_or_else(default_statuses)
    }

    pub fn status_of(&self, t: &Task) -> Status {
        let sts = self.statuses_for_list(t.list_id);
        sts.iter()
            .find(|s| s.name == t.status)
            .cloned()
            .unwrap_or_else(|| sts[0].clone())
    }

    pub fn is_closed(&self, t: &Task) -> bool {
        matches!(
            self.status_of(t).kind,
            StatusKind::Closed | StatusKind::Done
        )
    }

    /// Lists covered by a location (empty for cross-workspace locations).
    pub fn lists_in(&self, loc: Location) -> Option<Vec<u64>> {
        match loc {
            Location::List(id) => Some(vec![id]),
            Location::Folder(fid) => {
                for s in &self.spaces {
                    for f in &s.folders {
                        if f.id == fid {
                            return Some(f.lists.iter().map(|l| l.id).collect());
                        }
                    }
                }
                Some(vec![])
            }
            Location::Space(sid) => {
                let s = self.spaces.iter().find(|s| s.id == sid)?;
                let mut v: Vec<u64> = s.lists.iter().map(|l| l.id).collect();
                for f in &s.folders {
                    v.extend(f.lists.iter().map(|l| l.id));
                }
                Some(v)
            }
            _ => None,
        }
    }

    pub fn location_name(&self, loc: Location) -> String {
        match loc {
            Location::Home => "Home".into(),
            Location::Inbox => "Inbox".into(),
            Location::Replies => "Replies".into(),
            Location::Assigned => "Assigned Comments".into(),
            Location::TodayOverdue => "Today & Overdue".into(),
            Location::MyTasks => "My Tasks".into(),
            Location::Everything => "Everything".into(),
            Location::Space(id) => self
                .spaces
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            Location::Folder(id) => self
                .spaces
                .iter()
                .flat_map(|s| s.folders.iter())
                .find(|f| f.id == id)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            Location::List(id) => self.list_name(id),
        }
    }

    pub fn location_exists(&self, loc: Location) -> bool {
        match loc {
            Location::Space(id) => self.spaces.iter().any(|s| s.id == id),
            Location::Folder(id) => self
                .spaces
                .iter()
                .any(|s| s.folders.iter().any(|f| f.id == id)),
            Location::List(id) => self.space_of_list(id).is_some(),
            _ => true,
        }
    }

    /// The first list new tasks can go into: skips archived spaces and lists
    /// and other people's private spaces.
    pub fn first_list(&self) -> Option<u64> {
        for s in &self.spaces {
            if s.archived || s.private_owner.is_some_and(|o| o != self.me) {
                continue;
            }
            for f in &s.folders {
                if let Some(l) = f.lists.iter().find(|l| !l.archived) {
                    return Some(l.id);
                }
            }
            if let Some(l) = s.lists.iter().find(|l| !l.archived) {
                return Some(l.id);
            }
        }
        None
    }

    pub fn seed() -> Self {
        let mut d = Data {
            workspace: "My Workspace".into(),
            spaces: vec![],
            tasks: vec![],
            members: vec![
                Member {
                    id: 1,
                    name: "Me".into(),
                    color: [0x5f, 0x55, 0xee],
                    email: "me@example.com".into(),
                    role: "Owner".into(),
                    ..Default::default()
                },
                Member {
                    id: 2,
                    name: "Alex Kim".into(),
                    color: [0x10, 0x90, 0xe0],
                    email: "alex@example.com".into(),
                    role: "Admin".into(),
                    ..Default::default()
                },
                Member {
                    id: 3,
                    name: "Sam Lee".into(),
                    color: [0xe1, 0x6b, 0x16],
                    email: "sam@example.com".into(),
                    role: "Member".into(),
                    ..Default::default()
                },
                Member {
                    id: 4,
                    name: "Jordan Park".into(),
                    color: [0x00, 0x88, 0x44],
                    email: "jordan@example.com".into(),
                    role: "Member".into(),
                    ..Default::default()
                },
            ],
            me: 1,
            next_id: 100,
            favorites: vec![],
            dark: false,
            docs: vec![],
            channels: vec![],
            extra: Extra::default(),
        };
        let space = d.new_id();
        let folder = d.new_id();
        let l_web = d.new_id();
        let l_mkt = d.new_id();
        let l_personal = d.new_id();
        let space2 = d.new_id();
        let l_ideas = d.new_id();
        d.spaces.push(Space {
            id: space,
            name: "Team Space".into(),
            color: [0x5f, 0x55, 0xee],
            statuses: default_statuses(),
            folders: vec![Folder {
                id: folder,
                name: "Projects".into(),
                lists: vec![
                    ListItem {
                        id: l_web,
                        name: "Website Redesign".into(),
                        ..Default::default()
                    },
                    ListItem {
                        id: l_mkt,
                        name: "Marketing".into(),
                        ..Default::default()
                    },
                ],
                expanded: true,
                archived: false,
            }],
            lists: vec![ListItem {
                id: l_personal,
                name: "Personal".into(),
                ..Default::default()
            }],
            expanded: true,
            private_owner: None,
            ..Default::default()
        });
        d.spaces.push(Space {
            id: space2,
            name: "Side Projects".into(),
            color: [0xe1, 0x6b, 0x16],
            statuses: default_statuses(),
            folders: vec![],
            lists: vec![ListItem {
                id: l_ideas,
                name: "Ideas".into(),
                ..Default::default()
            }],
            expanded: false,
            private_owner: None,
            ..Default::default()
        });
        d.favorites.push(Location::List(l_web));

        let t = today();
        let wd = t.weekday().num_days_from_monday() as i64;
        let day = |n: i64| Some(t + Duration::days(n));
        let samples: Vec<(
            u64,
            &str,
            &str,
            u8,
            Vec<u64>,
            Option<NaiveDate>,
            Option<NaiveDate>,
            Vec<&str>,
        )> = vec![
            (
                l_web,
                "Design new homepage layout",
                "in progress",
                2,
                vec![1, 2],
                day(-wd),
                day(2),
                vec!["design"],
            ),
            (
                l_web,
                "Write copy for landing page",
                "to do",
                3,
                vec![3],
                day(1),
                day(4),
                vec!["content"],
            ),
            (
                l_web,
                "Set up analytics dashboard",
                "to do",
                4,
                vec![],
                None,
                day(9),
                vec![],
            ),
            (
                l_web,
                "Fix navigation on mobile",
                "in progress",
                1,
                vec![1],
                day(-3),
                day(-1),
                vec!["bug"],
            ),
            (
                l_web,
                "Collect feedback from beta users",
                "to do",
                0,
                vec![4],
                None,
                None,
                vec![],
            ),
            (
                l_web,
                "Choose color palette",
                "complete",
                3,
                vec![2],
                day(-8),
                day(-5),
                vec!["design"],
            ),
            (
                l_mkt,
                "Plan Q1 campaign",
                "in progress",
                2,
                vec![1, 3],
                day(-2),
                day(6),
                vec![],
            ),
            (
                l_mkt,
                "Newsletter draft",
                "to do",
                3,
                vec![3],
                None,
                day(3),
                vec!["content"],
            ),
            (
                l_mkt,
                "Social media calendar",
                "complete",
                4,
                vec![4],
                day(-10),
                day(-6),
                vec![],
            ),
            (
                l_personal,
                "Renew passport",
                "to do",
                2,
                vec![1],
                None,
                day(0),
                vec![],
            ),
            (
                l_personal,
                "Book dentist appointment",
                "to do",
                3,
                vec![1],
                None,
                day(12),
                vec![],
            ),
            (
                l_ideas,
                "Build a reading tracker",
                "to do",
                0,
                vec![1],
                None,
                None,
                vec![],
            ),
        ];
        let now = now_ts();
        for (i, (list, name, st, pr, asg, start, due, tags)) in samples.into_iter().enumerate() {
            let id = d.new_id();
            d.tasks.push(Task {
                id,
                list_id: list,
                parent: None,
                name: name.into(),
                description: String::new(),
                status: st.into(),
                priority: pr,
                assignees: asg,
                start,
                due,
                start_time: None,
                due_time: None,
                tags: tags.into_iter().map(String::from).collect(),
                checklist: vec![],
                activity: vec![Activity {
                    ts: now - 86400 * 3,
                    author: 1,
                    text: "created this task".into(),
                    comment: false,
                    ..Default::default()
                }],
                time_entries: vec![],
                timer_started: None,
                timer_by: None,
                timers: vec![],
                estimate_min: None,
                created: now - 86400 * 3 + i as i64,
                updated: now,
                order: i as i64,
                custom: Default::default(),
                recur: None,
                archived: false,
                done_at: None,
                closed_at: None,
                milestone: false,
                watchers: vec![],
                waiting_on: vec![],
                linked: vec![],
                attachments: vec![],
            });
        }
        // Subtasks and a checklist on the first task.
        let first = d.tasks[0].id;
        for (n, name) in ["Wireframe hero section", "Pick hero image"]
            .iter()
            .enumerate()
        {
            let id = d.new_id();
            d.tasks.push(Task {
                id,
                list_id: l_web,
                parent: Some(first),
                name: (*name).into(),
                description: String::new(),
                status: if n == 0 {
                    "complete".into()
                } else {
                    "to do".into()
                },
                priority: 0,
                assignees: vec![2],
                start: None,
                due: None,
                start_time: None,
                due_time: None,
                tags: vec![],
                checklist: vec![],
                activity: vec![],
                time_entries: vec![],
                timer_started: None,
                timer_by: None,
                timers: vec![],
                estimate_min: None,
                created: now,
                updated: now,
                order: 100 + n as i64,
                custom: Default::default(),
                recur: None,
                archived: false,
                done_at: None,
                closed_at: None,
                milestone: false,
                watchers: vec![],
                waiting_on: vec![],
                linked: vec![],
                attachments: vec![],
            });
        }
        d.tasks[0].description =
            "Refresh the homepage with a cleaner hero, clear call to action and a lighter grid."
                .into();
        d.tasks[0].checklist = vec![
            CheckItem {
                text: "Review competitors".into(),
                done: true,
                assignee: None,
            },
            CheckItem {
                text: "Sketch three options".into(),
                done: false,
                assignee: None,
            },
            CheckItem {
                text: "Share with the team".into(),
                done: false,
                assignee: None,
            },
        ];
        let now2 = now_ts();
        let doc_id = d.new_id();
        d.docs.push(Doc {
            id: doc_id,
            space: Some(space),
            owner: 1,
            private: false,
            archived: false,
            meeting_note: false,
            wiki: None,
            views: vec![],
            parent: None,
            title: "Project brief".into(),
            body: "Goals\n- Launch the new website\n- Grow the newsletter\n\nNotes\nKeep the design light and simple.".into(),
            created: now2 - 86400,
            updated: now2 - 3600,
            tags: vec![],
        });
        // chat
        let mut msg = |d: &mut Data, author: u64, text: &str, ago: i64| Message {
            id: d.new_id(),
            author,
            text: text.into(),
            mentions: vec![],
            ts: now2 - ago,
            reactions: vec![],
            replies: vec![],
            saved: false,
            task: None,
            post_title: None,
            post_type: String::new(),
            pinned: false,
            call: None,
            attachments: vec![],
            edited: None,
            imported: None,
        };
        let c1 = d.new_id();
        let mut m1 = msg(
            &mut d,
            2,
            "Morning everyone! The new homepage mockups are in the Website Redesign list.",
            3 * 3600,
        );
        m1.reactions.push(Reaction {
            emoji: "\u{1F44D}".into(),
            users: vec![3, 4],
        });
        let r1 = msg(
            &mut d,
            1,
            "Looks great, I'll review them after lunch.",
            2 * 3600,
        );
        m1.replies.push(r1);
        let m2 = msg(
            &mut d,
            3,
            "Reminder: newsletter draft is due Thursday.",
            3600,
        );
        let m3 = msg(
            &mut d,
            4,
            "@Me can you check the beta feedback list when you have a minute?",
            1200,
        );
        d.channels.push(Channel {
            id: c1,
            name: "general".into(),
            kind: ChannelKind::Channel,
            topic: "Company-wide announcements and chat".into(),
            members: vec![1, 2, 3, 4],
            messages: vec![m1, m2, m3],
            last_read: now2 - 1800,
            ..Default::default()
        });
        let c2 = d.new_id();
        let m4 = msg(
            &mut d,
            2,
            "Should the hero image be a photo or an illustration?",
            5400,
        );
        d.channels.push(Channel {
            id: c2,
            name: "website-redesign".into(),
            kind: ChannelKind::Channel,
            topic: "Everything about the new site".into(),
            members: vec![1, 2],
            messages: vec![m4],
            last_read: now2,
            ..Default::default()
        });
        let c3 = d.new_id();
        let m5 = msg(
            &mut d,
            2,
            "Hey, do you have a minute to talk about the launch date?",
            600,
        );
        d.channels.push(Channel {
            id: c3,
            name: String::new(),
            kind: ChannelKind::Direct,
            topic: String::new(),
            members: vec![1, 2],
            messages: vec![m5],
            last_read: 0,
            ..Default::default()
        });
        let c4 = d.new_id();
        d.channels.push(Channel {
            id: c4,
            name: String::new(),
            kind: ChannelKind::Direct,
            topic: String::new(),
            members: vec![1, 3],
            messages: vec![],
            last_read: now2,
            ..Default::default()
        });
        d.tasks[0].time_entries.push(TimeEntry {
            start: now2 - 86400 - 5400,
            end: now2 - 86400,
            member: 0,
        });
        d.tasks[3].time_entries.push(TimeEntry {
            start: now2 - 7200,
            end: now2 - 4800,
            member: 0,
        });
        d.tasks[0].activity.push(Activity {
            ts: now - 3600 * 5,
            author: 2,
            text: "Looks great so far, let's review on Friday.".into(),
            comment: true,
            ..Default::default()
        });
        d
    }
}
