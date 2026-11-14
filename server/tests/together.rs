// The server with real connections: a server on a free local port with its
// data in a temporary folder, and "apps" that hold a workspace, note their
// changes the way the app does (wb_sync::stamp) and talk through the same
// client the app uses (wb_net::Client).

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wb_net::{Client, Event, Msg, Person, Role, SignIn, Status};
use wb_sync::SyncMeta;

struct Server {
    port: u16,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    fn start(home: &PathBuf, port: u16) -> Server {
        let home = home.clone();
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                // the same port again after a restart (it can take a moment to free up)
                let mut tries = 0;
                let listener = loop {
                    match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
                        Ok(l) => break l,
                        Err(e) if tries < 50 => {
                            tries += 1;
                            let _ = e;
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        Err(e) => panic!("bind: {}", e),
                    }
                };
                port_tx.send(listener.local_addr().unwrap().port()).unwrap();
                workboard_server::serve(listener, home, async {
                    let _ = stop_rx.await;
                })
                .await
                .unwrap();
            });
        });
        let port = port_rx.recv_timeout(Duration::from_secs(10)).expect("server started");
        Server { port, stop: Some(stop_tx), thread: Some(thread) }
    }

    fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.halt();
    }
}

fn temp_home(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("wb-server-{}-{:x}", tag, wb_sync::random_u64()))
}

/// Milliseconds, like the app's change stamps.
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

fn workspace() -> Value {
    json!({
        "workspace": "Team", "spaces": [], "members": [{"id": 1, "name": "Ann", "role": "Owner"}, {"id": 2, "name": "Bo", "role": "Member"}, {"id": 3, "name": "Cy", "role": "Member"}], "me": 1, "next_id": 1, "favorites": [], "docs": [],
        "tasks": [{"id": 100, "name": "Plan", "activity": []}, {"id": 101, "name": "Ship", "activity": []}],
        "channels": [{"id": 10, "name": "general", "kind": "Channel", "topic": "", "members": [], "messages": [{"id": 11, "author": 1, "text": "hi", "ts": 1}], "last_read": 0}],
        "extra": {"sync": {}}
    })
}

/// One app: its own workspace copy and connection.
struct App {
    data: Value,
    client: Client,
    device: String,
    status: Option<Status>,
    member: Option<u64>,
    role: Option<Role>,
    token: Option<String>,
    invites: Vec<(u64, String)>,
    people: Vec<Person>,
    denied: Vec<String>,
}

impl App {
    /// The owner setting up an empty server with their workspace.
    fn connect(port: u16, data: Option<Value>) -> App {
        let mut d = data.unwrap();
        let mut meta = SyncMeta::default();
        wb_sync::stamp(&d, &mut meta, now());
        d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        let me = d["me"].as_u64();
        Self::start(port, d, SignIn { claim: me, ..Default::default() })
    }

    /// Someone joining with an invite code: they take whatever the server has.
    fn join(port: u16, code: &str) -> App {
        Self::start(port, Value::Null, SignIn { invite: Some(code.to_string()), ..Default::default() })
    }

    fn start(port: u16, data: Value, mut sign_in: SignIn) -> App {
        if sign_in.device.is_empty() {
            sign_in.device = format!("dev{:x}", wb_sync::random_u64());
        }
        let device = sign_in.device.clone();
        let client = Client::start(&format!("127.0.0.1:{}", port), sign_in, Arc::new(|| {}));
        App { data, client, device, status: None, member: None, role: None, token: None, invites: vec![], people: vec![], denied: vec![] }
    }

    /// Ask for an invite code for `member` and wait for it.
    fn invite(&mut self, member: u64, role: Role) -> String {
        self.client.send(Msg::CreateInvite { member, role });
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            self.pump();
            if let Some(i) = self.invites.iter().position(|(m, _)| *m == member) {
                return self.invites.remove(i).1;
            }
            if let Some(d) = self.denied.pop() {
                panic!("invite denied: {}", d);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("no invite code came back");
    }

    /// Wait until `cond` holds for this app's connection details.
    fn until(&mut self, what: &str, cond: impl Fn(&App) -> bool) {
        let until = Instant::now() + Duration::from_secs(15);
        while Instant::now() < until {
            self.pump();
            if cond(self) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for: {} (status {:?}, role {:?}, denied {:?})", what, self.status, self.role, self.denied);
    }

    fn meta(&self) -> SyncMeta {
        serde_json::from_value(self.data["extra"]["sync"].clone()).unwrap_or_default()
    }

    fn push(&self) {
        if !self.data.is_null() {
            self.client.push(wb_sync::shareable(&self.data));
        }
    }

    /// Edit, note the change, send it.
    fn change(&mut self, f: impl FnOnce(&mut Value)) {
        f(&mut self.data);
        let mut meta = self.meta();
        wb_sync::stamp(&self.data, &mut meta, now());
        self.data["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        self.push();
    }

    fn take_in(&mut self, other: Value) {
        if self.data.is_null() {
            // joining: start from the team's copy
            self.data = other;
            let mut meta = self.meta();
            meta.hashes = wb_sync::fingerprints(&self.data);
            self.data["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        } else {
            wb_sync::merge(&mut self.data, &[other]);
        }
    }

    fn pump(&mut self) {
        for e in self.client.events() {
            match e {
                Event::Status(s) => self.status = Some(s),
                Event::Welcome { snapshot, member, role, token, .. } => {
                    self.member = Some(member);
                    self.role = Some(role);
                    if token.is_some() {
                        self.token = token;
                    }
                    if let Some(s) = snapshot {
                        self.take_in(s);
                    }
                    // after (re)connecting the whole copy goes up
                    self.push();
                }
                Event::Update { data, .. } => self.take_in(data),
                Event::Invite { member, code } => self.invites.push((member, code)),
                Event::Access { people, role } => {
                    self.people = people;
                    self.role = Some(role);
                }
                Event::Denied(why) => self.denied.push(why),
                Event::CallToken { .. } => {}
            }
        }
    }

    fn wait(&mut self, what: &str, cond: impl Fn(&Value) -> bool) {
        let until = Instant::now() + Duration::from_secs(15);
        while Instant::now() < until {
            self.pump();
            if !self.data.is_null() && cond(&self.data) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for: {} (status {:?})\n{}", what, self.status, self.data);
    }

    fn wait_status(&mut self, want: fn(&Status) -> bool, what: &str) {
        let until = Instant::now() + Duration::from_secs(15);
        while Instant::now() < until {
            self.pump();
            if self.status.as_ref().is_some_and(want) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for status {} (have {:?})", what, self.status);
    }
}

fn texts(d: &Value) -> Vec<String> {
    d["channels"][0]["messages"].as_array().map(|a| a.iter().map(|m| m["text"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default()
}

fn task_names(d: &Value) -> Vec<String> {
    d["tasks"].as_array().map(|a| a.iter().map(|t| t["name"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default()
}

fn say(text: &str, id: u64) -> impl FnOnce(&mut Value) + '_ {
    move |d: &mut Value| {
        d["channels"][0]["messages"].as_array_mut().unwrap().push(json!({"id": id, "author": 1, "text": text, "ts": now()}));
    }
}

/// A sets up an empty server with a workspace, invites B, B joins.
fn two_apps(port: u16) -> (App, App) {
    let mut a = App::connect(port, Some(workspace()));
    a.wait("A's copy on the server", |_| true);
    a.until("A is the owner", |a| a.role == Some(Role::Owner));
    let code = a.invite(2, Role::Member);
    let mut b = App::join(port, &code);
    b.wait("B gets the team's copy", |d| task_names(d) == ["Plan", "Ship"]);
    assert_eq!(b.member, Some(2), "the code signs B in as member 2");
    (a, b)
}

#[test]
fn a_chat_message_from_a_reaches_b() {
    let home = temp_home("chat");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(say("hello from A", 2001));
    b.wait("A's message at B", |d| texts(d).contains(&"hello from A".to_string()));
    b.change(say("and from B", 3001));
    a.wait("B's answer at A", |d| texts(d).ends_with(&["hello from A".to_string(), "and from B".to_string()]));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn edits_to_different_tasks_at_once_both_stay() {
    let home = temp_home("edits");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    // both edit before seeing the other's change
    a.change(|d| d["tasks"][0]["name"] = json!("Plan (A)"));
    b.change(|d| d["tasks"][1]["name"] = json!("Ship (B)"));
    let both = |d: &Value| task_names(d) == ["Plan (A)", "Ship (B)"];
    a.wait("both edits at A", both);
    b.wait("both edits at B", both);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_task_deleted_by_a_does_not_come_back_from_b() {
    let home = temp_home("delete");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| {
        d["tasks"].as_array_mut().unwrap().retain(|t| t["id"] != 101);
    });
    b.wait("delete reaches B", |d| task_names(d) == ["Plan"]);
    // B keeps working and sends its whole copy again
    b.change(say("still here", 3002));
    a.wait("B's message at A", |d| texts(d).contains(&"still here".to_string()));
    assert_eq!(task_names(&a.data), ["Plan"], "the task stays deleted");
    // and a late, stale copy from before the delete doesn't bring it back either
    let mut stale = workspace();
    let mut meta = SyncMeta::default();
    wb_sync::stamp(&stale, &mut meta, now() - 100_000);
    stale["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    b.client.push(stale);
    b.change(say("after the stale copy", 3003));
    a.wait("message after the stale copy", |d| texts(d).contains(&"after the stale copy".to_string()));
    assert_eq!(task_names(&a.data), ["Plan"]);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_team_copy_survives_a_server_restart() {
    let home = temp_home("restart");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(say("before the restart", 2002));
    b.wait("message at B", |d| texts(d).contains(&"before the restart".to_string()));
    let (a_token, a_device) = (a.token.clone(), a.device.clone());
    drop(a);
    drop(b);
    server.stop();
    assert!(home.join(workboard_server::WORKSPACE_FILE).is_file(), "saved when the server stopped");

    let server = Server::start(&home, 0);
    // A's computer signs in again with its token
    let mut c = App::start(server.port, Value::Null, SignIn { token: a_token, device: a_device, ..Default::default() });
    c.wait("the copy is still there", |d| texts(d).contains(&"before the restart".to_string()) && task_names(d) == ["Plan", "Ship"]);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn messages_written_while_disconnected_merge_after_reconnecting() {
    let home = temp_home("offline");
    let server = Server::start(&home, 0);
    let port = server.port;
    let (mut a, mut b) = two_apps(port);
    a.change(say("online", 2003));
    b.wait("online message at B", |d| texts(d).contains(&"online".to_string()));

    server.stop();
    a.wait_status(|s| matches!(s, Status::Reconnecting(_)), "A notices the server is gone");
    b.wait_status(|s| matches!(s, Status::Reconnecting(_)), "B notices the server is gone");
    // both keep working on their own
    a.change(say("written offline by A", 2004));
    b.change(|d| d["tasks"][0]["name"] = json!("Plan (offline B)"));

    let server = Server::start(&home, port);
    a.wait_status(|s| *s == Status::Connected, "A reconnects by itself");
    b.wait("A's offline message at B", |d| texts(d).contains(&"written offline by A".to_string()));
    a.wait("B's offline edit at A", |d| task_names(d)[0] == "Plan (offline B)");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn only_invited_people_get_in_and_only_admins_invite() {
    let home = temp_home("access");
    let server = Server::start(&home, 0);
    let port = server.port;
    let (mut a, mut b) = two_apps(port);

    // no code, or a used code: refused, and the app stops trying
    let mut x = App::start(port, Value::Null, SignIn::default());
    x.wait_status(|s| matches!(s, Status::Refused(_)), "no code is refused");
    let mut y = App::start(port, Value::Null, SignIn { claim: Some(9), ..Default::default() });
    y.wait_status(|s| matches!(s, Status::Refused(_)), "claiming a server that has a workspace is refused");

    // a member can't invite; after the owner makes them admin they can
    b.client.send(Msg::CreateInvite { member: 3, role: Role::Member });
    b.until("member denied", |b| !b.denied.is_empty());
    b.denied.clear();
    a.client.send(Msg::SetRole { member: 2, role: Role::Admin });
    b.until("B learns it is admin", |b| b.role == Some(Role::Admin));
    b.wait("B's name shows Admin", |d| d["members"][1]["role"] == "Admin");
    let code = b.invite(3, Role::Member);
    let mut c = App::join(port, &code);
    c.wait("C joins", |d| task_names(d).len() == 2);
    assert_eq!((c.member, c.role), (Some(3), Some(Role::Member)));
    let mut z = App::join(port, &code);
    z.wait_status(|s| matches!(s, Status::Refused(_)), "a code works once");

    // nobody takes the owner's place or access
    b.client.send(Msg::Revoke { member: 1 });
    b.until("owner can't be removed", |b| b.denied.iter().any(|d| d.contains("owner")));
    b.denied.clear();
    b.client.send(Msg::CreateInvite { member: 1, role: Role::Member });
    b.until("an admin can't make a code for the owner", |b| b.denied.iter().any(|d| d.contains("Only the owner")));
    // and doesn't see one the owner made for their own second computer
    let owner_code = a.invite(1, Role::Member);
    a.until("the owner sees their code", |a| a.people.iter().any(|p| p.member == 1 && p.invite.as_deref() == Some(owner_code.as_str())));
    b.client.send(Msg::AccessList);
    b.until("B's list", |b| b.people.iter().any(|p| p.member == 1));
    assert!(b.people.iter().all(|p| p.member != 1 || p.invite.is_none()), "the owner's code stays hidden from admins");

    // the owner sees everyone and removes C: C is signed out at once and its token stops working
    a.client.send(Msg::AccessList);
    a.until("people list", |a| a.people.iter().any(|p| p.member == 3 && p.devices == 1));
    let (c_token, c_device) = (c.token.clone(), c.device.clone());
    assert!(c_token.is_some());
    a.client.send(Msg::Revoke { member: 3 });
    c.wait_status(|s| matches!(s, Status::Refused(_)), "C is signed out");
    let mut c2 = App::start(port, Value::Null, SignIn { token: c_token, device: c_device, ..Default::default() });
    c2.wait_status(|s| matches!(s, Status::Refused(_)), "C's token no longer works");
    a.until("C gone from the list", |a| a.people.iter().any(|p| p.member == 3 && p.devices == 0));

    // removing an admin's access also takes back admin rights
    a.client.send(Msg::Revoke { member: 2 });
    b.wait_status(|s| matches!(s, Status::Refused(_)), "B is signed out");
    a.until("B is a plain member again", |a| a.people.iter().any(|p| p.member == 2 && p.role == Role::Member && p.devices == 0));
    a.wait("B's role label is back to Member", |d| d["members"][1]["role"] == "Member");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

fn http_get(port: u16, path: &str) -> (String, Vec<u8>) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(format!("GET {} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n", path).as_bytes()).unwrap();
    let mut out = vec![];
    s.read_to_end(&mut out).unwrap();
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (String::from_utf8_lossy(&out[..split]).to_string(), out[split + 4..].to_vec())
}

#[test]
fn app_updates_are_served_from_the_updates_folder() {
    let home = temp_home("updates");
    let server = Server::start(&home, 0);
    // nothing published yet
    let (head, _) = http_get(server.port, "/updates/appcast.xml");
    assert!(head.starts_with("HTTP/1.1 404"), "{}", head);
    std::fs::create_dir_all(home.join("updates")).unwrap();
    std::fs::write(home.join("updates/appcast.xml"), "<rss/>").unwrap();
    std::fs::write(home.join("updates/Workboard-9.9.9-arm64.zip"), [0u8, 1, 2, 255]).unwrap();
    std::fs::write(home.join("server-secret.zip"), "no").unwrap();
    let (head, body) = http_get(server.port, "/updates/appcast.xml");
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    assert!(head.to_lowercase().contains("content-type: application/xml"), "{}", head);
    assert_eq!(body, b"<rss/>");
    let (head, body) = http_get(server.port, "/updates/Workboard-9.9.9-arm64.zip");
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    assert_eq!(body, [0u8, 1, 2, 255]);
    // nothing outside the folder, and not the team's data
    for path in ["/updates/..%2Fserver-secret.zip", "/updates/../server-secret.zip", "/updates/%2e%2e%2fworkspace.json", "/updates/workspace.json"] {
        let (head, _) = http_get(server.port, path);
        assert!(head.starts_with("HTTP/1.1 404"), "{} -> {}", path, head);
    }
    server.stop();
}

#[test]
fn health_and_admin_commands_answer() {
    let home = temp_home("health");
    let server = Server::start(&home, 0);
    let (_a, _b) = two_apps(server.port);
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    s.write_all(b"GET /health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let body: Value = serde_json::from_str(out.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["ok"], true);
    assert_eq!(body["workspace"], true);
    assert_eq!(body["devices"], 2);
    assert_eq!(body["connections"], 2);

    // the owner lost their computer: a new owner code from the Mac mini itself
    let res = workboard_server::admin_request(&home, &json!({"invite": 1, "role": "owner"}), Duration::from_secs(5)).unwrap().expect("answered");
    assert_eq!(res["ok"], true, "{}", res);
    let code = res["code"].as_str().unwrap().to_string();
    let mut a2 = App::join(server.port, &code);
    a2.until("signed in as owner again", |a| a.role == Some(Role::Owner) && a.member == Some(1));
    // within a minute of having a workspace there is a backup
    let until = Instant::now() + Duration::from_secs(5);
    while workboard_server::backup_files(&home.join(workboard_server::BACKUP_DIR), "hourly").is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(workboard_server::backup_files(&home.join(workboard_server::BACKUP_DIR), "daily").len(), 1);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn restoring_a_backup_reaches_every_app() {
    let home = temp_home("restore");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    // the backup taken now has both tasks
    let backup_path = home.join("manual-backup.json");
    let until = Instant::now() + Duration::from_secs(5);
    while !home.join(workboard_server::WORKSPACE_FILE).is_file() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    std::fs::copy(home.join(workboard_server::WORKSPACE_FILE), &backup_path).unwrap();
    // someone deletes a task by mistake
    a.change(|d| {
        d["tasks"].as_array_mut().unwrap().retain(|t| t["id"] != 101);
    });
    b.wait("delete reaches B", |d| task_names(d) == ["Plan"]);
    let res = workboard_server::admin_request(&home, &json!({"restore": backup_path.to_string_lossy()}), Duration::from_secs(5)).unwrap().expect("answered");
    assert_eq!(res["ok"], true, "{}", res);
    a.wait("the task is back at A", |d| task_names(d) == ["Plan", "Ship"]);
    b.wait("the task is back at B", |d| task_names(d) == ["Plan", "Ship"]);
    // and it stays back after both send their copies again
    a.change(say("after the restore", 2101));
    b.wait("message after restore", |d| texts(d).contains(&"after the restore".to_string()));
    assert_eq!(task_names(&b.data), ["Plan", "Ship"]);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}
