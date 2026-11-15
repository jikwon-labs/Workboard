// The time axis the Gantt and Timeline views share: the Day / Week / Month
// scales and the buttons that pick one, the day grid with its labels and the
// today line, and task bars that drag to move their dates or drag an end to
// change one date. Dates only; times of day are left as they are.

use super::*;

/// Where each day sits on the axis at one zoom level (0 Day, 1 Week, 2 Month).
#[derive(Clone, Copy)]
pub(super) struct DayScale {
    pub zoom: u8,
    /// The first day drawn.
    pub start: NaiveDate,
    /// How many days are drawn.
    pub days: i64,
    /// Width of one day in points.
    pub cw: f32,
}

impl DayScale {
    pub fn new(zoom: u8, today: NaiveDate) -> Self {
        // day columns get narrower as you zoom out
        let (cw, days, back) = match zoom {
            0 => (30.0, 70, 14),
            1 => (12.0, 182, 35),
            _ => (4.0, 548, 120),
        };
        let start = today - Duration::days(today.weekday().num_days_from_monday() as i64 + back);
        DayScale {
            zoom,
            start,
            days,
            cw,
        }
    }

    pub fn width(&self) -> f32 {
        self.cw * self.days as f32
    }

    /// Left edge of day `d`, `left` being the axis' left edge.
    pub fn x_of(&self, left: f32, d: NaiveDate) -> f32 {
        left + self.cw * (d - self.start).num_days() as f32
    }

    pub fn day_at(&self, left: f32, x: f32) -> NaiveDate {
        self.start + Duration::days(((x - left) / self.cw).floor() as i64)
    }

    /// Horizontal scroll offset that puts today a little left of the middle.
    pub fn today_offset(&self, today: NaiveDate, view_w: f32) -> f32 {
        (self.x_of(0.0, today) - view_w * 0.3).max(0.0)
    }

    /// Width of the bar drawn for the days `s..=e`.
    pub fn bar_w(&self, (s, e): (NaiveDate, NaiveDate)) -> f32 {
        (self.cw * ((e - s).num_days() + 1) as f32 - 2.0).max(8.0)
    }

    /// The last day a bar for `range` takes up on screen. A bar too narrow
    /// for its name writes the name to its right (`name_w` points wide), so
    /// it reaches past its due date by the days the name and a gap cover.
    pub fn reach(&self, range: (NaiveDate, NaiveDate), name_w: f32) -> NaiveDate {
        if self.bar_w(range) > NAME_INSIDE_W {
            return range.1;
        }
        // the name starts 5 points past the bar's day column; keep 6 clear
        let past = ((5.0 + name_w + 6.0) / self.cw).ceil() as i64;
        range.1 + Duration::days(past)
    }
}

/// Bars wider than this show the task name inside; narrower ones write it
/// to the right of the bar.
pub(super) const NAME_INSIDE_W: f32 = 60.0;

/// Font size of the task name on a bar.
pub(super) const BAR_NAME_SIZE: f32 = 11.5;

/// A name written beside a narrow bar is shortened to at most this wide.
pub(super) const BESIDE_NAME_MAX: f32 = 200.0;

/// The days a task's bar covers: start to due, or a single day when only one
/// of them is set. None when it has no dates.
pub(super) fn bar_range(
    start: Option<NaiveDate>,
    due: Option<NaiveDate>,
) -> Option<(NaiveDate, NaiveDate)> {
    match (start, due) {
        (Some(s), Some(e)) => Some((s.min(e), e.max(s))),
        (None, Some(e)) => Some((e, e)),
        (Some(s), None) => Some((s, s)),
        _ => None,
    }
}

/// The Today button and the zoom buttons, drawn into the current row.
/// Returns true when the view should scroll back to today.
pub(super) fn zoom_buttons(ui: &mut Ui, zoom: &mut u8) -> bool {
    let mut jump = false;
    if ui.button(tr("Today")).clicked() {
        jump = true;
    }
    ui.separator();
    for (z, label) in [(0u8, "Day"), (1, "Week"), (2, "Month")] {
        if ui.selectable_label(*zoom == z, tr(label)).clicked() && *zoom != z {
            *zoom = z;
            jump = true;
        }
    }
    if ui
        .small_button("\u{2212}")
        .on_hover_text(tr("Zoom out"))
        .clicked()
        && *zoom < 2
    {
        *zoom += 1;
        jump = true;
    }
    if ui.small_button("+").on_hover_text(tr("Zoom in")).clicked() && *zoom > 0 {
        *zoom -= 1;
        jump = true;
    }
    jump
}

/// The day grid: weekend shading, week or month lines with their labels,
/// day numbers at the Day scale, the header line and the today line.
pub(super) fn paint_axis(
    p: &egui::Painter,
    t: &Theme,
    area: Rect,
    head: f32,
    sc: &DayScale,
    td: NaiveDate,
) {
    let total_h = area.height();
    let cw = sc.cw;
    for i in 0..sc.days {
        let d = sc.start + Duration::days(i);
        let x = area.left() + cw * i as f32;
        let col_r = Rect::from_min_size(pos2(x, area.top() + head), vec2(cw, total_h - head));
        if sc.zoom == 0 && d.weekday().num_days_from_monday() >= 5 {
            p.rect_filled(col_r, 0.0, t.column_bg);
        }
        let tick = match sc.zoom {
            0 | 1 => d.weekday().num_days_from_monday() == 0,
            _ => d.day() == 1,
        };
        if tick {
            p.vline(
                x,
                area.top()..=area.top() + total_h,
                Stroke::new(0.5_f32, t.border),
            );
            let label = if sc.zoom == 2 {
                d.text("%b %Y")
            } else {
                d.text("%b %-d")
            };
            p.text(
                pos2(x + 4.0, area.top() + 12.0),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(11.5),
                t.text2,
            );
        }
        if sc.zoom == 0 {
            let dc = if d == td { Color32::WHITE } else { t.text3 };
            if d == td {
                p.circle_filled(pos2(x + cw / 2.0, area.top() + 31.0), 9.0, t.danger);
            }
            p.text(
                pos2(x + cw / 2.0, area.top() + 31.0),
                Align2::CENTER_CENTER,
                d.day().to_string(),
                FontId::proportional(11.0),
                dc,
            );
        }
    }
    p.hline(
        area.x_range(),
        area.top() + head,
        Stroke::new(1.0_f32, t.border),
    );
    let today_x = sc.x_of(area.left(), td) + cw / 2.0;
    p.vline(
        today_x,
        area.top() + head..=area.top() + total_h,
        Stroke::new(1.5_f32, t.danger),
    );
}

impl App {
    /// One task's bar covering `range`, in the row starting at `y`. Dragging
    /// the bar moves both dates, dragging an end moves that date, a click
    /// opens the task. `salt` keeps ids apart when a task has several bars
    /// on screen. Returns the bar's response (for a context menu).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn date_bar(
        &mut self,
        ui: &mut Ui,
        p: &egui::Painter,
        sc: &DayScale,
        left: f32,
        y: f32,
        rh: f32,
        task: &Task,
        (s, e): (NaiveDate, NaiveDate),
        salt: &str,
    ) -> egui::Response {
        let t = self.t();
        let cw = sc.cw;
        let x0 = sc.x_of(left, s) + 1.0;
        let x1 = sc.x_of(left, e) + cw - 1.0;
        let mut r = Rect::from_min_max(pos2(x0, y + 7.0), pos2(x1.max(x0 + 8.0), y + rh - 7.0));
        // drag a bar to move its dates, or one of its ends to change one date
        let resp = ui.interact(
            r,
            ui.id().with(("bar", salt, task.id)),
            Sense::click_and_drag(),
        );
        name_it(&resp, &format!("Bar {}", task.name));
        let edge_w = 6.0_f32.min(r.width() / 3.0);
        let lh = ui
            .interact(
                Rect::from_min_max(r.min, pos2(r.left() + edge_w, r.bottom())),
                ui.id().with(("barl", salt, task.id)),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        let rhd = ui
            .interact(
                Rect::from_min_max(pos2(r.right() - edge_w, r.top()), r.max),
                ui.id().with(("barr", salt, task.id)),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        name_it(&lh, &format!("Start of {}", task.name));
        name_it(&rhd, &format!("End of {}", task.name));
        let mut shifts = [0i64; 3];
        for (n, h) in [&resp, &lh, &rhd].into_iter().enumerate() {
            let acc_id = h.id.with("acc");
            if h.dragged() {
                let dx = h.drag_delta().x;
                ui.data_mut(|d| *d.get_temp_mut_or_default::<f32>(acc_id) += dx);
            }
            let acc: f32 = ui.data(|d| d.get_temp(acc_id)).unwrap_or(0.0);
            if h.drag_stopped() {
                ui.data_mut(|d| d.remove::<f32>(acc_id));
            }
            shifts[n] = (acc / cw).round() as i64;
        }
        let [shift_days, shift_start, shift_end] = shifts;
        // an end can't be pulled past the other end
        let span = (e - s).num_days();
        let shift_start = shift_start.min(span);
        let shift_end = shift_end.max(-span);
        if resp.dragged() {
            r = r.translate(vec2(shift_days as f32 * cw, 0.0));
        }
        if lh.dragged() {
            r.min.x += shift_start as f32 * cw;
        }
        if rhd.dragged() {
            r.max.x += shift_end as f32 * cw;
        }
        let active = resp.hovered() || resp.dragged() || lh.dragged() || rhd.dragged();
        // overdue bars are red, as the date is in the list and calendar
        let overdue = task.is_overdue() && !self.data.is_closed(task);
        let col = if overdue {
            t.danger
        } else {
            rgb(self.data.status_of(task).color)
        };
        p.rect_filled(
            r,
            5.0,
            if active {
                col
            } else {
                col.gamma_multiply(0.85)
            },
        );
        if lh.hovered() || rhd.hovered() || active {
            for hx in [r.left() + 3.0, r.right() - 3.0] {
                p.vline(
                    hx,
                    r.top() + 5.0..=r.bottom() - 5.0,
                    Stroke::new(2.0_f32, Color32::from_white_alpha(200)),
                );
            }
        }
        let font = FontId::proportional(BAR_NAME_SIZE);
        let view = p.clip_rect();
        if r.right() < view.left() || r.left() > view.right() {
            // the bar is out of sight: say which way and when, at that edge,
            // so the row doesn't look like a task without dates
            let (txt, pos, align) = if r.right() < view.left() {
                (
                    format!("\u{2039} {}", fmt_date(e)),
                    pos2(view.left() + 6.0, r.center().y),
                    Align2::LEFT_CENTER,
                )
            } else {
                (
                    format!("{} \u{203a}", fmt_date(s)),
                    pos2(view.right() - 6.0, r.center().y),
                    Align2::RIGHT_CENTER,
                )
            };
            let g = p.layout_no_wrap(txt.clone(), font.clone(), Color32::WHITE);
            let lr = align.anchor_size(pos, g.size());
            p.rect_filled(lr.expand(3.0), 4.0, t.bg);
            p.text(
                pos,
                align,
                txt,
                font,
                if overdue { t.danger } else { t.text3 },
            );
        } else if r.width() > NAME_INSIDE_W {
            let name = fit_text(p, &task.name, &font, r.width() - 12.0);
            p.with_clip_rect(r).text(
                r.left_center() + vec2(6.0, 0.0),
                Align2::LEFT_CENTER,
                name,
                font,
                Color32::WHITE,
            );
        } else {
            // beside a short bar, shortened, on the page colour so the today
            // line doesn't run through the letters
            let name = fit_text(p, &task.name, &font, BESIDE_NAME_MAX);
            let at = r.right_center() + vec2(6.0, 0.0);
            let g = p.layout_no_wrap(name.clone(), font.clone(), Color32::WHITE);
            p.rect_filled(
                Align2::LEFT_CENTER.anchor_size(at, g.size()).expand(2.0),
                3.0,
                t.bg,
            );
            p.text(at, Align2::LEFT_CENTER, name, font, t.text2);
        }
        if resp.drag_stopped() && shift_days != 0 {
            let mut e2 = task.clone();
            e2.start = e2.start.map(|d| d + Duration::days(shift_days));
            e2.due = e2.due.map(|d| d + Duration::days(shift_days));
            self.actions.push(Action::Replace(e2));
        } else if lh.drag_stopped() && shift_start != 0 {
            let mut e2 = task.clone();
            e2.start = Some(s + Duration::days(shift_start));
            e2.due = Some(e);
            self.actions.push(Action::Replace(e2));
        } else if rhd.drag_stopped() && shift_end != 0 {
            let mut e2 = task.clone();
            e2.start = Some(s);
            e2.due = Some(e + Duration::days(shift_end));
            self.actions.push(Action::Replace(e2));
        } else if resp.clicked() {
            self.actions.push(Action::Open(task.id));
        }
        resp
    }
}
