// Small painted widgets and the pickers used across views:
// status circle, priority flag, avatars, pills, and the status, priority,
// assignee and date pickers.

use crate::i18n::{tr, DateText};
use crate::model::*;
use crate::theme::{rgb, Theme};
use chrono::{Datelike, Duration, NaiveDate};
use egui::{pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui};

pub fn paint_status_circle(p: &egui::Painter, center: Pos2, r: f32, st: &Status) {
    let c = rgb(st.color);
    match st.kind {
        StatusKind::Closed | StatusKind::Done => {
            p.circle_filled(center, r, c);
            // check mark
            let a = center + vec2(-r * 0.45, 0.0);
            let b = center + vec2(-r * 0.1, r * 0.38);
            let d = center + vec2(r * 0.5, -r * 0.35);
            p.line_segment([a, b], Stroke::new(1.6, Color32::WHITE));
            p.line_segment([b, d], Stroke::new(1.6, Color32::WHITE));
        }
        StatusKind::Custom => {
            p.circle_stroke(center, r, Stroke::new(1.5, c));
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

/// Names a custom-drawn control so screen readers (and the click tests) can find it.
pub fn name_it(resp: &Response, label: &str) {
    let l = label.to_string();
    resp.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::Button, true, l.clone()));
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
            Stroke::new(1.5, color),
        );
    }
}

pub fn paint_flag(p: &egui::Painter, pos: Pos2, color: Color32, h: f32) {
    let top = pos + vec2(0.0, -h * 0.5);
    let bottom = pos + vec2(0.0, h * 0.5);
    p.line_segment([top, bottom], Stroke::new(1.5, color));
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

pub fn paint_avatar(p: &egui::Painter, center: Pos2, r: f32, m: &Member, ring: Color32) {
    p.circle_filled(center, r + 1.5, ring);
    p.circle_filled(center, r, rgb(m.color));
    p.text(
        center,
        Align2::CENTER_CENTER,
        m.initials(),
        FontId::proportional(r * 0.85),
        Color32::WHITE,
    );
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
    p.circle_stroke(center, r, Stroke::new(1.0, color));
    // person glyph
    p.circle_stroke(
        center + vec2(0.0, -r * 0.25),
        r * 0.28,
        Stroke::new(1.0, color),
    );
    p.line_segment(
        [
            center + vec2(-r * 0.45, r * 0.5),
            center + vec2(r * 0.45, r * 0.5),
        ],
        Stroke::new(1.0, color),
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
            ui.painter().circle_stroke(c, 4.5, Stroke::new(1.4, fg));
            ui.painter().circle_filled(c, 2.0, fg);
        }
        _ => {
            ui.painter().circle_filled(c, 5.0, fg);
            ui.painter().line_segment(
                [c + vec2(-2.3, 0.0), c + vec2(-0.6, 1.8)],
                Stroke::new(1.3, fill),
            );
            ui.painter().line_segment(
                [c + vec2(-0.6, 1.8), c + vec2(2.5, -1.8)],
                Stroke::new(1.3, fill),
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

pub fn fmt_date(d: NaiveDate) -> String {
    let t = today();
    let diff = (d - t).num_days();
    match diff {
        0 => tr("Today").into(),
        1 => tr("Tomorrow").into(),
        -1 => tr("Yesterday").into(),
        2..=6 => d.text("%a"),
        _ => {
            if d.year() == t.year() {
                d.text("%b %-d")
            } else {
                d.text("%-m/%-d/%y")
            }
        }
    }
}

pub fn date_color(d: NaiveDate, closed: bool, t: &Theme) -> Color32 {
    if closed {
        return t.text3;
    }
    let diff = (d - today()).num_days();
    if diff < 0 {
        t.danger
    } else if diff == 0 {
        Color32::from_rgb(0xe1, 0x6b, 0x16)
    } else {
        t.text2
    }
}

pub fn fmt_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        crate::trf!("{}h {}m", h, m)
    } else if m > 0 {
        crate::trf!("{}m {}s", m, s)
    } else {
        crate::trf!("{}s", s)
    }
}

pub fn fmt_ago(ts: i64) -> String {
    let d = now_ts() - ts;
    if d < 60 {
        tr("just now").into()
    } else if d < 3600 {
        crate::trf!("{} mins ago", (d / 60).max(1))
    } else if d < 86400 {
        let h = d / 3600;
        if h == 1 {
            tr("1 hour ago").into()
        } else {
            crate::trf!("{} hours ago", h)
        }
    } else {
        let dt = chrono::DateTime::from_timestamp(ts, 0)
            .map(|x| x.with_timezone(&chrono::Local).text("%b %-d at %-I:%M %p"))
            .unwrap_or_default();
        dt
    }
}

/// Opens a popup below `resp` when it is clicked; shows `add` while open.
pub fn popup<R>(
    ui: &Ui,
    resp: &Response,
    id: egui::Id,
    width: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    // a menu taller than the window scrolls inside itself instead of running
    // off the bottom (egui moves a shorter one up until it fits). Only menus
    // that were too tall last frame get the scroll area, so ordinary menus
    // are laid out exactly as before.
    let screen = ui.ctx().screen_rect();
    let room = (screen.bottom() - resp.rect.bottom() - 32.0).max(screen.height() * 0.6);
    let tall_id = id.with("__tall");
    let tall: f32 = ui.data(|d| d.get_temp(tall_id)).unwrap_or(0.0);
    egui::popup_below_widget(
        ui,
        id,
        resp,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(width);
            if tall > room {
                egui::ScrollArea::vertical()
                    .id_salt(id.with("scroll"))
                    .max_height(room)
                    .auto_shrink([true, true])
                    .show(ui, |ui| {
                        ui.set_min_width(width);
                        let r = add(ui);
                        let h = ui.min_rect().height();
                        ui.data_mut(|d| d.insert_temp(tall_id, h));
                        r
                    })
                    .inner
            } else {
                let top = ui.min_rect().top();
                let r = add(ui);
                let h = ui.min_rect().bottom() - top;
                ui.data_mut(|d| d.insert_temp(tall_id, h));
                r
            }
        },
    )
}

/// Opens a real egui menu below `resp` when it is clicked, the same kind a
/// right click opens: rows draw as menu rows and `ui.menu_button` inside
/// opens a side submenu instead of a new dropdown over the parent.
///
/// egui only opens submenus to the right, so near the right edge of the
/// window the menu is shifted left to leave room for one.
pub fn click_menu(resp: &Response, add: impl FnOnce(&mut Ui)) {
    const MENU_W: f32 = 230.0;
    const SUB_ROOM: f32 = 240.0;
    let ctx = &resp.ctx;
    let bar_id = resp.id.with("__click_menu");
    let mut st = egui::menu::BarState::load(ctx, bar_id);
    let open = st.as_ref().is_some_and(|r| r.id == resp.id);
    let pressed_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.primary_down())
        && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
            !resp.rect.contains(p)
                && !st
                    .as_ref()
                    .is_some_and(|r| r.menu_state.read().area_contains(p))
        });
    if open && (resp.clicked() || pressed_outside) {
        **st = None;
    } else if resp.clicked() {
        let screen = ctx.screen_rect();
        let margin = egui::Frame::menu(&ctx.style()).total_margin().left;
        let mut pos = resp.rect.left_bottom() + vec2(-margin, ctx.style().spacing.menu_spacing);
        let max_x = screen.right() - MENU_W - SUB_ROOM;
        if pos.x > max_x {
            pos.x = max_x.max(screen.left() + 4.0);
        }
        **st = Some(egui::menu::MenuRoot::new(pos, resp.id));
    }
    // (MenuRootManager::show returns None while the menu stays open, so
    // read the state instead)
    let shown = st.as_ref().is_some_and(|r| r.id == resp.id);
    st.show(resp, add);
    st.store(ctx, bar_id);
    if shown {
        let nr = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(MENU_SHOWN), nr));
    }
}

const MENU_SHOWN: &str = "__wb_click_menu_shown";

/// True when a popup, a right-click menu or a `click_menu` was on screen in
/// the previous pass. Call it at the start of a pass: egui closes those on
/// Escape during the pass, so checking later sees them already gone.
pub fn any_menu_was_open(ctx: &egui::Context) -> bool {
    let nr = ctx.cumulative_pass_nr();
    let last: Option<u64> = ctx.data(|d| d.get_temp(egui::Id::new(MENU_SHOWN)));
    ctx.memory(|m| m.any_popup_open())
        || ctx.is_context_menu_open()
        || last.is_some_and(|l| l + 1 >= nr)
}

pub fn close_popup(ui: &Ui) {
    ui.memory_mut(|m| m.close_popup());
}

/// A plain full-width menu row with an optional painted lead.
pub fn menu_row(
    ui: &mut Ui,
    t: &Theme,
    label: &str,
    selected: bool,
    lead: impl FnOnce(&egui::Painter, Pos2),
) -> Response {
    let w = ui.available_width().max(160.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    name_it(&resp, label);
    if resp.hovered() || selected {
        ui.painter()
            .rect_filled(rect, 5.0, if selected { t.selected_bg } else { t.hover });
    }
    lead(ui.painter(), rect.left_center() + vec2(14.0, 0.0));
    ui.painter().text(
        rect.left_center() + vec2(30.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        t.text,
    );
    resp
}

pub fn status_menu(ui: &mut Ui, t: &Theme, statuses: &[Status], current: &mut String) -> bool {
    let mut changed = false;
    ui.label(egui::RichText::new(tr("Statuses")).small().color(t.text3));
    for st in statuses {
        let sel = &st.name == current;
        let r = menu_row(
            ui,
            t,
            &crate::i18n::status_text(&st.name).to_uppercase(),
            sel,
            |p, c| paint_status_circle(p, c, 6.0, st),
        );
        if r.clicked() {
            if !sel {
                *current = st.name.clone();
                changed = true;
            }
            close_popup(ui);
        }
    }
    changed
}

pub fn priority_menu(ui: &mut Ui, t: &Theme, current: &mut u8) -> bool {
    match priority_pick(ui, t, *current) {
        Some(p) if p != *current => {
            *current = p;
            true
        }
        _ => false,
    }
}

/// The priority list; returns the row the user clicked this frame (0 for
/// Clear), even when it equals `current`. Bulk edit needs "Clear was
/// clicked" separately from "the value changed".
pub fn priority_pick(ui: &mut Ui, t: &Theme, current: u8) -> Option<u8> {
    let mut picked = None;
    ui.label(
        egui::RichText::new(tr("Task Priority"))
            .small()
            .color(t.text3),
    );
    for pr in [1u8, 2, 3, 4] {
        let col = priority_color(pr, t);
        let r = menu_row(
            ui,
            t,
            tr(PRIORITIES[pr as usize].label),
            current == pr,
            |p, c| paint_flag(p, c + vec2(-4.0, 0.0), col, 13.0),
        );
        if r.clicked() {
            picked = Some(pr);
            close_popup(ui);
        }
    }
    ui.separator();
    let r = menu_row(ui, t, tr("Clear"), false, |p, c| {
        p.text(
            c,
            Align2::CENTER_CENTER,
            "\u{2716}",
            FontId::proportional(10.0),
            Color32::GRAY,
        );
    });
    if r.clicked() {
        picked = Some(0);
        close_popup(ui);
    }
    picked
}

pub fn assignee_menu(ui: &mut Ui, t: &Theme, data: &Data, current: &mut Vec<u64>) -> bool {
    let mut changed = false;
    ui.label(egui::RichText::new(tr("People")).small().color(t.text3));
    for m in &data.members {
        let sel = current.contains(&m.id);
        let label = if m.id == data.me {
            crate::trf!("{} (you)", m.name)
        } else {
            m.name.clone()
        };
        let ring = t.card;
        let r = menu_row(ui, t, &label, sel, |p, c| paint_avatar(p, c, 9.0, m, ring));
        if sel {
            ui.painter().text(
                r.rect.right_center() - vec2(12.0, 0.0),
                Align2::RIGHT_CENTER,
                "\u{2714}",
                FontId::proportional(12.0),
                t.selected_text,
            );
        }
        if r.clicked() {
            if sel {
                current.retain(|x| *x != m.id);
            } else if data.extra.apps.multiple_assignees {
                current.push(m.id);
            } else {
                *current = vec![m.id];
                close_popup(ui);
            }
            changed = true;
        }
    }
    changed
}

/// Quick options plus a month calendar. Returns true when the date changed.
pub fn date_menu(ui: &mut Ui, t: &Theme, id: egui::Id, current: &mut Option<NaiveDate>) -> bool {
    date_menu_inner(ui, t, id, current, true)
}

fn date_menu_inner(
    ui: &mut Ui,
    t: &Theme,
    id: egui::Id,
    current: &mut Option<NaiveDate>,
    close: bool,
) -> bool {
    let close_popup = |ui: &Ui| {
        if close {
            close_popup(ui)
        }
    };
    let mut changed = false;
    let base = current.unwrap_or_else(today);
    let mut month: NaiveDate = ui
        .data(|d| d.get_temp(id))
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(base.year(), base.month(), 1).unwrap());
    let mut sep_x = 0.0;
    let cols = ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(130.0);
            let td = today();
            let wd = td.weekday().num_days_from_monday() as i64;
            let opts: [(&str, NaiveDate); 7] = [
                ("Today", td),
                ("Tomorrow", td + Duration::days(1)),
                ("This weekend", td + Duration::days((5 - wd).max(0))),
                ("Next week", td + Duration::days(7 - wd)),
                ("2 weeks", td + Duration::days(14)),
                ("4 weeks", td + Duration::days(28)),
                ("8 weeks", td + Duration::days(56)),
            ];
            for (label, d) in opts {
                let r = ui.add(ghost(
                    egui::Button::new(tr(label)).min_size(vec2(126.0, 24.0)),
                ));
                if r.clicked() {
                    *current = Some(d);
                    changed = true;
                    close_popup(ui);
                }
            }
            if current.is_some() {
                ui.separator();
                if ui
                    .add(ghost(egui::Button::new(
                        egui::RichText::new(tr("Clear")).color(t.danger),
                    )))
                    .clicked()
                {
                    *current = None;
                    changed = true;
                    close_popup(ui);
                }
            }
        });
        // a plain gap, with the line painted below once the height is known:
        // ui.separator() here takes all the height left in the window and
        // pushed the rows under this menu off screen
        let gap = ui.allocate_exact_size(vec2(9.0, 0.0), Sense::hover()).0;
        sep_x = gap.center().x;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(month.text("%B %Y")).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(">").clicked() {
                        month = add_months(month, 1);
                    }
                    if ui.small_button("<").clicked() {
                        month = add_months(month, -1);
                    }
                });
            });
            let cell = 28.0;
            let (rect, _) = ui.allocate_exact_size(vec2(cell * 7.0, cell * 7.0), Sense::hover());
            let p = ui.painter();
            for (i, d) in ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
                .iter()
                .enumerate()
            {
                p.text(
                    rect.min + vec2(cell * (i as f32 + 0.5), cell * 0.5),
                    Align2::CENTER_CENTER,
                    tr(d),
                    FontId::proportional(11.0),
                    t.text3,
                );
            }
            let start = month - Duration::days(month.weekday().num_days_from_sunday() as i64);
            for i in 0..42 {
                let d = start + Duration::days(i);
                let r = Rect::from_min_size(
                    rect.min + vec2(cell * (i % 7) as f32, cell * (1 + i / 7) as f32),
                    vec2(cell, cell),
                )
                .shrink(1.5);
                let resp = ui.interact(r, id.with(i), Sense::click());
                let sel = Some(d) == *current;
                let is_today = d == today();
                if sel {
                    ui.painter()
                        .circle_filled(r.center(), cell * 0.42, t.primary);
                } else if resp.hovered() {
                    ui.painter().circle_filled(r.center(), cell * 0.42, t.hover);
                }
                let col = if sel {
                    Color32::WHITE
                } else if d.month() != month.month() {
                    t.text3.gamma_multiply(0.6)
                } else if is_today {
                    t.danger
                } else {
                    t.text
                };
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    d.day().to_string(),
                    FontId::proportional(12.0),
                    col,
                );
                if resp.clicked() {
                    *current = Some(d);
                    changed = true;
                    close_popup(ui);
                }
            }
        });
    });
    ui.painter().vline(
        sep_x,
        cols.response.rect.y_range(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    ui.data_mut(|d| d.insert_temp(id, month));
    changed
}

pub fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + n;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap()
}

/// Rounded primary button.
pub fn primary_button(ui: &mut Ui, t: &Theme, text: &str) -> Response {
    ui.add(Tint(
        egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong())
            .fill(t.primary)
            .corner_radius(6)
            .min_size(vec2(0.0, 28.0)),
    ))
}

/// A small action button in a list row: room around the text, rounded like
/// the rest (egui's small_button has no vertical padding, so the text touched
/// the outline).
pub fn row_button(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = vec2(10.0, 3.0);
        ui.add(
            egui::Button::new(text)
                .corner_radius(6)
                .min_size(vec2(0.0, 26.0)),
        )
    })
    .inner
}

pub fn chip(ui: &mut Ui, text: &str, color: Color32) -> Response {
    let font = FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 18.0), Sense::click());
    let strength = if resp.hovered() { 0.28 } else { 0.16 };
    ui.painter()
        .rect_filled(rect, 4.0, color.gamma_multiply(strength));
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
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

/// Place a widget over an exact rectangle without moving the layout cursor
/// (unlike `Ui::put`, which pulls the following rows up to the rectangle).
pub fn put_over(ui: &mut egui::Ui, rect: egui::Rect, w: impl egui::Widget) -> Response {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
    ));
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

/// Repeat choices for recurring tasks (stored value, label).
pub const RECUR: [(&str, &str); 5] = [
    ("", "Never"),
    ("daily", "Daily"),
    ("weekly", "Weekly"),
    ("monthly", "Monthly"),
    ("yearly", "Yearly"),
];

pub fn recur_label(r: &Option<String>) -> &'static str {
    RECUR
        .iter()
        .find(|x| Some(x.0) == r.as_deref())
        .map(|x| x.1)
        .unwrap_or("Never")
}

/// The next date of a repeating task.
pub fn next_recur(d: NaiveDate, r: &str) -> NaiveDate {
    match r {
        "daily" => d + Duration::days(1),
        "weekly" => d + Duration::days(7),
        "monthly" => d
            .checked_add_months(chrono::Months::new(1))
            .unwrap_or(d + Duration::days(30)),
        "yearly" => d
            .checked_add_months(chrono::Months::new(12))
            .unwrap_or(d + Duration::days(365)),
        _ => d,
    }
}

/// Start date / Due date picker with a repeat choice, as in the original's
/// date popup. Returns true when anything changed.
pub fn dates_menu(
    ui: &mut Ui,
    t: &Theme,
    id: egui::Id,
    start: &mut Option<NaiveDate>,
    due: &mut Option<NaiveDate>,
    recur: &mut Option<String>,
) -> bool {
    let mut which: u8 = ui.data(|d| d.get_temp(id.with("which"))).unwrap_or(1);
    let mut changed = false;
    ui.horizontal(|ui| {
        for (k, label, v) in [(0u8, "Start date", *start), (1u8, "Due date", *due)] {
            let txt = match v {
                Some(d) => format!("{}: {}", tr(label), fmt_date(d)),
                None => tr(label).to_string(),
            };
            let sel = which == k;
            let b = Tint(
                egui::Button::new(egui::RichText::new(txt).color(if sel {
                    t.text
                } else {
                    t.text2
                }))
                .fill(if sel {
                    t.selected_bg
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(Stroke::new(1.0, t.border))
                .min_size(vec2(170.0, 28.0)),
            );
            if ui.add(b).clicked() {
                which = k;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id.with("which"), which));
    ui.add_space(4.0);
    // the calendar closes the popup on a pick; keep it open while choosing start
    let target = if which == 0 { start } else { due };
    let before = *target;
    let mut tmp = before;
    let picked = date_menu_inner(ui, t, id.with(which), &mut tmp, false);
    if picked {
        *target = tmp;
        changed = true;
        if which == 0 && tmp.is_some() {
            // after the start date, pick the due date
            ui.data_mut(|d| d.insert_temp(id.with("which"), 1u8));
        } else {
            close_popup(ui);
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr("\u{1F501}  Set Recurring")).color(t.text2));
        // plain buttons: a combo box would close this popup
        for (v, label) in RECUR {
            let sel = recur.as_deref().unwrap_or("") == v;
            if ui.selectable_label(sel, tr(label)).clicked() && !sel {
                *recur = if v.is_empty() {
                    None
                } else {
                    Some(v.to_string())
                };
                changed = true;
            }
        }
    });
    changed
}

/// The frame every dialog window uses (Create List, Create a Space, Create Task...).
pub fn dialog_frame(t: &Theme) -> egui::Frame {
    egui::Frame::new()
        .fill(t.card)
        .corner_radius(12)
        .stroke(Stroke::new(1.0, t.border))
        .inner_margin(egui::Margin::same(20))
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
                Stroke::new(1.5, t.text2),
            );
            ui.painter().line_segment(
                [c + vec2(4.5, -4.5), c + vec2(-4.5, 4.5)],
                Stroke::new(1.5, t.text2),
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
    let (r, resp) = ui.allocate_exact_size(vec2(34.0, 20.0), Sense::click());
    name_it(&resp, label);
    if resp.clicked() {
        *on = !*on;
    }
    let bg = if *on { t.primary } else { t.border };
    ui.painter().rect_filled(r, 10.0, bg);
    hover_bg(ui, &resp, r, 10.0);
    let x = if *on {
        r.right() - 10.0
    } else {
        r.left() + 10.0
    };
    ui.painter()
        .circle_filled(pos2(x, r.center().y), 7.0, Color32::WHITE);
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
/// reads the same on the page, a card, the sidebar or a popup.
pub fn hover_fill(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_white_alpha(22)
    } else {
        Color32::from_black_alpha(14)
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
                fill.gamma_multiply(1.8)
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
                hover_fill(ui).gamma_multiply(1.8)
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
            Stroke::new(1.5, ui.visuals().widgets.hovered.bg_stroke.color),
            egui::StrokeKind::Outside,
        );
    }
}
