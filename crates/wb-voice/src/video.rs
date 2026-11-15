// Video for huddles (stage 4): pictures to send (the camera, or a test
// pattern) and pictures received, as RGBA the window can draw.
//
// The camera is sent at up to quality::CAMERA_WIDTH (1280) pixels wide, which
// becomes the top simulcast layer; when the connection is poor the feed sends
// smaller pictures, fewer a second (`Limits`, set from quality::limits).
// Camera colors are converted here (BT.601, the usual for webcams) rather
// than through libwebrtc's helpers, so the byte order is never a guess;
// received pictures go through libyuv (I420 to RGBA bytes, checked by the
// round-trip test below).

use crate::quality::{self, Kind};
use livekit::webrtc::native::yuv_helper;
use livekit::webrtc::prelude::{I420Buffer, VideoBuffer, VideoFrame, VideoRotation};
use livekit::webrtc::video_source::native::NativeVideoSource;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// At most this many received pictures a second go to the window.
pub const FPS: u64 = 15;
/// Your own camera picture (the preview) is made this small.
pub const PREVIEW_WIDTH: u32 = 320;

/// How large and how often to send, changed while sending (the network).
pub struct Limits {
    width: AtomicU32,
    fps: AtomicU32,
}

impl Limits {
    pub fn new(kind: Kind) -> Arc<Limits> {
        let (w, f) = quality::limits(kind, quality::Network::Good);
        Arc::new(Limits {
            width: AtomicU32::new(w),
            fps: AtomicU32::new(f),
        })
    }

    pub fn set(&self, (width, fps): (u32, u32)) {
        self.width.store(width.max(2), Ordering::Relaxed);
        self.fps.store(fps.max(1), Ordering::Relaxed);
    }

    pub fn width(&self) -> u32 {
        self.width.load(Ordering::Relaxed)
    }

    pub fn fps(&self) -> u32 {
        self.fps.load(Ordering::Relaxed)
    }

    pub fn gap(&self) -> Duration {
        Duration::from_millis(1000 / self.fps() as u64)
    }
}

/// A picture to draw: RGBA, row after row.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Picture {
    /// Average color, for tests.
    pub fn average(&self) -> [u8; 3] {
        let n = (self.width * self.height).max(1) as u64;
        let mut s = [0u64; 3];
        for px in self.rgba.chunks_exact(4) {
            for i in 0..3 {
                s[i] += px[i] as u64;
            }
        }
        [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8]
    }
}

fn clamp(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (r, g, b) = (r as i32, g as i32, b as i32);
    let y = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
    let u = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
    let v = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
    (clamp(y), clamp(u), clamp(v))
}

/// The size to send a `w` x `h` picture at (even numbers, at most `max` wide).
pub fn send_size(w: u32, h: u32, max: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    let (ow, oh) = if w > max {
        (max, (h as u64 * max as u64 / w as u64) as u32)
    } else {
        (w, h)
    };
    (ow & !1, oh.max(2) & !1)
}

/// The size to send a `w` x `h` picture at to fit in `max_w` x `max_h`
/// (even numbers, never larger than it is).
pub fn fit_size(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    let by_h = (w as u64 * max_h as u64 / h as u64) as u32;
    send_size(w, h, max_w.min(by_h.max(2)))
}

/// Build an I420 frame of size `ow` x `oh` by sampling `pixel(x, y)` on the
/// source grid `w` x `h` (nearest neighbour).
fn build_i420(
    w: u32,
    h: u32,
    ow: u32,
    oh: u32,
    pixel: impl Fn(u32, u32) -> (u8, u8, u8),
) -> I420Buffer {
    let mut buf = I420Buffer::new(ow, oh);
    let (sy, su, sv) = buf.strides();
    let (dy, du, dv) = buf.data_mut();
    for oy in 0..oh {
        let y = oy * h / oh;
        for ox in 0..ow {
            let x = ox * w / ow;
            let (yy, uu, vv) = pixel(x, y);
            dy[(oy * sy + ox) as usize] = yy;
            if oy % 2 == 0 && ox % 2 == 0 {
                du[((oy / 2) * su + ox / 2) as usize] = uu;
                dv[((oy / 2) * sv + ox / 2) as usize] = vv;
            }
        }
    }
    buf
}

/// I420 from packed RGB (3 bytes a pixel).
pub fn rgb_to_i420(rgb: &[u8], w: u32, h: u32, max: u32) -> I420Buffer {
    let (ow, oh) = send_size(w, h, max);
    build_i420(w, h, ow, oh, |x, y| {
        let i = ((y * w + x) * 3) as usize;
        rgb_to_yuv(rgb[i], rgb[i + 1], rgb[i + 2])
    })
}

/// I420 from YUYV (YUY2), what many cameras send.
pub fn yuyv_to_i420(src: &[u8], w: u32, h: u32, max: u32) -> I420Buffer {
    let (ow, oh) = send_size(w, h, max);
    build_i420(w, h, ow, oh, |x, y| {
        let i = ((y * w + (x & !1)) * 2) as usize;
        let yy = if x % 2 == 0 { src[i] } else { src[i + 2] };
        (yy, src[i + 1], src[i + 3])
    })
}

/// I420 from NV12 (a Y plane, then interleaved U and V at half size).
pub fn nv12_to_i420(src: &[u8], w: u32, h: u32, max: u32) -> I420Buffer {
    let (ow, oh) = send_size(w, h, max);
    let uv0 = (w * h) as usize;
    build_i420(w, h, ow, oh, |x, y| {
        let c = uv0 + ((y / 2) * w + (x & !1)) as usize;
        (src[(y * w + x) as usize], src[c], src[c + 1])
    })
}

/// RGBA from an I420 frame. (libyuv's "ABGR" is R, G, B, A in memory.)
pub fn i420_to_picture(buf: &I420Buffer) -> Picture {
    let (w, h) = (buf.width(), buf.height());
    let (sy, su, sv) = buf.strides();
    let (dy, du, dv) = buf.data();
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    yuv_helper::i420_to_abgr(dy, sy, du, su, dv, sv, &mut rgba, w * 4, w as i32, h as i32);
    Picture {
        width: w,
        height: h,
        rgba,
    }
}

/// A smaller copy of a frame to show (at most `max` wide); the frame itself
/// when it is small already.
pub fn small_picture(buf: &mut I420Buffer, max: u32) -> Picture {
    let (w, h) = (buf.width(), buf.height());
    if w <= max {
        return i420_to_picture(buf);
    }
    let (ow, oh) = send_size(w, h, max);
    i420_to_picture(&buf.scale(ow as i32, oh as i32))
}

/// A solid `w` x `h` picture with a light vertical bar at `bar` (test pictures).
pub fn pattern_i420(w: u32, h: u32, rgb: [u8; 3], bar: u32) -> I420Buffer {
    let mut buf = I420Buffer::new(w, h);
    let (sy, su, _) = buf.strides();
    let (y0, u0, v0) = rgb_to_yuv(rgb[0], rgb[1], rgb[2]);
    let (y1, u1, v1) = rgb_to_yuv(255, 255, 255);
    let bar_w = (w / 40).max(2) & !1;
    let bar = bar & !1;
    let (dy, du, dv) = buf.data_mut();
    dy.fill(y0);
    du.fill(u0);
    dv.fill(v0);
    for row in 0..h {
        let r = (row * sy) as usize;
        for x in bar..(bar + bar_w).min(w) {
            dy[r + x as usize] = y1;
        }
    }
    for row in 0..h.div_ceil(2) {
        let r = (row * su) as usize;
        for x in bar / 2..((bar + bar_w) / 2).min(w.div_ceil(2)) {
            du[r + x as usize] = u1;
            dv[r + x as usize] = v1;
        }
    }
    buf
}

/// Where the pictures this computer sends come from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Video {
    /// The computer's camera (and its screens, when shared).
    Camera,
    /// A solid color with a moving bar instead of the camera, and another
    /// color instead of a shared screen; tests use it.
    Pattern([u8; 3]),
}

/// Something that feeds pictures into a video source until stopped.
pub struct Feed {
    stop: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
}

impl Feed {
    /// A feed, the flag that asks it to stop, and the flag it sets when its
    /// thread is over.
    pub(crate) fn new() -> (Feed, Arc<AtomicBool>, Arc<AtomicBool>) {
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        (
            Feed {
                stop: stop.clone(),
                finished: finished.clone(),
            },
            stop,
            finished,
        )
    }

    /// Sending ended by itself (the camera or screen capture stopped).
    pub fn stopped(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Start sending pictures. `preview` gets a small copy of each one sent (to
/// show yourself), and None when sending ends; `trouble` gets a message when
/// the camera can't be used.
pub fn start_feed(
    video: Video,
    source: NativeVideoSource,
    limits: Arc<Limits>,
    preview: impl Fn(Option<Picture>) + Send + 'static,
    trouble: impl Fn(String) + Send + 'static,
) -> Feed {
    let (feed, s, finished) = Feed::new();
    std::thread::Builder::new()
        .name("workboard-video".into())
        .spawn(move || {
            let show = |p: Picture| preview(Some(p));
            match video {
                Video::Pattern(rgb) => run_pattern(rgb, &source, &limits, &s, &show),
                Video::Camera => {
                    eprintln!("workboard camera: starting");
                    match run_camera(&source, &limits, &s, &show) {
                        Ok(()) => eprintln!("workboard camera: stopped"),
                        Err(e) => {
                            eprintln!("workboard camera: {}", e);
                            if !s.load(Ordering::Relaxed) {
                                trouble(e);
                            }
                        }
                    }
                }
            }
            // last, so no picture sent just before stopping stays on screen
            preview(None);
            finished.store(true, Ordering::Relaxed);
        })
        .expect("start video thread");
    feed
}

/// Hand a frame to the encoder.
pub(crate) fn push(source: &NativeVideoSource, buf: I420Buffer, started: Instant) {
    let mut frame = VideoFrame::new(VideoRotation::VideoRotation0, buf);
    frame.timestamp_us = started.elapsed().as_micros() as i64;
    source.capture_frame(&frame);
}

fn send(
    source: &NativeVideoSource,
    mut buf: I420Buffer,
    started: Instant,
    preview: &impl Fn(Picture),
) {
    preview(small_picture(&mut buf, PREVIEW_WIDTH));
    push(source, buf, started);
}

fn run_pattern(
    rgb: [u8; 3],
    source: &NativeVideoSource,
    limits: &Limits,
    stop: &AtomicBool,
    preview: &impl Fn(Picture),
) {
    let started = Instant::now();
    let mut n = 0u32;
    while !stop.load(Ordering::Relaxed) {
        // the camera's size (or less on a poor connection); a light bar
        // moves across, so frames differ like real video
        let (w, h) = send_size(quality::CAMERA_WIDTH, quality::CAMERA_WIDTH * 9 / 16, limits.width());
        let bar = (n * w / 40) % w;
        send(source, pattern_i420(w, h, rgb, bar), started, preview);
        n += 1;
        std::thread::sleep(limits.gap());
    }
}

/// Ask macOS for the camera (the first time it shows the permission window).
fn camera_allowed() -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    nokhwa::nokhwa_initialize(move |ok| {
        let _ = tx.send(ok);
    });
    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(true) => Ok(()),
        Ok(false) => Err("Workboard isn't allowed to use the camera. Turn it on in System Settings > Privacy & Security > Camera.".into()),
        Err(_) => Err("The camera didn't answer (no permission was given).".into()),
    }
}

fn run_camera(
    source: &NativeVideoSource,
    limits: &Limits,
    stop: &AtomicBool,
    preview: &impl Fn(Picture),
) -> Result<(), String> {
    use nokhwa::pixel_format::RgbFormat;
    use nokhwa::utils::{ApiBackend, FrameFormat, RequestedFormat, RequestedFormatType};
    camera_allowed()?;
    let cams =
        nokhwa::query(ApiBackend::Auto).map_err(|e| format!("Couldn't look for cameras: {}", e))?;
    let first = cams.first().ok_or("No camera found on this computer.")?;
    // frame_raw, not frame: frame() reconfigures the camera on every call.
    // (The camera lock nokhwa leaked is fixed in third_party/nokhwa-bindings-macos.)
    let want = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut cam = nokhwa::Camera::new(first.index().clone(), want)
        .map_err(|e| format!("Couldn't open the camera: {}", e))?;
    // the smallest picture at least as wide as the top layer, at a usual
    // frame rate (else the widest there is): it's scaled to fit anyway
    if let Ok(formats) = cam.compatible_camera_formats() {
        let usual: Vec<_> = formats
            .iter()
            .filter(|f| f.frame_rate() >= quality::CAMERA_FPS)
            .collect();
        let pick = usual
            .iter()
            .filter(|f| f.resolution().width() >= quality::CAMERA_WIDTH)
            .min_by_key(|f| (f.resolution().width(), u32::MAX - f.frame_rate()))
            .or_else(|| usual.iter().max_by_key(|f| f.resolution().width()));
        if let Some(f) = pick {
            let _ = cam.set_camera_requset(RequestedFormat::new::<RgbFormat>(
                RequestedFormatType::Exact(**f),
            ));
        }
    }
    cam.open_stream()
        .map_err(|e| format!("Couldn't start the camera: {}", e))?;
    let fmt = cam.camera_format();
    let (w, h) = (fmt.resolution().width(), fmt.resolution().height());
    let started = Instant::now();
    let mut last = Instant::now() - limits.gap();
    while !stop.load(Ordering::Relaxed) {
        let raw = cam
            .frame_raw()
            .map_err(|e| format!("The camera stopped: {}", e))?;
        if last.elapsed() < limits.gap() {
            continue;
        }
        last = Instant::now();
        let max = limits.width();
        let buf = match fmt.format() {
            FrameFormat::YUYV if raw.len() >= (w * h * 2) as usize => yuyv_to_i420(&raw, w, h, max),
            FrameFormat::NV12 if raw.len() >= (w * h * 3 / 2) as usize => {
                nv12_to_i420(&raw, w, h, max)
            }
            FrameFormat::RAWRGB if raw.len() >= (w * h * 3) as usize => {
                rgb_to_i420(&raw, w, h, max)
            }
            other => {
                let frame = nokhwa::Buffer::new(fmt.resolution(), &raw, other);
                let img = frame
                    .decode_image::<RgbFormat>()
                    .map_err(|e| format!("Couldn't read the camera picture: {}", e))?;
                rgb_to_i420(img.as_raw(), img.width(), img.height(), max)
            }
        };
        send(source, buf, started, preview);
    }
    let _ = cam.stop_stream();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_survive_the_round_trip() {
        for rgb in [
            [200u8, 30, 40],
            [20, 180, 60],
            [30, 60, 210],
            [128, 128, 128],
        ] {
            let px: Vec<u8> = std::iter::repeat(rgb).take(16 * 8).flatten().collect();
            let p = i420_to_picture(&rgb_to_i420(&px, 16, 8, 640));
            let a = p.average();
            for i in 0..3 {
                assert!(
                    (a[i] as i32 - rgb[i] as i32).abs() <= 4,
                    "{:?} came back as {:?}",
                    rgb,
                    a
                );
            }
        }
    }

    #[test]
    fn camera_formats_convert_and_scale_down() {
        assert_eq!(send_size(1920, 1080, 640), (640, 360));
        assert_eq!(send_size(1920, 1080, 1280), (1280, 720));
        assert_eq!(send_size(320, 181, 640), (320, 180));
        assert_eq!(fit_size(3024, 1964, 1920, 1080), (1662, 1078));
        assert_eq!(fit_size(1200, 1300, 1920, 1080), (996, 1078));
        assert_eq!(fit_size(800, 600, 1920, 1080), (800, 600));
        // YUYV: two pixels share U and V
        let (y, u, v) = rgb_to_yuv(250, 20, 20);
        let yuyv: Vec<u8> = std::iter::repeat([y, u, y, v])
            .take(1280 * 720 / 2)
            .flatten()
            .collect();
        let p = i420_to_picture(&yuyv_to_i420(&yuyv, 1280, 720, 640));
        assert_eq!((p.width, p.height), (640, 360));
        assert!(p.average()[0] > 200 && p.average()[1] < 60);
        let mut nv12 = vec![y; 64 * 32];
        nv12.extend(std::iter::repeat([u, v]).take(64 * 32 / 4).flatten());
        assert!(i420_to_picture(&nv12_to_i420(&nv12, 64, 32, 640)).average()[0] > 200);
    }

    #[test]
    fn test_pictures_previews_and_limits() {
        let mut p = pattern_i420(1280, 720, [200, 30, 40], 100);
        let full = i420_to_picture(&p);
        assert_eq!((full.width, full.height), (1280, 720));
        let a = full.average();
        assert!(a[0] > 170 && a[1] < 80, "{:?}", a);
        // the bar is white
        let i = ((10 * 1280 + 102) * 4) as usize;
        assert!(full.rgba[i..i + 3].iter().all(|c| *c > 230));
        // the preview is small
        let small = small_picture(&mut p, PREVIEW_WIDTH);
        assert_eq!((small.width, small.height), (320, 180));
        // limits follow the network
        let l = Limits::new(Kind::Camera);
        assert_eq!(l.width(), quality::CAMERA_WIDTH);
        l.set(quality::limits(Kind::Camera, quality::Network::Poor));
        assert_eq!((l.width(), l.fps()), (640, 10));
        assert_eq!(l.gap(), Duration::from_millis(100));
    }
}
