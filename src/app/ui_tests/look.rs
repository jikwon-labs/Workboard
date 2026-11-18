// Click-through tests for how controls look and respond: themes and
// contrast, hover fills and the hand cursor, control sizes, menus that stay
// on screen and Korean names.

use super::*;

#[test]
fn dots_menus_open_submenus_beside_the_parent() {
    let mut h = Ui1::new();
    // sidebar space "..." > Create new
    h.hover(h.find("Team Space").unwrap());
    h.click("Space options Team Space");
    let parent = h.rect("Create new");
    h.hover(parent.center());
    let sub = h.rect("Folder");
    assert!(
        sub.left() >= parent.right() - 2.0,
        "Create new opens to the right: parent {:?} sub {:?}",
        parent,
        sub
    );
    assert!(h.has("Task statuses"), "the parent menu stays open");
    h.key(egui::Key::Escape);

    // list row "..." > Status
    h.hover(h.find("Task Set up analytics dashboard").unwrap());
    h.click("Task options Set up analytics dashboard");
    let parent = h.rect("Status");
    h.hover(parent.center());
    let sub = h.rect("IN PROGRESS");
    assert!(
        sub.left() >= parent.right() - 2.0,
        "Status opens to the right: parent {:?} sub {:?}",
        parent,
        sub
    );
    h.click("IN PROGRESS");
    let t = task_named(&h, "Set up analytics dashboard");
    assert_eq!(t.status, "in progress", "picking from the submenu works");
}

#[test]
fn profile_mute_flyout_does_not_cover_the_menu() {
    let mut h = Ui1::new();
    h.click("Profile menu");
    let row = h.rect("Mute notifications");
    h.hover(row.center());
    let item = h.rect("For 1 hour");
    let settings = h.rect("Settings");
    assert!(
        item.right() <= settings.left() || item.left() >= settings.right(),
        "flyout {:?} sits beside the menu row {:?}",
        item,
        settings
    );
    // walk the pointer from the row to the flyout; it must stay open
    let to = item.center();
    for i in 1..=8 {
        let p = row.center() + (to - row.center()) * (i as f32 / 8.0);
        h.frame(vec![egui::Event::PointerMoved(p)]);
    }
    h.settle();
    h.click("For 1 hour");
    assert!(
        h.app.data.extra.muted_until > now_ts(),
        "muted from the flyout"
    );
}

#[test]
fn shortcut_symbols_have_glyphs() {
    if !std::path::Path::new("/System/Library/Fonts/SFNS.ttf").exists() {
        return; // the system font is only on macOS
    }
    let ctx = egui::Context::default();
    crate::app::setup_fonts(&ctx);
    let _ = ctx.run(egui::RawInput::default(), |_| {});
    for c in ['\u{2325}', '\u{2318}', '\u{21E7}'] {
        for f in [FontId::monospace(13.0), FontId::proportional(13.0)] {
            assert!(
                ctx.fonts(|x| x.has_glyph(&f, c)),
                "{:?} has a glyph in {:?}",
                c,
                f
            );
        }
    }
}

#[test]
fn short_window_profile_menu_scrolls_to_trash_and_switch_person() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.click("Profile menu");
    h.settle();
    let menu_top = h.rect("Settings").top();
    // the menu does not run off the window: its bottom row is reached by
    // scrolling inside the menu
    for name in ["Trash", "Switch person"] {
        h.scroll_into_view(name);
        let r = h.rect(name);
        assert!(
            r.top() >= menu_top - 400.0 && r.bottom() <= 560.0,
            "{} is on screen after scrolling the menu: {:?}",
            name,
            r
        );
    }
    h.click("Switch person");
    assert!(
        matches!(h.app.modal, Some(Modal::SwitchUser)),
        "the bottom row of the menu works"
    );
    // in the tall window the menu still works as before
    h.key(egui::Key::Escape);
    let mut w = Ui1::new();
    w.click("Profile menu");
    w.click("Trash");
    assert!(matches!(w.app.modal, Some(Modal::Trash)));
}

#[test]
fn add_column_plus_and_reply_link_read_in_both_themes() {
    for dark in [false, true] {
        let mut h = Ui1::new();
        h.app.data.dark = dark;
        h.settle();
        let t = h.app.t();
        // the "+" at the right end of the list column header
        let plus = h
            .painted_text("+")
            .into_iter()
            .find(|(r, _)| h.circle_fill_at(r.center()).is_some())
            .unwrap_or_else(|| panic!("dark {}: no + on a circle", dark));
        let fill = h.circle_fill_at(plus.0.center()).unwrap();
        assert_ne!(
            plus.1, fill,
            "dark {}: + is not drawn in the circle's color",
            dark
        );
        assert!(
            contrast(plus.1, fill) >= 4.5,
            "dark {}: + on circle contrast {:.1}",
            dark,
            contrast(plus.1, fill)
        );
        // the "1 reply" link under a threaded message in #general
        h.click("general");
        let reply = h.painted_text("1 reply");
        assert!(!reply.is_empty(), "dark {}: a thread link is shown", dark);
        for (_, c) in reply {
            assert!(
                contrast(c, t.bg) >= 4.5,
                "dark {}: 1 reply contrast {:.1}",
                dark,
                contrast(c, t.bg)
            );
        }
    }
}

#[test]
fn dark_theme_post_chip_and_ai_hint_read() {
    let mut h = Ui1::new();
    h.app.data.dark = true;
    h.settle();
    let t = h.app.t();
    let bgs = [t.bg, t.card, t.column_bg];
    let best = |c: Color32| bgs.iter().map(|b| contrast(c, *b)).fold(0.0f32, f32::max);
    // a post in the task activity carries a "Post" chip
    h.click_left("Task Write copy for landing page", 120.0);
    h.click("Post");
    h.click("Comment box");
    h.type_text("Shipped");
    h.click("Send Post");
    let chips: Vec<_> = h.painted_text("Post");
    assert!(!chips.is_empty());
    for (r, c) in chips {
        // the comment box's Post button is the theme colour, which is light
        // with dark text in the black theme: that label reads on its fill
        let on = if c == t.on_primary {
            contrast(c, t.primary)
        } else {
            best(c)
        };
        assert!(on >= 3.0, "Post at {:?} contrast {:.1}", r, on);
    }
    h.key(egui::Key::Escape);
    h.key(egui::Key::Escape);
    // the AI page hint
    h.click("Rail AI");
    let hint = h.painted_text("Ask anything... (available with the server)");
    assert!(!hint.is_empty(), "the hint is shown");
    for (_, c) in hint {
        assert!(best(c) >= 3.0, "AI hint contrast {:.1}", best(c));
    }
}

#[test]
fn frameless_buttons_show_the_hand_and_a_hover_fill() {
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
    // the channel header's bell, like Ask and Integrate next to it
    let bell = h.find("Channel notifications").unwrap();
    h.hover(pos2(bell.x, bell.y + 200.0));
    assert!(
        !h.hover_fill_at(bell),
        "no fill while the pointer is elsewhere"
    );
    h.hover(bell);
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand);
    assert!(h.hover_fill_at(bell), "a fill appears under the pointer");
}

#[test]
fn reaction_bar_buttons_show_which_one_is_under_the_pointer() {
    let (mut h, _) = general_with_my_message("hover the bar");
    let thumbs = h.find("\u{1F44D}").unwrap();
    h.hover(thumbs);
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand);
    assert!(h.hover_fill_at(thumbs));
    // only the button under the pointer is lit
    let other = h.find("More reactions").unwrap();
    assert!(!h.hover_fill_at(other));
}

#[test]
fn painted_controls_get_the_hand_but_fields_and_backdrops_do_not() {
    let mut h = Ui1::new();
    h.app.section = Section::Teams;
    h.settle();
    h.click_side("Create team", true);
    // a colour swatch is painted by hand, not an egui button
    let sw = h.find("Team color 3").unwrap();
    h.hover(sw);
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand);

    // a text field keeps its I-beam
    let mut h = Ui1::new();
    h.click("general");
    let field = h.find("Message composer").unwrap();
    h.hover(field);
    assert_eq!(h.cursor, egui::CursorIcon::Text);

    // the dim backdrop behind an open task is clickable (it closes the task)
    // but is not a control
    let mut h = Ui1::new();
    h.open_clean_task();
    h.hover(pos2(6.0, 450.0));
    assert!(h.app.open_task.is_some());
    assert_eq!(h.cursor, egui::CursorIcon::Default);
}

#[test]
fn filled_buttons_tint_on_hover() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Message composer");
    h.type_text("ready to send");
    // the channel composer's send arrow turns solid once there is text
    let send = h.find("Send").unwrap();
    h.hover(pos2(send.x, send.y + 200.0));
    assert!(!h.hover_fill_at(send));
    h.hover(send);
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand);
    assert!(
        h.hover_fill_at(send),
        "a tint goes over the button's own fill"
    );
}

/// A text field, the buttons and drop-downs beside it are one height, and
/// sit on one line (the fields used to be 20 pt next to 28 pt buttons).
#[test]
fn fields_buttons_and_drop_downs_in_a_dialog_are_one_height() {
    let mut h = Ui1::new();
    h.app.modal = Some(Modal::SetStatus {
        text: String::new(),
    });
    h.settle();
    // a text field reports the box its text sits in: add the field's
    // 5 pt top and bottom margin for its frame
    let framed = |r: Rect| r.expand2(vec2(0.0, 5.0));
    let field = framed(h.rect("Status text"));
    let save = h.rect("Save");
    let clear = h.rect("Clear status");
    assert!(
        field.height() >= save.height() - 0.5,
        "status field {} vs Save {}",
        field.height(),
        save.height()
    );
    assert!(
        (save.height() - clear.height()).abs() < 0.5,
        "Save and Clear status"
    );

    h.app.modal = Some(Modal::Reminder {
        text: String::new(),
        date: Some(today()),
        time: "09:00".into(),
    });
    h.settle();
    let day = h.rect("Reminder date");
    let time = framed(h.rect("Reminder time"));
    assert!(
        (day.height() - time.height()).abs() < 0.5,
        "Today and the time"
    );
    assert!((day.center().y - time.center().y).abs() < 0.5);

    h.app.modal = Some(Modal::CustomFields {
        name: String::new(),
        kind: "text".into(),
        opts: String::new(),
    });
    h.settle();
    let name = framed(h.rect("Field name"));
    let kind = h.rect("Field type");
    assert!(
        (name.height() - kind.height()).abs() < 0.5,
        "field and drop-down"
    );
    assert!(
        (name.top() - kind.top()).abs() < 0.5,
        "the drop-down sits on the field's line: {} vs {}",
        name.top(),
        kind.top()
    );
}

/// Check boxes are small squares, filled when on, outlined when off (egui's
/// own took the widgets' 6 pt corners and came out as circles).
#[test]
fn check_boxes_are_squares_that_fill_when_ticked() {
    let mut h = Ui1::new();
    h.app.modal = Some(Modal::CustomizeNav);
    h.settle();
    fn boxes(s: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match s {
            egui::Shape::Rect(r)
                if (r.rect.width() - 15.0).abs() < 0.1 && (r.rect.height() - 15.0).abs() < 0.1 =>
            {
                out.push(r.clone())
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| boxes(s, out)),
            _ => {}
        }
    }
    let mut found = vec![];
    for c in &h.shapes {
        boxes(&c.shape, &mut found);
    }
    assert!(found.len() >= 4, "a box per app: {}", found.len());
    for b in &found {
        assert!(b.corner_radius.nw <= 4, "square corners, not a circle");
    }
    let filled = found
        .iter()
        .filter(|b| {
            b.fill != h.app.t().bg && b.fill != Color32::TRANSPARENT && b.stroke.width == 0.0
        })
        .count();
    let outlined = found.iter().filter(|b| b.stroke.width > 0.0).count();
    assert!(
        filled > 0 && outlined > 0,
        "ticked boxes filled ({}), the others outlined ({})",
        filled,
        outlined
    );
}

/// What the app names a whiteboard, form, team or workspace when it makes
/// one is saved in the language of the person who made it.
#[test]
fn things_the_app_names_are_named_in_korean() {
    let mut h = Ui1::new();
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    h.app.new_whiteboard();
    h.app.new_form();
    h.app.new_team();
    let fresh = Data::fresh();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let d = &h.app.data.extra;
    assert_eq!(d.whiteboards.last().unwrap().name, "화이트보드 1");
    let f = d.forms.last().unwrap();
    assert_eq!(f.name, "새 양식");
    assert_eq!(f.fields[0].label, "제목");
    assert!(d.teams.last().unwrap().name.starts_with("팀 "));
    assert_eq!(fresh.workspace, "내 워크스페이스");
    assert_eq!(fresh.spaces[0].name, "기본 스페이스");
    assert_eq!(fresh.spaces[0].lists[0].name, "목록");
    // and in English, as before
    h.app.new_whiteboard();
    assert_eq!(
        h.app.data.extra.whiteboards.last().unwrap().name,
        "Whiteboard 2"
    );
}

/// A call message is saved in English for every device and shown in the
/// reader's language; a member who joined is "활성", not the task status.
#[test]
fn call_messages_and_member_state_read_in_korean() {
    let mut h = Ui1::new();
    let ch = h.app.data.channels[0].id;
    h.app.start_call(None, ch, "https://meet.example/x");
    let m = h.app.data.channels[0].messages.last().unwrap().clone();
    assert_eq!(m.text, "Started a call", "saved as before");
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    assert_eq!(crate::app::chat::shown_text(&m), "통화를 시작했습니다");
    assert_eq!(crate::i18n::tr_ctx("Active (member)", "Active"), "활성");
    assert_eq!(
        crate::i18n::tr("Active"),
        "진행",
        "the task status keeps its word"
    );
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(crate::app::chat::shown_text(&m), "Started a call");
}

/// The profile menu, opened at the right end of the top bar, keeps a margin
/// from the window's edge instead of touching it.
#[test]
fn profile_menu_keeps_off_the_window_edge() {
    let mut h = Ui1::new();
    h.click("Profile menu");
    h.settle();
    let r = h.rect("Set status");
    // 8 pt of margin outside the menu's own frame (about 7 pt)
    assert!(
        r.right() <= h.size.x - 14.0,
        "Set status ends at {}",
        r.right()
    );
}

/// Radio buttons that are off have a ring that shows on the page (egui's
/// own was border grey on the page colour and vanished in the dark theme).
#[test]
fn radio_buttons_that_are_off_have_a_visible_ring() {
    let mut h = Ui1::new();
    h.app.data.dark = true;
    // Settings > Language: three choices at the top of the page
    h.app.open_settings("Language");
    h.settle();
    fn rings(s: &egui::Shape, out: &mut Vec<egui::epaint::CircleShape>) {
        match s {
            // grey rings of the radio size (status icons are coloured)
            egui::Shape::Circle(c)
                if (c.radius - 6.9).abs() < 0.2
                    && c.stroke.color.r() == c.stroke.color.g()
                    && c.stroke.color.g() == c.stroke.color.b() =>
            {
                out.push(*c)
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| rings(s, out)),
            _ => {}
        }
    }
    let mut found = vec![];
    for c in &h.shapes {
        rings(&c.shape, &mut found);
    }
    assert!(found.len() >= 2, "the choices that are off draw rings");
    let page = h.app.t().bg;
    for r in &found {
        assert!(r.stroke.width >= 1.0);
        let d = (r.stroke.color.r() as i32 - page.r() as i32).abs();
        assert!(
            d >= 50,
            "the ring stands out from the page: {:?}",
            r.stroke.color
        );
    }
}

#[test]
fn black_theme_primary_button_reads_as_enabled_in_dark_mode() {
    let mut h = Ui1::new();
    h.app.data.dark = true;
    h.app.data.extra.accent = "black".into();
    h.click("Rail More");
    h.click("Goals");
    h.settle();
    let t = h.app.t();
    let (r, text) = h.painted_text("Set a Goal")[0];
    // the button's own fill under its label
    fn fills(s: &egui::Shape, p: Pos2, out: &mut Vec<Color32>) {
        match s {
            egui::Shape::Rect(r) if r.rect.contains(p) && r.rect.width() < 300.0 => {
                out.push(r.fill)
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| fills(s, p, out)),
            _ => {}
        }
    }
    let mut found = vec![];
    for c in &h.shapes {
        fills(&c.shape, r.center(), &mut found);
    }
    assert!(
        found.contains(&t.primary),
        "the button is filled: {found:?}"
    );
    // a mid-grey fill with white text looked like a disabled button: the
    // fill now stands out from the page and the label reads on it
    assert!(
        contrast(t.primary, t.bg) >= 3.0,
        "the button stands out from the page"
    );
    assert!(
        contrast(text, t.primary) >= 4.5,
        "the label reads: {text:?}"
    );
    // the other colours keep white text on their own fill
    h.app.data.extra.accent = "purple".into();
    let t = h.app.t();
    assert_eq!(t.on_primary, Color32::WHITE);
    assert!(contrast(t.on_primary, t.primary) >= 4.5);
}
