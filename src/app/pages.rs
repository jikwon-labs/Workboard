// The newer layout pieces: the icon rail on the far left, the task tray at
// the bottom of the work card, and the rail's pages (Planner, Teams, Docs,
// Dashboards, Timesheets). Layout values come from MECHANICS.md.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Home,
    Chat,
    Planner,
    Teams,
    Docs,
    Dashboards,
    Timesheets,
    People,
    Posts,
    Meetings,
    Goals,
    Forms,
    Whiteboards,
    Ai,
    Settings,
}

fn week_start(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_sunday() as i64)
}

fn bar_row(
    ui: &mut Ui,
    t: &Theme,
    label: &str,
    value: f32,
    max: f32,
    color: Color32,
    right: String,
) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    let p = ui.painter();
    p.text(
        r.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        t.text,
    );
    let bar = Rect::from_min_max(
        pos2(r.left() + 120.0, r.center().y - 5.0),
        pos2(r.right() - 60.0, r.center().y + 5.0),
    );
    p.rect_filled(bar, 5.0, t.hover);
    if max > 0.0 && value > 0.0 {
        let f = (value / max).clamp(0.0, 1.0);
        p.rect_filled(
            Rect::from_min_size(bar.min, vec2((bar.width() * f).max(6.0), bar.height())),
            5.0,
            color,
        );
    }
    p.text(
        r.right_center(),
        Align2::RIGHT_CENTER,
        right,
        FontId::proportional(12.0),
        t.text2,
    );
}

impl App {
    // ---------------------------------------------------------------- tray

    pub(super) fn tray(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::TopBottomPanel::bottom("tray")
            .exact_height(37.0)
            .show_separator_line(true)
            .frame(
                egui::Frame::new()
                    .fill(t.tray)
                    .corner_radius(egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 7,
                        se: 7,
                    })
                    .inner_margin(egui::Margin::same(8)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let ids = self.recent.clone();
                    if ids.is_empty() {
                        ui.label(
                            RichText::new(tr("Opened tasks stay here for quick access"))
                                .small()
                                .color(t.text3),
                        );
                    }
                    let mut remove = None;
                    for (i, id) in ids.iter().enumerate() {
                        let Some(task) = self.task(*id) else { continue };
                        let w = if i == 1 { 180.0 } else { 128.0 };
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 21.0), Sense::click());
                        ui.painter().rect_filled(
                            r,
                            5.0,
                            if resp.hovered() { t.border } else { t.hover },
                        );
                        let st = self.data.status_of(task);
                        paint_status_circle(
                            ui.painter(),
                            r.left_center() + vec2(10.0, 0.0),
                            5.0,
                            &st,
                        );
                        let chars = ((w - 40.0) / 6.5) as usize;
                        ui.painter().text(
                            r.left_center() + vec2(20.0, 0.0),
                            Align2::LEFT_CENTER,
                            truncate(&task.name, chars),
                            FontId::proportional(11.5),
                            t.text,
                        );
                        let xr = Rect::from_center_size(
                            r.right_center() - vec2(9.0, 0.0),
                            vec2(14.0, 14.0),
                        );
                        let x = ui.interact(xr, ui.id().with(("trayx", id)), Sense::click());
                        if resp.hovered() || x.hovered() {
                            ui.painter().text(
                                xr.center(),
                                Align2::CENTER_CENTER,
                                "\u{00D7}",
                                FontId::proportional(9.0),
                                t.text3,
                            );
                        }
                        if x.clicked() {
                            remove = Some(*id);
                        } else if resp.clicked() {
                            self.actions.push(Action::Open(*id));
                        }
                    }
                    if let Some(id) = remove {
                        self.recent.retain(|r| *r != id);
                    }
                });
            });
    }

    // ---------------------------------------------------------------- planner

    pub(super) fn planner(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        // Labs: start the week on Monday
        let monday = self.data.extra.labs.iter().any(|l| l == "week_monday");
        let wd = self.planner_week.weekday().num_days_from_sunday();
        if monday && wd == 0 {
            self.planner_week += Duration::days(1);
        } else if !monday && wd == 1 {
            self.planner_week -= Duration::days(1);
        }
        let week = self.planner_week;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(20, 14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Planner")).size(20.0).strong());
                    ui.add_space(16.0);
                    if ui.button(tr("Today")).clicked() {
                        self.planner_week =
                            week_start(today()) + Duration::days(if monday { 1 } else { 0 });
                    }
                    let (back, fwd) = super::icons::period_arrows(ui, &t);
                    if back {
                        self.planner_week = week - Duration::days(7);
                    }
                    if fwd {
                        self.planner_week = week + Duration::days(7);
                    }
                    ui.label(RichText::new(week_range(week)).size(15.0));
                });
                ui.add_space(10.0);
                let mine: Vec<Task> = self
                    .data
                    .tasks
                    .iter()
                    .filter(|x| {
                        x.assignees.contains(&me)
                            && !self.data.is_closed(x)
                            && self.data.task_live(x)
                    })
                    .cloned()
                    .collect();
                let avail = ui.available_rect_before_wrap();
                // the "To schedule" column gives up room first in a small
                // window; the seven days share what is left exactly
                let left_w = (avail.width() * 0.26).clamp(170.0, 260.0);
                let mut dropped: Option<(u64, Option<NaiveDate>)> = None;
                // a day with many tasks (overdue ones pile up on today) scrolls
                egui::ScrollArea::vertical()
                    .id_salt("planner_week")
                    .auto_shrink([false, false])
                    .drag_to_scroll(false)
                    .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // unscheduled list
                    ui.vertical(|ui| {
                        ui.set_width(left_w);
                        ui.set_min_height(avail.height() - 10.0);
                        let frame = egui::Frame::new()
                            .fill(t.column_bg)
                            .corner_radius(10)
                            .inner_margin(egui::Margin::same(10));
                        let (_, payload) = ui.dnd_drop_zone::<u64, ()>(frame, |ui| {
                            ui.set_width(left_w - 20.0);
                            ui.set_min_height(avail.height() - 40.0);
                            ui.label(RichText::new(tr("To schedule")).strong());
                            ui.label(
                                RichText::new(tr(
                                    "Your tasks without a due date. Drag one onto a day.",
                                ))
                                .small()
                                .color(t.text3),
                            );
                            ui.add_space(6.0);
                            let mut none = true;
                            for task in mine.iter().filter(|x| x.due.is_none()) {
                                none = false;
                                let resp = ui.dnd_drag_source(
                                    egui::Id::new(("plan", task.id)),
                                    task.id,
                                    |ui| self.plan_chip(ui, task, left_w - 20.0),
                                );
                                if resp.response.clicked() {
                                    self.actions.push(Action::Open(task.id));
                                }
                            }
                            if none {
                                ui.add_space(10.0);
                                ui.label(
                                    RichText::new(tr(
                                        "Nothing to schedule. Every task of yours has a day, or drag one here to take its date off.",
                                    ))
                                    .small()
                                    .color(t.text3),
                                );
                            }
                        });
                        if let Some(p) = payload {
                            dropped = Some((*p, None));
                        }
                    });
                    // week columns: equal parts of the rest, gaps included
                    let gap = ui.spacing().item_spacing.x;
                    let col_w = ((ui.available_width() - 7.0 * gap) / 7.0).floor().max(60.0);
                    for i in 0..7 {
                        let d = week + Duration::days(i);
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            let is_today = d == today();
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(d.text("%a")).color(t.text3));
                                let n = RichText::new(d.day().to_string()).strong();
                                ui.label(if is_today { n.color(t.danger) } else { n });
                            });
                            let frame = egui::Frame::new()
                                .fill(if is_today {
                                    t.selected_bg.gamma_multiply(0.5)
                                } else {
                                    t.bg
                                })
                                .stroke(Stroke::new(1.0_f32, t.border))
                                .corner_radius(8)
                                .inner_margin(egui::Margin::same(6));
                            let (_, payload) = ui.dnd_drop_zone::<u64, ()>(frame, |ui| {
                                ui.set_width(col_w - 12.0);
                                ui.set_min_height(avail.height() - 60.0);
                                // overdue tasks from before this week are shown on today's column
                                let carried = |x: &&Task| {
                                    is_today && x.due.map(|dd| dd < week).unwrap_or(false)
                                };
                                let mut day: Vec<&Task> = mine
                                    .iter()
                                    .filter(|x| x.due == Some(d) || carried(x))
                                    .collect();
                                // in the order of their due times
                                day.sort_by_key(|x| x.due_moment());
                                for task in day {
                                    let resp = ui.dnd_drag_source(
                                        egui::Id::new(("plan", task.id)),
                                        task.id,
                                        |ui| self.plan_chip(ui, task, col_w - 12.0),
                                    );
                                    if resp.response.clicked() {
                                        self.actions.push(Action::Open(task.id));
                                    }
                                }
                                self.planner_add_row(ui, d, col_w - 12.0);
                            });
                            if let Some(p) = payload {
                                dropped = Some((*p, Some(d)));
                            }
                        });
                    }
                });
                    });
                if let Some((id, due)) = dropped {
                    if let Some(mut task) = self.task(id).cloned() {
                        task.due = due;
                        self.actions.push(Action::Replace(task));
                    }
                }
            });
    }

    fn plan_chip(&self, ui: &mut Ui, task: &Task, w: f32) {
        let t = self.t();
        let st = self.data.status_of(task);
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_width(w - 18.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    paint_status_circle(ui.painter(), r.center(), 5.5, &st);
                    ui.add(egui::Label::new(RichText::new(&task.name).size(12.5)).truncate());
                });
                ui.horizontal(|ui| {
                    // one line: the list name gives up its room to the flag,
                    // time and "Overdue" after it and is shortened to fit; in a
                    // day too narrow for it, it is left out and the rest are
                    // shortened in turn
                    let small = egui::TextStyle::Small.resolve(ui.style());
                    let gap = ui.spacing().item_spacing.x;
                    let width_of = |ui: &Ui, s: &str| {
                        ui.painter()
                            .layout_no_wrap(s.to_string(), small.clone(), Color32::WHITE)
                            .size()
                            .x
                            + gap
                    };
                    let mut extras = 0.0;
                    if task.priority > 0 {
                        extras += 12.0 + gap;
                    }
                    if let Some(at) = task.due_at() {
                        extras += width_of(ui, &fmt_time(at));
                    }
                    if task.is_overdue() {
                        extras += width_of(ui, tr("Overdue"));
                    }
                    let room = ui.available_width() - extras;
                    if room >= 30.0 || extras == 0.0 {
                        ui.scope(|ui| {
                            ui.set_max_width(room.max(0.0).max(if extras == 0.0 {
                                ui.available_width()
                            } else {
                                0.0
                            }));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(self.data.list_name(task.list_id))
                                        .small()
                                        .color(t.text3),
                                )
                                .truncate(),
                            );
                        });
                    }
                    if task.priority > 0 {
                        let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                        paint_flag(
                            ui.painter(),
                            r.left_center() + vec2(2.0, 0.0),
                            priority_color(task.priority, &t),
                            10.0,
                        );
                    }
                    if let Some(at) = task.due_at() {
                        ui.add(
                            egui::Label::new(RichText::new(fmt_time(at)).small().color(t.text2))
                                .truncate(),
                        );
                    }
                    if task.is_overdue() {
                        ui.add(
                            egui::Label::new(RichText::new(tr("Overdue")).small().color(t.danger))
                                .truncate(),
                        );
                    }
                });
            });
        ui.add_space(4.0);
    }

    // ---------------------------------------------------------------- teams

    pub(super) fn teams_workload(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // the same margin and title as People and Org Chart
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Analytics")).size(20.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if super::icons::icon_button(ui, &t, "teams", tr("Manage people"), "Manage people", true)
                            .clicked()
                        {
                            // the People settings page: server roles, removal with a question
                            self.open_settings("People");
                        }
                    });
                });
                ui.label(RichText::new(tr("What everyone is working on")).color(t.text2));
                let members = self.data.members.clone();
                // with one person the bar would always be full and read like progress
                let bars = members.len() > 1;
                if bars {
                    ui.label(RichText::new(tr("Bars compare open tasks with the busiest person.")).small().color(t.text3));
                }
                ui.add_space(12.0);
                let max_open = members
                    .iter()
                    .map(|m| self.data.tasks.iter().filter(|x| x.assignees.contains(&m.id) && !self.data.is_closed(x) && self.data.task_live(x)).count())
                    .max()
                    .unwrap_or(1)
                    .max(1);
                let w = ui.available_width();
                let cols = ((w / 330.0).floor() as usize).max(1);
                let cw = (w - 12.0 * (cols as f32 - 1.0)) / cols as f32;
                for row in members.chunks(cols) {
                    ui.horizontal_top(|ui| {
                        for m in row {
                            let open: Vec<Task> = self.data.tasks.iter().filter(|x| x.assignees.contains(&m.id) && !self.data.is_closed(x) && self.data.task_live(x)).cloned().collect();
                            let active = open.iter().filter(|x| self.data.status_of(x).kind == StatusKind::Custom).count();
                            let overdue = open.iter().filter(|x| x.is_overdue()).count();
                            let done = self.data.tasks.iter().filter(|x| x.assignees.contains(&m.id) && self.data.is_closed(x) && self.data.task_live(x)).count();
                            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0_f32, t.border)).corner_radius(10).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(cw - 30.0);
                                    ui.horizontal(|ui| {
                                        let (r, _) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::hover());
                                        paint_avatar(ui.painter(), r.center(), 15.0, m, t.card);
                                        ui.vertical(|ui| {
                                            let name = if m.id == self.data.me { trf!("{} (you)", m.name) } else { m.name.clone() };
                                            ui.label(RichText::new(name).strong());
                                            ui.label(RichText::new(trf!("{} open  \u{2022}  {} in progress  \u{2022}  {} done", open.len(), active, done)).small().color(t.text2));
                                        });
                                    });
                                    ui.add_space(4.0);
                                    if bars {
                                        let (r, _) = ui.allocate_exact_size(vec2(cw - 30.0, 8.0), Sense::hover());
                                        ui.painter().rect_filled(r, 4.0, t.hover);
                                        let f = open.len() as f32 / max_open as f32;
                                        ui.painter().rect_filled(Rect::from_min_size(r.min, vec2(r.width() * f, r.height())), 4.0, rgb(m.color));
                                    }
                                    if overdue > 0 {
                                        ui.label(RichText::new(trf!("{} overdue", overdue)).small().color(t.danger));
                                    }
                                    ui.add_space(4.0);
                                    for task in open.iter().take(5) {
                                        let (r, resp) = ui.allocate_exact_size(vec2(cw - 30.0, 26.0), Sense::click());
                                        if resp.hovered() {
                                            ui.painter().rect_filled(r, 5.0, t.hover);
                                        }
                                        let st = self.data.status_of(task);
                                        paint_status_circle(ui.painter(), r.left_center() + vec2(10.0, 0.0), 5.5, &st);
                                        ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.right() - 64.0, r.bottom()))).text(r.left_center() + vec2(22.0, 0.0), Align2::LEFT_CENTER, &task.name, FontId::proportional(12.5), t.text);
                                        if let Some(d) = fmt_due(task) {
                                            ui.painter().text(r.right_center() - vec2(4.0, 0.0), Align2::RIGHT_CENTER, d, FontId::proportional(11.5), due_color(task, false, &t));
                                        }
                                        if resp.clicked() {
                                            self.actions.push(Action::Open(task.id));
                                        }
                                    }
                                    if open.len() > 5 {
                                        ui.label(RichText::new(trf!("+{} more", open.len() - 5)).small().color(t.text3));
                                    }
                                    if open.is_empty() {
                                        ui.label(RichText::new(tr("No open tasks")).small().color(t.text3));
                                    }
                                });
                            });
                        }
                    });
                    ui.add_space(12.0);
                }
            });
        });
    }

    // ---------------------------------------------------------------- dashboards

    pub(super) fn dashboard_content(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(20, 14))
                    .show(ui, |ui| {
                        let di = self.dash_open.and_then(|id| {
                            self.data.extra.dashboards.iter().position(|d| d.id == id)
                        });
                        ui.horizontal(|ui| {
                            let dashes = self.data.extra.dashboards.clone();
                            match di {
                                Some(di) => {
                                    let r = ui.add(
                                        egui::TextEdit::singleline(
                                            &mut self.data.extra.dashboards[di].name,
                                        )
                                        .font(FontId::proportional(20.0))
                                        .frame(false)
                                        .desired_width(240.0),
                                    );
                                    name_it(&r, "Dashboard name");
                                    if r.changed() {
                                        self.dirty = true;
                                    }
                                    let id = dashes[di].id;
                                    self.star_toggle(ui, format!("dash:{}", id));
                                    let d = &mut self.data.extra.dashboards[di];
                                    let (kind, label) = if d.private {
                                        ("lock", "Private")
                                    } else {
                                        ("teams", "Shared")
                                    };
                                    if super::icons::icon_button_ex(
                                        ui,
                                        &t,
                                        kind,
                                        tr(label),
                                        label,
                                        true,
                                        0.0,
                                        t.text,
                                        t.text2,
                                    )
                                    .on_hover_text(tr("Only you see a private dashboard"))
                                    .clicked()
                                    {
                                        d.private = !d.private;
                                        self.dirty = true;
                                    }
                                }
                                None => {
                                    ui.label(RichText::new(tr("Workspace overview")).size(20.0));
                                }
                            }
                            // switch between dashboards (the overview is one of them once made)
                            let mut pick = self.dash_open;
                            egui::ComboBox::from_id_salt("dash_pick")
                                .selected_text(tr("Switch"))
                                .show_ui(ui, |ui| {
                                    // the built-in overview; None would leave the dashboard for the hub
                                    ui.selectable_value(
                                        &mut pick,
                                        Some(super::hubs::WORKSPACE_OVERVIEW),
                                        tr("Workspace overview"),
                                    );
                                    for d in &dashes {
                                        ui.selectable_value(&mut pick, Some(d.id), &d.name);
                                    }
                                });
                            if pick != self.dash_open {
                                self.dash_open = pick;
                                self.sweep.confirm = None;
                                self.sweep.card_rename = None;
                            }
                            if let Some(di) = di {
                                let spaces: Vec<(u64, String)> = self
                                    .data
                                    .spaces
                                    .iter()
                                    .map(|s| (s.id, s.name.clone()))
                                    .collect();
                                let cur_space = self.data.extra.dashboards[di]
                                    .space
                                    .and_then(|sid| spaces.iter().find(|s| s.0 == sid))
                                    .map(|s| s.1.clone())
                                    .unwrap_or_else(|| tr("All Spaces").into());
                                let mut sp = self.data.extra.dashboards[di].space;
                                egui::ComboBox::from_id_salt("dash_space")
                                    .selected_text(cur_space)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut sp, None, tr("All Spaces"));
                                        for (id, n) in &spaces {
                                            ui.selectable_value(&mut sp, Some(*id), n);
                                        }
                                    });
                                if sp != self.data.extra.dashboards[di].space {
                                    self.data.extra.dashboards[di].space = sp;
                                    self.dirty = true;
                                }
                                let cards = self.data.extra.dashboards[di]
                                    .cards
                                    .clone()
                                    .unwrap_or_else(|| {
                                        super::more::default_dash_cards(
                                            &self.data.extra.dashboards[di].kind,
                                        )
                                    });
                                let add = ui.button(tr("+ Add card"));
                                popup(ui, &add, ui.id().with("dash_add_card"), 200.0, |ui| {
                                    for (k, title) in super::more::DASH_CARDS {
                                        let b = ui.add_enabled(
                                            !cards.iter().any(|c| c.kind == k),
                                            ghost(egui::Button::new(tr(title))),
                                        );
                                        name_it(&b, &format!("Add card {}", title));
                                        if b.clicked() {
                                            let mut cards = cards.clone();
                                            cards.push(DashCard {
                                                kind: k.into(),
                                                title: String::new(),
                                            });
                                            self.data.extra.dashboards[di].cards = Some(cards);
                                            self.dirty = true;
                                            close_popup(ui);
                                        }
                                    }
                                });
                                // framed like the other controls in this row
                                if ui
                                    .button(RichText::new(tr("Delete")).color(t.danger))
                                    .clicked()
                                {
                                    let d = &self.data.extra.dashboards[di];
                                    self.modal = Some(Modal::ConfirmRemove {
                                        kind: "dashboard",
                                        id: d.id,
                                        name: d.name.clone(),
                                    });
                                }
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if primary_button(ui, &t, tr("+ Dashboard")).clicked() {
                                    self.modal = Some(Modal::NewDashboard {
                                        name: tr("New Dashboard").into(),
                                    });
                                }
                            });
                        });
                        ui.add_space(10.0);
                        self.dashboard_cards(ui);
                    });
            });
    }

    /// The dashboard's cards in their order: totals and lists take the full
    /// width, the charts sit two to a row. Each card can move, be renamed or go.
    fn dashboard_cards(&mut self, ui: &mut Ui) {
        let t = self.t();
        let dash = self
            .dash_open
            .and_then(|id| self.data.extra.dashboards.iter().find(|d| d.id == id))
            .cloned();
        // the dashboard's own cards once customised, else its template's set
        let cards = dash
            .as_ref()
            .and_then(|d| d.cards.clone())
            .unwrap_or_else(|| {
                super::more::default_dash_cards(dash.as_ref().map_or("", |d| d.kind.as_str()))
            });
        if cards.is_empty() {
            ui.label(RichText::new(tr("No cards. Add one with + Add card.")).color(t.text3));
            return;
        }
        let space_lists: Option<Vec<u64>> = dash
            .as_ref()
            .and_then(|d| d.space)
            .and_then(|sid| self.data.lists_in(Location::Space(sid)));
        // the Space filter, and nothing archived (tasks, or their list or space)
        let in_view = |x: &Task| {
            space_lists
                .as_ref()
                .map(|l| l.contains(&x.list_id))
                .unwrap_or(true)
                && self.data.task_live(x)
        };
        let scope: Vec<Task> = self
            .data
            .tasks
            .iter()
            .filter(|x| in_view(x))
            .cloned()
            .collect();
        // time tracked stays counted after its task is archived (Timesheets
        // shows it too), so the time cards take archived tasks as well
        let tracked: Vec<Task> = self
            .data
            .tasks
            .iter()
            .filter(|x| {
                space_lists.as_ref().is_none_or(|l| l.contains(&x.list_id))
                    && (!x.time_entries.is_empty() || !x.running_timers().is_empty())
            })
            .cloned()
            .collect();
        let w = ui.available_width();
        let half = (w - 12.0) / 2.0;
        let full = |k: &str| k == "stats" || k == "overdue";
        let n = cards.len();
        let mut change: Option<(usize, i32)> = None; // (card, -1 up / 1 down / 0 remove / 2 rename)
        let mut open = None;
        let mut i = 0;
        while i < n {
            let pair = !full(&cards[i].kind) && i + 1 < n && !full(&cards[i + 1].kind);
            let row: Vec<usize> = if pair { vec![i, i + 1] } else { vec![i] };
            ui.horizontal_top(|ui| {
                for (k, &ci) in row.iter().enumerate() {
                    if k > 0 {
                        // 12 between the two, item spacing included, so the
                        // pair is exactly as wide as a full-width card
                        ui.add_space(12.0 - ui.spacing().item_spacing.x);
                    }
                    let cw = if full(&cards[ci].kind) { w } else { half };
                    egui::Frame::new()
                        .fill(t.card)
                        .stroke(Stroke::new(1.0_f32, t.border))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::same(16))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(cw - 34.0);
                                let title = super::more::dash_card_title(&cards[ci]);
                                ui.horizontal(|ui| {
                                    if let Some((_, text)) =
                                        self.sweep.card_rename.as_mut().filter(|r| r.0 == ci)
                                    {
                                        let r = ui.add(field(text).desired_width(200.0));
                                        name_it(&r, "Card name");
                                        if r.lost_focus() {
                                            change = Some((ci, 3));
                                        } else {
                                            r.request_focus();
                                        }
                                    } else {
                                        ui.label(
                                            RichText::new(if cards[ci].title.is_empty() {
                                                tr(&title)
                                            } else {
                                                &title
                                            })
                                            .size(14.5)
                                            .strong(),
                                        );
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        for (glyph, what, code, on) in [
                                            ("\u{00D7}", "Remove card", 0, true),
                                            ("pen", "Rename card", 2, true),
                                            ("\u{2193}", "Move card down", 1, ci + 1 < n),
                                            ("\u{2191}", "Move card up", -1, ci > 0),
                                        ] {
                                            // the pencil is a line icon; the rest characters
                                            let icon = glyph == "pen";
                                            let b = ui.add_enabled(
                                                on,
                                                ghost(egui::Button::new(
                                                    RichText::new(if icon { "   " } else { glyph })
                                                        .small()
                                                        .color(t.text3),
                                                )),
                                            );
                                            if icon {
                                                super::icons::line_icon_scaled(
                                                    ui.painter(),
                                                    "pen",
                                                    b.rect.center(),
                                                    t.text3,
                                                    0.7,
                                                );
                                            }
                                            name_it(&b, &format!("{} {}", what, title));
                                            if b.clicked() {
                                                change = Some((ci, code));
                                            }
                                        }
                                    });
                                });
                                ui.add_space(6.0);
                                // subtasks count like any task, as on the Teams page
                                let tasks: Vec<&Task> = scope.iter().collect();
                                if let Some(id) = self.dash_card_body(
                                    ui,
                                    &cards[ci].kind,
                                    cw - 34.0,
                                    &tasks,
                                    &tracked,
                                ) {
                                    open = Some(id);
                                }
                            });
                        });
                }
            });
            ui.add_space(12.0);
            i += row.len();
        }
        if let Some(id) = open {
            self.actions.push(Action::Open(id));
        }
        let Some((ci, code)) = change else { return };
        let Some(di) = self
            .dash_open
            .and_then(|id| self.data.extra.dashboards.iter().position(|d| d.id == id))
        else {
            return;
        };
        let mut cards = cards;
        match code {
            -1 => cards.swap(ci, ci - 1),
            1 => cards.swap(ci, ci + 1),
            0 => {
                cards.remove(ci);
            }
            2 => {
                self.sweep.card_rename = Some((ci, super::more::dash_card_title(&cards[ci])));
                return;
            }
            _ => {
                let text = self
                    .sweep
                    .card_rename
                    .take()
                    .map(|r| r.1.trim().to_string())
                    .unwrap_or_default();
                let standard = super::more::DASH_CARDS
                    .iter()
                    .find(|k| k.0 == cards[ci].kind)
                    .map(|k| k.1)
                    .unwrap_or("");
                cards[ci].title = if text == standard {
                    String::new()
                } else {
                    text
                };
            }
        }
        self.data.extra.dashboards[di].cards = Some(cards);
        self.dirty = true;
    }

    /// One card's contents; returns a task clicked in it. `tasks` are the
    /// tasks in view, subtasks included; `scope` every task with tracked
    /// time, archived ones included.
    fn dash_card_body(
        &self,
        ui: &mut Ui,
        kind: &str,
        w: f32,
        tasks: &[&Task],
        scope: &[Task],
    ) -> Option<u64> {
        let t = self.t();
        let td = today();
        let now = now_ts();
        let overdue: Vec<&&Task> = tasks
            .iter()
            .filter(|x| !self.data.is_closed(x) && x.is_overdue())
            .collect();
        match kind {
            "stats" => {
                let open = tasks.iter().filter(|x| !self.data.is_closed(x)).count();
                ui.horizontal(|ui| {
                    let tw = (w - 36.0) / 4.0;
                    for (label, v, c) in [
                        ("Total tasks", tasks.len(), t.text),
                        ("Open", open, rgb([0x5f, 0x55, 0xee])),
                        ("Overdue", overdue.len(), t.danger),
                        ("Completed", tasks.len() - open, rgb([0x00, 0x88, 0x44])),
                    ] {
                        egui::Frame::new()
                            .fill(t.bg)
                            .stroke(Stroke::new(1.0_f32, t.border))
                            .corner_radius(10)
                            .inner_margin(egui::Margin::same(14))
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(tw - 30.0);
                                    ui.label(
                                        RichText::new(if label == "Open" {
                                            crate::i18n::tr_ctx("Open (tasks)", label)
                                        } else {
                                            tr(label)
                                        })
                                        .color(t.text2),
                                    );
                                    ui.label(
                                        RichText::new(v.to_string()).size(28.0).strong().color(c),
                                    );
                                });
                            });
                    }
                });
            }
            "status" => {
                let mut sts: Vec<Status> = vec![];
                for s in &self.data.spaces {
                    for st in &s.statuses {
                        if !sts.iter().any(|x| x.name == st.name) {
                            sts.push(st.clone());
                        }
                    }
                }
                // no space yet: the statuses a new space starts with, all at 0
                if sts.is_empty() {
                    sts = default_statuses();
                }
                let counts: Vec<usize> = sts
                    .iter()
                    .map(|st| {
                        tasks
                            .iter()
                            .filter(|x| self.data.status_of(x).name == st.name)
                            .count()
                    })
                    .collect();
                let max = counts.iter().copied().max().unwrap_or(1) as f32;
                for (st, n) in sts.iter().zip(counts) {
                    bar_row(
                        ui,
                        &t,
                        &crate::i18n::status_text(&st.name).to_uppercase(),
                        n as f32,
                        max,
                        rgb(st.color),
                        n.to_string(),
                    );
                }
            }
            "priority" => {
                let counts: Vec<usize> = (0..5u8)
                    .map(|p| {
                        tasks
                            .iter()
                            .filter(|x| x.priority == p && !self.data.is_closed(x))
                            .count()
                    })
                    .collect();
                let max = counts.iter().copied().max().unwrap_or(1) as f32;
                for p in [1u8, 2, 3, 4, 0] {
                    bar_row(
                        ui,
                        &t,
                        tr(PRIORITIES[p as usize].label),
                        counts[p as usize] as f32,
                        max,
                        priority_color(p, &t),
                        counts[p as usize].to_string(),
                    );
                }
            }
            "workload" => {
                let counts: Vec<(String, usize, Color32)> = self
                    .data
                    .members
                    .iter()
                    .map(|m| {
                        (
                            m.name.clone(),
                            tasks
                                .iter()
                                .filter(|x| x.assignees.contains(&m.id) && !self.data.is_closed(x))
                                .count(),
                            rgb(m.color),
                        )
                    })
                    .collect();
                // one person: a bar against themselves is always full and
                // reads like progress, so show the number alone
                if counts.len() == 1 {
                    for (name, n, _) in counts {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(trf!("{} open", n)).color(t.text2));
                            });
                        });
                    }
                    return None;
                }
                ui.label(
                    RichText::new(tr("Bars compare open tasks with the busiest person."))
                        .small()
                        .color(t.text3),
                );
                let max = counts.iter().map(|c| c.1).max().unwrap_or(1) as f32;
                for (name, n, c) in counts {
                    bar_row(ui, &t, &name, n as f32, max, c, trf!("{} open", n));
                }
            }
            "time_people" => {
                let ws = week_start(td);
                let solo = self.data.solo();
                let me = self.data.me;
                let mut per: HashMap<u64, i64> = HashMap::new();
                for task in scope {
                    for e in &task.time_entries {
                        if (0..7).contains(&(local_date(e.start) - ws).num_days()) {
                            let o = match task.entry_owner(e) {
                                0 if solo => me,
                                o => o,
                            };
                            *per.entry(o).or_default() += e.end - e.start;
                        }
                    }
                    for (by, st) in task.running_timers() {
                        let Some(by) = by.or(if solo { Some(me) } else { None }) else {
                            continue;
                        };
                        if (0..7).contains(&(local_date(st) - ws).num_days()) {
                            *per.entry(by).or_default() += now - st;
                        }
                    }
                }
                let rows: Vec<(String, i64, Color32)> = self
                    .data
                    .members
                    .iter()
                    .map(|m| {
                        (
                            m.name.clone(),
                            per.get(&m.id).copied().unwrap_or(0),
                            rgb(m.color),
                        )
                    })
                    .collect();
                let max = rows.iter().map(|r| r.1).max().unwrap_or(1).max(1) as f32;
                for (name, secs, c) in rows {
                    bar_row(ui, &t, &name, secs as f32, max, c, fmt_hm(secs));
                }
            }
            "time" => {
                let ws = week_start(td);
                let mut per_day = [0i64; 7];
                for task in scope {
                    for e in &task.time_entries {
                        let i = (local_date(e.start) - ws).num_days();
                        if (0..7).contains(&i) {
                            per_day[i as usize] += e.end - e.start;
                        }
                    }
                    for (_, st) in task.running_timers() {
                        let i = (local_date(st) - ws).num_days();
                        if (0..7).contains(&i) {
                            per_day[i as usize] += now - st;
                        }
                    }
                }
                let max = per_day.iter().copied().max().unwrap_or(1).max(1) as f32;
                for (i, secs) in per_day.iter().enumerate() {
                    let d = ws + Duration::days(i as i64);
                    bar_row(
                        ui,
                        &t,
                        &d.text("%a %-d"),
                        *secs as f32,
                        max,
                        t.primary,
                        fmt_hm(*secs),
                    );
                }
            }
            _ => {
                if overdue.is_empty() {
                    ui.label(RichText::new(tr("Nothing is overdue")).color(t.text3));
                }
                let mut open = None;
                // the oldest first, and only so many: the count says the rest
                let mut overdue = overdue;
                overdue.sort_by_key(|x| x.due_moment());
                const ROWS: usize = 10;
                for task in overdue.iter().take(ROWS) {
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 5.0, t.hover);
                    }
                    let st = self.data.status_of(task);
                    paint_status_circle(ui.painter(), r.left_center() + vec2(10.0, 0.0), 6.0, &st);
                    let mut name_right = r.right() - 6.0;
                    if let Some(d) = fmt_due(task) {
                        let dr = ui.painter().text(
                            r.right_center() - vec2(6.0, 0.0),
                            Align2::RIGHT_CENTER,
                            d,
                            FontId::proportional(12.0),
                            t.danger,
                        );
                        name_right = dr.left() - 10.0;
                    }
                    paint_fit(
                        ui,
                        r.left_center() + vec2(24.0, 0.0),
                        Align2::LEFT_CENTER,
                        &task.name,
                        FontId::proportional(13.0),
                        t.text,
                        name_right - (r.left() + 24.0),
                    );
                    if resp.clicked() {
                        open = Some(task.id);
                    }
                }
                if overdue.len() > ROWS {
                    ui.label(
                        RichText::new(trf!("+{} more", overdue.len() - ROWS))
                            .small()
                            .color(t.text3),
                    );
                }
                return open;
            }
        }
        None
    }
}
