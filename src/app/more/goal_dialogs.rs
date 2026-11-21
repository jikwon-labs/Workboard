// The New goal dialog, the Goal achieved dialog and the Goals page before
// there are any goals (with samples to start from).

use super::goal_page::{new_target, paint_confetti, paint_tick, GOAL_COLORS};
use super::goals::*;
use super::*;
use egui::{Response, Vec2};

/// A goal being set up in the New goal dialog.
#[derive(Clone, Debug)]
pub(crate) struct GoalDraft {
    pub name: String,
    /// 0 end of this month, 1 end of this quarter, 2 end of next quarter,
    /// 3 a day picked
    pub when: u8,
    pub due: Option<NaiveDate>,
    /// the target being set up: 0 number, 1 done or not, 2 tasks
    pub kind: u8,
    pub target_name: String,
    pub from: f64,
    pub to: f64,
    pub unit: String,
    pub tasks: Vec<u64>,
    /// targets added with "+ Target" before this one
    pub more: Vec<GoalTarget>,
    pub owner: u64,
    pub color: [u8; 3],
}

impl GoalDraft {
    pub(crate) fn new(owner: u64) -> Self {
        Self {
            name: String::new(),
            when: 1,
            due: Some(due_preset(1, today())),
            kind: 0,
            target_name: String::new(),
            from: 0.0,
            to: 100.0,
            unit: String::new(),
            tasks: vec![],
            more: vec![],
            owner,
            color: GOAL_COLORS[0],
        }
    }

    /// The target set up below the kinds, named by default when left blank.
    fn current_target(&self) -> GoalTarget {
        let mut tg = new_target(self.kind);
        if !self.target_name.trim().is_empty() {
            tg.name = self.target_name.trim().to_string();
        }
        match &mut tg.kind {
            TargetKind::Number {
                start,
                target,
                unit,
                current,
            } => {
                *start = self.from;
                *current = self.from;
                *target = self.to;
                *unit = self.unit.trim().to_string();
            }
            TargetKind::Tasks { ids } => *ids = self.tasks.clone(),
            TargetKind::TrueFalse { .. } => {}
        }
        tg
    }

    fn reset_target(&mut self) {
        self.target_name.clear();
        self.from = 0.0;
        self.to = 100.0;
        self.unit.clear();
        self.tasks.clear();
    }
}

/// The number target being set up starts where it ends: there is nothing
/// to measure, so it can't be added (it would read as met from the start).
fn same_range(d: &GoalDraft) -> bool {
    d.kind == 0 && (d.from - d.to).abs() < f64::EPSILON
}

/// The last day of this month (0), this quarter (1) or next quarter (2).
fn due_preset(which: u8, day: NaiveDate) -> NaiveDate {
    let (y, m) = (day.year(), day.month());
    let last_month = match which {
        0 => m,
        1 => (m - 1) / 3 * 3 + 3,
        _ => (m - 1) / 3 * 3 + 6,
    };
    let (y, last_month) = if last_month > 12 {
        (y + 1, last_month - 12)
    } else {
        (y, last_month)
    };
    let first_next = if last_month == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, last_month + 1, 1)
    };
    first_next.and_then(|d| d.pred_opt()).unwrap_or(day)
}

/// Samples on the empty Goals page: a name and its targets.
fn samples() -> Vec<(&'static str, Vec<GoalTarget>, [u8; 3])> {
    let num = |name: &str, to: f64, unit: &str| GoalTarget {
        id: 0,
        name: tr(name).into(),
        kind: TargetKind::Number {
            start: 0.0,
            target: to,
            current: 0.0,
            unit: tr(unit).into(),
        },
    };
    let done = |name: &str| GoalTarget {
        id: 0,
        name: tr(name).into(),
        kind: TargetKind::TrueFalse { done: false },
    };
    let tasks = |name: &str| GoalTarget {
        id: 0,
        name: tr(name).into(),
        kind: TargetKind::Tasks { ids: vec![] },
    };
    vec![
        (
            "Quarterly revenue",
            vec![
                num("New contracts", 100.0, "M won"),
                num("New customers", 20.0, "customers"),
            ],
            GOAL_COLORS[1],
        ),
        (
            "Product launch",
            vec![tasks("Launch tasks"), done("Store review passed")],
            GOAL_COLORS[0],
        ),
        (
            "Team hiring",
            vec![
                num("People hired", 3.0, "people"),
                done("Onboarding guide ready"),
            ],
            GOAL_COLORS[7],
        ),
    ]
}

impl App {
    /// Opens New goal with a fresh draft in the next colour.
    pub(crate) fn open_new_goal(&mut self, sample: Option<usize>) {
        let mut d = GoalDraft::new(self.data.me);
        d.color = GOAL_COLORS[self.data.extra.goals.len() % GOAL_COLORS.len()];
        if let Some((name, mut targets, color)) = sample.and_then(|i| samples().into_iter().nth(i))
        {
            d.name = tr(name).into();
            d.color = color;
            // the last target is the one open for changes below the kinds
            if let Some(last) = targets.pop() {
                d.target_name = last.name.clone();
                match last.kind {
                    TargetKind::Number {
                        start,
                        target,
                        unit,
                        ..
                    } => {
                        d.kind = 0;
                        d.from = start;
                        d.to = target;
                        d.unit = unit;
                    }
                    TargetKind::TrueFalse { .. } => d.kind = 1,
                    TargetKind::Tasks { ids } => {
                        d.kind = 2;
                        d.tasks = ids;
                    }
                }
            }
            d.more = targets;
        }
        self.modal = Some(Modal::NewGoal(Box::new(d)));
    }

    /// The New goal dialog. Returns true when it should close.
    pub(crate) fn new_goal_body(&mut self, ui: &mut Ui, t: &Theme, d: &mut GoalDraft) -> bool {
        let mut close = false;
        let day = today();
        let w = ui.available_width();
        let label = |ui: &mut Ui, s: &str| {
            ui.label(RichText::new(tr(s)).size(13.5).strong());
        };
        label(ui, "Goal name");
        let r = ui.add(
            field(&mut d.name)
                .hint_text(tr("For example: Q1 revenue of 100M won"))
                .font(FontId::proportional(17.0))
                .desired_width(f32::INFINITY),
        );
        name_field(&r, "Goal name entry");
        if focus_once(ui, &r) {
            r.request_focus();
        }
        ui.add_space(14.0);
        label(ui, "Due date");
        ui.horizontal_wrapped(|ui| {
            for i in 0..4u8 {
                let (title, sub) = match i {
                    0 => (tr("End of this month"), due_preset(0, day).text("%b %-d")),
                    1 => (tr("End of this quarter"), due_preset(1, day).text("%b %-d")),
                    2 => (tr("End of next quarter"), due_preset(2, day).text("%b %-d")),
                    _ => (
                        tr("Pick a date"),
                        if d.when == 3 {
                            d.due.map(|x| x.text("%b %-d")).unwrap_or_default()
                        } else {
                            String::new()
                        },
                    ),
                };
                let sel = d.when == i;
                let resp = choice_box(ui, t, title, &sub, sel, vec2(0.0, 46.0));
                name_it(
                    &resp,
                    &format!(
                        "Due {}",
                        [
                            "end of month",
                            "end of quarter",
                            "end of next quarter",
                            "picked"
                        ][i as usize]
                    ),
                );
                if i < 3 && resp.clicked() {
                    d.when = i;
                    d.due = Some(due_preset(i, day));
                }
                if i == 3 {
                    let mut pick = d.due;
                    if let Some(true) =
                        popup(ui, &resp, ui.id().with("new goal due"), DATE_MENU_W, |ui| {
                            date_menu(ui, t, ui.id().with("new goal cal"), &mut pick)
                        })
                    {
                        d.when = 3;
                        d.due = pick;
                    }
                }
            }
        });
        ui.label(
            RichText::new(trf!(
                "It starts today, {}. Progress is compared with the time gone by.",
                day.text("%b %-d")
            ))
            .size(12.5)
            .color(t.text2),
        );
        ui.add_space(14.0);
        label(ui, "Measured by");
        ui.horizontal(|ui| {
            let bw = (w - 16.0) / 3.0;
            for (k, title, ex, icon) in [
                (0u8, "Number", "For example: revenue, customers", "hash"),
                (
                    1,
                    "Done or not",
                    "For example: contract signed, review passed",
                    "check",
                ),
                (2, "Tasks", "For example: the launch tasks", "list"),
            ] {
                let (r, resp) = ui.allocate_exact_size(vec2(bw, 92.0), Sense::click());
                let sel = d.kind == k;
                let p = ui.painter();
                p.rect_filled(
                    r,
                    12.0,
                    if resp.hovered() && !sel {
                        t.hover
                    } else {
                        t.card
                    },
                );
                p.rect_stroke(
                    r,
                    12.0,
                    Stroke::new(
                        if sel { 1.8 } else { 1.0 },
                        if sel { t.text } else { t.border },
                    ),
                    egui::StrokeKind::Inside,
                );
                let ib = Rect::from_min_size(r.min + vec2(14.0, 14.0), vec2(30.0, 30.0));
                let (bg, fg) = pace_colors(Pace::Done, t);
                p.rect_filled(ib, 8.0, bg);
                icons::line_icon(p, icon, ib.center(), fg);
                if sel {
                    let c = pos2(r.right() - 18.0, r.top() + 18.0);
                    p.circle_filled(c, 9.0, t.text);
                    paint_tick(p, c, 4.0, t.card, 1.8);
                }
                p.text(
                    pos2(r.left() + 14.0, r.top() + 56.0),
                    Align2::LEFT_CENTER,
                    tr(title),
                    FontId::proportional(14.0),
                    t.text,
                );
                let ef = FontId::proportional(12.0);
                p.text(
                    pos2(r.left() + 14.0, r.top() + 76.0),
                    Align2::LEFT_CENTER,
                    crate::widgets::fit_text(p, tr(ex), &ef, bw - 24.0),
                    ef,
                    t.text2,
                );
                name_it(&resp, &format!("Measure by {}", title));
                if resp.clicked() {
                    d.kind = k;
                }
            }
        });
        ui.add_space(8.0);
        // targets added already
        let mut drop = None;
        if !d.more.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for (i, tg) in d.more.iter().enumerate() {
                    // a long name starts a new row instead of running off the edge
                    let max_w = crate::widgets::chip_room(ui, &tg.name, 13.0, 41.0);
                    egui::Frame::new()
                        .fill(t.hover)
                        .corner_radius(14)
                        .inner_margin(egui::Margin {
                            left: 10,
                            right: 3,
                            top: 2,
                            bottom: 2,
                        })
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.scope(|ui| {
                                    ui.set_max_width(max_w);
                                    ui.add(
                                        egui::Label::new(RichText::new(&tg.name).size(13.0))
                                            .truncate(),
                                    );
                                });
                                let x = close_x(ui, 20.0, t.text2);
                                name_it(&x, &format!("Remove {}", tg.name));
                                if x.clicked() {
                                    drop = Some(i);
                                }
                            });
                        });
                }
            });
            ui.add_space(6.0);
        }
        if let Some(i) = drop {
            d.more.remove(i);
        }
        egui::Frame::new()
            .fill(if t.dark {
                t.hover
            } else {
                rgb([247, 247, 248])
            })
            .corner_radius(12)
            .inner_margin(egui::Margin::symmetric(14, 12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let small = |ui: &mut Ui, s: &str| {
                    ui.label(RichText::new(tr(s)).size(12.0).strong().color(t.text2));
                };
                let hint = new_target(d.kind).name;
                match d.kind {
                    0 => {
                        // a column each: its label over its box
                        ui.horizontal_top(|ui| {
                            let name_w = (ui.available_width() - 250.0).max(140.0);
                            ui.vertical(|ui| {
                                small(ui, "Target name");
                                let r = ui.add(
                                    field(&mut d.target_name)
                                        .hint_text(hint.as_str())
                                        .desired_width(name_w),
                                );
                                name_field(&r, "Target name entry");
                            });
                            ui.vertical(|ui| {
                                small(ui, "Start value");
                                let r = ui.add_sized(vec2(64.0, 26.0), number_box(&mut d.from));
                                name_it(&r, "Start value entry");
                            });
                            ui.vertical(|ui| {
                                small(ui, "Target value");
                                let r = ui.add_sized(vec2(64.0, 26.0), number_box(&mut d.to));
                                name_it(&r, "Target value entry");
                            });
                            ui.vertical(|ui| {
                                small(ui, "Unit");
                                let r = ui.add(
                                    field(&mut d.unit)
                                        .hint_text(tr("e.g. people"))
                                        .desired_width(70.0),
                                );
                                name_field(&r, "Unit entry");
                            });
                        });
                        ui.add_space(4.0);
                        let unit = d.unit.trim();
                        if same_range(d) {
                            // nothing to measure between them: it would read
                            // as met from the start
                            ui.label(
                                RichText::new(tr("The start and target values are the same"))
                                    .size(12.5)
                                    .color(t.danger),
                            );
                        } else {
                            ui.label(
                                RichText::new(trf!(
                                    "{} → {}, progress counted from where it starts",
                                    format!("{} {}", fmt_num(d.from), unit).trim_end(),
                                    format!("{} {}", fmt_num(d.to), unit).trim_end()
                                ))
                                .size(12.5)
                                .color(t.text2),
                            );
                        }
                    }
                    1 => {
                        small(ui, "Target name");
                        let r = ui.add(
                            field(&mut d.target_name)
                                .hint_text(hint.as_str())
                                .desired_width(f32::INFINITY),
                        );
                        name_field(&r, "Target name entry");
                        ui.label(
                            RichText::new(tr("Marking it done makes this target 100%"))
                                .size(12.5)
                                .color(t.text2),
                        );
                    }
                    _ => {
                        small(ui, "Target name");
                        ui.horizontal(|ui| {
                            let r = ui.add(
                                field(&mut d.target_name)
                                    .hint_text(hint.as_str())
                                    .desired_width((w - 160.0).max(140.0)),
                            );
                            name_field(&r, "Target name entry");
                            let all: Vec<(u64, String)> = self
                                .data
                                .tasks
                                .iter()
                                .filter(|x| x.parent.is_none() && !x.archived)
                                .map(|x| (x.id, x.name.clone()))
                                .collect();
                            let r = secondary_button(ui, tr("Pick tasks"));
                            name_it(&r, "Pick tasks");
                            popup(ui, &r, ui.id().with("new goal tasks"), 300.0, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(300.0)
                                    .show(ui, |ui| {
                                        for (id, name) in &all {
                                            let mut on = d.tasks.contains(id);
                                            if check(ui, &mut on, name).changed() {
                                                if on {
                                                    d.tasks.push(*id);
                                                } else {
                                                    d.tasks.retain(|x| x != id);
                                                }
                                            }
                                        }
                                    });
                                None::<bool>
                            });
                        });
                        let line = if d.tasks.is_empty() {
                            tr("Pick the tasks; it goes up as they are closed").to_string()
                        } else {
                            trf!(
                                "{} tasks picked; it goes up as they are closed",
                                d.tasks.len()
                            )
                        };
                        ui.label(RichText::new(line).size(12.5).color(t.text2));
                    }
                }
            });
        ui.add_space(6.0);
        let add = icons::icon_button(ui, t, "plus", tr("Target"), "Another target", true);
        if add.clicked() && !same_range(d) {
            let tg = d.current_target();
            d.more.push(tg);
            d.reset_target();
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(crate::i18n::tr_ctx("Owner (goal)", "Owner"))
                    .size(13.0)
                    .strong()
                    .color(t.text2),
            );
            let members: Vec<(u64, String)> = self
                .data
                .members
                .iter()
                .map(|m| (m.id, m.shown().to_string()))
                .collect();
            let cur = members
                .iter()
                .find(|m| m.0 == d.owner)
                .map(|m| m.1.clone())
                .unwrap_or_default();
            egui::ComboBox::from_id_salt("new goal owner")
                .selected_text(cur)
                .show_ui(ui, |ui| {
                    for (id, n) in &members {
                        ui.selectable_value(&mut d.owner, *id, n);
                    }
                });
            ui.add_space(16.0);
            ui.label(
                RichText::new(tr("Color"))
                    .size(13.0)
                    .strong()
                    .color(t.text2),
            );
            for c in GOAL_COLORS {
                let (r, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                ui.painter().circle_filled(r.center(), 10.0, rgb(c));
                hover_ring(ui, &resp, r.shrink(1.0), 11.0);
                if d.color == c {
                    ui.painter()
                        .circle_stroke(r.center(), 12.0, Stroke::new(1.5, t.text));
                }
                name_it(&resp, &format!("New goal color {:?}", c));
                if resp.clicked() {
                    d.color = c;
                }
            }
        });
        ui.add_space(14.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tr("You can change all of this later"))
                    .size(12.5)
                    .color(t.text2),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let named = !d.name.trim().is_empty() && !same_range(d);
                let create = ui
                    .add_enabled_ui(named, |ui| primary_button(ui, t, tr("Create goal")))
                    .inner;
                name_it(&create, "Create goal");
                if create.clicked() && named {
                    let mut targets = d.more.clone();
                    targets.push(d.current_target());
                    let g = Goal {
                        name: d.name.trim().to_string(),
                        color: d.color,
                        due: d.due,
                        owner: d.owner,
                        targets,
                        ..Default::default()
                    };
                    self.add_goal(g);
                    self.section = Section::Goals;
                    close = true;
                }
                let cancel = secondary_button(ui, tr("Cancel"));
                name_it(&cancel, "Cancel new goal");
                if cancel.clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// Goal achieved: the moment a goal reaches 100%.
    pub(crate) fn goal_achieved_body(&mut self, ui: &mut Ui, t: &Theme, gid: u64) -> bool {
        let mut close = false;
        let Some(g) = self.data.extra.goals.iter().find(|g| g.id == gid).cloned() else {
            return true;
        };
        let color = rgb(g.color);
        let w = ui.available_width();
        let (top, _) = ui.allocate_exact_size(vec2(w, 132.0), Sense::hover());
        // paper on both sides of the ring, none over it
        let side = top.width() * 0.32;
        paint_confetti(
            ui.painter(),
            Rect::from_min_size(top.min, vec2(side, top.height())),
        );
        paint_confetti(
            ui.painter(),
            Rect::from_min_size(
                pos2(top.right() - side, top.top()),
                vec2(side, top.height()),
            ),
        );
        let c = top.center();
        let p = ui.painter();
        p.circle_filled(c, 54.0, color.gamma_multiply(0.15));
        p.circle_stroke(c, 54.0, Stroke::new(9.0, color));
        paint_tick(p, c, 20.0, color, 5.0);
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.label(
                RichText::new(tr("Goal achieved!"))
                    .size(14.0)
                    .strong()
                    .color(color),
            );
            ui.label(RichText::new(&g.name).size(24.0).strong());
            let line = match (g.done_on, g.due) {
                (Some(d), Some(due)) if due > d => {
                    trf!("Done {} days before the deadline", (due - d).num_days())
                }
                (Some(d), Some(due)) if due == d => tr("Done right on the deadline").into(),
                (Some(d), Some(due)) => {
                    trf!("Done {} days after the deadline", (d - due).num_days())
                }
                _ => String::new(),
            };
            if !line.is_empty() {
                ui.label(RichText::new(line).size(14.0).color(t.text2));
            }
        });
        ui.add_space(12.0);
        // targets, days taken, the people who moved it
        let mut people: Vec<u64> = vec![];
        // (those still on the team: the others have no face to show)
        for e in &g.log {
            if e.who != 0 && !people.contains(&e.who) && self.data.member(e.who).is_some() {
                people.push(e.who);
            }
        }
        let days = match (g.start, g.done_on) {
            (Some(s), Some(d)) => Some((d - s).num_days().max(0) + 1),
            _ => None,
        };
        let n = g.targets.len();
        egui::Frame::new()
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(14)
            .inner_margin(egui::Margin::symmetric(8, 10))
            .show(ui, |ui| {
                ui.columns(3, |cols| {
                    cols[0].vertical_centered(|ui| {
                        ui.label(RichText::new(format!("{}/{}", n, n)).size(20.0).strong());
                        ui.label(RichText::new(tr("Targets")).size(12.0).color(t.text2));
                    });
                    cols[1].vertical_centered(|ui| {
                        let d = days
                            .map(|d| trf!("{} days", d))
                            .unwrap_or_else(|| "–".into());
                        ui.label(RichText::new(d).size(20.0).strong());
                        ui.label(RichText::new(tr("Time taken")).size(12.0).color(t.text2));
                    });
                    cols[2].vertical_centered(|ui| {
                        let ids: Vec<u64> = people.iter().copied().take(5).collect();
                        if ids.is_empty() {
                            ui.label(RichText::new("–").size(20.0).strong());
                        } else {
                            let aw = avatars_width(11.0, &ids, &self.data).max(22.0);
                            let (r, _) = ui.allocate_exact_size(vec2(aw, 26.0), Sense::hover());
                            paint_avatars(
                                ui.painter(),
                                pos2(r.left(), r.center().y),
                                11.0,
                                &ids,
                                &self.data,
                                t.card,
                            );
                        }
                        ui.label(
                            RichText::new(tr("People involved"))
                                .size(12.0)
                                .color(t.text2),
                        );
                    });
                });
            });
        ui.add_space(10.0);
        for tg in &g.targets {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(20.0, 22.0), Sense::hover());
                let bx = Rect::from_center_size(r.center(), vec2(18.0, 18.0));
                paint_check(ui.painter(), bx, true, color, t, false);
                ui.label(RichText::new(&tg.name).size(14.0));
            });
        }
        ui.add_space(12.0);
        let (share, next) = achieved_buttons(ui, color);
        name_it(&share, "Share from achieved");
        if share.clicked() {
            // the share dialog takes this one's place
            self.open_goal_share(gid);
            close = true;
        }
        name_it(&next, "Next goal from achieved");
        if next.clicked() {
            // the new dialog takes this one's place
            self.open_new_goal(None);
            close = true;
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(tr("Achieved goals gather at the bottom of the Goals page."))
                .size(12.5)
                .color(t.text2),
        );
        close
    }

    /// The Goals page before there are any goals.
    pub(crate) fn goals_empty(&mut self, ui: &mut Ui) {
        let t = self.t();
        let mut open: Option<Option<usize>> = None;
        egui::ScrollArea::vertical()
            .id_salt("goals_empty")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    let w = ui.available_width().min(760.0);
                    ui.set_max_width(w);
                    let (r, _) = ui.allocate_exact_size(vec2(380.0, 170.0), Sense::hover());
                    paint_goal_picture(ui.painter(), r, &t);
                    ui.add_space(20.0);
                    ui.label(RichText::new(tr("Set your first goal")).size(28.0).strong());
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(tr("Break a goal into targets you can measure to see its progress and pace at a glance."))
                            .size(15.0)
                            .color(t.text2),
                    );
                    ui.add_space(16.0);
                    let b = ui.add(primary_btn(&t, tr("+ New goal")).min_size(vec2(130.0, 42.0)));
                    name_it(&b, "New goal");
                    if b.clicked() {
                        open = Some(None);
                    }
                    ui.add_space(34.0);
                    ui.label(RichText::new(tr("Browse samples")).size(13.0).strong().color(t.text2));
                    ui.add_space(10.0);
                    let gap = 12.0;
                    let cols = if w > 640.0 { 3 } else { 1 };
                    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
                    let list = samples();
                    for chunk in list.chunks(cols).enumerate() {
                        let (ci, chunk) = chunk;
                        let (row, _) = ui.allocate_exact_size(vec2(w, 132.0), Sense::hover());
                        for (j, (name, targets, color)) in chunk.iter().enumerate() {
                            let idx = ci * cols + j;
                            let r = Rect::from_min_size(row.min + vec2(j as f32 * (cw + gap), 0.0), vec2(cw, 132.0));
                            let resp = ui.interact(r, ui.id().with(("goal sample", idx)), Sense::click());
                            name_it(&resp, &format!("Sample {}", name));
                            let p = ui.painter();
                            p.rect_filled(r, 14.0, if resp.hovered() { t.hover } else { t.card });
                            p.rect_stroke(r, 14.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                            p.circle_filled(pos2(r.left() + 22.0, r.top() + 24.0), 6.0, rgb(*color));
                            p.text(pos2(r.left() + 36.0, r.top() + 24.0), Align2::LEFT_CENTER, tr(name), FontId::proportional(15.0), t.text);
                            for (k, tg) in targets.iter().enumerate() {
                                let y = r.top() + 54.0 + k as f32 * 24.0;
                                let kind = match tg.kind {
                                    TargetKind::Number { .. } => tr("Number"),
                                    TargetKind::TrueFalse { .. } => tr("Done or not"),
                                    TargetKind::Tasks { .. } => tr("Tasks"),
                                };
                                let kg = p.layout_no_wrap(kind.to_string(), FontId::proportional(11.5), t.text2);
                                let kr = Rect::from_min_size(pos2(r.left() + 18.0, y - 9.0), vec2(kg.size().x + 10.0, 18.0));
                                p.rect_filled(kr, 5.0, t.hover);
                                p.galley(kr.center() - kg.size() / 2.0, kg, t.text2);
                                let nf = FontId::proportional(13.0);
                                p.text(pos2(kr.right() + 8.0, y), Align2::LEFT_CENTER, crate::widgets::fit_text(p, &tg.name, &nf, r.right() - kr.right() - 20.0), nf, t.text);
                            }
                            p.text(pos2(r.left() + 18.0, r.bottom() - 18.0), Align2::LEFT_CENTER, tr("Use this sample"), FontId::proportional(12.5), t.text2);
                            if resp.clicked() {
                                open = Some(Some(idx));
                            }
                        }
                        ui.add_space(gap);
                    }
                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(12.0);
                    let steps = [
                        ("Set a goal", "Name it and pick a due date"),
                        ("Add targets", "Measure it with a number, done or not, or tasks"),
                        ("Record progress", "It fills in as you record, with a note at 25, 50 and 75%"),
                    ];
                    let sw = (w - gap * 2.0) / 3.0;
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for (i, (title, body)) in steps.iter().enumerate() {
                            ui.vertical(|ui| {
                                ui.set_width(if cols == 3 { sw } else { w });
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                                    ui.painter().circle_filled(r.center(), 13.0, t.hover);
                                    ui.painter().text(r.center(), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(13.0), t.text);
                                    ui.label(RichText::new(tr(title)).size(14.0).strong());
                                });
                                ui.add(egui::Label::new(RichText::new(tr(body)).size(13.0).color(t.text2)).wrap());
                            });
                        }
                    });
                });
            });
        if let Some(s) = open {
            self.open_new_goal(s);
        }
    }
}

/// A selectable box with a title and a line under it.
fn choice_box(ui: &mut Ui, t: &Theme, title: &str, sub: &str, sel: bool, min: Vec2) -> Response {
    let tf = FontId::proportional(13.5);
    let sf = FontId::proportional(12.0);
    let tw = ui
        .painter()
        .layout_no_wrap(title.to_string(), tf.clone(), t.text)
        .size()
        .x;
    let sw = ui
        .painter()
        .layout_no_wrap(sub.to_string(), sf.clone(), t.text2)
        .size()
        .x;
    let size = vec2(tw.max(sw) + 28.0, min.y);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect_filled(
        r,
        11.0,
        if sel {
            t.hover
        } else if resp.hovered() {
            t.hover.gamma_multiply(0.5)
        } else {
            t.card
        },
    );
    p.rect_stroke(
        r,
        11.0,
        Stroke::new(
            if sel { 1.6 } else { 1.0 },
            if sel { t.text } else { t.border },
        ),
        egui::StrokeKind::Inside,
    );
    if sub.is_empty() {
        p.text(
            pos2(r.left() + 14.0, r.center().y),
            Align2::LEFT_CENTER,
            title,
            tf,
            t.text,
        );
    } else {
        p.text(
            pos2(r.left() + 14.0, r.center().y - 8.0),
            Align2::LEFT_CENTER,
            title,
            tf,
            t.text,
        );
        p.text(
            pos2(r.left() + 14.0, r.center().y + 10.0),
            Align2::LEFT_CENTER,
            sub,
            sf,
            t.text2,
        );
    }
    resp
}

/// The picture on the empty Goals page: a goal card with its ring and bars,
/// and a flag.
fn paint_goal_picture(p: &egui::Painter, r: Rect, t: &Theme) {
    let card = Rect::from_min_size(r.min + vec2(50.0, 16.0), vec2(280.0, 140.0));
    p.rect_filled(card, 16.0, t.card);
    p.rect_stroke(
        card,
        16.0,
        Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
    let c = card.left_center() + vec2(54.0, 0.0);
    paint_ring(p, c, 30.0, 9.0, 0.7, rgb(GOAL_COLORS[0]), t.hover);
    let x = card.left() + 104.0;
    p.rect_filled(
        Rect::from_min_size(pos2(x, card.top() + 36.0), vec2(110.0, 9.0)),
        4.5,
        t.hover,
    );
    for (i, (frac, col)) in [(0.74, GOAL_COLORS[1]), (0.44, GOAL_COLORS[3])]
        .iter()
        .enumerate()
    {
        let bar = Rect::from_min_size(
            pos2(x, card.top() + 62.0 + i as f32 * 20.0),
            vec2(150.0, 7.0),
        );
        paint_bar(p, bar, *frac, rgb(*col), t.hover);
    }
    let bx = Rect::from_min_size(pos2(x, card.top() + 102.0), vec2(14.0, 14.0));
    paint_check(p, bx, true, rgb(GOAL_COLORS[4]), t, false);
    p.rect_filled(
        Rect::from_min_size(pos2(x + 22.0, card.top() + 106.0), vec2(70.0, 6.0)),
        3.0,
        t.hover,
    );
    // the flag at the top right
    let pole = pos2(card.right() + 8.0, card.top() - 6.0);
    p.line_segment([pole, pole + vec2(0.0, 44.0)], Stroke::new(3.0, t.text));
    p.add(egui::Shape::convex_polygon(
        vec![
            pole + vec2(2.0, 2.0),
            pole + vec2(34.0, 2.0),
            pole + vec2(26.0, 12.0),
            pole + vec2(34.0, 22.0),
            pole + vec2(2.0, 22.0),
        ],
        rgb(GOAL_COLORS[5]),
        Stroke::NONE,
    ));
    // paper beside the card, not over it
    paint_confetti(
        p,
        Rect::from_min_max(r.min, pos2(card.left() - 6.0, r.bottom())),
    );
    paint_confetti(
        p,
        Rect::from_min_max(pos2(card.right() + 44.0, r.top() + 50.0), r.max),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_presets_end_months_and_quarters() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        assert_eq!(due_preset(0, d(2026, 2, 10)), d(2026, 2, 28));
        assert_eq!(due_preset(1, d(2026, 2, 10)), d(2026, 3, 31));
        assert_eq!(due_preset(2, d(2026, 2, 10)), d(2026, 6, 30));
        assert_eq!(due_preset(1, d(2026, 11, 21)), d(2026, 12, 31));
        assert_eq!(due_preset(2, d(2026, 11, 21)), d(2027, 3, 31));
        assert_eq!(due_preset(0, d(2026, 12, 5)), d(2026, 12, 31));
    }
}
