// App "hubs" as the person's ClickUp shows them: each app (Docs, Dashboards,
// Whiteboards, Forms, Teams) has its own sidebar (All / My / Shared / Private,
// Favorites) and a template gallery; Goals has a start page; Timesheets has
// tabs (My timesheet, All timesheets, Approvals) and editable hours.

use super::*;

fn hub_frame(t: &Theme) -> egui::Frame {
    egui::Frame::new().fill(t.sidebar).corner_radius(egui::CornerRadius { nw: 8, ne: 0, sw: 8, se: 0 }).inner_margin(egui::Margin::symmetric(8, 6))
}

fn empty_card(ui: &mut Ui, t: &Theme, glyph: &str, text: &str) {
    egui::Frame::new().fill(t.bg).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::symmetric(10, 14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(glyph).size(16.0).color(Color32::from_rgb(0xf8, 0xae, 0x00)));
            ui.label(RichText::new(text).size(13.0).color(t.text2));
        });
    });
}

/// A template card as in the galleries: round coloured icon, title, description.
fn gallery_card(ui: &mut Ui, t: &Theme, w: f32, color: [u8; 3], glyph: &str, title: &str, sub: &str, ai: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 142.0), Sense::click());
    name_it(&resp, title);
    ui.painter().rect_filled(r, 10.0, if resp.hovered() { t.sidebar } else { t.card });
    ui.painter().rect_stroke(r, 10.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let c = r.left_top() + vec2(38.0, 36.0);
    if glyph == "+" {
        ui.painter().circle_filled(c, 18.0, t.hover);
        ui.painter().text(c, Align2::CENTER_CENTER, "+", FontId::proportional(22.0), t.text2);
    } else {
        ui.painter().circle_filled(c, 18.0, rgb(color).gamma_multiply(0.35));
        ui.painter().text(c, Align2::CENTER_CENTER, glyph, FontId::proportional(15.0), rgb(color));
    }
    let tc = if ai { Color32::from_rgb(0x9b, 0x2d, 0xd6) } else { t.text };
    ui.painter().text(r.left_top() + vec2(20.0, 82.0), Align2::LEFT_CENTER, title, FontId::proportional(15.0), tc);
    let g = ui.painter().layout(sub.to_string(), FontId::proportional(13.0), t.text2, w - 40.0);
    ui.painter().galley(r.left_top() + vec2(20.0, 96.0), g, t.text2);
    resp
}

impl App {
    /// The sidebar used by every app hub. Returns (clicked item, plus clicked).
    fn hub_sidebar(&mut self, ui: &mut Ui, id: &str, title: &str, items: &[(&str, &str, Option<usize>)], selected: u8, create_button: bool) -> (Option<u8>, bool) {
        let t = self.t();
        let mut clicked = None;
        let mut plus = false;
        egui::SidePanel::left(egui::Id::new(("hub", id))).exact_width(255.0).resizable(false).frame(hub_frame(&t)).show_inside(ui, |ui| {
            let w = ui.available_width();
            let (hr, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
            ui.painter().text(hr.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(17.0), t.text);
            if create_button {
                let br = Rect::from_min_size(pos2(hr.right() - 80.0, hr.center().y - 14.0), vec2(80.0, 28.0));
                let b = ui.interact(br, ui.id().with("hub_create"), Sense::click());
                name_it(&b, &format!("Create in {}", title));
                ui.painter().rect_filled(br, 7.0, if b.hovered() { t.primary_hover } else { t.primary });
                ui.painter().text(br.center(), Align2::CENTER_CENTER, "+  Create", FontId::proportional(14.0), Color32::WHITE);
                plus |= b.clicked();
            } else {
                let br = Rect::from_center_size(pos2(hr.right() - 14.0, hr.center().y), vec2(28.0, 28.0));
                let b = ui.interact(br, ui.id().with("hub_plus"), Sense::click());
                name_it(&b, &format!("New in {}", title));
                ui.painter().rect_filled(br, 7.0, if b.hovered() { t.hover } else { t.bg });
                ui.painter().rect_stroke(br, 7.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                ui.painter().text(br.center(), Align2::CENTER_CENTER, "+", FontId::proportional(18.0), t.text);
                plus |= b.clicked();
            }
            let cr = Rect::from_center_size(pos2(hr.right() - (if create_button { 100.0 } else { 44.0 }), hr.center().y), vec2(24.0, 24.0));
            let c = ui.interact(cr, ui.id().with("hub_collapse"), Sense::click());
            if c.hovered() {
                ui.painter().rect_filled(cr, 5.0, t.row_selected);
            }
            ui.painter().text(cr.center(), Align2::CENTER_CENTER, "\u{00AB}", FontId::proportional(16.0), t.text2);
            if c.on_hover_text("Back to Home").clicked() {
                self.section = Section::Home;
            }
            ui.add_space(4.0);
            for (i, (icon, label, count)) in items.iter().enumerate() {
                let sel = selected == i as u8;
                let w = ui.available_width();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                name_it(&resp, label);
                if sel {
                    ui.painter().rect_filled(r, 7.0, t.row_selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 7.0, t.hover);
                }
                if *icon == "@me" {
                    if let Some(m) = self.data.member(self.data.me) {
                        paint_avatar(ui.painter(), r.left_center() + vec2(16.0, 0.0), 9.0, m, t.sidebar);
                    }
                } else {
                    ui.painter().text(r.left_center() + vec2(16.0, 0.0), Align2::CENTER_CENTER, *icon, FontId::proportional(13.0), t.text2);
                }
                ui.painter().text(r.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, *label, FontId::proportional(14.0), if sel { t.text } else { t.text2 });
                if let Some(n) = count {
                    if *n > 0 {
                        ui.painter().text(r.right_center() - vec2(12.0, 0.0), Align2::RIGHT_CENTER, n.to_string(), FontId::proportional(12.0), t.text3);
                    }
                }
                if resp.clicked() {
                    clicked = Some(i as u8);
                }
            }
            ui.add_space(6.0);
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.left() + 6.0..=r.right() - 6.0, r.top(), Stroke::new(1.0, t.border));
            ui.add_space(10.0);
            ui.label(RichText::new("  Favorites").size(13.0).color(t.text2));
            let prefix = match id {
                "docs" => "doc:",
                "wb" => "wb:",
                "forms" => "form:",
                "dash" => "dash:",
                _ => "",
            };
            let stars: Vec<String> = self.data.extra.starred.iter().filter(|s| !prefix.is_empty() && s.starts_with(prefix)).cloned().collect();
            if stars.is_empty() {
                let noun = match id {
                    "docs" => "a Doc",
                    "wb" => "a Whiteboard",
                    "forms" => "a Form",
                    _ => "a Dashboard",
                };
                if id != "teams" {
                    empty_card(ui, &t, "\u{2605}", &format!("Star {} to see it here", noun));
                }
            } else {
                for st in stars {
                    let oid: u64 = st[prefix.len()..].parse().unwrap_or(0);
                    let name = match id {
                        "docs" => self.data.docs.iter().find(|d| d.id == oid).map(|d| d.title.clone()),
                        "wb" => self.data.extra.whiteboards.iter().find(|d| d.id == oid).map(|d| d.name.clone()),
                        "forms" => self.data.extra.forms.iter().find(|d| d.id == oid).map(|d| d.name.clone()),
                        _ => self.data.extra.dashboards.iter().find(|d| d.id == oid).map(|d| d.name.clone()),
                    };
                    if let Some(n) = name {
                        if ui.add(egui::Button::new(format!("\u{2605}  {}", n)).frame(false)).clicked() {
                            match id {
                                "docs" => self.doc_open = Some(oid),
                                "wb" => self.wb_open = Some(oid),
                                "forms" => self.form_open = Some(oid),
                                _ => self.dash_open = Some(oid),
                            }
                        }
                    }
                }
            }
            if id == "docs" {
                ui.add_space(14.0);
                ui.label(RichText::new("  Popular Wikis").size(13.0).color(t.text2));
                empty_card(ui, &t, "\u{2714}", "Most viewed and active Wikis appear here");
            }
            if id == "teams" {
                ui.add_space(4.0);
                let teams = self.data.extra.teams.clone();
                if teams.is_empty() {
                    empty_card(ui, &t, "\u{1F465}", "Once you join or create a Team you will see it here");
                }
                for tm in teams {
                    let sel = self.team_open == Some(tm.id);
                    let w = ui.available_width();
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                    if sel {
                        ui.painter().rect_filled(r, 7.0, t.row_selected);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 7.0, t.hover);
                    }
                    ui.painter().rect_filled(Rect::from_center_size(r.left_center() + vec2(16.0, 0.0), vec2(16.0, 16.0)), 4.0, rgb(tm.color));
                    ui.painter().text(r.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, &tm.name, FontId::proportional(14.0), t.text);
                    if resp.clicked() {
                        self.team_open = Some(tm.id);
                        self.hub_filter = 9;
                    }
                }
            }
        });
        (clicked, plus)
    }

    pub(super) fn page_bar(&mut self, ui: &mut Ui, title: &str, right: impl FnOnce(&mut Self, &mut Ui)) {
        let t = self.t();
        egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 12, top: 8, bottom: 8 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(28.0);
                ui.label(RichText::new(title).size(15.5).color(t.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| right(self, ui));
            });
        });
        let r = ui.available_rect_before_wrap();
        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
    }

    // ---------------------------------------------------------------- docs

    pub(super) fn docs_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let counts = [
            None,
            Some(self.data.docs.iter().filter(|d| d.owner == me && !d.archived).count()),
            None,
            None,
            None,
            None,
        ];
        let items: [(&str, &str, Option<usize>); 6] = [
            ("\u{1F4C4}", "All Docs", counts[0]),
            ("@me", "My Docs", counts[1]),
            ("\u{1F465}", "Shared with me", None),
            ("\u{1F512}", "Private", None),
            ("\u{1F4DD}", "Meeting Notes", None),
            ("\u{1F5D1}", "Archived", None),
        ];
        let sel = if self.doc_open.is_some() { 99 } else { self.hub_filter.min(5) };
        let (clicked, plus) = self.hub_sidebar(ui, "docs", "Docs", &items, sel, true);
        if let Some(i) = clicked {
            self.hub_filter = i;
            self.doc_open = None;
        }
        if plus {
            self.new_doc(self.current_space());
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| {
            if self.doc_open.is_some() {
                self.doc_editor(ui);
                return;
            }
            let title = items[self.hub_filter.min(5) as usize].1;
            self.page_bar(ui, title, |s, ui| {
                let tt = s.t();
                if primary_button(ui, &tt, "New Doc").clicked() {
                    s.new_doc(s.current_space());
                }
                if ui.add(egui::Button::new("\u{2B07}  Import").min_size(vec2(0.0, 28.0))).clicked() {
                    s.modal = Some(Modal::Import { path: String::new(), result: String::new() });
                }
            });
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
                    ui.label(RichText::new("Templates").color(t.text2));
                    ui.add_space(4.0);
                    let w = ui.available_width();
                    let cw = ((w - 24.0) / 3.0).min(430.0);
                    ui.horizontal(|ui| {
                        for (color, glyph, name, sub, body) in [
                            ([0xe1, 0x6b, 0x16], "\u{1F4CB}", "Project Overview", "Summarize goals, scope, and milestones", "Goals\n- \n\nScope\n- \n\nMilestones\n- \n\nTeam\n- "),
                            ([0xf8, 0xae, 0x00], "\u{1F4DD}", "Meeting Notes", "Capture an agenda, notes, and action items", "Agenda\n- \n\nNotes\n- \n\nAction items\n[ ] "),
                            ([0x44, 0x66, 0xff], "\u{1F4D3}", "Wiki", "Organize information in one place", "Overview\n\nHow we work\n\nLinks\n"),
                        ] {
                            let (r, resp) = ui.allocate_exact_size(vec2(cw, 70.0), Sense::click());
                            ui.painter().rect_filled(r, 10.0, if resp.hovered() { t.sidebar } else { t.card });
                            ui.painter().rect_stroke(r, 10.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                            let c = r.left_center() + vec2(32.0, 0.0);
                            ui.painter().circle_filled(c, 18.0, rgb(color).gamma_multiply(0.35));
                            ui.painter().text(c, Align2::CENTER_CENTER, glyph, FontId::proportional(14.0), rgb(color));
                            ui.painter().text(r.left_center() + vec2(62.0, -10.0), Align2::LEFT_CENTER, name, FontId::proportional(15.0), t.text);
                            ui.painter().with_clip_rect(r.shrink(4.0)).text(r.left_center() + vec2(62.0, 12.0), Align2::LEFT_CENTER, sub, FontId::proportional(12.5), t.text2);
                            if resp.clicked() {
                                self.new_doc(self.current_space());
                                if let Some(d) = self.doc_open.and_then(|id| self.data.docs.iter_mut().find(|d| d.id == id)) {
                                    d.title = name.to_string();
                                    d.body = body.to_string();
                                    d.meeting_note = name == "Meeting Notes";
                                }
                            }
                        }
                    });
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        let r = ui.add(egui::Button::new(if self.docs_mine { "\u{2261}  Filters \u{2022} 1" } else { "\u{2261}  Filters" }).corner_radius(13));
                        popup(ui, &r, ui.id().with("docs_filters"), 200.0, |ui| {
                            ui.checkbox(&mut self.docs_mine, "Only Docs I created");
                        });
                        let sr = ui.add(egui::Button::new(format!("\u{21C5}  Sort: {}", ["Date updated", "Name", "Date created"][self.docs_sort as usize % 3])).corner_radius(13));
                        popup(ui, &sr, ui.id().with("docs_sort"), 180.0, |ui| {
                            for (i, l) in ["Date updated", "Name", "Date created"].iter().enumerate() {
                                if ui.selectable_label(self.docs_sort == i as u8, *l).clicked() {
                                    self.docs_sort = i as u8;
                                    close_popup(ui);
                                }
                            }
                        });
                        ui.label(RichText::new("|  Tags:").color(t.text2));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("\u{1F50D} Search").desired_width(160.0));
                        });
                    });
                    ui.add_space(8.0);
                    let w = ui.available_width();
                    let cols = [0.0, 0.42, 0.62, 0.76, 0.88];
                    let (hr, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
                    for (x, h) in cols.iter().zip(["Name", "Location", "Tags", "Date updated", "Sharing"]) {
                        ui.painter().text(pos2(hr.left() + w * x + 30.0, hr.center().y), Align2::LEFT_CENTER, h, FontId::proportional(13.0), t.text2);
                    }
                    ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
                    let q = self.search.to_lowercase();
                    let filter = self.hub_filter;
                    let mine_only = self.docs_mine;
                    let sort = self.docs_sort;
                    let mut docs: Vec<Doc> = self
                        .data
                        .docs
                        .iter()
                        .filter(|d| match filter {
                            1 => d.owner == me && !d.archived,
                            2 => d.owner != me && !d.private && !d.archived,
                            3 => d.private && !d.archived,
                            4 => d.meeting_note && !d.archived,
                            5 => d.archived,
                            _ => !d.archived && (!d.private || d.owner == me),
                        })
                        .filter(|d| d.parent.is_none())
                        .filter(|d| q.is_empty() || d.title.to_lowercase().contains(&q))
                        .filter(|d| !mine_only || d.owner == me)
                        .cloned()
                        .collect();
                    match sort {
                        1 => docs.sort_by_key(|d| d.title.to_lowercase()),
                        2 => docs.sort_by_key(|d| std::cmp::Reverse(d.created)),
                        _ => docs.sort_by_key(|d| std::cmp::Reverse(d.updated)),
                    }
                    if docs.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| ui.label(RichText::new("No Docs here yet").color(t.text3)));
                    }
                    for d in docs {
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 0.0, t.hover);
                        }
                        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(0.5, t.border));
                        let ic = Rect::from_center_size(r.left_center() + vec2(44.0, 0.0), vec2(12.0, 15.0));
                        ui.painter().rect_filled(ic, 2.0, Color32::from_rgb(16, 144, 224));
                        ui.painter().text(r.left_center() + vec2(60.0, 0.0), Align2::LEFT_CENTER, &d.title, FontId::proportional(14.5), t.text);
                        let loc = d.space.and_then(|sid| self.data.spaces.iter().find(|s| s.id == sid)).map(|s| (s.name.clone(), s.color));
                        if let Some((n, c)) = loc {
                            let lr = Rect::from_center_size(pos2(r.left() + w * cols[1] + 38.0, r.center().y), vec2(16.0, 16.0));
                            ui.painter().rect_filled(lr, 4.0, rgb(c));
                            ui.painter().text(pos2(r.left() + w * cols[1] + 52.0, r.center().y), Align2::LEFT_CENTER, n, FontId::proportional(14.0), t.text2);
                        } else {
                            ui.painter().text(pos2(r.left() + w * cols[1] + 30.0, r.center().y), Align2::LEFT_CENTER, "Workspace", FontId::proportional(14.0), t.text2);
                        }
                        ui.painter().text(pos2(r.left() + w * cols[2] + 30.0, r.center().y), Align2::LEFT_CENTER, "\u{2013}", FontId::proportional(14.0), t.text3);
                        ui.painter().text(pos2(r.left() + w * cols[3] + 30.0, r.center().y), Align2::LEFT_CENTER, fmt_ago(d.updated), FontId::proportional(14.0), t.text2);
                        if let Some(m) = self.data.member(d.owner).or_else(|| self.data.member(me)) {
                            paint_avatar(ui.painter(), pos2(r.left() + w * cols[4] + 42.0, r.center().y), 11.0, m, t.bg);
                        }
                        if resp.clicked() {
                            self.doc_open = Some(d.id);
                        }
                    }
                });
            });
        });
    }

    fn doc_editor(&mut self, ui: &mut Ui) {
        let t = self.t();
        let Some(idx) = self.doc_open.and_then(|id| self.data.docs.iter().position(|d| d.id == id)) else {
            self.doc_open = None;
            return;
        };
        let id = self.data.docs[idx].id;
        let key = format!("doc:{}", id);
        let starred = self.data.extra.starred.contains(&key);
        let mut delete = false;
        egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 12, top: 8, bottom: 8 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new("\u{2190}  All Docs").color(t.text2)).frame(false)).clicked() {
                    self.doc_open = None;
                }
                let loc = self.data.docs[idx].space.and_then(|sid| self.data.spaces.iter().find(|s| s.id == sid)).map(|s| s.name.clone()).unwrap_or_else(|| "Workspace".into());
                ui.label(RichText::new(format!("/  {}", loc)).color(t.text3));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("Delete").color(t.danger)).frame(false)).clicked() {
                        delete = true;
                    }
                    let arch = if self.data.docs[idx].archived { "Unarchive" } else { "Archive" };
                    if ui.add(egui::Button::new(arch).frame(false)).on_hover_text("Move to Archived and save a Markdown copy").clicked() {
                        self.toggle_archive(id);
                    }
                    if ui.add(egui::Button::new("Export").frame(false)).on_hover_text("Save as a Markdown file").clicked() {
                        self.toast = Some(match self.export_doc(id) {
                            Some(p) => (format!("Saved {}", p.display()), now_ts()),
                            None => ("Couldn't save the file".into(), now_ts()),
                        });
                    }
                    let d = &mut self.data.docs[idx];
                    if ui.add(egui::Button::new(if d.private { "\u{1F512} Private" } else { "\u{1F465} Shared" }).frame(false)).on_hover_text("Who can see this Doc").clicked() {
                        d.private = !d.private;
                        self.dirty = true;
                    }
                    let star = RichText::new(if starred { "\u{2605}" } else { "\u{2606}" }).size(16.0).color(if starred { Color32::from_rgb(0xf8, 0xae, 0x00) } else { t.text2 });
                    if ui.add(egui::Button::new(star).frame(false)).clicked() {
                        if starred {
                            self.data.extra.starred.retain(|s| s != &key);
                        } else {
                            self.data.extra.starred.push(key.clone());
                        }
                        self.dirty = true;
                    }
                    ui.label(RichText::new(format!("Last updated {}", fmt_ago(self.data.docs[idx].updated))).small().color(t.text3));
                });
            });
        });
        let r = ui.available_rect_before_wrap();
        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
        self.doc_body(ui, idx);
        if delete {
            let d = self.data.docs[idx].clone();
            let tid = self.data.new_id();
            self.data.extra.trash.push(TrashItem { id: tid, kind: "Doc".into(), title: d.title.clone(), deleted: now_ts(), payload: serde_json::to_string(&d).unwrap_or_default() });
            self.data.docs.remove(idx);
            for x in self.data.docs.iter_mut().filter(|x| x.parent == Some(d.id)) {
                x.parent = d.parent;
            }
            self.doc_open = d.parent;
            self.dirty = true;
        }
    }

    // ---------------------------------------------------------------- dashboards

    pub(super) fn dashboards_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let items: [(&str, &str, Option<usize>); 4] = [("\u{1F4CA}", "All Dashboards", None), ("@me", "My Dashboards", None), ("\u{1F465}", "Shared with me", None), ("\u{1F512}", "Private", None)];
        let sel = if self.dash_open.is_some() { 99 } else { self.hub_filter.min(3) };
        let (clicked, plus) = self.hub_sidebar(ui, "dash", "Dashboards", &items, sel, false);
        if let Some(i) = clicked {
            self.hub_filter = i;
            self.dash_open = None;
        }
        if plus {
            self.modal = Some(Modal::NewDashboard { name: "New Dashboard".into() });
        }
        let _ = me;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| {
            if self.dash_open.is_some() || (self.hub_filter == 0 && !self.data.extra.dashboards.is_empty() && self.dash_open.is_none() && false) {
                self.dashboard_content(ui);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if !self.data.extra.dashboards.is_empty() {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                        ui.label(RichText::new("Your Dashboards").size(15.0));
                        for d in self.data.extra.dashboards.clone() {
                            if ui.add(egui::Button::new(format!("\u{1F4CA}  {}", d.name)).frame(false)).clicked() {
                                self.dash_open = Some(d.id);
                            }
                        }
                        if ui.add(egui::Button::new("\u{1F4CA}  Workspace overview").frame(false)).clicked() {
                            let id = self.data.new_id();
                            self.data.extra.dashboards.push(Dashboard { id, name: "Workspace overview".into(), space: None });
                            self.dash_open = Some(id);
                            self.dirty = true;
                        }
                    });
                }
                let choice = self.gallery(ui, "Choose a Dashboard template", "Get started with a Dashboard template or create a custom Dashboard\nto fit your exact needs.", &[
                    ([0x44, 0x66, 0xff], "\u{1F4CA}", "Simple Dashboard", "Manage and prioritize tasks", false),
                    ([0xb6, 0x60, 0xe0], "\u{2728}", "AI Team Center", "View team activity with AI", true),
                    ([0x0f, 0x9d, 0x9f], "\u{23F1}", "Time Tracking", "View and report on time tracking metrics", false),
                    ([0x10, 0x90, 0xe0], "\u{2630}", "Project Management", "Analyze project progress and metrics", false),
                    ([0xb6, 0x60, 0xe0], "\u{2728}", "AI Personal Center", "Improve your performance and career with AI cards", true),
                ]);
                if let Some(name) = choice {
                    if name.starts_with("AI ") {
                        self.modal = Some(super::menus::unavailable(&name));
                    } else {
                        let id = self.data.new_id();
                        let n = if name == "Start from scratch" { "New Dashboard".to_string() } else { name };
                        self.data.extra.dashboards.push(Dashboard { id, name: n, space: None });
                        self.dash_open = Some(id);
                        self.dirty = true;
                    }
                }
            });
        });
    }

    /// Template gallery with a "Start from scratch" card; returns the chosen title.
    fn gallery(&mut self, ui: &mut Ui, title: &str, sub: &str, cards: &[([u8; 3], &str, &str, &str, bool)]) -> Option<String> {
        let t = self.t();
        let mut out = None;
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(title).size(22.0).color(t.text));
            ui.add_space(6.0);
            ui.label(RichText::new(sub).size(15.0).color(t.text2));
        });
        ui.add_space(24.0);
        let w = ui.available_width();
        let cw = 245.0f32.max(((w - 120.0) / 3.0).min(245.0));
        let total = cw * 3.0 + 28.0;
        let mut all: Vec<([u8; 3], &str, &str, &str, bool)> = cards.to_vec();
        all.push(([0, 0, 0], "+", "Start from scratch", "", false));
        for row in all.chunks(3) {
            ui.horizontal(|ui| {
                ui.add_space(((w - total) / 2.0).max(0.0));
                ui.spacing_mut().item_spacing.x = 14.0;
                for (color, glyph, name, desc, ai) in row {
                    if gallery_card(ui, &t, cw, *color, glyph, name, desc, *ai).clicked() {
                        out = Some(name.to_string());
                    }
                }
            });
            ui.add_space(14.0);
        }
        out
    }

    // ---------------------------------------------------------------- whiteboards

    pub(super) fn whiteboards_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let items: [(&str, &str, Option<usize>); 2] = [("\u{1F3A8}", "All Whiteboards", None), ("@me", "My Whiteboards", None)];
        let sel = if self.wb_open.is_some() { 99 } else { self.hub_filter.min(1) };
        let (clicked, plus) = self.hub_sidebar(ui, "wb", "Whiteboards", &items, sel, false);
        if let Some(i) = clicked {
            self.hub_filter = i;
            self.wb_open = None;
        }
        if plus {
            self.new_whiteboard();
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| {
            if self.wb_open.is_some() {
                self.whiteboard_canvas(ui);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if !self.data.extra.whiteboards.is_empty() {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                        ui.label(RichText::new("Your Whiteboards").size(15.0));
                        for w in self.data.extra.whiteboards.clone() {
                            if ui.add(egui::Button::new(format!("\u{1F3A8}  {}", w.name)).frame(false)).clicked() {
                                self.wb_open = Some(w.id);
                            }
                        }
                    });
                }
                let choice = self.gallery(ui, "Choose a Whiteboard template", "Create Whiteboards to map out project plans, brainstorm feature\nconcepts, or help remote co-workers work better together!", &[
                    ([0xb6, 0x60, 0xe0], "\u{1F465}", "Organizational Chart", "Visualize your team structure", false),
                    ([0x0f, 0x9d, 0x9f], "\u{2630}", "Action Plan", "Turn goals into actionable steps", false),
                    ([0x10, 0x90, 0xe0], "\u{27A4}", "Customer Journey Map", "Optimize every customer touchpoint", false),
                    ([0xee, 0x5e, 0x99], "\u{25A6}", "Flow Chart", "Structure and communicate processes and workflows", false),
                    ([0xe1, 0x6b, 0x16], "\u{25CE}", "Root Cause Analysis", "Analyze issues using the Ishikawa Fishbone diagram", false),
                ]);
                if let Some(name) = choice {
                    self.new_whiteboard();
                    let notes: Vec<(&str, [f32; 2])> = match name.as_str() {
                        "Organizational Chart" => vec![("CEO", [360.0, 40.0]), ("CTO", [160.0, 230.0]), ("COO", [360.0, 230.0]), ("CMO", [560.0, 230.0])],
                        "Action Plan" => vec![("Goal", [40.0, 40.0]), ("Step 1", [240.0, 40.0]), ("Step 2", [440.0, 40.0]), ("Step 3", [640.0, 40.0]), ("Owner / due date", [40.0, 220.0])],
                        "Customer Journey Map" => vec![("Awareness", [40.0, 40.0]), ("Consideration", [220.0, 40.0]), ("Purchase", [400.0, 40.0]), ("Retention", [580.0, 40.0]), ("Advocacy", [760.0, 40.0])],
                        "Flow Chart" => vec![("Start", [40.0, 120.0]), ("Step", [240.0, 120.0]), ("Decision?", [440.0, 120.0]), ("End", [640.0, 120.0])],
                        "Root Cause Analysis" => vec![("Problem", [680.0, 160.0]), ("People", [60.0, 40.0]), ("Process", [300.0, 40.0]), ("Tools", [60.0, 300.0]), ("Environment", [300.0, 300.0])],
                        _ => vec![],
                    };
                    if let Some(w) = self.wb_open.and_then(|id| self.data.extra.whiteboards.iter_mut().find(|w| w.id == id)) {
                        if name != "Start from scratch" {
                            w.name = name.clone();
                        }
                        for (i, (txt, pos)) in notes.into_iter().enumerate() {
                            w.notes.push(Sticky { pos, text: txt.to_string(), color: STATUS_PALETTE[1 + (i * 2) % 13] });
                        }
                    }
                }
            });
        });
    }

    // ---------------------------------------------------------------- forms

    pub(super) fn forms_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let items: [(&str, &str, Option<usize>); 2] = [("\u{1F4CB}", "All Forms", None), ("@me", "My Forms", None)];
        let sel = if self.form_open.is_some() { 99 } else { self.hub_filter.min(1) };
        let (clicked, plus) = self.hub_sidebar(ui, "forms", "Forms", &items, sel, false);
        if let Some(i) = clicked {
            self.hub_filter = i;
            self.form_open = None;
        }
        if plus {
            self.new_form();
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| {
            if self.form_open.is_some() {
                self.form_detail(ui);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if !self.data.extra.forms.is_empty() {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                        ui.label(RichText::new("Your Forms").size(15.0));
                        for f in self.data.extra.forms.clone() {
                            if ui.add(egui::Button::new(format!("\u{1F4CB}  {}", f.name)).frame(false)).clicked() {
                                self.form_open = Some(f.id);
                            }
                        }
                    });
                }
                let choice = self.gallery(ui, "Choose a Form template", "Create forms to kick off projects, collect feedback, and supercharge\nyour productivity.", &[
                    ([0x0f, 0x9d, 0x9f], "\u{1F44D}", "Feedback Form", "Survey and collect feedback", false),
                    ([0xee, 0x5e, 0x99], "\u{1F4CB}", "Project Intake", "Streamline new project requests", false),
                    ([0x5f, 0x55, 0xee], "\u{1F4BC}", "Order Form", "Capture and process client orders", false),
                    ([0xe1, 0x6b, 0x16], "\u{1F464}", "Job Application", "Accept and review applications for open roles", false),
                    ([0x44, 0x66, 0xff], "\u{2699}", "IT Requests", "Triage and prioritize IT service requests", false),
                ]);
                if let Some(name) = choice {
                    self.new_form();
                    let fields: Vec<(&str, &str)> = match name.as_str() {
                        "Feedback Form" => vec![("Your name", "name"), ("How was your experience?", "text"), ("Comments", "description")],
                        "Project Intake" => vec![("Project name", "name"), ("Goal", "description"), ("Deadline", "due"), ("Priority", "priority")],
                        "Order Form" => vec![("Customer", "name"), ("Items", "description"), ("Quantity", "text"), ("Needed by", "due")],
                        "Job Application" => vec![("Full name", "name"), ("Role", "text"), ("Experience", "description"), ("Links", "text")],
                        "IT Requests" => vec![("Issue", "name"), ("Urgency", "priority"), ("Details", "description")],
                        _ => vec![],
                    };
                    if let Some(f) = self.form_open.and_then(|id| self.data.extra.forms.iter_mut().find(|f| f.id == id)) {
                        if name != "Start from scratch" {
                            f.name = name.clone();
                            f.fields = fields.into_iter().map(|(l, m)| FormField { label: l.to_string(), map: m.to_string() }).collect();
                        }
                    }
                }
            });
        });
    }

    // ---------------------------------------------------------------- goals

    pub(super) fn goals_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        if !self.data.extra.goals.is_empty() {
            self.goals_page(ui);
            return;
        }
        let r = ui.max_rect();
        ui.painter().rect_filled(r, egui::CornerRadius { nw: 8, ne: 8, sw: 8, se: 8 }, Color32::from_rgb(243, 244, 247));
        ui.add_space(110.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Make your goals a reality.").size(26.0).color(t.text2));
            ui.add_space(8.0);
            ui.label(RichText::new("Create Goals, break them down into measurable targets, and track progress in real-time.").size(16.0).color(t.text3));
            ui.add_space(30.0);
            let (pr, _) = ui.allocate_exact_size(vec2(480.0, 300.0), Sense::hover());
            ui.painter().rect_filled(pr, 6.0, Color32::WHITE);
            ui.painter().rect_stroke(pr, 6.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            let band = Rect::from_min_size(pr.min + vec2(0.0, 34.0), vec2(pr.width(), 100.0));
            ui.painter().rect_filled(band, 0.0, Color32::from_rgb(0xe3, 0x9b, 0x6f));
            let card = Rect::from_min_size(pr.min + vec2(66.0, 76.0), vec2(pr.width() - 132.0, 120.0));
            ui.painter().rect_filled(card, 3.0, Color32::WHITE);
            ui.painter().rect_stroke(card, 3.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            for (i, c) in [[0x3d, 0xb8, 0x8b], [0x44, 0x66, 0xff], [0xe1, 0x6b, 0x16]].iter().enumerate() {
                let y = card.top() + 28.0 + i as f32 * 32.0;
                ui.painter().circle_filled(pos2(card.left() + 18.0, y), 8.0, rgb(*c));
                ui.painter().rect_filled(Rect::from_min_size(pos2(card.left() + 34.0, y - 3.0), vec2(120.0, 6.0)), 3.0, t.hover);
                ui.painter().rect_filled(Rect::from_min_size(pos2(card.right() - 110.0, y - 2.0), vec2(96.0, 4.0)), 2.0, t.hover);
            }
            ui.painter().circle_stroke(pr.min + vec2(80.0, 60.0), 14.0, Stroke::new(3.0, Color32::WHITE));
            ui.painter().text(pr.min + vec2(80.0, 60.0), Align2::CENTER_CENTER, "30%", FontId::proportional(8.0), Color32::WHITE);
            ui.add_space(30.0);
            if ui.add(egui::Button::new(RichText::new("Set a Goal").size(16.0).color(Color32::WHITE)).fill(t.primary).corner_radius(6).min_size(vec2(124.0, 42.0))).clicked() {
                let id = self.data.new_id();
                self.data.extra.goals.push(Goal { id, name: "New goal".into(), color: STATUS_PALETTE[1], due: None, owner: self.data.me, targets: vec![] });
                self.goal_open = Some(id);
                self.dirty = true;
            }
            ui.add_space(8.0);
            if ui.link(RichText::new("Learn more").color(t.text2)).clicked() {
                self.modal = Some(Modal::Unavailable { title: "Goals".into(), body: "Create a Goal, add targets (a number, done / not done, or a set of tasks) and the progress bar fills in as you go.".into() });
            }
        });
    }

    // ---------------------------------------------------------------- teams

    pub(super) fn teams_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let people = self.data.members.len();
        let items: [(&str, &str, Option<usize>); 4] = [("\u{1F465}", "All Teams", None), ("\u{1F4C7}", "All People", Some(people)), ("\u{2630}", "Org Chart", None), ("\u{1F4CA}", "Analytics", None)];
        let sel = if self.hub_filter == 9 { 99 } else { self.hub_filter.min(3) };
        let (clicked, plus) = self.hub_sidebar(ui, "teams", "Teams", &items, sel, true);
        if let Some(i) = clicked {
            self.hub_filter = i;
            self.team_open = None;
        }
        if plus {
            self.new_team();
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| match self.hub_filter {
            1 => self.people_page(ui),
            2 => self.org_chart(ui),
            3 => self.teams_workload(ui),
            9 => self.team_detail(ui),
            _ => self.all_teams(ui),
        });
    }

    fn new_team(&mut self) {
        let id = self.data.new_id();
        let n = self.data.extra.teams.len();
        self.data.extra.teams.push(Team { id, name: format!("Team {}", n + 1), members: vec![self.data.me], color: STATUS_PALETTE[1 + (n * 3) % 13] });
        self.team_open = Some(id);
        self.hub_filter = 9;
        self.dirty = true;
    }

    fn all_teams(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.page_bar(ui, "All Teams", |s, ui| {
            let tt = s.t();
            if primary_button(ui, &tt, "Create Team").clicked() {
                s.new_team();
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                let teams = self.data.extra.teams.clone();
                if teams.is_empty() {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Manage teams and see their work").size(24.0));
                        ui.label(RichText::new("Coordinate teams, organize members and see the details of their work.").color(t.text2));
                        ui.add_space(12.0);
                        if primary_button(ui, &t, "Create Team").clicked() {
                            self.new_team();
                        }
                    });
                }
                for tm in teams {
                    let resp = egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                            ui.painter().rect_filled(r, 5.0, rgb(tm.color));
                            ui.label(RichText::new(&tm.name).size(16.0));
                            ui.label(RichText::new(format!("{} members", tm.members.len())).color(t.text2));
                            let (r, _) = ui.allocate_exact_size(vec2(90.0, 22.0), Sense::hover());
                            paint_avatars(ui.painter(), r.left_center(), 10.0, &tm.members, &self.data, t.card);
                        });
                    });
                    if ui.interact(resp.response.rect, ui.id().with(("team", tm.id)), Sense::click()).clicked() {
                        self.team_open = Some(tm.id);
                        self.hub_filter = 9;
                    }
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn team_detail(&mut self, ui: &mut Ui) {
        let t = self.t();
        let Some(ti) = self.team_open.and_then(|id| self.data.extra.teams.iter().position(|x| x.id == id)) else {
            self.hub_filter = 0;
            return;
        };
        let members = self.data.members.clone();
        let mut delete = false;
        egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
            let tm = &mut self.data.extra.teams[ti];
            if ui.add(egui::TextEdit::singleline(&mut tm.name).font(FontId::proportional(24.0)).frame(false).desired_width(400.0)).changed() {
                self.dirty = true;
            }
            ui.label(RichText::new("Members").color(t.text2));
            ui.horizontal_wrapped(|ui| {
                for m in &members {
                    let mut on = tm.members.contains(&m.id);
                    if ui.checkbox(&mut on, &m.name).changed() {
                        if on {
                            tm.members.push(m.id);
                        } else {
                            tm.members.retain(|x| *x != m.id);
                        }
                        self.dirty = true;
                    }
                }
            });
            if ui.add(egui::Button::new(RichText::new("Delete team").color(t.danger)).frame(false)).clicked() {
                delete = true;
            }
        });
        ui.separator();
        let ids = self.data.extra.teams[ti].members.clone();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 8)).show(ui, |ui| {
                ui.label(RichText::new("What the team is working on").size(15.0));
                for id in ids {
                    let Some(m) = self.data.member(id).cloned() else { continue };
                    let open: Vec<Task> = self.data.tasks.iter().filter(|x| x.assignees.contains(&id) && !self.data.is_closed(x)).cloned().collect();
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                        paint_avatar(ui.painter(), r.center(), 11.0, &m, t.bg);
                        ui.label(RichText::new(&m.name).strong());
                        ui.label(RichText::new(format!("{} open tasks", open.len())).color(t.text2));
                    });
                    for task in open.iter().take(6) {
                        if ui.link(format!("      {}", task.name)).clicked() {
                            self.actions.push(Action::Open(task.id));
                        }
                    }
                    ui.add_space(6.0);
                }
            });
        });
        if delete {
            self.data.extra.teams.remove(ti);
            self.team_open = None;
            self.hub_filter = 0;
            self.dirty = true;
        }
    }

    fn org_chart(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.page_bar(ui, "Org Chart", |_, _| {});
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 24)).show(ui, |ui| {
                for role in ["Owner", "Admin", "Member", "Guest"] {
                    let people: Vec<Member> = self.data.members.iter().filter(|m| (if m.role.is_empty() { "Member" } else { m.role.as_str() }) == role).cloned().collect();
                    if people.is_empty() {
                        continue;
                    }
                    ui.label(RichText::new(format!("{}s", role)).color(t.text2));
                    ui.horizontal_wrapped(|ui| {
                        for m in people {
                            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                                ui.set_width(170.0);
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                                    paint_avatar(ui.painter(), r.center(), 13.0, &m, t.card);
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(&m.name).strong());
                                        ui.label(RichText::new(if m.invited { "Invited".to_string() } else { role.to_string() }).small().color(t.text3));
                                    });
                                });
                            });
                        }
                    });
                    ui.add_space(12.0);
                }
            });
        });
    }

    // ---------------------------------------------------------------- timesheets

    pub(super) fn timesheets_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let ws = self.sheet_week;
        let me = self.data.me;
        // tabs bar
        egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 12, top: 6, bottom: 0 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_height(40.0);
                ui.label(RichText::new("\u{23F1}  Timesheets").size(15.5));
                ui.separator();
                for (i, label) in ["My timesheet", "All timesheets", "Approvals"].iter().enumerate() {
                    let sel = self.hub_filter.min(2) == i as u8;
                    let g = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(15.0), if sel { t.text } else { t.text3 });
                    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 16.0, 40.0), Sense::click());
                    name_it(&resp, label);
                    ui.painter().galley(r.center() - g.size() / 2.0, g, if sel { t.text } else { t.text3 });
                    if sel {
                        ui.painter().rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 2.0), r.max), 1.0, t.text);
                    }
                    if resp.clicked() {
                        self.hub_filter = i as u8;
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new("\u{2699}  Settings").min_size(vec2(0.0, 28.0))).clicked() {
                        self.modal = Some(Modal::ClickApps);
                    }
                });
            });
        });
        let r = ui.available_rect_before_wrap();
        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
        let bg = Color32::from_rgb(249, 249, 249);
        ui.painter().rect_filled(Rect::from_min_max(r.min + vec2(0.0, 1.0), r.max), egui::CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 }, bg);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 18)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new("<").size(16.0)).frame(false)).clicked() {
                        self.sheet_week = ws - Duration::days(7);
                    }
                    if ui.add(egui::Button::new(RichText::new(">").size(16.0)).frame(false)).clicked() {
                        self.sheet_week = ws + Duration::days(7);
                    }
                    let r = ui.add(egui::Button::new(RichText::new(format!("{} - {}  \u{25BE}", ws.format("%b %-d"), (ws + Duration::days(6)).format("%b %-d"))).size(20.0)).frame(false));
                    if r.clicked() {
                        let td = today();
                        self.sheet_week = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
                    }
                    if self.hub_filter == 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.selectable_value(&mut self.sheet_entries, true, "\u{2630}  Time entries");
                            ui.selectable_value(&mut self.sheet_entries, false, "\u{25A4}  Timesheet");
                        });
                    }
                });
                ui.add_space(14.0);
                match self.hub_filter {
                    1 => self.all_timesheets(ui, ws),
                    2 => {
                        egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(30)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("No timesheets waiting for approval").size(16.0));
                                ui.label(RichText::new("When people submit their week, it shows here for you to approve.").color(t.text2));
                            });
                        });
                    }
                    _ => {
                        let has_any = self.data.tasks.iter().any(|x| x.time_entries.iter().any(|e| { let i = (ts_day(e.start) - ws).num_days(); (0..7).contains(&i) }) || x.timer_started.is_some()) || !self.sheet_rows.is_empty() || self.sheet_force;
                        if !has_any {
                            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::symmetric(30, 40)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.vertical_centered(|ui| {
                                    ui.label(RichText::new("\u{23F1}").size(44.0).color(t.text3));
                                    ui.label(RichText::new("Add entries to this week's timesheet").size(17.0));
                                    ui.add_space(14.0);
                                });
                                let w = ui.available_width();
                                let cw = 200.0;
                                ui.horizontal(|ui| {
                                    ui.add_space(((w - 4.0 * cw - 30.0) / 2.0).max(0.0));
                                    for (icon, label) in [("\u{1F465}", "All assigned tasks"), ("\u{25A4}", "Last week's tasks"), ("+", "Individual tasks"), ("\u{23F1}", "Track time")] {
                                        let (r, resp) = ui.allocate_exact_size(vec2(cw, 84.0), Sense::click());
                                        ui.painter().rect_filled(r, 10.0, if resp.hovered() { t.sidebar } else { t.card });
                                        ui.painter().rect_stroke(r, 10.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                                        let ib = Rect::from_min_size(r.min + vec2(14.0, 14.0), vec2(28.0, 28.0));
                                        ui.painter().rect_filled(ib, 6.0, t.hover);
                                        ui.painter().text(ib.center(), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
                                        ui.painter().text(r.left_top() + vec2(14.0, 62.0), Align2::LEFT_CENTER, label, FontId::proportional(13.5), t.text);
                                        if resp.clicked() {
                                            match label {
                                                "All assigned tasks" => {
                                                    self.sheet_rows = self.data.tasks.iter().filter(|x| x.assignees.contains(&me) && !self.data.is_closed(x)).map(|x| x.id).collect();
                                                }
                                                "Last week's tasks" => {
                                                    let lw = ws - Duration::days(7);
                                                    self.sheet_rows = self.data.tasks.iter().filter(|x| x.time_entries.iter().any(|e| { let i = (ts_day(e.start) - lw).num_days(); (0..7).contains(&i) })).map(|x| x.id).collect();
                                                    if self.sheet_rows.is_empty() {
                                                        self.toast = Some(("No time was tracked last week".into(), now_ts()));
                                                    }
                                                }
                                                "Individual tasks" => {
                                                    // show the grid; pick tasks with "+ Add task"
                                                    self.sheet_force = true;
                                                }
                                                _ => {
                                                    let first = self.data.tasks.iter().find(|x| x.assignees.contains(&me)).map(|x| x.id).unwrap_or(0);
                                                    self.modal = Some(Modal::TrackTime { task: first });
                                                }
                                            }
                                        }
                                    }
                                });
                            });
                        } else if self.sheet_entries {
                            self.time_entries_list(ui, ws);
                        } else {
                            self.timesheet_grid(ui, ws);
                        }
                    }
                }
            });
        });
    }

    fn timesheet_grid(&mut self, ui: &mut Ui, ws: NaiveDate) {
        let t = self.t();
        let now = now_ts();
        let mut ids: Vec<u64> = self.sheet_rows.clone();
        for x in &self.data.tasks {
            let any = x.time_entries.iter().any(|e| {
                let i = (ts_day(e.start) - ws).num_days();
                (0..7).contains(&i)
            }) || x.timer_started.is_some();
            if any && !ids.contains(&x.id) {
                ids.push(x.id);
            }
        }
        egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            let w = ui.available_width();
            let col = 92.0;
            let name_w = (w - col * 8.0).max(200.0);
            let (hr, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
            ui.painter().text(hr.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, "Task", FontId::proportional(13.0), t.text2);
            for i in 0..7 {
                let d = ws + Duration::days(i);
                let x = hr.left() + name_w + col * i as f32 + col / 2.0;
                ui.painter().text(pos2(x, hr.center().y), Align2::CENTER_CENTER, d.format("%a %-d").to_string(), FontId::proportional(12.5), if d == today() { t.danger } else { t.text2 });
            }
            ui.painter().text(pos2(hr.left() + name_w + col * 7.5, hr.center().y), Align2::CENTER_CENTER, "Total", FontId::proportional(12.5), t.text2);
            ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, t.border));
            let mut totals = [0i64; 7];
            for id in &ids {
                let Some(ti) = self.data.tasks.iter().position(|x| x.id == *id) else { continue };
                let name = self.data.tasks[ti].name.clone();
                let mut per = [0i64; 7];
                for e in &self.data.tasks[ti].time_entries {
                    let i = (ts_day(e.start) - ws).num_days();
                    if (0..7).contains(&i) {
                        per[i as usize] += e.end - e.start;
                    }
                }
                if let Some(st) = self.data.tasks[ti].timer_started {
                    let i = (ts_day(st) - ws).num_days();
                    if (0..7).contains(&i) {
                        per[i as usize] += now - st;
                    }
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let (nr, nresp) = ui.allocate_exact_size(vec2(name_w, 34.0), Sense::click());
                    ui.painter().with_clip_rect(nr).text(nr.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, &name, FontId::proportional(14.0), t.text);
                    if nresp.clicked() {
                        self.actions.push(Action::Open(*id));
                    }
                    let mut sum = 0;
                    for i in 0..7 {
                        totals[i] += per[i];
                        sum += per[i];
                        let mut hours = per[i] as f64 / 3600.0;
                        let r = ui.add_sized([col - 8.0, 26.0], egui::DragValue::new(&mut hours).range(0.0..=24.0).speed(0.05).max_decimals(2).suffix("h"));
                        ui.add_space(8.0);
                        if r.changed() {
                            let tid = self.data.tasks[ti].id;
                            self.set_day_time(tid, ws + Duration::days(i as i64), hours);
                        }
                    }
                    let (tr, _) = ui.allocate_exact_size(vec2(col, 34.0), Sense::hover());
                    ui.painter().text(tr.center(), Align2::CENTER_CENTER, fmt_duration(sum), FontId::proportional(13.0), t.text);
                });
            }
            let (fr, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
            ui.painter().hline(fr.x_range(), fr.top(), Stroke::new(1.0, t.border));
            ui.painter().text(fr.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, "Total", FontId::proportional(13.5), t.text);
            let mut all = 0;
            for (i, v) in totals.iter().enumerate() {
                all += v;
                ui.painter().text(pos2(fr.left() + name_w + col * i as f32 + col / 2.0 - 4.0, fr.center().y), Align2::CENTER_CENTER, if *v > 0 { fmt_duration(*v) } else { "-".into() }, FontId::proportional(12.5), t.text2);
            }
            ui.painter().text(pos2(fr.left() + name_w + col * 7.5, fr.center().y), Align2::CENTER_CENTER, fmt_duration(all), FontId::proportional(13.0), t.text);
            ui.horizontal(|ui| {
                let tasks: Vec<(u64, String)> = self.data.tasks.iter().filter(|x| !ids.contains(&x.id)).map(|x| (x.id, x.name.clone())).collect();
                ui.menu_button("+ Add task", |ui| {
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        for (id, n) in &tasks {
                            if ui.button(n).clicked() {
                                self.sheet_rows.push(*id);
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
        });
    }

    /// Replaces a task's time on one day with a single entry of `hours`.
    pub(super) fn set_day_time(&mut self, task_id: u64, day: NaiveDate, hours: f64) {
        let Some(task) = self.data.tasks.iter_mut().find(|x| x.id == task_id) else { return };
        task.time_entries.retain(|e| ts_day(e.start) != day);
        if hours > 0.0 {
            let start = day.and_hms_opt(9, 0, 0).and_then(|x| x.and_local_timezone(chrono::Local).single()).map(|x| x.timestamp()).unwrap_or_else(now_ts);
            task.time_entries.push(TimeEntry { start, end: start + (hours * 3600.0).round() as i64 });
        }
        self.dirty = true;
    }

    fn time_entries_list(&mut self, ui: &mut Ui, ws: NaiveDate) {
        let t = self.t();
        let mut rows: Vec<(i64, i64, String, u64, usize)> = vec![];
        for x in &self.data.tasks {
            for (k, e) in x.time_entries.iter().enumerate() {
                let i = (ts_day(e.start) - ws).num_days();
                if (0..7).contains(&i) {
                    rows.push((e.start, e.end, x.name.clone(), x.id, k));
                }
            }
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r.0));
        egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let mut remove = None;
            for (s, e, name, id, k) in &rows {
                ui.horizontal(|ui| {
                    let when = chrono::DateTime::from_timestamp(*s, 0).map(|d| d.with_timezone(&chrono::Local).format("%a %b %-d, %-I:%M %p").to_string()).unwrap_or_default();
                    ui.label(RichText::new(when).color(t.text2));
                    if ui.link(name).clicked() {
                        self.actions.push(Action::Open(*id));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("\u{2716}").color(t.text3)).frame(false)).clicked() {
                            remove = Some((*id, *k));
                        }
                        ui.label(fmt_duration(e - s));
                    });
                });
            }
            if rows.is_empty() {
                ui.label(RichText::new("No time entries this week").color(t.text3));
            }
            if let Some((id, k)) = remove {
                if let Some(task) = self.data.tasks.iter_mut().find(|x| x.id == id) {
                    if k < task.time_entries.len() {
                        task.time_entries.remove(k);
                        self.dirty = true;
                    }
                }
            }
        });
    }

    fn all_timesheets(&mut self, ui: &mut Ui, ws: NaiveDate) {
        let t = self.t();
        egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for m in self.data.members.clone() {
                let mut total = 0;
                for x in self.data.tasks.iter().filter(|x| x.assignees.contains(&m.id)) {
                    for e in &x.time_entries {
                        let i = (ts_day(e.start) - ws).num_days();
                        if (0..7).contains(&i) {
                            total += e.end - e.start;
                        }
                    }
                }
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                    paint_avatar(ui.painter(), r.center(), 11.0, &m, t.card);
                    ui.label(&m.name);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(if total > 0 { fmt_duration(total) } else { "-".into() });
                    });
                });
            }
        });
    }
}

pub(super) fn ts_day(ts: i64) -> NaiveDate {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.with_timezone(&chrono::Local).date_naive()).unwrap_or_else(today)
}
