// The Inbox page (Primary / Other / Later / Cleared).

use super::*;

// ---------------------------------------------------------------- inbox (Primary / Other / Later / Cleared)

impl App {
    pub(crate) fn inbox_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        // tabs
        let w = ui.available_width();
        let (tabs_r, _) = ui.allocate_exact_size(vec2(w, 50.0), Sense::hover());
        let tw = (w - 20.0) / 4.0;
        for (i, (icon, label)) in [
            ("inbox", "Primary"),
            ("bolt", "Other"),
            ("alarm", "Later"),
            ("check", "Cleared"),
        ]
        .iter()
        .enumerate()
        {
            let r = Rect::from_min_size(
                pos2(tabs_r.left() + 10.0 + tw * i as f32, tabs_r.top()),
                vec2(tw, 50.0),
            );
            let resp = ui.interact(r, ui.id().with(("itab", i)), Sense::click());
            name_it(&resp, &format!("Inbox {}", label));
            let sel = self.inbox_tab == i as u8;
            if resp.hovered() && !sel {
                ui.painter().rect_filled(r.shrink(4.0), 6.0, t.hover);
            }
            line_icon(
                ui.painter(),
                icon,
                r.left_center() + vec2(26.0, 0.0),
                if sel { t.text } else { t.text2 },
            );
            ui.painter().text(
                r.left_center() + vec2(44.0, 0.0),
                Align2::LEFT_CENTER,
                tr(label),
                FontId::proportional(15.0),
                if sel { t.text } else { t.text2 },
            );
            if sel {
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        pos2(r.left(), r.bottom() - 2.0),
                        pos2(r.right() - 4.0, r.bottom()),
                    ),
                    1.0,
                    t.text,
                );
            }
            if i > 0 {
                ui.painter().vline(
                    r.left(),
                    r.top() + 14.0..=r.bottom() - 14.0,
                    Stroke::new(1.0_f32, t.border),
                );
            }
            if resp.clicked() {
                self.inbox_tab = i as u8;
            }
        }
        ui.painter().hline(
            tabs_r.x_range(),
            tabs_r.bottom(),
            Stroke::new(1.0_f32, t.border),
        );
        // collect items (what is on which tab: inbox_items)
        let mut items: Vec<super::nav::InboxItem> = self
            .inbox_items()
            .iter()
            .cloned()
            .filter(|x| x.tab == self.inbox_tab)
            .collect();
        let on_tab = items.len();
        match self.inbox_filter {
            1 => items.retain(|x| x.comment),
            2 => items.retain(|x| x.mention),
            3 => items.retain(|x| x.assigned),
            _ => {}
        }
        // the tab has rows, only the filter hides them all
        let filtered_out = items.is_empty() && on_tab > 0;
        items.sort_by_key(|x| std::cmp::Reverse(x.ts));
        let mut clear_all = false;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(18, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let label = ["Filter", "Comments", "Mentions", "Assigned"]
                        [self.inbox_filter as usize % 4];
                    // framed like the buttons on the right of this row
                    let fr = icon_button(ui, &t, "filter", tr(label), label, true);
                    popup(ui, &fr, ui.id().with("inbox_filter"), 180.0, |ui| {
                        for (i, l) in ["Everything", "Comments", "Mentions", "Assigned to me"]
                            .iter()
                            .enumerate()
                        {
                            if ui
                                .selectable_label(self.inbox_filter == i as u8, tr(l))
                                .clicked()
                            {
                                self.inbox_filter = i as u8;
                                close_popup(ui);
                            }
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let can_clear = !items.is_empty() && self.inbox_tab != 3;
                        if ui
                            .add_enabled_ui(can_clear, |ui| {
                                icon_button(
                                    ui,
                                    &t,
                                    "check",
                                    crate::i18n::tr_ctx("Clear all (inbox)", "Clear all"),
                                    "Clear all",
                                    true,
                                )
                            })
                            .inner
                            .clicked()
                            && can_clear
                        {
                            clear_all = true;
                        }
                        if icon_button(ui, &t, "settings", "", "Notification settings", true)
                            .on_hover_text(tr("Notification settings"))
                            .clicked()
                        {
                            self.modal = Some(Modal::Notifications);
                        }
                        if icon_button(ui, &t, "alarm", tr("Reminder"), "Reminder", true).clicked()
                        {
                            self.modal = Some(Modal::reminder(String::new(), today()));
                        }
                    });
                });
            });
        if clear_all {
            for k in items.iter().map(|x| &x.key) {
                self.data.extra.inbox_later.retain(|x| x != k);
                self.data.extra.inbox_cleared.push(k.clone());
            }
            self.dirty = true;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(18, 0))
                    .show(ui, |ui| {
                        if self.inbox_tab == 0 {
                            self.reminders_block(ui);
                        }
                        // reminders are waiting above: not "all caught up"
                        let reminders_shown = self.inbox_tab == 0
                            && self
                                .data
                                .extra
                                .reminders
                                .iter()
                                .any(|r| r.owner == self.data.me && !r.done);
                        if items.is_empty() && reminders_shown && !filtered_out {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(tr("No new comments, mentions or assignments."))
                                    .color(t.text3),
                            );
                        } else if items.is_empty() {
                            ui.add_space(ui.available_height() * 0.3);
                            ui.vertical_centered(|ui| {
                                let alone = self.data.members.len() <= 1;
                                // only someone who may invite is pointed to inviting
                                let invite = alone && self.may_manage();
                                // alone, the Inbox stays empty: point at what is due
                                let me = self.data.me;
                                let due_work = self
                                    .data
                                    .tasks
                                    .iter()
                                    .filter(|x| {
                                        x.assignees.contains(&me)
                                            && x.due.is_some_and(|d| d <= today())
                                            && !self.data.is_closed(x)
                                            // as the sidebar's Today & Overdue counts
                                            && self.task_visible(x)
                                    })
                                    .count();
                                let point_to_due =
                                    self.inbox_tab == 0 && alone && due_work > 0 && !filtered_out;
                                // the people icon only where it invites people
                                let (title, sub, icon) = match self.inbox_tab {
                                    _ if filtered_out => (
                                        "Nothing matches this filter",
                                        "This tab has items the filter hides.",
                                        "filter",
                                    ),
                                    0 if point_to_due => (
                                        "Nothing from other people",
                                        "The Inbox collects comments, mentions and assignments from others.",
                                        "check",
                                    ),
                                    0 if invite => (
                                        "Looking to collaborate?",
                                        "Collaboration is one invite away.",
                                        "teams",
                                    ),
                                    0 => (
                                        "You're all caught up",
                                        "New comments, mentions and assignments show up here.",
                                        "check",
                                    ),
                                    1 => (
                                        "Nothing in Other",
                                        "Updates that don't need you show up here.",
                                        "inbox",
                                    ),
                                    2 => (
                                        "Nothing saved for later",
                                        "Use Later on an item to come back to it.",
                                        "alarm",
                                    ),
                                    _ => (
                                        "Nothing cleared yet",
                                        "Cleared items show up here.",
                                        "check",
                                    ),
                                };
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                                ui.painter().rect_stroke(
                                    r.shrink(2.0),
                                    14.0,
                                    Stroke::new(1.5_f32, t.border),
                                    egui::StrokeKind::Inside,
                                );
                                line_icon_scaled(
                                    ui.painter(),
                                    icon,
                                    r.center(),
                                    Color32::from_rgb(0x5f, 0x55, 0xee),
                                    1.6,
                                );
                                ui.add_space(10.0);
                                ui.label(RichText::new(tr(title)).size(18.0).color(t.text));
                                ui.label(RichText::new(tr(sub)).size(14.0).color(t.text2));
                                if filtered_out {
                                    ui.add_space(10.0);
                                    if ui.button(tr("Show everything")).clicked() {
                                        self.inbox_filter = 0;
                                    }
                                } else if point_to_due {
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(trf!(
                                            "Tasks due today or overdue: {}",
                                            due_work
                                        ))
                                        .size(14.0)
                                        .color(t.text),
                                    );
                                    ui.add_space(6.0);
                                    if ui
                                        .add(Tint(
                                            egui::Button::new(
                                                RichText::new(tr("Open Today & Overdue"))
                                                    .size(15.0)
                                                    .color(t.on_primary),
                                            )
                                            .fill(t.primary)
                                            .corner_radius(7)
                                            .min_size(vec2(0.0, 38.0)),
                                        ))
                                        .clicked()
                                    {
                                        self.actions.push(Action::Go(Location::TodayOverdue));
                                    }
                                    ui.add_space(4.0);
                                    if invite && ui.button(tr("Invite people")).clicked() {
                                        self.modal = Some(Modal::Invite {
                                            emails: String::new(),
                                            role: "Member".into(),
                                        });
                                    }
                                } else if self.inbox_tab == 0 && invite && !filtered_out {
                                    ui.add_space(10.0);
                                    if ui
                                        .add(Tint(
                                            egui::Button::new(
                                                RichText::new(tr("Invite people"))
                                                    .size(15.0)
                                                    .color(t.on_primary),
                                            )
                                            .fill(t.primary)
                                            .corner_radius(7)
                                            .min_size(vec2(0.0, 38.0)),
                                        ))
                                        .clicked()
                                    {
                                        self.modal = Some(Modal::Invite {
                                            emails: String::new(),
                                            role: "Member".into(),
                                        });
                                    }
                                }
                            });
                        }
                        for it in items {
                            let (key, ts, tname) = (it.key.clone(), it.ts, it.title.clone());
                            let w = ui.available_width();
                            let (r, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click());
                            name_it(&resp, &tname);
                            // a row scrolled out of sight keeps its place but isn't
                            // drawn: thousands of rows would be on every frame
                            if !ui.is_rect_visible(r) {
                                // (still opened from the keyboard or a screen reader)
                                if row_press(&resp) {
                                    self.open_inbox_src(it.src);
                                }
                                continue;
                            }
                            if resp.hovered() {
                                ui.painter().rect_filled(r, 8.0, t.hover);
                            }
                            ui.painter()
                                .hline(r.x_range(), r.bottom(), Stroke::new(0.5_f32, t.border));
                            if let Some(m) = self.data.member(it.author) {
                                paint_avatar(
                                    ui.painter(),
                                    r.left_center() + vec2(22.0, 0.0),
                                    13.0,
                                    m,
                                    t.bg,
                                );
                            }
                            // a comment on an archived task: say the task is archived
                            let archived = match it.src {
                                super::nav::InboxSrc::Task(id) => {
                                    self.task(id).is_some_and(|x| !self.data.task_live(x))
                                }
                                _ => false,
                            };
                            // the title stops where the line under it does, before the
                            // time and the buttons on the right
                            let text_clip =
                                Rect::from_min_max(r.min, pos2(r.right() - 190.0, r.bottom()));
                            ui.painter().with_clip_rect(text_clip).text(
                                r.left_top() + vec2(48.0, 18.0),
                                Align2::LEFT_CENTER,
                                if archived {
                                    trf!("{} (archived)", tname)
                                } else {
                                    tname.clone()
                                },
                                FontId::proportional(14.0),
                                if archived { t.text2 } else { t.text },
                            );
                            let line = self
                                .inbox_line(it.author, &it.text, it.comment, it.src, it.mention);
                            ui.painter()
                                .with_clip_rect(text_clip)
                                .text(
                                    r.left_top() + vec2(48.0, 40.0),
                                    Align2::LEFT_CENTER,
                                    line,
                                    FontId::proportional(13.0),
                                    t.text2,
                                );
                            // under the pointer the row's buttons take the time's place: a
                            // long time ("Oct 17 at 4:19 PM") would show through them
                            let buttons = resp.hovered() || ui.rect_contains_pointer(r);
                            if !buttons {
                                ui.painter().text(
                                    r.right_center() - vec2(12.0, 0.0),
                                    Align2::RIGHT_CENTER,
                                    fmt_ago(ts),
                                    FontId::proportional(12.0),
                                    t.text3,
                                );
                            }
                            let mut handled = false;
                            if buttons {
                                let lr = Rect::from_min_size(
                                    pos2(r.right() - 180.0, r.center().y - 13.0),
                                    vec2(56.0, 26.0),
                                );
                                let cr = Rect::from_min_size(
                                    pos2(r.right() - 120.0, r.center().y - 13.0),
                                    vec2(56.0, 26.0),
                                );
                                if self.inbox_tab != 2
                                    && pressed_once(
                                        &put_over(
                                            ui,
                                            lr,
                                            egui::Button::new(tr("Later")).corner_radius(6),
                                        ),
                                        &mut handled,
                                    )
                                {
                                    // a cleared row moves to Later, as it does on the iPhone
                                    let x = &mut self.data.extra;
                                    x.inbox_cleared.retain(|x| x != &key);
                                    if !x.inbox_later.contains(&key) {
                                        x.inbox_later.push(key.clone());
                                    }
                                    self.dirty = true;
                                    handled = true;
                                }
                                // Later items can go back to the inbox (imported history
                                // would go back to Cleared: only Clear for it)
                                if self.inbox_tab == 2
                                    && !it.imported
                                    && pressed_once(
                                        &put_over(
                                            ui,
                                            lr,
                                            egui::Button::new(tr("Primary")).corner_radius(6),
                                        )
                                        .on_hover_text(tr("Back to the inbox")),
                                        &mut handled,
                                    )
                                {
                                    self.data.extra.inbox_later.retain(|x| x != &key);
                                    self.dirty = true;
                                    handled = true;
                                }
                                let ctext = if self.inbox_tab == 3 {
                                    "Restore"
                                } else {
                                    "Clear"
                                };
                                // imported history stays cleared unless saved for later:
                                // Restore would do nothing
                                let can_restore = !it.imported;
                                if (self.inbox_tab != 3 || can_restore)
                                    && pressed_once(
                                        &put_over(
                                            ui,
                                            cr,
                                            egui::Button::new(tr(ctext)).corner_radius(6),
                                        ),
                                        &mut handled,
                                    )
                                {
                                    if self.inbox_tab == 3 {
                                        // back where it belongs, as on the iPhone: also out
                                        // of Later, where an older version could leave it
                                        self.data.extra.inbox_cleared.retain(|x| x != &key);
                                        self.data.extra.inbox_later.retain(|x| x != &key);
                                    } else {
                                        self.data.extra.inbox_later.retain(|x| x != &key);
                                        self.data.extra.inbox_cleared.push(key.clone());
                                    }
                                    self.dirty = true;
                                    handled = true;
                                }
                            }
                            if !handled && row_press(&resp) {
                                self.open_inbox_src(it.src);
                            }
                        }
                    });
            });
    }
}

/// A row button pressed once (see `row_press`). Any click on a button
/// keeps the row itself from opening.
fn pressed_once(r: &egui::Response, handled: &mut bool) -> bool {
    if !r.clicked() {
        return false;
    }
    *handled = true;
    row_press(r)
}

/// A click on a row or one of its buttons that acts, not the second (or
/// third) click of a quick double click on the rows: the first click moves
/// its row away and the next row comes up under the pointer, so the second
/// would land on that row. A quick click after a click elsewhere (a tab, the
/// filter) still acts.
fn row_press(r: &egui::Response) -> bool {
    if !r.clicked() {
        return false;
    }
    let id = egui::Id::new("inbox_row_press");
    let now = r.ctx.input(|i| i.time);
    let last: Option<f64> = r.ctx.data(|d| d.get_temp(id));
    let again = (r.double_clicked() || r.triple_clicked()) && last.is_some_and(|t| now - t < 0.6);
    if !again {
        r.ctx.data_mut(|d| d.insert_temp(id, now));
    }
    !again
}
