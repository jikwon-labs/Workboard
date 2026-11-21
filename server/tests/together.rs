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
use workboard_server::testing::{temp_home, wait_until, TestServer as Server};

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
    /// changes in this app's copy the server turned down and put back
    put_back: Vec<String>,
    backups: Option<Vec<wb_net::Backup>>,
    restored: Vec<String>,
    /// the members online, from the last presence message
    online: Option<Vec<u64>>,
    /// (conversation, member) for each "typing" heard
    typing: Vec<(u64, u64)>,
    /// typing heard in a thread: (channel, member, thread message)
    thread_typing: Vec<(u64, u64, u64)>,
    /// my scheduled messages, from the last `scheduled` answer
    scheduled: Option<Vec<wb_net::ScheduledMsg>>,
    /// the webhooks and commands, from the last `integrations` answer
    integrations: Option<(Vec<wb_net::HookInfo>, Vec<wb_net::CommandInfo>)>,
    /// account email from the welcome, then from `account` answers
    email: Option<String>,
    /// (member, emailed to, mail error) for each invite answer
    mailed: Vec<(u64, Option<String>, Option<String>)>,
    /// every copy (whole or delta) the server sent after the welcome
    updates: Vec<Value>,
    /// roles (from Access) and copies, in the order they came
    heard: Vec<String>,
    /// my own reading state, from the last `mine` message
    mine: Option<Value>,
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

    /// connect(), to an address as typed in the app.
    fn connect_at(addr: &str, mut d: Value) -> App {
        let mut meta = SyncMeta::default();
        wb_sync::stamp(&d, &mut meta, now());
        d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
        let me = d["me"].as_u64();
        Self::start_at(
            addr,
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

    fn start(port: u16, data: Value, sign_in: SignIn) -> App {
        Self::start_at(&format!("127.0.0.1:{}", port), data, sign_in)
    }

    /// Connect to the server as typed in the app (`addr`).
    fn start_at(addr: &str, data: Value, mut sign_in: SignIn) -> App {
        if sign_in.device.is_empty() {
            sign_in.device = format!("dev{:x}", wb_sync::random_u64());
        }
        // an app of this version (`old_app` tests leave it out)
        if sign_in.app.as_deref() != Some("none") {
            sign_in.app.get_or_insert_with(|| "test".into());
            sign_in.sync.get_or_insert(wb_sync::META_V);
        } else {
            sign_in.app = None;
        }
        let device = sign_in.device.clone();
        let client = Client::start(addr, sign_in, Arc::new(|| {}));
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
            put_back: vec![],
            backups: None,
            restored: vec![],
            online: None,
            typing: vec![],
            thread_typing: vec![],
            scheduled: None,
            integrations: None,
            email: None,
            mailed: vec![],
            updates: vec![],
            heard: vec![],
            mine: None,
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
        let held = wait_until(Duration::from_secs(15), Duration::from_millis(20), || {
            self.pump();
            cond(self)
        });
        if held {
            return;
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
                    self.heard.push(format!("copy {}", data.to_string().len()));
                    self.updates.push(data.clone());
                    self.take_in(data);
                    wb_sync::hide(&mut self.data, &hidden);
                }
                Event::Invite {
                    member,
                    code,
                    emailed,
                    mail_error,
                } => {
                    // like the Mac (together.rs): the answer is the only
                    // word of it this connection gets, so the code goes
                    // into the list it has
                    if let Some(p) = self.people.iter_mut().find(|p| p.member == member) {
                        p.invite = Some(code.clone());
                    }
                    self.invites.push((member, code));
                    self.mailed.push((member, emailed, mail_error));
                }
                Event::LinkCode { code, .. } => {
                    self.heard.push(format!("link code {}", code));
                }
                Event::Account(e) => self.email = Some(e),
                Event::Access { people, role } => {
                    self.heard.push(format!("role {:?}", role));
                    self.people = people;
                    self.role = Some(role);
                }
                Event::Denied(why) | Event::CallDenied { why, .. } => self.denied.push(why),
                Event::PutBack(why) => self.put_back.push(why),
                Event::CallToken { .. } => {}
                Event::Backups(b) => self.backups = Some(b),
                Event::Restored(n) => self.restored.push(n),
                Event::Presence(o) => self.online = Some(o),
                Event::Typing {
                    channel,
                    member,
                    thread: None,
                } => self.typing.push((channel, member)),
                Event::Typing {
                    channel,
                    member,
                    thread: Some(t),
                } => self.thread_typing.push((channel, member, t)),
                Event::Scheduled(items) => self.scheduled = Some(items),
                Event::Mine(m) => self.mine = Some(m),
                Event::Integrations { hooks, commands } => {
                    self.integrations = Some((hooks, commands))
                }
            }
        }
    }

    fn wait(&mut self, what: &str, cond: impl Fn(&Value) -> bool) {
        let held = wait_until(Duration::from_secs(15), Duration::from_millis(20), || {
            self.pump();
            !self.data.is_null() && cond(&self.data)
        });
        if held {
            return;
        }
        panic!(
            "timed out waiting for: {} (status {:?})\n{}",
            what, self.status, self.data
        );
    }

    fn wait_status(&mut self, want: fn(&Status) -> bool, what: &str) {
        let held = wait_until(Duration::from_secs(15), Duration::from_millis(20), || {
            self.pump();
            self.status.as_ref().is_some_and(want)
        });
        if held {
            return;
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

/// `say` as member `author` (a plain member can't post as someone else).
fn say_as(text: &str, id: u64, author: u64) -> impl FnOnce(&mut Value) + '_ {
    move |d: &mut Value| {
        d["channels"][0]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": id, "author": author, "text": text, "ts": now()}));
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

/// Another device of `a`'s person, signed in with a code `a` shows (the
/// phone link), holding the team's copy.
fn second_device(port: u16, a: &mut App) -> App {
    a.client.send(Msg::CreateLinkCode);
    a.until("a link code", |a| {
        a.heard.iter().any(|h| h.starts_with("link code "))
    });
    let code = a
        .heard
        .iter()
        .find_map(|h| h.strip_prefix("link code "))
        .unwrap()
        .to_string();
    let mut phone = App::start(
        port,
        Value::Null,
        SignIn {
            link_code: Some(code),
            ..Default::default()
        },
    );
    phone.wait("the second device gets the copy", |_| true);
    assert_eq!(phone.member, a.member, "the same person");
    phone
}

/// A person's reading state and notification choices go to their other
/// devices, never to anyone else, and stay on the server over a restart.
#[test]
fn my_reading_state_reaches_my_other_devices_only() {
    use wb_sync::mine::{collect, resolve, MineView};
    let home = temp_home("mine");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let mut phone = second_device(server.port, &mut a);
    // every connection hears what the server keeps right after welcome
    a.until("A's state", |a| a.mine.is_some());
    phone.until("the phone's state", |p| p.mine.is_some());
    b.until("B's state", |b| b.mine.is_some());
    assert!(resolve(phone.mine.as_ref().unwrap())
        .inbox_cleared
        .is_empty());

    // the Mac clears an Inbox row, reads #general and mutes notifications
    let mut mac = MineView::default();
    mac.inbox_cleared.insert("100:5:2".into());
    mac.read.insert(10, 1);
    mac.settings
        .insert("muted_until".into(), json!(4_000_000_000i64));
    let sent = collect(&mac, &json!({}), now());
    a.client.send(Msg::Mine { data: sent.clone() });
    phone.until("the phone gets the Mac's state", |p| {
        let v = resolve(p.mine.as_ref().unwrap());
        v.inbox_cleared.contains("100:5:2")
            && v.read.get(&10) == Some(&1)
            && v.settings.get("muted_until") == Some(&json!(4_000_000_000i64))
    });

    // the phone puts the row back and marks a reply read
    let mut on_phone = resolve(phone.mine.as_ref().unwrap());
    on_phone.inbox_cleared.clear();
    on_phone.replies_read.insert("chat:10:11".into());
    let known = phone.mine.clone().unwrap();
    phone.client.send(Msg::Mine {
        data: collect(&on_phone, &known, now() + 1),
    });
    a.until("the Mac gets the phone's change", |a| {
        let v = resolve(a.mine.as_ref().unwrap());
        v.inbox_cleared.is_empty() && v.replies_read.contains("chat:10:11")
    });

    // someone else never hears any of it
    b.change(say_as("ping", 3100, 2));
    a.wait("B's message at A", |d| {
        texts(d).contains(&"ping".to_string())
    });
    b.pump();
    assert_eq!(
        resolve(b.mine.as_ref().unwrap()),
        MineView::default(),
        "B's own state stays empty"
    );

    // a device that is behind is told what it lacks
    let stale = collect(&MineView::default(), &json!({}), 1);
    a.mine = None;
    a.client.send(Msg::Mine { data: stale });
    a.until("the server answers with what it keeps", |a| {
        a.mine
            .as_ref()
            .is_some_and(|m| resolve(m).replies_read.contains("chat:10:11"))
    });

    let (token, device) = (phone.token.clone(), phone.device.clone());
    drop(a);
    drop(b);
    drop(phone);
    server.stop();
    let server = Server::start_at(&home, 0);
    let mut again = App::start(
        server.port,
        Value::Null,
        SignIn {
            token,
            device,
            ..Default::default()
        },
    );
    again.until("kept over a restart", |p| {
        p.mine
            .as_ref()
            .is_some_and(|m| resolve(m).replies_read.contains("chat:10:11"))
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// A scheduled message waits on the server, then goes out as its writer's
/// (for everyone, even with the writer gone); one taken back never does.
#[test]
fn scheduled_messages_go_out_at_their_time_from_the_server() {
    let home = temp_home("schedule");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let now = now() / 1000;
    let item = |id: u64, text: &str, at: i64| wb_net::ScheduledMsg {
        id,
        channel: 10,
        parent: None,
        text: text.into(),
        at,
        mentions: vec![],
    };
    b.client.send(Msg::Schedule {
        item: item(7001, "good morning", now + 2),
    });
    b.until("the list comes back", |b| {
        b.scheduled.as_ref().is_some_and(|l| l.len() == 1)
    });
    b.client.send(Msg::Schedule {
        item: item(7002, "never mind", now + 2),
    });
    b.until("two waiting", |b| {
        b.scheduled.as_ref().is_some_and(|l| l.len() == 2)
    });
    b.client.send(Msg::Unschedule { id: 7002 });
    b.until("one left", |b| {
        b.scheduled.as_ref().is_some_and(|l| l.len() == 1)
    });
    // a conversation that isn't there is turned down
    b.client.send(Msg::Schedule {
        item: wb_net::ScheduledMsg {
            channel: 999,
            ..item(7003, "nowhere", now + 2)
        },
    });
    b.until("turned down", |b| !b.denied.is_empty());
    // the writer goes away; it still goes out, as theirs
    drop(b);
    a.wait("the scheduled message at A", |d| {
        texts(d).contains(&"good morning".to_string())
    });
    let m = a.data["channels"][0]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == 7001)
        .unwrap()
        .clone();
    assert_eq!(m["author"], 2);
    std::thread::sleep(Duration::from_millis(1500));
    assert!(!texts(&a.data).contains(&"never mind".to_string()));
    assert!(workboard_server::load_saved(&home)
        .unwrap()
        .scheduled
        .is_empty());
}

#[test]
fn a_chat_message_from_a_reaches_b() {
    let home = temp_home("chat");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(say("hello from A", 2001));
    b.wait("A's message at B", |d| {
        texts(d).contains(&"hello from A".to_string())
    });
    b.change(say_as("and from B", 3001, 2));
    a.wait("B's answer at A", |d| {
        texts(d).ends_with(&["hello from A".to_string(), "and from B".to_string()])
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn edits_to_different_tasks_at_once_both_stay() {
    let home = temp_home("edits");
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| {
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t["id"] != 101);
    });
    b.wait("delete reaches B", |d| task_names(d) == ["Plan"]);
    // B keeps working and sends its whole copy again
    b.change(say_as("still here", 3002, 2));
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
    b.change(say_as("after the stale copy", 3003, 2));
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
    let server = Server::start_at(&home, 0);
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

    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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

    let server = Server::start_at(&home, port);
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
fn everyone_sees_who_is_connected() {
    let home = temp_home("presence");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    b.until("B sees A and itself online", |b| {
        b.online.as_deref() == Some(&[1, 2])
    });
    a.until("A hears B came online", |a| {
        a.online.as_deref() == Some(&[1, 2])
    });
    drop(b);
    a.until("A hears B went away", |a| a.online.as_deref() == Some(&[1]));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn typing_reaches_only_the_others_who_see_the_conversation() {
    let home = temp_home("typing");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let code = a.invite(3, Role::Member);
    let mut c = App::join(server.port, &code);
    c.wait("C gets the team's copy", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    // a direct message between A and B
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({
            "id": 20, "name": "", "kind": "Direct", "topic": "", "members": [1, 2],
            "messages": [], "last_read": 0
        }))
    });
    b.wait("the DM at B", |d| {
        d["channels"].as_array().unwrap().len() == 2
    });
    // typing in the DM, then in #general
    let typing = |channel| Msg::Typing {
        channel,
        member: None,
        thread: None,
    };
    a.client.send(typing(20));
    a.client.send(typing(10));
    b.until("B hears A typing in both", |b| {
        b.typing == [(20, 1), (10, 1)]
    });
    c.until("C hears A typing in #general", |c| !c.typing.is_empty());
    assert_eq!(c.typing, [(10, 1)], "C isn't in the DM");
    // sent again right away it isn't passed on twice; nobody hears their own
    a.client.send(typing(10));
    b.client.send(typing(10));
    a.until("A hears B", |a| !a.typing.is_empty());
    assert_eq!(a.typing, [(10, 2)], "A doesn't hear its own typing");
    c.until("C hears B", |c| c.typing.len() == 2);
    assert_eq!(c.typing, [(10, 1), (10, 2)]);
    // a conversation that isn't there or that B isn't in goes nowhere
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({
            "id": 30, "name": "", "kind": "Direct", "topic": "", "members": [1, 3],
            "messages": [], "last_read": 0
        }))
    });
    c.wait("the A-C DM at C", |d| {
        d["channels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == 30)
    });
    b.client.send(typing(30));
    b.client.send(typing(99));
    b.client.send(typing(20));
    a.until("A hears B in their DM", |a| a.typing.len() == 2);
    assert_eq!(a.typing, [(10, 2), (20, 2)]);
    std::thread::sleep(Duration::from_millis(300));
    c.pump();
    assert_eq!(
        c.typing,
        [(10, 1), (10, 2)],
        "B can't type into A and C's DM"
    );
    // typing in a thread of #general goes on with the thread, right after
    // typing in the channel itself (another place, so not held back)
    a.client.send(Msg::Typing {
        channel: 10,
        member: None,
        thread: Some(77),
    });
    b.until("B hears A in the thread", |b| !b.thread_typing.is_empty());
    assert_eq!(b.thread_typing, [(10, 1, 77)]);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn only_invited_people_get_in_and_only_admins_invite() {
    let home = temp_home("access");
    let server = Server::start_at(&home, 0);
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
    http_get_with(port, path, "Host: x\r\n")
}

/// http_get with these header lines (each ending in \r\n) in place of "Host: x".
fn http_get_with(port: u16, path: &str, headers: &str) -> (String, Vec<u8>) {
    use std::io::{Read, Write};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(
        format!(
            "GET {} HTTP/1.1\r\n{}Connection: close\r\n\r\n",
            path, headers
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
fn the_privacy_policy_page_is_served_to_anyone() {
    let home = temp_home("privacy");
    let server = Server::start_at(&home, 0);
    // no sign-in, and from the internet too (App Store Connect links here)
    for headers in [
        "Host: x\r\n",
        "Host: wb.example\r\nCF-Connecting-IP: 1.2.3.4\r\n",
    ] {
        let (head, body) = http_get_with(server.port, "/privacy", headers);
        assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
        assert!(head.to_ascii_lowercase().contains("text/html"), "{}", head);
        let body = String::from_utf8(body).unwrap();
        assert!(body.contains("개인정보 처리방침"));
        assert!(body.contains("Privacy Policy"));
    }
}

#[test]
fn app_updates_are_served_from_the_updates_folder() {
    let home = temp_home("updates");
    let server = Server::start_at(&home, 0);
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
fn download_links_always_give_the_newest_app() {
    let home = temp_home("downloads");
    let server = Server::start_at(&home, 0);
    // nothing published yet: a page that says so, for either computer
    for (path, os) in [("/download/mac", "Mac"), ("/download/windows", "Windows")] {
        let (head, body) = http_get(server.port, path);
        assert!(head.starts_with("HTTP/1.1 200"), "{} -> {}", path, head);
        assert!(head.to_lowercase().contains("text/html"), "{}", head);
        let body = String::from_utf8(body).unwrap();
        assert!(
            body.contains(&format!("No Workboard for {} yet", os)),
            "{}",
            body
        );
    }
    let updates = home.join("updates");
    std::fs::create_dir_all(&updates).unwrap();
    for name in [
        "appcast.xml",
        "Workboard-0.1.9-arm64.zip",
        "Workboard-0.1.10-arm64.zip",
        "Workboard-0.1.2-arm64.zip",
    ] {
        std::fs::write(updates.join(name), name).unwrap();
    }
    // the newest by number, under its own name so it's saved with the version
    let (head, _) = http_get(server.port, "/download/mac");
    assert!(head.starts_with("HTTP/1.1 302"), "{}", head);
    let lower = head.to_lowercase();
    assert!(
        lower.contains("location: /updates/workboard-0.1.10-arm64.zip"),
        "{}",
        head
    );
    assert!(lower.contains("cache-control: no-store"), "{}", head);
    let (head, body) = http_get(server.port, "/updates/Workboard-0.1.10-arm64.zip");
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    assert_eq!(body, b"Workboard-0.1.10-arm64.zip");
    // the Mac zip is not the Windows one
    let (head, body) = http_get(server.port, "/download/windows");
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    assert!(String::from_utf8(body)
        .unwrap()
        .contains("No Workboard for Windows yet"));
    std::fs::write(updates.join("Workboard-0.1.11-windows-x64.zip"), "w").unwrap();
    let (head, _) = http_get(server.port, "/download/windows");
    assert!(head.starts_with("HTTP/1.1 302"), "{}", head);
    assert!(
        head.to_lowercase()
            .contains("location: /updates/workboard-0.1.11-windows-x64.zip"),
        "{}",
        head
    );
    let (head, _) = http_get(server.port, "/download/linux");
    assert!(head.starts_with("HTTP/1.1 404"), "{}", head);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn email_invite_cli_creates_member_and_reaches_connected_apps() {
    let home = temp_home("email-invite-cli");
    let server = Server::start_at(&home, 0);
    let mut owner = App::connect(server.port, Some(workspace()));
    owner.until("owner ready", |a| a.role == Some(Role::Owner));
    owner.wait("workspace ready", |d| d["workspace"] == "Team");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_workboard-server"))
        .args(["invite-email", "new.member@example.test", "--home"])
        .arg(&home)
        .output()
        .unwrap();
    assert!(output.status.success(), "invite-email must succeed");
    owner.wait("new member broadcast", |d| {
        d["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["email"] == "new.member@example.test")
    });
    let member = owner.data["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["email"] == "new.member@example.test")
        .unwrap();
    let id = member["id"].as_u64().unwrap();
    assert_eq!(member["role"], "Member");
    assert_eq!(member["invited"], true);
    assert!(member["color"].is_array());
    assert!(owner.data["channels"][0]["members"]
        .as_array()
        .unwrap()
        .contains(&json!(id)));
    let saved = workboard_server::load_saved(&home).unwrap();
    let code = saved
        .open_invite(id, now())
        .expect("persisted invite")
        .clone();
    assert_eq!(saved.role_of(id), Role::Member);
    assert_eq!(saved.invites[&code].role, Role::Member);
    let disk: Value = serde_json::from_str(
        &std::fs::read_to_string(home.join(workboard_server::WORKSPACE_FILE)).unwrap(),
    )
    .unwrap();
    assert!(disk["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == id));
    drop(owner);
    server.stop();
    let restarted = Server::start_at(&home, 0);
    let mut joined = App::join(restarted.port, &code);
    joined.until("member after restart", |a| {
        a.member == Some(id) && a.role == Some(Role::Member)
    });
    assert!(joined.data["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == id));
    let mut reused = App::join(restarted.port, &code);
    reused.until("code works only once", |a| {
        matches!(a.status, Some(Status::Refused(_)))
    });
    drop((joined, reused));
    restarted.stop();
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn email_invite_cli_storage_failure_preserves_workspace_and_pending_codes() {
    for file in [
        workboard_server::STATE_FILE,
        workboard_server::WORKSPACE_FILE,
    ] {
        for reinvite in [false, true] {
            let home = temp_home("email-invite-write-failure");
            let server = Server::start_at(&home, 0);
            let mut owner = App::connect(server.port, Some(workspace()));
            owner.until("owner ready", |a| a.role == Some(Role::Owner));
            let old_code = if reinvite {
                let first = local_admin(&home, json!({"invite_email": "pending@example.test"}));
                assert_eq!(first["ok"], true);
                owner.wait("initial invitation", |d| {
                    d["members"].as_array().unwrap().len() == 4
                });
                Some(first["code"].as_str().unwrap().to_string())
            } else {
                None
            };
            // A harmless admin request is also a persistence barrier for setup.
            local_admin(&home, json!({"people": true}));
            owner.pump();
            let updates_before = owner.updates.len();
            let workspace_path = home.join(workboard_server::WORKSPACE_FILE);
            let saved_path = home.join(workboard_server::STATE_FILE);
            let before_workspace = std::fs::read(&workspace_path).unwrap();
            let before_saved = std::fs::read(&saved_path).unwrap();
            let blocked = home.join(file).with_extension("json.tmp");
            std::fs::create_dir(&blocked).unwrap();
            let output = std::process::Command::new(env!("CARGO_BIN_EXE_workboard-server"))
                .args(["invite-email", "pending@example.test", "--home"])
                .arg(&home)
                .output()
                .unwrap();
            assert!(
                !output.status.success(),
                "invite-email must fail when {file} cannot be staged (reinvite={reinvite}); it printed {:?} {:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                !String::from_utf8_lossy(&output.stdout).contains("Invite code"),
                "failed CLI must not deliver a credential"
            );
            let res = local_admin(&home, json!({"invite_email": "pending@example.test"}));
            assert_eq!(res["ok"], false);
            assert!(res.get("code").is_none(), "failed admin result has no code");
            assert!(res["message"].as_str().unwrap().contains("save"));
            assert!(
                std::fs::read(&workspace_path).unwrap() == before_workspace,
                "no partial workspace write"
            );
            assert!(
                std::fs::read(&saved_path).unwrap() == before_saved,
                "no code rotation or partial saved-state write"
            );
            owner.pump();
            assert_eq!(
                owner.updates.len(),
                updates_before,
                "no failed-invite broadcast"
            );
            std::fs::remove_dir(&blocked).unwrap();
            // A later flush must not publish any latent in-memory mutation.
            local_admin(&home, json!({"people": true}));
            assert!(std::fs::read(&workspace_path).unwrap() == before_workspace);
            assert!(std::fs::read(&saved_path).unwrap() == before_saved);
            if file == workboard_server::STATE_FILE {
                if let Some(code) = &old_code {
                    let mut joined = App::join(server.port, code);
                    joined.until("old code still works in live memory", |a| {
                        a.role == Some(Role::Member)
                    });
                    drop(joined);
                }
            }
            drop(owner);
            server.stop();
            if file == workboard_server::WORKSPACE_FILE {
                if let Some(code) = &old_code {
                    let restarted = Server::start_at(&home, 0);
                    let mut joined = App::join(restarted.port, code);
                    joined.until("old code still works after restart", |a| {
                        a.role == Some(Role::Member)
                    });
                    let mut reused = App::join(restarted.port, code);
                    reused.until("preserved code remains single use", |a| {
                        matches!(a.status, Some(Status::Refused(_)))
                    });
                    drop((joined, reused));
                    restarted.stop();
                }
            }
            let _ = std::fs::remove_dir_all(home);
        }
    }
}

#[test]
fn email_invite_survives_stale_client_push_with_concurrent_general_edits() {
    let home = temp_home("email-stale-push");
    let server = Server::start_at(&home, 0);
    let mut owner = App::connect(server.port, Some(workspace()));
    owner.until("owner ready", |a| a.role == Some(Role::Owner));
    local_admin(&home, json!({"people": true}));
    // Do not pump the owner's queued invitation broadcast: their next push
    // still has the old member list and old general membership.
    let res = local_admin(&home, json!({"invite_email": "concurrent@example.test"}));
    assert_eq!(res["ok"], true);
    let id = res["member"].as_u64().unwrap();
    assert_eq!(owner.data["members"].as_array().unwrap().len(), 3);
    owner.change(|d| {
        d["channels"][0]["topic"] = json!("concurrent topic");
        d["tasks"][0]["name"] = json!("concurrent task");
    });
    owner.wait("stale client edit merged", |d| {
        d["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == id)
            && d["channels"][0]["topic"] == "concurrent topic"
            && d["tasks"][0]["name"] == "concurrent task"
    });
    // The client's own edit is already present locally; observe the server's
    // result independently through a fresh sign-in using the invitation.
    let mut joined = App::join(server.port, res["code"].as_str().unwrap());
    joined.until("Member joins merged workspace", |a| {
        a.role == Some(Role::Member)
    });
    joined.wait("server retained both concurrent edits", |d| {
        d["channels"][0]["topic"] == "concurrent topic"
            && d["tasks"][0]["name"] == "concurrent task"
    });
    assert!(joined.data["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == id));
    assert!(joined.data["channels"][0]["members"]
        .as_array()
        .unwrap()
        .contains(&json!(id)));
    local_admin(&home, json!({"people": true}));
    let disk: Value = serde_json::from_slice(
        &std::fs::read(home.join(workboard_server::WORKSPACE_FILE)).unwrap(),
    )
    .unwrap();
    assert!(disk["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["id"] == id));
    assert!(disk["channels"][0]["members"]
        .as_array()
        .unwrap()
        .contains(&json!(id)));
    assert_eq!(disk["channels"][0]["topic"], "concurrent topic");
    assert_eq!(disk["tasks"][0]["name"], "concurrent task");
    drop((owner, joined));
    server.stop();
    let _ = std::fs::remove_dir_all(home);
}

/// How long a test waits for what the server does on its once-a-second tick:
/// saving the workspace, backups, and answering a request left in its folder.
/// With the whole suite running beside a live server (the Mac mini's
/// auto-update) that has taken over five seconds. The waits end as soon as it
/// happens, so a long limit costs nothing when all is well.
const TICK_WAIT: Duration = Duration::from_secs(30);

fn local_admin(home: &PathBuf, request: Value) -> Value {
    workboard_server::admin_request(home, &request, TICK_WAIT)
        .unwrap()
        .expect("local admin answered")
}

#[test]
fn email_invite_reuses_unregistered_member_case_insensitively_with_fresh_code() {
    let home = temp_home("email-reinvite");
    let server = Server::start_at(&home, 0);
    let mut data = workspace();
    data["members"][1]["email"] = json!("Pending.Member@example.test");
    data["members"][1]["invited"] = json!(true);
    let mut owner = App::connect(server.port, Some(data));
    owner.until("ready", |a| a.role == Some(Role::Owner));
    let first = local_admin(
        &home,
        json!({"invite_email": " pending.member@EXAMPLE.TEST "}),
    );
    assert_eq!(first["ok"], true);
    assert_eq!(first["member"], 2, "reuse the existing identity");
    let second = local_admin(
        &home,
        json!({"invite_email": "PENDING.MEMBER@example.test"}),
    );
    assert_eq!(second["member"], 2);
    assert_ne!(first["code"], second["code"]);
    let saved = workboard_server::load_saved(&home).unwrap();
    assert_eq!(saved.invites.values().filter(|i| i.member == 2).count(), 1);
    assert!(!saved.invites.contains_key(first["code"].as_str().unwrap()));
    let mut old = App::join(server.port, first["code"].as_str().unwrap());
    old.until("old code invalid", |a| {
        matches!(a.status, Some(Status::Refused(_)))
    });
    let mut joined = App::join(server.port, second["code"].as_str().unwrap());
    joined.until("fresh code valid", |a| {
        a.member == Some(2) && a.role == Some(Role::Member)
    });
    assert_eq!(joined.data["members"].as_array().unwrap().len(), 3);
    drop((owner, old, joined));
    server.stop();
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn email_invite_protects_privileged_registered_and_ambiguous_identities() {
    let home = temp_home("email-protected");
    std::fs::create_dir_all(&home).unwrap();
    let mut data = workspace();
    data["members"] = json!([
        {"id": 1, "email": "owner@example.test", "role": "Owner"},
        {"id": 2, "email": "admin@example.test", "role": "Admin"},
        {"id": 3, "email": "saved-admin@example.test", "role": "Member"},
        {"id": 4, "email": "registered@example.test", "role": "Member"},
        {"id": 5, "email": "device@example.test", "role": "Member"},
        {"id": 6, "email": "duplicate@example.test", "role": "Member"},
        {"id": 7, "email": "DUPLICATE@example.test", "role": "Member"},
        {"id": 8, "email": "pending-admin@example.test", "role": "Member"},
        {"id": 9, "email": "guest@example.test", "role": "Guest"}
    ]);
    let mut saved = workboard_server::Saved::default();
    saved.roles.insert(1, Role::Owner);
    saved.roles.insert(3, Role::Admin);
    saved.accounts.insert(
        4,
        workboard_server::Account {
            email: "account-alias@example.test".into(),
            salt: String::new(),
            hash: String::new(),
            rounds: 1,
            created: 1,
        },
    );
    saved.devices.insert(
        "test-device".into(),
        workboard_server::Device {
            tag: 1,
            member: 5,
            token: String::new(),
            created: 1,
        },
    );
    saved.make_invite(8, Role::Admin);
    std::fs::write(
        home.join(workboard_server::WORKSPACE_FILE),
        data.to_string(),
    )
    .unwrap();
    std::fs::write(
        home.join(workboard_server::STATE_FILE),
        serde_json::to_vec(&saved).unwrap(),
    )
    .unwrap();
    let server = Server::start_at(&home, 0);
    for email in [
        "OWNER@example.test",
        "admin@example.test",
        "saved-admin@example.test",
        "registered@example.test",
        "ACCOUNT-ALIAS@example.test",
        "device@example.test",
        "duplicate@example.test",
        "pending-admin@example.test",
        "guest@example.test",
    ] {
        let res = local_admin(&home, json!({"invite_email": email}));
        assert_eq!(res["ok"], false, "must protect {email}");
        assert!(
            res.get("code").is_none(),
            "no sign-in credential on rejection"
        );
    }
    server.stop();
    let disk: Value = serde_json::from_slice(
        &std::fs::read(home.join(workboard_server::WORKSPACE_FILE)).unwrap(),
    )
    .unwrap();
    assert_eq!(disk, data, "no identities or canon roles changed");
    assert_eq!(
        serde_json::to_value(workboard_server::load_saved(&home).unwrap()).unwrap(),
        serde_json::to_value(saved).unwrap(),
        "all accounts, roles, devices and codes unchanged"
    );
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn email_invite_rejects_invalid_addresses_before_queuing_and_on_server() {
    let home = temp_home("email-invalid");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_workboard-server"))
        .args(["invite-email", "not-an-email", "--home"])
        .arg(&home)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "invalid email must fail before queuing"
    );
    assert!(
        !home.exists(),
        "invalid CLI input must not create a pending request"
    );
    let server = Server::start_at(&home, 0);
    let mut owner = App::connect(server.port, Some(workspace()));
    owner.until("ready", |a| a.role == Some(Role::Owner));
    for email in [
        "",
        "a",
        "a@b",
        "@example.test",
        "a@@example.test",
        "a b@example.test",
        "a@example..test",
        "a@-example.test",
        "a@example-.test",
        "a@.example.test",
        ".a@example.test",
        "a..b@example.test",
        "a.@example.test",
        "Name <a@example.test>",
        "a@example.test\r\nBcc: b@example.test",
        "a@example.test\0",
        "a@example.test\n",
    ] {
        assert_eq!(
            local_admin(&home, json!({"invite_email": email}))["ok"],
            false,
            "reject malformed input"
        );
    }
    for email in [
        format!("{}@example.test", "a".repeat(65)),
        format!("a@{}.test", "b".repeat(64)),
    ] {
        assert_eq!(
            local_admin(&home, json!({"invite_email": email}))["ok"],
            false
        );
    }
    let valid = local_admin(
        &home,
        json!({"invite_email": " New.Person+tag@EXAMPLE.TEST "}),
    );
    assert_eq!(valid["ok"], true);
    owner.wait("normalized address", |d| {
        d["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["email"] == "new.person+tag@example.test")
    });
    assert_eq!(owner.data["members"].as_array().unwrap().len(), 4);
    drop(owner);
    server.stop();
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn email_invite_cli_and_admin_explicitly_refuse_role_overrides() {
    let home = temp_home("email-role");
    for role in ["owner", "admin", "member", "invalid"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_workboard-server"))
            .args(["invite-email", "new@example.test", "--role", role, "--home"])
            .arg(&home)
            .output()
            .unwrap();
        assert!(!output.status.success(), "invite-email must refuse --role");
        assert!(String::from_utf8_lossy(&output.stderr).contains("Member only"));
        assert!(!home.exists());
    }
    let help = std::process::Command::new(env!("CARGO_BIN_EXE_workboard-server"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("invite-email EMAIL"));
    assert!(help.contains("Owner recovery") && help.contains("teammate"));
    let server = Server::start_at(&home, 0);
    let mut owner = App::connect(server.port, Some(workspace()));
    owner.until("ready", |a| a.role == Some(Role::Owner));
    for request in [
        json!({"invite_email": "new@example.test", "role": "owner"}),
        json!({"invite_email": "new@example.test", "role": "admin"}),
        json!({"invite_email": "new@example.test", "invite": 1}),
        json!({"invite_email": 1}),
        json!({"invite_email": null}),
    ] {
        assert_eq!(local_admin(&home, request)["ok"], false);
    }
    assert!(workboard_server::load_saved(&home)
        .unwrap()
        .invites
        .is_empty());
    drop(owner);
    server.stop();
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn email_invite_rejects_ambiguous_member_ids_without_changing_either_identity() {
    let home = temp_home("email-id-duplicate");
    let server = Server::start_at(&home, 0);
    let mut data = workspace();
    data["members"][1]["email"] = json!("pending@example.test");
    data["members"][2]["id"] = json!(2);
    let mut owner = App::connect(server.port, Some(data));
    owner.until("ready", |a| a.role == Some(Role::Owner));
    let res = local_admin(&home, json!({"invite_email": "pending@example.test"}));
    assert_eq!(
        res["ok"], false,
        "duplicate member ids cannot safely identify an invitee"
    );
    assert!(workboard_server::load_saved(&home)
        .unwrap()
        .invites
        .is_empty());
    drop(owner);
    server.stop();
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn email_invite_preserves_unrelated_sync_stamps_and_general_fields() {
    let home = temp_home("email-sync-stamps");
    std::fs::create_dir_all(&home).unwrap();
    let mut data = workspace();
    let mut meta = SyncMeta::default();
    wb_sync::stamp(&data, &mut meta, 1000);
    data["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    let data = wb_sync::shareable(&data);
    std::fs::write(
        home.join(workboard_server::WORKSPACE_FILE),
        data.to_string(),
    )
    .unwrap();
    let server = Server::start_at(&home, 0);
    let res = local_admin(&home, json!({"invite_email": "fresh@example.test"}));
    assert_eq!(res["ok"], true);
    let disk: Value = serde_json::from_slice(
        &std::fs::read(home.join(workboard_server::WORKSPACE_FILE)).unwrap(),
    )
    .unwrap();
    let after: SyncMeta = serde_json::from_value(disk["extra"]["sync"].clone()).unwrap();
    for id in [1, 2, 3, 11, 100, 101] {
        assert_eq!(
            after.stamps[&id], meta.stamps[&id],
            "unrelated item {id} must not be restamped"
        );
    }
    assert_eq!(
        after.fields[&10]
            .get("name")
            .copied()
            .unwrap_or(after.fields[&10][""]),
        1000
    );
    assert!(after.fields[&10]["members"] > 1000);
    assert!(after.stamps[&res["member"].as_u64().unwrap()] > 1000);
    assert_eq!(disk["tasks"], data["tasks"]);
    server.stop();
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn email_invite_fails_closed_on_malformed_workspace_metadata_or_role() {
    for (tag, field) in [("sync", 0), ("extra", 1), ("role", 2), ("members", 3)] {
        let home = temp_home(tag);
        std::fs::create_dir_all(&home).unwrap();
        let mut data = workspace();
        data["members"][1]["email"] = json!("pending@example.test");
        match field {
            0 => data["extra"]["sync"] = json!({"tombs": "corrupt"}),
            1 => data["extra"] = json!([]),
            2 => data["members"][1]["role"] = json!({"unknown": true}),
            _ => data["members"] = json!("corrupt"),
        }
        std::fs::write(
            home.join(workboard_server::WORKSPACE_FILE),
            data.to_string(),
        )
        .unwrap();
        let server = Server::start_at(&home, 0);
        let res = local_admin(&home, json!({"invite_email": "pending@example.test"}));
        assert_eq!(
            res["ok"], false,
            "malformed {tag} must fail without mutation"
        );
        server.stop();
        let disk: Value = serde_json::from_slice(
            &std::fs::read(home.join(workboard_server::WORKSPACE_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(disk, data);
        assert!(workboard_server::load_saved(&home)
            .unwrap()
            .invites
            .is_empty());
        let _ = std::fs::remove_dir_all(home);
    }
}

#[test]
fn health_and_admin_commands_answer() {
    let home = temp_home("health");
    let server = Server::start_at(&home, 0);
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
    let res =
        workboard_server::admin_request(&home, &json!({"invite": 1, "role": "owner"}), TICK_WAIT)
            .unwrap()
            .expect("answered");
    assert_eq!(res["ok"], true, "{}", res);
    let code = res["code"].as_str().unwrap().to_string();
    let mut a2 = App::join(server.port, &code);
    a2.until("signed in as owner again", |a| {
        a.role == Some(Role::Owner) && a.member == Some(1)
    });
    // within a minute of having a workspace there is a backup (the daily
    // copy is written just after the hourly one, so wait for that)
    let until = Instant::now() + TICK_WAIT;
    while workboard_server::backup_files(&home.join(workboard_server::BACKUP_DIR), "daily")
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
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    // the backup taken now has both tasks
    let backup_path = home.join("manual-backup.json");
    let until = Instant::now() + TICK_WAIT;
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
        TICK_WAIT,
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
    let server = Server::start_at(&home, 0);
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
    let until = Instant::now() + TICK_WAIT;
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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

/// A whiteboard's shapes and lines must survive a teammate on an app from
/// before them: that app reads the board without the two lists and writes
/// it back without them. It fingerprints what it writes (every app the
/// server takes changes from does), so it doesn't note them as deleted, and
/// the server keeps them from its own copy.
#[test]
fn whiteboard_shapes_survive_an_app_that_does_not_know_them() {
    let home = temp_home("shapes-old-app");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let shapes = json!([
        {"id": 501, "kind": "rect", "pos": [0.0, 0.0], "size": [160.0, 64.0], "text": "CEO", "color": [182, 96, 224], "filled": true},
        {"id": 502, "kind": "rect", "pos": [0.0, 124.0], "size": [160.0, 64.0], "text": "CTO", "color": [182, 96, 224]}
    ]);
    let lines = json!([{"id": 503, "from": 501, "to": 502, "a": [0.0, 0.0], "b": [0.0, 0.0], "color": [182, 96, 224], "elbow": true}]);
    let (s2, l2) = (shapes.clone(), lines.clone());
    a.change(move |d| {
        d["extra"]["whiteboards"] = json!([{"id": 500, "name": "Org", "strokes": [],
            "notes": [{"id": 504, "pos": [400.0, 0.0], "text": "note", "color": [248, 174, 0]}],
            "shapes": s2, "lines": l2}]);
    });
    b.wait("the board at B", |d| {
        d["extra"]["whiteboards"][0]["shapes"]
            .as_array()
            .is_some_and(|s| s.len() == 2)
    });
    // B is the old app: it reads the board without shapes and lines, and
    // fingerprints the copy it writes, as the app does after taking one in
    for w in b.data["extra"]["whiteboards"].as_array_mut().unwrap() {
        let o = w.as_object_mut().unwrap();
        o.remove("shapes");
        o.remove("lines");
    }
    let mut meta = b.meta();
    meta.hashes = wb_sync::fingerprints(&b.data);
    meta.field_hashes = wb_sync::part_hashes(&b.data);
    b.data["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    // then edits the note on the board
    b.change(|d| {
        d["extra"]["whiteboards"][0]["notes"][0]["text"] = json!("edited by B");
    });
    a.wait("B's note at A", |d| {
        d["extra"]["whiteboards"][0]["notes"][0]["text"] == "edited by B"
    });
    let w = &a.data["extra"]["whiteboards"][0];
    assert_eq!(w["shapes"], shapes, "the shapes are still there");
    assert_eq!(w["lines"], lines, "and the line");
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
    let server = Server::start_at(&home, 0);
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
        mail.contains(&format!("http://127.0.0.1:{}/download/mac", port)),
        "{}",
        mail
    );
    assert!(
        mail.contains(&format!("http://127.0.0.1:{}/download/windows", port)),
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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
    // and where to get the app when it isn't installed
    assert!(body.contains(r#"href="/download/mac""#), "{}", body);
    assert!(body.contains(r#"href="/download/windows""#), "{}", body);
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
    let server = Server::start_at(&home, 0);
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
    b.change(say_as("marker 2", 3201, 2));
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
    b.change(say_as("marker 3", 3202, 2));
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
        let until = Instant::now() + TICK_WAIT;
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
    let server = Server::start_at(&home, 0);
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
    let until = Instant::now() + TICK_WAIT;
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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
    let until = Instant::now() + TICK_WAIT;
    while !home.join(workboard_server::WORKSPACE_FILE).is_file() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    let backup = home.join("manual.json");
    std::fs::copy(home.join(workboard_server::WORKSPACE_FILE), &backup).unwrap();
    let res = workboard_server::admin_request(
        &home,
        &json!({"restore": backup.to_string_lossy()}),
        TICK_WAIT,
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
        TICK_WAIT,
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, port);
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
    let server = Server::start_at(&home, 0);
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
    let server = Server::start_at(&home, 0);
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
    b.change(say_as("marker 2", 3301, 2));
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
    let server = Server::start_at(&home, 0);
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
    b.change(say_as("marker 1", 3401, 2));
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
    let until = Instant::now() + TICK_WAIT;
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

/// Stands in for the webhooks' and commands' addresses: notes each request
/// and answers with `answer`.
#[derive(Debug, Default)]
struct FakePoster {
    got: std::sync::Mutex<Vec<(String, Value)>>,
    answer: std::sync::Mutex<String>,
}

impl workboard_server::hooks::Poster for FakePoster {
    fn post(&self, url: &str, body: &Value) -> Result<String, String> {
        self.got
            .lock()
            .unwrap()
            .push((url.to_string(), body.clone()));
        Ok(self.answer.lock().unwrap().clone())
    }
}

fn message_with<'a>(d: &'a Value, text: &str) -> Option<&'a Value> {
    d["channels"][0]["messages"]
        .as_array()?
        .iter()
        .find(|m| m["text"] == text)
}

/// Webhooks and slash commands: an owner sets them up; a POST to an
/// incoming webhook's address posts under its name; a channel's new
/// messages go to its outgoing webhook, whose answer is posted; a slash
/// command's answer is posted too. A member can't set them up.
#[test]
fn webhooks_and_slash_commands_post_under_their_names() {
    let home = temp_home("hooks");
    let fake = Arc::new(FakePoster::default());
    let opts = workboard_server::Options {
        poster: Some(fake.clone()),
        ..Default::default()
    };
    let server = Server::start_with(&home, 0, opts);
    let port = server.port;
    let (mut a, mut b) = two_apps(port);
    // a member may not
    b.client.send(Msg::HookAdd {
        hook: wb_net::HookInfo {
            kind: "in".into(),
            channel: 10,
            name: "Sneaky".into(),
            ..Default::default()
        },
    });
    b.until("turned down", |b| !b.denied.is_empty());
    // incoming
    a.client.send(Msg::HookAdd {
        hook: wb_net::HookInfo {
            kind: "in".into(),
            channel: 10,
            name: "CI".into(),
            ..Default::default()
        },
    });
    a.until("the webhook listed", |a| {
        a.integrations.as_ref().is_some_and(|(h, _)| h.len() == 1)
    });
    let token = a.integrations.as_ref().unwrap().0[0].token.clone();
    assert_eq!(token.len(), 36);
    let (head, _) = http_post(
        port,
        &format!("/hooks/{}", token),
        r#"{"text": "Build passed"}"#,
    );
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    let (head, _) = http_post(port, "/hooks/nope", r#"{"text": "x"}"#);
    assert!(head.starts_with("HTTP/1.1 404"), "{}", head);
    b.wait("the webhook's message at B", |d| {
        message_with(d, "Build passed").is_some()
    });
    let m = message_with(&b.data, "Build passed").unwrap();
    assert_eq!(m["bot"], "CI");
    assert_eq!(m["author"], 1, "its setter's");
    // outgoing, with a trigger word; the answer comes back under its name
    *fake.answer.lock().unwrap() = r#"{"text": "pong"}"#.into();
    a.client.send(Msg::HookAdd {
        hook: wb_net::HookInfo {
            kind: "out".into(),
            channel: 10,
            name: "Pinger".into(),
            url: "https://bots.example/ping".into(),
            trigger: "!ping".into(),
            ..Default::default()
        },
    });
    a.until("two webhooks", |a| {
        a.integrations.as_ref().is_some_and(|(h, _)| h.len() == 2)
    });
    b.change(in_general("no trigger here", 4001, 2));
    b.change(in_general("!ping are you there", 4002, 2));
    a.wait("the answer at A", |d| message_with(d, "pong").is_some());
    assert_eq!(message_with(&a.data, "pong").unwrap()["bot"], "Pinger");
    {
        let got = fake.got.lock().unwrap();
        assert_eq!(got.len(), 1, "only the message with the trigger word");
        assert_eq!(got[0].0, "https://bots.example/ping");
        assert_eq!(got[0].1["text"], "!ping are you there");
        assert_eq!(got[0].1["user_id"], 2);
        assert_eq!(got[0].1["channel_name"], "general");
    }
    // a slash command
    *fake.answer.lock().unwrap() = r#"{"text": "Deployed v2"}"#.into();
    a.client.send(Msg::CommandAdd {
        command: wb_net::CommandInfo {
            name: "/Deploy".into(),
            description: "Ship it".into(),
            url: "https://bots.example/deploy".into(),
        },
    });
    a.until("the command listed", |a| {
        a.integrations.as_ref().is_some_and(|(_, c)| c.len() == 1)
    });
    // a member sees the command but not its address
    b.client.send(Msg::Integrations);
    b.until("B's list", |b| b.integrations.is_some());
    let (hooks, cmds) = b.integrations.clone().unwrap();
    assert!(hooks.is_empty(), "webhooks are for owners and admins");
    assert_eq!(cmds[0].name, "deploy");
    assert!(cmds[0].url.is_empty());
    b.client.send(Msg::Command {
        channel: 10,
        parent: None,
        name: "deploy".into(),
        text: "v2".into(),
    });
    a.wait("the command's answer", |d| {
        message_with(d, "Deployed v2").is_some()
    });
    assert_eq!(
        message_with(&a.data, "Deployed v2").unwrap()["bot"],
        "/deploy"
    );
    assert_eq!(fake.got.lock().unwrap().last().unwrap().1["text"], "v2");
    // a command that isn't there
    b.client.send(Msg::Command {
        channel: 10,
        parent: None,
        name: "nope".into(),
        text: String::new(),
    });
    b.until("turned down", |b| b.denied.len() == 2);
}

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
    phone_with(owner, port, member, token, home, None)
}

/// `phone`, with the notification choices the iPhone app sends with its token.
fn phone_with(
    owner: &mut App,
    port: u16,
    member: u64,
    token: &str,
    home: &PathBuf,
    prefs: Option<wb_net::PushPrefs>,
) -> (String, Option<String>) {
    let code = owner.invite(member, Role::Member);
    let mut p = App::join(port, &code);
    p.wait("the phone gets the team's copy", |_| true);
    p.client.send(Msg::PushToken {
        token: token.to_string(),
        platform: "ios".into(),
        env: "development".into(),
        workspace: None,
        prefs,
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

/// A phone's choices (sent with its token) decide its pushes: a keyword
/// notifies like a mention, a channel set to nothing stays quiet.
#[test]
fn iphones_follow_the_choices_sent_with_their_token() {
    let home = temp_home("push-prefs");
    let fake = Arc::new(FakePush::default());
    let opts = || workboard_server::Options {
        push: Some(fake.clone()),
        ..Default::default()
    };
    let server = Server::start_with(&home, 0, opts());
    let port = server.port;
    let (mut a, mut b) = two_apps(port);
    let ann = "a1".repeat(32);
    let prefs = wb_net::PushPrefs {
        keywords: vec!["deploy".into()],
        ..Default::default()
    };
    phone_with(&mut a, port, 1, &ann, &home, Some(prefs));
    let kept = workboard_server::load_saved(&home)
        .unwrap()
        .push
        .into_values()
        .find(|t| t.token == ann)
        .unwrap();
    assert_eq!(
        kept.prefs.unwrap().keywords,
        vec!["deploy"],
        "kept in server.json"
    );
    b.change(in_general("lunch at noon", 30, 2));
    b.change(in_general("The Deploy is out", 31, 2));
    wait_for("the keyword pushed to Ann's phone", || {
        fake.to(&ann).len() == 1
    });
    assert_eq!(
        fake.to(&ann)[0]["aps"]["alert"]["body"],
        "The Deploy is out"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(fake.to(&ann).len(), 1, "the plain message stays quiet");
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

/// Collect what a watcher hears until `cond` holds of all of it (10 s at most).
fn heard_until(
    w: &wb_net::Watcher,
    got: &mut Vec<wb_net::Watch>,
    what: &str,
    cond: impl Fn(&[wb_net::Watch]) -> bool,
) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        got.extend(w.events());
        if cond(got) {
            return;
        }
        assert!(
            Instant::now() < until,
            "timed out waiting for {}: {:?}",
            what,
            got
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn note_count(got: &[wb_net::Watch]) -> usize {
    got.iter()
        .filter(|e| matches!(e, wb_net::Watch::Note { .. }))
        .count()
}

#[test]
fn a_computer_watching_a_workspace_hears_only_its_persons_new_messages() {
    let home = temp_home("watch");
    let fake = Arc::new(FakePush::default());
    let server = Server::start_with(
        &home,
        0,
        workboard_server::Options {
            push: Some(fake.clone()),
            ..Default::default()
        },
    );
    let port = server.port;
    let (mut a, mut b) = two_apps(port);
    // Cy's phone, which names this workspace for taps ("ws")
    let cy_phone = "c3".repeat(32);
    let code = a.invite(3, Role::Member);
    let mut p = App::join(port, &code);
    p.wait("the phone gets the team's copy", |_| true);
    p.client.send(Msg::PushToken {
        token: cy_phone.clone(),
        platform: "ios".into(),
        env: "development".into(),
        workspace: Some("teamKey42".into()),
        prefs: None,
    });
    let phone_device = p.device.clone();
    wait_for("the token in server.json", || {
        saved_tokens(&home).contains(&(phone_device.clone(), cy_phone.clone()))
    });
    drop(p);
    // Cy's computer, signed in once, now has another workspace open: it only watches this one
    let code = a.invite(3, Role::Member);
    let mut c = App::join(port, &code);
    c.wait("Cy's computer gets the team's copy", |_| true);
    let (device, token) = (c.device.clone(), c.token.clone().unwrap());
    let before = connections(port);
    drop(c);
    wait_for("Cy's computer's connection gone", || {
        connections(port) < before
    });
    let addr = format!("127.0.0.1:{}", port);
    let w = wb_net::Watcher::start(&addr, &device, &token, None, Arc::new(|| {}));
    let mut got = vec![];
    heard_until(&w, &mut got, "watching", |g| {
        g.contains(&wb_net::Watch::Started)
    });
    // not online there: no green dot for Cy
    a.until("the presence after Cy started watching", |a| {
        a.online.is_some()
    });
    std::thread::sleep(Duration::from_millis(300));
    a.pump();
    assert!(
        !a.online.clone().unwrap_or_default().contains(&3),
        "a watcher isn't online"
    );

    // a DM from Ann to Cy: the watcher hears it as the phone's push says it
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({"id": 20, "name": "", "kind": "Direct", "topic": "", "members": [1, 3], "messages": [{"id": 21, "author": 1, "text": "lunch?", "ts": now() / 1000}], "last_read": 0}));
    });
    heard_until(&w, &mut got, "the DM at Cy's watching computer", |g| {
        note_count(g) == 1
    });
    assert!(got.contains(&wb_net::Watch::Note {
        title: "Ann".into(),
        body: "lunch?".into(),
        channel: Some(20),
        parent: None,
    }));
    // the phone still gets its push, with the phone's name for the workspace
    wait_for("the DM pushed to Cy's phone", || {
        fake.to(&cy_phone).len() == 1
    });
    assert_eq!(fake.to(&cy_phone)[0]["ws"], "teamKey42");
    assert_eq!(fake.to(&cy_phone)[0]["channel"], 20);

    // a message to everyone and a mention of Bo: nothing for Cy; a mention of Cy: heard
    b.change(in_general("hi everyone", 30, 2));
    a.change(in_general("@Bo look", 31, 1));
    b.change(in_general("@Cy can you look", 32, 2));
    heard_until(&w, &mut got, "the mention at Cy's watching computer", |g| {
        note_count(g) == 2
    });
    std::thread::sleep(Duration::from_millis(300));
    got.extend(w.events());
    let notes: Vec<&wb_net::Watch> = got
        .iter()
        .filter(|e| matches!(e, wb_net::Watch::Note { .. }))
        .collect();
    assert_eq!(notes.len(), 2, "only the DM and the mention: {:?}", notes);
    assert_eq!(
        notes[1],
        &wb_net::Watch::Note {
            title: "Bo in #general".into(),
            body: "@Cy can you look".into(),
            channel: Some(10),
            parent: None,
        }
    );

    // Cy's access removed: the watcher hears it and stops
    a.client.send(Msg::Revoke { member: 3 });
    heard_until(&w, &mut got, "the watcher refused", |g| {
        g.iter().any(|e| matches!(e, wb_net::Watch::Refused(_)))
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_server_without_push_set_up_keeps_tokens_and_works_as_before() {
    let home = temp_home("nopush");
    let server = Server::start_at(&home, 0);
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
        workspace: None,
        prefs: None,
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
    let server = Server::start_at(&home, 0);
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

#[test]
fn an_app_too_old_to_know_every_field_gets_changes_but_cannot_send_them() {
    let home = temp_home("old-app");
    let server = Server::start_at(&home, 0);
    let (mut a, b) = two_apps(server.port);
    // B's computer runs an app from before the hello said its version
    let token = b.token.clone();
    let device = b.device.clone();
    drop(b);
    let mut old = App::start(
        server.port,
        Value::Null,
        SignIn {
            token,
            device,
            app: Some("none".into()),
            ..Default::default()
        },
    );
    old.wait("the old app still gets the team's copy", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    old.change(|d| d["tasks"][0]["name"] = json!("Plan (old app)"));
    old.until("the old app is told to update", |o| {
        o.denied.iter().any(|d| d == wb_net::UPDATE_NEEDED)
    });
    assert!(
        matches!(old.status, Some(Status::Connected)),
        "it stays connected: {:?}",
        old.status
    );
    // its change didn't reach the team, but the team's changes reach it
    a.change(|d| d["tasks"][1]["name"] = json!("Ship (A)"));
    old.wait("A's change at the old app", |d| {
        d["tasks"][1]["name"] == "Ship (A)"
    });
    a.wait("A's copy", |d| d["tasks"][1]["name"] == "Ship (A)");
    assert_eq!(
        a.data["tasks"][0]["name"], "Plan",
        "the old app's change was not taken"
    );
    // and it is said once, not on every change
    old.change(|d| d["tasks"][0]["name"] = json!("again"));
    std::thread::sleep(Duration::from_millis(300));
    old.pump();
    assert_eq!(old.denied.len(), 1, "{:?}", old.denied);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------------------------ deltas

/// Every item id in a copy (whole or delta), nested ones too.
fn all_ids(v: &Value) -> Vec<u64> {
    wb_sync::fingerprints(v).into_keys().collect()
}

#[test]
fn after_the_first_copy_only_what_changed_travels() {
    let home = temp_home("deltas");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    b.updates.clear();
    a.change(|d| d["tasks"][0]["name"] = json!("Plan (A)"));
    b.wait("A's rename at B", |d| d["tasks"][0]["name"] == "Plan (A)");
    let last = b.updates.last().unwrap();
    assert_eq!(
        all_ids(last),
        vec![100],
        "only the renamed task came: {}",
        last
    );
    assert!(last.get("members").is_none() && last.get("channels").is_none());
    // a message: the channel comes with only the new message in it
    b.change(say_as("from B", 3001, 2));
    a.wait("B's message at A", |d| {
        texts(d).contains(&"from B".to_string())
    });
    let last = a.updates.last().unwrap();
    let mut ids = all_ids(last);
    ids.sort();
    assert_eq!(ids, vec![10, 3001], "{}", last);
    // edits at once from both still merge field by field, through deltas
    a.change(|d| d["tasks"][1]["name"] = json!("Ship (A)"));
    b.change(|d| d["tasks"][1]["status"] = json!("done"));
    for app in [&mut a, &mut b] {
        app.wait("both edits", |d| {
            d["tasks"][1]["name"] == "Ship (A)" && d["tasks"][1]["status"] == "done"
        });
    }
    // and a delete
    a.change(|d| {
        d["tasks"].as_array_mut().unwrap().remove(0);
    });
    b.wait("the delete at B", |d| task_names(d) == ["Ship (A)"]);
    assert!(b.meta().tombs.contains_key(&100));
    // an app asking for whole copies (like one from before deltas) gets them
    let code = a.invite(3, Role::Member);
    let mut old = App::start(
        server.port,
        Value::Null,
        SignIn {
            invite: Some(code),
            full_copies: true,
            ..Default::default()
        },
    );
    old.wait("the old app's copy", |d| task_names(d) == ["Ship (A)"]);
    a.change(|d| d["tasks"][0]["name"] = json!("Ship!"));
    old.wait("the rename at the old app", |d| task_names(d) == ["Ship!"]);
    assert!(
        old.updates.last().unwrap()["members"]
            .as_array()
            .is_some_and(|m| m.len() == 3),
        "the whole copy"
    );
    old.change(|d| d["tasks"][0]["status"] = json!("closed"));
    a.wait("the old app's change", |d| {
        d["tasks"][0]["status"] == "closed"
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn an_app_that_was_away_catches_up_on_every_change() {
    let home = temp_home("deltas-away");
    let server = Server::start_at(&home, 0);
    let (mut a, b) = two_apps(server.port);
    let (token, device, kept) = (b.token.clone(), b.device.clone(), b.data.clone());
    drop(b);
    // three revisions while B is away
    a.change(|d| d["tasks"][0]["name"] = json!("one"));
    a.change(say("two", 2002));
    a.change(|d| {
        d["tasks"].as_array_mut().unwrap().remove(1);
    });
    let mut b = App::start(
        server.port,
        kept,
        SignIn {
            token,
            device,
            ..Default::default()
        },
    );
    b.wait("everything at B after reconnecting", |d| {
        task_names(d) == ["one"] && texts(d).contains(&"two".to_string())
    });
    // and B's copy from before doesn't bring the deleted task back
    b.change(|d| d["tasks"][0]["status"] = json!("done"));
    a.wait("B's change", |d| d["tasks"][0]["status"] == "done");
    assert_eq!(task_names(&a.data), ["one"]);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// A bare connection that speaks the protocol by hand.
struct Raw(tungstenite::WebSocket<std::net::TcpStream>);

impl Raw {
    fn connect(port: u16, hello: Value) -> Raw {
        let stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let (ws, _) = tungstenite::client::client_with_config(
            format!("ws://127.0.0.1:{}/ws", port).as_str(),
            stream,
            Some(
                tungstenite::protocol::WebSocketConfig::default()
                    .max_message_size(Some(wb_net::MAX_MESSAGE)),
            ),
        )
        .unwrap();
        let mut r = Raw(ws);
        r.send(hello);
        r
    }

    fn send(&mut self, v: Value) {
        self.0
            .send(tungstenite::Message::text(v.to_string()))
            .unwrap();
    }

    /// The next message of `kind` (others are skipped).
    fn next(&mut self, kind: &str) -> Value {
        loop {
            let m = self.0.read().expect("a message from the server");
            if let tungstenite::Message::Text(t) = m {
                let v: Value = serde_json::from_str(t.as_str()).unwrap();
                if v["type"] == kind {
                    return v;
                }
            }
        }
    }
}

#[test]
fn a_missed_revision_brings_the_whole_copy_and_partial_pushes_merge() {
    let home = temp_home("deltas-raw");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let mut raw = Raw::connect(
        server.port,
        json!({"type": "hello", "device": b.device, "token": b.token, "sync": wb_sync::META_V, "deltas": true}),
    );
    let w = raw.next("welcome");
    assert_eq!(w["deltas"], true, "the server takes deltas");
    let rev = w["rev"].as_u64().unwrap();
    a.change(|d| d["tasks"][0]["name"] = json!("r1"));
    let d1 = raw.next("delta");
    assert_eq!(d1["base"].as_u64().unwrap(), rev);
    assert_eq!(all_ids(&d1["data"]), vec![100]);
    a.change(|d| d["tasks"][0]["name"] = json!("r2"));
    let d2 = raw.next("delta");
    assert_eq!(
        d2["base"], d1["rev"],
        "each delta follows the last revision"
    );
    // pretend d2 was lost: the next one doesn't follow what we have, so we
    // ask for everything
    a.change(|d| d["tasks"][0]["name"] = json!("r3"));
    let d3 = raw.next("delta");
    assert!(d3["base"].as_u64().unwrap() > d1["rev"].as_u64().unwrap());
    raw.send(json!({"type": "resync"}));
    let whole = raw.next("update");
    assert_eq!(whole["rev"], d3["rev"]);
    assert_eq!(task_names(&whole["data"]), ["r3", "Ship"]);
    // a push of only what changed merges without taking anything else away
    let mut part = json!({"tasks": [whole["data"]["tasks"][1].clone()], "extra": {"sync": {"v": wb_sync::META_V}}});
    part["tasks"][0]["name"] = json!("Ship (partial)");
    let t = whole["data"]["extra"]["sync"]["stamps"]["101"]
        .as_i64()
        .unwrap();
    part["extra"]["sync"]["stamps"] = json!({"101": t + 1});
    raw.send(json!({"type": "push", "data": part}));
    a.wait("the partial push at A", |d| {
        task_names(d) == ["r3", "Ship (partial)"]
    });
    b.wait("and at B", |d| task_names(d) == ["r3", "Ship (partial)"]);
    assert_eq!(texts(&a.data), ["hi"], "nothing else went away");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn change_times_from_the_future_dont_win_over_later_edits() {
    let home = temp_home("future-stamps");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let mut raw = Raw::connect(
        server.port,
        json!({"type": "hello", "device": b.device, "token": b.token, "sync": wb_sync::META_V, "deltas": true}),
    );
    let whole = raw.next("welcome");
    let v = wb_sync::META_V;
    // Bo deletes Ann's message "forever" (i64::MAX also overflowed the
    // server's "+ 1" and buried it for good)
    raw.send(json!({"type": "push", "data": {"tasks": [], "extra": {"sync": {"v": v, "tombs": {"11": i64::MAX}}}}}));
    // and renames a task with a stamp far in the future
    let mut part =
        json!({"tasks": [whole["snapshot"]["tasks"][1].clone()], "extra": {"sync": {"v": v}}});
    part["tasks"][0]["name"] = json!("frozen");
    part["extra"]["sync"]["stamps"] = json!({"101": i64::MAX / 2});
    part["extra"]["sync"]["fields"] = json!({"101": {"name": i64::MAX / 2}});
    raw.send(json!({"type": "push", "data": part}));
    a.wait("Bo's rename at Ann", |d| {
        task_names(d) == ["Plan", "frozen"]
    });
    assert_eq!(texts(&a.data), ["hi"], "Ann's message stays");
    // a little later Ann renames it: her edit wins
    std::thread::sleep(Duration::from_millis(5));
    a.change(|d| d["tasks"][1]["name"] = json!("Ship again"));
    b.wait("Ann's later rename wins at Bo", |d| {
        task_names(d) == ["Plan", "Ship again"]
    });
    let far = now() + 3_600_000;
    let meta = a.meta();
    assert!(
        meta.stamps
            .values()
            .chain(meta.tombs.values())
            .all(|t| *t < far),
        "no stamp from the future: {:?}",
        meta
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_deleted_message_comes_back_only_as_it_was() {
    let home = temp_home("graves");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| d["channels"][0]["messages"] = json!([]));
    b.wait("Ann's message gone at Bo", |d| texts(d).is_empty());
    let mut raw = Raw::connect(
        server.port,
        json!({"type": "hello", "device": b.device, "token": b.token, "sync": wb_sync::META_V, "deltas": true}),
    );
    let w = raw.next("welcome");
    let channel = |text: &str| {
        let mut ch = w["snapshot"]["channels"][0].clone();
        ch["messages"] = json!([{"id": 11, "author": 1, "text": text, "ts": 1}]);
        ch
    };
    let back = |text: &str, t: i64| {
        json!({"type": "push", "data": {"channels": [channel(text)], "extra": {"sync": {
            "v": wb_sync::META_V, "revived": {"11": t}, "stamps": {"11": t}}}}})
    };
    // Bo makes up words for Ann under the deleted message's id
    raw.send(back("Ann says Bo is boss", now()));
    // then brings the real one back from the Trash
    std::thread::sleep(Duration::from_millis(5));
    raw.send(back("hi", now()));
    a.wait("the real message is back", |d| texts(d) == ["hi"]);
    assert!(!a.data.to_string().contains("boss"));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_plain_member_cant_move_someone_elses_message_away() {
    let home = temp_home("move-away");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    // Bo's own private channel
    b.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({
            "id": 3100, "name": "bo-only", "kind": "Channel", "private": true, "creator": 2,
            "topic": "", "members": [2], "messages": [], "last_read": 0
        }));
    });
    round_trip(&mut b, &mut a, "Bo's channel is in", 3024);
    let mut raw = Raw::connect(
        server.port,
        json!({"type": "hello", "device": b.device, "token": b.token, "sync": wb_sync::META_V, "deltas": true}),
    );
    let w = raw.next("welcome");
    let mut general = w["snapshot"]["channels"][0].clone();
    let msg = general["messages"][0].clone();
    assert_eq!(msg["id"], 11);
    general["messages"] = json!([]);
    let mut mine = find(&w["snapshot"], "channels", 3100).unwrap().clone();
    mine["messages"] = json!([msg]);
    let t = now();
    raw.send(
        json!({"type": "push", "data": {"channels": [general, mine], "extra": {"sync": {
        "v": wb_sync::META_V, "moved": {"11": t}, "stamps": {"10": t, "11": t, "3100": t}}}}}),
    );
    round_trip(&mut b, &mut a, "after the move", 3023);
    assert_eq!(
        find(&a.data, "channels", 10).unwrap()["messages"][0]["id"],
        11,
        "Ann's message stays in general: {}",
        a.data
    );
    assert!(!find(&a.data, "channels", 3100).is_some_and(|c| c.to_string().contains("\"hi\"")));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_plain_member_cant_make_a_shared_space_or_doc_their_own() {
    let home = temp_home("take-private");
    let server = Server::start_at(&home, 0);
    let mut ws = plans_workspace("edit");
    ws["docs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": 92, "owner": 1, "title": "Team doc", "body": "for all", "created": 0, "updated": 0}));
    let (mut a, mut b, mut c) = ann_bo_cy(server.port, ws);
    b.change(|d| {
        find_mut(d, "spaces", 60)["private_owner"] = json!(2);
        find_mut(d, "docs", 92)["private"] = json!(true);
        find_mut(d, "docs", 92)["owner"] = json!(2);
    });
    round_trip(&mut b, &mut a, "after making them Bo's", 3025);
    round_trip(&mut a, &mut c, "Cy hears", 2022);
    for (who, d) in [("Ann", &a.data), ("Cy", &c.data)] {
        assert!(
            find(d, "spaces", 60).is_some(),
            "{} keeps the Team space",
            who
        );
        assert!(find(d, "docs", 92).is_some(), "{} keeps the Team doc", who);
    }
    // nor move the team's list into a Personal space of theirs; their own
    // doc they may make private
    b.change(|d| {
        d["spaces"].as_array_mut().unwrap().push(json!(
            {"id": 3200, "name": "Bo's", "private_owner": 2, "statuses": [], "folders": [], "lists": []}));
        d["docs"].as_array_mut().unwrap().push(json!(
            {"id": 3300, "owner": 2, "title": "Bo's notes", "body": "", "created": 0, "updated": 0}));
    });
    round_trip(&mut b, &mut c, "Bo's doc is out", 3026);
    assert!(find(&c.data, "docs", 3300).is_some());
    b.change(|d| {
        let list = find(d, "spaces", 60).unwrap()["lists"][0].clone();
        assert_eq!(list["id"], 61);
        find_mut(d, "spaces", 60)["lists"] = json!([]);
        find_mut(d, "spaces", 3200)["lists"] = json!([list]);
        find_mut(d, "docs", 3300)["private"] = json!(true);
    });
    round_trip(&mut b, &mut c, "after the list move", 3027);
    round_trip(&mut a, &mut c, "Cy hears Ann", 2023);
    for (who, d) in [("Ann", &a.data), ("Cy", &c.data)] {
        assert!(
            find(d, "spaces", 60).is_some_and(|s| s["lists"][0]["id"] == 61),
            "{} keeps the Team list: {}",
            who,
            d
        );
    }
    assert!(find(&c.data, "docs", 3300).is_none(), "Bo's doc is his now");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_private_channel_stays_closed_when_its_last_member_leaves() {
    let home = temp_home("last-leaves");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b, mut c) = ann_bo_cy(server.port, plans_workspace("view"));
    b.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({
            "id": 3400, "name": "bo-alone", "kind": "Channel", "private": true, "creator": 2,
            "topic": "", "members": [2], "last_read": 0,
            "messages": [{"id": 3401, "author": 2, "text": "Bo's secret", "ts": now()}]
        }));
    });
    round_trip(&mut b, &mut c, "Bo's channel is in", 3028);
    assert!(!c.data.to_string().contains("Bo's secret"));
    b.change(|d| find_mut(d, "channels", 3400)["members"] = json!([]));
    round_trip(&mut b, &mut c, "after Bo leaves", 3029);
    round_trip(&mut a, &mut c, "and Ann writes", 2024);
    assert!(
        !c.data.to_string().contains("Bo's secret"),
        "Cy doesn't get the channel: {}",
        c.data
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn hidden_items_never_leak_through_deltas() {
    let home = temp_home("deltas-hidden");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    let code = a.invite(3, Role::Member);
    let mut cy = App::join(server.port, &code);
    cy.wait("Cy's copy", |_| true);
    cy.updates.clear();
    // a private channel of Ann and Bo, and a task in Ann's Personal space
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({"id": 20, "name": "plans", "kind": "Channel", "private": true, "topic": "", "members": [1, 2], "messages": [{"id": 21, "author": 1, "text": "secret plan", "ts": 5}], "last_read": 0}));
        d["spaces"] = json!([{"id": 50, "name": "Personal", "private_owner": 1, "folders": [], "lists": [{"id": 51, "name": "Mine"}]}]);
        d["tasks"].as_array_mut().unwrap().push(json!({"id": 52, "list_id": 51, "name": "secret task", "activity": []}));
    });
    b.wait("the channel at Bo", |d| channel(d, 20).is_some());
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
            .push(json!({"id": 22, "author": 1, "text": "more secret", "ts": 6}));
        d["tasks"][2]["name"] = json!("secret task renamed");
    });
    b.wait("the new message at Bo", |d| {
        channel(d, 20).is_some_and(|c| c["messages"].as_array().unwrap().len() == 2)
    });
    // something Cy may see, so Cy surely got the revisions before it
    a.change(say("for everyone", 2003));
    cy.wait("the message for everyone", |d| {
        texts(d).contains(&"for everyone".to_string())
    });
    assert!(cy.updates.len() >= 3);
    for u in &cy.updates {
        let text = u.to_string();
        assert!(
            !text.contains("secret")
                && !all_ids(u)
                    .iter()
                    .any(|i| [20, 21, 22, 50, 51, 52].contains(i)),
            "nothing hidden reached Cy: {}",
            text
        );
        let stamps = &u["extra"]["sync"]["stamps"];
        assert!(stamps.get("20").is_none() && stamps.get("52").is_none());
    }
    // added to the channel, Cy gets it whole, with what was said before
    a.change(|d| {
        let ch = d["channels"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["id"] == 20)
            .unwrap();
        ch["members"] = json!([1, 2, 3]);
    });
    cy.wait("the channel with its history at Cy", |d| {
        channel(d, 20).is_some_and(|c| c["messages"].as_array().unwrap().len() == 2)
    });
    // taken out again, it leaves Cy's copy (without being deleted for others)
    a.change(|d| {
        let ch = d["channels"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["id"] == 20)
            .unwrap();
        ch["members"] = json!([1, 2]);
    });
    cy.wait("the channel gone at Cy", |d| channel(d, 20).is_none());
    a.change(say("still here", 2004));
    b.wait("Bo still has the channel", |d| {
        channel(d, 20).is_some() && texts(d).contains(&"still here".to_string())
    });
    assert!(!b.meta().tombs.contains_key(&20));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// A channel Ann (1) made: Bo (2), a plain member, can join and leave it but
/// can't delete it, add or remove others, or drop who made it by sending a
/// copy from an older app. In a channel from before makers were kept, anyone
/// in it may.
#[test]
fn only_a_channels_maker_or_an_admin_deletes_it_or_changes_its_members() {
    let home = temp_home("changuard");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| {
        let c = d["channels"].as_array_mut().unwrap();
        c.push(json!({"id": 40, "name": "ann's", "kind": "Channel", "topic": "", "creator": 1, "members": [1, 3], "messages": [], "last_read": 0}));
        c.push(json!({"id": 50, "name": "old", "kind": "Channel", "topic": "", "members": [1, 2], "messages": [], "last_read": 0}));
    });
    b.wait("B gets both", |d| {
        channel(d, 40).is_some() && channel(d, 50).is_some()
    });
    // B joins, removes Cy and drops the maker (an app that doesn't know it)
    b.change(|d| {
        let c = d["channels"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["id"] == 40)
            .unwrap();
        c["members"] = json!([1, 2]);
        c["name"] = json!("renamed by Bo");
        c.as_object_mut().unwrap().remove("creator");
    });
    a.wait("B's rename at A", |d| {
        channel(d, 40).is_some_and(|c| c["name"] == "renamed by Bo")
    });
    let c = channel(&a.data, 40).unwrap();
    assert_eq!(c["members"], json!([1, 3, 2]), "B joined, Cy stays");
    assert_eq!(c["creator"], 1, "the maker stays");
    // B deletes it: nothing happens for the team; B deletes the old one: gone
    b.change(|d| {
        d["channels"]
            .as_array_mut()
            .unwrap()
            .retain(|c| c["id"] != 40 && c["id"] != 50);
    });
    assert!(b.meta().tombs.contains_key(&40));
    a.wait("the old channel deleted at A", |d| channel(d, 50).is_none());
    // B makes Ann's channel private (Cy, not in it, would lose it) and
    // turns it into a DM: it stays a public channel
    b.change(|d| {
        d["channels"].as_array_mut().unwrap().push(
            json!({"id": 40, "name": "renamed by Bo", "kind": "Direct", "private": true, "topic": "", "creator": 1, "members": [1, 3, 2], "messages": [], "last_read": 0}),
        );
    });
    round_trip(&mut b, &mut a, "after making it private", 3030);
    let c = channel(&a.data, 40).unwrap();
    assert_eq!(c["kind"], "Channel", "{}", c);
    assert!(c.get("private").is_none_or(|p| p == false), "{}", c);
    assert!(
        channel(&a.data, 40).is_some(),
        "Ann's channel is still there"
    );
    // an admin may
    a.change(|d| {
        d["channels"].as_array_mut().unwrap().push(json!({"id": 60, "name": "bo's", "kind": "Channel", "topic": "", "creator": 2, "members": [2], "messages": [], "last_read": 0}));
    });
    b.wait("B gets 60", |d| channel(d, 60).is_some());
    a.change(|d| {
        d["channels"]
            .as_array_mut()
            .unwrap()
            .retain(|c| c["id"] != 60);
    });
    b.wait("the owner deleted Bo's channel", |d| {
        channel(d, 60).is_none()
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ---- spaces shared with chosen people

/// A team space and "Plans", shared with Bo (2) at `level` only, with a
/// list, a folder with a list, tasks (one with Ann's comment), a doc with a
/// sub page, a dashboard and a form in it.
fn plans_workspace(level: &str) -> Value {
    let mut ws = workspace();
    ws["spaces"] = json!([
        {"id": 60, "name": "Team", "color": [1, 2, 3], "statuses": [], "folders": [], "lists": [{"id": 61, "name": "Team list"}]},
        {"id": 70, "name": "Plans", "color": [1, 2, 3], "statuses": [], "lists": [{"id": 71, "name": "Secret list"}],
         "folders": [{"id": 73, "name": "Plan folder", "lists": [{"id": 74, "name": "Folder list"}]}],
         "sharing": {"everyone": false, "people": [{"member": 2, "level": level}]}}
    ]);
    let tasks = ws["tasks"].as_array_mut().unwrap();
    tasks.push(
        json!({"id": 72, "list_id": 71, "name": "Secret task", "updated": 0,
        "activity": [{"id": 76, "ts": 1, "author": 1, "text": "secret comment", "comment": true}]}),
    );
    tasks.push(json!({"id": 75, "list_id": 74, "name": "Folder task", "activity": []}));
    tasks.push(json!({"id": 62, "list_id": 61, "name": "Team task", "activity": []}));
    ws["docs"] = json!([
        {"id": 90, "owner": 1, "space": 70, "title": "Secret doc", "body": "plans", "created": 0, "updated": 0},
        {"id": 91, "owner": 1, "parent": 90, "title": "Secret page", "body": "more", "created": 0, "updated": 0}
    ]);
    ws["extra"]["dashboards"] = json!([{"id": 95, "name": "Secret board", "space": 70}]);
    ws["extra"]["forms"] =
        json!([{"id": 96, "name": "Secret form", "list_id": 74, "description": "", "fields": []}]);
    ws
}

const SECRETS: [&str; 10] = [
    "Secret list",
    "Folder list",
    "Plan folder",
    "Secret task",
    "Folder task",
    "secret comment",
    "Secret doc",
    "Secret page",
    "Secret board",
    "Secret form",
];

/// Ann (the owner) with `ws` on the server, Bo (2) and Cy (3) joined.
fn ann_bo_cy(port: u16, ws: Value) -> (App, App, App) {
    let mut a = App::connect(port, Some(ws));
    a.wait("A's copy on the server", |_| true);
    a.until("A is the owner", |a| a.role == Some(Role::Owner));
    let mut joined = vec![];
    for m in [2, 3] {
        let code = a.invite(m, Role::Member);
        let mut x = App::join(port, &code);
        x.wait("the team's copy", |d| {
            task_names(d).contains(&"Team task".to_string())
        });
        joined.push(x);
    }
    let c = joined.pop().unwrap();
    let b = joined.pop().unwrap();
    (a, b, c)
}

fn find<'a>(d: &'a Value, path: &str, id: u64) -> Option<&'a Value> {
    path.split('.')
        .try_fold(d, |v, p| v.get(p))?
        .as_array()?
        .iter()
        .find(|x| x["id"] == id)
}

fn find_mut<'a>(d: &'a mut Value, path: &str, id: u64) -> &'a mut Value {
    let mut cur = d;
    for p in path.split('.') {
        cur = cur.get_mut(p).unwrap();
    }
    cur.as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|x| x["id"] == id)
        .unwrap()
}

fn has_all(d: &Value) -> bool {
    let text = d.to_string();
    SECRETS.iter().all(|s| text.contains(s))
}

fn has_none(d: &Value) -> bool {
    let text = d.to_string();
    SECRETS.iter().all(|s| !text.contains(s)) && find(d, "spaces", 70).is_none()
}

/// A marker message from `from` that `to` waits for: everything `from`
/// sent before it has been merged.
fn round_trip(from: &mut App, to: &mut App, text: &str, id: u64) {
    let author = from.member.expect("signed in");
    from.change(say_as(text, id, author));
    to.wait(text, |d| texts(d).contains(&text.to_string()));
}

/// Nothing of Plans (or anything in it) reaches Cy, who it isn't shared
/// with; Bo gets it all. An admin sees it too.
#[test]
fn a_space_shared_with_chosen_people_reaches_only_them() {
    let home = temp_home("sharehide");
    let server = Server::start_at(&home, 0);
    let (mut a, b, mut c) = ann_bo_cy(server.port, plans_workspace("view"));
    assert!(has_all(&b.data), "Bo gets Plans: {}", b.data);
    assert!(has_none(&c.data), "Cy gets none of it: {}", c.data);
    assert!(c.data.to_string().contains("Team list"));
    // what Cy sends doesn't touch it
    c.change(|d| {
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 72, "list_id": 71, "name": "Cy's", "activity": []}));
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 9001, "list_id": 71, "name": "Cy's new", "activity": []}));
    });
    round_trip(&mut c, &mut a, "after Cy", 3001);
    assert_eq!(find(&a.data, "tasks", 72).unwrap()["name"], "Secret task");
    assert!(
        find(&a.data, "tasks", 9001).is_none(),
        "nothing new in a space closed to Cy"
    );
    c.wait("Cy's new task leaves Cy's copy", |d| {
        find(d, "tasks", 9001).is_none()
    });
    assert!(!c.put_back.is_empty(), "Cy is told");
    // an admin sees every shared space
    a.client.send(Msg::SetRole {
        member: 3,
        role: Role::Admin,
    });
    c.wait("Cy, now an admin, gets Plans", has_all);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Bo may only view Plans: renames, new and deleted tasks, list and doc
/// changes, comments and a change to who it's shared with all stay out of
/// the team's copy, and Bo's own copy goes back to the team's version.
/// Opening a doc (his own view of it) still counts.
#[test]
fn a_view_only_member_changes_nothing_and_gets_the_teams_version_back() {
    let home = temp_home("shareview");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b, _c) = ann_bo_cy(server.port, plans_workspace("view"));
    b.change(|d| {
        find_mut(d, "tasks", 72)["name"] = json!("Bo was here");
        find_mut(d, "tasks", 72)["activity"]
            .as_array_mut()
            .unwrap()
            .push(
                json!({"id": 9002, "ts": 5, "author": 2, "text": "Bo's comment", "comment": true}),
            );
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 9001, "list_id": 71, "name": "Bo's new", "activity": []}));
        d["tasks"].as_array_mut().unwrap().retain(|t| t["id"] != 75);
        let sp = find_mut(d, "spaces", 70);
        sp["lists"][0]["name"] = json!("Bo's list");
        sp["name"] = json!("Bo's space");
        sp["sharing"]["people"] =
            json!([{"member": 2, "level": "full"}, {"member": 3, "level": "full"}]);
        find_mut(d, "docs", 90)["body"] = json!("changed by Bo");
        d["docs"].as_array_mut().unwrap().retain(|x| x["id"] != 91);
        find_mut(d, "extra.dashboards", 95)["name"] = json!("Bo's board");
    });
    assert!(b.meta().tombs.contains_key(&75));
    b.until("Bo is told", |b| !b.put_back.is_empty());
    b.wait("Bo's copy is the team's again", |d| {
        let sp = find(d, "spaces", 70);
        find(d, "tasks", 72)
            .is_some_and(|t| t["name"] == "Secret task" && !t.to_string().contains("Bo's comment"))
            && find(d, "tasks", 9001).is_none()
            && find(d, "tasks", 75).is_some()
            && find(d, "docs", 91).is_some()
            && find(d, "docs", 90).is_some_and(|x| x["body"] == "plans")
            && find(d, "extra.dashboards", 95).is_some_and(|x| x["name"] == "Secret board")
            && sp.is_some_and(|s| {
                s["name"] == "Plans"
                    && s["lists"][0]["name"] == "Secret list"
                    && s["sharing"]["people"] == json!([{"member": 2, "level": "view"}])
            })
    });
    round_trip(&mut b, &mut a, "after Bo", 3002);
    let ws = plans_workspace("view");
    for (path, id) in [
        ("tasks", 72),
        ("tasks", 75),
        ("docs", 90),
        ("docs", 91),
        ("spaces", 70),
        ("extra.dashboards", 95),
    ] {
        assert_eq!(
            find(&a.data, path, id).map(wb_sync::own_fields),
            find(&ws, path, id).map(wb_sync::own_fields),
            "{} {} as Ann made it",
            path,
            id
        );
    }
    assert!(find(&a.data, "tasks", 9001).is_none());
    assert!(!a.data.to_string().contains("Bo's comment"));
    // opening a doc notes Bo's own view of it
    b.put_back.clear();
    b.change(|d| {
        find_mut(d, "docs", 90)["views"] = json!([{"id": 9003, "member": 2, "at": [5]}]);
    });
    a.wait("Bo's view of the doc", |d| {
        find(d, "docs", 90).is_some_and(|x| x["views"][0]["member"] == 2)
    });
    round_trip(&mut b, &mut a, "after the view", 3003);
    b.pump();
    assert!(
        b.put_back.is_empty(),
        "nothing turned down: {:?}",
        b.put_back
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Bo may comment in Plans: his comments reach the team and he may edit and
/// delete them, but not Ann's comment or the task itself.
#[test]
fn a_comment_member_comments_but_changes_nothing_else() {
    let home = temp_home("sharecomment");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b, _c) = ann_bo_cy(server.port, plans_workspace("comment"));
    let comment = |d: &Value, id: u64| -> Option<Value> {
        find(d, "tasks", 72)?["activity"]
            .as_array()?
            .iter()
            .find(|x| x["id"] == id)
            .cloned()
    };
    // a comment moves the task's `updated`, as the app does
    b.change(|d| {
        let t = find_mut(d, "tasks", 72);
        t["updated"] = json!(999);
        t["activity"].as_array_mut().unwrap().push(
            json!({"id": 9010, "ts": 5, "author": 2, "text": "Bo's comment", "comment": true}),
        );
    });
    a.wait("Bo's comment at Ann", |d| comment(d, 9010).is_some());
    assert_eq!(find(&a.data, "tasks", 72).unwrap()["updated"], 0);
    round_trip(&mut b, &mut a, "after the comment", 3010);
    b.pump();
    assert!(b.put_back.is_empty(), "a comment is fine: {:?}", b.put_back);
    // his own comment: edited, then deleted
    b.change(|d| {
        let t = find_mut(d, "tasks", 72);
        let c = t["activity"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|x| x["id"] == 9010)
            .unwrap();
        c["text"] = json!("Bo's comment, edited");
    });
    a.wait("the edit at Ann", |d| {
        comment(d, 9010).is_some_and(|c| c["text"] == "Bo's comment, edited")
    });
    b.change(|d| {
        find_mut(d, "tasks", 72)["activity"]
            .as_array_mut()
            .unwrap()
            .retain(|x| x["id"] != 9010);
    });
    a.wait("the delete at Ann", |d| comment(d, 9010).is_none());
    // Ann's comment and the task stay hers
    b.change(|d| {
        let t = find_mut(d, "tasks", 72);
        t["name"] = json!("Renamed by Bo");
        t["activity"][0]["text"] = json!("Bo rewrote Ann");
        t["activity"][0]["author"] = json!(2);
    });
    b.until("Bo is told", |b| !b.put_back.is_empty());
    b.wait("Bo's copy is the team's again", |d| {
        find(d, "tasks", 72).is_some_and(|t| t["name"] == "Secret task")
            && comment(d, 76).is_some_and(|c| c["text"] == "secret comment" && c["author"] == 1)
    });
    round_trip(&mut b, &mut a, "after the rename", 3011);
    assert_eq!(find(&a.data, "tasks", 72).unwrap()["name"], "Secret task");
    assert_eq!(comment(&a.data, 76).unwrap()["text"], "secret comment");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Bo may edit in Plans: tasks yes, but the space's settings, who it's
/// shared with and deleting it need Full (or an owner or admin). Once Ann
/// gives him Full, he shares it with Cy.
#[test]
fn only_full_members_and_admins_change_a_spaces_sharing() {
    let home = temp_home("sharefull");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b, mut c) = ann_bo_cy(server.port, plans_workspace("edit"));
    b.change(|d| {
        find_mut(d, "tasks", 72)["name"] = json!("Edited by Bo");
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 9020, "list_id": 71, "name": "Bo's task", "activity": []}));
        find_mut(d, "spaces", 70)["lists"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 9021, "name": "Bo's list"}));
    });
    a.wait("Bo's edits at Ann", |d| {
        find(d, "tasks", 72).is_some_and(|t| t["name"] == "Edited by Bo")
            && find(d, "tasks", 9020).is_some()
            && d.to_string().contains("Bo's list")
    });
    // his own sharing change and a rename of the space go back
    b.change(|d| {
        let sp = find_mut(d, "spaces", 70);
        sp["name"] = json!("Bo's space");
        sp["sharing"]["people"] =
            json!([{"member": 2, "level": "full"}, {"member": 3, "level": "view"}]);
    });
    b.until("Bo is told", |b| !b.put_back.is_empty());
    b.wait("Bo's copy has the team's sharing again", |d| {
        find(d, "spaces", 70).is_some_and(|s| {
            s["name"] == "Plans"
                && s["sharing"]["people"] == json!([{"member": 2, "level": "edit"}])
        })
    });
    round_trip(&mut b, &mut a, "after the sharing change", 3020);
    assert_eq!(find(&a.data, "spaces", 70).unwrap()["name"], "Plans");
    round_trip(&mut a, &mut c, "Cy hears", 2020);
    assert!(find(&c.data, "spaces", 70).is_none(), "still not Cy's");
    // nor by sending the sharing as a list of items (which merged as items
    // and opened the space to everyone)
    b.change(|d| find_mut(d, "spaces", 70)["sharing"] = json!([{"id": 987654321}]));
    round_trip(&mut b, &mut a, "after the sharing list", 3022);
    assert_eq!(
        find(&a.data, "spaces", 70).unwrap()["sharing"]["people"],
        json!([{"member": 2, "level": "edit"}])
    );
    round_trip(&mut a, &mut c, "Cy hears again", 2021);
    assert!(find(&c.data, "spaces", 70).is_none(), "still not Cy's");
    // deleting the space: it comes back with everything in it
    b.change(|d| {
        d["spaces"]
            .as_array_mut()
            .unwrap()
            .retain(|s| s["id"] != 70);
        d["tasks"].as_array_mut().unwrap().retain(|t| t["id"] != 72);
    });
    assert!(b.meta().tombs.contains_key(&70));
    b.wait("Plans back in Bo's copy", |d| {
        find(d, "spaces", 70).is_some_and(|s| s.to_string().contains("Secret list"))
            && find(d, "tasks", 72).is_some()
    });
    round_trip(&mut b, &mut a, "after the delete", 3021);
    assert!(find(&a.data, "spaces", 70).is_some() && find(&a.data, "tasks", 72).is_some());
    // with Full he may share it
    a.change(|d| {
        find_mut(d, "spaces", 70)["sharing"]["people"] = json!([{"member": 2, "level": "full"}]);
    });
    b.wait("Bo has Full", |d| {
        find(d, "spaces", 70).is_some_and(|s| s["sharing"]["people"][0]["level"] == "full")
    });
    b.change(|d| {
        find_mut(d, "spaces", 70)["sharing"]["people"]
            .as_array_mut()
            .unwrap()
            .push(json!({"member": 3, "level": "view"}));
    });
    c.wait("Cy gets Plans", |d| {
        find(d, "spaces", 70).is_some() && find(d, "tasks", 72).is_some()
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// When Ann takes Plans away from Bo it leaves his copy without being
/// deleted: his copies without it (and a stale one that deletes it) take
/// nothing from the team, and he gets it all back when it's shared again.
#[test]
fn losing_access_to_a_space_deletes_nothing() {
    let home = temp_home("sharelose");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b, _c) = ann_bo_cy(server.port, plans_workspace("edit"));
    let stale = b.data.clone();
    a.change(|d| find_mut(d, "spaces", 70)["sharing"]["people"] = json!([]));
    b.wait("Plans leaves Bo's copy", has_none);
    let m = b.meta();
    for id in [70, 71, 72, 73, 74, 75, 90, 91, 95, 96] {
        assert!(!m.tombs.contains_key(&id), "{} isn't deleted", id);
    }
    round_trip(&mut b, &mut a, "Bo without Plans", 3030);
    assert!(has_all(&a.data), "everything stays: {}", a.data);
    // a stale copy that still has Plans, edits it, then deletes it
    b.data["spaces"] = stale["spaces"].clone();
    b.data["tasks"] = stale["tasks"].clone();
    b.data["docs"] = stale["docs"].clone();
    b.change(|d| find_mut(d, "tasks", 72)["name"] = json!("Bo, from before"));
    b.change(|d| {
        d["spaces"]
            .as_array_mut()
            .unwrap()
            .retain(|s| s["id"] != 70);
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .retain(|t| t["id"] != 72 && t["id"] != 75);
        d["docs"] = json!([]);
    });
    assert!(b.meta().tombs.contains_key(&72));
    round_trip(&mut b, &mut a, "after the stale copy", 3031);
    assert!(has_all(&a.data), "still everything: {}", a.data);
    assert_eq!(find(&a.data, "tasks", 72).unwrap()["name"], "Secret task");
    // shared again: back in Bo's copy
    a.change(|d| {
        find_mut(d, "spaces", 70)["sharing"]["people"] = json!([{"member": 2, "level": "view"}])
    });
    b.wait("Plans back at Bo", has_all);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// A folder moved out of a space closed to Cy into one Cy sees reaches Cy
/// with the list in it (and the list's task and form), though none of them
/// changed, so the change alone doesn't hold them.
#[test]
fn a_folder_moved_into_a_space_someone_sees_comes_with_its_lists() {
    let home = temp_home("sharemove");
    let server = Server::start_at(&home, 0);
    let (mut a, _b, mut c) = ann_bo_cy(server.port, plans_workspace("view"));
    assert!(has_none(&c.data), "Cy gets none of Plans: {}", c.data);
    a.change(|d| {
        let f = find_mut(d, "spaces", 70)["folders"]
            .as_array_mut()
            .unwrap()
            .remove(0);
        find_mut(d, "spaces", 60)["folders"]
            .as_array_mut()
            .unwrap()
            .push(f);
    });
    c.wait("the folder reaches Cy", |d| {
        find(d, "spaces", 60).is_some_and(|s| s["folders"][0]["id"] == 73)
    });
    let team = find(&c.data, "spaces", 60).unwrap();
    assert_eq!(
        team["folders"][0]["lists"][0]["name"], "Folder list",
        "with its list: {}",
        team
    );
    assert!(find(&c.data, "tasks", 75).is_some(), "and the task on it");
    assert!(
        find(&c.data, "extra.forms", 96).is_some(),
        "and the form on it"
    );
    assert!(find(&c.data, "spaces", 70).is_none(), "Plans stays closed");
    let text = c.data.to_string();
    for s in ["Secret list", "Secret task", "Secret doc"] {
        assert!(!text.contains(s), "{} stays out", s);
    }
    // Cy's next change keeps it all for the team
    round_trip(&mut c, &mut a, "after the move", 3040);
    let team = find(&a.data, "spaces", 60).unwrap();
    assert_eq!(team["folders"][0]["lists"][0]["id"], 74);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Made an admin, Cy hears the new role before the copy it opens up: an
/// app goes by its role for what it may see, and the copy with Plans must
/// not arrive while it still thinks it's a member.
#[test]
fn a_new_role_arrives_before_the_copy_it_opens_up() {
    let home = temp_home("roleorder");
    let server = Server::start_at(&home, 0);
    let (a, _b, mut c) = ann_bo_cy(server.port, plans_workspace("view"));
    assert!(has_none(&c.data));
    c.heard.clear();
    a.client.send(Msg::SetRole {
        member: 3,
        role: Role::Admin,
    });
    c.wait("Cy, now an admin, gets Plans", has_all);
    let first_copy = c.heard.iter().position(|h| h.starts_with("copy")).unwrap();
    let role = c
        .heard
        .iter()
        .position(|h| h == "role Admin")
        .expect("Cy hears the new role");
    assert!(role < first_copy, "the role first: {:?}", c.heard);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_slack_import_of_the_last_day_sends_no_iphone_pushes() {
    let home = temp_home("push-imported");
    let fake = Arc::new(FakePush::default());
    let server = Server::start_with(
        &home,
        0,
        workboard_server::Options {
            push: Some(fake.clone()),
            ..Default::default()
        },
    );
    let port = server.port;
    let (mut a, mut b) = two_apps(port);
    let ann = "a1".repeat(32);
    phone(&mut a, port, 1, &ann, &home);
    // Bo imports a Slack export: a DM to Ann, a mention of her and a reply
    // in her thread, all from the last hour
    b.change(|d| {
        let ts = now() / 1000 - 3600;
        d["channels"].as_array_mut().unwrap().push(json!({"id": 50, "name": "", "kind": "Direct", "topic": "", "members": [1, 2], "messages": [{"id": 51, "author": 2, "text": "from Slack", "ts": ts, "imported": "slack:D1:1.1"}], "last_read": 0}));
        d["channels"][0]["messages"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 52, "author": 2, "text": "@Ann from Slack", "ts": ts, "imported": "slack:C1:1.2"}));
        d["channels"][0]["messages"][0]["replies"] =
            json!([{"id": 53, "author": 2, "text": "old reply", "ts": ts, "imported": "slack:C1:1.3"}]);
    });
    a.wait("the import at Ann's computer", |d| channel(d, 50).is_some());
    // then a message written here: only that one pushes
    b.change(in_general("@Ann written here", 54, 2));
    wait_for("the new mention pushed", || fake.to(&ann).len() == 1);
    std::thread::sleep(Duration::from_millis(300));
    let sent = fake.to(&ann);
    assert_eq!(
        sent.len(),
        1,
        "nothing for the imported messages: {:?}",
        sent
    );
    assert_eq!(sent[0]["aps"]["alert"]["body"], "@Ann written here");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn an_app_on_sync_format_3_is_told_to_update_and_its_changes_are_not_taken() {
    // format 3 notes a whole time entry list or custom field map as one
    // field, so its copy could undo a teammate's entry
    assert_eq!(wb_net::OLDEST_SYNC, 4);
    assert_eq!(wb_sync::META_V, wb_net::OLDEST_SYNC);
    let home = temp_home("sync-3");
    let server = Server::start_at(&home, 0);
    let (mut a, b) = two_apps(server.port);
    let token = b.token.clone();
    let device = b.device.clone();
    drop(b);
    let mut old = App::start(
        server.port,
        Value::Null,
        SignIn {
            token,
            device,
            app: Some("0.1.9".into()),
            sync: Some(3),
            ..Default::default()
        },
    );
    old.wait("the round-3 app still gets the team's copy", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    old.change(|d| d["tasks"][0]["name"] = json!("Plan (round 3)"));
    old.until("the round-3 app is told to update", |o| {
        o.denied.iter().any(|d| d == wb_net::UPDATE_NEEDED)
    });
    a.change(|d| d["tasks"][1]["name"] = json!("Ship (A)"));
    old.wait("A's change at the round-3 app", |d| {
        d["tasks"][1]["name"] == "Ship (A)"
    });
    a.wait("A's copy", |d| d["tasks"][1]["name"] == "Ship (A)");
    assert_eq!(
        a.data["tasks"][0]["name"], "Plan",
        "the round-3 app's change was not taken"
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Everything a bare connection gets up to the pong for a ping sent now.
fn until_pong(raw: &mut Raw) -> Vec<Value> {
    raw.send(json!({"type": "ping"}));
    let mut got = vec![];
    loop {
        let m = raw.0.read().expect("a message from the server");
        if let tungstenite::Message::Text(t) = m {
            let v: Value = serde_json::from_str(t.as_str()).unwrap();
            if v["type"] == "pong" {
                return got;
            }
            got.push(v);
        }
    }
}

/// Each access request gets one answer on its connection: the access list
/// a change sends to every connection doesn't also go to the one whose
/// request made it, so back-to-back requests can't take each other's
/// answers. Answers say they are answers; the lists others get say they
/// weren't asked for.
#[test]
fn each_access_request_gets_one_answer_on_its_connection() {
    let home = temp_home("one-answer");
    let server = Server::start_at(&home, 0);
    let (a, _b) = two_apps(server.port);
    let hello =
        json!({"type": "hello", "device": a.device, "token": a.token, "sync": wb_sync::META_V});
    let mut asker = Raw::connect(server.port, hello.clone());
    assert_eq!(asker.next("welcome")["role"], "owner");
    let first = asker.next("access");
    assert_eq!(
        first["answer"], false,
        "the list after signing in wasn't asked for"
    );
    let mut other = Raw::connect(server.port, hello);
    other.next("welcome");
    other.next("access");
    // whatever the other connection's sign-in sent has arrived
    until_pong(&mut asker);
    until_pong(&mut other);

    asker.send(json!({"type": "set_role", "member": 2, "role": "admin"}));
    asker.send(json!({"type": "revoke", "member": 1}));
    asker.send(json!({"type": "access_list"}));
    asker.send(json!({"type": "create_invite", "member": 2, "role": "member"}));
    asker.send(json!({"type": "set_role", "member": 2, "role": "member"}));
    let got: Vec<Value> = until_pong(&mut asker)
        .into_iter()
        .filter(|m| m["type"] != "update" && m["type"] != "delta")
        .collect();
    let kinds: Vec<&str> = got.iter().map(|m| m["type"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["access", "error", "access", "invite", "access"],
        "one answer per request, in order: {:?}",
        got
    );
    assert!(got[1]["message"].as_str().unwrap().contains("owner"));
    assert_eq!(got[1]["code"], wb_net::CODE_DENIED);
    for i in [0, 2, 4] {
        assert_eq!(got[i]["answer"], true, "{}", got[i]);
    }
    let role_of_2 = |m: &Value| {
        m["people"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["member"] == 2)
            .unwrap()["role"]
            .clone()
    };
    assert_eq!(role_of_2(&got[0]), "admin");
    assert_eq!(role_of_2(&got[4]), "member");

    // the owner's other connection hears each change once, unasked (the
    // refusal and the plain list changed nothing)
    let heard: Vec<Value> = until_pong(&mut other)
        .into_iter()
        .filter(|m| m["type"] == "access")
        .collect();
    assert_eq!(heard.len(), 3, "{:?}", heard);
    assert!(heard.iter().all(|m| m["answer"] == false), "{:?}", heard);
    assert_eq!(role_of_2(&heard[2]), "member");
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------- behind an HTTPS front

/// A TLS front before the server, the way Cloudflare Tunnel puts one there:
/// apps reach https://localhost:<port>, it passes the bytes on to the
/// server's plain port. `cert`/`key` are DER files in tests/tls.
fn tls_front(server_port: u16, cert: &str, key: &str) -> u16 {
    use tokio_rustls::rustls;
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tls");
    let cert = rustls::pki_types::CertificateDer::from(std::fs::read(dir.join(cert)).unwrap());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(std::fs::read(dir.join(key)).unwrap().into());
    let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![cert], key)
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(cfg));
    let (port_tx, port_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let l = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .unwrap();
            port_tx.send(l.local_addr().unwrap().port()).unwrap();
            loop {
                let Ok((sock, _)) = l.accept().await else {
                    return;
                };
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    let Ok(mut tls) = acceptor.accept(sock).await else {
                        return;
                    };
                    let Ok(mut back) =
                        tokio::net::TcpStream::connect(("127.0.0.1", server_port)).await
                    else {
                        return;
                    };
                    let _ = tokio::io::copy_bidirectional(&mut tls, &mut back).await;
                    use tokio::io::AsyncWriteExt;
                    let _ = tls.shutdown().await;
                });
            }
        });
    });
    port_rx.recv_timeout(Duration::from_secs(10)).unwrap()
}

fn trust_test_certificate() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tls");
        wb_net::trust_certificate(std::fs::read(dir.join("cert.der")).unwrap());
    });
}

#[test]
fn apps_work_through_an_https_front() {
    trust_test_certificate();
    let home = temp_home("https-front");
    let server = Server::start_at(&home, 0);
    let front = tls_front(server.port, "cert.der", "key.der");
    let addr = format!("https://localhost:{}", front);

    // the owner sets up the server through the front, B joins through it
    let mut d = workspace();
    let mut meta = SyncMeta::default();
    wb_sync::stamp(&d, &mut meta, now());
    d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    let mut a = App::start_at(
        &addr,
        d,
        SignIn {
            claim: Some(1),
            ..Default::default()
        },
    );
    a.wait("A's copy on the server", |_| true);
    a.until("A is the owner", |a| a.role == Some(Role::Owner));
    let code = a.invite(2, Role::Member);
    let mut b = App::start_at(
        &addr,
        Value::Null,
        SignIn {
            invite: Some(code),
            ..Default::default()
        },
    );
    b.wait("B gets the team's copy", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    a.change(say("over TLS", 500));
    b.wait("A's message reaches B", |d| {
        texts(d).contains(&"over TLS".to_string())
    });

    // a picture goes up and comes down through the front too
    a.until("A has a token", |a| a.token.is_some());
    b.until("B has a token", |b| b.token.is_some());
    let (path, sha, bytes) = some_file(&home.join("work"), "photo.jpg", 2 << 20);
    wb_net::upload(
        &addr,
        &auth(&a),
        &sha,
        &path,
        "image/jpeg",
        &mut no_progress(),
    )
    .unwrap();
    assert_eq!(
        wb_net::remote_size(&addr, &auth(&b), &sha).unwrap(),
        Some(bytes.len() as u64)
    );
    let dest = home.join("work").join("b-cache").join(&sha);
    wb_net::download(&addr, &auth(&b), &sha, &dest, &mut no_progress()).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);
    assert_eq!(
        wb_net::forgot_password(&addr, "nobody@team.io"),
        Ok(false),
        "no mail set up on this server"
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_front_with_a_certificate_nobody_trusts_is_refused() {
    trust_test_certificate();
    let home = temp_home("https-untrusted");
    let server = Server::start_at(&home, 0);
    let front = tls_front(server.port, "untrusted-cert.der", "untrusted-key.der");
    let addr = format!("https://localhost:{}", front);
    let mut a = App::connect_at(&addr, workspace());
    a.until("refused or retrying", |a| {
        matches!(&a.status, Some(Status::Reconnecting(why)) if why.contains("secure connection"))
    });
    assert_eq!(a.role, None, "never signed in");
    let err = wb_net::remote_size(
        &addr,
        &wb_net::Auth {
            device: "d".into(),
            token: "t".into(),
        },
        &"0".repeat(64),
    )
    .unwrap_err();
    assert!(matches!(err, wb_net::FileError::Net(_)), "{:?}", err);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// What Cloudflare Tunnel adds to a request it passes on from the internet.
const VIA_TUNNEL: &str = "Host: wb.example.com\r\nX-Forwarded-Proto: https\r\nX-Forwarded-For: 203.0.113.9\r\nCf-Connecting-Ip: 203.0.113.9\r\n";

#[test]
fn through_the_tunnel_links_are_https_and_health_says_little() {
    let home = temp_home("via-tunnel");
    let server = Server::start_at(&home, 0);

    // the join page gives the address the browser came to, as https
    let (_, body) = http_get_with(server.port, "/join?code=ABCD-EFGH-JKMN", VIA_TUNNEL);
    let body = String::from_utf8(body).unwrap();
    assert!(
        body.contains(
            r#"href="workboard://join?server=https%3A%2F%2Fwb.example.com&amp;code=ABCD-EFGH-JKMN""#
        ),
        "{}",
        body
    );
    assert!(body.contains("<dd>https://wb.example.com</dd>"), "{}", body);
    let link = wb_net::parse_join(
        "workboard://join?server=https%3A%2F%2Fwb.example.com&code=ABCD-EFGH-JKMN",
    );
    assert_eq!(
        wb_net::server_url(link.server.as_deref().unwrap())
            .unwrap()
            .2,
        "wss://wb.example.com/ws",
        "the app takes the link's server as the secure address"
    );

    // from the internet /health says it is up and nothing more
    let (head, body) = http_get_with(server.port, "/health", VIA_TUNNEL);
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v, json!({"ok": true}));
    // asked on the Mac mini itself: everything, as before
    let (_, body) = http_get(server.port, "/health");
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
    assert!(
        v.get("devices").is_some() && v.get("rev").is_some(),
        "{}",
        v
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn one_server_answers_on_two_addresses_as_one_team() {
    // --bind with two addresses (say 127.0.0.1 for cloudflared and a LAN one)
    let home = temp_home("two-addresses");
    let (ports_tx, ports_rx) = std::sync::mpsc::channel();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let h = home.clone();
    let thread = std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let one = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                    .await
                    .unwrap();
                let two = tokio::net::TcpListener::bind(("127.0.0.1", 0))
                    .await
                    .unwrap();
                ports_tx
                    .send((
                        one.local_addr().unwrap().port(),
                        two.local_addr().unwrap().port(),
                    ))
                    .unwrap();
                workboard_server::serve_on(vec![one, two], h, Default::default(), async {
                    let _ = stop_rx.await;
                })
                .await
                .unwrap();
            });
    });
    let (one, two) = ports_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut a = App::connect(one, Some(workspace()));
    a.until("A is the owner", |a| a.role == Some(Role::Owner));
    let code = a.invite(2, Role::Member);
    let mut b = App::join(two, &code);
    b.wait("B, on the other address, gets the team's copy", |d| {
        task_names(d) == ["Plan", "Ship"]
    });
    b.change(say_as("from the other side", 600, 2));
    a.wait("B's message reaches A", |d| {
        texts(d).contains(&"from the other side".to_string())
    });
    let _ = stop_tx.send(());
    thread.join().unwrap();
    assert!(
        std::fs::read_to_string(home.join(workboard_server::WORKSPACE_FILE))
            .unwrap()
            .contains("from the other side"),
        "saved when it stopped"
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_plain_member_cant_change_people_or_team_settings_and_gets_the_teams_version_back() {
    let home = temp_home("memberguard");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    // the workspace has a colour, and Bo's app has saved once, so a changed
    // setting gets a stamp (as in the app)
    a.change(|_| {});
    // and team-wide things Ann set up: features, a field, a rule, a template, a team
    a.change(|d| {
        d["extra"]["workspace_color"] = json!([9, 9, 9]);
        d["extra"]["apps"] = json!({"priorities": true, "tags": true});
        d["extra"]["fields"] = json!([{"id": 501, "name": "Budget", "kind": "number", "options": []}]);
        d["extra"]["automations"] = json!([{"id": 502, "enabled": true, "trigger_status": "complete", "action": "comment", "value": "done"}]);
        d["extra"]["templates"] = json!([{"id": 503, "name": "Bug", "task_name": "Bug"}]);
        d["extra"]["teams"] = json!([{"id": 504, "name": "Design", "members": [1], "color": [1, 1, 1]}]);
    });
    b.wait("Ann's setup reaches Bo", |d| {
        d["extra"]["workspace_color"] == json!([9, 9, 9])
            && d["extra"]["teams"][0]["id"] == 504
            && d["extra"]["apps"]["priorities"] == true
    });
    b.change(|_| {});
    b.change(|d| {
        let m = d["members"].as_array_mut().unwrap();
        // invites someone, takes Cy out, renames Ann, makes themselves admin
        m.push(json!({"id": 9001, "name": "New person", "email": "new@team.io", "role": "Member", "invited": true}));
        m.retain(|x| x["id"] != 3);
        m[0]["name"] = json!("Bo renamed Ann");
        m[1]["role"] = json!("Admin");
        // their own name and status are theirs to change
        m[1]["name"] = json!("Bo B.");
        m[1]["status_text"] = json!("at lunch");
        d["workspace"] = json!("Bo's workspace");
        d["extra"]["workspace_color"] = json!([1, 2, 3]);
        // features off, a field renamed, a rule deleted, a new template, joins a team
        d["extra"]["apps"]["priorities"] = json!(false);
        d["extra"]["fields"][0]["name"] = json!("Bo's field");
        d["extra"]["automations"] = json!([]);
        d["extra"]["templates"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 9002, "name": "Bo's template", "task_name": "x"}));
        d["extra"]["teams"][0]["members"] = json!([1, 2]);
    });
    assert!(b.meta().tombs.contains_key(&3));
    b.until("Bo is told", |b| !b.put_back.is_empty());
    let team_again = |d: &Value| {
        let ids: Vec<u64> = d["members"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m["id"].as_u64())
            .collect();
        ids == [1, 2, 3]
            && d["members"][0]["name"] == "Ann"
            && d["members"][1]["role"] == "Member"
            && d["members"][1]["name"] == "Bo B."
            && d["members"][1]["status_text"] == "at lunch"
            && d["workspace"] == "Team"
            && d["extra"]["workspace_color"] == json!([9, 9, 9])
            && d["extra"]["apps"]["priorities"] == true
            && d["extra"]["fields"][0]["name"] == "Budget"
            && d["extra"]["automations"][0]["id"] == 502
            && d["extra"]["templates"].as_array().unwrap().len() == 1
            && d["extra"]["teams"][0]["members"] == json!([1])
    };
    b.wait(
        "Bo's copy is the team's again, with Bo's own name",
        team_again,
    );
    a.wait("Ann has only Bo's own changes", team_again);
    round_trip(&mut b, &mut a, "after Bo", 3002);
    assert!(team_again(&a.data) && team_again(&b.data));
    // nothing comes back with the next copy
    b.put_back.clear();
    round_trip(&mut b, &mut a, "again", 3003);
    b.pump();
    assert!(
        b.put_back.is_empty(),
        "nothing turned down: {:?}",
        b.put_back
    );
    // once admin, the same changes go through
    a.client.send(Msg::SetRole {
        member: 2,
        role: Role::Admin,
    });
    b.until("B learns it is admin", |b| b.role == Some(Role::Admin));
    b.wait("B's name shows Admin", |d| {
        d["members"][1]["role"] == "Admin"
    });
    b.change(|d| {
        let m = d["members"].as_array_mut().unwrap();
        m.push(json!({"id": 9005, "name": "Dee", "email": "dee@team.io", "role": "Member", "invited": true}));
        m[2]["name"] = json!("Cy C.");
        d["workspace"] = json!("Team 2");
        d["extra"]["teams"][0]["members"] = json!([1, 2]);
    });
    a.wait("an admin's changes reach Ann", |d| {
        d["workspace"] == "Team 2"
            && d["extra"]["teams"][0]["members"] == json!([1, 2])
            && d["members"][2]["name"] == "Cy C."
            && d["members"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["id"] == 9005)
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// A plain member sending a copy made by hand: they can't post as someone
/// else, change or delete someone else's message or comment, or bring in
/// "imported" history; reacting to Ann's message, their own messages and
/// comments, and deleting their own still go through.
#[test]
fn a_plain_member_cant_write_as_someone_else_or_change_their_words() {
    let home = temp_home("authors");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| {
        d["tasks"][0]["activity"] = json!([
            {"id": 8001, "ts": 1, "author": 1, "text": "Ann's note", "comment": true},
            {"id": 8004, "ts": 2, "author": 1, "text": "Ann's question", "comment": true}
        ]);
        d["channels"][0]["messages"][0]["replies"] =
            json!([{"id": 12, "author": 1, "text": "Ann's reply", "ts": 2}]);
    });
    b.wait("Ann's comments reach Bo", |d| {
        d["tasks"][0]["activity"].as_array().unwrap().len() == 2
            && d["channels"][0]["messages"][0]["replies"][0]["id"] == 12
    });
    b.change(|d| {
        let msgs = d["channels"][0]["messages"].as_array_mut().unwrap();
        msgs[0]["text"] = json!("Ann says Bo is boss");
        msgs[0]["reactions"] = json!([{"emoji": "👍", "users": [2]}]);
        msgs[0]["replies"][0]["text"] = json!("changed reply");
        msgs.push(json!({"id": 7001, "author": 1, "text": "forged by Bo", "ts": now()}));
        msgs.push(json!({"id": 7002, "author": 2, "text": "Bo's own", "ts": now()}));
        msgs.push(json!({"id": 7003, "author": 2, "text": "fake history", "ts": 1, "imported": "slack:C1:1"}));
        msgs.push(json!({"author": 1, "text": "no id", "ts": now()}));
        let acts = d["tasks"][0]["activity"].as_array_mut().unwrap();
        acts[0]["text"] = json!("Ann's note, edited by Bo");
        acts[1]["resolved"] = json!(true);
        acts.push(json!({"id": 8002, "ts": 3, "author": 1, "text": "forged comment", "comment": true}));
        acts.push(json!({"id": 8003, "ts": 4, "author": 2, "text": "Bo's comment", "comment": true}));
    });
    b.until("Bo is told", |b| !b.put_back.is_empty());
    let honest = |d: &Value| {
        let text = d.to_string();
        let m = &d["channels"][0]["messages"];
        m[0]["text"] == "hi"
            && m[0]["replies"][0]["text"] == "Ann's reply"
            && m[0]["reactions"][0]["users"] == json!([2])
            && text.contains("Bo's own")
            && text.contains("Bo's comment")
            && text.contains("Ann's note\"")
            && d["tasks"][0]["activity"][1]["resolved"] == true
            && !text.contains("forged")
            && !text.contains("fake history")
            && !text.contains("edited by Bo")
            && !text.contains("Bo is boss")
    };
    a.wait("Ann has only Bo's honest changes", |d| {
        honest(d) && !d.to_string().contains("no id")
    });
    // (the entry without an id stays only in Bo's own copy)
    b.wait("Bo's copy is the team's again", honest);
    // deleting Ann's message, reply and comment does nothing; Bo's own go
    b.change(|d| {
        let msgs = d["channels"][0]["messages"].as_array_mut().unwrap();
        msgs[0]["replies"] = json!([]);
        msgs.retain(|m| m["id"] != 7002);
        let m11 = msgs.iter().position(|m| m["id"] == 11).unwrap();
        msgs.remove(m11);
        d["tasks"][0]["activity"]
            .as_array_mut()
            .unwrap()
            .retain(|x| x["author"] != 1 && x["id"] != 8003);
    });
    assert!(b.meta().tombs.contains_key(&11));
    round_trip(&mut b, &mut a, "after the deletes", 3010);
    let text = a.data.to_string();
    assert!(!text.contains("Bo's own") && !text.contains("Bo's comment"));
    assert!(
        text.contains("\"hi\"") && text.contains("Ann's reply") && text.contains("Ann's question"),
        "Ann's words stay: {}",
        a.data["channels"][0]
    );
    b.wait("Bo gets Ann's words back", |d| {
        let text = d.to_string();
        text.contains("\"hi\"") && text.contains("Ann's reply") && text.contains("Ann's note\"")
    });
    // an admin is trusted as before (Import from Slack brings others' words)
    a.client.send(Msg::SetRole {
        member: 2,
        role: Role::Admin,
    });
    b.until("B learns it is admin", |b| b.role == Some(Role::Admin));
    b.change(|d| {
        d["channels"][0]["messages"].as_array_mut().unwrap().push(
            json!({"id": 7010, "author": 1, "text": "imported for Ann", "ts": 1, "imported": "slack:C1:2"}),
        );
    });
    a.wait("an admin's import reaches Ann", |d| {
        d.to_string().contains("imported for Ann")
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Who a Slack user comes in as is set by Import from Slack, which only the
/// owner and admins run: a plain member's copy can't change it.
#[test]
fn a_plain_member_cant_change_who_slack_users_come_in_as() {
    let home = temp_home("slackmap");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| d["extra"]["slack_map"] = json!({"U1": 1}));
    b.wait("the map reaches Bo", |d| d["extra"]["slack_map"]["U1"] == 1);
    b.change(|d| d["extra"]["slack_map"] = json!({"U1": 2, "U2": 2}));
    b.until("Bo is told", |b| !b.put_back.is_empty());
    round_trip(&mut b, &mut a, "after the map", 3020);
    assert_eq!(a.data["extra"]["slack_map"], json!({"U1": 1}));
    b.wait("Bo has the team's map again", |d| {
        d["extra"]["slack_map"] == json!({"U1": 1})
    });
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Admins are the owner's choice: an admin can't take another admin's role
/// or access away (removing access deletes their account too). They still
/// step down themselves, and the owner does both.
#[test]
fn only_the_owner_changes_or_removes_another_admin() {
    let home = temp_home("adminadmin");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.client.send(Msg::SetRole {
        member: 2,
        role: Role::Admin,
    });
    b.until("Bo is admin", |b| b.role == Some(Role::Admin));
    let code = a.invite(3, Role::Admin);
    let mut c = App::join(server.port, &code);
    c.until("Cy is admin", |c| c.role == Some(Role::Admin));
    let role_of = |app: &App, m: u64| app.people.iter().find(|p| p.member == m).map(|p| p.role);
    b.client.send(Msg::SetRole {
        member: 3,
        role: Role::Member,
    });
    b.until("Bo can't demote Cy", |b| {
        b.denied.iter().any(|d| d.contains("another admin's role"))
    });
    b.client.send(Msg::Revoke { member: 3 });
    b.until("Bo can't remove Cy", |b| {
        b.denied
            .iter()
            .any(|d| d.contains("another admin's access"))
    });
    b.client.send(Msg::AccessList);
    b.until("Cy is still an admin", |b| {
        role_of(b, 3) == Some(Role::Admin)
    });
    c.pump();
    assert_eq!(c.role, Some(Role::Admin));
    assert!(
        matches!(c.status, Some(Status::Connected)),
        "{:?}",
        c.status
    );
    // Bo steps down; the owner demotes Cy
    b.client.send(Msg::SetRole {
        member: 2,
        role: Role::Member,
    });
    b.until("Bo is a member", |b| b.role == Some(Role::Member));
    a.client.send(Msg::SetRole {
        member: 3,
        role: Role::Member,
    });
    c.until("Cy is a member", |c| c.role == Some(Role::Member));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// An admin manages people, but the owner's own entry stays the owner's:
/// an admin's copy can't rename them, change their email or role, or take
/// them out of the list.
#[test]
fn an_admin_cant_change_or_remove_the_owners_entry() {
    let home = temp_home("ownerentry");
    let server = Server::start_at(&home, 0);
    let (mut a, mut b) = two_apps(server.port);
    a.change(|d| d["members"][0]["email"] = json!("ann@team.io"));
    a.client.send(Msg::SetRole {
        member: 2,
        role: Role::Admin,
    });
    b.until("Bo is admin", |b| b.role == Some(Role::Admin));
    b.wait("Ann's email reaches Bo", |d| {
        d["members"][0]["email"] == "ann@team.io"
    });
    b.change(|d| {
        let m = d["members"].as_array_mut().unwrap();
        m[0]["name"] = json!("Not Ann");
        m[0]["email"] = json!("bo@evil.io");
        m[0]["role"] = json!("Member");
        m[2]["name"] = json!("Cy C.");
    });
    b.until("Bo is told", |b| !b.put_back.is_empty());
    let anns = |d: &Value| {
        find(d, "members", 1).is_some_and(|m| {
            m["name"] == "Ann" && m["email"] == "ann@team.io" && m["role"] == "Owner"
        })
    };
    a.wait("Bo's change to Cy reaches Ann", |d| {
        d["members"][2]["name"] == "Cy C."
    });
    assert!(anns(&a.data), "{}", a.data["members"]);
    b.wait("Bo has Ann's entry back", anns);
    // taking Ann out of the list does nothing
    b.change(|d| {
        d["members"]
            .as_array_mut()
            .unwrap()
            .retain(|m| m["id"] != 1)
    });
    assert!(b.meta().tombs.contains_key(&1));
    round_trip(&mut b, &mut a, "after taking Ann out", 3040);
    assert!(anns(&a.data), "{}", a.data["members"]);
    b.wait("Ann is back in Bo's list", anns);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

// ------------------------------------------------- several workspaces

fn second_workspace() -> Value {
    json!({
        "workspace": "Second", "spaces": [], "members": [{"id": 1, "name": "Ann", "role": "Owner", "email": "ann@team.io"}], "me": 1, "next_id": 1, "favorites": [], "docs": [],
        "tasks": [{"id": 500, "name": "Only in second", "activity": []}],
        "channels": [{"id": 10, "name": "general", "kind": "Channel", "topic": "", "members": [], "messages": [], "last_read": 0}],
        "extra": {"sync": {}}
    })
}

fn stamped(mut d: Value) -> Value {
    let mut meta = SyncMeta::default();
    wb_sync::stamp(&d, &mut meta, now());
    d["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    d
}

/// New workspaces on a server are for the team that keeps it: the owner or
/// an admin of its first workspace. A plain member, or someone signed in
/// only to another workspace there, can't make them.
#[test]
fn only_the_first_workspaces_owner_or_admins_make_workspaces() {
    let home = temp_home("makews");
    let server = Server::start_at(&home, 0);
    let port = server.port;
    let addr = format!("127.0.0.1:{}", port);
    let mut a = App::start(
        port,
        stamped(workspace()),
        SignIn {
            claim: Some(1),
            email: Some("ann@team.io".into()),
            new_password: Some("ann-secret-1".into()),
            ..Default::default()
        },
    );
    a.until("owner in", |a| a.role == Some(Role::Owner));
    let code = a.invite(2, Role::Member);
    let mut b = App::start(
        port,
        Value::Null,
        SignIn {
            invite: Some(code),
            email: Some("bo@team.io".into()),
            new_password: Some("bo-secret-1".into()),
            ..Default::default()
        },
    );
    b.until("Bo in with a password", |b| {
        b.token.is_some() && b.email.is_some()
    });
    let refused = wb_net::create_workspace(&addr, &auth(&b)).unwrap_err();
    assert!(
        refused.contains("Only the owner or an admin"),
        "{}",
        refused
    );
    assert!(!home.join("workspaces").exists());
    // an admin may
    a.client.send(Msg::SetRole {
        member: 2,
        role: Role::Admin,
    });
    b.until("Bo is admin", |b| b.role == Some(Role::Admin));
    let addr2 = wb_net::create_workspace(&addr, &auth(&b)).unwrap();
    let mut b2 = App::start_at(
        &addr2,
        stamped(second_workspace()),
        SignIn {
            claim: Some(1),
            email: Some("bo@team.io".into()),
            ..Default::default()
        },
    );
    b2.until("Bo owns the new one", |b| {
        b.role == Some(Role::Owner) && b.token.is_some()
    });
    // owning another workspace there isn't enough
    let refused = wb_net::create_workspace(&addr2, &auth(&b2)).unwrap_err();
    assert!(
        refused.contains("Only the owner or an admin"),
        "{}",
        refused
    );
    assert_eq!(
        std::fs::read_dir(home.join("workspaces")).unwrap().count(),
        1
    );
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn one_server_keeps_several_workspaces_apart() {
    let home = mail_home("several");
    let server = Server::start_at(&home, 0);
    let port = server.port;
    let addr = format!("127.0.0.1:{}", port);

    // Ann sets up the server's first workspace with a password
    let mut a = App::start(
        port,
        stamped(emailed_workspace()),
        SignIn {
            claim: Some(1),
            email: Some("ann@team.io".into()),
            new_password: Some("ann-secret-1".into()),
            ..Default::default()
        },
    );
    a.until("owner in", |a| {
        a.role == Some(Role::Owner) && a.token.is_some()
    });

    // making a workspace needs a device signed in to this server
    let (head, _) = http_post(port, "/workspaces", "{}");
    assert!(head.starts_with("HTTP/1.1 401"), "{}", head);
    let bad = wb_net::Auth {
        device: a.device.clone(),
        token: "nope".into(),
    };
    assert!(wb_net::create_workspace(&addr, &bad).is_err());
    let addr2 = wb_net::create_workspace(&addr, &auth(&a)).unwrap();
    assert!(addr2.starts_with(&format!("{}/w/", addr)), "{}", addr2);
    let id = addr2.rsplit('/').next().unwrap().to_string();
    assert!(home.join("workspaces").join(&id).is_dir());

    // only the person who made it sets it up
    let mut other = App::start_at(
        &addr2,
        stamped(second_workspace()),
        SignIn {
            claim: Some(1),
            email: Some("bo@team.io".into()),
            ..Default::default()
        },
    );
    assert!(refused_with(&mut other, "someone else refused").contains("someone else"));

    // Ann sets it up with the account she has: no new password
    let mut a2 = App::start_at(
        &addr2,
        stamped(second_workspace()),
        SignIn {
            claim: Some(1),
            email: Some("ann@team.io".into()),
            ..Default::default()
        },
    );
    a2.until("owner of the second", |a| {
        a.role == Some(Role::Owner) && a.token.is_some()
    });
    assert_eq!(a2.email.as_deref(), Some("ann@team.io"));

    // the two copies stay apart
    a2.change(|d| {
        d["tasks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 501, "name": "Second only too", "activity": []}))
    });
    a.change(|d| d["tasks"][0]["name"] = json!("Plan (first)"));
    let mut b = sign_in(
        port,
        SignIn {
            email: Some("ann@team.io".into()),
            password: Some("ann-secret-1".into()),
            ..Default::default()
        },
    );
    b.wait("first copy", |d| {
        task_names(d).contains(&"Plan (first)".to_string())
    });
    assert!(!task_names(&b.data).iter().any(|n| n.contains("econd")));
    let mut b2 = App::start_at(
        &addr2,
        Value::Null,
        SignIn {
            email: Some("ann@team.io".into()),
            password: Some("ann-secret-1".into()),
            ..Default::default()
        },
    );
    b2.wait("second copy", |d| {
        task_names(d).contains(&"Second only too".to_string())
    });
    assert_eq!(b2.data["workspace"], "Second");
    assert!(!task_names(&b2.data).contains(&"Plan (first)".to_string()));
    assert_eq!(b2.role, Some(Role::Owner));

    // the account's workspaces, with their addresses
    let mut list = wb_net::list_workspaces(&addr2, "ANN@team.io", "ann-secret-1").unwrap();
    list.sort();
    let mut want = vec![
        (addr.clone(), "Team".to_string()),
        (addr2.clone(), "Second".to_string()),
    ];
    want.sort();
    assert_eq!(list, want);
    assert_eq!(
        wb_net::list_workspaces(&addr, "bo@team.io", "whatever-1"),
        Err("That email and password don't match an account on this server.".into())
    );
    assert!(wb_net::list_workspaces(&addr, "ann@team.io", "wrong-pass-1").is_err());

    // a device of one workspace can't use another's files
    let work = home.join("work");
    let (path, sha, _) = some_file(&work, "p.jpg", 1000);
    wb_net::upload(
        &addr2,
        &auth(&a2),
        &sha,
        &path,
        "image/jpeg",
        &mut |_, _| true,
    )
    .unwrap();
    assert!(home
        .join("workspaces")
        .join(&id)
        .join("files")
        .join(&sha)
        .is_file());
    assert!(!home.join("files").join(&sha).exists());
    assert!(wb_net::remote_size(&addr2, &auth(&a), &sha).is_err());
    assert!(wb_net::remote_size(&addr, &auth(&a2), &sha).is_err());

    // invites from the second workspace link to it
    a2.change(|d| {
        d["members"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 2, "name": "Cy", "role": "Member", "email": "cy@team.io"}))
    });
    a2.until("Cy is on the server", |a| {
        a.people.iter().any(|p| p.member == 2)
    });
    let code = a2.invite(2, Role::Member);
    let mail = wait_for_mail(&home, "cy@team.io", 1);
    assert!(
        mail.contains(&format!("http://{}/w/{}/join?code={}", addr, id, code)),
        "{}",
        mail
    );
    let (head, page) = http_get(port, &format!("/w/{}/join?code={}", id, code));
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    let page = String::from_utf8_lossy(&page);
    assert!(page.contains(&format!("%2Fw%2F{}", id)), "{}", page);
    let (head, _) = http_get(port, "/w/nosuch/join");
    assert!(head.starts_with("HTTP/1.1 404"), "{}", head);
    let mut cy = App::start_at(
        &addr2,
        Value::Null,
        SignIn {
            invite: Some(code.clone()),
            ..Default::default()
        },
    );
    cy.wait("Cy has the second copy", |d| d["workspace"] == "Second");

    // after a restart both are there and the apps get back in
    let token2 = a2.token.clone().unwrap();
    let device2 = a2.device.clone();
    drop((a, a2, b, b2, cy, other));
    server.stop();
    let server = Server::start_at(&home, port);
    let mut again = App::start_at(
        &addr2,
        Value::Null,
        SignIn {
            device: device2,
            token: Some(token2),
            ..Default::default()
        },
    );
    again.wait("second copy after restart", |d| d["workspace"] == "Second");
    assert_eq!(
        wb_net::list_workspaces(&addr, "ann@team.io", "ann-secret-1")
            .unwrap()
            .len(),
        2
    );
    drop(again);
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// The owner of a workspace other than the server's first deletes it: its
/// apps are told and stop, its address answers 410, its folder waits in
/// deleted/ and `workboard-server undelete` brings it back.
#[test]
fn an_owner_deletes_a_workspace_and_it_can_come_back() {
    let home = temp_home("deletews");
    let server = Server::start_at(&home, 0);
    let port = server.port;
    let addr = format!("127.0.0.1:{}", port);
    let mut a = App::start(
        port,
        stamped(workspace()),
        SignIn {
            claim: Some(1),
            email: Some("ann@team.io".into()),
            new_password: Some("ann-secret-1".into()),
            ..Default::default()
        },
    );
    a.until("owner in", |a| {
        a.role == Some(Role::Owner) && a.token.is_some()
    });
    // the server's first workspace can't be deleted
    assert!(wb_net::delete_workspace(&addr, &auth(&a)).is_err());

    let addr2 = wb_net::create_workspace(&addr, &auth(&a)).unwrap();
    let id = addr2.rsplit('/').next().unwrap().to_string();
    let mut a2 = App::start_at(
        &addr2,
        stamped(second_workspace()),
        SignIn {
            claim: Some(1),
            email: Some("ann@team.io".into()),
            ..Default::default()
        },
    );
    a2.until("owner of the second", |a| {
        a.role == Some(Role::Owner) && a.token.is_some()
    });
    a2.change(|d| {
        d["members"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 2, "name": "Bo", "role": "Member", "email": "bo@team.io"}))
    });
    a2.until("Bo is on the server", |a| {
        a.people.iter().any(|p| p.member == 2)
    });
    let code = a2.invite(2, Role::Member);
    let mut b2 = App::start_at(
        &addr2,
        Value::Null,
        SignIn {
            invite: Some(code),
            ..Default::default()
        },
    );
    b2.until("Bo in", |b| b.token.is_some());

    // a member can't delete it, nor a device of another workspace
    let refused = wb_net::delete_workspace(&addr2, &auth(&b2)).unwrap_err();
    assert!(refused.contains("Only an owner"), "{}", refused);
    assert!(wb_net::delete_workspace(&addr2, &auth(&a)).is_err());
    assert!(home.join("workspaces").join(&id).is_dir());

    // its owner can: everyone connected is told and stops trying
    wb_net::delete_workspace(&addr2, &auth(&a2)).unwrap();
    for app in [&mut a2, &mut b2] {
        app.wait_status(|s| matches!(s, Status::Gone(_)), "told it's gone");
    }
    assert!(!home.join("workspaces").join(&id).exists());
    let kept = workboard_server::registry::deleted(&home);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].0, id);
    assert!(kept[0].2.join(workboard_server::STATE_FILE).is_file());
    // the first workspace goes on
    a.change(|d| d["tasks"][0]["name"] = json!("Still here"));
    assert_eq!(a.status, Some(Status::Connected));

    // coming back later: gone, not "reconnecting"
    let mut late = App::start_at(
        &addr2,
        Value::Null,
        SignIn {
            device: b2.device.clone(),
            token: b2.token.clone(),
            ..Default::default()
        },
    );
    late.wait_status(|s| matches!(s, Status::Gone(_)), "late app told it's gone");
    let (head, _) = http_get(port, &format!("/w/{}/health", id));
    assert!(head.starts_with("HTTP/1.1 410"), "{}", head);
    // a password reset there says it was deleted, and one for a workspace
    // that never was says so (not that the server can't reset passwords)
    assert_eq!(
        wb_net::forgot_password(&addr2, "bo@team.io"),
        Err(wb_net::WORKSPACE_GONE.to_string())
    );
    assert_eq!(
        wb_net::forgot_password(&format!("{}/w/nosuch", addr), "bo@team.io"),
        Err("There's no such workspace on this server.".to_string())
    );
    // deleting again is fine
    wb_net::delete_workspace(&addr2, &auth(&a2)).unwrap();
    // and the list of the account's workspaces leaves it out
    assert_eq!(
        wb_net::list_workspaces(&addr, "ann@team.io", "ann-secret-1")
            .unwrap()
            .len(),
        1
    );

    // brought back with everyone's accounts
    let res = workboard_server::workspaces_request(
        &home,
        &json!({ "undelete": id }),
        Duration::from_secs(5),
    )
    .unwrap()
    .expect("the server answered");
    assert_eq!(res["ok"], true, "{}", res);
    assert!(home.join("workspaces").join(&id).is_dir());
    assert!(workboard_server::registry::deleted(&home).is_empty());
    let mut back = App::start_at(
        &addr2,
        Value::Null,
        SignIn {
            device: a2.device.clone(),
            token: a2.token.clone(),
            ..Default::default()
        },
    );
    back.wait("the second copy is back", |d| d["workspace"] == "Second");
    drop((a, a2, b2, late, back));
    server.stop();
    let _ = std::fs::remove_dir_all(&home);
}

/// Deleted workspaces are removed for good after KEEP_DELETED_DAYS, and a
/// deleted one isn't opened again when the server starts.
#[test]
fn deleted_workspaces_stay_deleted_after_a_restart() {
    let home = temp_home("deletedkeep");
    let gone = home.join("deleted");
    std::fs::create_dir_all(gone.join("abc.100")).unwrap();
    let recent = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    std::fs::create_dir_all(gone.join(format!("def.{}", recent))).unwrap();
    let server = Server::start_at(&home, 0);
    let (head, _) = http_get(server.port, "/w/def/health");
    assert!(head.starts_with("HTTP/1.1 410"), "{}", head);
    let (head, _) = http_get(server.port, "/w/nosuch/health");
    assert!(head.starts_with("HTTP/1.1 404"), "{}", head);
    server.stop();
    let ids: Vec<String> = workboard_server::registry::deleted(&home)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids, vec!["abc".to_string(), "def".to_string()]);
    // the one from 1970 is long past keeping
    assert_eq!(
        workboard_server::registry::remove_old_deleted(&home, recent),
        vec!["abc".to_string()]
    );
    assert!(!gone.join("abc.100").exists());
    assert!(gone.join(format!("def.{}", recent)).is_dir());
    let _ = std::fs::remove_dir_all(&home);
}
