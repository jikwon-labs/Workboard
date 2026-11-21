// Dialogs for single items and odd jobs: CSV import, channel settings,
// tracking time, reminders, trash, bookmarks and new dashboards.

use super::*;

impl App {
    /// The Import dialog. Returns true when it should close.
    pub(super) fn import_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        path: &mut String,
        result: &mut String,
    ) -> bool {
        ui.label(tr("Import tasks from a CSV file (columns: name, status, priority, due date YYYY-MM-DD, assignee name). They go into the current list."));
        ui.add(
            field(path)
                .hint_text(tr("/Users/you/Downloads/tasks.csv"))
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if primary_button(ui, t, tr("Import")).clicked() {
                *result = self.import_csv(path.trim());
            }
            if ui.button(tr("Export current tasks to CSV")).clicked() {
                *result = self.export_csv();
            }
        });
        ui.add_space(6.0);
        // it brings people in: the owner's and admins' on a server
        if self.may_manage() && ui.button(tr("Import from Slack")).clicked() {
            self.start_slack_import();
        }
        if !result.is_empty() {
            ui.label(RichText::new(result.clone()).color(t.text2));
        }
        false
    }

    /// The ChannelSettings dialog. Returns true when it should close.
    pub(super) fn channel_settings_body(&mut self, ui: &mut Ui, t: &Theme, cid: &mut u64) -> bool {
        let mut close = false;
        let cid = *cid;
        let Some(ci) = self.data.channels.iter().position(|c| c.id == cid) else {
            return true;
        };
        let is_channel = self.data.channels[ci].kind == ChannelKind::Channel;
        if is_channel {
            ui.label(tr("Name"));
            if ui
                .add(field(&mut self.data.channels[ci].name).desired_width(f32::INFINITY))
                .changed()
            {
                self.dirty = true;
            }
            // a gap before each name, so it reads with the field below it
            ui.add_space(6.0);
            ui.label(tr("Topic"));
            if ui
                .add(field(&mut self.data.channels[ci].topic).desired_width(f32::INFINITY))
                .changed()
            {
                self.dirty = true;
            }
            ui.add_space(6.0);
            ui.label(tr("Members"));
            let members = self.data.members.clone();
            // anyone may join or leave; only who may delete it changes others
            let manager = self.may_manage_channel(&self.data.channels[ci]);
            let me = self.data.me;
            ui.horizontal_wrapped(|ui| {
                for m in &members {
                    let mut on = self.data.channels[ci].members.contains(&m.id);
                    let allowed = manager || m.id == me;
                    // in the channel once they join, like #general
                    let label = if m.invited {
                        crate::trf!("{} (invited)", m.shown())
                    } else {
                        m.shown().to_string()
                    };
                    let b = ui.add_enabled(allowed, Check::new(&mut on, label));
                    let b = if allowed { b } else { b.on_disabled_hover_text(tr("Only whoever made this channel, or an admin, can change who else is in it.")) };
                    if b.changed() && allowed {
                        let c = &mut self.data.channels[ci];
                        if on {
                            c.members.push(m.id);
                        } else {
                            c.members.retain(|x| *x != m.id);
                        }
                        self.dirty = true;
                    }
                }
            });
        }
        // Workflows and Integrations open windows of their own: full-width
        // rows that say what each is for. Buttons ending in "..." read as
        // cut-off text in Korean.
        if is_channel {
            ui.add_space(10.0);
            let mut rows = vec![(
                "Workflows",
                "Run steps automatically when a message is posted",
            )];
            // webhooks and slash commands: owners and admins on a server
            if self.on_server() && self.i_manage() {
                rows.push(("Integrations", "Webhooks and slash commands"));
            }
            match link_rows(ui, t, &rows) {
                Some(0) => {
                    self.modal = Some(Modal::Workflows {
                        form: Box::new(super::workflows::WorkflowForm {
                            channel: Some(cid),
                            ..Default::default()
                        }),
                    });
                    return false;
                }
                Some(_) => {
                    if let Some(net) = &self.sync_rt.net {
                        net.send(wb_net::Msg::Integrations);
                    }
                    self.modal = Some(Modal::Integrations {
                        channel: cid,
                        form: Default::default(),
                    });
                    return false;
                }
                None => {}
            }
        }
        ui.add_space(10.0);
        if ui.button(tr("Mark as read")).clicked() {
            self.mark_read(cid);
            self.dirty = true;
        }
        // what removes it sits apart, under a line, in red
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        if !is_channel {
            // only hides it for me; it comes back with the next message
            if ui
                .add(egui::Button::new(
                    RichText::new(tr("Close conversation")).color(t.danger),
                ))
                .on_hover_text(tr(
                    "Hides it from your sidebar until a new message arrives. Nothing is deleted.",
                ))
                .clicked()
            {
                self.close_dm(cid);
                close = true;
            }
            return close;
        }
        // deleting removes it for everyone: only whoever made it (any
        // member for channels from before that was kept) or an admin,
        // asked first, and it goes to my Trash
        let member = self.may_manage_channel(&self.data.channels[ci]);
        let ask = egui::Id::new(("delete_channel", cid));
        let asking = ui.data(|d| d.get_temp::<bool>(ask).unwrap_or(false));
        if !asking {
            let b = ui.add_enabled(
                member,
                egui::Button::new(RichText::new(tr("Delete channel")).color(t.danger)),
            );
            let b = if member {
                b
            } else {
                b.on_disabled_hover_text(tr(
                    "Only whoever made this channel, or an admin, can delete it.",
                ))
            };
            if b.clicked() {
                ui.data_mut(|d| d.insert_temp(ask, true));
            }
        }
        if ui.data(|d| d.get_temp::<bool>(ask).unwrap_or(false)) {
            let name = self.data.channels[ci].name.clone();
            ui.label(RichText::new(crate::trf!("Delete #{} for everyone? It goes to your Trash with its messages, and restoring it brings it back for the whole team.", name)).color(t.text));
            ui.horizontal(|ui| {
                if ui
                    .add(Tint(
                        egui::Button::new(RichText::new(tr("Delete")).color(Color32::WHITE))
                            .fill(t.danger),
                    ))
                    .clicked()
                    && self.may_manage_channel(&self.data.channels[ci])
                {
                    let gone = self.data.channels.remove(ci);
                    let tid = self.data.new_id();
                    self.data.extra.trash.push(TrashItem {
                        id: tid,
                        kind: "Channel".into(),
                        title: format!("#{}", gone.name),
                        deleted: now_ts(),
                        payload: serde_json::to_string(&gone).unwrap_or_default(),
                    });
                    if self.chat.open == Some(cid) {
                        self.chat.open = None;
                    }
                    self.dirty = true;
                    close = true;
                    ui.data_mut(|d| d.remove::<bool>(ask));
                }
                if ui.button(tr("Cancel")).clicked() {
                    ui.data_mut(|d| d.remove::<bool>(ask));
                }
            });
        }
        close
    }

    /// The TrackTime dialog. Returns true when it should close.
    pub(super) fn track_time_body(&mut self, ui: &mut Ui, t: &Theme, task: &mut u64) -> bool {
        let mut close = false;
        let tasks: Vec<(u64, String)> = self
            .data
            .tasks
            .iter()
            .map(|x| (x.id, x.name.clone()))
            .collect();
        let cur = tasks
            .iter()
            .find(|x| x.0 == *task)
            .map(|x| x.1.clone())
            .unwrap_or_else(|| tr("Choose a task").into());
        egui::ComboBox::from_id_salt("tt_task")
            .selected_text(cur)
            .width(380.0)
            .show_ui(ui, |ui| {
                for (id, n) in &tasks {
                    ui.selectable_value(task, *id, n);
                }
            });
        let now = now_ts();
        let running = self
            .data
            .tasks
            .iter()
            .find(|x| self.data.is_my_timer(x))
            .map(|x| (x.id, x.name.clone(), x.tracked_secs(now)));
        if let Some((_, n, secs)) = &running {
            ui.label(crate::trf!(
                "Running: {} \u{2022} {}",
                n,
                fmt_duration(*secs)
            ));
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
        ui.horizontal(|ui| {
            let is_running = running.as_ref().map(|r| r.0 == *task).unwrap_or(false);
            if primary_button(
                ui,
                t,
                if is_running {
                    tr("Stop timer")
                } else {
                    tr("Start timer")
                },
            )
            .clicked()
                && *task != 0
            {
                self.actions.push(Action::ToggleTimer(*task));
            }
            if ui.button(tr("Open Timesheets")).clicked() {
                self.section = Section::Timesheets;
                close = true;
            }
        });
        close
    }

    /// The Reminder dialog. Returns true when it should close.
    pub(super) fn reminder_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        text: &mut String,
        date: &mut Option<NaiveDate>,
        time: &mut String,
    ) -> bool {
        let mut close = false;
        let r = ui.add(
            field(text)
                .hint_text(tr("Remind me to..."))
                .desired_width(f32::INFINITY),
        );
        r.request_focus();
        ui.horizontal(|ui| {
            let d_txt = date.map(fmt_date).unwrap_or_else(|| tr("Date").into());
            let r = icon_button(ui, t, "calendar", &d_txt, "Reminder date", true);
            popup(ui, &r, ui.id().with("rem_date"), DATE_MENU_W, |ui| {
                date_menu(ui, t, ui.id().with("rem_cal"), date)
            });
            let r = ui.add(field(time).hint_text("09:00").desired_width(60.0));
            name_field(&r, "Reminder time");
        });
        if primary_button(ui, t, tr("Create Reminder")).clicked() && !text.trim().is_empty() {
            let ts = reminder_ts(date.unwrap_or_else(today), time);
            let id = self.data.new_id();
            let (task, message) = std::mem::take(&mut self.reminder_about);
            self.data.extra.reminders.push(Reminder {
                id,
                text: text.trim().to_string(),
                due_ts: ts,
                done: false,
                owner: self.data.me,
                task,
                message,
                ..Default::default()
            });
            self.dirty = true;
            self.toast(tr("Reminder created"));
            close = true;
        }
        close
    }

    /// The Trash dialog. Returns true when it should close.
    pub(super) fn trash_body(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        if self.data.extra.trash.is_empty() {
            ui.label(RichText::new(tr("Trash is empty.")).color(t.text3));
        } else {
            ui.horizontal(|ui| {
                if self.sweep.confirm.as_deref() == Some("empty_trash") {
                    ui.label(
                        RichText::new(trf!(
                            "Delete all {} items for good?",
                            self.data.extra.trash.len()
                        ))
                        .color(t.danger),
                    );
                    if ui.button(tr("Yes")).clicked() {
                        self.data.extra.trash.clear();
                        self.sweep.confirm = None;
                        self.dirty = true;
                    }
                    if ui.button(tr("No")).clicked() {
                        self.sweep.confirm = None;
                    }
                } else if ui
                    .add(egui::Button::new(
                        RichText::new(tr("Empty trash")).color(t.danger),
                    ))
                    .clicked()
                {
                    self.sweep.confirm = Some("empty_trash".into());
                }
            });
            ui.add_space(4.0);
        }
        let mut restore = None;
        let mut purge = None;
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                for (i, item) in self.data.extra.trash.iter().enumerate().rev() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr(&item.kind)).small().color(t.text3));
                        // something deleted without a name still says what it was
                        if item.title.trim().is_empty() {
                            ui.label(RichText::new(tr("Untitled")).italics().color(t.text2));
                        } else {
                            ui.label(&item.title);
                        }
                        ui.label(RichText::new(fmt_ago(item.deleted)).small().color(t.text3));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            // both framed buttons, the second in red
                            if ui
                                .button(RichText::new(tr("Delete forever")).color(t.danger))
                                .clicked()
                            {
                                purge = Some(i);
                            }
                            if ui.button(tr("Restore")).clicked() {
                                restore = Some(i);
                            }
                        });
                    });
                }
            });
        if let Some(i) = restore {
            self.restore_trash(i);
        }
        if let Some(i) = purge {
            self.data.extra.trash.remove(i);
            self.dirty = true;
        }
        false
    }

    /// The Bookmark dialog. Returns true when it should close.
    pub(super) fn bookmark_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        title: &mut String,
        url: &mut String,
    ) -> bool {
        let mut close = false;
        ui.add(
            field(title)
                .hint_text(tr("Name"))
                .desired_width(f32::INFINITY),
        );
        ui.add(
            field(url)
                .hint_text(tr("https://..."))
                .desired_width(f32::INFINITY),
        );
        if primary_button(ui, t, tr("Add bookmark")).clicked() && !url.trim().is_empty() {
            let id = self.data.new_id();
            let ttl = if title.trim().is_empty() {
                url.trim().to_string()
            } else {
                title.trim().to_string()
            };
            self.data.extra.bookmarks.push(Bookmark {
                id,
                title: ttl,
                url: url.trim().to_string(),
                personal: false,
            });
            self.dirty = true;
            close = true;
        }
        close
    }

    /// The NewDashboard dialog. Returns true when it should close.
    pub(super) fn new_dashboard_body(&mut self, ui: &mut Ui, t: &Theme, name: &mut String) -> bool {
        let mut close = false;
        ui.label(tr("Dashboard name"));
        let r = ui.add(
            field(name)
                .hint_text(tr("For example: Weekly overview"))
                .desired_width(f32::INFINITY),
        );
        name_field(&r, "Dashboard name entry");
        let spaces: Vec<(u64, String)> = self
            .data
            .spaces
            .iter()
            .map(|s| (s.id, s.name.clone()))
            .collect();
        ui.label(
            RichText::new(tr(
                "Shows the whole Workspace; pick a Space later from the Dashboards page.",
            ))
            .small()
            .color(t.text2),
        );
        let _ = spaces;
        // off until it has a name, rather than doing nothing
        let named = !name.trim().is_empty();
        let create = ui
            .add_enabled_ui(named, |ui| primary_button(ui, t, tr("Create Dashboard")))
            .inner;
        if create.clicked() && named {
            self.new_dashboard(name.trim());
            self.section = Section::Dashboards;
            close = true;
        }
        close
    }
}

/// A framed group of rows that each open another window: the name, a line
/// on what it is for, and a ">" at the right, as wide as the dialog.
/// `rows` are (name, note) in English; returns the index of the one clicked.
/// Each row is named by its English name for screen readers and tests.
fn link_rows(ui: &mut Ui, t: &Theme, rows: &[(&str, &str)]) -> Option<usize> {
    const ROW_H: f32 = 50.0;
    const R: u8 = 6;
    let w = ui.available_width();
    let mut clicked = None;
    let mut outer = Rect::NOTHING;
    for (i, (name, note)) in rows.iter().enumerate() {
        let (r, resp) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::click());
        ui.add_space(-ui.spacing().item_spacing.y);
        name_it(&resp, name);
        outer = outer.union(r);
        let first = i == 0;
        let last = i + 1 == rows.len();
        let p = ui.painter();
        if resp.hovered() {
            let corners = egui::CornerRadius {
                nw: if first { R } else { 0 },
                ne: if first { R } else { 0 },
                sw: if last { R } else { 0 },
                se: if last { R } else { 0 },
            };
            p.rect_filled(r.shrink(1.0), corners, crate::widgets::hover_fill(ui));
        }
        if !last {
            p.hline(
                r.left() + 1.0..=r.right() - 1.0,
                r.bottom(),
                Stroke::new(1.0_f32, t.border),
            );
        }
        let x = r.left() + 12.0;
        p.text(
            pos2(x, r.top() + 16.0),
            Align2::LEFT_CENTER,
            tr(name),
            FontId::proportional(14.0),
            t.text,
        );
        p.text(
            pos2(x, r.bottom() - 15.0),
            Align2::LEFT_CENTER,
            tr(note),
            FontId::proportional(12.0),
            t.text2,
        );
        line_icon(
            p,
            "chevron_right",
            pos2(r.right() - 18.0, r.center().y),
            t.text3,
        );
        if resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            clicked = Some(i);
        }
    }
    ui.add_space(ui.spacing().item_spacing.y);
    ui.painter().rect_stroke(
        outer,
        R,
        Stroke::new(1.0_f32, t.border),
        egui::StrokeKind::Inside,
    );
    clicked
}
