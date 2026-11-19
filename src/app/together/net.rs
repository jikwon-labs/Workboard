// Staying connected to a workboard-server: connecting and signing in,
// handling what it sends, joining a team, and making a workspace on it.

use super::*;

/// The team server to look for app updates on, chosen at start so updates
/// arrive whichever workspace is open, even one kept only on this computer:
/// the open workspace's server, else the one last signed in to, else the
/// first other workspace on a server.
pub(crate) fn update_server(open: &Data) -> Option<String> {
    let others = Data::workspaces()
        .into_iter()
        .map(|w| Data::load_file(&w.file).and_then(|d| d.extra.shared.and_then(|c| c.server)));
    pick_update_server(
        open.extra.shared.as_ref().and_then(|c| c.server.clone()),
        &super::login::last_server(),
        others,
    )
}

pub(crate) fn pick_update_server(
    open: Option<String>,
    last: &str,
    others: impl IntoIterator<Item = Option<String>>,
) -> Option<String> {
    let real = |s: &String| !s.trim().is_empty();
    open.filter(real)
        .or_else(|| Some(last.to_string()).filter(real))
        .or_else(|| others.into_iter().flatten().find(real))
}

impl App {
    /// The team server a new workspace can be made on: the one this
    /// workspace is on, when it is the server's first workspace and this
    /// person is its owner or an admin, signed in with an account (as the
    /// server allows).
    pub(crate) fn server_for_new_workspace(&self) -> Option<String> {
        let cfg = self.data.extra.shared.as_ref()?;
        let addr = cfg.server.clone()?;
        self.sync_rt.account.as_ref()?;
        if !self.sync_rt.role?.manages() || addr.contains("/w/") {
            return None;
        }
        Some(addr)
    }

    /// Make a workspace on the team server this one is on; it opens once
    /// the server has made it (making_ws_tick).
    pub(crate) fn make_workspace_on_server(
        &mut self,
        name: &str,
        apps: &[String],
        invite: &[String],
        role: &str,
        ctx: Option<egui::Context>,
    ) {
        let (Some(addr), Some(cfg), Some(email)) = (
            self.server_for_new_workspace(),
            self.data.extra.shared.clone(),
            self.sync_rt.account.clone(),
        ) else {
            return;
        };
        let Some(token) = load_token(&addr, &cfg.device) else {
            self.toast(tr(
                "Sign in to the team server again, then make the workspace.",
            ));
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let auth = wb_net::Auth {
            device: cfg.device.clone(),
            token,
        };
        std::thread::spawn(move || {
            let _ = tx.send(wb_net::create_workspace(&addr, &auth));
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        self.making_ws = Some(MakingWs {
            rx,
            name: name.to_string(),
            apps: apps.to_vec(),
            invite: invite.to_vec(),
            role: role.to_string(),
            email,
        });
        self.toast(tr("Making the workspace on the team server..."));
    }

    /// The server answered a make_workspace_on_server: open the new
    /// workspace on this computer and set it up on the server as its owner.
    pub(crate) fn making_ws_tick(&mut self) {
        let Some(m) = &self.making_ws else {
            return;
        };
        let answer = match m.rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err("The server had a problem. Try again.".into())
            }
        };
        let m = self.making_ws.take().expect("checked above");
        let addr = match answer {
            Ok(a) => a,
            Err(e) => {
                self.toast(trf!("Couldn't make the workspace: {}", server_text(&e)));
                return;
            }
        };
        let before = self.ws_file.clone();
        self.create_workspace(&m.name, &m.apps, &[], &m.role);
        if self.ws_file == before {
            return;
        }
        // the owner signs in with the account they have on the server
        if let Some(me) = self.data.members.iter_mut().find(|x| x.id == self.data.me) {
            me.email = m.email.clone();
        }
        let how = How::Claim {
            email: m.email,
            password: String::new(),
        };
        if let Err(e) = self.connect_with(&addr, how) {
            self.toast(server_text(&e));
            return;
        }
        self.invite_people(&m.invite, &m.role);
        self.save_local();
    }

    /// What the top bar says about a team server that isn't working: can't
    /// be reached, or won't let this computer in. None while connected,
    /// connecting, or without a server.
    /// Whether this workspace works through a team server.
    pub(crate) fn on_server(&self) -> bool {
        self.data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.is_some())
    }

    /// Whether this computer is connected to the team server right now.
    pub(crate) fn connected(&self) -> bool {
        self.on_server() && self.sync_rt.net_status == Some(NetStatus::Connected)
    }

    /// Whether `member` has Workboard open and connected somewhere: None
    /// when that isn't known (no server, not connected, or a server too old
    /// to say).
    pub(crate) fn is_online(&self, member: u64) -> Option<bool> {
        if !self.connected() {
            return None;
        }
        if member == self.data.me {
            return Some(true);
        }
        self.sync_rt.online.as_ref().map(|o| o.contains(&member))
    }

    pub(crate) fn server_trouble(&self) -> Option<String> {
        self.data.extra.shared.as_ref()?.server.as_ref()?;
        self.sync_rt.net.as_ref()?;
        match &self.sync_rt.net_status {
            Some(NetStatus::Reconnecting(_)) if self.sync_rt.fails >= FAILS_BEFORE_CANT_REACH => {
                Some(tr("Not connected").into())
            }
            Some(NetStatus::Reconnecting(_)) => Some(tr("Reconnecting\u{2026}").into()),
            Some(NetStatus::Refused(_)) => Some(tr("Not signed in").into()),
            Some(NetStatus::Gone(_)) => Some(tr("Deleted from the server").into()),
            _ => None,
        }
    }

    /// The server connection: start it when needed and take in what arrived.
    /// Returns true when the team's changes came in.
    pub(crate) fn net_tick(&mut self, ctx: Option<&egui::Context>) -> bool {
        self.listing_tick();
        if self.sync_rt.net.is_none() {
            let (addr, mut sign_in) = if let Some(j) = &self.sync_rt.joining {
                (j.addr.clone(), j.how.sign_in(&j.device, self.data.me))
            } else if let Some(SharedCfg {
                server: Some(addr),
                device,
                tag,
                ..
            }) = &self.data.extra.shared
            {
                let mut si = match &self.sync_rt.resign {
                    Some(how) => how.sign_in(device, self.data.me),
                    None => SignIn {
                        device: device.clone(),
                        token: load_token(addr, device),
                        ..Default::default()
                    },
                };
                si.tag = Some(*tag);
                (addr.clone(), si)
            } else {
                return false;
            };
            // the server takes changes only from apps that know every field
            sign_in.app = Some(APP_VERSION.to_string());
            sign_in.sync = Some(sync::META_V);
            let wake: wb_net::Wake = match ctx {
                Some(c) => {
                    let c = c.clone();
                    std::sync::Arc::new(move || c.request_repaint())
                }
                None => std::sync::Arc::new(|| {}),
            };
            self.sync_rt.net_status = None;
            self.sync_rt.fails = 0;
            self.sync_rt.net = Some(wb_net::Client::start(&addr, sign_in, wake));
        }
        let events = self
            .sync_rt
            .net
            .as_ref()
            .map(|n| n.events())
            .unwrap_or_default();
        self.take_net_events(events)
    }

    /// What the server connection said since the last frame. Teammates'
    /// changes that came one after another are merged in together: each
    /// merge goes over the whole workspace, so a busy channel's messages
    /// arriving in one frame cost one merge instead of one each. They are
    /// all merged (a change carries only what changed, not the whole copy),
    /// with the last hidden list, which is the whole list as of then.
    /// Anything else the server said is taken after the changes before it.
    /// True when something new arrived.
    pub(crate) fn take_net_events(&mut self, events: Vec<NetEvent>) -> bool {
        let mut changed = false;
        let mut updates = vec![];
        let mut hidden_now = vec![];
        for e in events {
            let e = match e {
                NetEvent::Update { data, hidden, .. } => {
                    updates.push(data);
                    hidden_now = hidden;
                    continue;
                }
                e => e,
            };
            if !updates.is_empty() {
                changed |= self.merge_in(&std::mem::take(&mut updates), &hidden_now);
            }
            match e {
                NetEvent::Status(s) => {
                    match &s {
                        NetStatus::Reconnecting(_) => self.sync_rt.fails += 1,
                        NetStatus::Connected => self.sync_rt.fails = 0,
                        _ => {}
                    }
                    if s != NetStatus::Connected {
                        // the list comes again after the next welcome
                        self.sync_rt.online = None;
                    }
                    if matches!(s, NetStatus::Gone(_)) && self.sync_rt.joining.is_none() {
                        self.toast(tr("This workspace was deleted from the server by its owner. Remove it from this computer in Work together."));
                    }
                    self.sync_rt.net_status = Some(s)
                }
                NetEvent::Welcome {
                    tag,
                    snapshot,
                    member,
                    role,
                    token,
                    email,
                    hidden,
                    ..
                } => {
                    if let Some(t) = token {
                        let (addr, device) = match (&self.sync_rt.joining, &self.data.extra.shared)
                        {
                            (Some(j), _) => (j.addr.clone(), j.device.clone()),
                            (None, Some(c)) => {
                                (c.server.clone().unwrap_or_default(), c.device.clone())
                            }
                            _ => continue,
                        };
                        save_token(&addr, &device, &t);
                    }
                    if self.sync_rt.resign.take().is_some() {
                        self.toast(tr("Signed in again."));
                    }
                    self.sync_rt.account = email;
                    self.sync_rt.form = Form::default();
                    self.sync_rt.role = Some(role);
                    // the server that let us in also publishes app updates
                    let server = self
                        .sync_rt
                        .joining
                        .as_ref()
                        .map(|j| j.addr.clone())
                        .or_else(|| {
                            self.data
                                .extra
                                .shared
                                .as_ref()
                                .and_then(|c| c.server.clone())
                        });
                    if let Some(addr) = server {
                        crate::updater::set_server(&addr);
                    }
                    changed |= self.net_welcome(tag, snapshot, member, &hidden);
                    // what I scheduled (servers from before scheduling don't answer)
                    if let Some(net) = &self.sync_rt.net {
                        net.send(wb_net::Msg::ScheduledList);
                        net.send(wb_net::Msg::Integrations);
                    }
                }
                NetEvent::Update { .. } => unreachable!("taken in above"),
                NetEvent::Invite {
                    member,
                    code,
                    emailed,
                    mail_error,
                } => {
                    if let Some(p) = self.sync_rt.people.iter_mut().find(|p| p.member == member) {
                        p.invite = Some(code);
                    }
                    let name = self
                        .data
                        .member(member)
                        .map(|m| m.shown().to_string())
                        .unwrap_or_default();
                    if let Some(to) = emailed {
                        self.toast(crate::i18n::particles(&trf!(
                            "Emailed {}'s invite code to {}.",
                            name,
                            to
                        )));
                    } else if let Some(why) = mail_error {
                        self.toast(trf!("Couldn't email {}'s invite code ({}). Copy it from Work together > People.", name, why));
                    }
                }
                NetEvent::LinkCode { code, seconds } => self.got_phone_link(code, seconds),
                NetEvent::Account(email) => {
                    self.toast(crate::i18n::particles(&trf!(
                        "Password saved. Sign in with {} on your other computers and phones.",
                        email
                    )));
                    self.sync_rt.account = Some(email);
                    self.sync_rt.acct = Form::default();
                }
                NetEvent::Access { people, role } => {
                    self.sync_rt.people = people;
                    self.sync_rt.role = Some(role);
                    self.send_auto_invites();
                }
                NetEvent::Denied(why) => {
                    self.huddle_denied_unnamed();
                    self.toast(server_text(&why));
                }
                NetEvent::PutBack(why) => {
                    // the team's version of what was put back comes with the next update
                    self.toast(server_text(&why));
                }
                NetEvent::CallDenied { channel, why } => {
                    self.huddle_denied(channel);
                    self.toast(server_text(&why));
                }
                NetEvent::CallToken {
                    channel,
                    url,
                    token,
                } => self.huddle_token(channel, url, token),
                NetEvent::Backups(list) => self.sync_rt.backups = Some(list),
                NetEvent::Presence(online) => self.sync_rt.online = Some(online),
                NetEvent::Scheduled(items) => self.sync_rt.scheduled = Some(items),
                NetEvent::Integrations { hooks, commands } => {
                    self.sync_rt.integrations = Some((hooks, commands))
                }
                NetEvent::Typing { channel, member } => self.heard_typing(channel, member),
                NetEvent::Restored(name) => {
                    let label = wb_net::Backup { name, bytes: 0 }.label();
                    self.toast(trf!(
                        "Restored the team's workspace to {}. Everyone gets it now.",
                        label
                    ));
                    self.sync_rt.restore_confirm = None;
                    // the copy from before the restore is a new backup
                    self.access_request(NetMsg::BackupList);
                }
            }
        }
        if !updates.is_empty() {
            changed |= self.merge_in(&updates, &hidden_now);
        }
        changed
    }

    /// Joined a server or set one up: the sign-in screen and the dialog
    /// that asked are done, and the server is filled in next time.
    fn signed_in_now(&mut self) {
        self.login = None;
        if self.data.extra.welcome {
            self.data.extra.welcome = false;
            self.save_local();
        }
        if matches!(
            self.modal,
            Some(Modal::Together { .. } | Modal::JoinTeam { .. })
        ) {
            self.modal = None;
        }
        if self.persist {
            if let Some(addr) = self
                .data
                .extra
                .shared
                .as_ref()
                .and_then(|c| c.server.clone())
            {
                super::login::set_last_server(&addr);
            }
        }
    }

    /// The server answered: finish joining, or catch up after a reconnect.
    pub(super) fn net_welcome(
        &mut self,
        tag: u16,
        snapshot: Option<serde_json::Value>,
        member: u64,
        hidden: &[u64],
    ) -> bool {
        if let Some(j) = self.sync_rt.joining.take() {
            let cfg = SharedCfg {
                folder: String::new(),
                device: j.device,
                tag,
                server: Some(j.addr),
            };
            match snapshot {
                None if self.data.extra.shared.is_some() => {
                    // an empty server, but this workspace already goes with
                    // a team: it stays with that one (setting up a server
                    // starts from a workspace that is only on this computer)
                    self.sync_rt.net = None;
                    self.sync_rt.net_status = None;
                    self.sync_rt.role = None;
                    self.toast(tr("That server has no team yet. To set it up, open a workspace that's only on this computer and connect it from Work together."));
                }
                None => {
                    // an empty server: this workspace becomes the team's
                    self.data.extra.shared = Some(cfg);
                    self.data.extra.sync = Default::default();
                    self.sync_note_change();
                    self.write_shared();
                    self.save_local();
                    self.share_cached_files();
                    self.toast(tr(
                        "Connected. This workspace is now on the server for your team.",
                    ));
                    self.signed_in_now();
                }
                Some(base) => match self.join_from(base, cfg) {
                    Ok(()) => {
                        // what the workspace left knew about its own server
                        // (its people, backups, ...) doesn't come along; the
                        // connection that just let us in does
                        let rt = std::mem::take(&mut self.sync_rt);
                        self.sync_rt = SyncRt {
                            net: rt.net,
                            net_status: rt.net_status,
                            role: rt.role,
                            account: rt.account,
                            ..Default::default()
                        };
                        self.after_join();
                        // the invite code says who this is
                        self.become_member(Some(member), "", "");
                        self.sync_note_change();
                        self.write_shared();
                        self.save_local();
                        let name = self
                            .data
                            .member(member)
                            .map(|m| m.shown().to_string())
                            .unwrap_or_default();
                        self.toast(crate::i18n::particles(&trf!("Joined as {}.", name)));
                        self.signed_in_now();
                    }
                    Err(e) => {
                        self.sync_rt.net = None;
                        self.toast(e);
                    }
                },
            }
            return true;
        }
        if let Some(cfg) = self.data.extra.shared.as_mut() {
            if cfg.tag != tag {
                cfg.tag = tag;
                self.save_local();
            }
        }
        if self.data.me != member && self.data.member(member).is_some() {
            // signed in again with an invite code for someone else
            self.data.me = member;
            self.save_local();
        }
        let changed = match snapshot {
            Some(s) => self.merge_in(&[s], hidden),
            None => false,
        };
        // after (re)connecting our whole copy goes up, with anything made offline
        self.write_shared();
        changed
    }

    #[cfg(test)]
    /// Start connecting to a server: with an invite code, or with none to
    /// set up an empty server.
    pub(crate) fn connect_server(&mut self, addr: &str, invite: &str) -> Result<(), String> {
        let how = if invite.trim().is_empty() {
            How::Claim {
                email: String::new(),
                password: String::new(),
            }
        } else {
            How::Invite {
                code: invite.to_string(),
                email: String::new(),
                password: String::new(),
            }
        };
        self.connect_with(addr, how)
    }

    /// Start connecting to a server from the dialog (shares this workspace
    /// there if the server is empty, else joins the team's workspace).
    pub(crate) fn connect_with(&mut self, addr: &str, how: How) -> Result<(), String> {
        wb_net::server_url(addr)?;
        // email and password: the account may be in any of the server's
        // workspaces, or in several (listing_tick goes on from there)
        if let How::Account { email, password } = &how {
            let (tx, rx) = std::sync::mpsc::channel();
            let (a, e, p) = (addr.trim().to_string(), email.clone(), password.clone());
            std::thread::spawn(move || {
                let _ = tx.send(wb_net::list_workspaces(&a, &e, &p));
            });
            self.sync_rt.choices = None;
            self.sync_rt.listing = Some(Listing {
                rx,
                addr: addr.trim().to_string(),
                how,
            });
            return Ok(());
        }
        self.start_joining(addr, how);
        Ok(())
    }

    /// The server said which workspaces the account signs in to: join the
    /// one, or let the person choose.
    pub(crate) fn listing_tick(&mut self) {
        if let Some(file) = self.sync_rt.open_local.take() {
            // the sign-in screen goes: this workspace is signed in already
            self.login = None;
            if self.data.extra.welcome {
                self.data.extra.welcome = false;
                self.save_local();
            }
            if matches!(self.modal, Some(Modal::JoinTeam { .. })) {
                self.modal = None;
            }
            self.switch_workspace(&file);
            return;
        }
        let Some(l) = &self.sync_rt.listing else {
            return;
        };
        let answer = match l.rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err("The server had a problem. Try again.".into())
            }
        };
        let how = self.sync_rt.listing.take().expect("checked above").how;
        match answer {
            Err(e) => self.sync_rt.form.note = e,
            Ok(list) if list.is_empty() => {
                self.sync_rt.form.note =
                    "That email and password don't match an account on this server.".into()
            }
            Ok(list) if list.len() == 1 => self.open_or_join(&list[0].0, how),
            Ok(list) => self.sync_rt.choices = Some((list, how)),
        }
    }

    /// Join the team workspace at `addr`, or open it when it is already on
    /// this computer.
    pub(crate) fn open_or_join(&mut self, addr: &str, how: How) {
        self.sync_rt.choices = None;
        let on_it = self
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.as_deref())
            .is_some_and(|a| wb_net::same_workspace(a, addr));
        if !on_it {
            if let Some(file) = local_workspace_on(addr, &self.ws_file) {
                // opened from listing_tick, outside the sign-in screen's drawing
                self.sync_rt.open_local = Some(file);
                return;
            }
        }
        self.start_joining(addr, how);
    }

    fn start_joining(&mut self, addr: &str, how: How) {
        if let How::Claim { email, .. } = &how {
            // the owner's email goes up with the workspace
            let me = self.data.me;
            if let Some(m) = self
                .data
                .members
                .iter_mut()
                .find(|m| m.id == me && m.email.trim().is_empty() && !email.trim().is_empty())
            {
                m.email = email.trim().to_string();
                self.dirty = true;
            }
        }
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
        self.sync_rt.joining = Some(Joining {
            addr: addr.trim().to_string(),
            device: format!("{:08x}", sync::random_u64() as u32),
            how,
        });
    }

    #[cfg(test)]
    /// Sign this computer in again with a new invite code.
    pub(crate) fn sign_in_again(&mut self, code: &str) {
        if code.trim().is_empty() {
            return;
        }
        self.sign_in_with(How::Invite {
            code: code.to_string(),
            email: String::new(),
            password: String::new(),
        });
    }

    /// Sign this computer in again: email and password, a code, a reset.
    pub(crate) fn sign_in_with(&mut self, how: How) {
        self.sync_rt.resign = Some(how);
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
    }

    /// Ask for the codes of people just invited, once the server knows them.
    fn send_auto_invites(&mut self) {
        if self.sync_rt.auto_invite.is_empty() || !self.sync_rt.role.is_some_and(|r| r.manages()) {
            return;
        }
        let known: Vec<u64> = self.sync_rt.people.iter().map(|p| p.member).collect();
        let (ready, wait): (Vec<_>, Vec<_>) = self
            .sync_rt
            .auto_invite
            .drain(..)
            .partition(|(m, _)| known.contains(m));
        self.sync_rt.auto_invite = wait;
        for (member, role) in ready {
            self.access_request(NetMsg::CreateInvite { member, role });
        }
    }

    /// Open what a workboard://join link (or a pasted invite) asks for.
    pub(crate) fn open_join_link(&mut self, text: &str) {
        let j = wb_net::parse_join(text);
        let form = &mut self.sync_rt.form;
        form.mode = Mode::Invite;
        form.code = j.code.clone();
        form.email = j.email.clone().unwrap_or_default();
        form.note.clear();
        let on_server = self.on_server();
        let addr = j.server.unwrap_or_default();
        if self.login.is_some() {
            // on the sign-in screen: it fills in the invite there
            self.open_login(super::login::LoginPage::Invite, &addr);
        } else if on_server && matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
            // this workspace's server let it go: the code is to sign back in
            self.modal = Some(self.together_modal());
        } else {
            // another team (or the same one as someone else): it opens as a
            // new workspace and this one stays as it is
            self.modal = Some(Modal::JoinTeam { addr });
        }
    }

    /// Open the form for joining a team from the workspace menu. Whatever
    /// this workspace is (on a server, in a folder or only here), the team's
    /// workspace opens as a new one beside it.
    pub(crate) fn open_join_team(&mut self, addr: String) {
        if self.sync_rt.joining.is_none() {
            self.sync_rt.form = Form {
                mode: Mode::Account,
                ..Default::default()
            };
        }
        self.modal = Some(Modal::JoinTeam { addr });
    }

    /// Send an access request to the server (owners and admins).
    pub(crate) fn access_request(&mut self, msg: NetMsg) {
        match &self.sync_rt.net {
            Some(n) if self.sync_rt.net_status == Some(NetStatus::Connected) => {
                if !matches!(msg, NetMsg::CallJoin { .. }) {
                    self.sync_rt.other_asked = Some(std::time::Instant::now());
                }
                n.send(msg)
            }
            _ => self.toast(tr("Not connected to the server right now.")),
        }
    }

    /// Keep this workspace on its server, reached at another address (the
    /// server moved, or the address was mistyped), and connect again. The
    /// sign-in goes along.
    pub(crate) fn change_server(&mut self, addr: &str) -> Result<(), String> {
        wb_net::server_url(addr)?;
        let new = addr.trim().to_string();
        let Some(cfg) = self.data.extra.shared.as_mut() else {
            return Ok(());
        };
        let Some(old) = cfg.server.clone() else {
            return Ok(());
        };
        if old != new {
            if let Some(token) = load_token(&old, &cfg.device) {
                save_token(&new, &cfg.device, &token);
                forget_token(&old, &cfg.device);
            }
            cfg.server = Some(new);
            self.save_local();
            self.dirty = true;
        }
        self.sync_rt.editing_addr = false;
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
        Ok(())
    }
}
