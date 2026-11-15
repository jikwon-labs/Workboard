// Small painted widgets and the pickers used across views:
// status circle, priority flag, avatars, pills, and the status, priority,
// assignee and date pickers.

use crate::i18n::{tr, DateText};
use crate::model::*;
use crate::theme::{rgb, Theme};
use chrono::{Datelike, Duration, NaiveDate, NaiveTime};
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

/// A time of day in the date style ("2:30 PM").
pub fn fmt_time(time: NaiveTime) -> String {
    time.text("%-I:%M %p")
}

/// A date with its time of day when there is one ("Today 2:30 PM").
pub fn fmt_when(d: NaiveDate, time: Option<NaiveTime>) -> String {
    match time {
        Some(time) => format!("{} {}", fmt_date(d), fmt_time(time)),
        None => fmt_date(d),
    }
}

/// A task's due date (and time) for display.
pub fn fmt_due(task: &Task) -> Option<String> {
    task.due.map(|d| fmt_when(d, task.due_at()))
}

/// A task's start date (and time) for display.
pub fn fmt_start(task: &Task) -> Option<String> {
    task.start.map(|d| fmt_when(d, task.start_at()))
}

/// The colour of a task's due date: red once it is overdue (with a due time,
/// from that time on), orange later today.
pub fn due_color(task: &Task, closed: bool, t: &Theme) -> Color32 {
    match task.due {
        Some(_) if !closed && task.is_overdue() => t.danger,
        Some(d) => date_color(d, closed, t),
        None => t.text3,
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

/// Tracked time in hours and minutes ("1h 20m", "40m", "2h"), the one way
/// timesheets show it, in their cells and their totals; "-" for none.
pub fn fmt_hm(secs: i64) -> String {
    let min = (secs as f64 / 60.0).round() as i64;
    match (min / 60, min % 60) {
        (0, 0) => "-".into(),
        (0, m) => crate::trf!("{}m", m),
        (h, 0) => crate::trf!("{}h", h),
        (h, m) => crate::trf!("{}h {}m", h, m),
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

/// `text` cut with an ellipsis so it is at most `max_w` wide in `font`.
pub fn fit_text(p: &egui::Painter, text: &str, font: &FontId, max_w: f32) -> String {
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
            let out = if tall > room {
                egui::ScrollArea::vertical()
                    .id_salt(id.with("scroll"))
                    .max_height(room)
                    .auto_shrink([true, true])
                    .show(ui, |ui| {
    // egui only pushes a popup back inside the window, so near the right
    // edge it touched the edge: move it left to leave a margin (the width
    // is last frame's, or the minimum before it has been shown)
    let wide_id = id.with("__wide");
    let wide: f32 = ui.data(|d| d.get_temp(wide_id)).unwrap_or(width + 24.0);
    let over = resp.rect.left() + wide + 8.0 - screen.right();
    let mut at = resp.clone();
    if over > 0.0 {
        at.rect = at.rect.translate(vec2(-over, 0.0));
    }
    let resp = &at;
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
            };
            // the frame's margin on both sides
            let w =
                ui.min_rect().width() + 2.0 * egui::Frame::popup(ui.style()).total_margin().left;
            ui.data_mut(|d| d.insert_temp(wide_id, w));
            out
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
    menu_row_named(ui, t, label, label, selected, lead)
}

/// A `menu_row` showing `label` whose accessibility name is `name` (a row
/// that shows a task's name but is called "Pick <name>").
pub fn menu_row_named(
    ui: &mut Ui,
    t: &Theme,
    name: &str,
    label: &str,
    selected: bool,
    lead: impl FnOnce(&egui::Painter, Pos2),
) -> Response {
    let w = ui.available_width().max(160.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    name_it(&resp, name);
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

/// A menu row with no icon: the text starts at the row's edge instead of
/// after an empty icon slot.
pub fn menu_row_plain(ui: &mut Ui, t: &Theme, label: &str, selected: bool) -> Response {
    let w = ui.available_width().max(160.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    name_it(&resp, label);
    if resp.hovered() || selected {
        ui.painter()
            .rect_filled(rect, 5.0, if selected { t.selected_bg } else { t.hover });
    }
    ui.painter().text(
        rect.left_center() + vec2(10.0, 0.0),
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
            "\u{00D7}",
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
            // a drawn tick (the glyph came from the emoji font)
            let c = r.rect.right_center() - vec2(16.0, 0.0);
            ui.painter().add(egui::Shape::line(
                vec![
                    c + vec2(-4.5, 0.3),
                    c + vec2(-1.5, 3.3),
                    c + vec2(4.8, -3.2),
                ],
                Stroke::new(1.5, t.selected_text),
            ));
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
    egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong())
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

/// Under a task's date picker: "Add time", or the time in a field (typed as
/// "14:30" or "2:30 pm", kept on Enter or when the field is left) and
/// "Remove time". Returns the new time when it changed (None inside: removed).
pub fn time_row(
    ui: &mut Ui,
    t: &Theme,
    id: egui::Id,
    time: Option<NaiveTime>,
) -> Option<Option<NaiveTime>> {
    let mut out = None;
    ui.horizontal(|ui| match time {
        None => {
            let b = ui.add(ghost(egui::Button::new(
                egui::RichText::new(tr("Add time")).color(t.text2),
            )));
            name_it(&b, "Add time");
            if b.clicked() {
                out = Some(NaiveTime::from_hms_opt(9, 0, 0));
            }
        }
        Some(cur) => {
            ui.label(egui::RichText::new(tr("Time")).color(t.text2));
            let buf_id = id.with("time_buf");
            let mut buf: String = ui
                .data(|d| d.get_temp(buf_id))
                .unwrap_or_else(|| cur.format("%H:%M").to_string());
            let r = ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .hint_text("14:30")
                    .desired_width(64.0),
            );
            name_field(&r, "Time of day");
            if r.lost_focus() {
                if let Some(nt) = parse_time_of_day(&buf).filter(|nt| *nt != cur) {
                    out = Some(Some(nt));
                }
                ui.data_mut(|d| d.remove::<String>(buf_id));
            } else if r.has_focus() {
                ui.data_mut(|d| d.insert_temp(buf_id, buf));
            } else {
                ui.data_mut(|d| d.remove::<String>(buf_id));
            }
            let b = ui.add(ghost(egui::Button::new(
                egui::RichText::new(tr("Remove time")).color(t.danger),
            )));
            name_it(&b, "Remove time");
            if b.clicked() {
                out = Some(None);
            }
        }
    });
    out
}

/// A task date picker changed `date` (it was `before`): a cleared date takes
/// its time along, and a date set where there was none starts without one.
fn date_picked(
    before: Option<NaiveDate>,
    date: Option<NaiveDate>,
    slot: &mut Option<Option<NaiveTime>>,
) {
    if date.is_none() || before.is_none() {
        set_slot(slot, None);
    }
}

/// Puts a `time_row` result on a task date. A time needs a date: adding one
/// to no date sets the date to today.
fn time_picked(
    date: &mut Option<NaiveDate>,
    slot: &mut Option<Option<NaiveTime>>,
    new: Option<NaiveTime>,
) {
    if new.is_some() && date.is_none() {
        *date = Some(today());
    }
    set_slot(slot, new);
}

/// `date_menu` for a task's date, with `time_row` under it. Returns true
/// when the date or the time changed.
pub fn date_time_menu(
    ui: &mut Ui,
    t: &Theme,
    id: egui::Id,
    date: &mut Option<NaiveDate>,
    time: &mut Option<Option<NaiveTime>>,
) -> bool {
    let before = *date;
    let mut changed = date_menu_inner(ui, t, id, date, true);
    if changed {
        date_picked(before, *date, time);
    }
    ui.separator();
    if let Some(nt) = time_row(ui, t, id.with("time"), date.and(time.flatten())) {
        time_picked(date, time, nt);
        changed = true;
    }
    changed
}

/// Start date / Due date picker with times and a repeat choice, as in the
/// original's date popup. Returns true when anything changed.
pub fn dates_menu(ui: &mut Ui, t: &Theme, id: egui::Id, task: &mut Task) -> bool {
    let mut which: u8 = ui.data(|d| d.get_temp(id.with("which"))).unwrap_or(1);
    let mut changed = false;
    let (start_txt, due_txt) = (fmt_start(task), fmt_due(task));
    ui.horizontal(|ui| {
        for (k, label, v) in [(0u8, "Start date", start_txt), (1u8, "Due date", due_txt)] {
            let txt = match v {
                Some(d) => format!("{}: {}", tr(label), d),
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
    let (target, slot) = if which == 0 {
        (&mut task.start, &mut task.start_time)
    } else {
        (&mut task.due, &mut task.due_time)
    };
    let before = *target;
    let mut tmp = before;
    let picked = date_menu_inner(ui, t, id.with(which), &mut tmp, false);
    if picked {
        *target = tmp;
        date_picked(before, tmp, slot);
        changed = true;
        if which == 0 && tmp.is_some() {
            // after the start date, pick the due date
            ui.data_mut(|d| d.insert_temp(id.with("which"), 1u8));
        } else {
            close_popup(ui);
        }
    }
    ui.separator();
    if let Some(nt) = time_row(ui, t, id.with(("time", which)), target.and(slot.flatten())) {
        time_picked(target, slot, nt);
        changed = true;
    }
    ui.separator();
    let recur = &mut task.recur;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tr("Set Recurring")).color(t.text2));
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

/// Where `theme::apply` leaves the accent colour for widgets that are not
/// handed the theme (the check box).
pub const ACCENT_ID: &str = "__wb_accent";

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
                    Stroke::new(1.8, fade(Color32::WHITE)),
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
                    Stroke::new(1.2, fade(line)),
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
            Stroke::new(1.5, hover_line(ui)),
            egui::StrokeKind::Outside,
        );
    }
}
