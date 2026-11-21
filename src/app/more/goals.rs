// Goals: their progress, pace and history, and the Goals page (cards or
// rows, with a summary on top and achieved goals below). A goal's own page
// is in goal_page.rs, the New goal and Goal achieved dialogs and the empty
// page in goal_dialogs.rs.
//
// Progress is the average of the targets (a number from start to target,
// done or not, or the share of picked tasks closed). Pace compares it with
// how much of start..due has gone by (`Goal::pace`). Every copy of the app
// keeps each goal's milestones, achieved day and daily progress up to date
// (`goal_watch`); edits made on the goal's page add a line to its history
// (`goal_edit`).

use super::*;
use egui::{Pos2, Response};

/// A goal's progress as a whole percent, rounded, but 100 only once it is
/// all done: 99.5% reads 99, since everything that goes by "100%" (the
/// Achieved shelf, the achieved day, Goal achieved, the 100% milestone)
/// means every target met, as `Goal::pace` and a target's `done` do.
pub(crate) fn pct_of(p: f32) -> u8 {
    let n = (p * 100.0).round().clamp(0.0, 100.0) as u8;
    if p < 1.0 {
        n.min(99)
    } else {
        n
    }
}

/// The largest a goal's number gets either way: far past any count or
/// amount people track, yet exact and printed in full.
pub(crate) const NUM_MAX: f64 = 1e15;

/// A box for one of a goal's numbers (start, target, now). It keeps them
/// finite: "inf", "NaN" or "1e999" typed in leave it as it was, and anything
/// past `NUM_MAX` stops there. The workspace file writes infinity as null,
/// which can't be read back.
pub(crate) fn number_box(v: &mut f64) -> egui::DragValue<'_> {
    egui::DragValue::new(v)
        .speed(1.0)
        .range(-NUM_MAX..=NUM_MAX)
        .custom_parser(parse_number)
}

/// A typed number, None unless it is a finite one. Thousands may be
/// grouped with commas, the way `fmt_num` shows them ("7,200").
pub(crate) fn parse_number(s: &str) -> Option<f64> {
    let s: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .map(|c| if c == '\u{2212}' { '-' } else { c })
        .collect();
    s.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Milestones announced on the way to 100%.
pub(crate) const MILESTONES: [u8; 4] = [25, 50, 75, 100];

/// The highest milestone at or below `pct` (0 below the first).
pub(crate) fn milestone_at(pct: u8) -> u8 {
    MILESTONES
        .iter()
        .rev()
        .copied()
        .find(|m| pct >= *m)
        .unwrap_or(0)
}

/// Colours of a pace pill: (fill, text).
pub(crate) fn pace_colors(p: Pace, t: &Theme) -> (Color32, Color32) {
    let c = |l: [u8; 3], d: [u8; 3]| rgb(if t.dark { d } else { l });
    match p {
        Pace::OnTrack => (
            c([231, 245, 238], [22, 58, 40]),
            c([11, 107, 58], [125, 220, 165]),
        ),
        Pace::Behind => (
            c([255, 243, 220], [70, 50, 12]),
            c([138, 83, 0], [250, 200, 110]),
        ),
        Pace::AtRisk => (
            c([253, 235, 236], [82, 26, 30]),
            c([180, 35, 44], [255, 145, 145]),
        ),
        Pace::Done => (
            c([236, 234, 255], [45, 40, 80]),
            c([75, 63, 196], [202, 203, 255]),
        ),
    }
}

pub(crate) fn pace_label(p: Pace) -> &'static str {
    match p {
        Pace::OnTrack => "On track",
        Pace::Behind => "Behind",
        Pace::AtRisk => "At risk",
        Pace::Done => "Achieved",
    }
}

/// Days left as "D-12", "Due today" or "D+3" (late).
pub(crate) fn dday(due: NaiveDate, day: NaiveDate) -> String {
    let n = (due - day).num_days();
    match n.cmp(&0) {
        std::cmp::Ordering::Greater => format!("D-{}", n),
        std::cmp::Ordering::Equal => tr("Due today").into(),
        std::cmp::Ordering::Less => format!("D+{}", -n),
    }
}

/// A number the way people write it: thousands grouped, at most two
/// decimals, none when whole.
pub(crate) fn fmt_num(v: f64) -> String {
    let neg = v < 0.0;
    let v = v.abs();
    let whole = v.trunc() as u64;
    let frac = ((v - v.trunc()) * 100.0).round() as u64;
    let (whole, frac) = if frac >= 100 {
        (whole + 1, 0)
    } else {
        (whole, frac)
    };
    let digits = whole.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if frac > 0 {
        let f = format!("{:02}", frac);
        out.push('.');
        out.push_str(f.trim_end_matches('0'));
    }
    // -0.001 rounds to nothing: "0", not "-0"
    if neg && (whole > 0 || frac > 0) {
        format!("-{}", out)
    } else {
        out
    }
}

/// A progress ring: the track all round, the share `frac` from the top,
/// clockwise, with round ends.
pub(crate) fn paint_ring(
    p: &egui::Painter,
    c: Pos2,
    r: f32,
    w: f32,
    frac: f32,
    color: Color32,
    track: Color32,
) {
    p.circle_stroke(c, r, Stroke::new(w, track));
    let frac = frac.clamp(0.0, 1.0);
    if frac <= 0.0 {
        return;
    }
    if frac >= 0.999 {
        p.circle_stroke(c, r, Stroke::new(w, color));
        return;
    }
    let n = ((72.0 * frac).ceil() as usize).max(2);
    let top = -std::f32::consts::FRAC_PI_2;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = top + std::f32::consts::TAU * frac * i as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    p.circle_filled(pts[0], w / 2.0, color);
    p.circle_filled(*pts.last().unwrap(), w / 2.0, color);
    p.add(egui::Shape::line(pts, Stroke::new(w, color)));
}

/// A rounded bar `frac` full.
pub(crate) fn paint_bar(p: &egui::Painter, r: Rect, frac: f32, color: Color32, track: Color32) {
    let rad = r.height() / 2.0;
    p.rect_filled(r, rad, track);
    let frac = frac.clamp(0.0, 1.0);
    if frac > 0.0 {
        let w = (r.width() * frac).max(r.height());
        p.rect_filled(Rect::from_min_size(r.min, vec2(w, r.height())), rad, color);
    }
}

/// The widest the Goals page and a goal's page get.
pub(crate) const PAGE_W: f32 = 1160.0;
/// The row at the top of both pages (the title and its controls, or the way
/// back), the same height on both so the boxes below start at one place.
const HEADER_H: f32 = 40.0;

/// The frame the Goals page and a goal's page share: the same margins and
/// header row, at most `PAGE_W` wide and centred in a wider window, so going
/// from one to the other moves nothing. `header` fills the row at the top,
/// `body` what is under it.
pub(crate) fn goals_frame(
    ui: &mut Ui,
    id: impl std::hash::Hash,
    header: impl FnOnce(&mut Ui),
    body: impl FnOnce(&mut Ui),
) {
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 32,
                    right: 32,
                    top: 24,
                    bottom: 40,
                })
                .show(ui, |ui| {
                    let avail = ui.available_rect_before_wrap();
                    let w = avail.width().min(PAGE_W);
                    let left = avail.left() + (avail.width() - w) / 2.0;
                    let rect =
                        Rect::from_min_max(pos2(left, avail.top()), pos2(left + w, avail.bottom()));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        ui.set_max_width(w);
                        ui.allocate_ui_with_layout(
                            vec2(w, HEADER_H),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.set_min_size(vec2(w, HEADER_H));
                                header(ui);
                            },
                        );
                        ui.add_space(18.0);
                        body(ui);
                    });
                });
        });
}

/// Share to team chat (in the goal's colour) and Set the next goal side by
/// side, the same height, as Goal achieved and an achieved goal's page show
/// them. Returns their responses.
pub(crate) fn achieved_buttons(ui: &mut Ui, color: Color32) -> (Response, Response) {
    ui.horizontal(|ui| {
        let share = ui.add(
            egui::Button::new(
                RichText::new(tr("Share to team chat"))
                    .color(Color32::WHITE)
                    .strong(),
            )
            .fill(color)
            .corner_radius(10)
            .min_size(vec2(0.0, ACHIEVED_BTN_H)),
        );
        let next = ui.add(
            egui::Button::new(RichText::new(tr("Set the next goal")).strong())
                .corner_radius(10)
                .min_size(vec2(0.0, ACHIEVED_BTN_H)),
        );
        (share, next)
    })
    .inner
}

pub(crate) const ACHIEVED_BTN_H: f32 = 36.0;

/// A pill with a dot and the pace's name.
pub(crate) fn pace_pill(ui: &mut Ui, t: &Theme, pace: Pace) -> Response {
    let (bg, fg) = pace_colors(pace, t);
    let font = FontId::proportional(12.0);
    let g = ui
        .painter()
        .layout_no_wrap(tr(pace_label(pace)).to_string(), font, fg);
    let size = vec2(g.size().x + 26.0, 22.0);
    let (r, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(r, 11.0, bg);
    ui.painter()
        .circle_filled(pos2(r.left() + 11.0, r.center().y), 3.5, fg);
    ui.painter().galley(
        pos2(r.left() + 18.0, r.center().y - g.size().y / 2.0),
        g,
        fg,
    );
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, pace_label(pace)));
    resp
}

/// What a target is worth now, 0-1, and how many of its parts are done.
#[derive(Clone)]
pub(crate) struct TargetState {
    pub frac: f32,
    pub done: bool,
    /// "7,200 / 10,000", "3 / 10", "" for done or not
    pub value: String,
    pub unit: String,
}

/// One row of the Goals page or the summary: a goal with what is worked out
/// from it.
#[derive(Clone)]
pub(crate) struct GoalRow {
    pub g: Goal,
    pub pct: u8,
    pub pace: Option<Pace>,
    /// Where it should be by today, 0-100, when it has start and due.
    pub expect: Option<u8>,
    pub targets: Vec<TargetState>,
}

impl App {
    pub(crate) fn goal_progress(&self, g: &Goal) -> f32 {
        if g.targets.is_empty() {
            return 0.0;
        }
        let sum: f32 = g.targets.iter().map(|tg| self.target_state(tg).frac).sum();
        sum / g.targets.len() as f32
    }

    pub(crate) fn target_state(&self, tg: &GoalTarget) -> TargetState {
        match &tg.kind {
            TargetKind::Number {
                start,
                target,
                current,
                unit,
            } => {
                let frac = if (target - start).abs() < f64::EPSILON {
                    1.0
                } else {
                    (((current - start) / (target - start)) as f32).clamp(0.0, 1.0)
                };
                TargetState {
                    frac,
                    done: frac >= 1.0,
                    value: format!("{} / {}", fmt_num(*current), fmt_num(*target)),
                    unit: unit.clone(),
                }
            }
            TargetKind::TrueFalse { done } => TargetState {
                frac: if *done { 1.0 } else { 0.0 },
                done: *done,
                value: String::new(),
                unit: String::new(),
            },
            TargetKind::Tasks { ids } => {
                // tasks deleted since they were picked don't count either way
                let live: Vec<&Task> = ids.iter().filter_map(|id| self.task(*id)).collect();
                let closed = live.iter().filter(|x| self.data.is_closed(x)).count();
                let frac = if live.is_empty() {
                    0.0
                } else {
                    closed as f32 / live.len() as f32
                };
                TargetState {
                    frac,
                    done: !live.is_empty() && closed == live.len(),
                    value: format!("{} / {}", closed, live.len()),
                    unit: tr("tasks").into(),
                }
            }
        }
    }

    pub(crate) fn goal_row(&self, g: &Goal, day: NaiveDate) -> GoalRow {
        let p = self.goal_progress(g);
        GoalRow {
            pct: pct_of(p),
            pace: g.pace(p, day),
            expect: g.time_used(day).map(pct_of),
            targets: g.targets.iter().map(|tg| self.target_state(tg)).collect(),
            g: g.clone(),
        }
    }

    /// Brings goal `gi` up to date: today's progress, the milestone reached
    /// and the achieved day. `who` is credited with a milestone it reaches
    /// (0: noticed by the app). Returns the milestone newly reached, if any.
    pub(crate) fn goal_settle(&mut self, gi: usize, who: u64, now: i64) -> Option<u8> {
        let p = self.goal_progress(&self.data.extra.goals[gi]);
        let pct = pct_of(p);
        let day = local(now).map(|l| l.date_naive()).unwrap_or_else(today);
        let g = &mut self.data.extra.goals[gi];
        let mut changed = false;
        let mut reached = None;
        if g.log.is_empty() {
            // a goal from before history was kept: what it has reached
            // already is taken as it is, without announcing it
            g.record(GoalEvent {
                ts: now,
                who: 0,
                what: GoalChange::Tracked,
                pct,
            });
            g.reached = g.reached.max(milestone_at(pct));
            changed = true;
        } else {
            let m = milestone_at(pct);
            if m > g.reached {
                g.reached = m;
                g.record(GoalEvent {
                    ts: now,
                    who,
                    what: GoalChange::Milestone(m),
                    pct,
                });
                reached = Some(m);
                changed = true;
            } else if m < g.reached {
                // dropped back (a target undone): the milestones above
                // aren't reached any more and leave the history; crossing
                // again records them anew (notify tells people only once)
                g.reached = m;
                g.log
                    .retain(|e| !matches!(e.what, GoalChange::Milestone(x) if x > m));
                changed = true;
            }
            if pct >= 100 && g.done_on.is_none() {
                g.done_on = Some(day);
                changed = true;
            }
        }
        if pct < 100 && g.done_on.is_some() {
            g.done_on = None;
            changed = true;
        }
        if g.days.last().map(|x| x.1) != Some(pct) {
            if g.days.last().map(|x| x.0) == Some(day) {
                g.days.last_mut().unwrap().1 = pct;
            } else {
                g.days.push((day, pct));
                if g.days.len() > GOAL_DAYS_KEEP {
                    let extra = g.days.len() - GOAL_DAYS_KEEP;
                    g.days.drain(..extra);
                }
            }
            changed = true;
        }
        // a milestone's banner goes when the goal drops back below it
        let gid = g.id;
        if let Some((bg, bm, _)) = self.goal_banner {
            if bg == gid && pct < bm {
                self.goal_banner = None;
            }
        }
        if changed {
            self.dirty = true;
        }
        reached
    }

    /// Keeps every goal's milestones, achieved day and daily progress up to
    /// date (tasks closed anywhere move them). Run on the notification clock.
    pub(crate) fn goal_watch(&mut self, now: i64) {
        for gi in 0..self.data.extra.goals.len() {
            self.goal_settle(gi, 0, now);
        }
    }

    /// An edit made on a goal's page: `f` changes it, `what` goes into its
    /// history. Reaching 100% opens Goal achieved; another milestone shows
    /// its banner on the page.
    pub(crate) fn goal_edit(
        &mut self,
        gid: u64,
        what: Option<GoalChange>,
        f: impl FnOnce(&mut Goal),
    ) {
        let Some(gi) = self.data.extra.goals.iter().position(|g| g.id == gid) else {
            return;
        };
        let before = pct_of(self.goal_progress(&self.data.extra.goals[gi]));
        self.goal_edit_from(gi, before, what, f);
    }

    /// `goal_edit` for goal `gi` with its progress `before` the change, for
    /// a change made outside it first (a task closed).
    fn goal_edit_from(
        &mut self,
        gi: usize,
        before: u8,
        what: Option<GoalChange>,
        f: impl FnOnce(&mut Goal),
    ) {
        let gid = self.data.extra.goals[gi].id;
        f(&mut self.data.extra.goals[gi]);
        let now = now_ts();
        let pct = pct_of(self.goal_progress(&self.data.extra.goals[gi]));
        if let Some(what) = what {
            let me = self.data.me;
            self.data.extra.goals[gi].record(GoalEvent {
                ts: now,
                who: me,
                what,
                pct,
            });
        }
        self.dirty = true;
        let me = self.data.me;
        let reached = self.goal_settle(gi, me, now);
        if before < 100 && pct >= 100 {
            self.goal_banner = None;
            self.modal = Some(Modal::GoalAchieved { id: gid });
        } else if let Some(m) = reached.filter(|m| *m < 100) {
            self.goal_banner = Some((gid, m, now));
        }
    }

    /// A new goal starting today, opened on its own page.
    pub(crate) fn add_goal(&mut self, mut g: Goal) -> u64 {
        let id = self.data.new_id();
        let now = now_ts();
        g.id = id;
        if g.start.is_none() {
            g.start = Some(today());
        }
        g.log.clear();
        g.days.clear();
        g.reached = 0;
        g.done_on = None;
        let me = self.data.me;
        g.record(GoalEvent {
            ts: now,
            who: me,
            what: GoalChange::Created,
            pct: 0,
        });
        self.data.extra.goals.push(g);
        let gi = self.data.extra.goals.len() - 1;
        let pct = pct_of(self.goal_progress(&self.data.extra.goals[gi]));
        self.data.extra.goals[gi].log[0].pct = pct;
        self.goal_settle(gi, me, now);
        self.goal_open = Some(id);
        self.dirty = true;
        id
    }

    /// The Goals page: goals in progress as cards or rows, achieved ones below.
    pub(crate) fn goals_page(&mut self, ui: &mut Ui) {
        if let Some(id) = self.goal_open {
            if self.data.extra.goals.iter().any(|g| g.id == id) {
                self.goal_page(ui, id);
                return;
            }
            self.goal_open = None;
        }
        let t = self.t();
        let day = today();
        let me = self.data.me;
        let rows: Vec<GoalRow> = self
            .data
            .extra
            .goals
            .iter()
            .map(|g| self.goal_row(g, day))
            .collect();
        let mine = |r: &&GoalRow| r.g.owner == me;
        let n_all = rows.len();
        let n_mine = rows.iter().filter(mine).count();
        let n_done = rows.iter().filter(|r| r.pct >= 100).count();
        let filter = self.goal_filter;
        let shown: Vec<&GoalRow> = rows
            .iter()
            .filter(|r| filter != 1 || r.g.owner == me)
            .collect();
        let mut active: Vec<&GoalRow> = shown.iter().copied().filter(|r| r.pct < 100).collect();
        // the most urgent first: far behind, a little behind, on track, then
        // goals without dates; sooner deadlines first within each
        let rank = |r: &GoalRow| match r.pace {
            Some(Pace::AtRisk) => 0,
            Some(Pace::Behind) => 1,
            Some(Pace::OnTrack) => 2,
            _ => 3,
        };
        active.sort_by_key(|r| (rank(r), r.g.due.unwrap_or(NaiveDate::MAX)));
        let mut achieved: Vec<&GoalRow> = shown.iter().copied().filter(|r| r.pct >= 100).collect();
        achieved.sort_by_key(|r| std::cmp::Reverse(r.g.done_on));
        let list_view = self.data.extra.goals_view == "list";
        let mut head_act: Option<GoalAct> = None;
        let mut act: Option<GoalAct> = None;
        goals_frame(
            ui,
            "goals_page",
            |ui| {
                ui.label(RichText::new(tr("Goals")).size(24.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let b = primary_button(ui, &t, tr("+ New goal"));
                    name_it(&b, "New goal");
                    if b.clicked() {
                        head_act = Some(GoalAct::NewGoal);
                    }
                    ui.add_space(6.0);
                    if let Some(v) = view_switch(ui, &t, list_view) {
                        head_act = Some(GoalAct::View(v));
                    }
                    ui.add_space(6.0);
                    let counts = [n_all, n_mine, n_done];
                    if let Some(f) = filter_tabs(ui, &t, filter, counts) {
                        head_act = Some(GoalAct::Filter(f));
                    }
                });
            },
            |ui| {
                // ---- summary
                let active_all: Vec<&GoalRow> = rows.iter().filter(|r| r.pct < 100).collect();
                let done_all: Vec<&GoalRow> = rows.iter().filter(|r| r.pct >= 100).collect();
                if let Some(a) = self.goals_summary(ui, &t, &active_all, &done_all, day) {
                    act = Some(a);
                }
                ui.add_space(24.0);
                if filter != 2 {
                    section_title(ui, &t, tr("In progress"), active.len());
                    ui.add_space(10.0);
                    if active.is_empty() {
                        ui.label(
                            RichText::new(tr("Nothing in progress. Start a new goal."))
                                .color(t.text2),
                        );
                    } else if list_view {
                        if let Some(a) = self.goal_rows(ui, &t, &active, day) {
                            act = Some(a);
                        }
                    } else if let Some(a) = self.goal_cards(ui, &t, &active, day) {
                        act = Some(a);
                    }
                    ui.add_space(28.0);
                }
                section_title(ui, &t, tr("Achieved"), achieved.len());
                ui.add_space(10.0);
                // as rows too in the list view, so the switch changes the
                // page even when nothing is in progress
                if list_view && !achieved.is_empty() {
                    if let Some(a) = self.goal_rows(ui, &t, &achieved, day) {
                        act = Some(a);
                    }
                } else {
                    let next = active
                        .iter()
                        .copied()
                        .max_by_key(|r| r.pct)
                        .filter(|_| filter != 2);
                    if let Some(a) = achieved_shelf(ui, &t, &achieved, next) {
                        act = Some(a);
                    }
                }
            },
        );
        if let Some(a) = head_act.or(act) {
            self.goal_apply(a);
        }
    }

    /// The three boxes on top: average progress and how many are on track,
    /// the next deadlines, and how many were achieved this year.
    fn goals_summary(
        &self,
        ui: &mut Ui,
        t: &Theme,
        active: &[&GoalRow],
        done: &[&GoalRow],
        day: NaiveDate,
    ) -> Option<GoalAct> {
        let mut act = None;
        let gap = 14.0;
        let w = ui.available_width();
        let cols = if w > 760.0 { 3 } else { 1 };
        let box_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        // the contents of each fit in this, so their bottoms line up
        let h = SUMMARY_H;
        let avg = if active.is_empty() {
            0
        } else {
            (active.iter().map(|r| r.pct as u32).sum::<u32>() / active.len() as u32) as u8
        };
        // change since a week ago, over the goals that had progress then
        let week = day - chrono::Duration::days(7);
        let then: Vec<(u8, u8)> = active
            .iter()
            .filter_map(|r| r.g.pct_on(week).map(|p| (p, r.pct)))
            .collect();
        let delta = if then.is_empty() {
            None
        } else {
            let a: i32 = then.iter().map(|x| x.1 as i32 - x.0 as i32).sum();
            Some(a / then.len() as i32)
        };
        let count = |p: Pace| active.iter().filter(|r| r.pace == Some(p)).count();
        let (on, behind, risk) = (
            count(Pace::OnTrack),
            count(Pace::Behind),
            count(Pace::AtRisk),
        );
        let mut upcoming: Vec<&&GoalRow> = active.iter().filter(|r| r.g.due.is_some()).collect();
        upcoming.sort_by_key(|r| r.g.due);
        let year_done: Vec<&&GoalRow> = done
            .iter()
            .filter(|r| r.g.done_on.is_none_or(|d| d.year() == day.year()))
            .collect();
        let latest = done
            .iter()
            .filter(|r| r.g.done_on.is_some())
            .max_by_key(|r| r.g.done_on);
        let by_month = achieved_by_month(done, day.year());
        let closest = active.iter().max_by_key(|r| r.pct);
        let mut boxes = |ui: &mut Ui, i: usize| {
            let resp = egui::Frame::new()
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.border))
                .corner_radius(14)
                .inner_margin(egui::Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    // the margins and the 1 pt outline on each side
                    ui.set_width(box_w - 38.0);
                    ui.set_height(h - 34.0);
                    match i {
                        0 => {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(tr("Average progress"))
                                        .size(13.0)
                                        .color(t.text2),
                                );
                                if let Some(d) = delta.filter(|d| *d != 0) {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let (txt, pace) = if d > 0 {
                                            (trf!("Up {} points this week", d), Pace::OnTrack)
                                        } else {
                                            (trf!("Down {} points this week", -d), Pace::AtRisk)
                                        };
                                        let (bg, fg) = pace_colors(pace, t);
                                        egui::Frame::new()
                                            .fill(bg)
                                            .corner_radius(10)
                                            .inner_margin(egui::Margin::symmetric(8, 2))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(txt)
                                                        .size(11.5)
                                                        .color(fg)
                                                        .strong(),
                                                );
                                            });
                                    });
                                }
                            });
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                ui.label(RichText::new(avg.to_string()).size(34.0).strong());
                                ui.label(RichText::new("%").size(17.0).color(t.text2));
                            });
                            ui.add_space(8.0);
                            let total = (on + behind + risk).max(1) as f32;
                            let (r, _) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 8.0),
                                Sense::hover(),
                            );
                            let mut x = r.left();
                            let parts = [
                                (on, Pace::OnTrack),
                                (behind, Pace::Behind),
                                (risk, Pace::AtRisk),
                            ];
                            if on + behind + risk == 0 {
                                ui.painter().rect_filled(r, 4.0, t.hover);
                            }
                            for (n, p) in parts {
                                if n == 0 {
                                    continue;
                                }
                                let seg = r.width() * n as f32 / total;
                                let rr = Rect::from_min_size(
                                    pos2(x, r.top()),
                                    vec2((seg - 3.0).max(4.0), r.height()),
                                );
                                ui.painter().rect_filled(rr, 4.0, pace_colors(p, t).1);
                                x += seg;
                            }
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                for (n, p) in parts {
                                    let (dot, _) =
                                        ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                                    ui.painter().circle_filled(
                                        dot.center(),
                                        4.0,
                                        pace_colors(p, t).1,
                                    );
                                    ui.label(
                                        RichText::new(format!("{} {}", tr(pace_label(p)), n))
                                            .size(12.5)
                                            .color(t.text2),
                                    );
                                    ui.add_space(6.0);
                                }
                            });
                        }
                        1 => {
                            ui.label(RichText::new(tr("Coming up")).size(13.0).color(t.text2));
                            ui.add_space(6.0);
                            if upcoming.is_empty() {
                                ui.label(RichText::new(tr("No deadlines set")).color(t.text3));
                            }
                            for r in upcoming.iter().take(3) {
                                let (row, click) = ui.allocate_exact_size(
                                    vec2(ui.available_width(), 26.0),
                                    Sense::click(),
                                );
                                hover_bg(ui, &click, row.expand(2.0), 6.0);
                                name_it(&click, &format!("Upcoming {}", r.g.name));
                                if click.clicked() {
                                    act = Some(GoalAct::Open(r.g.id));
                                }
                                let p = ui.painter();
                                let cy = row.center().y;
                                p.circle_filled(pos2(row.left() + 5.0, cy), 5.0, rgb(r.g.color));
                                // the days left in a pill at the right, the
                                // progress before it, the name in what is left
                                let due = r.g.due.unwrap();
                                let (bg, fg) = match r.pace {
                                    Some(pc @ (Pace::Behind | Pace::AtRisk)) => pace_colors(pc, t),
                                    _ => (t.hover, t.text),
                                };
                                let dg = p.layout_no_wrap(
                                    dday(due, day),
                                    FontId::proportional(12.0),
                                    fg,
                                );
                                let pill = Rect::from_min_max(
                                    pos2(row.right() - dg.size().x - 12.0, cy - 10.0),
                                    pos2(row.right(), cy + 10.0),
                                );
                                p.rect_filled(pill, 6.0, bg);
                                p.galley(pill.center() - dg.size() / 2.0, dg, fg);
                                let pw = p
                                    .text(
                                        pos2(pill.left() - 8.0, cy),
                                        Align2::RIGHT_CENTER,
                                        format!("{}%", r.pct),
                                        FontId::proportional(12.5),
                                        t.text2,
                                    )
                                    .width();
                                let nf = FontId::proportional(13.5);
                                let room = pill.left() - 8.0 - pw - 10.0 - (row.left() + 18.0);
                                p.text(
                                    pos2(row.left() + 18.0, cy),
                                    Align2::LEFT_CENTER,
                                    crate::widgets::fit_text(p, &r.g.name, &nf, room),
                                    nf,
                                    t.text,
                                );
                            }
                        }
                        _ => {
                            ui.label(
                                RichText::new(tr("Achieved this year"))
                                    .size(13.0)
                                    .color(t.text2),
                            );
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(46.0, 46.0), Sense::hover());
                                let (bg, fg) = gold(t);
                                ui.painter().rect_filled(r, 14.0, bg);
                                icons::line_icon(ui.painter(), "goals", r.center(), fg);
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(year_done.len().to_string())
                                        .size(34.0)
                                        .strong(),
                                );
                            });
                            ui.add_space(8.0);
                            month_strip(ui, t, &by_month, day);
                            ui.add_space(4.0);
                            // the latest one, else the one closest to done,
                            // so the box always says something
                            let line = if let Some(l) = latest {
                                trf!(
                                    "Latest: {}, {}",
                                    l.g.name,
                                    l.g.done_on.unwrap().text("%b %-d")
                                )
                            } else if let Some(c) = closest {
                                trf!("Next up: {} · {}%", c.g.name, c.pct)
                            } else {
                                tr("Goals reaching 100% are counted here").into()
                            };
                            ui.add(
                                egui::Label::new(RichText::new(line).size(12.5).color(t.text2))
                                    .truncate(),
                            );
                        }
                    }
                });
            let name = SUMMARY_NAMES[i];
            resp.response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, name));
        };
        if cols == 3 {
            // top-aligned: a centred row would nudge the boxes down a little,
            // off the line a goal's page starts its box on
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for i in 0..3 {
                    // a column each, so a box's contents stack
                    ui.vertical(|ui| boxes(ui, i));
                }
            });
        } else {
            for i in 0..3 {
                boxes(ui, i);
                ui.add_space(gap);
            }
        }
        act.take()
    }

    /// Goals in progress as cards, two to a row when there is room.
    fn goal_cards(
        &self,
        ui: &mut Ui,
        t: &Theme,
        rows: &[&GoalRow],
        day: NaiveDate,
    ) -> Option<GoalAct> {
        let mut act = None;
        let gap = 14.0;
        let w = ui.available_width();
        let cols = if w > 820.0 { 2 } else { 1 };
        let card_w = (w - gap * (cols as f32 - 1.0)) / cols as f32;
        for chunk in rows.chunks(cols) {
            let h = chunk.iter().map(|r| card_height(r)).fold(0.0, f32::max);
            let (row_rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
            for (i, r) in chunk.iter().enumerate() {
                let rect = Rect::from_min_size(
                    row_rect.min + vec2(i as f32 * (card_w + gap), 0.0),
                    vec2(card_w, h),
                );
                if let Some(a) = self.goal_card(ui, t, r, rect, day) {
                    act = Some(a);
                }
            }
            ui.add_space(gap);
        }
        act
    }

    fn goal_card(
        &self,
        ui: &mut Ui,
        t: &Theme,
        r: &GoalRow,
        rect: Rect,
        day: NaiveDate,
    ) -> Option<GoalAct> {
        let mut act = None;
        let g = &r.g;
        let color = rgb(g.color);
        // the card itself opens the goal; the controls on it come later and
        // so sit on top of it
        let card = ui.interact(rect, ui.id().with(("goal card", g.id)), Sense::click());
        name_it(&card, &format!("Goal {}", g.name));
        let painter = ui.painter().clone();
        let p = &painter;
        p.rect_filled(rect, 14.0, t.card);
        p.rect_stroke(
            rect,
            14.0,
            Stroke::new(1.0, if card.hovered() { t.text3 } else { t.border }),
            egui::StrokeKind::Inside,
        );
        if card.clicked() {
            act = Some(GoalAct::Open(g.id));
        }
        let inner = rect.shrink2(vec2(20.0, 18.0));
        // ring and title
        let ring_c = pos2(inner.left() + 32.0, inner.top() + 32.0);
        paint_ring(p, ring_c, 27.0, 7.0, r.pct as f32 / 100.0, color, t.hover);
        p.text(
            ring_c,
            Align2::CENTER_CENTER,
            format!("{}%", r.pct),
            FontId::proportional(15.0),
            t.text,
        );
        let tx = inner.left() + 80.0;
        let name_font = FontId::proportional(16.5);
        let name = crate::widgets::fit_text(p, &g.name, &name_font, inner.right() - tx);
        p.text(
            pos2(tx, inner.top() + 10.0),
            Align2::LEFT_TOP,
            name,
            name_font,
            t.text,
        );
        let meta = Rect::from_min_max(
            pos2(tx, inner.top() + 36.0),
            pos2(inner.right(), inner.top() + 60.0),
        );
        place(ui, meta, |ui| {
            if let Some(pace) = r.pace {
                pace_pill(ui, t, pace);
                if let Some(e) = r.expect.filter(|_| pace != Pace::Done) {
                    ui.label(RichText::new(pace_text(r.pct, e)).size(12.5).color(t.text2));
                }
            } else {
                ui.label(
                    RichText::new(tr("Add a start and due date to see the pace"))
                        .size(12.5)
                        .color(t.text3),
                );
            }
        });
        // targets
        let mut y = inner.top() + 82.0;
        if g.targets.is_empty() {
            p.text(
                pos2(inner.left(), y + 4.0),
                Align2::LEFT_TOP,
                tr("No targets yet"),
                FontId::proportional(13.0),
                t.text3,
            );
            y += TARGET_BOOL_H + TARGET_GAP;
        }
        for (i, (tg, st)) in g.targets.iter().zip(&r.targets).enumerate() {
            let row = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 22.0));
            let font = FontId::proportional(13.5);
            match &tg.kind {
                TargetKind::TrueFalse { done } => {
                    let bx = Rect::from_center_size(
                        pos2(row.left() + 10.0, row.center().y),
                        vec2(18.0, 18.0),
                    );
                    let resp = ui.interact(
                        bx.expand(3.0),
                        ui.id().with(("goal check", g.id, i)),
                        Sense::click(),
                    );
                    name_it(&resp, &format!("Done {}", tg.name));
                    paint_check(ui.painter(), bx, *done, color, t, resp.hovered());
                    if resp.clicked() {
                        act = Some(GoalAct::ToggleDone { g: g.id, i });
                    }
                    let p = ui.painter();
                    let (label, c) = if *done {
                        (tr("Done"), pace_colors(Pace::OnTrack, t).1)
                    } else {
                        (tr("Not done"), t.text3)
                    };
                    let lw = p
                        .text(
                            pos2(row.right(), row.center().y),
                            Align2::RIGHT_CENTER,
                            label,
                            FontId::proportional(12.5),
                            c,
                        )
                        .width();
                    let n = crate::widgets::fit_text(p, &tg.name, &font, row.width() - 40.0 - lw);
                    p.text(
                        pos2(row.left() + 28.0, row.center().y),
                        Align2::LEFT_CENTER,
                        n,
                        font,
                        t.text,
                    );
                    y += TARGET_BOOL_H + TARGET_GAP;
                }
                _ => {
                    let p = ui.painter();
                    let kind = if matches!(tg.kind, TargetKind::Number { .. }) {
                        "hash"
                    } else {
                        "list"
                    };
                    // three quarters size, the height of the name beside it
                    icons::line_icon_scaled(
                        p,
                        kind,
                        pos2(row.left() + 9.0, row.center().y),
                        t.text2,
                        0.75,
                    );
                    let mut right = row.right();
                    if let TargetKind::Number { current, .. } = &tg.kind {
                        let b = Rect::from_min_size(
                            pos2(right - 58.0, row.top() - 1.0),
                            vec2(58.0, 24.0),
                        );
                        let resp =
                            ui.interact(b, ui.id().with(("goal record", g.id, i)), Sense::click());
                        name_it(&resp, &format!("Record {}", tg.name));
                        let p = ui.painter();
                        p.rect_filled(b, 7.0, if resp.hovered() { t.hover } else { t.card });
                        p.rect_stroke(b, 7.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                        p.text(
                            b.center(),
                            Align2::CENTER_CENTER,
                            tr("+ Record"),
                            FontId::proportional(12.0),
                            t.text,
                        );
                        record_popup(ui, t, &resp, g.id, i, *current, &tg.name, &mut act);
                        right -= 66.0;
                    }
                    let p = ui.painter();
                    let unit_w = if st.unit.is_empty() {
                        0.0
                    } else {
                        p.text(
                            pos2(right, row.center().y),
                            Align2::RIGHT_CENTER,
                            &st.unit,
                            FontId::proportional(12.5),
                            t.text2,
                        )
                        .width()
                            + 4.0
                    };
                    let vw = p
                        .text(
                            pos2(right - unit_w, row.center().y),
                            Align2::RIGHT_CENTER,
                            &st.value,
                            FontId::proportional(13.5),
                            t.text,
                        )
                        .width();
                    let n = crate::widgets::fit_text(
                        p,
                        &tg.name,
                        &font,
                        (right - unit_w - vw - 12.0) - (row.left() + 28.0),
                    );
                    p.text(
                        pos2(row.left() + 28.0, row.center().y),
                        Align2::LEFT_CENTER,
                        n,
                        font,
                        t.text,
                    );
                    let bar = Rect::from_min_size(
                        pos2(row.left() + 28.0, row.bottom() + 6.0),
                        vec2(row.width() - 28.0, 6.0),
                    );
                    paint_bar(p, bar, st.frac, color, t.hover);
                    y += TARGET_BAR_H + TARGET_GAP;
                }
            }
        }
        // footer: owner and targets done, due date, in the middle of the
        // band under the line. The due date goes in first, at the right; the
        // owner's name and how many targets are done share what is left,
        // shortened to fit, so a long name never runs over the date.
        let p = ui.painter();
        let line_y = rect.bottom() - CARD_FOOTER_H;
        let fy = line_y + CARD_FOOTER_H / 2.0;
        p.hline(inner.x_range(), line_y, Stroke::new(1.0, t.border));
        let font = FontId::proportional(12.5);
        let mut right = inner.right();
        if let Some(due) = g.due {
            let (_, fg) = match r.pace {
                Some(pc @ (Pace::Behind | Pace::AtRisk)) => pace_colors(pc, t),
                _ => (t.card, t.text),
            };
            let dw = p
                .text(
                    pos2(inner.right(), fy),
                    Align2::RIGHT_CENTER,
                    dday(due, day),
                    font.clone(),
                    fg,
                )
                .width();
            let tw = p
                .text(
                    pos2(inner.right() - dw - 8.0, fy),
                    Align2::RIGHT_CENTER,
                    due.text("%b %-d"),
                    font.clone(),
                    t.text2,
                )
                .width();
            // the 16 pt icon, the usual gap before the date
            let icon_x = inner.right() - dw - 8.0 - tw - icons::ICON_GAP - 8.0;
            icons::line_icon(p, "calendar", pos2(icon_x, fy), t.text2);
            right = icon_x - 8.0 - 12.0;
        }
        let done = r.targets.iter().filter(|s| s.done).count();
        let summary = if g.targets.is_empty() {
            String::new()
        } else {
            trf!("{} of {} targets done", done, g.targets.len())
        };
        let summary_w = if summary.is_empty() {
            0.0
        } else {
            p.layout_no_wrap(summary.clone(), font.clone(), t.text2)
                .size()
                .x
                + 8.0
        };
        let mut x = inner.left();
        if let Some(m) = self.data.member(g.owner) {
            paint_avatar(p, pos2(x + 11.0, fy), 11.0, m, t.card);
            x += 30.0;
            // the name gives way to the summary, down to a few letters
            let room = (right - x - summary_w).max(60.0).min(right - x);
            let name = crate::widgets::fit_text(p, m.shown(), &font, room);
            x += p
                .text(pos2(x, fy), Align2::LEFT_CENTER, name, font.clone(), t.text)
                .width()
                + 8.0;
        }
        if !summary.is_empty() && right - x > 24.0 {
            p.text(
                pos2(x, fy),
                Align2::LEFT_CENTER,
                crate::widgets::fit_text(p, &summary, &font, right - x),
                font,
                t.text2,
            );
        }
        act
    }

    /// Goals in progress as rows: progress against where it should be by
    /// today, pace, due date and owner. A row's arrow shows its targets.
    fn goal_rows(
        &self,
        ui: &mut Ui,
        t: &Theme,
        rows: &[&GoalRow],
        day: NaiveDate,
    ) -> Option<GoalAct> {
        let mut act = None;
        // inside the 1 pt outline on each side
        let w = ui.available_width() - 2.0;
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(12)
            .show(ui, |ui| {
                ui.set_width(w);
                let cols = row_cols(w);
                // heading
                let (hr, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
                let p = ui.painter();
                let f = FontId::proportional(12.5);
                let heads = [
                    tr("Goal"),
                    tr("Progress"),
                    tr("Pace"),
                    tr("Due"),
                    crate::i18n::tr_ctx("Owner (goal)", "Owner"),
                ];
                for (i, h) in heads.iter().enumerate() {
                    p.text(
                        pos2(hr.left() + cols[i], hr.center().y),
                        Align2::LEFT_CENTER,
                        *h,
                        f.clone(),
                        t.text2,
                    );
                }
                p.hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
                let legend = rows.iter().any(|r| plan_mark(r).is_some());
                for (ri, r) in rows.iter().enumerate() {
                    let g = &r.g;
                    let open = self.goal_list_open == Some(g.id);
                    let (rr, _) = ui.allocate_exact_size(vec2(w, 58.0), Sense::hover());
                    let click = ui.interact(rr, ui.id().with(("goal row", g.id)), Sense::click());
                    name_it(&click, &format!("Goal row {}", g.name));
                    if click.hovered() {
                        ui.painter()
                            .rect_filled(rr, 0.0, t.hover.gamma_multiply(0.5));
                    }
                    if click.clicked() {
                        act = Some(GoalAct::Open(g.id));
                    }
                    let cy = rr.center().y;
                    // the arrow
                    let ab = Rect::from_center_size(pos2(rr.left() + 22.0, cy), vec2(24.0, 24.0));
                    let ar =
                        ui.interact(ab, ui.id().with(("goal row arrow", g.id)), Sense::click());
                    name_it(&ar, &format!("Show targets of {}", g.name));
                    hover_bg(ui, &ar, ab, 6.0);
                    let p = ui.painter();
                    paint_triangle(p, ab.center(), open, t.text2);
                    if ar.clicked() {
                        act = Some(GoalAct::ExpandRow(if open { None } else { Some(g.id) }));
                    }
                    p.circle_filled(pos2(rr.left() + 44.0, cy), 5.0, rgb(g.color));
                    let nf = FontId::proportional(14.0);
                    let name_w = cols[1] - 70.0;
                    p.text(
                        pos2(rr.left() + 56.0, cy - 8.0),
                        Align2::LEFT_CENTER,
                        crate::widgets::fit_text(p, &g.name, &nf, name_w),
                        nf,
                        t.text,
                    );
                    let done = r.targets.iter().filter(|s| s.done).count();
                    p.text(
                        pos2(rr.left() + 56.0, cy + 10.0),
                        Align2::LEFT_CENTER,
                        trf!("{} of {} targets done", done, g.targets.len()),
                        FontId::proportional(12.0),
                        t.text2,
                    );
                    // progress with today's mark
                    let bar_w = cols[2] - cols[1] - 70.0;
                    let bar =
                        Rect::from_min_size(pos2(rr.left() + cols[1], cy - 4.0), vec2(bar_w, 8.0));
                    paint_bar(p, bar, r.pct as f32 / 100.0, rgb(g.color), t.hover);
                    if let Some(e) = plan_mark(r) {
                        let x = bar.left() + bar.width() * e as f32 / 100.0;
                        let m = Rect::from_min_max(
                            pos2(x - 1.0, bar.top() - 5.0),
                            pos2(x + 1.0, bar.bottom() + 5.0),
                        );
                        p.rect_filled(m, 1.0, t.text);
                        let hint = ui
                            .interact(
                                m.expand2(vec2(4.0, 2.0)),
                                ui.id().with(("goal plan mark", g.id)),
                                Sense::hover(),
                            )
                            .on_hover_text(trf!("Planned for today: {}%", e));
                        name_it(&hint, &format!("Plan mark {}", g.name));
                    }
                    p.text(
                        pos2(bar.right() + 12.0, cy),
                        Align2::LEFT_CENTER,
                        format!("{}%", r.pct),
                        FontId::proportional(14.0),
                        t.text,
                    );
                    let pace_rect = Rect::from_min_size(
                        pos2(rr.left() + cols[2], cy - 11.0),
                        vec2(cols[3] - cols[2], 22.0),
                    );
                    if let Some(pace) = r.pace {
                        place(ui, pace_rect, |ui| {
                            pace_pill(ui, t, pace);
                        });
                    }
                    let p = ui.painter();
                    if let Some(due) = g.due {
                        p.text(
                            pos2(rr.left() + cols[3], cy - 8.0),
                            Align2::LEFT_CENTER,
                            due.text("%b %-d"),
                            FontId::proportional(13.0),
                            t.text,
                        );
                        let fg = match r.pace {
                            Some(pc @ (Pace::Behind | Pace::AtRisk)) => pace_colors(pc, t).1,
                            _ => t.text2,
                        };
                        // when it was achieved instead of the days left
                        let under = match g.done_on.filter(|_| r.pct >= 100) {
                            Some(d) => trf!("Achieved {}", d.text("%b %-d")),
                            None => dday(due, day),
                        };
                        p.text(
                            pos2(rr.left() + cols[3], cy + 10.0),
                            Align2::LEFT_CENTER,
                            under,
                            FontId::proportional(12.0),
                            fg,
                        );
                    }
                    if let Some(m) = self.data.member(g.owner) {
                        paint_avatar(p, pos2(rr.left() + cols[4] + 11.0, cy), 11.0, m, t.card);
                        // shortened to end inside the row
                        let nf = FontId::proportional(13.0);
                        let room = rr.right() - 16.0 - (rr.left() + cols[4] + 28.0);
                        p.text(
                            pos2(rr.left() + cols[4] + 28.0, cy),
                            Align2::LEFT_CENTER,
                            crate::widgets::fit_text(p, m.shown(), &nf, room),
                            nf,
                            t.text,
                        );
                    }
                    if open {
                        for (tg, st) in g.targets.iter().zip(&r.targets) {
                            let (tr_, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
                            let p = ui.painter();
                            let cy = tr_.center().y;
                            let kind = match tg.kind {
                                TargetKind::Number { .. } => tr("Number"),
                                TargetKind::TrueFalse { .. } => tr("Done or not"),
                                TargetKind::Tasks { .. } => tr("Tasks"),
                            };
                            let kg = p.layout_no_wrap(
                                kind.to_string(),
                                FontId::proportional(11.5),
                                t.text2,
                            );
                            let kr = Rect::from_min_size(
                                pos2(tr_.left() + 56.0, cy - 9.0),
                                vec2(kg.size().x + 10.0, 18.0),
                            );
                            p.rect_filled(kr, 5.0, t.hover);
                            p.galley(kr.center() - kg.size() / 2.0, kg, t.text2);
                            let nf = FontId::proportional(13.0);
                            p.text(
                                pos2(kr.right() + 8.0, cy),
                                Align2::LEFT_CENTER,
                                crate::widgets::fit_text(
                                    p,
                                    &tg.name,
                                    &nf,
                                    cols[1] - (kr.right() - tr_.left()) - 20.0,
                                ),
                                nf,
                                t.text,
                            );
                            let bar = Rect::from_min_size(
                                pos2(tr_.left() + cols[1], cy - 2.5),
                                vec2(bar_w, 5.0),
                            );
                            paint_bar(p, bar, st.frac, rgb(g.color).gamma_multiply(0.75), t.hover);
                            p.text(
                                pos2(bar.right() + 12.0, cy),
                                Align2::LEFT_CENTER,
                                format!("{}%", pct_of(st.frac)),
                                FontId::proportional(12.5),
                                t.text2,
                            );
                            let value = if st.value.is_empty() {
                                if st.done {
                                    tr("Done").to_string()
                                } else {
                                    tr("Not done").to_string()
                                }
                            } else if st.unit.is_empty() {
                                st.value.clone()
                            } else {
                                format!("{} {}", st.value, st.unit)
                            };
                            p.text(
                                pos2(tr_.left() + cols[2], cy),
                                Align2::LEFT_CENTER,
                                value,
                                FontId::proportional(13.0),
                                t.text,
                            );
                        }
                    }
                    // the last row ends on the card's own edge without a legend
                    if legend || ri + 1 < rows.len() {
                        ui.painter().hline(
                            rr.x_range(),
                            ui.cursor().top(),
                            Stroke::new(1.0, t.border),
                        );
                    }
                }
                // what the mark on the bars means, when a bar has one
                if !legend {
                    return;
                }
                let (lr, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
                let p = ui.painter();
                let x = lr.left() + 20.0;
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(x, lr.center().y - 7.0),
                        pos2(x + 2.0, lr.center().y + 7.0),
                    ),
                    1.0,
                    t.text,
                );
                p.text(
                    pos2(x + 10.0, lr.center().y),
                    Align2::LEFT_CENTER,
                    tr("Where it should be by today, from its start and due dates"),
                    FontId::proportional(12.0),
                    t.text2,
                );
            });
        act
    }

    pub(crate) fn goal_apply(&mut self, a: GoalAct) {
        match a {
            GoalAct::Open(id) => {
                self.goal_open = Some(id);
                self.goal_banner = None;
            }
            GoalAct::Back => {
                self.goal_open = None;
                self.goal_banner = None;
            }
            // in the next colour, as from the empty page and Goal achieved
            GoalAct::NewGoal => self.open_new_goal(None),
            GoalAct::View(v) => {
                self.data.extra.goals_view = v.into();
                self.dirty = true;
            }
            GoalAct::Filter(f) => self.goal_filter = f,
            GoalAct::ExpandRow(id) => self.goal_list_open = id,
            GoalAct::ToggleDone { g, i } => {
                let Some(goal) = self.data.extra.goals.iter().find(|x| x.id == g) else {
                    return;
                };
                let Some(tg) = goal.targets.get(i) else {
                    return;
                };
                let TargetKind::TrueFalse { done } = tg.kind else {
                    return;
                };
                let name = tg.name.clone();
                self.goal_edit(
                    g,
                    Some(GoalChange::Done {
                        target: name,
                        done: !done,
                    }),
                    |goal| {
                        if let Some(TargetKind::TrueFalse { done }) =
                            goal.targets.get_mut(i).map(|t| &mut t.kind)
                        {
                            *done = !*done;
                        }
                    },
                );
            }
            GoalAct::SetNumber { g, i, to } => {
                let Some(goal) = self.data.extra.goals.iter().find(|x| x.id == g) else {
                    return;
                };
                let Some(tg) = goal.targets.get(i) else {
                    return;
                };
                let TargetKind::Number { current, unit, .. } = &tg.kind else {
                    return;
                };
                if !to.is_finite() || (current - to).abs() < f64::EPSILON {
                    return;
                }
                let what = GoalChange::Number {
                    target: tg.name.clone(),
                    from: *current,
                    to,
                    unit: unit.clone(),
                };
                self.goal_edit(g, Some(what), |goal| {
                    if let Some(TargetKind::Number { current, .. }) =
                        goal.targets.get_mut(i).map(|t| &mut t.kind)
                    {
                        *current = to;
                    }
                });
            }
            GoalAct::ToggleTask { g, i, task } => {
                let Some(goal) = self.data.extra.goals.iter().find(|x| x.id == g) else {
                    return;
                };
                let Some(tg) = goal.targets.get(i) else {
                    return;
                };
                let Some(tk) = self.task(task).cloned() else {
                    return;
                };
                let target = tg.name.clone();
                let done = !self.data.is_closed(&tk);
                // where it stood before the task changes it, so closing the
                // last one opens Goal achieved as ticking a target does
                let before = pct_of(self.goal_progress(goal));
                self.toggle_done(task);
                // the task changes through the actions queue: apply it now so
                // the goal's progress counts it
                let acts = std::mem::take(&mut self.actions);
                for a in acts {
                    self.apply(a);
                }
                let Some(gi) = self.data.extra.goals.iter().position(|x| x.id == g) else {
                    return;
                };
                self.goal_edit_from(
                    gi,
                    before,
                    Some(GoalChange::Task {
                        target,
                        task: tk.name,
                        done,
                    }),
                    |_| {},
                );
            }
        }
    }
}

/// Things done on the Goals page and a goal's page, applied after drawing.
pub(crate) enum GoalAct {
    Open(u64),
    Back,
    NewGoal,
    View(&'static str),
    Filter(u8),
    ExpandRow(Option<u64>),
    ToggleDone { g: u64, i: usize },
    SetNumber { g: u64, i: usize, to: f64 },
    ToggleTask { g: u64, i: usize, task: u64 },
}

/// The height of the three boxes on top of the Goals page.
const SUMMARY_H: f32 = 180.0;
/// The three boxes on top, as screen readers and tests know them.
const SUMMARY_NAMES: [&str; 3] = [
    "Summary average progress",
    "Summary coming up",
    "Summary achieved this year",
];

const TARGET_BAR_H: f32 = 34.0;
const TARGET_BOOL_H: f32 = 22.0;
const TARGET_GAP: f32 = 12.0;
/// The band at the bottom of a card under its line: owner, targets done,
/// due date.
const CARD_FOOTER_H: f32 = 50.0;

/// A card's height: title, its targets, 18 pt to the line (the gap after the
/// last target and 6 more), the footer.
fn card_height(r: &GoalRow) -> f32 {
    let targets: f32 = if r.g.targets.is_empty() {
        TARGET_BOOL_H + TARGET_GAP
    } else {
        r.g.targets
            .iter()
            .map(|tg| match tg.kind {
                TargetKind::TrueFalse { .. } => TARGET_BOOL_H + TARGET_GAP,
                _ => TARGET_BAR_H + TARGET_GAP,
            })
            .sum()
    };
    18.0 + 82.0 + targets + 6.0 + CARD_FOOTER_H
}

/// Where a row's bar gets today's mark, 0-100: none before any time has
/// gone (a mark at the very start reads as part of the bar) or once done.
fn plan_mark(r: &GoalRow) -> Option<u8> {
    r.expect.filter(|e| *e > 0 && r.pace != Some(Pace::Done))
}

/// "12 points ahead of plan" / "12 points behind plan" / "On plan".
pub(crate) fn pace_text(pct: u8, expect: u8) -> String {
    let d = pct as i32 - expect as i32;
    match d.cmp(&0) {
        std::cmp::Ordering::Greater => trf!("{} points ahead of plan", d),
        std::cmp::Ordering::Less => trf!("{} points behind plan", -d),
        std::cmp::Ordering::Equal => tr("Right on plan").into(),
    }
}

/// A done-or-not box: filled with the goal's colour and ticked when done.
pub(crate) fn paint_check(
    p: &egui::Painter,
    r: Rect,
    done: bool,
    color: Color32,
    t: &Theme,
    hovered: bool,
) {
    if done {
        p.rect_filled(r, 5.0, color);
        let a = r.left_center() + vec2(r.width() * 0.22, 0.0);
        let b = r.center_bottom() + vec2(-r.width() * 0.08, -r.height() * 0.28);
        let c = r.right_top() + vec2(-r.width() * 0.22, r.height() * 0.3);
        p.line(vec![a, b, c], Stroke::new(2.0, Color32::WHITE));
    } else {
        p.rect_filled(r, 5.0, if hovered { t.hover } else { t.card });
        p.rect_stroke(r, 5.0, Stroke::new(1.5, t.text3), egui::StrokeKind::Inside);
    }
}

/// "+ Record" on a number target: type the value it has reached now.
fn record_popup(
    ui: &mut Ui,
    t: &Theme,
    resp: &Response,
    gid: u64,
    i: usize,
    current: f64,
    name: &str,
    act: &mut Option<GoalAct>,
) {
    let id = ui.id().with(("goal record popup", gid, i));
    let value_id = id.with("value");
    if resp.clicked() {
        ui.data_mut(|d| d.insert_temp(value_id, current));
    }
    popup(ui, resp, id, 240.0, |ui| {
        ui.label(RichText::new(name).strong());
        ui.add_space(4.0);
        let mut v: f64 = ui.data(|d| d.get_temp(value_id)).unwrap_or(current);
        // Enter in the box saves, as in the app's other one-line boxes
        let enter = ui
            .horizontal(|ui| {
                ui.label(tr("Now"));
                let r = ui.add(number_box(&mut v));
                name_it(&r, "Value now");
                r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
            })
            .inner;
        ui.data_mut(|d| d.insert_temp(value_id, v));
        ui.add_space(6.0);
        let save = primary_button(ui, t, tr("Save"));
        name_it(&save, "Save value");
        if save.clicked() || enter {
            *act = Some(GoalAct::SetNumber { g: gid, i, to: v });
            close_popup(ui);
        }
        None::<bool>
    });
}

/// Where each column of the rows view starts.
fn row_cols(w: f32) -> [f32; 5] {
    let fixed = 120.0 + 120.0 + 130.0;
    let flex = (w - fixed).max(300.0);
    let c1 = flex * 0.48;
    let c2 = c1 + flex * 0.52;
    [16.0, c1, c2, c2 + 120.0, c2 + 240.0]
}

/// The trophy's colours: (fill, ink).
fn gold(t: &Theme) -> (Color32, Color32) {
    if t.dark {
        (rgb([70, 55, 15]), rgb([250, 205, 100]))
    } else {
        (rgb([255, 244, 214]), rgb([143, 98, 0]))
    }
}

/// How many goals were achieved in each month of `year`.
fn achieved_by_month(done: &[&GoalRow], year: i32) -> [usize; 12] {
    let mut out = [0; 12];
    for r in done {
        if let Some(d) = r.g.done_on.filter(|d| d.year() == year) {
            out[d.month0() as usize] += 1;
        }
    }
    out
}

/// This year's months in a row, each filled in gold by how many goals were
/// achieved in it and the months still ahead fainter, with January, this
/// month and December named under them. Hovering a month says how many.
fn month_strip(ui: &mut Ui, t: &Theme, by_month: &[usize; 12], day: NaiveDate) {
    let (_, ink) = gold(t);
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 8.0 + 5.0 + 14.0), Sense::hover());
    let gap = 3.0;
    let seg = (w - gap * 11.0) / 12.0;
    let most = by_month.iter().copied().max().unwrap_or(0).max(1);
    let now = day.month0() as usize;
    let font = FontId::proportional(11.0);
    // the labels placed so far, left to right, so none overlaps another
    let mut taken: Vec<(f32, f32)> = vec![];
    for (m, n) in by_month.iter().copied().enumerate() {
        let x = r.left() + m as f32 * (seg + gap);
        let bar = Rect::from_min_size(pos2(x, r.top()), vec2(seg, 8.0));
        let first = NaiveDate::from_ymd_opt(day.year(), m as u32 + 1, 1).unwrap_or(day);
        let resp = ui.interact(
            bar.expand2(vec2(gap / 2.0, 6.0)),
            ui.id().with(("achieved month", m)),
            Sense::hover(),
        );
        name_it(&resp, &format!("Achieved in {}", first.format("%b")));
        let p = ui.painter();
        let track = if m > now {
            t.hover.gamma_multiply(0.5)
        } else {
            t.hover
        };
        p.rect_filled(bar, 4.0, track);
        if n > 0 {
            let k = 0.45 + 0.55 * n as f32 / most as f32;
            p.rect_filled(bar, 4.0, ink.gamma_multiply(k));
        }
        let label = first.text("%b");
        if m == 0 || m == now || m == 11 {
            let g = p.layout_no_wrap(
                label.clone(),
                font.clone(),
                if m == now { t.text2 } else { t.text3 },
            );
            let gx = match m {
                0 => bar.left(),
                11 => bar.right() - g.size().x,
                _ => bar.center().x - g.size().x / 2.0,
            };
            let span = (gx - 4.0, gx + g.size().x + 4.0);
            if !taken.iter().any(|s| span.0 < s.1 && s.0 < span.1) {
                taken.push(span);
                p.galley(pos2(gx, bar.bottom() + 5.0), g, t.text3);
            }
        }
        resp.on_hover_text(trf!("{}: {} achieved", label, n));
    }
}

fn section_title(ui: &mut Ui, t: &Theme, text: &str, n: usize) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).size(16.0).strong());
        egui::Frame::new()
            .fill(t.hover)
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(8, 1))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(n.to_string())
                        .size(12.5)
                        .color(t.text2)
                        .strong(),
                );
            });
    });
}

/// All / Mine / Achieved, each with how many.
fn filter_tabs(ui: &mut Ui, t: &Theme, on: u8, counts: [usize; 3]) -> Option<u8> {
    let mut picked = None;
    let labels = ["All", "Mine", "Achieved"];
    egui::Frame::new()
        .fill(t.hover)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            // sized to its buttons: in the header's right-to-left row they
            // go in from the right, so the last first
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let rtl = ui.layout().prefer_right_to_left();
                let order: Vec<usize> = if rtl { vec![2, 1, 0] } else { vec![0, 1, 2] };
                for i in order {
                    let l = &labels[i];
                    let sel = on as usize == i;
                    let text = format!("{} {}", tr(l), counts[i]);
                    let b = egui::Button::new(RichText::new(text).size(13.0).color(if sel {
                        t.text
                    } else {
                        t.text2
                    }))
                    .fill(if sel { t.card } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE)
                    .corner_radius(8)
                    .min_size(vec2(0.0, 30.0));
                    let r = ui.add(b);
                    name_it(&r, &format!("Show {}", l));
                    if r.clicked() {
                        picked = Some(i as u8);
                    }
                }
            });
        });
    picked
}

/// Cards / rows.
fn view_switch(ui: &mut Ui, t: &Theme, list: bool) -> Option<&'static str> {
    let mut picked = None;
    egui::Frame::new()
        .fill(t.hover)
        .corner_radius(10)
        .inner_margin(egui::Margin::same(3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let mut both = vec![
                    ("", "apps", "Show as cards", !list),
                    ("list", "list", "Show as list", list),
                ];
                if ui.layout().prefer_right_to_left() {
                    both.reverse();
                }
                for (kind, icon, name, sel) in both {
                    let (r, resp) = ui.allocate_exact_size(vec2(34.0, 30.0), Sense::click());
                    if sel {
                        ui.painter().rect_filled(r, 8.0, t.card);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 8.0, t.card.gamma_multiply(0.5));
                    }
                    icons::line_icon(
                        ui.painter(),
                        icon,
                        r.center(),
                        if sel { t.text } else { t.text2 },
                    );
                    name_it(&resp, name);
                    if resp.clicked() {
                        picked = Some(kind);
                    }
                }
            });
        });
    picked
}

/// Achieved goals: a tinted card each, and a dashed one for the goal in
/// progress closest to done.
fn achieved_shelf(
    ui: &mut Ui,
    t: &Theme,
    done: &[&GoalRow],
    next: Option<&GoalRow>,
) -> Option<GoalAct> {
    let mut act = None;
    let gap = 14.0;
    let w = ui.available_width();
    let cols = if w > 980.0 {
        3
    } else if w > 620.0 {
        2
    } else {
        1
    };
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    let h = 76.0;
    let mut items: Vec<Option<&GoalRow>> = done.iter().map(|r| Some(*r)).collect();
    if next.is_some() {
        items.push(None);
    }
    if items.is_empty() {
        ui.label(RichText::new(tr("Goals reaching 100% gather here.")).color(t.text2));
        return None;
    }
    for chunk in items.chunks(cols) {
        let (row, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        for (i, it) in chunk.iter().enumerate() {
            let r = Rect::from_min_size(row.min + vec2(i as f32 * (cw + gap), 0.0), vec2(cw, h));
            match it {
                Some(g) => {
                    let color = rgb(g.g.color);
                    let resp = ui.interact(r, ui.id().with(("achieved", g.g.id)), Sense::click());
                    name_it(&resp, &format!("Achieved {}", g.g.name));
                    if resp.clicked() {
                        act = Some(GoalAct::Open(g.g.id));
                    }
                    let p = ui.painter();
                    let tint = if t.dark {
                        color.gamma_multiply(0.22)
                    } else {
                        color.gamma_multiply(0.10)
                    };
                    p.rect_filled(r, 14.0, t.card);
                    p.rect_filled(r, 14.0, tint);
                    p.rect_stroke(
                        r,
                        14.0,
                        Stroke::new(
                            1.0,
                            if resp.hovered() {
                                color
                            } else {
                                color.gamma_multiply(0.35)
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    let c = pos2(r.left() + 38.0, r.center().y);
                    p.circle_filled(c, 24.0, t.card);
                    p.circle_filled(c, 20.0, color);
                    icons::line_icon(p, "check", c, Color32::WHITE);
                    let nf = FontId::proportional(15.0);
                    p.text(
                        pos2(r.left() + 72.0, r.center().y - 9.0),
                        Align2::LEFT_CENTER,
                        crate::widgets::fit_text(p, &g.g.name, &nf, cw - 150.0),
                        nf,
                        t.text,
                    );
                    let when = match (g.g.done_on, g.g.due) {
                        (Some(d), Some(due)) if due > d => trf!(
                            "Achieved {}, {} days early",
                            d.text("%b %-d"),
                            (due - d).num_days()
                        ),
                        (Some(d), _) => trf!("Achieved {}", d.text("%b %-d")),
                        _ => tr("Achieved").into(),
                    };
                    p.text(
                        pos2(r.left() + 72.0, r.center().y + 11.0),
                        Align2::LEFT_CENTER,
                        when,
                        FontId::proportional(12.5),
                        t.text2,
                    );
                    let n = g.g.targets.len();
                    p.text(
                        pos2(r.right() - 16.0, r.center().y),
                        Align2::RIGHT_CENTER,
                        format!("{}/{}", n, n),
                        FontId::proportional(12.5),
                        t.text2,
                    );
                }
                None => {
                    let g = next.unwrap();
                    let resp = ui.interact(r, ui.id().with(("next goal", g.g.id)), Sense::click());
                    name_it(&resp, "Closest goal");
                    if resp.clicked() {
                        act = Some(GoalAct::Open(g.g.id));
                    }
                    let p = ui.painter();
                    paint_dashed_rect(
                        p,
                        r.shrink(1.0),
                        14.0,
                        if resp.hovered() { t.text3 } else { t.border },
                    );
                    let c = pos2(r.left() + 38.0, r.center().y);
                    crate::widgets::paint_dashed(p, c, 20.0, t.text3);
                    icons::line_icon(p, "flag", c, t.text2);
                    p.text(
                        pos2(r.left() + 72.0, r.center().y - 9.0),
                        Align2::LEFT_CENTER,
                        tr("Closest to done"),
                        FontId::proportional(14.0),
                        t.text,
                    );
                    let nf = FontId::proportional(12.5);
                    let line = format!("{} · {}%", g.g.name, g.pct);
                    p.text(
                        pos2(r.left() + 72.0, r.center().y + 11.0),
                        Align2::LEFT_CENTER,
                        crate::widgets::fit_text(p, &line, &nf, cw - 90.0),
                        nf,
                        t.text2,
                    );
                }
            }
        }
        ui.add_space(gap);
    }
    act
}

/// Lays out `add` left to right inside `rect` without taking room in `ui`
/// (a scope would move `ui`'s cursor to the bottom of `rect`).
fn place<R>(ui: &mut Ui, rect: Rect, add: impl FnOnce(&mut Ui) -> R) -> R {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    add(&mut child)
}

/// A rounded rectangle outlined in dashes.
fn paint_dashed_rect(p: &egui::Painter, r: Rect, radius: f32, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let (dash, gap) = (6.0, 4.0);
    let seg = |a: Pos2, b: Pos2| {
        p.extend(egui::Shape::dashed_line(&[a, b], stroke, dash, gap));
    };
    seg(
        pos2(r.left() + radius, r.top()),
        pos2(r.right() - radius, r.top()),
    );
    seg(
        pos2(r.left() + radius, r.bottom()),
        pos2(r.right() - radius, r.bottom()),
    );
    seg(
        pos2(r.left(), r.top() + radius),
        pos2(r.left(), r.bottom() - radius),
    );
    seg(
        pos2(r.right(), r.top() + radius),
        pos2(r.right(), r.bottom() - radius),
    );
    for (c, a0) in [
        (pos2(r.left() + radius, r.top() + radius), 180.0f32),
        (pos2(r.right() - radius, r.top() + radius), 270.0),
        (pos2(r.right() - radius, r.bottom() - radius), 0.0),
        (pos2(r.left() + radius, r.bottom() - radius), 90.0),
    ] {
        let pts: Vec<Pos2> = (0..=6)
            .map(|i| {
                let a = (a0 + 90.0 * i as f32 / 6.0).to_radians();
                c + vec2(a.cos(), a.sin()) * radius
            })
            .collect();
        p.add(egui::Shape::line(pts, stroke));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_grouped_and_trimmed() {
        assert_eq!(fmt_num(7200.0), "7,200");
        assert_eq!(fmt_num(10000.0), "10,000");
        assert_eq!(fmt_num(4.6), "4.6");
        assert_eq!(fmt_num(0.25), "0.25");
        assert_eq!(fmt_num(1234567.5), "1,234,567.5");
        assert_eq!(fmt_num(-3.0), "-3");
        assert_eq!(fmt_num(2.999), "3");
        assert_eq!(fmt_num(-0.001), "0", "no minus on what shows as nothing");
        assert_eq!(fmt_num(-0.5), "-0.5");
        assert_eq!(fmt_num(-1234.567), "-1,234.57");
        assert_eq!(fmt_num(NUM_MAX), "1,000,000,000,000,000");
    }

    #[test]
    fn goal_numbers_stay_finite_and_read_back() {
        assert_eq!(parse_number("12.5"), Some(12.5));
        assert_eq!(parse_number(" \u{2212}3 "), Some(-3.0));
        assert_eq!(parse_number("1,234,567.5"), Some(1_234_567.5));
        for s in ["inf", "-inf", "NaN", "1e999", "", "abc"] {
            assert_eq!(parse_number(s), None, "{s:?}");
        }
        // a goal saved with infinity in it (written as null) still reads
        let g: Goal = serde_json::from_value(serde_json::json!({
            "id": 1, "name": "G", "color": [0, 0, 0], "due": null, "owner": 1,
            "targets": [{"name": "N", "kind": {"Number": {
                "start": 0.0, "target": null, "current": null, "unit": ""}}}],
            "log": [{"ts": 0, "who": 1, "pct": 0, "what": {"Number": {
                "target": "N", "from": 1.0, "to": null, "unit": ""}}}]
        }))
        .expect("a goal with nulls for numbers reads");
        assert!(matches!(
            g.targets[0].kind,
            TargetKind::Number { target, current, .. } if target == 0.0 && current == 0.0
        ));
    }

    #[test]
    fn a_goal_due_the_day_it_starts_uses_no_time_before_that_day() {
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        let g = Goal {
            start: Some(d(12, 1)),
            due: Some(d(12, 1)),
            ..Default::default()
        };
        assert_eq!(g.time_used(d(11, 21)), Some(0.0), "ten days left");
        assert_eq!(g.pace(0.0, d(11, 21)), Some(Pace::OnTrack));
        assert_eq!(g.time_used(d(12, 1)), Some(1.0), "due today");
        assert_eq!(g.pace(0.0, d(12, 1)), Some(Pace::AtRisk));
        assert_eq!(g.time_used(d(12, 2)), Some(1.0));
        // due before it starts: its time is up once the due day comes
        let g = Goal {
            start: Some(d(12, 10)),
            due: Some(d(12, 1)),
            ..Default::default()
        };
        assert_eq!(g.time_used(d(11, 21)), Some(0.0));
        assert_eq!(g.time_used(d(12, 5)), Some(1.0));
        // the usual: a share of the days, none before it starts
        let g = Goal {
            start: Some(d(11, 1)),
            due: Some(d(11, 11)),
            ..Default::default()
        };
        assert_eq!(g.time_used(d(10, 20)), Some(0.0));
        assert_eq!(g.time_used(d(11, 6)), Some(0.5));
        assert_eq!(g.time_used(d(11, 30)), Some(1.0));
        assert_eq!(Goal::default().time_used(d(11, 6)), None);
    }

    #[test]
    fn a_percent_reads_100_only_when_all_done() {
        assert_eq!(pct_of(0.995), 99);
        assert_eq!(pct_of(0.9999), 99);
        assert_eq!(pct_of(1.0), 100);
        assert_eq!(pct_of(0.5), 50);
        assert_eq!(pct_of(0.745), 75, "other milestones keep the rounding");
        assert_eq!(pct_of(f32::NAN), 0);
    }

    #[test]
    fn milestones_and_days_left_read_right() {
        assert_eq!(milestone_at(24), 0);
        assert_eq!(milestone_at(50), 50);
        assert_eq!(milestone_at(99), 75);
        assert_eq!(milestone_at(100), 100);
        let d = NaiveDate::from_ymd_opt(2026, 5, 10).unwrap();
        assert_eq!(dday(d + chrono::Duration::days(7), d), "D-7");
        assert_eq!(dday(d - chrono::Duration::days(2), d), "D+2");
        assert_eq!(pace_text(60, 50), "10 points ahead of plan");
        assert_eq!(pace_text(40, 50), "10 points behind plan");
    }
}
