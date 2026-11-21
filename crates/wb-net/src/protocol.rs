// The messages app and server send each other (JSON text over a WebSocket)
// and the types and limits they share.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_PORT: u16 = 8787;
pub const PATH: &str = "/ws";
/// A whole workspace travels in one message, so allow big ones.
pub const MAX_MESSAGE: usize = 256 << 20;
pub const PING_SECS: u64 = 30;
/// Nothing heard for this long (not even a pong) means the line is dead.
pub const SILENT_SECS: u64 = 75;

/// What someone may do on the server. The owner set the workspace up.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    Member,
    Admin,
    Owner,
}

impl Role {
    /// May invite people, change roles and remove access.
    pub fn manages(self) -> bool {
        self >= Role::Admin
    }

    pub fn label(self) -> &'static str {
        match self {
            Role::Member => "Member",
            Role::Admin => "Admin",
            Role::Owner => "Owner",
        }
    }
}

/// One person's access, for owners and admins.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Person {
    pub member: u64,
    pub role: Role,
    /// Computers signed in as this person.
    pub devices: usize,
    /// An invite code not used yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite: Option<String>,
    /// Has set up an account (email and password).
    #[serde(default, skip_serializing_if = "is_false")]
    pub account: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The shortest password the server takes.
pub const MIN_PASSWORD: usize = 8;

/// A huddle participant's LiveKit identity, "<member id>-<device>": the
/// server writes it into the call token (one identity per computer, so the
/// same person can be in a call from two), and apps read the member back
/// out of it (`identity_member`).
pub fn call_identity(member: u64, device: &str) -> String {
    format!("{}-{}", member, device)
}

/// The member a huddle participant is, from their identity (`call_identity`).
pub fn identity_member(identity: &str) -> Option<u64> {
    identity.split('-').next()?.parse().ok()
}

/// A backup of the team's copy on the server.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Backup {
    /// File name in the server's backups folder, e.g. hourly-20261115-03.json
    pub name: String,
    pub bytes: u64,
}

impl Backup {
    /// "2026-11-15 03:00 (hourly)" from the file name.
    pub fn label(&self) -> String {
        let stem = self.name.trim_end_matches(".json");
        let (kind, rest) = if let Some(r) = stem.strip_prefix("hourly-") {
            ("hourly", r)
        } else if let Some(r) = stem.strip_prefix("daily-") {
            ("daily", r)
        } else if let Some(r) = stem.strip_prefix("before-restore-") {
            ("before a restore", r)
        } else {
            return self.name.clone();
        };
        let d: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
        let date = if d.len() >= 8 {
            format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])
        } else {
            rest.to_string()
        };
        let time = match d.len() {
            10 => format!(" {}:00", &d[8..10]),
            14 => format!(" {}:{}:{}", &d[8..10], &d[10..12], &d[12..14]),
            _ => String::new(),
        };
        format!("{}{} ({})", date, time, kind)
    }
}

/// The oldest sync format (`wb_sync::META_V`) whose changes the server
/// takes. Older apps would erase fields they don't know when their copy
/// wins, or (format 3 and before) note a whole time entry list or custom
/// field map as one field, so a change of theirs would undo a teammate's
/// entry or value in it (see `UPDATE_NEEDED`). Format 4 apps drop the ids
/// of goal targets and history lines and, merging the team's copy into
/// theirs, keep both the copies with ids and their own without, so every
/// round trip doubled a goal's targets.
pub const OLDEST_SYNC: u32 = 5;

/// Said to an app older than `OLDEST_SYNC` when it sends changes. It goes
/// with `CODE_DENIED`, which every app shows and stays connected for.
pub const UPDATE_NEEDED: &str = "This version of Workboard is too old to send changes to your team's server. Update Workboard to keep working together; until then you only see what others change.";

/// Why the server said no to signing in. The app stops trying.
pub const CODE_AUTH: &str = "auth";
/// A request the person isn't allowed to make. The connection stays.
pub const CODE_DENIED: &str = "denied";

/// Said when the workspace an app connects to was deleted from the server.
pub const WORKSPACE_GONE: &str = "This workspace was deleted from the server by its owner.";

/// A device's notification choices, as its app keeps them (NotifySettings
/// on the Mac, NotifyPrefs on the iPhone), for the server's pushes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PushPrefs {
    #[serde(default = "yes")]
    pub direct: bool,
    #[serde(default = "yes")]
    pub mentions: bool,
    /// replies in threads I started or wrote in
    #[serde(default = "yes")]
    pub replies: bool,
    /// channel messages: "all", "mentions" or "nothing"
    #[serde(default = "mentions_mode")]
    pub channel_default: String,
    /// channels set apart from `channel_default`
    #[serde(default)]
    pub channel_modes: Vec<ChannelMode>,
    /// words to be told about like a mention
    #[serde(default)]
    pub keywords: Vec<String>,
    /// "Pause notifications" until this time (seconds)
    #[serde(default)]
    pub muted_until: i64,
}

/// A webhook an owner or admin set up (server/src/hooks.rs): "in" posts
/// what is sent to the server's /hooks/<token> address in `channel` under
/// `name`; "out" sends each new message of `channel` (starting with
/// `trigger`, when set) to `url`, and posts what that answers.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct HookInfo {
    /// made by the server (the secret part of an "in" hook's address)
    #[serde(default)]
    pub token: String,
    pub kind: String,
    pub channel: u64,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub trigger: String,
    #[serde(default)]
    pub creator: u64,
}

/// A slash command an owner or admin added: "/name text" in a message box
/// sends the text to `url` (only owners and admins are told it), and what
/// it answers is posted in the conversation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct CommandInfo {
    /// without the slash
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
}

/// A chat message the server keeps until `at` (seconds) and then posts as
/// its writer's ("Schedule message").
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ScheduledMsg {
    /// the message's id once posted (made by the app)
    pub id: u64,
    pub channel: u64,
    /// the thread it goes in
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
    pub text: String,
    pub at: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChannelMode {
    pub channel: u64,
    pub mode: String,
}

fn yes() -> bool {
    true
}

fn mentions_mode() -> String {
    "mentions".into()
}

impl Default for PushPrefs {
    fn default() -> Self {
        PushPrefs {
            direct: true,
            mentions: true,
            replies: true,
            channel_default: mentions_mode(),
            channel_modes: vec![],
            keywords: vec![],
            muted_until: 0,
        }
    }
}

impl PushPrefs {
    /// "all", "mentions" or "nothing" for a channel.
    pub fn channel_mode(&self, channel: u64) -> &str {
        self.channel_modes
            .iter()
            .find(|c| c.channel == channel)
            .map(|c| c.mode.as_str())
            .unwrap_or(&self.channel_default)
    }
}

/// What the AI is asked to do (`Msg::Ai`; the server's ai.rs does it).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AiAsk {
    /// Sum up a channel or DM: its newest messages, with their threads.
    SummarizeChannel { channel: u64 },
    /// Sum up the thread under message `message` of `channel`.
    SummarizeThread { channel: u64, message: u64 },
    /// Do `how` (one of `AI_WRITE`) to a piece of writing: a doc page, a
    /// part of it, or a task description. `title` says what it is about.
    Write {
        how: String,
        text: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        title: String,
    },
    /// Subtasks for a task; the answer is JSON `{"subtasks": [names]}`.
    Subtasks {
        name: String,
        #[serde(default)]
        description: String,
    },
    /// A question about the workspace (what the asker may see of it), after
    /// the questions and answers before it in the same conversation.
    Question {
        question: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        history: Vec<AiTurn>,
    },
}

/// One earlier question and its answer (`AiAsk::Question`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AiTurn {
    pub question: String,
    pub answer: String,
}

/// What `AiAsk::Write` can do with a piece of writing.
pub const AI_WRITE: [&str; 9] = [
    "improve",
    "fix",
    "shorter",
    "longer",
    "summarize",
    "korean",
    "english",
    "continue",
    "draft",
];

/// The longest text one AI request may carry (characters): writing to
/// change, a question, a task's name and description together.
pub const AI_MAX_TEXT: usize = 60_000;

/// Earlier questions and answers an AI question takes along; the server
/// reads no more than this.
pub const AI_MAX_HISTORY: usize = 10;

/// The largest file the server takes. Under the 100 MB a request may carry
/// through Cloudflare (Tunnel) on its free plan, so a server behind it takes
/// the same files as one reached directly.
pub const MAX_FILE: u64 = 90 << 20;

/// An email address as the server takes one: something before the @, a dot
/// inside what follows, no spaces (surrounding spaces don't count).
pub fn email_ok(e: &str) -> bool {
    let e = e.trim();
    e.split_once('@').is_some_and(|(name, domain)| {
        !name.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && !e.contains(char::is_whitespace)
    })
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Msg {
    Hello {
        device: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tag: Option<u16>,
        /// This device's token from an earlier welcome.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// A one-time invite code, when joining.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        invite: Option<String>,
        /// Setting up an empty server: who I am in the workspace I bring.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        claim: Option<u64>,
        /// The account's email: signing in with `password`, choosing a
        /// password with `new_password`, or resetting with `reset_code`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        password: Option<String>,
        /// With an invite, a claim or a reset code: the password to set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_password: Option<String>,
        /// The code a password-reset email gave.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reset_code: Option<String>,
        /// A short code another device of the same person showed (as a QR
        /// code or six letters and digits, see `create_link_code`): signs
        /// this device in as that person.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        link_code: Option<String>,
        /// The app's version ("0.1.7"), for the server's log and messages.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        app: Option<String>,
        /// The sync format the app writes (`wb_sync::META_V`). Apps from
        /// before it send none; the server takes no changes from apps older
        /// than `OLDEST_SYNC` (they still get the team's copy).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sync: Option<u32>,
        /// The app takes `delta` messages.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        deltas: bool,
        /// Only notifications, for a workspace that isn't open on this
        /// computer (`Watcher`): the server answers `watching`, then sends
        /// `note` for this person's new messages and nothing else. The
        /// device doesn't count as online. Servers from before it answer
        /// `welcome` as usual, and the watcher hangs up.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        watch: bool,
    },
    Welcome {
        /// The id tag this device must use; the server keeps them unique.
        tag: u16,
        /// The team's copy, or nothing when the server is still empty.
        snapshot: Option<Value>,
        rev: u64,
        /// The member this device signs in as.
        member: u64,
        role: Role,
        /// A new token to keep (after an invite or a claim).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// This person's account email, when they have set up an account.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        /// Items of the team's copy this person doesn't get (a private
        /// channel they left, someone's Personal space): the app takes its
        /// own copy of them away without deleting them (`wb_sync::hide`).
        /// Older apps ignore it.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hidden: Vec<u64>,
        /// The server sends `delta` messages to apps that take them, and
        /// takes pushes of only what changed.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        deltas: bool,
    },
    /// The app's shared copy, whole or only what changed (`wb_sync::delta`).
    Push {
        data: Value,
    },
    /// The team's whole copy.
    Update {
        data: Value,
        rev: u64,
        /// As in `Welcome`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hidden: Vec<u64>,
    },
    /// What changed in the team's copy from revision `base` to `rev`, for
    /// apps that said `deltas`. `hidden` is the whole list, as in `Welcome`.
    Delta {
        data: Value,
        rev: u64,
        base: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        hidden: Vec<u64>,
    },
    /// The app missed a revision: send the whole copy again (as `update`).
    Resync,
    Ping,
    Pong,
    /// iPhone: this device's Apple push token (hex), so the server can notify
    /// it of new messages while the app isn't connected. `env` is
    /// "development" or "production" (which APNs host the token belongs to).
    /// An empty token stops notifications to this device. Servers from before
    /// it ignore the message.
    PushToken {
        token: String,
        #[serde(default)]
        platform: String,
        #[serde(default)]
        env: String,
        /// The phone's own name for this workspace, sent back in each
        /// notification as "ws" so a tap opens the right one while another
        /// workspace is open. Servers from before it leave "ws" out.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
        /// What this device wants to be told about (sent again whenever it
        /// changes). Without it the device gets direct messages, mentions
        /// and thread replies, as before choices came.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prefs: Option<PushPrefs>,
    },
    /// The answer to a `watch` hello: signed in, notes follow.
    Watching,
    /// A new message for the person behind a `watch` connection: what the
    /// iPhone's push says (server/src/push.rs), and where it is.
    Note {
        title: String,
        #[serde(default)]
        body: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<u64>,
        /// The thread a reply is in.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<u64>,
    },
    /// Keep a message to post later (replaces one with the same id).
    /// Answered with `scheduled`. Servers from before it ignore it.
    Schedule {
        item: ScheduledMsg,
    },
    /// Don't post the scheduled message `id` after all.
    Unschedule {
        id: u64,
    },
    /// My scheduled messages, please.
    ScheduledList,
    /// The webhooks and slash commands, please (answered with `integrations`;
    /// webhooks only for owners and admins).
    Integrations,
    IntegrationList {
        #[serde(default)]
        hooks: Vec<HookInfo>,
        #[serde(default)]
        commands: Vec<CommandInfo>,
    },
    /// Owner or admin: set up a webhook (the server makes its token).
    HookAdd {
        hook: HookInfo,
    },
    HookRemove {
        token: String,
    },
    /// Owner or admin: add (or change) a slash command.
    CommandAdd {
        command: CommandInfo,
    },
    CommandRemove {
        name: String,
    },
    /// Run slash command `name` with `text` in `channel` (thread `parent`).
    Command {
        channel: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<u64>,
        name: String,
        #[serde(default)]
        text: String,
    },
    /// The asker's scheduled messages, soonest first.
    Scheduled {
        items: Vec<ScheduledMsg>,
    },
    /// Owner or admin: a one-time code that signs someone in as `member`.
    CreateInvite {
        member: u64,
        #[serde(default)]
        role: Role,
    },
    Invite {
        member: u64,
        code: String,
        /// The address the code was emailed to.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        emailed: Option<String>,
        /// Why it couldn't be emailed (mail is set up but sending failed).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mail_error: Option<String>,
    },
    /// A short one-time code that signs another device (a phone) in as
    /// this person, shown as a QR code and as six letters and digits.
    /// Servers from before it ignore the message.
    CreateLinkCode,
    /// The answer to `create_link_code`: the code and how many seconds it
    /// works for.
    LinkCode {
        code: String,
        seconds: u64,
    },
    /// Set up this person's account, or change its password
    /// (`old_password` is needed once an account exists).
    SetPassword {
        email: String,
        password: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_password: Option<String>,
    },
    /// The account is set up (answer to `set_password`).
    Account {
        email: String,
    },
    SetRole {
        member: u64,
        role: Role,
    },
    /// Sign out every computer of this member and cancel their invite.
    Revoke {
        member: u64,
    },
    AccessList,
    /// Owner: the backups on the server, newest first.
    BackupList,
    Backups {
        backups: Vec<Backup>,
    },
    /// Owner: go back to a backup (deleted items come back, newer ones stay).
    Restore {
        name: String,
    },
    Restored {
        name: String,
    },
    /// Join the voice huddle of a channel or DM.
    CallJoin {
        channel: u64,
    },
    /// Where and how to join it (a LiveKit server address and room token).
    CallToken {
        channel: u64,
        url: String,
        token: String,
    },
    Access {
        people: Vec<Person>,
        /// The asker's own role (it may have just changed).
        role: Role,
        /// True when this answers the connection's own access_list, set_role
        /// or revoke; false when it was sent unasked after a change (another
        /// connection's request, a sign-in, people added). Older servers
        /// leave it out. A connection's own requests never send it an
        /// unasked list as well, so each request gets one answer.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        answer: Option<bool>,
    },
    /// The members with the app open somewhere (connected to the server),
    /// sent after welcome and whenever it changes. Older servers never send
    /// it; older apps ignore it.
    Presence {
        online: Vec<u64>,
    },
    /// Someone is typing in a conversation. The app sends it (without
    /// `member`) at most every few seconds while its person types; the
    /// server passes it on, with `member`, to the others who can see that
    /// conversation. Nothing is kept. Older servers and apps ignore it.
    /// `thread` is the message whose thread is being answered: that typing
    /// shows in the thread, not under the conversation's own box.
    Typing {
        channel: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        member: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread: Option<u64>,
    },
    /// This person's own reading state and notification choices
    /// (`wb_sync::mine`), kept in step between their devices. The app sends
    /// what it has whenever it changes; the server merges it into what it
    /// keeps for that person and sends the result to all their connections.
    /// It also sends it right after `welcome` (`{}` when it has nothing yet),
    /// which tells the app the server takes it: until then the app sends
    /// none. Never sent to anyone else. Older servers and apps ignore it.
    Mine {
        data: Value,
    },
    /// Sent after `welcome` when the server can ask an AI (its ai.json has
    /// a key): the app shows its AI tools. Servers without it never send it;
    /// older apps ignore it.
    AiReady {
        model: String,
    },
    /// Ask the AI. `id` is the app's own number for the request; the answer
    /// comes as `ai_text` pieces and one `ai_done` with the same id.
    /// `lang` is the language to answer in ("ko", "en").
    Ai {
        id: u64,
        ask: AiAsk,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        lang: String,
    },
    /// The answer to request `id` isn't wanted any more (the window asking
    /// closed): the server stops it and sends nothing more for it.
    AiStop {
        id: u64,
    },
    /// The next piece of an answer, to add to what came before.
    AiText {
        id: u64,
        text: String,
    },
    /// The answer is complete, or (`error`) it failed or was turned down.
    AiDone {
        id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Error {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        /// Set when a call_join for this conversation was turned down.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        channel: Option<u64>,
        /// Set when changes in a pushed copy were turned down and the team's
        /// version goes back to the app (code "denied", so older apps show it
        /// like any refusal).
        #[serde(default, skip_serializing_if = "is_false")]
        put_back: bool,
        /// Set (with code "auth", so older apps stop like for any refusal)
        /// when the workspace was deleted from the server.
        #[serde(default, skip_serializing_if = "is_false")]
        gone: bool,
    },
}

impl Msg {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn parse(s: &str) -> Option<Msg> {
        serde_json::from_str(s).ok()
    }

    pub fn error(message: &str, code: Option<&str>) -> Msg {
        Msg::Error {
            message: message.to_string(),
            code: code.map(|c| c.to_string()),
            channel: None,
            put_back: false,
            gone: false,
        }
    }

    /// A call_join for `channel` turned down.
    pub fn call_denied(message: &str, channel: u64) -> Msg {
        Msg::Error {
            message: message.to_string(),
            code: Some(CODE_DENIED.to_string()),
            channel: Some(channel),
            put_back: false,
            gone: false,
        }
    }

    /// Changes in a pushed copy turned down; the team's version follows.
    pub fn put_back(message: &str) -> Msg {
        Msg::Error {
            message: message.to_string(),
            code: Some(CODE_DENIED.to_string()),
            channel: None,
            put_back: true,
            gone: false,
        }
    }

    /// The workspace was deleted: the connection ends and the app stops
    /// trying (see `Status::Gone`).
    pub fn gone() -> Msg {
        Msg::Error {
            message: WORKSPACE_GONE.to_string(),
            code: Some(CODE_AUTH.to_string()),
            channel: None,
            put_back: false,
            gone: true,
        }
    }

    /// A request that waits for the connection while reconnecting (pushes
    /// don't: the newest copy goes after every welcome anyway).
    pub(crate) fn waits_for_connection(&self) -> bool {
        matches!(
            self,
            Msg::CreateInvite { .. }
                | Msg::SetPassword { .. }
                | Msg::SetRole { .. }
                | Msg::Revoke { .. }
                | Msg::AccessList
                | Msg::BackupList
                | Msg::Restore { .. }
                | Msg::CallJoin { .. }
        )
    }

    /// A request that does the same thing when the server gets it twice.
    /// The others (a restore, a new invite code that replaces the last one,
    /// a password change) aren't sent again by themselves after a send
    /// failed, since the first one may have reached the server anyway.
    pub(crate) fn safe_to_resend(&self) -> bool {
        self.waits_for_connection()
            && !matches!(
                self,
                Msg::CreateInvite { .. } | Msg::SetPassword { .. } | Msg::Restore { .. }
            )
    }
}
