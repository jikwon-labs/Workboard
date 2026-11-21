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
/// Windows, ~/.workboard elsewhere. On iPad $HOME is the app's own container,
/// where only Library and Documents can be written, so it goes in Library too.
pub fn app_data_dir() -> PathBuf {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
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
        .replace('\u{2303}', "Ctrl+")
        .replace('\u{2325}', "Alt+")
        .replace('\u{21E7}', "Shift+")
        // "Ctrl+/Shift+ click" reads better as "Ctrl/Shift click"
        .replace("+/", "/")
        .replace("+ ", " ")
}

/// The Redo shortcut as this system shows it: \u{2318}\u{21E7}Z on the
/// Mac, Ctrl+Y elsewhere (where Ctrl+Shift+Z works too).
pub fn redo_keys() -> String {
    if cfg!(target_os = "macos") {
        keys("\u{2318}\u{21E7}Z")
    } else {
        "Ctrl+Y".to_string()
    }
}

/// Takes this system's Redo shortcut from the input: Cmd+Shift+Z, and on
/// Windows and Linux Ctrl+Y as well (on the Mac Cmd+Y is not Redo).
pub fn take_redo(i: &mut egui::InputState) -> bool {
    i.consume_key(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    ) || (!cfg!(target_os = "macos") && i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y))
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

/// End the syllable the Mac's input method has open, without typing it:
/// the box it was typed in already shows it (see `ImeFix::ends_syllable`).
/// The window's view drops its marked text first, so the input method
/// finishing the syllable on the way out types nothing either.
#[cfg(all(target_os = "macos", not(test)))]
pub fn end_composition() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let window: *mut Object = msg_send![app, keyWindow];
        if window.is_null() {
            return;
        }
        let view: *mut Object = msg_send![window, contentView];
        if view.is_null() {
            return;
        }
        // winit's unmarkText needs the context (it would panic without one)
        let context: *mut Object = msg_send![view, inputContext];
        if context.is_null() {
            return;
        }
        let _: () = msg_send![view, unmarkText];
    }
}

/// Mac only for now: Windows' input method hasn't been tried with this.
#[cfg(any(not(target_os = "macos"), test))]
pub fn end_composition() {}

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

/// Asks "Quit Workboard?" in the system's own alert: on macOS a sheet that
/// slides down from the window, on Windows a message box over it. The app
/// keeps drawing meanwhile; true arrives on Quit, false on Cancel.
#[cfg(any(target_os = "macos", windows))]
pub fn ask_quit(frame: &eframe::Frame, ctx: &egui::Context) -> std::sync::mpsc::Receiver<bool> {
    use crate::i18n::tr;
    let (tx, rx) = std::sync::mpsc::channel();
    let quit = tr("Quit").to_string();
    let answer = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(tr("Quit Workboard?"))
        .set_description(tr(
            "You won't get new messages or notifications once you quit.",
        ))
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            quit.clone(),
            tr("Cancel").to_string(),
        ))
        .set_parent(frame)
        .show();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let ok = matches!(block_on(answer), rfd::MessageDialogResult::Custom(b) if b == quit);
        let _ = tx.send(ok);
        ctx.request_repaint();
    });
    rx
}

/// Elsewhere there is no alert, and the app quits.
#[cfg(not(any(target_os = "macos", windows)))]
pub fn ask_quit(_: &eframe::Frame, _: &egui::Context) -> std::sync::mpsc::Receiver<bool> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = tx.send(true);
    rx
}

/// Waits on this thread for a future to finish.
#[cfg(any(target_os = "macos", windows))]
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::sync::Arc::new(Unpark(std::thread::current())).into();
    let mut cx = std::task::Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        if let std::task::Poll::Ready(v) = fut.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::park();
    }
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

/// iPad: how many egui points at the bottom of the window the on-screen
/// keyboard (or a hardware keyboard's shortcut bar) covers, so the app lays
/// itself out above it. 0 elsewhere, and for a floating keyboard.
pub fn keyboard_cover(ctx: &egui::Context, native_ppp: f32) -> f32 {
    #[cfg(target_os = "ios")]
    {
        ios::follow_rotation();
        ios::keyboard_cover(ctx) * native_ppp / ctx.pixels_per_point()
    }
    #[cfg(all(not(target_os = "ios"), test))]
    {
        let _ = (ctx, native_ppp);
        FAKE_KEYBOARD.with(|k| k.get())
    }
    #[cfg(all(not(target_os = "ios"), not(test)))]
    {
        let _ = (ctx, native_ppp);
        0.0
    }
}

#[cfg(test)]
thread_local! {
    static FAKE_KEYBOARD: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
}

/// Tests: an iPad keyboard covering this many points (0: none).
#[cfg(test)]
pub fn fake_keyboard(cover: f32) {
    FAKE_KEYBOARD.with(|k| k.set(cover));
}

/// iPad: the font files of the system font and of Apple SD Gothic Neo
/// (Korean), as iOS keeps them somewhere else than macOS. Empty elsewhere.
pub fn ios_font_files() -> Vec<String> {
    #[cfg(target_os = "ios")]
    {
        ios::font_files()
    }
    #[cfg(not(target_os = "ios"))]
    {
        Vec::new()
    }
}

/// iPad: iOS 27 ends an app at start unless it shows its window in a
/// UIScene, which winit 0.30 doesn't do. Info.plist names a scene delegate
/// (`WBSceneDelegate`, declared here before the event loop starts); the
/// window winit made is put in the scene once both exist. Does nothing
/// elsewhere.
pub fn adopt_scenes() {
    #[cfg(target_os = "ios")]
    ios::adopt_scenes();
}

/// The app's window, for `adopt_scenes` (iPad only).
pub fn attach_window(cc: &eframe::CreationContext<'_>) {
    #[cfg(target_os = "ios")]
    ios::attach_window(cc);
    #[cfg(not(target_os = "ios"))]
    let _ = cc;
}

// objc 0.2's macros check a cfg this crate doesn't declare
#[cfg(target_os = "ios")]
#[allow(unexpected_cfgs)]
mod ios {
    use block::ConcreteBlock;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Once, OnceLock};

    use objc::declare::ClassDecl;
    use objc::runtime::{Protocol, Sel};
    use std::sync::atomic::AtomicPtr;

    /// The window winit made and the scene iOS made, as they come.
    static WINDOW: AtomicPtr<Object> = AtomicPtr::new(null_mut());
    static SCENE: AtomicPtr<Object> = AtomicPtr::new(null_mut());
    static DELEGATE: AtomicPtr<Object> = AtomicPtr::new(null_mut());

    #[link(name = "CoreText", kind = "framework")]
    extern "C" {
        static kCTFontURLAttribute: *const std::ffi::c_void;
        fn CTFontCopyAttribute(
            font: *const std::ffi::c_void,
            attribute: *const std::ffi::c_void,
        ) -> *mut Object;
    }

    /// Where the font a UIFont comes from is kept (UIFont is a CTFont).
    unsafe fn font_file(font: *mut Object) -> Option<String> {
        if font.is_null() {
            return None;
        }
        let url = CTFontCopyAttribute(font as *const _, kCTFontURLAttribute);
        if url.is_null() {
            return None;
        }
        let path: *mut Object = msg_send![url, path];
        let utf8: *const std::os::raw::c_char = if path.is_null() {
            std::ptr::null()
        } else {
            msg_send![path, UTF8String]
        };
        let out = (!utf8.is_null()).then(|| {
            std::ffi::CStr::from_ptr(utf8)
                .to_string_lossy()
                .into_owned()
        });
        let _: () = msg_send![url, release];
        out
    }

    pub fn font_files() -> Vec<String> {
        // SAFETY: UIFont class methods, returning autoreleased fonts
        unsafe {
            let system: *mut Object = msg_send![class!(UIFont), systemFontOfSize: 14.0f64];
            let name = objc_string("AppleSDGothicNeo-Regular");
            let korean: *mut Object = msg_send![class!(UIFont), fontWithName: name size: 14.0f64];
            [font_file(system), font_file(korean)]
                .into_iter()
                .flatten()
                .collect()
        }
    }

    unsafe fn objc_string(s: &str) -> *mut Object {
        let c = std::ffi::CString::new(s).unwrap();
        msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()]
    }

    pub fn adopt_scenes() {
        let Some(mut decl) = ClassDecl::new("WBSceneDelegate", class!(UIResponder)) else {
            return;
        };
        decl.add_ivar::<*mut Object>("wbWindow");
        extern "C" fn will_connect(
            this: &mut Object,
            _: Sel,
            scene: *mut Object,
            _session: *mut Object,
            _options: *mut Object,
        ) {
            SCENE.store(scene, Ordering::SeqCst);
            DELEGATE.store(this as *mut Object, Ordering::SeqCst);
            put_in_scene();
        }
        extern "C" fn window(this: &Object, _: Sel) -> *mut Object {
            // SAFETY: declared above
            unsafe { *this.get_ivar::<*mut Object>("wbWindow") }
        }
        extern "C" fn set_window(this: &mut Object, _: Sel, w: *mut Object) {
            // SAFETY: declared above
            unsafe { this.set_ivar("wbWindow", w) };
        }
        // SAFETY: the methods match the selectors' signatures
        unsafe {
            decl.add_method(
                sel!(scene:willConnectToSession:options:),
                will_connect
                    as extern "C" fn(&mut Object, Sel, *mut Object, *mut Object, *mut Object),
            );
            decl.add_method(
                sel!(window),
                window as extern "C" fn(&Object, Sel) -> *mut Object,
            );
            decl.add_method(
                sel!(setWindow:),
                set_window as extern "C" fn(&mut Object, Sel, *mut Object),
            );
        }
        if let Some(p) = Protocol::get("UIWindowSceneDelegate") {
            decl.add_protocol(p);
        }
        decl.register();
    }

    pub fn attach_window(cc: &eframe::CreationContext<'_>) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Ok(handle) = cc.window_handle() else {
            return;
        };
        let RawWindowHandle::UiKit(h) = handle.as_raw() else {
            return;
        };
        // SAFETY: the handle's view is winit's UIView, inside its UIWindow,
        // both alive for as long as the app runs
        let window: *mut Object = unsafe { msg_send![h.ui_view.as_ptr() as *mut Object, window] };
        WINDOW.store(window, Ordering::SeqCst);
        put_in_scene();
    }

    /// Once there is a window and a scene: show the window in the scene.
    fn put_in_scene() {
        let window = WINDOW.load(Ordering::SeqCst);
        let scene = SCENE.load(Ordering::SeqCst);
        if window.is_null() || scene.is_null() {
            return;
        }
        // SAFETY: both are live UIKit objects, used on the main thread; the
        // delegate's ivar was declared in adopt_scenes
        unsafe {
            let delegate = DELEGATE.load(Ordering::SeqCst);
            if !delegate.is_null() {
                (*delegate).set_ivar("wbWindow", window);
            }
            let _: () = msg_send![window, setWindowScene: scene];
            let _: () = msg_send![window, makeKeyAndVisible];
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    #[link(name = "UIKit", kind = "framework")]
    extern "C" {
        static UIKeyboardWillChangeFrameNotification: *mut Object;
        static UIKeyboardWillHideNotification: *mut Object;
        static UIKeyboardFrameEndUserInfoKey: *mut Object;
    }

    /// The covered height in UIKit points, as f32 bits.
    static COVER: AtomicU32 = AtomicU32::new(0);
    static CTX: OnceLock<egui::Context> = OnceLock::new();
    static WATCH: Once = Once::new();

    /// The scene's orientation last frame (UIInterfaceOrientation; 0 unknown).
    static ORIENTATION: AtomicU32 = AtomicU32::new(0);
    /// When to bring the keyboard back after a turn.
    static REFOCUS_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

    /// In a scene the keyboard keeps the shape it came up in when the iPad
    /// turns, over the app turned the other way. So it goes down at the
    /// turn and comes back once the turn is over, the new way round.
    pub fn follow_rotation() {
        let window = WINDOW.load(Ordering::SeqCst);
        let scene = SCENE.load(Ordering::SeqCst);
        if window.is_null() || scene.is_null() {
            return;
        }
        // SAFETY: live UIKit objects, on the main thread (egui's input hook)
        unsafe {
            let root: *mut Object = msg_send![window, rootViewController];
            if root.is_null() {
                return;
            }
            let view: *mut Object = msg_send![root, view];
            let mut at = REFOCUS_AT.lock().unwrap();
            if at.is_some_and(|t| std::time::Instant::now() >= t) {
                *at = None;
                let _: bool = msg_send![view, becomeFirstResponder];
            }
            let now: i64 = msg_send![scene, interfaceOrientation];
            let before = ORIENTATION.swap(now as u32, Ordering::Relaxed);
            if before == 0 || before == now as u32 {
                return;
            }
            let first: bool = msg_send![view, isFirstResponder];
            if first || at.is_some() {
                let _: bool = msg_send![view, resignFirstResponder];
                let wait = std::time::Duration::from_millis(500);
                *at = Some(std::time::Instant::now() + wait);
                if let Some(ctx) = CTX.get() {
                    ctx.request_repaint_after(wait);
                }
            }
        }
    }

    pub fn keyboard_cover(ctx: &egui::Context) -> f32 {
        WATCH.call_once(|| {
            let _ = CTX.set(ctx.clone());
            // SAFETY: called on the main thread (egui's input hook); the
            // observers and their blocks stay for as long as the app runs
            unsafe { watch() }
        });
        f32::from_bits(COVER.load(Ordering::Relaxed))
    }

    fn set(cover: f64) {
        COVER.store((cover as f32).to_bits(), Ordering::Relaxed);
        if let Some(ctx) = CTX.get() {
            ctx.request_repaint();
        }
    }

    unsafe fn watch() {
        let center: *mut Object = msg_send![class!(NSNotificationCenter), defaultCenter];
        let queue: *mut Object = msg_send![class!(NSOperationQueue), mainQueue];
        let changed = ConcreteBlock::new(|note: *mut Object| {
            // SAFETY: a keyboard notification's userInfo holds the end frame
            // as an NSValue of a CGRect, in screen coordinates
            unsafe {
                let info: *mut Object = msg_send![note, userInfo];
                let value: *mut Object =
                    msg_send![info, objectForKey: UIKeyboardFrameEndUserInfoKey];
                if value.is_null() {
                    return;
                }
                let kb: CGRect = msg_send![value, CGRectValue];
                let screen: *mut Object = msg_send![class!(UIScreen), mainScreen];
                let s: CGRect = msg_send![screen, bounds];
                // a floating keyboard, narrower than the screen, sits over
                // the app wherever it is dragged: nothing to make room for
                let cover = if kb.w < s.w * 0.9 {
                    0.0
                } else {
                    (s.y + s.h - kb.y).clamp(0.0, s.h)
                };
                set(cover);
            }
        })
        .copy();
        let hidden = ConcreteBlock::new(|_note: *mut Object| set(0.0)).copy();
        let none = null_mut::<Object>();
        let _: *mut Object = msg_send![center, addObserverForName: UIKeyboardWillChangeFrameNotification object: none queue: queue usingBlock: &*changed];
        let _: *mut Object = msg_send![center, addObserverForName: UIKeyboardWillHideNotification object: none queue: queue usingBlock: &*hidden];
        std::mem::forget(changed);
        std::mem::forget(hidden);
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
            keys_for("\u{2303}1 \u{2013} \u{2303}9", false),
            "Ctrl+1 \u{2013} Ctrl+9"
        );
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
