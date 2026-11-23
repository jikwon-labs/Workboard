// The window's width bands (shell/narrow.rs): a medium window lays the
// sidebar over the page as a drawer, a compact one is a single column.

use super::*;
use crate::app::shell::Tier;
use crate::i18n::{set_lang, Lang};

fn cmd() -> egui::Modifiers {
    egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    }
}

/// The sidebar's "+ Create" is on screen.
fn sidebar_shown(h: &Ui1) -> bool {
    h.has("Create")
}

fn resize(h: &mut Ui1, w: f32, hh: f32) {
    h.size = vec2(w, hh);
    h.settle();
}

#[test]
fn width_picks_the_band() {
    assert_eq!(Tier::of(1440.0), Tier::Wide);
    assert_eq!(Tier::of(960.0), Tier::Wide);
    assert_eq!(Tier::of(959.0), Tier::Medium);
    assert_eq!(Tier::of(600.0), Tier::Medium);
    assert_eq!(Tier::of(599.0), Tier::Compact);
    assert_eq!(Tier::of(360.0), Tier::Compact);
}

#[test]
fn wide_window_has_no_drawer_button() {
    let h = Ui1::sized(960.0, 640.0);
    assert_eq!(h.app.tier, Tier::Wide);
    assert!(sidebar_shown(&h));
    assert!(!h.has("Toggle sidebar"));
}

#[test]
fn medium_window_opens_the_sidebar_as_a_drawer_and_closes_it_on_a_pick() {
    let mut h = Ui1::sized(700.0, 640.0);
    assert_eq!(h.app.tier, Tier::Medium);
    // folded away, the page has the width; the button sits by the window buttons
    assert!(!sidebar_shown(&h));
    assert!(!h.has("Open sidebar"), "no strip left behind");
    let b = h.rect("Toggle sidebar");
    assert!(b.left() < 120.0 && b.top() < 40.0, "{:?}", b);
    h.click("Toggle sidebar");
    assert!(h.app.sheet.open);
    assert!(sidebar_shown(&h));
    // over the page, from the rail's edge, inside the window
    let create = h.rect("Create");
    assert!(
        create.left() > 58.0 && create.right() < 700.0,
        "{:?}",
        create
    );
    assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
    // picking a page goes there and folds the drawer away
    h.click("My Tasks");
    assert!(!h.app.sheet.open);
    assert!(!sidebar_shown(&h));
    assert_eq!(h.app.loc, Location::MyTasks);
}

#[test]
fn medium_window_drawer_closes_on_the_shade_escape_and_its_close_button() {
    let mut h = Ui1::sized(700.0, 640.0);
    h.click("Toggle sidebar");
    assert!(h.app.sheet.open);
    // a click on the shaded page beside it
    h.click_at(pos2(650.0, 400.0));
    assert!(!h.app.sheet.open, "the shade closes it");
    h.click("Toggle sidebar");
    h.key(egui::Key::Escape);
    assert!(!h.app.sheet.open, "Escape closes it");
    h.click("Toggle sidebar");
    h.click("Toggle sidebar");
    assert!(!h.app.sheet.open, "the button closes it again");
    // the sidebar's own « folds the drawer, not the sidebar for good
    h.click("Toggle sidebar");
    h.hover(h.rect("Create").center());
    h.click("Close sidebar");
    assert!(!h.app.sheet.open);
    assert!(!h.app.sidebar_collapsed);
}

#[test]
fn medium_window_shortcuts_open_and_close_the_drawer() {
    let mut h = Ui1::sized(700.0, 640.0);
    h.key_with(egui::Key::Backslash, cmd());
    assert!(h.app.sheet.open);
    assert!(sidebar_shown(&h));
    h.key_with(egui::Key::Backslash, cmd());
    assert!(!h.app.sheet.open);
    assert!(!h.app.sidebar_collapsed, "the choice beside the page stays");
    if cfg!(target_os = "macos") {
        let ctrl_cmd = egui::Modifiers {
            ctrl: true,
            ..cmd()
        };
        h.key_with(egui::Key::S, ctrl_cmd);
        assert!(h.app.sheet.open, "Ctrl+Cmd+S opens it");
        h.key_with(egui::Key::S, ctrl_cmd);
        assert!(!h.app.sheet.open, "and closes it");
    }
}

#[test]
fn hub_sidebars_are_drawers_in_a_medium_window() {
    let mut h = Ui1::sized(700.0, 640.0);
    h.app.section = Section::Docs;
    h.settle();
    assert!(!h.has("Close sidebar"));
    h.click("Toggle sidebar");
    assert!(h.app.sheet.open);
    // the Docs hub's sidebar, with its title row's «
    assert!(h.has("Close sidebar"));
    h.click("Close sidebar");
    assert!(!h.app.sheet.open);
    assert!(!h.app.sidebar_collapsed);
}

#[test]
fn widening_puts_the_sidebar_back_as_the_person_left_it() {
    // open beside the page: a drawer while narrow, beside it again when wide
    let mut h = Ui1::sized(1440.0, 900.0);
    assert!(sidebar_shown(&h));
    resize(&mut h, 700.0, 640.0);
    assert!(!sidebar_shown(&h));
    h.click("Toggle sidebar");
    assert!(sidebar_shown(&h));
    resize(&mut h, 1440.0, 900.0);
    assert!(!h.app.sheet.open);
    assert!(sidebar_shown(&h));
    assert!(!h.has("Toggle sidebar"));
    // closed for good beside the page: the drawer still opens while narrow,
    // and widening brings back the closed strip, not the sidebar
    h.key_with(egui::Key::Backslash, cmd());
    assert!(h.app.sidebar_collapsed);
    assert!(h.has("Open sidebar"));
    resize(&mut h, 700.0, 640.0);
    assert!(!h.has("Open sidebar"));
    h.click("Toggle sidebar");
    assert!(sidebar_shown(&h));
    assert!(
        h.app.sidebar_collapsed,
        "the drawer doesn't undo the choice"
    );
    resize(&mut h, 1440.0, 900.0);
    assert!(!sidebar_shown(&h));
    assert!(h.has("Open sidebar"));
}

/// The top bar's controls side by side: none covers another, all inside.
fn top_bar_fits(h: &Ui1) {
    let names = [
        "Toggle sidebar",
        "Workspace menu",
        "Planner shortcut",
        "Go back",
        "Go forward",
        "Search",
        "AI Chats",
        "Profile menu",
    ];
    let rects: Vec<(&str, Rect)> = names
        .iter()
        .filter_map(|n| {
            h.tree
                .iter()
                .find(|(m, r)| m == n && r.top() < 40.0)
                .map(|(_, r)| (*n, *r))
        })
        .collect();
    for (i, (a, ra)) in rects.iter().enumerate() {
        assert!(
            ra.right() <= h.size.x,
            "{} past the edge at {}",
            a,
            h.size.x
        );
        for (b, rb) in &rects[i + 1..] {
            let o = ra.intersect(*rb);
            assert!(
                !(o.width() > 0.5 && o.height() > 0.5),
                "{} covers {} at {}: {:?} {:?}",
                a,
                b,
                h.size.x,
                ra,
                rb
            );
        }
    }
    let bar = Rect::from_min_max(pos2(0.0, 0.0), pos2(h.size.x, 40.0));
    let over = super::layout_tests::overlapping(&h.texts_in(bar, Seen::Visible));
    assert!(over.is_empty(), "at {}: {:?}", h.size.x, over);
    assert!(h.cut_texts(bar).is_empty(), "{:?}", h.cut_texts(bar));
}

#[test]
fn medium_top_bar_keeps_its_controls_apart() {
    for w in [600.0, 700.0, 959.0] {
        let mut h = Ui1::sized(w, 640.0);
        h.app.data.workspace = "A workspace with a rather long name".into();
        h.settle();
        assert!(h.has("Toggle sidebar"));
        assert!(h.has("Search"));
        top_bar_fits(&h);
    }
}

#[test]
fn wide_top_bar_keeps_its_controls_apart() {
    // a long workspace name pushed the arrows over the planner button: the
    // search box was placed from the window's middle alone
    for name in [
        "Seoul head office design and development workspace",
        "Seoul head office design, development, marketing, sales and support all in one workspace",
    ] {
        for w in [960.0, 1100.0, 1440.0] {
            let mut h = Ui1::sized(w, 560.0);
            h.app.data.workspace = name.into();
            h.settle();
            assert!(h.has("Search"));
            top_bar_fits(&h);
            let arrow = h.rect("Go back");
            let planner = h.rect("Planner shortcut");
            assert!(
                arrow.left() >= planner.right(),
                "at {}: the arrow {:?} starts left of the planner {:?}",
                w,
                arrow,
                planner
            );
        }
    }
}

/// Nothing past the right edge, no text cut mid-glyph, on the whole window.
fn fits(h: &Ui1, what: &str) {
    let past = h.past_right_edge();
    assert!(past.is_empty(), "{}: past the right edge: {:?}", what, past);
    let all = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
    let cut = h.cut_texts(all);
    assert!(cut.is_empty(), "{}: cut texts: {:?}", what, cut);
}

/// The control named `name` is inside the window.
fn inside(h: &Ui1, name: &str) {
    let r = h.rect(name);
    let w = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
    assert!(w.contains_rect(r), "{} {:?} outside {:?}", name, r, w);
}

fn go_home(h: &mut Ui1) {
    while h.has("Back") {
        h.click("Back");
    }
    assert!(h.app.compact_at_home(), "on the home screen");
}

#[test]
fn compact_window_is_one_column_entered_from_the_home_screen() {
    let mut h = Ui1::sized(360.0, 640.0);
    assert_eq!(h.app.tier, Tier::Compact);
    // no rail, no sidebar beside the page: the page has the window
    assert!(!h.has("Rail Home"));
    assert!(!sidebar_shown(&h));
    assert!(h.has("Back"));
    inside(&h, "Back");
    top_bar_fits(&h);
    // back from the first page: the home screen, the whole width
    go_home(&mut h);
    assert!(sidebar_shown(&h));
    assert!(h.has("App Home"), "the rail's apps lead the list");
    assert!(h.has("Inbox"));
    assert!(!h.has("Back"), "nowhere further back");
    assert!(h.has("Workspace menu"));
    let create = h.rect("Create");
    assert!(create.right() > 300.0, "the sidebar fills the window");
    fits(&h, "home screen");
    top_bar_fits(&h);
    // Escape doesn't leave the home screen for the page under it
    h.key(egui::Key::Escape);
    assert!(h.app.compact_at_home());
    // into the Inbox and back out
    h.click("Inbox");
    assert_eq!(h.app.loc, Location::Inbox);
    assert!(!h.app.sheet.open);
    assert!(!sidebar_shown(&h));
    inside(&h, "Back");
    h.click("Back");
    assert!(h.app.compact_at_home());
    // an app from the rail's rows
    h.click("App Planner");
    assert_eq!(h.app.section, Section::Planner);
    assert!(!h.app.sheet.open);
    h.click("Back");
    assert!(h.app.compact_at_home());
    assert_eq!(h.app.section, Section::Home);
}

#[test]
fn compact_back_climbs_from_a_list_to_its_folder_space_and_home() {
    let mut h = Ui1::sized(400.0, 700.0);
    let web = h.app.data.spaces[0].folders[0].lists[0].id;
    let folder = h.app.data.spaces[0].folders[0].id;
    let space = h.app.data.spaces[0].id;
    go_home(&mut h);
    // the favourite on the home screen
    h.click("Website Redesign");
    assert_eq!(h.app.loc, Location::List(web));
    assert!(!h.app.sheet.open);
    fits(&h, "list");
    assert!(h
        .painted_text("Projects")
        .iter()
        .any(|(r, _)| r.top() < 40.0));
    h.click("Back");
    assert_eq!(h.app.loc, Location::Folder(folder));
    // Cmd+[ takes the same step
    h.key_with(egui::Key::OpenBracket, cmd());
    assert_eq!(h.app.loc, Location::Space(space));
    h.click("Back");
    assert!(h.app.compact_at_home());
    h.key_with(egui::Key::OpenBracket, cmd());
    assert!(h.app.compact_at_home(), "Cmd+[ on the home screen stays");
}

#[test]
fn compact_hub_goes_back_through_its_own_sidebar() {
    let mut h = Ui1::sized(360.0, 640.0);
    let doc = h.app.data.docs[0].id;
    h.open_docs_list();
    h.app.doc_open = Some(doc);
    h.settle();
    // a Doc: back to the Docs list, then the Docs sidebar, then home
    h.click("Back");
    assert_eq!(h.app.doc_open, None);
    assert_eq!(h.app.section, Section::Docs);
    h.click("Back");
    assert!(h.app.sheet.open);
    assert!(h.has("Close sidebar"), "the Docs sidebar, the whole width");
    // picking a filter there shows its docs
    h.click(&tr("My Docs").to_string());
    assert!(!h.app.sheet.open);
    h.click("Back");
    h.click("Back");
    assert!(h.app.compact_at_home());
}

#[test]
fn compact_settings_sections_are_a_list_of_their_own() {
    let mut h = Ui1::sized(360.0, 640.0);
    h.app.open_settings("My settings");
    h.settle();
    fits(&h, "My settings");
    h.click("Back");
    assert!(h.app.sheet.open);
    h.click("Settings Themes");
    assert!(!h.app.sheet.open);
    assert_eq!(h.app.settings_tab.label(), "Themes");
}

#[test]
fn compact_task_window_shows_details_or_comments() {
    let mut h = Ui1::sized(360.0, 640.0);
    let id = h.task_id("Design new homepage layout");
    h.app.open_task = Some(id);
    h.settle();
    inside(&h, "Close task");
    inside(&h, "Task tab Details");
    inside(&h, "Task tab Comments");
    assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
    assert!(!h.has("Comment box"));
    h.click("Task tab Comments");
    assert!(h.has("Comment box"));
    inside(&h, "Comment box");
    assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
    h.click("Task tab Details");
    h.click("Close task");
    assert_eq!(h.app.open_task, None);
}

#[test]
fn compact_thread_is_laid_over_the_conversation() {
    let mut h = Ui1::sized(360.0, 640.0);
    go_home(&mut h);
    h.click("general");
    assert_eq!(h.app.section, Section::Chat);
    inside(&h, "Back");
    h.send_message("A question for the thread");
    let msg = *h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .messages
        .last()
        .map(|m| &m.id)
        .unwrap();
    h.app.chat.thread_open = Some(msg);
    h.settle();
    let r = h.ctx.memory(|m| m.area_rect(egui::Id::new("thread_over")));
    let r = r.expect("the thread over the conversation");
    assert!(r.width() > 300.0, "{:?}", r);
    assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
}

/// The message box's tools are squares as big as the emoji button. At the
/// smallest window nine of them and Send with its arrow are too wide for
/// the row, so B, I, S and </> are left out (their shortcuts still work); a
/// little wider, they are back and still clear of Send.
#[test]
fn compact_message_box_tools_fit_beside_send() {
    for (w, formats) in [(360.0, false), (440.0, true)] {
        let mut h = Ui1::sized(w, 640.0);
        go_home(&mut h);
        h.click("general");
        h.click("Message composer");
        h.type_text("Hello");
        assert_eq!(h.has("Bold"), formats, "{w}");
        assert_eq!(h.has("Code"), formats, "{w}");
        let task = h.rect("Create task");
        let send = h.rect("Send");
        let arrow = h.rect("Schedule message");
        assert!(task.right() + 4.0 <= send.left(), "{w}: {task:?} {send:?}");
        assert!(send.right() <= arrow.left(), "{w}: {send:?} {arrow:?}");
        for name in [
            "Add files",
            "Take a photo",
            "Mention someone",
            "Emoji",
            "Create task",
        ] {
            let r = h.rect(name);
            assert_eq!(r.width(), r.height(), "{w}: {name} {r:?}");
        }
        assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
        // Cmd+B still makes the word bold
        if !formats {
            h.key_with(egui::Key::A, cmd());
            h.key_with(egui::Key::B, cmd());
            assert_eq!(h.app.chat.channel.text, "**Hello**", "{w}");
        }
    }
}

#[test]
fn going_wide_from_the_home_screen_brings_back_rail_and_sidebar() {
    let mut h = Ui1::sized(360.0, 640.0);
    go_home(&mut h);
    resize(&mut h, 1440.0, 900.0);
    assert_eq!(h.app.tier, Tier::Wide);
    assert!(!h.app.sheet.open);
    assert!(h.has("Rail Home"));
    assert!(sidebar_shown(&h));
    assert!(!h.has("Back"));
    // and narrow again: the page, with its way back
    resize(&mut h, 380.0, 640.0);
    assert!(!sidebar_shown(&h));
    assert!(h.has("Back"));
}

#[test]
fn each_band_keeps_its_top_bar_inside() {
    for (w, back) in [(960.0, false), (700.0, false), (400.0, true), (360.0, true)] {
        let mut h = Ui1::sized(w, 600.0);
        if w < 960.0 {
            // (a long name runs into the arrows of a wide window, as it did)
            h.app.data.workspace = "A workspace with a rather long name".into();
        }
        h.settle();
        assert_eq!(h.has("Back"), back, "at {}", w);
        top_bar_fits(&h);
        if back {
            go_home(&mut h);
            top_bar_fits(&h);
        }
    }
}

/// Every screen the Korean check opens, in a compact and a medium window:
/// no control past the right edge, no text cut mid-glyph, no dialog wider
/// than the window. The views that scroll sideways on purpose (board,
/// table, Gantt, timeline, Planner, the forms table, the timesheet grid)
/// and the task window over the page are left out.
#[test]
fn narrow_windows_fit_every_screen() {
    let sideways = [
        "list board",
        "list table",
        "list gantt",
        "list timeline",
        "section Planner",
        "forms table",
        "section Timesheets",
        "open task",
        "filter groups and saved filters",
        "bulk delete",
        "notepad",
    ];
    let names: Vec<String> = {
        let h = Ui1::new();
        super::i18n_tests::screens(&h)
            .into_iter()
            .map(|(n, _)| n)
            .collect()
    };
    let mut bad = vec![];
    let sizes: Vec<(f32, f32)> = match std::env::var("WB_NARROW_SIZES") {
        Ok(v) => v
            .split(',')
            .filter_map(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .collect(),
        Err(_) => vec![(360.0, 640.0), (700.0, 640.0)],
    };
    // the seed with long names, and a new workspace's empty pages (their
    // "nothing here yet" cards and a channel's first-message intro)
    for fresh in [false, true] {
        for lang in [Lang::En, Lang::Ko] {
            for &(w, hh) in &sizes {
                // a new workspace's few pages: the compact window is enough
                if fresh && w > 400.0 {
                    continue;
                }
                for name in &names {
                    if sideways.contains(&name.as_str()) {
                        continue;
                    }
                    set_lang(lang);
                    let mut h = if fresh {
                        let mut d = Data::fresh();
                        d.extra.welcome = false;
                        Ui1::with_data(d)
                    } else {
                        let mut h = Ui1::new();
                        super::layout_tests::heavy(&mut h);
                        h
                    };
                    h.settle();
                    h.size = vec2(w, hh);
                    // (a new workspace has fewer screens: no task, doc or folder)
                    let Some(go) = super::i18n_tests::screens(&h)
                        .into_iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, go)| go)
                    else {
                        set_lang(Lang::En);
                        continue;
                    };
                    h.app.modal = None;
                    h.app.login = None;
                    go(&mut h.app);
                    h.settle();
                    let page = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
                    if let Some(m) = h.try_modal_rect() {
                        if m.right() > w + 1.0 || m.left() < -1.0 {
                            bad.push(format!(
                                "{}{:?} {} [{}] dialog {:?}",
                                if fresh { "new " } else { "" },
                                lang,
                                w,
                                name,
                                m
                            ));
                        }
                    }
                    for c in h.cut_texts(page) {
                        bad.push(format!(
                            "{}{:?} {} [{}] cut {:?}",
                            if fresh { "new " } else { "" },
                            lang,
                            w,
                            name,
                            c
                        ));
                    }
                    for p in h.past_right_edge() {
                        bad.push(format!(
                            "{}{:?} {} [{}] past right {:?}",
                            if fresh { "new " } else { "" },
                            lang,
                            w,
                            name,
                            p
                        ));
                    }
                    set_lang(Lang::En);
                }
            }
        }
    }
    bad.sort();
    bad.dedup();
    assert!(bad.is_empty(), "{:#?}", bad);
}

#[test]
fn compact_space_overview_stacks_its_cards() {
    for lang in [Lang::En, Lang::Ko] {
        let mut h = Ui1::sized(420.0, 800.0);
        set_lang(lang);
        let space = h.app.data.spaces[0].id;
        h.app.actions.push(Action::Go(Location::Space(space)));
        h.settle();
        h.app.view = ViewKind::Overview;
        h.settle();
        let recent = h.painted_text(tr("Recent"));
        let docs = h.painted_text(tr("Docs"));
        set_lang(Lang::En);
        assert!(!recent.is_empty() && !docs.is_empty(), "{:?}", lang);
        // one under another, not three in a row
        assert!(
            docs[0].0.top() > recent[0].0.bottom(),
            "{:?}: {:?} {:?}",
            lang,
            recent[0].0,
            docs[0].0
        );
        assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
    }
}

#[test]
fn compact_channel_intro_fits_and_its_tools_open_from_the_header() {
    for lang in [Lang::En, Lang::Ko] {
        let mut d = Data::fresh();
        d.extra.welcome = false;
        let mut h = Ui1::sized_with(360.0, 640.0, d);
        set_lang(lang);
        h.app.section = Section::Chat;
        h.settle();
        // the cards' lines end inside the cards, Invite is centred like Import
        let card_right = h.rect("Import from Slack").right();
        let sub = h.painted_text(tr("Manage tasks, bugs, people, and more"));
        let short = h
            .texts()
            .into_iter()
            .any(|x| x.ends_with('\u{2026}') || x == tr("Manage tasks, bugs, people, and more"));
        let invite = h.painted_text(tr("+  Invite people"));
        let import = h.rect("Import from Slack");
        set_lang(Lang::En);
        assert!(short, "{:?}: the line is there, shortened if need be", lang);
        for (r, _) in &sub {
            assert!(r.right() <= card_right + 1.0, "{:?}: {:?}", lang, r);
        }
        assert!(
            (invite[0].0.center().x - import.center().x).abs() < 20.0,
            "{:?}: Invite {:?} centred over {:?}",
            lang,
            invite[0].0,
            import
        );
        // the tools beside the messages: closed, then opened from the header
        assert!(!h.has("Followers"));
        h.click("Channel tools");
        assert!(h.has("Settings"));
        assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
    }
}

/// A profile (someone else's with a long name, and mine) opened in Chat, on
/// the Posts page or in People leaves the page room to read: laid over it
/// in a compact window, narrower in a medium one. It took most of a compact
/// window and squeezed the conversation beside it to 70 points, every line
/// cut mid-glyph.
#[test]
fn profile_leaves_the_page_room_in_narrow_windows() {
    for lang in [Lang::En, Lang::Ko] {
        for w in [360.0, 460.0, 700.0, 960.0, 1440.0] {
            for place in ["chat", "posts", "people"] {
                let mut h = Ui1::sized(w, 640.0);
                set_lang(lang);
                let me = h.app.data.me;
                let other = h.app.data.members.iter().find(|m| m.id != me).unwrap().id;
                let m = h
                    .app
                    .data
                    .members
                    .iter_mut()
                    .find(|m| m.id == other)
                    .unwrap();
                m.name = "Alexandra Konstantinopoulou-Whitfield".into();
                m.title = "Product designer and user researcher".into();
                let general = h.channel_id("general");
                match place {
                    "chat" => {
                        h.app.section = Section::Chat;
                        h.app.chat.open = Some(general);
                    }
                    "posts" => h.app.section = Section::Posts,
                    _ => h.app.section = Section::People,
                }
                for who in [other, me] {
                    if place == "people" {
                        h.app.people_profile = Some(who);
                    } else {
                        h.app.chat.profile = Some(who);
                    }
                    h.settle();
                    let what = format!("{:?} {} {} profile {}", lang, w, place, who);
                    let page = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
                    let cut = h.cut_texts(page);
                    let past = h.past_right_edge();
                    let close = h.rect("Close profile");
                    let composer = h
                        .has("Message composer")
                        .then(|| h.rect("Message composer"));
                    set_lang(Lang::En);
                    assert!(cut.is_empty(), "{}: cut texts: {:?}", what, cut);
                    assert!(past.is_empty(), "{}: past the right edge: {:?}", what, past);
                    assert!(close.right() <= w, "{}: {:?}", what, close);
                    if h.app.tier == Tier::Compact {
                        // over the page, the window's width
                        let r = h.ctx.memory(|m| m.area_rect(egui::Id::new("profile_over")));
                        let r = r.unwrap_or_else(|| panic!("{}: laid over the page", what));
                        assert!(r.width() > w - 40.0, "{}: {:?}", what, r);
                    } else if place == "chat" {
                        // beside it, with room for the message box
                        let c = composer.unwrap_or_else(|| panic!("{}: the message box", what));
                        assert!(c.width() >= 200.0, "{}: message box {:?}", what, c);
                        assert!(c.right() <= close.left(), "{}: {:?} {:?}", what, c, close);
                    }
                    set_lang(lang);
                }
                set_lang(Lang::En);
            }
        }
    }
}

/// The widths the narrow layout checks look at.
const WIDTHS: [(f32, f32); 5] = [
    (360.0, 640.0),
    (460.0, 640.0),
    (700.0, 640.0),
    (960.0, 560.0),
    (1440.0, 900.0),
];

/// A channel with its find box open (finding something, nothing, a file,
/// someone) or one kind of message picked: the find box and the chip go on
/// the next row when there is no room, so the conversation stays in the
/// window (they widened it past the edge, cutting the messages, the message
/// box's hint and the chip).
#[test]
fn channel_find_and_message_chips_keep_the_channel_inside() {
    use crate::app::chat::ChannelPanel;
    for lang in [Lang::En, Lang::Ko] {
        for (w, hh) in WIDTHS {
            let mut states: Vec<(Option<&str>, ChannelPanel)> =
                ["homepage", "zzzz", "has:file", "from:alex"]
                    .into_iter()
                    .map(|q| (Some(q), ChannelPanel::All))
                    .collect();
            for p in [
                ChannelPanel::Pinned,
                ChannelPanel::Replies,
                ChannelPanel::Tasks,
                ChannelPanel::Saved,
                ChannelPanel::Files,
                ChannelPanel::Links,
            ] {
                states.push((None, p));
                states.push((Some("homepage"), p));
            }
            for (q, p) in states {
                let mut h = Ui1::sized(w, hh);
                set_lang(lang);
                let general = h.channel_id("general");
                h.app.section = Section::Chat;
                h.app.chat.open = Some(general);
                h.app.channel_find = q.map(String::from);
                h.app.channel_panel = p;
                h.settle();
                let what = format!("{:?} {} find {:?} {:?}", lang, w, q, p);
                let past = h.past_right_edge();
                let cut = h.cut_texts(Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2()));
                set_lang(Lang::En);
                assert!(past.is_empty(), "{}: past the right edge {:?}", what, past);
                assert!(cut.is_empty(), "{}: cut {:?}", what, cut);
                if q.is_some() {
                    let r = h.rect("Close find");
                    assert!(r.right() <= w, "{}: {:?}", what, r);
                }
            }
        }
    }
}

/// Chat with no conversation yet: its three buttons fit the window, one
/// under another when a row has no room (the third was cut by the edge).
#[test]
fn chat_without_channels_fits_its_buttons() {
    for lang in [Lang::En, Lang::Ko] {
        for (w, hh) in WIDTHS {
            let mut h = Ui1::sized(w, hh);
            set_lang(lang);
            h.app.data.channels.clear();
            h.app.section = Section::Chat;
            h.app.chat.open = None;
            h.settle();
            let what = format!("{:?} {}", lang, w);
            let past = h.past_right_edge();
            let cut = h.cut_texts(Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2()));
            let names = ["Create a Channel", "Message someone", "Work together"];
            let rects: Vec<Rect> = names.iter().map(|n| h.rect(n)).collect();
            set_lang(Lang::En);
            assert!(past.is_empty(), "{}: past the right edge {:?}", what, past);
            assert!(cut.is_empty(), "{}: cut {:?}", what, cut);
            for (n, r) in names.iter().zip(&rects) {
                assert!(r.left() >= 0.0 && r.right() <= w, "{}: {} {:?}", what, n, r);
            }
            for i in 0..rects.len() {
                for j in i + 1..rects.len() {
                    assert!(!rects[i].intersects(rects[j]), "{}: {:?}", what, rects);
                }
            }
        }
    }
}
