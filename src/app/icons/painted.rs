// Icons the shell, the List and the Board paint from their own shapes: a
// checklist, a calendar, a person, a comment bubble, a flag and the view-tab
// icons.

use super::*;

// ------------------------------------------------------------------ icons

pub(crate) fn paint_checklist_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.3_f32, color);
    p.line_segment([c + vec2(-6.0, -3.5), c + vec2(-4.5, -2.0)], s);
    p.line_segment([c + vec2(-4.5, -2.0), c + vec2(-2.0, -5.0)], s);
    p.line_segment([c + vec2(0.0, -3.5), c + vec2(6.0, -3.5)], s);
    p.line_segment([c + vec2(-6.0, 3.0), c + vec2(6.0, 3.0)], s);
}

pub(crate) fn paint_calendar_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    let r = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(12.0, 11.0));
    p.rect_stroke(
        r,
        2.0,
        Stroke::new(1.2_f32, color),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [
            pos2(r.left(), r.top() + 3.5),
            pos2(r.right(), r.top() + 3.5),
        ],
        Stroke::new(1.2_f32, color),
    );
    p.line_segment(
        [
            pos2(r.left() + 3.0, r.top() - 1.5),
            pos2(r.left() + 3.0, r.top() + 1.0),
        ],
        Stroke::new(1.2_f32, color),
    );
    p.line_segment(
        [
            pos2(r.right() - 3.0, r.top() - 1.5),
            pos2(r.right() - 3.0, r.top() + 1.0),
        ],
        Stroke::new(1.2_f32, color),
    );
}

pub(crate) fn paint_person_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    p.circle_stroke(c + vec2(0.0, -3.0), 3.0, Stroke::new(1.2_f32, color));
    let mut pts = vec![];
    for i in 0..=10 {
        let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
        pts.push(c + vec2(0.0, 6.5) + vec2(a.cos() * 5.5, a.sin() * 4.0));
    }
    p.add(egui::Shape::line(pts, Stroke::new(1.2_f32, color)));
}

pub(crate) fn paint_comment_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    let r = Rect::from_center_size(c + vec2(0.0, -1.0), vec2(12.0, 9.0));
    p.rect_stroke(
        r,
        2.5,
        Stroke::new(1.2_f32, color),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [
            pos2(r.left() + 3.0, r.bottom()),
            pos2(r.left() + 2.0, r.bottom() + 3.0),
        ],
        Stroke::new(1.2_f32, color),
    );
}

pub(crate) fn paint_outline_flag(p: &egui::Painter, c: Pos2, color: Color32) {
    let top = c + vec2(-4.0, -6.0);
    p.line_segment([top, top + vec2(0.0, 12.0)], Stroke::new(1.2_f32, color));
    p.add(egui::Shape::closed_line(
        vec![
            top,
            top + vec2(8.5, 0.5),
            top + vec2(6.5, 3.5),
            top + vec2(8.5, 6.5),
            top + vec2(0.0, 6.5),
        ],
        Stroke::new(1.2_f32, color),
    ));
}

/// Coloured view icons as in the original's tabs.
pub(crate) fn paint_view_icon(p: &egui::Painter, c: Pos2, v: ViewKind) {
    let (col, glyph) = match v {
        ViewKind::Overview => {
            let r = Rect::from_center_size(c, vec2(14.0, 14.0));
            p.rect_filled(r, 3.0, Color32::from_rgb(100, 100, 100));
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                "i",
                FontId::proportional(10.0),
                Color32::WHITE,
            );
            return;
        }
        ViewKind::List => (Color32::from_rgb(131, 131, 131), 0),
        ViewKind::Board => (Color32::from_rgb(62, 99, 221), 1),
        ViewKind::Calendar => (Color32::from_rgb(247, 104, 8), 2),
        ViewKind::Gantt => (Color32::from_rgb(229, 72, 77), 3),
        ViewKind::Timeline => {
            // two staggered bars
            let r = Rect::from_center_size(c, vec2(14.0, 14.0));
            p.rect_filled(r, 3.0, Color32::from_rgb(124, 92, 230));
            let w = Stroke::new(2.0_f32, Color32::WHITE);
            p.line_segment(
                [
                    pos2(r.left() + 3.0, r.top() + 5.0),
                    pos2(r.left() + 9.0, r.top() + 5.0),
                ],
                w,
            );
            p.line_segment(
                [
                    pos2(r.left() + 6.0, r.top() + 9.5),
                    pos2(r.right() - 3.0, r.top() + 9.5),
                ],
                w,
            );
            return;
        }
        ViewKind::Table => (Color32::from_rgb(48, 164, 108), 4),
    };
    let r = Rect::from_center_size(c, vec2(14.0, 14.0));
    if glyph == 0 {
        p.rect_stroke(
            r.shrink(0.5),
            3.0,
            Stroke::new(1.2_f32, col),
            egui::StrokeKind::Inside,
        );
        for k in 0..3 {
            let y = r.top() + 4.0 + k as f32 * 3.0;
            p.line_segment(
                [pos2(r.left() + 3.5, y), pos2(r.right() - 3.5, y)],
                Stroke::new(1.1_f32, col),
            );
        }
        return;
    }
    p.rect_filled(r, 3.0, col);
    let w = Stroke::new(1.3_f32, Color32::WHITE);
    match glyph {
        1 => {
            p.line_segment(
                [
                    pos2(r.left() + 4.5, r.top() + 3.5),
                    pos2(r.left() + 4.5, r.bottom() - 3.5),
                ],
                w,
            );
            p.line_segment(
                [
                    pos2(r.right() - 4.5, r.top() + 3.5),
                    pos2(r.right() - 4.5, r.bottom() - 6.0),
                ],
                w,
            );
        }
        2 => {
            p.text(
                r.center() + vec2(0.0, 1.0),
                Align2::CENTER_CENTER,
                "31",
                FontId::proportional(7.5),
                Color32::WHITE,
            );
        }
        3 => {
            p.line_segment(
                [
                    pos2(r.left() + 3.0, r.top() + 4.5),
                    pos2(r.left() + 8.0, r.top() + 4.5),
                ],
                w,
            );
            p.line_segment(
                [
                    pos2(r.left() + 5.0, r.top() + 7.5),
                    pos2(r.right() - 3.0, r.top() + 7.5),
                ],
                w,
            );
            p.line_segment(
                [
                    pos2(r.left() + 3.0, r.top() + 10.5),
                    pos2(r.left() + 9.0, r.top() + 10.5),
                ],
                w,
            );
        }
        _ => {
            p.line_segment(
                [
                    pos2(r.left() + 3.0, r.center().y),
                    pos2(r.right() - 3.0, r.center().y),
                ],
                w,
            );
            p.line_segment(
                [
                    pos2(r.center().x, r.top() + 3.0),
                    pos2(r.center().x, r.bottom() - 3.0),
                ],
                w,
            );
        }
    }
}
