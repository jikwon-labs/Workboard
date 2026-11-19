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
