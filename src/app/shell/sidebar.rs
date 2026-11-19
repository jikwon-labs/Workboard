// The Home sidebar: its top row, the fixed rows (Inbox, Replies, My Tasks...),
// Favorites, Spaces, Channels and Direct Messages, and the Personal List.

use super::*;

// ---------------------------------------------------------------- sidebar

impl App {
    pub(crate) fn count_open(&self, list: u64) -> usize {
        self.data
            .tasks
            .iter()
            .filter(|t| {
                t.list_id == list && t.parent.is_none() && !t.archived && !self.data.is_closed(t)
            })
            .count()
    }

    pub(crate) fn sidebar(&mut self, ui: &mut Ui) {
        let t = self.t();
        if self.sidebar_collapsed {
            egui::SidePanel::left("sidebar_c")
                .exact_width(40.0)
                .resizable(false)
                .frame(side_panel_frame(&t, egui::Margin::same(6)))
                .show_inside(ui, |ui| {
                    if ui
                        .add(ghost(egui::Button::new(
                            RichText::new("\u{00BB}").size(16.0),
                        )))
                        .on_hover_text(tr("Open sidebar"))
                        .clicked()
                    {
                        self.sidebar_collapsed = false;
                    }
                });
            return;
        }
        // App hubs (hubs.rs) reuse this id, so they keep the same width.
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(255.0)
            .width_range(200.0..=400.0)
            .frame(side_panel_frame(&t, egui::Margin::symmetric(8, 6)))
            .show_inside(ui, |ui| {
                // header: Home, (hover) search / filter / close, + Create
                self.sidebar_top(ui, t);
                ui.add_space(4.0);
                let filter = self.side.filter.unwrap_or("");
                // The list still scrolls with the wheel or trackpad; the bar
                // itself is hidden so it does not crowd the narrow sidebar.
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        let tc = t.text2;
                        let muted = self.data.extra.muted_until > now_ts();
                        if filter.is_empty() {
                            self.sidebar_nav(ui, t, tc, muted);
                            ui.add_space(6.0);
                            let r = ui.available_rect_before_wrap();
                            ui.painter().hline(
                                r.left() + 6.0..=r.right() - 6.0,
                                r.top(),
                                Stroke::new(1.0_f32, t.border),
                            );

                            if !self.hidden("AI Chats") {
                                let hs = self.side_header(
                                    ui,
                                    "AI Chats",
                                    &[("\u{22EF}", "More actions")],
                                );
                                if hs[0].clicked() {
                                    self.section = Section::Ai;
                                }
                                if self.add_row(ui, "Ask, Build, Create").clicked() {
                                    self.section = Section::Ai;
                                }
                            }

                            self.sidebar_favorites(ui);
                        }

                        let ws = self.data.workspace.clone();
                        self.sidebar_spaces(ui, tc, filter, &ws);

                        // Channels
                        self.sidebar_channels(ui, t, tc, filter, muted, &ws);

                        // Direct Messages
                        self.sidebar_dms(ui, t, filter, muted);
                        ui.add_space(20.0);
                    });
            });
    }

    /// The sidebar's top: Home, the search, filter and close buttons shown on
    /// hover, + Create, and the search field and filter chips when open.
    fn sidebar_top(&mut self, ui: &mut Ui, t: Theme) {
        let w = ui.available_width();
        let (hr, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
        let hovered = ui.rect_contains_pointer(hr);
        ui.painter().text(
            hr.left_center() + vec2(6.0, 0.0),
            Align2::LEFT_CENTER,
            tr("Home"),
            FontId::proportional(17.0),
            t.text,
        );
        let create_r = Rect::from_min_size(
            pos2(hr.right() - 80.0, hr.center().y - 14.0),
            vec2(80.0, 28.0),
        );
        let cresp = ui.interact(create_r, ui.id().with("side_create"), Sense::click());
        name_it(&cresp, "Create");
        ui.painter().rect_filled(
            create_r,
            7.0,
            if cresp.hovered() {
                t.primary_hover
            } else {
                t.primary
            },
        );
        ui.painter().text(
            create_r.center(),
            Align2::CENTER_CENTER,
            tr("+  Create"),
            FontId::proportional(14.0),
            t.on_primary,
        );
        if self.force_menu.as_deref() == Some("create") {
            let pid = ui.id().with("create_pop");
            ui.memory_mut(|m| m.open_popup(pid));
        }
        popup(ui, &cresp, ui.id().with("create_pop"), 400.0, |ui| {
            self.create_menu(ui)
        });
        let icons: [(&str, &str); 3] = [
            ("search", "Search your sidebar"),
            ("filter", "Filter your sidebar"),
            ("\u{00AB}", "Close sidebar"),
        ];
        for (i, (glyph, tip)) in icons.iter().enumerate() {
            let br = Rect::from_center_size(
                pos2(
                    create_r.left() - 90.0 + i as f32 * 29.0 + 14.0,
                    hr.center().y,
                ),
                vec2(26.0, 26.0),
            );
            let resp = ui.interact(br, ui.id().with(("side_ic", i)), Sense::click());
            name_it(&resp, tip);
            let active =
                (i == 0 && self.side.search.is_some()) || (i == 1 && self.side.filter.is_some());
            if resp.hovered() || active {
                ui.painter().rect_filled(br, 6.0, t.row_selected);
            }
            if (hovered || active) && !line_icon(ui.painter(), glyph, br.center(), t.text2) {
                ui.painter().text(
                    br.center(),
                    Align2::CENTER_CENTER,
                    *glyph,
                    FontId::proportional(16.0),
                    t.text2,
                );
            }
            if resp.on_hover_text(tr(tip)).clicked() {
                match i {
                    0 => {
                        self.side.search = if self.side.search.is_some() {
                            None
                        } else {
                            Some(String::new())
                        };
                        self.side.search_focus = self.side.search.is_some();
                    }
                    1 => {
                        self.side.filter = if self.side.filter.is_some() {
                            None
                        } else {
                            Some("")
                        }
                    }
                    _ => self.sidebar_collapsed = true,
                }
            }
        }
        if let Some(mut q) = self.side.search.take() {
            let r = ui.add(
                field(&mut q)
                    .hint_text(tr("Search the sidebar"))
                    .desired_width(f32::INFINITY),
            );
            if std::mem::take(&mut self.side.search_focus) {
                r.request_focus();
            }
            self.side.search = Some(q);
        }
        if let Some(f) = self.side.filter {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (key, label) in [("space", "Space"), ("unread", "Unread"), ("dms", "DMs")] {
                    let on = f == key;
                    let g = ui.painter().layout_no_wrap(
                        tr(label).to_string(),
                        FontId::proportional(13.0),
                        if on { Color32::WHITE } else { t.text2 },
                    );
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(g.size().x + 20.0, 24.0), Sense::click());
                    name_it(&resp, &format!("Sidebar filter {}", label));
                    ui.painter().rect_filled(
                        r,
                        12.0,
                        if on {
                            t.primary
                        } else if resp.hovered() {
                            t.hover
                        } else {
                            t.bg
                        },
                    );
                    ui.painter().rect_stroke(
                        r,
                        12.0,
                        Stroke::new(1.0_f32, t.border),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().galley(
                        r.center() - g.size() / 2.0,
                        g,
                        if on { t.on_primary } else { t.text2 },
                    );
                    if resp.clicked() {
                        self.side.filter = Some(if on { "" } else { key });
                    }
                }
            });
        }
    }

    /// The fixed rows at the top of the sidebar: Inbox, Replies, Assigned
    /// Comments, Posts and the rest, with My Tasks and its three items.
    fn sidebar_nav(&mut self, ui: &mut Ui, t: Theme, tc: Color32, muted: bool) {
        let inbox_n = if muted { 0 } else { self.inbox_badge() };
        let replies_n = if muted || self.hidden("Replies") {
            0
        } else {
            self.unread_replies()
        };
        let nav: [(&str, &str); 9] = [
            ("Inbox", "\u{1F4E5}"),
            ("Replies", "\u{21A9}"),
            ("Assigned Comments", "\u{1F4AC}"),
            ("Posts", "\u{1F4E2}"),
            ("Skills", "\u{26A1}"),
            ("Artifacts", "\u{1F496}"),
            ("Meetings", "\u{260E}"),
            ("My Tasks", "\u{1F464}"),
            ("More", "\u{22EF}"),
        ];
        for (label, icon) in nav {
            if self.hidden(label) || !self.matches_search(label) {
                continue;
            }
            let sel = match label {
                "Inbox" => self.section == Section::Home && self.loc == Location::Inbox,
                "Replies" => self.section == Section::Home && self.loc == Location::Replies,
                "Assigned Comments" => {
                    self.section == Section::Home && self.loc == Location::Assigned
                }
                "My Tasks" => {
                    self.section == Section::Home
                        && self.loc == Location::MyTasks
                        && !self.my_tasks_open
                }
                "Posts" => self.section == Section::Posts,
                "Meetings" => self.section == Section::Meetings,
                _ => false,
            };
            let badge = match label {
                "Inbox" if inbox_n > 0 => Some(inbox_n.to_string()),
                "Replies" if replies_n > 0 => Some(replies_n.to_string()),
                _ => None,
            };
            let kind = match label {
                "Inbox" => "inbox",
                "Replies" => "reply",
                "Assigned Comments" => "person_comment",
                "Posts" => "megaphone",
                "Skills" => "bolt",
                "Artifacts" => "heart",
                "Meetings" => "phone",
                "My Tasks" => "person",
                _ => "dots",
            };
            let r = self.side_row_tr(
                ui,
                0.0,
                sel,
                |p, c| {
                    if !line_icon(p, kind, c, tc) {
                        p.text(
                            c,
                            Align2::CENTER_CENTER,
                            icon,
                            FontId::proportional(13.0),
                            tc,
                        );
                    }
                },
                label,
                None,
                badge,
                true,
            );
            match label {
                "More" => {
                    popup(ui, &r, ui.id().with("side_more"), 220.0, |ui| {
                        for (l, ic) in [
                            ("Overview", "home"),
                            ("Docs", "docs"),
                            ("Dashboards", "dashboards"),
                            ("Whiteboards", "whiteboards"),
                            ("Forms", "forms"),
                            ("Goals", "goals"),
                            ("Timesheets", "timesheets"),
                            ("People", "teams"),
                        ] {
                            if menu_row(ui, &t, tr(l), false, |p, c| {
                                line_icon(p, ic, c, t.text2);
                            })
                            .clicked()
                            {
                                match l {
                                    "Overview" => self.actions.push(Action::Go(Location::Home)),
                                    "People" => self.section = Section::People,
                                    other => self.open_app(other),
                                }
                                close_popup(ui);
                            }
                        }
                        ui.separator();
                        if menu_row(ui, &t, tr("Customize your sidebar"), false, |p, c| {
                            line_icon(p, "sidebar", c, t.text2);
                        })
                        .clicked()
                        {
                            self.modal = Some(Modal::CustomizeSidebar);
                            close_popup(ui);
                        }
                    });
                }
                "My Tasks" if self.my_tasks_open => {
                    // its three items stay open, as in the original
                    if r.clicked() {
                        self.actions.push(Action::Go(Location::MyTasks));
                    }
                    let me = self.data.me;
                    let td = self
                        .data
                        .tasks
                        .iter()
                        .filter(|x| {
                            x.assignees.contains(&me)
                                && x.due.map(|d| d <= today()).unwrap_or(false)
                                && self.task_visible(x)
                                && !self.data.is_closed(x)
                        })
                        .count();
                    let pl = self.personal_list_id();
                    let subs: [(&str, Location, Option<String>); 3] = [
                        ("Assigned to me", Location::MyTasks, None),
                        (
                            "Today & Overdue",
                            Location::TodayOverdue,
                            if td > 0 {
                                Some(format!("\u{2611} {}", td))
                            } else {
                                None
                            },
                        ),
                        (
                            "Personal List",
                            pl.map(Location::List).unwrap_or(Location::MyTasks),
                            None,
                        ),
                    ];
                    for (sl, loc, badge) in subs {
                        let sel = self.section == Section::Home
                            && self.loc == loc
                            && !(sl == "Personal List" && pl.is_none());
                        let mm = self.data.member(me).cloned();
                        let bg = t.sidebar;
                        let r2 = self.side_row_tr(
                            ui,
                            14.0,
                            sel,
                            |p, c| match sl {
                                "Assigned to me" => {
                                    if let Some(m) = &mm {
                                        paint_avatar(p, c, 8.0, m, bg);
                                    }
                                }
                                "Today & Overdue" => paint_calendar_icon(p, c, tc),
                                _ => paint_person_icon(p, c, tc),
                            },
                            sl,
                            None,
                            badge,
                            false,
                        );
                        if r2.clicked() {
                            if sl == "Personal List" && pl.is_none() {
                                let id = self.create_personal_list();
                                self.actions.push(Action::Go(Location::List(id)));
                            } else {
                                self.actions.push(Action::Go(loc));
                            }
                        }
                    }
                }
                _ => {
                    if r.clicked() {
                        match label {
                            "Inbox" => self.actions.push(Action::Go(Location::Inbox)),
                            "Replies" => self.actions.push(Action::Go(Location::Replies)),
                            "Assigned Comments" => {
                                self.actions.push(Action::Go(Location::Assigned))
                            }
                            "My Tasks" => {
                                self.my_tasks_open = !self.my_tasks_open;
                                self.actions.push(Action::Go(Location::MyTasks));
                            }
                            "Posts" => self.section = Section::Posts,
                            "Meetings" => self.section = Section::Meetings,
                            "Skills" => self.modal = Some(super::menus::unavailable("Skills")),
                            _ => self.modal = Some(super::menus::unavailable("Artifacts")),
                        }
                    }
                }
            }
        }
    }

    /// Favorites: the favourite places, then the favourite tasks.
    fn sidebar_favorites(&mut self, ui: &mut Ui) {
        // (while searching, the heading only shows with a match under it)
        let fav_tasks: Vec<(u64, String)> = self.favorite_tasks();
        // an archived Space, Folder or List leaves Favorites
        // until it is restored
        let favs: Vec<Location> = self
            .data
            .favorites
            .iter()
            .copied()
            .filter(|f| !self.data.location_archived(*f))
            .collect();
        let fav_match = favs
            .iter()
            .any(|f| self.matches_search(&self.data.location_name(*f)))
            || fav_tasks.iter().any(|(_, n)| self.matches_search(n));
        if fav_match && !self.hidden("Favorites") {
            self.side_header(ui, "Favorites", &[]);
            for f in favs {
                let name = self.data.location_name(f);
                if !self.matches_search(&name) {
                    continue;
                }
                let sel = self.section == Section::Home && self.loc == f;
                let r = self.side_row(
                    ui,
                    0.0,
                    sel,
                    |p, c| {
                        p.text(
                            c,
                            Align2::CENTER_CENTER,
                            "\u{2605}",
                            FontId::proportional(12.0),
                            STAR_YELLOW,
                        );
                    },
                    &name,
                    None,
                    None,
                    false,
                );
                if r.clicked() {
                    self.actions.push(Action::Go(f));
                }
                // the same menu as the item has where it lives
                r.context_menu(|ui| match f {
                    Location::List(id) => self.list_menu(ui, id),
                    Location::Folder(id) => self.folder_menu(ui, id),
                    Location::Space(id) => self.space_menu(ui, id),
                    _ => {
                        if ui.button(tr("Remove from Favorites")).clicked() {
                            self.actions.push(Action::ToggleFavorite(f));
                            ui.close_menu();
                        }
                    }
                });
            }
            for (tid, name) in fav_tasks {
                if !self.matches_search(&name) {
                    continue;
                }
                let sel = self.open_task == Some(tid);
                let r = self.side_row(
                    ui,
                    0.0,
                    sel,
                    |p, c| {
                        p.text(
                            c,
                            Align2::CENTER_CENTER,
                            "\u{2605}",
                            FontId::proportional(12.0),
                            STAR_YELLOW,
                        );
                    },
                    &name,
                    None,
                    None,
                    false,
                );
                if r.clicked() {
                    self.actions.push(Action::Open(tid));
                }
                r.context_menu(|ui| self.task_menu(ui, tid));
            }
        }
    }

    /// Spaces: the heading with its menu, All Tasks, each Space's tree and
    /// New Space.
    fn sidebar_spaces(&mut self, ui: &mut Ui, tc: Color32, filter: &str, ws: &str) {
        if filter.is_empty() || filter == "space" {
            let hs = self.side_header(
                ui,
                "Spaces",
                &[("\u{22EF}", "Spaces settings"), ("+", "Create Space")],
            );
            popup(ui, &hs[0], ui.id().with("spaces_settings"), 200.0, |ui| {
                if ui.button(tr("Create Space")).clicked() {
                    self.modal = Some(Modal::NewSpace {
                        name: String::new(),
                        color: STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13],
                    });
                    close_popup(ui);
                }
                if ui.button(tr("Expand all")).clicked() {
                    for s in &mut self.data.spaces {
                        s.expanded = true;
                    }
                    self.dirty = true;
                    close_popup(ui);
                }
                if ui.button(tr("Collapse all")).clicked() {
                    for s in &mut self.data.spaces {
                        s.expanded = false;
                    }
                    self.dirty = true;
                    close_popup(ui);
                }
            });
            if hs[1].clicked() {
                self.modal = Some(Modal::NewSpace {
                    name: String::new(),
                    color: STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13],
                });
            }
            let sub = format!("- {}", ws);
            if self.matches_search("All Tasks") {
                let sel = self.section == Section::Home && self.loc == Location::Everything;
                let r = self.side_row_tr(
                    ui,
                    0.0,
                    sel,
                    |p, c| {
                        p.rect_stroke(
                            Rect::from_center_size(c, vec2(16.0, 16.0)),
                            4.0,
                            Stroke::new(1.0_f32, tc),
                            egui::StrokeKind::Inside,
                        );
                        p.text(
                            c,
                            Align2::CENTER_CENTER,
                            "\u{2731}",
                            FontId::proportional(9.0),
                            tc,
                        );
                    },
                    "All Tasks",
                    Some(&sub),
                    None,
                    false,
                );
                if r.clicked() {
                    self.actions.push(Action::Go(Location::Everything));
                }
            }
            let spaces: Vec<Space> = self
                .data
                .spaces
                .iter()
                .filter(|s| s.private_owner.is_none() && !s.archived && !s.hidden)
                .cloned()
                .collect();
            for s in &spaces {
                if !self.space_matches_search(s) {
                    continue;
                }
                self.space_rows(ui, s);
            }
            if self.add_row(ui, "New Space").clicked() {
                self.modal = Some(Modal::NewSpace {
                    name: String::new(),
                    color: STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13],
                });
            }
        }
    }

    /// Channels: each channel I can see, with its unread count and settings
    /// button, and the field for a new channel.
    fn sidebar_channels(
        &mut self,
        ui: &mut Ui,
        t: Theme,
        tc: Color32,
        filter: &str,
        muted: bool,
        ws: &str,
    ) {
        if (filter.is_empty() || filter == "unread") && !self.hidden("Channels") {
            let hs = self.side_header(
                ui,
                "Channels",
                &[("bookmark", "Saved messages"), ("+", "Create a Channel")],
            );
            name_it(&hs[0], "Saved messages");
            if hs[0].clicked() {
                self.modal = Some(Modal::SavedMessages);
            }
            if hs[1].clicked() {
                self.modal = Some(Modal::NewChannel {
                    name: String::new(),
                    private: false,
                    add_list: false,
                });
            }
            // the Unread filter uses the real count; muting only hides the badge
            let chans: Vec<(u64, String, usize)> = self
                .data
                .channels
                .iter()
                .filter(|c| c.kind == ChannelKind::Channel && self.listed(c))
                .map(|c| (c.id, c.name.clone(), self.unread_count(c)))
                .collect();
            for (id, name, real_unread) in chans {
                if (filter == "unread" && real_unread == 0) || !self.matches_search(&name) {
                    continue;
                }
                let unread = if muted { 0 } else { real_unread };
                let sel = self.section == Section::Chat && self.chat.open == Some(id);
                let sub = if name == "general" {
                    Some(format!("- {}", ws))
                } else {
                    None
                };
                let r = self.side_row(
                    ui,
                    0.0,
                    sel,
                    |p, c| {
                        p.text(
                            c,
                            Align2::CENTER_CENTER,
                            "#",
                            FontId::proportional(15.0),
                            tc,
                        );
                    },
                    &name,
                    sub.as_deref(),
                    if unread > 0 {
                        Some(unread.to_string())
                    } else {
                        None
                    },
                    false,
                );
                let sr = Rect::from_center_size(
                    r.rect.right_center() - vec2(14.0, 0.0),
                    vec2(20.0, 20.0),
                );
                let sresp = ui.interact(sr, ui.id().with(("chset", id)), Sense::click());
                if r.hovered() || sresp.hovered() {
                    ui.painter().rect_filled(
                        sr,
                        5.0,
                        if sresp.hovered() {
                            t.border
                        } else {
                            t.row_selected
                        },
                    );
                    ui.painter().text(
                        sr.center(),
                        Align2::CENTER_CENTER,
                        "\u{22EF}",
                        FontId::proportional(13.0),
                        t.text2,
                    );
                }
                if sresp.on_hover_text(tr("Channel settings")).clicked() {
                    self.modal = Some(Modal::ChannelSettings(id));
                } else if r.clicked() {
                    self.section = Section::Chat;
                    self.chat.open = Some(id);
                    self.chat.thread_open = None;
                }
            }
            if let Some(mut name) = self.new_channel.take() {
                let mut keep = true;
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let r = ui.add(
                        field(&mut name)
                            .id(egui::Id::new("new_channel_input"))
                            .hint_text(tr("channel-name"))
                            .desired_width(170.0),
                    );
                    // focus once when the field appears, not every frame,
                    // and bring it into view: it sits at the end of the
                    // channel list, often below the bottom of the sidebar
                    // (for a few passes: egui can discard the first one)
                    let nr = ui.ctx().cumulative_pass_nr();
                    let shown_id = r.id.with("appeared");
                    if focus_once(ui, &r) {
                        ui.data_mut(|d| d.insert_temp(shown_id, nr));
                    }
                    let appeared: u64 = ui.data(|d| d.get_temp(shown_id)).unwrap_or(0);
                    if nr <= appeared + 3 && !ui.clip_rect().contains_rect(r.rect) {
                        r.scroll_to_me(Some(Align::Center));
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        r.surrender_focus();
                        keep = false;
                    } else if r.lost_focus() {
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.trim().is_empty()
                        {
                            let id = self.data.new_id();
                            let clean = name.trim().to_lowercase().replace(' ', "-");
                            let members = self.data.members.iter().map(|m| m.id).collect();
                            self.data.channels.push(Channel {
                                id,
                                name: clean,
                                kind: ChannelKind::Channel,
                                topic: String::new(),
                                members,
                                messages: vec![],
                                last_read: now_ts(),
                                creator: self.data.me,
                                ..Default::default()
                            });
                            self.section = Section::Chat;
                            self.chat.open = Some(id);
                            self.dirty = true;
                        }
                        keep = false;
                    }
                });
                if keep {
                    self.new_channel = Some(name);
                }
            } else if filter.is_empty() && self.add_row(ui, "Add Channel").clicked() {
                self.new_channel = Some(String::new());
            }
        }
    }

    /// Direct Messages: a row per conversation with the other person's
    /// avatar and presence, and New message.
    fn sidebar_dms(&mut self, ui: &mut Ui, t: Theme, filter: &str, muted: bool) {
        if (filter.is_empty() || filter == "dms" || filter == "unread")
            && !self.hidden("Direct Messages")
        {
            let hs = self.side_header(ui, "Direct Messages", &[("+", "New message")]);
            if hs[0].clicked() {
                self.new_message_open = true;
            }
            let me = self.data.me;
            let dms: Vec<(u64, Option<Member>, usize)> = self
                .data
                .channels
                .iter()
                .filter(|c| c.kind == ChannelKind::Direct && self.listed(c))
                .map(|c| {
                    (
                        c.id,
                        c.members
                            .iter()
                            .find(|m| **m != me)
                            .and_then(|m| self.data.member(*m))
                            .cloned(),
                        self.unread_count(c),
                    )
                })
                .collect();
            for (id, other, real_unread) in dms {
                let unread = if muted { 0 } else { real_unread };
                let (name, sub) = match &other {
                    Some(m) => (m.name.clone(), None),
                    None => (
                        self.data
                            .member(me)
                            .map(|m| m.name.clone())
                            .unwrap_or_default(),
                        Some(tr("\u{2014} You").to_string()),
                    ),
                };
                if (filter == "unread" && real_unread == 0) || !self.matches_search(&name) {
                    continue;
                }
                let sel = self.section == Section::Chat && self.chat.open == Some(id);
                let who = other.clone().or_else(|| self.data.member(me).cloned());
                let bg = t.sidebar;
                let grey = t.text3;
                // a dot only when the server says who is online
                let online = who.as_ref().and_then(|m| self.is_online(m.id));
                let r = self.side_row(
                    ui,
                    0.0,
                    sel,
                    |p, c| {
                        if let Some(m) = &who {
                            paint_avatar(p, c, 9.0, m, bg);
                            if let Some(on) = online {
                                crate::widgets::paint_presence(
                                    p,
                                    c + vec2(7.0, 7.0),
                                    2.2,
                                    on,
                                    bg,
                                    grey,
                                );
                            }
                        }
                    },
                    &name,
                    sub.as_deref(),
                    if unread > 0 {
                        Some(unread.to_string())
                    } else {
                        None
                    },
                    false,
                );
                let sr = Rect::from_center_size(
                    r.rect.right_center() - vec2(14.0, 0.0),
                    vec2(20.0, 20.0),
                );
                let sresp = ui.interact(sr, ui.id().with(("dmset", id)), Sense::click());
                if r.hovered() || sresp.hovered() {
                    ui.painter().rect_filled(
                        sr,
                        5.0,
                        if sresp.hovered() {
                            t.border
                        } else {
                            t.row_selected
                        },
                    );
                    ui.painter().text(
                        sr.center(),
                        Align2::CENTER_CENTER,
                        "\u{22EF}",
                        FontId::proportional(13.0),
                        t.text2,
                    );
                }
                if sresp.on_hover_text(tr("DM Settings")).clicked() {
                    self.modal = Some(Modal::ChannelSettings(id));
                } else if r.clicked() {
                    self.section = Section::Chat;
                    self.chat.open = Some(id);
                    self.chat.thread_open = None;
                }
            }
            if filter.is_empty() && self.add_row(ui, "New message").clicked() {
                self.new_message_open = true;
            }
        }
    }

    pub(crate) fn on_personal_list(&self) -> bool {
        matches!(self.loc, Location::List(id) if Some(id) == self.personal_list_id())
    }

    /// The Personal List opens with Group: None, the Date created column and
    /// only Board / List views; the earlier settings come back on leaving.
    pub(crate) fn personal_list_defaults(&mut self) {
        let on = self.section == Section::Home && self.on_personal_list();
        match (on, self.personal_saved) {
            (true, None) => {
                self.personal_saved =
                    Some((self.group_by, self.hidden_cols.contains("Date created")));
                self.group_by = GroupBy::None;
                self.hidden_cols.remove("Date created");
            }
            (false, Some((g, hid))) => {
                self.group_by = g;
                if hid {
                    self.hidden_cols.insert("Date created".into());
                }
                self.personal_saved = None;
            }
            _ => {}
        }
        if on && !matches!(self.view, ViewKind::Board | ViewKind::List) {
            self.view = ViewKind::List;
        }
    }

    pub(crate) fn personal_list_id(&self) -> Option<u64> {
        let me = self.data.me;
        self.data
            .spaces
            .iter()
            .find(|s| s.private_owner == Some(me))
            .and_then(|s| s.lists.first())
            .map(|l| l.id)
    }

    pub(crate) fn create_personal_list(&mut self) -> u64 {
        if let Some(id) = self.personal_list_id() {
            return id;
        }
        let me = self.data.me;
        let sid = self.data.new_id();
        let lid = self.data.new_id();
        self.data.spaces.push(Space {
            id: sid,
            name: tr("Personal").into(),
            color: [100, 100, 100],
            statuses: default_statuses(),
            folders: vec![],
            lists: vec![ListItem {
                id: lid,
                name: tr("Personal List").into(),
                ..Default::default()
            }],
            expanded: true,
            private_owner: Some(me),
            ..Default::default()
        });
        self.dirty = true;
        lid
    }

    fn unread_count(&self, c: &Channel) -> usize {
        if !self.data.extra.notify.chat {
            return 0;
        }
        self.unread(c)
    }

    /// Tasks starred with "Add to Favorites", for the Favorites section.
    fn favorite_tasks(&self) -> Vec<(u64, String)> {
        self.data
            .extra
            .starred
            .iter()
            .filter_map(|s| s.strip_prefix("task:")?.parse::<u64>().ok())
            .filter_map(|id| self.task(id))
            .filter(|t| self.data.task_live(t))
            .map(|t| (t.id, t.name.clone()))
            .collect()
    }
}
