// Line icons drawn with the painter: one size (16 pt), one stroke (1.5 px)
// and the colour the caller passes (t.text2 beside text), in place of emoji
// and symbol glyphs, which egui drew from a black-and-white emoji font at
// sizes and baselines of their own.

use super::*;
use chrono::Datelike;
use egui::{Pos2, Response};

mod painted;

pub(super) use painted::*;

/// A line icon (see `line_icon`). The last few are the smaller, thinner
/// ones the List, the Board and the shell draw beside their text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Icon {
    Home,
    Planner,
    Ai,
    Teams,
    More,
    Chat,
    Docs,
    Dashboards,
    Whiteboards,
    Forms,
    Goals,
    Timesheets,
    Clock,
    Upgrade,
    CheckPlus,
    Video,
    Mic,
    Phone,
    Robot,
    Edit,
    Share,
    Inbox,
    Reply,
    Person,
    PersonComment,
    Megaphone,
    Bolt,
    Heart,
    Dots,
    Settings,
    Apps,
    Templates,
    Fields,
    Tag,
    Building,
    Smile,
    Frown,
    Bell,
    BellOff,
    Palette,
    Sparkle,
    Keyboard,
    Download,
    Upload,
    Refresh,
    Help,
    Task,
    Send,
    List,
    Hash,
    Space,
    Sidebar,
    Briefcase,
    Alarm,
    Notebook,
    Pin,
    Calendar,
    Link,
    Play,
    Stop,
    Sun,
    Moon,
    Auto,
    Bus,
    Plane,
    Search,
    Lock,
    Filter,
    Columns,
    Table,
    Subtasks,
    Check,
    Flag,
    Status,
    Hourglass,
    Repeat,
    Attach,
    Eye,
    EyeOff,
    Bookmark,
    Pen,
    Sticky,
    Eraser,
    Connector,
    Move,
    Shapes,
    Chart,
    Trash,
    Logout,
    Plus,
    Undo,
    Redo,
    Fit,
    Login,
    Sort,
    Archive,
    Close,
    PopOut,
    ArrowUpLeft,
    Folder,
    At,
    Volume,
    Badge,
    Wifi,
    PersonPlus,
    External,
    Warning,
    ArrowRight,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChecklistSmall,
    CalendarSmall,
    PersonSmall,
    CommentSmall,
    FlagSmall,
}

impl Icon {
    /// Every icon (a test draws each).
    #[cfg(test)]
    pub(crate) const ALL: &'static [Icon] = &[
        Icon::Home,
        Icon::Planner,
        Icon::Ai,
        Icon::Teams,
        Icon::More,
        Icon::Chat,
        Icon::Docs,
        Icon::Dashboards,
        Icon::Whiteboards,
        Icon::Forms,
        Icon::Goals,
        Icon::Timesheets,
        Icon::Clock,
        Icon::Upgrade,
        Icon::CheckPlus,
        Icon::Video,
        Icon::Mic,
        Icon::Phone,
        Icon::Robot,
        Icon::Edit,
        Icon::Share,
        Icon::Inbox,
        Icon::Reply,
        Icon::Person,
        Icon::PersonComment,
        Icon::Megaphone,
        Icon::Bolt,
        Icon::Heart,
        Icon::Dots,
        Icon::Settings,
        Icon::Apps,
        Icon::Templates,
        Icon::Fields,
        Icon::Tag,
        Icon::Building,
        Icon::Smile,
        Icon::Frown,
        Icon::Bell,
        Icon::BellOff,
        Icon::Palette,
        Icon::Sparkle,
        Icon::Keyboard,
        Icon::Download,
        Icon::Upload,
        Icon::Refresh,
        Icon::Help,
        Icon::Task,
        Icon::Send,
        Icon::List,
        Icon::Hash,
        Icon::Space,
        Icon::Sidebar,
        Icon::Briefcase,
        Icon::Alarm,
        Icon::Notebook,
        Icon::Pin,
        Icon::Calendar,
        Icon::Link,
        Icon::Play,
        Icon::Stop,
        Icon::Sun,
        Icon::Moon,
        Icon::Auto,
        Icon::Bus,
        Icon::Plane,
        Icon::Search,
        Icon::Lock,
        Icon::Filter,
        Icon::Columns,
        Icon::Table,
        Icon::Subtasks,
        Icon::Check,
        Icon::Flag,
        Icon::Status,
        Icon::Hourglass,
        Icon::Repeat,
        Icon::Attach,
        Icon::Eye,
        Icon::EyeOff,
        Icon::Bookmark,
        Icon::Pen,
        Icon::Sticky,
        Icon::Eraser,
        Icon::Connector,
        Icon::Move,
        Icon::Shapes,
        Icon::Chart,
        Icon::Trash,
        Icon::Logout,
        Icon::Plus,
        Icon::Undo,
        Icon::Redo,
        Icon::Fit,
        Icon::Login,
        Icon::Sort,
        Icon::Archive,
        Icon::Close,
        Icon::PopOut,
        Icon::ArrowUpLeft,
        Icon::Folder,
        Icon::At,
        Icon::Volume,
        Icon::Badge,
        Icon::Wifi,
        Icon::PersonPlus,
        Icon::External,
        Icon::Warning,
        Icon::ArrowRight,
        Icon::ChevronLeft,
        Icon::ChevronRight,
        Icon::ChevronDown,
        Icon::ChecklistSmall,
        Icon::CalendarSmall,
        Icon::PersonSmall,
        Icon::CommentSmall,
        Icon::FlagSmall,
    ];

    /// The icon named `name` ("home", "check_plus"), for a name made at run
    /// time; None for a name no icon has.
    pub(crate) fn named(name: &str) -> Option<Icon> {
        Some(match name {
            "home" => Icon::Home,
            "planner" => Icon::Planner,
            "ai" => Icon::Ai,
            "teams" => Icon::Teams,
            "more" => Icon::More,
            "chat" => Icon::Chat,
            "docs" => Icon::Docs,
            "dashboards" => Icon::Dashboards,
            "whiteboards" => Icon::Whiteboards,
            "forms" => Icon::Forms,
            "goals" => Icon::Goals,
            "timesheets" => Icon::Timesheets,
            "clock" => Icon::Clock,
            "upgrade" => Icon::Upgrade,
            "check_plus" => Icon::CheckPlus,
            "video" => Icon::Video,
            "mic" => Icon::Mic,
            "phone" => Icon::Phone,
            "robot" => Icon::Robot,
            "edit" => Icon::Edit,
            "share" => Icon::Share,
            "inbox" => Icon::Inbox,
            "reply" => Icon::Reply,
            "person" => Icon::Person,
            "person_comment" => Icon::PersonComment,
            "megaphone" => Icon::Megaphone,
            "bolt" => Icon::Bolt,
            "heart" => Icon::Heart,
            "dots" => Icon::Dots,
            "settings" => Icon::Settings,
            "apps" => Icon::Apps,
            "templates" => Icon::Templates,
            "fields" => Icon::Fields,
            "tag" => Icon::Tag,
            "building" => Icon::Building,
            "smile" => Icon::Smile,
            "frown" => Icon::Frown,
            "bell" => Icon::Bell,
            "bell_off" => Icon::BellOff,
            "palette" => Icon::Palette,
            "sparkle" => Icon::Sparkle,
            "keyboard" => Icon::Keyboard,
            "download" => Icon::Download,
            "upload" => Icon::Upload,
            "refresh" => Icon::Refresh,
            "help" => Icon::Help,
            "task" => Icon::Task,
            "send" => Icon::Send,
            "list" => Icon::List,
            "hash" => Icon::Hash,
            "space" => Icon::Space,
            "sidebar" => Icon::Sidebar,
            "briefcase" => Icon::Briefcase,
            "alarm" => Icon::Alarm,
            "notebook" => Icon::Notebook,
            "pin" => Icon::Pin,
            "calendar" => Icon::Calendar,
            "link" => Icon::Link,
            "play" => Icon::Play,
            "stop" => Icon::Stop,
            "sun" => Icon::Sun,
            "moon" => Icon::Moon,
            "auto" => Icon::Auto,
            "bus" => Icon::Bus,
            "plane" => Icon::Plane,
            "search" => Icon::Search,
            "lock" => Icon::Lock,
            "filter" => Icon::Filter,
            "columns" => Icon::Columns,
            "table" => Icon::Table,
            "subtasks" => Icon::Subtasks,
            "check" => Icon::Check,
            "flag" => Icon::Flag,
            "status" => Icon::Status,
            "hourglass" => Icon::Hourglass,
            "repeat" => Icon::Repeat,
            "attach" => Icon::Attach,
            "eye" => Icon::Eye,
            "eye_off" => Icon::EyeOff,
            "bookmark" => Icon::Bookmark,
            "pen" => Icon::Pen,
            "sticky" => Icon::Sticky,
            "eraser" => Icon::Eraser,
            "connector" => Icon::Connector,
            "move" => Icon::Move,
            "shapes" => Icon::Shapes,
            "chart" => Icon::Chart,
            "trash" => Icon::Trash,
            "logout" => Icon::Logout,
            "plus" => Icon::Plus,
            "undo" => Icon::Undo,
            "redo" => Icon::Redo,
            "fit" => Icon::Fit,
            "login" => Icon::Login,
            "sort" => Icon::Sort,
            "archive" => Icon::Archive,
            "close" => Icon::Close,
            "pop_out" => Icon::PopOut,
            "arrow_up_left" => Icon::ArrowUpLeft,
            "folder" => Icon::Folder,
            "at" => Icon::At,
            "volume" => Icon::Volume,
            "badge" => Icon::Badge,
            "wifi" => Icon::Wifi,
            "person_plus" => Icon::PersonPlus,
            "external" => Icon::External,
            "warning" => Icon::Warning,
            "arrow_right" => Icon::ArrowRight,
            "chevron_left" => Icon::ChevronLeft,
            "chevron_right" => Icon::ChevronRight,
            "chevron_down" => Icon::ChevronDown,
            "checklist_small" => Icon::ChecklistSmall,
            "calendar_small" => Icon::CalendarSmall,
            "person_small" => Icon::PersonSmall,
            "comment_small" => Icon::CommentSmall,
            "flag_small" => Icon::FlagSmall,
            _ => return None,
        })
    }

    /// Its name, as `named` takes it (and as a form saves its icon).
    pub(crate) fn name(self) -> &'static str {
        match self {
            Icon::Home => "home",
            Icon::Planner => "planner",
            Icon::Ai => "ai",
            Icon::Teams => "teams",
            Icon::More => "more",
            Icon::Chat => "chat",
            Icon::Docs => "docs",
            Icon::Dashboards => "dashboards",
            Icon::Whiteboards => "whiteboards",
            Icon::Forms => "forms",
            Icon::Goals => "goals",
            Icon::Timesheets => "timesheets",
            Icon::Clock => "clock",
            Icon::Upgrade => "upgrade",
            Icon::CheckPlus => "check_plus",
            Icon::Video => "video",
            Icon::Mic => "mic",
            Icon::Phone => "phone",
            Icon::Robot => "robot",
            Icon::Edit => "edit",
            Icon::Share => "share",
            Icon::Inbox => "inbox",
            Icon::Reply => "reply",
            Icon::Person => "person",
            Icon::PersonComment => "person_comment",
            Icon::Megaphone => "megaphone",
            Icon::Bolt => "bolt",
            Icon::Heart => "heart",
            Icon::Dots => "dots",
            Icon::Settings => "settings",
            Icon::Apps => "apps",
            Icon::Templates => "templates",
            Icon::Fields => "fields",
            Icon::Tag => "tag",
            Icon::Building => "building",
            Icon::Smile => "smile",
            Icon::Frown => "frown",
            Icon::Bell => "bell",
            Icon::BellOff => "bell_off",
            Icon::Palette => "palette",
            Icon::Sparkle => "sparkle",
            Icon::Keyboard => "keyboard",
            Icon::Download => "download",
            Icon::Upload => "upload",
            Icon::Refresh => "refresh",
            Icon::Help => "help",
            Icon::Task => "task",
            Icon::Send => "send",
            Icon::List => "list",
            Icon::Hash => "hash",
            Icon::Space => "space",
            Icon::Sidebar => "sidebar",
            Icon::Briefcase => "briefcase",
            Icon::Alarm => "alarm",
            Icon::Notebook => "notebook",
            Icon::Pin => "pin",
            Icon::Calendar => "calendar",
            Icon::Link => "link",
            Icon::Play => "play",
            Icon::Stop => "stop",
            Icon::Sun => "sun",
            Icon::Moon => "moon",
            Icon::Auto => "auto",
            Icon::Bus => "bus",
            Icon::Plane => "plane",
            Icon::Search => "search",
            Icon::Lock => "lock",
            Icon::Filter => "filter",
            Icon::Columns => "columns",
            Icon::Table => "table",
            Icon::Subtasks => "subtasks",
            Icon::Check => "check",
            Icon::Flag => "flag",
            Icon::Status => "status",
            Icon::Hourglass => "hourglass",
            Icon::Repeat => "repeat",
            Icon::Attach => "attach",
            Icon::Eye => "eye",
            Icon::EyeOff => "eye_off",
            Icon::Bookmark => "bookmark",
            Icon::Pen => "pen",
            Icon::Sticky => "sticky",
            Icon::Eraser => "eraser",
            Icon::Connector => "connector",
            Icon::Move => "move",
            Icon::Shapes => "shapes",
            Icon::Chart => "chart",
            Icon::Trash => "trash",
            Icon::Logout => "logout",
            Icon::Plus => "plus",
            Icon::Undo => "undo",
            Icon::Redo => "redo",
            Icon::Fit => "fit",
            Icon::Login => "login",
            Icon::Sort => "sort",
            Icon::Archive => "archive",
            Icon::Close => "close",
            Icon::PopOut => "pop_out",
            Icon::ArrowUpLeft => "arrow_up_left",
            Icon::Folder => "folder",
            Icon::At => "at",
            Icon::Volume => "volume",
            Icon::Badge => "badge",
            Icon::Wifi => "wifi",
            Icon::PersonPlus => "person_plus",
            Icon::External => "external",
            Icon::Warning => "warning",
            Icon::ArrowRight => "arrow_right",
            Icon::ChevronLeft => "chevron_left",
            Icon::ChevronRight => "chevron_right",
            Icon::ChevronDown => "chevron_down",
            Icon::ChecklistSmall => "checklist_small",
            Icon::CalendarSmall => "calendar_small",
            Icon::PersonSmall => "person_small",
            Icon::CommentSmall => "comment_small",
            Icon::FlagSmall => "flag_small",
        }
    }
}

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
pub(crate) fn line_icon_scaled(p: &egui::Painter, kind: Icon, c: Pos2, color: Color32, scale: f32) {
    let layer = p.layer_id();
    let start = p.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    line_icon(p, kind, c, color);
    let end = p.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let tf = egui::emath::TSTransform::new(c.to_vec2() * (1.0 - scale), scale);
    p.ctx()
        .graphics_mut(|g| g.entry(layer).transform_range(start, end, tf));
}

/// A line icon before a label, in one row (a field's name in the task
/// window, "Drop on a day").
pub(super) fn icon_label(
    ui: &mut Ui,
    kind: Icon,
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
    kind: Icon,
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
    let back = icon_button(ui, t, Icon::ChevronLeft, "", "Previous period", true)
        .on_hover_text(tr("Previous"))
        .clicked();
    let fwd = icon_button(ui, t, Icon::ChevronRight, "", "Next period", true)
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
    kind: Icon,
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
    kind: Icon,
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

/// Draws a thin outline icon in the style of the original (1.5 px strokes)
/// in a 16 pt square around `c` (the `*Small` ones are 12 pt, 1.2 px).
pub(crate) fn line_icon(p: &egui::Painter, kind: Icon, c: Pos2, color: Color32) {
    let s = Stroke::new(1.5_f32, color);
    let line = |a: egui::Vec2, b: egui::Vec2| p.line_segment([c + a, c + b], s);
    match kind {
        Icon::Home => {
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
        Icon::Planner => {
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
        Icon::Ai => {
            for k in 0..4 {
                let a = std::f32::consts::FRAC_PI_2 * k as f32 + std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                p.circle_stroke(c + d * 3.6, 3.6, s);
            }
            p.circle_filled(c, 1.6, color);
        }
        Icon::Teams => {
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
        Icon::More => {
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
        Icon::Chat => {
            let r = Rect::from_center_size(c + vec2(0.0, -1.0), vec2(15.0, 11.0));
            p.rect_stroke(r, 3.5, s, egui::StrokeKind::Middle);
            line(vec2(-3.0, 4.5), vec2(-5.0, 8.0));
            line(vec2(-5.0, 8.0), vec2(0.0, 4.5));
        }
        Icon::Docs => {
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
        Icon::Dashboards => {
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
        Icon::Whiteboards => {
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
        Icon::Forms => {
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
        Icon::Goals => {
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
        Icon::Timesheets | Icon::Clock => {
            p.circle_stroke(c + vec2(0.0, 1.0), 6.5, s);
            line(vec2(0.0, 1.0), vec2(0.0, -2.5));
            line(vec2(0.0, 1.0), vec2(2.5, 2.5));
            line(vec2(-2.0, -7.5), vec2(2.0, -7.5));
        }
        Icon::Upgrade => {
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
        Icon::CheckPlus => {
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
        Icon::Video => {
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
        Icon::Mic => {
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
        Icon::Phone => {
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
        Icon::Robot => {
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
        Icon::Edit => {
            p.rect_stroke(
                Rect::from_center_size(c + vec2(-1.0, 1.0), vec2(12.0, 12.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(0.0, 0.0), vec2(6.5, -6.5));
        }
        Icon::Share => {
            p.circle_stroke(c + vec2(-1.5, -3.0), 3.0, s);
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                arc.push(c + vec2(-1.5, 7.0) + vec2(a.cos() * 6.0, a.sin() * 5.0));
            }
            p.add(egui::Shape::line(arc, s));
            p.circle_stroke(c + vec2(5.0, -2.0), 2.2, s);
        }
        Icon::Inbox => {
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
        Icon::Reply => {
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
        Icon::Person => {
            p.circle_stroke(c + vec2(0.0, -3.5), 3.2, s);
            let mut arc = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                arc.push(c + vec2(0.0, 7.0) + vec2(a.cos() * 6.0, a.sin() * 5.0));
            }
            p.add(egui::Shape::line(arc, s));
        }
        Icon::PersonComment => {
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
        Icon::Megaphone => {
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
        Icon::Bolt => {
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
        Icon::Heart => {
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
        Icon::Dots => {
            for k in -1..=1 {
                p.circle_filled(c + vec2(5.0 * k as f32, 0.0), 1.4, color);
            }
        }
        // ------------------------------------------------ menus and toolbars
        Icon::Settings => {
            p.circle_stroke(c, 4.8, s);
            p.circle_stroke(c, 1.9, s);
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 4.8, c + d * 7.3], Stroke::new(2.2_f32, color));
            }
        }
        Icon::Apps => {
            for (x, y) in [(-3.8, -3.8), (3.8, -3.8), (-3.8, 3.8), (3.8, 3.8)] {
                p.rect_stroke(
                    Rect::from_center_size(c + vec2(x, y), vec2(5.5, 5.5)),
                    1.5,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::Templates => {
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
        Icon::Fields => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(15.0, 10.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-4.0, -2.0), vec2(-4.0, 2.0));
            line(vec2(-1.0, 0.0), vec2(4.0, 0.0));
        }
        Icon::Tag => {
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
        Icon::Building => {
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
        Icon::Smile | Icon::Frown => {
            p.circle_stroke(c, 7.0, s);
            p.circle_filled(c + vec2(-2.5, -2.0), 1.0, color);
            p.circle_filled(c + vec2(2.5, -2.0), 1.0, color);
            let up = kind == Icon::Smile;
            let mut arc = vec![];
            for i in 0..=8 {
                let a = std::f32::consts::PI * (0.2 + 0.6 * i as f32 / 8.0);
                let (cy, dir) = if up { (0.5, 1.0) } else { (6.0, -1.0) };
                arc.push(c + vec2(a.cos() * 3.5, cy + dir * a.sin() * 3.0 - dir * 0.0));
            }
            p.add(egui::Shape::line(arc, s));
        }
        Icon::Bell | Icon::BellOff => {
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
            if kind == Icon::BellOff {
                line(vec2(-7.0, -7.0), vec2(7.0, 7.0));
            }
        }
        Icon::Palette => {
            p.circle_stroke(c, 7.0, s);
            for (x, y) in [(-3.0, -2.5), (1.0, -4.0), (3.8, -0.5)] {
                p.circle_filled(c + vec2(x, y), 1.2, color);
            }
            p.circle_stroke(c + vec2(-1.5, 3.0), 1.6, Stroke::new(1.2_f32, color));
        }
        Icon::Sparkle => {
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
        Icon::Keyboard => {
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
        Icon::Download | Icon::Upload => {
            let (a, b) = if kind == Icon::Download {
                (vec2(0.0, -7.0), vec2(0.0, 2.5))
            } else {
                (vec2(0.0, 2.5), vec2(0.0, -7.0))
            };
            line(a, b);
            line(
                b,
                b + vec2(-3.5, if kind == Icon::Download { -3.5 } else { 3.5 }),
            );
            line(
                b,
                b + vec2(3.5, if kind == Icon::Download { -3.5 } else { 3.5 }),
            );
            line(vec2(-6.5, 4.0), vec2(-6.5, 7.0));
            line(vec2(-6.5, 7.0), vec2(6.5, 7.0));
            line(vec2(6.5, 7.0), vec2(6.5, 4.0));
        }
        Icon::Refresh => {
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
        Icon::Help => {
            p.circle_stroke(c, 7.0, s);
            p.text(
                c + vec2(0.2, 0.3),
                Align2::CENTER_CENTER,
                "?",
                FontId::proportional(10.5),
                color,
            );
        }
        Icon::Task => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(14.0, 14.0)),
                3.0,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-3.5, 0.0), vec2(-1.0, 2.6));
            line(vec2(-1.0, 2.6), vec2(3.8, -2.4));
        }
        Icon::Send => {
            let pts = vec![
                c + vec2(-7.0, -6.0),
                c + vec2(7.5, 0.0),
                c + vec2(-7.0, 6.0),
                c + vec2(-4.5, 0.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-4.5, 0.0), vec2(1.0, 0.0));
        }
        Icon::List => {
            for y in [-4.5, 0.0, 4.5] {
                p.circle_filled(c + vec2(-5.5, y), 1.1, color);
                line(vec2(-2.5, y), vec2(7.0, y));
            }
        }
        Icon::Hash => {
            line(vec2(-2.0, -7.0), vec2(-3.5, 7.0));
            line(vec2(3.5, -7.0), vec2(2.0, 7.0));
            line(vec2(-6.5, -2.5), vec2(7.0, -2.5));
            line(vec2(-7.0, 2.5), vec2(6.5, 2.5));
        }
        Icon::Space => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(14.0, 14.0)),
                4.0,
                s,
                egui::StrokeKind::Middle,
            );
            p.circle_stroke(c, 2.5, s);
        }
        Icon::Sidebar => {
            p.rect_stroke(
                Rect::from_center_size(c, vec2(16.0, 13.0)),
                2.5,
                s,
                egui::StrokeKind::Middle,
            );
            line(vec2(-2.5, -6.5), vec2(-2.5, 6.5));
        }
        Icon::Briefcase => {
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
        Icon::Alarm => {
            p.circle_stroke(c + vec2(0.0, 1.0), 6.0, s);
            line(vec2(0.0, 1.0), vec2(0.0, -2.0));
            line(vec2(0.0, 1.0), vec2(2.2, 2.4));
            line(vec2(-7.0, -4.5), vec2(-4.5, -7.0));
            line(vec2(7.0, -4.5), vec2(4.5, -7.0));
        }
        Icon::Notebook => {
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
        Icon::Pin => {
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
        Icon::Calendar => {
            let r = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(15.0, 14.0));
            p.rect_stroke(r, 3.0, s, egui::StrokeKind::Middle);
            line(vec2(-7.5, -3.0), vec2(7.5, -3.0));
            line(vec2(-3.5, -8.5), vec2(-3.5, -5.0));
            line(vec2(3.5, -8.5), vec2(3.5, -5.0));
        }
        Icon::Link => {
            for x in [-3.2, 3.2] {
                p.rect_stroke(
                    Rect::from_center_size(c + vec2(x, 0.0), vec2(9.0, 6.5)),
                    3.25,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::Play => {
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
        Icon::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(10.0, 10.0)), 2.0, color);
        }
        Icon::Sun => {
            p.circle_stroke(c, 3.5, s);
            for i in 0..8 {
                let a = std::f32::consts::TAU * i as f32 / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment([c + d * 5.8, c + d * 7.8], s);
            }
        }
        Icon::Moon => {
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
        Icon::Auto => {
            p.circle_stroke(c, 7.0, s);
            let mut pts = vec![c + vec2(0.0, -7.0)];
            for i in 0..=12 {
                let a = std::f32::consts::PI * (-0.5 + i as f32 / 12.0);
                pts.push(c + vec2(a.cos(), a.sin()) * 7.0);
            }
            p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        }
        Icon::Bus => {
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
        Icon::Plane => {
            let pts = vec![
                c + vec2(7.5, -7.5),
                c + vec2(-7.0, -1.5),
                c + vec2(-1.0, 1.0),
                c + vec2(1.5, 7.0),
            ];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(-1.0, 1.0), vec2(7.5, -7.5));
        }
        Icon::Search => {
            p.circle_stroke(c + vec2(-1.5, -1.5), 5.0, s);
            line(vec2(2.2, 2.2), vec2(6.5, 6.5));
        }
        Icon::Lock => {
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
        Icon::Filter => {
            line(vec2(-7.0, -4.5), vec2(7.0, -4.5));
            line(vec2(-4.5, 0.0), vec2(4.5, 0.0));
            line(vec2(-2.0, 4.5), vec2(2.0, 4.5));
        }
        Icon::Columns | Icon::Table => {
            let r = Rect::from_center_size(c, vec2(15.0, 13.0));
            p.rect_stroke(r, 2.5, s, egui::StrokeKind::Middle);
            line(vec2(-2.5, -6.5), vec2(-2.5, 6.5));
            line(vec2(2.5, -6.5), vec2(2.5, 6.5));
            if kind == Icon::Table {
                line(vec2(-7.5, -2.0), vec2(7.5, -2.0));
                line(vec2(-7.5, 2.5), vec2(7.5, 2.5));
            }
        }
        Icon::Subtasks => {
            line(vec2(-5.0, -7.0), vec2(-5.0, 4.5));
            line(vec2(-5.0, -2.0), vec2(1.0, -2.0));
            line(vec2(-5.0, 4.5), vec2(1.0, 4.5));
            p.circle_stroke(c + vec2(4.0, -2.0), 2.3, s);
            p.circle_stroke(c + vec2(4.0, 4.5), 2.3, s);
        }
        Icon::Check => {
            line(vec2(-6.0, 0.5), vec2(-2.0, 4.5));
            line(vec2(-2.0, 4.5), vec2(6.5, -4.5));
        }
        Icon::Flag => {
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
        Icon::Status => {
            p.circle_stroke(c, 6.5, s);
            p.circle_filled(c, 2.4, color);
        }
        Icon::Hourglass => {
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
        Icon::Repeat => {
            line(vec2(-6.5, 1.0), vec2(-6.5, -3.0));
            line(vec2(-6.5, -3.0), vec2(5.5, -3.0));
            line(vec2(5.5, -3.0), vec2(3.0, -5.5));
            line(vec2(6.5, -1.0), vec2(6.5, 3.0));
            line(vec2(6.5, 3.0), vec2(-5.5, 3.0));
            line(vec2(-5.5, 3.0), vec2(-3.0, 5.5));
        }
        Icon::Attach => {
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
        Icon::Eye => {
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
        Icon::EyeOff => {
            line_icon(p, Icon::Eye, c, color);
            line(vec2(-6.5, 6.5), vec2(6.5, -6.5));
        }
        Icon::Bookmark => {
            let pts = vec![
                c + vec2(-5.0, -7.5),
                c + vec2(5.0, -7.5),
                c + vec2(5.0, 7.5),
                c + vec2(0.0, 3.5),
                c + vec2(-5.0, 7.5),
            ];
            p.add(egui::Shape::closed_line(pts, s));
        }
        Icon::Pen => {
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
        Icon::Sticky => {
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
        Icon::Eraser => {
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
        Icon::Connector => {
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
        Icon::Move => {
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
        Icon::Shapes => {
            p.circle_stroke(c + vec2(-3.0, -3.0), 3.8, s);
            p.rect_stroke(
                Rect::from_center_size(c + vec2(3.0, 3.0), vec2(7.5, 7.5)),
                1.5,
                s,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Chart => {
            line(vec2(-7.0, 7.0), vec2(7.5, 7.0));
            line(vec2(-4.0, 7.0), vec2(-4.0, 1.0));
            line(vec2(0.5, 7.0), vec2(0.5, -6.0));
            line(vec2(5.0, 7.0), vec2(5.0, -1.5));
        }
        Icon::Trash => {
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
        Icon::Logout => {
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
        Icon::Plus => {
            line(vec2(0.0, -6.5), vec2(0.0, 6.5));
            line(vec2(-6.5, 0.0), vec2(6.5, 0.0));
        }
        Icon::Undo | Icon::Redo => {
            // a hook from the left arrow head round to the bottom; Redo's
            // is the same turned to face right
            let m = if kind == Icon::Redo { -1.0 } else { 1.0 };
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
        Icon::Fit => {
            // four corners: the whole board in view
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let k = c + vec2(6.5 * sx, 6.5 * sy);
                p.line_segment([k, k - vec2(4.0 * sx, 0.0)], s);
                p.line_segment([k, k - vec2(0.0, 4.0 * sy)], s);
            }
        }
        Icon::Login => {
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
        Icon::Sort => {
            line(vec2(-3.5, -7.0), vec2(-3.5, 7.0));
            line(vec2(-3.5, -7.0), vec2(-6.5, -4.0));
            line(vec2(-3.5, -7.0), vec2(-0.5, -4.0));
            line(vec2(3.5, 7.0), vec2(3.5, -7.0));
            line(vec2(3.5, 7.0), vec2(0.5, 4.0));
            line(vec2(3.5, 7.0), vec2(6.5, 4.0));
        }
        Icon::Archive => {
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
        Icon::Close => {
            line(vec2(-5.0, -5.0), vec2(5.0, 5.0));
            line(vec2(5.0, -5.0), vec2(-5.0, 5.0));
        }
        Icon::PopOut => {
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
        Icon::ArrowUpLeft => {
            line(vec2(5.0, 6.0), vec2(5.0, -1.0));
            line(vec2(5.0, -1.0), vec2(-5.0, -1.0));
            line(vec2(-5.0, -1.0), vec2(-1.5, -4.5));
            line(vec2(-5.0, -1.0), vec2(-1.5, 2.5));
        }
        Icon::Folder => {
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
        Icon::At => {
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
        Icon::Volume => {
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
        Icon::Badge => {
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
        Icon::Wifi => {
            let (a, b) = (-130_f32.to_radians(), -50_f32.to_radians());
            let o = c + vec2(0.0, 6.5);
            for r in [4.0, 8.0, 12.0] {
                p.add(egui::Shape::line(arc_points(o, r, a, b), s));
            }
            p.circle_filled(o, 1.1, color);
        }
        Icon::PersonPlus => {
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
        Icon::External => {
            line(vec2(-4.5, 4.5), vec2(5.0, -5.0));
            line(vec2(-0.5, -5.0), vec2(5.0, -5.0));
            line(vec2(5.0, -5.0), vec2(5.0, 0.5));
        }
        Icon::Warning => {
            let pts = vec![c + vec2(0.0, -7.0), c + vec2(7.5, 6.5), c + vec2(-7.5, 6.5)];
            p.add(egui::Shape::closed_line(pts, s));
            line(vec2(0.0, -2.5), vec2(0.0, 2.0));
            p.circle_filled(c + vec2(0.0, 4.3), 0.9, color);
        }
        Icon::ArrowRight => {
            line(vec2(-6.5, 0.0), vec2(6.5, 0.0));
            line(vec2(6.5, 0.0), vec2(2.5, -4.0));
            line(vec2(6.5, 0.0), vec2(2.5, 4.0));
        }
        Icon::ChevronLeft => {
            line(vec2(2.5, -5.0), vec2(-2.5, 0.0));
            line(vec2(-2.5, 0.0), vec2(2.5, 5.0));
        }
        Icon::ChevronRight => {
            line(vec2(-2.5, -5.0), vec2(2.5, 0.0));
            line(vec2(2.5, 0.0), vec2(-2.5, 5.0));
        }
        Icon::ChevronDown => {
            line(vec2(-5.0, -2.5), vec2(0.0, 2.5));
            line(vec2(0.0, 2.5), vec2(5.0, -2.5));
        }
        Icon::ChecklistSmall => {
            let s = Stroke::new(1.3_f32, color);
            p.line_segment([c + vec2(-6.0, -3.5), c + vec2(-4.5, -2.0)], s);
            p.line_segment([c + vec2(-4.5, -2.0), c + vec2(-2.0, -5.0)], s);
            p.line_segment([c + vec2(0.0, -3.5), c + vec2(6.0, -3.5)], s);
            p.line_segment([c + vec2(-6.0, 3.0), c + vec2(6.0, 3.0)], s);
        }
        Icon::CalendarSmall => {
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
        Icon::PersonSmall => {
            p.circle_stroke(c + vec2(0.0, -3.0), 3.0, Stroke::new(1.2_f32, color));
            let mut pts = vec![];
            for i in 0..=10 {
                let a = std::f32::consts::PI * (1.0 + i as f32 / 10.0);
                pts.push(c + vec2(0.0, 6.5) + vec2(a.cos() * 5.5, a.sin() * 4.0));
            }
            p.add(egui::Shape::line(pts, Stroke::new(1.2_f32, color)));
        }
        Icon::CommentSmall => {
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
        Icon::FlagSmall => {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every icon draws something, and its name finds it again.
    #[test]
    fn every_icon_draws() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            for &k in Icon::ALL {
                let layer = p.layer_id();
                let before = ctx.graphics_mut(|g| g.entry(layer).next_idx());
                line_icon(&p, k, pos2(20.0, 20.0), Color32::GRAY);
                let after = ctx.graphics_mut(|g| g.entry(layer).next_idx());
                assert!(after != before, "{:?} draws nothing", k);
                assert_eq!(Icon::named(k.name()), Some(k));
            }
        });
        assert_eq!(Icon::named("no such icon"), None);
    }
}
