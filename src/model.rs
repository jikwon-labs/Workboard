// Data model for the workspace: spaces, folders, lists, tasks and members.
// Default statuses and priorities follow the values recorded in MECHANICS.md.

use chrono::{Datelike, Duration, Local, NaiveDate};
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
        Self { name: name.to_string(), color, kind }
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
    PriorityDef { label: "No Priority", color: [0x87, 0x90, 0x9e] },
    PriorityDef { label: "Urgent", color: [0xf5, 0x00, 0x00] },
    PriorityDef { label: "High", color: [0xff, 0xcc, 0x00] },
    PriorityDef { label: "Normal", color: [0x6f, 0xdd, 0xff] },
    PriorityDef { label: "Low", color: [0xd8, 0xd8, 0xd8] },
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
}

/// The people a text @mentions. At each "@" that starts a word the longest
/// matching name wins, so "@Alex Kim" is Alex Kim and not Alex. A Latin name
/// running on into more letters or digits ("@Alexander") is not that name;
/// anything else may follow it, so "@지원님" still mentions 지원. When several
/// people share the name, the ones in `picked` (chosen from the mention list)
/// are meant, else all of them.
pub fn mentioned_ids(text: &str, members: &[Member], picked: &[u64]) -> Vec<u64> {
    let mut out: Vec<u64> = vec![];
    for (i, _) in text.match_indices('@') {
        if i > 0 && !text[..i].ends_with(char::is_whitespace) {
            continue;
        }
        let rest = &text[i + 1..];
        let fits = |name: &str| {
            !name.is_empty()
                && rest.starts_with(name)
                && !(name.chars().last().is_some_and(|c| c.is_ascii_alphanumeric())
                    && rest[name.len()..].chars().next().is_some_and(|c| c.is_ascii_alphanumeric()))
        };
        let Some(best) = members.iter().filter(|m| fits(&m.name)).map(|m| m.name.len()).max() else { continue };
        let named: Vec<u64> = members.iter().filter(|m| m.name.len() == best && fits(&m.name)).map(|m| m.id).collect();
        let chosen: Vec<u64> = named.iter().copied().filter(|id| picked.contains(id)).collect();
        for id in if chosen.is_empty() { named } else { chosen } {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TimeEntry {
    pub start: i64,
    pub end: i64,
    /// Whose time this is; 0 for entries from before entries had an owner
    /// (see `Task::entry_owner`).
    #[serde(default)]
    pub member: u64,
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
    pub tags: Vec<String>,
    pub checklist: Vec<CheckItem>,
    pub activity: Vec<Activity>,
    pub time_entries: Vec<TimeEntry>,
    pub timer_started: Option<i64>,
    /// Who started the running timer; None for timers started before this
    /// was recorded (see `Data::is_my_timer`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_by: Option<u64>,
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
        let n = counts.entry((a.ts, a.author, a.comment, a.text.clone())).or_insert(0);
        if a.id == 0 {
            a.id = wb_sync::activity_id(tid, a.ts, a.author, a.comment, &a.text, *n);
        }
        *n += 1;
        seen.insert(a.id)
    });
}

impl Task {
    pub fn tracked_secs(&self, now: i64) -> i64 {
        let mut s: i64 = self.time_entries.iter().map(|e| e.end - e.start).sum();
        if let Some(st) = self.timer_started {
            s += now - st;
        }
        s
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
    Number { start: f64, target: f64, current: f64, unit: String },
    TrueFalse { done: bool },
    Tasks { ids: Vec<u64> },
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
    /// Who made it; 0 for forms from before this was recorded.
    #[serde(default)]
    pub owner: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WbStroke {
    pub color: [u8; 3],
    pub width: f32,
    pub points: Vec<[f32; 2]>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sticky {
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
    #[serde(default)]
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
}

impl ListTrash {
    pub fn parse(payload: &str) -> Option<ListTrash> {
        if let Ok(t) = serde_json::from_str::<ListTrash>(payload) {
            return Some(t);
        }
        let (space, list, tasks) = serde_json::from_str::<(u64, ListItem, Vec<Task>)>(payload).ok()?;
        Some(ListTrash { space, folder: None, list, tasks })
    }
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskTemplate {
    pub id: u64,
    pub name: String,
    pub task_name: String,
    pub description: String,
    pub checklist: Vec<String>,
    pub tags: Vec<String>,
    pub priority: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bookmark {
    pub id: u64,
    pub title: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Dashboard {
    pub id: u64,
    pub name: String,
    pub space: Option<u64>,
    /// Who made it; 0 for dashboards from before this was recorded.
    #[serde(default)]
    pub owner: u64,
    /// Which cards it shows: "simple", "time", "project", or "" for all of them.
    #[serde(default)]
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
        Self { priorities: true, tags: true, time_tracking: true, time_estimates: true, custom_fields: true, multiple_assignees: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct NotifySettings {
    pub comments: bool,
    pub mentions: bool,
    pub reminders: bool,
    pub chat: bool,
    pub assigned: bool,
}

impl Default for NotifySettings {
    fn default() -> Self {
        Self { comments: true, mentions: true, reminders: true, chat: true, assigned: true }
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
    /// Tasks people added to their own timesheet with "+ Add task":
    /// [member, task] pairs.
    #[serde(default)]
    pub sheet_rows: Vec<[u64; 2]>,
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
            replies_read: vec![],
            welcome: false,
            closed_dms: vec![],
            sheet_rows: vec![],
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
    }

    pub fn dir() -> PathBuf {
        let mut p = Self::path();
        p.pop();
        p
    }

    pub fn workspaces() -> Vec<WsEntry> {
        let p = Self::dir().join("workspaces.json");
        std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// The workspace file open when the app last ran.
    pub fn last_workspace() -> Option<String> {
        let f = std::fs::read_to_string(Self::dir().join("last-workspace")).ok()?.trim().to_string();
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
        let d = serde_json::from_str(&s).ok();
        if d.is_none() {
            Self::keep_unreadable(file, &s);
        }
        d
    }

    /// A saved workspace that couldn't be read is copied aside before
    /// anything can save over it; the app tells the person where it went.
    fn keep_unreadable(file: &str, text: &str) {
        let stem = file.strip_suffix(".json").unwrap_or(file);
        let p = Self::dir().join(format!("{}-unreadable-{}.json", stem, now_ts()));
        eprintln!("workboard: couldn't read {}; a copy is kept at {}", file, p.display());
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
        d.spaces.push(Space { id: sid, name: "Team Space".into(), color: [0x5f, 0x55, 0xee], statuses: default_statuses(), folders: vec![], lists: vec![ListItem { id: lid, name: "List".into(), ..Default::default() }], expanded: true, private_owner: None, ..Default::default() });
        let cid = d.new_id();
        d.channels.push(Channel { id: cid, name: "general".into(), kind: ChannelKind::Channel, topic: String::new(), members: vec![owner.id], messages: vec![], last_read: now_ts(), ..Default::default() });
        d.extra.demo_removed = true;
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
                    let _ = std::fs::write(Self::dir().join(format!("data-before-cleanup-{}.json", now_ts())), &s);
                    d.remove_demo();
                    d.save();
                }
                // every workspace has a #general channel to talk in
                if !d.channels.iter().any(|c| c.kind == ChannelKind::Channel) {
                    let id = d.new_id();
                    let members = d.members.iter().map(|m| m.id).collect();
                    d.channels.insert(0, Channel { id, name: "general".into(), kind: ChannelKind::Channel, topic: String::new(), members, messages: vec![], last_read: now_ts(), ..Default::default() });
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
        let owner = Member { id: 1, name, color: [0x5f, 0x55, 0xee], email: String::new(), role: "Owner".into(), ..Default::default() };
        let mut d = Self::blank("My Workspace", owner);
        d.extra.welcome = true;
        d
    }

    /// Take out the example content (people, spaces, tasks, docs, chat...)
    /// that earlier versions started with, keeping everything the person made.
    pub fn remove_demo(&mut self) {
        use serde_json::Value;
        use std::collections::HashSet;
        fn label(v: &Value) -> Option<String> {
            ["name", "title", "text"].iter().find_map(|k| v.get(*k).and_then(|x| x.as_str()).map(|x| x.to_string()))
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
        fn filter(v: &mut Value, key: &str, demo: &HashSet<(String, String)>, keep: &HashSet<u64>) -> bool {
            let mut stays = false;
            match v {
                Value::Array(a) => {
                    a.retain_mut(|x| {
                        let inner = filter(x, key, demo, keep);
                        let Some(id) = x.get("id").and_then(|i| i.as_u64()) else { return true };
                        let is_demo = id < 1000 && label(x).map_or(false, |l| demo.contains(&(key.to_string(), l)));
                        let keep_it = !is_demo || inner || keep.contains(&id);
                        stays |= keep_it;
                        keep_it
                    });
                }
                Value::Object(o) => {
                    for (k, x) in o.iter_mut() {
                        if x.is_array() {
                            let had = x.as_array().map_or(false, |a| a.iter().any(|i| i.get("id").is_some()));
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
        let demo_task = |t: &Task| t.id < 1000 && demo.contains(&("tasks".to_string(), t.name.clone()));
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
            let lists: HashSet<u64> = d.spaces.iter().flat_map(|s| s.lists.iter().map(|l| l.id).chain(s.folders.iter().flat_map(|f| f.lists.iter().map(|l| l.id)))).collect();
            let spaces: HashSet<u64> = d.spaces.iter().map(|s| s.id).collect();
            let folders: HashSet<u64> = d.spaces.iter().flat_map(|s| s.folders.iter().map(|f| f.id)).collect();
            d.favorites.retain(|f| match f {
                Location::List(id) => lists.contains(id),
                Location::Space(id) => spaces.contains(id),
                Location::Folder(id) => folders.contains(id),
                _ => true,
            });
            if d.spaces.is_empty() {
                let sid = d.new_id();
                let lid = d.new_id();
                d.spaces.push(Space { id: sid, name: "Team Space".into(), color: [0x5f, 0x55, 0xee], statuses: default_statuses(), folders: vec![], lists: vec![ListItem { id: lid, name: "List".into(), ..Default::default() }], expanded: true, private_owner: None, ..Default::default() });
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
                || s.folders.iter().any(|f| f.lists.iter().any(|l| l.id == list_id))
        })
    }

    /// True when the list, or the folder or space it sits in, is archived.
    pub fn list_archived(&self, id: u64) -> bool {
        self.list_item(id).is_some_and(|l| l.archived)
            || self.space_of_list(id).is_some_and(|s| s.archived || s.folders.iter().any(|f| f.archived && f.lists.iter().any(|l| l.id == id)))
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
        t.timer_started.is_some()
            && match t.timer_by {
                Some(m) => m == self.me,
                None => self.solo(),
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
        let me = self.me;
        let mine: Vec<usize> = (0..self.tasks.len()).filter(|i| self.is_my_timer(&self.tasks[*i])).collect();
        for i in mine {
            let t = &mut self.tasks[i];
            if let Some(st) = t.timer_started.take() {
                t.time_entries.push(TimeEntry { start: st, end: now, member: me });
            }
            t.timer_by = None;
        }
    }

    /// The letter or symbol drawn on a space's square.
    pub fn space_glyph(s: &Space) -> String {
        if s.icon.is_empty() {
            s.name.chars().next().unwrap_or('S').to_uppercase().to_string()
        } else {
            s.icon.clone()
        }
    }

    /// The list with this id, wherever it sits (space or folder).
    pub fn list_item(&self, id: u64) -> Option<&ListItem> {
        self.spaces.iter().flat_map(|s| s.lists.iter().chain(s.folders.iter().flat_map(|f| f.lists.iter()))).find(|l| l.id == id)
    }

    pub fn list_item_mut(&mut self, id: u64) -> Option<&mut ListItem> {
        self.spaces.iter_mut().flat_map(|s| s.lists.iter_mut().chain(s.folders.iter_mut().flat_map(|f| f.lists.iter_mut()))).find(|l| l.id == id)
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
        matches!(self.status_of(t).kind, StatusKind::Closed | StatusKind::Done)
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
            Location::Folder(id) => self.spaces.iter().any(|s| s.folders.iter().any(|f| f.id == id)),
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
                Member { id: 1, name: "Me".into(), color: [0x5f, 0x55, 0xee], email: "me@example.com".into(), role: "Owner".into(), ..Default::default() },
                Member { id: 2, name: "Alex Kim".into(), color: [0x10, 0x90, 0xe0], email: "alex@example.com".into(), role: "Admin".into(), ..Default::default() },
                Member { id: 3, name: "Sam Lee".into(), color: [0xe1, 0x6b, 0x16], email: "sam@example.com".into(), role: "Member".into(), ..Default::default() },
                Member { id: 4, name: "Jordan Park".into(), color: [0x00, 0x88, 0x44], email: "jordan@example.com".into(), role: "Member".into(), ..Default::default() },
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
                    ListItem { id: l_web, name: "Website Redesign".into(), ..Default::default() },
                    ListItem { id: l_mkt, name: "Marketing".into(), ..Default::default() },
                ],
                expanded: true,
                archived: false,
            }],
            lists: vec![ListItem { id: l_personal, name: "Personal".into(), ..Default::default() }],
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
            lists: vec![ListItem { id: l_ideas, name: "Ideas".into(), ..Default::default() }],
            expanded: false,
            private_owner: None,
            ..Default::default()
        });
        d.favorites.push(Location::List(l_web));

        let t = today();
        let wd = t.weekday().num_days_from_monday() as i64;
        let day = |n: i64| Some(t + Duration::days(n));
        let samples: Vec<(u64, &str, &str, u8, Vec<u64>, Option<NaiveDate>, Option<NaiveDate>, Vec<&str>)> = vec![
            (l_web, "Design new homepage layout", "in progress", 2, vec![1, 2], day(-wd), day(2), vec!["design"]),
            (l_web, "Write copy for landing page", "to do", 3, vec![3], day(1), day(4), vec!["content"]),
            (l_web, "Set up analytics dashboard", "to do", 4, vec![], None, day(9), vec![]),
            (l_web, "Fix navigation on mobile", "in progress", 1, vec![1], day(-3), day(-1), vec!["bug"]),
            (l_web, "Collect feedback from beta users", "to do", 0, vec![4], None, None, vec![]),
            (l_web, "Choose color palette", "complete", 3, vec![2], day(-8), day(-5), vec!["design"]),
            (l_mkt, "Plan Q1 campaign", "in progress", 2, vec![1, 3], day(-2), day(6), vec![]),
            (l_mkt, "Newsletter draft", "to do", 3, vec![3], None, day(3), vec!["content"]),
            (l_mkt, "Social media calendar", "complete", 4, vec![4], day(-10), day(-6), vec![]),
            (l_personal, "Renew passport", "to do", 2, vec![1], None, day(0), vec![]),
            (l_personal, "Book dentist appointment", "to do", 3, vec![1], None, day(12), vec![]),
            (l_ideas, "Build a reading tracker", "to do", 0, vec![1], None, None, vec![]),
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
                tags: tags.into_iter().map(String::from).collect(),
                checklist: vec![],
                activity: vec![Activity { ts: now - 86400 * 3, author: 1, text: "created this task".into(), comment: false, ..Default::default() }],
                time_entries: vec![],
                timer_started: None,
                timer_by: None,
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
        for (n, name) in ["Wireframe hero section", "Pick hero image"].iter().enumerate() {
            let id = d.new_id();
            d.tasks.push(Task {
                id,
                list_id: l_web,
                parent: Some(first),
                name: (*name).into(),
                description: String::new(),
                status: if n == 0 { "complete".into() } else { "to do".into() },
                priority: 0,
                assignees: vec![2],
                start: None,
                due: None,
                tags: vec![],
                checklist: vec![],
                activity: vec![],
                time_entries: vec![],
                timer_started: None,
                timer_by: None,
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
        d.tasks[0].description = "Refresh the homepage with a cleaner hero, clear call to action and a lighter grid.".into();
        d.tasks[0].checklist = vec![
            CheckItem { text: "Review competitors".into(), done: true, assignee: None },
            CheckItem { text: "Sketch three options".into(), done: false, assignee: None },
            CheckItem { text: "Share with the team".into(), done: false, assignee: None },
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
        };
        let c1 = d.new_id();
        let mut m1 = msg(&mut d, 2, "Morning everyone! The new homepage mockups are in the Website Redesign list.", 3 * 3600);
        m1.reactions.push(Reaction { emoji: "\u{1F44D}".into(), users: vec![3, 4] });
        let r1 = msg(&mut d, 1, "Looks great, I'll review them after lunch.", 2 * 3600);
        m1.replies.push(r1);
        let m2 = msg(&mut d, 3, "Reminder: newsletter draft is due Thursday.", 3600);
        let m3 = msg(&mut d, 4, "@Me can you check the beta feedback list when you have a minute?", 1200);
        d.channels.push(Channel { id: c1, name: "general".into(), kind: ChannelKind::Channel, topic: "Company-wide announcements and chat".into(), members: vec![1, 2, 3, 4], messages: vec![m1, m2, m3], last_read: now2 - 1800, ..Default::default() });
        let c2 = d.new_id();
        let m4 = msg(&mut d, 2, "Should the hero image be a photo or an illustration?", 5400);
        d.channels.push(Channel { id: c2, name: "website-redesign".into(), kind: ChannelKind::Channel, topic: "Everything about the new site".into(), members: vec![1, 2], messages: vec![m4], last_read: now2, ..Default::default() });
        let c3 = d.new_id();
        let m5 = msg(&mut d, 2, "Hey, do you have a minute to talk about the launch date?", 600);
        d.channels.push(Channel { id: c3, name: String::new(), kind: ChannelKind::Direct, topic: String::new(), members: vec![1, 2], messages: vec![m5], last_read: 0, ..Default::default() });
        let c4 = d.new_id();
        d.channels.push(Channel { id: c4, name: String::new(), kind: ChannelKind::Direct, topic: String::new(), members: vec![1, 3], messages: vec![], last_read: now2, ..Default::default() });
        d.tasks[0].time_entries.push(TimeEntry { start: now2 - 86400 - 5400, end: now2 - 86400, member: 0 });
        d.tasks[3].time_entries.push(TimeEntry { start: now2 - 7200, end: now2 - 4800, member: 0 });
        d.tasks[0].activity.push(Activity { ts: now - 3600 * 5, author: 2, text: "Looks great so far, let's review on Friday.".into(), comment: true, ..Default::default() });
        d
    }
}
