// Pictures and videos for chat attachments, through the system's own
// frameworks so every format macOS knows works (HEIC, ProRes, ...):
//   ImageIO          picture sizes and downscaled pictures
//   AVFoundation     video length and size, a still from the video, playback
//   AppKit           the file picker and the clipboard
// Pixels come back as premultiplied RGBA, ready for an egui texture.
// Everything that isn't macOS gets stubs that find nothing.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Picture or video (stored in each attachment).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MediaKind {
    #[default]
    Image,
    Video,
}

/// Premultiplied RGBA pixels.
#[derive(Clone)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Rgba {
    pub fn to_color_image(&self) -> egui::ColorImage {
        egui::ColorImage::from_rgba_premultiplied([self.w as usize, self.h as usize], &self.px)
    }
}

/// File name endings taken as pictures, and as videos (the file picker
/// offers exactly these).
pub const IMAGE_EXTS: [&str; 10] = [
    "jpg", "jpeg", "png", "gif", "heic", "heif", "webp", "tif", "tiff", "bmp",
];
pub const VIDEO_EXTS: [&str; 4] = ["mov", "mp4", "m4v", "3gp"];

/// What a file is, from its name. None for anything that isn't a picture or
/// a video AVFoundation can play.
pub fn kind_of(path: &Path) -> Option<MediaKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if IMAGE_EXTS.contains(&ext.as_str()) {
        Some(MediaKind::Image)
    } else if VIDEO_EXTS.contains(&ext.as_str()) {
        Some(MediaKind::Video)
    } else {
        None
    }
}

/// The content type sent with an upload.
pub fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("heic") => "image/heic",
        Some("heif") => "image/heif",
        Some("webp") => "image/webp",
        Some("tif" | "tiff") => "image/tiff",
        Some("bmp") => "image/bmp",
        Some("mov") => "video/quicktime",
        Some("mp4") => "video/mp4",
        Some("m4v") => "video/x-m4v",
        Some("3gp") => "video/3gpp",
        _ => "application/octet-stream",
    }
}

/// "1:05", "12:30", "1:02:03".
pub fn clock(secs: f32) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// What was on the clipboard when the person pasted.
#[derive(Default, Debug)]
pub struct Pasted {
    /// files copied in Finder
    pub files: Vec<PathBuf>,
    /// a picture (a screenshot, an image copied from a page), as PNG
    pub png: Option<Vec<u8>>,
}

#[cfg(all(target_os = "macos", not(test)))]
pub use mac::pick_zip;
#[cfg(target_os = "macos")]
pub use mac::{pasteboard, pick_files, probe, thumbnail, video_still_jpeg, Player};

/// Longer side of the still sent along with a video.
pub const STILL: u32 = 480;

#[cfg(not(target_os = "macos"))]
mod other {
    use super::*;
    pub fn probe(_: &Path, _: MediaKind) -> Result<(u32, u32, f32), String> {
        Err("Pictures and videos need macOS.".into())
    }
    pub fn thumbnail(_: &Path, _: MediaKind, _: u32) -> Option<Rgba> {
        None
    }
    pub fn video_still_jpeg(_: &Path, _: u32) -> Option<Vec<u8>> {
        None
    }
    pub fn pick_files() -> Vec<PathBuf> {
        vec![]
    }
    pub fn pick_zip(_: &str) -> Option<PathBuf> {
        None
    }
    pub fn pasteboard() -> Pasted {
        Pasted::default()
    }
    pub struct Player;
    impl Player {
        pub fn new(_: &Path, _: u32, _: u32) -> Option<Player> {
            None
        }
        pub fn frame(&mut self) -> Option<Rgba> {
            None
        }
        pub fn play(&mut self) {}
        pub fn pause(&mut self) {}
        pub fn playing(&self) -> bool {
            false
        }
        pub fn position(&self) -> f32 {
            0.0
        }
        pub fn duration(&self) -> f32 {
            0.0
        }
        pub fn seek(&mut self, _: f32) {}
        pub fn set_muted(&mut self, _: bool) {}
        pub fn muted(&self) -> bool {
            false
        }
        pub fn failed(&self) -> bool {
            false
        }
    }
}
#[cfg(not(target_os = "macos"))]
pub use other::{pasteboard, pick_files, pick_zip, probe, thumbnail, video_still_jpeg, Player};

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use objc::rc::autoreleasepool;
    use objc::runtime::{Object, BOOL, NO, YES};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::{c_void, CStr, CString};
    use std::ptr::{null, null_mut};

    type CFRef = *const c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    struct CGSize {
        w: f64,
        h: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    struct CGAffineTransform {
        a: f64,
        b: f64,
        c: f64,
        d: f64,
        tx: f64,
        ty: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    struct CMTime {
        value: i64,
        timescale: i32,
        flags: u32,
        epoch: i64,
    }

    const CMTIME_VALID: u32 = 1;
    const CMTIME_INDEFINITE: u32 = 1 << 4;

    impl CMTime {
        fn secs(s: f32) -> CMTime {
            CMTime {
                value: (s.max(0.0) as f64 * 600.0) as i64,
                timescale: 600,
                flags: CMTIME_VALID,
                epoch: 0,
            }
        }
        fn zero() -> CMTime {
            CMTime {
                value: 0,
                timescale: 1,
                flags: CMTIME_VALID,
                epoch: 0,
            }
        }
        fn to_secs(self) -> Option<f32> {
            if self.flags & CMTIME_VALID == 0
                || self.flags & CMTIME_INDEFINITE != 0
                || self.timescale == 0
            {
                return None;
            }
            Some((self.value as f64 / self.timescale as f64) as f32)
        }
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFRef);
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGImageGetWidth(img: CFRef) -> usize;
        fn CGImageGetHeight(img: CFRef) -> usize;
        fn CGColorSpaceCreateDeviceRGB() -> CFRef;
        fn CGBitmapContextCreate(
            data: *mut c_void,
            w: usize,
            h: usize,
            bits: usize,
            row: usize,
            space: CFRef,
            info: u32,
        ) -> CFRef;
        fn CGContextDrawImage(ctx: CFRef, rect: CGRect, img: CFRef);
    }
    #[link(name = "ImageIO", kind = "framework")]
    extern "C" {
        fn CGImageSourceCreateWithURL(url: CFRef, options: CFRef) -> CFRef;
        fn CGImageSourceCreateThumbnailAtIndex(src: CFRef, index: usize, options: CFRef) -> CFRef;
        fn CGImageSourceCopyPropertiesAtIndex(src: CFRef, index: usize, options: CFRef) -> CFRef;
        static kCGImageSourceCreateThumbnailFromImageAlways: CFRef;
        static kCGImageSourceCreateThumbnailWithTransform: CFRef;
        static kCGImageSourceThumbnailMaxPixelSize: CFRef;
        static kCGImagePropertyPixelWidth: CFRef;
        static kCGImagePropertyPixelHeight: CFRef;
        static kCGImagePropertyOrientation: CFRef;
        fn CGImageDestinationCreateWithData(
            data: CFRef,
            kind: CFRef,
            count: usize,
            options: CFRef,
        ) -> CFRef;
        fn CGImageDestinationAddImage(dest: CFRef, img: CFRef, props: CFRef);
        fn CGImageDestinationFinalize(dest: CFRef) -> bool;
        static kCGImageDestinationLossyCompressionQuality: CFRef;
    }
    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {
        static AVMediaTypeVideo: *mut Object;
    }
    #[link(name = "CoreMedia", kind = "framework")]
    extern "C" {}
    #[link(name = "CoreVideo", kind = "framework")]
    extern "C" {
        static kCVPixelBufferPixelFormatTypeKey: CFRef;
        static kCVPixelBufferWidthKey: CFRef;
        static kCVPixelBufferHeightKey: CFRef;
        fn CVPixelBufferLockBaseAddress(pb: CFRef, flags: u64) -> i32;
        fn CVPixelBufferUnlockBaseAddress(pb: CFRef, flags: u64) -> i32;
        fn CVPixelBufferGetBaseAddress(pb: CFRef) -> *const u8;
        fn CVPixelBufferGetBytesPerRow(pb: CFRef) -> usize;
        fn CVPixelBufferGetWidth(pb: CFRef) -> usize;
        fn CVPixelBufferGetHeight(pb: CFRef) -> usize;
        fn CVBufferRelease(pb: CFRef);
    }
    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {
        fn CACurrentMediaTime() -> f64;
    }
    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        static NSPasteboardTypePNG: *mut Object;
        static NSPasteboardTypeTIFF: *mut Object;
        static NSPasteboardURLReadingFileURLsOnlyKey: *mut Object;
    }
    #[link(name = "UniformTypeIdentifiers", kind = "framework")]
    extern "C" {}

    const BGRA: u32 = u32::from_be_bytes(*b"BGRA");

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

    unsafe fn file_url(path: &Path) -> *mut Object {
        msg_send![class!(NSURL), fileURLWithPath: ns(&path.to_string_lossy())]
    }

    unsafe fn number(n: i64) -> *mut Object {
        msg_send![class!(NSNumber), numberWithLongLong: n]
    }

    unsafe fn yes() -> *mut Object {
        msg_send![class!(NSNumber), numberWithBool: YES]
    }

    unsafe fn dict(pairs: &[(*const c_void, *mut Object)]) -> *mut Object {
        let keys: Vec<*const c_void> = pairs.iter().map(|p| p.0).collect();
        let vals: Vec<*mut Object> = pairs.iter().map(|p| p.1).collect();
        msg_send![class!(NSDictionary), dictionaryWithObjects: vals.as_ptr() forKeys: keys.as_ptr() count: pairs.len()]
    }

    unsafe fn array(items: &[*mut Object]) -> *mut Object {
        msg_send![class!(NSArray), arrayWithObjects: items.as_ptr() count: items.len()]
    }

    /// Draw a CGImage into premultiplied RGBA memory (top row first).
    unsafe fn pixels(img: CFRef) -> Option<Rgba> {
        let (w, h) = (CGImageGetWidth(img), CGImageGetHeight(img));
        if w == 0 || h == 0 {
            return None;
        }
        let mut px = vec![0u8; w * h * 4];
        let space = CGColorSpaceCreateDeviceRGB();
        // kCGImageAlphaPremultipliedLast, default (big-endian) byte order: R G B A
        let ctx = CGBitmapContextCreate(px.as_mut_ptr() as *mut c_void, w, h, 8, w * 4, space, 1);
        CFRelease(space);
        if ctx.is_null() {
            return None;
        }
        CGContextDrawImage(
            ctx,
            CGRect {
                origin: CGPoint::default(),
                size: CGSize {
                    w: w as f64,
                    h: h as f64,
                },
            },
            img,
        );
        CFRelease(ctx);
        Some(Rgba {
            w: w as u32,
            h: h as u32,
            px,
        })
    }

    unsafe fn image_source(path: &Path) -> Option<CFRef> {
        let src = CGImageSourceCreateWithURL(file_url(path) as CFRef, null());
        (!src.is_null()).then_some(src)
    }

    unsafe fn video_track(asset: *mut Object) -> *mut Object {
        let tracks: *mut Object = msg_send![asset, tracksWithMediaType: AVMediaTypeVideo];
        if tracks.is_null() {
            return null_mut();
        }
        msg_send![tracks, firstObject]
    }

    /// Width, height (as shown, after rotation) and length in seconds.
    pub fn probe(path: &Path, kind: MediaKind) -> Result<(u32, u32, f32), String> {
        autoreleasepool(|| unsafe {
            match kind {
                MediaKind::Image => {
                    let src = image_source(path).ok_or("That picture can't be opened.")?;
                    let props = CGImageSourceCopyPropertiesAtIndex(src, 0, null());
                    CFRelease(src);
                    if props.is_null() {
                        return Err("That picture can't be opened.".into());
                    }
                    let d = props as *mut Object;
                    let get = |k: CFRef| -> i64 {
                        let n: *mut Object = msg_send![d, objectForKey: k];
                        if n.is_null() {
                            0
                        } else {
                            msg_send![n, longLongValue]
                        }
                    };
                    let (w, h, orient) = (
                        get(kCGImagePropertyPixelWidth),
                        get(kCGImagePropertyPixelHeight),
                        get(kCGImagePropertyOrientation),
                    );
                    CFRelease(props);
                    if w <= 0 || h <= 0 {
                        return Err("That picture can't be opened.".into());
                    }
                    // EXIF orientations 5-8 are turned a quarter
                    Ok(if (5..=8).contains(&orient) {
                        (h as u32, w as u32, 0.0)
                    } else {
                        (w as u32, h as u32, 0.0)
                    })
                }
                MediaKind::Video => {
                    let asset: *mut Object = msg_send![class!(AVURLAsset), URLAssetWithURL: file_url(path) options: null_mut::<Object>()];
                    if asset.is_null() {
                        return Err("That video can't be opened.".into());
                    }
                    let playable: BOOL = msg_send![asset, isPlayable];
                    let track = video_track(asset);
                    if playable == NO || track.is_null() {
                        return Err("That video can't be played on this Mac.".into());
                    }
                    let dur: CMTime = msg_send![asset, duration];
                    let size: CGSize = msg_send![track, naturalSize];
                    let t: CGAffineTransform = msg_send![track, preferredTransform];
                    let turned = t.b.abs() > 0.5 && t.c.abs() > 0.5;
                    let (w, h) = (size.w.abs().round() as u32, size.h.abs().round() as u32);
                    Ok(if turned {
                        (h, w, dur.to_secs().unwrap_or(0.0))
                    } else {
                        (w, h, dur.to_secs().unwrap_or(0.0))
                    })
                }
            }
        })
    }

    /// A picture no larger than `max` on its longer side: the picture itself,
    /// or a still from early in the video.
    pub fn thumbnail(path: &Path, kind: MediaKind, max: u32) -> Option<Rgba> {
        autoreleasepool(|| unsafe {
            match kind {
                MediaKind::Image => {
                    let src = image_source(path)?;
                    let opts = dict(&[
                        (kCGImageSourceCreateThumbnailFromImageAlways, yes()),
                        (kCGImageSourceCreateThumbnailWithTransform, yes()),
                        (kCGImageSourceThumbnailMaxPixelSize, number(max as i64)),
                    ]);
                    let img = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFRef);
                    CFRelease(src);
                    if img.is_null() {
                        return None;
                    }
                    let out = pixels(img);
                    CFRelease(img);
                    out
                }
                MediaKind::Video => {
                    let asset: *mut Object = msg_send![class!(AVURLAsset), URLAssetWithURL: file_url(path) options: null_mut::<Object>()];
                    if asset.is_null() {
                        return None;
                    }
                    let gen: *mut Object = msg_send![class!(AVAssetImageGenerator), assetImageGeneratorWithAsset: asset];
                    let _: () = msg_send![gen, setAppliesPreferredTrackTransform: YES];
                    let _: () =
                        msg_send![gen, setMaximumSize: CGSize { w: max as f64, h: max as f64 }];
                    let dur: CMTime = msg_send![asset, duration];
                    // a little way in: the very first frame is often black
                    let at = (dur.to_secs().unwrap_or(0.0) / 10.0).min(1.0);
                    let mut err: *mut Object = null_mut();
                    let img: CFRef = msg_send![gen, copyCGImageAtTime: CMTime::secs(at) actualTime: null_mut::<CMTime>() error: &mut err];
                    if img.is_null() {
                        return None;
                    }
                    let out = pixels(img);
                    CFRelease(img);
                    out
                }
            }
        })
    }

    /// A still from early in a video as a JPEG no larger than `max` on its
    /// longer side: sent with the video so others can show it without
    /// fetching the whole file.
    pub fn video_still_jpeg(path: &Path, max: u32) -> Option<Vec<u8>> {
        autoreleasepool(|| unsafe {
            let asset: *mut Object = msg_send![class!(AVURLAsset), URLAssetWithURL: file_url(path) options: null_mut::<Object>()];
            if asset.is_null() {
                return None;
            }
            let gen: *mut Object =
                msg_send![class!(AVAssetImageGenerator), assetImageGeneratorWithAsset: asset];
            let _: () = msg_send![gen, setAppliesPreferredTrackTransform: YES];
            let _: () = msg_send![gen, setMaximumSize: CGSize { w: max as f64, h: max as f64 }];
            let dur: CMTime = msg_send![asset, duration];
            // a little way in: the very first frame is often black
            let at = (dur.to_secs().unwrap_or(0.0) / 10.0).min(1.0);
            let mut err: *mut Object = null_mut();
            let img: CFRef = msg_send![gen, copyCGImageAtTime: CMTime::secs(at) actualTime: null_mut::<CMTime>() error: &mut err];
            if img.is_null() {
                return None;
            }
            let data: *mut Object = msg_send![class!(NSMutableData), data];
            let dest = CGImageDestinationCreateWithData(
                data as CFRef,
                ns("public.jpeg") as CFRef,
                1,
                null(),
            );
            if dest.is_null() {
                CFRelease(img);
                return None;
            }
            let q: *mut Object = msg_send![class!(NSNumber), numberWithDouble: 0.75f64];
            let props = dict(&[(kCGImageDestinationLossyCompressionQuality, q)]);
            CGImageDestinationAddImage(dest, img, props as CFRef);
            let ok = CGImageDestinationFinalize(dest);
            CFRelease(dest);
            CFRelease(img);
            if !ok {
                return None;
            }
            let len: usize = msg_send![data, length];
            let bytes: *const u8 = msg_send![data, bytes];
            if bytes.is_null() || len == 0 {
                return None;
            }
            Some(std::slice::from_raw_parts(bytes, len).to_vec())
        })
    }

    #[cfg(not(test))]
    /// The system file picker for one .zip file (a Slack export). Blocks
    /// until the person chooses or cancels. Main thread only.
    pub fn pick_zip(message: &str) -> Option<PathBuf> {
        autoreleasepool(|| unsafe {
            let panel: *mut Object = msg_send![class!(NSOpenPanel), openPanel];
            let _: () = msg_send![panel, setAllowsMultipleSelection: NO];
            let _: () = msg_send![panel, setCanChooseDirectories: NO];
            let _: () = msg_send![panel, setCanChooseFiles: YES];
            let _: () = msg_send![panel, setMessage: ns(message)];
            let t: *mut Object = msg_send![class!(UTType), typeWithFilenameExtension: ns("zip")];
            if !t.is_null() {
                let _: () = msg_send![panel, setAllowedContentTypes: array(&[t])];
            }
            let r: isize = msg_send![panel, runModal];
            if r != 1 {
                return None;
            }
            let urls: *mut Object = msg_send![panel, URLs];
            let n: usize = msg_send![urls, count];
            if n == 0 {
                return None;
            }
            let u: *mut Object = msg_send![urls, objectAtIndex: 0usize];
            from_ns(msg_send![u, path]).map(PathBuf::from)
        })
    }

    /// The system file picker for pictures and videos (several at once).
    /// Blocks until the person chooses or cancels. Main thread only.
    pub fn pick_files() -> Vec<PathBuf> {
        autoreleasepool(|| unsafe {
            let panel: *mut Object = msg_send![class!(NSOpenPanel), openPanel];
            let _: () = msg_send![panel, setAllowsMultipleSelection: YES];
            let _: () = msg_send![panel, setCanChooseDirectories: NO];
            let _: () = msg_send![panel, setCanChooseFiles: YES];
            let _: () = msg_send![panel, setMessage: ns(crate::i18n::tr("Choose pictures or videos to send"))];
            // only the kinds kind_of takes, so nothing chosen is refused afterwards
            let types: Vec<*mut Object> = IMAGE_EXTS
                .iter()
                .chain(VIDEO_EXTS.iter())
                .map(|e| -> *mut Object {
                    msg_send![class!(UTType), typeWithFilenameExtension: ns(e)]
                })
                .filter(|t| !t.is_null())
                .collect();
            if !types.is_empty() {
                let _: () = msg_send![panel, setAllowedContentTypes: array(&types)];
            }
            let r: isize = msg_send![panel, runModal];
            if r != 1 {
                return vec![];
            }
            let urls: *mut Object = msg_send![panel, URLs];
            let n: usize = msg_send![urls, count];
            (0..n)
                .filter_map(|i| {
                    let u: *mut Object = msg_send![urls, objectAtIndex: i];
                    from_ns(msg_send![u, path]).map(PathBuf::from)
                })
                .collect()
        })
    }

    /// Files or a picture on the clipboard.
    pub fn pasteboard() -> Pasted {
        autoreleasepool(|| unsafe {
            let mut out = Pasted::default();
            let pb: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
            let classes = array(&[class!(NSURL) as *const _ as *mut Object]);
            let opts = dict(&[(NSPasteboardURLReadingFileURLsOnlyKey as CFRef, yes())]);
            let urls: *mut Object = msg_send![pb, readObjectsForClasses: classes options: opts];
            if !urls.is_null() {
                let n: usize = msg_send![urls, count];
                for i in 0..n {
                    let u: *mut Object = msg_send![urls, objectAtIndex: i];
                    if let Some(p) = from_ns(msg_send![u, path]) {
                        out.files.push(PathBuf::from(p));
                    }
                }
            }
            if !out.files.is_empty() {
                return out;
            }
            let mut data: *mut Object = msg_send![pb, dataForType: NSPasteboardTypePNG];
            if data.is_null() {
                let tiff: *mut Object = msg_send![pb, dataForType: NSPasteboardTypeTIFF];
                if !tiff.is_null() {
                    let rep: *mut Object =
                        msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
                    if !rep.is_null() {
                        // NSBitmapImageFileTypePNG
                        data =
                            msg_send![rep, representationUsingType: 4usize properties: dict(&[])];
                    }
                }
            }
            if !data.is_null() {
                let len: usize = msg_send![data, length];
                let bytes: *const u8 = msg_send![data, bytes];
                if len > 0 && !bytes.is_null() {
                    out.png = Some(std::slice::from_raw_parts(bytes, len).to_vec());
                }
            }
            out
        })
    }

    /// A video playing inside the app: AVPlayer makes the sound, and frames
    /// are copied out for an egui texture. Main thread.
    pub struct Player {
        player: *mut Object,
        item: *mut Object,
        output: *mut Object,
        muted: bool,
    }

    impl Player {
        /// `w`×`h` is the size frames are scaled to (keep the video's shape).
        /// None when there's nothing AVFoundation can play in the file.
        pub fn new(path: &Path, w: u32, h: u32) -> Option<Player> {
            autoreleasepool(|| unsafe {
                let asset: *mut Object = msg_send![class!(AVURLAsset), URLAssetWithURL: file_url(path) options: null_mut::<Object>()];
                if asset.is_null() {
                    return None;
                }
                let playable: BOOL = msg_send![asset, isPlayable];
                if playable == NO {
                    return None;
                }
                let item: *mut Object = msg_send![class!(AVPlayerItem), playerItemWithAsset: asset];
                if item.is_null() {
                    return None;
                }
                let _: *mut Object = msg_send![item, retain];
                let attrs = dict(&[
                    (kCVPixelBufferPixelFormatTypeKey, number(BGRA as i64)),
                    (kCVPixelBufferWidthKey, number(w.max(2) as i64)),
                    (kCVPixelBufferHeightKey, number(h.max(2) as i64)),
                ]);
                let output: *mut Object = msg_send![class!(AVPlayerItemVideoOutput), alloc];
                let output: *mut Object = msg_send![output, initWithPixelBufferAttributes: attrs];
                let _: () = msg_send![item, addOutput: output];
                let player: *mut Object = msg_send![class!(AVPlayer), alloc];
                let player: *mut Object = msg_send![player, initWithPlayerItem: item];
                Some(Player {
                    player,
                    item,
                    output,
                    muted: false,
                })
            })
        }

        /// The newest frame, when one is due that hasn't been handed out yet.
        pub fn frame(&mut self) -> Option<Rgba> {
            autoreleasepool(|| unsafe {
                let t: CMTime = msg_send![self.output, itemTimeForHostTime: CACurrentMediaTime()];
                let due: BOOL = msg_send![self.output, hasNewPixelBufferForItemTime: t];
                if due == NO {
                    return None;
                }
                let pb: CFRef = msg_send![self.output, copyPixelBufferForItemTime: t itemTimeForDisplay: null_mut::<CMTime>()];
                if pb.is_null() {
                    return None;
                }
                CVPixelBufferLockBaseAddress(pb, 1);
                let (w, h, row) = (
                    CVPixelBufferGetWidth(pb),
                    CVPixelBufferGetHeight(pb),
                    CVPixelBufferGetBytesPerRow(pb),
                );
                let base = CVPixelBufferGetBaseAddress(pb);
                let mut px = vec![0u8; w * h * 4];
                if !base.is_null() {
                    for y in 0..h {
                        let src = std::slice::from_raw_parts(base.add(y * row), w * 4);
                        let dst = &mut px[y * w * 4..(y + 1) * w * 4];
                        for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                            d[0] = s[2];
                            d[1] = s[1];
                            d[2] = s[0];
                            d[3] = 255;
                        }
                    }
                }
                CVPixelBufferUnlockBaseAddress(pb, 1);
                CVBufferRelease(pb);
                Some(Rgba {
                    w: w as u32,
                    h: h as u32,
                    px,
                })
            })
        }

        pub fn play(&mut self) {
            unsafe {
                // at the end, start over
                if self.duration() > 0.0 && self.position() >= self.duration() - 0.05 {
                    self.seek(0.0);
                }
                let _: () = msg_send![self.player, play];
            }
        }

        pub fn pause(&mut self) {
            unsafe {
                let _: () = msg_send![self.player, pause];
            }
        }

        pub fn playing(&self) -> bool {
            let rate: f32 = unsafe { msg_send![self.player, rate] };
            rate != 0.0
        }

        pub fn position(&self) -> f32 {
            let t: CMTime = unsafe { msg_send![self.player, currentTime] };
            t.to_secs().unwrap_or(0.0)
        }

        /// 0 until the video has loaded.
        pub fn duration(&self) -> f32 {
            let t: CMTime = unsafe { msg_send![self.item, duration] };
            t.to_secs().unwrap_or(0.0)
        }

        pub fn seek(&mut self, secs: f32) {
            unsafe {
                let _: () = msg_send![self.player, seekToTime: CMTime::secs(secs) toleranceBefore: CMTime::zero() toleranceAfter: CMTime::zero()];
            }
        }

        pub fn set_muted(&mut self, muted: bool) {
            self.muted = muted;
            unsafe {
                let _: () = msg_send![self.player, setMuted: if muted { YES } else { NO }];
            }
        }

        pub fn muted(&self) -> bool {
            self.muted
        }

        /// The video turned out not to play (AVPlayerItemStatusFailed).
        pub fn failed(&self) -> bool {
            let status: isize = unsafe { msg_send![self.item, status] };
            status == 2
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            unsafe {
                let _: () = msg_send![self.player, pause];
                let _: () =
                    msg_send![self.player, replaceCurrentItemWithPlayerItem: null_mut::<Object>()];
                let _: () = msg_send![self.player, release];
                let _: () = msg_send![self.output, release];
                let _: () = msg_send![self.item, release];
            }
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wb-media-{}-{:x}", tag, wb_sync::random_u64()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A 400×300 picture: red on the left half, blue on the right.
    fn picture(dir: &Path) -> PathBuf {
        let mut img = image::RgbaImage::new(400, 300);
        for (x, _, p) in img.enumerate_pixels_mut() {
            *p = if x < 200 {
                image::Rgba([220, 20, 20, 255])
            } else {
                image::Rgba([20, 20, 220, 255])
            };
        }
        let p = dir.join("pic.png");
        img.save(&p).unwrap();
        p
    }

    #[test]
    fn pictures_are_measured_and_shrunk() {
        let dir = temp("pic");
        let png = picture(&dir);
        assert_eq!(kind_of(&png), Some(MediaKind::Image));
        assert_eq!(probe(&png, MediaKind::Image).unwrap(), (400, 300, 0.0));
        let t = thumbnail(&png, MediaKind::Image, 100).unwrap();
        assert_eq!((t.w, t.h), (100, 75));
        assert_eq!(t.px.len(), 100 * 75 * 4);
        // left is red, right is blue, the top row first
        let at =
            |x: u32, y: u32| &t.px[((y * t.w + x) * 4) as usize..((y * t.w + x) * 4 + 4) as usize];
        assert!(
            at(10, 10)[0] > 180 && at(10, 10)[2] < 60,
            "{:?}",
            at(10, 10)
        );
        assert!(
            at(90, 60)[2] > 180 && at(90, 60)[0] < 60,
            "{:?}",
            at(90, 60)
        );
        // a JPEG made by the system tools reads the same way
        let jpg = dir.join("pic.jpg");
        let ok = std::process::Command::new("sips")
            .args(["-s", "format", "jpeg"])
            .arg(&png)
            .arg("--out")
            .arg(&jpg)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok);
        assert_eq!(probe(&jpg, MediaKind::Image).unwrap(), (400, 300, 0.0));
        // not a picture at all
        let bad = dir.join("bad.png");
        std::fs::write(&bad, b"not a picture").unwrap();
        assert!(probe(&bad, MediaKind::Image).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn videos_are_measured_with_a_still() {
        let Ok(ffmpeg) = which("ffmpeg") else {
            eprintln!("skipped: ffmpeg isn't installed (brew install ffmpeg)");
            return;
        };
        let dir = temp("vid");
        let mp4 = dir.join("clip.mp4");
        // 2 seconds of a test pattern at 320x240, H.264
        let ok = std::process::Command::new(ffmpeg)
            .args([
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=320x240:rate=30",
                "-pix_fmt",
                "yuv420p",
                "-c:v",
                "libx264",
            ])
            .arg(&mp4)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "ffmpeg made the clip");
        assert_eq!(kind_of(&mp4), Some(MediaKind::Video));
        let (w, h, secs) = probe(&mp4, MediaKind::Video).unwrap();
        assert_eq!((w, h), (320, 240));
        assert!((secs - 2.0).abs() < 0.1, "{}", secs);
        let t = thumbnail(&mp4, MediaKind::Video, 160).unwrap();
        assert_eq!((t.w, t.h), (160, 120));
        assert!(
            t.px.chunks(4)
                .any(|p| p[0] > 100 || p[1] > 100 || p[2] > 100),
            "the still isn't black"
        );
        // playback is checked in tests/player.rs: AVPlayer needs the main thread
        // something AVFoundation can't read
        let bad = dir.join("bad.mp4");
        std::fs::write(&bad, b"not a video").unwrap();
        assert!(probe(&bad, MediaKind::Video).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn which(name: &str) -> Result<PathBuf, ()> {
        ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
            .iter()
            .map(|d| Path::new(d).join(name))
            .find(|p| p.exists())
            .ok_or(())
    }

    #[test]
    fn kinds_types_and_clock() {
        assert_eq!(
            kind_of(Path::new("a/IMG_0001.HEIC")),
            Some(MediaKind::Image)
        );
        assert_eq!(kind_of(Path::new("screen.mov")), Some(MediaKind::Video));
        assert_eq!(kind_of(Path::new("notes.pdf")), None);
        assert_eq!(kind_of(Path::new("noext")), None);
        assert_eq!(content_type(Path::new("x.JPG")), "image/jpeg");
        assert_eq!(content_type(Path::new("x.mov")), "video/quicktime");
        assert_eq!(clock(65.4), "1:05");
        assert_eq!(clock(3723.0), "1:02:03");
    }
}
