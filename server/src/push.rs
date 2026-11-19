// Push notifications to iPhones through Apple's push service (APNs).
//
// The iPhone app sends `push_token {token, platform: "ios", env}` after every
// welcome. The server keeps the newest token of each device in server.json
// (Saved::push) and, when a push from an app brings chat messages the team's
// copy didn't have, notifies:
//   - in a DM: every member of it except the author, for every message;
//   - in a channel: members it @mentions (wb_sync::mentions; the app saves
//     team, @channel and @here mentions as ids too), and for a thread reply
//     also whoever started or wrote in that thread;
//   - and what each phone's choices add or take away (`prefs` on push_token,
//     wb_net::PushPrefs, `wanted`): channels set to every message or to
//     nothing, keywords, paused notifications, DMs, mentions or replies off;
// never the author (nor the member whose app sent the copy), never someone a
// private channel is closed to, never for messages that came back from
// someone's Trash, and never a device that is connected right now (the app
// shows those itself). At most MAX_PER_DEVICE notifications go
// to one device per merge; the rest become one "N more new messages".
//
// Set up by an apns.json in the server folder (like mail.json):
//
//   {
//     "key": "AuthKey_ABC123DEFG.p8",      the key file from Apple (a path;
//                                           relative to the server folder)
//     "key_id": "ABC123DEFG",              the key's id
//     "team_id": "ABCDE12345",             the Apple developer team id
//     "topic": "com.example.workboard"     the iPhone app's bundle id
//   }
//
// or the environment variables WB_APNS_KEY, WB_APNS_KEY_ID, WB_APNS_TEAM_ID
// and WB_APNS_TOPIC (each wins over the file). Without all four nothing is
// sent and the server says so once when it starts. It reads the setup when it
// starts, so restart it after changing either.
//
// Sending: token-based auth (a JWT signed with the key, ES256, made again
// every 50 minutes) and one HTTPS POST per notification to
// api.push.apple.com, or api.sandbox.push.apple.com for tokens whose env is
// "development". APNs speaks only HTTP/2, which the HTTP crates already in
// this build don't, so it goes through macOS's /usr/bin/curl --http2. The JWT
// and the payload travel on curl's stdin (a curl config), not its arguments,
// so they don't show in the process list.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use wb_sync::json::{array_at, text_field, u64_field};

use crate::Locked;

pub const APNS_FILE: &str = "apns.json";
/// Notifications one device gets from one merge before the rest are summed up.
pub const MAX_PER_DEVICE: usize = 3;
/// Messages older than this (from a computer that was offline a long time)
/// don't notify anyone.
pub const MAX_AGE_SECS: i64 = 24 * 3600;
/// A message in a conversation restored from Trash notifies only when it
/// was written at most this long before the conversation came back (one
/// sent from a computer that was offline meanwhile, say).
pub const RESTORED_SLACK_SECS: i64 = 5 * 60;
/// How long one JWT is used (Apple wants a new one at least every hour and at
/// most every 20 minutes).
pub const JWT_SECS: i64 = 50 * 60;
/// The longest body in a notification (characters).
const MAX_BODY: usize = 180;

/// A device's push token, as the app sent it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PushToken {
    /// hex, from the device
    pub token: String,
    pub platform: String,
    /// "development" or "production"
    pub env: String,
    pub updated: i64,
    /// The phone's own name for this workspace (`push_token`'s
    /// `workspace`), sent back as "ws" in each notification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// What the device wants to be told about (None: an app from before
    /// choices, `wanted`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefs: Option<wb_net::PushPrefs>,
}

/// A workspace name from a phone worth keeping: short, letters and digits.
pub fn workspace_key_ok(key: &str) -> bool {
    (1..=64).contains(&key.len()) && key.chars().all(|c| c.is_ascii_alphanumeric())
}

/// A token worth keeping: hex, of a sensible length.
pub fn token_ok(token: &str) -> bool {
    (16..=200).contains(&token.len()) && token.chars().all(|c| c.is_ascii_hexdigit())
}

/// The env an app may say; anything else counts as production.
pub fn norm_env(env: &str) -> &'static str {
    if env.trim().eq_ignore_ascii_case("development") || env.trim().eq_ignore_ascii_case("sandbox")
    {
        "development"
    } else {
        "production"
    }
}

// ------------------------------------------------------------------ setup

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct ApnsCfg {
    pub key: String,
    pub key_id: String,
    pub team_id: String,
    pub topic: String,
}

/// The APNs setup from apns.json and WB_APNS_* (these win). Err says why
/// push is off.
pub fn load_apns(home: &Path) -> Result<ApnsCfg, String> {
    let file: Option<ApnsCfg> = std::fs::read_to_string(home.join(APNS_FILE))
        .ok()
        .map(|s| serde_json::from_str(&s).map_err(|e| format!("{} doesn't read: {}", APNS_FILE, e)))
        .transpose()?;
    let env = |k: &str| {
        std::env::var(k)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let any_env = [
        "WB_APNS_KEY",
        "WB_APNS_KEY_ID",
        "WB_APNS_TEAM_ID",
        "WB_APNS_TOPIC",
    ]
    .iter()
    .any(|k| env(k).is_some());
    if file.is_none() && !any_env {
        return Err(format!(
            "not set up (no {} and no WB_APNS_* variables)",
            APNS_FILE
        ));
    }
    let f = file.unwrap_or_default();
    let pick = |var: &str, v: &str| env(var).unwrap_or_else(|| v.trim().to_string());
    let mut cfg = ApnsCfg {
        key: pick("WB_APNS_KEY", &f.key),
        key_id: pick("WB_APNS_KEY_ID", &f.key_id),
        team_id: pick("WB_APNS_TEAM_ID", &f.team_id),
        topic: pick("WB_APNS_TOPIC", &f.topic),
    };
    let missing: Vec<&str> = [
        ("key", &cfg.key),
        ("key_id", &cfg.key_id),
        ("team_id", &cfg.team_id),
        ("topic", &cfg.topic),
    ]
    .iter()
    .filter(|(_, v)| v.is_empty())
    .map(|(k, _)| *k)
    .collect();
    if !missing.is_empty() {
        return Err(format!(
            "{} missing in {} / WB_APNS_*",
            missing.join(", "),
            APNS_FILE
        ));
    }
    if Path::new(&cfg.key).is_relative() {
        cfg.key = home.join(&cfg.key).to_string_lossy().into_owned();
    }
    Ok(cfg)
}

// ---------------------------------------------------------------- sending

/// What APNs said to one notification.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Sent,
    /// The token is no good anymore (app removed, or not a token of this
    /// app): forget it.
    Gone,
    Failed(String),
}

/// Whatever delivers notifications; tests use a fake one.
pub trait Sender: Send + Sync + std::fmt::Debug {
    fn send(&self, env: &str, token: &str, payload: &Value) -> Outcome;
}

/// The real sender: APNs over HTTP/2 with token-based auth.
pub struct Apns {
    cfg: ApnsCfg,
    key: ring::signature::EcdsaKeyPair,
    /// (JWT, made at, in seconds)
    jwt: Mutex<Option<(String, i64)>>,
    curl: String,
}

impl std::fmt::Debug for Apns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Apns")
            .field("key_id", &self.cfg.key_id)
            .field("team_id", &self.cfg.team_id)
            .field("topic", &self.cfg.topic)
            .finish()
    }
}

/// The DER inside a PEM file (the .p8 Apple gives is one).
fn pem_der(text: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    let b64: String = text
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .flat_map(|l| l.chars())
        .filter(|c| !c.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("not a PEM key file: {}", e))
}

impl Apns {
    pub fn new(cfg: ApnsCfg) -> Result<Apns, String> {
        let text = std::fs::read_to_string(&cfg.key)
            .map_err(|e| format!("couldn't read the key {}: {}", cfg.key, e))?;
        let der = pem_der(&text)?;
        let rng = ring::rand::SystemRandom::new();
        let key = ring::signature::EcdsaKeyPair::from_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            &der,
            &rng,
        )
        .map_err(|e| format!("{} isn't an APNs (P-256) key: {}", cfg.key, e))?;
        Ok(Apns {
            cfg,
            key,
            jwt: Mutex::new(None),
            curl: "/usr/bin/curl".into(),
        })
    }

    /// The JWT to send at `now` (seconds): the same one for JWT_SECS.
    pub fn jwt(&self, now: i64) -> Result<String, String> {
        let mut g = self.jwt.locked();
        if let Some((t, made)) = g.as_ref() {
            if now - made < JWT_SECS && now >= *made {
                return Ok(t.clone());
            }
        }
        let header = super::b64url(
            json!({"alg": "ES256", "kid": self.cfg.key_id})
                .to_string()
                .as_bytes(),
        );
        let claims = super::b64url(
            json!({"iss": self.cfg.team_id, "iat": now})
                .to_string()
                .as_bytes(),
        );
        let input = format!("{}.{}", header, claims);
        let sig = self
            .key
            .sign(&ring::rand::SystemRandom::new(), input.as_bytes())
            .map_err(|_| "couldn't sign the APNs token".to_string())?;
        let t = format!("{}.{}", input, super::b64url(sig.as_ref()));
        *g = Some((t.clone(), now));
        Ok(t)
    }

    /// The bundle id notifications go to.
    pub fn topic(&self) -> &str {
        &self.cfg.topic
    }

    fn forget_jwt(&self) {
        *self.jwt.locked() = None;
    }
}

pub fn apns_host(env: &str) -> &'static str {
    if norm_env(env) == "development" {
        "api.sandbox.push.apple.com"
    } else {
        "api.push.apple.com"
    }
}

/// A value in a curl config file: quoted, with \ and " escaped.
pub fn curl_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The curl config for one notification.
pub fn curl_config(url: &str, headers: &[(&str, String)], body: &str) -> String {
    let mut c = format!("url = {}\n", curl_quote(url));
    for (k, v) in headers {
        c.push_str(&format!(
            "header = {}\n",
            curl_quote(&format!("{}: {}", k, v))
        ));
    }
    c.push_str(&format!("data-binary = {}\n", curl_quote(body)));
    c
}

/// What an APNs answer means: (status, body) -> outcome.
pub fn outcome(status: u16, body: &str) -> Outcome {
    let reason = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("reason")?.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    match status {
        200 => Outcome::Sent,
        410 => Outcome::Gone,
        _ if reason == "BadDeviceToken" || reason == "Unregistered" => Outcome::Gone,
        _ => Outcome::Failed(if reason.is_empty() {
            format!("APNs answered {}", status)
        } else {
            format!("APNs answered {} {}", status, reason)
        }),
    }
}

impl Sender for Apns {
    fn send(&self, env: &str, token: &str, payload: &Value) -> Outcome {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let jwt = match self.jwt(now) {
            Ok(j) => j,
            Err(e) => return Outcome::Failed(e),
        };
        let url = format!("https://{}/3/device/{}", apns_host(env), token);
        let headers = [
            ("authorization", format!("bearer {}", jwt)),
            ("apns-topic", self.cfg.topic.clone()),
            ("apns-push-type", "alert".to_string()),
            ("apns-priority", "10".to_string()),
        ];
        let config = curl_config(&url, &headers, &payload.to_string());
        let child = Command::new(&self.curl)
            .args([
                "--http2",
                "--silent",
                "--show-error",
                "--max-time",
                "20",
                "--config",
                "-",
                "--output",
                "-",
                "--write-out",
                "\n%{http_code}",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => return Outcome::Failed(format!("couldn't run {}: {}", self.curl, e)),
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(config.as_bytes());
        }
        let out = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => return Outcome::Failed(e.to_string()),
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
        let status: u16 = code.trim().parse().unwrap_or(0);
        if status == 0 {
            return Outcome::Failed(format!(
                "couldn't reach APNs: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let res = outcome(status, body);
        if status == 403 {
            // ExpiredProviderToken / InvalidProviderToken: make a new JWT next time
            self.forget_jwt();
        }
        res
    }
}

// ------------------------------------------------------- who gets what

/// The ids of every message and reply in a copy.
pub fn message_ids(copy: &Value) -> HashSet<u64> {
    message_ids_in(copy, None)
}

/// The ids of the channels and DMs in a copy (a whole one or a change).
pub fn channel_ids(copy: &Value) -> HashSet<u64> {
    array_at(copy, "channels")
        .iter()
        .filter_map(|c| u64_field(c, "id"))
        .collect()
}

/// `message_ids`, of the conversations in `only` alone (all when None).
pub fn message_ids_in(copy: &Value, only: Option<&HashSet<u64>>) -> HashSet<u64> {
    let mut s = HashSet::new();
    for ch in array_at(copy, "channels") {
        if only.is_some_and(|o| !u64_field(ch, "id").is_some_and(|i| o.contains(&i))) {
            continue;
        }
        for m in array_at(ch, "messages") {
            s.extend(m.get("id").and_then(|i| i.as_u64()));
            for r in array_at(m, "replies") {
                s.extend(r.get("id").and_then(|i| i.as_u64()));
            }
        }
    }
    s
}

/// Why a member might be told about a message, strongest last: each
/// device's choices (`wb_net::PushPrefs`, `wanted`) decide which reach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Why {
    /// a channel message that doesn't name them: only for channels set to
    /// every message, or when it has one of their keywords
    Channel,
    /// a reply in a thread they started or wrote in
    Thread,
    /// it @mentions them (by name, team, @channel or @here)
    Mention,
    /// a direct message
    Direct,
}

/// One notification for one member.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub member: u64,
    pub channel: u64,
    /// the message or reply
    pub id: u64,
    pub payload: Value,
    pub why: Why,
    /// the message's text, for keywords
    pub text: String,
}

/// Whether a device whose choices are `prefs` (None: an app that never sent
/// any, which gets what every app got before choices came: direct messages,
/// mentions and thread replies) wants `n`, at `now_secs`. The same rules as
/// the apps' own notifications (notify_candidates in src/app/notify.rs).
pub fn wanted(n: &Note, prefs: Option<&wb_net::PushPrefs>, now_secs: i64) -> bool {
    let Some(p) = prefs else {
        return n.why != Why::Channel;
    };
    if p.muted_until > now_secs {
        return false;
    }
    if n.why == Why::Direct {
        return p.direct;
    }
    match p.channel_mode(n.channel) {
        "nothing" => false,
        "all" => true,
        _ => match n.why {
            Why::Mention => p.mentions,
            Why::Thread => p.replies,
            _ => p.mentions && wb_sync::mentions::has_keyword(&n.text, &p.keywords).is_some(),
        },
    }
}

/// The notification the iPhone app understands (ios/Workboard/Notifications.swift).
pub fn payload(title: &str, body: &str, channel: u64, parent: Option<u64>) -> Value {
    let mut alert = json!({"title": title, "body": body});
    if parent.is_some() {
        alert["subtitle"] = json!("Thread reply");
    }
    let mut p = json!({"aps": {"alert": alert, "sound": "default", "thread-id": format!("ch-{}", channel)}, "channel": channel});
    if let Some(parent) = parent {
        p["parent"] = json!(parent);
    }
    p
}

fn body_of(m: &Value) -> String {
    let t = text_field(m, "text");
    let t = t.trim();
    if t.is_empty() {
        return sent_what(array_at(m, "attachments"));
    }
    if t.chars().count() > MAX_BODY {
        format!(
            "{}...",
            t.chars().take(MAX_BODY).collect::<String>().trim_end()
        )
    } else {
        t.to_string()
    }
}

/// What a message with no text says: a photo, a video, both, or a file
/// (the same words as the iPhone's own banners, which it translates).
fn sent_what(a: &[Value]) -> String {
    let kind = |x: &Value| {
        x.get("kind")
            .and_then(Value::as_str)
            .unwrap_or("Image")
            .to_string()
    };
    let kinds: HashSet<String> = a.iter().map(kind).collect();
    let has = |k: &str| kinds.contains(k);
    if a.is_empty() {
        "New message"
    } else if kinds.iter().any(|k| k != "Image" && k != "Video") {
        "Sent a file"
    } else if has("Image") && has("Video") {
        "Sent photos and videos"
    } else if has("Video") {
        "Sent a video"
    } else {
        "Sent a photo"
    }
    .into()
}

/// Seconds, also for a stamp that is in milliseconds.
fn secs(ts: i64) -> i64 {
    if ts > 100_000_000_000 {
        ts / 1000
    } else {
        ts
    }
}

/// The notifications that the messages and replies in `canon` whose ids are
/// not in `before` call for. `skip`: members who get nothing (the one whose
/// app sent them).
pub fn notes(canon: &Value, before: &HashSet<u64>, skip: &[u64], now_secs: i64) -> Vec<Note> {
    notes_in(canon, before, None, skip, now_secs)
}

/// `notes`, looking at the conversations in `only` alone (all when None):
/// after a merge, only those the incoming copy has can hold new messages,
/// so a busy workspace isn't read through for every change.
pub fn notes_in(
    canon: &Value,
    before: &HashSet<u64>,
    only: Option<&HashSet<u64>>,
    skip: &[u64],
    now_secs: i64,
) -> Vec<Note> {
    // (id, full name, display name): pushes name people as the app shows
    // them (the display name when there is one), and either name mentions
    let people: Vec<(u64, String, String)> = array_at(canon, "members")
        .iter()
        .filter_map(|m| {
            Some((
                u64_field(m, "id")?,
                text_field(m, "name").trim().to_string(),
                text_field(m, "display_name").trim().to_string(),
            ))
        })
        .collect();
    let name = |id: u64| {
        people
            .iter()
            .find(|(i, _, _)| *i == id)
            .map(|(_, n, d)| if d.is_empty() { n.clone() } else { d.clone() })
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Someone".into())
    };
    // the app's rule (wb_sync::mentions): the ids a message keeps, else its text
    let named: Vec<(u64, &str)> = people
        .iter()
        .flat_map(|(i, n, d)| [(*i, n.as_str()), (*i, d.as_str())])
        .filter(|(_, n)| !n.is_empty())
        .collect();
    let mentioned = |m: &Value| -> Vec<u64> {
        let saved: Vec<u64> = array_at(m, "mentions")
            .iter()
            .filter_map(|x| x.as_u64())
            .collect();
        wb_sync::mentions::mentions_of(&saved, &text_field(m, "text"), &named)
    };
    // restored from someone's Trash (or a delete undone): not new, so no
    // push again. An id the merge revived, or a message written well before
    // its conversation came back.
    let revived = |id: u64| {
        canon
            .pointer(&format!("/extra/sync/revived/{}", id))
            .and_then(|r| r.as_i64())
    };
    let mut out = vec![];
    for ch in array_at(canon, "channels") {
        let Some(cid) = u64_field(ch, "id") else {
            continue;
        };
        if only.is_some_and(|o| !o.contains(&cid)) {
            continue;
        }
        let back_at = revived(cid).map(secs);
        let restored = |m: &Value| {
            let ts = secs(m.get("ts").and_then(|t| t.as_i64()).unwrap_or(0));
            u64_field(m, "id").is_some_and(|id| revived(id).is_some())
                || back_at.is_some_and(|at| ts < at - RESTORED_SLACK_SECS)
        };
        let direct = ch.get("kind").and_then(|k| k.as_str()) == Some("Direct");
        let ch_members: Vec<u64> = array_at(ch, "members")
            .iter()
            .filter_map(|m| m.as_u64())
            .collect();
        let title = |author: u64| {
            if direct {
                name(author)
            } else {
                format!("{} in #{}", name(author), text_field(ch, "name"))
            }
        };
        // everyone who may read it: a public channel is everyone's
        let readers: Vec<u64> =
            if direct || ch.get("private").and_then(|p| p.as_bool()) == Some(true) {
                ch_members.clone()
            } else {
                people.iter().map(|p| p.0).collect()
            };
        let mut add = |author: u64, m: &Value, parent: Option<u64>, mut to: Vec<(u64, Why)>| {
            let Some(id) = u64_field(m, "id") else { return };
            // brought in by an import (Slack history, say): old history, not
            // news, however recent its time (Message::imported in the app)
            if m.get("imported").is_some_and(|i| !i.is_null()) {
                return;
            }
            if now_secs - secs(m.get("ts").and_then(|t| t.as_i64()).unwrap_or(0)) > MAX_AGE_SECS {
                return;
            }
            // the strongest reason for each member
            to.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
            to.dedup_by_key(|x| x.0);
            let p = payload(&title(author), &body_of(m), cid, parent);
            let text = text_field(m, "text");
            for (member, why) in to {
                if member == author || skip.contains(&member) || wb_sync::closed_to(ch, member) {
                    continue;
                }
                out.push(Note {
                    member,
                    channel: cid,
                    id,
                    payload: p.clone(),
                    why,
                    text: text.clone(),
                });
            }
        };
        let with = |ids: Vec<u64>, why: Why| ids.into_iter().map(move |i| (i, why));
        for m in array_at(ch, "messages") {
            let Some(mid) = u64_field(m, "id") else {
                continue;
            };
            let author = u64_field(m, "author").unwrap_or(0);
            if !before.contains(&mid) && !restored(m) {
                let to: Vec<(u64, Why)> = if direct {
                    with(ch_members.clone(), Why::Direct).collect()
                } else {
                    with(mentioned(m), Why::Mention)
                        .chain(with(readers.clone(), Why::Channel))
                        .collect()
                };
                add(author, m, None, to);
            }
            let replies = array_at(m, "replies");
            for r in replies {
                let Some(rid) = u64_field(r, "id") else {
                    continue;
                };
                if before.contains(&rid) || restored(r) {
                    continue;
                }
                let ra = u64_field(r, "author").unwrap_or(0);
                let to: Vec<(u64, Why)> = if direct {
                    with(ch_members.clone(), Why::Direct).collect()
                } else {
                    // the thread's people: who started it and who wrote in it
                    let mut thread = vec![author];
                    thread.extend(replies.iter().filter_map(|x| u64_field(x, "author")));
                    with(mentioned(r), Why::Mention)
                        .chain(with(thread, Why::Thread))
                        .chain(with(readers.clone(), Why::Channel))
                        .collect()
                };
                add(ra, r, Some(mid), to);
            }
        }
    }
    out
}

/// What one device gets from a merge's notes for its member: the newest
/// MAX_PER_DEVICE as they are, or the newest MAX_PER_DEVICE - 1 and one
/// "N more new messages".
pub fn batch(notes: &[&Note]) -> Vec<Value> {
    if notes.len() <= MAX_PER_DEVICE {
        return notes.iter().map(|n| n.payload.clone()).collect();
    }
    let keep = MAX_PER_DEVICE - 1;
    let (rest, newest) = notes.split_at(notes.len() - keep);
    let channels: HashSet<u64> = rest.iter().map(|n| n.channel).collect();
    let body = format!("{} more new messages", rest.len());
    let summary = if channels.len() == 1 {
        // all in one conversation: "#general" (or the DM's person) as the title
        let ch = rest[0].channel;
        let title = rest[0].payload["aps"]["alert"]["title"]
            .as_str()
            .unwrap_or("Workboard");
        let place = title
            .rsplit_once(" in #")
            .map(|(_, p)| format!("#{}", p))
            .unwrap_or_else(|| title.to_string());
        json!({"aps": {"alert": {"title": place, "body": body}, "sound": "default", "thread-id": format!("ch-{}", ch)}, "channel": ch})
    } else {
        json!({"aps": {"alert": {"title": "Workboard", "body": body}, "sound": "default"}})
    };
    let mut out = vec![summary];
    out.extend(newest.iter().map(|n| n.payload.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    fn ws(channels: Value) -> Value {
        json!({"members": [{"id": 1, "name": "Alex Kim"}, {"id": 2, "name": "Bo"}, {"id": 3, "name": "Cy"}, {"id": 4, "name": "Dee"}], "channels": channels})
    }

    fn msg(id: u64, author: u64, t: &str) -> Value {
        json!({"id": id, "author": author, "text": t, "ts": now()})
    }

    fn who(n: &[Note]) -> Vec<(u64, u64)> {
        n.iter().map(|n| (n.member, n.id)).collect()
    }

    // what a phone that never sent choices gets (the tests from before
    // choices came); `super::notes` also has the channel messages that only
    // choices let through
    fn notes(c: &Value, before: &HashSet<u64>, skip: &[u64], now: i64) -> Vec<Note> {
        super::notes(c, before, skip, now)
            .into_iter()
            .filter(|n| wanted(n, None, now))
            .collect()
    }

    fn notes_in(
        c: &Value,
        before: &HashSet<u64>,
        only: Option<&HashSet<u64>>,
        skip: &[u64],
        now: i64,
    ) -> Vec<Note> {
        super::notes_in(c, before, only, skip, now)
            .into_iter()
            .filter(|n| wanted(n, None, now))
            .collect()
    }

    #[test]
    fn each_phone_gets_what_its_choices_ask_for() {
        let mut root = msg(5, 1, "plan");
        root["replies"] = json!([msg(6, 2, "the deploy is done")]);
        let c = ws(json!([
            {"id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [root, msg(7, 2, "deploy tonight"), msg(8, 2, "@Cy lunch?")]},
            {"id": 11, "name": "", "kind": "Direct", "members": [2, 3], "messages": [msg(9, 2, "hey")]}
        ]));
        let all = super::notes(&c, &HashSet::new(), &[], now());
        let t = now();
        let got = |member: u64, p: Option<&wb_net::PushPrefs>| -> Vec<u64> {
            all.iter()
                .filter(|n| n.member == member && wanted(n, p, t))
                .map(|n| n.id)
                .collect()
        };
        // no choices sent: DMs, mentions and threads, as before
        assert_eq!(got(3, None), vec![8, 9]);
        assert_eq!(got(1, None), vec![6], "Alex started the thread");
        // keywords, like a mention
        let mut p = wb_net::PushPrefs {
            keywords: vec!["Deploy".into()],
            ..Default::default()
        };
        assert_eq!(got(3, Some(&p)), vec![6, 7, 8, 9]);
        // a channel set to every message, or to nothing
        p.keywords.clear();
        p.channel_modes = vec![wb_net::ChannelMode {
            channel: 10,
            mode: "all".into(),
        }];
        assert_eq!(got(4, Some(&p)), vec![5, 6, 7, 8]);
        p.channel_modes[0].mode = "nothing".into();
        assert_eq!(
            got(3, Some(&p)),
            vec![9],
            "not even the mention; the DM still"
        );
        p.direct = false;
        assert!(got(3, Some(&p)).is_empty());
        // paused
        let paused = wb_net::PushPrefs {
            muted_until: t + 60,
            ..Default::default()
        };
        assert!(got(3, Some(&paused)).is_empty());
        // the author never, the strongest reason once
        let loud = wb_net::PushPrefs {
            channel_default: "all".into(),
            ..Default::default()
        };
        assert_eq!(
            got(2, Some(&loud)),
            vec![5],
            "only Alex's: Bo wrote the rest"
        );
        assert_eq!(all.iter().filter(|n| n.member == 3 && n.id == 8).count(), 1);
    }

    #[test]
    fn a_dm_notifies_its_other_members_in_the_ios_payload_shape() {
        let c = ws(
            json!([{"id": 301, "name": "", "kind": "Direct", "members": [1, 2], "messages": [msg(5, 1, "lunch?")]}]),
        );
        let n = notes(&c, &HashSet::new(), &[], now());
        assert_eq!(who(&n), vec![(2, 5)], "the author never");
        assert_eq!(
            n[0].payload,
            json!({"aps": {"alert": {"title": "Alex Kim", "body": "lunch?"}, "sound": "default", "thread-id": "ch-301"}, "channel": 301})
        );
    }

    /// After a merge only the conversations the incoming copy brought are
    /// looked at: the rest of the workspace can't hold its new messages.
    #[test]
    fn only_the_conversations_a_change_brought_are_looked_at() {
        let c = ws(json!([
            {"id": 301, "name": "", "kind": "Direct", "members": [1, 2], "messages": [msg(5, 1, "lunch?")]},
            {"id": 302, "name": "", "kind": "Direct", "members": [1, 3], "messages": [msg(6, 1, "later?")]}
        ]));
        let change = json!({"channels": [{"id": 302, "messages": [msg(6, 1, "later?")]}]});
        let only = channel_ids(&change);
        assert_eq!(only, [302].into_iter().collect());
        let before = message_ids_in(&c, Some(&only));
        assert_eq!(
            before,
            [6].into_iter().collect(),
            "this conversation's alone"
        );
        let n = notes_in(&c, &HashSet::new(), Some(&only), &[], now());
        assert_eq!(who(&n), vec![(3, 6)], "not the DM the change didn't bring");
        assert_eq!(who(&notes(&c, &HashSet::new(), &[], now())).len(), 2);
    }

    #[test]
    fn a_channel_message_notifies_only_who_it_mentions() {
        let c = ws(
            json!([{"id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [msg(5, 2, "hi all"), msg(6, 2, "@Alex Kim and @Cy look"), msg(7, 3, "@Cy note to self")]}]),
        );
        let n = notes(&c, &[5].into_iter().collect(), &[], now());
        assert_eq!(who(&n), vec![(1, 6), (3, 6)]);
        assert_eq!(n[0].payload["aps"]["alert"]["title"], "Bo in #general");
        assert!(n[0].payload.get("parent").is_none());
    }

    #[test]
    fn a_thread_reply_notifies_who_started_or_wrote_in_the_thread() {
        let mut root = msg(5, 1, "plan");
        root["replies"] = json!([msg(6, 2, "ok"), msg(7, 3, "on it"), msg(8, 2, "thanks")]);
        let c = ws(
            json!([{"id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [root]}]),
        );
        let before: HashSet<u64> = [5, 6, 7].into_iter().collect();
        let n = notes(&c, &before, &[], now());
        assert_eq!(
            who(&n),
            vec![(1, 8), (3, 8)],
            "Alex started it, Cy wrote in it, Bo wrote the reply, Dee isn't in it"
        );
        assert_eq!(n[0].payload["parent"], 5);
        assert_eq!(n[0].payload["aps"]["alert"]["subtitle"], "Thread reply");
    }

    #[test]
    fn private_channels_old_messages_and_the_sender_stay_out() {
        let c = ws(json!([
            {"id": 10, "name": "secret", "kind": "Channel", "private": true, "members": [1, 2], "messages": [msg(5, 1, "@Bo @Cy")]},
            {"id": 11, "name": "general", "kind": "Channel", "members": [], "messages": [{"id": 6, "author": 1, "text": "@Bo old", "ts": now() - 3 * 86400}, {"id": 7, "author": 4, "text": "@Bo hi", "ts": now() * 1000}]}
        ]));
        let n = notes(&c, &HashSet::new(), &[], now());
        assert_eq!(
            who(&n),
            vec![(2, 5), (2, 7)],
            "Cy isn't in the private channel; a 3-day-old message is quiet; ms stamps work"
        );
        assert!(
            notes(&c, &HashSet::new(), &[2], now()).is_empty(),
            "nothing for the member whose app sent it"
        );
    }

    #[test]
    fn many_messages_at_once_become_a_few_and_a_summary() {
        let msgs: Vec<Value> = (0..7)
            .map(|i| msg(100 + i, 1, &format!("m{}", i)))
            .collect();
        let c = ws(
            json!([{"id": 301, "name": "", "kind": "Direct", "members": [1, 2], "messages": msgs}]),
        );
        let n = notes(&c, &HashSet::new(), &[], now());
        let refs: Vec<&Note> = n.iter().collect();
        let b = batch(&refs);
        assert_eq!(b.len(), MAX_PER_DEVICE);
        assert_eq!(b[0]["aps"]["alert"]["body"], "5 more new messages");
        assert_eq!(b[0]["channel"], 301);
        assert_eq!(b[1]["aps"]["alert"]["body"], "m5");
        assert_eq!(b[2]["aps"]["alert"]["body"], "m6");
        assert_eq!(batch(&refs[..2]).len(), 2);
    }

    #[test]
    fn long_and_empty_bodies() {
        let long = "x".repeat(500);
        let c = ws(
            json!([{"id": 301, "name": "", "kind": "Direct", "members": [1, 2], "messages": [msg(5, 1, &long), {"id": 6, "author": 1, "text": "", "ts": now(), "attachments": [{"file": "ab", "name": "a.png"}]}]}]),
        );
        let n = notes(&c, &HashSet::new(), &[], now());
        assert_eq!(
            n[0].payload["aps"]["alert"]["body"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            MAX_BODY + 3
        );
        assert_eq!(n[1].payload["aps"]["alert"]["body"], "Sent a photo");
    }

    #[test]
    fn attachment_only_messages_say_what_was_sent() {
        let one = |k: &str| json!({"file": "ab", "name": "a", "kind": k});
        assert_eq!(sent_what(&[]), "New message");
        assert_eq!(sent_what(&[one("Image"), one("Image")]), "Sent a photo");
        assert_eq!(sent_what(&[one("Video")]), "Sent a video");
        assert_eq!(
            sent_what(&[one("Image"), one("Video")]),
            "Sent photos and videos"
        );
        assert_eq!(sent_what(&[one("Image"), one("File")]), "Sent a file");
    }

    #[test]
    fn apns_answers_and_hosts() {
        assert_eq!(outcome(200, ""), Outcome::Sent);
        assert_eq!(
            outcome(410, r#"{"reason":"Unregistered","timestamp":1}"#),
            Outcome::Gone
        );
        assert_eq!(
            outcome(400, r#"{"reason":"BadDeviceToken"}"#),
            Outcome::Gone
        );
        assert_eq!(
            outcome(400, r#"{"reason":"DeviceTokenNotForTopic"}"#),
            Outcome::Failed("APNs answered 400 DeviceTokenNotForTopic".into())
        );
        assert_eq!(apns_host("development"), "api.sandbox.push.apple.com");
        assert_eq!(apns_host("production"), "api.push.apple.com");
        assert_eq!(apns_host("anything"), "api.push.apple.com");
        assert!(token_ok(&"ab".repeat(32)) && !token_ok("xyz") && !token_ok(""));
    }

    #[test]
    fn the_curl_config_quotes_what_it_carries() {
        let c = curl_config(
            "https://h/3/device/ab",
            &[("authorization", "bearer a.b.c".into())],
            r#"{"t":"say \"hi\"\\"}"#,
        );
        assert_eq!(c, "url = \"https://h/3/device/ab\"\nheader = \"authorization: bearer a.b.c\"\ndata-binary = \"{\\\"t\\\":\\\"say \\\\\\\"hi\\\\\\\"\\\\\\\\\\\"}\"\n");
    }

    #[test]
    fn the_jwt_is_es256_signed_and_reused_for_50_minutes() {
        use base64::Engine as _;
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::EcdsaKeyPair::generate_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            &rng,
        )
        .unwrap();
        let home = std::env::temp_dir().join(format!("wb-apns-{:x}", wb_sync::random_u64()));
        std::fs::create_dir_all(&home).unwrap();
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            base64::engine::general_purpose::STANDARD.encode(pkcs8.as_ref())
        );
        std::fs::write(home.join("AuthKey_TEST.p8"), pem).unwrap();
        std::fs::write(home.join(APNS_FILE), r#"{"key": "AuthKey_TEST.p8", "key_id": "KEY123", "team_id": "TEAM45", "topic": "com.example.wb"}"#).unwrap();
        let cfg = load_apns(&home).unwrap();
        assert!(
            cfg.key.starts_with(home.to_str().unwrap()),
            "relative to the server folder"
        );
        let apns = Apns::new(cfg).unwrap();
        let t = apns.jwt(1_000_000).unwrap();
        let parts: Vec<&str> = t.split('.').collect();
        let dec = |s: &str| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(s)
                .unwrap()
        };
        assert_eq!(
            serde_json::from_slice::<Value>(&dec(parts[0])).unwrap(),
            json!({"alg": "ES256", "kid": "KEY123"})
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&dec(parts[1])).unwrap(),
            json!({"iss": "TEAM45", "iat": 1_000_000})
        );
        use ring::signature::KeyPair;
        let public = ring::signature::EcdsaKeyPair::from_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            pkcs8.as_ref(),
            &rng,
        )
        .unwrap()
        .public_key()
        .as_ref()
        .to_vec();
        ring::signature::UnparsedPublicKey::new(&ring::signature::ECDSA_P256_SHA256_FIXED, public)
            .verify(
                format!("{}.{}", parts[0], parts[1]).as_bytes(),
                &dec(parts[2]),
            )
            .expect("signature checks out");
        assert_eq!(apns.jwt(1_000_000 + JWT_SECS - 1).unwrap(), t, "reused");
        assert_ne!(
            apns.jwt(1_000_000 + JWT_SECS).unwrap(),
            t,
            "made again after 50 minutes"
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn push_is_off_until_all_four_settings_are_there() {
        let home = std::env::temp_dir().join(format!("wb-apns-{:x}", wb_sync::random_u64()));
        std::fs::create_dir_all(&home).unwrap();
        if std::env::var("WB_APNS_KEY").is_err() {
            assert!(load_apns(&home).unwrap_err().contains("not set up"));
            std::fs::write(home.join(APNS_FILE), r#"{"key": "k.p8", "key_id": "K"}"#).unwrap();
            assert!(load_apns(&home)
                .unwrap_err()
                .contains("team_id, topic missing"));
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn mentions_follow_the_apps_rule_and_the_saved_ids() {
        let c = json!({"members": [{"id": 1, "name": "Alex"}, {"id": 2, "name": "Alex Kim"}, {"id": 3, "name": "Bo"}, {"id": 4, "name": "지원"}, {"id": 5, "name": "Bob"}],
        "channels": [{"id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [
            msg(5, 3, "@Alex Kim look"),
            msg(6, 3, "@Bobby and mail@Alex"),
            msg(7, 3, "@지원님 확인"),
            // the ids saved with the message win over its text (two Bobs, say)
            {"id": 8, "author": 3, "text": "@Bob hi", "ts": now(), "mentions": [5]}
        ]}]});
        let n = notes(&c, &HashSet::new(), &[], now());
        assert_eq!(who(&n), vec![(2, 5), (4, 7), (5, 8)]);
    }

    #[test]
    fn pushes_name_people_by_display_name_and_either_name_mentions() {
        let c = json!({"members": [
            {"id": 1, "name": "Seo Jiwon", "display_name": "Jiwon"},
            {"id": 2, "name": "Alex Kim", "display_name": ""},
            {"id": 3, "name": "Bo"}
        ],
        "channels": [{"id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [
            msg(5, 1, "@Alex Kim look"),
            msg(6, 2, "@Jiwon can you check"),
            msg(7, 3, "@Seo Jiwon too")
        ]}]});
        let n = notes(&c, &HashSet::new(), &[], now());
        assert_eq!(who(&n), vec![(2, 5), (1, 6), (1, 7)]);
        let title = |id: u64| {
            n.iter().find(|x| x.id == id).unwrap().payload["aps"]["alert"]["title"]
                .as_str()
                .unwrap()
                .to_string()
        };
        assert!(title(5).starts_with("Jiwon"), "{}", title(5));
        assert!(title(6).starts_with("Alex Kim"), "{}", title(6));
    }

    #[test]
    fn a_conversation_restored_from_trash_pushes_nothing_again() {
        let t = now();
        let mut root = msg(5, 2, "plan");
        root["replies"] = json!([msg(6, 3, "ok")]);
        let dm = |messages: Value| json!({"id": 301, "name": "", "kind": "Direct", "members": [1, 2, 3], "messages": messages});
        // the DM and its messages came back (their ids revived)
        let mut c = ws(json!([dm(json!([root.clone()]))]));
        c["extra"] = json!({"sync": {"revived": {"301": t * 1000, "5": t * 1000, "6": t * 1000}}});
        assert!(notes(&c, &HashSet::new(), &[], t).is_empty());
        // only the conversation was marked: its older messages still don't push,
        // one written just now does
        let mut fresh = msg(7, 2, "back again");
        fresh["ts"] = json!(t);
        let mut old = msg(8, 2, "from this morning");
        old["ts"] = json!(t - 3 * 3600);
        let mut c = ws(json!([dm(json!([old, fresh]))]));
        c["extra"] = json!({"sync": {"revived": {"301": t * 1000}}});
        let n = notes(&c, &HashSet::new(), &[], t);
        assert_eq!(who(&n), vec![(1, 7), (3, 7)]);
    }

    #[test]
    fn imported_messages_and_replies_send_no_push_however_recent() {
        let mut m = msg(5, 1, "@Bo from the Slack export");
        m["imported"] = json!("slack:C1:1700000000.000100");
        let mut r = msg(6, 3, "@Bo an imported reply");
        r["imported"] = json!("slack:C1:1700000000.000200");
        m["replies"] = json!([r, msg(7, 4, "a new reply here")]);
        let mut d = msg(9, 1, "imported DM");
        d["imported"] = json!("slack:D1:1700000000.000300");
        let c = ws(json!([
            {"id": 300, "name": "general", "kind": "Channel", "members": [], "messages": [m]},
            {"id": 301, "name": "", "kind": "Direct", "members": [1, 2], "messages": [d]}
        ]));
        let n = notes(&c, &HashSet::new(), &[], now());
        assert_eq!(
            who(&n),
            vec![(1, 7), (3, 7)],
            "only the reply written here notifies the thread's people"
        );
    }
}
