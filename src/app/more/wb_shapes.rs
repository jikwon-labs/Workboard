// Shapes and lines on a whiteboard: drawing them, picking one with the
// Select tool (its corners resize it, the + on each side adds a shape tied
// to it, the bar above it recolours, fills, restyles or deletes it), the
// Shape and Line tools, and undoing all of it.
//
// Before, a template laid out sticky notes standing in for the boxes of its
// picture; now it lays out the picture itself, and people grow it from there.

use super::wb_library::mix;
use super::wb_panels::{bar_sep, card};
use super::whiteboard::{WbUndo, WbView, WB_COLORS};
use super::*;
use crate::app::icons::paint_plus;
use egui::{Pos2, Response};

/// The shapes the Shape tool puts down: (kind, name, size on the board).
pub(crate) const SHAPE_KINDS: [(&str, &str, [f32; 2]); 5] = [
    ("rect", "Rectangle", [160.0, 80.0]),
    ("pill", "Rounded rectangle", [160.0, 64.0]),
    ("diamond", "Diamond", [150.0, 110.0]),
    ("ellipse", "Circle", [120.0, 120.0]),
    ("text", "Text", [160.0, 40.0]),
];

/// Space between a shape and one its + adds; the templates are laid out
/// with it, so what the + adds lines up with what was there.
pub(crate) const GAP: f32 = 60.0;

/// The smallest a shape is resized to.
const MIN_SIZE: [f32; 2] = [40.0, 24.0];

/// What the Select tool has picked on a board.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WbSel {
    Shape(u64),
    Line(u64),
}

/// How the Line tool draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WbLineStyle {
    pub elbow: bool,
    pub dashed: bool,
    pub arrow: bool,
}

impl Default for WbLineStyle {
    fn default() -> Self {
        WbLineStyle {
            elbow: false,
            dashed: false,
            arrow: true,
        }
    }
}

/// A line being dragged out with the Line tool: from a shape (or 0) and the
/// board point the press was at.
#[derive(Clone, Copy, Debug)]
pub struct WbLink {
    pub wid: u64,
    pub from: u64,
    pub a: [f32; 2],
}

fn bpos(p: [f32; 2]) -> Pos2 {
    pos2(p[0], p[1])
}

/// A shape's place on the board.
pub(crate) fn shape_rect(s: &WbShape) -> Rect {
    Rect::from_min_size(bpos(s.pos), vec2(s.size[0], s.size[1]))
}

/// Where a line from the middle of `s` toward `to` crosses its outline.
fn edge_toward(s: &WbShape, to: Pos2) -> Pos2 {
    let r = shape_rect(s);
    let c = r.center();
    let d = to - c;
    let (hw, hh) = (r.width().max(1.0) / 2.0, r.height().max(1.0) / 2.0);
    if d.length() < 0.01 {
        return c;
    }
    let k = match s.kind.as_str() {
        "diamond" => 1.0 / (d.x.abs() / hw + d.y.abs() / hh),
        "ellipse" => 1.0 / ((d.x / hw).powi(2) + (d.y / hh).powi(2)).sqrt(),
        _ => 1.0 / (d.x.abs() / hw).max(d.y.abs() / hh),
    };
    c + d * k
}

/// The points a line goes through on the board, from its `a` end to its
/// `b` end. A tied end starts at its shape's outline; an elbow line leaves
/// and enters through the middle of a side and turns at right angles.
pub(crate) fn line_path(l: &WbLine, shapes: &[WbShape]) -> Vec<Pos2> {
    let find = |id: u64| (id != 0).then(|| shapes.iter().find(|s| s.id == id))?;
    let (fs, ts) = (find(l.from), find(l.to));
    let a0 = fs.map_or(bpos(l.a), |s| shape_rect(s).center());
    let b0 = ts.map_or(bpos(l.b), |s| shape_rect(s).center());
    if l.elbow {
        let (ra, rb) = (fs.map(shape_rect), ts.map(shape_rect));
        let d = b0 - a0;
        // up and down when one shape is wholly above the other (an org
        // chart's boxes, however far apart sideways), across when they sit
        // side by side, else whichever way they are further apart
        let gap = |lo: fn(&Rect) -> f32, hi: fn(&Rect) -> f32| match (ra, rb) {
            (Some(x), Some(y)) => Some(lo(&y) >= hi(&x) || lo(&x) >= hi(&y)),
            _ => None,
        };
        let above = gap(|r| r.top(), |r| r.bottom());
        let beside = gap(|r| r.left(), |r| r.right());
        let upright = match (above, beside) {
            (Some(true), _) => true,
            (_, Some(true)) => false,
            _ => d.y.abs() >= d.x.abs(),
        };
        if upright {
            let down = d.y >= 0.0;
            let a = ra.map_or(a0, |r| {
                if down {
                    r.center_bottom()
                } else {
                    r.center_top()
                }
            });
            let b = rb.map_or(b0, |r| {
                if down {
                    r.center_top()
                } else {
                    r.center_bottom()
                }
            });
            let my = (a.y + b.y) / 2.0;
            vec![a, pos2(a.x, my), pos2(b.x, my), b]
        } else {
            let right = d.x >= 0.0;
            let a = ra.map_or(a0, |r| {
                if right {
                    r.right_center()
                } else {
                    r.left_center()
                }
            });
            let b = rb.map_or(b0, |r| {
                if right {
                    r.left_center()
                } else {
                    r.right_center()
                }
            });
            let mx = (a.x + b.x) / 2.0;
            vec![a, pos2(mx, a.y), pos2(mx, b.y), b]
        }
    } else {
        let a = fs.map_or(a0, |s| edge_toward(s, b0));
        let b = ts.map_or(b0, |s| edge_toward(s, a0));
        vec![a, b]
    }
}

/// The topmost shape at board point `at`.
fn shape_at(shapes: &[WbShape], at: Pos2) -> Option<u64> {
    shapes
        .iter()
        .rev()
        .find(|s| shape_rect(s).contains(at))
        .map(|s| s.id)
}

/// How far `q` is from the line through `path`.
fn dist_to_path(path: &[Pos2], q: Pos2) -> f32 {
    path.windows(2)
        .map(|w| {
            let ab = w[1] - w[0];
            let k = if ab.length_sq() > 0.0 {
                ((q - w[0]).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (w[0] + ab * k - q).length()
        })
        .fold(f32::INFINITY, f32::min)
}

/// Where the + on side `dir` (0 up, 1 right, 2 down, 3 left) of `s` puts a
/// new shape: a gap away, moved along past any shape already there (the
/// next box in an org chart's row).
fn next_spot(shapes: &[WbShape], s: &WbShape, dir: usize) -> Rect {
    let r = shape_rect(s);
    let (w, h) = (r.width(), r.height());
    let (step, along) = match dir {
        0 => (vec2(0.0, -(h + GAP)), vec2(w + GAP, 0.0)),
        1 => (vec2(w + GAP, 0.0), vec2(0.0, h + GAP)),
        2 => (vec2(0.0, h + GAP), vec2(w + GAP, 0.0)),
        _ => (vec2(-(w + GAP), 0.0), vec2(0.0, h + GAP)),
    };
    let mut at = r.translate(step);
    for _ in 0..40 {
        if !shapes
            .iter()
            .any(|o| shape_rect(o).expand(8.0).intersects(at))
        {
            break;
        }
        at = at.translate(along);
    }
    at
}

/// The inside of a shape: a tint of its colour when filled.
fn shape_fill(t: &Theme, s: &WbShape) -> Color32 {
    if s.filled {
        mix(t.bg, rgb(s.color), 0.28)
    } else {
        t.bg
    }
}

/// Draws a shape's inside and outline in `r` (on screen), lines `w` wide.
pub(crate) fn paint_shape_body(p: &egui::Painter, t: &Theme, s: &WbShape, r: Rect, w: f32) {
    let st = Stroke::new(w, rgb(s.color));
    let fill = shape_fill(t, s);
    match s.kind.as_str() {
        "text" => {
            if s.filled {
                p.rect_filled(r, 4.0, fill);
            } else if s.text.is_empty() {
                // an empty label would not show at all
                let pts = [
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                    r.left_top(),
                ];
                p.extend(egui::Shape::dashed_line(
                    &pts,
                    Stroke::new(1.0_f32, t.text3),
                    4.0,
                    3.0,
                ));
            }
        }
        "pill" => {
            let rad = r.height().min(r.width()) / 2.0;
            p.rect_filled(r, rad, fill);
            p.rect_stroke(r, rad, st, egui::StrokeKind::Middle);
        }
        "diamond" => {
            let pts = vec![
                r.center_top(),
                r.right_center(),
                r.center_bottom(),
                r.left_center(),
            ];
            p.add(egui::Shape::convex_polygon(pts, fill, st));
        }
        "ellipse" => {
            // as many points as it is big (egui's own ellipse showed its corners)
            let (rx, ry) = (r.width() / 2.0, r.height() / 2.0);
            let n = ((rx + ry) * 0.8).clamp(24.0, 240.0) as usize;
            let pts: Vec<Pos2> = (0..n)
                .map(|i| {
                    let a = std::f32::consts::TAU * i as f32 / n as f32;
                    r.center() + vec2(a.cos() * rx, a.sin() * ry)
                })
                .collect();
            p.add(egui::Shape::convex_polygon(pts, fill, st));
        }
        _ => {
            let rad = (r.height().min(r.width()) * 0.12).min(10.0);
            p.rect_filled(r, rad, fill);
            p.rect_stroke(r, rad, st, egui::StrokeKind::Middle);
        }
    }
}

/// Draws a line through `pts` (on screen), `w` wide, with its dashes and
/// arrow head.
pub(crate) fn paint_line(p: &egui::Painter, l: &WbLine, pts: &[Pos2], w: f32, col: Color32) {
    if pts.len() < 2 {
        return;
    }
    let st = Stroke::new(w, col);
    let mut pts = pts.to_vec();
    let tip = pts[pts.len() - 1];
    // the arrow points along the last stretch that has any length
    let back = pts
        .iter()
        .rev()
        .skip(1)
        .find(|q| (tip - **q).length() > 0.5)
        .copied();
    let head = (w * 4.0).max(7.0);
    let dir = back
        .filter(|b| l.arrow && (tip - *b).length() > head)
        .map(|b| (tip - b).normalized());
    if let Some(d) = dir {
        let n = pts.len();
        pts[n - 1] = tip - d * head * 0.8;
    }
    if l.dashed {
        p.extend(egui::Shape::dashed_line(&pts, st, w * 4.0, w * 3.0));
    } else {
        p.add(egui::Shape::line(pts, st));
    }
    if let Some(d) = dir {
        let side = vec2(-d.y, d.x) * head * 0.5;
        let base = tip - d * head;
        p.add(egui::Shape::convex_polygon(
            vec![tip, base + side, base - side],
            col,
            Stroke::NONE,
        ));
    }
}

/// Where a shape's text sits inside `r`.
fn text_area(kind: &str, r: Rect, z: f32) -> Rect {
    match kind {
        "diamond" => Rect::from_center_size(r.center(), r.size() * vec2(0.62, 0.5)),
        "ellipse" => Rect::from_center_size(r.center(), r.size() * 0.72),
        "pill" => r.shrink2(vec2(r.height() * 0.3 + 6.0 * z, 6.0 * z)),
        "text" => r.shrink2(vec2(4.0 * z, 2.0 * z)),
        _ => r.shrink(10.0 * z),
    }
}

fn shape_font(ctx: &egui::Context, kind: &str, z: f32) -> FontId {
    if kind == "text" {
        crate::app::bold_font(ctx, 15.0 * z)
    } else {
        FontId::proportional(14.0 * z)
    }
}

fn centred(text: &str, font: FontId, color: Color32, width: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple(text.to_string(), font, color, width.max(1.0));
    job.halign = egui::Align::Center;
    job
}

/// A small picture of a shape kind, for the Shape tool's choices.
pub(super) fn paint_kind_icon(p: &egui::Painter, kind: &str, c: Pos2, col: Color32) {
    let s = Stroke::new(1.5_f32, col);
    match kind {
        "pill" => {
            let r = Rect::from_center_size(c, vec2(18.0, 10.0));
            p.rect_stroke(r, 5.0, s, egui::StrokeKind::Middle);
        }
        "diamond" => {
            let pts = vec![
                c + vec2(0.0, -7.5),
                c + vec2(8.5, 0.0),
                c + vec2(0.0, 7.5),
                c + vec2(-8.5, 0.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
        }
        "ellipse" => {
            p.circle_stroke(c, 7.0, s);
        }
        "text" => {
            p.line_segment([c + vec2(-6.0, -6.5), c + vec2(6.0, -6.5)], s);
            p.line_segment([c + vec2(0.0, -6.5), c + vec2(0.0, 7.0)], s);
        }
        _ => {
            let r = Rect::from_center_size(c, vec2(16.0, 12.0));
            p.rect_stroke(r, 2.0, s, egui::StrokeKind::Middle);
        }
    }
}

/// The line style toggles: (name, what it does, icon).
pub(super) const STYLE_TOGGLES: [(&str, &str); 3] = [
    ("Elbow line", "Turn at right angles"),
    ("Dashed line", "Dashed"),
    ("Arrow", "Arrow at the end"),
];

pub(super) fn paint_style_icon(p: &egui::Painter, k: usize, c: Pos2, col: Color32) {
    let s = Stroke::new(1.5_f32, col);
    match k {
        0 => {
            let pts = vec![
                c + vec2(-7.0, -6.0),
                c + vec2(0.0, -6.0),
                c + vec2(0.0, 6.0),
                c + vec2(7.0, 6.0),
            ];
            p.add(egui::Shape::line(pts, s));
        }
        1 => {
            for x in [-8.0, -2.0, 4.0] {
                p.line_segment([c + vec2(x, 0.0), c + vec2(x + 3.5, 0.0)], s);
            }
        }
        _ => {
            p.line_segment([c + vec2(-8.0, 0.0), c + vec2(3.0, 0.0)], s);
            p.add(egui::Shape::convex_polygon(
                vec![c + vec2(8.0, 0.0), c + vec2(2.0, -4.0), c + vec2(2.0, 4.0)],
                col,
                Stroke::NONE,
            ));
        }
    }
}

fn paint_fill_icon(p: &egui::Painter, c: Pos2, col: Color32, tint: Color32) {
    let r = Rect::from_center_size(c, vec2(14.0, 14.0));
    p.rect_filled(r, 3.0, tint);
    p.rect_stroke(r, 3.0, Stroke::new(1.5_f32, col), egui::StrokeKind::Middle);
}

/// A square button on the bars here; `on` marks a toggle that is on.
fn small_button(
    ui: &Ui,
    t: &Theme,
    r: Rect,
    id: egui::Id,
    name: &str,
    tip: &str,
    on: bool,
) -> Response {
    let resp = ui.interact(r, id, Sense::click());
    name_it(&resp, name);
    let p = ui.painter();
    if on {
        p.rect_filled(r, 6.0, t.primary.gamma_multiply(0.16));
    } else if resp.hovered() {
        p.rect_filled(r, 6.0, t.hover);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(tip)
}

/// Space is held for moving the board (not typed into a text box).
fn space_down(ui: &Ui) -> bool {
    !ui.ctx().wants_keyboard_input() && ui.input(|i| i.key_down(egui::Key::Space))
}

/// What was pressed on the bar over a pick.
enum BarHit {
    Colour([u8; 3]),
    Toggle(usize),
    Delete,
}

/// Width of a colour dot and a button on the bars here.
const DOT: f32 = 26.0;
const SMALL: f32 = 30.0;

impl App {
    /// The lines and shapes of the board at `wi`, drawn under its notes.
    /// With the Select tool a shape is picked by a click, moved by a drag
    /// and typed in after a double click (or Enter).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn wb_shapes_layer(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        rect: Rect,
        origin: Pos2,
        view: WbView,
        wi: usize,
        space: bool,
        board_hovered: bool,
    ) {
        let z = view.zoom;
        let wid = self.data.extra.whiteboards[wi].id;
        let p = ui.painter_at(rect);
        let lw = (2.0 * z).max(1.0);
        let selecting = self.wb_tool == 2 && !space;
        let picked = self
            .sweep
            .wb_sel
            .filter(|s| s.0 == wid && self.wb_tool == 2)
            .map(|s| s.1);
        let hover = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|_| board_hovered);
        let to_screen = |q: &Pos2| view.to_screen(origin, [q.x, q.y]);
        {
            let wb = &self.data.extra.whiteboards[wi];
            for l in &wb.lines {
                let pts: Vec<Pos2> = line_path(l, &wb.shapes).iter().map(to_screen).collect();
                let near = hover.is_some_and(|h| dist_to_path(&pts, h) < 6.0);
                if picked == Some(WbSel::Line(l.id)) || (selecting && near) {
                    p.add(egui::Shape::line(
                        pts.clone(),
                        Stroke::new(lw + 6.0, t.primary.gamma_multiply(0.22)),
                    ));
                }
                if selecting && near {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                paint_line(&p, l, &pts, lw, rgb(l.color));
            }
        }
        // the line being dragged out with the Line tool, and the shape it
        // will be tied to
        if let Some(link) = self.sweep.wb_link.filter(|k| k.wid == wid) {
            if let Some(at) = ui.input(|i| i.pointer.interact_pos()) {
                let wb = &self.data.extra.whiteboards[wi];
                let b = view.to_board(origin, at);
                let to = shape_at(&wb.shapes, bpos(b)).filter(|&s| s != link.from);
                let style = self.sweep.wb_line_style;
                let l = WbLine {
                    from: link.from,
                    to: to.unwrap_or(0),
                    a: link.a,
                    b,
                    color: self.wb_color,
                    elbow: style.elbow,
                    dashed: style.dashed,
                    arrow: style.arrow,
                    ..Default::default()
                };
                for id in [link.from].into_iter().chain(to) {
                    if let Some(s) = wb.shapes.iter().find(|s| s.id == id) {
                        let r = Rect::from_min_size(
                            view.to_screen(origin, s.pos),
                            vec2(s.size[0], s.size[1]) * z,
                        );
                        p.rect_stroke(
                            r.expand(4.0),
                            4.0,
                            Stroke::new(1.5_f32, t.primary),
                            egui::StrokeKind::Middle,
                        );
                    }
                }
                // back over the shape it started from, letting go draws
                // nothing, so no line shows
                let home = link.from != 0 && shape_at(&wb.shapes, bpos(b)) == Some(link.from);
                if !home {
                    let pts: Vec<Pos2> = line_path(&l, &wb.shapes).iter().map(to_screen).collect();
                    paint_line(&p, &l, &pts, lw, rgb(l.color));
                }
            }
        }

        // (a shape a teammate took away meanwhile is not typed in)
        let mut editing = self.sweep.wb_edit.filter(|e| {
            e.0 == wid
                && self.data.extra.whiteboards[wi]
                    .shapes
                    .iter()
                    .any(|s| s.id == e.1)
        });
        let mut grab = None;
        let mut moved = None;
        let mut pick = None;
        let mut edit = None;
        let wb = &mut self.data.extra.whiteboards[wi];
        for (i, s) in wb.shapes.iter_mut().enumerate() {
            let r = Rect::from_min_size(
                view.to_screen(origin, s.pos),
                vec2(s.size[0], s.size[1]) * z,
            );
            if !r.intersects(rect) {
                continue;
            }
            paint_shape_body(&p, t, s, r, lw);
            let area = text_area(&s.kind, r, z);
            let font = shape_font(ui.ctx(), &s.kind, z);
            if let Some(e) = editing.as_mut().filter(|e| e.1 == s.id) {
                // typed in place, in the middle like the text it stands for
                let g = p.layout_job(centred(
                    if s.text.is_empty() { " " } else { &s.text },
                    font.clone(),
                    t.text,
                    area.width(),
                ));
                let er = Rect::from_center_size(area.center(), vec2(area.width(), g.size().y));
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(("wb_shape_edit", wid, s.id))
                        .max_rect(er),
                );
                child.set_clip_rect(r.expand(2.0).intersect(rect));
                let te = child.add(
                    egui::TextEdit::multiline(&mut s.text)
                        .id(egui::Id::new(("wb_shape_text", wid, s.id)))
                        .frame(false)
                        .font(font)
                        .text_color(t.text)
                        .horizontal_align(egui::Align::Center)
                        .margin(egui::Margin::ZERO)
                        .desired_width(area.width())
                        .desired_rows(1),
                );
                name_it(&te, "Shape text");
                if te.changed() {
                    self.dirty = true;
                }
                if e.2 {
                    te.request_focus();
                    e.2 = false;
                } else if !te.has_focus() {
                    // a click elsewhere (or Escape) ends the typing
                    e.1 = 0;
                }
            } else {
                if !s.text.is_empty() {
                    let g = p.layout_job(centred(&s.text, font, t.text, area.width()));
                    let y = (area.center().y - g.size().y / 2.0).max(r.top());
                    p.with_clip_rect(r.intersect(rect))
                        .galley(pos2(area.center().x, y), g, t.text);
                }
                if selecting {
                    let h = ui.interact(
                        r.intersect(rect),
                        ui.id().with(("wb_shape", wid, s.id)),
                        Sense::click_and_drag(),
                    );
                    let label = if s.text.is_empty() {
                        (i + 1).to_string()
                    } else {
                        s.text.clone()
                    };
                    name_it(&h, &format!("Shape {}", label));
                    if h.hovered() && !h.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
                    }
                    // only the left button moves it (the middle one moves
                    // the board)
                    let left = egui::PointerButton::Primary;
                    if h.drag_started_by(left) {
                        grab = Some(s.clone());
                    }
                    if h.dragged_by(left) {
                        moved = Some((i, h.drag_delta() / z));
                    }
                    if h.clicked() || h.drag_started_by(left) {
                        pick = Some(s.id);
                    }
                    if h.double_clicked() {
                        edit = Some(s.id);
                    }
                }
            }
        }
        if let Some((i, d)) = moved {
            wb.shapes[i].pos[0] += d.x;
            wb.shapes[i].pos[1] += d.y;
            self.dirty = true;
        }
        if let Some(old) = grab {
            self.sweep.wb_undo.push((wid, WbUndo::ShapeWas(old)));
        }
        self.sweep.wb_edit = editing.filter(|e| e.1 != 0);
        if let Some(id) = pick {
            self.sweep.wb_sel = Some((wid, WbSel::Shape(id)));
        }
        if let Some(id) = edit {
            self.sweep.wb_edit = Some((wid, id, true));
        }
    }

    /// With the Select tool, over everything else: the picked shape's
    /// frame, its corners (drag to resize), a + on each side (adds a shape
    /// joined to it) and the bar above it; or the picked line's bar and
    /// loose ends. Delete takes the pick away and Escape lets it go.
    pub(crate) fn wb_pick_layer(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        rect: Rect,
        origin: Pos2,
        view: WbView,
        wi: usize,
    ) {
        let wid = self.data.extra.whiteboards[wi].id;
        if self.wb_tool != 2 {
            return;
        }
        let Some(sel) = self.sweep.wb_sel.filter(|s| s.0 == wid).map(|s| s.1) else {
            return;
        };
        let wb = &self.data.extra.whiteboards[wi];
        let there = match sel {
            WbSel::Shape(id) => wb.shapes.iter().any(|s| s.id == id),
            WbSel::Line(id) => wb.lines.iter().any(|l| l.id == id),
        };
        // taken away meanwhile (Undo, or a teammate)
        if !there {
            self.sweep.wb_sel = None;
            return;
        }
        // the keys are for a dialog or menu open over the board (a Delete
        // there took the pick away out of sight)
        let over = self.modal.is_some() || ui.memory(|m| m.any_popup_open());
        // nor for a key that just ended typing: the Escape that ends typing
        // in a shape keeps it picked (the same key let it go too)
        let typed = self.focus_was.is_some();
        if !over && !typed && !ui.ctx().wants_keyboard_input() {
            let (del, esc, enter) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                    i.key_pressed(egui::Key::Escape),
                    i.key_pressed(egui::Key::Enter),
                )
            });
            if del {
                self.wb_delete_pick(wid);
                return;
            }
            if esc {
                self.sweep.wb_sel = None;
                return;
            }
            if let (true, WbSel::Shape(id)) = (enter, sel) {
                self.sweep.wb_edit = Some((wid, id, true));
            }
        }
        match sel {
            WbSel::Shape(id) => self.wb_shape_handles(ui, t, rect, origin, view, wi, id),
            WbSel::Line(id) => self.wb_line_handles(ui, t, rect, origin, view, wi, id),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn wb_shape_handles(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        rect: Rect,
        origin: Pos2,
        view: WbView,
        wi: usize,
        id: u64,
    ) {
        let wid = self.data.extra.whiteboards[wi].id;
        let z = view.zoom;
        // with Space down a drag on a handle moves the board
        let space = space_down(ui);
        let shapes = &self.data.extra.whiteboards[wi].shapes;
        let Some(si) = shapes.iter().position(|s| s.id == id) else {
            return;
        };
        let s = shapes[si].clone();
        let r = Rect::from_min_size(
            view.to_screen(origin, s.pos),
            vec2(s.size[0], s.size[1]) * z,
        );
        let p = ui.painter_at(rect);
        p.rect_stroke(
            r.expand(3.0),
            4.0,
            Stroke::new(1.5_f32, t.primary),
            egui::StrokeKind::Middle,
        );

        // a + on each side adds a shape like it there, joined by a line;
        // hovering shows where it goes
        let mut add = None;
        for (dir, name, off) in [
            (0, "Add shape above", vec2(0.0, -1.0)),
            (1, "Add shape right", vec2(1.0, 0.0)),
            (2, "Add shape below", vec2(0.0, 1.0)),
            (3, "Add shape left", vec2(-1.0, 0.0)),
        ] {
            let side = match dir {
                0 => r.center_top(),
                1 => r.right_center(),
                2 => r.center_bottom(),
                _ => r.left_center(),
            };
            let c = side + off * 24.0;
            let br = Rect::from_center_size(c, vec2(22.0, 22.0));
            if !rect.contains_rect(br) {
                continue;
            }
            let h = ui.interact(br, ui.id().with(("wb_grow", wid, id, dir)), Sense::click());
            name_it(&h, name);
            let hovered = h.hovered();
            if hovered {
                let spot = next_spot(&self.data.extra.whiteboards[wi].shapes, &s, dir);
                let sr = Rect::from_min_max(
                    view.to_screen(origin, [spot.min.x, spot.min.y]),
                    view.to_screen(origin, [spot.max.x, spot.max.y]),
                );
                let pts = [
                    sr.left_top(),
                    sr.right_top(),
                    sr.right_bottom(),
                    sr.left_bottom(),
                    sr.left_top(),
                ];
                p.extend(egui::Shape::dashed_line(
                    &pts,
                    Stroke::new(1.5_f32, t.primary),
                    6.0,
                    4.0,
                ));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            p.circle_filled(c, 10.0, if hovered { t.primary } else { t.bg });
            p.circle_stroke(c, 10.0, Stroke::new(1.5_f32, t.primary));
            paint_plus(&p, c, 11.0, if hovered { t.on_primary } else { t.primary });
            if h.on_hover_text(tr("Add a shape joined to this one"))
                .clicked()
            {
                add = Some(dir);
            }
        }

        // the corners resize it; the opposite corner stays put
        let mut resized = None;
        for (k, corner) in [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
        ]
        .into_iter()
        .enumerate()
        {
            let hr = Rect::from_center_size(corner, vec2(10.0, 10.0));
            if !rect.contains_rect(hr) || space {
                continue;
            }
            let h = ui.interact(
                hr.expand(3.0),
                ui.id().with(("wb_corner", wid, id, k)),
                Sense::drag(),
            );
            name_it(&h, &format!("Resize shape {}", k + 1));
            p.rect_filled(hr, 2.0, t.bg);
            p.rect_stroke(
                hr,
                2.0,
                Stroke::new(1.5_f32, t.primary),
                egui::StrokeKind::Middle,
            );
            if h.hovered() || h.dragged() {
                ui.ctx().set_cursor_icon(if k % 2 == 0 {
                    egui::CursorIcon::ResizeNwSe
                } else {
                    egui::CursorIcon::ResizeNeSw
                });
            }
            if h.drag_started_by(egui::PointerButton::Primary) {
                self.sweep.wb_undo.push((wid, WbUndo::ShapeWas(s.clone())));
            }
            if h.dragged_by(egui::PointerButton::Primary) {
                if let Some(at) = h.interact_pointer_pos() {
                    resized = Some((k, view.to_board(origin, at)));
                }
            }
        }
        if let Some((k, q)) = resized {
            let br = shape_rect(&s);
            let fixed = [
                br.right_bottom(),
                br.left_bottom(),
                br.left_top(),
                br.right_top(),
            ][k];
            let sx = if k == 0 || k == 3 { -1.0 } else { 1.0 };
            let sy = if k < 2 { -1.0 } else { 1.0 };
            let w = ((q[0] - fixed.x) * sx).max(MIN_SIZE[0]);
            let h = ((q[1] - fixed.y) * sy).max(MIN_SIZE[1]);
            let x0 = if sx < 0.0 { fixed.x - w } else { fixed.x };
            let y0 = if sy < 0.0 { fixed.y - h } else { fixed.y };
            let sh = &mut self.data.extra.whiteboards[wi].shapes[si];
            sh.pos = [x0, y0];
            sh.size = [w, h];
            self.dirty = true;
        }

        // the bar above it
        let mut toggles: Vec<(&str, &str, bool, usize)> = vec![];
        if s.kind != "text" {
            toggles.push(("Fill shape", "Fill", s.filled, 9));
        }
        let hit = self.wb_pick_bar(
            ui,
            t,
            rect,
            r,
            wid,
            s.color,
            &toggles,
            "Delete shape",
            |p, _, c, fg| {
                paint_fill_icon(p, c, fg, mix(t.bg, rgb(s.color), 0.28));
            },
        );
        match hit {
            Some(BarHit::Colour(c)) => self.wb_change_shape(wid, wi, id, |x| x.color = c),
            Some(BarHit::Toggle(_)) => self.wb_change_shape(wid, wi, id, |x| x.filled = !x.filled),
            Some(BarHit::Delete) => self.wb_delete_pick(wid),
            None => {}
        }
        if let Some(dir) = add {
            self.wb_grow(wid, wi, id, dir);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn wb_line_handles(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        rect: Rect,
        origin: Pos2,
        view: WbView,
        wi: usize,
        id: u64,
    ) {
        let wid = self.data.extra.whiteboards[wi].id;
        let space = space_down(ui);
        let wb = &self.data.extra.whiteboards[wi];
        let Some(li) = wb.lines.iter().position(|l| l.id == id) else {
            return;
        };
        let l = wb.lines[li].clone();
        let pts: Vec<Pos2> = line_path(&l, &wb.shapes)
            .iter()
            .map(|q| view.to_screen(origin, [q.x, q.y]))
            .collect();
        let p = ui.painter_at(rect);
        // an end tied to no shape can be dragged
        let mut moved = None;
        for (k, free) in [(0usize, l.from == 0), (1, l.to == 0)] {
            let c = if k == 0 { pts[0] } else { pts[pts.len() - 1] };
            let hr = Rect::from_center_size(c, vec2(14.0, 14.0));
            if !free || !rect.contains_rect(hr) || space {
                continue;
            }
            let h = ui.interact(hr, ui.id().with(("wb_line_end", wid, id, k)), Sense::drag());
            name_it(&h, &format!("Line end {}", k + 1));
            p.circle_filled(c, 5.0, t.bg);
            p.circle_stroke(c, 5.0, Stroke::new(1.5_f32, t.primary));
            if h.hovered() || h.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            if h.drag_started_by(egui::PointerButton::Primary) {
                self.sweep.wb_undo.push((wid, WbUndo::LineWas(l.clone())));
            }
            if h.dragged_by(egui::PointerButton::Primary) {
                if let Some(at) = h.interact_pointer_pos() {
                    moved = Some((k, view.to_board(origin, at)));
                }
            }
        }
        if let Some((k, q)) = moved {
            let ln = &mut self.data.extra.whiteboards[wi].lines[li];
            if k == 0 {
                ln.a = q;
            } else {
                ln.b = q;
            }
            self.dirty = true;
        }
        let bb = Rect::from_points(&pts);
        let toggles: Vec<(&str, &str, bool, usize)> = STYLE_TOGGLES
            .iter()
            .enumerate()
            .map(|(k, (name, tip))| (*name, *tip, [l.elbow, l.dashed, l.arrow][k], k))
            .collect();
        let hit = self.wb_pick_bar(
            ui,
            t,
            rect,
            bb,
            wid,
            l.color,
            &toggles,
            "Delete line",
            |p, k, c, fg| {
                paint_style_icon(p, k, c, fg);
            },
        );
        match hit {
            Some(BarHit::Colour(c)) => self.wb_change_line(wid, wi, id, move |x| x.color = c),
            Some(BarHit::Toggle(0)) => self.wb_change_line(wid, wi, id, |x| x.elbow = !x.elbow),
            Some(BarHit::Toggle(1)) => self.wb_change_line(wid, wi, id, |x| x.dashed = !x.dashed),
            Some(BarHit::Toggle(_)) => self.wb_change_line(wid, wi, id, |x| x.arrow = !x.arrow),
            Some(BarHit::Delete) => self.wb_delete_pick(wid),
            None => {}
        }
    }

    /// The bar over a pick (`over`, on screen): its colours, its toggles
    /// (name, tip, on, icon number for `icon`), then Delete.
    #[allow(clippy::too_many_arguments)]
    fn wb_pick_bar(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        rect: Rect,
        over: Rect,
        wid: u64,
        colour: [u8; 3],
        toggles: &[(&str, &str, bool, usize)],
        delete: &str,
        icon: impl Fn(&egui::Painter, usize, Pos2, Color32),
    ) -> Option<BarHit> {
        const H: f32 = 40.0;
        // a pick scrolled off the page takes its bar along (it stayed at
        // the page's edge, over nothing)
        if !over.intersects(rect) {
            return None;
        }
        let mut w = 8.0 + WB_COLORS.len() as f32 * DOT + 9.0;
        if !toggles.is_empty() {
            w += toggles.len() as f32 * SMALL + 9.0;
        }
        w += SMALL + 8.0;
        // above the + over the top; under the shape when that would run off
        // the page or under the name bar (top left) or the zoom bar (top
        // right), as wide as they can be
        let left = (over.center().x - w / 2.0).clamp(
            rect.left() + 8.0,
            (rect.right() - w - 8.0).max(rect.left() + 8.0),
        );
        let mut top = over.top() - 46.0 - H;
        let under_bars = top < rect.top() + 68.0
            && (left < rect.left() + 400.0 || left + w > rect.right() - 190.0);
        if top < rect.top() + 8.0 || under_bars {
            top = over.bottom() + 46.0;
        }
        // a pick taller than the page: over it, above the bar along the
        // bottom, rather than off the page (out of reach)
        let lowest = rect.bottom() - 16.0 - 48.0 - 12.0 - H;
        top = top.min(lowest).max(rect.top() + 8.0);
        let bar = Rect::from_min_size(pos2(left, top), vec2(w, H));
        card(ui, t, bar, 10.0, ui.id().with(("wb_pick_bar", wid)));
        self.sweep.wb_bars.push(bar);
        let p = ui.painter().clone();
        let cy = bar.center().y;
        let mut x = bar.left() + 8.0;
        let mut hit = None;
        for (i, c) in WB_COLORS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, cy - DOT / 2.0), vec2(DOT, DOT));
            let resp = ui.interact(r, ui.id().with(("wb_pick_colour", wid, i)), Sense::click());
            name_it(&resp, &format!("Pick colour {}", i + 1));
            p.circle_filled(r.center(), 8.0, rgb(*c));
            if *c == colour {
                p.circle_stroke(r.center(), 11.0, Stroke::new(2.0_f32, t.text));
            } else {
                hover_ring(
                    ui,
                    &resp,
                    Rect::from_center_size(r.center(), vec2(22.0, 22.0)),
                    11.0,
                );
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                hit = Some(BarHit::Colour(*c));
            }
            x += DOT;
        }
        bar_sep(&p, t, x + 4.5, cy);
        x += 9.0;
        if !toggles.is_empty() {
            for (k, (name, tip, on, ic)) in toggles.iter().enumerate() {
                let r = Rect::from_min_size(pos2(x, cy - SMALL / 2.0), vec2(SMALL, SMALL));
                let resp = small_button(
                    ui,
                    t,
                    r,
                    ui.id().with(("wb_pick_toggle", wid, k)),
                    name,
                    tr(tip),
                    *on,
                );
                icon(&p, *ic, r.center(), if *on { t.primary } else { t.text2 });
                if resp.clicked() {
                    hit = Some(BarHit::Toggle(k));
                }
                x += SMALL;
            }
            bar_sep(&p, t, x + 4.5, cy);
            x += 9.0;
        }
        let r = Rect::from_min_size(pos2(x, cy - SMALL / 2.0), vec2(SMALL, SMALL));
        let resp = small_button(
            ui,
            t,
            r,
            ui.id().with(("wb_pick_delete", wid)),
            delete,
            tr("Delete"),
            false,
        );
        line_icon(
            &p,
            "trash",
            r.center(),
            if resp.hovered() { t.danger } else { t.text2 },
        );
        if resp.clicked() {
            hit = Some(BarHit::Delete);
        }
        hit
    }

    /// Changes a shape, keeping how it was for Undo.
    fn wb_change_shape(&mut self, wid: u64, wi: usize, id: u64, f: impl FnOnce(&mut WbShape)) {
        if let Some(s) = self.data.extra.whiteboards[wi]
            .shapes
            .iter_mut()
            .find(|s| s.id == id)
        {
            self.sweep.wb_undo.push((wid, WbUndo::ShapeWas(s.clone())));
            f(s);
            self.dirty = true;
        }
    }

    /// Changes a line, keeping how it was for Undo.
    fn wb_change_line(&mut self, wid: u64, wi: usize, id: u64, f: impl FnOnce(&mut WbLine)) {
        if let Some(l) = self.data.extra.whiteboards[wi]
            .lines
            .iter_mut()
            .find(|l| l.id == id)
        {
            self.sweep.wb_undo.push((wid, WbUndo::LineWas(l.clone())));
            f(l);
            // what the next line drawn with the Line tool looks like
            self.sweep.wb_line_style = WbLineStyle {
                elbow: l.elbow,
                dashed: l.dashed,
                arrow: l.arrow,
            };
            self.dirty = true;
        }
    }

    /// The + on side `dir` of shape `id`: a shape like it (empty, not
    /// filled) a gap away, joined by a line like the ones already on it,
    /// picked and ready to type in.
    pub(crate) fn wb_grow(&mut self, wid: u64, wi: usize, id: u64, dir: usize) {
        let new_id = self.data.new_id();
        let line_id = self.data.new_id();
        let wb = &mut self.data.extra.whiteboards[wi];
        let Some(s) = wb.shapes.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        let spot = next_spot(&wb.shapes, &s, dir);
        let like = wb
            .lines
            .iter()
            .find(|l| l.from == id)
            .or_else(|| wb.lines.iter().find(|l| l.to == id))
            .cloned()
            .unwrap_or(WbLine {
                color: s.color,
                arrow: true,
                ..Default::default()
            });
        wb.shapes.push(WbShape {
            id: new_id,
            kind: s.kind.clone(),
            pos: [spot.min.x, spot.min.y],
            size: s.size,
            text: String::new(),
            color: s.color,
            filled: false,
        });
        let c = shape_rect(&s).center();
        wb.lines.push(WbLine {
            id: line_id,
            from: id,
            to: new_id,
            a: [c.x, c.y],
            b: [spot.center().x, spot.center().y],
            color: like.color,
            elbow: like.elbow,
            dashed: like.dashed,
            arrow: like.arrow,
        });
        self.sweep
            .wb_undo
            .push((wid, WbUndo::Added(vec![new_id], vec![line_id])));
        self.sweep.wb_sel = Some((wid, WbSel::Shape(new_id)));
        self.sweep.wb_edit = Some((wid, new_id, true));
        self.dirty = true;
    }

    /// The Shape tool's click: the chosen shape, centred where it was
    /// clicked, picked and ready to type in (the tool goes back to Select).
    pub(crate) fn wb_place_shape(&mut self, wid: u64, wi: usize, at: [f32; 2]) {
        let (kind, _, size) = SHAPE_KINDS[self.sweep.wb_kind.min(SHAPE_KINDS.len() - 1)];
        let id = self.data.new_id();
        self.data.extra.whiteboards[wi].shapes.push(WbShape {
            id,
            kind: kind.to_string(),
            pos: [at[0] - size[0] / 2.0, at[1] - size[1] / 2.0],
            size,
            text: String::new(),
            color: self.wb_color,
            filled: false,
        });
        self.sweep
            .wb_undo
            .push((wid, WbUndo::Added(vec![id], vec![])));
        self.wb_tool = 2;
        self.sweep.wb_sel = Some((wid, WbSel::Shape(id)));
        self.sweep.wb_edit = Some((wid, id, true));
        self.dirty = true;
    }

    /// The Line tool's drag: from the shape it starts on (or the board) to
    /// the shape it ends on (or where it ends).
    pub(crate) fn wb_link_drag(
        &mut self,
        resp: &Response,
        wid: u64,
        wi: usize,
        at: [f32; 2],
        press: Option<[f32; 2]>,
    ) {
        if resp.drag_started() {
            let a = press.unwrap_or(at);
            let from = shape_at(&self.data.extra.whiteboards[wi].shapes, bpos(a)).unwrap_or(0);
            self.sweep.wb_link = Some(WbLink { wid, from, a });
        }
        if !resp.drag_stopped() {
            return;
        }
        let Some(link) = self.sweep.wb_link.take().filter(|k| k.wid == wid) else {
            return;
        };
        let on = shape_at(&self.data.extra.whiteboards[wi].shapes, bpos(at));
        // let go on the shape it started from: no line (it made a short
        // one stuck inside the shape)
        if link.from != 0 && on == Some(link.from) {
            return;
        }
        let to = on.unwrap_or(0);
        if to == 0 && (bpos(at) - bpos(link.a)).length() < 12.0 {
            return;
        }
        let id = self.data.new_id();
        let style = self.sweep.wb_line_style;
        self.data.extra.whiteboards[wi].lines.push(WbLine {
            id,
            from: link.from,
            to,
            a: link.a,
            b: at,
            color: self.wb_color,
            elbow: style.elbow,
            dashed: style.dashed,
            arrow: style.arrow,
        });
        self.sweep
            .wb_undo
            .push((wid, WbUndo::Added(vec![], vec![id])));
        self.dirty = true;
    }

    /// A click on the board with the Select tool: picks the line under it,
    /// or lets the pick go.
    pub(crate) fn wb_click_board(
        &mut self,
        wid: u64,
        wi: usize,
        origin: Pos2,
        view: WbView,
        at: Pos2,
    ) {
        let wb = &self.data.extra.whiteboards[wi];
        let hit = wb.lines.iter().rev().find(|l| {
            let pts: Vec<Pos2> = line_path(l, &wb.shapes)
                .iter()
                .map(|q| view.to_screen(origin, [q.x, q.y]))
                .collect();
            dist_to_path(&pts, at) < 6.0
        });
        self.sweep.wb_sel = hit.map(|l| (wid, WbSel::Line(l.id)));
        self.sweep.wb_edit = None;
    }

    /// Deletes the pick: a shape takes the lines tied to it along. Undo
    /// brings them back.
    pub(crate) fn wb_delete_pick(&mut self, wid: u64) {
        let Some((_, sel)) = self.sweep.wb_sel.take().filter(|s| s.0 == wid) else {
            return;
        };
        self.sweep.wb_edit = None;
        let Some(wb) = self.data.extra.whiteboards.iter_mut().find(|w| w.id == wid) else {
            return;
        };
        let mut shapes = vec![];
        let mut lines = vec![];
        match sel {
            WbSel::Shape(id) => {
                if let Some(i) = wb.shapes.iter().position(|s| s.id == id) {
                    shapes.push((i, wb.shapes.remove(i)));
                }
                let mut i = 0;
                while i < wb.lines.len() {
                    if wb.lines[i].from == id || wb.lines[i].to == id {
                        lines.push((i, wb.lines.remove(i)));
                    } else {
                        i += 1;
                    }
                }
            }
            WbSel::Line(id) => {
                if let Some(i) = wb.lines.iter().position(|l| l.id == id) {
                    lines.push((i, wb.lines.remove(i)));
                }
            }
        }
        if !shapes.is_empty() || !lines.is_empty() {
            self.sweep
                .wb_undo
                .push((wid, WbUndo::Removed(shapes, lines)));
            self.dirty = true;
        }
    }
}

/// Takes back one of the shape and line steps on `wb`; true when anything
/// changed. By id, like the rest of the board's Undo: what a teammate
/// changed or took away meanwhile is left alone.
pub(crate) fn undo_shapes(wb: &mut Whiteboard, op: WbUndo) -> bool {
    match op {
        WbUndo::Added(shapes, lines) => {
            let (ns, nl) = (wb.shapes.len(), wb.lines.len());
            wb.shapes.retain(|s| !shapes.contains(&s.id));
            wb.lines.retain(|l| !lines.contains(&l.id));
            ns != wb.shapes.len() || nl != wb.lines.len()
        }
        WbUndo::Removed(shapes, lines) => {
            let mut back = false;
            // put back in the opposite order they were taken out, so each
            // lands where it was
            for (i, s) in shapes.into_iter().rev() {
                if !wb.shapes.iter().any(|x| x.id == s.id) {
                    wb.shapes.insert(i.min(wb.shapes.len()), s);
                    back = true;
                }
            }
            for (i, l) in lines.into_iter().rev() {
                if !wb.lines.iter().any(|x| x.id == l.id) {
                    wb.lines.insert(i.min(wb.lines.len()), l);
                    back = true;
                }
            }
            back
        }
        WbUndo::ShapeWas(old) => match wb.shapes.iter_mut().find(|s| s.id == old.id) {
            Some(s) => {
                // the text typed since stays
                let text = std::mem::take(&mut s.text);
                *s = WbShape { text, ..old };
                true
            }
            None => false,
        },
        WbUndo::LineWas(old) => match wb.lines.iter_mut().find(|l| l.id == old.id) {
            Some(l) => {
                *l = old;
                true
            }
            None => false,
        },
        WbUndo::NoteResized(id, size) => match wb.notes.iter_mut().find(|n| n.id == id) {
            Some(n) => {
                n.size = size;
                true
            }
            None => false,
        },
        _ => false,
    }
}
