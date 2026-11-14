// Talking to workboard-server: the messages both sides send (JSON text over a
// WebSocket) and the app's side of the connection, which runs on its own
// thread so the window never waits for the network.
//
// Order of a connection: the app sends `hello` (with its device token, or an
// invite code, or a claim on an empty server), the server answers `welcome`
// with the team's copy (and a new token when it issued one), then the app
// sends `push` with its whole shared copy whenever it changes and the server
// sends `update` to everyone when the team's copy changed. Both sides answer
// `ping` with `pong`. Owners and admins also send the access requests
// (`create_invite`, `set_role`, `revoke`, `access_list`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

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
}

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
    },
    Push {
        data: Value,
    },
    Update {
        data: Value,
        rev: u64,
    },
    Ping,
    Pong,
    /// Owner or admin: a one-time code that signs someone in as `member`.
    CreateInvite {
        member: u64,
        #[serde(default)]
        role: Role,
    },
    Invite {
        member: u64,
        code: String,
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
    },
    Error {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
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
        Msg::Error { message: message.to_string(), code: code.map(|c| c.to_string()) }
    }
}

/// Turn what a person typed ("macmini", "100.1.2.3", "host:9000",
/// "ws://host:8787/ws") into (host, port, url).
pub fn server_url(addr: &str) -> Result<(String, u16, String), String> {
    let a = addr.trim().trim_end_matches('/');
    if a.is_empty() {
        return Err("Type the server's address first.".into());
    }
    if a.starts_with("wss://") || a.starts_with("https://") {
        return Err("Secure (wss) addresses aren't supported. Use the server's Tailscale name or address.".into());
    }
    let rest = a.strip_prefix("ws://").or_else(|| a.strip_prefix("http://")).unwrap_or(a);
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, PATH),
    };
    let (host, port) = if let Some(h) = hostport.strip_prefix('[') {
        // [ipv6]:port
        let end = h.find(']').ok_or("That address doesn't look right.")?;
        let port = h[end + 1..].strip_prefix(':').map(|p| p.parse::<u16>()).transpose().map_err(|_| "That port isn't a number.")?;
        (h[..end].to_string(), port)
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (h.to_string(), Some(p.parse::<u16>().map_err(|_| "That port isn't a number.")?)),
            _ => (hostport.to_string(), None),
        }
    };
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err("That address doesn't look right.".into());
    }
    let port = port.unwrap_or(DEFAULT_PORT);
    let shown = if host.contains(':') { format!("[{}]", host) } else { host.clone() };
    Ok((host, port, format!("ws://{}:{}{}", shown, port, path)))
}

/// Where the server publishes app updates for Sparkle (see the server's
/// /updates route): http on the same host and port as the sync connection.
pub fn updates_url(addr: &str) -> Result<String, String> {
    let (host, port, _) = server_url(addr)?;
    let shown = if host.contains(':') { format!("[{}]", host) } else { host };
    Ok(format!("http://{}:{}/updates/appcast.xml", shown, port))
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
    Welcome { tag: u16, snapshot: Option<Value>, rev: u64, member: u64, role: Role, token: Option<String> },
    Update { data: Value, rev: u64 },
    Invite { member: u64, code: String },
    Access { people: Vec<Person>, role: Role },
    /// A request was turned down; the connection stays.
    Denied(String),
    CallToken { channel: u64, url: String, token: String },
}

/// How this computer signs in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignIn {
    pub device: String,
    pub tag: Option<u16>,
    pub token: Option<String>,
    pub invite: Option<String>,
    pub claim: Option<u64>,
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

    /// Send a request (access requests). Dropped when not connected.
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

fn connect(addr: &str) -> Result<WebSocket<TcpStream>, String> {
    let (host, port, url) = server_url(addr)?;
    let addrs: Vec<_> = (host.as_str(), port).to_socket_addrs().map_err(|e| format!("Can't find {}: {}", host, e))?.collect();
    let mut last = format!("Can't find {}", host);
    for sa in addrs {
        match TcpStream::connect_timeout(&sa, Duration::from_secs(5)) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                stream.set_read_timeout(Some(Duration::from_secs(10))).map_err(|e| e.to_string())?;
                let cfg = WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE)).max_frame_size(Some(MAX_MESSAGE));
                let (ws, _) = tungstenite::client::client_with_config(url.as_str(), stream, Some(cfg)).map_err(|e| format!("The server didn't answer like Workboard: {}", e))?;
                ws.get_ref().set_read_timeout(Some(Duration::from_millis(100))).map_err(|e| e.to_string())?;
                return Ok(ws);
            }
            Err(e) => last = format!("Can't reach {}:{}: {}", host, port, e),
        }
    }
    Err(last)
}

/// Wait before trying again; false when the app asked to stop.
fn wait(crx: &Receiver<Cmd>, pending: &mut Option<Value>, secs: u64) -> bool {
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        match crx.recv_timeout(Duration::from_millis(100)) {
            Ok(Cmd::Push(v)) => *pending = Some(v),
            Ok(Cmd::Send(_)) => {}
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
    let mut tries: u64 = 0;
    line.emit(Event::Status(Status::Connecting));
    loop {
        let end = match connect(&addr) {
            Ok(mut ws) => {
                let end = session(&mut ws, &line, &crx, &mut sign_in, &mut pending, &mut tries);
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
        if !wait(&crx, &mut pending, tries.min(5)) {
            return;
        }
    }
}

fn session(ws: &mut WebSocket<TcpStream>, line: &Line, crx: &Receiver<Cmd>, sign_in: &mut SignIn, pending: &mut Option<Value>, tries: &mut u64) -> End {
    let send = |ws: &mut WebSocket<TcpStream>, m: &Msg| ws.send(Message::text(m.to_json())).map_err(|e| e.to_string());
    let hello = Msg::Hello { device: sign_in.device.clone(), tag: sign_in.tag, token: sign_in.token.clone(), invite: sign_in.invite.clone(), claim: sign_in.claim };
    if let Err(e) = send(ws, &hello) {
        return End::Lost(e);
    }
    let mut welcomed = false;
    let mut last_heard = Instant::now();
    let mut last_ping = Instant::now();
    loop {
        let mut outbox: Vec<Msg> = vec![];
        loop {
            match crx.try_recv() {
                Ok(Cmd::Push(v)) => *pending = Some(v),
                Ok(Cmd::Send(m)) => outbox.push(m),
                Ok(Cmd::Stop) | Err(TryRecvError::Disconnected) => return End::Stop,
                Err(TryRecvError::Empty) => break,
            }
        }
        if welcomed {
            if let Some(v) = pending.take() {
                if let Err(e) = send(ws, &Msg::Push { data: v }) {
                    return End::Lost(e);
                }
            }
            for m in outbox {
                if let Err(e) = send(ws, &m) {
                    return End::Lost(e);
                }
            }
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
        let emit = |e: Event| -> Result<(), End> { if line.emit(e) { Ok(()) } else { Err(End::Stop) } };
        let res: Result<(), End> = match ws.read() {
            Ok(Message::Text(t)) => {
                last_heard = Instant::now();
                match Msg::parse(t.as_str()) {
                    Some(Msg::Welcome { tag, snapshot, rev, member, role, token }) => {
                        // sign in the same way next time
                        sign_in.tag = Some(tag);
                        if let Some(t) = &token {
                            sign_in.token = Some(t.clone());
                        }
                        sign_in.invite = None;
                        sign_in.claim = None;
                        welcomed = true;
                        *tries = 0;
                        emit(Event::Status(Status::Connected)).and_then(|_| emit(Event::Welcome { tag, snapshot, rev, member, role, token }))
                    }
                    Some(Msg::Update { data, rev }) => emit(Event::Update { data, rev }),
                    Some(Msg::Invite { member, code }) => emit(Event::Invite { member, code }),
                    Some(Msg::Access { people, role }) => emit(Event::Access { people, role }),
                    Some(Msg::CallToken { channel, url, token }) => emit(Event::CallToken { channel, url, token }),
                    Some(Msg::Ping) => send(ws, &Msg::Pong).map_err(End::Lost),
                    Some(Msg::Error { message, code }) => match code.as_deref() {
                        Some(CODE_AUTH) => Err(End::Refused(message)),
                        Some(CODE_DENIED) => emit(Event::Denied(message)),
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
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Ok(()),
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
        assert_eq!(updates_url("macmini").unwrap(), "http://macmini:8787/updates/appcast.xml");
        assert_eq!(updates_url("ws://mac-mini.tail1.ts.net:9000/ws").unwrap(), "http://mac-mini.tail1.ts.net:9000/updates/appcast.xml");
        assert_eq!(updates_url("[::1]:8787").unwrap(), "http://[::1]:8787/updates/appcast.xml");
        assert!(updates_url("").is_err());
        assert_eq!(server_url(" 100.1.2.3:9000 ").unwrap(), ("100.1.2.3".into(), 9000, "ws://100.1.2.3:9000/ws".into()));
        assert_eq!(server_url("ws://mac-mini.tail1.ts.net/").unwrap().2, "ws://mac-mini.tail1.ts.net:8787/ws");
        assert_eq!(server_url("http://h:1/ws").unwrap().2, "ws://h:1/ws");
        assert_eq!(server_url("[::1]:8787").unwrap().2, "ws://[::1]:8787/ws");
        assert!(server_url("").is_err());
        assert!(server_url("wss://x").is_err());
        assert!(server_url("h:port").is_err());
    }

    #[test]
    fn messages_round_trip_as_tagged_json() {
        let m = Msg::Hello { device: "d1".into(), tag: Some(3), token: None, invite: None, claim: None };
        assert_eq!(m.to_json(), r#"{"type":"hello","device":"d1","tag":3}"#);
        assert_eq!(Msg::parse(r#"{"type":"hello","device":"d1"}"#), Some(Msg::Hello { device: "d1".into(), tag: None, token: None, invite: None, claim: None }));
        assert_eq!(Msg::parse(r#"{"type":"ping"}"#), Some(Msg::Ping));
        assert_eq!(Msg::parse(r#"{"type":"set_role","member":5,"role":"admin"}"#), Some(Msg::SetRole { member: 5, role: Role::Admin }));
    }

    #[test]
    fn roles_order_from_member_to_owner() {
        assert!(Role::Owner.manages() && Role::Admin.manages() && !Role::Member.manages());
        assert!(Role::Owner > Role::Admin && Role::Admin > Role::Member);
    }
}
