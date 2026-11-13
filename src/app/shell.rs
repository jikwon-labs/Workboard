// The window shell measured from the person's running ClickUp: black icon
// rail, light top bar with back/forward and search, the Home sidebar (with
// Channels and Direct Messages), the page header with coloured view tabs and
// pill toolbar, and the List and Board views. See MECHANICS.md.

use super::*;
use egui::{Pos2, Response};
use chrono::Datelike;

// ------------------------------------------------------------------ icons

pub(super) fn paint_dashed_circle(p: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let n = 10;
    for i in 0..n {
        let a0 = std::f32::consts::TAU * (i as f32 / n as f32);
        let a1 = a0 + std::f32::consts::TAU / n as f32 * 0.55;
        p.line_segment([c + vec2(a0.cos(), a0.sin()) * r, c + vec2(a1.cos(), a1.sin()) * r], Stroke::new(1.4, color));
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
    p.line_segment([pos2(r.left(), r.top() + 3.5), pos2(r.right(), r.top() + 3.5)], Stroke::new(1.2, color));
    p.line_segment([pos2(r.left() + 3.0, r.top() - 1.5), pos2(r.left() + 3.0, r.top() + 1.0)], Stroke::new(1.2, color));
    p.line_segment([pos2(r.right() - 3.0, r.top() - 1.5), pos2(r.right() - 3.0, r.top() + 1.0)], Stroke::new(1.2, color));
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
    p.line_segment([pos2(r.left() + 3.0, r.bottom()), pos2(r.left() + 2.0, r.bottom() + 3.0)], Stroke::new(1.2, color));
}

fn paint_outline_flag(p: &egui::Painter, c: Pos2, color: Color32) {
    let top = c + vec2(-4.0, -6.0);
    p.line_segment([top, top + vec2(0.0, 12.0)], Stroke::new(1.2, color));
    p.add(egui::Shape::closed_line(vec![top, top + vec2(8.5, 0.5), top + vec2(6.5, 3.5), top + vec2(8.5, 6.5), top + vec2(0.0, 6.5)], Stroke::new(1.2, color)));
}

/// Coloured view icons as in the original's tabs.
fn paint_view_icon(p: &egui::Painter, c: Pos2, v: ViewKind) {
    let (col, glyph) = match v {
        ViewKind::Overview => {
            let r = Rect::from_center_size(c, vec2(14.0, 14.0));
            p.rect_filled(r, 3.0, Color32::from_rgb(100, 100, 100));
            p.text(r.center(), Align2::CENTER_CENTER, "i", FontId::proportional(10.0), Color32::WHITE);
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
        p.rect_stroke(r.shrink(0.5), 3.0, Stroke::new(1.2, col), egui::StrokeKind::Inside);
        for k in 0..3 {
            let y = r.top() + 4.0 + k as f32 * 3.0;
            p.line_segment([pos2(r.left() + 3.5, y), pos2(r.right() - 3.5, y)], Stroke::new(1.1, col));
        }
        return;
    }
    p.rect_filled(r, 3.0, col);
    let w = Stroke::new(1.3, Color32::WHITE);
    match glyph {
        1 => {
            p.line_segment([pos2(r.left() + 4.5, r.top() + 3.5), pos2(r.left() + 4.5, r.bottom() - 3.5)], w);
            p.line_segment([pos2(r.right() - 4.5, r.top() + 3.5), pos2(r.right() - 4.5, r.bottom() - 6.0)], w);
        }
        2 => {
            p.text(r.center() + vec2(0.0, 1.0), Align2::CENTER_CENTER, "31", FontId::proportional(7.5), Color32::WHITE);
        }
        3 => {
            p.line_segment([pos2(r.left() + 3.0, r.top() + 4.5), pos2(r.left() + 8.0, r.top() + 4.5)], w);
            p.line_segment([pos2(r.left() + 5.0, r.top() + 7.5), pos2(r.right() - 3.0, r.top() + 7.5)], w);
            p.line_segment([pos2(r.left() + 3.0, r.top() + 10.5), pos2(r.left() + 9.0, r.top() + 10.5)], w);
        }
        _ => {
            p.line_segment([pos2(r.left() + 3.0, r.center().y), pos2(r.right() - 3.0, r.center().y)], w);
            p.line_segment([pos2(r.center().x, r.top() + 3.0), pos2(r.center().x, r.bottom() - 3.0)], w);
        }
    }
}

/// A rounded pill button with an outline, like the original's toolbar.
fn pill(ui: &mut Ui, t: &Theme, text: &str, active: bool, accent: bool) -> Response {
    let font = FontId::proportional(13.0);
    let (fg, bg, stroke) = if accent {
        (t.accent_text, t.accent_bg, t.accent_bg)
    } else if active {
        (t.text, t.row_selected, t.border)
    } else {
        (t.text2, t.bg, t.border)
    };
    let g = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 26.0), Sense::click());
    name_it(&resp, text);
    let fill = if resp.hovered() && !accent { t.hover } else { bg };
    ui.painter().rect_filled(r, 13.0, fill);
    ui.painter().rect_stroke(r, 13.0, Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    ui.painter().galley(r.center() - g.size() / 2.0, g, fg);
    resp
}

/// Thin outline icons in the style of the original (1.5 px strokes).
pub(super) fn line_icon(p: &egui::Painter, kind: &str, c: Pos2, color: Color32) -> bool {
    let s = Stroke::new(1.5, color);
    let line = |a: egui::Vec2, b: egui::Vec2| p.line_segment([c + a, c + b], s);
    match kind {
        "home" => {
            let pts = vec![c + vec2(-7.0, -1.0), c + vec2(0.0, -7.5), c + vec2(7.0, -1.0), c + vec2(7.0, 7.0), c + vec2(-7.0, 7.0)];
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
            p.text(c + vec2(0.0, 2.8), Align2::CENTER_CENTER, today().day().to_string(), FontId::proportional(7.5), color);
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
                    p.circle_filled(c + vec2(-5.0 + 5.0 * i as f32, -5.0 + 5.0 * j as f32), 1.5, color);
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
            let pts = vec![c + vec2(-5.5, -7.5), c + vec2(2.0, -7.5), c + vec2(5.5, -4.0), c + vec2(5.5, 7.5), c + vec2(-5.5, 7.5)];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-2.5, 0.0), vec2(2.5, 0.0));
            line(vec2(-2.5, 3.5), vec2(2.5, 3.5));
        }
        "dashboards" => {
            p.rect_stroke(Rect::from_center_size(c, vec2(15.0, 15.0)), 3.0, s, egui::StrokeKind::Middle);
            line(vec2(-3.0, 4.0), vec2(-3.0, 0.0));
            line(vec2(0.5, 4.0), vec2(0.5, -3.5));
            line(vec2(4.0, 4.0), vec2(4.0, 1.5));
        }
        "whiteboards" => {
            p.rect_stroke(Rect::from_center_size(c, vec2(15.0, 15.0)), 3.0, s, egui::StrokeKind::Middle);
            let pts = vec![c + vec2(-4.0, 2.0), c + vec2(-1.0, -2.0), c + vec2(1.5, 2.0), c + vec2(4.0, -2.5)];
            p.add(egui::Shape::line(pts, s));
        }
        "forms" => {
            p.rect_stroke(Rect::from_center_size(c + vec2(0.0, 0.5), vec2(13.0, 15.0)), 3.0, s, egui::StrokeKind::Middle);
            line(vec2(-3.0, 1.0), vec2(-0.8, 3.2));
            line(vec2(-0.8, 3.2), vec2(3.5, -1.5));
            line(vec2(-2.5, -7.5), vec2(2.5, -7.5));
        }
        "goals" => {
            let pts = vec![c + vec2(-5.0, -7.0), c + vec2(5.0, -7.0), c + vec2(4.0, -1.0), c + vec2(0.0, 2.0), c + vec2(-4.0, -1.0)];
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
        "install" => {
            line(vec2(0.0, -7.0), vec2(0.0, 3.0));
            line(vec2(-3.5, -0.5), vec2(0.0, 3.0));
            line(vec2(3.5, -0.5), vec2(0.0, 3.0));
            let pts = vec![c + vec2(-7.0, 2.0), c + vec2(-7.0, 7.0), c + vec2(7.0, 7.0), c + vec2(7.0, 2.0)];
            p.add(egui::Shape::line(pts, s));
        }
        "upgrade" => {
            let pts = vec![c + vec2(0.0, -8.0), c + vec2(7.0, -1.0), c + vec2(3.0, -1.0), c + vec2(3.0, 6.0), c + vec2(-3.0, 6.0), c + vec2(-3.0, -1.0), c + vec2(-7.0, -1.0)];
            p.add(egui::Shape::convex_polygon(pts, Color32::from_rgb(171, 140, 255), Stroke::NONE));
            p.rect_filled(Rect::from_center_size(c + vec2(0.0, 8.0), vec2(6.0, 1.6)), 0.5, Color32::from_rgb(171, 140, 255));
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
            p.rect_stroke(Rect::from_center_size(c + vec2(-2.0, 0.0), vec2(12.0, 10.0)), 2.5, s, egui::StrokeKind::Middle);
            let pts = vec![c + vec2(4.0, -1.5), c + vec2(8.0, -4.0), c + vec2(8.0, 4.0), c + vec2(4.0, 1.5)];
            p.add(egui::Shape::line(pts, s));
        }
        "mic" => {
            p.rect_stroke(Rect::from_center_size(c + vec2(0.0, -2.5), vec2(6.0, 10.0)), 3.0, s, egui::StrokeKind::Middle);
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (i as f32 / 10.0);
                arc.push(c + vec2(0.0, -1.0) + vec2(a.cos() * 5.5, a.sin() * 5.5));
            }
            p.add(egui::Shape::line(arc, s));
            line(vec2(0.0, 4.5), vec2(0.0, 7.5));
        }
        "phone" => {
            let pts = vec![c + vec2(-6.0, -6.0), c + vec2(-3.0, -7.0), c + vec2(-1.0, -3.0), c + vec2(-3.0, -1.5), c + vec2(1.5, 3.0), c + vec2(3.0, 1.0), c + vec2(7.0, 3.0), c + vec2(6.0, 6.0), c + vec2(3.0, 7.0), c + vec2(-6.5, -2.5)];
            p.add(egui::Shape::closed_line(pts, s));
        }
        "robot" => {
            p.rect_stroke(Rect::from_center_size(c + vec2(0.0, 1.5), vec2(13.0, 10.0)), 3.0, s, egui::StrokeKind::Middle);
            p.circle_filled(c + vec2(-2.5, 1.5), 1.2, color);
            p.circle_filled(c + vec2(2.5, 1.5), 1.2, color);
            line(vec2(0.0, -3.5), vec2(0.0, -6.5));
            p.circle_filled(c + vec2(0.0, -7.0), 1.2, color);
        }
        "edit" => {
            p.rect_stroke(Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(12.0, 12.0)), 2.5, s, egui::StrokeKind::Middle);
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
            p.rect_stroke(Rect::from_center_size(c, vec2(15.0, 13.0)), 3.0, s, egui::StrokeKind::Middle);
            let pts = vec![c + vec2(-7.5, 1.0), c + vec2(-3.0, 1.0), c + vec2(-2.0, 3.0), c + vec2(2.0, 3.0), c + vec2(3.0, 1.0), c + vec2(7.5, 1.0)];
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
            p.rect_stroke(Rect::from_center_size(c + vec2(5.0, 3.0), vec2(5.0, 4.0)), 1.0, Stroke::new(1.2, color), egui::StrokeKind::Middle);
        }
        "megaphone" => {
            let pts = vec![c + vec2(-6.0, -2.0), c + vec2(3.0, -6.5), c + vec2(3.0, 6.5), c + vec2(-6.0, 2.0)];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-4.0, 2.0), vec2(-2.5, 7.0));
            line(vec2(5.5, -2.0), vec2(5.5, 2.0));
        }
        "bolt" => {
            let pts = vec![c + vec2(1.5, -8.0), c + vec2(-5.0, 1.0), c + vec2(0.0, 1.0), c + vec2(-1.5, 8.0), c + vec2(5.0, -1.0), c + vec2(0.0, -1.0)];
            p.add(egui::Shape::closed_line(pts, s));
        }
        "heart" => {
            let col = Color32::from_rgb(233, 61, 130);
            p.circle_filled(c + vec2(-3.0, -2.0), 3.6, Color32::from_rgb(171, 74, 186));
            p.circle_filled(c + vec2(3.0, -2.0), 3.6, col);
            p.add(egui::Shape::convex_polygon(vec![c + vec2(-6.4, -0.8), c + vec2(6.4, -0.8), c + vec2(0.0, 6.5)], col, Stroke::NONE));
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

pub(super) struct Cols {
    pub assignee: Rect,
    pub due: Rect,
    pub priority: Rect,
    pub status: Rect,
    pub comments: Rect,
}

pub(super) fn col_layout(row: Rect) -> Cols {
    let right = row.right() - 40.0;
    let col = |l: f32, w: f32| Rect::from_min_max(pos2(l, row.top()), pos2(l + w, row.bottom()));
    let comments = col(right - 90.0, 90.0);
    let status = col(comments.left() - 130.0, 130.0);
    let priority = col(status.left() - 100.0, 100.0);
    let due = col(priority.left() - 100.0, 100.0);
    let assignee = col(due.left() - 100.0, 100.0);
    Cols { assignee, due, priority, status, comments }
}

impl App {
    // ---------------------------------------------------------------- history

    pub(super) fn track_history(&mut self) {
        let cur = (self.section, self.loc);
        match self.nav_last {
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
        if let Some(prev) = self.nav_back.pop() {
            self.nav_fwd.push((self.section, self.loc));
            self.section = prev.0;
            self.loc = prev.1;
            self.nav_skip = true;
        }
    }

    pub(super) fn go_forward(&mut self) {
        if let Some(next) = self.nav_fwd.pop() {
            self.nav_back.push((self.section, self.loc));
            self.section = next.0;
            self.loc = next.1;
            self.nav_skip = true;
        }
    }

    // ---------------------------------------------------------------- top bar

    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("top")
            .exact_height(40.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(t.app_bg).inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                // empty header space moves the window (there is no title bar)
                let drag = ui.interact(full.expand(6.0), ui.id().with("drag_header"), Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                }
                // workspace picker
                let name = self.data.workspace.clone();
                let g = ui.painter().layout_no_wrap(name, FontId::proportional(14.0), t.text);
                let left = if cfg!(target_os = "macos") { 74.0 } else { 4.0 };
                let wr = Rect::from_min_size(pos2(full.left() + left, full.center().y - 14.0), vec2(g.size().x + 58.0, 28.0));
                let wresp = ui.interact(wr, ui.id().with("ws_pill"), Sense::click());
                name_it(&wresp, "Workspace menu");
                ui.painter().rect_filled(wr, 7.0, if wresp.hovered() { t.hover } else { t.sidebar });
                let av = Rect::from_center_size(wr.left_center() + vec2(15.0, 0.0), vec2(18.0, 18.0));
                ui.painter().rect_filled(av, 4.0, rgb(self.data.extra.workspace_color));
                let letter = self.data.workspace.chars().next().unwrap_or('W').to_uppercase().to_string();
                ui.painter().text(av.center(), Align2::CENTER_CENTER, letter, FontId::proportional(11.0), Color32::WHITE);
                ui.painter().galley(pos2(wr.left() + 30.0, wr.center().y - g.size().y / 2.0), g, t.text);
                let cv = wr.right_center() - vec2(14.0, 1.0);
                ui.painter().line_segment([cv + vec2(-4.0, -2.0), cv + vec2(0.0, 2.0)], Stroke::new(1.3, t.text2));
                ui.painter().line_segment([cv + vec2(0.0, 2.0), cv + vec2(4.0, -2.0)], Stroke::new(1.3, t.text2));
                if self.force_menu.as_deref() == Some("workspace") { let pid = ui.id().with("ws_menu"); ui.memory_mut(|m| m.open_popup(pid)); }
                popup(ui, &wresp, ui.id().with("ws_menu"), 300.0, |ui| self.workspace_menu(ui));
                // planner shortcut
                let cr = Rect::from_center_size(pos2(wr.right() + 18.0, full.center().y), vec2(28.0, 28.0));
                let cresp = ui.interact(cr, ui.id().with("cal_btn"), Sense::click());
                name_it(&cresp, "Planner shortcut");
                if cresp.hovered() {
                    ui.painter().rect_filled(cr, 6.0, t.hover);
                }
                paint_calendar_icon(ui.painter(), cr.center(), t.text2);
                ui.painter().text(cr.center() + vec2(0.0, 2.0), Align2::CENTER_CENTER, today().day().to_string(), FontId::proportional(6.5), t.text2);
                if cresp.on_hover_text("Planner").clicked() {
                    self.section = Section::Planner;
                }
                // back / forward + search with AI Chats
                let sw = 287.0f32.min(full.width() * 0.25).max(220.0);
                let sr = Rect::from_center_size(full.center() + vec2(40.0, 0.0), vec2(sw, 28.0));
                for (i, (glyph, enabled, tip)) in [("\u{2190}", !self.nav_back.is_empty(), "Go back"), ("\u{2192}", !self.nav_fwd.is_empty(), "Go forward")].into_iter().enumerate() {
                    let ar = Rect::from_center_size(pos2(sr.left() - 40.0 + i as f32 * 24.0, full.center().y), vec2(24.0, 24.0));
                    let a = ui.interact(ar, ui.id().with(("nav", i)), Sense::click());
                    name_it(&a, tip);
                    if a.hovered() && enabled {
                        ui.painter().rect_filled(ar, 5.0, t.hover);
                    }
                    ui.painter().text(ar.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(15.0), if enabled { t.text2 } else { Color32::from_rgb(190, 190, 190) });
                    if a.on_hover_text(tip).clicked() && enabled {
                        if i == 0 {
                            self.go_back();
                        } else {
                            self.go_forward();
                        }
                    }
                }
                let ai_r = Rect::from_min_max(pos2(sr.right() - 94.0, sr.top() + 4.0), pos2(sr.right() - 4.0, sr.bottom() - 4.0));
                let sresp = ui.interact(Rect::from_min_max(sr.min, pos2(ai_r.left(), sr.bottom())), ui.id().with("search_pill"), Sense::click());
                name_it(&sresp, "Search");
                ui.painter().rect_filled(sr, 14.0, if sresp.hovered() { t.hover } else { t.bg });
                ui.painter().rect_stroke(sr, 14.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                ui.painter().text(sr.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, "\u{1F50D}", FontId::proportional(12.0), t.text2);
                let sg = ui.painter().layout_no_wrap("Search".into(), FontId::proportional(14.0), t.text3);
                let sgw = sg.size().x;
                ui.painter().galley(pos2(sr.left() + 32.0, sr.center().y - sg.size().y / 2.0), sg, t.text3);
                ui.painter().text(pos2(sr.left() + 38.0 + sgw, sr.center().y), Align2::LEFT_CENTER, "\u{2318}J", FontId::proportional(11.5), Color32::from_rgb(170, 170, 170));
                if sresp.clicked() {
                    self.palette = Some(String::new());
                }
                let air = ui.interact(ai_r, ui.id().with("ai_chats"), Sense::click());
                name_it(&air, "AI Chats");
                ui.painter().rect_filled(ai_r, 10.0, if air.hovered() { t.hover } else { t.sidebar });
                ui.painter().text(ai_r.center() - vec2(8.0, 0.0), Align2::CENTER_CENTER, "AI Chats", FontId::proportional(13.0), t.text2);
                ui.painter().text(ai_r.right_center() - vec2(10.0, 0.0), Align2::CENTER_CENTER, "\u{2728}", FontId::proportional(11.0), Color32::from_rgb(171, 74, 186));
                if air.clicked() {
                    self.section = Section::Ai;
                }
                // right part: pinned personal tools, avatar menu
                let mut right = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(full.right() - 480.0, full.top()), full.max)).layout(Layout::right_to_left(Align::Center)));
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
                    ui.painter().circle_filled(c + vec2(9.0, 9.0), 4.0, t.app_bg);
                    ui.painter().circle_filled(c + vec2(9.0, 9.0), 2.8, if muted { Color32::from_rgb(0xe1, 0x6b, 0x16) } else { Color32::from_rgb(48, 164, 108) });
                    let cv = r.right_center() - vec2(8.0, 1.0);
                    ui.painter().line_segment([cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5)], Stroke::new(1.2, t.text2));
                    ui.painter().line_segment([cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)], Stroke::new(1.2, t.text2));
                    if self.force_menu.as_deref() == Some("avatar") { let pid = ui.id().with("avatar_menu"); ui.memory_mut(|m| m.open_popup(pid)); }
                popup(ui, &resp, ui.id().with("avatar_menu"), 270.0, |ui| self.avatar_menu(ui));
                }
                ui.add_space(6.0);
                let pinned = self.data.extra.pinned_tools.clone();
                for id in pinned.iter().rev() {
                    if let Some((tid, label, icon)) = super::menus::TOOLS.iter().find(|x| x.0 == id) {
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
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, *icon, FontId::proportional(13.0), t.text2);
                        }
                        if resp.on_hover_text(*label).clicked() {
                            self.run_tool(tid);
                        }
                    }
                }
                let now = now_ts();
                if let Some(task) = self.data.tasks.iter().find(|x| x.timer_started.is_some()) {
                    let txt = format!("\u{23F9} {}  {}", fmt_duration(task.tracked_secs(now)), truncate(&task.name, 22));
                    let b = egui::Button::new(RichText::new(txt).color(Color32::WHITE)).fill(t.danger).corner_radius(14);
                    let id = task.id;
                    ui.add_space(8.0);
                    if ui.add(b).on_hover_text("Stop timer").clicked() {
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
            .frame(egui::Frame::new().fill(t.app_bg).inner_margin(egui::Margin { left: 6, right: 0, top: 0, bottom: 6 }))
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
                    p.rect_filled(Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 16.0)), 8.0, t.rail_top);
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
                let pinned_sections: Vec<Section> = items.iter().filter_map(|l| section_of(l)).collect();
                for label in &items {
                    let sec = section_of(label);
                    let active = match sec {
                        Some(Section::Home) => matches!(self.section, Section::Home | Section::Posts | Section::Meetings) || (self.section == Section::Chat && !pinned_sections.contains(&Section::Chat)),
                        Some(s) => s == self.section,
                        None => false,
                    };
                    let ibox = Rect::from_center_size(pos2(cx, y + 22.0), vec2(32.0, 32.0));
                    let hit = Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                    let resp = ui.interact(hit, ui.id().with(("rail", label.as_str())), Sense::click());
                    name_it(&resp, &format!("Rail {}", label));
                    if active {
                        p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                    } else if resp.hovered() {
                        p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                    }
                    if !line_icon(&p, &label.to_lowercase(), ibox.center(), white) {
                        p.text(ibox.center(), Align2::CENTER_CENTER, icon_of(label), FontId::proportional(17.0), white);
                    }
                    p.text(pos2(cx, ibox.bottom() + 10.0), Align2::CENTER_CENTER, label, FontId::proportional(11.0), white);
                    if resp.on_hover_text(label.as_str()).clicked() {
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
                        let hit = Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                        let resp = ui.interact(hit, ui.id().with("rail_temp"), Sense::click());
                        let active = self.section == ts;
                        if active {
                            p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                        } else if resp.hovered() {
                            p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                        }
                        if !line_icon(&p, &label.to_lowercase(), ibox.center(), white) {
                            p.text(ibox.center(), Align2::CENTER_CENTER, icon_of(label), FontId::proportional(17.0), white);
                        }
                        let short = if label.chars().count() > 8 { format!("{}..", label.chars().take(7).collect::<String>()) } else { label.to_string() };
                        p.text(pos2(cx, ibox.bottom() + 10.0), Align2::CENTER_CENTER, short, FontId::proportional(11.0), white);
                        if resp.on_hover_text(label).clicked() {
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
                let more_active = matches!(self.section, Section::Docs | Section::Dashboards | Section::Timesheets | Section::Whiteboards | Section::Forms | Section::Goals) && !pinned_sections.contains(&self.section) && self.temp_app != Some(self.section);
                if more_active {
                    p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                } else if resp.hovered() {
                    p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                }
                line_icon(&p, "more", ibox.center(), white);
                p.text(pos2(cx, ibox.bottom() + 10.0), Align2::CENTER_CENTER, "More", FontId::proportional(11.0), white);
                if self.force_menu.as_deref() == Some("more") { let pid = ui.id().with("more_pop"); ui.memory_mut(|m| m.open_popup(pid)); }
                popup(ui, &resp, ui.id().with("more_pop"), 220.0, |ui| self.rail_more_menu(ui));
                // bottom group: Install, Invite (every 56 px); Labs and Upgrade removed at the person's request
                let bottom: [(&str, &str); 2] = [("Install", "\u{2B07}"), ("Invite", "")];
                let mut by = rect.bottom() - bottom.len() as f32 * 56.0 - 2.0;
                for (label, icon) in bottom {
                    let ibox = Rect::from_center_size(pos2(cx, by + 16.0), vec2(32.0, 32.0));
                    let hit = Rect::from_min_max(pos2(rect.left(), by - 2.0), pos2(rect.right(), by + 52.0));
                    let resp = ui.interact(hit, ui.id().with(("railb", label)), Sense::click());
                    name_it(&resp, &format!("Rail {}", label));
                    if resp.hovered() {
                        p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                    }
                    if label == "Invite" {
                        paint_person_icon(&p, ibox.center() + vec2(-1.0, -1.0), white);
                        p.text(ibox.center() + vec2(6.0, 4.0), Align2::CENTER_CENTER, "+", FontId::proportional(10.0), white);
                    } else if !line_icon(&p, &label.to_lowercase(), ibox.center(), white) {
                        p.text(ibox.center(), Align2::CENTER_CENTER, icon, FontId::proportional(16.0), white);
                    }
                    p.text(pos2(cx, ibox.bottom() + 10.0), Align2::CENTER_CENTER, label, FontId::proportional(11.0), white);
                    if resp.on_hover_text(label).clicked() {
                        match label {
                            "Install" => self.modal = Some(Modal::Unavailable { title: "Install".into(), body: "You're already using the desktop app on this computer. Mobile apps come with the shared server.".into() }),
                            _ => self.modal = Some(Modal::Invite { emails: String::new(), role: "Member".into() }),
                        }
                    }
                    by += 56.0;
                }
            });
    }

    // ---------------------------------------------------------------- sidebar

    pub(super) fn count_open(&self, list: u64) -> usize {
        self.data.tasks.iter().filter(|t| t.list_id == list && t.parent.is_none() && !self.data.is_closed(t)).count()
    }

    fn side_row(&self, ui: &mut Ui, indent: f32, selected: bool, lead: impl FnOnce(&egui::Painter, Pos2), label: &str, sub: Option<&str>, trailing: Option<String>, muted: bool) -> Response {
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
        let clip = Rect::from_min_max(pos2(x + 24.0, rect.top()), pos2(rect.right() - 28.0, rect.bottom()));
        let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(14.0), col);
        let gw = g.size().x;
        ui.painter().with_clip_rect(clip).galley(pos2(x + 24.0, rect.center().y - g.size().y / 2.0), g, col);
        if let Some(s) = sub {
            ui.painter().with_clip_rect(clip).text(pos2(x + 28.0 + gw, rect.center().y + 1.0), Align2::LEFT_CENTER, s, FontId::proportional(12.0), t.text3);
        }
        if let Some(tr) = trailing {
            ui.painter().text(rect.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, tr, FontId::proportional(12.0), t.text3);
        }
        resp
    }

    fn add_row(&self, ui: &mut Ui, label: &str) -> Response {
        let t = self.t();
        let tc = t.text3;
        self.side_row(ui, 0.0, false, |p, c| {
            p.text(c, Align2::CENTER_CENTER, "+", FontId::proportional(16.0), tc);
        }, label, None, None, true)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    fn side_header(&self, ui: &mut Ui, title: &str, icons: &[(&str, &str)]) -> Vec<Response> {
        let t = self.t();
        ui.add_space(10.0);
        let w = ui.available_width();
        let (r, hresp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
        ui.painter().text(r.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(13.0), t.text2);
        let hovered = hresp.hovered() || ui.rect_contains_pointer(r);
        let mut out = vec![];
        let mut x = r.right() - 13.0;
        for (i, (glyph, tip)) in icons.iter().enumerate().rev() {
            let br = Rect::from_center_size(pos2(x, r.center().y), vec2(22.0, 22.0));
            let resp = ui.interact(br, ui.id().with(("shdr", title, i)), Sense::click());
            let boxed = *glyph == "+" && i == icons.len() - 1;
            if boxed {
                ui.painter().rect_filled(br, 5.0, if resp.hovered() { t.border } else { t.row_selected });
            } else if resp.hovered() {
                ui.painter().rect_filled(br, 5.0, t.row_selected);
            }
            if boxed || hovered || resp.hovered() {
                ui.painter().text(br.center(), Align2::CENTER_CENTER, *glyph, FontId::proportional(if *glyph == "+" { 15.0 } else { 13.0 }), t.text2);
            }
            out.push(resp.on_hover_text(*tip));
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
            Some(q) if !q.trim().is_empty() => label.to_lowercase().contains(&q.trim().to_lowercase()),
            _ => true,
        }
    }

    pub(super) fn sidebar(&mut self, ui: &mut Ui) {
        let t = self.t();
        if self.sidebar_collapsed {
            egui::SidePanel::left("sidebar_c").exact_width(40.0).resizable(false).frame(egui::Frame::new().fill(t.sidebar).corner_radius(egui::CornerRadius { nw: 8, ne: 0, sw: 8, se: 0 }).inner_margin(egui::Margin::same(6))).show_inside(ui, |ui| {
                if ui.add(egui::Button::new(RichText::new("\u{00BB}").size(16.0)).frame(false)).on_hover_text("Open sidebar").clicked() {
                    self.sidebar_collapsed = false;
                }
            });
            return;
        }
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(255.0)
            .width_range(200.0..=400.0)
            .frame(egui::Frame::new().fill(t.sidebar).corner_radius(egui::CornerRadius { nw: 8, ne: 0, sw: 8, se: 0 }).inner_margin(egui::Margin::symmetric(8, 6)))
            .show_inside(ui, |ui| {
                // header: Home, (hover) search / filter / close, + Create
                let w = ui.available_width();
                let (hr, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
                let hovered = ui.rect_contains_pointer(hr);
                ui.painter().text(hr.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, "Home", FontId::proportional(17.0), t.text);
                let create_r = Rect::from_min_size(pos2(hr.right() - 80.0, hr.center().y - 14.0), vec2(80.0, 28.0));
                let cresp = ui.interact(create_r, ui.id().with("side_create"), Sense::click());
                name_it(&cresp, "Create");
                ui.painter().rect_filled(create_r, 7.0, if cresp.hovered() { t.primary_hover } else { t.primary });
                ui.painter().text(create_r.center(), Align2::CENTER_CENTER, "+  Create", FontId::proportional(14.0), Color32::WHITE);
                if self.force_menu.as_deref() == Some("create") { let pid = ui.id().with("create_pop"); ui.memory_mut(|m| m.open_popup(pid)); }
                popup(ui, &cresp, ui.id().with("create_pop"), 400.0, |ui| self.create_menu(ui));
                let icons: [(&str, &str); 3] = [("\u{1F50D}", "Search your sidebar"), ("\u{2261}", "Filter your sidebar"), ("\u{00AB}", "Close sidebar")];
                for (i, (glyph, tip)) in icons.iter().enumerate() {
                    let br = Rect::from_center_size(pos2(create_r.left() - 90.0 + i as f32 * 29.0 + 14.0, hr.center().y), vec2(26.0, 26.0));
                    let resp = ui.interact(br, ui.id().with(("side_ic", i)), Sense::click());
                    name_it(&resp, tip);
                    let active = (i == 0 && self.side_search.is_some()) || (i == 1 && self.side_filter.is_some());
                    if resp.hovered() || active {
                        ui.painter().rect_filled(br, 6.0, t.row_selected);
                    }
                    if hovered || active {
                        ui.painter().text(br.center(), Align2::CENTER_CENTER, *glyph, FontId::proportional(if i == 2 { 16.0 } else { 13.0 }), t.text2);
                    }
                    if resp.on_hover_text(*tip).clicked() {
                        match i {
                            0 => self.side_search = if self.side_search.is_some() { None } else { Some(String::new()) },
                            1 => self.side_filter = if self.side_filter.is_some() { None } else { Some("") },
                            _ => self.sidebar_collapsed = true,
                        }
                    }
                }
                if let Some(mut q) = self.side_search.take() {
                    let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search the sidebar").desired_width(f32::INFINITY));
                    if q.is_empty() && !r.has_focus() && !r.lost_focus() {
                        r.request_focus();
                    }
                    self.side_search = Some(q);
                }
                if let Some(f) = self.side_filter {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (key, label) in [("space", "Space"), ("unread", "Unread"), ("dms", "DMs")] {
                            let on = f == key;
                            let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(13.0), if on { Color32::WHITE } else { t.text2 });
                            let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 24.0), Sense::click());
                            ui.painter().rect_filled(r, 12.0, if on { t.primary } else if resp.hovered() { t.hover } else { t.bg });
                            ui.painter().rect_stroke(r, 12.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                            ui.painter().galley(r.center() - g.size() / 2.0, g, if on { Color32::WHITE } else { t.text2 });
                            if resp.clicked() {
                                self.side_filter = Some(if on { "" } else { key });
                            }
                        }
                    });
                }
                ui.add_space(4.0);
                let filter = self.side_filter.unwrap_or("");
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let tc = t.text2;
                    let muted = self.data.extra.muted_until > now_ts();
                    if filter.is_empty() {
                        let inbox_n = if muted { 0 } else { self.unseen_activity() + self.due_reminders() };
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
                                "Inbox" => self.section == Section::Home && self.loc == Location::Inbox,
                                "Replies" => self.section == Section::Home && self.loc == Location::Replies,
                                "Assigned Comments" => self.section == Section::Home && self.loc == Location::Assigned,
                                "My Tasks" => self.section == Section::Home && self.loc == Location::MyTasks && !self.my_tasks_open,
                                "Posts" => self.section == Section::Posts,
                                "Meetings" => self.section == Section::Meetings,
                                _ => false,
                            };
                            let tr = if label == "Inbox" && inbox_n > 0 { Some(inbox_n.to_string()) } else { None };
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
                            let r = self.side_row(ui, 0.0, sel, |p, c| {
                                if !line_icon(p, kind, c, tc) {
                                    p.text(c, Align2::CENTER_CENTER, icon, FontId::proportional(13.0), tc);
                                }
                            }, label, None, tr, true);
                            match label {
                                "More" => {
                                    popup(ui, &r, ui.id().with("side_more"), 220.0, |ui| {
                                        for (l, ic) in [("Overview", "\u{1F3E0}"), ("Docs", "\u{1F4C4}"), ("Dashboards", "\u{1F4CA}"), ("Whiteboards", "\u{1F3A8}"), ("Forms", "\u{1F4CB}"), ("Goals", "\u{1F3AF}"), ("Timesheets", "\u{23F1}"), ("People", "\u{1F465}")] {
                                            if ui.add(egui::Button::new(format!("{}  {}", ic, l)).frame(false)).clicked() {
                                                match l {
                                                    "Overview" => self.actions.push(Action::Go(Location::Home)),
                                                    "People" => self.section = Section::People,
                                                    other => self.open_app(other),
                                                }
                                                close_popup(ui);
                                            }
                                        }
                                        ui.separator();
                                        if ui.add(egui::Button::new("\u{25A6}  Customize your sidebar").frame(false)).clicked() {
                                            self.modal = Some(Modal::CustomizeSidebar);
                                            close_popup(ui);
                                        }
                                    });
                                }
                                "My Tasks" if self.my_tasks_open => {
                                    if r.clicked() {
                                        self.my_tasks_open = false;
                                    }
                                    let me = self.data.me;
                                    let td = self.data.tasks.iter().filter(|x| x.assignees.contains(&me) && x.due.map(|d| d <= today()).unwrap_or(false) && !self.data.is_closed(x)).count();
                                    let pl = self.personal_list_id();
                                    let subs: [(&str, Location, Option<String>); 3] = [
                                        ("Assigned to me", Location::MyTasks, None),
                                        ("Today & Overdue", Location::TodayOverdue, if td > 0 { Some(format!("\u{2611} {}", td)) } else { None }),
                                        ("Personal List", pl.map(Location::List).unwrap_or(Location::MyTasks), None),
                                    ];
                                    for (sl, loc, tr) in subs {
                                        let sel = self.section == Section::Home && self.loc == loc && !(sl == "Personal List" && pl.is_none());
                                        let mm = self.data.member(me).cloned();
                                        let bg = t.sidebar;
                                        let r2 = self.side_row(ui, 14.0, sel, |p, c| {
                                            match sl {
                                                "Assigned to me" => {
                                                    if let Some(m) = &mm {
                                                        paint_avatar(p, c, 8.0, m, bg);
                                                    }
                                                }
                                                "Today & Overdue" => paint_calendar_icon(p, c, tc),
                                                _ => paint_person_icon(p, c, tc),
                                            }
                                        }, sl, None, tr, false);
                                        if r2.clicked() {
                                            if sl == "Personal List" && pl.is_none() {
                                                let id = self.create_personal_list();
                                                self.actions.push(Action::Go(Location::List(id)));
                                            } else {
                                                self.actions.push(Action::Go(loc));
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    if r.clicked() {
                                        match label {
                                            "Inbox" => self.actions.push(Action::Go(Location::Inbox)),
                                            "Replies" => self.actions.push(Action::Go(Location::Replies)),
                                            "Assigned Comments" => self.actions.push(Action::Go(Location::Assigned)),
                                            "My Tasks" => {
                                                self.my_tasks_open = !self.my_tasks_open;
                                                self.actions.push(Action::Go(Location::MyTasks));
                                            }
                                            "Posts" => self.section = Section::Posts,
                                            "Meetings" => self.section = Section::Meetings,
                                            "Skills" => self.modal = Some(super::menus::unavailable("Skills")),
                                            _ => self.modal = Some(super::menus::unavailable("Artifacts")),
                                        }
                                    }
                                }
                            }
                        }
                        ui.add_space(6.0);
                        let r = ui.available_rect_before_wrap();
                        ui.painter().hline(r.left() + 6.0..=r.right() - 6.0, r.top(), Stroke::new(1.0, t.border));

                        if !self.hidden("AI Chats") {
                            let hs = self.side_header(ui, "AI Chats", &[("\u{22EF}", "More actions")]);
                            if hs[0].clicked() {
                                self.section = Section::Ai;
                            }
                            if self.add_row(ui, "Ask, Build, Create").clicked() {
                                self.section = Section::Ai;
                            }
                        }

                        if !self.data.favorites.is_empty() && !self.hidden("Favorites") {
                            self.side_header(ui, "Favorites", &[]);
                            for f in self.data.favorites.clone() {
                                let name = self.data.location_name(f);
                                if !self.matches_search(&name) {
                                    continue;
                                }
                                let sel = self.section == Section::Home && self.loc == f;
                                let r = self.side_row(ui, 0.0, sel, |p, c| {
                                    p.text(c, Align2::CENTER_CENTER, "\u{2605}", FontId::proportional(12.0), Color32::from_rgb(0xf8, 0xae, 0x00));
                                }, &name, None, None, false);
                                if r.clicked() {
                                    self.actions.push(Action::Go(f));
                                }
                            }
                        }
                    }

                    let ws = self.data.workspace.clone();
                    if filter.is_empty() || filter == "space" {
                        let hs = self.side_header(ui, "Spaces", &[("\u{22EF}", "Spaces settings"), ("+", "Create Space")]);
                        popup(ui, &hs[0], ui.id().with("spaces_settings"), 200.0, |ui| {
                            if ui.button("Create Space").clicked() {
                                self.actions.push(Action::NewSpace);
                                close_popup(ui);
                            }
                            if ui.button("Expand all").clicked() {
                                for s in &mut self.data.spaces {
                                    s.expanded = true;
                                }
                                self.dirty = true;
                                close_popup(ui);
                            }
                            if ui.button("Collapse all").clicked() {
                                for s in &mut self.data.spaces {
                                    s.expanded = false;
                                }
                                self.dirty = true;
                                close_popup(ui);
                            }
                        });
                        if hs[1].clicked() {
                            self.actions.push(Action::NewSpace);
                        }
                        let sub = format!("- {}", ws);
                        if self.matches_search("All Tasks") {
                            let sel = self.section == Section::Home && self.loc == Location::Everything;
                            let r = self.side_row(ui, 0.0, sel, |p, c| {
                                p.rect_stroke(Rect::from_center_size(c, vec2(16.0, 16.0)), 4.0, Stroke::new(1.0, tc), egui::StrokeKind::Inside);
                                p.text(c, Align2::CENTER_CENTER, "\u{2731}", FontId::proportional(9.0), tc);
                            }, "All Tasks", Some(&sub), None, false);
                            if r.clicked() {
                                self.actions.push(Action::Go(Location::Everything));
                            }
                        }
                        let spaces: Vec<Space> = self.data.spaces.iter().filter(|s| s.private_owner.is_none()).cloned().collect();
                        for s in &spaces {
                            self.space_rows(ui, s);
                        }
                        if self.add_row(ui, "New Space").clicked() {
                            self.actions.push(Action::NewSpace);
                        }
                    }

                    // Channels
                    if (filter.is_empty() || filter == "unread") && !self.hidden("Channels") {
                        let hs = self.side_header(ui, "Channels", &[("+", "Create a Channel")]);
                        if hs[0].clicked() {
                            self.new_channel = Some(String::new());
                        }
                        let chans: Vec<(u64, String, usize)> = self.data.channels.iter().filter(|c| c.kind == ChannelKind::Channel).map(|c| (c.id, c.name.clone(), if muted { 0 } else { self.unread_count(c) })).collect();
                        for (id, name, unread) in chans {
                            if (filter == "unread" && unread == 0) || !self.matches_search(&name) {
                                continue;
                            }
                            let sel = self.section == Section::Chat && self.chat_open == Some(id);
                            let sub = if name == "general" { Some(format!("- {}", ws)) } else { None };
                            let r = self.side_row(ui, 0.0, sel, |p, c| {
                                p.text(c, Align2::CENTER_CENTER, "#", FontId::proportional(15.0), tc);
                            }, &name, sub.as_deref(), if unread > 0 { Some(unread.to_string()) } else { None }, false);
                            let sr = Rect::from_center_size(r.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
                            let sresp = ui.interact(sr, ui.id().with(("chset", id)), Sense::click());
                            if r.hovered() || sresp.hovered() {
                                ui.painter().rect_filled(sr, 5.0, if sresp.hovered() { t.border } else { t.row_selected });
                                ui.painter().text(sr.center(), Align2::CENTER_CENTER, "\u{22EF}", FontId::proportional(13.0), t.text2);
                            }
                            if sresp.on_hover_text("Channel Settings").clicked() {
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
                                let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text("channel-name").desired_width(170.0));
                                r.request_focus();
                                if r.lost_focus() {
                                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.trim().is_empty() {
                                        let id = self.data.new_id();
                                        let clean = name.trim().to_lowercase().replace(' ', "-");
                                        let members = self.data.members.iter().map(|m| m.id).collect();
                                        self.data.channels.push(Channel { id, name: clean, kind: ChannelKind::Channel, topic: String::new(), members, messages: vec![], last_read: now_ts() });
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
                        } else if filter.is_empty() && self.add_row(ui, "Add Channel").clicked() {
                            self.new_channel = Some(String::new());
                        }
                    }

                    // Direct Messages
                    if (filter.is_empty() || filter == "dms" || filter == "unread") && !self.hidden("Direct Messages") {
                        let hs = self.side_header(ui, "Direct Messages", &[("+", "New message")]);
                        if hs[0].clicked() {
                            self.new_message_open = true;
                        }
                        let me = self.data.me;
                        let dms: Vec<(u64, Option<Member>, usize)> = self
                            .data
                            .channels
                            .iter()
                            .filter(|c| c.kind == ChannelKind::Direct)
                            .map(|c| (c.id, c.members.iter().find(|m| **m != me).and_then(|m| self.data.member(*m)).cloned(), if muted { 0 } else { self.unread_count(c) }))
                            .collect();
                        for (id, other, unread) in dms {
                            let (name, sub) = match &other {
                                Some(m) => (m.name.clone(), None),
                                None => (self.data.member(me).map(|m| m.name.clone()).unwrap_or_default(), Some("\u{2014} You".to_string())),
                            };
                            if (filter == "unread" && unread == 0) || !self.matches_search(&name) {
                                continue;
                            }
                            let sel = self.section == Section::Chat && self.chat_open == Some(id);
                            let who = other.clone().or_else(|| self.data.member(me).cloned());
                            let bg = t.sidebar;
                            let r = self.side_row(ui, 0.0, sel, |p, c| {
                                if let Some(m) = &who {
                                    paint_avatar(p, c, 9.0, m, bg);
                                    p.circle_filled(c + vec2(7.0, 7.0), 3.0, bg);
                                    p.circle_filled(c + vec2(7.0, 7.0), 2.0, Color32::from_rgb(48, 164, 108));
                                }
                            }, &name, sub.as_deref(), if unread > 0 { Some(unread.to_string()) } else { None }, false);
                            let sr = Rect::from_center_size(r.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
                            let sresp = ui.interact(sr, ui.id().with(("dmset", id)), Sense::click());
                            if r.hovered() || sresp.hovered() {
                                ui.painter().rect_filled(sr, 5.0, if sresp.hovered() { t.border } else { t.row_selected });
                                ui.painter().text(sr.center(), Align2::CENTER_CENTER, "\u{22EF}", FontId::proportional(13.0), t.text2);
                            }
                            if sresp.on_hover_text("DM Settings").clicked() {
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

    pub(super) fn personal_list_id(&self) -> Option<u64> {
        let me = self.data.me;
        self.data.spaces.iter().find(|s| s.private_owner == Some(me)).and_then(|s| s.lists.first()).map(|l| l.id)
    }

    pub(super) fn create_personal_list(&mut self) -> u64 {
        if let Some(id) = self.personal_list_id() {
            return id;
        }
        let me = self.data.me;
        let sid = self.data.new_id();
        let lid = self.data.new_id();
        self.data.spaces.push(Space { id: sid, name: "Personal".into(), color: [100, 100, 100], statuses: default_statuses(), folders: vec![], lists: vec![ListItem { id: lid, name: "Personal List".into() }], expanded: true, private_owner: Some(me) });
        self.dirty = true;
        lid
    }

    fn unread_count(&self, c: &Channel) -> usize {
        if !self.data.extra.notify.chat {
            return 0;
        }
        c.messages.iter().filter(|m| m.ts > c.last_read && m.author != self.data.me).count()
    }

    fn unseen_activity(&self) -> usize {
        let day = now_ts() - 86400;
        self.data.tasks.iter().flat_map(|t| t.activity.iter()).filter(|a| a.comment && a.author != self.data.me && a.ts > day).count()
    }

    fn space_rows(&mut self, ui: &mut Ui, s: &Space) {
        let t = self.t();
        let loc = Location::Space(s.id);
        let color = rgb(s.color);
        let letter = s.name.chars().next().unwrap_or('S').to_uppercase().to_string();
        let expanded = s.expanded;
        let sel = self.section == Section::Home && self.loc == loc;
        let r = self.side_row(ui, 0.0, sel, |p, c| {
            let rr = Rect::from_center_size(c, vec2(19.0, 19.0));
            p.rect_filled(rr, 5.0, color);
            p.text(c, Align2::CENTER_CENTER, letter, FontId::proportional(11.0), Color32::WHITE);
        }, &s.name, None, None, false);
        // add button on hover / expand toggle on the icon
        let plus = Rect::from_center_size(r.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
        let pr = ui.interact(plus, ui.id().with(("splus", s.id)), Sense::click());
        if r.hovered() || pr.hovered() || sel {
            ui.painter().text(plus.center(), Align2::CENTER_CENTER, "+", FontId::proportional(16.0), t.text2);
        }
        let sid = s.id;
        popup(ui, &pr, ui.id().with(("splus_pop", s.id)), 220.0, |ui| {
            ui.label(RichText::new("Create").small().color(t.text3));
            for (icon, label) in [("\u{2630}", "List"), ("\u{1F4C1}", "Folder"), ("\u{1F4C4}", "Doc"), ("\u{1F4CB}", "Form"), ("\u{1F4CA}", "Dashboard"), ("\u{1F3A8}", "Whiteboard")] {
                if ui.add(egui::Button::new(format!("{}  {}", icon, label)).frame(false)).clicked() {
                    match label {
                        "List" => self.actions.push(Action::NewList { space: sid, folder: None }),
                        "Folder" => self.actions.push(Action::NewFolder(sid)),
                        "Doc" => self.new_doc(Some(sid)),
                        "Form" => self.new_form(),
                        "Dashboard" => self.modal = Some(Modal::NewDashboard { name: "New Dashboard".into() }),
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
        } else if r.clicked() && !pr.clicked() {
            self.actions.push(Action::Go(loc));
            if !expanded {
                self.actions.push(Action::ToggleSpace(s.id));
            }
        }
        r.context_menu(|ui| {
            if ui.button("New List").clicked() {
                self.actions.push(Action::NewList { space: sid, folder: None });
                ui.close_menu();
            }
            if ui.button("New Folder").clicked() {
                self.actions.push(Action::NewFolder(sid));
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Rename").clicked() {
                self.actions.push(Action::StartRename(Target::Space(sid)));
                ui.close_menu();
            }
            if ui.button("Edit statuses").clicked() {
                self.actions.push(Action::EditStatuses(sid));
                ui.close_menu();
            }
            ui.menu_button("Color", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.set_max_width(170.0);
                    for c in STATUS_PALETTE.iter().skip(1) {
                        let (rr, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                        ui.painter().circle_filled(rr.center(), 8.0, rgb(*c));
                        if resp.clicked() {
                            self.actions.push(Action::SetSpaceColor(sid, *c));
                            ui.close_menu();
                        }
                    }
                });
            });
            let fav = if self.data.favorites.contains(&loc) { "Remove from Favorites" } else { "Add to Favorites" };
            if ui.button(fav).clicked() {
                self.actions.push(Action::ToggleFavorite(loc));
                ui.close_menu();
            }
            ui.separator();
            if ui.button(RichText::new("Delete").color(t.danger)).clicked() {
                self.actions.push(Action::AskDelete(Target::Space(sid)));
                ui.close_menu();
            }
        });
        if !expanded {
            return;
        }
        let tc = t.text2;
        for f in &s.folders {
            let floc = Location::Folder(f.id);
            let fexp = f.expanded;
            let fsel = self.section == Section::Home && self.loc == floc;
            let fr = self.side_row(ui, 14.0, fsel, |p, c| {
                let rr = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(14.0, 11.0));
                p.rect_stroke(rr, 2.0, Stroke::new(1.2, tc), egui::StrokeKind::Inside);
                p.line_segment([rr.left_top() + vec2(0.0, 2.5), rr.left_top() + vec2(5.0, 2.5)], Stroke::new(1.2, tc));
            }, &f.name, None, None, false);
            if fr.clicked() {
                self.actions.push(Action::Go(floc));
                self.actions.push(Action::ToggleFolder(f.id));
                if !fexp {
                    // opening also expands
                }
            }
            let fid = f.id;
            fr.context_menu(|ui| {
                if ui.button("New List").clicked() {
                    self.actions.push(Action::NewList { space: sid, folder: Some(fid) });
                    ui.close_menu();
                }
                if ui.button("Rename").clicked() {
                    self.actions.push(Action::StartRename(Target::Folder(fid)));
                    ui.close_menu();
                }
                let fav = if self.data.favorites.contains(&floc) { "Remove from Favorites" } else { "Add to Favorites" };
                if ui.button(fav).clicked() {
                    self.actions.push(Action::ToggleFavorite(floc));
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(RichText::new("Delete").color(t.danger)).clicked() {
                    self.actions.push(Action::AskDelete(Target::Folder(fid)));
                    ui.close_menu();
                }
            });
            if fexp {
                for l in &f.lists {
                    self.list_row(ui, 30.0, l);
                }
            }
        }
        for l in &s.lists {
            self.list_row(ui, 14.0, l);
        }
        let docs: Vec<(u64, String)> = self.data.docs.iter().filter(|d| d.space == Some(s.id)).map(|d| (d.id, d.title.clone())).collect();
        for (id, title) in docs {
            if !self.matches_search(&title) {
                continue;
            }
            let sel = self.section == Section::Docs && self.doc_open == Some(id);
            let r = self.side_row(ui, 14.0, sel, |p, c| {
                let rr = Rect::from_center_size(c, vec2(12.0, 14.0));
                p.rect_filled(rr, 2.0, Color32::from_rgb(16, 144, 224));
                for k in 0..3 {
                    let y = rr.top() + 4.0 + k as f32 * 3.0;
                    p.line_segment([pos2(rr.left() + 3.0, y), pos2(rr.right() - 3.0, y)], Stroke::new(1.0, Color32::WHITE));
                }
            }, &title, None, None, false);
            if r.clicked() {
                self.doc_open = Some(id);
                self.section = Section::Docs;
            }
        }
    }

    fn list_row(&mut self, ui: &mut Ui, indent: f32, l: &ListItem) {
        let t = self.t();
        let loc = Location::List(l.id);
        let n = self.count_open(l.id);
        let tc = t.text2;
        let sel = self.section == Section::Home && self.loc == loc;
        let r = self.side_row(ui, indent, sel, |p, c| paint_checklist_icon(p, c, tc), &l.name, None, if n > 0 { Some(n.to_string()) } else { None }, false);
        if r.clicked() {
            self.actions.push(Action::Go(loc));
        }
        let lid = l.id;
        r.context_menu(|ui| {
            if ui.button("Rename").clicked() {
                self.actions.push(Action::StartRename(Target::List(lid)));
                ui.close_menu();
            }
            let fav = if self.data.favorites.contains(&loc) { "Remove from Favorites" } else { "Add to Favorites" };
            if ui.button(fav).clicked() {
                self.actions.push(Action::ToggleFavorite(loc));
                ui.close_menu();
            }
            ui.separator();
            if ui.button(RichText::new("Delete").color(t.danger)).clicked() {
                self.actions.push(Action::AskDelete(Target::List(lid)));
                ui.close_menu();
            }
        });
    }

    // ---------------------------------------------------------------- page header

    pub(super) fn header(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 12, top: 8, bottom: 0 }).show(ui, |ui| {
            // breadcrumb
            ui.horizontal(|ui| {
                ui.set_height(30.0);
                let mut crumbs: Vec<(String, u8, Option<[u8; 3]>)> = vec![];
                match self.loc {
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
                    l => crumbs.push((self.data.location_name(l), 3, None)),
                }
                let n = crumbs.len();
                for (i, (name, kind, color)) in crumbs.iter().enumerate() {
                    let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                    match kind {
                        0 => {
                            ui.painter().rect_filled(r.shrink(1.0), 4.0, rgb(color.unwrap_or([96, 73, 231])));
                            ui.painter().text(r.center(), Align2::CENTER_CENTER, name.chars().next().unwrap_or('S').to_uppercase().to_string(), FontId::proportional(10.0), Color32::WHITE);
                        }
                        1 => {
                            ui.painter().rect_stroke(Rect::from_center_size(r.center(), vec2(14.0, 11.0)), 2.0, Stroke::new(1.2, t.text2), egui::StrokeKind::Inside);
                        }
                        2 => paint_checklist_icon(ui.painter(), r.center(), t.text),
                        _ => {}
                    }
                    let last = i + 1 == n;
                    ui.label(RichText::new(name).size(15.0).color(if last { t.text } else { t.text2 }));
                    if last {
                        let (cr, _) = ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
                        let cv = cr.center();
                        ui.painter().line_segment([cv + vec2(-3.5, -1.5), cv + vec2(0.0, 2.0)], Stroke::new(1.3, t.text2));
                        ui.painter().line_segment([cv + vec2(0.0, 2.0), cv + vec2(3.5, -1.5)], Stroke::new(1.3, t.text2));
                    }
                    if !last {
                        ui.label(RichText::new("/").color(t.text3));
                    }
                }
                let fav = self.data.favorites.contains(&self.loc);
                if !matches!(self.loc, Location::MyTasks | Location::Everything) {
                    let star = RichText::new(if fav { "\u{2605}" } else { "\u{2606}" }).size(16.0).color(if fav { Color32::from_rgb(0xf8, 0xae, 0x00) } else { t.text2 });
                    if ui.add(egui::Button::new(star).frame(false)).on_hover_text("Favorite").clicked() {
                        self.actions.push(Action::ToggleFavorite(self.loc));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let hb = |ui: &mut Ui, text: &str| -> Response {
                        let (kind, label) = match text {
                            t if t.contains("Share") => ("share", "Share"),
                            t if t.contains("Ask") => ("edit", "Ask"),
                            t if t.contains("Automate") => ("robot", "Automate"),
                            _ => ("phone", ""),
                        };
                        let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(14.5), t.text2);
                        let w = if label.is_empty() { 40.0 } else { g.size().x + 32.0 };
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                        name_it(&resp, if label.is_empty() { "Huddle" } else { label });
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, t.hover);
                        }
                        line_icon(ui.painter(), kind, r.left_center() + vec2(12.0, 0.0), t.text2);
                        if label.is_empty() {
                            let cv = r.right_center() - vec2(9.0, 1.0);
                            ui.painter().line_segment([cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5)], Stroke::new(1.2, t.text2));
                            ui.painter().line_segment([cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)], Stroke::new(1.2, t.text2));
                        } else {
                            ui.painter().galley(pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0), g, t.text2);
                        }
                        resp
                    };
                    if matches!(self.loc, Location::List(_)) {
                        let r = ui.add(egui::Button::new(RichText::new("Manage").size(14.0)).stroke(Stroke::new(1.0, t.border)).corner_radius(7).min_size(vec2(0.0, 28.0)));
                        if let Location::List(lid) = self.loc {
                            popup(ui, &r, ui.id().with("manage_pop"), 200.0, |ui| {
                                if ui.button("Rename list").clicked() {
                                    self.actions.push(Action::StartRename(Target::List(lid)));
                                    close_popup(ui);
                                }
                                if let Some(sid) = self.data.space_of_list(lid).map(|s| s.id) {
                                    if ui.button("Edit statuses").clicked() {
                                        self.actions.push(Action::EditStatuses(sid));
                                        close_popup(ui);
                                    }
                                }
                                if ui.button("Custom Fields").clicked() {
                                    self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
                                    close_popup(ui);
                                }
                                if ui.button("Export to CSV").clicked() {
                                    self.modal = Some(Modal::Import { path: String::new(), result: String::new() });
                                    close_popup(ui);
                                }
                                if ui.button(RichText::new("Delete list").color(t.danger)).clicked() {
                                    self.actions.push(Action::AskDelete(Target::List(lid)));
                                    close_popup(ui);
                                }
                            });
                        }
                    }
                    if hb(ui, "\u{1F465}  Share").clicked() {
                        self.modal = Some(Modal::Share { emails: String::new() });
                    }
                    if hb(ui, "\u{1F4DD}  Ask").clicked() {
                        self.section = Section::Ai;
                    }
                    if hb(ui, "\u{2699}  Automate").clicked() {
                        self.modal = Some(Modal::Automations { status: "complete".into(), action: "comment".into(), value: String::new() });
                    }
                    if hb(ui, "\u{260E}").on_hover_text("Start a huddle").clicked() {
                        self.open_call_dialog();
                    }
                });
            });
            // view tabs
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                // "Add Channel": a chat channel for this location
                let ac = ui.add(egui::Button::new(RichText::new("Add Channel").size(13.5).color(t.text3)).stroke(Stroke::new(1.0, t.border)).corner_radius(6).min_size(vec2(0.0, 26.0)));
                if ac.clicked() {
                    let name = self.data.location_name(self.loc).to_lowercase().replace(' ', "-");
                    let existing = self.data.channels.iter().find(|c| c.kind == ChannelKind::Channel && c.name == name).map(|c| c.id);
                    let id = existing.unwrap_or_else(|| {
                        let id = self.data.new_id();
                        let members = self.data.members.iter().map(|m| m.id).collect();
                        self.data.channels.push(Channel { id, name: name.clone(), kind: ChannelKind::Channel, topic: format!("Conversations about {}", self.data.location_name(self.loc)), members, messages: vec![], last_read: now_ts() });
                        self.dirty = true;
                        id
                    });
                    self.section = Section::Chat;
                    self.chat_open = Some(id);
                }
                ui.add_space(8.0);
                let is_list = matches!(self.loc, Location::List(_) | Location::MyTasks);
                if is_list && self.view == ViewKind::Overview {
                    self.view = ViewKind::List;
                }
                for v in ViewKind::ALL {
                    if (v == ViewKind::Overview && is_list) || (v != self.view && self.data.extra.hidden_views.iter().any(|h| h == v.label())) {
                        continue;
                    }
                    let sel = self.view == v;
                    let g = ui.painter().layout_no_wrap(v.label().to_string(), FontId::proportional(14.0), if sel { t.text } else { t.text2 });
                    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 32.0, 34.0), Sense::click());
                    name_it(&resp, &format!("View {}", v.label()));
                    if resp.hovered() && !sel {
                        ui.painter().rect_filled(r.shrink2(vec2(0.0, 5.0)), 6.0, t.hover);
                    }
                    paint_view_icon(ui.painter(), pos2(r.left() + 13.0, r.center().y), v);
                    let gc = if sel { t.text } else { t.text2 };
                    ui.painter().galley(pos2(r.left() + 25.0, r.center().y - g.size().y / 2.0), g.clone(), gc);
                    if sel {
                        ui.painter().galley(pos2(r.left() + 25.3, r.center().y - g.size().y / 2.0), g, gc);
                        ui.painter().rect_filled(Rect::from_min_max(pos2(r.left() + 2.0, r.bottom() - 2.0), pos2(r.right() - 2.0, r.bottom())), 1.0, t.text);
                    }
                    if resp.clicked() {
                        self.view = v;
                    }
                }
                ui.add_space(6.0);
                let vr = ui.add(egui::Button::new(RichText::new("+  View").size(14.0).color(t.text2)).frame(false).min_size(vec2(0.0, 30.0)));
                popup(ui, &vr, ui.id().with("add_view"), 200.0, |ui| {
                    ui.label(RichText::new("Views").small().color(t.text3));
                    for v in ViewKind::ALL {
                        let mut on = !self.data.extra.hidden_views.iter().any(|h| h == v.label());
                        if ui.checkbox(&mut on, v.label()).changed() {
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
        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
        // toolbar
        egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 8)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if matches!(self.view, ViewKind::List | ViewKind::Board) {
                    let r = pill(ui, &t, &format!("\u{2261}  Group: {}", self.group_by.label()), false, true);
                    popup(ui, &r, ui.id().with("group_pop"), 160.0, |ui| {
                        for g in [GroupBy::Status, GroupBy::Priority, GroupBy::Assignee, GroupBy::DueDate, GroupBy::None] {
                            if ui.selectable_label(self.group_by == g, g.label()).clicked() {
                                self.group_by = g;
                                close_popup(ui);
                            }
                        }
                    });
                    if pill(ui, &t, "\u{2934}  Subtasks", self.subtasks_open, false).on_hover_text("Show all subtasks").clicked() {
                        self.subtasks_open = !self.subtasks_open;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let b = egui::Button::new(RichText::new("Add Task").color(Color32::WHITE).size(13.0)).fill(t.primary).corner_radius(6).min_size(vec2(0.0, 26.0));
                    if ui.add(b).clicked() {
                        self.open_quick_add(None);
                    }
                    // search toggle
                    let (sr, sresp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                    ui.painter().rect_stroke(sr, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                    if sresp.hovered() || self.show_search {
                        ui.painter().rect_filled(sr.shrink(1.0), 6.0, t.hover);
                    }
                    ui.painter().text(sr.center(), Align2::CENTER_CENTER, "\u{1F50D}", FontId::proportional(12.0), t.text2);
                    if sresp.on_hover_text("Search tasks").clicked() {
                        self.show_search = !self.show_search;
                        if !self.show_search {
                            self.search.clear();
                        }
                    }
                    if self.show_search {
                        let r = ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search tasks...").desired_width(180.0));
                        if self.search.is_empty() && !r.has_focus() && !r.lost_focus() {
                            r.request_focus();
                        }
                    }
                    // me mode avatar
                    if let Some(m) = self.data.member(self.data.me).cloned() {
                        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                        if self.me_mode {
                            ui.painter().circle_stroke(r.center(), 12.5, Stroke::new(1.5, t.text));
                        }
                        paint_avatar(ui.painter(), r.center(), 10.5, &m, t.bg);
                        if resp.on_hover_text("Me mode: only my tasks").clicked() {
                            self.me_mode = !self.me_mode;
                        }
                    }
                    let alabel = match self.assignee_filter.and_then(|a| self.data.member(a)) {
                        Some(m) => format!("\u{1F464}  {}", m.name),
                        None => "\u{1F464}  Assignee".into(),
                    };
                    let ar = pill(ui, &t, &alabel, self.assignee_filter.is_some(), false);
                    let data = self.data.clone();
                    popup(ui, &ar, ui.id().with("asg_filter"), 200.0, |ui| {
                        if ui.selectable_label(self.assignee_filter.is_none(), "Everyone").clicked() {
                            self.assignee_filter = None;
                            close_popup(ui);
                        }
                        for m in &data.members {
                            if ui.selectable_label(self.assignee_filter == Some(m.id), &m.name).clicked() {
                                self.assignee_filter = Some(m.id);
                                close_popup(ui);
                            }
                        }
                    });
                    if pill(ui, &t, "\u{2714}  Closed", self.show_closed, false).on_hover_text("Show closed tasks").clicked() {
                        self.show_closed = !self.show_closed;
                    }
                    let fr = pill(ui, &t, "\u{2261}  Filter", !self.search.is_empty(), false);
                    popup(ui, &fr, ui.id().with("filter_pop"), 220.0, |ui| {
                        ui.label(RichText::new("Filter by name or tag").small().color(t.text3));
                        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search tasks...").desired_width(200.0));
                        ui.checkbox(&mut self.me_mode, "Only my tasks");
                        if ui.button("Clear").clicked() {
                            self.search.clear();
                            self.me_mode = false;
                            self.assignee_filter = None;
                        }
                    });
                    let sr = pill(ui, &t, &format!("\u{21C5}  Sort: {}", self.sort_by.label()), self.sort_by != SortBy::Manual, false);
                    popup(ui, &sr, ui.id().with("sort_pop"), 160.0, |ui| {
                        for s in [SortBy::Manual, SortBy::DueDate, SortBy::Priority, SortBy::Name, SortBy::Created] {
                            if ui.selectable_label(self.sort_by == s, s.label()).clicked() {
                                self.sort_by = s;
                                close_popup(ui);
                            }
                        }
                    });
                });
            });
        });
    }

    // ---------------------------------------------------------------- list view

    pub(super) fn list_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let tasks: Vec<Task> = self.filtered().into_iter().cloned().collect();
        let refs: Vec<&Task> = tasks.iter().collect();
        let groups = self.groups(&refs, false);
        // one column header row for the whole list
        let w = ui.available_width();
        let (hr, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
        let cols = col_layout(hr);
        let p = ui.painter();
        p.text(hr.left_center() + vec2(36.0, 0.0), Align2::LEFT_CENTER, "Name", FontId::proportional(13.0), t.text2);
        let show_pr = self.data.extra.apps.priorities;
        for (label, r) in [("Assignee", cols.assignee), ("Due date", cols.due), ("Priority", cols.priority), ("Status", cols.status), ("Comments", cols.comments)] {
            if label == "Priority" && !show_pr {
                continue;
            }
            p.text(r.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(13.0), t.text2);
        }
        let plus = Rect::from_center_size(pos2(hr.right() - 20.0, hr.center().y), vec2(16.0, 16.0));
        p.circle_filled(plus.center(), 8.0, t.text);
        p.text(plus.center(), Align2::CENTER_CENTER, "+", FontId::proportional(13.0), Color32::WHITE);
        p.hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
        if ui.interact(plus.expand(4.0), ui.id().with("add_column"), Sense::click()).on_hover_text("Add a column (Custom Field)").clicked() {
            self.modal = Some(Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() });
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            if groups.iter().all(|g| g.tasks.is_empty()) {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No tasks here yet").size(16.0).color(t.text2));
                    ui.add_space(6.0);
                    if primary_button(ui, &t, "Add Task").clicked() {
                        self.open_quick_add(None);
                    }
                });
            }
            for g in &groups {
                self.list_group(ui, g, &tasks);
                ui.add_space(12.0);
            }
            ui.add_space(40.0);
        });
    }

    fn group_pill(&self, ui: &mut Ui, g: &Group) {
        match &g.status {
            Some(st) => {
                status_pill(ui, st, Sense::hover());
            }
            None => {
                let fake = Status { name: g.label.clone(), color: [g.color.r(), g.color.g(), g.color.b()], kind: StatusKind::Custom };
                status_pill(ui, &fake, Sense::hover());
            }
        }
    }

    fn list_group(&mut self, ui: &mut Ui, g: &Group, tasks: &[Task]) {
        let t = self.t();
        let collapsed = self.collapsed.contains(&g.key);
        let w = ui.available_width();
        let (hr, hresp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
        let arrow = Rect::from_center_size(hr.left_center() + vec2(14.0, 0.0), vec2(18.0, 18.0));
        let ar = ui.interact(arrow, ui.id().with(("garrow", &g.key)), Sense::click());
        paint_triangle(ui.painter(), arrow.center(), !collapsed, if ar.hovered() { t.text } else { t.text2 });
        if ar.clicked() {
            if collapsed {
                self.collapsed.remove(&g.key);
            } else {
                self.collapsed.insert(g.key.clone());
            }
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(hr.left() + 28.0, hr.top()), hr.max)).layout(Layout::left_to_right(Align::Center)));
        self.group_pill(&mut child, g);
        child.label(RichText::new(g.tasks.len().to_string()).color(t.text2));
        if hresp.hovered() || child.ui_contains_pointer() {
            if child.add(egui::Button::new(RichText::new("+ Add Task").color(t.text3).size(12.5)).frame(false)).clicked() {
                self.adding = Some((g.key.clone(), String::new()));
                self.collapsed.remove(&g.key);
            }
        }
        if collapsed {
            return;
        }
        let rows: Vec<&Task> = g.tasks.iter().filter_map(|id| tasks.iter().find(|x| x.id == *id)).collect();
        for task in rows {
            self.task_row(ui, task, 0);
            if self.subtasks_open || self.expanded_tasks.contains(&task.id) {
                let subs: Vec<Task> = self.subtasks(task.id).into_iter().cloned().collect();
                for s in &subs {
                    self.task_row(ui, s, 1);
                }
            }
        }
        let w = ui.available_width();
        let adding_here = self.adding.as_ref().map(|(k, _)| k == &g.key).unwrap_or(false);
        if adding_here {
            let mut text = self.adding.as_ref().unwrap().1.clone();
            let mut submit = false;
            let mut cancel = false;
            ui.horizontal(|ui| {
                ui.add_space(36.0);
                let r = ui.add(egui::TextEdit::singleline(&mut text).hint_text("Task Name").desired_width((w - 240.0).max(200.0)));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                } else {
                    r.request_focus();
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                if primary_button(ui, &t, "Save").clicked() {
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
            ui.painter().text(r.left_center() + vec2(40.0, 0.0), Align2::LEFT_CENTER, "+ Add Task", FontId::proportional(13.5), t.text3);
            if resp.clicked() {
                self.adding = Some((g.key.clone(), String::new()));
            }
        }
    }

    fn task_row(&mut self, ui: &mut Ui, task: &Task, depth: usize) {
        let t = self.t();
        let w = ui.available_width();
        let row_h = if self.data.extra.labs.iter().any(|l| l == "compact") { 30.0 } else { 36.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(w, row_h), Sense::click());
        name_it(&resp, &format!("Task {}", task.name));
        let id = ui.id().with(("row", task.id));
        let apps = self.data.extra.apps.clone();
        if resp.hovered() {
            ui.painter().rect_filled(rect, 0.0, t.hover);
        }
        ui.painter().hline(rect.left() + 28.0..=rect.right(), rect.bottom() - 0.5, Stroke::new(1.0, t.border));
        let cols = col_layout(rect);
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
                paint_triangle(ui.painter(), ar.center(), open, if a.hovered() { t.text } else { t.text3 });
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
        paint_status_circle(ui.painter(), cr.center(), if c.hovered() { 7.5 } else { 7.0 }, &st);
        let statuses = self.data.statuses_for_list(task.list_id);
        let mut edited = task.clone();
        let mut changed = false;
        if let Some(true) = popup(ui, &c, id.with("stp"), 200.0, |ui| status_menu(ui, &t, &statuses, &mut edited.status)) {
            changed = true;
        }
        x += 26.0;
        let name_col = if closed { t.text3 } else { t.text };
        let max_name = cols.assignee.left() - x - 110.0;
        let galley = ui.painter().layout_no_wrap(truncate(&task.name, ((max_name / 7.5) as usize).max(8)), FontId::proportional(14.0), name_col);
        let gw = galley.size().x;
        ui.painter().galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, name_col);
        if closed {
            ui.painter().hline(x..=x + gw, rect.center().y, Stroke::new(1.0, t.text3));
        }
        x += gw + 10.0;
        if nsub > 0 {
            ui.painter().text(pos2(x, rect.center().y), Align2::LEFT_CENTER, format!("\u{2934} {}", nsub), FontId::proportional(12.0), t.text3);
            x += 34.0;
        }
        if !task.checklist.is_empty() {
            let done = task.checklist.iter().filter(|c| c.done).count();
            ui.painter().text(pos2(x, rect.center().y), Align2::LEFT_CENTER, format!("\u{2611} {}/{}", done, task.checklist.len()), FontId::proportional(12.0), t.text3);
            x += 42.0;
        }
        for tag in task.tags.iter().take(if apps.tags { 2 } else { 0 }) {
            let col = tag_color(tag);
            let g = ui.painter().layout_no_wrap(tag.clone(), FontId::proportional(11.5), col);
            let r = Rect::from_min_size(pos2(x, rect.center().y - 9.0), vec2(g.size().x + 12.0, 18.0));
            if r.right() < cols.assignee.left() - 8.0 {
                ui.painter().rect_filled(r, 9.0, col.gamma_multiply(0.16));
                ui.painter().galley(r.center() - g.size() / 2.0, g, col);
            }
            x = r.right() + 4.0;
        }
        let icon_col = Color32::from_rgb(160, 160, 160);
        // assignee
        let ar = ui.interact(cols.assignee, id.with("as"), Sense::click());
        let used = paint_avatars(ui.painter(), cols.assignee.left_center(), 11.0, &task.assignees, &self.data, if resp.hovered() { t.hover } else { t.bg });
        if used == 0.0 {
            paint_person_icon(ui.painter(), cols.assignee.left_center() + vec2(12.0, 0.0), icon_col);
        }
        let data = &self.data;
        if let Some(true) = popup(ui, &ar, id.with("asp"), 220.0, |ui| assignee_menu(ui, &t, data, &mut edited.assignees)) {
            changed = true;
        }
        // due date
        let dr = ui.interact(cols.due, id.with("du"), Sense::click());
        match task.due {
            Some(d) => {
                ui.painter().text(cols.due.left_center(), Align2::LEFT_CENTER, fmt_date(d), FontId::proportional(13.5), date_color(d, closed, &t));
            }
            None => paint_calendar_icon(ui.painter(), cols.due.left_center() + vec2(10.0, 0.0), icon_col),
        }
        if let Some(true) = popup(ui, &dr, id.with("dup"), 380.0, |ui| date_menu(ui, &t, id.with("dcal"), &mut edited.due)) {
            changed = true;
        }
        // priority
        let pr = ui.interact(cols.priority, id.with("pr"), Sense::click());
        if !apps.priorities {
            // Priorities are turned off in Apps
        } else if task.priority > 0 {
            paint_flag(ui.painter(), cols.priority.left_center() + vec2(4.0, 0.0), priority_color(task.priority, &t), 13.0);
            ui.painter().text(cols.priority.left_center() + vec2(18.0, 0.0), Align2::LEFT_CENTER, PRIORITIES[task.priority as usize].label, FontId::proportional(13.5), t.text);
        } else {
            paint_outline_flag(ui.painter(), cols.priority.left_center() + vec2(8.0, 0.0), icon_col);
        }
        if apps.priorities {
            if let Some(true) = popup(ui, &pr, id.with("prp"), 180.0, |ui| priority_menu(ui, &t, &mut edited.priority)) {
                changed = true;
            }
        }
        // status pill column
        let sr = ui.interact(cols.status, id.with("stcol"), Sense::click());
        {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(cols.status).layout(Layout::left_to_right(Align::Center)));
            status_pill(&mut child, &st, Sense::hover());
        }
        if let Some(true) = popup(ui, &sr, id.with("stcolp"), 200.0, |ui| status_menu(ui, &t, &statuses, &mut edited.status)) {
            changed = true;
        }
        // comments
        let comments = task.activity.iter().filter(|a| a.comment).count();
        paint_comment_icon(ui.painter(), cols.comments.left_center() + vec2(10.0, 0.0), icon_col);
        if comments > 0 {
            ui.painter().text(cols.comments.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, comments.to_string(), FontId::proportional(12.5), t.text2);
        }
        let tid = task.id;
        resp.context_menu(|ui| {
            if ui.button("Open").clicked() {
                self.actions.push(Action::Open(tid));
                ui.close_menu();
            }
            if ui.button("Rename").clicked() {
                self.actions.push(Action::StartRename(Target::Task(tid)));
                ui.close_menu();
            }
            if ui.button("Start timer").clicked() {
                self.actions.push(Action::ToggleTimer(tid));
                ui.close_menu();
            }
            ui.separator();
            if ui.button(RichText::new("Delete").color(t.danger)).clicked() {
                self.actions.push(Action::DeleteTask(tid));
                ui.close_menu();
            }
        });
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
        egui::ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                for (gi, g) in groups.iter().enumerate() {
                    let col_w = 252.0;
                    let open = g.status.as_ref().map(|s| s.kind == StatusKind::Open).unwrap_or(false);
                    let tint = if open || g.status.is_none() && g.color == t.text3 { Color32::from_black_alpha(9) } else { g.color.gamma_multiply(0.07) };
                    let accent = if open { t.text2 } else { g.color };
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        let frame = egui::Frame::new().fill(tint).corner_radius(10).inner_margin(egui::Margin::same(8));
                        let (_, payload) = ui.dnd_drop_zone::<u64, ()>(frame, |ui| {
                            ui.set_width(col_w - 16.0);
                            ui.horizontal(|ui| {
                                self.group_pill(ui, g);
                                ui.label(RichText::new(g.tasks.len().to_string()).color(accent));
                            });
                            ui.add_space(4.0);
                            egui::ScrollArea::vertical().id_salt(("col", gi)).auto_shrink([false, true]).max_height(ui.available_height() - 60.0).show(ui, |ui| {
                                for id in &g.tasks {
                                    if let Some(task) = tasks.iter().find(|x| x.id == *id) {
                                        let resp = ui.dnd_drag_source(egui::Id::new(("card", task.id)), task.id, |ui| {
                                            self.card(ui, task, col_w - 16.0);
                                        });
                                        name_it(&resp.response, &format!("Card {}", task.name));
                                        if resp.response.clicked() {
                                            self.actions.push(Action::Open(task.id));
                                        }
                                    }
                                }
                                let (r, resp) = ui.allocate_exact_size(vec2(col_w - 16.0, 32.0), Sense::click());
                                if resp.hovered() {
                                    ui.painter().rect_filled(r, 6.0, Color32::from_black_alpha(8));
                                }
                                ui.painter().text(r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, "+  Add Task", FontId::proportional(14.0), accent);
                                if resp.clicked() {
                                    self.add_from_group(g, String::new(), true);
                                }
                            });
                        });
                        if let Some(p) = payload {
                            dropped = Some((*p, gi));
                        }
                    });
                }
                if self.group_by == GroupBy::Status {
                    if let Some(sid) = self.data.lists_in(self.loc).and_then(|l| l.first().copied()).and_then(|l| self.data.space_of_list(l)).map(|s| s.id) {
                        if ui.add(egui::Button::new(RichText::new("+  Add group").color(t.text2).size(14.0)).frame(false)).on_hover_text("Edit statuses").clicked() {
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
                    GroupBy::DueDate => task.due = g.due,
                    GroupBy::None => {}
                }
                self.actions.push(Action::Replace(task));
            }
        }
    }

    fn card(&self, ui: &mut Ui, task: &Task, w: f32) {
        let t = self.t();
        let closed = self.data.is_closed(task);
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(11))
            .show(ui, |ui| {
                ui.set_width(w - 24.0);
                if !matches!(self.loc, Location::List(_)) {
                    ui.label(RichText::new(self.data.list_name(task.list_id)).size(11.5).color(t.text3));
                }
                let mut name = RichText::new(&task.name).size(14.0).color(t.text);
                if closed {
                    name = name.strikethrough().color(t.text3);
                }
                ui.add(egui::Label::new(name).wrap());
                if !task.tags.is_empty() && self.data.extra.apps.tags {
                    ui.horizontal_wrapped(|ui| {
                        for tag in &task.tags {
                            chip(ui, tag, tag_color(tag));
                        }
                    });
                }
                ui.add_space(2.0);
                let (r, _) = ui.allocate_exact_size(vec2(w - 24.0, 22.0), Sense::hover());
                let p = ui.painter();
                let icon_col = Color32::from_rgb(150, 150, 150);
                let boxed = |x: f32| Rect::from_min_size(pos2(x, r.top()), vec2(22.0, 22.0));
                let mut x = r.left();
                let used = paint_avatars(p, pos2(x, r.center().y), 10.0, &task.assignees, &self.data, t.card);
                if used > 0.0 {
                    x += used + 6.0;
                } else {
                    let b = boxed(x);
                    p.rect_stroke(b, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                    paint_person_icon(p, b.center(), icon_col);
                    x += 26.0;
                }
                match task.due {
                    Some(d) => {
                        let g = p.layout_no_wrap(fmt_date(d), FontId::proportional(12.5), date_color(d, closed, &t));
                        let b = Rect::from_min_size(pos2(x, r.top()), vec2(g.size().x + 26.0, 22.0));
                        p.rect_stroke(b, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                        paint_calendar_icon(p, b.left_center() + vec2(11.0, 0.0), icon_col);
                        let gw = g.size().x;
                        p.galley(pos2(b.left() + 20.0, b.center().y - g.size().y / 2.0), g, t.text2);
                        x += gw + 30.0;
                    }
                    None => {
                        let b = boxed(x);
                        p.rect_stroke(b, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                        paint_calendar_icon(p, b.center(), icon_col);
                        x += 26.0;
                    }
                }
                let b = boxed(x);
                let show_pr = self.data.extra.apps.priorities;
                if show_pr {
                    p.rect_stroke(b, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                }
                if !show_pr {
                } else if task.priority > 0 {
                    paint_flag(p, b.center() + vec2(-3.0, 0.0), priority_color(task.priority, &t), 12.0);
                } else {
                    paint_outline_flag(p, b.center(), icon_col);
                }
                let nsub = self.data.tasks.iter().filter(|s| s.parent == Some(task.id)).count();
                if nsub > 0 {
                    p.text(pos2(b.right() + 8.0, r.center().y), Align2::LEFT_CENTER, format!("\u{2934} {}", nsub), FontId::proportional(12.0), t.text3);
                }
            });
        ui.add_space(6.0);
    }
}
