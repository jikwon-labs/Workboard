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

    /// The thin strip a closed sidebar leaves (Home and the app hubs alike):
    /// its "»" opens the sidebar again, as Cmd+\ does.
    pub(crate) fn collapsed_sidebar(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::SidePanel::left("sidebar_c")
            .exact_width(40.0)
            .resizable(false)
            .frame(side_panel_frame(&t, egui::Margin::same(6)))
            .show_inside(ui, |ui| {
                // a square across the strip, its "»" drawn as two chevrons
                // in the middle (the character sat high and to the left, in
                // a button narrower than tall)
                let w = ui.available_width();
                let (br, r) = ui.allocate_exact_size(vec2(w, w), Sense::click());
                if r.hovered() || r.is_pointer_button_down_on() {
                    let fill = crate::widgets::hover_fill(ui);
                    let fill = if r.is_pointer_button_down_on() {
                        fill.gamma_multiply(1.5)
                    } else {
                        fill
                    };
                    ui.painter().rect_filled(br, 6.0, fill);
                }
                let s = Stroke::new(1.5_f32, if r.hovered() { t.text } else { t.text2 });
                for dx in [-2.5, 2.5] {
                    let c = br.center() + vec2(dx, 0.0);
                    ui.painter().line(
                        vec![
                            c + vec2(-2.0, -4.0),
                            c + vec2(2.0, 0.0),
                            c + vec2(-2.0, 4.0),
                        ],
                        s,
                    );
                }
                let r = r.on_hover_text(tr("Open sidebar"));
                name_it(&r, "Open sidebar");
                if r.clicked() {
                    self.sidebar_collapsed = false;
                }
            });
    }

    pub(crate) fn sidebar(&mut self, ui: &mut Ui) {
        let t = self.t();
        if self.sidebar_collapsed {
            self.collapsed_sidebar(ui);
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
                self.side.unread_rows.clear();
                let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
                if let Some(y) = self.side.scroll_to.take() {
                    area = area.vertical_scroll_offset(y);
                }
                let out = area.show(ui, |ui| {
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
                            // the row is the one way in (a "..." on the
                            // heading opened the same page)
                            let (folded, _) = self.side_header(ui, "AI Chats", &[], 0);
                            if !folded && self.add_row(ui, "Ask, Build, Create").clicked() {
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
                    // room under the last row once the list is taller than
                    // the sidebar, so its end scrolls clear of the edge and
                    // the unread pill; a list that fits gets none, or the
                    // gap alone made it scroll
                    if ui.min_rect().height() > ui.clip_rect().height() {
                        ui.add_space(20.0);
                    }
                });
                self.unread_below(ui, &out);
            });
    }

    /// A red "Unread messages" pill at the bottom of the sidebar while a
    /// channel or DM with an unread count is scrolled out of view below;
    /// clicking it scrolls that row into the middle.
    fn unread_below(&mut self, ui: &mut Ui, out: &egui::scroll_area::ScrollAreaOutput<()>) {
        let view = out.inner_rect;
        let h = 26.0;
        // a row behind the pill counts as out of view
        let Some(first) = self
            .side
            .unread_rows
            .iter()
            .filter(|r| r.center().y > view.bottom() - h - 8.0)
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .copied()
        else {
            return;
        };
        let label = format!("{} \u{2193}", tr("Unread messages"));
        let g =
            ui.painter()
                .layout_no_wrap(label.clone(), FontId::proportional(13.0), Color32::WHITE);
        let pill = Rect::from_center_size(
            pos2(view.center().x, view.bottom() - 8.0 - h / 2.0),
            vec2(g.size().x + 24.0, h),
        );
        let resp = ui.interact(pill, ui.id().with("unread_below"), Sense::click());
        name_it(&resp, &label);
        let t = self.t();
        let fill = if resp.hovered() {
            t.danger.gamma_multiply(0.85)
        } else {
            t.danger
        };
        ui.painter().rect_filled(pill, h / 2.0, fill);
        ui.painter()
            .galley(pill.center() - g.size() / 2.0, g, Color32::WHITE);
        if resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let y = out.state.offset.y + first.center().y - view.center().y;
            self.side.scroll_to = Some(y.max(0.0));
            ui.ctx().request_repaint();
        }
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
                "My Tasks" => self.section == Section::Home && self.loc == Location::MyTasks,
                "Posts" => self.section == Section::Posts,
                "Meetings" => self.section == Section::Meetings,
                _ => false,
            };
            let unread = match label {
                "Inbox" => inbox_n,
                "Replies" => replies_n,
                _ => 0,
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
                None,
                true,
            );
            self.unread_pill(ui, r.rect, unread);
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
                    // its two items stay open; the row itself is the
                    // "Assigned to me" page, so there is no row for that
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
                    let subs: [(&str, Location, Option<String>); 2] = [
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
                        let r2 = self.side_row_tr(
                            ui,
                            14.0,
                            sel,
                            |p, c| match sl {
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
            let (folded, _) = self.side_header(ui, "Favorites", &[], 0);
            if !self.fold_shows(folded, "") {
                return;
            }
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
                    Location::List(id) => self.list_menu(ui, id, true),
                    Location::Folder(id) => self.folder_menu(ui, id, true),
                    Location::Space(id) => self.space_menu(ui, id, true),
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

    /// Spaces: the heading with its menu and "+", All Tasks and each
    /// Space's tree.
    fn sidebar_spaces(&mut self, ui: &mut Ui, tc: Color32, filter: &str, ws: &str) {
        if filter.is_empty() || filter == "space" {
            // the "+" beside the heading is the one way to add a Space (no
            // "New Space" row under the list too)
            let (folded, hs) = self.side_header(
                ui,
                "Spaces",
                &[("\u{22EF}", "Spaces settings"), ("+", "Create Space")],
                0,
            );
            name_it(&hs[0], "Spaces settings");
            if hs[1].clicked() {
                self.modal = Some(Modal::NewSpace {
                    name: String::new(),
                    color: STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13],
                });
            }
            click_menu(&hs[0], |ui| {
                if ui.button(tr("Expand all")).clicked() {
                    for s in &mut self.data.spaces {
                        s.expanded = true;
                    }
                    self.dirty = true;
                    ui.close_menu();
                }
                if ui.button(tr("Collapse all")).clicked() {
                    for s in &mut self.data.spaces {
                        s.expanded = false;
                    }
                    self.dirty = true;
                    ui.close_menu();
                }
            });
            if !self.fold_shows(folded, filter) {
                return;
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
        }
    }

    /// Channels: each channel I can see, with its unread count and settings
    /// button, and the field for a new channel. "Add Channel" is the only add
    /// button here (a "+" in the header too showed the same thing twice).
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
            // the Unread filter uses the real count; muting only hides the badge
            let chans: Vec<(u64, String, usize)> = self
                .data
                .channels
                .iter()
                .filter(|c| c.kind == ChannelKind::Channel && self.listed(c))
                .map(|c| (c.id, c.name.clone(), self.unread_count(c)))
                .collect();
            // folded, the heading counts what the rows would: not muted, not quiet
            let total = if muted {
                0
            } else {
                chans
                    .iter()
                    .filter(|c| self.data.extra.notify.channel_mode(c.0) != "nothing")
                    .map(|c| c.2)
                    .sum()
            };
            let (folded, hs) = self.side_header(
                ui,
                "Channels",
                &[
                    ("edit", "Drafts"),
                    ("clock", "Scheduled messages"),
                    ("bookmark", "Saved messages"),
                ],
                total,
            );
            name_it(&hs[0], "Drafts");
            if hs[0].clicked() {
                self.modal = Some(Modal::Drafts);
            }
            name_it(&hs[1], "Scheduled messages");
            if hs[1].clicked() {
                if let Some(net) = &self.sync_rt.net {
                    net.send(wb_net::Msg::ScheduledList);
                }
                self.modal = Some(Modal::Scheduled);
            }
            name_it(&hs[2], "Saved messages");
            if hs[2].clicked() {
                self.modal = Some(Modal::SavedMessages);
            }
            if !self.fold_shows(folded, filter) {
                return;
            }
            for (id, name, real_unread) in chans {
                if (filter == "unread" && real_unread == 0) || !self.matches_search(&name) {
                    continue;
                }
                // a channel set to notify about nothing is quiet: dimmed, no count
                let quiet = self.data.extra.notify.channel_mode(id) == "nothing";
                let unread = if muted || quiet { 0 } else { real_unread };
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
                    None,
                    quiet,
                );
                self.unread_pill(ui, r.rect, unread);
                if unread > 0 {
                    self.side.unread_rows.push(r.rect);
                }
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
                    self.leave_thread();
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
            let total = if muted {
                0
            } else {
                dms.iter().map(|d| d.2).sum()
            };
            // "New message" is the row at the end, not a "+" on the heading
            // too (two buttons for the same thing)
            let (folded, _) = self.side_header(ui, "Direct Messages", &[], total);
            if !self.fold_shows(folded, filter) {
                return;
            }
            for (id, other, real_unread) in dms {
                let unread = if muted { 0 } else { real_unread };
                let (name, sub) = match &other {
                    Some(m) => (m.shown().to_string(), None),
                    None => (
                        self.data
                            .member(me)
                            .map(|m| m.shown().to_string())
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
                    None,
                    false,
                );
                // the other person writing: moving dots right after their name
                if let Some(c) = self.data.channels.iter().find(|c| c.id == id) {
                    let name_w = ui
                        .painter()
                        .layout_no_wrap(name.clone(), FontId::proportional(14.0), t.text)
                        .size()
                        .x;
                    let left = (r.rect.left() + 34.0 + name_w).min(r.rect.right() - 54.0);
                    let dots =
                        Rect::from_min_size(pos2(left, r.rect.center().y - 8.0), vec2(26.0, 16.0));
                    self.typing_dots(ui, c, dots);
                }
                self.unread_pill(ui, r.rect, unread);
                if unread > 0 {
                    self.side.unread_rows.push(r.rect);
                }
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
                    self.leave_thread();
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

    /// The red count at the right end of a sidebar row, when something in
    /// it hasn't been seen.
    fn unread_pill(&self, ui: &Ui, row: Rect, n: usize) {
        if n > 0 {
            paint_unread_badge(
                ui.painter(),
                self.t().danger,
                row.right_center() - vec2(8.0, 0.0),
                Align2::RIGHT_CENTER,
                n,
            );
        }
    }

    /// A channel set to notify about nothing: dimmed, no count.
    pub(in crate::app) fn quiet_channel(&self, c: &Channel) -> bool {
        c.kind == ChannelKind::Channel && self.data.extra.notify.channel_mode(c.id) == "nothing"
    }

    /// Unread messages in the channels and direct messages the sidebar
    /// lists, for the Chat badge on the rail.
    pub(in crate::app) fn chat_unread_total(&self) -> usize {
        self.data
            .channels
            .iter()
            .filter(|c| self.listed(c) && !self.quiet_channel(c))
            .map(|c| self.unread_count(c))
            .sum()
    }

    pub(in crate::app) fn unread_count(&self, c: &Channel) -> usize {
        if !self.data.extra.notify.chat || self.reading_now(c) {
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
