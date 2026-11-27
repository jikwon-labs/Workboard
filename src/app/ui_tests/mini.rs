// Click-through tests for the small window above other apps (mini.rs): one
// conversation with its newest messages and a box to write in, and the
// huddle's people and buttons when the huddle is in that conversation.

use super::*;

/// A message from someone else in channel `ch`, now.
fn message_from(h: &mut Ui1, ch: u64, author: u64, text: &str) -> u64 {
    let id = h.app.data.new_id();
    let c = h.app.data.channels.iter_mut().find(|c| c.id == ch).unwrap();
    c.messages.push(Message {
        goal: None,
        event: None,
        poll: None,
        votes: Default::default(),
        edited: None,
        imported: None,
        preview: None,
        forwarded: None,
        thread_of: None,
        bot: None,
        mentions: vec![],
        attachments: vec![],
        id,
        author,
        text: text.into(),
        ts: now_ts(),
        reactions: vec![],
        replies: vec![],
        saved: false,
        task: None,
        post_title: None,
        post_type: String::new(),
        pinned: false,
        call: None,
    });
    id
}

fn someone_else(h: &Ui1) -> Member {
    let me = h.app.data.me;
    h.app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone()
}

/// The workspace on a (made-up) team server, so huddles are in the app.
fn on_server(h: &mut Ui1) {
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
}

fn person(id: u64, dev: &str, me: bool, speaking: bool) -> wb_voice::Person {
    wb_voice::Person {
        identity: format!("{}-{}", id, dev),
        name: String::new(),
        me,
        speaking,
        muted: false,
        heard: 0.0,
        camera: false,
        screen: false,
    }
}

#[test]
fn small_window_opens_from_a_conversation_and_sends_from_its_own_box() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    // something half written in the main window's box stays there
    h.click("Message composer");
    h.type_text("still in the main box");
    assert!(!h.has("Small window message box"));

    h.click("Open in a small window");
    assert_eq!(h.app.mini.channel, Some(cid));
    assert!(h.has("Small window message box"));
    assert!(h.has("Close small window") && h.has("Open in the main window"));
    // it shows the conversation's newest message
    let newest = {
        let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
        c.messages
            .iter()
            .max_by_key(|m| (m.ts, m.id))
            .unwrap()
            .text
            .clone()
    };
    assert!(
        h.texts().iter().filter(|t| **t == newest).count() >= 2,
        "{:?} in the conversation and in the small window",
        newest
    );

    // Enter sends what is in its box, to its conversation
    h.click("Small window message box");
    h.type_text("from the small window");
    h.key(egui::Key::Enter);
    let last = |h: &Ui1| {
        let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
        c.messages.last().unwrap().clone()
    };
    assert_eq!(last(&h).text, "from the small window");
    assert_eq!(last(&h).author, h.app.data.me);
    assert!(h.app.mini.text.is_empty(), "its box is empty again");
    assert_eq!(
        h.app.chat.channel.text, "still in the main box",
        "the main window's box is left as it was"
    );

    // Shift+Enter starts a new line instead
    let count = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    h.type_text("two");
    h.key_with(egui::Key::Enter, egui::Modifiers::SHIFT);
    h.type_text("lines");
    assert_eq!(h.app.mini.text, "two\nlines");
    let after = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    assert_eq!(after, count, "nothing sent");
    // and the send button sends it
    h.click("Send from the small window");
    assert_eq!(last(&h).text, "two\nlines");
    assert!(h.app.mini.text.is_empty());
    // an empty box sends nothing
    h.click("Small window message box");
    h.type_text("   ");
    h.key(egui::Key::Enter);
    assert_eq!(last(&h).text, "two\nlines");

    // back to the main window: the small window closes, the conversation opens
    h.click("Rail Home");
    assert_ne!(h.app.section, Section::Chat);
    h.click("Open in the main window");
    assert_eq!(h.app.mini.channel, None);
    assert!(!h.has("Small window message box"));
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat.open, Some(cid));

    // its close button closes it; what was written waits for it on the
    // same conversation, not on another
    h.click("Open in a small window");
    h.click("Small window message box");
    h.type_text("not sent yet");
    h.click("Close small window");
    assert_eq!(h.app.mini.channel, None);
    assert!(!h.has("Small window message box"));
    h.click("Open in a small window");
    assert_eq!(h.app.mini.text, "not sent yet");
    h.click("Close small window");
    let other = h.app.data.channels.iter().find(|c| c.id != cid).unwrap().id;
    h.app.open_mini(other);
    assert!(h.app.mini.text.is_empty());
}

#[test]
fn small_window_keeps_an_unknown_command_in_its_box() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let count = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    h.click("Open in a small window");
    h.click("Small window message box");
    h.type_text("/nosuchthing hello");
    h.key(egui::Key::Enter);
    assert_eq!(h.app.mini.text, "/nosuchthing hello", "back in its box");
    assert!(h.app.chat.channel.text.is_empty(), "not in the main box");
    let after = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    assert_eq!(after, count);
    // a command it knows works from it
    h.app.mini.text.clear();
    h.type_text("/remind stretch in 30 minutes");
    h.key(egui::Key::Enter);
    assert!(h.app.mini.text.is_empty());
    assert!(h
        .app
        .data
        .extra
        .reminders
        .iter()
        .any(|r| r.text.contains("stretch")));
}

#[test]
fn small_window_takes_the_huddle_from_the_main_window_and_gives_it_back() {
    let mut h = Ui1::new();
    let ch = h.app.data.channels[0].id;
    let me = h.app.data.me;
    let other = someone_else(&h);
    let my_name = h.app.data.member(me).unwrap().name.clone();
    on_server(&mut h);
    h.app.huddle.channel = Some(ch);
    h.app.huddle.joined = true;
    h.app.huddle.people = vec![
        person(me, "dev1", true, false),
        person(other.id, "dev2", false, true),
    ];
    h.settle();
    let speaking = format!("{}, speaking", other.name);
    assert!(h.has("Huddle in a small window") && h.has(&speaking));

    h.click("Huddle in a small window");
    assert_eq!(h.app.mini.channel, Some(ch));
    assert!(h.app.mini_has_huddle());
    // the bar leaves the main window: one set of the huddle's buttons, in
    // the small window
    assert!(
        !h.has("Huddle in a small window"),
        "no bar in the main window"
    );
    let count = |h: &Ui1, name: &str| h.tree.iter().filter(|(n, _)| n == name).count();
    assert_eq!(count(&h, &speaking), 1);
    assert_eq!(count(&h, "Mute"), 1);
    assert!(h.has(&format!("{} (you), listening", my_name)));
    assert!(h.has("2 people"));
    let mini = h.rect("Small window message box");
    assert!(
        h.rect("Mute").top() < mini.top(),
        "the huddle's buttons are in the small window, above its box"
    );

    // its buttons work there
    h.click("Mute");
    assert!(h.app.huddle.muted);
    h.click("Unmute");
    assert!(!h.app.huddle.muted);
    h.click("Video");
    assert!(h.app.huddle.camera);
    h.click("Stop video");
    assert!(!h.app.huddle.camera);
    h.click("Share screen");
    assert!(h.app.huddle.share_menu);
    assert!(h.has("Share Test display"));
    h.click("Share Test display");
    assert_eq!(h.app.huddle.screen, Some(wb_voice::Share::Display(1)));
    assert!(h.has("Sharing your screen"));
    h.click("Stop sharing screen");
    assert!(h.app.huddle.screen.is_none());

    // someone's video shows in it, small, and the call is told its size
    let pic = wb_voice::Picture {
        width: 64,
        height: 36,
        rgba: vec![150u8; 64 * 36 * 4],
    };
    let them = format!("{}-dev2", other.id);
    h.app
        .huddle
        .pictures
        .insert(them.clone(), (std::sync::Arc::new(pic.clone()), true));
    h.settle();
    let video = format!("Video of {}", other.name);
    assert_eq!(count(&h, &video), 1, "one picture, in the small window");
    assert!(h.rect(&video).width() < h.rect("mini_window").width() / 2.0);
    assert!(h.app.huddle.views.contains_key(&them));
    // a shared screen as wide as the window at most
    let screen = wb_voice::screen_key(&them);
    h.app
        .huddle
        .pictures
        .insert(screen.clone(), (std::sync::Arc::new(pic), true));
    h.settle();
    let shared = format!("Screen of {}", other.name);
    assert!(h.has(&shared));
    assert!(h.rect(&shared).width() <= h.rect("mini_window").width());
    assert!(h.app.huddle.views.contains_key(&screen));
    // the buttons, the messages and the box still show under a tall screen
    let win = h.rect("mini_window");
    for b in [
        "Mute",
        "Video",
        "Share screen",
        "Leave",
        "Small window message box",
    ] {
        assert!(win.contains_rect(h.rect(b)), "{} inside {:?}", b, win);
    }
    assert!(h.rect("Leave").bottom() < h.rect("Small window message box").top());

    // closed: the huddle goes on, back in the main window's bar
    h.click("Close small window");
    assert_eq!(h.app.mini.channel, None);
    assert_eq!(h.app.huddle.channel, Some(ch));
    assert!(h.has("Huddle in a small window"));
    assert_eq!(count(&h, "Mute"), 1);
    assert!(h.has(&shared), "the screen shows in the main window again");

    // leaving from the small window: it stays, with the conversation
    h.click("Huddle in a small window");
    h.click("Leave");
    assert!(h.app.huddle.channel.is_none());
    assert_eq!(h.app.mini.channel, Some(ch));
    assert!(h.has("Small window message box"));
    assert!(!h.has("Mute") && !h.has("Huddle in a small window"));
}

#[test]
fn a_huddle_elsewhere_stays_in_the_main_window() {
    let mut h = Ui1::new();
    let mut random = h.app.data.channels[0].clone();
    random.id = 7_777;
    random.name = "random".into();
    random.messages.clear();
    h.app.data.channels.push(random);
    let general = h.app.data.channels[0].id;
    let me = h.app.data.me;
    on_server(&mut h);
    h.app.open_mini(7_777);
    h.app.huddle.channel = Some(general);
    h.app.huddle.joined = true;
    h.app.huddle.people = vec![person(me, "dev1", true, false)];
    h.settle();
    assert!(!h.app.mini_has_huddle());
    assert!(h.has("Huddle in a small window"), "the bar stays");
    assert!(h.has("No messages yet"), "the small window shows #random");
    // the bar's button moves the small window to the huddle's conversation
    // (here the small window is drawn over the main window and covers the
    // card's top right corner; in the app it is a window of its own)
    h.accessibility_click("Huddle in a small window");
    assert_eq!(h.app.mini.channel, Some(general));
    assert!(!h.has("Huddle in a small window"));
}

#[test]
fn small_window_closes_when_its_conversation_goes_out_of_reach() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let mut secret = h.app.data.channels[0].clone();
    secret.id = 8_888;
    secret.name = "secret".into();
    secret.private = true;
    secret.members = vec![me];
    h.app.data.channels.push(secret);
    h.app.open_mini(8_888);
    h.settle();
    assert!(h.has("Small window message box"));
    // taken out of the private channel
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == 8_888)
        .unwrap()
        .members
        .clear();
    h.settle();
    assert_eq!(h.app.mini.channel, None);
    assert!(!h.has("Small window message box"));
    // a conversation deleted
    let general = h.app.data.channels[0].id;
    h.app.open_mini(general);
    h.settle();
    assert!(h.has("Small window message box"));
    h.app.data.channels.retain(|c| c.id != general);
    h.settle();
    assert_eq!(h.app.mini.channel, None);
}

#[test]
fn small_window_reads_what_comes_in_unless_away_or_marked_unread() {
    let mut h = Ui1::new();
    let general = h.app.data.channels[0].id;
    let other = someone_else(&h);
    // the main window on another page, the small window on #general
    h.click("Rail Home");
    h.app.open_mini(general);
    h.settle();
    let unread = |h: &Ui1| {
        let c = h
            .app
            .data
            .channels
            .iter()
            .find(|c| c.id == general)
            .unwrap();
        h.app.unread(c)
    };
    message_from(&mut h, general, other.id, "seen in the small window");
    h.settle();
    assert_eq!(unread(&h), 0, "read while the app is in front");
    assert!(h.has("seen in the small window"));

    // the app behind another one (and the small window not in front): it
    // waits, and counts
    h.focused = false;
    h.settle();
    assert!(h.app.notify_rt.away);
    message_from(&mut h, general, other.id, "while away");
    h.settle();
    assert_eq!(unread(&h), 1);
    h.focused = true;
    h.settle();
    assert_eq!(unread(&h), 0);

    // marked unread in the main window: it stays so
    h.app.chat.keep_unread = Some(general);
    message_from(&mut h, general, other.id, "kept unread");
    h.settle();
    assert_eq!(unread(&h), 1);
}

#[test]
fn signing_out_closes_the_small_window() {
    let mut h = Ui1::new();
    let general = h.app.data.channels[0].id;
    h.app.open_mini(general);
    h.app.mini.text = "half".into();
    h.settle();
    assert!(h.has("Small window message box"));
    h.app.close_mini();
    h.app.open_mini(general);
    assert_eq!(h.app.mini.text, "half", "kept for the same conversation");
    h.settle();
    // logging out of the team server closes it
    on_server(&mut h);
    h.app.log_out();
    assert_eq!(h.app.mini.channel, None);
    assert!(h.app.mini.text.is_empty(), "nothing written is kept");
    h.settle();
    assert!(!h.has("Small window message box"));
    // and nothing opens it while the sign-in screen is up
    let general = h.app.data.channels[0].id;
    h.app.open_mini(general);
    h.app.login = Some(crate::app::login::LoginScreen::new(
        crate::app::login::LoginPage::Server,
        "",
    ));
    h.settle();
    assert_eq!(h.app.mini.channel, None);
    assert!(!h.has("Small window message box"));
}

#[test]
fn another_workspace_closes_the_small_window_and_forgets_its_draft() {
    let guard = crate::app::tests::HOME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home-mini-ws");
    std::fs::create_dir_all(&home).unwrap();
    crate::app::tests::point_home(&home);
    let mut h = Ui1::new();
    let general = h.channel_id("general");
    h.app.open_mini(general);
    h.app.mini.text = "for the first workspace".into();
    h.settle();
    assert!(h.has("Small window message box"));
    // a new workspace opens in its place: its #general may have the same id
    h.click("Workspace menu");
    h.click("Create Workspace");
    h.type_text("Second");
    h.click("Next");
    h.click("Create");
    h.settle();
    assert_eq!(h.app.data.workspace, "Second");
    assert_eq!(h.app.mini.channel, None);
    assert!(h.app.mini.text.is_empty());
    assert!(!h.has("Small window message box"));
    drop(guard);
}

#[test]
fn what_the_small_window_shows_makes_no_toast() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let other = someone_else(&h);
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members.contains(&other.id))
        .map(|c| c.id)
        .unwrap_or_else(|| {
            let id = 9_999;
            let mut c = h.app.data.channels[0].clone();
            c.id = id;
            c.kind = ChannelKind::Direct;
            c.private = false;
            c.members = vec![me, other.id];
            c.messages.clear();
            h.app.data.channels.push(c);
            id
        });
    h.click("Rail Home");
    h.settle();
    // without the small window a direct message shows a toast
    message_from(&mut h, dm, other.id, "toast for this one");
    h.app.notify_rt.pending = true;
    h.settle();
    assert!(
        h.app
            .toast
            .as_ref()
            .is_some_and(|t| t.0.contains("toast for this one")),
        "{:?}",
        h.app.toast
    );
    h.app.toast = None;
    // shown in the small window: no toast, and read
    h.app.open_mini(dm);
    h.settle();
    message_from(&mut h, dm, other.id, "already in sight");
    h.app.notify_rt.pending = true;
    h.settle();
    assert!(
        !h.app
            .toast
            .as_ref()
            .is_some_and(|t| t.0.contains("already in sight")),
        "{:?}",
        h.app.toast
    );
    assert!(h.has("already in sight"));
}

#[test]
fn small_window_shows_who_is_typing_under_its_box() {
    let mut h = Ui1::new();
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    h.app.section = Section::Chat;
    h.app.chat.open = Some(dm);
    h.settle();
    h.click("Open in a small window");
    assert_eq!(h.app.mini.channel, Some(dm));
    let name = h.app.data.member(2).unwrap().shown().to_string();
    let line = format!("{} is typing...", name);
    let box_at = h.rect("Small window message box");
    assert!(!h.has(&line));

    h.app.take_net_events(vec![wb_net::Event::Typing {
        channel: dm,
        member: 2,
        thread: None,
    }]);
    h.settle();
    assert!(
        h.tree.iter().any(|(n, r)| n == &line
            && r.top() >= box_at.bottom() - 1.0
            && r.left() >= box_at.left() - 20.0
            && r.right() <= box_at.right() + 60.0),
        "the line sits just under the small window's box"
    );
    assert_eq!(
        h.rect("Small window message box"),
        box_at,
        "the box stays put"
    );

    // their message arrives: gone
    message_from(&mut h, dm, 2, "here it is");
    h.settle();
    assert!(!h.has(&line));
}
