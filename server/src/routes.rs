// The plain HTTP pages and endpoints: /auth/forgot, /join, /privacy,
// /updates and /health.

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

/// GET /privacy: the privacy policy App Store Connect links to (TestFlight
/// external testing needs one). Plain text that doesn't change per request.
pub(crate) async fn privacy_route() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        PRIVACY_PAGE,
    )
        .into_response()
}

pub(crate) const PRIVACY_PAGE: &str = r#"<!doctype html>
<html lang="ko"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Workboard Privacy Policy</title>
<style>
:root { color-scheme: light dark; --bg: #f7f7f9; --card: #fff; --text: #1d1d22; --muted: #6b6b76; --line: #e3e3e8; }
@media (prefers-color-scheme: dark) { :root { --bg: #18181b; --card: #222226; --text: #ececf0; --muted: #a0a0aa; --line: #34343a; } }
body { margin: 0; background: var(--bg); color: var(--text); font: 15px/1.6 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
main { max-width: 680px; margin: 40px auto; padding: 0 16px; }
.card { background: var(--card); border: 1px solid var(--line); border-radius: 12px; padding: 24px; margin-bottom: 16px; }
h1 { font-size: 22px; margin: 0 0 4px; }
h2 { font-size: 16px; margin: 20px 0 4px; }
p, li { margin: 4px 0; }
.muted { color: var(--muted); font-size: 13px; }
</style></head><body><main>
<div class="card" lang="ko">
<h1>Workboard 개인정보 처리방침</h1>
<p class="muted">시행일: 2026년 11월 19일</p>
<p>Workboard 는 팀이 직접 운영하는 서버에서 작업, 채팅, 문서, 통화를 함께 쓰는 앱입니다. 이 방침은 Mac 앱과 iPhone 앱에 똑같이 적용됩니다.</p>
<h2>모으는 정보</h2>
<ul>
<li>계정: 이메일 주소와 비밀번호. 비밀번호는 원문을 저장하지 않고 PBKDF2 로 바꾼 값만 저장합니다.</li>
<li>팀에서 입력한 내용: 이름, 작업, 채팅 메시지, 문서, 올린 사진·영상·파일.</li>
<li>기기 정보: 로그인을 유지하는 기기 토큰, 알림을 보내기 위한 Apple 푸시 토큰.</li>
</ul>
<h2>쓰는 곳</h2>
<p>모은 정보는 팀원끼리 내용을 동기화하고, 로그인과 비밀번호 재설정을 처리하고, 알림을 보내는 데만 씁니다. 광고, 분석, 추적에는 쓰지 않고 판매하지 않습니다.</p>
<h2>저장 위치와 다른 회사</h2>
<ul>
<li>데이터는 팀이 운영하는 서버에 저장되고 Cloudflare 를 거쳐 암호화된 연결로 오갑니다.</li>
<li>음성·영상 통화는 LiveKit Cloud 를 거쳐 전달되며 Workboard 는 통화를 녹음하지 않습니다.</li>
<li>알림은 Apple 푸시 서비스(APNs)로, 초대·비밀번호 재설정 메일은 팀이 정한 메일 서비스로 보냅니다.</li>
</ul>
<h2>보관과 삭제</h2>
<p>정보는 팀이 서비스를 쓰는 동안 보관하고, 복구용 백업은 최대 30일 보관합니다. 계정이나 내용을 지우고 싶으면 팀 관리자(초대한 사람)에게 요청하면 접근을 끊고 지웁니다.</p>
<h2>문의</h2>
<p>개인정보에 관한 문의는 소속 팀의 Workboard 관리자에게 하거나, App Store 에 적힌 개발자 연락처로 보내 주세요.</p>
</div>
<div class="card" lang="en">
<h1>Workboard Privacy Policy</h1>
<p class="muted">Effective November 19, 2026</p>
<p>Workboard is a work app (tasks, chat, documents, calls) that a team runs on its own server. This policy covers the Mac and iPhone apps.</p>
<h2>What we collect</h2>
<ul>
<li>Account: email address and password. Passwords are never stored as typed, only as a PBKDF2 hash.</li>
<li>What your team enters: names, tasks, chat messages, documents, and uploaded pictures, videos and files.</li>
<li>Device data: a device token that keeps you signed in, and an Apple push token for notifications.</li>
</ul>
<h2>How it is used</h2>
<p>Only to sync your team's work, sign you in, reset passwords and send notifications. Nothing is used for ads, analytics or tracking, and nothing is sold.</p>
<h2>Where it goes</h2>
<ul>
<li>Data is stored on the server your team runs and travels over encrypted connections through Cloudflare.</li>
<li>Voice and video calls are relayed through LiveKit Cloud. Workboard does not record calls.</li>
<li>Notifications go through Apple Push Notification service; invite and password-reset mails go through the team's mail provider.</li>
</ul>
<h2>Keeping and deleting</h2>
<p>Data is kept while your team uses the service; recovery backups are kept for up to 30 days. To have your account or content removed, ask your team's administrator (the person who invited you).</p>
<h2>Contact</h2>
<p>Ask your team's Workboard administrator, or write to the developer contact shown on the App Store.</p>
</div>
</main></body></html>
"#;

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
    let g = hub.state();
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
