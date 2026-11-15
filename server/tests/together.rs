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
        Self::start_with(home, port, workboard_server::Options::default())
    }

    fn start_with(home: &PathBuf, port: u16, options: workboard_server::Options) -> Server {
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
                workboard_server::serve_with(listener, home, options, async {
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
    backups: Option<Vec<wb_net::Backup>>,
    restored: Vec<String>,
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
        App { data, client, device, status: None, member: None, role: None, token: None, invites: vec![], people: vec![], denied: vec![], backups: None, restored: vec![] }
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
                Event::Backups(b) => self.backups = Some(b),
                Event::Restored(n) => self.restored.push(n),
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

#[test]
fn two_people_setting_up_an_empty_server_at_once_get_one_owner_and_no_merge() {
    let home = temp_home("race");
    let server = Server::start(&home, 0);
    let mut theirs = workspace();
    theirs["tasks"] = json!([{"id": 900, "name": "Their own task", "activity": []}]);
    theirs["members"] = json!([{"id": 1, "name": "Other person"}]);
    // both press Connect at the same moment
    let mut a = App::connect(server.port, Some(workspace()));
    let mut b = App::connect(server.port, Some(theirs));
    let settled = |x: &App| matches!(x.status, Some(Status::Connected) | Some(Status::Refused(_)));
    let until = Instant::now() + Duration::from_secs(15);
    while !(settled(&a) && settled(&b)) && Instant::now() < until {
        a.pump();
        b.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
    let owners: Vec<&App> = [&a, &b].into_iter().filter(|x| x.status == Some(Status::Connected)).collect();
    assert_eq!(owners.len(), 1, "exactly one sets the server up (a: {:?}, b: {:?})", a.status, b.status);
    let (winner, loser) = if a.status == Some(Status::Connected) { (&mut a, &mut b) } else { (&mut b, &mut a) };
    assert_eq!(winner.role, Some(Role::Owner));
    let Some(Status::Refused(why)) = &loser.status else { unreachable!() };
    assert!(why.contains("setting up this server") || why.contains("already has a workspace"), "{}", why);
    let expected = task_names(&winner.data);
    winner.wait("winner's copy on the server", |_| true);
    // the team copy is the winner's workspace only
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let canon: Value = std::fs::read_to_string(home.join(workboard_server::WORKSPACE_FILE)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
        if !canon.is_null() {
            assert_eq!(task_names(&canon), expected, "nothing from the other workspace got in");
            break;
        }
        assert!(Instant::now() < until, "server never saved the workspace");
        winner.pump();
        std::thread::sleep(Duration::from_millis(50));
    }
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_owner_restores_a_backup_from_the_app() {
    let home = temp_home("apprestore");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    // wait for the first backup (made within a minute of having a workspace)
    let dir = home.join(workboard_server::BACKUP_DIR);
    let until = Instant::now() + Duration::from_secs(10);
    while workboard_server::backup_files(&dir, "hourly").is_empty() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    a.client.send(Msg::BackupList);
    a.until("the owner sees the backups", |a| a.backups.as_ref().is_some_and(|l| l.iter().any(|b| b.name.starts_with("hourly-"))));
    let hourly = a.backups.as_ref().unwrap().iter().find(|b| b.name.starts_with("hourly-")).unwrap().name.clone();

    // a member may not look or restore
    b.client.send(Msg::BackupList);
    b.until("member can't list", |b| b.denied.iter().any(|d| d.contains("Only the owner")));
    b.denied.clear();
    b.client.send(Msg::Restore { name: hourly.clone() });
    b.until("member can't restore", |b| b.denied.iter().any(|d| d.contains("Only the owner")));

    // nothing outside the backups folder
    for bad in ["../workspace.json", "/etc/hosts", "hourly-../../x.json", "server.json"] {
        a.denied.clear();
        a.client.send(Msg::Restore { name: bad.into() });
        a.until("bad name refused", |a| a.denied.iter().any(|d| d.contains("isn't on the server")));
    }

    // a task is deleted by mistake; the owner restores the backup from the app
    a.change(|d| {
        d["tasks"].as_array_mut().unwrap().retain(|t| t["id"] != 101);
    });
    b.wait("delete reaches B", |d| task_names(d) == ["Plan"]);
    a.client.send(Msg::Restore { name: hourly.clone() });
    a.until("restored", |a| a.restored.contains(&hourly));
    b.wait("the task is back at B", |d| task_names(d) == ["Plan", "Ship"]);
    a.wait("and at A", |d| task_names(d) == ["Plan", "Ship"]);
    assert!(!workboard_server::backup_files(&dir, "before-restore").is_empty(), "the copy from before is kept");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------------------------ files

fn auth(app: &App) -> wb_net::Auth {
    wb_net::Auth { device: app.device.clone(), token: app.token.clone().expect("signed in with a token") }
}

/// A file of `n` bytes that isn't all the same byte.
fn some_file(dir: &PathBuf, name: &str, n: usize) -> (PathBuf, String, Vec<u8>) {
    std::fs::create_dir_all(dir).unwrap();
    let bytes: Vec<u8> = (0..n).map(|i| (i * 31 % 251) as u8).collect();
    let path = dir.join(name);
    std::fs::write(&path, &bytes).unwrap();
    let (sha, len) = wb_net::sha256_file(&path).unwrap();
    assert_eq!(len as usize, n);
    (path, sha, bytes)
}

fn no_progress() -> impl FnMut(u64, u64) -> bool {
    |_, _| true
}

#[test]
fn a_picture_goes_up_from_a_and_comes_down_at_b() {
    let home = temp_home("files");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.until("A has a token", |a| a.token.is_some());
    b.until("B has a token", |b| b.token.is_some());
    let addr = format!("127.0.0.1:{}", server.port);
    let work = home.join("work");
    let (path, sha, bytes) = some_file(&work, "photo.jpg", 3 << 20);

    assert_eq!(wb_net::remote_size(&addr, &auth(&a), &sha).unwrap(), None, "not there yet");
    let mut seen = vec![];
    wb_net::upload(&addr, &auth(&a), &sha, &path, "image/jpeg", &mut |d, t| {
        seen.push((d, t));
        true
    })
    .unwrap();
    assert!(seen.len() > 1 && seen.last() == Some(&(bytes.len() as u64, bytes.len() as u64)), "progress up to the whole file");
    assert_eq!(wb_net::remote_size(&addr, &auth(&a), &sha).unwrap(), Some(bytes.len() as u64));
    assert_eq!(std::fs::read(home.join("files").join(&sha)).unwrap(), bytes, "stored under its sha256");

    // B fetches it, checked against the name
    let dest = work.join("b-cache").join(&sha);
    wb_net::download(&addr, &auth(&b), &sha, &dest, &mut no_progress()).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);

    // the same file again isn't sent again
    let mut calls = 0;
    wb_net::upload(&addr, &auth(&b), &sha, &path, "image/jpeg", &mut |_, _| {
        calls += 1;
        true
    })
    .unwrap();
    assert_eq!(calls, 1, "only the final progress report, nothing streamed");
    let stored: Vec<_> = std::fs::read_dir(home.join("files")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(stored.len(), 2, "one file and its type: {:?}", stored);

    // a range, for seeking in a video
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    let t = auth(&a);
    s.write_all(format!("GET /files/{} HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {}:{}\r\nRange: bytes=10-19\r\nConnection: close\r\n\r\n", sha, t.device, t.token).as_bytes()).unwrap();
    let mut out = vec![];
    s.read_to_end(&mut out).unwrap();
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&out[..split]).to_lowercase();
    assert!(head.starts_with("http/1.1 206"), "{}", head);
    assert!(head.contains(&format!("content-range: bytes 10-19/{}", bytes.len())), "{}", head);
    assert!(head.contains("content-type: image/jpeg"), "{}", head);
    assert_eq!(&out[split + 4..], &bytes[10..20]);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn files_need_a_valid_token_and_honest_contents() {
    let home = temp_home("files-auth");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.until("A has a token", |a| a.token.is_some());
    b.until("B has a token", |b| b.token.is_some());
    let addr = format!("127.0.0.1:{}", server.port);
    let (path, sha, _) = some_file(&home.join("work"), "clip.mov", 4096);

    // no token, a made-up token
    let (head, _) = http_get(server.port, &format!("/files/{}", sha));
    assert!(head.starts_with("HTTP/1.1 401"), "{}", head);
    let fake = wb_net::Auth { device: a.device.clone(), token: "0".repeat(64) };
    assert_eq!(wb_net::upload(&addr, &fake, &sha, &path, "video/quicktime", &mut no_progress()), Err(wb_net::FileError::NotSignedIn));

    // contents that don't match the name are refused
    let wrong = "a".repeat(64);
    match wb_net::upload(&addr, &auth(&a), &wrong, &path, "video/quicktime", &mut no_progress()) {
        Err(wb_net::FileError::Other(e)) => assert!(e.contains("don't match"), "{}", e),
        other => panic!("expected a refusal, got {:?}", other),
    }
    assert!(!home.join("files").join(&wrong).exists());
    assert_eq!(wb_net::download(&addr, &auth(&a), &wrong, &home.join("work/x"), &mut no_progress()), Err(wb_net::FileError::Missing));
    // not a file name at all
    let (head, _) = http_get(server.port, "/files/..%2Fworkspace.json");
    assert!(head.starts_with("HTTP/1.1 404"), "{}", head);

    // once B's access is removed, B's token stops working for files too
    wb_net::upload(&addr, &auth(&a), &sha, &path, "video/quicktime", &mut no_progress()).unwrap();
    let b_auth = auth(&b);
    a.client.send(Msg::Revoke { member: 2 });
    b.wait_status(|s| matches!(s, Status::Refused(_)), "B is signed out");
    assert_eq!(wb_net::download(&addr, &b_auth, &sha, &home.join("work/b"), &mut no_progress()), Err(wb_net::FileError::NotSignedIn));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn files_over_the_limit_are_refused() {
    let home = temp_home("files-big");
    let server = Server::start_with(&home, 0, workboard_server::Options { max_file: 64 << 10 });
    let (mut a, _b) = two_apps(server.port);
    a.until("A has a token", |a| a.token.is_some());
    let addr = format!("127.0.0.1:{}", server.port);
    let (path, sha, _) = some_file(&home.join("work"), "long.mp4", 1 << 20);
    assert_eq!(wb_net::upload(&addr, &auth(&a), &sha, &path, "video/mp4", &mut no_progress()), Err(wb_net::FileError::TooLarge));
    let leftovers = std::fs::read_dir(home.join("files")).map(|r| r.flatten().count()).unwrap_or(0);
    assert_eq!(leftovers, 0, "nothing half-written stays behind");
    // a small one still goes
    let (small, sha2, _) = some_file(&home.join("work"), "short.mp4", 1000);
    wb_net::upload(&addr, &auth(&a), &sha2, &small, "video/mp4", &mut no_progress()).unwrap();
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_broken_download_continues_where_it_stopped() {
    let home = temp_home("files-resume");
    let server = Server::start(&home, 0);
    let (mut a, _b) = two_apps(server.port);
    a.until("A has a token", |a| a.token.is_some());
    let addr = format!("127.0.0.1:{}", server.port);
    let (path, sha, bytes) = some_file(&home.join("work"), "movie.mp4", 1 << 20);
    wb_net::upload(&addr, &auth(&a), &sha, &path, "video/mp4", &mut no_progress()).unwrap();
    // half of it arrived last time
    let dest = home.join("cache").join(&sha);
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::write(dest.with_extension("part"), &bytes[..bytes.len() / 2]).unwrap();
    let mut first = None;
    wb_net::download(&addr, &auth(&a), &sha, &dest, &mut |d, t| {
        first.get_or_insert((d, t));
        true
    })
    .unwrap();
    let (d, t) = first.unwrap();
    assert!(d > (bytes.len() / 2) as u64 && t == bytes.len() as u64, "counted on from the half: {} of {}", d, t);
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);
    assert!(!dest.with_extension("part").exists());
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// An app from before attachments (0.1.4) reads messages without the field
/// and edits them. The attachments must survive: they are a plain array (no
/// ids), so the server keeps them from its own copy.
#[test]
fn attachments_survive_an_app_that_does_not_know_them() {
    let home = temp_home("files-old-app");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let pic = json!([{"file": "ab".repeat(32), "name": "photo.jpg", "kind": "Image", "bytes": 1234, "w": 640, "h": 480, "secs": 0.0}]);
    let p2 = pic.clone();
    a.change(move |d| {
        d["channels"][0]["messages"].as_array_mut().unwrap().push(json!({"id": 4001, "author": 1, "text": "look", "ts": now(), "attachments": p2}));
    });
    b.wait("the picture message at B", |d| texts(d).contains(&"look".to_string()));
    // B is the old app: it never keeps the field, and it reacts to the message
    b.change(|d| {
        for m in d["channels"][0]["messages"].as_array_mut().unwrap() {
            m.as_object_mut().unwrap().remove("attachments");
            if m["id"] == 4001 {
                m["reactions"] = json!([{"emoji": "👍", "users": [2]}]);
            }
        }
    });
    a.wait("B's reaction at A", |d| d["channels"][0]["messages"].as_array().unwrap().iter().any(|m| m["id"] == 4001 && m["reactions"][0]["users"] == json!([2])));
    let at_a = a.data["channels"][0]["messages"].as_array().unwrap().iter().find(|m| m["id"] == 4001).unwrap().clone();
    assert_eq!(at_a["attachments"], pic, "the picture is still there");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}
