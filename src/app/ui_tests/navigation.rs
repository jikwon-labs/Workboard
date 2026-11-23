// Click-through tests for getting around: the rail, sidebar, search, Home,
// Inbox, reminders, settings, notifications and keyboard shortcuts.

use super::*;

#[test]
fn click_sidebar_inbox_and_my_tasks() {
    let mut h = Ui1::new();
    h.click("Inbox");
    assert_eq!(h.app.loc, Location::Inbox);
    h.click("My Tasks");
    assert_eq!(h.app.loc, Location::MyTasks);
    h.click("Today & Overdue");
    assert_eq!(h.app.loc, Location::TodayOverdue);
    h.click("Personal List");
    let pl = h
        .app
        .personal_list_id()
        .expect("personal list created by the click");
    assert_eq!(h.app.loc, Location::List(pl));
}

#[test]
fn click_workspace_menu_people_and_profile_themes() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("People");
    assert_eq!(h.app.section, Section::People);
    h.click("Profile menu");
    h.click("Settings");
    h.click("Settings Themes");
    h.click("Dark");
    assert!(h.app.data.dark, "dark theme switched on");
}

#[test]
fn click_hide_recent_tasks_row_and_turn_it_back_on_in_themes() {
    let mut h = Ui1::new();
    h.click("Search");
    h.type_text("passport");
    h.key(egui::Key::Enter);
    assert!(h.app.open_task.is_some());
    h.key(egui::Key::Escape);
    h.settle();
    assert!(
        h.has("Hide recent tasks"),
        "the opened task shows in the bottom row"
    );
    h.click("Hide recent tasks");
    assert!(h.app.data.extra.hide_tray);
    h.settle();
    assert!(!h.has("Hide recent tasks"), "the bottom row is gone");
    h.click("Profile menu");
    h.click("Settings");
    h.click("Settings Themes");
    assert!(
        !h.painted_text("Tasks you open line up along the bottom of the window, so you can open them again in one click.").is_empty(),
        "the box says what the row is"
    );
    h.click("Show the recent tasks row");
    assert!(!h.app.data.extra.hide_tray);
    h.click("\u{2190}  Back to Workspace");
    h.settle();
    assert!(h.has("Hide recent tasks"), "the bottom row is back");
}

#[test]
fn click_search_and_open_result() {
    let mut h = Ui1::new();
    h.click("Search");
    assert!(h.app.palette.is_some());
    h.type_text("passport");
    h.key(egui::Key::Enter);
    let id = h.app.open_task.expect("Enter opens the first match");
    assert_eq!(h.app.task(id).unwrap().name, "Renew passport");
}

/// The colour of each glyph of the painted text `text` (the first one
/// found), as the screen shows it: (letter, colour).
fn glyph_colors(h: &Ui1, text: &str) -> Vec<(char, Color32)> {
    let mut out = None;
    each_text(&h.shapes, |ts, _| {
        if out.is_some() || ts.galley.text() != text {
            return;
        }
        let mut v = vec![];
        for row in &ts.galley.rows {
            let mut at = row.visuals.glyph_vertex_range.start;
            for g in &row.glyphs {
                if g.uv_rect.is_nothing() {
                    continue;
                }
                let c = row.visuals.mesh.vertices[at].color;
                let c = if let Some(o) = ts.override_text_color {
                    o
                } else if c == Color32::PLACEHOLDER {
                    ts.fallback_color
                } else {
                    c
                };
                v.push((g.chr, c));
                at += 4;
            }
        }
        out = Some(v);
    });
    out.unwrap_or_else(|| panic!("no painted text {:?}", text))
}

#[test]
fn an_opened_search_result_shows_the_searched_words_in_orange_for_two_seconds() {
    let mut h = Ui1::new();
    h.click("Search");
    h.type_text("PASSport");
    h.key(egui::Key::Enter);
    h.settle();
    assert!(h.app.open_task.is_some());
    assert_eq!(crate::find_flash::lit(&h.ctx).as_deref(), Some("passport"));
    // the task window's name: the searched letters orange, whatever their case
    let colors = glyph_colors(&h, "Renew passport");
    let orange: String = colors
        .iter()
        .filter(|(_, c)| *c == WARN_ORANGE)
        .map(|(ch, _)| *ch)
        .collect();
    assert_eq!(orange, "passport");
    let own = colors[0].1;
    assert_ne!(own, WARN_ORANGE, "the rest keeps its colour");
    // two seconds later the words are back in the text's own colour
    for _ in 0..42 {
        h.frame(vec![]);
    }
    assert!(crate::find_flash::lit(&h.ctx).is_none());
    assert!(glyph_colors(&h, "Renew passport")
        .iter()
        .all(|(_, c)| *c == own));
}

#[test]
fn a_message_from_the_search_opens_the_chat_at_it_lit_up() {
    let mut h = Ui1::new();
    let text = "Reminder: newsletter draft is due Thursday.";
    h.click("Search");
    h.type_text("newsletter draft");
    h.click(text);
    h.settle();
    assert_eq!(h.app.section, Section::Chat);
    assert!(
        h.app.chat.flash.is_some(),
        "the message row is lit, as from a post"
    );
    let orange: String = glyph_colors(&h, text)
        .iter()
        .filter(|(_, c)| *c == WARN_ORANGE)
        .map(|(ch, _)| *ch)
        .collect();
    assert_eq!(
        orange, "newsletterdraft",
        "the searched words, not the spaces"
    );
}

#[test]
fn a_search_result_opened_without_typing_lights_nothing() {
    let mut h = Ui1::new();
    h.click("Search");
    h.key(egui::Key::Enter);
    h.settle();
    assert!(crate::find_flash::lit(&h.ctx).is_none());
}

#[test]
fn click_back_and_forward() {
    let mut h = Ui1::new();
    let start = h.app.loc;
    h.click("Inbox");
    h.click("Replies");
    h.click("Go back");
    assert_eq!(h.app.loc, Location::Inbox);
    h.click("Go back");
    assert_eq!(h.app.loc, start);
    h.click("Go forward");
    assert_eq!(h.app.loc, Location::Inbox);
}

#[test]
fn click_inbox_item_later_and_clear() {
    let mut h = Ui1::new();
    h.click("Inbox");
    // hover the first inbox item to reveal its buttons
    let row = h
        .find("Inbox Primary")
        .map(|p| pos2(700.0, p.y + 120.0))
        .unwrap();
    h.hover(row);
    h.click("Later");
    assert_eq!(h.app.data.extra.inbox_later.len(), 1);
    h.click("Inbox Later");
    h.hover(row);
    h.click("Clear");
    assert_eq!(h.app.data.extra.inbox_cleared.len(), 1);
}

#[test]
fn click_settings_dialogs_open_and_close() {
    let mut h = Ui1::new();
    for item in [
        "Apps",
        "Templates",
        "Custom fields",
        "Automations",
        "Tag manager",
    ] {
        h.click("Workspace menu");
        h.click(item);
        assert!(h.app.modal.is_some(), "{} opens a dialog", item);
        h.key(egui::Key::Escape);
        assert!(h.app.modal.is_none(), "Esc closes {}", item);
    }
    for item in ["Labs", "Help"] {
        h.click("Profile menu");
        h.click(item);
        assert!(h.app.modal.is_some(), "{} opens a dialog", item);
        h.key(egui::Key::Escape);
        assert!(h.app.modal.is_none(), "Esc closes {}", item);
    }
}

#[test]
fn the_profile_menu_leaves_out_rows_settings_already_has_and_tools_that_cannot_run() {
    let mut h = Ui1::new();
    // a pin saved before these tools left the menu draws nothing
    h.app.data.extra.pinned_tools = vec!["create_task".into(), "clip".into(), "people".into()];
    h.settle();
    assert!(!h.has("Record a Clip") && !h.has("View People"));
    h.click("Profile menu");
    for gone in [
        "Notifications",
        "Themes",
        "Keyboard shortcuts",
        "Download app",
        "My Work",
        "Record a Clip",
        "Talk to Text",
        "View People",
        "AI Notetaker",
    ] {
        assert!(!h.has(gone), "{} is not in the menu", gone);
    }
    for kept in [
        "Set status",
        "Settings",
        "Labs",
        "Check for updates",
        "Help",
        "Create Doc",
    ] {
        assert!(h.has(kept), "{} is still in the menu", kept);
    }
    // what left the menu is a section of Settings
    h.click("Settings");
    assert_eq!(h.app.section, Section::Settings);
    for section in ["Notifications", "Themes", "Keyboard shortcuts"] {
        h.click(&format!("Settings {}", section));
        assert_eq!(
            h.app.settings_tab.label(),
            section,
            "{} opens in Settings",
            section
        );
    }
}

#[test]
fn check_for_updates_explains_why_it_cannot_run_outside_the_installed_app() {
    let mut h = Ui1::new();
    h.click("Profile menu");
    h.click("Check for updates");
    // tests don't run inside Workboard.app, so there is no Sparkle to ask
    match &h.app.modal {
        Some(Modal::Unavailable { title, body }) => {
            assert_eq!(title, "Check for updates");
            assert!(body.contains(env!("CARGO_PKG_VERSION")), "{}", body);
        }
        other => panic!("expected an explanation, got {:?}", other.is_some()),
    }
    h.key(egui::Key::Escape);
    assert!(h.app.modal.is_none());
}

#[test]
fn a_waiting_update_puts_a_red_dot_on_the_profile_and_check_for_updates() {
    let mut h = Ui1::new();
    // nothing waiting: no dots
    crate::updater::pretend_update_available(false);
    h.settle();
    assert_eq!(red_dots_in(&h, "Profile menu"), 0);
    h.click("Profile menu");
    assert_eq!(red_dots_in(&h, "Check for updates"), 0);
    h.key(egui::Key::Escape);

    // the server has a newer version
    crate::updater::pretend_update_available(true);
    h.settle();
    assert_eq!(red_dots_in(&h, "Profile menu"), 1, "the profile shows it");
    h.click("Profile menu");
    assert_eq!(
        red_dots_in(&h, "Check for updates"),
        1,
        "the menu row says where to go"
    );
    assert_eq!(red_dots_in(&h, "Settings"), 0, "only that row");
    crate::updater::pretend_update_available(false);
}

/// A team server with a Windows build 99.0.0 put up the way
/// scripts/publish-windows.sh does it: the zip (holding a workboard.exe
/// that says `program`) and appcast-windows.xml naming it.
/// Two new direct messages from someone else, after everything there is read.
fn two_unread_dms(h: &mut Ui1) -> String {
    let me = h.app.data.me;
    let c = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members.len() == 2 && h.app.listed(c))
        .expect("a direct message in the sample workspace");
    let (dm, other) = (c.id, *c.members.iter().find(|m| **m != me).unwrap());
    let name = h.app.data.member(other).unwrap().shown().to_string();
    h.app.mark_read(dm);
    for text in ["one", "two"] {
        let id = h.app.data.new_id();
        let m: Message = serde_json::from_value(serde_json::json!(
            {"id": id, "author": other, "text": text, "ts": now_ts() + 5}
        ))
        .unwrap();
        let ch = h.app.data.channels.iter_mut().find(|c| c.id == dm).unwrap();
        ch.messages.push(m);
    }
    name
}

#[test]
fn unseen_notifications_show_a_red_count_on_the_rail_and_sidebar() {
    let mut h = Ui1::new();
    // Chat isn't in the bar by default, so its messages count on Home
    assert!(!h.app.data.extra.rail_items.iter().any(|r| r == "Chat"));
    let name = two_unread_dms(&mut h);
    h.settle();
    let home = h.app.inbox_badge() + h.app.chat_unread_total();
    assert_eq!(red_badge_in(&h, "Rail Home"), Some(home.to_string()));
    assert_eq!(red_badge_in(&h, &name), Some("2".into()), "beside the DM");
    let inbox = h.app.inbox_badge();
    assert_eq!(
        red_badge_in(&h, "Inbox"),
        (inbox > 0).then(|| inbox.to_string())
    );

    // pinned to the bar, Chat carries the messages and Home the Inbox
    h.app.data.extra.rail_items.push("Chat".into());
    h.settle();
    let chat = h.app.chat_unread_total();
    assert_eq!(red_badge_in(&h, "Rail Chat"), Some(chat.to_string()));
    assert_eq!(
        red_badge_in(&h, "Rail Home"),
        (inbox > 0).then(|| inbox.to_string())
    );

    // reading the conversation takes its count away
    h.click(&name);
    h.settle();
    assert_eq!(red_badge_in(&h, &name), None, "read now");
    assert_eq!(h.app.chat_unread_total(), chat - 2);

    // paused notifications: no red counts anywhere
    two_unread_dms(&mut h);
    h.app.data.extra.muted_until = now_ts() + 3600;
    h.settle();
    for n in ["Rail Home", "Rail Chat", "Inbox", name.as_str()] {
        assert_eq!(red_badge_in(&h, n), None, "{} while muted", n);
    }
}

#[test]
fn clicking_the_direct_messages_heading_folds_them_and_shows_the_count_on_it() {
    let mut h = Ui1::new();
    let name = two_unread_dms(&mut h);
    h.settle();
    h.scroll_into_view("Direct Messages");
    assert!(h.has(&name));
    assert_eq!(red_badge_in(&h, &name), Some("2".into()));
    assert_eq!(red_badge_in(&h, "Direct Messages"), None, "rows carry it");

    h.click("Direct Messages");
    h.settle();
    assert!(!h.has(&name), "the conversations fold away");
    assert!(h
        .app
        .data
        .extra
        .folded_nav
        .iter()
        .any(|x| x == "Direct Messages"));
    let me = h.app.data.me;
    let total: usize = h
        .app
        .data
        .channels
        .iter()
        .filter(|c| c.kind == ChannelKind::Direct && h.app.listed(c))
        .map(|c| h.app.unread_count(c))
        .sum();
    assert!(total >= 2, "{} unread for {}", total, me);
    assert_eq!(
        red_badge_in(&h, "Direct Messages"),
        Some(total.to_string()),
        "the heading carries the count while folded"
    );

    // the heading's "+" stays to start one while folded
    assert!(h.has("Sidebar New message"));

    h.click("Direct Messages");
    h.settle();
    assert!(h.has(&name), "unfolded again");
    assert!(h.app.data.extra.folded_nav.is_empty());
    assert_eq!(red_badge_in(&h, "Direct Messages"), None);
}

#[test]
fn every_sidebar_section_folds_from_its_heading() {
    let mut h = Ui1::new();
    let fav = h.app.data.location_name(h.app.data.favorites[0]);
    // a new message in #general, so Channels has a count to carry
    let me = h.app.data.me;
    let other = h.app.data.members.iter().find(|m| m.id != me).unwrap().id;
    let gid = h.channel_id("general");
    h.app.mark_read(gid);
    let id = h.app.data.new_id();
    let m: Message = serde_json::from_value(serde_json::json!(
        {"id": id, "author": other, "text": "hi", "ts": now_ts() + 5}
    ))
    .unwrap();
    let ch = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == gid)
        .unwrap();
    ch.messages.push(m);
    h.settle();

    // the favorite also shows under its Space; folding Favorites takes one away
    let before = h.count_named(&fav);
    h.scroll_into_view("Favorites");
    h.click("Favorites");
    h.settle();
    assert_eq!(h.count_named(&fav), before - 1, "Favorites folds");

    // each heading with a row only it shows
    for (heading, row) in [
        ("AI Chats", "Ask, Build, Create"),
        ("Spaces", "All Tasks"),
        ("Channels", "general"),
    ] {
        h.scroll_into_view(heading);
        assert!(h.has(row), "{} open", heading);
        h.click(heading);
        h.settle();
        assert!(!h.has(row), "{} folds away {}", heading, row);
    }
    let total: usize = h
        .app
        .data
        .channels
        .iter()
        .filter(|c| c.kind == ChannelKind::Channel && h.app.listed(c) && !h.app.quiet_channel(c))
        .map(|c| h.app.unread_count(c))
        .sum();
    assert!(total >= 1);
    assert_eq!(
        red_badge_in(&h, "Channels"),
        Some(total.to_string()),
        "folded Channels carries the count"
    );

    // a sidebar search still looks inside folded sections
    h.app.side.search = Some("general".into());
    h.settle();
    assert!(h.has("general"), "found while folded");
    h.app.side.search = None;
    h.settle();

    // and each opens again
    for (heading, row) in [
        ("AI Chats", "Ask, Build, Create"),
        ("Spaces", "All Tasks"),
        ("Channels", "general"),
    ] {
        h.scroll_into_view(heading);
        h.click(heading);
        h.settle();
        assert!(h.has(row), "{} open again", heading);
    }
    h.scroll_into_view("Favorites");
    h.click("Favorites");
    h.settle();
    assert_eq!(h.count_named(&fav), before);
    assert!(h.app.data.extra.folded_nav.is_empty());
}

#[test]
fn messages_in_an_open_conversation_stay_unread_while_the_window_is_behind() {
    let mut h = Ui1::new();
    let name = two_unread_dms(&mut h);
    h.settle();
    h.click(&name);
    h.settle();
    let (base, chat) = (h.app.dock_count(), h.app.chat_unread_total());

    // another app comes to the front; the conversation stays open
    h.focused = false;
    h.frame(vec![egui::Event::WindowFocused(false)]);
    two_unread_dms_again(&mut h, &name);
    h.settle();
    assert_eq!(h.app.dock_count(), base + 2, "counted on the Dock");
    assert_eq!(h.app.chat_unread_total(), chat + 2, "and in Chat");

    // coming back to the window reads them
    h.focused = true;
    h.frame(vec![egui::Event::WindowFocused(true)]);
    h.settle();
    assert_eq!(h.app.dock_count(), base);
    assert_eq!(h.app.chat_unread_total(), chat);
}

/// Two more messages from the person `name` in the conversation with them,
/// without marking anything read first.
fn two_unread_dms_again(h: &mut Ui1, name: &str) {
    let me = h.app.data.me;
    let c = h
        .app
        .data
        .channels
        .iter()
        .find(|c| {
            c.kind == ChannelKind::Direct
                && c.members.len() == 2
                && c.members
                    .iter()
                    .any(|m| *m != me && h.app.data.member(*m).is_some_and(|p| p.shown() == name))
        })
        .unwrap();
    let (dm, other) = (c.id, *c.members.iter().find(|m| **m != me).unwrap());
    for text in ["three", "four"] {
        let id = h.app.data.new_id();
        let m: Message = serde_json::from_value(serde_json::json!(
            {"id": id, "author": other, "text": text, "ts": now_ts() + 10}
        ))
        .unwrap();
        let ch = h.app.data.channels.iter_mut().find(|c| c.id == dm).unwrap();
        ch.messages.push(m);
    }
}

#[test]
fn a_big_unread_count_reads_99_plus() {
    let mut h = Ui1::new();
    h.app.data.extra.rail_items.push("Chat".into());
    let me = h.app.data.me;
    let c = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel && h.app.listed(c))
        .unwrap()
        .id;
    let other = h.app.data.members.iter().find(|m| m.id != me).unwrap().id;
    for i in 0..120 {
        let id = h.app.data.new_id();
        let m: Message = serde_json::from_value(serde_json::json!(
            {"id": id, "author": other, "text": format!("m{}", i), "ts": now_ts() + 5}
        ))
        .unwrap();
        let ch = h.app.data.channels.iter_mut().find(|x| x.id == c).unwrap();
        ch.messages.push(m);
    }
    h.settle();
    assert!(h.app.chat_unread_total() > 99);
    assert_eq!(red_badge_in(&h, "Rail Chat"), Some("99+".into()));
}

fn server_with_windows_build(program: &[u8]) -> workboard_server::testing::TestServer {
    use crate::updater::own;
    let server = workboard_server::testing::TestServer::start();
    let updates = server.home.join(workboard_server::UPDATES_DIR);
    let build = server.home.join("build");
    std::fs::create_dir_all(&updates).unwrap();
    std::fs::create_dir_all(&build).unwrap();
    std::fs::write(build.join("workboard.exe"), program).unwrap();
    let zip = updates.join("Workboard-99.0.0-windows-x64.zip");
    let made = std::process::Command::new(own::tar())
        .current_dir(&build)
        .args(["-a", "-cf"])
        .arg(&zip)
        .arg("workboard.exe")
        .status()
        .unwrap();
    assert!(made.success(), "tar made the zip");
    let (sha, len) = wb_net::sha256_file(&zip).unwrap();
    std::fs::write(
        updates.join(own::FEED),
        format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<update>\n  <version>99.0.0</version>\n  <file>Workboard-99.0.0-windows-x64.zip</file>\n  <length>{}</length>\n  <sha256>{}</sha256>\n</update>\n",
            len, sha
        ),
    )
    .unwrap();
    server
}

/// Frames until `name` is on screen, for at most 20 seconds (the updater
/// works on a background thread), then a few more so the dialog has grown
/// to its new size before anything in it is clicked.
fn settle_until(h: &mut Ui1, name: &str) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !h.has(name) {
        assert!(
            std::time::Instant::now() < end,
            "waited for {:?}; the updater says {:?}",
            name,
            crate::updater::own::status()
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
        h.settle();
    }
    h.settle();
    h.settle();
}

#[test]
fn on_windows_check_for_updates_finds_installs_and_swaps_the_program() {
    let server = server_with_windows_build(b"new program");
    let dir = temp_home("winupdate");
    let target = dir.join("workboard.exe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&target, b"old program").unwrap();
    let mut h = Ui1::new();
    crate::updater::own::pretend_windows(&format!("127.0.0.1:{}", server.port), &target);

    h.click("Profile menu");
    h.click("Check for updates");
    assert!(
        matches!(h.app.modal, Some(Modal::Update)),
        "the app's own window"
    );
    let ready = format!(
        "Workboard 99.0.0 is ready to install. You have {}.",
        env!("CARGO_PKG_VERSION")
    );
    settle_until(&mut h, &ready);
    assert!(crate::updater::update_available(), "the red dot comes on");
    assert!(h.has("Later"));

    h.click("Install and restart");
    settle_until(&mut h, "Opening the new version...");
    assert_eq!(std::fs::read(&target).unwrap(), b"new program");
    assert_eq!(
        std::fs::read(dir.join("workboard.old.exe")).unwrap(),
        b"old program",
        "the running program is moved aside, not overwritten"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn on_windows_check_for_updates_says_when_the_server_has_no_windows_build() {
    let server = workboard_server::testing::TestServer::start();
    let dir = temp_home("winupdate-none");
    let mut h = Ui1::new();
    crate::updater::own::pretend_windows(
        &format!("127.0.0.1:{}", server.port),
        &dir.join("workboard.exe"),
    );
    h.click("Profile menu");
    h.click("Check for updates");
    settle_until(
        &mut h,
        "Your team's server has no Windows version to update to yet.",
    );
    assert!(!crate::updater::update_available());
    h.click("Close");
    assert!(h.app.modal.is_none());
}

#[test]
fn an_online_only_tool_says_workboard_has_no_such_service() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Clips");
    // the team server exists now, so the note must not promise that setting
    // one up makes AI, video or voice processing work
    match &h.app.modal {
        Some(Modal::Unavailable { title, body }) => {
            assert_eq!(title, "Clips");
            assert!(
                body.starts_with("Clips needs an online service"),
                "{}",
                body
            );
            assert!(body.contains("Workboard doesn't have"), "{}", body);
            assert!(!body.contains("server"), "{}", body);
        }
        other => panic!("expected an explanation, got {:?}", other.is_some()),
    }
    h.key(egui::Key::Escape);
    assert!(h.app.modal.is_none());
}

#[test]
fn click_every_rail_more_app() {
    let mut h = Ui1::new();
    for (item, sec) in [
        ("Docs", Section::Docs),
        ("Dashboards", Section::Dashboards),
        ("Whiteboards", Section::Whiteboards),
        ("Forms", Section::Forms),
        ("Goals", Section::Goals),
        ("Timesheets", Section::Timesheets),
    ] {
        h.click("Rail More");
        h.click(item);
        assert_eq!(h.app.section, sec, "{} opens", item);
    }
    for item in ["Rail Home", "Rail Planner", "Rail AI", "Rail Teams"] {
        h.click(item);
    }
    assert_eq!(h.app.section, Section::Teams);
}

#[test]
fn a_rail_item_names_itself_beside_the_bar_level_with_its_icon() {
    let mut h = Ui1::new();
    let item = h.rect("Rail Planner");
    let below = h.rect("Rail AI");
    // the pointer comes in from the page (egui only counts it as moving
    // from its third position on)
    for x in [400.0, 300.0, 200.0] {
        h.frame(vec![egui::Event::PointerMoved(pos2(x, item.center().y))]);
    }
    h.frame(vec![egui::Event::PointerMoved(item.center())]);
    h.frame(vec![]);
    assert_eq!(
        h.painted_text("Planner").len(),
        1,
        "only under the icon until the pointer has rested a moment"
    );
    for _ in 0..14 {
        h.frame(vec![]);
    }
    let names = h.painted_text("Planner");
    let tips: Vec<Rect> = names
        .iter()
        .map(|(r, _)| *r)
        .filter(|r| r.left() > item.right())
        .collect();
    assert_eq!(tips.len(), 1, "named beside the bar: {:?}", names);
    let icon_y = item.top() + 22.0;
    assert!(
        (tips[0].center().y - icon_y).abs() < 3.0,
        "level with the icon at {}: {:?}",
        icon_y,
        tips[0]
    );
    assert!(tips[0].left() < item.right() + 40.0, "{:?}", tips[0]);
    assert!(
        names.iter().all(|(r, _)| !r.intersects(below)),
        "nothing over the item below: {:?}",
        names
    );

    // moving on to the next item names that one at once, beside it
    h.frame(vec![egui::Event::PointerMoved(below.center())]);
    h.frame(vec![]);
    h.frame(vec![]);
    assert!(
        h.painted_text("AI")
            .iter()
            .any(|(r, _)| r.left() > below.right()
                && (r.center().y - (below.top() + 22.0)).abs() < 3.0),
        "{:?}",
        h.painted_text("AI")
    );
}

#[test]
fn the_more_menu_opens_beside_the_bar_and_stays_in_the_window() {
    let mut h = Ui1::new();
    let more = h.rect("Rail More");
    h.click("Rail More");
    h.settle();
    let clips = h.rect("Clips");
    assert!(clips.left() > more.right(), "beside the bar: {:?}", clips);
    // (the sidebar has a Spaces heading of its own further down)
    let first = h.rect_in(
        "Spaces",
        Rect::from_min_max(
            pos2(more.right(), more.top() - 100.0),
            pos2(more.right() + 400.0, more.bottom() + 100.0),
        ),
    );
    assert!(
        (first.top() - more.top()).abs() < 24.0,
        "level with More at {}: {:?}",
        more.top(),
        first
    );
    h.key(egui::Key::Escape);

    // with every app pinned More is far down the bar: the menu moves up
    // as far as it must to stay whole in the window
    h.app.data.extra.rail_items = [
        "Planner",
        "AI",
        "Teams",
        "Chat",
        "Docs",
        "Dashboards",
        "Whiteboards",
        "Forms",
        "Goals",
        "Timesheets",
        "Clips",
    ]
    .map(String::from)
    .to_vec();
    h.settle();
    let more = h.rect("Rail More");
    h.click("Rail More");
    h.settle();
    let clips = h.rect("Clips");
    let last = h.rect("Customize navigation");
    assert!(clips.left() > more.right(), "beside the bar: {:?}", clips);
    assert!(
        last.bottom() <= h.size.y - 8.0,
        "whole in the window: {:?}",
        last
    );
    // (the seventh row: level with More it would be far below it)
    assert!(clips.top() < more.top(), "moved up: {:?}", clips);
}

/// Whether a left-bar item is in the part of the bar in view (scrolled-out
/// items are still named, behind More).
fn in_bar(h: &Ui1, label: &str) -> bool {
    let name = format!("Rail {}", label);
    if !h.has(&name) {
        return false;
    }
    let r = h.rect(&name);
    let more = h.rect("Rail More");
    label == "More" || (r.top() >= 0.0 && r.bottom() <= more.top() + 1.0)
}

/// Wheels over the left bar's items, `dy` points at a time.
fn wheel_rail(h: &mut Ui1, dy: f32) {
    h.frame(vec![egui::Event::PointerMoved(pos2(30.0, 150.0))]);
    h.frame(vec![egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, dy),
        modifiers: Default::default(),
    }]);
    h.settle();
}

#[test]
fn a_full_bar_keeps_invite_at_the_bottom_and_scrolls_the_rest() {
    // the smallest window the app allows
    let mut h = Ui1::sized(960.0, 560.0);
    let pinned = [
        "Docs",
        "Dashboards",
        "Whiteboards",
        "Forms",
        "Goals",
        "Timesheets",
        "AI",
        "Teams",
        "Planner",
        "Chat",
    ];
    h.app.data.extra.rail_items = pinned.map(String::from).to_vec();
    h.settle();
    let invite = h.rect("Rail Invite");
    let more = h.rect("Rail More");
    assert!(
        more.bottom() <= invite.top(),
        "More above Invite: {:?} {:?}",
        more,
        invite
    );
    let shown: Vec<&str> = pinned.into_iter().filter(|l| in_bar(&h, l)).collect();
    assert!(
        !shown.is_empty() && shown.len() < pinned.len(),
        "{:?}",
        shown
    );
    assert_eq!(shown, pinned[..shown.len()], "the first ones stay");
    for l in &shown {
        let r = h.rect(&format!("Rail {}", l));
        assert!(r.bottom() <= more.top(), "{} above More: {:?}", l, r);
    }

    // Chat is hidden, so its unread messages count on More
    two_unread_dms(&mut h);
    h.settle();
    let chat = h.app.chat_unread_total();
    assert_eq!(red_badge_in(&h, "Rail More"), Some(chat.to_string()));

    // the hidden ones open from More, Planner too though More doesn't list
    // it otherwise, and then More stands for it
    h.click("Rail More");
    h.click("Planner");
    assert_eq!(h.app.section, Section::Planner);
    h.settle();
    let more = h.rect("Rail More");
    assert_eq!(
        h.circle_fill_at(pos2(more.center().x, more.top() + 22.0)),
        Some(Color32::from_white_alpha(46)),
        "More is lit for the open app"
    );

    // the wheel over the bar brings the last ones into view, above More,
    // and takes the first ones out; the wheel back brings them back
    for _ in 0..10 {
        wheel_rail(&mut h, -120.0);
    }
    assert!(in_bar(&h, "Chat"), "the last app scrolls into view");
    assert!(!in_bar(&h, "Home"), "the first ones scroll out");
    let chat = h.rect("Rail Chat");
    assert!(chat.bottom() <= h.rect("Rail More").top() + 1.0);
    assert!(h.rect("Rail More").bottom() <= h.rect("Rail Invite").top());
    h.click("Rail Chat");
    assert_eq!(h.app.section, Section::Chat, "a scrolled-in app opens");
    for _ in 0..10 {
        wheel_rail(&mut h, 120.0);
    }
    assert!(in_bar(&h, "Home") && !in_bar(&h, "Chat"));

    // a taller window has room for them all again
    h.size = vec2(1440.0, 1200.0);
    h.settle();
    for l in pinned {
        assert!(in_bar(&h, l), "{}", l);
    }
}

/// Home dragged to the bottom of a full bar goes into More's menu in a
/// small window, and opens from there. Clicking it there did nothing.
#[test]
fn home_moved_into_more_still_opens_from_there() {
    let mut h = Ui1::sized(960.0, 560.0);
    h.app.data.extra.rail_items = [
        "Planner",
        "AI",
        "Teams",
        "Docs",
        "Dashboards",
        "Whiteboards",
        "Forms",
        "Home",
    ]
    .map(String::from)
    .to_vec();
    h.settle();
    h.click("Rail Planner");
    assert_eq!(h.app.section, Section::Planner);
    assert!(!in_bar(&h, "Home"), "Home is out of view, in More's menu");
    h.click("Rail More");
    h.click("Home");
    assert_eq!(h.app.section, Section::Home);
}

#[test]
fn the_ai_page_closes_with_its_button_or_escape() {
    let mut h = Ui1::new();
    h.click("Rail Planner");
    h.click("Rail AI");
    assert_eq!(h.app.section, Section::Ai);
    h.click("Close AI");
    assert_eq!(
        h.app.section,
        Section::Planner,
        "the X goes back to the page it was opened from"
    );

    h.click("Rail AI");
    assert_eq!(h.app.section, Section::Ai);
    h.key(egui::Key::Escape);
    assert_eq!(h.app.section, Section::Planner, "Esc does the same");
}

#[test]
fn help_says_where_the_workspace_is_kept() {
    let mut h = Ui1::new();
    h.app.modal = Some(Modal::Help);
    h.settle();
    assert!(
        h.has("This is your own copy of the work manager. Everything is saved on this computer.")
    );
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: "/tmp/wb-help".into(),
        device: "dev-help".into(),
        tag: 1,
        server: None,
    });
    h.settle();
    assert!(h.has("This workspace is shared through a folder your sync app keeps in step. Your copy is saved on this computer and in that folder."));
    h.app.data.extra.shared.as_mut().unwrap().server = Some("127.0.0.1:1".into());
    h.settle();
    assert!(h.has("This workspace is on your team's server. A copy is kept on this computer, so you can keep working while the server can't be reached."));
    assert!(
        !h.has("This is your own copy of the work manager. Everything is saved on this computer.")
    );
}

#[test]
fn single_key_t_opens_create_task_and_cmd_backslash_hides_the_sidebar() {
    let mut h = Ui1::new();
    h.key(egui::Key::T);
    assert!(h.app.quick_add.is_some());
    h.app.quick_add = None;
    h.settle();
    let cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Backslash,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: cmd,
    }]);
    h.settle();
    assert!(h.app.sidebar_collapsed);
}

#[test]
fn settings_is_a_full_page_with_sections() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Settings");
    assert_eq!(h.app.section, Section::Settings);
    h.click("Settings Spaces");
    assert!(h.has("Team Space"));
    h.click("Settings Themes");
    h.click("Auto");
    assert!(h.app.data.extra.auto_theme);
    h.click("\u{2190}  Back to Workspace");
    assert_eq!(h.app.section, Section::Home);
}

#[test]
fn back_to_workspace_returns_to_the_page_settings_was_opened_from() {
    let mut h = Ui1::new();
    h.click("general");
    assert_eq!(h.app.section, Section::Chat);
    let channel = h.app.chat.open;
    assert!(channel.is_some());
    h.click("Workspace menu");
    h.click("Settings");
    h.click("Settings Spaces");
    h.click("Settings Themes");
    h.click("\u{2190}  Back to Workspace");
    assert_eq!(
        h.app.section,
        Section::Chat,
        "back where Settings was opened"
    );
    assert_eq!(h.app.chat.open, channel);
}

#[test]
fn search_shows_recent_items_and_finds_docs() {
    let mut h = Ui1::new();
    h.click("Task Set up analytics dashboard");
    h.key(egui::Key::Escape);
    h.app.palette = Some(String::new());
    h.settle();
    assert!(h.has("Recent"));
    h.click("Docs");
    // (results are named by their title; the doc glyph is drawn beside it)
    h.click_side("Project brief", true);
    assert_eq!(h.app.section, Section::Docs);
}

#[test]
fn search_hides_empty_group_headings() {
    let mut h = Ui1::new();
    assert!(
        !h.app
            .data
            .tasks
            .iter()
            .any(|t| t.name.to_lowercase().contains("gen")),
        "seed check"
    );
    // distinct places on screen with this name
    let count = |h: &Ui1, n: &str| {
        let mut r: Vec<String> = h
            .tree
            .iter()
            .filter(|(x, _)| x == n)
            .map(|(_, r)| format!("{:?}", r))
            .collect();
        r.sort();
        r.dedup();
        r.len()
    };
    h.click("Search");
    h.type_text("a"); // matches tasks and locations
    let tasks_with = count(&h, "Tasks");
    assert!(h.has("Locations"));
    h.app.palette = Some(String::new());
    h.settle();
    h.type_text("gen"); // only the #general channel
    assert_eq!(h.app.palette.as_deref(), Some("gen"));
    assert!(h.has("Channels"));
    assert!(!h.has("Locations"), "no empty Locations heading");
    assert_eq!(
        count(&h, "Tasks"),
        tasks_with - 1,
        "no empty Tasks heading (the Tasks tab stays)"
    );
}

#[test]
fn sidebar_search_filters_spaces_folders_and_lists() {
    let mut h = Ui1::new();
    let side = Rect::from_min_max(pos2(0.0, 0.0), pos2(320.0, 5000.0));
    let in_side = |h: &Ui1, name: &str| {
        h.tree
            .iter()
            .any(|(n, r)| n == name && side.contains(r.center()))
    };
    let spaces: Vec<Space> = h
        .app
        .data
        .spaces
        .iter()
        .filter(|s| s.private_owner.is_none() && !s.archived && !s.hidden)
        .cloned()
        .collect();
    assert!(spaces.len() >= 2);
    for s in &spaces {
        assert!(in_side(&h, &s.name), "{} shows before searching", s.name);
    }
    h.click("Search your sidebar");
    h.type_text("zzz");
    h.settle();
    for s in &spaces {
        assert!(
            !in_side(&h, &s.name),
            "no match: space {} is hidden",
            s.name
        );
        for f in &s.folders {
            assert!(
                !in_side(&h, &f.name),
                "no match: folder {} is hidden",
                f.name
            );
        }
        for l in &s.lists {
            assert!(!in_side(&h, &l.name), "no match: list {} is hidden", l.name);
        }
    }
    // part of one list's name: that list, its folder and space stay; other rows go
    let (space, folder, list) = spaces
        .iter()
        .find_map(|s| {
            s.folders
                .iter()
                .find_map(|f| f.lists.first().map(|l| (s.clone(), f.clone(), l.clone())))
        })
        .expect("a list in a folder");
    let q: String = list.name.to_lowercase().chars().take(5).collect();
    h.app.side.search = Some(q.clone());
    h.settle();
    assert!(
        in_side(&h, &space.name) && in_side(&h, &folder.name) && in_side(&h, &list.name),
        "{:?}: the list, its folder and space show",
        q
    );
    for s in spaces.iter().filter(|s| s.id != space.id) {
        let inside = s.name.to_lowercase().contains(&q)
            || s.folders.iter().any(|f| {
                f.name.to_lowercase().contains(&q)
                    || f.lists.iter().any(|l| l.name.to_lowercase().contains(&q))
            })
            || s.lists.iter().any(|l| l.name.to_lowercase().contains(&q))
            || h.app
                .data
                .docs
                .iter()
                .any(|d| d.space == Some(s.id) && d.title.to_lowercase().contains(&q));
        assert_eq!(
            in_side(&h, &s.name),
            inside,
            "space {} shows only with a match",
            s.name
        );
    }
    for l in space
        .lists
        .iter()
        .chain(folder.lists.iter())
        .filter(|l| !l.name.to_lowercase().contains(&q))
    {
        assert!(
            !in_side(&h, &l.name) || h.app.data.favorites.contains(&Location::List(l.id)),
            "list {} without a match is hidden",
            l.name
        );
    }
    // an empty query shows everything again
    h.app.side.search = Some(String::new());
    h.settle();
    for s in &spaces {
        assert!(in_side(&h, &s.name));
    }
}

#[test]
fn sidebar_search_hides_the_favorites_heading_without_a_match() {
    let mut h = Ui1::new();
    let side = Rect::from_min_max(pos2(0.0, 0.0), pos2(320.0, 5000.0));
    let heading = |h: &Ui1| {
        h.painted_text("Favorites")
            .iter()
            .any(|(r, _)| side.contains(r.center()))
    };
    if h.app.data.favorites.is_empty() {
        h.app
            .data
            .favorites
            .push(Location::List(h.app.data.spaces[0].folders[0].lists[0].id));
        h.settle();
    }
    assert!(heading(&h), "Favorites shows before searching");
    let fav = h.app.data.location_name(h.app.data.favorites[0]);
    h.app.side.search = Some("zzz".into());
    h.settle();
    assert!(!heading(&h), "no favorite matches: no heading");
    h.app.side.search = Some(fav.to_lowercase());
    h.settle();
    assert!(heading(&h), "a matching favorite keeps the heading");
}

#[test]
fn favorite_a_task_and_open_it_from_the_sidebar() {
    let mut h = Ui1::new();
    h.click("Task options Write copy for landing page");
    h.click("Add to Favorites");
    let id = task_named(&h, "Write copy for landing page").id;
    assert!(h.app.data.extra.starred.contains(&format!("task:{}", id)));
    h.click_side("Write copy for landing page", false);
    assert_eq!(h.app.open_task, Some(id));
}

#[test]
fn sidebar_lists_reorder_by_drag() {
    let mut h = Ui1::sized(1440.0, 1200.0);
    let personal = h.app.data.spaces[0].lists[0].id;
    h.drag("Personal", "Marketing");
    let f = &h.app.data.spaces[0].folders[0];
    assert_eq!(
        f.lists.iter().map(|l| l.id).collect::<Vec<_>>()[1],
        personal,
        "Personal lands before Marketing"
    );
    assert!(h.app.data.spaces[0].lists.is_empty());
}

#[test]
fn home_my_work_row_ticks_done() {
    let mut h = Ui1::new();
    h.app.actions.push(Action::Go(Location::Home));
    h.settle();
    h.click("Complete Renew passport");
    let t = task_named(&h, "Renew passport");
    assert!(h.app.data.is_closed(t));
    // the Done section starts folded
    h.app.home_collapsed.remove("Done");
    h.settle();
    h.click("Reopen Renew passport");
    let t = task_named(&h, "Renew passport");
    assert!(!h.app.data.is_closed(t));
}

#[test]
fn reminders_snooze_edit_delete_and_show_done() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    h.app.data.extra.reminders.push(Reminder {
        id: 501,
        text: "Call the bank".into(),
        due_ts: now_ts() + 600,
        done: false,
        owner: me,
        ..Default::default()
    });
    h.app.data.extra.reminders.push(Reminder {
        id: 502,
        text: "Old one".into(),
        due_ts: now_ts() - 600,
        done: true,
        owner: me,
        ..Default::default()
    });
    h.click("Inbox");
    h.click("Snooze Call the bank");
    h.click("Tomorrow 9 AM");
    let r = h
        .app
        .data
        .extra
        .reminders
        .iter()
        .find(|r| r.id == 501)
        .unwrap();
    assert!(r.due_ts > now_ts() + 3600);
    h.click("Edit Call the bank");
    h.click("Reminder text");
    h.type_text(" back");
    h.click("Save");
    assert!(h
        .app
        .data
        .extra
        .reminders
        .iter()
        .any(|r| r.text == "Call the bank back"));
    assert!(!h.has("Old one"));
    h.click("Show done reminders");
    assert!(h.has("Old one"));
    h.click("Done Old one");
    assert!(
        !h.app
            .data
            .extra
            .reminders
            .iter()
            .find(|r| r.id == 502)
            .unwrap()
            .done,
        "a done reminder can be ticked back"
    );
    h.click("Delete Call the bank back");
    assert!(!h.app.data.extra.reminders.iter().any(|r| r.id == 501));
}

#[test]
fn inbox_later_item_goes_back_to_primary() {
    let mut h = Ui1::new();
    let tid = h.app.data.tasks[1].id;
    let ts = now_ts() - 30;
    h.app.data.tasks[1].activity.push(Activity {
        ts,
        author: 2,
        text: "Over to you".into(),
        comment: true,
        post: false,
        ..Default::default()
    });
    let key = format!("{}:{}:2", tid, ts);
    h.app.data.extra.inbox_later.push(key.clone());
    h.click("Inbox");
    h.click("Inbox Later");
    let p = h
        .find("Write copy for landing page")
        .expect("the item is in Later");
    h.hover(p);
    h.click("Primary");
    assert!(!h.app.data.extra.inbox_later.contains(&key));
}

#[test]
fn search_link_arrows_and_enter() {
    let mut h = Ui1::new();
    let tid = h.task_id("Newsletter draft");
    h.app.palette = Some(format!("workboard://task/{}", tid));
    h.settle();
    h.key(egui::Key::Enter);
    assert_eq!(h.app.open_task, Some(tid));
    h.app.open_task = None;
    // arrows move the highlight; Enter opens the highlighted result
    h.app.palette = Some("Projects".into());
    h.settle();
    h.key(egui::Key::ArrowDown);
    let rows = h.app.search_rows("Projects", 0);
    assert_eq!(h.app.sweep.search_sel, 1);
    h.key(egui::Key::Enter);
    assert!(h.app.palette.is_none());
    assert_eq!(
        h.app.sweep.search_recent.first().map(String::as_str),
        Some("Projects")
    );
    if let crate::app::more::Hit::Go(l) = rows[1].4 {
        assert_eq!(h.app.loc, l);
    }
}

#[test]
fn cmd_brackets_and_comma_shortcuts() {
    let mut h = Ui1::new();
    let start = h.app.loc;
    h.click("Inbox");
    cmd_key(&mut h, egui::Key::OpenBracket);
    assert_eq!(h.app.loc, start, "Cmd+[ goes back");
    cmd_key(&mut h, egui::Key::CloseBracket);
    assert_eq!(h.app.loc, Location::Inbox, "Cmd+] goes forward");
    cmd_key(&mut h, egui::Key::Comma);
    assert_eq!(h.app.section, Section::Settings);
}

#[test]
fn cmd_brackets_go_back_and_forward() {
    let mut h = Ui1::new();
    let start = h.app.loc;
    h.click("Inbox");
    let cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    let press = |h: &mut Ui1, key| {
        h.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: cmd,
        }]);
        h.settle();
    };
    press(&mut h, egui::Key::OpenBracket);
    assert_eq!(h.app.loc, start);
    press(&mut h, egui::Key::CloseBracket);
    assert_eq!(h.app.loc, Location::Inbox);
}

#[test]
fn cmd_arrows_go_back_and_forward_outside_text_fields() {
    let mut h = Ui1::new();
    let start = h.app.loc;
    h.click("Inbox");
    cmd_key(&mut h, egui::Key::ArrowLeft);
    assert_eq!(h.app.loc, start, "Cmd+Left goes back");
    cmd_key(&mut h, egui::Key::ArrowRight);
    assert_eq!(h.app.loc, Location::Inbox, "Cmd+Right goes forward");

    // in a text field they move the cursor and stay on the page
    h.ctx.set_os(egui::os::OperatingSystem::Mac);
    h.click("general");
    h.click("Message composer");
    let spot = h.app.nav_spot();
    h.type_text("hello");
    cmd_key(&mut h, egui::Key::ArrowLeft);
    assert_eq!(h.app.nav_spot(), spot, "Cmd+Left in a text field stays");
    h.type_text("X");
    assert_eq!(
        h.app.chat.channel.text, "Xhello",
        "and moves to the line start"
    );
}

#[test]
fn mouse_side_buttons_go_back_and_forward() {
    let mut h = Ui1::new();
    let start = h.app.loc;
    h.click("Inbox");
    h.click("My Tasks");
    let mid = pos2(700.0, 400.0);
    h.click_button(mid, egui::PointerButton::Extra1);
    assert_eq!(h.app.loc, Location::Inbox, "the back button goes back");
    h.click_button(mid, egui::PointerButton::Extra1);
    assert_eq!(h.app.loc, start, "and back again");
    h.click_button(mid, egui::PointerButton::Extra2);
    assert_eq!(
        h.app.loc,
        Location::Inbox,
        "the forward button goes forward"
    );
    // not while a dialog is open
    h.app.palette = Some(String::new());
    h.settle();
    h.click_button(mid, egui::PointerButton::Extra1);
    assert_eq!(h.app.loc, Location::Inbox);
}

#[test]
fn option_t_while_typing_and_enter_in_search() {
    let mut h = Ui1::new();
    h.app.palette = Some(String::new());
    h.settle();
    // Option+T types into the search box, it doesn't open Create Task
    let alt = egui::Modifiers {
        alt: true,
        ..Default::default()
    };
    h.frame(vec![egui::Event::Key {
        key: egui::Key::T,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: alt,
    }]);
    h.settle();
    assert!(h.app.quick_add.is_none());
    // a pasted task link opens the task on Enter
    let tid = h.app.data.tasks[0].id;
    h.app.palette = Some(format!("workboard://task/{}", tid));
    h.settle();
    h.key(egui::Key::Enter);
    h.settle();
    assert!(h.app.palette.is_none());
    assert_eq!(h.app.open_task, Some(tid));
    // Enter on a search that only finds a channel opens the channel
    h.app.open_task = None;
    let ch = h.channel_id("website-redesign");
    h.app.palette = Some("website-redesign".into());
    h.settle();
    h.key(egui::Key::Enter);
    h.settle();
    assert_eq!((h.app.section, h.app.chat.open), (Section::Chat, Some(ch)));
}

#[test]
fn search_leaves_out_other_peoples_personal_space_and_archived_lists() {
    let mut h = Ui1::new();
    let other = h.app.data.spaces.len() as u64 + 90_000;
    h.app.data.spaces.push(Space {
        id: other,
        name: "Someone Private".into(),
        private_owner: Some(2),
        lists: vec![ListItem {
            id: other + 1,
            name: "Their Secret List".into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    if let Some(l) = h.app.data.spaces[0].lists.first_mut() {
        l.name = "Old Archived List".into();
        l.archived = true;
    }
    h.app.palette = Some(String::new());
    h.settle();
    // the palette names lists by their path
    assert!(h.has("Team Space / Projects / Marketing"));
    assert!(
        !h.has("Someone Private")
            && !h.has("Someone Private / Their Secret List")
            && !h.has("Team Space / Old Archived List")
    );
}

#[test]
fn header_automate_opens_for_the_current_list_and_manage_people_opens_settings() {
    let mut h = Ui1::new();
    let lid = h.app.data.first_list().unwrap();
    h.app.loc = Location::List(lid);
    h.settle();
    h.click("Automate");
    assert!(
        matches!(h.app.modal, Some(Modal::Automations { scope: Some(Location::List(l)), .. }) if l == lid),
        "new rules run in this list"
    );
    h.app.modal = None;
    h.app.section = Section::Teams;
    h.app.hub.teams = TeamsPage::Analytics;
    h.settle();
    h.click("Manage people");
    assert_eq!(h.app.section, Section::Settings);
    assert_eq!(h.app.settings_tab.label(), "People");
}

#[test]
fn a_link_from_macos_opens_in_the_frame_it_arrives() {
    let mut h = Ui1::new();
    let id = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| h.app.task_visible(t))
        .map(|t| t.id)
        .unwrap();
    assert_eq!(h.app.open_task, None);
    // macOS hands the link over between frames (no click or key follows)
    crate::links::arrived(format!("workboard://task/{}", id));
    h.frame(vec![]);
    assert_eq!(
        h.app.open_task,
        Some(id),
        "the link opened without waiting for more input"
    );
    assert!(
        h.ctx.has_requested_repaint(),
        "the app draws again to show it"
    );
}

#[test]
fn click_notification_settings_turn_kinds_off_and_set_channels() {
    let mut h = Ui1::new();
    h.click("Inbox");
    h.click("Notification settings");
    assert!(matches!(h.app.modal, Some(Modal::Notifications)));
    assert!(h.app.data.extra.notify.direct);
    h.click("Direct messages");
    assert!(!h.app.data.extra.notify.direct);
    // rows past the dialog's scroll area are laid out but clipped: bring
    // this one up before clicking it
    let over = h.find("Direct messages").unwrap();
    h.frame(vec![egui::Event::PointerMoved(over)]);
    h.frame(vec![egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -120.0),
        modifiers: Default::default(),
    }]);
    h.settle();
    h.click("Reactions to my messages");
    assert!(h.app.data.extra.notify.reactions);
    // the dialog scrolls: wheel down over it to reach the channel choices
    let over = h.find("Direct messages").unwrap();
    for _ in 0..6 {
        h.frame(vec![egui::Event::PointerMoved(over)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -120.0),
            modifiers: Default::default(),
        }]);
    }
    h.settle();
    h.click("Every new message");
    assert_eq!(h.app.data.extra.notify.channel_default, "all");
    // the count and the help name this system's places
    let (badge, settings) = if cfg!(windows) {
        (
            "Show the unread count on the taskbar icon",
            "Windows Settings > System > Notifications",
        )
    } else {
        (
            "Show the unread count on the Dock icon",
            "System Settings > Notifications",
        )
    };
    assert!(h.has(badge));
    h.click("Send a test notification");
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|t| t.0.starts_with("Sent a test notification") && t.0.contains(settings)));
}

#[test]
fn home_lists_card_says_when_there_are_no_lists_and_offers_one() {
    let mut h = Ui1::new();
    // a workspace without Spaces or Lists, as a first start has
    h.app.data.spaces.retain(|s| s.private_owner.is_some());
    h.app.section = Section::Home;
    h.app.loc = Location::Home;
    h.settle();
    assert!(h.has("No Lists yet. Lists keep a Space's tasks together."));
    h.click("Home new list");
    assert!(
        matches!(h.app.modal, Some(Modal::NewSpace { .. })),
        "no Space yet: make one first"
    );
    h.key(egui::Key::Escape);
    // with a Space, the button makes a List in it
    let mut h = Ui1::new();
    for s in h.app.data.spaces.iter_mut() {
        s.lists.clear();
        s.folders.clear();
    }
    h.app.section = Section::Home;
    h.app.loc = Location::Home;
    h.settle();
    h.click("Home new list");
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })));
}

#[test]
fn inbox_with_reminders_waiting_is_not_all_caught_up() {
    let mut h = Ui1::new();
    h.click("Inbox");
    h.app.data.extra.inbox_cleared = h.app.inbox_items().iter().cloned().map(|i| i.key).collect();
    h.settle();
    assert!(h.has("You're all caught up") || h.has("Looking to collaborate?"));
    let id = h.app.data.new_id();
    h.app.data.extra.reminders.push(Reminder {
        id,
        text: "Call the bank".into(),
        due_ts: now_ts() - 60,
        owner: h.app.data.me,
        ..Default::default()
    });
    h.settle();
    assert!(h.has("Call the bank"));
    assert!(!h.has("You're all caught up"), "a reminder is waiting");
    assert!(h.has("No new comments, mentions or assignments."));
}

#[test]
fn hand_made_dialogs_close_like_the_others() {
    let mut h = Ui1::new();
    let list = task_named(&h, "Write copy for landing page").list_id;
    h.app.apply(Action::Go(Location::List(list)));
    h.settle();
    // Rename: the header's close button, and Cancel
    h.click("List menu");
    h.click("Rename list");
    assert!(h.app.rename.is_some());
    h.click("Close");
    assert!(h.app.rename.is_none(), "the header's close button");
    h.click("List menu");
    h.click("Rename list");
    h.click("Cancel");
    assert!(h.app.rename.is_none(), "Cancel");
    // Create Task: a click on the dimmed page behind it
    h.click("Create");
    h.click("Task");
    assert!(h.app.quick_add.is_some());
    h.click_at(pos2(20.0, 880.0));
    assert!(h.app.quick_add.is_none(), "a click outside the dialog");
    // New message: the header's close button
    h.app.new_message_open = true;
    h.settle();
    h.click("Close");
    assert!(!h.app.new_message_open);
}

#[test]
fn list_crumb_opens_the_list_menu() {
    let mut h = Ui1::new();
    let list = task_named(&h, "Write copy for landing page").list_id;
    h.app.apply(Action::Go(Location::List(list)));
    h.settle();
    // the arrow (and the icon and name beside it) open the Manage menu
    h.click("List menu");
    assert!(h.has("Rename list"));
    assert!(h.has("Delete list"));
    h.click("Rename list");
    assert!(matches!(h.app.rename, Some((Target::List(id), _)) if id == list));
    // the Rename dialog keeps the page behind it until it is closed
    h.key(egui::Key::Escape);
    assert!(h.app.rename.is_none());
    // a second click on the crumb closes the menu again
    h.click("List menu");
    assert!(h.has("Export to CSV"));
    h.click("List menu");
    assert!(!h.has("Export to CSV"));
    // not on a Space: there is no List to manage, the Space's own menu instead
    let space = h.app.data.space_of_list(list).unwrap().id;
    h.app.apply(Action::Go(Location::Space(space)));
    h.settle();
    assert!(!h.has("List menu"));
    h.click("Space menu");
    assert!(h.has("Share & permissions") || h.has("Add to Favorites"));
    h.click("Space menu");
}

#[test]
fn folder_crumb_opens_the_folder_menu() {
    let mut h = Ui1::new();
    let fid = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| s.folders.iter())
        .map(|f| f.id)
        .next()
        .expect("a folder in the sample workspace");
    h.app.apply(Action::Go(Location::Folder(fid)));
    h.settle();
    h.click("Folder menu");
    h.click("Rename");
    assert!(matches!(h.app.rename, Some((Target::Folder(id), _)) if id == fid));
}

#[test]
fn gathered_views_have_no_crumb_arrow() {
    let mut h = Ui1::new();
    // Everything, My Tasks and Today & Overdue have nothing to manage: their
    // crumb has no arrow that would look like a menu and do nothing
    for loc in [
        Location::Everything,
        Location::MyTasks,
        Location::TodayOverdue,
    ] {
        h.app.apply(Action::Go(loc));
        h.settle();
        let name = h.app.data.location_name(loc);
        let crumb = h
            .painted_text(tr(&name))
            .iter()
            .map(|(r, _)| *r)
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .unwrap_or_else(|| panic!("the {} crumb", name));
        let after = Rect::from_min_max(
            pos2(crumb.right(), crumb.top()),
            pos2(crumb.right() + 18.0, crumb.bottom()),
        );
        let arrow = h
            .shapes
            .iter()
            .filter(|s| {
                matches!(&s.shape, egui::Shape::LineSegment { points: [a, b], .. }
                    if after.contains(*a) && after.contains(*b))
            })
            .count();
        assert_eq!(arrow, 0, "an arrow after the {} crumb", name);
        for m in ["List menu", "Folder menu", "Space menu"] {
            assert!(!h.has(m), "{} on {}", m, name);
        }
    }
}

// the small number drawn at the corner of a rail icon, if any
fn rail_hint(h: &Ui1, name: &str) -> Option<String> {
    let (_, area) = h.tree.iter().rev().find(|(n, _)| n == name)?;
    let mut found = None;
    each_text(&h.shapes, |ts, _| {
        let c = ts
            .galley
            .job
            .sections
            .first()
            .map_or(ts.fallback_color, |s| s.format.color);
        if c == Color32::from_gray(20) && area.contains(ts.visual_bounding_rect().center()) {
            found = Some(ts.galley.text().to_string());
        }
    });
    found
}

#[test]
fn holding_ctrl_numbers_the_rail_and_ctrl_number_opens_that_item() {
    // no other workspaces: Cmd+2 would open the second one
    let _home = own_home("rail-keys");
    let mut h = Ui1::new();
    // the bar by default: Home, Planner, AI, Teams
    assert_eq!(h.app.data.extra.rail_items, ["Planner", "AI", "Teams"]);
    assert_eq!(
        rail_hint(&h, "Rail Planner"),
        None,
        "no numbers without Ctrl"
    );

    h.frame_held(vec![], egui::Modifiers::CTRL);
    assert_eq!(rail_hint(&h, "Rail Home"), Some("1".into()));
    assert_eq!(rail_hint(&h, "Rail Planner"), Some("2".into()));
    assert_eq!(rail_hint(&h, "Rail Teams"), Some("4".into()));
    assert_eq!(rail_hint(&h, "Rail More"), None, "More opens a menu");
    h.settle();
    assert_eq!(
        rail_hint(&h, "Rail Planner"),
        None,
        "gone when Ctrl is let go"
    );

    h.key_with(egui::Key::Num2, egui::Modifiers::CTRL);
    assert_eq!(h.app.section, Section::Planner);
    h.key_with(egui::Key::Num4, egui::Modifiers::CTRL);
    assert_eq!(h.app.section, Section::Teams);
    h.key_with(egui::Key::Num1, egui::Modifiers::CTRL);
    assert_eq!(h.app.section, Section::Home);
    // a number past the last item does nothing
    h.key_with(egui::Key::Num9, egui::Modifiers::CTRL);
    assert_eq!(h.app.section, Section::Home);
    // the Mac's Cmd key neither numbers the bar nor moves along it (with
    // a number it opens a workspace, and this is the only one)
    let cmd = egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND;
    h.frame_held(vec![], cmd);
    assert_eq!(rail_hint(&h, "Rail Planner"), None, "no numbers with Cmd");
    h.settle();
    h.key_with(egui::Key::Num2, cmd);
    assert_eq!(h.app.section, Section::Home);

    // an app opened from More sits in the bar and takes the next number
    h.app.open_app("Docs");
    h.key_with(egui::Key::Num1, egui::Modifiers::CTRL);
    h.frame_held(vec![], egui::Modifiers::CTRL);
    assert_eq!(rail_hint(&h, "Rail Docs"), Some("5".into()));
    h.settle();
    h.key_with(egui::Key::Num5, egui::Modifiers::CTRL);
    assert_eq!(h.app.section, Section::Docs);

    // not while a task is open: off the Mac Ctrl+2 is its priority there
    h.key_with(egui::Key::Num1, egui::Modifiers::CTRL);
    let id = h.app.data.tasks[0].id;
    h.app.open_task = Some(id);
    h.settle();
    h.key_with(egui::Key::Num2, egui::Modifiers::CTRL);
    assert_eq!(h.app.section, Section::Home);
}

// the open slot the rail shows while one of its items is dragged
fn rail_drop_slot(h: &Ui1) -> Option<Rect> {
    h.shapes.iter().find_map(|c| match &c.shape {
        egui::Shape::Rect(r) if r.fill == Color32::from_white_alpha(18) => Some(r.rect),
        _ => None,
    })
}

#[test]
fn dragging_a_rail_item_shows_where_it_lands_and_moves_it_there() {
    let mut h = Ui1::new();
    assert_eq!(h.app.rail_order(), ["Home", "Planner", "AI", "Teams"]);
    let planner = h.rect("Rail Planner");
    let ai = h.rect("Rail AI");
    let teams = h.rect("Rail Teams");
    assert_eq!(
        rail_drop_slot(&h),
        None,
        "no slot until something is dragged"
    );

    // pick Teams up and carry it over Planner without letting go
    let (from, to) = (teams.center(), planner.center());
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    for i in 1..=12 {
        h.frame(vec![egui::Event::PointerMoved(
            from + (to - from) * (i as f32 / 12.0),
        )]);
    }
    h.settle();
    let slot = rail_drop_slot(&h).expect("the slot it would land in is shown");
    assert!(
        (slot.center().y - planner.center().y).abs() < 2.0,
        "the slot is where Planner was: {slot:?} vs {planner:?}"
    );
    // Planner and AI moved down a place to leave the slot open
    assert!((h.rect("Rail Planner").center().y - ai.center().y).abs() < 1.0);
    assert!((h.rect("Rail AI").center().y - teams.center().y).abs() < 1.0);
    // Teams follows the mouse
    assert!((h.rect("Rail Teams").center().y - to.y).abs() < 1.0);
    // nothing is moved until it is let go
    assert_eq!(h.app.data.extra.rail_items, ["Planner", "AI", "Teams"]);
    assert_eq!(h.app.section, Section::Home, "a drag doesn't open the item");

    h.frame(vec![egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    h.settle();
    assert_eq!(h.app.data.extra.rail_items, ["Teams", "Planner", "AI"]);
    assert_eq!(rail_drop_slot(&h), None, "the slot goes away when let go");
    assert!((h.rect("Rail Teams").center().y - planner.center().y).abs() < 1.0);
    assert!((h.rect("Rail AI").center().y - teams.center().y).abs() < 1.0);
    assert_eq!(h.app.section, Section::Home, "a drop doesn't open the item");

    // Home moves too; it is only written down while it isn't on top
    h.drag("Rail Home", "Rail AI");
    assert_eq!(h.app.rail_order(), ["Teams", "Planner", "AI", "Home"]);
    assert_eq!(
        h.app.data.extra.rail_items,
        ["Teams", "Planner", "AI", "Home"]
    );
    h.drag("Rail Home", "Rail Teams");
    assert_eq!(h.app.data.extra.rail_items, ["Teams", "Planner", "AI"]);

    // let go where it was picked up: nothing changes
    h.drag("Rail Planner", "Rail Planner");
    assert_eq!(h.app.data.extra.rail_items, ["Teams", "Planner", "AI"]);
    // and a plain click still opens the item
    h.click("Rail AI");
    assert_eq!(h.app.section, Section::Ai);
}

#[test]
fn on_a_full_bar_a_dragged_item_stays_above_more() {
    // the smallest window: some pinned apps are scrolled out of view
    let mut h = Ui1::sized(960.0, 560.0);
    let pinned = [
        "Docs",
        "Dashboards",
        "Forms",
        "Goals",
        "AI",
        "Teams",
        "Chat",
    ];
    h.app.data.extra.rail_items = pinned.map(String::from).to_vec();
    h.settle();
    let shown: Vec<&str> = pinned.into_iter().filter(|l| in_bar(&h, l)).collect();
    assert!(shown.len() >= 2 && shown.len() < pinned.len(), "{shown:?}");
    let last = *shown.last().unwrap();

    // carried far below the bar, the first app lands in the last place shown,
    // not among the hidden ones
    let from = h.rect("Rail Docs").center();
    h.drag_by(from, pos2(from.x, 555.0));
    let mut want: Vec<&str> = shown[1..].to_vec();
    want.push("Docs");
    want.extend(&pinned[shown.len()..]);
    assert_eq!(h.app.data.extra.rail_items, want);
    assert!(in_bar(&h, "Docs"), "still in view, after {last}");
    assert!(h.rect("Rail Docs").bottom() <= h.rect("Rail More").top());
}

/// The shortcuts list names Cmd+Left / Cmd+Right for back and forward, which
/// work as Cmd+[ / Cmd+] do.
#[test]
fn shortcuts_list_names_cmd_arrows_for_back_and_forward() {
    let mut h = Ui1::new();
    h.app.open_settings("Keyboard shortcuts");
    h.settle();
    let keys = crate::os::keys("\u{2318}\u{2190}");
    assert!(
        h.texts().iter().any(|s| s.contains(&keys)),
        "{:?} is listed",
        keys
    );
}
