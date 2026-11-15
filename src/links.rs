// workboard:// links opened from a browser or a mail (an invite:
// workboard://join?server=...&code=...&email=...). macOS hands them to the
// app as a "get URL" Apple event, which can come before the window exists;
// they wait here until the app takes them on its next frame. The bundle
// declares the scheme (CFBundleURLTypes in scripts/bundle.sh).

use std::sync::Mutex;

#[cfg(not(test))]
static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());
static WAKE: Mutex<Option<egui::Context>> = Mutex::new(None);

/// The links not taken yet. Tests run side by side in one process, each
/// with its own app, so there each test thread keeps its own.
#[cfg(not(test))]
fn pending<R>(f: impl FnOnce(&mut Vec<String>) -> R) -> R {
    f(&mut PENDING.lock().unwrap())
}

#[cfg(test)]
fn pending<R>(f: impl FnOnce(&mut Vec<String>) -> R) -> R {
    thread_local! {
        static PENDING: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    PENDING.with(|p| f(&mut p.borrow_mut()))
}

/// A link arrived: keep it and redraw so the app takes it.
pub fn arrived(url: String) {
    pending(|p| p.push(url));
    if let Some(ctx) = WAKE.lock().unwrap().as_ref() {
        ctx.request_repaint();
    }
}

/// Redraw this context when a link arrives.
pub fn wake(ctx: &egui::Context) {
    *WAKE.lock().unwrap() = Some(ctx.clone());
}

/// The newest workboard:// link not taken yet.
pub fn take() -> Option<String> {
    pending(|p| {
        let last = p.pop();
        p.clear();
        last
    })
}

/// Ask macOS to send workboard:// links here. Call before the event loop starts.
#[cfg(target_os = "macos")]
pub fn listen() {
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::CStr;
    use std::os::raw::c_char;

    /// 'GURL': both the event class and the event id of "open this URL".
    const GURL: u32 = u32::from_be_bytes(*b"GURL");
    /// keyDirectObject, '----': where the URL is in the event.
    const DIRECT: u32 = u32::from_be_bytes(*b"----");

    extern "C" fn handle(_this: &Object, _cmd: Sel, event: *mut Object, _reply: *mut Object) {
        unsafe {
            let desc: *mut Object = msg_send![event, paramDescriptorForKeyword: DIRECT];
            if desc.is_null() {
                return;
            }
            let text: *mut Object = msg_send![desc, stringValue];
            if text.is_null() {
                return;
            }
            let utf8: *const c_char = msg_send![text, UTF8String];
            if !utf8.is_null() {
                arrived(CStr::from_ptr(utf8).to_string_lossy().into_owned());
            }
        }
    }

    unsafe {
        let cls = match Class::get("WorkboardLinkHandler") {
            Some(c) => c,
            None => {
                let Some(mut decl) = ClassDecl::new("WorkboardLinkHandler", class!(NSObject))
                else {
                    return;
                };
                decl.add_method(
                    sel!(handleURLEvent:withReplyEvent:),
                    handle as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
                );
                decl.register()
            }
        };
        let handler: *mut Object = msg_send![cls, new];
        let manager: *mut Object = msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
        let _: () = msg_send![manager, setEventHandler: handler andSelector: sel!(handleURLEvent:withReplyEvent:) forEventClass: GURL andEventID: GURL];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn listen() {}
