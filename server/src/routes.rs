// The plain HTTP pages and endpoints: /auth/forgot, /join, /updates and
// /health.

use super::*;

/// POST /auth/forgot {"email"}: email a password-reset code. Always "ok", so
/// it doesn't tell who has an account; "mail": false when mail isn't set up.
pub(crate) async fn forgot_route(State(hub): State<Arc<Hub>>, body: axum::body::Bytes) -> Response {
    let email = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| {
            v.get("email")
                .and_then(|e| e.as_str())
                .map(|e| e.to_string())
        })
        .unwrap_or_default();
    let h = hub.clone();
    let mail = tokio::task::spawn_blocking(move || h.forgot(&email))
        .await
        .unwrap_or(false);
    Json(json!({"ok": true, "mail": mail})).into_response()
}

pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Whether a request came through a proxy in front of the server
/// (Cloudflare Tunnel says who asked in these headers; a request made
/// straight to the server has neither).
pub(crate) fn through_proxy(h: &axum::http::HeaderMap) -> bool {
    h.contains_key("cf-connecting-ip") || h.contains_key("x-forwarded-for")
}

/// The server's address as the asking app or browser typed it: the Host
/// header, as an https address when a front in between says the request
/// came over https (X-Forwarded-Proto), since an app then needs the
/// secure address to get here.
pub(crate) fn reached_at(h: &axum::http::HeaderMap) -> Option<String> {
    let host = h
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())?
        .trim()
        .to_string();
    let https = h
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("https"));
    Some(if https {
        format!("https://{}", host.strip_suffix(":443").unwrap_or(&host))
    } else {
        host
    })
}

/// GET /join?code=...&email=...: the page an invite mail links to. It opens
/// the app with everything filled in, and shows what to type otherwise.
pub(crate) async fn join_route(
    State(hub): State<Arc<Hub>>,
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let host = reached_at(&headers)
        .map(|h| format!("{}{}", h, hub.prefix()))
        .unwrap_or_default();
    let code = q
        .get("code")
        .map(|c| c.trim().to_string())
        .unwrap_or_default();
    let email = q
        .get("email")
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty());
    let link = wb_net::join_link(&host, &code, email.as_deref());
    let page = JOIN_PAGE
        .replace("{link}", &html_escape(&link))
        .replace("{server}", &html_escape(&host))
        .replace(
            "{code}",
            &html_escape(if code.is_empty() {
                "(ask for your invite code)"
            } else {
                &code
            }),
        );
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        page,
    )
        .into_response()
}

pub(crate) const JOIN_PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Join your team on Workboard</title>
<style>
:root { color-scheme: light dark; --bg: #f7f7f9; --card: #fff; --text: #1d1d22; --muted: #6b6b76; --line: #e3e3e8; --primary: #5f55ee; }
@media (prefers-color-scheme: dark) { :root { --bg: #18181b; --card: #222226; --text: #ececf0; --muted: #a0a0aa; --line: #34343a; } }
body { margin: 0; background: var(--bg); color: var(--text); font: 15px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
main { max-width: 440px; margin: 48px auto; padding: 0 16px; }
.card { background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 24px; }
h1 { font-size: 20px; margin: 0 0 8px; }
p { margin: 0 0 16px; color: var(--muted); }
a.button { display: block; text-align: center; background: var(--primary); color: #fff; text-decoration: none; padding: 11px; border-radius: 8px; font-weight: 600; }
dl { margin: 20px 0 0; display: grid; grid-template-columns: auto 1fr; gap: 6px 14px; }
dt { color: var(--muted); }
dd { margin: 0; font-family: ui-monospace, Menlo, monospace; word-break: break-all; }
</style></head>
<body><main><div class="card">
<h1>Join your team on Workboard</h1>
<p>Open the app with your invite filled in. You'll choose a password, and then sign in with your email on your other computers and phones.</p>
<a class="button" href="{link}">Open in Workboard</a>
<p style="margin-top:20px">Nothing happened? Install Workboard first, or open it, choose <b>Join my team</b> and type:</p>
<dl><dt>Server</dt><dd>{server}</dd><dt>Invite code</dt><dd>{code}</dd></dl>
</div></main></body></html>
"#;

/// The content type for a file the updates route may serve: a plain name
/// (no folders, not hidden) of a kind Sparkle asks for. None for anything else.
pub fn update_content_type(name: &str) -> Option<&'static str> {
    if name.is_empty()
        || name.starts_with('.')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
    {
        return None;
    }
    Some(match name.rsplit_once('.')?.1 {
        "xml" => "application/xml",
        "zip" => "application/zip",
        "delta" => "application/octet-stream",
        "html" => "text/html; charset=utf-8",
        _ => return None,
    })
}

/// App updates for Sparkle: GET /updates/appcast.xml and the files it lists.
pub(crate) async fn update_route(
    axum::extract::Path(name): axum::extract::Path<String>,
    State(hub): State<Arc<Hub>>,
) -> Response {
    let Some(kind) = update_content_type(&name) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // streamed, not read whole: anyone may ask for the app (tens of MB)
    // many times at once
    let path = hub.home.join(UPDATES_DIR).join(&name);
    let (Ok(f), Ok(meta)) = (
        tokio::fs::File::open(&path).await,
        tokio::fs::metadata(&path).await,
    ) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !meta.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let stream = tokio_util::io::ReaderStream::with_capacity(f, 256 << 10);
    (
        [
            (header::CONTENT_TYPE, kind.to_string()),
            (header::CACHE_CONTROL, "no-cache".to_string()),
            (header::CONTENT_LENGTH, meta.len().to_string()),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}

pub(crate) async fn health(
    State(hub): State<Arc<Hub>>,
    headers: axum::http::HeaderMap,
) -> Response {
    // from the internet (through the tunnel): only that it is up
    if through_proxy(&headers) {
        return Json(json!({ "ok": true })).into_response();
    }
    let g = hub.inner.lock().unwrap();
    let backups = backup_files(&hub.home.join(BACKUP_DIR), "hourly")
        .last()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
    Json(json!({
        "ok": true,
        "rev": g.saved.rev,
        "workspace": g.canon.is_some(),
        "devices": g.saved.devices.len(),
        "connections": hub.connections.load(Ordering::Relaxed),
        "latest_backup": backups,
    }))
    .into_response()
}
