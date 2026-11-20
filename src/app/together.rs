// Working with other people: either a shared folder that a sync service
// keeps in step, or a workboard-server the app stays connected to (see
// crate::sync and wb_net). Signing in to a server: an account's email and
// password, an invite code (choosing a password at the same time), a code
// from a password-reset mail, or setting up an empty server (the sign-in
// screen itself is in login.rs). Also invite links (workboard://join),
// choosing who you are after joining a folder, calls that open in the
// browser with a Join button posted in chat, and a Markdown archive for
// documents.
//
// In together/: the server token (tokens.rs), the connection's wording
// (status.rs), the shared folder (shared_folder.rs), the server connection
// (net.rs), the sign-in forms (forms.rs), the dialogs (dialogs.rs) and
// signing in a phone with a QR code (phone.rs).

use super::*;
use crate::sync::{self, SharedCfg};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
mod dialogs;
mod forms;
mod mine;
mod net;
mod phone;
mod shared_folder;
mod status;
mod tokens;

pub(super) use net::*;
pub(super) use phone::*;
pub(super) use shared_folder::*;
pub(super) use status::*;
pub(super) use tokens::*;

use wb_net::{Event as NetEvent, Msg as NetMsg, Person, Role, SignIn, Status as NetStatus};

/// How often teammates' copies are looked at (folder), and how often ours is
/// written (folder) or sent (server).
const POLL_SECS: f64 = 2.0;
const WRITE_SECS: f64 = 1.5;

/// How this computer signs in to a server the first time, or again.
/// Empty text fields are left out of the hello.
#[derive(Clone, Debug, PartialEq)]
pub enum How {
    /// An invite code; with a password, the person's account is set up too
    /// (the email is the one on their member record unless one is typed).
    Invite {
        code: String,
        email: String,
        password: String,
    },
    /// An account's email and password.
    Account { email: String, password: String },
    /// The code from a password-reset mail, with the new password.
    Reset {
        email: String,
        code: String,
        password: String,
    },
    /// An empty server: this workspace goes up and I become the owner,
    /// with an account when an email and password are given.
    Claim { email: String, password: String },
}

impl How {
    /// The hello fields for signing in this way.
    pub(super) fn sign_in(&self, device: &str, me: u64) -> SignIn {
        let text = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
        let pass = |s: &str| Some(s.to_string()).filter(|s| !s.is_empty());
        let mut si = SignIn {
            device: device.to_string(),
            ..Default::default()
        };
        match self {
            How::Invite {
                code,
                email,
                password,
            } => {
                si.invite = text(code);
                si.email = text(email);
                si.new_password = pass(password);
            }
            How::Account { email, password } => {
                si.email = text(email);
                si.password = pass(password);
            }
            How::Reset {
                email,
                code,
                password,
            } => {
                si.email = text(email);
                si.reset_code = text(code);
                si.new_password = pass(password);
            }
            How::Claim { email, password } => {
                si.claim = Some(me);
                si.email = text(email);
                si.new_password = pass(password);
            }
        }
        si
    }
}

/// Connecting to a server for the first time from the dialog.
pub struct Joining {
    pub addr: String,
    pub device: String,
    pub how: How,
}

/// Which sign-in fields the form shows.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Mode {
    #[default]
    Account,
    Invite,
    Forgot,
    NewServer,
}

/// Shown with a shared folder: every computer's copy in the folder holds the
/// whole workspace, so what the app hides from other people (Personal
/// spaces, private docs, channels and dashboards) can still be read from the
/// files. Only a team server leaves those out of other people's copies.
pub(super) const FOLDER_PRIVACY: &str = "Private items (Personal spaces, private docs, channels and dashboards) are hidden in the app, but anyone with this folder can read them in its files. Use a team server to keep them private.";

/// What's typed into the sign-in and account fields.
#[derive(Default)]
pub struct Form {
    pub mode: Mode,
    pub email: String,
    pub password: String,
    pub repeat: String,
    /// An invite code, or the code from a reset mail.
    pub code: String,
    /// The current password, when changing it.
    pub old: String,
    /// A reset mail was asked for: show the code fields.
    pub sent: bool,
    /// "Change password" is open.
    pub changing: bool,
    /// What went wrong, or what happens next.
    pub note: String,
    /// Why "Update Workboard" couldn't run, under the button.
    pub update_note: String,
    /// "Update Workboard" was pressed on Windows: the app's own updater
    /// shows how it goes under the button.
    pub app_updates: bool,
}

/// Why a new password won't do, if it won't.
pub(super) fn new_password_problem(password: &str, repeat: &str) -> Option<String> {
    if password.chars().count() < wb_net::MIN_PASSWORD {
        return Some(trf!(
            "Choose a password of at least {} characters.",
            wb_net::MIN_PASSWORD
        ));
    }
    (password != repeat).then(|| tr("The two passwords aren't the same.").to_string())
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
    /// Sign in again this way (after access was removed or the token stopped working).
    pub(super) resign: Option<How>,
    /// The email this person signs in with, once they have an account.
    pub(super) account: Option<String>,
    /// The sign-in fields.
    pub(super) form: Form,
    /// The account fields (setting up or changing a password).
    pub(super) acct: Form,
    /// People just invited on the server: ask for their codes (which the
    /// server emails) once the server has them.
    pub(super) auto_invite: Vec<(u64, Role)>,
    /// A "Forgot password" request on its way.
    pub(super) forgot: Option<std::sync::mpsc::Receiver<Result<bool, String>>>,
    /// Signing in with email and password: asking the server which of its
    /// workspaces that account has (POST /auth/workspaces), before joining.
    pub(super) listing: Option<Listing>,
    /// The account has several workspaces on the server: which to open.
    pub(super) choices: Option<(Vec<(String, String)>, How)>,
    /// A workspace on this computer to open (the one chosen is here already).
    pub(super) open_local: Option<String>,
    /// The server's webhooks (owners and admins) and slash commands.
    pub(super) integrations: Option<(Vec<wb_net::HookInfo>, Vec<wb_net::CommandInfo>)>,
    /// My scheduled messages the server keeps (None until it answered).
    pub(super) scheduled: Option<Vec<wb_net::ScheduledMsg>>,
    /// The owner's view of the server's backups (None until asked for).
    pub(super) backups: Option<Vec<wb_net::Backup>>,
    /// The backup the owner clicked Restore on, waiting for "Yes, restore".
    pub(super) restore_confirm: Option<String>,
    /// Failed tries to reach the server in a row (none once connected).
    pub(super) fails: u32,
    /// Work together shows a field for the server's new address.
    pub(super) editing_addr: bool,
    /// The person was told the team's copy couldn't be read (said once).
    told_unreadable: bool,
    /// Told this session that someone in the shared folder runs an older app.
    pub(super) told_old_app: bool,
    /// When a request other than joining a huddle last went to the server
    /// (see huddle_denied_unnamed).
    pub(super) other_asked: Option<std::time::Instant>,
    /// The members connected to the server, as it last said (None until it
    /// says, or after the connection drops; older servers never say).
    pub(super) online: Option<Vec<u64>>,
    /// Who the server says is typing where (see typing.rs).
    pub(super) typing: Vec<super::typing::Typist>,
    /// The conversation (and thread) I last told the server I'm typing in,
    /// and when.
    pub(super) typed: Option<(u64, Option<u64>, std::time::Instant)>,
    /// A code that signs my phone in, while Work together shows it.
    pub(super) phone: Option<PhoneLink>,
    /// When the server was asked for a new one that hasn't come yet.
    pub(super) phone_asked: Option<std::time::Instant>,
    /// "Show code" was pressed: the code shows under the QR code.
    pub(super) phone_code_shown: bool,
    /// The server keeps my reading state for my other devices (it sent
    /// `mine` on this connection): changes to it go up from now on.
    pub(super) mine_ok: bool,
    /// When my reading state was last looked at for changes (`ctx` time).
    mine_checked: f64,
}

/// Asking a server which workspaces an email and password sign in to.
pub struct Listing {
    rx: std::sync::mpsc::Receiver<Result<Vec<(String, String)>, String>>,
    #[cfg_attr(not(test), allow(dead_code))] // tests check which server was asked
    pub(super) addr: String,
    pub(super) how: How,
}

/// A local workspace file that is on the team server workspace `addr`
/// (`wb_net::same_workspace`), other than `except`.
pub(super) fn local_workspace_on(addr: &str, except: &str) -> Option<String> {
    Data::workspaces()
        .into_iter()
        .map(|e| e.file)
        .filter(|f| f != except)
        .find(|f| {
            let path = if f == "data.json" {
                Data::path()
            } else {
                Data::dir().join(f)
            };
            std::fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| {
                    v.get("extra")?
                        .get("shared")?
                        .get("server")?
                        .as_str()
                        .map(|a| wb_net::same_workspace(a, addr))
                })
                .unwrap_or(false)
        })
}

/// A new workspace asked of the team server (POST /workspaces), waiting for
/// its address. Kept outside SyncRt, which starts over when the workspace
/// changes.
pub struct MakingWs {
    rx: std::sync::mpsc::Receiver<Result<String, String>>,
    name: String,
    apps: Vec<String>,
    invite: Vec<String>,
    role: String,
    /// the account email the new workspace is set up with
    email: String,
}

/// What the shared folder holds, for the dialog.
pub(super) fn folder_people(folder: &str) -> usize {
    if folder.trim().is_empty() {
        return 0;
    }
    sync::other_files(Path::new(folder.trim()), "").len()
}

/// A long folder path cut down from the front to whole folder names
/// ("\u{2026}/Dropbox/Team/_shared"), so it fits on one line.
pub(super) fn short_path(p: &str, max: usize) -> String {
    if p.chars().count() <= max {
        return p.to_string();
    }
    // Windows paths are written with backslashes
    let sep = if p.contains('\\') && !p.contains('/') {
        '\\'
    } else {
        '/'
    };
    let mut out = String::new();
    for part in p.rsplit(sep).filter(|s| !s.is_empty()) {
        let next = if out.is_empty() {
            part.to_string()
        } else {
            format!("{}{}{}", part, sep, out)
        };
        if !out.is_empty() && next.chars().count() + 2 > max {
            break;
        }
        out = next;
    }
    format!("\u{2026}{}{}", sep, out)
}

/// This app's version, written into the shared folder and sent in hello.
pub(super) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(super) fn call_link() -> String {
    format!("https://meet.jit.si/Workboard-{:016x}", sync::random_u64())
}

impl App {
    /// Post a call in a channel or DM and open it in the browser.
    pub(super) fn start_call(
        &mut self,
        ctx: Option<&egui::Context>,
        ch: u64,
        custom: &str,
    ) -> String {
        let custom = custom.trim();
        let link = if custom.starts_with("https://") || custom.starts_with("http://") {
            custom.to_string()
        } else {
            call_link()
        };
        let id = self.data.new_id();
        let me = self.data.me;
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
            c.messages.push(Message {
                edited: None,
                imported: None,
                preview: None,
                forwarded: None,
                thread_of: None,
                bot: None,
                mentions: vec![],
                attachments: vec![],
                id,
                author: me,
                text: "Started a call".into(),
                ts: now_ts(),
                reactions: vec![],
                replies: vec![],
                saved: false,
                task: None,
                post_title: None,
                post_type: String::new(),
                pinned: false,
                call: Some(link.clone()),
            });
            c.last_read = now_ts();
        }
        self.section = Section::Chat;
        self.chat.open = Some(ch);
        self.dirty = true;
        if let Some(ctx) = ctx {
            ctx.open_url(egui::OpenUrl::new_tab(&link));
        }
        link
    }

    /// Where archived documents are written as Markdown.
    pub(super) fn archive_dir(&self) -> PathBuf {
        match &self.data.extra.shared {
            Some(cfg) if cfg.server.is_none() && !cfg.folder.is_empty() => {
                Path::new(&cfg.folder).join("Docs archive")
            }
            _ => crate::os::home_dir()
                .join("Documents")
                .join("Workboard Docs archive"),
        }
    }

    /// Save one document as a Markdown file in the archive folder.
    pub(super) fn export_doc(&self, id: u64) -> Option<PathBuf> {
        let d = self.data.docs.iter().find(|d| d.id == id)?;
        let dir = self.archive_dir();
        std::fs::create_dir_all(&dir).ok()?;
        let safe: String = d
            .title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let safe = if safe.trim().is_empty() {
            "Untitled".to_string()
        } else {
            safe.trim().to_string()
        };
        let path = dir.join(format!("{} ({}).md", safe, d.id % 100000));
        let author = self
            .data
            .member(d.owner)
            .map(|m| m.shown().to_string())
            .unwrap_or_default();
        let when = local(d.updated)
            .map(|x| x.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let mut text = format!(
            "# {}\n\n_{} \u{2022} last updated {}_\n\n{}\n",
            d.title, author, when, d.body
        );
        // sub pages follow as sections, one heading level down per level
        fn subs(docs: &[Doc], parent: u64, level: usize, out: &mut String, seen: &mut Vec<u64>) {
            for c in docs.iter().filter(|c| c.parent == Some(parent)) {
                if seen.contains(&c.id) {
                    continue;
                }
                seen.push(c.id);
                out.push_str(&format!(
                    "\n{} {}\n\n{}\n",
                    "#".repeat(level.min(6)),
                    c.title,
                    c.body
                ));
                subs(docs, c.id, level + 1, out, seen);
            }
        }
        subs(&self.data.docs, d.id, 2, &mut text, &mut vec![d.id]);
        std::fs::write(&path, text).ok()?;
        Some(path)
    }

    /// Archive (or bring back) a document; archiving also saves a Markdown copy.
    pub(super) fn toggle_archive(&mut self, id: u64) {
        let Some(d) = self.data.docs.iter_mut().find(|d| d.id == id) else {
            return;
        };
        d.archived = !d.archived;
        let archived = d.archived;
        self.dirty = true;
        if archived {
            let msg = match self.export_doc(id) {
                Some(p) => trf!(
                    "Archived. A copy was saved in {}",
                    p.parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                ),
                None => tr("Archived. (The Markdown copy couldn't be saved.)").into(),
            };
            self.toast(msg);
        } else {
            self.toast(tr("Moved back to Docs"));
        }
    }
}
