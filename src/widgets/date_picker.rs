// The date picker with its month grid and time row, and the menus built
// on it (due date, start and due dates).

use super::*;

/// How wide a date menu is: the popups that show one open at this width, and
/// the quick options and the month fill it exactly.
pub const DATE_MENU_W: f32 = 380.0;
/// A square of the month grid. Every row of the menu is this tall, so the
/// quick options line up with the rows of the month beside them.
const CELL: f32 = 32.0;
/// The gap between the quick options and the month, with a line in its middle.
const GAP: f32 = 17.0;
/// The column of quick options: what the month leaves of the menu.
const OPTIONS_W: f32 = DATE_MENU_W - GAP - 7.0 * CELL;

/// How wide a date menu is in this window: `DATE_MENU_W`, or what a narrow
/// window leaves inside its margins (a 360 pt window cut the menu's left
/// side off). Narrower than `DATE_MENU_W`, the quick options go above the
/// month instead of beside it.
pub fn date_menu_w(ui: &Ui) -> f32 {
    let margin = egui::Frame::popup(ui.style()).total_margin().sum().x;
    DATE_MENU_W
        .min(ui.ctx().screen_rect().width() - 16.0 - margin)
        .max(7.0 * CELL)
}
/// The circle behind a day (hovered or picked), and the arrows' squares.
const DAY_R: f32 = 13.0;
/// Text in a row starts this far in from the row's hover fill.
const ROW_PAD: f32 = 10.0;

/// Quick options plus a month calendar. Returns true when the date changed.
pub fn date_menu(ui: &mut Ui, t: &Theme, id: egui::Id, current: &mut Option<NaiveDate>) -> bool {
    date_menu_inner(ui, t, id, current, true)
}

/// A full-width row of a date menu: the hover fill spans the row and the text
/// sits inside it. Returns its response (named `label`).
fn menu_line(ui: &mut Ui, rect: Rect, id: egui::Id, label: &str, color: Color32) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    name_it(&resp, label);
    if resp.hovered() || resp.is_pointer_button_down_on() {
        let fill = if resp.is_pointer_button_down_on() {
            hover_fill(ui).gamma_multiply(1.5)
        } else {
            hover_fill(ui)
        };
        // a little shorter than the row, as tall as a menu row
        let r = rect.shrink2(vec2(0.0, ((rect.height() - 28.0) / 2.0).max(0.0)));
        ui.painter().rect_filled(r, 6.0, fill);
    }
    ui.painter().text(
        rect.left_center() + vec2(ROW_PAD, 0.0),
        Align2::LEFT_CENTER,
        tr(label),
        FontId::proportional(14.0),
        color,
    );
    resp
}

/// A square button with a chevron, for the previous or next month.
fn month_arrow(ui: &mut Ui, t: &Theme, rect: Rect, id: egui::Id, name: &str, left: bool) -> bool {
    let resp = ui.interact(rect, id, Sense::click());
    name_it(&resp, name);
    let hovered = resp.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 6.0, hover_fill(ui));
    }
    ui.painter().rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0_f32, if hovered { hover_line(ui) } else { t.border }),
        egui::StrokeKind::Inside,
    );
    // the chevron of the app's line icons, centred in the square
    let c = rect.center();
    let dx = if left { 2.5 } else { -2.5 };
    ui.painter().add(egui::Shape::line(
        vec![c + vec2(dx, -5.0), c + vec2(-dx, 0.0), c + vec2(dx, 5.0)],
        Stroke::new(1.5_f32, t.text2),
    ));
    resp.clicked()
}

fn date_menu_inner(
    ui: &mut Ui,
    t: &Theme,
    id: egui::Id,
    current: &mut Option<NaiveDate>,
    close: bool,
) -> bool {
    let base = current.unwrap_or_else(today);
    let mut month: NaiveDate = ui
        .data(|d| d.get_temp(id))
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(base.year(), base.month(), 1).unwrap());
    let td = today();
    let w = date_menu_w(ui);
    // a narrow window: the quick options in two columns above the month
    let narrow = w < DATE_MENU_W - 0.5;
    let opts_h = if narrow { CELL * 4.0 + GAP } else { 0.0 };
    // eight rows: the month's name, the weekdays and six weeks. The quick
    // options take a row each beside them, Clear the last, so the menu is
    // as tall with or without a date and nothing is left empty below
    let (rect, _) = ui.allocate_exact_size(vec2(w, opts_h + CELL * 8.0), Sense::hover());
    let mut picked: Option<Option<NaiveDate>> = None;
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
    let option_row = |i: usize| {
        if narrow {
            let half = w / 2.0;
            Rect::from_min_size(
                rect.min + vec2(half * (i % 2) as f32, CELL * (i / 2) as f32),
                vec2(half, CELL),
            )
        } else {
            Rect::from_min_size(rect.min + vec2(0.0, CELL * i as f32), vec2(OPTIONS_W, CELL))
        }
    };
    for (i, (label, d)) in opts.into_iter().enumerate() {
        if menu_line(ui, option_row(i), id.with(("opt", i)), label, t.text).clicked() {
            picked = Some(Some(d));
        }
    }
    let line = ui.visuals().widgets.noninteractive.bg_stroke;
    if current.is_some() {
        let r = option_row(7);
        if !narrow {
            ui.painter().hline(r.x_range(), r.top(), line);
        }
        if menu_line(ui, r, id.with("clear"), "Clear", t.danger).clicked() {
            picked = Some(None);
        }
    }
    if narrow {
        ui.painter()
            .hline(rect.x_range(), rect.top() + CELL * 4.0 + GAP / 2.0, line);
    } else {
        ui.painter()
            .vline(rect.left() + OPTIONS_W + GAP / 2.0, rect.y_range(), line);
    }

    // the month: beside the options at the right, or under them, centred
    let grid = if narrow {
        let left = rect.left() + (w - 7.0 * CELL) / 2.0;
        Rect::from_min_size(
            pos2(left, rect.top() + opts_h),
            vec2(7.0 * CELL, CELL * 8.0),
        )
    } else {
        Rect::from_min_max(pos2(rect.right() - 7.0 * CELL, rect.top()), rect.max)
    };
    let col = |c: usize| grid.left() + CELL * (c as f32 + 0.5);
    let row = |r: usize| grid.top() + CELL * (r as f32 + 0.5);
    // the month's name starts where the first day's circle does; the arrows
    // stand over the last two columns, their right edge on the last circle's
    ui.painter().text(
        pos2(col(0) - DAY_R, row(0)),
        Align2::LEFT_CENTER,
        month.text("%B %Y"),
        FontId::proportional(14.0),
        t.text,
    );
    let square =
        |c: usize| Rect::from_center_size(pos2(col(c), row(0)), egui::Vec2::splat(DAY_R * 2.0));
    if month_arrow(ui, t, square(5), id.with("prev"), "Previous month", true) {
        month = add_months(month, -1);
    }
    if month_arrow(ui, t, square(6), id.with("next"), "Next month", false) {
        month = add_months(month, 1);
    }
    for (i, d) in ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
        .iter()
        .enumerate()
    {
        ui.painter().text(
            pos2(col(i), row(1)),
            Align2::CENTER_CENTER,
            tr(d),
            FontId::proportional(12.0),
            t.text3,
        );
    }
    let start = crate::model::sunday_week_start(month);
    for i in 0..42usize {
        let d = start + Duration::days(i as i64);
        let c = pos2(col(i % 7), row(2 + i / 7));
        let cell = Rect::from_center_size(c, egui::Vec2::splat(CELL));
        let resp = ui.interact(cell, id.with(("day", i)), Sense::click());
        name_it(&resp, &format!("Day {}", d.format("%Y-%m-%d")));
        let sel = Some(d) == *current;
        if sel {
            ui.painter().circle_filled(c, DAY_R, t.primary);
        } else if resp.hovered() {
            ui.painter().circle_filled(c, DAY_R, hover_fill(ui));
        }
        let color = if sel {
            t.on_primary
        } else if d.month() != month.month() {
            t.text3.gamma_multiply(0.6)
        } else if d == td {
            t.danger
        } else {
            t.text
        };
        ui.painter().text(
            c,
            Align2::CENTER_CENTER,
            d.day().to_string(),
            FontId::proportional(13.0),
            color,
        );
        if resp.clicked() {
            picked = Some(Some(d));
        }
    }
    ui.data_mut(|d| d.insert_temp(id, month));
    let Some(v) = picked else { return false };
    *current = v;
    if close {
        close_popup(ui);
    }
    true
}

pub fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + n;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap()
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
    let Some(cur) = time else {
        let (rect, _) = ui.allocate_exact_size(vec2(date_menu_w(ui), 28.0), Sense::hover());
        if menu_line(ui, rect, id.with("add"), "Add time", t.text2).clicked() {
            out = Some(NaiveTime::from_hms_opt(9, 0, 0));
        }
        return out;
    };
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
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
        // the two tabs share the menu's width
        let tab_w = (date_menu_w(ui) - ui.spacing().item_spacing.x) / 2.0;
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
                .stroke(Stroke::new(1.0_f32, t.border))
                .min_size(vec2(tab_w, 28.0)),
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
    // the label over the choices: beside them the row was wider than the
    // month and left it short of the menu's right edge
    ui.horizontal(|ui| {
        ui.add_space(ROW_PAD);
        ui.label(egui::RichText::new(tr("Set Recurring")).color(t.text2));
    });
    ui.horizontal(|ui| {
        let gap = ui.spacing().item_spacing.x;
        let w = (date_menu_w(ui) - gap * (RECUR.len() - 1) as f32) / RECUR.len() as f32;
        // plain buttons: a combo box would close this popup
        for (v, label) in RECUR {
            let sel = recur.as_deref().unwrap_or("") == v;
            let r = ui.add_sized([w, 28.0], egui::SelectableLabel::new(sel, tr(label)));
            name_it(&r, label);
            if r.clicked() && !sel {
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
