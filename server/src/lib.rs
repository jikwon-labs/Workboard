// workboard-server: keeps the team's copy of a Workboard workspace, keeps
// every connected app in step with it (see wb_net for the messages), decides
// who may sign in, and keeps backups.
//
// Files in the server folder:
//   workspace.json        the team's copy
//   server.json           devices (hashed tokens, member, id tag), roles,
//                         open invites, revision number
//   backups/              hourly-YYYYMMDD-HH.json (48 kept),
//                         daily-YYYYMMDD.json (30 kept), before-restore-*.json
//   admin-request.json    a request from the `workboard-server` admin
//   admin-result.json     commands, picked up within a second (see admin)
//   updates/              app updates: appcast.xml and the Workboard-*.zip it
//                         lists, served at /updates/<file> for Sparkle
//                         (scripts/publish-update.sh puts them here)
// Everything is written to a temporary file first and then renamed, at most
// a second after a change, and once more when the server stops.
//
// Signing in: the first app to reach an empty server claims it and its
// person becomes the owner. Everyone else needs a one-time invite code made
// by the owner or an admin, which signs that computer in as one member of
// the workspace and gives it a token for later. Owners and admins can also
// change roles and remove someone's access.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, StatusCode};
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
use wb_net::{Backup, Msg, Person, Role, CODE_AUTH, CODE_DENIED};

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

    fn drop_expired_invites(&mut self, now: i64) {
        self.invites.retain(|_, i| now - i.created < INVITE_DAYS * 86_400_000);
    }

    /// A fresh one-time code for `member`, replacing any earlier one.
    pub fn make_invite(&mut self, member: u64, role: Role) -> String {
        self.invites.retain(|_, i| i.member != member);
        let code = new_code();
        self.invites.insert(code.clone(), InviteRec { member, role, created: now_ms() });
        code
    }

    fn take_invite(&mut self, typed: &str) -> Option<InviteRec> {
        self.drop_expired_invites(now_ms());
        let want = normalize_code(typed);
        let key = self.invites.keys().find(|k| normalize_code(k) == want)?.clone();
        self.invites.remove(&key)
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
                invite: self.invites.iter().find(|(_, i)| i.member == member).map(|(c, _)| c.clone()).filter(|_| viewer == Role::Owner || self.role_of(member) != Role::Owner),
            })
            .collect()
    }
}

fn member_ids(canon: Option<&Value>) -> Vec<u64> {
    canon.and_then(|c| c.get("members")).and_then(|m| m.as_array()).map(|a| a.iter().filter_map(|m| m.get("id")?.as_u64()).collect()).unwrap_or_default()
}

fn member_exists(canon: Option<&Value>, member: u64) -> bool {
    canon.and_then(|c| c.get("members")).and_then(|m| m.as_array()).is_some_and(|a| a.iter().any(|m| m.get("id").and_then(|i| i.as_u64()) == Some(member)))
}

/// Make every item in a restored copy win over what the apps have, and bring
/// back the items deleted since: their change stamps become `now` and each
/// one counts as restored.
pub fn mark_restored(data: &mut Value, now: i64) {
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
}

/// What a request from an app turned into.
enum Reply {
    To(Msg),
    Denied(String),
}

struct Hub {
    home: PathBuf,
    inner: Mutex<Inner>,
    updates: broadcast::Sender<Arc<str>>,
    /// device ids whose access was just removed
    kicks: broadcast::Sender<Arc<Vec<String>>>,
    /// roles or invites changed: each connection re-sends access info
    access: broadcast::Sender<()>,
    stop: watch::Receiver<bool>,
    connections: AtomicUsize,
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
            let _ = self.updates.send(Arc::from(Msg::Update { data: c.clone(), rev: g.saved.rev }.to_json()));
        }
    }

    /// Check a hello. Ok: (member, role, tag, new token).
    fn sign_in(&self, device: &str, tag: Option<u16>, token: Option<String>, invite: Option<String>, claim: Option<u64>) -> Result<(u64, Role, u16, Option<String>), String> {
        let mut g = self.inner.lock().unwrap();
        if let Some(code) = invite.filter(|c| !c.trim().is_empty()) {
            let Some(inv) = g.saved.take_invite(&code) else {
                return Err("That invite code doesn't work. It may be used already or expired (codes last 7 days). Ask for a new one.".into());
            };
            let (tag, token) = g.saved.add_device(device, inv.member, tag);
            if inv.role > g.saved.role_of(inv.member) {
                g.saved.set_role(inv.member, inv.role);
            }
            g.dirty = true;
            let role = g.saved.role_of(inv.member);
            drop(g);
            let _ = self.access.send(());
            return Ok((inv.member, role, tag, Some(token)));
        }
        if let Some(token) = token {
            return match g.saved.devices.get(device) {
                Some(d) if d.token == hash_token(&token) => Ok((d.member, g.saved.role_of(d.member), d.tag, None)),
                _ => Err("This computer isn't signed in to this server anymore. Ask the owner or an admin for a new invite code.".into()),
            };
        }
        if let Some(member) = claim {
            // only the very first claim wins: two people setting up the same
            // empty server at once must not end up with their workspaces merged
            if g.canon.is_none() && !g.saved.devices.is_empty() {
                return Err("Someone else is setting up this server right now. Ask them for an invite code.".into());
            }
            if g.canon.is_none() && g.saved.devices.is_empty() {
                let (tag, token) = g.saved.add_device(device, member, tag);
                g.saved.set_role(member, Role::Owner);
                g.dirty = true;
                return Ok((member, Role::Owner, tag, Some(token)));
            }
        }
        Err("This server already has a workspace. Ask its owner or an admin for an invite code.".into())
    }

    fn welcome(&self, tag: u16, member: u64, role: Role, token: Option<String>) -> String {
        let g = self.inner.lock().unwrap();
        Msg::Welcome { tag, snapshot: g.canon.clone(), rev: g.saved.rev, member, role, token }.to_json()
    }

    /// Merge an app's copy into the team's copy; tell everyone when it changed.
    fn push(&self, data: Value) {
        let data = wb_sync::shareable(&data);
        let mut g = self.inner.lock().unwrap();
        let members_before = member_ids(g.canon.as_ref());
        let changed = match g.canon.as_mut() {
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
            self.broadcast_canon(&mut g);
            // someone added or removed people: owners and admins see the new list
            if member_ids(g.canon.as_ref()) != members_before {
                drop(g);
                let _ = self.access.send(());
            }
        }
    }

    fn latest(&self) -> Option<String> {
        let g = self.inner.lock().unwrap();
        g.canon.as_ref().map(|c| Msg::Update { data: c.clone(), rev: g.saved.rev }.to_json())
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
                Some(Msg::Invite { member: who, code })
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
                g.saved.invites.retain(|_, i| i.member != who);
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
        {
            mark_restored(&mut data, now_ms());
            let data = wb_sync::shareable(&data);
            let mut g = self.inner.lock().unwrap();
            if let Some(cur) = &g.canon {
                let keep = self.home.join(BACKUP_DIR).join(format!("before-restore-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S")));
                let _ = std::fs::create_dir_all(self.home.join(BACKUP_DIR));
                let _ = write_atomic(&keep, &serde_json::to_string(cur).unwrap_or_default());
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

async fn ws_route(ws: WebSocketUpgrade, State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    ws.max_message_size(wb_net::MAX_MESSAGE).max_frame_size(wb_net::MAX_MESSAGE).on_upgrade(move |socket| connection(socket, hub))
}

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
    socket.send(Message::Text(text.into())).await.is_ok()
}

struct Counted(Arc<Hub>);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::Relaxed);
    }
}

async fn connection(mut socket: WebSocket, hub: Arc<Hub>) {
    hub.connections.fetch_add(1, Ordering::Relaxed);
    let _count = Counted(hub.clone());
    let mut stop = hub.stop.clone();
    // the first message must be hello
    let hello = tokio::select! {
        m = tokio::time::timeout(Duration::from_secs(10), socket.recv()) => m,
        _ = stop.changed() => return,
    };
    let Ok(Some(Ok(Message::Text(t)))) = hello else { return };
    let Some(Msg::Hello { device, tag, token, invite, claim }) = Msg::parse(t.as_str()) else {
        let _ = send(&mut socket, &Msg::error("Say hello first.", None).to_json()).await;
        return;
    };
    let (member, role, tag, token) = match hub.sign_in(&device, tag, token, invite, claim) {
        Ok(x) => x,
        Err(why) => {
            let _ = send(&mut socket, &Msg::error(&why, Some(CODE_AUTH)).to_json()).await;
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
    };
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
            m = tokio::time::timeout(silent, socket.recv()) => {
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
                        let hub2 = hub.clone();
                        // merging can take a moment on a big workspace
                        let _ = tokio::task::spawn_blocking(move || hub2.push(data)).await;
                        None
                    }
                    Some(Msg::Ping) => Some(Msg::Pong.to_json()),
                    Some(Msg::CallJoin { channel }) => Some(match hub.call(member, &device, channel) {
                        Ok(m) => m.to_json(),
                        Err(why) => Msg::error(&why, Some(CODE_DENIED)).to_json(),
                    }),
                    Some(m @ (Msg::CreateInvite { .. } | Msg::SetRole { .. } | Msg::Revoke { .. } | Msg::AccessList | Msg::BackupList | Msg::Restore { .. })) => match hub.request(member, m) {
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
                    Ok(t) => t.to_string(),
                    // fell behind: the newest copy says it all
                    Err(broadcast::error::RecvError::Lagged(_)) => match hub.latest() { Some(t) => t, None => continue },
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

/// Serve on `listener` until `shutdown` finishes, keeping the copy in `home`.
pub async fn serve(listener: tokio::net::TcpListener, home: PathBuf, shutdown: impl Future<Output = ()> + Send + 'static) -> std::io::Result<()> {
    std::fs::create_dir_all(&home)?;
    let canon = read_json(&home.join(WORKSPACE_FILE))?;
    let saved = load_saved(&home)?;
    let (stop_tx, stop_rx) = watch::channel(false);
    let hub = Arc::new(Hub {
        home,
        inner: Mutex::new(Inner { canon, saved, dirty: false }),
        updates: broadcast::channel(64).0,
        kicks: broadcast::channel(16).0,
        access: broadcast::channel(16).0,
        stop: stop_rx,
        connections: AtomicUsize::new(0),
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
    let app = Router::new().route(wb_net::PATH, get(ws_route)).route("/health", get(health)).route("/updates/{file}", get(update_route)).with_state(hub.clone());
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
}
