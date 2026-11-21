// Screen sharing on macOS (see screen.rs): ScreenCaptureKit through the
// Objective-C runtime, with libwebrtc's capturer where it isn't there.

use super::*;
use block::ConcreteBlock;
use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Protocol, Sel, BOOL, YES};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::{c_char, c_void, CStr};
use std::ptr::null_mut;
use std::sync::mpsc;
use std::time::Duration;

#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct CGRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

unsafe impl objc::Encode for CMTime {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{?=qiIq}") }
    }
}

unsafe impl objc::Encode for CGRect {
    fn encode() -> objc::Encoding {
        unsafe { objc::Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGMainDisplayID() -> u32;
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayIsBuiltin(display: u32) -> u32;
    fn CGDisplayPixelsWide(display: u32) -> usize;
    fn CGDisplayPixelsHigh(display: u32) -> usize;
}

#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    fn CMSampleBufferGetImageBuffer(sbuf: *mut c_void) -> *mut c_void;
}

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    fn CVPixelBufferLockBaseAddress(pb: *mut c_void, flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pb: *mut c_void, flags: u64) -> i32;
    fn CVPixelBufferGetWidth(pb: *mut c_void) -> usize;
    fn CVPixelBufferGetHeight(pb: *mut c_void) -> usize;
    fn CVPixelBufferGetPlaneCount(pb: *mut c_void) -> usize;
    fn CVPixelBufferGetBaseAddressOfPlane(pb: *mut c_void, plane: usize) -> *mut u8;
    fn CVPixelBufferGetBytesPerRowOfPlane(pb: *mut c_void, plane: usize) -> usize;
    fn CVPixelBufferGetHeightOfPlane(pb: *mut c_void, plane: usize) -> usize;
}

// NSScreen, for display names
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

extern "C" {
    fn dlopen(path: *const c_char, mode: i32) -> *mut c_void;
    fn dispatch_queue_create(label: *const c_char, attr: *mut c_void) -> *mut Object;
    fn dispatch_release(obj: *mut Object);
}

/// '420v': NV12 with video-range luma, what the encoder wants anyway.
const NV12: u32 = 0x3432_3076;
/// SCStreamErrorUserDeclined, SCStreamErrorUserStopped.
const DECLINED: i64 = -3801;
const USER_STOPPED: i64 = -3817;
const READ_ONLY: u64 = 1;

pub fn allowed() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

pub fn ask() -> bool {
    unsafe { CGRequestScreenCaptureAccess() }
}

/// ScreenCaptureKit is there (macOS 12.3 and later); loads it the first time.
fn sck() -> bool {
    static LOADED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LOADED.get_or_init(|| unsafe {
        let path = c"/System/Library/Frameworks/ScreenCaptureKit.framework/ScreenCaptureKit";
        !dlopen(path.as_ptr(), 2).is_null()
            && Class::get("SCShareableContent").is_some()
            && Class::get("SCStream").is_some()
            && Class::get("SCContentFilter").is_some()
    })
}

unsafe fn ns(s: &str) -> *mut Object {
    let c = std::ffi::CString::new(s).unwrap_or_default();
    msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()]
}

unsafe fn string(s: *mut Object) -> Option<String> {
    if s.is_null() {
        return None;
    }
    let p: *const c_char = msg_send![s, UTF8String];
    (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
}

unsafe fn items(array: *mut Object) -> Vec<*mut Object> {
    if array.is_null() {
        return vec![];
    }
    let n: usize = msg_send![array, count];
    (0..n).map(|i| msg_send![array, objectAtIndex: i]).collect()
}

unsafe fn trouble(err: *mut Object) -> ScreenTrouble {
    if err.is_null() {
        return ScreenTrouble::Failed("Screen capture failed.".into());
    }
    let code: isize = msg_send![err, code];
    if code as i64 == DECLINED || !allowed() {
        return ScreenTrouble::NotAllowed;
    }
    let what: *mut Object = msg_send![err, localizedDescription];
    ScreenTrouble::Failed(string(what).unwrap_or_else(|| format!("error {}", code)))
}

/// Display names from NSScreen, by display id.
unsafe fn screen_names() -> Vec<(u32, String)> {
    let screens: *mut Object = msg_send![class!(NSScreen), screens];
    let key = ns("NSScreenNumber");
    let mut out = vec![];
    for s in items(screens) {
        let d: *mut Object = msg_send![s, deviceDescription];
        let n: *mut Object = msg_send![d, objectForKey: key];
        if n.is_null() {
            continue;
        }
        let id: u32 = msg_send![n, unsignedIntValue];
        let responds: BOOL = msg_send![s, respondsToSelector: sel!(localizedName)];
        if responds == YES {
            let name: *mut Object = msg_send![s, localizedName];
            if let Some(name) = string(name) {
                out.push((id, name));
            }
        }
    }
    out
}

unsafe fn display_name(id: u32, n: usize, names: &[(u32, String)]) -> String {
    if let Some((_, name)) = names.iter().find(|(i, _)| *i == id) {
        return name.clone();
    }
    if CGDisplayIsBuiltin(id) != 0 {
        "Built-in display".into()
    } else {
        format!(
            "Display {} ({}x{})",
            n + 1,
            CGDisplayPixelsWide(id),
            CGDisplayPixelsHigh(id)
        )
    }
}

/// The shareable displays and windows (an SCShareableContent, retained).
unsafe fn content() -> Result<*mut Object, ScreenTrouble> {
    let (tx, rx) = mpsc::channel::<Result<usize, ScreenTrouble>>();
    let tx = Mutex::new(Some(tx));
    let done = ConcreteBlock::new(move |content: *mut Object, err: *mut Object| {
        let r = if content.is_null() {
            Err(trouble(err))
        } else {
            let _: *mut Object = msg_send![content, retain];
            Ok(content as usize)
        };
        if let Some(tx) = tx.lock().unwrap().take() {
            let _ = tx.send(r);
        }
    })
    .copy();
    let _: () = msg_send![class!(SCShareableContent), getShareableContentExcludingDesktopWindows: YES onScreenWindowsOnly: YES completionHandler: &*done];
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(r) => r.map(|p| p as *mut Object),
        Err(_) => Err(ScreenTrouble::Failed(
            "macOS didn't answer with the screens to share.".into(),
        )),
    }
}

/// Windows worth offering: normal windows of other apps, with a title.
unsafe fn window_ok(w: *mut Object) -> Option<(u32, String)> {
    let layer: isize = msg_send![w, windowLayer];
    let frame: CGRect = msg_send![w, frame];
    let title: *mut Object = msg_send![w, title];
    let title = string(title).filter(|t| !t.trim().is_empty())?;
    let app: *mut Object = msg_send![w, owningApplication];
    if layer != 0 || frame.w < 120.0 || frame.h < 80.0 || app.is_null() {
        return None;
    }
    let pid: i32 = msg_send![app, processID];
    if pid as u32 == std::process::id() {
        return None;
    }
    let name: *mut Object = msg_send![app, applicationName];
    let id: u32 = msg_send![w, windowID];
    Some((
        id,
        match string(name).filter(|n| !n.is_empty()) {
            Some(app) => format!("{}: {}", app, title),
            None => title,
        },
    ))
}

pub fn sources() -> Result<Vec<Source>, ScreenTrouble> {
    unsafe {
        let names = screen_names();
        let main = CGMainDisplayID();
        let mut out = vec![];
        if sck() {
            let c = content()?;
            let displays: *mut Object = msg_send![c, displays];
            for (n, d) in items(displays).into_iter().enumerate() {
                let id: u32 = msg_send![d, displayID];
                out.push(Source {
                    kind: SourceKind::Display,
                    name: display_name(id, n, &names),
                    main: id == main,
                    id,
                });
            }
            let windows: *mut Object = msg_send![c, windows];
            let mut wins: Vec<Source> = items(windows)
                .into_iter()
                .filter_map(|w| window_ok(w))
                .map(|(id, name)| Source {
                    kind: SourceKind::Window,
                    id,
                    name,
                    main: false,
                })
                .collect();
            wins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            out.extend(wins);
            let _: () = msg_send![c, release];
        } else {
            let mut ids = [0u32; 16];
            let mut n = 0u32;
            if CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut n) == 0 {
                for (i, id) in ids[..n as usize].iter().enumerate() {
                    out.push(Source {
                        kind: SourceKind::Display,
                        id: *id,
                        name: display_name(*id, i, &names),
                        main: *id == main,
                    });
                }
            }
        }
        out.sort_by_key(|s| (s.kind == SourceKind::Window, !s.main));
        Ok(out)
    }
}

pub fn capture(
    share: Share,
    source: &NativeVideoSource,
    limits: &Limits,
    stop: &AtomicBool,
) -> Result<(), ScreenTrouble> {
    if !allowed() {
        return Err(ScreenTrouble::NotAllowed);
    }
    if sck() {
        unsafe { run_sck(share, source, limits, stop) }
    } else {
        run_webrtc(share, source, limits, stop)
    }
}

/// Frames from the stream: the newest one goes into `Shared` (the ivar
/// holds an Arc<Shared> made with Arc::into_raw).
extern "C" fn did_output(this: &Object, _: Sel, _: *mut Object, sbuf: *mut c_void, kind: isize) {
    unsafe {
        let state: *mut c_void = *this.get_ivar("state");
        if kind != 0 || state.is_null() || sbuf.is_null() {
            return;
        }
        // frames where nothing changed come without a picture
        let pb = CMSampleBufferGetImageBuffer(sbuf);
        if pb.is_null() || CVPixelBufferLockBaseAddress(pb, READ_ONLY) != 0 {
            return;
        }
        if CVPixelBufferGetPlaneCount(pb) == 2 {
            let (w, h) = (
                CVPixelBufferGetWidth(pb) as u32,
                CVPixelBufferGetHeight(pb) as u32,
            );
            let y = CVPixelBufferGetBaseAddressOfPlane(pb, 0);
            let uv = CVPixelBufferGetBaseAddressOfPlane(pb, 1);
            let (sy, suv) = (
                CVPixelBufferGetBytesPerRowOfPlane(pb, 0),
                CVPixelBufferGetBytesPerRowOfPlane(pb, 1),
            );
            let (hy, huv) = (
                CVPixelBufferGetHeightOfPlane(pb, 0),
                CVPixelBufferGetHeightOfPlane(pb, 1),
            );
            if !y.is_null() && !uv.is_null() && w >= 2 && h >= 2 {
                let p = Planes::from_nv12(
                    std::slice::from_raw_parts(y, sy * hy),
                    sy as u32,
                    std::slice::from_raw_parts(uv, suv * huv),
                    suv as u32,
                    w,
                    h,
                );
                let shared = &*(state as *const Shared);
                *shared.latest.lock().unwrap() = Some(p);
            }
        }
        CVPixelBufferUnlockBaseAddress(pb, READ_ONLY);
    }
}

/// The stream stopped by itself (window closed, permission taken away,
/// or the person pressed stop in macOS's menu bar).
extern "C" fn did_stop(this: &Object, _: Sel, _: *mut Object, err: *mut Object) {
    unsafe {
        let state: *mut c_void = *this.get_ivar("state");
        if state.is_null() {
            return;
        }
        let why = if err.is_null() {
            None
        } else {
            let code: isize = msg_send![err, code];
            if code as i64 == USER_STOPPED {
                None
            } else {
                Some(trouble(err))
            }
        };
        let shared = &*(state as *const Shared);
        *shared.ended.lock().unwrap() = Some(why);
    }
}

fn output_class() -> &'static Class {
    if let Some(c) = Class::get("WorkboardScreenOutput") {
        return c;
    }
    let mut decl =
        ClassDecl::new("WorkboardScreenOutput", class!(NSObject)).expect("declare screen output");
    decl.add_ivar::<*mut c_void>("state");
    unsafe {
        decl.add_method(
            sel!(stream:didOutputSampleBuffer:ofType:),
            did_output as extern "C" fn(&Object, Sel, *mut Object, *mut c_void, isize),
        );
        decl.add_method(
            sel!(stream:didStopWithError:),
            did_stop as extern "C" fn(&Object, Sel, *mut Object, *mut Object),
        );
    }
    for p in ["SCStreamOutput", "SCStreamDelegate"] {
        if let Some(p) = Protocol::get(p) {
            decl.add_protocol(p);
        }
    }
    decl.register()
}

/// Run a completion-handler call that reports an NSError (or nil), and
/// wait for it.
unsafe fn wait_done(
    call: impl FnOnce(&block::Block<(*mut Object,), ()>),
) -> Result<(), ScreenTrouble> {
    let (tx, rx) = mpsc::channel::<Result<(), ScreenTrouble>>();
    let tx = Mutex::new(Some(tx));
    let done = ConcreteBlock::new(move |err: *mut Object| {
        let r = if err.is_null() {
            Ok(())
        } else {
            Err(trouble(err))
        };
        if let Some(tx) = tx.lock().unwrap().take() {
            let _ = tx.send(r);
        }
    })
    .copy();
    call(&done);
    rx.recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| Err(ScreenTrouble::Failed("macOS didn't answer.".into())))
}

unsafe fn run_sck(
    share: Share,
    source: &NativeVideoSource,
    limits: &Limits,
    stop: &AtomicBool,
) -> Result<(), ScreenTrouble> {
    let content = content()?;
    // the filter for what to share, and its size in pixels
    let mut chosen: Option<(*mut Object, f64, f64)> = None;
    match share {
        Share::Display(id) => {
            let displays: *mut Object = msg_send![content, displays];
            let all = items(displays);
            let d = all
                .iter()
                .copied()
                .find(|d| {
                    let did: u32 = msg_send![*d, displayID];
                    did == id
                })
                .or_else(|| all.first().copied());
            if let Some(d) = d {
                let empty: *mut Object = msg_send![class!(NSArray), array];
                let f: *mut Object = msg_send![class!(SCContentFilter), alloc];
                let f: *mut Object = msg_send![f, initWithDisplay: d excludingWindows: empty];
                let w: isize = msg_send![d, width];
                let h: isize = msg_send![d, height];
                chosen = Some((f, w as f64, h as f64));
            }
        }
        Share::Window(id) => {
            let windows: *mut Object = msg_send![content, windows];
            if let Some(win) = items(windows).into_iter().find(|w| {
                let wid: u32 = msg_send![*w, windowID];
                wid == id
            }) {
                let f: *mut Object = msg_send![class!(SCContentFilter), alloc];
                let f: *mut Object = msg_send![f, initWithDesktopIndependentWindow: win];
                let frame: CGRect = msg_send![win, frame];
                chosen = Some((f, frame.w, frame.h));
            }
        }
        Share::Pattern(_) => unreachable!(),
    }
    let _: () = msg_send![content, release];
    let Some((filter, pw, ph)) = chosen else {
        return Err(ScreenTrouble::Gone);
    };
    if filter.is_null() {
        return Err(ScreenTrouble::Failed("Couldn't share that.".into()));
    }
    // points to pixels (macOS 14 tells; older: assume a Retina screen)
    let has_scale: BOOL = msg_send![filter, respondsToSelector: sel!(pointPixelScale)];
    let scale = if has_scale == YES {
        let s: f32 = msg_send![filter, pointPixelScale];
        s.max(1.0) as f64
    } else {
        2.0
    };
    // within 1920x1080 (a tall window gets narrower, not taller)
    let (w, h) = video::fit_size(
        (pw * scale) as u32,
        (ph * scale) as u32,
        quality::SCREEN_WIDTH,
        quality::SCREEN_WIDTH * 9 / 16,
    );
    let config: *mut Object = msg_send![class!(SCStreamConfiguration), new];
    let _: () = msg_send![config, setWidth: w as usize];
    let _: () = msg_send![config, setHeight: h as usize];
    let _: () = msg_send![config, setPixelFormat: NV12];
    let _: () = msg_send![config, setShowsCursor: YES];
    let _: () = msg_send![config, setQueueDepth: 5isize];
    let _: () = msg_send![config, setMinimumFrameInterval: CMTime { value: 1, timescale: quality::SCREEN_FPS as i32, flags: 1, epoch: 0 }];

    let shared = Arc::new(Shared::default());
    let handler: *mut Object = msg_send![output_class(), new];
    let state = Arc::into_raw(shared.clone()) as *mut c_void;
    (*handler).set_ivar("state", state);
    let stream: *mut Object = msg_send![class!(SCStream), alloc];
    let stream: *mut Object =
        msg_send![stream, initWithFilter: filter configuration: config delegate: handler];
    let queue = dispatch_queue_create(c"workboard.screen".as_ptr(), null_mut());
    let mut err: *mut Object = null_mut();
    let added: BOOL = msg_send![stream, addStreamOutput: handler type: 0isize sampleHandlerQueue: queue error: &mut err];
    let started = if added == YES {
        wait_done(|done| {
            let _: () = msg_send![stream, startCaptureWithCompletionHandler: done];
        })
    } else {
        Err(trouble(err))
    };
    let r = match started {
        Ok(()) => {
            let r = pump(source, limits, stop, &shared, || {});
            let _ = wait_done(|done| {
                let _: () = msg_send![stream, stopCaptureWithCompletionHandler: done];
            });
            r
        }
        Err(e) => Err(e),
    };
    // no more frames come once stopped: let go of everything
    (*handler).set_ivar("state", null_mut::<c_void>());
    let _: () = msg_send![stream, release];
    let _: () = msg_send![handler, release];
    let _: () = msg_send![filter, release];
    let _: () = msg_send![config, release];
    dispatch_release(queue);
    // a callback could still be running on the queue: give it a moment
    std::thread::sleep(Duration::from_millis(50));
    drop(Arc::from_raw(state as *const Shared));
    r
}
