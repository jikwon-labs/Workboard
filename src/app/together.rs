// Working with other people: either a shared folder that a sync service
// keeps in step, or a workboard-server the app stays connected to (see
// crate::sync and wb_net). Also choosing who you are after joining, calls
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

/// Connecting to a server for the first time from the dialog.
pub struct Joining {
    pub addr: String,
    pub device: String,
    /// The invite code typed in; empty means "set up this empty server".
    pub invite: String,
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
    /// Sign in again with this invite code (after access was removed).
    pub(super) reinvite: Option<String>,
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
        (false, _) => "Disconnected".into(),
        (true, None) | (true, Some(NetStatus::Connecting)) => "Connecting\u{2026}".into(),
        (true, Some(NetStatus::Connected)) => "Connected".into(),
        (true, Some(NetStatus::Reconnecting(why))) => format!("Reconnecting\u{2026} {}", why),
        (true, Some(NetStatus::Refused(why))) => format!("Not signed in. {}", why),
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

impl App {
    fn data_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.data).unwrap_or_default()
    }

    /// After a change: note what changed so teammates' copies merge correctly.
    pub(super) fn sync_note_change(&mut self) {
        if self.data.extra.shared.is_none() {
            return;
        }
        let v = self.data_value();
        let mut meta = std::mem::take(&mut self.data.extra.sync);
        sync::stamp(&v, &mut meta, chrono::Utc::now().timestamp_millis());
        self.data.extra.sync = meta;
        self.sync_rt.pending = true;
    }

    pub(super) fn write_shared(&mut self) {
        let Some(cfg) = self.data.extra.shared.clone() else { return };
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
            self.toast = Some(("Couldn't write to the shared folder. Is it still there?".into(), now_ts()));
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
        self.merge_in(&others)
    }

    /// Merge the team's copies into ours. Returns true when something new arrived.
    fn merge_in(&mut self, others: &[serde_json::Value]) -> bool {
        if others.is_empty() {
            return false;
        }
        let me = self.data.me;
        let count_new = |d: &Data| d.channels.iter().flat_map(|c| c.messages.iter()).map(|m| (m.author != me) as usize + m.replies.iter().filter(|r| r.author != me).count()).sum::<usize>();
        let before = count_new(&self.data);
        let mut v = self.data_value();
        let changed = sync::merge(&mut v, others);
        match serde_json::from_value::<Data>(v) {
            Ok(d) => self.data = d,
            Err(_) => return false,
        }
        if changed {
            let n = count_new(&self.data).saturating_sub(before);
            if n > 0 {
                self.toast = Some((if n == 1 { "1 new message from your team".into() } else { format!("{} new messages from your team", n) }, now_ts()));
            }
            self.save_local();
            self.sync_rt.pending = true;
        }
        changed
    }

    /// The server connection: start it when needed and take in what arrived.
    /// Returns true when the team's changes came in.
    pub(super) fn net_tick(&mut self, ctx: Option<&egui::Context>) -> bool {
        if self.sync_rt.net.is_none() {
            let (addr, sign_in) = if let Some(j) = &self.sync_rt.joining {
                let invite = Some(j.invite.trim().to_string()).filter(|c| !c.is_empty());
                // no code: set up an empty server as the person I am here
                let claim = if invite.is_none() { Some(self.data.me) } else { None };
                (j.addr.clone(), SignIn { device: j.device.clone(), invite, claim, ..Default::default() })
            } else if let Some(SharedCfg { server: Some(addr), device, tag, .. }) = &self.data.extra.shared {
                let invite = self.sync_rt.reinvite.clone();
                let token = if invite.is_some() { None } else { load_token(addr, device) };
                (addr.clone(), SignIn { device: device.clone(), tag: Some(*tag), token, invite, claim: None })
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
                NetEvent::Welcome { tag, snapshot, member, role, token, .. } => {
                    if let Some(t) = token {
                        let (addr, device) = match (&self.sync_rt.joining, &self.data.extra.shared) {
                            (Some(j), _) => (j.addr.clone(), j.device.clone()),
                            (None, Some(c)) => (c.server.clone().unwrap_or_default(), c.device.clone()),
                            _ => continue,
                        };
                        save_token(&addr, &device, &t);
                    }
                    self.sync_rt.reinvite = None;
                    self.sync_rt.role = Some(role);
                    changed |= self.net_welcome(tag, snapshot, member);
                }
                NetEvent::Update { data, .. } => changed |= self.merge_in(&[data]),
                NetEvent::Invite { member, code } => {
                    if let Some(p) = self.sync_rt.people.iter_mut().find(|p| p.member == member) {
                        p.invite = Some(code);
                    }
                }
                NetEvent::Access { people, role } => {
                    self.sync_rt.people = people;
                    self.sync_rt.role = Some(role);
                }
                NetEvent::Denied(why) => self.toast = Some((why, now_ts())),
            }
        }
        changed
    }

    /// The server answered: finish joining, or catch up after a reconnect.
    fn net_welcome(&mut self, tag: u16, snapshot: Option<serde_json::Value>, member: u64) -> bool {
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
                    self.toast = Some(("Connected. This workspace is now on the server for your team.".into(), now_ts()));
                    if matches!(self.modal, Some(Modal::Together { .. })) {
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
                        self.toast = Some((format!("Joined as {}.", name), now_ts()));
                        if matches!(self.modal, Some(Modal::Together { .. })) {
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
            Some(s) => self.merge_in(&[s]),
            None => false,
        };
        // after (re)connecting our whole copy goes up, with anything made offline
        self.write_shared();
        changed
    }

    /// Start connecting to a server from the dialog (shares this workspace
    /// there if the server is empty, else joins the team's workspace).
    pub(super) fn connect_server(&mut self, addr: &str, invite: &str) -> Result<(), String> {
        wb_net::server_url(addr)?;
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
        self.sync_rt.joining = Some(Joining { addr: addr.trim().to_string(), device: format!("{:08x}", sync::random_u64() as u32), invite: invite.trim().to_string() });
        Ok(())
    }

    /// Sign this computer in again with a new invite code.
    pub(super) fn sign_in_again(&mut self, code: &str) {
        if code.trim().is_empty() {
            return;
        }
        self.sync_rt.reinvite = Some(code.trim().to_string());
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
    }

    /// Send an access request to the server (owners and admins).
    pub(super) fn access_request(&mut self, msg: NetMsg) {
        match &self.sync_rt.net {
            Some(n) if self.sync_rt.net_status == Some(NetStatus::Connected) => n.send(msg),
            _ => self.toast = Some(("Not connected to the server right now.".into(), now_ts())),
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
            return Err("Choose a folder first.".into());
        }
        std::fs::create_dir_all(folder).map_err(|e| format!("Couldn't use that folder: {}", e))?;
        self.data.extra.shared = Some(Self::new_cfg(folder));
        self.data.extra.sync = Default::default();
        self.sync_note_change();
        self.write_shared();
        self.save_local();
        self.sync_rt = SyncRt::default();
        Ok(())
    }

    /// Join the workspace someone shared in a folder. It opens as a new
    /// workspace, so the one you had stays in the workspace menu.
    pub(super) fn join_shared(&mut self, folder: &str) -> Result<(), String> {
        let folder = folder.trim();
        let files = sync::other_files(Path::new(folder), "");
        let newest = files.iter().max_by_key(|(_, m)| *m).ok_or("No shared workspace in that folder yet.")?;
        let base = sync::read_json(&newest.0).ok_or("The shared workspace couldn't be read.")?;
        self.join_from(base, Self::new_cfg(folder))?;
        self.sync_rt = SyncRt::default();
        self.sync_pull();
        self.after_join();
        Ok(())
    }

    /// Open the team's copy as a new workspace on this computer.
    fn join_from(&mut self, base: serde_json::Value, cfg: SharedCfg) -> Result<(), String> {
        let mut d: Data = serde_json::from_value(base).map_err(|_| "The shared workspace couldn't be read.".to_string())?;
        // keep only the team's things; personal ones start empty
        d.favorites.clear();
        d.extra.notepad.clear();
        d.extra.reminders.clear();
        d.extra.trash.clear();
        d.extra.inbox_cleared.clear();
        d.extra.inbox_later.clear();
        d.extra.starred.clear();
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
        self.save_local();
    }

    pub(super) fn stop_sharing(&mut self) {
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
            if c.kind == ChannelKind::Channel && !c.members.contains(&id) {
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
            c.messages.push(Message { id, author: me, text: "Started a call".into(), ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: Some(link.clone()) });
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
        let text = format!("# {}\n\n_{} \u{2022} last updated {}_\n\n{}\n", d.title, author, when, d.body);
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
                Some(p) => (format!("Archived. A copy was saved in {}", p.parent().map(|p| p.display().to_string()).unwrap_or_default()), now_ts()),
                None => ("Archived. (The Markdown copy couldn't be saved.)".into(), now_ts()),
            });
        } else {
            self.toast = Some(("Moved back to Docs".into(), now_ts()));
        }
    }

    // ------------------------------------------------------------ dialogs

    /// Owners and admins: everyone's access, invite codes, roles.
    fn people_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new("People").strong());
        ui.label(RichText::new("Give each person their invite code (it works once, for 7 days). Add people first with Invite.").small().color(t.text3));
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
                    ui.label(RichText::new(p.role.label()).small().color(t.text2));
                    ui.label(RichText::new(match p.devices { 0 => "not signed in".to_string(), 1 => "1 computer".to_string(), n => format!("{} computers", n) }).small().color(t.text3));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if p.role != Role::Owner && p.member != me {
                            if ui.small_button(RichText::new("Remove access").color(t.danger)).on_hover_text(format!("Sign out {}'s computers", name)).clicked() {
                                req = Some(NetMsg::Revoke { member: p.member });
                            }
                            if my_role.manages() {
                                let (label, role) = if p.role == Role::Admin { ("Make member", Role::Member) } else { ("Make admin", Role::Admin) };
                                if ui.small_button(label).clicked() {
                                    req = Some(NetMsg::SetRole { member: p.member, role });
                                }
                            }
                        }
                        // a code for the owner signs in as the owner: only the owner makes one
                        let may_invite = p.member != me && (p.role != Role::Owner || my_role == Role::Owner);
                        if may_invite && ui.small_button(if p.invite.is_some() { "New code" } else { "Invite code" }).clicked() {
                            req = Some(NetMsg::CreateInvite { member: p.member, role: Role::Member });
                        }
                    });
                });
                if let Some(code) = &p.invite {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new(format!("Code for {}: {}", name, code)).monospace().color(t.text));
                        if ui.small_button("Copy").clicked() {
                            ui.ctx().copy_text(code.clone());
                            self.toast = Some(("Invite code copied".into(), now_ts()));
                        }
                    });
                }
            }
        });
        if let Some(m) = req {
            self.access_request(m);
        }
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
            ui.label(RichText::new("This workspace is on a server").size(15.0).strong());
            ui.label(RichText::new(&server).color(t.text2));
            let connected = self.sync_rt.net_status == Some(NetStatus::Connected);
            let status = net_status_text(self.sync_rt.net.is_some(), self.sync_rt.net_status.as_ref());
            ui.label(RichText::new(format!("Status: {}", status)).color(if connected { t.text } else { t.text2 }));
            if let Some(role) = self.sync_rt.role.filter(|_| connected) {
                let name = self.data.member(self.data.me).map(|m| m.name.clone()).unwrap_or_default();
                ui.label(RichText::new(format!("Signed in as {} ({})", name, role.label())).color(t.text2));
            }
            if matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
                ui.add_space(6.0);
                ui.label(RichText::new("Sign this computer in again with an invite code from the owner or an admin.").color(t.text2));
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(code).hint_text("XXXX-XXXX-XXXX").desired_width(220.0));
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Invite code entry"));
                    if primary_button(ui, t, "Sign in").clicked() {
                        self.sign_in_again(code);
                        code.clear();
                    }
                });
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Tasks, chat and docs show up for everyone within a second or two. While the server can't be reached you keep working here, and your changes go up when it's back.").small().color(t.text3));
            if connected && self.sync_rt.role.is_some_and(|r| r.manages()) {
                ui.add_space(10.0);
                self.people_section(ui, t);
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Reconnect now").clicked() {
                    self.sync_rt.net = None;
                    self.sync_rt.net_status = None;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("Stop sharing").color(t.danger)).on_hover_text("Keep working on this computer only; your copy stays").clicked() {
                        self.stop_sharing();
                        close = true;
                    }
                });
            });
        } else if let Some(cfg) = self.data.extra.shared.clone() {
            ui.label(RichText::new("This workspace is shared").size(15.0).strong());
            ui.label(RichText::new(&cfg.folder).color(t.text2));
            let n = sync::other_files(Path::new(&cfg.folder), &cfg.device).len();
            ui.label(RichText::new(match n {
                0 => "Nobody else has joined yet. Share this folder with your teammates in your sync app, then they choose Work together > Join.".to_string(),
                1 => "1 other computer is working in it.".to_string(),
                n => format!("{} other computers are working in it.", n),
            }).color(t.text2));
            ui.add_space(8.0);
            ui.label(RichText::new("Tasks, chat, docs and calls show up for everyone within a few seconds of their sync app copying the folder.").small().color(t.text3));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Check for changes now").clicked() {
                    let changed = self.sync_pull();
                    *result = if changed { "Got the latest changes.".into() } else { "Everything is up to date.".into() };
                }
                if ui.button("Show folder").clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("file://{}", cfg.folder)));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("Stop sharing").color(t.danger)).on_hover_text("Keep working on this computer only; your copy stays").clicked() {
                        self.stop_sharing();
                        close = true;
                    }
                });
            });
        } else {
            ui.label("Work on this workspace with other people, without a server.");
            ui.label(RichText::new("Pick a folder that a sync app shares with your teammates (iCloud Drive, Dropbox, Google Drive, OneDrive or Syncthing). Everyone opens Workboard on their own computer and points it at the same folder.").color(t.text2));
            ui.add_space(8.0);
            let sugg = sync::suggested_folders();
            if !sugg.is_empty() {
                ui.label(RichText::new("Found on this computer").small().color(t.text3));
                ui.horizontal_wrapped(|ui| {
                    for (name, p) in &sugg {
                        if ui.button(name).on_hover_text(p).clicked() {
                            *path = p.clone();
                        }
                    }
                });
            }
            ui.label(RichText::new("Folder").color(t.text2));
            ui.add(egui::TextEdit::singleline(path).hint_text("Paste a folder path, or pick one above").desired_width(f32::INFINITY));
            let n = folder_people(path);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if n > 0 {
                    ui.label(RichText::new(format!("A shared workspace is already here ({} {}).", n, if n == 1 { "person" } else { "people" })).color(t.text2));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, t, "Join this workspace").clicked() {
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
                        if primary_button(ui, t, "Share this workspace here").clicked() {
                            match self.share_here(path) {
                                Ok(()) => {
                                    self.toast = Some(("Shared. Now share the folder with your teammates in your sync app.".into(), now_ts()));
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
            ui.label(RichText::new("Or connect to your team's server").strong());
            ui.label(RichText::new("A Workboard server (for example a Mac mini your team reaches through Tailscale). To join your team, type the invite code you were given. Setting up a new, empty server? Leave the code empty: this workspace goes up and you become its owner.").color(t.text2));
            ui.label(RichText::new("Server").color(t.text2));
            let r = ui.add(egui::TextEdit::singleline(addr).hint_text("macmini or 100.101.102.103").desired_width(f32::INFINITY));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Server address"));
            ui.label(RichText::new("Invite code").color(t.text2));
            let r = ui.add(egui::TextEdit::singleline(code).hint_text("XXXX-XXXX-XXXX (empty for a new server)").desired_width(f32::INFINITY));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Invite code entry"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if self.sync_rt.joining.is_some() {
                    let status = net_status_text(self.sync_rt.net.is_some(), self.sync_rt.net_status.as_ref());
                    ui.label(RichText::new(status).color(t.text2));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.sync_rt.joining.is_some() {
                        if ui.button("Cancel").clicked() {
                            self.sync_rt.joining = None;
                            self.sync_rt.net = None;
                            self.sync_rt.net_status = None;
                        }
                    } else if primary_button(ui, t, "Connect").clicked() {
                        result.clear();
                        if let Err(e) = self.connect_server(addr, code) {
                            *result = e;
                        }
                    }
                });
            });
        }
        if !result.is_empty() {
            ui.label(RichText::new(result.as_str()).color(t.text2));
        }
        close
    }

    pub(super) fn who_am_i_body(&mut self, ui: &mut Ui, t: &Theme, name: &mut String, email: &mut String) -> bool {
        let mut close = false;
        ui.label(RichText::new("Pick yourself so your messages and tasks show your name.").color(t.text2));
        ui.add_space(6.0);
        let members: Vec<Member> = self.data.members.clone();
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for m in &members {
                let label = if m.invited { format!("{}   {}  (invited)", m.name, m.email) } else { format!("{}   {}", m.name, m.email) };
                if ui.add(egui::Button::new(label).frame(false)).clicked() {
                    self.become_member(Some(m.id), "", "");
                    close = true;
                }
            }
        });
        ui.separator();
        ui.label(RichText::new("I'm new here").color(t.text2));
        ui.add(egui::TextEdit::singleline(name).hint_text("Your name").desired_width(f32::INFINITY));
        ui.add(egui::TextEdit::singleline(email).hint_text("Email").desired_width(f32::INFINITY));
        ui.add_space(6.0);
        if primary_button(ui, t, "Join as new member").clicked() && !name.trim().is_empty() {
            self.become_member(None, name, email);
            close = true;
        }
        close
    }

    pub(super) fn call_body(&mut self, ui: &mut Ui, t: &Theme, ch: &mut u64, link: &mut String) -> bool {
        let mut close = false;
        ui.label(RichText::new("The call opens in your browser (Jitsi Meet: free, no account). Everyone in the conversation gets a Join button.").color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new("Call in").color(t.text2));
        let chans: Vec<(u64, String)> = self.data.channels.iter().map(|c| (c.id, if c.kind == ChannelKind::Channel { format!("# {}", c.name) } else { self.channel_label(c) })).collect();
        let cur = chans.iter().find(|c| c.0 == *ch).map(|c| c.1.clone()).unwrap_or_else(|| "Choose".into());
        egui::ComboBox::from_id_salt("call_in").selected_text(cur).width(300.0).show_ui(ui, |ui| {
            for (id, n) in &chans {
                ui.selectable_value(ch, *id, n);
            }
        });
        ui.add_space(6.0);
        ui.label(RichText::new("Or use your own meeting link (Zoom, Google Meet...)").small().color(t.text3));
        ui.add(egui::TextEdit::singleline(link).hint_text("https://...").desired_width(f32::INFINITY));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, t, "Start call").clicked() && chans.iter().any(|c| c.0 == *ch) {
                    let ctx = ui.ctx().clone();
                    self.start_call(Some(&ctx), *ch, link);
                    close = true;
                }
                if ui.button("Cancel").clicked() {
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
        self.modal = Some(Modal::Call { ch, link: String::new() });
    }
}
