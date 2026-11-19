// Copies on disk: the files in a shared folder, writing this device's copy,
// and telling which app version saved one.

use super::*;

/// The app version a copy in a shared folder was written by (`"app"`,
/// written by `write_mine` from this version on). None for copies from
/// before.
pub fn app_version_of(copy: &Value) -> Option<&str> {
    copy.get("app").and_then(|a| a.as_str())
}

/// "0.1.7" -> (0, 1, 7); missing parts count as 0.
pub(crate) fn version_key(v: &str) -> Vec<u64> {
    v.split('.')
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// A copy written by an app older than `mine` (or by one from before apps
/// wrote their version, or with an older sync format): such an app erases
/// fields it doesn't know when its copy wins. Returns its version when it
/// wrote one ("" when it didn't).
pub fn older_app<'a>(copy: &'a Value, mine: &str) -> Option<&'a str> {
    let v = meta_of(copy).v;
    match app_version_of(copy) {
        None => Some(""),
        Some(a) if version_key(a) < version_key(mine) || v < META_V => Some(a),
        Some(_) => None,
    }
}

/// The other people's files in a shared folder, with their change times.
pub fn other_files(folder: &Path, device: &str) -> Vec<(PathBuf, std::time::SystemTime)> {
    let mine = format!("{}{}", device, FILE_SUFFIX);
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(folder) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(FILE_SUFFIX) && name != mine && !name.starts_with('.') {
                if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                    out.push((e.path(), m));
                }
            }
        }
    }
    out.sort();
    out
}

pub fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// Write this computer's copy (personal notes and reminders left out),
/// saying which app version wrote it (`app`, see `older_app`).
pub fn write_mine(folder: &Path, device: &str, data: &Value, app: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let mut v = shareable(data);
    if let Some(o) = v.as_object_mut() {
        o.insert("app".into(), Value::from(app));
    }
    let tmp = folder.join(format!(".{}.tmp", device));
    std::fs::write(&tmp, serde_json::to_string(&v).unwrap_or_default())?;
    std::fs::rename(&tmp, folder.join(format!("{}{}", device, FILE_SUFFIX)))
}

/// Folders of sync services found on this computer, as places to share from.
pub fn suggested_folders() -> Vec<(String, String)> {
    // Windows has no $HOME
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    let h = PathBuf::from(&home);
    let mut out = vec![];
    let icloud = h.join("Library/Mobile Documents/com~apple~CloudDocs");
    if icloud.is_dir() {
        out.push((
            "iCloud Drive".to_string(),
            icloud.join("Workboard").to_string_lossy().to_string(),
        ));
    }
    if let Ok(rd) = std::fs::read_dir(h.join("Library/CloudStorage")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("GoogleDrive") {
                let my = e.path().join("My Drive");
                let base = if my.is_dir() { my } else { e.path() };
                out.push((
                    "Google Drive".to_string(),
                    base.join("Workboard").to_string_lossy().to_string(),
                ));
            } else if name.starts_with("Dropbox") {
                out.push((
                    "Dropbox".to_string(),
                    e.path().join("Workboard").to_string_lossy().to_string(),
                ));
            } else if name.starts_with("OneDrive") {
                out.push((
                    "OneDrive".to_string(),
                    e.path().join("Workboard").to_string_lossy().to_string(),
                ));
            }
        }
    }
    let dropbox = h.join("Dropbox");
    if dropbox.is_dir() && !out.iter().any(|(n, _)| n == "Dropbox") {
        out.push((
            "Dropbox".to_string(),
            dropbox.join("Workboard").to_string_lossy().to_string(),
        ));
    }
    out
}
