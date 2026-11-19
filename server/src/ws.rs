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
    /// `member` is typing in `channel`: tell the others who can see it.
    /// Nothing when the conversation isn't there or is closed to them.
    pub(crate) fn typing_in(&self, member: u64, channel: u64) {
        let t = {
            let g = self.state();
            let Some(ch) = g
                .canon
                .as_ref()
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
    let (device, app, sync, deltas, watch) = (
        hello.device.clone(),
        hello.app.clone(),
        hello.sync,
        !hello.full_copies,
        hello.watch,
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
    was_hidden: Vec<u64>,
    /// the conversation this connection's typing was last passed on for,
    /// and when (see TYPING_GAP)
    typed: Option<(u64, std::time::Instant)>,
}

impl Conn {
    /// The `integrations` answer for `member`.
    fn integrations(&self, hub: &Arc<Hub>, member: u64) -> String {
        let (hooks, commands) = hub.integrations(member);
        Msg::IntegrationList { hooks, commands }.to_json()
    }

    /// What to answer a message from the app with, if anything.
    async fn answer(&mut self, msg: Option<Msg>) -> Option<String> {
        let (hub, member, conn) = (&self.hub, self.member, self.conn);
        match msg {
            Some(Msg::Push { .. }) if self.too_old => {
                // said once per connection; the app stays connected
                (!std::mem::replace(&mut self.told_too_old, true))
                    .then(|| Msg::error(wb_net::UPDATE_NEEDED, Some(CODE_DENIED)).to_json())
            }
            Some(Msg::Push { data }) => {
                let (hub2, device2) = (hub.clone(), self.device.clone());
                // merging can take a moment on a big workspace
                let refused =
                    tokio::task::spawn_blocking(move || hub2.push(member, &device2, data))
                        .await
                        .unwrap_or(false);
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
            Some(Msg::Schedule { item }) => Some(match hub.schedule(member, item) {
                Ok(items) => Msg::Scheduled { items }.to_json(),
                Err(why) => Msg::error(&why, Some(CODE_DENIED)).to_json(),
            }),
            Some(Msg::Unschedule { id }) => Some(
                Msg::Scheduled {
                    items: hub.unschedule(member, id),
                }
                .to_json(),
            ),
            Some(Msg::Integrations) => Some(self.integrations(hub, member)),
            Some(Msg::HookAdd { hook }) => Some(match hub.add_hook(member, hook) {
                Ok(()) => self.integrations(hub, member),
                Err(why) => Msg::error(&why, Some(CODE_DENIED)).to_json(),
            }),
            Some(Msg::HookRemove { token }) => Some(match hub.remove_hook(member, &token) {
                Ok(()) => self.integrations(hub, member),
                Err(why) => Msg::error(&why, Some(CODE_DENIED)).to_json(),
            }),
            Some(Msg::CommandAdd { command }) => Some(match hub.add_command(member, command) {
                Ok(()) => self.integrations(hub, member),
                Err(why) => Msg::error(&why, Some(CODE_DENIED)).to_json(),
            }),
            Some(Msg::CommandRemove { name }) => Some(match hub.remove_command(member, &name) {
                Ok(()) => self.integrations(hub, member),
                Err(why) => Msg::error(&why, Some(CODE_DENIED)).to_json(),
            }),
            Some(Msg::Command {
                channel,
                parent,
                name,
                text,
            }) => match hub.run_command(member, channel, parent, &name, &text) {
                Ok(()) => None,
                Err(why) => Some(Msg::error(&why, Some(CODE_DENIED)).to_json()),
            },
            Some(Msg::ScheduledList) => Some(
                Msg::Scheduled {
                    items: hub.scheduled_of(member),
                }
                .to_json(),
            ),
            Some(Msg::Typing { channel, .. }) => {
                let soon = self
                    .typed
                    .is_some_and(|(c, t)| c == channel && t.elapsed() < TYPING_GAP);
                if !soon {
                    self.typed = Some((channel, std::time::Instant::now()));
                    hub.typing_in(member, channel);
                }
                None
            }
            // it missed a revision: the whole copy again
            Some(Msg::Resync) => hub.latest(member).map(|(t, h)| {
                self.was_hidden = h;
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
                    Err(why) => Msg::error(&why, Some(CODE_DENIED)).to_json(),
                })
            }
            Some(
                m @ (Msg::CreateInvite { .. }
                | Msg::SetRole { .. }
                | Msg::Revoke { .. }
                | Msg::AccessList
                | Msg::BackupList
                | Msg::Restore { .. }),
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
                Reply::Denied(why) => Some(Msg::error(&why, Some(CODE_DENIED)).to_json()),
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
        let _ = (role, tag, token, app, sync, deltas);
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
    // listen before sending the copy so no update falls in between
    let mut updates = hub.updates.subscribe();
    let mut kicks = hub.kicks.subscribe();
    // a Revoke between signing in and listening for kicks isn't heard: look
    if !hub.device_is(&device, member) {
        let _ = send(
            &mut socket,
            &Msg::error("Your access to this server was removed.", Some(CODE_AUTH)).to_json(),
        )
        .await;
        return;
    }
    let mut access = hub.access.subscribe();
    let mut presence = hub.presence.subscribe();
    let mut typing = hub.typing.subscribe();
    // this connection's requests don't bring it the access broadcast too
    let conn = NEXT_CONNECTION.fetch_add(1, Ordering::Relaxed);
    let (welcome, was_hidden) = hub.welcome(tag, member, role, token);
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
    let mut me = Conn {
        hub: hub.clone(),
        member,
        device: device.clone(),
        conn,
        host,
        too_old,
        told_too_old: false,
        told_put_back: None,
        was_hidden,
        typed: None,
    };
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
                    let _ = send(&mut socket, &Msg::error("Your access to this server was removed.", Some(CODE_AUTH)).to_json()).await;
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
                    let m = Msg::Typing { channel: t.channel, member: Some(t.member) };
                    if !send(&mut socket, &m.to_json()).await {
                        break;
                    }
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
                let (text, hidden) = match u {
                    Ok(c) => match deltas.then(|| c.delta_for(member, &me.was_hidden)).flatten() {
                        Some(d) => d,
                        None => c.update_for(member),
                    },
                    // fell behind: the newest copy says it all
                    Err(broadcast::error::RecvError::Lagged(_)) => match hub.latest(member) { Some(t) => t, None => continue },
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                me.was_hidden = hidden;
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

/// A watch connection (hello with `watch`): a computer where this workspace
/// isn't open hears of `member`'s new messages, the ones an iPhone gets a
/// push for, and nothing else. It isn't counted online (no green dot, and
/// the person's iPhone still gets its pushes) and takes no changes; it only
/// answers ping.
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
    let removed =
        || Msg::error("Your access to this server was removed.", Some(CODE_AUTH)).to_json();
    if !hub.device_is(&device, member) {
        let _ = send(&mut socket, &removed()).await;
        return;
    }
    if !send(&mut socket, &Msg::Watching.to_json()).await {
        return;
    }
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
                    let _ = send(&mut socket, &removed()).await;
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
                    Message::Text(t) if matches!(Msg::parse(t.as_str()), Some(Msg::Ping)) => {
                        if !send(&mut socket, &Msg::Pong.to_json()).await {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}
