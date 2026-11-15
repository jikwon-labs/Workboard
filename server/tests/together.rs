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
        let port = port_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("server started");
        Server {
            port,
            stop: Some(stop_tx),
            thread: Some(thread),
        }
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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
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
    /// account email from the welcome, then from `account` answers
    email: Option<String>,
    /// (member, emailed to, mail error) for each invite answer
    mailed: Vec<(u64, Option<String>, Option<String>)>,
}

impl App {
    /// The owner setting up an empty server with their workspace.
    fn connect(port: u16, data: Option<Value>) -> App {
        let mut d = data.unwrap();
        let mut meta = SyncMeta::default();
        wb_sync::stamp(&d, &mut meta, now());
        d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        let me = d["me"].as_u64();
        Self::start(
            port,
            d,
            SignIn {
                claim: me,
                ..Default::default()
            },
        )
    }

    /// Someone joining with an invite code: they take whatever the server has.
    fn join(port: u16, code: &str) -> App {
        Self::start(
            port,
            Value::Null,
            SignIn {
                invite: Some(code.to_string()),
                ..Default::default()
            },
        )
    }

    fn start(port: u16, data: Value, mut sign_in: SignIn) -> App {
        if sign_in.device.is_empty() {
            sign_in.device = format!("dev{:x}", wb_sync::random_u64());
        }
        let device = sign_in.device.clone();
        let client = Client::start(&format!("127.0.0.1:{}", port), sign_in, Arc::new(|| {}));
        App {
            data,
            client,
            device,
            status: None,
            member: None,
            role: None,
            token: None,
            invites: vec![],
            people: vec![],
            denied: vec![],
            backups: None,
            restored: vec![],
            email: None,
            mailed: vec![],
        }
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
        panic!(
            "timed out waiting for: {} (status {:?}, role {:?}, denied {:?})",
            what, self.status, self.role, self.denied
        );
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

    /// Edit like an app from before field stamps (and before fields at
    /// their default were left out): its copy says only when each item
    /// changed.
    fn change_old(&mut self, f: impl FnOnce(&mut Value)) {
        f(&mut self.data);
        let mut meta = self.meta();
        wb_sync::stamp(&self.data, &mut meta, now());
        meta.fields.clear();
        meta.field_hashes.clear();
        meta.moved.clear();
        meta.v = 1;
        self.data["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        self.push();
    }

    fn take_in(&mut self, other: Value) {
        if self.data.is_null() {
            // joining: start from the team's copy
            self.data = other;
            let mut meta = self.meta();
            meta.hashes = wb_sync::fingerprints(&self.data);
            meta.field_hashes = wb_sync::part_hashes(&self.data);
            self.data["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        } else {
            wb_sync::merge(&mut self.data, &[other]);
        }
    }

    fn pump(&mut self) {
        for e in self.client.events() {
            match e {
                Event::Status(s) => self.status = Some(s),
                Event::Welcome {
                    snapshot,
                    member,
                    role,
                    token,
                    email,
                    hidden,
                    ..
                } => {
                    self.member = Some(member);
                    self.email = email;
                    self.role = Some(role);
                    if token.is_some() {
                        self.token = token;
                    }
                    if let Some(s) = snapshot {
                        self.take_in(s);
                    }
                    // what this person may no longer see leaves their copy (without deleting it)
                    if !self.data.is_null() {
                        wb_sync::hide(&mut self.data, &hidden);
                    }
                    // after (re)connecting the whole copy goes up
                    self.push();
                }
                Event::Update { data, hidden, .. } => {
                    self.take_in(data);
                    wb_sync::hide(&mut self.data, &hidden);
                }
                Event::Invite {
                    member,
                    code,
                    emailed,
                    mail_error,
                } => {
                    self.invites.push((member, code));
                    self.mailed.push((member, emailed, mail_error));
                }
                Event::Account(e) => self.email = Some(e),
                Event::Access { people, role } => {
                    self.people = people;
                    self.role = Some(role);
                }
                Event::Denied(why) | Event::CallDenied { why, .. } => self.denied.push(why),
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
        panic!(
            "timed out waiting for: {} (status {:?})\n{}",
            what, self.status, self.data
        );
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
        panic!(
            "timed out waiting for status {} (have {:?})",
            what, self.status
        );
    }
}

fn texts(d: &Value) -> Vec<String> {
    d["channels"][0]["messages"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|m| m["text"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn task_names(d: &Value) -> Vec<String> {
    d["tasks"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|t| t["name"].as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn say(text: &str, id: u64) -> impl FnOnce(&mut Value) + '_ {
    move |d: &mut Value| {
        d["channels"][0]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": id, "author": 1, "text": text, "ts": now()}));
    }
}

/// A sets up an empty server with a workspace, invites B, B joins.
fn two_apps(port: u16) -> (App, App) {
    let mut a = App::connect(port, Some(workspace()));
    a.wait("A's copy on the server", |_| true);
    a.until("A is the owner", |a| a.role == Some(Role::Owner));
    let code = a.invite(2, Role::Member);
    let mut b = App::join(port, &code);
    b.wait("B gets the team's copy", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    assert_eq!(b.member, Some(2), "the code signs B in as member 2");
    (a, b)
}

#[test]
fn a_chat_message_from_a_reaches_b() {
    let home = temp_home("chat");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(say("hello from A", 2001));
    b.wait("A's message at B", |d| {
        texts(d).contains(&"hello from A".to_string())
    });
    b.change(say("and from B", 3001));
    a.wait("B's answer at A", |d| {
        texts(d).ends_with(&["hello from A".to_string(), "and from B".to_string()])
    });
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
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t["id"] != 101);
    });
    b.wait("delete reaches B", |d| task_names(d) == ["Plan"]);
    // B keeps working and sends its whole copy again
    b.change(say("still here", 3002));
    a.wait("B's message at A", |d| {
        texts(d).contains(&"still here".to_string())
    });
    assert_eq!(task_names(&a.data), ["Plan"], "the task stays deleted");
    // and a late, stale copy from before the delete doesn't bring it back either
    let mut stale = workspace();
    let mut meta = SyncMeta::default();
    wb_sync::stamp(&stale, &mut meta, now() - 100_000);
    stale["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    b.client.push(stale);
    b.change(say("after the stale copy", 3003));
    a.wait("message after the stale copy", |d| {
        texts(d).contains(&"after the stale copy".to_string())
    });
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
    b.wait("message at B", |d| {
        texts(d).contains(&"before the restart".to_string())
    });
    let (a_token, a_device) = (a.token.clone(), a.device.clone());
    drop(a);
    drop(b);
    server.stop();
    assert!(
        home.join(workboard_server::WORKSPACE_FILE).is_file(),
        "saved when the server stopped"
    );

    let server = Server::start(&home, 0);
    // A's computer signs in again with its token
    let mut c = App::start(
        server.port,
        Value::Null,
        SignIn {
            token: a_token,
            device: a_device,
            ..Default::default()
        },
    );
    c.wait("the copy is still there", |d| {
        texts(d).contains(&"before the restart".to_string()) && task_names(d) == ["Plan", "Ship"]
    });
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
    b.wait("online message at B", |d| {
        texts(d).contains(&"online".to_string())
    });

    server.stop();
    a.wait_status(
        |s| matches!(s, Status::Reconnecting(_)),
        "A notices the server is gone",
    );
    b.wait_status(
        |s| matches!(s, Status::Reconnecting(_)),
        "B notices the server is gone",
    );
    // both keep working on their own
    a.change(say("written offline by A", 2004));
    b.change(|d| d["tasks"][0]["name"] = json!("Plan (offline B)"));

    let server = Server::start(&home, port);
    a.wait_status(|s| *s == Status::Connected, "A reconnects by itself");
    b.wait("A's offline message at B", |d| {
        texts(d).contains(&"written offline by A".to_string())
    });
    a.wait("B's offline edit at A", |d| {
        task_names(d)[0] == "Plan (offline B)"
    });
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
    let mut y = App::start(
        port,
        Value::Null,
        SignIn {
            claim: Some(9),
            ..Default::default()
        },
    );
    y.wait_status(
        |s| matches!(s, Status::Refused(_)),
        "claiming a server that has a workspace is refused",
    );

    // a member can't invite; after the owner makes them admin they can
    b.client.send(Msg::CreateInvite {
        member: 3,
        role: Role::Member,
    });
    b.until("member denied", |b| !b.denied.is_empty());
    b.denied.clear();
    a.client.send(Msg::SetRole {
        member: 2,
        role: Role::Admin,
    });
    b.until("B learns it is admin", |b| b.role == Some(Role::Admin));
    b.wait("B's name shows Admin", |d| {
        d["members"][1]["role"] == "Admin"
    });
    let code = b.invite(3, Role::Member);
    let mut c = App::join(port, &code);
    c.wait("C joins", |d| task_names(d).len() == 2);
    assert_eq!((c.member, c.role), (Some(3), Some(Role::Member)));
    let mut z = App::join(port, &code);
    z.wait_status(|s| matches!(s, Status::Refused(_)), "a code works once");

    // nobody takes the owner's place or access
    b.client.send(Msg::Revoke { member: 1 });
    b.until("owner can't be removed", |b| {
        b.denied.iter().any(|d| d.contains("owner"))
    });
    b.denied.clear();
    b.client.send(Msg::CreateInvite {
        member: 1,
        role: Role::Member,
    });
    b.until("an admin can't make a code for the owner", |b| {
        b.denied.iter().any(|d| d.contains("Only the owner"))
    });
    // and doesn't see one the owner made for their own second computer
    let owner_code = a.invite(1, Role::Member);
    a.until("the owner sees their code", |a| {
        a.people
            .iter()
            .any(|p| p.member == 1 && p.invite.as_deref() == Some(owner_code.as_str()))
    });
    b.client.send(Msg::AccessList);
    b.until("B's list", |b| b.people.iter().any(|p| p.member == 1));
    assert!(
        b.people.iter().all(|p| p.member != 1 || p.invite.is_none()),
        "the owner's code stays hidden from admins"
    );

    // the owner sees everyone and removes C: C is signed out at once and its token stops working
    a.client.send(Msg::AccessList);
    a.until("people list", |a| {
        a.people.iter().any(|p| p.member == 3 && p.devices == 1)
    });
    let (c_token, c_device) = (c.token.clone(), c.device.clone());
    assert!(c_token.is_some());
    a.client.send(Msg::Revoke { member: 3 });
    c.wait_status(|s| matches!(s, Status::Refused(_)), "C is signed out");
    let mut c2 = App::start(
        port,
        Value::Null,
        SignIn {
            token: c_token,
            device: c_device,
            ..Default::default()
        },
    );
    c2.wait_status(
        |s| matches!(s, Status::Refused(_)),
        "C's token no longer works",
    );
    a.until("C gone from the list", |a| {
        a.people.iter().any(|p| p.member == 3 && p.devices == 0)
    });

    // removing an admin's access also takes back admin rights
    a.client.send(Msg::Revoke { member: 2 });
    b.wait_status(|s| matches!(s, Status::Refused(_)), "B is signed out");
    a.until("B is a plain member again", |a| {
        a.people
            .iter()
            .any(|p| p.member == 2 && p.role == Role::Member && p.devices == 0)
    });
    a.wait("B's role label is back to Member", |d| {
        d["members"][1]["role"] == "Member"
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

fn http_get(port: u16, path: &str) -> (String, Vec<u8>) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(
        format!(
            "GET {} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
            path
        )
        .as_bytes(),
    )
    .unwrap();
    let mut out = vec![];
    s.read_to_end(&mut out).unwrap();
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8_lossy(&out[..split]).to_string(),
        out[split + 4..].to_vec(),
    )
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
    std::fs::write(
        home.join("updates/Workboard-9.9.9-arm64.zip"),
        [0u8, 1, 2, 255],
    )
    .unwrap();
    std::fs::write(home.join("server-secret.zip"), "no").unwrap();
    let (head, body) = http_get(server.port, "/updates/appcast.xml");
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    assert!(
        head.to_lowercase()
            .contains("content-type: application/xml"),
        "{}",
        head
    );
    assert_eq!(body, b"<rss/>");
    let (head, body) = http_get(server.port, "/updates/Workboard-9.9.9-arm64.zip");
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    assert_eq!(body, [0u8, 1, 2, 255]);
    // nothing outside the folder, and not the team's data
    for path in [
        "/updates/..%2Fserver-secret.zip",
        "/updates/../server-secret.zip",
        "/updates/%2e%2e%2fworkspace.json",
        "/updates/workspace.json",
    ] {
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
    s.write_all(b"GET /health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let body: Value = serde_json::from_str(out.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["ok"], true);
    assert_eq!(body["workspace"], true);
    assert_eq!(body["devices"], 2);
    assert_eq!(body["connections"], 2);

    // the owner lost their computer: a new owner code from the Mac mini itself
    let res = workboard_server::admin_request(
        &home,
        &json!({"invite": 1, "role": "owner"}),
        Duration::from_secs(5),
    )
    .unwrap()
    .expect("answered");
    assert_eq!(res["ok"], true, "{}", res);
    let code = res["code"].as_str().unwrap().to_string();
    let mut a2 = App::join(server.port, &code);
    a2.until("signed in as owner again", |a| {
        a.role == Some(Role::Owner) && a.member == Some(1)
    });
    // within a minute of having a workspace there is a backup
    let until = Instant::now() + Duration::from_secs(5);
    while workboard_server::backup_files(&home.join(workboard_server::BACKUP_DIR), "hourly")
        .is_empty()
        && Instant::now() < until
    {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        workboard_server::backup_files(&home.join(workboard_server::BACKUP_DIR), "daily").len(),
        1
    );
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
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t["id"] != 101);
    });
    b.wait("delete reaches B", |d| task_names(d) == ["Plan"]);
    let res = workboard_server::admin_request(
        &home,
        &json!({"restore": backup_path.to_string_lossy()}),
        Duration::from_secs(5),
    )
    .unwrap()
    .expect("answered");
    assert_eq!(res["ok"], true, "{}", res);
    a.wait("the task is back at A", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    b.wait("the task is back at B", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    // and it stays back after both send their copies again
    a.change(say("after the restore", 2101));
    b.wait("message after restore", |d| {
        texts(d).contains(&"after the restore".to_string())
    });
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
    let owners: Vec<&App> = [&a, &b]
        .into_iter()
        .filter(|x| x.status == Some(Status::Connected))
        .collect();
    assert_eq!(
        owners.len(),
        1,
        "exactly one sets the server up (a: {:?}, b: {:?})",
        a.status,
        b.status
    );
    let (winner, loser) = if a.status == Some(Status::Connected) {
        (&mut a, &mut b)
    } else {
        (&mut b, &mut a)
    };
    assert_eq!(winner.role, Some(Role::Owner));
    let Some(Status::Refused(why)) = &loser.status else {
        unreachable!()
    };
    assert!(
        why.contains("setting up this server") || why.contains("already has a workspace"),
        "{}",
        why
    );
    let expected = task_names(&winner.data);
    winner.wait("winner's copy on the server", |_| true);
    // the team copy is the winner's workspace only
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let canon: Value = std::fs::read_to_string(home.join(workboard_server::WORKSPACE_FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
        if !canon.is_null() {
            assert_eq!(
                task_names(&canon),
                expected,
                "nothing from the other workspace got in"
            );
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
    a.until("the owner sees the backups", |a| {
        a.backups
            .as_ref()
            .is_some_and(|l| l.iter().any(|b| b.name.starts_with("hourly-")))
    });
    let hourly = a
        .backups
        .as_ref()
        .unwrap()
        .iter()
        .find(|b| b.name.starts_with("hourly-"))
        .unwrap()
        .name
        .clone();

    // a member may not look or restore
    b.client.send(Msg::BackupList);
    b.until("member can't list", |b| {
        b.denied.iter().any(|d| d.contains("Only the owner"))
    });
    b.denied.clear();
    b.client.send(Msg::Restore {
        name: hourly.clone(),
    });
    b.until("member can't restore", |b| {
        b.denied.iter().any(|d| d.contains("Only the owner"))
    });

    // nothing outside the backups folder
    for bad in [
        "../workspace.json",
        "/etc/hosts",
        "hourly-../../x.json",
        "server.json",
    ] {
        a.denied.clear();
        a.client.send(Msg::Restore { name: bad.into() });
        a.until("bad name refused", |a| {
            a.denied.iter().any(|d| d.contains("isn't on the server"))
        });
    }

    // a task is deleted by mistake; the owner restores the backup from the app
    a.change(|d| {
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t["id"] != 101);
    });
    b.wait("delete reaches B", |d| task_names(d) == ["Plan"]);
    a.client.send(Msg::Restore {
        name: hourly.clone(),
    });
    a.until("restored", |a| a.restored.contains(&hourly));
    b.wait("the task is back at B", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    a.wait("and at A", |d| task_names(d) == ["Plan", "Ship"]);
    assert!(
        !workboard_server::backup_files(&dir, "before-restore").is_empty(),
        "the copy from before is kept"
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------------------------ files

fn auth(app: &App) -> wb_net::Auth {
    wb_net::Auth {
        device: app.device.clone(),
        token: app.token.clone().expect("signed in with a token"),
    }
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

    assert_eq!(
        wb_net::remote_size(&addr, &auth(&a), &sha).unwrap(),
        None,
        "not there yet"
    );
    let mut seen = vec![];
    wb_net::upload(&addr, &auth(&a), &sha, &path, "image/jpeg", &mut |d, t| {
        seen.push((d, t));
        true
    })
    .unwrap();
    assert!(
        seen.len() > 1 && seen.last() == Some(&(bytes.len() as u64, bytes.len() as u64)),
        "progress up to the whole file"
    );
    assert_eq!(
        wb_net::remote_size(&addr, &auth(&a), &sha).unwrap(),
        Some(bytes.len() as u64)
    );
    assert_eq!(
        std::fs::read(home.join("files").join(&sha)).unwrap(),
        bytes,
        "stored under its sha256"
    );

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
    let stored: Vec<_> = std::fs::read_dir(home.join("files"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
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
    assert!(
        head.contains(&format!("content-range: bytes 10-19/{}", bytes.len())),
        "{}",
        head
    );
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
    let fake = wb_net::Auth {
        device: a.device.clone(),
        token: "0".repeat(64),
    };
    assert_eq!(
        wb_net::upload(
            &addr,
            &fake,
            &sha,
            &path,
            "video/quicktime",
            &mut no_progress()
        ),
        Err(wb_net::FileError::NotSignedIn)
    );

    // contents that don't match the name are refused
    let wrong = "a".repeat(64);
    match wb_net::upload(
        &addr,
        &auth(&a),
        &wrong,
        &path,
        "video/quicktime",
        &mut no_progress(),
    ) {
        Err(wb_net::FileError::Other(e)) => assert!(e.contains("don't match"), "{}", e),
        other => panic!("expected a refusal, got {:?}", other),
    }
    assert!(!home.join("files").join(&wrong).exists());
    assert_eq!(
        wb_net::download(
            &addr,
            &auth(&a),
            &wrong,
            &home.join("work/x"),
            &mut no_progress()
        ),
        Err(wb_net::FileError::Missing)
    );
    // not a file name at all
    let (head, _) = http_get(server.port, "/files/..%2Fworkspace.json");
    assert!(head.starts_with("HTTP/1.1 404"), "{}", head);

    // once B's access is removed, B's token stops working for files too
    wb_net::upload(
        &addr,
        &auth(&a),
        &sha,
        &path,
        "video/quicktime",
        &mut no_progress(),
    )
    .unwrap();
    let b_auth = auth(&b);
    a.client.send(Msg::Revoke { member: 2 });
    b.wait_status(|s| matches!(s, Status::Refused(_)), "B is signed out");
    assert_eq!(
        wb_net::download(
            &addr,
            &b_auth,
            &sha,
            &home.join("work/b"),
            &mut no_progress()
        ),
        Err(wb_net::FileError::NotSignedIn)
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn files_over_the_limit_are_refused() {
    let home = temp_home("files-big");
    let server = Server::start_with(
        &home,
        0,
        workboard_server::Options {
            max_file: 64 << 10,
            ..Default::default()
        },
    );
    let (mut a, _b) = two_apps(server.port);
    a.until("A has a token", |a| a.token.is_some());
    let addr = format!("127.0.0.1:{}", server.port);
    let (path, sha, _) = some_file(&home.join("work"), "long.mp4", 1 << 20);
    assert_eq!(
        wb_net::upload(
            &addr,
            &auth(&a),
            &sha,
            &path,
            "video/mp4",
            &mut no_progress()
        ),
        Err(wb_net::FileError::TooLarge)
    );
    let leftovers = std::fs::read_dir(home.join("files"))
        .map(|r| r.flatten().count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "nothing half-written stays behind");
    // a small one still goes
    let (small, sha2, _) = some_file(&home.join("work"), "short.mp4", 1000);
    wb_net::upload(
        &addr,
        &auth(&a),
        &sha2,
        &small,
        "video/mp4",
        &mut no_progress(),
    )
    .unwrap();
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
    wb_net::upload(
        &addr,
        &auth(&a),
        &sha,
        &path,
        "video/mp4",
        &mut no_progress(),
    )
    .unwrap();
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
    assert!(
        d > (bytes.len() / 2) as u64 && t == bytes.len() as u64,
        "counted on from the half: {} of {}",
        d,
        t
    );
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
        d["channels"][0]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 4001, "author": 1, "text": "look", "ts": now(), "attachments": p2}));
    });
    b.wait("the picture message at B", |d| {
        texts(d).contains(&"look".to_string())
    });
    // B is the old app: it never keeps the field, and it reacts to the message
    b.change_old(|d| {
        for m in d["channels"][0]["messages"].as_array_mut().unwrap() {
            m.as_object_mut().unwrap().remove("attachments");
            if m["id"] == 4001 {
                m["reactions"] = json!([{"emoji": "👍", "users": [2]}]);
            }
        }
    });
    a.wait("B's reaction at A", |d| {
        d["channels"][0]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == 4001 && m["reactions"][0]["users"] == json!([2]))
    });
    let at_a = a.data["channels"][0]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == 4001)
        .unwrap()
        .clone();
    assert_eq!(at_a["attachments"], pic, "the picture is still there");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ---------------------------------------------------------------- accounts

/// A workspace whose people have emails, and a server that writes its mails
/// to outbox/ instead of sending them.
fn mail_home(tag: &str) -> PathBuf {
    let home = temp_home(tag);
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join(workboard_server::MAIL_FILE),
        r#"{"transport": "folder"}"#,
    )
    .unwrap();
    home
}

fn emailed_workspace() -> Value {
    let mut w = workspace();
    w["members"][0]["email"] = json!("ann@team.io");
    w["members"][1]["email"] = json!("bo@team.io");
    w["members"][2]["email"] = json!("cy@team.io");
    w
}

/// The mails written so far to `to`, oldest first.
fn outbox(home: &PathBuf, to: &str) -> Vec<String> {
    let mut files: Vec<_> = std::fs::read_dir(home.join(workboard_server::OUTBOX_DIR))
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.sort();
    files
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .filter(|t| t.starts_with(&format!("To: {}\n", to)))
        .collect()
}

/// The first XXXX-XXXX-XXXX in a text.
fn code_in(text: &str) -> String {
    text.split(|c: char| c.is_whitespace() || c == '=' || c == '&')
        .find(|w| {
            w.len() == 14
                && w.chars().filter(|c| *c == '-').count() == 2
                && w.chars()
                    .all(|c| c == '-' || c.is_ascii_uppercase() || c.is_ascii_digit())
        })
        .unwrap_or_else(|| panic!("no code in: {}", text))
        .to_string()
}

fn sign_in(port: u16, s: SignIn) -> App {
    App::start(port, Value::Null, s)
}

fn refused_with(app: &mut App, what: &str) -> String {
    app.wait_status(|s| matches!(s, Status::Refused(_)), what);
    match &app.status {
        Some(Status::Refused(why)) => why.clone(),
        _ => unreachable!(),
    }
}

fn http_post(port: u16, path: &str, body: &str) -> (String, String) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(format!("POST {} HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", path, body.len(), body).as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let (h, b) = out.split_once("\r\n\r\n").unwrap();
    (h.to_string(), b.to_string())
}

fn wait_for_mail(home: &PathBuf, to: &str, n: usize) -> String {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        let m = outbox(home, to);
        if m.len() >= n {
            return m[n - 1].clone();
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    panic!("no mail #{} to {}", n, to);
}

#[test]
fn an_emailed_invite_sets_a_password_that_signs_in_on_any_device() {
    let home = mail_home("accounts");
    let server = Server::start(&home, 0);
    let port = server.port;

    // the owner sets up the server and chooses a password at the same time
    let mut d = emailed_workspace();
    let mut meta = SyncMeta::default();
    wb_sync::stamp(&d, &mut meta, now());
    d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    let mut a = App::start(
        port,
        d,
        SignIn {
            claim: Some(1),
            email: Some("ann@team.io".into()),
            new_password: Some("ann-secret-1".into()),
            ..Default::default()
        },
    );
    a.until("owner in", |a| a.role == Some(Role::Owner));
    assert_eq!(a.email.as_deref(), Some("ann@team.io"));
    a.until("workspace up", |a| a.people.iter().any(|p| p.member == 2));

    // the invite code goes out by email, with a link to the join page
    let code = a.invite(2, Role::Member);
    a.until("mail answer", |a| !a.mailed.is_empty());
    assert_eq!(a.mailed[0], (2, Some("bo@team.io".to_string()), None));
    let mail = wait_for_mail(&home, "bo@team.io", 1);
    assert!(mail.contains(&code), "{}", mail);
    assert!(
        mail.contains(&format!(
            "http://127.0.0.1:{}/join?code={}&email=bo%40team.io",
            port, code
        )),
        "{}",
        mail
    );
    assert!(
        mail.contains("Subject: You're invited to Team on Workboard"),
        "{}",
        mail
    );
    assert!(mail.contains("Ann invited you"), "{}", mail);

    // a too-short password doesn't use up the code
    let mut short = sign_in(
        port,
        SignIn {
            invite: Some(code.clone()),
            new_password: Some("short".into()),
            ..Default::default()
        },
    );
    assert!(refused_with(&mut short, "short password").contains("at least 8"));
    // Bo joins with the code and a password; the email comes from the member record
    let mut b = sign_in(
        port,
        SignIn {
            invite: Some(code.to_lowercase()),
            new_password: Some("bo-secret-22".into()),
            ..Default::default()
        },
    );
    b.wait("B joins", |d| task_names(d).len() == 2);
    assert_eq!(
        (b.member, b.email.as_deref()),
        (Some(2), Some("bo@team.io"))
    );
    a.until("A sees B has an account", |a| {
        a.people
            .iter()
            .any(|p| p.member == 2 && p.account && p.devices == 1)
    });

    // Bo's phone: email and password, no new code; any case of the email
    let mut phone = sign_in(
        port,
        SignIn {
            email: Some(" Bo@Team.io".into()),
            password: Some("bo-secret-22".into()),
            ..Default::default()
        },
    );
    phone.wait("phone gets the workspace", |d| task_names(d).len() == 2);
    assert_eq!(phone.member, Some(2));
    assert!(phone.token.is_some(), "the phone gets its own token");
    a.until("two devices", |a| {
        a.people.iter().any(|p| p.member == 2 && p.devices == 2)
    });
    // and later just the token
    let mut again = sign_in(
        port,
        SignIn {
            device: phone.device.clone(),
            token: phone.token.clone(),
            ..Default::default()
        },
    );
    again.until("token works", |a| a.member == Some(2));

    // wrong passwords, unknown emails: the same answer; too many locks the email
    let mut wrong = sign_in(
        port,
        SignIn {
            email: Some("bo@team.io".into()),
            password: Some("nope-nope".into()),
            ..Default::default()
        },
    );
    let why = refused_with(&mut wrong, "wrong password");
    let mut nobody = sign_in(
        port,
        SignIn {
            email: Some("zed@team.io".into()),
            password: Some("nope-nope".into()),
            ..Default::default()
        },
    );
    assert_eq!(refused_with(&mut nobody, "unknown email"), why);
    for _ in 0..4 {
        let mut w = sign_in(
            port,
            SignIn {
                email: Some("bo@team.io".into()),
                password: Some("nope-nope".into()),
                ..Default::default()
            },
        );
        refused_with(&mut w, "wrong again");
    }
    let mut locked = sign_in(
        port,
        SignIn {
            email: Some("bo@team.io".into()),
            password: Some("bo-secret-22".into()),
            ..Default::default()
        },
    );
    assert!(refused_with(&mut locked, "locked").contains("Too many"));

    // removing access deletes the account too
    a.client.send(Msg::Revoke { member: 2 });
    b.wait_status(|s| matches!(s, Status::Refused(_)), "B signed out");
    a.until("no account", |a| {
        a.people
            .iter()
            .any(|p| p.member == 2 && !p.account && p.devices == 0)
    });

    // nobody else can take an email already in use
    let code = a.invite(3, Role::Member);
    let mut cy = sign_in(
        port,
        SignIn {
            invite: Some(code.clone()),
            email: Some("ann@team.io".into()),
            new_password: Some("cy-secret-33".into()),
            ..Default::default()
        },
    );
    assert!(refused_with(&mut cy, "taken email").contains("already"));
    let mut cy = sign_in(
        port,
        SignIn {
            invite: Some(code),
            email: Some("cy.home@mail.io".into()),
            new_password: Some("cy-secret-33".into()),
            ..Default::default()
        },
    );
    cy.until("C joins with another email", |c| {
        c.email.as_deref() == Some("cy.home@mail.io")
    });
    server.stop();

    // passwords are kept hashed, never as typed
    let state = std::fs::read_to_string(home.join(workboard_server::STATE_FILE)).unwrap();
    assert!(
        state.contains("ann@team.io")
            && !state.contains("ann-secret-1")
            && !state.contains("cy-secret-33")
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_forgotten_password_is_reset_with_an_emailed_code() {
    let home = mail_home("forgot");
    let server = Server::start(&home, 0);
    let port = server.port;
    let mut a = App::start(
        port,
        {
            let mut d = emailed_workspace();
            let mut meta = SyncMeta::default();
            wb_sync::stamp(&d, &mut meta, now());
            d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
            d
        },
        SignIn {
            claim: Some(1),
            email: Some("ann@team.io".into()),
            new_password: Some("ann-secret-1".into()),
            ..Default::default()
        },
    );
    a.until("owner in", |a| a.role == Some(Role::Owner));

    // the same answer whether or not the email has an account
    let (h, body) = http_post(port, "/auth/forgot", r#"{"email": "nobody@team.io"}"#);
    assert!(h.starts_with("HTTP/1.1 200"), "{}", h);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({"ok": true, "mail": true})
    );
    assert_eq!(
        wb_net::forgot_password(&format!("127.0.0.1:{}", port), "ANN@team.io"),
        Ok(true)
    );
    let mail = wait_for_mail(&home, "ann@team.io", 1);
    assert!(mail.contains("password reset code"), "{}", mail);
    let code = code_in(&mail);
    assert!(outbox(&home, "nobody@team.io").is_empty());
    // asking again within a minute sends nothing new
    wb_net::forgot_password(&format!("127.0.0.1:{}", port), "ann@team.io").unwrap();

    // a wrong code fails; the right one sets the new password and signs in
    let mut bad = sign_in(
        port,
        SignIn {
            email: Some("ann@team.io".into()),
            reset_code: Some("AAAA-BBBB-CCCC".into()),
            new_password: Some("ann-secret-2".into()),
            ..Default::default()
        },
    );
    assert!(refused_with(&mut bad, "wrong code").contains("reset code"));
    let mut laptop = sign_in(
        port,
        SignIn {
            email: Some("ann@team.io".into()),
            reset_code: Some(code.clone()),
            new_password: Some("ann-secret-2".into()),
            ..Default::default()
        },
    );
    laptop.until("reset signs in", |l| {
        l.member == Some(1) && l.role == Some(Role::Owner)
    });
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(outbox(&home, "ann@team.io").len(), 1, "one mail only");
    // the code worked once; the old password is gone
    let mut reuse = sign_in(
        port,
        SignIn {
            email: Some("ann@team.io".into()),
            reset_code: Some(code),
            new_password: Some("ann-secret-3".into()),
            ..Default::default()
        },
    );
    refused_with(&mut reuse, "code used");
    let mut old = sign_in(
        port,
        SignIn {
            email: Some("ann@team.io".into()),
            password: Some("ann-secret-1".into()),
            ..Default::default()
        },
    );
    refused_with(&mut old, "old password");
    let mut new = sign_in(
        port,
        SignIn {
            email: Some("ann@team.io".into()),
            password: Some("ann-secret-2".into()),
            ..Default::default()
        },
    );
    new.until("new password", |n| n.member == Some(1));
    server.stop();

    // without mail.json the server says it can't email
    std::fs::remove_file(home.join(workboard_server::MAIL_FILE)).unwrap();
    let server = Server::start(&home, 0);
    assert_eq!(
        wb_net::forgot_password(&format!("127.0.0.1:{}", server.port), "ann@team.io"),
        Ok(false)
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn someone_signed_in_without_a_password_sets_one_up_later() {
    let home = temp_home("set-password");
    let server = Server::start(&home, 0);
    let port = server.port;
    let mut a = App::connect(port, Some(emailed_workspace()));
    a.until("owner in", |a| a.role == Some(Role::Owner));
    assert_eq!(a.email, None, "no account yet");
    // no mail.json: the code isn't emailed and nothing complains
    a.until("workspace up", |a| a.people.iter().any(|p| p.member == 2));
    let code = a.invite(2, Role::Member);
    assert_eq!(a.mailed.last(), Some(&(2, None, None)));

    // an older app joins with the code alone
    let mut b = App::join(port, &code);
    b.wait("B joins", |d| task_names(d).len() == 2);
    assert_eq!(b.email, None);
    b.client.send(Msg::SetPassword {
        email: "bo@team.io".into(),
        password: "short".into(),
        old_password: None,
    });
    b.until("short refused", |b| {
        b.denied.iter().any(|d| d.contains("at least 8"))
    });
    b.client.send(Msg::SetPassword {
        email: "bo@team.io".into(),
        password: "bo-secret-22".into(),
        old_password: None,
    });
    b.until("account set up", |b| {
        b.email.as_deref() == Some("bo@team.io")
    });
    // changing it needs the current one
    b.denied.clear();
    b.client.send(Msg::SetPassword {
        email: "bo@team.io".into(),
        password: "bo-secret-33".into(),
        old_password: Some("wrong-one".into()),
    });
    b.until("needs the current password", |b| {
        b.denied.iter().any(|d| d.contains("current password"))
    });
    b.email = None;
    b.client.send(Msg::SetPassword {
        email: "bo@team.io".into(),
        password: "bo-secret-33".into(),
        old_password: Some("bo-secret-22".into()),
    });
    b.until("changed", |b| b.email.is_some());
    let mut phone = sign_in(
        port,
        SignIn {
            email: Some("bo@team.io".into()),
            password: Some("bo-secret-33".into()),
            ..Default::default()
        },
    );
    phone.until("phone signs in with the changed password", |p| {
        p.member == Some(2)
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_join_page_opens_the_app_with_the_invite() {
    let home = temp_home("join-page");
    let server = Server::start(&home, 0);
    let (head, body) = http_get(server.port, "/join?code=ABCD-EFGH-JKMN&email=bo%40team.io");
    let body = String::from_utf8(body).unwrap();
    assert!(
        head.starts_with("HTTP/1.1 200") && head.to_lowercase().contains("text/html"),
        "{}",
        head
    );
    // http_get sends "Host: x"
    assert!(
        body.contains(
            r#"href="workboard://join?server=x&amp;code=ABCD-EFGH-JKMN&amp;email=bo%40team.io""#
        ),
        "{}",
        body
    );
    assert!(body.contains("<dd>ABCD-EFGH-JKMN</dd>"));
    let (_, body) = http_get(server.port, "/join?code=%3Cscript%3E");
    assert!(
        !String::from_utf8(body).unwrap().contains("<script>"),
        "what's typed into the link is escaped"
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------- private conversations

fn channel<'a>(d: &'a Value, id: u64) -> Option<&'a Value> {
    d["channels"]
        .as_array()
        .and_then(|a| a.iter().find(|c| c["id"] == id))
}

/// A DM between Ann (1) and Cy (3): Bo (2) doesn't get it, and can't join
/// it, write in it or delete it by sending a doctored copy.
#[test]
fn private_conversations_reach_only_their_members_and_cannot_be_joined() {
    let home = temp_home("private");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({"id": 20, "name": "", "kind": "Direct", "topic": "", "members": [1, 3], "messages": [{"id": 21, "author": 1, "text": "secret", "ts": now()}], "last_read": 0}));
        d["channels"].as_array_mut().unwrap().push(json!({"id": 30, "name": "plans", "kind": "Channel", "private": true, "topic": "", "members": [1, 3], "messages": [{"id": 31, "author": 1, "text": "private plans", "ts": now()}], "last_read": 0}));
    });
    a.change(say("marker 1", 2201));
    b.wait("marker 1 at B", |d| {
        texts(d).contains(&"marker 1".to_string())
    });
    assert!(
        channel(&b.data, 20).is_none() && channel(&b.data, 30).is_none(),
        "B never gets them: {}",
        b.data["channels"]
    );
    assert!(
        !b.data.to_string().contains("secret"),
        "nothing of the DM reaches B"
    );
    // a new connection (welcome snapshot) doesn't carry them either
    let code = a.invite(2, Role::Member);
    let mut b2 = App::join(server.port, &code);
    b2.wait("B's second computer gets the copy", |d| {
        texts(d).contains(&"marker 1".to_string())
    });
    assert!(channel(&b2.data, 20).is_none() && channel(&b2.data, 30).is_none());

    // B sends a copy that adds B to both, writes in the DM and renames the channel
    b.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({"id": 20, "name": "", "kind": "Direct", "topic": "", "members": [1, 2, 3], "messages": [{"id": 21, "author": 1, "text": "secret", "ts": 1}, {"id": 22, "author": 2, "text": "sneaky", "ts": now()}], "last_read": 0}));
        d["channels"].as_array_mut().unwrap().push(json!({"id": 30, "name": "taken over", "kind": "Channel", "private": false, "topic": "", "members": [1, 2, 3], "messages": [], "last_read": 0}));
    });
    b.change(say("marker 2", 3201));
    a.wait("marker 2 at A", |d| {
        texts(d).contains(&"marker 2".to_string())
    });
    assert_eq!(
        channel(&a.data, 20).unwrap()["members"],
        json!([1, 3]),
        "B didn't get into the DM"
    );
    assert_eq!(
        channel(&a.data, 20).unwrap()["messages"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "B's message didn't get in"
    );
    assert_eq!(channel(&a.data, 30).unwrap()["name"], "plans");
    assert_eq!(channel(&a.data, 30).unwrap()["private"], true);
    assert_eq!(channel(&a.data, 30).unwrap()["members"], json!([1, 3]));

    // and deleting them from B's copy deletes nothing for the team
    b.change(|d| {
        d["channels"]
            .as_array_mut()
            .unwrap()
            .retain(|c| c["id"] != 20 && c["id"] != 30);
    });
    assert!(
        b.meta().tombs.contains_key(&20) && b.meta().tombs.contains_key(&21),
        "B's copy did mark them deleted"
    );
    b.change(say("marker 3", 3202));
    a.wait("marker 3 at A", |d| {
        texts(d).contains(&"marker 3".to_string())
    });
    assert!(
        channel(&a.data, 20).is_some_and(|c| c["messages"][0]["text"] == "secret"),
        "the DM is still there"
    );
    assert!(
        channel(&a.data, 30).is_some(),
        "the private channel is still there"
    );
    // the members keep using it
    a.change(|d| {
        let ch = d["channels"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["id"] == 20)
            .unwrap();
        ch["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 23, "author": 1, "text": "still private", "ts": now()}));
    });
    a.change(say("marker 4", 2202));
    b.wait("marker 4 at B", |d| {
        texts(d).contains(&"marker 4".to_string())
    });
    assert!(!b.data.to_string().contains("still private"));
    let canon: Value = {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let c: Value = std::fs::read_to_string(home.join(workboard_server::WORKSPACE_FILE))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);
            if c.to_string().contains("still private") || Instant::now() > until {
                break c;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    assert_eq!(
        channel(&canon, 20).unwrap()["messages"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "the team's copy has both DM messages"
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------------ hello size limit

/// Open a WebSocket by hand and send one text frame of `payload`; returns
/// everything the server sent back before closing (or 5 seconds).
fn raw_first_message(port: u16, payload: &[u8]) -> Vec<u8> {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(b"GET /ws HTTP/1.1\r\nHost: x\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut head = vec![];
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        s.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).to_string();
    assert!(head.starts_with("HTTP/1.1 101"), "{}", head);
    assert!(
        head.contains("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
        "the accept key from RFC 6455: {}",
        head
    );
    // a masked text frame (clients always mask)
    let mut frame = vec![0x81u8, 0x80 | 127];
    frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    let mask = [1u8, 2, 3, 4];
    frame.extend_from_slice(&mask);
    frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    let _ = s.write_all(&frame);
    let mut out = vec![];
    let _ = s.read_to_end(&mut out);
    out
}

#[test]
fn a_huge_message_before_signing_in_is_refused_but_big_copies_go_after() {
    let home = temp_home("hello-size");
    let server = Server::start(&home, 0);
    // a small first message that isn't hello gets an answer
    let small = raw_first_message(server.port, br#"{"type":"ping"}"#);
    assert!(String::from_utf8_lossy(&small).contains("Say hello first"));
    // a big one is cut off without being read
    let mut big = br#"{"type":"hello","device":""#.to_vec();
    big.extend(std::iter::repeat_n(b'x', 200 << 10));
    big.extend_from_slice(br#""}"#);
    let out = raw_first_message(server.port, &big);
    assert!(
        !String::from_utf8_lossy(&out).contains("error"),
        "not even a refusal: {:?}",
        String::from_utf8_lossy(&out)
    );
    // signed in, a copy far over the hello limit goes through
    let (mut a, mut b) = two_apps(server.port);
    let long = "y".repeat(300 << 10);
    let l2 = long.clone();
    a.change(move |d| {
        d["channels"][0]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 2301, "author": 1, "text": l2, "ts": now()}));
    });
    b.wait("the long message at B", |d| texts(d).contains(&long));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ---------------------------------------------------------- claims

/// Sign in with a claim and quit as soon as the welcome comes, before the
/// workspace goes up (the app closed right after pressing Connect).
fn claim_and_quit(port: u16, device: &str) -> Status {
    let client = Client::start(
        &format!("127.0.0.1:{}", port),
        SignIn {
            device: device.into(),
            claim: Some(1),
            ..Default::default()
        },
        Arc::new(|| {}),
    );
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        for e in client.events() {
            match e {
                Event::Welcome { .. } => return Status::Connected,
                Event::Status(s @ Status::Refused(_)) => return s,
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("no answer to the claim");
}

#[test]
fn a_claim_whose_app_quit_can_be_made_again() {
    let home = temp_home("reclaim");
    let server = Server::start_with(
        &home,
        0,
        workboard_server::Options {
            claim_wait: Duration::from_secs(2),
            ..Default::default()
        },
    );
    assert_eq!(claim_and_quit(server.port, "first"), Status::Connected);
    // someone else right away: still refused
    assert!(
        matches!(claim_and_quit(server.port, "other"), Status::Refused(w) if w.contains("setting up"))
    );
    // the same computer may claim again at once
    assert_eq!(claim_and_quit(server.port, "first"), Status::Connected);
    // after claim_wait anyone may
    std::thread::sleep(Duration::from_millis(2200));
    let mut a = App::start(
        server.port,
        {
            let mut d = workspace();
            let mut meta = SyncMeta::default();
            wb_sync::stamp(&d, &mut meta, now());
            d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
            d
        },
        SignIn {
            device: "second".into(),
            claim: Some(1),
            ..Default::default()
        },
    );
    a.until("the new claim is the owner", |a| {
        a.role == Some(Role::Owner)
    });
    let devices = || {
        workboard_server::load_saved(&home)
            .map(|s| s.devices.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let until = Instant::now() + Duration::from_secs(5);
    while !(home.join(workboard_server::WORKSPACE_FILE).is_file() && devices() == ["second"])
        && Instant::now() < until
    {
        a.pump();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(devices(), ["second"], "the old claim is gone");
    assert!(
        home.join(workboard_server::WORKSPACE_FILE).is_file(),
        "the new claim's workspace is on the server"
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------------ broken copies

#[test]
fn a_broken_first_copy_is_ignored_and_the_server_keeps_working() {
    let home = temp_home("broken");
    let server = Server::start(&home, 0);
    let mut a = App::start(
        server.port,
        Value::Null,
        SignIn {
            claim: Some(1),
            ..Default::default()
        },
    );
    a.until("signed in", |a| a.role == Some(Role::Owner));
    for bad in [
        json!("not a workspace"),
        json!([1, 2]),
        json!({"members": [{"id": 1}], "extra": "x"}),
        json!({"members": [{"id": 1}], "extra": {"sync": {"stamps": 5}}}),
    ] {
        a.client.push(bad);
    }
    // the real copy still goes up and comes back
    let mut d = workspace();
    let mut meta = SyncMeta::default();
    wb_sync::stamp(&d, &mut meta, now());
    d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    a.client.push(d);
    let code = a.invite(2, Role::Member);
    let mut b = App::join(server.port, &code);
    b.wait("B gets the real copy", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    assert_eq!(b.data["members"][0]["role"], "Owner");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn only_the_newest_copies_from_before_a_restore_are_kept() {
    let home = temp_home("prune");
    let server = Server::start(&home, 0);
    let (mut a, _b) = two_apps(server.port);
    let dir = home.join(workboard_server::BACKUP_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..25 {
        std::fs::write(
            dir.join(format!("before-restore-20200101-0000{:02}.json", i)),
            "{}",
        )
        .unwrap();
    }
    let until = Instant::now() + Duration::from_secs(5);
    while !home.join(workboard_server::WORKSPACE_FILE).is_file() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    let backup = home.join("manual.json");
    std::fs::copy(home.join(workboard_server::WORKSPACE_FILE), &backup).unwrap();
    let res = workboard_server::admin_request(
        &home,
        &json!({"restore": backup.to_string_lossy()}),
        Duration::from_secs(5),
    )
    .unwrap()
    .expect("answered");
    assert_eq!(res["ok"], true, "{}", res);
    let kept = workboard_server::backup_files(&dir, "before-restore");
    assert_eq!(kept.len(), workboard_server::KEEP_BEFORE_RESTORE);
    assert!(
        !kept[0].ends_with("before-restore-20200101-000000.json"),
        "the oldest went first"
    );
    assert!(
        !kept.last().unwrap().to_string_lossy().contains("20200101"),
        "the new one is kept"
    );
    a.pump();
    // a file that isn't a workspace isn't restored
    std::fs::write(&backup, "[1, 2, 3]").unwrap();
    let res = workboard_server::admin_request(
        &home,
        &json!({"restore": backup.to_string_lossy()}),
        Duration::from_secs(5),
    )
    .unwrap()
    .expect("answered");
    assert_eq!(res["ok"], false, "{}", res);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ---------------------------------------------------- requests and calls

#[test]
fn a_request_made_while_reconnecting_goes_once_back_in() {
    let home = temp_home("queued");
    let server = Server::start(&home, 0);
    let port = server.port;
    let (mut a, _b) = two_apps(port);
    server.stop();
    a.wait_status(
        |s| matches!(s, Status::Reconnecting(_)),
        "A notices the server is gone",
    );
    a.client.send(Msg::CreateInvite {
        member: 3,
        role: Role::Member,
    });
    std::thread::sleep(Duration::from_millis(300));
    let server = Server::start(&home, port);
    a.wait_status(|s| *s == Status::Connected, "A reconnects");
    a.until("the code asked for while offline", |a| {
        a.invites.iter().any(|(m, _)| *m == 3)
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_refused_call_says_which_conversation() {
    let home = temp_home("calldenied");
    let server = Server::start(&home, 0);
    let (_a, b) = two_apps(server.port);
    // voice isn't set up on this server: the refusal names the conversation
    b.client.send(Msg::CallJoin { channel: 10 });
    let until = Instant::now() + Duration::from_secs(10);
    let mut got = None;
    while got.is_none() && Instant::now() < until {
        for e in b.client.events() {
            if let Event::CallDenied { channel, why } = e {
                got = Some((channel, why));
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let (channel, why) = got.expect("a call refusal");
    assert_eq!(channel, 10);
    assert!(why.contains("aren't set up"), "{}", why);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

fn chan_mut(d: &mut Value, id: u64) -> &mut Value {
    d["channels"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["id"] == id)
        .unwrap()
}

/// Bo (2) is taken out of a private channel, and another channel is made
/// private without him: both leave his copy, nothing counts as deleted, and
/// what he sends afterwards deletes nothing for the others.
#[test]
fn someone_taken_out_of_a_private_channel_loses_it_without_deleting_it() {
    let home = temp_home("removed");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({"id": 30, "name": "plans", "kind": "Channel", "private": true, "topic": "", "members": [1, 2, 3], "messages": [{"id": 31, "author": 1, "text": "plans for all three", "ts": now()}], "last_read": 0}));
        d["channels"].as_array_mut().unwrap().push(json!({"id": 40, "name": "open", "kind": "Channel", "topic": "", "members": [1, 3], "messages": [{"id": 41, "author": 3, "text": "open talk", "ts": now()}], "last_read": 0}));
    });
    b.wait("B gets both channels", |d| {
        channel(d, 30).is_some() && channel(d, 40).is_some()
    });

    a.change(|d| chan_mut(d, 30)["members"] = json!([1, 3]));
    a.change(|d| chan_mut(d, 40)["private"] = json!(true));
    a.change(say("marker 1", 2301));
    b.wait("marker 1 at B", |d| {
        texts(d).contains(&"marker 1".to_string())
    });
    assert!(
        channel(&b.data, 30).is_none() && channel(&b.data, 40).is_none(),
        "both left B's copy: {}",
        b.data["channels"]
    );
    let m = b.meta();
    for id in [30, 31, 40, 41] {
        assert!(
            !m.tombs.contains_key(&id)
                && !m.stamps.contains_key(&id)
                && !m.hashes.contains_key(&id),
            "no bookkeeping for {} at B",
            id
        );
    }
    // B keeps working; nothing of it deletes the channels for the others
    b.change(say("marker 2", 3301));
    assert!(!b.meta().tombs.contains_key(&30) && !b.meta().tombs.contains_key(&40));
    a.wait("marker 2 at A", |d| {
        texts(d).contains(&"marker 2".to_string())
    });
    assert!(channel(&a.data, 30).is_some_and(|c| c["messages"][0]["text"] == "plans for all three"));
    assert!(channel(&a.data, 40).is_some_and(|c| c["messages"][0]["text"] == "open talk"));
    // and taken back in, B gets it again
    a.change(|d| chan_mut(d, 30)["members"] = json!([1, 2, 3]));
    b.wait("B back in plans", |d| {
        channel(d, 30).is_some_and(|c| c["messages"][0]["text"] == "plans for all three")
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Ann's Personal space (with its list and task) and her private doc never
/// reach Bo, and nothing Bo sends changes or deletes them; a copy of them
/// Bo still has from an older version leaves his copy.
#[test]
fn personal_spaces_and_private_docs_stay_with_their_owner() {
    let home = temp_home("personal");
    let server = Server::start(&home, 0);
    let mut ws = workspace();
    ws["spaces"] = json!([
        {"id": 50, "name": "Personal", "color": [1, 2, 3], "statuses": [], "folders": [], "lists": [{"id": 51, "name": "Ann's list"}], "private_owner": 1},
        {"id": 60, "name": "Team", "color": [1, 2, 3], "statuses": [], "folders": [], "lists": [{"id": 61, "name": "Team list"}]}
    ]);
    ws["tasks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": 52, "list_id": 51, "name": "Ann's own task", "activity": []}));
    ws["docs"] = json!([
        {"id": 80, "owner": 1, "private": true, "title": "Diary", "body": "dear diary", "created": 0, "updated": 0},
        {"id": 81, "owner": 1, "private": true, "parent": 80, "title": "Diary page", "body": "more", "created": 0, "updated": 0},
        {"id": 82, "owner": 1, "private": false, "title": "Handbook", "body": "for all", "created": 0, "updated": 0}
    ]);
    let mut a = App::connect(server.port, Some(ws));
    a.wait("A's copy on the server", |_| true);
    a.until("A is the owner", |a| a.role == Some(Role::Owner));
    let code = a.invite(2, Role::Member);
    let mut b = App::join(server.port, &code);
    b.wait("B gets the team's copy", |d| {
        d["docs"].as_array().is_some_and(|x| !x.is_empty())
    });
    let text = b.data.to_string();
    assert!(
        !text.contains("Ann's list")
            && !text.contains("Ann's own task")
            && !text.contains("dear diary")
            && !text.contains("Diary page"),
        "B gets none of it: {}",
        text
    );
    assert!(text.contains("Handbook") && text.contains("Team list"));

    // B sends a copy that renames all of them, then one that deletes them
    b.change(|d| {
        d["spaces"].as_array_mut().unwrap().push(json!({"id": 50, "name": "Taken", "color": [1, 2, 3], "statuses": [], "folders": [], "lists": [{"id": 51, "name": "Taken list"}], "private_owner": 2}));
        d["tasks"].as_array_mut().unwrap().push(json!({"id": 52, "list_id": 51, "name": "Taken task", "activity": []}));
        d["docs"].as_array_mut().unwrap().push(json!({"id": 80, "owner": 2, "private": false, "title": "Read it", "body": "", "created": 0, "updated": 0}));
    });
    b.change(|d| {
        d["spaces"]
            .as_array_mut()
            .unwrap()
            .retain(|s| s["id"] != 50);
        d["tasks"].as_array_mut().unwrap().retain(|t| t["id"] != 52);
        d["docs"].as_array_mut().unwrap().retain(|x| x["id"] != 80);
    });
    assert!(b.meta().tombs.contains_key(&50) && b.meta().tombs.contains_key(&52));
    b.change(say("marker 1", 3401));
    a.wait("marker 1 at A", |d| {
        texts(d).contains(&"marker 1".to_string())
    });
    let sp = a.data["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == 50)
        .expect("Ann's space is still there");
    assert_eq!(
        (sp["name"].as_str(), sp["private_owner"].as_u64()),
        (Some("Personal"), Some(1))
    );
    assert!(task_names(&a.data).contains(&"Ann's own task".to_string()));
    let diary = a.data["docs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == 80)
        .expect("the diary is still there");
    assert_eq!(diary["body"], "dear diary");

    // an older app's copy of them leaves B's copy with the next update
    b.data["spaces"].as_array_mut().unwrap().push(json!({"id": 50, "name": "Personal", "color": [1, 2, 3], "statuses": [], "folders": [], "lists": [{"id": 51, "name": "Ann's list"}], "private_owner": 1}));
    a.change(say("marker 2", 2401));
    b.wait("marker 2 at B", |d| {
        texts(d).contains(&"marker 2".to_string())
    });
    assert!(!b.data["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == 50));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_server_whose_workspace_file_went_missing_is_not_up_for_grabs() {
    let home = temp_home("lostfile");
    let options = || workboard_server::Options {
        claim_wait: Duration::from_millis(100),
        ..Default::default()
    };
    let server = Server::start_with(&home, 0, options());
    let port = server.port;
    let (_a, _b) = two_apps(port);
    let until = Instant::now() + Duration::from_secs(5);
    while !home.join(workboard_server::WORKSPACE_FILE).is_file() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    server.stop();
    std::fs::remove_file(home.join(workboard_server::WORKSPACE_FILE)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    // server.json is still there: long past claim_wait, a stranger still can't claim it
    let server = Server::start_with(&home, port, options());
    let mut x = App::start(
        port,
        workspace(),
        SignIn {
            claim: Some(1),
            ..Default::default()
        },
    );
    assert!(refused_with(&mut x, "claim refused").contains("restore"));
    let saved = workboard_server::load_saved(&home).unwrap();
    assert_eq!(
        saved.role_of(1),
        Role::Owner,
        "the owner and their computers are kept"
    );
    assert_eq!(saved.devices.len(), 2);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------------------ iPhone pushes

/// Stands in for APNs: notes what would have gone out, and answers "gone"
/// for the tokens in `gone`.
#[derive(Debug, Default)]
struct FakePush {
    sent: std::sync::Mutex<Vec<(String, String, Value)>>,
    gone: std::sync::Mutex<Vec<String>>,
}

impl workboard_server::push::Sender for FakePush {
    fn send(&self, env: &str, token: &str, payload: &Value) -> workboard_server::push::Outcome {
        self.sent
            .lock()
            .unwrap()
            .push((env.to_string(), token.to_string(), payload.clone()));
        if self.gone.lock().unwrap().iter().any(|t| t == token) {
            workboard_server::push::Outcome::Gone
        } else {
            workboard_server::push::Outcome::Sent
        }
    }
}

impl FakePush {
    /// What went to `token` so far.
    fn to(&self, token: &str) -> Vec<Value> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, t, _)| t == token)
            .map(|(_, _, p)| p.clone())
            .collect()
    }
}

fn wait_for(what: &str, cond: impl Fn() -> bool) {
    let until = Instant::now() + Duration::from_secs(15);
    while Instant::now() < until {
        if cond() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for: {}", what);
}

fn connections(port: u16) -> u64 {
    let (_, body) = http_get(port, "/health");
    serde_json::from_slice::<Value>(&body).unwrap()["connections"]
        .as_u64()
        .unwrap()
}

/// The push tokens server.json holds: device id -> token.
fn saved_tokens(home: &PathBuf) -> Vec<(String, String)> {
    workboard_server::load_saved(home)
        .map(|s| s.push.into_iter().map(|(d, t)| (d, t.token)).collect())
        .unwrap_or_default()
}

/// An iPhone signing in as `member` (with a code from `owner`), sending its
/// push token the way the iOS app does, then going away (app in the background).
fn phone(
    owner: &mut App,
    port: u16,
    member: u64,
    token: &str,
    home: &PathBuf,
) -> (String, Option<String>) {
    let code = owner.invite(member, Role::Member);
    let mut p = App::join(port, &code);
    p.wait("the phone gets the team's copy", |_| true);
    p.client.send(Msg::PushToken {
        token: token.to_string(),
        platform: "ios".into(),
        env: "development".into(),
    });
    let device = p.device.clone();
    wait_for("the token in server.json", || {
        saved_tokens(home).contains(&(device.clone(), token.to_string()))
    });
    let before = connections(port);
    let t = p.token.clone();
    drop(p);
    wait_for("the phone's connection gone", || connections(port) < before);
    (device, t)
}

fn in_general(text: &str, id: u64, author: u64) -> impl FnOnce(&mut Value) + '_ {
    move |d: &mut Value| {
        d["channels"][0]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": id, "author": author, "text": text, "ts": now() / 1000}));
    }
}

#[test]
fn iphones_get_pushes_for_dms_mentions_and_their_threads_while_away() {
    let home = temp_home("push");
    let fake = Arc::new(FakePush::default());
    let opts = || workboard_server::Options {
        push: Some(fake.clone()),
        ..Default::default()
    };
    let server = Server::start_with(&home, 0, opts());
    let port = server.port;
    let (mut a, mut b) = two_apps(port);
    let (ann, bo) = ("a1".repeat(32), "b2".repeat(32));
    phone(&mut a, port, 1, &ann, &home);
    let (bo_device, bo_token) = phone(&mut a, port, 2, &bo, &home);
    assert!(
        fake.sent.lock().unwrap().is_empty(),
        "joining and sending tokens notifies no one"
    );

    // a DM from Ann to Bo: Bo's phone, in the payload the iPhone app reads; never Ann's own
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({"id": 20, "name": "", "kind": "Direct", "topic": "", "members": [1, 2], "messages": [{"id": 21, "author": 1, "text": "lunch?", "ts": now() / 1000}], "last_read": 0}));
    });
    wait_for("the DM pushed to Bo's phone", || fake.to(&bo).len() == 1);
    let sent = fake.sent.lock().unwrap().clone();
    assert_eq!(sent[0].0, "development", "the env the phone said");
    assert_eq!(
        sent[0].2,
        json!({"aps": {"alert": {"title": "Ann", "body": "lunch?"}, "sound": "default", "thread-id": "ch-20"}, "channel": 20})
    );

    // in a channel only who is mentioned; a message without mentions notifies no one
    b.change(in_general("hi everyone", 30, 2));
    b.change(in_general("@Ann can you look", 31, 2));
    wait_for("the mention pushed to Ann's phone", || {
        fake.to(&ann).len() == 1
    });
    assert_eq!(
        fake.to(&ann)[0]["aps"]["alert"],
        json!({"title": "Bo in #general", "body": "@Ann can you look"})
    );
    assert_eq!(fake.to(&bo).len(), 1, "Bo wrote it");

    // a reply in Ann's thread (message 11 is hers): Ann's phone, with the thread
    b.change(|d| {
        d["channels"][0]["messages"][0]["replies"] =
            json!([{"id": 40, "author": 2, "text": "on it", "ts": now() / 1000}]);
    });
    wait_for("the reply pushed to Ann's phone", || {
        fake.to(&ann).len() == 2
    });
    assert_eq!(fake.to(&ann)[1]["parent"], 11);
    assert_eq!(fake.to(&ann)[1]["channel"], 10);

    // Bo's phone back online: it shows notifications itself, so no push
    let mut p = App::start(
        port,
        Value::Null,
        SignIn {
            token: bo_token,
            device: bo_device.clone(),
            ..Default::default()
        },
    );
    p.wait_status(|s| matches!(s, Status::Connected), "the phone connected");
    a.change(|d| {
        d["channels"][1]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 22, "author": 1, "text": "you there?", "ts": now() / 1000}))
    });
    b.wait("the DM at Bo's computer", |d| {
        channel(d, 20).is_some_and(|c| c["messages"].as_array().unwrap().len() == 2)
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(fake.to(&bo).len(), 1, "nothing to a connected phone");
    let before = connections(port);
    drop(p);
    wait_for("the phone's connection gone", || connections(port) < before);

    // APNs says the token is gone (app removed): the server forgets it
    fake.gone.lock().unwrap().push(bo.clone());
    a.change(|d| {
        d["channels"][1]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 23, "author": 1, "text": "ok later", "ts": now() / 1000}))
    });
    wait_for("the token dropped", || {
        !saved_tokens(&home).iter().any(|(_, t)| *t == bo)
    });
    assert_eq!(fake.to(&bo).len(), 2);

    // tokens survive a restart: after it Ann's phone still gets her mentions
    server.stop();
    assert!(
        saved_tokens(&home).iter().any(|(_, t)| *t == ann),
        "kept in server.json"
    );
    let server = Server::start_with(&home, port, opts());
    a.wait_status(
        |s| matches!(s, Status::Connected),
        "A back after the restart",
    );
    b.wait_status(
        |s| matches!(s, Status::Connected),
        "B back after the restart",
    );
    b.change(in_general("@Ann after the restart", 32, 2));
    wait_for("the mention after the restart", || fake.to(&ann).len() == 3);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_server_without_push_set_up_keeps_tokens_and_works_as_before() {
    let home = temp_home("nopush");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let bo = "b2".repeat(32);
    phone(&mut a, server.port, 2, &bo, &home);
    // a token that isn't hex is ignored
    let code = a.invite(3, Role::Member);
    let mut cy = App::join(server.port, &code);
    cy.wait("Cy's copy", |_| true);
    cy.client.send(Msg::PushToken {
        token: "not a token".into(),
        platform: "ios".into(),
        env: "production".into(),
    });
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({"id": 20, "name": "", "kind": "Direct", "topic": "", "members": [1, 2], "messages": [{"id": 21, "author": 1, "text": "lunch?", "ts": now() / 1000}], "last_read": 0}));
    });
    b.wait("the DM at Bo's computer", |d| channel(d, 20).is_some());
    cy.change(in_general("still fine", 30, 3));
    a.wait("Cy's message at A", |d| {
        texts(d).contains(&"still fine".to_string())
    });
    let tokens = saved_tokens(&home);
    assert_eq!(tokens.len(), 1, "only the good token: {:?}", tokens);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------------- field-level merge

#[test]
fn two_people_editing_one_task_at_once_both_keep_their_change() {
    let home = temp_home("fields");
    let server = Server::start(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    // both edit task 100 before seeing the other's change: different fields
    a.change(|d| d["tasks"][0]["name"] = json!("Plan (A)"));
    b.change(|d| d["tasks"][0]["status"] = json!("done"));
    for app in [&mut a, &mut b] {
        app.wait("both changes to the task", |d| {
            d["tasks"][0]["name"] == "Plan (A)" && d["tasks"][0]["status"] == "done"
        });
    }
    // reactions and timers made at the same moment
    a.change(|d| {
        d["channels"][0]["messages"][0]["reactions"] = json!([{"emoji": "👍", "users": [1]}]);
        d["tasks"][1]["timer_started"] = json!(1000);
        d["tasks"][1]["timer_by"] = json!(1);
    });
    b.change(|d| {
        d["channels"][0]["messages"][0]["reactions"] = json!([{"emoji": "🎉", "users": [2]}]);
        d["tasks"][1]["timer_started"] = json!(1005);
        d["tasks"][1]["timer_by"] = json!(2);
    });
    for app in [&mut a, &mut b] {
        app.wait("both reactions and both timers", |d| {
            let r = &d["channels"][0]["messages"][0]["reactions"];
            let t = &d["tasks"][1];
            r.as_array().is_some_and(|r| r.len() == 2)
                && t["timer_by"] == 1
                && t["timers"] == json!([{"member": 2, "start": 1005}])
        });
    }
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}
