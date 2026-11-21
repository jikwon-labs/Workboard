// Talking to workboard-server: the messages both sides send (JSON text over a
// WebSocket) and the app's side of the connection, which runs on its own
// thread so the window never waits for the network.
//
// Order of a connection: the app sends `hello` (with its device token, an
// invite code, an email and password, a password-reset code, or a claim on
// an empty server), the server answers `welcome` with the team's copy (and a
// new token when it issued one), then the app
// sends `push` with its shared copy whenever it changes and the server
// sends `update` to everyone when the team's copy changed. Both sides answer
// `ping` with `pong`. Owners and admins also send the access requests
// (`create_invite`, `set_role`, `revoke`, `access_list`). Anyone signed in
// may send `set_password` to set up or change their account.
//
// Deltas: when both sides say `deltas` (in hello and welcome), only what
// changed travels after the first whole copy. The app pushes the part of
// its copy that differs from what the server has (`wb_sync::delta`; merging
// it is the same as merging the whole copy, since a missing item never
// counts as deleted), and the server sends `delta` messages with the part
// of the team's copy that changed since the revision `base`. An app that
// missed a revision asks for the whole copy again (`resync`). Apps and
// servers from before deltas keep sending and getting whole copies.
//
// Accounts: an email and a password that sign a person in on any computer
// or phone. Someone joining with an invite code chooses the password then;
// the owner chooses one when setting up an empty server. A forgotten
// password is reset with a code the server emails (POST /auth/forgot).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::ErrorKind;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

mod conn;
mod files;
mod watch;
pub use conn::{trust_certificate, Conn};
pub use files::{
    create_workspace, delete_workspace, download, download_update, forgot_password,
    list_workspaces, name_ok, remote_size, sha256_file, update_text, upload, Auth, FileError,
};
pub use watch::{Watch, Watcher};

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

/// What a person typed into an invite code field: a bare code, or a
/// `workboard://join?...` link (also inside a whole pasted email).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JoinLink {
    pub server: Option<String>,
    pub code: String,
    pub email: Option<String>,
}

/// The link that opens Workboard ready to join.
pub fn join_link(server: &str, code: &str, email: Option<&str>) -> String {
    let mut s = format!(
        "workboard://join?server={}&code={}",
        pct_encode(server),
        pct_encode(code)
    );
    if let Some(e) = email.filter(|e| !e.is_empty()) {
        s.push_str("&email=");
        s.push_str(&pct_encode(e));
    }
    s
}

/// Read a pasted invite: the first workboard://join link in the text, else
/// the text itself as a code.
pub fn parse_join(text: &str) -> JoinLink {
    let t = text.trim();
    let Some(start) = t.find("workboard://join") else {
        return JoinLink {
            code: t.to_string(),
            ..Default::default()
        };
    };
    let link: String = t[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && !"<>\"'".contains(*c))
        .collect();
    let mut out = JoinLink::default();
    if let Some((_, query)) = link.split_once('?') {
        for pair in query.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let v = pct_decode(v);
            match k {
                "server" if !v.trim().is_empty() => out.server = Some(v.trim().to_string()),
                "code" => out.code = v.trim().to_string(),
                "email" if !v.trim().is_empty() => out.email = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    out
}

/// Percent-encode for a URL query value: everything but letters, digits
/// and `-._~` becomes %XX.
pub fn pct_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect()
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

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
/// entry or value in it (see `UPDATE_NEEDED`).
pub const OLDEST_SYNC: u32 = 4;

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
    fn waits_for_connection(&self) -> bool {
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
    fn safe_to_resend(&self) -> bool {
        self.waits_for_connection()
            && !matches!(
                self,
                Msg::CreateInvite { .. } | Msg::SetPassword { .. } | Msg::Restore { .. }
            )
    }
}

/// The web address of `path` ("/hooks/<token>") on the server (or the
/// server's workspace) the address a person typed points to.
pub fn http_address(addr: &str, path: &str) -> Option<String> {
    let (_, _, url) = server_url(addr).ok()?;
    let url = url
        .strip_prefix("wss://")
        .map(|r| format!("https://{}", r))
        .or_else(|| url.strip_prefix("ws://").map(|r| format!("http://{}", r)))?;
    let base = url.strip_suffix(PATH).unwrap_or(&url);
    Some(format!("{}{}", base, path))
}

/// Turn what a person typed ("macmini", "100.1.2.3", "host:9000",
/// "ws://host:8787/ws", "https://wb.example.com") into (host, port, url).
/// A wss:// or https:// address is a secure connection (port 443 unless
/// it says otherwise), for a server behind an HTTPS front such as
/// Cloudflare Tunnel; anything else is plain ws on port 8787 by default.
pub fn server_url(addr: &str) -> Result<(String, u16, String), String> {
    let a = addr.trim().trim_end_matches('/');
    if a.is_empty() {
        return Err("Type the server's address first.".into());
    }
    let lower = a.to_ascii_lowercase();
    let (secure, rest) = if lower.starts_with("wss://") {
        (true, &a[6..])
    } else if lower.starts_with("https://") {
        (true, &a[8..])
    } else if lower.starts_with("ws://") {
        (false, &a[5..])
    } else if lower.starts_with("http://") {
        (false, &a[7..])
    } else {
        (false, a)
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, PATH),
    };
    // a workspace other than the server's first: its connection is below it
    let path = match workspace_id(path) {
        Some(id) => format!("/w/{}{}", id, PATH),
        None => path.to_string(),
    };
    let (host, port) = if let Some(h) = hostport.strip_prefix('[') {
        // [ipv6]:port
        let end = h.find(']').ok_or("That address doesn't look right.")?;
        let port = h[end + 1..]
            .strip_prefix(':')
            .map(|p| p.parse::<u16>())
            .transpose()
            .map_err(|_| "That port isn't a number.")?;
        (h[..end].to_string(), port)
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (
                h.to_string(),
                Some(p.parse::<u16>().map_err(|_| "That port isn't a number.")?),
            ),
            _ => (hostport.to_string(), None),
        }
    };
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err("That address doesn't look right.".into());
    }
    let port = port.unwrap_or(if secure { 443 } else { DEFAULT_PORT });
    let base = origin(&host, port, secure);
    let scheme = if secure { "wss" } else { "ws" };
    Ok((host, port, format!("{}://{}{}", scheme, base, path)))
}

/// The workspace id in an address's path ("/w/<id>" or "/w/<id>/ws"); None
/// for the server's first workspace.
fn workspace_id(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/w/")?;
    let id = rest
        .strip_suffix(PATH)
        .unwrap_or(rest)
        .trim_end_matches('/');
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

/// What paths of a workspace's address start with on the server: "" for
/// the server's first workspace, "/w/<id>" for the others (see the
/// server's registry.rs).
pub fn workspace_prefix(addr: &str) -> String {
    let a = addr.trim().trim_end_matches('/');
    let rest = a.find("://").map(|i| &a[i + 3..]).unwrap_or(a);
    match rest.find('/').and_then(|i| workspace_id(&rest[i..])) {
        Some(id) => format!("/w/{}", id),
        None => String::new(),
    }
}

/// The server part of an address, as typed, without any path.
pub fn server_part(addr: &str) -> String {
    let a = addr.trim().trim_end_matches('/');
    let start = a.find("://").map(|i| i + 3).unwrap_or(0);
    match a[start..].find('/') {
        Some(i) => a[..start + i].to_string(),
        None => a.to_string(),
    }
}

/// The address of workspace `id` on the server `addr` is on ("" is the
/// server's first workspace).
pub fn workspace_addr(addr: &str, id: &str) -> String {
    let base = server_part(addr);
    if id.is_empty() {
        base
    } else {
        format!("{}/w/{}", base, id)
    }
}

/// Whether two addresses reach the same workspace ("macmini" and
/// "ws://macmini:8787/ws" do).
pub fn same_workspace(a: &str, b: &str) -> bool {
    match (server_url(a), server_url(b)) {
        (Ok(x), Ok(y)) => x.2.eq_ignore_ascii_case(&y.2),
        _ => false,
    }
}

/// host[:port] as it goes into a URL or a Host header: IPv6 in brackets,
/// the port left out when it is the secure default.
fn origin(host: &str, port: u16, secure: bool) -> String {
    let shown = if host.contains(':') {
        format!("[{}]", host)
    } else {
        host.to_string()
    };
    if secure && port == 443 {
        shown
    } else {
        format!("{}:{}", shown, port)
    }
}

/// Whether a server_url() url is a secure (TLS) one.
pub fn is_secure(url: &str) -> bool {
    url.starts_with("wss://")
}

/// Where the server publishes app updates for Sparkle (see the server's
/// /updates route): the same host and port as the sync connection, over
/// https when that is secure.
pub fn updates_url(addr: &str) -> Result<String, String> {
    let (host, port, url) = server_url(addr)?;
    let secure = is_secure(&url);
    Ok(format!(
        "{}://{}/updates/appcast.xml",
        if secure { "https" } else { "http" },
        origin(&host, port, secure)
    ))
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Connecting,
    Connected,
    /// Lost or couldn't reach the server; trying again. Holds why.
    Reconnecting(String),
    /// The server won't let this computer in (no or bad invite, access
    /// removed). Not trying again.
    Refused(String),
    /// The workspace was deleted from the server. Not trying again.
    Gone(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Status(Status),
    Welcome {
        tag: u16,
        snapshot: Option<Value>,
        rev: u64,
        member: u64,
        role: Role,
        token: Option<String>,
        email: Option<String>,
        hidden: Vec<u64>,
    },
    Update {
        data: Value,
        rev: u64,
        hidden: Vec<u64>,
    },
    Invite {
        member: u64,
        code: String,
        emailed: Option<String>,
        mail_error: Option<String>,
    },
    /// A code that signs a phone in as this person (`Msg::LinkCode`).
    LinkCode {
        code: String,
        seconds: u64,
    },
    /// The account is set up or its password changed.
    Account(String),
    Access {
        people: Vec<Person>,
        role: Role,
    },
    /// A request was turned down; the connection stays.
    Denied(String),
    /// Changes in the copy this app sent were turned down (a space shared
    /// with it at a level that doesn't allow them); the team's version of
    /// them comes back with the next update.
    PutBack(String),
    /// The huddle of this conversation was turned down (not in it, voice
    /// not set up...); the connection stays.
    CallDenied {
        channel: u64,
        why: String,
    },
    CallToken {
        channel: u64,
        url: String,
        token: String,
    },
    Backups(Vec<Backup>),
    Restored(String),
    /// The members connected to the server right now.
    Presence(Vec<u64>),
    /// `member` is typing in `channel`, in the thread of message `thread`
    /// when there is one (see `Msg::Typing`).
    Typing {
        channel: u64,
        member: u64,
        thread: Option<u64>,
    },
    /// My scheduled messages (the answer to schedule, unschedule and
    /// scheduled_list).
    Scheduled(Vec<ScheduledMsg>),
    /// My reading state and notification choices as the server keeps them
    /// (`Msg::Mine`).
    Mine(Value),
    /// The webhooks (owners and admins) and slash commands.
    Integrations {
        hooks: Vec<HookInfo>,
        commands: Vec<CommandInfo>,
    },
    /// The server can ask an AI, with this model (`Msg::AiReady`).
    AiReady(String),
    /// More of the answer to AI request `id`.
    AiText {
        id: u64,
        text: String,
    },
    /// AI request `id` is answered, or failed with `error`.
    AiDone {
        id: u64,
        error: Option<String>,
    },
}

/// How this computer signs in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignIn {
    pub device: String,
    pub tag: Option<u16>,
    pub token: Option<String>,
    pub invite: Option<String>,
    pub claim: Option<u64>,
    pub email: Option<String>,
    pub password: Option<String>,
    pub new_password: Option<String>,
    pub reset_code: Option<String>,
    /// A code another device of the same person showed (`Msg::Hello::link_code`).
    pub link_code: Option<String>,
    /// The app's version and sync format, sent in hello.
    pub app: Option<String>,
    pub sync: Option<u32>,
    /// Ask for whole copies every time, like apps from before deltas (tests).
    pub full_copies: bool,
    /// Only notifications (see Msg::Hello::watch).
    pub watch: bool,
}

/// The hello a computer signs in with.
impl From<&SignIn> for Msg {
    fn from(s: &SignIn) -> Msg {
        Msg::Hello {
            device: s.device.clone(),
            tag: s.tag,
            token: s.token.clone(),
            invite: s.invite.clone(),
            claim: s.claim,
            email: s.email.clone(),
            password: s.password.clone(),
            new_password: s.new_password.clone(),
            reset_code: s.reset_code.clone(),
            link_code: s.link_code.clone(),
            app: s.app.clone(),
            sync: s.sync,
            deltas: !s.full_copies,
            watch: s.watch,
        }
    }
}

/// How a hello signs in (the server's side). Any other message comes back
/// as the error.
impl TryFrom<Msg> for SignIn {
    type Error = Msg;

    fn try_from(m: Msg) -> Result<SignIn, Msg> {
        match m {
            Msg::Hello {
                device,
                tag,
                token,
                invite,
                claim,
                email,
                password,
                new_password,
                reset_code,
                link_code,
                app,
                sync,
                deltas,
                watch,
            } => Ok(SignIn {
                device,
                tag,
                token,
                invite,
                claim,
                email,
                password,
                new_password,
                reset_code,
                link_code,
                app,
                sync,
                full_copies: !deltas,
                watch,
            }),
            m => Err(m),
        }
    }
}

impl SignIn {
    /// After a welcome: sign in with the token next time, not with the
    /// password or a code that is used up now.
    pub fn forget_secrets(&mut self) {
        self.invite = None;
        self.claim = None;
        self.email = None;
        self.password = None;
        self.new_password = None;
        self.reset_code = None;
        self.link_code = None;
    }
}

enum Cmd {
    Push(Value),
    Send(Msg),
    Stop,
}

/// The app's connection. Dropping it closes the connection and ends the thread.
pub struct Client {
    tx: Sender<Cmd>,
    rx: Receiver<Event>,
}

pub type Wake = Arc<dyn Fn() + Send + Sync>;

impl Client {
    /// Start connecting in the background. `wake` is called after each event
    /// so the window can redraw. Reconnects by itself until dropped or refused.
    pub fn start(addr: &str, sign_in: SignIn, wake: Wake) -> Client {
        let (tx, crx) = mpsc::channel();
        let (etx, rx) = mpsc::channel();
        let addr = addr.to_string();
        std::thread::Builder::new()
            .name("workboard-net".into())
            .spawn(move || run(addr, sign_in, crx, etx, wake))
            .expect("start network thread");
        Client { tx, rx }
    }

    /// Send our whole shared copy (sent once connected if not yet).
    pub fn push(&self, data: Value) {
        let _ = self.tx.send(Cmd::Push(data));
    }

    /// Send a request (access requests, call_join). While (re)connecting it
    /// waits and goes once the server lets this computer in.
    pub fn send(&self, msg: Msg) {
        let _ = self.tx.send(Cmd::Send(msg));
    }

    /// Everything that happened since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Stop);
    }
}

struct Line {
    tx: Sender<Event>,
    wake: Wake,
}

impl Line {
    fn emit(&self, e: Event) -> bool {
        let ok = self.tx.send(e).is_ok();
        (self.wake)();
        ok
    }
}

fn connect(addr: &str) -> Result<WebSocket<Conn>, End> {
    let (host, port, url) = server_url(addr).map_err(End::Lost)?;
    let stream =
        conn::dial(&host, port, is_secure(&url), Duration::from_secs(5)).map_err(End::Lost)?;
    stream
        .tcp()
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| End::Lost(e.to_string()))?;
    let cfg = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let (ws, _) = tungstenite::client::client_with_config(url.as_str(), stream, Some(cfg))
        .map_err(|e| match e {
            tungstenite::HandshakeError::Failure(e) => e,
            tungstenite::HandshakeError::Interrupted(_) => {
                tungstenite::Error::Io(ErrorKind::TimedOut.into())
            }
        })
        .map_err(|e| match e {
            // the workspace was deleted (410 Gone): stop trying
            tungstenite::Error::Http(r) if r.status().as_u16() == 410 => {
                End::Gone(WORKSPACE_GONE.to_string())
            }
            // rustls reports a certificate it doesn't trust as InvalidData
            tungstenite::Error::Io(e) if e.kind() == ErrorKind::InvalidData => {
                End::Lost(format!("The secure connection to {} failed: {}", host, e))
            }
            tungstenite::Error::Io(e) => End::Lost(format!("Can't reach {}:{}: {}", host, port, e)),
            e => End::Lost(format!("The server didn't answer like Workboard: {}", e)),
        })?;
    ws.get_ref()
        .tcp()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| End::Lost(e.to_string()))?;
    Ok(ws)
}

/// Said when the connection broke while sending a request that isn't sent
/// twice by itself (see Msg::safe_to_resend).
pub const UNSURE_SEND: &str = "The connection broke while sending that, so it may not have reached the server. Check whether it happened, then try again.";

/// Requests kept while not connected; older ones beyond this are dropped.
const MAX_QUEUED: usize = 32;

/// Keep a request for after the next welcome.
fn queue(queued: &mut Vec<Msg>, m: Msg) {
    if m.waits_for_connection() {
        queued.push(m);
        if queued.len() > MAX_QUEUED {
            queued.remove(0);
        }
    }
}

/// Sending `m` failed, though it may have reached the server anyway: try it
/// again after reconnecting when doing it twice doesn't matter, else say so.
fn send_failed(m: Msg, queued: &mut Vec<Msg>) -> Option<Event> {
    if m.safe_to_resend() {
        queued.insert(0, m);
        None
    } else {
        m.waits_for_connection()
            .then(|| Event::Denied(UNSURE_SEND.into()))
    }
}

/// Wait before trying again; false when the app asked to stop.
fn wait(
    crx: &Receiver<Cmd>,
    pending: &mut Option<Value>,
    queued: &mut Vec<Msg>,
    secs: u64,
) -> bool {
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        match crx.recv_timeout(Duration::from_millis(100)) {
            Ok(Cmd::Push(v)) => *pending = Some(v),
            Ok(Cmd::Send(m)) => queue(queued, m),
            Ok(Cmd::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return false,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    true
}

enum End {
    Stop,
    Lost(String),
    Refused(String),
    Gone(String),
}

fn run(addr: String, mut sign_in: SignIn, crx: Receiver<Cmd>, etx: Sender<Event>, wake: Wake) {
    let line = Line { tx: etx, wake };
    let mut pending: Option<Value> = None;
    let mut queued: Vec<Msg> = vec![];
    let mut tries: u64 = 0;
    line.emit(Event::Status(Status::Connecting));
    loop {
        let end = match connect(&addr) {
            Ok(mut ws) => {
                let end = session(
                    &mut ws,
                    &line,
                    &crx,
                    &mut sign_in,
                    &mut pending,
                    &mut queued,
                    &mut tries,
                );
                let _ = ws.close(None);
                let _ = ws.flush();
                end
            }
            Err(e) => e,
        };
        let gave_up = match end {
            End::Stop => return,
            End::Refused(why) => Some(Status::Refused(why)),
            End::Gone(why) => Some(Status::Gone(why)),
            End::Lost(why) => {
                if !line.emit(Event::Status(Status::Reconnecting(why))) {
                    return;
                }
                None
            }
        };
        if let Some(status) = gave_up {
            line.emit(Event::Status(status));
            // stay until the app lets go, so it can read the status
            while let Ok(c) = crx.recv() {
                if matches!(c, Cmd::Stop) {
                    return;
                }
            }
            return;
        }
        // 1, 2, 3, 4, 5, 5... seconds
        tries += 1;
        if !wait(&crx, &mut pending, &mut queued, tries.min(5)) {
            return;
        }
    }
}

fn session(
    ws: &mut WebSocket<Conn>,
    line: &Line,
    crx: &Receiver<Cmd>,
    sign_in: &mut SignIn,
    pending: &mut Option<Value>,
    queued: &mut Vec<Msg>,
    tries: &mut u64,
) -> End {
    let hello = Msg::from(&*sign_in);
    if let Err(e) = send(ws, &hello) {
        return End::Lost(e);
    }
    let mut st = Synced::default();
    let mut last_heard = Instant::now();
    let mut last_ping = Instant::now();
    // the app let go: what it pushed or sent just before still goes, then the
    // connection closes (logging out or quitting right after a change)
    let mut stopping = false;
    loop {
        loop {
            match crx.try_recv() {
                Ok(Cmd::Push(v)) => *pending = Some(v),
                Ok(Cmd::Send(m)) if st.welcomed => queued.push(m),
                Ok(Cmd::Send(m)) => queue(queued, m),
                Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => {
                    stopping = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if stopping && !st.welcomed {
            return End::Stop;
        }
        if st.welcomed {
            // the copy first, so a conversation made a moment ago is on the
            // server before a call_join for it
            if let Some(v) = pending.take() {
                let push = match st.summary.as_mut() {
                    // only what the server doesn't have yet (nothing: no push)
                    Some(s) => wb_sync::delta(s, &v).inspect(|d| s.absorb(d)),
                    None => {
                        if st.deltas {
                            st.summary = Some(wb_sync::Summary::of(&v));
                        }
                        Some(v)
                    }
                };
                if let Some(data) = push {
                    if let Err(e) = send(ws, &Msg::Push { data }) {
                        return End::Lost(e);
                    }
                }
            }
            while !queued.is_empty() {
                let m = queued.remove(0);
                if let Err(e) = send(ws, &m) {
                    if let Some(ev) = send_failed(m, queued) {
                        line.emit(ev);
                    }
                    return End::Lost(e);
                }
            }
        }
        if stopping {
            return End::Stop;
        }
        if last_ping.elapsed() >= Duration::from_secs(PING_SECS) {
            last_ping = Instant::now();
            if let Err(e) = send(ws, &Msg::Ping) {
                return End::Lost(e);
            }
        }
        if last_heard.elapsed() >= Duration::from_secs(SILENT_SECS) {
            return End::Lost("The server stopped answering.".into());
        }
        let res: Result<(), End> = match ws.read() {
            Ok(Message::Text(t)) => {
                last_heard = Instant::now();
                on_server_msg(Msg::parse(t.as_str()), ws, line, sign_in, tries, &mut st)
            }
            Ok(Message::Close(_)) => Err(End::Lost("The server closed the connection.".into())),
            Ok(_) => {
                last_heard = Instant::now();
                Ok(())
            }
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                Ok(())
            }
            Err(e) => Err(End::Lost(e.to_string())),
        };
        if let Err(end) = res {
            return end;
        }
    }
}

/// Send a message to the server.
fn send(ws: &mut WebSocket<Conn>, m: &Msg) -> Result<(), String> {
    ws.send(Message::text(m.to_json()))
        .map_err(|e| e.to_string())
}

/// What a session knows of the server.
#[derive(Default)]
struct Synced {
    welcomed: bool,
    /// what the server has of our copy, when it takes deltas (else None)
    summary: Option<wb_sync::Summary>,
    deltas: bool,
    rev: u64,
}

/// Act on a message from the server: tell the app, answer it, or end the
/// session.
fn on_server_msg(
    m: Option<Msg>,
    ws: &mut WebSocket<Conn>,
    line: &Line,
    sign_in: &mut SignIn,
    tries: &mut u64,
    st: &mut Synced,
) -> Result<(), End> {
    let emit = |e: Event| -> Result<(), End> {
        if line.emit(e) {
            Ok(())
        } else {
            Err(End::Stop)
        }
    };
    match m {
        Some(Msg::Welcome {
            tag,
            snapshot,
            rev: r,
            member,
            role,
            token,
            email,
            hidden,
            deltas: server_deltas,
        }) => {
            st.deltas = server_deltas && !sign_in.full_copies;
            st.rev = r;
            st.summary = if st.deltas {
                snapshot.as_ref().map(wb_sync::Summary::of)
            } else {
                None
            };
            // sign in the same way next time: with the token, not the password
            sign_in.tag = Some(tag);
            if let Some(t) = &token {
                sign_in.token = Some(t.clone());
            }
            sign_in.forget_secrets();
            st.welcomed = true;
            *tries = 0;
            emit(Event::Status(Status::Connected)).and_then(|_| {
                emit(Event::Welcome {
                    tag,
                    snapshot,
                    rev: r,
                    member,
                    role,
                    token,
                    email,
                    hidden,
                })
            })
        }
        Some(Msg::Update {
            data,
            rev: r,
            hidden,
        }) => {
            if st.deltas {
                st.summary = Some(wb_sync::Summary::of(&data));
            }
            st.rev = r;
            emit(Event::Update {
                data,
                rev: r,
                hidden,
            })
        }
        Some(Msg::Delta {
            data,
            rev: r,
            base,
            hidden,
        }) => {
            // a revision went missing: ask for the whole copy
            // (this part still merges fine meanwhile)
            let asked = if base > st.rev {
                send(ws, &Msg::Resync).map_err(End::Lost)
            } else {
                Ok(())
            };
            if let Some(s) = st.summary.as_mut() {
                s.absorb(&data);
            }
            st.rev = st.rev.max(r);
            asked.and_then(|_| {
                emit(Event::Update {
                    data,
                    rev: r,
                    hidden,
                })
            })
        }
        Some(Msg::Invite {
            member,
            code,
            emailed,
            mail_error,
        }) => emit(Event::Invite {
            member,
            code,
            emailed,
            mail_error,
        }),
        Some(Msg::LinkCode { code, seconds }) => emit(Event::LinkCode { code, seconds }),
        Some(Msg::Account { email }) => emit(Event::Account(email)),
        Some(Msg::Access { people, role, .. }) => emit(Event::Access { people, role }),
        Some(Msg::CallToken {
            channel,
            url,
            token,
        }) => emit(Event::CallToken {
            channel,
            url,
            token,
        }),
        Some(Msg::Backups { backups }) => emit(Event::Backups(backups)),
        Some(Msg::Restored { name }) => emit(Event::Restored(name)),
        Some(Msg::Presence { online }) => emit(Event::Presence(online)),
        Some(Msg::Scheduled { items }) => emit(Event::Scheduled(items)),
        Some(Msg::Mine { data }) => emit(Event::Mine(data)),
        Some(Msg::IntegrationList { hooks, commands }) => {
            emit(Event::Integrations { hooks, commands })
        }
        Some(Msg::AiReady { model }) => emit(Event::AiReady(model)),
        Some(Msg::AiText { id, text }) => emit(Event::AiText { id, text }),
        Some(Msg::AiDone { id, error }) => emit(Event::AiDone { id, error }),
        Some(Msg::Typing {
            channel,
            member: Some(member),
            thread,
        }) => emit(Event::Typing {
            channel,
            member,
            thread,
        }),
        Some(Msg::Ping) => send(ws, &Msg::Pong).map_err(End::Lost),
        Some(Msg::Error {
            message,
            code,
            channel,
            put_back,
            gone,
        }) => match code.as_deref() {
            Some(CODE_AUTH) if gone => Err(End::Gone(message)),
            Some(CODE_AUTH) => Err(End::Refused(message)),
            Some(CODE_DENIED) if put_back => emit(Event::PutBack(message)),
            Some(CODE_DENIED) => match channel {
                Some(channel) => emit(Event::CallDenied {
                    channel,
                    why: message,
                }),
                None => emit(Event::Denied(message)),
            },
            _ => Err(End::Lost(message)),
        },
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AI requests and answers on the wire: the kind of request rides in
    /// `kind`, empty extras stay out, and the answer's pieces and end carry
    /// the app's own request number.
    #[test]
    fn ai_messages_on_the_wire() {
        let ask = Msg::Ai {
            id: 7,
            ask: AiAsk::SummarizeThread {
                channel: 3,
                message: 9,
            },
            lang: "ko".into(),
        };
        let v: Value = serde_json::from_str(&ask.to_json()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"type": "ai", "id": 7, "lang": "ko",
                "ask": {"kind": "summarize_thread", "channel": 3, "message": 9}})
        );
        assert_eq!(Msg::parse(&ask.to_json()), Some(ask));
        let q = Msg::parse(r#"{"type":"ai","id":1,"ask":{"kind":"question","question":"Who?"}}"#);
        assert_eq!(
            q,
            Some(Msg::Ai {
                id: 1,
                ask: AiAsk::Question {
                    question: "Who?".into(),
                    history: vec![]
                },
                lang: String::new()
            })
        );
        let w = Msg::Ai {
            id: 2,
            ask: AiAsk::Write {
                how: "fix".into(),
                text: "teh".into(),
                title: String::new(),
            },
            lang: String::new(),
        };
        assert!(!w.to_json().contains("title") && !w.to_json().contains("lang"));
        assert_eq!(
            Msg::AiDone { id: 2, error: None }.to_json(),
            r#"{"type":"ai_done","id":2}"#
        );
        assert_eq!(
            Msg::AiText {
                id: 2,
                text: "the".into()
            }
            .to_json(),
            r#"{"type":"ai_text","id":2,"text":"the"}"#
        );
        assert_eq!(
            Msg::parse(r#"{"type":"ai_ready","model":"m"}"#),
            Some(Msg::AiReady { model: "m".into() })
        );
    }

    #[test]
    fn web_addresses_on_the_server() {
        assert_eq!(
            http_address("https://wb.example.com/w/ab12", "/hooks/t1").as_deref(),
            Some("https://wb.example.com/w/ab12/hooks/t1")
        );
        assert_eq!(
            http_address("macmini", "/hooks/t1").as_deref(),
            Some("http://macmini:8787/hooks/t1")
        );
    }

    #[test]
    fn a_workspace_below_the_server_has_its_own_paths() {
        assert_eq!(
            server_url("macmini/w/ab12").unwrap().2,
            "ws://macmini:8787/w/ab12/ws"
        );
        assert_eq!(
            server_url("https://wb.example.com/w/ab12/").unwrap().2,
            "wss://wb.example.com/w/ab12/ws"
        );
        assert_eq!(
            server_url("ws://h:1/w/ab12/ws").unwrap().2,
            "ws://h:1/w/ab12/ws"
        );
        // the first workspace keeps the addresses it always had
        assert_eq!(server_url("http://h:1/ws").unwrap().2, "ws://h:1/ws");
        assert_eq!(workspace_prefix("https://wb.example.com"), "");
        assert_eq!(workspace_prefix("ws://h:1/ws"), "");
        assert_eq!(workspace_prefix("https://wb.example.com/w/ab12"), "/w/ab12");
        assert_eq!(workspace_prefix("macmini/w/ab12/ws"), "/w/ab12");
        assert_eq!(
            workspace_addr("https://wb.example.com/w/ab12", "cd34"),
            "https://wb.example.com/w/cd34"
        );
        assert_eq!(workspace_addr("macmini:9000/w/ab12", ""), "macmini:9000");
        assert_eq!(
            server_part("https://wb.example.com/w/ab12"),
            "https://wb.example.com"
        );
        // app updates come from the server, whichever workspace
        assert_eq!(
            updates_url("https://wb.example.com/w/ab12").unwrap(),
            "https://wb.example.com/updates/appcast.xml"
        );
        assert!(same_workspace("macmini", "ws://macmini:8787/ws"));
        assert!(same_workspace(
            "macmini/w/ab12",
            "ws://MACMINI:8787/w/ab12/ws"
        ));
        assert!(!same_workspace("macmini", "macmini/w/ab12"));
        assert!(!same_workspace("macmini/w/ab12", "macmini/w/cd34"));
    }

    #[test]
    fn addresses_people_type_become_urls() {
        assert_eq!(server_url("macmini").unwrap().2, "ws://macmini:8787/ws");
        assert_eq!(
            updates_url("macmini").unwrap(),
            "http://macmini:8787/updates/appcast.xml"
        );
        assert_eq!(
            updates_url("ws://mac-mini.tail1.ts.net:9000/ws").unwrap(),
            "http://mac-mini.tail1.ts.net:9000/updates/appcast.xml"
        );
        assert_eq!(
            updates_url("[::1]:8787").unwrap(),
            "http://[::1]:8787/updates/appcast.xml"
        );
        assert!(updates_url("").is_err());
        assert_eq!(
            server_url(" 100.1.2.3:9000 ").unwrap(),
            ("100.1.2.3".into(), 9000, "ws://100.1.2.3:9000/ws".into())
        );
        assert_eq!(
            server_url("ws://mac-mini.tail1.ts.net/").unwrap().2,
            "ws://mac-mini.tail1.ts.net:8787/ws"
        );
        assert_eq!(server_url("http://h:1/ws").unwrap().2, "ws://h:1/ws");
        assert_eq!(server_url("[::1]:8787").unwrap().2, "ws://[::1]:8787/ws");
        assert!(server_url("").is_err());
        assert!(server_url("h:port").is_err() && server_url("https://h:port").is_err());

        // behind an HTTPS front (Cloudflare Tunnel): TLS on 443 unless said
        assert_eq!(
            server_url("https://wb.example.com").unwrap(),
            (
                "wb.example.com".into(),
                443,
                "wss://wb.example.com/ws".into()
            )
        );
        assert_eq!(
            server_url("WSS://wb.example.com/").unwrap().2,
            "wss://wb.example.com/ws"
        );
        assert_eq!(
            server_url("wss://wb.example.com:8443/ws").unwrap(),
            (
                "wb.example.com".into(),
                8443,
                "wss://wb.example.com:8443/ws".into()
            )
        );
        assert!(is_secure(&server_url("https://wb.example.com").unwrap().2));
        assert!(!is_secure(&server_url("macmini").unwrap().2));
        assert_eq!(
            updates_url("https://wb.example.com").unwrap(),
            "https://wb.example.com/updates/appcast.xml"
        );
        assert_eq!(
            updates_url("wss://wb.example.com:8443").unwrap(),
            "https://wb.example.com:8443/updates/appcast.xml"
        );
        assert!(server_url("h:port").is_err());
    }

    #[test]
    fn a_call_identity_names_its_member() {
        assert_eq!(call_identity(42, "dev-1"), "42-dev-1");
        assert_eq!(identity_member(&call_identity(42, "dev-1")), Some(42));
        assert_eq!(identity_member("someone"), None);
    }

    #[test]
    fn a_sign_in_becomes_its_hello_and_back_byte_for_byte() {
        let full = SignIn {
            device: "d1".into(),
            tag: Some(3),
            token: Some("t".into()),
            invite: Some("ABCD-EFGH-JKLM".into()),
            claim: Some(7),
            email: Some("a@b.co".into()),
            password: Some("pw".into()),
            new_password: Some("new pw".into()),
            reset_code: Some("R".into()),
            link_code: Some("K7Q2MX".into()),
            app: Some("0.1.7".into()),
            sync: Some(4),
            full_copies: false,
            watch: false,
        };
        let by_hand = Msg::Hello {
            device: "d1".into(),
            tag: Some(3),
            token: Some("t".into()),
            invite: Some("ABCD-EFGH-JKLM".into()),
            claim: Some(7),
            email: Some("a@b.co".into()),
            password: Some("pw".into()),
            new_password: Some("new pw".into()),
            reset_code: Some("R".into()),
            link_code: Some("K7Q2MX".into()),
            app: Some("0.1.7".into()),
            sync: Some(4),
            deltas: true,
            watch: false,
        };
        assert_eq!(Msg::from(&full).to_json(), by_hand.to_json());
        assert_eq!(
            Msg::from(&full).to_json(),
            r#"{"type":"hello","device":"d1","tag":3,"token":"t","invite":"ABCD-EFGH-JKLM","claim":7,"email":"a@b.co","password":"pw","new_password":"new pw","reset_code":"R","link_code":"K7Q2MX","app":"0.1.7","sync":4,"deltas":true}"#
        );
        let back = SignIn::try_from(Msg::parse(&by_hand.to_json()).unwrap()).unwrap();
        assert_eq!(back, full);
        // whole copies (no deltas) and nothing else: only the device goes
        let bare = SignIn {
            device: "d2".into(),
            full_copies: true,
            ..Default::default()
        };
        assert_eq!(
            Msg::from(&bare).to_json(),
            r#"{"type":"hello","device":"d2"}"#
        );
        assert_eq!(
            SignIn::try_from(Msg::parse(r#"{"type":"hello","device":"d2"}"#).unwrap()),
            Ok(bare)
        );
        assert_eq!(SignIn::try_from(Msg::Ping), Err(Msg::Ping));
    }

    #[test]
    fn messages_round_trip_as_tagged_json() {
        let hello = |tag| Msg::Hello {
            device: "d1".into(),
            tag,
            token: None,
            invite: None,
            claim: None,
            email: None,
            password: None,
            new_password: None,
            reset_code: None,
            link_code: None,
            app: None,
            sync: None,
            deltas: false,
            watch: false,
        };
        assert_eq!(
            hello(Some(3)).to_json(),
            r#"{"type":"hello","device":"d1","tag":3}"#
        );
        // the app's version goes along; an older server ignores it
        let mut h = hello(None);
        if let Msg::Hello { app, sync, .. } = &mut h {
            *app = Some("0.1.7".into());
            *sync = Some(3);
        }
        assert_eq!(
            h.to_json(),
            r#"{"type":"hello","device":"d1","app":"0.1.7","sync":3}"#
        );
        assert_eq!(
            Msg::parse(r#"{"type":"hello","device":"d1"}"#),
            Some(hello(None))
        );
        // an older server's welcome (no email) still reads
        assert!(matches!(
            Msg::parse(
                r#"{"type":"welcome","tag":1,"snapshot":null,"rev":0,"member":2,"role":"member"}"#
            ),
            Some(Msg::Welcome { email: None, .. })
        ));
        assert_eq!(Msg::parse(r#"{"type":"ping"}"#), Some(Msg::Ping));
        assert_eq!(
            Msg::parse(r#"{"type":"set_role","member":5,"role":"admin"}"#),
            Some(Msg::SetRole {
                member: 5,
                role: Role::Admin
            })
        );
        // what the iPhone app sends (ios/WorkboardKit Protocol.swift)
        let t = Msg::PushToken {
            token: "ab12".into(),
            platform: "ios".into(),
            env: "development".into(),
            workspace: None,
            prefs: None,
        };
        assert_eq!(
            Msg::parse(
                r#"{"type":"push_token","token":"ab12","platform":"ios","env":"development"}"#
            ),
            Some(t.clone())
        );
        assert_eq!(Msg::parse(&t.to_json()), Some(t));
        // with the phone's name for the workspace (Protocol.swift sends it too)
        assert_eq!(
            Msg::parse(
                r#"{"type":"push_token","token":"ab12","platform":"ios","env":"production","workspace":"k1"}"#
            ),
            Some(Msg::PushToken {
                token: "ab12".into(),
                platform: "ios".into(),
                env: "production".into(),
                workspace: Some("k1".into()),
                prefs: None,
            })
        );
        // a watch connection: the hello says so, the server answers watching and notes
        let w = SignIn {
            device: "d3".into(),
            token: Some("t".into()),
            watch: true,
            full_copies: true,
            ..Default::default()
        };
        assert_eq!(
            Msg::from(&w).to_json(),
            r#"{"type":"hello","device":"d3","token":"t","watch":true}"#
        );
        assert_eq!(Msg::parse(r#"{"type":"watching"}"#), Some(Msg::Watching));
        assert_eq!(
            Msg::parse(r#"{"type":"note","title":"Ann","body":"hi","channel":20,"parent":11}"#),
            Some(Msg::Note {
                title: "Ann".into(),
                body: "hi".into(),
                channel: Some(20),
                parent: Some(11),
            })
        );
        // with the device's choices; missing ones take their defaults
        let Some(Msg::PushToken { prefs: Some(p), .. }) = Msg::parse(
            r#"{"type":"push_token","token":"ab12","prefs":{"channel_modes":[{"channel":7,"mode":"all"}],"keywords":["deploy"],"muted_until":5}}"#,
        ) else {
            panic!("prefs read")
        };
        assert!(p.direct && p.mentions && p.replies);
        assert_eq!(p.channel_mode(7), "all");
        assert_eq!(p.channel_mode(8), "mentions");
        assert_eq!(p.keywords, vec!["deploy"]);
        assert_eq!(p.muted_until, 5);
    }

    #[test]
    fn invite_links_round_trip_and_read_from_pasted_mail() {
        let link = join_link(
            "mac-mini.tail1.ts.net:8787",
            "ABCD-EFGH-JKMN",
            Some("kim.minji+wb@company.com"),
        );
        assert_eq!(link, "workboard://join?server=mac-mini.tail1.ts.net%3A8787&code=ABCD-EFGH-JKMN&email=kim.minji%2Bwb%40company.com");
        let want = JoinLink {
            server: Some("mac-mini.tail1.ts.net:8787".into()),
            code: "ABCD-EFGH-JKMN".into(),
            email: Some("kim.minji+wb@company.com".into()),
        };
        assert_eq!(parse_join(&link), want);
        let mail = format!("Hi,\n\nOpen this: <{}>\nThanks", link);
        assert_eq!(parse_join(&mail), want);
        assert_eq!(
            parse_join("  abcd-efgh-jkmn "),
            JoinLink {
                code: "abcd-efgh-jkmn".into(),
                ..Default::default()
            }
        );
        assert_eq!(
            parse_join("workboard://join?code=X%2"),
            JoinLink {
                code: "X%2".into(),
                ..Default::default()
            }
        );
    }

    #[test]
    fn backup_names_read_as_dates() {
        let b = |n: &str| {
            Backup {
                name: n.into(),
                bytes: 0,
            }
            .label()
        };
        assert_eq!(b("hourly-20261115-03.json"), "2026-11-15 03:00 (hourly)");
        assert_eq!(b("daily-20261115.json"), "2026-11-15 (daily)");
        assert_eq!(
            b("before-restore-20261115-031500.json"),
            "2026-11-15 03:15:00 (before a restore)"
        );
        assert_eq!(b("other.json"), "other.json");
    }

    #[test]
    fn roles_order_from_member_to_owner() {
        assert!(Role::Owner.manages() && Role::Admin.manages() && !Role::Member.manages());
        assert!(Role::Owner > Role::Admin && Role::Admin > Role::Member);
    }

    #[test]
    fn a_put_back_refusal_reads_as_denied_to_older_apps() {
        let text = Msg::put_back("no").to_json();
        assert_eq!(
            text,
            r#"{"type":"error","message":"no","code":"denied","put_back":true}"#
        );
        // an app from before reads the same text as a plain refusal
        let old: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(old["code"], CODE_DENIED);
        assert_eq!(Msg::parse(&text), Some(Msg::put_back("no")));
    }

    #[test]
    fn a_call_refusal_names_its_conversation_and_old_errors_still_read() {
        assert_eq!(
            Msg::call_denied("no", 5).to_json(),
            r#"{"type":"error","message":"no","code":"denied","channel":5}"#
        );
        assert_eq!(
            Msg::parse(r#"{"type":"error","message":"no","code":"denied"}"#),
            Some(Msg::error("no", Some(CODE_DENIED)))
        );
        assert_eq!(
            Msg::error("x", None).to_json(),
            r#"{"type":"error","message":"x"}"#
        );
    }

    #[test]
    fn requests_wait_while_reconnecting_but_pings_and_pushes_do_not() {
        let mut q = vec![];
        queue(&mut q, Msg::CallJoin { channel: 1 });
        queue(&mut q, Msg::Ping);
        queue(&mut q, Msg::Push { data: Value::Null });
        queue(&mut q, Msg::BackupList);
        assert_eq!(q, vec![Msg::CallJoin { channel: 1 }, Msg::BackupList]);
        for _ in 0..MAX_QUEUED {
            queue(&mut q, Msg::AccessList);
        }
        assert_eq!(q.len(), MAX_QUEUED);
        assert_eq!(q[0], Msg::AccessList, "the oldest went first");
    }

    #[test]
    fn a_failed_send_is_repeated_only_when_twice_does_no_harm() {
        let mut q = vec![Msg::AccessList];
        for m in [
            Msg::Revoke { member: 3 },
            Msg::SetRole {
                member: 3,
                role: Role::Admin,
            },
            Msg::CallJoin { channel: 1 },
            Msg::BackupList,
        ] {
            assert_eq!(send_failed(m.clone(), &mut q), None);
            assert_eq!(q[0], m, "goes again first thing after reconnecting");
        }
        let before = q.len();
        for m in [
            Msg::Restore {
                name: "daily-20261115.json".into(),
            },
            Msg::CreateInvite {
                member: 3,
                role: Role::Member,
            },
            Msg::SetPassword {
                email: "a@b.c".into(),
                password: "x".into(),
                old_password: None,
            },
        ] {
            assert_eq!(
                send_failed(m, &mut q),
                Some(Event::Denied(UNSURE_SEND.into()))
            );
        }
        assert_eq!(q.len(), before, "not sent again by itself");
        assert_eq!(send_failed(Msg::Ping, &mut q), None);
        assert_eq!(q.len(), before);
    }
}
