// Saving to disk: the workspace and server.json, email invites, backups
// and restoring one.

use super::*;

/// Email invitations must save both snapshots before exposing a usable code.
/// Stage both files while the hub/persistence locks keep live state unchanged.
pub(crate) fn persist_email_invite(
    home: &Path,
    canon: &Value,
    saved: &Saved,
) -> Result<(), String> {
    let workspace = home.join(WORKSPACE_FILE);
    let state = home.join(STATE_FILE);
    let workspace_tmp = tmp_path(&workspace);
    let state_tmp = tmp_path(&state);
    let result = (|| -> std::io::Result<()> {
        let before = match std::fs::read(&workspace) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        // Catch known destination failures before installing either snapshot.
        match std::fs::metadata(&state) {
            Ok(meta) if !meta.is_file() => {
                return Err(std::io::Error::other("server.json is not a regular file"));
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        write_private(&workspace_tmp, &serde_json::to_vec(canon)?)?;
        write_private(&state_tmp, &serde_json::to_vec_pretty(saved)?)?;
        commit_email_invite(&workspace, &state, before.as_deref())?;
        sync_folder(&workspace);
        Ok(())
    })();
    // Never remove a directory (including a fault-injection blocker).
    let _ = std::fs::remove_file(&workspace_tmp);
    let _ = std::fs::remove_file(&state_tmp);
    result.map_err(|e| format!("Couldn't save invitation: {e}. No new code was issued."))
}

/// Two renames are not a filesystem transaction. If the second fails, restore
/// the exact old workspace (or its absence); never install live invite state.
/// Rollback is best effort: a second I/O failure requires operator recovery.
pub(crate) fn commit_email_invite(
    workspace: &Path,
    state: &Path,
    before: Option<&[u8]>,
) -> std::io::Result<()> {
    let workspace_tmp = tmp_path(workspace);
    std::fs::rename(&workspace_tmp, workspace)?;
    if let Err(e) = std::fs::rename(tmp_path(state), state) {
        let rollback = match before {
            Some(bytes) => write_private(&workspace_tmp, bytes)
                .and_then(|()| std::fs::rename(&workspace_tmp, workspace)),
            None => std::fs::remove_file(workspace),
        };
        if let Err(rollback) = rollback {
            return Err(std::io::Error::other(format!(
                "{e}; workspace rollback also failed: {rollback}; repair workspace.json before restarting"
            )));
        }
        return Err(e);
    }
    Ok(())
}

/// Replace a file whole or not at all, even across a power loss: write a
/// temp file next to it, flush that to disk, rename it over, then flush the
/// folder. Only this user may read it: the workspace holds everyone's
/// messages, server.json the sign-ins.
pub(crate) fn write_atomic(path: &Path, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    let tmp = tmp_path(path);
    write_private(&tmp, bytes.as_ref())?;
    std::fs::rename(&tmp, path)?;
    sync_folder(path);
    Ok(())
}

/// Where [`write_atomic`] stages `path`: `workspace.json.tmp` and so on.
pub(crate) fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Write `bytes` to `path` readable by this user only, and flush it to disk.
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        open.mode(0o600);
        // (an older file keeps its mode when opened again)
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    let mut f = open.open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Flush the folder holding `path`, so a rename into it lasts too (not
/// supported everywhere, so failures are ignored).
pub(crate) fn sync_folder(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::File::open(dir).and_then(|d| d.sync_all());
    }
}

pub(crate) fn read_json(path: &Path) -> std::io::Result<Option<Value>> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).map(Some).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{}: {}", path.display(), e),
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Load server.json; one from stage 1 (devices without tokens) keeps only its
/// revision, so those computers have to be invited again.
pub fn load_saved(home: &Path) -> std::io::Result<Saved> {
    Ok(match read_json(&home.join(STATE_FILE))? {
        None => Saved::default(),
        Some(v) => serde_json::from_value(v.clone()).unwrap_or_else(|_| Saved {
            rev: v.get("rev").and_then(|r| r.as_u64()).unwrap_or(0),
            ..Default::default()
        }),
    })
}

// ------------------------------------------------------------------ backups

/// Copy the team's copy into backups/ once an hour and once a day (only when
/// it differs from the newest copy of that kind), keeping the newest
/// KEEP_HOURLY and KEEP_DAILY. Returns the files written.
pub fn backup(
    home: &Path,
    canon_json: &str,
    now: chrono::NaiveDateTime,
) -> std::io::Result<Vec<PathBuf>> {
    let dir = home.join(BACKUP_DIR);
    std::fs::create_dir_all(&dir)?;
    let mut written = vec![];
    for (kind, name, keep) in [
        (
            "hourly",
            now.format("hourly-%Y%m%d-%H.json").to_string(),
            KEEP_HOURLY,
        ),
        (
            "daily",
            now.format("daily-%Y%m%d.json").to_string(),
            KEEP_DAILY,
        ),
    ] {
        let mut files = backup_files(&dir, kind);
        let path = dir.join(&name);
        let same_as_newest = files
            .last()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .is_some_and(|s| s == canon_json);
        if !path.exists() && !same_as_newest {
            write_atomic(&path, canon_json)?;
            written.push(path.clone());
            files.push(path);
            files.sort();
        }
        while files.len() > keep {
            let _ = std::fs::remove_file(files.remove(0));
        }
    }
    Ok(written)
}

/// Backups of one kind ("hourly", "daily"), oldest first.
pub fn backup_files(dir: &Path, kind: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                        n.starts_with(&format!("{}-", kind)) && n.ends_with(".json")
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

impl Hub {
    /// Every backup, newest first.
    pub(crate) fn list_backups(&self) -> Vec<Backup> {
        let dir = self.home.join(BACKUP_DIR);
        let mut all: Vec<(std::time::SystemTime, Backup)> = ["hourly", "daily", "before-restore"]
            .iter()
            .flat_map(|k| backup_files(&dir, k))
            .filter_map(|p| {
                let meta = std::fs::metadata(&p).ok()?;
                Some((
                    meta.modified().ok()?,
                    Backup {
                        name: p.file_name()?.to_string_lossy().to_string(),
                        bytes: meta.len(),
                    },
                ))
            })
            .collect();
        all.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.name.cmp(&a.1.name)));
        all.into_iter().map(|(_, b)| b).collect()
    }

    /// A backup by its file name, only from the backups folder.
    pub(crate) fn backup_path(&self, name: &str) -> Result<PathBuf, String> {
        let ok_name = !name.contains('/')
            && !name.contains('\\')
            && !name.contains("..")
            && name.ends_with(".json")
            && ["hourly-", "daily-", "before-restore-"]
                .iter()
                .any(|p| name.starts_with(p));
        let path = self.home.join(BACKUP_DIR).join(name);
        if !ok_name || !path.is_file() {
            return Err("That backup isn't on the server.".into());
        }
        Ok(path)
    }

    /// Go back to a backup: its items win and deleted ones come back; things
    /// made after it stay. The copy from before goes to backups/ first.
    pub(crate) fn restore(&self, path: &Path) -> Result<(), String> {
        let mut data = match read_json(path) {
            Ok(Some(v)) => v,
            Ok(None) => return Err(format!("{} doesn't exist", path.display())),
            Err(e) => return Err(e.to_string()),
        };
        if !copy_shape_ok(&data) {
            return Err(format!("{} isn't a Workboard workspace", path.display()));
        }
        {
            mark_restored(&mut data, now_ms());
            let data = wb_sync::shareable(&data);
            let mut g = self.inner.lock().unwrap();
            if let Some(cur) = &g.canon {
                let keep = self.home.join(BACKUP_DIR).join(format!(
                    "before-restore-{}.json",
                    chrono::Local::now().format("%Y%m%d-%H%M%S")
                ));
                let _ = std::fs::create_dir_all(self.home.join(BACKUP_DIR));
                let _ = write_atomic(&keep, &serde_json::to_string(cur).unwrap_or_default());
                let mut kept = backup_files(&self.home.join(BACKUP_DIR), "before-restore");
                while kept.len() > KEEP_BEFORE_RESTORE {
                    let _ = std::fs::remove_file(kept.remove(0));
                }
            }
            match g.canon.as_mut() {
                Some(c) => {
                    // the restored items win; things made after the backup stay
                    let mut restored = data;
                    wb_sync::merge(&mut restored, &[c.clone()]);
                    *c = wb_sync::shareable(&restored);
                }
                None => g.canon = Some(data),
            }
            self.broadcast_canon(&mut g, true);
        }
        Ok(())
    }
}
