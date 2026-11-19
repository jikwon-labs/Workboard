// (tests keep tokens in memory and need nothing from the parent)
#[cfg_attr(test, allow(unused_imports))]
use super::*;

// ---------------------------------------------------------------- tokens
//
// The token a server gives this computer lives in the macOS keychain (never
// in data.json). When trying the app with a separate WORKBOARD_HOME folder it
// lives in that folder instead, and tests keep it in memory.

#[cfg(all(not(test), target_os = "macos"))]
const KEYCHAIN_SERVICE: &str = "Workboard server";

fn token_account(addr: &str, device: &str) -> String {
    format!("{}#{}", addr.trim(), device)
}

#[cfg(test)]
fn test_tokens() -> &'static std::sync::Mutex<std::collections::HashMap<String, String>> {
    static T: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
        std::sync::OnceLock::new();
    T.get_or_init(Default::default)
}

#[cfg(not(test))]
fn token_file() -> Option<PathBuf> {
    let separate = std::env::var("WORKBOARD_HOME").is_ok_and(|v| !v.is_empty());
    (separate || !cfg!(target_os = "macos")).then(|| Data::dir().join("server-tokens.json"))
}

#[cfg(not(test))]
fn read_token_file(p: &Path) -> std::collections::BTreeMap<String, String> {
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(not(test))]
fn write_token_file(p: &Path, m: &std::collections::BTreeMap<String, String>) {
    let _ = std::fs::create_dir_all(p.parent().unwrap_or(Path::new(".")));
    if std::fs::write(p, serde_json::to_string_pretty(m).unwrap_or_default()).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
        }
    }
}

/// Goes up whenever a token is saved or forgotten, so a copy kept to save
/// reading the keychain every frame knows it's out of date.
static TOKEN_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(crate) fn token_gen() -> u64 {
    TOKEN_GEN.load(std::sync::atomic::Ordering::SeqCst)
}

pub(crate) fn load_token(addr: &str, device: &str) -> Option<String> {
    let key = token_account(addr, device);
    #[cfg(test)]
    return test_tokens().lock().unwrap().get(&key).cloned();
    #[cfg(not(test))]
    {
        if let Some(p) = token_file() {
            return read_token_file(&p).get(&key).cloned();
        }
        #[cfg(target_os = "macos")]
        return security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, &key)
            .ok()
            .and_then(|b| String::from_utf8(b).ok());
        #[allow(unreachable_code)]
        None
    }
}

pub(crate) fn save_token(addr: &str, device: &str, token: &str) {
    let key = token_account(addr, device);
    TOKEN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    #[cfg(test)]
    {
        test_tokens().lock().unwrap().insert(key, token.to_string());
    }
    #[cfg(not(test))]
    {
        if let Some(p) = token_file() {
            let mut m = read_token_file(&p);
            m.insert(key, token.to_string());
            write_token_file(&p, &m);
            return;
        }
        #[cfg(target_os = "macos")]
        let _ = security_framework::passwords::set_generic_password(
            KEYCHAIN_SERVICE,
            &key,
            token.as_bytes(),
        );
    }
}

pub(crate) fn forget_token(addr: &str, device: &str) {
    let key = token_account(addr, device);
    TOKEN_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    #[cfg(test)]
    {
        test_tokens().lock().unwrap().remove(&key);
    }
    #[cfg(not(test))]
    {
        if let Some(p) = token_file() {
            let mut m = read_token_file(&p);
            m.remove(&key);
            write_token_file(&p, &m);
            return;
        }
        #[cfg(target_os = "macos")]
        let _ = security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, &key);
    }
}
