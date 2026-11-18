// Small helpers: time, random bytes, hashing tokens and passwords,
// checking emails, passwords and invite codes.

use super::*;

/// `WORKBOARD_SERVER_HOME`, else ~/Library/Application Support/WorkboardServer.
pub fn home_dir() -> PathBuf {
    if let Ok(d) = std::env::var("WORKBOARD_SERVER_HOME") {
        if !d.trim().is_empty() {
            return PathBuf::from(d);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    Path::new(&home).join("Library/Application Support/WorkboardServer")
}

/// How long the server remembers what a deleted message said (see
/// Saved::graves): after that, only owners and admins bring it back.
pub(crate) const GRAVE_DAYS: i64 = 365;

/// How far ahead of the server's clock an app's change times may be (a
/// computer's clock a little fast); later ones count as now + this.
pub(crate) const FUTURE_SKEW_MS: i64 = 5 * 60_000;

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Bytes from the system's secure random source.
pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    use std::io::Read;
    let mut b = [0u8; N];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .expect("read /dev/urandom");
    b
}

pub(crate) fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

pub(crate) fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(token.as_bytes()))
}

/// PBKDF2 with HMAC-SHA256, one 32-byte block.
pub(crate) fn pbkdf2_sha256(password: &[u8], salt: &[u8], rounds: u32) -> [u8; 32] {
    use hmac::{KeyInit, Mac};
    let prf = hmac::Hmac::<sha2::Sha256>::new_from_slice(password).expect("any key length");
    let mut mac = prf.clone();
    mac.update(salt);
    mac.update(&1u32.to_be_bytes());
    let mut u: [u8; 32] = mac.finalize().into_bytes().into();
    let mut out = u;
    for _ in 1..rounds {
        let mut mac = prf.clone();
        mac.update(&u);
        u = mac.finalize().into_bytes().into();
        for (o, x) in out.iter_mut().zip(u.iter()) {
            *o ^= x;
        }
    }
    out
}

/// Emails compare without case or surrounding spaces.
pub(crate) fn norm_email(e: &str) -> String {
    e.trim().to_lowercase()
}

pub(crate) fn email_ok(e: &str) -> bool {
    let e = e.trim();
    match e.split_once('@') {
        Some((a, b)) => {
            !a.is_empty()
                && b.contains('.')
                && !b.starts_with('.')
                && !b.ends_with('.')
                && !e.contains(char::is_whitespace)
        }
        None => false,
    }
}

/// Conservative ASCII mailbox syntax for the local teammate-invite command.
/// This intentionally does not accept display names or quoted local parts.
/// Existing account/login validation is unchanged.
pub fn normalize_invite_email(input: &str) -> Result<String, String> {
    let invalid =
        || "Type a valid ASCII email address (for example, member@example.test).".to_string();
    if !input.is_ascii() || input.chars().any(char::is_control) {
        return Err(invalid());
    }
    let email = norm_email(input);
    let (local, domain) = email.split_once('@').ok_or_else(invalid)?;
    let atom = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-/=?^_`{|}~.".contains(&b);
    let domain_ok = domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        });
    if email.len() > 254
        || local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || !local.bytes().all(atom)
        || !domain_ok
    {
        return Err(invalid());
    }
    Ok(email)
}

pub(crate) fn password_problem(p: &str) -> Option<String> {
    (p.chars().count() < MIN_PASSWORD)
        .then(|| format!("Choose a password of at least {} characters.", MIN_PASSWORD))
}

/// Compare without leaking where two strings differ.
pub(crate) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// The `<device>:<token>` pair of an `Authorization: Bearer` header, which
/// is how the app signs its HTTP requests.
pub(crate) fn bearer(headers: &axum::http::HeaderMap) -> Option<(&str, &str)> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|v| v.split_once(':'))
}

/// XXXX-XXXX-XXXX without look-alike letters.
pub(crate) fn new_code() -> String {
    const ABC: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let b = random_bytes::<12>();
    let s: String = b
        .iter()
        .map(|x| ABC[*x as usize % ABC.len()] as char)
        .collect();
    format!("{}-{}-{}", &s[0..4], &s[4..8], &s[8..12])
}

/// What people type: any case, with or without dashes and spaces.
pub(crate) fn normalize_code(c: &str) -> String {
    c.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}
