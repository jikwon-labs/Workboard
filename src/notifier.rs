// macOS side of notifications: Notification Center banners, the Dock badge,
// bouncing the Dock icon and a short sound.
//
// Banners go through UNUserNotificationCenter when the app runs as the
// installed Workboard.app: they show the app's icon and name, and a click
// brings the app forward and opens what the banner is about (`take_clicked`).
// Run any other way (`cargo run`), macOS refuses that API, so banners fall
// back to `osascript`; those show under Script Editor and a click does not
// open anything.
//
// Everything here must be called from the main thread (the egui frame).

/// Banners ask for permission once, the first time one is needed.
pub fn post(title: &str, body: &str, target: &str, sound: bool) {
    #[cfg(target_os = "macos")]
    mac::post(title, body, target, sound);
    #[cfg(not(target_os = "macos"))]
    let _ = (title, body, target, sound);
}

/// What banners the person clicked since the last call (their targets).
pub fn take_clicked() -> Vec<String> {
    #[cfg(target_os = "macos")]
    return mac::take_clicked();
    #[cfg(not(target_os = "macos"))]
    vec![]
}

/// The red number on the Dock icon ("" for none).
pub fn set_badge(label: &str) {
    #[cfg(target_os = "macos")]
    mac::set_badge(label);
    #[cfg(not(target_os = "macos"))]
    let _ = label;
}

/// Bounce the Dock icon: once, or until the app is brought forward.
pub fn bounce(until_seen: bool) {
    #[cfg(target_os = "macos")]
    mac::bounce(until_seen);
    #[cfg(not(target_os = "macos"))]
    let _ = until_seen;
}

/// A short sound (for news that arrives while the app is in front).
pub fn play_sound() {
    #[cfg(target_os = "macos")]
    mac::play_sound();
}

#[cfg(target_os = "macos")]
mod mac {
    use block::{Block, ConcreteBlock};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel, BOOL, YES};
    use objc::{class, msg_send, sel, sel_impl};
    use std::cell::Cell;
    use std::ffi::{CStr, CString};
    use std::ptr::null_mut;
    use std::sync::Mutex;

    #[link(name = "UserNotifications", kind = "framework")]
    extern "C" {}

    static CLICKED: Mutex<Vec<String>> = Mutex::new(Vec::new());

    thread_local! {
        /// Whether this process may use UNUserNotificationCenter: None until checked.
        static CENTER_OK: Cell<Option<bool>> = const { Cell::new(None) };
        static ASKED: Cell<bool> = const { Cell::new(false) };
        static BADGE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    }

    fn on_main_thread() -> bool {
        unsafe {
            let main: BOOL = msg_send![class!(NSThread), isMainThread];
            main
        }
    }

    /// The installed app (a bundle with an identifier): only then does
    /// UNUserNotificationCenter work; elsewhere it throws.
    fn center_ok() -> bool {
        CENTER_OK.with(|c| {
            if let Some(ok) = c.get() {
                return ok;
            }
            let ok = unsafe {
                let main: *mut Object = msg_send![class!(NSBundle), mainBundle];
                let id: *mut Object = if main.is_null() {
                    null_mut()
                } else {
                    msg_send![main, bundleIdentifier]
                };
                let path: *mut Object = if main.is_null() {
                    null_mut()
                } else {
                    msg_send![main, bundlePath]
                };
                !id.is_null()
                    && from_ns(path).is_some_and(|p| p.ends_with(".app"))
                    && Class::get("UNUserNotificationCenter").is_some()
            };
            c.set(Some(ok));
            ok
        })
    }

    unsafe fn center() -> *mut Object {
        let c: *mut Object = msg_send![class!(UNUserNotificationCenter), currentNotificationCenter];
        if !ASKED.with(|a| a.replace(true)) {
            let _: () = msg_send![c, setDelegate: delegate()];
            // alert | sound | badge
            let done = ConcreteBlock::new(|_granted: BOOL, _err: *mut Object| {}).copy();
            let _: () =
                msg_send![c, requestAuthorizationWithOptions: 7usize completionHandler: &*done];
        }
        c
    }

    pub fn post(title: &str, body: &str, target: &str, sound: bool) {
        if !on_main_thread() {
            return;
        }
        if !center_ok() {
            fallback(title, body, sound);
            return;
        }
        unsafe {
            let c = center();
            let content: *mut Object = msg_send![class!(UNMutableNotificationContent), new];
            let _: () = msg_send![content, setTitle: ns(title)];
            let _: () = msg_send![content, setBody: ns(body)];
            let info: *mut Object = msg_send![class!(NSDictionary), dictionaryWithObject: ns(target) forKey: ns("target")];
            let _: () = msg_send![content, setUserInfo: info];
            if sound {
                let s: *mut Object = msg_send![class!(UNNotificationSound), defaultSound];
                let _: () = msg_send![content, setSound: s];
            }
            let uuid: *mut Object = msg_send![class!(NSUUID), UUID];
            let id: *mut Object = msg_send![uuid, UUIDString];
            let req: *mut Object = msg_send![class!(UNNotificationRequest), requestWithIdentifier: id content: content trigger: null_mut::<Object>()];
            let _: () = msg_send![c, addNotificationRequest: req withCompletionHandler: null_mut::<Object>()];
            let _: () = msg_send![content, release];
        }
    }

    /// Not the installed app: a banner through AppleScript.
    fn fallback(title: &str, body: &str, sound: bool) {
        let q = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let mut script = format!(
            "display notification \"{}\" with title \"{}\"",
            q(body),
            q(title)
        );
        if sound {
            script.push_str(" sound name \"Glass\"");
        }
        std::thread::spawn(move || {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(script)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        });
    }

    pub fn take_clicked() -> Vec<String> {
        std::mem::take(&mut *CLICKED.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Answers banner clicks (keeps the target for the app) and lets banners
    /// show while the app is in front (the app decides that itself).
    unsafe fn delegate() -> *mut Object {
        extern "C" fn clicked(
            _this: &Object,
            _cmd: Sel,
            _center: *mut Object,
            response: *mut Object,
            done: *mut Object,
        ) {
            unsafe {
                let note: *mut Object = msg_send![response, notification];
                let req: *mut Object = msg_send![note, request];
                let content: *mut Object = msg_send![req, content];
                let info: *mut Object = msg_send![content, userInfo];
                let t: *mut Object = msg_send![info, objectForKey: ns("target")];
                if let Some(t) = from_ns(t) {
                    CLICKED.lock().unwrap_or_else(|e| e.into_inner()).push(t);
                }
                let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                let _: () = msg_send![app, activateIgnoringOtherApps: YES];
                if !done.is_null() {
                    let b = &*(done as *const Block<(), ()>);
                    b.call(());
                }
            }
        }
        extern "C" fn present(
            _this: &Object,
            _cmd: Sel,
            _center: *mut Object,
            _note: *mut Object,
            done: *mut Object,
        ) {
            unsafe {
                if !done.is_null() {
                    // list | banner | sound
                    let b = &*(done as *const Block<(usize,), ()>);
                    b.call(((1 << 3) | (1 << 4) | (1 << 1),));
                }
            }
        }
        let cls = match Class::get("WorkboardNotifyDelegate") {
            Some(c) => c,
            None => {
                let mut decl = ClassDecl::new("WorkboardNotifyDelegate", class!(NSObject))
                    .expect("declare notification delegate");
                decl.add_method(sel!(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:), clicked as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object));
                decl.add_method(
                    sel!(userNotificationCenter:willPresentNotification:withCompletionHandler:),
                    present as extern "C" fn(&Object, Sel, *mut Object, *mut Object, *mut Object),
                );
                decl.register()
            }
        };
        // kept for the life of the app (the center holds its delegate weakly)
        msg_send![cls, new]
    }

    pub fn set_badge(label: &str) {
        if !on_main_thread() || BADGE.with(|b| *b.borrow() == label) {
            return;
        }
        BADGE.with(|b| *b.borrow_mut() = label.to_string());
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let tile: *mut Object = msg_send![app, dockTile];
            if tile.is_null() {
                return;
            }
            let s = if label.is_empty() {
                null_mut()
            } else {
                ns(label)
            };
            let _: () = msg_send![tile, setBadgeLabel: s];
        }
    }

    pub fn bounce(until_seen: bool) {
        if !on_main_thread() {
            return;
        }
        unsafe {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            // NSCriticalRequest 0, NSInformationalRequest 10
            let _: isize =
                msg_send![app, requestUserAttention: if until_seen { 0usize } else { 10usize }];
        }
    }

    pub fn play_sound() {
        if !on_main_thread() {
            return;
        }
        unsafe {
            let s: *mut Object = msg_send![class!(NSSound), soundNamed: ns("Glass")];
            if !s.is_null() {
                let _: BOOL = msg_send![s, play];
            }
        }
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
