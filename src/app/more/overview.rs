// The Overview view of a Space or Folder: its cards, and the Lists card.

use super::*;

/// Space Overview cards that Customize can turn off.
pub const OV_CARDS: [&str; 5] = ["Recent", "Docs", "Bookmarks", "Folders", "Lists"];

// ---------------------------------------------------------------- overview view

impl App {
    pub(crate) fn overview_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        let lists = self.data.lists_in(self.loc).unwrap_or_default();
        // top row: Customize (turn cards on and off) and Add card
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let hidden: Vec<String> = self.data.extra.hidden_cards.clone();
                let add = ui.add_enabled(!hidden.is_empty(), egui::Button::new(tr("+ Add card")));
                popup(ui, &add, egui::Id::new("ov_add_card"), 180.0, |ui| {
                    for c in &hidden {
                        let b = ui.button(tr(c));
                        name_it(&b, &format!("Add card {}", c));
                        if b.clicked() {
                            self.data.extra.hidden_cards.retain(|x| x != c);
                            self.dirty = true;
                            close_popup(ui);
                        }
                    }
                });
                let cz = icon_button(ui, &t, "settings", tr("Customize"), "Customize", true);
                popup(ui, &cz, egui::Id::new("ov_customize"), 200.0, |ui| {
                    ui.label(RichText::new(tr("Cards")).small().color(t.text3));
                    for c in OV_CARDS {
                        let mut on = !self.data.extra.hidden_cards.iter().any(|x| x == c);
                        let cb = check(ui, &mut on, tr(c));
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
                        ui.label(RichText::new(tr("Your recently opened items will appear here.")).color(tt.text2));
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
                            ui.label(RichText::new(trf!("\u{2022} in {}", place)).small().color(tt.text3));
                        });
                    }
                });
                }
                // Docs
                let space = self.current_space();
                if card_on(self, "Docs") {
                self.ov_card(ui, cw, "Docs", |s, ui| {
                    let me = s.data.me;
                    let docs: Vec<(u64, String)> = s.data.docs.iter().filter(|d| (space.is_none() || d.space == space) && d.parent.is_none() && !d.archived && (!d.private || d.owner == me)).map(|d| (d.id, d.title.clone())).collect();
                    if docs.is_empty() {
                        ui.label(RichText::new(tr("There are no Docs in this location yet.")).color(s.t().text2));
                    }
                    for (id, n) in docs {
                        if ui.link(format!("\u{1F4C4} {}", n)).clicked() {
                            s.doc_open = Some(id);
                            s.section = Section::Docs;
                        }
                    }
                    ui.add_space(6.0);
                    if primary_button(ui, &s.t(), tr("Add a Doc")).clicked() {
                        s.new_doc(space);
                    }
                });
                }
                // Bookmarks
                if card_on(self, "Bookmarks") {
                self.ov_card(ui, cw, "Bookmarks", |s, ui| {
                    let bms = s.data.extra.bookmarks.clone();
                    if bms.is_empty() {
                        ui.label(RichText::new(tr("Bookmarks make it easy to save items or any URL from around the web.")).color(s.t().text2));
                    }
                    let mut remove = None;
                    for b in &bms {
                        ui.horizontal(|ui| {
                            if ui.link(format!("\u{1F516} {}", b.title)).on_hover_text(&b.url).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(&b.url));
                            }
                            if ui.add(ghost(egui::Button::new(RichText::new("\u{00D7}").size(10.0).color(s.t().text3)))).clicked() {
                                remove = Some(b.id);
                            }
                        });
                    }
                    if let Some(id) = remove {
                        s.data.extra.bookmarks.retain(|b| b.id != id);
                        s.dirty = true;
                    }
                    ui.add_space(6.0);
                    if primary_button(ui, &s.t(), tr("Add bookmark")).clicked() {
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
                        ui.label(RichText::new(tr("Add new Folder to your Space")).color(s.t().text2));
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (id, n, c) in &folders {
                            let label = if *c == 1 { trf!("\u{1F4C1} {}  (1 list)", n) } else { trf!("\u{1F4C1} {}  ({} lists)", n, c) };
                            if ui.add(egui::Button::new(label).min_size(vec2(200.0, 40.0)).stroke(Stroke::new(1.0_f32, s.t().border))).clicked() {
                                s.actions.push(Action::Go(Location::Folder(*id)));
                            }
                        }
                    });
                    if let Location::Space(sid) = s.loc {
                        ui.add_space(6.0);
                        if primary_button(ui, &s.t(), tr("Add Folder")).clicked() {
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
        const COLS: [(f32, &str); 7] = [
            (0.0, "Name"),
            (0.28, "Color"),
            (0.36, "Progress"),
            (0.55, "Start"),
            (0.66, "End"),
            (0.77, "Priority"),
            (0.88, "Owner"),
        ];
        let (hr, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
        for (x, h) in COLS {
            ui.painter().text(
                pos2(hr.left() + w * x, hr.center().y),
                Align2::LEFT_CENTER,
                tr(h),
                FontId::proportional(12.5),
                tt.text2,
            );
        }
        let cell = |r: Rect, i: usize| {
            let x0 = r.left() + w * COLS[i].0;
            let x1 = if i + 1 < COLS.len() {
                r.left() + w * COLS[i + 1].0
            } else {
                r.right()
            };
            Rect::from_min_max(pos2(x0, r.top()), pos2(x1 - 6.0, r.bottom()))
        };
        for lid in lists {
            let Some(item) = self.data.list_item(*lid).cloned().filter(|l| !l.archived) else {
                continue;
            };
            let total = self
                .data
                .tasks
                .iter()
                .filter(|x| x.list_id == *lid && x.parent.is_none() && !x.archived)
                .count();
            let done = self
                .data
                .tasks
                .iter()
                .filter(|x| {
                    x.list_id == *lid && x.parent.is_none() && !x.archived && self.data.is_closed(x)
                })
                .count();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click());
            name_it(&resp, &format!("Overview list {}", item.name));
            if resp.hovered() {
                ui.painter().rect_filled(r, 5.0, tt.hover);
            }
            let p = ui.painter().clone();
            p.with_clip_rect(cell(r, 0)).text(
                r.left_center() + vec2(4.0, 0.0),
                Align2::LEFT_CENTER,
                &item.name,
                FontId::proportional(14.0),
                tt.text,
            );
            let mut edited = item.clone();
            // color
            let c0 = cell(r, 1);
            let cr = ui.interact(c0, ui.id().with(("ov_color", lid)), Sense::click());
            hover_bg(ui, &cr, c0.shrink(2.0), 4.0);
            name_it(&cr, &format!("Color of {}", item.name));
            match item.color {
                Some(c) => {
                    p.circle_filled(pos2(c0.left() + 8.0, c0.center().y), 6.0, rgb(c));
                }
                None => {
                    p.circle_stroke(
                        pos2(c0.left() + 8.0, c0.center().y),
                        6.0,
                        Stroke::new(1.0_f32, tt.text3),
                    );
                }
            }
            popup(ui, &cr, ui.id().with(("ov_color_p", lid)), 160.0, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for c in STATUS_PALETTE.iter().skip(1) {
                        let (b, br) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                        ui.painter().circle_filled(b.center(), 8.0, rgb(*c));
                        hover_ring(ui, &br, b.shrink(1.0), 9.0);
                        if br.clicked() {
                            edited.color = Some(*c);
                            close_popup(ui);
                        }
                    }
                });
                if ui.small_button(tr("No color")).clicked() {
                    edited.color = None;
                    close_popup(ui);
                }
            });
            // progress
            let c2 = cell(r, 2);
            let bar = Rect::from_min_size(
                pos2(c2.left(), c2.center().y - 3.0),
                vec2((c2.width() - 40.0).max(20.0), 6.0),
            );
            p.rect_filled(bar, 3.0, tt.hover);
            if total > 0 {
                p.rect_filled(
                    Rect::from_min_size(
                        bar.min,
                        vec2(bar.width() * done as f32 / total as f32, 6.0),
                    ),
                    3.0,
                    rgb(COMPLETE_COLOR),
                );
            }
            p.text(
                pos2(bar.right() + 6.0, c2.center().y),
                Align2::LEFT_CENTER,
                format!("{}/{}", done, total),
                FontId::proportional(12.0),
                tt.text2,
            );
            // start / end
            for (i, which) in [(3usize, 0u8), (4, 1)] {
                let c = cell(r, i);
                let dr = ui.interact(c, ui.id().with(("ov_date", lid, which)), Sense::click());
                hover_bg(ui, &dr, c.shrink(2.0), 4.0);
                name_it(
                    &dr,
                    &format!(
                        "{} of {}",
                        if which == 0 { "Start" } else { "End" },
                        item.name
                    ),
                );
                let v = if which == 0 { item.start } else { item.end };
                match v {
                    Some(d) => {
                        p.text(
                            c.left_center(),
                            Align2::LEFT_CENTER,
                            fmt_date(d),
                            FontId::proportional(13.0),
                            tt.text,
                        );
                    }
                    None => {
                        p.text(
                            c.left_center(),
                            Align2::LEFT_CENTER,
                            "\u{2013}",
                            FontId::proportional(13.0),
                            tt.text3,
                        );
                    }
                }
                let mut val = v;
                if let Some(true) = popup(
                    ui,
                    &dr,
                    ui.id().with(("ov_date_p", lid, which)),
                    300.0,
                    |ui| date_menu(ui, &tt, ui.id().with(("ov_cal", lid, which)), &mut val),
                ) {
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
            hover_bg(ui, &prr, c5.shrink(2.0), 4.0);
            name_it(&prr, &format!("Priority of {}", item.name));
            if item.priority > 0 {
                paint_flag(
                    &p,
                    c5.left_center() + vec2(4.0, 0.0),
                    priority_color(item.priority, &tt),
                    12.0,
                );
                p.text(
                    c5.left_center() + vec2(16.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr(PRIORITIES[item.priority as usize].label),
                    FontId::proportional(13.0),
                    tt.text,
                );
            } else {
                p.text(
                    c5.left_center(),
                    Align2::LEFT_CENTER,
                    "\u{2013}",
                    FontId::proportional(13.0),
                    tt.text3,
                );
            }
            let mut pv = item.priority;
            if let Some(true) = popup(ui, &prr, ui.id().with(("ov_pr_p", lid)), 180.0, |ui| {
                priority_menu(ui, &tt, &mut pv)
            }) {
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
                    if menu_row(ui, &tt, &m.name, item.owner == Some(m.id), |p, c| {
                        paint_avatar(p, c, 8.0, m, ring)
                    })
                    .clicked()
                    {
                        edited.owner = if item.owner == Some(m.id) {
                            None
                        } else {
                            Some(m.id)
                        };
                        close_popup(ui);
                    }
                }
            });
            let any_cell = [cr.hovered(), prr.hovered(), orr.hovered()]
                .iter()
                .any(|x| *x)
                || [c0, cell(r, 3), cell(r, 4), c5, c6]
                    .iter()
                    .any(|c| ui.rect_contains_pointer(*c));
            let changed = edited.color != item.color
                || edited.start != item.start
                || edited.end != item.end
                || edited.priority != item.priority
                || edited.owner != item.owner;
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
        if ui
            .add(ghost(egui::Button::new(
                RichText::new(tr("+ New List")).color(tt.text2),
            )))
            .clicked()
        {
            let space = self.current_space();
            let folder = if let Location::Folder(f) = self.loc {
                Some(f)
            } else {
                None
            };
            self.open_new_list(space, folder);
        }
    }

    fn ov_card(&mut self, ui: &mut Ui, w: f32, title: &str, add: impl FnOnce(&mut Self, &mut Ui)) {
        let t = self.t();
        card_frame(&t).fill(t.card).show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(w - 34.0);
                ui.set_min_height(150.0);
                ui.label(RichText::new(tr(title)).size(15.0));

                ui.add_space(8.0);
                add(self, ui);
            });
        });
    }
}
