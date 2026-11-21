// Click-through tests for the Inbox page: moving rows between Primary,
// Other, Later and Cleared with the buttons a row shows under the pointer,
// the filter, and what the rows say.

use super::*;

/// The task the Inbox rows below are about: mine (I made it and I'm on it).
const CASE: &str = "Inbox case";

/// A window on the Inbox with only the rows `setup` adds: every line by
/// someone else on a task and every message are taken out first.
fn inbox(setup: impl FnOnce(&mut Data)) -> Ui1 {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    for t in &mut h.app.data.tasks {
        t.activity.retain(|a| a.author == me);
    }
    for c in &mut h.app.data.channels {
        c.messages.clear();
    }
    setup(&mut h.app.data);
    h.app.data_changed();
    // the pointer is on the window, as it is for a person
    h.hover(pos2(700.0, 400.0));
    h.click("Inbox");
    h
}

/// Adds the task `CASE` (or `name`) with a comment by Alex `ago` seconds
/// ago; returns the comment's Inbox key.
fn comment_on(d: &mut Data, name: &str, text: &str, ago: i64) -> String {
    let me = d.me;
    let ts = now_ts() - ago;
    let list = d.tasks[0].list_id;
    let id = match d.tasks.iter().find(|t| t.name == name) {
        Some(t) => t.id,
        None => {
            let id = d.new_id();
            d.tasks.push(Task {
                id,
                list_id: list,
                name: name.into(),
                assignees: vec![me],
                activity: vec![Activity {
                    ts: ts - 100,
                    author: me,
                    text: "created this task".into(),
                    ..Default::default()
                }],
                ..d.tasks[0].clone()
            });
            id
        }
    };
    let t = d.tasks.iter_mut().find(|t| t.id == id).unwrap();
    t.activity.push(Activity {
        ts,
        author: 2,
        text: text.into(),
        comment: true,
        ..Default::default()
    });
    format!("{}:{}:2", id, ts)
}

/// Puts the pointer on the row named `name`, so it shows its buttons.
fn hover_row(h: &mut Ui1, name: &str) {
    let p = h
        .find(name)
        .unwrap_or_else(|| panic!("no Inbox row named {:?}", name));
    h.hover(p);
}

#[test]
fn later_on_a_cleared_row_moves_it_to_later_and_it_can_go_back() {
    let mut key = String::new();
    let mut h = inbox(|d| {
        key = comment_on(d, CASE, "Over to you", 60);
        d.extra.inbox_cleared.push(key.clone());
    });
    h.click("Inbox Cleared");
    hover_row(&mut h, CASE);
    h.click("Later");
    let x = &h.app.data.extra;
    assert!(!x.inbox_cleared.contains(&key), "no longer cleared");
    assert!(x.inbox_later.contains(&key), "saved for later");
    assert!(!h.has(CASE), "gone from Cleared");
    h.click("Inbox Later");
    hover_row(&mut h, CASE);
    // and from Later back to Primary, where it came from
    h.click("Primary");
    h.click("Inbox Primary");
    assert!(h.has(CASE));
    assert!(h.app.data.extra.inbox_later.is_empty());
    assert!(h.app.data.extra.inbox_cleared.is_empty());
}

/// Adds a message in #general by `author` that mentions me; returns its
/// Inbox key.
fn mention_in_general(d: &mut Data, author: u64, text: &str, ago: i64) -> String {
    let me = d.me;
    let id = d.new_id();
    let mut m = crate::app::slack::new_message(id, author, text.into(), now_ts() - ago);
    m.mentions = vec![me];
    let c = d
        .channels
        .iter_mut()
        .find(|c| c.name == "general")
        .expect("#general");
    c.messages.push(m);
    format!("chat:{}:{}", c.id, id)
}

#[test]
fn an_imported_row_offers_no_restore_it_cannot_do() {
    let mut key = String::new();
    let mut h = inbox(|d| {
        key = mention_in_general(d, 2, "@Me an old Slack mention", 30 * 86400);
        let c = d.channels.iter_mut().find(|c| c.name == "general").unwrap();
        c.messages.last_mut().unwrap().imported = Some("slack".into());
    });
    // old history starts out cleared
    h.click("Inbox Cleared");
    hover_row(&mut h, "#general");
    assert!(
        !h.has("Restore"),
        "imported history stays cleared, so Restore would do nothing"
    );
    // it can still be saved for later, and cleared again from there
    h.click("Later");
    assert!(h.app.data.extra.inbox_later.contains(&key));
    h.click("Inbox Later");
    hover_row(&mut h, "#general");
    h.click("Clear");
    h.click("Inbox Cleared");
    assert!(h.has("#general"));
}

/// Presses and releases the mouse button twice at `p` in quick succession.
fn double_click_at(h: &mut Ui1, p: Pos2) {
    h.frame(vec![egui::Event::PointerMoved(p)]);
    for _ in 0..2 {
        for pressed in [true, false] {
            h.frame(vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            }]);
        }
    }
    h.settle();
}

#[test]
fn a_double_click_on_clear_clears_that_row_only() {
    let (mut first, mut second) = (String::new(), String::new());
    let mut h = inbox(|d| {
        first = comment_on(d, CASE, "First", 60);
        second = comment_on(d, "Inbox case two", "Second", 120);
    });
    hover_row(&mut h, CASE);
    let clear = h.find("Clear").expect("the row's Clear button");
    // the first click clears the row and the next one moves up under the
    // pointer: the second click of the double click must not clear it too
    double_click_at(&mut h, clear);
    assert_eq!(h.app.data.extra.inbox_cleared, vec![first]);
    assert!(h.has("Inbox case two"), "the next row is still in Primary");
    assert_eq!(h.app.open_task, None, "and no task opened");
    // a separate click later clears the next row
    h.hover(pos2(clear.x, clear.y + 200.0));
    hover_row(&mut h, "Inbox case two");
    h.click("Clear");
    assert!(h.app.data.extra.inbox_cleared.contains(&second));
}

#[test]
fn a_filter_that_hides_every_row_says_so_and_offers_everything() {
    let mut h = inbox(|d| {
        comment_on(d, CASE, "Over to you", 60);
    });
    // the only row is a comment, not an assignment
    h.click("Filter");
    h.click("Assigned to me");
    assert!(!h.has(CASE));
    assert!(
        !h.has("You're all caught up"),
        "there is a row on this tab, only filtered out"
    );
    assert!(h.has("Nothing matches this filter"));
    // with a reminder above the rows, too
    let me = h.app.data.me;
    let id = h.app.data.new_id();
    h.app.data.extra.reminders.push(Reminder {
        id,
        text: "Call the bank".into(),
        due_ts: now_ts() + 3600,
        owner: me,
        ..Default::default()
    });
    h.settle();
    assert!(!h.has("No new comments, mentions or assignments."));
    assert!(h.has("Nothing matches this filter"));
    h.click("Show everything");
    assert_eq!(h.app.inbox_filter, 0);
    assert!(h.has(CASE));
    // an empty tab with no filter still says it is empty
    h.click("Inbox Other");
    assert!(h.has("Nothing in Other"));
    assert!(!h.has("Nothing matches this filter"));
}

#[test]
fn a_comment_on_several_lines_shows_on_one_line_under_its_title() {
    let mut h = inbox(|d| {
        comment_on(d, CASE, "First line\nSecond line\n\n  Third line", 60);
    });
    let line = h.painted_text("Alex Kim commented: First line Second line Third line");
    assert_eq!(line.len(), 1, "the comment runs on in one line");
    let title = h.painted_text(CASE);
    let (t, l) = (title[0].0, line[0].0);
    assert!(
        l.top() >= t.bottom() - 1.0 && l.height() < 20.0,
        "the line stays under the title and in its row: {t:?} {l:?}"
    );
    // the Replies page shows the same line
    h.click("Replies");
    assert_eq!(
        h.painted_text("Alex Kim commented: First line Second line Third line")
            .len(),
        1
    );
}

#[test]
fn a_chat_row_shows_the_message_without_its_formatting_marks() {
    let mut h = inbox(|d| {
        mention_in_general(d, 2, "@Me please read **this** and `that` ~~now~~", 60);
        // a task comment keeps its text as written: comments show no formatting
        comment_on(d, CASE, "2 * 3 = 6", 120);
    });
    assert_eq!(
        h.painted_text("Alex Kim mentioned you: @Me please read this and that now")
            .len(),
        1,
        "the message as Chat shows it, without ** ` ~~"
    );
    assert_eq!(h.painted_text("Alex Kim commented: 2 * 3 = 6").len(), 1);
}

/// Where the text `text` was painted and the rectangle it was clipped to.
fn text_and_clip(h: &Ui1, text: &str) -> Option<(Rect, Rect)> {
    let mut found = None;
    each_text(&h.shapes, |ts, clip| {
        if ts.galley.text() == text {
            found = Some((ts.visual_bounding_rect(), clip));
        }
    });
    found
}

#[test]
fn a_long_title_stops_before_the_time_in_the_smallest_window() {
    const LONG: &str = "A very long task name that keeps going and going so that it runs right across the row and under the time on the right";
    let mut h = Ui1::sized(900.0, 560.0);
    let me = h.app.data.me;
    for t in &mut h.app.data.tasks {
        t.activity.retain(|a| a.author == me);
    }
    for c in &mut h.app.data.channels {
        c.messages.clear();
    }
    comment_on(&mut h.app.data, LONG, "Short comment", 3 * 3600);
    h.app.data_changed();
    // the pointer is on the window, as it is for a person
    h.hover(pos2(450.0, 300.0));
    h.click("Inbox");
    // away from the row, so its buttons don't show
    h.hover(pos2(450.0, 80.0));
    let (title, clip) = text_and_clip(&h, LONG).expect("the title is drawn");
    let time = h.painted_text(&fmt_ago(now_ts() - 3 * 3600));
    assert_eq!(time.len(), 1);
    let shown_right = title.right().min(clip.right());
    assert!(
        shown_right <= time[0].0.left() - 8.0,
        "the title ({shown_right}) runs into the time ({:?})",
        time[0].0
    );
}

#[test]
fn the_row_buttons_do_not_sit_on_top_of_a_long_time() {
    let ago = 400 * 86400;
    let mut h = inbox(|d| {
        comment_on(d, CASE, "A comment from last year", ago);
    });
    let when = fmt_ago(now_ts() - ago);
    assert_eq!(h.painted_text(&when).len(), 1, "{when} is shown");
    hover_row(&mut h, CASE);
    for b in ["Later", "Clear"] {
        let br = h.rect(b);
        for (tr, _) in h.painted_text(&when) {
            assert!(
                !tr.intersects(br),
                "{b} {br:?} is drawn over the time {when:?} {tr:?}"
            );
        }
    }
    // away from the row the time shows again
    h.hover(pos2(700.0, 80.0));
    assert_eq!(h.painted_text(&when).len(), 1);
}

#[test]
fn rows_from_someone_who_left_say_someone_instead_of_nothing() {
    let mut h = inbox(|d| {
        let me = d.me;
        // member 99 left the team: no longer in members
        let id = d.new_id();
        let mut m = crate::app::slack::new_message(
            id,
            99,
            "@Me a message before I left".into(),
            now_ts() - 60,
        );
        m.mentions = vec![me];
        let dm = d.new_id();
        d.channels.push(Channel {
            id: dm,
            kind: ChannelKind::Direct,
            members: vec![me, 99],
            messages: vec![m],
            ..Default::default()
        });
        comment_on(d, CASE, "A comment before I left", 120);
        let t = d.tasks.iter_mut().find(|t| t.name == CASE).unwrap();
        t.activity.last_mut().unwrap().author = 99;
    });
    assert!(h.has("Someone"), "the direct message row is named");
    assert!(!h.has(""), "no row without a name");
    assert_eq!(
        h.painted_text("Someone mentioned you: @Me a message before I left")
            .len(),
        1
    );
    assert_eq!(
        h.painted_text("Someone commented: A comment before I left")
            .len(),
        1
    );
}
