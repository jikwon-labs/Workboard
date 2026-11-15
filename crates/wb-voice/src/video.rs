// Video for huddles (stage 4): pictures to send (the camera, or a test
// pattern) and pictures received, as RGBA the window can draw.
//
// Everything sent is scaled down to at most SEND_WIDTH pixels wide, so the
// conversions stay cheap (about 640x360 at 15 frames a second). Colors are
// converted here (BT.601, the usual for webcams) rather than through
// libwebrtc's helpers, so the byte order is never a guess.

use livekit::webrtc::prelude::{I420Buffer, VideoBuffer, VideoFrame, VideoRotation};
use livekit::webrtc::video_source::native::NativeVideoSource;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const SEND_WIDTH: u32 = 640;
pub const FPS: u64 = 15;

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

fn yuv_to_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let c = y as i32 - 16;
    let d = u as i32 - 128;
    let e = v as i32 - 128;
    [
        clamp((298 * c + 409 * e + 128) >> 8),
        clamp((298 * c - 100 * d - 208 * e + 128) >> 8),
        clamp((298 * c + 516 * d + 128) >> 8),
    ]
}

/// The size to send a `w` x `h` picture at (even numbers, at most SEND_WIDTH wide).
pub fn send_size(w: u32, h: u32) -> (u32, u32) {
    let (w, h) = (w.max(2), h.max(2));
    let (ow, oh) = if w > SEND_WIDTH {
        (SEND_WIDTH, (h as u64 * SEND_WIDTH as u64 / w as u64) as u32)
    } else {
        (w, h)
    };
    (ow & !1, oh.max(2) & !1)
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
pub fn rgb_to_i420(rgb: &[u8], w: u32, h: u32) -> I420Buffer {
    let (ow, oh) = send_size(w, h);
    build_i420(w, h, ow, oh, |x, y| {
        let i = ((y * w + x) * 3) as usize;
        rgb_to_yuv(rgb[i], rgb[i + 1], rgb[i + 2])
    })
}

/// I420 from YUYV (YUY2), what many cameras send.
pub fn yuyv_to_i420(src: &[u8], w: u32, h: u32) -> I420Buffer {
    let (ow, oh) = send_size(w, h);
    build_i420(w, h, ow, oh, |x, y| {
        let i = ((y * w + (x & !1)) * 2) as usize;
        let yy = if x % 2 == 0 { src[i] } else { src[i + 2] };
        (yy, src[i + 1], src[i + 3])
    })
}

/// I420 from NV12 (a Y plane, then interleaved U and V at half size).
pub fn nv12_to_i420(src: &[u8], w: u32, h: u32) -> I420Buffer {
    let (ow, oh) = send_size(w, h);
    let uv0 = (w * h) as usize;
    build_i420(w, h, ow, oh, |x, y| {
        let c = uv0 + ((y / 2) * w + (x & !1)) as usize;
        (src[(y * w + x) as usize], src[c], src[c + 1])
    })
}

/// RGBA from an I420 frame.
pub fn i420_to_picture(buf: &I420Buffer) -> Picture {
    let (w, h) = (buf.width(), buf.height());
    let (sy, su, sv) = buf.strides();
    let (dy, du, dv) = buf.data();
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let c = yuv_to_rgb(
                dy[(y * sy + x) as usize],
                du[((y / 2) * su + x / 2) as usize],
                dv[((y / 2) * sv + x / 2) as usize],
            );
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    Picture {
        width: w,
        height: h,
        rgba,
    }
}

/// Where the pictures this computer sends come from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Video {
    /// The computer's camera.
    Camera,
    /// A solid color with a moving bar instead of the camera; tests use it.
    Pattern([u8; 3]),
}

/// Something that feeds pictures into a video source until stopped.
pub struct Feed {
    stop: Arc<AtomicBool>,
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Start sending pictures. `preview` gets each one sent (to show yourself),
/// and None when sending ends; `trouble` gets a message when the camera
/// can't be used.
pub fn start_feed(
    video: Video,
    source: NativeVideoSource,
    preview: impl Fn(Option<Picture>) + Send + 'static,
    trouble: impl Fn(String) + Send + 'static,
) -> Feed {
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    std::thread::Builder::new()
        .name("workboard-video".into())
        .spawn(move || {
            let show = |p: Picture| preview(Some(p));
            match video {
                Video::Pattern(rgb) => run_pattern(rgb, &source, &s, &show),
                Video::Camera => {
                    eprintln!("workboard camera: starting");
                    match run_camera(&source, &s, &show) {
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
        })
        .expect("start video thread");
    Feed { stop }
}

fn send(source: &NativeVideoSource, buf: I420Buffer, started: Instant, preview: &impl Fn(Picture)) {
    preview(i420_to_picture(&buf));
    let mut frame = VideoFrame::new(VideoRotation::VideoRotation0, buf);
    frame.timestamp_us = started.elapsed().as_micros() as i64;
    source.capture_frame(&frame);
}

fn run_pattern(
    rgb: [u8; 3],
    source: &NativeVideoSource,
    stop: &AtomicBool,
    preview: &impl Fn(Picture),
) {
    let (w, h) = (320u32, 180u32);
    let started = Instant::now();
    let mut n = 0u32;
    while !stop.load(Ordering::Relaxed) {
        // a light bar moves across, so frames differ like real video
        let bar = (n * 8) % w;
        let mut px = vec![0u8; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 3) as usize;
                let c = if x >= bar && x < bar + 8 {
                    [255, 255, 255]
                } else {
                    rgb
                };
                px[i..i + 3].copy_from_slice(&c);
            }
        }
        send(source, rgb_to_i420(&px, w, h), started, preview);
        n += 1;
        std::thread::sleep(Duration::from_millis(1000 / FPS));
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
    // the smallest picture at a usual frame rate: everything is scaled down anyway
    let want = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut cam = nokhwa::Camera::new(first.index().clone(), want)
        .map_err(|e| format!("Couldn't open the camera: {}", e))?;
    if let Ok(formats) = cam.compatible_camera_formats() {
        if let Some(small) = formats
            .iter()
            .filter(|f| f.frame_rate() >= 15 && f.resolution().width() >= 320)
            .min_by_key(|f| (f.resolution().width(), u32::MAX - f.frame_rate()))
        {
            let _ = cam.set_camera_requset(RequestedFormat::new::<RgbFormat>(
                RequestedFormatType::Exact(*small),
            ));
        }
    }
    cam.open_stream()
        .map_err(|e| format!("Couldn't start the camera: {}", e))?;
    let fmt = cam.camera_format();
    let (w, h) = (fmt.resolution().width(), fmt.resolution().height());
    let started = Instant::now();
    let gap = Duration::from_millis(1000 / FPS);
    let mut last = Instant::now() - gap;
    while !stop.load(Ordering::Relaxed) {
        let raw = cam
            .frame_raw()
            .map_err(|e| format!("The camera stopped: {}", e))?;
        if last.elapsed() < gap {
            continue;
        }
        last = Instant::now();
        let buf = match fmt.format() {
            FrameFormat::YUYV if raw.len() >= (w * h * 2) as usize => yuyv_to_i420(&raw, w, h),
            FrameFormat::NV12 if raw.len() >= (w * h * 3 / 2) as usize => nv12_to_i420(&raw, w, h),
            FrameFormat::RAWRGB if raw.len() >= (w * h * 3) as usize => rgb_to_i420(&raw, w, h),
            other => {
                let frame = nokhwa::Buffer::new(fmt.resolution(), &raw, other);
                let img = frame
                    .decode_image::<RgbFormat>()
                    .map_err(|e| format!("Couldn't read the camera picture: {}", e))?;
                rgb_to_i420(img.as_raw(), img.width(), img.height())
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
            let p = i420_to_picture(&rgb_to_i420(&px, 16, 8));
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
        assert_eq!(send_size(1920, 1080), (640, 360));
        assert_eq!(send_size(320, 181), (320, 180));
        // YUYV: two pixels share U and V
        let (y, u, v) = rgb_to_yuv(250, 20, 20);
        let yuyv: Vec<u8> = std::iter::repeat([y, u, y, v])
            .take(1280 * 720 / 2)
            .flatten()
            .collect();
        let p = i420_to_picture(&yuyv_to_i420(&yuyv, 1280, 720));
        assert_eq!((p.width, p.height), (640, 360));
        assert!(p.average()[0] > 200 && p.average()[1] < 60);
        let mut nv12 = vec![y; 64 * 32];
        nv12.extend(std::iter::repeat([u, v]).take(64 * 32 / 4).flatten());
        assert!(i420_to_picture(&nv12_to_i420(&nv12, 64, 32)).average()[0] > 200);
    }
}
