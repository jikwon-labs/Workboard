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
//   outbox/               mails written here instead of sent (mail.json
//                         "transport": "folder")
//   backups/              hourly-YYYYMMDD-HH.json (48 kept),
//                         daily-YYYYMMDD.json (30 kept), before-restore-*.json
//                         (20 kept)
//   admin-request.json    a request from the `workboard-server` admin
//   admin-result.json     commands, picked up within a second (see admin)
//   updates/              app updates: appcast.xml and the Workboard-*.zip it
//                         lists, served at /updates/<file> for Sparkle
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
// also choose a password; the account's email is the one on the person's
// member record. With it the person signs in on any other computer or phone
// without a new code. When mail.json is set up the server emails invite codes
// (with a link to /join) and password-reset codes (POST /auth/forgot).
// Removing someone's access also deletes their account.

use axum::extract::State;
use axum::http::{header, Method, StatusCode, Version};
use futures_util::{SinkExt, StreamExt};
use hyper_util::rt::TokioIo;
use tokio_tungstenite::tungstenite::protocol::{Role as WsRole, WebSocketConfig};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use wb_net::{Backup, Msg, Person, Role, CODE_AUTH, CODE_DENIED, MIN_PASSWORD};

mod files;
pub use files::{file_name_ok, parse_range, FILES_DIR, MAX_FILE};
pub mod mail;
pub use mail::{load_mail, MailCfg, MAIL_FILE, OUTBOX_DIR};
pub mod push;
pub use push::{load_apns, ApnsCfg, APNS_FILE};

pub const WORKSPACE_FILE: &str = "workspace.json";
pub const STATE_FILE: &str = "server.json";
pub const BACKUP_DIR: &str = "backups";
pub const UPDATES_DIR: &str = "updates";
pub const REQUEST_FILE: &str = "admin-request.json";
pub const RESULT_FILE: &str = "admin-result.json";
/// LiveKit address and keys for voice huddles (written by deploy/install-livekit.sh).
pub const LIVEKIT_FILE: &str = "livekit.json";
/// How long a call token lets someone into a huddle.
pub const CALL_TOKEN_SECS: i64 = 6 * 3600;
pub const KEEP_HOURLY: usize = 48;
pub const KEEP_DAILY: usize = 30;
/// Invite codes stop working after this long.
pub const INVITE_DAYS: i64 = 7;
/// Password-reset codes stop working after this long.
pub const RESET_MINUTES: i64 = 30;
/// Wrong passwords or reset codes in a row before an email is locked...
pub const MAX_TRIES: u32 = 5;
/// ...for this long.
pub const LOCK_MINUTES: i64 = 15;
/// PBKDF2-HMAC-SHA256 rounds for new password hashes.
const PBKDF2_ROUNDS: u32 = 100_000;
/// before-restore-*.json copies kept (the newest ones).
pub const KEEP_BEFORE_RESTORE: usize = 20;
/// A claim on an empty server that never brought its workspace (the app quit
/// right after) gives way to another claim after this long.
pub const CLAIM_WAIT_SECS: u64 = 60;
/// The largest first message (hello) before someone has signed in.
pub const MAX_HELLO: usize = 64 << 10;

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

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Bytes from the system's secure random source.
fn random_bytes<const N: usize>() -> [u8; N] {
    use std::io::Read;
    let mut b = [0u8; N];
    std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut b)).expect("read /dev/urandom");
    b
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(token.as_bytes()))
}

/// PBKDF2 with HMAC-SHA256, one 32-byte block.
fn pbkdf2_sha256(password: &[u8], salt: &[u8], rounds: u32) -> [u8; 32] {
    use hmac::{KeyInit, Mac};
    let prf = hmac::Hmac::<sha2::Sha256>::new_from_slice(password).expect("any key length");
    let mut mac = prf.clone();
    mac.update(salt);
    mac.update(&1u32.to_be_bytes());
    let mut u: [u8; 32] = mac.finalize().into_bytes().into();
    let mut out = u;
    for _ in 1..rounds {
        let mut mac = prf.clone();
        mac.update(&u);
        u = mac.finalize().into_bytes().into();
        for (o, x) in out.iter_mut().zip(u.iter()) {
            *o ^= x;
        }
    }
    out
}

/// Emails compare without case or surrounding spaces.
fn norm_email(e: &str) -> String {
    e.trim().to_lowercase()
}

fn email_ok(e: &str) -> bool {
    let e = e.trim();
    match e.split_once('@') {
        Some((a, b)) => !a.is_empty() && b.contains('.') && !b.starts_with('.') && !b.ends_with('.') && !e.contains(char::is_whitespace),
        None => false,
    }
}

fn password_problem(p: &str) -> Option<String> {
    (p.chars().count() < MIN_PASSWORD).then(|| format!("Choose a password of at least {} characters.", MIN_PASSWORD))
}

/// Compare without leaking where two strings differ.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// XXXX-XXXX-XXXX without look-alike letters.
fn new_code() -> String {
    const ABC: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let b = random_bytes::<12>();
    let s: String = b.iter().map(|x| ABC[*x as usize % ABC.len()] as char).collect();
    format!("{}-{}-{}", &s[0..4], &s[4..8], &s[8..12])
}

/// What people type: any case, with or without dashes and spaces.
fn normalize_code(c: &str) -> String {
    c.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

// ------------------------------------------------------------------ voice

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LiveKitCfg {
    /// What apps connect to, e.g. ws://100.101.102.103:7880
    pub url: String,
    pub key: String,
    pub secret: String,
}

pub fn load_livekit(home: &Path) -> Option<LiveKitCfg> {
    serde_json::from_str(&std::fs::read_to_string(home.join(LIVEKIT_FILE)).ok()?).ok()
}

fn b64url(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..(c.len() + 1) {
            out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// A LiveKit access token (a JWT signed with HS256) that lets `identity`
/// join `room` to talk and listen, for CALL_TOKEN_SECS.
pub fn call_token(cfg: &LiveKitCfg, room: &str, identity: &str, name: &str, now_secs: i64) -> String {
    use hmac::{KeyInit, Mac};
    let header = b64url(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = json!({
        "iss": cfg.key, "sub": identity, "name": name,
        "nbf": now_secs - 10, "exp": now_secs + CALL_TOKEN_SECS,
        "video": {"room": room, "roomJoin": true, "canPublish": true, "canSubscribe": true, "canPublishData": true},
    });
    let body = b64url(claims.to_string().as_bytes());
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(cfg.secret.as_bytes()).expect("any key length");
    mac.update(format!("{}.{}", header, body).as_bytes());
    format!("{}.{}.{}", header, body, b64url(&mac.finalize().into_bytes()))
}

/// The huddle room of a channel or DM.
pub fn room_name(channel: u64) -> String {
    format!("wb-{}", channel)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Device {
    pub tag: u16,
    pub member: u64,
    /// sha256 of the device token (the token itself only lives on the device)
    pub token: String,
    pub created: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InviteRec {
    pub member: u64,
    pub role: Role,
    pub created: i64,
}

/// A person's account: the email and password that sign them in anywhere.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub email: String,
    /// hex
    pub salt: String,
    /// hex PBKDF2-HMAC-SHA256 of the password
    pub hash: String,
    pub rounds: u32,
    pub created: i64,
}

impl Account {
    fn new(email: &str, password: &str) -> Account {
        let salt = random_bytes::<16>();
        Account { email: email.trim().to_string(), salt: hex(&salt), hash: hex(&pbkdf2_sha256(password.as_bytes(), &salt, PBKDF2_ROUNDS)), rounds: PBKDF2_ROUNDS, created: now_ms() }
    }

    fn check(&self, password: &str) -> bool {
        let salt: Vec<u8> = (0..self.salt.len() / 2).filter_map(|i| u8::from_str_radix(&self.salt[2 * i..2 * i + 2], 16).ok()).collect();
        same(&hex(&pbkdf2_sha256(password.as_bytes(), &salt, self.rounds)), &self.hash)
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Saved {
    pub rev: u64,
    /// device id -> device
    pub devices: BTreeMap<String, Device>,
    /// member -> role (members not listed are plain members)
    pub roles: BTreeMap<u64, Role>,
    /// code (as shown, XXXX-XXXX-XXXX) -> invite
    pub invites: BTreeMap<String, InviteRec>,
    /// member -> account
    pub accounts: BTreeMap<u64, Account>,
    /// device id -> its push token (iPhones; see push.rs)
    pub push: BTreeMap<String, push::PushToken>,
}

impl Saved {
    pub fn role_of(&self, member: u64) -> Role {
        self.roles.get(&member).copied().unwrap_or_default()
    }

    fn set_role(&mut self, member: u64, role: Role) {
        if role == Role::Member {
            self.roles.remove(&member);
        } else {
            self.roles.insert(member, role);
        }
    }

    fn free_tag(&self, asked: Option<u16>) -> u16 {
        let used: Vec<u16> = self.devices.values().map(|d| d.tag).collect();
        let mut tag = asked.map(|t| t % 1024).unwrap_or(u16::from_le_bytes(random_bytes::<2>()) % 1024);
        for _ in 0..1024 {
            if !used.contains(&tag) {
                break;
            }
            tag = (tag + 1) % 1024;
        }
        tag
    }

    /// Sign a device in as `member`; returns (tag, token).
    fn add_device(&mut self, device: &str, member: u64, asked_tag: Option<u16>) -> (u16, String) {
        let old = self.devices.remove(device);
        let tag = old.map(|d| d.tag).unwrap_or_else(|| self.free_tag(asked_tag));
        let token = hex(&random_bytes::<32>());
        self.devices.insert(device.to_string(), Device { tag, member, token: hash_token(&token), created: now_ms() });
        (tag, token)
    }

    /// Keep (or with an empty token, forget) `device`'s push token. True when
    /// that changed anything.
    fn set_push_token(&mut self, device: &str, token: &str, platform: &str, env: &str) -> bool {
        let token = token.trim().to_ascii_lowercase();
        if token.is_empty() {
            return self.push.remove(device).is_some();
        }
        if !self.devices.contains_key(device) || !push::token_ok(&token) || !platform.trim().eq_ignore_ascii_case("ios") {
            return false;
        }
        let env = push::norm_env(env);
        if self.push.get(device).is_some_and(|t| t.token == token && t.env == env) {
            return false;
        }
        // one phone, one entry: a token that moved to a new device id (the app
        // was installed again) leaves the old one
        self.push.retain(|d, t| d == device || t.token != token);
        self.push.insert(device.to_string(), push::PushToken { token, platform: "ios".into(), env: env.into(), updated: now_ms() });
        true
    }

    fn drop_expired_invites(&mut self, now: i64) {
        self.invites.retain(|_, i| invite_alive(i, now));
    }

    /// The code `member` can still sign in with (expired ones don't count).
    pub fn open_invite(&self, member: u64, now: i64) -> Option<&String> {
        self.invites.iter().find(|(_, i)| i.member == member && invite_alive(i, now)).map(|(c, _)| c)
    }

    /// A fresh one-time code for `member`, replacing any earlier one.
    pub fn make_invite(&mut self, member: u64, role: Role) -> String {
        self.invites.retain(|_, i| i.member != member);
        let code = new_code();
        self.invites.insert(code.clone(), InviteRec { member, role, created: now_ms() });
        code
    }

    fn find_invite(&mut self, typed: &str) -> Option<String> {
        self.drop_expired_invites(now_ms());
        let want = normalize_code(typed);
        self.invites.keys().find(|k| normalize_code(k) == want).cloned()
    }

    #[cfg(test)]
    fn take_invite(&mut self, typed: &str) -> Option<InviteRec> {
        let key = self.find_invite(typed)?;
        self.invites.remove(&key)
    }

    /// The member whose account has this email.
    pub fn account_member(&self, email: &str) -> Option<u64> {
        let want = norm_email(email);
        self.accounts.iter().find(|(_, a)| norm_email(&a.email) == want).map(|(m, _)| *m)
    }

    /// Why `member` can't have an account with `email`, if they can't.
    fn account_problem(&self, member: u64, email: &str, password: &str) -> Option<String> {
        if !email_ok(email) {
            return Some("Type a valid email address.".into());
        }
        if self.account_member(email).is_some_and(|m| m != member) {
            return Some("Someone else in this workspace already signs in with that email.".into());
        }
        password_problem(password)
    }

    /// Everyone the workspace knows, with their access, as `viewer` may see
    /// it: a code for the owner signs in as the owner, so only the owner sees it.
    fn people(&self, canon: Option<&Value>, viewer: Role) -> Vec<Person> {
        let mut ids: Vec<u64> = canon.and_then(|c| c.get("members")).and_then(|m| m.as_array()).map(|a| a.iter().filter_map(|m| m.get("id")?.as_u64()).collect()).unwrap_or_default();
        for d in self.devices.values() {
            if !ids.contains(&d.member) {
                ids.push(d.member);
            }
        }
        ids.iter()
            .map(|&member| Person {
                member,
                role: self.role_of(member),
                devices: self.devices.values().filter(|d| d.member == member).count(),
                invite: self.open_invite(member, now_ms()).cloned().filter(|_| viewer == Role::Owner || self.role_of(member) != Role::Owner),
                account: self.accounts.contains_key(&member),
            })
            .collect()
    }
}

fn invite_alive(i: &InviteRec, now: i64) -> bool {
    now - i.created < INVITE_DAYS * 86_400_000
}

fn member_ids(canon: Option<&Value>) -> Vec<u64> {
    canon.and_then(|c| c.get("members")).and_then(|m| m.as_array()).map(|a| a.iter().filter_map(|m| m.get("id")?.as_u64()).collect()).unwrap_or_default()
}

/// A text field of a member in the team's copy ("name", "email").
fn member_field(canon: Option<&Value>, member: u64, field: &str) -> String {
    canon
        .and_then(|c| c.get("members"))
        .and_then(|m| m.as_array())
        .and_then(|a| a.iter().find(|m| m.get("id").and_then(|i| i.as_u64()) == Some(member)))
        .and_then(|m| m.get(field))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn member_exists(canon: Option<&Value>, member: u64) -> bool {
    canon.and_then(|c| c.get("members")).and_then(|m| m.as_array()).is_some_and(|a| a.iter().any(|m| m.get("id").and_then(|i| i.as_u64()) == Some(member)))
}

/// Make every item in a restored copy win over what the apps have, and bring
/// back the items deleted since: their change stamps become `now` and each
/// one counts as restored.
pub fn mark_restored(data: &mut Value, now: i64) {
    if !copy_shape_ok(data) {
        return;
    }
    let ids: Vec<u64> = wb_sync::fingerprints(data).into_keys().collect();
    let mut meta: wb_sync::SyncMeta = data.get("extra").and_then(|e| e.get("sync")).and_then(|s| serde_json::from_value(s.clone()).ok()).unwrap_or_default();
    for id in ids {
        meta.stamps.insert(id, now);
        meta.revived.insert(id, now);
    }
    meta.hashes.clear();
    if data.get("extra").is_none() {
        data["extra"] = json!({});
    }
    data["extra"]["sync"] = serde_json::to_value(&meta).unwrap_or_default();
}

/// Write a member's role into the team's copy so everyone's app shows it.
fn label_role(canon: &mut Value, member: u64, role: Role) -> bool {
    if !copy_shape_ok(canon) {
        return false;
    }
    let Some(m) = canon.get_mut("members").and_then(|m| m.as_array_mut()).and_then(|a| a.iter_mut().find(|m| m.get("id").and_then(|i| i.as_u64()) == Some(member))) else { return false };
    if m.get("role").and_then(|r| r.as_str()) == Some(role.label()) {
        return false;
    }
    m["role"] = json!(role.label());
    let stamps = &mut canon["extra"]["sync"]["stamps"];
    if !stamps.is_object() {
        *stamps = json!({});
    }
    let prev = stamps.get(member.to_string()).and_then(|s| s.as_i64()).unwrap_or(0);
    stamps[member.to_string()] = json!(now_ms().max(prev + 1));
    true
}

/// A workspace copy the server can work with: an object whose `extra`,
/// `extra.sync` and `extra.sync.stamps` are objects (or missing), so writing
/// into them can't fail.
fn copy_shape_ok(v: &Value) -> bool {
    let fine = |x: Option<&Value>| x.is_none_or(|x| x.is_null() || x.is_object());
    let extra = v.get("extra");
    let sync = extra.and_then(|e| e.get("sync"));
    v.is_object() && fine(extra) && fine(sync) && fine(sync.and_then(|s| s.get("stamps")))
}

/// A DM or a private channel with a member list that doesn't include
/// `member`: they don't get it and can't change it.
fn closed_to(ch: &Value, member: u64) -> bool {
    let restricted = ch.get("kind").and_then(|k| k.as_str()) == Some("Direct") || ch.get("private").and_then(|p| p.as_bool()) == Some(true);
    let members: Vec<u64> = ch.get("members").and_then(|m| m.as_array()).map(|a| a.iter().filter_map(|m| m.as_u64()).collect()).unwrap_or_default();
    restricted && !members.is_empty() && !members.contains(&member)
}

/// The conversations in the team's copy closed to `member`, each with the
/// ids of everything in it (the channel, its messages and replies).
fn closed_channels(canon: &Value, member: u64) -> Vec<(&Value, Vec<u64>)> {
    canon
        .get("channels")
        .and_then(|c| c.as_array())
        .map(|a| a.iter().filter(|ch| ch.get("id").and_then(|i| i.as_u64()).is_some() && closed_to(ch, member)).map(|ch| (ch, wb_sync::fingerprints(&json!({ "channels": [ch] })).into_keys().collect())).collect())
        .unwrap_or_default()
}

fn drop_ids(meta: Option<&mut Value>, keys: &[&str], ids: &[u64]) {
    let Some(meta) = meta else { return };
    for k in keys {
        if let Some(m) = meta.get_mut(*k).and_then(|m| m.as_object_mut()) {
            for id in ids {
                m.remove(&id.to_string());
            }
        }
    }
}

/// What `member` gets of the team's copy: DMs and private channels they
/// aren't in left out (with their change stamps). None when nothing is
/// left out. Tombstones stay, so deletes still reach everyone; an app never
/// treats a conversation missing here as deleted (only a tombstone deletes).
fn copy_for(canon: &Value, member: u64) -> Option<Value> {
    let closed = closed_channels(canon, member);
    if closed.is_empty() {
        return None;
    }
    let hide: Vec<u64> = closed.iter().filter_map(|(ch, _)| ch.get("id")?.as_u64()).collect();
    let inner: Vec<u64> = closed.iter().flat_map(|(_, ids)| ids.iter().copied()).collect();
    let mut out = canon.clone();
    if let Some(a) = out.get_mut("channels").and_then(|c| c.as_array_mut()) {
        a.retain(|ch| !ch.get("id").and_then(|i| i.as_u64()).is_some_and(|i| hide.contains(&i)));
    }
    drop_ids(out.get_mut("extra").and_then(|e| e.get_mut("sync")), &["stamps"], &inner);
    Some(out)
}

/// Before merging a copy from `member`: the conversations closed to them
/// stay as the team has them. Their version of one (say, with themselves
/// added to its members) is swapped for the team's, and their stamps,
/// tombstones and revivals for anything in it are dropped, so they can't
/// change, join or delete it.
fn guard_push(canon: &Value, data: &mut Value, member: u64) {
    for (ch, ids) in closed_channels(canon, member) {
        let id = ch.get("id").and_then(|i| i.as_u64());
        if let Some(theirs) = data.get_mut("channels").and_then(|c| c.as_array_mut()).and_then(|a| a.iter_mut().find(|c| c.get("id").and_then(|i| i.as_u64()) == id)) {
            *theirs = ch.clone();
        }
        drop_ids(data.get_mut("extra").and_then(|e| e.get_mut("sync")), &["stamps", "tombs", "revived"], &ids);
    }
}

fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

fn read_json(path: &Path) -> std::io::Result<Option<Value>> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).map(Some).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}: {}", path.display(), e))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Load server.json; one from stage 1 (devices without tokens) keeps only its
/// revision, so those computers have to be invited again.
pub fn load_saved(home: &Path) -> std::io::Result<Saved> {
    Ok(match read_json(&home.join(STATE_FILE))? {
        None => Saved::default(),
        Some(v) => serde_json::from_value(v.clone()).unwrap_or_else(|_| Saved { rev: v.get("rev").and_then(|r| r.as_u64()).unwrap_or(0), ..Default::default() }),
    })
}

// ------------------------------------------------------------------ backups

/// Copy the team's copy into backups/ once an hour and once a day (only when
/// it differs from the newest copy of that kind), keeping the newest
/// KEEP_HOURLY and KEEP_DAILY. Returns the files written.
pub fn backup(home: &Path, canon_json: &str, now: chrono::NaiveDateTime) -> std::io::Result<Vec<PathBuf>> {
    let dir = home.join(BACKUP_DIR);
    std::fs::create_dir_all(&dir)?;
    let mut written = vec![];
    for (kind, name, keep) in [("hourly", now.format("hourly-%Y%m%d-%H.json").to_string(), KEEP_HOURLY), ("daily", now.format("daily-%Y%m%d.json").to_string(), KEEP_DAILY)] {
        let mut files = backup_files(&dir, kind);
        let path = dir.join(&name);
        let same_as_newest = files.last().and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|s| s == canon_json);
        if !path.exists() && !same_as_newest {
            write_atomic(&path, canon_json)?;
            written.push(path.clone());
            files.push(path);
            files.sort();
        }
        while files.len() > keep {
            let _ = std::fs::remove_file(files.remove(0));
        }
    }
    Ok(written)
}

/// Backups of one kind ("hourly", "daily"), oldest first.
pub fn backup_files(dir: &Path, kind: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&format!("{}-", kind)) && n.ends_with(".json"))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

// --------------------------------------------------------------------- hub

struct Inner {
    canon: Option<Value>,
    saved: Saved,
    dirty: bool,
    /// email (normalized) -> (reset code, when made); kept in memory only
    resets: std::collections::HashMap<String, (String, i64)>,
    /// email (normalized) -> (wrong tries in a row, time of the last one)
    fails: std::collections::HashMap<String, (u32, i64)>,
}

impl Inner {
    /// Refuse while an email has had too many wrong tries.
    fn locked(&self, email: &str) -> Option<String> {
        let (n, last) = self.fails.get(&norm_email(email)).copied()?;
        (n >= MAX_TRIES && now_ms() - last < LOCK_MINUTES * 60_000).then(|| format!("Too many wrong tries. Wait {} minutes and try again, or use Forgot password.", LOCK_MINUTES))
    }

    fn failed(&mut self, email: &str) {
        let now = now_ms();
        let e = self.fails.entry(norm_email(email)).or_insert((0, now));
        if now - e.1 >= LOCK_MINUTES * 60_000 {
            e.0 = 0;
        }
        e.0 += 1;
        e.1 = now;
    }
}

/// What a hello asked for.
#[derive(Default)]
struct Hello {
    device: String,
    tag: Option<u16>,
    token: Option<String>,
    invite: Option<String>,
    claim: Option<u64>,
    email: Option<String>,
    password: Option<String>,
    new_password: Option<String>,
    reset_code: Option<String>,
}

fn given(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|v| !v.trim().is_empty())
}

/// What a request from an app turned into.
enum Reply {
    To(Msg),
    Denied(String),
}

/// A new team copy for every connection; each one sends what its person may see.
struct Canon {
    data: Value,
    rev: u64,
    /// the whole copy as an update message, for people nothing is hidden from
    json: String,
}

impl Canon {
    fn new(data: &Value, rev: u64) -> Canon {
        Canon { json: Msg::Update { data: data.clone(), rev }.to_json(), data: data.clone(), rev }
    }

    fn update_for(&self, member: u64) -> String {
        match copy_for(&self.data, member) {
            Some(data) => Msg::Update { data, rev: self.rev }.to_json(),
            None => self.json.clone(),
        }
    }
}

struct Hub {
    home: PathBuf,
    inner: Mutex<Inner>,
    updates: broadcast::Sender<Arc<Canon>>,
    /// device ids whose access was just removed
    kicks: broadcast::Sender<Arc<Vec<String>>>,
    /// roles or invites changed: each connection re-sends access info
    access: broadcast::Sender<()>,
    stop: watch::Receiver<bool>,
    connections: AtomicUsize,
    /// the largest file /files takes
    max_file: u64,
    claim_wait: Duration,
    /// sends iPhone notifications; None when push isn't set up
    pusher: Option<Arc<dyn push::Sender>>,
    /// device id -> its open connections (connected devices get no pushes)
    online: Mutex<std::collections::HashMap<String, usize>>,
}

impl Hub {
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
        let state = self.home.join(STATE_FILE);
        ok &= write_atomic(&state, &saved).map_err(|e| eprintln!("couldn't save {}: {}", STATE_FILE, e)).is_ok();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o600));
        }
        if !ok {
            self.inner.lock().unwrap().dirty = true;
        }
    }

    fn backup_now(&self) {
        let canon = self.inner.lock().unwrap().canon.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default());
        if let Some(c) = canon {
            if let Err(e) = backup(&self.home, &c, chrono::Local::now().naive_local()) {
                eprintln!("couldn't make a backup: {}", e);
            }
        }
    }

    fn broadcast_canon(&self, g: &mut Inner) {
        g.saved.rev += 1;
        g.dirty = true;
        if let Some(c) = &g.canon {
            let _ = self.updates.send(Arc::new(Canon::new(c, g.saved.rev)));
        }
    }

    /// Check a hello. Ok: (member, role, tag, new token).
    fn sign_in(&self, h: &Hello) -> Result<(u64, Role, u16, Option<String>), String> {
        let mut g = self.inner.lock().unwrap();
        let device = h.device.as_str();
        // a password-reset code from the email POST /auth/forgot sent
        if let Some(code) = given(&h.reset_code) {
            let email = given(&h.email).ok_or("Type the email you sign in with.")?;
            if let Some(why) = g.locked(email) {
                return Err(why);
            }
            let pw = h.new_password.as_deref().unwrap_or("");
            if let Some(why) = password_problem(pw) {
                return Err(why);
            }
            let key = norm_email(email);
            let fresh = g.resets.get(&key).is_some_and(|(c, t)| same(&normalize_code(c), &normalize_code(code)) && now_ms() - t < RESET_MINUTES * 60_000);
            let member = g.saved.account_member(email).filter(|_| fresh);
            let Some(member) = member else {
                g.failed(email);
                return Err(format!("That reset code doesn't work. It may be used already or expired (codes last {} minutes). Ask for a new one.", RESET_MINUTES));
            };
            g.resets.remove(&key);
            g.fails.remove(&key);
            let kept = g.saved.accounts[&member].email.clone();
            g.saved.accounts.insert(member, Account::new(&kept, pw));
            return Ok(self.signed_in(g, device, member, h.tag));
        }
        if let Some(code) = given(&h.invite) {
            let Some(key) = g.saved.find_invite(code) else {
                return Err("That invite code doesn't work. It may be used already or expired (codes last 7 days). Ask for a new one.".into());
            };
            let member = g.saved.invites[&key].member;
            // choosing a password: check it all before the code is used up
            let account = match h.new_password.as_deref() {
                Some(pw) => {
                    let email = given(&h.email).map(|e| e.trim().to_string()).unwrap_or_else(|| member_field(g.canon.as_ref(), member, "email"));
                    if email.is_empty() {
                        return Err("Type your email too: you sign in with it on your other computers and phones.".into());
                    }
                    if let Some(why) = g.saved.account_problem(member, &email, pw) {
                        return Err(why);
                    }
                    Some(Account::new(&email, pw))
                }
                None => None,
            };
            let inv = g.saved.invites.remove(&key).expect("found above");
            if let Some(a) = account {
                g.saved.accounts.insert(member, a);
            }
            if inv.role > g.saved.role_of(member) {
                g.saved.set_role(member, inv.role);
            }
            return Ok(self.signed_in(g, device, member, h.tag));
        }
        if let (Some(email), Some(pw)) = (given(&h.email), h.password.as_deref()) {
            if let Some(why) = g.locked(email) {
                return Err(why);
            }
            let found = g.saved.account_member(email);
            let ok = match found {
                Some(m) => g.saved.accounts[&m].check(pw),
                None => {
                    // as slow as a real check, so the answer doesn't tell who has an account
                    let _ = Account::new("x@x.x", pw);
                    false
                }
            };
            let Some(member) = found.filter(|_| ok) else {
                g.failed(email);
                return Err("That email and password don't match an account on this server.".into());
            };
            g.fails.remove(&norm_email(email));
            return Ok(self.signed_in(g, device, member, h.tag));
        }
        if let Some(token) = &h.token {
            return match g.saved.devices.get(device) {
                Some(d) if same(&d.token, &hash_token(token)) => Ok((d.member, g.saved.role_of(d.member), d.tag, None)),
                _ => Err("This computer isn't signed in to this server anymore. Sign in with your email and password, or ask the owner or an admin for a new invite code.".into()),
            };
        }
        if let Some(member) = h.claim {
            // only the very first claim wins: two people setting up the same
            // empty server at once must not end up with their workspaces merged.
            // A claim that never brought its workspace (the app quit right
            // after) can be made again from the same computer, and gives way
            // to anyone after claim_wait.
            if g.canon.is_none() && !g.saved.devices.is_empty() {
                let same_device = g.saved.devices.len() == 1 && g.saved.devices.contains_key(device);
                let stale = g.saved.devices.values().all(|d| now_ms() - d.created >= self.claim_wait.as_millis() as i64);
                if !same_device && !stale {
                    return Err("Someone else is setting up this server right now. Ask them for an invite code.".into());
                }
                let gone: Vec<String> = g.saved.devices.keys().filter(|d| d.as_str() != device).cloned().collect();
                g.saved.devices.clear();
                g.saved.roles.clear();
                g.saved.invites.clear();
                g.saved.accounts.clear();
                if !gone.is_empty() {
                    let _ = self.kicks.send(Arc::new(gone));
                }
            }
            if g.canon.is_none() && g.saved.devices.is_empty() {
                if let Some(pw) = h.new_password.as_deref() {
                    let email = given(&h.email).unwrap_or("").trim().to_string();
                    if let Some(why) = g.saved.account_problem(member, &email, pw) {
                        return Err(why);
                    }
                    g.saved.accounts.insert(member, Account::new(&email, pw));
                }
                g.saved.set_role(member, Role::Owner);
                return Ok(self.signed_in(g, device, member, h.tag));
            }
        }
        Err("This server already has a workspace. Sign in with your email and password, or ask its owner or an admin for an invite code.".into())
    }

    /// Give `device` a new token for `member`.
    fn signed_in(&self, mut g: std::sync::MutexGuard<'_, Inner>, device: &str, member: u64, tag: Option<u16>) -> (u64, Role, u16, Option<String>) {
        let (tag, token) = g.saved.add_device(device, member, tag);
        g.dirty = true;
        let role = g.saved.role_of(member);
        drop(g);
        let _ = self.access.send(());
        (member, role, tag, Some(token))
    }

    /// Set up `member`'s account or change its password.
    fn set_password(&self, member: u64, email: &str, password: &str, old: Option<&str>) -> Result<Msg, String> {
        let mut g = self.inner.lock().unwrap();
        if let Some(existing) = g.saved.accounts.get(&member).cloned() {
            if let Some(why) = g.locked(&existing.email) {
                return Err(why);
            }
            if !existing.check(old.unwrap_or("")) {
                g.failed(&existing.email);
                return Err("Your current password isn't right.".into());
            }
        }
        if let Some(why) = g.saved.account_problem(member, email, password) {
            return Err(why);
        }
        g.saved.accounts.insert(member, Account::new(email, password));
        g.dirty = true;
        drop(g);
        let _ = self.access.send(());
        Ok(Msg::Account { email: email.trim().to_string() })
    }

    /// Email a password-reset code when `email` has an account (quietly
    /// does nothing otherwise). False when this server can't send email.
    fn forgot(&self, email: &str) -> bool {
        let Some(cfg) = load_mail(&self.home) else { return false };
        if !email_ok(email) {
            return true;
        }
        let target = {
            let mut g = self.inner.lock().unwrap();
            let key = norm_email(email);
            let now = now_ms();
            // at most one mail a minute per address
            let recent = g.resets.get(&key).is_some_and(|(_, t)| now - t < 60_000);
            match g.saved.account_member(email) {
                Some(m) if !recent => {
                    let code = new_code();
                    g.resets.insert(key, (code.clone(), now));
                    Some((g.saved.accounts[&m].email.clone(), code))
                }
                _ => None,
            }
        };
        if let Some((to, code)) = target {
            let home = self.home.clone();
            std::thread::spawn(move || {
                if let Err(e) = mail::send(&home, &cfg, &mail::reset_mail(&to, &code, RESET_MINUTES)) {
                    eprintln!("couldn't email a reset code to {}: {}", to, e);
                }
            });
        }
        true
    }

    /// Email `who`'s new invite code, when mail is set up and they have an
    /// email. (emailed to, why it failed)
    fn email_invite(&self, from: u64, who: u64, code: &str, connected_to: Option<&str>) -> (Option<String>, Option<String>) {
        let Some(cfg) = load_mail(&self.home) else { return (None, None) };
        let (to, workspace, inviter) = {
            let g = self.inner.lock().unwrap();
            let c = g.canon.as_ref();
            (member_field(c, who, "email"), c.and_then(|c| c.get("workspace")).and_then(|w| w.as_str()).unwrap_or("").to_string(), member_field(c, from, "name"))
        };
        if !email_ok(&to) {
            return (None, None);
        }
        let server = mail::link_address(&cfg, connected_to);
        let m = mail::invite_mail(&to, &workspace, &inviter, code, server.as_deref(), INVITE_DAYS);
        match mail::send(&self.home, &cfg, &m) {
            Ok(()) => (Some(to), None),
            Err(e) => (None, Some(e)),
        }
    }

    fn welcome(&self, tag: u16, member: u64, role: Role, token: Option<String>) -> String {
        let g = self.inner.lock().unwrap();
        let email = g.saved.accounts.get(&member).map(|a| a.email.clone());
        let snapshot = g.canon.as_ref().map(|c| copy_for(c, member).unwrap_or_else(|| c.clone()));
        Msg::Welcome { tag, snapshot, rev: g.saved.rev, member, role, token, email }.to_json()
    }

    /// Merge the copy `member` sent from `device` into the team's copy; tell
    /// everyone when it changed.
    fn push(self: &Arc<Self>, member: u64, device: &str, data: Value) {
        if !copy_shape_ok(&data) {
            eprintln!("ignored a copy from member {} that isn't a workspace", member);
            return;
        }
        let mut data = wb_sync::shareable(&data);
        let mut g = self.inner.lock().unwrap();
        let members_before = member_ids(g.canon.as_ref());
        // the messages already there, to tell which ones are new (only when
        // someone could be notified)
        let before = match &g.canon {
            Some(c) if self.pusher.is_some() && !g.saved.push.is_empty() => Some(push::message_ids(c)),
            _ => None,
        };
        if let Some(c) = &g.canon {
            guard_push(c, &mut data, member);
        }
        let claimer = g.saved.devices.contains_key(device);
        let changed = match g.canon.as_mut() {
            // the workspace comes from the computer that claimed the server
            // (a claim taken over after claim_wait no longer counts)
            None if !claimer => false,
            None => {
                // the owner's first copy brings the workspace
                let mut data = data;
                let owners: Vec<u64> = g.saved.roles.iter().filter(|(_, r)| **r == Role::Owner).map(|(m, _)| *m).collect();
                for m in owners {
                    label_role(&mut data, m, Role::Owner);
                }
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
        if changed {
            if let (Some(before), Some(canon)) = (&before, &g.canon) {
                let notes = push::notes(canon, before, &[member], now_ms() / 1000);
                self.notify(&g.saved, notes);
            }
            self.broadcast_canon(&mut g);
            // someone added or removed people: owners and admins see the new list
            if member_ids(g.canon.as_ref()) != members_before {
                drop(g);
                let _ = self.access.send(());
            }
        }
    }

    /// Send `notes` to the devices of their members that have a push token
    /// and aren't connected, MAX_PER_DEVICE each at most, on another thread.
    fn notify(self: &Arc<Self>, saved: &Saved, notes: Vec<push::Note>) {
        let Some(sender) = self.pusher.clone() else { return };
        if notes.is_empty() {
            return;
        }
        let online = self.online.lock().unwrap().clone();
        let mut jobs: Vec<(String, push::PushToken, Value)> = vec![];
        for (device, tok) in &saved.push {
            let Some(d) = saved.devices.get(device) else { continue };
            if online.get(device).is_some_and(|n| *n > 0) {
                continue;
            }
            let mine: Vec<&push::Note> = notes.iter().filter(|n| n.member == d.member).collect();
            for p in push::batch(&mine) {
                jobs.push((device.clone(), tok.clone(), p));
            }
        }
        if jobs.is_empty() {
            return;
        }
        let hub = self.clone();
        std::thread::spawn(move || {
            for (device, tok, payload) in jobs {
                match sender.send(&tok.env, &tok.token, &payload) {
                    push::Outcome::Sent => {}
                    push::Outcome::Gone => {
                        // the app was removed (or the token was never good): forget it
                        let mut g = hub.inner.lock().unwrap();
                        if g.saved.push.get(&device).is_some_and(|t| t.token == tok.token) {
                            g.saved.push.remove(&device);
                            g.dirty = true;
                        }
                    }
                    push::Outcome::Failed(why) => eprintln!("couldn't send a notification to device {}: {}", device, why),
                }
            }
        });
    }

    /// An iPhone's push token (`push_token`).
    fn set_push_token(&self, device: &str, token: &str, platform: &str, env: &str) {
        let mut g = self.inner.lock().unwrap();
        if g.saved.set_push_token(device, token, platform, env) {
            g.dirty = true;
        }
    }

    fn latest(&self, member: u64) -> Option<String> {
        let g = self.inner.lock().unwrap();
        g.canon.as_ref().map(|c| Msg::Update { data: copy_for(c, member).unwrap_or_else(|| c.clone()), rev: g.saved.rev }.to_json())
    }

    fn access_for(&self, member: u64) -> Msg {
        let g = self.inner.lock().unwrap();
        let role = g.saved.role_of(member);
        let people = if role.manages() { g.saved.people(g.canon.as_ref(), role) } else { vec![] };
        Msg::Access { people, role }
    }

    /// An access request from `member`.
    fn request(&self, member: u64, msg: Msg) -> Reply {
        // backups: the owner only (a restore changes everyone's workspace)
        if matches!(msg, Msg::BackupList | Msg::Restore { .. }) {
            if self.inner.lock().unwrap().saved.role_of(member) != Role::Owner {
                return Reply::Denied("Only the owner can see and restore backups.".into());
            }
            return match msg {
                Msg::Restore { name } => match self.backup_path(&name).and_then(|p| self.restore(&p)) {
                    Ok(()) => Reply::To(Msg::Restored { name }),
                    Err(e) => Reply::Denied(e),
                },
                _ => Reply::To(Msg::Backups { backups: self.list_backups() }),
            };
        }
        let mut g = self.inner.lock().unwrap();
        let me = g.saved.role_of(member);
        if !me.manages() {
            return Reply::Denied("Only the owner or an admin can do that.".into());
        }
        let mut kicked: Vec<String> = vec![];
        let reply = match msg {
            Msg::AccessList => None,
            Msg::CreateInvite { member: who, role } => {
                if role == Role::Owner {
                    return Reply::Denied("There is one owner. Invite them as an admin instead.".into());
                }
                if !member_exists(g.canon.as_ref(), who) {
                    return Reply::Denied("That person isn't in the workspace yet. Add them with Invite first.".into());
                }
                // a code for the owner signs in as the owner: only the owner makes one
                if g.saved.role_of(who) == Role::Owner && me != Role::Owner {
                    return Reply::Denied("Only the owner can make a code for the owner.".into());
                }
                g.saved.drop_expired_invites(now_ms());
                let code = g.saved.make_invite(who, role);
                g.dirty = true;
                Some(Msg::Invite { member: who, code, emailed: None, mail_error: None })
            }
            Msg::SetRole { member: who, role } => {
                if role == Role::Owner || g.saved.role_of(who) == Role::Owner {
                    return Reply::Denied("The owner's role can't be changed here.".into());
                }
                g.saved.set_role(who, role);
                g.dirty = true;
                let relabeled = g.canon.as_mut().is_some_and(|c| label_role(c, who, role));
                if relabeled {
                    self.broadcast_canon(&mut g);
                }
                None
            }
            Msg::Revoke { member: who } => {
                if g.saved.role_of(who) == Role::Owner {
                    return Reply::Denied("The owner's access can't be removed.".into());
                }
                kicked = g.saved.devices.iter().filter(|(_, d)| d.member == who).map(|(id, _)| id.clone()).collect();
                g.saved.devices.retain(|_, d| d.member != who);
                let devices = &g.saved.devices;
                let push: BTreeMap<String, push::PushToken> = g.saved.push.iter().filter(|(d, _)| devices.contains_key(*d)).map(|(d, t)| (d.clone(), t.clone())).collect();
                g.saved.push = push;
                g.saved.invites.retain(|_, i| i.member != who);
                // and the password that would let them back in
                g.saved.accounts.remove(&who);
                // removing access also takes back admin rights
                g.saved.set_role(who, Role::Member);
                g.dirty = true;
                if g.canon.as_mut().is_some_and(|c| label_role(c, who, Role::Member)) {
                    self.broadcast_canon(&mut g);
                }
                None
            }
            _ => return Reply::Denied("Unknown request.".into()),
        };
        drop(g);
        if !kicked.is_empty() {
            let _ = self.kicks.send(Arc::new(kicked));
        }
        let _ = self.access.send(());
        match reply {
            Some(m) => Reply::To(m),
            None => Reply::To(self.access_for(member)),
        }
    }

    /// A token for the huddle of `channel`, for someone allowed in it.
    fn call(&self, member: u64, device: &str, channel: u64) -> Result<Msg, String> {
        let cfg = load_livekit(&self.home).ok_or("Voice huddles aren't set up on this server yet.")?;
        let g = self.inner.lock().unwrap();
        let canon = g.canon.as_ref().ok_or("There's no workspace on the server yet.")?;
        let ch = canon.get("channels").and_then(|c| c.as_array()).and_then(|a| a.iter().find(|c| c.get("id").and_then(|i| i.as_u64()) == Some(channel))).ok_or("That conversation isn't on the server yet. Try again in a moment.")?;
        let members: Vec<u64> = ch.get("members").and_then(|m| m.as_array()).map(|a| a.iter().filter_map(|m| m.as_u64()).collect()).unwrap_or_default();
        if !members.is_empty() && !members.contains(&member) {
            return Err("You're not in that conversation.".into());
        }
        let name = canon.get("members").and_then(|m| m.as_array()).and_then(|a| a.iter().find(|m| m.get("id").and_then(|i| i.as_u64()) == Some(member))).and_then(|m| m.get("name")?.as_str()).unwrap_or("").to_string();
        let token = call_token(&cfg, &room_name(channel), &format!("{}-{}", member, device), &name, now_ms() / 1000);
        Ok(Msg::CallToken { channel, url: cfg.url, token })
    }

    /// Every backup, newest first.
    fn list_backups(&self) -> Vec<Backup> {
        let dir = self.home.join(BACKUP_DIR);
        let mut all: Vec<(std::time::SystemTime, Backup)> = ["hourly", "daily", "before-restore"]
            .iter()
            .flat_map(|k| backup_files(&dir, k))
            .filter_map(|p| {
                let meta = std::fs::metadata(&p).ok()?;
                Some((meta.modified().ok()?, Backup { name: p.file_name()?.to_string_lossy().to_string(), bytes: meta.len() }))
            })
            .collect();
        all.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.name.cmp(&a.1.name)));
        all.into_iter().map(|(_, b)| b).collect()
    }

    /// A backup by its file name, only from the backups folder.
    fn backup_path(&self, name: &str) -> Result<PathBuf, String> {
        let ok_name = !name.contains('/') && !name.contains('\\') && !name.contains("..") && name.ends_with(".json") && ["hourly-", "daily-", "before-restore-"].iter().any(|p| name.starts_with(p));
        let path = self.home.join(BACKUP_DIR).join(name);
        if !ok_name || !path.is_file() {
            return Err("That backup isn't on the server.".into());
        }
        Ok(path)
    }

    /// Go back to a backup: its items win and deleted ones come back; things
    /// made after it stay. The copy from before goes to backups/ first.
    fn restore(&self, path: &Path) -> Result<(), String> {
        let mut data = match read_json(path) {
            Ok(Some(v)) => v,
            Ok(None) => return Err(format!("{} doesn't exist", path.display())),
            Err(e) => return Err(e.to_string()),
        };
        if !copy_shape_ok(&data) {
            return Err(format!("{} isn't a Workboard workspace", path.display()));
        }
        {
            mark_restored(&mut data, now_ms());
            let data = wb_sync::shareable(&data);
            let mut g = self.inner.lock().unwrap();
            if let Some(cur) = &g.canon {
                let keep = self.home.join(BACKUP_DIR).join(format!("before-restore-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S")));
                let _ = std::fs::create_dir_all(self.home.join(BACKUP_DIR));
                let _ = write_atomic(&keep, &serde_json::to_string(cur).unwrap_or_default());
                let mut kept = backup_files(&self.home.join(BACKUP_DIR), "before-restore");
                while kept.len() > KEEP_BEFORE_RESTORE {
                    let _ = std::fs::remove_file(kept.remove(0));
                }
            }
            match g.canon.as_mut() {
                Some(c) => {
                    // the restored items win; things made after the backup stay
                    let mut restored = data;
                    wb_sync::merge(&mut restored, &[c.clone()]);
                    *c = wb_sync::shareable(&restored);
                }
                None => g.canon = Some(data),
            }
            self.broadcast_canon(&mut g);
        }
        Ok(())
    }

    /// Requests from the `workboard-server` admin commands.
    fn admin(&self, req: &Value) -> Value {
        if let Some(file) = req.get("restore").and_then(|f| f.as_str()) {
            return match self.restore(Path::new(file)) {
                Ok(()) => json!({"ok": true, "message": format!("Restored {}. Everyone's app gets it now (the copy from before is in {}/before-restore-*.json).", file, BACKUP_DIR)}),
                Err(e) => json!({"ok": false, "message": e}),
            };
        }
        if let Some(member) = req.get("invite").and_then(|m| m.as_u64()) {
            let role: Role = req.get("role").and_then(|r| serde_json::from_value(r.clone()).ok()).unwrap_or_default();
            let mut g = self.inner.lock().unwrap();
            if g.canon.is_some() && !member_exists(g.canon.as_ref(), member) {
                return json!({"ok": false, "message": format!("No member with id {} in the workspace (see `workboard-server people`).", member)});
            }
            let code = g.saved.make_invite(member, role);
            g.dirty = true;
            drop(g);
            let _ = self.access.send(());
            return json!({"ok": true, "message": format!("Invite code for member {} as {}: {}  (works once, for {} days)", member, role.label(), code, INVITE_DAYS), "code": code});
        }
        json!({"ok": false, "message": "unknown request"})
    }

    /// Pick up an admin request left in the server folder.
    fn admin_file(&self) {
        let req_path = self.home.join(REQUEST_FILE);
        let Ok(Some(req)) = read_json(&req_path) else { return };
        let _ = std::fs::remove_file(&req_path);
        let res = self.admin(&req);
        self.flush();
        let _ = write_atomic(&self.home.join(RESULT_FILE), &res.to_string());
    }
}

type WebSocket = WebSocketStream<TokioIo<hyper::upgrade::Upgraded>>;

fn ws_config(max: usize) -> WebSocketConfig {
    WebSocketConfig::default().max_message_size(Some(max)).max_frame_size(Some(max))
}

/// Sec-WebSocket-Accept for a Sec-WebSocket-Key (RFC 6455).
fn accept_key(key: &[u8]) -> String {
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
async fn ws_route(State(hub): State<Arc<Hub>>, mut req: axum::extract::Request) -> Response {
    let h = req.headers();
    // the address this app reached us at, for links in invite mails
    let host = h.get(header::HOST).and_then(|h| h.to_str().ok()).map(|h| h.to_string());
    let has = |name: header::HeaderName, want: &str| h.get_all(name).iter().any(|v| v.to_str().is_ok_and(|v| v.split(',').any(|p| p.trim().eq_ignore_ascii_case(want))));
    let ok = req.method() == Method::GET && req.version() == Version::HTTP_11 && has(header::CONNECTION, "upgrade") && has(header::UPGRADE, "websocket") && has(header::SEC_WEBSOCKET_VERSION, "13");
    let Some(key) = h.get(header::SEC_WEBSOCKET_KEY).filter(|_| ok).map(|k| accept_key(k.as_bytes())) else {
        return (StatusCode::BAD_REQUEST, "This address takes Workboard's WebSocket connection.").into_response();
    };
    let on_upgrade = hyper::upgrade::on(&mut req);
    tokio::spawn(async move {
        let Ok(upgraded) = on_upgrade.await else { return };
        let socket = WebSocketStream::from_raw_socket(TokioIo::new(upgraded), WsRole::Server, Some(ws_config(MAX_HELLO))).await;
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

/// POST /auth/forgot {"email"}: email a password-reset code. Always "ok", so
/// it doesn't tell who has an account; "mail": false when mail isn't set up.
async fn forgot_route(State(hub): State<Arc<Hub>>, body: axum::body::Bytes) -> Response {
    let email = serde_json::from_slice::<Value>(&body).ok().and_then(|v| v.get("email").and_then(|e| e.as_str()).map(|e| e.to_string())).unwrap_or_default();
    let h = hub.clone();
    let mail = tokio::task::spawn_blocking(move || h.forgot(&email)).await.unwrap_or(false);
    Json(json!({"ok": true, "mail": mail})).into_response()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// GET /join?code=...&email=...: the page an invite mail links to. It opens
/// the app with everything filled in, and shows what to type otherwise.
async fn join_route(axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>, headers: axum::http::HeaderMap) -> Response {
    let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
    let code = q.get("code").map(|c| c.trim().to_string()).unwrap_or_default();
    let email = q.get("email").map(|e| e.trim().to_string()).filter(|e| !e.is_empty());
    let link = wb_net::join_link(&host, &code, email.as_deref());
    let page = JOIN_PAGE.replace("{link}", &html_escape(&link)).replace("{server}", &html_escape(&host)).replace("{code}", &html_escape(if code.is_empty() { "(ask for your invite code)" } else { &code }));
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], page).into_response()
}

const JOIN_PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Join your team on Workboard</title>
<style>
:root { color-scheme: light dark; --bg: #f7f7f9; --card: #fff; --text: #1d1d22; --muted: #6b6b76; --line: #e3e3e8; --primary: #5f55ee; }
@media (prefers-color-scheme: dark) { :root { --bg: #18181b; --card: #222226; --text: #ececf0; --muted: #a0a0aa; --line: #34343a; } }
body { margin: 0; background: var(--bg); color: var(--text); font: 15px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
main { max-width: 440px; margin: 48px auto; padding: 0 16px; }
.card { background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 24px; }
h1 { font-size: 20px; margin: 0 0 8px; }
p { margin: 0 0 16px; color: var(--muted); }
a.button { display: block; text-align: center; background: var(--primary); color: #fff; text-decoration: none; padding: 11px; border-radius: 8px; font-weight: 600; }
dl { margin: 20px 0 0; display: grid; grid-template-columns: auto 1fr; gap: 6px 14px; }
dt { color: var(--muted); }
dd { margin: 0; font-family: ui-monospace, Menlo, monospace; word-break: break-all; }
</style></head>
<body><main><div class="card">
<h1>Join your team on Workboard</h1>
<p>Open the app with your invite filled in. You'll choose a password, and then sign in with your email on your other computers and phones.</p>
<a class="button" href="{link}">Open in Workboard</a>
<p style="margin-top:20px">Nothing happened? Install Workboard first, or open it, choose <b>Join my team</b> and type:</p>
<dl><dt>Server</dt><dd>{server}</dd><dt>Invite code</dt><dd>{code}</dd></dl>
</div></main></body></html>
"#;

/// The content type for a file the updates route may serve: a plain name
/// (no folders, not hidden) of a kind Sparkle asks for. None for anything else.
pub fn update_content_type(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.starts_with('.') || !name.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        return None;
    }
    Some(match name.rsplit_once('.')?.1 {
        "xml" => "application/xml",
        "zip" => "application/zip",
        "delta" => "application/octet-stream",
        "html" => "text/html; charset=utf-8",
        _ => return None,
    })
}

/// App updates for Sparkle: GET /updates/appcast.xml and the files it lists.
async fn update_route(axum::extract::Path(name): axum::extract::Path<String>, State(hub): State<Arc<Hub>>) -> Response {
    let Some(kind) = update_content_type(&name) else { return StatusCode::NOT_FOUND.into_response() };
    match tokio::fs::read(hub.home.join(UPDATES_DIR).join(&name)).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "no-cache")], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn health(State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    let g = hub.inner.lock().unwrap();
    let backups = backup_files(&hub.home.join(BACKUP_DIR), "hourly").last().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
    Json(json!({
        "ok": true,
        "rev": g.saved.rev,
        "workspace": g.canon.is_some(),
        "devices": g.saved.devices.len(),
        "connections": hub.connections.load(Ordering::Relaxed),
        "latest_backup": backups,
    }))
}

async fn send(socket: &mut WebSocket, text: &str) -> bool {
    socket.send(Message::text(text)).await.is_ok()
}

struct Counted(Arc<Hub>);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::Relaxed);
    }
}

/// A signed-in device's connection, counted in Hub::online while it lasts.
struct Online(Arc<Hub>, String);

impl Online {
    fn new(hub: &Arc<Hub>, device: &str) -> Online {
        *hub.online.lock().unwrap().entry(device.to_string()).or_insert(0) += 1;
        Online(hub.clone(), device.to_string())
    }
}

impl Drop for Online {
    fn drop(&mut self) {
        let mut o = self.0.online.lock().unwrap();
        if let Some(n) = o.get_mut(&self.1) {
            *n -= 1;
            if *n == 0 {
                o.remove(&self.1);
            }
        }
    }
}

async fn connection(mut socket: WebSocket, hub: Arc<Hub>, host: Option<String>) {
    hub.connections.fetch_add(1, Ordering::Relaxed);
    let _count = Counted(hub.clone());
    let mut stop = hub.stop.clone();
    // the first message must be hello
    let hello = tokio::select! {
        m = tokio::time::timeout(Duration::from_secs(10), socket.next()) => m,
        _ = stop.changed() => return,
    };
    let Ok(Some(Ok(Message::Text(t)))) = hello else { return };
    let Some(Msg::Hello { device, tag, token, invite, claim, email, password, new_password, reset_code }) = Msg::parse(t.as_str()) else {
        let _ = send(&mut socket, &Msg::error("Say hello first.", None).to_json()).await;
        return;
    };
    let hello = Hello { device: device.clone(), tag, token, invite, claim, email, password, new_password, reset_code };
    let h = hub.clone();
    // password hashing takes a moment
    let signed = tokio::task::spawn_blocking(move || h.sign_in(&hello)).await.unwrap_or_else(|_| Err("The server had a problem. Try again.".into()));
    let (member, role, tag, token) = match signed {
        Ok(x) => x,
        Err(why) => {
            let _ = send(&mut socket, &Msg::error(&why, Some(CODE_AUTH)).to_json()).await;
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
    };
    let _online = Online::new(&hub, &device);
    // signed in: from now on a message may hold a whole workspace. The app
    // sends nothing between hello and welcome, so nothing read ahead is lost.
    let mut socket = WebSocketStream::from_raw_socket(socket.into_inner(), WsRole::Server, Some(ws_config(wb_net::MAX_MESSAGE))).await;
    // listen before sending the copy so no update falls in between
    let mut updates = hub.updates.subscribe();
    let mut kicks = hub.kicks.subscribe();
    let mut access = hub.access.subscribe();
    if !send(&mut socket, &hub.welcome(tag, member, role, token)).await {
        return;
    }
    if role.manages() && !send(&mut socket, &hub.access_for(member).to_json()).await {
        return;
    }
    let silent = Duration::from_secs(wb_net::SILENT_SECS + 15);
    loop {
        tokio::select! {
            m = tokio::time::timeout(silent, socket.next()) => {
                let m = match m {
                    Err(_) => break, // heard nothing for too long
                    Ok(None) | Ok(Some(Err(_))) => break,
                    Ok(Some(Ok(m))) => m,
                };
                let Message::Text(t) = m else {
                    if matches!(m, Message::Close(_)) { break } else { continue }
                };
                let out = match Msg::parse(t.as_str()) {
                    Some(Msg::Push { data }) => {
                        let (hub2, device2) = (hub.clone(), device.clone());
                        // merging can take a moment on a big workspace
                        let _ = tokio::task::spawn_blocking(move || hub2.push(member, &device2, data)).await;
                        None
                    }
                    Some(Msg::Ping) => Some(Msg::Pong.to_json()),
                    Some(Msg::PushToken { token, platform, env }) => {
                        hub.set_push_token(&device, &token, &platform, &env);
                        None
                    }
                    Some(Msg::CallJoin { channel }) => Some(match hub.call(member, &device, channel) {
                        Ok(m) => m.to_json(),
                        Err(why) => Msg::call_denied(&why, channel).to_json(),
                    }),
                    Some(Msg::SetPassword { email, password, old_password }) => {
                        let h = hub.clone();
                        let r = tokio::task::spawn_blocking(move || h.set_password(member, &email, &password, old_password.as_deref())).await.unwrap_or_else(|_| Err("The server had a problem. Try again.".into()));
                        Some(match r {
                            Ok(m) => m.to_json(),
                            Err(why) => Msg::error(&why, Some(CODE_DENIED)).to_json(),
                        })
                    }
                    Some(m @ (Msg::CreateInvite { .. } | Msg::SetRole { .. } | Msg::Revoke { .. } | Msg::AccessList | Msg::BackupList | Msg::Restore { .. })) => match hub.request(member, m) {
                        Reply::To(Msg::Invite { member: who, code, .. }) => {
                            // email it when the server can (sending takes a moment)
                            let (h, c, at) = (hub.clone(), code.clone(), host.clone());
                            let (emailed, mail_error) = tokio::task::spawn_blocking(move || h.email_invite(member, who, &c, at.as_deref())).await.unwrap_or((None, None));
                            Some(Msg::Invite { member: who, code, emailed, mail_error }.to_json())
                        }
                        Reply::To(r) => Some(r.to_json()),
                        Reply::Denied(why) => Some(Msg::error(&why, Some(CODE_DENIED)).to_json()),
                    },
                    _ => None,
                };
                if let Some(o) = out {
                    if !send(&mut socket, &o).await {
                        break;
                    }
                }
            }
            u = updates.recv() => {
                let text = match u {
                    Ok(c) => c.update_for(member),
                    // fell behind: the newest copy says it all
                    Err(broadcast::error::RecvError::Lagged(_)) => match hub.latest(member) { Some(t) => t, None => continue },
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if !send(&mut socket, &text).await {
                    break;
                }
            }
            k = kicks.recv() => {
                if let Ok(list) = k {
                    if list.contains(&device) {
                        let _ = send(&mut socket, &Msg::error("Your access to this server was removed.", Some(CODE_AUTH)).to_json()).await;
                        break;
                    }
                }
            }
            a = access.recv() => {
                if a.is_ok() && !send(&mut socket, &hub.access_for(member).to_json()).await {
                    break;
                }
            }
            _ = stop.changed() => break,
        }
    }
    let _ = socket.send(Message::Close(None)).await;
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
}

impl Default for Options {
    fn default() -> Self {
        Options { max_file: MAX_FILE, claim_wait: Duration::from_secs(CLAIM_WAIT_SECS), push: None }
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
pub async fn serve(listener: tokio::net::TcpListener, home: PathBuf, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    serve_with(listener, home, Options::default(), shutdown).await
}

/// `serve` with other settings.
pub async fn serve_with(listener: tokio::net::TcpListener, home: PathBuf, options: Options, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    std::fs::create_dir_all(&home)?;
    let canon = read_json(&home.join(WORKSPACE_FILE))?;
    let saved = load_saved(&home)?;
    let pusher = options.push.clone().or_else(|| apns_sender(&home));
    let (stop_tx, stop_rx) = watch::channel(false);
    let hub = Arc::new(Hub {
        home,
        inner: Mutex::new(Inner { canon, saved, dirty: false, resets: Default::default(), fails: Default::default() }),
        updates: broadcast::channel(64).0,
        kicks: broadcast::channel(16).0,
        access: broadcast::channel(16).0,
        stop: stop_rx,
        connections: AtomicUsize::new(0),
        max_file: options.max_file,
        claim_wait: options.claim_wait,
        pusher,
        online: Mutex::new(Default::default()),
    });
    // a request left while the server was off
    hub.admin_file();
    let worker = {
        let hub = hub.clone();
        tokio::spawn(async move {
            let mut tick: u64 = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                tick += 1;
                let hub = hub.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    hub.flush();
                    hub.admin_file();
                    if tick % 60 == 1 {
                        hub.backup_now();
                    }
                })
                .await;
            }
        })
    };
    let app = Router::new().route(wb_net::PATH, get(ws_route)).route("/health", get(health)).route("/updates/{file}", get(update_route))
        // get() also answers HEAD. Uploads are read as a stream, which axum's
        // body limit doesn't cover; put_file enforces max_file itself.
        .route("/files/{file}", get(files::get_file).put(files::put_file))
        .route("/auth/forgot", axum::routing::post(forgot_route))
        .route("/join", get(join_route))
        .with_state(hub.clone());
    let res = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown.await;
            // open connections end themselves when they see this
            let _ = stop_tx.send(true);
        })
        .await;
    worker.abort();
    hub.flush();
    hub.backup_now();
    res
}

/// Leave a request for the running server (or the next start) and wait for
/// its answer. None when the server didn't answer in time.
pub fn admin_request(home: &Path, req: &Value, wait: Duration) -> std::io::Result<Option<Value>> {
    std::fs::create_dir_all(home)?;
    let _ = std::fs::remove_file(home.join(RESULT_FILE));
    write_atomic(&home.join(REQUEST_FILE), &req.to_string())?;
    let until = std::time::Instant::now() + wait;
    while std::time::Instant::now() < until {
        if let Ok(Some(v)) = read_json(&home.join(RESULT_FILE)) {
            let _ = std::fs::remove_file(home.join(RESULT_FILE));
            return Ok(Some(v));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(None)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(s: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn the_updates_route_serves_only_plain_update_files() {
        assert_eq!(update_content_type("appcast.xml"), Some("application/xml"));
        assert_eq!(update_content_type("Workboard-0.1.2-arm64.zip"), Some("application/zip"));
        assert_eq!(update_content_type("Workboard12-11.delta"), Some("application/octet-stream"));
        for bad in ["", ".appcast.xml", "../server.json", "a/b.zip", "workspace.json", "x.zip ", "%2e%2e.zip", "appcast"] {
            assert_eq!(update_content_type(bad), None, "{:?}", bad);
        }
    }

    #[test]
    fn backups_keep_48_hourly_and_30_daily_copies() {
        let home = std::env::temp_dir().join(format!("wb-backup-{:x}", wb_sync::random_u64()));
        let start = dt("2026-11-01 00:00");
        // 40 days of hourly changes
        for h in 0..(40 * 24) {
            backup(&home, &format!("{{\"v\":{}}}", h), start + chrono::Duration::hours(h)).unwrap();
        }
        let dir = home.join(BACKUP_DIR);
        let hourly = backup_files(&dir, "hourly");
        let daily = backup_files(&dir, "daily");
        assert_eq!(hourly.len(), KEEP_HOURLY);
        assert_eq!(daily.len(), KEEP_DAILY);
        assert!(hourly.last().unwrap().ends_with("hourly-20261210-23.json"), "{:?}", hourly.last());
        assert!(daily.first().unwrap().ends_with("daily-20261111.json"), "{:?}", daily.first());
        // nothing changed: no new copy next hour
        let n = backup(&home, "{\"v\":959}", start + chrono::Duration::hours(40 * 24)).unwrap();
        assert!(n.iter().all(|p| !p.to_string_lossy().contains("hourly")), "{:?}", n);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn a_restored_copy_wins_and_brings_deleted_items_back() {
        let mut meta = wb_sync::SyncMeta::default();
        let old = json!({"tasks": [{"id": 1, "name": "old name"}, {"id": 2, "name": "deleted later"}], "extra": {}});
        wb_sync::stamp(&old, &mut meta, 1000);
        let mut backup = old.clone();
        backup["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        // later: task 1 renamed, task 2 deleted
        let mut now = json!({"tasks": [{"id": 1, "name": "new name"}, {"id": 3, "name": "made later"}], "extra": {}});
        wb_sync::stamp(&now, &mut meta, 2000);
        now["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        mark_restored(&mut backup, 3000);
        wb_sync::merge(&mut backup, &[now]);
        let names: Vec<&str> = backup["tasks"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["old name", "deleted later", "made later"]);
    }

    #[test]
    fn call_tokens_are_signed_livekit_jwts() {
        use hmac::{KeyInit, Mac};
        let cfg = LiveKitCfg { url: "ws://x:7880".into(), key: "APIkey".into(), secret: "s3cret-s3cret-s3cret-s3cret-s3cret".into() };
        let t = call_token(&cfg, "wb-10", "2-dev1", "Bora", 1_000_000);
        let parts: Vec<&str> = t.split('.').collect();
        assert_eq!(parts.len(), 3);
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(cfg.secret.as_bytes()).unwrap();
        mac.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
        assert_eq!(parts[2], b64url(&mac.finalize().into_bytes()), "signature checks out");
        assert!(!parts.iter().any(|p| p.contains('=') || p.contains('+') || p.contains('/')), "base64url without padding");
        // decode the claims back
        let dec = |s: &str| -> Vec<u8> {
            const T: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let mut bits = 0u32;
            let mut n = 0;
            let mut out = vec![];
            for c in s.chars() {
                bits = bits << 6 | T.find(c).unwrap() as u32;
                n += 6;
                if n >= 8 {
                    n -= 8;
                    out.push((bits >> n) as u8);
                }
            }
            out
        };
        let claims: Value = serde_json::from_slice(&dec(parts[1])).unwrap();
        assert_eq!(claims["iss"], "APIkey");
        assert_eq!(claims["sub"], "2-dev1");
        assert_eq!(claims["name"], "Bora");
        assert_eq!(claims["video"]["room"], "wb-10");
        assert_eq!(claims["video"]["roomJoin"], true);
        assert_eq!(claims["exp"], 1_000_000 + CALL_TOKEN_SECS);
    }

    #[test]
    fn passwords_hash_with_pbkdf2_and_check() {
        // PBKDF2-HMAC-SHA256 test vectors (RFC 7914 section 11 / common sets)
        assert_eq!(hex(&pbkdf2_sha256(b"password", b"salt", 1)), "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b");
        assert_eq!(hex(&pbkdf2_sha256(b"password", b"salt", 4096)), "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a");
        let a = Account::new(" Kim@Company.com ", "correct horse");
        assert_eq!(a.email, "Kim@Company.com");
        assert!(a.check("correct horse") && !a.check("correct horsE") && !a.check(""));
        assert_ne!(Account::new("k@c.co", "correct horse").salt, a.salt, "every hash has its own salt");
        let mut s = Saved::default();
        s.accounts.insert(4, a);
        assert_eq!(s.account_member("kim@company.COM"), Some(4));
        assert!(s.account_problem(5, "kim@company.com", "long enough").unwrap().contains("already"));
        assert!(s.account_problem(4, "kim@company.com", "short").unwrap().contains("at least 8"));
        assert!(s.account_problem(4, "not an email", "long enough").is_some());
        assert_eq!(s.account_problem(4, "kim@company.com", "long enough"), None);
    }

    #[test]
    fn invite_codes_work_once_and_ignore_case_and_dashes() {
        let mut s = Saved::default();
        let code = s.make_invite(7, Role::Admin);
        assert_eq!(code.len(), 14);
        let typed = code.to_lowercase().replace('-', " ");
        let inv = s.take_invite(&typed).expect("works");
        assert_eq!((inv.member, inv.role), (7, Role::Admin));
        assert!(s.take_invite(&code).is_none(), "only once");
        s.invites.insert("OLDC-ODEX-XXXX".into(), InviteRec { member: 8, role: Role::Member, created: now_ms() - 8 * 86_400_000 });
        assert!(s.take_invite("OLDC-ODEX-XXXX").is_none(), "expired");
    }

    #[test]
    fn a_stage_one_state_file_keeps_its_revision() {
        let home = std::env::temp_dir().join(format!("wb-state-{:x}", wb_sync::random_u64()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(STATE_FILE), r#"{"rev": 9, "devices": {"aaaa": 3}}"#).unwrap();
        let s = load_saved(&home).unwrap();
        assert_eq!(s.rev, 9);
        assert!(s.devices.is_empty());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn expired_invite_codes_are_not_shown() {
        let mut s = Saved::default();
        s.invites.insert("OLDC-ODEX-XXXX".into(), InviteRec { member: 8, role: Role::Member, created: now_ms() - 8 * 86_400_000 });
        let fresh = s.make_invite(9, Role::Member);
        let canon = json!({"members": [{"id": 8}, {"id": 9}]});
        let people = s.people(Some(&canon), Role::Owner);
        assert_eq!(people.iter().find(|p| p.member == 8).unwrap().invite, None);
        assert_eq!(people.iter().find(|p| p.member == 9).unwrap().invite, Some(fresh));
        assert_eq!(s.open_invite(8, now_ms()), None);
    }

    #[test]
    fn odd_copies_never_panic() {
        for mut v in [json!("x"), json!([1]), json!({"members": [{"id": 1}], "extra": 3}), json!({"members": [{"id": 1}], "extra": {"sync": "s"}}), json!({"members": [{"id": 1}], "extra": {"sync": {"stamps": []}}})] {
            assert!(!copy_shape_ok(&v));
            assert!(!label_role(&mut v, 1, Role::Owner));
            mark_restored(&mut v, 1);
        }
        let mut ok = json!({"members": [{"id": 1}]});
        assert!(label_role(&mut ok, 1, Role::Owner));
        assert_eq!(ok["members"][0]["role"], "Owner");
    }
}
