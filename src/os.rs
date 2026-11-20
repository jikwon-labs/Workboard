// The few things that differ between macOS, Windows and Linux outside the
// window itself: the home folder and handing a file to the system.

use std::path::{Path, PathBuf};

/// The person's home folder: $HOME on macOS and Linux, %USERPROFILE% on
/// Windows (which has no $HOME). "." when neither is set.
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|h| !h.is_empty()))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The folder the app keeps its own data in (data.json and the rest):
/// ~/Library/Application Support/Workboard on macOS, %APPDATA%\Workboard on
/// Windows, ~/.workboard elsewhere.
pub fn app_data_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        return home_dir().join("Library/Application Support/Workboard");
    }
    if cfg!(windows) {
        if let Some(d) = std::env::var_os("APPDATA").filter(|d| !d.is_empty()) {
            return PathBuf::from(d).join("Workboard");
        }
    }
    home_dir().join(".workboard")
}

/// A command that runs without flashing a console window on Windows (the
/// app has none, so Windows would open one for each console program).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn quiet_command(program: &str) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// A shortcut as this system writes it: the Mac's symbols on macOS
/// ("\u{2318}J"), words elsewhere ("Ctrl+J"). egui's command key is Ctrl
/// there, so the same keys work.
pub fn keys(s: &str) -> String {
    keys_for(s, cfg!(target_os = "macos"))
}

fn keys_for(s: &str, mac: bool) -> String {
    if mac {
        return s.to_string();
    }
    s.replace("\u{2318}+", "Ctrl+")
        .replace('\u{2318}', "Ctrl+")
        .replace('\u{2325}', "Alt+")
        .replace('\u{21E7}', "Shift+")
        // "Ctrl+/Shift+ click" reads better as "Ctrl/Shift click"
        .replace("+/", "/")
        .replace("+ ", " ")
}

/// Open a file or web address with whatever the system uses for it.
pub fn open_path(p: impl AsRef<std::ffi::OsStr>) {
    let p = p.as_ref();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(windows)]
    {
        // `start` treats its first quoted argument as a window title
        let _ = quiet_command("cmd")
            .args(["/C", "start", ""])
            .arg(p)
            .spawn();
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

/// Open the system's notification settings, where Workboard's banners are
/// allowed or turned off: System Settings > Notifications on macOS, Settings
/// > System > Notifications on Windows. Elsewhere nothing opens.
pub fn open_notification_settings() {
    #[cfg(target_os = "macos")]
    open_path("x-apple.systempreferences:com.apple.Notifications-Settings.extension");
    #[cfg(windows)]
    open_path("ms-settings:notifications");
}

/// Show a file selected in Finder (macOS) or File Explorer (Windows); on
/// other systems open its folder.
pub fn reveal_path(p: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
    #[cfg(windows)]
    {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(p);
        let _ = std::process::Command::new("explorer").arg(arg).spawn();
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    if let Some(dir) = p.parent() {
        open_path(dir);
    }
}

/// Wake the Mac's input method before the first key. The first key the
/// window's input context ever gets goes past the Korean IME: a "ㅇ" on its
/// own that never joins the next letters ("ㅇㅏㄴ녕"). So the context is
/// handed one letter key here, and the syllable it starts is taken back at
/// once. That leaves IME events for the next frame, which the caller drops
/// (`ImeFix::drop_wake`). False when there is no window with the keyboard
/// yet.
#[cfg(all(target_os = "macos", not(test)))]
pub fn wake_input_method() -> bool {
    use objc::runtime::{Object, BOOL};
    use objc::{class, msg_send, sel, sel_impl};
    // a key event the way the keyboard makes it: one built in AppKit alone
    // (keyEventWithType) does not reach the input method
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreateKeyboardEvent(
            source: *const std::ffi::c_void,
            key: u16,
            down: bool,
        ) -> *mut std::ffi::c_void;
        fn CFRelease(cf: *const std::ffi::c_void);
    }
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let window: *mut Object = msg_send![app, keyWindow];
        if window.is_null() {
            return false;
        }
        let view: *mut Object = msg_send![window, contentView];
        if view.is_null() {
            return false;
        }
        let context: *mut Object = msg_send![view, inputContext];
        if context.is_null() {
            return false;
        }
        // kVK_ANSI_D
        let cg = CGEventCreateKeyboardEvent(std::ptr::null(), 0x02, true);
        if cg.is_null() {
            return false;
        }
        let event: *mut Object = msg_send![class!(NSEvent), eventWithCGEvent: cg];
        if !event.is_null() {
            let _: BOOL = msg_send![context, handleEvent: event];
            let _: () = msg_send![view, unmarkText];
        }
        CFRelease(cg);
    }
    true
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn wake_input_method() -> bool {
    false
}

/// Where the pointer is on the screen, in egui points (`ppp` is egui's
/// pixels per point, `zoom` its zoom factor), or None where that isn't
/// known. While files are dragged in from Finder or File Explorer the window
/// gets no mouse moves, so this is how it knows which part they are over.
#[cfg(all(target_os = "macos", not(test)))]
pub fn pointer_on_screen(_ppp: f32, zoom: f32) -> Option<(f32, f32)> {
    #[repr(C)]
    struct CGPoint {
        x: f64,
        y: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreate(source: *const std::ffi::c_void) -> *mut std::ffi::c_void;
        fn CGEventGetLocation(event: *const std::ffi::c_void) -> CGPoint;
        fn CFRelease(cf: *const std::ffi::c_void);
    }
    // in points from the top left of the main screen, as the window's place
    unsafe {
        let e = CGEventCreate(std::ptr::null());
        if e.is_null() {
            return None;
        }
        let p = CGEventGetLocation(e);
        CFRelease(e);
        Some((p.x as f32 / zoom, p.y as f32 / zoom))
    }
}

#[cfg(all(windows, not(test)))]
pub fn pointer_on_screen(ppp: f32, _zoom: f32) -> Option<(f32, f32)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    // in pixels, as the window's place
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok()?;
    Some((p.x as f32 / ppp, p.y as f32 / ppp))
}

#[cfg(any(not(any(target_os = "macos", windows)), test))]
pub fn pointer_on_screen(_ppp: f32, _zoom: f32) -> Option<(f32, f32)> {
    None
}

/// Cmd+Q (Workboard > Quit) was chosen and the app hasn't asked about it yet.
static QUIT_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// True once after Cmd+Q: the app asks before quitting (`ask_before_quit`).
pub fn take_quit_request() -> bool {
    QUIT_ASKED.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// Point the Mac's Quit menu item (Cmd+Q) at the app, so it can ask first;
/// it quits at once otherwise, without a window close the app could stop.
/// Called every frame; done once, after winit has made the menu. A quit from
/// the Dock, logging out or an update (Sparkle) still quits at once.
#[cfg(all(target_os = "macos", not(test)))]
pub fn route_quit_menu(ctx: &egui::Context) {
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::OnceLock;

    static WAKE: OnceLock<egui::Context> = OnceLock::new();
    thread_local! {
        static DONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    if DONE.with(|d| d.get()) {
        return;
    }

    extern "C" fn quit(_this: &Object, _cmd: Sel, _sender: *mut Object) {
        QUIT_ASKED.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(ctx) = WAKE.get() {
            ctx.request_repaint();
        }
    }

    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let menu: *mut Object = msg_send![app, mainMenu];
        if menu.is_null() {
            return;
        }
        let app_item: *mut Object = msg_send![menu, itemAtIndex: 0isize];
        if app_item.is_null() {
            return;
        }
        let sub: *mut Object = msg_send![app_item, submenu];
        if sub.is_null() {
            return;
        }
        let n: isize = msg_send![sub, numberOfItems];
        let mut quit_item: *mut Object = std::ptr::null_mut();
        for i in 0..n {
            let item: *mut Object = msg_send![sub, itemAtIndex: i];
            let action: Sel = msg_send![item, action];
            if action == sel!(terminate:) {
                quit_item = item;
                break;
            }
        }
        if quit_item.is_null() {
            return;
        }
        let cls = match Class::get("WorkboardQuitTarget") {
            Some(c) => c,
            None => {
                let Some(mut decl) = ClassDecl::new("WorkboardQuitTarget", class!(NSObject)) else {
                    return;
                };
                decl.add_method(
                    sel!(askToQuit:),
                    quit as extern "C" fn(&Object, Sel, *mut Object),
                );
                decl.register()
            }
        };
        let _ = WAKE.set(ctx.clone());
        // kept for good: a menu item doesn't keep its target alive
        let target: *mut Object = msg_send![cls, new];
        let _: () = msg_send![quit_item, setTarget: target];
        let _: () = msg_send![quit_item, setAction: sel!(askToQuit:)];
    }
    DONE.with(|d| d.set(true));
}

#[cfg(any(not(target_os = "macos"), test))]
pub fn route_quit_menu(_ctx: &egui::Context) {}

/// A color picture of one emoji (`text` is one emoji, perhaps several
/// characters), about `px` pixels tall, cut to what it covers. Drawn by the
/// system's emoji font, so it looks like emoji elsewhere on the computer.
/// `None` where the system gives no color emoji (only macOS does for now).
pub fn render_emoji(text: &str, px: u32) -> Option<egui::ColorImage> {
    #[cfg(target_os = "macos")]
    {
        mac_emoji::render(text, px)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (text, px);
        None
    }
}

#[cfg(target_os = "macos")]
mod mac_emoji {
    use std::ffi::c_void;

    type Ref = *const c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFTypeDictionaryKeyCallBacks: u8;
        static kCFTypeDictionaryValueCallBacks: u8;
        fn CFStringCreateWithBytes(
            alloc: Ref,
            bytes: *const u8,
            len: isize,
            encoding: u32,
            external: u8,
        ) -> Ref;
        fn CFDictionaryCreate(
            alloc: Ref,
            keys: *const Ref,
            values: *const Ref,
            n: isize,
            key_cb: *const u8,
            value_cb: *const u8,
        ) -> Ref;
        fn CFAttributedStringCreate(alloc: Ref, s: Ref, attrs: Ref) -> Ref;
        fn CFRelease(r: Ref);
    }

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        static kCTFontAttributeName: Ref;
        fn CTFontCreateWithName(name: Ref, size: f64, matrix: *const c_void) -> Ref;
        fn CTLineCreateWithAttributedString(s: Ref) -> Ref;
        fn CTLineGetTypographicBounds(
            line: Ref,
            ascent: *mut f64,
            descent: *mut f64,
            leading: *mut f64,
        ) -> f64;
        fn CTLineDraw(line: Ref, ctx: Ref);
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGColorSpaceCreateDeviceRGB() -> Ref;
        fn CGColorSpaceRelease(cs: Ref);
        fn CGBitmapContextCreate(
            data: *mut c_void,
            w: usize,
            h: usize,
            bits: usize,
            row_bytes: usize,
            cs: Ref,
            info: u32,
        ) -> Ref;
        fn CGContextRelease(c: Ref);
        fn CGContextSetTextPosition(c: Ref, x: f64, y: f64);
    }

    fn cf_string(s: &str) -> Ref {
        const UTF8: u32 = 0x0800_0100;
        unsafe { CFStringCreateWithBytes(std::ptr::null(), s.as_ptr(), s.len() as isize, UTF8, 0) }
    }

    pub(super) fn render(text: &str, px: u32) -> Option<egui::ColorImage> {
        if text.is_empty() || px == 0 || px > 512 {
            return None;
        }
        unsafe {
            let name = cf_string("AppleColorEmoji");
            let font = CTFontCreateWithName(name, px as f64, std::ptr::null());
            CFRelease(name);
            if font.is_null() {
                return None;
            }
            let keys = [kCTFontAttributeName];
            let values = [font];
            let attrs = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            let s = cf_string(text);
            let astr = CFAttributedStringCreate(std::ptr::null(), s, attrs);
            CFRelease(s);
            CFRelease(attrs);
            CFRelease(font);
            let line = CTLineCreateWithAttributedString(astr);
            CFRelease(astr);
            if line.is_null() {
                return None;
            }
            let (mut ascent, mut descent, mut leading) = (0.0, 0.0, 0.0);
            let width = CTLineGetTypographicBounds(line, &mut ascent, &mut descent, &mut leading);
            // room around the line, then cut to what the emoji covers
            let pad = (px / 4).max(2) as usize;
            let w = width.ceil() as usize + pad * 2;
            let h = (ascent + descent).ceil() as usize + pad * 2;
            if w == 0 || h == 0 || w > 4096 || h > 4096 {
                CFRelease(line);
                return None;
            }
            let mut buf = vec![0u8; w * h * 4];
            let cs = CGColorSpaceCreateDeviceRGB();
            const PREMULTIPLIED_LAST: u32 = 1;
            let cg = CGBitmapContextCreate(
                buf.as_mut_ptr() as *mut c_void,
                w,
                h,
                8,
                w * 4,
                cs,
                PREMULTIPLIED_LAST,
            );
            CGColorSpaceRelease(cs);
            if cg.is_null() {
                CFRelease(line);
                return None;
            }
            CGContextSetTextPosition(cg, pad as f64, pad as f64 + descent);
            CTLineDraw(line, cg);
            CGContextRelease(cg);
            CFRelease(line);
            crop(&buf, w, h)
        }
    }

    /// The part of an RGBA picture (rows from the top) that is not see-through.
    fn crop(buf: &[u8], w: usize, h: usize) -> Option<egui::ColorImage> {
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for y in 0..h {
            for x in 0..w {
                if buf[(y * w + x) * 4 + 3] > 0 {
                    x0 = x0.min(x);
                    x1 = x1.max(x + 1);
                    y0 = y0.min(y);
                    y1 = y1.max(y + 1);
                }
            }
        }
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let mut out = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
        for y in y0..y1 {
            out.extend_from_slice(&buf[(y * w + x0) * 4..(y * w + x1) * 4]);
        }
        Some(egui::ColorImage::from_rgba_premultiplied(
            [x1 - x0, y1 - y0],
            &out,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_use_words_outside_the_mac() {
        assert_eq!(keys_for("\u{2318}J", true), "\u{2318}J");
        assert_eq!(keys_for("\u{2318}J", false), "Ctrl+J");
        assert_eq!(keys_for("\u{2325}T", false), "Alt+T");
        assert_eq!(
            keys_for("\u{2318}N  /  \u{2325}T  /  T", false),
            "Ctrl+N  /  Alt+T  /  T"
        );
        assert_eq!(
            keys_for("\u{2318}/\u{21E7} click", false),
            "Ctrl/Shift click"
        );
        assert_eq!(
            keys_for("pinch or \u{2318}+scroll to zoom", false),
            "pinch or Ctrl+scroll to zoom"
        );
    }

    #[test]
    fn the_data_folder_is_under_the_home_folder_or_appdata() {
        let d = app_data_dir();
        assert!(
            d.ends_with("Workboard") || d.ends_with(".workboard"),
            "{:?}",
            d
        );
        if cfg!(target_os = "macos") {
            assert!(d.starts_with(home_dir()));
        }
    }
}
