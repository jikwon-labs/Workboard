// Whiteboards: the canvas with its pen, sticky notes, pan and zoom, and
// Undo. The bars floating over it are in wb_panels.rs, the list of boards
// and the templates in wb_library.rs, shapes and lines in wb_shapes.rs.

use super::*;

/// A whiteboard just made on this screen: its row in the sidebar lights up,
/// its name field takes the typing focus (the name selected, so typing
/// replaces it), and while it is empty a card in its middle says it was made
/// and offers ways to start. Before, a new board was an empty grey box and
/// people were not sure their click had done anything.
#[derive(Clone, Debug)]
pub struct WbNew {
    pub id: u64,
    /// Input time of the first frame it was on screen (the light fades from there).
    pub since: Option<f64>,
    /// The name field still has to take the focus.
    pub focus: bool,
    /// The "how to start" card is still up.
    pub start: bool,
    /// The name it was made with: a template renames it only if untouched.
    pub name: String,
    /// "More templates" was pressed: the card lists every template, and the
    /// one picked fills this board (it used to go to the list of boards,
    /// where a template made another board and left this one empty).
    pub all_templates: bool,
}

/// Where the board's 0,0 sits in the canvas at the starting view: clear of
/// the tool bar down the left and the name bar at the top, which float over
/// the board.
pub const WB_ORIGIN: egui::Vec2 = egui::Vec2::new(88.0, 84.0);

/// A sticky note's paper: its colour as a tint of the page, so the dark
/// text on it reads (a full blue or green behind black text did not).
pub(crate) fn note_fill(t: &Theme, c: [u8; 3]) -> Color32 {
    let k = if t.dark { 0.42 } else { 0.32 };
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round() as u8;
    Color32::from_rgb(
        mix(t.bg.r(), c[0]),
        mix(t.bg.g(), c[1]),
        mix(t.bg.b(), c[2]),
    )
}

/// What the whiteboard's Undo takes back, newest last. Only my own actions
/// are kept here, by the id of the stroke or note, so a teammate's stroke
/// or note merged in meanwhile is never the one taken back.
pub enum WbUndo {
    /// I drew this stroke.
    Stroke(u64),
    /// I added this note.
    Note(u64),
    /// I deleted this note; it sat at this place in the list.
    NoteGone(usize, Sticky),
    /// I erased this stroke; it sat at this place in the list.
    StrokeGone(usize, WbStroke),
    /// I dragged this note away from this spot.
    NoteMoved(u64, [f32; 2]),
    /// I resized this note from this size.
    NoteResized(u64, [f32; 2]),
    /// I added these shapes and lines (by id).
    Added(Vec<u64>, Vec<u64>),
    /// I deleted these shapes and lines; each sat at this place in its
    /// list when it was taken out.
    Removed(Vec<(usize, WbShape)>, Vec<(usize, WbLine)>),
    /// I moved, resized or restyled this shape; this is how it was.
    ShapeWas(WbShape),
    /// I moved an end of or restyled this line; this is how it was.
    LineWas(WbLine),
}

/// How a whiteboard is shown: board coordinates (where strokes and notes
/// are stored) map to the screen as `origin + pan + board * zoom`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WbView {
    pub pan: egui::Vec2,
    pub zoom: f32,
}

impl Default for WbView {
    fn default() -> Self {
        WbView {
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl WbView {
    pub const MIN_ZOOM: f32 = 0.2;
    pub const MAX_ZOOM: f32 = 4.0;

    pub fn to_screen(&self, origin: egui::Pos2, b: [f32; 2]) -> egui::Pos2 {
        origin + self.pan + vec2(b[0], b[1]) * self.zoom
    }

    pub fn to_board(&self, origin: egui::Pos2, p: egui::Pos2) -> [f32; 2] {
        let v = (p - origin - self.pan) / self.zoom;
        [v.x, v.y]
    }

    /// Zooms by `factor`, keeping the board point under `anchor` in place.
    pub fn zoom_at(&mut self, origin: egui::Pos2, anchor: egui::Pos2, factor: f32) {
        let before = self.to_board(origin, anchor);
        self.zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        let after = self.to_screen(origin, before);
        self.pan += anchor - after;
    }
}

/// Height of a note's top strip (its colour dot and x sit on it): taken
/// there, a note moves whatever the tool.
const NOTE_GRIP: f32 = 18.0;
/// How near, on screen, a dragged note has to come to lining up with
/// another before it snaps there.
const SNAP_REACH: f32 = 6.0;
/// The dashed lines a dragged note shows when it lines up with others.
pub(crate) const GUIDE: Color32 = Color32::from_rgb(0xe8, 0x3e, 0x8c);

/// A dashed line shown while a dragged note lines up with others.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NoteGuide {
    /// 0: an upright line at x = `at`; 1: a level line at y = `at`.
    pub axis: usize,
    pub at: f32,
    /// Where it starts and ends along the other axis.
    pub from: f32,
    pub to: f32,
}

/// Where a note `size` big dragged to `at` settles among `others` (their
/// top-left corners and sizes, in board units), and the lines it lines up
/// on. On each axis, within `reach` of another note's middle it takes that
/// middle, and within reach of an edge it takes that edge (flush with it,
/// or just touching it); the nearest wins.
pub(crate) fn snap_note(
    at: [f32; 2],
    size: [f32; 2],
    others: &[([f32; 2], [f32; 2])],
    reach: f32,
) -> ([f32; 2], Vec<NoteGuide>) {
    // a note's lines on an axis: (offset from its corner, the middle?)
    let lines = |s: f32| [(0.0, false), (s / 2.0, true), (s, false)];
    let mut pos = at;
    for axis in 0..2 {
        let mut best: Option<f32> = None;
        for (o, os) in others {
            for (a, am) in lines(size[axis]) {
                for (b, bm) in lines(os[axis]) {
                    let d = o[axis] + b - (at[axis] + a);
                    if am == bm && d.abs() <= reach && best.is_none_or(|x| d.abs() < x.abs()) {
                        best = Some(d);
                    }
                }
            }
        }
        pos[axis] += best.unwrap_or(0.0);
    }
    let on = |x: f32, y: f32| (x - y).abs() < 0.01;
    let mut guides = vec![];
    for axis in 0..2 {
        let (s, k) = (size[axis], 1 - axis);
        let mut add = |at: f32, with: Vec<&([f32; 2], [f32; 2])>| {
            if with.is_empty() {
                return;
            }
            let from = with.iter().map(|(o, _)| o[k]).fold(pos[k], f32::min);
            let to = with
                .iter()
                .map(|(o, os)| o[k] + os[k])
                .fold(pos[k] + size[k], f32::max);
            guides.push(NoteGuide { axis, at, from, to });
        };
        // middles in line: one line through them
        let mid = pos[axis] + s / 2.0;
        let in_line = |o: &&([f32; 2], [f32; 2])| on(o.0[axis] + o.1[axis] / 2.0, mid);
        add(mid, others.iter().filter(in_line).collect());
        // a note edge to edge with it
        for edge in [pos[axis], pos[axis] + s] {
            add(
                edge,
                others
                    .iter()
                    .filter(|o| {
                        !in_line(o) && (on(o.0[axis], edge) || on(o.0[axis] + o.1[axis], edge))
                    })
                    .collect(),
            );
        }
    }
    (pos, guides)
}

/// The pointer while drawing with the pen: a pencil, its tip (in the pen's
/// colour) where the line starts. egui has no pencil among its cursors, so
/// the real one is hidden and this is painted at its place.
fn paint_pencil(p: &egui::Painter, tip: egui::Pos2, ink: Color32) {
    let u = vec2(1.0, -1.0).normalized();
    let v = vec2(1.0, 1.0).normalized();
    let at = |along: f32, side: f32| tip + u * along + v * side;
    let line = Stroke::new(1.2_f32, Color32::from_gray(40));
    let (cone, end) = (6.5, 20.0);
    p.add(egui::Shape::convex_polygon(
        vec![at(cone, -3.5), at(cone, 3.5), at(end, 3.5), at(end, -3.5)],
        Color32::WHITE,
        line,
    ));
    p.add(egui::Shape::convex_polygon(
        vec![tip, at(cone, 3.5), at(cone, -3.5)],
        Color32::from_rgb(0xf3, 0xdc, 0xb8),
        line,
    ));
    p.add(egui::Shape::convex_polygon(
        vec![tip, at(2.6, 1.4), at(2.6, -1.4)],
        ink,
        Stroke::new(0.8_f32, ink),
    ));
    p.line_segment([at(end - 4.0, -3.5), at(end - 4.0, 3.5)], line);
}

/// Pen and note colours on a whiteboard.
pub const WB_COLORS: [[u8; 3]; 6] = [
    [32, 32, 32],
    [0xd3, 0x3d, 0x44],
    [8, 128, 234],
    [41, 151, 100],
    [0xf8, 0xae, 0x00],
    [0x5f, 0x55, 0xee],
];

// ---------------------------------------------------------------- whiteboards

impl App {
    pub(crate) fn whiteboard_canvas(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.column_bg))
            .show_inside(ui, |ui| {
                let Some(wi) = self
                    .wb_open
                    .and_then(|id| self.data.extra.whiteboards.iter().position(|w| w.id == id))
                else {
                    ui.label(
                        RichText::new(tr(
                            "Create a whiteboard to sketch ideas and add sticky notes.",
                        ))
                        .color(t.text2),
                    );
                    return;
                };
                let wid = self.data.extra.whiteboards[wi].id;
                // Cmd+Z takes back my last change on the board, as the Undo
                // button does; typing in a note or the board's name, it
                // undoes the typing instead (Cmd+Shift+Z is not an undo)
                if self.modal.is_none()
                    && !ui.ctx().wants_keyboard_input()
                    && ui.input_mut(|i| {
                        let redo = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
                        !i.consume_key(redo, egui::Key::Z)
                            && i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)
                    })
                {
                    self.wb_undo(wid);
                }
                if self.sweep.wb_width <= 0.0 {
                    self.sweep.wb_width = 2.5;
                }
                // the board fills the whole page; its bars float over it
                let (rect, resp) =
                    ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                name_it(&resp, "Whiteboard canvas");
                crate::widgets::no_pointer(&resp);
                let origin = rect.min + WB_ORIGIN;
                // pan and zoom: Space or middle-button drag and two-finger
                // scroll move the view; pinch, Cmd+scroll and the buttons zoom
                let mut view = self.sweep.wb_view.get(&wid).copied().unwrap_or_default();
                let space =
                    !ui.ctx().wants_keyboard_input() && ui.input(|i| i.key_down(egui::Key::Space));
                // the middle button moves the view from wherever on the board
                // its drag starts, a shape, a note or a handle too (which
                // took the drag: a shape moved instead), but not from a
                // floating bar, a menu or a dialog
                let bars = std::mem::take(&mut self.sweep.wb_bars);
                let middle_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle));
                let middle = middle_down
                    && self.modal.is_none()
                    && !ui.memory(|m| m.any_popup_open())
                    && ui
                        .input(|i| i.pointer.press_origin())
                        .is_some_and(|o| rect.contains(o) && !bars.iter().any(|b| b.contains(o)));
                let panning = middle || (space && resp.dragged_by(egui::PointerButton::Primary));
                if middle {
                    view.pan += ui.input(|i| i.pointer.delta());
                } else if panning {
                    let started = resp.drag_started_by(egui::PointerButton::Primary);
                    // the first step counts from where the button went down
                    let from_press = ui.input(|i| {
                        i.pointer
                            .press_origin()
                            .zip(i.pointer.interact_pos())
                            .map(|(o, p)| p - o)
                    });
                    view.pan += match from_press {
                        Some(d) if started => d,
                        _ => resp.drag_delta(),
                    };
                }
                if ui.rect_contains_pointer(rect) {
                    let (zoom, scroll, at) = ui
                        .input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
                    if zoom != 1.0 {
                        if let Some(at) = at {
                            view.zoom_at(origin, at, zoom);
                        }
                    } else if scroll != egui::Vec2::ZERO {
                        view.pan += scroll;
                    }
                }
                if (space || middle) && ui.rect_contains_pointer(rect) {
                    ui.ctx().set_cursor_icon(if panning {
                        egui::CursorIcon::Grabbing
                    } else {
                        egui::CursorIcon::Grab
                    });
                }
                self.set_wb_view(wid, view);
                let z = view.zoom;
                let p = ui.painter_at(rect);
                p.rect_filled(rect, 0.0, t.column_bg);
                // dot grid, moving with the board
                let step = 24.0 * z;
                if step >= 6.0 {
                    let off = WB_ORIGIN + view.pan;
                    let first = |lo: f32, o: f32| lo + (o.rem_euclid(step));
                    let mut y = first(rect.top(), off.y);
                    while y < rect.bottom() {
                        let mut x = first(rect.left(), off.x);
                        while x < rect.right() {
                            p.circle_filled(pos2(x, y), 1.0, t.border);
                            x += step;
                        }
                        y += step;
                    }
                }
                // strokes and notes put in without an id (an older copy)
                // get theirs before anything goes by id
                self.data.fix_board_ids();
                let wb = &self.data.extra.whiteboards[wi];
                for s in &wb.strokes {
                    let pts: Vec<egui::Pos2> = s
                        .points
                        .iter()
                        .map(|q| view.to_screen(origin, *q))
                        .collect();
                    p.add(egui::Shape::line(
                        pts,
                        Stroke::new(s.width * z, rgb(s.color)),
                    ));
                }
                // shapes and their lines, under the notes
                self.wb_shapes_layer(ui, &t, rect, origin, view, wi, space, resp.hovered());
                let wb = &mut self.data.extra.whiteboards[wi];
                // a note being dragged follows the pointer from where it was
                // taken, lining up with the notes in view as it nears them
                let mut guides = vec![];
                if let Some((_, nid, start)) = self.sweep.wb_note_drag.filter(|d| d.0 == wid) {
                    let ni = wb.notes.iter().position(|n| n.id == nid);
                    let moved = ui.input(|i| {
                        i.pointer
                            .press_origin()
                            .zip(i.pointer.interact_pos())
                            .filter(|_| i.pointer.primary_down())
                            .map(|(o, q)| (q - o) / z)
                    });
                    match (ni, moved) {
                        (Some(ni), Some(d)) => {
                            let in_view: Vec<([f32; 2], [f32; 2])> = wb
                                .notes
                                .iter()
                                .filter(|n| n.id != nid)
                                .map(|n| (n.pos, n.wh()))
                                .filter(|(pos, wh)| {
                                    Rect::from_min_size(
                                        view.to_screen(origin, *pos),
                                        vec2(wh[0], wh[1]) * z,
                                    )
                                    .intersects(rect)
                                })
                                .collect();
                            let (at, lines) = snap_note(
                                [start[0] + d.x, start[1] + d.y],
                                wb.notes[ni].wh(),
                                &in_view,
                                SNAP_REACH / z,
                            );
                            if wb.notes[ni].pos != at {
                                wb.notes[ni].pos = at;
                                self.dirty = true;
                            }
                            guides = lines;
                        }
                        _ => {
                            // let go; a drag that left it where it was is
                            // nothing to undo
                            self.sweep.wb_note_drag = None;
                            let back = ni.is_some_and(|ni| wb.notes[ni].pos == start);
                            if back
                                && matches!(self.sweep.wb_undo.last(),
                                    Some((w, WbUndo::NoteMoved(id, _))) if *w == wid && *id == nid)
                            {
                                self.sweep.wb_undo.pop();
                            }
                        }
                    }
                }
                let mut move_from = None;
                let mut size_from = None;
                let mut size_note = None;
                let mut drop_note = None;
                let mut recolor = None;
                let mut type_in = None;
                let hover = ui.input(|i| i.pointer.hover_pos());
                for (i, n) in wb.notes.iter_mut().enumerate() {
                    // every widget of a note takes its id from the note, so
                    // notes skipped off screen don't hand their ids (and the
                    // typing focus) to another note
                    let key = if n.id != 0 {
                        n.id
                    } else {
                        (1 << 63) | i as u64
                    };
                    let wh = n.wh();
                    let r =
                        Rect::from_min_size(view.to_screen(origin, n.pos), vec2(wh[0], wh[1]) * z);
                    // notes off the visible part of the board are not drawn
                    if !r.intersects(rect) {
                        continue;
                    }
                    p.rect_filled(
                        r.translate(vec2(2.0, 3.0) * z),
                        4.0 * z,
                        Color32::from_black_alpha(25),
                    );
                    p.rect_filled(r, 4.0 * z, note_fill(&t, n.color));
                    let inner = Rect::from_min_max(
                        r.min + vec2(8.0, NOTE_GRIP) * z,
                        r.max - vec2(8.0, 8.0) * z,
                    );
                    let text_id = egui::Id::new(("wb_note_text", wid, key));
                    let typable = z >= 0.45;
                    // a click anywhere on the note is for typing in it (with
                    // the sticky note tool it made another note on top); a
                    // drag from its margins still draws or erases
                    let body = ui.interact(
                        r.intersect(rect),
                        ui.id().with(("note_body", wid, key)),
                        Sense::click(),
                    );
                    crate::widgets::no_pointer(&body);
                    if body.clicked() && typable && !space {
                        type_in = Some((text_id, n.text.chars().count()));
                    }
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt(("wb_note", wid, key))
                            .max_rect(inner),
                    );
                    child.set_clip_rect(inner.intersect(rect));
                    if typable
                        && child
                            .add(
                                egui::TextEdit::multiline(&mut n.text)
                                    .id(text_id)
                                    .frame(false)
                                    .font(FontId::proportional(13.0 * z))
                                    .desired_width((wh[0] - 16.0) * z)
                                    .desired_rows((((wh[1] - 26.0) / 18.0) as usize).max(1))
                                    // with Space or the middle button down a
                                    // drag over the text moves the board
                                    .interactive(!space && !middle_down),
                            )
                            .changed()
                    {
                        self.dirty = true;
                    }
                    if !typable {
                        // too small to type in: the text is only drawn
                        p.with_clip_rect(inner.intersect(rect)).text(
                            inner.left_top(),
                            Align2::LEFT_TOP,
                            &n.text,
                            FontId::proportional(13.0 * z),
                            t.text,
                        );
                    }
                    // a note moves taken by its top strip, whatever the
                    // tool, or with the move tool anywhere on it. A click
                    // there is for typing; once typing, the move tool too
                    // moves it by the strip only, so the text can be
                    // clicked into and selected
                    let editing = typable && ui.memory(|m| m.has_focus(text_id));
                    let grip = Rect::from_min_size(r.min, vec2(r.width(), NOTE_GRIP * z));
                    if !space {
                        let area = if self.wb_tool == 2 && !editing {
                            r
                        } else {
                            grip
                        };
                        let h = ui.interact(
                            area.intersect(rect),
                            ui.id().with(("note", wid, key)),
                            Sense::click_and_drag(),
                        );
                        crate::widgets::no_pointer(&h);
                        if h.drag_started_by(egui::PointerButton::Primary) {
                            move_from = Some((n.id, n.pos));
                        }
                        if h.clicked() && typable {
                            type_in = Some((text_id, n.text.chars().count()));
                        }
                        let moving = self
                            .sweep
                            .wb_note_drag
                            .is_some_and(|d| d.0 == wid && d.1 == n.id);
                        if h.dragged() || moving {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                        } else if h.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        }
                    }
                    // dots on the strip show where to take it, while the
                    // pointer is on the note or it is being typed in
                    let pointed = hover.is_some_and(|q| r.contains(q) && rect.contains(q));
                    if typable && (pointed || editing) {
                        let k = z.min(1.5);
                        for dx in [-5.0, 0.0, 5.0] {
                            for dy in [-2.0, 2.0] {
                                p.circle_filled(
                                    grip.center() + vec2(dx, dy) * k,
                                    1.2 * k,
                                    Color32::from_black_alpha(80),
                                );
                            }
                        }
                    }
                    // a colour dot along the note's top edge recolours it
                    let dot = Rect::from_center_size(
                        r.left_top() + vec2(12.0, 10.0) * z,
                        vec2(14.0, 14.0),
                    );
                    if rect.contains_rect(dot) {
                        let dr = ui.interact(
                            dot,
                            ui.id().with(("note_color", wid, key)),
                            Sense::click(),
                        );
                        name_it(&dr, &format!("Note colour {}", i + 1));
                        if dr.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        p.circle_filled(dot.center(), 5.0, rgb(n.color));
                        hover_ring(ui, &dr, dot.shrink(1.0), 6.0);
                        p.circle_stroke(
                            dot.center(),
                            5.0,
                            Stroke::new(1.0_f32, Color32::from_black_alpha(60)),
                        );
                        popup(
                            ui,
                            &dr,
                            ui.id().with(("note_color_pop", wid, key)),
                            150.0,
                            |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    for c in WB_COLORS
                                        .iter()
                                        .skip(1)
                                        .chain(STATUS_PALETTE.iter().skip(5).step_by(2))
                                    {
                                        let (cr, cresp) = ui
                                            .allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                                        ui.painter().circle_filled(cr.center(), 8.0, rgb(*c));
                                        hover_ring(ui, &cresp, cr.shrink(1.0), 9.0);
                                        if *c == n.color {
                                            ui.painter().circle_stroke(
                                                cr.center(),
                                                10.0,
                                                Stroke::new(1.5_f32, t.text),
                                            );
                                        }
                                        if cresp.clicked() {
                                            recolor = Some((i, *c));
                                            close_popup(ui);
                                        }
                                    }
                                });
                            },
                        );
                    }
                    // a small x on the corner deletes just this note
                    let xr = Rect::from_center_size(
                        r.right_top() + vec2(-9.0, 9.0) * z,
                        vec2(16.0, 16.0),
                    );
                    if rect.contains_rect(xr) {
                        let x = ui.interact(xr, ui.id().with(("note_x", wid, key)), Sense::click());
                        hover_bg(ui, &x, xr, 4.0);
                        p.text(
                            xr.center(),
                            Align2::CENTER_CENTER,
                            "\u{00D7}",
                            FontId::proportional(9.0),
                            if x.hovered() { t.text } else { t.text3 },
                        );
                        if x.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let x = x.on_hover_text(tr("Delete note"));
                        name_it(&x, &format!("Delete note {}", i));
                        if x.clicked() {
                            drop_note = Some(i);
                        }
                    }
                    // the bottom right corner resizes it, whatever the tool
                    let gr =
                        Rect::from_min_max(r.right_bottom() - vec2(16.0, 16.0), r.right_bottom());
                    if typable && !space && rect.contains_rect(gr) {
                        let g =
                            ui.interact(gr, ui.id().with(("note_size", wid, key)), Sense::drag());
                        name_it(&g, &format!("Resize note {}", i));
                        let on = g.hovered() || g.dragged();
                        if on {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                        }
                        if on || pointed || editing {
                            let k = gr.right_bottom() - vec2(4.0, 4.0);
                            for d in [5.0, 9.0] {
                                p.line_segment(
                                    [k - vec2(d, 0.0), k - vec2(0.0, d)],
                                    Stroke::new(
                                        1.2_f32,
                                        Color32::from_black_alpha(if on { 150 } else { 80 }),
                                    ),
                                );
                            }
                        }
                        if g.drag_started_by(egui::PointerButton::Primary) {
                            size_from = Some((n.id, n.size));
                        }
                        if g.dragged_by(egui::PointerButton::Primary) {
                            size_note = Some((i, g.drag_delta() / z));
                        }
                    }
                }
                if let Some((id, was)) = size_from {
                    self.sweep.wb_undo.push((wid, WbUndo::NoteResized(id, was)));
                }
                if let Some((i, d)) = size_note {
                    let n = &mut wb.notes[i];
                    let wh = n.wh();
                    n.size = [
                        (wh[0] + d.x).clamp(100.0, 800.0),
                        (wh[1] + d.y).clamp(70.0, 800.0),
                    ];
                    self.dirty = true;
                }
                if let Some((i, c)) = recolor {
                    wb.notes[i].color = c;
                    self.dirty = true;
                }
                if let Some(i) = drop_note {
                    // Undo brings it back
                    let n = wb.notes.remove(i);
                    self.sweep.wb_undo.push((wid, WbUndo::NoteGone(i, n)));
                    self.dirty = true;
                }
                if let Some((id, at)) = move_from {
                    self.sweep.wb_undo.push((wid, WbUndo::NoteMoved(id, at)));
                    self.sweep.wb_note_drag = Some((wid, id, at));
                    ui.ctx().request_repaint();
                }
                if let Some((id, len)) = type_in {
                    // the caret goes after the text, ready to add to it
                    ui.memory_mut(|m| m.request_focus(id));
                    let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(len),
                    )));
                    st.store(ui.ctx(), id);
                }
                // the lines the dragged note lines up on, over the notes
                for g in &guides {
                    let (a, b, ext) = if g.axis == 0 {
                        ([g.at, g.from], [g.at, g.to], vec2(0.0, 12.0))
                    } else {
                        ([g.from, g.at], [g.to, g.at], vec2(12.0, 0.0))
                    };
                    p.extend(egui::Shape::dashed_line(
                        &[
                            view.to_screen(origin, a) - ext,
                            view.to_screen(origin, b) + ext,
                        ],
                        Stroke::new(1.0_f32, GUIDE),
                        4.0,
                        3.0,
                    ));
                }
                // the tools draw with the left button, in board coordinates;
                // a pan (Space or the middle button) never draws
                let primary = resp.dragged_by(egui::PointerButton::Primary)
                    || resp.drag_started_by(egui::PointerButton::Primary)
                    || resp.drag_stopped_by(egui::PointerButton::Primary)
                    || resp.clicked();
                // over the notes: the pick's handles and bar
                self.wb_pick_layer(ui, &t, rect, origin, view, wi);
                if let Some(pos) = resp.interact_pointer_pos().filter(|_| primary && !space) {
                    let b = view.to_board(origin, pos);
                    let local = vec2(b[0], b[1]);
                    match self.wb_tool {
                        0 => {
                            if resp.drag_started() {
                                // the line starts where the button went down
                                let start = ui
                                    .input(|i| i.pointer.press_origin())
                                    .map(|o| view.to_board(origin, o));
                                let mut points: Vec<[f32; 2]> = start.into_iter().collect();
                                points.push([local.x, local.y]);
                                // a real id from the first point on: a merge
                                // while the button is still down must see a
                                // board whose every item has one (see
                                // wb_sync::is_id_array)
                                let id = self.data.new_id();
                                self.data.extra.whiteboards[wi].strokes.push(WbStroke {
                                    id,
                                    color: self.wb_color,
                                    width: self.sweep.wb_width,
                                    points,
                                });
                                self.sweep.wb_drawing = Some((wid, id));
                                self.sweep.wb_undo.push((wid, WbUndo::Stroke(id)));
                                self.dirty = true;
                            } else if resp.dragged() {
                                // the stroke being drawn, wherever a merge put
                                // it (a teammate's strokes can come after it)
                                let drawing = self.sweep.wb_drawing.filter(|d| d.0 == wid);
                                if let Some(s) = drawing.and_then(|(_, sid)| {
                                    self.data.extra.whiteboards[wi]
                                        .strokes
                                        .iter_mut()
                                        .find(|s| s.id == sid)
                                }) {
                                    s.points.push([local.x, local.y]);
                                }
                            }
                            if resp.drag_stopped() {
                                self.sweep.wb_drawing = None;
                                self.dirty = true;
                            }
                        }
                        1 => {
                            if resp.clicked() {
                                let id = self.data.new_id();
                                self.data.extra.whiteboards[wi].notes.push(Sticky {
                                    id,
                                    pos: [local.x - 80.0, local.y - 60.0],
                                    text: String::new(),
                                    color: super::wb_panels::sticky_color(self.wb_color),
                                    ..Default::default()
                                });
                                self.sweep.wb_undo.push((wid, WbUndo::Note(id)));
                                self.dirty = true;
                            }
                        }
                        3 => {
                            // the eraser takes away whole strokes it touches
                            if resp.dragged() || resp.clicked() {
                                let hit = |s: &WbStroke| {
                                    s.points.iter().any(|q| {
                                        vec2(q[0] - local.x, q[1] - local.y).length()
                                            < s.width + 6.0 / z
                                    })
                                };
                                let wb = &mut self.data.extra.whiteboards[wi];
                                while let Some(i) = wb.strokes.iter().position(hit) {
                                    let s = wb.strokes.remove(i);
                                    self.sweep.wb_undo.push((wid, WbUndo::StrokeGone(i, s)));
                                    self.dirty = true;
                                }
                            }
                        }
                        2 => {
                            if resp.clicked() {
                                self.wb_click_board(wid, wi, origin, view, pos);
                            }
                        }
                        4 => {
                            if resp.clicked() {
                                self.wb_place_shape(wid, wi, b);
                            }
                        }
                        5 => {
                            let press = ui
                                .input(|i| i.pointer.press_origin())
                                .map(|o| view.to_board(origin, o));
                            self.wb_link_drag(&resp, wid, wi, b, press);
                        }
                        _ => {}
                    }
                }
                if !resp.dragged() {
                    // a Line tool drag that ended off the board
                    self.sweep.wb_link = None;
                }
                // the pen's pointer is a pencil on the board (on a note's text
                // it stays the I-beam, on its top strip the hand)
                let drawing = resp.dragged_by(egui::PointerButton::Primary);
                if self.wb_tool == 0 && !space && !panning && (resp.hovered() || drawing) {
                    if let Some(at) = hover.filter(|q| rect.contains(*q)) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
                        let top =
                            egui::LayerId::new(egui::Order::Tooltip, ui.id().with("wb_pencil"));
                        paint_pencil(&ui.ctx().layer_painter(top), at, rgb(self.wb_color));
                    }
                }
                // drawn last, so they sit on top of the board and take the
                // clicks over them
                self.wb_panels(ui, rect, origin, wi);
            });
    }

    /// Keeps a board's view, forgetting it when it is the starting one.
    pub(crate) fn set_wb_view(&mut self, wid: u64, view: WbView) {
        if view == WbView::default() {
            self.sweep.wb_view.remove(&wid);
        } else {
            self.sweep.wb_view.insert(wid, view);
        }
    }

    /// Takes back my last step on a board: a stroke, note, shape or line
    /// added, deleted, moved, resized or restyled, or an erasing.
    pub(crate) fn wb_undo(&mut self, wid: u64) {
        let Some(k) = self.sweep.wb_undo.iter().rposition(|u| u.0 == wid) else {
            return;
        };
        let (_, op) = self.sweep.wb_undo.remove(k);
        let Some(wb) = self.data.extra.whiteboards.iter_mut().find(|w| w.id == wid) else {
            return;
        };
        // by id: a stroke or note a teammate took away meanwhile is left
        // alone (nothing happens), and theirs are never touched
        let done = match op {
            WbUndo::Stroke(id) => match wb.strokes.iter().position(|s| s.id == id) {
                Some(i) => {
                    wb.strokes.remove(i);
                    true
                }
                None => false,
            },
            WbUndo::Note(id) => match wb.notes.iter().position(|n| n.id == id) {
                Some(i) => {
                    wb.notes.remove(i);
                    true
                }
                None => false,
            },
            WbUndo::NoteGone(i, n) => {
                let back = !wb.notes.iter().any(|x| x.id == n.id);
                if back {
                    wb.notes.insert(i.min(wb.notes.len()), n);
                }
                back
            }
            WbUndo::StrokeGone(i, s) => {
                let back = !wb.strokes.iter().any(|x| x.id == s.id);
                if back {
                    wb.strokes.insert(i.min(wb.strokes.len()), s);
                }
                back
            }
            WbUndo::NoteMoved(id, at) => match wb.notes.iter_mut().find(|n| n.id == id) {
                Some(n) => {
                    n.pos = at;
                    true
                }
                None => false,
            },
            op => super::wb_shapes::undo_shapes(wb, op),
        };
        if done {
            self.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dragged_note_lines_up_with_a_note_near_it() {
        let a = ([0.0, 0.0], Sticky::SIZE);
        let sz = Sticky::SIZE;
        // 4 under the other's middle line: it takes the line, on both axes
        let (at, lines) = snap_note([300.0, 4.0], sz, &[a], 6.0);
        assert_eq!(at, [300.0, 0.0]);
        assert_eq!(
            lines,
            vec![NoteGuide {
                axis: 1,
                at: 60.0,
                from: 0.0,
                to: 460.0
            }],
            "one level line through both middles, from one note's far side to the other's"
        );
        // under it, 5 off its middle: an upright line
        let (at, lines) = snap_note([5.0, 300.0], sz, &[a], 6.0);
        assert_eq!(at, [0.0, 300.0]);
        assert_eq!(lines.len(), 1);
        assert_eq!((lines[0].axis, lines[0].at), (0, 80.0));
        // just right of it, 3 apart: the edges meet
        let (at, lines) = snap_note([163.0, 200.0], sz, &[a], 6.0);
        assert_eq!(at, [160.0, 200.0]);
        assert_eq!((lines[0].axis, lines[0].at), (0, 160.0));
        // too far off: it stays where it is put, with no lines
        let (at, lines) = snap_note([310.0, 9.0], sz, &[a], 6.0);
        assert_eq!(at, [310.0, 9.0]);
        assert!(lines.is_empty());
        // the nearer of two wins
        let (at, _) = snap_note([300.0, 4.0], sz, &[a, ([600.0, 2.0], sz)], 6.0);
        assert_eq!(at, [300.0, 2.0]);
        // a note made bigger lines up by its own middle and edges
        let big = ([0.0, 0.0], [320.0, 200.0]);
        let (at, lines) = snap_note([400.0, 43.0], sz, &[big], 6.0);
        assert_eq!(at, [400.0, 40.0], "middles in line: 100 = 40 + 120 / 2");
        assert_eq!((lines[0].axis, lines[0].at, lines[0].to), (1, 100.0, 560.0));
        let (at, _) = snap_note([323.0, 300.0], sz, &[big], 6.0);
        assert_eq!(at[0], 320.0, "touching its right edge");
    }
}
