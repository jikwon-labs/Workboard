// The look the form pages share: a form's tile (its colour and icon) and the
// icon picker, and the few kinds of control every button on these pages is
// one of: the primary button with an icon, a drop-down's button, the dashed
// "add" button and small pills. Segmented tabs (`seg`) are in icons.rs.

use super::*;

/// The colours a form's tile can take, by the name saved in `Form::color`.
pub(crate) const FORM_COLORS: [(&str, [u8; 3]); 8] = [
    ("blue", [0x33, 0x55, 0xe6]),
    ("teal", [0x0b, 0x7e, 0x80]),
    ("pink", [0xd2, 0x3f, 0x7e]),
    ("purple", [0x50, 0x46, 0xd8]),
    ("orange", [0xc2, 0x55, 0x10]),
    ("green", [0x1f, 0x8f, 0x4e]),
    ("red", [0xc2, 0x34, 0x3b]),
    ("gray", [0x60, 0x60, 0x60]),
];

/// The icons a form's tile can show (saved by their names).
pub(crate) const FORM_ICONS: [Icon; 24] = [
    Icon::Forms,
    Icon::Settings,
    Icon::Smile,
    Icon::Briefcase,
    Icon::Person,
    Icon::Calendar,
    Icon::Flag,
    Icon::Inbox,
    Icon::Bell,
    Icon::Heart,
    Icon::Home,
    Icon::Teams,
    Icon::Hash,
    Icon::List,
    Icon::Check,
    Icon::Sparkle,
    Icon::Edit,
    Icon::Eye,
    Icon::Chat,
    Icon::Docs,
    Icon::Goals,
    Icon::Megaphone,
    Icon::Bolt,
    Icon::Tag,
];

/// A form's name as the pages show it: "Untitled form" when it has none,
/// as an unnamed doc shows "Untitled".
pub(super) fn form_title(f: &Form) -> String {
    if f.name.trim().is_empty() {
        tr("Untitled form").into()
    } else {
        f.name.clone()
    }
}

/// A form's tile as (icon, icon colour, fill). Older forms (no icon or
/// colour saved) show the forms icon in a colour taken from their id; an
/// icon a newer app saved that this one doesn't draw falls back the same way.
pub(super) fn form_look(f: &Form, t: &Theme) -> (Icon, Color32, Color32) {
    let icon = FORM_ICONS
        .iter()
        .copied()
        .find(|k| k.name() == f.icon)
        .unwrap_or(Icon::Forms);
    let c = rgb(form_color(f).1);
    let fg = if t.dark { chip_text_on_dark(c) } else { c };
    (icon, fg, c.gamma_multiply(if t.dark { 0.3 } else { 0.13 }))
}

/// The form's colour as (name, rgb): the saved one, or one from its id.
pub(super) fn form_color(f: &Form) -> (&'static str, [u8; 3]) {
    FORM_COLORS
        .iter()
        .copied()
        .find(|c| c.0 == f.color)
        .unwrap_or(FORM_COLORS[(f.id % FORM_COLORS.len() as u64) as usize])
}

/// Paints a form's tile over `r`: the fill with rounded corners and the icon
/// scaled to the tile.
pub(super) fn paint_tile(p: &egui::Painter, r: Rect, look: (Icon, Color32, Color32)) {
    let (icon, fg, bg) = look;
    p.rect_filled(r, (r.width() * 0.25).round(), bg);
    line_icon_scaled(p, icon, r.center(), fg, r.width() / 28.0);
}

/// The icon picker: the tile's colours and icons. True when one was picked;
/// a pick applies at once.
pub(super) fn icon_picker(ui: &mut Ui, t: &Theme, f: &mut Form) -> bool {
    let mut changed = false;
    ui.set_width(292.0);
    ui.spacing_mut().item_spacing.y = 8.0;
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        paint_tile(ui.painter(), r, form_look(f, t));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(form_title(f)).size(14.0).color(t.text));
            ui.label(
                RichText::new(tr("Changes in the table and the sidebar too"))
                    .size(12.0)
                    .color(t.text2),
            );
        });
    });
    ui.separator();
    ui.label(RichText::new(tr("Color")).size(12.0).color(t.text2));
    let cur = form_color(f).0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (name, c) in FORM_COLORS {
            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            name_it(&resp, &format!("Color {}", name));
            let col = rgb(c);
            if name == cur {
                ui.painter()
                    .circle_stroke(r.center(), 12.0, Stroke::new(2.0_f32, col));
            } else if resp.hovered() {
                ui.painter()
                    .circle_stroke(r.center(), 12.0, Stroke::new(1.5_f32, hover_line(ui)));
            }
            ui.painter().circle_filled(r.center(), 8.5, col);
            if resp.clicked() && f.color != name {
                f.color = name.to_string();
                changed = true;
            }
        }
    });
    ui.label(RichText::new(tr("Icon")).size(12.0).color(t.text2));
    let (icon, fg, bg) = form_look(f, t);
    for row in FORM_ICONS.chunks(8) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for k in row {
                let (r, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                name_it(&resp, &format!("Icon {}", k.name()));
                let on = *k == icon;
                if on {
                    ui.painter().rect_filled(r, 8.0, bg);
                    ui.painter().rect_stroke(
                        r,
                        8.0,
                        Stroke::new(1.5_f32, fg),
                        egui::StrokeKind::Inside,
                    );
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 8.0, t.hover);
                }
                line_icon(ui.painter(), *k, r.center(), if on { fg } else { t.text2 });
                if resp.clicked() && f.icon != k.name() {
                    f.icon = k.name().to_string();
                    changed = true;
                }
            }
        });
    }
    ui.label(
        RichText::new(tr("Picks apply right away."))
            .size(12.0)
            .color(t.text3),
    );
    changed
}

/// The page's main action: the primary fill, a line icon, then the text.
pub(super) fn primary_icon(
    ui: &mut Ui,
    t: &Theme,
    kind: Icon,
    text: &str,
    name: &str,
) -> egui::Response {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(14.0), t.on_primary);
    let w = 10.0 + 16.0 + 6.0 + g.size().x + 14.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w, CONTROL_H), Sense::click());
    name_it(&resp, name);
    let fill = if resp.hovered() {
        t.primary_hover
    } else {
        t.primary
    };
    ui.painter().rect_filled(r, 6.0, fill);
    let ic = pos2(r.left() + 10.0 + 8.0, r.center().y);
    line_icon(ui.painter(), kind, ic, t.on_primary);
    ui.painter().galley(
        pos2(ic.x + 8.0 + 6.0, r.center().y - g.size().y / 2.0),
        g,
        t.on_primary,
    );
    resp
}

/// A drop-down's button: framed like a secondary button, an icon and the
/// choice on the left, a chevron on the right.
#[allow(clippy::too_many_arguments)]
pub(super) fn select_button(
    ui: &mut Ui,
    t: &Theme,
    kind: Option<Icon>,
    icon_color: Color32,
    text: &str,
    text_color: Color32,
    size: egui::Vec2,
    name: &str,
) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    name_it(&resp, name);
    let hovered = resp.hovered();
    let p = ui.painter();
    p.rect_filled(r, 6.0, if hovered { t.hover } else { t.bg });
    p.rect_stroke(
        r,
        6.0,
        Stroke::new(1.0_f32, if hovered { hover_line(ui) } else { t.border }),
        egui::StrokeKind::Inside,
    );
    let mut x = r.left() + 10.0;
    if let Some(kind) = kind {
        line_icon(p, kind, pos2(x + 8.0, r.center().y), icon_color);
        x += 24.0;
    }
    let font = FontId::proportional(14.0);
    let shown = fit_text(p, text, &font, (r.right() - 28.0 - x).max(10.0));
    p.text(
        pos2(x, r.center().y),
        Align2::LEFT_CENTER,
        shown,
        font,
        text_color,
    );
    chevron(p, pos2(r.right() - 14.0, r.center().y), t.text2);
    resp
}

/// A small chevron pointing down, centred on `c`.
pub(super) fn chevron(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    let s = Stroke::new(1.5_f32, color);
    p.line_segment([c + vec2(-4.0, -2.0), c + vec2(0.0, 2.0)], s);
    p.line_segment([c + vec2(0.0, 2.0), c + vec2(4.0, -2.0)], s);
}

/// A full-width button with a dashed outline and a "+" before its text
/// (Add question).
pub(super) fn dashed_button(ui: &mut Ui, t: &Theme, text: &str, name: &str) -> egui::Response {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
    name_it(&resp, name);
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(r, 10.0, t.hover);
    } else {
        p.rect_filled(r, 10.0, t.card);
    }
    let r2 = r.shrink(0.5);
    let pts = [
        r2.left_top(),
        r2.right_top(),
        r2.right_bottom(),
        r2.left_bottom(),
        r2.left_top(),
    ];
    p.extend(egui::Shape::dashed_line(
        &pts,
        Stroke::new(1.0_f32, hover_line(ui)),
        5.0,
        4.0,
    ));
    let g = p.layout_no_wrap(text.to_string(), FontId::proportional(14.0), t.text2);
    let content = 16.0 + 6.0 + g.size().x;
    let left = r.center().x - content / 2.0;
    line_icon(p, Icon::Plus, pos2(left + 8.0, r.center().y), t.text2);
    p.galley(
        pos2(left + 22.0, r.center().y - g.size().y / 2.0),
        g,
        t.text2,
    );
    resp
}

/// A small rounded label ("New", "Just made") starting at `left_center`;
/// returns its width.
pub(super) fn tag_pill(
    p: &egui::Painter,
    left_center: egui::Pos2,
    text: &str,
    fg: Color32,
    bg: Color32,
) -> f32 {
    let g = p.layout_no_wrap(text.to_string(), FontId::proportional(11.0), fg);
    let r = Rect::from_min_size(
        pos2(left_center.x, left_center.y - 9.0),
        vec2(g.size().x + 14.0, 18.0),
    );
    p.rect_filled(r, 9.0, bg);
    p.galley(r.center() - g.size() / 2.0, g, fg);
    r.width()
}

/// A section's small heading in a side panel ("List for new tasks").
pub(super) fn panel_heading(ui: &mut Ui, t: &Theme, text: &str) {
    ui.label(RichText::new(text).size(12.0).strong().color(t.text2));
}

/// "Last edited" as the table shows it: how long ago within a day, the date
/// after that, a dash for forms from before this was kept.
pub(super) fn edited_text(ts: i64) -> String {
    if ts <= 0 {
        "\u{2014}".into()
    } else if now_ts() - ts < 86_400 {
        fmt_ago(ts)
    } else {
        local(ts).map(|d| d.text("%b %-d")).unwrap_or_default()
    }
}
