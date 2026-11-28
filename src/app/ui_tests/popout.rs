// Pages moved out to windows of their own (popout.rs): a doc, a whiteboard,
// a meeting, a task and a huddle's shared screen each move out with "Open in
// new window", work there while the main window shows something else, and
// go back with "Back to main window". Here the windows are windows inside
// the app's, as there is only one window in the tests.

use super::*;
use crate::app::popout::Pop;

#[test]
fn doc_moves_to_its_own_window_and_back() {
    let mut h = Ui1::new();
    h.app.section = Section::Docs;
    let id = h.new_doc_focused();
    h.type_text("before");
    assert!(!h.has("Back to main window"));

    h.click("Open in new window");
    assert!(h.app.popped(Pop::Doc(id)), "the doc has its own window");
    assert_eq!(h.app.doc_open, None, "the main window shows the list");
    assert!(h.has("Back to main window"));
    assert!(
        !h.has("Open in new window"),
        "no second window from the window"
    );

    // written in the window while the main window shows a conversation
    h.app.section = Section::Chat;
    h.settle();
    h.ctx
        .memory_mut(|m| m.request_focus(egui::Id::new(("doc_body", id))));
    h.settle();
    h.type_text(" after");
    assert_eq!(h.doc_body(id), "before after");
    assert!(h.has("Back to main window"), "the window stays");

    // opened again in the main window: its window comes up instead
    h.app.section = Section::Docs;
    h.app.doc_open = Some(id);
    h.settle();
    assert_eq!(h.app.doc_open, None, "not shown twice");
    assert!(h.app.popped(Pop::Doc(id)));

    // back where it was
    h.click("Back to main window");
    assert!(!h.app.popped(Pop::Doc(id)));
    assert_eq!(h.app.section, Section::Docs);
    assert_eq!(h.app.doc_open, Some(id));
    assert!(h.has("Open in new window"));
}

#[test]
fn deleting_a_doc_closes_its_window() {
    let mut h = Ui1::new();
    h.app.section = Section::Docs;
    let id = h.new_doc_focused();
    h.click("Open in new window");
    assert!(h.app.popped(Pop::Doc(id)));
    h.app.trash_doc(id);
    h.settle();
    assert!(!h.app.popped(Pop::Doc(id)));
    assert!(!h.has("Back to main window"));
}

#[test]
fn whiteboard_moves_to_its_own_window_and_back() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.sweep.wb.open.unwrap();
    // not the card a board just made shows
    h.app.sweep.wb.new = None;
    h.settle();
    h.click("Open in new window");
    assert!(h.app.popped(Pop::Board(wid)));
    assert_eq!(
        h.app.sweep.wb.open, None,
        "the main window shows the boards"
    );
    assert!(h.has("Whiteboard canvas"), "the board is in its window");
    assert!(h.has("Back to main window"));

    // drawn on in the window
    let c = h.rect("Whiteboard canvas").center();
    h.click("Pen");
    h.drag_by(c, c + vec2(80.0, 30.0));
    let strokes = |h: &Ui1| {
        h.app
            .data
            .extra
            .whiteboards
            .iter()
            .find(|w| w.id == wid)
            .unwrap()
            .strokes
            .len()
    };
    assert_eq!(strokes(&h), 1, "the pen draws in the window");

    h.click("Back to main window");
    assert!(!h.app.popped(Pop::Board(wid)));
    assert_eq!(h.app.section, Section::Whiteboards);
    assert_eq!(h.app.sweep.wb.open, Some(wid));
}

#[test]
fn meeting_moves_to_its_own_window_and_back() {
    let mut h = Ui1::new();
    h.app.section = Section::Meetings;
    h.settle();
    h.click("Add a meeting");
    let mid = h.app.meeting_open.unwrap();
    h.click("Open in new window");
    assert!(h.app.popped(Pop::Meeting(mid)));
    assert_eq!(h.app.meeting_open, None);
    assert!(h.has("Meeting name"), "the meeting is in its window");

    h.app.section = Section::Chat;
    h.settle();
    h.click("Meeting name");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("Weekly sync");
    let title = |h: &Ui1| {
        h.app
            .data
            .extra
            .meetings
            .iter()
            .find(|m| m.id == mid)
            .unwrap()
            .title
            .clone()
    };
    assert_eq!(title(&h), "Weekly sync");

    h.click("Back to main window");
    assert!(!h.app.popped(Pop::Meeting(mid)));
    assert_eq!(h.app.section, Section::Meetings);
    assert_eq!(h.app.meeting_open, Some(mid));
}

#[test]
fn task_moves_to_its_own_window_and_back() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    h.click("Open in new window");
    assert!(h.app.popped(Pop::Task(id)));
    assert_eq!(h.app.open_task, None, "the dialog over the page is gone");
    assert!(h.has("Close task"), "the task is in its window");

    // a comment from the window
    h.click("Comment box");
    h.type_text("from the window");
    h.cmd_enter();
    assert!(
        h.comments(id).iter().any(|a| a.text == "from the window"),
        "{:?}",
        h.comments(id)
    );

    h.click("Back to main window");
    assert!(!h.app.popped(Pop::Task(id)));
    assert_eq!(h.app.open_task, Some(id), "back in the dialog");

    // Close in the window closes it
    h.click("Open in new window");
    assert!(h.app.popped(Pop::Task(id)));
    h.click("Close task");
    assert!(!h.app.popped(Pop::Task(id)));
    assert_eq!(h.app.open_task, None);
}

#[test]
fn shared_screen_moves_to_its_own_window() {
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
        camera: false,
        screen,
        hand: false,
    };
    let them = person(other.id, "dev2", false, true);
    h.app.huddle.channel = Some(ch);
    h.app.huddle.joined = true;
    h.app.huddle.people = vec![person(me, "dev1", true, false), them.clone()];
    let screen = wb_voice::screen_key(&them.identity);
    let pic = wb_voice::Picture {
        width: 192,
        height: 108,
        rgba: vec![120u8; 192 * 108 * 4],
    };
    h.app
        .huddle
        .pictures
        .insert(screen.clone(), (std::sync::Arc::new(pic), true));
    h.settle();
    let name = format!("Screen of {}", other.name);
    let on_stage = h.rect(&name);

    h.click("Open screen in new window");
    assert!(h.app.popped(Pop::Screen));
    assert!(h.has("Back to main window"));
    let in_window = h.rect(&name);
    assert!(
        in_window.width() > on_stage.width(),
        "larger in its window: {:?} {:?}",
        in_window,
        on_stage
    );
    assert!(
        !h.has("Open screen in new window"),
        "the stage doesn't show it"
    );
    assert!(
        h.app.huddle.views.contains_key(&screen),
        "the call still sends it at the size it is drawn"
    );

    // back on the stage
    h.click("Back to main window");
    assert!(!h.app.popped(Pop::Screen));
    assert!(h.has("Open screen in new window"));

    // the sharing ends: its window closes
    h.click("Open screen in new window");
    assert!(h.app.popped(Pop::Screen));
    h.app.huddle.drop_picture(&screen);
    h.settle();
    assert!(!h.app.popped(Pop::Screen));
    assert!(!h.has("Back to main window"));
}
