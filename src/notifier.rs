// Notifications: banners, the count on the app's icon, asking for attention
// and a short sound.
//
// macOS: Notification Center banners, the Dock badge, bouncing the Dock icon.
// Banners go through UNUserNotificationCenter when the app runs as the
// installed Workboard.app: they show the app's icon and name, and a click
// brings the app forward and opens what the banner is about (`take_clicked`).
// Run any other way (`cargo run`), macOS refuses that API, so banners fall
// back to `osascript`; those show under Script Editor and a click does not
// open anything.
//
// While the app is quit, the team server sends banners through Apple's push
// service (server/src/push.rs). `start_push` asks Apple for this Mac's push
// token, which works only in the installed app signed with its provisioning
// profile (scripts/bundle.sh); `push_token` has it once it came. Clicking
// such a banner launches the app, and `take_clicked` gives
// "push:WS:TARGET" (WS: the workspace key the app sent with the token,
// src/app/push.rs; TARGET what the banner opens, or for a server from
// before targets "CHANNEL[:THREAD]").
//
// Notices on the clock (tasks due, meetings, goals, reminders) are handed to
// macOS ahead of time (`schedule`), so they show at their time even while the
// app is quit. While the app is in front macOS keeps those to itself and the
// app tells them in its own toast (src/app/notify.rs `plan_clock`).
//
// Windows: toast notifications, a count drawn over the taskbar button and
// flashing the taskbar button (`win` below).
//
// Everything here must be called from the main thread (the egui frame).

/// A banner handed to macOS ahead of time (`schedule`).
#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    /// the notice's key (src/app/notify.rs), unique among the planned
    pub key: String,
    pub title: String,
    pub body: String,
    /// what a click opens
    pub target: String,
    /// when it shows (seconds)
    pub at: i64,
    pub sound: bool,
}

/// The identifier macOS keeps a planned banner under.
pub const PLANNED_PREFIX: &str = "wb-clock-";

/// Hand `list` to macOS to show at their times, in place of what was handed
/// before (`old`: those keys). `wait`: until macOS has them (when quitting).
/// Only macOS keeps banners for later; elsewhere nothing happens.
pub fn schedule(list: &[Planned], old: &[String], wait: bool) {
    #[cfg(target_os = "macos")]
    mac::schedule(list, old, wait);
    #[cfg(not(target_os = "macos"))]
    let _ = (list, old, wait);
}

/// Banners ask for permission once, the first time one (or a Dock badge) is
/// needed.
pub fn post(title: &str, body: &str, target: &str, sound: bool) {
    #[cfg(target_os = "macos")]
    mac::post(title, body, target, sound);
    #[cfg(windows)]
    win::post(title, body, target, sound);
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = (title, body, target, sound);
}

/// Ask Apple for this Mac's push token (once; the installed app only).
pub fn start_push() {
    #[cfg(target_os = "macos")]
    mac::start_push();
}

/// This Mac's push token (hex), once Apple gave it.
pub fn push_token() -> Option<String> {
    #[cfg(target_os = "macos")]
    return mac::push_token();
    #[cfg(not(target_os = "macos"))]
    None
}

/// What banners the person clicked since the last call (their targets).
pub fn take_clicked() -> Vec<String> {
    #[cfg(target_os = "macos")]
    return mac::take_clicked();
    #[cfg(windows)]
    return win::take_clicked();
    #[cfg(not(any(target_os = "macos", windows)))]
    vec![]
}

/// The red number on the Dock icon, or over the taskbar button ("" for none).
pub fn set_badge(label: &str) {
    #[cfg(target_os = "macos")]
    mac::set_badge(label);
    #[cfg(windows)]
    win::set_badge(label);
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = label;
}

/// Bounce the Dock icon (flash the taskbar button): once, or until the app
/// is brought forward.
pub fn bounce(until_seen: bool) {
    #[cfg(target_os = "macos")]
    mac::bounce(until_seen);
    #[cfg(windows)]
    win::bounce(until_seen);
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = until_seen;
}

/// A short sound (for news that arrives while the app is in front).
pub fn play_sound() {
    #[cfg(target_os = "macos")]
    mac::play_sound();
    #[cfg(windows)]
    win::play_sound();
}

/// Remember the app's window (Windows needs it for the taskbar button and
/// to bring the app forward). Nothing to do on macOS.
pub fn attach(cc: &eframe::CreationContext<'_>) {
    #[cfg(windows)]
    win::attach(cc);
    #[cfg(not(windows))]
    let _ = cc;
}

/// Bring the window forward, restored if it was minimized: a copy started
/// later handed its link here (src/single.rs). macOS does this itself.
/// May be called from any thread.
pub fn bring_forward() {
    #[cfg(windows)]
    win::bring_forward();
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
    /// This Mac's push token, from Apple
    static TOKEN: Mutex<Option<String>> = Mutex::new(None);

    thread_local! {
        /// Whether this process may use UNUserNotificationCenter: None until checked.
        static CENTER_OK: Cell<Option<bool>> = const { Cell::new(None) };
        static ASKED: Cell<bool> = const { Cell::new(false) };
        static PUSH_STARTED: Cell<bool> = const { Cell::new(false) };
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

    pub fn schedule(list: &[super::Planned], old: &[String], wait: bool) {
        if !on_main_thread() || !center_ok() {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        // how many macOS hasn't confirmed yet
        let pending = std::sync::Arc::new((Mutex::new(0usize), std::sync::Condvar::new()));
        unsafe {
            let c = center();
            // the ones no longer planned go; a planned one with the same
            // key replaces its old self
            let gone: Vec<&String> = old
                .iter()
                .filter(|k| !list.iter().any(|p| &p.key == *k))
                .collect();
            if !gone.is_empty() {
                let ids: *mut Object = msg_send![class!(NSMutableArray), array];
                for k in gone {
                    let _: () =
                        msg_send![ids, addObject: ns(&format!("{}{}", super::PLANNED_PREFIX, k))];
                }
                let _: () = msg_send![c, removePendingNotificationRequestsWithIdentifiers: ids];
            }
            for p in list {
                let secs = (p.at - now) as f64;
                if secs < 1.0 {
                    continue;
                }
                let content: *mut Object = msg_send![class!(UNMutableNotificationContent), new];
                let _: () = msg_send![content, setTitle: ns(&p.title)];
                let _: () = msg_send![content, setBody: ns(&p.body)];
                let info: *mut Object = msg_send![class!(NSDictionary), dictionaryWithObject: ns(&p.target) forKey: ns("target")];
                let _: () = msg_send![content, setUserInfo: info];
                if p.sound {
                    let s: *mut Object = msg_send![class!(UNNotificationSound), defaultSound];
                    let _: () = msg_send![content, setSound: s];
                }
                let trigger: *mut Object = msg_send![class!(UNTimeIntervalNotificationTrigger), triggerWithTimeInterval: secs repeats: objc::runtime::NO];
                let id = ns(&format!("{}{}", super::PLANNED_PREFIX, p.key));
                let req: *mut Object = msg_send![class!(UNNotificationRequest), requestWithIdentifier: id content: content trigger: trigger];
                if wait {
                    *pending.0.lock().unwrap_or_else(|e| e.into_inner()) += 1;
                    let left = pending.clone();
                    let done = ConcreteBlock::new(move |_err: *mut Object| {
                        let mut n = left.0.lock().unwrap_or_else(|e| e.into_inner());
                        *n = n.saturating_sub(1);
                        left.1.notify_all();
                    })
                    .copy();
                    let _: () =
                        msg_send![c, addNotificationRequest: req withCompletionHandler: &*done];
                } else {
                    let _: () = msg_send![c, addNotificationRequest: req withCompletionHandler: null_mut::<Object>()];
                }
                let _: () = msg_send![content, release];
            }
        }
        if wait {
            // macOS answers on its own queue; quitting before it has them
            // would lose them
            let (lock, cv) = &*pending;
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
            let mut n = lock.lock().unwrap_or_else(|e| e.into_inner());
            while *n > 0 {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                if left.is_zero() {
                    break;
                }
                n = cv
                    .wait_timeout(n, left)
                    .unwrap_or_else(|e| e.into_inner())
                    .0;
            }
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

    pub fn start_push() {
        if !on_main_thread() || !center_ok() || PUSH_STARTED.with(|s| s.replace(true)) {
            return;
        }
        extern "C" fn registered(_this: &Object, _cmd: Sel, _app: *mut Object, token: *mut Object) {
            unsafe {
                if token.is_null() {
                    return;
                }
                let bytes: *const u8 = msg_send![token, bytes];
                let len: usize = msg_send![token, length];
                if bytes.is_null() || len == 0 {
                    return;
                }
                let hex: String = std::slice::from_raw_parts(bytes, len)
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect();
                eprintln!("push notifications while the app is quit: ready");
                // for trying a push by hand (docs/server-design.md "Mac")
                if std::env::var_os("WB_SHOW_PUSH_TOKEN").is_some() {
                    eprintln!("push token: {}", hex);
                }
                *TOKEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(hex);
            }
        }
        extern "C" fn failed(_this: &Object, _cmd: Sel, _app: *mut Object, err: *mut Object) {
            unsafe {
                let why: *mut Object = if err.is_null() {
                    null_mut()
                } else {
                    msg_send![err, localizedDescription]
                };
                eprintln!(
                    "no push notifications while the app is quit: {}",
                    from_ns(why).unwrap_or_default()
                );
            }
        }
        unsafe {
            // the delegate first: a click on a banner that launched the app
            // comes to it
            center();
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let d: *mut Object = msg_send![app, delegate];
            if d.is_null() {
                return;
            }
            // the answers go to the app's delegate (winit's): add them to its class
            let cls = (*d).class() as *const Class as *mut Class;
            let types = c"v@:@@".as_ptr();
            let imp: objc::runtime::Imp = std::mem::transmute(
                registered as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
            );
            objc::runtime::class_addMethod(
                cls,
                sel!(application:didRegisterForRemoteNotificationsWithDeviceToken:),
                imp,
                types,
            );
            let imp: objc::runtime::Imp = std::mem::transmute(
                failed as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
            );
            objc::runtime::class_addMethod(
                cls,
                sel!(application:didFailToRegisterForRemoteNotificationsWithError:),
                imp,
                types,
            );
            let _: () = msg_send![app, registerForRemoteNotifications];
        }
    }

    pub fn push_token() -> Option<String> {
        TOKEN.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// What a pushed banner opens: "push:WS:TARGET" from its "ws" and
    /// "target", or from a server from before targets
    /// "push:WS:CHANNEL[:THREAD]" from its "channel" and "parent"
    /// (server/src/push.rs).
    unsafe fn pushed_target(info: *mut Object) -> Option<String> {
        let aps: *mut Object = msg_send![info, objectForKey: ns("aps")];
        if aps.is_null() {
            return None;
        }
        let number = |key: &str| -> Option<u64> {
            let n: *mut Object = msg_send![info, objectForKey: ns(key)];
            if n.is_null() {
                return None;
            }
            let ok: BOOL = msg_send![n, respondsToSelector: sel!(unsignedLongLongValue)];
            if ok == objc::runtime::NO {
                return None;
            }
            let v: u64 = msg_send![n, unsignedLongLongValue];
            Some(v)
        };
        let ws: *mut Object = msg_send![info, objectForKey: ns("ws")];
        let ok: BOOL = if ws.is_null() {
            objc::runtime::NO
        } else {
            msg_send![ws, isKindOfClass: class!(NSString)]
        };
        let ws = if ok == YES { from_ns(ws) } else { None };
        let mut t = format!("push:{}:", ws.unwrap_or_default());
        let target: *mut Object = msg_send![info, objectForKey: ns("target")];
        let is_text: BOOL = if target.is_null() {
            objc::runtime::NO
        } else {
            msg_send![target, isKindOfClass: class!(NSString)]
        };
        if is_text == YES {
            if let Some(target) = from_ns(target).filter(|t| !t.is_empty()) {
                t.push_str(&target);
                return Some(t);
            }
        }
        if let Some(ch) = number("channel") {
            t.push_str(&ch.to_string());
            if let Some(p) = number("parent") {
                t.push_str(&format!(":{}", p));
            }
        }
        Some(t)
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
                // a push (it has "aps") names its workspace; the app's own
                // banners carry only their target
                let t: *mut Object = msg_send![info, objectForKey: ns("target")];
                if let Some(t) = pushed_target(info).or_else(|| from_ns(t)) {
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
            note: *mut Object,
            done: *mut Object,
        ) {
            unsafe {
                // a notice on the clock handed over ahead of time: with the
                // app in front it shows in the app's own toast instead
                let req: *mut Object = msg_send![note, request];
                let id: *mut Object = msg_send![req, identifier];
                let planned = from_ns(id).is_some_and(|i| i.starts_with(super::PLANNED_PREFIX));
                let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
                let active: BOOL = msg_send![app, isActive];
                if !done.is_null() {
                    let b = &*(done as *const Block<(usize,), ()>);
                    if planned && active == YES {
                        b.call((0,));
                    } else {
                        // list | banner | sound
                        b.call(((1 << 3) | (1 << 4) | (1 << 1),));
                    }
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
            // macOS hides the Dock badge of an app that never asked for
            // notification permission, and the ask used to wait for the
            // first banner (which may never come while the app is in front)
            if !label.is_empty() && center_ok() {
                center();
            }
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

/// A toast's XML: title and body as plain text, silent unless `sound`.
#[cfg(any(windows, test))]
fn toast_xml(title: &str, body: &str, sound: bool) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    };
    format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual>{}</toast>",
        esc(title),
        esc(body),
        if sound { "" } else { "<audio silent=\"true\"/>" }
    )
}

/// The taskbar overlay: a red dot `size` pixels wide with the count in white
/// ("9+" past nine: the icon is tiny), as RGBA. None for no count.
#[cfg(any(windows, test))]
fn badge_pixels(label: &str, size: usize) -> Option<Vec<u8>> {
    const GLYPHS: [(char, [u8; 5]); 11] = [
        ('0', [7, 5, 5, 5, 7]),
        ('1', [2, 6, 2, 2, 7]),
        ('2', [7, 1, 7, 4, 7]),
        ('3', [7, 1, 7, 1, 7]),
        ('4', [5, 5, 7, 1, 1]),
        ('5', [7, 4, 7, 1, 7]),
        ('6', [7, 4, 7, 5, 7]),
        ('7', [7, 1, 1, 1, 1]),
        ('8', [7, 5, 7, 5, 7]),
        ('9', [7, 5, 7, 1, 7]),
        ('+', [0, 2, 7, 2, 0]),
    ];
    let text = match label.trim() {
        "" => return None,
        l if l.len() == 1 && l.chars().all(|c| c.is_ascii_digit()) => l.to_string(),
        _ => "9+".to_string(),
    };
    let mut px = vec![0u8; size * size * 4];
    // the dot, with a soft edge
    let c = (size as f32 - 1.0) / 2.0;
    let r = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let a = (r - d + 0.5).clamp(0.0, 1.0);
            let i = (y * size + x) * 4;
            px[i..i + 4].copy_from_slice(&[220, 38, 38, (a * 255.0) as u8]);
        }
    }
    // the digits: 3x5 cells, one cell apart, as large as two of them fit
    let s = ((size.saturating_sub(2)) / 7).max(1);
    let n = text.chars().count();
    let w = n * 3 * s + (n - 1) * s;
    let (x0, y0) = (size.saturating_sub(w) / 2, size.saturating_sub(5 * s) / 2);
    for (k, ch) in text.chars().enumerate() {
        let rows = GLYPHS.iter().find(|g| g.0 == ch).map(|g| g.1)?;
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..3 {
                if bits & (4 >> col) == 0 {
                    continue;
                }
                for dy in 0..s {
                    for dx in 0..s {
                        let x = x0 + k * 4 * s + col * s + dx;
                        let y = y0 + row * s + dy;
                        if x < size && y < size {
                            let i = (y * size + x) * 4;
                            px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                        }
                    }
                }
            }
        }
    }
    Some(px)
}

#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{mpsc, Mutex, OnceLock};
    use windows::core::{HSTRING, PCWSTR};
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::Win32::Foundation::{BOOL, HWND};
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
    use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateIconFromResourceEx, DestroyIcon, FlashWindowEx, GetForegroundWindow,
        GetSystemMetrics, IsIconic, SetForegroundWindow, ShowWindow, FLASHWINFO, FLASHW_TIMERNOFG,
        FLASHW_TRAY, HICON, LR_DEFAULTCOLOR, SM_CXSMICON, SW_RESTORE,
    };
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

    /// Who the toasts are from. Registered for this person under
    /// HKEY_CURRENT_USER with the app's name and icon, so no installer is needed.
    const AUMID: &str = "Workboard.Desktop";

    static WINDOW: AtomicIsize = AtomicIsize::new(0);
    static CTX: OnceLock<egui::Context> = OnceLock::new();
    static CLICKED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    /// Shown toasts: a click only reaches the app while its toast is kept.
    static SHOWN: Mutex<Vec<ToastNotification>> = Mutex::new(Vec::new());
    static BADGE: Mutex<String> = Mutex::new(String::new());

    type Job = Box<dyn FnOnce() + Send>;

    /// Toasts and the taskbar talk to Windows through COM, which can be slow
    /// the first time: they run on a thread of their own, not in the frame.
    fn run(job: impl FnOnce() + Send + 'static) {
        static WORKER: OnceLock<Mutex<mpsc::Sender<Job>>> = OnceLock::new();
        let tx = WORKER.get_or_init(|| {
            let (tx, rx) = mpsc::channel::<Job>();
            let _ = std::thread::Builder::new()
                .name("notifier".into())
                .spawn(move || {
                    unsafe {
                        let _ = RoInitialize(RO_INIT_MULTITHREADED);
                    }
                    register();
                    for job in rx {
                        job();
                    }
                });
            Mutex::new(tx)
        });
        let _ = tx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .send(Box::new(job));
    }

    fn window() -> HWND {
        HWND(WINDOW.load(Ordering::Relaxed) as _)
    }

    pub fn attach(cc: &eframe::CreationContext<'_>) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        if let Ok(h) = cc.window_handle() {
            if let RawWindowHandle::Win32(w) = h.as_raw() {
                WINDOW.store(w.hwnd.get(), Ordering::Relaxed);
            }
        }
        let _ = CTX.set(cc.egui_ctx.clone());
    }

    /// The app's name and icon for its toasts (HKCU\Software\Classes\AppUserModelId).
    fn register() {
        let key = format!(r"HKCU\Software\Classes\AppUserModelId\{}", AUMID);
        let icon = crate::model::Data::dir().join("notification-icon.png");
        if !icon.exists() {
            let _ = std::fs::create_dir_all(crate::model::Data::dir());
            let _ = std::fs::write(&icon, include_bytes!("../Resources/AppIcon-256.png"));
        }
        let icon = icon.display().to_string();
        for args in [
            vec![key.as_str(), "/v", "DisplayName", "/d", "Workboard", "/f"],
            vec![key.as_str(), "/v", "IconUri", "/d", icon.as_str(), "/f"],
        ] {
            let _ = crate::os::quiet_command("reg")
                .arg("add")
                .args(args)
                .status();
        }
    }

    pub fn post(title: &str, body: &str, target: &str, sound: bool) {
        let xml = super::toast_xml(title, body, sound);
        let target = target.to_string();
        run(move || {
            if let Err(e) = show(&xml, target) {
                eprintln!("notification: {}", e);
            }
        });
    }

    fn show(xml: &str, target: String) -> windows::core::Result<()> {
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        toast.Activated(&TypedEventHandler::new(move |_, _| {
            CLICKED
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(target.clone());
            bring_forward();
            if let Some(ctx) = CTX.get() {
                ctx.request_repaint();
            }
            Ok(())
        }))?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?.Show(&toast)?;
        let mut shown = SHOWN.lock().unwrap_or_else(|e| e.into_inner());
        shown.push(toast);
        let extra = shown.len().saturating_sub(30);
        shown.drain(..extra);
        Ok(())
    }

    pub fn take_clicked() -> Vec<String> {
        std::mem::take(&mut *CLICKED.lock().unwrap_or_else(|e| e.into_inner()))
    }

    pub fn set_badge(label: &str) {
        {
            let mut b = BADGE.lock().unwrap_or_else(|e| e.into_inner());
            if *b == label {
                return;
            }
            *b = label.to_string();
        }
        let label = label.to_string();
        run(move || {
            if let Err(e) = overlay(&label) {
                eprintln!("taskbar badge: {}", e);
            }
        });
    }

    fn overlay(label: &str) -> windows::core::Result<()> {
        let hwnd = window();
        if hwnd.0.is_null() {
            return Ok(());
        }
        unsafe {
            let bar: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
            bar.HrInit()?;
            let size = GetSystemMetrics(SM_CXSMICON).clamp(16, 64) as usize;
            let Some(px) = super::badge_pixels(label, size) else {
                return bar.SetOverlayIcon(hwnd, HICON::default(), PCWSTR::null());
            };
            let mut png = vec![];
            let encoded =
                image::RgbaImage::from_raw(size as u32, size as u32, px).and_then(|img| {
                    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                        .ok()
                });
            if encoded.is_none() {
                return Ok(());
            }
            // icon resources may be PNG; 0x00030000 is the icon format version
            let icon = CreateIconFromResourceEx(
                &png,
                BOOL(1),
                0x0003_0000,
                size as i32,
                size as i32,
                LR_DEFAULTCOLOR,
            )?;
            let r = bar.SetOverlayIcon(hwnd, icon, &HSTRING::from(label));
            let _ = DestroyIcon(icon);
            r
        }
    }

    pub fn bounce(until_seen: bool) {
        let hwnd = window();
        if hwnd.0.is_null() || unsafe { GetForegroundWindow() } == hwnd {
            return;
        }
        let info = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd,
            dwFlags: if until_seen {
                FLASHW_TRAY | FLASHW_TIMERNOFG
            } else {
                FLASHW_TRAY
            },
            uCount: if until_seen { 0 } else { 2 },
            dwTimeout: 0,
        };
        unsafe {
            let _ = FlashWindowEx(&info);
        }
    }

    pub fn play_sound() {
        unsafe {
            let _ = PlaySoundW(
                &HSTRING::from("Notification.Default"),
                None,
                SND_ALIAS | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }

    pub fn bring_forward() {
        let hwnd = window();
        if hwnd.0.is_null() {
            return;
        }
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_text_is_plain_text_in_the_xml() {
        let x = toast_xml("Kim <dev>", "Tom & \"Jerry\"", false);
        assert!(x.contains("<text>Kim &lt;dev&gt;</text>"));
        assert!(x.contains("<text>Tom &amp; &quot;Jerry&quot;</text>"));
        assert!(x.contains("silent=\"true\""));
        assert!(!toast_xml("a", "b", true).contains("silent"));
    }

    #[test]
    fn the_taskbar_count_is_a_red_dot_with_white_digits() {
        assert!(badge_pixels("", 16).is_none());
        for size in [16, 20, 24, 32] {
            for label in ["3", "12", "99+"] {
                let px = badge_pixels(label, size).unwrap();
                assert_eq!(px.len(), size * size * 4);
                let at = |x: usize, y: usize| &px[(y * size + x) * 4..(y * size + x) * 4 + 4];
                assert_eq!(at(0, 0)[3], 0, "the corners are clear");
                assert_eq!(at(size / 2, 1), [220, 38, 38, 255], "the dot is red");
                assert!(
                    px.chunks(4).any(|p| p == [255, 255, 255, 255]),
                    "{} at {}px shows digits",
                    label,
                    size
                );
            }
        }
        // more than nine reads "9+": the same pixels for any big count
        assert_eq!(badge_pixels("12", 16), badge_pixels("99+", 16));
        assert_ne!(badge_pixels("3", 16), badge_pixels("8", 16));
    }
}
