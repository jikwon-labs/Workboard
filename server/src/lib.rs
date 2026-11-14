// workboard-server: keeps the team's copy of a Workboard workspace and keeps
// every connected app in step with it (see wb_net for the messages).
//
// The team's copy lives in `workspace.json` in the server folder; device id
// tags and the revision number live in `server.json`. Both are written to a
// temporary file first and then renamed, at most a second after a change, and
// once more when the server stops.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use wb_net::Msg;

pub const WORKSPACE_FILE: &str = "workspace.json";
pub const STATE_FILE: &str = "server.json";

/// `WORKBOARD_SERVER_HOME`, else ~/Library/Application Support/WorkboardServer.
pub fn home_dir() -> PathBuf {
    if let Ok(d) = std::env::var("WORKBOARD_SERVER_HOME") {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    Path::new(&home).join("Library/Application Support/WorkboardServer")
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
struct Saved {
    rev: u64,
    /// device id -> the id tag it was given
    devices: BTreeMap<String, u16>,
}

struct Inner {
    canon: Option<Value>,
    saved: Saved,
    dirty: bool,
}

struct Hub {
    home: PathBuf,
    inner: Mutex<Inner>,
    updates: broadcast::Sender<Arc<str>>,
    stop: watch::Receiver<bool>,
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

impl Hub {
    fn load(home: &Path) -> std::io::Result<Inner> {
        std::fs::create_dir_all(home)?;
        let read = |name: &str| -> std::io::Result<Option<Value>> {
            match std::fs::read_to_string(home.join(name)) {
                Ok(s) => serde_json::from_str(&s).map(Some).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}: {}", name, e))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            }
        };
        let canon = read(WORKSPACE_FILE)?;
        let saved = read(STATE_FILE)?.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        Ok(Inner { canon, saved, dirty: false })
    }

    /// Write to disk when something changed.
    fn flush(&self) {
        let (canon, saved) = {
            let mut g = self.inner.lock().unwrap();
            if !g.dirty {
                return;
            }
            g.dirty = false;
            (g.canon.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default()), serde_json::to_string_pretty(&g.saved).unwrap_or_default())
        };
        let mut ok = true;
        if let Some(c) = canon {
            ok &= write_atomic(&self.home.join(WORKSPACE_FILE), &c).map_err(|e| eprintln!("couldn't save {}: {}", WORKSPACE_FILE, e)).is_ok();
        }
        ok &= write_atomic(&self.home.join(STATE_FILE), &saved).map_err(|e| eprintln!("couldn't save {}: {}", STATE_FILE, e)).is_ok();
        if !ok {
            // try again on the next round
            self.inner.lock().unwrap().dirty = true;
        }
    }

    /// The tag a device uses: the one it had here before, else the one it asks
    /// for if nobody else has it, else a free one.
    fn tag_for(&self, device: &str, asked: Option<u16>) -> u16 {
        let mut g = self.inner.lock().unwrap();
        if let Some(t) = g.saved.devices.get(device) {
            return *t;
        }
        let used: Vec<u16> = g.saved.devices.values().copied().collect();
        let mut tag = asked.map(|t| t % 1024).unwrap_or((wb_sync::random_u64() % 1024) as u16);
        for _ in 0..1024 {
            if !used.contains(&tag) {
                break;
            }
            tag = (tag + 1) % 1024;
        }
        g.saved.devices.insert(device.to_string(), tag);
        g.dirty = true;
        tag
    }

    fn welcome(&self, tag: u16) -> String {
        let g = self.inner.lock().unwrap();
        Msg::Welcome { tag, snapshot: g.canon.clone(), rev: g.saved.rev }.to_json()
    }

    /// Merge an app's copy into the team's copy; tell everyone when it changed.
    fn push(&self, data: Value) {
        let data = wb_sync::shareable(&data);
        let msg = {
            let mut g = self.inner.lock().unwrap();
            let changed = match g.canon.as_mut() {
                None => {
                    // the first app to arrive brings the workspace
                    g.canon = Some(data);
                    true
                }
                Some(c) => {
                    let changed = wb_sync::merge(c, &[data]);
                    if changed {
                        *c = wb_sync::shareable(c);
                    }
                    changed
                }
            };
            if !changed {
                return;
            }
            g.saved.rev += 1;
            g.dirty = true;
            Msg::Update { data: g.canon.clone().unwrap_or_default(), rev: g.saved.rev }.to_json()
        };
        let _ = self.updates.send(Arc::from(msg));
    }

    fn latest(&self) -> Option<String> {
        let g = self.inner.lock().unwrap();
        g.canon.as_ref().map(|c| Msg::Update { data: c.clone(), rev: g.saved.rev }.to_json())
    }
}

async fn ws_route(ws: WebSocketUpgrade, State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    ws.max_message_size(wb_net::MAX_MESSAGE).max_frame_size(wb_net::MAX_MESSAGE).on_upgrade(move |socket| connection(socket, hub))
}

async fn send(socket: &mut WebSocket, text: &str) -> bool {
    socket.send(Message::Text(text.into())).await.is_ok()
}

async fn connection(mut socket: WebSocket, hub: Arc<Hub>) {
    let mut stop = hub.stop.clone();
    // the first message must be hello
    let hello = tokio::select! {
        m = tokio::time::timeout(Duration::from_secs(10), socket.recv()) => m,
        _ = stop.changed() => return,
    };
    let Ok(Some(Ok(Message::Text(t)))) = hello else { return };
    let Some(Msg::Hello { device, tag, token: _ }) = Msg::parse(t.as_str()) else {
        let _ = send(&mut socket, &Msg::Error { message: "Say hello first.".into() }.to_json()).await;
        return;
    };
    let tag = hub.tag_for(&device, tag);
    // listen before sending the copy so no update falls in between
    let mut updates = hub.updates.subscribe();
    if !send(&mut socket, &hub.welcome(tag)).await {
        return;
    }
    let silent = Duration::from_secs(wb_net::SILENT_SECS + 15);
    loop {
        tokio::select! {
            m = tokio::time::timeout(silent, socket.recv()) => {
                let m = match m {
                    Err(_) => break, // heard nothing for too long
                    Ok(None) | Ok(Some(Err(_))) => break,
                    Ok(Some(Ok(m))) => m,
                };
                match m {
                    Message::Text(t) => match Msg::parse(t.as_str()) {
                        Some(Msg::Push { data }) => {
                            let hub2 = hub.clone();
                            // merging can take a moment on a big workspace
                            let _ = tokio::task::spawn_blocking(move || hub2.push(data)).await;
                        }
                        Some(Msg::Ping) => {
                            if !send(&mut socket, &Msg::Pong.to_json()).await {
                                break;
                            }
                        }
                        _ => {}
                    },
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            u = updates.recv() => {
                let text = match u {
                    Ok(t) => t.to_string(),
                    // fell behind: the newest copy says it all
                    Err(broadcast::error::RecvError::Lagged(_)) => match hub.latest() { Some(t) => t, None => continue },
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if !send(&mut socket, &text).await {
                    break;
                }
            }
            _ = stop.changed() => break,
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}

/// Serve on `listener` until `shutdown` finishes, keeping the copy in `home`.
pub async fn serve(listener: tokio::net::TcpListener, home: PathBuf, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    let inner = Hub::load(&home)?;
    let (stop_tx, stop_rx) = watch::channel(false);
    let (updates, _) = broadcast::channel(64);
    let hub = Arc::new(Hub { home, inner: Mutex::new(inner), updates, stop: stop_rx });
    let saver = {
        let hub = hub.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let hub = hub.clone();
                let _ = tokio::task::spawn_blocking(move || hub.flush()).await;
            }
        })
    };
    let app = Router::new().route(wb_net::PATH, get(ws_route)).with_state(hub.clone());
    let res = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.await;
            // open connections end themselves when they see this
            let _ = stop_tx.send(true);
        })
        .await;
    saver.abort();
    hub.flush();
    res
}

/// The address to listen on by default: this Mac's Tailscale address, else
/// only this computer.
pub fn default_bind() -> String {
    for exe in ["tailscale", "/usr/local/bin/tailscale", "/opt/homebrew/bin/tailscale", "/Applications/Tailscale.app/Contents/MacOS/Tailscale"] {
        if let Ok(out) = std::process::Command::new(exe).args(["ip", "-4"]).output() {
            if out.status.success() {
                if let Some(ip) = String::from_utf8_lossy(&out.stdout).lines().next().map(|l| l.trim().to_string()).filter(|l| l.parse::<std::net::Ipv4Addr>().is_ok()) {
                    return ip;
                }
            }
        }
    }
    "127.0.0.1".into()
}
