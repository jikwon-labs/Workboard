// What `Data::extra` holds: reminders, goals, forms, whiteboards, meetings,
// trash, custom fields, automations, templates, dashboards and settings.

use super::*;

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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Goal {
    pub id: u64,
    pub name: String,
    pub color: [u8; 3],
    pub due: Option<NaiveDate>,
    pub owner: u64,
    pub targets: Vec<GoalTarget>,
    /// When it started: progress is compared with how much of start..due
    /// has gone by (`Goal::pace`). None on goals from before it was kept;
    /// set once and changed, never cleared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<NaiveDate>,
    /// The day it reached 100%; None again if it drops back. Always
    /// written (null too) so an older copy's day isn't filled back in.
    #[serde(default)]
    pub done_on: Option<NaiveDate>,
    /// The highest milestone (25, 50, 75 or 100) reached so far. It only
    /// goes up, so dropping back and crossing again isn't announced twice.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub reached: u8,
    /// What happened to it, oldest first (the last `GOAL_LOG_KEEP`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub log: Vec<GoalEvent>,
    /// Its progress (percent) at the end of each day it changed, oldest
    /// first (the last `GOAL_DAYS_KEEP`): for "up N points since last week".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<(NaiveDate, u8)>,
}

pub const GOAL_LOG_KEEP: usize = 100;
pub const GOAL_DAYS_KEEP: usize = 60;

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

/// One line of a goal's history.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GoalEvent {
    pub ts: i64,
    /// Who did it; 0 when the app noticed it (a milestone reached through
    /// tasks closed elsewhere).
    pub who: u64,
    pub what: GoalChange,
    /// The goal's progress afterwards, 0-100.
    pub pct: u8,
}

/// What a `GoalEvent` records. Target names are as they were then.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GoalChange {
    Created,
    /// A goal from before history was kept, first seen by this version.
    Tracked,
    Number {
        target: String,
        from: f64,
        to: f64,
        unit: String,
    },
    Done {
        target: String,
        done: bool,
    },
    Task {
        target: String,
        task: String,
        done: bool,
    },
    Milestone(u8),
}

/// Where a goal stands against the time it has had.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pace {
    /// At or ahead of where it should be by now.
    OnTrack,
    /// A little behind.
    Behind,
    /// Far behind.
    AtRisk,
    Done,
}

impl Goal {
    /// The share of start..due gone by on `day`, 0-1; None without both days.
    pub fn time_used(&self, day: NaiveDate) -> Option<f32> {
        let (s, d) = (self.start?, self.due?);
        let total = (d - s).num_days();
        if total <= 0 {
            return Some(1.0);
        }
        Some(((day - s).num_days() as f32 / total as f32).clamp(0.0, 1.0))
    }

    /// `progress` (0-1) against the time used on `day`. None when it can't be
    /// told (no start or due) and it isn't done.
    pub fn pace(&self, progress: f32, day: NaiveDate) -> Option<Pace> {
        if progress >= 1.0 {
            return Some(Pace::Done);
        }
        let used = self.time_used(day)?;
        // the first tenth of the time is too early to call anything late
        let gap = used - progress;
        Some(if used < 0.1 || gap <= 0.05 {
            Pace::OnTrack
        } else if gap <= 0.25 {
            Pace::Behind
        } else {
            Pace::AtRisk
        })
    }

    /// The progress (percent) it had at the end of `day`: the last day kept
    /// on or before it.
    pub fn pct_on(&self, day: NaiveDate) -> Option<u8> {
        self.days.iter().rev().find(|(d, _)| *d <= day).map(|x| x.1)
    }

    /// Adds a line to the history, dropping the oldest past the limit.
    pub fn record(&mut self, ev: GoalEvent) {
        self.log.push(ev);
        if self.log.len() > GOAL_LOG_KEEP {
            let extra = self.log.len() - GOAL_LOG_KEEP;
            self.log.drain(..extra);
        }
    }

    /// The day the milestone `m` was reached, from the history.
    pub fn milestone_day(&self, m: u8) -> Option<i64> {
        self.log
            .iter()
            .rev()
            .find(|e| e.what == GoalChange::Milestone(m))
            .map(|e| e.ts)
    }
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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
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
    /// The line icon on the form's tile ("settings", "smile"); empty on
    /// forms from before tiles had icons, which show the forms icon. Once
    /// set it is only ever changed to another icon, never emptied, so the
    /// team merge needs no default rule for it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub icon: String,
    /// The tile's colour by name ("blue", "teal", see `FORM_COLORS` in the
    /// app); empty on older forms, which take one from their id. Like `icon`
    /// it never goes back to empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
    /// When the form was last changed (seconds); 0 on older forms.
    #[serde(default, skip_serializing_if = "is_zero_ts")]
    pub updated: i64,
}

fn is_zero_ts(n: &i64) -> bool {
    *n == 0
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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Automation {
    pub id: u64,
    pub enabled: bool,
    /// When a task's status changes to this status...
    pub trigger_status: String,
    /// ...then: assign, priority, comment or move. For a workflow (`trigger`
    /// "message"): post (a message in `target`), reply (in its thread) or
    /// task (made in the List `value`).
    pub action: String,
    pub value: String,
    /// The List or Space (by id; ids are unique across both) it runs in;
    /// None runs it everywhere.
    pub list: Option<u64>,
    /// What starts it: "" for a task's status (`trigger_status`), or
    /// "message" (a workflow): a message posted in `channel`, with
    /// `keyword` in it when set (app/workflows.rs). Older apps never match
    /// a workflow, since its `trigger_status` is empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub trigger: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<u64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub keyword: String,
    /// Where a workflow posts ("post").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<u64>,
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
    /// My goals passing 25, 50, 75 and 100%.
    pub goal_milestones: bool,
    /// My goals falling far behind their time.
    pub goal_risk: bool,
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
    /// Words to be told about in channels set to "Only @mentions" (and in
    /// threads), separated by commas, like a mention ("deploy, 배포").
    pub keywords: String,
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
            goal_milestones: true,
            goal_risk: true,
            team: true,
            forms: true,
            connection: true,
            channel_default: "mentions".into(),
            channel_modes: vec![],
            keywords: String::new(),
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

    /// `keywords` one by one, trimmed, blanks left out.
    pub fn keyword_list(&self) -> Vec<String> {
        self.keywords
            .split([',', '\n'])
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect()
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
    /// Sidebar sections folded shut by clicking their heading.
    pub folded_nav: Vec<String>,
    pub hidden_views: Vec<String>,
    /// How the Goals page lists goals: "" cards, "list" rows.
    pub goals_view: String,
    /// Theme colour: black, purple, blue, pink, violet, indigo, orange, teal, bronze, mint.
    pub accent: String,
    /// Follow the computer's light / dark setting.
    pub auto_theme: bool,
    /// The row of recently opened tasks at the bottom of the window is hidden.
    pub hide_tray: bool,
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
    /// Messages I started and haven't sent, by conversation and thread
    /// (only on this computer), kept when the workspace is saved so they
    /// come back after a restart; listed in Drafts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chat_drafts: Vec<ChatDraft>,
    /// Messages to post later, kept on this computer when there's no team
    /// server (one keeps them itself; see app/schedule.rs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scheduled: Vec<wb_net::ScheduledMsg>,
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
    /// matching (`slack_map`) and their imports never change it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slack_under_me: Vec<String>,
    /// Slack user ids an earlier version kept under my name on my shared
    /// member (`Member::slack_mine`, still read) that I since pointed
    /// somewhere else: my own, like `slack_under_me`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slack_not_under_me: Vec<String>,
    /// Slack ids whose shared match this computer only carried over from an
    /// older version's lists (`Data::fold_legacy_slack`, or an import that
    /// kept what matching found), with nobody deciding it. If that member is
    /// deleted, the next member the older lists name still counts. My own
    /// note, never shared, so sharing and joining don't change it; a real
    /// decision in the import dialog takes the id off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slack_carried: Vec<String>,
    /// Slack user id -> the member an Import from Slack brings them in as,
    /// so importing again (from any teammate's computer) finds them after a
    /// rename or with no email to match by. Shared entry by entry
    /// (`sync::SHARED_MAPS`): pointing a Slack person at a member sets only
    /// their entry, and of two people setting one entry at once the later
    /// wins everywhere. Several Slack people can go to one member.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub slack_map: std::collections::BTreeMap<String, u64>,
    /// My reading state and notification choices as last agreed with my
    /// other devices through the team server (`sync::mine`), so a change
    /// made here is told from one made there. Only on this computer.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub mine_sync: serde_json::Value,
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
            folded_nav: vec![],
            hidden_views: vec![],
            goals_view: String::new(),
            accent: "black".into(),
            auto_theme: false,
            hide_tray: false,
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
            chat_drafts: vec![],
            scheduled: vec![],
            sheet_rows: vec![],
            sheet_week_rows: vec![],
            slack_under_me: vec![],
            slack_not_under_me: vec![],
            slack_carried: vec![],
            slack_map: Default::default(),
            mine_sync: serde_json::Value::Null,
        }
    }
}

/// An unsent message: in conversation `channel`, or in the thread of its
/// message `thread`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ChatDraft {
    pub channel: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread: Option<u64>,
    pub text: String,
}
