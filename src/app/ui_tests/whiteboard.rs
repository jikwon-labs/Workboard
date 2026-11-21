// Click-through tests for whiteboards: notes, strokes, pan and zoom, undo
// and a teammate's changes arriving mid-edit.

use super::*;
use crate::app::more::WB_ORIGIN;

/// A whiteboard on a team (a server that isn't connected), open on screen,
/// with three strokes and two notes the team already knows.
fn shared_board(h: &mut Ui1) -> u64 {
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let ids: Vec<u64> = (0..5).map(|_| h.app.data.new_id()).collect();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    for (k, id) in ids[..3].iter().enumerate() {
        wb.strokes.push(WbStroke {
            id: *id,
            color: [0, 0, 0],
            width: 2.0,
            points: vec![
                [10.0, 10.0 + k as f32 * 20.0],
                [60.0, 10.0 + k as f32 * 20.0],
            ],
        });
    }
    for (k, id) in ids[3..].iter().enumerate() {
        wb.notes.push(Sticky {
            id: *id,
            pos: [300.0 + k as f32 * 200.0, 600.0],
            text: format!("Known {}", k),
            color: [0xf8, 0xae, 0x00],
        });
    }
    h.app.dirty = true;
    h.settle();
    h.app.sync_note_change();
    wid
}

fn board_of(h: &Ui1, wid: u64) -> Whiteboard {
    h.app
        .data
        .extra
        .whiteboards
        .iter()
        .find(|w| w.id == wid)
        .cloned()
        .unwrap()
}

#[test]
fn click_whiteboard_template() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Whiteboards");
    assert_eq!(h.app.section, Section::Whiteboards);
    h.click("Action Plan");
    let w = h
        .app
        .data
        .extra
        .whiteboards
        .last()
        .expect("a whiteboard was made");
    assert_eq!(w.name, "Action Plan");
    assert!(w.notes.len() >= 4, "the template's sticky notes are there");
}

#[test]
fn whiteboard_notes_delete_undo_and_clear_asks() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    wb.notes.push(Sticky {
        id: 0,
        pos: [40.0, 40.0],
        text: "One".into(),
        color: [0xf8, 0xae, 0x00],
    });
    wb.strokes.push(WbStroke {
        id: 0,
        color: [0, 0, 0],
        width: 2.0,
        points: vec![[1.0, 1.0], [5.0, 5.0]],
    });
    h.settle();
    h.click("Note colour 1");
    h.settle();
    let board = |h: &Ui1| {
        h.app
            .data
            .extra
            .whiteboards
            .iter()
            .find(|w| w.id == wid)
            .cloned()
            .unwrap()
    };
    h.key(egui::Key::Escape);
    h.click("Delete note 0");
    assert!(board(&h).notes.is_empty());
    h.click("Undo");
    assert_eq!(board(&h).notes.len(), 1, "undo brings the note back");
    // Clear and Delete wait in the board menu, away from the zoom buttons
    h.click("Board menu");
    h.click("Clear board");
    assert_eq!(board(&h).strokes.len(), 1, "Clear asks first");
    h.click("Clear");
    let b = board(&h);
    assert!(
        b.strokes.is_empty() && b.notes.is_empty(),
        "{:?} {:?} {:?}",
        b.strokes.len(),
        b.notes.len(),
        h.app.sweep.confirm
    );
    h.click("Board menu");
    h.click("Delete whiteboard");
    assert!(
        matches!(
            h.app.modal,
            Some(Modal::ConfirmRemove {
                kind: "whiteboard",
                ..
            })
        ),
        "Delete asks first: {:?}",
        h.app.modal.is_some()
    );
    // the confirm dialog's own Delete button (the lowest of the "Delete"s)
    let at = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Delete" && r.height() < 40.0)
        .map(|(_, r)| r.center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    h.click_at(at);
    assert!(h.app.data.extra.whiteboards.iter().all(|w| w.id != wid));
    assert!(h
        .app
        .data
        .extra
        .trash
        .iter()
        .any(|x| x.kind == "Whiteboard"));
}

#[test]
fn whiteboard_undo_puts_a_moved_note_back() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    wb.notes.push(Sticky {
        id: 0,
        pos: [60.0, 60.0],
        text: "One".into(),
        color: [0xf8, 0xae, 0x00],
    });
    h.app.wb_tool = 2;
    h.settle();
    // press near the note's bottom edge and drag it
    let x = h.rect("Delete note 0");
    let a = pos2(x.center().x - 60.0, x.center().y + 100.0);
    h.frame(vec![egui::Event::PointerMoved(a)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: a,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    for k in 1..=10 {
        h.frame(vec![egui::Event::PointerMoved(
            a + vec2(10.0 * k as f32, 8.0 * k as f32),
        )]);
    }
    h.frame(vec![egui::Event::PointerButton {
        pos: a + vec2(100.0, 80.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    h.settle();
    let pos = |h: &Ui1| {
        h.app
            .data
            .extra
            .whiteboards
            .iter()
            .find(|w| w.id == wid)
            .unwrap()
            .notes[0]
            .pos
    };
    assert_ne!(pos(&h), [60.0, 60.0], "the note moved");
    h.click("Undo");
    assert_eq!(pos(&h), [60.0, 60.0], "undo puts it back");
}

#[test]
fn whiteboard_pans_and_zooms_and_draws_in_board_coordinates() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let board = |h: &Ui1| {
        h.app
            .data
            .extra
            .whiteboards
            .iter()
            .find(|w| w.id == wid)
            .cloned()
            .unwrap()
    };
    // a note far beyond the window, like a big template's
    h.app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap()
        .notes
        .push(Sticky {
            id: 0,
            pos: [2400.0, 1600.0],
            text: "Far".into(),
            color: [0xf8, 0xae, 0x00],
        });
    h.settle();
    assert!(!h.has("Delete note 0"), "the far note is off screen");
    let canvas = h.rect("Whiteboard canvas");
    let view = |h: &Ui1| h.app.sweep.wb_view.get(&wid).copied().unwrap_or_default();
    // two-finger scroll moves the view until the note shows
    h.frame(vec![egui::Event::PointerMoved(canvas.center())]);
    for _ in 0..40 {
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(-7.0, -5.0) * 10.0,
            modifiers: Default::default(),
        }]);
        h.settle();
        if h.has("Delete note 0") {
            break;
        }
    }
    assert!(h.has("Delete note 0"), "scrolling reaches the far note");
    assert!(view(&h).pan.x < -1000.0);
    // Reset view comes back
    h.click("Reset view");
    assert_eq!(view(&h), crate::app::more::WbView::default());
    assert!(!h.has("Delete note 0"));
    // the buttons zoom around the canvas centre
    h.assert_plus_centered("Zoom in");
    h.click("Zoom in");
    assert!((view(&h).zoom - 1.25).abs() < 1e-3);
    h.click("Zoom out");
    assert!((view(&h).zoom - 1.0).abs() < 1e-3);
    // Cmd+scroll zooms at the pointer
    let at = canvas.center() + vec2(100.0, 50.0);
    h.frame(vec![egui::Event::PointerMoved(at)]);
    let before = view(&h).to_board(canvas.min + WB_ORIGIN, at);
    h.frame(vec![egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, 5.0),
        modifiers: egui::Modifiers::COMMAND,
    }]);
    h.settle();
    let v = view(&h);
    assert!(v.zoom > 1.0, "zoomed in: {}", v.zoom);
    let after = v.to_board(canvas.min + WB_ORIGIN, at);
    assert!((before[0] - after[0]).abs() < 0.5 && (before[1] - after[1]).abs() < 0.5);
    // a pen stroke is stored in board coordinates
    h.app.wb_tool = 0;
    let from = canvas.center();
    let to = from + vec2(80.0, 40.0);
    h.drag_by(from, to);
    let s = board(&h).strokes.last().cloned().expect("a stroke");
    let want = v.to_board(canvas.min + WB_ORIGIN, from);
    assert!((s.points[0][0] - want[0]).abs() < 2.0 && (s.points[0][1] - want[1]).abs() < 2.0);
    // the middle button pans and doesn't draw
    let n = board(&h).strokes.len();
    let pan = view(&h).pan;
    h.drag_with(from, from + vec2(60.0, 30.0), egui::PointerButton::Middle);
    assert_eq!(board(&h).strokes.len(), n);
    assert!((view(&h).pan - pan - vec2(60.0, 30.0)).length() < 2.0);
    // Space + drag pans too
    let pan = view(&h).pan;
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    h.drag_with(from, from - vec2(50.0, 20.0), egui::PointerButton::Primary);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Default::default(),
    }]);
    h.settle();
    assert_eq!(board(&h).strokes.len(), n, "Space + drag doesn't draw");
    assert!((view(&h).pan - pan + vec2(50.0, 20.0)).length() < 2.0);
}

#[test]
fn whiteboard_view_keeps_the_anchor_point_when_zooming() {
    use crate::app::more::WbView;
    let o = pos2(100.0, 50.0);
    let mut v = WbView::default();
    let anchor = pos2(400.0, 300.0);
    let b = v.to_board(o, anchor);
    v.zoom_at(o, anchor, 2.0);
    assert_eq!(v.zoom, 2.0);
    let b2 = v.to_board(o, anchor);
    assert!((b[0] - b2[0]).abs() < 1e-3 && (b[1] - b2[1]).abs() < 1e-3);
    v.zoom_at(o, anchor, 100.0);
    assert_eq!(v.zoom, WbView::MAX_ZOOM);
    let p = v.to_screen(o, [10.0, 20.0]);
    let back = v.to_board(o, p);
    assert!((back[0] - 10.0).abs() < 1e-3 && (back[1] - 20.0).abs() < 1e-3);
}

#[test]
fn a_teammate_change_arriving_while_i_draw_keeps_every_stroke() {
    let mut h = Ui1::new();
    let wid = shared_board(&mut h);
    let known: Vec<u64> = board_of(&h, wid).strokes.iter().map(|s| s.id).collect();
    let before = h.app.data.clone();
    // I start a stroke and keep the button down
    h.app.wb_tool = 0;
    let canvas = h.rect("Whiteboard canvas");
    let from = canvas.center();
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    for i in 1..=6 {
        h.frame(vec![egui::Event::PointerMoved(
            from + vec2(10.0 * i as f32, 4.0 * i as f32),
        )]);
    }
    assert!(
        board_of(&h, wid).strokes.iter().all(|s| s.id != 0),
        "the stroke being drawn has an id already"
    );
    // a teammate's new note merges in mid-stroke
    let note_id = 1 << 40;
    let team = teammate_change(&before, |d| {
        let w = d
            .extra
            .whiteboards
            .iter_mut()
            .find(|w| w.id == wid)
            .unwrap();
        w.notes.push(Sticky {
            id: note_id,
            pos: [600.0, 40.0],
            text: "Theirs".into(),
            color: [8, 128, 234],
        });
    });
    h.app.merge_in(&[team], &[]);
    // the stroke goes on and ends
    let last = from + vec2(120.0, 50.0);
    for i in 7..=12 {
        h.frame(vec![egui::Event::PointerMoved(
            from + vec2(10.0 * i as f32, 4.0 * i as f32),
        )]);
    }
    h.frame(vec![egui::Event::PointerButton {
        pos: last,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    h.settle();
    let b = board_of(&h, wid);
    for id in &known {
        assert!(
            b.strokes.iter().any(|s| s.id == *id),
            "known stroke {} is kept: {:?}",
            id,
            b.strokes.iter().map(|s| s.id).collect::<Vec<_>>()
        );
        assert!(
            !h.app.data.extra.sync.tombs.contains_key(id),
            "and not marked deleted"
        );
    }
    assert!(
        b.notes.iter().any(|n| n.id == note_id),
        "their note came in"
    );
    assert_eq!(b.strokes.len(), known.len() + 1);
    let mine = b.strokes.iter().find(|s| !known.contains(&s.id)).unwrap();
    assert!(mine.id != 0);
    assert!(
        mine.points.len() >= 12,
        "every point went into my stroke: {}",
        mine.points.len()
    );
    // and the next merge doesn't take them away either
    let team = teammate_change(&h.app.data, |_| {});
    h.app.merge_in(&[team], &[]);
    assert_eq!(board_of(&h, wid).strokes.len(), known.len() + 1);
}

#[test]
fn whiteboard_undo_takes_back_my_own_last_action_not_a_teammates() {
    let mut h = Ui1::new();
    let wid = shared_board(&mut h);
    let known: Vec<u64> = board_of(&h, wid).strokes.iter().map(|s| s.id).collect();
    let canvas = h.rect("Whiteboard canvas");
    // I draw a stroke
    h.app.wb_tool = 0;
    h.drag_by(canvas.center(), canvas.center() + vec2(60.0, 20.0));
    let mine = board_of(&h, wid)
        .strokes
        .iter()
        .find(|s| !known.contains(&s.id))
        .unwrap()
        .id;
    // I add a note
    h.app.wb_tool = 1;
    let at = canvas.left_top() + vec2(120.0, 420.0);
    h.click_at(at);
    let b = board_of(&h, wid);
    let my_note = b.notes.last().unwrap().id;
    assert!(my_note != 0);
    // a teammate adds a stroke and a note after mine (merged in at the end)
    let (their_stroke, their_note) = (1 << 41, (1 << 41) + 1);
    let team = teammate_change(&h.app.data, |d| {
        let w = d
            .extra
            .whiteboards
            .iter_mut()
            .find(|w| w.id == wid)
            .unwrap();
        w.strokes.push(WbStroke {
            id: their_stroke,
            color: [8, 128, 234],
            width: 3.0,
            points: vec![[500.0, 500.0], [520.0, 540.0]],
        });
        w.notes.push(Sticky {
            id: their_note,
            pos: [700.0, 40.0],
            text: "Theirs".into(),
            color: [8, 128, 234],
        });
    });
    h.app.merge_in(&[team], &[]);
    h.settle();
    let b = board_of(&h, wid);
    assert_eq!(b.strokes.last().unwrap().id, their_stroke);
    assert_eq!(b.notes.last().unwrap().id, their_note);
    // Undo takes back my note, then my stroke; theirs stay
    h.click("Undo");
    let b = board_of(&h, wid);
    assert!(
        !b.notes.iter().any(|n| n.id == my_note),
        "my note is undone"
    );
    assert!(b.notes.iter().any(|n| n.id == their_note), "theirs stays");
    h.click("Undo");
    let b = board_of(&h, wid);
    assert!(
        !b.strokes.iter().any(|s| s.id == mine),
        "my stroke is undone"
    );
    assert!(
        b.strokes.iter().any(|s| s.id == their_stroke),
        "theirs stays"
    );
    assert_eq!(b.strokes.len(), known.len() + 1);

    // a note I moved, with a teammate's note merged in before it
    let first = b.notes[0].id;
    let at0 = b.notes[0].pos;
    h.app.wb_tool = 2;
    h.settle();
    let view = h.app.sweep.wb_view.get(&wid).copied().unwrap_or_default();
    let grab = view.to_screen(canvas.min + WB_ORIGIN, [at0[0] + 80.0, at0[1] + 4.0]);
    h.drag_by(grab, grab + vec2(40.0, 30.0));
    let moved = board_of(&h, wid)
        .notes
        .iter()
        .find(|n| n.id == first)
        .unwrap()
        .pos;
    assert!(moved != at0, "the note moved");
    let front = (1 << 41) + 2;
    let mut team = teammate_change(&h.app.data, |d| {
        let w = d
            .extra
            .whiteboards
            .iter_mut()
            .find(|w| w.id == wid)
            .unwrap();
        w.notes.insert(
            0,
            Sticky {
                id: front,
                pos: [900.0, 900.0],
                text: "Front".into(),
                color: [8, 128, 234],
            },
        );
    });
    // the merge keeps their order: the new note goes first
    let _ = &mut team;
    h.app.merge_in(&[team], &[]);
    h.settle();
    h.click("Undo");
    let b = board_of(&h, wid);
    assert_eq!(
        b.notes.iter().find(|n| n.id == first).unwrap().pos,
        at0,
        "my moved note goes back"
    );
    assert_eq!(
        b.notes.iter().find(|n| n.id == front).unwrap().pos,
        [900.0, 900.0],
        "theirs doesn't move"
    );

    // a stroke of mine a teammate already erased: Undo does nothing to the board
    h.app.wb_tool = 0;
    h.drag_by(
        canvas.center() + vec2(0.0, 60.0),
        canvas.center() + vec2(60.0, 80.0),
    );
    let gone = board_of(&h, wid).strokes.last().unwrap().id;
    let team = teammate_change(&h.app.data, |d| {
        let w = d
            .extra
            .whiteboards
            .iter_mut()
            .find(|w| w.id == wid)
            .unwrap();
        w.strokes.retain(|s| s.id != gone);
        let now = chrono::Utc::now().timestamp_millis() + 5;
        d.extra.sync.tombs.insert(gone, now);
    });
    h.app.merge_in(&[team], &[]);
    h.settle();
    let before = board_of(&h, wid);
    assert!(!before.strokes.iter().any(|s| s.id == gone));
    h.click("Undo");
    let after = board_of(&h, wid);
    assert_eq!(
        after.strokes.iter().map(|s| s.id).collect::<Vec<_>>(),
        before.strokes.iter().map(|s| s.id).collect::<Vec<_>>(),
        "nothing else is taken away"
    );
}

#[test]
fn typing_in_a_note_keeps_going_to_it_after_scrolling_the_board() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    // one note at the top that scrolls away, two lower down
    for (k, (text, pos)) in [
        ("Top", [20.0, 0.0]),
        ("Mid", [20.0, 300.0]),
        ("Low", [240.0, 300.0]),
    ]
    .into_iter()
    .enumerate()
    {
        wb.notes.push(Sticky {
            id: 7000 + k as u64,
            pos,
            text: text.into(),
            color: [0xf8, 0xae, 0x00],
        });
    }
    h.settle();
    h.click("Low");
    h.key(egui::Key::End);
    h.type_text("A");
    let text = |h: &Ui1, id: u64| {
        board_of(h, wid)
            .notes
            .iter()
            .find(|n| n.id == id)
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(text(&h, 7002), "LowA");
    // scroll the board up so the top note leaves the screen
    let canvas = h.rect("Whiteboard canvas");
    h.frame(vec![egui::Event::PointerMoved(canvas.center())]);
    for _ in 0..4 {
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -60.0),
            modifiers: Default::default(),
        }]);
    }
    h.settle();
    assert!(
        h.app
            .sweep
            .wb_view
            .get(&wid)
            .is_some_and(|v| v.pan.y < -150.0),
        "the board scrolled"
    );
    assert!(!h.has("Top"), "the top note is off screen");
    h.type_text("B");
    assert_eq!(
        text(&h, 7002),
        "LowAB",
        "typing still goes to the same note"
    );
    assert_eq!(text(&h, 7001), "Mid");
    assert_eq!(text(&h, 7000), "Top");
}

/// Opens the Whiteboards page from the rail.
fn open_whiteboards(h: &mut Ui1) {
    h.click("Rail More");
    h.click("Whiteboards");
    assert_eq!(h.app.section, Section::Whiteboards);
}

/// The hub sidebar's "«" looks like closing the sidebar, so it closes it
/// (it used to go Home); the "»" strip it leaves opens it again.
#[test]
fn hub_sidebar_close_button_closes_it_and_the_strip_opens_it() {
    let mut h = Ui1::new();
    open_whiteboards(&mut h);
    assert!(h.has("All Whiteboards"));
    h.click("Close sidebar");
    assert!(h.app.sidebar_collapsed);
    assert_eq!(h.app.section, Section::Whiteboards, "still on Whiteboards");
    assert!(!h.has("All Whiteboards"), "the sidebar is closed");
    h.click("Open sidebar");
    assert!(!h.app.sidebar_collapsed);
    assert!(h.has("All Whiteboards"));
}

/// Whiteboards has no "+ Create" button by its title any more: boards are
/// made from the "New whiteboard" row above the list of boards. A board
/// just made shows it was made: its row is in the list, its name is ready
/// to type over, and a card offers ways to start until one is picked.
#[test]
fn a_new_whiteboard_shows_it_was_made_and_offers_ways_to_start() {
    let mut h = Ui1::new();
    open_whiteboards(&mut h);
    assert!(!h.has("Create in Whiteboards"));
    assert!(!h.has("New in Whiteboards"));
    h.click("New whiteboard");
    let wid = h.app.wb_open.expect("a board was made and opened");
    let made = board_of(&h, wid).name;
    assert!(h.has(&made), "its row is in the sidebar");
    assert!(h.has("Draw with the pen"), "the card says how to start");
    let name_id = egui::Id::new(("wb_name", wid));
    assert!(
        h.ctx.memory(|m| m.has_focus(name_id)),
        "the name has the focus"
    );
    // the whole name is selected, so typing replaces it
    h.type_text("Roadmap");
    assert_eq!(board_of(&h, wid).name, "Roadmap");
    // a template fills it, keeping the name typed
    h.click("Action Plan");
    let b = board_of(&h, wid);
    assert_eq!(b.notes.len(), 5);
    assert_eq!(b.name, "Roadmap", "a name already typed stays");
    assert!(!h.has("Draw with the pen"), "the card has gone");

    // an untouched name takes the template's
    h.click("New whiteboard");
    let wid2 = h.app.wb_open.unwrap();
    assert_ne!(wid, wid2);
    h.click("Flow Chart");
    let b = board_of(&h, wid2);
    assert_eq!(b.name, "Flow Chart");
    assert_eq!(b.notes.len(), 4);

    // picking a way to draw puts the card away and chooses the tool
    h.click("New whiteboard");
    h.app.wb_tool = 2;
    h.settle();
    h.click("Draw with the pen");
    assert_eq!(h.app.wb_tool, 0);
    assert!(!h.has("Draw with the pen"));
}

/// The tools are down the left; the bar along the bottom shows only what
/// the chosen tool can be set to.
#[test]
fn each_whiteboard_tool_shows_its_own_settings() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    h.app.sweep.wb_new = None;
    h.settle();
    h.click("Pen");
    assert_eq!(h.app.wb_tool, 0);
    assert!(h.has("Pen width"));
    assert!(h.has("Colour 6"), "six pen colours");
    h.click("Colour 3");
    assert_eq!(h.app.wb_color, crate::app::more::WB_COLORS[2]);
    h.click("Sticky note");
    assert_eq!(h.app.wb_tool, 1);
    assert!(!h.has("Pen width"), "no width for a note");
    assert!(h.has("Colour 5") && !h.has("Colour 6"), "no black paper");
    h.click("Eraser");
    assert_eq!(h.app.wb_tool, 3);
    assert!(!h.has("Colour 1"));
    h.click("Move notes");
    assert_eq!(h.app.wb_tool, 2);
}

/// The bars float over the board: pressing on one does not draw under it.
#[test]
fn drawing_does_not_start_on_a_floating_bar() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    h.app.sweep.wb_new = None;
    h.app.wb_tool = 0;
    h.settle();
    let wid = h.app.wb_open.unwrap();
    // from the gap between the tool bar's buttons, across the board
    let pen = h.rect("Pen");
    let from = pos2(pen.center().x, pen.bottom() + 2.0);
    h.drag_by(from, from + vec2(300.0, 40.0));
    assert!(board_of(&h, wid).strokes.is_empty());
    // on the board itself it draws
    let canvas = h.rect("Whiteboard canvas");
    h.drag_by(canvas.center(), canvas.center() + vec2(60.0, 30.0));
    assert_eq!(board_of(&h, wid).strokes.len(), 1);
}

/// The zoom in the top right bar goes back to 100% when pressed, and Reset
/// view goes back to the starting view.
#[test]
fn whiteboard_zoom_goes_back_to_100_percent() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    h.app.sweep.wb_new = None;
    h.settle();
    let wid = h.app.wb_open.unwrap();
    let view = |h: &Ui1| h.app.sweep.wb_view.get(&wid).copied().unwrap_or_default();
    // moved away from the start first
    let canvas = h.rect("Whiteboard canvas");
    h.frame(vec![egui::Event::PointerMoved(canvas.center())]);
    h.frame(vec![egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(-60.0, -40.0),
        modifiers: Default::default(),
    }]);
    h.settle();
    assert!(view(&h).pan != egui::Vec2::ZERO);
    h.click("Zoom in");
    h.click("Zoom in");
    assert!((view(&h).zoom - 1.5625).abs() < 1e-3);
    h.click("Zoom to 100%");
    assert!((view(&h).zoom - 1.0).abs() < 1e-3);
    assert!(
        view(&h).pan != egui::Vec2::ZERO,
        "100% keeps the place on the board"
    );
    h.click("Reset view");
    assert_eq!(view(&h), crate::app::more::WbView::default());
}

/// With no board open the page shows the templates and a card for each
/// board; a card opens its board and its star adds it to Favorites.
#[test]
fn whiteboard_list_shows_a_card_for_each_board() {
    let mut h = Ui1::new();
    open_whiteboards(&mut h);
    assert!(h.has("Start from scratch"));
    assert!(h.has("Root Cause Analysis"));
    h.click("Action Plan");
    let wid = h.app.wb_open.unwrap();
    h.click("All Whiteboards");
    assert_eq!(h.app.wb_open, None, "back to the list");
    let key = format!("Star wb:{}", wid);
    h.click(&key);
    assert!(h.app.data.extra.starred.contains(&format!("wb:{}", wid)));
    assert_eq!(h.app.wb_open, None, "the star does not open the board");
    // its card (drawn after the template card and the sidebar row of the
    // same name) opens it
    h.click("Action Plan");
    assert_eq!(h.app.wb_open, Some(wid));
}

/// "More templates" on a new board's card lists every template on the card,
/// and the one picked fills that board. It used to go to the list of
/// boards, where picking a template made another board and left the new
/// one empty.
#[test]
fn more_templates_fill_the_board_just_made() {
    let mut h = Ui1::new();
    open_whiteboards(&mut h);
    h.click("New whiteboard");
    let wid = h.app.wb_open.unwrap();
    let boards = h.app.data.extra.whiteboards.len();
    assert!(!h.has("Root Cause Analysis"));
    h.click("More templates");
    assert_eq!(h.app.wb_open, Some(wid), "the board stays open");
    assert!(h.has("Draw with the pen"), "the card stays up");
    assert!(!h.has("More templates"), "every template is shown");
    // all of them on the card, wrapped inside its edges
    let left = h.rect("Draw with the pen").left();
    let right = h.rect("Add sticky notes").right();
    for name in [
        "Action Plan",
        "Flow Chart",
        "Organizational Chart",
        "Customer Journey Map",
        "Root Cause Analysis",
    ] {
        let r = h.rect(name);
        assert!(
            r.left() >= left - 0.5 && r.right() <= right + 0.5,
            "{name} {r:?} inside {left}..{right}"
        );
    }
    h.click("Root Cause Analysis");
    assert_eq!(
        h.app.data.extra.whiteboards.len(),
        boards,
        "no other board was made"
    );
    assert_eq!(h.app.wb_open, Some(wid));
    let b = board_of(&h, wid);
    assert_eq!(b.name, "Root Cause Analysis");
    assert_eq!(b.notes.len(), 5);
    assert!(!h.has("Draw with the pen"), "the card has gone");
}

/// The "Clear the whole board?" question in the board menu has room inside
/// the menu's edge, the words on one line and the buttons just under them.
/// The buttons were centred in the height the menu had before, far below
/// the words, and the words sat against the edge.
#[test]
fn clear_board_question_has_even_room_around_it() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    h.app.sweep.wb_new = None;
    h.settle();
    h.click("Board menu");
    h.click("Clear board");
    h.settle();
    let title = h.painted_text("Clear the whole board?")[0].0;
    let body = h.painted_text("Every stroke and note on it is removed.")[0].0;
    let clear = h.rect("Clear");
    let cancel = h.rect("Cancel");
    let menu = h
        .ctx
        .memory(|m| {
            m.layer_ids()
                .filter(|l| l.order == egui::Order::Foreground)
                .filter_map(|l| m.area_rect(l.id))
                .find(|r| r.contains_rect(clear))
        })
        .expect("the menu is open");
    assert!(title.left() - menu.left() >= 12.0, "{title:?} in {menu:?}");
    assert!(title.top() - menu.top() >= 12.0, "{title:?} in {menu:?}");
    assert!(
        menu.right() - clear.right() >= 12.0,
        "{clear:?} in {menu:?}"
    );
    let below = menu.bottom() - clear.bottom();
    assert!((10.0..=16.0).contains(&below), "{clear:?} in {menu:?}");
    assert!(body.height() < 20.0, "the words fit on one line: {body:?}");
    let gap = clear.top() - body.bottom();
    assert!((8.0..=24.0).contains(&gap), "buttons {gap} below the words");
    assert_eq!(cancel.height(), clear.height(), "the two buttons match");
    h.click("Cancel");
    assert_eq!(h.app.sweep.confirm, None);
    assert!(h.has("Clear board"), "back to the menu");
}
