// Working with other people: either a shared folder that a sync service
// keeps in step, or a workboard-server the app stays connected to (see
// crate::sync and wb_net). Signing in to a server: an account's email and
// password, an invite code (choosing a password at the same time), a code
// from a password-reset mail, or setting up an empty server. Also the
// first-start welcome, invite links (workboard://join), choosing who you are
// after joining a folder, calls
// that open in the browser with a Join button posted in chat, and a Markdown
// archive for documents.

use super::*;
use crate::sync::{self, SharedCfg};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use wb_net::{Event as NetEvent, Msg as NetMsg, Person, Role, SignIn, Status as NetStatus};

/// How often teammates' copies are looked at (folder), and how often ours is
/// written (folder) or sent (server).
const POLL_SECS: f64 = 2.0;
const WRITE_SECS: f64 = 1.5;

/// How this computer signs in to a server the first time, or again.
/// Empty text fields are left out of the hello.
#[derive(Clone, Debug, PartialEq)]
pub enum How {
    /// An invite code; with a password, the person's account is set up too
    /// (the email is the one on their member record unless one is typed).
    Invite { code: String, email: String, password: String },
    /// An account's email and password.
    Account { email: String, password: String },
    /// The code from a password-reset mail, with the new password.
    Reset { email: String, code: String, password: String },
    /// An empty server: this workspace goes up and I become the owner,
    /// with an account when an email and password are given.
    Claim { email: String, password: String },
}

impl How {
    /// The hello fields for signing in this way.
    pub(super) fn sign_in(&self, device: &str, me: u64) -> SignIn {
        let text = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        let pass = |s: &str| Some(s.to_string()).filter(|s| !s.is_empty());
        let mut si = SignIn { device: device.to_string(), ..Default::default() };
        match self {
            How::Invite { code, email, password } => {
                si.invite = text(code);
                si.email = text(email);
                si.new_password = pass(password);
            }
            How::Account { email, password } => {
                si.email = text(email);
                si.password = pass(password);
            }
            How::Reset { email, code, password } => {
                si.email = text(email);
                si.reset_code = text(code);
                si.new_password = pass(password);
            }
            How::Claim { email, password } => {
                si.claim = Some(me);
                si.email = text(email);
                si.new_password = pass(password);
            }
        }
        si
    }
}

/// Connecting to a server for the first time from the dialog.
pub struct Joining {
    pub addr: String,
    pub device: String,
    pub how: How,
}

/// Which sign-in fields the form shows.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Mode {
    #[default]
    Account,
    Invite,
    Forgot,
    NewServer,
}

/// What's typed into the sign-in and account fields.
#[derive(Default)]
pub struct Form {
    pub mode: Mode,
    pub email: String,
    pub password: String,
    pub repeat: String,
    /// An invite code, or the code from a reset mail.
    pub code: String,
    /// The current password, when changing it.
    pub old: String,
    /// A reset mail was asked for: show the code fields.
    pub sent: bool,
    /// "Change password" is open.
    pub changing: bool,
    /// What went wrong, or what happens next.
    pub note: String,
}

/// Why a new password won't do, if it won't.
pub(super) fn new_password_problem(password: &str, repeat: &str) -> Option<String> {
    if password.chars().count() < wb_net::MIN_PASSWORD {
        return Some(trf!("Choose a password of at least {} characters.", wb_net::MIN_PASSWORD));
    }
    (password != repeat).then(|| tr("The two passwords aren't the same.").to_string())
}

#[derive(Default)]
pub struct SyncRt {
    last_poll: f64,
    seen: Vec<(PathBuf, SystemTime)>,
    last_write: f64,
    pending: bool,
    /// The server connection, while this workspace uses a server.
    pub(super) net: Option<wb_net::Client>,
    pub(super) net_status: Option<NetStatus>,
    pub(super) joining: Option<Joining>,
    /// This computer's role on the server, once signed in.
    pub(super) role: Option<Role>,
    /// Everyone's access (owners and admins only).
    pub(super) people: Vec<Person>,
    /// Sign in again this way (after access was removed or the token stopped working).
    pub(super) resign: Option<How>,
    /// The email this person signs in with, once they have an account.
    pub(super) account: Option<String>,
    /// The sign-in fields.
    pub(super) form: Form,
    /// The account fields (setting up or changing a password).
    pub(super) acct: Form,
    /// People just invited on the server: ask for their codes (which the
    /// server emails) once the server has them.
    pub(super) auto_invite: Vec<(u64, Role)>,
    /// A "Forgot password" request on its way.
    pub(super) forgot: Option<std::sync::mpsc::Receiver<Result<bool, String>>>,
    /// The owner's view of the server's backups (None until asked for).
    pub(super) backups: Option<Vec<wb_net::Backup>>,
    /// The backup the owner clicked Restore on, waiting for "Yes, restore".
    pub(super) restore_confirm: Option<String>,
    /// The person was told the team's copy couldn't be read (said once).
    told_unreadable: bool,
    /// When a request other than joining a huddle last went to the server
    /// (see huddle_denied_unnamed).
    pub(super) other_asked: Option<std::time::Instant>,
}

// ---------------------------------------------------------------- tokens
//
// The token a server gives this computer lives in the macOS keychain (never
// in data.json). When trying the app with a separate WORKBOARD_HOME folder it
// lives in that folder instead, and tests keep it in memory.

const KEYCHAIN_SERVICE: &str = "Workboard server";

fn token_account(addr: &str, device: &str) -> String {
    format!("{}#{}", addr.trim(), device)
}

#[cfg(test)]
fn test_tokens() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static T: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> = std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

fn token_file() -> Option<PathBuf> {
    let separate = std::env::var("WORKBOARD_HOME").is_ok_and(|v| !v.is_empty());
    (separate || !cfg!(target_os = "macos")).then(|| Data::dir().join("server-tokens.json"))
}

fn read_token_file(p: &Path) -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn write_token_file(p: &Path, m: &std::collections::BTreeMap<String, String>) {
    let _ = std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
    if std::fs::write(p, serde_json::to_string_pretty(m).unwrap_or_default()).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
        }
    }
}

/// Goes up whenever a token is saved or forgotten, so a copy kept to save
/// reading the keychain every frame knows it's out of date.
static TOKEN_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(super) fn token_gen() -> u64 {
    TOKEN_GEN.load(std::sync::atomic::Ordering::SeqCst)
}

pub(super) fn load_token(addr: &str, device: &str) -> Option<String> {
    let key = token_account(addr, device);
    #[cfg(test)]
    return test_tokens().lock().unwrap().get(&key).cloned();
    #[cfg(not(test))]
    {
        if let Some(p) = token_file() {
            return read_token_file(&p).get(&key).cloned();
        }
        #[cfg(target_os = "macos")]
        return security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, &key).ok().and_then(|b| String::from_utf8(b).ok());
        #[allow(unreachable_code)]
        None
    }
}

pub(super) fn save_token(addr: &str, device: &str, token: &str) {
    let key = token_account(addr, device);
    TOKEN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    #[cfg(test)]
    {
        test_tokens().lock().unwrap().insert(key, token.to_string());
    }
    #[cfg(not(test))]
    {
        if let Some(p) = token_file() {
            let mut m = read_token_file(&p);
            m.insert(key, token.to_string());
            write_token_file(&p, &m);
            return;
        }
        #[cfg(target_os = "macos")]
        let _ = security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, &key, token.as_bytes());
    }
}

pub(super) fn forget_token(addr: &str, device: &str) {
    let key = token_account(addr, device);
    TOKEN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    #[cfg(test)]
    {
        test_tokens().lock().unwrap().remove(&key);
    }
    #[cfg(not(test))]
    {
        if let Some(p) = token_file() {
            let mut m = read_token_file(&p);
            m.remove(&key);
            write_token_file(&p, &m);
            return;
        }
        #[cfg(target_os = "macos")]
        let _ = security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, &key);
    }
}

/// How the server connection is doing, in words.
pub(super) fn net_status_text(net: bool, s: Option<&NetStatus>) -> String {
    match (net, s) {
        (false, _) => tr("Disconnected").into(),
        (true, None) | (true, Some(NetStatus::Connecting)) => tr("Connecting\u{2026}").into(),
        (true, Some(NetStatus::Connected)) => tr("Connected").into(),
        (true, Some(NetStatus::Reconnecting(why))) => trf!("Reconnecting\u{2026} {}", why),
        (true, Some(NetStatus::Refused(why))) => trf!("Not signed in. {}", why),
    }
}

/// What the shared folder holds, for the dialog.
pub(super) fn folder_people(folder: &str) -> usize {
    if folder.trim().is_empty() {
        return 0;
    }
    sync::other_files(Path::new(folder.trim()), "").len()
}

pub(super) fn call_link() -> String {
    format!("https://meet.jit.si/Workboard-{:016x}", sync::random_u64())
}

/// A workspace as the app wrote it before fields left at their default were
/// left out of the file (time entries' `member`, forms', whiteboards' and
/// dashboards' `owner`, dashboards' `kind`), for `sync::carry_hashes`.
pub(super) fn legacy_shape(v: &serde_json::Value) -> serde_json::Value {
    let mut v = v.clone();
    let fill = |items: Option<&mut serde_json::Value>, keys: &[(&str, serde_json::Value)]| {
        for it in items.and_then(|a| a.as_array_mut()).into_iter().flatten() {
            if let Some(o) = it.as_object_mut() {
                for (k, d) in keys {
                    o.entry(k.to_string()).or_insert_with(|| d.clone());
                }
            }
        }
    };
    for t in v.get_mut("tasks").and_then(|a| a.as_array_mut()).into_iter().flatten() {
        fill(t.get_mut("time_entries"), &[("member", 0.into())]);
    }
    if let Some(e) = v.get_mut("extra") {
        fill(e.get_mut("forms"), &[("owner", 0.into())]);
        fill(e.get_mut("whiteboards"), &[("owner", 0.into())]);
        fill(e.get_mut("dashboards"), &[("owner", 0.into()), ("kind", "".into())]);
    }
    v
}

impl App {
    fn data_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.data).unwrap_or_default()
    }

    /// After a change: note what changed so teammates' copies merge correctly.
    pub(super) fn sync_note_change(&mut self) {
        if self.data.extra.shared.is_none() {
            return;
        }
        self.stamp_now();
        self.sync_rt.pending = true;
    }

    /// Record what changed since the last save in the change bookkeeping.
    /// True when anything did.
    fn stamp_now(&mut self) -> bool {
        let v = self.data_value();
        let mut meta = std::mem::take(&mut self.data.extra.sync);
        let before = meta.clone();
        if meta.v == 1 {
            // fingerprints from before fields left at their default were
            // left out of the file: the new shape alone isn't an edit
            sync::carry_hashes(&mut meta, &legacy_shape(&v), &v);
        }
        sync::stamp(&v, &mut meta, chrono::Utc::now().timestamp_millis());
        let changed = meta != before;
        self.data.extra.sync = meta;
        changed
    }

    pub(super) fn write_shared(&mut self) {
        let Some(cfg) = self.data.extra.shared.clone() else { return };
        self.data.fix_activity_ids();
        let v = self.data_value();
        if cfg.server.is_some() {
            // sent when connected; after a reconnect the whole copy goes again anyway
            if let Some(net) = &self.sync_rt.net {
                net.push(sync::shareable(&v));
            }
            self.sync_rt.pending = false;
            return;
        }
        if sync::write_mine(Path::new(&cfg.folder), &cfg.device, &v).is_err() {
            self.toast = Some((tr("Couldn't write to the shared folder. Is it still there?").into(), now_ts()));
        }
        self.sync_rt.pending = false;
    }

    fn save_local(&self) {
        if !self.persist {
            return;
        }
        if self.ws_file == "data.json" {
            self.data.save();
        } else {
            self.data.save_file(&self.ws_file);
        }
    }

    /// Read teammates' copies and merge them in. Returns true when something new arrived.
    pub(super) fn sync_pull(&mut self) -> bool {
        let Some(cfg) = self.data.extra.shared.clone() else { return false };
        if cfg.server.is_some() {
            return self.net_tick(None);
        }
        let files = sync::other_files(Path::new(&cfg.folder), &cfg.device);
        self.sync_rt.seen = files.clone();
        let others: Vec<serde_json::Value> = files.iter().filter_map(|(p, _)| sync::read_json(p)).collect();
        self.merge_in(&others, &[])
    }

    /// Merge the team's copies into ours. On a server, `hidden` (with what
    /// our own copy shows we may not see) is taken out of our copy without
    /// deleting it. Returns true when something new arrived.
    pub(super) fn merge_in(&mut self, others: &[serde_json::Value], hidden: &[u64]) -> bool {
        if others.is_empty() {
            return false;
        }
        // changes not recorded yet (made just before quitting, say) are
        // recorded first, or the merge would take them as already known
        if self.data.extra.shared.is_some() && !self.data.extra.sync.hashes.is_empty() && self.stamp_now() {
            self.sync_rt.pending = true;
        }
        let me = self.data.me;
        let seen: Vec<u64> = self.data.channels.iter().filter(|c| c.kind == ChannelKind::Channel && (!c.private || c.members.contains(&me))).map(|c| c.id).collect();
        let mut v = self.data_value();
        let mut changed = sync::merge(&mut v, others);
        let on_server = self.data.extra.shared.as_ref().is_some_and(|c| c.server.is_some());
        let gone = if on_server {
            let mut ids = hidden.to_vec();
            ids.extend(sync::hidden_from(&v, me));
            sync::hide(&mut v, &ids)
        } else {
            vec![]
        };
        changed |= !gone.is_empty();
        match serde_json::from_value::<Data>(v) {
            Ok(d) => {
                self.data = d;
                // fingerprints of what the app writes, which can differ a
                // little from what came in (fields an older app left out)
                self.data.extra.sync.hashes = sync::fingerprints(&self.data_value());
            }
            Err(e) => {
                // most likely saved by a newer version of the app
                if !self.sync_rt.told_unreadable {
                    eprintln!("workboard: couldn't take in the team's changes: {}", e);
                    self.toast = Some((tr("Your team's changes couldn't be read. Someone may be using a newer version of the app, so update Workboard.").into(), now_ts()));
                    self.sync_rt.told_unreadable = true;
                }
                return false;
            }
        }
        for ch in gone.iter().filter(|c| c.get("id").and_then(|i| i.as_u64()).is_some_and(|i| seen.contains(&i))) {
            self.left_channel(ch);
        }
        if changed {
            // what's new for me is announced on the next frame (notify.rs)
            self.notify_rt.pending = true;
            self.save_local();
            self.sync_rt.pending = true;
        }
        changed
    }

    /// A channel I could see was taken out of my copy because I'm no longer
    /// in it: say so, and keep what I wrote there in the last few minutes in
    /// my Notepad, since it may not have reached the team before I left.
    fn left_channel(&mut self, ch: &serde_json::Value) {
        let me = self.data.me;
        let name = ch.get("name").and_then(|n| n.as_str()).unwrap_or_default().to_string();
        let since = now_ts() - 10 * 60;
        let mut mine: Vec<String> = vec![];
        for m in ch.get("messages").and_then(|m| m.as_array()).into_iter().flatten() {
            for x in std::iter::once(m).chain(m.get("replies").and_then(|r| r.as_array()).into_iter().flatten()) {
                let author = x.get("author").and_then(|a| a.as_u64());
                let ts = x.get("ts").and_then(|t| t.as_i64()).unwrap_or(0);
                let text = x.get("text").and_then(|t| t.as_str()).unwrap_or_default();
                if author == Some(me) && ts >= since && !text.trim().is_empty() {
                    mine.push(text.to_string());
                }
            }
        }
        if mine.is_empty() {
            self.toast = Some((trf!("You're no longer in #{}, so it was taken off your list.", name), now_ts()));
            return;
        }
        let notepad = &mut self.data.extra.notepad;
        if !notepad.is_empty() && !notepad.ends_with('\n') {
            notepad.push('\n');
        }
        notepad.push_str(&format!("{}\n{}\n", trf!("Written in #{} before you were taken out of it:", name), mine.join("\n")));
        self.toast = Some((trf!("You're no longer in #{}. What you wrote there in the last few minutes is in your Notepad.", name), now_ts()));
    }

    /// The server connection: start it when needed and take in what arrived.
    /// Returns true when the team's changes came in.
    pub(super) fn net_tick(&mut self, ctx: Option<&egui::Context>) -> bool {
        if self.sync_rt.net.is_none() {
            let (addr, sign_in) = if let Some(j) = &self.sync_rt.joining {
                (j.addr.clone(), j.how.sign_in(&j.device, self.data.me))
            } else if let Some(SharedCfg { server: Some(addr), device, tag, .. }) = &self.data.extra.shared {
                let mut si = match &self.sync_rt.resign {
                    Some(how) => how.sign_in(device, self.data.me),
                    None => SignIn { device: device.clone(), token: load_token(addr, device), ..Default::default() },
                };
                si.tag = Some(*tag);
                (addr.clone(), si)
            } else {
                return false;
            };
            let wake: wb_net::Wake = match ctx {
                Some(c) => {
                    let c = c.clone();
                    std::sync::Arc::new(move || c.request_repaint())
                }
                None => std::sync::Arc::new(|| {}),
            };
            self.sync_rt.net_status = None;
            self.sync_rt.net = Some(wb_net::Client::start(&addr, sign_in, wake));
        }
        let events = self.sync_rt.net.as_ref().map(|n| n.events()).unwrap_or_default();
        let mut changed = false;
        for e in events {
            match e {
                NetEvent::Status(s) => self.sync_rt.net_status = Some(s),
                NetEvent::Welcome { tag, snapshot, member, role, token, email, hidden, .. } => {
                    if let Some(t) = token {
                        let (addr, device) = match (&self.sync_rt.joining, &self.data.extra.shared) {
                            (Some(j), _) => (j.addr.clone(), j.device.clone()),
                            (None, Some(c)) => (c.server.clone().unwrap_or_default(), c.device.clone()),
                            _ => continue,
                        };
                        save_token(&addr, &device, &t);
                    }
                    if self.sync_rt.resign.take().is_some() {
                        self.toast = Some((tr("Signed in again.").into(), now_ts()));
                    }
                    self.sync_rt.account = email;
                    self.sync_rt.form = Form::default();
                    self.sync_rt.role = Some(role);
                    // the server that let us in also publishes app updates
                    let server = self.sync_rt.joining.as_ref().map(|j| j.addr.clone()).or_else(|| self.data.extra.shared.as_ref().and_then(|c| c.server.clone()));
                    if let Some(addr) = server {
                        crate::updater::set_server(&addr);
                    }
                    changed |= self.net_welcome(tag, snapshot, member, &hidden);
                }
                NetEvent::Update { data, hidden, .. } => changed |= self.merge_in(&[data], &hidden),
                NetEvent::Invite { member, code, emailed, mail_error } => {
                    if let Some(p) = self.sync_rt.people.iter_mut().find(|p| p.member == member) {
                        p.invite = Some(code);
                    }
                    let name = self.data.member(member).map(|m| m.name.clone()).unwrap_or_default();
                    if let Some(to) = emailed {
                        self.toast = Some((trf!("Emailed {}'s invite code to {}.", name, to), now_ts()));
                    } else if let Some(why) = mail_error {
                        self.toast = Some((trf!("Couldn't email {}'s invite code ({}). Copy it from Work together > People.", name, why), now_ts()));
                    }
                }
                NetEvent::Account(email) => {
                    self.toast = Some((trf!("Password saved. Sign in with {} on your other computers and phones.", email), now_ts()));
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
                    self.toast = Some((tr(&why).to_string(), now_ts()));
                }
                NetEvent::CallDenied { channel, why } => {
                    self.huddle_denied(channel);
                    self.toast = Some((why, now_ts()));
                }
                NetEvent::CallToken { channel, url, token } => self.huddle_token(channel, url, token),
                NetEvent::Backups(list) => self.sync_rt.backups = Some(list),
                NetEvent::Restored(name) => {
                    let label = wb_net::Backup { name, bytes: 0 }.label();
                    self.toast = Some((trf!("Restored the team's workspace to {}. Everyone gets it now.", label), now_ts()));
                    self.sync_rt.restore_confirm = None;
                    // the copy from before the restore is a new backup
                    self.access_request(NetMsg::BackupList);
                }
            }
        }
        changed
    }

    /// The server answered: finish joining, or catch up after a reconnect.
    fn net_welcome(&mut self, tag: u16, snapshot: Option<serde_json::Value>, member: u64, hidden: &[u64]) -> bool {
        if let Some(j) = self.sync_rt.joining.take() {
            let cfg = SharedCfg { folder: String::new(), device: j.device, tag, server: Some(j.addr) };
            match snapshot {
                None => {
                    // an empty server: this workspace becomes the team's
                    self.data.extra.shared = Some(cfg);
                    self.data.extra.sync = Default::default();
                    self.sync_note_change();
                    self.write_shared();
                    self.save_local();
                    self.share_cached_files();
                    self.toast = Some((tr("Connected. This workspace is now on the server for your team.").into(), now_ts()));
                    if matches!(self.modal, Some(Modal::Together { .. } | Modal::Welcome { .. })) {
                        self.modal = None;
                    }
                }
                Some(base) => match self.join_from(base, cfg) {
                    Ok(()) => {
                        self.after_join();
                        // the invite code says who this is
                        self.become_member(Some(member), "", "");
                        self.sync_note_change();
                        self.write_shared();
                        self.save_local();
                        let name = self.data.member(member).map(|m| m.name.clone()).unwrap_or_default();
                        self.toast = Some((trf!("Joined as {}.", name), now_ts()));
                        if matches!(self.modal, Some(Modal::Together { .. } | Modal::Welcome { .. })) {
                            self.modal = None;
                        }
                    }
                    Err(e) => {
                        self.sync_rt.net = None;
                        self.toast = Some((e, now_ts()));
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

    /// Start connecting to a server: with an invite code, or with none to
    /// set up an empty server.
    pub(super) fn connect_server(&mut self, addr: &str, invite: &str) -> Result<(), String> {
        let how = if invite.trim().is_empty() { How::Claim { email: String::new(), password: String::new() } } else { How::Invite { code: invite.to_string(), email: String::new(), password: String::new() } };
        self.connect_with(addr, how)
    }

    /// Start connecting to a server from the dialog (shares this workspace
    /// there if the server is empty, else joins the team's workspace).
    pub(super) fn connect_with(&mut self, addr: &str, how: How) -> Result<(), String> {
        wb_net::server_url(addr)?;
        if let How::Claim { email, .. } = &how {
            // the owner's email goes up with the workspace
            let me = self.data.me;
            if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me && m.email.trim().is_empty() && !email.trim().is_empty()) {
                m.email = email.trim().to_string();
                self.dirty = true;
            }
        }
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
        self.sync_rt.joining = Some(Joining { addr: addr.trim().to_string(), device: format!("{:08x}", sync::random_u64() as u32), how });
        Ok(())
    }

    /// Sign this computer in again with a new invite code.
    pub(super) fn sign_in_again(&mut self, code: &str) {
        if code.trim().is_empty() {
            return;
        }
        self.sign_in_with(How::Invite { code: code.to_string(), email: String::new(), password: String::new() });
    }

    /// Sign this computer in again: email and password, a code, a reset.
    pub(super) fn sign_in_with(&mut self, how: How) {
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
        let (ready, wait): (Vec<_>, Vec<_>) = self.sync_rt.auto_invite.drain(..).partition(|(m, _)| known.contains(m));
        self.sync_rt.auto_invite = wait;
        for (member, role) in ready {
            self.access_request(NetMsg::CreateInvite { member, role });
        }
    }

    /// Open what a workboard://join link (or a pasted invite) asks for.
    pub(super) fn open_join_link(&mut self, text: &str) {
        let j = wb_net::parse_join(text);
        self.data.extra.welcome = false;
        let form = &mut self.sync_rt.form;
        form.mode = Mode::Invite;
        form.code = j.code.clone();
        form.email = j.email.clone().unwrap_or_default();
        form.note.clear();
        match self.data.extra.shared.as_ref().and_then(|c| c.server.clone()) {
            Some(server) => {
                if !matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
                    self.toast = Some((trf!("This workspace is already on {}. To join another team, choose Stop sharing here first.", server), now_ts()));
                }
                self.modal = Some(self.together_modal());
            }
            None => self.modal = Some(Modal::Welcome { step: 2, name: String::new(), addr: j.server.unwrap_or_default() }),
        }
    }

    /// Send an access request to the server (owners and admins).
    pub(super) fn access_request(&mut self, msg: NetMsg) {
        match &self.sync_rt.net {
            Some(n) if self.sync_rt.net_status == Some(NetStatus::Connected) => {
                if !matches!(msg, NetMsg::CallJoin { .. }) {
                    self.sync_rt.other_asked = Some(std::time::Instant::now());
                }
                n.send(msg)
            }
            _ => self.toast = Some((tr("Not connected to the server right now.").into(), now_ts())),
        }
    }

    /// Once per frame: write our copy when it changed, look for teammates' changes.
    pub(super) fn sync_tick(&mut self, ctx: &egui::Context) {
        if self.sync_rt.joining.is_some() {
            self.net_tick(Some(ctx));
            return;
        }
        let Some(cfg) = self.data.extra.shared.clone() else { return };
        let now = ctx.input(|i| i.time);
        if cfg.server.is_some() {
            self.net_tick(Some(ctx));
            if self.sync_rt.pending {
                if now - self.sync_rt.last_write >= WRITE_SECS {
                    self.sync_rt.last_write = now;
                    self.write_shared();
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(WRITE_SECS));
                }
            }
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(POLL_SECS));
        if self.sync_rt.pending && now - self.sync_rt.last_write >= WRITE_SECS {
            self.sync_rt.last_write = now;
            self.write_shared();
        }
        if now - self.sync_rt.last_poll >= POLL_SECS {
            self.sync_rt.last_poll = now;
            let cfg = self.data.extra.shared.clone().unwrap();
            let files = sync::other_files(Path::new(&cfg.folder), &cfg.device);
            if files != self.sync_rt.seen {
                self.sync_pull();
            }
        }
    }

    fn new_cfg(folder: &str) -> SharedCfg {
        // pick an id tag nobody in the folder uses yet
        let used: Vec<u16> = sync::other_files(Path::new(folder), "")
            .iter()
            .filter_map(|(p, _)| sync::read_json(p))
            .filter_map(|v| v.get("extra")?.get("shared")?.get("tag")?.as_u64().map(|t| t as u16))
            .collect();
        let mut tag = (sync::random_u64() % 1024) as u16;
        while used.contains(&tag) {
            tag = (tag + 1) % 1024;
        }
        SharedCfg { folder: folder.to_string(), device: format!("{:08x}", sync::random_u64() as u32), tag, server: None }
    }

    /// Share the open workspace in an (empty) folder.
    pub(super) fn share_here(&mut self, folder: &str) -> Result<(), String> {
        let folder = folder.trim();
        if folder.is_empty() {
            return Err(tr("Choose a folder first.").into());
        }
        std::fs::create_dir_all(folder).map_err(|e| trf!("Couldn't use that folder: {}", e))?;
        self.data.extra.shared = Some(Self::new_cfg(folder));
        self.data.extra.sync = Default::default();
        self.sync_note_change();
        self.write_shared();
        self.save_local();
        self.sync_rt = SyncRt::default();
        self.share_cached_files();
        Ok(())
    }

    /// Join the workspace someone shared in a folder. It opens as a new
    /// workspace, so the one you had stays in the workspace menu.
    pub(super) fn join_shared(&mut self, folder: &str) -> Result<(), String> {
        let folder = folder.trim();
        let files = sync::other_files(Path::new(folder), "");
        let newest = files.iter().max_by_key(|(_, m)| *m).ok_or(tr("No shared workspace in that folder yet."))?;
        let base = sync::read_json(&newest.0).ok_or(tr("The shared workspace couldn't be read."))?;
        self.join_from(base, Self::new_cfg(folder))?;
        self.sync_rt = SyncRt::default();
        self.sync_pull();
        self.after_join();
        Ok(())
    }

    /// Open the team's copy as a new workspace on this computer.
    fn join_from(&mut self, base: serde_json::Value, cfg: SharedCfg) -> Result<(), String> {
        serde_json::from_value::<Data>(base.clone()).map_err(|_| tr("The shared workspace couldn't be read.").to_string())?;
        // the team's things with this person's own settings (theme, notifications,
        // pinned tools...) instead of the owner's
        let mut d: Data = serde_json::from_value(sync::with_team_things(&self.data_value(), &base)).map_err(|_| tr("The shared workspace couldn't be read.").to_string())?;
        // personal things that point into the old workspace start empty
        d.favorites.clear();
        d.extra.recent.clear();
        d.extra.notepad.clear();
        d.extra.reminders.clear();
        d.extra.trash.clear();
        d.extra.inbox_cleared.clear();
        d.extra.inbox_later.clear();
        d.extra.starred.clear();
        d.extra.closed_dms.clear();
        d.extra.shared = Some(cfg);
        d.extra.sync.hashes.clear();
        d.me = 0;
        if self.persist {
            self.data.save_file(&self.ws_file);
            let mut list = Data::workspaces();
            if list.is_empty() {
                list.push(WsEntry { name: self.data.workspace.clone(), file: self.ws_file.clone() });
            }
            let file = format!("workspace-{}.json", now_ts());
            list.push(WsEntry { name: d.workspace.clone(), file: file.clone() });
            Data::save_workspaces(&list);
            Data::set_last_workspace(&file);
            self.ws_places.clear();
            self.ws_file = file;
        }
        self.data = d;
        Ok(())
    }

    fn after_join(&mut self) {
        self.data.extra.sync.hashes = sync::fingerprints(&self.data_value());
        self.loc = self.data.first_list().map(Location::List).unwrap_or(Location::Home);
        self.section = Section::Home;
        self.open_task = None;
        self.chat_open = None;
        self.recent.clear();
        self.clear_history();
        // a workspace I just joined: what is in it already isn't news
        self.notify_rt.seeded = false;
        self.save_local();
    }

    pub(super) fn stop_sharing(&mut self) {
        self.leave_huddle();
        self.ws_places.clear();
        if let Some(SharedCfg { server: Some(addr), device, .. }) = &self.data.extra.shared {
            forget_token(addr, device);
        }
        self.data.extra.shared = None;
        self.data.extra.sync = Default::default();
        self.sync_rt = SyncRt::default();
        self.dirty = true;
    }

    /// Who you are in a joined workspace: an existing (invited) member, or someone new.
    pub(super) fn become_member(&mut self, id: Option<u64>, name: &str, email: &str) {
        let id = match id {
            Some(id) => id,
            None => {
                let id = self.data.new_id();
                let color = STATUS_PALETTE[1 + (id % 13) as usize];
                self.data.members.push(Member { id, name: name.trim().to_string(), color, email: email.trim().to_string(), role: "Member".into(), ..Default::default() });
                id
            }
        };
        if let Some(m) = self.data.members.iter_mut().find(|m| m.id == id) {
            m.invited = false;
        }
        self.data.me = id;
        for c in &mut self.data.channels {
            if c.kind == ChannelKind::Channel && !c.private && !c.members.contains(&id) {
                c.members.push(id);
            }
        }
        self.dirty = true;
    }

    /// Post a call in a channel or DM and open it in the browser.
    pub(super) fn start_call(&mut self, ctx: Option<&egui::Context>, ch: u64, custom: &str) -> String {
        let custom = custom.trim();
        let link = if custom.starts_with("https://") || custom.starts_with("http://") { custom.to_string() } else { call_link() };
        let id = self.data.new_id();
        let me = self.data.me;
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
            c.messages.push(Message { edited: None, mentions: vec![], attachments: vec![], id, author: me, text: "Started a call".into(), ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: Some(link.clone()) });
            c.last_read = now_ts();
        }
        self.section = Section::Chat;
        self.chat_open = Some(ch);
        self.dirty = true;
        if let Some(ctx) = ctx {
            ctx.open_url(egui::OpenUrl::new_tab(&link));
        }
        link
    }

    /// Where archived documents are written as Markdown.
    pub(super) fn archive_dir(&self) -> PathBuf {
        match &self.data.extra.shared {
            Some(cfg) if cfg.server.is_none() && !cfg.folder.is_empty() => Path::new(&cfg.folder).join("Docs archive"),
            _ => {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                Path::new(&home).join("Documents").join("Workboard Docs archive")
            }
        }
    }

    /// Save one document as a Markdown file in the archive folder.
    pub(super) fn export_doc(&self, id: u64) -> Option<PathBuf> {
        let d = self.data.docs.iter().find(|d| d.id == id)?;
        let dir = self.archive_dir();
        std::fs::create_dir_all(&dir).ok()?;
        let safe: String = d.title.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
        let safe = if safe.trim().is_empty() { "Untitled".to_string() } else { safe.trim().to_string() };
        let path = dir.join(format!("{} ({}).md", safe, d.id % 100000));
        let author = self.data.member(d.owner).map(|m| m.name.clone()).unwrap_or_default();
        let when = chrono::DateTime::from_timestamp(d.updated, 0).map(|x| x.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
        let mut text = format!("# {}\n\n_{} \u{2022} last updated {}_\n\n{}\n", d.title, author, when, d.body);
        // sub pages follow as sections, one heading level down per level
        fn subs(docs: &[Doc], parent: u64, level: usize, out: &mut String, seen: &mut Vec<u64>) {
            for c in docs.iter().filter(|c| c.parent == Some(parent)) {
                if seen.contains(&c.id) {
                    continue;
                }
                seen.push(c.id);
                out.push_str(&format!("\n{} {}\n\n{}\n", "#".repeat(level.min(6)), c.title, c.body));
                subs(docs, c.id, level + 1, out, seen);
            }
        }
        subs(&self.data.docs, d.id, 2, &mut text, &mut vec![d.id]);
        std::fs::write(&path, text).ok()?;
        Some(path)
    }

    /// Archive (or bring back) a document; archiving also saves a Markdown copy.
    pub(super) fn toggle_archive(&mut self, id: u64) {
        let Some(d) = self.data.docs.iter_mut().find(|d| d.id == id) else { return };
        d.archived = !d.archived;
        let archived = d.archived;
        self.dirty = true;
        if archived {
            self.toast = Some(match self.export_doc(id) {
                Some(p) => (trf!("Archived. A copy was saved in {}", p.parent().map(|p| p.display().to_string()).unwrap_or_default()), now_ts()),
                None => (tr("Archived. (The Markdown copy couldn't be saved.)").into(), now_ts()),
            });
        } else {
            self.toast = Some((tr("Moved back to Docs").into(), now_ts()));
        }
    }

    // ------------------------------------------------------------ dialogs

    /// The owner: go back to an earlier copy of the team's workspace.
    fn backups_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("Backups")).strong());
        ui.label(RichText::new(tr("Go back to an earlier copy of the team's workspace. Deleted items come back; anything made since stays. The server keeps a copy from just before.")).small().color(t.text3));
        let Some(list) = self.sync_rt.backups.clone() else {
            if ui.button(tr("Show backups")).clicked() {
                self.access_request(NetMsg::BackupList);
            }
            return;
        };
        if list.is_empty() {
            ui.label(RichText::new(tr("No backups yet. The first one is made within a minute of the server having a workspace.")).color(t.text2));
            return;
        }
        let mut req: Option<NetMsg> = None;
        let mut confirm = self.sync_rt.restore_confirm.clone();
        egui::ScrollArea::vertical().id_salt("backups").max_height(160.0).show(ui, |ui| {
            for b in list.iter().take(80) {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(b.label()).color(t.text));
                    ui.label(RichText::new(format!("{} KB", b.bytes.div_ceil(1024))).small().color(t.text3));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if confirm.as_deref() == Some(b.name.as_str()) {
                            if row_button(ui, tr("Cancel")).clicked() {
                                confirm = None;
                            }
                            if row_button(ui, RichText::new(tr("Yes, restore")).color(t.danger)).clicked() {
                                req = Some(NetMsg::Restore { name: b.name.clone() });
                            }
                        } else if row_button(ui, tr("Restore")).on_hover_text(trf!("Go back to {}", b.label())).clicked() {
                            confirm = Some(b.name.clone());
                        }
                    });
                });
            }
        });
        self.sync_rt.restore_confirm = confirm;
        if let Some(m) = req {
            self.access_request(m);
        }
    }

    /// Owners and admins: everyone's access, invite codes, roles.
    fn people_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("People")).strong());
        ui.label(RichText::new(tr("Give each person their invite code (it works once, for 7 days). Add people first with Invite.")).small().color(t.text3));
        let me = self.data.me;
        let my_role = self.sync_rt.role.unwrap_or_default();
        let people = self.sync_rt.people.clone();
        let mut req: Option<NetMsg> = None;
        egui::ScrollArea::vertical().id_salt("people").max_height(220.0).show(ui, |ui| {
            for p in &people {
                let Some(m) = self.data.member(p.member) else { continue };
                let name = m.name.clone();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&name).color(t.text));
                    ui.label(RichText::new(tr(p.role.label())).small().color(t.text2));
                    ui.label(RichText::new(match p.devices { 0 => tr("not signed in").to_string(), 1 => tr("1 computer").to_string(), n => trf!("{} computers", n) }).small().color(t.text3));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if p.role != Role::Owner && p.member != me {
                            if row_button(ui, RichText::new(tr("Remove access")).color(t.danger)).on_hover_text(trf!("Sign out {}'s computers", name)).clicked() {
                                req = Some(NetMsg::Revoke { member: p.member });
                            }
                            if my_role.manages() {
                                let (label, role) = if p.role == Role::Admin { ("Make member", Role::Member) } else { ("Make admin", Role::Admin) };
                                if row_button(ui, tr(label)).clicked() {
                                    req = Some(NetMsg::SetRole { member: p.member, role });
                                }
                            }
                        }
                        // a code for the owner signs in as the owner: only the owner makes one
                        let may_invite = p.member != me && (p.role != Role::Owner || my_role == Role::Owner);
                        if may_invite && row_button(ui, if p.invite.is_some() { tr("New code") } else { tr("Invite code") }).clicked() {
                            req = Some(NetMsg::CreateInvite { member: p.member, role: Role::Member });
                        }
                    });
                });
                if let Some(code) = &p.invite {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(trf!("Code for {}: {}", name, code)).monospace().color(t.text));
                        if row_button(ui, tr("Copy")).clicked() {
                            ui.ctx().copy_text(code.clone());
                            self.toast = Some((tr("Invite code copied").into(), now_ts()));
                        }
                    });
                }
            }
        });
        if let Some(m) = req {
            self.access_request(m);
        }
    }

    /// Server address, sign-in fields, and Connect (or the connection's
    /// progress and Cancel while it tries).
    fn server_form(&mut self, ui: &mut Ui, t: &Theme, addr: &mut String, modes: &[Mode]) {
        ui.label(RichText::new(tr("Server")).color(t.text2));
        let r = ui.add(egui::TextEdit::singleline(addr).hint_text(tr("macmini or 100.101.102.103")).desired_width(f32::INFINITY));
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Server address"));
        if self.sync_rt.joining.is_some() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let status = net_status_text(self.sync_rt.net.is_some(), self.sync_rt.net_status.as_ref());
                ui.label(RichText::new(status).color(t.text2));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        self.sync_rt.joining = None;
                        self.sync_rt.net = None;
                        self.sync_rt.net_status = None;
                    }
                });
            });
            // refused: say why and let them fix what they typed
            if let Some(NetStatus::Refused(why)) = self.sync_rt.net_status.clone() {
                self.sync_rt.joining = None;
                self.sync_rt.net = None;
                self.sync_rt.net_status = None;
                self.sync_rt.form.note = why;
            }
            return;
        }
        ui.add_space(6.0);
        if let Some(how) = self.login_form(ui, t, addr, modes, "Connect") {
            if let Err(e) = self.connect_with(addr, how) {
                self.sync_rt.form.note = e;
            }
        }
    }

    /// The sign-in fields for one of `modes` (with tabs when there are
    /// several, and "Forgot password?" under the password). The way to sign
    /// in when `button` is clicked and everything needed is there.
    fn login_form(&mut self, ui: &mut Ui, t: &Theme, addr: &mut String, modes: &[Mode], button: &str) -> Option<How> {
        let mut form = std::mem::take(&mut self.sync_rt.form);
        if !modes.contains(&form.mode) && !(form.mode == Mode::Forgot && modes.contains(&Mode::Account)) {
            form.mode = modes[0];
        }
        if modes.len() > 1 {
            ui.horizontal(|ui| {
                for m in modes {
                    let label = match m {
                        Mode::Account => "Email and password",
                        Mode::Invite => "Invite code",
                        Mode::Forgot => "Forgot password",
                        Mode::NewServer => "New server",
                    };
                    let on = form.mode == *m || (form.mode == Mode::Forgot && *m == Mode::Account);
                    if ui.selectable_label(on, tr(label)).clicked() && form.mode != *m {
                        // the email carries over; passwords and codes don't
                        form = Form { mode: *m, email: std::mem::take(&mut form.email), ..Default::default() };
                    }
                }
            });
            ui.add_space(4.0);
        }
        let field = |ui: &mut Ui, text: &mut String, label: &str, hint: &str, secret: bool| {
            ui.label(RichText::new(tr(label)).color(t.text2));
            let r = ui.add(egui::TextEdit::singleline(text).hint_text(tr(hint)).password(secret).desired_width(f32::INFINITY));
            let name = format!("{} entry", label);
            r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name));
        };
        let mut go = false;
        let mut how = None;
        match form.mode {
            Mode::Account => {
                field(ui, &mut form.email, "Email", "you@company.com", false);
                field(ui, &mut form.password, "Password", "", true);
                if ui.link(tr("Forgot password?")).clicked() {
                    form.mode = Mode::Forgot;
                    form.sent = false;
                    form.note.clear();
                }
            }
            Mode::Invite => {
                field(ui, &mut form.code, "Invite code", "XXXX-XXXX-XXXX, or paste the invite link", false);
                // a pasted link (or a whole invite mail) fills in everything it has
                if form.code.contains("workboard://") {
                    let j = wb_net::parse_join(&form.code);
                    form.code = j.code;
                    if let Some(e) = j.email {
                        form.email = e;
                    }
                    if let Some(s) = j.server {
                        *addr = s;
                    }
                }
                field(ui, &mut form.email, "Email", "the email you were invited with", false);
                field(ui, &mut form.password, "Choose a password", &trf!("at least {} characters", wb_net::MIN_PASSWORD), true);
                field(ui, &mut form.repeat, "Repeat password", "", true);
                ui.label(RichText::new(tr("You sign in with this email and password on your other computers and phones.")).small().color(t.text3));
            }
            Mode::Forgot => {
                field(ui, &mut form.email, "Email", "you@company.com", false);
                if let Some(rx) = &self.sync_rt.forgot {
                    match rx.try_recv() {
                        Ok(Ok(true)) => {
                            form.sent = true;
                            form.note = trf!("If {} has an account on this server, a reset code is on its way. It works for 30 minutes.", form.email.trim());
                            self.sync_rt.forgot = None;
                        }
                        Ok(Ok(false)) => {
                            form.note = tr("This server can't send email. Ask the owner or an admin for a new invite code instead.").into();
                            self.sync_rt.forgot = None;
                        }
                        Ok(Err(e)) => {
                            form.note = e;
                            self.sync_rt.forgot = None;
                        }
                        Err(_) => {
                            ui.label(RichText::new(tr("Asking the server\u{2026}")).color(t.text2));
                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                        }
                    }
                }
                if self.sync_rt.forgot.is_none() && ui.button(tr(if form.sent { "Email me another code" } else { "Email me a code" })).clicked() {
                    if wb_net::server_url(addr).is_err() {
                        form.note = tr("Type the server's address first.").into();
                    } else if !form.email.contains('@') {
                        form.note = tr("Type the email you sign in with.").into();
                    } else {
                        let (tx, rx) = std::sync::mpsc::channel();
                        let (a, e, ctx) = (addr.clone(), form.email.clone(), ui.ctx().clone());
                        std::thread::spawn(move || {
                            let _ = tx.send(wb_net::forgot_password(&a, &e));
                            ctx.request_repaint();
                        });
                        self.sync_rt.forgot = Some(rx);
                        form.note.clear();
                    }
                }
                if form.sent {
                    field(ui, &mut form.code, "Reset code", "XXXX-XXXX-XXXX from the mail", false);
                    field(ui, &mut form.password, "New password", &trf!("at least {} characters", wb_net::MIN_PASSWORD), true);
                    field(ui, &mut form.repeat, "Repeat password", "", true);
                }
                if ui.link(tr("Back to sign in")).clicked() {
                    form.mode = Mode::Account;
                    form.note.clear();
                }
            }
            Mode::NewServer => {
                ui.label(RichText::new(tr("This workspace goes up to the empty server and you become its owner. Choose how you sign in on your other computers and phones.")).small().color(t.text3));
                field(ui, &mut form.email, "Email", "you@company.com", false);
                field(ui, &mut form.password, "Choose a password", &trf!("at least {} characters", wb_net::MIN_PASSWORD), true);
                field(ui, &mut form.repeat, "Repeat password", "", true);
            }
        }
        if !form.note.is_empty() {
            ui.label(RichText::new(&form.note).color(t.danger));
        }
        ui.add_space(6.0);
        let shown = form.mode != Mode::Forgot || form.sent;
        if shown {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    go = primary_button(ui, t, tr(if form.mode == Mode::Forgot { "Set password and sign in" } else { button })).clicked();
                });
            });
        }
        if go {
            let email_ok = form.email.trim().contains('@');
            let problem = match form.mode {
                Mode::Account if !email_ok => Some(tr("Type your email.").to_string()),
                Mode::Account if form.password.is_empty() => Some(tr("Type your password.").to_string()),
                Mode::Invite if form.code.trim().is_empty() => Some(tr("Type your invite code.").to_string()),
                Mode::Invite if !form.email.trim().is_empty() && !email_ok => Some(tr("That email doesn't look right.").to_string()),
                Mode::Forgot if form.code.trim().is_empty() => Some(tr("Type the code from the mail.").to_string()),
                Mode::NewServer if !email_ok => Some(tr("Type your email.").to_string()),
                Mode::Invite | Mode::Forgot | Mode::NewServer => new_password_problem(&form.password, &form.repeat),
                _ => None,
            };
            match problem {
                Some(p) => form.note = p,
                None => {
                    form.note.clear();
                    how = Some(match form.mode {
                        Mode::Account => How::Account { email: form.email.clone(), password: form.password.clone() },
                        Mode::Invite => How::Invite { code: form.code.clone(), email: form.email.clone(), password: form.password.clone() },
                        Mode::Forgot => How::Reset { email: form.email.clone(), code: form.code.clone(), password: form.password.clone() },
                        Mode::NewServer => How::Claim { email: form.email.clone(), password: form.password.clone() },
                    });
                }
            }
        }
        self.sync_rt.form = form;
        how
    }

    /// Signed in on a server: the email you sign in with elsewhere, setting
    /// up a password or changing it.
    fn account_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("Your account")).strong());
        let mut form = std::mem::take(&mut self.sync_rt.acct);
        let field = |ui: &mut Ui, text: &mut String, label: &str, secret: bool| {
            ui.horizontal(|ui| {
                ui.add_sized([130.0, 20.0], egui::Label::new(RichText::new(tr(label)).color(t.text2)));
                let r = ui.add(egui::TextEdit::singleline(text).password(secret).desired_width(260.0));
                let name = format!("{} entry", label);
                r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name));
            });
        };
        let mut send = None;
        match self.sync_rt.account.clone() {
            Some(email) => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(trf!("You sign in with {} on your other computers and phones.", email)).color(t.text2));
                    if !form.changing && row_button(ui, tr("Change password")).clicked() {
                        form.changing = true;
                        form.note.clear();
                    }
                });
                if form.changing {
                    field(ui, &mut form.old, "Current password", true);
                    field(ui, &mut form.password, "New password", true);
                    field(ui, &mut form.repeat, "Repeat password", true);
                    ui.horizontal(|ui| {
                        if primary_button(ui, t, tr("Save password")).clicked() {
                            match new_password_problem(&form.password, &form.repeat) {
                                Some(p) => form.note = p,
                                None => send = Some(NetMsg::SetPassword { email: email.clone(), password: form.password.clone(), old_password: Some(form.old.clone()) }),
                            }
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            form = Form::default();
                        }
                    });
                }
            }
            None => {
                ui.label(RichText::new(tr("Set a password to sign in on your other computers and phones without a new invite code.")).color(t.text2));
                if form.email.is_empty() {
                    form.email = self.data.member(self.data.me).map(|m| m.email.clone()).unwrap_or_default();
                }
                field(ui, &mut form.email, "Email", false);
                field(ui, &mut form.password, "Password", true);
                field(ui, &mut form.repeat, "Repeat password", true);
                if primary_button(ui, t, tr("Save password")).clicked() {
                    if !form.email.trim().contains('@') {
                        form.note = tr("Type your email.").into();
                    } else {
                        match new_password_problem(&form.password, &form.repeat) {
                            Some(p) => form.note = p,
                            None => send = Some(NetMsg::SetPassword { email: form.email.trim().to_string(), password: form.password.clone(), old_password: None }),
                        }
                    }
                }
            }
        }
        if !form.note.is_empty() {
            ui.label(RichText::new(&form.note).color(t.danger));
        }
        self.sync_rt.acct = form;
        if let Some(m) = send {
            self.access_request(m);
        }
    }

    /// The first start: work alone on this Mac, join a team, or set up a
    /// team server. `step`: 0 the choice, 1 your name, 2 join, 3 new server.
    pub(super) fn welcome_body(&mut self, ui: &mut Ui, t: &Theme, step: &mut u8, name: &mut String, addr: &mut String) -> bool {
        let mut close = false;
        match *step {
            0 => {
                ui.label(RichText::new(tr("Tasks, chat and docs for you and your team. How do you want to start?")).color(t.text2));
                ui.add_space(10.0);
                let choices = [
                    (1u8, "Use it on this Mac", "Start your own workspace. You can share it with your team later."),
                    (2, "Join my team", "Sign in with your email, or use the invite code someone sent you."),
                    (3, "Set up a server for my team", "Put this workspace on your team's Workboard server and become its owner."),
                ];
                for (to, title, desc) in choices {
                    let r = ui.add(egui::Button::new(RichText::new(format!("{}\n{}", tr(title), tr(desc))).color(t.text)).min_size(vec2(ui.available_width(), 52.0)).corner_radius(8));
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
                    if r.clicked() {
                        *step = to;
                        self.sync_rt.form = Form { mode: if to == 3 { Mode::NewServer } else { Mode::Account }, ..Default::default() };
                        if to == 1 && name.is_empty() {
                            *name = self.data.member(self.data.me).map(|m| m.name.clone()).unwrap_or_default();
                        }
                    }
                    ui.add_space(6.0);
                }
            }
            1 => {
                ui.label(RichText::new(tr("What should your team call you?")).color(t.text2));
                let r = ui.add(egui::TextEdit::singleline(name).hint_text(tr("Your name")).desired_width(f32::INFINITY));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Your name entry"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Back")).clicked() {
                        *step = 0;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, t, tr("Start")).clicked() {
                            let me = self.data.me;
                            if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me).filter(|_| !name.trim().is_empty()) {
                                m.name = name.trim().to_string();
                            }
                            self.dirty = true;
                            close = true;
                        }
                    });
                });
            }
            _ => {
                let modes: &[Mode] = if *step == 3 { &[Mode::NewServer] } else { &[Mode::Account, Mode::Invite] };
                ui.label(RichText::new(tr(if *step == 3 {
                    "Type the address of the Mac running workboard-server (for example a Mac mini your team reaches through Tailscale)."
                } else {
                    "Type your team's server address. Then sign in with your email and password, or use the invite code you were sent."
                })).color(t.text2));
                ui.add_space(4.0);
                self.server_form(ui, t, addr, modes);
                ui.add_space(4.0);
                if self.sync_rt.joining.is_none() && ui.button(tr("Back")).clicked() {
                    *step = 0;
                    self.sync_rt.form = Form::default();
                }
            }
        }
        close
    }

    /// The Work together dialog, filled in with what this workspace uses.
    pub(super) fn together_modal(&self) -> Modal {
        let cfg = self.data.extra.shared.as_ref();
        Modal::Together {
            path: cfg.filter(|c| c.server.is_none()).map(|c| c.folder.clone()).unwrap_or_default(),
            addr: cfg.and_then(|c| c.server.clone()).unwrap_or_default(),
            code: String::new(),
            result: String::new(),
        }
    }

    pub(super) fn together_body(&mut self, ui: &mut Ui, t: &Theme, path: &mut String, addr: &mut String, code: &mut String, result: &mut String) -> bool {
        let mut close = false;
        if let Some(SharedCfg { server: Some(server), .. }) = self.data.extra.shared.clone() {
            ui.label(RichText::new(tr("This workspace is on a server")).size(15.0).strong());
            ui.label(RichText::new(&server).color(t.text2));
            let connected = self.sync_rt.net_status == Some(NetStatus::Connected);
            let status = net_status_text(self.sync_rt.net.is_some(), self.sync_rt.net_status.as_ref());
            ui.label(RichText::new(trf!("Status: {}", status)).color(if connected { t.text } else { t.text2 }));
            if let Some(role) = self.sync_rt.role.filter(|_| connected) {
                let name = self.data.member(self.data.me).map(|m| m.name.clone()).unwrap_or_default();
                ui.label(RichText::new(trf!("Signed in as {} ({})", name, tr(role.label()))).color(t.text2));
            }
            if matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Sign this computer in again with your email and password, or with a new invite code from the owner or an admin.")).color(t.text2));
                let mut at = server.clone();
                if let Some(how) = self.login_form(ui, t, &mut at, &[Mode::Account, Mode::Invite], "Sign in") {
                    self.sign_in_with(how);
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Tasks, chat and docs show up for everyone within a second or two. While the server can't be reached you keep working here, and your changes go up when it's back.")).small().color(t.text3));
            if connected {
                ui.add_space(10.0);
                self.account_section(ui, t);
            }
            if connected && self.sync_rt.role.is_some_and(|r| r.manages()) {
                ui.add_space(10.0);
                self.people_section(ui, t);
                if self.sync_rt.role == Some(Role::Owner) {
                    ui.add_space(10.0);
                    self.backups_section(ui, t);
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Reconnect now")).clicked() {
                    self.sync_rt.net = None;
                    self.sync_rt.net_status = None;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new(tr("Stop sharing")).color(t.danger)).on_hover_text(tr("Keep working on this computer only; your copy stays")).clicked() {
                        self.stop_sharing();
                        close = true;
                    }
                });
            });
        } else if let Some(cfg) = self.data.extra.shared.clone() {
            ui.label(RichText::new(tr("This workspace is shared")).size(15.0).strong());
            ui.label(RichText::new(&cfg.folder).color(t.text2));
            let n = sync::other_files(Path::new(&cfg.folder), &cfg.device).len();
            ui.label(RichText::new(match n {
                0 => tr("Nobody else has joined yet. Share this folder with your teammates in your sync app, then they choose Work together > Join.").to_string(),
                1 => tr("1 other computer is working in it.").to_string(),
                n => trf!("{} other computers are working in it.", n),
            }).color(t.text2));
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Tasks, chat, docs and calls show up for everyone within a few seconds of their sync app copying the folder.")).small().color(t.text3));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Check for changes now")).clicked() {
                    let changed = self.sync_pull();
                    *result = if changed { tr("Got the latest changes.").into() } else { tr("Everything is up to date.").into() };
                }
                if ui.button(tr("Show folder")).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("file://{}", cfg.folder)));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new(tr("Stop sharing")).color(t.danger)).on_hover_text(tr("Keep working on this computer only; your copy stays")).clicked() {
                        self.stop_sharing();
                        close = true;
                    }
                });
            });
        } else {
            ui.label(tr("Work on this workspace with other people, without a server."));
            ui.label(RichText::new(tr("Pick a folder that a sync app shares with your teammates (iCloud Drive, Dropbox, Google Drive, OneDrive or Syncthing). Everyone opens Workboard on their own computer and points it at the same folder.")).color(t.text2));
            ui.add_space(8.0);
            let sugg = sync::suggested_folders();
            if !sugg.is_empty() {
                ui.label(RichText::new(tr("Found on this computer")).small().color(t.text3));
                ui.horizontal_wrapped(|ui| {
                    for (name, p) in &sugg {
                        if ui.button(name).on_hover_text(p).clicked() {
                            *path = p.clone();
                        }
                    }
                });
            }
            ui.label(RichText::new(tr("Folder")).color(t.text2));
            ui.add(egui::TextEdit::singleline(path).hint_text(tr("Paste a folder path, or pick one above")).desired_width(f32::INFINITY));
            let n = folder_people(path);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if n > 0 {
                    ui.label(RichText::new(if n == 1 { tr("A shared workspace is already here (1 person).").to_string() } else { trf!("A shared workspace is already here ({} people).", n) }).color(t.text2));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, t, tr("Join this workspace")).clicked() {
                            match self.join_shared(path) {
                                Ok(()) => {
                                    self.modal = Some(Modal::WhoAmI { name: String::new(), email: String::new() });
                                }
                                Err(e) => *result = e,
                            }
                        }
                    });
                } else {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, t, tr("Share this workspace here")).clicked() {
                            match self.share_here(path) {
                                Ok(()) => {
                                    self.toast = Some((tr("Shared. Now share the folder with your teammates in your sync app.").into(), now_ts()));
                                    close = true;
                                }
                                Err(e) => *result = e,
                            }
                        }
                    });
                }
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new(tr("Or connect to your team's server")).strong());
            ui.label(RichText::new(tr("A Workboard server (for example a Mac mini your team reaches through Tailscale). Sign in with your email and password, or join with the invite code you were given. Setting up a new, empty server? This workspace goes up and you become its owner.")).color(t.text2));
            let _ = code;
            self.server_form(ui, t, addr, &[Mode::Account, Mode::Invite, Mode::NewServer]);
        }
        if !result.is_empty() {
            ui.label(RichText::new(result.as_str()).color(t.text2));
        }
        close
    }

    pub(super) fn who_am_i_body(&mut self, ui: &mut Ui, t: &Theme, name: &mut String, email: &mut String) -> bool {
        let mut close = false;
        ui.label(RichText::new(tr("Pick yourself so your messages and tasks show your name.")).color(t.text2));
        ui.add_space(6.0);
        let members: Vec<Member> = self.data.members.clone();
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for m in &members {
                let label = if m.invited { trf!("{}   {}  (invited)", m.name, m.email) } else { format!("{}   {}", m.name, m.email) };
                if ui.add(egui::Button::new(label).frame(false)).clicked() {
                    self.become_member(Some(m.id), "", "");
                    close = true;
                }
            }
        });
        ui.separator();
        ui.label(RichText::new(tr("I'm new here")).color(t.text2));
        ui.add(egui::TextEdit::singleline(name).hint_text(tr("Your name")).desired_width(f32::INFINITY));
        ui.add(egui::TextEdit::singleline(email).hint_text(tr("Email")).desired_width(f32::INFINITY));
        ui.add_space(6.0);
        if primary_button(ui, t, tr("Join as new member")).clicked() && !name.trim().is_empty() {
            self.become_member(None, name, email);
            close = true;
        }
        close
    }

    pub(super) fn call_body(&mut self, ui: &mut Ui, t: &Theme, ch: &mut u64, link: &mut String) -> bool {
        let mut close = false;
        ui.label(RichText::new(tr("The call opens in your browser on Jitsi Meet (free). The person who starts it may be asked to sign in with Google, GitHub or Facebook; everyone else joins without an account. Everyone in the conversation gets a Join button.")).color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Call in")).color(t.text2));
        let chans: Vec<(u64, String)> = self.data.channels.iter().filter(|c| self.listed(c)).map(|c| (c.id, if c.kind == ChannelKind::Channel { format!("# {}", c.name) } else { self.channel_label(c) })).collect();
        let cur = chans.iter().find(|c| c.0 == *ch).map(|c| c.1.clone()).unwrap_or_else(|| tr("Choose").into());
        egui::ComboBox::from_id_salt("call_in").selected_text(cur).width(300.0).show_ui(ui, |ui| {
            for (id, n) in &chans {
                ui.selectable_value(ch, *id, n);
            }
        });
        ui.add_space(6.0);
        ui.label(RichText::new(tr("Or use your own meeting link (Zoom, Google Meet...)")).small().color(t.text3));
        ui.add(egui::TextEdit::singleline(link).hint_text("https://...").desired_width(f32::INFINITY));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, t, tr("Start call")).clicked() && chans.iter().any(|c| c.0 == *ch) {
                    let ctx = ui.ctx().clone();
                    self.start_call(Some(&ctx), *ch, link);
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// The call to open from a Huddle / call button: the open conversation, else #general.
    pub(super) fn open_call_dialog(&mut self) {
        let ch = if self.section == Section::Chat { self.chat_open } else { None }
            .or_else(|| self.data.channels.iter().find(|c| c.kind == ChannelKind::Channel).map(|c| c.id))
            .unwrap_or(0);
        // on the team's server: a voice huddle right here
        if self.huddles_on_server() && ch != 0 {
            self.start_huddle(ch);
            return;
        }
        self.modal = Some(Modal::Call { ch, link: String::new() });
    }
}
