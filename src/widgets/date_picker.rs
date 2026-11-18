// The date picker with its month grid and time row, and the menus built
// on it (due date, start and due dates).

use super::*;

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
                    t.on_primary
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
                .stroke(Stroke::new(1.0_f32, t.border))
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
