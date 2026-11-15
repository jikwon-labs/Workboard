// Pages reached from the sidebar, the rail and the menus: People, Posts,
// Meetings, Goals, Forms, Whiteboards, Reminders, AI and the Overview view.

use super::*;
use super::menus::ROLES;

fn title(ui: &mut Ui, t: &Theme, text: &str, sub: &str) {
    ui.label(RichText::new(text).size(20.0));
    if !sub.is_empty() {
        ui.label(RichText::new(sub).color(t.text2));
    }
    ui.add_space(10.0);
}

fn list_panel_frame(t: &Theme) -> egui::Frame {
    egui::Frame::new().fill(t.sidebar).corner_radius(egui::CornerRadius { nw: 8, ne: 0, sw: 8, se: 0 }).inner_margin(egui::Margin::symmetric(10, 12))
}

/// Space Overview cards that Customize can turn off.
pub const OV_CARDS: [&str; 5] = ["Recent", "Docs", "Bookmarks", "Folders", "Lists"];

impl App {
    // ---------------------------------------------------------------- people

    pub(super) fn people_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("People").size(20.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, &t, "Invite people").clicked() {
                            self.modal = Some(Modal::Invite { emails: String::new(), role: "Member".into() });
                        }
                    });
                });
                ui.label(RichText::new(format!("Manage who can access {}", self.data.workspace)).color(t.text2));
                ui.add_space(12.0);
                let w = ui.available_width();
                let (hr, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
                for (x, h) in [(0.0, "Name"), (0.32, "Email"), (0.6, "Role"), (0.76, "Status")] {
                    ui.painter().text(pos2(hr.left() + w * x + 8.0, hr.center().y), Align2::LEFT_CENTER, h, FontId::proportional(13.0), t.text2);
                }
                ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
                let members = self.data.members.clone();
                let me = self.data.me;
                let mut remove = None;
                for m in &members {
                    let (r, _) = ui.allocate_exact_size(vec2(w, 46.0), Sense::hover());
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                    paint_avatar(ui.painter(), pos2(r.left() + 22.0, r.center().y), 13.0, m, t.bg);
                    let name = if m.id == me { format!("{} (you)", m.name) } else { m.name.clone() };
                    ui.painter().text(pos2(r.left() + 44.0, r.center().y - (if m.status_text.is_empty() { 0.0 } else { 8.0 })), Align2::LEFT_CENTER, name, FontId::proportional(14.0), t.text);
                    if !m.status_text.is_empty() {
                        ui.painter().text(pos2(r.left() + 44.0, r.center().y + 9.0), Align2::LEFT_CENTER, &m.status_text, FontId::proportional(12.0), t.text3);
                    }
                    ui.painter().text(pos2(r.left() + w * 0.32 + 8.0, r.center().y), Align2::LEFT_CENTER, if m.email.is_empty() { "-" } else { &m.email }, FontId::proportional(13.5), t.text2);
                    let role_rect = Rect::from_min_size(pos2(r.left() + w * 0.6, r.center().y - 13.0), vec2(w * 0.15, 26.0));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(role_rect).layout(Layout::left_to_right(Align::Center)));
                    let mut role = if m.role.is_empty() { "Member".to_string() } else { m.role.clone() };
                    if m.role == "Owner" {
                        child.label("Owner");
                    } else {
                        egui::ComboBox::from_id_salt(("role", m.id)).selected_text(role.clone()).width(100.0).show_ui(&mut child, |ui| {
                            for (n, _) in ROLES.iter().take(3) {
                                ui.selectable_value(&mut role, n.to_string(), *n);
                            }
                        });
                        if role != m.role && !(m.role.is_empty() && role == "Member") {
                            if let Some(mm) = self.data.members.iter_mut().find(|x| x.id == m.id) {
                                mm.role = role;
                                self.dirty = true;
                            }
                        }
                    }
                    let (st, col) = if m.invited { ("Invited", Color32::from_rgb(0xe1, 0x6b, 0x16)) } else { ("Active", Color32::from_rgb(41, 151, 100)) };
                    ui.painter().text(pos2(r.left() + w * 0.76 + 8.0, r.center().y), Align2::LEFT_CENTER, st, FontId::proportional(13.0), col);
                    if m.id != me && m.role != "Owner" {
                        let br = Rect::from_min_size(pos2(r.right() - 80.0, r.center().y - 12.0), vec2(72.0, 24.0));
                        if put_over(ui, br, egui::Button::new(RichText::new("Remove").color(t.danger)).frame(false)).clicked() {
                            remove = Some(m.id);
                        }
                    }
                }
                if let Some(id) = remove {
                    self.data.members.retain(|m| m.id != id);
                    for task in &mut self.data.tasks {
                        task.assignees.retain(|a| *a != id);
                    }
                    for c in &mut self.data.channels {
                        c.members.retain(|a| *a != id);
                    }
                    self.dirty = true;
                }
                ui.add_space(12.0);
                ui.label(RichText::new("Invited people get access when the shared server connects them. Until then they can use this computer through Log out \u{2192} Switch person.").small().color(t.text3));
            });
        });
    }

    // ---------------------------------------------------------------- inbox (Primary / Other / Later / Cleared)

    pub(super) fn inbox_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let my_name = self.data.member(me).map(|m| format!("@{}", m.name)).unwrap_or_default();
        // tabs
        let w = ui.available_width();
        let (tr, _) = ui.allocate_exact_size(vec2(w, 50.0), Sense::hover());
        let tw = (w - 20.0) / 4.0;
        for (i, (icon, label)) in [("\u{1F4E5}", "Primary"), ("\u{26A1}", "Other"), ("\u{23F0}", "Later"), ("\u{2714}", "Cleared")].iter().enumerate() {
            let r = Rect::from_min_size(pos2(tr.left() + 10.0 + tw * i as f32, tr.top()), vec2(tw, 50.0));
            let resp = ui.interact(r, ui.id().with(("itab", i)), Sense::click());
            name_it(&resp, &format!("Inbox {}", label));
            let sel = self.inbox_tab == i as u8;
            if resp.hovered() && !sel {
                ui.painter().rect_filled(r.shrink(4.0), 6.0, t.hover);
            }
            ui.painter().text(r.left_center() + vec2(26.0, 0.0), Align2::CENTER_CENTER, *icon, FontId::proportional(14.0), if sel { t.text } else { t.text2 });
            ui.painter().text(r.left_center() + vec2(44.0, 0.0), Align2::LEFT_CENTER, *label, FontId::proportional(15.0), if sel { t.text } else { t.text2 });
            if sel {
                ui.painter().rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 2.0), pos2(r.right() - 4.0, r.bottom())), 1.0, t.text);
            }
            if i > 0 {
                ui.painter().vline(r.left(), r.top() + 14.0..=r.bottom() - 14.0, Stroke::new(1.0, t.border));
            }
            if resp.clicked() {
                self.inbox_tab = i as u8;
            }
        }
        ui.painter().hline(tr.x_range(), tr.bottom(), Stroke::new(1.0, t.border));
        // collect items
        let cleared = self.data.extra.inbox_cleared.clone();
        let later = self.data.extra.inbox_later.clone();
        let n = self.data.extra.notify.clone();
        let mut items: Vec<(String, i64, u64, String, Activity)> = vec![];
        for task in &self.data.tasks {
            for a in &task.activity {
                let key = format!("{}:{}:{}", task.id, a.ts, a.author);
                let primary = a.author != me && ((a.comment && (n.comments || (n.mentions && a.text.contains(&my_name)))) || (n.assigned && a.text.contains("assigned to")));
                let tab = if cleared.contains(&key) {
                    3
                } else if later.contains(&key) {
                    2
                } else if primary {
                    0
                } else {
                    1
                };
                if tab == self.inbox_tab {
                    items.push((key, a.ts, task.id, task.name.clone(), a.clone()));
                }
            }
        }
        match self.inbox_filter {
            1 => items.retain(|x| x.4.comment),
            2 => items.retain(|x| x.4.comment && x.4.text.contains(&my_name)),
            3 => items.retain(|x| x.4.text.contains("assigned to")),
            _ => {}
        }
        items.sort_by_key(|x| std::cmp::Reverse(x.1));
        let mut clear_all = false;
        egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 10)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let label = ["\u{2261}  Filter", "\u{2261}  Comments", "\u{2261}  Mentions", "\u{2261}  Assigned"][self.inbox_filter as usize % 4];
                let fr = ui.add(egui::Button::new(label).corner_radius(13));
                popup(ui, &fr, ui.id().with("inbox_filter"), 180.0, |ui| {
                    for (i, l) in ["Everything", "Comments", "Mentions", "Assigned to me"].iter().enumerate() {
                        if ui.selectable_label(self.inbox_filter == i as u8, *l).clicked() {
                            self.inbox_filter = i as u8;
                            close_popup(ui);
                        }
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add_enabled(!items.is_empty() && self.inbox_tab != 3, egui::Button::new("\u{2714}  Clear all").corner_radius(6)).clicked() {
                        clear_all = true;
                    }
                    if ui.add(egui::Button::new("\u{2699}").corner_radius(6)).on_hover_text("Notification settings").clicked() {
                        self.modal = Some(Modal::Notifications);
                    }
                    if ui.add(egui::Button::new("\u{23F0}  Reminder").corner_radius(6)).clicked() {
                        self.modal = Some(Modal::Reminder { text: String::new(), date: Some(today()), time: "09:00".into() });
                    }
                });
            });
        });
        if clear_all {
            for (k, ..) in &items {
                self.data.extra.inbox_later.retain(|x| x != k);
                self.data.extra.inbox_cleared.push(k.clone());
            }
            self.dirty = true;
        }
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 0)).show(ui, |ui| {
                if self.inbox_tab == 0 {
                    self.reminders_block(ui);
                }
                if items.is_empty() {
                    ui.add_space(ui.available_height() * 0.3);
                    ui.vertical_centered(|ui| {
                        let alone = self.data.members.len() <= 1;
                        let (title, sub) = match self.inbox_tab {
                            0 if alone => ("Looking to collaborate?", "Collaboration is one invite away."),
                            0 => ("You're all caught up", "New comments, mentions and assignments show up here."),
                            1 => ("Nothing in Other", "Updates that don't need you show up here."),
                            2 => ("Nothing saved for later", "Use Later on an item to come back to it."),
                            _ => ("Nothing cleared yet", "Cleared items show up here."),
                        };
                        let (r, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                        ui.painter().rect_stroke(r.shrink(2.0), 14.0, Stroke::new(1.5, t.border), egui::StrokeKind::Inside);
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, "\u{1F465}", FontId::proportional(22.0), Color32::from_rgb(0x5f, 0x55, 0xee));
                        ui.add_space(10.0);
                        ui.label(RichText::new(title).size(18.0).color(t.text));
                        ui.label(RichText::new(sub).size(14.0).color(t.text2));
                        if self.inbox_tab == 0 {
                            ui.add_space(10.0);
                            if ui.add(egui::Button::new(RichText::new("Invite people").size(15.0).color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(0.0, 38.0))).clicked() {
                                self.modal = Some(Modal::Invite { emails: String::new(), role: "Member".into() });
                            }
                        }
                    });
                }
                for (key, ts, tid, tname, a) in items {
                    let w = ui.available_width();
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 58.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 8.0, t.hover);
                    }
                    ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                    if let Some(m) = self.data.member(a.author) {
                        paint_avatar(ui.painter(), r.left_center() + vec2(22.0, 0.0), 13.0, m, t.bg);
                    }
                    ui.painter().text(r.left_top() + vec2(48.0, 18.0), Align2::LEFT_CENTER, &tname, FontId::proportional(14.0), t.text);
                    let who = self.data.member(a.author).map(|m| m.name.clone()).unwrap_or_default();
                    let line = if a.comment { format!("{} commented: {}", who, a.text) } else { format!("{} {}", who, a.text) };
                    ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.right() - 190.0, r.bottom()))).text(r.left_top() + vec2(48.0, 40.0), Align2::LEFT_CENTER, line, FontId::proportional(13.0), t.text2);
                    ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, fmt_ago(ts), FontId::proportional(12.0), t.text3);
                    let mut handled = false;
                    if resp.hovered() || ui.rect_contains_pointer(r) {
                        let lr = Rect::from_min_size(pos2(r.right() - 180.0, r.center().y - 13.0), vec2(56.0, 26.0));
                        let cr = Rect::from_min_size(pos2(r.right() - 120.0, r.center().y - 13.0), vec2(56.0, 26.0));
                        if self.inbox_tab != 2 && put_over(ui, lr, egui::Button::new("Later").corner_radius(6)).clicked() {
                            self.data.extra.inbox_later.push(key.clone());
                            self.dirty = true;
                            handled = true;
                        }
                        let ctext = if self.inbox_tab == 3 { "Restore" } else { "Clear" };
                        if put_over(ui, cr, egui::Button::new(ctext).corner_radius(6)).clicked() {
                            if self.inbox_tab == 3 {
                                self.data.extra.inbox_cleared.retain(|x| x != &key);
                            } else {
                                self.data.extra.inbox_later.retain(|x| x != &key);
                                self.data.extra.inbox_cleared.push(key.clone());
                            }
                            self.dirty = true;
                            handled = true;
                        }
                    }
                    if resp.clicked() && !handled {
                        self.actions.push(Action::Open(tid));
                    }
                }
            });
        });
    }

    // ---------------------------------------------------------------- posts

    pub(super) fn posts_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.page_bar(ui, "Posts", |s, ui| {
            let tt = s.t();
            if primary_button(ui, &tt, "New Post").clicked() {
                s.post_title.clear();
                s.post_body.clear();
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let w = ui.available_width();
            let cw = 960.0f32.min(w - 60.0);
            let pad = ((w - cw) / 2.0).max(0.0);
            ui.add_space(20.0);
            ui.horizontal_top(|ui| {
                ui.add_space((pad - 50.0).max(0.0));
                if let Some(m) = self.data.member(self.data.me).cloned() {
                    let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                    paint_avatar(ui.painter(), r.center(), 15.0, &m, t.bg);
                }
                egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::symmetric(16, 10)).show(ui, |ui| {
                    ui.set_width(cw - 34.0);
                    // the frame sits in a horizontal row; stack its contents top to bottom
                    ui.vertical(|ui| {
                    let chans: Vec<(u64, String)> = self.data.channels.iter().filter(|c| c.kind == ChannelKind::Channel).map(|c| (c.id, c.name.clone())).collect();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Channel:").color(t.text2));
                        let cur = chans.iter().find(|c| c.0 == self.post_channel).map(|c| format!("# {}", c.1)).unwrap_or_else(|| "Select Channel".into());
                        egui::ComboBox::from_id_salt("post_ch").selected_text(cur).show_ui(ui, |ui| {
                            for (id, n) in &chans {
                                ui.selectable_value(&mut self.post_channel, *id, format!("# {}", n));
                            }
                        });
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Type:").color(t.text2));
                        egui::ComboBox::from_id_salt("post_type").selected_text(format!("\u{1F4E2} {}", self.post_type)).show_ui(ui, |ui| {
                            for k in ["Announcement", "Update", "Idea", "Question"] {
                                ui.selectable_value(&mut self.post_type, k.to_string(), k);
                            }
                        });
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Title:").color(t.text2));
                        ui.add(egui::TextEdit::singleline(&mut self.post_title).frame(false).desired_width(f32::INFINITY));
                    });
                    ui.separator();
                    ui.add(egui::TextEdit::multiline(&mut self.post_body).hint_text("Share an announcement...").frame(false).desired_rows(3).desired_width(f32::INFINITY));
                    ui.horizontal(|ui| {
                        for (g, tip) in [("+", "Create task"), ("@", "Mention someone"), ("\u{263A}", "Emoji"), ("\u{2611}", "Create task")] {
                            if ui.add(egui::Button::new(RichText::new(g).color(t.text2)).frame(false)).on_hover_text(tip).clicked() {
                                match g {
                                    "@" => self.post_body.push('@'),
                                    "\u{263A}" => self.post_body.push_str("\u{1F44D}"),
                                    _ => self.open_quick_add(None),
                                }
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let ready = !self.post_title.trim().is_empty() && self.post_channel != 0;
                            let b = egui::Button::new(RichText::new("Post").color(if ready { Color32::WHITE } else { t.text3 })).fill(if ready { t.primary } else { t.hover }).corner_radius(6).min_size(vec2(56.0, 30.0));
                            if ui.add(b).on_hover_text(if self.post_channel == 0 { "Select a channel first" } else { "" }).clicked() && ready {
                                let id = self.data.new_id();
                                let me = self.data.me;
                                let m = Message { edited: None, attachments: vec![], id, author: me, text: self.post_body.trim().to_string(), ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: Some(self.post_title.trim().to_string()), post_type: self.post_type.clone(), pinned: false, call: None };
                                if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == self.post_channel) {
                                    c.messages.push(m);
                                    c.last_read = now_ts();
                                }
                                self.post_title.clear();
                                self.post_body.clear();
                                self.dirty = true;
                            }
                        });
                    });
                    });
                });
            });
            ui.add_space(20.0);
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let fol = egui::Button::new(RichText::new("\u{1F514}  Channels I'm Following").color(if self.post_following { t.accent_text } else { t.text2 })).fill(if self.post_following { t.accent_bg } else { t.bg }).corner_radius(13);
                if ui.add(fol).clicked() {
                    self.post_following = !self.post_following;
                }
                let chans: Vec<(u64, String)> = self.data.channels.iter().filter(|c| c.kind == ChannelKind::Channel).map(|c| (c.id, c.name.clone())).collect();
                let cl = self.post_f_channel.and_then(|id| chans.iter().find(|c| c.0 == id)).map(|c| format!("#  {}", c.1)).unwrap_or_else(|| "#  Channel".into());
                let cr = ui.add(egui::Button::new(cl).corner_radius(13));
                popup(ui, &cr, ui.id().with("pf_ch"), 180.0, |ui| {
                    if ui.selectable_label(self.post_f_channel.is_none(), "All channels").clicked() {
                        self.post_f_channel = None;
                        close_popup(ui);
                    }
                    for (id, n) in &chans {
                        if ui.selectable_label(self.post_f_channel == Some(*id), format!("# {}", n)).clicked() {
                            self.post_f_channel = Some(*id);
                            close_popup(ui);
                        }
                    }
                });
                let members: Vec<(u64, String)> = self.data.members.iter().map(|m| (m.id, m.name.clone())).collect();
                let al = self.post_f_author.and_then(|id| members.iter().find(|m| m.0 == id)).map(|m| format!("@  {}", m.1)).unwrap_or_else(|| "@  Author".into());
                let ar = ui.add(egui::Button::new(al).corner_radius(13));
                popup(ui, &ar, ui.id().with("pf_au"), 180.0, |ui| {
                    if ui.selectable_label(self.post_f_author.is_none(), "Anyone").clicked() {
                        self.post_f_author = None;
                        close_popup(ui);
                    }
                    for (id, n) in &members {
                        if ui.selectable_label(self.post_f_author == Some(*id), n).clicked() {
                            self.post_f_author = Some(*id);
                            close_popup(ui);
                        }
                    }
                });
                let tl = self.post_f_type.clone().map(|t| format!("\u{25A6}  {}", t)).unwrap_or_else(|| "\u{25A6}  Type".into());
                let tr = ui.add(egui::Button::new(tl).corner_radius(13));
                popup(ui, &tr, ui.id().with("pf_ty"), 180.0, |ui| {
                    if ui.selectable_label(self.post_f_type.is_none(), "Any type").clicked() {
                        self.post_f_type = None;
                        close_popup(ui);
                    }
                    for k in ["Announcement", "Update", "Idea", "Question"] {
                        if ui.selectable_label(self.post_f_type.as_deref() == Some(k), k).clicked() {
                            self.post_f_type = Some(k.to_string());
                            close_popup(ui);
                        }
                    }
                });
                ui.add_space((cw - 520.0).max(10.0));
                ui.label(RichText::new("Sort:").color(t.text2));
                egui::ComboBox::from_id_salt("post_sort").selected_text(if self.post_sort_old { "Oldest" } else { "Newest" }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.post_sort_old, false, "Newest");
                    ui.selectable_value(&mut self.post_sort_old, true, "Oldest");
                });
            });
            ui.add_space(12.0);
            let mut posts: Vec<(String, u64, Message)> = vec![];
            for c in &self.data.channels {
                for m in &c.messages {
                    if m.post_title.is_some() {
                        posts.push((c.name.clone(), c.id, m.clone()));
                    }
                }
            }
            let me = self.data.me;
            let following: Vec<u64> = self.data.channels.iter().filter(|c| c.members.contains(&me)).map(|c| c.id).collect();
            posts.retain(|p| {
                (!self.post_following || following.contains(&p.1))
                    && self.post_f_channel.map(|c| c == p.1).unwrap_or(true)
                    && self.post_f_author.map(|a| a == p.2.author).unwrap_or(true)
                    && self.post_f_type.as_ref().map(|ty| (if p.2.post_type.is_empty() { "Announcement" } else { p.2.post_type.as_str() }) == ty).unwrap_or(true)
            });
            posts.sort_by_key(|p| p.2.ts);
            if !self.post_sort_old {
                posts.reverse();
            }
            if posts.is_empty() {
                ui.add_space(60.0);
                ui.vertical_centered(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                    ui.painter().rect_stroke(r.shrink(2.0), 14.0, Stroke::new(1.5, t.border), egui::StrokeKind::Inside);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "\u{1F4E2}", FontId::proportional(22.0), t.text2);
                    ui.add_space(10.0);
                    ui.label(RichText::new("There are no Posts yet!").size(18.0).color(t.text));
                });
            }
            for (cname, cid, m) in posts {
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                        ui.set_width(cw - 28.0);
                        ui.horizontal(|ui| {
                            if let Some(a) = self.data.member(m.author) {
                                let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                                paint_avatar(ui.painter(), r.center(), 12.0, a, t.card);
                                ui.label(RichText::new(&a.name).strong());
                            }
                            ui.label(RichText::new(format!("{} in # {} \u{2022} {}", if m.post_type.is_empty() { "Announcement" } else { &m.post_type }, cname, fmt_ago(m.ts))).small().color(t.text3));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.link("Open in channel").clicked() {
                                    self.section = Section::Chat;
                                    self.chat_open = Some(cid);
                                }
                            });
                        });
                        ui.label(RichText::new(m.post_title.clone().unwrap_or_default()).size(17.0));
                        if !m.text.is_empty() {
                            ui.add(egui::Label::new(&m.text).wrap());
                        }
                    });
                });
                ui.add_space(8.0);
            }
        });
    }

    // ---------------------------------------------------------------- meetings

    pub(super) fn meetings_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::SidePanel::left("meet_list").exact_width(260.0).resizable(false).frame(list_panel_frame(&t)).show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Meetings").size(16.0));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, &t, "+ Meeting").clicked() {
                        let id = self.data.new_id();
                        self.data.extra.meetings.push(Meeting { id, title: "New meeting".into(), date: today(), time: "10:00".into(), attendees: vec![self.data.me], agenda: String::new(), notes: String::new() });
                        self.meeting_open = Some(id);
                        self.dirty = true;
                    }
                });
            });
            ui.add_space(8.0);
            let mut ms = self.data.extra.meetings.clone();
            ms.sort_by_key(|m| (m.date, m.time.clone()));
            for m in &ms {
                let sel = self.meeting_open == Some(m.id);
                let w = ui.available_width();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 44.0), Sense::click());
                if sel {
                    ui.painter().rect_filled(r, 6.0, t.row_selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                ui.painter().with_clip_rect(r).text(r.left_top() + vec2(10.0, 14.0), Align2::LEFT_CENTER, &m.title, FontId::proportional(14.0), t.text);
                ui.painter().text(r.left_top() + vec2(10.0, 31.0), Align2::LEFT_CENTER, format!("{} \u{2022} {}", fmt_date(m.date), m.time), FontId::proportional(11.5), if m.date < today() { t.text3 } else { t.text2 });
                if resp.clicked() {
                    self.meeting_open = Some(m.id);
                }
            }
            if ms.is_empty() {
                ui.label(RichText::new("No meetings yet").color(t.text3));
            }
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::symmetric(32, 20))).show_inside(ui, |ui| {
            let Some(idx) = self.meeting_open.and_then(|id| self.data.extra.meetings.iter().position(|m| m.id == id)) else {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| ui.label(RichText::new("Pick a meeting or create one to keep the agenda and notes together.").color(t.text2)));
                return;
            };
            let members = self.data.members.clone();
            let mut delete = false;
            let mut make_tasks: Vec<String> = vec![];
            {
                let m = &mut self.data.extra.meetings[idx];
                let mut ch = false;
                ch |= ui.add(egui::TextEdit::singleline(&mut m.title).font(FontId::proportional(26.0)).frame(false).desired_width(f32::INFINITY)).changed();
                ui.horizontal(|ui| {
                    let mut d = Some(m.date);
                    let r = ui.button(format!("\u{1F4C5} {}", fmt_date(m.date)));
                    if let Some(true) = popup(ui, &r, ui.id().with("meet_date"), 380.0, |ui| date_menu(ui, &t, ui.id().with("meet_cal"), &mut d)) {
                        if let Some(d) = d {
                            m.date = d;
                            ch = true;
                        }
                    }
                    ui.label("at");
                    ch |= ui.add(egui::TextEdit::singleline(&mut m.time).desired_width(60.0)).changed();
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("Delete").color(t.danger)).frame(false)).clicked() {
                            delete = true;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.label(RichText::new("Attendees").color(t.text2));
                ui.horizontal_wrapped(|ui| {
                    for mem in &members {
                        let mut on = m.attendees.contains(&mem.id);
                        if ui.checkbox(&mut on, &mem.name).changed() {
                            if on {
                                m.attendees.push(mem.id);
                            } else {
                                m.attendees.retain(|a| *a != mem.id);
                            }
                            ch = true;
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label(RichText::new("Agenda").color(t.text2));
                ch |= ui.add(egui::TextEdit::multiline(&mut m.agenda).hint_text("What will you discuss?").desired_rows(4).desired_width(f32::INFINITY)).changed();
                ui.add_space(8.0);
                ui.label(RichText::new("Notes").color(t.text2));
                ch |= ui.add(egui::TextEdit::multiline(&mut m.notes).hint_text("Write notes. Lines starting with [ ] become action items.").desired_rows(10).desired_width(f32::INFINITY)).changed();
                let actions: Vec<String> = m.notes.lines().filter_map(|l| l.trim().strip_prefix("[ ]").map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect();
                if !actions.is_empty() && ui.button(format!("Create {} tasks from action items", actions.len())).clicked() {
                    make_tasks = actions;
                    m.notes = m.notes.replace("[ ]", "[x]");
                    ch = true;
                }
                if ch {
                    self.dirty = true;
                }
            }
            if !make_tasks.is_empty() {
                if let Some(list) = self.data.first_list() {
                    let who = self.data.extra.meetings[idx].attendees.clone();
                    for n in make_tasks {
                        self.actions.push(Action::Add { list, name: n, status: None, priority: 0, assignees: who.iter().take(1).copied().collect(), due: None, parent: None, open: false });
                    }
                    self.toast = Some(("Tasks created from the meeting notes".into(), now_ts()));
                }
            }
            if delete {
                self.data.extra.meetings.remove(idx);
                self.meeting_open = None;
                self.dirty = true;
            }
        });
    }

    // ---------------------------------------------------------------- goals

    pub(super) fn goal_progress(&self, g: &Goal) -> f32 {
        if g.targets.is_empty() {
            return 0.0;
        }
        let sum: f32 = g
            .targets
            .iter()
            .map(|tg| match &tg.kind {
                TargetKind::Number { start, target, current, .. } => {
                    if (target - start).abs() < f64::EPSILON {
                        1.0
                    } else {
                        (((current - start) / (target - start)) as f32).clamp(0.0, 1.0)
                    }
                }
                TargetKind::TrueFalse { done } => {
                    if *done {
                        1.0
                    } else {
                        0.0
                    }
                }
                TargetKind::Tasks { ids } => {
                    if ids.is_empty() {
                        0.0
                    } else {
                        ids.iter().filter(|id| self.task(**id).map(|x| self.data.is_closed(x)).unwrap_or(false)).count() as f32 / ids.len() as f32
                    }
                }
            })
            .sum();
        sum / g.targets.len() as f32
    }

    pub(super) fn goals_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Goals").size(20.0));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, &t, "+ New Goal").clicked() {
                            let id = self.data.new_id();
                            let color = STATUS_PALETTE[1 + self.data.extra.goals.len() % 13];
                            self.data.extra.goals.push(Goal { id, name: "New goal".into(), color, due: None, owner: self.data.me, targets: vec![] });
                            self.goal_open = Some(id);
                            self.dirty = true;
                        }
                    });
                });
                ui.label(RichText::new("Track progress with targets: numbers, done / not done, or tasks").color(t.text2));
                ui.add_space(12.0);
                let goals = self.data.extra.goals.clone();
                if goals.is_empty() {
                    ui.label(RichText::new("No goals yet.").color(t.text3));
                }
                for g in &goals {
                    let p = self.goal_progress(g);
                    let open = self.goal_open == Some(g.id);
                    let resp = egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, if open { t.text3 } else { t.border })).corner_radius(10).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 6.0, rgb(g.color));
                            ui.label(RichText::new(&g.name).size(16.0));
                            if let Some(d) = g.due {
                                ui.label(RichText::new(format!("Due {}", fmt_date(d))).small().color(date_color(d, p >= 1.0, &t)));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(format!("{:.0}%", p * 100.0)).strong());
                            });
                        });
                        ui.add(egui::ProgressBar::new(p).desired_height(6.0).fill(rgb(g.color)));
                    });
                    if ui.interact(resp.response.rect, ui.id().with(("goal", g.id)), Sense::click()).clicked() {
                        self.goal_open = if open { None } else { Some(g.id) };
                    }
                    if open {
                        self.goal_editor(ui, g.id);
                    }
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn goal_editor(&mut self, ui: &mut Ui, gid: u64) {
        let t = self.t();
        let Some(gi) = self.data.extra.goals.iter().position(|g| g.id == gid) else { return };
        let tasks: Vec<(u64, String)> = self.data.tasks.iter().filter(|x| x.parent.is_none()).map(|x| (x.id, x.name.clone())).collect();
        let closed: Vec<u64> = self.data.tasks.iter().filter(|x| self.data.is_closed(x)).map(|x| x.id).collect();
        let mut delete = false;
        egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 0, top: 6, bottom: 6 }).show(ui, |ui| {
            let g = &mut self.data.extra.goals[gi];
            let mut ch = false;
            ui.horizontal(|ui| {
                ui.label("Name");
                ch |= ui.add(egui::TextEdit::singleline(&mut g.name).desired_width(260.0)).changed();
                let mut d = g.due;
                let r = ui.button(g.due.map(|d| format!("Due {}", fmt_date(d))).unwrap_or_else(|| "\u{1F4C5} Due date".into()));
                if let Some(true) = popup(ui, &r, ui.id().with(("goal_due", gid)), 380.0, |ui| date_menu(ui, &t, ui.id().with(("goal_cal", gid)), &mut d)) {
                    g.due = d;
                    ch = true;
                }
                if ui.add(egui::Button::new(RichText::new("Delete goal").color(t.danger)).frame(false)).clicked() {
                    delete = true;
                }
            });
            let mut remove = None;
            for (i, tg) in g.targets.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ch |= ui.add(egui::TextEdit::singleline(&mut tg.name).desired_width(180.0)).changed();
                    match &mut tg.kind {
                        TargetKind::Number { start, target, current, unit } => {
                            ui.label("now");
                            ch |= ui.add(egui::DragValue::new(current).speed(1.0)).changed();
                            ui.label("of");
                            ch |= ui.add(egui::DragValue::new(target).speed(1.0)).changed();
                            ui.label("from");
                            ch |= ui.add(egui::DragValue::new(start).speed(1.0)).changed();
                            ch |= ui.add(egui::TextEdit::singleline(unit).hint_text("unit").desired_width(50.0)).changed();
                        }
                        TargetKind::TrueFalse { done } => {
                            ch |= ui.checkbox(done, "Done").changed();
                        }
                        TargetKind::Tasks { ids } => {
                            let n = ids.iter().filter(|id| closed.contains(id)).count();
                            ui.label(format!("{}/{} tasks done", n, ids.len()));
                            ui.menu_button("Pick tasks", |ui| {
                                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                    for (id, name) in &tasks {
                                        let mut on = ids.contains(id);
                                        if ui.checkbox(&mut on, name).changed() {
                                            if on {
                                                ids.push(*id);
                                            } else {
                                                ids.retain(|x| x != id);
                                            }
                                            ch = true;
                                        }
                                    }
                                });
                            });
                        }
                    }
                    if ui.add(egui::Button::new(RichText::new("\u{2716}").color(t.text3)).frame(false)).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                g.targets.remove(i);
                ch = true;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Add target:").color(t.text2));
                if ui.button("Number").clicked() {
                    g.targets.push(GoalTarget { name: "Target".into(), kind: TargetKind::Number { start: 0.0, target: 100.0, current: 0.0, unit: String::new() } });
                    ch = true;
                }
                if ui.button("Done / not done").clicked() {
                    g.targets.push(GoalTarget { name: "Target".into(), kind: TargetKind::TrueFalse { done: false } });
                    ch = true;
                }
                if ui.button("Tasks").clicked() {
                    g.targets.push(GoalTarget { name: "Finish tasks".into(), kind: TargetKind::Tasks { ids: vec![] } });
                    ch = true;
                }
            });
            if ch {
                self.dirty = true;
            }
        });
        if delete {
            self.data.extra.goals.remove(gi);
            self.goal_open = None;
            self.dirty = true;
        }
    }

    // ---------------------------------------------------------------- forms

    pub(super) fn form_detail(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::symmetric(32, 20))).show_inside(ui, |ui| {
            let Some(fi) = self.form_open.and_then(|id| self.data.extra.forms.iter().position(|f| f.id == id)) else {
                ui.label(RichText::new("Create a form to collect requests; each answer becomes a task.").color(t.text2));
                return;
            };
            let lists: Vec<(u64, String)> = self.data.spaces.iter().flat_map(|s| s.lists.iter().chain(s.folders.iter().flat_map(|f| f.lists.iter())).map(move |l| (l.id, format!("{} / {}", s.name, l.name)))).collect();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.form_mode_fill, false, "\u{1F4DD} Edit");
                ui.selectable_value(&mut self.form_mode_fill, true, "\u{1F441} Fill out");
            });
            ui.separator();
            if !self.form_mode_fill {
                let mut delete = false;
                let f = &mut self.data.extra.forms[fi];
                let mut ch = false;
                ch |= ui.add(egui::TextEdit::singleline(&mut f.name).font(FontId::proportional(24.0)).frame(false).desired_width(f32::INFINITY)).changed();
                ch |= ui.add(egui::TextEdit::multiline(&mut f.description).desired_rows(2).desired_width(f32::INFINITY)).changed();
                ui.horizontal(|ui| {
                    ui.label("Answers create tasks in");
                    let cur = lists.iter().find(|l| l.0 == f.list_id).map(|l| l.1.clone()).unwrap_or_else(|| "choose a list".into());
                    egui::ComboBox::from_id_salt("form_list_sel").selected_text(cur).show_ui(ui, |ui| {
                        for (id, n) in &lists {
                            ch |= ui.selectable_value(&mut f.list_id, *id, n).changed();
                        }
                    });
                });
                ui.add_space(8.0);
                ui.label(RichText::new("Questions").strong());
                let mut remove = None;
                for (i, fld) in f.fields.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ch |= ui.add(egui::TextEdit::singleline(&mut fld.label).desired_width(240.0)).changed();
                        egui::ComboBox::from_id_salt(("map", i)).selected_text(match fld.map.as_str() {
                            "name" => "Task name",
                            "description" => "Description",
                            "priority" => "Priority",
                            "due" => "Due date",
                            _ => "Added to description",
                        }).show_ui(ui, |ui| {
                            for (k, n) in [("name", "Task name"), ("description", "Description"), ("priority", "Priority"), ("due", "Due date"), ("text", "Added to description")] {
                                ch |= ui.selectable_value(&mut fld.map, k.to_string(), n).changed();
                            }
                        });
                        if ui.add(egui::Button::new(RichText::new("\u{2716}").color(t.text3)).frame(false)).clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    f.fields.remove(i);
                    ch = true;
                }
                ui.horizontal(|ui| {
                    if ui.button("+ Add question").clicked() {
                        f.fields.push(FormField { label: "Question".into(), map: "text".into() });
                        ch = true;
                    }
                    if ui.add(egui::Button::new(RichText::new("Delete form").color(t.danger)).frame(false)).clicked() {
                        delete = true;
                    }
                });
                if ch {
                    self.dirty = true;
                }
                if delete {
                    self.data.extra.forms.remove(fi);
                    self.form_open = None;
                    self.dirty = true;
                }
            } else {
                let f = self.data.extra.forms[fi].clone();
                egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::same(20)).show(ui, |ui| {
                    ui.set_width(ui.available_width().min(640.0));
                    ui.label(RichText::new(&f.name).size(22.0));
                    ui.label(RichText::new(&f.description).color(t.text2));
                    ui.add_space(10.0);
                    for (i, fld) in f.fields.iter().enumerate() {
                        ui.label(RichText::new(&fld.label).strong());
                        let v = self.form_fill.entry(i).or_default();
                        match fld.map.as_str() {
                            "priority" => {
                                egui::ComboBox::from_id_salt(("fp", i)).selected_text(if v.is_empty() { "Choose".to_string() } else { v.clone() }).show_ui(ui, |ui| {
                                    for p in ["Urgent", "High", "Normal", "Low"] {
                                        ui.selectable_value(v, p.to_string(), p);
                                    }
                                });
                            }
                            "due" => {
                                ui.add(egui::TextEdit::singleline(v).hint_text("YYYY-MM-DD").desired_width(140.0));
                            }
                            "description" | "text" => {
                                ui.add(egui::TextEdit::multiline(v).desired_rows(3).desired_width(f32::INFINITY));
                            }
                            _ => {
                                ui.add(egui::TextEdit::singleline(v).desired_width(f32::INFINITY));
                            }
                        }
                        ui.add_space(6.0);
                    }
                    if primary_button(ui, &t, "Submit").clicked() {
                        let mut name = String::new();
                        let mut desc = String::new();
                        let mut priority = 0u8;
                        let mut due = None;
                        for (i, fld) in f.fields.iter().enumerate() {
                            let v = self.form_fill.get(&i).cloned().unwrap_or_default();
                            match fld.map.as_str() {
                                "name" => name = v,
                                "description" => desc.push_str(&v),
                                "priority" => priority = PRIORITIES.iter().position(|p| p.label == v).unwrap_or(0) as u8,
                                "due" => due = NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d").ok(),
                                _ => {
                                    if !v.is_empty() {
                                        desc.push_str(&format!("\n{}: {}", fld.label, v));
                                    }
                                }
                            }
                        }
                        if name.trim().is_empty() {
                            name = format!("{} submission", f.name);
                        }
                        if self.data.space_of_list(f.list_id).is_some() {
                            self.actions.push(Action::Add { list: f.list_id, name, status: None, priority, assignees: vec![], due, parent: None, open: false });
                            self.pending_description = Some(desc.trim().to_string());
                            self.form_fill.clear();
                            self.toast = Some((format!("Submitted. A task was created in {}", self.data.list_name(f.list_id)), now_ts()));
                        }
                    }
                });
            }
        });
    }

    fn side_like_row(&self, ui: &mut Ui, sel: bool, icon: &str, label: &str) -> egui::Response {
        let t = self.t();
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
        name_it(&resp, label);
        if sel {
            ui.painter().rect_filled(r, 7.0, t.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 7.0, t.hover);
        }
        ui.painter().text(r.left_center() + vec2(14.0, 0.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
        ui.painter().with_clip_rect(r).text(r.left_center() + vec2(30.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), t.text);
        resp
    }

    // ---------------------------------------------------------------- whiteboards

    pub(super) fn whiteboard_canvas(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg).inner_margin(egui::Margin::same(12))).show_inside(ui, |ui| {
            let Some(wi) = self.wb_open.and_then(|id| self.data.extra.whiteboards.iter().position(|w| w.id == id)) else {
                ui.label(RichText::new("Create a whiteboard to sketch ideas and add sticky notes.").color(t.text2));
                return;
            };
            let mut delete = false;
            ui.horizontal(|ui| {
                if ui.add(egui::TextEdit::singleline(&mut self.data.extra.whiteboards[wi].name).font(FontId::proportional(18.0)).frame(false).desired_width(240.0)).changed() {
                    self.dirty = true;
                }
                ui.separator();
                ui.selectable_value(&mut self.wb_tool, 0, "\u{1F4DD} Pen");
                ui.selectable_value(&mut self.wb_tool, 1, "\u{1F4DD} Sticky note");
                ui.selectable_value(&mut self.wb_tool, 2, "\u{270B} Move notes");
                for c in [[32, 32, 32], [0xd3, 0x3d, 0x44], [8, 128, 234], [41, 151, 100], [0xf8, 0xae, 0x00], [0x5f, 0x55, 0xee]] {
                    let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                    ui.painter().circle_filled(r.center(), 8.0, rgb(c));
                    if self.wb_color == c {
                        ui.painter().circle_stroke(r.center(), 10.0, Stroke::new(1.5, t.text));
                    }
                    if resp.clicked() {
                        self.wb_color = c;
                    }
                }
                ui.separator();
                if ui.button("Undo").clicked() {
                    self.data.extra.whiteboards[wi].strokes.pop();
                    self.dirty = true;
                }
                if ui.button("Clear").clicked() {
                    self.data.extra.whiteboards[wi].strokes.clear();
                    self.data.extra.whiteboards[wi].notes.clear();
                    self.dirty = true;
                }
                if ui.add(egui::Button::new(RichText::new("Delete").color(t.danger)).frame(false)).clicked() {
                    delete = true;
                }
            });
            let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 8.0, t.column_bg);
            // dot grid
            let step = 24.0;
            let mut y = rect.top() + step;
            while y < rect.bottom() {
                let mut x = rect.left() + step;
                while x < rect.right() {
                    p.circle_filled(pos2(x, y), 1.0, t.border);
                    x += step;
                }
                y += step;
            }
            let origin = rect.min;
            let wb = &mut self.data.extra.whiteboards[wi];
            for s in &wb.strokes {
                let pts: Vec<egui::Pos2> = s.points.iter().map(|q| origin + vec2(q[0], q[1])).collect();
                p.add(egui::Shape::line(pts, Stroke::new(s.width, rgb(s.color))));
            }
            let mut drag_note = None;
            for (i, n) in wb.notes.iter_mut().enumerate() {
                let r = Rect::from_min_size(origin + vec2(n.pos[0], n.pos[1]), vec2(160.0, 120.0));
                p.rect_filled(r.translate(vec2(2.0, 3.0)), 4.0, Color32::from_black_alpha(25));
                p.rect_filled(r, 4.0, rgb(n.color).gamma_multiply(0.35).to_opaque());
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(8.0)));
                if child.add(egui::TextEdit::multiline(&mut n.text).frame(false).desired_width(144.0).desired_rows(5)).changed() {
                    self.dirty = true;
                }
                if self.wb_tool == 2 {
                    let h = ui.interact(r, ui.id().with(("note", i)), Sense::drag());
                    if h.dragged() {
                        drag_note = Some((i, h.drag_delta()));
                    }
                }
            }
            if let Some((i, d)) = drag_note {
                wb.notes[i].pos[0] += d.x;
                wb.notes[i].pos[1] += d.y;
                self.dirty = true;
            }
            if let Some(pos) = resp.interact_pointer_pos() {
                let local = pos - origin;
                match self.wb_tool {
                    0 => {
                        if resp.drag_started() {
                            wb.strokes.push(WbStroke { color: self.wb_color, width: 2.5, points: vec![[local.x, local.y]] });
                        } else if resp.dragged() {
                            if let Some(s) = wb.strokes.last_mut() {
                                s.points.push([local.x, local.y]);
                            }
                        }
                        if resp.drag_stopped() {
                            self.dirty = true;
                        }
                    }
                    1 => {
                        if resp.clicked() {
                            wb.notes.push(Sticky { pos: [local.x - 80.0, local.y - 60.0], text: String::new(), color: if self.wb_color == [32, 32, 32] { [0xf8, 0xae, 0x00] } else { self.wb_color } });
                            self.dirty = true;
                        }
                    }
                    _ => {}
                }
            }
            if delete {
                self.data.extra.whiteboards.remove(wi);
                self.wb_open = None;
                self.dirty = true;
            }
        });
    }

    // ---------------------------------------------------------------- reminders

    pub(super) fn reminders_block(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let now = now_ts();
        let mut rs: Vec<Reminder> = self.data.extra.reminders.iter().filter(|r| r.owner == me && !r.done).cloned().collect();
        if rs.is_empty() {
            return;
        }
        rs.sort_by_key(|r| r.due_ts);
        ui.label(RichText::new("Reminders").strong());
        for r in rs {
            ui.horizontal(|ui| {
                let mut done = false;
                if ui.checkbox(&mut done, "").changed() {
                    if let Some(x) = self.data.extra.reminders.iter_mut().find(|x| x.id == r.id) {
                        x.done = true;
                        self.dirty = true;
                    }
                }
                ui.label(&r.text);
                let when = chrono::DateTime::from_timestamp(r.due_ts, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %-I:%M %p").to_string()).unwrap_or_default();
                ui.label(RichText::new(when).small().color(if r.due_ts < now { t.danger } else { t.text3 }));
            });
        }
        ui.add_space(10.0);
    }

    pub(super) fn due_reminders(&self) -> usize {
        let now = now_ts();
        self.data.extra.reminders.iter().filter(|r| r.owner == self.data.me && !r.done && r.due_ts <= now).count()
    }

    // ---------------------------------------------------------------- AI

    pub(super) fn ai_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::Frame::new().inner_margin(egui::Margin::symmetric(40, 40)).show(ui, |ui| {
            ui.label(RichText::new("\u{2728} AI").size(24.0));
            ui.add_space(8.0);
            ui.label(RichText::new("Ask, Build, Create, Skills, Artifacts, Super Agents, AI Chats and the AI Notetaker need an AI service.").color(t.text2));
            ui.label(RichText::new("They will be connected together with the shared server. Everything else works now.").color(t.text2));
            ui.add_space(16.0);
            // read-only rather than disabled: a disabled field fades its hint
            // to an unreadable grey on the dark theme
            ui.add(egui::TextEdit::multiline(&mut String::new()).interactive(false).hint_text(RichText::new("Ask anything... (available with the server)").color(t.text3)).desired_rows(3).desired_width(600.0));
        });
    }

    // ---------------------------------------------------------------- overview view

    pub(super) fn overview_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let lists = self.data.lists_in(self.loc).unwrap_or_default();
        // top row: Customize (turn cards on and off) and Add card
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let hidden: Vec<String> = self.data.extra.hidden_cards.clone();
                let add = ui.add_enabled(!hidden.is_empty(), egui::Button::new("+ Add card"));
                popup(ui, &add, egui::Id::new("ov_add_card"), 180.0, |ui| {
                    for c in &hidden {
                        let b = ui.button(c);
                        name_it(&b, &format!("Add card {}", c));
                        if b.clicked() {
                            self.data.extra.hidden_cards.retain(|x| x != c);
                            self.dirty = true;
                            close_popup(ui);
                        }
                    }
                });
                let cz = ui.button("\u{2699}  Customize");
                popup(ui, &cz, egui::Id::new("ov_customize"), 200.0, |ui| {
                    ui.label(RichText::new("Cards").small().color(t.text3));
                    for c in OV_CARDS {
                        let mut on = !self.data.extra.hidden_cards.iter().any(|x| x == c);
                        let cb = ui.checkbox(&mut on, c);
                        name_it(&cb, &format!("Card {}", c));
                        if cb.changed() {
                            if on {
                                self.data.extra.hidden_cards.retain(|x| x != c);
                            } else {
                                self.data.extra.hidden_cards.push(c.to_string());
                            }
                            self.dirty = true;
                        }
                    }
                });
            });
        });
        let card_on = |s: &Self, c: &str| !s.data.extra.hidden_cards.iter().any(|x| x == c);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(8.0);
            let w = ui.available_width();
            let top: Vec<&str> = ["Recent", "Docs", "Bookmarks"].into_iter().filter(|c| card_on(self, c)).collect();
            let cw = (w - 12.0 * (top.len().max(1) as f32 - 1.0)) / top.len().max(1) as f32;
            ui.horizontal_top(|ui| {
                // Recent
                if card_on(self, "Recent") {
                self.ov_card(ui, cw, "Recent", |s, ui| {
                    let tt = s.t();
                    let items = s.recent_items(Some(&lists));
                    if items.is_empty() {
                        ui.label(RichText::new("Your recently opened items will appear here.").color(tt.text2));
                    }
                    for (kind, id, name, place) in items.into_iter().take(8) {
                        ui.horizontal(|ui| {
                            let glyph = match kind.as_str() {
                                "task" => "\u{2611}",
                                "doc" => "\u{1F4C4}",
                                _ => "\u{2630}",
                            };
                            ui.label(RichText::new(glyph).color(tt.text3));
                            if ui.link(&name).clicked() {
                                match kind.as_str() {
                                    "task" => s.actions.push(Action::Open(id)),
                                    "doc" => {
                                        s.doc_open = Some(id);
                                        s.section = Section::Docs;
                                    }
                                    _ => s.actions.push(Action::Go(Location::List(id))),
                                }
                            }
                            ui.label(RichText::new(format!("\u{2022} in {}", place)).small().color(tt.text3));
                        });
                    }
                });
                }
                // Docs
                let space = self.current_space();
                if card_on(self, "Docs") {
                self.ov_card(ui, cw, "Docs", |s, ui| {
                    let docs: Vec<(u64, String)> = s.data.docs.iter().filter(|d| (space.is_none() || d.space == space) && d.parent.is_none()).map(|d| (d.id, d.title.clone())).collect();
                    if docs.is_empty() {
                        ui.label(RichText::new("There are no Docs in this location yet.").color(s.t().text2));
                    }
                    for (id, n) in docs {
                        if ui.link(format!("\u{1F4C4} {}", n)).clicked() {
                            s.doc_open = Some(id);
                            s.section = Section::Docs;
                        }
                    }
                    ui.add_space(6.0);
                    if primary_button(ui, &s.t(), "Add a Doc").clicked() {
                        s.new_doc(space);
                    }
                });
                }
                // Bookmarks
                if card_on(self, "Bookmarks") {
                self.ov_card(ui, cw, "Bookmarks", |s, ui| {
                    let bms = s.data.extra.bookmarks.clone();
                    if bms.is_empty() {
                        ui.label(RichText::new("Bookmarks make it easy to save items or any URL from around the web.").color(s.t().text2));
                    }
                    let mut remove = None;
                    for b in &bms {
                        ui.horizontal(|ui| {
                            if ui.link(format!("\u{1F516} {}", b.title)).on_hover_text(&b.url).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(&b.url));
                            }
                            if ui.add(egui::Button::new(RichText::new("\u{2716}").size(10.0).color(s.t().text3)).frame(false)).clicked() {
                                remove = Some(b.id);
                            }
                        });
                    }
                    if let Some(id) = remove {
                        s.data.extra.bookmarks.retain(|b| b.id != id);
                        s.dirty = true;
                    }
                    ui.add_space(6.0);
                    if primary_button(ui, &s.t(), "Add Bookmark").clicked() {
                        s.modal = Some(Modal::Bookmark { title: String::new(), url: String::new() });
                    }
                });
                }
            });
            ui.add_space(12.0);
            // Folders
            let folders: Vec<(u64, String, usize)> = match self.loc {
                Location::Space(sid) => self.data.spaces.iter().filter(|s| s.id == sid).flat_map(|s| s.folders.iter().map(|f| (f.id, f.name.clone(), f.lists.len()))).collect(),
                _ => vec![],
            };
            if matches!(self.loc, Location::Space(_)) && card_on(self, "Folders") {
                self.ov_card(ui, w, "Folders", |s, ui| {
                    if folders.is_empty() {
                        ui.label(RichText::new("Add new Folder to your Space").color(s.t().text2));
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (id, n, c) in &folders {
                            if ui.add(egui::Button::new(format!("\u{1F4C1} {}  ({} lists)", n, c)).min_size(vec2(200.0, 40.0)).stroke(Stroke::new(1.0, s.t().border))).clicked() {
                                s.actions.push(Action::Go(Location::Folder(*id)));
                            }
                        }
                    });
                    if let Location::Space(sid) = s.loc {
                        ui.add_space(6.0);
                        if primary_button(ui, &s.t(), "Add Folder").clicked() {
                            s.actions.push(Action::NewFolder(sid));
                        }
                    }
                });
                ui.add_space(12.0);
            }
            // Lists: Name, Color, Progress, Start, End, Priority, Owner
            if card_on(self, "Lists") {
                self.ov_card(ui, w, "Lists", |s, ui| s.ov_lists(ui, w - 34.0, &lists));
            }
        });
    }

    fn ov_lists(&mut self, ui: &mut Ui, w: f32, lists: &[u64]) {
        let tt = self.t();
        const COLS: [(f32, &str); 7] = [(0.0, "Name"), (0.28, "Color"), (0.36, "Progress"), (0.55, "Start"), (0.66, "End"), (0.77, "Priority"), (0.88, "Owner")];
        let (hr, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
        for (x, h) in COLS {
            ui.painter().text(pos2(hr.left() + w * x, hr.center().y), Align2::LEFT_CENTER, h, FontId::proportional(12.5), tt.text2);
        }
        let cell = |r: Rect, i: usize| {
            let x0 = r.left() + w * COLS[i].0;
            let x1 = if i + 1 < COLS.len() { r.left() + w * COLS[i + 1].0 } else { r.right() };
            Rect::from_min_max(pos2(x0, r.top()), pos2(x1 - 6.0, r.bottom()))
        };
        for lid in lists {
            let Some(item) = self.data.list_item(*lid).cloned().filter(|l| !l.archived) else { continue };
            let total = self.data.tasks.iter().filter(|x| x.list_id == *lid && x.parent.is_none() && !x.archived).count();
            let done = self.data.tasks.iter().filter(|x| x.list_id == *lid && x.parent.is_none() && !x.archived && self.data.is_closed(x)).count();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
            name_it(&resp, &format!("Overview list {}", item.name));
            if resp.hovered() {
                ui.painter().rect_filled(r, 5.0, tt.hover);
            }
            let p = ui.painter().clone();
            p.with_clip_rect(cell(r, 0)).text(r.left_center() + vec2(4.0, 0.0), Align2::LEFT_CENTER, &item.name, FontId::proportional(14.0), tt.text);
            let mut edited = item.clone();
            // color
            let c0 = cell(r, 1);
            let cr = ui.interact(c0, ui.id().with(("ov_color", lid)), Sense::click());
            name_it(&cr, &format!("Color of {}", item.name));
            match item.color {
                Some(c) => {
                    p.circle_filled(pos2(c0.left() + 8.0, c0.center().y), 6.0, rgb(c));
                }
                None => {
                    p.circle_stroke(pos2(c0.left() + 8.0, c0.center().y), 6.0, Stroke::new(1.0, tt.text3));
                }
            }
            popup(ui, &cr, ui.id().with(("ov_color_p", lid)), 160.0, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for c in STATUS_PALETTE.iter().skip(1) {
                        let (b, br) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                        ui.painter().circle_filled(b.center(), 8.0, rgb(*c));
                        if br.clicked() {
                            edited.color = Some(*c);
                            close_popup(ui);
                        }
                    }
                });
                if ui.small_button("No color").clicked() {
                    edited.color = None;
                    close_popup(ui);
                }
            });
            // progress
            let c2 = cell(r, 2);
            let bar = Rect::from_min_size(pos2(c2.left(), c2.center().y - 3.0), vec2((c2.width() - 40.0).max(20.0), 6.0));
            p.rect_filled(bar, 3.0, tt.hover);
            if total > 0 {
                p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * done as f32 / total as f32, 6.0)), 3.0, rgb(COMPLETE_COLOR));
            }
            p.text(pos2(bar.right() + 6.0, c2.center().y), Align2::LEFT_CENTER, format!("{}/{}", done, total), FontId::proportional(12.0), tt.text2);
            // start / end
            for (i, which) in [(3usize, 0u8), (4, 1)] {
                let c = cell(r, i);
                let dr = ui.interact(c, ui.id().with(("ov_date", lid, which)), Sense::click());
                name_it(&dr, &format!("{} of {}", if which == 0 { "Start" } else { "End" }, item.name));
                let v = if which == 0 { item.start } else { item.end };
                match v {
                    Some(d) => {
                        p.text(c.left_center(), Align2::LEFT_CENTER, fmt_date(d), FontId::proportional(13.0), tt.text);
                    }
                    None => {
                        p.text(c.left_center(), Align2::LEFT_CENTER, "\u{2013}", FontId::proportional(13.0), tt.text3);
                    }
                }
                let mut val = v;
                if let Some(true) = popup(ui, &dr, ui.id().with(("ov_date_p", lid, which)), 300.0, |ui| date_menu(ui, &tt, ui.id().with(("ov_cal", lid, which)), &mut val)) {
                    if which == 0 {
                        edited.start = val;
                    } else {
                        edited.end = val;
                    }
                }
            }
            // priority
            let c5 = cell(r, 5);
            let prr = ui.interact(c5, ui.id().with(("ov_pr", lid)), Sense::click());
            name_it(&prr, &format!("Priority of {}", item.name));
            if item.priority > 0 {
                paint_flag(&p, c5.left_center() + vec2(4.0, 0.0), priority_color(item.priority, &tt), 12.0);
                p.text(c5.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, PRIORITIES[item.priority as usize].label, FontId::proportional(13.0), tt.text);
            } else {
                p.text(c5.left_center(), Align2::LEFT_CENTER, "\u{2013}", FontId::proportional(13.0), tt.text3);
            }
            let mut pv = item.priority;
            if let Some(true) = popup(ui, &prr, ui.id().with(("ov_pr_p", lid)), 180.0, |ui| priority_menu(ui, &tt, &mut pv)) {
                edited.priority = pv;
            }
            // owner
            let c6 = cell(r, 6);
            let orr = ui.interact(c6, ui.id().with(("ov_owner", lid)), Sense::click());
            name_it(&orr, &format!("Owner of {}", item.name));
            match item.owner.and_then(|o| self.data.member(o)) {
                Some(m) => paint_avatar(&p, c6.left_center() + vec2(10.0, 0.0), 10.0, m, tt.card),
                None => paint_empty_avatar(&p, c6.left_center() + vec2(10.0, 0.0), 10.0, tt.text3),
            }
            let members = self.data.members.clone();
            popup(ui, &orr, ui.id().with(("ov_owner_p", lid)), 200.0, |ui| {
                for m in &members {
                    let ring = tt.card;
                    if menu_row(ui, &tt, &m.name, item.owner == Some(m.id), |p, c| paint_avatar(p, c, 8.0, m, ring)).clicked() {
                        edited.owner = if item.owner == Some(m.id) { None } else { Some(m.id) };
                        close_popup(ui);
                    }
                }
            });
            let any_cell = [cr.hovered(), prr.hovered(), orr.hovered()].iter().any(|x| *x) || [c0, cell(r, 3), cell(r, 4), c5, c6].iter().any(|c| ui.rect_contains_pointer(*c));
            let changed = edited.color != item.color || edited.start != item.start || edited.end != item.end || edited.priority != item.priority || edited.owner != item.owner;
            if changed {
                if let Some(l) = self.data.list_item_mut(*lid) {
                    *l = edited;
                }
                self.dirty = true;
            } else if resp.clicked() && !any_cell {
                self.actions.push(Action::Go(Location::List(*lid)));
            }
        }
        ui.add_space(6.0);
        if ui.add(egui::Button::new(RichText::new("+ New List").color(tt.text2)).frame(false)).clicked() {
            let space = self.current_space();
            let folder = if let Location::Folder(f) = self.loc { Some(f) } else { None };
            self.open_new_list(space, folder);
        }
    }

    fn ov_card(&mut self, ui: &mut Ui, w: f32, title: &str, add: impl FnOnce(&mut Self, &mut Ui)) {
        let t = self.t();
        egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(w - 34.0);
                ui.set_min_height(150.0);
                ui.label(RichText::new(title).size(15.0));
                ui.add_space(8.0);
                add(self, ui);
            });
        });
    }
}
