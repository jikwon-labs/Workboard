// Behaviour tests for chat, the Inbox and calls: channels and direct
// messages, mentions, unread counts, notifications and huddles.

use super::*;

#[test]
fn chat_send_react_pin_thread_post_and_task() {
    let mut a = app();
    let cid = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    let before = a
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    a.apply_chat(vec![ChatAction::Send {
        ch: cid,
        text: "Hello team".into(),
        parent: None,
        files: vec![],
    }]);
    let ch = a.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(ch.messages.len(), before + 1);
    let mid = ch.messages.last().unwrap().id;
    // reactions toggle
    a.apply_chat(vec![ChatAction::React {
        ch: cid,
        msg: mid,
        emoji: "\u{1F44D}".into(),
    }]);
    let m = a
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone();
    assert_eq!(m.reactions.len(), 1);
    a.apply_chat(vec![ChatAction::React {
        ch: cid,
        msg: mid,
        emoji: "\u{1F44D}".into(),
    }]);
    assert!(a
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .reactions
        .is_empty());
    // pin
    a.apply_chat(vec![ChatAction::Pin { ch: cid, msg: mid }]);
    assert!(
        a.data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .messages
            .last()
            .unwrap()
            .pinned
    );
    // thread reply
    a.apply_chat(vec![ChatAction::Send {
        ch: cid,
        text: "Reply".into(),
        parent: Some(mid),
        files: vec![],
    }]);
    assert_eq!(
        a.data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .messages
            .last()
            .unwrap()
            .replies
            .len(),
        1
    );
    // post
    a.apply_chat(vec![ChatAction::SendPost {
        ch: cid,
        title: "Launch".into(),
        text: "We ship Friday".into(),
        files: vec![],
    }]);
    assert_eq!(
        a.data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .messages
            .last()
            .unwrap()
            .post_title
            .as_deref(),
        Some("Launch")
    );
    // message -> task
    let tasks = a.data.tasks.len();
    a.apply_chat(vec![ChatAction::Task { ch: cid, msg: mid }]);
    assert_eq!(a.data.tasks.len(), tasks + 1);
    let m = a
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .iter()
        .find(|m| m.id == mid)
        .unwrap()
        .clone();
    assert!(m.task.is_some());
    assert_eq!(a.task(m.task.unwrap()).unwrap().name, "Hello team");
}

#[test]
fn chat_new_channel_and_dm_reuse() {
    let mut a = app();
    let n = a.data.channels.len();
    a.apply_chat(vec![ChatAction::NewChannel("Design Crit".into())]);
    assert_eq!(a.data.channels.len(), n + 1);
    assert_eq!(a.data.channels.last().unwrap().name, "design-crit");
    a.apply_chat(vec![ChatAction::Dm(4)]);
    let after_first = a.data.channels.len();
    a.apply_chat(vec![ChatAction::Dm(4)]);
    assert_eq!(
        a.data.channels.len(),
        after_first,
        "opening the same DM twice reuses it"
    );
}

#[test]
fn starting_a_call_posts_a_join_link() {
    let mut a = app();
    let ch = a.data.channels[0].id;
    let link = a.start_call(None, ch, "");
    assert!(link.starts_with("https://meet.jit.si/Workboard-"));
    let m = a.data.channels[0].messages.last().unwrap();
    assert_eq!(m.call.as_deref(), Some(link.as_str()));
    assert_eq!(a.section, Section::Chat);
    let own = a.start_call(None, ch, "https://zoom.us/j/123");
    assert_eq!(
        own, "https://zoom.us/j/123",
        "your own meeting link is used as given"
    );
}

#[test]
fn a_teammates_message_through_the_server_becomes_a_notification() {
    use crate::app::notify::Kind;
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A shared on the server", |a| {
        a.data.extra.shared.is_some()
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !server.home.join(workboard_server::WORKSPACE_FILE).is_file()
        && std::time::Instant::now() < until
    {
        a.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    net_until(&mut a, "A sees the people list", |a| {
        a.sync_rt.people.iter().any(|p| p.member == 3)
    });
    a.access_request(wb_net::Msg::CreateInvite {
        member: 3,
        role: wb_net::Role::Member,
    });
    net_until(&mut a, "invite code", |a| {
        a.sync_rt
            .people
            .iter()
            .any(|p| p.member == 3 && p.invite.is_some())
    });
    let code = a
        .sync_rt
        .people
        .iter()
        .find(|p| p.member == 3)
        .unwrap()
        .invite
        .clone()
        .unwrap();
    let mut b = App::build(
        None,
        Some(Data::blank(
            "Theirs",
            Member {
                id: 1,
                name: "B".into(),
                ..Default::default()
            },
        )),
    );
    b.connect_server(&addr, &code).unwrap();
    net_until(&mut b, "B joined", |b| b.data.extra.shared.is_some());
    assert_eq!(b.data.me, 3);

    // A's window is in the background; what both have now is old news
    a.notify_rt.away = true;
    a.notify_tick(None);
    b.notify_tick(None);
    a.notify_rt.sent.clear();
    b.notify_rt.sent.clear();

    // Sam (B) writes to A directly, mentions A in #general and gives A a task
    let dm = b
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members.contains(&1) && c.members.contains(&3))
        .unwrap()
        .id;
    let g = b
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    b.apply_chat(vec![ChatAction::Send {
        ch: dm,
        text: "are you free at 3?".into(),
        parent: None,
        files: vec![],
    }]);
    b.apply_chat(vec![ChatAction::Send {
        ch: g,
        text: "@Me the build is green".into(),
        parent: None,
        files: vec![],
    }]);
    let list = b.data.first_list().unwrap();
    b.apply(Action::Add {
        list,
        name: "Review the copy".into(),
        status: None,
        priority: 0,
        assignees: vec![1],
        due: None,
        parent: None,
        open: false,
    });
    b.sync_note_change();
    b.write_shared();
    net_until(&mut a, "B's changes at A", |a| {
        a.data.tasks.iter().any(|t| t.name == "Review the copy")
            && a.data
                .channels
                .iter()
                .any(|c| c.id == dm && c.messages.iter().any(|m| m.text == "are you free at 3?"))
    });
    assert!(
        a.notify_rt.pending,
        "taking in the team's changes asks for a look"
    );
    a.notify_tick(None);
    let sent = a.notify_rt.sent.clone();
    let dm_note = sent
        .iter()
        .find(|n| n.kind == Kind::Direct)
        .unwrap_or_else(|| panic!("a direct message notice: {:?}", sent));
    assert_eq!(dm_note.title, "Sam Lee");
    assert_eq!(dm_note.body, "are you free at 3?");
    assert_eq!(dm_note.target, format!("chat:{}", dm));
    assert!(
        sent.iter()
            .any(|n| n.kind == Kind::Mention && n.body == "@Me the build is green"),
        "{:?}",
        sent
    );
    assert!(
        sent.iter()
            .any(|n| n.kind == Kind::Assigned && n.body == "Review the copy"),
        "{:?}",
        sent
    );
    assert_eq!(sent.len(), 3, "{:?}", sent);
    // and nothing twice
    a.notify_rt.pending = true;
    a.notify_tick(None);
    assert_eq!(a.notify_rt.sent.len(), 3);
    // B is never told about what B did
    b.notify_tick(None);
    assert!(b.notify_rt.sent.is_empty(), "{:?}", b.notify_rt.sent);
}

/// A local livekit-server for huddle tests (None when it isn't installed).
struct TestLiveKit(std::process::Child, u16);

impl Drop for TestLiveKit {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

const LK_SECRET: &str = "app-test-secret-app-test-secret-0123456";

fn test_livekit() -> Option<TestLiveKit> {
    use std::process::{Command, Stdio};
    let bin = [
        "livekit-server",
        "/opt/homebrew/bin/livekit-server",
        "/usr/local/bin/livekit-server",
    ]
    .into_iter()
    .find(|b| {
        Command::new(b)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })?;
    let free = || {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    };
    let (port, rtc) = (free(), free());
    let config = format!("port: {port}\nbind_addresses: [\"127.0.0.1\"]\nrtc:\n  tcp_port: {rtc}\n  udp_port: {rtc}\n  use_external_ip: false\n  node_ip: 127.0.0.1\nkeys:\n  appkey: {LK_SECRET}\nlogging:\n  level: warn\n");
    let child = Command::new(bin)
        .arg("--config-body")
        .arg(config)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            std::time::Instant::now() < until,
            "livekit-server didn't start"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Some(TestLiveKit(child, port))
}

/// Keep both apps' connections and calls going until `cond` holds.
fn huddle_until(a: &mut App, b: &mut App, what: &str, cond: impl Fn(&App, &App) -> bool) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(25);
    while std::time::Instant::now() < until {
        for x in [&mut *a, &mut *b] {
            x.net_tick(None);
            x.huddle_tick(None);
        }
        if cond(a, b) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    panic!(
        "timed out waiting for: {} (A {:?} / B {:?})",
        what, a.huddle.people, b.huddle.people
    );
}

// WebRTC opens UDP on every network, so macOS asks about incoming connections
// for each freshly built test binary. Run it on purpose: cargo test -- --ignored
#[test]
#[ignore = "opens WebRTC sockets; run with --ignored"]
fn a_huddle_on_the_server_connects_two_computers() {
    let Some(lk) = test_livekit() else {
        eprintln!("skipped: livekit-server isn't installed");
        return;
    };
    let server = TestServer::start();
    // the server makes its folder when it starts serving; don't race it
    std::fs::create_dir_all(&server.home).unwrap();
    std::fs::write(server.home.join(workboard_server::LIVEKIT_FILE), serde_json::json!({"url": format!("ws://127.0.0.1:{}", lk.1), "key": "appkey", "secret": LK_SECRET}).to_string()).unwrap();
    let addr = format!("127.0.0.1:{}", server.port);

    // A sets the server up, Sam (member 3) joins with a code
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A on the server", |a| {
        a.data.extra.shared.is_some() && a.sync_rt.people.iter().any(|p| p.member == 3)
    });
    a.access_request(wb_net::Msg::CreateInvite {
        member: 3,
        role: wb_net::Role::Member,
    });
    net_until(&mut a, "code", |a| {
        a.sync_rt
            .people
            .iter()
            .any(|p| p.member == 3 && p.invite.is_some())
    });
    let code = a
        .sync_rt
        .people
        .iter()
        .find(|p| p.member == 3)
        .unwrap()
        .invite
        .clone()
        .unwrap();
    let mut b = App::build(
        None,
        Some(Data::blank(
            "Theirs",
            Member {
                id: 1,
                name: "B".into(),
                ..Default::default()
            },
        )),
    );
    b.connect_server(&addr, &code).unwrap();
    net_until(&mut b, "B joined", |b| b.data.extra.shared.is_some());
    b.sync_note_change();
    b.write_shared();
    let ch = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    net_until(&mut a, "Sam is in #general on the server", |a| {
        a.data
            .channels
            .iter()
            .any(|c| c.id == ch && c.members.contains(&3))
    });

    // A presses Huddle in #general: a huddle message goes out and A joins
    a.section = Section::Chat;
    a.chat.open = Some(ch);
    a.open_call_dialog();
    assert!(a.modal.is_none(), "no browser-call dialog on the server");
    assert_eq!(a.huddle.channel, Some(ch));
    let msg_is_huddle = |x: &App| {
        x.data
            .channels
            .iter()
            .find(|c| c.id == ch)
            .and_then(|c| c.messages.last())
            .and_then(|m| m.call.as_deref())
            .and_then(huddle::huddle_channel)
            == Some(ch)
    };
    // the card goes out once the server hands A the call token
    net_until(&mut a, "A's huddle card", msg_is_huddle);
    a.sync_note_change();
    a.write_shared();
    net_until(&mut b, "B sees the huddle message", |b| msg_is_huddle(b));

    // B presses Join on it; both hear each other (a tone in tests)
    b.apply_chat(vec![ChatAction::JoinHuddle(ch)]);
    huddle_until(&mut a, &mut b, "both in the huddle and heard", |a, b| {
        let heard = |x: &App| {
            x.huddle.people.len() == 2 && x.huddle.people.iter().any(|p| !p.me && p.heard > 0.02)
        };
        heard(a) && heard(b)
    });
    let sam = a.huddle.people.iter().find(|p| !p.me).unwrap();
    assert_eq!(sam.member(), Some(3), "A sees Sam");

    // A turns on video (a red test picture): B sees it, and A sees itself
    a.set_huddle_camera(true);
    let a_identity = b
        .huddle
        .people
        .iter()
        .find(|p| !p.me)
        .unwrap()
        .identity
        .clone();
    huddle_until(&mut a, &mut b, "B sees A's video", |a, b| {
        let red = |p: &wb_voice::Picture| p.average()[0] > 150 && p.average()[1] < 100;
        b.huddle
            .pictures
            .get(&a_identity)
            .is_some_and(|(p, _)| red(p))
            && a.huddle
                .pictures
                .get(wb_voice::ME)
                .is_some_and(|(p, _)| red(p))
    });
    a.set_huddle_camera(false);
    huddle_until(&mut a, &mut b, "A's video is gone at B", |a, b| {
        !b.huddle.pictures.contains_key(&a_identity)
            && !a.huddle.pictures.contains_key(wb_voice::ME)
    });

    // A shares a screen (a test picture stands in for it): B gets it as A's screen
    a.open_share_menu();
    let display = a.huddle.screen_sources[0].share();
    a.set_huddle_screen(Some(display));
    let a_screen = wb_voice::screen_key(&a_identity);
    huddle_until(&mut a, &mut b, "B sees A's screen", |a, b| {
        b.huddle.pictures.contains_key(&a_screen)
            && b.huddle.people.iter().any(|p| !p.me && p.screen)
            && a.huddle.people.iter().any(|p| p.me && p.screen)
    });
    a.set_huddle_screen(None);
    huddle_until(&mut a, &mut b, "A's screen is gone at B", |_, b| {
        !b.huddle.pictures.contains_key(&a_screen)
    });

    // B mutes, then leaves
    b.set_huddle_muted(true);
    huddle_until(&mut a, &mut b, "A sees B muted", |a, _| {
        a.huddle.people.iter().any(|p| !p.me && p.muted)
    });
    b.leave_huddle();
    assert!(b.huddle.channel.is_none() && b.huddle.call.is_none());
    huddle_until(&mut a, &mut b, "A alone", |a, _| a.huddle.people.len() == 1);
    a.stop_sharing();
    assert!(
        a.huddle.channel.is_none(),
        "stopping sharing ends the huddle"
    );
}

#[test]
fn without_a_server_the_huddle_button_still_opens_a_browser_call() {
    let mut a = app();
    let ch = a.data.channels[0].id;
    a.section = Section::Chat;
    a.chat.open = Some(ch);
    a.open_call_dialog();
    assert!(matches!(a.modal, Some(Modal::Call { .. })));
    assert!(a.huddle.channel.is_none());
    a.join_huddle(ch);
    assert!(
        a.huddle.channel.is_none(),
        "can't join an in-app huddle without the server"
    );
}

#[test]
fn a_huddle_join_gives_up_after_a_while_and_only_its_own_denial_cancels_it() {
    let mut a = app();
    let ch = a.data.channels[0].id;
    let other = a.data.channels.get(1).map(|c| c.id).unwrap_or(ch + 1);
    let waiting = |asked: std::time::Instant| {
        let mut h = huddle::HuddleRt::default();
        (h.channel, h.waiting, h.asked, h.announce) = (Some(ch), true, Some(asked), true);
        h
    };
    let cards = |a: &App| {
        a.data.channels[0]
            .messages
            .iter()
            .filter(|m| m.call.as_deref().and_then(huddle::huddle_channel) == Some(ch))
            .count()
    };

    // an unrelated refusal (a backup, an invite, another conversation) leaves it waiting
    a.huddle = waiting(std::time::Instant::now());
    a.huddle_denied(other);
    assert!(a.huddle.waiting && a.huddle.channel == Some(ch));
    // its own refusal ends it, and no "Started a huddle" card was posted
    a.huddle_denied(ch);
    assert!(a.huddle.channel.is_none());
    assert_eq!(cards(&a), 0);

    // no answer for HUDDLE_JOIN_SECS: give up and say so
    a.huddle = waiting(
        std::time::Instant::now() - std::time::Duration::from_secs(huddle::HUDDLE_JOIN_SECS + 1),
    );
    a.toast = None;
    a.huddle_tick(None);
    assert!(a.huddle.channel.is_none() && !a.huddle.waiting);
    assert!(
        a.toast
            .as_ref()
            .is_some_and(|(t, _)| t.contains("didn't answer")),
        "{:?}",
        a.toast
    );
    assert_eq!(cards(&a), 0);

    // the token came: now the card goes out, once
    a.huddle = waiting(std::time::Instant::now());
    a.huddle_token(ch, "ws://x".into(), "t".into());
    assert_eq!(cards(&a), 1);
    assert!(!a.huddle.waiting && a.huddle.token.is_some());
    a.huddle_token(ch, "ws://x".into(), "t".into());
    assert_eq!(cards(&a), 1);
}

fn test_msg(id: u64, author: u64, text: &str, ts: i64) -> Message {
    Message {
        mentions: vec![],
        id,
        author,
        text: text.into(),
        ts,
        reactions: vec![],
        replies: vec![],
        saved: false,
        task: None,
        post_title: None,
        post_type: String::new(),
        pinned: false,
        call: None,
        attachments: vec![],
        edited: None,
        imported: None,
    }
}

#[test]
fn private_channels_and_dms_show_only_to_their_members() {
    let mut a = app();
    let secret = a.create_channel("secret plans", true, false);
    let public = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    let dm_me_alex = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    let ch = |a: &App, id: u64| a.data.channels.iter().find(|c| c.id == id).unwrap().clone();
    assert!(
        a.can_see(&ch(&a, secret)),
        "the person who made it is in it"
    );
    // Sam (3) is in neither the private channel nor my DM with Alex
    a.data.me = 3;
    assert!(!a.can_see(&ch(&a, secret)));
    assert!(!a.can_see(&ch(&a, dm_me_alex)));
    assert!(
        a.can_see(&ch(&a, public)),
        "public channels stay open to all"
    );
    // added as a member, Sam sees it
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == secret)
        .unwrap()
        .members
        .push(3);
    assert!(a.can_see(&ch(&a, secret)));
}

#[test]
fn closing_a_dm_hides_it_for_me_until_a_new_message() {
    let mut a = app();
    let dm = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    let n = a.data.channels.len();
    a.chat.open = Some(dm);
    a.close_dm(dm);
    assert_eq!(a.data.channels.len(), n, "nothing is deleted");
    assert_eq!(a.chat.open, None);
    let listed = |a: &App| a.listed(a.data.channels.iter().find(|c| c.id == dm).unwrap());
    assert!(!listed(&a));
    // only this computer remembers it: it's not in what the team shares
    let v = serde_json::to_value(&a.data).unwrap();
    assert!(v["channels"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c.get("closed").is_none()));
    assert!(!crate::sync::SHARED.iter().any(|p| p.contains("closed_dms")));
    // Alex writes again: it's back
    let id = a.data.new_id();
    let c = a.data.channels.iter_mut().find(|c| c.id == dm).unwrap();
    c.messages
        .push(test_msg(id, 2, "still there?", now_ts() + 5));
    assert!(listed(&a));
    // opening it from New message also brings it back
    a.close_dm(dm);
    assert!(!listed(&a));
    a.apply_chat(vec![ChatAction::Dm(2)]);
    assert_eq!(a.chat.open, Some(dm));
}

#[test]
fn two_dms_with_the_same_people_show_as_one() {
    let mut a = app();
    let older = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 3])
        .unwrap()
        .id;
    let newer = a.data.new_id();
    a.data.channels.push(Channel {
        id: newer,
        name: String::new(),
        kind: ChannelKind::Direct,
        topic: String::new(),
        members: vec![3, 1],
        messages: vec![],
        last_read: now_ts(),
        ..Default::default()
    });
    let get = |a: &App, id: u64| a.data.channels.iter().find(|c| c.id == id).unwrap().clone();
    assert!(a.listed(&get(&a, older)));
    assert!(!a.listed(&get(&a, newer)), "the newer empty copy gives way");
    a.apply_chat(vec![ChatAction::Dm(3)]);
    assert_eq!(a.chat.open, Some(older));
    // a copy with messages wins over an empty one
    let id = a.data.new_id();
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == newer)
        .unwrap()
        .messages
        .push(test_msg(id, 3, "hi", now_ts()));
    assert!(!a.listed(&get(&a, older)));
    assert!(a.listed(&get(&a, newer)));
}

fn chat_msg(id: u64, author: u64, text: &str, ts: i64) -> Message {
    Message {
        mentions: vec![],
        id,
        author,
        text: text.into(),
        ts,
        reactions: vec![],
        replies: vec![],
        saved: false,
        task: None,
        post_title: None,
        post_type: String::new(),
        pinned: false,
        call: None,
        attachments: vec![],
        edited: None,
        imported: None,
    }
}

#[test]
fn inbox_primary_and_badge_follow_the_settings() {
    let mut a = app();
    a.data.tasks.clear();
    a.data.channels.clear();
    let list = list_ids(&a)[0];
    let mine = add_task(&mut a, list, "Mine");
    let theirs = add_task(&mut a, list, "Theirs");
    // someone else made "Theirs" and I'm not on it
    a.data
        .tasks
        .iter_mut()
        .find(|t| t.id == theirs)
        .unwrap()
        .activity
        .iter_mut()
        .for_each(|x| x.author = 2);
    let now = now_ts();
    let push = |a: &mut App, id: u64, ts: i64, author: u64, text: &str, comment: bool| {
        a.data
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .activity
            .push(Activity {
                ts,
                author,
                text: text.into(),
                comment,
                ..Default::default()
            });
    };
    push(&mut a, mine, now - 10, 2, "Looks good", true);
    push(&mut a, theirs, now - 9, 3, "Not for you", true);
    push(&mut a, theirs, now - 8, 3, "@Me can you check?", true);
    push(&mut a, theirs, now - 7, 3, "assigned to Me", false);
    push(&mut a, theirs, now - 6, 3, "assigned to Alex Kim", false);
    push(&mut a, mine, now - 5, 1, "my own note", true);
    let primary = |a: &App| {
        a.inbox_items()
            .iter()
            .cloned()
            .filter(|i| i.tab == 0)
            .map(|i| i.text)
            .collect::<Vec<_>>()
    };
    let mut p = primary(&a);
    p.sort();
    assert_eq!(
        p,
        vec!["@Me can you check?", "Looks good", "assigned to Me"]
    );
    // my own actions are nowhere; the rest is Other
    assert!(!a.inbox_items().iter().any(|i| i.text == "my own note"));
    assert!(a
        .inbox_items()
        .iter()
        .any(|i| i.text == "Not for you" && i.tab == 1));
    assert!(a
        .inbox_items()
        .iter()
        .any(|i| i.text == "assigned to Alex Kim" && i.tab == 1 && !i.assigned));
    assert_eq!(a.inbox_badge(), 3);
    // clearing and Later take rows off the badge
    let keys: Vec<String> = a
        .inbox_items()
        .iter()
        .cloned()
        .filter(|i| i.tab == 0)
        .map(|i| i.key)
        .collect();
    a.data.extra.inbox_cleared.push(keys[0].clone());
    a.data.extra.inbox_later.push(keys[1].clone());
    assert_eq!(a.inbox_badge(), 1);
    // turning comments off takes "Looks good" out of Primary
    a.data.extra.inbox_cleared.clear();
    a.data.extra.inbox_later.clear();
    a.data.extra.notify.comments = false;
    assert!(!primary(&a).contains(&"Looks good".to_string()));
    a.data.extra.notify.assigned = false;
    a.data.extra.notify.mentions = false;
    assert_eq!(a.inbox_badge(), 0);
}

#[test]
fn chat_mentions_and_thread_replies_reach_the_inbox() {
    let mut a = app();
    a.data.tasks.clear();
    let now = now_ts();
    let cid = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel)
        .unwrap()
        .id;
    let c = a.data.channels.iter_mut().find(|c| c.id == cid).unwrap();
    c.messages.clear();
    let mut root = chat_msg(900, 1, "Plan for Friday?", now - 100);
    root.replies.push(chat_msg(901, 2, "Sounds good", now - 50));
    c.messages.push(root);
    c.messages
        .push(chat_msg(902, 3, "@Me please look", now - 40));
    c.messages
        .push(chat_msg(903, 3, "nothing for me", now - 30));
    let items = a.inbox_items();
    assert!(items
        .iter()
        .any(|i| i.text == "Sounds good" && i.reply && i.tab == 0));
    assert!(items
        .iter()
        .any(|i| i.text == "@Me please look" && i.mention && i.tab == 0));
    assert!(!items.iter().any(|i| i.text == "nothing for me"));
    assert_eq!(a.inbox_badge(), 2);
    assert_eq!(a.chat_replies().len(), 1);
    // opening the reply shows its thread
    let src = items.iter().find(|i| i.reply).unwrap().src;
    a.open_inbox_src(src);
    assert_eq!(
        (a.section, a.chat.open, a.chat.thread_open),
        (Section::Chat, Some(cid), Some(900))
    );
}

#[test]
fn reminders_alert_once_when_due() {
    let mut a = app();
    let me = a.data.me;
    let list = list_ids(&a)[0];
    let tid = add_task(&mut a, list, "Call back");
    a.data.extra.reminders.push(Reminder {
        id: 50,
        text: "Call back".into(),
        due_ts: now_ts() - 1,
        owner: me,
        task: Some(tid),
        ..Default::default()
    });
    a.data.extra.reminders.push(Reminder {
        id: 51,
        text: "Later on".into(),
        due_ts: now_ts() + 3600,
        owner: me,
        ..Default::default()
    });
    a.toast = None;
    a.reminder_tick(None);
    assert!(a
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("Call back")));
    assert!(
        a.data
            .extra
            .reminders
            .iter()
            .find(|r| r.id == 50)
            .unwrap()
            .alerted
    );
    assert!(
        !a.data
            .extra
            .reminders
            .iter()
            .find(|r| r.id == 51)
            .unwrap()
            .alerted
    );
    // only once
    a.toast = None;
    a.reminder_tick(None);
    assert!(a.toast.is_none());
    // the reminder opens its task
    let r = a
        .data
        .extra
        .reminders
        .iter()
        .find(|r| r.id == 50)
        .unwrap()
        .clone();
    a.open_reminder(&r);
    a.apply_actions();
    assert_eq!(a.open_task, Some(tid));
    // with "A reminder is due" off there is no notice
    a.data.extra.notify.reminders = false;
    a.data.extra.reminders.push(Reminder {
        id: 52,
        text: "Quiet".into(),
        due_ts: now_ts() - 1,
        owner: me,
        ..Default::default()
    });
    a.reminder_tick(None);
    assert!(a.toast.is_none());
}

/// The sidebar and the Dock badge ask for the Inbox rows on every frame:
/// they're built once and kept until the workspace changes, and a change
/// (made here, merged in from the team, or one a count gives away) builds
/// them again.
#[test]
fn inbox_rows_are_kept_between_frames_until_the_workspace_changes() {
    let mut a = app();
    let ctx = egui::Context::default();
    let frame = |a: &mut App| {
        let _ = ctx.run(egui::RawInput::default(), |ctx| a.ui_frame(ctx));
    };
    for _ in 0..3 {
        frame(&mut a);
    }
    let kept = a.inbox_items();
    frame(&mut a);
    frame(&mut a);
    assert!(
        std::rc::Rc::ptr_eq(&kept, &a.inbox_items()),
        "idle frames don't build them again"
    );
    let badge = a.inbox_badge();

    // a new message that mentions me
    let g = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel && !c.private)
        .unwrap()
        .id;
    let id = a.data.new_id();
    let ch = a.data.channels.iter_mut().find(|c| c.id == g).unwrap();
    ch.messages.push(chat_msg(id, 2, "hello all", now_ts()));
    assert_eq!(a.inbox_badge(), badge, "not a mention");

    // the same message edited into one (no count changes): an edit made
    // here leaves the workspace changed for the next frame
    let ch = a.data.channels.iter_mut().find(|c| c.id == g).unwrap();
    ch.messages.last_mut().unwrap().text = "@Me look at this".into();
    a.dirty = true;
    assert_eq!(a.inbox_badge(), badge + 1, "a change not taken in yet");
    frame(&mut a);
    assert_eq!(a.inbox_badge(), badge + 1);
    let kept = a.inbox_items();
    frame(&mut a);
    assert!(std::rc::Rc::ptr_eq(&kept, &a.inbox_items()));

    // and so does a change merged in from the team
    let ch = a.data.channels.iter_mut().find(|c| c.id == g).unwrap();
    ch.messages.last_mut().unwrap().text = "never mind".into();
    a.data_changed();
    assert_eq!(a.inbox_badge(), badge);
}

#[test]
fn mentions_in_private_channels_i_am_not_in_stay_out() {
    let mut a = app();
    a.data.tasks.clear();
    let now = now_ts();
    for c in &mut a.data.channels {
        c.messages.clear();
    }
    let c = a
        .data
        .channels
        .iter_mut()
        .find(|c| c.kind == ChannelKind::Channel)
        .unwrap();
    c.private = true;
    c.members = vec![2, 3];
    c.messages.push(chat_msg(950, 3, "@Me secret", now - 10));
    assert!(a.inbox_items().is_empty());
    a.data
        .channels
        .iter_mut()
        .find(|c| c.private)
        .unwrap()
        .members
        .push(1);
    assert_eq!(a.inbox_badge(), 1);
}

#[test]
fn an_older_servers_unnamed_refusal_ends_a_waiting_huddle() {
    let mut a = app();
    let ch = a.data.channels[0].id;
    let now = std::time::Instant::now();
    let waiting = |asked: std::time::Instant| {
        let mut h = huddle::HuddleRt::default();
        (h.channel, h.waiting, h.asked) = (Some(ch), true, Some(asked));
        h
    };
    // only the call_join went out: its refusal ends the huddle at once
    a.huddle = waiting(now);
    a.sync_rt.other_asked = None;
    a.huddle_denied_unnamed();
    assert!(a.huddle.channel.is_none() && !a.huddle.waiting);
    // another request went just before it: the refusal may be that one's, keep waiting
    a.huddle = waiting(now);
    a.sync_rt.other_asked = Some(now - std::time::Duration::from_secs(1));
    a.huddle_denied_unnamed();
    assert!(a.huddle.waiting && a.huddle.channel == Some(ch));
    // one sent after the call_join is answered after it, and one long before is done
    a.sync_rt.other_asked = Some(now + std::time::Duration::from_millis(5));
    a.huddle_denied_unnamed();
    assert!(a.huddle.channel.is_none());
    a.huddle = waiting(now);
    a.sync_rt.other_asked =
        Some(now - std::time::Duration::from_secs(huddle::HUDDLE_JOIN_SECS + 1));
    a.huddle_denied_unnamed();
    assert!(a.huddle.channel.is_none());
    // not waiting for a huddle: nothing to end
    a.huddle = huddle::HuddleRt::default();
    a.huddle_denied_unnamed();
    assert!(a.huddle.channel.is_none());
}

#[test]
fn a_late_message_or_one_from_a_clock_behind_counts_as_unread() {
    let mut a = app();
    let cid = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    a.mark_read(cid);
    let get = |a: &App| {
        a.data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .clone()
    };
    assert_eq!(a.unread(&get(&a)), 0);
    // written an hour ago by its author's clock, arriving only now
    let id = a.data.new_id();
    let behind = get(&a).last_read - 3600;
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .push(test_msg(id, 2, "late", behind));
    assert_eq!(
        a.unread(&get(&a)),
        1,
        "a message not seen yet is new whatever its time"
    );
    a.mark_read(cid);
    assert_eq!(a.unread(&get(&a)), 0);
    // "Mark unread" from a message on brings it and the later ones back
    let ts = get(&a)
        .messages
        .iter()
        .filter(|m| m.author != 1)
        .map(|m| m.ts)
        .min()
        .unwrap();
    a.apply_chat(vec![ChatAction::MarkUnread { ch: cid, ts }]);
    let others = get(&a)
        .messages
        .iter()
        .filter(|m| m.author != 1 && m.ts >= ts)
        .count();
    assert_eq!(a.unread(&get(&a)), others);
}

#[test]
fn a_closed_dm_comes_back_for_a_new_message_from_a_clock_behind() {
    let mut a = app();
    let dm = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    let get = |a: &App| a.data.channels.iter().find(|c| c.id == dm).unwrap().clone();
    a.close_dm(dm);
    assert!(!a.listed(&get(&a)));
    let id = a.data.new_id();
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == dm)
        .unwrap()
        .messages
        .push(test_msg(id, 2, "earlier by my clock", now_ts() - 86_400));
    assert!(
        a.listed(&get(&a)),
        "a message that wasn't there when it was closed brings it back"
    );
}

#[test]
fn two_dms_that_both_have_messages_show_as_one_conversation() {
    let mut a = app();
    let older = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    let newer = a.data.new_id();
    let mid = a.data.new_id();
    a.data.channels.push(Channel {
        id: newer,
        kind: ChannelKind::Direct,
        members: vec![2, 1],
        messages: vec![test_msg(mid, 2, "in the other copy", now_ts())],
        ..Default::default()
    });
    let get = |a: &App, id: u64| a.data.channels.iter().find(|c| c.id == id).unwrap().clone();
    assert!(a.listed(&get(&a, older)));
    assert!(!a.listed(&get(&a, newer)));
    assert_eq!(a.conversation(&get(&a, older)).len(), 2);
    assert!(
        a.unread(&get(&a, older)) >= 1,
        "the other copy's messages count in the one shown"
    );
    // acting on a message from the other copy changes it where it is
    a.apply_chat(vec![ChatAction::React {
        ch: older,
        msg: mid,
        emoji: "\u{1F44D}".into(),
    }]);
    assert_eq!(get(&a, newer).messages[0].reactions.len(), 1);
    // new messages go to the older one; reading it reads both
    a.apply_chat(vec![ChatAction::Send {
        ch: older,
        text: "hello".into(),
        parent: None,
        files: vec![],
    }]);
    assert!(get(&a, older).messages.iter().any(|m| m.text == "hello"));
    a.mark_read(older);
    assert_eq!(a.unread(&get(&a, older)), 0);
    assert!(a
        .data
        .extra
        .chat_seen
        .get(&newer)
        .is_some_and(|s| s.has(mid)));
}

#[test]
fn only_a_channels_maker_or_an_admin_may_delete_it() {
    let mut a = app();
    a.apply_chat(vec![ChatAction::NewChannel("mine".into())]);
    let c = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "mine")
        .unwrap()
        .clone();
    assert_eq!(c.creator, a.data.me);
    assert!(a.may_manage_channel(&c));
    let mut theirs = c.clone();
    theirs.creator = 2;
    // alone or in a shared folder, the role in People decides
    let me = a.data.me;
    assert!(a.may_manage_channel(&theirs), "the owner may");
    a.data.members.iter_mut().find(|m| m.id == me).unwrap().role = "Admin".into();
    assert!(a.may_manage_channel(&theirs), "an admin may");
    a.data.members.iter_mut().find(|m| m.id == me).unwrap().role = "Member".into();
    assert!(
        !a.may_manage_channel(&theirs),
        "a member who didn't make it"
    );
    // on a server, the server's role decides (People can't be trusted there)
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    a.data.members.iter_mut().find(|m| m.id == me).unwrap().role = "Owner".into();
    assert!(
        !a.may_manage_channel(&theirs),
        "a role in People alone isn't enough"
    );
    a.sync_rt.role = Some(wb_net::Role::Admin);
    assert!(a.may_manage_channel(&theirs), "the server's admins may");
    a.sync_rt.role = Some(wb_net::Role::Member);
    let mut old = theirs.clone();
    old.creator = 0;
    assert!(
        a.may_manage_channel(&old),
        "from before makers were kept: any member"
    );
    old.members.retain(|m| *m != a.data.me);
    assert!(!a.may_manage_channel(&old));
    // the maker isn't saved when there is none, so older copies stay the same
    assert!(!serde_json::to_string(&old).unwrap().contains("creator"));
}

#[test]
fn a_mention_needs_the_whole_name() {
    let a = app();
    let ids = |t: &str| crate::model::mentioned_ids(t, &a.data.members, &[]);
    let alex = a
        .data
        .members
        .iter()
        .find(|m| m.name == "Alex Kim")
        .unwrap()
        .id;
    assert_eq!(ids("hi @Alex Kim!"), vec![alex]);
    assert_eq!(ids("(cc @Alex Kim)"), vec![alex]);
    assert!(ids("hi @Alex Kimberly").is_empty());
    assert!(ids("mail@Alex Kim").is_empty());
}

#[test]
fn comments_on_a_task_i_duplicated_go_to_primary() {
    let mut a = app();
    let me = a.data.me;
    let lists = list_ids(&a);
    let src = add_task(&mut a, lists[0], "Original");
    let copy = a.duplicate_task(src).unwrap();
    let other = a
        .data
        .members
        .iter()
        .map(|m| m.id)
        .find(|id| *id != me)
        .unwrap();
    let t = a.data.tasks.iter_mut().find(|t| t.id == copy).unwrap();
    t.assignees.clear();
    t.activity.push(Activity {
        ts: now_ts() + 5,
        author: other,
        text: "looks good".into(),
        comment: true,
        ..Default::default()
    });
    let row = a
        .inbox_items()
        .iter()
        .cloned()
        .find(|i| i.src == crate::app::nav::InboxSrc::Task(copy) && i.text == "looks good")
        .unwrap();
    assert_eq!(row.tab, 0);
}

/// Runs `f` as another member, then switches back.
fn as_member<R>(a: &mut App, who: u64, f: impl FnOnce(&mut App) -> R) -> R {
    let me = a.data.me;
    a.data.me = who;
    let r = f(a);
    a.data.me = me;
    r
}

/// The tasks with an "assigned" row in Primary for the person using the app.
fn assigned_in_primary(a: &App) -> Vec<u64> {
    a.inbox_items()
        .iter()
        .cloned()
        .filter(|i| i.assigned && i.tab == 0)
        .filter_map(|i| match i.src {
            nav::InboxSrc::Task(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn mentioned_in_primary(a: &App) -> Vec<String> {
    a.inbox_items()
        .iter()
        .cloned()
        .filter(|i| i.mention && i.tab == 0)
        .map(|i| i.text)
        .collect()
}

fn quiet_app() -> App {
    let mut a = app();
    a.data.tasks.clear();
    for c in &mut a.data.channels {
        c.messages.clear();
    }
    a
}

#[test]
fn tasks_made_or_copied_with_me_on_them_reach_my_inbox() {
    let mut a = quiet_app();
    let list = list_ids(&a)[0];
    // Alex Kim (2) makes a task with me (1) on it, and copies another one
    let made = as_member(&mut a, 2, |a| {
        a.apply(Action::Add {
            list,
            name: "Made for you".into(),
            status: None,
            priority: 0,
            assignees: vec![1, 2],
            due: None,
            parent: None,
            open: false,
        });
        a.data
            .tasks
            .iter()
            .find(|t| t.name == "Made for you")
            .unwrap()
            .id
    });
    let copy = as_member(&mut a, 2, |a| a.duplicate_task(made).unwrap());
    let rows = assigned_in_primary(&a);
    assert!(
        rows.contains(&made),
        "a task made with me on it: {:?}",
        rows
    );
    assert!(
        rows.contains(&copy),
        "a copy of a task with me on it: {:?}",
        rows
    );
    // the maker putting themselves on it says nothing to them
    as_member(&mut a, 2, |a| assert!(assigned_in_primary(a).is_empty()));
    // a task I make for myself is not news to me
    a.apply(Action::Add {
        list,
        name: "Mine".into(),
        status: None,
        priority: 0,
        assignees: vec![1],
        due: None,
        parent: None,
        open: false,
    });
    assert_eq!(assigned_in_primary(&a).len(), 2);
}

#[test]
fn automation_assignments_reach_the_inbox() {
    let mut a = quiet_app();
    let list = list_ids(&a)[0];
    a.data.extra.automations.push(Automation {
        id: 1,
        enabled: true,
        trigger_status: "complete".into(),
        action: "assign".into(),
        value: "1".into(),
        list: None,
    });
    let id = add_task(&mut a, list, "Finish me");
    a.data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .activity
        .iter_mut()
        .for_each(|x| x.author = 2);
    as_member(&mut a, 2, |a| {
        let mut t = a.task(id).unwrap().clone();
        t.status = "complete".into();
        a.apply(Action::Replace(t));
    });
    assert!(a.task(id).unwrap().assignees.contains(&1));
    assert_eq!(assigned_in_primary(&a), vec![id]);
    // the automation's own line stays as it was
    assert!(a
        .task(id)
        .unwrap()
        .activity
        .iter()
        .any(|x| x.text.starts_with("Automation ran: ")));
}

#[test]
fn a_mention_of_alex_kim_is_not_a_mention_of_alex() {
    let mut a = quiet_app();
    a.data.members.push(Member {
        id: 5,
        name: "Alex".into(),
        ..Default::default()
    });
    let cid = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel)
        .unwrap()
        .id;
    // sent from the app, and one from a file written before ids were kept
    as_member(&mut a, 3, |a| {
        a.apply_chat(vec![ChatAction::Send {
            ch: cid,
            text: "@Alex Kim new one".into(),
            parent: None,
            files: vec![],
        }])
    });
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .push(chat_msg(960, 3, "@Alex Kim old one", now_ts() - 5));
    let list = list_ids(&a)[0];
    let tid = add_task(&mut a, list, "Talk");
    a.data
        .tasks
        .iter_mut()
        .find(|t| t.id == tid)
        .unwrap()
        .activity
        .push(Activity {
            ts: now_ts() - 4,
            author: 3,
            text: "@Alex Kim on a task".into(),
            comment: true,
            ..Default::default()
        });
    as_member(&mut a, 5, |a| {
        assert!(
            mentioned_in_primary(a).is_empty(),
            "Alex: {:?}",
            mentioned_in_primary(a)
        )
    });
    as_member(&mut a, 2, |a| {
        let mut got = mentioned_in_primary(a);
        got.sort();
        assert_eq!(
            got,
            vec![
                "@Alex Kim new one",
                "@Alex Kim old one",
                "@Alex Kim on a task"
            ]
        );
    });
    // "@Alex" alone still reaches Alex, and a Korean ending may follow a name
    as_member(&mut a, 3, |a| {
        a.apply_chat(vec![ChatAction::Send {
            ch: cid,
            text: "@Alex, and @Alex님".into(),
            parent: None,
            files: vec![],
        }])
    });
    as_member(&mut a, 5, |a| {
        assert_eq!(mentioned_in_primary(a), vec!["@Alex, and @Alex님"])
    });
    as_member(&mut a, 2, |a| assert_eq!(mentioned_in_primary(a).len(), 3));
}

#[test]
fn same_names_and_renames_do_not_mix_people_up() {
    let mut a = quiet_app();
    a.data.members.push(Member {
        id: 6,
        name: "Sam Lee".into(),
        ..Default::default()
    });
    let list = list_ids(&a)[0];
    let tid = add_task(&mut a, list, "Shared name");
    // Alex Kim puts the second Sam Lee (6) on the task
    as_member(&mut a, 2, |a| {
        let mut t = a.task(tid).unwrap().clone();
        t.assignees.push(6);
        a.apply(Action::Replace(t));
    });
    as_member(&mut a, 6, |a| assert_eq!(assigned_in_primary(a), vec![tid]));
    as_member(&mut a, 3, |a| {
        assert!(assigned_in_primary(a).is_empty(), "the other Sam Lee")
    });
    // Jordan Park (4) is assigned and mentioned, then renamed
    let cid = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel)
        .unwrap()
        .id;
    let tid2 = add_task(&mut a, list, "For Jordan");
    as_member(&mut a, 2, |a| {
        let mut t = a.task(tid2).unwrap().clone();
        t.assignees.push(4);
        a.apply(Action::Replace(t));
        a.apply_chat(vec![ChatAction::Send {
            ch: cid,
            text: "@Jordan Park please look".into(),
            parent: None,
            files: vec![],
        }]);
    });
    a.data.members.iter_mut().find(|m| m.id == 4).unwrap().name = "Jordan P".into();
    as_member(&mut a, 4, |a| {
        assert_eq!(assigned_in_primary(a), vec![tid2]);
        assert_eq!(mentioned_in_primary(a), vec!["@Jordan Park please look"]);
    });
}

#[test]
fn mentioned_ids_take_the_longest_name_and_honour_picks() {
    let m = |id: u64, name: &str| Member {
        id,
        name: name.into(),
        ..Default::default()
    };
    let people = vec![
        m(1, "Alex"),
        m(2, "Alex Kim"),
        m(3, "지원"),
        m(4, "Sam"),
        m(5, "Sam"),
    ];
    assert_eq!(mentioned_ids("@Alex Kim hi", &people, &[]), vec![2]);
    assert_eq!(mentioned_ids("@Alex hi", &people, &[]), vec![1]);
    assert!(
        mentioned_ids("@Alexander hi", &people, &[]).is_empty(),
        "a longer word is someone else"
    );
    assert!(
        mentioned_ids("mail@Alex", &people, &[]).is_empty(),
        "@ inside a word"
    );
    assert_eq!(mentioned_ids("@지원님 확인 부탁", &people, &[]), vec![3]);
    assert_eq!(
        mentioned_ids("@Sam", &people, &[]),
        vec![4, 5],
        "a shared name with no pick"
    );
    assert_eq!(
        mentioned_ids("@Sam", &people, &[5]),
        vec![5],
        "the picked one"
    );
    assert_eq!(
        mentioned_ids("@Alex and @Alex Kim and @Alex", &people, &[]),
        vec![1, 2]
    );
}
