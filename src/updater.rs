// App updates: through Sparkle on macOS, through the app's own updater
// (`own`, below) on Windows.
//
// The Sparkle framework ships inside the Mac app (Contents/Frameworks, copied
// there by scripts/bundle.sh) and is loaded at run time, so `cargo run` and
// the tests work without it; there this module does nothing.
//
// Updates are published on the team's server (GET /updates/appcast.xml, see
// workboard-server), so the feed address comes from the server this app is
// signed in to. The updater starts the first time the app reaches a server;
// from then on Sparkle checks once a day by itself and asks before installing.
// Sparkle must be used from the main thread; calls from other threads are
// ignored.
//
// Besides that, the app quietly asks the server every few hours (no window)
// so the profile menu can mark "Check for updates" with a red dot while a
// newer version is waiting.
//
// Windows has no Sparkle. There the app reads the server's
// appcast-windows.xml (written by scripts/publish-windows.sh) itself, and
// when asked downloads the zip it names, swaps its own workboard.exe for the
// one inside and starts again (see `own`).

use std::cell::Cell;

thread_local! {
    /// A newer version is on the server. Set from Sparkle's answers, which
    /// arrive on the main thread, where the interface reads it too.
    static READY: Cell<bool> = const { Cell::new(false) };
}

/// Whether the team's server has a newer version than this one.
pub fn update_available() -> bool {
    READY.with(|r| r.get()) || own::newest().is_some()
}

#[cfg(any(test, target_os = "macos"))]
fn set_update_available(on: bool) {
    READY.with(|r| r.set(on));
}

/// For tests: pretend the server has (or hasn't) a newer version.
#[cfg(test)]
pub fn pretend_update_available(on: bool) {
    set_update_available(on);
}

/// Why "Check for updates" can't run right now.
#[derive(Debug, PartialEq)]
pub enum Unavailable {
    /// Not the installed Mac app (no Sparkle inside), e.g. `cargo run`.
    NotInstalled,
    /// No server known yet, so there is nowhere to look.
    NoServer,
}

impl Unavailable {
    pub fn explain(&self) -> String {
        match self {
            Unavailable::NotInstalled if cfg!(target_os = "macos") => crate::trf!("Workboard {} can only update itself when it runs as the installed app.", env!("CARGO_PKG_VERSION")),
            // Linux and other systems: neither Sparkle nor the Windows updater
            Unavailable::NotInstalled => crate::trf!("Workboard {} can't update itself on this system. Get the new version from whoever set up your team's server.", env!("CARGO_PKG_VERSION")),
            Unavailable::NoServer => crate::trf!("Workboard {} gets its updates from your team's server. Connect to it in Settings > Work together, then try again.", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Who shows how "Check for updates" goes.
#[derive(Debug, PartialEq)]
pub enum Shown {
    /// Sparkle, in its own window (macOS).
    BySparkle,
    /// The app: show `status_ui` (Windows).
    ByApp,
}

/// The app is signed in to the server at `addr`: look for updates there.
pub fn set_server(addr: &str) {
    if own::enabled() {
        own::set_server(addr);
        return;
    }
    #[cfg(target_os = "macos")]
    mac::set_server(addr);
    #[cfg(not(target_os = "macos"))]
    let _ = addr;
}

/// Called every frame: now and then look for an update without showing
/// anything, and repaint `ctx` when the answer changes the red dot.
pub fn tick(ctx: &egui::Context) {
    if own::enabled() {
        own::tick(ctx);
        return;
    }
    #[cfg(target_os = "macos")]
    mac::tick(ctx);
    #[cfg(not(target_os = "macos"))]
    let _ = ctx;
}

/// Look for an update now. On macOS Sparkle's window shows the result; on
/// Windows the caller shows `status_ui`.
pub fn check_now() -> Result<Shown, Unavailable> {
    if own::enabled() {
        return own::check_now().map(|()| Shown::ByApp);
    }
    #[cfg(target_os = "macos")]
    return mac::check_now().map(|()| Shown::BySparkle);
    #[cfg(not(target_os = "macos"))]
    Err(Unavailable::NotInstalled)
}

/// The app is quitting: if an update was installed, start the new version.
pub fn relaunch_if_updated() {
    if own::enabled() {
        own::relaunch_if_updated();
    }
}

/// How the Windows update is going, with what can be done next: the body of
/// the "Check for updates" window (`in_window`, with buttons to close it)
/// or a few lines in another form. True when the window should close.
pub fn status_ui(ui: &mut egui::Ui, t: &crate::theme::Theme, in_window: bool) -> bool {
    own::status_ui(ui, t, in_window)
}

pub mod own {
    //! The Windows updater. The team's server keeps the newest Windows build
    //! as updates/Workboard-<version>-windows-x64.zip (workboard.exe inside)
    //! and describes it in updates/appcast-windows.xml:
    //!
    //! ```xml
    //! <update>
    //!   <version>0.1.15</version>
    //!   <file>Workboard-0.1.15-windows-x64.zip</file>
    //!   <length>12345678</length>
    //!   <sha256>(64 hex digits)</sha256>
    //! </update>
    //! ```
    //!
    //! The app reads that file every few hours (the red dot) and when asked.
    //! Installing downloads the zip, checks its size and sha256, unpacks it
    //! with Windows' own tar.exe, renames the running workboard.exe to
    //! workboard.old.exe (Windows lets a running program be renamed, not
    //! overwritten), puts the new one in its place and closes; the app's
    //! exit starts the new one, which removes the old file.
    //!
    //! State lives per thread like the rest of the interface (each UI test
    //! has its own); the background work reports through `Shared`.

    use super::Unavailable;
    use crate::i18n::tr;
    use crate::theme::Theme;
    use std::cell::{Cell, RefCell};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// The Windows update information on the team's server, beside the
    /// Mac's appcast.xml (which only Sparkle reads).
    pub const FEED: &str = "appcast-windows.xml";

    /// This build's version.
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");

    /// How often to look quietly: one small file on the team's own server.
    const PROBE_EVERY: Duration = Duration::from_secs(3 * 60 * 60);

    /// One build on the server, as appcast-windows.xml describes it.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Release {
        pub version: String,
        /// The zip's name in the server's updates/ folder.
        pub file: String,
        pub length: u64,
        pub sha256: String,
    }

    /// Why looking for or installing an update didn't work.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Problem {
        Unreachable,
        /// The server has no appcast-windows.xml (no Windows build put up yet).
        NotOnServer,
        /// appcast-windows.xml didn't make sense.
        BadFeed,
        /// The download's size or sha256 isn't what the server said.
        Damaged,
        /// workboard.exe couldn't be swapped in this folder.
        CantReplace(String),
    }

    impl Problem {
        pub fn explain(&self) -> String {
            match self {
                Problem::Unreachable => tr("Couldn't reach your team's server. Check the connection and try again.").into(),
                Problem::NotOnServer => tr("Your team's server has no Windows version to update to yet.").into(),
                Problem::BadFeed => tr("Your team's server has update information this version can't read.").into(),
                Problem::Damaged => tr("The update arrived damaged. Try again.").into(),
                Problem::CantReplace(folder) => crate::trf!("Workboard couldn't replace itself in {}. Move workboard.exe to a folder you can change (like Desktop or Documents), or download the new version from your team's server.", folder),
            }
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub enum Status {
        Idle,
        Checking,
        UpToDate,
        Available(Release),
        Downloading {
            version: String,
            done: u64,
            total: u64,
        },
        Failed(Problem),
        /// The new workboard.exe is in place; the app closes and starts it.
        Restarting,
    }

    /// What the background work hands back.
    struct Shared {
        status: Status,
        newest: Option<Release>,
        /// The workboard.exe that was replaced, to start when the app quits.
        installed: Option<PathBuf>,
        ctx: Option<egui::Context>,
    }

    struct Own {
        enabled: Cell<bool>,
        /// Tests: the file to replace instead of this program.
        target: RefCell<Option<PathBuf>>,
        server: RefCell<Option<String>>,
        checked: Cell<Option<Instant>>,
        cleaned: Cell<bool>,
        closing: Cell<bool>,
        shared: Arc<Mutex<Shared>>,
    }

    thread_local! {
        static OWN: Own = Own {
            enabled: Cell::new(cfg!(windows)),
            target: RefCell::new(None),
            server: RefCell::new(None),
            checked: Cell::new(None),
            cleaned: Cell::new(false),
            closing: Cell::new(false),
            shared: Arc::new(Mutex::new(Shared {
                status: Status::Idle,
                newest: None,
                installed: None,
                ctx: None,
            })),
        };
    }

    pub fn enabled() -> bool {
        OWN.with(|o| o.enabled.get())
    }

    /// For tests: use this updater (as on Windows) on this thread, against
    /// the server at `addr`, and replace `target` instead of the test
    /// program when installing.
    #[cfg(test)]
    pub fn pretend_windows(addr: &str, target: &Path) {
        OWN.with(|o| {
            o.enabled.set(true);
            *o.target.borrow_mut() = Some(target.to_path_buf());
            *o.server.borrow_mut() = Some(addr.to_string());
        });
    }

    /// For tests: show `status` (and count `newest` as waiting).
    #[cfg(test)]
    pub fn pretend_status(status: Status, newest: Option<Release>) {
        OWN.with(|o| {
            let mut s = o.shared.lock().unwrap();
            s.status = status;
            s.newest = newest;
        });
    }

    /// The newer version waiting on the server, if the updater found one.
    pub fn newest() -> Option<Release> {
        OWN.with(|o| o.shared.lock().unwrap().newest.clone())
    }

    pub fn status() -> Status {
        OWN.with(|o| o.shared.lock().unwrap().status.clone())
    }

    /// The version numbers in "1.2.3", None for anything else.
    fn numbers(v: &str) -> Option<Vec<u64>> {
        let v = v.trim();
        if v.is_empty() {
            return None;
        }
        v.split('.').map(|p| p.parse().ok()).collect()
    }

    /// Whether `remote` is a later version than `local` ("0.1.10" after
    /// "0.1.9"; "0.2" is "0.2.0").
    pub fn newer(remote: &str, local: &str) -> bool {
        let (Some(mut a), Some(mut b)) = (numbers(remote), numbers(local)) else {
            return false;
        };
        let n = a.len().max(b.len());
        a.resize(n, 0);
        b.resize(n, 0);
        a > b
    }

    /// Reads appcast-windows.xml (see the module's notes).
    pub fn parse_feed(xml: &str) -> Result<Release, String> {
        fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
            let open = format!("<{}>", name);
            let close = format!("</{}>", name);
            let start = xml.find(&open)? + open.len();
            let end = start + xml[start..].find(&close)?;
            Some(xml[start..end].trim())
        }
        let version = tag(xml, "version")
            .filter(|v| numbers(v).is_some())
            .ok_or("no version")?;
        // a plain zip name: it becomes a request to the server and a file here
        let file = tag(xml, "file")
            .filter(|f| {
                f.ends_with(".zip")
                    && !f.starts_with('.')
                    && f.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
            })
            .ok_or("no zip file")?;
        let length = tag(xml, "length")
            .and_then(|l| l.parse::<u64>().ok())
            .filter(|l| *l > 0)
            .ok_or("no length")?;
        let sha256 = tag(xml, "sha256")
            .map(|s| s.to_ascii_lowercase())
            .filter(|s| wb_net::name_ok(s))
            .ok_or("no sha256")?;
        Ok(Release {
            version: version.to_string(),
            file: file.to_string(),
            length,
            sha256,
        })
    }

    fn update(shared: &Mutex<Shared>, f: impl FnOnce(&mut Shared)) {
        let mut s = shared.lock().unwrap();
        f(&mut s);
        if let Some(ctx) = &s.ctx {
            ctx.request_repaint();
        }
    }

    fn busy(s: &Status) -> bool {
        matches!(
            s,
            Status::Checking | Status::Downloading { .. } | Status::Restarting
        )
    }

    pub fn set_server(addr: &str) {
        OWN.with(|o| {
            let changed = o.server.borrow().as_deref() != Some(addr);
            *o.server.borrow_mut() = Some(addr.to_string());
            if changed || o.checked.get().is_none() {
                start_check(o);
            }
        });
    }

    pub fn tick(ctx: &egui::Context) {
        OWN.with(|o| {
            if !o.cleaned.get() {
                o.cleaned.set(true);
                // what the last update left behind (the old program, which
                // can only go once it has stopped running)
                if o.target.borrow().is_none() {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::fs::remove_file(exe.with_extension("old.exe"));
                    }
                }
                let _ = std::fs::remove_dir_all(work_dir());
            }
            let restarting = {
                let mut s = o.shared.lock().unwrap();
                if s.ctx.is_none() {
                    s.ctx = Some(ctx.clone());
                }
                s.status == Status::Restarting
            };
            if restarting && !o.closing.get() && o.target.borrow().is_none() {
                // on_exit saves the work, then starts the new version
                o.closing.set(true);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let due = o.checked.get().is_some_and(|t| t.elapsed() >= PROBE_EVERY);
            if due {
                start_check(o);
            }
        });
    }

    pub fn check_now() -> Result<(), Unavailable> {
        OWN.with(|o| {
            if o.server.borrow().is_none() {
                return Err(Unavailable::NoServer);
            }
            start_check(o);
            Ok(())
        })
    }

    /// Ask the server for appcast-windows.xml on a background thread.
    fn start_check(o: &Own) {
        let Some(addr) = o.server.borrow().clone() else {
            return;
        };
        {
            let mut s = o.shared.lock().unwrap();
            if busy(&s.status) {
                return;
            }
            s.status = Status::Checking;
        }
        o.checked.set(Some(Instant::now()));
        let shared = o.shared.clone();
        std::thread::spawn(move || {
            let answer = wb_net::update_text(&addr, FEED);
            update(&shared, |s| match answer {
                Ok(Some(xml)) => match parse_feed(&xml) {
                    Ok(r) if newer(&r.version, VERSION) => {
                        s.newest = Some(r.clone());
                        s.status = Status::Available(r);
                    }
                    Ok(_) => {
                        s.newest = None;
                        s.status = Status::UpToDate;
                    }
                    Err(e) => {
                        eprintln!("workboard: {} from {}: {}", FEED, addr, e);
                        s.status = Status::Failed(Problem::BadFeed);
                    }
                },
                Ok(None) => {
                    s.newest = None;
                    s.status = Status::Failed(Problem::NotOnServer);
                }
                // keeps what an earlier look found
                Err(e) => {
                    eprintln!("workboard: update check at {}: {}", addr, e.explain());
                    s.status = Status::Failed(Problem::Unreachable);
                }
            });
        });
    }

    /// Where downloads are unpacked; emptied when the app starts.
    fn work_dir() -> PathBuf {
        std::env::temp_dir().join("workboard-update")
    }

    /// Download the newest version and put it in place of this program
    /// (or the test's target); the app then closes and starts it.
    pub fn install() {
        OWN.with(|o| {
            let Some(addr) = o.server.borrow().clone() else {
                return;
            };
            let target = o
                .target
                .borrow()
                .clone()
                .or_else(|| std::env::current_exe().ok());
            let Some(target) = target else {
                return;
            };
            let release = {
                let mut s = o.shared.lock().unwrap();
                let Some(r) = s.newest.clone() else {
                    return;
                };
                if busy(&s.status) {
                    return;
                }
                s.status = Status::Downloading {
                    version: r.version.clone(),
                    done: 0,
                    total: r.length,
                };
                r
            };
            // tests each unpack in their own folder
            let dir = if o.target.borrow().is_some() {
                target.with_extension("update")
            } else {
                work_dir()
            };
            let shared = o.shared.clone();
            std::thread::spawn(move || {
                let result = download_and_replace(&addr, &release, &dir, &target, &shared);
                let _ = std::fs::remove_dir_all(&dir);
                update(&shared, |s| match result {
                    Ok(()) => {
                        s.installed = Some(target);
                        s.status = Status::Restarting;
                    }
                    Err(p) => s.status = Status::Failed(p),
                });
            });
        });
    }

    fn download_and_replace(
        addr: &str,
        r: &Release,
        dir: &Path,
        target: &Path,
        shared: &Mutex<Shared>,
    ) -> Result<(), Problem> {
        let _ = std::fs::remove_dir_all(dir);
        let zip = dir.join(&r.file);
        let mut shown = 0u64;
        let got = wb_net::download_update(addr, &r.file, &zip, &mut |done, _| {
            // repaint at most every percent
            if done.saturating_sub(shown) * 100 >= r.length.max(1) {
                shown = done;
                update(shared, |s| {
                    if let Status::Downloading { done: d, .. } = &mut s.status {
                        *d = done;
                    }
                });
            }
            true
        });
        if let Err(e) = got {
            eprintln!("workboard: downloading {}: {}", r.file, e.explain());
            return Err(match e {
                wb_net::FileError::Missing => Problem::NotOnServer,
                _ => Problem::Unreachable,
            });
        }
        let (sha, len) = wb_net::sha256_file(&zip).map_err(|_| Problem::Damaged)?;
        if sha != r.sha256 || len != r.length {
            return Err(Problem::Damaged);
        }
        replace_from_zip(&zip, target)
    }

    /// tar.exe from Windows itself (since Windows 10 1803), which reads zip
    /// files; Git's tar earlier on PATH can't. The Mac's tar reads them too.
    pub fn tar() -> PathBuf {
        if cfg!(windows) {
            let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
            PathBuf::from(root).join("System32").join("tar.exe")
        } else {
            PathBuf::from("tar")
        }
    }

    fn unzip(zip: &Path, into: &Path) -> bool {
        let ok = crate::os::quiet_command(&tar().to_string_lossy())
            .arg("-xf")
            .arg(zip)
            .arg("-C")
            .arg(into)
            .status()
            .is_ok_and(|s| s.success());
        if ok || !cfg!(windows) {
            return ok;
        }
        // an older Windows without tar.exe
        let quote = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', "''"));
        crate::os::quiet_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(format!(
                "Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
                quote(zip),
                quote(into)
            ))
            .status()
            .is_ok_and(|s| s.success())
    }

    /// Unpack `zip` beside it and put its workboard.exe where `target` is,
    /// the old one kept as <name>.old.exe (removed next start). If the new
    /// one can't be put there, the old one goes back.
    pub fn replace_from_zip(zip: &Path, target: &Path) -> Result<(), Problem> {
        let folder = target
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let into = zip.with_extension("unpacked");
        let _ = std::fs::remove_dir_all(&into);
        std::fs::create_dir_all(&into).map_err(|_| Problem::Damaged)?;
        if !unzip(zip, &into) {
            return Err(Problem::Damaged);
        }
        let new = into.join("workboard.exe");
        if !new.is_file() {
            return Err(Problem::Damaged);
        }
        let old = target.with_extension("old.exe");
        let _ = std::fs::remove_file(&old);
        if let Err(e) = std::fs::rename(target, &old) {
            eprintln!("workboard: moving {} aside: {}", target.display(), e);
            return Err(Problem::CantReplace(folder));
        }
        // a copy, not a rename: the temporary folder can be on another drive
        if let Err(e) = std::fs::copy(&new, target) {
            eprintln!(
                "workboard: putting the new {} in place: {}",
                target.display(),
                e
            );
            let _ = std::fs::remove_file(target);
            let _ = std::fs::rename(&old, target);
            return Err(Problem::CantReplace(folder));
        }
        let _ = std::fs::remove_dir_all(&into);
        Ok(())
    }

    pub fn relaunch_if_updated() {
        OWN.with(|o| {
            if o.target.borrow().is_some() {
                return;
            }
            let Some(exe) = o.shared.lock().unwrap().installed.take() else {
                return;
            };
            let args: Vec<_> = std::env::args_os().skip(1).collect();
            if let Err(e) = std::process::Command::new(&exe).args(args).spawn() {
                eprintln!("workboard: starting {}: {}", exe.display(), e);
            }
        });
    }

    pub fn status_ui(ui: &mut egui::Ui, t: &Theme, in_window: bool) -> bool {
        use crate::widgets::{primary_button, secondary_button};
        use egui::RichText;
        let say = |ui: &mut egui::Ui, text: String| {
            ui.add(egui::Label::new(RichText::new(text).color(t.text)).wrap());
        };
        let mut close = false;
        match status() {
            Status::Idle | Status::Checking => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    say(
                        ui,
                        tr("Looking for a newer Workboard on your team's server...").into(),
                    );
                });
            }
            Status::UpToDate => {
                say(
                    ui,
                    crate::trf!("Workboard {} is the newest version.", VERSION),
                );
                if in_window {
                    ui.add_space(8.0);
                    if primary_button(ui, t, tr("OK")).clicked() {
                        close = true;
                    }
                }
            }
            Status::Available(r) => {
                say(
                    ui,
                    crate::trf!(
                        "Workboard {} is ready to install. You have {}.",
                        r.version,
                        VERSION
                    ),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(tr(
                            "Workboard saves your work, closes, replaces itself and opens again.",
                        ))
                        .small()
                        .color(t.text2),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, t, tr("Install and restart")).clicked() {
                        install();
                    }
                    if in_window && secondary_button(ui, tr("Later")).clicked() {
                        close = true;
                    }
                });
            }
            Status::Downloading {
                version,
                done,
                total,
            } => {
                say(ui, crate::trf!("Downloading Workboard {}...", version));
                let part = if total > 0 {
                    (done as f32 / total as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(part).show_percentage());
            }
            Status::Failed(p) => {
                say(ui, p.explain());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, t, tr("Try again")).clicked() {
                        if newest().is_some() {
                            install();
                        } else {
                            OWN.with(start_check);
                        }
                    }
                    if in_window && secondary_button(ui, tr("Close")).clicked() {
                        close = true;
                    }
                });
            }
            Status::Restarting => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    say(ui, tr("Opening the new version...").into());
                });
            }
        }
        close
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn later_versions_are_newer() {
            assert!(newer("0.1.15", "0.1.14"));
            assert!(newer("0.1.10", "0.1.9"), "by number, not by text");
            assert!(newer("0.2", "0.1.99"));
            assert!(newer("1.0.0", "0.9.9"));
            assert!(!newer("0.1.14", "0.1.14"));
            assert!(!newer("0.2", "0.2.0"));
            assert!(!newer("0.1.13", "0.1.14"), "never back to an older one");
            assert!(!newer("", "0.1.0") && !newer("x.y", "0.1.0") && !newer("0.1.", "0.1.0"));
        }

        fn feed(version: &str, file: &str, length: &str, sha: &str) -> String {
            format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<update>\n  <version>{}</version>\n  <file>{}</file>\n  <length>{}</length>\n  <sha256>{}</sha256>\n</update>\n",
                version, file, length, sha
            )
        }

        #[test]
        fn the_feed_names_one_zip_with_its_size_and_sha256() {
            let sha = "ab".repeat(32);
            assert_eq!(
                parse_feed(&feed(
                    "0.1.15",
                    "Workboard-0.1.15-windows-x64.zip",
                    "123",
                    &sha.to_uppercase()
                )),
                Ok(Release {
                    version: "0.1.15".into(),
                    file: "Workboard-0.1.15-windows-x64.zip".into(),
                    length: 123,
                    sha256: sha.clone(),
                })
            );
            // anything that could leave the updates folder or isn't a zip
            for file in ["../x.zip", "a/b.zip", ".x.zip", "x.exe", "x zip.zip", ""] {
                assert!(
                    parse_feed(&feed("0.1.15", file, "1", &sha)).is_err(),
                    "{}",
                    file
                );
            }
            assert!(parse_feed(&feed("soon", "x.zip", "1", &sha)).is_err());
            assert!(parse_feed(&feed("0.1.15", "x.zip", "0", &sha)).is_err());
            assert!(parse_feed(&feed("0.1.15", "x.zip", "1", "abc")).is_err());
            assert!(parse_feed("<html>not found</html>").is_err());
        }

        #[test]
        fn installing_swaps_the_program_and_keeps_the_old_one_aside() {
            let dir = workboard_server::testing::temp_home("swap");
            std::fs::create_dir_all(dir.join("build")).unwrap();
            std::fs::write(dir.join("build/workboard.exe"), b"new program").unwrap();
            let zip = dir.join("Workboard-9.0.0-windows-x64.zip");
            let made = std::process::Command::new(tar())
                .current_dir(dir.join("build"))
                .args(["-a", "-cf"])
                .arg(&zip)
                .arg("workboard.exe")
                .status()
                .unwrap();
            assert!(made.success());
            let target = dir.join("app").join("workboard.exe");
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(&target, b"old program").unwrap();

            replace_from_zip(&zip, &target).unwrap();
            assert_eq!(std::fs::read(&target).unwrap(), b"new program");
            assert_eq!(
                std::fs::read(dir.join("app/workboard.old.exe")).unwrap(),
                b"old program"
            );

            // a zip without workboard.exe changes nothing
            std::fs::write(dir.join("build/readme.txt"), b"hi").unwrap();
            let bad = dir.join("bad.zip");
            std::process::Command::new(tar())
                .current_dir(dir.join("build"))
                .args(["-a", "-cf"])
                .arg(&bad)
                .arg("readme.txt")
                .status()
                .unwrap();
            assert_eq!(replace_from_zip(&bad, &target), Err(Problem::Damaged));
            assert_eq!(std::fs::read(&target).unwrap(), b"new program");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::Unavailable;
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel, BOOL, NO};
    use objc::{class, msg_send, sel, sel_impl};
    use std::cell::{Cell, RefCell};
    use std::ffi::{CStr, CString};
    use std::ptr::null_mut;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// The appcast address handed to Sparkle each time it checks.
    static FEED: Mutex<Option<String>> = Mutex::new(None);

    thread_local! {
        /// The SPUStandardUpdaterController: None until first asked for,
        /// then Some(null) when Sparkle isn't in this app.
        static CONTROLLER: Cell<Option<*mut Object>> = const { Cell::new(None) };
        static STARTED: Cell<bool> = const { Cell::new(false) };
        /// When the last quiet look for an update was asked for.
        static PROBED: Cell<Option<Instant>> = const { Cell::new(None) };
        /// The window to repaint when Sparkle answers while the app is idle.
        static REPAINT: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
    }

    /// How often to look quietly. The appcast is a small file on the team's
    /// own server, so this costs next to nothing.
    const PROBE_EVERY: Duration = Duration::from_secs(3 * 60 * 60);

    fn on_main_thread() -> bool {
        unsafe {
            let main: BOOL = msg_send![class!(NSThread), isMainThread];
            main
        }
    }

    pub fn set_server(addr: &str) {
        let Ok(url) = wb_net::updates_url(addr) else {
            return;
        };
        if !on_main_thread() {
            return;
        }
        *FEED.lock().unwrap() = Some(url);
        let c = controller();
        if c.is_null() || STARTED.with(|s| s.get()) {
            return;
        }
        STARTED.with(|s| s.set(true));
        unsafe {
            let _: () = msg_send![c, startUpdater];
        }
        probe(c);
    }

    pub fn tick(ctx: &egui::Context) {
        REPAINT.with(|r| {
            if r.borrow().is_none() {
                *r.borrow_mut() = Some(ctx.clone());
            }
        });
        if !STARTED.with(|s| s.get()) {
            return;
        }
        let due = PROBED.with(|p| p.get().map_or(true, |t| t.elapsed() >= PROBE_EVERY));
        if due {
            probe(controller());
        }
    }

    /// Ask the server for its newest version without showing a window; the
    /// answer comes back through the delegate. Sparkle skips this while a
    /// check of its own is running, and that check answers the same way.
    fn probe(c: *mut Object) {
        PROBED.with(|p| p.set(Some(Instant::now())));
        if c.is_null() {
            return;
        }
        unsafe {
            let updater: *mut Object = msg_send![c, updater];
            if !updater.is_null() {
                let _: () = msg_send![updater, checkForUpdateInformation];
            }
        }
    }

    fn found(on: bool) {
        if super::update_available() == on {
            return;
        }
        super::set_update_available(on);
        REPAINT.with(|r| {
            if let Some(ctx) = r.borrow().as_ref() {
                ctx.request_repaint();
            }
        });
    }

    pub fn check_now() -> Result<(), Unavailable> {
        if !on_main_thread() {
            return Err(Unavailable::NotInstalled);
        }
        let c = controller();
        if c.is_null() {
            return Err(Unavailable::NotInstalled);
        }
        if !STARTED.with(|s| s.get()) {
            return Err(Unavailable::NoServer);
        }
        unsafe {
            let _: () = msg_send![c, checkForUpdates: null_mut::<Object>()];
        }
        Ok(())
    }

    fn controller() -> *mut Object {
        CONTROLLER.with(|c| {
            if let Some(p) = c.get() {
                return p;
            }
            let p = unsafe { create() };
            c.set(Some(p));
            p
        })
    }

    /// Load Contents/Frameworks/Sparkle.framework and make the controller
    /// (not started). Null when there is no framework.
    unsafe fn create() -> *mut Object {
        let main: *mut Object = msg_send![class!(NSBundle), mainBundle];
        if main.is_null() {
            return null_mut();
        }
        let frameworks: *mut Object = msg_send![main, privateFrameworksPath];
        let Some(frameworks) = from_ns(frameworks) else {
            return null_mut();
        };
        let path = format!("{}/Sparkle.framework", frameworks);
        if !std::path::Path::new(&path).is_dir() {
            return null_mut();
        }
        let bundle: *mut Object = msg_send![class!(NSBundle), bundleWithPath: ns(&path)];
        if bundle.is_null() {
            return null_mut();
        }
        let loaded: BOOL = msg_send![bundle, load];
        if !loaded {
            eprintln!("workboard: couldn't load {}", path);
            return null_mut();
        }
        let Some(cls) = Class::get("SPUStandardUpdaterController") else {
            return null_mut();
        };
        let delegate = delegate();
        let c: *mut Object = msg_send![cls, alloc];
        // kept for the life of the app (the controller holds its delegate weakly; ours is never freed either)
        msg_send![c, initWithStartingUpdater: NO updaterDelegate: delegate userDriverDelegate: null_mut::<Object>()]
    }

    /// An object answering -feedURLStringForUpdater: with the current FEED,
    /// and hearing whether a newer version was found.
    unsafe fn delegate() -> *mut Object {
        extern "C" fn feed_url(_this: &Object, _cmd: Sel, _updater: *mut Object) -> *mut Object {
            match FEED.lock().ok().and_then(|f| f.clone()) {
                Some(url) => unsafe { ns(&url) },
                None => null_mut(),
            }
        }
        // -updater:didFindValidUpdate:
        extern "C" fn did_find(
            _this: &Object,
            _cmd: Sel,
            _updater: *mut Object,
            _item: *mut Object,
        ) {
            found(true);
        }
        // -updaterDidNotFindUpdate: (also when the newest one was skipped)
        extern "C" fn did_not_find(_this: &Object, _cmd: Sel, _updater: *mut Object) {
            found(false);
        }
        // -updater:userDidMakeChoice:forUpdate:state:, SPUUserUpdateChoiceSkip is 0
        extern "C" fn did_choose(
            _this: &Object,
            _cmd: Sel,
            _updater: *mut Object,
            choice: isize,
            _item: *mut Object,
            _state: *mut Object,
        ) {
            if choice == 0 {
                found(false);
            }
        }
        let cls = match Class::get("WorkboardUpdaterDelegate") {
            Some(c) => c,
            None => {
                let mut decl = ClassDecl::new("WorkboardUpdaterDelegate", class!(NSObject))
                    .expect("declare updater delegate");
                decl.add_method(
                    sel!(feedURLStringForUpdater:),
                    feed_url as extern "C" fn(&Object, Sel, *mut Object) -> *mut Object,
                );
                decl.add_method(
                    sel!(updater:didFindValidUpdate:),
                    did_find as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
                );
                decl.add_method(
                    sel!(updaterDidNotFindUpdate:),
                    did_not_find as extern "C" fn(&Object, Sel, *mut Object),
                );
                decl.add_method(
                    sel!(updater:userDidMakeChoice:forUpdate:state:),
                    did_choose
                        as extern "C" fn(
                            &Object,
                            Sel,
                            *mut Object,
                            isize,
                            *mut Object,
                            *mut Object,
                        ),
                );
                decl.register()
            }
        };
        msg_send![cls, new]
    }

    /// An autoreleased NSString.
    unsafe fn ns(s: &str) -> *mut Object {
        let c = CString::new(s.replace('\0', "")).unwrap_or_default();
        msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()]
    }

    unsafe fn from_ns(s: *mut Object) -> Option<String> {
        if s.is_null() {
            return None;
        }
        let p: *const std::os::raw::c_char = msg_send![s, UTF8String];
        if p.is_null() {
            return None;
        }
        Some(CStr::from_ptr(p).to_string_lossy().into_owned())
    }
}
