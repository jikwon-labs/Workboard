// The bars that float over a whiteboard: its name with the star and the
// board menu at the top left, zoom at the top right, the tools down the
// left, the chosen tool's colours and width along the bottom, and on a board
// just made a card that says so and offers ways to start.
//
// Before, all of it was one wrapped row of text buttons above a grey box,
// with Clear and Delete next to the zoom buttons.

use super::wb_shapes::{paint_kind_icon, paint_style_icon, SHAPE_KINDS, STYLE_TOGGLES};
use super::whiteboard::{ink, note_fill, WB_COLORS};
use super::*;
use crate::app::icons::{paint_minus, paint_plus};
use egui::{Pos2, Response};

/// Space between the bars and the edges of the page.
const EDGE: f32 = 16.0;
/// Height of the bars at the top.
const BAR_H: f32 = 44.0;
/// Height of the bar at the bottom.
const OPTIONS_H: f32 = 48.0;
/// A button in the top bars.
const BTN: f32 = 32.0;
/// A button in the tool bar.
const TOOL: f32 = 40.0;
/// The zoom (percent) button in the zoom bar, and the whole bar.
const PCT_W: f32 = 54.0;
const ZOOM_W: f32 = 6.0 + BTN + PCT_W + BTN + 9.0 + BTN + 6.0;
/// The tools, top to bottom: (`wb_tool`, icon, name).
const TOOLS: [(u8, &str, &str); 6] = [
    (2, "move", "Select"),
    (0, "pen", "Pen"),
    (3, "eraser", "Eraser"),
    (1, "sticky", "Sticky note"),
    (4, "shapes", "Shape"),
    (5, "connector", "Line"),
];
/// The note a black pen colour makes (black paper would hide the text).
const STICKY_FOR_BLACK: [u8; 3] = [0xf8, 0xae, 0x00];

/// The colour a new sticky note gets with `pen` chosen.
pub(crate) fn sticky_color(pen: [u8; 3]) -> [u8; 3] {
    if pen == WB_COLORS[0] {
        STICKY_FOR_BLACK
    } else {
        pen
    }
}

/// A floating bar's card. Presses on it stay off the board beneath: the
/// board's drag area covers the whole page, and this is laid over it later.
pub(super) fn card(ui: &Ui, t: &Theme, r: Rect, radius: f32, id: egui::Id) {
    ui.interact(r, id, Sense::click_and_drag());
    let p = ui.painter();
    let shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if t.dark { 70 } else { 22 }),
    };
    p.add(shadow.as_shape(r, radius));
    p.rect_filled(r, radius, t.bg);
    p.rect_stroke(
        r,
        radius,
        Stroke::new(1.0_f32, t.border),
        egui::StrokeKind::Inside,
    );
}

/// A button on a bar, its face painted by `paint` in the colour it gets;
/// `on` fills it with the accent (the tool in use).
#[allow(clippy::too_many_arguments)]
fn bar_button(
    ui: &Ui,
    t: &Theme,
    r: Rect,
    id: egui::Id,
    name: &str,
    tip: &str,
    on: bool,
    enabled: bool,
    paint: impl FnOnce(&egui::Painter, Pos2, Color32),
) -> Response {
    let resp = ui.interact(
        r,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    name_it(&resp, name);
    let p = ui.painter();
    if on {
        p.rect_filled(r, 8.0, t.primary);
    } else if enabled && resp.hovered() {
        p.rect_filled(r, 8.0, t.hover);
    }
    let fg = if on {
        t.on_primary
    } else if enabled {
        t.text2
    } else {
        t.text3
    };
    paint(p, r.center(), fg);
    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(tip)
}

/// A thin upright line between groups on a bar.
pub(super) fn bar_sep(p: &egui::Painter, t: &Theme, x: f32, cy: f32) {
    p.vline(x, cy - 11.0..=cy + 11.0, Stroke::new(1.0_f32, t.border));
}

/// A menu row in the danger colour (Delete).
fn danger_row(ui: &mut Ui, t: &Theme, name: &str, label: &str, icon: &str) -> Response {
    let w = ui.available_width().max(160.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    name_it(&resp, name);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 5.0, t.hover);
    }
    line_icon(
        ui.painter(),
        icon,
        rect.left_center() + vec2(14.0, 0.0),
        t.danger,
    );
    ui.painter().text(
        rect.left_center() + vec2(30.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        t.danger,
    );
    resp
}

impl App {
    /// Draws the bars over the board at `wi` (on the page `rect`, its 0,0 at
    /// `origin` before panning) and does what they are used for.
    pub(crate) fn wb_panels(&mut self, ui: &mut Ui, rect: Rect, origin: Pos2, wi: usize) {
        let t = self.t();
        let wid = self.data.extra.whiteboards[wi].id;
        let now = ui.input(|i| i.time);
        if let Some(n) = self.sweep.wb_new.as_mut().filter(|n| n.id == wid) {
            n.since.get_or_insert(now);
        }
        self.wb_name_bar(ui, &t, rect, wi);
        self.wb_zoom_bar(ui, &t, rect, origin, wid);
        let tools = self.wb_tool_bar(ui, &t, rect, wid);
        self.wb_tool_options(ui, &t, rect, tools);
        self.wb_start_card(ui, &t, rect, wi, now);
    }

    /// The "how to start" card of a board just made goes away.
    fn close_wb_start(&mut self, wid: u64) {
        if let Some(n) = self.sweep.wb_new.as_mut().filter(|n| n.id == wid) {
            n.start = false;
        }
    }

    /// Top left: the board's name (typed in place), its star, and the board
    /// menu with Reset view, Clear board and Delete whiteboard.
    fn wb_name_bar(&mut self, ui: &mut Ui, t: &Theme, rect: Rect, wi: usize) {
        let wid = self.data.extra.whiteboards[wi].id;
        let font = FontId::proportional(16.0);
        let shown = &self.data.extra.whiteboards[wi].name;
        let text_w = ui
            .painter()
            .layout_no_wrap(
                if shown.is_empty() {
                    tr("Untitled whiteboard").to_string()
                } else {
                    shown.clone()
                },
                font.clone(),
                t.text,
            )
            .size()
            .x;
        // as wide as the name, but in a narrow window clear of the zoom bar
        // (a long name ran under it)
        let rest = 6.0 + 4.0 + BTN + 2.0 + BTN + 6.0;
        let room = rect.width() - 2.0 * EDGE - ZOOM_W - 8.0 - rest;
        let name_w = (text_w + 24.0).clamp(120.0, room.min(300.0).max(120.0));
        let w = name_w + rest;
        let bar = Rect::from_min_size(rect.min + vec2(EDGE, EDGE), vec2(w, BAR_H));
        card(ui, t, bar, 12.0, ui.id().with(("wb_name_bar", wid)));
        self.sweep.wb_bars.push(bar);

        let nr = Rect::from_min_size(
            pos2(bar.left() + 6.0, bar.center().y - 15.0),
            vec2(name_w, 30.0),
        );
        let te_id = egui::Id::new(("wb_name", wid));
        let resp = ui.put(
            nr,
            egui::TextEdit::singleline(&mut self.data.extra.whiteboards[wi].name)
                .id(te_id)
                .font(font)
                .frame(false)
                .margin(egui::Margin::symmetric(8, 5))
                .desired_width(name_w)
                .hint_text(tr("Untitled whiteboard")),
        );
        name_it(&resp, "Whiteboard name");
        if resp.changed() {
            self.dirty = true;
        }
        let focused = resp.has_focus();
        if focused || resp.hovered() {
            ui.painter().rect_stroke(
                nr,
                7.0,
                Stroke::new(1.0_f32, if focused { t.primary } else { t.border }),
                egui::StrokeKind::Inside,
            );
        }
        // a board just made: its name is ready to type over (again after a
        // template renamed it, while the name is still being typed)
        if let Some(n) = self
            .sweep
            .wb_new
            .as_mut()
            .filter(|n| n.id == wid && (n.focus || (n.renamed && resp.has_focus())))
        {
            n.focus = false;
            n.renamed = false;
            resp.request_focus();
            let len = self.data.extra.whiteboards[wi].name.chars().count();
            let mut st = egui::TextEdit::load_state(ui.ctx(), te_id).unwrap_or_default();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(len),
            )));
            st.store(ui.ctx(), te_id);
        } else if let Some(n) = self.sweep.wb_new.as_mut().filter(|n| n.id == wid) {
            // renamed while the name wasn't being typed: nothing to fix
            n.renamed = false;
        }

        let key = format!("wb:{}", wid);
        let starred = self.data.extra.starred.contains(&key);
        let sr = Rect::from_min_size(
            pos2(nr.right() + 4.0, bar.center().y - BTN / 2.0),
            vec2(BTN, BTN),
        );
        let star = bar_button(
            ui,
            t,
            sr,
            ui.id().with(("wb_star", wid)),
            &format!("Star {}", key),
            if starred {
                tr("Remove from Favorites")
            } else {
                tr("Add to Favorites")
            },
            false,
            true,
            |p, c, fg| {
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    if starred { "\u{2605}" } else { "\u{2606}" },
                    FontId::proportional(17.0),
                    if starred { STAR_YELLOW } else { fg },
                );
            },
        );
        if star.clicked() {
            if starred {
                self.data.extra.starred.retain(|s| s != &key);
            } else {
                self.data.extra.starred.push(key);
            }
            self.dirty = true;
        }

        let mr = Rect::from_min_size(pos2(sr.right() + 2.0, sr.top()), vec2(BTN, BTN));
        let pop = ui.id().with(("wb_menu", wid));
        if ui.memory(|m| m.is_popup_open(pop)) {
            ui.painter().rect_filled(mr, 8.0, t.hover);
        }
        let menu = bar_button(
            ui,
            t,
            mr,
            ui.id().with(("wb_menu_button", wid)),
            "Board menu",
            tr("Board menu"),
            false,
            true,
            |p, c, fg| {
                line_icon(p, "dots", c, fg);
            },
        );
        let clear_key = format!("wb_clear:{}", wid);
        if menu.clicked() && self.sweep.confirm.as_deref() == Some(clear_key.as_str()) {
            self.sweep.confirm = None;
        }
        popup(ui, &menu, pop, 230.0, |ui| {
            self.wb_menu(ui, t, wid, &clear_key)
        });
    }

    /// The board menu: the view and the two things that take the board away,
    /// kept here instead of beside the zoom buttons. Clearing asks first.
    fn wb_menu(&mut self, ui: &mut Ui, t: &Theme, wid: u64, clear_key: &str) {
        if self.sweep.confirm.as_deref() == Some(clear_key) {
            // wide enough for the words on one line in English too
            ui.set_min_width(250.0);
            // room inside the menu's thin edge, like a dialog's
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 6,
                    bottom: 6,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label(
                        RichText::new(tr("Clear the whole board?"))
                            .font(crate::app::bold_font(ui.ctx(), 14.0))
                            .color(t.text),
                    );
                    ui.label(
                        RichText::new(tr("Everything on it is removed."))
                            .size(12.5)
                            .color(t.text2),
                    );
                    ui.add_space(12.0);
                    // a row only as tall as the buttons: a layout given the
                    // rest of the popup kept the menu's height and centred
                    // the buttons in it, far below the words
                    ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), crate::widgets::CONTROL_H),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().button_padding.x = 12.0;
                            let clear = ui.add(Tint(
                                egui::Button::new(RichText::new(tr("Clear")).color(Color32::WHITE))
                                    .fill(t.danger)
                                    .corner_radius(6)
                                    .min_size(vec2(0.0, crate::widgets::CONTROL_H)),
                            ));
                            name_it(&clear, "Clear");
                            if clear.clicked() {
                                if let Some(wb) =
                                    self.data.extra.whiteboards.iter_mut().find(|w| w.id == wid)
                                {
                                    wb.strokes.clear();
                                    wb.notes.clear();
                                    wb.shapes.clear();
                                    wb.lines.clear();
                                }
                                self.sweep.wb_sel = None;
                                self.sweep.wb_edit = None;
                                self.sweep.wb_undo.retain(|u| u.0 != wid);
                                self.sweep.wb_redo.retain(|u| u.0 != wid);
                                self.sweep.confirm = None;
                                self.dirty = true;
                                close_popup(ui);
                            }
                            let cancel = crate::widgets::secondary_button(ui, tr("Cancel"));
                            name_it(&cancel, "Cancel");
                            if cancel.clicked() {
                                self.sweep.confirm = None;
                            }
                        },
                    );
                });
            return;
        }
        if menu_row_named(ui, t, "Reset view", tr("Reset view"), false, |p, c| {
            line_icon(p, "fit", c, t.text2);
        })
        .clicked()
        {
            self.sweep.wb_view.remove(&wid);
            close_popup(ui);
        }
        if menu_row_named(ui, t, "Clear board", tr("Clear board"), false, |p, c| {
            line_icon(p, "eraser", c, t.text2);
        })
        .clicked()
        {
            self.sweep.confirm = Some(clear_key.to_string());
        }
        ui.separator();
        if danger_row(ui, t, "Delete whiteboard", tr("Delete whiteboard"), "trash").clicked() {
            if let Some(w) = self.data.extra.whiteboards.iter().find(|w| w.id == wid) {
                self.modal = Some(Modal::ConfirmRemove {
                    kind: "whiteboard",
                    id: w.id,
                    name: w.name.clone(),
                });
            }
            close_popup(ui);
        }
    }

    /// Top right: zoom out, the zoom (100% when pressed), zoom in, and back
    /// to the starting view.
    fn wb_zoom_bar(&mut self, ui: &mut Ui, t: &Theme, rect: Rect, origin: Pos2, wid: u64) {
        let w = ZOOM_W;
        let bar = Rect::from_min_size(
            pos2(rect.right() - EDGE - w, rect.top() + EDGE),
            vec2(w, BAR_H),
        );
        card(ui, t, bar, 12.0, ui.id().with(("wb_zoom_bar", wid)));
        self.sweep.wb_bars.push(bar);
        let mut view = self.sweep.wb_view.get(&wid).copied().unwrap_or_default();
        let before = view;
        let y = bar.center().y - BTN / 2.0;
        let mut x = bar.left() + 6.0;
        let id = |k: &str| ui.id().with(("wb_zoom", wid, k.to_string()));

        let r = Rect::from_min_size(pos2(x, y), vec2(BTN, BTN));
        if bar_button(
            ui,
            t,
            r,
            id("out"),
            "Zoom out",
            tr("Zoom out"),
            false,
            true,
            |p, c, fg| {
                paint_minus(p, c, 16.0, fg);
            },
        )
        .clicked()
        {
            view.zoom_at(origin, rect.center(), 1.0 / 1.25);
        }
        x += BTN;

        let r = Rect::from_min_size(pos2(x, y), vec2(PCT_W, BTN));
        let pct = format!("{:.0}%", view.zoom * 100.0);
        if bar_button(
            ui,
            t,
            r,
            id("pct"),
            "Zoom to 100%",
            tr("Zoom to 100%"),
            false,
            true,
            |p, c, _| {
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    &pct,
                    FontId::proportional(13.0),
                    t.text,
                );
            },
        )
        .clicked()
        {
            view.zoom_at(origin, rect.center(), 1.0 / view.zoom);
        }
        x += PCT_W;

        let r = Rect::from_min_size(pos2(x, y), vec2(BTN, BTN));
        if bar_button(
            ui,
            t,
            r,
            id("in"),
            "Zoom in",
            tr("Zoom in"),
            false,
            true,
            |p, c, fg| {
                paint_plus(p, c, 16.0, fg);
            },
        )
        .clicked()
        {
            view.zoom_at(origin, rect.center(), 1.25);
        }
        x += BTN;
        bar_sep(ui.painter(), t, x + 4.5, bar.center().y);
        x += 9.0;

        let r = Rect::from_min_size(pos2(x, y), vec2(BTN, BTN));
        let tip = crate::os::keys(tr(
            "Back to 100% at the board's top left corner. Drag with Space or the middle button, or scroll, to move around; pinch or \u{2318}+scroll to zoom.",
        ));
        if bar_button(
            ui,
            t,
            r,
            id("reset"),
            "Reset view",
            &tip,
            false,
            true,
            |p, c, fg| {
                line_icon(p, "fit", c, fg);
            },
        )
        .clicked()
        {
            view = WbView::default();
        }
        if view != before {
            self.set_wb_view(wid, view);
        }
    }

    /// Down the left: the tools, Undo and Redo. Returns the bar's place.
    fn wb_tool_bar(&mut self, ui: &mut Ui, t: &Theme, rect: Rect, wid: u64) -> Rect {
        let h = 6.0 + TOOLS.len() as f32 * (TOOL + 4.0) + 9.0 + TOOL + 4.0 + TOOL + 6.0;
        // in a short window it stays under the name bar
        let top = (rect.center().y - h / 2.0).max(rect.top() + EDGE + BAR_H + 12.0);
        let bar = Rect::from_min_size(pos2(rect.left() + EDGE, top), vec2(TOOL + 12.0, h));
        card(ui, t, bar, 14.0, ui.id().with(("wb_tool_bar", wid)));
        self.sweep.wb_bars.push(bar);
        let mut y = bar.top() + 6.0;
        for (tool, icon, name) in TOOLS {
            let r = Rect::from_min_size(pos2(bar.left() + 6.0, y), vec2(TOOL, TOOL));
            let on = self.wb_tool == tool;
            if bar_button(
                ui,
                t,
                r,
                ui.id().with(("wb_tool", wid, tool)),
                name,
                tr(name),
                on,
                true,
                |p, c, fg| {
                    line_icon(p, icon, c, fg);
                },
            )
            .clicked()
            {
                self.wb_tool = tool;
                self.sweep.wb_sel = None;
                self.sweep.wb_edit = None;
                self.close_wb_start(wid);
            }
            y += TOOL + 4.0;
        }
        ui.painter().hline(
            bar.left() + 12.0..=bar.right() - 12.0,
            y + 2.5,
            Stroke::new(1.0_f32, t.border),
        );
        y += 9.0;
        let can_undo = self.sweep.wb_undo.iter().any(|u| u.0 == wid);
        let r = Rect::from_min_size(pos2(bar.left() + 6.0, y), vec2(TOOL, TOOL));
        let tip = format!("{}  {}", tr("Undo"), crate::os::keys("\u{2318}Z"));
        if bar_button(
            ui,
            t,
            r,
            ui.id().with(("wb_undo", wid)),
            "Undo",
            &tip,
            false,
            can_undo,
            |p, c, fg| {
                line_icon(p, "undo", c, fg);
            },
        )
        .clicked()
        {
            self.wb_undo(wid);
        }
        y += TOOL + 4.0;
        let can_redo = self.sweep.wb_redo.iter().any(|u| u.0 == wid);
        let r = Rect::from_min_size(pos2(bar.left() + 6.0, y), vec2(TOOL, TOOL));
        let redo_keys = if cfg!(target_os = "macos") {
            crate::os::keys("\u{2318}\u{21E7}Z")
        } else {
            "Ctrl+Y".to_string()
        };
        let tip = format!("{}  {}", tr("Redo"), redo_keys);
        if bar_button(
            ui,
            t,
            r,
            ui.id().with(("wb_redo", wid)),
            "Redo",
            &tip,
            false,
            can_redo,
            |p, c, fg| {
                line_icon(p, "redo", c, fg);
            },
        )
        .clicked()
        {
            self.wb_redo(wid);
        }
        bar
    }

    /// Along the bottom: what the chosen tool can be set to. The pen has
    /// its colours and width, a sticky note its paper colours, a shape its
    /// kinds and colours, a line its colours and style, the others a line
    /// on how they are used. (Before, every setting showed whatever the
    /// tool, so it was not clear which one a colour went with.)
    fn wb_tool_options(&mut self, ui: &mut Ui, t: &Theme, rect: Rect, tools: Rect) {
        const SW: f32 = 30.0;
        const SEP: f32 = 17.0;
        const SLIDER: f32 = 104.0;
        let tool = self.wb_tool;
        let name = TOOLS.iter().find(|x| x.0 == tool).map_or("Pen", |x| x.2);
        let p = ui.painter().clone();
        let label = p.layout_no_wrap(
            tr(name).to_string(),
            crate::app::bold_font(ui.ctx(), 13.0),
            t.text,
        );
        let swatches: &[[u8; 3]] = match tool {
            0 | 4 | 5 => &WB_COLORS,
            1 => &WB_COLORS[1..],
            _ => &[],
        };
        let kinds = if tool == 4 { SHAPE_KINDS.len() } else { 0 };
        let toggles = if tool == 5 { STYLE_TOGGLES.len() } else { 0 };
        let hint = match tool {
            1 => Some(tr("Click the board to add a note")),
            2 => Some(tr(
                "Click a shape to change it, drag notes and shapes to move them",
            )),
            3 => Some(tr("Click or drag over a stroke to erase it")),
            4 => Some(tr("Click the board to put it there")),
            5 => Some(tr("Drag from one shape to another")),
            _ => None,
        }
        .map(|h| p.layout_no_wrap(h.to_string(), FontId::proportional(13.0), t.text2));
        let width_label = (tool == 0).then(|| {
            p.layout_no_wrap(tr("Width").to_string(), FontId::proportional(13.0), t.text2)
        });

        let mut w = 16.0 + label.size().x + SEP + swatches.len() as f32 * SW;
        if kinds > 0 {
            w += kinds as f32 * SW + SEP;
        }
        if toggles > 0 {
            w += SEP + toggles as f32 * SW;
        }
        if let Some(g) = &width_label {
            w += SEP + g.size().x + 8.0 + SLIDER + 8.0 + 30.0 + 26.0;
        }
        let hint_w = |g: &std::sync::Arc<egui::Galley>| {
            (if swatches.is_empty() { 0.0 } else { SEP }) + g.size().x + 4.0
        };
        let mut hint = hint;
        if let Some(g) = &hint {
            w += hint_w(g);
        }
        w += 12.0;
        // in a narrow window the hint goes before the bar runs off the page
        // (the Shape and Line tools' was cut off at the window's edge)
        let room = rect.right() - EDGE - (tools.right() + 12.0);
        if w > room {
            if let Some(g) = hint.take() {
                w -= hint_w(&g);
            }
        }
        // centred under the board, but clear of the tool bar
        let left = (rect.center().x - w / 2.0).max(tools.right() + 12.0);
        let bar = Rect::from_min_size(
            pos2(left, rect.bottom() - EDGE - OPTIONS_H),
            vec2(w, OPTIONS_H),
        );
        card(ui, t, bar, 14.0, ui.id().with("wb_tool_options"));
        self.sweep.wb_bars.push(bar);
        let cy = bar.center().y;
        let mut x = bar.left() + 16.0;
        p.galley(pos2(x, cy - label.size().y / 2.0), label.clone(), t.text);
        x += label.size().x;
        bar_sep(&p, t, x + SEP / 2.0, cy);
        x += SEP;

        // a square button with a picture, lit when `on`
        let pic_button = |x: f32, id: egui::Id, name: &str, on: bool| {
            let r = Rect::from_min_size(pos2(x, cy - SW / 2.0), vec2(SW, SW));
            let resp = ui.interact(r, id, Sense::click());
            name_it(&resp, name);
            if on {
                p.rect_filled(r, 7.0, t.primary.gamma_multiply(0.16));
            } else if resp.hovered() {
                p.rect_filled(r, 7.0, t.hover);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            (r.center(), resp)
        };
        if kinds > 0 {
            for (i, (kind, name, _)) in SHAPE_KINDS.iter().enumerate() {
                let on = self.sweep.wb_kind == i;
                let (c, resp) = pic_button(x, ui.id().with(("wb_kind", i)), name, on);
                paint_kind_icon(&p, kind, c, if on { t.primary } else { t.text2 });
                if resp.on_hover_text(tr(name)).clicked() {
                    self.sweep.wb_kind = i;
                }
                x += SW;
            }
            bar_sep(&p, t, x + SEP / 2.0, cy);
            x += SEP;
        }

        let chosen = if tool == 1 {
            sticky_color(self.wb_color)
        } else {
            self.wb_color
        };
        for (i, c) in swatches.iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, cy - SW / 2.0), vec2(SW, SW));
            let resp = ui.interact(r, ui.id().with(("wb_colour", tool, i)), Sense::click());
            name_it(&resp, &format!("Colour {}", i + 1));
            let fill = if tool == 1 {
                note_fill(t, *c)
            } else {
                ink(t, *c)
            };
            p.circle_filled(r.center(), 9.0, fill);
            if tool == 1 {
                // pale paper still shows against the bar
                p.circle_stroke(r.center(), 9.0, Stroke::new(1.0_f32, rgb(*c)));
            }
            if *c == chosen {
                p.circle_stroke(r.center(), 12.5, Stroke::new(2.0_f32, t.text));
            } else {
                hover_ring(
                    ui,
                    &resp,
                    Rect::from_center_size(r.center(), vec2(24.0, 24.0)),
                    12.0,
                );
            }
            if resp.clicked() {
                self.wb_color = *c;
            }
            x += SW;
        }

        if toggles > 0 {
            bar_sep(&p, t, x + SEP / 2.0, cy);
            x += SEP;
            for (k, (name, tip)) in STYLE_TOGGLES.iter().enumerate() {
                let st = self.sweep.wb_line_style;
                let on = [st.elbow, st.dashed, st.arrow][k];
                let (c, resp) = pic_button(x, ui.id().with(("wb_line_style", k)), name, on);
                paint_style_icon(&p, k, c, if on { t.primary } else { t.text2 });
                if resp.on_hover_text(tr(tip)).clicked() {
                    let st = &mut self.sweep.wb_line_style;
                    match k {
                        0 => st.elbow = !st.elbow,
                        1 => st.dashed = !st.dashed,
                        _ => st.arrow = !st.arrow,
                    }
                }
                x += SW;
            }
        }

        if let Some(g) = width_label {
            bar_sep(&p, t, x + SEP / 2.0, cy);
            x += SEP;
            p.galley(pos2(x, cy - g.size().y / 2.0), g.clone(), t.text2);
            x += g.size().x + 8.0;
            // the slider takes a control's height from the top of its place
            let sh = ui.spacing().interact_size.y;
            let sr = Rect::from_min_size(pos2(x, cy - sh / 2.0), vec2(SLIDER, sh));
            ui.scope_builder(egui::UiBuilder::new().max_rect(sr), |ui| {
                ui.spacing_mut().slider_width = SLIDER;
                ui.spacing_mut().slider_rail_height = 4.0;
                // the rail in the line colour (the page colour hid it), the
                // part up to the knob in the accent
                let v = ui.visuals_mut();
                v.widgets.inactive.bg_fill = t.border;
                v.widgets.inactive.fg_stroke = Stroke::new(1.5_f32, t.primary);
                v.widgets.hovered.fg_stroke = Stroke::new(2.0_f32, t.primary);
                v.widgets.active.fg_stroke = Stroke::new(2.0_f32, t.primary);
                v.selection.bg_fill = t.primary;
                let r = ui.add(
                    egui::Slider::new(&mut self.sweep.wb_width, 1.0..=16.0)
                        .show_value(false)
                        .step_by(0.5)
                        .trailing_fill(true),
                );
                name_it(&r, "Pen width");
            });
            x += SLIDER + 8.0;
            p.text(
                pos2(x, cy),
                Align2::LEFT_CENTER,
                format!("{:.1}", self.sweep.wb_width),
                FontId::proportional(13.0),
                t.text,
            );
            x += 30.0;
            // how thick the line comes out, in its colour
            let lw = self.sweep.wb_width.min(12.0);
            p.line_segment(
                [pos2(x + 2.0, cy), pos2(x + 22.0, cy)],
                Stroke::new(lw, ink(t, self.wb_color)),
            );
        }
        if let Some(g) = hint {
            if !swatches.is_empty() {
                bar_sep(&p, t, x + SEP / 2.0, cy);
                x += SEP;
            }
            p.galley(pos2(x, cy - g.size().y / 2.0), g.clone(), t.text2);
        }
    }

    /// The card in the middle of a board just made, while it is empty: it
    /// says the board was made and offers ways to start.
    fn wb_start_card(&mut self, ui: &mut Ui, t: &Theme, rect: Rect, wi: usize, now: f64) {
        let w = &self.data.extra.whiteboards[wi];
        let wid = w.id;
        let Some(new) = self.sweep.wb_new.clone().filter(|n| n.id == wid && n.start) else {
            return;
        };
        // anything on it means it has started
        if !w.strokes.is_empty()
            || !w.notes.is_empty()
            || !w.shapes.is_empty()
            || !w.lines.is_empty()
        {
            self.close_wb_start(wid);
            return;
        }
        // it rises into place as it fades in
        let k = ((now - new.since.unwrap_or(now)) / 0.25).clamp(0.0, 1.0) as f32;
        let e = 1.0 - (1.0 - k).powi(3);
        if k < 1.0 {
            ui.ctx().request_repaint();
        }
        let cw = 404.0f32.min(rect.width() - 150.0).max(300.0);
        let mut job = egui::text::LayoutJob::simple(
            tr("Type its name at the top left. How would you like to start?").to_string(),
            FontId::proportional(13.0),
            t.text2,
            cw - 56.0,
        );
        job.halign = egui::Align::Center;
        let sub = ui.painter().layout_job(job);
        // the templates as chips, wrapped onto rows: two and "More templates",
        // or all of them once that is pressed (the two first, where they were)
        const SHOWN: [&str; 2] = ["Action Plan", "Flow Chart"];
        let mut names: Vec<&str> = SHOWN.to_vec();
        if new.all_templates {
            names.extend(
                super::wb_library::TEMPLATES
                    .iter()
                    .map(|(n, ..)| *n)
                    .filter(|n| !SHOWN.contains(n)),
            );
        }
        let mut more = (!new.all_templates).then(|| {
            ui.painter().layout_no_wrap(
                format!("{} \u{203A}", tr("More templates")),
                FontId::proportional(13.0),
                t.text2,
            )
        });
        let mut chips: Vec<(Option<&str>, egui::Vec2)> = names
            .iter()
            .map(|n| {
                let w = ui
                    .painter()
                    .layout_no_wrap(tr(n).to_string(), FontId::proportional(13.0), t.text)
                    .size()
                    .x;
                (Some(*n), vec2(w + 40.0, 32.0))
            })
            .collect();
        if let Some(g) = &more {
            chips.push((None, vec2(g.size().x + 16.0, 32.0)));
        }
        let mut at = vec![];
        let (mut x, mut row) = (0.0f32, 0.0f32);
        for (_, size) in &chips {
            if x > 0.0 && x + size.x > cw - 48.0 {
                x = 0.0;
                row += 1.0;
            }
            at.push(vec2(x, row * 40.0));
            x += size.x + 8.0;
        }
        let chips_h = row * 40.0 + 32.0;
        let h = 24.0
            + 40.0
            + 14.0
            + 22.0
            + 6.0
            + sub.size().y
            + 20.0
            + 72.0
            + 18.0
            + 22.0
            + chips_h
            + 22.0;
        let cr = Rect::from_center_size(rect.center() + vec2(0.0, (1.0 - e) * 10.0), vec2(cw, h));
        let mut c = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(cr)
                .id_salt(("wb_start", wid)),
        );
        c.multiply_opacity(e);
        card(&c, t, cr, 16.0, c.id().with("card"));
        self.sweep.wb_bars.push(cr);
        let p = c.painter().clone();
        let cx = cr.center().x;
        let mut y = cr.top() + 24.0;

        let green = if t.dark {
            rgb([74, 222, 128])
        } else {
            rgb([22, 128, 61])
        };
        let badge = pos2(cx, y + 20.0);
        p.circle_filled(badge, 20.0, green.gamma_multiply(0.16));
        line_icon_scaled(&p, "check", badge, green, 1.3);
        y += 40.0 + 14.0;
        p.text(
            pos2(cx, y),
            Align2::CENTER_TOP,
            tr("New whiteboard created"),
            crate::app::bold_font(c.ctx(), 17.0),
            t.text,
        );
        y += 22.0 + 6.0;
        let sub_h = sub.size().y;
        p.galley(pos2(cx, y), sub, t.text2);
        y += sub_h + 20.0;

        let bw = (cw - 48.0 - 10.0) / 2.0;
        for (k, (icon, name, tool)) in [
            ("pen", "Draw with the pen", 0u8),
            ("sticky", "Add sticky notes", 1),
        ]
        .into_iter()
        .enumerate()
        {
            let r = Rect::from_min_size(
                pos2(cr.left() + 24.0 + k as f32 * (bw + 10.0), y),
                vec2(bw, 72.0),
            );
            let resp = c.interact(r, c.id().with(("way", k)), Sense::click());
            name_it(&resp, name);
            let hovered = resp.hovered();
            p.rect_filled(r, 10.0, if hovered { t.hover } else { t.bg });
            p.rect_stroke(
                r,
                10.0,
                Stroke::new(
                    1.0_f32,
                    if hovered {
                        crate::widgets::hover_line(&c)
                    } else {
                        t.border
                    },
                ),
                egui::StrokeKind::Inside,
            );
            line_icon_scaled(&p, icon, r.center() - vec2(0.0, 11.0), t.text, 1.25);
            p.text(
                r.center() + vec2(0.0, 16.0),
                Align2::CENTER_CENTER,
                tr(name),
                FontId::proportional(13.0),
                t.text,
            );
            if hovered {
                c.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.wb_tool = tool;
                self.close_wb_start(wid);
            }
        }
        y += 72.0 + 18.0;
        p.text(
            pos2(cr.left() + 24.0, y),
            Align2::LEFT_TOP,
            tr("Or fill it from a template"),
            FontId::proportional(12.0),
            t.text2,
        );
        y += 22.0;

        let top = pos2(cr.left() + 24.0, y);
        for ((name, size), off) in chips.into_iter().zip(at) {
            let r = Rect::from_min_size(top + off, size);
            let Some(name) = name else {
                // every template, here on the card, so the one picked fills
                // this board
                let resp = c.interact(r, c.id().with("more"), Sense::click());
                name_it(&resp, "More templates");
                if let Some(g) = more.take() {
                    p.galley(
                        pos2(r.left() + 8.0, r.center().y - g.size().y / 2.0),
                        g,
                        if resp.hovered() { t.text } else { t.text2 },
                    );
                }
                if resp.hovered() {
                    c.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    if let Some(n) = self.sweep.wb_new.as_mut().filter(|n| n.id == wid) {
                        n.all_templates = true;
                    }
                }
                continue;
            };
            let colour = super::wb_library::TEMPLATES
                .iter()
                .find(|(n, ..)| *n == name)
                .map_or([0x88, 0x88, 0x88], |(_, _, c)| *c);
            let g = p.layout_no_wrap(tr(name).to_string(), FontId::proportional(13.0), t.text);
            let resp = c.interact(r, c.id().with(("template", name)), Sense::click());
            name_it(&resp, name);
            let hovered = resp.hovered();
            p.rect_filled(r, 16.0, if hovered { t.hover } else { t.bg });
            p.rect_stroke(
                r,
                16.0,
                Stroke::new(1.0_f32, t.border),
                egui::StrokeKind::Inside,
            );
            p.rect_filled(
                Rect::from_center_size(pos2(r.left() + 17.0, r.center().y), vec2(9.0, 9.0)),
                2.5,
                rgb(colour),
            );
            p.galley(
                pos2(r.left() + 28.0, r.center().y - g.size().y / 2.0),
                g,
                t.text,
            );
            if hovered {
                c.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.wb_fill_template(wid, name);
                self.wb_tool = 2;
                self.close_wb_start(wid);
            }
        }
    }
}
