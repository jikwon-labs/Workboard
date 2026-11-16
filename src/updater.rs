// App updates through Sparkle (macOS only).
//
// The Sparkle framework ships inside the app (Contents/Frameworks, copied
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

use std::cell::Cell;

thread_local! {
    /// A newer version is on the server. Set from Sparkle's answers, which
    /// arrive on the main thread, where the interface reads it too.
    static READY: Cell<bool> = const { Cell::new(false) };
}

/// Whether the team's server has a newer version than this one.
pub fn update_available() -> bool {
    READY.with(|r| r.get())
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
    /// Not the installed app (no Sparkle inside), e.g. `cargo run`.
    NotInstalled,
    /// No server known yet, so there is nowhere to look.
    NoServer,
}

impl Unavailable {
    pub fn explain(&self) -> String {
        match self {
            Unavailable::NotInstalled => crate::trf!("Workboard {} can only update itself when it runs as the installed app.", env!("CARGO_PKG_VERSION")),
            Unavailable::NoServer => crate::trf!("Workboard {} gets its updates from your team's server. Connect to it in Settings > Work together, then try again.", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// The app is signed in to the server at `addr`: look for updates there.
pub fn set_server(addr: &str) {
    #[cfg(target_os = "macos")]
    mac::set_server(addr);
    #[cfg(not(target_os = "macos"))]
    let _ = addr;
}

/// Called every frame: now and then look for an update without showing
/// anything, and repaint `ctx` when the answer changes the red dot.
pub fn tick(ctx: &egui::Context) {
    #[cfg(target_os = "macos")]
    mac::tick(ctx);
    #[cfg(not(target_os = "macos"))]
    let _ = ctx;
}

/// Look for an update now and show Sparkle's window with the result.
pub fn check_now() -> Result<(), Unavailable> {
    #[cfg(target_os = "macos")]
    return mac::check_now();
    #[cfg(not(target_os = "macos"))]
    Err(Unavailable::NotInstalled)
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
