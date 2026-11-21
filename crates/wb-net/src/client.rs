// The app's side of the connection: a thread that signs in, keeps the
// line alive, sends what the app queues and hands back what the server says.

use crate::*;
use serde_json::Value;
use std::io::ErrorKind;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tungstenite::protocol::WebSocketConfig;
use tungstenite::{Message, WebSocket};

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

pub(crate) fn connect(addr: &str) -> Result<WebSocket<Conn>, End> {
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
pub(crate) const MAX_QUEUED: usize = 32;

/// Keep a request for after the next welcome.
pub(crate) fn queue(queued: &mut Vec<Msg>, m: Msg) {
    if m.waits_for_connection() {
        queued.push(m);
        if queued.len() > MAX_QUEUED {
            queued.remove(0);
        }
    }
}

/// Sending `m` failed, though it may have reached the server anyway: try it
/// again after reconnecting when doing it twice doesn't matter, else say so.
pub(crate) fn send_failed(m: Msg, queued: &mut Vec<Msg>) -> Option<Event> {
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

pub(crate) enum End {
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
pub(crate) fn send(ws: &mut WebSocket<Conn>, m: &Msg) -> Result<(), String> {
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
