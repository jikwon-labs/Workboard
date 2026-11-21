// Schedule message: the clock next to Send keeps a message to post later.
// On a team server the server keeps it and posts it at its time, even
// while this computer is asleep (server/src/schedule.rs); its list comes
// back in `scheduled` answers. Without a server this computer keeps it in
// its own data (`Extra::scheduled`, never shared) and posts it when the
// time comes while the app is running. Scheduled messages lists them with
// Send now, Edit and Cancel.

use super::icons::icon_button;
use super::*;
use wb_net::ScheduledMsg;

/// The times the clock offers: (label, when).
pub(super) fn schedule_choices(now: i64) -> Vec<(&'static str, i64)> {
    let today = today();
    let tomorrow = local_ts(today + Duration::days(1), nine_am()).unwrap_or(now + 86_400);
    let days_to_monday = (7 - today.weekday().num_days_from_monday() as i64) % 7;
    let monday = local_ts(
        today
            + Duration::days(if days_to_monday == 0 {
                7
            } else {
                days_to_monday
            }),
        nine_am(),
    )
    .unwrap_or(now + 7 * 86_400);
    vec![
        ("In 1 hour", now + 3600),
        ("Tomorrow at 9:00 AM", tomorrow),
        ("Monday at 9:00 AM", monday),
    ]
}

impl App {
    /// My scheduled messages, soonest first: the server's list on a
    /// server, else this computer's.
    pub(super) fn scheduled(&self) -> Vec<ScheduledMsg> {
        let mut v = if self.on_server() {
            self.sync_rt.scheduled.clone().unwrap_or_default()
        } else {
            self.data.extra.scheduled.clone()
        };
        // posted meanwhile (the server's list catches up on its next answer)
        let posted: std::collections::HashSet<u64> = self
            .data
            .channels
            .iter()
            .flat_map(|c| c.messages.iter())
            .flat_map(|m| std::iter::once(m.id).chain(m.replies.iter().map(|r| r.id)))
            .collect();
        v.retain(|i| !posted.contains(&i.id));
        v.sort_by_key(|i| (i.at, i.id));
        v
    }

    /// Keep `text` (for channel `ch`, thread `parent`) to post at `at`.
    /// False (with a word why) when it can't be kept now.
    pub(super) fn schedule_message(
        &mut self,
        ch: u64,
        parent: Option<u64>,
        text: &str,
        picked: &[u64],
        at: i64,
    ) -> bool {
        let text = text.trim();
        if text.is_empty() {
            return false;
        }
        let item = ScheduledMsg {
            id: self.data.new_id(),
            channel: ch,
            parent,
            text: text.to_string(),
            at,
            mentions: self.chat_mentions(text, ch, picked),
        };
        if self.on_server() {
            let connected = self.sync_rt.net_status == Some(wb_net::Status::Connected);
            let Some(net) = self.sync_rt.net.as_ref().filter(|_| connected) else {
                self.toast(tr("Connect to the team server to schedule a message."));
                return false;
            };
            net.send(wb_net::Msg::Schedule { item: item.clone() });
            // shown now; the server's answer replaces the list
            let list = self.sync_rt.scheduled.get_or_insert_with(Vec::new);
            list.retain(|i| i.id != item.id);
            list.push(item);
        } else {
            self.data.extra.scheduled.push(item);
            self.dirty = true;
        }
        let when = local(at)
            .map(|x| x.text("%b %-d, %-I:%M %p"))
            .unwrap_or_default();
        self.toast(trf!("Scheduled for {}", when));
        true
    }

    /// Take a scheduled message back.
    pub(super) fn unschedule(&mut self, id: u64) {
        if self.on_server() {
            if let Some(net) = &self.sync_rt.net {
                net.send(wb_net::Msg::Unschedule { id });
            }
            if let Some(l) = self.sync_rt.scheduled.as_mut() {
                l.retain(|i| i.id != id);
            }
        } else {
            self.data.extra.scheduled.retain(|i| i.id != id);
            self.dirty = true;
        }
    }

    /// Post a scheduled message now, as written.
    pub(super) fn post_scheduled(&mut self, item: &ScheduledMsg) {
        let m = Message {
            id: item.id,
            author: self.data.me,
            text: item.text.clone(),
            ts: now_ts(),
            mentions: item.mentions.clone(),
            ..serde_json::from_value(serde_json::json!({"id": 0, "author": 0, "text": "", "ts": 0}))
                .expect("an empty message")
        };
        let Some(c) = self.data.channels.iter_mut().find(|c| c.id == item.channel) else {
            return;
        };
        match item.parent {
            Some(p) => {
                if let Some(pm) = c.messages.iter_mut().find(|x| x.id == p) {
                    pm.replies.push(m);
                }
            }
            None => c.messages.push(m),
        }
        self.dirty = true;
        self.fetch_preview(item.channel, item.id, &item.text);
    }

    /// Without a server: post this computer's scheduled messages whose
    /// time has come.
    pub(super) fn scheduled_tick(&mut self) {
        if self.on_server() || self.data.extra.scheduled.is_empty() {
            return;
        }
        let now = now_ts();
        let (due, keep): (Vec<ScheduledMsg>, Vec<ScheduledMsg>) =
            std::mem::take(&mut self.data.extra.scheduled)
                .into_iter()
                .partition(|i| i.at <= now);
        self.data.extra.scheduled = keep;
        for item in &due {
            self.post_scheduled(item);
        }
        if !due.is_empty() {
            self.dirty = true;
        }
    }

    /// The Scheduled messages window. True when it should close.
    pub(super) fn scheduled_body(&mut self, ui: &mut Ui) -> bool {
        let t = self.t();
        let items = self.scheduled();
        if items.is_empty() {
            ui.label(
                RichText::new(tr(
                    "Messages you schedule show up here until they are sent.",
                ))
                .color(t.text3),
            );
            return false;
        }
        let mut act: Option<(u8, ScheduledMsg)> = None;
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .show(ui, |ui| {
                for item in &items {
                    let place = self
                        .data
                        .channels
                        .iter()
                        .find(|c| c.id == item.channel)
                        .map(|c| match c.kind {
                            ChannelKind::Channel => format!("#{}", c.name),
                            ChannelKind::Direct => self.channel_label(c),
                        })
                        .unwrap_or_default();
                    let when = local(item.at)
                        .map(|x| x.text("%b %-d, %-I:%M %p"))
                        .unwrap_or_default();
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new(format!("{} \u{00B7} {}", when, place))
                                    .small()
                                    .color(t.text2),
                            );
                            ui.label(truncate(&super::chatfmt::plain(&item.text), 120));
                            ui.horizontal(|ui| {
                                for (i, label) in
                                    ["Send now", "Edit", "Cancel"].into_iter().enumerate()
                                {
                                    let b = ui.small_button(tr(label));
                                    name_it(&b, &format!("{} {}", label, item.id));
                                    if b.clicked() {
                                        act = Some((i as u8, item.clone()));
                                    }
                                }
                            });
                        });
                    ui.separator();
                }
            });
        match act {
            Some((0, item)) => {
                self.unschedule(item.id);
                self.post_scheduled(&item);
                self.toast(tr("Sent"));
            }
            Some((1, item)) => {
                // back into its conversation's box, to change and send or schedule again
                self.unschedule(item.id);
                self.section = Section::Chat;
                self.chat.open = Some(item.channel);
                self.leave_thread();
                self.drafts
                    .chat
                    .text
                    .saved
                    .insert(item.channel, item.text.clone());
                if self.drafts.chat.text.on == Some(item.channel) {
                    self.chat.channel.text = item.text;
                }
                return true;
            }
            Some((_, item)) => self.unschedule(item.id),
            None => {}
        }
        false
    }

    /// Schedule for a time of my choosing: a date and a time. True when it
    /// should close.
    pub(super) fn schedule_at_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        date: &mut Option<NaiveDate>,
        time: &mut String,
    ) -> bool {
        let Some(ch) = self.chat.open else {
            return true;
        };
        ui.horizontal(|ui| {
            let d_txt = date.map(fmt_date).unwrap_or_else(|| tr("Date").into());
            let r = icon_button(ui, t, Icon::Calendar, &d_txt, "Schedule date", true);
            popup(ui, &r, ui.id().with("sched_date"), DATE_MENU_W, |ui| {
                date_menu(ui, t, ui.id().with("sched_cal"), date)
            });
            let r = ui.add(field(time).hint_text("09:00").desired_width(60.0));
            name_it(&r, "Schedule time");
        });
        ui.add_space(8.0);
        let at = reminder_ts(date.unwrap_or_else(today), time);
        let later = at > now_ts();
        if !later {
            ui.label(
                RichText::new(tr("Pick a time later than now."))
                    .small()
                    .color(t.text3),
            );
        }
        let mut close = false;
        let (schedule, cancel) = dialog_footer(ui, t, tr("Schedule"), later);
        if schedule {
            let text = self.chat.channel.text.clone();
            let picked = std::mem::take(&mut self.chat.channel.picked);
            if self.schedule_message(ch, None, &text, &picked, at) {
                self.chat.channel.text.clear();
            }
            close = true;
        }
        if cancel {
            close = true;
        }
        close
    }
}
