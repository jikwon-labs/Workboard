// Pictures and videos in chat: getting them ready, moving them to and from
// the server, and turning them into textures.
//
// Every file sits in a cache folder named by its sha256 (the attachment's
// `file`). Sending: the file is checked (kind, size, picture/video size),
// hashed and copied into the cache on a worker thread, then put on the server
// (or in the shared folder's files/), and only then can the message go;
// while the server can't be reached (or this computer isn't signed in) the
// message goes anyway and its files follow from the backlog.
// Showing: a message's file comes from the cache, else the server, else the
// shared folder; pictures for the screen are decoded on a worker thread.
//
// Moving files runs on two lanes of two "workboard-files" threads each, one
// lane for pictures and one for videos, so a big video never holds up a
// picture; "workboard-media" prepares and decodes. A file that couldn't be
// fetched is tried again later (see RETRY_SECS), as is one that a sent
// message refers to but that didn't get to the server (`upload_soon`). Those
// waiting uploads are written to <cache>/.uploads.json, so quitting doesn't
// lose them: they carry on at the next start.
//
// A video goes out with a still (a small JPEG, `Attachment::thumb`), so the
// others show the still and fetch the video only when it's played.
//
// Cleaning up (`clean_cache`, `clean_shared_folder`): the cache drops files
// no workspace on this computer refers to (the Trash counts) after a while,
// and the least recently used shared ones past a size cap (never ones only a
// Trash refers to); the shared folder's files/ drops files none of the
// members' copies refer to once they're 30 days old and nothing has named
// them for 60 days.

use super::*;
use crate::media::{self, Rgba};
use std::cell::RefCell;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Longer side of a picture in the chat.
pub(super) const THUMB: u32 = 720;
/// Longer side of a picture opened large.
pub(super) const FULL: u32 = 2560;
/// The size `Files::fetch` asks for: the file only, nothing decoded.
const FETCH_ONLY: u32 = 0;
/// The server takes files up to this size (MAX_FILE in server/src/files.rs:
/// under the 100 MB Cloudflare passes on its free plan).
pub(super) const MAX_FILE: u64 = 90 << 20;
/// Pictures and videos in one message.
pub(super) const MAX_PER_MESSAGE: usize = 10;
/// Waits before fetching a missing file again: after the first failure, the
/// second, the third, and from then on.
const RETRY_SECS: [u64; 4] = [5, 15, 60, 300];
/// A file a message names that isn't on the server (or in the shared
/// folder) yet is looked for this often, this many times, before the
/// waits above: a message goes out while its files are still going up.
const SOON_SECS: u64 = 3;
const SOON_TRIES: usize = 60;
/// Why an attachment waits when the workspace is on a server this computer
/// isn't signed in to (it goes up by itself once it is).
pub(super) const NOT_SIGNED_IN: &str =
    "This computer isn't signed in to the team server yet. The file goes up once it is.";
/// The waiting uploads, kept in the cache folder.
const BACKLOG: &str = ".uploads.json";
/// A cached file no workspace refers to is kept this long (a message box
/// may be about to send it, a deleted message may come back by undo).
pub(super) const CACHE_GRACE: Duration = Duration::from_secs(14 * 24 * 3600);
/// The cache's size past which the least recently used team files go
/// (they can be fetched again).
pub(super) const CACHE_CAP: u64 = 2 << 30;
/// A file in the shared folder no member's copy refers to is removed once
/// it's this old.
pub(super) const FOLDER_GRACE: Duration = Duration::from_secs(30 * 24 * 3600);
/// ... and only once nothing has named it for this long (the team server's
/// FILE_KEEP_DAYS): a deleted message's files outlast it a while, for a
/// restore from someone's Trash.
pub(super) const FOLDER_NAMED_GRACE: Duration = Duration::from_secs(60 * 24 * 3600);
/// In the shared folder's files/: file -> when a member's copy last named
/// it (unix seconds), as seen by `clean_shared_folder`.
pub(super) const FOLDER_NAMED: &str = ".named.json";
/// How often the cleaning runs (and the first time, after the start).
const CLEAN_EVERY: Duration = Duration::from_secs(6 * 3600);
const CLEAN_FIRST: Duration = Duration::from_secs(90);

/// An attachment waiting in the message box.
#[derive(Clone, Debug)]
pub(super) struct Pending {
    pub local: u64,
    pub name: String,
    /// None while it's being checked and copied.
    pub att: Option<Attachment>,
    pub error: Option<String>,
}

/// A fetch found the file not there (yet).
fn not_there_yet(e: &str) -> bool {
    e == "The server doesn't have this file." || e == "The file isn't in the shared folder yet."
}

/// A file waits for the server rather than failed for good: the server
/// can't be reached, or this computer isn't signed in to it. It goes up by
/// itself once it can.
pub(super) fn waits_for_server(e: &str) -> bool {
    e == NOT_SIGNED_IN || e.starts_with("Couldn't reach the server")
}

impl Pending {
    /// Ready to go out: checked, and on the server when there is one (with
    /// a video's still).
    pub fn ready(&self, files: &Files) -> bool {
        self.error.is_none()
            && self.att.as_ref().is_some_and(|a| {
                parts(a)
                    .iter()
                    .all(|p| files.state(&p.file) == Some(Transfer::Done))
            })
    }

    /// A message may go with it: ready, or in the cache while it's still
    /// going up or the server can't be reached; then the file follows
    /// (`send_files` puts it in the backlog).
    pub fn sendable(&self, files: &Files) -> bool {
        self.error.is_none()
            && self.att.as_ref().is_some_and(|a| {
                parts(a).iter().all(|p| match files.state(&p.file) {
                    Some(Transfer::Done) => true,
                    Some(Transfer::Up(_)) => files.local(&p.file).is_some(),
                    Some(Transfer::Failed(e)) => {
                        waits_for_server(&e) && files.local(&p.file).is_some()
                    }
                    _ => false,
                })
            })
    }

    /// Still being checked and copied: a message may go before it, and it's
    /// put on the message once it's ready (`ChatUi::following`).
    pub fn getting_ready(&self) -> bool {
        self.att.is_none() && self.error.is_none()
    }

    /// Where it is: getting ready, going up (0..1), failed (why), or done.
    pub fn transfer(&self, files: &Files) -> Option<Transfer> {
        if let Some(e) = &self.error {
            return Some(Transfer::Failed(e.clone()));
        }
        let a = self.att.as_ref()?;
        let states: Vec<Option<Transfer>> = parts(a).iter().map(|p| files.state(&p.file)).collect();
        if let Some(f) = states
            .iter()
            .flatten()
            .find(|t| matches!(t, Transfer::Failed(_)))
        {
            return Some(f.clone());
        }
        states.into_iter().next().flatten()
    }
}

/// A video's still as an attachment of its own (a picture), for fetching
/// and showing it like one.
pub(super) fn still_of(a: &Attachment) -> Option<Attachment> {
    let sha = a.thumb.as_ref().filter(|t| wb_server_name_ok(t))?;
    (a.kind == MediaKind::Video).then(|| Attachment {
        file: sha.clone(),
        name: "still.jpg".into(),
        kind: MediaKind::Image,
        w: a.w,
        h: a.h,
        ..Default::default()
    })
}

/// What shows for an attachment in a tile: a video's still when it has
/// one (so the video itself isn't fetched), else the attachment.
pub(super) fn shown(a: &Attachment) -> Attachment {
    still_of(a).unwrap_or_else(|| a.clone())
}

/// The files an attachment is made of: itself, and a video's still.
pub(super) fn parts(a: &Attachment) -> Vec<Attachment> {
    std::iter::once(a.clone()).chain(still_of(a)).collect()
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
    /// a team server (its address), but no sign-in for it on this
    /// computer (yet)
    SignedOut(String),
}

impl Remote {
    /// Where an upload is meant to end up, as kept in the backlog:
    /// "server:<addr>" or "folder:<path>"; None without anywhere to go.
    pub fn target(&self) -> Option<String> {
        match self {
            Remote::None => None,
            Remote::Folder(f) => Some(format!("folder:{}", f.display())),
            Remote::Server { addr, .. } | Remote::SignedOut(addr) => {
                Some(format!("server:{}", addr))
            }
        }
    }
}

/// One upload waiting in the backlog file.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
struct Waiting {
    target: String,
    att: Attachment,
}

/// A file that must get somewhere: where, and the tries so far.
struct Bg {
    att: Attachment,
    target: String,
    fetch: Fetch,
}

enum Job {
    Prepare {
        local: u64,
        path: Option<PathBuf>,
        png: Option<Vec<u8>>,
        name: String,
        folder: Option<PathBuf>,
    },
    Upload {
        sha: String,
        ctype: String,
        addr: String,
        auth: wb_net::Auth,
    },
    Download {
        sha: String,
        addr: String,
        auth: wb_net::Auth,
    },
    FromFolder {
        sha: String,
        folder: PathBuf,
    },
    /// copy a cached file into the shared folder
    ToFolder {
        sha: String,
        folder: PathBuf,
    },
    Decode {
        key: String,
        sha: String,
        ext: String,
        kind: MediaKind,
        max: u32,
    },
}

enum Done {
    Prepared {
        local: u64,
        result: Result<Attachment, String>,
    },
    Decoded {
        key: String,
        img: Option<Rgba>,
    },
}

/// Fetching a file that isn't here: how many tries so far, and when the
/// next one is due (set once the last one failed).
struct Fetch {
    tries: usize,
    next: Option<Instant>,
}

pub(super) struct Files {
    dir: PathBuf,
    /// moving pictures
    small: Sender<Job>,
    /// moving videos
    net: Sender<Job>,
    work: Sender<Job>,
    done: Receiver<Done>,
    states: Arc<Mutex<HashMap<String, Transfer>>>,
    /// files a message on screen wants (asked while drawing)
    wants: RefCell<Vec<(String, String, MediaKind, u32)>>,
    asked: HashSet<String>,
    fetches: HashMap<String, Fetch>,
    /// files that must get to the server (or the shared folder) though no
    /// message box waits for them (sent messages, earlier ones when sharing
    /// starts, task attachments), tried again like fetches until they're
    /// there; kept in BACKLOG
    background: HashMap<String, Bg>,
    /// `background` changed since it was last written
    backlog_dirty: bool,
    /// when the cleaning last started, and whether it's running
    cleaned: Option<Instant>,
    cleaning: Arc<AtomicBool>,
    started: Instant,
    /// "server\nsha" for each file known to be on a server
    on_server: Arc<Mutex<HashSet<String>>>,
    /// uploads to stop (their attachment was taken out of the message box)
    cancelled: Arc<Mutex<HashSet<String>>>,
    /// the server sign-in last read from the keychain: (server, device,
    /// together::token_gen() then, token)
    token: RefCell<Option<(String, String, u64, Option<String>)>>,
    textures: HashMap<String, Option<egui::TextureHandle>>,
    /// decoded keys that came out as a picture
    ok: HashSet<String>,
    /// prepared attachments, by the Pending they belong to
    prepared: Vec<(u64, Result<Attachment, String>)>,
    /// Keep prepared files back from the message boxes (tests: a file
    /// still getting ready).
    #[cfg(test)]
    pub hold_prepared: bool,
    next_local: u64,
    ctx: Arc<Mutex<Option<egui::Context>>>,
}

impl Files {
    pub fn new(dir: PathBuf) -> Files {
        let (net, net_rx) = mpsc::channel::<Job>();
        let (small, small_rx) = mpsc::channel::<Job>();
        let cancelled: Arc<Mutex<HashSet<String>>> = Default::default();
        let (work, work_rx) = mpsc::channel::<Job>();
        let (done_tx, done) = mpsc::channel::<Done>();
        let states: Arc<Mutex<HashMap<String, Transfer>>> = Default::default();
        let ctx: Arc<Mutex<Option<egui::Context>>> = Default::default();
        let on_server: Arc<Mutex<HashSet<String>>> = Default::default();
        let wake = {
            let ctx = ctx.clone();
            move || {
                if let Some(c) = ctx.lock().unwrap().as_ref() {
                    c.request_repaint();
                }
            }
        };
        for rx in [net_rx, small_rx] {
            let rx = Arc::new(Mutex::new(rx));
            for _ in 0..2 {
                let (dir, states, wake, rx, cancelled, on_server) = (
                    dir.clone(),
                    states.clone(),
                    wake.clone(),
                    rx.clone(),
                    cancelled.clone(),
                    on_server.clone(),
                );
                let _ = std::thread::Builder::new()
                    .name("workboard-files".into())
                    .spawn(move || loop {
                        let job = rx.lock().unwrap().recv();
                        match job {
                            Ok(job) => move_file(&dir, job, &states, &cancelled, &on_server, &wake),
                            Err(_) => return,
                        }
                    });
            }
        }
        {
            let (dir, done_tx) = (dir.clone(), done_tx);
            let _ = std::thread::Builder::new()
                .name("workboard-media".into())
                .spawn(move || {
                    for job in work_rx {
                        let out = match job {
                            Job::Prepare {
                                local,
                                path,
                                png,
                                name,
                                folder,
                            } => Done::Prepared {
                                local,
                                result: prepare(
                                    &dir,
                                    path.as_deref(),
                                    png,
                                    &name,
                                    folder.as_deref(),
                                ),
                            },
                            Job::Decode {
                                key,
                                sha,
                                ext,
                                kind,
                                max,
                            } => {
                                touch(&dir.join(&sha));
                                Done::Decoded {
                                    key,
                                    img: media::thumbnail(&with_ext(&dir, &sha, &ext), kind, max),
                                }
                            }
                            _ => continue,
                        };
                        if done_tx.send(out).is_err() {
                            return;
                        }
                        wake();
                    }
                });
        }
        let background = load_backlog(&dir)
            .into_iter()
            .map(|w| {
                (
                    w.att.file.clone(),
                    Bg {
                        att: w.att,
                        target: w.target,
                        fetch: Fetch {
                            tries: 1,
                            next: None,
                        },
                    },
                )
            })
            .collect();
        Files {
            dir,
            small,
            net,
            work,
            done,
            states,
            wants: Default::default(),
            asked: Default::default(),
            fetches: Default::default(),
            background,
            backlog_dirty: false,
            cleaned: None,
            cleaning: Default::default(),
            started: Instant::now(),
            on_server,
            cancelled,
            token: Default::default(),
            textures: Default::default(),
            ok: Default::default(),
            prepared: vec![],
            #[cfg(test)]
            hold_prepared: false,
            next_local: 1,
            ctx,
        }
    }

    /// The file in the cache, when it's there.
    pub fn local(&self, sha: &str) -> Option<PathBuf> {
        let p = self.dir.join(sha);
        (wb_server_name_ok(sha) && p.is_file()).then_some(p)
    }

    pub fn state(&self, sha: &str) -> Option<Transfer> {
        self.states.lock().unwrap().get(sha).cloned()
    }

    /// A Pending for a file already in the cache (one read back after a
    /// restart).
    pub fn pending_for(&mut self, att: Attachment) -> Pending {
        let local = self.next_local;
        self.next_local += 1;
        Pending {
            local,
            name: att.name.clone(),
            att: Some(att),
            error: None,
        }
    }

    /// Start checking and copying a file (or pasted PNG) for the message box.
    pub fn add(&mut self, path: Option<PathBuf>, png: Option<Vec<u8>>, remote: &Remote) -> Pending {
        let local = self.next_local;
        self.next_local += 1;
        let name = path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Pasted image.png".into());
        let folder = match remote {
            Remote::Folder(f) => Some(f.join("files")),
            _ => None,
        };
        let _ = self.work.send(Job::Prepare {
            local,
            path,
            png,
            name: name.clone(),
            folder,
        });
        Pending {
            local,
            name,
            att: None,
            error: None,
        }
    }

    /// The lane a transfer goes on: pictures on their own, so a big video
    /// or file never holds one up.
    fn lane(&self, kind: MediaKind) -> &Sender<Job> {
        match kind {
            MediaKind::Image => &self.small,
            MediaKind::Video | MediaKind::File => &self.net,
        }
    }

    /// The token this computer has for a server. Read from the keychain once
    /// and kept until the server or device changes or a sign-in is saved or
    /// forgotten (the app asks every frame).
    pub fn token(&self, addr: &str, device: &str) -> Option<String> {
        let gen = together::token_gen();
        let mut c = self.token.borrow_mut();
        if let Some((a, d, g, t)) = c.as_ref() {
            if a == addr && d == device && *g == gen {
                return t.clone();
            }
        }
        let t = together::load_token(addr, device);
        *c = Some((addr.to_string(), device.to_string(), gen, t.clone()));
        t
    }

    /// A file is in the cache: send it to the server, or (without one) it's
    /// ready as it is. On a server this computer isn't signed in to, it
    /// waits with NOT_SIGNED_IN as its error.
    pub fn upload(&self, att: &Attachment, remote: &Remote) {
        for p in parts(att) {
            self.upload_one(&p, remote);
        }
    }

    fn upload_one(&self, att: &Attachment, remote: &Remote) {
        let (addr, auth) = match remote {
            Remote::Server { addr, auth } => (addr, auth),
            Remote::SignedOut(_) => {
                self.states
                    .lock()
                    .unwrap()
                    .insert(att.file.clone(), Transfer::Failed(NOT_SIGNED_IN.into()));
                return;
            }
            Remote::None | Remote::Folder(_) => {
                self.states
                    .lock()
                    .unwrap()
                    .insert(att.file.clone(), Transfer::Done);
                return;
            }
        };
        {
            let mut s = self.states.lock().unwrap();
            if matches!(s.get(&att.file), Some(Transfer::Up(_))) {
                return;
            }
            s.insert(att.file.clone(), Transfer::Up(0.0));
            drop(s);
            self.cancelled.lock().unwrap().remove(&att.file);
            let ctype = media::content_type(Path::new(&att.name)).to_string();
            let _ = self.lane(att.kind).send(Job::Upload {
                sha: att.file.clone(),
                ctype,
                addr: addr.clone(),
                auth: auth.clone(),
            });
        }
    }

    /// A file a sent message refers to (or one from before sharing started):
    /// get it where the team can fetch it, and on a server keep trying
    /// (after RETRY_SECS) until it's there. Only files this computer has.
    /// The backlog is kept on disk, so this carries on after a restart.
    pub fn upload_soon(&mut self, att: &Attachment, remote: &Remote) {
        let Some(target) = remote.target() else {
            return;
        };
        for p in parts(att) {
            match remote {
                Remote::None => {}
                Remote::Folder(f) => {
                    if self.local(&p.file).is_none() || f.join("files").join(&p.file).is_file() {
                        continue;
                    }
                    self.wait_for(&p, &target);
                    self.to_folder(&p, &f.join("files"));
                }
                Remote::Server { addr, .. } if self.is_on_server(addr, &p.file) => {}
                Remote::Server { .. } | Remote::SignedOut(_) => {
                    if self.local(&p.file).is_none() {
                        continue;
                    }
                    self.wait_for(&p, &target);
                    self.upload_one(&p, remote);
                }
            }
        }
    }

    /// Like `upload_soon`, without trusting what this computer last knew
    /// of the server: a file the server had may be gone since (cleaned up
    /// while only someone's Trash named it).
    pub fn reupload(&mut self, att: &Attachment, remote: &Remote) {
        if let Remote::Server { addr, .. } = remote {
            self.on_server
                .lock()
                .unwrap()
                .remove(&format!("{}\n{}", addr, att.file));
        }
        self.upload_soon(att, remote);
    }

    /// Put a file in the backlog (once).
    fn wait_for(&mut self, att: &Attachment, target: &str) {
        if !self.background.contains_key(&att.file) {
            self.background.insert(
                att.file.clone(),
                Bg {
                    att: att.clone(),
                    target: target.to_string(),
                    fetch: Fetch {
                        tries: 1,
                        next: None,
                    },
                },
            );
            self.backlog_dirty = true;
        }
    }

    /// Take a file out of the backlog and stop its upload (a task
    /// attachment dismissed before it got there).
    pub fn forget(&mut self, att: &Attachment) {
        for p in parts(att) {
            if self.background.remove(&p.file).is_some() {
                self.backlog_dirty = true;
            }
            self.cancel_upload(&p.file);
        }
    }

    /// Files waiting in the backlog (they must stay in the cache).
    pub fn waiting(&self) -> impl Iterator<Item = &String> {
        self.background.keys()
    }

    fn is_on_server(&self, addr: &str, sha: &str) -> bool {
        self.on_server
            .lock()
            .unwrap()
            .contains(&format!("{}\n{}", addr, sha))
    }

    /// Stop sending a file (its tile was removed before it got there),
    /// unless a sent message or sharing still needs it on the server.
    pub fn cancel_upload(&self, sha: &str) {
        if self.background.contains_key(sha) {
            return;
        }
        let mut s = self.states.lock().unwrap();
        if matches!(s.get(sha), Some(Transfer::Up(_))) {
            self.cancelled.lock().unwrap().insert(sha.to_string());
            s.remove(sha);
        }
    }

    /// Stop an attachment's upload (with its still), see `cancel_upload`.
    pub fn cancel_attachment(&self, att: &Attachment) {
        for p in parts(att) {
            self.cancel_upload(&p.file);
        }
    }

    /// Copy a cached file into the shared folder's files/ (quietly; for
    /// files sent before the workspace was shared).
    pub fn to_folder(&self, att: &Attachment, folder: &Path) {
        if self.local(&att.file).is_some() && !folder.join(&att.file).is_file() {
            let _ = self.lane(att.kind).send(Job::ToFolder {
                sha: att.file.clone(),
                folder: folder.to_path_buf(),
            });
        }
    }

    /// While drawing: this attachment is on screen and should be shown at `max`.
    /// A plain file is never decoded (see `fetch`).
    pub fn want(&self, a: &Attachment, max: u32) {
        // the name comes from another member's synced message: only a sha256
        // may become a cache path or a request to the server
        if !wb_server_name_ok(&a.file) || a.kind == MediaKind::File {
            return;
        }
        self.wants
            .borrow_mut()
            .push((a.file.clone(), ext_of(&a.name), a.kind, max));
    }

    /// Bring a file into the cache without showing it (a plain file someone
    /// asked to save); asked each frame until it's here, tried again like
    /// pictures when it can't be fetched.
    pub fn fetch(&self, a: &Attachment) {
        if !wb_server_name_ok(&a.file) {
            return;
        }
        self.wants
            .borrow_mut()
            .push((a.file.clone(), ext_of(&a.name), a.kind, FETCH_ONLY));
    }

    /// The cached file under a name with the attachment's extension, for
    /// AVFoundation (which goes by the extension, not the contents).
    pub fn playable(&self, a: &Attachment) -> Option<PathBuf> {
        touch(&self.local(&a.file)?);
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

    /// Try a failed transfer again (now, not when the next try is due).
    pub fn retry(&mut self, sha: &str) {
        self.states.lock().unwrap().remove(sha);
        self.asked.retain(|k| !k.starts_with(sha));
        self.fetches.remove(sha);
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
                    let tex = img.zip(ctx).map(|(img, c)| {
                        c.load_texture(
                            format!("media-{}", key),
                            img.to_color_image(),
                            egui::TextureOptions::LINEAR,
                        )
                    });
                    self.textures.insert(key, tex);
                }
            }
        }
        self.tick_background(ctx, remote);
        if self.backlog_dirty {
            self.backlog_dirty = false;
            save_backlog(
                &self.dir,
                &self
                    .background
                    .values()
                    .map(|b| Waiting {
                        target: b.target.clone(),
                        att: b.att.clone(),
                    })
                    .collect::<Vec<_>>(),
            );
        }
        for (sha, ext, kind, max) in self.wants.take() {
            let k = key(&sha, max);
            if self.textures.contains_key(&k) || self.asked.contains(&k) {
                continue;
            }
            if max == FETCH_ONLY && self.local(&sha).is_some() {
                self.fetches.remove(&sha);
                continue;
            }
            if self.local(&sha).is_some() {
                self.fetches.remove(&sha);
                self.asked.insert(k.clone());
                let _ = self.work.send(Job::Decode {
                    key: k,
                    sha,
                    ext,
                    kind,
                    max,
                });
                continue;
            }
            // not here yet: fetch it, and when that fails try again a while later
            if matches!(remote, Remote::None | Remote::SignedOut(_))
                || matches!(self.state(&sha), Some(Transfer::Down(_) | Transfer::Up(_)))
            {
                continue;
            }
            let now = Instant::now();
            // not there yet: the sender may still be putting it up (a
            // message goes before its files), so look again soon for a while
            let on_its_way =
                matches!(self.state(&sha), Some(Transfer::Failed(e)) if not_there_yet(&e));
            if let Some(f) = self.fetches.get_mut(&sha) {
                let at = *f.next.get_or_insert_with(|| {
                    now + Duration::from_secs(if on_its_way && f.tries < SOON_TRIES {
                        SOON_SECS
                    } else {
                        RETRY_SECS[(f.tries - 1).min(RETRY_SECS.len() - 1)]
                    })
                });
                if now < at {
                    if let Some(c) = ctx {
                        c.request_repaint_after(at - now);
                    }
                    continue;
                }
                f.tries += 1;
                f.next = None;
            } else {
                self.fetches.insert(
                    sha.clone(),
                    Fetch {
                        tries: 1,
                        next: None,
                    },
                );
            }
            let job = match remote {
                Remote::Server { addr, auth } => Job::Download {
                    sha: sha.clone(),
                    addr: addr.clone(),
                    auth: auth.clone(),
                },
                Remote::Folder(f) => Job::FromFolder {
                    sha: sha.clone(),
                    folder: f.join("files"),
                },
                Remote::None | Remote::SignedOut(_) => continue,
            };
            self.states.lock().unwrap().insert(sha, Transfer::Down(0.0));
            let _ = self.lane(kind).send(job);
        }
    }

    /// Files that must get to the server (or the shared folder): start the
    /// ones not sent there yet (or waiting for a sign-in), and try failed
    /// ones again when due. Only the ones meant for where this workspace
    /// is shared now; the rest wait for theirs.
    fn tick_background(&mut self, ctx: Option<&egui::Context>, remote: &Remote) {
        let Some(target) = remote.target() else {
            return;
        };
        let now = Instant::now();
        let shas: Vec<String> = self
            .background
            .iter()
            .filter(|(_, b)| b.target == target)
            .map(|(s, _)| s.clone())
            .collect();
        for sha in shas {
            match remote {
                Remote::Folder(f) => {
                    let folder = f.join("files");
                    let Some(b) = self.background.get(&sha) else {
                        continue;
                    };
                    // copied a moment ago: look again a little later
                    if let Some(at) = b.fetch.next {
                        if now < at {
                            if let Some(c) = ctx {
                                c.request_repaint_after(at - now);
                            }
                            continue;
                        }
                    }
                    // there now, or gone from the cache (nothing to copy)
                    if folder.join(&sha).is_file() || self.local(&sha).is_none() {
                        self.background.remove(&sha);
                        self.backlog_dirty = true;
                        continue;
                    }
                    let Some(b) = self.background.get_mut(&sha) else {
                        continue;
                    };
                    if b.fetch.next.is_some() {
                        b.fetch.tries += 1;
                    }
                    b.fetch.next = Some(
                        now + Duration::from_secs(
                            RETRY_SECS[(b.fetch.tries - 1).min(RETRY_SECS.len() - 1)],
                        ),
                    );
                    let att = b.att.clone();
                    let _ = self.lane(att.kind).send(Job::ToFolder { sha, folder });
                }
                Remote::Server { addr, .. } => {
                    if self.is_on_server(addr, &sha) {
                        self.background.remove(&sha);
                        self.backlog_dirty = true;
                        continue;
                    }
                    let state = self.state(&sha);
                    if matches!(state, Some(Transfer::Up(_))) {
                        continue;
                    }
                    // gone from the cache (cleaned up): nothing to send
                    if state.is_none() && self.local(&sha).is_none() {
                        self.background.remove(&sha);
                        self.backlog_dirty = true;
                        continue;
                    }
                    let Some(b) = self.background.get_mut(&sha) else {
                        continue;
                    };
                    let f = &mut b.fetch;
                    if matches!(&state, Some(Transfer::Failed(e)) if e != NOT_SIGNED_IN) {
                        let at = *f.next.get_or_insert_with(|| {
                            now + Duration::from_secs(
                                RETRY_SECS[(f.tries - 1).min(RETRY_SECS.len() - 1)],
                            )
                        });
                        if now < at {
                            if let Some(c) = ctx {
                                c.request_repaint_after(at - now);
                            }
                            continue;
                        }
                        f.tries += 1;
                        f.next = None;
                    }
                    let att = b.att.clone();
                    self.upload_one(&att, remote);
                }
                // waits for a sign-in (upload says why)
                Remote::SignedOut(_) => {
                    if self.state(&sha).is_none() {
                        if let Some(b) = self.background.get(&sha) {
                            let att = b.att.clone();
                            self.upload_one(&att, remote);
                        }
                    }
                }
                Remote::None => {}
            }
        }
    }

    /// Not found where it's fetched from, and still looked for every few
    /// seconds: it may be on its way (see SOON_SECS).
    pub fn on_its_way(&self, sha: &str) -> bool {
        matches!(self.state(sha), Some(Transfer::Failed(e)) if not_there_yet(&e))
            && self.fetches.get(sha).is_some_and(|f| f.tries < SOON_TRIES)
    }

    /// Prepared results for the message boxes to pick up.
    pub fn take_prepared(&mut self) -> Vec<(u64, Result<Attachment, String>)> {
        #[cfg(test)]
        if self.hold_prepared {
            return vec![];
        }
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

    /// Tries so far at fetching a file, and whether the next one waits (tests).
    #[cfg(test)]
    pub fn fetch_tries(&self, sha: &str) -> Option<(usize, bool)> {
        self.fetches
            .get(sha)
            .map(|f| (f.tries, f.next.is_some_and(|n| n > Instant::now())))
    }

    /// Make a waiting retry due now (tests).
    #[cfg(test)]
    pub fn retry_due(&mut self, sha: &str) {
        if let Some(f) = self.fetches.get_mut(sha) {
            f.next = Some(Instant::now());
        }
        if let Some(b) = self.background.get_mut(sha) {
            b.fetch.next = Some(Instant::now());
        }
    }

    /// Tries so far at getting a file to the server, and whether the next
    /// one waits (tests).
    #[cfg(test)]
    pub fn upload_tries(&self, sha: &str) -> Option<(usize, bool)> {
        self.background.get(sha).map(|b| {
            (
                b.fetch.tries,
                b.fetch.next.is_some_and(|n| n > Instant::now()),
            )
        })
    }

    /// Say where a transfer is (tests: one still going up).
    #[cfg(test)]
    pub fn set_state(&self, sha: &str, t: Transfer) {
        self.states.lock().unwrap().insert(sha.to_string(), t);
    }

    /// Whether a file waits in the backlog (tests).
    #[cfg(test)]
    pub fn is_waiting(&self, sha: &str) -> bool {
        self.background.contains_key(sha)
    }

    /// Start the cleaning on a worker thread when it's due (not while the
    /// last one runs). `job` does the work there.
    pub fn clean_due(&mut self) -> bool {
        let due = match self.cleaned {
            None => self.started.elapsed() >= CLEAN_FIRST,
            Some(at) => at.elapsed() >= CLEAN_EVERY,
        };
        due && !self.cleaning.load(Ordering::SeqCst)
    }

    pub fn clean_in_background(&mut self, job: impl FnOnce(&Path) + Send + 'static) {
        self.cleaned = Some(Instant::now());
        self.cleaning.store(true, Ordering::SeqCst);
        let (dir, flag) = (self.dir.clone(), self.cleaning.clone());
        let _ = std::thread::Builder::new()
            .name("workboard-clean".into())
            .spawn(move || {
                job(&dir);
                flag.store(false, Ordering::SeqCst);
            });
    }

    /// The cache folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// The waiting uploads saved in the cache folder.
fn load_backlog(dir: &Path) -> Vec<Waiting> {
    std::fs::read_to_string(dir.join(BACKLOG))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<Waiting>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|w| wb_server_name_ok(&w.att.file))
        .collect()
}

fn save_backlog(dir: &Path, list: &[Waiting]) {
    let path = dir.join(BACKLOG);
    if list.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    if let Ok(s) = serde_json::to_string(list) {
        let tmp = dir.join(format!(".uploads-{:x}", wb_sync::random_u64()));
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// Mark a cached file as just used (the cleaning keeps the most recently
/// used ones).
fn touch(p: &Path) {
    if let Ok(f) = std::fs::File::options().write(true).open(p) {
        let _ = f.set_modified(SystemTime::now());
    }
}

/// "mov" for "Clip.MOV"; empty without one.
/// The short label a plain file's tile shows for its kind: "PDF", "MD";
/// "FILE" without an extension.
pub(super) fn ext_label(name: &str) -> String {
    let e = ext_of(name).to_ascii_uppercase();
    if e.is_empty() || e.len() > 5 {
        "FILE".into()
    } else {
        e
    }
}

/// A file's size for people: "512 B", "14 KB", "2.3 MB".
pub(super) fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{} KB", bytes.div_ceil(1024))
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// A plain file's tile in a message box or on a task: its kind label
/// with the name under it.
pub(super) fn paint_file_tile(painter: &egui::Painter, rect: Rect, name: &str, t: &Theme) {
    painter.rect_filled(rect, 8.0, t.hover);
    let small = rect.height() < 80.0;
    painter.text(
        rect.center() - vec2(0.0, if small { 8.0 } else { 10.0 }),
        Align2::CENTER_CENTER,
        ext_label(name),
        FontId::proportional(if small { 13.0 } else { 16.0 }),
        t.text2,
    );
    let room = ((rect.width() - 8.0) / if small { 5.5 } else { 6.5 }).max(4.0) as usize;
    painter.text(
        rect.center() + vec2(0.0, if small { 12.0 } else { 14.0 }),
        Align2::CENTER_CENTER,
        crate::widgets::truncate_chars(name, room),
        FontId::proportional(if small { 9.5 } else { 11.0 }),
        t.text3,
    );
}

fn ext_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_default()
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
    wb_net::name_ok(sha)
}

/// Copy `from` into the cache (or the shared folder) under its hash
/// (temporary name first). It's marked as changed now, whatever the
/// original's date: the cleaning goes by when a file was put there.
fn store(dir: &Path, from: &Path, sha: &str) -> Result<(), String> {
    if !wb_server_name_ok(sha) {
        return Err("not a file name".into());
    }
    let dest = dir.join(sha);
    if dest.is_file() {
        touch(&dest);
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!(".copy-{:x}", wb_sync::random_u64()));
    std::fs::copy(from, &tmp).map_err(|e| e.to_string())?;
    touch(&tmp);
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())
}

/// A video's still: made, put in the cache (and the shared folder) under
/// its own hash. None when the video gives no picture.
fn make_still(dir: &Path, video: &Path, folder: Option<&Path>) -> Option<String> {
    let jpeg = media::video_still_jpeg(video, media::STILL)?;
    std::fs::create_dir_all(dir).ok()?;
    let tmp = dir.join(format!(".still-{:x}.jpg", wb_sync::random_u64()));
    std::fs::write(&tmp, jpeg).ok()?;
    let out = (|| {
        let (sha, _) = wb_net::sha256_file(&tmp).ok()?;
        store(dir, &tmp, &sha).ok()?;
        if let Some(f) = folder {
            store(f, &tmp, &sha).ok()?;
        }
        Some(sha)
    })();
    let _ = std::fs::remove_file(&tmp);
    out
}

/// Check a file and get it into the cache (and the shared folder).
fn prepare(
    dir: &Path,
    path: Option<&Path>,
    png: Option<Vec<u8>>,
    name: &str,
    folder: Option<&Path>,
) -> Result<Attachment, String> {
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
        let kind = media::kind_of(&src);
        let meta = std::fs::metadata(&src).map_err(|_| format!("{} can't be read.", name))?;
        if meta.is_dir() {
            return Err(format!("{} is a folder.", name));
        }
        if meta.len() > MAX_FILE {
            return Err(format!("{} is larger than 90 MB.", name));
        }
        // a picture or video goes without where it was taken, the phone and
        // its tags (src/scrub.rs): a cleaned copy is what's measured, kept
        // and sent
        let clean = match kind {
            MediaKind::File => None,
            _ => scrubbed_copy(dir, &src, name)?,
        };
        let src = clean.as_ref().map(|c| c.0.clone()).unwrap_or(src.clone());
        // a plain file goes as it is: nothing to measure
        let (w, h, secs) = match kind {
            MediaKind::File => (0, 0, 0.0),
            _ => media::probe(&src, kind)?,
        };
        let (sha, bytes) =
            wb_net::sha256_file(&src).map_err(|_| format!("{} can't be read.", name))?;
        store(dir, &src, &sha)?;
        if let Some(f) = folder {
            store(f, &src, &sha)
                .map_err(|e| format!("Couldn't put {} in the shared folder: {}", name, e))?;
        }
        let thumb = match kind {
            MediaKind::Video => make_still(dir, &src, folder),
            MediaKind::Image | MediaKind::File => None,
        };
        Ok(Attachment {
            file: sha,
            name: name.to_string(),
            kind,
            bytes,
            w,
            h,
            secs,
            thumb,
        })
    })();
    if let Some(t) = tmp {
        let _ = std::fs::remove_file(t);
    }
    out
}

/// A copy of a picture or video with its personal details taken out, in
/// `dir`; None when there was nothing to take out. Removed when dropped.
fn scrubbed_copy(dir: &Path, src: &Path, name: &str) -> Result<Option<TempFile>, String> {
    let bytes = std::fs::read(src).map_err(|_| format!("{} can't be read.", name))?;
    let Some(clean) = crate::scrub::scrub(&bytes) else {
        return Ok(None);
    };
    if clean == bytes {
        return Ok(None);
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // the same ending, so the kind and content type stay
    let ext = src
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let t = dir.join(format!(".scrub-{:x}{}", wb_sync::random_u64(), ext));
    std::fs::write(&t, clean).map_err(|e| e.to_string())?;
    Ok(Some(TempFile(t)))
}

/// A file removed when this goes out of scope.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A files thread: one upload, download or folder copy.
fn move_file(
    dir: &Path,
    job: Job,
    states: &Mutex<HashMap<String, Transfer>>,
    cancelled: &Mutex<HashSet<String>>,
    on_server: &Mutex<HashSet<String>>,
    wake: &impl Fn(),
) {
    let set = |sha: &str, t: Transfer| {
        states.lock().unwrap().insert(sha.to_string(), t);
        wake();
    };
    let stopped = |sha: &str| cancelled.lock().unwrap().contains(sha);
    // progress at most every 1%; false once the upload was cancelled
    let report = |sha: &str, up: bool| {
        let mut last = -1i32;
        let sha = sha.to_string();
        move |done: u64, total: u64| {
            if up && stopped(&sha) {
                return false;
            }
            let pct = if total == 0 {
                100
            } else {
                (done * 100 / total) as i32
            };
            if pct != last {
                last = pct;
                let f = pct as f32 / 100.0;
                set(
                    &sha,
                    if up {
                        Transfer::Up(f)
                    } else {
                        Transfer::Down(f)
                    },
                );
            }
            true
        }
    };
    match job {
        Job::Upload {
            sha,
            ctype,
            addr,
            auth,
        } => {
            let path = dir.join(&sha);
            let result = if stopped(&sha) {
                Err(wb_net::FileError::Cancelled)
            } else {
                let mut progress = report(&sha, true);
                wb_net::upload(&addr, &auth, &sha, &path, &ctype, &mut progress)
            };
            match result {
                Ok(()) => {
                    on_server
                        .lock()
                        .unwrap()
                        .insert(format!("{}\n{}", addr, sha));
                    set(&sha, Transfer::Done)
                }
                Err(wb_net::FileError::Cancelled) => {
                    cancelled.lock().unwrap().remove(&sha);
                    states.lock().unwrap().remove(&sha);
                    wake();
                }
                Err(e) => set(&sha, Transfer::Failed(e.explain())),
            }
        }
        Job::Download { sha, addr, auth } => {
            let mut progress = report(&sha, false);
            match wb_net::download(&addr, &auth, &sha, &dir.join(&sha), &mut progress) {
                Ok(()) => {
                    on_server
                        .lock()
                        .unwrap()
                        .insert(format!("{}\n{}", addr, sha));
                    set(&sha, Transfer::Done)
                }
                Err(e) => set(&sha, Transfer::Failed(e.explain())),
            }
        }
        Job::FromFolder { sha, folder } => match store(dir, &folder.join(&sha), &sha) {
            Ok(()) => set(&sha, Transfer::Done),
            Err(_) => set(
                &sha,
                Transfer::Failed("The file isn't in the shared folder yet.".into()),
            ),
        },
        Job::ToFolder { sha, folder } => {
            let _ = store(&folder, &dir.join(&sha), &sha);
        }
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
    /// a video found not to play here (so it isn't tried every frame)
    unplayable: Option<String>,
}

impl Viewer {
    pub fn new(ch: u64, msg: u64, index: usize) -> Viewer {
        Viewer {
            ch,
            msg,
            index,
            player: None,
            frame: None,
            unplayable: None,
        }
    }
}

pub(super) const CHAT_BOX: &str = "chat_composer_text";
pub(super) const THREAD_BOX: &str = "thread_composer_text";

impl App {
    /// Where this workspace's files go besides the cache.
    pub(super) fn media_remote(&self) -> Remote {
        match &self.data.extra.shared {
            Some(crate::sync::SharedCfg {
                server: Some(addr),
                device,
                ..
            }) => match self.files.token(addr, device) {
                Some(token) => Remote::Server {
                    addr: addr.clone(),
                    auth: wb_net::Auth {
                        device: device.clone(),
                        token,
                    },
                },
                None => Remote::SignedOut(addr.clone()),
            },
            Some(cfg) if !cfg.folder.is_empty() => Remote::Folder(PathBuf::from(&cfg.folder)),
            _ => Remote::None,
        }
    }

    /// Once a frame: background work, prepared attachments, a paste.
    pub(super) fn files_tick(&mut self, ctx: Option<&egui::Context>) {
        let remote = self.media_remote();
        // before the tick, which takes the files asked for
        self.save_fetched_files();
        self.want_profile_photos();
        self.files.tick(ctx, &remote);
        if let Some(c) = ctx {
            self.publish_profile_photos(c);
        }
        for (local, result) in self.files.take_prepared() {
            self.profile_photo_prepared(local, &result, &remote);
            let boxes = [&mut self.chat.files, &mut self.chat.thread_files]
                .into_iter()
                .chain(self.drafts.chat_files.saved.values_mut())
                .chain(self.drafts.thread_files.saved.values_mut());
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
            self.follow_prepared(local, &result, &remote);
            if let Some((_, p)) = self.task_files.iter_mut().find(|(_, p)| p.local == local) {
                match &result {
                    Ok(a) => {
                        // into the backlog too: tried again after a failure
                        // (waiting longer each time), carried on after a
                        // restart, and sent once there's a sign-in
                        self.files.upload(a, &remote);
                        self.files.upload_soon(a, &remote);
                        p.att = Some(a.clone());
                    }
                    Err(e) => p.error = Some(e.clone()),
                }
            }
        }
        self.restore_task_files(&remote);
        self.attach_ready_task_files();
        self.save_task_files();
        self.clean_tick();
        // attachments that waited for a sign-in go up once there is one
        if matches!(remote, Remote::Server { .. }) {
            let waiting: Vec<Attachment> = self.boxed_attachments().into_iter().filter(|a| matches!(self.files.state(&a.file), Some(Transfer::Failed(e)) if e == NOT_SIGNED_IN)).collect();
            for a in waiting {
                self.files.upload(&a, &remote);
            }
        }
        if let Some(p) = self.pasted.take() {
            if self.section == Section::Chat && self.chat.open.is_some() {
                let thread = ctx
                    .is_some_and(|c| c.memory(|m| m.focused()) == Some(egui::Id::new(THREAD_BOX)))
                    && self.chat.thread_open.is_some();
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

    /// The attachments in every message box (this one and the ones kept
    /// with other channels).
    fn boxed_attachments(&self) -> Vec<Attachment> {
        let boxes = [&self.chat.files, &self.chat.thread_files]
            .into_iter()
            .chain(self.drafts.chat_files.saved.values())
            .chain(self.drafts.thread_files.saved.values());
        boxes
            .flat_map(|l| l.iter().filter_map(|p| p.att.clone()))
            .chain(self.task_files.iter().filter_map(|(_, p)| p.att.clone()))
            .collect()
    }

    /// Whether a sent message, a task (or anything else in the workspace,
    /// the Trash too) or a message box still has this file.
    pub(super) fn file_in_use(&self, sha: &str) -> bool {
        let data = serde_json::to_value(&self.data).unwrap_or_default();
        shas_in(&data).contains(sha)
            || self
                .boxed_attachments()
                .iter()
                .any(|a| parts(a).iter().any(|p| p.file == sha))
    }

    /// Now and then: tidy the cache (and the shared folder's files/) on a
    /// worker thread. Only for the real data folder.
    fn clean_tick(&mut self) {
        if !self.persist || !self.files.clean_due() {
            return;
        }
        let current = SpaceRefs::of(&serde_json::to_value(&self.data).unwrap_or_default());
        let keep: HashSet<String> = self
            .files
            .waiting()
            .cloned()
            .chain(
                self.boxed_attachments()
                    .iter()
                    // task attachments on their way in other workspaces too
                    .chain(
                        super::taskfields::read_task_files(self.files.dir())
                            .iter()
                            .map(|e| &e.att),
                    )
                    .flat_map(parts)
                    .map(|p| p.file),
            )
            .collect();
        let folder = match self.media_remote() {
            Remote::Folder(f) => Some(f),
            _ => None,
        };
        self.files.clean_in_background(move |dir| {
            let mut spaces = workspaces_on_disk(&Data::dir());
            let now = SystemTime::now();
            if let Some(f) = folder {
                let refs = folder_refs(&f).map(|mut r| {
                    r.extend(current.all.iter().cloned());
                    r
                });
                clean_shared_folder(&f.join("files"), &refs, now, FOLDER_GRACE);
            }
            spaces.push(current);
            clean_cache(dir, &spaces, &keep, now, CACHE_GRACE, CACHE_CAP);
        });
    }

    /// Sharing just started: the pictures and videos anything in the
    /// workspace refers to (messages and replies, tasks, the Trash ...; the
    /// ones this computer has), and the ones waiting in message boxes (which
    /// were ready with nowhere to go), go where the team can get them;
    /// failed uploads are tried again. The list is kept on disk, so a
    /// restart carries on with it.
    pub(super) fn share_cached_files(&mut self) {
        let remote = self.media_remote();
        let mut seen = HashSet::new();
        let mut atts = vec![];
        attachments_in(
            &serde_json::to_value(&self.data).unwrap_or_default(),
            &mut atts,
        );
        atts.extend(self.boxed_attachments());
        for a in atts.iter().filter(|a| seen.insert(a.file.clone())) {
            self.files.upload_soon(a, &remote);
        }
    }

    /// Items came back from Trash: their pictures and videos (the ones this
    /// computer has) go where the team can get them again.
    pub(super) fn reupload_files(&mut self, atts: &[Attachment]) {
        if atts.is_empty() {
            return;
        }
        let remote = self.media_remote();
        for a in atts {
            self.files.reupload(a, &remote);
        }
    }

    /// A file a sent message waited for is ready: on the message it goes,
    /// and up to the server after it. A message of nothing but files that
    /// weren't ready goes out with the first that is. One that can't be
    /// sent is said so.
    fn follow_prepared(
        &mut self,
        local: u64,
        result: &Result<Attachment, String>,
        remote: &Remote,
    ) {
        if let Some(i) = self
            .chat
            .held
            .iter()
            .position(|h| h.coming.iter().any(|p| p.local == local))
        {
            let mut h = self.chat.held.remove(i);
            let p = h.coming.iter().position(|p| p.local == local).unwrap();
            let p = h.coming.remove(p);
            match result {
                Ok(a) => {
                    self.files.upload(a, remote);
                    // the box's own state stays as it is for what's written next
                    let also = std::mem::replace(&mut self.chat.also_channel, h.also_channel);
                    let coming = std::mem::replace(
                        if h.parent.is_none() {
                            &mut self.chat.coming
                        } else {
                            &mut self.chat.thread_coming
                        },
                        h.coming,
                    );
                    self.apply_chat(vec![super::chat::ChatAction::Send {
                        ch: h.ch,
                        text: String::new(),
                        parent: h.parent,
                        files: vec![a.clone()],
                    }]);
                    self.chat.also_channel = also;
                    if h.parent.is_none() {
                        self.chat.coming = coming;
                    } else {
                        self.chat.thread_coming = coming;
                    }
                }
                Err(e) => {
                    self.toast(format!("{}: {}", p.name, App::error_text(e)));
                    if !h.coming.is_empty() {
                        self.chat.held.insert(i, h);
                    }
                }
            }
            return;
        }
        let Some(i) = self
            .chat
            .following
            .iter()
            .position(|(_, _, p)| p.local == local)
        else {
            return;
        };
        let (ch, ids, p) = self.chat.following.remove(i);
        let a = match result {
            Ok(a) => a,
            Err(e) => {
                // the message has its words or another file: it stays
                self.toast(format!("{}: {}", p.name, App::error_text(e)));
                return;
            }
        };
        self.files.upload(a, remote);
        self.files.upload_soon(a, remote);
        let put = |m: &mut Message| {
            if ids.contains(&m.id) && !m.attachments.iter().any(|x| x.file == a.file) {
                m.attachments.push(a.clone());
                true
            } else {
                false
            }
        };
        let mut changed = false;
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
            for m in c.messages.iter_mut() {
                changed |= put(m);
                for r in m.replies.iter_mut() {
                    changed |= put(r);
                }
            }
        }
        self.dirty |= changed;
    }

    /// A message is going out: make sure its pictures and videos are where
    /// the team can get them (one put in the box before sharing started was
    /// ready without going anywhere).
    pub(super) fn send_files(&mut self, files: &[Attachment]) {
        if files.is_empty() {
            return;
        }
        let remote = self.media_remote();
        for a in files {
            self.files.upload_soon(a, &remote);
        }
    }

    /// Error text from the files and media threads, in the interface
    /// language (some carry a file name or a reason inside).
    pub(super) fn error_text(e: &str) -> String {
        if e.starts_with("Couldn't reach the server") {
            // the system's own words would follow, untranslated
            return tr("Can't reach the team server right now. It's tried again by itself.").into();
        }
        const WITH_VALUES: [&str; 5] = [
            "{} is a folder.",
            "{} can't be read.",
            "{} is larger than 90 MB.",
            "Couldn't put {} in the shared folder: {}",
            "The server said {} {}",
        ];
        for pattern in WITH_VALUES {
            if let Some(mut values) = fill_back(pattern, e) {
                if pattern == "The server said {} {}" {
                    if let Some(body) = values.get_mut(1) {
                        *body = server_body(body);
                    }
                }
                return crate::i18n::fill(tr(pattern), &values);
            }
        }
        tr(e).to_string()
    }

    /// Put a file (or a pasted picture) in the channel or thread message box.
    /// At most MAX_PER_MESSAGE per box: past that it says so and leaves the
    /// file out (false).
    pub(super) fn add_media(
        &mut self,
        thread: bool,
        path: Option<PathBuf>,
        png: Option<Vec<u8>>,
    ) -> bool {
        let count = if thread {
            self.chat.thread_files.len()
        } else {
            self.chat.files.len()
        };
        if count >= MAX_PER_MESSAGE {
            self.toast(trf!(
                "You can send up to {} files in one message.",
                MAX_PER_MESSAGE
            ));
            return false;
        }
        let remote = self.media_remote();
        let p = self.files.add(path, png, &remote);
        if thread {
            self.chat.thread_files.push(p);
        } else {
            self.chat.files.push(p);
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
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        let at = drag_pointer(ctx);
        // over an open task they belong to the task (see task_window)
        if dropped.is_empty() || self.open_task.is_some() {
            return;
        }
        let thread = self.chat.thread_open.is_some()
            && thread_rect.zip(at).is_some_and(|(r, p)| r.contains(p));
        for f in dropped {
            self.add_media(thread, Some(f), None);
        }
    }

    /// Before egui sees the input (eframe's raw_input_hook): the input
    /// method fixes, then a paste of files or a picture.
    pub(super) fn fix_input(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // every text box: a syllable left open in one isn't typed into the next
        if self.ime.ends_syllable(ctx.memory(|m| m.focused()), raw) {
            crate::os::end_composition();
        }
        if self.ime.fix(raw) {
            ctx.request_repaint();
        }
        if !self.ime.woken && raw.focused {
            self.ime.woken = crate::os::wake_input_method();
            self.ime.drop_wake = self.ime.woken;
        }
        self.catch_paste(ctx, raw);
    }

    /// Before egui sees the input: a Cmd+V in a message box with files or a
    /// picture on the clipboard. egui only passes on text, so a picture alone
    /// shows up as nothing but the release of V.
    fn catch_paste(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let v_up = self.paste_key_up(raw);
        let chat_box = egui::Id::new(CHAT_BOX);
        let focused = ctx.memory(|m| m.focused());
        if focused != Some(chat_box) && focused != Some(egui::Id::new(THREAD_BOX)) {
            // Nothing to type in has the keyboard (say a click on the
            // messages after taking a screenshot in another app): the paste
            // goes to the open channel's message box, as in Slack.
            let typing_elsewhere =
                focused.is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
            let pasting = v_up
                || raw
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Paste(_)));
            if !pasting || typing_elsewhere || !self.chat_box_shown(ctx) {
                self.paste_text_seen = false;
                return;
            }
            // the box takes this frame's pasted text itself
            ctx.memory_mut(|m| m.request_focus(chat_box));
        }
        if raw
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::Paste(_)))
        {
            self.paste_text_seen = true;
            // files copied in Finder come with their names as text: send the files instead
            let p = media::pasteboard();
            if !p.files.is_empty() {
                raw.events.retain(|e| !matches!(e, egui::Event::Paste(_)));
                self.pasted = Some(p);
            }
        }
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

    /// Whether this frame ends a Cmd+V (Ctrl+V on Windows). egui keeps the
    /// V going down to itself, so only its release shows, and when Cmd was
    /// let go first that release has no Cmd on it: a V released without
    /// having gone down as typing, while Cmd was held, counts too.
    fn paste_key_up(&mut self, raw: &egui::RawInput) -> bool {
        let mut up = false;
        for e in &raw.events {
            let egui::Event::Key {
                key,
                pressed,
                modifiers,
                ..
            } = e
            else {
                continue;
            };
            if modifiers.command {
                self.paste_cmd = true;
            }
            if *key == egui::Key::V {
                if *pressed {
                    self.v_typed = true;
                } else {
                    up |= modifiers.command || (self.paste_cmd && !self.v_typed);
                    self.v_typed = false;
                    self.paste_cmd = false;
                }
            } else if *pressed && !modifiers.command {
                self.paste_cmd = false;
            }
        }
        if raw.modifiers.command {
            self.paste_cmd = true;
        }
        up
    }

    /// The open channel's message box was on screen last frame, with
    /// nothing (an open task) over it.
    fn chat_box_shown(&self, ctx: &egui::Context) -> bool {
        self.section == Section::Chat
            && self.chat.open.is_some()
            && self.open_task.is_none()
            && ctx.read_response(egui::Id::new(CHAT_BOX)).is_some()
    }

    /// Attachments waiting in a message box, as small tiles with a remove
    /// button and how far the upload is.
    pub(super) fn pending_strip(&mut self, ui: &mut Ui, thread: bool) {
        let list = if thread {
            &self.chat.thread_files
        } else {
            &self.chat.files
        };
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
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Image,
                        true,
                        format!("Attachment {}", p.name),
                    )
                });
                // a click only means something on a failed upload (try again)
                if failed.is_none() || p.error.is_some() {
                    no_pointer(&resp);
                }
                let painter = ui.painter_at(rect.expand(6.0));
                let plain = media::kind_of(Path::new(&p.name)) == MediaKind::File;
                match p.att.as_ref().filter(|_| !plain).and_then(|a| {
                    self.files.want(a, THUMB);
                    self.files.texture(&a.file, THUMB)
                }) {
                    _ if plain => paint_file_tile(&painter, rect, &p.name, &t),
                    Some(tex) => {
                        let uv = cover_uv(tex.size_vec2(), size);
                        painter.add(
                            egui::epaint::RectShape::filled(rect, 8.0, Color32::WHITE)
                                .with_texture(tex.id(), uv),
                        );
                    }
                    None => {
                        painter.rect_filled(rect, 8.0, t.hover);
                        painter.text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            if p.att.is_none() && failed.is_none() {
                                "..."
                            } else {
                                ""
                            },
                            FontId::proportional(14.0),
                            t.text3,
                        );
                    }
                }
                if p.att.as_ref().is_some_and(|a| a.kind == MediaKind::Video) {
                    play_badge(&painter, rect.center(), 12.0);
                }
                if let Some(Transfer::Up(f)) = state {
                    upload_bar(&painter, rect, f);
                }
                if let Some(why) = &failed {
                    // waiting for the server isn't a failure: the message can go
                    let waits = p.error.is_none() && waits_for_server(why);
                    let c = if waits {
                        super::together::WARN
                    } else {
                        t.danger
                    };
                    painter.rect_stroke(rect, 8.0, Stroke::new(2.0_f32, c), egui::StrokeKind::Inside);
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "!",
                        FontId::proportional(18.0),
                        c,
                    );
                    let mut tip = App::error_text(why);
                    if waits {
                        tip = format!(
                            "{}\n{}",
                            tip,
                            tr("You can send the message now. The file follows once the server is back.")
                        );
                    }
                    if p.error.is_none() && why != NOT_SIGNED_IN {
                        tip = format!("{}\n{}", tip, tr("Click to try again"));
                    }
                    let r = resp.clone().on_hover_text(tip);
                    if r.clicked() && p.error.is_none() {
                        retry = p.att.as_ref().map(|a| a.clone());
                    }
                } else {
                    resp.clone().on_hover_text(&p.name);
                }
                // remove
                let x =
                    Rect::from_center_size(rect.right_top() + vec2(-8.0, 8.0), vec2(18.0, 18.0));
                let xr = ui.interact(x, ui.id().with(("unattach", p.local)), Sense::click());
                xr.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        format!("Remove {}", p.name),
                    )
                });
                painter.circle_filled(
                    x.center(),
                    9.0,
                    if xr.hovered() {
                        Color32::from_black_alpha(220)
                    } else {
                        Color32::from_black_alpha(160)
                    },
                );
                for (a, b) in [
                    (vec2(-3.5, -3.5), vec2(3.5, 3.5)),
                    (vec2(-3.5, 3.5), vec2(3.5, -3.5)),
                ] {
                    painter.line_segment(
                        [x.center() + a, x.center() + b],
                        Stroke::new(1.6_f32, Color32::WHITE),
                    );
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
            let list = if thread {
                &mut self.chat.thread_files
            } else {
                &mut self.chat.files
            };
            let gone = list
                .iter()
                .find(|p| p.local == l)
                .and_then(|p| p.att.clone());
            list.retain(|p| p.local != l);
            // stop its upload, unless another message box, a sent message or
            // a task has the same file
            if let Some(a) = gone {
                if !self.file_in_use(&a.file) {
                    self.files.cancel_attachment(&a);
                }
            }
        }
    }

    /// Whether a message box may send, and the attachments it would send.
    /// Ones still being checked and copied don't hold it up: they follow
    /// the message (`pending_coming`).
    pub(super) fn pending_ready(&self, thread: bool) -> (bool, Vec<Attachment>) {
        let list = if thread {
            &self.chat.thread_files
        } else {
            &self.chat.files
        };
        let ok = list
            .iter()
            .all(|p| p.sendable(&self.files) || p.getting_ready());
        (
            ok,
            if ok {
                list.iter().filter_map(|p| p.att.clone()).collect()
            } else {
                vec![]
            },
        )
    }

    /// The attachments in a message box still being checked and copied:
    /// they're put on the message once they're ready.
    pub(super) fn pending_coming(&self, thread: bool) -> Vec<Pending> {
        let list = if thread {
            &self.chat.thread_files
        } else {
            &self.chat.files
        };
        list.iter().filter(|p| p.getting_ready()).cloned().collect()
    }

    /// A message's pictures and videos. Returns the one clicked.
    pub(super) fn attachment_tiles(&self, ui: &mut Ui, m: &Message) -> Option<usize> {
        // files sent before they were ready, still being copied
        let coming: Vec<&Pending> = self
            .chat
            .following
            .iter()
            .filter(|(_, ids, _)| ids.contains(&m.id))
            .map(|(_, _, p)| p)
            .collect();
        if m.attachments.is_empty() && coming.is_empty() {
            return None;
        }
        let t = self.t();
        let mut clicked = None;
        let one = m.attachments.len() + coming.len() == 1;
        let avail = ui.available_width();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for (i, a) in m.attachments.iter().enumerate() {
                if a.kind == MediaKind::File {
                    if self.file_card(ui, a, avail) {
                        clicked = Some(i);
                    }
                    continue;
                }
                let aspect = if a.w > 0 && a.h > 0 {
                    a.w as f32 / a.h as f32
                } else {
                    4.0 / 3.0
                };
                let size = if one {
                    let w = (320.0 * aspect).min(360.0).min(avail).max(80.0);
                    vec2(w, (w / aspect).min(320.0))
                } else {
                    let h = 150.0;
                    vec2((h * aspect).clamp(100.0, 260.0), h)
                };
                let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                let label = format!(
                    "{} {}",
                    if a.kind == MediaKind::Video {
                        "Video"
                    } else {
                        "Picture"
                    },
                    a.name
                );
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Image, true, label.clone())
                });
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                // a video with a still shows the still; the video itself is
                // fetched when it's played
                let s = shown(a);
                self.files.want(&s, THUMB);
                let painter = ui.painter_at(rect);
                match self.files.texture(&s.file, THUMB) {
                    Some(tex) => {
                        let uv = cover_uv(tex.size_vec2(), size);
                        painter.add(
                            egui::epaint::RectShape::filled(rect, 8.0, Color32::WHITE)
                                .with_texture(tex.id(), uv),
                        );
                    }
                    None => {
                        painter.rect_filled(rect, 8.0, t.hover);
                        let status = match self.files.state(&s.file) {
                            Some(Transfer::Down(f)) => trf!("Loading {}%", (f * 100.0) as u32),
                            _ if self.files.on_its_way(&s.file) => tr("Loading...").into(),
                            Some(Transfer::Failed(e)) => {
                                format!("{}\n{}", App::error_text(&e), tr("Click to try again"))
                            }
                            _ if self.files.broken(&s.file, THUMB) => {
                                tr("Can't show this file").into()
                            }
                            _ => a.name.clone(),
                        };
                        painter.text(
                            rect.center()
                                + vec2(
                                    0.0,
                                    if a.kind == MediaKind::Video {
                                        26.0
                                    } else {
                                        0.0
                                    },
                                ),
                            Align2::CENTER_CENTER,
                            status,
                            FontId::proportional(12.0),
                            t.text3,
                        );
                    }
                }
                if a.kind == MediaKind::Video {
                    play_badge(&painter, rect.center(), 20.0);
                    if a.secs > 0.0 {
                        let txt = media::clock(a.secs);
                        let g =
                            painter.layout_no_wrap(txt, FontId::proportional(11.5), Color32::WHITE);
                        let r = Rect::from_min_size(
                            rect.right_bottom() - g.size() - vec2(14.0, 10.0),
                            g.size() + vec2(8.0, 4.0),
                        );
                        painter.rect_filled(r, 4.0, Color32::from_black_alpha(160));
                        painter.galley(r.min + vec2(4.0, 2.0), g, Color32::WHITE);
                    }
                }
                // mine, still going up: how far
                if let Some(Transfer::Up(f)) = self.files.state(&a.file) {
                    upload_bar(&painter, rect, f);
                }
                if resp.clicked() {
                    clicked = Some(i);
                }
            }
            for p in coming {
                let plain = media::kind_of(Path::new(&p.name)) == MediaKind::File;
                let size = if plain {
                    vec2(260.0_f32.min(avail).max(160.0), 52.0)
                } else if one {
                    vec2(240.0_f32.min(avail).max(80.0), 180.0)
                } else {
                    vec2(200.0, 150.0)
                };
                let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Image,
                        true,
                        format!("Sending {}", p.name),
                    )
                });
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 8.0, t.hover);
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!(
                        "{}\n{}",
                        crate::widgets::truncate_chars(&p.name, 28),
                        tr("Getting ready...")
                    ),
                    FontId::proportional(12.0),
                    t.text3,
                );
            }
        });
        clicked
    }

    /// A plain file in a message: its kind, name and size, or how its
    /// download is going. True when clicked (to save it).
    fn file_card(&self, ui: &mut Ui, a: &Attachment, avail: f32) -> bool {
        let t = self.t();
        let size = vec2(260.0_f32.min(avail).max(160.0), 52.0);
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let label = format!("File {}", a.name);
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
        });
        if !ui.is_rect_visible(rect) {
            return resp.clicked();
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8.0, if resp.hovered() { t.hover } else { t.card });
        painter.rect_stroke(
            rect,
            8.0,
            Stroke::new(1.0_f32, t.border),
            egui::StrokeKind::Inside,
        );
        let icon = Rect::from_min_size(rect.min + vec2(8.0, 8.0), vec2(36.0, 36.0));
        painter.rect_filled(icon, 6.0, t.accent_bg);
        painter.text(
            icon.center(),
            Align2::CENTER_CENTER,
            ext_label(&a.name),
            FontId::proportional(if ext_label(&a.name).len() > 3 {
                9.5
            } else {
                11.0
            }),
            t.accent_text,
        );
        let left = icon.right() + 10.0;
        let room = ((rect.right() - left - 8.0) / 6.5).max(4.0) as usize;
        painter.text(
            pos2(left, rect.top() + 17.0),
            Align2::LEFT_CENTER,
            crate::widgets::truncate_chars(&a.name, room),
            FontId::proportional(13.0),
            t.text,
        );
        let saving = self.file_saves.iter().any(|s| s.file == a.file);
        let (status, bad) = match self.files.state(&a.file) {
            Some(Transfer::Down(f)) => (trf!("Loading {}%", (f * 100.0) as u32), false),
            Some(Transfer::Up(f)) => (trf!("Sending {}%", (f * 100.0) as u32), false),
            _ if self.files.on_its_way(&a.file) => (tr("Loading...").to_string(), false),
            Some(Transfer::Failed(e)) => (
                format!("{} {}", App::error_text(&e), tr("Click to try again")),
                true,
            ),
            _ if saving => (tr("Loading...").to_string(), false),
            _ => (size_text(a.bytes), false),
        };
        painter.text(
            pos2(left, rect.top() + 36.0),
            Align2::LEFT_CENTER,
            crate::widgets::truncate_chars(&status, room + 4),
            FontId::proportional(11.5),
            if bad { t.danger } else { t.text3 },
        );
        resp.on_hover_text(tr("Click to save to Downloads"))
            .clicked()
    }

    /// Save a plain file to Downloads: now when it's here, else once it's
    /// fetched (see `save_fetched_files`).
    pub(super) fn save_file(&mut self, a: &Attachment) {
        if matches!(self.files.state(&a.file), Some(Transfer::Failed(_))) {
            self.files.retry(&a.file);
        }
        match self.files.local(&a.file) {
            Some(path) => self.save_to_downloads(&path, &a.name),
            None => {
                if !self.file_saves.iter().any(|s| s.file == a.file) {
                    self.file_saves.push(a.clone());
                }
                self.files.fetch(a);
            }
        }
    }

    /// Once a frame: files asked to be saved that have come in are saved;
    /// the rest are asked for again.
    pub(super) fn save_fetched_files(&mut self) {
        if self.file_saves.is_empty() {
            return;
        }
        let saves = std::mem::take(&mut self.file_saves);
        for a in saves {
            match self.files.local(&a.file) {
                Some(path) => self.save_to_downloads(&path, &a.name),
                None => {
                    self.files.fetch(&a);
                    self.file_saves.push(a);
                }
            }
        }
    }

    fn save_to_downloads(&mut self, path: &Path, name: &str) {
        match save_copy(path, name) {
            Ok(dest) => self.toast(trf!(
                "Saved {} in Downloads",
                dest.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            )),
            Err(e) => self.toast = Some((trf!("Couldn't save: {}", e), now_ts())),
        }
    }

    /// The message a viewer shows.
    fn viewer_message(&self, v: &Viewer) -> Option<Message> {
        let c = self.data.channels.iter().find(|c| c.id == v.ch)?;
        c.messages
            .iter()
            .flat_map(|m| std::iter::once(m).chain(m.replies.iter()))
            .find(|m| m.id == v.msg)
            .cloned()
    }

    /// A picture or video opened large, over everything.
    pub(super) fn media_viewer(&mut self, ctx: &egui::Context) {
        let Some(mut v) = self.viewer.take() else {
            return;
        };
        let Some(mut m) = self.viewer_message(&v) else {
            return;
        };
        // only the pictures and videos: plain files are saved, not shown;
        // from here on the index counts those
        let media: Vec<usize> = (0..m.attachments.len())
            .filter(|&i| m.attachments[i].kind != MediaKind::File)
            .collect();
        if media.is_empty() {
            return;
        }
        v.index = media.iter().position(|&i| i == v.index).unwrap_or(0);
        m.attachments = media.iter().map(|&i| m.attachments[i].clone()).collect();
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
        let still = still_of(&a);
        if a.kind == MediaKind::Image {
            self.files.want(&a, FULL);
            self.files.want(&a, THUMB);
        } else if let Some(s) = &still {
            // the still shows while the video comes
            self.files.want(s, THUMB);
        } else if local.is_none() {
            self.files.want(&a, THUMB);
        }
        // the video player follows the file shown
        if a.kind == MediaKind::Video {
            if v.player.as_ref().map(|(s, _)| s != &a.file).unwrap_or(true)
                && v.unplayable.as_deref() != Some(a.file.as_str())
            {
                v.player = None;
                v.frame = None;
                if let Some(path) = self.files.playable(&a).filter(|_| local.is_some()) {
                    let path = &path;
                    let (w, h) = fit(a.w.max(1) as f32, a.h.max(1) as f32, 1280.0, 1280.0);
                    match media::Player::new(path, w as u32, h as u32) {
                        Some(mut p) => {
                            p.play();
                            v.player = Some((a.file.clone(), p));
                        }
                        None => v.unplayable = Some(a.file.clone()),
                    }
                }
            }
            // it opened but AVFoundation gave up on it: say so instead of a dead Play button
            if v.player
                .as_ref()
                .is_some_and(|(s, p)| s == &a.file && p.failed())
            {
                v.player = None;
                v.frame = None;
                v.unplayable = Some(a.file.clone());
            }
            if let Some((_, p)) = v.player.as_mut() {
                if let Some(f) = p.frame() {
                    match v.frame.as_mut() {
                        Some(tex) => tex.set(f.to_color_image(), egui::TextureOptions::LINEAR),
                        None => {
                            v.frame = Some(ctx.load_texture(
                                "media-player",
                                f.to_color_image(),
                                egui::TextureOptions::LINEAR,
                            ))
                        }
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

        egui::Area::new(egui::Id::new("media_viewer"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (bg, bg_resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                crate::widgets::no_pointer(&bg_resp);
                ui.painter()
                    .rect_filled(bg, 0.0, Color32::from_black_alpha(225));
                let bar_h = 56.0;
                let area = Rect::from_min_max(
                    bg.min + vec2(60.0, 50.0),
                    bg.max - vec2(60.0, bar_h + 20.0),
                );
                // the picture or frame, fitted
                let tex = match a.kind {
                    MediaKind::Image => self
                        .files
                        .texture(&a.file, FULL)
                        .or_else(|| self.files.texture(&a.file, THUMB))
                        .cloned(),
                    MediaKind::File => None,
                    MediaKind::Video => v.frame.clone().or_else(|| {
                        still
                            .as_ref()
                            .and_then(|s| self.files.texture(&s.file, THUMB))
                            .or_else(|| self.files.texture(&a.file, THUMB))
                            .cloned()
                    }),
                };
                let (w, h) = fit(
                    a.w.max(1) as f32,
                    a.h.max(1) as f32,
                    area.width(),
                    area.height(),
                );
                let pic = Rect::from_center_size(area.center(), vec2(w, h));
                let pic_resp = ui.interact(pic, ui.id().with("media_pic"), Sense::click());
                pic_resp.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Image,
                        true,
                        format!("Large {}", a.name),
                    )
                });
                // the picture takes a click only to retry a failed download
                let retryable =
                    tex.is_none() && matches!(self.files.state(&a.file), Some(Transfer::Failed(_)));
                if !retryable {
                    no_pointer(&pic_resp);
                }
                match &tex {
                    Some(tex) => {
                        ui.painter().image(
                            tex.id(),
                            pic,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    None => {
                        let failed = matches!(self.files.state(&a.file), Some(Transfer::Failed(_)));
                        if failed && pic_resp.clicked() {
                            self.files.retry(&a.file);
                        }
                        let status = match self.files.state(&a.file) {
                            Some(Transfer::Down(f)) => trf!("Loading {}%", (f * 100.0) as u32),
                            _ if self.files.on_its_way(&a.file) => tr("Loading...").into(),
                            Some(Transfer::Failed(e)) => {
                                format!("{}\n{}", App::error_text(&e), tr("Click to try again"))
                            }
                            _ => tr("Loading...").into(),
                        };
                        ui.painter().text(
                            pic.center(),
                            Align2::CENTER_CENTER,
                            status,
                            FontId::proportional(15.0),
                            Color32::from_gray(200),
                        );
                    }
                }
                if bg_resp.clicked()
                    && !pic.contains(bg_resp.interact_pointer_pos().unwrap_or(pic.center()))
                {
                    close = true;
                }
                // top: name and the buttons
                let top =
                    Rect::from_min_size(bg.min + vec2(20.0, 12.0), vec2(bg.width() - 40.0, 30.0));
                ui.allocate_new_ui(
                    egui::UiBuilder::new()
                        .max_rect(top)
                        .layout(Layout::left_to_right(Align::Center)),
                    |ui| {
                        let count = if m.attachments.len() > 1 {
                            trf!("  {} of {}", v.index + 1, m.attachments.len())
                        } else {
                            String::new()
                        };
                        ui.label(
                            RichText::new(format!("{}{}", a.name, count))
                                .color(Color32::WHITE)
                                .size(14.0),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let b = |ui: &mut Ui, s: &str| {
                                ui.add(Tint(
                                    egui::Button::new(RichText::new(s).color(Color32::WHITE))
                                        .fill(Color32::from_white_alpha(28))
                                        .corner_radius(6)
                                        .min_size(vec2(0.0, 28.0)),
                                ))
                            };
                            if b(ui, tr("Close")).on_hover_text("Esc").clicked() {
                                close = true;
                            }
                            if let Some(path) = &local {
                                if b(ui, tr("Save to Downloads")).clicked() {
                                    match save_copy(path, &a.name) {
                                        Ok(dest) => self.toast(trf!(
                                            "Saved {}",
                                            dest.file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_default()
                                        )),
                                        Err(e) => {
                                            self.toast =
                                                Some((trf!("Couldn't save: {}", e), now_ts()))
                                        }
                                    }
                                }
                                let reveal = if cfg!(windows) {
                                    tr("Show in File Explorer")
                                } else {
                                    tr("Show in Finder")
                                };
                                if b(ui, reveal).clicked() {
                                    crate::os::reveal_path(path);
                                }
                                // a video the app can't play (every video
                                // outside macOS) opens in the system's player
                                if a.kind == MediaKind::Video
                                    && v.unplayable.as_deref() == Some(a.file.as_str())
                                    && b(ui, tr("Open in another app")).clicked()
                                {
                                    if let Some(p) = self.files.playable(&a) {
                                        if !cfg!(test) {
                                            crate::os::open_path(p);
                                        }
                                    }
                                }
                            }
                        });
                    },
                );
                // previous / next
                if m.attachments.len() > 1 {
                    for (dir, x, glyph) in [
                        (-1, bg.left() + 30.0, "\u{2039}"),
                        (1, bg.right() - 30.0, "\u{203A}"),
                    ] {
                        let r = Rect::from_center_size(pos2(x, area.center().y), vec2(40.0, 60.0));
                        let resp =
                            ui.interact(r, ui.id().with(("media_step", dir)), Sense::click());
                        resp.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                true,
                                if dir < 0 { "Previous" } else { "Next" },
                            )
                        });
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            glyph,
                            FontId::proportional(40.0),
                            if resp.hovered() {
                                Color32::WHITE
                            } else {
                                Color32::from_gray(170)
                            },
                        );
                        if resp.clicked() {
                            step = dir;
                        }
                    }
                }
                // video controls
                if a.kind == MediaKind::Video {
                    let bar = Rect::from_min_size(
                        pos2(area.center().x - 300.0, bg.bottom() - bar_h - 12.0),
                        vec2(600.0, bar_h),
                    );
                    ui.painter()
                        .rect_filled(bar, 10.0, Color32::from_white_alpha(22));
                    if let Some((_, p)) = v.player.as_mut() {
                        let (pos, dur) = (p.position(), p.duration().max(a.secs).max(0.001));
                        ui.allocate_new_ui(
                            egui::UiBuilder::new()
                                .max_rect(bar.shrink2(vec2(12.0, 0.0)))
                                .layout(Layout::left_to_right(Align::Center)),
                            |ui| {
                                let playing = p.playing();
                                let pb = ui.add(ghost(
                                    egui::Button::new(
                                        RichText::new(if playing {
                                            "\u{23F8}"
                                        } else {
                                            "\u{25B6}"
                                        })
                                        .color(Color32::WHITE)
                                        .size(16.0),
                                    )
                                    .min_size(vec2(32.0, 32.0)),
                                ));
                                pb.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        if playing { "Pause" } else { "Play" },
                                    )
                                });
                                if pb.clicked() || ui.input(|i| i.key_pressed(egui::Key::Space)) {
                                    if playing {
                                        p.pause();
                                    } else {
                                        p.play();
                                    }
                                }
                                ui.label(
                                    RichText::new(media::clock(pos))
                                        .color(Color32::WHITE)
                                        .monospace()
                                        .size(12.0),
                                );
                                // the bar: click or drag to move
                                let track_w = ui.available_width() - 110.0;
                                let (tr, tresp) = ui.allocate_exact_size(
                                    vec2(track_w.max(40.0), 24.0),
                                    Sense::click_and_drag(),
                                );
                                tresp.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Slider,
                                        true,
                                        "Video position",
                                    )
                                });
                                let line =
                                    Rect::from_center_size(tr.center(), vec2(tr.width(), 4.0));
                                ui.painter()
                                    .rect_filled(line, 2.0, Color32::from_white_alpha(60));
                                let f = (pos / dur).clamp(0.0, 1.0);
                                ui.painter().rect_filled(
                                    Rect::from_min_size(line.min, vec2(line.width() * f, 4.0)),
                                    2.0,
                                    Color32::WHITE,
                                );
                                ui.painter().circle_filled(
                                    pos2(line.left() + line.width() * f, line.center().y),
                                    if tresp.hovered() || tresp.dragged() {
                                        7.0
                                    } else {
                                        5.0
                                    },
                                    Color32::WHITE,
                                );
                                if let Some(at) = tresp
                                    .interact_pointer_pos()
                                    .filter(|_| tresp.clicked() || tresp.dragged())
                                {
                                    p.seek(
                                        ((at.x - line.left()) / line.width()).clamp(0.0, 1.0) * dur,
                                    );
                                }
                                ui.label(
                                    RichText::new(media::clock(dur))
                                        .color(Color32::from_gray(200))
                                        .monospace()
                                        .size(12.0),
                                );
                                let muted = p.muted();
                                let mb = ui.add(ghost(
                                    egui::Button::new(
                                        RichText::new(if muted {
                                            "\u{1F507}"
                                        } else {
                                            "\u{1F50A}"
                                        })
                                        .color(Color32::WHITE)
                                        .size(15.0),
                                    )
                                    .min_size(vec2(32.0, 32.0)),
                                ));
                                mb.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        if muted { "Turn sound on" } else { "Mute" },
                                    )
                                });
                                if mb.clicked() {
                                    p.set_muted(!muted);
                                }
                            },
                        );
                    } else {
                        let status = match self.files.state(&a.file) {
                            Some(Transfer::Down(f)) => {
                                trf!("Loading the video {}%", (f * 100.0) as u32)
                            }
                            Some(Transfer::Failed(e)) => App::error_text(&e),
                            _ if local.is_some()
                                || v.unplayable.as_deref() == Some(a.file.as_str()) =>
                            {
                                if cfg!(windows) {
                                    tr("This video can't be played on this PC.").into()
                                } else {
                                    tr("This video can't be played on this Mac.").into()
                                }
                            }
                            _ => tr("Loading the video...").into(),
                        };
                        ui.painter().text(
                            bar.center(),
                            Align2::CENTER_CENTER,
                            status,
                            FontId::proportional(13.0),
                            Color32::from_gray(220),
                        );
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
        v.index = media[v.index];
        let _ = t;
        if !close {
            self.viewer = Some(v);
        }
    }
}

/// The values in `text` that fill the "{}"s of `pattern`, when it was made
/// from it.
/// What the team server's file route answers when it refuses (the short
/// lowercase texts in server/src/files.rs), in the interface language; any
/// other text as it is.
pub(super) fn server_body(body: &str) -> String {
    match body {
        "upload broke off" => tr("upload broke off"),
        "file is too large" => tr("file is too large"),
        "can't write files" => tr("can't write files"),
        "can't read the file" => tr("can't read the file"),
        "contents don't match the name" => tr("contents don't match the name"),
        "no such file" => tr("no such file"),
        "sign in first" => tr("sign in first"),
        "already here" => tr("already here"),
        "stored" => tr("stored"),
        _ => body,
    }
    .to_string()
}

pub(super) fn fill_back(pattern: &str, text: &str) -> Option<Vec<String>> {
    let parts: Vec<&str> = pattern.split("{}").collect();
    let mut rest = text.strip_prefix(parts[0])?;
    let mut values = vec![];
    for (i, part) in parts.iter().enumerate().skip(1) {
        let at = if i == parts.len() - 1 {
            if part.is_empty() {
                rest.len()
            } else {
                rest.strip_suffix(part)?.len()
            }
        } else {
            rest.find(part)?
        };
        values.push(rest[..at].to_string());
        rest = &rest[at + part.len()..];
    }
    Some(values)
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
    p.add(egui::Shape::convex_polygon(
        vec![
            c + vec2(-k * 0.7, -k),
            c + vec2(k, 0.0),
            c + vec2(-k * 0.7, k),
        ],
        Color32::WHITE,
        Stroke::NONE,
    ));
}

/// Copy a file to ~/Downloads under its name, without overwriting.
fn save_copy(from: &Path, name: &str) -> Result<PathBuf, String> {
    // tests keep out of the real Downloads folder
    let dir = if cfg!(test) {
        std::env::temp_dir().join("workboard-test-downloads")
    } else {
        crate::os::home_dir().join("Downloads")
    };
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut dest = dir.join(format!("{}{}", stem, ext));
    let mut n = 2;
    while dest.exists() {
        dest = dir.join(format!("{} {}{}", stem, n, ext));
        n += 1;
    }
    std::fs::copy(from, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// The pictures and videos named anywhere in a Trash item (a channel's
/// messages, tasks, the tasks of a list, folder or space), each once.
pub(super) fn trash_attachments(payload: &str) -> Vec<Attachment> {
    fn walk(v: &serde_json::Value, out: &mut Vec<Attachment>) {
        match v {
            serde_json::Value::Object(o) => {
                for (k, x) in o {
                    if k == "attachments" {
                        if let Ok(a) = serde_json::from_value::<Vec<Attachment>>(x.clone()) {
                            for a in a {
                                if !out.iter().any(|o| o.file == a.file) {
                                    out.push(a);
                                }
                            }
                            continue;
                        }
                    }
                    walk(x, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
        walk(&v, &mut out);
    }
    out
}

// ------------------------------------------------------------------ cleaning

/// Every attachment anywhere in a workspace (messages and replies, tasks,
/// the Trash's saved JSON text, ...), found by its shape so nothing that
/// holds one is missed.
pub(super) fn attachments_in(v: &serde_json::Value, out: &mut Vec<Attachment>) {
    match v {
        serde_json::Value::Object(o) => {
            let looks = o
                .get("file")
                .and_then(|f| f.as_str())
                .is_some_and(wb_server_name_ok)
                && o.get("name").is_some_and(|n| n.is_string());
            if looks {
                if let Ok(a) = serde_json::from_value::<Attachment>(v.clone()) {
                    out.push(a);
                    return;
                }
            }
            for x in o.values() {
                attachments_in(x, out);
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                attachments_in(x, out);
            }
        }
        // the Trash keeps what was deleted as JSON text
        serde_json::Value::String(t) if t.starts_with('{') || t.starts_with('[') => {
            if let Ok(inner) = serde_json::from_str::<serde_json::Value>(t) {
                attachments_in(&inner, out);
            }
        }
        _ => {}
    }
}

/// The files (and video stills) a workspace refers to.
pub(super) fn shas_in(v: &serde_json::Value) -> HashSet<String> {
    let mut atts = vec![];
    attachments_in(v, &mut atts);
    atts.iter().flat_map(parts).map(|p| p.file).collect()
}

/// What one workspace on this computer refers to, for cleaning the cache.
#[derive(Clone, Debug, Default)]
pub(super) struct SpaceRefs {
    /// shared with a team (its files are on the server or in the folder)
    pub shared: bool,
    /// every file it refers to, the Trash included
    pub all: HashSet<String>,
    /// the ones only its Trash refers to: the team may have dropped them by
    /// the time someone restores, so the cache may be the last copy
    pub trash_only: HashSet<String>,
}

impl SpaceRefs {
    pub fn of(v: &serde_json::Value) -> SpaceRefs {
        let shared = v
            .get("extra")
            .and_then(|x| x.get("shared"))
            .is_some_and(|s| !s.is_null());
        let all = shas_in(v);
        let mut rest = v.clone();
        if let Some(e) = rest.get_mut("extra").and_then(|e| e.as_object_mut()) {
            e.remove("trash");
        }
        let live = shas_in(&rest);
        let trash_only = all.difference(&live).cloned().collect();
        SpaceRefs {
            shared,
            all,
            trash_only,
        }
    }
}

/// Every 64-hex-digit word in a text (for a file that can't be read as a
/// workspace: anything that looks like a file name counts as used).
fn hex_words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_ascii_hexdigit())
        .filter(|w| wb_server_name_ok(w))
        .map(String::from)
        .collect()
}

/// The workspaces saved on this computer: whether each is shared, and the
/// files it refers to (and which of those only its Trash does).
pub(super) fn workspaces_on_disk(dir: &Path) -> Vec<SpaceRefs> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "json") || !p.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => out.push(SpaceRefs::of(&v)),
            Err(_) => out.push(SpaceRefs {
                shared: false,
                all: hex_words(&text),
                trash_only: HashSet::new(),
            }),
        }
    }
    out
}

/// The files the members' copies in a shared folder refer to. None when
/// one of them can't be read (half synced, say): then nothing is cleaned.
pub(super) fn folder_refs(folder: &Path) -> Option<HashSet<String>> {
    let mut refs = HashSet::new();
    let mut any = false;
    for e in std::fs::read_dir(folder).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(wb_sync::FILE_SUFFIX) || name.starts_with('.') {
            continue;
        }
        let v = wb_sync::read_json(&e.path())?;
        refs.extend(shas_in(&v));
        any = true;
    }
    any.then_some(refs)
}

/// A cached file name: the hash, or the hash with an extension (a link
/// made for the player); the hash it belongs to.
fn cache_base(name: &str) -> Option<&str> {
    let base = name.split('.').next()?;
    wb_server_name_ok(base).then_some(base)
}

fn age(m: &std::fs::Metadata, now: SystemTime) -> Duration {
    m.modified()
        .ok()
        .and_then(|t| now.duration_since(t).ok())
        .unwrap_or_default()
}

/// Tidy the cache: a file no workspace refers to (the Trash counts) goes
/// once it's older than `grace`; then, past `cap` bytes, the least recently
/// used files that only shared workspaces refer to go (they're on the
/// server or in the shared folder and come back when needed). Files in
/// `keep` (waiting to go up, in a message box), files a workspace that
/// isn't shared refers to (the cache is their only copy) and files only a
/// Trash refers to (the team may drop them before a restore) always stay.
/// Returns the hashes removed.
pub(super) fn clean_cache(
    dir: &Path,
    spaces: &[SpaceRefs],
    keep: &HashSet<String>,
    now: SystemTime,
    grace: Duration,
    cap: u64,
) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let used: HashSet<&String> = spaces.iter().flat_map(|s| &s.all).collect();
    // referred to outside a Trash somewhere: fetched again from there
    let live: HashSet<&String> = spaces
        .iter()
        .flat_map(|s| s.all.iter().filter(|f| !s.trash_only.contains(*f)))
        .collect();
    let only_here: HashSet<&String> = spaces
        .iter()
        .filter(|s| !s.shared)
        .flat_map(|s| &s.all)
        .chain(keep)
        .chain(used.iter().copied().filter(|f| !live.contains(f)))
        .collect();
    // hash -> (size, last used); links by the hash they belong to
    let mut bases: HashMap<String, (u64, SystemTime)> = HashMap::new();
    let mut links: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(m) = e.metadata() else { continue };
        if !m.is_file() {
            continue;
        }
        if name.starts_with('.') {
            // a copy, paste or still left half made
            if name != BACKLOG
                && name != super::taskfields::TASK_FILES
                && age(&m, now) > Duration::from_secs(24 * 3600)
            {
                let _ = std::fs::remove_file(e.path());
            }
            continue;
        }
        let Some(base) = cache_base(&name) else {
            continue;
        };
        if base == name {
            bases.insert(
                name.clone(),
                (m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)),
            );
        } else {
            links.entry(base.to_string()).or_default().push(e.path());
        }
    }
    let mut removed = vec![];
    let drop_file = |sha: &str, removed: &mut Vec<String>| {
        let _ = std::fs::remove_file(dir.join(sha));
        for l in links.get(sha).into_iter().flatten() {
            let _ = std::fs::remove_file(l);
        }
        removed.push(sha.to_string());
    };
    // links whose file is gone
    for (base, paths) in &links {
        if !bases.contains_key(base) {
            for p in paths {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    let stale: Vec<String> = bases
        .iter()
        .filter(|(sha, (_, used_at))| {
            !used.contains(sha)
                && !keep.contains(*sha)
                && now.duration_since(*used_at).unwrap_or_default() > grace
        })
        .map(|(sha, _)| sha.clone())
        .collect();
    for sha in stale {
        bases.remove(&sha);
        drop_file(&sha, &mut removed);
    }
    let mut total: u64 = bases.values().map(|(n, _)| n).sum();
    if total > cap {
        let mut team: Vec<(&String, &(u64, SystemTime))> = bases
            .iter()
            .filter(|(sha, _)| used.contains(sha) && !only_here.contains(sha))
            .collect();
        team.sort_by_key(|(_, (_, at))| *at);
        let mut gone = vec![];
        for (sha, (n, _)) in team {
            if total <= cap {
                break;
            }
            total -= n;
            gone.push(sha.clone());
        }
        for sha in gone {
            drop_file(&sha, &mut removed);
        }
    }
    removed
}

/// Tidy a shared folder's files/: a file none of the members' copies refer
/// to (`refs`, see `folder_refs`) goes once it's older than `grace` (it was
/// put there that long ago, so every member's copy has long caught up) and
/// nothing has named it for FOLDER_NAMED_GRACE (files/.named.json keeps when
/// each was last named, like the team server's): a message deleted to
/// someone's Trash, which stays on their computer, keeps its files a good
/// while for a restore. The first clean up, or one that finds that record
/// unreadable, starts every file's clock and deletes nothing it hasn't
/// watched. Any member's app may do this; a file still in use is never
/// touched.
pub(super) fn clean_shared_folder(
    files: &Path,
    refs: &Option<HashSet<String>>,
    now: SystemTime,
    grace: Duration,
) -> Vec<String> {
    let Some(refs) = refs else {
        return vec![];
    };
    let Ok(rd) = std::fs::read_dir(files) else {
        return vec![];
    };
    let entries: Vec<std::fs::DirEntry> = rd.flatten().collect();
    let secs = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let record = files.join(FOLDER_NAMED);
    let all_now = || -> HashMap<String, i64> {
        entries
            .iter()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| wb_server_name_ok(n))
            .map(|n| (n, secs))
            .collect()
    };
    let mut last: HashMap<String, i64> = match std::fs::read(&record) {
        // there but unreadable (cut short, or a sync conflict): start again
        Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|_| all_now()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => all_now(),
        Err(_) => return vec![],
    };
    for n in refs {
        last.insert(n.clone(), secs);
    }
    let named_lately = |n: &str| {
        last.get(n)
            .is_some_and(|t| secs - t < FOLDER_NAMED_GRACE.as_secs() as i64)
    };
    let mut removed = vec![];
    for e in &entries {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(m) = e.metadata() else { continue };
        if !m.is_file() || age(&m, now) <= grace {
            continue;
        }
        if name.starts_with(".copy-") || name.starts_with(".still-") || name.starts_with(".named-")
        {
            let _ = std::fs::remove_file(e.path());
            continue;
        }
        if wb_server_name_ok(&name)
            && !refs.contains(&name)
            && !named_lately(&name)
            && std::fs::remove_file(e.path()).is_ok()
        {
            removed.push(name);
        }
    }
    // remember only files still here
    last.retain(|n, _| files.join(n).is_file());
    let sorted: std::collections::BTreeMap<&String, &i64> = last.iter().collect();
    if let Ok(text) = serde_json::to_string(&sorted) {
        let tmp = files.join(format!(".named-{:x}", wb_sync::random_u64()));
        let ok = std::fs::File::create(&tmp).and_then(|mut f| {
            use std::io::Write;
            f.write_all(text.as_bytes())?;
            f.sync_all()
        });
        if ok.is_ok() {
            let _ = std::fs::rename(&tmp, &record);
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }
    removed
}

/// Where in the window files dragged in from Finder are. The window gets no
/// mouse moves while they are dragged, so egui's pointer is wherever the
/// mouse last was in it; the screen's pointer is asked instead where the OS
/// can tell (os.rs).
pub(super) fn drag_pointer(ctx: &egui::Context) -> Option<egui::Pos2> {
    let zoom = ctx.zoom_factor();
    let (ppp, inner, last) = ctx.input(|i| {
        (
            i.pixels_per_point,
            i.viewport().inner_rect,
            i.pointer.latest_pos(),
        )
    });
    match (crate::os::pointer_on_screen(ppp, zoom), inner) {
        (Some((x, y)), Some(r)) => Some(pos2(x, y) - r.min.to_vec2()),
        _ => last,
    }
}

/// How far an upload is, as a thin bar along a tile's bottom.
fn upload_bar(painter: &egui::Painter, rect: Rect, f: f32) {
    let bar = Rect::from_min_size(
        pos2(rect.left() + 6.0, rect.bottom() - 10.0),
        vec2(rect.width() - 12.0, 4.0),
    );
    painter.rect_filled(bar, 2.0, Color32::from_black_alpha(120));
    painter.rect_filled(
        Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height())),
        2.0,
        Color32::WHITE,
    );
}
