// Several workspaces on one server.
//
// The server folder itself holds the first workspace, as it always has
// (workspace.json, server.json, backups/, files/ ...), and apps reach it at
// the server's address. Every other workspace lives in its own folder,
// workspaces/<id>/, with the same files, and apps reach it below
// /w/<id> (/w/<id>/ws, /w/<id>/files/<sha>, /w/<id>/auth/forgot,
// /w/<id>/join). So a workspace's address is the server's address plus
// /w/<id>, and invite links, saved addresses and tokens need nothing else.
// Each workspace has its own hub: its own people, accounts, roles, backups
// and files. Nothing is shared between them but the setup files in the
// server folder (mail.json, apns.json, livekit.json), used by a workspace
// that has none of its own.
//
//   POST /workspaces        Authorization: Bearer <device>:<token>, a device
//                           of an owner or admin of the first workspace.
//                           -> {"id"}: a new, empty workspace. Its owner is
//                           whoever claims it with the account (email) of
//                           the person who made it; they keep the password
//                           they already have.
//   POST /auth/workspaces   {"email", "password"} -> {"workspaces": [{"id",
//                           "name"}]}: every workspace that account signs in
//                           to ("" is the first one). Wrong tries count as
//                           they do when signing in.

use super::*;
use axum::extract::Path as UrlPath;

/// The most workspaces a server keeps besides its first one.
pub const MAX_WORKSPACES: usize = 50;
/// Where the other workspaces live in the server folder.
pub const WORKSPACES_DIR: &str = "workspaces";
/// A made workspace nobody set up is removed after this long.
const UNCLAIMED_SECS: u64 = 24 * 3600;

/// A workspace id: what POST /workspaces makes (16 lowercase hex digits),
/// or a folder someone made by hand of letters, digits and dashes.
pub fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub(crate) struct Registry {
    root: PathBuf,
    first: Arc<Hub>,
    others: Mutex<BTreeMap<String, Arc<Hub>>>,
    options: Options,
    pusher: Option<Arc<dyn push::Sender>>,
    stop: watch::Receiver<bool>,
}

impl Registry {
    /// The first workspace in `root` and every one in root/workspaces.
    pub(crate) fn open(
        root: PathBuf,
        options: Options,
        stop: watch::Receiver<bool>,
    ) -> std::io::Result<Registry> {
        std::fs::create_dir_all(&root)?;
        let pusher = options.push.clone().or_else(|| apns_sender(&root));
        let first = Arc::new(Hub::open(
            root.clone(),
            root.clone(),
            None,
            &options,
            pusher.clone(),
            stop.clone(),
        )?);
        let mut others = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(root.join(WORKSPACES_DIR)) {
            for e in rd.flatten() {
                let Some(id) = e.file_name().to_str().map(|s| s.to_string()) else {
                    continue;
                };
                if !id_ok(&id) || !e.path().is_dir() {
                    continue;
                }
                match Hub::open(
                    e.path(),
                    root.clone(),
                    Some(id.clone()),
                    &options,
                    pusher.clone(),
                    stop.clone(),
                ) {
                    Ok(h) => {
                        others.insert(id, Arc::new(h));
                    }
                    Err(err) => eprintln!("couldn't open workspace {}: {}", id, err),
                }
            }
        }
        Ok(Registry {
            root,
            first,
            others: Mutex::new(others),
            options,
            pusher,
            stop,
        })
    }

    pub(crate) fn first(&self) -> Arc<Hub> {
        self.first.clone()
    }

    fn hub(&self, id: &str) -> Option<Arc<Hub>> {
        self.others.lock().unwrap().get(id).cloned()
    }

    /// Every workspace, the first one first.
    pub(crate) fn all(&self) -> Vec<Arc<Hub>> {
        let mut v = vec![self.first.clone()];
        v.extend(self.others.lock().unwrap().values().cloned());
        v
    }

    /// The account of the person signed in on the device a request names
    /// (`Bearer <device>:<token>`): an owner or admin of the server's first
    /// workspace, the team that keeps the server. New workspaces are theirs
    /// to make, so a member can't fill the server with them.
    fn requester(&self, headers: &axum::http::HeaderMap) -> Result<Account, (StatusCode, String)> {
        let unknown = (
            StatusCode::UNAUTHORIZED,
            "Sign in to a workspace on this server first.".to_string(),
        );
        let (device, token) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .and_then(|v| v.split_once(':'))
            .ok_or(unknown.clone())?;
        let g = self.first.inner.lock().unwrap();
        let signed_in = g
            .saved
            .devices
            .get(device)
            .filter(|d| same(&d.token, &hash_token(token)));
        let Some(d) = signed_in else {
            // a device of another workspace here: signed in, but not of this team
            let elsewhere = self.others.lock().unwrap().values().any(|h| {
                h.inner
                    .lock()
                    .unwrap()
                    .saved
                    .devices
                    .get(device)
                    .is_some_and(|d| same(&d.token, &hash_token(token)))
            });
            return Err(if elsewhere { not_allowed() } else { unknown });
        };
        if !g.saved.role_of(d.member).manages() {
            return Err(not_allowed());
        }
        g.saved.accounts.get(&d.member).cloned().ok_or((
            StatusCode::FORBIDDEN,
            "Choose a password for your account first (Work together).".to_string(),
        ))
    }

    /// A new, empty workspace that `owner`'s account sets up.
    fn create(&self, owner: Account) -> Result<String, String> {
        let mut others = self.others.lock().unwrap();
        if others.len() >= MAX_WORKSPACES {
            return Err(format!(
                "This server already keeps {} workspaces, the most it can.",
                MAX_WORKSPACES + 1
            ));
        }
        let id = loop {
            let id = hex(&random_bytes::<8>());
            if !others.contains_key(&id) {
                break id;
            }
        };
        let home = self.root.join(WORKSPACES_DIR).join(&id);
        let hub = Hub::open(
            home,
            self.root.clone(),
            Some(id.clone()),
            &self.options,
            self.pusher.clone(),
            self.stop.clone(),
        )
        .map_err(|e| format!("Couldn't make the workspace: {}", e))?;
        {
            let mut g = hub.inner.lock().unwrap();
            g.saved.seed = Some(owner);
            g.dirty = true;
        }
        hub.flush();
        others.insert(id.clone(), Arc::new(hub));
        Ok(id)
    }

    /// The workspaces `email` and `password` sign in to: (id, name), "" for
    /// the first. Each workspace counts a wrong try as signing in does.
    fn signs_in_to(&self, email: &str, password: &str) -> Result<Vec<(String, String)>, String> {
        let hubs: Vec<Arc<Hub>> = self
            .all()
            .into_iter()
            .filter(|h| {
                let g = h.inner.lock().unwrap();
                g.canon.is_some() && g.saved.account_member(email).is_some()
            })
            .collect();
        if hubs.is_empty() {
            // counts and takes as long as a real check
            return match self.first.check_password(email, password) {
                Ok(_) => Ok(vec![]),
                Err(e) => Err(e),
            };
        }
        let mut found = vec![];
        let mut why = None;
        for hub in hubs {
            match hub.check_password(email, password) {
                Ok((g, _)) => {
                    drop(g);
                    found.push((hub.id.clone().unwrap_or_default(), hub.name()));
                }
                Err(e) => why = Some(e),
            }
        }
        match (found.is_empty(), why) {
            (true, Some(e)) => Err(e),
            _ => Ok(found),
        }
    }

    /// Remove workspaces made with POST /workspaces that nobody set up within
    /// a day. Returns their ids.
    pub(crate) fn clean_up(&self) -> Vec<String> {
        let mut others = self.others.lock().unwrap();
        let unclaimed: Vec<String> = others
            .iter()
            .filter(|(_, h)| {
                let g = h.inner.lock().unwrap();
                let empty = g.canon.is_none() && g.saved.devices.is_empty() && g.saved.rev == 0;
                let old = std::fs::metadata(h.home.join(STATE_FILE))
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age.as_secs() >= UNCLAIMED_SECS);
                empty && old && h.connections.load(Ordering::Relaxed) == 0
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &unclaimed {
            if let Some(h) = others.remove(id) {
                let _ = std::fs::remove_dir_all(&h.home);
            }
        }
        unclaimed
    }
}

fn not_allowed() -> (StatusCode, String) {
    (
        StatusCode::FORBIDDEN,
        "Only the owner or an admin of this server's first workspace can make a new workspace on it.".to_string(),
    )
}

fn no_workspace() -> Response {
    (
        StatusCode::NOT_FOUND,
        "There's no such workspace on this server.",
    )
        .into_response()
}

async fn create_route(State(r): State<Arc<Registry>>, headers: axum::http::HeaderMap) -> Response {
    let owner = match r.requester(&headers) {
        Ok(a) => a,
        Err((code, why)) => return (code, Json(json!({ "error": why }))).into_response(),
    };
    let r2 = r.clone();
    match tokio::task::spawn_blocking(move || r2.create(owner)).await {
        Ok(Ok(id)) => {
            println!("made workspace {}", id);
            Json(json!({ "id": id })).into_response()
        }
        Ok(Err(why)) => (StatusCode::CONFLICT, Json(json!({ "error": why }))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn list_route(State(r): State<Arc<Registry>>, body: axum::body::Bytes) -> Response {
    let v: Value = serde_json::from_slice(&body).unwrap_or_default();
    let field = |k: &str| v.get(k).and_then(|e| e.as_str()).unwrap_or("").to_string();
    let (email, password) = (field("email"), field("password"));
    if email.trim().is_empty() || password.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Type your email and password." })),
        )
            .into_response();
    }
    // password hashing takes a moment
    match tokio::task::spawn_blocking(move || r.signs_in_to(&email, &password)).await {
        Ok(Ok(list)) => Json(json!({
            "workspaces": list
                .into_iter()
                .map(|(id, name)| json!({ "id": id, "name": name }))
                .collect::<Vec<_>>()
        }))
        .into_response(),
        Ok(Err(why)) => (StatusCode::UNAUTHORIZED, Json(json!({ "error": why }))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn w_ws(
    State(r): State<Arc<Registry>>,
    UrlPath(id): UrlPath<String>,
    req: axum::extract::Request,
) -> Response {
    match r.hub(&id) {
        Some(h) => ws_route(State(h), req).await,
        None => no_workspace(),
    }
}

async fn w_get_file(
    State(r): State<Arc<Registry>>,
    UrlPath((id, file)): UrlPath<(String, String)>,
    method: Method,
    headers: axum::http::HeaderMap,
) -> Response {
    match r.hub(&id) {
        Some(h) => files::get_file(UrlPath(file), State(h), method, headers).await,
        None => no_workspace(),
    }
}

async fn w_put_file(
    State(r): State<Arc<Registry>>,
    UrlPath((id, file)): UrlPath<(String, String)>,
    req: axum::extract::Request,
) -> Response {
    match r.hub(&id) {
        Some(h) => files::put_file(UrlPath(file), State(h), req).await,
        None => no_workspace(),
    }
}

async fn w_forgot(
    State(r): State<Arc<Registry>>,
    UrlPath(id): UrlPath<String>,
    body: axum::body::Bytes,
) -> Response {
    match r.hub(&id) {
        Some(h) => forgot_route(State(h), body).await,
        None => no_workspace(),
    }
}

async fn w_join(
    State(r): State<Arc<Registry>>,
    UrlPath(id): UrlPath<String>,
    q: axum::extract::Query<std::collections::HashMap<String, String>>,
    headers: axum::http::HeaderMap,
) -> Response {
    match r.hub(&id) {
        Some(h) => join_route(State(h), q, headers).await,
        None => no_workspace(),
    }
}

async fn w_health(
    State(r): State<Arc<Registry>>,
    UrlPath(id): UrlPath<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    match r.hub(&id) {
        Some(h) => health(State(h), headers).await,
        None => no_workspace(),
    }
}

/// The routes for workspaces other than the first, and for making them.
pub(crate) fn routes(r: Arc<Registry>) -> Router {
    Router::new()
        .route("/workspaces", axum::routing::post(create_route))
        .route("/auth/workspaces", axum::routing::post(list_route))
        .route("/w/{id}/ws", get(w_ws))
        .route("/w/{id}/files/{file}", get(w_get_file).put(w_put_file))
        .route("/w/{id}/auth/forgot", axum::routing::post(w_forgot))
        .route("/w/{id}/join", get(w_join))
        .route("/w/{id}/health", get(w_health))
        .with_state(r)
}
