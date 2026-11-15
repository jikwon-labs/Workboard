// Sharing a screen (or one window) in a huddle.
//
// On macOS 12.3 and later the picture comes from ScreenCaptureKit, called
// through the Objective-C runtime like src/media.rs does: SCShareableContent
// lists the displays and windows, an SCStream delivers frames (scaled by the
// system to the size we send, NV12) to a small Objective-C object declared
// here, and libyuv turns them into I420 for the encoder. ScreenCaptureKit is
// loaded with dlopen, so the app still starts on macOS 11, where libwebrtc's
// own capturer (CGDisplayStream) is used instead.
//
// A screen only sends frames when something on it changes, so the last
// picture is sent again at the frame rate: someone who joins later, or a
// layer the server switches to, still gets a picture of a still screen.
//
// Capturing needs the Screen Recording permission (System Settings > Privacy
// & Security > Screen & System Audio Recording). `allowed` checks it without
// asking, `ask` shows macOS's request (once), `open_settings` opens the page.

use crate::quality;
use crate::video::{self, Feed, Limits};
use livekit::webrtc::native::yuv_helper;
use livekit::webrtc::prelude::I420Buffer;
use livekit::webrtc::video_source::native::NativeVideoSource;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// What to share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Share {
    /// A whole display (CGDirectDisplayID).
    Display(u32),
    /// One window (CGWindowID).
    Window(u32),
    /// A solid color with a moving bar; tests use it.
    Pattern([u8; 3]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Display,
    Window,
}

/// A display or window that can be shared.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub kind: SourceKind,
    pub id: u32,
    /// "Built-in Retina Display", "Safari: Workboard docs".
    pub name: String,
    pub main: bool,
}

impl Source {
    pub fn share(&self) -> Share {
        match self.kind {
            SourceKind::Display => Share::Display(self.id),
            SourceKind::Window => Share::Window(self.id),
        }
    }
}

/// Why sharing stopped (or couldn't start).
#[derive(Clone, Debug, PartialEq)]
pub enum ScreenTrouble {
    /// No Screen Recording permission.
    NotAllowed,
    /// The display was unplugged or the window closed.
    Gone,
    Failed(String),
}

/// Whether Workboard may record the screen (asks nothing).
pub fn allowed() -> bool {
    imp::allowed()
}

/// Ask for the Screen Recording permission. macOS shows its window only the
/// first time; after that the person has to turn it on in System Settings
/// (and reopen the app).
pub fn ask() -> bool {
    imp::ask()
}

/// Open System Settings at the Screen Recording page.
pub fn open_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .spawn();
}

/// The displays (main one first) and windows that can be shared.
pub fn sources() -> Result<Vec<Source>, ScreenTrouble> {
    if !allowed() {
        return Err(ScreenTrouble::NotAllowed);
    }
    imp::sources()
}

/// Raw I420 planes with tight rows, passed from the capture thread.
pub(crate) struct Planes {
    w: u32,
    h: u32,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl Planes {
    fn empty(w: u32, h: u32) -> Planes {
        let (w, h) = (w & !1, h & !1);
        let c = (w / 2 * (h / 2)) as usize;
        Planes {
            w,
            h,
            y: vec![0; (w * h) as usize],
            u: vec![0; c],
            v: vec![0; c],
        }
    }

    /// From NV12 (a Y plane, then U and V interleaved at half size).
    #[allow(dead_code)]
    pub(crate) fn from_nv12(y: &[u8], sy: u32, uv: &[u8], suv: u32, w: u32, h: u32) -> Planes {
        let mut p = Planes::empty(w, h);
        let (pw, ph) = (p.w, p.h);
        yuv_helper::nv12_to_i420(
            y,
            sy,
            uv,
            suv,
            &mut p.y,
            pw,
            &mut p.u,
            pw / 2,
            &mut p.v,
            pw / 2,
            pw as i32,
            ph as i32,
        );
        p
    }

    /// From BGRA (libwebrtc's desktop frames; libyuv calls it ARGB).
    pub(crate) fn from_bgra(src: &[u8], stride: u32, w: u32, h: u32) -> Planes {
        let mut p = Planes::empty(w, h);
        let (pw, ph) = (p.w, p.h);
        yuv_helper::argb_to_i420(
            src,
            stride,
            &mut p.y,
            pw,
            &mut p.u,
            pw / 2,
            &mut p.v,
            pw / 2,
            pw as i32,
            ph as i32,
        );
        p
    }

    /// A frame to send, at most `max` wide.
    fn to_buffer(&self, max: u32) -> I420Buffer {
        let mut buf = I420Buffer::new(self.w, self.h);
        let (sy, su, sv) = buf.strides();
        let (dy, du, dv) = buf.data_mut();
        let cw = (self.w / 2) as usize;
        for r in 0..self.h as usize {
            let w = self.w as usize;
            dy[r * sy as usize..r * sy as usize + w].copy_from_slice(&self.y[r * w..r * w + w]);
        }
        for r in 0..(self.h / 2) as usize {
            du[r * su as usize..r * su as usize + cw].copy_from_slice(&self.u[r * cw..r * cw + cw]);
            dv[r * sv as usize..r * sv as usize + cw].copy_from_slice(&self.v[r * cw..r * cw + cw]);
        }
        if self.w > max {
            let (ow, oh) = video::send_size(self.w, self.h, max);
            buf.scale(ow as i32, oh as i32)
        } else {
            buf
        }
    }
}

/// Between the capture callbacks and the sending loop.
#[derive(Default)]
pub(crate) struct Shared {
    /// The newest picture, taken by the sending loop.
    latest: Mutex<Option<Planes>>,
    /// Set when capture ended by itself: why (None: the person stopped it
    /// from macOS's own menu).
    ended: Mutex<Option<Option<ScreenTrouble>>>,
}

/// Start sharing. `ended` is called when sharing stops without being asked
/// to (with why, or None when it was stopped from macOS's own controls); it
/// isn't called when the returned Feed is dropped.
pub fn start_screen(
    share: Share,
    source: NativeVideoSource,
    limits: Arc<Limits>,
    ended: impl Fn(Option<ScreenTrouble>) + Send + 'static,
) -> Feed {
    let (feed, stop, finished) = Feed::new();
    std::thread::Builder::new()
        .name("workboard-screen".into())
        .spawn(move || {
            let r = match share {
                Share::Pattern(rgb) => {
                    run_pattern(rgb, &source, &limits, &stop);
                    Ok(())
                }
                _ => {
                    eprintln!("workboard screen: starting");
                    imp::capture(share, &source, &limits, &stop)
                }
            };
            if let Err(e) = &r {
                eprintln!("workboard screen: {:?}", e);
            }
            finished.store(true, Ordering::Relaxed);
            if !stop.load(Ordering::Relaxed) {
                ended(r.err());
            }
        })
        .expect("start screen thread");
    feed
}

fn run_pattern(rgb: [u8; 3], source: &NativeVideoSource, limits: &Limits, stop: &AtomicBool) {
    let started = Instant::now();
    let mut n = 0u32;
    while !stop.load(Ordering::Relaxed) {
        let (w, h) = video::send_size(
            quality::SCREEN_WIDTH,
            quality::SCREEN_WIDTH * 9 / 16,
            limits.width(),
        );
        let bar = (n * w / 40) % w;
        video::push(source, video::pattern_i420(w, h, rgb, bar), started);
        n += 1;
        std::thread::sleep(limits.gap());
    }
}

/// Send the newest picture (or the last one again) at the frame rate until
/// stopped or capture ends; `each` runs once a round (to ask for a frame).
fn pump(
    source: &NativeVideoSource,
    limits: &Limits,
    stop: &AtomicBool,
    shared: &Shared,
    mut each: impl FnMut(),
) -> Result<(), ScreenTrouble> {
    let started = Instant::now();
    let mut last: Option<Planes> = None;
    while !stop.load(Ordering::Relaxed) {
        each();
        if let Some(p) = shared.latest.lock().unwrap().take() {
            last = Some(p);
        }
        if let Some(why) = shared.ended.lock().unwrap().take() {
            return why.map_or(Ok(()), Err);
        }
        if let Some(p) = &last {
            video::push(source, p.to_buffer(limits.width()), started);
        }
        std::thread::sleep(limits.gap());
    }
    Ok(())
}

/// libwebrtc's capturer: CGDisplayStream for displays (where ScreenCaptureKit
/// isn't there), CGWindowList for windows.
fn run_webrtc(
    share: Share,
    source: &NativeVideoSource,
    limits: &Limits,
    stop: &AtomicBool,
) -> Result<(), ScreenTrouble> {
    use livekit::webrtc::desktop_capturer::{
        CaptureError, DesktopCaptureSourceType, DesktopCapturer, DesktopCapturerOptions,
    };
    let (ty, id) = match share {
        Share::Display(id) => (DesktopCaptureSourceType::Screen, id),
        Share::Window(id) => (DesktopCaptureSourceType::Window, id),
        Share::Pattern(_) => unreachable!(),
    };
    let mut opts = DesktopCapturerOptions::new(ty);
    opts.set_include_cursor(true);
    #[cfg(target_os = "macos")]
    opts.set_sck_system_picker(false);
    let mut cap = DesktopCapturer::new(opts)
        .ok_or_else(|| ScreenTrouble::Failed("Couldn't start capturing the screen.".into()))?;
    let pick = cap
        .get_source_list()
        .into_iter()
        .find(|s| s.id() == id as u64);
    if pick.is_none() && matches!(share, Share::Window(_)) {
        return Err(ScreenTrouble::Gone);
    }
    let shared = Arc::new(Shared::default());
    let s = shared.clone();
    cap.start_capture(pick, move |r| match r {
        Ok(f) if f.width() >= 2 && f.height() >= 2 => {
            let p = Planes::from_bgra(f.data(), f.stride(), f.width() as u32, f.height() as u32);
            *s.latest.lock().unwrap() = Some(p);
        }
        Ok(_) | Err(CaptureError::Temporary) => {}
        Err(CaptureError::Permanent) => {
            *s.ended.lock().unwrap() = Some(Some(if allowed() {
                ScreenTrouble::Failed("Screen capture stopped.".into())
            } else {
                ScreenTrouble::NotAllowed
            }));
        }
    });
    pump(source, limits, stop, &shared, || cap.capture_frame())
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;
    pub fn allowed() -> bool {
        true
    }
    pub fn ask() -> bool {
        true
    }
    pub fn sources() -> Result<Vec<Source>, ScreenTrouble> {
        Err(ScreenTrouble::Failed("Screen sharing needs macOS.".into()))
    }
    pub fn capture(
        share: Share,
        source: &NativeVideoSource,
        limits: &Limits,
        stop: &AtomicBool,
    ) -> Result<(), ScreenTrouble> {
        run_webrtc(share, source, limits, stop)
    }
}

// objc 0.2's macros check a cfg this crate doesn't declare
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
mod imp {
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
    extern "C" fn did_output(
        this: &Object,
        _: Sel,
        _: *mut Object,
        sbuf: *mut c_void,
        kind: isize,
    ) {
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
        let mut decl = ClassDecl::new("WorkboardScreenOutput", class!(NSObject))
            .expect("declare screen output");
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality::Kind;

    #[test]
    fn frames_convert_and_scale_to_the_limit() {
        // a blue BGRA picture, 64x36
        let (w, h) = (64u32, 36u32);
        let px: Vec<u8> = std::iter::repeat([220u8, 40, 30, 255])
            .take((w * h) as usize)
            .flatten()
            .collect();
        let p = Planes::from_bgra(&px, w * 4, w, h);
        let pic = video::i420_to_picture(&p.to_buffer(1920));
        assert_eq!((pic.width, pic.height), (64, 36));
        let a = pic.average();
        assert!(a[2] > 180 && a[0] < 70, "{:?}", a);
        // scaled down to the limit
        let small = video::i420_to_picture(&p.to_buffer(32));
        assert_eq!((small.width, small.height), (32, 18));
        // NV12 too (odd sizes are cut to even)
        let (y, u, v) = (81u8, 90u8, 240u8); // red
        let y_plane = vec![y; 65 * 37];
        let uv: Vec<u8> = std::iter::repeat([u, v]).take(33 * 19).flatten().collect();
        let p = Planes::from_nv12(&y_plane, 65, &uv, 66, 65, 37);
        let pic = video::i420_to_picture(&p.to_buffer(1920));
        assert_eq!((pic.width, pic.height), (64, 36));
        assert!(pic.average()[0] > 200, "{:?}", pic.average());
    }

    /// Capture `share` for real and give the size of the first picture with
    /// something on it.
    fn capture_one(share: Share) -> (u32, u32) {
        use livekit::webrtc::prelude::VideoBuffer;
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _in_rt = rt.enter();
        let source = NativeVideoSource::new(
            livekit::webrtc::video_source::VideoResolution {
                width: 1920,
                height: 1080,
            },
            true,
        );
        let track = livekit::prelude::LocalVideoTrack::create_video_track(
            "screen",
            livekit::webrtc::prelude::RtcVideoSource::Native(source.clone()),
        );
        let mut stream =
            livekit::webrtc::video_stream::native::NativeVideoStream::new(track.rtc_track());
        let ended = Arc::new(Mutex::new(None));
        let e = ended.clone();
        let feed = start_screen(share, source, Limits::new(Kind::Screen), move |why| {
            *e.lock().unwrap() = Some(why)
        });
        // the first frames can be the source's black keep-alive pictures:
        // wait for one with something on it
        let got = rt.block_on(async {
            use futures_util::StreamExt;
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                while let Some(f) = stream.next().await {
                    let b = f.buffer.to_i420();
                    let (y, _, _) = b.data();
                    let avg = y.iter().map(|v| *v as u64).sum::<u64>() / y.len().max(1) as u64;
                    if avg > 20 {
                        return Some((b.width(), b.height()));
                    }
                }
                None
            })
            .await
        });
        drop(feed);
        assert!(
            ended.lock().unwrap().is_none(),
            "{:?}",
            ended.lock().unwrap()
        );
        got.expect("a frame within 10 s").expect("stream open")
    }

    /// Real capture of this computer's main display and of a window, for a
    /// hands-on check: `cargo test -p wb-voice real_ -- --ignored --nocapture`.
    /// Needs the Screen Recording permission for the terminal.
    #[test]
    #[ignore]
    fn real_screen_and_window_capture_send_frames() {
        assert!(allowed(), "no Screen Recording permission");
        let list = sources().expect("sources");
        let displays = list
            .iter()
            .filter(|s| s.kind == SourceKind::Display)
            .count();
        eprintln!("{} displays, {} windows", displays, list.len() - displays);
        let main = list.iter().find(|s| s.main).expect("a main display");
        let (w, h) = capture_one(main.share());
        eprintln!("display: {}x{}", w, h);
        assert!(w >= 640 && w <= quality::SCREEN_WIDTH, "{}x{}", w, h);
        if let Some(win) = list.iter().find(|s| s.kind == SourceKind::Window) {
            let (w, h) = capture_one(win.share());
            eprintln!("window: {}x{}", w, h);
            assert!(w >= 100 && w <= quality::SCREEN_WIDTH, "{}x{}", w, h);
        }
    }
}
