// Click-through tests for chat: channels, direct messages, threads, the
// composer, message actions, mentions and who can see or change a channel.

use super::*;

fn general_msg<'a>(h: &'a Ui1, cid: u64, start: &str) -> Option<&'a Message> {
    h.app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .iter()
        .find(|m| m.text.starts_with(start))
}

#[test]
fn click_channel_and_send_message() {
    let mut h = Ui1::new();
    h.click("general");
    assert_eq!(h.app.section, Section::Chat);
    let cid = h.app.chat.open.unwrap();
    let n = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    h.click("Message composer");
    h.type_text("hello from a click test");
    h.key(egui::Key::Enter);
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(ch.messages.len(), n + 1);
    assert_eq!(ch.messages.last().unwrap().text, "hello from a click test");
}

/// A channel with a long history draws its newest messages; "Show earlier
/// messages" at the top of the list brings the earlier ones a page at a time.
#[test]
fn a_long_channel_shows_its_newest_messages_and_earlier_ones_on_asking() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let ch = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap();
    let before = ch.messages.len();
    let old = 450;
    for i in 0..old {
        // long before the channel's other messages
        ch.messages.push(crate::app::slack::new_message(
            900_000 + i as u64,
            2,
            format!("Old message {}", i),
            1_600_000_000 + i as i64 * 60,
        ));
    }
    ch.messages.sort_by_key(|m| m.ts);
    let total = before + old;
    h.settle();
    assert!(
        h.has(&format!("Old message {}", old - 1)),
        "the newest of them shows"
    );
    assert!(!h.has("Old message 0"), "the oldest waits");
    let up = |h: &mut Ui1| {
        // to the top of the list
        let at = pos2(500.0, 400.0);
        h.frame(vec![egui::Event::PointerMoved(at)]);
        for _ in 0..6 {
            h.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, 20_000.0),
                modifiers: Default::default(),
            }]);
        }
        h.settle();
    };
    up(&mut h);
    h.click(&format!(
        "Show {} earlier messages",
        crate::app::chat::CHAT_PAGE
    ));
    assert!(!h.has("Old message 0"));
    up(&mut h);
    let rest = total - 2 * crate::app::chat::CHAT_PAGE;
    h.click(&format!("Show {} earlier messages", rest));
    assert!(h.has("Old message 0"), "all of them now");
    assert!(!h.has(&format!("Show {} earlier messages", rest)));

    // another conversation starts again from its newest messages
    h.app.chat.window = Some((cid + 1, 10));
    h.settle();
    assert!(
        h.has(&format!(
            "Show {} earlier messages",
            crate::app::chat::CHAT_PAGE
        )),
        "only the conversation it was asked in keeps more"
    );
}

#[test]
fn click_message_replies_opens_thread() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    assert!(h.app.chat.thread_open.is_some(), "the thread panel opens");
}

#[test]
fn click_thread_reply_and_hover_reaction() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("1 reply");
    let mid = h.app.chat.thread_open.unwrap();
    h.click("Reply composer");
    h.type_text("on it");
    h.key(egui::Key::Enter);
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let m = ch.messages.iter().find(|m| m.id == mid).unwrap();
    assert_eq!(
        m.replies.last().unwrap().text,
        "on it",
        "reply posted in the thread"
    );
    let before = m.reactions.iter().map(|r| r.users.len()).sum::<usize>();

    // hover the last message to show its toolbar, then react with a party emoji
    let p = h
        .find("@Me can you check the beta feedback list when you have a minute?")
        .unwrap();
    h.hover(p);
    h.click("\u{1F389}");
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let last = ch
        .messages
        .iter()
        .find(|m| m.text.starts_with("@Me"))
        .unwrap();
    assert!(
        last.reactions.iter().any(|r| r.emoji == "\u{1F389}"),
        "reaction added from the hover toolbar"
    );
    let _ = before;
}

#[test]
fn rail_has_no_labs_or_upgrade_and_empty_chat_can_start_a_channel() {
    let mut h = Ui1::new();
    assert!(!h.has("Rail Labs") && !h.has("Rail Upgrade") && !h.has("Rail Install"));
    assert!(h.has("Rail Invite"));
    h.app.data.channels.clear();
    h.app.section = Section::Chat;
    h.app.chat.open = None;
    h.settle();
    h.click("Create a Channel");
    assert_eq!(h.app.data.channels.len(), 1);
    assert_eq!(h.app.data.channels[0].name, "general");
    assert_eq!(h.app.chat.open, Some(h.app.data.channels[0].id));
}

#[test]
fn sidebar_add_channel_takes_enter_and_lets_go_on_escape() {
    let mut h = Ui1::new();
    let n = h.app.data.channels.len();
    h.click_side("Add Channel", false);
    assert!(h.app.new_channel.is_some());
    h.type_text("dev team");
    h.key(egui::Key::Enter);
    assert_eq!(
        h.app.data.channels.len(),
        n + 1,
        "Enter creates the channel"
    );
    assert!(h.app.data.channels.iter().any(|c| c.name == "dev-team"));
    assert!(h.app.new_channel.is_none());
    // Escape leaves the field and typing goes elsewhere again
    h.app.section = Section::Home;
    h.settle();
    h.click_side("Add Channel", false);
    assert!(h.app.new_channel.is_some());
    h.key(egui::Key::Escape);
    assert!(h.app.new_channel.is_none(), "Escape closes the field");
    assert!(
        h.ctx.memory(|m| m.focused().is_none()),
        "nothing keeps the keyboard"
    );
}

#[test]
fn new_message_window_closes_on_escape_and_when_moving_away() {
    let mut h = Ui1::new();
    h.app.new_message_open = true;
    h.settle();
    h.key(egui::Key::Escape);
    assert!(!h.app.new_message_open);
    h.app.new_message_open = true;
    h.settle();
    h.click("Inbox");
    assert!(!h.app.new_message_open, "going to another page closes it");
}

#[test]
fn chat_drafts_stay_with_their_channel() {
    let mut h = Ui1::new();
    h.click("general");
    let a = h.app.chat.open.unwrap();
    h.click("Message composer");
    h.type_text("hello");
    // move to another channel, write and send there
    let b = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id != a)
        .expect("seed has two channels")
        .clone();
    h.click_side(&b.name, false);
    assert_eq!(h.app.chat.open, Some(b.id));
    assert_eq!(h.app.chat.text, "", "the other channel starts empty");
    h.click("Message composer");
    h.type_text("first message");
    h.key(egui::Key::Enter);
    let sent = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == b.id)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .text
        .clone();
    assert_eq!(sent, "first message");
    // back to the first channel: the unsent text is still there
    h.click_side("general", false);
    assert_eq!(h.app.chat.text, "hello");
}

#[test]
fn new_channel_field_comes_into_view_in_a_long_sidebar() {
    // (+ Create > Channel now opens the Create Channel window; the sidebar's
    // "Add Channel" row keeps the field at the end of the channel list)
    for via_create in [false] {
        let mut h = Ui1::new();
        // a long sidebar: many lists in the first space
        let base = h
            .app
            .data
            .spaces
            .iter()
            .flat_map(|s| s.lists.iter())
            .next()
            .cloned()
            .or_else(|| {
                h.app
                    .data
                    .spaces
                    .iter()
                    .flat_map(|s| s.folders.iter().flat_map(|f| f.lists.iter()))
                    .next()
                    .cloned()
            })
            .unwrap();
        for i in 0..30 {
            let mut l = base.clone();
            l.id = h.app.data.new_id();
            l.name = format!("Extra list {}", i);
            h.app.data.spaces[0].lists.push(l);
        }
        h.settle();
        if via_create {
            h.click("Create");
            h.click("Channel");
        } else {
            // the sidebar's own "Add Channel" row, scrolled to with the wheel
            h.click_side("Add Channel", false);
        }
        assert!(h.app.new_channel.is_some());
        for _ in 0..30 {
            h.frame(vec![]); // let the scroll animation finish
        }
        let r = h
            .ctx
            .read_response(egui::Id::new("new_channel_input"))
            .expect("the field is drawn")
            .rect;
        assert!(
            r.top() >= 40.0 && r.bottom() <= 880.0,
            "via_create={}: the field is on screen ({:?})",
            via_create,
            r
        );
        h.type_text("design");
        h.key(egui::Key::Enter);
        assert!(h.app.data.channels.iter().any(|c| c.name == "design"));
    }
}

#[test]
fn new_message_window_searches_people_and_channels() {
    let mut h = Ui1::new();
    h.click("Create");
    h.click("Message");
    assert!(h.has("People or Channels"));
    assert!(h.has("Recents") && h.has("People") && h.has("Channels"));
    // Channels tab, search, pick: goes to the channel
    h.click("Channels");
    h.click("People or Channels");
    h.type_text("website");
    assert!(!h.has("# general"), "search narrows the list");
    h.click("# website-redesign");
    let ch = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat.open, Some(ch));
    assert!(!h.app.new_message_open);
    // reopened, the old search is gone; the avatar row opens a DM
    h.app.new_message_open = true;
    h.settle();
    assert!(h.has("# general"));
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != h.app.data.me)
        .unwrap()
        .clone();
    h.click(&format!("Message {}", other.name));
    let open = h.app.chat.open.unwrap();
    let c = h.app.data.channels.iter().find(|c| c.id == open).unwrap();
    assert_eq!(c.kind, ChannelKind::Direct);
    assert!(c.members.contains(&other.id));
}

#[test]
fn create_channel_window_with_private_and_list() {
    let mut h = Ui1::new();
    h.click("Create");
    h.click("Channel");
    assert!(
        matches!(h.app.modal, Some(Modal::NewChannel { .. })),
        "Create Channel window"
    );
    h.type_text("Design Team");
    h.click_side("Make Private", true); // the switch, right of its label
    h.click_side("Add a List", true);
    h.click_side("Create", true);
    let c = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "design-team")
        .expect("channel made")
        .clone();
    assert!(c.private);
    let lid = c.list.expect("a List tied to the channel");
    assert_eq!(h.app.data.list_name(lid), "Design Team");
    assert_eq!(h.app.chat.open, Some(c.id));
    assert!(h.app.modal.is_none());
    // the channel's List tab opens that List
    h.click("List");
    h.app.apply_actions();
    h.settle();
    assert_eq!(h.app.loc, Location::List(lid));
}

#[test]
fn create_channel_needs_a_name_and_old_files_still_load() {
    let mut h = Ui1::new();
    let n = h.app.data.channels.len();
    h.app.modal = Some(Modal::NewChannel {
        name: String::new(),
        private: false,
        add_list: false,
    });
    h.settle();
    h.click_side("Create", true);
    assert_eq!(h.app.data.channels.len(), n, "no channel without a name");
    let old = r#"{"id":1,"name":"x","kind":"Channel","topic":"","members":[],"messages":[]}"#;
    let c: Channel = serde_json::from_str(old).unwrap();
    assert!(!c.private && c.list.is_none());
}

#[test]
fn unread_filter_still_lists_unread_chats_while_muted() {
    let mut h = Ui1::new();
    let side = Rect::from_min_max(pos2(0.0, 0.0), pos2(320.0, 5000.0));
    let me = h.app.data.me;
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| {
            c.kind == ChannelKind::Direct
                && c.messages
                    .iter()
                    .any(|m| m.ts > c.last_read && m.author != me)
        })
        .expect("a DM with an unread message")
        .clone();
    let other = dm
        .members
        .iter()
        .find(|m| **m != me)
        .and_then(|m| h.app.data.member(*m))
        .unwrap()
        .name
        .clone();
    h.click("Profile menu");
    h.hover(h.rect("Mute notifications").center());
    h.click("For 4 hours");
    assert!(h.app.data.extra.muted_until > now_ts());
    h.click("Filter your sidebar");
    h.click("Sidebar filter Unread");
    assert_eq!(h.app.side.filter, Some("unread"));
    assert!(
        h.tree
            .iter()
            .any(|(n, r)| *n == other && side.contains(r.center())),
        "the unread DM with {} is listed while muted",
        other
    );
    // a DM without unread messages is not
    let read = h.app.data.channels.iter().find(|c| {
        c.kind == ChannelKind::Direct
            && !c
                .messages
                .iter()
                .any(|m| m.ts > c.last_read && m.author != me)
    });
    if let Some(c) = read {
        if let Some(n) = c
            .members
            .iter()
            .find(|m| **m != me)
            .and_then(|m| h.app.data.member(*m))
            .map(|m| m.name.clone())
        {
            assert!(
                !h.tree
                    .iter()
                    .any(|(x, r)| *x == n && side.contains(r.center())),
                "read DM {} is filtered out",
                n
            );
        }
    }
}

#[test]
fn click_more_actions_edit_my_message() {
    let (mut h, cid) = general_with_my_message("first draft");
    h.click("More actions");
    h.click("Edit message");
    assert!(
        h.app.chat.edit.is_some(),
        "the message turns into a text box"
    );
    h.click("Edit message");
    for _ in 0.."first draft".len() {
        h.key(egui::Key::Backspace);
    }
    h.type_text("final words");
    h.key(egui::Key::Enter);
    assert!(h.app.chat.edit.is_none());
    let m = general_msg(&h, cid, "final words").expect("text changed");
    assert!(m.edited.is_some(), "marked as edited");
    assert!(h.has("(edited)"));
}

#[test]
fn escape_cancels_an_edit_and_up_arrow_starts_one() {
    let (mut h, cid) = general_with_my_message("keep me");
    h.click("Message composer");
    h.key(egui::Key::ArrowUp);
    let mid = general_msg(&h, cid, "keep me").unwrap().id;
    assert_eq!(
        h.app.chat.edit.as_ref().map(|e| e.1),
        Some(mid),
        "Up in an empty box edits my last message"
    );
    h.click("Edit message");
    h.type_text(" changed");
    h.key(egui::Key::Escape);
    assert!(h.app.chat.edit.is_none());
    let m = general_msg(&h, cid, "keep me").unwrap();
    assert_eq!(m.text, "keep me", "Escape leaves the text as it was");
    assert!(m.edited.is_none());
}

#[test]
fn click_more_actions_delete_needs_two_clicks() {
    let (mut h, cid) = general_with_my_message("oops wrong channel");
    h.click("More actions");
    h.click("Delete message");
    assert!(
        general_msg(&h, cid, "oops").is_some(),
        "one click only asks"
    );
    h.click("Click again to delete");
    assert!(
        general_msg(&h, cid, "oops").is_none(),
        "the second click deletes"
    );
}

#[test]
fn others_messages_cannot_be_edited_or_deleted() {
    let mut h = Ui1::new();
    h.click("general");
    let p = h
        .find("Reminder: newsletter draft is due Thursday.")
        .unwrap();
    h.hover(p);
    h.click("More actions");
    assert!(h.has("Copy text") && h.has("Mark unread"));
    assert!(!h.has("Edit message") && !h.has("Delete message"));
}

#[test]
fn click_more_reactions_picks_any_emoji() {
    let (mut h, cid) = general_with_my_message("ship it");
    h.click("More reactions");
    h.click("\u{1F680}");
    let m = general_msg(&h, cid, "ship it").unwrap();
    assert!(m
        .reactions
        .iter()
        .any(|r| r.emoji == "\u{1F680}" && r.users == vec![h.app.data.me]));
}

#[test]
fn click_mark_unread_keeps_the_channel_unread() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let text = "Reminder: newsletter draft is due Thursday.";
    let p = h.find(text).unwrap();
    h.hover(p);
    h.click("More actions");
    h.click("Mark unread");
    h.settle();
    let ts = general_msg(&h, cid, "Reminder:").unwrap().ts;
    let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert!(
        c.last_read < ts,
        "still unread while the channel stays open"
    );
    // leaving and coming back reads it
    h.click("website-redesign");
    h.click("general");
    let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert!(c.last_read >= ts);
}

#[test]
fn click_remind_me_about_a_message() {
    let mut h = Ui1::new();
    h.click("general");
    let p = h
        .find("Reminder: newsletter draft is due Thursday.")
        .unwrap();
    h.hover(p);
    h.click("More actions");
    h.click("Remind me in 1 hour");
    let r = h
        .app
        .data
        .extra
        .reminders
        .last()
        .expect("a reminder is made");
    assert!(
        r.text
            .starts_with("Message from Sam Lee: Reminder: newsletter"),
        "{}",
        r.text
    );
    assert!((r.due_ts - now_ts() - 3600).abs() < 10);
}

#[test]
fn composer_mention_and_emoji_pickers() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Mention someone");
    h.click("Alex Kim");
    assert_eq!(h.app.chat.text, "@Alex Kim ");
    h.click("Emoji");
    h.click("\u{1F525}");
    assert_eq!(h.app.chat.text, "@Alex Kim \u{1F525}");
    // typing "@" opens the list by itself and the pick replaces the "@"
    h.type_text(" @");
    assert!(h.has("Jordan Park"), "typing @ offers people");
    // (the right-most "Jordan Park" is the list's row, not a name in the feed)
    h.click_side("Jordan Park", true);
    assert_eq!(h.app.chat.text, "@Alex Kim \u{1F525} @Jordan Park ");
}

#[test]
fn composer_emoji_picker_opens_above_the_composer_not_over_it() {
    // a short window, like a small window beside a terminal
    let mut h = Ui1::sized(1000.0, 560.0);
    h.click("general");
    let button = h.rect("Emoji");
    h.click("Emoji");
    h.settle();
    // the first and last emoji of the grid both sit above the button
    for e in ["\u{1F44D}", "\u{1F355}"] {
        let r = h.rect(e);
        assert!(
            r.bottom() <= button.top() && r.top() >= 0.0,
            "emoji {:?} at {:?} covers the composer button {:?}",
            e,
            r,
            button
        );
    }
    h.click("\u{1F355}");
    assert_eq!(h.app.chat.text, "\u{1F355}");
}

#[test]
fn replies_show_chat_replies_and_mark_one_read() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.name == "general")
        .unwrap();
    let mut m = c.messages[0].clone();
    m.id = 9001;
    m.author = me;
    m.text = "Who has the keys?".into();
    let mut r = m.clone();
    r.id = 9002;
    r.author = 3;
    r.text = "I do".into();
    r.ts = now_ts();
    r.replies = vec![];
    m.replies = vec![r];
    c.messages.push(m);
    h.app.data.extra.replies_seen = now_ts() - 100;
    let ch = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    h.click("Replies");
    assert!(h.has("Mark read I do"));
    h.click("Mark read I do");
    assert!(h
        .app
        .data
        .extra
        .replies_read
        .contains(&format!("chat:{}:9002", ch)));
    assert!(!h.has("Mark read I do"));
}

#[test]
fn enter_in_the_middle_sends_without_a_line_break_and_shift_enter_keeps_one() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("Message composer");
    h.type_text("helloworld");
    for _ in 0..5 {
        h.key(egui::Key::ArrowLeft);
    }
    h.key(egui::Key::Enter);
    let last = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .text
        .clone();
    assert_eq!(last, "helloworld", "the cursor's spot gets no line break");
    // Shift+Enter is still a new line
    h.click("Message composer");
    h.type_text("one");
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: shift,
    }]);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: shift,
    }]);
    h.type_text("two");
    assert_eq!(h.app.chat.text, "one\ntwo");
}

#[test]
fn others_never_see_a_private_channel_even_when_it_was_left_open() {
    let mut h = Ui1::new();
    let secret = h.app.create_channel("hush", true, false);
    h.settle();
    assert!(h.has("hush"));
    // Sam signs in on this computer with the private channel still open
    h.app.data.me = 3;
    h.app.section = Section::Chat;
    h.app.chat.open = Some(secret);
    h.settle();
    assert!(!h.has("hush"), "not in the sidebar");
    assert_ne!(
        h.app.chat.open,
        Some(secret),
        "the chat moved to a channel Sam may see"
    );
    // and my DM with Alex isn't Sam's either
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .clone();
    assert!(!h.app.listed(&dm));
}

#[test]
fn a_message_from_a_clock_ahead_is_read_once() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let ahead = now_ts() + 3600;
    let id = h.app.data.new_id();
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap();
    let mut m = c.messages[0].clone();
    m.id = id;
    m.author = 2;
    m.ts = ahead;
    c.messages.push(m);
    h.settle();
    let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert!(c.last_read >= ahead, "read up to the newest message");
    assert_eq!(c.messages.iter().filter(|m| m.ts > c.last_read).count(), 0);
}

#[test]
fn delete_channel_asks_first_and_close_conversation_only_hides() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    h.click("Delete channel");
    assert!(
        h.app.data.channels.iter().any(|c| c.id == cid),
        "one click doesn't delete"
    );
    h.click("Cancel");
    assert!(h.app.data.channels.iter().any(|c| c.id == cid));
    h.click("Delete channel");
    h.click("Delete");
    assert!(
        !h.app.data.channels.iter().any(|c| c.id == cid),
        "deleted after saying yes"
    );
    // a DM is only closed for me
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    h.app.modal = Some(Modal::ChannelSettings(dm));
    h.settle();
    h.click("Close conversation");
    assert!(h.app.data.channels.iter().any(|c| c.id == dm));
    assert!(!h.has("Alex Kim"));
}

#[test]
fn mentions_of_names_with_spaces_are_marked_whole() {
    let h = Ui1::new();
    let job = h
        .app
        .mention_text("hi @Alex Kim and @Sam Lee, also @someone", 14.0);
    let t = h.app.t();
    let marked: Vec<&str> = job
        .sections
        .iter()
        .filter(|s| s.format.background == t.selected_bg)
        .map(|s| &job.text[s.byte_range.clone()])
        .collect();
    assert_eq!(marked, vec!["@Alex Kim", "@Sam Lee", "@someone"]);
}

#[test]
fn a_deleted_channel_goes_to_trash_and_comes_back_with_its_messages() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    let messages = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    h.click("Delete channel");
    h.click("Delete");
    assert!(!h.app.data.channels.iter().any(|c| c.id == cid));
    assert!(h
        .app
        .data
        .extra
        .trash
        .iter()
        .any(|t| t.kind == "Channel" && t.title == "#website-redesign"));
    h.app.modal = Some(Modal::Trash);
    h.settle();
    h.click("Restore");
    let back = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .expect("restored");
    assert_eq!(back.messages.len(), messages);
}

#[test]
fn someone_elses_channel_can_be_joined_but_not_deleted_or_its_people_changed() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .creator = 2;
    // a plain member (an owner or admin may manage anyone's channel)
    let me = h.app.data.me;
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == me)
        .unwrap()
        .role = "Member".into();
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    // Alex Kim (2) is in it and Sam Lee (3) isn't: neither checkbox changes
    h.click("Sam Lee");
    h.click("Alex Kim");
    assert_eq!(
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .members,
        vec![1, 2]
    );
    h.click("Delete channel");
    assert!(!h.has("Delete"), "the delete button is off");
    assert!(h.app.data.channels.iter().any(|c| c.id == cid));
}

#[test]
fn saved_messages_from_every_channel_open_from_the_sidebar() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages[0]
        .saved = true;
    h.settle();
    h.click("Saved messages");
    assert!(matches!(h.app.modal, Some(Modal::SavedMessages)));
    h.click("Saved message");
    assert_eq!((h.app.section, h.app.chat.open), (Section::Chat, Some(cid)));
    assert!(h.app.modal.is_none());
}

#[test]
fn add_channel_doesnt_open_someone_elses_private_channel() {
    let mut h = Ui1::new();
    let list = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .find(|l| l.name == "Marketing")
        .unwrap()
        .id;
    let secret = h.app.data.new_id();
    h.app.data.channels.push(Channel {
        id: secret,
        name: "marketing".into(),
        kind: ChannelKind::Channel,
        members: vec![2],
        private: true,
        ..Default::default()
    });
    h.app.apply(Action::Go(Location::List(list)));
    h.settle();
    h.click("Add Channel");
    let open = h.app.chat.open.unwrap();
    assert_ne!(open, secret);
    assert!(h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == open)
        .is_some_and(|c| c.name == "marketing" && !c.private));
}

#[test]
fn click_the_channel_bell_to_choose_what_it_notifies() {
    let mut h = Ui1::new();
    let g = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    h.app.section = Section::Chat;
    h.app.chat.open = Some(g);
    h.settle();
    h.click("Channel notifications");
    h.click("Every new message");
    assert_eq!(h.app.data.extra.notify.channel_mode(g), "all");
    h.click("Channel notifications");
    h.click("Nothing");
    assert_eq!(h.app.data.extra.notify.channel_mode(g), "nothing");
}

#[test]
fn click_a_notification_toast_to_open_the_conversation() {
    let mut h = Ui1::new();
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    // a teammate's message arrives, as a merge would bring it
    let id = h.app.data.new_id();
    let m: Message = serde_json::from_value(
        serde_json::json!({"id": id, "author": 2, "text": "Got a minute?", "ts": now_ts()}),
    )
    .unwrap();
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == dm)
        .unwrap()
        .messages
        .push(m);
    h.app.notify_rt.pending = true;
    h.settle();
    let toast = h.app.toast.clone().expect("a toast for the new message").0;
    assert!(toast.contains("Got a minute?"), "{}", toast);
    assert_ne!(h.app.section, Section::Chat);
    h.click(&format!("Open {}", toast));
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat.open, Some(dm));
}

#[test]
fn an_owner_without_a_server_manages_someone_elses_channel() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .creator = 2;
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    // the Sam Lee checkbox in the dialog, not a name on the page behind it
    let dialog = h.modal_rect();
    let sam = h.rect_in("Sam Lee", dialog);
    h.click_at(sam.center());
    assert!(h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .members
        .contains(&3));
}

#[test]
fn a_reply_notice_from_a_duplicate_dm_copy_opens_its_thread() {
    let mut h = Ui1::new();
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .clone();
    // the shown copy has a message; a second copy (made on another
    // computer at the same time) holds a thread with a reply
    let first = h.app.data.new_id();
    let mut primary = dm.clone();
    primary.messages.push(
        serde_json::from_value(
            serde_json::json!({"id": first, "author": 1, "text": "hello", "ts": now_ts() - 100}),
        )
        .unwrap(),
    );
    let pid = primary.id;
    *h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == pid)
        .unwrap() = primary;
    let dup_id = h.app.data.new_id();
    let parent = h.app.data.new_id();
    let reply = h.app.data.new_id();
    let mut dup = dm.clone();
    dup.id = dup_id;
    dup.messages = vec![serde_json::from_value(serde_json::json!({
        "id": parent, "author": 2, "text": "question in the other copy", "ts": now_ts() - 50,
        "replies": [{"id": reply, "author": 2, "text": "the answer", "ts": now_ts() - 10}]
    }))
    .unwrap()];
    h.app.data.channels.push(dup);
    h.app.open_target(&format!("chat:{}:{}", dup_id, parent));
    h.settle();
    assert_eq!(h.app.chat.open, Some(pid), "the shown copy opens");
    assert_eq!(
        h.app.chat.thread_open,
        Some(parent),
        "with the thread still open"
    );
    assert!(h.has("Thread"), "the thread panel is on screen");
}

#[test]
fn a_chat_mention_picked_from_the_list_means_that_person_when_two_share_a_name() {
    let mut h = Ui1::new();
    let sam = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Sam Lee")
        .unwrap()
        .id;
    let sam2 = h.app.data.new_id();
    h.app.data.members.push(Member {
        id: sam2,
        name: "Sam Lee".into(),
        color: [1, 2, 3],
        email: "sam2@example.com".into(),
        role: "Member".into(),
        invited: false,
        status_text: String::new(),
        slack: vec![],
        slack_mine: vec![],
        ..Default::default()
    });
    h.click("general");
    let before: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Sam Lee")
        .map(|(_, r)| *r)
        .collect();
    h.click("Mention someone");
    // the list's rows: the second Sam Lee is the lower one
    let mut rows: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Sam Lee" && !before.contains(r))
        .map(|(_, r)| *r)
        .collect();
    rows.sort_by(|a, b| a.center().y.total_cmp(&b.center().y));
    rows.dedup();
    assert_eq!(rows.len(), 2, "both are offered");
    h.click_at(rows[1].center());
    assert_eq!(h.app.chat.text, "@Sam Lee ");
    h.type_text("can you look?");
    h.key(egui::Key::Enter);
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap();
    let m = general.messages.last().unwrap();
    assert_eq!(m.text, "@Sam Lee can you look?");
    assert_eq!(m.mentions, vec![sam2], "the one picked, not both");
    assert!(!m.mentions.contains(&sam));
    assert!(h.app.chat.picked.is_empty(), "used up by the message");
    // typed by hand, without the list: both (it can't tell which)
    h.type_text("@Sam Lee again");
    h.key(egui::Key::Enter);
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap();
    let mut both = general.messages.last().unwrap().mentions.clone();
    both.sort();
    let mut want = vec![sam, sam2];
    want.sort();
    assert_eq!(both, want);
}

#[test]
fn a_thread_reply_keeps_the_same_named_person_picked_from_its_mention_list() {
    let mut h = Ui1::new();
    let sam = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Sam Lee")
        .unwrap()
        .id;
    let sam2 = h.app.data.new_id();
    h.app.data.members.push(Member {
        id: sam2,
        name: "Sam Lee".into(),
        color: [1, 2, 3],
        email: "sam2@example.com".into(),
        role: "Member".into(),
        ..Default::default()
    });
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("1 reply");
    let mid = h.app.chat.thread_open.unwrap();
    let before: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Sam Lee")
        .map(|(_, r)| *r)
        .collect();
    h.click("Mention someone in the reply");
    let mut rows: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Sam Lee" && !before.contains(r))
        .map(|(_, r)| *r)
        .collect();
    rows.sort_by(|a, b| a.center().y.total_cmp(&b.center().y));
    rows.dedup();
    assert_eq!(rows.len(), 2, "both are offered in the thread's list");
    h.click_at(rows[1].center());
    assert_eq!(h.app.chat.thread_text, "@Sam Lee ");
    assert!(h.app.chat.text.is_empty(), "not into the channel's box");
    assert_eq!(h.app.chat.thread_picked, vec![sam2]);
    h.type_text("see this");
    h.key(egui::Key::Enter);
    let reply = |h: &Ui1| {
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .messages
            .iter()
            .find(|m| m.id == mid)
            .unwrap()
            .replies
            .last()
            .unwrap()
            .clone()
    };
    let r = reply(&h);
    assert_eq!(r.text, "@Sam Lee see this");
    assert_eq!(r.mentions, vec![sam2], "the one picked, not both");
    assert!(h.app.chat.thread_picked.is_empty(), "used up by the reply");
    // typing "@" opens the same list
    h.click("Reply composer");
    h.type_text("@");
    let rows: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Sam Lee" && !before.contains(r))
        .map(|(_, r)| *r)
        .collect();
    assert!(rows.len() >= 2, "the @ list opens on typing @");
    let first = rows
        .iter()
        .min_by(|a, b| a.center().y.total_cmp(&b.center().y))
        .unwrap();
    h.click_at(first.center());
    assert_eq!(
        h.app.chat.thread_text, "@Sam Lee ",
        "the typed @ is replaced"
    );
    h.type_text("too");
    h.key(egui::Key::Enter);
    assert_eq!(reply(&h).mentions, vec![sam]);
}

#[test]
fn hover_toolbar_floats_above_a_one_line_message() {
    // a follow-up message is one short row; the toolbar sits over its top
    // edge instead of on top of the row
    let (mut h, _) = general_with_my_message("first of two");
    h.click("Message composer");
    h.type_text("ok then");
    h.key(egui::Key::Enter);
    let p = h.find("ok then").unwrap();
    h.hover(p);
    h.settle();
    let text = h.painted_text("ok then")[0].0;
    let more = h.rect("More actions");
    assert!(
        more.bottom() < text.center().y,
        "the toolbar covers the row: buttons end at {} below the text's middle {}",
        more.bottom(),
        text.center().y
    );
    // on its way up to the toolbar the pointer crosses into the row above;
    // the toolbar stays, and that row doesn't show a second one
    h.hover(pos2(more.center().x, more.top() + 2.0));
    h.settle();
    assert_eq!(h.count_named("More actions"), 1);
    assert_eq!(h.rect("More actions"), more, "the same toolbar stays put");
}

/// What winit sends while a Korean syllable is put together and finished.
fn preedit(s: &str) -> Vec<egui::Event> {
    vec![
        egui::Event::Ime(egui::ImeEvent::Enabled),
        egui::Event::Ime(egui::ImeEvent::Preedit(s.into())),
    ]
}

fn finished(s: &str) -> Vec<egui::Event> {
    use egui::{Event::Ime, ImeEvent::*};
    vec![
        Ime(Preedit(s.into())),
        Ime(Disabled),
        Ime(Commit(s.into())),
        Ime(Disabled),
        Ime(Disabled),
    ]
}

fn backspace(pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Backspace,
        physical_key: Some(egui::Key::Backspace),
        pressed,
        repeat: false,
        modifiers,
    }
}

#[test]
fn korean_typing_keeps_backspace_cmd_backspace_and_fast_keys() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Message composer");
    let none = egui::Modifiers::NONE;
    // a Backspace in the same frame as the end of a syllable deletes it
    h.frame(preedit("안"));
    let mut ev = finished("안");
    ev.extend([backspace(true, none), backspace(false, none)]);
    h.frame(ev);
    h.settle();
    assert_eq!(h.app.chat.text, "");

    // Cmd+Backspace while a syllable is unfinished: macOS finishes it and
    // passes on only the release of the key; the line still goes
    h.type_text("hi ");
    h.frame(preedit("녕"));
    h.frame(finished("녕"));
    // (as the Mac sends Cmd)
    let cmd = egui::Modifiers {
        mac_cmd: true,
        command: true,
        ..Default::default()
    };
    h.frame(vec![backspace(false, cmd)]);
    h.settle();
    assert_eq!(h.app.chat.text, "");

    // fast typing: a space and the next syllable in the same frame as the
    // end of the last one stay in order
    h.frame(preedit("가"));
    let mut ev = finished("가");
    ev.push(egui::Event::Text(" ".into()));
    ev.extend(preedit("ㄴ"));
    h.frame(ev);
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit("나".into()))]);
    h.frame(finished("나"));
    h.settle();
    assert_eq!(h.app.chat.text, "가 나");
}

#[test]
fn delete_keys_in_the_message_box_work_as_on_the_mac() {
    use egui::{Key, Modifiers};
    let mut h = Ui1::new();
    h.ctx.set_os(egui::os::OperatingSystem::Mac);
    h.click("general");
    h.click("Message composer");
    let cmd = Modifiers {
        mac_cmd: true,
        command: true,
        ..Default::default()
    };
    let opt = Modifiers {
        alt: true,
        ..Default::default()
    };
    let ctrl = Modifiers {
        ctrl: true,
        ..Default::default()
    };

    // Option+Backspace takes one Korean word, not the whole sentence
    h.type_text("안녕하세요, 반갑습니다");
    h.key_with(Key::Backspace, opt);
    assert_eq!(h.app.chat.text, "안녕하세요, ");
    // spaces and punctuation go with the word before them
    h.key_with(Key::Backspace, opt);
    assert_eq!(h.app.chat.text, "");
    h.type_text("hello world ");
    h.key_with(Key::Backspace, opt);
    assert_eq!(h.app.chat.text, "hello ");

    // Option+Delete takes the next word; Option+Left stops at a Korean word
    h.type_text("회의 메모");
    h.key_with(Key::ArrowLeft, opt);
    h.key_with(Key::ArrowLeft, opt);
    h.key_with(Key::Delete, opt);
    assert_eq!(h.app.chat.text, "hello  메모");

    // Cmd+Backspace takes the line up to the cursor; at the start of a line
    // it joins it to the line before
    h.key_with(Key::ArrowRight, cmd);
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.text, "");
    h.type_text("first line\nsecond line");
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.text, "first line\n");
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.text, "first line");

    // with a selection, Cmd+Backspace takes only the selection
    h.key_with(Key::ArrowLeft, opt | Modifiers::SHIFT);
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.text, "first ");

    // Cmd+Delete takes the rest of the line
    h.type_text("one two");
    h.key_with(Key::ArrowLeft, opt);
    h.key_with(Key::Delete, cmd);
    assert_eq!(h.app.chat.text, "first one ");

    // Ctrl+Backspace and Ctrl+D take one letter each
    h.type_text("xyz");
    h.key_with(Key::Backspace, ctrl);
    assert_eq!(h.app.chat.text, "first one xy");
    h.key_with(Key::ArrowLeft, Modifiers::NONE);
    h.key_with(Key::D, ctrl);
    assert_eq!(h.app.chat.text, "first one x");

    // a long line wraps: Cmd+Backspace takes only the row the cursor is on
    h.key_with(Key::ArrowRight, cmd);
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.text, "");
    let long = "word ".repeat(80);
    h.type_text(&long);
    h.key_with(Key::Backspace, cmd);
    let left = h.app.chat.text.clone();
    assert!(
        !left.is_empty() && left.len() < long.len() && long.starts_with(&left),
        "only the last row went: {left:?}"
    );
}

#[test]
fn a_sign_typed_right_after_a_korean_syllable_is_kept() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Message composer");
    h.type_text("안녕하세");
    h.frame(preedit("요"));
    // macOS finishes "요" and then commits "!" too, in one key press
    use egui::{Event::Ime, ImeEvent::*};
    h.frame(vec![
        Ime(Disabled),
        Ime(Commit("요".into())),
        Ime(Commit("!".into())),
    ]);
    h.settle();
    assert_eq!(h.app.chat.text, "안녕하세요!");
}

/// Sending from higher up a long conversation goes down to the message just
/// sent, in the channel and in a thread.
#[test]
fn sending_from_higher_up_goes_down_to_the_new_message() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let ch = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap();
    for i in 0..60 {
        ch.messages.push(crate::app::slack::new_message(
            910_000 + i as u64,
            2,
            format!("Filler message {}", i),
            1_600_000_000 + i as i64 * 60,
        ));
    }
    ch.messages.sort_by_key(|m| m.ts);
    let parent = ch.messages.last().unwrap().id;
    let m = ch.messages.last_mut().unwrap();
    for i in 0..40 {
        m.replies.push(crate::app::slack::new_message(
            920_000 + i as u64,
            2,
            format!("Filler reply {}", i),
            1_700_000_000 + i as i64 * 60,
        ));
    }
    h.settle();
    // up the list with the wheel, then send
    let up = |h: &mut Ui1, at: Pos2| {
        h.frame(vec![egui::Event::PointerMoved(at)]);
        for _ in 0..4 {
            h.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, 20_000.0),
                modifiers: Default::default(),
            }]);
        }
        for _ in 0..30 {
            h.frame(vec![]);
        }
    };
    let on_screen =
        |h: &Ui1, name: &str, above: f32| h.find(name).map_or(false, |p| p.y > 40.0 && p.y < above);
    up(&mut h, pos2(500.0, 400.0));
    let composer = h.rect("Message composer").top();
    assert!(!on_screen(&h, "Filler message 59", composer), "scrolled up");
    h.click("Message composer");
    h.type_text("sent from up the list");
    h.key(egui::Key::Enter);
    for _ in 0..30 {
        h.frame(vec![]);
    }
    assert!(
        on_screen(&h, "sent from up the list", composer),
        "the list went down to the message just sent"
    );

    // the same in a thread
    h.app.chat.thread_open = Some(parent);
    h.settle();
    let reply_box = h.rect("Reply composer");
    up(&mut h, pos2(reply_box.center().x, 300.0));
    assert!(
        !on_screen(&h, "Filler reply 39", reply_box.top()),
        "thread scrolled up"
    );
    h.click("Reply composer");
    h.type_text("reply from up the thread");
    h.key(egui::Key::Enter);
    for _ in 0..30 {
        h.frame(vec![]);
    }
    assert!(
        on_screen(&h, "reply from up the thread", reply_box.top()),
        "the thread went down to the reply just sent"
    );
}
