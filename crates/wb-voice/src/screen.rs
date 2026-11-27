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
// picture is sent again every STILL_AGAIN: someone who joins later, or a
// layer the server switches to, still gets a picture of a still screen.
//
// Capturing needs the Screen Recording permission (System Settings > Privacy
// & Security > Screen & System Audio Recording). `allowed` checks it without
// asking, `ask` shows macOS's request (once), `open_settings` opens the page.
//
// On Windows the displays, the windows and the picture come from libwebrtc's
// own capturer, which LiveKit already links in: Windows Graphics Capture,
// falling back to DXGI desktop duplication and GDI. Windows has no
// permission for this, so `allowed` and `ask` are always true and
// `open_settings` does nothing.

use crate::quality;
use crate::video::{self, Feed, Limits, Picture};
use livekit::webrtc::native::yuv_helper;
use livekit::webrtc::prelude::I420Buffer;
use livekit::webrtc::video_source::native::NativeVideoSource;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

/// Open System Settings at the Screen Recording page (macOS; elsewhere there
/// is no such setting, and it does nothing).
pub fn open_settings() {
    #[cfg(target_os = "macos")]
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

/// Your own shared screen is shown at most this wide (a copy of what is sent).
pub const PREVIEW_WIDTH: u32 = 960;

/// Where each shared picture goes: the encoder, and a smaller copy to the
/// window so the sharer sees what the others see.
pub(crate) struct Out {
    source: NativeVideoSource,
    preview: Box<dyn Fn(Option<Picture>) + Send>,
}

impl Out {
    fn send(&self, mut buf: I420Buffer, started: Instant) {
        (self.preview)(Some(video::small_picture(&mut buf, PREVIEW_WIDTH)));
        video::push(&self.source, buf, started);
    }
}

/// Start sharing. `preview` gets a copy of each picture sent (at most
/// PREVIEW_WIDTH wide, to show the sharer), and None when sharing ends.
/// `ended` is called when sharing stops without being asked to (with why,
/// or None when it was stopped from macOS's own controls); it isn't called
/// when the returned Feed is dropped.
pub fn start_screen(
    share: Share,
    source: NativeVideoSource,
    limits: Arc<Limits>,
    preview: impl Fn(Option<Picture>) + Send + 'static,
    ended: impl Fn(Option<ScreenTrouble>) + Send + 'static,
) -> Feed {
    let (feed, stop, finished) = Feed::new();
    std::thread::Builder::new()
        .name("workboard-screen".into())
        .spawn(move || {
            let out = Out {
                source,
                preview: Box::new(preview),
            };
            let r = match share {
                Share::Pattern(rgb) => {
                    video::run_pattern(
                        rgb,
                        quality::SCREEN_WIDTH,
                        &limits,
                        &stop,
                        |buf, started| out.send(buf, started),
                    );
                    Ok(())
                }
                _ => {
                    eprintln!("workboard screen: starting");
                    imp::capture(share, &out, &limits, &stop)
                }
            };
            if let Err(e) = &r {
                eprintln!("workboard screen: {:?}", e);
            }
            // last, so no picture sent just before stopping stays on screen
            (out.preview)(None);
            finished.store(true, Ordering::Relaxed);
            if !stop.load(Ordering::Relaxed) {
                ended(r.err());
            }
        })
        .expect("start screen thread");
    feed
}

/// How often the last picture of a still screen is sent again.
pub(crate) const STILL_AGAIN: Duration = Duration::from_secs(1);

/// Whether to send the last picture now: a new one came, or the screen has
/// been still for STILL_AGAIN since the last one was sent.
pub(crate) fn send_now(new: bool, last_sent: Option<Instant>, now: Instant) -> bool {
    new || last_sent.is_some_and(|t| now.duration_since(t) >= STILL_AGAIN)
}

/// Send each new picture at most at the frame rate (and the last one again
/// while the screen is still) until stopped or capture ends; `each` runs
/// once a round (to ask for a frame).
fn pump(
    out: &Out,
    limits: &Limits,
    stop: &AtomicBool,
    shared: &Shared,
    mut each: impl FnMut(),
) -> Result<(), ScreenTrouble> {
    let started = Instant::now();
    let mut last: Option<Planes> = None;
    let mut last_sent: Option<Instant> = None;
    while !stop.load(Ordering::Relaxed) {
        each();
        let new = match shared.latest.lock().unwrap().take() {
            Some(p) => {
                last = Some(p);
                true
            }
            None => false,
        };
        if let Some(why) = shared.ended.lock().unwrap().take() {
            return why.map_or(Ok(()), Err);
        }
        let now = Instant::now();
        if let Some(p) = last.as_ref().filter(|_| send_now(new, last_sent, now)) {
            out.send(p.to_buffer(limits.width()), started);
            last_sent = Some(now);
        }
        std::thread::sleep(limits.gap());
    }
    Ok(())
}

#[cfg(not(target_os = "ios"))]
/// libwebrtc's capturer, started on `share`: on macOS CGDisplayStream for
/// displays (where ScreenCaptureKit isn't there) and CGWindowList for
/// windows; on Windows Windows Graphics Capture (or DXGI, or GDI). Frames
/// arrive in the returned `Shared` each time `capture_frame` is called.
fn open_webrtc(
    share: Share,
) -> Result<
    (
        livekit::webrtc::desktop_capturer::DesktopCapturer,
        Arc<Shared>,
    ),
    ScreenTrouble,
> {
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
    // (a Windows window id is a handle, which only uses its low 32 bits)
    let pick = cap
        .get_source_list()
        .into_iter()
        .find(|s| s.id() as u32 == id);
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
    Ok((cap, shared))
}

/// Share through libwebrtc's capturer until stopped.
#[cfg(not(target_os = "ios"))]
fn run_webrtc(
    share: Share,
    out: &Out,
    limits: &Limits,
    stop: &AtomicBool,
) -> Result<(), ScreenTrouble> {
    let (mut cap, shared) = open_webrtc(share)?;
    pump(out, limits, stop, &shared, || cap.capture_frame())
}

/// iPad: libwebrtc has no screen capturer there (iOS shares the screen
/// through a broadcast extension, not built yet), so there is nothing to share.
#[cfg(target_os = "ios")]
#[path = "screen/ios.rs"]
mod imp;

/// Windows and other systems: everything through libwebrtc's capturer.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
#[path = "screen/windows.rs"]
mod imp;

/// The little of Windows itself that sharing needs.
#[cfg(windows)]
pub(crate) mod os {
    use std::ffi::c_void;

    #[repr(C)]
    struct DisplayDevice {
        cb: u32,
        name: [u16; 32],
        string: [u16; 128],
        flags: u32,
        id: [u16; 128],
        key: [u16; 128],
    }

    const ACTIVE: u32 = 0x1;
    const PRIMARY: u32 = 0x4;

    #[link(name = "user32")]
    extern "system" {
        fn EnumDisplayDevicesW(
            device: *const u16,
            n: u32,
            dd: *mut DisplayDevice,
            flags: u32,
        ) -> i32;
    }

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
        fn CoUninitialize();
    }

    /// The display device number of the main display (libwebrtc's screen id).
    pub fn primary_screen() -> Option<u32> {
        for n in 0..64 {
            let mut d: DisplayDevice = unsafe { std::mem::zeroed() };
            d.cb = std::mem::size_of::<DisplayDevice>() as u32;
            if unsafe { EnumDisplayDevicesW(std::ptr::null(), n, &mut d, 0) } == 0 {
                break;
            }
            if d.flags & (ACTIVE | PRIMARY) == ACTIVE | PRIMARY {
                return Some(n);
            }
        }
        None
    }

    /// COM (and WinRT, which Windows Graphics Capture uses) on this thread
    /// while it lives.
    pub struct Com(bool);

    impl Com {
        /// `sta`: single-threaded, as Media Foundation in nokhwa asks for.
        fn start(sta: bool) -> Com {
            const MULTITHREADED: u32 = 0x0;
            const APARTMENT_THREADED: u32 = 0x2;
            const DISABLE_OLE1DDE: u32 = 0x4;
            let mode = if sta {
                APARTMENT_THREADED | DISABLE_OLE1DDE
            } else {
                MULTITHREADED
            };
            Com(unsafe { CoInitializeEx(std::ptr::null_mut(), mode) } >= 0)
        }

        pub fn init() -> Com {
            Com::start(false)
        }

        pub fn init_sta() -> Com {
            Com::start(true)
        }
    }

    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() }
            }
        }
    }
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub(crate) mod os {
    pub fn primary_screen() -> Option<u32> {
        None
    }

    pub struct Com;

    impl Com {
        pub fn init() -> Com {
            Com
        }

        pub fn init_sta() -> Com {
            Com
        }
    }
}

// objc 0.2's macros check a cfg this crate doesn't declare
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
#[path = "screen/macos.rs"]
mod imp;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality::Kind;

    #[test]
    fn a_still_screen_is_sent_again_once_a_second_not_at_the_frame_rate() {
        let t0 = Instant::now();
        // a new picture goes at once
        assert!(send_now(true, None, t0));
        assert!(send_now(true, Some(t0), t0));
        // nothing new: not again within the second
        assert!(!send_now(false, None, t0));
        assert!(!send_now(false, Some(t0), t0 + Duration::from_millis(100)));
        assert!(!send_now(false, Some(t0), t0 + Duration::from_millis(999)));
        // then again
        assert!(send_now(false, Some(t0), t0 + STILL_AGAIN));
    }

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
        let feed = start_screen(
            share,
            source,
            Limits::new(Kind::Screen),
            |_| {},
            move |why| *e.lock().unwrap() = Some(why),
        );
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

    /// Windows: the displays are listed (main one first) and libwebrtc's
    /// capturer gives a picture of the main one, the way sharing does.
    #[test]
    #[cfg(windows)]
    fn windows_lists_and_captures_the_main_display() {
        let list = sources().expect("sources");
        eprintln!("{:?}", list);
        let main = list.first().expect("a display");
        assert!(main.main && main.kind == SourceKind::Display, "{:?}", list);
        let _com = os::Com::init();
        let (mut cap, shared) = open_webrtc(main.share()).expect("start capturing");
        let until = Instant::now() + std::time::Duration::from_secs(10);
        let p = loop {
            cap.capture_frame();
            if let Some(p) = shared.latest.lock().unwrap().take() {
                break p;
            }
            if let Some(why) = shared.ended.lock().unwrap().take() {
                panic!("capture ended: {:?}", why);
            }
            assert!(Instant::now() < until, "no picture in 10 s");
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        eprintln!("main display: {}x{}", p.w, p.h);
        assert!(p.w >= 320 && p.h >= 200, "{}x{}", p.w, p.h);
        // and it can be sent
        let pic = video::i420_to_picture(&p.to_buffer(quality::SCREEN_WIDTH));
        assert!(pic.width <= quality::SCREEN_WIDTH);
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
