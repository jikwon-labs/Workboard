// "Import from Slack": a Slack workspace export (.zip, from Slack's
// Settings > Import/Export Data) becomes channels and messages here.
//
// The export holds users.json (the people), channels.json (public
// channels; groups.json holds private ones when the export has them) and a
// folder per channel with one JSON file per day: [{"user", "text", "ts",
// "thread_ts", "files", ...}]. People are matched to members first by the
// Slack id an earlier import remembered on the member (`Member::slack`, so a
// renamed member or a teammate's import still finds them), then by email,
// then by a name only one member has; two Slack people are never matched to
// the same member on their own. The dialog shows the matching and each person
// can be pointed at a member (listed with their email, so two of one name
// differ), added as a new member, or brought in under my name (their name in
// front of their messages). A Slack person is remembered on one member at
// most: pointing them at another member (or making a new member for them)
// moves them there for everyone. "Under my name" is my own choice, kept in
// my personal data (`Extra::slack_under_me`, never shared): my next import
// starts from it, it leaves the shared match alone (a teammate's import
// still goes to that member) and teammates' imports never change it. A new
// member is made only when a message of theirs actually comes in. Messages keep their times,
// thread replies go under their message, files come in as links (the files
// themselves need a Slack sign-in to fetch). Imported messages are marked
// (`Message::imported`, with Slack's unique ts) so they aren't news for
// anyone, and importing the same export again adds nothing twice.
//
// The zip is opened with macOS's own `ditto`.

use super::*;
use crate::media;
use std::collections::BTreeMap;
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

/// Where one Slack person's messages go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Assign {
    /// a member here
    Member(u64),
    /// a new member made for them
    New,
    /// under my name, with theirs in front
    Me,
}

/// An export read and matched against this workspace, waiting for Import.
#[derive(Clone, Debug, Default)]
pub(super) struct SlackPlan {
    pub users: Vec<SlackUser>,
    pub channels: Vec<SlackChannel>,
    /// Slack id -> where their messages go, for everyone in `people`
    pub assign: HashMap<String, Assign>,
    /// the people the dialog lists, in its order: everyone who wrote
    /// something, then channel members matched to a member here
    pub people: Vec<String>,
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

    /// The name to show for a Slack id.
    fn shown(&self, id: &str) -> String {
        self.user(id)
            .map(|u| u.shown())
            .unwrap_or_else(|| id.to_string())
    }

    /// The member a Slack person is matched to, if any.
    pub fn matched(&self, id: &str) -> Option<u64> {
        match self.assign.get(id) {
            Some(Assign::Member(m)) => Some(*m),
            _ => None,
        }
    }

    /// People not matched to a member (Slack ids, in the dialog's order).
    pub fn unmatched(&self) -> Vec<String> {
        self.people
            .iter()
            .filter(|id| self.matched(id).is_none())
            .cloned()
            .collect()
    }

    /// Members more than one Slack person is pointed at, with those people.
    pub fn shared_members(&self) -> Vec<(u64, Vec<String>)> {
        let mut by: BTreeMap<u64, Vec<String>> = BTreeMap::new();
        for id in &self.people {
            if let Some(m) = self.matched(id) {
                by.entry(m).or_default().push(id.clone());
            }
        }
        by.into_iter().filter(|(_, v)| v.len() > 1).collect()
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

/// Match the people in an export to members. People I chose to bring in
/// under my name at an earlier import (`Extra::slack_under_me`, my own
/// choice on this computer) are again. Everyone else goes first to the
/// member an earlier import brought them in as (`Member::slack`, kept in the
/// shared workspace so a teammate's import finds it too, after a rename,
/// with no email, or for an author the export's users.json leaves out), then
/// by email, then by a name only one member has. A member two Slack people
/// would both match by email or name is left unmatched for both (the dialog
/// lets someone choose), so nobody's messages end up under somebody else
/// without anyone deciding so. Unmatched people who wrote something are
/// added as new members unless changed.
pub(super) fn plan(data: &Data, users: Vec<SlackUser>, channels: Vec<SlackChannel>) -> SlackPlan {
    let low = |x: &str| x.trim().to_lowercase();
    // under my name, as I chose at an earlier import
    let mine = data.slack_under_me();
    let mut people: Vec<String> = vec![];
    for c in &channels {
        for m in &c.messages {
            if !m.user.is_empty() && !people.contains(&m.user) {
                people.push(m.user.clone());
            }
        }
    }
    // decided at an earlier import (also for authors missing from users.json)
    let mut known: HashMap<String, u64> = HashMap::new();
    let ids = users.iter().map(|u| &u.id).chain(people.iter());
    for id in ids.filter(|id| !mine.contains(id)) {
        if let Some(m) = data.members.iter().find(|m| m.slack.contains(id)) {
            known.insert(id.clone(), m.id);
        }
    }
    let taken = |m: u64| known.values().any(|x| *x == m);
    // Slack id -> a member, each way
    let mut by_mail: HashMap<String, u64> = HashMap::new();
    let mut by_name: HashMap<String, u64> = HashMap::new();
    for u in users
        .iter()
        .filter(|u| !known.contains_key(&u.id) && !mine.contains(&u.id))
    {
        if !u.email.trim().is_empty() {
            if let Some(m) = data.members.iter().find(|m| low(&m.email) == low(&u.email)) {
                by_mail.insert(u.id.clone(), m.id);
                continue;
            }
        }
        let names: Vec<String> = [&u.real_name, &u.display, &u.handle]
            .iter()
            .filter(|n| !n.trim().is_empty())
            .map(|n| low(n))
            .collect();
        let found: Vec<u64> = data
            .members
            .iter()
            .filter(|m| names.contains(&low(&m.name)))
            .map(|m| m.id)
            .collect();
        // two members with that name: can't tell which
        if let [m] = found[..] {
            by_name.insert(u.id.clone(), m);
        }
    }
    let count = |map: &HashMap<String, u64>, m: u64| map.values().filter(|x| **x == m).count();
    let mut matched: HashMap<String, u64> = known.clone();
    for (id, m) in &by_mail {
        // not a member an earlier import gave to another Slack person
        if count(&by_mail, *m) == 1 && !taken(*m) {
            matched.insert(id.clone(), *m);
        }
    }
    for (id, m) in &by_name {
        // not a member someone else matched by email, and nobody else by name
        if count(&by_mail, *m) == 0 && count(&by_name, *m) == 1 && !taken(*m) {
            matched.insert(id.clone(), *m);
        }
    }
    let mut quiet: Vec<String> = channels
        .iter()
        .flat_map(|c| c.members.iter())
        .filter(|u| matched.contains_key(*u) && !people.contains(u))
        .cloned()
        .collect();
    quiet.sort();
    quiet.dedup();
    people.extend(quiet);
    let assign = people
        .iter()
        .map(|id| {
            let a = if mine.contains(id) {
                Assign::Me
            } else {
                match matched.get(id) {
                    Some(m) => Assign::Member(*m),
                    None => Assign::New,
                }
            };
            (id.clone(), a)
        })
        .collect();
    SlackPlan {
        users,
        channels,
        assign,
        people,
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
        imported: None,
    }
}

/// Each member as the matching dialog lists them: the name, with the email
/// next to it so two members of one name can be told apart; a member of a
/// name others share and no email gets their role, and if that is still the
/// same, the end of their id.
pub(super) fn member_choices(members: &[Member]) -> Vec<(u64, String)> {
    let low = |x: &str| x.trim().to_lowercase();
    let shared = |m: &Member| {
        members
            .iter()
            .filter(|x| low(&x.name) == low(&m.name))
            .count()
            > 1
    };
    let first: Vec<(u64, String)> = members
        .iter()
        .map(|m| {
            let name = m.name.trim();
            let label = if !m.email.trim().is_empty() {
                format!("{} ({})", name, m.email.trim())
            } else if shared(m) && !m.role.trim().is_empty() {
                format!("{} ({})", name, tr(m.role.trim()))
            } else {
                name.to_string()
            };
            (m.id, label)
        })
        .collect();
    first
        .iter()
        .map(|(id, label)| {
            if first.iter().filter(|(_, l)| low(l) == low(label)).count() > 1 {
                (*id, format!("{} #{:04}", label, id % 10_000))
            } else {
                (*id, label.clone())
            }
        })
        .collect()
}

/// The name a channel from Slack gets here, and the one it gets when a
/// channel of the other kind (public or private) already has that name.
fn channel_names(sc: &SlackChannel) -> (String, String) {
    let name = sc.name.trim().to_lowercase().replace(' ', "-");
    let alt = format!("{}-{}", name, if sc.private { "private" } else { "public" });
    (name, alt)
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
        self.import_slack_report(plan).0
    }

    /// `import_slack`, also naming the private channels made with only me
    /// in them (none of their Slack members is a member here).
    pub(super) fn import_slack_report(
        &mut self,
        plan: &SlackPlan,
    ) -> ((usize, usize), Vec<String>) {
        let me = self.data.me;
        // choices an earlier version kept on my shared member
        if self.data.keep_slack_under_me_personal() {
            self.dirty = true;
        }
        // Slack id -> member; people under my name aren't in it, and people
        // who get a new member only once a message of theirs comes in (an
        // import that adds nothing makes no one)
        let mut people: HashMap<String, u64> = HashMap::new();
        let mut to_make: Vec<String> = vec![];
        for id in &plan.people {
            match plan.assign.get(id).copied().unwrap_or(Assign::New) {
                Assign::Member(m) if self.data.member(m).is_some() => {
                    people.insert(id.clone(), m);
                }
                Assign::Member(_) | Assign::Me => {}
                Assign::New => {
                    if plan.user(id).is_some() {
                        to_make.push(id.clone());
                    }
                }
            }
        }
        // names for @mentions: the member's name here, else Slack's (which
        // is also the name a new member gets)
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
        let mut alone = vec![];
        for sc in &plan.channels {
            let (name, alt) = channel_names(sc);
            // the same kind of channel only: a private Slack channel never
            // pours into a public one of the same name, or the other way
            let existing = self.data.channels.iter().position(|c| {
                c.kind == ChannelKind::Channel
                    && c.private == sc.private
                    && (c.name == name || c.name == alt)
            });
            let ci = match existing {
                Some(i) => i,
                None => {
                    let id = self.data.new_id();
                    let mut members: Vec<u64> = sc
                        .members
                        .iter()
                        .filter_map(|u| people.get(u).copied())
                        .collect();
                    members.sort();
                    members.dedup();
                    if members.is_empty() {
                        if sc.private {
                            // nobody to share it with: only me, not everyone
                            alone.push((name.clone(), id));
                        } else {
                            members = self.data.members.iter().map(|m| m.id).collect();
                        }
                    }
                    if !members.contains(&me) {
                        members.push(me);
                    }
                    let taken = self
                        .data
                        .channels
                        .iter()
                        .any(|c| c.kind == ChannelKind::Channel && c.name == name);
                    self.data.channels.push(Channel {
                        id,
                        name: if taken { alt.clone() } else { name.clone() },
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
                // a new member still to make: the message counts as theirs
                let unmade = !people.contains_key(&m.user) && to_make.contains(&m.user);
                let (author, prefix) = match people.get(&m.user) {
                    Some(id) => (*id, String::new()),
                    None if unmade => (0, String::new()),
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
                // Slack's ts is unique within a channel
                let key = format!("slack:{}:{}", sc.name, m.ts);
                let ch = &self.data.channels[ci];
                let same = |x: &Message| match &x.imported {
                    Some(k) => *k == key,
                    // imported before messages kept where they came from
                    None => x.ts == ts && (unmade || x.author == author) && x.text == text,
                };
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
                let author = if unmade {
                    let Some(u) = plan.user(&m.user) else {
                        continue;
                    };
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
                        slack: vec![],
                        slack_mine: vec![],
                    });
                    people.insert(m.user.clone(), mid);
                    if existing.is_none() {
                        // a channel made by this import: they're in it
                        let ch = &mut self.data.channels[ci];
                        if !ch.members.contains(&mid) {
                            ch.members.push(mid);
                        }
                    }
                    mid
                } else {
                    author
                };
                let id = self.data.new_id();
                let mut msg = new_message(id, author, text, ts);
                msg.imported = Some(key);
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
        // remember where each person went, for the next import. The shared
        // match (`Member::slack`, here and on teammates' computers) moves
        // only to a member they now are, chosen or made for them, and is
        // taken off any other then. "Under my name" is only my choice: kept
        // in my personal data, it leaves the shared match alone, as does a
        // person nobody was made for (nothing of theirs came in, or they're
        // missing from users.json).
        for sid in &plan.people {
            let target = people.get(sid).copied();
            let a = plan.assign.get(sid).copied().unwrap_or(Assign::New);
            // a member no longer here: nothing was decided
            if matches!(a, Assign::Member(m) if target.is_none() && self.data.member(m).is_none()) {
                continue;
            }
            let under_me = a == Assign::Me;
            let mine = &mut self.data.extra.slack_under_me;
            if under_me != mine.contains(sid) {
                if under_me {
                    mine.push(sid.clone());
                } else {
                    mine.retain(|x| x != sid);
                }
                self.dirty = true;
            }
            let Some(target) = target else {
                continue;
            };
            for mem in self.data.members.iter_mut() {
                let theirs = mem.id == target;
                if theirs != mem.slack.contains(sid) {
                    if theirs {
                        mem.slack.push(sid.clone());
                    } else {
                        mem.slack.retain(|x| x != sid);
                    }
                    self.dirty = true;
                }
            }
        }
        // people who came in with their messages joined some of them
        let alone: Vec<String> = alone
            .into_iter()
            .filter(|(_, id)| {
                self.data
                    .channels
                    .iter()
                    .find(|c| c.id == *id)
                    .is_some_and(|c| c.members.iter().all(|m| *m == me))
            })
            .map(|(n, _)| n)
            .collect();
        if count == 0 && chans == 0 {
            return ((0, 0), alone);
        }
        // old messages coming in aren't news
        for id in touched {
            self.mark_read(id);
        }
        self.notify_rt.seeded = false;
        self.dirty = true;
        ((chans, count), alone)
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
                plan.people
                    .iter()
                    .filter(|p| plan.matched(p).is_some())
                    .count()
            ))
            .color(t.text2),
        );
        if !plan.people.is_empty() {
            ui.add_space(6.0);
            ui.label(tr("Whose messages go where:"));
            let mut members = member_choices(&self.data.members);
            members.sort_by_key(|m| m.1.to_lowercase());
            egui::ScrollArea::vertical()
                .id_salt("slack_people")
                .max_height(220.0)
                .show(ui, |ui| {
                    egui::Grid::new("slack_people_grid")
                        .num_columns(2)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            for id in plan.people.clone() {
                                let u = plan.user(&id).cloned().unwrap_or_default();
                                let mut who = plan.shown(&id);
                                // the email, else the Slack handle, tells
                                // two people of one name apart
                                if !u.email.trim().is_empty() {
                                    who = format!("{} ({})", who, u.email.trim());
                                } else if !u.handle.trim().is_empty() {
                                    who = format!("{} (@{})", who, u.handle.trim());
                                }
                                ui.label(who);
                                let cur = plan.assign.get(&id).copied().unwrap_or(Assign::New);
                                let label = |a: Assign| -> String {
                                    match a {
                                        Assign::New => tr("New member").into(),
                                        Assign::Me => tr("Import under my name").into(),
                                        Assign::Member(m) => members
                                            .iter()
                                            .find(|x| x.0 == m)
                                            .map(|x| x.1.clone())
                                            .unwrap_or_default(),
                                    }
                                };
                                let mut pick = cur;
                                let a11y = format!("Messages from {}", plan.shown(&id));
                                let combo =
                                    egui::ComboBox::from_id_salt(("slack_person", id.as_str()))
                                        .selected_text(label(cur))
                                        .width(200.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut pick,
                                                Assign::New,
                                                label(Assign::New),
                                            );
                                            ui.selectable_value(
                                                &mut pick,
                                                Assign::Me,
                                                label(Assign::Me),
                                            );
                                            ui.separator();
                                            for (mid, name) in &members {
                                                ui.selectable_value(
                                                    &mut pick,
                                                    Assign::Member(*mid),
                                                    name.as_str(),
                                                );
                                            }
                                        });
                                combo.response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::ComboBox,
                                        true,
                                        &a11y,
                                    )
                                });
                                if pick != cur {
                                    plan.assign.insert(id.clone(), pick);
                                }
                                ui.end_row();
                            }
                        });
                });
            for (m, ids) in plan.shared_members() {
                let who: Vec<String> = ids.iter().map(|id| plan.shown(id)).collect();
                let name = self
                    .data
                    .member(m)
                    .map(|x| x.name.clone())
                    .unwrap_or_default();
                ui.label(
                    RichText::new(trf!(
                        "{} all go to {}: their messages will look like one person's.",
                        who.join(", "),
                        name
                    ))
                    .color(t.text2),
                );
            }
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(tr(
                "Messages keep their dates and thread replies stay in their threads. Files come in as links to Slack. Old mentions and replies don't show up as new in anyone's Inbox.",
            ))
            .small()
            .color(t.text3),
        );
        ui.add_space(8.0);
        let mut close = false;
        ui.horizontal(|ui| {
            if primary_button(ui, t, tr("Import")).clicked() {
                let ((c, m), alone) = self.import_slack_report(plan);
                let mut done = trf!("Imported {} messages into {} channels.", m, c);
                if !alone.is_empty() {
                    let names: Vec<String> = alone.iter().map(|n| format!("#{}", n)).collect();
                    done.push('\n');
                    done.push_str(&trf!(
                        "No one in these private channels is a member here, so only you are in them for now: {}",
                        names.join(", ")
                    ));
                }
                plan.done = Some(done);
            }
            if ui.button(tr("Cancel")).clicked() {
                close = true;
            }
        });
        close
    }
}
