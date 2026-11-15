// The window shell measured from the person's running ClickUp: black icon
// rail, light top bar with back/forward and search, the Home sidebar (with
// Channels and Direct Messages), the page header with coloured view tabs and
// pill toolbar, and the List and Board views. See MECHANICS.md.

use super::*;
use chrono::Datelike;
use egui::{Pos2, Response};

/// What is being dragged in the sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SideDrag {
    Space(u64),
    List(u64),
}

// ------------------------------------------------------------------ icons

pub(super) fn paint_dashed_circle(p: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let n = 10;
    for i in 0..n {
        let a0 = std::f32::consts::TAU * (i as f32 / n as f32);
        let a1 = a0 + std::f32::consts::TAU / n as f32 * 0.55;
        p.line_segment(
            [
                c + vec2(a0.cos(), a0.sin()) * r,
                c + vec2(a1.cos(), a1.sin()) * r,
            ],
            Stroke::new(1.4, color),
        );
    }
}

fn paint_checklist_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.3, color);
    p.line_segment([c + vec2(-6.0, -3.5), c + vec2(-4.5, -2.0)], s);
    p.line_segment([c + vec2(-4.5, -2.0), c + vec2(-2.0, -5.0)], s);
    p.line_segment([c + vec2(0.0, -3.5), c + vec2(6.0, -3.5)], s);
    p.line_segment([c + vec2(-6.0, 3.0), c + vec2(6.0, 3.0)], s);
}

fn paint_calendar_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    let r = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(12.0, 11.0));
    p.rect_stroke(r, 2.0, Stroke::new(1.2, color), egui::StrokeKind::Inside);
    p.line_segment(
        [
            pos2(r.left(), r.top() + 3.5),
            pos2(r.right(), r.top() + 3.5),
        ],
        Stroke::new(1.2, color),
    );
    p.line_segment(
        [
            pos2(r.left() + 3.0, r.top() - 1.5),
            pos2(r.left() + 3.0, r.top() + 1.0),
        ],
        Stroke::new(1.2, color),
    );
    p.line_segment(
        [
            pos2(r.right() - 3.0, r.top() - 1.5),
            pos2(r.right() - 3.0, r.top() + 1.0),
        ],
        Stroke::new(1.2, color),
    );
}

fn paint_person_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    p.circle_stroke(c + vec2(0.0, -3.0), 3.0, Stroke::new(1.2, color));
    let mut pts = vec![];
    for i in 0..=10 {
        let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
        pts.push(c + vec2(0.0, 6.5) + vec2(a.cos() * 5.5, a.sin() * 4.0));
    }
    p.add(egui::Shape::line(pts, Stroke::new(1.2, color)));
}

fn paint_comment_icon(p: &egui::Painter, c: Pos2, color: Color32) {
    let r = Rect::from_center_size(c + vec2(0.0, -1.0), vec2(12.0, 9.0));
    p.rect_stroke(r, 2.5, Stroke::new(1.2, color), egui::StrokeKind::Inside);
    p.line_segment(
        [
            pos2(r.left() + 3.0, r.bottom()),
            pos2(r.left() + 2.0, r.bottom() + 3.0),
        ],
        Stroke::new(1.2, color),
    );
}

fn paint_outline_flag(p: &egui::Painter, c: Pos2, color: Color32) {
    let top = c + vec2(-4.0, -6.0);
    p.line_segment([top, top + vec2(0.0, 12.0)], Stroke::new(1.2, color));
    p.add(egui::Shape::closed_line(
        vec![
            top,
            top + vec2(8.5, 0.5),
            top + vec2(6.5, 3.5),
            top + vec2(8.5, 6.5),
            top + vec2(0.0, 6.5),
        ],
        Stroke::new(1.2, color),
    ));
}

/// Coloured view icons as in the original's tabs.
fn paint_view_icon(p: &egui::Painter, c: Pos2, v: ViewKind) {
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
        ViewKind::Table => (Color32::from_rgb(48, 164, 108), 4),
    };
    let r = Rect::from_center_size(c, vec2(14.0, 14.0));
    if glyph == 0 {
        p.rect_stroke(
            r.shrink(0.5),
            3.0,
            Stroke::new(1.2, col),
            egui::StrokeKind::Inside,
        );
        for k in 0..3 {
            let y = r.top() + 4.0 + k as f32 * 3.0;
            p.line_segment(
                [pos2(r.left() + 3.5, y), pos2(r.right() - 3.5, y)],
                Stroke::new(1.1, col),
            );
        }
        return;
    }
    p.rect_filled(r, 3.0, col);
    let w = Stroke::new(1.3, Color32::WHITE);
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

/// A rounded pill button with an outline, like the original's toolbar.
fn pill(ui: &mut Ui, t: &Theme, text: &str, shown: &str, active: bool, accent: bool) -> Response {
    let font = FontId::proportional(13.0);
    let (fg, bg, stroke) = if accent {
        (t.accent_text, t.accent_bg, t.accent_bg)
    } else if active {
        (t.text, t.row_selected, t.border)
    } else {
        (t.text2, t.bg, t.border)
    };
    let g = ui.painter().layout_no_wrap(shown.to_string(), font, fg);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 26.0), Sense::click());
    name_it(&resp, text);
    let fill = if resp.hovered() && !accent {
        t.hover
    } else {
        bg
    };
    ui.painter().rect_filled(r, 13.0, fill);
    ui.painter()
        .rect_stroke(r, 13.0, Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    ui.painter().galley(r.center() - g.size() / 2.0, g, fg);
    resp
}

/// Thin outline icons in the style of the original (1.5 px strokes).
pub(super) fn line_icon(p: &egui::Painter, kind: &str, c: Pos2, color: Color32) -> bool {
    let s = Stroke::new(1.5, color);
    let line = |a: egui::Vec2, b: egui::Vec2| p.line_segment([c + a, c + b], s);
    match kind {
        "home" => {
            let pts = vec![
                c + vec2(-7.0, -1.0),
                c + vec2(0.0, -7.5),
                c + vec2(7.0, -1.0),
                c + vec2(7.0, 7.0),
                c + vec2(-7.0, 7.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-2.5, 7.0), vec2(-2.5, 2.5));
            line(vec2(-2.5, 2.5), vec2(2.5, 2.5));
            line(vec2(2.5, 2.5), vec2(2.5, 7.0));
        }
        "planner" => {
            let r = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(15.0, 14.0));
            p.rect_stroke(r, 3.0, s, egui::StrokeKind::Middle);
            line(vec2(-7.5, -3.0), vec2(7.5, -3.0));
            line(vec2(-3.5, -8.5), vec2(-3.5, -5.0));
            line(vec2(3.5, -8.5), vec2(3.5, -5.0));
            p.text(
                c + vec2(0.0, 2.8),
                Align2::CENTER_CENTER,
                today().day().to_string(),
                FontId::proportional(7.5),
                color,
            );
        }
        "ai" => {
            for k in 0..4 {
                let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                p.circle_stroke(c + d * 3.6, 3.6, s);
            }
            p.circle_filled(c, 1.6, color);
        }
        "teams" => {
            p.circle_stroke(c + vec2(-2.5, -3.0), 3.0, s);
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                arc.push(c + vec2(-2.5, 7.0) + vec2(a.cos() * 6.0, a.sin() * 5.0));
            }
            p.add(egui::Shape::line(arc, s));
            p.circle_stroke(c + vec2(4.5, -2.0), 2.4, s);
            line(vec2(4.5, 2.5), vec2(8.0, 6.5));
        }
        "more" => {
            for i in 0..3 {
                for j in 0..3 {
                    p.circle_filled(
                        c + vec2(-5.0 + 5.0 * i as f32, -5.0 + 5.0 * j as f32),
                        1.5,
                        color,
                    );
                }
            }
        }
        "chat" => {
            let r = Rect::from_center_size(c + vec2(0.0, -1.0), vec2(15.0, 11.0));
            p.rect_stroke(r, 3.5, s, egui::StrokeKind::Middle);
            line(vec2(-3.0, 4.5), vec2(-5.0, 8.0));
            line(vec2(-5.0, 8.0), vec2(0.0, 4.5));
        }
        "docs" => {
            let pts = vec![
                c + vec2(-5.5, -7.5),
                c + vec2(2.0, -7.5),
                c + vec2(5.5, -4.0),
                c + vec2(5.5, 7.5),
                c + vec2(-5.5, 7.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-2.5, 0.0), vec2(2.5, 0.0));
            line(vec2(-2.5, 3.5), vec2(2.5, 3.5));
        }
        "dashboards" => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(15.0, 15.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-3.0, 4.0), vec2(-3.0, 0.0));
            line(vec2(0.5, 4.0), vec2(0.5, -3.5));
            line(vec2(4.0, 4.0), vec2(4.0, 1.5));
        }
        "whiteboards" => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(15.0, 15.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            let pts = vec![
                c + vec2(-4.0, 2.0),
                c + vec2(-1.0, -2.0),
                c + vec2(1.5, 2.0),
                c + vec2(4.0, -2.5),
            ];
            p.add(egui::Shape::line(pts, s));
        }
        "forms" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, 0.5), vec2(13.0, 15.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-3.0, 1.0), vec2(-0.8, 3.2));
            line(vec2(-0.8, 3.2), vec2(3.5, -1.5));
            line(vec2(-2.5, -7.5), vec2(2.5, -7.5));
        }
        "goals" => {
            let pts = vec![
                c + vec2(-5.0, -7.0),
                c + vec2(5.0, -7.0),
                c + vec2(4.0, -1.0),
                c + vec2(0.0, 2.0),
                c + vec2(-4.0, -1.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(0.0, 2.0), vec2(0.0, 6.0));
            line(vec2(-3.5, 7.0), vec2(3.5, 7.0));
        }
        "timesheets" | "clock" => {
            p.circle_stroke(c + vec2(0.0, 1.0), 6.5, s);
            line(vec2(0.0, 1.0), vec2(0.0, -2.5));
            line(vec2(0.0, 1.0), vec2(2.5, 2.5));
            line(vec2(-2.0, -7.5), vec2(2.0, -7.5));
        }
        "upgrade" => {
            let pts = vec![
                c + vec2(0.0, -8.0),
                c + vec2(7.0, -1.0),
                c + vec2(3.0, -1.0),
                c + vec2(3.0, 6.0),
                c + vec2(-3.0, 6.0),
                c + vec2(-3.0, -1.0),
                c + vec2(-7.0, -1.0),
            ];
            p.add(egui::Shape::convex_polygon(
                pts,
                Color32::from_rgb(171, 140, 255),
                Stroke::NONE,
            ));
            p.rect_filled(
                Rect::from_center_size(c + vec2(0.0, 8.0), vec2(6.0, 1.6)),
                0.5,
                Color32::from_rgb(171, 140, 255),
            );
        }
        "check_plus" => {
            let mut arc = vec![];
            for i in 0..=22 {
                let a = std::f32::consts::TAU * (0.1 + 0.75 * i as f32 / 22.0);
                arc.push(c + vec2(a.cos(), a.sin()) * 7.0);
            }
            p.add(egui::Shape::line(arc, s));
            line(vec2(-3.0, 0.0), vec2(-0.8, 2.3));
            line(vec2(-0.8, 2.3), vec2(3.0, -2.0));
            line(vec2(5.5, -8.0), vec2(5.5, -3.0));
            line(vec2(3.0, -5.5), vec2(8.0, -5.5));
        }
        "video" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(-2.0, 0.0), vec2(12.0, 10.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            let pts = vec![
                c + vec2(4.0, -1.5),
                c + vec2(8.0, -4.0),
                c + vec2(8.0, 4.0),
                c + vec2(4.0, 1.5),
            ];
            p.add(egui::Shape::line(pts, s));
        }
        "mic" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, -2.5), vec2(6.0, 10.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (i as f32 / 10.0);
                arc.push(c + vec2(0.0, -1.0) + vec2(a.cos() * 5.5, a.sin() * 5.5));
            }
            p.add(egui::Shape::line(arc, s));
            line(vec2(0.0, 4.5), vec2(0.0, 7.5));
        }
        "phone" => {
            let pts = vec![
                c + vec2(-6.0, -6.0),
                c + vec2(-3.0, -7.0),
                c + vec2(-1.0, -3.0),
                c + vec2(-3.0, -1.5),
                c + vec2(1.5, 3.0),
                c + vec2(3.0, 1.0),
                c + vec2(7.0, 3.0),
                c + vec2(6.0, 6.0),
                c + vec2(3.0, 7.0),
                c + vec2(-6.5, -2.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
        }
        "robot" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, 1.5), vec2(13.0, 10.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            p.circle_filled(c + vec2(-2.5, 1.5), 1.2, color);
            p.circle_filled(c + vec2(2.5, 1.5), 1.2, color);
            line(vec2(0.0, -3.5), vec2(0.0, -6.5));
            p.circle_filled(c + vec2(0.0, -7.0), 1.2, color);
        }
        "edit" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(12.0, 12.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(0.0, 0.0), vec2(6.5, -6.5));
        }
        "share" => {
            p.circle_stroke(c + vec2(-1.5, -3.0), 3.0, s);
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                arc.push(c + vec2(-1.5, 7.0) + vec2(a.cos() * 6.0, a.sin() * 5.0));
            }
            p.add(egui::Shape::line(arc, s));
            p.circle_stroke(c + vec2(5.0, -2.0), 2.2, s);
        }
        "inbox" => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(15.0, 13.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            let pts = vec![
                c + vec2(-7.5, 1.0),
                c + vec2(-3.0, 1.0),
                c + vec2(-2.0, 3.0),
                c + vec2(2.0, 3.0),
                c + vec2(3.0, 1.0),
                c + vec2(7.5, 1.0),
            ];
            p.add(egui::Shape::line(pts, s));
        }
        "reply" => {
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.5 + 0.5 * i as f32 / 10.0);
                arc.push(c + vec2(1.0, 4.0) + vec2(a.cos() * 6.0, a.sin() * 6.0));
            }
            arc.insert(0, c + vec2(-6.0, -2.0));
            p.add(egui::Shape::line(arc, s));
            line(vec2(-6.0, -2.0), vec2(-3.0, -5.0));
            line(vec2(-6.0, -2.0), vec2(-3.0, 1.0));
        }
        "person" => {
            p.circle_stroke(c + vec2(0.0, -3.5), 3.2, s);
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                arc.push(c + vec2(0.0, 7.0) + vec2(a.cos() * 6.0, a.sin() * 5.0));
            }
            p.add(egui::Shape::line(arc, s));
        }
        "person_comment" => {
            p.circle_stroke(c + vec2(-2.0, -3.5), 3.0, s);
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                arc.push(c + vec2(-2.0, 7.0) + vec2(a.cos() * 5.5, a.sin() * 5.0));
            }
            p.add(egui::Shape::line(arc, s));
            p.rect_stroke(
                Rect::from_center_size(c + vec2(5.0, 3.0), vec2(5.0, 4.0)),
                1.0,
                Stroke::new(1.2, color),
                egui::StrokeKind::Middle,
            );
        }
        "megaphone" => {
            let pts = vec![
                c + vec2(-6.0, -2.0),
                c + vec2(3.0, -6.5),
                c + vec2(3.0, 6.5),
                c + vec2(-6.0, 2.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-4.0, 2.0), vec2(-2.5, 7.0));
            line(vec2(5.5, -2.0), vec2(5.5, 2.0));
        }
        "bolt" => {
            let pts = vec![
                c + vec2(1.5, -8.0),
                c + vec2(-5.0, 1.0),
                c + vec2(0.0, 1.0),
                c + vec2(-1.5, 8.0),
                c + vec2(5.0, -1.0),
                c + vec2(0.0, -1.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
        }
        "heart" => {
            let col = Color32::from_rgb(233, 61, 130);
            p.circle_filled(c + vec2(-3.0, -2.0), 3.6, Color32::from_rgb(171, 74, 186));
            p.circle_filled(c + vec2(3.0, -2.0), 3.6, col);
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + vec2(-6.4, -0.8),
                    c + vec2(6.4, -0.8),
                    c + vec2(0.0, 6.5),
                ],
                col,
                Stroke::NONE,
            ));
        }
        "dots" => {
            for k in -1..=1 {
                p.circle_filled(c + vec2(5.0 * k as f32, 0.0), 1.4, color);
            }
        }
        _ => return false,
    }
    true
}

/// `text` cut with an ellipsis so it is at most `max_w` wide in `font`.
pub(super) fn fit_text(p: &egui::Painter, text: &str, font: &FontId, max_w: f32) -> String {
    let w = |s: &str| {
        p.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x
    };
    if w(text) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let cand: String = chars[..mid]
            .iter()
            .collect::<String>()
            .trim_end()
            .to_string()
            + "\u{2026}";
        if w(&cand) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    chars[..lo]
        .iter()
        .collect::<String>()
        .trim_end()
        .to_string()
        + "\u{2026}"
}

pub(super) struct Cols {
    pub assignee: Rect,
    pub due: Rect,
    pub priority: Rect,
    pub status: Rect,
    pub comments: Rect,
    pub created: Rect,
    /// Extra property columns that are shown (Created by, Tags...).
    pub extra: Vec<(&'static str, Rect)>,
    /// Custom field columns that are shown, by field id.
    pub fields: Vec<(u64, Rect)>,
    /// Left edge of the leftmost shown column (the name ends before it).
    pub left: f32,
}

/// The List's `hidden_cols` key of a custom field column.
pub(super) fn field_col_key(id: u64) -> String {
    format!("field:{}", id)
}

/// Width of a custom field column in the List.
pub(super) const FIELD_COL_W: f32 = 130.0;

/// Column rectangles from the right; hidden columns get an empty rect.
pub(super) fn col_layout(row: Rect, hidden: &HashSet<String>, show_pr: bool) -> Cols {
    col_layout_with(row, hidden, show_pr, &[])
}

/// `col_layout` with the workspace's custom fields (by id): those not
/// hidden come right after the built-in columns, before the properties,
/// while the name keeps room to read.
pub(super) fn col_layout_with(
    row: Rect,
    hidden: &HashSet<String>,
    show_pr: bool,
    fields: &[u64],
) -> Cols {
    // In a narrow window the name keeps MIN_BUILTIN px; the built-in columns
    // that do not fit are dropped, least important first.
    const MIN_BUILTIN: f32 = 220.0;
    const BUILTIN: [(&str, f32); 6] = [
        ("Assignee", 100.0),
        ("Due date", 100.0),
        ("Priority", 100.0),
        ("Status", 130.0),
        ("Comments", 90.0),
        ("Date created", 110.0),
    ];
    let mut budget = row.width() - 40.0 - MIN_BUILTIN;
    let mut dropped: Vec<&str> = vec![];
    for (name, w) in BUILTIN {
        if hidden.contains(name) || (name == "Priority" && !show_pr) {
            continue;
        }
        if w <= budget {
            budget -= w;
        } else {
            budget = -1.0;
            dropped.push(name);
        }
    }
    let mut x = row.right() - 40.0;
    let mut take = |name: &str, w: f32| {
        let off =
            hidden.contains(name) || (name == "Priority" && !show_pr) || dropped.contains(&name);
        if off {
            return Rect::from_min_max(
                pos2(row.right(), row.top()),
                pos2(row.right(), row.bottom()),
            );
        }
        x -= w;
        Rect::from_min_max(pos2(x, row.top()), pos2(x + w, row.bottom()))
    };
    let comments = take("Comments", 90.0);
    let status = take("Status", 130.0);
    let priority = take("Priority", 100.0);
    let due = take("Due date", 100.0);
    let assignee = take("Assignee", 100.0);
    let created = take("Date created", 110.0);
    // extra property columns only while the name keeps room to read
    const MIN_NAME: f32 = 300.0;
    let mut field_cols = vec![];
    for id in fields {
        let w = FIELD_COL_W;
        if !hidden.contains(&field_col_key(*id)) && x - w >= row.left() + MIN_NAME {
            x -= w;
            field_cols.push((
                *id,
                Rect::from_min_max(pos2(x, row.top()), pos2(x + w, row.bottom())),
            ));
        }
    }
    let mut extra = vec![];
    for p in parity::PROPERTIES {
        let w = parity::prop_width(p);
        if !hidden.contains(p) && x - w >= row.left() + MIN_NAME {
            x -= w;
            extra.push((
                p,
                Rect::from_min_max(pos2(x, row.top()), pos2(x + w, row.bottom())),
            ));
        }
    }
    Cols {
        assignee,
        due,
        priority,
        status,
        comments,
        created,
        extra,
        fields: field_cols,
        left: x,
    }
}

impl App {
    // ---------------------------------------------------------------- history

    pub(super) fn track_history(&mut self) {
        let cur = self.nav_spot();
        match self.nav_last {
            // a page filling in its default item (Chat opening the first
            // channel) is not a move
            Some(last)
                if last != cur
                    && last.section == cur.section
                    && last.loc == cur.loc
                    && last.item.is_none() => {}
            Some(last) if last != cur => {
                if self.nav_skip {
                    self.nav_skip = false;
                } else {
                    self.nav_back.push(last);
                    self.nav_fwd.clear();
                }
            }
            _ => {}
        }
        self.nav_last = Some(cur);
    }

    pub(super) fn go_back(&mut self) {
        // pages deleted since are dropped from the history
        while let Some(prev) = self.nav_back.pop() {
            if !self.nav_spot_exists(&prev) {
                continue;
            }
            self.nav_fwd.push(self.nav_spot());
            self.show_nav_spot(prev);
            self.nav_skip = true;
            break;
        }
    }

    pub(super) fn go_forward(&mut self) {
        while let Some(next) = self.nav_fwd.pop() {
            if !self.nav_spot_exists(&next) {
                continue;
            }
            self.nav_back.push(self.nav_spot());
            self.show_nav_spot(next);
            self.nav_skip = true;
            break;
        }
    }

    // ---------------------------------------------------------------- top bar

    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("top")
            .exact_height(40.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.app_bg)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ctx, |ui| {
                let full = ui.max_rect();
                // empty header space moves the window (there is no title bar)
                let drag = ui.interact(
                    full.expand(6.0),
                    ui.id().with("drag_header"),
                    Sense::click_and_drag(),
                );
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                }
                // workspace picker
                let name = self.data.workspace.clone();
                let g = ui
                    .painter()
                    .layout_no_wrap(name, FontId::proportional(14.0), t.text);
                let left = if cfg!(target_os = "macos") { 74.0 } else { 4.0 };
                let wr = Rect::from_min_size(
                    pos2(full.left() + left, full.center().y - 14.0),
                    vec2(g.size().x + 58.0, 28.0),
                );
                let wresp = ui.interact(wr, ui.id().with("ws_pill"), Sense::click());
                name_it(&wresp, "Workspace menu");
                ui.painter().rect_filled(
                    wr,
                    7.0,
                    if wresp.hovered() { t.hover } else { t.sidebar },
                );
                let av =
                    Rect::from_center_size(wr.left_center() + vec2(15.0, 0.0), vec2(18.0, 18.0));
                ui.painter()
                    .rect_filled(av, 4.0, rgb(self.data.extra.workspace_color));
                let letter = self
                    .data
                    .workspace
                    .chars()
                    .next()
                    .unwrap_or('W')
                    .to_uppercase()
                    .to_string();
                ui.painter().text(
                    av.center(),
                    Align2::CENTER_CENTER,
                    letter,
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
                ui.painter().galley(
                    pos2(wr.left() + 30.0, wr.center().y - g.size().y / 2.0),
                    g,
                    t.text,
                );
                let cv = wr.right_center() - vec2(14.0, 1.0);
                ui.painter().line_segment(
                    [cv + vec2(-4.0, -2.0), cv + vec2(0.0, 2.0)],
                    Stroke::new(1.3, t.text2),
                );
                ui.painter().line_segment(
                    [cv + vec2(0.0, 2.0), cv + vec2(4.0, -2.0)],
                    Stroke::new(1.3, t.text2),
                );
                if self.force_menu.as_deref() == Some("workspace") {
                    let pid = ui.id().with("ws_menu");
                    ui.memory_mut(|m| m.open_popup(pid));
                }
                popup(ui, &wresp, ui.id().with("ws_menu"), 300.0, |ui| {
                    self.workspace_menu(ui)
                });
                // planner shortcut
                let cr = Rect::from_center_size(
                    pos2(wr.right() + 18.0, full.center().y),
                    vec2(28.0, 28.0),
                );
                let cresp = ui.interact(cr, ui.id().with("cal_btn"), Sense::click());
                name_it(&cresp, "Planner shortcut");
                if cresp.hovered() {
                    ui.painter().rect_filled(cr, 6.0, t.hover);
                }
                paint_calendar_icon(ui.painter(), cr.center(), t.text2);
                ui.painter().text(
                    cr.center() + vec2(0.0, 2.0),
                    Align2::CENTER_CENTER,
                    today().day().to_string(),
                    FontId::proportional(6.5),
                    t.text2,
                );
                if cresp.on_hover_text(tr("Planner")).clicked() {
                    self.section = Section::Planner;
                }
                // back / forward + search with AI Chats
                let sw = 287.0f32.min(full.width() * 0.25).max(220.0);
                let sr = Rect::from_center_size(full.center() + vec2(40.0, 0.0), vec2(sw, 28.0));
                for (i, (glyph, enabled, tip)) in [
                    ("\u{2190}", !self.nav_back.is_empty(), "Go back"),
                    ("\u{2192}", !self.nav_fwd.is_empty(), "Go forward"),
                ]
                .into_iter()
                .enumerate()
                {
                    let ar = Rect::from_center_size(
                        pos2(sr.left() - 40.0 + i as f32 * 24.0, full.center().y),
                        vec2(24.0, 24.0),
                    );
                    let a = ui.interact(ar, ui.id().with(("nav", i)), Sense::click());
                    name_it(&a, tip);
                    if a.hovered() && enabled {
                        ui.painter().rect_filled(ar, 5.0, t.hover);
                    }
                    ui.painter().text(
                        ar.center(),
                        Align2::CENTER_CENTER,
                        glyph,
                        FontId::proportional(15.0),
                        if enabled {
                            t.text2
                        } else {
                            Color32::from_rgb(190, 190, 190)
                        },
                    );
                    if a.on_hover_text(tr(tip)).clicked() && enabled {
                        if i == 0 {
                            self.go_back();
                        } else {
                            self.go_forward();
                        }
                    }
                }
                let ai_r = Rect::from_min_max(
                    pos2(sr.right() - 94.0, sr.top() + 4.0),
                    pos2(sr.right() - 4.0, sr.bottom() - 4.0),
                );
                let sresp = ui.interact(
                    Rect::from_min_max(sr.min, pos2(ai_r.left(), sr.bottom())),
                    ui.id().with("search_pill"),
                    Sense::click(),
                );
                name_it(&sresp, "Search");
                ui.painter()
                    .rect_filled(sr, 14.0, if sresp.hovered() { t.hover } else { t.bg });
                ui.painter().rect_stroke(
                    sr,
                    14.0,
                    Stroke::new(1.0, t.border),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    sr.left_center() + vec2(14.0, 0.0),
                    Align2::LEFT_CENTER,
                    "\u{1F50D}",
                    FontId::proportional(12.0),
                    t.text2,
                );
                let sg = ui.painter().layout_no_wrap(
                    tr("Search").into(),
                    FontId::proportional(14.0),
                    t.text3,
                );
                let sgw = sg.size().x;
                ui.painter().galley(
                    pos2(sr.left() + 32.0, sr.center().y - sg.size().y / 2.0),
                    sg,
                    t.text3,
                );
                ui.painter().text(
                    pos2(sr.left() + 38.0 + sgw, sr.center().y),
                    Align2::LEFT_CENTER,
                    "\u{2318}J",
                    FontId::proportional(11.5),
                    Color32::from_rgb(170, 170, 170),
                );
                if sresp.clicked() {
                    self.palette = Some(String::new());
                }
                let air = ui.interact(ai_r, ui.id().with("ai_chats"), Sense::click());
                name_it(&air, "AI Chats");
                ui.painter().rect_filled(
                    ai_r,
                    10.0,
                    if air.hovered() { t.hover } else { t.sidebar },
                );
                ui.painter().text(
                    ai_r.center() - vec2(8.0, 0.0),
                    Align2::CENTER_CENTER,
                    tr("AI Chats"),
                    FontId::proportional(13.0),
                    t.text2,
                );
                ui.painter().text(
                    ai_r.right_center() - vec2(10.0, 0.0),
                    Align2::CENTER_CENTER,
                    "\u{2728}",
                    FontId::proportional(11.0),
                    Color32::from_rgb(171, 74, 186),
                );
                if air.clicked() {
                    self.section = Section::Ai;
                }
                // right part: pinned personal tools, avatar menu
                let mut right = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_max(
                            pos2(full.right() - 480.0, full.top()),
                            full.max,
                        ))
                        .layout(Layout::right_to_left(Align::Center)),
                );
                let ui = &mut right;
                ui.spacing_mut().item_spacing.x = 2.0;
                if let Some(m) = self.data.member(self.data.me).cloned() {
                    let (r, resp) = ui.allocate_exact_size(vec2(44.0, 30.0), Sense::click());
                    name_it(&resp, "Profile menu");
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 15.0, t.hover);
                    }
                    let c = r.left_center() + vec2(15.0, 0.0);
                    paint_avatar(ui.painter(), c, 12.0, &m, t.app_bg);
                    let muted = self.data.extra.muted_until > now_ts();
                    ui.painter()
                        .circle_filled(c + vec2(9.0, 9.0), 4.0, t.app_bg);
                    ui.painter().circle_filled(
                        c + vec2(9.0, 9.0),
                        2.8,
                        if muted {
                            Color32::from_rgb(0xe1, 0x6b, 0x16)
                        } else {
                            Color32::from_rgb(48, 164, 108)
                        },
                    );
                    let cv = r.right_center() - vec2(8.0, 1.0);
                    ui.painter().line_segment(
                        [cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5)],
                        Stroke::new(1.2, t.text2),
                    );
                    ui.painter().line_segment(
                        [cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)],
                        Stroke::new(1.2, t.text2),
                    );
                    if self.force_menu.as_deref() == Some("avatar") {
                        let pid = ui.id().with("avatar_menu");
                        ui.memory_mut(|m| m.open_popup(pid));
                    }
                    popup(ui, &resp, ui.id().with("avatar_menu"), 270.0, |ui| {
                        self.avatar_menu(ui)
                    });
                }
                ui.add_space(6.0);
                let pinned = self.data.extra.pinned_tools.clone();
                for id in pinned.iter().rev() {
                    if let Some((tid, label, icon)) = super::menus::TOOLS.iter().find(|x| x.0 == id)
                    {
                        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                        name_it(&resp, label);
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, t.hover);
                        }
                        let kind = match *tid {
                            "create_task" => "check_plus",
                            "clip" => "video",
                            "stt" => "mic",
                            "track_time" => "clock",
                            "doc" => "docs",
                            "whiteboard" => "whiteboards",
                            "dashboard" => "dashboards",
                            "people" => "teams",
                            _ => "",
                        };
                        if !line_icon(ui.painter(), kind, r.center(), t.text2) {
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                *icon,
                                FontId::proportional(13.0),
                                t.text2,
                            );
                        }
                        if resp.on_hover_text(tr(label)).clicked() {
                            self.run_tool(tid);
                        }
                    }
                }
                let now = now_ts();
                if let Some(task) = self.data.tasks.iter().find(|x| self.data.is_my_timer(x)) {
                    let txt = format!(
                        "\u{23F9} {}  {}",
                        fmt_duration(self.data.my_tracked_secs(task, now)),
                        truncate(&task.name, 22)
                    );
                    let b = Tint(
                        egui::Button::new(RichText::new(txt).color(Color32::WHITE))
                            .fill(t.danger)
                            .corner_radius(14),
                    );
                    let id = task.id;
                    ui.add_space(8.0);
                    if ui.add(b).on_hover_text(tr("Stop timer")).clicked() {
                        self.actions.push(Action::ToggleTimer(id));
                    }
                    ctx.request_repaint_after(std::time::Duration::from_secs(1));
                }
            });
    }

    // ---------------------------------------------------------------- rail

    pub(super) fn rail(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::SidePanel::left("rail")
            .exact_width(58.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.app_bg)
                    .inner_margin(egui::Margin {
                        left: 6,
                        right: 0,
                        top: 0,
                        bottom: 6,
                    }),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let p = ui.painter().clone();
                if t.rail_top == t.rail_bottom {
                    p.rect_filled(rect, 8.0, t.rail_top);
                } else {
                    p.rect_filled(rect, 8.0, t.rail_bottom);
                    let mut mesh = egui::Mesh::default();
                    let m = rect.shrink2(vec2(0.0, 8.0));
                    mesh.colored_vertex(m.left_top(), t.rail_top);
                    mesh.colored_vertex(m.right_top(), t.rail_top);
                    mesh.colored_vertex(m.right_bottom(), t.rail_bottom);
                    mesh.colored_vertex(m.left_bottom(), t.rail_bottom);
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    p.rect_filled(
                        Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 16.0)),
                        8.0,
                        t.rail_top,
                    );
                    p.add(egui::Shape::mesh(mesh));
                }
                let cx = rect.center().x;
                let white = Color32::WHITE;
                let mut items: Vec<String> = vec!["Home".into()];
                items.extend(self.data.extra.rail_items.iter().cloned());
                let mut y = rect.top() + 4.0;
                let icon_of = |l: &str| -> &'static str {
                    match l {
                        "Home" => "\u{1F3E0}",
                        "Planner" => "\u{1F4C5}",
                        "AI" => "\u{2728}",
                        "Teams" => "\u{1F465}",
                        "Chat" => "\u{1F4AC}",
                        "Docs" => "\u{1F4C4}",
                        "Dashboards" => "\u{1F4CA}",
                        "Whiteboards" => "\u{1F3A8}",
                        "Forms" => "\u{1F4CB}",
                        "Goals" => "\u{1F3AF}",
                        "Timesheets" => "\u{23F1}",
                        "Clips" => "\u{1F3A5}",
                        "Apps" => "\u{25A6}",
                        "Spaces" => "\u{25CE}",
                        _ => "\u{2022}",
                    }
                };
                let section_of = |l: &str| -> Option<Section> {
                    match l {
                        "Home" => Some(Section::Home),
                        "Planner" => Some(Section::Planner),
                        "AI" => Some(Section::Ai),
                        "Teams" => Some(Section::Teams),
                        "Chat" => Some(Section::Chat),
                        "Docs" => Some(Section::Docs),
                        "Dashboards" => Some(Section::Dashboards),
                        "Whiteboards" => Some(Section::Whiteboards),
                        "Forms" => Some(Section::Forms),
                        "Goals" => Some(Section::Goals),
                        "Timesheets" => Some(Section::Timesheets),
                        _ => None,
                    }
                };
                let pinned_sections: Vec<Section> =
                    items.iter().filter_map(|l| section_of(l)).collect();
                for label in &items {
                    let sec = section_of(label);
                    let active = match sec {
                        Some(Section::Home) => {
                            matches!(
                                self.section,
                                Section::Home | Section::Posts | Section::Meetings
                            ) || (self.section == Section::Chat
                                && !pinned_sections.contains(&Section::Chat))
                        }
                        Some(s) => s == self.section,
                        None => false,
                    };
                    let ibox = Rect::from_center_size(pos2(cx, y + 22.0), vec2(32.0, 32.0));
                    let hit =
                        Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                    let resp =
                        ui.interact(hit, ui.id().with(("rail", label.as_str())), Sense::click());
                    name_it(&resp, &format!("Rail {}", label));
                    if active {
                        p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                    } else if resp.hovered() {
                        p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                    }
                    if !line_icon(&p, &label.to_lowercase(), ibox.center(), white) {
                        p.text(
                            ibox.center(),
                            Align2::CENTER_CENTER,
                            icon_of(label),
                            FontId::proportional(17.0),
                            white,
                        );
                    }
                    p.text(
                        pos2(cx, ibox.bottom() + 10.0),
                        Align2::CENTER_CENTER,
                        tr(label),
                        FontId::proportional(11.0),
                        white,
                    );
                    if resp.on_hover_text(tr(label)).clicked() {
                        self.open_app(label);
                        if label == "Home" {
                            self.section = Section::Home;
                        }
                    }
                    y += 62.0;
                }
                // an app opened from More stays in the bar while it's open
                if let Some(ts) = self.temp_app {
                    let label = match ts {
                        Section::Docs => "Docs",
                        Section::Dashboards => "Dashboards",
                        Section::Whiteboards => "Whiteboards",
                        Section::Forms => "Forms",
                        Section::Goals => "Goals",
                        Section::Timesheets => "Timesheets",
                        _ => "",
                    };
                    if !label.is_empty() && !pinned_sections.contains(&ts) {
                        let ibox = Rect::from_center_size(pos2(cx, y + 22.0), vec2(32.0, 32.0));
                        let hit =
                            Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                        let resp = ui.interact(hit, ui.id().with("rail_temp"), Sense::click());
                        let active = self.section == ts;
                        if active {
                            p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                        } else if resp.hovered() {
                            p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                        }
                        if !line_icon(&p, &label.to_lowercase(), ibox.center(), white) {
                            p.text(
                                ibox.center(),
                                Align2::CENTER_CENTER,
                                icon_of(label),
                                FontId::proportional(17.0),
                                white,
                            );
                        }
                        let shown = tr(label);
                        let short = if shown.chars().count() > 8 {
                            format!("{}..", shown.chars().take(7).collect::<String>())
                        } else {
                            shown.to_string()
                        };
                        p.text(
                            pos2(cx, ibox.bottom() + 10.0),
                            Align2::CENTER_CENTER,
                            short,
                            FontId::proportional(11.0),
                            white,
                        );
                        if resp.on_hover_text(tr(label)).clicked() {
                            self.section = ts;
                        }
                        y += 62.0;
                    }
                }
                // More
                let ibox = Rect::from_center_size(pos2(cx, y + 22.0), vec2(32.0, 32.0));
                let hit = Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                let resp = ui.interact(hit, ui.id().with("rail_more"), Sense::click());
                name_it(&resp, "Rail More");
                let more_active = matches!(
                    self.section,
                    Section::Docs
                        | Section::Dashboards
                        | Section::Timesheets
                        | Section::Whiteboards
                        | Section::Forms
                        | Section::Goals
                ) && !pinned_sections.contains(&self.section)
                    && self.temp_app != Some(self.section);
                if more_active {
                    p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                } else if resp.hovered() {
                    p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                }
                line_icon(&p, "more", ibox.center(), white);
                p.text(
                    pos2(cx, ibox.bottom() + 10.0),
                    Align2::CENTER_CENTER,
                    tr("More"),
                    FontId::proportional(11.0),
                    white,
                );
                if self.force_menu.as_deref() == Some("more") {
                    let pid = ui.id().with("more_pop");
                    ui.memory_mut(|m| m.open_popup(pid));
                }
                popup(ui, &resp, ui.id().with("more_pop"), 220.0, |ui| {
                    self.rail_more_menu(ui)
                });
                // bottom group: Invite (every 56 px); Labs, Upgrade and Install removed at the person's request
                let bottom: [(&str, &str); 1] = [("Invite", "")];
                let mut by = rect.bottom() - bottom.len() as f32 * 56.0 - 2.0;
                for (label, icon) in bottom {
                    let ibox = Rect::from_center_size(pos2(cx, by + 16.0), vec2(32.0, 32.0));
                    let hit = Rect::from_min_max(
                        pos2(rect.left(), by - 2.0),
                        pos2(rect.right(), by + 52.0),
                    );
                    let resp = ui.interact(hit, ui.id().with(("railb", label)), Sense::click());
                    name_it(&resp, &format!("Rail {}", label));
                    if resp.hovered() {
                        p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                    }
                    if label == "Invite" {
                        paint_person_icon(&p, ibox.center() + vec2(-1.0, -1.0), white);
                        p.text(
                            ibox.center() + vec2(6.0, 4.0),
                            Align2::CENTER_CENTER,
                            "+",
                            FontId::proportional(10.0),
                            white,
                        );
                    } else if !line_icon(&p, &label.to_lowercase(), ibox.center(), white) {
                        p.text(
                            ibox.center(),
                            Align2::CENTER_CENTER,
                            icon,
                            FontId::proportional(16.0),
                            white,
                        );
                    }
                    p.text(
                        pos2(cx, ibox.bottom() + 10.0),
                        Align2::CENTER_CENTER,
                        tr(label),
                        FontId::proportional(11.0),
                        white,
                    );
                    if resp.on_hover_text(tr(label)).clicked() {
                        self.modal = Some(Modal::Invite {
                            emails: String::new(),
                            role: "Member".into(),
                        });
                    }
                    by += 56.0;
                }
            });
    }

    // ---------------------------------------------------------------- sidebar

    pub(super) fn count_open(&self, list: u64) -> usize {
        self.data
            .tasks
            .iter()
            .filter(|t| {
                t.list_id == list && t.parent.is_none() && !t.archived && !self.data.is_closed(t)
            })
            .count()
    }

    fn side_row(
        &self,
        ui: &mut Ui,
        indent: f32,
        selected: bool,
        lead: impl FnOnce(&egui::Painter, Pos2),
        label: &str,
        sub: Option<&str>,
        trailing: Option<String>,
        muted: bool,
    ) -> Response {
        self.side_row_as(
            ui, indent, selected, lead, label, label, sub, trailing, muted,
        )
    }

    /// A sidebar row with a fixed English name (named by it for the tests,
    /// drawn in the interface language). Names people chose use `side_row`.
    fn side_row_tr(
        &self,
        ui: &mut Ui,
        indent: f32,
        selected: bool,
        lead: impl FnOnce(&egui::Painter, Pos2),
        label: &str,
        sub: Option<&str>,
        trailing: Option<String>,
        muted: bool,
    ) -> Response {
        self.side_row_as(
            ui,
            indent,
            selected,
            lead,
            label,
            tr(label),
            sub,
            trailing,
            muted,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn side_row_as(
        &self,
        ui: &mut Ui,
        indent: f32,
        selected: bool,
        lead: impl FnOnce(&egui::Painter, Pos2),
        label: &str,
        shown: &str,
        sub: Option<&str>,
        trailing: Option<String>,
        muted: bool,
    ) -> Response {
        let t = self.t();
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
        name_it(&resp, label);
        if selected {
            ui.painter().rect_filled(rect, 7.0, t.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 7.0, t.hover);
        }
        let x = rect.left() + 8.0 + indent;
        lead(ui.painter(), pos2(x + 8.0, rect.center().y));
        let col = if muted && !selected { t.text2 } else { t.text };
        let clip = Rect::from_min_max(
            pos2(x + 24.0, rect.top()),
            pos2(rect.right() - 28.0, rect.bottom()),
        );
        let g = ui
            .painter()
            .layout_no_wrap(shown.to_string(), FontId::proportional(14.0), col);
        let gw = g.size().x;
        ui.painter().with_clip_rect(clip).galley(
            pos2(x + 24.0, rect.center().y - g.size().y / 2.0),
            g,
            col,
        );
        if let Some(s) = sub {
            ui.painter().with_clip_rect(clip).text(
                pos2(x + 28.0 + gw, rect.center().y + 1.0),
                Align2::LEFT_CENTER,
                s,
                FontId::proportional(12.0),
                t.text3,
            );
        }
        if let Some(count) = trailing {
            ui.painter().text(
                rect.right_center() - vec2(12.0, 0.0),
                Align2::RIGHT_CENTER,
                count,
                FontId::proportional(12.0),
                t.text3,
            );
        }
        resp
    }

    fn add_row(&self, ui: &mut Ui, label: &str) -> Response {
        let t = self.t();
        let tc = t.text3;
        self.side_row_tr(
            ui,
            0.0,
            false,
            |p, c| {
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    "+",
                    FontId::proportional(16.0),
                    tc,
                );
            },
            label,
            None,
            None,
            true,
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    fn side_header(&self, ui: &mut Ui, title: &str, icons: &[(&str, &str)]) -> Vec<Response> {
        let t = self.t();
        ui.add_space(10.0);
        let w = ui.available_width();
        let (r, hresp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
        ui.painter().text(
            r.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            tr(title),
            FontId::proportional(13.0),
            t.text2,
        );
        let hovered = hresp.hovered() || ui.rect_contains_pointer(r);
        let mut out = vec![];
        let mut x = r.right() - 13.0;
        for (i, (glyph, tip)) in icons.iter().enumerate().rev() {
            let br = Rect::from_center_size(pos2(x, r.center().y), vec2(22.0, 22.0));
            let resp = ui.interact(br, ui.id().with(("shdr", title, i)), Sense::click());
            let boxed = *glyph == "+" && i == icons.len() - 1;
            if boxed {
                ui.painter().rect_filled(
                    br,
                    5.0,
                    if resp.hovered() {
                        t.border
                    } else {
                        t.row_selected
                    },
                );
            } else if resp.hovered() {
                ui.painter().rect_filled(br, 5.0, t.row_selected);
            }
            if boxed || hovered || resp.hovered() {
                ui.painter().text(
                    br.center(),
                    Align2::CENTER_CENTER,
                    *glyph,
                    FontId::proportional(if *glyph == "+" { 15.0 } else { 13.0 }),
                    t.text2,
                );
            }
            out.push(resp.on_hover_text(tr(tip)));
            x -= 26.0;
        }
        out.reverse();
        out
    }

    fn hidden(&self, item: &str) -> bool {
        self.data.extra.hidden_nav.iter().any(|h| h == item)
    }

    fn matches_search(&self, label: &str) -> bool {
        match &self.side_search {
            Some(q) if !q.trim().is_empty() => {
                // built-in rows are found by the name shown (Korean) as well as the English one
                let q = q.trim().to_lowercase();
                label.to_lowercase().contains(&q) || tr(label).to_lowercase().contains(&q)
            }
            _ => true,
        }
    }

    fn side_searching(&self) -> bool {
        self.side_search
            .as_ref()
            .is_some_and(|q| !q.trim().is_empty())
    }

    /// While the sidebar is searched, a space stays when its name or the name
    /// of one of its folders, lists or docs matches.
    fn space_matches_search(&self, s: &Space) -> bool {
        self.matches_search(&s.name)
            || s.folders.iter().any(|f| {
                self.matches_search(&f.name)
                    || f.lists
                        .iter()
                        .any(|l| !l.archived && self.matches_search(&l.name))
            })
            || s.lists
                .iter()
                .any(|l| !l.archived && self.matches_search(&l.name))
            || self.data.docs.iter().any(|d| {
                d.space == Some(s.id)
                    && d.parent.is_none()
                    && self.doc_shown(d)
                    && self.matches_search(&d.title)
            })
    }

    pub(super) fn sidebar(&mut self, ui: &mut Ui) {
        let t = self.t();
        if self.sidebar_collapsed {
            egui::SidePanel::left("sidebar_c")
                .exact_width(40.0)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(t.sidebar)
                        .corner_radius(egui::CornerRadius {
                            nw: 8,
                            ne: 0,
                            sw: 8,
                            se: 0,
                        })
                        .inner_margin(egui::Margin::same(6)),
                )
                .show_inside(ui, |ui| {
                    if ui
                        .add(ghost(egui::Button::new(
                            RichText::new("\u{00BB}").size(16.0),
                        )))
                        .on_hover_text(tr("Open sidebar"))
                        .clicked()
                    {
                        self.sidebar_collapsed = false;
                    }
                });
            return;
        }
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(255.0)
            .width_range(200.0..=400.0)
            .frame(
                egui::Frame::new()
                    .fill(t.sidebar)
                    .corner_radius(egui::CornerRadius {
                        nw: 8,
                        ne: 0,
                        sw: 8,
                        se: 0,
                    })
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show_inside(ui, |ui| {
                // header: Home, (hover) search / filter / close, + Create
                let w = ui.available_width();
                let (hr, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
                let hovered = ui.rect_contains_pointer(hr);
                ui.painter().text(
                    hr.left_center() + vec2(6.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr("Home"),
                    FontId::proportional(17.0),
                    t.text,
                );
                let create_r = Rect::from_min_size(
                    pos2(hr.right() - 80.0, hr.center().y - 14.0),
                    vec2(80.0, 28.0),
                );
                let cresp = ui.interact(create_r, ui.id().with("side_create"), Sense::click());
                name_it(&cresp, "Create");
                ui.painter().rect_filled(
                    create_r,
                    7.0,
                    if cresp.hovered() {
                        t.primary_hover
                    } else {
                        t.primary
                    },
                );
                ui.painter().text(
                    create_r.center(),
                    Align2::CENTER_CENTER,
                    tr("+  Create"),
                    FontId::proportional(14.0),
                    Color32::WHITE,
                );
                if self.force_menu.as_deref() == Some("create") {
                    let pid = ui.id().with("create_pop");
                    ui.memory_mut(|m| m.open_popup(pid));
                }
                popup(ui, &cresp, ui.id().with("create_pop"), 400.0, |ui| {
                    self.create_menu(ui)
                });
                let icons: [(&str, &str); 3] = [
                    ("\u{1F50D}", "Search your sidebar"),
                    ("\u{2261}", "Filter your sidebar"),
                    ("\u{00AB}", "Close sidebar"),
                ];
                for (i, (glyph, tip)) in icons.iter().enumerate() {
                    let br = Rect::from_center_size(
                        pos2(
                            create_r.left() - 90.0 + i as f32 * 29.0 + 14.0,
                            hr.center().y,
                        ),
                        vec2(26.0, 26.0),
                    );
                    let resp = ui.interact(br, ui.id().with(("side_ic", i)), Sense::click());
                    name_it(&resp, tip);
                    let active = (i == 0 && self.side_search.is_some())
                        || (i == 1 && self.side_filter.is_some());
                    if resp.hovered() || active {
                        ui.painter().rect_filled(br, 6.0, t.row_selected);
                    }
                    if hovered || active {
                        ui.painter().text(
                            br.center(),
                            Align2::CENTER_CENTER,
                            *glyph,
                            FontId::proportional(if i == 2 { 16.0 } else { 13.0 }),
                            t.text2,
                        );
                    }
                    if resp.on_hover_text(tr(tip)).clicked() {
                        match i {
                            0 => {
                                self.side_search = if self.side_search.is_some() {
                                    None
                                } else {
                                    Some(String::new())
                                };
                                self.side_search_focus = self.side_search.is_some();
                            }
                            1 => {
                                self.side_filter = if self.side_filter.is_some() {
                                    None
                                } else {
                                    Some("")
                                }
                            }
                            _ => self.sidebar_collapsed = true,
                        }
                    }
                }
                if let Some(mut q) = self.side_search.take() {
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut q)
                            .hint_text(tr("Search the sidebar"))
                            .desired_width(f32::INFINITY),
                    );
                    if std::mem::take(&mut self.side_search_focus) {
                        r.request_focus();
                    }
                    self.side_search = Some(q);
                }
                if let Some(f) = self.side_filter {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (key, label) in
                            [("space", "Space"), ("unread", "Unread"), ("dms", "DMs")]
                        {
                            let on = f == key;
                            let g = ui.painter().layout_no_wrap(
                                tr(label).to_string(),
                                FontId::proportional(13.0),
                                if on { Color32::WHITE } else { t.text2 },
                            );
                            let (r, resp) = ui
                                .allocate_exact_size(vec2(g.size().x + 20.0, 24.0), Sense::click());
                            name_it(&resp, &format!("Sidebar filter {}", label));
                            ui.painter().rect_filled(
                                r,
                                12.0,
                                if on {
                                    t.primary
                                } else if resp.hovered() {
                                    t.hover
                                } else {
                                    t.bg
                                },
                            );
                            ui.painter().rect_stroke(
                                r,
                                12.0,
                                Stroke::new(1.0, t.border),
                                egui::StrokeKind::Inside,
                            );
                            ui.painter().galley(
                                r.center() - g.size() / 2.0,
                                g,
                                if on { Color32::WHITE } else { t.text2 },
                            );
                            if resp.clicked() {
                                self.side_filter = Some(if on { "" } else { key });
                            }
                        }
                    });
                }
                ui.add_space(4.0);
                let filter = self.side_filter.unwrap_or("");
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let tc = t.text2;
                        let muted = self.data.extra.muted_until > now_ts();
                        if filter.is_empty() {
                            let inbox_n = if muted { 0 } else { self.inbox_badge() };
                            let nav: [(&str, &str); 9] = [
                                ("Inbox", "\u{1F4E5}"),
                                ("Replies", "\u{21A9}"),
                                ("Assigned Comments", "\u{1F4AC}"),
                                ("Posts", "\u{1F4E2}"),
                                ("Skills", "\u{26A1}"),
                                ("Artifacts", "\u{1F496}"),
                                ("Meetings", "\u{260E}"),
                                ("My Tasks", "\u{1F464}"),
                                ("More", "\u{22EF}"),
                            ];
                            for (label, icon) in nav {
                                if self.hidden(label) || !self.matches_search(label) {
                                    continue;
                                }
                                let sel = match label {
                                    "Inbox" => {
                                        self.section == Section::Home && self.loc == Location::Inbox
                                    }
                                    "Replies" => {
                                        self.section == Section::Home
                                            && self.loc == Location::Replies
                                    }
                                    "Assigned Comments" => {
                                        self.section == Section::Home
                                            && self.loc == Location::Assigned
                                    }
                                    "My Tasks" => {
                                        self.section == Section::Home
                                            && self.loc == Location::MyTasks
                                            && !self.my_tasks_open
                                    }
                                    "Posts" => self.section == Section::Posts,
                                    "Meetings" => self.section == Section::Meetings,
                                    _ => false,
                                };
                                let badge = if label == "Inbox" && inbox_n > 0 {
                                    Some(inbox_n.to_string())
                                } else {
                                    None
                                };
                                let kind = match label {
                                    "Inbox" => "inbox",
                                    "Replies" => "reply",
                                    "Assigned Comments" => "person_comment",
                                    "Posts" => "megaphone",
                                    "Skills" => "bolt",
                                    "Artifacts" => "heart",
                                    "Meetings" => "phone",
                                    "My Tasks" => "person",
                                    _ => "dots",
                                };
                                let r = self.side_row_tr(
                                    ui,
                                    0.0,
                                    sel,
                                    |p, c| {
                                        if !line_icon(p, kind, c, tc) {
                                            p.text(
                                                c,
                                                Align2::CENTER_CENTER,
                                                icon,
                                                FontId::proportional(13.0),
                                                tc,
                                            );
                                        }
                                    },
                                    label,
                                    None,
                                    badge,
                                    true,
                                );
                                match label {
                                    "More" => {
                                        popup(ui, &r, ui.id().with("side_more"), 220.0, |ui| {
                                            for (l, ic) in [
                                                ("Overview", "\u{1F3E0}"),
                                                ("Docs", "\u{1F4C4}"),
                                                ("Dashboards", "\u{1F4CA}"),
                                                ("Whiteboards", "\u{1F3A8}"),
                                                ("Forms", "\u{1F4CB}"),
                                                ("Goals", "\u{1F3AF}"),
                                                ("Timesheets", "\u{23F1}"),
                                                ("People", "\u{1F465}"),
                                            ] {
                                                if ui
                                                    .add(ghost(egui::Button::new(format!(
                                                        "{}  {}",
                                                        ic,
                                                        tr(l)
                                                    ))))
                                                    .clicked()
                                                {
                                                    match l {
                                                        "Overview" => self
                                                            .actions
                                                            .push(Action::Go(Location::Home)),
                                                        "People" => self.section = Section::People,
                                                        other => self.open_app(other),
                                                    }
                                                    close_popup(ui);
                                                }
                                            }
                                            ui.separator();
                                            if ui
                                                .add(ghost(egui::Button::new(tr(
                                                    "\u{25A6}  Customize your sidebar",
                                                ))))
                                                .clicked()
                                            {
                                                self.modal = Some(Modal::CustomizeSidebar);
                                                close_popup(ui);
                                            }
                                        });
                                    }
                                    "My Tasks" if self.my_tasks_open => {
                                        // its three items stay open, as in the original
                                        if r.clicked() {
                                            self.actions.push(Action::Go(Location::MyTasks));
                                        }
                                        let me = self.data.me;
                                        let td = self
                                            .data
                                            .tasks
                                            .iter()
                                            .filter(|x| {
                                                x.assignees.contains(&me)
                                                    && x.due.map(|d| d <= today()).unwrap_or(false)
                                                    && self.task_visible(x)
                                                    && !self.data.is_closed(x)
                                            })
                                            .count();
                                        let pl = self.personal_list_id();
                                        let subs: [(&str, Location, Option<String>); 3] = [
                                            ("Assigned to me", Location::MyTasks, None),
                                            (
                                                "Today & Overdue",
                                                Location::TodayOverdue,
                                                if td > 0 {
                                                    Some(format!("\u{2611} {}", td))
                                                } else {
                                                    None
                                                },
                                            ),
                                            (
                                                "Personal List",
                                                pl.map(Location::List).unwrap_or(Location::MyTasks),
                                                None,
                                            ),
                                        ];
                                        for (sl, loc, badge) in subs {
                                            let sel = self.section == Section::Home
                                                && self.loc == loc
                                                && !(sl == "Personal List" && pl.is_none());
                                            let mm = self.data.member(me).cloned();
                                            let bg = t.sidebar;
                                            let r2 = self.side_row_tr(
                                                ui,
                                                14.0,
                                                sel,
                                                |p, c| match sl {
                                                    "Assigned to me" => {
                                                        if let Some(m) = &mm {
                                                            paint_avatar(p, c, 8.0, m, bg);
                                                        }
                                                    }
                                                    "Today & Overdue" => {
                                                        paint_calendar_icon(p, c, tc)
                                                    }
                                                    _ => paint_person_icon(p, c, tc),
                                                },
                                                sl,
                                                None,
                                                badge,
                                                false,
                                            );
                                            if r2.clicked() {
                                                if sl == "Personal List" && pl.is_none() {
                                                    let id = self.create_personal_list();
                                                    self.actions
                                                        .push(Action::Go(Location::List(id)));
                                                } else {
                                                    self.actions.push(Action::Go(loc));
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        if r.clicked() {
                                            match label {
                                                "Inbox" => {
                                                    self.actions.push(Action::Go(Location::Inbox))
                                                }
                                                "Replies" => {
                                                    self.actions.push(Action::Go(Location::Replies))
                                                }
                                                "Assigned Comments" => self
                                                    .actions
                                                    .push(Action::Go(Location::Assigned)),
                                                "My Tasks" => {
                                                    self.my_tasks_open = !self.my_tasks_open;
                                                    self.actions
                                                        .push(Action::Go(Location::MyTasks));
                                                }
                                                "Posts" => self.section = Section::Posts,
                                                "Meetings" => self.section = Section::Meetings,
                                                "Skills" => {
                                                    self.modal =
                                                        Some(super::menus::unavailable("Skills"))
                                                }
                                                _ => {
                                                    self.modal =
                                                        Some(super::menus::unavailable("Artifacts"))
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ui.add_space(6.0);
                            let r = ui.available_rect_before_wrap();
                            ui.painter().hline(
                                r.left() + 6.0..=r.right() - 6.0,
                                r.top(),
                                Stroke::new(1.0, t.border),
                            );

                            if !self.hidden("AI Chats") {
                                let hs = self.side_header(
                                    ui,
                                    "AI Chats",
                                    &[("\u{22EF}", "More actions")],
                                );
                                if hs[0].clicked() {
                                    self.section = Section::Ai;
                                }
                                if self.add_row(ui, "Ask, Build, Create").clicked() {
                                    self.section = Section::Ai;
                                }
                            }

                            // (while searching, the heading only shows with a match under it)
                            let fav_tasks: Vec<(u64, String)> = self.favorite_tasks();
                            let fav_match = self
                                .data
                                .favorites
                                .iter()
                                .any(|f| self.matches_search(&self.data.location_name(*f)))
                                || fav_tasks.iter().any(|(_, n)| self.matches_search(n));
                            if fav_match && !self.hidden("Favorites") {
                                self.side_header(ui, "Favorites", &[]);
                                for f in self.data.favorites.clone() {
                                    let name = self.data.location_name(f);
                                    if !self.matches_search(&name) {
                                        continue;
                                    }
                                    let sel = self.section == Section::Home && self.loc == f;
                                    let r = self.side_row(
                                        ui,
                                        0.0,
                                        sel,
                                        |p, c| {
                                            p.text(
                                                c,
                                                Align2::CENTER_CENTER,
                                                "\u{2605}",
                                                FontId::proportional(12.0),
                                                Color32::from_rgb(0xf8, 0xae, 0x00),
                                            );
                                        },
                                        &name,
                                        None,
                                        None,
                                        false,
                                    );
                                    if r.clicked() {
                                        self.actions.push(Action::Go(f));
                                    }
                                    // the same menu as the item has where it lives
                                    r.context_menu(|ui| match f {
                                        Location::List(id) => self.list_menu(ui, id),
                                        Location::Folder(id) => self.folder_menu(ui, id),
                                        Location::Space(id) => self.space_menu(ui, id),
                                        _ => {
                                            if ui.button(tr("Remove from Favorites")).clicked() {
                                                self.actions.push(Action::ToggleFavorite(f));
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                }
                                for (tid, name) in fav_tasks {
                                    if !self.matches_search(&name) {
                                        continue;
                                    }
                                    let sel = self.open_task == Some(tid);
                                    let r = self.side_row(
                                        ui,
                                        0.0,
                                        sel,
                                        |p, c| {
                                            p.text(
                                                c,
                                                Align2::CENTER_CENTER,
                                                "\u{2605}",
                                                FontId::proportional(12.0),
                                                Color32::from_rgb(0xf8, 0xae, 0x00),
                                            );
                                        },
                                        &name,
                                        None,
                                        None,
                                        false,
                                    );
                                    if r.clicked() {
                                        self.actions.push(Action::Open(tid));
                                    }
                                    r.context_menu(|ui| self.task_menu(ui, tid));
                                }
                            }
                        }

                        let ws = self.data.workspace.clone();
                        if filter.is_empty() || filter == "space" {
                            let hs = self.side_header(
                                ui,
                                "Spaces",
                                &[("\u{22EF}", "Spaces settings"), ("+", "Create Space")],
                            );
                            popup(ui, &hs[0], ui.id().with("spaces_settings"), 200.0, |ui| {
                                if ui.button(tr("Create Space")).clicked() {
                                    self.modal = Some(Modal::NewSpace {
                                        name: String::new(),
                                        color: STATUS_PALETTE
                                            [1 + (self.data.spaces.len() * 3) % 13],
                                    });
                                    close_popup(ui);
                                }
                                if ui.button(tr("Expand all")).clicked() {
                                    for s in &mut self.data.spaces {
                                        s.expanded = true;
                                    }
                                    self.dirty = true;
                                    close_popup(ui);
                                }
                                if ui.button(tr("Collapse all")).clicked() {
                                    for s in &mut self.data.spaces {
                                        s.expanded = false;
                                    }
                                    self.dirty = true;
                                    close_popup(ui);
                                }
                            });
                            if hs[1].clicked() {
                                self.modal = Some(Modal::NewSpace {
                                    name: String::new(),
                                    color: STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13],
                                });
                            }
                            let sub = format!("- {}", ws);
                            if self.matches_search("All Tasks") {
                                let sel = self.section == Section::Home
                                    && self.loc == Location::Everything;
                                let r = self.side_row_tr(
                                    ui,
                                    0.0,
                                    sel,
                                    |p, c| {
                                        p.rect_stroke(
                                            Rect::from_center_size(c, vec2(16.0, 16.0)),
                                            4.0,
                                            Stroke::new(1.0, tc),
                                            egui::StrokeKind::Inside,
                                        );
                                        p.text(
                                            c,
                                            Align2::CENTER_CENTER,
                                            "\u{2731}",
                                            FontId::proportional(9.0),
                                            tc,
                                        );
                                    },
                                    "All Tasks",
                                    Some(&sub),
                                    None,
                                    false,
                                );
                                if r.clicked() {
                                    self.actions.push(Action::Go(Location::Everything));
                                }
                            }
                            let spaces: Vec<Space> = self
                                .data
                                .spaces
                                .iter()
                                .filter(|s| s.private_owner.is_none() && !s.archived && !s.hidden)
                                .cloned()
                                .collect();
                            for s in &spaces {
                                if !self.space_matches_search(s) {
                                    continue;
                                }
                                self.space_rows(ui, s);
                            }
                            if self.add_row(ui, "New Space").clicked() {
                                self.modal = Some(Modal::NewSpace {
                                    name: String::new(),
                                    color: STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13],
                                });
                            }
                        }

                        // Channels
                        if (filter.is_empty() || filter == "unread") && !self.hidden("Channels") {
                            let hs = self.side_header(
                                ui,
                                "Channels",
                                &[("\u{1F516}", "Saved messages"), ("+", "Create a Channel")],
                            );
                            name_it(&hs[0], "Saved messages");
                            if hs[0].clicked() {
                                self.modal = Some(Modal::SavedMessages);
                            }
                            if hs[1].clicked() {
                                self.modal = Some(Modal::NewChannel {
                                    name: String::new(),
                                    private: false,
                                    add_list: false,
                                });
                            }
                            // the Unread filter uses the real count; muting only hides the badge
                            let chans: Vec<(u64, String, usize)> = self
                                .data
                                .channels
                                .iter()
                                .filter(|c| c.kind == ChannelKind::Channel && self.listed(c))
                                .map(|c| (c.id, c.name.clone(), self.unread_count(c)))
                                .collect();
                            for (id, name, real_unread) in chans {
                                if (filter == "unread" && real_unread == 0)
                                    || !self.matches_search(&name)
                                {
                                    continue;
                                }
                                let unread = if muted { 0 } else { real_unread };
                                let sel =
                                    self.section == Section::Chat && self.chat_open == Some(id);
                                let sub = if name == "general" {
                                    Some(format!("- {}", ws))
                                } else {
                                    None
                                };
                                let r = self.side_row(
                                    ui,
                                    0.0,
                                    sel,
                                    |p, c| {
                                        p.text(
                                            c,
                                            Align2::CENTER_CENTER,
                                            "#",
                                            FontId::proportional(15.0),
                                            tc,
                                        );
                                    },
                                    &name,
                                    sub.as_deref(),
                                    if unread > 0 {
                                        Some(unread.to_string())
                                    } else {
                                        None
                                    },
                                    false,
                                );
                                let sr = Rect::from_center_size(
                                    r.rect.right_center() - vec2(14.0, 0.0),
                                    vec2(20.0, 20.0),
                                );
                                let sresp =
                                    ui.interact(sr, ui.id().with(("chset", id)), Sense::click());
                                if r.hovered() || sresp.hovered() {
                                    ui.painter().rect_filled(
                                        sr,
                                        5.0,
                                        if sresp.hovered() {
                                            t.border
                                        } else {
                                            t.row_selected
                                        },
                                    );
                                    ui.painter().text(
                                        sr.center(),
                                        Align2::CENTER_CENTER,
                                        "\u{22EF}",
                                        FontId::proportional(13.0),
                                        t.text2,
                                    );
                                }
                                if sresp.on_hover_text(tr("Channel Settings")).clicked() {
                                    self.modal = Some(Modal::ChannelSettings(id));
                                } else if r.clicked() {
                                    self.section = Section::Chat;
                                    self.chat_open = Some(id);
                                    self.thread_open = None;
                                }
                            }
                            if let Some(mut name) = self.new_channel.take() {
                                let mut keep = true;
                                ui.horizontal(|ui| {
                                    ui.add_space(8.0);
                                    let r = ui.add(
                                        egui::TextEdit::singleline(&mut name)
                                            .id(egui::Id::new("new_channel_input"))
                                            .hint_text(tr("channel-name"))
                                            .desired_width(170.0),
                                    );
                                    // focus once when the field appears, not every frame,
                                    // and bring it into view: it sits at the end of the
                                    // channel list, often below the bottom of the sidebar
                                    // (for a few passes: egui can discard the first one)
                                    let nr = ui.ctx().cumulative_pass_nr();
                                    let shown_id = r.id.with("appeared");
                                    if focus_once(ui, &r) {
                                        ui.data_mut(|d| d.insert_temp(shown_id, nr));
                                    }
                                    let appeared: u64 =
                                        ui.data(|d| d.get_temp(shown_id)).unwrap_or(0);
                                    if nr <= appeared + 3 && !ui.clip_rect().contains_rect(r.rect) {
                                        r.scroll_to_me(Some(Align::Center));
                                    }
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        r.surrender_focus();
                                        keep = false;
                                    } else if r.lost_focus() {
                                        if ui.input(|i| i.key_pressed(egui::Key::Enter))
                                            && !name.trim().is_empty()
                                        {
                                            let id = self.data.new_id();
                                            let clean =
                                                name.trim().to_lowercase().replace(' ', "-");
                                            let members =
                                                self.data.members.iter().map(|m| m.id).collect();
                                            self.data.channels.push(Channel {
                                                id,
                                                name: clean,
                                                kind: ChannelKind::Channel,
                                                topic: String::new(),
                                                members,
                                                messages: vec![],
                                                last_read: now_ts(),
                                                creator: self.data.me,
                                                ..Default::default()
                                            });
                                            self.section = Section::Chat;
                                            self.chat_open = Some(id);
                                            self.dirty = true;
                                        }
                                        keep = false;
                                    }
                                });
                                if keep {
                                    self.new_channel = Some(name);
                                }
                            } else if filter.is_empty() && self.add_row(ui, "Add Channel").clicked()
                            {
                                self.new_channel = Some(String::new());
                            }
                        }

                        // Direct Messages
                        if (filter.is_empty() || filter == "dms" || filter == "unread")
                            && !self.hidden("Direct Messages")
                        {
                            let hs =
                                self.side_header(ui, "Direct Messages", &[("+", "New message")]);
                            if hs[0].clicked() {
                                self.new_message_open = true;
                            }
                            let me = self.data.me;
                            let dms: Vec<(u64, Option<Member>, usize)> = self
                                .data
                                .channels
                                .iter()
                                .filter(|c| c.kind == ChannelKind::Direct && self.listed(c))
                                .map(|c| {
                                    (
                                        c.id,
                                        c.members
                                            .iter()
                                            .find(|m| **m != me)
                                            .and_then(|m| self.data.member(*m))
                                            .cloned(),
                                        self.unread_count(c),
                                    )
                                })
                                .collect();
                            for (id, other, real_unread) in dms {
                                let unread = if muted { 0 } else { real_unread };
                                let (name, sub) = match &other {
                                    Some(m) => (m.name.clone(), None),
                                    None => (
                                        self.data
                                            .member(me)
                                            .map(|m| m.name.clone())
                                            .unwrap_or_default(),
                                        Some(tr("\u{2014} You").to_string()),
                                    ),
                                };
                                if (filter == "unread" && real_unread == 0)
                                    || !self.matches_search(&name)
                                {
                                    continue;
                                }
                                let sel =
                                    self.section == Section::Chat && self.chat_open == Some(id);
                                let who = other.clone().or_else(|| self.data.member(me).cloned());
                                let bg = t.sidebar;
                                let r = self.side_row(
                                    ui,
                                    0.0,
                                    sel,
                                    |p, c| {
                                        if let Some(m) = &who {
                                            paint_avatar(p, c, 9.0, m, bg);
                                            p.circle_filled(c + vec2(7.0, 7.0), 3.0, bg);
                                            p.circle_filled(
                                                c + vec2(7.0, 7.0),
                                                2.0,
                                                Color32::from_rgb(48, 164, 108),
                                            );
                                        }
                                    },
                                    &name,
                                    sub.as_deref(),
                                    if unread > 0 {
                                        Some(unread.to_string())
                                    } else {
                                        None
                                    },
                                    false,
                                );
                                let sr = Rect::from_center_size(
                                    r.rect.right_center() - vec2(14.0, 0.0),
                                    vec2(20.0, 20.0),
                                );
                                let sresp =
                                    ui.interact(sr, ui.id().with(("dmset", id)), Sense::click());
                                if r.hovered() || sresp.hovered() {
                                    ui.painter().rect_filled(
                                        sr,
                                        5.0,
                                        if sresp.hovered() {
                                            t.border
                                        } else {
                                            t.row_selected
                                        },
                                    );
                                    ui.painter().text(
                                        sr.center(),
                                        Align2::CENTER_CENTER,
                                        "\u{22EF}",
                                        FontId::proportional(13.0),
                                        t.text2,
                                    );
                                }
                                if sresp.on_hover_text(tr("DM Settings")).clicked() {
                                    self.modal = Some(Modal::ChannelSettings(id));
                                } else if r.clicked() {
                                    self.section = Section::Chat;
                                    self.chat_open = Some(id);
                                    self.thread_open = None;
                                }
                            }
                            if filter.is_empty() && self.add_row(ui, "New message").clicked() {
                                self.new_message_open = true;
                            }
                        }
                        ui.add_space(20.0);
                    });
            });
    }

    pub(super) fn on_personal_list(&self) -> bool {
        matches!(self.loc, Location::List(id) if Some(id) == self.personal_list_id())
    }

    /// The Personal List opens with Group: None, the Date created column and
    /// only Board / List views; the earlier settings come back on leaving.
    pub(super) fn personal_list_defaults(&mut self) {
        let on = self.section == Section::Home && self.on_personal_list();
        match (on, self.personal_saved) {
            (true, None) => {
                self.personal_saved =
                    Some((self.group_by, self.hidden_cols.contains("Date created")));
                self.group_by = GroupBy::None;
                self.hidden_cols.remove("Date created");
            }
            (false, Some((g, hid))) => {
                self.group_by = g;
                if hid {
                    self.hidden_cols.insert("Date created".into());
                }
                self.personal_saved = None;
            }
            _ => {}
        }
        if on && !matches!(self.view, ViewKind::Board | ViewKind::List) {
            self.view = ViewKind::List;
        }
    }

    pub(super) fn personal_list_id(&self) -> Option<u64> {
        let me = self.data.me;
        self.data
            .spaces
            .iter()
            .find(|s| s.private_owner == Some(me))
            .and_then(|s| s.lists.first())
            .map(|l| l.id)
    }

    pub(super) fn create_personal_list(&mut self) -> u64 {
        if let Some(id) = self.personal_list_id() {
            return id;
        }
        let me = self.data.me;
        let sid = self.data.new_id();
        let lid = self.data.new_id();
        self.data.spaces.push(Space {
            id: sid,
            name: "Personal".into(),
            color: [100, 100, 100],
            statuses: default_statuses(),
            folders: vec![],
            lists: vec![ListItem {
                id: lid,
                name: "Personal List".into(),
                ..Default::default()
            }],
            expanded: true,
            private_owner: Some(me),
            ..Default::default()
        });
        self.dirty = true;
        lid
    }

    fn unread_count(&self, c: &Channel) -> usize {
        if !self.data.extra.notify.chat {
            return 0;
        }
        self.unread(c)
    }

    fn space_rows(&mut self, ui: &mut Ui, s: &Space) {
        let t = self.t();
        let loc = Location::Space(s.id);
        let color = rgb(s.color);
        let letter = Data::space_glyph(s);
        // a search shows the matches inside collapsed spaces and folders too
        let searching = self.side_searching();
        let expanded = s.expanded || searching;
        let sel = self.section == Section::Home && self.loc == loc;
        let r = self.side_row(
            ui,
            0.0,
            sel,
            |p, c| {
                let rr = Rect::from_center_size(c, vec2(19.0, 19.0));
                p.rect_filled(rr, 5.0, color);
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    letter,
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
            },
            &s.name,
            None,
            None,
            false,
        );
        // add button on hover / expand toggle on the icon
        let plus =
            Rect::from_center_size(r.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
        let pr = ui.interact(plus, ui.id().with(("splus", s.id)), Sense::click());
        if r.hovered() || pr.hovered() || sel {
            ui.painter().text(
                plus.center(),
                Align2::CENTER_CENTER,
                "+",
                FontId::proportional(16.0),
                t.text2,
            );
        }
        let sid = s.id;
        popup(ui, &pr, ui.id().with(("splus_pop", s.id)), 220.0, |ui| {
            ui.label(RichText::new(tr("Create")).small().color(t.text3));
            for (icon, label) in [
                ("\u{2630}", "List"),
                ("\u{1F4C1}", "Folder"),
                ("\u{1F4C4}", "Doc"),
                ("\u{1F4CB}", "Form"),
                ("\u{1F4CA}", "Dashboard"),
                ("\u{1F3A8}", "Whiteboard"),
            ] {
                if ui
                    .add(ghost(egui::Button::new(format!("{}  {}", icon, tr(label)))))
                    .clicked()
                {
                    match label {
                        "List" => self.open_new_list(Some(sid), None),
                        "Folder" => self.actions.push(Action::NewFolder(sid)),
                        "Doc" => self.new_doc(Some(sid)),
                        "Form" => self.new_form(),
                        "Dashboard" => {
                            self.modal = Some(Modal::NewDashboard {
                                name: "New Dashboard".into(),
                            })
                        }
                        _ => self.new_whiteboard(),
                    }
                    close_popup(ui);
                }
            }
        });
        let icon = Rect::from_center_size(r.rect.left_center() + vec2(16.0, 0.0), vec2(20.0, 20.0));
        let ir = ui.interact(icon, ui.id().with(("sicon", s.id)), Sense::click());
        if ir.hovered() {
            ui.painter().rect_filled(icon, 5.0, t.sidebar);
            paint_triangle(ui.painter(), icon.center(), expanded, t.text2);
        }
        if ir.clicked() {
            self.actions.push(Action::ToggleSpace(s.id));
        } else if r.clicked()
            && !pr.clicked()
            && !Rect::from_center_size(r.rect.right_center() - vec2(36.0, 0.0), vec2(20.0, 20.0))
                .contains(r.interact_pointer_pos().unwrap_or_default())
        {
            self.actions.push(Action::Go(loc));
            if !expanded {
                self.actions.push(Action::ToggleSpace(s.id));
            }
        }
        // "..." on hover opens the same menu as a right click
        let dots =
            Rect::from_center_size(r.rect.right_center() - vec2(36.0, 0.0), vec2(20.0, 20.0));
        let dr = ui.interact(dots, ui.id().with(("sdots", s.id)), Sense::click());
        name_it(&dr, &format!("Space options {}", s.name));
        if r.hovered() || dr.hovered() || pr.hovered() {
            if dr.hovered() {
                ui.painter().rect_filled(dots, 5.0, t.border);
            }
            ui.painter().text(
                dots.center(),
                Align2::CENTER_CENTER,
                "\u{22EF}",
                FontId::proportional(13.0),
                t.text2,
            );
        }
        click_menu(&dr, |ui| self.space_menu(ui, sid));
        r.context_menu(|ui| self.space_menu(ui, sid));
        // drag a space onto another to reorder; drop a list on it to move it here
        let r = r.interact(Sense::click_and_drag());
        if r.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), SideDrag::Space(sid));
        }
        if let Some(d) = self.side_drop(ui, r.rect) {
            match d {
                SideDrag::Space(other) => self.place_space_before(other, sid),
                SideDrag::List(l) => self.move_list(l, sid, None),
            }
        }
        if !expanded {
            return;
        }
        let tc = t.text2;
        for f in s.folders.iter().filter(|f| !f.archived) {
            if searching
                && !self.matches_search(&f.name)
                && !f
                    .lists
                    .iter()
                    .any(|l| !l.archived && self.matches_search(&l.name))
            {
                continue;
            }
            let floc = Location::Folder(f.id);
            let fexp = f.expanded || searching;
            let fsel = self.section == Section::Home && self.loc == floc;
            let fr = self.side_row(
                ui,
                14.0,
                fsel,
                |p, c| {
                    let rr = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(14.0, 11.0));
                    p.rect_stroke(rr, 2.0, Stroke::new(1.2, tc), egui::StrokeKind::Inside);
                    p.line_segment(
                        [
                            rr.left_top() + vec2(0.0, 2.5),
                            rr.left_top() + vec2(5.0, 2.5),
                        ],
                        Stroke::new(1.2, tc),
                    );
                },
                &f.name,
                None,
                None,
                false,
            );
            let fid = f.id;
            // "..." and "+" on hover, as on a space or list row
            let dots =
                Rect::from_center_size(fr.rect.right_center() - vec2(36.0, 0.0), vec2(20.0, 20.0));
            let plus =
                Rect::from_center_size(fr.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
            let dr = ui.interact(dots, ui.id().with(("fdots", fid)), Sense::click());
            let pr = ui.interact(plus, ui.id().with(("fplus", fid)), Sense::click());
            name_it(&dr, &format!("Folder options {}", f.name));
            name_it(&pr, &format!("Create in {}", f.name));
            if fr.hovered() || dr.hovered() || pr.hovered() {
                for (rr, hov, glyph) in
                    [(dots, dr.hovered(), "\u{22EF}"), (plus, pr.hovered(), "+")]
                {
                    ui.painter()
                        .rect_filled(rr, 5.0, if hov { t.border } else { t.hover });
                    ui.painter().text(
                        rr.center(),
                        Align2::CENTER_CENTER,
                        glyph,
                        FontId::proportional(14.0),
                        t.text2,
                    );
                }
            }
            click_menu(&dr, |ui| self.folder_menu(ui, fid));
            click_menu(&pr, |ui| {
                ui.set_min_width(180.0);
                ui.label(RichText::new(tr("Create")).small().color(t.text3));
                if ui.button(format!("\u{2630}  {}", tr("List"))).clicked() {
                    self.open_new_list(Some(sid), Some(fid));
                    ui.close_menu();
                }
                if ui.button(format!("\u{1F4C4}  {}", tr("Doc"))).clicked() {
                    self.new_doc(Some(sid));
                    ui.close_menu();
                }
            });
            // the folder icon is the expand arrow (as on spaces); clicking the
            // rest of the row opens the folder and expands it
            let ficon =
                Rect::from_center_size(fr.rect.left_center() + vec2(30.0, 0.0), vec2(20.0, 20.0));
            let fir = ui.interact(ficon, ui.id().with(("ficon", f.id)), Sense::click());
            if fir.hovered() {
                ui.painter().rect_filled(ficon, 5.0, t.sidebar);
                paint_triangle(ui.painter(), ficon.center(), fexp, t.text2);
            }
            let on_button = fr
                .interact_pointer_pos()
                .is_some_and(|p| dots.contains(p) || plus.contains(p));
            if fir.clicked() {
                self.actions.push(Action::ToggleFolder(f.id));
            } else if fr.clicked() && !on_button {
                self.actions.push(Action::Go(floc));
                if !f.expanded {
                    self.actions.push(Action::ToggleFolder(f.id));
                }
            }
            fr.context_menu(|ui| self.folder_menu(ui, fid));
            // a list dropped on a folder moves into it
            let fr = fr.interact(Sense::click_and_drag());
            if let Some(SideDrag::List(l)) = self.side_drop(ui, fr.rect) {
                self.move_list(l, sid, Some(fid));
            }
            if fexp {
                let shown: Vec<&ListItem> = f
                    .lists
                    .iter()
                    .filter(|l| !l.archived && self.matches_search(&l.name))
                    .collect();
                for l in shown {
                    self.list_row(ui, 30.0, l);
                }
            }
        }
        let shown: Vec<&ListItem> = s
            .lists
            .iter()
            .filter(|l| !l.archived && self.matches_search(&l.name))
            .collect();
        for l in shown {
            self.list_row(ui, 14.0, l);
        }
        // someone else's Private docs stay out (as in the Docs list)
        let docs: Vec<(u64, String)> = self
            .data
            .docs
            .iter()
            .filter(|d| d.space == Some(s.id) && d.parent.is_none() && self.doc_shown(d))
            .map(|d| (d.id, d.title.clone()))
            .collect();
        for (id, title) in docs {
            if !self.matches_search(&title) {
                continue;
            }
            let sel = self.section == Section::Docs && self.doc_open == Some(id);
            let r = self.side_row(
                ui,
                14.0,
                sel,
                |p, c| {
                    let rr = Rect::from_center_size(c, vec2(12.0, 14.0));
                    p.rect_filled(rr, 2.0, Color32::from_rgb(16, 144, 224));
                    for k in 0..3 {
                        let y = rr.top() + 4.0 + k as f32 * 3.0;
                        p.line_segment(
                            [pos2(rr.left() + 3.0, y), pos2(rr.right() - 3.0, y)],
                            Stroke::new(1.0, Color32::WHITE),
                        );
                    }
                },
                &title,
                None,
                None,
                false,
            );
            if r.clicked() {
                self.doc_open = Some(id);
                self.section = Section::Docs;
            }
            r.context_menu(|ui| self.doc_menu(ui, id, true));
        }
    }

    fn list_row(&mut self, ui: &mut Ui, indent: f32, l: &ListItem) {
        let t = self.t();
        let loc = Location::List(l.id);
        let n = self.count_open(l.id);
        let tc = t.text2;
        let sel = self.section == Section::Home && self.loc == loc;
        // the List's colour (Space Overview, or the list menu's Color) tints its icon
        let ic = l.color.map(rgb).unwrap_or(tc);
        let r = self.side_row(
            ui,
            indent,
            sel,
            |p, c| paint_checklist_icon(p, c, ic),
            &l.name,
            None,
            if n > 0 { Some(n.to_string()) } else { None },
            false,
        );
        let dots_hit = r
            .interact_pointer_pos()
            .map_or(false, |p| p.x > r.rect.right() - 48.0);
        if r.clicked() && !dots_hit {
            self.actions.push(Action::Go(loc));
        }
        let lid = l.id;
        let dots =
            Rect::from_center_size(r.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
        let dr = ui.interact(dots, ui.id().with(("ldots", l.id)), Sense::click());
        name_it(&dr, &format!("List options {}", l.name));
        if r.hovered() || dr.hovered() {
            // the open-task count sits under the button; cover it
            ui.painter().rect_filled(
                dots.expand2(vec2(6.0, 0.0)),
                5.0,
                if dr.hovered() { t.border } else { t.hover },
            );
            ui.painter().text(
                dots.center(),
                Align2::CENTER_CENTER,
                "\u{22EF}",
                FontId::proportional(13.0),
                t.text2,
            );
        }
        click_menu(&dr, |ui| self.list_menu(ui, lid));
        // "+" next to it creates a task in this list
        let plus =
            Rect::from_center_size(r.rect.right_center() - vec2(36.0, 0.0), vec2(20.0, 20.0));
        let pr = ui.interact(plus, ui.id().with(("lplus", l.id)), Sense::click());
        name_it(&pr, &format!("Create in {}", l.name));
        if r.hovered() || pr.hovered() || dr.hovered() {
            ui.painter()
                .rect_filled(plus, 5.0, if pr.hovered() { t.border } else { t.hover });
            ui.painter().text(
                plus.center(),
                Align2::CENTER_CENTER,
                "+",
                FontId::proportional(15.0),
                t.text2,
            );
        }
        // the same "Create" menu as the original's list "+"
        click_menu(&pr, |ui| self.list_create_menu(ui, lid));
        r.context_menu(|ui| self.list_menu(ui, lid));
        // drag a list onto another list to put it there; drop a task card on
        // a list to move the task into it
        let r = r.interact(Sense::click_and_drag());
        if r.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), SideDrag::List(lid));
        }
        if let Some(SideDrag::List(other)) = self.side_drop(ui, r.rect) {
            self.place_list_before(other, lid);
        }
        let task_over = egui::DragAndDrop::has_payload_of_type::<u64>(ui.ctx())
            && ui.rect_contains_pointer(r.rect);
        if task_over {
            ui.painter().rect_stroke(
                r.rect,
                7.0,
                Stroke::new(1.5_f32, t.accent_text),
                egui::StrokeKind::Inside,
            );
            if ui.input(|i| i.pointer.any_released()) {
                if let Some(tid) = egui::DragAndDrop::take_payload::<u64>(ui.ctx()) {
                    self.move_tasks(&[*tid], lid);
                }
            }
        }
    }

    /// Tasks starred with "Add to Favorites", for the Favorites section.
    fn favorite_tasks(&self) -> Vec<(u64, String)> {
        self.data
            .extra
            .starred
            .iter()
            .filter_map(|s| s.strip_prefix("task:")?.parse::<u64>().ok())
            .filter_map(|id| self.task(id))
            .filter(|t| !t.archived)
            .map(|t| (t.id, t.name.clone()))
            .collect()
    }

    /// While a sidebar item is dragged over `rect`: marks the spot and, when
    /// the mouse is let go there, hands back what was dropped.
    fn side_drop(&self, ui: &Ui, rect: Rect) -> Option<SideDrag> {
        if !egui::DragAndDrop::has_payload_of_type::<SideDrag>(ui.ctx())
            || !ui.rect_contains_pointer(rect)
        {
            return None;
        }
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            Stroke::new(2.0_f32, self.t().accent_text),
        );
        if !ui.input(|i| i.pointer.any_released()) {
            return None;
        }
        egui::DragAndDrop::take_payload::<SideDrag>(ui.ctx()).map(|d| *d)
    }

    // ---------------------------------------------------------------- page header

    pub(super) fn header(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 12,
                top: 8,
                bottom: 0,
            })
            .show(ui, |ui| {
                // breadcrumb
                ui.horizontal(|ui| {
                    ui.set_height(30.0);
                    let mut crumbs: Vec<(String, u8, Option<[u8; 3]>)> = vec![];
                    let personal = self.on_personal_list();
                    match self.loc {
                        Location::List(_) if personal => {
                            crumbs.push((tr("My Tasks").into(), 3, None));
                            crumbs.push((tr("Personal List").into(), 2, None));
                        }
                        Location::List(id) => {
                            if let Some(s) = self.data.space_of_list(id) {
                                crumbs.push((s.name.clone(), 0, Some(s.color)));
                                for f in &s.folders {
                                    if f.lists.iter().any(|l| l.id == id) {
                                        crumbs.push((f.name.clone(), 1, None));
                                    }
                                }
                            }
                            crumbs.push((self.data.list_name(id), 2, None));
                        }
                        Location::Folder(fid) => {
                            for s in &self.data.spaces {
                                if let Some(f) = s.folders.iter().find(|f| f.id == fid) {
                                    crumbs.push((s.name.clone(), 0, Some(s.color)));
                                    crumbs.push((f.name.clone(), 1, None));
                                }
                            }
                        }
                        Location::Space(sid) => {
                            if let Some(s) = self.data.spaces.iter().find(|s| s.id == sid) {
                                crumbs.push((s.name.clone(), 0, Some(s.color)));
                            }
                        }
                        l => crumbs.push((tr(&self.data.location_name(l)).to_string(), 3, None)),
                    }
                    let n = crumbs.len();
                    // the buttons on the right keep their room (their width is
                    // remembered from the last frame); crumb names share what is
                    // left and are cut with an ellipsis when it is not enough
                    let right_id = ui.id().with("crumb_right_w");
                    let right_w = ui.data(|d| d.get_temp::<f32>(right_id)).unwrap_or(360.0);
                    let sp = ui.spacing().item_spacing.x;
                    let font = FontId::proportional(15.0);
                    let mut widths: Vec<f32> = crumbs
                        .iter()
                        .map(|(name, _, _)| {
                            ui.painter()
                                .layout_no_wrap(name.clone(), font.clone(), t.text)
                                .size()
                                .x
                        })
                        .collect();
                    let room_for = |k: usize, ui: &Ui| {
                        let fixed = k as f32 * (18.0 + 2.0 * sp)
                            + (k.saturating_sub(1)) as f32 * (6.0 + sp)
                            + (10.0 + sp)
                            + 30.0
                            + if personal { 20.0 } else { 0.0 };
                        ui.available_width() - right_w - fixed - 12.0
                    };
                    // too narrow for every level: keep only the last ones
                    let mut skip = 0;
                    while n - skip > 1 && room_for(n - skip, ui) < 48.0 * (n - skip) as f32 {
                        skip += 1;
                    }
                    crumbs.drain(..skip);
                    widths.drain(..skip);
                    let n = crumbs.len();
                    let room = room_for(n, ui).max(40.0);
                    let cap = {
                        // largest cap with sum(min(w, cap)) <= room
                        let mut sorted = widths.clone();
                        sorted.sort_by(f32::total_cmp);
                        let mut left = room;
                        let mut cap = f32::INFINITY;
                        for (k, w) in sorted.iter().enumerate() {
                            let share = left / (sorted.len() - k) as f32;
                            if *w > share {
                                cap = share;
                                break;
                            }
                            left -= w;
                        }
                        cap
                    };
                    for (i, (name, kind, color)) in crumbs.iter().enumerate() {
                        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        match kind {
                            0 => {
                                ui.painter().rect_filled(
                                    r.shrink(1.0),
                                    4.0,
                                    rgb(color.unwrap_or([96, 73, 231])),
                                );
                                let glyph = self
                                    .data
                                    .spaces
                                    .iter()
                                    .find(|s| &s.name == name && Some(s.color) == *color)
                                    .map(Data::space_glyph)
                                    .unwrap_or_else(|| {
                                        name.chars()
                                            .next()
                                            .unwrap_or('S')
                                            .to_uppercase()
                                            .to_string()
                                    });
                                ui.painter().text(
                                    r.center(),
                                    Align2::CENTER_CENTER,
                                    glyph,
                                    FontId::proportional(10.0),
                                    Color32::WHITE,
                                );
                            }
                            1 => {
                                ui.painter().rect_stroke(
                                    Rect::from_center_size(r.center(), vec2(14.0, 11.0)),
                                    2.0,
                                    Stroke::new(1.2, t.text2),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            2 => paint_checklist_icon(ui.painter(), r.center(), t.text),
                            _ => {}
                        }
                        let last = i + 1 == n;
                        let text = RichText::new(name).size(15.0).color(if last {
                            t.text
                        } else {
                            t.text2
                        });
                        if widths[i] > cap {
                            ui.scope(|ui| {
                                ui.set_max_width(cap.max(20.0));
                                ui.add(egui::Label::new(text).truncate());
                            });
                        } else {
                            ui.label(text);
                        }
                        if last && personal {
                            ui.label(RichText::new("\u{1F512}").size(12.0).color(t.text3))
                                .on_hover_text(tr("Private: only you can see this List"));
                        }
                        if last {
                            let (cr, _) = ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
                            let cv = cr.center();
                            ui.painter().line_segment(
                                [cv + vec2(-3.5, -1.5), cv + vec2(0.0, 2.0)],
                                Stroke::new(1.3, t.text2),
                            );
                            ui.painter().line_segment(
                                [cv + vec2(0.0, 2.0), cv + vec2(3.5, -1.5)],
                                Stroke::new(1.3, t.text2),
                            );
                        }
                        if !last {
                            ui.label(RichText::new("/").color(t.text3));
                        }
                    }
                    let fav = self.data.favorites.contains(&self.loc);
                    if !matches!(self.loc, Location::MyTasks | Location::Everything) {
                        let star = RichText::new(if fav { "\u{2605}" } else { "\u{2606}" })
                            .size(16.0)
                            .color(if fav {
                                Color32::from_rgb(0xf8, 0xae, 0x00)
                            } else {
                                t.text2
                            });
                        if ui
                            .add(ghost(egui::Button::new(star)))
                            .on_hover_text(tr("Favorite"))
                            .clicked()
                        {
                            self.actions.push(Action::ToggleFavorite(self.loc));
                        }
                    }
                    let right_start = ui.max_rect().right();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let hb = |ui: &mut Ui, text: &str| -> Response {
                            let (kind, label) = match text {
                                t if t.contains("Share") => ("share", "Share"),
                                t if t.contains("Ask") => ("edit", "Ask"),
                                t if t.contains("Automate") => ("robot", "Automate"),
                                _ => ("phone", ""),
                            };
                            let g = ui.painter().layout_no_wrap(
                                tr(label).to_string(),
                                FontId::proportional(14.5),
                                t.text2,
                            );
                            let w = if label.is_empty() {
                                40.0
                            } else {
                                g.size().x + 32.0
                            };
                            let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                            name_it(&resp, if label.is_empty() { "Huddle" } else { label });
                            if resp.hovered() {
                                ui.painter().rect_filled(r, 6.0, t.hover);
                            }
                            line_icon(
                                ui.painter(),
                                kind,
                                r.left_center() + vec2(12.0, 0.0),
                                t.text2,
                            );
                            if label.is_empty() {
                                let cv = r.right_center() - vec2(9.0, 1.0);
                                ui.painter().line_segment(
                                    [cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5)],
                                    Stroke::new(1.2, t.text2),
                                );
                                ui.painter().line_segment(
                                    [cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)],
                                    Stroke::new(1.2, t.text2),
                                );
                            } else {
                                ui.painter().galley(
                                    pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0),
                                    g,
                                    t.text2,
                                );
                            }
                            resp
                        };
                        if matches!(self.loc, Location::List(_)) {
                            let r = ui.add(
                                egui::Button::new(RichText::new(tr("Manage")).size(14.0))
                                    .stroke(Stroke::new(1.0, t.border))
                                    .corner_radius(7)
                                    .min_size(vec2(0.0, 28.0)),
                            );
                            if let Location::List(lid) = self.loc {
                                popup(ui, &r, ui.id().with("manage_pop"), 200.0, |ui| {
                                    if ui.button(tr("Rename list")).clicked() {
                                        self.actions.push(Action::StartRename(Target::List(lid)));
                                        close_popup(ui);
                                    }
                                    if let Some(sid) = self.data.space_of_list(lid).map(|s| s.id) {
                                        if ui.button(tr("Edit statuses")).clicked() {
                                            self.actions.push(Action::EditStatuses(sid));
                                            close_popup(ui);
                                        }
                                    }
                                    if ui.button(tr("Custom Fields")).clicked() {
                                        self.modal = Some(Modal::CustomFields {
                                            name: String::new(),
                                            kind: "text".into(),
                                            opts: String::new(),
                                        });
                                        close_popup(ui);
                                    }
                                    if ui.button(tr("Export to CSV")).clicked() {
                                        self.toast = Some((self.export_csv(), now_ts()));
                                        close_popup(ui);
                                    }
                                    if ui
                                        .button(RichText::new(tr("Delete list")).color(t.danger))
                                        .clicked()
                                    {
                                        self.actions.push(Action::AskDelete(Target::List(lid)));
                                        close_popup(ui);
                                    }
                                });
                            }
                        }
                        if hb(ui, "\u{1F465}  Share").clicked() {
                            self.modal = Some(Modal::Share {
                                emails: String::new(),
                            });
                        }
                        if hb(ui, "\u{1F4DD}  Ask").clicked() {
                            self.section = Section::Ai;
                        }
                        if hb(ui, "\u{2699}  Automate").clicked() {
                            // new rules run where I am, as from the sidebar menus
                            // (a folder's rules run in its space)
                            let scope = match self.loc {
                                Location::List(_) | Location::Space(_) => Some(self.loc),
                                Location::Folder(f) => self
                                    .data
                                    .spaces
                                    .iter()
                                    .find(|s| s.folders.iter().any(|x| x.id == f))
                                    .map(|s| Location::Space(s.id)),
                                _ => None,
                            };
                            self.modal = Some(Modal::Automations {
                                status: "complete".into(),
                                action: "comment".into(),
                                value: String::new(),
                                scope,
                            });
                        }
                        if hb(ui, "\u{260E}")
                            .on_hover_text(tr("Start a huddle"))
                            .clicked()
                        {
                            self.open_call_dialog();
                        }
                        let used = right_start - ui.min_rect().left();
                        ui.data_mut(|d| d.insert_temp(right_id, used));
                    });
                });
                // view tabs
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    // "Add Channel": a chat channel for this location (not on the
                    // Personal List, as in the original)
                    let personal = self.on_personal_list();
                    let ac = if personal {
                        ui.allocate_response(vec2(0.0, 0.0), Sense::hover())
                    } else {
                        ui.add(
                            egui::Button::new(
                                RichText::new(tr("Add Channel")).size(13.5).color(t.text3),
                            )
                            .stroke(Stroke::new(1.0, t.border))
                            .corner_radius(6)
                            .min_size(vec2(0.0, 26.0)),
                        )
                    };
                    if ac.clicked() {
                        let name = self
                            .data
                            .location_name(self.loc)
                            .to_lowercase()
                            .replace(' ', "-");
                        // (only one I may see: someone else's private channel of that name doesn't count)
                        let existing = self
                            .data
                            .channels
                            .iter()
                            .find(|c| {
                                c.kind == ChannelKind::Channel && c.name == name && self.can_see(c)
                            })
                            .map(|c| c.id);
                        let id = existing.unwrap_or_else(|| {
                            let id = self.data.new_id();
                            let members = self.data.members.iter().map(|m| m.id).collect();
                            self.data.channels.push(Channel {
                                id,
                                name: name.clone(),
                                kind: ChannelKind::Channel,
                                topic: format!(
                                    "Conversations about {}",
                                    self.data.location_name(self.loc)
                                ),
                                members,
                                messages: vec![],
                                last_read: now_ts(),
                                creator: self.data.me,
                                ..Default::default()
                            });
                            self.dirty = true;
                            id
                        });
                        self.section = Section::Chat;
                        self.chat_open = Some(id);
                    }
                    ui.add_space(8.0);
                    // Overview only covers a space or a folder
                    let is_list = !matches!(self.loc, Location::Space(_) | Location::Folder(_));
                    if is_list && self.view == ViewKind::Overview {
                        self.view = ViewKind::List;
                    }
                    for v in ViewKind::ALL {
                        if (v == ViewKind::Overview && is_list)
                            || (v != self.view
                                && self.data.extra.hidden_views.iter().any(|h| h == v.label()))
                        {
                            continue;
                        }
                        if personal && !matches!(v, ViewKind::Board | ViewKind::List) {
                            continue;
                        }
                        let sel = self.view == v;
                        let g = ui.painter().layout_no_wrap(
                            tr(v.label()).to_string(),
                            FontId::proportional(14.0),
                            if sel { t.text } else { t.text2 },
                        );
                        let (r, resp) =
                            ui.allocate_exact_size(vec2(g.size().x + 32.0, 34.0), Sense::click());
                        name_it(&resp, &format!("View {}", v.label()));
                        if resp.hovered() && !sel {
                            ui.painter()
                                .rect_filled(r.shrink2(vec2(0.0, 5.0)), 6.0, t.hover);
                        }
                        paint_view_icon(ui.painter(), pos2(r.left() + 13.0, r.center().y), v);
                        let gc = if sel { t.text } else { t.text2 };
                        ui.painter().galley(
                            pos2(r.left() + 25.0, r.center().y - g.size().y / 2.0),
                            g.clone(),
                            gc,
                        );
                        if sel {
                            ui.painter().galley(
                                pos2(r.left() + 25.3, r.center().y - g.size().y / 2.0),
                                g,
                                gc,
                            );
                            ui.painter().rect_filled(
                                Rect::from_min_max(
                                    pos2(r.left() + 2.0, r.bottom() - 2.0),
                                    pos2(r.right() - 2.0, r.bottom()),
                                ),
                                1.0,
                                t.text,
                            );
                        }
                        if resp.clicked() {
                            self.view = v;
                        }
                    }
                    ui.add_space(6.0);
                    let vr = ui.add(ghost(
                        egui::Button::new(RichText::new(tr("+  View")).size(14.0).color(t.text2))
                            .min_size(vec2(0.0, 30.0)),
                    ));
                    popup(ui, &vr, ui.id().with("add_view"), 200.0, |ui| {
                        ui.label(RichText::new(tr("Views")).small().color(t.text3));
                        for v in ViewKind::ALL {
                            let mut on =
                                !self.data.extra.hidden_views.iter().any(|h| h == v.label());
                            if ui.checkbox(&mut on, tr(v.label())).changed() {
                                if on {
                                    self.data.extra.hidden_views.retain(|h| h != v.label());
                                } else {
                                    self.data.extra.hidden_views.push(v.label().to_string());
                                }
                                self.dirty = true;
                            }
                        }
                    });
                });
            });
        let r = ui.available_rect_before_wrap();
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
        // the Overview has cards, not tasks: no task toolbar
        if self.view == ViewKind::Overview {
            ui.add_space(8.0);
            return;
        }
        // toolbar
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(16, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // full chip labels, and whether they all fit in one row; if
                    // not (a narrow window) the chips show only their icon
                    let grouped = matches!(self.view, ViewKind::List | ViewKind::Board);
                    let group_label = format!("\u{2261}  Group: {}", self.group_by.label());
                    let alabel = match self.assignee_filter.and_then(|a| self.data.member(a)) {
                        Some(m) => format!("\u{1F464}  {}", m.name),
                        None => "\u{1F464}  Assignee".into(),
                    };
                    let nf = self.filter.count()
                        + usize::from(!self.search.is_empty())
                        + usize::from(self.me_mode);
                    let flabel = if nf > 0 {
                        format!("\u{2261}  Filter \u{00B7} {}", nf)
                    } else {
                        "\u{2261}  Filter".to_string()
                    };
                    let sort_arrow = if self.sort_desc && self.sort_by != SortBy::Manual {
                        " \u{2193}"
                    } else {
                        ""
                    };
                    let slabel = format!("\u{21C5}  Sort: {}{}", self.sort_by.label(), sort_arrow);
                    // the same labels in the interface language (the English ones name the chips)
                    let group_shown = trf!("\u{2261}  Group: {}", tr(self.group_by.label()));
                    let ashown = match self.assignee_filter.and_then(|a| self.data.member(a)) {
                        Some(_) => alabel.clone(),
                        None => tr("\u{1F464}  Assignee").to_string(),
                    };
                    let fshown = if nf > 0 {
                        trf!("\u{2261}  Filter \u{00B7} {}", nf)
                    } else {
                        tr("\u{2261}  Filter").to_string()
                    };
                    let sshown = trf!("\u{21C5}  Sort: {}{}", tr(self.sort_by.label()), sort_arrow);
                    let compact = {
                        let pw = |s: &str| {
                            ui.painter()
                                .layout_no_wrap(s.to_string(), FontId::proportional(13.0), t.text)
                                .size()
                                .x
                                + 20.0
                                + 6.0
                        };
                        let mut need = pw(&ashown)
                            + pw(tr("\u{2714}  Closed"))
                            + pw(&fshown)
                            + pw(&sshown)
                            + 90.0
                            + 32.0
                            + 32.0;
                        if grouped {
                            need += pw(&group_shown) + pw(tr("\u{2934}  Subtasks"));
                        }
                        if self.view == ViewKind::List {
                            need += pw(tr("\u{25A5}  Columns"));
                        }
                        if self.show_search {
                            need += 186.0;
                        }
                        need > ui.available_width()
                    };
                    // `full` names the chip (English), `shown` is what it says
                    let chip = |ui: &mut Ui,
                                full: &str,
                                shown: &str,
                                short: &str,
                                active: bool,
                                accent: bool|
                     -> Response {
                        if compact {
                            let r = pill(ui, &t, short, short, active, accent);
                            name_it(&r, full);
                            r.on_hover_text(shown)
                        } else {
                            pill(ui, &t, full, shown, active, accent)
                        }
                    };
                    if grouped {
                        let r = chip(ui, &group_label, &group_shown, "\u{2261}", false, true);
                        popup(ui, &r, ui.id().with("group_pop"), 160.0, |ui| {
                            for g in GroupBy::ALL {
                                if ui
                                    .selectable_label(self.group_by == g, tr(g.label()))
                                    .clicked()
                                {
                                    self.group_by = g;
                                    close_popup(ui);
                                }
                            }
                        });
                        if chip(
                            ui,
                            "\u{2934}  Subtasks",
                            tr("\u{2934}  Subtasks"),
                            "\u{2934}",
                            self.subtasks_open,
                            false,
                        )
                        .on_hover_text(tr("Show all subtasks"))
                        .clicked()
                        {
                            self.subtasks_open = !self.subtasks_open;
                        }
                    }
                    if self.view == ViewKind::List {
                        let n = self
                            .hidden_cols
                            .iter()
                            .filter(|c| {
                                c.as_str() != "Date created"
                                    && !parity::PROPERTIES.contains(&c.as_str())
                                    // the Table's and custom field columns
                                    // are not the List's built-in columns
                                    && !c.starts_with("table:")
                                    && !c.starts_with("field:")
                            })
                            .count();
                        let extra = parity::PROPERTIES.iter().filter(|p| self.col_on(p)).count();
                        let cr = chip(
                            ui,
                            "\u{25A5}  Columns",
                            tr("\u{25A5}  Columns"),
                            "\u{25A5}",
                            n > 0 || extra > 0 || self.fields_open,
                            false,
                        );
                        if cr.clicked() {
                            self.fields_open = !self.fields_open;
                        }
                        if self.fields_open {
                            self.fields_panel(ui.ctx(), cr.rect.bottom() + 6.0);
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let b = Tint(
                            egui::Button::new(
                                RichText::new(tr("Add Task"))
                                    .color(Color32::WHITE)
                                    .size(13.0),
                            )
                            .fill(t.primary)
                            .corner_radius(6)
                            .min_size(vec2(0.0, 26.0)),
                        );
                        if ui.add(b).clicked() {
                            self.open_quick_add(None);
                        }
                        // search toggle
                        let (sr, sresp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                        ui.painter().rect_stroke(
                            sr,
                            6.0,
                            Stroke::new(1.0, t.border),
                            egui::StrokeKind::Inside,
                        );
                        if sresp.hovered() || self.show_search {
                            ui.painter().rect_filled(sr.shrink(1.0), 6.0, t.hover);
                        }
                        ui.painter().text(
                            sr.center(),
                            Align2::CENTER_CENTER,
                            "\u{1F50D}",
                            FontId::proportional(12.0),
                            t.text2,
                        );
                        name_it(&sresp, "Search tasks");
                        if sresp.on_hover_text(tr("Search tasks")).clicked() {
                            self.show_search = !self.show_search;
                            self.search_focus = self.show_search;
                            if !self.show_search {
                                self.search.clear();
                            }
                        }
                        if self.show_search {
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut self.search)
                                    .hint_text(tr("Search tasks..."))
                                    .desired_width(if compact { 110.0 } else { 180.0 }),
                            );
                            name_field(&r, "Search tasks...");
                            if std::mem::take(&mut self.search_focus) {
                                r.request_focus();
                            }
                        }
                        // me mode avatar
                        if let Some(m) = self.data.member(self.data.me).cloned() {
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                            if self.me_mode {
                                ui.painter().circle_stroke(
                                    r.center(),
                                    12.5,
                                    Stroke::new(1.5, t.text),
                                );
                            }
                            paint_avatar(ui.painter(), r.center(), 10.5, &m, t.bg);
                            if !self.me_mode {
                                hover_ring(ui, &resp, r.shrink(1.5), 11.5);
                            }
                            if resp.on_hover_text(tr("Me mode: only my tasks")).clicked() {
                                self.me_mode = !self.me_mode;
                            }
                        }
                        let ar = chip(
                            ui,
                            &alabel,
                            &ashown,
                            "\u{1F464}",
                            self.assignee_filter.is_some(),
                            false,
                        );
                        let data = self.data.clone();
                        popup(ui, &ar, ui.id().with("asg_filter"), 200.0, |ui| {
                            if ui
                                .selectable_label(self.assignee_filter.is_none(), tr("Everyone"))
                                .clicked()
                            {
                                self.assignee_filter = None;
                                close_popup(ui);
                            }
                            for m in &data.members {
                                if ui
                                    .selectable_label(self.assignee_filter == Some(m.id), &m.name)
                                    .clicked()
                                {
                                    self.assignee_filter = Some(m.id);
                                    close_popup(ui);
                                }
                            }
                        });
                        if chip(
                            ui,
                            "\u{2714}  Closed",
                            tr("\u{2714}  Closed"),
                            "\u{2714}",
                            self.show_closed,
                            false,
                        )
                        .on_hover_text(tr("Show closed tasks"))
                        .clicked()
                        {
                            self.show_closed = !self.show_closed;
                        }
                        let fshort = if nf > 0 {
                            format!("\u{2261} {}", nf)
                        } else {
                            "\u{2261}".to_string()
                        };
                        let fr = chip(
                            ui,
                            &flabel,
                            &fshown,
                            &fshort,
                            nf > 0 || self.filter_open,
                            false,
                        );
                        if fr.clicked() {
                            self.filter_open = !self.filter_open;
                        }
                        if self.filter_open {
                            self.filter_panel(
                                ui.ctx(),
                                fr.rect.left_bottom() + vec2(0.0, 6.0),
                                fr.rect,
                            );
                        }
                        let sr = chip(
                            ui,
                            &slabel,
                            &sshown,
                            &format!("\u{21C5}{}", sort_arrow),
                            self.sort_by != SortBy::Manual,
                            false,
                        );
                        popup(ui, &sr, ui.id().with("sort_pop"), 160.0, |ui| {
                            for s in SortBy::ALL {
                                if ui
                                    .selectable_label(self.sort_by == s, tr(s.label()))
                                    .clicked()
                                {
                                    self.sort_by = s;
                                    close_popup(ui);
                                }
                            }
                            if self.sort_by != SortBy::Manual {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    if ui
                                        .selectable_label(!self.sort_desc, tr("Ascending"))
                                        .clicked()
                                    {
                                        self.sort_desc = false;
                                    }
                                    if ui
                                        .selectable_label(self.sort_desc, tr("Descending"))
                                        .clicked()
                                    {
                                        self.sort_desc = true;
                                    }
                                });
                            }
                        });
                    });
                });
            });
    }

    // ---------------------------------------------------------------- list view

    /// The custom fields the List can show as columns (none while the
    /// Custom Fields ClickApp is off).
    pub(super) fn list_field_ids(&self) -> Vec<u64> {
        if self.data.extra.apps.custom_fields {
            self.data.extra.fields.iter().map(|f| f.id).collect()
        } else {
            vec![]
        }
    }

    /// The List's columns in a row rectangle.
    pub(super) fn list_cols(&self, row: Rect) -> Cols {
        col_layout_with(
            row,
            &self.hidden_cols,
            self.data.extra.apps.priorities,
            &self.list_field_ids(),
        )
    }

    /// A custom field's cell in a List row: shows the value and edits it in
    /// place (a checkbox ticks, a dropdown or date opens its picker, text and
    /// numbers open a small box). Returns whether `edited` changed.
    fn field_cell(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        id: egui::Id,
        task: &Task,
        fid: u64,
        edited: &mut Task,
    ) -> bool {
        let t = self.t();
        let Some(f) = self.data.extra.fields.iter().find(|f| f.id == fid).cloned() else {
            return false;
        };
        let v = task.custom.get(&fid).cloned().unwrap_or_default();
        let cell = r.shrink2(vec2(0.0, 4.0));
        let resp = ui.interact(cell, id, Sense::click());
        name_it(&resp, &format!("{} of {}", f.name, task.name));
        if resp.hovered() {
            ui.painter().rect_stroke(
                cell.shrink2(vec2(2.0, 0.0)),
                4.0,
                Stroke::new(1.0, t.border),
                egui::StrokeKind::Inside,
            );
        }
        let p = ui.painter_at(cell);
        let at = cell.left_center() + vec2(6.0, 0.0);
        let font = FontId::proportional(13.0);
        let mut set: Option<String> = None;
        match f.kind.as_str() {
            "checkbox" => {
                let on = v == "true";
                let b = Rect::from_center_size(at + vec2(7.0, 0.0), vec2(14.0, 14.0));
                p.rect_stroke(b, 3.0, Stroke::new(1.2, t.text3), egui::StrokeKind::Inside);
                if on {
                    p.rect_filled(b, 3.0, t.primary);
                    p.text(
                        b.center(),
                        Align2::CENTER_CENTER,
                        "\u{2713}",
                        FontId::proportional(11.0),
                        Color32::WHITE,
                    );
                }
                if resp.clicked() {
                    set = Some(if on { String::new() } else { "true".into() });
                }
            }
            "dropdown" => {
                if !v.is_empty() {
                    p.text(at, Align2::LEFT_CENTER, &v, font, t.text);
                }
                popup(ui, &resp, id.with("pop"), 180.0, |ui| {
                    if ui.selectable_label(v.is_empty(), tr("Empty")).clicked() {
                        set = Some(String::new());
                        close_popup(ui);
                    }
                    for o in &f.options {
                        if ui.selectable_label(v == *o, o).clicked() {
                            set = Some(o.clone());
                            close_popup(ui);
                        }
                    }
                });
            }
            "date" => {
                let mut d = NaiveDate::parse_from_str(&v, "%Y-%m-%d").ok();
                if let Some(day) = d {
                    p.text(at, Align2::LEFT_CENTER, fmt_date(day), font, t.text2);
                }
                if let Some(true) = popup(ui, &resp, id.with("pop"), 380.0, |ui| {
                    date_menu(ui, &t, id.with("cal"), &mut d)
                }) {
                    set = Some(d.map(|d| d.to_string()).unwrap_or_default());
                }
            }
            kind => {
                if !v.is_empty() {
                    p.text(at, Align2::LEFT_CENTER, &v, font, t.text2);
                }
                let number = kind == "number";
                let tid = id.with("text");
                popup(ui, &resp, id.with("pop"), 200.0, |ui| {
                    let mut text: String = ui
                        .data(|d| d.get_temp::<String>(tid))
                        .unwrap_or_else(|| v.clone());
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .hint_text(tr("Empty"))
                            .desired_width(180.0),
                    );
                    name_field(&r, &format!("Edit {}", f.name));
                    focus_once(ui, &r);
                    let ok =
                        !number || text.trim().is_empty() || text.trim().parse::<f64>().is_ok();
                    if number && !ok {
                        ui.label(RichText::new(tr("Enter a number")).small().color(t.danger));
                    }
                    if r.changed() && ok {
                        set = Some(text.trim().to_string());
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        close_popup(ui);
                        ui.data_mut(|d| d.remove::<String>(tid));
                    } else {
                        ui.data_mut(|d| d.insert_temp(tid, text));
                    }
                });
            }
        }
        match set {
            Some(new) if new != v => {
                if new.is_empty() {
                    edited.custom.remove(&fid);
                } else {
                    edited.custom.insert(fid, new);
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn list_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.list_keys(ui.ctx());
        self.row_order = self.row_spots.drain(..).map(|s| s.id).collect();
        self.row_groups.clear();
        let tasks: Vec<Task> = self.filtered().into_iter().cloned().collect();
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = self.groups(&refs, false);
        // one column header row for the whole list
        let w = ui.available_width();
        let (hr, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
        self.list_width = w;
        let cols = self.list_cols(hr);
        let p = ui.painter();
        p.text(
            hr.left_center() + vec2(36.0, 0.0),
            Align2::LEFT_CENTER,
            tr("Name"),
            FontId::proportional(13.0),
            t.text2,
        );
        let show_pr = self.data.extra.apps.priorities;
        for (label, r) in [
            ("Assignee", cols.assignee),
            ("Due date", cols.due),
            ("Priority", cols.priority),
            ("Status", cols.status),
            ("Comments", cols.comments),
            ("Date created", cols.created),
        ] {
            if (label == "Priority" && !show_pr) || !self.col_on(label) || r.width() <= 0.0 {
                continue;
            }
            p.text(
                r.left_center(),
                Align2::LEFT_CENTER,
                tr(label),
                FontId::proportional(13.0),
                t.text2,
            );
        }
        for (label, r) in &cols.extra {
            p.with_clip_rect(*r).text(
                r.left_center(),
                Align2::LEFT_CENTER,
                tr(label),
                FontId::proportional(13.0),
                t.text2,
            );
        }
        for (fid, r) in &cols.fields {
            let name = self
                .data
                .extra
                .fields
                .iter()
                .find(|f| f.id == *fid)
                .map(|f| f.name.clone())
                .unwrap_or_default();
            p.with_clip_rect(*r).text(
                r.left_center(),
                Align2::LEFT_CENTER,
                format!("\u{25A4} {}", name),
                FontId::proportional(13.0),
                t.text2,
            );
        }
        let plus = Rect::from_center_size(pos2(hr.right() - 20.0, hr.center().y), vec2(16.0, 16.0));
        p.circle_filled(plus.center(), 8.0, t.text);
        // the background color reads on the circle in both themes
        p.text(
            plus.center(),
            Align2::CENTER_CENTER,
            "+",
            FontId::proportional(13.0),
            t.bg,
        );
        p.hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
        let add_col = ui.interact(plus.expand(4.0), ui.id().with("add_column"), Sense::click());
        hover_bg(ui, &add_col, add_col.rect, 5.0);
        if add_col
            .on_hover_text(tr("Add a column (Custom Field)"))
            .clicked()
        {
            self.modal = Some(Modal::CustomFields {
                name: String::new(),
                kind: "text".into(),
                opts: String::new(),
            });
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                if groups.iter().all(|g| g.tasks.is_empty()) {
                    // completed tasks are hidden by default: say so instead of "no tasks"
                    let hidden = if self.show_closed {
                        0
                    } else {
                        self.show_closed = true;
                        let n = self.filtered().len();
                        self.show_closed = false;
                        n.saturating_sub(self.filtered().len())
                    };
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        if hidden > 0 {
                            ui.label(
                                RichText::new(if hidden == 1 {
                                    tr("All tasks here are done").to_string()
                                } else {
                                    trf!("All {} tasks here are done", hidden)
                                })
                                .size(16.0)
                                .color(t.text2),
                            );
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.add_space((ui.available_width() - 230.0).max(0.0) / 2.0);
                                let label = if hidden == 1 {
                                    tr("Show 1 closed task").to_string()
                                } else {
                                    trf!("Show {} closed tasks", hidden)
                                };
                                if ui
                                    .add(
                                        egui::Button::new(label)
                                            .stroke(Stroke::new(1.0, t.border))
                                            .corner_radius(6),
                                    )
                                    .clicked()
                                {
                                    self.show_closed = true;
                                }
                                if primary_button(ui, &t, tr("Add Task")).clicked() {
                                    self.open_quick_add(None);
                                }
                            });
                        } else if self.on_personal_list() {
                            // the inline "+ Add Task" row below is the empty state
                        } else {
                            ui.label(
                                RichText::new(tr("No tasks here yet"))
                                    .size(16.0)
                                    .color(t.text2),
                            );
                            ui.add_space(6.0);
                            if primary_button(ui, &t, tr("Add Task")).clicked() {
                                self.open_quick_add(None);
                            }
                        }
                    });
                }
                for g in &groups {
                    self.list_group(ui, g, &tasks);
                    ui.add_space(12.0);
                }
                self.finish_row_drag(ui);
                ui.add_space(40.0);
            });
    }

    fn group_pill(&self, ui: &mut Ui, g: &Group) {
        match &g.status {
            Some(st) => {
                status_pill(ui, st, Sense::hover());
            }
            None => {
                let fake = Status {
                    name: g.label.clone(),
                    color: [g.color.r(), g.color.g(), g.color.b()],
                    kind: StatusKind::Custom,
                };
                status_pill(ui, &fake, Sense::hover());
            }
        }
    }

    fn list_group(&mut self, ui: &mut Ui, g: &Group, tasks: &[Task]) {
        let t = self.t();
        self.row_groups.push(g.clone());
        let collapsed = self.collapsed.contains(&g.key);
        let w = ui.available_width();
        let (hr, hresp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
        hresp.context_menu(|ui| self.group_menu(ui, g));
        let arrow = Rect::from_center_size(hr.left_center() + vec2(14.0, 0.0), vec2(18.0, 18.0));
        let ar = ui.interact(arrow, ui.id().with(("garrow", &g.key)), Sense::click());
        paint_triangle(
            ui.painter(),
            arrow.center(),
            !collapsed,
            if ar.hovered() { t.text } else { t.text2 },
        );
        if ar.clicked() {
            if collapsed {
                self.collapsed.remove(&g.key);
            } else {
                self.collapsed.insert(g.key.clone());
            }
        }
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_max(pos2(hr.left() + 28.0, hr.top()), hr.max))
                .layout(Layout::left_to_right(Align::Center)),
        );
        self.group_pill(&mut child, g);
        child.label(RichText::new(g.tasks.len().to_string()).color(t.text2));
        // "..." group menu: always laid out so its menu stays open while
        // the pointer moves into it, drawn only on hover
        let (dr, dresp) = child.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
        name_it(&dresp, &format!("Group options {}", g.label));
        let show_dots = hresp.hovered() || child.ui_contains_pointer();
        if show_dots {
            if dresp.hovered() {
                child.painter().rect_filled(dr, 5.0, t.hover);
            }
            child.painter().text(
                dr.center(),
                Align2::CENTER_CENTER,
                "\u{22EF}",
                FontId::proportional(14.0),
                t.text2,
            );
        }
        click_menu(&dresp, |ui| self.group_menu(ui, g));
        if show_dots {
            if child
                .add(ghost(egui::Button::new(
                    RichText::new(tr("+ Add Task")).color(t.text3).size(12.5),
                )))
                .clicked()
            {
                self.adding = Some((g.key.clone(), String::new()));
                self.collapsed.remove(&g.key);
            }
        }
        if collapsed {
            return;
        }
        let rows: Vec<&Task> = g
            .tasks
            .iter()
            .filter_map(|id| tasks.iter().find(|x| x.id == *id))
            .collect();
        for task in rows {
            self.task_row(ui, task, 0);
            if self.subtasks_open || self.expanded_tasks.contains(&task.id) {
                // a subtask that is a row of its own here (my subtasks on My
                // Tasks) isn't shown a second time under its parent
                let subs: Vec<Task> = self
                    .subtasks(task.id)
                    .into_iter()
                    .filter(|s| !tasks.iter().any(|x| x.id == s.id))
                    .cloned()
                    .collect();
                for s in &subs {
                    self.task_row(ui, s, 1);
                }
            }
            if self.sub_adding.as_ref().is_some_and(|(p, _)| *p == task.id) {
                self.sub_add_row(ui, task);
            }
        }
        let w = ui.available_width();
        let adding_here = self
            .adding
            .as_ref()
            .map(|(k, _)| k == &g.key)
            .unwrap_or(false);
        if adding_here {
            let mut text = self.adding.as_ref().unwrap().1.clone();
            let mut submit = false;
            let mut cancel = false;
            ui.horizontal(|ui| {
                ui.add_space(36.0);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .hint_text(tr("Task Name"))
                        .desired_width((w - 240.0).max(200.0)),
                );
                focus_once(ui, &r);
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                    r.request_focus();
                } else if r.lost_focus() && text.trim().is_empty() {
                    // clicking away from an empty row closes it
                    cancel = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    cancel = true;
                }
                if primary_button(ui, &t, tr("Save")).clicked() {
                    submit = true;
                }
            });
            if submit && !text.trim().is_empty() {
                self.add_from_group(g, text.clone(), false);
                self.adding = Some((g.key.clone(), String::new()));
            } else if cancel || (submit && text.trim().is_empty()) {
                self.adding = None;
            } else {
                self.adding = Some((g.key.clone(), text));
            }
        } else {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
            name_it(&resp, &format!("Add Task in {}", g.label));
            if resp.hovered() {
                ui.painter().rect_filled(r, 4.0, t.hover);
            }
            ui.painter().text(
                r.left_center() + vec2(40.0, 0.0),
                Align2::LEFT_CENTER,
                tr("+ Add Task"),
                FontId::proportional(13.5),
                t.text3,
            );
            if resp.clicked() {
                self.adding = Some((g.key.clone(), String::new()));
            }
        }
    }

    fn task_row(&mut self, ui: &mut Ui, task: &Task, depth: usize) {
        let t = self.t();
        let w = ui.available_width();
        let row_h = if self.data.extra.labs.iter().any(|l| l == "compact") {
            30.0
        } else {
            36.0
        };
        // drag a row to put it in a new place (Sort: Manual) or group
        let (rect, resp) = ui.allocate_exact_size(vec2(w, row_h), Sense::click_and_drag());
        name_it(&resp, &format!("Task {}", task.name));
        let id = ui.id().with(("row", task.id));
        let apps = self.data.extra.apps.clone();
        self.row_spots.push(super::listview::RowSpot {
            id: task.id,
            parent: task.parent,
            rect,
            group: self.row_groups.len().saturating_sub(1),
        });
        if resp.drag_started() && self.row_rename.is_none() {
            self.row_drag = Some(task.id);
        }
        if self.selected.contains(&task.id) {
            ui.painter().rect_filled(rect, 0.0, t.selected_bg);
        } else if resp.hovered() || self.row_drag == Some(task.id) {
            ui.painter().rect_filled(rect, 0.0, t.hover);
        }
        if self.row_focus == Some(task.id) {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                3.0,
                Stroke::new(1.5_f32, t.primary),
                egui::StrokeKind::Inside,
            );
        }
        let row_hover = (ui.rect_contains_pointer(rect) && self.row_drag.is_none())
            || Self::quick_tags_open(ui, task.id);
        let renaming = self.row_rename.as_ref().is_some_and(|(r, _)| *r == task.id);
        ui.painter().hline(
            rect.left() + 28.0..=rect.right(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, t.border),
        );
        let cols = self.list_cols(rect);
        let st = self.data.status_of(task);
        let closed = matches!(st.kind, StatusKind::Closed | StatusKind::Done);
        let mut x = rect.left() + 14.0 + depth as f32 * 26.0;
        let subs: Vec<Task> = self.subtasks(task.id).into_iter().cloned().collect();
        let nsub = subs.len();
        if nsub > 0 {
            let ar = Rect::from_center_size(pos2(x, rect.center().y), vec2(16.0, 16.0));
            let a = ui.interact(ar, id.with("exp"), Sense::click());
            let open = self.subtasks_open || self.expanded_tasks.contains(&task.id);
            if a.hovered() || resp.hovered() || open {
                paint_triangle(
                    ui.painter(),
                    ar.center(),
                    open,
                    if a.hovered() { t.text } else { t.text3 },
                );
            }
            if a.clicked() {
                if self.expanded_tasks.contains(&task.id) {
                    self.expanded_tasks.remove(&task.id);
                } else {
                    self.expanded_tasks.insert(task.id);
                }
            }
        }
        x += 14.0;
        let cr = Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(18.0, 18.0));
        let c = ui.interact(cr, id.with("st"), Sense::click());
        name_it(&c, &format!("Status of {}", task.name));
        paint_status_circle(
            ui.painter(),
            cr.center(),
            if c.hovered() { 7.5 } else { 7.0 },
            &st,
        );
        let statuses = self.data.statuses_for_list(task.list_id);
        let mut edited = task.clone();
        let mut changed = false;
        if let Some(true) = popup(ui, &c, id.with("stp"), 200.0, |ui| {
            status_menu(ui, &t, &statuses, &mut edited.status)
        }) {
            changed = true;
        }
        x += 26.0;
        let name_col = if closed { t.text3 } else { t.text };
        // the name takes the room up to the columns, minus the counts drawn
        // after it; cut by measured width, not by a guessed character count
        // (room for the quick action buttons while hovered)
        let quick_w = if row_hover && !renaming { 72.0 } else { 0.0 };
        let after = if nsub > 0 { 34.0 } else { 0.0 }
            + if task.checklist.is_empty() { 0.0 } else { 42.0 }
            + 12.0
            + quick_w;
        let max_name = (cols.left - x - after).max(60.0);
        if renaming {
            let nr = Rect::from_min_max(
                pos2(x - 4.0, rect.top() + 4.0),
                pos2((cols.left - 12.0).max(x + 120.0), rect.bottom() - 4.0),
            );
            self.row_rename_field(ui, nr, task);
            x = nr.right() + 10.0;
        } else {
            let name = fit_text(
                ui.painter(),
                &task.name,
                &FontId::proportional(14.0),
                max_name,
            );
            let galley = ui
                .painter()
                .layout_no_wrap(name, FontId::proportional(14.0), name_col);
            let gw = galley.size().x;
            ui.painter().galley(
                pos2(x, rect.center().y - galley.size().y / 2.0),
                galley,
                name_col,
            );
            if closed {
                ui.painter()
                    .hline(x..=x + gw, rect.center().y, Stroke::new(1.0, t.text3));
            }
            x += gw + 10.0;
        }
        if nsub > 0 {
            ui.painter().text(
                pos2(x, rect.center().y),
                Align2::LEFT_CENTER,
                format!("\u{2934} {}", nsub),
                FontId::proportional(12.0),
                t.text3,
            );
            x += 34.0;
        }
        if !task.checklist.is_empty() {
            let done = task.checklist.iter().filter(|c| c.done).count();
            ui.painter().text(
                pos2(x, rect.center().y),
                Align2::LEFT_CENTER,
                format!("\u{2611} {}/{}", done, task.checklist.len()),
                FontId::proportional(12.0),
                t.text3,
            );
            x += 42.0;
        }
        if quick_w > 0.0 {
            self.row_quick_actions(ui, rect, x, cols.left - 8.0, task);
            x += quick_w;
        }
        for tag in task.tags.iter().take(if apps.tags { 2 } else { 0 }) {
            let col = tag_color(tag);
            let g = ui
                .painter()
                .layout_no_wrap(tag.clone(), FontId::proportional(11.5), col);
            let r = Rect::from_min_size(
                pos2(x, rect.center().y - 9.0),
                vec2(g.size().x + 12.0, 18.0),
            );
            if r.right() < cols.left - 8.0 {
                ui.painter().rect_filled(r, 9.0, col.gamma_multiply(0.16));
                ui.painter().galley(r.center() - g.size() / 2.0, g, col);
            }
            x = r.right() + 4.0;
        }
        let icon_col = Color32::from_rgb(160, 160, 160);
        // assignee
        let ar = ui.interact(cols.assignee, id.with("as"), Sense::click());
        let used = paint_avatars(
            &ui.painter_at(cols.assignee),
            cols.assignee.left_center(),
            11.0,
            &task.assignees,
            &self.data,
            if resp.hovered() { t.hover } else { t.bg },
        );
        if used == 0.0 {
            paint_person_icon(
                &ui.painter_at(cols.assignee),
                cols.assignee.left_center() + vec2(12.0, 0.0),
                icon_col,
            );
        }
        let data = &self.data;
        if let Some(true) = popup(ui, &ar, id.with("asp"), 220.0, |ui| {
            assignee_menu(ui, &t, data, &mut edited.assignees)
        }) {
            changed = true;
        }
        // due date
        let dr = ui.interact(cols.due, id.with("du"), Sense::click());
        hover_bg(ui, &dr, cols.due.expand2(vec2(4.0, -6.0)), 4.0);
        name_it(&dr, &format!("Due date of {}", task.name));
        match task.due {
            Some(d) => {
                ui.painter_at(cols.due).text(
                    cols.due.left_center(),
                    Align2::LEFT_CENTER,
                    fmt_date(d),
                    FontId::proportional(13.5),
                    date_color(d, closed, &t),
                );
            }
            None => paint_calendar_icon(
                &ui.painter_at(cols.due),
                cols.due.left_center() + vec2(10.0, 0.0),
                icon_col,
            ),
        }
        if task.recur.is_some() && cols.due.width() > 0.0 {
            ui.painter_at(cols.due).text(
                cols.due.right_center() - vec2(10.0, 0.0),
                Align2::RIGHT_CENTER,
                "\u{1F501}",
                FontId::proportional(11.0),
                t.text3,
            );
        }
        if let Some(true) = popup(ui, &dr, id.with("dup"), 380.0, |ui| {
            dates_menu(
                ui,
                &t,
                id.with("dcal"),
                &mut edited.start,
                &mut edited.due,
                &mut edited.recur,
            )
        }) {
            changed = true;
        }
        // priority
        let pr = ui.interact(cols.priority, id.with("pr"), Sense::click());
        hover_bg(ui, &pr, cols.priority.expand2(vec2(4.0, -6.0)), 4.0);
        if !apps.priorities {
            // Priorities are turned off in Apps
        } else if task.priority > 0 {
            paint_flag(
                &ui.painter_at(cols.priority),
                cols.priority.left_center() + vec2(4.0, 0.0),
                priority_color(task.priority, &t),
                13.0,
            );
            ui.painter_at(cols.priority).text(
                cols.priority.left_center() + vec2(18.0, 0.0),
                Align2::LEFT_CENTER,
                tr(PRIORITIES[task.priority as usize].label),
                FontId::proportional(13.5),
                t.text,
            );
        } else {
            paint_outline_flag(
                &ui.painter_at(cols.priority),
                cols.priority.left_center() + vec2(8.0, 0.0),
                icon_col,
            );
        }
        if apps.priorities {
            if let Some(true) = popup(ui, &pr, id.with("prp"), 180.0, |ui| {
                priority_menu(ui, &t, &mut edited.priority)
            }) {
                changed = true;
            }
        }
        // status pill column
        let sr = ui.interact(cols.status, id.with("stcol"), Sense::click());
        hover_bg(ui, &sr, cols.status.expand2(vec2(4.0, -6.0)), 4.0);
        if cols.status.width() > 0.0 {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(cols.status)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            status_pill(&mut child, &st, Sense::hover());
        }
        if cols.created.width() > 0.0 {
            let created = chrono::DateTime::from_timestamp(task.created, 0)
                .map(|d| d.with_timezone(&chrono::Local).date_naive())
                .unwrap_or_else(today);
            ui.painter_at(cols.created).text(
                cols.created.left_center(),
                Align2::LEFT_CENTER,
                fmt_date(created),
                FontId::proportional(13.0),
                t.text2,
            );
        }
        for (name, r) in &cols.extra {
            let txt = self.prop_text(task, name);
            if txt.is_empty() {
                continue;
            }
            let p = ui.painter_at(r.shrink2(vec2(0.0, 2.0)));
            if *name == "Tags" {
                let mut x = r.left();
                for tag in &task.tags {
                    let g =
                        p.layout_no_wrap(tag.clone(), FontId::proportional(11.0), tag_color(tag));
                    let chip = Rect::from_min_size(
                        pos2(x, r.center().y - 9.0),
                        vec2(g.size().x + 12.0, 18.0),
                    );
                    p.rect_filled(chip, 4.0, tag_color(tag).gamma_multiply(0.16));
                    p.galley(chip.center() - g.size() / 2.0, g, tag_color(tag));
                    x = chip.right() + 4.0;
                }
            } else {
                let short = if *name == "Latest comment" {
                    truncate_chars(&txt, 26)
                } else {
                    txt
                };
                p.text(
                    r.left_center(),
                    Align2::LEFT_CENTER,
                    short,
                    FontId::proportional(13.0),
                    t.text2,
                );
            }
        }
        if let Some(true) = popup(ui, &sr, id.with("stcolp"), 200.0, |ui| {
            status_menu(ui, &t, &statuses, &mut edited.status)
        }) {
            changed = true;
        }
        // comments
        let comments = task.activity.iter().filter(|a| a.comment).count();
        paint_comment_icon(
            &ui.painter_at(cols.comments),
            cols.comments.left_center() + vec2(10.0, 0.0),
            icon_col,
        );
        if comments > 0 {
            ui.painter_at(cols.comments).text(
                cols.comments.left_center() + vec2(22.0, 0.0),
                Align2::LEFT_CENTER,
                comments.to_string(),
                FontId::proportional(12.5),
                t.text2,
            );
        }
        // custom field cells, edited in place by the field's kind
        for (fid, r) in &cols.fields {
            if self.field_cell(ui, *r, id.with(("cf", *fid)), task, *fid, &mut edited) {
                changed = true;
            }
        }
        let tid = task.id;
        // "..." at the end of the row, and the selection box at the start
        let dots =
            Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(22.0, 22.0));
        let dr = ui.interact(dots, id.with("dots"), Sense::click());
        name_it(&dr, &format!("Task options {}", task.name));
        if resp.hovered() || dr.hovered() {
            if dr.hovered() {
                ui.painter().rect_filled(dots, 5.0, t.border);
            }
            ui.painter().text(
                dots.center(),
                Align2::CENTER_CENTER,
                "\u{22EF}",
                FontId::proportional(14.0),
                t.text2,
            );
        }
        click_menu(&dr, |ui| self.task_menu(ui, tid));
        let ticked = self.row_checkbox(ui, rect, tid, resp.hovered());
        resp.context_menu(|ui| self.task_menu(ui, tid));
        if resp.clicked() && ui.input(|i| i.modifiers.command || i.modifiers.shift) {
            // Cmd click toggles the row; Shift click selects the range from
            // the last clicked row
            let shift = ui.input(|i| i.modifiers.shift);
            self.select_click(tid, shift);
            return;
        }
        if ticked {
            return;
        }
        if changed {
            self.actions.push(Action::Replace(edited));
        } else if resp.clicked() {
            self.actions.push(Action::Open(task.id));
        }
    }

    // ---------------------------------------------------------------- board view

    pub(super) fn board_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let tasks: Vec<Task> = self.filtered().into_iter().cloned().collect();
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = self.groups(&refs, true);
        let mut dropped: Option<(u64, usize)> = None;
        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for (gi, g) in groups.iter().enumerate() {
                        let open = g
                            .status
                            .as_ref()
                            .map(|s| s.kind == StatusKind::Open)
                            .unwrap_or(false);
                        let tint = if open || g.status.is_none() && g.color == t.text3 {
                            Color32::from_black_alpha(9)
                        } else {
                            g.color.gamma_multiply(0.07)
                        };
                        let accent = if open { t.text2 } else { g.color };
                        // a collapsed column is a narrow strip with the name running down it
                        let ckey = format!("board:{}", g.key);
                        let folded = self.collapsed.contains(&ckey);
                        let col_w = if folded { 44.0 } else { 252.0 };
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            let frame = egui::Frame::new()
                                .fill(tint)
                                .corner_radius(10)
                                .inner_margin(egui::Margin::same(8));
                            let (_, payload) = ui.dnd_drop_zone::<u64, ()>(frame, |ui| {
                                ui.set_width(col_w - 16.0);
                                if folded {
                                    let (r, resp) = ui.allocate_exact_size(
                                        vec2(
                                            col_w - 16.0,
                                            (ui.available_height() - 40.0).clamp(160.0, 420.0),
                                        ),
                                        Sense::click(),
                                    );
                                    name_it(&resp, &format!("Expand group {}", g.label));
                                    if resp.hovered() {
                                        ui.painter().rect_filled(
                                            r,
                                            6.0,
                                            Color32::from_black_alpha(8),
                                        );
                                    }
                                    let p = ui.painter();
                                    p.text(
                                        r.center_top() + vec2(0.0, 10.0),
                                        Align2::CENTER_CENTER,
                                        g.tasks.len().to_string(),
                                        FontId::proportional(12.5),
                                        accent,
                                    );
                                    let gal = p.layout_no_wrap(
                                        g.label.to_uppercase(),
                                        FontId::proportional(12.0),
                                        accent,
                                    );
                                    let at =
                                        pos2(r.center().x + gal.size().y / 2.0, r.top() + 24.0);
                                    p.add(
                                        egui::epaint::TextShape::new(at, gal, accent)
                                            .with_angle(std::f32::consts::FRAC_PI_2),
                                    );
                                    if resp.clicked() {
                                        self.collapsed.remove(&ckey);
                                    }
                                    return;
                                }
                                ui.horizontal(|ui| {
                                    self.group_pill(ui, g);
                                    ui.label(
                                        RichText::new(g.tasks.len().to_string()).color(accent),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let b = ui
                                            .add(ghost(egui::Button::new(
                                                RichText::new("\u{2039}").color(t.text3).size(15.0),
                                            )))
                                            .on_hover_text(tr("Collapse group"));
                                        name_it(&b, &format!("Collapse group {}", g.label));
                                        if b.clicked() {
                                            self.collapsed.insert(ckey.clone());
                                        }
                                    });
                                });
                                ui.add_space(4.0);
                                egui::ScrollArea::vertical()
                                    .id_salt(("col", gi))
                                    .auto_shrink([false, true])
                                    .max_height(ui.available_height() - 60.0)
                                    .show(ui, |ui| {
                                        for id in &g.tasks {
                                            if let Some(task) = tasks.iter().find(|x| x.id == *id) {
                                                let resp = self.board_card(ui, task, col_w - 16.0);
                                                name_it(&resp, &format!("Card {}", task.name));
                                                let tid = task.id;
                                                resp.context_menu(|ui| self.task_menu(ui, tid));
                                                if resp.clicked() {
                                                    self.actions.push(Action::Open(task.id));
                                                }
                                            }
                                        }
                                        self.board_add_task(ui, g, col_w - 16.0, accent);
                                    });
                            });
                            if let Some(p) = payload {
                                dropped = Some((*p, gi));
                            }
                        });
                    }
                    if self.group_by == GroupBy::Status {
                        if let Some(sid) = self
                            .data
                            .lists_in(self.loc)
                            .and_then(|l| l.first().copied())
                            .and_then(|l| self.data.space_of_list(l))
                            .map(|s| s.id)
                        {
                            if ui
                                .add(ghost(egui::Button::new(
                                    RichText::new(tr("+  Add group")).color(t.text2).size(14.0),
                                )))
                                .on_hover_text(tr("Edit statuses"))
                                .clicked()
                            {
                                self.actions.push(Action::EditStatuses(sid));
                            }
                        }
                    }
                });
            });
        if let Some((tid, gi)) = dropped {
            if let Some(mut task) = self.task(tid).cloned() {
                let g = &groups[gi];
                match self.group_by {
                    GroupBy::Status => {
                        if let Some(st) = &g.status {
                            let own = self.data.statuses_for_list(task.list_id);
                            if let Some(s) = own.iter().find(|s| s.name == st.name) {
                                task.status = s.name.clone();
                            }
                        }
                    }
                    GroupBy::Priority => task.priority = g.priority.unwrap_or(0),
                    GroupBy::Assignee => task.assignees = g.assignee.into_iter().collect(),
                    // the DONE column closes the task and keeps its date
                    GroupBy::DueDate if g.key == "d:done" => {
                        let own = self.data.statuses_for_list(task.list_id);
                        if let Some(s) = own
                            .iter()
                            .find(|s| s.kind == StatusKind::Done)
                            .or_else(|| own.iter().find(|s| s.kind == StatusKind::Closed))
                        {
                            task.status = s.name.clone();
                        }
                    }
                    GroupBy::DueDate => task.due = g.due,
                    GroupBy::None => {}
                    GroupBy::Tags | GroupBy::List => self.move_to_group(&mut task, g),
                }
                self.actions.push(Action::Replace(task));
            }
        }
    }

    /// "+ Add Task" at the foot of a board column: a name field, as in the
    /// List (a task is only made once it has a name).
    fn board_add_task(&mut self, ui: &mut Ui, g: &Group, w: f32, accent: Color32) {
        let t = self.t();
        let key = format!("board:{}", g.key);
        let here = self.adding.as_ref().is_some_and(|(k, _)| *k == key);
        if !here {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
            name_it(&resp, &format!("Add Task in {}", g.label));
            if resp.hovered() {
                ui.painter()
                    .rect_filled(r, 6.0, Color32::from_black_alpha(8));
            }
            ui.painter().text(
                r.left_center() + vec2(10.0, 0.0),
                Align2::LEFT_CENTER,
                tr("+  Add Task"),
                FontId::proportional(14.0),
                accent,
            );
            if resp.clicked() {
                self.adding = Some((key, String::new()));
            }
            return;
        }
        let mut text = self
            .adding
            .as_ref()
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        let (mut submit, mut cancel) = (false, false);
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(w - 18.0);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .hint_text(tr("Task Name"))
                        .desired_width(f32::INFINITY),
                );
                focus_once(ui, &r);
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                    r.request_focus();
                } else if r.lost_focus() && text.trim().is_empty() {
                    cancel = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel = true;
                }
                ui.horizontal(|ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        cancel = true;
                    }
                    if primary_button(ui, &t, tr("Save")).clicked() {
                        submit = true;
                    }
                });
            });
        if submit && !text.trim().is_empty() {
            self.add_from_group(g, text.trim().to_string(), false);
            self.adding = Some((key, String::new()));
        } else if cancel || submit {
            self.adding = None;
        } else {
            self.adding = Some((key, text));
        }
    }

    /// A board card that can be dragged to another column and clicked to
    /// open, with its assignee, date and priority boxes and "..." clickable
    /// on top. (egui's drag source covers what's inside it, so the card
    /// takes its drag and click under the boxes instead.)
    fn board_card(&mut self, ui: &mut Ui, task: &Task, w: f32) -> Response {
        let id = egui::Id::new(("card", task.id));
        let h_id = id.with("h");
        if ui.ctx().is_being_dragged(id) {
            egui::DragAndDrop::set_payload(ui.ctx(), task.id);
            let layer = egui::LayerId::new(egui::Order::Tooltip, id);
            let r = ui
                .scope_builder(egui::UiBuilder::new().layer_id(layer), |ui| {
                    self.card(ui, task, w, false)
                })
                .inner;
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                ui.ctx().transform_layer_shapes(
                    layer,
                    egui::emath::TSTransform::from_translation(p - r.center()),
                );
            }
            ui.add_space(6.0);
            return ui.interact(r, id, Sense::click_and_drag());
        }
        let h: f32 = ui.data(|d| d.get_temp(h_id)).unwrap_or(84.0);
        let rect = Rect::from_min_size(ui.cursor().min, vec2(w, h));
        let body = ui
            .interact(rect, id, Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab);
        let drawn = self.card(ui, task, w, true);
        if body.hovered() {
            // a darker outline says the card can be opened or picked up
            ui.painter().rect_stroke(
                drawn,
                8.0,
                Stroke::new(1.0, hover_line(ui)),
                egui::StrokeKind::Inside,
            );
        }
        ui.add_space(6.0);
        if (drawn.height() - h).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(h_id, drawn.height()));
            ui.ctx().request_repaint();
        }
        body
    }

    /// Draws a card and returns where it went. `live` makes the boxes and
    /// the "..." clickable (not on the copy that follows the mouse).
    fn card(&mut self, ui: &mut Ui, task: &Task, w: f32, live: bool) -> Rect {
        let t = self.t();
        let closed = self.data.is_closed(task);
        let cid = egui::Id::new(("cardp", task.id));
        let mut edited = task.clone();
        let mut changed = false;
        let mut open = false;
        let frame = egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(11))
            .show(ui, |ui| {
                // text on a card must not take the card's click or drag
                ui.style_mut().interaction.selectable_labels = false;
                ui.set_width(w - 24.0);
                if !matches!(self.loc, Location::List(_)) {
                    ui.label(
                        RichText::new(self.data.list_name(task.list_id))
                            .size(11.5)
                            .color(t.text3),
                    );
                }
                let mut name = RichText::new(&task.name).size(14.0).color(t.text);
                if closed {
                    name = name.strikethrough().color(t.text3);
                }
                ui.add(egui::Label::new(name).wrap());
                if !task.tags.is_empty() && self.data.extra.apps.tags {
                    ui.horizontal_wrapped(|ui| {
                        for tag in &task.tags {
                            open |= chip(ui, tag, tag_color(tag)).clicked();
                        }
                    });
                }
                ui.add_space(2.0);
                let (r, _) = ui.allocate_exact_size(vec2(w - 24.0, 22.0), Sense::hover());
                let p = ui.painter().clone();
                let icon_col = Color32::from_rgb(150, 150, 150);
                let boxed = |x: f32| Rect::from_min_size(pos2(x, r.top()), vec2(22.0, 22.0));
                let mut x = r.left();
                let used = paint_avatars(
                    &p,
                    pos2(x, r.center().y),
                    10.0,
                    &task.assignees,
                    &self.data,
                    t.card,
                );
                let a_rect = if used > 0.0 {
                    let b = Rect::from_min_size(pos2(x - 2.0, r.top()), vec2(used + 4.0, 22.0));
                    x += used + 6.0;
                    b
                } else {
                    let b = boxed(x);
                    p.rect_stroke(b, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                    paint_person_icon(&p, b.center(), icon_col);
                    x += 26.0;
                    b
                };
                let d_rect = match task.due {
                    Some(d) => {
                        let g = p.layout_no_wrap(
                            fmt_date(d),
                            FontId::proportional(12.5),
                            date_color(d, closed, &t),
                        );
                        let b =
                            Rect::from_min_size(pos2(x, r.top()), vec2(g.size().x + 26.0, 22.0));
                        p.rect_stroke(b, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                        paint_calendar_icon(&p, b.left_center() + vec2(11.0, 0.0), icon_col);
                        let gw = g.size().x;
                        p.galley(
                            pos2(b.left() + 20.0, b.center().y - g.size().y / 2.0),
                            g,
                            t.text2,
                        );
                        x += gw + 30.0;
                        b
                    }
                    None => {
                        let b = boxed(x);
                        p.rect_stroke(b, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                        paint_calendar_icon(&p, b.center(), icon_col);
                        x += 26.0;
                        b
                    }
                };
                let b = boxed(x);
                let show_pr = self.data.extra.apps.priorities;
                if show_pr {
                    p.rect_stroke(b, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                }
                if !show_pr {
                } else if task.priority > 0 {
                    paint_flag(
                        &p,
                        b.center() + vec2(-3.0, 0.0),
                        priority_color(task.priority, &t),
                        12.0,
                    );
                } else {
                    paint_outline_flag(&p, b.center(), icon_col);
                }
                let nsub = self.subtasks(task.id).len();
                if nsub > 0 {
                    p.text(
                        pos2(b.right() + 8.0, r.center().y),
                        Align2::LEFT_CENTER,
                        format!("\u{2934} {}", nsub),
                        FontId::proportional(12.0),
                        t.text3,
                    );
                }
                if !live {
                    return;
                }
                // the same pickers as a List row's cells
                let ar = ui.interact(a_rect, cid.with("as"), Sense::click());
                hover_bg(ui, &ar, a_rect, 4.0);
                name_it(&ar, &format!("Card assignees {}", task.name));
                let data = &self.data;
                if let Some(true) = popup(ui, &ar, cid.with("asp"), 220.0, |ui| {
                    assignee_menu(ui, &t, data, &mut edited.assignees)
                }) {
                    changed = true;
                }
                let dr = ui.interact(d_rect, cid.with("du"), Sense::click());
                hover_bg(ui, &dr, d_rect, 4.0);
                name_it(&dr, &format!("Card due date {}", task.name));
                if let Some(true) = popup(ui, &dr, cid.with("dup"), 380.0, |ui| {
                    dates_menu(
                        ui,
                        &t,
                        cid.with("dcal"),
                        &mut edited.start,
                        &mut edited.due,
                        &mut edited.recur,
                    )
                }) {
                    changed = true;
                }
                if show_pr {
                    let pr = ui.interact(b, cid.with("pr"), Sense::click());
                    hover_bg(ui, &pr, b, 4.0);
                    name_it(&pr, &format!("Card priority {}", task.name));
                    if let Some(true) = popup(ui, &pr, cid.with("prp"), 180.0, |ui| {
                        priority_menu(ui, &t, &mut edited.priority)
                    }) {
                        changed = true;
                    }
                }
            });
        let rect = frame.response.rect;
        if live {
            // "..." in the corner on hover opens the task menu
            let dots =
                Rect::from_center_size(rect.right_top() + vec2(-16.0, 16.0), vec2(22.0, 22.0));
            let dr = ui.interact(dots, cid.with("dots"), Sense::click());
            name_it(&dr, &format!("Card options {}", task.name));
            if ui.rect_contains_pointer(rect) || dr.hovered() {
                ui.painter()
                    .rect_filled(dots, 5.0, if dr.hovered() { t.border } else { t.card });
                ui.painter().text(
                    dots.center(),
                    Align2::CENTER_CENTER,
                    "\u{22EF}",
                    FontId::proportional(14.0),
                    t.text2,
                );
            }
            let tid = task.id;
            click_menu(&dr, |ui| self.task_menu(ui, tid));
        }
        if changed {
            self.actions.push(Action::Replace(edited));
        } else if open {
            self.actions.push(Action::Open(task.id));
        }
        rect
    }
}

fn truncate_chars(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}\u{2026}", s.chars().take(n).collect::<String>())
    }
}
