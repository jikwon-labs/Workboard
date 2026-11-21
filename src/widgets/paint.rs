// Painted pieces: the status circle, priority flag, avatars and presence
// dots, the status pill, tags, and text fitted to a width.

use super::*;

pub fn paint_status_circle(p: &egui::Painter, center: Pos2, r: f32, st: &Status) {
    let c = rgb(st.color);
    match st.kind {
        StatusKind::Closed | StatusKind::Done => {
            p.circle_filled(center, r, c);
            // check mark
            let a = center + vec2(-r * 0.45, 0.0);
            let b = center + vec2(-r * 0.1, r * 0.38);
            let d = center + vec2(r * 0.5, -r * 0.35);
            p.line_segment([a, b], Stroke::new(1.6_f32, Color32::WHITE));
            p.line_segment([b, d], Stroke::new(1.6_f32, Color32::WHITE));
        }
        StatusKind::Custom => {
            p.circle_stroke(center, r, Stroke::new(1.5_f32, c));
            // half filled to show progress
            let mut pts = vec![center];
            for i in 0..=16 {
                let a = std::f32::consts::PI * (-0.5 + i as f32 / 16.0);
                pts.push(center + vec2(a.cos(), a.sin()) * (r - 2.5));
            }
            p.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
        }
        StatusKind::Open => {
            paint_dashed(p, center, r, c);
        }
    }
}

pub fn paint_dashed(p: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let n = 10;
    for i in 0..n {
        let a0 = std::f32::consts::TAU * (i as f32 / n as f32);
        let a1 = a0 + std::f32::consts::TAU / n as f32 * 0.55;
        p.line_segment(
            [
                c + vec2(a0.cos(), a0.sin()) * r,
                c + vec2(a1.cos(), a1.sin()) * r,
            ],
            Stroke::new(1.5_f32, color),
        );
    }
}

pub fn paint_flag(p: &egui::Painter, pos: Pos2, color: Color32, h: f32) {
    let top = pos + vec2(0.0, -h * 0.5);
    let bottom = pos + vec2(0.0, h * 0.5);
    p.line_segment([top, bottom], Stroke::new(1.5_f32, color));
    let w = h * 0.7;
    let pts = vec![top, top + vec2(w, h * 0.18), top + vec2(0.0, h * 0.4)];
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

pub fn priority_color(pr: u8, t: &Theme) -> Color32 {
    if pr == 0 {
        t.text3
    } else if pr == 4 && !t.dark {
        Color32::from_rgb(0xb0, 0xb0, 0xb0)
    } else {
        rgb(PRIORITIES[pr.min(4) as usize].color)
    }
}

/// The green of "online" dots.
pub const ONLINE: Color32 = Color32::from_rgb(48, 164, 108);

/// The orange of warnings and of what needs attention soon (due today,
/// waiting on, invited, saved messages, a connection that isn't working).
pub const WARN_ORANGE: Color32 = Color32::from_rgb(0xe1, 0x6b, 0x16);

/// The orange of "notifications muted".
pub const MUTED: Color32 = WARN_ORANGE;

/// The yellow of a favourite's or starred item's star.
pub const STAR_YELLOW: Color32 = Color32::from_rgb(0xf8, 0xae, 0x00);

/// A presence dot at `c` on an avatar: filled green when `online`, a grey
/// ring when not; `bg` is what's behind the avatar (it outlines the dot).
pub fn paint_presence(
    p: &egui::Painter,
    c: Pos2,
    r: f32,
    online: bool,
    bg: Color32,
    grey: Color32,
) {
    p.circle_filled(c, r + 1.2, bg);
    if online {
        p.circle_filled(c, r, ONLINE);
    } else {
        p.circle_filled(c, r, bg);
        p.circle_stroke(c, r - 0.6, Stroke::new(1.2_f32, grey));
    }
}

/// Where the app keeps the profile photos ready to draw this frame: an
/// `AvatarTextures` in egui's temp data, by photo file (sha256).
pub const AVATAR_TEXTURES: &str = "avatar_textures";
pub type AvatarTextures = std::sync::Arc<std::collections::HashMap<String, egui::TextureId>>;

/// The texture of a profile photo, once the app has it decoded.
pub fn avatar_texture(ctx: &egui::Context, file: &str) -> Option<egui::TextureId> {
    ctx.data(|d| d.get_temp::<AvatarTextures>(egui::Id::new(AVATAR_TEXTURES)))
        .and_then(|m| m.get(file).copied())
}

/// A square picture drawn as a circle of radius `r` at `c`.
pub fn paint_round_image(p: &egui::Painter, c: Pos2, r: f32, tex: egui::TextureId) {
    let mut mesh = egui::Mesh::with_texture(tex);
    let n: u32 = 48;
    let vertex = |pos: Pos2, uv: Pos2| egui::epaint::Vertex {
        pos,
        uv,
        color: Color32::WHITE,
    };
    mesh.vertices.push(vertex(c, pos2(0.5, 0.5)));
    for i in 0..=n {
        let (s, co) = (i as f32 / n as f32 * std::f32::consts::TAU).sin_cos();
        mesh.vertices.push(vertex(
            c + vec2(co, s) * r,
            pos2(0.5 + 0.5 * co, 0.5 + 0.5 * s),
        ));
    }
    for i in 0..n {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    p.add(egui::Shape::mesh(mesh));
}

/// A person's avatar: their profile photo when there is one (and it's
/// here), else their initials on their colour.
pub fn paint_avatar(p: &egui::Painter, center: Pos2, r: f32, m: &Member, ring: Color32) {
    p.circle_filled(center, r + 1.5, ring);
    if let Some(tex) = m
        .photo
        .as_ref()
        .and_then(|a| avatar_texture(p.ctx(), &a.file))
    {
        paint_round_image(p, center, r, tex);
        return;
    }
    p.circle_filled(center, r, rgb(m.color));
    p.text(
        center,
        Align2::CENTER_CENTER,
        m.initials(),
        FontId::proportional(r * 0.85),
        Color32::WHITE,
    );
}

/// The width `paint_avatars` will use for these people, to make room first.
pub fn avatars_width(r: f32, ids: &[u64], data: &Data) -> f32 {
    let n = ids
        .iter()
        .take(3)
        .filter(|id| data.member(**id).is_some())
        .count();
    if n == 0 {
        return 0.0;
    }
    r + n as f32 * r * 1.4 + if ids.len() > 3 { 18.0 } else { 0.0 }
}

/// Overlapping avatars; returns the width used.
pub fn paint_avatars(
    p: &egui::Painter,
    left: Pos2,
    r: f32,
    ids: &[u64],
    data: &Data,
    ring: Color32,
) -> f32 {
    let mut x = left.x + r;
    let mut n = 0;
    for id in ids.iter().take(3) {
        if let Some(m) = data.member(*id) {
            paint_avatar(p, pos2(x, left.y), r, m, ring);
            x += r * 1.4;
            n += 1;
        }
    }
    if ids.len() > 3 {
        p.text(
            pos2(x + 2.0, left.y),
            Align2::LEFT_CENTER,
            format!("+{}", ids.len() - 3),
            FontId::proportional(11.0),
            Color32::GRAY,
        );
        x += 18.0;
    }
    if n == 0 {
        0.0
    } else {
        x - left.x
    }
}

pub fn paint_empty_avatar(p: &egui::Painter, center: Pos2, r: f32, color: Color32) {
    p.circle_stroke(center, r, Stroke::new(1.0_f32, color));
    // person glyph
    p.circle_stroke(
        center + vec2(0.0, -r * 0.25),
        r * 0.28,
        Stroke::new(1.0_f32, color),
    );
    p.line_segment(
        [
            center + vec2(-r * 0.45, r * 0.5),
            center + vec2(r * 0.45, r * 0.5),
        ],
        Stroke::new(1.0_f32, color),
    );
}

/// Status pill: open statuses are a light grey pill with dark text and a dashed
/// circle; the others are filled with their colour and white text (as measured).
pub fn status_pill(ui: &mut Ui, st: &Status, sense: Sense) -> Response {
    let open = st.kind == StatusKind::Open;
    let dark = ui.visuals().dark_mode;
    let (fill, fg) = if open {
        (
            if dark {
                Color32::from_white_alpha(22)
            } else {
                Color32::from_black_alpha(15)
            },
            if dark {
                Color32::from_rgb(230, 230, 230)
            } else {
                Color32::from_rgb(32, 32, 32)
            },
        )
    } else {
        (rgb(st.color), Color32::WHITE)
    };
    let text = crate::i18n::status_text(&st.name).to_uppercase();
    let galley = ui
        .painter()
        .layout_no_wrap(text, FontId::proportional(12.0), fg);
    let size = vec2(galley.size().x + 30.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let fill = if resp.hovered() {
        fill.gamma_multiply(0.9)
    } else {
        fill
    };
    ui.painter().rect_filled(rect, 5.0, fill);
    let c = rect.left_center() + vec2(11.0, 0.0);
    match st.kind {
        StatusKind::Open => paint_dashed(ui.painter(), c, 4.5, fg),
        StatusKind::Custom => {
            ui.painter().circle_stroke(c, 4.5, Stroke::new(1.4_f32, fg));
            ui.painter().circle_filled(c, 2.0, fg);
        }
        _ => {
            ui.painter().circle_filled(c, 5.0, fg);
            ui.painter().line_segment(
                [c + vec2(-2.3, 0.0), c + vec2(-0.6, 1.8)],
                Stroke::new(1.3_f32, fill),
            );
            ui.painter().line_segment(
                [c + vec2(-0.6, 1.8), c + vec2(2.5, -1.8)],
                Stroke::new(1.3_f32, fill),
            );
        }
    }
    ui.painter().galley(
        pos2(rect.left() + 20.0, rect.center().y - galley.size().y / 2.0),
        galley,
        fg,
    );
    resp
}

/// For a chip in a `horizontal_wrapped` row whose text is `text` at `size`
/// points and whose padding and buttons add `extra`: starts a new row when
/// the chip doesn't fit in what's left of this one (egui wraps only text, so
/// the chip ran past the edge, or wrapped its words a letter per line), and
/// returns the widest its text may be before it is shortened.
pub fn chip_room(ui: &mut Ui, text: &str, size: f32, extra: f32) -> f32 {
    let w = ui
        .painter()
        .layout_no_wrap(one_line(text), FontId::proportional(size), Color32::WHITE)
        .size()
        .x;
    wrap_unless_room(ui, w + extra);
    (ui.max_rect().width() - extra).max(24.0)
}

/// In a `horizontal_wrapped` row: starts the next row when something `w`
/// wide doesn't fit in what's left of this one. For what egui doesn't wrap by
/// itself (a ComboBox, a framed chip), which otherwise runs past the edge.
pub fn wrap_unless_room(ui: &mut Ui, w: f32) {
    let at_start = ui.cursor().left() <= ui.max_rect().left() + 0.5;
    if !at_start && w > ui.available_size_before_wrap().x {
        ui.end_row();
    }
}

/// `text` on one line: each line break, with the spaces around it, becomes a
/// single space and blank lines are dropped.
pub fn one_line(text: &str) -> String {
    if !text.contains(['\n', '\r']) {
        return text.to_string();
    }
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `text` on one line, cut with an ellipsis so it is at most `max_w` wide in
/// `font`.
pub fn fit_text(p: &egui::Painter, text: &str, font: &FontId, max_w: f32) -> String {
    let text = &one_line(text);
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

/// Paints `text` at `pos`, cut with an ellipsis to at most `max_w` wide.
/// When it had to be cut, hovering it shows the whole text. Returns the
/// painted rect.
#[allow(clippy::too_many_arguments)]
pub fn paint_fit(
    ui: &Ui,
    pos: Pos2,
    align: Align2,
    text: &str,
    font: FontId,
    color: Color32,
    max_w: f32,
) -> Rect {
    let shown = fit_text(ui.painter(), text, &font, max_w.max(0.0));
    let cut = shown != text;
    let rect = ui.painter().text(pos, align, shown, font, color);
    if cut && ui.rect_contains_pointer(rect) {
        egui::show_tooltip_at_pointer(
            ui.ctx(),
            ui.layer_id(),
            egui::Id::new(("fit_text_tip", text)),
            |ui| {
                ui.set_max_width(360.0);
                ui.label(text);
            },
        );
    }
    rect
}

/// `a` moved toward `b` by `k` (0 = a, 1 = b), both taken as opaque.
pub fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// What to write or draw on a colour (a goal's): white, or near-black on a
/// colour too light for white to read (white below 3:1 against it, the
/// yellow among the goal colours).
pub fn ink_on(c: Color32) -> Color32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = 0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b());
    if 1.05 / (l + 0.05) < 3.0 {
        Color32::from_rgb(26, 26, 26)
    } else {
        Color32::WHITE
    }
}

/// The colour whiteboard ink `c` is drawn in: black (the pen's first
/// colour, which is the dark page's own colour) shows light on a dark page,
/// where it vanished. The board keeps it black, for teammates on a light
/// page.
pub fn ink(t: &Theme, c: [u8; 3]) -> Color32 {
    if t.dark && c.iter().all(|&v| v < 64) {
        t.text
    } else {
        rgb(c)
    }
}

/// A chip's text colour on a dark theme: the colour mixed halfway to white,
/// so a dark green or purple tag stays readable on its tinted background.
pub fn chip_text_on_dark(c: Color32) -> Color32 {
    let mix = |x: u8| x + ((255 - x as u16) / 2) as u8;
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

pub fn tag_color(tag: &str) -> Color32 {
    let mut h: u32 = 0;
    for b in tag.bytes() {
        h = h.wrapping_mul(31).wrapping_add(b as u32);
    }
    rgb(STATUS_PALETTE[1 + (h as usize % (STATUS_PALETTE.len() - 1))])
}

pub fn paint_triangle(p: &egui::Painter, center: Pos2, open: bool, color: Color32) {
    let s = 4.0;
    let pts = if open {
        vec![
            center + vec2(-s, -s * 0.5),
            center + vec2(s, -s * 0.5),
            center + vec2(0.0, s * 0.6),
        ]
    } else {
        vec![
            center + vec2(-s * 0.5, -s),
            center + vec2(s * 0.6, 0.0),
            center + vec2(-s * 0.5, s),
        ]
    };
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_joins_lines_with_single_spaces() {
        assert_eq!(one_line("plain  text"), "plain  text");
        assert_eq!(one_line("Lunch\nPizza \r\n\n  cash"), "Lunch Pizza cash");
    }

    #[test]
    fn fit_text_never_paints_a_line_break() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            let font = FontId::proportional(14.0);
            assert_eq!(fit_text(&p, "a\nb", &font, 1000.0), "a b");
            let cut = fit_text(&p, "first line\nsecond line", &font, 60.0);
            assert!(!cut.contains('\n') && cut.ends_with('\u{2026}'), "{cut}");
        });
    }
}
