// workboard-server: keeps the team's copy of a Workboard workspace, keeps
// every connected app in step with it (see wb_net for the messages), decides
// who may sign in, and keeps backups.
//
// Files in the server folder:
//   workspace.json        the team's copy
//   server.json           devices (hashed tokens, member, id tag), roles,
//                         open invites, accounts (email, salted password
//                         hash), iPhone push tokens, revision number
//   mail.json             how to send email (see mail.rs); optional
//   apns.json             how to reach Apple's push service, for
//                         notifications on iPhones (see push.rs); optional
//   ai.json               the Anthropic API key for the apps' AI tools
//                         (see ai.rs); optional
//   outbox/               mails written here instead of sent (mail.json
//                         "transport": "folder")
//   backups/              hourly-YYYYMMDD-HH.json (48 kept),
//                         daily-YYYYMMDD.json (30 kept), before-restore-*.json
//                         (20 kept)
//   admin-request.json    a request from the `workboard-server` admin
//   admin-result.json     commands, picked up within a second (see admin)
//   updates/              app updates: appcast.xml and the Workboard-*.zip it
//                         lists, served at /updates/<file> for Sparkle; the
//                         newest zip is also at /download/mac (-arm64.zip)
//                         and /download/windows (-windows-x64.zip)
//                         (scripts/publish-update.sh puts them here)
//   files/                pictures and videos for chat messages, named by
//                         the sha256 of their contents (see files.rs)
// Everything is written to a temporary file first and then renamed, at most
// a second after a change, and once more when the server stops.
//
// Signing in: the first app to reach an empty server claims it and its
// person becomes the owner. Everyone else needs a one-time invite code made
// by the owner or an admin, which signs that computer in as one member of
// the workspace and gives it a token for later. Owners and admins can also
// change roles and remove someone's access.
//
// Accounts: joining with an invite code (or setting up an empty server) can
// also choose a password (someone who already has one keeps it); the
// account's email is the one on the person's member record. With it the person signs in on any other computer or phone
// without a new code. When mail.json is set up the server emails invite codes
// (with a link to /join) and password-reset codes (POST /auth/forgot).
// Removing someone's access also deletes their account.

use axum::extract::State;
use axum::http::{header, Method, StatusCode, Version};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio_tungstenite::tungstenite::protocol::{Role as WsRole, WebSocketConfig};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use wb_net::{Backup, Msg, Person, Role, SignIn, CODE_AUTH, CODE_DENIED, MIN_PASSWORD};

pub mod ai;
mod files;
pub use files::{collect_garbage, file_name_ok, parse_range, FILES_DIR, FILE_KEEP_DAYS, MAX_FILE};
pub mod mail;
pub use mail::{load_mail, MailCfg, MAIL_FILE, OUTBOX_DIR};
pub mod push;
pub use push::{load_apns, ApnsCfg, APNS_FILE};
pub mod registry;
pub mod spaces;
#[cfg(feature = "testing")]
pub mod testing;

pub const WORKSPACE_FILE: &str = "workspace.json";
pub const STATE_FILE: &str = "server.json";
pub const BACKUP_DIR: &str = "backups";
pub const UPDATES_DIR: &str = "updates";
pub const REQUEST_FILE: &str = "admin-request.json";
pub const RESULT_FILE: &str = "admin-result.json";
/// Kept locked while the server runs, so a command can tell a busy server
/// from one that is off.
pub const RUNNING_FILE: &str = "running.lock";
/// LiveKit address and keys for voice huddles (written by deploy/use-livekit-cloud.sh).
pub const LIVEKIT_FILE: &str = "livekit.json";
/// How long a call token lets someone into a huddle.
pub const CALL_TOKEN_SECS: i64 = 6 * 3600;
pub const KEEP_HOURLY: usize = 48;
pub const KEEP_DAILY: usize = 30;
/// Invite codes stop working after this long.
pub const INVITE_DAYS: i64 = 7;
/// Password-reset codes stop working after this long.
pub const RESET_MINUTES: i64 = 30;
/// A code that signs a phone in (`create_link_code`) works this long...
pub const LINK_SECS: i64 = 5 * 60;
/// ...and at most this many wrong ones are taken a minute in a workspace.
/// With 31^6 possible codes that leaves no real chance of guessing one.
pub const LINK_TRIES_A_MINUTE: u32 = 20;
/// At most this many link codes wait at once in a workspace.
const MAX_LINKS: usize = 1_000;
/// Wrong passwords or reset codes in a row before an email is locked...
pub const MAX_TRIES: u32 = 5;
/// ...for this long.
pub const LOCK_MINUTES: i64 = 15;
/// At most this many emails' wrong tries are kept in memory.
const MAX_FAIL_ENTRIES: usize = 10_000;
/// PBKDF2-HMAC-SHA256 rounds for new password hashes.
const PBKDF2_ROUNDS: u32 = 100_000;
/// before-restore-*.json copies kept (the newest ones).
pub const KEEP_BEFORE_RESTORE: usize = 20;
/// A claim on an empty server that never brought its workspace (the app quit
/// right after) gives way to another claim after this long.
pub const CLAIM_WAIT_SECS: u64 = 60;
/// The largest first message (hello) before someone has signed in.
pub const MAX_HELLO: usize = 64 << 10;

mod admin;
mod auth;
mod canon;
mod curl;
pub mod hooks;
mod jobs;
mod livekit;
mod mine;
mod notify;
mod routes;
mod schedule;
mod state;
mod store;
mod util;
mod ws;

pub use admin::*;
pub use canon::*;
pub use livekit::*;
pub use routes::*;
pub use state::*;
pub use store::*;
pub use util::*;
use ws::*;

// --------------------------------------------------------------------- hub

struct Inner {
    canon: Option<Arc<Value>>,
    saved: Saved,
    dirty: bool,
    /// Only someone's own reading state changed (mine.rs): server.json is
    /// written on the next save tick, the team's copy keeps its own pace.
    mine_dirty: bool,
    /// The team's copy changed and isn't on disk yet: when the first and the
    /// latest of those changes were made (see `Hub::save`).
    canon_due: Option<(std::time::Instant, std::time::Instant)>,
    /// email (normalized) -> (reset code, when made); kept in memory only
    resets: std::collections::HashMap<String, (String, i64)>,
    /// email (normalized) -> (wrong tries in a row, time of the last one)
    fails: std::collections::HashMap<String, (u32, i64)>,
    /// email (normalized) -> when a reset code was last mailed to it
    mailed: std::collections::HashMap<String, i64>,
    /// codes that sign a phone in as the person whose computer showed them
    /// (`create_link_code`); kept in memory only
    links: std::collections::HashMap<String, LinkRec>,
    /// wrong link codes typed in this workspace: (how many, when the first
    /// of the current minute came)
    link_fails: (u32, i64),
    /// what the team's copy held at the last broadcast, to send apps only
    /// what changed since (`wb_sync::delta`)
    summary: Option<wb_sync::Summary>,
}

impl Inner {
    /// Refuse while an email has had too many wrong tries.
    fn locked(&self, email: &str) -> Option<String> {
        let (n, last) = self.fails.get(&norm_email(email)).copied()?;
        (n >= MAX_TRIES && now_ms() - last < LOCK_MINUTES * 60_000).then(|| {
            format!(
                "Too many wrong tries. Wait {} minutes and try again, or use Forgot password.",
                LOCK_MINUTES
            )
        })
    }

    /// Take back one wrong try counted for an email (it proved right).
    fn unfail(&mut self, email: &str) {
        let key = norm_email(email);
        if let Some(e) = self.fails.get_mut(&key) {
            e.0 = e.0.saturating_sub(1);
            if e.0 == 0 {
                self.fails.remove(&key);
            }
        }
    }

    fn failed(&mut self, email: &str) {
        let now = now_ms();
        if self.fails.len() >= MAX_FAIL_ENTRIES {
            self.prune_fails(now);
        }
        let e = self.fails.entry(norm_email(email)).or_insert((0, now));
        if now - e.1 >= LOCK_MINUTES * 60_000 {
            e.0 = 0;
        }
        e.0 += 1;
        e.1 = now;
    }

    /// Forget wrong tries that no longer count (older than LOCK_MINUTES).
    /// When that isn't enough (someone typing many different emails), drop
    /// the oldest emails that aren't locked; locked ones stay until they expire.
    fn prune_fails(&mut self, now: i64) {
        self.fails
            .retain(|_, (_, last)| now - *last < LOCK_MINUTES * 60_000);
        if self.fails.len() >= MAX_FAIL_ENTRIES {
            let mut open: Vec<(i64, String)> = self
                .fails
                .iter()
                .filter(|(_, (n, _))| *n < MAX_TRIES)
                // made-up emails go first: tries at a real account's
                // password stay counted however many others come
                .filter(|(e, _)| self.saved.account_member(e).is_none())
                .map(|(e, (_, last))| (*last, e.clone()))
                .collect();
            open.sort();
            let extra = self.fails.len() + 1 - MAX_FAIL_ENTRIES / 2;
            for (_, e) in open.into_iter().take(extra) {
                self.fails.remove(&e);
            }
        }
    }
}

/// A turn at working out a password hash (100 000 rounds). Anyone on the
/// internet can ask for one with a made-up email, so only a few run at
/// once per workspace; more are turned away instead of taking every core
/// and holding up syncing for everyone.
struct HashSlot<'a>(&'a AtomicUsize);

impl<'a> HashSlot<'a> {
    fn take(running: &'a AtomicUsize) -> Result<HashSlot<'a>, String> {
        let most = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .max(2);
        if running.fetch_add(1, Ordering::SeqCst) >= most {
            running.fetch_sub(1, Ordering::SeqCst);
            return Err("The server is busy. Try again in a moment.".into());
        }
        Ok(HashSlot(running))
    }
}

impl Drop for HashSlot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A link code waiting to be typed or scanned on a phone.
#[derive(Clone, Debug)]
struct LinkRec {
    member: u64,
    /// the computer that showed it: it must still be signed in as `member`
    device: String,
    made: i64,
}

/// The key wrong reset codes for `email` are counted under in `fails`.
fn reset_tries(email: &str) -> String {
    format!("reset code {}", norm_email(email))
}

/// What a hello asked for: how the app signs in, as wb_net reads it from
/// the message (`SignIn::try_from`).
type Hello = SignIn;

fn given(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|v| !v.trim().is_empty())
}

/// What a request from an app turned into.
enum Reply {
    To(Msg),
    Denied(String),
}

/// The team's copy is written once its changes stop for this long...
const SAVE_QUIET: Duration = Duration::from_secs(3);
/// ...or this long after the first change not written, while they go on.
const SAVE_AT_MOST: Duration = Duration::from_secs(15);

/// What an app hears when changes in its copy were turned down: a space
/// shared with its person at a level that doesn't allow them (spaces.rs).
const PUT_BACK: &str =
    "Some changes were put back because that space is shared with you at a level that doesn't allow them.";
/// How long after telling that a connection isn't told again.
const PUT_BACK_QUIET_SECS: u64 = 10;

/// A message made for one revision and what it left out (`left_out`).
type Made = Arc<std::sync::OnceLock<(String, Vec<u64>)>>;

/// A new team copy for every connection; each one sends what its person may see.
struct Canon {
    /// shared with the hub's state, not copied: the next change copies it
    /// only if a connection still holds this revision (`Arc::make_mut`)
    data: Arc<Value>,
    rev: u64,
    /// the whole copy as an update message, for people nothing is hidden
    /// from; made the first time one needs it (apps that take changes only
    /// never do)
    json: std::sync::OnceLock<String>,
    /// what changed since the last revision (None: send the whole copy)
    delta: Option<Value>,
    /// owners and admins when it was made (they see every shared space)
    managers: Vec<u64>,
    /// the messages already made for people something is hidden from, by
    /// (a change?, what is hidden, what comes back whole): people who see
    /// the same get the same message, made once
    made: Mutex<std::collections::HashMap<(bool, Vec<u64>, Vec<u64>), Made>>,
    /// what is left out for each set of hidden items (`left_out`), worked
    /// out once for everyone who sees the same
    gone: Mutex<std::collections::HashMap<Vec<u64>, Arc<std::sync::OnceLock<Vec<u64>>>>>,
}

impl Canon {
    fn new(data: Arc<Value>, rev: u64, delta: Option<Value>, saved: &Saved) -> Canon {
        Canon {
            json: std::sync::OnceLock::new(),
            data,
            rev,
            delta,
            managers: saved
                .roles
                .iter()
                .filter(|(_, r)| r.manages())
                .map(|(m, _)| *m)
                .collect(),
            made: Mutex::new(std::collections::HashMap::new()),
            gone: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// The message for `key`, made with `make` by the first connection
    /// that needs it. Others that need the same wait for it rather than
    /// make it again; different ones are made side by side.
    fn made(
        &self,
        key: (bool, Vec<u64>, Vec<u64>),
        make: impl FnOnce() -> (String, Vec<u64>),
    ) -> (String, Vec<u64>) {
        let cell = self.made.locked().entry(key).or_default().clone();
        cell.get_or_init(make).clone()
    }

    /// What is left out for `hidden` (`left_out`), made once.
    fn left_out(&self, hidden: &[u64]) -> Vec<u64> {
        if hidden.is_empty() {
            return vec![];
        }
        let cell = self
            .gone
            .locked()
            .entry(hidden.to_vec())
            .or_default()
            .clone();
        cell.get_or_init(|| left_out(&self.data, hidden)).clone()
    }

    /// The whole copy `member` gets, and what is left out of it (`left_out`).
    fn update_for(&self, member: u64) -> (String, Vec<u64>) {
        // (copy_for, made once for each set of hidden items)
        let hidden = wb_sync::hidden_from(&self.data, member, self.managers.contains(&member));
        if hidden.is_empty() {
            return (self.whole_json().to_string(), vec![]);
        }
        self.made((false, hidden.clone(), vec![]), || {
            let mut data = (*self.data).clone();
            wb_sync::hide(&mut data, &hidden);
            let gone = self.left_out(&hidden);
            (update_json(&data, self.rev, &hidden), gone)
        })
    }

    /// The whole copy as an update message (`json`).
    fn whole_json(&self) -> &str {
        self.json
            .get_or_init(|| update_json(&self.data, self.rev, &[]))
    }

    /// What changed for `member`, whose last copy left out `was_hidden`
    /// (`left_out`): the change without what they may not see, and whole
    /// (with everything inside) the items they may see now, top-level or
    /// not: a folder moved out of a space closed to them comes with its
    /// lists, which didn't change and so aren't in the change. None when
    /// this revision goes whole.
    fn delta_for(&self, member: u64, was_hidden: &[u64]) -> Option<(String, Vec<u64>)> {
        let delta = self.delta.as_ref()?;
        let hidden = wb_sync::hidden_from(&self.data, member, self.managers.contains(&member));
        let gone = self.left_out(&hidden);
        let still: std::collections::HashSet<u64> = gone.iter().copied().collect();
        let back: Vec<u64> = was_hidden
            .iter()
            .filter(|i| !still.contains(i))
            .copied()
            .collect();
        Some(self.made((true, hidden.clone(), back.clone()), || {
            let mut d = delta.clone();
            wb_sync::hide(&mut d, &hidden);
            wb_sync::include_whole(&mut d, &self.data, &back);
            (
                Msg::Delta {
                    data: d,
                    rev: self.rev,
                    base: self.rev - 1,
                    hidden,
                }
                .to_json(),
                gone,
            )
        }))
    }
}

/// `Msg::Update` for `data`, written from a borrowed copy: the same message
/// without copying the whole workspace first.
fn update_json(data: &Value, rev: u64, hidden: &[u64]) -> String {
    #[derive(Serialize)]
    #[serde(tag = "type", rename = "update")]
    struct Update<'a> {
        data: &'a Value,
        rev: u64,
        #[serde(skip_serializing_if = "<[u64]>::is_empty")]
        hidden: &'a [u64],
    }
    serde_json::to_string(&Update { data, rev, hidden }).unwrap_or_default()
}

/// `Msg::Welcome`, written from a borrowed copy (see `update_json`).
#[allow(clippy::too_many_arguments)]
fn welcome_json(
    tag: u16,
    snapshot: Option<&Value>,
    rev: u64,
    member: u64,
    role: Role,
    token: Option<String>,
    email: Option<String>,
    hidden: &[u64],
) -> String {
    #[derive(Serialize)]
    #[serde(tag = "type", rename = "welcome")]
    struct Welcome<'a> {
        tag: u16,
        snapshot: Option<&'a Value>,
        rev: u64,
        member: u64,
        role: Role,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        email: Option<String>,
        #[serde(skip_serializing_if = "<[u64]>::is_empty")]
        hidden: &'a [u64],
        deltas: bool,
    }
    serde_json::to_string(&Welcome {
        tag,
        snapshot,
        rev,
        member,
        role,
        token,
        email,
        hidden,
        deltas: true,
    })
    .unwrap_or_default()
}

struct Hub {
    home: PathBuf,
    /// The server folder; the same as `home` for the first workspace. Setup
    /// files (mail.json, apns.json, livekit.json) missing from `home` come from here.
    root: PathBuf,
    /// None for the server's first workspace, else its id: it lives in
    /// root/workspaces/<id> and apps reach it at /w/<id> (see registry.rs).
    id: Option<String>,
    /// Serialize disk snapshots with the email-invite commit (lock before inner).
    persistence: Mutex<()>,
    inner: Mutex<Inner>,
    updates: broadcast::Sender<Arc<Canon>>,
    /// device ids whose access was just removed
    kicks: broadcast::Sender<Arc<Vec<String>>>,
    /// the notifications a merge called for, to the watch connections
    /// (each picks out its member's; ws.rs)
    notes: broadcast::Sender<Arc<Vec<push::Note>>>,
    /// roles or invites changed: each connection re-sends access info,
    /// except the connection whose request made the change (its answer
    /// already says it); `EVERYONE` when no connection's request did
    access: broadcast::Sender<u64>,
    stop: watch::Receiver<bool>,
    connections: AtomicUsize,
    /// password hashes being worked out (see HashSlot)
    hashing: AtomicUsize,
    /// the largest file /files takes
    max_file: u64,
    claim_wait: Duration,
    /// sends iPhone notifications; None when push isn't set up
    pusher: Option<Arc<dyn push::Sender>>,
    /// sends outgoing webhooks and slash commands (hooks.rs)
    poster: Arc<dyn hooks::Poster>,
    /// asks the AI for the apps' AI tools; None when ai.json has no key
    ai: Option<ai::AiSetup>,
    /// how much each person asks the AI (ai.rs)
    ai_allowance: Arc<ai::Allowance>,
    /// device id -> its open connections (connected devices get no pushes)
    online: Mutex<std::collections::HashMap<String, usize>>,
    /// member id -> their open connections; `presence` holds its keys
    members_online: Mutex<BTreeMap<u64, usize>>,
    /// the members with an open connection, sorted: each connection tells
    /// its app when it changes (the green dot by someone's name)
    presence: watch::Sender<Vec<u64>>,
    /// someone typing in a conversation: each connection passes it on to
    /// its app when that member can see the conversation (ws.rs)
    typing: broadcast::Sender<Typing>,
    /// a member's own reading state changed (mine.rs): their connections
    /// send it to their apps
    mines: broadcast::Sender<(u64, Arc<Value>)>,
    /// true once the workspace is deleted (registry.rs): its connections
    /// end, telling their apps, and nothing more is written to its folder
    closed: watch::Sender<bool>,
}

impl Hub {
    /// A workspace's hub, its copy and state read from `home`.
    fn open(
        home: PathBuf,
        root: PathBuf,
        id: Option<String>,
        options: &Options,
        pusher: Option<Arc<dyn push::Sender>>,
        stop: watch::Receiver<bool>,
    ) -> std::io::Result<Hub> {
        std::fs::create_dir_all(&home)?;
        let canon = read_json(&home.join(WORKSPACE_FILE))?;
        let saved = load_saved(&home)?;
        Ok(Hub {
            home,
            root,
            id,
            persistence: Mutex::new(()),
            inner: Mutex::new(Inner {
                canon: canon.map(Arc::new),
                saved,
                dirty: false,
                mine_dirty: false,
                canon_due: None,
                resets: Default::default(),
                mailed: Default::default(),
                fails: Default::default(),
                links: Default::default(),
                link_fails: (0, 0),
                summary: None,
            }),
            updates: broadcast::channel(64).0,
            kicks: broadcast::channel(16).0,
            notes: broadcast::channel(64).0,
            access: broadcast::channel(16).0,
            stop,
            connections: AtomicUsize::new(0),
            hashing: AtomicUsize::new(0),
            max_file: options.max_file,
            claim_wait: options.claim_wait,
            pusher,
            poster: options
                .poster
                .clone()
                .unwrap_or_else(|| Arc::new(hooks::Curl)),
            ai: options.ai.clone(),
            ai_allowance: Default::default(),
            online: Mutex::new(Default::default()),
            members_online: Mutex::new(Default::default()),
            presence: watch::channel(vec![]).0,
            typing: broadcast::channel(64).0,
            mines: broadcast::channel(64).0,
            closed: watch::channel(false).0,
        })
    }

    /// The workspace's state, locked (see `Locked`). A merge on a big
    /// workspace holds it for a while; a connection's task waiting for it
    /// then hands its worker thread's other connections to another thread
    /// (`block_in_place`) instead of stalling them all.
    fn state(&self) -> std::sync::MutexGuard<'_, Inner> {
        match self.inner.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) if on_async_worker() => {
                tokio::task::block_in_place(|| self.inner.locked())
            }
            Err(std::sync::TryLockError::WouldBlock) => self.inner.locked(),
        }
    }

    /// Where apps reach this workspace below the server's address: "" for
    /// the first workspace, "/w/<id>" for the others.
    fn prefix(&self) -> String {
        self.id
            .as_ref()
            .map(|id| format!("/w/{}", id))
            .unwrap_or_default()
    }

    /// The folder a setup file is read from: this workspace's folder when it
    /// has its own, else the server folder.
    fn setup_home(&self, file: &str) -> &Path {
        if self.home.join(file).exists() {
            &self.home
        } else {
            &self.root
        }
    }

    /// The huddle room of a channel or DM in this workspace (channel ids
    /// repeat between workspaces that share a LiveKit server).
    fn room(&self, channel: u64) -> String {
        match &self.id {
            Some(id) => format!("wb-{}-{}", id, channel),
            None => room_name(channel),
        }
    }

    /// The workspace's name, as its copy says.
    fn name(&self) -> String {
        let g = self.state();
        g.canon
            .as_deref()
            .and_then(|c| c.get("workspace"))
            .and_then(|w| w.as_str())
            .unwrap_or("")
            .to_string()
    }

    /// Write everything not on disk yet, now (stopping, a workspace made or
    /// deleted).
    fn flush(&self) {
        self.save(true);
    }

    /// Once a second: write what changed, the team's copy once its changes
    /// settle (see `save`).
    fn save_tick(&self) {
        self.save(false);
    }

    /// Write the team's copy and server.json. A change to people, devices or
    /// settings (`dirty`) is written at once. Changes to the team's copy
    /// come several a second while people work and each write is the whole
    /// copy, so it waits until they stop for SAVE_QUIET (or SAVE_AT_MOST
    /// after the first one), unless `now`; the very first copy goes at once.
    /// Apps keep their own copy, so changes a crash loses meanwhile come
    /// back with their next push.
    fn save(&self, now: bool) {
        let _persistence = self.persistence.locked();
        if *self.closed.borrow() {
            return;
        }
        let first_copy = !self.home.join(WORKSPACE_FILE).exists();
        let (canon, saved) = {
            let mut g = self.state();
            let settled = g.canon_due.is_some_and(|(first, last)| {
                now || first_copy || last.elapsed() >= SAVE_QUIET || first.elapsed() >= SAVE_AT_MOST
            });
            if !g.dirty && !settled && !g.mine_dirty {
                return;
            }
            // only someone's reading state: server.json alone
            let canon_too = g.dirty || settled;
            g.dirty = false;
            g.mine_dirty = false;
            if canon_too {
                g.canon_due = None;
            }
            (
                // written out after the lock is let go
                g.canon.clone().filter(|_| canon_too),
                serde_json::to_string_pretty(&g.saved).unwrap_or_default(),
            )
        };
        let mut ok = true;
        if let Some(c) = canon.map(|c| serde_json::to_string(&*c).unwrap_or_default()) {
            ok &= write_atomic(&self.home.join(WORKSPACE_FILE), &c)
                .map_err(|e| eprintln!("couldn't save {}: {}", WORKSPACE_FILE, e))
                .is_ok();
        }
        let state = self.home.join(STATE_FILE);
        ok &= write_atomic(&state, &saved)
            .map_err(|e| eprintln!("couldn't save {}: {}", STATE_FILE, e))
            .is_ok();
        if !ok {
            self.state().dirty = true;
        }
    }

    fn backup_now(&self) {
        if *self.closed.borrow() {
            return;
        }
        let canon = self.state().canon.clone();
        if let Some(c) = canon.map(|c| serde_json::to_string(&*c).unwrap_or_default()) {
            if let Err(e) = backup(&self.home, &c, chrono::Local::now().naive_local()) {
                eprintln!("couldn't make a backup: {}", e);
            }
        }
    }

    /// Daily: delete chat files nothing names anymore (see collect_garbage),
    /// and forget wrong sign-in tries that no longer count.
    fn clean_up(&self) {
        let canon = {
            let mut g = self.state();
            g.prune_fails(now_ms());
            g.canon.clone()
        }
        .map(|c| serde_json::to_string(&*c).unwrap_or_default());
        let gone = collect_garbage(&self.home, canon.as_deref(), std::time::SystemTime::now());
        if !gone.is_empty() {
            eprintln!("deleted {} file(s) no message names anymore", gone.len());
        }
    }

    /// Tell every connection the team's copy changed: apps that take deltas
    /// get what changed since the last revision, unless `whole` (a restore,
    /// after which apps send their whole copy back).
    fn broadcast_canon(&self, g: &mut Inner, whole: bool) {
        g.saved.rev += 1;
        let now = std::time::Instant::now();
        let first = g.canon_due.map_or(now, |(first, _)| first);
        g.canon_due = Some((first, now));
        if let Some(c) = &g.canon {
            let delta = match &g.summary {
                Some(s) if !whole => {
                    Some(wb_sync::delta(s, c).unwrap_or_else(|| serde_json::json!({})))
                }
                _ => None,
            };
            g.summary = Some(wb_sync::Summary::of(c));
            let _ = self.updates.send(Arc::new(Canon::new(
                c.clone(),
                g.saved.rev,
                delta,
                &g.saved,
            )));
        }
    }
}

/// Settings that tests change; `Options::default()` is what the real server uses.
#[derive(Clone, Debug)]
pub struct Options {
    /// The largest picture or video an app may upload.
    pub max_file: u64,
    /// How long a claim on an empty server holds without its workspace.
    pub claim_wait: Duration,
    /// What sends iPhone notifications. None: APNs as apns.json or WB_APNS_*
    /// say (or nothing when they don't).
    pub push: Option<Arc<dyn push::Sender>>,
    /// What sends outgoing webhooks and slash commands. None: curl.
    pub poster: Option<Arc<dyn hooks::Poster>>,
    /// What answers the apps' AI requests. None: Anthropic's API as ai.json
    /// or ANTHROPIC_API_KEY say (or no AI when they don't).
    pub ai: Option<ai::AiSetup>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            max_file: MAX_FILE,
            claim_wait: Duration::from_secs(CLAIM_WAIT_SECS),
            push: None,
            poster: None,
            ai: None,
        }
    }
}

/// APNs from the server folder's setup; None (said once) when it isn't set up.
fn apns_sender(home: &Path) -> Option<Arc<dyn push::Sender>> {
    match push::load_apns(home).and_then(push::Apns::new) {
        Ok(a) => {
            println!("iPhone notifications on (APNs, topic {})", a.topic());
            Some(Arc::new(a))
        }
        Err(why) => {
            println!("iPhone notifications off: {}", why);
            None
        }
    }
}

/// Serve on `listener` until `shutdown` finishes, keeping the copy in `home`.
pub async fn serve(
    listener: tokio::net::TcpListener,
    home: PathBuf,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    serve_with(listener, home, Options::default(), shutdown).await
}

/// `serve` with other settings.
pub async fn serve_with(
    listener: tokio::net::TcpListener,
    home: PathBuf,
    options: Options,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    serve_on(vec![listener], home, options, shutdown).await
}

/// `serve_with` on several addresses at once, one team (for example
/// 127.0.0.1 for Cloudflare Tunnel's connector and a LAN address).
pub async fn serve_on(
    listeners: Vec<tokio::net::TcpListener>,
    home: PathBuf,
    options: Options,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let (stop_tx, stop_rx) = watch::channel(false);
    let stopping = stop_rx.clone();
    // held until this returns
    let _running = admin::hold_running(&home);
    let registry = Arc::new(registry::Registry::open(home, options, stop_rx)?);
    let hub = registry.first();
    // requests left while the server was off
    for h in registry.all() {
        h.admin_file();
    }
    let worker = {
        let registry = registry.clone();
        tokio::spawn(async move {
            let mut tick: u64 = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                tick += 1;
                let registry = registry.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    for hub in registry.all() {
                        hub.save_tick();
                        hub.post_due();
                        hub.admin_file();
                        if tick % 60 == 1 {
                            hub.backup_now();
                        }
                        // ten minutes after starting (after a backup), then daily
                        if tick % 86_400 == 600 {
                            hub.clean_up();
                        }
                    }
                    registry.admin_file();
                    if tick % 3600 == 900 {
                        for id in registry.clean_up() {
                            println!("removed workspace {}: nobody set it up", id);
                        }
                        for id in registry.purge() {
                            println!(
                                "removed deleted workspace {} for good after {} days",
                                id,
                                registry::KEEP_DELETED_DAYS
                            );
                        }
                    }
                })
                .await;
            }
        })
    };
    let app = Router::new()
        .route(wb_net::PATH, get(ws_route))
        .route("/health", get(health))
        .route("/updates/{file}", get(update_route))
        .route("/download/{os}", get(download_route))
        // get() also answers HEAD. Uploads are read as a stream, which axum's
        // body limit doesn't cover; put_file enforces max_file itself.
        .route("/files/{file}", get(files::get_file).put(files::put_file))
        .route("/auth/forgot", axum::routing::post(forgot_route))
        .route("/hooks/{token}", axum::routing::post(hook_route))
        .route("/join", get(join_route))
        .route("/privacy", get(privacy_route))
        .with_state(hub.clone())
        .merge(registry::routes(registry.clone()));
    tokio::spawn(async move {
        shutdown.await;
        // open connections end themselves when they see this
        let _ = stop_tx.send(true);
    });
    let serving = listeners.into_iter().map(|listener| {
        let mut stopping = stopping.clone();
        std::future::IntoFuture::into_future(
            axum::serve(listener, app.clone()).with_graceful_shutdown(async move {
                let _ = stopping.wait_for(|s| *s).await;
            }),
        )
    });
    let res = futures_util::future::try_join_all(serving)
        .await
        .map(|_| ());
    worker.abort();
    for hub in registry.all() {
        hub.flush();
        hub.backup_now();
    }
    res
}

#[cfg(test)]
mod tests;
