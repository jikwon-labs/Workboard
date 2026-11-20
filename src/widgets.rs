// Small painted widgets and the pickers used across views:
// status circle, priority flag, avatars, pills, and the status, priority,
// assignee and date pickers. Here: text fields, buttons, chips, checkboxes,
// switches, dialog frames and hover helpers. In widgets/: painted pieces
// (paint.rs), dates and times as text (format.rs), popups and pickers
// (menu.rs) and the date picker (date_picker.rs), re-exported from here.

use crate::i18n::{tr, DateText};
use crate::model::*;
use crate::theme::{rgb, Theme};
use chrono::{Datelike, Duration, NaiveDate, NaiveTime};
use egui::{
    pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, Ui,
};

mod date_picker;
mod format;
mod menu;
mod paint;

pub use date_picker::*;
pub use format::*;
pub use menu::*;
pub use paint::*;

/// Names a custom-drawn control so screen readers (and the click tests) can find it.
pub fn name_it(resp: &Response, label: &str) {
    let l = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, l.clone()));
}

/// The height a one-line text field, a framed button and a drop-down share,
/// so they line up when they sit in one row. The theme's button padding
/// gives buttons and drop-downs this height with 14 px text.
pub const CONTROL_H: f32 = 28.0;

/// A one-line text field as tall as a button, with room around the text.
/// egui has no style setting for a text field's inner margin (every
/// `TextEdit` starts at 4,2), so fields are made here.
pub fn field(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .margin(egui::Margin::symmetric(8, 5))
        .min_size(vec2(0.0, CONTROL_H))
        .vertical_align(egui::Align::Center)
}

/// A framed text box of several lines, with the same side margin as `field`
/// so the hint does not sit in the corner.
pub fn field_multi(text: &mut dyn egui::TextBuffer) -> egui::TextEdit<'_> {
    egui::TextEdit::multiline(text).margin(egui::Margin::symmetric(8, 6))
}

/// Rounded primary button.
pub fn primary_button(ui: &mut Ui, t: &Theme, text: &str) -> Response {
    ui.add(Tint(primary_btn(t, text)))
}

/// The primary button's look, for `ui.add_enabled` or a wider minimum:
/// `ui.add_enabled(ok, Tint(primary_btn(t, text)))`.
pub fn primary_btn(t: &Theme, text: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text).color(t.on_primary).strong())
        .fill(t.primary)
        .corner_radius(6)
        .min_size(vec2(0.0, CONTROL_H))
}

/// A framed button beside a primary one (Cancel, Clear status, Copy link):
/// the same height and corners, the theme's outline instead of the fill.
pub fn secondary_button(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> Response {
    ui.add(
        egui::Button::new(text)
            .corner_radius(6)
            .min_size(vec2(0.0, CONTROL_H)),
    )
}

/// A small action button in a list row: room around the text, rounded like
/// the rest (egui's small_button has no vertical padding, so the text touched
/// the outline).
/// It is as tall as the other buttons (CONTROL_H), so it lines up with them
/// where a dialog puts it beside a primary button.
pub fn row_button(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding.x = 10.0;
        ui.add(
            egui::Button::new(text)
                .corner_radius(6)
                .min_size(vec2(0.0, CONTROL_H)),
        )
    })
    .inner
}

pub fn chip(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let font = FontId::proportional(11.0);
    let bg = color;
    // on a dark theme the full colour is too dark to read on its own tint
    let color = if ui.visuals().dark_mode {
        chip_text_on_dark(color)
    } else {
        color
    };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 18.0), Sense::click());
    let strength = if resp.hovered() { 0.28 } else { 0.16 };
    ui.painter()
        .rect_filled(rect, 4.0, bg.gamma_multiply(strength));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

/// Place a widget over an exact rectangle without moving the layout cursor
/// (unlike `Ui::put`, which pulls the following rows up to the rectangle).
pub fn put_over(ui: &mut egui::Ui, rect: egui::Rect, w: impl egui::Widget) -> Response {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
    ));
    // the widget fills the cell: no 28 pt minimum, and a framed button's
    // padding shrinks to fit a shorter cell instead of spilling out of it
    let pad = ((rect.height() - 17.0) / 2.0).clamp(0.0, ui.spacing().button_padding.y);
    child.spacing_mut().interact_size.y = 0.0;
    child.spacing_mut().button_padding.y = pad;
    child.add(w)
}

/// Gives a text field keyboard focus on the first frame it is shown, then
/// leaves focus alone so the person can click elsewhere or press Escape.
/// Returns true on the pass the field appeared (and got the focus).
pub fn focus_once(ui: &Ui, r: &Response) -> bool {
    let id = r.id.with("focus_once");
    let nr = ui.ctx().cumulative_pass_nr();
    let last = ui.ctx().data(|d| d.get_temp::<u64>(id));
    let first = last.map_or(true, |l| l + 2 < nr);
    if first {
        r.request_focus();
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, nr));
    first
}

/// The frame every dialog window uses (Create List, Create a Space, Create Task...).
pub fn dialog_frame(t: &Theme) -> egui::Frame {
    egui::Frame::new()
        .fill(t.card)
        .corner_radius(12)
        .stroke(Stroke::new(1.0_f32, t.border))
        .inner_margin(egui::Margin::same(20))
}

/// The grey panel on the left of a page (the sidebar, a hub's or a page's
/// list), rounded on its left corners, with `margin` inside.
pub fn side_panel_frame(t: &Theme, margin: egui::Margin) -> egui::Frame {
    egui::Frame::new()
        .fill(t.sidebar)
        .corner_radius(egui::CornerRadius {
            nw: 8,
            ne: 0,
            sw: 8,
            se: 0,
        })
        .inner_margin(margin)
}

/// A card on a page: a thin border, rounded by 12, with 16 inside.
pub fn card_frame(t: &Theme) -> egui::Frame {
    egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, t.border))
        .corner_radius(12)
        .inner_margin(egui::Margin::same(16))
}

/// Dialog heading: title on the left, round close button on the right.
/// Returns true when the close button was clicked.
pub fn dialog_header(ui: &mut Ui, t: &Theme, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).size(18.0).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
            name_it(&resp, "Close");
            ui.painter().circle_filled(
                r.center(),
                14.0,
                if resp.hovered() { t.border } else { t.hover },
            );
            let c = r.center();
            ui.painter().line_segment(
                [c + vec2(-4.5, -4.5), c + vec2(4.5, 4.5)],
                Stroke::new(1.5_f32, t.text2),
            );
            ui.painter().line_segment(
                [c + vec2(4.5, -4.5), c + vec2(-4.5, 4.5)],
                Stroke::new(1.5_f32, t.text2),
            );
            if resp.on_hover_text(tr("Close")).clicked() {
                close = true;
            }
        });
    });
    close
}

/// A small switch (on/off) with a label, named for tests and screen readers.
pub fn switch(ui: &mut Ui, t: &Theme, on: &mut bool, label: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(SWITCH_SIZE, Sense::click());
    name_it(&resp, label);
    if resp.clicked() {
        *on = !*on;
    }
    paint_switch(ui, t, &resp, r, *on);
    resp
}

/// The size `switch` draws at.
pub const SWITCH_SIZE: egui::Vec2 = vec2(34.0, 20.0);

/// Draws a `switch` in `r` (SWITCH_SIZE) for a control whose response is
/// `resp`: a settings row that turns on and off as a whole draws one.
pub fn paint_switch(ui: &Ui, t: &Theme, resp: &Response, r: Rect, on: bool) {
    // off: a grey that still stands out on a white card (the border grey
    // all but vanished there)
    let off = if t.dark {
        Color32::from_gray(80)
    } else {
        Color32::from_gray(212)
    };
    let bg = if on { t.primary } else { off };
    let bg = if resp.enabled() {
        bg
    } else {
        bg.gamma_multiply(0.5)
    };
    ui.painter().rect_filled(r, 10.0, bg);
    hover_bg(ui, resp, r, 10.0);
    let x = if on {
        r.right() - 10.0
    } else {
        r.left() + 10.0
    };
    let knob = if on { t.on_primary } else { Color32::WHITE };
    ui.painter().circle_filled(pos2(x, r.center().y), 7.0, knob);
}

/// Where `theme::apply` leaves the accent colour for widgets that are not
/// handed the theme (the check box).
pub const ACCENT_ID: &str = "__wb_accent";
/// The colour of a tick or dot drawn on the accent fill (`Theme::on_primary`).
pub const ON_ACCENT_ID: &str = "__wb_on_accent";

fn on_accent(ui: &Ui) -> Color32 {
    ui.ctx()
        .data(|d| d.get_temp(egui::Id::new(ON_ACCENT_ID)))
        .unwrap_or(Color32::WHITE)
}

/// A check box: a small square with slightly rounded corners, filled with
/// the accent colour and a white tick when on, an outline that stands out
/// from the page when off. egui's own check box takes the corner radius of
/// every widget (6), which turns a 14 px box into a circle.
pub struct Check<'a> {
    checked: &'a mut bool,
    text: egui::WidgetText,
}

impl<'a> Check<'a> {
    pub fn new(checked: &'a mut bool, text: impl Into<egui::WidgetText>) -> Self {
        Check {
            checked,
            text: text.into(),
        }
    }
}

/// `check(ui, &mut on, label)` in place of `ui.checkbox(&mut on, label)`.
pub fn check(ui: &mut Ui, checked: &mut bool, text: impl Into<egui::WidgetText>) -> Response {
    ui.add(Check::new(checked, text))
}

impl egui::Widget for Check<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Check { checked, text } = self;
        const BOX: f32 = 15.0;
        let gap = ui.spacing().icon_spacing + 2.0;
        let galley = if text.is_empty() {
            None
        } else {
            let wrap = ui.available_width() - BOX - gap;
            Some(text.into_galley(ui, None, wrap, egui::TextStyle::Button))
        };
        // a fixed row height, not interact_size (28): a list of check boxes
        // stays as compact as lines of text
        let mut size = vec2(BOX, 20.0);
        if let Some(g) = &galley {
            size.x += gap + g.size().x;
            size.y = size.y.max(g.size().y);
        }
        let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
        if resp.clicked() {
            *checked = !*checked;
            resp.mark_changed();
        }
        let on = *checked;
        let label = galley
            .as_ref()
            .map_or(String::new(), |g| g.text().to_string());
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, &label)
        });
        if ui.is_rect_visible(rect) {
            let dark = ui.visuals().dark_mode;
            let accent: Color32 = ui
                .ctx()
                .data(|d| d.get_temp(egui::Id::new(ACCENT_ID)))
                .unwrap_or(ui.visuals().selection.bg_fill);
            let b = Rect::from_center_size(
                pos2(rect.left() + BOX / 2.0, rect.center().y),
                vec2(BOX, BOX),
            );
            let enabled = resp.enabled();
            let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.5) };
            if on {
                let fill = if resp.hovered() {
                    accent.gamma_multiply(0.85).to_opaque()
                } else {
                    accent
                };
                ui.painter().rect_filled(b, 3.5, fade(fill));
                let c = b.center();
                ui.painter().add(egui::Shape::line(
                    vec![
                        c + vec2(-3.6, 0.2),
                        c + vec2(-1.0, 2.8),
                        c + vec2(3.8, -2.6),
                    ],
                    Stroke::new(1.8_f32, fade(on_accent(ui))),
                ));
            } else {
                let line = match (dark, resp.hovered()) {
                    (true, false) => Color32::from_gray(110),
                    (true, true) => Color32::from_gray(150),
                    (false, false) => Color32::from_gray(165),
                    (false, true) => Color32::from_gray(120),
                };
                ui.painter().rect(
                    b,
                    3.5,
                    ui.visuals().extreme_bg_color,
                    Stroke::new(1.2_f32, fade(line)),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(g) = galley {
                let color = ui.style().interact(&resp).text_color();
                let pos = pos2(b.right() + gap, rect.center().y - g.size().y / 2.0);
                ui.painter().galley(pos, g, color);
            }
        }
        resp
    }
}

/// A radio button drawn like `Check`: an outlined circle that stands out from
/// the page when off, the accent colour with a white dot when on (egui's
/// own was the page colour with a border-grey ring and disappeared in the
/// dark theme). `radio(ui, on, label)` in place of `ui.radio(on, label)`.
pub fn radio(ui: &mut Ui, on: bool, text: impl Into<egui::WidgetText>) -> Response {
    const D: f32 = 15.0;
    let text: egui::WidgetText = text.into();
    let gap = ui.spacing().icon_spacing + 2.0;
    let galley = (!text.is_empty()).then(|| {
        let wrap = ui.available_width() - D - gap;
        text.into_galley(ui, None, wrap, egui::TextStyle::Button)
    });
    let mut size = vec2(D, 20.0);
    if let Some(g) = &galley {
        size.x += gap + g.size().x;
        size.y = size.y.max(g.size().y);
    }
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let label = galley
        .as_ref()
        .map_or(String::new(), |g| g.text().to_string());
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, ui.is_enabled(), on, &label)
    });
    if ui.is_rect_visible(rect) {
        let dark = ui.visuals().dark_mode;
        let c = pos2(rect.left() + D / 2.0, rect.center().y);
        if on {
            let accent: Color32 = ui
                .ctx()
                .data(|d| d.get_temp(egui::Id::new(ACCENT_ID)))
                .unwrap_or(ui.visuals().selection.bg_fill);
            ui.painter().circle_filled(c, D / 2.0, accent);
            ui.painter().circle_filled(c, 2.8, on_accent(ui));
        } else {
            let line = match (dark, resp.hovered()) {
                (true, false) => Color32::from_gray(110),
                (true, true) => Color32::from_gray(150),
                (false, false) => Color32::from_gray(165),
                (false, true) => Color32::from_gray(120),
            };
            ui.painter().circle(
                c,
                D / 2.0 - 0.6,
                ui.visuals().extreme_bg_color,
                Stroke::new(1.2_f32, line),
            );
        }
        if let Some(g) = galley {
            let color = ui.style().interact(&resp).text_color();
            ui.painter().galley(
                pos2(c.x + D / 2.0 + gap, rect.center().y - g.size().y / 2.0),
                g,
                color,
            );
        }
    }
    resp
}

/// Names a text field by its hint so screen readers (and the click tests) can find it.
pub fn name_field(resp: &Response, label: &str) {
    let l = label.to_string();
    resp.widget_info(move || {
        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, l.clone())
    });
}

/// A full-width faint "+ Add ..." row (Add subtask, Create checklist).
pub fn add_line(ui: &mut Ui, t: &Theme, label: &str) -> Response {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
    name_it(&resp, label);
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    ui.painter().text(
        r.left_center() + vec2(10.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        t.text2,
    );
    resp
}

// ---------------------------------------------------------------- hover feedback

const NO_POINTER: &str = "no_pointer_ids";

/// Keeps the plain arrow over a click area that is not a control: the dim
/// backdrop behind a dialog, the media viewer's surround, a drawing canvas.
pub fn no_pointer(resp: &Response) {
    let id = resp.id;
    resp.ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<egui::Id>>(egui::Id::new(NO_POINTER))
            .push(id);
    });
}

/// Run once at the end of every frame: if the pointer rests on something that
/// can be clicked and no widget picked a cursor of its own (a text field's
/// I-beam, a resize arrow), show the pointing hand. egui's own buttons get it
/// from `Visuals::interact_cursor`; this covers the painted controls, toggles,
/// selectable labels and pickers.
pub fn pointer_over_clickables(ctx: &egui::Context) {
    let skip: Vec<egui::Id> = ctx.data_mut(|d| {
        std::mem::take(d.get_temp_mut_or_default::<Vec<egui::Id>>(egui::Id::new(NO_POINTER)))
    });
    if ctx.output(|o| o.cursor_icon) != egui::CursorIcon::Default || ctx.dragged_id().is_some() {
        return;
    }
    let hovered = ctx.interaction_snapshot(|s| s.hovered.clone());
    let clickable = hovered.into_iter().any(|id| {
        !skip.contains(&id)
            && ctx.read_response(id).is_some_and(|r| {
                // a popup or panel's own background takes clicks only to come to the front
                let area_bg = r.id == r.layer_id.id.with("move");
                r.enabled() && r.sense.senses_click() && !area_bg
            })
    });
    if clickable {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// The soft fill behind a frameless control under the pointer. Translucent so it
/// reads the same on the page, a card, the sidebar or a popup; about as strong
/// as the theme's `hover` grey on the page background.
pub fn hover_fill(ui: &Ui) -> Color32 {
    hover_fill_for(ui.visuals().dark_mode)
}

pub fn hover_fill_for(dark: bool) -> Color32 {
    if dark {
        Color32::from_white_alpha(12)
    } else {
        Color32::from_black_alpha(10)
    }
}

/// The outline drawn around a swatch or card under the pointer: a step above
/// the border colour, not the bright grey egui uses for framed buttons.
pub fn hover_line(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_gray(88)
    } else {
        Color32::from_gray(190)
    }
}

/// Fills a rounded rectangle into a slot reserved with `painter.add(Shape::Noop)`
/// before the control was drawn, so the fill sits behind its text.
pub fn paint_hover_slot(ui: &Ui, slot: egui::layers::ShapeIdx, rect: Rect, fill: Color32) {
    ui.painter().set(
        slot,
        egui::epaint::RectShape::filled(rect, egui::CornerRadius::same(6), fill),
    );
}

/// A button without a frame (toolbar icons, text actions, emoji in the reaction
/// bar). egui draws nothing on hover for these, so this adds a rounded fill a
/// little larger than the label; the layout is the same as `frame(false)`.
pub struct Ghost<'a>(pub egui::Button<'a>);

/// `ghost(egui::Button::new(..))` in place of `egui::Button::new(..).frame(false)`.
pub fn ghost(button: egui::Button<'_>) -> Ghost<'_> {
    Ghost(button)
}

impl egui::Widget for Ghost<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let slot = ui.painter().add(egui::Shape::Noop);
        let r = ui.add(self.0.frame(false));
        if r.enabled() && (r.hovered() || r.is_pointer_button_down_on()) {
            let fill = hover_fill(ui);
            let fill = if r.is_pointer_button_down_on() {
                fill.gamma_multiply(1.5)
            } else {
                fill
            };
            let pad = vec2(
                if r.rect.width() < 28.0 { 3.0 } else { 6.0 },
                if r.rect.height() < 22.0 { 2.0 } else { 0.0 },
            );
            // a button laid over an exact cell (put_over) keeps the fill inside it
            let rect = r.rect.expand2(pad).intersect(ui.max_rect().union(r.rect));
            paint_hover_slot(ui, slot, rect, fill);
        }
        r
    }
}

/// For a button with a fill of its own (primary, danger, a selected tab, a
/// reaction chip): egui keeps that fill on hover, so this lays a light tint over
/// it while the pointer is there.
pub struct Tint<W>(pub W);

impl<W: egui::Widget> egui::Widget for Tint<W> {
    fn ui(self, ui: &mut Ui) -> Response {
        let r = ui.add(self.0);
        if r.enabled() && r.hovered() {
            let tint = if r.is_pointer_button_down_on() {
                hover_fill(ui).gamma_multiply(1.5)
            } else {
                hover_fill(ui)
            };
            ui.painter().rect_filled(r.rect, 6.0, tint);
        }
        r
    }
}

/// For a painted control: draws the hover fill over `rect` while the pointer is
/// on it. Call it before painting the control's own shapes.
pub fn hover_bg(ui: &Ui, resp: &Response, rect: Rect, radius: f32) {
    if resp.enabled() && resp.hovered() {
        ui.painter().rect_filled(rect, radius, hover_fill(ui));
    }
}

/// For a painted swatch, avatar or icon tile: a thin ring around `rect` while the
/// pointer is on it (`radius` half the width makes it a circle).
pub fn hover_ring(ui: &Ui, resp: &Response, rect: Rect, radius: f32) {
    if resp.enabled() && resp.hovered() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.5_f32, hover_line(ui)),
            egui::StrokeKind::Outside,
        );
    }
}

/// A bordered card saying a section is empty: a yellow glyph over a line of
/// grey text.
pub fn empty_card(ui: &mut Ui, t: &Theme, glyph: &str, text: &str) {
    egui::Frame::new()
        .fill(t.bg)
        .stroke(Stroke::new(1.0_f32, t.border))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(10, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(glyph).size(16.0).color(STAR_YELLOW));
                ui.label(RichText::new(text).size(13.0).color(t.text2));
            });
        });
}

/// A list kept at its bottom (`stick_to_bottom`) catches up with new content
/// only on the frame after it changes size: a message that comes in first
/// shows lower down (cut off) and the list then jumps to it. While such a
/// list sits at its bottom and its content or height changes, the frame is
/// laid out again before it shows, already at the bottom.
pub fn redo_when_stuck_list_changes<R>(
    ctx: &egui::Context,
    out: &egui::scroll_area::ScrollAreaOutput<R>,
) {
    let key = out.id.with("stuck_list_size");
    let now = [
        out.content_size.y,
        out.inner_rect.height(),
        out.state.offset.y,
    ];
    let before: Option<[f32; 3]> = ctx.data_mut(|d| {
        let before = d.get_temp(key);
        d.insert_temp(key, now);
        before
    });
    let Some([content, height, offset]) = before else {
        return;
    };
    let was_at_bottom = offset >= content - height - 0.5;
    if was_at_bottom && (now[0] != content || now[1] != height) {
        ctx.request_discard("a list kept at its bottom changed size");
    }
}
