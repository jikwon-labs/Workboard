// The server with real connections: a server on a free local port with its
// data in a temporary folder, and "apps" that hold a workspace, note their
// changes the way the app does (wb_sync::stamp) and talk through the same
// client the app uses (wb_net::Client).

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wb_net::{Client, Event, Status};
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
        "workspace": "Team", "spaces": [], "members": [], "me": 1, "next_id": 1, "favorites": [], "docs": [],
        "tasks": [{"id": 100, "name": "Plan", "activity": []}, {"id": 101, "name": "Ship", "activity": []}],
        "channels": [{"id": 10, "name": "general", "kind": "Channel", "topic": "", "members": [], "messages": [{"id": 11, "author": 1, "text": "hi", "ts": 1}], "last_read": 0}],
        "extra": {"sync": {}}
    })
}

/// One app: its own workspace copy and connection.
struct App {
    data: Value,
    client: Client,
    status: Option<Status>,
}

impl App {
    /// `data` is None for someone joining: they take whatever the server has.
    fn connect(port: u16, data: Option<Value>) -> App {
        let mut d = data.unwrap_or(Value::Null);
        if !d.is_null() {
            let mut meta = SyncMeta::default();
            wb_sync::stamp(&d, &mut meta, now());
            d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        }
        let client = Client::start(&format!("127.0.0.1:{}", port), format!("dev{:x}", wb_sync::random_u64()), None, Arc::new(|| {}));
        App { data: d, client, status: None }
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
                Event::Welcome { snapshot, .. } => {
                    if let Some(s) = snapshot {
                        self.take_in(s);
                    }
                    // after (re)connecting the whole copy goes up
                    self.push();
                }
                Event::Update { data, .. } => self.take_in(data),
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

/// A shares a workspace on an empty server, B joins it.
fn two_apps(port: u16) -> (App, App) {
    let mut a = App::connect(port, Some(workspace()));
    a.wait_status(|s| *s == Status::Connected, "A connected");
    let mut b = App::connect(port, None);
    b.wait("B gets the team's copy", |d| task_names(d) == ["Plan", "Ship"]);
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
    drop(a);
    drop(b);
    server.stop();
    assert!(home.join(workboard_server::WORKSPACE_FILE).is_file(), "saved when the server stopped");

    let server = Server::start(&home, 0);
    let mut c = App::connect(server.port, None);
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
