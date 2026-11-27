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
    h.send_message("hello from a click test");
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
fn thread_moves_to_its_own_window_and_back() {
    let mut h = Ui1::new();
    // a second channel to go to while the thread is in its window
    let mut random = h.app.data.channels[0].clone();
    random.id = 7_777;
    random.name = "random".into();
    random.messages.clear();
    h.app.data.channels.push(random);
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("1 reply");
    let tid = h.app.chat.thread_open.unwrap();
    assert!(!h.has("Back to the sidebar"));

    h.click("Open in new window");
    assert_eq!(
        h.app.chat.thread_window,
        Some(cid),
        "the thread has its own window"
    );
    assert!(h.has("Back to the sidebar"));
    assert!(
        !h.has("Open in new window"),
        "and no panel beside the conversation"
    );

    // replies are written in the window
    h.click("Reply composer");
    h.type_text("from the window");
    h.key(egui::Key::Enter);
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let m = ch.messages.iter().find(|m| m.id == tid).unwrap();
    assert_eq!(m.replies.last().unwrap().text, "from the window");

    // another channel: the window stays with its thread
    h.click("random");
    assert_eq!(h.app.chat.open, Some(7_777));
    assert_eq!(h.app.chat.thread_open, Some(tid));
    assert!(h.has("Back to the sidebar"), "the window is still open");

    // back beside its conversation
    h.click("Back to the sidebar");
    assert_eq!(h.app.chat.thread_window, None);
    assert_eq!(h.app.chat.open, Some(cid));
    assert_eq!(h.app.chat.thread_open, Some(tid));
    assert!(h.has("Open in new window"), "the panel is back");

    // closing the window closes the thread
    h.click("Open in new window");
    h.click("Close thread");
    assert_eq!(h.app.chat.thread_open, None);
    assert_eq!(h.app.chat.thread_window, None);
    assert!(!h.has("Back to the sidebar"));
}

#[test]
fn escape_closes_the_thread() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    let tid = h.app.chat.thread_open.unwrap();
    h.key(egui::Key::Escape);
    assert_eq!(h.app.chat.thread_open, None, "the panel closes");
    assert!(!h.has("Close thread"));

    // from its reply box too, and the half-written reply waits for it
    h.click("1 reply");
    h.click("Reply composer");
    h.type_text("half a reply");
    h.key(egui::Key::Escape);
    assert_eq!(h.app.chat.thread_open, None);
    h.click("1 reply");
    assert_eq!(h.app.chat.thread_open, Some(tid));
    assert_eq!(h.app.chat.thread.text, "half a reply", "the draft is kept");

    // a menu over the thread closes first, the thread stays
    h.click("More actions");
    assert!(h.has("Remind me in 1 hour"), "the menu is open");
    h.key(egui::Key::Escape);
    assert!(!h.has("Remind me in 1 hour"), "Escape closes the menu");
    assert_eq!(h.app.chat.thread_open, Some(tid), "not the thread");

    // in its own window as well
    h.click("Open in new window");
    assert!(h.app.chat.thread_window.is_some());
    h.key(egui::Key::Escape);
    assert_eq!(h.app.chat.thread_open, None);
    assert_eq!(h.app.chat.thread_window, None);
    assert!(!h.has("Back to the sidebar"));
}

#[test]
fn escape_while_changing_a_message_in_a_thread_only_cancels_it() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    let tid = h.app.chat.thread_open.unwrap();
    h.click("Reply composer");
    h.type_text("mine");
    h.key(egui::Key::Enter);
    // my reply's menu, on its toolbar when the pointer is over it
    let mine = h.find("mine").unwrap();
    h.hover(mine);
    let more = h
        .tree
        .iter()
        .filter(|(n, _)| n == "More actions")
        .map(|(_, r)| r.center())
        .max_by(|a, b| (a.x + a.y).total_cmp(&(b.x + b.y)))
        .unwrap();
    h.click_at(more);
    h.click("Edit message");
    assert!(h.app.chat.edit.is_some(), "my reply turns into a text box");
    h.key(egui::Key::Escape);
    assert!(h.app.chat.edit.is_none(), "Escape cancels the change");
    assert_eq!(h.app.chat.thread_open, Some(tid), "the thread stays open");
}

#[test]
fn thread_header_drags_out_to_a_new_window() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    let grip = h.find("Drag the thread out to a new window").unwrap();
    // a slip of the hand leaves it where it is
    h.drag_by(grip, grip + vec2(-12.0, 4.0));
    assert_eq!(h.app.chat.thread_window, None);
    h.drag_by(grip, grip + vec2(-400.0, 80.0));
    assert!(
        h.app.chat.thread_window.is_some(),
        "dragged out: its own window"
    );
    assert!(h.has("Back to the sidebar"));
}

#[test]
fn thread_close_button_is_square_like_its_neighbours() {
    // a bare "\u{00D7}" was as narrow as the glyph but 28 tall, so its hover
    // fill was a tall pill
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    let close = h.rect("Close thread");
    let out = h.rect("Open in new window");
    assert!(close.width() >= close.height(), "{close:?}");
    assert_eq!(
        close.size(),
        out.size(),
        "same size as the button beside it"
    );
}

#[test]
fn thread_panel_is_wider_by_dragging_its_edge() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    let before = h.rect("Reply composer");
    // the panel's left edge, just left of its 14-point margin
    let edge = pos2(before.left() - 22.0, before.top() - 200.0);
    h.drag_by(edge, edge + vec2(-150.0, 0.0));
    let after = h.rect("Reply composer");
    assert!(
        after.width() > before.width() + 100.0,
        "wider: {} -> {}",
        before.width(),
        after.width()
    );
    // not so wide that the conversation has no room
    h.drag_by(pos2(after.left() - 22.0, edge.y), pos2(10.0, edge.y));
    let widest = h.rect("Reply composer");
    assert!(
        widest.left() > 600.0,
        "the conversation keeps room: {:?}",
        widest
    );
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
fn sidebar_channels_have_one_add_button() {
    // the "+" on the heading adds a channel; no "Add Channel" row under the
    // list too (two buttons for the same thing)
    let mut h = Ui1::new();
    assert!(!h.has("Sidebar Create a Channel"));
    assert!(
        h.tree
            .iter()
            .filter(|(n, r)| n == "Add Channel" && r.left() < 320.0)
            .count()
            == 0,
        "no Add Channel row in the sidebar"
    );
    let n = h.app.data.channels.len();
    h.click_side("Sidebar Add Channel", false);
    h.type_text("one button");
    h.key(egui::Key::Enter);
    assert_eq!(h.app.data.channels.len(), n + 1);
}

#[test]
fn sidebar_shows_each_button_once() {
    // each section adds from one place only, and a row's "..." leaves
    // creating to the "+" beside it
    let mut h = Ui1::new();
    assert!(!h.has("New Space"), "Spaces add from the heading's \"+\"");
    let n = h.app.data.spaces.len();
    h.click("Sidebar Create Space");
    h.type_text("Heading plus");
    h.key(egui::Key::Enter);
    assert_eq!(h.app.data.spaces.len(), n + 1);
    assert!(!h.has("Assigned to me"), "that is My Tasks itself");
    h.click("My Tasks");
    assert_eq!(h.app.loc, Location::MyTasks);
    h.hover(h.find("Team Space").unwrap());
    h.click("Space options Team Space");
    assert!(!h.has("Create new"));
    h.key(egui::Key::Escape);
    h.click("Create in Team Space");
    assert!(h.has("Folder") && h.has("From a template"));
    h.key(egui::Key::Escape);
    // the Spaces "..." keeps folding all of them, not another New Space
    h.click("Spaces settings");
    assert!(h.has("Expand all") && !h.has("Create Space"));
    h.key(egui::Key::Escape);
    // the AI row is the one way to the AI page from the sidebar
    h.click_side("Ask, Build, Create", false);
    assert_eq!(h.app.section, Section::Ai);
}

#[test]
fn sidebar_direct_messages_have_one_add_button() {
    // same for Direct Messages: the "+" on the heading, no "New message" row
    let mut h = Ui1::new();
    assert!(!h.has("New message"));
    h.click_side("Sidebar New message", false);
    assert!(h.app.new_message_open);
}

/// The sidebar's direct messages, top to bottom, by the other person's name.
fn sidebar_dm_order(h: &mut Ui1, names: &[&str]) -> Vec<String> {
    for n in names {
        h.scroll_into_view(n);
    }
    let mut v: Vec<(f32, String)> = names
        .iter()
        .map(|n| {
            // the left-most one: the conversation's header and messages are
            // to the right of the sidebar
            let r = h
                .tree
                .iter()
                .filter(|(t, _)| t == n)
                .map(|(_, r)| *r)
                .min_by(|a, b| a.left().total_cmp(&b.left()))
                .unwrap_or_else(|| panic!("no sidebar row for {}", n));
            (r.top(), n.to_string())
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v.into_iter().map(|x| x.1).collect()
}

/// Direct Messages list the conversation with the newest message first,
/// whoever wrote it, and move it up when a message comes or I send one.
#[test]
fn sidebar_direct_messages_put_the_newest_conversation_on_top() {
    let mut h = Ui1::new();
    let names = ["Alex Kim", "Sam Lee", "Jordan Park"];
    // Alex wrote ten minutes ago, Sam's has nothing yet; Jordan's is made
    // last but wrote two hours ago
    let from_jordan = |h: &mut Ui1, text: &str, ago: i64| {
        let mut m = h
            .app
            .data
            .channels
            .iter()
            .find_map(|c| c.messages.first())
            .unwrap()
            .clone();
        m.id = h.app.data.new_id();
        m.author = 4;
        m.text = text.into();
        m.ts = now_ts() - ago;
        m.replies.clear();
        m
    };
    let jordan = h.app.data.new_id();
    let old = from_jordan(&mut h, "Old news", 7200);
    h.app.data.channels.push(Channel {
        id: jordan,
        kind: ChannelKind::Direct,
        members: vec![1, 4],
        messages: vec![old],
        last_read: now_ts(),
        ..Default::default()
    });
    h.settle();
    assert_eq!(
        sidebar_dm_order(&mut h, &names),
        ["Alex Kim", "Jordan Park", "Sam Lee"],
        "newest message first, nothing yet at the end"
    );

    // Jordan writes: their conversation comes up
    let m = from_jordan(&mut h, "Are you around?", 60);
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == jordan)
        .unwrap()
        .messages
        .push(m);
    h.app.data_changed();
    h.settle();
    assert_eq!(
        sidebar_dm_order(&mut h, &names),
        ["Jordan Park", "Alex Kim", "Sam Lee"]
    );

    // I write to Sam: theirs goes on top
    h.click_side("Sam Lee", false);
    h.send_message("Lunch at noon?");
    assert_eq!(
        sidebar_dm_order(&mut h, &names),
        ["Sam Lee", "Jordan Park", "Alex Kim"]
    );
}

#[test]
fn folded_channels_open_up_for_a_new_channel() {
    // the name field sits at the end of the list, so a folded section opens
    let mut h = Ui1::new();
    h.click("Channels");
    h.settle();
    assert!(!h.has("general"));
    h.click_side("Sidebar Add Channel", false);
    h.settle();
    assert!(h.app.new_channel.is_some());
    assert!(h.has("general"), "unfolded");
    h.type_text("from folded");
    h.key(egui::Key::Enter);
    assert!(h.app.data.channels.iter().any(|c| c.name == "from-folded"));
}

#[test]
fn sidebar_add_channel_takes_enter_and_lets_go_on_escape() {
    let mut h = Ui1::new();
    let n = h.app.data.channels.len();
    h.click_side("Sidebar Add Channel", false);
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
    h.click_side("Sidebar Add Channel", false);
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
    assert_eq!(
        h.app.chat.channel.text, "",
        "the other channel starts empty"
    );
    h.send_message("first message");
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
    assert_eq!(h.app.chat.channel.text, "hello");
}

#[test]
fn new_channel_field_comes_into_view_in_a_long_sidebar() {
    // (+ Create > Channel now opens the Create Channel window; the sidebar's
    // "+" on the Channels heading opens the field at the end of the list)
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
            // the "+" on the Channels heading, scrolled to with the wheel
            h.click_side("Sidebar Add Channel", false);
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
            r.top() >= 40.0 && r.bottom() <= h.size.y - 10.0,
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
    let ch = h.channel_id("website-redesign");
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
    h.click("Tab List");
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
fn emptying_a_message_and_saving_deletes_it() {
    let (mut h, cid) = general_with_my_message("wrong words");
    h.click("More actions");
    h.click("Edit message");
    h.click("Edit message");
    for _ in 0.."wrong words".len() {
        h.key(egui::Key::Backspace);
    }
    assert!(
        h.has("Empty: saving deletes the message"),
        "the box says what Save will do"
    );
    h.click("Delete message");
    assert!(
        general_msg(&h, cid, "wrong words").is_some(),
        "the first Save only asks"
    );
    assert!(h.has("Press Enter again to delete, Esc to cancel"));
    // Escape backs out and leaves the message alone
    h.click("Edit message");
    h.key(egui::Key::Escape);
    assert!(h.app.chat.edit.is_none());
    assert!(general_msg(&h, cid, "wrong words").is_some());
    // again, with Enter twice
    let p = h.find("wrong words").unwrap();
    h.hover(p);
    h.click("More actions");
    h.click("Edit message");
    h.click("Edit message");
    for _ in 0.."wrong words".len() {
        h.key(egui::Key::Backspace);
    }
    assert!(!h.has("Press Enter again to delete, Esc to cancel"));
    h.key(egui::Key::Enter);
    assert!(general_msg(&h, cid, "wrong words").is_some());
    h.key(egui::Key::Enter);
    assert!(h.app.chat.edit.is_none());
    assert!(
        general_msg(&h, cid, "wrong words").is_none(),
        "the second Enter deletes it"
    );
}

#[test]
fn emptying_a_picture_message_keeps_the_picture() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let mut m = crate::app::slack::new_message(
        h.app.data.new_id(),
        h.app.data.me,
        "look at this".into(),
        now_ts(),
    );
    m.attachments.push(Attachment {
        file: "no-such-file".into(),
        name: "sky.png".into(),
        kind: MediaKind::Image,
        bytes: 10,
        w: 400,
        h: 300,
        secs: 0.0,
        thumb: None,
    });
    let mid = m.id;
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .push(m);
    h.settle();
    let p = h.find("look at this").unwrap();
    h.hover(p);
    h.click("More actions");
    h.click("Edit message");
    h.click("Edit message");
    for _ in 0.."look at this".len() {
        h.key(egui::Key::Backspace);
    }
    assert!(!h.has("Empty: saving deletes the message"));
    h.key(egui::Key::Enter);
    let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let m = c
        .messages
        .iter()
        .find(|m| m.id == mid)
        .expect("still there");
    assert_eq!(m.text, "", "the text is emptied");
    assert_eq!(m.attachments.len(), 1, "the picture stays");
    assert!(m.edited.is_some());
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
    // the picker's entry, right of a sidebar row of the same name
    h.click_side("Alex Kim", true);
    assert_eq!(h.app.chat.channel.text, "@Alex Kim ");
    h.click("Emoji");
    h.click("\u{1F525}");
    assert_eq!(h.app.chat.channel.text, "@Alex Kim \u{1F525}");
    // typing "@" opens the list by itself and the pick replaces the "@"
    h.type_text(" @");
    assert!(h.has("Jordan Park"), "typing @ offers people");
    // (the right-most "Jordan Park" is the list's row, not a name in the feed)
    h.click_side("Jordan Park", true);
    assert_eq!(h.app.chat.channel.text, "@Alex Kim \u{1F525} @Jordan Park ");
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
    assert_eq!(h.app.chat.channel.text, "\u{1F355}");
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
    let ch = h.channel_id("general");
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
    assert_eq!(h.app.chat.channel.text, "one\ntwo");
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
    let cid = h.channel_id("website-redesign");
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

/// Channel settings: Workflows opens from a full-width row that says what
/// it is for (not a "Workflows..." button that read as cut-off text in
/// Korean), and Delete channel sits on its own under Mark as read.
#[test]
fn channel_settings_open_workflows_from_a_row_and_keep_delete_apart() {
    let mut h = Ui1::new();
    let cid = h.channel_id("website-redesign");
    h.app.modal = Some(Modal::ChannelSettings(cid));
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    h.settle();
    let row = h.rect("Workflows");
    let title = h.painted_text("워크플로");
    let note = h.painted_text("메시지가 올라오면 정해 둔 일을 자동으로 실행");
    let dots = h.painted_text("워크플로...");
    let read = h.rect("읽음으로 표시");
    let delete = h.rect("채널 삭제");
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert!(dots.is_empty(), "no trailing dots");
    assert!(
        title.iter().any(|(r, _)| row.contains(r.center())),
        "the name is in the row"
    );
    assert!(
        note.iter().any(|(r, _)| row.contains(r.center())),
        "with a line on what it is for"
    );
    assert!(
        row.width() > read.width() * 2.0,
        "the row is as wide as the dialog"
    );
    assert!(read.top() > row.bottom() && delete.top() > read.bottom());
    h.click("Workflows");
    assert!(matches!(h.app.modal, Some(Modal::Workflows { .. })));
}

#[test]
fn mentions_of_names_with_spaces_are_marked_whole() {
    let h = Ui1::new();
    let text = "hi @Alex Kim and @Sam Lee, also @someone";
    let marked: Vec<&str> = h
        .app
        .mention_ranges(text)
        .into_iter()
        .map(|(a, b)| &text[a..b])
        .collect();
    assert_eq!(marked, vec!["@Alex Kim", "@Sam Lee", "@someone"]);
}

/// The parts of the painted text `whole` drawn in the mention tint.
fn tinted_parts(h: &Ui1, whole: &str) -> Vec<String> {
    let (fg, bg) = h.app.t().mention();
    let mut out = vec![];
    each_text(&h.shapes, |ts, _| {
        let g = &ts.galley;
        if g.text() != whole {
            return;
        }
        for s in &g.job.sections {
            if s.format.color == fg && s.format.background == bg {
                out.push(g.job.text[s.byte_range.clone()].to_string());
            }
        }
    });
    out
}

#[test]
fn a_mention_is_tinted_while_it_is_typed() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Message composer");
    h.type_text("hi @Alex Kim and @Sam Lee");
    assert_eq!(
        tinted_parts(&h, "hi @Alex Kim and @Sam Lee"),
        vec!["@Alex Kim", "@Sam Lee"],
        "the channel's box tints the names, not the words around them"
    );
    // once sent it keeps the same tint
    h.key(egui::Key::Enter);
    assert_eq!(
        tinted_parts(&h, "hi @Alex Kim and @Sam Lee"),
        vec!["@Alex Kim", "@Sam Lee"]
    );
    // a task's comment box too
    let mut h = Ui1::new();
    open_design_task(&mut h);
    h.click("Comment box");
    h.type_text("@Alex Kim please check");
    assert_eq!(
        tinted_parts(&h, "@Alex Kim please check"),
        vec!["@Alex Kim"]
    );
}

#[test]
fn a_deleted_channel_goes_to_trash_and_comes_back_with_its_messages() {
    let mut h = Ui1::new();
    let cid = h.channel_id("website-redesign");
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
fn only_the_workspace_owner_renames_a_channel_on_a_server() {
    let mut h = Ui1::new();
    let cid = h.channel_id("website-redesign");
    let name_of = |h: &Ui1| {
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .name
            .clone()
    };
    // a channel I made, on a team server
    let me = h.app.data.me;
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .creator = me;
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    // a plain member or an admin: the name can't be typed into
    for role in [wb_net::Role::Member, wb_net::Role::Admin] {
        h.app.sync_rt.role = Some(role);
        h.app.modal = Some(Modal::ChannelSettings(cid));
        h.settle();
        h.click("Channel name");
        h.key(egui::Key::End);
        h.type_text("-x");
        assert_eq!(name_of(&h), "website-redesign", "{:?} renamed it", role);
        h.key(egui::Key::Escape);
        h.app.modal = None;
    }
    // the owner
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    h.click("Channel name");
    h.key(egui::Key::End);
    h.type_text("-x");
    assert_eq!(name_of(&h), "website-redesign-x");
}

/// #general is a default channel from the start (the server announces
/// someone joining there); in Channel settings whoever made a channel, or
/// an admin, turns it on or off, and anyone else finds the box greyed out.
/// A private channel has no such box.
#[test]
fn a_channels_maker_or_an_admin_makes_it_a_default_channel() {
    let mut h = Ui1::new();
    let general = h.channel_id("general");
    let cid = h.channel_id("website-redesign");
    let ch = |h: &Ui1, id: u64| {
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .clone()
    };
    assert!(ch(&h, general).is_default());
    assert!(!ch(&h, cid).is_default());
    // Alex's channel, on a team server, and I'm a plain member
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .creator = 2;
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    // the pointer is on the window, as it is for a person
    h.hover(egui::pos2(10.0, 10.0));
    assert!(h.has("When someone joins the workspace, it's announced here."));
    h.click("Default channel");
    assert_eq!(ch(&h, cid).default_channel, None, "a plain member can't");
    h.key(egui::Key::Escape);
    // an admin can
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    h.click("Default channel");
    assert_eq!(ch(&h, cid).default_channel, Some(true));
    assert!(ch(&h, cid).is_default());
    h.key(egui::Key::Escape);
    // and turn #general off: it stays off
    h.app.modal = Some(Modal::ChannelSettings(general));
    h.settle();
    h.click("Default channel");
    assert_eq!(ch(&h, general).default_channel, Some(false));
    assert!(!ch(&h, general).is_default());
    h.key(egui::Key::Escape);
    // a private channel is never one
    let secret = h.app.create_channel("hush", true, false);
    h.app.modal = Some(Modal::ChannelSettings(secret));
    h.settle();
    assert!(h.has("Members"));
    assert!(!h.has("Default channel"));
}

/// What the server posts for someone who joined (`event` "joined") reads
/// in my language; the same words typed by someone stay as they were.
#[test]
fn a_join_message_from_the_server_reads_in_my_language() {
    let mut m: crate::model::Message = serde_json::from_value(serde_json::json!({
        "id": 1, "author": 2, "text": "Hi! I just joined the workspace.", "ts": 1, "event": "joined"
    }))
    .unwrap();
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    assert_eq!(
        crate::app::chat::shown_text(&m),
        "안녕하세요! 워크스페이스에 새로 가입했습니다."
    );
    m.text = "Joined the workspace.".into();
    assert_eq!(
        crate::app::chat::shown_text(&m),
        "워크스페이스에 들어왔습니다."
    );
    m.event = None;
    assert_eq!(crate::app::chat::shown_text(&m), "Joined the workspace.");
    crate::i18n::set_lang(crate::i18n::Lang::En);
}

#[test]
fn someone_elses_channel_can_be_joined_but_not_deleted_or_its_people_changed() {
    let mut h = Ui1::new();
    let cid = h.channel_id("website-redesign");
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
    let cid = h.channel_id("website-redesign");
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
    let g = h.channel_id("general");
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
    let cid = h.channel_id("website-redesign");
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
    assert_eq!(h.app.chat.channel.text, "@Sam Lee ");
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
    assert!(
        h.app.chat.channel.picked.is_empty(),
        "used up by the message"
    );
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
    assert_eq!(h.app.chat.thread.text, "@Sam Lee ");
    assert!(
        h.app.chat.channel.text.is_empty(),
        "not into the channel's box"
    );
    assert_eq!(h.app.chat.thread.picked, vec![sam2]);
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
    assert!(h.app.chat.thread.picked.is_empty(), "used up by the reply");
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
        h.app.chat.thread.text, "@Sam Lee ",
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
    h.send_message("ok then");
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
    assert_eq!(h.app.chat.channel.text, "");

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
    assert_eq!(h.app.chat.channel.text, "");

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
    assert_eq!(h.app.chat.channel.text, "가 나");
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
    assert_eq!(h.app.chat.channel.text, "안녕하세요, ");
    // spaces and punctuation go with the word before them
    h.key_with(Key::Backspace, opt);
    assert_eq!(h.app.chat.channel.text, "");
    h.type_text("hello world ");
    h.key_with(Key::Backspace, opt);
    assert_eq!(h.app.chat.channel.text, "hello ");

    // Option+Delete takes the next word; Option+Left stops at a Korean word
    h.type_text("회의 메모");
    h.key_with(Key::ArrowLeft, opt);
    h.key_with(Key::ArrowLeft, opt);
    h.key_with(Key::Delete, opt);
    assert_eq!(h.app.chat.channel.text, "hello  메모");

    // Cmd+Backspace takes the line up to the cursor; at the start of a line
    // it joins it to the line before
    h.key_with(Key::ArrowRight, cmd);
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.channel.text, "");
    h.type_text("first line\nsecond line");
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.channel.text, "first line\n");
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.channel.text, "first line");

    // with a selection, Cmd+Backspace takes only the selection
    h.key_with(Key::ArrowLeft, opt | Modifiers::SHIFT);
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.channel.text, "first ");

    // Cmd+Delete takes the rest of the line
    h.type_text("one two");
    h.key_with(Key::ArrowLeft, opt);
    h.key_with(Key::Delete, cmd);
    assert_eq!(h.app.chat.channel.text, "first one ");

    // Ctrl+Backspace and Ctrl+D take one letter each
    h.type_text("xyz");
    h.key_with(Key::Backspace, ctrl);
    assert_eq!(h.app.chat.channel.text, "first one xy");
    h.key_with(Key::ArrowLeft, Modifiers::NONE);
    h.key_with(Key::D, ctrl);
    assert_eq!(h.app.chat.channel.text, "first one x");

    // a long line wraps: Cmd+Backspace takes only the row the cursor is on
    h.key_with(Key::ArrowRight, cmd);
    h.key_with(Key::Backspace, cmd);
    assert_eq!(h.app.chat.channel.text, "");
    let long = "word ".repeat(80);
    h.type_text(&long);
    h.key_with(Key::Backspace, cmd);
    let left = h.app.chat.channel.text.clone();
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
    // the Mac finishes "요" and passes on only the release of Shift+1
    h.frame(finished("요"));
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Exclamationmark,
        physical_key: Some(egui::Key::Num1),
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    }]);
    h.settle();
    assert_eq!(h.app.chat.channel.text, "안녕하세요!");
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
    h.send_message("sent from up the list");
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

#[test]
fn who_else_is_typing_shows_under_the_message_box_for_a_moment() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let me = h.app.data.me;
    let others: Vec<(u64, String)> = h
        .app
        .data
        .members
        .iter()
        .filter(|m| m.id != me)
        .map(|m| (m.id, m.shown().to_string()))
        .take(3)
        .collect();
    let (a, an) = others[0].clone();
    let (b, bn) = others[1].clone();
    let typing = |channel, member| wb_net::Event::Typing {
        channel,
        member,
        thread: None,
    };
    let line_for = |n: &str| format!("{} is typing...", n);

    // nobody yet: the row under the box is empty
    let box_at = h.rect("Message composer");
    assert!(!h.tree.iter().any(|(n, _)| n.ends_with("typing...")));

    // the server says one teammate is typing here
    h.app.take_net_events(vec![typing(cid, a)]);
    h.settle();
    assert!(h.has(&line_for(&an)), "their name and 'is typing...'");
    assert_eq!(
        h.rect("Message composer"),
        box_at,
        "the message box doesn't move"
    );

    // a second one: both names; a third: "Several people"
    h.app.take_net_events(vec![typing(cid, b)]);
    h.settle();
    assert!(h.has(&format!("{} and {} are typing...", an, bn)));
    h.app.take_net_events(vec![typing(cid, others[2].0)]);
    h.settle();
    assert!(h.has("Several people are typing..."));

    // my own typing (from my other computer) never shows; nor in another channel
    h.app.take_net_events(vec![typing(cid, me)]);
    h.settle();
    assert!(h.has("Several people are typing..."));
    assert!(!h.app.sync_rt.typing.iter().any(|t| t.member == me));

    // the first one's message arrives: they're done
    let now = chrono::Utc::now().timestamp();
    let copy = teammate_change(&h.app.data, |d| {
        let c = d.channels.iter_mut().find(|c| c.id == cid).unwrap();
        c.messages.push(crate::app::slack::new_message(
            990_001,
            a,
            "done typing".into(),
            now,
        ));
    });
    h.app.take_net_events(vec![wb_net::Event::Update {
        data: copy,
        rev: 1,
        hidden: vec![],
    }]);
    h.settle();
    let cn = &others[2].1;
    assert!(
        h.has(&format!("{} and {} are typing...", bn, cn)),
        "the other two still are"
    );

    // nothing heard for a few seconds: gone
    for t in h.app.sync_rt.typing.iter_mut() {
        t.heard -= crate::app::typing::TYPING_SHOWS;
    }
    h.settle();
    assert!(!h.has(&line_for(&bn)));
    assert!(h.app.sync_rt.typing.is_empty(), "forgotten");

    // in Korean
    h.app.take_net_events(vec![typing(cid, a)]);
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    h.settle();
    let ko = format!("{}님이 입력 중...", an);
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert!(h.has(&ko));
}

#[test]
fn in_a_direct_message_the_other_person_typing_shows_under_the_box_and_as_dots_in_the_sidebar() {
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
    let name = h.app.data.member(2).unwrap().shown().to_string();
    let line = format!("{} is typing...", name);
    let box_at = h.rect("Message composer");
    // their row in the sidebar's Direct Messages (left of the conversation)
    let row = h
        .tree
        .iter()
        .find(|(n, r)| n == &name && r.right() < box_at.left())
        .map(|(_, r)| *r)
        .expect("their sidebar row");
    assert!(!h.has(&line));

    h.app.take_net_events(vec![wb_net::Event::Typing {
        channel: dm,
        member: 2,
        thread: None,
    }]);
    h.settle();
    let shown: Vec<Rect> = h
        .tree
        .iter()
        .filter(|(n, _)| n == &line)
        .map(|(_, r)| *r)
        .collect();
    assert_eq!(shown.len(), 2, "in the sidebar and under the box");
    assert!(
        shown
            .iter()
            .any(|d| row.contains_rect(*d) && d.left() > row.left() + 30.0),
        "the dots sit after their name in the sidebar"
    );
    assert!(
        shown
            .iter()
            .any(|l| l.top() >= box_at.bottom() - 1.0 && l.left() >= box_at.left() - 30.0),
        "the line sits under the message box, seen when the sidebar is hidden too"
    );
    assert_eq!(h.rect("Message composer"), box_at);

    // nothing heard for a few seconds: gone
    for t in h.app.sync_rt.typing.iter_mut() {
        t.heard -= crate::app::typing::TYPING_SHOWS;
    }
    h.settle();
    assert!(!h.has(&line));
}

#[test]
fn typing_in_the_message_box_tells_the_server_every_few_seconds() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    // not on a server: nothing to tell
    h.click("Message composer");
    h.type_text("h");
    assert_eq!(h.app.sync_rt.typed, None);
    // connected (to a server that isn't there, so nothing really goes out)
    h.app.sync_rt.net = Some(wb_net::Client::start(
        "127.0.0.1:1",
        wb_net::SignIn::default(),
        std::sync::Arc::new(|| {}),
    ));
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.type_text("i");
    let first = h.app.sync_rt.typed.expect("told the server");
    assert_eq!((first.0, first.1), (cid, None));
    // more keys right after: not again
    h.type_text(" there");
    assert_eq!(h.app.sync_rt.typed.unwrap().2, first.2);
    // a few seconds on, the next key tells it again
    h.app.sync_rt.typed = Some((cid, None, first.2 - crate::app::typing::TYPING_EVERY));
    h.type_text("!");
    assert!(h.app.sync_rt.typed.unwrap().2 > first.2);
    h.app.sync_rt.net = None;
}

#[test]
fn typing_in_a_thread_shows_only_under_that_threads_reply_box() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("1 reply");
    let tid = h.app.chat.thread_open.unwrap();
    let name = h.app.data.member(2).unwrap().shown().to_string();
    let line = format!("{} is typing...", name);

    // the server says Alex is answering in this thread
    h.app.take_net_events(vec![wb_net::Event::Typing {
        channel: cid,
        member: 2,
        thread: Some(tid),
    }]);
    h.settle();
    let at = h.rect(&line);
    let reply_box = h.rect("Reply composer");
    let channel_box = h.rect("Message composer");
    assert_eq!(
        h.tree.iter().filter(|(n, _)| n == &line).count(),
        1,
        "shown once"
    );
    assert!(
        at.top() > reply_box.top() && at.left() >= reply_box.left() - 20.0,
        "under the thread's reply box {:?}, at {:?}",
        reply_box,
        at
    );
    assert!(
        at.left() > channel_box.right() - 1.0,
        "not under the channel's box"
    );
    // the line doesn't push "Also send to" out of the window
    let also = h.rect("Also send to the channel");
    assert!(
        also.top() > at.bottom() && also.bottom() <= h.size.y - 4.0,
        "{:?} below the window",
        also
    );

    // back in the channel: typing there shows under the channel's box only
    h.app.take_net_events(vec![wb_net::Event::Typing {
        channel: cid,
        member: 2,
        thread: None,
    }]);
    h.settle();
    let at = h.rect(&line);
    assert!(
        at.left() < channel_box.right() && at.top() > channel_box.top(),
        "under the channel's box {:?}, at {:?}",
        channel_box,
        at
    );
    assert_eq!(h.tree.iter().filter(|(n, _)| n == &line).count(), 1);

    // typing in the reply box tells the server which thread
    h.app.sync_rt.net = Some(wb_net::Client::start(
        "127.0.0.1:1",
        wb_net::SignIn::default(),
        std::sync::Arc::new(|| {}),
    ));
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.click("Reply composer");
    h.type_text("on it");
    let told = h.app.sync_rt.typed.expect("told the server");
    assert_eq!((told.0, told.1), (cid, Some(tid)));
    h.app.sync_rt.net = None;
}

/// "@channel", "@here" and a team's name in the @ list: the message keeps
/// everyone they stand for in its mentions, so every app and the server
/// notify them.
#[test]
fn group_mentions_from_the_list_keep_their_people() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let others: Vec<u64> = h
        .app
        .data
        .members
        .iter()
        .filter(|m| !m.invited && m.id != me)
        .map(|m| m.id)
        .collect();
    assert!(others.len() >= 2);
    h.app.data.extra.teams.push(Team {
        id: 900,
        name: "Design Crew".into(),
        members: vec![others[0], others[1], me],
        color: [1, 2, 3],
    });
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let last = |h: &Ui1| {
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .messages
            .last()
            .unwrap()
            .clone()
    };
    h.click("Mention someone");
    h.click("@Design Crew");
    assert_eq!(h.app.chat.channel.text, "@Design Crew ");
    h.type_text("review please");
    h.key(egui::Key::Enter);
    let m = last(&h);
    assert_eq!(m.text, "@Design Crew review please");
    assert_eq!(m.mentions, vec![others[0], others[1]], "not me, the writer");
    h.click("Mention someone");
    h.click("@channel");
    // (the pick leaves the box focused with the cursor at the end)
    h.type_text("standup");
    h.key(egui::Key::Enter);
    let mut got = last(&h).mentions;
    got.sort();
    let mut want = others.clone();
    want.sort();
    assert_eq!(got, want, "@channel is everyone in a public channel");
    // without a server no one is known to be online: @here names no one
    h.click("Mention someone");
    h.click("@here");
    h.type_text("anyone?");
    h.key(egui::Key::Enter);
    assert!(last(&h).mentions.is_empty());
}

/// A channel set to notify about nothing is dimmed in the sidebar and shows
/// no unread count.
#[test]
fn a_channel_set_to_nothing_shows_no_unread_count() {
    let mut h = Ui1::new();
    let g = h.channel_id("general");
    h.app.section = Section::Home;
    let unread = h
        .app
        .unread_count(h.app.data.channels.iter().find(|c| c.id == g).unwrap());
    if unread == 0 {
        let id = h.app.data.new_id();
        let c = h.app.data.channels.iter_mut().find(|c| c.id == g).unwrap();
        let mut m = c.messages.last().unwrap().clone();
        m.id = id;
        m.author = 2;
        m.ts = now_ts() + 5;
        c.messages.push(m);
    }
    h.settle();
    let n = h
        .app
        .unread_count(h.app.data.channels.iter().find(|c| c.id == g).unwrap());
    assert!(n > 0);
    let side = Rect::from_min_max(pos2(0.0, 0.0), pos2(320.0, 5000.0));
    let count_on_row = |h: &Ui1| {
        let row = h.rect_in("general", side);
        h.painted_text(&n.to_string())
            .iter()
            .any(|(r, _)| side.contains(r.center()) && (r.center().y - row.center().y).abs() < 8.0)
    };
    assert!(count_on_row(&h), "the count shows");
    h.app.data.extra.notify.set_channel_mode(g, "nothing");
    h.settle();
    assert!(!count_on_row(&h), "a quiet channel shows no count");
}

/// Keywords are typed in the Notifications settings.
#[test]
fn notification_keywords_are_typed_in_settings() {
    let mut h = Ui1::new();
    h.click("Inbox");
    h.click("Notification settings");
    // the dialog scrolls: wheel down over it to reach the keywords
    let over = h.find("Direct messages").unwrap();
    for _ in 0..10 {
        h.frame(vec![egui::Event::PointerMoved(over)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -120.0),
            modifiers: Default::default(),
        }]);
    }
    h.settle();
    h.click("Notification keywords");
    h.type_text("deploy, 배포");
    h.key(egui::Key::Enter);
    assert_eq!(
        h.app.data.extra.notify.keyword_list(),
        vec!["deploy", "배포"]
    );
}

fn last_in(h: &Ui1, cid: u64) -> Message {
    h.app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone()
}

/// Formatting marks are kept in the message but hidden when it is shown;
/// the B button and Cmd+B put them around the selection.
#[test]
fn formatting_buttons_and_keys_wrap_the_selection_and_marks_are_hidden() {
    let mut h = Ui1::new();
    let cmd = egui::Modifiers {
        mac_cmd: true,
        command: true,
        ..Default::default()
    };
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("Message composer");
    h.type_text("ship it");
    h.key_with(egui::Key::A, cmd);
    h.click("Bold");
    assert_eq!(h.app.chat.channel.text, "**ship it**");
    // the same text stays selected: Italic goes inside the bold
    h.click("Italic");
    assert_eq!(h.app.chat.channel.text, "***ship it***");
    h.click("Italic");
    assert_eq!(h.app.chat.channel.text, "**ship it**", "again takes it off");
    // at the end with nothing selected: two marks, typing goes between them
    h.key(egui::Key::End);
    h.type_text(" now ");
    h.key_with(egui::Key::C, cmd | egui::Modifiers::SHIFT);
    h.type_text("cargo test");
    assert_eq!(h.app.chat.channel.text, "**ship it** now `cargo test`");
    h.key(egui::Key::Enter);
    assert_eq!(last_in(&h, cid).text, "**ship it** now `cargo test`");
    assert!(
        !h.painted_text("ship it now cargo test").is_empty(),
        "shown without its marks"
    );
    assert!(h.painted_text("**ship it** now `cargo test`").is_empty());
}

/// A message with a web address gets the page's preview (fetched once by
/// the sender's app); its writer can take it away.
#[test]
fn a_web_address_gets_a_preview_its_writer_can_remove() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.send_message("plan at https://example.com/plan.");
    let m = last_in(&h, cid);
    let p = m.preview.expect("a preview");
    assert_eq!(p.url, "https://example.com/plan");
    assert_eq!(p.title, "Example page");
    assert!(h.has("Example page"), "the card shows the page's title");
    h.click("Remove preview");
    assert!(last_in(&h, cid).preview.is_none());
    assert!(!h.has("Example page"));
    // a message without an address gets none; someone else's has no remove button
    h.send_message("no address here");
    assert!(last_in(&h, cid).preview.is_none());
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap();
    let mut other = c.messages.last().unwrap().clone();
    other.id = 987_654;
    other.author = 2;
    other.text = "see https://example.com/x".into();
    other.preview = Some(LinkPreview {
        url: "https://example.com/x".into(),
        title: "Their page".into(),
        ..Default::default()
    });
    c.messages.push(other);
    h.settle();
    assert!(h.has("Their page"));
    assert!(!h.has("Remove preview"));
}

/// Korean written as separate letters (a caption typed on a Mac, a Finder
/// file name) shows as whole syllables: in a kept link preview and in
/// text pasted into the message box.
#[test]
fn korean_in_separate_letters_shows_as_syllables() {
    // "바이브 특징" decomposed
    let nfd = "\u{1107}\u{1161}\u{110B}\u{1175}\u{1107}\u{1173} \u{1110}\u{1173}\u{11A8}\u{110C}\u{1175}\u{11BC}";
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.send_message("see https://example.com/caption");
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap();
    c.messages.last_mut().unwrap().preview = Some(LinkPreview {
        url: "https://example.com/caption".into(),
        title: format!("Title {nfd}"),
        description: format!("Caption {nfd}"),
        site: "Instagram".into(),
    });
    h.settle();
    assert!(h.has("Title 바이브 특징"));
    assert!(!h.painted_text("Caption 바이브 특징").is_empty());

    h.click("Message composer");
    h.frame(vec![egui::Event::Paste(nfd.into())]);
    h.key(egui::Key::Enter);
    assert_eq!(last_in(&h, cid).text, "바이브 특징");
}

/// More actions > Forward message: pick a conversation, add a note; the
/// message arrives there carrying the original, which opens where it came from.
#[test]
fn forward_a_message_to_another_channel_with_a_note() {
    let (mut h, cid) = general_with_my_message("the numbers are in");
    let random = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel && c.id != cid && h.app.listed(c))
        .expect("another channel")
        .clone();
    h.click("More actions");
    h.click("Forward message");
    assert!(matches!(h.app.modal, Some(Modal::Forward { .. })));
    h.click(&format!("# {}", random.name));
    h.click("Forward note");
    h.type_text("FYI");
    h.click("Forward");
    assert!(h.app.modal.is_none());
    let got = last_in(&h, random.id);
    assert_eq!(got.text, "FYI");
    let f = got.forwarded.expect("carries the message");
    assert_eq!(f.text, "the numbers are in");
    assert_eq!(f.channel, cid);
    // shown in that channel, and the link goes back to #general
    h.app.chat.open = Some(random.id);
    h.settle();
    assert!(h.has("Forwarded from"));
    h.click("Forwarded from");
    assert_eq!(h.app.chat.open, Some(cid));
}

/// "Also send to #general" under a thread's box: the reply also goes in the
/// channel, marked, with a link back to the thread; it isn't told twice.
#[test]
fn a_thread_reply_can_also_go_to_the_channel() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("1 reply");
    let tid = h.app.chat.thread_open.unwrap();
    h.click("Also send to the channel");
    assert!(h.app.chat.also_channel);
    h.click("Reply composer");
    h.type_text("done, see the doc");
    h.key(egui::Key::Enter);
    let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let parent = c.messages.iter().find(|m| m.id == tid).unwrap();
    assert_eq!(parent.replies.last().unwrap().text, "done, see the doc");
    let copy = c.messages.last().unwrap();
    assert_eq!(copy.text, "done, see the doc");
    assert_eq!(copy.thread_of, Some(tid));
    assert!(!h.app.chat.also_channel, "unticked for the next reply");
    // the next reply stays in the thread only
    h.click("Reply composer");
    h.type_text("one more");
    h.key(egui::Key::Enter);
    assert_eq!(last_in(&h, cid).text, "done, see the doc");
    // the copy's link opens the thread
    h.app.chat.thread_open = None;
    h.settle();
    h.click("View thread");
    assert_eq!(h.app.chat.thread_open, Some(tid));
}

/// An unsent message is listed in Drafts, opens its conversation, and is
/// kept with the workspace when it is saved.
#[test]
fn drafts_are_listed_and_saved_with_the_workspace() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("Message composer");
    h.type_text("half a thought");
    // go elsewhere: the draft is put away
    h.app.section = Section::Home;
    h.app.chat.open = None;
    h.settle();
    let saved = h.app.data_to_save();
    assert_eq!(
        saved.extra.chat_drafts,
        vec![ChatDraft {
            channel: cid,
            thread: None,
            text: "half a thought".into()
        }]
    );
    h.click("Drafts");
    h.click("Draft in #general");
    assert_eq!(h.app.chat.open, Some(cid));
    h.settle();
    assert_eq!(h.app.chat.channel.text, "half a thought");
    // a workspace saved with a draft brings it back
    let mut h2 = Ui1::new();
    h2.app.data.extra.chat_drafts = saved.extra.chat_drafts.clone();
    h2.app.drafts.loaded_for = None;
    h2.settle();
    h2.click("general");
    assert_eq!(h2.app.chat.channel.text, "half a thought");
}

/// The small arrow joined to Send keeps the message for later; Scheduled
/// messages lists it with Send now, Edit and Cancel. Without a server this
/// computer posts it when its time comes.
#[test]
fn schedule_a_message_and_send_edit_or_cancel_it() {
    let mut h = Ui1::new();
    h.click("general");
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
    h.type_text("standup notes");
    h.click("Schedule message");
    h.click("In 1 hour");
    assert_eq!(h.app.chat.channel.text, "", "the box is emptied");
    let s = h.app.scheduled();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].text, "standup notes");
    assert!((s[0].at - now_ts() - 3600).abs() < 5);
    assert_eq!(
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .messages
            .len(),
        n,
        "not posted yet"
    );
    // listed; Edit puts it back in the box
    h.click("Scheduled messages");
    assert!(h.has("standup notes"));
    let id = s[0].id;
    h.click(&format!("Edit {}", id));
    assert!(h.app.scheduled().is_empty());
    h.settle();
    assert_eq!(h.app.chat.channel.text, "standup notes");
    // schedule again, then Send now
    h.click("Schedule message");
    h.click("Tomorrow at 9:00 AM");
    let id = h.app.scheduled()[0].id;
    h.click("Scheduled messages");
    h.click(&format!("Send now {}", id));
    assert_eq!(last_in(&h, cid).text, "standup notes");
    assert!(h.app.scheduled().is_empty());
    // its time comes while the app runs
    h.app.modal = None;
    h.click("Message composer");
    h.type_text("later on");
    h.click("Schedule message");
    h.click("In 1 hour");
    h.app.data.extra.scheduled[0].at = now_ts() - 1;
    h.settle();
    assert_eq!(last_in(&h, cid).text, "later on");
    assert!(h.app.data.extra.scheduled.is_empty());
    // Cancel
    h.click("Message composer");
    h.type_text("not this");
    h.click("Schedule message");
    h.click("In 1 hour");
    let id = h.app.scheduled()[0].id;
    h.click("Scheduled messages");
    h.click(&format!("Cancel {}", id));
    assert!(h.app.scheduled().is_empty());
}

/// Send and its arrow are one button split in two: the arrow sits right
/// against Send, as tall and narrower, and offers its times only once
/// there is text to schedule. A post has no arrow.
#[test]
fn the_arrow_joined_to_send_waits_for_text() {
    let mut h = Ui1::new();
    h.click("general");
    let send = h.rect("Send");
    let arrow = h.rect("Schedule message");
    assert!(
        (arrow.left() - send.right()).abs() <= 1.5,
        "{send:?} {arrow:?}"
    );
    assert_eq!(arrow.height(), send.height());
    assert!(arrow.width() < send.width(), "{send:?} {arrow:?}");
    assert!((arrow.center().y - send.center().y).abs() < 0.5);
    h.click("Schedule message");
    assert!(!h.has("In 1 hour"), "nothing to schedule yet");
    h.click("Message composer");
    h.type_text("later");
    h.click("Schedule message");
    assert!(h.has("In 1 hour"));
    h.key(egui::Key::Escape);
    h.app.composer_mode = crate::app::chat::ComposerMode::Post;
    h.settle();
    assert!(!h.has("Schedule message"), "a post goes now");
    assert!(h.has("Post"));
}

/// Built-in slash commands, the "/" list, and plain text that only starts
/// with a slash.
#[test]
fn slash_commands_work_in_the_message_box() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let send = |h: &mut Ui1, text: &str| {
        h.send_message(text);
    };
    // the list: "/sh" offers /shrug
    h.click("Message composer");
    h.type_text("/sh");
    h.click("Command /shrug");
    assert_eq!(h.app.chat.channel.text, "/shrug ");
    h.type_text("oh well");
    h.key(egui::Key::Enter);
    assert_eq!(
        last_in(&h, cid).text,
        "oh well \u{00AF}\\_(\u{30C4})_/\u{00AF}"
    );
    send(&mut h, "/me waves");
    assert_eq!(last_in(&h, cid).text, "*waves*");
    let n = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    send(&mut h, "/topic Launch week");
    assert_eq!(
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .topic,
        "Launch week"
    );
    send(&mut h, "/mute");
    assert_eq!(h.app.data.extra.notify.channel_mode(cid), "nothing");
    send(&mut h, "/unmute");
    assert_eq!(h.app.data.extra.notify.channel_mode(cid), "mentions");
    let r = h.app.data.extra.reminders.len();
    send(&mut h, "/remind me to call Sam in 30 minutes");
    let rem = h.app.data.extra.reminders.last().unwrap();
    assert_eq!(h.app.data.extra.reminders.len(), r + 1);
    assert_eq!(rem.text, "to call Sam");
    assert!((rem.due_ts - now_ts() - 1800).abs() < 5);
    assert_eq!(
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .messages
            .len(),
        n,
        "none of those posted anything"
    );
    // an unknown one goes back in the box; a path is just text
    send(&mut h, "/nope now");
    assert_eq!(h.app.chat.channel.text, "/nope now");
    h.app.chat.channel.text.clear();
    send(&mut h, "/usr/bin is full");
    assert_eq!(last_in(&h, cid).text, "/usr/bin is full");
}

/// Workflows: a message posted in #general posts in another channel,
/// replies in its thread and makes a task; what they post starts nothing.
#[test]
fn workflows_run_when_a_message_is_posted() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let other = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel && c.id != cid && h.app.listed(c))
        .unwrap()
        .clone();
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    h.click("Workflows");
    assert!(matches!(h.app.modal, Some(Modal::Workflows { .. })));
    // post in the other channel when a message mentions "outage"
    h.click("Workflow keyword");
    h.type_text("outage");
    h.click("Workflow target");
    h.click(&format!("#{}", other.name));
    h.click("Workflow message");
    h.type_text("{user} reported: {text}");
    h.click("Add workflow");
    // and reply in the thread for every message
    let list = h.app.data.first_list().unwrap();
    h.app.data.extra.automations.push(Automation {
        id: 77_001,
        enabled: true,
        trigger: "message".into(),
        channel: Some(cid),
        action: "reply".into(),
        value: "Thanks, {user}!".into(),
        ..Default::default()
    });
    h.app.data.extra.automations.push(Automation {
        id: 77_002,
        enabled: true,
        trigger: "message".into(),
        channel: Some(cid),
        keyword: "bug".into(),
        action: "task".into(),
        value: list.to_string(),
        ..Default::default()
    });
    h.app.modal = None;
    h.settle();
    let wf = h
        .app
        .data
        .extra
        .automations
        .iter()
        .filter(|a| a.trigger == "message")
        .count();
    assert_eq!(wf, 3);
    h.send_message("Big outage in the bug tracker");
    let me = h.app.who(h.app.data.me);
    let posted = last_in(&h, other.id);
    assert_eq!(
        posted.text,
        format!("{} reported: Big outage in the bug tracker", me)
    );
    assert_eq!(posted.bot.as_deref(), Some("Workflow"));
    let mine = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .iter()
        .find(|m| m.text == "Big outage in the bug tracker")
        .unwrap()
        .clone();
    assert_eq!(mine.replies.len(), 1);
    assert_eq!(mine.replies[0].text, format!("Thanks, {}!", me));
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.name == "Big outage in the bug tracker"));
    // a message without the words: only the reply
    h.send_message("all good");
    assert_eq!(last_in(&h, other.id).text, posted.text, "no second post");
    // shown under the workflow's name
    h.app.chat.open = Some(other.id);
    h.settle();
    assert!(h.has("APP") || !h.painted_text("APP").is_empty());
}

/// Owners and admins on a server see a channel's webhooks with their
/// address and the slash commands in Integrations; the "/" list offers the
/// server's commands.
#[test]
fn integrations_show_webhook_addresses_and_commands() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 1,
        server: Some("https://wb.example.com/w/ab12".into()),
    });
    let me = h.app.data.me;
    if let Some(m) = h.app.data.members.iter_mut().find(|m| m.id == me) {
        m.role = "Owner".into();
    }
    let cid = h.channel_id("general");
    h.app.sync_rt.integrations = Some((
        vec![wb_net::HookInfo {
            token: "tok123".into(),
            kind: "in".into(),
            channel: cid,
            name: "CI".into(),
            ..Default::default()
        }],
        vec![wb_net::CommandInfo {
            name: "deploy".into(),
            description: "Ship it".into(),
            url: "https://bots.example/deploy".into(),
        }],
    ));
    assert_eq!(
        h.app.hook_address("tok123"),
        "https://wb.example.com/w/ab12/hooks/tok123"
    );
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    h.click("Integrations");
    assert!(matches!(h.app.modal, Some(Modal::Integrations { .. })));
    assert!(h.has("Copy address CI"));
    assert!(h.has("Add incoming webhook") && h.has("Add command"));
    // the server's command is in the "/" list
    assert!(h
        .app
        .slash_choices("/dep")
        .iter()
        .any(|(n, d)| n == "deploy" && d == "Ship it"));
}

/// Messages from someone else in channel `cid`, `secs` from now and on.
fn messages_from_others(h: &mut Ui1, cid: u64, texts: &[String], secs: i64) {
    let me = h.app.data.me;
    let other = h.app.data.members.iter().find(|m| m.id != me).unwrap().id;
    for (i, text) in texts.iter().enumerate() {
        let id = h.app.data.new_id();
        let m: Message = serde_json::from_value(serde_json::json!(
            {"id": id, "author": other, "text": text, "ts": now_ts() + secs + i as i64}
        ))
        .unwrap();
        let ch = h
            .app
            .data
            .channels
            .iter_mut()
            .find(|c| c.id == cid)
            .unwrap();
        ch.messages.push(m);
    }
}

/// A long, read #general open and down at its newest message.
fn long_general_open() -> (Ui1, u64) {
    let mut h = Ui1::new();
    let cid = h.channel_id("general");
    let old: Vec<String> = (0..30).map(|i| format!("older note {}", i)).collect();
    messages_from_others(&mut h, cid, &old, -3000);
    h.app.mark_read(cid);
    h.click("general");
    h.settle();
    assert!(h.app.chat.at_bottom);
    (h, cid)
}

#[test]
fn opening_a_channel_draws_a_red_line_above_the_first_unread_message() {
    let mut h = Ui1::new();
    let cid = h.channel_id("general");
    let texts = vec!["first new one".to_string(), "second new one".to_string()];
    messages_from_others(&mut h, cid, &texts, 10);
    h.click("general");
    h.settle();
    assert!(h.has("Read up to here"));
    let line = h.rect("Read up to here");
    let first = h.find("first new one").unwrap();
    assert!(
        line.bottom() <= first.y,
        "the line is above the first unread"
    );
    let (word, color) = h
        .painted_text("New")
        .into_iter()
        .find(|(r, _)| r.intersects(line))
        .expect("labelled New");
    assert_eq!(color, h.app.t().danger, "drawn in red");
    assert!(
        (word.center().x - line.center().x).abs() < 2.0,
        "the word sits in the middle of the line: {:?} {:?}",
        word,
        line
    );
    // they're read now: leaving and coming back draws no line
    h.click("website-redesign");
    h.click("general");
    h.settle();
    assert!(!h.has("Read up to here"));
}

#[test]
fn messages_scrolled_far_out_of_sight_are_not_drawn_but_still_found() {
    let (mut h, cid) = long_general_open();
    // the oldest is far above the window: kept in place and named, not painted
    assert!(h.has("older note 0"));
    assert!(h.painted_text("older note 0").is_empty());
    assert!(!h.painted_text("older note 29").is_empty());
    let y29 = h.find("older note 29").unwrap().y;
    // scrolled to the top, it is drawn where its place was kept
    for _ in 0..30 {
        let at = pos2(h.size.x / 2.0, h.size.y / 2.0);
        h.frame(vec![egui::Event::PointerMoved(at)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, 400.0),
            modifiers: Default::default(),
        }]);
        h.settle();
    }
    let (r, _) = h.painted_text("older note 0")[0];
    assert!((r.center().y - h.find("older note 0").unwrap().y).abs() < 30.0);
    assert!(h.find("older note 29").unwrap().y > y29);
    // a message coming in while up there keeps its place below the others
    messages_from_others(&mut h, cid, &["one more".to_string()], 10);
    h.settle();
    assert!(h.painted_text("one more").is_empty());
    assert!(h.find("older note 29").unwrap().y < h.find("one more").unwrap().y);
}

#[test]
fn messages_that_come_in_while_scrolled_up_show_a_grey_note_that_goes_down() {
    let (mut h, cid) = long_general_open();
    assert!(!h.has("Read up to here"), "everything was read");
    h.scroll_into_view("older note 2");
    assert!(!h.app.chat.at_bottom);
    assert!(!h.has("New messages"));

    let texts = vec!["fresh one".to_string(), "fresh two".to_string()];
    messages_from_others(&mut h, cid, &texts, 10);
    h.settle();
    assert!(!h.app.chat.at_bottom, "the list stays where I'm reading");
    assert!(h.has("New messages"));
    let (_, color) = h.painted_text("2 new messages")[0];
    assert_eq!(color, h.app.t().text2, "a quiet grey note, not a colour");
    assert!(
        h.has("Read up to here"),
        "the red line marks where they start"
    );

    h.click("New messages");
    for _ in 0..30 {
        h.settle();
    }
    assert!(h.app.chat.at_bottom, "down at the newest message");
    assert!(!h.has("New messages"));
    assert!(h.has("Read up to here"), "the line stays until I leave");
}

#[test]
fn messages_that_come_in_at_the_bottom_need_no_note_or_line() {
    let (mut h, cid) = long_general_open();
    messages_from_others(&mut h, cid, &["just now".to_string()], 10);
    h.settle();
    assert!(h.app.chat.at_bottom);
    assert!(!h.has("New messages"));
    assert!(!h.has("Read up to here"));
}

#[test]
fn messages_that_come_in_while_the_window_is_behind_get_the_red_line() {
    let (mut h, cid) = long_general_open();
    h.focused = false;
    h.frame(vec![egui::Event::WindowFocused(false)]);
    messages_from_others(&mut h, cid, &["while away".to_string()], 10);
    h.settle();
    h.focused = true;
    h.frame(vec![egui::Event::WindowFocused(true)]);
    h.settle();
    let line = h.rect("Read up to here");
    assert!(line.bottom() <= h.find("while away").unwrap().y);
    assert!(!h.has("New messages"), "the list is down at them already");
}

// #general with a thread of mine that others replied to, by `authors`
fn general_with_thread(authors: &[u64]) -> (Ui1, u64) {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.name == "general")
        .unwrap();
    let cid = c.id;
    let mut m = c.messages[0].clone();
    m.id = 9001;
    m.author = me;
    m.text = "Who has the keys?".into();
    m.ts = now_ts() - 60;
    m.replies = authors
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let mut r = m.clone();
            r.id = 9100 + i as u64;
            r.author = *a;
            r.text = format!("answer {}", i);
            r.ts = now_ts() - 50 + i as i64;
            r.replies = vec![];
            r
        })
        .collect();
    c.messages.push(m);
    h.app.data.extra.replies_seen = now_ts() - 100;
    h.click("general");
    (h, cid)
}

#[test]
fn opening_a_thread_marks_its_replies_read() {
    let (mut h, cid) = general_with_thread(&[2, 3]);
    assert_eq!(h.app.unread_replies(), 2);
    let badge = h.app.inbox_badge();
    h.click("2 replies");
    assert_eq!(h.app.chat.thread_open, Some(9001));
    let read = &h.app.data.extra.replies_read;
    assert!(read.contains(&format!("chat:{}:9100", cid)));
    assert!(read.contains(&format!("chat:{}:9101", cid)));
    assert_eq!(h.app.unread_replies(), 0, "gone from Replies' Unread");
    assert_eq!(h.app.inbox_badge(), badge - 2, "and from the Home badge");
}

#[test]
fn thread_summary_avatars_sit_on_the_text_line_and_count_people_once() {
    // Alex twice, then Sam, Jordan and me: four people, so three faces and "+1"
    let (h, _) = general_with_thread(&[2, 2, 3, 4, 1]);
    let label = h.painted_text(&tr("{} replies").replace("{}", "5"))[0].0;
    let more = h.painted_text("+1");
    assert_eq!(more.len(), 1, "each person once: +1, not +2");
    let more = more[0].0;
    assert!(
        more.right() <= label.left(),
        "+1 {:?} runs into the label {:?}",
        more,
        label
    );
    // Alex's face is the first, just left of the label
    let face = h
        .painted_text("AK")
        .into_iter()
        .map(|(r, _)| r)
        .find(|r| r.right() <= label.left() && label.left() - r.right() < 80.0)
        .expect("Alex's face beside the label");
    assert!(
        (face.center().y - label.center().y).abs() <= 2.0,
        "faces {:?} off the label's line {:?}",
        face,
        label
    );
}

#[test]
fn reply_box_shows_no_enter_hint() {
    // the "Enter to send" line was cut short in a narrow thread, so it is gone;
    // Shift+Enter is still listed under Keyboard shortcuts
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    let _ = h.rect("Reply composer");
    let mut found = false;
    each_text(&h.shapes, |ts, _| {
        let s = ts.galley.text();
        if s.starts_with("Enter to send") || s.starts_with("Enter로 보내기") {
            found = true;
        }
    });
    assert!(!found, "the reply box has no Enter hint");
}

/// A red "Unread messages" pill sits at the bottom of the sidebar while an
/// unread DM is scrolled out of view below; clicking it brings the DM into
/// view and the pill goes away.
#[test]
fn unread_pill_at_the_sidebar_bottom_scrolls_to_an_unread_dm_below() {
    let pill = UNREAD_PILL;
    let mut h = Ui1::sized(960.0, 560.0);
    h.app.section = Section::Home;
    let me = h.app.data.me;
    let later = now_ts() + 100;
    for c in h.app.data.channels.iter_mut() {
        c.last_read = later;
    }
    // the last DM in the sidebar, at the very bottom of the list
    let dm = h
        .app
        .data
        .channels
        .iter()
        .filter(|c| c.kind == ChannelKind::Direct && h.app.listed(c))
        .last()
        .expect("a DM")
        .clone();
    let other = dm.members.iter().copied().find(|m| *m != me).unwrap();
    let name = h.app.data.member(other).unwrap().shown().to_string();
    let mut m = h
        .app
        .data
        .channels
        .iter()
        .flat_map(|c| c.messages.iter())
        .next()
        .cloned()
        .expect("a message somewhere");
    m.id = h.app.data.new_id();
    m.author = other;
    m.ts = later + 5;
    m.imported = None;
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == dm.id)
        .unwrap();
    c.messages.push(m);
    h.settle();
    let side = Rect::from_min_max(pos2(0.0, 0.0), pos2(320.0, 560.0));
    let visible = |h: &Ui1| {
        h.tree
            .iter()
            .any(|(n, r)| *n == name && side.contains(r.center()))
    };
    assert!(!visible(&h), "the DM starts below the visible part");
    let at = h.find(pill).expect("the pill shows while the DM is below");
    assert!(side.contains(at) && at.y > 400.0, "at the sidebar's bottom");
    h.click_at(at);
    h.settle();
    assert!(visible(&h), "clicking the pill scrolls the DM into view");
    assert!(!h.has(pill), "and the pill goes away");

    // nothing unread below in a window tall enough to show it all
    let mut h = Ui1::new();
    h.app.section = Section::Home;
    for c in h.app.data.channels.iter_mut() {
        c.last_read = now_ts() + 100;
    }
    h.settle();
    assert!(!h.has(pill));
}

/// The Post tab puts a megaphone in the message box so it is plain this goes
/// out as a post, and once the post is sent the box is back to messages.
#[test]
fn a_post_shows_its_icon_in_the_box_and_then_goes_back_to_messages() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    assert!(!h.has("Post mode"), "a plain message box has no megaphone");
    h.click("Post");
    assert!(h.has("Post mode"), "the post box shows the megaphone");
    h.send_message("Office closed on Friday");
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let last = ch.messages.last().unwrap();
    assert_eq!(last.text, "Office closed on Friday");
    assert!(last.post_title.is_some(), "it went out as a post");
    h.settle();
    assert_eq!(
        h.app.composer_mode,
        crate::app::chat::ComposerMode::Message,
        "the box is back to messages"
    );
    assert!(!h.has("Post mode"));
    // the next thing typed is a plain message
    h.send_message("thanks");
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert!(ch.messages.last().unwrap().post_title.is_none());
}

/// On the Posts page each post is a card that reads top to bottom: the
/// author line, the title under it and the text under that across the card
/// (the text once came out one letter per line beside the title). A post in a
/// direct message names the person, not an empty "# ".
#[test]
fn a_post_card_stacks_its_title_and_text_and_names_where_it_went() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Post");
    h.send_message("Office closed on Friday for the move");
    let me = h.app.data.me;
    let post = h
        .app
        .data
        .channels
        .iter()
        .find_map(|c| c.messages.iter().find(|m| m.post_title.is_some()))
        .unwrap()
        .clone();
    let dm = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.kind == ChannelKind::Direct && c.members.contains(&me) && c.members.len() > 1)
        .unwrap();
    let mut in_dm = post.clone();
    in_dm.id += 1_000_000;
    in_dm.text = "Lunch is on me".into();
    dm.messages.push(in_dm);
    let dm = dm.clone();
    let who = h.app.channel_label(&dm);
    h.app.section = Section::Posts;
    h.settle();

    let title = h.painted_text("Update");
    let body = h.painted_text("Office closed on Friday for the move");
    assert!(!title.is_empty() && !body.is_empty(), "the post is listed");
    let (t, b) = (title[0].0, body[0].0);
    assert!(
        b.top() >= t.bottom() - 1.0,
        "the text is under the title: {t:?} {b:?}"
    );
    assert!(b.height() < 40.0, "the text stays on one line: {b:?}");
    let open = h.rect("Open in channel");
    assert!(
        t.right() < open.left(),
        "the title is not beside the link: {t:?} {open:?}"
    );

    let texts = h.texts();
    assert!(
        texts
            .iter()
            .any(|x| x.starts_with(&format!("Announcement in {who} "))),
        "the DM post names {who}: {texts:?}"
    );
    assert!(!texts
        .iter()
        .any(|x| x.contains("in #  ") || x.contains("in  ")));
}

/// The Sort menu above the posts ends where the post cards end, in a wide
/// and a narrow window and in both languages (the filter chips before it
/// differ in width), and never sits on top of the chips: with no room
/// beside them (the smallest window, a long channel name picked) it goes
/// under them.
#[test]
fn the_posts_sort_menu_lines_up_with_the_cards() {
    for (w, ko, long) in [
        (1440.0, false, false),
        (960.0, false, false),
        (1440.0, true, false),
        (960.0, false, true),
        (1000.0, false, true),
    ] {
        let mut h = Ui1::sized(w, 900.0);
        h.click("general");
        h.click("Post");
        h.send_message("Office closed on Friday for the move");
        if long {
            let id = h.app.data.new_id();
            let me = h.app.data.me;
            h.app.data.channels.push(Channel {
                id,
                name: "engineering-announcements".into(),
                kind: ChannelKind::Channel,
                members: vec![me],
                ..Default::default()
            });
            h.app.post.f_channel = Some(id);
            // the post is in #general: show it too, to measure the card
            h.app
                .data
                .channels
                .iter_mut()
                .find(|c| c.id == id)
                .unwrap()
                .messages = h
                .app
                .data
                .channels
                .iter()
                .find(|c| c.name == "general")
                .unwrap()
                .messages
                .iter()
                .filter(|m| m.post_title.is_some())
                .cloned()
                .collect();
        }
        if ko {
            crate::i18n::set_lang(crate::i18n::Lang::Ko);
        }
        h.app.section = Section::Posts;
        h.settle();
        let case = format!("{w} ko {ko} long filter {long}");
        let sort = h.rect("Sort posts");
        let label = h.painted_text(tr("Sort:"))[0].0;
        let chip = h.rect("Post type filter");
        let open = h.rect(tr("Open in channel"));
        // the card's inner margin is 14 points
        let card_right = open.right() + 14.0;
        assert!(
            (sort.right() - card_right).abs() <= 2.0,
            "{case}: Sort ends at {} but the cards at {card_right}",
            sort.right()
        );
        assert!(
            !sort.intersects(chip) && !label.intersects(chip),
            "{case}: Sort {sort:?} / {label:?} sits on the Type chip {chip:?}"
        );
        // and it still sorts
        h.click("Sort posts");
        h.click(tr("Oldest"));
        assert!(h.app.post.sort_old, "{case}: Oldest picked");
        crate::i18n::set_lang(crate::i18n::Lang::En);
    }
}

/// A post sent without a title is saved as "Update" for everyone, and each
/// reader sees that default in their own language: in the channel and on the
/// Posts page. A title someone typed is shown as typed.
#[test]
fn the_default_post_title_is_shown_in_the_readers_language() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("Post");
    h.send_message("Office closed on Friday");
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let mut typed = ch.messages.last().unwrap().clone();
    assert_eq!(
        typed.post_title.as_deref(),
        Some("Update"),
        "saved in English"
    );
    typed.id += 1_000_000;
    typed.post_title = Some("Update on the move".into());
    typed.text = "Boxes go out Monday".into();
    let ch = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap();
    ch.messages.push(typed);

    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    h.settle();
    let in_chat = h.painted_text("업데이트").len();
    let english_in_chat = h.painted_text("Update").len();
    let typed_in_chat = h.painted_text("Update on the move").len();
    h.app.section = Section::Posts;
    h.settle();
    let on_posts = h.painted_text("업데이트").len();
    let english_on_posts = h.painted_text("Update").len();
    let typed_on_posts = h.painted_text("Update on the move").len();
    crate::i18n::set_lang(crate::i18n::Lang::En);

    assert!(
        in_chat > 0 && english_in_chat == 0,
        "the channel shows 업데이트"
    );
    assert!(
        on_posts > 0 && english_on_posts == 0,
        "the Posts page shows 업데이트"
    );
    assert!(
        typed_in_chat > 0 && typed_on_posts > 0,
        "a typed title stays as typed"
    );
}

#[test]
fn a_message_that_comes_in_while_reading_at_the_bottom_takes_the_red_line_away() {
    let mut h = Ui1::new();
    let cid = h.channel_id("general");
    h.app.mark_read(cid);
    h.click("general");
    h.settle();
    // two come in while the window is behind another app
    h.focused = false;
    h.settle();
    for i in 1..=2 {
        messages_from_others(&mut h, cid, &[format!("while away {}", i)], i);
        h.settle();
    }
    h.focused = true;
    h.settle();
    assert!(
        h.has("Read up to here"),
        "back: the line shows where they start"
    );
    // a third while I'm reading at the bottom: all three are read, so the
    // line doesn't stay above the first two as if they weren't
    messages_from_others(&mut h, cid, &["while here".to_string()], 3);
    h.settle();
    assert!(!h.has("Read up to here"));
    let c = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .clone();
    assert_eq!(h.app.unread(&c), 0);
}

/// A long message grows the box only so far, then scrolls inside it: the
/// buttons under the box stay in the window and the conversation keeps its
/// room above it, in the channel and in a thread.
#[test]
fn a_long_message_scrolls_inside_the_box_and_keeps_what_is_under_it() {
    let long = "a long line of words that wraps in the message box ".repeat(80);
    let mut h = Ui1::new();
    h.click("general");
    let tabs_before = h.rect("Message").top();
    h.click("Message composer");
    h.type_text(&long);
    h.settle();
    let emoji = h.rect("Emoji");
    assert!(
        emoji.bottom() <= 900.0,
        "the box's buttons stay in the window: {:?}",
        emoji
    );
    let tabs = h.rect("Message").top();
    assert!(
        tabs_before - tabs <= crate::app::chat::COMPOSER_MAX_H,
        "the box grows only so far: {} -> {}",
        tabs_before,
        tabs
    );
    assert!(tabs > 400.0, "the conversation keeps its room: {}", tabs);
    // still sent whole
    h.key(egui::Key::Enter);
    let cid = h.app.chat.open.unwrap();
    assert!(general_msg(&h, cid, long.trim()).is_some());

    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    h.click("Reply composer");
    h.type_text(&long);
    h.settle();
    let also = h.rect("Also send to the channel");
    assert!(
        also.bottom() <= 900.0,
        "the thread's check stays in the window: {:?}",
        also
    );
}

#[test]
fn a_message_coming_into_the_open_chat_shows_no_count_and_no_jump() {
    let mut h = Ui1::new();
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    let now = chrono::Utc::now().timestamp();
    let long = |i: usize| format!("Earlier line {} that runs on long enough to wrap", i);
    {
        let c = h.app.data.channels.iter_mut().find(|c| c.id == dm).unwrap();
        for i in 0..30 {
            c.messages.push(crate::app::slack::new_message(
                930_000 + i as u64,
                2,
                long(i),
                now - 600 + i as i64,
            ));
        }
    }
    h.app.section = Section::Chat;
    h.app.chat.open = Some(dm);
    for _ in 0..10 {
        h.frame(vec![]);
    }
    // the red counts painted in the sidebar
    let danger = h.app.t().danger;
    let counts = |h: &Ui1| {
        h.shapes
            .iter()
            .filter(|s| match &s.shape {
                egui::Shape::Rect(r) => r.fill == danger && r.rect.height() <= 17.0,
                _ => false,
            })
            .count()
    };
    let before = counts(&h);
    let arrive = |h: &mut Ui1, id: u64, text: &str, ts: i64| {
        let copy = teammate_change(&h.app.data, |d| {
            let c = d.channels.iter_mut().find(|c| c.id == dm).unwrap();
            c.messages
                .push(crate::app::slack::new_message(id, 2, text.into(), ts));
        });
        h.app.take_net_events(vec![wb_net::Event::Update {
            data: copy,
            rev: 1,
            hidden: vec![],
        }]);
    };

    arrive(&mut h, 990_101, "hello there", now + 1);
    h.frame(vec![]);
    assert_eq!(counts(&h), before, "no count flashes on the open chat");
    let first = (h.find(&long(29)), h.find("hello there"));
    assert!(first.1.is_some(), "the new message shows at once");
    for _ in 0..5 {
        h.frame(vec![]);
        assert_eq!(counts(&h), before);
        assert_eq!(
            (h.find(&long(29)), h.find("hello there")),
            first,
            "the list doesn't move after the first frame"
        );
    }

    // with the window behind another app it isn't read: it counts
    h.focused = false;
    h.settle();
    arrive(&mut h, 990_102, "while away", now + 2);
    h.settle();
    assert!(counts(&h) > before, "counted while away");
    h.focused = true;
    h.settle();
    assert_eq!(counts(&h), before, "read on coming back");
}

/// Letters typed after "@" narrow the @ list, and the name picked replaces
/// the "@" and those letters (it used to leave "@Al@Alex Kim").
#[test]
fn a_picked_mention_replaces_what_was_typed_after_the_at() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Message composer");
    h.type_text("Hi ");
    // Jordan may be named elsewhere on screen (a direct message)
    let jordan = h.count_named("Jordan Park");
    let alex = h.count_named("Alex Kim");
    h.type_text("@");
    h.type_text("Al");
    h.settle();
    assert_eq!(h.count_named("Alex Kim"), alex + 1, "the list offers Alex");
    assert_eq!(
        h.count_named("Jordan Park"),
        jordan,
        "and only the names with Al"
    );
    // the picker's entry, right of a sidebar row of the same name
    h.click_side("Alex Kim", true);
    assert_eq!(h.app.chat.channel.text, "Hi @Alex Kim ");
}

#[test]
fn trailing_mention_is_the_at_word_at_the_end() {
    use crate::app::chat::trailing_mention;
    assert_eq!(trailing_mention("Hi @Al"), Some((3, "Al")));
    assert_eq!(trailing_mention("@"), Some((0, "")));
    assert_eq!(trailing_mention("mail a@b"), None);
    assert_eq!(trailing_mention("@Al done"), None);
    assert_eq!(trailing_mention("안녕 @민"), Some((7, "민")));
}

/// An empty direct message says "Chat with" the person, not "Chat in" them
/// as a channel does.
#[test]
fn an_empty_direct_message_is_a_chat_with_someone() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Jordan Park")
        .unwrap()
        .clone();
    let id = h.app.data.new_id();
    h.app.data.channels.push(Channel {
        id,
        name: String::new(),
        kind: ChannelKind::Direct,
        members: vec![me, other.id],
        ..Default::default()
    });
    h.app.section = Section::Chat;
    h.app.chat.open = Some(id);
    h.settle();
    let head = trf!("Chat with {}", other.shown());
    assert!(!h.painted_text(&head).is_empty(), "{:?}", h.texts());
}

/// The tabs over a channel are dragged into a new order (the Channel tab
/// too), "+ View" adds a tab there without leaving the channel, and a view
/// tab is taken off with a right click. The order is kept per channel.
#[test]
fn channel_tabs_drag_into_a_new_order_and_add_view_adds_a_tab() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.expect("general is open");
    let order = |h: &Ui1| h.app.channel_tabs(cid);
    assert_eq!(order(&h), ["Channel", "List", "Board", "Calendar"]);
    let x = |h: &Ui1, n: &str| h.rect(&format!("Tab {}", n)).center().x;
    assert!(x(&h, "List") < x(&h, "Board") && x(&h, "Board") < x(&h, "Calendar"));

    // Calendar dropped on the left half of List goes before it
    let list = h.rect("Tab List");
    let from = h.rect("Tab Calendar").center();
    h.drag_by(from, pos2(list.left() + 3.0, list.center().y));
    assert_eq!(order(&h), ["Channel", "Calendar", "List", "Board"]);
    assert!(x(&h, "Calendar") < x(&h, "List"), "drawn in the new order");
    assert_eq!(h.app.chat.open, Some(cid), "a drag doesn't open the view");
    assert_eq!(h.app.section, Section::Chat);

    // the Channel tab moves too, here to the end
    let board = h.rect("Tab Board");
    let from = h.rect("Tab Channel").center();
    h.drag_by(from, pos2(board.right() + 20.0, board.center().y));
    assert_eq!(order(&h), ["Calendar", "List", "Board", "Channel"]);

    // "+ View" adds a tab and stays in the channel; one already there is greyed
    h.click(tr("+  View"));
    h.click(tr("Gantt"));
    assert_eq!(order(&h), ["Calendar", "List", "Board", "Channel", "Gantt"]);
    assert_eq!(h.app.section, Section::Chat, "still in the channel");
    assert_eq!(h.app.chat.open, Some(cid));
    assert!(h.has("Tab Gantt"));

    // a right click takes a view tab off again
    h.right_click("Tab Gantt", 4.0);
    h.click(tr("Remove"));
    assert_eq!(order(&h), ["Calendar", "List", "Board", "Channel"]);
    assert!(!h.has("Tab Gantt"));

    // a click on a tab still opens its view
    h.click("Tab Board");
    h.app.apply_actions();
    h.settle();
    assert!(h.app.view == ViewKind::Board);

    // other channels keep the usual row; the order survives a save
    assert!(h.app.channel_tabs(cid + 1) == crate::app::chat::CHANNEL_TABS);
    let json = serde_json::to_string(&h.app.data).unwrap();
    let back: crate::model::Data = serde_json::from_str(&json).unwrap();
    assert_eq!(
        back.extra.channel_tabs[&cid],
        ["Calendar", "List", "Board", "Channel"]
    );
}

fn att(name: &str, kind: MediaKind) -> Attachment {
    Attachment {
        file: format!("{:0>64}", name.len()),
        name: name.into(),
        kind,
        bytes: 1000,
        w: 800,
        h: 600,
        secs: 0.0,
        thumb: None,
    }
}

/// #general with a picture, a file, a link, a plain message whose thread
/// has a link, and a thread reply with a file. Returns the channel and the
/// plain message's id.
fn files_and_links_channel(h: &mut Ui1) -> (u64, u64) {
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let me = h.app.data.me;
    let now = now_ts();
    let mut msg = |h: &mut Ui1, text: &str, ts: i64| {
        crate::app::slack::new_message(h.app.data.new_id(), me, text.into(), ts)
    };
    let mut pic = msg(h, "From the beach", now);
    pic.attachments.push(att("beach.jpg", MediaKind::Image));
    let mut file = msg(h, "Here is the plan", now + 1);
    file.attachments.push(att("plan.pdf", MediaKind::File));
    let link = msg(h, "The spec is at https://example.com/spec", now + 2);
    let mut plain = msg(h, "Lunch at noon", now + 3);
    plain
        .replies
        .push(msg(h, "Menu: https://example.com/menu", now + 4));
    let mut notes = msg(h, "My notes", now + 5);
    notes.attachments.push(att("notes.md", MediaKind::File));
    plain.replies.push(notes);
    let plain_id = plain.id;
    let ch = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap();
    ch.messages.extend([pic, file, link, plain]);
    h.settle();
    (cid, plain_id)
}

/// A row of the panel on the right, by the words on it.
/// Picks one of a conversation's tools: its own button in the header, or
/// else the row in the header's menu.
fn panel_row(h: &mut Ui1, label: &str) {
    if !h.has(label) {
        h.click("Channel tools");
    }
    h.click(label);
}

fn shown(h: &Ui1, text: &str) -> bool {
    !h.painted_text(text).is_empty()
}

/// Files in a conversation's panel gathers its files in place of the
/// messages: pictures and videos as a grid, other files as a list, thread
/// replies' files too. A picture opens large, a file's "Show in
/// conversation" goes to its message (a reply's opens its thread).
#[test]
fn files_shows_the_conversations_files_in_one_place() {
    let mut h = Ui1::new();
    let (_, plain) = files_and_links_channel(&mut h);
    assert!(shown(&h, "Lunch at noon"));

    panel_row(&mut h, "Files");
    assert_eq!(h.app.channel_panel, crate::app::chat::ChannelPanel::Files);
    assert!(
        h.has("Messages with files  \u{00D7}"),
        "named over the list"
    );
    assert!(
        !shown(&h, "Lunch at noon") && !shown(&h, "Here is the plan"),
        "no messages"
    );
    assert!(h.has("Picture beach.jpg"), "the picture in the grid");
    assert!(!h.has("File plan.pdf"), "the other files wait on their tab");
    h.click("Picture beach.jpg");
    assert!(h.app.viewer.is_some(), "opens large");
    h.key(egui::Key::Escape);
    assert!(h.app.viewer.is_none());

    h.click("Other files  2");
    assert!(h.has("File plan.pdf"), "a message's file");
    assert!(h.has("File notes.md"), "and a thread reply's");
    assert!(!h.has("Picture beach.jpg"));
    // nothing drawn over anything, nor cut off, in the smallest window too
    for size in [h.size, vec2(960.0, 560.0)] {
        h.size = size;
        h.settle();
        let page = Rect::from_min_max(pos2(320.0, 40.0), h.size.to_pos2());
        let over = super::layout_tests::overlapping(&h.texts_in(page, Seen::Visible));
        assert!(over.is_empty(), "{:?}: {:?}", size, over);
        assert!(
            h.cut_texts(page).is_empty(),
            "{:?}: {:?}",
            size,
            h.cut_texts(page)
        );
    }
    h.size = vec2(1440.0, 900.0);
    h.settle();
    // newest first: the reply's file above the message's
    let notes = h.rect("File notes.md");
    assert!(notes.top() < h.rect("File plan.pdf").top());
    let go = h
        .painted_text("Show in conversation")
        .into_iter()
        .map(|(r, _)| r.center())
        .find(|p| notes.y_range().contains(p.y))
        .expect("beside the reply's file");
    h.click_at(go);
    assert_eq!(
        h.app.channel_panel,
        crate::app::chat::ChannelPanel::All,
        "every message again"
    );
    assert_eq!(
        h.app.chat.thread_open,
        Some(plain),
        "the reply's thread opens"
    );
    assert!(shown(&h, "Lunch at noon"));
}

/// Links shows the messages with a web address, and the ones whose thread
/// has one; clicking the row again shows them all.
#[test]
fn links_shows_messages_with_web_addresses() {
    let mut h = Ui1::new();
    files_and_links_channel(&mut h);
    panel_row(&mut h, "Links");
    assert_eq!(h.app.channel_panel, crate::app::chat::ChannelPanel::Links);
    assert!(h.has("Messages with links  \u{00D7}"));
    assert!(shown(&h, "The spec is at https://example.com/spec"));
    assert!(shown(&h, "Lunch at noon"), "a link in its thread");
    assert!(!shown(&h, "Here is the plan") && !shown(&h, "From the beach"));
    panel_row(&mut h, "Links");
    assert_eq!(h.app.channel_panel, crate::app::chat::ChannelPanel::All);
    assert!(shown(&h, "Here is the plan"));
}

/// A conversation with no files or links says so under Files and Links.
#[test]
fn files_and_links_say_when_there_are_none() {
    let mut h = Ui1::new();
    h.click("general");
    panel_row(&mut h, "Files");
    assert!(shown(&h, "No files in this conversation yet"));
    panel_row(&mut h, "Links");
    assert!(shown(&h, "No links in this conversation yet"));
}

/// The words painted on `bg` in the last frame: the find bar's yellow
/// (`FOUND_BG`) or the orange of the match it is at (`FOUND_AT_BG`).
fn lit(h: &Ui1, bg: Color32) -> Vec<String> {
    h.shapes_where(|s| match s {
        egui::Shape::Text(ts) => Some(
            ts.galley
                .job
                .sections
                .iter()
                .filter(|x| x.format.background == bg)
                .map(|x| ts.galley.job.text[x.byte_range.clone()].to_string())
                .collect::<Vec<_>>(),
        ),
        _ => None,
    })
    .concat()
}

/// The text of the message the channel's find bar is at.
fn found_at(h: &Ui1) -> Option<String> {
    let (cid, _, (mid, _)) = h.app.chat.find_at.clone()?;
    let c = h.app.data.channels.iter().find(|c| c.id == cid)?;
    Some(c.messages.iter().find(|m| m.id == mid)?.text.clone())
}

/// "has:file", "has:link" and "from:name" typed in the channel's Find box
/// say which messages it finds; every message keeps showing.
#[test]
fn find_in_channel_counts_by_files_links_and_writer() {
    use crate::app::chat::{FOUND_AT_BG, FOUND_BG};
    let mut h = Ui1::new();
    files_and_links_channel(&mut h);
    // opened from the panel with the cursor in it
    panel_row(&mut h, "Find in channel");
    h.type_text("has:file");
    h.settle();
    assert_eq!(h.app.channel_find.as_deref(), Some("has:file"));
    assert!(shown(&h, "2/2"), "two messages with files, at the newest");
    assert_eq!(found_at(&h).as_deref(), Some("Here is the plan"));
    assert!(
        shown(&h, "Here is the plan") && shown(&h, "From the beach") && shown(&h, "Lunch at noon"),
        "nothing is hidden"
    );
    for _ in 0.."has:file".len() {
        h.key(egui::Key::Backspace);
    }
    h.type_text("has:link spec");
    h.settle();
    // "spec" twice in the one message with a link: the words, then its address
    assert!(shown(&h, "2/2"));
    assert!(shown(&h, "The spec is at https://example.com/spec"));
    assert!(shown(&h, "Here is the plan") && shown(&h, "Lunch at noon"));
    assert_eq!(lit(&h, FOUND_AT_BG), ["spec"]);
    assert_eq!(lit(&h, FOUND_BG), ["spec"]);
    for _ in 0.."has:link spec".len() {
        h.key(egui::Key::Backspace);
    }
    h.type_text("from:nobody");
    h.settle();
    assert!(shown(&h, "0/0"));
    assert!(shown(&h, "Here is the plan") && shown(&h, "Lunch at noon"));
    assert!(lit(&h, FOUND_AT_BG).is_empty() && lit(&h, FOUND_BG).is_empty());
}

/// The channel's find bar works as a web browser's: it sits at the right of
/// the row over the messages, beside the panel. Every message keeps
/// showing, the words found are lit up (the one it is at in orange) and
/// "2/3" says where it is. The arrows, Enter and Shift+Enter go from one
/// match to the next, round past either end, and a match far back in the
/// conversation (before "Show earlier messages") comes into sight.
#[test]
fn find_bar_goes_through_the_matches_like_a_browser() {
    use crate::app::chat::{FOUND_AT_BG, FOUND_BG};
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let me = h.app.data.me;
    let now = now_ts();
    let mut msg = |h: &mut Ui1, text: &str, ts: i64| {
        crate::app::slack::new_message(h.app.data.new_id(), me, text.into(), ts)
    };
    // the first match, more than a page of other messages, then two more
    let mut msgs = vec![msg(&mut h, "a quokka at the start", now - 1000)];
    for i in 0..250 {
        msgs.push(msg(&mut h, &format!("filler {i}"), now - 900 + i));
    }
    msgs.push(msg(&mut h, "two quokka sightings, QUOKKA again", now - 10));
    msgs.push(msg(&mut h, "nothing here", now - 5));
    let ch = h.app.data.channels.iter_mut().find(|c| c.id == cid);
    ch.unwrap().messages.extend(msgs);
    h.settle();

    // at the right of the row with the bookmarks chip, before the panel
    h.key_with(egui::Key::F, egui::Modifiers::COMMAND);
    let close = h.rect("Close find");
    let row = h.rect("Add bookmarks, notes, and more");
    assert!(
        (close.center().y - row.center().y).abs() < 6.0,
        "in the chips' row: {close:?} {row:?}"
    );
    // (no panel beside the messages any more: the tools are in the header)
    let header = h.rect("Followers");
    assert!(
        close.right() <= h.size.x && close.right() > header.left() - 40.0,
        "at the row's right, under the header's tools: {close:?} {header:?}"
    );

    h.type_text("quokka");
    h.settle();
    assert!(shown(&h, "3/3"), "at the newest, where the list is");
    assert!(
        shown(&h, "nothing here") && shown(&h, "filler 249"),
        "nothing is hidden"
    );
    assert_eq!(lit(&h, FOUND_AT_BG), ["QUOKKA"], "whatever the case");
    assert!(lit(&h, FOUND_BG).contains(&"quokka".to_string()));

    // up a match, in the same message
    h.click("Previous match");
    assert!(shown(&h, "2/3"));
    assert_eq!(lit(&h, FOUND_AT_BG), ["quokka"]);
    assert!(lit(&h, FOUND_BG).contains(&"QUOKKA".to_string()));

    // Shift+Enter (the cursor stayed in the box): up to the first, far back
    h.key_with(egui::Key::Enter, egui::Modifiers::SHIFT);
    h.settle();
    assert!(shown(&h, "1/3"));
    assert_eq!(found_at(&h).as_deref(), Some("a quokka at the start"));
    assert!(shown(&h, "a quokka at the start"), "brought into sight");
    assert_eq!(lit(&h, FOUND_AT_BG), ["quokka"]);
    assert_eq!(h.app.channel_find.as_deref(), Some("quokka"), "still open");

    // Enter and the down arrow: on to the newest, then round to the first
    h.key(egui::Key::Enter);
    h.settle();
    assert!(shown(&h, "2/3"));
    assert!(shown(&h, "two quokka sightings, QUOKKA again"));
    h.click("Next match");
    assert!(shown(&h, "3/3"));
    h.click("Next match");
    h.settle();
    assert!(shown(&h, "1/3"), "round past the end");
    assert!(shown(&h, "a quokka at the start"));

    // a match coming in doesn't move it
    let late = msg(&mut h, "a late quokka", now);
    let ch = h.app.data.channels.iter_mut().find(|c| c.id == cid);
    ch.unwrap().messages.push(late);
    h.settle();
    assert!(shown(&h, "1/4"));
    assert_eq!(found_at(&h).as_deref(), Some("a quokka at the start"));

    // × closes it: nothing is lit any more
    h.click("Close find");
    h.settle();
    assert_eq!(h.app.channel_find, None);
    assert!(lit(&h, FOUND_AT_BG).is_empty() && lit(&h, FOUND_BG).is_empty());
}

/// Search finds messages by "has:file", "has:link" and "from:name" (thread
/// replies too), lists only messages then, and a result opens the
/// conversation at its message.
#[test]
fn search_narrows_messages_and_opens_one() {
    let mut h = Ui1::new();
    let (cid, plain) = files_and_links_channel(&mut h);
    let titles = |h: &Ui1, q: &str| -> Vec<String> {
        h.app.search_rows(q, 0).into_iter().map(|r| r.2).collect()
    };
    let files = titles(&h, "has:file");
    assert!(files.iter().any(|t| t == "Here is the plan"), "{:?}", files);
    assert!(
        files.iter().any(|t| t == "My notes"),
        "a reply: {:?}",
        files
    );
    assert!(!files.iter().any(|t| t == "Lunch at noon"), "{:?}", files);
    assert!(
        h.app
            .search_rows("has:file", 0)
            .iter()
            .all(|r| r.0 == "Messages"),
        "only messages"
    );
    let me = h.app.data.member(h.app.data.me).unwrap().name.clone();
    let first = me.split_whitespace().next().unwrap().to_lowercase();
    let mine = titles(&h, &format!("has:link from:{}", first));
    assert!(mine.iter().any(|t| t.starts_with("Menu:")), "{:?}", mine);
    assert!(titles(&h, "has:link from:nobody").is_empty());
    assert!(titles(&h, "has:link from:me spec")
        .iter()
        .all(|t| t.contains("spec")));

    // typed in the box and picked: at the reply, in its thread
    h.app.chat.open = None;
    h.app.section = Section::Home;
    h.app.palette = Some(String::new());
    h.settle();
    h.type_text("has:file notes");
    h.settle();
    assert!(h.has("My notes"), "listed");
    h.key(egui::Key::Enter);
    assert!(h.app.palette.is_none());
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat.open, Some(cid));
    assert_eq!(h.app.chat.thread_open, Some(plain));
}

/// Cmd+F in a conversation opens its find box with the cursor in it: what is
/// typed next is found (every message still showing). Pressed again (the
/// box already open, the cursor elsewhere) it comes back to the box with its
/// words selected, and Escape there closes it.
#[test]
fn cmd_f_finds_in_the_open_conversation() {
    let shown = |h: &Ui1, text: &str| !h.painted_text(text).is_empty();
    let mut h = Ui1::new();
    h.click("general");
    h.send_message("apples for the team");
    h.type_text("bananas tomorrow");
    h.key(egui::Key::Enter);
    assert!(shown(&h, "apples for the team") && shown(&h, "bananas tomorrow"));

    h.key_with(egui::Key::F, egui::Modifiers::COMMAND);
    assert_eq!(
        h.app.channel_find.as_deref(),
        Some(""),
        "the find box opens"
    );
    h.type_text("apple");
    assert_eq!(
        h.app.channel_find.as_deref(),
        Some("apple"),
        "typed into it"
    );
    assert_eq!(h.app.chat.channel.text, "", "not into the composer");
    assert_eq!(found_at(&h).as_deref(), Some("apples for the team"));
    assert!(
        shown(&h, "bananas tomorrow"),
        "the rest still shows, as in a browser"
    );

    // again from the composer: back to the box, its words replaced by typing
    h.click("Message composer");
    h.key_with(egui::Key::F, egui::Modifiers::COMMAND);
    h.type_text("banana");
    assert_eq!(h.app.channel_find.as_deref(), Some("banana"));
    assert_eq!(h.app.chat.channel.text, "");
    assert_eq!(found_at(&h).as_deref(), Some("bananas tomorrow"));
    assert!(shown(&h, "apples for the team"));

    // Escape closes it, and the conversation stays open
    h.key(egui::Key::Escape);
    assert_eq!(h.app.channel_find, None, "the find box closes");
    assert!(h.app.chat.open.is_some());
    assert!(shown(&h, "apples for the team") && shown(&h, "bananas tomorrow"));

    // in a direct message as well
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct)
        .map(|c| c.id)
        .expect("a direct message in the sample data");
    h.app.chat.open = Some(dm);
    h.settle();
    h.key_with(egui::Key::F, egui::Modifiers::COMMAND);
    h.type_text("zzz");
    assert_eq!(h.app.channel_find.as_deref(), Some("zzz"));

    // and the shortcuts list names it
    h.app.open_settings("Keyboard shortcuts");
    h.settle();
    assert!(shown(&h, &crate::os::keys("\u{2318}F")));
    assert!(shown(&h, "Find in a conversation"));
}

/// On a computer, opening a conversation puts the cursor in its message box
/// (a thread's in its reply box), so typing goes straight in, as in Slack.
/// A click elsewhere takes it out and it stays out until something is opened
/// again.
#[test]
fn opening_a_conversation_puts_the_cursor_in_its_box() {
    let chat_box = egui::Id::new(super::super::files::CHAT_BOX);
    let thread_box = egui::Id::new(super::super::files::THREAD_BOX);
    let focused = |h: &Ui1| h.ctx.memory(|m| m.focused());
    let mut h = Ui1::new();
    let mut random = h.app.data.channels[0].clone();
    random.id = 7_777;
    random.name = "random".into();
    random.messages.clear();
    h.app.data.channels.push(random);

    h.click("general");
    assert_eq!(focused(&h), Some(chat_box), "the channel's box");
    h.type_text("typed right away");
    assert_eq!(h.app.chat.channel.text, "typed right away");

    h.click("random");
    assert_eq!(h.app.chat.open, Some(7_777));
    assert_eq!(focused(&h), Some(chat_box), "another channel: its box");

    // a click on the messages takes the cursor out, and it stays out
    h.click("general");
    let messages = h.find("Message composer").unwrap() - vec2(0.0, 300.0);
    h.click_at(messages);
    h.settle();
    assert_eq!(focused(&h), None);

    // a thread: its reply box; closing it, the channel's again
    h.click("1 reply");
    assert_eq!(focused(&h), Some(thread_box), "the thread's reply box");
    h.key(egui::Key::Escape);
    assert_eq!(h.app.chat.thread_open, None);
    assert_eq!(focused(&h), Some(chat_box), "back to the channel's box");

    // back to Chat from another page
    h.click("Rail Home");
    assert_ne!(h.app.section, Section::Chat);
    h.click("general");
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(focused(&h), Some(chat_box), "Chat again: the box");
}

/// On an iPad the cursor doesn't jump into the box: the keyboard would
/// come up over the conversation just from looking at it.
#[test]
fn on_an_ipad_opening_a_conversation_leaves_the_keyboard_down() {
    let mut h = Ui1::new();
    h.app.touch.enabled = true;
    h.click("general");
    h.settle();
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.ctx.memory(|m| m.focused()), None);
}

/// A message by someone with a long name: the name is shortened with "…"
/// and the time stays beside it, in the window (the name pushed the time
/// and the rest of the row past a narrow window's edge).
#[test]
fn a_long_author_name_is_shortened_before_the_time() {
    for w in [360.0, 460.0, 700.0, 960.0] {
        let mut h = Ui1::sized(w, 640.0);
        let general = h.channel_id("general");
        let author = h
            .app
            .data
            .channels
            .iter()
            .find(|c| c.id == general)
            .unwrap()
            .messages
            .iter()
            .rev()
            .find(|m| m.author != h.app.data.me)
            .unwrap()
            .author;
        let long = "Alexandra Konstantinopoulou-Whitfield of the Product Design Team";
        h.app
            .data
            .members
            .iter_mut()
            .find(|m| m.id == author)
            .unwrap()
            .name = long.into();
        h.app.section = Section::Chat;
        h.app.chat.open = Some(general);
        h.settle();
        let past = h.past_right_edge();
        assert!(past.is_empty(), "{}: past the right edge {:?}", w, past);
        let name = h
            .painted_text(long)
            .first()
            .map(|(r, _)| *r)
            .unwrap_or_else(|| panic!("{}: the name is shown", w));
        assert!(
            name.right() < w - 60.0,
            "{}: room for the time {:?}",
            w,
            name
        );
    }
}

/// A long name in the message box's @ list is shortened with "…" inside the
/// list (it ran past the list's right edge, over the messages behind it).
#[test]
fn a_long_name_in_the_mention_list_stays_inside_it() {
    let long = "Alexandra Konstantinopoulou-Whitfield of the Product Design Team";
    for (w, hh) in [(1440.0, 900.0), (960.0, 560.0)] {
        let mut h = Ui1::sized(w, hh);
        let me = h.app.data.me;
        h.app
            .data
            .members
            .iter_mut()
            .find(|m| m.id != me)
            .unwrap()
            .name = long.into();
        let general = h.channel_id("general");
        h.app.section = Section::Chat;
        h.app.chat.open = Some(general);
        h.settle();
        h.click("Mention someone");
        let list = h
            .ctx
            .memory(|m| m.area_rect(egui::Id::new("chat_mention_popup")))
            .expect("the @ list is open");
        let window = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
        let name: Vec<Rect> = h
            .texts_in(window, Seen::All)
            .into_iter()
            .filter(|(t, r)| {
                t.starts_with("Alexandra") && (list.top()..=list.bottom()).contains(&r.center().y)
            })
            .map(|(_, r)| r)
            .collect();
        assert!(!name.is_empty(), "{}: the name is listed", w);
        for r in name {
            assert!(
                r.right() <= list.right() - 4.0,
                "{}: {:?} past {:?}",
                w,
                r,
                list
            );
        }
    }
}

/// "/" while no text field has the cursor puts it back in the message box
/// (a thread's reply box while one is open) with the "/" typed there, so the
/// command list opens, as in Slack. A "/" typed in another field stays there.
#[test]
fn slash_puts_the_cursor_back_in_the_box() {
    let chat_box = egui::Id::new(super::super::files::CHAT_BOX);
    let thread_box = egui::Id::new(super::super::files::THREAD_BOX);
    let focused = |h: &Ui1| h.ctx.memory(|m| m.focused());
    let mut h = Ui1::new();
    h.click("general");
    // a click on the messages takes the cursor out
    let messages = h.find("Message composer").unwrap() - vec2(0.0, 300.0);
    h.click_at(messages);
    h.settle();
    assert_eq!(focused(&h), None);

    h.type_text("/");
    h.settle();
    assert_eq!(focused(&h), Some(chat_box), "back in the box");
    assert_eq!(h.app.chat.channel.text, "/");
    assert!(
        h.app.slash_choices(&h.app.chat.channel.text).len() > 0,
        "the command list has something to show"
    );
    h.type_text("remind");
    assert_eq!(h.app.chat.channel.text, "/remind", "typing goes on there");

    // a thread open: its reply box
    h.app.chat.channel.text.clear();
    h.click("1 reply");
    let messages = h.find("Message composer").unwrap() - vec2(0.0, 300.0);
    h.click_at(messages);
    h.settle();
    assert_eq!(focused(&h), None);
    h.type_text("/");
    h.settle();
    assert_eq!(focused(&h), Some(thread_box), "the thread's reply box");
    assert_eq!(h.app.chat.channel.text, "", "not the channel's box");
}

/// A conversation's tools are in its header, not a panel beside the
/// messages: Followers, Find, the kinds of messages I picked as buttons
/// (Pinned and Files at first), the menu with every kind and Settings. The
/// menu's checkboxes choose which kinds have a button, kept for every
/// channel and direct message alike.
#[test]
fn the_header_has_the_conversation_tools_i_pick() {
    use crate::app::chat::ChannelPanel;
    let mut h = Ui1::new();
    h.click("general");
    for name in [
        "Followers",
        "Find in channel",
        "Pinned messages",
        "Files",
        "Channel tools",
        "Channel settings",
    ] {
        assert!(h.has(name), "{name} in the header");
    }
    let header = h.rect("Channel tools");
    assert!(
        h.rect("Files").center().y == header.center().y,
        "in one row"
    );
    assert!(!h.has("Links"), "a kind not picked is only in the menu");
    // the menu shows a kind that has no button of its own
    h.click("Channel tools");
    h.click("Links");
    assert_eq!(h.app.channel_panel, ChannelPanel::Links);
    assert!(
        h.has("Messages with links  \u{00D7}"),
        "the chip over the messages says so"
    );
    // a header button shows its kind, and clicked again shows everything
    h.click("Pinned messages");
    assert_eq!(h.app.channel_panel, ChannelPanel::Pinned);
    h.click("Pinned messages");
    assert_eq!(h.app.channel_panel, ChannelPanel::All);

    // Links gets a button, Files loses its own
    h.click("Channel tools");
    h.click("Show links in the header");
    h.click("Show files in the header");
    h.key(egui::Key::Escape);
    h.settle();
    assert!(h.has("Links") && !h.has("Files"));
    assert!(h.rect("Links").center().y == header.center().y);
    let mut views = h.app.data.extra.chat_header_views.clone();
    views.sort();
    assert_eq!(views, ["links", "pinned"]);

    // a direct message has the same header, with my choice
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    h.app.chat.open = Some(dm);
    h.settle();
    for name in [
        "Followers",
        "Find in channel",
        "Pinned messages",
        "Links",
        "Channel tools",
        "Channel settings",
    ] {
        assert!(h.has(name), "{name} in the direct message's header");
    }
    assert!(!h.has("Files"));
    // Find and Settings from the header
    h.click("Find in channel");
    assert_eq!(h.app.channel_find.as_deref(), Some(""));
    h.click("Channel settings");
    assert!(matches!(h.app.modal, Some(Modal::ChannelSettings(c)) if c == dm));
}
