// Notifications from a workspace that isn't open on this computer.
//
// A `Watcher` keeps a small connection of its own to one team workspace,
// signed in with that workspace's device token: hello with `watch`, the
// server answers `watching`, then sends a `note` for each new message the
// person should hear about (the same ones an iPhone gets a push for: DMs,
// mentions, replies in their threads). No copy of the workspace travels and
// the device doesn't show as online there.
//
// It reconnects by itself (waiting longer each time, up to a minute). A
// server from before `watch` answers `welcome` instead; the watcher hangs up
// and asks again every OLD_SERVER_SECS, so it starts working once that server
// is updated. Signed out there (`auth` error) or the workspace deleted: it
// says so once and stops.

use super::*;
use crate::client::{connect, send, End};
use std::io::ErrorKind;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};
use tungstenite::{Message, WebSocket};

/// How long to wait before asking a server from before `watch` again.
const OLD_SERVER_SECS: u64 = 10 * 60;
/// The longest wait between tries.
const MAX_BACKOFF_SECS: u64 = 60;

/// What a watcher heard.
#[derive(Clone, Debug, PartialEq)]
pub enum Watch {
    /// Signed in and listening (after each connect).
    Started,
    /// A new message for this person.
    Note {
        title: String,
        body: String,
        channel: Option<u64>,
        parent: Option<u64>,
    },
    /// This device isn't signed in there any more (it stopped).
    Refused(String),
    /// The workspace was deleted (it stopped).
    Gone(String),
}

/// A watch connection on its own thread. Dropping it hangs up.
pub struct Watcher {
    stop: Sender<()>,
    rx: Receiver<Watch>,
}

impl Watcher {
    /// Start watching the workspace at `addr` as `device` (signed in with
    /// `token`). `wake` is called after each event.
    pub fn start(
        addr: &str,
        device: &str,
        token: &str,
        app: Option<String>,
        wake: Wake,
    ) -> Watcher {
        let (stop, srx) = mpsc::channel();
        let (etx, rx) = mpsc::channel();
        let addr = addr.to_string();
        let sign_in = SignIn {
            device: device.to_string(),
            token: Some(token.to_string()),
            app,
            watch: true,
            ..Default::default()
        };
        std::thread::Builder::new()
            .name("workboard-watch".into())
            .spawn(move || watch_run(addr, sign_in, srx, Heard { tx: etx, wake }))
            .expect("start watch thread");
        Watcher { stop, rx }
    }

    /// Everything heard since the last call.
    pub fn events(&self) -> Vec<Watch> {
        self.rx.try_iter().collect()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

/// Where a watcher's events go.
struct Heard {
    tx: Sender<Watch>,
    wake: Wake,
}

impl Heard {
    fn emit(&self, w: Watch) -> bool {
        let ok = self.tx.send(w).is_ok();
        (self.wake)();
        ok
    }
}

/// How one watch session ended.
enum Ended {
    Stop,
    /// try again after this many seconds (None: the usual backoff)
    Again(Option<u64>),
    /// for good, telling the app this
    Done(Watch),
}

/// Wait `secs`; false when asked to stop.
fn pause(srx: &Receiver<()>, secs: u64) -> bool {
    matches!(
        srx.recv_timeout(Duration::from_secs(secs)),
        Err(mpsc::RecvTimeoutError::Timeout)
    )
}

fn stopped(srx: &Receiver<()>) -> bool {
    !matches!(srx.try_recv(), Err(TryRecvError::Empty))
}

fn watch_run(addr: String, sign_in: SignIn, srx: Receiver<()>, line: Heard) {
    let mut tries: u32 = 0;
    loop {
        let ended = match connect(&addr) {
            Ok(mut ws) => {
                let e = watch_session(&mut ws, &sign_in, &srx, &line, &mut tries);
                let _ = ws.close(None);
                let _ = ws.flush();
                e
            }
            Err(End::Gone(why)) => Ended::Done(Watch::Gone(why)),
            Err(_) => Ended::Again(None),
        };
        let secs = match ended {
            Ended::Stop => return,
            Ended::Done(w) => {
                line.emit(w);
                // stay until let go
                let _ = srx.recv();
                return;
            }
            Ended::Again(Some(s)) => s,
            Ended::Again(None) => {
                tries = tries.saturating_add(1);
                (1u64 << tries.min(6)).min(MAX_BACKOFF_SECS)
            }
        };
        if !pause(&srx, secs) {
            return;
        }
    }
}

fn watch_session(
    ws: &mut WebSocket<Conn>,
    sign_in: &SignIn,
    srx: &Receiver<()>,
    line: &Heard,
    tries: &mut u32,
) -> Ended {
    if send(ws, &Msg::from(sign_in)).is_err() {
        return Ended::Again(None);
    }
    let mut watching = false;
    let mut last_heard = Instant::now();
    let mut last_ping = Instant::now();
    loop {
        if stopped(srx) {
            return Ended::Stop;
        }
        if watching && last_ping.elapsed() >= Duration::from_secs(PING_SECS) {
            last_ping = Instant::now();
            if send(ws, &Msg::Ping).is_err() {
                return Ended::Again(None);
            }
        }
        if last_heard.elapsed() >= Duration::from_secs(SILENT_SECS) {
            return Ended::Again(None);
        }
        let text = match ws.read() {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => return Ended::Again(None),
            Ok(_) => {
                last_heard = Instant::now();
                continue;
            }
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                continue;
            }
            Err(_) => return Ended::Again(None),
        };
        last_heard = Instant::now();
        match Msg::parse(text.as_str()) {
            Some(Msg::Watching) => {
                watching = true;
                *tries = 0;
                if !line.emit(Watch::Started) {
                    return Ended::Stop;
                }
            }
            // a server from before watch connections: it sent the whole copy
            Some(Msg::Welcome { .. }) => return Ended::Again(Some(OLD_SERVER_SECS)),
            Some(Msg::Note {
                title,
                body,
                channel,
                parent,
            }) => {
                if !line.emit(Watch::Note {
                    title,
                    body,
                    channel,
                    parent,
                }) {
                    return Ended::Stop;
                }
            }
            Some(Msg::Ping) => {
                if send(ws, &Msg::Pong).is_err() {
                    return Ended::Again(None);
                }
            }
            Some(Msg::Error {
                message,
                code,
                gone,
                ..
            }) if code.as_deref() == Some(CODE_AUTH) => {
                return Ended::Done(if gone {
                    Watch::Gone(message)
                } else {
                    Watch::Refused(message)
                })
            }
            Some(Msg::Error { .. }) => return Ended::Again(None),
            _ => {}
        }
    }
}
