// Moving pictures and videos to and from the server's /files route (see
// server/src/files.rs): plain HTTP/1.1 over a TcpStream, one request per
// connection, so there is no extra client library. Every call blocks; the
// app runs them on its own thread and reports progress through a callback.

use crate::server_url;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

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
            FileError::TooLarge => "The file is larger than the server takes (500 MB).".into(),
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
}

fn open(addr: &str) -> Result<(TcpStream, String), FileError> {
    let (host, port, _) = server_url(addr).map_err(FileError::Other)?;
    let shown = if host.contains(':') {
        format!("[{}]", host)
    } else {
        host.clone()
    };
    let sock = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| FileError::Net(e.to_string()))?
        .next()
        .ok_or_else(|| FileError::Net("no address".into()))?;
    let s = TcpStream::connect_timeout(&sock, Duration::from_secs(10))
        .map_err(|e| FileError::Net(e.to_string()))?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(60)));
    let _ = s.set_nodelay(true);
    Ok((s, format!("{}:{}", shown, port)))
}

fn request(
    s: &mut TcpStream,
    method: &str,
    host: &str,
    auth: &Auth,
    sha: &str,
    extra: &[(&str, String)],
) -> Result<(), FileError> {
    let mut r = format!(
        "{} /files/{} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}:{}\r\nConnection: close\r\n",
        method, sha, host, auth.device, auth.token
    );
    for (k, v) in extra {
        r.push_str(&format!("{}: {}\r\n", k, v));
    }
    r.push_str("\r\n");
    s.write_all(r.as_bytes())
        .map_err(|e| FileError::Net(e.to_string()))
}

fn read_head(r: &mut impl BufRead) -> Result<Head, FileError> {
    let mut line = String::new();
    r.read_line(&mut line)
        .map_err(|e| FileError::Net(e.to_string()))?;
    let status = line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| FileError::Net("the server's answer didn't make sense".into()))?;
    let mut head = Head {
        status,
        len: None,
        range_total: None,
    };
    loop {
        line.clear();
        if r.read_line(&mut line)
            .map_err(|e| FileError::Net(e.to_string()))?
            == 0
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
    let mut body = String::new();
    let _ = r
        .take(head.len.unwrap_or(0).min(4096))
        .read_to_string(&mut body);
    body
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
    let (mut s, host) = open(addr)?;
    request(&mut s, "HEAD", &host, auth, sha, &[])?;
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
    let mut f = File::open(path).map_err(|e| FileError::Other(e.to_string()))?;
    let total = f
        .metadata()
        .map_err(|e| FileError::Other(e.to_string()))?
        .len();
    if remote_size(addr, auth, sha)? == Some(total) {
        progress(total, total);
        return Ok(());
    }
    let (mut s, host) = open(addr)?;
    request(
        &mut s,
        "PUT",
        &host,
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
    let part = dest.with_extension("part");
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).map_err(|e| FileError::Other(e.to_string()))?;
    }
    let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let (mut s, host) = open(addr)?;
    let extra: Vec<(&str, String)> = if have > 0 {
        vec![("Range", format!("bytes={}-", have))]
    } else {
        vec![]
    };
    request(&mut s, "GET", &host, auth, sha, &extra)?;
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
    while got < want {
        let k = r
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
    let (mut s, host) = open(addr).map_err(|e| e.explain())?;
    let body = serde_json::json!({ "email": email.trim() }).to_string();
    let req = format!("POST /auth/forgot HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", host, body.len(), body);
    s.write_all(req.as_bytes())
        .map_err(|e| format!("Couldn't reach the server: {}", e))?;
    let mut text = String::new();
    s.read_to_string(&mut text)
        .map_err(|e| format!("Couldn't reach the server: {}", e))?;
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    if status == 404 || status == 405 {
        return Err("This server can't reset passwords yet. Update workboard-server, or ask the owner or an admin for a new invite code.".into());
    }
    let json = text.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
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
