// Whiteboards: the canvas with its pen, shapes, sticky notes, pan and zoom,
// and Undo.

use super::*;

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
            .frame(
                egui::Frame::new()
                    .fill(t.bg)
                    .inner_margin(egui::Margin::same(12)),
            )
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
                let mut delete = false;
                let mut btn_zoom: Option<f32> = None;
                let mut reset_view = false;
                if self.sweep.wb_width <= 0.0 {
                    self.sweep.wb_width = 2.5;
                }
                // wraps onto a second row in a narrow window
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.data.extra.whiteboards[wi].name)
                                .font(FontId::proportional(18.0))
                                .frame(false)
                                .desired_width(200.0),
                        )
                        .changed()
                    {
                        self.dirty = true;
                    }
                    ui.separator();
                    // the tool in use is outlined; each tool has its own icon
                    // (the pen and the sticky note shared one emoji)
                    for (tool, kind, label) in [
                        (0u8, "pen", "Pen"),
                        (3, "eraser", "Eraser"),
                        (1, "sticky", "Sticky note"),
                        (2, "move", "Move notes"),
                    ] {
                        let sel = self.wb_tool == tool;
                        let text_color = if sel { t.text } else { t.text2 };
                        if icon_button_ex(ui, &t, kind, tr(label), label, sel, 0.0, text_color, t.text2)
                            .clicked()
                        {
                            self.wb_tool = tool;
                        }
                    }
                    for c in WB_COLORS {
                        let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 8.0, rgb(c));
                        hover_ring(ui, &resp, r.shrink(1.0), 9.0);
                        if self.wb_color == c {
                            ui.painter()
                                .circle_stroke(r.center(), 10.0, Stroke::new(1.5_f32, t.text));
                        }
                        if resp.clicked() {
                            self.wb_color = c;
                        }
                    }
                    let wr = ui.add(
                        egui::DragValue::new(&mut self.sweep.wb_width)
                            .range(1.0..=16.0)
                            .speed(0.2)
                            .max_decimals(1)
                            .prefix(tr("width ")),
                    );
                    name_it(&wr, "Pen width");
                    ui.separator();
                    let can_undo = self.sweep.wb_undo.iter().any(|u| u.0 == wid);
                    if ui
                        .add_enabled(can_undo, egui::Button::new(tr("Undo")))
                        .clicked()
                    {
                        self.wb_undo(wid);
                    }
                    ui.separator();
                    let zr = ui.button(crate::app::icons::sign_text(true));
                    crate::app::icons::paint_sign(ui, &zr, true, None);
                    let zr = zr.on_hover_text(tr("Zoom out"));
                    name_it(&zr, "Zoom out");
                    if zr.clicked() {
                        btn_zoom = Some(1.0 / 1.25);
                    }
                    let zoom_now = self.sweep.wb_view.get(&wid).map_or(1.0, |v| v.zoom);
                    ui.label(
                        RichText::new(format!("{:.0}%", zoom_now * 100.0))
                            .color(t.text2)
                            .small(),
                    );
                    let zr = ui.button(crate::app::icons::sign_text(false));
                    crate::app::icons::paint_sign(ui, &zr, false, None);
                    let zr = zr.on_hover_text(tr("Zoom in"));
                    name_it(&zr, "Zoom in");
                    if zr.clicked() {
                        btn_zoom = Some(1.25);
                    }
                    let rr = ui.button(tr("Reset view")).on_hover_text(crate::os::keys(tr(
                        "Back to 100% at the board's top left corner. Drag with Space or the middle button, or scroll, to move around; pinch or \u{2318}+scroll to zoom.",
                    )));
                    name_it(&rr, "Reset view");
                    if rr.clicked() {
                        reset_view = true;
                    }
                    // the question belongs to this board only
                    let asking = self
                        .sweep
                        .confirm
                        .clone()
                        .and_then(|c| c.strip_suffix(&format!(":{}", wid)).map(String::from));
                    match asking.as_deref() {
                        Some("wb_clear") => {
                            ui.label(RichText::new(tr("Clear the whole board?")).color(t.danger));
                            if ui.button(tr("Yes")).clicked() {
                                self.sweep.confirm = None;
                                let wb = &mut self.data.extra.whiteboards[wi];
                                wb.strokes.clear();
                                wb.notes.clear();
                                self.sweep.wb_undo.retain(|u| u.0 != wid);
                                self.dirty = true;
                            }
                            if ui.button(tr("No")).clicked() {
                                self.sweep.confirm = None;
                            }
                        }
                        _ => {
                            if ui.button(tr("Clear")).clicked() {
                                self.sweep.confirm = Some(format!("wb_clear:{}", wid));
                            }
                            // framed like the other buttons in this bar
                            if ui
                                .button(RichText::new(tr("Delete")).color(t.danger))
                                .clicked()
                            {
                                delete = true;
                            }
                        }
                    }
                    let wid = self.data.extra.whiteboards[wi].id;
                    self.star_toggle(ui, format!("wb:{}", wid));
                });
                let (rect, resp) =
                    ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
                name_it(&resp, "Whiteboard canvas");
                crate::widgets::no_pointer(&resp);
                let origin = rect.min;
                // pan and zoom: Space or middle-button drag and two-finger
                // scroll move the view; pinch, Cmd+scroll and the buttons zoom
                let mut view = self.sweep.wb_view.get(&wid).copied().unwrap_or_default();
                if reset_view {
                    view = WbView::default();
                }
                if let Some(f) = btn_zoom {
                    view.zoom_at(origin, rect.center(), f);
                }
                let space = !ui.ctx().wants_keyboard_input()
                    && ui.input(|i| i.key_down(egui::Key::Space));
                let panning = resp.dragged_by(egui::PointerButton::Middle)
                    || (space && resp.dragged_by(egui::PointerButton::Primary));
                if panning {
                    let started = resp.drag_started_by(egui::PointerButton::Middle)
                        || resp.drag_started_by(egui::PointerButton::Primary);
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
                    let (zoom, scroll, at) = ui.input(|i| {
                        (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos())
                    });
                    if zoom != 1.0 {
                        if let Some(at) = at {
                            view.zoom_at(origin, at, zoom);
                        }
                    } else if scroll != egui::Vec2::ZERO {
                        view.pan += scroll;
                    }
                }
                if space && ui.rect_contains_pointer(rect) {
                    ui.ctx().set_cursor_icon(if panning {
                        egui::CursorIcon::Grabbing
                    } else {
                        egui::CursorIcon::Grab
                    });
                }
                if view == WbView::default() {
                    self.sweep.wb_view.remove(&wid);
                } else {
                    self.sweep.wb_view.insert(wid, view);
                }
                let z = view.zoom;
                let p = ui.painter_at(rect);
                p.rect_filled(rect, 8.0, t.column_bg);
                // dot grid, moving with the board
                let step = 24.0 * z;
                if step >= 6.0 {
                    let off = view.pan;
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
                let wb = &mut self.data.extra.whiteboards[wi];
                for s in &wb.strokes {
                    let pts: Vec<egui::Pos2> =
                        s.points.iter().map(|q| view.to_screen(origin, *q)).collect();
                    p.add(egui::Shape::line(
                        pts,
                        Stroke::new(s.width * z, rgb(s.color)),
                    ));
                }
                let mut drag_note = None;
                let mut move_from = None;
                let mut drop_note = None;
                let mut recolor = None;
                for (i, n) in wb.notes.iter_mut().enumerate() {
                    // every widget of a note takes its id from the note, so
                    // notes skipped off screen don't hand their ids (and the
                    // typing focus) to another note
                    let key = if n.id != 0 { n.id } else { (1 << 63) | i as u64 };
                    let r = Rect::from_min_size(view.to_screen(origin, n.pos), vec2(160.0, 120.0) * z);
                    // notes off the visible part of the board are not drawn
                    if !r.intersects(rect) {
                        continue;
                    }
                    p.rect_filled(
                        r.translate(vec2(2.0, 3.0) * z),
                        4.0 * z,
                        Color32::from_black_alpha(25),
                    );
                    p.rect_filled(r, 4.0 * z, rgb(n.color).gamma_multiply(0.35).to_opaque());
                    let inner =
                        Rect::from_min_max(r.min + vec2(8.0, 18.0) * z, r.max - vec2(8.0, 8.0) * z);
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt(("wb_note", wid, key))
                            .max_rect(inner),
                    );
                    child.set_clip_rect(inner.intersect(rect));
                    if z >= 0.45
                        && child
                            .add(
                                egui::TextEdit::multiline(&mut n.text)
                                    .id(egui::Id::new(("wb_note_text", wid, key)))
                                    .frame(false)
                                    .font(FontId::proportional(13.0 * z))
                                    .desired_width(144.0 * z)
                                    .desired_rows(4),
                            )
                            .changed()
                    {
                        self.dirty = true;
                    }
                    if z < 0.45 {
                        // too small to type in: the text is only drawn
                        p.with_clip_rect(inner.intersect(rect)).text(
                            inner.left_top(),
                            Align2::LEFT_TOP,
                            &n.text,
                            FontId::proportional(13.0 * z),
                            t.text,
                        );
                    }
                    if self.wb_tool == 2 && !space {
                        let h = ui.interact(r.intersect(rect), ui.id().with(("note", wid, key)), Sense::drag());
                        if h.drag_started() {
                            move_from = Some((n.id, n.pos));
                        }
                        if h.dragged() {
                            drag_note = Some((i, h.drag_delta() / z));
                        }
                    }
                    // a colour dot along the note's top edge recolours it
                    let dot =
                        Rect::from_center_size(r.left_top() + vec2(12.0, 10.0) * z, vec2(14.0, 14.0));
                    if rect.contains_rect(dot) {
                        let dr =
                            ui.interact(dot, ui.id().with(("note_color", wid, key)), Sense::click());
                        name_it(&dr, &format!("Note colour {}", i + 1));
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
                    let xr =
                        Rect::from_center_size(r.right_top() + vec2(-9.0, 9.0) * z, vec2(16.0, 16.0));
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
                }
                if let Some((i, d)) = drag_note {
                    wb.notes[i].pos[0] += d.x;
                    wb.notes[i].pos[1] += d.y;
                    self.dirty = true;
                }
                // the tools draw with the left button, in board coordinates;
                // a pan (Space or the middle button) never draws
                let primary = resp.dragged_by(egui::PointerButton::Primary)
                    || resp.drag_started_by(egui::PointerButton::Primary)
                    || resp.drag_stopped_by(egui::PointerButton::Primary)
                    || resp.clicked();
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
                                    color: if self.wb_color == [32, 32, 32] {
                                        [0xf8, 0xae, 0x00]
                                    } else {
                                        self.wb_color
                                    },
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
                        _ => {}
                    }
                }
                if delete {
                    let w = &self.data.extra.whiteboards[wi];
                    self.modal = Some(Modal::ConfirmRemove {
                        kind: "whiteboard",
                        id: w.id,
                        name: w.name.clone(),
                    });
                }
            });
    }

    /// Takes back the last pen stroke, note, note removal, note move or erasing on a board.
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
        };
        if done {
            self.dirty = true;
        }
    }
}
