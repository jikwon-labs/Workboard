// The Posts page.

use super::*;

// ---------------------------------------------------------------- posts

/// The post's text box and its @ list (egui ids).
const POST_TEXT: &str = "post_text";
const POST_MENTIONS: &str = "post_mentions";

impl App {
    /// Another workspace is open: the Posts page's picked channel, its
    /// channel and author filters, the people picked from its @ list and
    /// the profile shown named this one's channels and people (the same ids
    /// may name others there). What is being written stays.
    pub(crate) fn posts_in_another_workspace(&mut self) {
        self.post.channel = 0;
        self.post.f_channel = None;
        self.post.f_author = None;
        self.post.picked.clear();
        self.chat.profile = None;
    }

    /// The text box's cursor: where it is (the end when the box never had
    /// it), and the selection it replaces, as char indexes.
    fn post_cursor(&self, ctx: &egui::Context) -> (usize, usize) {
        let n = self.post.body.chars().count();
        egui::TextEdit::load_state(ctx, egui::Id::new(POST_TEXT))
            .and_then(|st| st.cursor.char_range())
            .map(|r| {
                let [a, b] = r.sorted();
                (a.index.min(n), b.index.min(n))
            })
            .unwrap_or((n, n))
    }

    /// The character just typed in the post's text is an "@" starting a word.
    fn typed_at_sign(&self, ctx: &egui::Context) -> bool {
        let (at, _) = self.post_cursor(ctx);
        let chars: Vec<char> = self.post.body.chars().collect();
        at > 0 && chars[at - 1] == '@' && (at == 1 || chars[at - 2].is_whitespace())
    }

    /// Put `s` in the post's text at the cursor, in place of what is
    /// selected; a picked @name takes the place of an "@" typed just before
    /// and starts a word. The box gets the keyboard back, the cursor after it.
    fn insert_in_post(&mut self, ctx: &egui::Context, s: &str) {
        let (a, b) = self.post_cursor(ctx);
        let mut chars: Vec<char> = self.post.body.chars().collect();
        chars.drain(a..b);
        let mut at = a;
        let mut s = s.to_string();
        if s.starts_with('@') {
            if at > 0 && chars[at - 1] == '@' {
                chars.remove(at - 1);
                at -= 1;
            }
            if at > 0 && !chars[at - 1].is_whitespace() {
                s.insert(0, ' ');
            }
        }
        let added: Vec<char> = s.chars().collect();
        let end = at + added.len();
        chars.splice(at..at, added);
        self.post.body = chars.into_iter().collect();
        let id = egui::Id::new(POST_TEXT);
        let mut st = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(end),
        )));
        st.store(ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
    }

    pub(crate) fn posts_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.page_bar(ui, tr("Posts"), |s, ui| {
            let tt = s.t();
            // to the title, keeping what is being written
            if primary_button(ui, &tt, tr("New Post")).clicked() {
                s.post.focus_title = true;
            }
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let w = ui.available_width();
                let cw = 960.0f32.min(w - 60.0);
                let pad = ((w - cw) / 2.0).max(0.0);
                ui.add_space(20.0);
                self.post_composer(ui, pad, cw);
                ui.add_space(20.0);
                let r = ui.available_rect_before_wrap();
                ui.painter()
                    .hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
                ui.add_space(16.0);
                self.post_filters(ui, pad, cw);
                ui.add_space(12.0);
                let (posts, any) = self.posts_shown();
                if posts.is_empty() {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
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
                            // posts there, all of them filtered out
                            let say = if any {
                                tr("No posts match")
                            } else {
                                tr("There are no Posts yet!")
                            };
                            ui.label(RichText::new(say).size(18.0).color(t.text));
                        });
                    });
                }
                for (cname, cid, m) in posts {
                    self.post_card(ui, pad, cw, &cname, cid, m);
                    ui.add_space(8.0);
                }
            });
    }

    /// The box for writing a post: channel, type, title, text and Post.
    fn post_composer(&mut self, ui: &mut Ui, pad: f32, cw: f32) {
        let t = self.t();
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
                            let pick = egui::ComboBox::from_id_salt("post_ch")
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
                            name_it(&pick.response, "Post channel");
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("Type:")).color(t.text2));
                            let pick = egui::ComboBox::from_id_salt("post_type")
                                .selected_text(tr(&self.post.kind))
                                .show_ui(ui, |ui| {
                                    for k in ["Announcement", "Update", "Idea", "Question"] {
                                        ui.selectable_value(
                                            &mut self.post.kind,
                                            k.to_string(),
                                            tr(k),
                                        );
                                    }
                                });
                            name_it(&pick.response, "Post type");
                        });
                        ui.separator();
                        let title_focus = ui
                            .horizontal(|ui| {
                                ui.label(RichText::new(tr("Title:")).color(t.text2));
                                // frameless like the body below, so a hint shows where to type
                                let r = ui.add(
                                    egui::TextEdit::singleline(&mut self.post.title)
                                        .hint_text(tr("Add a title"))
                                        .frame(false)
                                        .desired_width(f32::INFINITY),
                                );
                                name_field(&r, "Post title");
                                if std::mem::take(&mut self.post.focus_title) {
                                    r.request_focus();
                                    r.scroll_to_me(Some(Align::Center));
                                }
                                r.has_focus()
                            })
                            .inner;
                        ui.separator();
                        let names = self.mention_names();
                        let mut lay = |ui: &Ui, s: &str, w: f32| {
                            crate::app::chat::mention_layout(ui, s, w, &names, &t)
                        };
                        let r = ui.add(
                            egui::TextEdit::multiline(&mut self.post.body)
                                .id(egui::Id::new(POST_TEXT))
                                .hint_text(tr("Share an announcement..."))
                                .frame(false)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY)
                                .layouter(&mut lay),
                        );
                        name_field(&r, "Post text");
                        // "@" typed at the start of a word lists the people
                        if r.changed() && self.typed_at_sign(ui.ctx()) {
                            ui.memory_mut(|m| m.open_popup(egui::Id::new(POST_MENTIONS)));
                        }
                        // Cmd+Enter in the title or the text posts, as
                        // it sends a comment (Enter is a new line)
                        let cmd_enter = (title_focus || r.has_focus())
                            && ui.input_mut(|i| {
                                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                            });
                        ui.horizontal(|ui| {
                            // as in a channel's box: "@" lists the people to
                            // mention, the smile the emoji, put in at the cursor
                            let mut insert: Option<String> = None;
                            let at = ui
                                .add(ghost(egui::Button::new(RichText::new("@").color(t.text2))))
                                .on_hover_text(tr("Mention someone"));
                            name_it(&at, "Mention someone");
                            let me = self.data.me;
                            let people: Vec<Member> = self
                                .data
                                .members
                                .iter()
                                .filter(|m| !m.invited && m.id != me)
                                .cloned()
                                .collect();
                            let teams = self.team_names();
                            popup(ui, &at, egui::Id::new(POST_MENTIONS), 210.0, |ui| {
                                if people.is_empty() {
                                    ui.label(
                                        RichText::new(tr("No one else here yet")).color(t.text3),
                                    );
                                }
                                for m in &people {
                                    if menu_row(ui, &t, m.shown(), false, |p, c| {
                                        paint_avatar(p, c, 8.0, m, t.card)
                                    })
                                    .clicked()
                                    {
                                        insert = Some(format!("@{} ", m.shown()));
                                        self.post.picked.push(m.id);
                                        close_popup(ui);
                                    }
                                }
                                if let Some(g) =
                                    crate::app::chat::group_mention_rows(ui, &t, &teams)
                                {
                                    insert = Some(g);
                                    close_popup(ui);
                                }
                            });
                            let em = icon_button(ui, &t, "smile", "", "Emoji", false)
                                .on_hover_text(tr("Emoji"));
                            popup(ui, &em, egui::Id::new("post_emoji"), 248.0, |ui| {
                                if let Some(e) = crate::app::chat::emoji_grid(ui) {
                                    insert = Some(e.to_string());
                                    close_popup(ui);
                                }
                            });
                            if icon_button(ui, &t, "task", "", "Create task", false)
                                .on_hover_text(tr("Create task"))
                                .clicked()
                            {
                                self.open_quick_add(None);
                            }
                            if let Some(text) = insert {
                                self.insert_in_post(ui.ctx(), &text);
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                // a channel still listed: not one deleted
                                // or left since it was picked
                                let picked = chans.iter().any(|c| c.0 == self.post.channel);
                                let ready = !self.post.title.trim().is_empty() && picked;
                                let b = Tint(
                                    egui::Button::new(RichText::new(tr("Post")).color(if ready {
                                        t.on_primary
                                    } else {
                                        t.text3
                                    }))
                                    .fill(if ready { t.primary } else { t.hover })
                                    .corner_radius(6)
                                    .min_size(vec2(56.0, 30.0)),
                                );
                                let mut sent = ui.add(b);
                                // (no tooltip at all once a channel is picked)
                                if !picked {
                                    sent = sent.on_hover_text(tr("Select a channel first"));
                                }
                                if (sent.clicked() || cmd_enter) && ready {
                                    let title = self.post.title.trim().to_string();
                                    let kind = self.post.kind.clone();
                                    let text = self.post.body.trim().to_string();
                                    let picked = std::mem::take(&mut self.post.picked);
                                    self.add_post(
                                        self.post.channel,
                                        title,
                                        &kind,
                                        text,
                                        vec![],
                                        &picked,
                                    );
                                    self.post.title.clear();
                                    self.post.body.clear();
                                }
                            });
                        });
                    });
                });
        });
    }

    /// The filters over the posts (following, channel, author, type) and
    /// the sort menu, beside them or under them when there is no room.
    fn post_filters(&mut self, ui: &mut Ui, pad: f32, cw: f32) {
        let t = self.t();
        let mut sort_below = false;
        ui.horizontal(|ui| {
            let row_left = ui.cursor().left();
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
            // a channel deleted or left since it was picked: the
            // filter lets go (the button already reads "#  Channel")
            if self
                .post
                .f_channel
                .is_some_and(|id| !chans.iter().any(|c| c.0 == id))
            {
                self.post.f_channel = None;
            }
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
                        .selectable_label(self.post.f_channel == Some(*id), format!("# {}", n))
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
            // the same for someone no longer in the workspace
            if self
                .post
                .f_author
                .is_some_and(|id| !members.iter().any(|m| m.0 == id))
            {
                self.post.f_author = None;
            }
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
            // the sort menu ends where the post cards below end,
            // however wide the filters before it are; with no room
            // beside them (a narrow window, a long channel name) it
            // goes under them rather than on top of them
            let room = row_left + pad + cw - ui.cursor().left();
            if room >= sort_menu_width(ui) {
                let h = ui.min_rect().height();
                ui.allocate_ui_with_layout(
                    vec2(room, h),
                    Layout::right_to_left(Align::Center),
                    |ui| sort_menu(ui, &mut self.post.sort_old, t.text2),
                );
            } else {
                sort_below = true;
            }
        });
        if sort_below {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let h = ui.spacing().interact_size.y;
                ui.allocate_ui_with_layout(
                    vec2(pad + cw, h),
                    Layout::right_to_left(Align::Center),
                    |ui| sort_menu(ui, &mut self.post.sort_old, t.text2),
                );
            });
        }
    }

    /// The posts the filters let through, sorted, with their channel's name
    /// and id; and whether there are any posts at all.
    fn posts_shown(&self) -> (Vec<(String, u64, Message)>, bool) {
        let mut posts: Vec<(String, u64, Message)> = vec![];
        for c in self.data.channels.iter().filter(|c| self.can_see(c)) {
            for m in &c.messages {
                if m.post_title.is_some() {
                    posts.push((self.channel_label(c), c.id, m.clone()));
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
        let any = !posts.is_empty();
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
        (posts, any)
    }

    /// One post's card.
    fn post_card(&mut self, ui: &mut Ui, pad: f32, cw: f32, cname: &str, cid: u64, m: Message) {
        let t = self.t();
        ui.horizontal(|ui| {
            ui.add_space(pad);
            egui::Frame::new()
                .fill(t.card)
                .stroke(Stroke::new(1.0_f32, t.border))
                .corner_radius(10)
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(cw - 28.0);
                    // the frame sits in a horizontal row; stack its contents top to bottom
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            // the author's avatar and name open their profile
                            if let Some(a) = self.data.member(m.author) {
                                let (r, resp) =
                                    ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                                paint_avatar(ui.painter(), r.center(), 12.0, a, t.card);
                                if resp
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.chat.profile = Some(m.author);
                                }
                            }
                            // "Someone" for a person no longer here, as in the channel
                            let name = egui::Label::new(RichText::new(self.who(m.author)).strong());
                            if self.data.member(m.author).is_some() {
                                if ui
                                    .add(name.sense(Sense::click()))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.chat.profile = Some(m.author);
                                }
                            } else {
                                ui.add(name);
                            }
                            ui.label(
                                RichText::new(trf!(
                                    "{} in {} \u{2022} {}",
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
                                    // as the Inbox opens one (an old
                                    // thread closes), at the post:
                                    // nothing filtered out of the list
                                    self.open_chat_at(cid, None);
                                    self.chat.jump_to = Some(m.id);
                                    self.channel_find = None;
                                    self.channel_panel = 0;
                                }
                            });
                        });
                        ui.label(
                            RichText::new(
                                crate::app::chat::shown_post_title(&m).unwrap_or_default(),
                            )
                            .size(17.0),
                        );
                        // formatted as in the channel: bold, code,
                        // mentions and links
                        if !m.text.is_empty() {
                            let text = crate::app::chat::shown_text(&m);
                            if let Some(link) = self.message_text(ui, &text, 14.0) {
                                self.link_pending = Some(link);
                            }
                        }
                        // its pictures and files, as in the channel
                        if let Some(index) = self.attachment_tiles(ui, &m) {
                            self.apply_chat(vec![crate::app::chat::ChatAction::OpenMedia {
                                ch: cid,
                                msg: m.id,
                                index,
                            }]);
                        }
                    });
                });
        });
    }
}

/// "Sort:" and the Newest / Oldest menu, laid out right to left.
fn sort_menu(ui: &mut Ui, sort_old: &mut bool, label: Color32) {
    let sort = egui::ComboBox::from_id_salt("post_sort")
        .selected_text(if *sort_old {
            tr("Oldest")
        } else {
            tr("Newest")
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(sort_old, false, tr("Newest"));
            ui.selectable_value(sort_old, true, tr("Oldest"));
        });
    name_it(&sort.response, "Sort posts");
    ui.label(RichText::new(tr("Sort:")).color(label));
}

/// The width `sort_menu` takes.
fn sort_menu_width(ui: &Ui) -> f32 {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let label = ui
        .fonts(|f| f.layout_no_wrap(tr("Sort:").to_string(), font, Color32::WHITE))
        .size()
        .x;
    label + ui.spacing().item_spacing.x + ui.spacing().combo_width
}
