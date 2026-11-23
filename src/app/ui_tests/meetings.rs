// The Meetings page: the agenda and notes stay reachable with many
// attendees in a small window, times typed loosely are tidied and ordered by
// the clock, and a meeting whose name was cleared still has a row.

use super::*;

const NOTES_HINT: &str = "Meeting notes";

fn many_people(h: &mut Ui1) {
    for k in 0..24u64 {
        h.app.data.members.push(Member {
            id: 900 + k,
            name: format!("Teammate number {}", k),
            color: [120, 80, 200],
            email: format!("t{}@example.com", k),
            role: "Member".into(),
            ..Default::default()
        });
    }
}

fn open_meetings(h: &mut Ui1) {
    h.app.section = Section::Meetings;
    h.settle();
}

/// Many attendees in the smallest window push the notes below the window:
/// the page scrolls to them and they take typing.
#[test]
fn meeting_notes_scroll_into_reach_with_many_attendees() {
    let mut h = Ui1::sized(960.0, 560.0);
    many_people(&mut h);
    open_meetings(&mut h);
    h.click("Add a meeting");
    let below = |h: &Ui1| h.find(NOTES_HINT).is_none_or(|p| p.y > h.size.y);
    assert!(below(&h), "the notes start below the window");
    // the mouse wheel over the meeting brings them up
    for _ in 0..20 {
        if !below(&h) {
            break;
        }
        h.frame(vec![egui::Event::PointerMoved(pos2(760.0, 300.0))]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -120.0),
            modifiers: Default::default(),
        }]);
        h.settle();
    }
    assert!(!below(&h), "the notes scroll into the window");
    h.click(NOTES_HINT);
    h.type_text("Decide the launch date");
    let m = h.app.data.extra.meetings.last().unwrap();
    assert_eq!(m.notes, "Decide the launch date");
}

/// "9" typed for the time becomes 09:00 once the field is left, and that
/// meeting is listed before one at 10:00 the same day.
#[test]
fn meeting_time_is_tidied_and_ordered_by_the_clock() {
    let mut h = Ui1::new();
    open_meetings(&mut h);
    h.click("Add a meeting");
    h.click("Add a meeting");
    let second = h.app.meeting_open.unwrap();
    h.click("Meeting time");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("9");
    h.key(egui::Key::Tab);
    h.settle();
    let m = h
        .app
        .data
        .extra
        .meetings
        .iter()
        .find(|m| m.id == second)
        .unwrap();
    assert_eq!(m.time, "09:00");
    // both are "New meeting": the 09:00 one's row comes first
    let rows: Vec<f32> = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Open meeting New meeting")
        .map(|(_, r)| r.center().y)
        .collect();
    assert_eq!(rows.len(), 2);
    let times: Vec<f32> = ["09:00", "10:00"]
        .iter()
        .map(|t| {
            h.painted_text(&format!("{} \u{2022} {}", fmt_date(today()), t))[0]
                .0
                .center()
                .y
        })
        .collect();
    assert!(times[0] < times[1], "09:00 above 10:00: {:?}", times);
}

/// A time that can't be read says how to write one.
#[test]
fn meeting_time_that_is_not_a_time_says_so() {
    let mut h = Ui1::new();
    open_meetings(&mut h);
    h.click("Add a meeting");
    h.click("Meeting time");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("soon");
    h.key(egui::Key::Tab);
    h.settle();
    assert!(!h.painted_text("Use a time like 14:30").is_empty());
}

/// Clearing a meeting's name leaves "Untitled" in its row, not a blank.
#[test]
fn meeting_without_a_name_is_untitled_in_the_list() {
    let mut h = Ui1::new();
    open_meetings(&mut h);
    h.click("Add a meeting");
    let id = h.app.meeting_open.unwrap();
    if let Some(m) = h.app.data.extra.meetings.iter_mut().find(|m| m.id == id) {
        m.title.clear();
    }
    h.settle();
    assert!(!h.painted_text("Untitled").is_empty());
}

/// With the Delete dialog up, typing doesn't go on in the notes behind it.
/// Delete is pressed the way a screen reader or the keyboard does, which
/// leaves the notes focused (a mouse click elsewhere would end the editing).
#[test]
fn typing_with_a_dialog_up_does_not_reach_the_page_behind() {
    let mut h = Ui1::new();
    open_meetings(&mut h);
    h.click("Add a meeting");
    h.click(NOTES_HINT);
    h.type_text("Agreed");
    h.accessibility_click("Delete");
    h.settle();
    assert!(h.app.modal.is_some());
    h.type_text("zz");
    let m = h.app.data.extra.meetings.last().unwrap();
    assert_eq!(m.notes, "Agreed");
}

/// One action item: the button says one task, not "1 tasks".
#[test]
fn one_action_item_makes_a_task() {
    let mut h = Ui1::new();
    open_meetings(&mut h);
    h.click("Add a meeting");
    h.click(NOTES_HINT);
    h.type_text("[ ] Book the room");
    h.settle();
    assert!(h
        .painted_text("Create 1 tasks from action items")
        .is_empty());
    h.click("Create a task from the action item");
    assert!(h.app.data.tasks.iter().any(|x| x.name == "Book the room"));
}

/// Below a wide window the Meetings page is one column: the meeting open
/// takes the page (a way back leads to the list) instead of a sliver beside
/// the list. A long name wraps instead of being cut, and a time that can't
/// be read says so under the row, clear of Delete and the field.
#[test]
fn meeting_page_fits_every_window_size() {
    use crate::i18n::{set_lang, Lang};
    const LONG: &str =
        "Weekly product planning meeting about the launch dates and the work still left";
    for (w, hh) in [
        (1440.0, 900.0),
        (960.0, 560.0),
        (700.0, 640.0),
        (460.0, 640.0),
        (360.0, 640.0),
    ] {
        for lang in [Lang::En, Lang::Ko] {
            set_lang(lang);
            let mut h = Ui1::sized(w, hh);
            open_meetings(&mut h);
            h.click("Add a meeting");
            let id = h.app.meeting_open.unwrap();
            let m = h.app.data.extra.meetings.last_mut().unwrap();
            m.title = LONG.into();
            m.time = "abc".into();
            h.settle();
            let at = format!("{}x{} {:?}", w, hh, lang);
            let one_column = w < 960.0;
            assert_eq!(
                h.has(&format!("Open meeting {}", LONG)),
                !one_column,
                "{}: the list beside the meeting",
                at
            );
            // the meeting has the page's width
            let agenda = h.rect("Meeting agenda");
            let room = if one_column { w - 130.0 } else { 250.0 };
            assert!(agenda.width() >= room, "{}: agenda {:?}", at, agenda);
            let page = Rect::from_min_max(pos2(agenda.left() - 4.0, 40.0), h.size.to_pos2());
            let cut = h.cut_texts(page);
            assert!(cut.is_empty(), "{}: cut {:?}", at, cut);
            let over = layout_tests::overlapping(&h.texts_in(page, Seen::Visible));
            assert!(over.is_empty(), "{}: overlapping {:?}", at, over);
            // Delete is clear of the time field and the warning
            let del = h.rect_in(tr("Delete"), page);
            let time = h.rect("Meeting time");
            assert!(!del.intersects(time), "{}: {:?} over {:?}", at, del, time);
            for (r, _) in h.painted_text(tr("Use a time like 14:30")) {
                assert!(!del.intersects(r), "{}: {:?} over {:?}", at, del, r);
                assert!(r.right() <= agenda.right() + 1.0, "{}: {:?}", at, r);
            }
            assert!(
                h.past_right_edge().is_empty(),
                "{}: {:?}",
                at,
                h.past_right_edge()
            );
            if one_column {
                // back to the list, which then has the page
                h.click("Back to all meetings");
                assert_eq!(h.app.meeting_open, None, "{}", at);
                let row = h.rect(&format!("Open meeting {}", LONG));
                assert!(
                    row.right() <= w && row.width() >= w - 140.0,
                    "{}: {:?}",
                    at,
                    row
                );
                h.click(&format!("Open meeting {}", LONG));
                assert_eq!(h.app.meeting_open, Some(id), "{}", at);
            }
        }
    }
    set_lang(Lang::En);
}
