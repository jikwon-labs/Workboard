// Pictures and videos in chat: getting them ready, moving them to and from
// the server, and turning them into textures.
//
// Every file sits in a cache folder named by its sha256 (the attachment's
// `file`). Sending: the file is checked (kind, size, picture/video size),
// hashed and copied into the cache on a worker thread, then put on the server
// (or in the shared folder's files/), and only then can the message go.
// Showing: a message's file comes from the cache, else the server, else the
// shared folder; pictures for the screen are decoded on a worker thread.
//
// Two threads: "workboard-files" moves files (one at a time, so a big video
// doesn't stall behind another), "workboard-media" prepares and decodes.

use super::*;
use crate::media::{self, Rgba};
use std::cell::RefCell;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Longer side of a picture in the chat.
pub(super) const THUMB: u32 = 720;
/// Longer side of a picture opened large.
pub(super) const FULL: u32 = 2560;
/// The server takes files up to this size.
pub(super) const MAX_FILE: u64 = 500 << 20;
/// Pictures and videos in one message.
pub(super) const MAX_PER_MESSAGE: usize = 10;

/// An attachment waiting in the message box.
#[derive(Clone, Debug)]
pub(super) struct Pending {
    pub local: u64,
    pub name: String,
    /// None while it's being checked and copied.
    pub att: Option<Attachment>,
    pub error: Option<String>,
}

impl Pending {
    /// Ready to go out: checked, and on the server when there is one.
    pub fn ready(&self, files: &Files) -> bool {
        self.error.is_none() && self.att.as_ref().is_some_and(|a| files.state(&a.file) == Some(Transfer::Done))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Transfer {
    /// sending, 0..1
    Up(f32),
    /// fetching, 0..1
    Down(f32),
    Failed(String),
    /// here (and on the server, when sending)
    Done,
}

/// Where files go besides the cache.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Remote {
    /// only this computer
    None,
    /// a shared folder: files go in <folder>/files
    Folder(PathBuf),
    /// the team server
    Server { addr: String, auth: wb_net::Auth },
}

enum Job {
    Prepare { local: u64, path: Option<PathBuf>, png: Option<Vec<u8>>, name: String, folder: Option<PathBuf> },
    Upload { sha: String, ctype: String, addr: String, auth: wb_net::Auth },
    Download { sha: String, addr: String, auth: wb_net::Auth },
    FromFolder { sha: String, folder: PathBuf },
    Decode { key: String, sha: String, ext: String, kind: MediaKind, max: u32 },
}

enum Done {
    Prepared { local: u64, result: Result<Attachment, String> },
    Decoded { key: String, img: Option<Rgba> },
}

pub(super) struct Files {
    dir: PathBuf,
    net: Sender<Job>,
    work: Sender<Job>,
    done: Receiver<Done>,
    states: Arc<Mutex<HashMap<String, Transfer>>>,
    /// files a message on screen wants (asked while drawing)
    wants: RefCell<Vec<(String, String, MediaKind, u32)>>,
    asked: HashSet<String>,
    textures: HashMap<String, Option<egui::TextureHandle>>,
    /// decoded keys that came out as a picture
    ok: HashSet<String>,
    /// prepared attachments, by the Pending they belong to
    prepared: Vec<(u64, Result<Attachment, String>)>,
    next_local: u64,
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl Files {
    pub fn new(dir: PathBuf) -> Files {
        let (net, net_rx) = mpsc::channel::<Job>();
        let (work, work_rx) = mpsc::channel::<Job>();
        let (done_tx, done) = mpsc::channel::<Done>();
        let states: Arc<Mutex<HashMap<String, Transfer>>> = Default::default();
        let ctx: Arc<Mutex<Option<egui::Context>>> = Default::default();
        let wake = {
            let ctx = ctx.clone();
            move || {
                if let Some(c) = ctx.lock().unwrap().as_ref() {
                    c.request_repaint();
                }
            }
        };
        {
            let (dir, states, wake) = (dir.clone(), states.clone(), wake.clone());
            let _ = std::thread::Builder::new().name("workboard-files".into()).spawn(move || {
                for job in net_rx {
                    move_file(&dir, job, &states, &wake);
                }
            });
        }
        {
            let (dir, done_tx) = (dir.clone(), done_tx);
            let _ = std::thread::Builder::new().name("workboard-media".into()).spawn(move || {
                for job in work_rx {
                    let out = match job {
                        Job::Prepare { local, path, png, name, folder } => Done::Prepared { local, result: prepare(&dir, path.as_deref(), png, &name, folder.as_deref()) },
                        Job::Decode { key, sha, ext, kind, max } => Done::Decoded { key, img: media::thumbnail(&with_ext(&dir, &sha, &ext), kind, max) },
                        _ => continue,
                    };
                    if done_tx.send(out).is_err() {
                        return;
                    }
                    wake();
                }
            });
        }
        Files { dir, net, work, done, states, wants: Default::default(), asked: Default::default(), textures: Default::default(), ok: Default::default(), prepared: vec![], next_local: 1, ctx }
    }

    /// The file in the cache, when it's there.
    pub fn local(&self, sha: &str) -> Option<PathBuf> {
        let p = self.dir.join(sha);
        (wb_server_name_ok(sha) && p.is_file()).then_some(p)
    }

    pub fn state(&self, sha: &str) -> Option<Transfer> {
        self.states.lock().unwrap().get(sha).cloned()
    }

    /// Start checking and copying a file (or pasted PNG) for the message box.
    pub fn add(&mut self, path: Option<PathBuf>, png: Option<Vec<u8>>, remote: &Remote) -> Pending {
        let local = self.next_local;
        self.next_local += 1;
        let name = path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Pasted image.png".into());
        let folder = match remote {
            Remote::Folder(f) => Some(f.join("files")),
            _ => None,
        };
        let _ = self.work.send(Job::Prepare { local, path, png, name: name.clone(), folder });
        Pending { local, name, att: None, error: None }
    }

    /// A file is in the cache: send it to the server, or (without one) it's
    /// ready as it is.
    pub fn upload(&self, att: &Attachment, remote: &Remote) {
        let Remote::Server { addr, auth } = remote else {
            self.states.lock().unwrap().insert(att.file.clone(), Transfer::Done);
            return;
        };
        {
            let mut s = self.states.lock().unwrap();
            if matches!(s.get(&att.file), Some(Transfer::Up(_))) {
                return;
            }
            s.insert(att.file.clone(), Transfer::Up(0.0));
            drop(s);
            let ctype = media::content_type(Path::new(&att.name)).to_string();
            let _ = self.net.send(Job::Upload { sha: att.file.clone(), ctype, addr: addr.clone(), auth: auth.clone() });
        }
    }

    /// While drawing: this attachment is on screen and should be shown at `max`.
    pub fn want(&self, a: &Attachment, max: u32) {
        self.wants.borrow_mut().push((a.file.clone(), ext_of(&a.name), a.kind, max));
    }

    /// The cached file under a name with the attachment's extension, for
    /// AVFoundation (which goes by the extension, not the contents).
    pub fn playable(&self, a: &Attachment) -> Option<PathBuf> {
        self.local(&a.file)?;
        Some(with_ext(&self.dir, &a.file, &ext_of(&a.name)))
    }

    pub fn texture(&self, sha: &str, max: u32) -> Option<&egui::TextureHandle> {
        self.textures.get(&key(sha, max)).and_then(|t| t.as_ref())
    }

    /// A picture that couldn't be decoded.
    pub fn broken(&self, sha: &str, max: u32) -> bool {
        let k = key(sha, max);
        self.textures.contains_key(&k) && !self.ok.contains(&k)
    }

    /// Try a failed transfer again.
    pub fn retry(&mut self, sha: &str) {
        self.states.lock().unwrap().remove(sha);
        self.asked.retain(|k| !k.starts_with(sha));
    }

    /// Once a frame: collect finished work, start what's wanted.
    pub fn tick(&mut self, ctx: Option<&egui::Context>, remote: &Remote) {
        if let Some(c) = ctx {
            let mut g = self.ctx.lock().unwrap();
            if g.is_none() {
                *g = Some(c.clone());
            }
        }
        for d in self.done.try_iter().collect::<Vec<_>>() {
            match d {
                Done::Prepared { local, result } => self.prepared.push((local, result)),
                Done::Decoded { key, img } => {
                    if img.is_some() {
                        self.ok.insert(key.clone());
                    } else {
                        log_broken(&key);
                    }
                    // tests have no context: only remember how it went
                    let tex = img.zip(ctx).map(|(img, c)| c.load_texture(format!("media-{}", key), img.to_color_image(), egui::TextureOptions::LINEAR));
                    self.textures.insert(key, tex);
                }
            }
        }
        for (sha, ext, kind, max) in self.wants.take() {
            let k = key(&sha, max);
            if self.textures.contains_key(&k) || self.asked.contains(&k) {
                continue;
            }
            if self.local(&sha).is_some() {
                self.asked.insert(k.clone());
                let _ = self.work.send(Job::Decode { key: k, sha, ext, kind, max });
                continue;
            }
            // not here yet: fetch it once
            let fetch = format!("{}:fetch", sha);
            if self.asked.contains(&fetch) {
                continue;
            }
            match remote {
                Remote::Server { addr, auth } => {
                    self.asked.insert(fetch);
                    self.states.lock().unwrap().insert(sha.clone(), Transfer::Down(0.0));
                    let _ = self.net.send(Job::Download { sha, addr: addr.clone(), auth: auth.clone() });
                }
                Remote::Folder(f) => {
                    self.asked.insert(fetch);
                    let _ = self.net.send(Job::FromFolder { sha, folder: f.join("files") });
                }
                Remote::None => {}
            }
        }
    }

    /// Prepared results for the message boxes to pick up.
    pub fn take_prepared(&mut self) -> Vec<(u64, Result<Attachment, String>)> {
        std::mem::take(&mut self.prepared)
    }

    /// Wait until background work settles (tests).
    #[cfg(test)]
    pub fn settle(&mut self, remote: &Remote, until: impl Fn(&Files) -> bool) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while std::time::Instant::now() < end {
            self.tick(None, remote);
            if until(self) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("files never settled: {:?}", self.states.lock().unwrap());
    }

    #[cfg(test)]
    pub fn decoded(&self, sha: &str, max: u32) -> bool {
        self.ok.contains(&key(sha, max))
    }
}

/// "mov" for "Clip.MOV"; empty without one.
fn ext_of(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or_default()
}

/// <dir>/<sha>.<ext>: a hard link to the cached file (a copy if linking fails).
fn with_ext(dir: &Path, sha: &str, ext: &str) -> PathBuf {
    let plain = dir.join(sha);
    if ext.is_empty() {
        return plain;
    }
    let named = dir.join(format!("{}.{}", sha, ext));
    if !named.exists() && std::fs::hard_link(&plain, &named).is_err() {
        let _ = std::fs::copy(&plain, &named);
    }
    named
}

fn key(sha: &str, max: u32) -> String {
    format!("{}@{}", sha, max)
}

fn log_broken(key: &str) {
    eprintln!("workboard: couldn't show {}", key);
}

fn wb_server_name_ok(sha: &str) -> bool {
    sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Copy `from` into the cache under its hash (temporary name first).
fn store(dir: &Path, from: &Path, sha: &str) -> Result<(), String> {
    let dest = dir.join(sha);
    if dest.is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!(".copy-{:x}", wb_sync::random_u64()));
    std::fs::copy(from, &tmp).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())
}

/// Check a file and get it into the cache (and the shared folder).
fn prepare(dir: &Path, path: Option<&Path>, png: Option<Vec<u8>>, name: &str, folder: Option<&Path>) -> Result<Attachment, String> {
    let (src, tmp) = match (path, png) {
        (Some(p), _) => (p.to_path_buf(), None),
        (None, Some(bytes)) => {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let t = dir.join(format!(".paste-{:x}.png", wb_sync::random_u64()));
            std::fs::write(&t, bytes).map_err(|e| e.to_string())?;
            (t.clone(), Some(t))
        }
        (None, None) => return Err("Nothing to send.".into()),
    };
    let out = (|| {
        let kind = media::kind_of(&src).ok_or_else(|| format!("{} isn't a picture or a video.", name))?;
        let bytes = std::fs::metadata(&src).map_err(|_| format!("{} can't be read.", name))?.len();
        if bytes > MAX_FILE {
            return Err(format!("{} is larger than 500 MB.", name));
        }
        let (w, h, secs) = media::probe(&src, kind)?;
        let (sha, bytes) = wb_net::sha256_file(&src).map_err(|_| format!("{} can't be read.", name))?;
        store(dir, &src, &sha)?;
        if let Some(f) = folder {
            store(f, &src, &sha).map_err(|e| format!("Couldn't put {} in the shared folder: {}", name, e))?;
        }
        Ok(Attachment { file: sha, name: name.to_string(), kind, bytes, w, h, secs })
    })();
    if let Some(t) = tmp {
        let _ = std::fs::remove_file(t);
    }
    out
}

/// The files thread: one upload, download or folder copy.
fn move_file(dir: &Path, job: Job, states: &Mutex<HashMap<String, Transfer>>, wake: &impl Fn()) {
    let set = |sha: &str, t: Transfer| {
        states.lock().unwrap().insert(sha.to_string(), t);
        wake();
    };
    // progress at most every 1%
    let report = |sha: &str, up: bool| {
        let mut last = -1i32;
        let sha = sha.to_string();
        move |done: u64, total: u64| {
            let pct = if total == 0 { 100 } else { (done * 100 / total) as i32 };
            if pct != last {
                last = pct;
                let f = pct as f32 / 100.0;
                set(&sha, if up { Transfer::Up(f) } else { Transfer::Down(f) });
            }
            true
        }
    };
    match job {
        Job::Upload { sha, ctype, addr, auth } => {
            let path = dir.join(&sha);
            let mut progress = report(&sha, true);
            match wb_net::upload(&addr, &auth, &sha, &path, &ctype, &mut progress) {
                Ok(()) => set(&sha, Transfer::Done),
                Err(e) => set(&sha, Transfer::Failed(e.explain())),
            }
        }
        Job::Download { sha, addr, auth } => {
            let mut progress = report(&sha, false);
            match wb_net::download(&addr, &auth, &sha, &dir.join(&sha), &mut progress) {
                Ok(()) => set(&sha, Transfer::Done),
                Err(e) => set(&sha, Transfer::Failed(e.explain())),
            }
        }
        Job::FromFolder { sha, folder } => match store(dir, &folder.join(&sha), &sha) {
            Ok(()) => set(&sha, Transfer::Done),
            Err(_) => set(&sha, Transfer::Failed("The file isn't in the shared folder yet.".into())),
        },
        _ => {}
    }
}

// ------------------------------------------------------------------ the app

/// A picture or video opened large over the window.
pub(super) struct Viewer {
    pub ch: u64,
    pub msg: u64,
    pub index: usize,
    player: Option<(String, media::Player)>,
    frame: Option<egui::TextureHandle>,
}

impl Viewer {
    pub fn new(ch: u64, msg: u64, index: usize) -> Viewer {
        Viewer { ch, msg, index, player: None, frame: None }
    }
}

pub(super) const CHAT_BOX: &str = "chat_composer_text";
pub(super) const THREAD_BOX: &str = "thread_composer_text";

impl App {
    /// Where this workspace's files go besides the cache.
    pub(super) fn media_remote(&self) -> Remote {
        match &self.data.extra.shared {
            Some(crate::sync::SharedCfg { server: Some(addr), device, .. }) => match together::load_token(addr, device) {
                Some(token) => Remote::Server { addr: addr.clone(), auth: wb_net::Auth { device: device.clone(), token } },
                None => Remote::None,
            },
            Some(cfg) if !cfg.folder.is_empty() => Remote::Folder(PathBuf::from(&cfg.folder)),
            _ => Remote::None,
        }
    }

    /// Once a frame: background work, prepared attachments, a paste.
    pub(super) fn files_tick(&mut self, ctx: Option<&egui::Context>) {
        let remote = self.media_remote();
        self.files.tick(ctx, &remote);
        for (local, result) in self.files.take_prepared() {
            let boxes = [&mut self.chat_files, &mut self.thread_files].into_iter().chain(self.drafts.chat_files.saved.values_mut()).chain(self.drafts.thread_files.saved.values_mut());
            for list in boxes {
                if let Some(p) = list.iter_mut().find(|p| p.local == local) {
                    match &result {
                        Ok(a) => {
                            self.files.upload(a, &remote);
                            p.att = Some(a.clone());
                        }
                        Err(e) => p.error = Some(e.clone()),
                    }
                }
            }
            if let Some((_, p)) = self.task_files.iter_mut().find(|(_, p)| p.local == local) {
                match &result {
                    Ok(a) => {
                        self.files.upload(a, &remote);
                        p.att = Some(a.clone());
                    }
                    Err(e) => p.error = Some(e.clone()),
                }
            }
        }
        self.attach_ready_task_files();
        if let Some(p) = self.pasted.take() {
            if self.section == Section::Chat && self.chat_open.is_some() {
                let thread = ctx.is_some_and(|c| c.memory(|m| m.focused()) == Some(egui::Id::new(THREAD_BOX))) && self.thread_open.is_some();
                if p.files.is_empty() {
                    if let Some(png) = p.png {
                        self.add_media(thread, None, Some(png));
                    }
                } else {
                    for f in p.files {
                        self.add_media(thread, Some(f), None);
                    }
                }
            }
        }
    }

    /// Put a file (or a pasted picture) in the channel or thread message box.
    /// At most MAX_PER_MESSAGE per box: past that it says so and leaves the
    /// file out (false).
    pub(super) fn add_media(&mut self, thread: bool, path: Option<PathBuf>, png: Option<Vec<u8>>) -> bool {
        let count = if thread { self.thread_files.len() } else { self.chat_files.len() };
        if count >= MAX_PER_MESSAGE {
            self.toast = Some((trf!("You can send up to {} pictures or videos in one message.", MAX_PER_MESSAGE), now_ts()));
            return false;
        }
        let remote = self.media_remote();
        let p = self.files.add(path, png, &remote);
        if thread {
            self.thread_files.push(p);
        } else {
            self.chat_files.push(p);
        }
        true
    }

    /// The file picker for the + button.
    pub(super) fn pick_media(&mut self, thread: bool) {
        for f in media::pick_files() {
            self.add_media(thread, Some(f), None);
        }
    }

    /// Files dropped on the chat: into the thread box when dropped over the
    /// thread panel, else the channel's.
    pub(super) fn take_dropped(&mut self, ctx: &egui::Context, thread_rect: Option<Rect>) {
        let (dropped, at) = ctx.input(|i| (i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>(), i.pointer.latest_pos()));
        // over an open task they belong to the task (see task_window)
        if dropped.is_empty() || self.open_task.is_some() {
            return;
        }
        let thread = self.thread_open.is_some() && thread_rect.zip(at).is_some_and(|(r, p)| r.contains(p));
        for f in dropped {
            self.add_media(thread, Some(f), None);
        }
    }

    /// Before egui sees the input: a Cmd+V in a message box with files or a
    /// picture on the clipboard. egui only passes on text, so a picture alone
    /// shows up as nothing but the release of V.
    pub(super) fn catch_paste(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let focused = ctx.memory(|m| m.focused());
        if focused != Some(egui::Id::new(CHAT_BOX)) && focused != Some(egui::Id::new(THREAD_BOX)) {
            self.paste_text_seen = false;
            return;
        }
        if raw.events.iter().any(|e| matches!(e, egui::Event::Paste(_))) {
            self.paste_text_seen = true;
            // files copied in Finder come with their names as text: send the files instead
            let p = media::pasteboard();
            if !p.files.is_empty() {
                raw.events.retain(|e| !matches!(e, egui::Event::Paste(_)));
                self.pasted = Some(p);
            }
        }
        let v_up = raw.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::V, pressed: false, modifiers, .. } if modifiers.command));
        if v_up {
            if !self.paste_text_seen {
                let p = media::pasteboard();
                if !p.files.is_empty() || p.png.is_some() {
                    self.pasted = Some(p);
                }
            }
            self.paste_text_seen = false;
        }
    }

    /// Attachments waiting in a message box, as small tiles with a remove
    /// button and how far the upload is.
    pub(super) fn pending_strip(&mut self, ui: &mut Ui, thread: bool) {
        let list = if thread { &self.thread_files } else { &self.chat_files };
        if list.is_empty() {
            return;
        }
        let t = self.t();
        let mut remove = None;
        let mut retry = None;
        ui.horizontal_wrapped(|ui| {
            for p in list {
                let size = vec2(64.0, 64.0);
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let state = p.att.as_ref().and_then(|a| self.files.state(&a.file));
                let failed = p.error.clone().or(match &state {
                    Some(Transfer::Failed(e)) => Some(e.clone()),
                    _ => None,
                });
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, format!("Attachment {}", p.name)));
                let painter = ui.painter_at(rect.expand(6.0));
                match p.att.as_ref().and_then(|a| {
                    self.files.want(a, THUMB);
                    self.files.texture(&a.file, THUMB)
                }) {
                    Some(tex) => {
                        let uv = cover_uv(tex.size_vec2(), size);
                        painter.add(egui::epaint::RectShape::filled(rect, 8.0, Color32::WHITE).with_texture(tex.id(), uv));
                    }
                    None => {
                        painter.rect_filled(rect, 8.0, t.hover);
                        painter.text(rect.center(), Align2::CENTER_CENTER, if p.att.is_none() && failed.is_none() { "..." } else { "" }, FontId::proportional(14.0), t.text3);
                    }
                }
                if p.att.as_ref().is_some_and(|a| a.kind == MediaKind::Video) {
                    play_badge(&painter, rect.center(), 12.0);
                }
                if let Some(Transfer::Up(f)) = state {
                    let bar = Rect::from_min_size(pos2(rect.left() + 6.0, rect.bottom() - 10.0), vec2(rect.width() - 12.0, 4.0));
                    painter.rect_filled(bar, 2.0, Color32::from_black_alpha(120));
                    painter.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height())), 2.0, Color32::WHITE);
                }
                if let Some(why) = &failed {
                    painter.rect_stroke(rect, 8.0, Stroke::new(2.0, t.danger), egui::StrokeKind::Inside);
                    painter.text(rect.center(), Align2::CENTER_CENTER, "!", FontId::proportional(18.0), t.danger);
                    let r = resp.clone().on_hover_text(tr(why));
                    if r.clicked() && p.error.is_none() {
                        retry = p.att.as_ref().map(|a| a.clone());
                    }
                } else {
                    resp.clone().on_hover_text(&p.name);
                }
                // remove
                let x = Rect::from_center_size(rect.right_top() + vec2(-8.0, 8.0), vec2(18.0, 18.0));
                let xr = ui.interact(x, ui.id().with(("unattach", p.local)), Sense::click());
                xr.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Remove {}", p.name)));
                painter.circle_filled(x.center(), 9.0, if xr.hovered() { Color32::from_black_alpha(220) } else { Color32::from_black_alpha(160) });
                for (a, b) in [(vec2(-3.5, -3.5), vec2(3.5, 3.5)), (vec2(-3.5, 3.5), vec2(3.5, -3.5))] {
                    painter.line_segment([x.center() + a, x.center() + b], Stroke::new(1.6, Color32::WHITE));
                }
                if xr.clicked() {
                    remove = Some(p.local);
                }
            }
        });
        if let Some(a) = retry {
            self.files.retry(&a.file);
            let remote = self.media_remote();
            self.files.upload(&a, &remote);
        }
        if let Some(l) = remove {
            let list = if thread { &mut self.thread_files } else { &mut self.chat_files };
            list.retain(|p| p.local != l);
        }
    }

    /// Whether a message box may send, and the attachments it would send.
    pub(super) fn pending_ready(&self, thread: bool) -> (bool, Vec<Attachment>) {
        let list = if thread { &self.thread_files } else { &self.chat_files };
        let ok = list.iter().all(|p| p.ready(&self.files));
        (ok, if ok { list.iter().filter_map(|p| p.att.clone()).collect() } else { vec![] })
    }

    /// A message's pictures and videos. Returns the one clicked.
    pub(super) fn attachment_tiles(&self, ui: &mut Ui, m: &Message) -> Option<usize> {
        if m.attachments.is_empty() {
            return None;
        }
        let t = self.t();
        let mut clicked = None;
        let one = m.attachments.len() == 1;
        let avail = ui.available_width();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (i, a) in m.attachments.iter().enumerate() {
                let aspect = if a.w > 0 && a.h > 0 { a.w as f32 / a.h as f32 } else { 4.0 / 3.0 };
                let size = if one {
                    let w = (320.0 * aspect).min(360.0).min(avail).max(80.0);
                    vec2(w, (w / aspect).min(320.0))
                } else {
                    let h = 150.0;
                    vec2((h * aspect).clamp(100.0, 260.0), h)
                };
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let label = format!("{} {}", if a.kind == MediaKind::Video { "Video" } else { "Picture" }, a.name);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, label.clone()));
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                self.files.want(a, THUMB);
                let painter = ui.painter_at(rect);
                match self.files.texture(&a.file, THUMB) {
                    Some(tex) => {
                        let uv = cover_uv(tex.size_vec2(), size);
                        painter.add(egui::epaint::RectShape::filled(rect, 8.0, Color32::WHITE).with_texture(tex.id(), uv));
                    }
                    None => {
                        painter.rect_filled(rect, 8.0, t.hover);
                        let status = match self.files.state(&a.file) {
                            Some(Transfer::Down(f)) => trf!("Loading {}%", (f * 100.0) as u32),
                            Some(Transfer::Failed(e)) => tr(&e).to_string(),
                            _ if self.files.broken(&a.file, THUMB) => tr("Can't show this file").into(),
                            _ => a.name.clone(),
                        };
                        painter.text(rect.center() + vec2(0.0, if a.kind == MediaKind::Video { 26.0 } else { 0.0 }), Align2::CENTER_CENTER, status, FontId::proportional(12.0), t.text3);
                    }
                }
                if a.kind == MediaKind::Video {
                    play_badge(&painter, rect.center(), 20.0);
                    if a.secs > 0.0 {
                        let txt = media::clock(a.secs);
                        let g = painter.layout_no_wrap(txt, FontId::proportional(11.5), Color32::WHITE);
                        let r = Rect::from_min_size(rect.right_bottom() - g.size() - vec2(14.0, 10.0), g.size() + vec2(8.0, 4.0));
                        painter.rect_filled(r, 4.0, Color32::from_black_alpha(160));
                        painter.galley(r.min + vec2(4.0, 2.0), g, Color32::WHITE);
                    }
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    clicked = Some(i);
                }
            }
        });
        clicked
    }

    /// The message a viewer shows.
    fn viewer_message(&self, v: &Viewer) -> Option<Message> {
        let c = self.data.channels.iter().find(|c| c.id == v.ch)?;
        c.messages.iter().flat_map(|m| std::iter::once(m).chain(m.replies.iter())).find(|m| m.id == v.msg).cloned()
    }

    /// A picture or video opened large, over everything.
    pub(super) fn media_viewer(&mut self, ctx: &egui::Context) {
        let Some(mut v) = self.viewer.take() else { return };
        let Some(m) = self.viewer_message(&v) else { return };
        if m.attachments.is_empty() {
            return;
        }
        v.index = v.index.min(m.attachments.len() - 1);
        let a = m.attachments[v.index].clone();
        let t = self.t();
        let screen = ctx.screen_rect();
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut step: i32 = 0;
        if m.attachments.len() > 1 {
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                step = 1;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                step = -1;
            }
        }
        let local = self.files.local(&a.file);
        if a.kind == MediaKind::Image {
            self.files.want(&a, FULL);
            self.files.want(&a, THUMB);
        } else if local.is_none() {
            self.files.want(&a, THUMB);
        }
        // the video player follows the file shown
        if a.kind == MediaKind::Video {
            if v.player.as_ref().map(|(s, _)| s != &a.file).unwrap_or(true) {
                v.player = None;
                v.frame = None;
                if let Some(path) = self.files.playable(&a).filter(|_| local.is_some()) {
                    let path = &path;
                    let (w, h) = fit(a.w.max(1) as f32, a.h.max(1) as f32, 1280.0, 1280.0);
                    if let Some(mut p) = media::Player::new(path, w as u32, h as u32) {
                        p.play();
                        v.player = Some((a.file.clone(), p));
                    }
                }
            }
            if let Some((_, p)) = v.player.as_mut() {
                if let Some(f) = p.frame() {
                    match v.frame.as_mut() {
                        Some(tex) => tex.set(f.to_color_image(), egui::TextureOptions::LINEAR),
                        None => v.frame = Some(ctx.load_texture("media-player", f.to_color_image(), egui::TextureOptions::LINEAR)),
                    }
                }
                if p.playing() {
                    ctx.request_repaint();
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(250));
                }
            }
        } else {
            v.player = None;
            v.frame = None;
        }

        egui::Area::new(egui::Id::new("media_viewer")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
            let (bg, bg_resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(bg, 0.0, Color32::from_black_alpha(225));
            let bar_h = 56.0;
            let area = Rect::from_min_max(bg.min + vec2(60.0, 50.0), bg.max - vec2(60.0, bar_h + 20.0));
            // the picture or frame, fitted
            let tex = match a.kind {
                MediaKind::Image => self.files.texture(&a.file, FULL).or_else(|| self.files.texture(&a.file, THUMB)).cloned(),
                MediaKind::Video => v.frame.clone().or_else(|| self.files.texture(&a.file, THUMB).cloned()),
            };
            let (w, h) = fit(a.w.max(1) as f32, a.h.max(1) as f32, area.width(), area.height());
            let pic = Rect::from_center_size(area.center(), vec2(w, h));
            let pic_resp = ui.interact(pic, ui.id().with("media_pic"), Sense::click());
            pic_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, format!("Large {}", a.name)));
            match &tex {
                Some(tex) => {
                    ui.painter().image(tex.id(), pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                None => {
                    let status = match self.files.state(&a.file) {
                        Some(Transfer::Down(f)) => trf!("Loading {}%", (f * 100.0) as u32),
                        Some(Transfer::Failed(e)) => tr(&e).to_string(),
                        _ => tr("Loading...").into(),
                    };
                    ui.painter().text(pic.center(), Align2::CENTER_CENTER, status, FontId::proportional(15.0), Color32::from_gray(200));
                }
            }
            if bg_resp.clicked() && !pic.contains(bg_resp.interact_pointer_pos().unwrap_or(pic.center())) {
                close = true;
            }
            // top: name and the buttons
            let top = Rect::from_min_size(bg.min + vec2(20.0, 12.0), vec2(bg.width() - 40.0, 30.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(top).layout(Layout::left_to_right(Align::Center)), |ui| {
                let count = if m.attachments.len() > 1 { trf!("  {} of {}", v.index + 1, m.attachments.len()) } else { String::new() };
                ui.label(RichText::new(format!("{}{}", a.name, count)).color(Color32::WHITE).size(14.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let b = |ui: &mut Ui, s: &str| ui.add(egui::Button::new(RichText::new(s).color(Color32::WHITE)).fill(Color32::from_white_alpha(28)).corner_radius(6).min_size(vec2(0.0, 28.0)));
                    if b(ui, tr("Close")).on_hover_text("Esc").clicked() {
                        close = true;
                    }
                    if let Some(path) = &local {
                        if b(ui, tr("Save to Downloads")).clicked() {
                            match save_copy(path, &a.name) {
                                Ok(dest) => self.toast = Some((trf!("Saved {}", dest.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), now_ts())),
                                Err(e) => self.toast = Some((trf!("Couldn't save: {}", e), now_ts())),
                            }
                        }
                        if b(ui, tr("Show in Finder")).clicked() {
                            let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
                        }
                    }
                });
            });
            // previous / next
            if m.attachments.len() > 1 {
                for (dir, x, glyph) in [(-1, bg.left() + 30.0, "\u{2039}"), (1, bg.right() - 30.0, "\u{203A}")] {
                    let r = Rect::from_center_size(pos2(x, area.center().y), vec2(40.0, 60.0));
                    let resp = ui.interact(r, ui.id().with(("media_step", dir)), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, if dir < 0 { "Previous" } else { "Next" }));
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(40.0), if resp.hovered() { Color32::WHITE } else { Color32::from_gray(170) });
                    if resp.clicked() {
                        step = dir;
                    }
                }
            }
            // video controls
            if a.kind == MediaKind::Video {
                let bar = Rect::from_min_size(pos2(area.center().x - 300.0, bg.bottom() - bar_h - 12.0), vec2(600.0, bar_h));
                ui.painter().rect_filled(bar, 10.0, Color32::from_white_alpha(22));
                if let Some((_, p)) = v.player.as_mut() {
                    let (pos, dur) = (p.position(), p.duration().max(a.secs).max(0.001));
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(bar.shrink2(vec2(12.0, 0.0))).layout(Layout::left_to_right(Align::Center)), |ui| {
                        let playing = p.playing();
                        let pb = ui.add(egui::Button::new(RichText::new(if playing { "\u{23F8}" } else { "\u{25B6}" }).color(Color32::WHITE).size(16.0)).frame(false).min_size(vec2(32.0, 32.0)));
                        pb.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, if playing { "Pause" } else { "Play" }));
                        if pb.clicked() || ui.input(|i| i.key_pressed(egui::Key::Space)) {
                            if playing {
                                p.pause();
                            } else {
                                p.play();
                            }
                        }
                        ui.label(RichText::new(media::clock(pos)).color(Color32::WHITE).monospace().size(12.0));
                        // the bar: click or drag to move
                        let track_w = ui.available_width() - 110.0;
                        let (tr, tresp) = ui.allocate_exact_size(vec2(track_w.max(40.0), 24.0), Sense::click_and_drag());
                        tresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Video position"));
                        let line = Rect::from_center_size(tr.center(), vec2(tr.width(), 4.0));
                        ui.painter().rect_filled(line, 2.0, Color32::from_white_alpha(60));
                        let f = (pos / dur).clamp(0.0, 1.0);
                        ui.painter().rect_filled(Rect::from_min_size(line.min, vec2(line.width() * f, 4.0)), 2.0, Color32::WHITE);
                        ui.painter().circle_filled(pos2(line.left() + line.width() * f, line.center().y), if tresp.hovered() || tresp.dragged() { 7.0 } else { 5.0 }, Color32::WHITE);
                        if let Some(at) = tresp.interact_pointer_pos().filter(|_| tresp.clicked() || tresp.dragged()) {
                            p.seek(((at.x - line.left()) / line.width()).clamp(0.0, 1.0) * dur);
                        }
                        ui.label(RichText::new(media::clock(dur)).color(Color32::from_gray(200)).monospace().size(12.0));
                        let muted = p.muted();
                        let mb = ui.add(egui::Button::new(RichText::new(if muted { "\u{1F507}" } else { "\u{1F50A}" }).color(Color32::WHITE).size(15.0)).frame(false).min_size(vec2(32.0, 32.0)));
                        mb.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, if muted { "Turn sound on" } else { "Mute" }));
                        if mb.clicked() {
                            p.set_muted(!muted);
                        }
                    });
                } else {
                    let status = match self.files.state(&a.file) {
                        Some(Transfer::Down(f)) => trf!("Loading the video {}%", (f * 100.0) as u32),
                        Some(Transfer::Failed(e)) => tr(&e).to_string(),
                        _ if local.is_some() => tr("This video can't be played on this Mac.").into(),
                        _ => tr("Loading the video...").into(),
                    };
                    ui.painter().text(bar.center(), Align2::CENTER_CENTER, status, FontId::proportional(13.0), Color32::from_gray(220));
                    if local.is_none() {
                        // the download is what the player waits for
                        self.files.want(&a, FULL);
                    }
                }
            }
        });
        if step != 0 {
            let n = m.attachments.len() as i32;
            v.index = ((v.index as i32 + step).rem_euclid(n)) as usize;
        }
        let _ = t;
        if !close {
            self.viewer = Some(v);
        }
    }
}

/// The part of a texture that fills `size` without stretching (cropped).
pub(super) fn cover_uv(tex: egui::Vec2, size: egui::Vec2) -> Rect {
    let (ta, sa) = (tex.x / tex.y.max(1.0), size.x / size.y.max(1.0));
    if ta > sa {
        let w = sa / ta;
        Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
    } else {
        let h = ta / sa;
        Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
    }
}

/// `w`×`h` scaled to fit inside `mw`×`mh` (never enlarged past 2x).
fn fit(w: f32, h: f32, mw: f32, mh: f32) -> (f32, f32) {
    let s = (mw / w).min(mh / h).min(2.0);
    (w * s, h * s)
}

/// A round dark badge with a play triangle.
fn play_badge(p: &egui::Painter, c: egui::Pos2, r: f32) {
    p.circle_filled(c, r, Color32::from_black_alpha(150));
    let k = r * 0.45;
    p.add(egui::Shape::convex_polygon(vec![c + vec2(-k * 0.7, -k), c + vec2(k, 0.0), c + vec2(-k * 0.7, k)], Color32::WHITE, Stroke::NONE));
}

/// Copy a file to ~/Downloads under its name, without overwriting.
fn save_copy(from: &Path, name: &str) -> Result<PathBuf, String> {
    let dir = PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Downloads");
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut dest = dir.join(format!("{}{}", stem, ext));
    let mut n = 2;
    while dest.exists() {
        dest = dir.join(format!("{} {}{}", stem, n, ext));
        n += 1;
    }
    std::fs::copy(from, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}
