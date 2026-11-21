// Sharing an achieved goal to a conversation: the Share to team chat dialog
// and the goal's card under a message.

use super::goal_page::paint_tick;
use super::goals::*;
use super::*;

impl App {
    /// Opens Share to team chat for goal `gid`, set to the general channel.
    pub(crate) fn open_goal_share(&mut self, gid: u64) {
        let Some(g) = self.data.extra.goals.iter().find(|g| g.id == gid) else {
            return;
        };
        let channels: Vec<&Channel> = self
            .data
            .channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Channel && self.listed(c))
            .collect();
        let channel = channels
            .iter()
            .find(|c| c.name == "general")
            .or(channels.first())
            .map(|c| c.id)
            .unwrap_or(0);
        let text = trf!("{} is done. Thank you all!", g.name);
        self.modal = Some(Modal::ShareGoal {
            id: gid,
            channel,
            text,
            mention: true,
        });
    }

    /// The goal as a card for a message, as it stands now.
    pub(crate) fn goal_snapshot(&self, gid: u64) -> Option<GoalCard> {
        let g = self.data.extra.goals.iter().find(|g| g.id == gid)?;
        let row = self.goal_row(g, today());
        let mut people: Vec<u64> = vec![];
        for e in &g.log {
            if e.who != 0 && !people.contains(&e.who) {
                people.push(e.who);
            }
        }
        Some(GoalCard {
            goal: g.id,
            name: g.name.clone(),
            color: g.color,
            pct: row.pct,
            targets: g.targets.len(),
            targets_done: row.targets.iter().filter(|s| s.done).count(),
            early: g.done_on.zip(g.due).map(|(d, due)| (due - d).num_days()),
            days: g
                .done_on
                .zip(g.start)
                .map(|(d, s)| (d - s).num_days().max(0) + 1),
            people,
        })
    }

    /// The people who moved goal `gid`, other than me.
    fn goal_people(&self, gid: u64) -> Vec<u64> {
        let me = self.data.me;
        self.goal_snapshot(gid)
            .map(|c| c.people.into_iter().filter(|p| *p != me).collect())
            .unwrap_or_default()
    }

    /// Posts goal `gid`'s card with `text` in channel `dest`, @mentioning
    /// the people who moved it when `mention`.
    pub(crate) fn share_goal(&mut self, dest: u64, gid: u64, text: &str, mention: bool) {
        let Some(card) = self.goal_snapshot(gid) else {
            return;
        };
        let mut text = text.trim().to_string();
        let picked = if mention {
            self.goal_people(gid)
        } else {
            vec![]
        };
        for p in &picked {
            if let Some(m) = self.data.member(*p) {
                let at = format!("@{}", m.shown());
                if !text.contains(&at) {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&at);
                }
            }
        }
        let mentions = self.chat_mentions(&text, dest, &picked);
        let m = Message {
            id: self.data.new_id(),
            author: self.data.me,
            text,
            ts: now_ts(),
            mentions,
            goal: Some(card),
            ..serde_json::from_value(serde_json::json!({"id": 0, "author": 0, "text": "", "ts": 0}))
                .expect("an empty message")
        };
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == dest) {
            c.messages.push(m);
            c.last_read = c.last_read.max(now_ts());
            self.dirty = true;
        }
    }

    /// Share to team chat. Returns true when it should close.
    pub(crate) fn share_goal_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        gid: u64,
        channel: &mut u64,
        text: &mut String,
        mention: &mut bool,
    ) -> bool {
        let mut close = false;
        let Some(card) = self.goal_snapshot(gid) else {
            return true;
        };
        let places: Vec<(u64, String)> = self
            .data
            .channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Channel && self.listed(c))
            .map(|c| (c.id, format!("# {}", c.name)))
            .collect();
        ui.label(
            RichText::new(tr("Send to"))
                .size(12.5)
                .strong()
                .color(t.text2),
        );
        let cur = places
            .iter()
            .find(|p| p.0 == *channel)
            .map(|p| p.1.clone())
            .unwrap_or_default();
        // as wide as the message field below it
        let pick = egui::ComboBox::from_id_salt("share goal channel")
            .selected_text(cur)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (id, name) in &places {
                    let r = ui.selectable_value(channel, *id, name);
                    name_it(&r, &format!("Share in {}", name));
                }
            });
        name_it(&pick.response, "Share to channel");
        ui.add_space(10.0);
        ui.label(
            RichText::new(tr("Message"))
                .size(12.5)
                .strong()
                .color(t.text2),
        );
        let r = ui.add(
            egui::TextEdit::multiline(text)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        name_field(&r, "Share message");
        let people = self.goal_people(gid);
        if !people.is_empty() {
            ui.add_space(6.0);
            let names: Vec<String> = people
                .iter()
                .filter_map(|p| self.data.member(*p).map(|m| m.shown().to_string()))
                .collect();
            let r = check(
                ui,
                mention,
                trf!("Mention the people involved ({})", names.join(", ")),
            );
            name_it(&r, "Mention the people involved");
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new(tr("The card sent with it"))
                .size(12.5)
                .strong()
                .color(t.text2),
        );
        ui.add_space(4.0);
        paint_goal_card(ui, t, &card, &self.data, false);
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ok = *channel != 0;
                let send = ui
                    .add_enabled_ui(ok, |ui| primary_button(ui, t, tr("Send")))
                    .inner;
                name_it(&send, "Send goal to chat");
                if send.clicked() && ok {
                    self.share_goal(*channel, gid, text, *mention);
                    let to = places
                        .iter()
                        .find(|p| p.0 == *channel)
                        .map(|p| p.1.clone())
                        .unwrap_or_default();
                    self.toast(&trf!("Shared in {}", to));
                    close = true;
                }
                let cancel = secondary_button(ui, tr("Cancel"));
                name_it(&cancel, "Cancel share");
                if cancel.clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// A goal's card under a message. Returns the goal to open when its
    /// "Open goal" is clicked and the goal still exists.
    pub(crate) fn goal_chat_card(&self, ui: &mut Ui, card: &GoalCard) -> Option<u64> {
        let t = self.t();
        let exists = self.data.extra.goals.iter().any(|g| g.id == card.goal);
        paint_goal_card(ui, &t, card, &self.data, exists).then_some(card.goal)
    }
}

/// The card: a medal (a tick at 100%, else the percent on a ring), the goal's
/// name, what it took, who moved it and, when `link`, "Open goal". Returns
/// true when "Open goal" is clicked.
fn paint_goal_card(ui: &mut Ui, t: &Theme, card: &GoalCard, data: &Data, link: bool) -> bool {
    let mut open = false;
    let color = rgb(card.color);
    let done = card.pct >= 100;
    let w = ui.available_width().min(460.0);
    let tint = if t.dark {
        color.gamma_multiply(0.30)
    } else {
        color.gamma_multiply(0.35)
    };
    egui::Frame::new()
        .fill(t.card)
        .stroke(Stroke::new(1.0, tint))
        .corner_radius(14)
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_width(w - 32.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                let p = ui.painter();
                if done {
                    p.circle_filled(r.center(), 20.0, color);
                    paint_tick(p, r.center(), 8.0, Color32::WHITE, 2.8);
                } else {
                    paint_ring(
                        p,
                        r.center(),
                        16.0,
                        5.0,
                        card.pct as f32 / 100.0,
                        color,
                        t.hover,
                    );
                    p.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        card.pct.to_string(),
                        FontId::proportional(11.0),
                        t.text,
                    );
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let label = if done {
                        tr("Goal achieved")
                    } else {
                        tr("Goal update")
                    };
                    ui.label(
                        RichText::new(label)
                            .size(12.0)
                            .strong()
                            .color(pace_colors(if done { Pace::OnTrack } else { Pace::Done }, t).1),
                    );
                    ui.add(
                        egui::Label::new(RichText::new(&card.name).size(16.0).strong()).truncate(),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{}%", card.pct)).size(18.0).strong());
                });
            });
            ui.add_space(10.0);
            let mut facts: Vec<(String, String)> = vec![(
                format!("{}/{}", card.targets_done, card.targets),
                tr("Targets").into(),
            )];
            if let Some(e) = card.early {
                let (v, l) = match e.cmp(&0) {
                    std::cmp::Ordering::Greater => (trf!("{} days", e), tr("Before the deadline")),
                    std::cmp::Ordering::Equal => (tr("On the day").to_string(), tr("Deadline")),
                    std::cmp::Ordering::Less => (trf!("{} days", -e), tr("After the deadline")),
                };
                facts.push((v, l.into()));
            }
            if let Some(d) = card.days {
                facts.push((trf!("{} days", d), tr("Time taken").into()));
            }
            egui::Frame::new()
                .fill(t.hover.gamma_multiply(0.6))
                .corner_radius(10)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.columns(facts.len(), |cols| {
                        for (c, (v, l)) in cols.iter_mut().zip(&facts) {
                            c.label(RichText::new(v).size(14.0).strong());
                            c.label(RichText::new(l).size(11.5).color(t.text2));
                        }
                    });
                });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if !card.people.is_empty() {
                    let ids: Vec<u64> = card.people.iter().copied().take(5).collect();
                    let aw = avatars_width(11.0, &ids, data).max(22.0);
                    let (r, _) = ui.allocate_exact_size(vec2(aw, 24.0), Sense::hover());
                    paint_avatars(
                        ui.painter(),
                        pos2(r.left(), r.center().y),
                        11.0,
                        &ids,
                        data,
                        t.card,
                    );
                    let names: Vec<String> = ids
                        .iter()
                        .filter_map(|p| data.member(*p).map(|m| m.shown().to_string()))
                        .collect();
                    ui.add(
                        egui::Label::new(RichText::new(names.join(", ")).size(12.5).color(t.text2))
                            .truncate(),
                    );
                }
                if link {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // a frameless button: the soft fill shows it under the pointer
                        let b = ui.add(crate::widgets::ghost(egui::Button::new(
                            RichText::new(tr("Open goal"))
                                .size(13.0)
                                .strong()
                                .color(pace_colors(Pace::Done, t).1),
                        )));
                        name_it(&b, &format!("Open goal {}", card.name));
                        open = b.clicked();
                    });
                }
            });
        });
    open
}
