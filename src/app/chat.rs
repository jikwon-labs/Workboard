// Chat: Channels and Direct Messages in a list on the left (the original's
// sidebar sections "Channels" and "Direct Messages"), the message feed with
// reactions, saved messages and "Reply in thread", a thread panel on the
// right, and turning a message into a task. Everything stays on this computer.

use super::*;

const QUICK_REACTIONS: [&str; 4] = ["\u{1F44D}", "\u{2705}", "\u{1F440}", "\u{1F389}"];

pub(super) enum ChatAction {
    Open(u64),
    JoinHuddle(u64),
    React { ch: u64, msg: u64, emoji: String },
    Thread(u64),
    CloseThread,
    Save { ch: u64, msg: u64 },
    Task { ch: u64, msg: u64 },
    Send { ch: u64, text: String, parent: Option<u64>, files: Vec<Attachment> },
    NewChannel(String),
    Dm(u64),
    SendPost { ch: u64, title: String, text: String, files: Vec<Attachment> },
    Pin { ch: u64, msg: u64 },
    /// Open a message's picture or video large.
    OpenMedia { ch: u64, msg: u64, index: usize },
}

fn find_msg(msgs: &mut [Message], id: u64) -> Option<&mut Message> {
    for m in msgs.iter_mut() {
        if m.id == id {
            return Some(m);
        }
        if let Some(r) = m.replies.iter_mut().find(|r| r.id == id) {
            return Some(r);
        }
    }
    None
}

impl App {
    pub(super) fn channel_label(&self, c: &Channel) -> String {
        match c.kind {
            ChannelKind::Channel => format!("# {}", c.name),
            ChannelKind::Direct => c
                .members
                .iter()
                .find(|m| **m != self.data.me)
                .and_then(|m| self.data.member(*m))
                .map(|m| m.name.clone())
                .unwrap_or_else(|| "Just you".into()),
        }
    }

    fn unread(&self, c: &Channel) -> usize {
        c.messages.iter().filter(|m| m.ts > c.last_read && m.author != self.data.me).count()
    }

    fn mention_text(&self, text: &str, size: f32) -> egui::text::LayoutJob {
        let t = self.t();
        let mut job = egui::text::LayoutJob::default();
        for (i, word) in text.split(' ').enumerate() {
            if i > 0 {
                job.append(" ", 0.0, egui::TextFormat { font_id: FontId::proportional(size), color: t.text, ..Default::default() });
            }
            let mention = word.starts_with('@') && word.len() > 1;
            let fmt = egui::TextFormat {
                font_id: FontId::proportional(size),
                color: if mention { t.selected_text } else { t.text },
                background: if mention { t.selected_bg } else { Color32::TRANSPARENT },
                ..Default::default()
            };
            job.append(word, 0.0, fmt);
        }
        job.wrap.max_width = f32::INFINITY;
        job
    }

    pub(super) fn chat(&mut self, ui: &mut Ui) {
        let t = self.t();
        if self.chat_open.is_none() {
            self.chat_open = self.data.channels.first().map(|c| c.id);
        }
        let mut acts: Vec<ChatAction> = vec![];
        let Some(cid) = self.chat_open else {
            // empty state: no channel or DM yet
            let mut make = false;
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.28).max(40.0));
                let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                ui.painter().rect_filled(r, 14.0, t.accent_bg);
                super::shell::line_icon(ui.painter(), "chat", r.center(), t.accent_text);
                ui.add_space(14.0);
                ui.label(RichText::new("Chat with your team").size(22.0).color(t.text));
                ui.add_space(4.0);
                ui.label(RichText::new("Channels keep conversations about a topic in one place. Direct messages are for one-on-one talks and calls.").color(t.text2));
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    let w = 170.0 * 2.0 + 150.0 + 16.0;
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    let b = ui.add(egui::Button::new(RichText::new("#  Create a Channel").color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(170.0, 34.0)));
                    name_it(&b, "Create a Channel");
                    if b.clicked() {
                        make = true;
                    }
                    if ui.add(egui::Button::new("\u{1F464}  Message someone").stroke(Stroke::new(1.0, t.border)).corner_radius(7).min_size(vec2(170.0, 34.0))).clicked() {
                        self.new_message_open = true;
                    }
                    if ui.add(egui::Button::new("\u{1F465}  Work together").stroke(Stroke::new(1.0, t.border)).corner_radius(7).min_size(vec2(150.0, 34.0))).on_hover_text("Share this workspace so others can chat with you").clicked() {
                        self.modal = Some(self.together_modal());
                    }
                });
            });
            if make {
                self.apply_chat(vec![ChatAction::NewChannel("general".into())]);
            }
            return;
        };
        let Some(channel) = self.data.channels.iter().find(|c| c.id == cid).cloned() else {
            self.chat_open = None;
            return;
        };

        // ---- thread panel
        let mut thread_rect: Option<Rect> = None;
        if let Some(tid) = self.thread_open {
            if let Some(parent) = channel.messages.iter().find(|m| m.id == tid).cloned() {
                let panel = egui::SidePanel::right("thread")
                    .exact_width(360.0)
                    .resizable(false)
                    .frame(egui::Frame::new().fill(t.column_bg).inner_margin(egui::Margin::symmetric(14, 10)))
                    .show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Thread").size(15.0).strong());
                            ui.label(RichText::new(self.channel_label(&channel)).color(t.text3));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(egui::Button::new("\u{2716}").frame(false)).clicked() {
                                    acts.push(ChatAction::CloseThread);
                                }
                            });
                        });
                        ui.separator();
                        // one row of tiles when pictures wait in the box
                        let composer_h = 96.0 + if self.thread_files.is_empty() { 0.0 } else { 76.0 };
                        egui::ScrollArea::vertical().id_salt("thread_scroll").max_height(ui.available_height() - composer_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                            self.message_row(ui, cid, &parent, false, &mut acts);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(if parent.replies.len() == 1 { "1 reply".to_string() } else { format!("{} replies", parent.replies.len()) }).small().color(t.text3));
                                let r = ui.available_rect_before_wrap();
                                ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.border));
                            });
                            for r in &parent.replies {
                                self.message_row(ui, cid, r, false, &mut acts);
                            }
                        });
                        if let Some((text, files)) = self.composer(ui, "Reply...", true) {
                            acts.push(ChatAction::Send { ch: cid, text, parent: Some(tid), files });
                        }
                    });
                thread_rect = Some(panel.response.rect);
            }
        }
        // pictures and videos dragged in from Finder
        self.take_dropped(ui.ctx(), thread_rect);
        let dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());

        // ---- conversation (header with tabs, side panel, messages, composer)
        let ws_name = self.data.workspace.clone();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 12, top: 8, bottom: 0 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(30.0);
                    let title = match channel.kind {
                        ChannelKind::Channel if channel.name == "general" => ws_name.clone(),
                        ChannelKind::Channel if channel.private => format!("\u{1F512} {}", channel.name),
                        ChannelKind::Channel => format!("# {}", channel.name),
                        ChannelKind::Direct => self.channel_label(&channel),
                    };
                    if channel.kind == ChannelKind::Direct {
                        if let Some(m) = channel.members.iter().find(|m| **m != self.data.me).or(channel.members.first()).and_then(|m| self.data.member(*m)) {
                            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                            paint_avatar(ui.painter(), r.center(), 10.0, m, t.bg);
                        }
                    }
                    ui.label(RichText::new(title).size(15.5).color(t.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let hb = |ui: &mut Ui, text: &str| ui.add(egui::Button::new(RichText::new(text).size(14.0).color(t.text2)).frame(false));
                        if hb(ui, "\u{1F4DD}  Ask").clicked() {
                            self.section = Section::Ai;
                        }
                        if hb(ui, "\u{2699}  Integrate").clicked() {
                            self.modal = Some(super::menus::unavailable("Integrations"));
                        }
                        if hb(ui, "\u{260E}").on_hover_text("Start a SyncUp").clicked() {
                            self.open_call_dialog();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let g = ui.painter().layout_no_wrap("Channel".into(), FontId::proportional(14.0), t.text);
                    let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 34.0, 34.0), Sense::hover());
                    let ic = Rect::from_center_size(pos2(r.left() + 13.0, r.center().y), vec2(15.0, 15.0));
                    ui.painter().rect_filled(ic, 3.0, Color32::from_rgb(0x5f, 0x55, 0xee));
                    ui.painter().text(ic.center(), Align2::CENTER_CENTER, "#", FontId::proportional(10.0), Color32::WHITE);
                    ui.painter().galley(pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0), g.clone(), t.text);
                    ui.painter().galley(pos2(r.left() + 26.3, r.center().y - g.size().y / 2.0), g, t.text);
                    ui.painter().rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 2.0), r.max), 1.0, t.text);
                    for (label, v) in [("List", ViewKind::List), ("Board", ViewKind::Board), ("Calendar", ViewKind::Calendar)] {
                        if ui.add(egui::Button::new(RichText::new(label).size(14.0).color(t.text2)).frame(false).min_size(vec2(0.0, 30.0))).clicked() {
                            self.view = v;
                            // a channel made with "Add a List" opens its own List
                            let to = channel.list.filter(|l| self.data.location_exists(Location::List(*l))).map(Location::List).unwrap_or(Location::Everything);
                            self.actions.push(Action::Go(to));
                        }
                    }
                    let vr = ui.add(egui::Button::new(RichText::new("+  View").size(14.0).color(t.text2)).frame(false).min_size(vec2(0.0, 30.0)));
                    popup(ui, &vr, ui.id().with(("chan_view", cid)), 180.0, |ui| {
                        for (label, v) in [("List", ViewKind::List), ("Board", ViewKind::Board), ("Calendar", ViewKind::Calendar), ("Gantt", ViewKind::Gantt), ("Table", ViewKind::Table)] {
                            if ui.button(label).clicked() {
                                self.view = v;
                                self.actions.push(Action::Go(Location::Everything));
                                close_popup(ui);
                            }
                        }
                    });
                });
            });
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
            // right panel
            egui::SidePanel::right(egui::Id::new(("chan_panel", cid))).exact_width(230.0).resizable(false).show_separator_line(false).frame(egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 12, top: 12, bottom: 8 })).show_inside(ui, |ui| {
                egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let rows: [(&str, &str, u8); 6] = [("\u{1F464}", "Followers", 10), ("\u{1F50D}", "Find in Channel", 11), ("\u{21A9}", "Replies", 2), ("\u{1F4CC}", "Pinned Messages", 1), ("\u{1F464}", "Assigned", 3), ("\u{2699}", "Settings", 12)];
                    for (icon, label, code) in rows {
                        let w = ui.available_width();
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                        let sel = code < 10 && self.channel_panel == code;
                        if sel {
                            ui.painter().rect_filled(r, 6.0, t.row_selected);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, t.hover);
                        }
                        if code == 10 {
                            paint_avatars(ui.painter(), r.left_center() + vec2(4.0, 0.0), 8.0, &channel.members.iter().take(1).copied().collect::<Vec<_>>(), &self.data, t.bg);
                        } else {
                            ui.painter().text(r.left_center() + vec2(12.0, 0.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
                        }
                        ui.painter().text(r.left_center() + vec2(28.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), t.text);
                        if code == 10 {
                            ui.painter().text(r.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, channel.members.len().to_string(), FontId::proportional(12.0), t.text3);
                        }
                        if resp.clicked() {
                            match code {
                                10 | 12 => self.modal = Some(Modal::ChannelSettings(cid)),
                                11 => self.channel_find = if self.channel_find.is_some() { None } else { Some(String::new()) },
                                c => self.channel_panel = if self.channel_panel == c { 0 } else { c },
                            }
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                if ui.add(egui::Button::new(RichText::new("Add bookmarks, notes, and more").size(13.5).color(t.text2)).corner_radius(14)).clicked() {
                    self.modal = Some(Modal::Bookmark { title: String::new(), url: String::new() });
                }
                if let Some(mut q) = self.channel_find.take() {
                    ui.add(egui::TextEdit::singleline(&mut q).hint_text("\u{1F50D} Find in channel").desired_width(200.0));
                    self.channel_find = Some(q);
                }
                if self.channel_panel != 0 {
                    let label = match self.channel_panel {
                        1 => "Pinned messages",
                        2 => "Messages with replies",
                        _ => "Messages turned into tasks",
                    };
                    if ui.add(egui::Button::new(format!("{}  \u{2716}", label)).corner_radius(14)).clicked() {
                        self.channel_panel = 0;
                    }
                }
            });
            let composer_h = 140.0 + if self.chat_files.is_empty() { 0.0 } else { 76.0 };
            let q = self.channel_find.clone().unwrap_or_default().to_lowercase();
            let panel = self.channel_panel;
            let shown: Vec<Message> = channel
                .messages
                .iter()
                .filter(|m| q.is_empty() || m.text.to_lowercase().contains(&q) || m.post_title.as_ref().map(|t| t.to_lowercase().contains(&q)).unwrap_or(false))
                .filter(|m| match panel {
                    1 => m.pinned,
                    2 => !m.replies.is_empty(),
                    3 => m.task.is_some(),
                    _ => true,
                })
                .cloned()
                .collect();
            egui::ScrollArea::vertical().id_salt(("chat_scroll", cid)).max_height(ui.available_height() - composer_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 10)).show(ui, |ui| {
                    if channel.messages.is_empty() {
                        // empty state, as in the original
                        ui.add_space(50.0);
                        let w = ui.available_width();
                        let cw = 400.0f32.min(w - 40.0);
                        let pad = ((w - cw) / 2.0).max(0.0);
                        let name = if channel.kind == ChannelKind::Channel { format!("#{}", if channel.name == "general" { "General".to_string() } else { channel.name.clone() }) } else { self.channel_label(&channel) };
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_width(cw);
                                ui.label(RichText::new(format!("Chat in {}", name)).size(16.0).color(t.text));
                                ui.label(RichText::new("Collaborate seamlessly across tasks and conversations. Start chatting with your team or connect tasks to stay on top of your work.").color(t.text2));
                                ui.add_space(10.0);
                                if ui.add(egui::Button::new("+  Add People").min_size(vec2(cw, 32.0))).clicked() {
                                    self.modal = Some(Modal::ChannelSettings(cid));
                                }
                                if ui.add(egui::Button::new("\u{2728}  Import from Slack").min_size(vec2(cw, 32.0))).clicked() {
                                    self.modal = Some(super::menus::unavailable("Import from Slack"));
                                }
                                ui.add_space(8.0);
                                ui.separator();
                                for (bg, icon, title, sub) in [([0xb6, 0x60, 0xe0], "\u{2630}", "Track Tasks", "Manage tasks, bugs, people, and more"), ([0x10, 0x90, 0xe0], "\u{1F4C4}", "Add Doc", "Take notes or create detailed documents"), ([0x00, 0x88, 0x44], "\u{260E}", "Start SyncUp", "Jump on a voice call or video call")] {
                                    let (r, resp) = ui.allocate_exact_size(vec2(cw, 54.0), Sense::click());
                                    ui.painter().rect_filled(r, 8.0, rgb(bg).gamma_multiply(if resp.hovered() { 0.14 } else { 0.07 }));
                                    let ib = Rect::from_center_size(r.left_center() + vec2(26.0, 0.0), vec2(34.0, 34.0));
                                    ui.painter().rect_filled(ib, 8.0, Color32::WHITE);
                                    ui.painter().text(ib.center(), Align2::CENTER_CENTER, icon, FontId::proportional(15.0), rgb(bg));
                                    ui.painter().text(r.left_center() + vec2(54.0, -9.0), Align2::LEFT_CENTER, title, FontId::proportional(14.5), t.text);
                                    ui.painter().text(r.left_center() + vec2(54.0, 10.0), Align2::LEFT_CENTER, sub, FontId::proportional(12.5), t.text2);
                                    if resp.clicked() {
                                        match title {
                                            "Track Tasks" => self.actions.push(Action::Go(Location::Everything)),
                                            "Add Doc" => self.new_doc(None),
                                            _ => self.open_call_dialog(),
                                        }
                                    }
                                    ui.add_space(6.0);
                                }
                            });
                        });
                    }
                    let mut last_day: Option<NaiveDate> = None;
                    let mut prev: Option<(u64, i64)> = None;
                    for m in &shown {
                        let d = chrono::DateTime::from_timestamp(m.ts, 0).map(|x| x.with_timezone(&chrono::Local).date_naive()).unwrap_or_else(today);
                        if last_day != Some(d) {
                            last_day = Some(d);
                            ui.add_space(6.0);
                            let w = ui.available_width();
                            let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
                            ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.border));
                            let label = match (d - today()).num_days() {
                                0 => "Today".to_string(),
                                -1 => "Yesterday".to_string(),
                                _ => d.format("%A, %B %-d").to_string(),
                            };
                            let g = ui.painter().layout_no_wrap(label, FontId::proportional(11.5), t.text2);
                            let pill = Rect::from_center_size(r.center(), g.size() + vec2(20.0, 8.0));
                            ui.painter().rect_filled(pill, 11.0, t.bg);
                            ui.painter().rect_stroke(pill, 11.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                            ui.painter().galley(pill.center() - g.size() / 2.0, g, t.text2);
                            prev = None;
                        }
                        let compact = prev.map_or(false, |(a, ts)| a == m.author && m.ts - ts < 300) && m.post_title.is_none() && m.call.is_none();
                        self.message_row_c(ui, cid, m, true, compact, &mut acts);
                        prev = Some((m.author, m.ts));
                    }
                });
            });
            egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 12, top: 4, bottom: 10 }).show(ui, |ui| {
                let target = match channel.kind {
                    ChannelKind::Channel => if channel.name == "general" { "General".to_string() } else { channel.name.clone() },
                    ChannelKind::Direct => self.channel_label(&channel),
                };
                if let Some((text, title, files)) = self.channel_composer(ui, &target) {
                    match title {
                        Some(tt) => acts.push(ChatAction::SendPost { ch: cid, title: tt, text, files }),
                        None => acts.push(ChatAction::Send { ch: cid, text, parent: None, files }),
                    }
                }
            });
        });
        if dragging {
            let r = ui.max_rect();
            let p = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop_hint")));
            p.rect_filled(r.shrink(8.0), 12.0, t.primary.gamma_multiply(0.12));
            p.rect_stroke(r.shrink(8.0), 12.0, Stroke::new(2.0, t.primary), egui::StrokeKind::Inside);
            p.text(r.center(), Align2::CENTER_CENTER, "Drop pictures or videos to send", FontId::proportional(18.0), t.text);
        }

        // mark as read
        let now = now_ts();
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == cid) {
            if c.messages.iter().any(|m| m.ts > c.last_read) {
                c.last_read = now;
                if self.persist {
                    self.dirty = true;
                }
            }
        }
        self.apply_chat(acts);
    }

    /// Channel message box with Message / Post / Ask tabs. Returns (text, post
    /// title, attachments).
    fn channel_composer(&mut self, ui: &mut Ui, target: &str) -> Option<(String, Option<String>, Vec<Attachment>)> {
        let t = self.t();
        let mut out = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (i, (icon, label)) in [("\u{1F4AC}", "Message"), ("\u{1F4E2}", "Post"), ("\u{2728}", "Ask")].iter().enumerate() {
                let sel = self.composer_mode == i as u8;
                let b = egui::Button::new(RichText::new(format!("{} {}", icon, label)).size(13.5).color(if sel { t.text } else { t.text2 })).fill(if sel { t.bg } else { Color32::TRANSPARENT }).stroke(if sel { Stroke::new(1.0, t.border) } else { Stroke::NONE }).corner_radius(6);
                if ui.add(b).clicked() {
                    if i == 2 {
                        self.section = Section::Ai;
                    } else {
                        self.composer_mode = i as u8;
                    }
                }
            }
        });
        let text_id = egui::Id::new(super::files::CHAT_BOX);
        let bg_id = egui::Id::new("chat_composer_bg");
        let frame_resp = egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::symmetric(12, 8)).fill(t.card).show(ui, |ui| {
            // a click anywhere on the box (not on its buttons) puts the cursor in the message
            let last: Option<Rect> = ui.ctx().data(|d| d.get_temp(bg_id));
            if let Some(r) = last {
                let bg = ui.interact(r, bg_id, Sense::click());
                if bg.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                }
                if bg.clicked() {
                    ui.memory_mut(|m| m.request_focus(text_id));
                }
            }
            self.pending_strip(ui, false);
            if self.composer_mode == 1 {
                ui.add(egui::TextEdit::singleline(&mut self.post_title).hint_text("Title").font(FontId::proportional(15.0)).frame(false).desired_width(f32::INFINITY));
            }
            let hint = if self.composer_mode == 1 { "Share an announcement...".to_string() } else { format!("Write to {}, press 'space' for AI, '/' for commands", target) };
            let r = ui.add(egui::TextEdit::multiline(&mut self.chat_text).id(text_id).hint_text(hint).frame(false).desired_rows(1).desired_width(f32::INFINITY));
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Message composer"));
            let enter = r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
            ui.horizontal(|ui| {
                for (g, tip) in [("+", "Add pictures or videos"), ("@", "Mention someone"), ("\u{263A}", "Emoji"), ("\u{2611}", "Create task")] {
                    let b = ui.add(egui::Button::new(RichText::new(g).color(t.text2)).frame(false)).on_hover_text(tip);
                    b.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
                    if b.clicked() {
                        match g {
                            "+" => self.pick_media(false),
                            "@" => self.chat_text.push('@'),
                            "\u{263A}" => self.chat_text.push_str("\u{1F44D}"),
                            "\u{2611}" => self.open_quick_add(None),
                            _ => {}
                        }
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if self.composer_mode == 1 { "Post" } else { "\u{27A4}" };
                    let (files_ok, files) = self.pending_ready(false);
                    let ready = files_ok && (!self.chat_text.trim().is_empty() || !files.is_empty() || (self.composer_mode == 1 && !self.post_title.trim().is_empty()));
                    let b = egui::Button::new(RichText::new(label).color(if ready { Color32::WHITE } else { t.text3 })).fill(if ready { t.primary } else { t.hover }).corner_radius(6).min_size(vec2(34.0, 28.0));
                    if (ui.add(b).clicked() || enter) && ready {
                        let text = self.chat_text.trim().to_string();
                        let title = if self.composer_mode == 1 { Some(if self.post_title.trim().is_empty() { "Update".to_string() } else { self.post_title.trim().to_string() }) } else { None };
                        out = Some((text, title, files));
                        self.chat_text.clear();
                        self.post_title.clear();
                        self.chat_files.clear();
                    } else if enter && files_ok && self.chat_files.is_empty() {
                        self.chat_text.clear();
                    }
                });
            });
        });
        ui.ctx().data_mut(|d| d.insert_temp(bg_id, frame_resp.response.rect));
        out
    }

    /// The thread's message box; returns the text and attachments when Enter
    /// (without Shift) or Send is pressed.
    fn composer(&mut self, ui: &mut Ui, hint: &str, thread: bool) -> Option<(String, Vec<Attachment>)> {
        let t = self.t();
        let mut out = None;
        egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(8)).fill(t.card).show(ui, |ui| {
            self.pending_strip(ui, thread);
            let (files_ok, files) = self.pending_ready(thread);
            let has_files = !(if thread { &self.thread_files } else { &self.chat_files }).is_empty();
            let id = egui::Id::new(if thread { super::files::THREAD_BOX } else { super::files::CHAT_BOX });
            let text = if thread { &mut self.thread_text } else { &mut self.chat_text };
            let r = ui.add(egui::TextEdit::multiline(text).id(id).hint_text(hint).frame(false).desired_rows(2).desired_width(f32::INFINITY));
            r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, if thread { "Reply composer" } else { "Message composer" }));
            let enter = r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
            let mut pick = false;
            ui.horizontal(|ui| {
                let b = ui.add(egui::Button::new(RichText::new("+").color(t.text2)).frame(false)).on_hover_text("Add pictures or videos");
                b.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Add pictures or videos to the reply"));
                pick = b.clicked();
                ui.label(RichText::new("Enter to send, Shift+Enter for a new line").small().color(t.text3));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ready = files_ok && (!text.trim().is_empty() || !files.is_empty());
                    if (primary_button(ui, &t, "Send").clicked() || enter) && ready {
                        out = Some((text.trim().to_string(), files));
                        text.clear();
                    } else if enter && !has_files {
                        text.clear();
                    }
                });
            });
            if out.is_some() {
                if thread {
                    self.thread_files.clear();
                } else {
                    self.chat_files.clear();
                }
            }
            if pick {
                self.pick_media(thread);
            }
        });
        out
    }

    fn message_row(&self, ui: &mut Ui, cid: u64, m: &Message, top_level: bool, acts: &mut Vec<ChatAction>) {
        self.message_row_c(ui, cid, m, top_level, false, acts)
    }

    /// `compact`: a follow-up from the same person a moment later, shown
    /// without the avatar and name (as the original groups messages).
    fn message_row_c(&self, ui: &mut Ui, cid: u64, m: &Message, top_level: bool, compact: bool, acts: &mut Vec<ChatAction>) {
        let t = self.t();
        let w = ui.available_width();
        let start = ui.cursor().min;
        // the hover background goes underneath the message, not over it
        let bg = ui.painter().add(egui::Shape::Noop);
        let inner = ui.horizontal_top(|ui| {
            ui.set_width(w);
            let (ar, _) = ui.allocate_exact_size(vec2(36.0, if compact { 18.0 } else { 36.0 }), Sense::hover());
            if compact {
                if ui.rect_contains_pointer(ar.expand2(vec2(0.0, 6.0)).union(Rect::from_min_size(ar.min, vec2(w, 22.0)))) {
                    let time = chrono::DateTime::from_timestamp(m.ts, 0).map(|x| x.with_timezone(&chrono::Local).format("%-I:%M").to_string()).unwrap_or_default();
                    ui.painter().text(ar.center(), Align2::CENTER_CENTER, time, FontId::proportional(10.5), t.text3);
                }
            } else if let Some(au) = self.data.member(m.author) {
                paint_avatar(ui.painter(), ar.center(), 15.0, au, t.bg);
            }
            ui.vertical(|ui| {
                ui.set_width(w - 50.0);
                if !compact {
                ui.horizontal(|ui| {
                    let name = self.data.member(m.author).map(|x| x.name.clone()).unwrap_or_else(|| "Someone".into());
                    ui.label(RichText::new(name).strong());
                    let time = chrono::DateTime::from_timestamp(m.ts, 0).map(|x| x.with_timezone(&chrono::Local).format("%-I:%M %p").to_string()).unwrap_or_default();
                    ui.label(RichText::new(time).small().color(t.text3));
                    if m.saved {
                        ui.label(RichText::new("\u{1F516} Saved").small().color(Color32::from_rgb(0xe1, 0x6b, 0x16)));
                    }
                    if m.pinned {
                        ui.label(RichText::new("\u{1F4CC} Pinned").small().color(t.text3));
                    }
                });
                }
                if let Some(pt) = &m.post_title {
                    ui.label(RichText::new(format!("\u{1F4E2} {}", pt)).size(16.0).color(t.text));
                }
                if !m.text.is_empty() {
                    ui.add(egui::Label::new(self.mention_text(&m.text, 14.0)).wrap());
                }
                if let Some(index) = self.attachment_tiles(ui, m) {
                    acts.push(ChatAction::OpenMedia { ch: cid, msg: m.id, index });
                }
                if let Some(ch) = m.call.as_deref().and_then(super::huddle::huddle_channel) {
                    let here = self.huddle.channel == Some(ch);
                    egui::Frame::new().fill(Color32::from_rgb(0x00, 0x88, 0x44).gamma_multiply(0.08)).stroke(Stroke::new(1.0, t.border)).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 15.0, Color32::from_rgb(0x00, 0x88, 0x44));
                            super::shell::line_icon(ui.painter(), "phone", r.center(), Color32::WHITE);
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Huddle").strong());
                                ui.label(RichText::new(if here { "You're in it" } else { "Voice call in Workboard" }).small().color(t.text3));
                            });
                            ui.add_space(16.0);
                            if !here {
                                let join = ui.add(egui::Button::new(RichText::new("Join").color(Color32::WHITE)).fill(Color32::from_rgb(0x00, 0x88, 0x44)).corner_radius(6).min_size(vec2(64.0, 28.0)));
                                name_it(&join, "Join huddle");
                                if join.clicked() {
                                    acts.push(ChatAction::JoinHuddle(ch));
                                }
                            }
                        });
                    });
                } else if let Some(link) = &m.call {
                    egui::Frame::new().fill(Color32::from_rgb(0x00, 0x88, 0x44).gamma_multiply(0.08)).stroke(Stroke::new(1.0, t.border)).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 15.0, Color32::from_rgb(0x00, 0x88, 0x44));
                            super::shell::line_icon(ui.painter(), "phone", r.center(), Color32::WHITE);
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Call").strong());
                                ui.label(RichText::new(truncate(link, 46)).small().color(t.text3));
                            });
                            ui.add_space(16.0);
                            let join = ui.add(egui::Button::new(RichText::new("Join").color(Color32::WHITE)).fill(Color32::from_rgb(0x00, 0x88, 0x44)).corner_radius(6).min_size(vec2(64.0, 28.0)));
                            name_it(&join, "Join call");
                            if join.clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(link));
                            }
                            if ui.button("Copy link").clicked() {
                                ui.ctx().copy_text(link.clone());
                            }
                        });
                    });
                }
                if let Some(tid) = m.task {
                    if let Some(task) = self.task(tid) {
                        let st = self.data.status_of(task);
                        let r = egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(6).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                paint_status_circle(ui.painter(), r.center(), 5.5, &st);
                                ui.label(RichText::new(&task.name).size(12.5));
                                ui.label(RichText::new(self.data.list_name(task.list_id)).small().color(t.text3));
                            });
                        });
                        if ui.interact(r.response.rect, ui.id().with(("msgtask", m.id)), Sense::click()).clicked() {
                            // open the linked task through the main action queue
                            acts.push(ChatAction::Task { ch: u64::MAX, msg: tid });
                        }
                    }
                }
                if !m.reactions.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for rc in &m.reactions {
                            let mine = rc.users.contains(&self.data.me);
                            let txt = format!("{} {}", rc.emoji, rc.users.len());
                            let b = egui::Button::new(RichText::new(txt).size(12.0))
                                .fill(if mine { t.selected_bg } else { t.hover })
                                .stroke(Stroke::new(1.0, if mine { t.primary } else { t.border }))
                                .corner_radius(10);
                            let names: Vec<String> = rc.users.iter().filter_map(|u| self.data.member(*u)).map(|x| x.name.clone()).collect();
                            if ui.add(b).on_hover_text(names.join(", ")).clicked() {
                                acts.push(ChatAction::React { ch: cid, msg: m.id, emoji: rc.emoji.clone() });
                            }
                        }
                    });
                }
                if top_level && !m.replies.is_empty() {
                    let last = m.replies.last().map(|r| fmt_ago(r.ts)).unwrap_or_default();
                    ui.horizontal(|ui| {
                        paint_avatars(ui.painter(), ui.cursor().left_center() + vec2(0.0, 9.0), 8.0, &m.replies.iter().map(|r| r.author).collect::<Vec<_>>(), &self.data, t.bg);
                        ui.add_space(30.0);
                        let label = if m.replies.len() == 1 { "1 reply".to_string() } else { format!("{} replies", m.replies.len()) };
                        // a link color that reads on both themes (dark primary is a dim grey)
                        let (link, link_hover) = if t.dark { (t.accent_text, t.text) } else { (t.primary, t.accent_text) };
                        let g = ui.painter().layout_no_wrap(label.clone(), FontId::proportional(13.0), link);
                        let (lr, lk) = ui.allocate_exact_size(g.size() + vec2(4.0, 4.0), Sense::click());
                        name_it(&lk, &label);
                        let col = if lk.hovered() { link_hover } else { link };
                        ui.painter().galley(lr.min + vec2(2.0, 2.0), g, col);
                        if lk.hovered() {
                            ui.painter().hline(lr.x_range().shrink(2.0), lr.bottom() - 1.0, Stroke::new(1.0, col));
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if lk.clicked() {
                            acts.push(ChatAction::Thread(m.id));
                        }
                        ui.label(RichText::new(format!("Last reply {}", last)).small().color(t.text3));
                    });
                }
            });
        });
        let rect = Rect::from_min_max(start, pos2(start.x + w, inner.response.rect.bottom() + if compact { 1.0 } else { 4.0 }));
        ui.add_space(if compact { 2.0 } else { 8.0 });
        // hover toolbar
        if ui.rect_contains_pointer(rect) {
            ui.painter().set(bg, egui::Shape::rect_filled(rect.expand2(vec2(6.0, 2.0)), 6.0, t.hover));
            let n = QUICK_REACTIONS.len() + if top_level { 4 } else { 1 };
            let bw = 30.0;
            let bar = Rect::from_min_size(pos2(rect.right() - bw * n as f32 - 8.0, rect.top() - 10.0), vec2(bw * n as f32 + 4.0, 28.0));
            ui.painter().rect_filled(bar, 7.0, t.card);
            ui.painter().rect_stroke(bar, 7.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            let mut x = bar.left() + 2.0;
            let mut button = |ui: &mut Ui, label: &str, tip: &str| -> bool {
                let r = Rect::from_min_size(pos2(x, bar.top() + 2.0), vec2(bw, 24.0));
                x += bw;
                put_over(ui, r, egui::Button::new(label).frame(false)).on_hover_text(tip).clicked()
            };
            for e in QUICK_REACTIONS {
                if button(ui, e, "Add reaction") {
                    acts.push(ChatAction::React { ch: cid, msg: m.id, emoji: e.to_string() });
                }
            }
            if top_level {
                if button(ui, "\u{1F4AC}", "Reply in thread") {
                    acts.push(ChatAction::Thread(m.id));
                }
                if button(ui, "\u{2611}", "Create task from message") {
                    acts.push(ChatAction::Task { ch: cid, msg: m.id });
                }
            }
            if top_level && button(ui, "\u{1F4CC}", if m.pinned { "Unpin" } else { "Pin message" }) {
                acts.push(ChatAction::Pin { ch: cid, msg: m.id });
            }
            if button(ui, "\u{1F516}", if m.saved { "Remove from saved" } else { "Save for later" }) {
                acts.push(ChatAction::Save { ch: cid, msg: m.id });
            }
        }
    }

    pub(super) fn apply_chat(&mut self, acts: Vec<ChatAction>) {
        let me = self.data.me;
        for a in acts {
            match a {
                ChatAction::JoinHuddle(ch) => self.join_huddle(ch),
                ChatAction::Open(id) => {
                    self.chat_open = Some(id);
                    self.thread_open = None;
                }
                ChatAction::Thread(id) => self.thread_open = Some(id),
                ChatAction::CloseThread => self.thread_open = None,
                ChatAction::React { ch, msg, emoji } => {
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(m) = find_msg(&mut c.messages, msg) {
                            if let Some(i) = m.reactions.iter().position(|r| r.emoji == emoji) {
                                let r = &mut m.reactions[i];
                                if let Some(j) = r.users.iter().position(|u| *u == me) {
                                    r.users.remove(j);
                                } else {
                                    r.users.push(me);
                                }
                                if r.users.is_empty() {
                                    m.reactions.remove(i);
                                }
                            } else {
                                m.reactions.push(Reaction { emoji, users: vec![me] });
                            }
                            self.dirty = true;
                        }
                    }
                }
                ChatAction::Pin { ch, msg } => {
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(m) = c.messages.iter_mut().find(|m| m.id == msg) {
                            m.pinned = !m.pinned;
                            self.dirty = true;
                        }
                    }
                }
                ChatAction::SendPost { ch, title, text, files } => {
                    let id = self.data.new_id();
                    let m = Message { attachments: files, id, author: me, text, ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: Some(title), post_type: "Announcement".into(), pinned: false, call: None };
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        c.messages.push(m);
                        c.last_read = now_ts();
                    }
                    self.dirty = true;
                }
                ChatAction::Save { ch, msg } => {
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(m) = find_msg(&mut c.messages, msg) {
                            m.saved = !m.saved;
                            self.dirty = true;
                        }
                    }
                }
                ChatAction::Task { ch, msg } => {
                    if ch == u64::MAX {
                        // msg holds a task id here: open it
                        self.apply(Action::Open(msg));
                        continue;
                    }
                    let text = self.data.channels.iter().find(|c| c.id == ch).and_then(|c| c.messages.iter().find(|m| m.id == msg)).map(|m| m.text.clone());
                    let Some(text) = text else { continue };
                    let Some(list) = self.data.first_list() else { continue };
                    let name: String = text.lines().next().unwrap_or("").chars().take(80).collect();
                    self.apply(Action::Add { list, name, status: None, priority: 0, assignees: vec![me], due: None, parent: None, open: true });
                    let tid = self.data.next_id;
                    if let Some(task) = self.data.tasks.iter_mut().find(|x| x.id == tid) {
                        task.description = text.clone();
                    }
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(m) = c.messages.iter_mut().find(|m| m.id == msg) {
                            m.task = Some(tid);
                        }
                    }
                    self.dirty = true;
                }
                ChatAction::OpenMedia { ch, msg, index } => {
                    self.viewer = Some(super::files::Viewer::new(ch, msg, index));
                }
                ChatAction::Send { ch, text, parent, files } => {
                    let id = self.data.new_id();
                    let m = Message { attachments: files, id, author: me, text, ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: None };
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        match parent {
                            Some(pid) => {
                                if let Some(p) = c.messages.iter_mut().find(|x| x.id == pid) {
                                    p.replies.push(m);
                                }
                            }
                            None => c.messages.push(m),
                        }
                        c.last_read = now_ts();
                    }
                    self.dirty = true;
                }
                ChatAction::NewChannel(name) => {
                    let id = self.data.new_id();
                    let name = name.trim().to_lowercase().replace(' ', "-");
                    let members = self.data.members.iter().map(|m| m.id).collect();
                    self.data.channels.push(Channel { id, name, kind: ChannelKind::Channel, topic: String::new(), members, messages: vec![], last_read: now_ts(), ..Default::default() });
                    self.chat_open = Some(id);
                    self.thread_open = None;
                    self.dirty = true;
                }
                ChatAction::Dm(uid) => {
                    let existing = self.data.channels.iter().find(|c| c.kind == ChannelKind::Direct && c.members.contains(&uid) && c.members.contains(&me)).map(|c| c.id);
                    let id = match existing {
                        Some(id) => id,
                        None => {
                            let id = self.data.new_id();
                            self.data.channels.push(Channel { id, name: String::new(), kind: ChannelKind::Direct, topic: String::new(), members: vec![me, uid], messages: vec![], last_read: now_ts(), ..Default::default() });
                            self.dirty = true;
                            id
                        }
                    };
                    self.chat_open = Some(id);
                    self.thread_open = None;
                }
            }
        }
    }
}
