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
            Unavailable::NotInstalled => format!("Workboard {} can only update itself when it runs as the installed app.", env!("CARGO_PKG_VERSION")),
            Unavailable::NoServer => format!(
                "Workboard {} gets its updates from your team's server. Connect to it in Settings > Work together, then try again.",
                env!("CARGO_PKG_VERSION")
            ),
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
    use std::cell::Cell;
    use std::ffi::{CStr, CString};
    use std::ptr::null_mut;
    use std::sync::Mutex;

    /// The appcast address handed to Sparkle each time it checks.
    static FEED: Mutex<Option<String>> = Mutex::new(None);

    thread_local! {
        /// The SPUStandardUpdaterController: None until first asked for,
        /// then Some(null) when Sparkle isn't in this app.
        static CONTROLLER: Cell<Option<*mut Object>> = const { Cell::new(None) };
        static STARTED: Cell<bool> = const { Cell::new(false) };
    }

    fn on_main_thread() -> bool {
        unsafe {
            let main: BOOL = msg_send![class!(NSThread), isMainThread];
            main
        }
    }

    pub fn set_server(addr: &str) {
        let Ok(url) = wb_net::updates_url(addr) else { return };
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
        let Some(frameworks) = from_ns(frameworks) else { return null_mut() };
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
        let Some(cls) = Class::get("SPUStandardUpdaterController") else { return null_mut() };
        let delegate = delegate();
        let c: *mut Object = msg_send![cls, alloc];
        // kept for the life of the app (the controller holds its delegate weakly; ours is never freed either)
        msg_send![c, initWithStartingUpdater: NO updaterDelegate: delegate userDriverDelegate: null_mut::<Object>()]
    }

    /// An object answering -feedURLStringForUpdater: with the current FEED.
    unsafe fn delegate() -> *mut Object {
        extern "C" fn feed_url(_this: &Object, _cmd: Sel, _updater: *mut Object) -> *mut Object {
            match FEED.lock().ok().and_then(|f| f.clone()) {
                Some(url) => unsafe { ns(&url) },
                None => null_mut(),
            }
        }
        let cls = match Class::get("WorkboardUpdaterDelegate") {
            Some(c) => c,
            None => {
                let mut decl = ClassDecl::new("WorkboardUpdaterDelegate", class!(NSObject)).expect("declare updater delegate");
                decl.add_method(sel!(feedURLStringForUpdater:), feed_url as extern "C" fn(&Object, Sel, *mut Object) -> *mut Object);
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
