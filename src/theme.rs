// Colour tokens. Light values come from the original's design tokens
// (grey and purple scales); dark values are estimated. See MECHANICS.md.

use egui::Color32;

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub bg: Color32,
    pub sidebar: Color32,
    pub top: Color32,
    pub top_text: Color32,
    pub top_field: Color32,
    pub text: Color32,
    pub text2: Color32,
    pub text3: Color32,
    pub border: Color32,
    pub hover: Color32,
    pub selected_bg: Color32,
    pub selected_text: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub card: Color32,
    pub column_bg: Color32,
    pub danger: Color32,
    pub overlay: Color32,
    pub app_bg: Color32,
    pub rail_top: Color32,
    pub rail_bottom: Color32,
    pub tray: Color32,
    pub accent_bg: Color32,
    pub accent_text: Color32,
    pub row_selected: Color32,
}

/// Theme colours offered in Themes (the original's theme classes, e.g. cu-black).
pub const ACCENTS: [(&str, [u8; 3]); 10] = [
    ("black", [32, 32, 32]),
    ("purple", [96, 73, 231]),
    ("blue", [16, 144, 224]),
    ("pink", [233, 61, 130]),
    ("violet", [171, 74, 186]),
    ("indigo", [74, 84, 196]),
    ("orange", [247, 104, 8]),
    ("teal", [18, 165, 148]),
    ("bronze", [160, 117, 82]),
    ("mint", [46, 178, 128]),
];

pub fn theme_accent(dark: bool, accent: &str) -> Theme {
    let mut t = theme(dark);
    if let Some((_, c)) = ACCENTS.iter().find(|a| a.0 == accent) {
        if accent != "black" {
            let base = rgb(*c);
            t.primary = base;
            t.primary_hover = base.gamma_multiply(0.85).to_opaque();
            t.rail_top = base;
            t.rail_bottom = Color32::from_rgb(c[0] / 3, c[1] / 3, c[2] / 3);
        }
    }
    t
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn theme(dark: bool) -> Theme {
    if !dark {
        Theme {
            dark,
            bg: Color32::WHITE,
            sidebar: Color32::from_rgb(249, 249, 249),
            top: Color32::WHITE,
            top_text: Color32::from_rgb(32, 32, 32),
            top_field: Color32::WHITE,
            text: Color32::from_rgb(32, 32, 32),
            text2: Color32::from_rgb(100, 100, 100),
            text3: Color32::from_rgb(131, 131, 131),
            border: Color32::from_rgb(232, 232, 232),
            hover: Color32::from_rgb(240, 240, 240),
            selected_bg: Color32::from_rgb(240, 240, 240),
            selected_text: Color32::from_rgb(32, 32, 32),
            primary: Color32::from_rgb(32, 32, 32),
            primary_hover: Color32::from_rgb(64, 64, 64),
            card: Color32::WHITE,
            column_bg: Color32::from_rgb(249, 249, 249),
            danger: Color32::from_rgb(0xd3, 0x3d, 0x44),
            overlay: Color32::from_rgba_unmultiplied(0, 0, 0, 153),
            app_bg: Color32::WHITE,
            rail_top: Color32::BLACK,
            rail_bottom: Color32::BLACK,
            tray: Color32::from_rgb(249, 249, 249),
            accent_bg: Color32::from_rgb(240, 241, 254),
            accent_text: Color32::from_rgb(90, 67, 214),
            row_selected: Color32::from_rgb(234, 234, 234),
        }
    } else {
        Theme {
            dark,
            bg: Color32::from_rgb(32, 32, 32),
            sidebar: Color32::from_rgb(25, 25, 25),
            top: Color32::from_rgb(20, 16, 50),
            top_text: Color32::from_rgb(230, 230, 240),
            top_field: Color32::from_rgba_unmultiplied(255, 255, 255, 26),
            text: Color32::from_rgb(232, 232, 232),
            text2: Color32::from_rgb(180, 180, 180),
            text3: Color32::from_rgb(141, 141, 141),
            border: Color32::from_rgb(56, 56, 56),
            hover: Color32::from_rgb(46, 46, 46),
            selected_bg: Color32::from_rgb(50, 50, 50),
            selected_text: Color32::from_rgb(240, 240, 240),
            primary: Color32::from_rgb(80, 80, 80),
            primary_hover: Color32::from_rgb(100, 100, 100),
            card: Color32::from_rgb(40, 40, 40),
            column_bg: Color32::from_rgb(27, 27, 27),
            danger: Color32::from_rgb(0xe5, 0x48, 0x4d),
            overlay: Color32::from_rgba_unmultiplied(0, 0, 0, 170),
            app_bg: Color32::from_rgb(24, 24, 24),
            rail_top: Color32::from_rgb(10, 10, 10),
            rail_bottom: Color32::from_rgb(10, 10, 10),
            tray: Color32::from_rgb(28, 28, 28),
            accent_bg: Color32::from_rgb(45, 40, 80),
            accent_text: Color32::from_rgb(202, 203, 255),
            row_selected: Color32::from_rgb(50, 50, 50),
        }
    }
}

pub fn apply(ctx: &egui::Context, t: &Theme) {
    let mut v = if t.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = t.bg;
    v.window_fill = t.card;
    v.extreme_bg_color = if t.dark {
        Color32::from_rgb(28, 28, 28)
    } else {
        Color32::WHITE
    };
    v.faint_bg_color = t.sidebar;
    // text colour through the widget styles (not override_text_color, which
    // would also paint input placeholders as dark as typed text)
    v.override_text_color = None;
    v.widgets.noninteractive.fg_stroke.color = t.text;
    v.widgets.active.fg_stroke.color = t.text;
    v.widgets.open.fg_stroke.color = t.text;
    v.selection.bg_fill = if t.dark {
        t.primary
    } else {
        Color32::from_rgb(200, 215, 250)
    };
    v.selection.stroke = egui::Stroke::new(1.0, t.text);
    // links in the accent text colour, so they read as links (white or
    // near-black bold looked like any label)
    v.hyperlink_color = t.accent_text;
    v.window_stroke = egui::Stroke::new(1.0, t.border);
    v.window_corner_radius = egui::CornerRadius::same(12);
    v.menu_corner_radius = egui::CornerRadius::same(8);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, t.border);
    v.widgets.inactive.bg_fill = t.hover;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    // inputs and framed buttons get the original's 1 px border
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, t.border);
    v.widgets.inactive.bg_fill = t.bg;
    v.widgets.hovered.weak_bg_fill = t.hover;
    v.widgets.hovered.bg_fill = t.hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(187, 187, 187));
    v.widgets.active.weak_bg_fill = t.selected_bg;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(6);
    }
    v.widgets.inactive.fg_stroke.color = t.text;
    v.widgets.hovered.fg_stroke.color = t.text;
    // buttons show the pointing hand like a web app (widgets::pointer_over_clickables
    // does the same for painted controls)
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    ctx.set_visuals(v);
    let accent = t.primary;
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(crate::widgets::ACCENT_ID), accent));
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        // 5.5 above and below 14 px text (17 px tall) makes framed buttons
        // and drop-downs 28 px, the height of a primary button and a text
        // field (widgets::CONTROL_H), so a row of them lines up
        s.spacing.button_padding = egui::vec2(8.0, 5.5);
        // egui starts every horizontal row at this height and centres what
        // comes first in it, and a drop-down builds a row of its own: at
        // egui's 18 a label sat above the drop-down beside it and the
        // drop-down sat lower than a field next to it
        s.spacing.interact_size.y = crate::widgets::CONTROL_H;
        // drop-down lists show as many rows as before the rows grew to 28
        s.spacing.combo_height = 300.0;
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.5));
        s.text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
    });
}
