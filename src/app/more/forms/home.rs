// The Forms hub: its sidebar (the filters, a search and every form, so a
// form just made is there at once), the table of forms and the templates
// under it.

use super::*;
use crate::app::hubs::OwnFilter;

/// The table's fixed columns after the name: (heading, width).
const COLS: [(&str, f32); 6] = [
    ("List for new tasks", 200.0),
    ("Questions", 72.0),
    ("Answers", 84.0),
    ("Made by", 80.0),
    ("Edited", 110.0),
    ("", 120.0),
];

impl App {
    pub(crate) fn forms_hub(&mut self, ui: &mut Ui) {
        let t = self.t();
        self.forms_sidebar(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.bg))
            .show_inside(ui, |ui| {
                if self.form_open.is_some() {
                    self.form_detail(ui);
                } else {
                    self.forms_home(ui);
                }
            });
    }

    /// The forms this person sees, in the table's order: last edited first
    /// (or by name), narrowed by the search.
    fn forms_listed(&self) -> Vec<Form> {
        let q = self.sweep.forms.search.trim().to_lowercase();
        let mut v: Vec<Form> = self
            .data
            .extra
            .forms
            .iter()
            .filter(|f| self.form_shown(f))
            .filter(|f| q.is_empty() || form_title(f).to_lowercase().contains(&q))
            .cloned()
            .collect();
        if self.sweep.forms.by_name {
            v.sort_by_key(|f| form_title(f).to_lowercase());
        } else {
            v.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.name.cmp(&b.name)));
        }
        v
    }

    /// Whether a form passes the sidebar filter (0 all, 1 mine, 2 favorites).
    fn form_in_filter(&self, f: &Form, filter: OwnFilter) -> bool {
        match filter {
            // mine: the ones I made (older ones without a maker show in both)
            OwnFilter::Mine => f.owner == self.data.me || f.owner == 0,
            OwnFilter::Favorites => self.data.extra.starred.contains(&format!("form:{}", f.id)),
            OwnFilter::All => true,
        }
    }

    /// The hub's sidebar: a search, All / My / Favorites, and every form
    /// with its tile; « closes it, as on the other hubs. The other hubs'
    /// sidebars are laid out as this one, from the same pieces
    /// (hubs/side.rs).
    fn forms_sidebar(&mut self, ui: &mut Ui) {
        use crate::app::hubs::side::{
            side_empty, side_heading, side_rule, side_search, SideFilter,
        };
        self.hub_side_panel(ui, |app, ui| {
            let t = app.t();
            app.side_title(ui, tr("Forms"));
            side_search(
                ui,
                &t,
                &mut app.sweep.forms.search,
                tr("Find a form"),
                "Find a form",
            );
            let shown: Vec<Form> = app
                .data
                .extra
                .forms
                .iter()
                .filter(|f| app.form_shown(f))
                .cloned()
                .collect();
            let filters: Vec<SideFilter> = [
                ("forms", "All forms"),
                ("@me", "My forms"),
                ("star", "Favorites"),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (icon, label))| SideFilter {
                key: i as u8,
                icon,
                label,
                count: Some(
                    shown
                        .iter()
                        .filter(|f| app.form_in_filter(f, OwnFilter::from_key(i as u8)))
                        .count(),
                ),
            })
            .collect();
            let sel = app.form_open.is_none().then_some(app.hub.forms.key());
            if let Some(i) = app.side_filters(ui, &filters, sel) {
                app.hub.forms = OwnFilter::from_key(i);
                app.form_open = None;
            }
            side_rule(ui, &t);
            if side_heading(ui, &t, tr("Forms"), Some(("Add a form", tr("New form")))) {
                app.new_form_from(None);
            }
            let listed = app.forms_listed();
            if listed.is_empty() {
                side_empty(
                    ui,
                    &t,
                    if app.sweep.forms.search.trim().is_empty() {
                        tr("No forms yet")
                    } else {
                        tr("No form has that name")
                    },
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("forms_side_list")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for f in &listed {
                        app.form_side_row(ui, &t, f);
                    }
                });
        });
    }

    /// A form in the sidebar: its tile and name, "New" when made this
    /// session, and a warning when its answers have no List to go to.
    fn form_side_row(&mut self, ui: &mut Ui, t: &Theme, f: &Form) {
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
        let title = form_title(f);
        name_it(&resp, &format!("Open form {}", title));
        let open = self.form_open == Some(f.id);
        if open {
            ui.painter().rect_filled(r, 7.0, t.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 7.0, t.hover);
        }
        let tile = Rect::from_center_size(r.left_center() + vec2(18.0, 0.0), vec2(22.0, 22.0));
        paint_tile(ui.painter(), tile, form_look(f, t));
        let mut right = r.right() - 8.0;
        if self.data.space_of_list(f.list_id).is_none() {
            let c = pos2(right - 8.0, r.center().y);
            line_icon_scaled(ui.painter(), "help", c, WARN_ORANGE, 0.85);
            let warn = ui.interact(
                Rect::from_center_size(c, vec2(18.0, 18.0)),
                ui.id().with(("form_no_list", f.id)),
                Sense::hover(),
            );
            warn.on_hover_text(tr("No List for its answers yet"));
            right -= 22.0;
        } else if self.sweep.forms.made.contains(&f.id) {
            let label = crate::i18n::tr_ctx("New (form badge)", "New");
            let w = ui
                .painter()
                .layout_no_wrap(label.to_string(), FontId::proportional(11.0), t.accent_text)
                .size()
                .x
                + 14.0;
            tag_pill(
                ui.painter(),
                pos2(right - w, r.center().y),
                label,
                t.accent_text,
                t.accent_bg,
            );
            right -= w + 6.0;
        }
        let font = FontId::proportional(14.0);
        let left = tile.right() + 10.0;
        let shown = fit_text(ui.painter(), &title, &font, (right - left).max(10.0));
        ui.painter().text(
            pos2(left, r.center().y),
            Align2::LEFT_CENTER,
            shown,
            font,
            if open { t.text } else { t.text2 },
        );
        if resp.clicked() && !open {
            self.open_form(f.id, FormTab::Edit);
        }
    }

    /// The hub's page with no form open: the filter's forms in a table and
    /// the templates under it.
    pub(super) fn forms_home(&mut self, ui: &mut Ui) {
        let t = self.t();
        let filter = self.hub.forms;
        let forms: Vec<Form> = self
            .forms_listed()
            .into_iter()
            .filter(|f| self.form_in_filter(f, filter))
            .collect();
        let answers = self.form_answer_ids();
        egui::ScrollArea::vertical()
            .id_salt("forms_home")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 32,
                        right: 32,
                        top: 24,
                        bottom: 40,
                    })
                    .show(ui, |ui| {
                        let title = ["All forms", "My forms", "Favorites"][filter as usize];
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr(title)).size(22.0).color(t.text));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if primary_icon(ui, &t, "plus", tr("New form"), "New form")
                                    .clicked()
                                {
                                    self.new_form_from(None);
                                }
                            });
                        });
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(if forms.len() == 1 {
                                    tr("1 form").to_string()
                                } else {
                                    trf!("{} forms", forms.len())
                                })
                                .size(13.0)
                                .color(t.text2),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let label = if self.sweep.forms.by_name {
                                    crate::i18n::tr_ctx("Name (sort)", "Name")
                                } else {
                                    tr("Last edited")
                                };
                                if icon_button(ui, &t, "sort", label, "Sort forms", false)
                                    .on_hover_text(tr("Sort by last edited or by name"))
                                    .clicked()
                                {
                                    self.sweep.forms.by_name = !self.sweep.forms.by_name;
                                }
                            });
                        });
                        ui.add_space(8.0);
                        if forms.is_empty() {
                            let text = if !self.sweep.forms.search.trim().is_empty() {
                                tr("No form has that name")
                            } else {
                                match filter {
                                    OwnFilter::Mine => tr("You haven't made a form yet."),
                                    OwnFilter::Favorites => tr("Star a Form to see it here"),
                                    OwnFilter::All => tr(
                                        "No forms yet. Make one, or start from a template below.",
                                    ),
                                }
                            };
                            egui::Frame::new()
                                .stroke(Stroke::new(1.0_f32, t.border))
                                .corner_radius(12)
                                .inner_margin(egui::Margin::symmetric(16, 28))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.vertical_centered(|ui| {
                                        ui.label(RichText::new(text).size(14.0).color(t.text2));
                                    });
                                });
                        } else {
                            self.forms_table(ui, &t, &forms, &answers);
                        }
                        ui.add_space(32.0);
                        self.form_templates(ui, &t);
                    });
            });
    }

    /// The table of forms: name, List, questions, answers, maker, last
    /// edit, and Fill out / star / menu on each row. A row opens its form.
    fn forms_table(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        forms: &[Form],
        answers: &HashMap<u64, Vec<u64>>,
    ) {
        let fixed: f32 = COLS.iter().map(|c| c.1).sum();
        let w = ui.available_width().max(fixed + 240.0);
        egui::ScrollArea::horizontal()
            .id_salt("forms_table")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let (head, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
                ui.painter().rect_filled(
                    head,
                    egui::CornerRadius {
                        nw: 12,
                        ne: 12,
                        sw: 0,
                        se: 0,
                    },
                    t.column_bg,
                );
                let name_w = w - fixed;
                let font = FontId::proportional(12.0);
                ui.painter().text(
                    pos2(head.left() + 16.0, head.center().y),
                    Align2::LEFT_CENTER,
                    tr("Name"),
                    font.clone(),
                    t.text2,
                );
                let mut x = head.left() + name_w;
                for (i, (label, cw)) in COLS.iter().enumerate() {
                    if !label.is_empty() {
                        let (at, align) = if i == 0 {
                            (pos2(x + 12.0, head.center().y), Align2::LEFT_CENTER)
                        } else {
                            (pos2(x + cw / 2.0, head.center().y), Align2::CENTER_CENTER)
                        };
                        ui.painter()
                            .text(at, align, tr(label), font.clone(), t.text2);
                    }
                    x += cw;
                }
                let top = head.top();
                let mut bottom = head.bottom();
                for f in forms {
                    let n = answers.get(&f.id).map_or(0, |v| v.len());
                    bottom = self.form_table_row(ui, t, f, n, w, name_w).bottom();
                }
                ui.painter().rect_stroke(
                    Rect::from_min_max(pos2(head.left(), top), pos2(head.left() + w, bottom)),
                    12.0,
                    Stroke::new(1.0_f32, t.border),
                    egui::StrokeKind::Inside,
                );
            });
    }

    /// One row of the forms table; returns its rectangle.
    fn form_table_row(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        f: &Form,
        answers: usize,
        w: f32,
        name_w: f32,
    ) -> Rect {
        let (r, row) = ui.allocate_exact_size(vec2(w, 60.0), Sense::click());
        let title = form_title(f);
        name_it(&row, &format!("Form {}", title));
        let p = ui.painter().clone();
        let just_made = self.sweep.forms.made.contains(&f.id);
        if row.hovered() {
            p.rect_filled(r.shrink(1.0), 0.0, t.column_bg);
        } else if just_made {
            p.rect_filled(r.shrink(1.0), 0.0, t.accent_bg.gamma_multiply(0.7));
        }
        p.hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
        let cy = r.center().y;
        // the tile opens the icon picker
        let tile = Rect::from_min_size(pos2(r.left() + 16.0, cy - 16.0), vec2(32.0, 32.0));
        let tresp = ui
            .interact(tile, ui.id().with(("form_tile", f.id)), Sense::click())
            .on_hover_text(tr("Change icon"));
        name_it(&tresp, &format!("Change icon {}", title));
        hover_ring(ui, &tresp, tile, 8.0);
        paint_tile(&p, tile, form_look(f, t));
        let picked = popup(
            ui,
            &tresp,
            ui.id().with(("form_icon_pick", f.id)),
            292.0,
            |ui| match self.data.extra.forms.iter_mut().find(|x| x.id == f.id) {
                Some(form) => icon_picker(ui, t, form),
                None => false,
            },
        );
        if picked == Some(true) {
            self.form_touched(f.id);
        }
        // name, "Just made", the description
        let left = tile.right() + 12.0;
        let room = (r.left() + name_w - 12.0 - left).max(20.0);
        let font = FontId::proportional(14.0);
        let name = fit_text(&p, &title, &font, room);
        let g = p.layout_no_wrap(name, font, t.text);
        let name_end = left + g.size().x;
        p.galley(pos2(left, cy - 9.0 - g.size().y / 2.0), g, t.text);
        if just_made && name_end + 80.0 < left + room {
            tag_pill(
                &p,
                pos2(name_end + 8.0, cy - 9.0),
                tr("Just made"),
                t.accent_text,
                t.accent_bg,
            );
        }
        let desc = crate::widgets::one_line(&f.description);
        let desc = fit_text(&p, &desc, &FontId::proportional(12.0), room);
        p.text(
            pos2(left, cy + 10.0),
            Align2::LEFT_CENTER,
            desc,
            FontId::proportional(12.0),
            t.text2,
        );
        // List for new tasks, or a warning that opens the form to choose one
        let mut x = r.left() + name_w;
        let lw = COLS[0].1;
        let path = self
            .all_list_paths()
            .into_iter()
            .find(|l| l.0 == f.list_id)
            .map(|l| l.1);
        match path {
            Some(path) => {
                line_icon_scaled(&p, "list", pos2(x + 20.0, cy), t.text2, 0.85);
                let font = FontId::proportional(13.0);
                let shown = fit_text(&p, &path, &font, lw - 40.0);
                p.text(
                    pos2(x + 32.0, cy),
                    Align2::LEFT_CENTER,
                    shown,
                    font,
                    t.text2,
                );
            }
            None if self.data.space_of_list(f.list_id).is_some() => {
                // archived or in a place this person can't pick from
                let font = FontId::proportional(13.0);
                let shown = fit_text(&p, &self.data.list_name(f.list_id), &font, lw - 24.0);
                p.text(
                    pos2(x + 12.0, cy),
                    Align2::LEFT_CENTER,
                    shown,
                    font,
                    t.text3,
                );
            }
            None => {
                let label = tr("Choose a List");
                let g =
                    p.layout_no_wrap(label.to_string(), FontId::proportional(12.0), WARN_ORANGE);
                let cr =
                    Rect::from_min_size(pos2(x + 10.0, cy - 12.0), vec2(g.size().x + 34.0, 24.0));
                let c = ui.interact(cr, ui.id().with(("form_pick_list", f.id)), Sense::click());
                name_it(&c, &format!("Choose a List for {}", title));
                p.rect_filled(
                    cr,
                    6.0,
                    WARN_ORANGE.gamma_multiply(if c.hovered() { 0.22 } else { 0.12 }),
                );
                line_icon_scaled(&p, "help", pos2(cr.left() + 13.0, cy), WARN_ORANGE, 0.75);
                p.galley(
                    pos2(cr.left() + 24.0, cy - g.size().y / 2.0),
                    g,
                    WARN_ORANGE,
                );
                if c.clicked() {
                    self.open_form(f.id, FormTab::Edit);
                }
            }
        }
        x += lw;
        // questions and answers, centred
        p.text(
            pos2(x + COLS[1].1 / 2.0, cy),
            Align2::CENTER_CENTER,
            f.fields.len().to_string(),
            FontId::proportional(13.0),
            t.text,
        );
        x += COLS[1].1;
        p.text(
            pos2(x + COLS[2].1 / 2.0, cy),
            Align2::CENTER_CENTER,
            answers.to_string(),
            FontId::proportional(13.0),
            if answers == 0 { t.text3 } else { t.text },
        );
        x += COLS[2].1;
        // the maker's avatar; the name on hover
        let c = pos2(x + COLS[3].1 / 2.0, cy);
        match self.data.member(f.owner) {
            Some(m) => {
                paint_avatar(&p, c, 11.0, m, t.bg);
                let a = ui.interact(
                    Rect::from_center_size(c, vec2(24.0, 24.0)),
                    ui.id().with(("form_owner", f.id)),
                    Sense::hover(),
                );
                a.on_hover_text(m.shown().to_string());
            }
            None => {
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    "\u{2014}",
                    FontId::proportional(13.0),
                    t.text3,
                );
            }
        }
        x += COLS[3].1;
        p.text(
            pos2(x + COLS[4].1 / 2.0, cy),
            Align2::CENTER_CENTER,
            edited_text(f.updated),
            FontId::proportional(13.0),
            t.text2,
        );
        x += COLS[4].1;
        // Fill out, the star and the menu
        let fill_r = Rect::from_min_size(pos2(x, cy - 14.0), vec2(56.0, 28.0));
        let fill = ui.interact(
            fill_r,
            ui.id().with(("form_row_fill", f.id)),
            Sense::click(),
        );
        name_it(&fill, &format!("Fill out {}", title));
        if fill.hovered() {
            p.rect_filled(fill_r, 6.0, hover_fill(ui));
        }
        p.text(
            fill_r.center(),
            Align2::CENTER_CENTER,
            tr("Fill out"),
            FontId::proportional(13.0),
            t.text2,
        );
        if fill.clicked() {
            self.open_form(f.id, FormTab::Fill);
        }
        let key = format!("form:{}", f.id);
        let starred = self.data.extra.starred.contains(&key);
        let star_r = Rect::from_min_size(pos2(fill_r.right() + 2.0, cy - 14.0), vec2(28.0, 28.0));
        let star = ui
            .interact(
                star_r,
                ui.id().with(("form_row_star", f.id)),
                Sense::click(),
            )
            .on_hover_text(if starred {
                tr("Remove from Favorites")
            } else {
                tr("Add to Favorites")
            });
        name_it(&star, &format!("Star {}", key));
        if star.hovered() {
            p.rect_filled(star_r, 6.0, hover_fill(ui));
        }
        p.text(
            star_r.center(),
            Align2::CENTER_CENTER,
            if starred { "\u{2605}" } else { "\u{2606}" },
            FontId::proportional(16.0),
            if starred { STAR_YELLOW } else { t.text2 },
        );
        if star.clicked() {
            if starred {
                self.data.extra.starred.retain(|s| s != &key);
            } else {
                self.data.extra.starred.push(key);
            }
            self.dirty = true;
        }
        let more_r = Rect::from_min_size(pos2(star_r.right() + 2.0, cy - 14.0), vec2(28.0, 28.0));
        let more = ui.interact(
            more_r,
            ui.id().with(("form_row_more", f.id)),
            Sense::click(),
        );
        name_it(&more, &format!("Form menu {}", title));
        if more.hovered() {
            p.rect_filled(more_r, 6.0, hover_fill(ui));
        }
        line_icon(&p, "dots", more_r.center(), t.text2);
        popup(
            ui,
            &more,
            ui.id().with(("form_row_menu", f.id)),
            180.0,
            |ui| {
                if menu_row(ui, t, tr("Edit"), false, |p, c| {
                    line_icon(p, "edit", c, t.text2);
                })
                .clicked()
                {
                    self.open_form(f.id, FormTab::Edit);
                    close_popup(ui);
                }
                if menu_row(ui, t, tr("Answers"), false, |p, c| {
                    line_icon(p, "inbox", c, t.text2);
                })
                .clicked()
                {
                    self.open_form(f.id, FormTab::Answers);
                    close_popup(ui);
                }
                ui.separator();
                let del = menu_row_named(ui, t, "Delete form", tr("Delete form"), false, |p, c| {
                    line_icon(p, "trash", c, t.danger);
                });
                if del.clicked() {
                    self.modal = Some(Modal::ConfirmRemove {
                        kind: "form",
                        id: f.id,
                        name: title.clone(),
                    });
                    close_popup(ui);
                }
            },
        );
        if row.clicked() {
            self.open_form(f.id, FormTab::Edit);
        }
        r
    }

    /// The templates under the table: a card each, and a blank form.
    fn form_templates(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(
            RichText::new(tr("Start from a template"))
                .size(15.0)
                .color(t.text),
        );
        ui.label(
            RichText::new(tr("Start from a filled-in draft instead of a blank form."))
                .size(13.0)
                .color(t.text2),
        );
        ui.add_space(12.0);
        let w = ui.available_width();
        let gap = 12.0;
        let cols = (((w + gap) / 232.0).floor() as usize).clamp(1, 6);
        let cw = (w - gap * (cols - 1) as f32) / cols as f32;
        let cards: Vec<Option<usize>> = (0..FORM_TEMPLATES.len()).map(Some).chain([None]).collect();
        let mut chosen = None;
        for row in cards.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for c in row {
                    if self.form_template_card(ui, t, *c, cw).clicked() {
                        chosen = Some(*c);
                    }
                }
            });
            ui.add_space(gap);
        }
        if let Some(c) = chosen {
            self.new_form_from(c);
        }
    }

    /// A template's card: its tile, name and questions; `None` is the blank form.
    fn form_template_card(
        &self,
        ui: &mut Ui,
        t: &Theme,
        tpl: Option<usize>,
        w: f32,
    ) -> egui::Response {
        let (r, resp) = ui.allocate_exact_size(vec2(w, 64.0), Sense::click());
        let p = ui.painter();
        let (name, look, sub) = match tpl.and_then(|i| FORM_TEMPLATES.get(i)) {
            Some((n, icon, color, qs)) => {
                let c = FORM_COLORS
                    .iter()
                    .find(|x| x.0 == *color)
                    .map_or(FORM_COLORS[0].1, |x| x.1);
                let fg = if t.dark {
                    chip_text_on_dark(rgb(c))
                } else {
                    rgb(c)
                };
                let bg = rgb(c).gamma_multiply(if t.dark { 0.3 } else { 0.13 });
                let labels: Vec<&str> = qs.iter().map(|(l, _)| form_question(l)).collect();
                (
                    *n,
                    (*icon, fg, bg),
                    format!(
                        "{} \u{00B7} {}",
                        trf!("{} questions", qs.len()),
                        labels.join(", ")
                    ),
                )
            }
            None => (
                "Blank form",
                ("plus", t.text2, t.hover),
                format!(
                    "{} \u{00B7} {}",
                    trf!("{} questions", 2),
                    [tr("Title"), tr("Details")].join(", ")
                ),
            ),
        };
        name_it(&resp, name);
        let hovered = resp.hovered();
        p.rect_filled(r, 12.0, if hovered { t.column_bg } else { t.card });
        let stroke = Stroke::new(1.0_f32, if hovered { hover_line(ui) } else { t.border });
        if tpl.is_some() {
            p.rect_stroke(r, 12.0, stroke, egui::StrokeKind::Inside);
        } else {
            let r2 = r.shrink(0.5);
            p.extend(egui::Shape::dashed_line(
                &[
                    r2.left_top(),
                    r2.right_top(),
                    r2.right_bottom(),
                    r2.left_bottom(),
                    r2.left_top(),
                ],
                stroke,
                5.0,
                4.0,
            ));
        }
        let tile =
            Rect::from_min_size(pos2(r.left() + 14.0, r.center().y - 16.0), vec2(32.0, 32.0));
        paint_tile(p, tile, look);
        let left = tile.right() + 12.0;
        let room = r.right() - 12.0 - left;
        p.text(
            pos2(left, r.center().y - 9.0),
            Align2::LEFT_CENTER,
            fit_text(p, tr(name), &FontId::proportional(14.0), room),
            FontId::proportional(14.0),
            t.text,
        );
        p.text(
            pos2(left, r.center().y + 10.0),
            Align2::LEFT_CENTER,
            fit_text(p, &sub, &FontId::proportional(12.0), room),
            FontId::proportional(12.0),
            t.text2,
        );
        resp
    }

    /// Notes that a form changed now (the table's "Edited").
    pub(super) fn form_touched(&mut self, id: u64) {
        if let Some(f) = self.data.extra.forms.iter_mut().find(|f| f.id == id) {
            f.updated = now_ts();
        }
        self.dirty = true;
    }
}
