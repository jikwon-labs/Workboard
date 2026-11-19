// The Posts page.

use super::*;

// ---------------------------------------------------------------- posts

impl App {
    pub(crate) fn posts_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.page_bar(ui, tr("Posts"), |s, ui| {
            let tt = s.t();
            if primary_button(ui, &tt, tr("New Post")).clicked() {
                s.post.title.clear();
                s.post.body.clear();
            }
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
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
                    egui::Frame::new()
                        .stroke(Stroke::new(1.0_f32, t.border))
                        .corner_radius(12)
                        .inner_margin(egui::Margin::symmetric(16, 10))
                        .show(ui, |ui| {
                            ui.set_width(cw - 34.0);
                            // the frame sits in a horizontal row; stack its contents top to bottom
                            ui.vertical(|ui| {
                                let chans: Vec<(u64, String)> = self
                                    .data
                                    .channels
                                    .iter()
                                    .filter(|c| c.kind == ChannelKind::Channel && self.can_see(c))
                                    .map(|c| (c.id, c.name.clone()))
                                    .collect();
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(tr("Channel:")).color(t.text2));
                                    let cur = chans
                                        .iter()
                                        .find(|c| c.0 == self.post.channel)
                                        .map(|c| format!("# {}", c.1))
                                        .unwrap_or_else(|| tr("Select Channel").into());
                                    egui::ComboBox::from_id_salt("post_ch")
                                        .selected_text(cur)
                                        .show_ui(ui, |ui| {
                                            for (id, n) in &chans {
                                                ui.selectable_value(
                                                    &mut self.post.channel,
                                                    *id,
                                                    format!("# {}", n),
                                                );
                                            }
                                        });
                                });
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(tr("Type:")).color(t.text2));
                                    egui::ComboBox::from_id_salt("post_type")
                                        .selected_text(tr(&self.post.kind))
                                        .show_ui(ui, |ui| {
                                            for k in ["Announcement", "Update", "Idea", "Question"]
                                            {
                                                ui.selectable_value(
                                                    &mut self.post.kind,
                                                    k.to_string(),
                                                    tr(k),
                                                );
                                            }
                                        });
                                });
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(tr("Title:")).color(t.text2));
                                    // frameless like the body below, so a hint shows where to type
                                    let r = ui.add(
                                        egui::TextEdit::singleline(&mut self.post.title)
                                            .hint_text(tr("Add a title"))
                                            .frame(false)
                                            .desired_width(f32::INFINITY),
                                    );
                                    name_field(&r, "Post title");
                                });
                                ui.separator();
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.post.body)
                                        .hint_text(tr("Share an announcement..."))
                                        .frame(false)
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                                ui.horizontal(|ui| {
                                    // "@" is a character, "+" two lines; the others line icons
                                    for (g, tip) in [
                                        ("+", "Create task"),
                                        ("@", "Mention someone"),
                                        ("smile", "Emoji"),
                                        ("task", "Create task"),
                                    ] {
                                        let b = if g == "+" {
                                            // drawn as lines so it sits in
                                            // the middle (the character sat low)
                                            let b = ui.add(ghost(egui::Button::new(
                                                crate::app::icons::sign_text(false),
                                            )));
                                            crate::app::icons::paint_sign(
                                                ui,
                                                &b,
                                                false,
                                                Some(t.text2),
                                            );
                                            b
                                        } else if g.len() == 1 {
                                            ui.add(ghost(egui::Button::new(
                                                RichText::new(g).color(t.text2),
                                            )))
                                        } else {
                                            icon_button(ui, &t, g, "", tip, false)
                                        };
                                        if b.on_hover_text(tr(tip)).clicked() {
                                            match g {
                                                "@" => self.post.body.push('@'),
                                                "smile" => self.post.body.push_str("\u{1F44D}"),
                                                _ => self.open_quick_add(None),
                                            }
                                        }
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let ready = !self.post.title.trim().is_empty()
                                            && self.post.channel != 0;
                                        let b =
                                            Tint(
                                                egui::Button::new(RichText::new(tr("Post")).color(
                                                    if ready { t.on_primary } else { t.text3 },
                                                ))
                                                .fill(if ready { t.primary } else { t.hover })
                                                .corner_radius(6)
                                                .min_size(vec2(56.0, 30.0)),
                                            );
                                        if ui
                                            .add(b)
                                            .on_hover_text(if self.post.channel == 0 {
                                                tr("Select a channel first")
                                            } else {
                                                ""
                                            })
                                            .clicked()
                                            && ready
                                        {
                                            let id = self.data.new_id();
                                            let me = self.data.me;
                                            let m = Message {
                                                edited: None,
                                                imported: None,
                                                preview: None,
                                                forwarded: None,
                                                thread_of: None,
                                                bot: None,
                                                mentions: self
                                                    .data
                                                    .mentions_in(self.post.body.trim()),
                                                attachments: vec![],
                                                id,
                                                author: me,
                                                text: self.post.body.trim().to_string(),
                                                ts: now_ts(),
                                                reactions: vec![],
                                                replies: vec![],
                                                saved: false,
                                                task: None,
                                                post_title: Some(
                                                    self.post.title.trim().to_string(),
                                                ),
                                                post_type: self.post.kind.clone(),
                                                pinned: false,
                                                call: None,
                                            };
                                            if let Some(c) = self
                                                .data
                                                .channels
                                                .iter_mut()
                                                .find(|c| c.id == self.post.channel)
                                            {
                                                c.messages.push(m);
                                                c.last_read = now_ts();
                                            }
                                            self.post.title.clear();
                                            self.post.body.clear();
                                            self.dirty = true;
                                        }
                                    });
                                });
                            });
                        });
                });
                ui.add_space(20.0);
                let r = ui.available_rect_before_wrap();
                ui.painter()
                    .hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(pad);
                    let fol = Tint(
                        egui::Button::new(RichText::new(tr("Channels I'm Following")).color(
                            if self.post.following {
                                t.accent_text
                            } else {
                                t.text2
                            },
                        ))
                        .fill(if self.post.following {
                            t.accent_bg
                        } else {
                            t.bg
                        })
                        .corner_radius(13),
                    );
                    if ui.add(fol).clicked() {
                        self.post.following = !self.post.following;
                    }
                    let chans: Vec<(u64, String)> = self
                        .data
                        .channels
                        .iter()
                        .filter(|c| c.kind == ChannelKind::Channel && self.can_see(c))
                        .map(|c| (c.id, c.name.clone()))
                        .collect();
                    let cl = self
                        .post
                        .f_channel
                        .and_then(|id| chans.iter().find(|c| c.0 == id))
                        .map(|c| format!("#  {}", c.1))
                        .unwrap_or_else(|| tr("#  Channel").into());
                    let cr = ui.add(egui::Button::new(cl).corner_radius(13));
                    popup(ui, &cr, ui.id().with("pf_ch"), 180.0, |ui| {
                        if ui
                            .selectable_label(self.post.f_channel.is_none(), tr("All channels"))
                            .clicked()
                        {
                            self.post.f_channel = None;
                            close_popup(ui);
                        }
                        for (id, n) in &chans {
                            if ui
                                .selectable_label(
                                    self.post.f_channel == Some(*id),
                                    format!("# {}", n),
                                )
                                .clicked()
                            {
                                self.post.f_channel = Some(*id);
                                close_popup(ui);
                            }
                        }
                    });
                    let members: Vec<(u64, String)> = self
                        .data
                        .members
                        .iter()
                        .map(|m| (m.id, m.shown().to_string()))
                        .collect();
                    let al = self
                        .post
                        .f_author
                        .and_then(|id| members.iter().find(|m| m.0 == id))
                        .map(|m| format!("@  {}", m.1))
                        .unwrap_or_else(|| tr("@  Author").into());
                    let ar = ui.add(egui::Button::new(al).corner_radius(13));
                    popup(ui, &ar, ui.id().with("pf_au"), 180.0, |ui| {
                        if ui
                            .selectable_label(self.post.f_author.is_none(), tr("Anyone"))
                            .clicked()
                        {
                            self.post.f_author = None;
                            close_popup(ui);
                        }
                        for (id, n) in &members {
                            if ui
                                .selectable_label(self.post.f_author == Some(*id), n)
                                .clicked()
                            {
                                self.post.f_author = Some(*id);
                                close_popup(ui);
                            }
                        }
                    });
                    let tl = self
                        .post
                        .f_type
                        .clone()
                        .map(|t| tr(&t).to_string())
                        .unwrap_or_else(|| tr("Type").into());
                    let tyr = icon_button(ui, &t, "shapes", &tl, "Post type filter", true);
                    popup(ui, &tyr, ui.id().with("pf_ty"), 180.0, |ui| {
                        if ui
                            .selectable_label(self.post.f_type.is_none(), tr("Any type"))
                            .clicked()
                        {
                            self.post.f_type = None;
                            close_popup(ui);
                        }
                        for k in ["Announcement", "Update", "Idea", "Question"] {
                            if ui
                                .selectable_label(self.post.f_type.as_deref() == Some(k), tr(k))
                                .clicked()
                            {
                                self.post.f_type = Some(k.to_string());
                                close_popup(ui);
                            }
                        }
                    });
                    ui.add_space((cw - 520.0).max(10.0));
                    ui.label(RichText::new(tr("Sort:")).color(t.text2));
                    egui::ComboBox::from_id_salt("post_sort")
                        .selected_text(if self.post.sort_old {
                            tr("Oldest")
                        } else {
                            tr("Newest")
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.post.sort_old, false, tr("Newest"));
                            ui.selectable_value(&mut self.post.sort_old, true, tr("Oldest"));
                        });
                });
                ui.add_space(12.0);
                let mut posts: Vec<(String, u64, Message)> = vec![];
                for c in self.data.channels.iter().filter(|c| self.can_see(c)) {
                    for m in &c.messages {
                        if m.post_title.is_some() {
                            posts.push((c.name.clone(), c.id, m.clone()));
                        }
                    }
                }
                let me = self.data.me;
                let following: Vec<u64> = self
                    .data
                    .channels
                    .iter()
                    .filter(|c| c.members.contains(&me))
                    .map(|c| c.id)
                    .collect();
                posts.retain(|p| {
                    (!self.post.following || following.contains(&p.1))
                        && self.post.f_channel.map(|c| c == p.1).unwrap_or(true)
                        && self.post.f_author.map(|a| a == p.2.author).unwrap_or(true)
                        && self
                            .post
                            .f_type
                            .as_ref()
                            .map(|ty| {
                                (if p.2.post_type.is_empty() {
                                    "Announcement"
                                } else {
                                    p.2.post_type.as_str()
                                }) == ty
                            })
                            .unwrap_or(true)
                });
                posts.sort_by_key(|p| p.2.ts);
                if !self.post.sort_old {
                    posts.reverse();
                }
                if posts.is_empty() {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                        ui.painter().rect_stroke(
                            r.shrink(2.0),
                            14.0,
                            Stroke::new(1.5_f32, t.border),
                            egui::StrokeKind::Inside,
                        );
                        line_icon_scaled(ui.painter(), "megaphone", r.center(), t.text2, 1.6);
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(tr("There are no Posts yet!"))
                                .size(18.0)
                                .color(t.text),
                        );
                    });
                }
                for (cname, cid, m) in posts {
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        egui::Frame::new()
                            .fill(t.card)
                            .stroke(Stroke::new(1.0_f32, t.border))
                            .corner_radius(10)
                            .inner_margin(egui::Margin::same(14))
                            .show(ui, |ui| {
                                ui.set_width(cw - 28.0);
                                ui.horizontal(|ui| {
                                    if let Some(a) = self.data.member(m.author) {
                                        let (r, _) = ui
                                            .allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                                        paint_avatar(ui.painter(), r.center(), 12.0, a, t.card);
                                        ui.label(RichText::new(a.shown()).strong());
                                    }
                                    ui.label(
                                        RichText::new(trf!(
                                            "{} in # {} \u{2022} {}",
                                            tr(if m.post_type.is_empty() {
                                                "Announcement"
                                            } else {
                                                &m.post_type
                                            }),
                                            cname,
                                            fmt_ago(m.ts)
                                        ))
                                        .small()
                                        .color(t.text3),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.link(tr("Open in channel")).clicked() {
                                            self.section = Section::Chat;
                                            self.chat.open = Some(cid);
                                        }
                                    });
                                });
                                ui.label(
                                    RichText::new(m.post_title.clone().unwrap_or_default())
                                        .size(17.0),
                                );
                                if !m.text.is_empty() {
                                    ui.add(egui::Label::new(&m.text).wrap());
                                }
                            });
                    });
                    ui.add_space(8.0);
                }
            });
    }
}
