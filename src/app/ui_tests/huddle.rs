// Click-through tests for huddles (calls): starting one, the call bar,
// screen sharing and why a call is not offered.

use super::*;

#[test]
fn click_huddle_starts_a_call_in_general() {
    let mut h = Ui1::new();
    h.click("Huddle");
    assert!(matches!(h.app.modal, Some(Modal::Call { .. })));
    h.click("Start call");
    assert!(h.app.modal.is_none());
    assert_eq!(h.app.section, Section::Chat);
    let ch = h.app.chat.open.unwrap();
    let m = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == ch)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone();
    assert!(m.call.is_some(), "a call message with a Join button");
    assert!(h.has("Join call"));
}

#[test]
fn calls_say_why_there_is_no_huddle_when_the_server_is_down_and_when_nobody_else_is_there() {
    let alone_note = "You're the only one in this conversation, so nobody else gets the Join button. Invite people first.";
    let down_note = "The team's server can't be reached, so a huddle in the app isn't available. This call opens in the browser instead, and the others see its Join button once you're connected again.";
    let mut h = Ui1::new();
    let general = h.channel_id("general");
    h.app.section = Section::Chat;
    h.app.chat.open = Some(general);
    // a team in #general, no server: the browser call, no extra words
    h.app.open_call_dialog();
    h.settle();
    assert!(matches!(h.app.modal, Some(Modal::Call { .. })));
    assert!(!h.has(alone_note) && !h.has(down_note));
    h.key(egui::Key::Escape);
    // the server is set up but down: says why it's a browser call
    server_down(&mut h);
    h.app.open_call_dialog();
    h.settle();
    assert!(matches!(h.app.modal, Some(Modal::Call { .. })));
    assert!(h.has(down_note));
    h.key(egui::Key::Escape);
    // joining someone's huddle: not "huddles need a server" when there is one
    h.app.join_huddle(general);
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Huddles go through the team's server, and it can't be reached right now. Try again once it's connected.")
    );
    // alone in the conversation: nobody else gets the Join button
    let mut solo = Ui1::new();
    let me = solo.app.data.me;
    solo.app.data.members.retain(|m| m.id == me);
    for c in &mut solo.app.data.channels {
        c.members.retain(|id| *id == me);
    }
    solo.app.section = Section::Chat;
    solo.app.chat.open = Some(general);
    solo.app.open_call_dialog();
    solo.settle();
    assert!(solo.has(alone_note));
}

#[test]
fn huddle_bar_shows_people_mute_and_leave() {
    let mut h = Ui1::new();
    let ch = h.app.data.channels[0].id;
    let me = h.app.data.me;
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone();
    let my_name = h.app.data.member(me).unwrap().name.clone();
    // a huddle message in chat has a Join button
    let id = h.app.data.new_id();
    h.app.data.channels[0].messages.push(Message {
        goal: None,
        edited: None,
        imported: None,
        preview: None,
        forwarded: None,
        thread_of: None,
        bot: None,
        mentions: vec![],
        attachments: vec![],
        id,
        author: other.id,
        text: "Started a huddle".into(),
        ts: now_ts(),
        reactions: vec![],
        replies: vec![],
        saved: false,
        task: None,
        post_title: None,
        post_type: String::new(),
        pinned: false,
        call: Some(format!("{}{}", crate::app::huddle::HUDDLE_PREFIX, ch)),
    });
    h.app.section = Section::Chat;
    h.app.chat.open = Some(ch);
    h.settle();
    h.click("Join huddle");
    assert!(
        h.app.huddle.channel.is_none(),
        "not on a server: no in-app huddle"
    );
    assert!(h.app.toast.as_ref().is_some_and(|t| t.0.contains("server")));

    // in a huddle (on a server): the bar shows who is there and who is speaking
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.huddle.channel = Some(ch);
    h.app.huddle.joined = true;
    h.app.huddle.people = vec![
        wb_voice::Person {
            identity: format!("{}-dev1", me),
            name: String::new(),
            me: true,
            speaking: false,
            muted: false,
            heard: 0.0,
            camera: false,
            screen: false,
        },
        wb_voice::Person {
            identity: format!("{}-dev2", other.id),
            name: String::new(),
            me: false,
            speaking: true,
            muted: false,
            heard: 0.3,
            camera: true,
            screen: false,
        },
    ];
    h.settle();
    assert!(h.has(&format!("{} (you), listening", my_name)));
    assert!(h.has(&format!("{}, speaking", other.name)));
    assert!(h.has("2 people"));
    assert!(
        !h.has("Join huddle"),
        "no Join button for the huddle you're in"
    );
    h.click("Mute");
    assert!(h.app.huddle.muted);
    assert!(h.has("Unmute"));
    // video: the other person's picture shows with their name, the Video button toggles
    let pic = wb_voice::Picture {
        width: 32,
        height: 18,
        rgba: vec![200u8; 32 * 18 * 4],
    };
    h.app
        .huddle
        .pictures
        .insert(format!("{}-dev2", other.id), (pic.clone(), true));
    h.settle();
    assert!(h.has(&format!("Video of {}", other.name)));
    // clicking a video makes it large, clicking again makes it small
    let small = h.rect(&format!("Video of {}", other.name));
    h.click(&format!("Video of {}", other.name));
    assert_eq!(
        h.app.huddle.focus.as_deref(),
        Some(format!("{}-dev2", other.id).as_str())
    );
    let large = h.rect(&format!("Video of {}", other.name));
    assert!(
        large.width() > small.width() * 1.5,
        "{:?} -> {:?}",
        small,
        large
    );
    h.click(&format!("Video of {}", other.name));
    assert!(h.app.huddle.focus.is_none());
    h.click("Video");
    assert!(h.app.huddle.camera);
    h.app
        .huddle
        .pictures
        .insert(wb_voice::ME.to_string(), (pic, true));
    h.settle();
    assert!(h.has("Video of You") && h.has("Stop video"));
    h.click("Stop video");
    assert!(!h.app.huddle.camera);
    h.click("Leave");
    assert!(h.app.huddle.channel.is_none());
    assert!(!h.has("Unmute"));
}

#[test]
fn huddle_shares_a_screen_and_shows_others_screens_large() {
    let mut h = Ui1::new();
    let ch = h.app.data.channels[0].id;
    let me = h.app.data.me;
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    let person = |id: u64, dev: &str, me: bool, screen: bool| wb_voice::Person {
        identity: format!("{}-{}", id, dev),
        name: String::new(),
        me,
        speaking: false,
        muted: false,
        heard: 0.0,
        camera: true,
        screen,
    };
    h.app.huddle.channel = Some(ch);
    h.app.huddle.joined = true;
    h.app.huddle.people = vec![person(me, "dev1", true, false)];
    h.settle();

    // Share screen offers the displays and the windows
    h.click("Share screen");
    assert!(h.app.huddle.share_menu);
    assert!(h.has("Share Test display") && h.has("Share Notes: Plan"));
    h.click("Share Notes: Plan");
    assert_eq!(h.app.huddle.screen, Some(wb_voice::Share::Window(7)));
    assert!(!h.app.huddle.share_menu);
    assert!(h.has("Stop sharing screen") && h.has("Sharing your screen"));
    h.click("Stop sharing screen");
    assert!(h.app.huddle.screen.is_none());
    h.click("Share screen");
    h.click("Share Test display");
    assert_eq!(h.app.huddle.screen, Some(wb_voice::Share::Display(1)));
    h.click("Stop sharing screen");

    // without the Screen Recording permission: how to turn it on
    h.app.huddle.screen_env = crate::app::huddle::ScreenEnv::Fake {
        allowed: false,
        sources: vec![],
    };
    h.click("Share screen");
    assert!(h.has("Workboard needs the Screen Recording permission to share your screen."));
    assert!(!h.has("Share Test display"));
    h.click("Open System Settings");
    assert!(h.app.huddle.opened_settings);
    h.click("Cancel");
    assert!(!h.app.huddle.share_menu);
    // capture refused once sharing started: the same guidance comes up
    h.app.huddle.screen = Some(wb_voice::Share::Display(1));
    h.app.huddle.screen_problem = Some(wb_voice::ScreenTrouble::NotAllowed);
    h.app.huddle.share_menu = true;
    h.settle();
    assert!(h.has("Open System Settings"));
    h.click("Cancel");

    // someone else shares: their screen shows large with their name, their
    // camera small, and each picture asks for the layer that fits its size
    let them = person(other.id, "dev2", false, true);
    h.app.huddle.people.push(them.clone());
    let pic = |w: u32, hh: u32| wb_voice::Picture {
        width: w,
        height: hh,
        rgba: vec![120u8; (w * hh * 4) as usize],
    };
    let screen = wb_voice::screen_key(&them.identity);
    h.app
        .huddle
        .pictures
        .insert(screen.clone(), (pic(192, 108), true));
    h.app
        .huddle
        .pictures
        .insert(them.identity.clone(), (pic(64, 36), true));
    h.settle();
    assert!(h.has(&format!("Screen of {}", other.name)));
    assert!(h.has(&format!("{}'s screen", other.name)));
    let big = h.rect(&format!("Screen of {}", other.name));
    let small = h.rect(&format!("Video of {}", other.name));
    assert!(big.width() > small.width() * 3.0, "{:?} {:?}", big, small);
    let views = h.app.huddle.views.clone();
    assert_eq!(views.len(), 2, "{:?}", views);
    assert_eq!(
        wb_voice::quality::layer_for(views[&them.identity]),
        wb_voice::quality::VideoQuality::Low
    );
    assert!(wb_voice::quality::layer_for(views[&screen]) != wb_voice::quality::VideoQuality::Low);
    // the screen share ends: back to normal-sized video
    h.app.huddle.drop_picture(&screen);
    h.settle();
    assert!(!h.has(&format!("Screen of {}", other.name)));
    assert!(h.rect(&format!("Video of {}", other.name)).width() > small.width() * 1.5);
    assert_eq!(h.app.huddle.views.len(), 1);
}
