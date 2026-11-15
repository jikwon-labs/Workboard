// workboard:// links opened from outside the app (a browser, Notes, the
// `open` command). The bundle's Info.plist claims the scheme (see
// scripts/bundle.sh); macOS then sends a "get URL" Apple Event, which winit
// does not pass on, so a small handler is registered with the Apple Event
// manager here. The links wait in a queue the app reads every frame.

use std::sync::{Mutex, OnceLock};

static QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());
static CTX: OnceLock<egui::Context> = OnceLock::new();

/// The links received since the last call.
pub fn take() -> Vec<String> {
    QUEUE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}

fn push(url: String) {
    if let Ok(mut q) = QUEUE.lock() {
        q.push(url);
    }
    if let Some(ctx) = CTX.get() {
        ctx.request_repaint();
    }
}

/// Lets a link arriving while the app is idle wake it up.
pub fn set_context(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
}

/// Registers the handler. Call on the main thread before the event loop
/// starts, so the link that launched the app is not missed.
#[cfg(target_os = "macos")]
pub fn install() {
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    // 'GURL' (event class and id) and '----' (the direct object)
    const GURL: u32 = 0x4755_524C;
    const DIRECT: u32 = 0x2D2D_2D2D;

    extern "C" fn handle(_this: &Object, _cmd: Sel, event: *mut Object, _reply: *mut Object) {
        unsafe {
            if event.is_null() {
                return;
            }
            let desc: *mut Object = msg_send![event, paramDescriptorForKeyword: DIRECT];
            if desc.is_null() {
                return;
            }
            let s: *mut Object = msg_send![desc, stringValue];
            if s.is_null() {
                return;
            }
            let p: *const std::os::raw::c_char = msg_send![s, UTF8String];
            if p.is_null() {
                return;
            }
            push(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned());
        }
    }

    unsafe {
        let cls = match Class::get("WorkboardUrlHandler") {
            Some(c) => c,
            None => {
                let Some(mut decl) = ClassDecl::new("WorkboardUrlHandler", class!(NSObject)) else { return };
                decl.add_method(sel!(handleUrl:withReply:), handle as extern "C" fn(&Object, Sel, *mut Object, *mut Object));
                decl.register()
            }
        };
        // kept for the life of the app
        let obj: *mut Object = msg_send![cls, new];
        let mgr: *mut Object = msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
        let _: () = msg_send![mgr, setEventHandler: obj andSelector: sel!(handleUrl:withReply:) forEventClass: GURL andEventID: GURL];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn install() {}

#[cfg(test)]
mod tests {
    #[test]
    fn queue_hands_links_over_once() {
        super::push("workboard://task/1".into());
        assert_eq!(super::take(), vec!["workboard://task/1".to_string()]);
        assert!(super::take().is_empty());
    }
}
