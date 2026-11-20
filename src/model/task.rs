// A task with its checklist, activity, time entries and timer, and the time
// of day helpers its reminders and due times use.

use super::*;

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

pub(super) fn is_zero(n: &u64) -> bool {
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
    /// The form whose answer made the task (its Answers tab lists it). Set
    /// once, by the person who submitted, and never cleared, so the team
    /// merge needs no default rule for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<u64>,
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
