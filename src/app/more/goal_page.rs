// A goal's own page: its ring, pace and milestones on top, its targets with
// the controls that move them (a number up or down, done or not, tasks
// ticked off) and its history. Its settings open in a dialog from the "⋯"
// by its name; a milestone reached floats over the page for a few seconds.

use super::goals::*;
use super::*;
use egui::Pos2;

/// What a target shows on the page, worked out before drawing.
struct TargetView {
    name: String,
    kind: u8,
    st: TargetState,
    start: f64,
    target: f64,
    current: f64,
    unit: String,
    /// tasks target: (id, name, closed, first assignee)
    tasks: Vec<(u64, String, bool, Option<u64>)>,
}

impl App {
    pub(crate) fn goal_page(&mut self, ui: &mut Ui, gid: u64) {
        let Some(gi) = self.data.extra.goals.iter().position(|g| g.id == gid) else {
            return;
        };
        let t = self.t();
        let day = today();
        let row = self.goal_row(&self.data.extra.goals[gi], day);
        let g = row.g.clone();
        let color = rgb(g.color);
        let views: Vec<TargetView> = g
            .targets
            .iter()
            .zip(&row.targets)
            .map(|(tg, st)| {
                let (kind, start, target, current, unit, tasks) = match &tg.kind {
                    TargetKind::Number {
                        start,
                        target,
                        current,
                        unit,
                    } => (0, *start, *target, *current, unit.clone(), vec![]),
                    TargetKind::TrueFalse { .. } => (1, 0.0, 0.0, 0.0, String::new(), vec![]),
                    TargetKind::Tasks { ids } => {
                        let tasks = ids
                            .iter()
                            .filter_map(|id| self.task(*id))
                            .map(|x| {
                                (
                                    x.id,
                                    x.name.clone(),
                                    self.data.is_closed(x),
                                    x.assignees.first().copied(),
                                )
                            })
                            .collect();
                        (2, 0.0, 0.0, 0.0, String::new(), tasks)
                    }
                };
                TargetView {
                    name: tg.name.clone(),
                    kind,
                    st: st.clone(),
                    start,
                    target,
                    current,
                    unit,
                    tasks,
                }
            })
            .collect();
        let mut act: Option<GoalAct> = None;
        let mut page_act: Option<PageAct> = None;
        let page = ui.max_rect();
        let mut head_act: Option<PageAct> = None;
        goals_frame(
            ui,
            ("goal_page", gid),
            |ui| {
                // ---- back to the Goals page
                let b =
                    icons::icon_button(ui, &t, "chevron_left", tr("Goals"), "Back to goals", false);
                if b.clicked() {
                    act = Some(GoalAct::Back);
                }
                ui.label(RichText::new("/").color(t.text3));
                // the name gives way to the settings button
                let name_w = (ui.available_width() - 40.0).max(0.0);
                ui.scope(|ui| {
                    ui.set_max_width(name_w);
                    ui.add(egui::Label::new(RichText::new(&g.name).color(t.text)).truncate());
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let b = icons::icon_button(ui, &t, "dots", "", "Goal settings", false);
                    if b.clicked() {
                        head_act = Some(PageAct::Settings);
                    }
                });
            },
            |ui| {
                let w = ui.available_width();
                if let Some(a) = self.goal_hero(ui, &t, &row, day) {
                    page_act = Some(a);
                }
                ui.add_space(18.0);
                let wide = w > 900.0;
                let gap = 18.0;
                let left_w = if wide { (w - gap) * 0.62 } else { w };
                let right_w = if wide { w - gap - left_w } else { w };
                let left = |ui: &mut Ui, me: &mut App| -> Option<PageAct> {
                    ui.set_width(left_w);
                    me.goal_targets_card(ui, &t, gi, &g, &views, color)
                };
                if wide {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        if let Some(a) = ui.vertical(|ui| left(ui, self)).inner {
                            page_act = Some(a);
                        }
                        ui.vertical(|ui| {
                            ui.set_width(right_w);
                            self.goal_history_card(ui, &t, &g);
                        });
                    });
                } else {
                    if let Some(a) = left(ui, self) {
                        page_act = Some(a);
                    }
                    ui.add_space(gap);
                    self.goal_history_card(ui, &t, &g);
                }
            },
        );
        if head_act.is_some() {
            page_act = head_act;
        }
        if let Some((_, m, at)) = self.goal_banner.filter(|b| b.0 == gid) {
            if now_ts() - at > MILESTONE_SECS {
                self.goal_banner = None;
            } else if self.modal.is_none() {
                // (not over a dialog: the goal's settings, say)
                let left: Vec<&str> = views
                    .iter()
                    .filter(|v| !v.st.done)
                    .map(|v| v.name.as_str())
                    .collect();
                // above the app's own toast when one is up
                let lift = if self.toast.is_some() { 64.0 } else { 0.0 };
                match milestone_float(ui.ctx(), &t, page, lift, m, &left) {
                    Float::Closed => page_act = Some(PageAct::DismissBanner),
                    // read while the pointer is on it: it stays
                    Float::Held => self.goal_banner = Some((gid, m, now_ts())),
                    Float::Shown => {}
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(500));
            }
        }
        if let Some(a) = act {
            self.goal_apply(a);
        }
        if let Some(a) = page_act {
            self.goal_page_apply(gid, a);
        }
    }

    /// The top of the page: ring, name, pace, dates, progress against time
    /// and the milestones.
    fn goal_hero(&self, ui: &mut Ui, t: &Theme, row: &GoalRow, day: NaiveDate) -> Option<PageAct> {
        let mut act = None;
        let g = &row.g;
        let color = rgb(g.color);
        let done = row.pct >= 100;
        // an achieved goal's box takes a light wash of its colour
        let fill = if done {
            let a = if t.dark { 0.18 } else { 0.07 };
            let mix = |c: u8, k: u8| (c as f32 * (1.0 - a) + k as f32 * a).round() as u8;
            Color32::from_rgb(
                mix(t.card.r(), color.r()),
                mix(t.card.g(), color.g()),
                mix(t.card.b(), color.b()),
            )
        } else {
            t.card
        };
        let w = ui.available_width();
        let resp = egui::Frame::new()
            .fill(fill)
            .stroke(Stroke::new(
                1.0,
                if done {
                    color.gamma_multiply(0.4)
                } else {
                    t.border
                },
            ))
            .corner_radius(18)
            .inner_margin(egui::Margin::same(26))
            .show(ui, |ui| {
                // the margins and the 1 pt outline on each side
                ui.set_width(w - 54.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 28.0;
                    // ring
                    let (rr, _) = ui.allocate_exact_size(vec2(140.0, 140.0), Sense::hover());
                    paint_ring(
                        ui.painter(),
                        rr.center(),
                        60.0,
                        12.0,
                        row.pct as f32 / 100.0,
                        color,
                        t.hover,
                    );
                    if done {
                        ui.painter().circle_filled(rr.center(), 34.0, color);
                        paint_tick(ui.painter(), rr.center(), 15.0, Color32::WHITE, 3.5);
                    } else {
                        ui.painter().text(
                            rr.center() - vec2(0.0, 6.0),
                            Align2::CENTER_CENTER,
                            format!("{}%", row.pct),
                            FontId::proportional(32.0),
                            t.text,
                        );
                        ui.painter().text(
                            rr.center() + vec2(0.0, 22.0),
                            Align2::CENTER_CENTER,
                            tr("progress"),
                            FontId::proportional(12.0),
                            t.text2,
                        );
                    }
                    let info_w =
                        (ui.available_width() - if w > 760.0 { 250.0 } else { 0.0 }).max(240.0);
                    ui.vertical(|ui| {
                        ui.set_width(info_w);
                        ui.spacing_mut().item_spacing.y = 10.0;
                        if done {
                            let line = match (g.done_on, g.due) {
                                (Some(d), Some(due)) if due > d => trf!(
                                    "Goal achieved, {} days before the deadline",
                                    (due - d).num_days()
                                ),
                                _ => tr("Goal achieved!").into(),
                            };
                            ui.label(
                                RichText::new(line)
                                    .size(14.0)
                                    .strong()
                                    .color(pace_colors(Pace::Done, t).1),
                            );
                        }
                        ui.add(egui::Label::new(RichText::new(&g.name).size(26.0).strong()).wrap());
                        ui.horizontal(|ui| {
                            if let Some(p) = row.pace {
                                pace_pill(ui, t, p);
                            }
                            let line = match (row.pace, row.expect) {
                                (Some(Pace::Done), _) => {
                                    trf!("All {} targets met", g.targets.len())
                                }
                                (Some(_), Some(e)) => trf!(
                                    "{}% of the time used, {}% done · {}",
                                    e,
                                    row.pct,
                                    pace_text(row.pct, e)
                                ),
                                _ => tr("Add a start and due date to see the pace").into(),
                            };
                            ui.label(RichText::new(line).size(13.0).color(t.text2));
                        });
                        ui.horizontal_wrapped(|ui| {
                            if let Some(m) = self.data.member(g.owner) {
                                info_chip(ui, t, |ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                                    paint_avatar(ui.painter(), r.center(), 10.0, m, t.card);
                                    ui.label(RichText::new(m.shown()).size(13.0));
                                });
                            }
                            if let Some(due) = g.due {
                                info_chip(ui, t, |ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(16.0, 20.0), Sense::hover());
                                    icons::line_icon(ui.painter(), "calendar", r.center(), t.text2);
                                    ui.label(RichText::new(due.text("%b %-d")).size(13.0));
                                    if !done {
                                        let fg = match row.pace {
                                            Some(p @ (Pace::Behind | Pace::AtRisk)) => {
                                                pace_colors(p, t).1
                                            }
                                            _ => t.text,
                                        };
                                        ui.label(
                                            RichText::new(dday(due, day))
                                                .size(13.0)
                                                .strong()
                                                .color(fg),
                                        );
                                    }
                                });
                            }
                            if let Some(s) = g.start {
                                info_chip(ui, t, |ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(16.0, 20.0), Sense::hover());
                                    icons::line_icon(
                                        ui.painter(),
                                        "hourglass",
                                        r.center(),
                                        t.text2,
                                    );
                                    ui.label(
                                        RichText::new(trf!("Since {}", s.text("%b %-d")))
                                            .size(13.0),
                                    );
                                });
                            }
                        });
                        if let Some(e) = row.expect {
                            let bw = (info_w - 100.0).min(420.0);
                            for (label, frac, c, name) in [
                                (
                                    tr("Progress"),
                                    row.pct as f32 / 100.0,
                                    color,
                                    "Progress bar",
                                ),
                                (tr("Time"), e as f32 / 100.0, t.text3, "Time bar"),
                            ] {
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        vec2(40.0, 16.0),
                                        egui::Label::new(
                                            RichText::new(label).size(12.5).color(t.text2),
                                        ),
                                    );
                                    let (r, resp) =
                                        ui.allocate_exact_size(vec2(bw, 8.0), Sense::hover());
                                    paint_bar(ui.painter(), r, frac, c, t.hover);
                                    name_it(&resp, name);
                                    ui.label(
                                        RichText::new(format!("{}%", pct_of(frac)))
                                            .size(12.5)
                                            .strong(),
                                    );
                                });
                            }
                        }
                        if done {
                            let (share, next) = achieved_buttons(ui, color);
                            name_it(&share, "Share to team chat");
                            if share.clicked() {
                                act = Some(PageAct::Share);
                            }
                            name_it(&next, "Set the next goal");
                            if next.clicked() {
                                act = Some(PageAct::NextGoal);
                            }
                        }
                    });
                    if w > 760.0 {
                        ui.vertical(|ui| {
                            ui.set_width(210.0);
                            ui.label(
                                RichText::new(tr("Milestones"))
                                    .size(12.5)
                                    .strong()
                                    .color(t.text2),
                            );
                            ui.add_space(4.0);
                            let mut next_found = false;
                            for m in MILESTONES {
                                let reached = row.pct >= m;
                                let next = !reached && !next_found;
                                next_found |= next;
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(28.0, 40.0), Sense::hover());
                                    let c = r.center();
                                    let p = ui.painter();
                                    if reached {
                                        p.circle_filled(c, 13.0, color);
                                        paint_tick(p, c, 6.0, Color32::WHITE, 2.4);
                                    } else if next {
                                        p.circle_stroke(c, 12.0, Stroke::new(2.0, color));
                                    } else {
                                        crate::widgets::paint_dashed(p, c, 12.0, t.text3);
                                    }
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        ui.add_space(3.0);
                                        let label = if m == 100 {
                                            tr("100% achieved").to_string()
                                        } else {
                                            format!("{}%", m)
                                        };
                                        ui.label(
                                            RichText::new(label).size(14.0).strong().color(
                                                if reached || next { t.text } else { t.text2 },
                                            ),
                                        );
                                        let sub = if reached && m == 100 {
                                            // "100% achieved" says it already: just the day
                                            g.done_on.map(|d| d.text("%b %-d")).unwrap_or_default()
                                        } else if reached {
                                            g.milestone_day(m)
                                                .and_then(local)
                                                .map(|l| {
                                                    trf!(
                                                        "Reached {}",
                                                        l.date_naive().text("%b %-d")
                                                    )
                                                })
                                                .unwrap_or_else(|| tr("Reached").into())
                                        } else if next {
                                            trf!("{} points to go", m - row.pct)
                                        } else {
                                            String::new()
                                        };
                                        ui.label(RichText::new(sub).size(12.0).color(t.text2));
                                    });
                                });
                            }
                        });
                    }
                });
            });
        resp.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Goal overview")
        });
        act
    }

    /// The targets, each with what moves it.
    fn goal_targets_card(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        gi: usize,
        g: &Goal,
        views: &[TargetView],
        color: Color32,
    ) -> Option<PageAct> {
        let mut act = None;
        let gid = g.id;
        let w = ui.available_width();
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(16)
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                // the margins and the 1 pt outline on each side
                ui.set_width(w - 38.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Targets")).size(16.0).strong());
                    egui::Frame::new()
                        .fill(t.hover)
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(8, 1))
                        .show(ui, |ui| {
                            ui.label(RichText::new(views.len().to_string()).size(12.5).color(t.text2).strong());
                        });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let r = icons::icon_button(ui, t, "plus", tr("Target"), "Add target", true);
                        popup(ui, &r, ui.id().with(("add target", gid)), 200.0, |ui| {
                            for (k, label) in [(0u8, "Number"), (1, "Done or not"), (2, "Tasks")] {
                                let b = menu_row_plain(ui, t, tr(label), false);
                                name_it(&b, &format!("Add {} target", label));
                                if b.clicked() {
                                    act = Some(PageAct::AddTarget(k));
                                    close_popup(ui);
                                }
                            }
                            None::<bool>
                        });
                    });
                });
                ui.add_space(10.0);
                if views.is_empty() {
                    ui.label(RichText::new(tr("Add a target to measure this goal: a number, done or not, or a set of tasks.")).color(t.text2));
                }
                for (i, v) in views.iter().enumerate() {
                    egui::Frame::new()
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(12)
                        .inner_margin(egui::Margin::symmetric(16, 14))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if let Some(a) = self.goal_target_view(ui, t, gi, gid, i, v, color) {
                                act = Some(a);
                            }
                        });
                    ui.add_space(10.0);
                }
            });
        act
    }

    #[allow(clippy::too_many_arguments)]
    fn goal_target_view(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        gi: usize,
        gid: u64,
        i: usize,
        v: &TargetView,
        color: Color32,
    ) -> Option<PageAct> {
        let mut act = None;
        let accent = pace_colors(Pace::Done, t);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter().rect_filled(r, 10.0, accent.0);
            let icon = ["hash", "check", "list"][v.kind as usize];
            icons::line_icon(ui.painter(), icon, r.center(), accent.1);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let name_w = (ui.available_width() - 190.0).max(120.0);
                let mut name = self.data.extra.goals[gi].targets[i].name.clone();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut name)
                        .frame(false)
                        .font(FontId::proportional(15.0))
                        .desired_width(name_w)
                        .margin(egui::Margin::ZERO),
                );
                name_field(&r, &format!("Target name {}", v.name));
                if r.changed() {
                    self.data.extra.goals[gi].targets[i].name = name;
                    self.dirty = true;
                }
                let sub = match v.kind {
                    0 => trf!(
                        "Number · {} to {} {}",
                        fmt_num(v.start),
                        fmt_num(v.target),
                        v.unit
                    )
                    .trim_end()
                    .to_string(),
                    1 => tr("Done or not").into(),
                    _ => trf!("Tasks · {} picked", v.tasks.len()),
                };
                ui.label(RichText::new(sub).size(12.5).color(t.text2));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let x = close_x(ui, 28.0, t.text3);
                name_it(&x, &format!("Remove target {}", v.name));
                if x.clicked() {
                    act = Some(PageAct::RemoveTarget(i));
                }
                match v.kind {
                    1 => {
                        let b = done_toggle(ui, t, v.st.done, color);
                        name_it(&b, &format!("Done {}", v.name));
                        if b.clicked() {
                            act = Some(PageAct::Goal(GoalAct::ToggleDone { g: gid, i }));
                        }
                    }
                    _ => {
                        if !v.st.unit.is_empty() {
                            ui.label(RichText::new(&v.st.unit).size(14.0).color(t.text2));
                        }
                        ui.label(RichText::new(&v.st.value).size(22.0).strong());
                    }
                }
            });
        });
        if v.kind == 1 {
            return act;
        }
        ui.add_space(8.0);
        let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
        paint_bar(ui.painter(), bar, v.st.frac, color, t.hover);
        ui.add_space(8.0);
        if v.kind == 0 {
            ui.horizontal(|ui| {
                for (label, d, name) in [
                    ("\u{2212}", -1.0, "Minus 1"),
                    ("+1", 1.0, "Plus 1"),
                    ("+10", 10.0, "Plus 10"),
                ] {
                    let b = ui.add(
                        egui::Button::new(RichText::new(label).strong())
                            .fill(t.card)
                            .stroke(Stroke::new(1.0, t.border))
                            .corner_radius(9)
                            .min_size(vec2(40.0, 32.0)),
                    );
                    name_it(&b, &format!("{} {}", name, v.name));
                    if b.clicked() {
                        act = Some(PageAct::Goal(GoalAct::SetNumber {
                            g: gid,
                            i,
                            to: v.current + d,
                        }));
                    }
                }
                let e = ui.add(
                    egui::Button::new(RichText::new(tr("Edit")).color(t.text2))
                        .fill(t.card)
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(9)
                        .min_size(vec2(0.0, 32.0)),
                );
                name_it(&e, &format!("Edit {}", v.name));
                popup(ui, &e, ui.id().with(("edit number", gid, i)), 260.0, |ui| {
                    let mut ch = false;
                    if let Some(TargetKind::Number {
                        start,
                        target,
                        current,
                        unit,
                    }) = self.data.extra.goals[gi]
                        .targets
                        .get_mut(i)
                        .map(|x| &mut x.kind)
                    {
                        egui::Grid::new("number target")
                            .num_columns(2)
                            .spacing([10.0, 8.0])
                            .show(ui, |ui| {
                                ui.label(tr("Now"));
                                // recorded like + Record once it is let go
                                // (typed and left, or dragged and dropped):
                                // a line in the history, the milestone
                                // banner, Goal achieved. Escape drops it.
                                let now_id = egui::Id::new(("goal number now", gid, i));
                                let mut now: f64 =
                                    ui.data(|d| d.get_temp(now_id)).unwrap_or(*current);
                                let r = ui.add(number_box(&mut now));
                                name_it(&r, "Number now");
                                if r.dragged() || r.has_focus() {
                                    ui.data_mut(|d| d.insert_temp(now_id, now));
                                } else {
                                    ui.data_mut(|d| d.remove_temp::<f64>(now_id));
                                    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                    if now != *current && !esc {
                                        act = Some(PageAct::Goal(GoalAct::SetNumber {
                                            g: gid,
                                            i,
                                            to: now,
                                        }));
                                    }
                                }
                                ui.end_row();
                                ui.label(tr("Start value"));
                                ch |= ui.add(number_box(start)).changed();
                                ui.end_row();
                                ui.label(tr("Target value"));
                                ch |= ui.add(number_box(target)).changed();
                                ui.end_row();
                                ui.label(tr("Unit"));
                                ch |= ui.add(field(unit).desired_width(80.0)).changed();
                                ui.end_row();
                            });
                    }
                    if ch {
                        self.dirty = true;
                    }
                    None::<bool>
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let left = v.target - v.current;
                    let note = if v.st.done {
                        tr("Target met").to_string()
                    } else {
                        trf!(
                            "{} to go",
                            format!("{} {}", fmt_num(left.abs()), v.unit).trim_end()
                        )
                    };
                    ui.label(RichText::new(note).size(12.5).color(t.text2));
                });
            });
            return act;
        }
        // tasks: the open ones to tick off, the closed ones on request
        let show_done = self.goal_show_done == Some((gid, i));
        let open: Vec<_> = v.tasks.iter().filter(|x| !x.2).collect();
        let closed: Vec<_> = v.tasks.iter().filter(|x| x.2).collect();
        if !open.is_empty() {
            ui.label(
                RichText::new(tr("Still to do"))
                    .size(12.0)
                    .strong()
                    .color(t.text2),
            );
        }
        let rows: Vec<_> = if show_done {
            open.iter().chain(closed.iter()).copied().collect()
        } else {
            open.clone()
        };
        for (tid, name, is_closed, who) in rows {
            let (r, resp) =
                ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
            name_it(&resp, &format!("Tick {}", name));
            hover_bg(ui, &resp, r, 6.0);
            let p = ui.painter();
            let bx = Rect::from_center_size(pos2(r.left() + 12.0, r.center().y), vec2(18.0, 18.0));
            paint_check(p, bx, *is_closed, color, t, false);
            let nf = FontId::proportional(13.5);
            let txt = crate::widgets::fit_text(p, name, &nf, r.width() - 80.0);
            let tr_ = p.text(
                pos2(r.left() + 32.0, r.center().y),
                Align2::LEFT_CENTER,
                txt,
                nf,
                if *is_closed { t.text3 } else { t.text },
            );
            if *is_closed {
                p.hline(tr_.x_range(), r.center().y, Stroke::new(1.0, t.text3));
            }
            if let Some(m) = who.and_then(|w| self.data.member(w)) {
                paint_avatar(p, pos2(r.right() - 14.0, r.center().y), 10.0, m, t.card);
            }
            p.hline(
                r.x_range(),
                r.bottom(),
                Stroke::new(1.0, t.border.gamma_multiply(0.6)),
            );
            if resp.clicked() {
                act = Some(PageAct::Goal(GoalAct::ToggleTask {
                    g: gid,
                    i,
                    task: *tid,
                }));
            }
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if !closed.is_empty() {
                let label = if show_done {
                    tr("Hide done tasks").to_string()
                } else {
                    trf!("Show {} done tasks", closed.len())
                };
                let b = ui.add(
                    egui::Label::new(RichText::new(label).size(13.0).strong().color(accent.1))
                        .sense(Sense::click()),
                );
                name_it(
                    &b,
                    if show_done {
                        "Hide done tasks"
                    } else {
                        "Show done tasks"
                    },
                );
                if b.clicked() {
                    act = Some(PageAct::ShowDone(if show_done {
                        None
                    } else {
                        Some((gid, i))
                    }));
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let all: Vec<(u64, String)> = self
                    .data
                    .tasks
                    .iter()
                    .filter(|x| x.parent.is_none() && !x.archived)
                    .map(|x| (x.id, x.name.clone()))
                    .collect();
                let r = ui.add(
                    egui::Button::new(RichText::new(tr("Pick tasks")).color(t.text2))
                        .fill(t.card)
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(9),
                );
                name_it(&r, &format!("Pick tasks for {}", v.name));
                popup(ui, &r, ui.id().with(("pick tasks", gid, i)), 300.0, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            for (id, name) in &all {
                                if let Some(TargetKind::Tasks { ids }) = self.data.extra.goals[gi]
                                    .targets
                                    .get_mut(i)
                                    .map(|x| &mut x.kind)
                                {
                                    let mut on = ids.contains(id);
                                    if check(ui, &mut on, name).changed() {
                                        if on {
                                            ids.push(*id);
                                        } else {
                                            ids.retain(|x| x != id);
                                        }
                                        self.dirty = true;
                                    }
                                }
                            }
                        });
                    None::<bool>
                });
            });
        });
        act
    }

    /// The goal's history, newest first.
    fn goal_history_card(&self, ui: &mut Ui, t: &Theme, g: &Goal) {
        let w = ui.available_width();
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(16)
            .inner_margin(egui::Margin::same(18))
            .show(ui, |ui| {
                // the margins and the 1 pt outline on each side
                ui.set_width(w - 38.0);
                ui.label(RichText::new(tr("History")).size(16.0).strong());
                ui.add_space(8.0);
                if g.log.is_empty() {
                    ui.label(
                        RichText::new(tr("Changes made here show up in the history."))
                            .color(t.text2),
                    );
                }
                for ev in g.log.iter().rev().take(8) {
                    ui.horizontal_top(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                        let p = ui.painter();
                        let person = self.data.member(ev.who);
                        match (&ev.what, person) {
                            (GoalChange::Milestone(_) | GoalChange::Tracked, _) | (_, None) => {
                                let (bg, fg) = pace_colors(Pace::Done, t);
                                p.circle_filled(r.center(), 14.0, bg);
                                icons::line_icon(p, "flag", r.center(), fg);
                            }
                            (_, Some(m)) => paint_avatar(p, r.center(), 14.0, m, t.card),
                        }
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            let who = person.map(|m| m.shown().to_string()).unwrap_or_default();
                            ui.add(
                                egui::Label::new(
                                    RichText::new(history_text(&ev.what, &who)).size(13.5),
                                )
                                .wrap(),
                            );
                            ui.label(RichText::new(fmt_ago(ev.ts)).size(12.0).color(t.text2));
                        });
                    });
                    ui.add_space(6.0);
                }
            });
    }

    /// Goal settings, opened from the "⋯" on the goal's page: name, dates,
    /// owner, colour and Delete. Changes apply as they are made. Returns
    /// true when it should close.
    pub(crate) fn goal_settings_body(&mut self, ui: &mut Ui, t: &Theme, gid: u64) -> bool {
        let Some(gi) = self.data.extra.goals.iter().position(|g| g.id == gid) else {
            return true;
        };
        let mut close = false;
        let mut delete = false;
        let w = ui.available_width();
        let members: Vec<(u64, String)> = self
            .data
            .members
            .iter()
            .map(|m| (m.id, m.shown().to_string()))
            .collect();
        let label = |ui: &mut Ui, s: &str| {
            ui.label(RichText::new(s).size(12.5).strong().color(t.text2));
        };
        let g = &mut self.data.extra.goals[gi];
        let mut ch = false;
        label(ui, tr("Goal name"));
        let r = ui.add(field(&mut g.name).desired_width(f32::INFINITY));
        name_field(&r, "Goal name");
        ch |= r.changed();
        ui.add_space(10.0);
        ui.columns(2, |cols| {
            for (c, (name, which)) in cols.iter_mut().zip([("Start date", 0), ("Due date", 1)]) {
                label(c, tr(name));
                let cur = if which == 0 { g.start } else { g.due };
                let txt = cur
                    .map(|d| d.text("%b %-d"))
                    .unwrap_or_else(|| tr("Pick a date").into());
                let r = icons::icon_button(c, t, "calendar", &txt, name, true);
                let mut d = cur;
                if let Some(true) = popup(c, &r, c.id().with((name, gid)), DATE_MENU_W, |ui| {
                    date_menu(ui, t, ui.id().with((name, "cal", gid)), &mut d)
                }) {
                    if which == 0 {
                        // a start is kept once set: clearing it puts it back
                        if d.is_some() {
                            g.start = d;
                        }
                    } else {
                        g.due = d;
                    }
                    ch = true;
                }
            }
        });
        ui.add_space(10.0);
        label(ui, crate::i18n::tr_ctx("Owner (goal)", "Owner"));
        let cur = members
            .iter()
            .find(|m| m.0 == g.owner)
            .map(|m| m.1.clone())
            .unwrap_or_else(|| tr("Nobody").into());
        egui::ComboBox::from_id_salt(("goal_owner", gid))
            .selected_text(cur)
            .width(w)
            .show_ui(ui, |ui| {
                for (id, n) in &members {
                    ch |= ui.selectable_value(&mut g.owner, *id, n).changed();
                }
            });
        ui.add_space(10.0);
        label(ui, tr("Color"));
        ui.horizontal_wrapped(|ui| {
            for c in GOAL_COLORS {
                let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                ui.painter().circle_filled(r.center(), 11.0, rgb(c));
                hover_ring(ui, &resp, r.shrink(1.0), 12.0);
                if g.color == c {
                    ui.painter()
                        .circle_stroke(r.center(), 13.0, Stroke::new(1.5, t.text));
                }
                name_it(&resp, &format!("Goal color {:?}", c));
                if resp.clicked() {
                    g.color = c;
                    ch = true;
                }
            }
        });
        if ch {
            self.dirty = true;
        }
        ui.add_space(14.0);
        ui.separator();
        ui.horizontal(|ui| {
            let d = ui.add(ghost(egui::Button::new(
                RichText::new(tr("Delete goal")).color(t.danger),
            )));
            name_it(&d, "Delete goal");
            delete = d.clicked();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let b = primary_button(ui, t, tr("Done"));
                name_it(&b, "Close goal settings");
                close = b.clicked();
            });
        });
        if delete {
            let name = self.data.extra.goals[gi].name.clone();
            self.modal = Some(Modal::ConfirmRemove {
                kind: "goal",
                id: gid,
                name,
            });
        }
        close
    }

    fn goal_page_apply(&mut self, gid: u64, a: PageAct) {
        match a {
            PageAct::Goal(g) => self.goal_apply(g),
            PageAct::AddTarget(k) => {
                let tg = new_target(k);
                self.goal_edit(gid, None, |g| g.targets.push(tg));
            }
            PageAct::RemoveTarget(i) => self.goal_edit(gid, None, |g| {
                if i < g.targets.len() {
                    g.targets.remove(i);
                }
            }),
            PageAct::DismissBanner => self.goal_banner = None,
            PageAct::ShowDone(v) => self.goal_show_done = v,
            PageAct::NextGoal => self.goal_apply(GoalAct::NewGoal),
            PageAct::Share => self.open_goal_share(gid),
            PageAct::Settings => self.modal = Some(Modal::GoalSettings { id: gid }),
        }
    }
}

enum PageAct {
    Goal(GoalAct),
    AddTarget(u8),
    RemoveTarget(usize),
    DismissBanner,
    ShowDone(Option<(u64, usize)>),
    NextGoal,
    Share,
    Settings,
}

/// The colours offered for a goal.
pub(crate) const GOAL_COLORS: [[u8; 3]; 8] = [
    [0x5f, 0x55, 0xee],
    [0x44, 0x66, 0xff],
    [0x10, 0x90, 0xe0],
    [0x0f, 0x9d, 0x9f],
    [0x00, 0x88, 0x44],
    [0xf8, 0xae, 0x00],
    [0xe1, 0x6b, 0x16],
    [0xee, 0x5e, 0x99],
];

/// A new target of kind 0 number, 1 done or not, 2 tasks, named in the
/// screen's language.
pub(crate) fn new_target(kind: u8) -> GoalTarget {
    match kind {
        0 => GoalTarget {
            name: tr("Target").into(),
            kind: TargetKind::Number {
                start: 0.0,
                target: 100.0,
                current: 0.0,
                unit: String::new(),
            },
        },
        1 => GoalTarget {
            name: tr("Target").into(),
            kind: TargetKind::TrueFalse { done: false },
        },
        _ => GoalTarget {
            name: tr("Finish tasks").into(),
            kind: TargetKind::Tasks { ids: vec![] },
        },
    }
}

/// A line of a goal's history.
fn history_text(what: &GoalChange, who: &str) -> String {
    match what {
        GoalChange::Created => trf!("{} created the goal", who),
        GoalChange::Tracked => tr("History starts here").into(),
        GoalChange::Number {
            target,
            from,
            to,
            unit,
        } => {
            trf!(
                "{} recorded {}: {} → {}",
                who,
                target,
                fmt_num(*from),
                format!("{} {}", fmt_num(*to), unit).trim_end()
            )
        }
        GoalChange::Done { target, done: true } => trf!("{} marked {} done", who, target),
        GoalChange::Done {
            target,
            done: false,
        } => trf!("{} marked {} not done", who, target),
        GoalChange::Task {
            task, done: true, ..
        } => trf!("{} finished the task {}", who, task),
        GoalChange::Task {
            task, done: false, ..
        } => trf!("{} reopened the task {}", who, task),
        GoalChange::Milestone(100) => tr("Goal achieved").into(),
        GoalChange::Milestone(m) => trf!("{}% reached", m),
    }
}

/// How long a milestone just reached floats over the page, in seconds.
const MILESTONE_SECS: i64 = 6;

/// What became of the floating milestone this frame.
enum Float {
    Shown,
    /// the pointer is on it: it stays while being read
    Held,
    Closed,
}

/// "75% reached. Almost there" with what is left and a close button,
/// floating at the foot of the goal's page (`page`, `lift` higher) so
/// nothing on the page moves when it comes or goes.
fn milestone_float(
    ctx: &egui::Context,
    t: &Theme,
    page: Rect,
    lift: f32,
    m: u8,
    left: &[&str],
) -> Float {
    let mut out = Float::Shown;
    let (bg, fg) = pace_colors(Pace::Done, t);
    let w = (page.width() - 48.0).clamp(260.0, 520.0);
    egui::Area::new(egui::Id::new("goal milestone"))
        .order(egui::Order::Foreground)
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(pos2(page.center().x, page.bottom() - 24.0 - lift))
        .show(ctx, |ui| {
            let r = egui::Frame::new()
                .fill(bg)
                .stroke(Stroke::new(1.0, fg.gamma_multiply(0.25)))
                .corner_radius(14)
                .inner_margin(egui::Margin::symmetric(14, 12))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 6],
                    blur: 18,
                    spread: 0,
                    color: Color32::from_black_alpha(if t.dark { 90 } else { 30 }),
                })
                .show(ui, |ui| {
                    ui.set_width(w - 28.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 16.0, fg);
                        icons::line_icon(ui.painter(), "flag", r.center(), bg);
                        let text_w = ui.available_width() - 36.0;
                        ui.vertical(|ui| {
                            ui.set_width(text_w);
                            ui.spacing_mut().item_spacing.y = 2.0;
                            let title = match m {
                                25 => tr("25% reached. A good start"),
                                50 => tr("50% reached. Halfway there"),
                                _ => tr("75% reached. Almost there"),
                            };
                            ui.label(RichText::new(title).size(14.0).strong().color(fg));
                            if !left.is_empty() {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(trf!("Still open: {}", left.join(", ")))
                                            .size(12.5)
                                            .color(t.text2),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                        let x = close_x(ui, 26.0, t.text2);
                        name_it(&x, "Close milestone");
                        if x.clicked() {
                            out = Float::Closed;
                        }
                    });
                });
            if !matches!(out, Float::Closed) && r.response.contains_pointer() {
                out = Float::Held;
            }
        });
    out
}

/// A done-or-not target's button: "✓ Done" filled in the goal's colour or
/// "Mark done" outlined, both as wide as the wider of the two so the button
/// keeps its size when pressed and the rows line up.
fn done_toggle(ui: &mut Ui, t: &Theme, done: bool, color: Color32) -> egui::Response {
    let font = FontId::proportional(14.0);
    let tick_w = 16.0;
    let text_w = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_string(), font.clone(), t.text)
            .size()
            .x
    };
    let w = (text_w(tr("Done")) + tick_w).max(text_w(tr("Mark done"))) + 32.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w.ceil(), 34.0), Sense::click());
    let hovered = resp.hovered() && resp.enabled();
    let p = ui.painter();
    let (text, fg) = if done {
        p.rect_filled(r, 10.0, color);
        if hovered {
            p.rect_filled(r, 10.0, crate::widgets::hover_fill(ui));
        }
        (tr("Done"), Color32::WHITE)
    } else {
        p.rect_filled(r, 10.0, if hovered { t.hover } else { t.card });
        p.rect_stroke(
            r,
            10.0,
            Stroke::new(
                1.0,
                if hovered {
                    crate::widgets::hover_line(ui)
                } else {
                    t.border
                },
            ),
            egui::StrokeKind::Inside,
        );
        (tr("Mark done"), t.text)
    };
    let g = p.layout_no_wrap(text.to_string(), font, fg);
    let content = g.size().x + if done { tick_w } else { 0.0 };
    let mut x = r.center().x - content / 2.0;
    if done {
        paint_tick(p, pos2(x + 5.0, r.center().y), 5.0, fg, 2.0);
        x += tick_w;
    }
    p.galley(pos2(x, r.center().y - g.size().y / 2.0), g, fg);
    resp
}

/// A small framed group (owner, dates) under the goal's name.
fn info_chip(ui: &mut Ui, t: &Theme, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(t.card)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(16)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                add(ui);
            });
        });
}

/// A tick mark centred on `c`, `s` half its width.
pub(crate) fn paint_tick(p: &egui::Painter, c: Pos2, s: f32, color: Color32, w: f32) {
    p.line(
        vec![
            c + vec2(-s, 0.0),
            c + vec2(-s * 0.3, s * 0.7),
            c + vec2(s, -s * 0.7),
        ],
        Stroke::new(w, color),
    );
}

/// Bits of coloured paper scattered over `r` for a goal achieved.
pub(crate) fn paint_confetti(p: &egui::Painter, r: Rect) {
    const BITS: [(f32, f32, u8, f32); 12] = [
        (0.13, 0.12, 0, 24.0),
        (0.31, 0.29, 1, -30.0),
        (0.44, 0.10, 2, 0.0),
        (0.57, 0.37, 3, 50.0),
        (0.69, 0.20, 4, 0.0),
        (0.79, 0.08, 5, -18.0),
        (0.87, 0.42, 0, 0.0),
        (0.40, 0.61, 6, 36.0),
        (0.23, 0.49, 1, 0.0),
        (0.71, 0.66, 2, -44.0),
        (0.95, 0.80, 3, 0.0),
        (0.53, 0.86, 4, 14.0),
    ];
    let colors = [
        rgb([0x5f, 0x55, 0xee]),
        rgb([0xf8, 0xae, 0x00]),
        rgb([0xee, 0x5e, 0x99]),
        rgb([0x0f, 0x9d, 0x9f]),
        rgb([0x44, 0x66, 0xff]),
        rgb([0xe1, 0x6b, 0x16]),
        rgb([0x3d, 0xb8, 0x8b]),
    ];
    for (i, (x, y, c, rot)) in BITS.iter().enumerate() {
        let at = pos2(r.left() + r.width() * x, r.top() + r.height() * y);
        let color = colors[*c as usize];
        if i % 2 == 0 {
            let a = rot.to_radians();
            let (dx, dy) = (a.cos() * 5.0, a.sin() * 5.0);
            p.line_segment(
                [at - vec2(dx, dy), at + vec2(dx, dy)],
                Stroke::new(4.5, color),
            );
        } else {
            p.circle_filled(at, 3.8, color);
        }
    }
}
