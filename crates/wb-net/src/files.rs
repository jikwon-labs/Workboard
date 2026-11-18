// Moving pictures and videos to and from the server's /files route (see
// server/src/files.rs): plain HTTP/1.1 over a TcpStream (in TLS for an
// https server), one request per connection, so there is no extra client
// library. Every call blocks; the
// app runs them on its own thread and reports progress through a callback.

use crate::conn::{self, Conn};
use crate::{is_secure, server_url};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

/// A file name the server's /files route uses: 64 lowercase hex digits (a
/// sha256). Names come from synced data another member wrote, so anything
/// else is refused before it reaches a request line or a path on disk.
pub fn name_ok(sha: &str) -> bool {
    sha.len() == 64
        && sha
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// How an app proves who it is: the device id and token it signs in with.
#[derive(Clone, Debug, PartialEq)]
pub struct Auth {
    pub device: String,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FileError {
    /// The server doesn't know this token (signed out or access removed).
    NotSignedIn,
    TooLarge,
    /// The server has no such file.
    Missing,
    /// The server is older and has no /files route.
    Unsupported,
    /// The person stopped it.
    Cancelled,
    Net(String),
    Other(String),
}

impl FileError {
    pub fn explain(&self) -> String {
        match self {
            FileError::NotSignedIn => "You're not signed in to the server.".into(),
            FileError::TooLarge => "The file is larger than the server takes (90 MB).".into(),
            FileError::Missing => "The server doesn't have this file.".into(),
            FileError::Unsupported => {
                "The server doesn't take pictures and videos yet. Update workboard-server.".into()
            }
            FileError::Cancelled => "Stopped.".into(),
            FileError::Net(e) => format!("Couldn't reach the server: {}", e),
            FileError::Other(e) => e.clone(),
        }
    }
}

/// sha256 of a file's contents (hex), and its size.
pub fn sha256_file(path: &Path) -> std::io::Result<(String, u64)> {
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 << 10];
    let mut n = 0u64;
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((
        h.finalize().iter().map(|b| format!("{:02x}", b)).collect(),
        n,
    ))
}

struct Head {
    status: u16,
    len: Option<u64>,
    range_total: Option<u64>,
    /// The body comes in chunks (Transfer-Encoding: chunked), as a proxy in
    /// front of the server may send it; `len` is None then.
    chunked: bool,
}

/// Reads a chunked body as the bytes it carries.
struct Chunked<R> {
    inner: R,
    /// Bytes left in the current chunk.
    left: u64,
    done: bool,
}

impl<R: BufRead> Chunked<R> {
    fn new(inner: R) -> Self {
        Chunked {
            inner,
            left: 0,
            done: false,
        }
    }

    fn line(&mut self) -> std::io::Result<String> {
        let mut l = String::new();
        if read_line(&mut self.inner, &mut l)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        Ok(l)
    }
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.done || buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            let l = self.line()?;
            let size = l.trim().split(';').next().unwrap_or("");
            self.left = u64::from_str_radix(size.trim(), 16)
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad chunk"))?;
            if self.left == 0 {
                // trailers until the empty line
                while !self.line()?.trim().is_empty() {}
                self.done = true;
                return Ok(0);
            }
        }
        let want = buf.len().min(self.left as usize);
        let k = self.inner.read(&mut buf[..want])?;
        if k == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        self.left -= k as u64;
        if self.left == 0 {
            // the line break after the chunk
            self.line()?;
        }
        Ok(k)
    }
}

/// The body after `head`, whichever way it comes.
fn body<'a, R: BufRead>(r: &'a mut R, head: &Head) -> Box<dyn Read + 'a> {
    if head.chunked {
        Box::new(Chunked::new(r))
    } else {
        Box::new(r.take(head.len.unwrap_or(u64::MAX)))
    }
}

/// A connection to the server of `addr`, the Host header to send, and
/// what paths of the workspace start with (`workspace_prefix`).
fn open(addr: &str) -> Result<(Conn, String, String), FileError> {
    let (host, port, url) = server_url(addr).map_err(FileError::Other)?;
    let secure = is_secure(&url);
    let s = conn::dial(&host, port, secure, Duration::from_secs(10)).map_err(FileError::Net)?;
    let _ = s.tcp().set_read_timeout(Some(Duration::from_secs(60)));
    let _ = s.tcp().set_write_timeout(Some(Duration::from_secs(60)));
    // the Host header: what the URL had (a proxy in front picks the server by it)
    let start = url.find("://").map(|i| i + 3).unwrap_or(0);
    let end = url[start..]
        .find('/')
        .map(|i| start + i)
        .unwrap_or(url.len());
    Ok((
        s,
        url[start..end].to_string(),
        crate::workspace_prefix(addr),
    ))
}

fn request(
    s: &mut Conn,
    method: &str,
    (host, prefix): (&str, &str),
    auth: &Auth,
    sha: &str,
    extra: &[(&str, String)],
) -> Result<(), FileError> {
    if !name_ok(sha) {
        return Err(FileError::Other(
            "That isn't a file name the server uses.".into(),
        ));
    }
    let mut r = format!(
        "{} {}/files/{} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}:{}\r\nConnection: close\r\n",
        method, prefix, sha, host, auth.device, auth.token
    );
    for (k, v) in extra {
        r.push_str(&format!("{}: {}\r\n", k, v));
    }
    r.push_str("\r\n");
    s.write_all(r.as_bytes())
        .map_err(|e| FileError::Net(e.to_string()))
}

/// One header or chunk-size line, at most 8 KB (an endless line from a
/// broken or hostile server would otherwise fill memory).
fn read_line(r: &mut impl BufRead, line: &mut String) -> std::io::Result<usize> {
    let n = r.take(8 << 10).read_line(line)?;
    if n == 8 << 10 && !line.ends_with('\n') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the server's answer had a line that was too long",
        ));
    }
    Ok(n)
}

fn read_head(r: &mut impl BufRead) -> Result<Head, FileError> {
    let mut line = String::new();
    read_line(r, &mut line).map_err(|e| FileError::Net(e.to_string()))?;
    let status = line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| FileError::Net("the server's answer didn't make sense".into()))?;
    let mut head = Head {
        status,
        len: None,
        range_total: None,
        chunked: false,
    };
    loop {
        line.clear();
        if read_line(r, &mut line).map_err(|e| FileError::Net(e.to_string()))? == 0
            || line == "\r\n"
            || line == "\n"
        {
            break;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => head.len = v.parse().ok(),
            "transfer-encoding" => head.chunked = v.to_ascii_lowercase().contains("chunked"),
            // bytes a-b/total
            "content-range" => {
                head.range_total = v.rsplit_once('/').and_then(|(_, t)| t.parse().ok())
            }
            _ => {}
        }
    }
    Ok(head)
}

fn read_text(r: &mut impl BufRead, head: &Head) -> String {
    let mut text = String::new();
    let limit = if head.chunked {
        4096
    } else {
        head.len.unwrap_or(0).min(4096)
    };
    let _ = body(r, head).take(limit).read_to_string(&mut text);
    text
}

fn fail(status: u16, body: &str) -> FileError {
    match status {
        401 => FileError::NotSignedIn,
        413 => FileError::TooLarge,
        // our route says why; an older server's router answers 404 with nothing
        404 if body.is_empty() => FileError::Unsupported,
        404 | 405 => {
            if body.contains("no such file") {
                FileError::Missing
            } else {
                FileError::Unsupported
            }
        }
        _ => FileError::Other(format!("The server said {} {}", status, body.trim())),
    }
}

/// The size of a file on the server, or None when it isn't there.
pub fn remote_size(addr: &str, auth: &Auth, sha: &str) -> Result<Option<u64>, FileError> {
    if !name_ok(sha) {
        return Err(FileError::Other(
            "That isn't a file name the server uses.".into(),
        ));
    }
    let (mut s, host, prefix) = open(addr)?;
    request(&mut s, "HEAD", (&host, &prefix), auth, sha, &[])?;
    let mut r = BufReader::new(s);
    let head = read_head(&mut r)?;
    match head.status {
        200 => Ok(head.len),
        // HEAD has no body to tell a missing file from an older server
        404 => Ok(None),
        c => Err(fail(c, "")),
    }
}

/// Put a local file on the server under its sha256. `progress(done, total)`
/// returns false to stop. Nothing is sent when the server already has it.
pub fn upload(
    addr: &str,
    auth: &Auth,
    sha: &str,
    path: &Path,
    content_type: &str,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<(), FileError> {
    if !name_ok(sha) {
        return Err(FileError::Other(
            "That isn't a file name the server uses.".into(),
        ));
    }
    let mut f = File::open(path).map_err(|e| FileError::Other(e.to_string()))?;
    let total = f
        .metadata()
        .map_err(|e| FileError::Other(e.to_string()))?
        .len();
    if remote_size(addr, auth, sha)? == Some(total) {
        progress(total, total);
        return Ok(());
    }
    let (mut s, host, prefix) = open(addr)?;
    request(
        &mut s,
        "PUT",
        (&host, &prefix),
        auth,
        sha,
        &[
            ("Content-Type", content_type.to_string()),
            ("Content-Length", total.to_string()),
        ],
    )?;
    let mut buf = vec![0u8; 256 << 10];
    let mut done = 0u64;
    loop {
        let k = f
            .read(&mut buf)
            .map_err(|e| FileError::Other(e.to_string()))?;
        if k == 0 {
            break;
        }
        if let Err(e) = s.write_all(&buf[..k]) {
            // the server may have answered early (too large, signed out): read it
            let mut r = BufReader::new(s);
            return match read_head(&mut r) {
                Ok(head) if head.status >= 400 => Err(fail(head.status, &read_text(&mut r, &head))),
                _ => Err(FileError::Net(e.to_string())),
            };
        }
        done += k as u64;
        if !progress(done, total) {
            return Err(FileError::Cancelled);
        }
    }
    let mut r = BufReader::new(s);
    let head = read_head(&mut r)?;
    let body = read_text(&mut r, &head);
    match head.status {
        200 | 201 => Ok(()),
        c => Err(fail(c, &body)),
    }
}

/// Fetch a file from the server into `dest`. It arrives in `dest` + ".part"
/// first, so a broken download continues where it stopped next time; the
/// contents are checked against the sha256 before the file takes its name.
pub fn download(
    addr: &str,
    auth: &Auth,
    sha: &str,
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64) -> bool,
) -> Result<(), FileError> {
    if !name_ok(sha) {
        return Err(FileError::Other(
            "That isn't a file name the server uses.".into(),
        ));
    }
    let part = dest.with_extension("part");
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| FileError::Other(e.to_string()))?;
    }
    let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let (mut s, host, prefix) = open(addr)?;
    let extra: Vec<(&str, String)> = if have > 0 {
        vec![("Range", format!("bytes={}-", have))]
    } else {
        vec![]
    };
    request(&mut s, "GET", (&host, &prefix), auth, sha, &extra)?;
    let mut r = BufReader::with_capacity(256 << 10, s);
    let head = read_head(&mut r)?;
    let (mut out, mut done, total) = match head.status {
        200 => (
            File::create(&part).map_err(|e| FileError::Other(e.to_string()))?,
            0u64,
            head.len.unwrap_or(0),
        ),
        206 => {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&part)
                .map_err(|e| FileError::Other(e.to_string()))?;
            f.seek(SeekFrom::End(0))
                .map_err(|e| FileError::Other(e.to_string()))?;
            (
                f,
                have,
                head.range_total.unwrap_or(have + head.len.unwrap_or(0)),
            )
        }
        416 => {
            // what we have is already everything (or nonsense): start over
            let _ = std::fs::remove_file(&part);
            return download(addr, auth, sha, dest, progress);
        }
        c => {
            let body = read_text(&mut r, &head);
            return Err(fail(c, &body));
        }
    };
    let want = head.len.unwrap_or(u64::MAX);
    let mut got = 0u64;
    let mut buf = vec![0u8; 256 << 10];
    let mut b = body(&mut r, &head);
    while got < want {
        let k = b
            .read(&mut buf)
            .map_err(|e| FileError::Net(e.to_string()))?;
        if k == 0 {
            break;
        }
        out.write_all(&buf[..k])
            .map_err(|e| FileError::Other(e.to_string()))?;
        got += k as u64;
        done += k as u64;
        if !progress(done, total) {
            return Err(FileError::Cancelled);
        }
    }
    drop(b);
    out.flush().map_err(|e| FileError::Other(e.to_string()))?;
    drop(out);
    if head.len.is_some_and(|n| got < n) {
        return Err(FileError::Net("the download broke off".into()));
    }
    let (hash, _) = sha256_file(&part).map_err(|e| FileError::Other(e.to_string()))?;
    if hash != sha {
        let _ = std::fs::remove_file(&part);
        return Err(FileError::Other(
            "The file arrived damaged. Try again.".into(),
        ));
    }
    std::fs::rename(&part, dest).map_err(|e| FileError::Other(e.to_string()))?;
    Ok(())
}

/// Ask the server to email a password-reset code (POST /auth/forgot).
/// Ok(true): a code is on its way if that email has an account. Ok(false):
/// this server can't send email.
pub fn forgot_password(addr: &str, email: &str) -> Result<bool, String> {
    let payload = serde_json::json!({ "email": email.trim() });
    let (status, json) = post_json(addr, "/auth/forgot", None, &payload)?;
    if status == 404 || status == 405 {
        return Err("This server can't reset passwords yet. Update workboard-server, or ask the owner or an admin for a new invite code.".into());
    }
    let v: serde_json::Value = serde_json::from_str(json.trim())
        .map_err(|_| format!("The server answered {}.", status))?;
    if status != 200 {
        return Err(v
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("The server said no.")
            .to_string());
    }
    Ok(v.get("mail").and_then(|m| m.as_bool()).unwrap_or(false))
}

/// POST `payload` to `path` below the workspace of `addr`; the status and
/// the body as text.
fn post_json(
    addr: &str,
    path: &str,
    auth: Option<&Auth>,
    payload: &serde_json::Value,
) -> Result<(u16, String), String> {
    send_json("POST", addr, path, auth, payload)
}

fn send_json(
    method: &str,
    addr: &str,
    path: &str,
    auth: Option<&Auth>,
    payload: &serde_json::Value,
) -> Result<(u16, String), String> {
    let (mut s, host, prefix) = open(addr).map_err(|e| e.explain())?;
    let payload = payload.to_string();
    let auth = auth
        .map(|a| format!("Authorization: Bearer {}:{}\r\n", a.device, a.token))
        .unwrap_or_default();
    let req = format!("{} {}{} HTTP/1.1\r\nHost: {}\r\n{}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", method, prefix, path, host, auth, payload.len(), payload);
    s.write_all(req.as_bytes())
        .map_err(|e| format!("Couldn't reach the server: {}", e))?;
    let mut r = BufReader::new(s);
    let head = read_head(&mut r).map_err(|e| e.explain())?;
    let mut json = String::new();
    body(&mut r, &head)
        .take(64 << 10)
        .read_to_string(&mut json)
        .map_err(|e| format!("Couldn't reach the server: {}", e))?;
    Ok((head.status, json))
}

/// The error a server's JSON answer gives, or a plain one for `status`.
fn answer_error(status: u16, json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json.trim())
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.as_str())
                .map(|e| e.to_string())
        })
        .unwrap_or_else(|| {
            if status == 404 || status == 405 {
                "This server keeps only one workspace. Update workboard-server to keep more.".into()
            } else {
                format!("The server answered {}.", status)
            }
        })
}

/// Make a new workspace on the server `addr` is on (POST /workspaces), as
/// the person signed in with `auth` to the workspace at `addr`. Its address
/// (`workspace_addr`); the app then sets it up as an empty server, signing
/// in with that person's email.
pub fn create_workspace(addr: &str, auth: &Auth) -> Result<String, String> {
    let base = crate::server_part(addr);
    // the token is checked against every workspace on the server
    let (status, json) = post_json(&base, "/workspaces", Some(auth), &serde_json::json!({}))?;
    if status != 200 {
        return Err(answer_error(status, &json));
    }
    let id = serde_json::from_str::<serde_json::Value>(json.trim())
        .ok()
        .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(|i| i.to_string()))
        .filter(|i| !i.is_empty() && i.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .ok_or("The server's answer didn't make sense.")?;
    Ok(crate::workspace_addr(addr, &id))
}

/// Delete the workspace at `addr` from its server (DELETE /w/<id>), as its
/// owner signed in with `auth`. The server keeps it for a while
/// (`workboard-server undelete`), but everyone's apps lose it now.
pub fn delete_workspace(addr: &str, auth: &Auth) -> Result<(), String> {
    if !addr.contains("/w/") {
        return Err("The server's first workspace can't be deleted.".into());
    }
    let (status, json) = send_json("DELETE", addr, "", Some(auth), &serde_json::json!({}))?;
    match status {
        200 => Ok(()),
        // already gone
        410 => Ok(()),
        // an older server has no such address
        404 | 405 if !json.trim_start().starts_with('{') => {
            Err("This server can't delete workspaces yet. Update workboard-server first.".into())
        }
        _ => Err(answer_error(status, &json)),
    }
}

/// The workspaces an email and password sign in to on the server `addr` is
/// on (POST /auth/workspaces): (address, name). An older server that keeps
/// one workspace answers with just `addr`.
pub fn list_workspaces(
    addr: &str,
    email: &str,
    password: &str,
) -> Result<Vec<(String, String)>, String> {
    let base = crate::server_part(addr);
    let payload = serde_json::json!({ "email": email.trim(), "password": password });
    let (status, json) = post_json(&base, "/auth/workspaces", None, &payload)?;
    if status == 404 || status == 405 {
        return Ok(vec![(addr.trim().to_string(), String::new())]);
    }
    if status != 200 {
        return Err(answer_error(status, &json));
    }
    let v: serde_json::Value =
        serde_json::from_str(json.trim()).map_err(|_| "The server's answer didn't make sense.")?;
    Ok(v.get("workspaces")
        .and_then(|w| w.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|w| {
                    let id = w.get("id")?.as_str()?;
                    let name = w.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    Some((crate::workspace_addr(addr, id), name.to_string()))
                })
                .collect()
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunked_answer_reads_as_its_bytes() {
        // how a proxy in front (Cloudflare) may pass a body on
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n7;x=1\r\n, world\r\n0\r\nX-Trailer: y\r\n\r\n";
        let mut r = BufReader::new(&raw[..]);
        let head = read_head(&mut r).unwrap();
        assert!(head.chunked && head.len.is_none());
        let mut out = String::new();
        body(&mut r, &head).read_to_string(&mut out).unwrap();
        assert_eq!(out, "hello, world");

        let raw = b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 17\r\n\r\nfile is too large";
        let mut r = BufReader::new(&raw[..]);
        let head = read_head(&mut r).unwrap();
        assert_eq!(read_text(&mut r, &head), "file is too large");

        let raw = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n3\r\nab";
        let mut r = BufReader::new(&raw[..]);
        let head = read_head(&mut r).unwrap();
        let mut out = vec![];
        assert!(
            body(&mut r, &head).read_to_end(&mut out).is_err(),
            "a body that breaks off is an error, not a short file"
        );
    }

    #[test]
    fn a_name_from_synced_data_never_reaches_a_path_or_request() {
        let sha = "a".repeat(64);
        assert!(name_ok(&sha));
        let dir = std::env::temp_dir().join(format!("wb-name-{}", std::process::id()));
        for bad in [
            format!("{sha} HTTP/1.1\r\nX: y"),
            format!("../../{sha}"),
            "A".repeat(64),
            "a".repeat(63),
            String::new(),
        ] {
            assert!(!name_ok(&bad), "{bad:?}");
            let auth = Auth {
                device: "d".into(),
                token: "t".into(),
            };
            // refused before any connection or directory is made
            let dest = dir.join("x").join(&bad);
            let e = download("127.0.0.1:1", &auth, &bad, &dest, &mut |_, _| true);
            assert!(matches!(e, Err(FileError::Other(_))), "{bad:?}: {e:?}");
            assert!(!dir.exists());
            assert!(matches!(
                remote_size("127.0.0.1:1", &auth, &bad),
                Err(FileError::Other(_))
            ));
        }
    }

    #[test]
    fn an_endless_header_line_is_an_error() {
        let mut raw = b"HTTP/1.1 200 OK\r\nX: ".to_vec();
        raw.extend(std::iter::repeat_n(b'a', 100_000));
        let mut r = BufReader::new(&raw[..]);
        assert!(read_head(&mut r).is_err());
    }
}
