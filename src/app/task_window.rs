// The task window: the dialog a task opens in, with its fields on the left
// and comments and activity on the right.

use super::icons::icon_button_ex;
use super::*;

impl App {
    pub(super) fn task_window(&mut self, ctx: &egui::Context) {
        let Some(tid) = self.open_task else { return };
        self.swap_comment_draft(tid);
        // comments need ids to be edited, reacted to and replied to
        if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == tid) {
            fix_task_activity_ids(t);
        }
        let Some(orig) = self.task(tid).cloned() else {
            self.open_task = None;
            return;
        };
        let t = self.t();
        let screen = ctx.screen_rect();
        // dim backdrop that closes on click
        let mut close = false;
        egui::Area::new(egui::Id::new("task_dim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let r = ui.allocate_rect(screen, Sense::click());
                crate::widgets::no_pointer(&r);
                ui.painter().rect_filled(screen, 0.0, t.overlay);
                if r.clicked() {
                    close = true;
                }
            });
        let size = vec2(screen.width() - 48.0, screen.height() - 48.0);
        let win_id = egui::Id::new("task_window");
        let mut task = orig.clone();
        let now = now_ts();
        let statuses = self.data.statuses_for_list(task.list_id);
        let level = self.list_level(task.list_id);
        egui::Window::new("task")
            .id(win_id)
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .fixed_size(size)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                egui::Frame::new()
                    .fill(t.card)
                    .corner_radius(12)
                    .stroke(Stroke::new(1.0_f32, t.border)),
            )
            .show(ctx, |ui| {
                ui.set_min_size(size);
                // header
                if self.task_header(ui, t, tid, &mut task, win_id, level) {
                    close = true;
                }
                let r = ui.available_rect_before_wrap();
                ui.painter()
                    .hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
                let body = ui.available_rect_before_wrap();
                let right_w = 380.0;
                let left =
                    Rect::from_min_max(body.min, pos2(body.right() - right_w, body.bottom()));
                let right = Rect::from_min_max(pos2(body.right() - right_w, body.top()), body.max);
                ui.painter().rect_filled(
                    right,
                    egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 0,
                        se: 12,
                    },
                    t.column_bg,
                );
                ui.painter().vline(
                    right.left(),
                    right.y_range(),
                    Stroke::new(1.0_f32, t.border),
                );
                // ---- left
                let mut left_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(left.shrink2(vec2(28.0, 14.0)))
                        .layout(Layout::top_down(Align::Min)),
                );
                egui::ScrollArea::vertical()
                    .id_salt("detail_left")
                    .auto_shrink([false, false])
                    .show(&mut left_ui, |ui| {
                        // shared with me to view or comment: the fields read only
                        if level < crate::sync::Level::Edit {
                            ui.disable();
                        }
                        // task type label above the title, as in the original
                        taskfields::task_type_pill(ui, &t, win_id, &mut task.milestone);
                        ui.add_space(6.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut task.name)
                                .font(FontId::proportional(30.0))
                                .frame(false)
                                .desired_width(f32::INFINITY),
                        );
                        ui.add_space(10.0);
                        self.task_fields(ui, ctx, t, tid, &mut task, win_id, now, &statuses);
                        ui.add_space(14.0);
                        let r = ui.available_rect_before_wrap();
                        ui.painter()
                            .hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
                        ui.add_space(10.0);
                        if self.ai_offered() {
                            // the AI's writing tools for the description
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(tr("Description")).strong());
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let b = icon_button_ex(
                                        ui,
                                        &t,
                                        Icon::Sparkle,
                                        tr("Write with AI"),
                                        "Description AI",
                                        false,
                                        0.0,
                                        t.text2,
                                        super::ai::AI_VIOLET,
                                    );
                                    let now_says = task.clone();
                                    popup(ui, &b, ui.id().with(("desc_ai", tid)), 240.0, |ui| {
                                        self.description_ai_menu(ui, &now_says);
                                    });
                                });
                            });
                        }
                        self.description_field(ui, &t, &mut task);
                        ui.add_space(16.0);
                        self.task_subtasks(ui, t, tid, &mut task, win_id, &statuses);
                        ui.add_space(16.0);
                        self.relationships_section(ui, &t, win_id, tid);
                        ui.add_space(16.0);
                        self.task_checklist(ui, t, tid, &mut task, win_id);
                        ui.add_space(16.0);
                        self.attachments_section(ui, &t, &mut task);
                        ui.add_space(30.0);
                    });
                self.task_activity(ui, t, tid, &mut task, win_id, level, right);
            });
        // a dialog opened from the window (picker, Move to) must stay above
        // it: egui keeps the old order between two layers that both ask
        if self.modal.is_none() {
            ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, win_id));
        }
        // pictures and videos dropped on the window attach to the task
        for f in ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        }) {
            self.add_task_file(tid, f);
        }
        if self.task_window_keys(ctx, &mut task) {
            close = true;
        }
        if task_changed(&orig, &task) {
            self.actions.push(Action::Replace(task));
        }
        if close {
            self.open_task = None;
        }
    }

    /// The title row: breadcrumb, previous/next, share, menu and close.
    fn task_header(
        &mut self,
        ui: &mut Ui,
        t: Theme,
        tid: u64,
        mut task: &mut Task,
        win_id: egui::Id,
        level: crate::sync::Level,
    ) -> bool {
        let mut close = false;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(18, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // previous / next task in the current view
                    let (prev, next) = self.neighbours(tid);
                    if ui
                        .add_enabled(
                            prev.is_some(),
                            ghost(
                                egui::Button::new("\u{2191}").min_size(vec2(CONTROL_H, CONTROL_H)),
                            ),
                        )
                        .on_hover_text(tr("Previous task"))
                        .clicked()
                    {
                        if let Some(p) = prev {
                            self.actions.push(Action::Open(p));
                        }
                    }
                    if ui
                        .add_enabled(
                            next.is_some(),
                            ghost(
                                egui::Button::new("\u{2193}").min_size(vec2(CONTROL_H, CONTROL_H)),
                            ),
                        )
                        .on_hover_text(tr("Next task"))
                        .clicked()
                    {
                        if let Some(n) = next {
                            self.actions.push(Action::Open(n));
                        }
                    }
                    let path = self.data.list_path(task.list_id).join("  /  ");
                    if ui
                        .add(ghost(egui::Button::new(RichText::new(path).color(t.text2))))
                        .on_hover_text(tr("Move to another List"))
                        .clicked()
                    {
                        self.modal = Some(Modal::MoveTasks {
                            ids: vec![tid],
                            query: String::new(),
                        });
                    }
                    if task.archived {
                        ui.label(RichText::new(tr("Archived")).small().color(t.danger));
                    }
                    self.level_note(ui, &t, level);
                    if let Some(pid) = task.parent {
                        if let Some(p) = self.task(pid) {
                            ui.label(RichText::new("/").color(t.text3));
                            if ui.link(truncate(&p.name, 30)).clicked() {
                                self.actions.push(Action::Open(pid));
                            }
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icons::icon_button(ui, &t, Icon::Close, "", "Close task", false)
                            .on_hover_text(tr("Close"))
                            .clicked()
                        {
                            close = true;
                        }
                        // the menu has "Copy ID", so no separate ID button here
                        let r = icons::icon_button(ui, &t, Icon::Dots, "", "Task actions", false)
                            .on_hover_text(tr("More"));
                        self.watchers_header(ui, &t, win_id, &mut task);
                        // a real menu, like the one a task's right click opens:
                        // rows without a frame each
                        click_menu(&r, |ui| {
                            if ui.button(tr("Move to...")).clicked() {
                                self.modal = Some(Modal::MoveTasks {
                                    ids: vec![tid],
                                    query: String::new(),
                                });
                                ui.close_menu();
                            }
                            if ui.button(tr("Make subtask of...")).clicked() {
                                self.modal = Some(Modal::PickTask {
                                    task: tid,
                                    mode: TaskPick::Parent,
                                    query: String::new(),
                                });
                                ui.close_menu();
                            }
                            if task.parent.is_some()
                                && ui.button(tr("Make top-level task")).clicked()
                            {
                                task.parent = None;
                                self.make_top_level(tid);
                                ui.close_menu();
                            }
                            if ui.button(tr("Duplicate")).clicked() {
                                if let Some(n) = self.duplicate_task(tid) {
                                    self.actions.push(Action::Open(n));
                                }
                                ui.close_menu();
                            }
                            if ui.button(tr("Remind me")).clicked() {
                                self.modal = Some(Modal::reminder(
                                    task.name.clone(),
                                    today() + Duration::days(1),
                                ));
                                self.reminder_about = (Some(task.id), None);
                                ui.close_menu();
                            }
                            if ui.button(tr("Copy ID")).clicked() {
                                ui.ctx().copy_text(tid.to_string());
                                self.toast(tr("Task ID copied"));
                                ui.close_menu();
                            }
                            if ui
                                .button(if task.archived {
                                    tr("Unarchive")
                                } else {
                                    tr("Archive")
                                })
                                .clicked()
                            {
                                let a = !task.archived;
                                self.set_archived(&[tid], a);
                                ui.close_menu();
                            }
                            ui.separator();
                            if self.may_manage() && ui.button(tr("Save as template")).clicked() {
                                let id = self.data.new_id();
                                self.data.extra.templates.push(TaskTemplate {
                                    id,
                                    name: task.name.clone(),
                                    task_name: task.name.clone(),
                                    description: task.description.clone(),
                                    checklist: task
                                        .checklist
                                        .iter()
                                        .map(|c| c.text.clone())
                                        .collect(),
                                    tags: task.tags.clone(),
                                    priority: task.priority,
                                    ..Default::default()
                                });
                                self.toast = Some((tr("Saved as a template").into(), now_ts()));
                                self.dirty = true;
                                ui.close_menu();
                            }
                            if ui.button(tr("Copy link")).clicked() {
                                self.copy_link(ui, nav::task_link(tid));
                                ui.close_menu();
                            }
                            if ui
                                .button(if self.data.is_my_timer(&task) {
                                    tr("Stop timer")
                                } else {
                                    tr("Start timer")
                                })
                                .clicked()
                            {
                                self.actions.push(Action::ToggleTimer(tid));
                                ui.close_menu();
                            }
                            if ui
                                .button(RichText::new(tr("Delete task")).color(t.danger))
                                .clicked()
                            {
                                self.actions.push(Action::DeleteTask(tid));
                                ui.close_menu();
                            }
                        });
                    });
                });
            });
        close
    }

    /// The grid of fields: status, assignees, dates, priority, estimate, time, tags and custom fields.
    fn task_fields(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        t: Theme,
        tid: u64,
        mut task: &mut Task,
        win_id: egui::Id,
        now: i64,
        statuses: &Vec<Status>,
    ) {
        // a narrow window stacks the fields one per row instead of
        // two side by side (they ran under the comments column)
        let one_col = ui.available_width() < 560.0;
        let field_w = if one_col {
            (ui.available_width() - 10.0).max(200.0)
        } else {
            (ui.available_width() / 2.0 - 10.0).max(200.0)
        };
        let apps = self.data.extra.apps.clone();
        let fields = self.data.extra.fields.clone();
        egui::Grid::new("fields")
            .num_columns(4)
            .spacing(vec2(12.0, 10.0))
            .min_col_width(90.0)
            .show(ui, |ui| {
                // Status
                icons::icon_label(
                    ui,
                    Icon::Status,
                    RichText::new(tr("Status")).color(t.text2),
                    t.text2,
                );
                ui.horizontal(|ui| {
                    ui.set_width(field_w - 110.0);
                    let st = self.data.status_of(&task);
                    let r = status_pill(ui, &st, Sense::click());
                    name_it(&r, "Task status");
                    popup(ui, &r, win_id.with("st"), 200.0, |ui| {
                        status_menu(ui, &t, &statuses, &mut task.status)
                    });
                });
                if one_col {
                    ui.end_row();
                }
                // Assignees
                icons::icon_label(
                    ui,
                    Icon::Person,
                    RichText::new(tr("Assignees")).color(t.text2),
                    t.text2,
                );
                let (ar, aresp) =
                    ui.allocate_exact_size(vec2(field_w - 110.0, 26.0), Sense::click());
                if aresp.hovered() {
                    ui.painter().rect_filled(ar, 6.0, t.hover);
                }
                let used = paint_avatars(
                    ui.painter(),
                    ar.left_center() + vec2(4.0, 0.0),
                    11.0,
                    &task.assignees,
                    &self.data,
                    t.card,
                );
                if used == 0.0 {
                    ui.painter().text(
                        ar.left_center() + vec2(6.0, 0.0),
                        Align2::LEFT_CENTER,
                        tr("Empty"),
                        FontId::proportional(13.0),
                        t.text3,
                    );
                }
                let data = &self.data;
                popup(ui, &aresp, win_id.with("as"), 220.0, |ui| {
                    assignee_menu(ui, &t, data, &mut task.assignees)
                });
                ui.end_row();
                // Dates
                icons::icon_label(
                    ui,
                    Icon::Calendar,
                    RichText::new(tr("Dates")).color(t.text2),
                    t.text2,
                );
                ui.horizontal(|ui| {
                    let s_txt = fmt_start(&task).unwrap_or_else(|| tr("Start").into());
                    let r = ui.add(ghost(egui::Button::new(RichText::new(s_txt).color(
                        if task.start.is_some() {
                            t.text
                        } else {
                            t.text3
                        },
                    ))));
                    if r.clicked() {
                        ui.data_mut(|d| d.insert_temp(win_id.with("dates").with("which"), 0u8));
                    }
                    popup(ui, &r, win_id.with("sd"), DATE_MENU_W, |ui| {
                        dates_menu(ui, &t, win_id.with("dates"), &mut task)
                    });
                    ui.label(RichText::new("\u{2192}").color(t.text3));
                    let closed = statuses
                        .iter()
                        .find(|s| s.name == task.status)
                        .map(|s| matches!(s.kind, StatusKind::Closed | StatusKind::Done))
                        .unwrap_or(false);
                    let (d_txt, d_col) = match fmt_due(&task) {
                        Some(d) => (d, due_color(&task, closed, &t)),
                        None => (tr("Due").into(), t.text3),
                    };
                    let r = ui.add(ghost(egui::Button::new(RichText::new(d_txt).color(d_col))));
                    popup(ui, &r, win_id.with("dd"), DATE_MENU_W, |ui| {
                        dates_menu(ui, &t, win_id.with("dates2"), &mut task)
                    });
                    if task.recur.is_some() {
                        icons::icon_label(
                            ui,
                            Icon::Repeat,
                            RichText::new(tr(recur_label(&task.recur)))
                                .small()
                                .color(t.text2),
                            t.text2,
                        )
                        .on_hover_text(tr("Repeats when closed"));
                    }
                });
                if one_col {
                    ui.end_row();
                }
                // Priority
                if !apps.priorities {
                    if !one_col {
                        ui.label("");
                        ui.label("");
                    }
                } else {
                    icons::icon_label(
                        ui,
                        Icon::Flag,
                        RichText::new(tr("Priority")).color(t.text2),
                        t.text2,
                    );
                    let (pr, presp) =
                        ui.allocate_exact_size(vec2(field_w - 110.0, 26.0), Sense::click());
                    if presp.hovered() {
                        ui.painter().rect_filled(pr, 6.0, t.hover);
                    }
                    if task.priority > 0 {
                        paint_flag(
                            ui.painter(),
                            pr.left_center() + vec2(8.0, 0.0),
                            priority_color(task.priority, &t),
                            13.0,
                        );
                        ui.painter().text(
                            pr.left_center() + vec2(22.0, 0.0),
                            Align2::LEFT_CENTER,
                            tr(PRIORITIES[task.priority as usize].label),
                            FontId::proportional(13.0),
                            t.text,
                        );
                    } else {
                        ui.painter().text(
                            pr.left_center() + vec2(6.0, 0.0),
                            Align2::LEFT_CENTER,
                            tr("Empty"),
                            FontId::proportional(13.0),
                            t.text3,
                        );
                    }
                    if self.force_menu.as_deref() == Some("priority") {
                        ui.memory_mut(|m| m.open_popup(win_id.with("pr")));
                    }
                    popup(ui, &presp, win_id.with("pr"), 180.0, |ui| {
                        priority_menu(ui, &t, &mut task.priority)
                    });
                }
                if !one_col || apps.priorities {
                    ui.end_row();
                }
                // Time estimate
                if apps.time_estimates {
                    icons::icon_label(
                        ui,
                        Icon::Hourglass,
                        RichText::new(tr("Time estimate")).color(t.text2),
                        t.text2,
                    );
                    taskfields::estimate_field(
                        ui,
                        &t,
                        win_id,
                        field_w - 110.0,
                        &mut task.estimate_min,
                    );
                } else if !one_col {
                    ui.label("");
                    ui.label("");
                }
                if one_col && apps.time_estimates {
                    ui.end_row();
                }
                // Track time
                if !apps.time_tracking {
                    if !one_col {
                        ui.label("");
                        ui.label("");
                    }
                } else {
                    icons::icon_label(
                        ui,
                        Icon::Clock,
                        RichText::new(tr("Track time")).color(t.text2),
                        t.text2,
                    );
                    ui.horizontal(|ui| {
                        let running = self.data.is_my_timer(&task);
                        let (kind, fill, name) = if running {
                            (Icon::Stop, t.danger, "Stop timer")
                        } else {
                            (Icon::Play, t.primary, "Start timer")
                        };
                        let tb = ui.add(Tint(
                            egui::Button::new("")
                                .fill(fill)
                                // as wide as the row's control height, or the
                                // button is taller than wide and not a circle
                                .corner_radius(CONTROL_H / 2.0)
                                .min_size(vec2(CONTROL_H, CONTROL_H)),
                        ));
                        name_it(&tb, name);
                        // play and stop drawn small inside the round button
                        icons::line_icon_scaled(
                            ui.painter(),
                            kind,
                            tb.rect.center() + vec2(if running { 0.0 } else { 0.8 }, 0.0),
                            Color32::WHITE,
                            0.7,
                        );
                        if tb
                            .on_hover_text(if running {
                                tr("Stop timer")
                            } else {
                                tr("Start timer")
                            })
                            .clicked()
                        {
                            self.actions.push(Action::ToggleTimer(tid));
                        }
                        let secs = task.tracked_secs(now);
                        let te = ui.add(ghost(egui::Button::new(if secs > 0 {
                            fmt_duration(secs)
                        } else {
                            tr("Start").into()
                        })));
                        name_it(&te, "Time entries");
                        let te = te.on_hover_text(tr("Time entries, add time by hand"));
                        popup(ui, &te, win_id.with("entries"), 260.0, |ui| {
                            taskfields::time_entries_menu(
                                ui,
                                &t,
                                win_id.with("entries_m"),
                                &mut task,
                                self.data.me,
                            )
                        });
                        // a timer an older app started without an owner
                        // would otherwise run forever in a shared workspace
                        if self.data.ownerless_timer(&task)
                            && ui.button(tr("Stop (started by an older app)")).clicked()
                        {
                            self.actions.push(Action::StopOldTimer(tid));
                        }
                        if task.timer_started.is_some() || !task.timers.is_empty() {
                            ctx.request_repaint_after(std::time::Duration::from_secs(1));
                        }
                    });
                }
                if !one_col || apps.time_tracking {
                    ui.end_row();
                }
                // Tags (when the Tags ClickApp is on)
                if apps.tags {
                    icons::icon_label(
                        ui,
                        Icon::Tag,
                        RichText::new(tr("Tags")).color(t.text2),
                        t.text2,
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.set_max_width(field_w * 2.0);
                        let mut remove = None;
                        for (i, tag) in task.tags.iter().enumerate() {
                            if chip(ui, &format!("{}  \u{00D7}", tag), tag_color(tag))
                                .on_hover_text(tr("Remove tag"))
                                .clicked()
                            {
                                remove = Some(i);
                            }
                        }
                        if let Some(i) = remove {
                            task.tags.remove(i);
                        }
                        self.tag_field(ui, &t, win_id, &mut task);
                    });
                    ui.end_row();
                }
                // custom fields, two per row
                if apps.custom_fields {
                    for (i, f) in fields.iter().enumerate() {
                        icons::icon_label(
                            ui,
                            Icon::Fields,
                            RichText::new(&f.name).color(t.text2),
                            t.text2,
                        );
                        let v = task.custom.entry(f.id).or_default();
                        match f.kind.as_str() {
                            "checkbox" => {
                                let mut on = v == "true";
                                if check(ui, &mut on, "").changed() {
                                    *v = if on { "true".into() } else { String::new() };
                                }
                            }
                            "dropdown" => {
                                egui::ComboBox::from_id_salt(("cf", f.id))
                                    .selected_text(if v.is_empty() {
                                        tr("Empty").to_string()
                                    } else {
                                        v.clone()
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(v, String::new(), tr("Empty"));
                                        for o in &f.options {
                                            ui.selectable_value(v, o.clone(), o);
                                        }
                                    });
                            }
                            "date" => {
                                let mut d = NaiveDate::parse_from_str(v, "%Y-%m-%d").ok();
                                let r =
                                    ui.add(ghost(egui::Button::new(
                                        RichText::new(
                                            d.map(fmt_date).unwrap_or_else(|| tr("Empty").into()),
                                        )
                                        .color(if d.is_some() { t.text } else { t.text3 }),
                                    )));
                                if let Some(true) =
                                    popup(ui, &r, win_id.with(("cfd", f.id)), DATE_MENU_W, |ui| {
                                        date_menu(ui, &t, win_id.with(("cfdc", f.id)), &mut d)
                                    })
                                {
                                    *v = d.map(|d| d.to_string()).unwrap_or_default();
                                }
                            }
                            "number" => {
                                let r =
                                    ui.add(field(v).hint_text(tr("Empty")).desired_width(120.0));
                                if r.changed() {
                                    v.retain(|c| c.is_ascii_digit() || c == '.' || c == '-');
                                }
                            }
                            _ => {
                                ui.add(
                                    field(v)
                                        .hint_text(tr("Empty"))
                                        .desired_width(field_w - 120.0),
                                );
                            }
                        }
                        if one_col || i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                    if !one_col && fields.len() % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    }

    /// Subtasks: their rows, adding one and adding an existing task.
    fn task_subtasks(
        &mut self,
        ui: &mut Ui,
        t: Theme,
        tid: u64,
        task: &mut Task,
        win_id: egui::Id,
        statuses: &Vec<Status>,
    ) {
        // subtasks
        let subs: Vec<Task> = self.subtasks(tid).into_iter().cloned().collect();
        let done = subs.iter().filter(|s| self.data.is_closed(s)).count();
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Subtasks")).strong());
            if !subs.is_empty() {
                ui.label(RichText::new(format!("{}/{}", done, subs.len())).color(t.text3));
            }
            if self.ai_offered() {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let b = icon_button_ex(
                        ui,
                        &t,
                        Icon::Sparkle,
                        tr("Suggest subtasks"),
                        "Suggest subtasks",
                        false,
                        0.0,
                        t.text2,
                        super::ai::AI_VIOLET,
                    );
                    if b.clicked() {
                        self.ai_suggest_subtasks(task);
                    }
                });
            }
        });
        for s in &subs {
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 6.0, t.hover);
            }
            let st = self.data.status_of(s);
            let cr = Rect::from_center_size(r.left_center() + vec2(14.0, 0.0), vec2(18.0, 18.0));
            let c = ui.interact(cr, win_id.with(("subst", s.id)), Sense::click());
            paint_status_circle(ui.painter(), cr.center(), 7.0, &st);
            let mut se = s.clone();
            let mut sub_changed = false;
            if let Some(true) = popup(ui, &c, win_id.with(("substp", s.id)), 200.0, |ui| {
                status_menu(ui, &t, &statuses, &mut se.status)
            }) {
                sub_changed = true;
            }
            let closed = matches!(st.kind, StatusKind::Closed | StatusKind::Done);
            ui.painter()
                .with_clip_rect(Rect::from_min_max(
                    r.min,
                    pos2(r.right() - 200.0, r.bottom()),
                ))
                .text(
                    r.left_center() + vec2(32.0, 0.0),
                    Align2::LEFT_CENTER,
                    &s.name,
                    FontId::proportional(13.0),
                    if closed { t.text3 } else { t.text },
                );
            // assignees and due date open their menus; on hover
            // the row also offers "make top-level" and delete
            let hover = ui.rect_contains_pointer(r);
            let ar = Rect::from_min_max(
                pos2(r.right() - 146.0, r.top() + 4.0),
                pos2(r.right() - 84.0, r.bottom() - 4.0),
            );
            let a = ui.interact(ar, win_id.with(("subas", s.id)), Sense::click());
            name_it(&a, &format!("Subtask assignees {}", s.name));
            if a.hovered() {
                ui.painter().rect_filled(ar, 5.0, t.hover);
            }
            if paint_avatars(
                ui.painter(),
                ar.left_center() + vec2(4.0, 0.0),
                10.0,
                &s.assignees,
                &self.data,
                t.card,
            ) == 0.0
                && hover
            {
                icons::line_icon_scaled(
                    ui.painter(),
                    Icon::Person,
                    ar.left_center() + vec2(12.0, 0.0),
                    t.text3,
                    0.8,
                );
            }
            let data = &self.data;
            if let Some(true) = popup(ui, &a, win_id.with(("subasp", s.id)), 220.0, |ui| {
                assignee_menu(ui, &t, data, &mut se.assignees)
            }) {
                sub_changed = true;
            }
            let dr = Rect::from_min_max(
                pos2(r.right() - 80.0, r.top() + 4.0),
                pos2(r.right() - 4.0, r.bottom() - 4.0),
            );
            let d = ui.interact(dr, win_id.with(("subdue", s.id)), Sense::click());
            name_it(&d, &format!("Subtask due {}", s.name));
            if d.hovered() {
                ui.painter().rect_filled(dr, 5.0, t.hover);
            }
            match fmt_due(s) {
                Some(dd) => {
                    ui.painter().text(
                        dr.right_center() - vec2(4.0, 0.0),
                        Align2::RIGHT_CENTER,
                        dd,
                        FontId::proportional(12.0),
                        due_color(s, closed, &t),
                    );
                }
                None if hover => {
                    icons::line_icon_scaled(
                        ui.painter(),
                        Icon::Calendar,
                        dr.right_center() - vec2(10.0, 0.0),
                        t.text3,
                        0.8,
                    );
                }
                None => {}
            }
            if let Some(true) = popup(ui, &d, win_id.with(("subduep", s.id)), DATE_MENU_W, |ui| {
                date_time_menu(
                    ui,
                    &t,
                    win_id.with(("subduec", s.id)),
                    &mut se.due,
                    &mut se.due_time,
                )
            }) {
                sub_changed = true;
            }
            let mut top = false;
            let mut del = false;
            if hover {
                // interact instead of `ui.put`: put allocates layout space,
                // so the buttons appearing on hover pushed the rows below down
                let mut hover_btn = |dx: f32, key: &str, icon: Icon| {
                    let br = Rect::from_center_size(
                        pos2(r.right() - dx, r.center().y),
                        vec2(22.0, 22.0),
                    );
                    let b = ui.interact(br, win_id.with((key, s.id)), Sense::click());
                    if b.hovered() {
                        ui.painter().rect_filled(br, 5.0, t.hover);
                    }
                    icons::line_icon(ui.painter(), icon, br.center(), t.text2);
                    b
                };
                let b = hover_btn(162.0, "subtop", Icon::ArrowUpLeft);
                name_it(&b, &format!("Make top-level {}", s.name));
                top = b.on_hover_text(tr("Make it a task of its own")).clicked();
                let b = hover_btn(186.0, "subdel", Icon::Close);
                name_it(&b, &format!("Delete subtask {}", s.name));
                del = b.on_hover_text(tr("Delete")).clicked();
            }
            if del {
                self.actions.push(Action::DeleteTask(s.id));
            } else if top {
                se.parent = None;
                self.actions.push(Action::Replace(se));
            } else if sub_changed {
                self.actions.push(Action::Replace(se));
            } else if resp.clicked() {
                self.actions.push(Action::Open(s.id));
            }
        }
        // "Add subtask" row; the field appears only after a click
        let sub_on_id = win_id.with(("adding_sub", tid));
        let mut sub_on: bool = ui.data(|d| d.get_temp(sub_on_id)).unwrap_or(false);
        if sub_on {
            ui.horizontal(|ui| {
                let r = ui.add(
                    field(&mut self.sub_text)
                        .hint_text(tr("Task Name"))
                        .desired_width(ui.available_width() - 70.0),
                );
                name_field(&r, "Subtask name");
                focus_once(ui, &r);
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button(tr("Save")).clicked() || enter) && !self.sub_text.trim().is_empty() {
                    self.actions.push(Action::Add {
                        list: task.list_id,
                        name: self.sub_text.clone(),
                        status: None,
                        priority: DEFAULT_PRIORITY,
                        assignees: vec![],
                        due: None,
                        parent: Some(tid),
                        open: false,
                    });
                    self.sub_text.clear();
                    r.request_focus();
                } else if r.lost_focus() && self.sub_text.trim().is_empty() {
                    sub_on = false;
                }
            });
        } else if add_line(ui, &t, tr("+  Add subtask")).clicked() {
            sub_on = true;
        }
        ui.data_mut(|d| d.insert_temp(sub_on_id, sub_on));
        if add_line(ui, &t, tr("+  Add existing task")).clicked() {
            self.modal = Some(Modal::PickTask {
                task: tid,
                mode: TaskPick::Child,
                query: String::new(),
            });
        }
    }

    /// The checklist: its heading, rows and adding an item.
    fn task_checklist(
        &mut self,
        ui: &mut Ui,
        t: Theme,
        tid: u64,
        mut task: &mut Task,
        win_id: egui::Id,
    ) {
        // checklist
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Checklist")).strong());
            if !task.checklist.is_empty() {
                let d = task.checklist.iter().filter(|c| c.done).count();
                ui.label(RichText::new(format!("{}/{}", d, task.checklist.len())).color(t.text3));
            }
        });
        if !task.checklist.is_empty() {
            let d = task.checklist.iter().filter(|c| c.done).count() as f32
                / task.checklist.len() as f32;
            // a thin track with the done share filled; egui's ProgressBar
            // drew only a lone green dot at 0% because its track matched
            // the window background
            let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0), Sense::hover());
            ui.painter().rect_filled(bar, 2.0, t.hover);
            if d > 0.0 {
                let fill = Rect::from_min_size(bar.min, vec2(bar.width() * d, bar.height()));
                ui.painter().rect_filled(fill, 2.0, rgb([0x00, 0x88, 0x44]));
            }
        }
        self.checklist_rows(ui, &t, win_id, &mut task);
        let ck_on_id = win_id.with(("adding_check", tid));
        let mut ck_on: bool = ui.data(|d| d.get_temp(ck_on_id)).unwrap_or(false);
        if ck_on {
            ui.horizontal(|ui| {
                let r = ui.add(
                    field(&mut self.check_text)
                        .hint_text(tr("Checklist item"))
                        .desired_width(ui.available_width() - 70.0),
                );
                name_field(&r, "Checklist item");
                focus_once(ui, &r);
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button(tr("Save")).clicked() || enter) && !self.check_text.trim().is_empty()
                {
                    task.checklist.push(CheckItem {
                        text: self.check_text.trim().to_string(),
                        done: false,
                        assignee: None,
                    });
                    self.check_text.clear();
                    r.request_focus();
                } else if r.lost_focus() && self.check_text.trim().is_empty() {
                    ck_on = false;
                }
            });
        } else if add_line(
            ui,
            &t,
            if task.checklist.is_empty() {
                tr("+  Create checklist")
            } else {
                tr("+  Add item")
            },
        )
        .clicked()
        {
            ck_on = true;
        }
        ui.data_mut(|d| d.insert_temp(ck_on_id, ck_on));
    }

    /// The right column: comments and activity, search, and the comment box.
    fn task_activity(
        &mut self,
        ui: &mut Ui,
        t: Theme,
        tid: u64,
        mut task: &mut Task,
        win_id: egui::Id,
        level: crate::sync::Level,
        right: Rect,
    ) {
        // ---- right: activity
        let mut right_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(right.shrink2(vec2(16.0, 12.0)))
                .layout(Layout::top_down(Align::Min)),
        );
        right_ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Comments")).size(15.0).strong());
            let created = local(task.created)
                .map(|d| d.text("%b %-d"))
                .unwrap_or_default();
            ui.label(RichText::new(trf!("Created {}", created)).color(t.text3));
        });
        right_ui.add_space(6.0);
        let comment_id = win_id.with("comment_box");
        // comment search (the magnifier next to the heading)
        let find_id = win_id.with(("comment_find", tid));
        let mut find: Option<String> = right_ui.data(|d| d.get_temp(find_id)).unwrap_or(None);
        if let Some(q) = find.as_mut() {
            let mut drop = false;
            right_ui.horizontal(|ui| {
                let r = ui.add(
                    field(q)
                        .hint_text(tr("Search comments..."))
                        .desired_width(ui.available_width() - 30.0),
                );
                name_field(&r, "Search comments");
                focus_once(ui, &r);
                if icons::icon_button(ui, &t, Icon::Close, "", "Close search", false)
                    .on_hover_text(tr("Close search"))
                    .clicked()
                {
                    drop = true;
                }
            });
            if drop {
                find = None;
            }
        }
        let has_comments = task.activity.iter().any(|a| a.comment);
        let mut assign_resp: Option<egui::Response> = None;
        // alone in the workspace: nobody to mention or hand it to
        let alone = self.data.solo();
        let input_h = 150.0;
        let list_h = right_ui.available_height() - input_h;
        let q = find
            .as_ref()
            .map(|q| q.trim().to_lowercase())
            .filter(|q| !q.is_empty());
        let only_id = win_id.with("comments_only");
        let mut only_comments: bool = right_ui.data(|d| d.get_temp(only_id)).unwrap_or(false);
        let feed = egui::ScrollArea::vertical()
            .id_salt("activity")
            .max_height(list_h)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(&mut right_ui, |ui| {
                // the history lines show from the start, not only once a
                // comment is there
                self.comment_feed(ui, &mut task, q.as_deref(), only_comments, win_id);
                if has_comments || q.is_some() {
                    return;
                }
                // no comments yet: invite the first one under the history
                ui.add_space(18.0);
                ui.label(
                    RichText::new(tr("Send the first comment"))
                        .size(16.0)
                        .color(t.text),
                );
                ui.label(
                    RichText::new(tr(if alone {
                        "Keep notes and updates on this task here."
                    } else {
                        "Ask a question, share an update, or loop someone in."
                    }))
                    .size(13.0)
                    .color(t.text2),
                );
                if alone {
                    return;
                }
                ui.add_space(8.0);
                ui.horizontal(|bui| {
                    if bui.button(tr("@  Mention a teammate")).clicked() {
                        if !self.comment.text.ends_with('@') {
                            if !self.comment.text.is_empty() && !self.comment.text.ends_with(' ') {
                                self.comment.text.push(' ');
                            }
                            self.comment.text.push('@');
                        }
                        bui.data_mut(|d| d.insert_temp(win_id.with("mention_open"), true));
                        bui.memory_mut(|m| m.request_focus(comment_id));
                    }
                    assign_resp = Some(icons::icon_button(
                        bui,
                        &t,
                        Icon::Person,
                        tr("Assign this task"),
                        "Assign this task",
                        true,
                    ));
                });
            });
        if let Some(r) = assign_resp {
            let data = &self.data;
            popup(&right_ui, &r, win_id.with("assign_popup"), 220.0, |ui| {
                assignee_menu(ui, &t, data, &mut task.assignees)
            });
        }
        crate::widgets::redo_when_stuck_list_changes(right_ui.ctx(), &feed);
        right_ui.add_space(6.0);
        if level < crate::sync::Level::Comment {
            right_ui.disable();
        }
        // Comment / Post tabs, the box, then the tool row
        let tab_id = win_id.with("comment_tab");
        let mut post: bool = right_ui.data(|d| d.get_temp(tab_id)).unwrap_or(false);
        right_ui.horizontal(|ui| {
            if ui.selectable_label(!post, tr("Comment")).clicked() {
                post = false;
            }
            if ui.selectable_label(post, tr("Post")).clicked() {
                post = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let r =
                    icons::icon_button(ui, &t, Icon::Search, "", "Search comments button", false);
                if r.on_hover_text(tr("Search comments")).clicked() {
                    find = if find.is_some() {
                        None
                    } else {
                        Some(String::new())
                    };
                }
                // hide the "changed status..." lines
                let only =
                    ui.selectable_label(only_comments, RichText::new(tr("Comments only")).small());
                name_it(&only, "Comments only");
                if only.clicked() {
                    only_comments = !only_comments;
                }
            });
        });
        right_ui.data_mut(|d| {
            d.insert_temp(only_id, only_comments);
            d.insert_temp(tab_id, post);
            d.insert_temp(find_id, find.clone());
        });
        // Cmd+Enter sends (taken before the box sees it as a new line)
        let send_key = right_ui.memory(|m| m.has_focus(comment_id))
            && right_ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
        let names = self.mention_names();
        let tc = t.clone();
        let mut lay = |ui: &Ui, s: &str, w: f32| super::chat::mention_layout(ui, s, w, &names, &tc);
        let cr = right_ui.add(
            field_multi(&mut self.comment.text)
                .id(comment_id)
                .hint_text(if post {
                    tr("Write a post...")
                } else if alone {
                    tr("Write a comment")
                } else {
                    tr("Write a comment, @mention someone")
                })
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .layouter(&mut lay),
        );
        name_field(&cr, "Comment box");
        let mention_id = win_id.with("mention_open");
        // typing "@" (or a name after it) brings up the people list
        let typed_at = mention_word(&self.comment.text).map(str::to_string);
        if cr.changed() && typed_at.as_deref() == Some("") {
            right_ui.data_mut(|d| d.insert_temp(mention_id, true));
        }
        let mut insert: Option<String> = None;
        right_ui.horizontal(|ui| {
            let at = ui.add(ghost(egui::Button::new("@").min_size(vec2(24.0, 24.0))));
            name_it(&at, "Mention someone");
            let force: bool = ui.data(|d| d.get_temp(mention_id)).unwrap_or(false);
            // (opened a pass after the click, or that click would close it again)
            if force && !ui.input(|i| i.pointer.any_click() || i.pointer.any_released()) {
                ui.memory_mut(|m| m.open_popup(win_id.with("mention_popup")));
                ui.data_mut(|d| d.remove::<bool>(mention_id));
            }
            let typed = typed_at.as_deref().unwrap_or("").to_lowercase();
            let members: Vec<(u64, String)> = self
                .data
                .members
                .iter()
                .filter(|m| !m.invited && m.any_name(|n| n.to_lowercase().starts_with(&typed)))
                .map(|m| (m.id, m.shown().to_string()))
                .collect();
            popup(ui, &at, win_id.with("mention_popup"), 200.0, |ui| {
                if members.is_empty() {
                    ui.label(RichText::new(tr("No one by that name")).color(t.text3));
                }
                for (id, name) in &members {
                    let m = self.data.member(*id).cloned().unwrap_or_default();
                    let ring = t.card;
                    if menu_row(ui, &t, name, false, |p, c| {
                        paint_avatar(p, c, 8.0, &m, ring)
                    })
                    .clicked()
                    {
                        insert = Some(format!("@{} ", name));
                        self.comment.picked.push(*id);
                        close_popup(ui);
                    }
                }
            });
            let em = icons::icon_button(ui, &t, Icon::Smile, "", "Emoji", false);
            popup(ui, &em, win_id.with("emoji_popup"), 180.0, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for e in super::comments::REACTIONS {
                        if ui
                            .add(ghost(egui::Button::new(RichText::new(e).size(18.0))))
                            .clicked()
                        {
                            insert = Some(e.to_string());
                            close_popup(ui);
                        }
                    }
                });
            });
            let ck = icons::icon_button(ui, &t, Icon::Task, "", "Checklist item in comment", false);
            if ck.on_hover_text(tr("Add a checklist item")).clicked() {
                let lead = if self.comment.text.is_empty() || self.comment.text.ends_with('\n') {
                    ""
                } else {
                    "\n"
                };
                insert = Some(format!("{}\u{2610} ", lead));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if post { "Post" } else { "Comment" };
                let send = primary_button(ui, &t, tr(label));
                name_it(&send, &format!("Send {}", label));
                if (send.clicked() || send_key) && !self.comment.text.trim().is_empty() {
                    let id = self.data.new_id();
                    let text = self.comment.text.trim().to_string();
                    let mentions = crate::model::mentioned_ids(
                        &text,
                        &self.data.members,
                        &self.comment.picked,
                    );
                    task.activity.push(Activity {
                        ts: now_ts(),
                        author: self.data.me,
                        text,
                        comment: true,
                        post,
                        id,
                        mentions,
                        ..Default::default()
                    });
                    self.comment.text.clear();
                    self.comment.picked.clear();
                    close_popup(ui);
                }
            });
        });
        if let Some(text) = insert {
            // a picked @name replaces the "@" (and part of a name) already typed
            if text.starts_with('@') {
                if let Some(w) = mention_word(&self.comment.text) {
                    let keep = self.comment.text.len() - w.len() - 1;
                    self.comment.text.truncate(keep);
                }
            }
            self.comment.text.push_str(&text);
            right_ui.memory_mut(|m| m.request_focus(comment_id));
            let n = self.comment.text.chars().count();
            let mut st = egui::TextEdit::load_state(right_ui.ctx(), comment_id).unwrap_or_default();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(n),
            )));
            st.store(right_ui.ctx(), comment_id);
        }
    }

    /// Keys while the window is open: Escape and Cmd+digit priority.
    fn task_window_keys(&mut self, ctx: &egui::Context, task: &mut Task) -> bool {
        let mut close = false;
        // Escape in a text field (subtask, checklist, comment) only leaves the field
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && !self.menu_was_open
            && !self.typing_was_on
        {
            close = true;
        }
        // Cmd+1..4 set the priority, Cmd+0 clears it
        if self.data.extra.apps.priorities {
            for (k, p) in [
                (egui::Key::Num1, 1u8),
                (egui::Key::Num2, 2),
                (egui::Key::Num3, 3),
                (egui::Key::Num4, 4),
                (egui::Key::Num0, 0),
            ] {
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, k)) {
                    task.priority = p;
                }
            }
        }
        close
    }
}
