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

/// Open a file or web address with whatever the system uses for it.
pub fn open_path(p: impl AsRef<std::ffi::OsStr>) {
    let p = p.as_ref();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(windows)]
    {
        // `start` treats its first quoted argument as a window title
        let _ = std::process::Command::new("cmd")
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

#[cfg(test)]
mod tests {
    use super::*;

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
