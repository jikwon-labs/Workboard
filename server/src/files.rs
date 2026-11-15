// Pictures and videos for chat messages. A message only names its files (by
// the sha256 of their contents); the files themselves live here:
//
//   PUT  /files/<sha256>   store a file; the contents must hash to the name
//   GET  /files/<sha256>   the file, with Range support (resume, seeking)
//   HEAD /files/<sha256>   whether it's here, and its size
//
// Every request needs `Authorization: Bearer <device>:<token>`, the same
// device token an app signs in with. Files sit in the server folder under
// files/, next to <sha256>.type holding the content type given on upload.
// They are not part of backups (those copy workspace.json only). Once a day
// collect_garbage deletes files that have sat unchanged for FILE_KEEP_DAYS and
// that neither the team's copy nor any kept backup names anymore.

use super::*;
use axum::body::Body;
use axum::extract::{Path as UrlPath, Request};
use axum::http::{HeaderMap, HeaderValue, Method};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

pub const FILES_DIR: &str = "files";
/// The largest file an app may put here.
pub const MAX_FILE: u64 = 500 << 20;
/// A file nothing names anymore is deleted only after this long unchanged.
pub const FILE_KEEP_DAYS: u64 = 30;

/// A file name the files route accepts: 64 lowercase hex digits.
pub fn file_name_ok(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The request carries a device token this server issued (and hasn't taken back).
fn authorized(hub: &Hub, headers: &HeaderMap) -> bool {
    let Some(rest) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")) else { return false };
    let Some((device, token)) = rest.split_once(':') else { return false };
    let g = hub.inner.lock().unwrap();
    g.saved.devices.get(device).is_some_and(|d| d.token == hash_token(token))
}

fn status(code: StatusCode, why: &'static str) -> Response {
    (code, why).into_response()
}

/// A content type safe to store and send back as a header.
fn clean_type(headers: &HeaderMap) -> String {
    let t = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
    if t.is_empty() || t.len() > 100 || !t.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
        "application/octet-stream".into()
    } else {
        t.to_string()
    }
}

pub(crate) async fn put_file(UrlPath(name): UrlPath<String>, State(hub): State<Arc<Hub>>, req: Request) -> Response {
    if !file_name_ok(&name) {
        return status(StatusCode::NOT_FOUND, "no such file");
    }
    if !authorized(&hub, req.headers()) {
        return status(StatusCode::UNAUTHORIZED, "sign in first");
    }
    let dir = hub.home.join(FILES_DIR);
    let dest = dir.join(&name);
    if tokio::fs::metadata(&dest).await.is_ok() {
        // a message is about to name it again: it's new for collect_garbage
        let d = dest.clone();
        let _ = tokio::task::spawn_blocking(move || std::fs::File::options().write(true).open(&d).and_then(|f| f.set_modified(std::time::SystemTime::now()))).await;
        return status(StatusCode::OK, "already here");
    }
    let declared = req.headers().get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > hub.max_file) {
        return status(StatusCode::PAYLOAD_TOO_LARGE, "file is too large");
    }
    let kind = clean_type(req.headers());
    if tokio::fs::create_dir_all(&dir).await.is_err() {
        return status(StatusCode::INTERNAL_SERVER_ERROR, "can't write files");
    }
    let tmp = dir.join(format!(".tmp-{}", hex(&random_bytes::<8>())));
    let Ok(mut f) = tokio::fs::File::create(&tmp).await else { return status(StatusCode::INTERNAL_SERVER_ERROR, "can't write files") };
    let mut hash = Sha256::new();
    let mut n: u64 = 0;
    let mut body = req.into_body().into_data_stream();
    let fail = |code, why| {
        let tmp = tmp.clone();
        async move {
            let _ = tokio::fs::remove_file(&tmp).await;
            status(code, why)
        }
    };
    while let Some(chunk) = body.next().await {
        let Ok(bytes) = chunk else { return fail(StatusCode::BAD_REQUEST, "upload broke off").await };
        n += bytes.len() as u64;
        if n > hub.max_file {
            return fail(StatusCode::PAYLOAD_TOO_LARGE, "file is too large").await;
        }
        hash.update(&bytes);
        if f.write_all(&bytes).await.is_err() {
            return fail(StatusCode::INTERNAL_SERVER_ERROR, "can't write files").await;
        }
    }
    if f.flush().await.is_err() || f.sync_all().await.is_err() {
        return fail(StatusCode::INTERNAL_SERVER_ERROR, "can't write files").await;
    }
    drop(f);
    if declared.is_some_and(|d| d != n) {
        return fail(StatusCode::BAD_REQUEST, "upload broke off").await;
    }
    if hex(&hash.finalize()) != name {
        return fail(StatusCode::BAD_REQUEST, "contents don't match the name").await;
    }
    let _ = tokio::fs::write(dir.join(format!("{}.type", name)), kind).await;
    if tokio::fs::rename(&tmp, &dest).await.is_err() {
        return fail(StatusCode::INTERNAL_SERVER_ERROR, "can't write files").await;
    }
    status(StatusCode::CREATED, "stored")
}

/// `bytes=a-b` or `bytes=a-` (one range only); None when absent or unreadable.
pub fn parse_range(v: &str, len: u64) -> Option<Result<(u64, u64), ()>> {
    let spec = v.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let r = if a.is_empty() {
        // the last N bytes
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return Some(Err(()));
        }
        (len.saturating_sub(n), len.saturating_sub(1))
    } else {
        let a: u64 = a.parse().ok()?;
        let b: u64 = if b.is_empty() { len.saturating_sub(1) } else { b.parse::<u64>().ok()?.min(len.saturating_sub(1)) };
        (a, b)
    };
    if len == 0 || r.0 >= len || r.0 > r.1 {
        return Some(Err(()));
    }
    Some(Ok(r))
}

pub(crate) async fn get_file(UrlPath(name): UrlPath<String>, State(hub): State<Arc<Hub>>, method: Method, headers: HeaderMap) -> Response {
    if !file_name_ok(&name) {
        return status(StatusCode::NOT_FOUND, "no such file");
    }
    if !authorized(&hub, &headers) {
        return status(StatusCode::UNAUTHORIZED, "sign in first");
    }
    let dir = hub.home.join(FILES_DIR);
    let Ok(mut f) = tokio::fs::File::open(dir.join(&name)).await else { return status(StatusCode::NOT_FOUND, "no such file") };
    let Ok(len) = f.metadata().await.map(|m| m.len()) else { return status(StatusCode::NOT_FOUND, "no such file") };
    let kind = tokio::fs::read_to_string(dir.join(format!("{}.type", name))).await.unwrap_or_else(|_| "application/octet-stream".into());
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|v| parse_range(v, len));
    let (code, start, end) = match range {
        None => (StatusCode::OK, 0, len.saturating_sub(1)),
        Some(Ok((a, b))) => (StatusCode::PARTIAL_CONTENT, a, b),
        Some(Err(())) => {
            return Response::builder().status(StatusCode::RANGE_NOT_SATISFIABLE).header(header::CONTENT_RANGE, format!("bytes */{}", len)).body(Body::empty()).unwrap();
        }
    };
    let count = if len == 0 { 0 } else { end - start + 1 };
    let mut res = Response::builder()
        .status(code)
        .header(header::CONTENT_TYPE, HeaderValue::from_str(&kind).unwrap_or(HeaderValue::from_static("application/octet-stream")))
        .header(header::CONTENT_LENGTH, count)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, "private, max-age=31536000, immutable");
    if code == StatusCode::PARTIAL_CONTENT {
        res = res.header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, len));
    }
    if method == Method::HEAD || count == 0 {
        return res.body(Body::empty()).unwrap();
    }
    if start > 0 && f.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return status(StatusCode::INTERNAL_SERVER_ERROR, "can't read the file");
    }
    let stream = tokio_util::io::ReaderStream::with_capacity(f.take(count), 256 << 10);
    res.body(Body::from_stream(stream)).unwrap()
}

/// Every file name (64 hex digits in quotes) a JSON text mentions.
fn named_files(text: &str, into: &mut std::collections::HashSet<String>) {
    let b = text.as_bytes();
    let mut i = 0;
    while i + 66 <= b.len() {
        if b[i] == b'"' && b[i + 65] == b'"' {
            if let Some(name) = text.get(i + 1..i + 65).filter(|n| file_name_ok(n)) {
                into.insert(name.to_string());
                i += 66;
                continue;
            }
        }
        i += 1;
    }
}

/// Delete the files in files/ that nothing needs anymore: unchanged for
/// FILE_KEEP_DAYS, and named neither in `canon_json` (the team's copy) nor
/// in any file in backups/ (a restore may bring their messages back).
/// Nothing is deleted without a team copy, or when a backup can't be read.
/// Leftover partial uploads that old go too. Returns the names deleted.
pub fn collect_garbage(home: &Path, canon_json: Option<&str>, now: std::time::SystemTime) -> Vec<String> {
    let Some(canon) = canon_json else { return vec![] };
    let mut named = std::collections::HashSet::new();
    named_files(canon, &mut named);
    if let Ok(rd) = std::fs::read_dir(home.join(BACKUP_DIR)) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "json") {
                match std::fs::read_to_string(&p) {
                    Ok(t) => named_files(&t, &mut named),
                    Err(err) => {
                        eprintln!("not cleaning up files: couldn't read {}: {}", p.display(), err);
                        return vec![];
                    }
                }
            }
        }
    }
    let keep = Duration::from_secs(FILE_KEEP_DAYS * 86_400);
    let old = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|age| age >= keep);
    let dir = home.join(FILES_DIR);
    let mut gone = vec![];
    for e in std::fs::read_dir(&dir).map(|rd| rd.flatten().collect::<Vec<_>>()).unwrap_or_default() {
        let name = e.file_name().to_string_lossy().to_string();
        let path = e.path();
        if name.starts_with(".tmp-") {
            if old(&path) {
                let _ = std::fs::remove_file(&path);
            }
            continue;
        }
        // checked last thing before deleting: an upload of the same file
        // a moment ago made it new again
        if file_name_ok(&name) && !named.contains(&name) && old(&path) && std::fs::remove_file(&path).is_ok() {
            let _ = std::fs::remove_file(dir.join(format!("{}.type", name)));
            gone.push(name);
        }
    }
    gone
}
