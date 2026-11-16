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
pub use conn::{trust_certificate, Conn};
pub use files::{download, forgot_password, remote_size, sha256_file, upload, Auth, FileError};

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

fn pct_encode(s: &str) -> String {
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
        }
    }

    /// A call_join for `channel` turned down.
    pub fn call_denied(message: &str, channel: u64) -> Msg {
        Msg::Error {
            message: message.to_string(),
            code: Some(CODE_DENIED.to_string()),
            channel: Some(channel),
            put_back: false,
        }
    }

    /// Changes in a pushed copy turned down; the team's version follows.
    pub fn put_back(message: &str) -> Msg {
        Msg::Error {
            message: message.to_string(),
            code: Some(CODE_DENIED.to_string()),
            channel: None,
            put_back: true,
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
    /// The app's version and sync format, sent in hello.
    pub app: Option<String>,
    pub sync: Option<u32>,
    /// Ask for whole copies every time, like apps from before deltas (tests).
    pub full_copies: bool,
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

fn connect(addr: &str) -> Result<WebSocket<Conn>, String> {
    let (host, port, url) = server_url(addr)?;
    let stream = conn::dial(&host, port, is_secure(&url), Duration::from_secs(5))?;
    stream
        .tcp()
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
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
            // rustls reports a certificate it doesn't trust as InvalidData
            tungstenite::Error::Io(e) if e.kind() == ErrorKind::InvalidData => {
                format!("The secure connection to {} failed: {}", host, e)
            }
            tungstenite::Error::Io(e) => format!("Can't reach {}:{}: {}", host, port, e),
            e => format!("The server didn't answer like Workboard: {}", e),
        })?;
    ws.get_ref()
        .tcp()
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| e.to_string())?;
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
            Err(e) => End::Lost(e),
        };
        match end {
            End::Stop => return,
            End::Refused(why) => {
                line.emit(Event::Status(Status::Refused(why)));
                // stay until the app lets go, so it can read the status
                while let Ok(c) = crx.recv() {
                    if matches!(c, Cmd::Stop) {
                        return;
                    }
                }
                return;
            }
            End::Lost(why) => {
                if !line.emit(Event::Status(Status::Reconnecting(why))) {
                    return;
                }
            }
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
    let send = |ws: &mut WebSocket<Conn>, m: &Msg| {
        ws.send(Message::text(m.to_json()))
            .map_err(|e| e.to_string())
    };
    let hello = Msg::Hello {
        device: sign_in.device.clone(),
        tag: sign_in.tag,
        token: sign_in.token.clone(),
        invite: sign_in.invite.clone(),
        claim: sign_in.claim,
        email: sign_in.email.clone(),
        password: sign_in.password.clone(),
        new_password: sign_in.new_password.clone(),
        reset_code: sign_in.reset_code.clone(),
        app: sign_in.app.clone(),
        sync: sign_in.sync,
        deltas: !sign_in.full_copies,
    };
    if let Err(e) = send(ws, &hello) {
        return End::Lost(e);
    }
    let mut welcomed = false;
    // what the server has of our copy, when it takes deltas (else None)
    let mut summary: Option<wb_sync::Summary> = None;
    let mut deltas = false;
    let mut rev: u64 = 0;
    let mut last_heard = Instant::now();
    let mut last_ping = Instant::now();
    // the app let go: what it pushed or sent just before still goes, then the
    // connection closes (logging out or quitting right after a change)
    let mut stopping = false;
    loop {
        loop {
            match crx.try_recv() {
                Ok(Cmd::Push(v)) => *pending = Some(v),
                Ok(Cmd::Send(m)) if welcomed => queued.push(m),
                Ok(Cmd::Send(m)) => queue(queued, m),
                Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => {
                    stopping = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if stopping && !welcomed {
            return End::Stop;
        }
        if welcomed {
            // the copy first, so a conversation made a moment ago is on the
            // server before a call_join for it
            if let Some(v) = pending.take() {
                let push = match summary.as_mut() {
                    // only what the server doesn't have yet (nothing: no push)
                    Some(s) => wb_sync::delta(s, &v).inspect(|d| s.absorb(d)),
                    None => {
                        if deltas {
                            summary = Some(wb_sync::Summary::of(&v));
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
        let emit = |e: Event| -> Result<(), End> {
            if line.emit(e) {
                Ok(())
            } else {
                Err(End::Stop)
            }
        };
        let res: Result<(), End> = match ws.read() {
            Ok(Message::Text(t)) => {
                last_heard = Instant::now();
                match Msg::parse(t.as_str()) {
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
                        deltas = server_deltas && !sign_in.full_copies;
                        rev = r;
                        summary = if deltas {
                            snapshot.as_ref().map(wb_sync::Summary::of)
                        } else {
                            None
                        };
                        // sign in the same way next time: with the token, not the password
                        sign_in.tag = Some(tag);
                        if let Some(t) = &token {
                            sign_in.token = Some(t.clone());
                        }
                        sign_in.invite = None;
                        sign_in.claim = None;
                        sign_in.email = None;
                        sign_in.password = None;
                        sign_in.new_password = None;
                        sign_in.reset_code = None;
                        welcomed = true;
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
                        if deltas {
                            summary = Some(wb_sync::Summary::of(&data));
                        }
                        rev = r;
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
                        let asked = if base > rev {
                            send(ws, &Msg::Resync).map_err(End::Lost)
                        } else {
                            Ok(())
                        };
                        if let Some(s) = summary.as_mut() {
                            s.absorb(&data);
                        }
                        rev = rev.max(r);
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
                    Some(Msg::Ping) => send(ws, &Msg::Pong).map_err(End::Lost),
                    Some(Msg::Error {
                        message,
                        code,
                        channel,
                        put_back,
                    }) => match code.as_deref() {
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

#[cfg(test)]
mod tests {
    use super::*;

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
            app: None,
            sync: None,
            deltas: false,
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
        };
        assert_eq!(
            Msg::parse(
                r#"{"type":"push_token","token":"ab12","platform":"ios","env":"development"}"#
            ),
            Some(t.clone())
        );
        assert_eq!(Msg::parse(&t.to_json()), Some(t));
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
