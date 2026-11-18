// Working with other people: either a shared folder that a sync service
// keeps in step, or a workboard-server the app stays connected to (see
// crate::sync and wb_net). Signing in to a server: an account's email and
// password, an invite code (choosing a password at the same time), a code
// from a password-reset mail, or setting up an empty server (the sign-in
// screen itself is in login.rs). Also invite links (workboard://join),
// choosing who you are after joining a folder, calls that open in the
// browser with a Join button posted in chat, and a Markdown archive for
// documents.

use super::*;
use crate::sync::{self, SharedCfg};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
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

// ---------------------------------------------------------------- tokens
//
// The token a server gives this computer lives in the macOS keychain (never
// in data.json). When trying the app with a separate WORKBOARD_HOME folder it
// lives in that folder instead, and tests keep it in memory.

#[cfg(not(test))]
const KEYCHAIN_SERVICE: &str = "Workboard server";

/// What a password may hold: English letters, digits, symbols and spaces.
/// Typed in Korean mode the same keys give other letters, and the password
/// then won't match the one typed in English on another computer or phone.
pub(super) fn password_char(c: char) -> bool {
    (' '..='~').contains(&c)
}

/// A password box: dots by default, an eye button inside its right edge
/// shows what was typed, and only `password_char`s go in. Anything else
/// (Korean input, pasted text in another script) is left out and a note
/// under the box asks for English input.
pub(super) fn secret_edit(
    ui: &mut Ui,
    t: &Theme,
    text: &mut String,
    label: &str,
    hint: &str,
    big: bool,
) -> egui::Response {
    let id = ui.make_persistent_id(("password field", label));
    let shown_id = id.with("shown");
    let note_id = id.with("not english");
    let mut shown = ui.data(|d| d.get_temp::<bool>(shown_id)).unwrap_or(false);
    let mut note = ui.data(|d| d.get_temp::<bool>(note_id)).unwrap_or(false);
    if ui.memory(|m| m.has_focus(id)) {
        ui.input_mut(|i| {
            i.events.retain_mut(|e| {
                let s = match e {
                    egui::Event::Text(s) | egui::Event::Paste(s) => s,
                    egui::Event::Ime(egui::ImeEvent::Commit(s)) => s,
                    // a word still being put together in another script:
                    // nothing of it goes in
                    egui::Event::Ime(egui::ImeEvent::Preedit(s)) => {
                        if !s.is_empty() {
                            note = true;
                        }
                        return false;
                    }
                    _ => return true,
                };
                let kept: String = s.chars().filter(|c| password_char(*c)).collect();
                if kept.len() != s.len() {
                    note = true;
                } else if !kept.is_empty() {
                    note = false;
                }
                *s = kept;
                !s.is_empty()
            })
        });
    }
    let (eye_w, margin) = if big {
        (
            40.0,
            egui::Margin {
                left: 14,
                right: 40,
                top: 12,
                bottom: 12,
            },
        )
    } else {
        (
            30.0,
            egui::Margin {
                left: 8,
                right: 30,
                top: 5,
                bottom: 5,
            },
        )
    };
    let mut te = field(text)
        .id(id)
        .hint_text(tr(hint))
        .password(!shown)
        .margin(margin)
        .desired_width(f32::INFINITY);
    if big {
        te = te
            .font(FontId::proportional(17.0))
            .min_size(vec2(0.0, super::login::FIELD_H));
    }
    let r = ui.add(te);
    let name = format!("{} entry", label);
    r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name));
    // the eye, inside the box's right edge (the response holds the box
    // without its margin)
    let outer = r.rect + margin;
    let er = egui::Rect::from_min_max(
        egui::pos2(outer.right() - eye_w, outer.top()),
        outer.right_bottom(),
    );
    let eye = ui.interact(er, id.with("eye"), Sense::click());
    let what = if shown {
        "Hide password"
    } else {
        "Show password"
    };
    name_it(&eye, what);
    let color = if eye.hovered() { t.text } else { t.text2 };
    super::icons::line_icon(
        ui.painter(),
        if shown { "eye_off" } else { "eye" },
        er.center(),
        color,
    );
    let eye = eye
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tr(what));
    // pressing the eye takes the focus off the box (egui drops it on any
    // press elsewhere); give it straight back so typing carries on
    let was_focused = ui.data(|d| d.get_temp::<bool>(id.with("focused")).unwrap_or(false));
    if was_focused && (eye.is_pointer_button_down_on() || eye.clicked()) {
        r.request_focus();
    }
    if eye.clicked() {
        shown = !shown;
        r.request_focus();
    }
    let focused = ui.memory(|m| m.has_focus(id));
    ui.data_mut(|d| d.insert_temp(id.with("focused"), focused));
    text.retain(password_char);
    if note && !focused {
        note = false;
    }
    if note {
        ui.label(
            RichText::new(tr(
                "Passwords take English letters, numbers and symbols. Switch to English input.",
            ))
            .size(12.0)
            .color(t.danger),
        );
    }
    ui.data_mut(|d| {
        d.insert_temp(shown_id, shown);
        d.insert_temp(note_id, note);
    });
    r
}

fn token_account(addr: &str, device: &str) -> String {
    format!("{}#{}", addr.trim(), device)
}

#[cfg(test)]
fn test_tokens() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static T: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
        std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

#[cfg(not(test))]
fn token_file() -> Option<PathBuf> {
    let separate = std::env::var("WORKBOARD_HOME").is_ok_and(|v| !v.is_empty());
    (separate || !cfg!(target_os = "macos")).then(|| Data::dir().join("server-tokens.json"))
}

#[cfg(not(test))]
fn read_token_file(p: &Path) -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(not(test))]
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

/// Goes up whenever a token is saved or forgotten, so a copy kept to save
/// reading the keychain every frame knows it's out of date.
static TOKEN_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(super) fn token_gen() -> u64 {
    TOKEN_GEN.load(std::sync::atomic::Ordering::SeqCst)
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
        return security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, &key)
            .ok()
            .and_then(|b| String::from_utf8(b).ok());
        #[allow(unreachable_code)]
        None
    }
}

pub(super) fn save_token(addr: &str, device: &str, token: &str) {
    let key = token_account(addr, device);
    TOKEN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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
        let _ = security_framework::passwords::set_generic_password(
            KEYCHAIN_SERVICE,
            &key,
            token.as_bytes(),
        );
    }
}

pub(super) fn forget_token(addr: &str, device: &str) {
    let key = token_account(addr, device);
    TOKEN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

/// What the team server (or its client) said, in the interface language:
/// the server sends English, and some of it carries a number inside.
pub(super) fn server_text(s: &str) -> String {
    if s.starts_with("Couldn't reach the server: ") {
        // the system's own words follow, untranslated
        return tr("Couldn't reach the server. Check the address and your connection.").into();
    }
    const WITH_VALUES: [&str; 5] = [
        "Choose a password of at least {} characters.",
        "This server already keeps {} workspaces, the most it can.",
        "Too many wrong tries. Wait {} minutes and try again, or use Forgot password.",
        "That reset code doesn't work. It may be used already or expired (codes last {} minutes). Ask for a new one.",
        "The server answered {}.",
    ];
    for pattern in WITH_VALUES {
        if let Some(values) = super::files::fill_back(pattern, s) {
            return crate::i18n::fill(tr(pattern), &values);
        }
    }
    tr(s).to_string()
}

/// The colour of a connection that isn't working.
pub(super) const WARN: Color32 = Color32::from_rgb(0xe1, 0x6b, 0x16);

/// Failed tries in a row after which "Reconnecting" becomes "Can't reach
/// the server" (the app keeps trying either way).
pub(super) const FAILS_BEFORE_CANT_REACH: u32 = 3;

/// Why the server can't be reached, in plain words: the connection code
/// says it in English, with the system's own error inside.
pub(super) fn reach_text(why: &str) -> String {
    use super::files::fill_back;
    if let Some(v) = fill_back("Can't reach {}: {}", why) {
        return if v[1].to_lowercase().contains("refused") {
            trf!(
                "Nothing answers at {}. Check that workboard-server is running there.",
                v[0]
            )
        } else {
            trf!(
                "Can't reach {}. Check the address and that this computer is on the team's network.",
                v[0]
            )
        };
    }
    if let Some(v) = fill_back("Can't find {}: {}", why).or_else(|| fill_back("Can't find {}", why))
    {
        return trf!("Can't find {}. Check the server address.", v[0]);
    }
    if let Some(v) = fill_back("The secure connection to {} failed: {}", why) {
        return trf!(
            "Couldn't make a secure connection to {}. Check the address: the certificate there isn't one this computer trusts for it.",
            v[0]
        );
    }
    if why.starts_with("The server didn't answer like Workboard") {
        return tr("Something answered at this address, but not a Workboard server. Check the address and port.").into();
    }
    let said = server_text(why);
    // what the connection library or the system said, untranslated
    let raw = why.contains("os error")
        || why.starts_with("IO error")
        || (crate::i18n::is_ko() && said == why);
    if raw {
        tr("The connection to the server broke.").into()
    } else {
        said
    }
}

/// How the server connection is doing, in a word or two. `fails`: failed
/// tries in a row.
pub(super) fn net_status_text(net: bool, s: Option<&NetStatus>, fails: u32) -> String {
    match (net, s) {
        (false, _) => tr("Disconnected").into(),
        (true, None) | (true, Some(NetStatus::Connecting)) => tr("Connecting\u{2026}").into(),
        (true, Some(NetStatus::Connected)) => tr("Connected").into(),
        (true, Some(NetStatus::Reconnecting(_))) if fails >= FAILS_BEFORE_CANT_REACH => {
            tr("Can't reach the server").into()
        }
        (true, Some(NetStatus::Reconnecting(_))) => tr("Reconnecting\u{2026}").into(),
        (true, Some(NetStatus::Refused(_))) => tr("Not signed in").into(),
        (true, Some(NetStatus::Gone(_))) => tr("Deleted from the server").into(),
    }
}

/// Why the connection isn't working, when it isn't.
pub(super) fn net_problem(s: Option<&NetStatus>) -> Option<String> {
    match s {
        Some(NetStatus::Reconnecting(why)) => {
            Some(trf!("{} Trying again every few seconds.", reach_text(why)))
        }
        Some(NetStatus::Refused(why) | NetStatus::Gone(why)) => Some(server_text(why)),
        _ => None,
    }
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
    let mut out = String::new();
    for part in p.rsplit('/').filter(|s| !s.is_empty()) {
        let next = if out.is_empty() {
            part.to_string()
        } else {
            format!("{}/{}", part, out)
        };
        if !out.is_empty() && next.chars().count() + 2 > max {
            break;
        }
        out = next;
    }
    format!("\u{2026}/{}", out)
}

/// This app's version, written into the shared folder and sent in hello.
pub(super) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(super) fn call_link() -> String {
    format!("https://meet.jit.si/Workboard-{:016x}", sync::random_u64())
}

/// A workspace as the app wrote it before fields left at their default were
/// left out of the file (time entries' `member`, forms', whiteboards' and
/// dashboards' `owner`, dashboards' `kind`), for `sync::carry_hashes`.
pub(super) fn legacy_shape(v: &serde_json::Value) -> serde_json::Value {
    let mut v = v.clone();
    let fill = |items: Option<&mut serde_json::Value>, keys: &[(&str, serde_json::Value)]| {
        for it in items.and_then(|a| a.as_array_mut()).into_iter().flatten() {
            if let Some(o) = it.as_object_mut() {
                for (k, d) in keys {
                    o.entry(k.to_string()).or_insert_with(|| d.clone());
                }
            }
        }
    };
    for t in v
        .get_mut("tasks")
        .and_then(|a| a.as_array_mut())
        .into_iter()
        .flatten()
    {
        fill(t.get_mut("time_entries"), &[("member", 0.into())]);
    }
    if let Some(e) = v.get_mut("extra") {
        fill(e.get_mut("forms"), &[("owner", 0.into())]);
        fill(e.get_mut("whiteboards"), &[("owner", 0.into())]);
        fill(
            e.get_mut("dashboards"),
            &[("owner", 0.into()), ("kind", "".into())],
        );
    }
    v
}

/// The team server to look for app updates on, chosen at start so updates
/// arrive whichever workspace is open, even one kept only on this computer:
/// the open workspace's server, else the one last signed in to, else the
/// first other workspace on a server.
pub(super) fn update_server(open: &Data) -> Option<String> {
    let others = Data::workspaces()
        .into_iter()
        .map(|w| Data::load_file(&w.file).and_then(|d| d.extra.shared.and_then(|c| c.server)));
    pick_update_server(
        open.extra.shared.as_ref().and_then(|c| c.server.clone()),
        &super::login::last_server(),
        others,
    )
}

pub(super) fn pick_update_server(
    open: Option<String>,
    last: &str,
    others: impl IntoIterator<Item = Option<String>>,
) -> Option<String> {
    let real = |s: &String| !s.trim().is_empty();
    open.filter(real)
        .or_else(|| Some(last.to_string()).filter(real))
        .or_else(|| others.into_iter().flatten().find(real))
}

/// After opening a workspace: a copy saved before field fingerprints were
/// kept gets them for every item unchanged since its last save, so changes
/// made before the next sync are noted field by field (as the iPhone does
/// when it opens an older copy). Without them a whole item counts as
/// changed, and a field emptied offline (the last attachment removed) comes
/// back from the team's copy.
pub(super) fn fill_sync_baseline(data: &mut Data) {
    if data.extra.shared.is_none() || data.extra.sync.hashes.is_empty() {
        return;
    }
    let v = serde_json::to_value(&*data).unwrap_or_default();
    let meta = &mut data.extra.sync;
    if meta.v == 1 {
        // fingerprints from before fields at their default were left out
        // of the file: the new shape alone isn't an edit
        sync::carry_hashes(meta, &legacy_shape(&v), &v);
    }
    sync::fill_field_hashes(meta, &v);
    // a shared setting or Slack match never fingerprinted (every save since
    // shared maps came in fingerprints them all) is as it is now, so the
    // next change doesn't take it as set then, as the iPhone does
    sync::fill_value_hashes(meta, &v);
}

impl App {
    /// The team server a new workspace can be made on: the one this
    /// workspace is on, when it is the server's first workspace and this
    /// person is its owner or an admin, signed in with an account (as the
    /// server allows).
    pub(super) fn server_for_new_workspace(&self) -> Option<String> {
        let cfg = self.data.extra.shared.as_ref()?;
        let addr = cfg.server.clone()?;
        self.sync_rt.account.as_ref()?;
        if !self.sync_rt.role?.manages() || addr.contains("/w/") {
            return None;
        }
        Some(addr)
    }

    /// Make a workspace on the team server this one is on; it opens once
    /// the server has made it (making_ws_tick).
    pub(super) fn make_workspace_on_server(
        &mut self,
        name: &str,
        apps: &[String],
        invite: &[String],
        role: &str,
        ctx: Option<egui::Context>,
    ) {
        let (Some(addr), Some(cfg), Some(email)) = (
            self.server_for_new_workspace(),
            self.data.extra.shared.clone(),
            self.sync_rt.account.clone(),
        ) else {
            return;
        };
        let Some(token) = load_token(&addr, &cfg.device) else {
            self.toast(tr(
                "Sign in to the team server again, then make the workspace.",
            ));
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let auth = wb_net::Auth {
            device: cfg.device.clone(),
            token,
        };
        std::thread::spawn(move || {
            let _ = tx.send(wb_net::create_workspace(&addr, &auth));
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
        self.making_ws = Some(MakingWs {
            rx,
            name: name.to_string(),
            apps: apps.to_vec(),
            invite: invite.to_vec(),
            role: role.to_string(),
            email,
        });
        self.toast(tr("Making the workspace on the team server..."));
    }

    /// The server answered a make_workspace_on_server: open the new
    /// workspace on this computer and set it up on the server as its owner.
    pub(super) fn making_ws_tick(&mut self) {
        let Some(m) = &self.making_ws else {
            return;
        };
        let answer = match m.rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err("The server had a problem. Try again.".into())
            }
        };
        let m = self.making_ws.take().expect("checked above");
        let addr = match answer {
            Ok(a) => a,
            Err(e) => {
                self.toast(trf!("Couldn't make the workspace: {}", server_text(&e)));
                return;
            }
        };
        let before = self.ws_file.clone();
        self.create_workspace(&m.name, &m.apps, &[], &m.role);
        if self.ws_file == before {
            return;
        }
        // the owner signs in with the account they have on the server
        if let Some(me) = self.data.members.iter_mut().find(|x| x.id == self.data.me) {
            me.email = m.email.clone();
        }
        let how = How::Claim {
            email: m.email,
            password: String::new(),
        };
        if let Err(e) = self.connect_with(&addr, how) {
            self.toast(server_text(&e));
            return;
        }
        self.invite_people(&m.invite, &m.role);
        self.save_local();
    }

    fn data_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.data).unwrap_or_default()
    }

    /// After a change: note what changed so teammates' copies merge correctly.
    pub(super) fn sync_note_change(&mut self) {
        if self.data.extra.shared.is_none() {
            return;
        }
        self.stamp_now();
        self.sync_rt.pending = true;
    }

    /// Record what changed since the last save in the change bookkeeping.
    /// True when anything did.
    fn stamp_now(&mut self) -> bool {
        let v = self.data_value();
        let mut meta = std::mem::take(&mut self.data.extra.sync);
        let before = meta.clone();
        if meta.v == 1 {
            // fingerprints from before fields left at their default were
            // left out of the file: the new shape alone isn't an edit
            sync::carry_hashes(&mut meta, &legacy_shape(&v), &v);
        }
        sync::stamp(&v, &mut meta, chrono::Utc::now().timestamp_millis());
        let changed = meta != before;
        self.data.extra.sync = meta;
        changed
    }

    pub(super) fn write_shared(&mut self) {
        let Some(cfg) = self.data.extra.shared.clone() else {
            return;
        };
        self.data.fix_activity_ids();
        let v = self.data_value();
        if cfg.server.is_some() {
            // sent when connected; after a reconnect the whole copy goes again anyway
            if let Some(net) = &self.sync_rt.net {
                net.push(sync::shareable(&v));
            }
            self.sync_rt.pending = false;
            return;
        }
        if sync::write_mine(Path::new(&cfg.folder), &cfg.device, &v, APP_VERSION).is_err() {
            self.toast(tr(
                "Couldn't write to the shared folder. Is it still there?",
            ));
        }
        self.sync_rt.pending = false;
    }

    fn save_local(&self) {
        self.save_now();
    }

    /// Read teammates' copies and merge them in. Returns true when something new arrived.
    pub(super) fn sync_pull(&mut self) -> bool {
        let Some(cfg) = self.data.extra.shared.clone() else {
            return false;
        };
        if cfg.server.is_some() {
            return self.net_tick(None);
        }
        let files = sync::other_files(Path::new(&cfg.folder), &cfg.device);
        self.sync_rt.seen = files.clone();
        let others: Vec<serde_json::Value> = files
            .iter()
            .filter_map(|(p, _)| sync::read_json(p))
            .collect();
        if !self.sync_rt.told_old_app {
            if let Some(v) = others.iter().find_map(|o| sync::older_app(o, APP_VERSION)) {
                self.sync_rt.told_old_app = true;
                self.toast(if v.is_empty() {
                        tr("Someone in this shared folder uses an older version of Workboard. Ask them to update, or their changes can undo yours.").to_string()
                    } else {
                        trf!("Someone in this shared folder uses Workboard {}, an older version. Ask them to update, or their changes can undo yours.", v)
                    });
            }
        }
        self.merge_in(&others, &[])
    }

    /// Merge the team's copies into ours. On a server, `hidden` (with what
    /// our own copy shows we may not see) is taken out of our copy without
    /// deleting it. Returns true when something new arrived.
    pub(super) fn merge_in(&mut self, others: &[serde_json::Value], hidden: &[u64]) -> bool {
        if others.is_empty() {
            return false;
        }
        // changes not recorded yet (made just before quitting, say) are
        // recorded first, or the merge would take them as already known
        if self.data.extra.shared.is_some()
            && !self.data.extra.sync.hashes.is_empty()
            && self.stamp_now()
        {
            self.sync_rt.pending = true;
        }
        let me = self.data.me;
        let seen: Vec<u64> = self
            .data
            .channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Channel && (!c.private || c.members.contains(&me)))
            .map(|c| c.id)
            .collect();
        let slack_before: std::collections::BTreeSet<String> =
            self.data.extra.slack_map.keys().cloned().collect();
        let mut v = self.data_value();
        let mut changed = sync::merge(&mut v, others);
        let on_server = self
            .data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.is_some());
        let gone = if on_server {
            // the server says what our role keeps from us (spaces shared
            // with chosen people); our own copy adds only what no role
            // opens (others' DMs, Personal spaces and private docs). Our
            // role here may be a step behind the server's: just made an
            // admin, a space that just came would go again.
            let mut ids = hidden.to_vec();
            ids.extend(sync::hidden_from(&v, me, true));
            sync::hide(&mut v, &ids)
        } else {
            vec![]
        };
        changed |= !gone.is_empty();
        match serde_json::from_value::<Data>(v) {
            Ok(d) => {
                self.data = d;
                self.data_changed();
                self.data.note_carried_slack(&slack_before);
                // fingerprints of what the app writes, which can differ a
                // little from what came in (fields an older app left out)
                let v = self.data_value();
                self.data.extra.sync.hashes = sync::fingerprints(&v);
                self.data.extra.sync.field_hashes = sync::part_hashes(&v);
            }
            Err(e) => {
                // most likely saved by a newer version of the app
                if !self.sync_rt.told_unreadable {
                    eprintln!("workboard: couldn't take in the team's changes: {}", e);
                    self.toast(tr("Your team's changes couldn't be read. Someone may be using a newer version of the app, so update Workboard."));
                    self.sync_rt.told_unreadable = true;
                }
                return false;
            }
        }
        for ch in gone.iter().filter(|c| {
            c.get("id")
                .and_then(|i| i.as_u64())
                .is_some_and(|i| seen.contains(&i))
        }) {
            self.left_channel(ch);
        }
        if changed {
            // what's new for me is announced on the next frame (notify.rs)
            self.notify_rt.pending = true;
            // teammates' changes come often: saved like my own edits, once
            // they settle and off the screen's thread
            self.save_soon();
            self.sync_rt.pending = true;
        }
        changed
    }

    /// A channel I could see was taken out of my copy because I'm no longer
    /// in it: say so, and keep what I wrote there in the last few minutes in
    /// my Notepad, since it may not have reached the team before I left.
    fn left_channel(&mut self, ch: &serde_json::Value) {
        let me = self.data.me;
        let name = ch
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_default()
            .to_string();
        let since = now_ts() - 10 * 60;
        let mut mine: Vec<String> = vec![];
        for m in ch
            .get("messages")
            .and_then(|m| m.as_array())
            .into_iter()
            .flatten()
        {
            for x in std::iter::once(m).chain(
                m.get("replies")
                    .and_then(|r| r.as_array())
                    .into_iter()
                    .flatten(),
            ) {
                let author = x.get("author").and_then(|a| a.as_u64());
                let ts = x.get("ts").and_then(|t| t.as_i64()).unwrap_or(0);
                let text = x.get("text").and_then(|t| t.as_str()).unwrap_or_default();
                if author == Some(me) && ts >= since && !text.trim().is_empty() {
                    mine.push(text.to_string());
                }
            }
        }
        if mine.is_empty() {
            self.toast(trf!(
                "You're no longer in #{}, so it was taken off your list.",
                name
            ));
            return;
        }
        let notepad = &mut self.data.extra.notepad;
        if !notepad.is_empty() && !notepad.ends_with('\n') {
            notepad.push('\n');
        }
        notepad.push_str(&format!(
            "{}\n{}\n",
            trf!("Written in #{} before you were taken out of it:", name),
            mine.join("\n")
        ));
        self.toast(trf!("You're no longer in #{}. What you wrote there in the last few minutes is in your Notepad.", name));
    }

    /// What the top bar says about a team server that isn't working: can't
    /// be reached, or won't let this computer in. None while connected,
    /// connecting, or without a server.
    /// Whether this workspace works through a team server.
    pub(super) fn on_server(&self) -> bool {
        self.data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.is_some())
    }

    /// Whether this computer is connected to the team server right now.
    pub(super) fn connected(&self) -> bool {
        self.on_server() && self.sync_rt.net_status == Some(NetStatus::Connected)
    }

    /// Whether `member` has Workboard open and connected somewhere: None
    /// when that isn't known (no server, not connected, or a server too old
    /// to say).
    pub(super) fn is_online(&self, member: u64) -> Option<bool> {
        if !self.connected() {
            return None;
        }
        if member == self.data.me {
            return Some(true);
        }
        self.sync_rt.online.as_ref().map(|o| o.contains(&member))
    }

    pub(super) fn server_trouble(&self) -> Option<String> {
        self.data.extra.shared.as_ref()?.server.as_ref()?;
        self.sync_rt.net.as_ref()?;
        match &self.sync_rt.net_status {
            Some(NetStatus::Reconnecting(_)) if self.sync_rt.fails >= FAILS_BEFORE_CANT_REACH => {
                Some(tr("Not connected").into())
            }
            Some(NetStatus::Reconnecting(_)) => Some(tr("Reconnecting\u{2026}").into()),
            Some(NetStatus::Refused(_)) => Some(tr("Not signed in").into()),
            Some(NetStatus::Gone(_)) => Some(tr("Deleted from the server").into()),
            _ => None,
        }
    }

    /// The server connection: start it when needed and take in what arrived.
    /// Returns true when the team's changes came in.
    pub(super) fn net_tick(&mut self, ctx: Option<&egui::Context>) -> bool {
        self.listing_tick();
        if self.sync_rt.net.is_none() {
            let (addr, mut sign_in) = if let Some(j) = &self.sync_rt.joining {
                (j.addr.clone(), j.how.sign_in(&j.device, self.data.me))
            } else if let Some(SharedCfg {
                server: Some(addr),
                device,
                tag,
                ..
            }) = &self.data.extra.shared
            {
                let mut si = match &self.sync_rt.resign {
                    Some(how) => how.sign_in(device, self.data.me),
                    None => SignIn {
                        device: device.clone(),
                        token: load_token(addr, device),
                        ..Default::default()
                    },
                };
                si.tag = Some(*tag);
                (addr.clone(), si)
            } else {
                return false;
            };
            // the server takes changes only from apps that know every field
            sign_in.app = Some(APP_VERSION.to_string());
            sign_in.sync = Some(sync::META_V);
            let wake: wb_net::Wake = match ctx {
                Some(c) => {
                    let c = c.clone();
                    std::sync::Arc::new(move || c.request_repaint())
                }
                None => std::sync::Arc::new(|| {}),
            };
            self.sync_rt.net_status = None;
            self.sync_rt.fails = 0;
            self.sync_rt.net = Some(wb_net::Client::start(&addr, sign_in, wake));
        }
        let events = self
            .sync_rt
            .net
            .as_ref()
            .map(|n| n.events())
            .unwrap_or_default();
        self.take_net_events(events)
    }

    /// What the server connection said since the last frame. Teammates'
    /// changes that came one after another are merged in together: each
    /// merge goes over the whole workspace, so a busy channel's messages
    /// arriving in one frame cost one merge instead of one each. They are
    /// all merged (a change carries only what changed, not the whole copy),
    /// with the last hidden list, which is the whole list as of then.
    /// Anything else the server said is taken after the changes before it.
    /// True when something new arrived.
    pub(super) fn take_net_events(&mut self, events: Vec<NetEvent>) -> bool {
        let mut changed = false;
        let mut updates = vec![];
        let mut hidden_now = vec![];
        for e in events {
            let e = match e {
                NetEvent::Update { data, hidden, .. } => {
                    updates.push(data);
                    hidden_now = hidden;
                    continue;
                }
                e => e,
            };
            if !updates.is_empty() {
                changed |= self.merge_in(&std::mem::take(&mut updates), &hidden_now);
            }
            match e {
                NetEvent::Status(s) => {
                    match &s {
                        NetStatus::Reconnecting(_) => self.sync_rt.fails += 1,
                        NetStatus::Connected => self.sync_rt.fails = 0,
                        _ => {}
                    }
                    if s != NetStatus::Connected {
                        // the list comes again after the next welcome
                        self.sync_rt.online = None;
                    }
                    if matches!(s, NetStatus::Gone(_)) && self.sync_rt.joining.is_none() {
                        self.toast(tr("This workspace was deleted from the server by its owner. Remove it from this computer in Work together."));
                    }
                    self.sync_rt.net_status = Some(s)
                }
                NetEvent::Welcome {
                    tag,
                    snapshot,
                    member,
                    role,
                    token,
                    email,
                    hidden,
                    ..
                } => {
                    if let Some(t) = token {
                        let (addr, device) = match (&self.sync_rt.joining, &self.data.extra.shared)
                        {
                            (Some(j), _) => (j.addr.clone(), j.device.clone()),
                            (None, Some(c)) => {
                                (c.server.clone().unwrap_or_default(), c.device.clone())
                            }
                            _ => continue,
                        };
                        save_token(&addr, &device, &t);
                    }
                    if self.sync_rt.resign.take().is_some() {
                        self.toast(tr("Signed in again."));
                    }
                    self.sync_rt.account = email;
                    self.sync_rt.form = Form::default();
                    self.sync_rt.role = Some(role);
                    // the server that let us in also publishes app updates
                    let server = self
                        .sync_rt
                        .joining
                        .as_ref()
                        .map(|j| j.addr.clone())
                        .or_else(|| {
                            self.data
                                .extra
                                .shared
                                .as_ref()
                                .and_then(|c| c.server.clone())
                        });
                    if let Some(addr) = server {
                        crate::updater::set_server(&addr);
                    }
                    changed |= self.net_welcome(tag, snapshot, member, &hidden);
                }
                NetEvent::Update { .. } => unreachable!("taken in above"),
                NetEvent::Invite {
                    member,
                    code,
                    emailed,
                    mail_error,
                } => {
                    if let Some(p) = self.sync_rt.people.iter_mut().find(|p| p.member == member) {
                        p.invite = Some(code);
                    }
                    let name = self
                        .data
                        .member(member)
                        .map(|m| m.name.clone())
                        .unwrap_or_default();
                    if let Some(to) = emailed {
                        self.toast(crate::i18n::particles(&trf!(
                            "Emailed {}'s invite code to {}.",
                            name,
                            to
                        )));
                    } else if let Some(why) = mail_error {
                        self.toast(trf!("Couldn't email {}'s invite code ({}). Copy it from Work together > People.", name, why));
                    }
                }
                NetEvent::Account(email) => {
                    self.toast(crate::i18n::particles(&trf!(
                        "Password saved. Sign in with {} on your other computers and phones.",
                        email
                    )));
                    self.sync_rt.account = Some(email);
                    self.sync_rt.acct = Form::default();
                }
                NetEvent::Access { people, role } => {
                    self.sync_rt.people = people;
                    self.sync_rt.role = Some(role);
                    self.send_auto_invites();
                }
                NetEvent::Denied(why) => {
                    self.huddle_denied_unnamed();
                    self.toast(server_text(&why));
                }
                NetEvent::PutBack(why) => {
                    // the team's version of what was put back comes with the next update
                    self.toast(server_text(&why));
                }
                NetEvent::CallDenied { channel, why } => {
                    self.huddle_denied(channel);
                    self.toast(server_text(&why));
                }
                NetEvent::CallToken {
                    channel,
                    url,
                    token,
                } => self.huddle_token(channel, url, token),
                NetEvent::Backups(list) => self.sync_rt.backups = Some(list),
                NetEvent::Presence(online) => self.sync_rt.online = Some(online),
                NetEvent::Restored(name) => {
                    let label = wb_net::Backup { name, bytes: 0 }.label();
                    self.toast(trf!(
                        "Restored the team's workspace to {}. Everyone gets it now.",
                        label
                    ));
                    self.sync_rt.restore_confirm = None;
                    // the copy from before the restore is a new backup
                    self.access_request(NetMsg::BackupList);
                }
            }
        }
        if !updates.is_empty() {
            changed |= self.merge_in(&updates, &hidden_now);
        }
        changed
    }

    /// Joined a server or set one up: the sign-in screen and the dialog
    /// that asked are done, and the server is filled in next time.
    fn signed_in_now(&mut self) {
        self.login = None;
        if self.data.extra.welcome {
            self.data.extra.welcome = false;
            self.save_local();
        }
        if matches!(
            self.modal,
            Some(Modal::Together { .. } | Modal::JoinTeam { .. })
        ) {
            self.modal = None;
        }
        if self.persist {
            if let Some(addr) = self
                .data
                .extra
                .shared
                .as_ref()
                .and_then(|c| c.server.clone())
            {
                super::login::set_last_server(&addr);
            }
        }
    }

    /// The server answered: finish joining, or catch up after a reconnect.
    fn net_welcome(
        &mut self,
        tag: u16,
        snapshot: Option<serde_json::Value>,
        member: u64,
        hidden: &[u64],
    ) -> bool {
        if let Some(j) = self.sync_rt.joining.take() {
            let cfg = SharedCfg {
                folder: String::new(),
                device: j.device,
                tag,
                server: Some(j.addr),
            };
            match snapshot {
                None if self.data.extra.shared.is_some() => {
                    // an empty server, but this workspace already goes with
                    // a team: it stays with that one (setting up a server
                    // starts from a workspace that is only on this computer)
                    self.sync_rt.net = None;
                    self.sync_rt.net_status = None;
                    self.sync_rt.role = None;
                    self.toast(tr("That server has no team yet. To set it up, open a workspace that's only on this computer and connect it from Work together."));
                }
                None => {
                    // an empty server: this workspace becomes the team's
                    self.data.extra.shared = Some(cfg);
                    self.data.extra.sync = Default::default();
                    self.sync_note_change();
                    self.write_shared();
                    self.save_local();
                    self.share_cached_files();
                    self.toast(tr(
                        "Connected. This workspace is now on the server for your team.",
                    ));
                    self.signed_in_now();
                }
                Some(base) => match self.join_from(base, cfg) {
                    Ok(()) => {
                        // what the workspace left knew about its own server
                        // (its people, backups, ...) doesn't come along; the
                        // connection that just let us in does
                        let rt = std::mem::take(&mut self.sync_rt);
                        self.sync_rt = SyncRt {
                            net: rt.net,
                            net_status: rt.net_status,
                            role: rt.role,
                            account: rt.account,
                            ..Default::default()
                        };
                        self.after_join();
                        // the invite code says who this is
                        self.become_member(Some(member), "", "");
                        self.sync_note_change();
                        self.write_shared();
                        self.save_local();
                        let name = self
                            .data
                            .member(member)
                            .map(|m| m.name.clone())
                            .unwrap_or_default();
                        self.toast(crate::i18n::particles(&trf!("Joined as {}.", name)));
                        self.signed_in_now();
                    }
                    Err(e) => {
                        self.sync_rt.net = None;
                        self.toast(e);
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
            Some(s) => self.merge_in(&[s], hidden),
            None => false,
        };
        // after (re)connecting our whole copy goes up, with anything made offline
        self.write_shared();
        changed
    }

    #[cfg(test)]
    /// Start connecting to a server: with an invite code, or with none to
    /// set up an empty server.
    pub(super) fn connect_server(&mut self, addr: &str, invite: &str) -> Result<(), String> {
        let how = if invite.trim().is_empty() {
            How::Claim {
                email: String::new(),
                password: String::new(),
            }
        } else {
            How::Invite {
                code: invite.to_string(),
                email: String::new(),
                password: String::new(),
            }
        };
        self.connect_with(addr, how)
    }

    /// Start connecting to a server from the dialog (shares this workspace
    /// there if the server is empty, else joins the team's workspace).
    pub(super) fn connect_with(&mut self, addr: &str, how: How) -> Result<(), String> {
        wb_net::server_url(addr)?;
        // email and password: the account may be in any of the server's
        // workspaces, or in several (listing_tick goes on from there)
        if let How::Account { email, password } = &how {
            let (tx, rx) = std::sync::mpsc::channel();
            let (a, e, p) = (addr.trim().to_string(), email.clone(), password.clone());
            std::thread::spawn(move || {
                let _ = tx.send(wb_net::list_workspaces(&a, &e, &p));
            });
            self.sync_rt.choices = None;
            self.sync_rt.listing = Some(Listing {
                rx,
                addr: addr.trim().to_string(),
                how,
            });
            return Ok(());
        }
        self.start_joining(addr, how);
        Ok(())
    }

    /// The server said which workspaces the account signs in to: join the
    /// one, or let the person choose.
    pub(super) fn listing_tick(&mut self) {
        if let Some(file) = self.sync_rt.open_local.take() {
            // the sign-in screen goes: this workspace is signed in already
            self.login = None;
            if self.data.extra.welcome {
                self.data.extra.welcome = false;
                self.save_local();
            }
            if matches!(self.modal, Some(Modal::JoinTeam { .. })) {
                self.modal = None;
            }
            self.switch_workspace(&file);
            return;
        }
        let Some(l) = &self.sync_rt.listing else {
            return;
        };
        let answer = match l.rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err("The server had a problem. Try again.".into())
            }
        };
        let how = self.sync_rt.listing.take().expect("checked above").how;
        match answer {
            Err(e) => self.sync_rt.form.note = e,
            Ok(list) if list.is_empty() => {
                self.sync_rt.form.note =
                    "That email and password don't match an account on this server.".into()
            }
            Ok(list) if list.len() == 1 => self.open_or_join(&list[0].0, how),
            Ok(list) => self.sync_rt.choices = Some((list, how)),
        }
    }

    /// Join the team workspace at `addr`, or open it when it is already on
    /// this computer.
    pub(super) fn open_or_join(&mut self, addr: &str, how: How) {
        self.sync_rt.choices = None;
        let on_it = self
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.as_deref())
            .is_some_and(|a| wb_net::same_workspace(a, addr));
        if !on_it {
            if let Some(file) = local_workspace_on(addr, &self.ws_file) {
                // opened from listing_tick, outside the sign-in screen's drawing
                self.sync_rt.open_local = Some(file);
                return;
            }
        }
        self.start_joining(addr, how);
    }

    fn start_joining(&mut self, addr: &str, how: How) {
        if let How::Claim { email, .. } = &how {
            // the owner's email goes up with the workspace
            let me = self.data.me;
            if let Some(m) = self
                .data
                .members
                .iter_mut()
                .find(|m| m.id == me && m.email.trim().is_empty() && !email.trim().is_empty())
            {
                m.email = email.trim().to_string();
                self.dirty = true;
            }
        }
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
        self.sync_rt.joining = Some(Joining {
            addr: addr.trim().to_string(),
            device: format!("{:08x}", sync::random_u64() as u32),
            how,
        });
    }

    #[cfg(test)]
    /// Sign this computer in again with a new invite code.
    pub(super) fn sign_in_again(&mut self, code: &str) {
        if code.trim().is_empty() {
            return;
        }
        self.sign_in_with(How::Invite {
            code: code.to_string(),
            email: String::new(),
            password: String::new(),
        });
    }

    /// Sign this computer in again: email and password, a code, a reset.
    pub(super) fn sign_in_with(&mut self, how: How) {
        self.sync_rt.resign = Some(how);
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
    }

    /// Ask for the codes of people just invited, once the server knows them.
    fn send_auto_invites(&mut self) {
        if self.sync_rt.auto_invite.is_empty() || !self.sync_rt.role.is_some_and(|r| r.manages()) {
            return;
        }
        let known: Vec<u64> = self.sync_rt.people.iter().map(|p| p.member).collect();
        let (ready, wait): (Vec<_>, Vec<_>) = self
            .sync_rt
            .auto_invite
            .drain(..)
            .partition(|(m, _)| known.contains(m));
        self.sync_rt.auto_invite = wait;
        for (member, role) in ready {
            self.access_request(NetMsg::CreateInvite { member, role });
        }
    }

    /// Open what a workboard://join link (or a pasted invite) asks for.
    pub(super) fn open_join_link(&mut self, text: &str) {
        let j = wb_net::parse_join(text);
        let form = &mut self.sync_rt.form;
        form.mode = Mode::Invite;
        form.code = j.code.clone();
        form.email = j.email.clone().unwrap_or_default();
        form.note.clear();
        let on_server = self
            .data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.is_some());
        let addr = j.server.unwrap_or_default();
        if self.login.is_some() {
            // on the sign-in screen: it fills in the invite there
            self.open_login(super::login::LoginPage::Invite, &addr);
        } else if on_server && matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
            // this workspace's server let it go: the code is to sign back in
            self.modal = Some(self.together_modal());
        } else {
            // another team (or the same one as someone else): it opens as a
            // new workspace and this one stays as it is
            self.modal = Some(Modal::JoinTeam { addr });
        }
    }

    /// Open the form for joining a team from the workspace menu. Whatever
    /// this workspace is (on a server, in a folder or only here), the team's
    /// workspace opens as a new one beside it.
    pub(super) fn open_join_team(&mut self, addr: String) {
        if self.sync_rt.joining.is_none() {
            self.sync_rt.form = Form {
                mode: Mode::Account,
                ..Default::default()
            };
        }
        self.modal = Some(Modal::JoinTeam { addr });
    }

    /// Send an access request to the server (owners and admins).
    pub(super) fn access_request(&mut self, msg: NetMsg) {
        match &self.sync_rt.net {
            Some(n) if self.sync_rt.net_status == Some(NetStatus::Connected) => {
                if !matches!(msg, NetMsg::CallJoin { .. }) {
                    self.sync_rt.other_asked = Some(std::time::Instant::now());
                }
                n.send(msg)
            }
            _ => self.toast(tr("Not connected to the server right now.")),
        }
    }

    /// Once per frame: write our copy when it changed, look for teammates' changes.
    pub(super) fn sync_tick(&mut self, ctx: &egui::Context) {
        if self.sync_rt.listing.is_some() || self.sync_rt.open_local.is_some() {
            self.listing_tick();
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if self.sync_rt.joining.is_some() {
            self.net_tick(Some(ctx));
            return;
        }
        let Some(cfg) = self.data.extra.shared.clone() else {
            return;
        };
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
            .filter_map(|v| {
                v.get("extra")?
                    .get("shared")?
                    .get("tag")?
                    .as_u64()
                    .map(|t| t as u16)
            })
            .collect();
        let mut tag = (sync::random_u64() % 1024) as u16;
        while used.contains(&tag) {
            tag = (tag + 1) % 1024;
        }
        SharedCfg {
            folder: folder.to_string(),
            device: format!("{:08x}", sync::random_u64() as u32),
            tag,
            server: None,
        }
    }

    /// Share the open workspace in an (empty) folder.
    pub(super) fn share_here(&mut self, folder: &str) -> Result<(), String> {
        let folder = folder.trim();
        if folder.is_empty() {
            return Err(tr("Choose a folder first.").into());
        }
        std::fs::create_dir_all(folder).map_err(|e| trf!("Couldn't use that folder: {}", e))?;
        self.data.extra.shared = Some(Self::new_cfg(folder));
        self.data.extra.sync = Default::default();
        self.sync_note_change();
        self.write_shared();
        self.save_local();
        self.sync_rt = SyncRt::default();
        self.share_cached_files();
        Ok(())
    }

    /// Join the workspace someone shared in a folder. It opens as a new
    /// workspace, so the one you had stays in the workspace menu.
    pub(super) fn join_shared(&mut self, folder: &str) -> Result<(), String> {
        let folder = folder.trim();
        let files = sync::other_files(Path::new(folder), "");
        let newest = files
            .iter()
            .max_by_key(|(_, m)| *m)
            .ok_or(tr("No shared workspace in that folder yet."))?;
        let base =
            sync::read_json(&newest.0).ok_or(tr("The shared workspace couldn't be read."))?;
        self.join_from(base, Self::new_cfg(folder))?;
        self.sync_rt = SyncRt::default();
        self.sync_pull();
        self.after_join();
        Ok(())
    }

    /// Open the team's copy as a new workspace on this computer.
    fn join_from(&mut self, base: serde_json::Value, cfg: SharedCfg) -> Result<(), String> {
        serde_json::from_value::<Data>(base.clone())
            .map_err(|_| tr("The shared workspace couldn't be read.").to_string())?;
        // the team's things with this person's own settings (theme, notifications,
        // pinned tools...) instead of the owner's
        let mut d: Data = serde_json::from_value(sync::with_team_things(&self.data_value(), &base))
            .map_err(|_| tr("The shared workspace couldn't be read.").to_string())?;
        // personal things that point into the old workspace start empty
        d.favorites.clear();
        d.extra.recent.clear();
        d.extra.notepad.clear();
        d.extra.reminders.clear();
        d.extra.trash.clear();
        d.extra.inbox_cleared.clear();
        d.extra.inbox_later.clear();
        d.extra.starred.clear();
        d.extra.closed_dms.clear();
        // Slack matches this computer carried belong to the workspace left;
        // the team's own carried ones (no stamp) count as carried here
        d.extra.slack_carried.clear();
        d.note_carried_slack(&Default::default());
        d.extra.shared = Some(cfg);
        d.extra.welcome = false;
        d.extra.sync.hashes.clear();
        d.extra.sync.field_hashes.clear();
        d.me = 0;
        // a status editor still open: its edits are done (and saved with the
        // workspace being left) like on closing it, and it doesn't stay open
        // on the joined workspace, where it would put its names on a space
        // of the same id
        self.finish_status_edit();
        // the workspace left was chosen past the sign-in screen
        self.data.extra.welcome = false;
        if self.persist {
            self.save_now();
            let mut list = Data::workspaces();
            if list.is_empty() {
                list.push(WsEntry {
                    name: self.data.workspace.clone(),
                    file: self.ws_file.clone(),
                });
            }
            // the same team workspace joined again (signed out, or signed
            // in once more): its copy here is replaced, not added to
            let again = d
                .extra
                .shared
                .as_ref()
                .and_then(|c| c.server.as_deref())
                .and_then(|a| local_workspace_on(a, ""))
                .filter(|f| list.iter().any(|e| &e.file == f));
            let file = match again {
                Some(f) => {
                    if let Some(e) = list.iter_mut().find(|e| e.file == f) {
                        e.name = d.workspace.clone();
                    }
                    f
                }
                None => {
                    let file = format!("workspace-{}.json", now_ts());
                    list.push(WsEntry {
                        name: d.workspace.clone(),
                        file: file.clone(),
                    });
                    file
                }
            };
            Data::save_workspaces(&list);
            Data::set_last_workspace(&file);
            self.ws_places.clear();
            self.ws_file = file;
        }
        self.data = d;
        self.data_changed();
        // a huddle in the workspace left ends there (its conversation isn't here)
        self.leave_huddle();
        Ok(())
    }

    fn after_join(&mut self) {
        let v = self.data_value();
        self.data.extra.sync.hashes = sync::fingerprints(&v);
        self.data.extra.sync.field_hashes = sync::part_hashes(&v);
        // the team's settings and Slack matches as they came (the copy
        // teammates send leaves these out): a change to one, even the
        // first thing done here, is then noted as one (as the iPhone does)
        self.data.extra.sync.value_hashes = sync::value_hashes(&v);
        self.loc = self
            .data
            .first_list()
            .map(Location::List)
            .unwrap_or(Location::Home);
        self.section = Section::Home;
        self.open_task = None;
        self.chat_open = None;
        self.recent.clear();
        self.clear_history();
        // a workspace I just joined: what is in it already isn't news
        self.notify_rt.seeded = false;
        self.save_local();
    }

    pub(super) fn stop_sharing(&mut self) {
        self.leave_huddle();
        self.ws_places.clear();
        if let Some(SharedCfg {
            server: Some(addr),
            device,
            ..
        }) = &self.data.extra.shared
        {
            forget_token(addr, device);
        }
        self.data.extra.shared = None;
        self.data.extra.sync = Default::default();
        self.sync_rt = SyncRt::default();
        self.dirty = true;
    }

    /// Keep this workspace on its server, reached at another address (the
    /// server moved, or the address was mistyped), and connect again. The
    /// sign-in goes along.
    pub(super) fn change_server(&mut self, addr: &str) -> Result<(), String> {
        wb_net::server_url(addr)?;
        let new = addr.trim().to_string();
        let Some(cfg) = self.data.extra.shared.as_mut() else {
            return Ok(());
        };
        let Some(old) = cfg.server.clone() else {
            return Ok(());
        };
        if old != new {
            if let Some(token) = load_token(&old, &cfg.device) {
                save_token(&new, &cfg.device, &token);
                forget_token(&old, &cfg.device);
            }
            cfg.server = Some(new);
            self.save_local();
            self.dirty = true;
        }
        self.sync_rt.editing_addr = false;
        self.sync_rt.net = None;
        self.sync_rt.net_status = None;
        Ok(())
    }

    /// Who you are in a joined workspace: an existing (invited) member, or someone new.
    pub(super) fn become_member(&mut self, id: Option<u64>, name: &str, email: &str) {
        let id = match id {
            Some(id) => id,
            None => {
                let id = self.data.new_id();
                let color = STATUS_PALETTE[1 + (id % 13) as usize];
                self.data.members.push(Member {
                    id,
                    name: name.trim().to_string(),
                    color,
                    email: email.trim().to_string(),
                    role: "Member".into(),
                    ..Default::default()
                });
                id
            }
        };
        if let Some(m) = self.data.members.iter_mut().find(|m| m.id == id) {
            m.invited = false;
        }
        self.data.me = id;
        for c in &mut self.data.channels {
            if c.kind == ChannelKind::Channel && !c.private && !c.members.contains(&id) {
                c.members.push(id);
            }
        }
        self.dirty = true;
    }

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
            Some(cfg) if cfg.server.is_none() && !cfg.folder.is_empty() => {
                Path::new(&cfg.folder).join("Docs archive")
            }
            _ => {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                Path::new(&home)
                    .join("Documents")
                    .join("Workboard Docs archive")
            }
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
            .map(|m| m.name.clone())
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

    // ------------------------------------------------------------ dialogs

    /// The owner: go back to an earlier copy of the team's workspace.
    fn backups_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("Backups")).strong());
        ui.label(RichText::new(tr("Go back to an earlier copy of the team's workspace. Deleted items come back; anything made since stays. The server keeps a copy from just before.")).small().color(t.text3));
        let Some(list) = self.sync_rt.backups.clone() else {
            if ui.button(tr("Show backups")).clicked() {
                self.access_request(NetMsg::BackupList);
            }
            return;
        };
        if list.is_empty() {
            ui.label(RichText::new(tr("No backups yet. The first one is made within a minute of the server having a workspace.")).color(t.text2));
            return;
        }
        let mut req: Option<NetMsg> = None;
        let mut confirm = self.sync_rt.restore_confirm.clone();
        egui::ScrollArea::vertical()
            .id_salt("backups")
            .max_height(160.0)
            .show(ui, |ui| {
                for b in list.iter().take(80) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(b.label()).color(t.text));
                        ui.label(
                            RichText::new(format!("{} KB", b.bytes.div_ceil(1024)))
                                .small()
                                .color(t.text3),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if confirm.as_deref() == Some(b.name.as_str()) {
                                if row_button(ui, tr("Cancel")).clicked() {
                                    confirm = None;
                                }
                                if row_button(ui, RichText::new(tr("Yes, restore")).color(t.danger))
                                    .clicked()
                                {
                                    req = Some(NetMsg::Restore {
                                        name: b.name.clone(),
                                    });
                                }
                            } else if row_button(ui, tr("Restore"))
                                .on_hover_text(trf!("Go back to {}", b.label()))
                                .clicked()
                            {
                                confirm = Some(b.name.clone());
                            }
                        });
                    });
                }
            });
        self.sync_rt.restore_confirm = confirm;
        if let Some(m) = req {
            self.access_request(m);
        }
    }

    /// Whether I may bring new people into the workspace and change its
    /// name and colour. On a team server
    /// only its owner and admins: what the server last said, or before it
    /// is reached the role in People (which follows the server's). Alone or
    /// in a shared folder anyone may, as the People page there lets anyone
    /// manage people.
    pub(super) fn may_manage(&self) -> bool {
        let server = self
            .data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.is_some());
        if !server {
            return true;
        }
        match self.sync_rt.role {
            Some(r) => r.manages(),
            None => self
                .data
                .member(self.data.me)
                .is_some_and(|m| m.role == "Owner" || m.role == "Admin"),
        }
    }

    /// Owners and admins: everyone's access, invite codes, roles.
    fn people_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("People")).strong());
        ui.label(RichText::new(tr("Give each person their invite code (it works once, for 7 days). Add people first with Invite.")).small().color(t.text3));
        let me = self.data.me;
        let my_role = self.sync_rt.role.unwrap_or_default();
        let people = self.sync_rt.people.clone();
        let mut req: Option<NetMsg> = None;
        egui::ScrollArea::vertical()
            .id_salt("people")
            .max_height(220.0)
            .show(ui, |ui| {
                for p in &people {
                    let Some(m) = self.data.member(p.member) else {
                        continue;
                    };
                    let name = m.name.clone();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&name).color(t.text));
                        ui.label(RichText::new(tr(p.role.label())).small().color(t.text2));
                        ui.label(
                            RichText::new(match p.devices {
                                0 => tr("not signed in").to_string(),
                                1 => tr("1 computer").to_string(),
                                n => trf!("{} computers", n),
                            })
                            .small()
                            .color(t.text3),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            // another admin is the owner's to change (as the server)
                            let others_admin = p.role == Role::Admin && my_role != Role::Owner;
                            if p.role != Role::Owner && p.member != me && !others_admin {
                                if row_button(
                                    ui,
                                    RichText::new(tr("Remove access")).color(t.danger),
                                )
                                .on_hover_text(trf!("Sign out {}'s computers", name))
                                .clicked()
                                {
                                    req = Some(NetMsg::Revoke { member: p.member });
                                }
                                if my_role.manages() {
                                    let (label, role) = if p.role == Role::Admin {
                                        ("Make member", Role::Member)
                                    } else {
                                        ("Make admin", Role::Admin)
                                    };
                                    if row_button(ui, tr(label)).clicked() {
                                        req = Some(NetMsg::SetRole {
                                            member: p.member,
                                            role,
                                        });
                                    }
                                }
                            }
                            // a code signs in as its person: the owner makes any, an
                            // admin only for members without a password (as the server)
                            let may_manage = p.member != me
                                && (my_role == Role::Owner
                                    || (p.role == Role::Member && !p.account));
                            if may_manage
                                && row_button(
                                    ui,
                                    if p.invite.is_some() {
                                        tr("New code")
                                    } else {
                                        tr("Invite code")
                                    },
                                )
                                .clicked()
                            {
                                req = Some(NetMsg::CreateInvite {
                                    member: p.member,
                                    role: Role::Member,
                                });
                            }
                        });
                    });
                    if let Some(code) = &p.invite {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(trf!("Code for {}: {}", name, code))
                                    .monospace()
                                    .color(t.text),
                            );
                            if row_button(ui, tr("Copy")).clicked() {
                                ui.ctx().copy_text(code.clone());
                                self.toast(tr("Invite code copied"));
                            }
                        });
                    }
                }
            });
        if let Some(m) = req {
            self.access_request(m);
        }
    }

    /// Server address, sign-in fields, and Connect (or the connection's
    /// progress and Cancel while it tries).
    pub(super) fn server_form(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        addr: &mut String,
        modes: &[Mode],
        button: &str,
    ) {
        ui.label(RichText::new(tr("Server")).color(t.text2));
        let r = ui.add(
            field(addr)
                .hint_text("https://wb.example.com")
                .desired_width(f32::INFINITY),
        );
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Server address")
        });
        if self.join_progress(ui, t) {
            return;
        }
        ui.add_space(6.0);
        if let Some(how) = self.login_form(ui, t, addr, modes, button, false) {
            if let Err(e) = self.connect_with(addr, how) {
                self.sync_rt.form.note = e;
            }
        }
    }

    /// While connecting from a sign-in form: how it goes, and Cancel. True
    /// while it connects; a refusal goes back to the form with the reason.
    pub(super) fn join_progress(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        if self.sync_rt.listing.is_some() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("Looking up your workspaces...")).color(t.text2));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        self.sync_rt.listing = None;
                    }
                });
            });
            return self.sync_rt.listing.is_some();
        }
        if let Some((list, how)) = self.sync_rt.choices.clone() {
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr("Choose a workspace"))
                    .size(15.0)
                    .strong()
                    .color(t.text),
            );
            ui.label(
                RichText::new(tr("Your account is in these workspaces on this server. You can open the others later the same way."))
                    .small()
                    .color(t.text2),
            );
            ui.add_space(6.0);
            for (addr, name) in &list {
                let label = if name.trim().is_empty() {
                    addr.clone()
                } else {
                    name.clone()
                };
                let r = ui.add(
                    egui::Button::new(RichText::new(label).size(15.0))
                        .min_size(vec2(ui.available_width(), 36.0)),
                );
                if r.clicked() {
                    self.open_or_join(addr, how.clone());
                }
            }
            ui.add_space(6.0);
            if ui.button(tr("Cancel")).clicked() {
                self.sync_rt.choices = None;
            }
            return self.sync_rt.choices.is_some() || self.sync_rt.joining.is_some();
        }
        if self.sync_rt.joining.is_some() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let status = net_status_text(
                    self.sync_rt.net.is_some(),
                    self.sync_rt.net_status.as_ref(),
                    self.sync_rt.fails,
                );
                ui.label(RichText::new(status).color(t.text2));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        self.sync_rt.joining = None;
                        self.sync_rt.net = None;
                        self.sync_rt.net_status = None;
                    }
                });
            });
            if let Some(why) = net_problem(self.sync_rt.net_status.as_ref())
                .filter(|_| matches!(self.sync_rt.net_status, Some(NetStatus::Reconnecting(_))))
            {
                ui.label(RichText::new(why).small().color(WARN));
            }
            // refused: say why and let them fix what they typed
            if let Some(NetStatus::Refused(why) | NetStatus::Gone(why)) =
                self.sync_rt.net_status.clone()
            {
                self.sync_rt.joining = None;
                self.sync_rt.net = None;
                self.sync_rt.net_status = None;
                self.sync_rt.form.note = why;
            }
            return self.sync_rt.joining.is_some();
        }
        false
    }

    /// The sign-in fields for one of `modes` (with tabs when there are
    /// several, and "Forgot password?" under the password). The way to sign
    /// in when `button` is clicked and everything needed is there. `big`:
    /// the sign-in screen's look (bold names, tall fields, a button as wide
    /// as the fields, "Forgot password?" beside the password's name).
    pub(super) fn login_form(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        addr: &mut String,
        modes: &[Mode],
        button: &str,
        big: bool,
    ) -> Option<How> {
        let mut form = std::mem::take(&mut self.sync_rt.form);
        if !modes.contains(&form.mode)
            && !(form.mode == Mode::Forgot && modes.contains(&Mode::Account))
        {
            form.mode = modes[0];
        }
        if modes.len() > 1 {
            ui.horizontal(|ui| {
                for m in modes {
                    let label = match m {
                        Mode::Account => "Email and password",
                        Mode::Invite => "Invite code",
                        Mode::Forgot => "Forgot password",
                        Mode::NewServer => "New server",
                    };
                    let on = form.mode == *m || (form.mode == Mode::Forgot && *m == Mode::Account);
                    if ui.selectable_label(on, tr(label)).clicked() && form.mode != *m {
                        // the email carries over; passwords and codes don't
                        form = Form {
                            mode: *m,
                            email: std::mem::take(&mut form.email),
                            ..Default::default()
                        };
                    }
                }
            });
            ui.add_space(4.0);
        }
        let ctx = ui.ctx().clone();
        let name_text = |label: &str| {
            if big {
                RichText::new(tr(label))
                    .font(super::bold_font(&ctx, 15.0))
                    .color(t.text)
            } else {
                RichText::new(tr(label)).color(t.text2)
            }
        };
        let edit = |ui: &mut Ui, text: &mut String, label: &str, hint: &str, secret: bool| {
            if secret {
                secret_edit(ui, t, text, label, hint, big);
                return;
            }
            let mut te = field(text)
                .hint_text(tr(hint))
                .password(secret)
                .desired_width(f32::INFINITY);
            if big {
                te = te
                    .font(FontId::proportional(17.0))
                    .margin(egui::Margin::symmetric(14, 12))
                    .min_size(vec2(0.0, super::login::FIELD_H));
            }
            let r = ui.add(te);
            let name = format!("{} entry", label);
            r.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name)
            });
        };
        let field = |ui: &mut Ui, text: &mut String, label: &str, hint: &str, secret: bool| {
            // a gap above each name, so a name groups with its own field
            ui.add_space(if big { 12.0 } else { 4.0 });
            ui.label(name_text(label));
            if big {
                ui.add_space(2.0);
            }
            edit(ui, text, label, hint, secret);
        };
        let mut go = false;
        let mut how = None;
        match form.mode {
            Mode::Account => {
                field(ui, &mut form.email, "Email", "you@company.com", false);
                let forgot = if big {
                    ui.add_space(12.0);
                    let forgot = ui
                        .horizontal(|ui| {
                            ui.label(name_text("Password"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.link(tr("Forgot password?")).clicked()
                            })
                            .inner
                        })
                        .inner;
                    ui.add_space(2.0);
                    edit(ui, &mut form.password, "Password", "", true);
                    forgot
                } else {
                    field(ui, &mut form.password, "Password", "", true);
                    ui.link(tr("Forgot password?")).clicked()
                };
                if forgot {
                    form.mode = Mode::Forgot;
                    form.sent = false;
                    form.note.clear();
                }
            }
            Mode::Invite => {
                field(
                    ui,
                    &mut form.code,
                    "Invite code",
                    "XXXX-XXXX-XXXX, or paste the invite link",
                    false,
                );
                // a pasted link (or a whole invite mail) fills in everything it has
                if form.code.contains("workboard://") {
                    let j = wb_net::parse_join(&form.code);
                    form.code = j.code;
                    if let Some(e) = j.email {
                        form.email = e;
                    }
                    if let Some(s) = j.server {
                        *addr = s;
                    }
                }
                field(
                    ui,
                    &mut form.email,
                    "Email",
                    "the email you were invited with",
                    false,
                );
                field(
                    ui,
                    &mut form.password,
                    "Choose a password",
                    &trf!("at least {} characters", wb_net::MIN_PASSWORD),
                    true,
                );
                field(
                    ui,
                    &mut form.repeat,
                    "Repeat password",
                    "the same password again",
                    true,
                );
                ui.label(RichText::new(tr("You sign in with this email and password on your other computers and phones.")).small().color(t.text3));
            }
            Mode::Forgot => {
                field(ui, &mut form.email, "Email", "you@company.com", false);
                if let Some(rx) = &self.sync_rt.forgot {
                    match rx.try_recv() {
                        Ok(Ok(true)) => {
                            form.sent = true;
                            form.note = trf!("If {} has an account on this server, a reset code is on its way. It works for 30 minutes.", form.email.trim());
                            self.sync_rt.forgot = None;
                        }
                        Ok(Ok(false)) => {
                            form.note = tr("This server can't send email. Ask the owner or an admin for a new invite code instead.").into();
                            self.sync_rt.forgot = None;
                        }
                        Ok(Err(e)) => {
                            form.note = e;
                            self.sync_rt.forgot = None;
                        }
                        Err(_) => {
                            ui.label(RichText::new(tr("Asking the server\u{2026}")).color(t.text2));
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_millis(200));
                        }
                    }
                }
                if self.sync_rt.forgot.is_none()
                    && ui
                        .button(tr(if form.sent {
                            "Email me another code"
                        } else {
                            "Email me a code"
                        }))
                        .clicked()
                {
                    if wb_net::server_url(addr).is_err() {
                        form.note = tr("Type the server's address first.").into();
                    } else if !form.email.contains('@') {
                        form.note = tr("Type the email you sign in with.").into();
                    } else {
                        let (tx, rx) = std::sync::mpsc::channel();
                        let (a, e, ctx) = (addr.clone(), form.email.clone(), ui.ctx().clone());
                        std::thread::spawn(move || {
                            let _ = tx.send(wb_net::forgot_password(&a, &e));
                            ctx.request_repaint();
                        });
                        self.sync_rt.forgot = Some(rx);
                        form.note.clear();
                    }
                }
                if form.sent {
                    field(
                        ui,
                        &mut form.code,
                        "Reset code",
                        "XXXX-XXXX-XXXX from the mail",
                        false,
                    );
                    field(
                        ui,
                        &mut form.password,
                        "New password",
                        &trf!("at least {} characters", wb_net::MIN_PASSWORD),
                        true,
                    );
                    field(
                        ui,
                        &mut form.repeat,
                        "Repeat password",
                        "the same password again",
                        true,
                    );
                }
                if ui.link(tr("Back to sign in")).clicked() {
                    form.mode = Mode::Account;
                    form.note.clear();
                }
            }
            Mode::NewServer => {
                ui.label(RichText::new(tr("This workspace goes up to the empty server and you become its owner. Choose how you sign in on your other computers and phones.")).small().color(t.text3));
                field(ui, &mut form.email, "Email", "you@company.com", false);
                field(
                    ui,
                    &mut form.password,
                    "Choose a password",
                    &trf!("at least {} characters", wb_net::MIN_PASSWORD),
                    true,
                );
                field(
                    ui,
                    &mut form.repeat,
                    "Repeat password",
                    "the same password again",
                    true,
                );
            }
        }
        if !form.note.is_empty() {
            ui.add_space(if big { 8.0 } else { 0.0 });
            ui.label(RichText::new(server_text(&form.note)).color(t.danger));
        }
        ui.add_space(if big { 20.0 } else { 6.0 });
        let shown = form.mode != Mode::Forgot || form.sent;
        let label = tr(if form.mode == Mode::Forgot {
            "Set password and sign in"
        } else {
            button
        });
        if shown && big {
            go = super::login::big_button(ui, t, label).clicked();
        } else if shown {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    go = primary_button(ui, t, label).clicked();
                });
            });
        }
        if go {
            let email_ok = form.email.trim().contains('@');
            let problem = match form.mode {
                Mode::Account if !email_ok => Some(tr("Type your email.").to_string()),
                Mode::Account if form.password.is_empty() => {
                    Some(tr("Type your password.").to_string())
                }
                Mode::Invite if form.code.trim().is_empty() => {
                    Some(tr("Type your invite code.").to_string())
                }
                Mode::Invite if !form.email.trim().is_empty() && !email_ok => {
                    Some(tr("That email doesn't look right.").to_string())
                }
                Mode::Forgot if form.code.trim().is_empty() => {
                    Some(tr("Type the code from the mail.").to_string())
                }
                Mode::NewServer if !email_ok => Some(tr("Type your email.").to_string()),
                Mode::Invite | Mode::Forgot | Mode::NewServer => {
                    new_password_problem(&form.password, &form.repeat)
                }
                _ => None,
            };
            match problem {
                Some(p) => form.note = p,
                None => {
                    form.note.clear();
                    how = Some(match form.mode {
                        Mode::Account => How::Account {
                            email: form.email.clone(),
                            password: form.password.clone(),
                        },
                        Mode::Invite => How::Invite {
                            code: form.code.clone(),
                            email: form.email.clone(),
                            password: form.password.clone(),
                        },
                        Mode::Forgot => How::Reset {
                            email: form.email.clone(),
                            code: form.code.clone(),
                            password: form.password.clone(),
                        },
                        Mode::NewServer => How::Claim {
                            email: form.email.clone(),
                            password: form.password.clone(),
                        },
                    });
                }
            }
        }
        self.sync_rt.form = form;
        how
    }

    /// Signed in on a server: the email you sign in with elsewhere, setting
    /// up a password or changing it.
    fn account_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("Your account")).strong());
        let mut form = std::mem::take(&mut self.sync_rt.acct);
        let field = |ui: &mut Ui, text: &mut String, label: &str, secret: bool| {
            ui.horizontal(|ui| {
                // names start at one left edge (add_sized centred them), and
                // the field takes the rest of the row
                ui.allocate_ui_with_layout(
                    vec2(130.0, CONTROL_H),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_min_width(130.0);
                        ui.label(RichText::new(tr(label)).color(t.text2));
                    },
                );
                if secret {
                    secret_edit(ui, t, text, label, "", false);
                    return;
                }
                let r = ui.add(field(text).desired_width(f32::INFINITY));
                let name = format!("{} entry", label);
                r.widget_info(move || {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name)
                });
            });
        };
        let mut send = None;
        match self.sync_rt.account.clone() {
            Some(email) => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(crate::i18n::particles(&trf!(
                            "You sign in with {} on your other computers and phones.",
                            email
                        )))
                        .color(t.text2),
                    );
                    if !form.changing && row_button(ui, tr("Change password")).clicked() {
                        form.changing = true;
                        form.note.clear();
                    }
                });
                if form.changing {
                    field(ui, &mut form.old, "Current password", true);
                    field(ui, &mut form.password, "New password", true);
                    field(ui, &mut form.repeat, "Repeat password", true);
                    ui.horizontal(|ui| {
                        if primary_button(ui, t, tr("Save password")).clicked() {
                            match new_password_problem(&form.password, &form.repeat) {
                                Some(p) => form.note = p,
                                None => {
                                    send = Some(NetMsg::SetPassword {
                                        email: email.clone(),
                                        password: form.password.clone(),
                                        old_password: Some(form.old.clone()),
                                    })
                                }
                            }
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            form = Form::default();
                        }
                    });
                }
            }
            None => {
                ui.label(RichText::new(tr("Set a password to sign in on your other computers and phones without a new invite code.")).color(t.text2));
                if form.email.is_empty() {
                    form.email = self
                        .data
                        .member(self.data.me)
                        .map(|m| m.email.clone())
                        .unwrap_or_default();
                }
                field(ui, &mut form.email, "Email", false);
                field(ui, &mut form.password, "Password", true);
                field(ui, &mut form.repeat, "Repeat password", true);
                if primary_button(ui, t, tr("Save password")).clicked() {
                    if !form.email.trim().contains('@') {
                        form.note = tr("Type your email.").into();
                    } else {
                        match new_password_problem(&form.password, &form.repeat) {
                            Some(p) => form.note = p,
                            None => {
                                send = Some(NetMsg::SetPassword {
                                    email: form.email.trim().to_string(),
                                    password: form.password.clone(),
                                    old_password: None,
                                })
                            }
                        }
                    }
                }
            }
        }
        if !form.note.is_empty() {
            ui.label(RichText::new(server_text(&form.note)).color(t.danger));
        }
        self.sync_rt.acct = form;
        if let Some(m) = send {
            self.access_request(m);
        }
    }

    /// Joining a team while this computer already has a workspace (the
    /// workspace menu's "Join a team", or an invite link): the server
    /// address and sign-in or invite code. The team's workspace opens as a
    /// new one and the others stay as they are.
    pub(super) fn join_team_body(&mut self, ui: &mut Ui, t: &Theme, addr: &mut String) -> bool {
        ui.label(RichText::new(tr("Type your team's server address. Then sign in with your email and password, or use the invite code you were sent.")).color(t.text2));
        ui.label(
            RichText::new(tr("The team's workspace opens as a new workspace. The ones you already have stay in the workspace menu, as they are."))
                .small()
                .color(t.text3),
        );
        ui.add_space(4.0);
        self.server_form(ui, t, addr, &[Mode::Account, Mode::Invite], "Connect");
        false
    }

    /// The Work together dialog, filled in with what this workspace uses.
    pub(super) fn together_modal(&self) -> Modal {
        let cfg = self.data.extra.shared.as_ref();
        Modal::Together {
            path: cfg
                .filter(|c| c.server.is_none())
                .map(|c| c.folder.clone())
                .unwrap_or_default(),
            addr: cfg.and_then(|c| c.server.clone()).unwrap_or_default(),
            code: String::new(),
            result: String::new(),
        }
    }

    pub(super) fn together_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        path: &mut String,
        addr: &mut String,
        code: &mut String,
        result: &mut String,
    ) -> bool {
        let mut close = false;
        if let Some(SharedCfg {
            server: Some(server),
            ..
        }) = self.data.extra.shared.clone()
        {
            ui.label(
                RichText::new(tr("This workspace is on a server"))
                    .size(15.0)
                    .strong(),
            );
            ui.label(RichText::new(&server).color(t.text2));
            let connected = self.sync_rt.net_status == Some(NetStatus::Connected);
            let status = net_status_text(
                self.sync_rt.net.is_some(),
                self.sync_rt.net_status.as_ref(),
                self.sync_rt.fails,
            );
            let trouble = matches!(
                self.sync_rt.net_status,
                Some(NetStatus::Reconnecting(_) | NetStatus::Refused(_) | NetStatus::Gone(_))
            );
            ui.label(
                RichText::new(trf!("Status: {}", status)).color(if connected {
                    t.text
                } else if trouble {
                    WARN
                } else {
                    t.text2
                }),
            );
            if let Some(why) = net_problem(self.sync_rt.net_status.as_ref()) {
                ui.label(RichText::new(why).small().color(t.text2));
            }
            if let Some(role) = self.sync_rt.role.filter(|_| connected) {
                let name = self
                    .data
                    .member(self.data.me)
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                ui.label(
                    RichText::new(crate::i18n::particles(&trf!(
                        "Signed in as {} ({})",
                        name,
                        tr(role.label())
                    )))
                    .color(t.text2),
                );
            }
            let gone = matches!(self.sync_rt.net_status, Some(NetStatus::Gone(_)));
            if gone {
                ui.add_space(6.0);
                ui.label(RichText::new(tr("What's here stays on this computer until you remove it. Nothing you change here reaches anyone.")).color(t.text2));
                ui.add_space(4.0);
                if ui
                    .add(Tint(
                        egui::Button::new(
                            RichText::new(tr("Remove from this computer")).color(Color32::WHITE),
                        )
                        .fill(t.danger),
                    ))
                    .clicked()
                {
                    self.remove_gone_workspace();
                    return true;
                }
            }
            if matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Sign this computer in again with your email and password, or with a new invite code from the owner or an admin.")).color(t.text2));
                let mut at = server.clone();
                if let Some(how) = self.login_form(
                    ui,
                    t,
                    &mut at,
                    &[Mode::Account, Mode::Invite],
                    "Sign in",
                    false,
                ) {
                    self.sign_in_with(how);
                }
            }
            ui.add_space(8.0);
            let how = if connected {
                tr("Tasks, chat and docs show up for everyone within a second or two.")
            } else if gone {
                tr("The server no longer has this workspace.")
            } else if matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
                tr("You can keep working here. Your changes are kept on this computer and go up once it's signed in again.")
            } else {
                tr("You can keep working here. Your changes are kept on this computer and go up when the server is back.")
            };
            ui.label(RichText::new(how).small().color(t.text3));
            if !connected && self.sync_rt.editing_addr {
                ui.add_space(8.0);
                ui.label(RichText::new(tr("New server address")).color(t.text2));
                let r = ui.add(
                    egui::TextEdit::singleline(addr)
                        .hint_text("https://wb.example.com")
                        .desired_width(f32::INFINITY),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Server address")
                });
                ui.horizontal(|ui| {
                    if primary_button(ui, t, tr("Save and reconnect")).clicked() {
                        match self.change_server(addr) {
                            Ok(()) => result.clear(),
                            Err(e) => *result = server_text(&e),
                        }
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        self.sync_rt.editing_addr = false;
                        *addr = server.clone();
                        result.clear();
                    }
                });
            }
            if connected {
                ui.add_space(10.0);
                self.account_section(ui, t);
            }
            if connected && self.sync_rt.role.is_some_and(|r| r.manages()) {
                ui.add_space(10.0);
                self.people_section(ui, t);
                if self.sync_rt.role == Some(Role::Owner) {
                    ui.add_space(10.0);
                    self.backups_section(ui, t);
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Reconnect now")).clicked() {
                    self.sync_rt.net = None;
                    self.sync_rt.net_status = None;
                }
                // the server moved or the address was mistyped
                if !connected
                    && !self.sync_rt.editing_addr
                    && ui
                        .button(tr("Change address"))
                        .on_hover_text(tr(
                            "Connect this workspace to the server at another address",
                        ))
                        .clicked()
                {
                    self.sync_rt.editing_addr = true;
                    *addr = server.clone();
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new(tr("Stop sharing")).color(t.danger))
                        .on_hover_text(tr("Keep working on this computer only; your copy stays"))
                        .clicked()
                    {
                        self.stop_sharing();
                        close = true;
                    }
                });
            });
        } else if let Some(cfg) = self.data.extra.shared.clone() {
            ui.label(
                RichText::new(tr("This workspace is shared"))
                    .size(15.0)
                    .strong(),
            );
            ui.add(
                egui::Label::new(RichText::new(short_path(&cfg.folder, 48)).color(t.text2))
                    .wrap_mode(egui::TextWrapMode::Truncate),
            )
            .on_hover_text(&cfg.folder);
            // computers, not people: the people list counts everyone added
            let n = sync::other_files(Path::new(&cfg.folder), &cfg.device).len();
            ui.label(RichText::new(match n {
                0 => tr("No other computer is working in this folder yet. Share the folder with your teammates in your sync app. On their computer they open Work together, paste the folder's path and choose Join this workspace.").to_string(),
                1 => tr("1 other computer is working in it.").to_string(),
                n => trf!("{} other computers are working in it.", n),
            }).color(t.text2));
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Tasks, chat and docs show up for everyone a few seconds after their sync app copies the folder. Calls open in the browser from a link posted in chat.")).small().color(t.text3));
            ui.label(RichText::new(tr(FOLDER_PRIVACY)).small().color(t.text2));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Check for changes now")).clicked() {
                    let changed = self.sync_pull();
                    *result = if changed {
                        tr("Got the latest changes.").into()
                    } else {
                        tr("Everything is up to date.").into()
                    };
                }
                if ui.button(tr("Show folder")).clicked() {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(format!("file://{}", cfg.folder)));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new(tr("Stop sharing")).color(t.danger))
                        .on_hover_text(tr("Keep working on this computer only; your copy stays"))
                        .clicked()
                    {
                        self.stop_sharing();
                        close = true;
                    }
                });
            });
        } else {
            ui.label(tr(
                "Work on this workspace with other people, without a server.",
            ));
            ui.label(RichText::new(tr("Pick a folder that a sync app shares with your teammates (iCloud Drive, Dropbox, Google Drive, OneDrive or Syncthing). Everyone opens Workboard on their own computer and points it at the same folder.")).color(t.text2));
            ui.label(RichText::new(tr(FOLDER_PRIVACY)).small().color(t.text2));
            ui.add_space(8.0);
            let sugg = sync::suggested_folders();
            if !sugg.is_empty() {
                ui.label(
                    RichText::new(tr("Found on this computer"))
                        .small()
                        .color(t.text3),
                );
                ui.horizontal_wrapped(|ui| {
                    for (name, p) in &sugg {
                        if ui.button(name).on_hover_text(p).clicked() {
                            *path = p.clone();
                        }
                    }
                });
            }
            ui.label(RichText::new(tr("Folder")).color(t.text2));
            ui.add(
                field(path)
                    .hint_text(if sugg.is_empty() {
                        tr("Paste a folder path")
                    } else {
                        tr("Paste a folder path, or pick one above")
                    })
                    .desired_width(f32::INFINITY),
            );
            let n = folder_people(path);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if n > 0 {
                    ui.label(RichText::new(if n == 1 { tr("A shared workspace is already here (1 person).").to_string() } else { trf!("A shared workspace is already here ({} people).", n) }).color(t.text2));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, t, tr("Join this workspace")).clicked() {
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
                        if primary_button(ui, t, tr("Share this workspace here")).clicked() {
                            match self.share_here(path) {
                                Ok(()) => {
                                    self.toast(tr("Shared. Now share the folder with your teammates in your sync app."));
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
            ui.label(RichText::new(tr("Or connect to your team's server")).strong());
            ui.label(RichText::new(tr("A Workboard server (for example https://wb.example.com). Sign in with your email and password, or join with the invite code you were given. Setting up a new, empty server? This workspace goes up and you become its owner.")).color(t.text2));
            let _ = code;
            self.server_form(
                ui,
                t,
                addr,
                &[Mode::Account, Mode::Invite, Mode::NewServer],
                "Connect",
            );
        }
        if !result.is_empty() {
            ui.label(RichText::new(result.as_str()).color(t.text2));
        }
        close
    }

    pub(super) fn who_am_i_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        name: &mut String,
        email: &mut String,
    ) -> bool {
        let mut close = false;
        ui.label(
            RichText::new(tr(
                "Pick yourself so your messages and tasks show your name.",
            ))
            .color(t.text2),
        );
        ui.add_space(6.0);
        let members: Vec<Member> = self.data.members.clone();
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for m in &members {
                    let label = if m.invited {
                        trf!("{}   {}  (invited)", m.name, m.email)
                    } else {
                        format!("{}   {}", m.name, m.email)
                    };
                    if ui.add(ghost(egui::Button::new(label))).clicked() {
                        self.become_member(Some(m.id), "", "");
                        close = true;
                    }
                }
            });
        ui.separator();
        ui.label(RichText::new(tr("I'm new here")).color(t.text2));
        ui.add(
            field(name)
                .hint_text(tr("Your name"))
                .desired_width(f32::INFINITY),
        );
        ui.add(
            field(email)
                .hint_text(tr("Email"))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(6.0);
        if primary_button(ui, t, tr("Join as new member")).clicked() && !name.trim().is_empty() {
            self.become_member(None, name, email);
            close = true;
        }
        close
    }

    pub(super) fn call_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        ch: &mut u64,
        link: &mut String,
    ) -> bool {
        let mut close = false;
        // why it's a browser call rather than a huddle here
        if self
            .data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.is_some())
            && self.sync_rt.net_status != Some(NetStatus::Connected)
        {
            ui.label(RichText::new(tr("The team's server can't be reached, so a huddle in the app isn't available. This call opens in the browser instead, and the others see its Join button once you're connected again.")).color(WARN));
            ui.add_space(6.0);
        }
        ui.label(RichText::new(tr("The call opens in your browser on Jitsi Meet (free). The person who starts it may be asked to sign in with Google, GitHub or Facebook; everyone else joins without an account. Everyone in the conversation gets a Join button.")).color(t.text2));
        // nobody else to get the Join button
        let me = self.data.me;
        let alone = self
            .data
            .channels
            .iter()
            .find(|c| c.id == *ch)
            .is_some_and(|c| {
                !c.members
                    .iter()
                    .any(|id| *id != me && self.data.member(*id).is_some_and(|m| !m.invited))
            });
        if alone {
            ui.label(RichText::new(tr("You're the only one in this conversation, so nobody else gets the Join button. Invite people first.")).small().color(t.text3));
        }
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Call in")).color(t.text2));
        let chans: Vec<(u64, String)> = self
            .data
            .channels
            .iter()
            .filter(|c| self.listed(c))
            .map(|c| {
                (
                    c.id,
                    if c.kind == ChannelKind::Channel {
                        format!("# {}", c.name)
                    } else {
                        self.channel_label(c)
                    },
                )
            })
            .collect();
        let cur = chans
            .iter()
            .find(|c| c.0 == *ch)
            .map(|c| c.1.clone())
            .unwrap_or_else(|| tr("Choose").into());
        // as wide as the link field below it
        egui::ComboBox::from_id_salt("call_in")
            .selected_text(cur)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (id, n) in &chans {
                    ui.selectable_value(ch, *id, n);
                }
            });
        ui.add_space(6.0);
        ui.label(
            RichText::new(tr("Or use your own meeting link (Zoom, Google Meet...)"))
                .small()
                .color(t.text3),
        );
        ui.add(
            field(link)
                .hint_text(tr("https://..."))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, t, tr("Start call")).clicked()
                    && chans.iter().any(|c| c.0 == *ch)
                {
                    let ctx = ui.ctx().clone();
                    self.start_call(Some(&ctx), *ch, link);
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// The call to open from a Huddle / call button: the open conversation, else #general.
    pub(super) fn open_call_dialog(&mut self) {
        let ch = if self.section == Section::Chat {
            self.chat_open
        } else {
            None
        }
        .or_else(|| {
            self.data
                .channels
                .iter()
                .find(|c| c.kind == ChannelKind::Channel)
                .map(|c| c.id)
        })
        .unwrap_or(0);
        // on the team's server: a voice huddle right here
        if self.huddles_on_server() && ch != 0 {
            self.start_huddle(ch);
            return;
        }
        self.modal = Some(Modal::Call {
            ch,
            link: String::new(),
        });
    }
}
