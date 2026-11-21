// The shapes of a workspace: statuses and priorities, Spaces, Folders and
// Lists, members, documents, chat channels and messages, and locations.

use super::*;

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

/// `default_statuses`, made once, to lend out.
pub fn default_statuses_ref() -> &'static [Status] {
    static D: std::sync::OnceLock<Vec<Status>> = std::sync::OnceLock::new();
    D.get_or_init(default_statuses)
}

/// Priority levels: 0 = none, 1 = urgent, 2 = high, 3 = normal, 4 = low.
pub struct PriorityDef {
    pub label: &'static str,
    pub color: [u8; 3],
}

/// The priority a new task starts with when whoever makes it doesn't pick
/// one: Normal (an index into `PRIORITIES`).
pub const DEFAULT_PRIORITY: u8 = 3;

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

pub(super) fn yes() -> bool {
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
    /// The full name (Edit profile's "Full name").
    pub name: String,
    pub color: [u8; 3],
    /// What the person likes to be called; shown before the full name
    /// wherever others see them (`shown`). Empty when not set. Always
    /// written, so clearing it reaches the others too.
    #[serde(default)]
    pub display_name: String,
    /// What they do on the team ("Designer"), shown with their name.
    #[serde(default)]
    pub title: String,
    /// Their profile photo: a square picture in the file cache and on the
    /// server, like a chat picture. Kept as an attachment so the cache
    /// cleanup, sharing and the server's file cleanup count it as in use.
    #[serde(default)]
    pub photo: Option<Attachment>,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub role: MemberRole,
    /// Invited by email and not joined yet.
    #[serde(default)]
    pub invited: bool,
    #[serde(default)]
    pub status_text: String,
    /// Only read: the Slack user ids an earlier version's Import from Slack
    /// brought in as this member. Matching now keeps them per Slack id in
    /// the shared `Extra::slack_map` (one list per member merged as a whole,
    /// so two people pointing Slack people at one member at once lost one);
    /// these go into it when a workspace opens (`Data::fold_legacy_slack`)
    /// and stay here as they are, for older versions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slack: Vec<String>,
    /// Only read, from files of an earlier version that kept "Import under
    /// my name" here, on the shared member: my own member's list is copied
    /// to my personal `Extra::slack_under_me` (`Data::keep_slack_under_me_personal`)
    /// and left as it is, so each of my computers takes it over on its own
    /// when it updates. Nothing writes it any more.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slack_mine: Vec<String>,
}

/// The people a text @mentions, by the rule the team server uses too
/// (`wb_sync::mentions`): at each "@" that starts a word the longest
/// matching name wins, so "@Alex Kim" is Alex Kim and not Alex; "@Alexander"
/// is not Alex but "@지원님" is 지원. When several people share the name, the
/// ones in `picked` (chosen from the mention list) are meant, else all.
pub fn mentioned_ids(text: &str, members: &[Member], picked: &[u64]) -> Vec<u64> {
    let people: Vec<(u64, &str)> = members
        .iter()
        .flat_map(|m| m.names().into_iter().map(move |n| (m.id, n)))
        .collect();
    wb_sync::mentions::mentioned_ids(text, &people, picked)
}

/// A member's role in People. Saved as its name ("Owner", "Admin",
/// "Member", "Guest"); "" from members made before roles counts as Member
/// and is saved as "" again, and a role from a newer app is kept as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum MemberRole {
    /// "": counts as Member.
    #[default]
    Unset,
    Owner,
    Admin,
    Member,
    Guest,
    Other(String),
}

impl MemberRole {
    pub fn of(name: &str) -> MemberRole {
        match name {
            "" => MemberRole::Unset,
            "Owner" => MemberRole::Owner,
            "Admin" => MemberRole::Admin,
            "Member" => MemberRole::Member,
            "Guest" => MemberRole::Guest,
            _ => MemberRole::Other(name.to_string()),
        }
    }

    /// The name saved for it.
    pub fn name(&self) -> &str {
        match self {
            MemberRole::Unset => "",
            MemberRole::Owner => "Owner",
            MemberRole::Admin => "Admin",
            MemberRole::Member => "Member",
            MemberRole::Guest => "Guest",
            MemberRole::Other(s) => s,
        }
    }

    /// What it counts as: Member for "".
    pub fn counted(&self) -> MemberRole {
        match self {
            MemberRole::Unset => MemberRole::Member,
            r => r.clone(),
        }
    }
}

impl From<&str> for MemberRole {
    fn from(name: &str) -> Self {
        MemberRole::of(name)
    }
}

impl Serialize for MemberRole {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for MemberRole {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(MemberRole::of(&String::deserialize(d)?))
    }
}

impl Member {
    /// An owner or admin, by the role in People.
    pub fn manages(&self) -> bool {
        matches!(self.role, MemberRole::Owner | MemberRole::Admin)
    }

    /// The name others see: the display name when there is one, else the
    /// full name.
    pub fn shown(&self) -> &str {
        let d = self.display_name.trim();
        if d.is_empty() {
            &self.name
        } else {
            d
        }
    }

    /// Every name the person goes by (the full name, then the display name
    /// when it differs), for @mentions and search.
    pub fn names(&self) -> Vec<&str> {
        let mut v = vec![self.name.as_str()];
        let d = self.display_name.trim();
        if !d.is_empty() && d != self.name.trim() {
            v.push(d);
        }
        v
    }

    /// Whether `pred` holds for any of the person's names.
    pub fn any_name(&self, pred: impl Fn(&str) -> bool) -> bool {
        self.names().into_iter().any(pred)
    }

    pub fn initials(&self) -> String {
        let mut s: String = self
            .shown()
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
    /// The page its first web address points to (title, description), got
    /// once by the writer's app after sending, so others don't each visit
    /// the page. Its writer can take it away.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<LinkPreview>,
    /// A message forwarded from another conversation, shown under the text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forwarded: Option<Forwarded>,
    /// The channel's copy of a reply in thread `thread_of` ("Also send to
    /// the channel"): the reply itself notifies, not this copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_of: Option<u64>,
    /// Posted by a webhook or slash command (server/src/hooks.rs): its name,
    /// shown in place of the person who set it up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<String>,
    /// A goal shared to the conversation from its page ("Share to team
    /// chat"), as it stood then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<GoalCard>,
}

/// The message a forwarded one carries: what it said, who wrote it, when
/// and where.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Forwarded {
    pub channel: u64,
    pub message: u64,
    pub author: u64,
    pub ts: i64,
    #[serde(default)]
    pub text: String,
}

/// A goal as a message shows it under its text: a copy taken when it was
/// shared, so the card reads the same after the goal changes.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct GoalCard {
    /// The goal it came from (opened by "Open goal" while it exists).
    pub goal: u64,
    pub name: String,
    pub color: [u8; 3],
    pub pct: u8,
    pub targets: usize,
    pub targets_done: usize,
    /// Days before (more than 0) or after (less) the due date it was
    /// achieved; None without both days.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early: Option<i64>,
    /// Days from start to achieved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
    /// Who moved it, from its history.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<u64>,
}

/// A web page as a message shows it under its text.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct LinkPreview {
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// the site's name ("GitHub"), else its address ("github.com")
    #[serde(default)]
    pub site: String,
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

    #[cfg(test)]
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
