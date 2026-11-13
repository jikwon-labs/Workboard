// Working with other people without a server: a shared folder that a sync
// service keeps in step (see crate::sync), choosing who you are after
// joining, calls that open in the browser with a Join button posted in chat,
// and a Markdown archive for documents.

use super::*;
use crate::sync::{self, SharedCfg};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// How often teammates' copies are looked at, and how often ours is written.
const POLL_SECS: f64 = 2.0;
const WRITE_SECS: f64 = 1.5;

#[derive(Default)]
pub struct SyncRt {
    last_poll: f64,
    seen: Vec<(PathBuf, SystemTime)>,
    last_write: f64,
    pending: bool,
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
        sync::stamp(&v, &mut meta, now_ts());
        self.data.extra.sync = meta;
        self.sync_rt.pending = true;
    }

    pub(super) fn write_shared(&mut self) {
        let Some(cfg) = self.data.extra.shared.clone() else { return };
        let v = self.data_value();
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
        let files = sync::other_files(Path::new(&cfg.folder), &cfg.device);
        self.sync_rt.seen = files.clone();
        let others: Vec<serde_json::Value> = files.iter().filter_map(|(p, _)| sync::read_json(p)).collect();
        if others.is_empty() {
            return false;
        }
        let me = self.data.me;
        let count_new = |d: &Data| d.channels.iter().flat_map(|c| c.messages.iter()).map(|m| (m.author != me) as usize + m.replies.iter().filter(|r| r.author != me).count()).sum::<usize>();
        let before = count_new(&self.data);
        let mut v = self.data_value();
        let changed = sync::merge(&mut v, &others);
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

    /// Once per frame: write our copy when it changed, look for teammates' changes.
    pub(super) fn sync_tick(&mut self, ctx: &egui::Context) {
        if self.data.extra.shared.is_none() {
            return;
        }
        let now = ctx.input(|i| i.time);
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
        SharedCfg { folder: folder.to_string(), device: format!("{:08x}", sync::random_u64() as u32), tag }
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
        let mut d: Data = serde_json::from_value(base).map_err(|_| "The shared workspace couldn't be read.".to_string())?;
        // keep only the team's things; personal ones start empty
        d.favorites.clear();
        d.extra.notepad.clear();
        d.extra.reminders.clear();
        d.extra.trash.clear();
        d.extra.inbox_cleared.clear();
        d.extra.inbox_later.clear();
        d.extra.starred.clear();
        d.extra.shared = Some(Self::new_cfg(folder));
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
            self.ws_file = file;
        }
        self.data = d;
        self.sync_rt = SyncRt::default();
        self.sync_pull();
        self.data.extra.sync.hashes = sync::fingerprints(&self.data_value());
        self.loc = self.data.first_list().map(Location::List).unwrap_or(Location::Home);
        self.section = Section::Home;
        self.open_task = None;
        self.chat_open = None;
        self.recent.clear();
        self.save_local();
        Ok(())
    }

    pub(super) fn stop_sharing(&mut self) {
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
            Some(cfg) => Path::new(&cfg.folder).join("Docs archive"),
            None => {
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

    pub(super) fn together_body(&mut self, ui: &mut Ui, t: &Theme, path: &mut String, result: &mut String) -> bool {
        let mut close = false;
        if let Some(cfg) = self.data.extra.shared.clone() {
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
