// Timeline view: tasks as bars on a time axis, in swimlanes by assignee (or
// status, priority or list). A task with several assignees shows in each of
// their lanes. Bars in a lane share rows when they don't overlap on screen,
// counting a name written beside a narrow bar.
// Tasks without dates wait in a tray above the axis. The axis, the zoom and
// the bar dragging are the Gantt's (src/app/timebar.rs).
//
// Nothing here is saved in the workspace: the lane choice and zoom last for
// the session, like the Gantt's zoom. The view's name only reaches data.json
// through `hidden_views`, a list of plain strings older apps already accept.

use super::timebar::{self, bar_range, DayScale};
use super::*;

/// The groupings the Timeline offers for its lanes.
pub(super) const LANES: [GroupBy; 4] = [
    GroupBy::Assignee,
    GroupBy::Status,
    GroupBy::Priority,
    GroupBy::List,
];

pub(super) struct TimelineState {
    pub by: GroupBy,
    pub zoom: timebar::Zoom,
    /// Scroll back to today on the next frame.
    pub jump: bool,
    /// The tray of tasks without dates is open.
    pub tray: bool,
    /// Bars out of sight that leave the "‹ date" / "date ›" at the edge to
    /// another in their row (worked out each frame while the bars are drawn)
    pub quiet_edge: std::collections::HashSet<u64>,
}

impl Default for TimelineState {
    fn default() -> Self {
        TimelineState {
            by: GroupBy::Assignee,
            zoom: Default::default(),
            jump: true,
            tray: false,
            quiet_edge: Default::default(),
        }
    }
}

/// One swimlane: its group and the bars in it.
pub(super) struct Lane {
    pub group: Group,
    /// (task, first and last day, row in the lane)
    pub bars: Vec<(u64, (NaiveDate, NaiveDate), usize)>,
    /// Rows the lane needs (at least one, so an empty lane still shows).
    pub rows: usize,
}

/// A row for each range so that ranges sharing a row don't overlap: each
/// goes, in order of first day, into the first row free by then. Returns
/// the row of each range (in the order given) and the number of rows.
pub(super) fn pack_rows(ranges: &[(NaiveDate, NaiveDate)]) -> (Vec<usize>, usize) {
    let mut order: Vec<usize> = (0..ranges.len()).collect();
    order.sort_by_key(|&i| ranges[i]);
    let mut row_end: Vec<NaiveDate> = vec![];
    let mut rows = vec![0; ranges.len()];
    for i in order {
        let (s, e) = ranges[i];
        let k = match row_end.iter().position(|&end| end < s) {
            Some(k) => k,
            None => {
                row_end.push(e);
                row_end.len() - 1
            }
        };
        row_end[k] = e;
        rows[i] = k;
    }
    (rows, row_end.len())
}

impl App {
    /// The lanes for `tasks` grouped by `by`, holding the tasks that have
    /// dates. By assignee there is always an Unassigned lane. Rows here
    /// follow dates alone; the view packs them with `timeline_lanes_with`.
    #[cfg(test)]
    pub(super) fn timeline_lanes(&self, tasks: &[&Task], by: GroupBy) -> Vec<Lane> {
        self.timeline_lanes_with(tasks, by, |_, r| r.1)
    }

    /// `timeline_lanes`, packing rows by the last day each bar takes up on
    /// screen (`reach`), so a name drawn beside a bar keeps its row clear.
    pub(super) fn timeline_lanes_with(
        &self,
        tasks: &[&Task],
        by: GroupBy,
        reach: impl Fn(&Task, (NaiveDate, NaiveDate)) -> NaiveDate,
    ) -> Vec<Lane> {
        let dated: Vec<&Task> = tasks
            .iter()
            .copied()
            .filter(|x| bar_range(x.start, x.due).is_some())
            .collect();
        let mut groups = self.groups_by(by, &dated, false);
        if by == GroupBy::Assignee && !groups.iter().any(|g| g.key == "a:none") {
            if let Some(g) = self
                .groups_by(by, &[], true)
                .into_iter()
                .find(|g| g.key == "a:none")
            {
                groups.push(g);
            }
        }
        // looked up by id for every bar (not a search of all of them each)
        let by_id: std::collections::HashMap<u64, &Task> =
            dated.iter().rev().map(|x| (x.id, *x)).collect();
        groups
            .into_iter()
            .map(|group| {
                let mut taken = vec![];
                let ranges: Vec<(u64, (NaiveDate, NaiveDate))> = group
                    .tasks
                    .iter()
                    .filter_map(|id| by_id.get(id).copied())
                    .filter_map(|x| {
                        let r = bar_range(x.start, x.due)?;
                        taken.push((r.0, reach(x, r).max(r.1)));
                        Some((x.id, r))
                    })
                    .collect();
                let (rows, n) = pack_rows(&taken);
                Lane {
                    group,
                    bars: ranges
                        .into_iter()
                        .zip(rows)
                        .map(|((id, r), row)| (id, r, row))
                        .collect(),
                    rows: n.max(1),
                }
            })
            .collect()
    }

    pub(super) fn timeline_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let td = today();
        let sc = DayScale::new(self.timeline.zoom, td);
        let font = FontId::proportional(timebar::BAR_NAME_SIZE);
        // the lanes and the tray hold ids: each bar looks its task up when
        // it's drawn, so the tasks aren't copied on every frame
        let (lanes, undated) = {
            let tasks = self.filtered();
            let lanes = self.timeline_lanes_with(&tasks, self.timeline.by, |x, r| {
                let w = ui.fonts(|f| {
                    f.layout_no_wrap(x.name.clone(), font.clone(), Color32::WHITE)
                        .size()
                        .x
                });
                sc.reach(r, w.min(timebar::BESIDE_NAME_MAX))
            });
            let undated: Vec<u64> = tasks
                .iter()
                .filter(|x| bar_range(x.start, x.due).is_none())
                .map(|x| x.id)
                .collect();
            (lanes, undated)
        };
        // wraps in a small window: "without dates" ran over the lane choices
        ui.horizontal_wrapped(|ui| {
            if timebar::zoom_buttons(ui, &mut self.timeline.zoom) {
                self.timeline.jump = true;
            }
            ui.separator();
            ui.label(RichText::new(tr("Lanes")).small().color(t.text3));
            for by in LANES {
                let r = ui.selectable_label(self.timeline.by == by, tr(by.label()));
                name_it(&r, &format!("Lanes by {}", by.label()));
                if r.clicked() {
                    self.timeline.by = by;
                }
            }
            if !undated.is_empty() {
                let label = format!("\u{25B8} {}", trf!("{} without dates", undated.len()));
                let w = ui
                    .painter()
                    .layout_no_wrap(label, egui::TextStyle::Button.resolve(ui.style()), t.text)
                    .size()
                    .x;
                crate::widgets::wrap_unless_room(ui, w + 24.0);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !undated.is_empty() {
                    let arrow = if self.timeline.tray {
                        "\u{25BE}"
                    } else {
                        "\u{25B8}"
                    };
                    let r = ui
                        .selectable_label(
                            self.timeline.tray,
                            format!("{} {}", arrow, trf!("{} without dates", undated.len())),
                        )
                        .on_hover_text(tr("Show the tasks that have no start or due date"));
                    name_it(&r, "Tasks without dates");
                    if r.clicked() {
                        self.timeline.tray = !self.timeline.tray;
                    }
                }
            });
        });
        if self.timeline.tray && !undated.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for &tid in &undated {
                    let Some((name, st)) = self
                        .task(tid)
                        .map(|x| (x.name.clone(), self.data.status_of(x)))
                    else {
                        continue;
                    };
                    let g = ui.painter().layout_no_wrap(
                        name.clone(),
                        FontId::proportional(12.5),
                        t.text,
                    );
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(g.size().x + 34.0, 26.0), Sense::click());
                    let fill = if resp.hovered() { t.hover } else { t.column_bg };
                    ui.painter().rect_filled(r, 6.0, fill);
                    ui.painter().rect_stroke(
                        r,
                        6.0,
                        Stroke::new(1.0_f32, t.border),
                        egui::StrokeKind::Inside,
                    );
                    paint_status_circle(
                        ui.painter(),
                        pos2(r.left() + 13.0, r.center().y),
                        5.5,
                        &st,
                    );
                    ui.painter().galley(
                        pos2(r.left() + 24.0, r.center().y - g.size().y / 2.0),
                        g,
                        t.text,
                    );
                    self.task_hit(&resp, tid, &format!("Undated {}", name));
                }
            });
        }
        ui.add_space(4.0);
        let rh = 34.0;
        let pad = 4.0;
        let head = 44.0;
        let label_w = 200.0;
        let lane_h = |l: &Lane| l.rows as f32 * rh + pad * 2.0;
        let body_h: f32 = lanes.iter().map(lane_h).sum();
        // fill the view so the horizontal scrollbar sits at the bottom
        let total_h = (head + body_h).max(ui.available_height() - 24.0);
        egui::ScrollArea::vertical()
            .id_salt("timeline_v")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    // lane names
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.set_width(label_w);
                        let (hr, _) = ui.allocate_exact_size(vec2(label_w, head), Sense::hover());
                        ui.painter().text(
                            hr.left_center() + vec2(8.0, 0.0),
                            Align2::LEFT_CENTER,
                            tr(self.timeline.by.label()),
                            FontId::proportional(12.0),
                            t.text3,
                        );
                        ui.painter().hline(
                            hr.x_range(),
                            hr.bottom(),
                            Stroke::new(1.0_f32, t.border),
                        );
                        for lane in &lanes {
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(label_w, lane_h(lane)), Sense::hover());
                            name_it(&resp, &format!("Lane {}", lane.group.label));
                            let cy = r.top() + pad + rh / 2.0;
                            ui.painter().circle_filled(
                                pos2(r.left() + 14.0, cy),
                                5.0,
                                lane.group.color,
                            );
                            let name = ui.painter().layout_no_wrap(
                                lane.group.label.clone(),
                                FontId::proportional(12.5),
                                t.text,
                            );
                            let nw = name.size().x.min(label_w - 70.0);
                            ui.painter().with_clip_rect(r.shrink(2.0)).galley(
                                pos2(r.left() + 26.0, cy - name.size().y / 2.0),
                                name,
                                t.text,
                            );
                            ui.painter().text(
                                pos2(r.left() + 34.0 + nw, cy),
                                Align2::LEFT_CENTER,
                                lane.bars.len().to_string(),
                                FontId::proportional(11.5),
                                t.text3,
                            );
                            ui.painter().hline(
                                r.x_range(),
                                r.bottom(),
                                Stroke::new(1.0_f32, t.border),
                            );
                        }
                        if lanes.is_empty() {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(tr("No tasks with dates in this view yet"))
                                    .small()
                                    .color(t.text3),
                            );
                        }
                    });
                    let vr = ui.available_rect_before_wrap();
                    ui.painter().vline(
                        vr.left(),
                        vr.top()..=vr.top() + head + body_h,
                        Stroke::new(1.0_f32, t.border),
                    );
                    let jump = std::mem::take(&mut self.timeline.jump);
                    let mut sa = egui::ScrollArea::horizontal()
                        .id_salt(("timeline_h", self.timeline.zoom))
                        .auto_shrink([false, true]);
                    if jump {
                        sa = sa.horizontal_scroll_offset(sc.today_offset(td, vr.width()));
                    }
                    sa.show(ui, |ui| {
                        let (area, _) =
                            ui.allocate_exact_size(vec2(sc.width(), total_h), Sense::hover());
                        let p = ui.painter().clone();
                        timebar::paint_axis(&p, &t, area, head, &sc, td);
                        // a row holds several bars: one "‹ date" at each edge,
                        // for the nearest bar out of sight, and none where a bar
                        // in view already reaches that edge (they were drawn over
                        // each other and over the bar's name)
                        let view = ui.clip_rect();
                        let mut quiet = std::collections::HashSet::new();
                        for lane in &lanes {
                            let rows: std::collections::BTreeSet<usize> =
                                lane.bars.iter().map(|b| b.2).collect();
                            for row in rows {
                                let bars: Vec<(u64, f32, f32, NaiveDate, NaiveDate)> = lane
                                    .bars
                                    .iter()
                                    .filter(|b| b.2 == row)
                                    .map(|(tid, (s, e), _)| {
                                        let x0 = sc.x_of(area.left(), *s) + 1.0;
                                        let x1 =
                                            (sc.x_of(area.left(), *e) + sc.cw - 1.0).max(x0 + 8.0);
                                        (*tid, x0, x1, *s, *e)
                                    })
                                    .collect();
                                let shown = |b: &&(u64, f32, f32, NaiveDate, NaiveDate)| {
                                    b.2 >= view.left() && b.1 <= view.right()
                                };
                                let near_left =
                                    bars.iter().filter(shown).any(|b| b.1 < view.left() + 90.0);
                                let near_right = bars
                                    .iter()
                                    .filter(shown)
                                    .any(|b| b.2 + 160.0 > view.right() - 90.0);
                                let left: Vec<_> =
                                    bars.iter().filter(|b| b.2 < view.left()).collect();
                                let keep_l = left.iter().max_by_key(|b| b.4).map(|b| b.0);
                                for b in &left {
                                    if near_left || Some(b.0) != keep_l {
                                        quiet.insert(b.0);
                                    }
                                }
                                let right: Vec<_> =
                                    bars.iter().filter(|b| b.1 > view.right()).collect();
                                let keep_r = right.iter().min_by_key(|b| b.3).map(|b| b.0);
                                for b in &right {
                                    if near_right || Some(b.0) != keep_r {
                                        quiet.insert(b.0);
                                    }
                                }
                            }
                        }
                        self.timeline.quiet_edge = quiet;
                        let mut y = area.top() + head;
                        for lane in &lanes {
                            for (tid, range, row) in &lane.bars {
                                let Some(task) = self.task(*tid).cloned() else {
                                    continue;
                                };
                                let task = &task;
                                let resp = self.date_bar(
                                    ui,
                                    &p,
                                    &sc,
                                    area.left(),
                                    y + pad + rh * *row as f32,
                                    rh,
                                    task,
                                    *range,
                                    &lane.group.key,
                                );
                                let tid = *tid;
                                resp.context_menu(|ui| self.task_menu(ui, tid));
                            }
                            y += lane_h(lane);
                            p.hline(area.x_range(), y, Stroke::new(1.0_f32, t.border));
                        }
                        // the Gantt view draws its bars with the same code, one a row
                        self.timeline.quiet_edge.clear();
                    });
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::app;

    fn d(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 3, 2).unwrap() + Duration::days(n)
    }

    fn id_of(a: &App, name: &str) -> u64 {
        a.data.tasks.iter().find(|t| t.name == name).unwrap().id
    }

    /// (lane key, task ids) for the current location's filtered tasks.
    fn lanes_of(a: &App, by: GroupBy) -> Vec<(String, Vec<u64>)> {
        let tasks = a.filtered();
        a.timeline_lanes(&tasks, by)
            .into_iter()
            .map(|l| (l.group.key, l.bars.iter().map(|b| b.0).collect()))
            .collect()
    }

    #[test]
    fn bar_range_covers_start_to_due_or_one_day() {
        assert_eq!(bar_range(Some(d(0)), Some(d(3))), Some((d(0), d(3))));
        assert_eq!(bar_range(None, Some(d(3))), Some((d(3), d(3))));
        assert_eq!(bar_range(Some(d(1)), None), Some((d(1), d(1))));
        // a start after the due date still draws forwards
        assert_eq!(bar_range(Some(d(5)), Some(d(2))), Some((d(2), d(5))));
        assert_eq!(bar_range(None, None), None);
    }

    #[test]
    fn pack_rows_shares_rows_only_without_overlap() {
        assert_eq!(pack_rows(&[]), (vec![], 0));
        // 0-2 and 3-4 fit one row; 1-5 overlaps both
        let (rows, n) = pack_rows(&[(d(0), d(2)), (d(1), d(5)), (d(3), d(4))]);
        assert_eq!(rows, vec![0, 1, 0]);
        assert_eq!(n, 2);
        // a bar ending on the day another starts shares that day: new row
        let (rows, n) = pack_rows(&[(d(0), d(2)), (d(2), d(2))]);
        assert_eq!((rows, n), (vec![0, 1], 2));
        // the order given doesn't matter, rows follow first days
        let (rows, n) = pack_rows(&[(d(6), d(7)), (d(0), d(1)), (d(3), d(4))]);
        assert_eq!((rows, n), (vec![0, 0, 0], 1));
    }

    #[test]
    fn a_name_beside_a_narrow_bar_counts_as_taken() {
        // Day scale: 30 points a day
        let sc = DayScale::new(crate::app::timebar::Zoom::Day, d(0));
        // a three-day bar (88 points) holds its name: it reaches its due date
        assert_eq!(sc.reach((d(0), d(2)), 300.0), d(2));
        // a one-day bar writes a 100-point name to its right: 5 + 100 + 6
        // points cover 4 more day columns
        assert_eq!(sc.reach((d(0), d(0)), 100.0), d(4));
        // at the Month scale (4 points a day) the same name covers 28 days
        let sc = DayScale::new(crate::app::timebar::Zoom::Month, d(0));
        assert_eq!(sc.reach((d(0), d(0)), 100.0), d(28));
    }

    #[test]
    fn bars_next_to_each_other_split_rows_when_a_name_would_overlap() {
        let mut a = app();
        let ids: Vec<u64> = a.filtered().iter().map(|t| t.id).take(2).collect();
        for (n, id) in ids.iter().enumerate() {
            let t = a.data.tasks.iter_mut().find(|t| t.id == *id).unwrap();
            t.assignees.clear();
            t.start = None;
            t.due = Some(d(n as i64));
        }
        let tasks = a.filtered();
        let unassigned = |lanes: Vec<Lane>| lanes.into_iter().last().unwrap();
        // by dates alone the two one-day bars share a row
        let lane = unassigned(a.timeline_lanes(&tasks, GroupBy::Assignee));
        let rows: Vec<usize> = lane
            .bars
            .iter()
            .filter(|b| ids.contains(&b.0))
            .map(|b| b.2)
            .collect();
        assert_eq!(rows[0], rows[1]);
        // with the first bar's name written beside it, they don't
        let sc = DayScale::new(crate::app::timebar::Zoom::Day, d(0));
        let lane =
            unassigned(a.timeline_lanes_with(&tasks, GroupBy::Assignee, |_, r| sc.reach(r, 100.0)));
        let rows: Vec<usize> = lane
            .bars
            .iter()
            .filter(|b| ids.contains(&b.0))
            .map(|b| b.2)
            .collect();
        assert_ne!(rows[0], rows[1]);
        assert_eq!(lane.rows, lane.bars.iter().map(|b| b.2).max().unwrap() + 1);
    }

    #[test]
    fn lanes_by_assignee_repeat_shared_tasks_and_keep_unassigned() {
        let a = app();
        let lanes = lanes_of(&a, GroupBy::Assignee);
        let keys: Vec<&str> = lanes.iter().map(|l| l.0.as_str()).collect();
        // Website Redesign: Jordan only has a task without dates, so no lane
        assert_eq!(keys, vec!["a:1", "a:2", "a:3", "a:none"]);
        let design = id_of(&a, "Design new homepage layout");
        assert!(lanes[0].1.contains(&design) && lanes[1].1.contains(&design));
        let analytics = id_of(&a, "Set up analytics dashboard");
        assert_eq!(lanes[3].1, vec![analytics]);
        let undated = id_of(&a, "Collect feedback from beta users");
        assert!(lanes.iter().all(|l| !l.1.contains(&undated)));
    }

    #[test]
    fn unassigned_lane_shows_even_when_empty() {
        let mut a = app();
        for t in a.data.tasks.iter_mut() {
            if t.assignees.is_empty() {
                t.assignees.push(2);
            }
        }
        let lanes = lanes_of(&a, GroupBy::Assignee);
        let last = lanes.last().unwrap();
        assert_eq!(last.0, "a:none");
        assert!(last.1.is_empty());
        let tasks = a.filtered();
        let l = a.timeline_lanes(&tasks, GroupBy::Assignee);
        assert_eq!(l.last().unwrap().rows, 1, "an empty lane keeps one row");
    }

    #[test]
    fn lanes_by_status_priority_and_list() {
        let mut a = app();
        let keys =
            |a: &App, by| -> Vec<String> { lanes_of(a, by).into_iter().map(|l| l.0).collect() };
        assert_eq!(keys(&a, GroupBy::Status), vec!["s:to do", "s:in progress"]);
        assert_eq!(
            keys(&a, GroupBy::Priority),
            vec!["p:1", "p:2", "p:3", "p:4"]
        );
        a.loc = Location::Everything;
        let lists = keys(&a, GroupBy::List);
        assert!(lists.len() >= 3, "{:?}", lists);
        assert!(lists.iter().all(|k| k.starts_with("l:")));
        // every dated task sits in exactly one list lane
        let n: usize = lanes_of(&a, GroupBy::List).iter().map(|l| l.1.len()).sum();
        let dated = a
            .filtered()
            .iter()
            .filter(|t| bar_range(t.start, t.due).is_some())
            .count();
        assert_eq!(n, dated);
    }

    #[test]
    fn overlapping_bars_in_a_lane_take_separate_rows() {
        let a = app();
        let tasks = a.filtered();
        let lanes = a.timeline_lanes(&tasks, GroupBy::Assignee);
        // Me: Design new homepage layout (this Monday to +2) and Fix
        // navigation on mobile (-3 to -1) overlap on Mondays to Wednesdays
        let me = &lanes[0];
        let rows: HashSet<usize> = me.bars.iter().map(|b| b.2).collect();
        assert_eq!(me.rows, rows.len());
        for (i, x) in me.bars.iter().enumerate() {
            for y in &me.bars[i + 1..] {
                if x.2 == y.2 {
                    assert!(x.1 .1 < y.1 .0 || y.1 .1 < x.1 .0, "no overlap in a row");
                }
            }
        }
    }
}
