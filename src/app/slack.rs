// "Import from Slack": a Slack workspace export (.zip, from Slack's
// Settings > Import/Export Data) becomes channels and messages here.
//
// The export holds users.json (the people), channels.json (public
// channels; groups.json holds private ones when the export has them) and a
// folder per channel with one JSON file per day: [{"user", "text", "ts",
// "thread_ts", "files", ...}]. People are matched to members by email, then
// by name; the ones not found can be added as members, else their messages
// come in under my name with theirs in front. Messages keep their times,
// thread replies go under their message, files come in as links (the files
// themselves need a Slack sign-in to fetch). Importing the same export again
// adds nothing twice.
//
// The zip is opened with macOS's own `ditto`.

use super::*;
use crate::media;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SlackUser {
    pub id: String,
    /// the Slack handle
    pub handle: String,
    pub real_name: String,
    pub display: String,
    pub email: String,
}

impl SlackUser {
    /// The name to show: the real name, else the display name, else the handle.
    pub fn shown(&self) -> String {
        [&self.real_name, &self.display, &self.handle]
            .into_iter()
            .find(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| self.id.clone())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SlackMsg {
    /// the author's Slack id (empty for a bot or an app)
    pub user: String,
    /// a name to fall back on (a bot's, or the profile kept with the message)
    pub name: String,
    pub text: String,
    /// Slack's "1700000000.000200"
    pub ts: String,
    /// the message a reply belongs to (its ts); None for a plain message
    pub thread: Option<String>,
    /// (name, link) of each file
    pub files: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SlackChannel {
    pub name: String,
    pub topic: String,
    pub private: bool,
    pub members: Vec<String>,
    pub messages: Vec<SlackMsg>,
}

/// An export read and matched against this workspace, waiting for Import.
#[derive(Clone, Debug, Default)]
pub(super) struct SlackPlan {
    pub users: Vec<SlackUser>,
    pub channels: Vec<SlackChannel>,
    /// Slack id -> member
    pub matched: HashMap<String, u64>,
    /// people who wrote something and aren't members here (Slack ids)
    pub missing: Vec<String>,
    /// add the missing people as members (else their messages come in
    /// under my name)
    pub create_missing: bool,
    /// what happened, once imported
    pub done: Option<String>,
}

impl SlackPlan {
    pub fn message_count(&self) -> usize {
        self.channels.iter().map(|c| c.messages.len()).sum()
    }

    fn user(&self, id: &str) -> Option<&SlackUser> {
        self.users.iter().find(|u| u.id == id)
    }
}

fn s(v: &serde_json::Value, path: &[&str]) -> String {
    let mut v = v;
    for k in path {
        match v.get(k) {
            Some(x) => v = x,
            None => return String::new(),
        }
    }
    v.as_str().unwrap_or_default().to_string()
}

fn read(p: &Path) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {}", p.display(), e))
}

/// The folder in an unpacked export that holds channels.json (the zip may
/// have everything inside one folder).
fn export_root(dir: &Path) -> Option<PathBuf> {
    let has = |d: &Path| d.join("channels.json").is_file() || d.join("groups.json").is_file();
    if has(dir) {
        return Some(dir.to_path_buf());
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !p.file_name().is_some_and(|n| n == "__MACOSX"))
        .find(|p| has(p))
}

/// Read an unpacked Slack export.
pub(super) fn read_export(dir: &Path) -> Result<(Vec<SlackUser>, Vec<SlackChannel>), String> {
    let root = export_root(dir)
        .ok_or_else(|| "This isn't a Slack export: channels.json is missing.".to_string())?;
    let users: Vec<SlackUser> = match read(&root.join("users.json")) {
        Ok(v) => v
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|u| SlackUser {
                id: s(u, &["id"]),
                handle: s(u, &["name"]),
                real_name: {
                    let r = s(u, &["real_name"]);
                    if r.is_empty() {
                        s(u, &["profile", "real_name"])
                    } else {
                        r
                    }
                },
                display: s(u, &["profile", "display_name"]),
                email: s(u, &["profile", "email"]),
            })
            .filter(|u| !u.id.is_empty())
            .collect(),
        Err(_) => vec![],
    };
    let mut channels = vec![];
    for (file, private) in [("channels.json", false), ("groups.json", true)] {
        let Ok(list) = read(&root.join(file)) else {
            continue;
        };
        for c in list.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
            let name = s(c, &["name"]);
            // only a plain folder name inside the export
            if name.is_empty() || name.contains('/') || name.contains("..") {
                continue;
            }
            let mut days: Vec<PathBuf> = std::fs::read_dir(root.join(&name))
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.path())
                        .filter(|p| p.extension().is_some_and(|x| x == "json"))
                        .collect()
                })
                .unwrap_or_default();
            days.sort();
            let mut messages = vec![];
            for day in days {
                let Ok(v) = read(&day) else { continue };
                for m in v.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
                    if let Some(m) = message(m) {
                        messages.push(m);
                    }
                }
            }
            messages.sort_by(|a, b| ts_secs_f(&a.ts).total_cmp(&ts_secs_f(&b.ts)));
            channels.push(SlackChannel {
                name,
                topic: {
                    let t = s(c, &["topic", "value"]);
                    if t.is_empty() {
                        s(c, &["purpose", "value"])
                    } else {
                        t
                    }
                },
                private,
                members: c
                    .get("members")
                    .and_then(|m| m.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                messages,
            });
        }
    }
    if channels.is_empty() {
        return Err("This Slack export has no channels.".into());
    }
    Ok((users, channels))
}

/// Slack's notices (someone joined, the topic changed) aren't messages.
const SKIPPED: [&str; 9] = [
    "channel_join",
    "channel_leave",
    "channel_topic",
    "channel_purpose",
    "channel_name",
    "channel_archive",
    "channel_unarchive",
    "group_join",
    "group_leave",
];

fn message(m: &serde_json::Value) -> Option<SlackMsg> {
    if m.get("type").and_then(|t| t.as_str()).unwrap_or("message") != "message" {
        return None;
    }
    let sub = s(m, &["subtype"]);
    if SKIPPED.contains(&sub.as_str()) {
        return None;
    }
    let ts = s(m, &["ts"]);
    if ts.is_empty() {
        return None;
    }
    let thread = Some(s(m, &["thread_ts"])).filter(|t| !t.is_empty() && *t != ts);
    let files: Vec<(String, String)> = m
        .get("files")
        .and_then(|f| f.as_array())
        .map(|a| {
            a.iter()
                .map(|f| {
                    let name = [s(f, &["name"]), s(f, &["title"])]
                        .into_iter()
                        .find(|x| !x.is_empty())
                        .unwrap_or_else(|| "file".into());
                    let link = [s(f, &["permalink"]), s(f, &["url_private"])]
                        .into_iter()
                        .find(|x| !x.is_empty())
                        .unwrap_or_default();
                    (name, link)
                })
                .collect()
        })
        .unwrap_or_default();
    let name = [
        s(m, &["user_profile", "real_name"]),
        s(m, &["user_profile", "display_name"]),
        s(m, &["username"]),
        s(m, &["bot_profile", "name"]),
    ]
    .into_iter()
    .find(|x| !x.is_empty())
    .unwrap_or_default();
    let text = s(m, &["text"]);
    if text.trim().is_empty() && files.is_empty() {
        return None;
    }
    Some(SlackMsg {
        user: s(m, &["user"]),
        name,
        text,
        ts,
        thread,
        files,
    })
}

fn ts_secs_f(ts: &str) -> f64 {
    ts.parse::<f64>().unwrap_or(0.0)
}

/// Slack's "1700000000.000200" as whole seconds.
pub(super) fn ts_secs(ts: &str) -> i64 {
    ts.split('.')
        .next()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0)
}

/// Slack's markup as plain text: <@U1> becomes @Name, <#C1|general>
/// #general, <https://x|label> "label (https://x)", and &amp; &lt; &gt; the
/// characters.
pub(super) fn plain_text(text: &str, names: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(j) = after.find('>') else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let inner = &after[..j];
        let (target, label) = match inner.split_once('|') {
            Some((a, b)) => (a, Some(b)),
            None => (inner, None),
        };
        if let Some(id) = target.strip_prefix('@') {
            let name = label
                .map(String::from)
                .or_else(|| names.get(id).cloned())
                .unwrap_or_else(|| id.to_string());
            out.push('@');
            out.push_str(&name);
        } else if let Some(id) = target.strip_prefix('#') {
            out.push('#');
            out.push_str(label.unwrap_or(id));
        } else if let Some(special) = target.strip_prefix('!') {
            out.push('@');
            out.push_str(label.unwrap_or(special.split('^').next().unwrap_or(special)));
        } else {
            let link = target.strip_prefix("mailto:").unwrap_or(target);
            match label {
                Some(l) if l != link => out.push_str(&format!("{} ({})", l, link)),
                _ => out.push_str(link),
            }
        }
        rest = &after[j + 1..];
    }
    out.push_str(rest);
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Match the people in an export to members: by email, then by name.
pub(super) fn plan(data: &Data, users: Vec<SlackUser>, channels: Vec<SlackChannel>) -> SlackPlan {
    let low = |x: &str| x.trim().to_lowercase();
    let mut matched = HashMap::new();
    for u in &users {
        let by_mail = (!u.email.trim().is_empty())
            .then(|| data.members.iter().find(|m| low(&m.email) == low(&u.email)))
            .flatten();
        let by_name = || {
            data.members.iter().find(|m| {
                [&u.real_name, &u.display, &u.handle]
                    .iter()
                    .any(|n| !n.trim().is_empty() && low(n) == low(&m.name))
            })
        };
        if let Some(m) = by_mail.or_else(by_name) {
            matched.insert(u.id.clone(), m.id);
        }
    }
    let mut missing = vec![];
    for c in &channels {
        for m in &c.messages {
            if !m.user.is_empty() && !matched.contains_key(&m.user) && !missing.contains(&m.user) {
                missing.push(m.user.clone());
            }
        }
    }
    SlackPlan {
        users,
        channels,
        matched,
        missing,
        create_missing: true,
        done: None,
    }
}

/// Unpack a Slack export .zip into a new folder (macOS's `ditto`).
pub(super) fn unzip(zip: &Path) -> Result<PathBuf, String> {
    let dest = std::env::temp_dir().join(format!("wb-slack-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let out = std::process::Command::new("/usr/bin/ditto")
        .args(["-x", "-k"])
        .arg(zip)
        .arg(&dest)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let _ = std::fs::remove_dir_all(&dest);
        return Err("That file couldn't be opened as a .zip.".into());
    }
    Ok(dest)
}

/// Open an export .zip and match it against the workspace.
pub(super) fn load(data: &Data, zip: &Path) -> Result<SlackPlan, String> {
    let dir = unzip(zip)?;
    let read = read_export(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let (users, channels) = read?;
    Ok(plan(data, users, channels))
}

#[cfg(test)]
thread_local! {
    /// What the file picker "chooses" in tests (the real one is a window).
    pub(super) static PICKED: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

fn pick_export() -> Option<PathBuf> {
    #[cfg(test)]
    {
        PICKED.with(|p| p.borrow_mut().take())
    }
    #[cfg(not(test))]
    {
        media::pick_zip(tr("Choose the Slack export (.zip)"))
    }
}

fn new_message(id: u64, author: u64, text: String, ts: i64) -> Message {
    Message {
        id,
        author,
        text,
        mentions: vec![],
        ts,
        reactions: vec![],
        replies: vec![],
        saved: false,
        task: None,
        post_title: None,
        post_type: String::new(),
        pinned: false,
        call: None,
        attachments: vec![],
        edited: None,
    }
}

impl App {
    /// "Import from Slack": pick the export and show what it holds.
    pub(super) fn start_slack_import(&mut self) {
        let Some(zip) = pick_export() else {
            return;
        };
        self.open_slack_export(&zip);
    }

    pub(super) fn open_slack_export(&mut self, zip: &Path) {
        match load(&self.data, zip) {
            Ok(p) => self.modal = Some(Modal::SlackImport(Box::new(p))),
            Err(e) => {
                self.toast = Some((trf!("Couldn't read the Slack export: {}", tr(&e)), now_ts()))
            }
        }
    }

    /// Bring the export in. Returns (channels, messages) imported.
    pub(super) fn import_slack(&mut self, plan: &SlackPlan) -> (usize, usize) {
        let me = self.data.me;
        let mut people = plan.matched.clone();
        if plan.create_missing {
            for id in &plan.missing {
                let Some(u) = plan.user(id) else { continue };
                let mid = self.data.new_id();
                let color = STATUS_PALETTE[1 + (mid as usize % 13)];
                self.data.members.push(Member {
                    id: mid,
                    name: u.shown(),
                    color,
                    email: u.email.trim().to_string(),
                    role: "Member".into(),
                    invited: false,
                    status_text: String::new(),
                });
                people.insert(id.clone(), mid);
            }
        }
        // names for @mentions: the member's name here, else Slack's
        let names: HashMap<String, String> = plan
            .users
            .iter()
            .map(|u| {
                let n = people
                    .get(&u.id)
                    .and_then(|m| self.data.member(*m))
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| u.shown());
                (u.id.clone(), n)
            })
            .collect();
        let mut chans = 0;
        let mut count = 0;
        let mut touched = vec![];
        for sc in &plan.channels {
            let name = sc.name.trim().to_lowercase().replace(' ', "-");
            let existing = self
                .data
                .channels
                .iter()
                .position(|c| c.kind == ChannelKind::Channel && c.name == name);
            let ci = match existing {
                Some(i) => i,
                None => {
                    let id = self.data.new_id();
                    let mut members: Vec<u64> = sc
                        .members
                        .iter()
                        .filter_map(|u| people.get(u).copied())
                        .collect();
                    if members.is_empty() {
                        members = self.data.members.iter().map(|m| m.id).collect();
                    }
                    if !members.contains(&me) {
                        members.push(me);
                    }
                    self.data.channels.push(Channel {
                        id,
                        name,
                        kind: ChannelKind::Channel,
                        topic: sc.topic.clone(),
                        members,
                        messages: vec![],
                        last_read: now_ts(),
                        private: sc.private,
                        creator: me,
                        ..Default::default()
                    });
                    self.data.channels.len() - 1
                }
            };
            // Slack ts -> where that message landed (for its replies)
            let mut parents: HashMap<String, u64> = HashMap::new();
            let mut added = 0;
            for m in &sc.messages {
                let (author, prefix) = match people.get(&m.user) {
                    Some(id) => (*id, String::new()),
                    None => {
                        let who = plan
                            .user(&m.user)
                            .map(|u| u.shown())
                            .filter(|n| !n.is_empty())
                            .unwrap_or_else(|| {
                                if m.name.is_empty() {
                                    "Slack".into()
                                } else {
                                    m.name.clone()
                                }
                            });
                        (me, format!("{}: ", who))
                    }
                };
                let mut text = format!("{}{}", prefix, plain_text(&m.text, &names));
                for (n, link) in &m.files {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    if link.is_empty() {
                        text.push_str(&format!("[{}]", n));
                    } else {
                        text.push_str(&format!("[{}] {}", n, link));
                    }
                }
                let ts = ts_secs(&m.ts);
                let ch = &self.data.channels[ci];
                let same = |x: &Message| x.ts == ts && x.author == author && x.text == text;
                // already here (imported before)
                let found = ch.messages.iter().find_map(|x| {
                    if same(x) {
                        return Some(x.id);
                    }
                    x.replies.iter().find(|r| same(r)).map(|r| r.id)
                });
                if let Some(id) = found {
                    parents.insert(m.ts.clone(), id);
                    continue;
                }
                let id = self.data.new_id();
                let msg = new_message(id, author, text, ts);
                let ch = &mut self.data.channels[ci];
                let parent = m.thread.as_ref().and_then(|t| parents.get(t)).copied();
                match parent.and_then(|p| ch.messages.iter_mut().find(|x| x.id == p)) {
                    Some(p) => p.replies.push(msg),
                    None => {
                        ch.messages.push(msg);
                        parents.insert(m.ts.clone(), id);
                    }
                }
                added += 1;
            }
            let ch = &mut self.data.channels[ci];
            ch.messages.sort_by_key(|m| m.ts);
            if added > 0 || existing.is_none() {
                chans += 1;
                touched.push(ch.id);
            }
            count += added;
        }
        // old messages coming in aren't news
        for id in touched {
            self.mark_read(id);
        }
        self.notify_rt.seeded = false;
        self.dirty = true;
        (chans, count)
    }

    /// The Import from Slack dialog's contents. True when it should close.
    pub(super) fn slack_modal(&mut self, ui: &mut Ui, t: &Theme, plan: &mut SlackPlan) -> bool {
        if let Some(done) = &plan.done {
            ui.label(done.as_str());
            ui.add_space(8.0);
            return primary_button(ui, t, tr("OK")).clicked();
        }
        ui.label(trf!(
            "{} channels and {} messages in this export.",
            plan.channels.len(),
            plan.message_count()
        ));
        ui.label(
            RichText::new(trf!(
                "{} people are already members here (matched by email or name).",
                plan.matched.len()
            ))
            .color(t.text2),
        );
        if !plan.missing.is_empty() {
            let names: Vec<String> = plan
                .missing
                .iter()
                .map(|id| {
                    plan.user(id)
                        .map(|u| u.shown())
                        .unwrap_or_else(|| id.clone())
                })
                .collect();
            ui.add_space(6.0);
            ui.label(trf!("Not members here yet: {}", names.join(", ")));
            check(
                ui,
                &mut plan.create_missing,
                tr("Add them as members (otherwise their messages come in under your name)"),
            );
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(tr(
                "Messages keep their dates and thread replies stay in their threads. Files come in as links to Slack.",
            ))
            .small()
            .color(t.text3),
        );
        ui.add_space(8.0);
        let mut close = false;
        ui.horizontal(|ui| {
            if primary_button(ui, t, tr("Import")).clicked() {
                let (c, m) = self.import_slack(plan);
                plan.done = Some(trf!("Imported {} messages into {} channels.", m, c));
            }
            if ui.button(tr("Cancel")).clicked() {
                close = true;
            }
        });
        close
    }
}
