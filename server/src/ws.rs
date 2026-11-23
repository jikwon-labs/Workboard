// The WebSocket connection each app keeps open, and who is online.

use super::*;

pub(crate) type WebSocket = WebSocketStream<TokioIo<hyper::upgrade::Upgraded>>;

pub(crate) fn ws_config(max: usize) -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(max))
        .max_frame_size(Some(max))
}

/// Sec-WebSocket-Accept for a Sec-WebSocket-Key (RFC 6455).
pub(crate) fn accept_key(key: &[u8]) -> String {
    use base64::Engine as _;
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(key);
    h.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64::engine::general_purpose::STANDARD.encode(h.finalize())
}

/// The WebSocket upgrade, done here rather than with axum's extractor so the
/// size limit can change after signing in: until then a message may be at
/// most MAX_HELLO, afterwards MAX_MESSAGE (a whole workspace).
pub(crate) async fn ws_route(
    State(hub): State<Arc<Hub>>,
    mut req: axum::extract::Request,
) -> Response {
    let h = req.headers();
    // the address this app reached us at, for links in invite mails
    let host = reached_at(h);
    let has = |name: header::HeaderName, want: &str| {
        h.get_all(name).iter().any(|v| {
            v.to_str()
                .is_ok_and(|v| v.split(',').any(|p| p.trim().eq_ignore_ascii_case(want)))
        })
    };
    let ok = req.method() == Method::GET
        && req.version() == Version::HTTP_11
        && has(header::CONNECTION, "upgrade")
        && has(header::UPGRADE, "websocket")
        && has(header::SEC_WEBSOCKET_VERSION, "13");
    let Some(key) = h
        .get(header::SEC_WEBSOCKET_KEY)
        .filter(|_| ok)
        .map(|k| accept_key(k.as_bytes()))
    else {
        return (
            StatusCode::BAD_REQUEST,
            "This address takes Workboard's WebSocket connection.",
        )
            .into_response();
    };
    let on_upgrade = hyper::upgrade::on(&mut req);
    tokio::spawn(async move {
        let Ok(upgraded) = on_upgrade.await else {
            return;
        };
        let socket = WebSocketStream::from_raw_socket(
            TokioIo::new(upgraded),
            WsRole::Server,
            Some(ws_config(MAX_HELLO)),
        )
        .await;
        connection(socket, hub, host).await;
    });
    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_ACCEPT, key)
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

pub(crate) async fn send(socket: &mut WebSocket, text: &str) -> bool {
    socket.send(Message::text(text)).await.is_ok()
}

pub(crate) struct Counted(Arc<Hub>);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::Relaxed);
    }
}

/// A signed-in device's connection, counted in Hub::online (and its member
/// in Hub::members_online) while it lasts.
pub(crate) struct Online(Arc<Hub>, String, u64);

impl Online {
    pub(crate) fn new(hub: &Arc<Hub>, device: &str, member: u64) -> Online {
        *hub.online.locked().entry(device.to_string()).or_insert(0) += 1;
        hub.member_seen(member, true);
        Online(hub.clone(), device.to_string(), member)
    }
}

impl Drop for Online {
    fn drop(&mut self) {
        {
            let mut o = self.0.online.locked();
            if let Some(n) = o.get_mut(&self.1) {
                *n -= 1;
                if *n == 0 {
                    o.remove(&self.1);
                }
            }
        }
        self.0.member_seen(self.2, false);
    }
}

impl Hub {
    /// A connection of `member` opened (`on`) or closed; tells everyone
    /// when that changes who is online.
    pub(crate) fn member_seen(&self, member: u64, on: bool) {
        let mut m = self.members_online.locked();
        if on {
            *m.entry(member).or_insert(0) += 1;
        } else if let Some(n) = m.get_mut(&member) {
            *n -= 1;
            if *n == 0 {
                m.remove(&member);
            }
        }
        let now: Vec<u64> = m.keys().copied().collect();
        self.presence.send_if_modified(|v| {
            let changed = *v != now;
            if changed {
                *v = now;
            }
            changed
        });
    }
}

/// Someone typing in a conversation, on its way to the other connections.
#[derive(Clone, Debug)]
pub(crate) struct Typing {
    pub(crate) member: u64,
    pub(crate) channel: u64,
    /// the message whose thread they're answering, if any
    pub(crate) thread: Option<u64>,
    /// who may hear it: the members of a DM or private channel; None for a
    /// channel everyone sees
    pub(crate) to: Option<Arc<Vec<u64>>>,
}

impl Typing {
    /// Whether `member`'s app hears it (never the typist's own).
    pub(crate) fn for_member(&self, member: u64) -> bool {
        member != self.member && self.to.as_ref().is_none_or(|to| to.contains(&member))
    }
}

/// How often a connection's typing in one conversation is passed on at
/// most; the app sends it every few seconds while its person types.
const TYPING_GAP: Duration = Duration::from_millis(1500);

impl Hub {
    /// `member` is typing in `channel` (in the thread of message `thread`):
    /// tell the others who can see it. Nothing when the conversation isn't
    /// there or is closed to them.
    pub(crate) fn typing_in(&self, member: u64, channel: u64, thread: Option<u64>) {
        let t = {
            let g = self.state();
            let Some(ch) = g
                .canon
                .as_deref()
                .and_then(|c| c.get("channels"))
                .and_then(|c| c.as_array())
                .and_then(|a| {
                    a.iter()
                        .find(|c| c.get("id").and_then(|i| i.as_u64()) == Some(channel))
                })
            else {
                return;
            };
            if wb_sync::closed_to(ch, member) {
                return;
            }
            // the same rule as closed_to: a DM or private channel with a
            // member list is only for them
            let restricted = ch.get("kind").and_then(|k| k.as_str()) == Some("Direct")
                || ch.get("private").and_then(|p| p.as_bool()) == Some(true);
            let members: Vec<u64> = ch
                .get("members")
                .and_then(|m| m.as_array())
                .map(|a| a.iter().filter_map(|m| m.as_u64()).collect())
                .unwrap_or_default();
            Typing {
                member,
                channel,
                thread,
                to: (restricted && !members.is_empty()).then(|| Arc::new(members)),
            }
        };
        // nobody listening is fine
        let _ = self.typing.send(t);
    }
}

/// An access broadcast no connection's request caused: every connection
/// re-sends its list. Connections are numbered from 1.
pub(crate) const EVERYONE: u64 = 0;
pub(crate) static NEXT_CONNECTION: AtomicU64 = AtomicU64::new(1);

/// What an app hears when the work done for its request failed on the way
/// (the blocking task panicked).
const PROBLEM: &str = "The server had a problem. Try again.";

/// What a device whose access was taken away hears before it's let go.
pub(crate) const ACCESS_REMOVED: &str = "Your access to this server was removed.";

/// The error a device whose access was taken away hears.
fn access_removed() -> String {
    Msg::error(ACCESS_REMOVED, Some(CODE_AUTH)).to_json()
}

/// The error a request that isn't allowed (or failed) is answered with.
fn denied(why: &str) -> String {
    Msg::error(why, Some(CODE_DENIED)).to_json()
}

/// Run `f` away from the connections (password hashing takes a moment);
/// PROBLEM when it failed on the way.
async fn off_thread<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(f)
        .await
        .unwrap_or_else(|_| Err(PROBLEM.into()))
}

/// A connection whose hello signed in.
struct SignedIn {
    device: String,
    member: u64,
    role: Role,
    tag: u16,
    token: Option<String>,
    app: Option<String>,
    sync: Option<u32>,
    deltas: bool,
    /// only notifications (see `watching`)
    watch: bool,
    /// the copy the app has from this server already (`Hello::have`)
    have: Option<wb_net::Have>,
    /// the app takes `mine_part` messages
    mine_parts: bool,
}

/// The hello a connection starts with, and signing in with it. None when
/// the connection is done (it was told why, if it should hear).
async fn hello(
    socket: &mut WebSocket,
    hub: &Arc<Hub>,
    stop: &mut watch::Receiver<bool>,
) -> Option<SignedIn> {
    // the first message must be hello
    let hello = tokio::select! {
        m = tokio::time::timeout(Duration::from_secs(10), socket.next()) => m,
        _ = stop.changed() => return None,
    };
    let Ok(Some(Ok(Message::Text(t)))) = hello else {
        return None;
    };
    let Some(Ok(hello)) = Msg::parse(t.as_str()).map(Hello::try_from) else {
        let _ = send(socket, &Msg::error("Say hello first.", None).to_json()).await;
        return None;
    };
    let (device, app, sync, deltas, watch, have, mine_parts) = (
        hello.device.clone(),
        hello.app.clone(),
        hello.sync,
        !hello.full_copies,
        hello.watch,
        hello.have,
        hello.mine_parts,
    );
    let h = hub.clone();
    // password hashing takes a moment
    let signed = off_thread(move || h.sign_in(&hello)).await;
    let (member, role, tag, token) = match signed {
        Ok(x) => x,
        Err(why) => {
            let _ = send(socket, &Msg::error(&why, Some(CODE_AUTH)).to_json()).await;
            let _ = socket.send(Message::Close(None)).await;
            return None;
        }
    };
    Some(SignedIn {
        device,
        member,
        role,
        tag,
        token,
        app,
        sync,
        deltas,
        watch,
        have,
        mine_parts,
    })
}

/// A signed-in connection: who it is, and what it remembers between the
/// app's messages.
struct Conn {
    hub: Arc<Hub>,
    member: u64,
    device: String,
    /// this connection's number (see EVERYONE)
    conn: u64,
    /// the address the app reached us at, for links in invite mails
    host: Option<String>,
    /// the app is too old to know every field (see OLDEST_SYNC)
    too_old: bool,
    told_too_old: bool,
    told_put_back: Option<std::time::Instant>,
    /// what the last copy sent left out (see Canon::delta_for)
    was_hidden: Arc<Vec<u64>>,
    /// the copies sent, for catching the app up when it reconnects
    seen: Arc<Mutex<Seen>>,
    /// the app takes `mine_part` messages
    mine_parts: bool,
    /// the conversation this connection's typing was last passed on for,
    /// and when (see TYPING_GAP)
    typed: Option<(u64, Option<u64>, std::time::Instant)>,
    /// where AI answers go on their way to the app (see `ask_ai`)
    ai_out: tokio::sync::mpsc::UnboundedSender<String>,
    /// AI requests still being answered, by the app's id: set to stop one
    ai_going: std::collections::HashMap<u64, Arc<std::sync::atomic::AtomicBool>>,
}

impl Conn {
    /// A copy of revision `rev` that left out `left_out` went to the app.
    fn sent(&mut self, rev: u64, left_out: Vec<u64>) {
        self.was_hidden = Arc::new(left_out);
        self.seen.locked().sent(rev, &self.was_hidden);
    }

    /// Starts answering AI request `id` (ai.rs). The answer goes to the app
    /// in pieces through `ai_out`, then `ai_done`, while this connection
    /// goes on with everything else; nothing more is sent once the app
    /// stopped it (`ai_stop`) or left.
    fn ask_ai(&mut self, id: u64, ask: wb_net::AiAsk, lang: String) {
        use std::sync::atomic::{AtomicBool, Ordering};
        // forget the requests that are done
        self.ai_going.retain(|_, s| Arc::strong_count(s) > 1);
        let stop = Arc::new(AtomicBool::new(false));
        if let Some(old) = self.ai_going.insert(id, stop.clone()) {
            // the same id again: the earlier answer isn't wanted
            old.store(true, Ordering::Relaxed);
        }
        let (hub, member, out) = (self.hub.clone(), self.member, self.ai_out.clone());
        tokio::task::spawn_blocking(move || {
            let done = |error: Option<String>| Msg::AiDone { id, error }.to_json();
            let wanted = || !stop.load(Ordering::Relaxed) && !out.is_closed();
            let started = match hub.ai_start(member, &ask, &lang) {
                Ok(s) => s,
                Err(why) => {
                    if wanted() {
                        let _ = out.send(done(Some(why)));
                    }
                    return;
                }
            };
            // pieces go a few a second, not one message per word: what came
            // meanwhile goes every PIECE_GAP, even while the model pauses
            let waiting = Mutex::new(String::new());
            let streaming = std::sync::atomic::AtomicBool::new(true);
            let send_waiting = || {
                let text = std::mem::take(&mut *waiting.locked());
                if !text.is_empty() && wanted() {
                    let _ = out.send(Msg::AiText { id, text }.to_json());
                }
            };
            let result = std::thread::scope(|s| {
                s.spawn(|| {
                    while streaming.load(Ordering::Relaxed) {
                        std::thread::sleep(ai::PIECE_GAP);
                        send_waiting();
                    }
                });
                let r = started.claude.stream(
                    &started.body,
                    &mut |piece| waiting.locked().push_str(piece),
                    &|| !wanted(),
                );
                streaming.store(false, Ordering::Relaxed);
                r
            });
            if !wanted() {
                return;
            }
            send_waiting();
            match result {
                Ok(u) => {
                    println!(
                        "AI: {} for member {}: {} tokens in ({} from the cache), {} out",
                        started.kind, member, u.input, u.cached, u.output
                    );
                    let _ = out.send(done(None));
                }
                Err(why) => {
                    eprintln!("AI: {} for member {} failed: {}", started.kind, member, why);
                    let _ = out.send(done(Some(why)));
                }
            }
            drop(started.ticket);
        });
    }

    /// The `integrations` answer for `member`.
    fn integrations(&self, hub: &Arc<Hub>, member: u64) -> String {
        let (hooks, commands) = hub.integrations(member);
        Msg::IntegrationList { hooks, commands }.to_json()
    }

    /// The `integrations` answer after a change to them, or why it was refused.
    fn integrations_after(&self, hub: &Arc<Hub>, member: u64, r: Result<(), String>) -> String {
        match r {
            Ok(()) => self.integrations(hub, member),
            Err(why) => denied(&why),
        }
    }

    /// What to answer a message from the app with, if anything.
    async fn answer(&mut self, msg: Option<Msg>) -> Option<String> {
        let (hub, member, conn) = (&self.hub, self.member, self.conn);
        match msg {
            Some(Msg::Push { .. }) if self.too_old => {
                // said once per connection; the app stays connected
                (!std::mem::replace(&mut self.told_too_old, true))
                    .then(|| denied(wb_net::UPDATE_NEEDED))
            }
            Some(Msg::Push { data }) => {
                let (hub2, device2) = (hub.clone(), self.device.clone());
                // merging can take a moment on a big workspace; a merge that
                // failed on the way is said, not taken as accepted
                let refused = match off_thread(move || Ok(hub2.push(member, &device2, data))).await
                {
                    Ok(refused) => refused,
                    Err(why) => return Some(denied(&why)),
                };
                // said once in a while, as an app may send again before the update reaches it
                let quiet = self
                    .told_put_back
                    .is_some_and(|t| t.elapsed() < Duration::from_secs(PUT_BACK_QUIET_SECS));
                (refused && !quiet).then(|| {
                    self.told_put_back = Some(std::time::Instant::now());
                    Msg::put_back(PUT_BACK).to_json()
                })
            }
            Some(Msg::Ping) => Some(Msg::Pong.to_json()),
            Some(Msg::Mine { data }) => hub.take_mine(member, &data, self.mine_parts),
            Some(Msg::Schedule { item }) => Some(match hub.schedule(member, item) {
                Ok(items) => Msg::Scheduled { items }.to_json(),
                Err(why) => denied(&why),
            }),
            Some(Msg::Unschedule { id }) => Some(
                Msg::Scheduled {
                    items: hub.unschedule(member, id),
                }
                .to_json(),
            ),
            Some(Msg::Integrations) => Some(self.integrations(hub, member)),
            Some(Msg::HookAdd { hook }) => {
                Some(self.integrations_after(hub, member, hub.add_hook(member, hook)))
            }
            Some(Msg::HookRemove { token }) => {
                Some(self.integrations_after(hub, member, hub.remove_hook(member, &token)))
            }
            Some(Msg::CommandAdd { command }) => {
                Some(self.integrations_after(hub, member, hub.add_command(member, command)))
            }
            Some(Msg::CommandRemove { name }) => {
                Some(self.integrations_after(hub, member, hub.remove_command(member, &name)))
            }
            Some(Msg::Command {
                channel,
                parent,
                name,
                text,
            }) => match hub.run_command(member, channel, parent, &name, &text) {
                Ok(()) => None,
                Err(why) => Some(denied(&why)),
            },
            Some(Msg::ScheduledList) => Some(
                Msg::Scheduled {
                    items: hub.scheduled_of(member),
                }
                .to_json(),
            ),
            Some(Msg::Typing {
                channel, thread, ..
            }) => {
                let soon = self.typed.is_some_and(|(c, th, t)| {
                    c == channel && th == thread && t.elapsed() < TYPING_GAP
                });
                if !soon {
                    self.typed = Some((channel, thread, std::time::Instant::now()));
                    hub.typing_in(member, channel, thread);
                }
                None
            }
            // it missed a revision: the whole copy again
            Some(Msg::Resync) => hub.latest(member).map(|(t, h, rev)| {
                self.sent(rev, h);
                t
            }),
            Some(Msg::PushToken {
                token,
                platform,
                env,
                workspace,
                prefs,
            }) => {
                hub.set_push_token(
                    &self.device,
                    &token,
                    &platform,
                    &env,
                    workspace.as_deref(),
                    prefs,
                );
                None
            }
            Some(Msg::CallJoin { channel }) => {
                Some(match hub.call(member, &self.device, channel) {
                    Ok(m) => m.to_json(),
                    Err(why) => Msg::call_denied(&why, channel).to_json(),
                })
            }
            Some(Msg::CreateLinkCode) => Some(hub.new_link(member, &self.device).to_json()),
            Some(Msg::Ai { id, ask, lang }) => {
                self.ask_ai(id, ask, lang);
                None
            }
            Some(Msg::AiStop { id }) => {
                if let Some(s) = self.ai_going.remove(&id) {
                    s.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                None
            }
            Some(Msg::SetPassword {
                email,
                password,
                old_password,
            }) => {
                let h = hub.clone();
                let d = self.device.clone();
                let r = off_thread(move || {
                    h.set_password(member, &d, &email, &password, old_password.as_deref())
                })
                .await;
                Some(match r {
                    Ok(m) => m.to_json(),
                    Err(why) => denied(&why),
                })
            }
            // reading a backup, merging it and saving it to disk takes a
            // moment: away from the connections
            Some(m @ Msg::Restore { .. }) => {
                let h = hub.clone();
                let r = off_thread(move || match h.request(member, conn, m) {
                    Reply::To(r) => Ok(r.to_json()),
                    Reply::Denied(why) => Err(why),
                })
                .await;
                Some(r.unwrap_or_else(|why| denied(&why)))
            }
            Some(
                m @ (Msg::CreateInvite { .. }
                | Msg::SetRole { .. }
                | Msg::Revoke { .. }
                | Msg::AccessList
                | Msg::BackupList),
            ) => match hub.request(member, conn, m) {
                Reply::To(Msg::Invite {
                    member: who, code, ..
                }) => {
                    // email it when the server can (sending takes a moment)
                    let (h, c, at) = (hub.clone(), code.clone(), self.host.clone());
                    let (emailed, mail_error) = tokio::task::spawn_blocking(move || {
                        h.email_invite(member, who, &c, at.as_deref())
                    })
                    .await
                    .unwrap_or((None, None));
                    Some(
                        Msg::Invite {
                            member: who,
                            code,
                            emailed,
                            mail_error,
                        }
                        .to_json(),
                    )
                }
                Reply::To(r) => Some(r.to_json()),
                Reply::Denied(why) => Some(denied(&why)),
            },
            _ => None,
        }
    }
}

pub(crate) async fn connection(mut socket: WebSocket, hub: Arc<Hub>, host: Option<String>) {
    hub.connections.fetch_add(1, Ordering::Relaxed);
    let _count = Counted(hub.clone());
    let mut stop = hub.stop.clone();
    let Some(SignedIn {
        device,
        member,
        role,
        tag,
        token,
        app,
        sync,
        deltas,
        watch,
        have,
        mine_parts,
    }) = hello(&mut socket, &hub, &mut stop).await
    else {
        return;
    };
    let mut closed = hub.closed.subscribe();
    if *closed.borrow_and_update() {
        let _ = send(&mut socket, &Msg::gone().to_json()).await;
        return;
    }
    if watch {
        let _ = (role, tag, token, app, sync, deltas, have, mine_parts);
        return watching(socket, hub, device, member, stop, closed).await;
    }
    let _online = Online::new(&hub, &device, member);
    // an app too old to know every field would erase the ones it doesn't
    // know when its copy wins: it gets the team's copy but sends no changes
    let too_old = sync.is_none_or(|v| v < wb_net::OLDEST_SYNC);
    if too_old {
        eprintln!(
            "device {} runs an old app ({}): taking no changes from it",
            device,
            app.as_deref().unwrap_or("no version")
        );
    }
    // signed in: from now on a message may hold a whole workspace. The app
    // sends nothing between hello and welcome, so nothing read ahead is lost.
    let mut socket = WebSocketStream::from_raw_socket(
        socket.into_inner(),
        WsRole::Server,
        Some(ws_config(wb_net::MAX_MESSAGE)),
    )
    .await;
    // someone invited is in now (the welcome below already says so)
    hub.joined(member);
    // listen before sending the copy so no update falls in between
    let mut updates = hub.updates.subscribe();
    let mut kicks = hub.kicks.subscribe();
    // a Revoke between signing in and listening for kicks isn't heard: look
    if !hub.device_is(&device, member) {
        let _ = send(&mut socket, &access_removed()).await;
        return;
    }
    let mut access = hub.access.subscribe();
    let mut presence = hub.presence.subscribe();
    let mut typing = hub.typing.subscribe();
    let mut mines = hub.mines.subscribe();
    // this connection's requests don't bring it the access broadcast too
    let conn = NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed);
    // a reconnect to this run of the server: only what changed since the
    // copy its last connection sent, when the server can say
    let before = have
        .filter(|h| deltas && h.epoch == hub.epoch)
        .and_then(|h| Some((h.rev, hub.seen_at(&device, member, h.rev)?)));
    let seen = hub.seen_for(&device, member);
    let caught =
        before.and_then(|(rev, was)| hub.catch_up(tag, member, role, token.clone(), rev, &was));
    let (welcome, was_hidden, rev) = match caught {
        Some(w) => w,
        None => hub.welcome(tag, member, role, token),
    };
    if !send(&mut socket, &welcome).await {
        return;
    }
    if role.manages() && !send(&mut socket, &hub.access_for(member, false).to_json()).await {
        return;
    }
    let online = presence.borrow_and_update().clone();
    if !send(&mut socket, &Msg::Presence { online }.to_json()).await {
        return;
    }
    // this person's own reading state (also: this server keeps it)
    if !send(&mut socket, &hub.mine_for(member)).await {
        return;
    }
    // the AI tools, when this server has them
    if let Some(model) = hub.ai_model() {
        if !send(&mut socket, &Msg::AiReady { model }.to_json()).await {
            return;
        }
    }
    let (ai_out, mut ai_answers) = tokio::sync::mpsc::unbounded_channel::<String>();
    let mut me = Conn {
        hub: hub.clone(),
        member,
        device: device.clone(),
        conn,
        host,
        too_old,
        told_too_old: false,
        told_put_back: None,
        was_hidden: Arc::new(vec![]),
        seen,
        mine_parts,
        typed: None,
        ai_out,
        ai_going: Default::default(),
    };
    me.sent(rev, was_hidden);
    let silent = Duration::from_secs(wb_net::SILENT_SECS + 15);
    loop {
        tokio::select! {
            // in this order when several are ready: a new role (Access)
            // reaches the app before the copy it changes, as the app goes
            // by its role for what it may see
            biased;
            _ = stop.changed() => break,
            _ = closed.changed() => {
                let _ = send(&mut socket, &Msg::gone().to_json()).await;
                break;
            }
            k = kicks.recv() => {
                let out = match k {
                    Ok(list) => list.contains(&device),
                    // fell behind: whether this device is still signed in says it
                    Err(broadcast::error::RecvError::Lagged(_)) => !hub.device_is(&device, member),
                    Err(broadcast::error::RecvError::Closed) => false,
                };
                if out {
                    let _ = send(&mut socket, &access_removed()).await;
                    break;
                }
            }
            a = access.recv() => {
                // fell behind: the list now says it all the same
                let tell = match a {
                    Ok(from) => from != conn,
                    Err(broadcast::error::RecvError::Lagged(_)) => true,
                    Err(broadcast::error::RecvError::Closed) => false,
                };
                if tell && !send(&mut socket, &hub.access_for(member, false).to_json()).await {
                    break;
                }
            }
            p = presence.changed() => {
                if p.is_err() {
                    break;
                }
                let online = presence.borrow_and_update().clone();
                if !send(&mut socket, &Msg::Presence { online }.to_json()).await {
                    break;
                }
            }
            t = typing.recv() => {
                // fell behind: typing that old doesn't matter any more
                let t = match t {
                    Ok(t) => t,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if t.for_member(member) {
                    let m = Msg::Typing { channel: t.channel, member: Some(t.member), thread: t.thread };
                    if !send(&mut socket, &m.to_json()).await {
                        break;
                    }
                }
            }
            m = mines.recv() => {
                let data = match m {
                    // only what changed, to apps that take that
                    Ok((who, _, Some(part))) if who == member && mine_parts => {
                        if !send(&mut socket, &mine::mine_part_json(&part)).await {
                            break;
                        }
                        continue;
                    }
                    Ok((who, data, _)) if who == member => data,
                    Ok(_) => continue,
                    // fell behind: what the server keeps now says it all
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if !send(&mut socket, &hub.mine_for(member)).await {
                            break;
                        }
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if !send(&mut socket, &mine::mine_json(&data)).await {
                    break;
                }
            }
            Some(a) = ai_answers.recv() => {
                if !send(&mut socket, &a).await {
                    break;
                }
            }
            m = tokio::time::timeout(silent, socket.next()) => {
                let m = match m {
                    Err(_) => break, // heard nothing for too long
                    Ok(None) | Ok(Some(Err(_))) => break,
                    Ok(Some(Ok(m))) => m,
                };
                let Message::Text(t) = m else {
                    if matches!(m, Message::Close(_)) { break } else { continue }
                };
                if let Some(o) = me.answer(Msg::parse(t.as_str())).await {
                    if !send(&mut socket, &o).await {
                        break;
                    }
                }
            }
            u = updates.recv() => {
                let (text, hidden, rev) = match u {
                    Ok(c) => {
                        let (text, hidden) = match deltas.then(|| c.delta_for(member, &me.was_hidden)).flatten() {
                            Some(d) => d,
                            None => c.update_for(member),
                        };
                        (text, hidden, c.rev)
                    }
                    // fell behind: the newest copy says it all
                    Err(broadcast::error::RecvError::Lagged(_)) => match hub.latest(member) { Some(t) => t, None => continue },
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                me.sent(rev, hidden);
                if !send(&mut socket, &text).await {
                    break;
                }
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}

/// What a watch connection sends for one notification (push::payload's shape).
pub(crate) fn note_msg(p: &Value) -> Msg {
    let alert = &p["aps"]["alert"];
    Msg::Note {
        title: alert["title"].as_str().unwrap_or("Workboard").to_string(),
        body: alert["body"].as_str().unwrap_or_default().to_string(),
        channel: p["channel"].as_u64(),
        parent: p["parent"].as_u64(),
    }
}

/// A device's watch connection, counted in Hub::watching while it lasts.
struct Watched(Arc<Hub>, String);

impl Watched {
    fn new(hub: &Arc<Hub>, device: &str) -> Watched {
        *hub.watching.locked().entry(device.to_string()).or_insert(0) += 1;
        Watched(hub.clone(), device.to_string())
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        let mut w = self.0.watching.locked();
        if let Some(n) = w.get_mut(&self.1) {
            *n -= 1;
            if *n == 0 {
                w.remove(&self.1);
            }
        }
    }
}

/// A watch connection (hello with `watch`): a computer where this workspace
/// isn't open hears of `member`'s new messages, the ones an iPhone gets a
/// push for, and nothing else. It isn't counted online (no green dot, and
/// the person's iPhone still gets its pushes) and takes no changes; it only
/// answers ping and keeps the Mac's push token (`push_token`), which gets no
/// pushes while the connection lasts.
async fn watching(
    mut socket: WebSocket,
    hub: Arc<Hub>,
    device: String,
    member: u64,
    mut stop: watch::Receiver<bool>,
    mut closed: watch::Receiver<bool>,
) {
    let mut notes = hub.notes.subscribe();
    let mut kicks = hub.kicks.subscribe();
    if !hub.device_is(&device, member) {
        let _ = send(&mut socket, &access_removed()).await;
        return;
    }
    if !send(&mut socket, &Msg::Watching.to_json()).await {
        return;
    }
    let _watched = Watched::new(&hub, &device);
    let silent = Duration::from_secs(wb_net::SILENT_SECS + 15);
    'conn: loop {
        tokio::select! {
            biased;
            _ = stop.changed() => break,
            _ = closed.changed() => {
                let _ = send(&mut socket, &Msg::gone().to_json()).await;
                break;
            }
            k = kicks.recv() => {
                let out = match k {
                    Ok(list) => list.contains(&device),
                    Err(broadcast::error::RecvError::Lagged(_)) => !hub.device_is(&device, member),
                    Err(broadcast::error::RecvError::Closed) => false,
                };
                if out {
                    let _ = send(&mut socket, &access_removed()).await;
                    break;
                }
            }
            n = notes.recv() => {
                let list = match n {
                    Ok(l) => l,
                    // fell behind: those are missed, the next ones still come
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                // what an iPhone without choices gets: DMs, mentions, threads
                let now = now_ms() / 1000;
                let mine: Vec<&push::Note> = list
                    .iter()
                    .filter(|n| n.member == member && push::wanted(n, None, now))
                    .collect();
                for p in push::batch(&mine) {
                    if !send(&mut socket, &note_msg(&p).to_json()).await {
                        break 'conn;
                    }
                }
            }
            m = tokio::time::timeout(silent, socket.next()) => {
                let m = match m {
                    Err(_) | Ok(None) | Ok(Some(Err(_))) => break,
                    Ok(Some(Ok(m))) => m,
                };
                match m {
                    Message::Close(_) => break,
                    Message::Text(t) => match Msg::parse(t.as_str()) {
                        Some(Msg::Ping) => {
                            if !send(&mut socket, &Msg::Pong.to_json()).await {
                                break;
                            }
                        }
                        // without choices (the Mac doesn't have this
                        // workspace's at hand): the ones it sent when the
                        // workspace was open stay
                        Some(Msg::PushToken {
                            token,
                            platform,
                            env,
                            workspace,
                            prefs,
                        }) => {
                            let prefs = prefs.or_else(|| hub.push_prefs(&device, &token));
                            hub.set_push_token(
                                &device,
                                &token,
                                &platform,
                                &env,
                                workspace.as_deref(),
                                prefs,
                            );
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}
