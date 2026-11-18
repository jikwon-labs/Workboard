// App "hubs" as the person's ClickUp shows them: each app (Docs, Dashboards,
// Whiteboards, Forms, Teams) has its own sidebar (All / My / Shared / Private,
// Favorites) and a template gallery; Goals has a start page; Timesheets has
// tabs (My timesheet, All timesheets) and editable hours.

use super::icons::{icon_button, icon_button_ex, line_icon, line_icon_scaled};
use super::*;

/// `dash_open` for the built-in Workspace overview (not a saved dashboard).
pub(super) const WORKSPACE_OVERVIEW: u64 = 0;

fn hub_frame(t: &Theme) -> egui::Frame {
    egui::Frame::new()
        .fill(t.sidebar)
        .corner_radius(egui::CornerRadius {
            nw: 8,
            ne: 0,
            sw: 8,
            se: 0,
        })
        .inner_margin(egui::Margin::symmetric(8, 6))
}

/// A template card as in the galleries: round coloured icon (`glyph` is a
/// line icon kind, or "+"), title, description.
fn gallery_card(
    ui: &mut Ui,
    t: &Theme,
    w: f32,
    color: [u8; 3],
    glyph: &str,
    title: &str,
    sub: &str,
    ai: bool,
) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 142.0), Sense::click());
    name_it(&resp, title);
    ui.painter()
        .rect_filled(r, 10.0, if resp.hovered() { t.sidebar } else { t.card });
    ui.painter().rect_stroke(
        r,
        10.0,
        Stroke::new(1.0_f32, t.border),
        egui::StrokeKind::Inside,
    );
    let c = r.left_top() + vec2(38.0, 36.0);
    if glyph == "+" {
        ui.painter().circle_filled(c, 18.0, t.hover);
        ui.painter().text(
            c,
            Align2::CENTER_CENTER,
            "+",
            FontId::proportional(22.0),
            t.text2,
        );
    } else {
        // a pale tile of the colour with the icon in the full colour (the
        // emoji sat on a tile of nearly its own colour and hardly showed)
        ui.painter()
            .circle_filled(c, 18.0, rgb(color).gamma_multiply(0.14));
        line_icon_scaled(ui.painter(), glyph, c, rgb(color), 1.15);
    }
    let tc = if ai {
        Color32::from_rgb(0x9b, 0x2d, 0xd6)
    } else {
        t.text
    };
    ui.painter().text(
        r.left_top() + vec2(20.0, 82.0),
        Align2::LEFT_CENTER,
        tr(title),
        FontId::proportional(15.0),
        tc,
    );
    let g = ui.painter().layout(
        tr(sub).to_string(),
        FontId::proportional(13.0),
        t.text2,
        w - 40.0,
    );
    ui.painter()
        .galley(r.left_top() + vec2(20.0, 96.0), g, t.text2);
    resp
}

/// The small "Wiki" label after a doc's title.
pub(super) fn wiki_badge(p: &egui::Painter, t: &Theme, left_center: egui::Pos2) {
    let c = Color32::from_rgb(0x44, 0x66, 0xff);
    let g = p.layout_no_wrap(tr("Wiki").to_string(), FontId::proportional(11.0), c);
    let r = Rect::from_min_size(
        pos2(left_center.x + 8.0, left_center.y - 9.0),
        vec2(g.size().x + 12.0, 18.0),
    );
    p.rect_filled(r, 4.0, c.gamma_multiply(if t.dark { 0.3 } else { 0.14 }));
    p.galley(r.center() - g.size() / 2.0, g, c);
}

impl App {
    /// The Wikis I can see, most popular first: views in the last 30 days,
    /// then the latest edit. (id, title, views)
    pub(super) fn popular_wikis(&self, now: i64) -> Vec<(u64, String, usize)> {
        let since = now - DOC_VIEW_DAYS * 86_400;
        let mut v: Vec<(u64, String, usize, i64)> = self
            .data
            .docs
            .iter()
            .filter(|d| d.is_wiki() && self.doc_shown(d))
            .map(|d| (d.id, d.title.clone(), d.views_since(since), d.updated))
            .collect();
        v.sort_by(|a, b| b.2.cmp(&a.2).then(b.3.cmp(&a.3)).then(a.0.cmp(&b.0)));
        v.into_iter().map(|(id, t, n, _)| (id, t, n)).collect()
    }

    /// Counts an open of a doc by me (for Popular Wikis), once each time it
    /// is opened, not every frame. Opens older than 30 days are dropped.
    pub(super) fn count_doc_view(&mut self, id: u64, now: i64) {
        let me = self.data.me;
        let fresh = if self
            .data
            .docs
            .iter()
            .any(|d| d.id == id && d.views.iter().all(|v| v.member != me))
        {
            Some(self.data.new_id())
        } else {
            None
        };
        let Some(d) = self.data.docs.iter_mut().find(|d| d.id == id) else {
            return;
        };
        if let Some(nid) = fresh {
            d.views.push(DocViews {
                id: nid,
                member: me,
                at: vec![],
            });
        }
        if let Some(v) = d.views.iter_mut().find(|v| v.member == me) {
            let since = now - DOC_VIEW_DAYS * 86_400;
            v.at.retain(|t| *t >= since);
            v.at.push(now);
        }
        self.dirty = true;
    }

    /// The sidebar used by every app hub. Returns (clicked item, plus clicked).
    fn hub_sidebar(
        &mut self,
        ui: &mut Ui,
        id: &str,
        title: &str,
        items: &[(&str, &str, Option<usize>)],
        selected: u8,
        create_button: bool,
    ) -> (Option<u8>, bool) {
        let t = self.t();
        let mut clicked = None;
        let mut plus = false;
        egui::SidePanel::left(egui::Id::new(("hub", id)))
            .exact_width(255.0)
            .resizable(false)
            .frame(hub_frame(&t))
            .show_inside(ui, |ui| {
                let w = ui.available_width();
                let (hr, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
                ui.painter().text(
                    hr.left_center() + vec2(6.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr(title),
                    FontId::proportional(17.0),
                    t.text,
                );
                if create_button {
                    let br = Rect::from_min_size(
                        pos2(hr.right() - 80.0, hr.center().y - 14.0),
                        vec2(80.0, 28.0),
                    );
                    let b = ui.interact(br, ui.id().with("hub_create"), Sense::click());
                    name_it(&b, &format!("Create in {}", title));
                    ui.painter().rect_filled(
                        br,
                        7.0,
                        if b.hovered() {
                            t.primary_hover
                        } else {
                            t.primary
                        },
                    );
                    ui.painter().text(
                        br.center(),
                        Align2::CENTER_CENTER,
                        tr("+  Create"),
                        FontId::proportional(14.0),
                        t.on_primary,
                    );
                    plus |= b.clicked();
                } else {
                    let br = Rect::from_center_size(
                        pos2(hr.right() - 14.0, hr.center().y),
                        vec2(28.0, 28.0),
                    );
                    let b = ui.interact(br, ui.id().with("hub_plus"), Sense::click());
                    name_it(&b, &format!("New in {}", title));
                    ui.painter()
                        .rect_filled(br, 7.0, if b.hovered() { t.hover } else { t.bg });
                    ui.painter().rect_stroke(
                        br,
                        7.0,
                        Stroke::new(1.0_f32, t.border),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        br.center(),
                        Align2::CENTER_CENTER,
                        "+",
                        FontId::proportional(18.0),
                        t.text,
                    );
                    plus |= b.clicked();
                }
                let cr = Rect::from_center_size(
                    pos2(
                        hr.right() - (if create_button { 100.0 } else { 44.0 }),
                        hr.center().y,
                    ),
                    vec2(24.0, 24.0),
                );
                let c = ui.interact(cr, ui.id().with("hub_collapse"), Sense::click());
                if c.hovered() {
                    ui.painter().rect_filled(cr, 5.0, t.row_selected);
                }
                ui.painter().text(
                    cr.center(),
                    Align2::CENTER_CENTER,
                    "\u{00AB}",
                    FontId::proportional(16.0),
                    t.text2,
                );
                if c.on_hover_text(tr("Back to Home")).clicked() {
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
                            paint_avatar(
                                ui.painter(),
                                r.left_center() + vec2(16.0, 0.0),
                                9.0,
                                m,
                                t.sidebar,
                            );
                        }
                    } else {
                        line_icon(
                            ui.painter(),
                            icon,
                            r.left_center() + vec2(16.0, 0.0),
                            t.text2,
                        );
                    }
                    ui.painter().text(
                        r.left_center() + vec2(34.0, 0.0),
                        Align2::LEFT_CENTER,
                        tr(label),
                        FontId::proportional(14.0),
                        if sel { t.text } else { t.text2 },
                    );
                    if let Some(n) = count {
                        if *n > 0 {
                            ui.painter().text(
                                r.right_center() - vec2(12.0, 0.0),
                                Align2::RIGHT_CENTER,
                                n.to_string(),
                                FontId::proportional(12.0),
                                t.text3,
                            );
                        }
                    }
                    if resp.clicked() {
                        clicked = Some(i as u8);
                    }
                }
                ui.add_space(6.0);
                let r = ui.available_rect_before_wrap();
                ui.painter().hline(
                    r.left() + 6.0..=r.right() - 6.0,
                    r.top(),
                    Stroke::new(1.0_f32, t.border),
                );
                ui.add_space(10.0);
                ui.label(RichText::new(tr("  Favorites")).size(13.0).color(t.text2));
                let prefix = match id {
                    "docs" => "doc:",
                    "wb" => "wb:",
                    "forms" => "form:",
                    "dash" => "dash:",
                    _ => "",
                };
                let stars: Vec<String> = self
                    .data
                    .extra
                    .starred
                    .iter()
                    .filter(|s| !prefix.is_empty() && s.starts_with(prefix))
                    .cloned()
                    .collect();
                if stars.is_empty() {
                    let text = match id {
                        "docs" => tr("Star a Doc to see it here"),
                        "wb" => tr("Star a Whiteboard to see it here"),
                        "forms" => tr("Star a Form to see it here"),
                        _ => tr("Star a Dashboard to see it here"),
                    };
                    if id != "teams" {
                        empty_card(ui, &t, "\u{2605}", text);
                    }
                } else {
                    for st in stars {
                        let oid: u64 = st[prefix.len()..].parse().unwrap_or(0);
                        let name = match id {
                            "docs" => self
                                .data
                                .docs
                                .iter()
                                .find(|d| d.id == oid)
                                .map(|d| d.title.clone()),
                            "wb" => self
                                .data
                                .extra
                                .whiteboards
                                .iter()
                                .find(|d| d.id == oid)
                                .map(|d| d.name.clone()),
                            "forms" => self
                                .data
                                .extra
                                .forms
                                .iter()
                                .find(|d| d.id == oid)
                                .map(|d| d.name.clone()),
                            _ => self
                                .data
                                .extra
                                .dashboards
                                .iter()
                                .find(|d| d.id == oid)
                                .map(|d| d.name.clone()),
                        };
                        if let Some(n) = name {
                            if ui
                                .add(ghost(egui::Button::new(format!("\u{2605}  {}", n))))
                                .clicked()
                            {
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
                    ui.label(
                        RichText::new(tr("  Popular Wikis"))
                            .size(13.0)
                            .color(t.text2),
                    );
                    let wikis = self.popular_wikis(now_ts());
                    if wikis.is_empty() {
                        empty_card(
                            ui,
                            &t,
                            "\u{2714}",
                            tr("Most viewed and active Wikis appear here"),
                        );
                    }
                    for (wid, title, views) in wikis.into_iter().take(5) {
                        let w = ui.available_width();
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                        name_it(&resp, &format!("Popular Wiki {}", title));
                        if self.doc_open == Some(wid) {
                            ui.painter().rect_filled(r, 7.0, t.row_selected);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 7.0, t.hover);
                        }
                        line_icon(
                            ui.painter(),
                            "notebook",
                            r.left_center() + vec2(16.0, 0.0),
                            t.text2,
                        );
                        let p = ui.painter().with_clip_rect(r.shrink2(vec2(50.0, 0.0)));
                        p.text(
                            r.left_center() + vec2(34.0, 0.0),
                            Align2::LEFT_CENTER,
                            &title,
                            FontId::proportional(13.5),
                            t.text,
                        );
                        ui.painter().text(
                            r.right_center() - vec2(10.0, 0.0),
                            Align2::RIGHT_CENTER,
                            views.to_string(),
                            FontId::proportional(12.0),
                            t.text3,
                        );
                        let resp = resp.on_hover_text(trf!("{} views in the last 30 days", views));
                        if resp.clicked() {
                            self.doc_open = Some(wid);
                        }
                    }
                }
                if id == "teams" {
                    ui.add_space(4.0);
                    let teams = self.data.extra.teams.clone();
                    if teams.is_empty() {
                        empty_card(
                            ui,
                            &t,
                            "\u{1F465}",
                            tr("Once you join or create a team, you will see it here"),
                        );
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
                        ui.painter().rect_filled(
                            Rect::from_center_size(
                                r.left_center() + vec2(16.0, 0.0),
                                vec2(16.0, 16.0),
                            ),
                            4.0,
                            rgb(tm.color),
                        );
                        ui.painter().text(
                            r.left_center() + vec2(34.0, 0.0),
                            Align2::LEFT_CENTER,
                            &tm.name,
                            FontId::proportional(14.0),
                            t.text,
                        );
                        if resp.clicked() {
                            self.team_open = Some(tm.id);
                            self.hub_filter = 9;
                        }
                    }
                }
            });
        (clicked, plus)
    }

    pub(super) fn page_bar(
        &mut self,
        ui: &mut Ui,
        title: &str,
        right: impl FnOnce(&mut Self, &mut Ui),
    ) {
        let t = self.t();
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 12,
                top: 8,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(28.0);
                    ui.label(RichText::new(tr(title)).size(15.5).color(t.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| right(self, ui));
                });
            });
        let r = ui.available_rect_before_wrap();
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
    }

    // ---------------------------------------------------------------- docs

    /// The "My Docs" badge: top-level docs only, as the list shows them
    /// (sub pages are counted with their doc).
    pub(super) fn my_docs_count(&self) -> usize {
        self.data
            .docs
            .iter()
            .filter(|d| d.owner == self.data.me && !d.archived && d.parent.is_none())
            .count()
    }

    pub(super) fn docs_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let items: [(&str, &str, Option<usize>); 7] = [
            ("docs", "All Docs", None),
            ("@me", "My Docs", Some(self.my_docs_count())),
            ("teams", "Shared with me", None),
            ("lock", "Private", None),
            ("edit", "Meeting Notes", None),
            ("notebook", "Wikis", None),
            ("archive", "Archived", None),
        ];
        let sel = if self.doc_open.is_some() {
            99
        } else {
            self.hub_filter.min(6)
        };
        if self.doc_open != self.doc_viewed {
            self.doc_viewed = self.doc_open;
            if let Some(id) = self.doc_open {
                self.count_doc_view(id, now_ts());
            }
        }
        let (clicked, plus) = self.hub_sidebar(ui, "docs", "Docs", &items, sel, true);
        if let Some(i) = clicked {
            self.hub_filter = i;
            self.doc_open = None;
        }
        if plus {
            self.new_doc(self.current_space());
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.bg))
            .show_inside(ui, |ui| {
                if self.doc_open.is_some() {
                    self.doc_editor(ui);
                    return;
                }
                let title = items[self.hub_filter.min(6) as usize].1;
                self.page_bar(ui, title, |s, ui| {
                    let tt = s.t();
                    if primary_button(ui, &tt, tr("New Doc")).clicked() {
                        s.new_doc(s.current_space());
                    }
                    // no Import here: the only importer reads tasks from a CSV
                    // into a List, which is not what a Docs page promises
                });
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Frame::new()
                            .inner_margin(egui::Margin::symmetric(16, 12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(tr("Templates")).color(t.text2));
                                    if ui
                                        .add(ghost(egui::Button::new(
                                            RichText::new(tr("Browse all templates"))
                                                .color(t.text2),
                                        )))
                                        .clicked()
                                    {
                                        self.open_templates(super::templates::Category::Docs, None);
                                    }
                                });
                                ui.add_space(4.0);
                                let w = ui.available_width();
                                let gap = ui.spacing().item_spacing.x;
                                let cw = ((w - 2.0 * gap) / 3.0).floor().min(430.0);
                                ui.horizontal(|ui| {
                                    for b in super::templates::builtins()
                                        .into_iter()
                                        .filter(|b| b.tpl.kind == super::templates::KIND_DOC)
                                    {
                                        let (color, glyph, name) = (b.color, b.glyph, b.en);
                                        let (r, resp) =
                                            ui.allocate_exact_size(vec2(cw, 70.0), Sense::click());
                                        name_it(&resp, &format!("Template {}", name));
                                        ui.painter().rect_filled(
                                            r,
                                            10.0,
                                            if resp.hovered() { t.sidebar } else { t.card },
                                        );
                                        ui.painter().rect_stroke(
                                            r,
                                            10.0,
                                            Stroke::new(1.0_f32, t.border),
                                            egui::StrokeKind::Inside,
                                        );
                                        let c = r.left_center() + vec2(32.0, 0.0);
                                        ui.painter().circle_filled(
                                            c,
                                            18.0,
                                            rgb(color).gamma_multiply(0.14),
                                        );
                                        line_icon_scaled(ui.painter(), glyph, c, rgb(color), 1.15);
                                        // shortened with an ellipsis in a narrow card
                                        paint_fit(
                                            ui,
                                            r.left_center() + vec2(62.0, -10.0),
                                            Align2::LEFT_CENTER,
                                            &b.tpl.name,
                                            FontId::proportional(15.0),
                                            t.text,
                                            cw - 62.0 - 10.0,
                                        );
                                        paint_fit(
                                            ui,
                                            r.left_center() + vec2(62.0, 12.0),
                                            Align2::LEFT_CENTER,
                                            b.about,
                                            FontId::proportional(12.5),
                                            t.text2,
                                            cw - 62.0 - 10.0,
                                        );
                                        if resp.clicked() {
                                            self.use_builtin_doc(b.key, self.current_space());
                                        }
                                    }
                                });
                                ui.add_space(16.0);
                                ui.horizontal(|ui| {
                                    let r = icon_button(
                                        ui,
                                        &t,
                                        "filter",
                                        if self.docs.mine {
                                            tr("Filters \u{2022} 1")
                                        } else {
                                            tr("Filters")
                                        },
                                        "Docs filters",
                                        true,
                                    );
                                    popup(ui, &r, ui.id().with("docs_filters"), 200.0, |ui| {
                                        check(ui, &mut self.docs.mine, tr("Only Docs I created"));
                                    });
                                    let sort_text = trf!(
                                        "Sort: {}",
                                        tr(["Date updated", "Name", "Date created"]
                                            [self.docs.sort as usize % 3])
                                    );
                                    let sr =
                                        icon_button(ui, &t, "sort", &sort_text, "Docs sort", true);
                                    popup(ui, &sr, ui.id().with("docs_sort"), 180.0, |ui| {
                                        for (i, l) in ["Date updated", "Name", "Date created"]
                                            .iter()
                                            .enumerate()
                                        {
                                            if ui
                                                .selectable_label(self.docs.sort == i as u8, tr(l))
                                                .clicked()
                                            {
                                                self.docs.sort = i as u8;
                                                close_popup(ui);
                                            }
                                        }
                                    });
                                    let tag_label = match &self.docs.tag {
                                        Some(g) => trf!("Tags: {}", g),
                                        None => tr("Tags").to_string(),
                                    };
                                    let tagr = icon_button(
                                        ui,
                                        &t,
                                        "tag",
                                        &tag_label,
                                        "Docs tag filter",
                                        true,
                                    );
                                    let all_tags = self.doc_tags();
                                    popup(ui, &tagr, ui.id().with("docs_tags"), 180.0, |ui| {
                                        if all_tags.is_empty() {
                                            ui.label(
                                                RichText::new(tr(
                                                    "No tags yet. Add tags under a Doc's title.",
                                                ))
                                                .color(t.text3),
                                            );
                                        }
                                        if ui
                                            .selectable_label(
                                                self.docs.tag.is_none(),
                                                tr("Any tag"),
                                            )
                                            .clicked()
                                        {
                                            self.docs.tag = None;
                                            close_popup(ui);
                                        }
                                        for g in all_tags {
                                            if ui
                                                .selectable_label(
                                                    self.docs.tag.as_ref() == Some(&g),
                                                    RichText::new(&g).color(tag_color(&g)),
                                                )
                                                .clicked()
                                            {
                                                self.docs.tag = Some(g);
                                                close_popup(ui);
                                            }
                                        }
                                    });
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let r = ui.add(
                                            field(&mut self.docs.search)
                                                .hint_text(tr("Search"))
                                                .desired_width(160.0),
                                        );
                                        name_field(&r, "Search Docs");
                                    });
                                });
                                ui.add_space(8.0);
                                let w = ui.available_width();
                                let cols = [0.0, 0.42, 0.62, 0.76, 0.88];
                                let (hr, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
                                for (x, h) in cols.iter().zip([
                                    "Name",
                                    "Location",
                                    "Tags",
                                    "Date updated",
                                    "Sharing",
                                ]) {
                                    ui.painter().text(
                                        pos2(hr.left() + w * x + 30.0, hr.center().y),
                                        Align2::LEFT_CENTER,
                                        tr(h),
                                        FontId::proportional(13.0),
                                        t.text2,
                                    );
                                }
                                ui.painter().hline(
                                    hr.x_range(),
                                    hr.bottom(),
                                    Stroke::new(1.0_f32, t.border),
                                );
                                let q = self.docs.search.to_lowercase();
                                let tag = self.docs.tag.clone();
                                let filter = self.hub_filter;
                                let mine_only = self.docs.mine;
                                let sort = self.docs.sort;
                                let mut docs: Vec<Doc> = self
                                    .data
                                    .docs
                                    .iter()
                                    .filter(|d| match filter.min(6) {
                                        1 => d.owner == me && !d.archived,
                                        2 => d.owner != me && !d.private && !d.archived,
                                        3 => d.private && !d.archived,
                                        4 => d.meeting_note && !d.archived,
                                        5 => d.is_wiki() && !d.archived,
                                        6 => d.archived,
                                        _ => !d.archived,
                                    })
                                    // someone's Private docs are for them alone, and
                                    // docs in archived spaces or other people's
                                    // Personal spaces stay out, as in the sidebar
                                    .filter(|d| self.doc_reachable(d))
                                    .filter(|d| d.parent.is_none())
                                    .filter(|d| q.is_empty() || d.title.to_lowercase().contains(&q))
                                    .filter(|d| !mine_only || d.owner == me)
                                    .filter(|d| tag.as_ref().is_none_or(|g| d.tags.contains(g)))
                                    .cloned()
                                    .collect();
                                match sort {
                                    1 => docs.sort_by_key(|d| d.title.to_lowercase()),
                                    2 => docs.sort_by_key(|d| std::cmp::Reverse(d.created)),
                                    _ => docs.sort_by_key(|d| std::cmp::Reverse(d.updated)),
                                }
                                if docs.is_empty() {
                                    ui.add_space(20.0);
                                    ui.vertical_centered(|ui| {
                                        ui.label(
                                            RichText::new(tr("No Docs here yet")).color(t.text3),
                                        )
                                    });
                                }
                                for d in docs {
                                    let (r, resp) =
                                        ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
                                    if resp.hovered() {
                                        ui.painter().rect_filled(r, 0.0, t.hover);
                                    }
                                    ui.painter().hline(
                                        r.x_range(),
                                        r.bottom(),
                                        Stroke::new(0.5_f32, t.border),
                                    );
                                    let ic = Rect::from_center_size(
                                        r.left_center() + vec2(44.0, 0.0),
                                        vec2(12.0, 15.0),
                                    );
                                    ui.painter().rect_filled(
                                        ic,
                                        2.0,
                                        Color32::from_rgb(16, 144, 224),
                                    );
                                    let tr_ = ui.painter().text(
                                        r.left_center() + vec2(60.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        &d.title,
                                        FontId::proportional(14.5),
                                        t.text,
                                    );
                                    if d.is_wiki() {
                                        wiki_badge(ui.painter(), &t, tr_.right_center());
                                    }
                                    let loc = d
                                        .space
                                        .and_then(|sid| {
                                            self.data.spaces.iter().find(|s| s.id == sid)
                                        })
                                        .map(|s| (s.name.clone(), s.color));
                                    if let Some((n, c)) = loc {
                                        let lr = Rect::from_center_size(
                                            pos2(r.left() + w * cols[1] + 38.0, r.center().y),
                                            vec2(16.0, 16.0),
                                        );
                                        ui.painter().rect_filled(lr, 4.0, rgb(c));
                                        ui.painter().text(
                                            pos2(r.left() + w * cols[1] + 52.0, r.center().y),
                                            Align2::LEFT_CENTER,
                                            n,
                                            FontId::proportional(14.0),
                                            t.text2,
                                        );
                                    } else {
                                        ui.painter().text(
                                            pos2(r.left() + w * cols[1] + 30.0, r.center().y),
                                            Align2::LEFT_CENTER,
                                            tr("Workspace"),
                                            FontId::proportional(14.0),
                                            t.text2,
                                        );
                                    }
                                    if d.tags.is_empty() {
                                        ui.painter().text(
                                            pos2(r.left() + w * cols[2] + 30.0, r.center().y),
                                            Align2::LEFT_CENTER,
                                            "\u{2013}",
                                            FontId::proportional(14.0),
                                            t.text3,
                                        );
                                    } else {
                                        // tag chips, clipped to the column
                                        let col = Rect::from_min_max(
                                            pos2(r.left() + w * cols[2] + 30.0, r.top()),
                                            pos2(r.left() + w * cols[3] + 24.0, r.bottom()),
                                        );
                                        let p = ui.painter().with_clip_rect(col);
                                        let mut x = col.left();
                                        for g in &d.tags {
                                            let c = tag_color(g);
                                            let gal = p.layout_no_wrap(
                                                g.clone(),
                                                FontId::proportional(11.0),
                                                c,
                                            );
                                            let cr = Rect::from_min_size(
                                                pos2(x, r.center().y - 9.0),
                                                vec2(gal.size().x + 12.0, 18.0),
                                            );
                                            p.rect_filled(cr, 4.0, c.gamma_multiply(0.16));
                                            p.galley(cr.center() - gal.size() / 2.0, gal, c);
                                            x = cr.right() + 4.0;
                                        }
                                    }
                                    ui.painter().text(
                                        pos2(r.left() + w * cols[3] + 30.0, r.center().y),
                                        Align2::LEFT_CENTER,
                                        fmt_ago(d.updated),
                                        FontId::proportional(14.0),
                                        t.text2,
                                    );
                                    if let Some(m) =
                                        self.data.member(d.owner).or_else(|| self.data.member(me))
                                    {
                                        paint_avatar(
                                            ui.painter(),
                                            pos2(r.left() + w * cols[4] + 42.0, r.center().y),
                                            11.0,
                                            m,
                                            t.bg,
                                        );
                                    }
                                    name_it(&resp, &format!("Doc {}", d.title));
                                    if resp.clicked() {
                                        self.doc_open = Some(d.id);
                                    }
                                    resp.context_menu(|ui| self.doc_menu(ui, d.id, true));
                                }
                            });
                    });
            });
    }

    fn doc_editor(&mut self, ui: &mut Ui) {
        let t = self.t();
        let Some(idx) = self
            .doc_open
            .and_then(|id| self.data.docs.iter().position(|d| d.id == id))
        else {
            self.doc_open = None;
            return;
        };
        let id = self.data.docs[idx].id;
        let key = format!("doc:{}", id);
        let starred = self.data.extra.starred.contains(&key);
        let mut delete = false;
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 12,
                top: 8,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add(ghost(egui::Button::new(
                            RichText::new(tr("\u{2190}  All Docs")).color(t.text2),
                        )))
                        .clicked()
                    {
                        self.doc_open = None;
                    }
                    let loc = self.data.docs[idx]
                        .space
                        .and_then(|sid| self.data.spaces.iter().find(|s| s.id == sid))
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| tr("Workspace").into());
                    // the location moves the doc to another Space
                    let lr = ui
                        .add(ghost(egui::Button::new(
                            RichText::new(format!("/  {}  \u{25BE}", loc)).color(t.text3),
                        )))
                        .on_hover_text(tr("Move to another Space"));
                    name_it(&lr, "Doc location");
                    let cur_space = self.data.docs[idx].space;
                    let mut spaces: Vec<(Option<u64>, String)> =
                        vec![(None, tr("Workspace (no Space)").into())];
                    spaces.extend(
                        self.open_spaces()
                            .iter()
                            .map(|s| (Some(s.id), s.name.clone())),
                    );
                    popup(ui, &lr, ui.id().with(("doc_move", id)), 200.0, |ui| {
                        ui.label(RichText::new(tr("Move to")).small().color(t.text3));
                        for (sid, name) in spaces {
                            if ui.selectable_label(cur_space == sid, name).clicked() {
                                self.move_doc_to_space(id, sid);
                                close_popup(ui);
                            }
                        }
                    });
                    if self.data.docs[idx].is_wiki() {
                        let (r, br) = ui.allocate_exact_size(vec2(44.0, 18.0), Sense::hover());
                        name_it(&br, "Wiki badge");
                        wiki_badge(ui.painter(), &t, r.left_center() - vec2(8.0, 0.0));
                    }
                    if self.data.docs[idx].meeting_note {
                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        line_icon(ui.painter(), "edit", r.center(), t.text3);
                        ui.label(RichText::new(tr("Meeting Note")).small().color(t.text3));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mr = icon_button(ui, &t, "dots", "", "Doc options", false)
                            .on_hover_text(tr("More"));
                        click_menu(&mr, |ui| self.doc_menu(ui, id, false));
                        if ui
                            .add(ghost(egui::Button::new(
                                RichText::new(tr("Delete")).color(t.danger),
                            )))
                            .clicked()
                        {
                            delete = true;
                        }
                        let arch = if self.data.docs[idx].archived {
                            "Unarchive"
                        } else {
                            "Archive"
                        };
                        if ui
                            .add(ghost(egui::Button::new(tr(arch))))
                            .on_hover_text(tr("Move to Archived and save a Markdown copy"))
                            .clicked()
                        {
                            self.toggle_archive(id);
                        }
                        if ui
                            .add(ghost(egui::Button::new(tr("Export"))))
                            .on_hover_text(tr("Save as a Markdown file"))
                            .clicked()
                        {
                            let msg = match self.export_doc(id) {
                                Some(p) => trf!("Saved {}", p.display()),
                                None => tr("Couldn't save the file").into(),
                            };
                            self.toast(msg);
                        }
                        if ui
                            .add(ghost(egui::Button::new(tr("Duplicate"))))
                            .on_hover_text(tr("Copy this doc and its pages"))
                            .clicked()
                        {
                            if let Some(n) = self.duplicate_doc(id) {
                                self.doc_open = Some(n);
                                self.toast(tr("Doc duplicated"));
                            }
                        }
                        if ui.add(ghost(egui::Button::new(tr("Copy link")))).clicked() {
                            self.copy_link(ui, nav::doc_link(id));
                        }
                        let Some(idx) = self.data.docs.iter().position(|d| d.id == id) else {
                            return;
                        };
                        let d = &mut self.data.docs[idx];
                        let (kind, label) = if d.private {
                            ("lock", "Private")
                        } else {
                            ("teams", "Shared")
                        };
                        if icon_button_ex(
                            ui,
                            &t,
                            kind,
                            tr(label),
                            label,
                            false,
                            0.0,
                            t.text,
                            t.text2,
                        )
                        .on_hover_text(tr("Who can see this Doc"))
                        .clicked()
                        {
                            d.private = !d.private;
                            self.dirty = true;
                        }
                        let star = RichText::new(if starred { "\u{2605}" } else { "\u{2606}" })
                            .size(16.0)
                            .color(if starred {
                                Color32::from_rgb(0xf8, 0xae, 0x00)
                            } else {
                                t.text2
                            });
                        if ui.add(ghost(egui::Button::new(star))).clicked() {
                            if starred {
                                self.data.extra.starred.retain(|s| s != &key);
                            } else {
                                self.data.extra.starred.push(key.clone());
                            }
                            self.dirty = true;
                        }
                        if let Some(d) = self.data.docs.iter().find(|d| d.id == id) {
                            ui.label(
                                RichText::new(trf!("Last updated {}", fmt_ago(d.updated)))
                                    .small()
                                    .color(t.text3),
                            );
                        }
                    });
                });
            });
        let r = ui.available_rect_before_wrap();
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
        if delete {
            // the page and its sub pages go to Trash together
            self.trash_doc(id);
            return;
        }
        // the header may have opened a copy or deleted this doc
        let Some(idx) = self
            .doc_open
            .and_then(|o| self.data.docs.iter().position(|d| d.id == o))
        else {
            return;
        };
        self.doc_body(ui, idx);
    }

    // ---------------------------------------------------------------- dashboards

    pub(super) fn dashboards_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let me = self.data.me;
        let items: [(&str, &str, Option<usize>); 2] = [
            ("dashboards", "All Dashboards", None),
            ("@me", "My Dashboards", None),
        ];
        let sel = if self.dash_open.is_some() {
            99
        } else {
            self.hub_filter.min(1)
        };
        let (clicked, plus) = self.hub_sidebar(ui, "dash", "Dashboards", &items, sel, true);
        if let Some(i) = clicked {
            self.hub_filter = i;
            self.dash_open = None;
        }
        if plus {
            self.modal = Some(Modal::NewDashboard {
                name: tr("New Dashboard").into(),
            });
        }
        let mine_only = self.hub_filter.min(1) == 1;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| {
            if self.dash_open.is_some() {
                self.dashboard_content(ui);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if !self.data.extra.dashboards.is_empty() {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                        ui.label(RichText::new(tr("Your Dashboards")).size(15.0));
                        // My: the ones I made (older ones without a maker show in both)
                        // someone else's private dashboard stays out of the list
                        let shown: Vec<Dashboard> = self.data.extra.dashboards.iter().filter(|d| self.dash_shown(d)).cloned().collect();
                        for d in shown.into_iter().filter(|d| !mine_only || d.owner == me || d.owner == 0) {
                            ui.horizontal(|ui| {
                                self.star_toggle(ui, format!("dash:{}", d.id));
                                if icon_button_ex(ui, &t, "dashboards", &d.name, &d.name, false, 0.0, t.text, t.text2).clicked() {
                                    self.dash_open = Some(d.id);
                                }
                            });
                        }
                        if icon_button_ex(ui, &t, "dashboards", tr("Workspace overview"), "Workspace overview", false, 0.0, t.text, t.text2).clicked() {
                            self.dash_open = Some(WORKSPACE_OVERVIEW);
                        }
                    });
                }
                let choice = self.gallery(ui, tr("Choose a Dashboard template"), tr("Get started with a Dashboard template or create a custom Dashboard\nto fit your exact needs."), &[
                    ([0x44, 0x66, 0xff], "dashboards", "Simple Dashboard", "Manage and prioritize tasks", false),
                    ([0xb6, 0x60, 0xe0], "sparkle", "AI Team Center", "View team activity with AI", true),
                    ([0x0f, 0x9d, 0x9f], "clock", "Time Tracking", "View and report on time tracking metrics", false),
                    ([0x10, 0x90, 0xe0], "list", "Project Management", "Analyze project progress and metrics", false),
                    ([0xb6, 0x60, 0xe0], "sparkle", "AI Personal Center", "Improve your performance and career with AI cards", true),
                ]);
                if let Some(name) = choice {
                    if name.starts_with("AI ") {
                        self.modal = Some(super::menus::unavailable(tr(&name)));
                    } else {
                        let id = self.data.new_id();
                        // each template shows its own set of cards; scratch shows them all
                        let kind = match name.as_str() {
                            "Simple Dashboard" => "simple",
                            "Time Tracking" => "time",
                            "Project Management" => "project",
                            _ => "",
                        };
                        let n = if name == "Start from scratch" { tr("New Dashboard").to_string() } else { tr(&name).to_string() };
                        self.data.extra.dashboards.push(Dashboard { id, name: n, space: None, owner: me, kind: kind.into(), ..Default::default() });
                        self.dash_open = Some(id);
                        self.dirty = true;
                    }
                }
            });
        });
    }

    /// A star that adds the item ("dash:ID", "wb:ID", "form:ID") to the
    /// sidebar's Favorites, or takes it off.
    pub(super) fn star_toggle(&mut self, ui: &mut Ui, key: String) {
        let t = self.t();
        let starred = self.data.extra.starred.contains(&key);
        let star = RichText::new(if starred { "\u{2605}" } else { "\u{2606}" })
            .size(16.0)
            .color(if starred {
                Color32::from_rgb(0xf8, 0xae, 0x00)
            } else {
                t.text2
            });
        let r = ui
            .add(ghost(egui::Button::new(star)))
            .on_hover_text(if starred {
                tr("Remove from Favorites")
            } else {
                tr("Add to Favorites")
            });
        name_it(&r, &format!("Star {}", key));
        if r.clicked() {
            if starred {
                self.data.extra.starred.retain(|s| s != &key);
            } else {
                self.data.extra.starred.push(key);
            }
            self.dirty = true;
        }
    }

    /// Template gallery with a "Start from scratch" card; returns the chosen title.
    fn gallery(
        &mut self,
        ui: &mut Ui,
        title: &str,
        sub: &str,
        cards: &[([u8; 3], &str, &str, &str, bool)],
    ) -> Option<String> {
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
        let items: [(&str, &str, Option<usize>); 2] = [
            ("whiteboards", "All Whiteboards", None),
            ("@me", "My Whiteboards", None),
        ];
        let sel = if self.wb_open.is_some() {
            99
        } else {
            self.hub_filter.min(1)
        };
        let (clicked, plus) = self.hub_sidebar(ui, "wb", "Whiteboards", &items, sel, true);
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
                        ui.label(RichText::new(tr("Your Whiteboards")).size(15.0));
                        let me = self.data.me;
                        let mine_only = self.hub_filter.min(1) == 1;
                        // My: the ones I made (older ones without a maker show in both)
                        for w in self.data.extra.whiteboards.clone().into_iter().filter(|w| !mine_only || w.owner == me || w.owner == 0) {
                            ui.horizontal(|ui| {
                                self.star_toggle(ui, format!("wb:{}", w.id));
                                if icon_button_ex(ui, &t, "whiteboards", &w.name, &w.name, false, 0.0, t.text, t.text2).clicked() {
                                    self.wb_open = Some(w.id);
                                }
                            });
                        }
                    });
                }
                let choice = self.gallery(ui, tr("Choose a Whiteboard template"), tr("Create Whiteboards to map out project plans and brainstorm feature\nconcepts, on your own or with your team."), &[
                    ([0xb6, 0x60, 0xe0], "teams", "Organizational Chart", "Visualize your team structure", false),
                    ([0x0f, 0x9d, 0x9f], "list", "Action Plan", "Turn goals into actionable steps", false),
                    ([0x10, 0x90, 0xe0], "send", "Customer Journey Map", "Optimize every customer touchpoint", false),
                    ([0xee, 0x5e, 0x99], "shapes", "Flow Chart", "Structure and communicate processes and workflows", false),
                    ([0xe1, 0x6b, 0x16], "search", "Root Cause Analysis", "Analyze issues using the Ishikawa Fishbone diagram", false),
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
                    // each note gets its id now, like any new item
                    let ids: Vec<u64> = notes.iter().map(|_| self.data.new_id()).collect();
                    if let Some(w) = self.wb_open.and_then(|id| self.data.extra.whiteboards.iter_mut().find(|w| w.id == id)) {
                        // made in the language of the person making it, like a new task's name
                        if name != "Start from scratch" {
                            w.name = tr(&name).to_string();
                        }
                        for (i, ((txt, pos), id)) in notes.into_iter().zip(ids).enumerate() {
                            w.notes.push(Sticky { id, pos, text: tr(txt).to_string(), color: STATUS_PALETTE[1 + (i * 2) % 13] });
                        }
                    }
                }
            });
        });
    }

    // ---------------------------------------------------------------- forms

    pub(super) fn forms_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let items: [(&str, &str, Option<usize>); 2] =
            [("forms", "All Forms", None), ("@me", "My Forms", None)];
        let sel = if self.form_open.is_some() {
            99
        } else {
            self.hub_filter.min(1)
        };
        let (clicked, plus) = self.hub_sidebar(ui, "forms", "Forms", &items, sel, true);
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
                        ui.label(RichText::new(tr("Your Forms")).size(15.0));
                        let me = self.data.me;
                        let mine_only = self.hub_filter.min(1) == 1;
                        // My: the ones I made (older ones without a maker show in both)
                        for f in self.data.extra.forms.clone().into_iter().filter(|f| self.form_shown(f)).collect::<Vec<_>>().into_iter().filter(|f| !mine_only || f.owner == me || f.owner == 0) {
                            ui.horizontal(|ui| {
                                self.star_toggle(ui, format!("form:{}", f.id));
                                if icon_button_ex(ui, &t, "forms", &f.name, &f.name, false, 0.0, t.text, t.text2).clicked() {
                                    self.form_open = Some(f.id);
                                }
                            });
                        }
                    });
                }
                let choice = self.gallery(ui, tr("Choose a Form template"), tr("Create forms to kick off projects, collect feedback, and supercharge\nyour productivity."), &[
                    ([0x0f, 0x9d, 0x9f], "smile", "Feedback Form", "Survey and collect feedback", false),
                    ([0xee, 0x5e, 0x99], "forms", "Project Intake", "Streamline new project requests", false),
                    ([0x5f, 0x55, 0xee], "briefcase", "Order Form", "Capture and process client orders", false),
                    ([0xe1, 0x6b, 0x16], "person", "Job Application", "Accept and review applications for open roles", false),
                    ([0x44, 0x66, 0xff], "settings", "IT Requests", "Triage and prioritize IT service requests", false),
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
                            f.name = tr(&name).to_string();
                            f.fields = fields.into_iter().map(|(l, m)| FormField { label: form_question(l).to_string(), map: m.to_string(), ..Default::default() }).collect();
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
        ui.painter().rect_filled(
            r,
            egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: 8,
                se: 8,
            },
            // the theme's panel grey: a fixed light grey hid the dark theme's text
            t.sidebar,
        );
        ui.add_space(110.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(tr("Make your goals a reality.")).size(26.0).color(t.text2));
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Create Goals, break them down into measurable targets, and track progress in real-time.")).size(16.0).color(t.text3));
            ui.add_space(30.0);
            let (pr, _) = ui.allocate_exact_size(vec2(480.0, 300.0), Sense::hover());
            ui.painter().rect_filled(pr, 6.0, t.card);
            ui.painter().rect_stroke(pr, 6.0, Stroke::new(1.0_f32, t.border), egui::StrokeKind::Inside);
            let band = Rect::from_min_size(pr.min + vec2(0.0, 34.0), vec2(pr.width(), 100.0));
            ui.painter().rect_filled(band, 0.0, Color32::from_rgb(0xe3, 0x9b, 0x6f));
            let card = Rect::from_min_size(pr.min + vec2(66.0, 76.0), vec2(pr.width() - 132.0, 120.0));
            ui.painter().rect_filled(card, 3.0, t.card);
            ui.painter().rect_stroke(card, 3.0, Stroke::new(1.0_f32, t.border), egui::StrokeKind::Inside);
            for (i, c) in [[0x3d, 0xb8, 0x8b], [0x44, 0x66, 0xff], [0xe1, 0x6b, 0x16]].iter().enumerate() {
                let y = card.top() + 28.0 + i as f32 * 32.0;
                ui.painter().circle_filled(pos2(card.left() + 18.0, y), 8.0, rgb(*c));
                ui.painter().rect_filled(Rect::from_min_size(pos2(card.left() + 34.0, y - 3.0), vec2(120.0, 6.0)), 3.0, t.hover);
                ui.painter().rect_filled(Rect::from_min_size(pos2(card.right() - 110.0, y - 2.0), vec2(96.0, 4.0)), 2.0, t.hover);
            }
            ui.painter().circle_stroke(pr.min + vec2(80.0, 60.0), 14.0, Stroke::new(3.0_f32, Color32::WHITE));
            ui.painter().text(pr.min + vec2(80.0, 60.0), Align2::CENTER_CENTER, "30%", FontId::proportional(8.0), Color32::WHITE);
            ui.add_space(30.0);
            if ui.add(Tint(egui::Button::new(RichText::new(tr("Set a Goal")).size(16.0).color(t.on_primary)).fill(t.primary).corner_radius(6).min_size(vec2(124.0, 42.0)))).clicked() {
                let id = self.data.new_id();
                self.data.extra.goals.push(Goal { id, name: tr("New goal").into(), color: STATUS_PALETTE[1], due: None, owner: self.data.me, targets: vec![] });
                self.goal_open = Some(id);
                self.dirty = true;
            }
            ui.add_space(8.0);
            if ui.link(RichText::new(tr("Learn more")).color(t.text2)).clicked() {
                self.modal = Some(Modal::Unavailable { title: tr("Goals").into(), body: tr("Create a Goal, add targets (a number, done / not done, or a set of tasks) and the progress bar fills in as you go.").into() });
            }
        });
    }

    // ---------------------------------------------------------------- teams

    pub(super) fn teams_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        let people = self.data.members.len();
        let mine = self
            .data
            .extra
            .teams
            .iter()
            .filter(|x| x.members.contains(&self.data.me))
            .count();
        let items: [(&str, &str, Option<usize>); 5] = [
            ("teams", "All Teams", None),
            ("person", "All People", Some(people)),
            ("subtasks", "Org Chart", None),
            ("chart", "Analytics", None),
            ("@me", "My Teams", Some(mine)),
        ];
        let sel = if self.hub_filter == 9 {
            99
        } else {
            self.hub_filter.min(4)
        };
        // teams are made and changed by the owner and admins on a server
        let may = self.may_manage();
        let (clicked, plus) = self.hub_sidebar(ui, "teams", "Teams", &items, sel, may);
        if let Some(i) = clicked {
            self.hub_filter = i;
            self.team_open = None;
        }
        if plus && may {
            self.new_team();
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.bg))
            .show_inside(ui, |ui| match self.hub_filter {
                1 => self.people_page(ui),
                2 => self.org_chart(ui),
                3 => self.teams_workload(ui),
                4 => self.all_teams(ui, true),
                9 => self.team_detail(ui),
                _ => self.all_teams(ui, false),
            });
    }

    pub(super) fn new_team(&mut self) {
        let id = self.data.new_id();
        let n = self.data.extra.teams.len();
        self.data.extra.teams.push(Team {
            id,
            name: crate::trf!("Team {}", n + 1),
            members: vec![self.data.me],
            color: STATUS_PALETTE[1 + (n * 3) % 13],
        });
        self.team_open = Some(id);
        self.hub_filter = 9;
        self.dirty = true;
    }

    fn all_teams(&mut self, ui: &mut Ui, mine: bool) {
        let t = self.t();
        self.page_bar(ui, if mine { "My Teams" } else { "All Teams" }, |s, ui| {
            let tt = s.t();
            if s.may_manage() && primary_button(ui, &tt, tr("Create team")).clicked() {
                s.new_team();
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
                let me = self.data.me;
                let teams: Vec<Team> = self.data.extra.teams.iter().filter(|x| !mine || x.members.contains(&me)).cloned().collect();
                if teams.is_empty() && mine && !self.data.extra.teams.is_empty() {
                    ui.label(RichText::new(tr("You're not in any team yet")).color(t.text3));
                } else if teams.is_empty() && self.data.solo() && self.may_manage() {
                    // alone: a team of one means nothing yet, so invite first;
                    // Create team stays in the bar above
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(tr("Invite people to make teams")).size(24.0));
                        ui.label(RichText::new(tr("You're the only one in this workspace. Invite the people you work with, then group them into teams.")).color(t.text2));
                        ui.add_space(12.0);
                        if primary_button(ui, &t, tr("Invite people")).clicked() {
                            self.modal = Some(Modal::Invite { emails: String::new(), role: "Member".into() });
                        }
                    });
                } else if teams.is_empty() {
                    ui.add_space(60.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(tr("Manage teams and see their work")).size(24.0));
                        ui.label(RichText::new(tr("Coordinate teams, organize members and see the details of their work.")).color(t.text2));
                        ui.add_space(12.0);
                        if self.may_manage() && primary_button(ui, &t, tr("Create team")).clicked() {
                            self.new_team();
                        }
                    });
                }
                for tm in teams {
                    let resp = egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0_f32, t.border)).corner_radius(10).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                            ui.painter().rect_filled(r, 5.0, rgb(tm.color));
                            ui.label(RichText::new(&tm.name).size(16.0));
                            ui.label(RichText::new(if tm.members.len() == 1 { tr("1 member").to_string() } else { trf!("{} members", tm.members.len()) }).color(t.text2));
                            let (r, _) = ui.allocate_exact_size(vec2(90.0, 22.0), Sense::hover());
                            paint_avatars(ui.painter(), r.left_center(), 10.0, &tm.members, &self.data, t.card);
                        });
                    });
                    let card = ui.interact(resp.response.rect, ui.id().with(("team", tm.id)), Sense::click());
                    hover_bg(ui, &card, card.rect, 8.0);
                    if card.clicked() {
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
        let Some(ti) = self
            .team_open
            .and_then(|id| self.data.extra.teams.iter().position(|x| x.id == id))
        else {
            self.hub_filter = 0;
            return;
        };
        let members = self.data.members.clone();
        // a plain member on a server sees the team but doesn't change it
        let may = self.may_manage();
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(24, 16))
            .show(ui, |ui| {
                if !may {
                    ui.disable();
                }
                let tm = &mut self.data.extra.teams[ti];
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut tm.name)
                            .font(FontId::proportional(24.0))
                            .frame(false)
                            .desired_width(400.0),
                    )
                    .changed()
                {
                    self.dirty = true;
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Color")).color(t.text2));
                    for (i, c) in STATUS_PALETTE.iter().skip(1).enumerate() {
                        let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                        name_it(&resp, &format!("Team color {}", i + 1));
                        ui.painter().rect_filled(r.shrink(2.0), 4.0, rgb(*c));
                        hover_ring(ui, &resp, r.shrink(1.0), 5.0);
                        if tm.color == *c {
                            ui.painter().rect_stroke(
                                r,
                                5.0,
                                Stroke::new(1.5_f32, t.text),
                                egui::StrokeKind::Inside,
                            );
                        }
                        if resp.clicked() {
                            tm.color = *c;
                            self.dirty = true;
                        }
                    }
                });
                ui.label(RichText::new(tr("Members")).color(t.text2));
                ui.horizontal_wrapped(|ui| {
                    for m in &members {
                        let mut on = tm.members.contains(&m.id);
                        let label = if m.invited {
                            trf!("{} (invited)", m.name)
                        } else {
                            m.name.clone()
                        };
                        if check(ui, &mut on, label).changed() {
                            if on {
                                tm.members.push(m.id);
                            } else {
                                tm.members.retain(|x| *x != m.id);
                            }
                            self.dirty = true;
                        }
                    }
                });
                ui.add_space(4.0);
                // a framed button like Delete on dashboards and whiteboards
                if may
                    && ui
                        .button(RichText::new(tr("Delete team")).color(t.danger))
                        .clicked()
                {
                    self.modal = Some(Modal::ConfirmRemove {
                        kind: "team",
                        id: tm.id,
                        name: tm.name.clone(),
                    });
                }
            });
        ui.separator();
        let ids = self.data.extra.teams[ti].members.clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(24, 8))
                    .show(ui, |ui| {
                        ui.label(RichText::new(tr("What the team is working on")).size(15.0));
                        for id in ids {
                            let Some(m) = self.data.member(id).cloned() else {
                                continue;
                            };
                            let mut open: Vec<Task> = self
                                .data
                                .tasks
                                .iter()
                                .filter(|x| {
                                    x.assignees.contains(&id)
                                        && !self.data.is_closed(x)
                                        && self.data.task_live(x)
                                })
                                .cloned()
                                .collect();
                            // the soonest due first (overdue on top), undated last
                            open.sort_by_key(|x| (x.due.is_none(), x.due_moment()));
                            ui.horizontal(|ui| {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                                paint_avatar(ui.painter(), r.center(), 11.0, &m, t.bg);
                                ui.label(RichText::new(&m.name).strong());
                                ui.label(
                                    RichText::new(if open.len() == 1 {
                                        tr("1 open task").to_string()
                                    } else {
                                        trf!("{} open tasks", open.len())
                                    })
                                    .color(t.text2),
                                );
                            });
                            for task in open.iter().take(6) {
                                if ui.link(format!("      {}", task.name)).clicked() {
                                    self.actions.push(Action::Open(task.id));
                                }
                            }
                            if open.len() > 6 {
                                ui.label(
                                    RichText::new(format!(
                                        "      {}",
                                        trf!("+{} more", open.len() - 6)
                                    ))
                                    .small()
                                    .color(t.text3),
                                );
                            }
                            ui.add_space(6.0);
                        }
                    });
            });
    }

    fn org_chart(&mut self, ui: &mut Ui) {
        let t = self.t();
        // titled like People and Analytics beside it (it had the smaller
        // page bar with a line under it)
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(24, 16))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("Org Chart")).size(20.0));
                        });
                        ui.add_space(8.0);
                        for role in ["Owner", "Admin", "Member", "Guest"] {
                            let people: Vec<Member> = self
                                .data
                                .members
                                .iter()
                                .filter(|m| {
                                    (if m.role.is_empty() {
                                        "Member"
                                    } else {
                                        m.role.as_str()
                                    }) == role
                                })
                                .cloned()
                                .collect();
                            if people.is_empty() {
                                continue;
                            }
                            let heading = match role {
                                "Owner" => tr("Owners"),
                                "Admin" => tr("Admins"),
                                "Guest" => tr("Guests"),
                                _ => tr("Members"),
                            };
                            ui.label(RichText::new(heading).color(t.text2));
                            // cards line up at the top (a wrapped row centres
                            // each on the row so far, which stepped them down)
                            ui.with_layout(
                                Layout::left_to_right(Align::Min).with_main_wrap(true),
                                |ui| {
                                    for m in people {
                                        egui::Frame::new()
                                            .fill(t.card)
                                            .stroke(Stroke::new(1.0_f32, t.border))
                                            .corner_radius(10)
                                            .inner_margin(egui::Margin::same(12))
                                            .show(ui, |ui| {
                                                ui.set_width(170.0);
                                                ui.horizontal(|ui| {
                                                    let (r, _) = ui.allocate_exact_size(
                                                        vec2(30.0, 30.0),
                                                        Sense::hover(),
                                                    );
                                                    paint_avatar(
                                                        ui.painter(),
                                                        r.center(),
                                                        13.0,
                                                        &m,
                                                        t.card,
                                                    );
                                                    ui.vertical(|ui| {
                                                        ui.add(
                                                            egui::Label::new(
                                                                RichText::new(&m.name).strong(),
                                                            )
                                                            .truncate(),
                                                        );
                                                        ui.label(
                                                            RichText::new(if m.invited {
                                                                tr("Invited")
                                                            } else {
                                                                tr(role)
                                                            })
                                                            .small()
                                                            .color(t.text3),
                                                        );
                                                    });
                                                });
                                            });
                                    }
                                },
                            );
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
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 20,
                right: 12,
                top: 6,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(40.0);
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 20.0), Sense::hover());
                    line_icon(ui.painter(), "timesheets", r.center(), t.text2);
                    ui.label(RichText::new(tr("Timesheets")).size(15.5));
                    ui.separator();
                    // no Approvals tab: submitting a week for approval isn't built
                    for (i, label) in ["My timesheet", "All timesheets"].iter().enumerate() {
                        let sel = self.hub_filter.min(1) == i as u8;
                        let g = ui.painter().layout_no_wrap(
                            tr(label).to_string(),
                            FontId::proportional(15.0),
                            if sel { t.text } else { t.text3 },
                        );
                        let (r, resp) =
                            ui.allocate_exact_size(vec2(g.size().x + 16.0, 40.0), Sense::click());
                        name_it(&resp, label);
                        hover_bg(ui, &resp, r.shrink2(vec2(0.0, 6.0)), 6.0);
                        ui.painter().galley(
                            r.center() - g.size() / 2.0,
                            g,
                            if sel { t.text } else { t.text3 },
                        );
                        if sel {
                            ui.painter().rect_filled(
                                Rect::from_min_max(pos2(r.left(), r.bottom() - 2.0), r.max),
                                1.0,
                                t.text,
                            );
                        }
                        if resp.clicked() {
                            self.hub_filter = i as u8;
                        }
                    }
                });
            });
        let r = ui.available_rect_before_wrap();
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
        // the theme's panel grey (a fixed light grey hid the dark theme's text)
        let bg = t.sidebar;
        ui.painter().rect_filled(
            Rect::from_min_max(r.min + vec2(0.0, 1.0), r.max),
            egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: 8,
                se: 8,
            },
            bg,
        );
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 18)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (back, fwd) = super::icons::period_arrows(ui, &t);
                    if back {
                        self.sheet_week = ws - Duration::days(7);
                    }
                    if fwd {
                        self.sheet_week = ws + Duration::days(7);
                    }
                    let r = ui.add(ghost(egui::Button::new(RichText::new(format!("{} - {}  \u{25BE}", ws.text("%b %-d"), (ws + Duration::days(6)).text("%b %-d"))).size(20.0))));
                    name_it(&r, "Pick a week");
                    // a calendar: the week of the picked day opens
                    let mut d = Some(ws);
                    if let Some(true) = popup(ui, &r, ui.id().with("sheet_week_pick"), 380.0, |ui| date_menu(ui, &t, ui.id().with("sheet_week_cal"), &mut d)) {
                        let d = d.unwrap_or_else(today);
                        self.sheet_week = d - Duration::days(d.weekday().num_days_from_sunday() as i64);
                    }
                    if self.hub_filter == 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            // the view shown is outlined, like the composer's tabs
                            for (on, kind, label) in [(true, "list", "Time entries"), (false, "table", "Timesheet")] {
                                let sel = self.sheet_entries == on;
                                if icon_button_ex(ui, &t, kind, tr(label), label, sel, 0.0, if sel { t.text } else { t.text2 }, t.text2).clicked() {
                                    self.sheet_entries = on;
                                }
                            }
                        });
                    }
                });
                ui.add_space(14.0);
                match self.hub_filter.min(1) {
                    1 => self.all_timesheets(ui, ws),
                    _ => {
                        let has_any = self.data.tasks.iter().any(|x| x.time_entries.iter().any(|e| { let i = (local_date(e.start) - ws).num_days(); (0..7).contains(&i) && self.data.is_my_entry(x, e) }) || self.data.is_my_timer(x)) || !self.my_sheet_rows(ws).is_empty() || self.sheet_force;
                        if !has_any {
                            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0_f32, t.border)).corner_radius(12).inner_margin(egui::Margin::symmetric(30, 40)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.vertical_centered(|ui| {
                                    let (r, _) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::hover());
                                    line_icon_scaled(ui.painter(), "timesheets", r.center(), t.text3, 2.6);
                                    ui.label(RichText::new(tr("Add entries to this week's timesheet")).size(17.0));
                                    ui.add_space(14.0);
                                });
                                let w = ui.available_width();
                                let cw = 200.0;
                                ui.horizontal(|ui| {
                                    ui.add_space(((w - 4.0 * cw - 30.0) / 2.0).max(0.0));
                                    for (icon, label) in [("teams", "All assigned tasks"), ("table", "Last week's tasks"), ("check_plus", "Individual tasks"), ("clock", "Track time")] {
                                        let (r, resp) = ui.allocate_exact_size(vec2(cw, 84.0), Sense::click());
                                        name_it(&resp, label);
                                        ui.painter().rect_filled(r, 10.0, if resp.hovered() { t.sidebar } else { t.card });
                                        ui.painter().rect_stroke(r, 10.0, Stroke::new(1.0_f32, t.border), egui::StrokeKind::Inside);
                                        let ib = Rect::from_min_size(r.min + vec2(14.0, 14.0), vec2(28.0, 28.0));
                                        ui.painter().rect_filled(ib, 6.0, t.hover);
                                        line_icon(ui.painter(), icon, ib.center(), t.text2);
                                        ui.painter().text(r.left_top() + vec2(14.0, 62.0), Align2::LEFT_CENTER, tr(label), FontId::proportional(13.5), t.text);
                                        if resp.clicked() {
                                            match label {
                                                "All assigned tasks" => {
                                                    let rows: Vec<u64> = self.data.tasks.iter().filter(|x| x.assignees.contains(&me) && !self.data.is_closed(x) && self.data.task_live(x)).map(|x| x.id).collect();
                                                    let none = rows.is_empty();
                                                    self.add_sheet_rows(ws, rows);
                                                    if none {
                                                        self.toast(tr("No open tasks are assigned to you"));
                                                    }
                                                }
                                                "Last week's tasks" => {
                                                    let lw = ws - Duration::days(7);
                                                    let rows: Vec<u64> = self.data.tasks.iter().filter(|x| x.time_entries.iter().any(|e| { let i = (local_date(e.start) - lw).num_days(); (0..7).contains(&i) && self.data.is_my_entry(x, e) })).map(|x| x.id).collect();
                                                    let none = rows.is_empty();
                                                    self.add_sheet_rows(ws, rows);
                                                    if none {
                                                        self.toast(tr("No time was tracked last week"));
                                                    }
                                                }
                                                "Individual tasks" => {
                                                    // show the grid; pick tasks with "+ Add task"
                                                    self.sheet_force = true;
                                                }
                                                // nothing to pick from: say why instead of an empty list
                                                _ if !self.data.tasks.iter().any(|x| self.data.task_live(x)) => {
                                                    self.toast(tr("Make a task first: time is tracked on tasks"));
                                                }
                                                _ => {
                                                    let first = self.data.tasks.iter().find(|x| x.assignees.contains(&me) && self.data.task_live(x)).map(|x| x.id).unwrap_or(0);
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
        let mut ids: Vec<u64> = self.my_sheet_rows(ws);
        for x in &self.data.tasks {
            let any = x.time_entries.iter().any(|e| {
                let i = (local_date(e.start) - ws).num_days();
                (0..7).contains(&i) && self.data.is_my_entry(x, e)
            }) || self.data.is_my_timer(x);
            if any && !ids.contains(&x.id) {
                ids.push(x.id);
            }
        }
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(10)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                let w = ui.available_width();
                let col = 92.0;
                let name_w = (w - col * 8.0).max(200.0);
                let (hr, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
                ui.painter().text(
                    hr.left_center() + vec2(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr("Task"),
                    FontId::proportional(13.0),
                    t.text2,
                );
                for i in 0..7 {
                    let d = ws + Duration::days(i);
                    let x = hr.left() + name_w + col * i as f32 + col / 2.0;
                    ui.painter().text(
                        pos2(x, hr.center().y),
                        Align2::CENTER_CENTER,
                        d.text("%a %-d"),
                        FontId::proportional(12.5),
                        if d == today() { t.danger } else { t.text2 },
                    );
                }
                ui.painter().text(
                    pos2(hr.left() + name_w + col * 7.5, hr.center().y),
                    Align2::CENTER_CENTER,
                    tr("Total"),
                    FontId::proportional(12.5),
                    t.text2,
                );
                ui.painter()
                    .hline(hr.x_range(), hr.bottom(), Stroke::new(1.0_f32, t.border));
                let mut totals = [0i64; 7];
                let mut drop_row = None;
                for id in &ids {
                    let Some(ti) = self.data.tasks.iter().position(|x| x.id == *id) else {
                        continue;
                    };
                    let name = self.data.tasks[ti].name.clone();
                    let mut per = [0i64; 7];
                    // my time only: teammates' entries on the same task aren't mine to edit
                    for e in self.data.tasks[ti]
                        .time_entries
                        .iter()
                        .filter(|e| self.data.is_my_entry(&self.data.tasks[ti], e))
                    {
                        let i = (local_date(e.start) - ws).num_days();
                        if (0..7).contains(&i) {
                            per[i as usize] += e.end - e.start;
                        }
                    }
                    if let Some(st) = self.data.my_timer_start(&self.data.tasks[ti]) {
                        let i = (local_date(st) - ws).num_days();
                        if (0..7).contains(&i) {
                            per[i as usize] += now - st;
                        }
                    }
                    // time on an archived task still counts: mark the row so
                    let archived = !self.data.task_live(&self.data.tasks[ti]);
                    let shown = if archived {
                        trf!("{} (archived)", name)
                    } else {
                        name.clone()
                    };
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let (nr, nresp) =
                            ui.allocate_exact_size(vec2(name_w, 34.0), Sense::click());
                        ui.painter()
                            .with_clip_rect(Rect::from_min_max(nr.min, nr.max - vec2(26.0, 0.0)))
                            .text(
                                nr.left_center() + vec2(8.0, 0.0),
                                Align2::LEFT_CENTER,
                                &shown,
                                FontId::proportional(14.0),
                                if archived { t.text3 } else { t.text },
                            );
                        // take the row off (and my time on it this week)
                        let xr = Rect::from_center_size(
                            nr.right_center() - vec2(14.0, 0.0),
                            vec2(20.0, 20.0),
                        );
                        let x = ui.interact(xr, ui.id().with(("sheet_x", *id)), Sense::click());
                        name_it(&x, &format!("Remove row {}", name));
                        if nresp.hovered() || x.hovered() {
                            ui.painter().text(
                                xr.center(),
                                Align2::CENTER_CENTER,
                                "\u{00D7}",
                                FontId::proportional(10.0),
                                if x.hovered() { t.danger } else { t.text3 },
                            );
                        }
                        if x.clicked() {
                            drop_row = Some(*id);
                        } else if nresp.clicked() {
                            self.actions.push(Action::Open(*id));
                        }
                        let mut sum = 0;
                        for i in 0..7 {
                            totals[i] += per[i];
                            sum += per[i];
                            let before = per[i] as f64 / 3600.0;
                            let mut hours = before;
                            // never clamp what is already there: a day over 24h
                            // (entries from elsewhere) would be cut just by drawing
                            let r = ui.add_sized(
                                [col - 8.0, 26.0],
                                egui::DragValue::new(&mut hours)
                                    .range(0.0..=24.0)
                                    .clamp_existing_to_range(false)
                                    .speed(0.05)
                                    .custom_formatter(|h, _| fmt_hm((h * 3600.0).round() as i64))
                                    .custom_parser(|s| {
                                        if s.trim() == "-" {
                                            return Some(0.0);
                                        }
                                        super::taskfields::parse_estimate(s)
                                            .map(|m| m as f64 / 60.0)
                                    }),
                            );
                            ui.add_space(8.0);
                            if r.changed() && hours != before {
                                let tid = self.data.tasks[ti].id;
                                self.set_day_time(tid, ws + Duration::days(i as i64), hours);
                            }
                        }
                        let (tr, _) = ui.allocate_exact_size(vec2(col, 34.0), Sense::hover());
                        ui.painter().text(
                            tr.center(),
                            Align2::CENTER_CENTER,
                            fmt_hm(sum),
                            FontId::proportional(13.0),
                            t.text,
                        );
                    });
                }
                if let Some(id) = drop_row {
                    self.drop_sheet_row(ws, id);
                    if let Some(ti) = self.data.tasks.iter().position(|x| x.id == id) {
                        let task = &self.data.tasks[ti];
                        let keep: Vec<bool> = task
                            .time_entries
                            .iter()
                            .map(|e| {
                                !((0..7).contains(&(local_date(e.start) - ws).num_days())
                                    && self.data.is_my_entry(task, e))
                            })
                            .collect();
                        if keep.iter().any(|k| !k) {
                            let task = &mut self.data.tasks[ti];
                            let mut it = keep.iter();
                            task.time_entries.retain(|_| *it.next().unwrap_or(&true));
                            self.dirty = true;
                            self.toast =
                                Some((trf!("Removed this week's time on {}", task.name), now_ts()));
                        }
                    }
                }
                let (fr, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
                ui.painter()
                    .hline(fr.x_range(), fr.top(), Stroke::new(1.0_f32, t.border));
                ui.painter().text(
                    fr.left_center() + vec2(8.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr("Total"),
                    FontId::proportional(13.5),
                    t.text,
                );
                let mut all = 0;
                for (i, v) in totals.iter().enumerate() {
                    all += v;
                    ui.painter().text(
                        pos2(
                            fr.left() + name_w + col * i as f32 + col / 2.0 - 4.0,
                            fr.center().y,
                        ),
                        Align2::CENTER_CENTER,
                        fmt_hm(*v),
                        FontId::proportional(12.5),
                        t.text2,
                    );
                }
                ui.painter().text(
                    pos2(fr.left() + name_w + col * 7.5, fr.center().y),
                    Align2::CENTER_CENTER,
                    fmt_hm(all),
                    FontId::proportional(13.0),
                    t.text,
                );
                ui.horizontal(|ui| {
                    let tasks: Vec<(u64, String)> = self
                        .data
                        .tasks
                        .iter()
                        .filter(|x| !ids.contains(&x.id) && self.data.task_live(x))
                        .map(|x| (x.id, x.name.clone()))
                        .collect();
                    ui.menu_button(tr("+ Add task"), |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(420.0)
                            .show(ui, |ui| {
                                for (id, n) in &tasks {
                                    if ui.button(n).clicked() {
                                        self.add_sheet_rows(ws, vec![*id]);
                                        ui.close_menu();
                                    }
                                }
                            });
                    });
                });
            });
    }

    /// The tasks I added to the timesheet of the week starting `ws` with
    /// "+ Add task" (kept per person). Rows added before rows had a week
    /// show in every week until removed.
    pub(super) fn my_sheet_rows(&self, ws: NaiveDate) -> Vec<u64> {
        let me = self.data.me;
        let wk = sheet_week_key(ws);
        let mut ids: Vec<u64> = self
            .data
            .extra
            .sheet_rows
            .iter()
            .filter(|r| r[0] == me)
            .map(|r| r[1])
            .collect();
        for r in &self.data.extra.sheet_week_rows {
            if r[0] == me && r[2] == wk && !ids.contains(&r[1]) {
                ids.push(r[1]);
            }
        }
        ids.retain(|id| self.task(*id).is_some_and(|x| self.data.task_live(x)));
        ids
    }

    /// Adds rows to my timesheet for the week starting `ws` only.
    pub(super) fn add_sheet_rows(&mut self, ws: NaiveDate, ids: Vec<u64>) {
        let me = self.data.me;
        let wk = sheet_week_key(ws);
        for id in ids {
            if !self.data.extra.sheet_week_rows.contains(&[me, id, wk]) {
                self.data.extra.sheet_week_rows.push([me, id, wk]);
            }
        }
        self.dirty = true;
    }

    /// Takes a row off my timesheet for that week (an old row without a
    /// week goes from every week).
    pub(super) fn drop_sheet_row(&mut self, ws: NaiveDate, id: u64) {
        let me = self.data.me;
        let wk = sheet_week_key(ws);
        self.data.extra.sheet_rows.retain(|r| *r != [me, id]);
        self.data
            .extra
            .sheet_week_rows
            .retain(|r| *r != [me, id, wk]);
        self.dirty = true;
    }

    /// Sets my time on one task and day to `hours` (as the grid shows it, my
    /// running timer included) by adding or taking away the difference from
    /// my entries that day. Teammates' entries are never touched.
    pub(super) fn set_day_time(&mut self, task_id: u64, day: NaiveDate, hours: f64) {
        let now = now_ts();
        let me = self.data.me;
        let Some(ti) = self.data.tasks.iter().position(|x| x.id == task_id) else {
            return;
        };
        let task = &self.data.tasks[ti];
        let running = self
            .data
            .my_timer_start(task)
            .filter(|st| local_date(*st) == day)
            .map(|st| now - st)
            .unwrap_or(0);
        let mine: Vec<usize> = task
            .time_entries
            .iter()
            .enumerate()
            .filter(|(_, e)| local_date(e.start) == day && self.data.is_my_entry(task, e))
            .map(|(k, _)| k)
            .collect();
        let have: i64 = mine
            .iter()
            .map(|k| task.time_entries[*k].end - task.time_entries[*k].start)
            .sum();
        let want = ((hours * 3600.0).round() as i64 - running).max(0);
        let task = &mut self.data.tasks[ti];
        if want > have {
            let start = local_ts(day, nine_am()).unwrap_or(now);
            // one entry typed in the grid per day: grow it instead of adding
            // a new one on every step of a drag
            if let Some(e) = task
                .time_entries
                .iter_mut()
                .rev()
                .find(|e| e.start == start && e.member == me)
            {
                e.end += want - have;
            } else {
                task.time_entries.push(TimeEntry {
                    start,
                    end: start + (want - have),
                    member: me,
                });
            }
        } else if want < have {
            // take it off my latest entries first
            let mut cut = have - want;
            let mut gone = vec![];
            for k in mine.into_iter().rev() {
                let e = &mut task.time_entries[k];
                let len = e.end - e.start;
                if len <= cut {
                    cut -= len;
                    gone.push(k);
                } else {
                    e.end -= cut;
                    cut = 0;
                }
                if cut == 0 {
                    break;
                }
            }
            gone.sort_unstable();
            for k in gone.into_iter().rev() {
                task.time_entries.remove(k);
            }
        }
        self.dirty = true;
    }

    fn time_entries_list(&mut self, ui: &mut Ui, ws: NaiveDate) {
        let t = self.t();
        // the entry being changed is found again by whose it is and when it
        // starts: a merge since the last frame may have moved it in the list,
        // changed it or taken it away
        if let Some(ed) = &self.sweep.entry_edit {
            let there = self
                .data
                .tasks
                .iter()
                .find(|x| x.id == ed.task)
                .is_some_and(|x| {
                    x.time_entries
                        .iter()
                        .any(|e| e.member == ed.member && e.start == ed.start)
                });
            if !there {
                self.sweep.entry_edit = None;
                self.toast(tr("The time entry you were changing was changed somewhere else, so it was closed without saving."));
            }
        }
        // (start, end, task name, task, whose as saved)
        let mut rows: Vec<(i64, i64, String, u64, u64)> = vec![];
        for x in &self.data.tasks {
            for e in &x.time_entries {
                let i = (local_date(e.start) - ws).num_days();
                if (0..7).contains(&i) && self.data.is_my_entry(x, e) {
                    // time on an archived task still counts: mark the entry so
                    let name = if self.data.task_live(x) {
                        x.name.clone()
                    } else {
                        trf!("{} (archived)", x.name)
                    };
                    rows.push((e.start, e.end, name, x.id, e.member));
                }
            }
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r.0));
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(10)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // (task, whose, start) of the entry to take away
                let mut remove = None;
                // (task, whose, old start, new start, hours)
                let mut save = None;
                for (s, e, name, id, member) in &rows {
                    ui.horizontal(|ui| {
                        let local = local(*s);
                        let when = local
                            .map(|d| d.text("%a %b %-d, %-I:%M %p"))
                            .unwrap_or_default();
                        if let Some(ed) = self
                            .sweep
                            .entry_edit
                            .as_mut()
                            .filter(|x| x.task == *id && x.member == *member && x.start == *s)
                        {
                            ui.label(
                                RichText::new(
                                    local.map(|d| d.text("%a %b %-d")).unwrap_or_default(),
                                )
                                .color(t.text2),
                            );
                            let st =
                                ui.add(field(&mut ed.at).hint_text("09:00").desired_width(54.0));
                            name_it(&st, "Entry start");
                            let hr = ui.add(
                                egui::DragValue::new(&mut ed.hours)
                                    .range(0.0..=24.0)
                                    .speed(0.05)
                                    .max_decimals(2)
                                    .suffix("h"),
                            );
                            name_it(&hr, "Entry hours");
                            ui.label(name.as_str());
                            if ui.button(tr("Save")).clicked() {
                                let day = local.map(|d| d.date_naive()).unwrap_or_else(today);
                                save = Some((*id, *member, *s, reminder_ts(day, &ed.at), ed.hours));
                            }
                            if ui.button(tr("Cancel")).clicked() {
                                self.sweep.entry_edit = None;
                            }
                            return;
                        }
                        ui.label(RichText::new(when).color(t.text2));
                        if ui.link(name).clicked() {
                            self.actions.push(Action::Open(*id));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(ghost(egui::Button::new(
                                    RichText::new("\u{00D7}").color(t.text3),
                                )))
                                .clicked()
                            {
                                remove = Some((*id, *member, *s));
                            }
                            let ed =
                                ui.add(ghost(egui::Button::new(RichText::new(tr("Edit")).small())));
                            name_it(&ed, &format!("Edit entry {}", name));
                            if ed.clicked() {
                                let at = local
                                    .map(|d| d.format("%H:%M").to_string())
                                    .unwrap_or_else(|| "09:00".into());
                                self.sweep.entry_edit = Some(super::more::EntryEdit {
                                    task: *id,
                                    member: *member,
                                    start: *s,
                                    at,
                                    hours: (e - s) as f64 / 3600.0,
                                });
                            }
                            ui.label(fmt_duration(e - s));
                        });
                    });
                }
                if rows.is_empty() {
                    ui.label(RichText::new(tr("No time entries this week")).color(t.text3));
                }
                if let Some((id, member, old, start, hours)) = save {
                    if let Some(e) = self
                        .data
                        .tasks
                        .iter_mut()
                        .find(|x| x.id == id)
                        .and_then(|x| {
                            x.time_entries
                                .iter_mut()
                                .find(|e| e.member == member && e.start == old)
                        })
                    {
                        e.start = start;
                        e.end = start + (hours * 3600.0).round() as i64;
                        self.dirty = true;
                    }
                    self.sweep.entry_edit = None;
                }
                if let Some((id, member, start)) = remove {
                    if let Some(task) = self.data.tasks.iter_mut().find(|x| x.id == id) {
                        if let Some(k) = task
                            .time_entries
                            .iter()
                            .position(|e| e.member == member && e.start == start)
                        {
                            task.time_entries.remove(k);
                            self.sweep.entry_edit = None;
                            self.dirty = true;
                        }
                    }
                }
            });
    }

    fn all_timesheets(&mut self, ui: &mut Ui, ws: NaiveDate) {
        let t = self.t();
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(10)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // by whose time it is, not by who the task is assigned to
                let me = self.data.me;
                let solo = self.data.solo();
                let mut per: std::collections::HashMap<u64, i64> = std::collections::HashMap::new();
                for x in &self.data.tasks {
                    for e in &x.time_entries {
                        let i = (local_date(e.start) - ws).num_days();
                        if (0..7).contains(&i) {
                            let owner = match x.entry_owner(e) {
                                0 if solo => me,
                                o => o,
                            };
                            *per.entry(owner).or_default() += e.end - e.start;
                        }
                    }
                }
                for m in self.data.members.clone() {
                    let total = per.remove(&m.id).unwrap_or(0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                        paint_avatar(ui.painter(), r.center(), 11.0, &m, t.card);
                        ui.label(&m.name);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(if total > 0 {
                                fmt_duration(total)
                            } else {
                                "-".into()
                            });
                        });
                    });
                }
                // old entries on unassigned tasks, and people no longer here
                let rest: i64 = per.values().sum();
                if rest > 0 {
                    ui.horizontal(|ui| {
                        ui.add_space(30.0);
                        ui.label(RichText::new(tr("Unknown")).color(t.text2));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(fmt_duration(rest));
                        });
                    });
                }
            });
    }
}

/// How a week is stored with a timesheet row: its first day, counted in days.
fn sheet_week_key(ws: NaiveDate) -> u64 {
    ws.num_days_from_ce().max(0) as u64
}

/// A form template's question in the interface language: it is saved as the
/// question's text, so a Korean screen starts with Korean questions. Some
/// read differently in a form than elsewhere ("Comments" is 의견 here, not 댓글).
fn form_question(label: &str) -> &str {
    match label {
        "Your name" => crate::i18n::tr_ctx("Your name (form question)", label),
        "Comments" => crate::i18n::tr_ctx("Comments (form question)", label),
        "Goal" => crate::i18n::tr_ctx("Goal (form question)", label),
        "Role" => crate::i18n::tr_ctx("Role (form question)", label),
        "Full name" => crate::i18n::tr_ctx("Full name (form question)", label),
        "How was your experience?" => tr("How was your experience?"),
        "Project name" => tr("Project name"),
        "Deadline" => tr("Deadline"),
        "Customer" => tr("Customer"),
        "Items" => tr("Items"),
        "Quantity" => tr("Quantity"),
        "Needed by" => tr("Needed by"),
        "Experience" => tr("Experience"),
        "Links" => tr("Links"),
        "Issue" => tr("Issue"),
        "Urgency" => tr("Urgency"),
        _ => tr(label),
    }
}
