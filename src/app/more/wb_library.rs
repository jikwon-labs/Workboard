// The whiteboard list: the page shown with no board open (the templates and
// a card for each board with a small picture of it), the boards in the
// hub's sidebar, and filling a board from a template.

use super::wb_shapes::{line_path, paint_line, paint_shape_body, shape_rect, GAP};
use super::whiteboard::note_fill;
use super::*;
use egui::Pos2;

/// The templates: (name, what it is for, colour).
pub(super) const TEMPLATES: [(&str, &str, [u8; 3]); 5] = [
    (
        "Organizational Chart",
        "Visualize your team structure",
        [0xb6, 0x60, 0xe0],
    ),
    (
        "Action Plan",
        "Turn goals into actionable steps",
        [0x0f, 0x9d, 0x9f],
    ),
    (
        "Customer Journey Map",
        "Optimize every customer touchpoint",
        [0x10, 0x90, 0xe0],
    ),
    (
        "Flow Chart",
        "Structure and communicate processes and workflows",
        [0xee, 0x5e, 0x99],
    ),
    (
        "Root Cause Analysis",
        "Analyze issues using the Ishikawa Fishbone diagram",
        [0xe1, 0x6b, 0x16],
    ),
];

/// A shape a template lays out: (kind, text in English, top left, size,
/// filled). The text is made in the language of whoever makes the board.
type TplShape = (&'static str, &'static str, [f32; 2], [f32; 2], bool);

/// A line a template lays out: (from shape, to shape, where a loose end
/// sits, elbow, dashed, arrow). A shape is its place in the template's
/// list; `None` leaves that end loose at the point given.
type TplLine = (Option<usize>, Option<usize>, [f32; 2], bool, bool, bool);

/// What a template starts a board with: the picture on its card, as shapes
/// and lines that can be typed in, moved and added to. Shapes are a `GAP`
/// apart, so the + on a shape puts the next one in line with the others.
fn template_items(name: &str) -> (Vec<TplShape>, Vec<TplLine>) {
    const ORG: [f32; 2] = [160.0, 64.0];
    const STEP: [f32; 2] = [180.0, 110.0];
    const DOT: [f32; 2] = [130.0, 130.0];
    const LABEL: [f32; 2] = [150.0, 36.0];
    let free = [0.0, 0.0];
    match name {
        "Organizational Chart" => (
            vec![
                ("rect", "CEO", [260.0, 40.0], ORG, true),
                ("rect", "CTO", [40.0, 164.0], ORG, false),
                ("rect", "COO", [260.0, 164.0], ORG, false),
                ("rect", "CMO", [480.0, 164.0], ORG, false),
            ],
            (1..4)
                .map(|k| (Some(0), Some(k), free, true, false, false))
                .collect(),
        ),
        "Action Plan" => (
            vec![
                ("rect", "Goal", [40.0, 40.0], STEP, true),
                ("rect", "Step 1", [280.0, 40.0], STEP, false),
                ("rect", "Step 2", [520.0, 40.0], STEP, false),
                ("rect", "Step 3", [760.0, 40.0], STEP, false),
                (
                    "rect",
                    "Owner / due date",
                    [40.0, 210.0],
                    [420.0, 90.0],
                    false,
                ),
            ],
            vec![],
        ),
        "Customer Journey Map" => (
            // up and down like the ups and downs of the journey
            [
                "Awareness",
                "Consideration",
                "Purchase",
                "Retention",
                "Advocacy",
            ]
            .iter()
            .zip([240.0, 100.0, 268.0, 114.0, 142.0])
            .enumerate()
            .map(|(k, (txt, cy))| {
                let x = 40.0 + k as f32 * (DOT[0] + GAP);
                ("ellipse", *txt, [x, cy - DOT[1] / 2.0], DOT, k == 0)
            })
            .collect(),
            (0..4)
                .map(|k| (Some(k), Some(k + 1), free, false, true, false))
                .collect(),
        ),
        "Flow Chart" => (
            vec![
                ("pill", "Start", [40.0, 120.0], [150.0, 64.0], true),
                ("diamond", "Decision?", [250.0, 92.0], [150.0, 120.0], false),
                ("rect", "Step", [460.0, 120.0], [150.0, 64.0], false),
            ],
            vec![
                (Some(0), Some(1), free, false, false, true),
                (Some(1), Some(2), free, false, false, true),
            ],
        ),
        "Root Cause Analysis" => (
            // the problem at the fish's head, the causes along its bones
            vec![
                ("rect", "Problem", [640.0, 160.0], [170.0, 80.0], true),
                ("text", "People", [55.0, 34.0], LABEL, false),
                ("text", "Process", [295.0, 34.0], LABEL, false),
                ("text", "Tools", [55.0, 330.0], LABEL, false),
                ("text", "Environment", [295.0, 330.0], LABEL, false),
            ],
            vec![
                // the spine, from its tail to the head
                (None, Some(0), [20.0, 200.0], false, false, false),
                (Some(1), None, [250.0, 200.0], false, false, false),
                (Some(2), None, [490.0, 200.0], false, false, false),
                (Some(3), None, [250.0, 200.0], false, false, false),
                (Some(4), None, [490.0, 200.0], false, false, false),
            ],
        ),
        _ => (vec![], vec![]),
    }
}

/// `a` moved toward `b` by `k` (0 = a, 1 = b), both taken as opaque.
pub(super) fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// `text` on one line, cut with "…" to fit `max_w`.
fn elided(
    p: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: max_w,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('\u{2026}'),
    };
    p.layout_job(job)
}

/// A small drawing of what a template lays out, in its colour, around `c`.
fn paint_template(p: &egui::Painter, t: &Theme, name: &str, c: Pos2, col: Color32) {
    let s = Stroke::new(1.5_f32, col);
    let tint = mix(t.bg, col, 0.28);
    // the drawings are 128 x 64, measured from their top left corner
    let at = |x: f32, y: f32| c + vec2(x - 64.0, y - 32.0);
    let boxed = |x: f32, y: f32, w: f32, h: f32, r: f32, fill: Color32| {
        let b = Rect::from_min_size(at(x, y), vec2(w, h));
        p.rect_filled(b, r, fill);
        p.rect_stroke(b, r, s, egui::StrokeKind::Middle);
    };
    match name {
        "Organizational Chart" => {
            p.add(egui::Shape::line(vec![at(64.0, 20.0), at(64.0, 30.0)], s));
            p.add(egui::Shape::line(
                vec![
                    at(26.0, 40.0),
                    at(26.0, 30.0),
                    at(102.0, 30.0),
                    at(102.0, 40.0),
                ],
                s,
            ));
            p.line_segment([at(64.0, 30.0), at(64.0, 40.0)], s);
            boxed(50.0, 4.0, 28.0, 16.0, 3.0, tint);
            for x in [12.0, 50.0, 88.0] {
                boxed(x, 40.0, 28.0, 16.0, 3.0, t.bg);
            }
        }
        "Action Plan" => {
            boxed(4.0, 8.0, 24.0, 20.0, 3.0, tint);
            for x in [36.0, 68.0, 100.0] {
                boxed(x, 8.0, 24.0, 20.0, 3.0, t.bg);
            }
            boxed(4.0, 38.0, 56.0, 18.0, 3.0, t.bg);
        }
        "Customer Journey Map" => {
            let dots = [
                (12.0, 40.0),
                (38.0, 20.0),
                (62.0, 44.0),
                (88.0, 22.0),
                (116.0, 26.0),
            ];
            let line: Vec<Pos2> = dots.iter().map(|d| at(d.0, d.1)).collect();
            p.extend(egui::Shape::dashed_line(&line, s, 3.0, 3.0));
            for (i, d) in dots.iter().enumerate() {
                p.circle_filled(at(d.0, d.1), 6.0, if i == 0 { tint } else { t.bg });
                p.circle_stroke(at(d.0, d.1), 6.0, s);
            }
        }
        "Flow Chart" => {
            for x in [32.0, 78.0] {
                p.line_segment([at(x, 32.0), at(x + 17.0, 32.0)], s);
                p.line_segment([at(x + 17.0, 32.0), at(x + 13.0, 28.0)], s);
                p.line_segment([at(x + 17.0, 32.0), at(x + 13.0, 36.0)], s);
            }
            boxed(4.0, 22.0, 28.0, 20.0, 10.0, tint);
            let diamond = vec![
                at(64.0, 16.0),
                at(80.0, 32.0),
                at(64.0, 48.0),
                at(48.0, 32.0),
            ];
            p.add(egui::Shape::convex_polygon(diamond, t.bg, s));
            boxed(96.0, 22.0, 28.0, 20.0, 3.0, t.bg);
        }
        _ => {
            // a fishbone: the spine, four bones and the problem at its head
            p.line_segment([at(8.0, 32.0), at(94.0, 32.0)], s);
            for (x0, y0) in [(16.0, 12.0), (46.0, 12.0), (16.0, 52.0), (46.0, 52.0)] {
                p.line_segment([at(x0, y0), at(x0 + 20.0, 32.0)], s);
            }
            boxed(94.0, 20.0, 30.0, 24.0, 4.0, tint);
        }
    }
}

/// A small picture of a board in `area`: its shapes, lines, notes and
/// strokes, scaled to fit.
fn paint_thumbnail(p: &egui::Painter, t: &Theme, w: &Whiteboard, area: Rect) {
    let mut bb = Rect::NOTHING;
    for n in &w.notes {
        let wh = n.wh();
        bb = bb.union(Rect::from_min_size(
            pos2(n.pos[0], n.pos[1]),
            vec2(wh[0], wh[1]),
        ));
    }
    for s in &w.shapes {
        bb = bb.union(shape_rect(s));
    }
    for l in &w.lines {
        for q in line_path(l, &w.shapes) {
            bb.extend_with(q);
        }
    }
    for s in &w.strokes {
        for q in &s.points {
            bb.extend_with(pos2(q[0], q[1]));
        }
    }
    if !bb.is_finite() {
        return;
    }
    let bb = bb.expand(1.0);
    let scale = (area.width() / bb.width())
        .min(area.height() / bb.height())
        .min(0.6);
    let off = area.center().to_vec2() - bb.center().to_vec2() * scale;
    let to = |q: [f32; 2]| (vec2(q[0], q[1]) * scale + off).to_pos2();
    for s in &w.strokes {
        let pts: Vec<Pos2> = s.points.iter().map(|q| to(*q)).collect();
        p.add(egui::Shape::line(
            pts,
            Stroke::new((s.width * scale).max(1.0), rgb(s.color)),
        ));
    }
    for l in &w.lines {
        let pts: Vec<Pos2> = line_path(l, &w.shapes)
            .iter()
            .map(|q| to([q.x, q.y]))
            .collect();
        paint_line(p, l, &pts, 1.2, rgb(l.color));
    }
    for s in &w.shapes {
        let r = Rect::from_min_size(to(s.pos), vec2(s.size[0], s.size[1]) * scale);
        paint_shape_body(p, t, s, r, 1.2);
        if s.kind == "text" {
            // a label: a short bar where its words are
            p.hline(
                r.center().x - r.width() * 0.3..=r.center().x + r.width() * 0.3,
                r.center().y,
                Stroke::new(2.0_f32, t.text3),
            );
        }
    }
    for n in &w.notes {
        let wh = n.wh();
        let r = Rect::from_min_size(to(n.pos), vec2(wh[0], wh[1]) * scale);
        p.rect_filled(
            r.translate(vec2(0.0, 1.0)),
            2.0,
            Color32::from_black_alpha(18),
        );
        p.rect_filled(r, 2.0, note_fill(t, n.color));
    }
}

/// A card's frame: the page colour, a line round it that darkens on hover.
fn card_frame(ui: &Ui, t: &Theme, r: Rect, hovered: bool) {
    let p = ui.painter();
    if hovered {
        let shadow = egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(if t.dark { 60 } else { 18 }),
        };
        p.add(shadow.as_shape(r, 12.0));
    }
    p.rect_filled(r, 12.0, t.bg);
    p.rect_stroke(
        r,
        12.0,
        Stroke::new(
            1.0_f32,
            if hovered {
                crate::widgets::hover_line(ui)
            } else {
                t.border
            },
        ),
        egui::StrokeKind::Inside,
    );
}

/// The top part of a card, rounded only at the top, and the line under it.
fn card_top(p: &egui::Painter, t: &Theme, r: Rect, fill: Color32) {
    p.rect_filled(
        r.shrink(1.0),
        egui::CornerRadius {
            nw: 11,
            ne: 11,
            sw: 0,
            se: 0,
        },
        fill,
    );
    p.hline(r.x_range(), r.bottom(), Stroke::new(1.0_f32, t.border));
}

impl App {
    /// Lays a template's picture out on a board, and names the board after
    /// the template if its name is still the one it was made with.
    pub(crate) fn wb_fill_template(&mut self, wid: u64, name: &str) {
        let colour = TEMPLATES
            .iter()
            .find(|x| x.0 == name)
            .map_or(super::whiteboard::WB_COLORS[0], |x| x.2);
        let (shapes, lines) = template_items(name);
        let sids: Vec<u64> = shapes.iter().map(|_| self.data.new_id()).collect();
        let lids: Vec<u64> = lines.iter().map(|_| self.data.new_id()).collect();
        let Some(w) = self.data.extra.whiteboards.iter_mut().find(|w| w.id == wid) else {
            return;
        };
        if let Some(n) = self.sweep.wb_new.as_mut().filter(|n| n.id == wid) {
            if n.name == w.name {
                // made in the language of the person making it, like a new task's name
                w.name = tr(name).to_string();
                n.name = w.name.clone();
            }
        }
        let made: Vec<WbShape> = shapes
            .into_iter()
            .zip(&sids)
            .map(|((kind, txt, pos, size, filled), id)| WbShape {
                id: *id,
                kind: kind.to_string(),
                pos,
                size,
                text: tr(txt).to_string(),
                color: colour,
                filled,
            })
            .collect();
        let centre = |k: Option<usize>, at: [f32; 2]| {
            k.map_or(at, |k| {
                let c = shape_rect(&made[k]).center();
                [c.x, c.y]
            })
        };
        for ((from, to, at, elbow, dashed, arrow), id) in lines.into_iter().zip(lids.clone()) {
            w.lines.push(WbLine {
                id,
                from: from.map_or(0, |k| sids[k]),
                to: to.map_or(0, |k| sids[k]),
                a: centre(from, at),
                b: centre(to, at),
                color: colour,
                elbow,
                dashed,
                arrow,
            });
        }
        w.shapes.extend(made);
        // Undo takes it back, as anything else added to the board
        if !sids.is_empty() {
            self.sweep
                .wb_undo
                .push((wid, super::whiteboard::WbUndo::Added(sids, lids)));
        }
        self.dirty = true;
    }

    /// The page with no board open: a header with "New whiteboard", the
    /// templates, and a card for each board with a small picture of it.
    pub(crate) fn wb_library(&mut self, ui: &mut Ui) {
        let t = self.t();
        let filter = self.hub_filter.min(2);
        // the sidebar's filter: All, My or Favorites
        let shown: Vec<usize> = (0..self.data.extra.whiteboards.len())
            .filter(|&i| self.wb_in_filter(&self.data.extra.whiteboards[i], filter))
            .collect();
        let mut open = None;
        let mut star = None;
        let mut make: Option<&str> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let avail = ui.available_width();
                let w = (avail - 80.0).min(1120.0).max((avail - 32.0).min(240.0));
                let x0 = ui.max_rect().left() + ((avail - w) / 2.0).max(0.0);
                let row = |ui: &mut Ui, h: f32| {
                    let (r, _) = ui.allocate_exact_size(vec2(avail, h), Sense::hover());
                    Rect::from_min_size(pos2(x0, r.top()), vec2(w, h))
                };
                ui.add_space(28.0);

                // the title, and the one button that makes a board
                let head = row(ui, 40.0);
                ui.painter().text(
                    head.left_center(),
                    Align2::LEFT_CENTER,
                    tr(match filter {
                        1 => "My Whiteboards",
                        2 => "Favorites",
                        _ => "All Whiteboards",
                    }),
                    crate::app::bold_font(ui.ctx(), 22.0),
                    t.text,
                );
                let g = ui.painter().layout_no_wrap(
                    tr("New whiteboard").to_string(),
                    crate::app::bold_font(ui.ctx(), 14.0),
                    t.on_primary,
                );
                let br = Rect::from_min_size(
                    pos2(head.right() - g.size().x - 46.0, head.center().y - 18.0),
                    vec2(g.size().x + 46.0, 36.0),
                );
                let b = ui.interact(br, ui.id().with("wb_new_button"), Sense::click());
                name_it(&b, "New whiteboard");
                ui.painter().rect_filled(
                    br,
                    9.0,
                    if b.hovered() {
                        t.primary_hover
                    } else {
                        t.primary
                    },
                );
                crate::app::icons::paint_plus(
                    ui.painter(),
                    pos2(br.left() + 20.0, br.center().y),
                    15.0,
                    t.on_primary,
                );
                ui.painter().galley(
                    pos2(br.left() + 34.0, br.center().y - g.size().y / 2.0),
                    g,
                    t.on_primary,
                );
                if b.clicked() {
                    make = Some("Start from scratch");
                }
                ui.add_space(28.0);

                // templates
                let lr = row(ui, 22.0);
                ui.painter().text(
                    lr.left_center(),
                    Align2::LEFT_CENTER,
                    tr("Start from a template"),
                    crate::app::bold_font(ui.ctx(), 15.0),
                    t.text,
                );
                ui.add_space(12.0);
                let mut cards: Vec<(&str, &str, Option<[u8; 3]>)> =
                    vec![("Start from scratch", "A blank board", None)];
                cards.extend(TEMPLATES.iter().map(|(n, d, c)| (*n, *d, Some(*c))));
                let gap = 12.0;
                // all six side by side in a usual window
                let cols = (((w + gap) / (150.0 + gap)).floor() as usize).max(1);
                let cw = (w - gap * (cols - 1) as f32) / cols as f32;
                for chunk in cards.chunks(cols) {
                    let rr = row(ui, 168.0);
                    for (k, (name, desc, colour)) in chunk.iter().enumerate() {
                        let r = Rect::from_min_size(
                            pos2(rr.left() + k as f32 * (cw + gap), rr.top()),
                            vec2(cw, 168.0),
                        );
                        let resp = ui.interact(r, ui.id().with(("wb_tpl", *name)), Sense::click());
                        name_it(&resp, name);
                        card_frame(ui, &t, r, resp.hovered());
                        let top = Rect::from_min_size(r.min, vec2(cw, 88.0));
                        let p = ui.painter();
                        match colour {
                            Some(c) => {
                                card_top(p, &t, top, mix(t.bg, rgb(*c), 0.1));
                                paint_template(p, &t, name, top.center(), rgb(*c));
                            }
                            None => {
                                card_top(p, &t, top, t.column_bg);
                                let b = Rect::from_center_size(top.center(), vec2(40.0, 40.0));
                                let pts = [
                                    b.left_top(),
                                    b.right_top(),
                                    b.right_bottom(),
                                    b.left_bottom(),
                                    b.left_top(),
                                ];
                                p.extend(egui::Shape::dashed_line(
                                    &pts,
                                    Stroke::new(1.5_f32, t.text3),
                                    4.0,
                                    3.0,
                                ));
                                crate::app::icons::paint_plus(p, b.center(), 18.0, t.text2);
                            }
                        }
                        let title = elided(
                            p,
                            tr(name),
                            crate::app::bold_font(ui.ctx(), 14.0),
                            t.text,
                            cw - 28.0,
                        );
                        p.galley(pos2(r.left() + 14.0, top.bottom() + 14.0), title, t.text);
                        let mut job = egui::text::LayoutJob::simple(
                            tr(desc).to_string(),
                            FontId::proportional(12.0),
                            t.text2,
                            cw - 28.0,
                        );
                        job.wrap.max_rows = 2;
                        job.wrap.overflow_character = Some('\u{2026}');
                        let d = p.layout_job(job);
                        p.galley(pos2(r.left() + 14.0, top.bottom() + 36.0), d, t.text2);
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            make = Some(name);
                        }
                    }
                    ui.add_space(gap);
                }
                ui.add_space(24.0);

                // the boards
                let lr = row(ui, 22.0);
                ui.painter().text(
                    lr.left_center(),
                    Align2::LEFT_CENTER,
                    tr("Boards"),
                    crate::app::bold_font(ui.ctx(), 15.0),
                    t.text,
                );
                ui.add_space(12.0);
                if shown.is_empty() {
                    let er = row(ui, 20.0);
                    ui.painter().text(
                        er.left_center(),
                        Align2::LEFT_CENTER,
                        tr("Create a whiteboard to sketch ideas and add sticky notes."),
                        FontId::proportional(13.0),
                        t.text2,
                    );
                }
                let gap = 16.0;
                let cols = (((w + gap) / (232.0 + gap)).floor() as usize).max(1);
                let cw = (w - gap * (cols - 1) as f32) / cols as f32;
                for chunk in shown.chunks(cols) {
                    let rr = row(ui, 210.0);
                    for (k, &i) in chunk.iter().enumerate() {
                        let board = &self.data.extra.whiteboards[i];
                        let r = Rect::from_min_size(
                            pos2(rr.left() + k as f32 * (cw + gap), rr.top()),
                            vec2(cw, 210.0),
                        );
                        let resp =
                            ui.interact(r, ui.id().with(("wb_card", board.id)), Sense::click());
                        let title = if board.name.is_empty() {
                            tr("Untitled whiteboard").to_string()
                        } else {
                            board.name.clone()
                        };
                        name_it(&resp, &title);
                        card_frame(ui, &t, r, resp.hovered());
                        let thumb = Rect::from_min_size(r.min, vec2(cw, 148.0));
                        let p = ui.painter();
                        card_top(p, &t, thumb, t.column_bg);
                        let dots = p.with_clip_rect(thumb.shrink(2.0));
                        let mut y = thumb.top() + 10.0;
                        while y < thumb.bottom() {
                            let mut x = thumb.left() + 10.0;
                            while x < thumb.right() {
                                dots.circle_filled(pos2(x, y), 0.8, t.border);
                                x += 12.0;
                            }
                            y += 12.0;
                        }
                        paint_thumbnail(&dots, &t, board, thumb.shrink(18.0));
                        let name = elided(
                            p,
                            &title,
                            crate::app::bold_font(ui.ctx(), 14.0),
                            t.text,
                            cw - 64.0,
                        );
                        p.galley(pos2(r.left() + 14.0, thumb.bottom() + 12.0), name, t.text);
                        let meta_y = thumb.bottom() + 43.0;
                        let mut x = r.left() + 14.0;
                        if let Some(m) = self.data.member(board.owner) {
                            paint_avatar(p, pos2(x + 8.0, meta_y), 8.0, m, t.bg);
                            x += 22.0;
                        }
                        let meta = if !board.notes.is_empty() {
                            crate::trf!("{} notes", board.notes.len())
                        } else if !board.strokes.is_empty() {
                            tr("Drawing only").to_string()
                        } else {
                            tr("Empty").to_string()
                        };
                        p.text(
                            pos2(x, meta_y),
                            Align2::LEFT_CENTER,
                            meta,
                            FontId::proportional(12.0),
                            t.text2,
                        );
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        // the star sits on the card, over its click
                        let key = format!("wb:{}", board.id);
                        let starred = self.data.extra.starred.contains(&key);
                        let sr = Rect::from_center_size(
                            pos2(r.right() - 26.0, (thumb.bottom() + r.bottom()) / 2.0),
                            vec2(32.0, 32.0),
                        );
                        let s = ui.interact(
                            sr,
                            ui.id().with(("wb_card_star", board.id)),
                            Sense::click(),
                        );
                        name_it(&s, &format!("Star {}", key));
                        if s.hovered() {
                            ui.painter().rect_filled(sr, 8.0, t.hover);
                        }
                        ui.painter().text(
                            sr.center(),
                            Align2::CENTER_CENTER,
                            if starred { "\u{2605}" } else { "\u{2606}" },
                            FontId::proportional(17.0),
                            if starred { STAR_YELLOW } else { t.text2 },
                        );
                        let s = s.on_hover_text(if starred {
                            tr("Remove from Favorites")
                        } else {
                            tr("Add to Favorites")
                        });
                        if s.clicked() {
                            star = Some(key);
                        } else if resp.clicked() {
                            open = Some(board.id);
                        }
                    }
                    ui.add_space(gap);
                }
                ui.add_space(40.0);
            });
        if let Some(key) = star {
            if self.data.extra.starred.contains(&key) {
                self.data.extra.starred.retain(|s| s != &key);
            } else {
                self.data.extra.starred.push(key);
            }
            self.dirty = true;
        }
        if let Some(id) = open {
            self.wb_open = Some(id);
        }
        if let Some(name) = make {
            self.new_whiteboard();
            if name != "Start from scratch" {
                if let Some(id) = self.wb_open {
                    self.wb_fill_template(id, name);
                }
            }
        }
    }

    /// The hub sidebar's boards, under its "Boards" heading and + (the
    /// shared sidebar, hubs/side.rs): every board narrowed by the search
    /// `q`, the open one marked. A board just made lights up for a moment,
    /// so it is clear the click made it.
    pub(crate) fn wb_sidebar_boards(&mut self, ui: &mut Ui, t: &Theme, q: &str) {
        let now = ui.input(|i| i.time);
        let rows: Vec<(u64, String, Option<[u8; 3]>)> = self
            .data
            .extra
            .whiteboards
            .iter()
            .filter(|w| q.is_empty() || w.name.to_lowercase().contains(q))
            .map(|w| (w.id, w.name.clone(), w.notes.first().map(|n| n.color)))
            .collect();
        if rows.is_empty() {
            crate::app::hubs::side::side_empty(
                ui,
                t,
                if q.is_empty() {
                    tr("No whiteboards yet")
                } else {
                    tr("No whiteboard has that name")
                },
            );
        }
        let open = self.wb_open;
        let lit = self
            .sweep
            .wb_new
            .as_mut()
            .map(|n| (n.id, *n.since.get_or_insert(now)));
        let mut pick = None;
        for (id, name, colour) in rows {
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
            let title = if name.is_empty() {
                tr("Untitled whiteboard").to_string()
            } else {
                name
            };
            name_it(&resp, &title);
            let sel = open == Some(id);
            let mut bg = if sel {
                Some(t.row_selected)
            } else if resp.hovered() {
                Some(t.hover)
            } else {
                None
            };
            if let Some((_, since)) = lit.filter(|l| l.0 == id) {
                let k = ((now - since) / 1.4).clamp(0.0, 1.0) as f32;
                if k < 1.0 {
                    let base = bg.unwrap_or(t.sidebar);
                    bg = Some(mix(mix(t.sidebar, t.primary, 0.28), base, k * k));
                    ui.ctx().request_repaint();
                }
            }
            let p = ui.painter();
            if let Some(bg) = bg {
                p.rect_filled(r, 7.0, bg);
            }
            let sw = Rect::from_center_size(r.left_center() + vec2(16.0, 0.0), vec2(14.0, 14.0));
            p.rect_filled(sw, 3.5, colour.map_or(t.bg, |c| note_fill(t, c)));
            p.rect_stroke(
                sw,
                3.5,
                Stroke::new(1.0_f32, colour.map_or(t.border, rgb)),
                egui::StrokeKind::Inside,
            );
            let g = elided(
                p,
                &title,
                FontId::proportional(14.0),
                if sel { t.text } else { t.text2 },
                w - 46.0,
            );
            p.galley(
                pos2(r.left() + 34.0, r.center().y - g.size().y / 2.0),
                g,
                if sel { t.text } else { t.text2 },
            );
            if resp.clicked() {
                pick = Some(id);
            }
        }
        if let Some(id) = pick {
            self.wb_open = Some(id);
        }
    }
}
