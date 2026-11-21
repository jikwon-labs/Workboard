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
            ..Default::default()
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
    assert_eq!(w.shapes.len(), 5, "the template's boxes are there");
    assert!(
        w.notes.is_empty(),
        "boxes, not sticky notes standing in for them"
    );
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
        ..Default::default()
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
        ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
                ..Default::default()
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
            ..Default::default()
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
    // the strip's "»" is a square across it, its chevrons in the middle
    let r = h.rect("Open sidebar");
    assert_eq!((r.width(), r.height()), (28.0, 28.0));
    let marks: Vec<Pos2> = h
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Path(p)
                if !p.closed && p.points.len() == 3 && p.points.iter().all(|q| r.contains(*q)) =>
            {
                Some(p.points.clone())
            }
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(marks.len(), 6, "two chevrons");
    assert!((Rect::from_points(&marks).center() - r.center()).length() < 0.5);
    h.click("Open sidebar");
    assert!(!h.app.sidebar_collapsed);
    assert!(h.has("All Whiteboards"));
}

/// Whiteboards has no "+ Create" button by its title any more: boards are
/// made from the "+" at the end of the "Boards" heading above the list, as
/// on every hub sidebar. A board just made shows it was made: its row is in
/// the list, its name is ready to type over, and a card offers ways to
/// start until one is picked.
#[test]
fn a_new_whiteboard_shows_it_was_made_and_offers_ways_to_start() {
    let mut h = Ui1::new();
    open_whiteboards(&mut h);
    assert!(!h.has("Create in Whiteboards"));
    assert!(!h.has("New in Whiteboards"));
    // a small "+" at the end of the "Boards" heading, not a whole row
    let plus = h.rect("Add a whiteboard");
    let side = crate::widgets::CONTROL_H + 0.5;
    assert!(plus.width() <= side && plus.height() <= side);
    h.click("Add a whiteboard");
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
    assert_eq!(b.shapes.len(), 5);
    assert_eq!(b.name, "Roadmap", "a name already typed stays");
    assert!(!h.has("Draw with the pen"), "the card has gone");

    // an untouched name takes the template's
    h.click("Add a whiteboard");
    let wid2 = h.app.wb_open.unwrap();
    assert_ne!(wid, wid2);
    h.click("Flow Chart");
    let b = board_of(&h, wid2);
    assert_eq!(b.name, "Flow Chart");
    assert_eq!((b.shapes.len(), b.lines.len()), (3, 2));

    // picking a way to draw puts the card away and chooses the tool
    h.click("Add a whiteboard");
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
    h.click("Select");
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
    assert_eq!((b.shapes.len(), b.lines.len()), (5, 5));
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
    let body = h.painted_text("Everything on it is removed.")[0].0;
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

/// A board open on screen at the starting view with notes "A", "B"... at
/// these places, and no "how to start" card.
fn board_with_notes(h: &mut Ui1, at: &[[f32; 2]]) -> u64 {
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    h.app.sweep.wb_new = None;
    let wid = h.app.wb_open.unwrap();
    let ids: Vec<u64> = at.iter().map(|_| h.app.data.new_id()).collect();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    for (k, (pos, id)) in at.iter().zip(ids).enumerate() {
        wb.notes.push(Sticky {
            id,
            pos: *pos,
            text: char::from(b'A' + k as u8).to_string(),
            color: [0xf8, 0xae, 0x00],
            ..Default::default()
        });
    }
    h.settle();
    wid
}

/// Where a point of the board is on screen, at the starting view.
fn on_screen(h: &Ui1, b: [f32; 2]) -> Pos2 {
    h.rect("Whiteboard canvas").min + WB_ORIGIN + vec2(b[0], b[1])
}

fn note_pos(h: &Ui1, wid: u64, id: u64) -> [f32; 2] {
    board_of(h, wid)
        .notes
        .iter()
        .find(|n| n.id == id)
        .unwrap()
        .pos
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

/// Whether the dashed lines of a note lining up with others are drawn.
fn guides_shown(h: &Ui1) -> bool {
    h.shapes.iter().any(|c| {
        matches!(&c.shape, egui::Shape::LineSegment { stroke, .. }
            if stroke.color == crate::app::more::WB_GUIDE)
    })
}

/// A note is taken by its top strip with any tool (here the pen, which
/// drew a line instead before), and while dragged it snaps into line with
/// another note near it, a dashed line showing it; moved on, it lets go
/// again. Cmd+Z puts it back.
#[test]
fn a_note_moves_by_its_top_strip_and_lines_up_with_the_others() {
    let mut h = Ui1::new();
    let wid = board_with_notes(&mut h, &[[100.0, 100.0], [400.0, 113.0]]);
    let b = board_of(&h, wid).notes[1].id;
    h.app.wb_tool = 0;
    h.settle();
    let grab = on_screen(&h, [480.0, 113.0 + 9.0]);
    h.hover(grab);
    assert_eq!(h.cursor, egui::CursorIcon::Grab, "the strip shows the hand");
    h.frame(vec![button(grab, true)]);
    // up 10: 3 below A, so it takes A's line
    for k in 1..=10 {
        h.frame(vec![egui::Event::PointerMoved(grab - vec2(0.0, k as f32))]);
    }
    assert_eq!(note_pos(&h, wid, b), [400.0, 100.0], "in line with A");
    assert!(guides_shown(&h), "a dashed line shows it");
    assert_eq!(h.cursor, egui::CursorIcon::Grabbing);
    // on up, it lets go of the line and follows the pointer again
    for k in 1..=10 {
        h.frame(vec![egui::Event::PointerMoved(
            grab - vec2(0.0, 10.0 + 3.0 * k as f32),
        )]);
    }
    assert_eq!(note_pos(&h, wid, b), [400.0, 73.0]);
    assert!(!guides_shown(&h));
    // back near the line and let go there
    let end = grab - vec2(0.0, 12.0);
    h.frame(vec![egui::Event::PointerMoved(end)]);
    h.frame(vec![button(end, false)]);
    h.settle();
    assert_eq!(note_pos(&h, wid, b), [400.0, 100.0]);
    assert!(!guides_shown(&h), "the line goes when it is let go");
    assert!(board_of(&h, wid).strokes.is_empty(), "the pen drew nothing");
    assert!(h.app.sweep.wb_note_drag.is_none());
    cmd_key(&mut h, egui::Key::Z);
    assert_eq!(note_pos(&h, wid, b), [400.0, 113.0], "Cmd+Z put it back");
}

/// With the pen the pointer is a pencil, its tip where the line starts and
/// in the pen's colour; on a note's text it is the I-beam, on the tool bar
/// the hand, and the other tools keep the arrow.
#[test]
fn the_pen_shows_a_pencil_for_the_pointer() {
    let mut h = Ui1::new();
    board_with_notes(&mut h, &[[100.0, 100.0]]);
    h.app.wb_tool = 0;
    h.app.wb_color = crate::app::more::WB_COLORS[2];
    h.settle();
    let at = h.rect("Whiteboard canvas").center() + vec2(100.0, 100.0);
    h.hover(at);
    assert_eq!(h.cursor, egui::CursorIcon::None, "the arrow is hidden");
    let ink = rgb(crate::app::more::WB_COLORS[2]);
    let tip = h.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Path(p) => p.fill == ink && (p.points[0] - at).length() < 0.5,
        _ => false,
    });
    assert!(tip, "a pencil tip in the pen colour at the pointer");
    h.hover(on_screen(&h, [140.0, 150.0]));
    assert_eq!(h.cursor, egui::CursorIcon::Text, "on a note's text");
    h.hover(h.rect("Eraser").center());
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand, "on the tool bar");
    h.app.wb_tool = 1;
    h.hover(at);
    assert_eq!(
        h.cursor,
        egui::CursorIcon::Default,
        "other tools: the arrow"
    );
}

/// A click anywhere on a note is for typing in it, whatever the tool: the
/// sticky note tool made another note on top of it, and the move tool
/// could not type at all. With the move tool a drag still moves it, and
/// once typing, its text can be dragged over to select while the strip
/// still moves it.
#[test]
fn a_click_on_a_note_types_in_it_with_any_tool() {
    let mut h = Ui1::new();
    let wid = board_with_notes(&mut h, &[[100.0, 100.0]]);
    let nid = board_of(&h, wid).notes[0].id;
    let text = egui::Id::new(("wb_note_text", wid, nid));
    let typing = |h: &Ui1| h.ctx.memory(|m| m.has_focus(text));
    let note = |h: &Ui1| board_of(h, wid).notes;
    h.app.wb_tool = 1;
    h.settle();
    // low on the note, below its text
    h.click_at(on_screen(&h, [180.0, 212.0]));
    assert_eq!(note(&h).len(), 1, "no new note on top of it");
    assert!(typing(&h), "it is ready to type in");
    h.type_text("!");
    assert_eq!(note(&h)[0].text, "A!", "typed after its text");
    // on its strip
    h.click_at(on_screen(&h, [180.0, 109.0]));
    assert_eq!(note(&h).len(), 1);

    h.app.wb_tool = 2;
    h.settle();
    h.click_at(h.rect("Whiteboard canvas").center());
    assert!(!typing(&h));
    h.click_at(on_screen(&h, [140.0, 150.0]));
    assert!(typing(&h), "the move tool types too");
    h.type_text("?");
    assert_eq!(note(&h)[0].text, "A!?");
    h.drag_by(on_screen(&h, [112.0, 126.0]), on_screen(&h, [200.0, 126.0]));
    assert_eq!(
        note(&h)[0].pos,
        [100.0, 100.0],
        "dragging over its text selects"
    );
    h.drag_by(on_screen(&h, [180.0, 109.0]), on_screen(&h, [480.0, 309.0]));
    assert_eq!(note(&h)[0].pos, [400.0, 300.0], "its strip still moves it");
    // not typing, the move tool takes it anywhere
    h.click_at(on_screen(&h, [50.0, 500.0]));
    assert!(!typing(&h));
    h.drag_by(on_screen(&h, [480.0, 380.0]), on_screen(&h, [380.0, 380.0]));
    assert_eq!(note(&h)[0].pos, [300.0, 300.0]);
}

/// Cmd+Z takes back my last change on the board, as the Undo button does
/// (its tip says so); typing in a note it is the note's own undo, and
/// Cmd+Shift+Z is not an undo.
#[test]
fn cmd_z_takes_back_on_the_board_but_not_while_typing() {
    let mut h = Ui1::new();
    let wid = board_with_notes(&mut h, &[[100.0, 100.0]]);
    h.app.wb_tool = 0;
    h.settle();
    let c = h.rect("Whiteboard canvas").center();
    h.drag_by(c, c + vec2(80.0, 30.0));
    assert_eq!(board_of(&h, wid).strokes.len(), 1);
    cmd_key(&mut h, egui::Key::Z);
    assert!(board_of(&h, wid).strokes.is_empty(), "Cmd+Z took it back");
    h.drag_by(c, c + vec2(80.0, 30.0));
    let shift_cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        shift: true,
        ..Default::default()
    };
    h.key_with(egui::Key::Z, shift_cmd);
    assert_eq!(board_of(&h, wid).strokes.len(), 1, "Cmd+Shift+Z is no undo");
    h.click_at(on_screen(&h, [140.0, 150.0]));
    h.type_text("xyz");
    cmd_key(&mut h, egui::Key::Z);
    assert_eq!(board_of(&h, wid).strokes.len(), 1, "the stroke stays");
    assert_eq!(board_of(&h, wid).notes.len(), 1);
    h.hover(h.rect("Undo").center());
    for _ in 0..12 {
        h.frame(vec![]);
    }
    assert!(
        drawn_texts(&h.shapes)
            .iter()
            .any(|t| t.contains("\u{2318}Z")),
        "the tip shows the keys"
    );
}

// ------------------------------------------------------------ shapes, lines

/// A board open on screen at the starting view, filled from a template,
/// with the Select tool and no "how to start" card.
fn template_board(h: &mut Ui1, name: &str) -> u64 {
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    h.app.wb_fill_template(wid, name);
    h.app.sweep.wb_new = None;
    h.app.wb_tool = 2;
    h.settle();
    wid
}

fn shape<'a>(b: &'a Whiteboard, text: &str) -> &'a WbShape {
    b.shapes
        .iter()
        .find(|s| s.text == text)
        .unwrap_or_else(|| panic!("no shape {:?}", text))
}

fn overlap(a: &WbShape, b: &WbShape) -> bool {
    let r = |s: &WbShape| Rect::from_min_size(pos2(s.pos[0], s.pos[1]), vec2(s.size[0], s.size[1]));
    r(a).intersects(r(b))
}

/// Each template lays out the picture on its card: boxes joined by lines
/// that bend at right angles (org chart), circles joined by dashes
/// (customer journey), a start, a decision and a step joined by arrows
/// (flow chart), a fishbone with the problem at its head. None of it
/// overlaps, and it all shows at the starting view.
#[test]
fn templates_lay_out_their_picture_as_shapes_and_lines() {
    let mut h = Ui1::new();
    let canvas = |h: &Ui1| h.rect("Whiteboard canvas");

    let wid = template_board(&mut h, "Organizational Chart");
    let b = board_of(&h, wid);
    let ceo = shape(&b, "CEO");
    assert!(ceo.filled, "the top box is tinted like on the card");
    assert_eq!(b.lines.len(), 3);
    for (l, name) in b.lines.iter().zip(["CTO", "COO", "CMO"]) {
        let to = shape(&b, name);
        assert_eq!((l.from, l.to), (ceo.id, to.id));
        assert!(l.elbow && !l.arrow && !l.dashed);
        // down from the CEO's bottom, across, down into the box's top
        let path = crate::app::more::line_path(l, &b.shapes);
        let (first, last) = (path[0], path[path.len() - 1]);
        assert_eq!(first.y, ceo.pos[1] + ceo.size[1], "{name}: {path:?}");
        assert_eq!(first.x, ceo.pos[0] + ceo.size[0] / 2.0, "{name}: {path:?}");
        assert_eq!(last.y, to.pos[1], "{name}: {path:?}");
        assert_eq!(last.x, to.pos[0] + to.size[0] / 2.0, "{name}: {path:?}");
    }
    for name in ["CEO", "CTO", "COO", "CMO"] {
        assert!(h.has(&format!("Shape {}", name)), "{name} is drawn");
    }

    let wid = template_board(&mut h, "Customer Journey Map");
    let b = board_of(&h, wid);
    assert_eq!(b.shapes.len(), 5);
    assert!(b.shapes.iter().all(|s| s.kind == "ellipse"));
    assert_eq!(b.lines.len(), 4);
    assert!(b.lines.iter().all(|l| l.dashed && !l.arrow));

    let wid = template_board(&mut h, "Flow Chart");
    let b = board_of(&h, wid);
    let kinds: Vec<&str> = b.shapes.iter().map(|s| s.kind.as_str()).collect();
    assert_eq!(kinds, ["pill", "diamond", "rect"]);
    assert!(b.lines.iter().all(|l| l.arrow && !l.dashed));

    let wid = template_board(&mut h, "Root Cause Analysis");
    let b = board_of(&h, wid);
    let problem = shape(&b, "Problem");
    let spine = &b.lines[0];
    assert_eq!(
        (spine.from, spine.to),
        (0, problem.id),
        "the spine runs to the head"
    );
    let labels = b.shapes.iter().filter(|s| s.kind == "text").count();
    assert_eq!(labels, 4, "a label at the end of each bone");

    for name in [
        "Organizational Chart",
        "Action Plan",
        "Customer Journey Map",
        "Flow Chart",
        "Root Cause Analysis",
    ] {
        let wid = template_board(&mut h, name);
        let b = board_of(&h, wid);
        assert!(b.notes.is_empty());
        for (i, a) in b.shapes.iter().enumerate() {
            for c in &b.shapes[i + 1..] {
                assert!(
                    !overlap(a, c),
                    "{name}: {:?} and {:?} overlap",
                    a.text,
                    c.text
                );
            }
            let far = on_screen(&h, [a.pos[0] + a.size[0], a.pos[1] + a.size[1]]);
            assert!(canvas(&h).contains(far), "{name}: {:?} off screen", a.text);
        }
    }
}

/// With the Select tool a click picks a shape; the + under the CEO adds a
/// box in the row with the others, past the last one, joined by a line
/// like theirs, ready to type in. Undo takes it away with its line.
#[test]
fn the_plus_on_a_shape_adds_one_joined_to_it() {
    let mut h = Ui1::new();
    open_whiteboards(&mut h);
    h.click("Organizational Chart");
    let wid = h.app.wb_open.unwrap();
    h.click("Select");
    h.click("Shape CEO");
    let ceo = shape(&board_of(&h, wid), "CEO").id;
    assert_eq!(
        h.app.sweep.wb_sel,
        Some((wid, crate::app::more::WbSel::Shape(ceo)))
    );
    assert!(h.has("Add shape below") && h.has("Resize shape 3"));
    h.click("Add shape below");
    let b = board_of(&h, wid);
    assert_eq!(b.shapes.len(), 5);
    let new = b.shapes.last().unwrap().clone();
    let line = b.lines.last().unwrap();
    assert_eq!((line.from, line.to), (ceo, new.id));
    assert!(line.elbow, "joined like the other boxes");
    let cmo = shape(&b, "CMO");
    assert_eq!(new.pos[1], cmo.pos[1], "in the row");
    assert!(new.pos[0] > cmo.pos[0], "after the last one");
    assert!(b.shapes[..4].iter().all(|s| !overlap(s, &new)));
    // it is picked and takes the typing
    assert_eq!(
        h.app.sweep.wb_sel,
        Some((wid, crate::app::more::WbSel::Shape(new.id)))
    );
    h.type_text("CFO");
    assert_eq!(board_of(&h, wid).shapes.last().unwrap().text, "CFO");
    assert!(h.has("Shape text"));
    h.key(egui::Key::Escape);
    h.settle();
    assert!(!h.has("Shape text"), "Escape ends the typing");
    assert!(h.has("Shape CFO"));
    h.click("Undo");
    let b = board_of(&h, wid);
    assert_eq!(
        (b.shapes.len(), b.lines.len()),
        (4, 3),
        "Undo takes both away"
    );
}

/// A picked shape moves with a drag (its line follows), resizes by a
/// corner, takes a colour and a fill from the bar above it, and Delete
/// takes it away with its lines. Undo takes each step back.
#[test]
fn a_shape_moves_resizes_recolours_and_deletes() {
    let mut h = Ui1::new();
    let wid = template_board(&mut h, "Flow Chart");
    let step = shape(&board_of(&h, wid), "Step").clone();
    let path = |h: &Ui1| {
        let b = board_of(h, wid);
        crate::app::more::line_path(&b.lines[1], &b.shapes)
    };
    let end = *path(&h).last().unwrap();
    let at = h.rect("Shape Step").center();
    h.drag_by(at, at + vec2(100.0, 50.0));
    let moved = shape(&board_of(&h, wid), "Step").pos;
    assert_eq!(moved, [step.pos[0] + 100.0, step.pos[1] + 50.0]);
    assert_ne!(*path(&h).last().unwrap(), end, "its line follows it");
    h.click("Undo");
    assert_eq!(shape(&board_of(&h, wid), "Step").pos, step.pos);

    h.click("Shape Step");
    let c = h.rect("Resize shape 3").center();
    h.drag_by(c, c + vec2(40.0, 20.0));
    let s = shape(&board_of(&h, wid), "Step").clone();
    assert_eq!(s.pos, step.pos, "the opposite corner stays");
    assert!((s.size[0] - (step.size[0] + 40.0)).abs() < 0.5);
    assert!((s.size[1] - (step.size[1] + 20.0)).abs() < 0.5);
    h.click("Pick colour 3");
    assert_eq!(
        shape(&board_of(&h, wid), "Step").color,
        crate::app::more::WB_COLORS[2]
    );
    h.click("Fill shape");
    assert!(shape(&board_of(&h, wid), "Step").filled);
    h.click("Undo");
    assert!(!shape(&board_of(&h, wid), "Step").filled);

    h.key(egui::Key::Delete);
    let b = board_of(&h, wid);
    assert_eq!((b.shapes.len(), b.lines.len()), (2, 1), "with its line");
    assert_eq!(h.app.sweep.wb_sel, None);
    h.click("Undo");
    let b = board_of(&h, wid);
    assert_eq!((b.shapes.len(), b.lines.len()), (3, 2), "both back");
    assert_eq!(b.lines[1].to, step.id);
}

/// A click on a line picks it; its bar bends it at right angles, dashes it
/// and deletes it.
#[test]
fn a_line_is_picked_restyled_and_deleted() {
    let mut h = Ui1::new();
    let wid = template_board(&mut h, "Flow Chart");
    // between the start and the decision
    let mid = on_screen(&h, [220.0, 152.0]);
    h.click_at(mid);
    let first = board_of(&h, wid).lines[0].id;
    assert_eq!(
        h.app.sweep.wb_sel,
        Some((wid, crate::app::more::WbSel::Line(first)))
    );
    h.click("Dashed line");
    h.click("Elbow line");
    let l = board_of(&h, wid).lines[0].clone();
    assert!(l.dashed && l.elbow && l.arrow);
    h.click("Delete line");
    assert_eq!(board_of(&h, wid).lines.len(), 1);
    // a click on the empty board lets the pick go
    h.click_at(mid);
    assert_eq!(h.app.sweep.wb_sel, None);
}

/// The Shape tool puts the chosen shape where the board is clicked, ready
/// to type in; the Line tool joins two shapes with a drag, in the style set
/// on its bar.
#[test]
fn shape_tool_puts_shapes_down_and_the_line_tool_joins_them() {
    let mut h = Ui1::new();
    let wid = board_with_notes(&mut h, &[]);
    // the pointer is on the window before the first click, as it is for
    // anyone clicking
    h.hover(h.rect("Whiteboard canvas").center());
    h.click("Shape");
    assert_eq!(h.app.wb_tool, 4);
    assert!(h.has("Circle") && h.has("Colour 6"));
    h.click("Diamond");
    h.click_at(on_screen(&h, [100.0, 100.0]));
    let b = board_of(&h, wid);
    assert_eq!(b.shapes.len(), 1);
    let d = b.shapes[0].clone();
    assert_eq!(d.kind, "diamond");
    assert_eq!(
        [d.pos[0] + d.size[0] / 2.0, d.pos[1] + d.size[1] / 2.0],
        [100.0, 100.0],
        "centred where clicked"
    );
    assert_eq!(h.app.wb_tool, 2, "back to Select");
    h.type_text("Ship?");
    assert_eq!(board_of(&h, wid).shapes[0].text, "Ship?");
    h.key(egui::Key::Escape);

    h.click("Shape");
    h.click("Rectangle");
    h.click_at(on_screen(&h, [400.0, 100.0]));
    h.key(egui::Key::Escape);
    assert_eq!(board_of(&h, wid).shapes[1].kind, "rect");

    h.click("Line");
    h.click("Dashed line");
    h.drag_by(on_screen(&h, [100.0, 100.0]), on_screen(&h, [400.0, 100.0]));
    let b = board_of(&h, wid);
    assert_eq!(b.lines.len(), 1);
    let l = &b.lines[0];
    assert_eq!((l.from, l.to), (b.shapes[0].id, b.shapes[1].id));
    assert!(l.arrow && l.dashed);
    // a short drag on the empty board draws nothing
    h.drag_by(on_screen(&h, [600.0, 300.0]), on_screen(&h, [604.0, 302.0]));
    assert_eq!(board_of(&h, wid).lines.len(), 1);
}

/// A sticky note's bottom right corner resizes it, with any tool; Undo
/// puts its size back.
#[test]
fn a_note_is_resized_by_its_corner() {
    let mut h = Ui1::new();
    let wid = board_with_notes(&mut h, &[[100.0, 100.0]]);
    h.app.wb_tool = 0;
    h.settle();
    let c = h.rect("Resize note 0").center();
    h.drag_by(c, c + vec2(80.0, 40.0));
    let b = board_of(&h, wid);
    assert!(b.strokes.is_empty(), "the pen did not draw");
    let wh = b.notes[0].wh();
    assert!(
        (wh[0] - 240.0).abs() < 0.5 && (wh[1] - 160.0).abs() < 0.5,
        "{wh:?}"
    );
    // its text box is as wide as the note
    let r = h.rect("Resize note 0");
    assert!((r.right() - on_screen(&h, [340.0, 0.0]).x).abs() < 1.0);
    h.click("Undo");
    assert_eq!(board_of(&h, wid).notes[0].wh(), Sticky::SIZE);
}

// ------------------------------------------------------------ QA round 1

fn view_of(h: &Ui1, wid: u64) -> crate::app::more::WbView {
    h.app.sweep.wb_view.get(&wid).copied().unwrap_or_default()
}

fn space_key(h: &mut Ui1, pressed: bool) {
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    }]);
}

fn push_note(h: &mut Ui1, wid: u64, pos: [f32; 2], text: &str) -> u64 {
    let id = h.app.data.new_id();
    h.app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap()
        .notes
        .push(Sticky {
            id,
            pos,
            text: text.into(),
            color: [0xf8, 0xae, 0x00],
            ..Default::default()
        });
    id
}

/// While the board menu or a dialog is open, Delete and Backspace are
/// theirs: the shape picked on the board behind them stays. It was deleted
/// out of sight, with its lines.
#[test]
fn delete_keys_leave_the_board_alone_while_a_menu_or_dialog_is_open() {
    let mut h = Ui1::new();
    let wid = template_board(&mut h, "Flow Chart");
    h.hover(h.rect("Whiteboard canvas").center());
    h.click("Shape Step");
    h.click("Board menu");
    assert!(h.has("Clear board"), "the menu is open");
    h.key(egui::Key::Backspace);
    assert_eq!(board_of(&h, wid).shapes.len(), 3, "kept with the menu open");
    h.click("Delete whiteboard");
    assert!(h.app.modal.is_some(), "the dialog is open");
    h.key(egui::Key::Backspace);
    h.key(egui::Key::Delete);
    let b = board_of(&h, wid);
    assert_eq!(
        (b.shapes.len(), b.lines.len()),
        (3, 2),
        "kept with the dialog open"
    );
    h.click("Cancel");
    assert!(h.app.modal.is_none());
    h.key(egui::Key::Delete);
    assert_eq!(board_of(&h, wid).shapes.len(), 2, "then Delete takes it");
}

/// A board a teammate deleted while it was open: the page goes back to the
/// list of boards. It stayed on an empty page with only a hint.
#[test]
fn a_board_deleted_by_a_teammate_while_open_goes_back_to_the_list() {
    let mut h = Ui1::new();
    let wid = shared_board(&mut h);
    let team = teammate_change(&h.app.data, |d| d.extra.whiteboards.retain(|w| w.id != wid));
    h.app.merge_in(&[team], &[]);
    h.settle();
    assert!(h.app.data.extra.whiteboards.iter().all(|w| w.id != wid));
    assert_eq!(h.app.wb_open, None);
    assert!(h.has("Start from scratch"), "the list of boards shows");
}

/// The middle button and Space + drag move the view wherever the drag
/// starts on the board: on a shape (which moved instead), on a note's text
/// (nothing happened), on a picked shape's corner (which resized). Only the
/// left button moves a shape.
#[test]
fn panning_works_over_shapes_notes_and_handles() {
    let mut h = Ui1::new();
    let wid = template_board(&mut h, "Flow Chart");
    let nid = push_note(&mut h, wid, [100.0, 320.0], "Idea");
    h.settle();
    let shapes = board_of(&h, wid).shapes;
    let note = |h: &Ui1| {
        board_of(h, wid)
            .notes
            .into_iter()
            .find(|n| n.id == nid)
            .unwrap()
    };
    let moved_by = |h: &Ui1, from: egui::Vec2, d: egui::Vec2| {
        let got = view_of(h, wid).pan - from;
        assert!((got - d).length() < 1.0, "panned {got:?}, not {d:?}");
    };
    // the middle button on a shape
    let pan = view_of(&h, wid).pan;
    let at = h.rect("Shape Step").center();
    h.drag_with(at, at + vec2(100.0, 50.0), egui::PointerButton::Middle);
    assert_eq!(board_of(&h, wid).shapes, shapes, "the shape stays");
    moved_by(&h, pan, vec2(100.0, 50.0));
    // the middle button on a note's text
    let pan = view_of(&h, wid).pan;
    let x = h.rect("Delete note 0");
    let text = pos2(x.center().x - 100.0, x.center().y + 40.0);
    h.drag_with(text, text + vec2(-60.0, 40.0), egui::PointerButton::Middle);
    moved_by(&h, pan, vec2(-60.0, 40.0));
    assert_eq!(note(&h).pos, [100.0, 320.0]);
    let typing = h.ctx.memory(|m| m.focused());
    assert_eq!(typing, None, "the middle button doesn't start typing");
    // Space + drag on a note's text
    let pan = view_of(&h, wid).pan;
    let x = h.rect("Delete note 0");
    let text = pos2(x.center().x - 100.0, x.center().y + 40.0);
    h.hover(text);
    space_key(&mut h, true);
    h.drag_by(text, text + vec2(50.0, -30.0));
    space_key(&mut h, false);
    h.settle();
    moved_by(&h, pan, vec2(50.0, -30.0));
    assert_eq!(note(&h).text, "Idea");
    // Space + drag on a picked shape's corner
    h.click("Shape Step");
    let pan = view_of(&h, wid).pan;
    let corner = h.rect("Resize shape 3").center();
    h.hover(corner);
    space_key(&mut h, true);
    h.drag_by(corner, corner + vec2(60.0, 30.0));
    space_key(&mut h, false);
    h.settle();
    assert_eq!(board_of(&h, wid).shapes, shapes, "not resized");
    moved_by(&h, pan, vec2(60.0, 30.0));
    // the right button doesn't move a shape either; the left one does
    let at = h.rect("Shape Step").center();
    h.drag_with(at, at + vec2(40.0, 0.0), egui::PointerButton::Secondary);
    assert_eq!(board_of(&h, wid).shapes, shapes);
    h.drag_by(at, at + vec2(40.0, 0.0));
    assert_ne!(board_of(&h, wid).shapes, shapes, "the left button moves it");
}

/// The eraser takes a stroke wherever it touches it: between two points
/// far apart (a line drawn fast) and on a fast swipe across it, not only
/// near one of its points.
#[test]
fn the_eraser_takes_a_stroke_between_its_points_and_on_a_fast_swipe() {
    let mut h = Ui1::new();
    let wid = board_with_notes(&mut h, &[]);
    let ids: Vec<u64> = (0..2).map(|_| h.app.data.new_id()).collect();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    wb.strokes.push(WbStroke {
        id: ids[0],
        color: [0, 0, 0],
        width: 2.5,
        points: vec![[300.0, 300.0], [600.0, 300.0]],
    });
    wb.strokes.push(WbStroke {
        id: ids[1],
        color: [0, 0, 0],
        width: 2.5,
        points: (0..100).map(|k| [300.0 + k as f32 * 3.0, 500.0]).collect(),
    });
    h.app.wb_tool = 3;
    h.settle();
    h.hover(on_screen(&h, [450.0, 250.0]));
    h.click_at(on_screen(&h, [450.0, 300.0]));
    let left: Vec<u64> = board_of(&h, wid).strokes.iter().map(|s| s.id).collect();
    assert_eq!(
        left,
        vec![ids[1]],
        "the click between its two points took it"
    );
    // 35 apart a step: no step lands within reach of the line at 500
    h.drag_by(on_screen(&h, [450.0, 130.0]), on_screen(&h, [450.0, 550.0]));
    assert!(
        board_of(&h, wid).strokes.is_empty(),
        "the swipe across took it"
    );
}

/// A note's corner stays under the pointer while it is resized, also after
/// going past the smallest size and coming back.
#[test]
fn a_resized_note_keeps_its_corner_under_the_pointer() {
    let mut h = Ui1::new();
    let wid = board_with_notes(&mut h, &[[100.0, 100.0]]);
    h.app.wb_tool = 2;
    h.settle();
    let c = h.rect("Resize note 0").center();
    h.hover(c);
    h.frame(vec![button(c, true)]);
    for k in 1..=10 {
        h.frame(vec![egui::Event::PointerMoved(
            c - vec2(20.0 * k as f32, 0.0),
        )]);
    }
    assert_eq!(board_of(&h, wid).notes[0].wh()[0], 100.0, "the smallest");
    for k in 1..=12 {
        h.frame(vec![egui::Event::PointerMoved(
            c - vec2(200.0, 0.0) + vec2(20.0 * k as f32, 0.0),
        )]);
    }
    let end = c + vec2(40.0, 0.0);
    h.frame(vec![button(end, false)]);
    h.settle();
    let wh = board_of(&h, wid).notes[0].wh();
    assert!(
        (wh[0] - 200.0).abs() < 0.5,
        "40 wider, as the pointer: {wh:?}"
    );
    assert!((wh[1] - 120.0).abs() < 0.5, "{wh:?}");
}

/// Escape while typing in a shape ends the typing and keeps the shape
/// picked; a second Escape lets it go.
#[test]
fn escape_ends_typing_in_a_shape_before_letting_it_go() {
    let mut h = Ui1::new();
    let wid = template_board(&mut h, "Flow Chart");
    h.hover(h.rect("Whiteboard canvas").center());
    h.click("Shape Step");
    let id = shape(&board_of(&h, wid), "Step").id;
    h.key(egui::Key::Enter);
    h.settle();
    assert!(h.has("Shape text"), "typing in it");
    h.key(egui::Key::Escape);
    assert!(!h.has("Shape text"), "the typing ended");
    assert_eq!(
        h.app.sweep.wb_sel,
        Some((wid, crate::app::more::WbSel::Shape(id))),
        "still picked"
    );
    h.key(egui::Key::Escape);
    assert_eq!(h.app.sweep.wb_sel, None);
}

/// Undo takes back a template laid out on a board, as it does everything
/// else added to it.
#[test]
fn undo_takes_back_a_template() {
    let mut h = Ui1::new();
    open_whiteboards(&mut h);
    h.click("New whiteboard");
    let wid = h.app.wb_open.unwrap();
    h.click("Flow Chart");
    let b = board_of(&h, wid);
    assert_eq!((b.shapes.len(), b.lines.len()), (3, 2));
    h.click("Undo");
    let b = board_of(&h, wid);
    assert!(
        b.shapes.is_empty() && b.lines.is_empty(),
        "the template is gone"
    );
}

/// Undo puts back a note's colour, as it does a shape's.
#[test]
fn undo_puts_a_notes_colour_back() {
    let mut h = Ui1::new();
    let wid = board_with_notes(&mut h, &[[100.0, 100.0]]);
    h.hover(h.rect("Whiteboard canvas").center());
    h.click("Note colour 1");
    // the popup's first colour, red
    let red = rgb(crate::app::more::WB_COLORS[1]);
    let at = h
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Circle(ci) if ci.fill == red && ci.radius == 8.0 => Some(ci.center),
            _ => None,
        })
        .expect("the colours are shown");
    h.click_at(at);
    assert_eq!(
        board_of(&h, wid).notes[0].color,
        crate::app::more::WB_COLORS[1]
    );
    h.click("Undo");
    assert_eq!(board_of(&h, wid).notes[0].color, [0xf8, 0xae, 0x00]);
}

/// A Line tool drag that starts and ends on the same shape draws nothing:
/// it made a short line stuck inside the shape.
#[test]
fn a_line_dragged_within_one_shape_is_not_drawn() {
    let mut h = Ui1::new();
    let wid = template_board(&mut h, "Flow Chart");
    h.app.wb_tool = 5;
    h.settle();
    let s = shape(&board_of(&h, wid), "Step").clone();
    let from = on_screen(&h, [s.pos[0] + 20.0, s.pos[1] + 15.0]);
    let to = on_screen(
        &h,
        [s.pos[0] + s.size[0] - 20.0, s.pos[1] + s.size[1] - 15.0],
    );
    // while it is held there no line shows either, only the shape's outline
    let lines = |h: &Ui1| {
        h.shapes
            .iter()
            .filter(
                |c| matches!(&c.shape, egui::Shape::Path(p) if !p.closed && p.points.len() == 2),
            )
            .count()
    };
    h.hover(from);
    let before = lines(&h);
    h.frame(vec![button(from, true)]);
    for k in 1..=12 {
        h.frame(vec![egui::Event::PointerMoved(
            from + (to - from) * (k as f32 / 12.0),
        )]);
    }
    assert!(h.app.sweep.wb_link.is_some(), "a line is being dragged out");
    assert_eq!(lines(&h), before, "no line is shown");
    h.frame(vec![button(to, false)]);
    h.settle();
    assert_eq!(board_of(&h, wid).lines.len(), 2);
    assert!(!h.app.sweep.wb_undo.iter().any(|u| matches!(
        u,
        (w, crate::app::more::WbUndo::Added(s, l)) if *w == wid && s.is_empty() && !l.is_empty()
    )));
}

/// A board's card says how many notes and shapes it has: a board made from
/// a template, all shapes, said "Empty". Its row in the sidebar takes the
/// colour of its shapes when it has no notes.
#[test]
fn a_board_card_counts_its_notes_and_shapes() {
    let mut h = Ui1::new();
    open_whiteboards(&mut h);
    h.click("Flow Chart");
    let wid = h.app.wb_open.unwrap();
    // the sidebar row's swatch, in the template's colour
    let colour = board_of(&h, wid).shapes[0].color;
    let row = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Flow Chart" && r.left() < 300.0)
        .map(|x| x.1)
        .next()
        .expect("the sidebar row");
    let fill = crate::app::more::note_fill(&h.app.t(), colour);
    let swatch = h.shapes.iter().any(|c| match &c.shape {
        egui::Shape::Rect(r) => r.fill == fill && row.contains_rect(r.rect),
        _ => false,
    });
    assert!(swatch, "the row's swatch has the shapes' colour");
    push_note(&mut h, wid, [100.0, 400.0], "Mine");
    h.click("All Whiteboards");
    let texts = drawn_texts(&h.shapes);
    assert!(!texts.iter().any(|t| t == "Empty"), "{texts:?}");
    assert!(
        texts.iter().any(|t| t == "1 note \u{b7} 3 shapes"),
        "{texts:?}"
    );
}
