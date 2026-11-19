//! When the app can't start or stops on an error. The Windows app has no
//! console, so an error before its window opened (a virtual machine without
//! the graphics the window needs, say) ended it without a word. Now it says
//! why in a message box and keeps the text in startup-error.log in the data
//! folder, where it can be sent along.

use crate::i18n::tr;

const LOG: &str = "startup-error.log";

/// Windows: report a panic on the main thread in a message box (other
/// threads only write the log). `quiet` (screenshots) only writes the log.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn watch_panics(quiet: bool) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        let text = info.to_string();
        let log = save(&text);
        if !quiet && std::thread::current().name() == Some("main") {
            show(tr("Workboard stopped because of an error."), &text, log);
        }
    }));
}

/// The window couldn't be opened: say why instead of ending silently.
pub fn failed(error: &str, quiet: bool) {
    let log = save(error);
    if !quiet {
        show(tr("Workboard couldn't open its window."), error, log);
    }
}

/// Write `text` to the log; where it went.
fn save(text: &str) -> Option<std::path::PathBuf> {
    let dir = crate::model::Data::dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(LOG);
    let when = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    std::fs::write(&path, format!("{when}\n{text}\n")).ok()?;
    Some(path)
}

/// The language the person chose (the app sets it only once its window is
/// open, which may never happen here).
fn set_lang() {
    crate::i18n::set_lang(crate::i18n::resolve(crate::model::Data::language()));
}

#[cfg(windows)]
fn show(what: &str, detail: &str, log: Option<std::path::PathBuf>) {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    set_lang();
    let mut text = format!("{}\n\n{detail}", tr(what));
    if let Some(log) = log {
        text.push_str(&format!(
            "\n\n{}\n{}",
            tr("The details were saved to:"),
            log.display()
        ));
    }
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (text, title) = (wide(&text), wide("Workboard"));
    unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show(what: &str, detail: &str, log: Option<std::path::PathBuf>) {
    let _ = set_lang;
    eprintln!("{what}\n{detail}");
    if let Some(log) = log {
        eprintln!("{}", log.display());
    }
}
