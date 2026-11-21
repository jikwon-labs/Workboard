// Line icons drawn with the painter: one size (16 pt), one stroke (1.5 px)
// and the colour the caller passes (t.text2 beside text), in place of emoji
// and symbol glyphs, which egui drew from a black-and-white emoji font at
// sizes and baselines of their own.

use super::*;
use chrono::Datelike;
use egui::{Pos2, Response};

mod painted;

pub(super) use painted::*;

/// Every kind `line_icon` draws (checked by a test, so a misspelt kind shows up).
pub(super) const KINDS: &[&str] = &[
    "home",
    "planner",
    "ai",
    "teams",
    "more",
    "chat",
    "docs",
    "dashboards",
    "whiteboards",
    "forms",
    "goals",
    "timesheets",
    "clock",
    "upgrade",
    "check_plus",
    "video",
    "mic",
    "phone",
    "robot",
    "edit",
    "share",
    "inbox",
    "reply",
    "person",
    "person_comment",
    "megaphone",
    "bolt",
    "heart",
    "dots",
    "settings",
    "apps",
    "templates",
    "fields",
    "tag",
    "building",
    "smile",
    "frown",
    "bell",
    "bell_off",
    "palette",
    "sparkle",
    "keyboard",
    "download",
    "upload",
    "refresh",
    "help",
    "task",
    "send",
    "list",
    "hash",
    "space",
    "sidebar",
    "briefcase",
    "alarm",
    "notebook",
    "pin",
    "calendar",
    "link",
    "play",
    "stop",
    "sun",
    "moon",
    "auto",
    "bus",
    "plane",
    "search",
    "lock",
    "filter",
    "columns",
    "table",
    "subtasks",
    "check",
    "flag",
    "status",
    "hourglass",
    "repeat",
    "attach",
    "eye",
    "eye_off",
    "bookmark",
    "pen",
    "sticky",
    "eraser",
    "move",
    "shapes",
    "chart",
    "trash",
    "logout",
    "plus",
    "login",
    "undo",
    "redo",
    "fit",
    "at",
    "volume",
    "badge",
    "wifi",
    "person_plus",
    "external",
    "chevron_down",
];

/// Points on a circle around `c` of radius `r`, from angle `a0` to `a1`
/// (radians, clockwise on screen when `a1 > a0`).
fn arc_points(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = ((a1 - a0).abs() * r / 1.5).ceil().max(4.0) as usize;
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

/// `line_icon` drawn `scale` times larger around `c` (the big icon of an
/// empty page), with the stroke kept at about the usual weight.
pub(super) fn line_icon_scaled(
    p: &egui::Painter,
    kind: &str,
    c: Pos2,
    color: Color32,
    scale: f32,
) -> bool {
    let layer = p.layer_id();
    let start = p.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let ok = line_icon(p, kind, c, color);
    let end = p.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let tf = egui::emath::TSTransform::new(c.to_vec2() * (1.0 - scale), scale);
    p.ctx()
        .graphics_mut(|g| g.entry(layer).transform_range(start, end, tf));
    ok
}

/// A line icon before a label, in one row (a field's name in the task
/// window, "Drop on a day").
pub(super) fn icon_label(
    ui: &mut Ui,
    kind: &str,
    text: impl Into<egui::WidgetText>,
    color: Color32,
) -> Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = ICON_GAP;
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        line_icon(ui.painter(), kind, r.center(), color);
        ui.label(text)
    })
    .inner
}

/// A small line icon and a count after it ("3", "2/5") painted from the
/// left centre `at`; returns the width used.
pub(super) fn icon_count(
    p: &egui::Painter,
    kind: &str,
    at: Pos2,
    text: &str,
    color: Color32,
) -> f32 {
    line_icon_scaled(p, kind, at + vec2(6.0, 0.0), color, 0.75);
    let r = p.text(
        at + vec2(15.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(12.0),
        color,
    );
    r.right() - at.x
}

/// The back and forward buttons beside a week or month (Calendar,
/// Planner, Timesheets): framed, square, as tall as the other controls.
/// Returns (back clicked, forward clicked).
pub(super) fn period_arrows(ui: &mut Ui, t: &Theme) -> (bool, bool) {
    let back = icon_button(ui, t, "chevron_left", "", "Previous period", true)
        .on_hover_text(tr("Previous"))
        .clicked();
    let fwd = icon_button(ui, t, "chevron_right", "", "Next period", true)
        .on_hover_text(tr("Next"))
        .clicked();
    (back, fwd)
}

/// The gap between an icon and the text after it.
pub(super) const ICON_GAP: f32 = 6.0;

/// A button with a line icon before its text (or only the icon when `text`
/// is empty, named `name` for screen readers and tests). `framed` draws the
/// theme's outline like a plain button; otherwise only the hover fill shows.
/// It is as tall as the other controls (widgets::CONTROL_H).
pub(super) fn icon_button(
    ui: &mut Ui,
    t: &Theme,
    kind: &str,
    text: &str,
    name: &str,
    framed: bool,
) -> Response {
    icon_button_ex(ui, t, kind, text, name, framed, 0.0, t.text, t.text2)
}

/// `icon_button` at least `min_w` wide (the content centred) and in colours
/// of the caller's choosing.
#[allow(clippy::too_many_arguments)]
pub(super) fn icon_button_ex(
    ui: &mut Ui,
    t: &Theme,
    kind: &str,
    text: &str,
    name: &str,
    framed: bool,
    min_w: f32,
    text_color: Color32,
    icon_color: Color32,
) -> Response {
    let (text_color, icon_color) = if ui.is_enabled() {
        (text_color, icon_color)
    } else {
        (t.text3, t.text3)
    };
    let g = (!text.is_empty()).then(|| {
        ui.painter()
            .layout_no_wrap(text.to_string(), FontId::proportional(14.0), text_color)
    });
    let pad = if g.is_some() { 9.0 } else { 6.0 };
    let content = 16.0 + g.as_ref().map_or(0.0, |g| ICON_GAP + g.size().x);
    let w = (pad * 2.0 + content).max(min_w);
    let (r, resp) = ui.allocate_exact_size(vec2(w, crate::widgets::CONTROL_H), Sense::click());
    name_it(&resp, if name.is_empty() { text } else { name });
    let hovered = resp.hovered() && resp.enabled();
    if framed {
        let fill = if hovered { t.hover } else { t.bg };
        ui.painter().rect_filled(r, 6.0, fill);
        ui.painter().rect_stroke(
            r,
            6.0,
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
    } else if hovered {
        ui.painter()
            .rect_filled(r, 6.0, crate::widgets::hover_fill(ui));
    }
    let left = r.center().x - content / 2.0;
    let ic = pos2(left + 8.0, r.center().y);
    line_icon(ui.painter(), kind, ic, icon_color);
    if let Some(g) = g {
        let x = ic.x + 8.0 + ICON_GAP;
        ui.painter()
            .galley(pos2(x, r.center().y - g.size().y / 2.0), g, text_color);
    }
    resp
}

/// Thin outline icons in the style of the original (1.5 px strokes), drawn
/// in a 16 pt square around `c`. Returns false for a kind it does not know.
/// A "+" drawn as two lines centred on `c`, about as big as the "+"
/// character of a `size` px font. The character sits below the middle of
/// its line (by about a tenth of the size in the system font), so in a box,
/// a circle or a button it looked low.
pub(super) fn paint_plus(p: &egui::Painter, c: Pos2, size: f32, color: Color32) {
    let (arm, s) = sign_metrics(size, color);
    p.line_segment([c - vec2(arm, 0.0), c + vec2(arm, 0.0)], s);
    p.line_segment([c - vec2(0.0, arm), c + vec2(0.0, arm)], s);
}

/// The "-" that goes with `paint_plus` (a zoom out beside a zoom in).
pub(super) fn paint_minus(p: &egui::Painter, c: Pos2, size: f32, color: Color32) {
    let (arm, s) = sign_metrics(size, color);
    p.line_segment([c - vec2(arm, 0.0), c + vec2(arm, 0.0)], s);
}

fn sign_metrics(size: f32, color: Color32) -> (f32, Stroke) {
    let arm = (size * 0.32).round().max(3.0);
    (arm, Stroke::new((size * 0.1).clamp(1.2, 2.0), color))
}

/// The text of a button that shows a "+" (or "-") in its middle: the
/// character keeps the button's size and frame but is not seen, and
/// `paint_sign` draws the sign over it.
pub(super) fn sign_text(minus: bool) -> egui::RichText {
    egui::RichText::new(if minus { "\u{2212}" } else { "+" }).color(Color32::TRANSPARENT)
}

/// A framed button `side` across with `text` (one glyph) in its middle. A
/// button is as wide as its text plus the side padding, so with one glyph
/// in it it came out wider than tall. `name` is its accessible name.
pub(super) fn square_button(
    ui: &mut Ui,
    text: impl Into<egui::WidgetText>,
    side: f32,
    name: &str,
) -> Response {
    let resp = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = egui::Vec2::ZERO;
            // small: no stretching to the row height (interact_size)
            ui.add(
                egui::Button::new(text)
                    .small()
                    .min_size(egui::Vec2::splat(side)),
            )
        })
        .inner;
    crate::widgets::name_it(&resp, name);
    resp
}

/// A small framed "+" (or "-") button, square and as tall as a line of text.
/// `tip` is its tooltip and its name.
pub(super) fn sign_button(ui: &mut Ui, minus: bool, tip: &str) -> Response {
    let side = ui.text_style_height(&egui::TextStyle::Body);
    let resp = square_button(ui, sign_text(minus), side, tip);
    paint_sign(ui, &resp, minus, None);
    resp.on_hover_text(crate::i18n::tr(tip))
}

/// Draws the sign of a `sign_text` button in its middle, in `color` or the
/// colour the button's text would have.
pub(super) fn paint_sign(ui: &Ui, resp: &Response, minus: bool, color: Option<Color32>) {
    let color = color.unwrap_or_else(|| ui.style().interact(resp).text_color());
    let color = if resp.enabled() {
        color
    } else {
        color.gamma_multiply(0.5)
    };
    let size = egui::TextStyle::Body.resolve(ui.style()).size;
    if minus {
        paint_minus(ui.painter(), resp.rect.center(), size, color);
    } else {
        paint_plus(ui.painter(), resp.rect.center(), size, color);
    }
}

pub(super) fn line_icon(p: &egui::Painter, kind: &str, c: Pos2, color: Color32) -> bool {
    let s = Stroke::new(1.5_f32, color);
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
                Stroke::new(1.2_f32, color),
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
        // ------------------------------------------------ menus and toolbars
        "settings" => {
            p.circle_stroke(c, 4.8, s);
            p.circle_stroke(c, 1.9, s);
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 4.8, c + d * 7.3], Stroke::new(2.2_f32, color));
            }
        }
        "apps" => {
            for (x, y) in [(-3.8, -3.8), (3.8, -3.8), (-3.8, 3.8), (3.8, 3.8)] {
                p.rect_stroke(
                    Rect::from_center_size(c + vec2(x, y), vec2(5.5, 5.5)),
                    1.5,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
        }
        "templates" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(1.5, 1.5), vec2(11.0, 11.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            let pts = vec![
                c + vec2(-6.5, 3.0),
                c + vec2(-6.5, -6.5),
                c + vec2(3.0, -6.5),
            ];
            p.add(egui::Shape::line(pts, s));
        }
        "fields" => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(15.0, 10.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-4.0, -2.0), vec2(-4.0, 2.0));
            line(vec2(-1.0, 0.0), vec2(4.0, 0.0));
        }
        "tag" => {
            let pts = vec![
                c + vec2(-7.0, -7.0),
                c + vec2(0.5, -7.0),
                c + vec2(7.5, 0.0),
                c + vec2(0.0, 7.5),
                c + vec2(-7.0, 0.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            p.circle_filled(c + vec2(-3.2, -3.2), 1.3, color);
        }
        "building" => {
            p.rect_stroke(
                Rect::from_min_max(c + vec2(-6.0, -7.5), c + vec2(6.0, 7.5)),
                1.5,
                s,
                egui::StrokeKind::Middle,
            );
            for (x, y) in [(-2.5, -4.0), (2.5, -4.0), (-2.5, -0.5), (2.5, -0.5)] {
                p.circle_filled(c + vec2(x, y), 1.0, color);
            }
            line(vec2(-1.5, 7.5), vec2(-1.5, 3.5));
            line(vec2(-1.5, 3.5), vec2(1.5, 3.5));
            line(vec2(1.5, 3.5), vec2(1.5, 7.5));
        }
        "smile" | "frown" => {
            p.circle_stroke(c, 7.0, s);
            p.circle_filled(c + vec2(-2.5, -2.0), 1.0, color);
            p.circle_filled(c + vec2(2.5, -2.0), 1.0, color);
            let up = kind == "smile";
            let mut arc = vec![];
            for i in 0..=8 {
                let a = std::f32::consts::PI * (0.2 + 0.6 * i as f32 / 8.0);
                let (cy, dir) = if up { (0.5, 1.0) } else { (6.0, -1.0) };
                arc.push(c + vec2(a.cos() * 3.5, cy + dir * a.sin() * 3.0 - dir * 0.0));
            }
            p.add(egui::Shape::line(arc, s));
        }
        "bell" | "bell_off" => {
            let mut pts = vec![c + vec2(-6.0, 4.0)];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                pts.push(c + vec2(0.0, -2.0) + vec2(a.cos() * 4.5, a.sin() * 4.5));
            }
            pts.push(c + vec2(6.0, 4.0));
            pts.insert(1, c + vec2(-4.5, 1.0));
            pts.insert(pts.len() - 1, c + vec2(4.5, 1.0));
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-1.8, 6.8), vec2(1.8, 6.8));
            if kind == "bell_off" {
                line(vec2(-7.0, -7.0), vec2(7.0, 7.0));
            }
        }
        "palette" => {
            p.circle_stroke(c, 7.0, s);
            for (x, y) in [(-3.0, -2.5), (1.0, -4.0), (3.8, -0.5)] {
                p.circle_filled(c + vec2(x, y), 1.2, color);
            }
            p.circle_stroke(c + vec2(-1.5, 3.0), 1.6, Stroke::new(1.2_f32, color));
        }
        "sparkle" => {
            let star = |o: egui::Vec2, r: f32| {
                let k = r * 0.28;
                let pts = vec![
                    c + o + vec2(0.0, -r),
                    c + o + vec2(k, -k),
                    c + o + vec2(r, 0.0),
                    c + o + vec2(k, k),
                    c + o + vec2(0.0, r),
                    c + o + vec2(-k, k),
                    c + o + vec2(-r, 0.0),
                    c + o + vec2(-k, -k),
                ];
                p.add(egui::Shape::closed_line(pts, s));
            };
            star(vec2(-1.5, 1.0), 6.0);
            p.circle_filled(c + vec2(5.5, -5.5), 1.3, color);
        }
        "keyboard" => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(16.0, 11.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            for x in [-4.5, -1.5, 1.5, 4.5] {
                p.circle_filled(c + vec2(x, -2.0), 0.9, color);
            }
            line(vec2(-3.5, 2.0), vec2(3.5, 2.0));
        }
        "download" | "upload" => {
            let (a, b) = if kind == "download" {
                (vec2(0.0, -7.0), vec2(0.0, 2.5))
            } else {
                (vec2(0.0, 2.5), vec2(0.0, -7.0))
            };
            line(a, b);
            line(
                b,
                b + vec2(-3.5, if kind == "download" { -3.5 } else { 3.5 }),
            );
            line(
                b,
                b + vec2(3.5, if kind == "download" { -3.5 } else { 3.5 }),
            );
            line(vec2(-6.5, 4.0), vec2(-6.5, 7.0));
            line(vec2(-6.5, 7.0), vec2(6.5, 7.0));
            line(vec2(6.5, 7.0), vec2(6.5, 4.0));
        }
        "refresh" => {
            let (a0, a1) = (-1.2f32, 3.9f32);
            let arc: Vec<Pos2> = (0..=20)
                .map(|i| a0 + (a1 - a0) * i as f32 / 20.0)
                .map(|a| c + vec2(a.cos(), a.sin()) * 6.0)
                .collect();
            p.add(egui::Shape::line(arc, s));
            // arrow head at the start, pointing back along the circle
            let tip = c + vec2(a0.cos(), a0.sin()) * 6.0;
            let back = vec2(a0.sin(), -a0.cos());
            let out = vec2(a0.cos(), a0.sin());
            p.line_segment([tip, tip - back * 3.2 + out * 2.6], s);
            p.line_segment([tip, tip - back * 3.2 - out * 2.6], s);
        }
        "help" => {
            p.circle_stroke(c, 7.0, s);
            p.text(
                c + vec2(0.2, 0.3),
                Align2::CENTER_CENTER,
                "?",
                FontId::proportional(10.5),
                color,
            );
        }
        "task" => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(14.0, 14.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-3.5, 0.0), vec2(-1.0, 2.6));
            line(vec2(-1.0, 2.6), vec2(3.8, -2.4));
        }
        "send" => {
            let pts = vec![
                c + vec2(-7.0, -6.0),
                c + vec2(7.5, 0.0),
                c + vec2(-7.0, 6.0),
                c + vec2(-4.5, 0.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-4.5, 0.0), vec2(1.0, 0.0));
        }
        "list" => {
            for y in [-4.5, 0.0, 4.5] {
                p.circle_filled(c + vec2(-5.5, y), 1.1, color);
                line(vec2(-2.5, y), vec2(7.0, y));
            }
        }
        "hash" => {
            line(vec2(-2.0, -7.0), vec2(-3.5, 7.0));
            line(vec2(3.5, -7.0), vec2(2.0, 7.0));
            line(vec2(-6.5, -2.5), vec2(7.0, -2.5));
            line(vec2(-7.0, 2.5), vec2(6.5, 2.5));
        }
        "space" => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(14.0, 14.0)),
                4.0,
                s,
                egui::StrokeKind::Middle,
            );
            p.circle_stroke(c, 2.5, s);
        }
        "sidebar" => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(16.0, 13.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-2.5, -6.5), vec2(-2.5, 6.5));
        }
        "briefcase" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, 1.5), vec2(16.0, 11.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            p.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, -5.5), vec2(6.0, 3.0)),
                1.0,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-8.0, 1.5), vec2(8.0, 1.5));
        }
        "alarm" => {
            p.circle_stroke(c + vec2(0.0, 1.0), 6.0, s);
            line(vec2(0.0, 1.0), vec2(0.0, -2.0));
            line(vec2(0.0, 1.0), vec2(2.2, 2.4));
            line(vec2(-7.0, -4.5), vec2(-4.5, -7.0));
            line(vec2(7.0, -4.5), vec2(4.5, -7.0));
        }
        "notebook" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(1.0, 0.0), vec2(12.0, 15.0)),
                2.0,
                s,
                egui::StrokeKind::Middle,
            );
            for y in [-4.0, 0.0, 4.0] {
                line(vec2(-6.5, y), vec2(-3.5, y));
            }
            line(vec2(0.5, -2.0), vec2(4.0, -2.0));
        }
        "pin" => {
            let pts = vec![
                c + vec2(-3.0, -7.0),
                c + vec2(3.0, -7.0),
                c + vec2(2.0, -1.5),
                c + vec2(5.0, 1.5),
                c + vec2(-5.0, 1.5),
                c + vec2(-2.0, -1.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(0.0, 1.5), vec2(0.0, 7.5));
        }
        "calendar" => {
            let r = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(15.0, 14.0));
            p.rect_stroke(r, 3.0, s, egui::StrokeKind::Middle);
            line(vec2(-7.5, -3.0), vec2(7.5, -3.0));
            line(vec2(-3.5, -8.5), vec2(-3.5, -5.0));
            line(vec2(3.5, -8.5), vec2(3.5, -5.0));
        }
        "link" => {
            for x in [-3.2, 3.2] {
                p.rect_stroke(
                    Rect::from_center_size(c + vec2(x, 0.0), vec2(9.0, 6.5)),
                    3.25,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
        }
        "play" => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    c + vec2(-4.0, -6.0),
                    c + vec2(6.0, 0.0),
                    c + vec2(-4.0, 6.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
        "stop" => {
            p.rect_filled(Rect::from_center_size(c, vec2(10.0, 10.0)), 2.0, color);
        }
        "sun" => {
            p.circle_stroke(c, 3.5, s);
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 5.8, c + d * 7.8], s);
            }
        }
        "moon" => {
            // a crescent: the part of one circle outside a second, offset one
            let (r1, o2, r2) = (7.0, vec2(3.2, -2.6), 5.6);
            let n = 72;
            let outer: Vec<Pos2> = (0..n)
                .map(|i| std::f32::consts::TAU * i as f32 / n as f32)
                .map(|a| c + vec2(a.cos(), a.sin()) * r1)
                .collect();
            let start = (0..n)
                .find(|&i| {
                    (outer[i] - (c + o2)).length() > r2
                        && (outer[(i + n - 1) % n] - (c + o2)).length() <= r2
                })
                .unwrap_or(0);
            let mut pts: Vec<Pos2> = (0..n)
                .map(|k| outer[(start + k) % n])
                .take_while(|q| (*q - (c + o2)).length() > r2)
                .collect();
            let inner: Vec<Pos2> = (0..n)
                .map(|i| std::f32::consts::TAU * i as f32 / n as f32)
                .map(|a| c + o2 + vec2(a.cos(), a.sin()) * r2)
                .filter(|q| (*q - c).length() < r1)
                .collect();
            let mut inner = inner;
            // walk back from where the outer arc ends
            if let (Some(end), Some(first)) = (pts.last(), inner.first()) {
                if (*end - *first).length() > (*end - *inner.last().unwrap()).length() {
                    inner.reverse();
                }
            }
            // the inner points come in angle order: rotate so the run is unbroken
            let gap = (1..inner.len())
                .max_by(|&a, &b| {
                    let da = (inner[a] - inner[a - 1]).length();
                    let db = (inner[b] - inner[b - 1]).length();
                    da.partial_cmp(&db).unwrap()
                })
                .unwrap_or(0);
            if inner.len() > 2 && (inner[gap] - inner[gap - 1]).length() > 2.0 {
                inner.rotate_left(gap);
                if let (Some(end), Some(first)) = (pts.last(), inner.first()) {
                    if (*end - *first).length() > (*end - *inner.last().unwrap()).length() {
                        inner.reverse();
                    }
                }
            }
            pts.extend(inner);
            p.add(egui::Shape::closed_line(pts, s));
        }
        "auto" => {
            p.circle_stroke(c, 7.0, s);
            let mut pts = vec![c + vec2(0.0, -7.0)];
            for i in 0..=12 {
                let a = std::f32::consts::PI * (-0.5 + i as f32 / 12.0);
                pts.push(c + vec2(a.cos(), a.sin()) * 7.0);
            }
            p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        }
        "bus" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, -0.5), vec2(13.0, 13.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-6.5, 1.5), vec2(6.5, 1.5));
            p.circle_filled(c + vec2(-3.5, 4.0), 1.0, color);
            p.circle_filled(c + vec2(3.5, 4.0), 1.0, color);
            line(vec2(-4.0, 6.0), vec2(-4.0, 8.0));
            line(vec2(4.0, 6.0), vec2(4.0, 8.0));
        }
        "plane" => {
            let pts = vec![
                c + vec2(7.5, -7.5),
                c + vec2(-7.0, -1.5),
                c + vec2(-1.0, 1.0),
                c + vec2(1.5, 7.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-1.0, 1.0), vec2(7.5, -7.5));
        }
        "search" => {
            p.circle_stroke(c + vec2(-1.5, -1.5), 5.0, s);
            line(vec2(2.2, 2.2), vec2(6.5, 6.5));
        }
        "lock" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, 2.5), vec2(12.0, 9.0)),
                2.0,
                s,
                egui::StrokeKind::Middle,
            );
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                arc.push(c + vec2(0.0, -2.0) + vec2(a.cos() * 3.5, a.sin() * 4.0));
            }
            arc.insert(0, c + vec2(-3.5, -2.0));
            arc.push(c + vec2(3.5, -2.0));
            p.add(egui::Shape::line(arc, s));
        }
        "filter" => {
            line(vec2(-7.0, -4.5), vec2(7.0, -4.5));
            line(vec2(-4.5, 0.0), vec2(4.5, 0.0));
            line(vec2(-2.0, 4.5), vec2(2.0, 4.5));
        }
        "columns" | "table" => {
            let r = Rect::from_center_size(c, vec2(15.0, 13.0));
            p.rect_stroke(r, 2.5, s, egui::StrokeKind::Middle);
            line(vec2(-2.5, -6.5), vec2(-2.5, 6.5));
            line(vec2(2.5, -6.5), vec2(2.5, 6.5));
            if kind == "table" {
                line(vec2(-7.5, -2.0), vec2(7.5, -2.0));
                line(vec2(-7.5, 2.5), vec2(7.5, 2.5));
            }
        }
        "subtasks" => {
            line(vec2(-5.0, -7.0), vec2(-5.0, 4.5));
            line(vec2(-5.0, -2.0), vec2(1.0, -2.0));
            line(vec2(-5.0, 4.5), vec2(1.0, 4.5));
            p.circle_stroke(c + vec2(4.0, -2.0), 2.3, s);
            p.circle_stroke(c + vec2(4.0, 4.5), 2.3, s);
        }
        "check" => {
            line(vec2(-6.0, 0.5), vec2(-2.0, 4.5));
            line(vec2(-2.0, 4.5), vec2(6.5, -4.5));
        }
        "flag" => {
            line(vec2(-5.0, -7.5), vec2(-5.0, 7.5));
            let pts = vec![
                c + vec2(-5.0, -7.0),
                c + vec2(6.0, -7.0),
                c + vec2(3.5, -3.5),
                c + vec2(6.0, 0.0),
                c + vec2(-5.0, 0.0),
            ];
            p.add(egui::Shape::line(pts, s));
        }
        "status" => {
            p.circle_stroke(c, 6.5, s);
            p.circle_filled(c, 2.4, color);
        }
        "hourglass" => {
            line(vec2(-5.0, -7.5), vec2(5.0, -7.5));
            line(vec2(-5.0, 7.5), vec2(5.0, 7.5));
            let pts = vec![
                c + vec2(-4.0, -7.5),
                c + vec2(4.0, -7.5),
                c + vec2(-4.0, 7.5),
                c + vec2(4.0, 7.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
        }
        "repeat" => {
            line(vec2(-6.5, 1.0), vec2(-6.5, -3.0));
            line(vec2(-6.5, -3.0), vec2(5.5, -3.0));
            line(vec2(5.5, -3.0), vec2(3.0, -5.5));
            line(vec2(6.5, -1.0), vec2(6.5, 3.0));
            line(vec2(6.5, 3.0), vec2(-5.5, 3.0));
            line(vec2(-5.5, 3.0), vec2(-3.0, 5.5));
        }
        "attach" => {
            let mut pts = vec![c + vec2(4.0, -2.0), c + vec2(4.0, 3.5)];
            for i in 0..=8 {
                let a = std::f32::consts::PI * (i as f32 / 8.0);
                pts.push(c + vec2(0.0, 3.5) + vec2(a.cos() * 4.0, a.sin() * 4.0));
            }
            pts.push(c + vec2(-4.0, -4.0));
            for i in 0..=8 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 8.0);
                pts.push(c + vec2(-1.5, -4.0) + vec2(a.cos() * 2.5, a.sin() * 2.5));
            }
            pts.push(c + vec2(1.0, 3.0));
            p.add(egui::Shape::line(pts, s));
        }
        "eye" => {
            // two wide arcs meeting in small round tips (the usual eye
            // outline), in a 20-unit box scaled to the icon size
            let k = 0.75;
            let mut pts: Vec<Pos2> = vec![];
            let mut arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32, n: usize| {
                // each arc starts where the last ended: skip that point, as
                // a zero-length segment throws spikes off the stroke
                for i in 1..=n {
                    let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                    pts.push(c + vec2(cx + r * a.cos(), cy + r * a.sin()) * k);
                }
            };
            arc(0.0, 3.75, 10.75, -157.6, -22.4, 20); // top
            arc(9.0, 0.0, 1.0, -20.4, 20.4, 4); // right tip
            arc(0.0, -3.75, 10.75, 22.4, 157.6, 20); // bottom
            arc(-9.0, 0.0, 1.0, 159.6, 200.4, 4); // left tip
            p.add(egui::Shape::closed_line(pts, s));
            p.circle_stroke(c, 2.2, s);
        }
        "eye_off" => {
            line_icon(p, "eye", c, color);
            line(vec2(-6.5, 6.5), vec2(6.5, -6.5));
        }
        "bookmark" => {
            let pts = vec![
                c + vec2(-5.0, -7.5),
                c + vec2(5.0, -7.5),
                c + vec2(5.0, 7.5),
                c + vec2(0.0, 3.5),
                c + vec2(-5.0, 7.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
        }
        "pen" => {
            let pts = vec![
                c + vec2(4.0, -7.0),
                c + vec2(7.0, -4.0),
                c + vec2(-3.5, 6.5),
                c + vec2(-7.5, 7.5),
                c + vec2(-6.5, 3.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(2.0, -5.0), vec2(5.0, -2.0));
        }
        "sticky" => {
            let pts = vec![
                c + vec2(-7.0, -7.0),
                c + vec2(7.0, -7.0),
                c + vec2(7.0, 2.0),
                c + vec2(2.0, 7.0),
                c + vec2(-7.0, 7.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(2.0, 7.0), vec2(2.0, 2.0));
            line(vec2(2.0, 2.0), vec2(7.0, 2.0));
        }
        "eraser" => {
            let pts = vec![
                c + vec2(-1.0, -7.0),
                c + vec2(7.0, 1.0),
                c + vec2(1.5, 6.5),
                c + vec2(-3.0, 6.5),
                c + vec2(-7.5, 2.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-4.5, -3.5), vec2(3.5, 4.5));
            line(vec2(1.5, 7.5), vec2(7.5, 7.5));
        }
        "connector" => {
            // two boxes joined by a line that turns a corner
            for k in [vec2(-4.5, -4.5), vec2(4.5, 4.5)] {
                p.rect_stroke(
                    Rect::from_center_size(c + k, vec2(6.0, 6.0)),
                    1.0,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
            line(vec2(-1.5, -4.5), vec2(4.5, -4.5));
            line(vec2(4.5, -4.5), vec2(4.5, 1.5));
        }
        "move" => {
            line(vec2(0.0, -7.5), vec2(0.0, 7.5));
            line(vec2(-7.5, 0.0), vec2(7.5, 0.0));
            for d in [
                vec2(0.0, -1.0),
                vec2(0.0, 1.0),
                vec2(-1.0, 0.0),
                vec2(1.0, 0.0),
            ] {
                let tip = d * 7.5;
                let side = vec2(-d.y, d.x) * 2.5;
                line(tip, tip - d * 2.5 + side);
                line(tip, tip - d * 2.5 - side);
            }
        }
        "shapes" => {
            p.circle_stroke(c + vec2(-3.0, -3.0), 3.8, s);
            p.rect_stroke(
                Rect::from_center_size(c + vec2(3.0, 3.0), vec2(7.5, 7.5)),
                1.5,
                s,
                egui::StrokeKind::Middle,
            );
        }
        "chart" => {
            line(vec2(-7.0, 7.0), vec2(7.5, 7.0));
            line(vec2(-4.0, 7.0), vec2(-4.0, 1.0));
            line(vec2(0.5, 7.0), vec2(0.5, -6.0));
            line(vec2(5.0, 7.0), vec2(5.0, -1.5));
        }
        "trash" => {
            line(vec2(-7.0, -4.5), vec2(7.0, -4.5));
            line(vec2(-2.5, -4.5), vec2(-2.0, -7.0));
            line(vec2(-2.0, -7.0), vec2(2.0, -7.0));
            line(vec2(2.0, -7.0), vec2(2.5, -4.5));
            let pts = vec![
                c + vec2(-5.5, -4.5),
                c + vec2(-4.5, 7.0),
                c + vec2(4.5, 7.0),
                c + vec2(5.5, -4.5),
            ];
            p.add(egui::Shape::line(pts, s));
        }
        "logout" => {
            let pts = vec![
                c + vec2(1.0, -7.0),
                c + vec2(-6.0, -7.0),
                c + vec2(-6.0, 7.0),
                c + vec2(1.0, 7.0),
            ];
            p.add(egui::Shape::line(pts, s));
            line(vec2(-2.0, 0.0), vec2(7.0, 0.0));
            line(vec2(7.0, 0.0), vec2(4.0, -3.0));
            line(vec2(7.0, 0.0), vec2(4.0, 3.0));
        }
        "plus" => {
            line(vec2(0.0, -6.5), vec2(0.0, 6.5));
            line(vec2(-6.5, 0.0), vec2(6.5, 0.0));
        }
        "undo" | "redo" => {
            // a hook from the left arrow head round to the bottom; Redo's
            // is the same turned to face right
            let m = if kind == "redo" { -1.0 } else { 1.0 };
            let v = |x: f32, y: f32| vec2(x * m, y);
            let mut pts = vec![c + v(-6.5, -2.5)];
            for i in 0..=10 {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 10.0;
                pts.push(c + v(1.5 + a.cos() * 5.0, 1.5 + a.sin() * 4.0));
            }
            pts.push(c + v(-1.5, 5.5));
            p.add(egui::Shape::line(pts, s));
            line(v(-6.5, -2.5), v(-3.0, -6.0));
            line(v(-6.5, -2.5), v(-3.0, 1.0));
        }
        "fit" => {
            // four corners: the whole board in view
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let k = c + vec2(6.5 * sx, 6.5 * sy);
                p.line_segment([k, k - vec2(4.0 * sx, 0.0)], s);
                p.line_segment([k, k - vec2(0.0, 4.0 * sy)], s);
            }
        }
        "login" => {
            let pts = vec![
                c + vec2(-1.0, -7.0),
                c + vec2(6.0, -7.0),
                c + vec2(6.0, 7.0),
                c + vec2(-1.0, 7.0),
            ];
            p.add(egui::Shape::line(pts, s));
            line(vec2(-7.0, 0.0), vec2(2.0, 0.0));
            line(vec2(2.0, 0.0), vec2(-1.0, -3.0));
            line(vec2(2.0, 0.0), vec2(-1.0, 3.0));
        }
        "sort" => {
            line(vec2(-3.5, -7.0), vec2(-3.5, 7.0));
            line(vec2(-3.5, -7.0), vec2(-6.5, -4.0));
            line(vec2(-3.5, -7.0), vec2(-0.5, -4.0));
            line(vec2(3.5, 7.0), vec2(3.5, -7.0));
            line(vec2(3.5, 7.0), vec2(0.5, 4.0));
            line(vec2(3.5, 7.0), vec2(6.5, 4.0));
        }
        "archive" => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(0.0, -4.5), vec2(16.0, 4.5)),
                1.0,
                s,
                egui::StrokeKind::Middle,
            );
            let pts = vec![
                c + vec2(-6.5, -2.0),
                c + vec2(-6.5, 7.0),
                c + vec2(6.5, 7.0),
                c + vec2(6.5, -2.0),
            ];
            p.add(egui::Shape::line(pts, s));
            line(vec2(-2.5, 1.5), vec2(2.5, 1.5));
        }
        "close" => {
            line(vec2(-5.0, -5.0), vec2(5.0, 5.0));
            line(vec2(5.0, -5.0), vec2(-5.0, 5.0));
        }
        "pop_out" => {
            // a window with an arrow leaving it by the top right corner
            let pts = vec![
                c + vec2(-1.0, -6.0),
                c + vec2(-6.0, -6.0),
                c + vec2(-6.0, 6.0),
                c + vec2(6.0, 6.0),
                c + vec2(6.0, 1.0),
            ];
            p.add(egui::Shape::line(pts, s));
            line(vec2(-1.0, 1.0), vec2(6.5, -6.5));
            line(vec2(6.5, -6.5), vec2(2.0, -6.5));
            line(vec2(6.5, -6.5), vec2(6.5, -2.0));
        }
        "arrow_up_left" => {
            line(vec2(5.0, 6.0), vec2(5.0, -1.0));
            line(vec2(5.0, -1.0), vec2(-5.0, -1.0));
            line(vec2(-5.0, -1.0), vec2(-1.5, -4.5));
            line(vec2(-5.0, -1.0), vec2(-1.5, 2.5));
        }
        "folder" => {
            let pts = vec![
                c + vec2(-7.5, -5.5),
                c + vec2(-2.5, -5.5),
                c + vec2(-1.0, -3.5),
                c + vec2(7.5, -3.5),
                c + vec2(7.5, 6.0),
                c + vec2(-7.5, 6.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
        }
        "at" => {
            use std::f32::consts::PI;
            p.circle_stroke(c, 2.7, s);
            line(vec2(2.7, -2.7), vec2(2.7, 0.7));
            // the small hook back up, then the big ring round to the bottom
            p.add(egui::Shape::line(
                arc_points(c + vec2(4.7, 0.7), 2.0, PI, 0.0),
                s,
            ));
            line(vec2(6.7, 0.7), vec2(6.7, 0.0));
            p.add(egui::Shape::line(
                arc_points(c, 6.7, 0.0, 52.6_f32.to_radians() - 2.0 * PI),
                s,
            ));
        }
        "volume" => {
            let pts = vec![
                c + vec2(-7.0, -2.5),
                c + vec2(-4.0, -2.5),
                c + vec2(0.0, -6.0),
                c + vec2(0.0, 6.0),
                c + vec2(-4.0, 2.5),
                c + vec2(-7.0, 2.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            let (a, b) = (-50_f32.to_radians(), 50_f32.to_radians());
            p.add(egui::Shape::line(arc_points(c, 3.6, a, b), s));
            p.add(egui::Shape::line(arc_points(c, 6.8, a, b), s));
        }
        "badge" => {
            use std::f32::consts::{FRAC_PI_2, PI};
            // an app icon with a dot on its corner: the outline stops short
            // of the dot
            let mut pts = vec![c + vec2(1.0, -5.0)];
            pts.extend(arc_points(c + vec2(-4.0, -2.0), 3.0, -FRAC_PI_2, -PI));
            pts.extend(arc_points(c + vec2(-4.0, 4.0), 3.0, PI, FRAC_PI_2));
            pts.extend(arc_points(c + vec2(2.0, 4.0), 3.0, FRAC_PI_2, 0.0));
            pts.push(c + vec2(5.0, 0.0));
            p.add(egui::Shape::line(pts, s));
            p.circle_stroke(c + vec2(5.0, -4.5), 2.6, s);
        }
        "wifi" => {
            let (a, b) = (-130_f32.to_radians(), -50_f32.to_radians());
            let o = c + vec2(0.0, 6.5);
            for r in [4.0, 8.0, 12.0] {
                p.add(egui::Shape::line(arc_points(o, r, a, b), s));
            }
            p.circle_filled(o, 1.1, color);
        }
        "person_plus" => {
            p.circle_stroke(c + vec2(-2.5, -3.5), 3.0, s);
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                arc.push(c + vec2(-2.5, 7.0) + vec2(a.cos() * 5.5, a.sin() * 5.0));
            }
            p.add(egui::Shape::line(arc, s));
            line(vec2(5.5, -4.0), vec2(5.5, 2.0));
            line(vec2(2.5, -1.0), vec2(8.5, -1.0));
        }
        "external" => {
            line(vec2(-4.5, 4.5), vec2(5.0, -5.0));
            line(vec2(-0.5, -5.0), vec2(5.0, -5.0));
            line(vec2(5.0, -5.0), vec2(5.0, 0.5));
        }
        "warning" => {
            let pts = vec![c + vec2(0.0, -7.0), c + vec2(7.5, 6.5), c + vec2(-7.5, 6.5)];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(0.0, -2.5), vec2(0.0, 2.0));
            p.circle_filled(c + vec2(0.0, 4.3), 0.9, color);
        }
        "arrow_right" => {
            line(vec2(-6.5, 0.0), vec2(6.5, 0.0));
            line(vec2(6.5, 0.0), vec2(2.5, -4.0));
            line(vec2(6.5, 0.0), vec2(2.5, 4.0));
        }
        "chevron_left" => {
            line(vec2(2.5, -5.0), vec2(-2.5, 0.0));
            line(vec2(-2.5, 0.0), vec2(2.5, 5.0));
        }
        "chevron_right" => {
            line(vec2(-2.5, -5.0), vec2(2.5, 0.0));
            line(vec2(2.5, 0.0), vec2(-2.5, 5.0));
        }
        "chevron_down" => {
            line(vec2(-5.0, -2.5), vec2(0.0, 2.5));
            line(vec2(0.0, 2.5), vec2(5.0, -2.5));
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every listed kind draws, and the menus only name listed kinds.
    #[test]
    fn every_icon_kind_draws() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            for k in KINDS {
                assert!(line_icon(&p, k, pos2(20.0, 20.0), Color32::GRAY), "{}", k);
            }
            assert!(!line_icon(
                &p,
                "no such icon",
                pos2(20.0, 20.0),
                Color32::GRAY
            ));
        });
        for (_, _, kind) in super::super::menus::TOOLS {
            assert!(KINDS.contains(&kind), "tool icon {}", kind);
        }
    }
}
