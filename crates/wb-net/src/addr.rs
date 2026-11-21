// Server addresses as people type them (host, port, /w/<id> workspace),
// the web addresses made from them, and invite links.

use crate::{DEFAULT_PORT, PATH};

/// What a person typed into an invite code field: a bare code, or a
/// `workboard://join?...` link (also inside a whole pasted email).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JoinLink {
    pub server: Option<String>,
    pub code: String,
    pub email: Option<String>,
}

/// The link that opens Workboard ready to join.
pub fn join_link(server: &str, code: &str, email: Option<&str>) -> String {
    let mut s = format!(
        "workboard://join?server={}&code={}",
        pct_encode(server),
        pct_encode(code)
    );
    if let Some(e) = email.filter(|e| !e.is_empty()) {
        s.push_str("&email=");
        s.push_str(&pct_encode(e));
    }
    s
}

/// Read a pasted invite: the first workboard://join link in the text, else
/// the text itself as a code.
pub fn parse_join(text: &str) -> JoinLink {
    let t = text.trim();
    let Some(start) = t.find("workboard://join") else {
        return JoinLink {
            code: t.to_string(),
            ..Default::default()
        };
    };
    let link: String = t[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && !"<>\"'".contains(*c))
        .collect();
    let mut out = JoinLink::default();
    if let Some((_, query)) = link.split_once('?') {
        for pair in query.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let v = pct_decode(v);
            match k {
                "server" if !v.trim().is_empty() => out.server = Some(v.trim().to_string()),
                "code" => out.code = v.trim().to_string(),
                "email" if !v.trim().is_empty() => out.email = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    out
}

/// Percent-encode for a URL query value: everything but letters, digits
/// and `-._~` becomes %XX.
pub fn pct_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect()
}

/// Undo `pct_encode` (and form encoding): %XX becomes its byte, + a
/// space; a % not followed by two hex digits stays as it is.
pub fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The web address of `path` ("/hooks/<token>") on the server (or the
/// server's workspace) the address a person typed points to.
pub fn http_address(addr: &str, path: &str) -> Option<String> {
    let (_, _, url) = server_url(addr).ok()?;
    let url = url
        .strip_prefix("wss://")
        .map(|r| format!("https://{}", r))
        .or_else(|| url.strip_prefix("ws://").map(|r| format!("http://{}", r)))?;
    let base = url.strip_suffix(PATH).unwrap_or(&url);
    Some(format!("{}{}", base, path))
}

/// Turn what a person typed ("macmini", "100.1.2.3", "host:9000",
/// "ws://host:8787/ws", "https://wb.example.com") into (host, port, url).
/// A wss:// or https:// address is a secure connection (port 443 unless
/// it says otherwise), for a server behind an HTTPS front such as
/// Cloudflare Tunnel; anything else is plain ws on port 8787 by default.
pub fn server_url(addr: &str) -> Result<(String, u16, String), String> {
    let a = addr.trim().trim_end_matches('/');
    if a.is_empty() {
        return Err("Type the server's address first.".into());
    }
    let lower = a.to_ascii_lowercase();
    let (secure, rest) = if lower.starts_with("wss://") {
        (true, &a[6..])
    } else if lower.starts_with("https://") {
        (true, &a[8..])
    } else if lower.starts_with("ws://") {
        (false, &a[5..])
    } else if lower.starts_with("http://") {
        (false, &a[7..])
    } else {
        (false, a)
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, PATH),
    };
    // a workspace other than the server's first: its connection is below it
    let path = match workspace_id(path) {
        Some(id) => format!("/w/{}{}", id, PATH),
        None => path.to_string(),
    };
    let (host, port) = if let Some(h) = hostport.strip_prefix('[') {
        // [ipv6]:port
        let end = h.find(']').ok_or("That address doesn't look right.")?;
        let port = h[end + 1..]
            .strip_prefix(':')
            .map(|p| p.parse::<u16>())
            .transpose()
            .map_err(|_| "That port isn't a number.")?;
        (h[..end].to_string(), port)
    } else {
        match hostport.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (
                h.to_string(),
                Some(p.parse::<u16>().map_err(|_| "That port isn't a number.")?),
            ),
            _ => (hostport.to_string(), None),
        }
    };
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err("That address doesn't look right.".into());
    }
    let port = port.unwrap_or(if secure { 443 } else { DEFAULT_PORT });
    let base = origin(&host, port, secure);
    let scheme = if secure { "wss" } else { "ws" };
    Ok((host, port, format!("{}://{}{}", scheme, base, path)))
}

/// The workspace id in an address's path ("/w/<id>" or "/w/<id>/ws"); None
/// for the server's first workspace.
fn workspace_id(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/w/")?;
    let id = rest
        .strip_suffix(PATH)
        .unwrap_or(rest)
        .trim_end_matches('/');
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

/// What paths of a workspace's address start with on the server: "" for
/// the server's first workspace, "/w/<id>" for the others (see the
/// server's registry.rs).
pub fn workspace_prefix(addr: &str) -> String {
    let a = addr.trim().trim_end_matches('/');
    let rest = a.find("://").map(|i| &a[i + 3..]).unwrap_or(a);
    match rest.find('/').and_then(|i| workspace_id(&rest[i..])) {
        Some(id) => format!("/w/{}", id),
        None => String::new(),
    }
}

/// The server part of an address, as typed, without any path.
pub fn server_part(addr: &str) -> String {
    let a = addr.trim().trim_end_matches('/');
    let start = a.find("://").map(|i| i + 3).unwrap_or(0);
    match a[start..].find('/') {
        Some(i) => a[..start + i].to_string(),
        None => a.to_string(),
    }
}

/// The address of workspace `id` on the server `addr` is on ("" is the
/// server's first workspace).
pub fn workspace_addr(addr: &str, id: &str) -> String {
    let base = server_part(addr);
    if id.is_empty() {
        base
    } else {
        format!("{}/w/{}", base, id)
    }
}

/// Whether two addresses reach the same workspace ("macmini" and
/// "ws://macmini:8787/ws" do).
pub fn same_workspace(a: &str, b: &str) -> bool {
    match (server_url(a), server_url(b)) {
        (Ok(x), Ok(y)) => x.2.eq_ignore_ascii_case(&y.2),
        _ => false,
    }
}

/// host[:port] as it goes into a URL or a Host header: IPv6 in brackets,
/// the port left out when it is the secure default.
fn origin(host: &str, port: u16, secure: bool) -> String {
    let shown = if host.contains(':') {
        format!("[{}]", host)
    } else {
        host.to_string()
    };
    if secure && port == 443 {
        shown
    } else {
        format!("{}:{}", shown, port)
    }
}

/// Whether a server_url() url is a secure (TLS) one.
pub fn is_secure(url: &str) -> bool {
    url.starts_with("wss://")
}

/// Where the server publishes app updates for Sparkle (see the server's
/// /updates route): the same host and port as the sync connection, over
/// https when that is secure.
pub fn updates_url(addr: &str) -> Result<String, String> {
    let (host, port, url) = server_url(addr)?;
    let secure = is_secure(&url);
    Ok(format!(
        "{}://{}/updates/appcast.xml",
        if secure { "https" } else { "http" },
        origin(&host, port, secure)
    ))
}
