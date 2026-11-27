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
            hand: false,
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
            hand: false,
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
    h.app.huddle.pictures.insert(
        format!("{}-dev2", other.id),
        (std::sync::Arc::new(pic.clone()), true),
    );
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
        .insert(wb_voice::ME.to_string(), (std::sync::Arc::new(pic), true));
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
        hand: false,
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
    // you see your own screen as it is sent, under "Your screen"
    let mine = wb_voice::screen_key(wb_voice::ME);
    let own = wb_voice::Picture {
        width: 192,
        height: 108,
        rgba: vec![90u8; 192 * 108 * 4],
    };
    h.app
        .huddle
        .pictures
        .insert(mine.clone(), (std::sync::Arc::new(own), true));
    h.settle();
    assert!(h.has("Your screen"));
    assert!(
        !h.app.huddle.views.contains_key(&mine),
        "your own screen asks the server for nothing"
    );
    h.click("Stop sharing screen");
    h.app.huddle.drop_picture(&mine);
    h.settle();
    assert!(!h.has("Your screen"));

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
        .insert(screen.clone(), (std::sync::Arc::new(pic(192, 108)), true));
    h.app.huddle.pictures.insert(
        them.identity.clone(),
        (std::sync::Arc::new(pic(64, 36)), true),
    );
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

/// In a huddle in #general on a server, with two others there.
fn in_huddle(h: &mut Ui1) {
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    let me = h.app.data.me;
    let others: Vec<u64> = h
        .app
        .data
        .members
        .iter()
        .filter(|m| m.id != me)
        .map(|m| m.id)
        .take(2)
        .collect();
    let general = h.channel_id("general");
    h.app.huddle.channel = Some(general);
    h.app.huddle.joined = true;
    let person = |id: u64, dev: &str, me: bool| wb_voice::Person {
        identity: format!("{}-{}", id, dev),
        name: String::new(),
        me,
        speaking: !me,
        muted: false,
        heard: 0.0,
        camera: false,
        screen: false,
        hand: false,
    };
    h.app.huddle.people = vec![person(me, "dev1", true)];
    for (i, id) in others.iter().enumerate() {
        h.app
            .huddle
            .people
            .push(person(*id, &format!("dev{}", i + 2), false));
    }
    h.app.section = Section::Chat;
    h.app.chat.open = Some(general);
    h.settle();
}

/// The huddle bar sits over the message box, clear of its Message / Post /
/// Ask tabs and of the sidebar, inside the window however narrow (it
/// covered the top of the tabs, the sidebar's "Today & Overdue" beside a
/// narrow page, and ran past both edges of a compact window); the screen
/// chooser and its no-permission note fit the window too (they started left
/// of it).
#[test]
fn huddle_bar_and_screen_chooser_fit_the_window() {
    use crate::i18n::{set_lang, Lang};
    for lang in [Lang::En, Lang::Ko] {
        for (w, hh) in [
            (1440.0, 900.0),
            (960.0, 560.0),
            (700.0, 640.0),
            (460.0, 640.0),
            (360.0, 640.0),
        ] {
            for chooser in [None, Some(false), Some(true)] {
                let mut h = Ui1::sized(w, hh);
                set_lang(lang);
                in_huddle(&mut h);
                if let Some(denied) = chooser {
                    h.app.huddle.share_menu = true;
                    if denied {
                        h.app.huddle.screen_problem = Some(wb_voice::ScreenTrouble::NotAllowed);
                    }
                    h.settle();
                }
                let what = format!("{:?} {}x{} chooser {:?}", lang, w, hh, chooser);
                let window = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
                let bar = h
                    .ctx
                    .memory(|m| m.area_rect(egui::Id::new("huddle_bar")))
                    .unwrap_or_else(|| panic!("{}: the bar", what));
                let tabs: Vec<Rect> = ["Message", "Post", "Ask"]
                    .iter()
                    .map(|n| h.rect(n))
                    .collect();
                let today = h
                    .has(tr("Today & Overdue"))
                    .then(|| h.rect(tr("Today & Overdue")));
                let menu = h
                    .ctx
                    .memory(|m| m.area_rect(egui::Id::new("huddle_share_menu")));
                let buttons: Vec<(&str, Rect)> = [
                    "Mute",
                    "Microphone and speaker",
                    "Video",
                    "Choose a camera",
                    "Share screen",
                    "Raise hand",
                    "Reactions",
                    "Open the huddle's conversation",
                    "Leave",
                    "Huddle in a small window",
                ]
                .iter()
                .map(|n| (*n, h.rect(n)))
                .collect();
                let cut = h.cut_texts(window);
                set_lang(Lang::En);
                assert!(window.contains_rect(bar), "{}: bar {:?}", what, bar);
                // every button inside the card (more rows when narrow)
                for (n, r) in &buttons {
                    assert!(
                        bar.contains_rect(*r),
                        "{}: {} {:?} outside the card {:?}",
                        what,
                        n,
                        r,
                        bar
                    );
                }
                for t in &tabs {
                    assert!(
                        !bar.intersects(*t),
                        "{}: bar {:?} over a tab {:?}",
                        what,
                        bar,
                        t
                    );
                }
                if let Some(r) = today {
                    assert!(
                        !bar.intersects(r),
                        "{}: bar {:?} over the sidebar {:?}",
                        what,
                        bar,
                        r
                    );
                }
                if chooser.is_some() {
                    let m = menu.unwrap_or_else(|| panic!("{}: the chooser", what));
                    assert!(window.contains_rect(m), "{}: chooser {:?}", what, m);
                    assert!(
                        !m.intersects(bar),
                        "{}: chooser {:?} over the bar {:?}",
                        what,
                        m,
                        bar
                    );
                }
                assert!(cut.is_empty(), "{}: cut {:?}", what, cut);
            }
        }
    }
}

/// Raise hand, reactions and the microphone, speaker and camera menus: the
/// hand shows on your tile and the button turns to Lower hand, a reaction
/// picked (or one that comes from someone else) floats over the sender's
/// tile, and a device picked in its menu is the one used.
#[test]
fn huddle_raises_a_hand_reacts_and_picks_devices() {
    let mut h = Ui1::new();
    in_huddle(&mut h);
    let me = h.app.data.me;
    let my_name = h.app.data.member(me).unwrap().name.clone();
    let mine = format!("{} (you), listening", my_name);
    assert!(h.has(&mine));

    // the hand goes up on your tile, and down again
    h.click("Raise hand");
    assert!(h.app.huddle.hand);
    assert!(h.has(&format!("{}, hand raised", mine)));
    h.click("Lower hand");
    assert!(!h.app.huddle.hand);
    assert!(h.has(&mine));
    // someone else's raised hand shows on theirs
    h.app.huddle.people[1].hand = true;
    let them = h.app.huddle.people[1].clone();
    let their_name = h
        .app
        .data
        .member(them.member().unwrap())
        .unwrap()
        .name
        .clone();
    h.settle();
    assert!(h.has(&format!("{}, speaking, hand raised", their_name)));

    // reactions: the menu opens above its button, a pick closes it and the
    // reaction floats over your tile
    h.click("Reactions");
    assert_eq!(
        h.app.huddle.menu,
        Some(crate::app::huddle::HuddleMenu::Reactions)
    );
    let clap = "\u{1F44F}";
    assert!(h.rect(&format!("React {}", clap)).bottom() < h.rect("Reactions").top());
    h.click(&format!("React {}", clap));
    assert!(h.app.huddle.menu.is_none());
    assert!(h
        .app
        .huddle
        .reactions
        .iter()
        .any(|(who, e, _)| who == wb_voice::ME && e == clap));
    // and they go after a few seconds
    let long_ago = std::time::Instant::now() - std::time::Duration::from_secs(10);
    for r in h.app.huddle.reactions.iter_mut() {
        r.2 = long_ago;
    }
    h.settle();
    assert!(h.app.huddle.reactions.is_empty());

    // the microphone's arrow lists the microphones and speakers; the first
    // is checked until another is picked
    h.app.huddle.mics = vec![
        wb_voice::Device {
            id: "m1".into(),
            name: "MacBook Microphone".into(),
        },
        wb_voice::Device {
            id: "m2".into(),
            name: "USB Microphone".into(),
        },
    ];
    h.app.huddle.speakers = vec![wb_voice::Device {
        id: "s1".into(),
        name: "MacBook Speakers".into(),
    }];
    h.click("Microphone and speaker");
    assert_eq!(
        h.app.huddle.menu,
        Some(crate::app::huddle::HuddleMenu::Devices)
    );
    assert!(h.has("MacBook Microphone") && h.has("MacBook Speakers"));
    h.click("USB Microphone");
    assert_eq!(h.app.huddle.mic.as_deref(), Some("m2"));
    assert!(h.app.huddle.menu.is_none());
    // a click elsewhere closes a menu without picking
    h.click("Microphone and speaker");
    assert!(h.app.huddle.menu.is_some());
    let away = h.rect("Message").center();
    h.click_at(away);
    assert!(h.app.huddle.menu.is_none());
    assert_eq!(h.app.huddle.mic.as_deref(), Some("m2"));

    // the camera's arrow lists the cameras
    h.click("Choose a camera");
    assert!(h.has("Test camera"));
    h.click("Test camera");
    assert_eq!(h.app.huddle.camera_pick.as_deref(), Some("Test camera"));
    // the picks stay for the next huddle
    h.app.leave_huddle();
    assert_eq!(h.app.huddle.mic.as_deref(), Some("m2"));
    assert_eq!(h.app.huddle.camera_pick.as_deref(), Some("Test camera"));
}
