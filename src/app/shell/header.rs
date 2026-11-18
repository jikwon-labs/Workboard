// The page header above a List, Board or other view: the breadcrumb row with
// its buttons, the view tabs, and the toolbar of filter and sort chips.

use super::*;

/// A toolbar chip with an outline: a line icon (`icon`), then `shown` (empty
/// in a narrow window), named `text`. As tall as the other controls and
/// rounded like them, so it matches the buttons beside it.
pub(super) fn pill(
    ui: &mut Ui,
    t: &Theme,
    text: &str,
    shown: &str,
    active: bool,
    accent: bool,
    icon: &str,
) -> Response {
    let font = FontId::proportional(13.0);
    let (fg, bg, stroke) = if accent {
        (t.accent_text, t.accent_bg, t.accent_bg)
    } else if active {
        (t.text, t.row_selected, t.border)
    } else {
        (t.text2, t.bg, t.border)
    };
    let g = ui.painter().layout_no_wrap(shown.to_string(), font, fg);
    let text_w = if shown.is_empty() {
        0.0
    } else {
        g.size().x + 6.0
    };
    let (r, resp) = ui.allocate_exact_size(
        vec2(16.0 + text_w + 18.0, crate::widgets::CONTROL_H),
        Sense::click(),
    );
    name_it(&resp, text);
    let fill = if resp.hovered() && !accent {
        t.hover
    } else {
        bg
    };
    ui.painter().rect_filled(r, 6.0, fill);
    ui.painter().rect_stroke(
        r,
        6.0,
        Stroke::new(1.0_f32, stroke),
        egui::StrokeKind::Inside,
    );
    let ic = pos2(r.left() + 9.0 + 8.0, r.center().y);
    line_icon(ui.painter(), icon, ic, fg);
    if !shown.is_empty() {
        ui.painter().galley(
            pos2(ic.x + 8.0 + 6.0, r.center().y - g.size().y / 2.0),
            g,
            fg,
        );
    }
    resp
}

// ---------------------------------------------------------------- page header

impl App {
    pub(crate) fn header(&mut self, ui: &mut Ui) {
        let t = self.t();
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 12,
                top: 8,
                bottom: 0,
            })
            .show(ui, |ui| {
                // breadcrumb
                self.header_breadcrumb(ui, t);
                // view tabs
                self.header_view_tabs(ui, t);
            });
        let r = ui.available_rect_before_wrap();
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
        // the Overview has cards, not tasks: no task toolbar
        if self.view == ViewKind::Overview {
            ui.add_space(8.0);
            return;
        }
        // toolbar
        self.header_toolbar(ui, t);
    }

    /// The header's first row: where I am (Space / Folder / List), the
    /// favourite star and notes, and Manage, Share, Ask, Automate and huddle
    /// on the right.
    fn header_breadcrumb(&mut self, ui: &mut Ui, t: Theme) {
        ui.horizontal(|ui| {
            ui.set_height(30.0);
            let mut crumbs: Vec<(String, u8, Option<[u8; 3]>)> = vec![];
            let personal = self.on_personal_list();
            match self.loc {
                Location::List(_) if personal => {
                    crumbs.push((tr("My Tasks").into(), 3, None));
                    crumbs.push((tr("Personal List").into(), 2, None));
                }
                Location::List(id) => {
                    if let Some(s) = self.data.space_of_list(id) {
                        crumbs.push((s.name.clone(), 0, Some(s.color)));
                        for f in &s.folders {
                            if f.lists.iter().any(|l| l.id == id) {
                                crumbs.push((f.name.clone(), 1, None));
                            }
                        }
                    }
                    crumbs.push((self.data.list_name(id), 2, None));
                }
                Location::Folder(fid) => {
                    for s in &self.data.spaces {
                        if let Some(f) = s.folders.iter().find(|f| f.id == fid) {
                            crumbs.push((s.name.clone(), 0, Some(s.color)));
                            crumbs.push((f.name.clone(), 1, None));
                        }
                    }
                }
                Location::Space(sid) => {
                    if let Some(s) = self.data.spaces.iter().find(|s| s.id == sid) {
                        crumbs.push((s.name.clone(), 0, Some(s.color)));
                    }
                }
                l => crumbs.push((tr(&self.data.location_name(l)).to_string(), 3, None)),
            }
            let n = crumbs.len();
            // the buttons on the right keep their room (their width is
            // remembered from the last frame); crumb names share what is
            // left and are cut with an ellipsis when it is not enough
            let right_id = ui.id().with("crumb_right_w");
            let right_w = ui.data(|d| d.get_temp::<f32>(right_id)).unwrap_or(360.0);
            let sp = ui.spacing().item_spacing.x;
            let font = FontId::proportional(15.0);
            let mut widths: Vec<f32> = crumbs
                .iter()
                .map(|(name, _, _)| {
                    ui.painter()
                        .layout_no_wrap(name.clone(), font.clone(), t.text)
                        .size()
                        .x
                })
                .collect();
            let room_for = |k: usize, ui: &Ui| {
                let fixed = k as f32 * (18.0 + 2.0 * sp)
                    + (k.saturating_sub(1)) as f32 * (6.0 + sp)
                    + (10.0 + sp)
                    + 30.0
                    + if personal { 20.0 } else { 0.0 };
                ui.available_width() - right_w - fixed - 12.0
            };
            // too narrow for every level: keep only the last ones
            let mut skip = 0;
            while n - skip > 1 && room_for(n - skip, ui) < 48.0 * (n - skip) as f32 {
                skip += 1;
            }
            crumbs.drain(..skip);
            widths.drain(..skip);
            let n = crumbs.len();
            let room = room_for(n, ui).max(40.0);
            let cap = {
                // largest cap with sum(min(w, cap)) <= room
                let mut sorted = widths.clone();
                sorted.sort_by(f32::total_cmp);
                let mut left = room;
                let mut cap = f32::INFINITY;
                for (k, w) in sorted.iter().enumerate() {
                    let share = left / (sorted.len() - k) as f32;
                    if *w > share {
                        cap = share;
                        break;
                    }
                    left -= w;
                }
                cap
            };
            for (i, (name, kind, color)) in crumbs.iter().enumerate() {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                match kind {
                    0 => {
                        ui.painter().rect_filled(
                            r.shrink(1.0),
                            4.0,
                            rgb(color.unwrap_or([96, 73, 231])),
                        );
                        let glyph = self
                            .data
                            .spaces
                            .iter()
                            .find(|s| &s.name == name && Some(s.color) == *color)
                            .map(Data::space_glyph)
                            .unwrap_or_else(|| {
                                name.chars()
                                    .next()
                                    .unwrap_or('S')
                                    .to_uppercase()
                                    .to_string()
                            });
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            glyph,
                            FontId::proportional(10.0),
                            Color32::WHITE,
                        );
                    }
                    1 => {
                        ui.painter().rect_stroke(
                            Rect::from_center_size(r.center(), vec2(14.0, 11.0)),
                            2.0,
                            Stroke::new(1.2_f32, t.text2),
                            egui::StrokeKind::Inside,
                        );
                    }
                    2 => paint_checklist_icon(ui.painter(), r.center(), t.text),
                    _ => {}
                }
                let last = i + 1 == n;
                let text =
                    RichText::new(name)
                        .size(15.0)
                        .color(if last { t.text } else { t.text2 });
                if widths[i] > cap {
                    ui.scope(|ui| {
                        ui.set_max_width(cap.max(20.0));
                        ui.add(egui::Label::new(text).truncate());
                    });
                } else {
                    ui.label(text);
                }
                if last && personal {
                    let (r, lr) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                    line_icon(ui.painter(), "lock", r.center(), t.text3);
                    lr.on_hover_text(tr("Private: only you can see this List"));
                }
                if last {
                    let (cr, _) = ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
                    let cv = cr.center();
                    ui.painter().line_segment(
                        [cv + vec2(-3.5, -1.5), cv + vec2(0.0, 2.0)],
                        Stroke::new(1.3_f32, t.text2),
                    );
                    ui.painter().line_segment(
                        [cv + vec2(0.0, 2.0), cv + vec2(3.5, -1.5)],
                        Stroke::new(1.3_f32, t.text2),
                    );
                }
                if !last {
                    ui.label(RichText::new("/").color(t.text3));
                }
            }
            let fav = self.data.favorites.contains(&self.loc);
            if !matches!(self.loc, Location::MyTasks | Location::Everything) {
                let star = RichText::new(if fav { "\u{2605}" } else { "\u{2606}" })
                    .size(16.0)
                    .color(if fav {
                        Color32::from_rgb(0xf8, 0xae, 0x00)
                    } else {
                        t.text2
                    });
                if ui
                    .add(ghost(egui::Button::new(star)))
                    .on_hover_text(tr("Favorite"))
                    .clicked()
                {
                    self.actions.push(Action::ToggleFavorite(self.loc));
                }
            }
            self.level_note(ui, &t, self.loc_level(self.loc));
            self.archived_note(ui, &t);
            let right_start = ui.max_rect().right();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let hb = |ui: &mut Ui, text: &str| -> Response {
                    let (kind, label) = match text {
                        t if t.contains("Share") => ("share", "Share"),
                        t if t.contains("Ask") => ("edit", "Ask"),
                        t if t.contains("Automate") => ("robot", "Automate"),
                        _ => ("phone", ""),
                    };
                    let g = ui.painter().layout_no_wrap(
                        tr(label).to_string(),
                        FontId::proportional(14.5),
                        t.text2,
                    );
                    let w = if label.is_empty() {
                        40.0
                    } else {
                        g.size().x + 32.0
                    };
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                    name_it(&resp, if label.is_empty() { "Huddle" } else { label });
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 6.0, t.hover);
                    }
                    line_icon(
                        ui.painter(),
                        kind,
                        r.left_center() + vec2(12.0, 0.0),
                        t.text2,
                    );
                    if label.is_empty() {
                        let cv = r.right_center() - vec2(9.0, 1.0);
                        ui.painter().line_segment(
                            [cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5)],
                            Stroke::new(1.2_f32, t.text2),
                        );
                        ui.painter().line_segment(
                            [cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)],
                            Stroke::new(1.2_f32, t.text2),
                        );
                    } else {
                        ui.painter().galley(
                            pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0),
                            g,
                            t.text2,
                        );
                    }
                    resp
                };
                if matches!(self.loc, Location::List(_)) {
                    // frameless like Share, Ask and Automate beside it
                    let r = super::icons::icon_button_ex(
                        ui,
                        &t,
                        "dots",
                        tr("Manage"),
                        "Manage",
                        false,
                        0.0,
                        t.text2,
                        t.text2,
                    );
                    if let Location::List(lid) = self.loc {
                        popup(ui, &r, ui.id().with("manage_pop"), 200.0, |ui| {
                            if ui.button(tr("Rename list")).clicked() {
                                self.actions.push(Action::StartRename(Target::List(lid)));
                                close_popup(ui);
                            }
                            if let Some(sid) = self.data.space_of_list(lid).map(|s| s.id) {
                                if ui.button(tr("Edit statuses")).clicked() {
                                    self.actions.push(Action::EditStatuses(sid));
                                    close_popup(ui);
                                }
                            }
                            if self.may_manage() && ui.button(tr("Custom fields")).clicked() {
                                self.modal = Some(Modal::CustomFields {
                                    name: String::new(),
                                    kind: "text".into(),
                                    opts: String::new(),
                                });
                                close_popup(ui);
                            }
                            if ui.button(tr("Export to CSV")).clicked() {
                                self.toast(self.export_csv());
                                close_popup(ui);
                            }
                            if ui
                                .button(RichText::new(tr("Delete list")).color(t.danger))
                                .clicked()
                            {
                                self.actions.push(Action::AskDelete(Target::List(lid)));
                                close_popup(ui);
                            }
                        });
                    }
                }
                if hb(ui, "\u{1F465}  Share").clicked() {
                    self.modal = Some(Modal::Share {
                        emails: String::new(),
                    });
                }
                if hb(ui, "\u{1F4DD}  Ask").clicked() {
                    self.section = Section::Ai;
                }
                if self.may_manage() && hb(ui, "\u{2699}  Automate").clicked() {
                    // new rules run where I am, as from the sidebar menus
                    // (a folder's rules run in its space)
                    let scope = match self.loc {
                        Location::List(_) | Location::Space(_) => Some(self.loc),
                        Location::Folder(f) => self
                            .data
                            .spaces
                            .iter()
                            .find(|s| s.folders.iter().any(|x| x.id == f))
                            .map(|s| Location::Space(s.id)),
                        _ => None,
                    };
                    self.modal = Some(Modal::Automations {
                        status: "complete".into(),
                        action: "comment".into(),
                        value: String::new(),
                        scope,
                    });
                }
                if hb(ui, "\u{260E}")
                    .on_hover_text(tr("Start a huddle"))
                    .clicked()
                {
                    self.open_call_dialog();
                }
                let used = right_start - ui.min_rect().left();
                ui.data_mut(|d| d.insert_temp(right_id, used));
            });
        });
    }

    /// The header's second row: Add Channel, a tab per view and "+ View".
    fn header_view_tabs(&mut self, ui: &mut Ui, t: Theme) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            // "Add Channel": a chat channel for this location (not on the
            // Personal List, as in the original, nor on the views that
            // gather tasks from everywhere, which aren't a place to talk about)
            let personal = self.on_personal_list();
            let gathered = matches!(
                self.loc,
                Location::TodayOverdue | Location::MyTasks | Location::Everything
            );
            let ac = if personal || gathered {
                ui.allocate_response(vec2(0.0, 0.0), Sense::hover())
            } else {
                // a button with an icon and text in the text colour (grey
                // text in a bare outline read as a disabled field)
                super::icons::icon_button_ex(
                    ui,
                    &t,
                    "hash",
                    tr("Add Channel"),
                    "Add Channel",
                    false,
                    0.0,
                    t.text2,
                    t.text2,
                )
            };
            if ac.clicked() {
                let name = self
                    .data
                    .location_name(self.loc)
                    .to_lowercase()
                    .replace(' ', "-");
                // (only one I may see: someone else's private channel of that name doesn't count)
                let existing = self
                    .data
                    .channels
                    .iter()
                    .find(|c| c.kind == ChannelKind::Channel && c.name == name && self.can_see(c))
                    .map(|c| c.id);
                let id = existing.unwrap_or_else(|| {
                    let id = self.data.new_id();
                    let members = self.data.members.iter().map(|m| m.id).collect();
                    self.data.channels.push(Channel {
                        id,
                        name: name.clone(),
                        kind: ChannelKind::Channel,
                        topic: trf!("Conversations about {}", self.data.location_name(self.loc)),
                        members,
                        messages: vec![],
                        last_read: now_ts(),
                        creator: self.data.me,
                        ..Default::default()
                    });
                    self.dirty = true;
                    id
                });
                self.section = Section::Chat;
                self.chat.open = Some(id);
            }
            ui.add_space(8.0);
            // Overview only covers a space or a folder
            let is_list = !matches!(self.loc, Location::Space(_) | Location::Folder(_));
            if is_list && self.view == ViewKind::Overview {
                self.view = ViewKind::List;
            }
            for v in ViewKind::ALL {
                if (v == ViewKind::Overview && is_list)
                    || (v != self.view
                        && self.data.extra.hidden_views.iter().any(|h| h == v.label()))
                {
                    continue;
                }
                if personal && !matches!(v, ViewKind::Board | ViewKind::List) {
                    continue;
                }
                let sel = self.view == v;
                let g = ui.painter().layout_no_wrap(
                    tr(v.label()).to_string(),
                    FontId::proportional(14.0),
                    if sel { t.text } else { t.text2 },
                );
                let (r, resp) =
                    ui.allocate_exact_size(vec2(g.size().x + 32.0, 34.0), Sense::click());
                name_it(&resp, &format!("View {}", v.label()));
                if resp.hovered() && !sel {
                    ui.painter()
                        .rect_filled(r.shrink2(vec2(0.0, 5.0)), 6.0, t.hover);
                }
                paint_view_icon(ui.painter(), pos2(r.left() + 13.0, r.center().y), v);
                let gc = if sel { t.text } else { t.text2 };
                ui.painter().galley(
                    pos2(r.left() + 25.0, r.center().y - g.size().y / 2.0),
                    g.clone(),
                    gc,
                );
                if sel {
                    ui.painter().galley(
                        pos2(r.left() + 25.3, r.center().y - g.size().y / 2.0),
                        g,
                        gc,
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            pos2(r.left() + 2.0, r.bottom() - 2.0),
                            pos2(r.right() - 2.0, r.bottom()),
                        ),
                        1.0,
                        t.text,
                    );
                }
                if resp.clicked() {
                    self.view = v;
                }
            }
            ui.add_space(6.0);
            let vr = ui.add(ghost(
                egui::Button::new(RichText::new(tr("+  View")).size(14.0).color(t.text2))
                    .min_size(vec2(0.0, 30.0)),
            ));
            popup(ui, &vr, ui.id().with("add_view"), 200.0, |ui| {
                ui.label(RichText::new(tr("Views")).small().color(t.text3));
                for v in ViewKind::ALL {
                    let mut on = !self.data.extra.hidden_views.iter().any(|h| h == v.label());
                    if check(ui, &mut on, tr(v.label())).changed() {
                        if on {
                            self.data.extra.hidden_views.retain(|h| h != v.label());
                        } else {
                            self.data.extra.hidden_views.push(v.label().to_string());
                        }
                        self.dirty = true;
                    }
                }
            });
        });
    }

    /// The toolbar under the view tabs: Group, Subtasks, Columns, and on the
    /// right Add Task, search, me mode, Assignee, Closed, Filter and Sort.
    fn header_toolbar(&mut self, ui: &mut Ui, t: Theme) {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(16, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    // full chip labels, and whether they all fit in one row; if
                    // not (a narrow window) the chips show only their icon
                    let grouped = matches!(self.view, ViewKind::List | ViewKind::Board);
                    let group_label = format!("Group: {}", self.group_by.label());
                    let alabel = match self.assignee_filter.and_then(|a| self.data.member(a)) {
                        Some(m) => m.name.clone(),
                        None => "Assignee".into(),
                    };
                    let nf = self.filter.count()
                        + usize::from(!self.search.is_empty())
                        + usize::from(self.me_mode);
                    let flabel = if nf > 0 {
                        format!("Filter \u{00B7} {}", nf)
                    } else {
                        "Filter".to_string()
                    };
                    let sort_arrow = if self.sort_desc && self.sort_by != SortBy::Manual {
                        " \u{2193}"
                    } else {
                        ""
                    };
                    let slabel = format!("Sort: {}{}", self.sort_by.label(), sort_arrow);
                    // the same labels in the interface language (the English ones name the chips)
                    let group_shown = trf!("Group: {}", tr(self.group_by.label()));
                    let ashown = match self.assignee_filter.and_then(|a| self.data.member(a)) {
                        Some(_) => alabel.clone(),
                        None => tr("Assignee").to_string(),
                    };
                    let fshown = if nf > 0 {
                        trf!("Filter \u{00B7} {}", nf)
                    } else {
                        tr("Filter").to_string()
                    };
                    let sshown = trf!("Sort: {}{}", tr(self.sort_by.label()), sort_arrow);
                    let closed_shown = crate::i18n::tr_ctx("Closed (tasks)", "Closed");
                    let compact = {
                        let pw = |s: &str| {
                            ui.painter()
                                .layout_no_wrap(s.to_string(), FontId::proportional(13.0), t.text)
                                .size()
                                .x
                                + 16.0
                                + 6.0
                                + 18.0
                                + 6.0
                        };
                        let mut need = pw(&ashown)
                            + pw(closed_shown)
                            + pw(&fshown)
                            + pw(&sshown)
                            + 90.0
                            + 32.0
                            + 32.0;
                        if grouped {
                            need += pw(&group_shown) + pw(tr("Subtasks"));
                        }
                        if self.view == ViewKind::List {
                            need += pw(tr("Columns"));
                        }
                        if self.show_search {
                            need += 186.0;
                        }
                        need > ui.available_width()
                    };
                    // `full` names the chip (English), `shown` is what it says
                    // compact: the icon alone (and a count where there is one)
                    let chip = |ui: &mut Ui,
                                full: &str,
                                shown: &str,
                                short: &str,
                                active: bool,
                                accent: bool,
                                icon: &str|
                     -> Response {
                        if compact {
                            let r = pill(ui, &t, full, short, active, accent, icon);
                            r.on_hover_text(shown)
                        } else {
                            pill(ui, &t, full, shown, active, accent, icon)
                        }
                    };
                    if grouped {
                        let r = chip(ui, &group_label, &group_shown, "", false, true, "list");
                        popup(ui, &r, ui.id().with("group_pop"), 160.0, |ui| {
                            for g in GroupBy::ALL {
                                if ui
                                    .selectable_label(self.group_by == g, tr(g.label()))
                                    .clicked()
                                {
                                    self.group_by = g;
                                    close_popup(ui);
                                }
                            }
                        });
                        if chip(
                            ui,
                            "Subtasks",
                            tr("Subtasks"),
                            "",
                            self.subtasks_open,
                            false,
                            "subtasks",
                        )
                        .on_hover_text(tr("Show all subtasks"))
                        .clicked()
                        {
                            self.subtasks_open = !self.subtasks_open;
                        }
                    }
                    if self.view == ViewKind::List {
                        let n = self
                            .hidden_cols
                            .iter()
                            .filter(|c| {
                                c.as_str() != "Date created"
                                    && !parity::PROPERTIES.contains(&c.as_str())
                                    // the Table's and custom field columns
                                    // are not the List's built-in columns
                                    && !c.starts_with("table:")
                                    && !c.starts_with("field:")
                            })
                            .count();
                        let extra = parity::PROPERTIES.iter().filter(|p| self.col_on(p)).count();
                        let cr = chip(
                            ui,
                            "Columns",
                            tr("Columns"),
                            "",
                            n > 0 || extra > 0 || self.fields_open,
                            false,
                            "columns",
                        );
                        if cr.clicked() {
                            self.fields_open = !self.fields_open;
                        }
                        if self.fields_open {
                            self.fields_panel(ui.ctx(), cr.rect.bottom() + 6.0);
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let b = Tint(
                            egui::Button::new(
                                RichText::new(tr("Add Task")).color(t.on_primary).size(13.0),
                            )
                            .fill(t.primary)
                            .corner_radius(6)
                            .min_size(vec2(0.0, CONTROL_H)),
                        );
                        if ui.add(b).clicked() {
                            self.open_quick_add(None);
                        }
                        // search toggle
                        let (sr, sresp) =
                            ui.allocate_exact_size(vec2(CONTROL_H, CONTROL_H), Sense::click());
                        ui.painter().rect_stroke(
                            sr,
                            6.0,
                            Stroke::new(1.0_f32, t.border),
                            egui::StrokeKind::Inside,
                        );
                        if sresp.hovered() || self.show_search {
                            ui.painter().rect_filled(sr.shrink(1.0), 6.0, t.hover);
                        }
                        line_icon(ui.painter(), "search", sr.center(), t.text2);
                        name_it(&sresp, "Search tasks");
                        if sresp.on_hover_text(tr("Search tasks")).clicked() {
                            self.show_search = !self.show_search;
                            self.search_focus = self.show_search;
                            if !self.show_search {
                                self.search.clear();
                            }
                        }
                        if self.show_search {
                            let r = ui.add(
                                field(&mut self.search)
                                    .hint_text(tr("Search tasks..."))
                                    .desired_width(if compact { 110.0 } else { 180.0 }),
                            );
                            name_field(&r, "Search tasks...");
                            if std::mem::take(&mut self.search_focus) {
                                r.request_focus();
                            }
                        }
                        // me mode avatar
                        if let Some(m) = self.data.member(self.data.me).cloned() {
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                            if self.me_mode {
                                ui.painter().circle_stroke(
                                    r.center(),
                                    12.5,
                                    Stroke::new(1.5_f32, t.text),
                                );
                            }
                            paint_avatar(ui.painter(), r.center(), 10.5, &m, t.bg);
                            if !self.me_mode {
                                hover_ring(ui, &resp, r.shrink(1.5), 11.5);
                            }
                            if resp.on_hover_text(tr("Me mode: only my tasks")).clicked() {
                                self.me_mode = !self.me_mode;
                            }
                        }
                        let ar = chip(
                            ui,
                            &alabel,
                            &ashown,
                            "",
                            self.assignee_filter.is_some(),
                            false,
                            "person",
                        );
                        let data = self.data.clone();
                        popup(ui, &ar, ui.id().with("asg_filter"), 200.0, |ui| {
                            if ui
                                .selectable_label(self.assignee_filter.is_none(), tr("Everyone"))
                                .clicked()
                            {
                                self.assignee_filter = None;
                                close_popup(ui);
                            }
                            for m in &data.members {
                                if ui
                                    .selectable_label(self.assignee_filter == Some(m.id), &m.name)
                                    .clicked()
                                {
                                    self.assignee_filter = Some(m.id);
                                    close_popup(ui);
                                }
                            }
                        });
                        if chip(
                            ui,
                            "Closed",
                            closed_shown,
                            "",
                            self.show_closed,
                            false,
                            "check",
                        )
                        .on_hover_text(tr("Show closed tasks"))
                        .clicked()
                        {
                            self.show_closed = !self.show_closed;
                        }
                        let fshort = if nf > 0 {
                            nf.to_string()
                        } else {
                            String::new()
                        };
                        let fr = chip(
                            ui,
                            &flabel,
                            &fshown,
                            &fshort,
                            nf > 0 || self.filter_open,
                            false,
                            "filter",
                        );
                        if fr.clicked() {
                            self.filter_open = !self.filter_open;
                        }
                        if self.filter_open {
                            self.filter_panel(
                                ui.ctx(),
                                fr.rect.left_bottom() + vec2(0.0, 6.0),
                                fr.rect,
                            );
                        }
                        let sr = chip(
                            ui,
                            &slabel,
                            &sshown,
                            sort_arrow.trim(),
                            self.sort_by != SortBy::Manual,
                            false,
                            "sort",
                        );
                        popup(ui, &sr, ui.id().with("sort_pop"), 160.0, |ui| {
                            for s in SortBy::ALL {
                                if ui
                                    .selectable_label(self.sort_by == s, tr(s.label()))
                                    .clicked()
                                {
                                    self.sort_by = s;
                                    close_popup(ui);
                                }
                            }
                            if self.sort_by != SortBy::Manual {
                                ui.separator();
                                ui.horizontal(|ui| {
                                    if ui
                                        .selectable_label(!self.sort_desc, tr("Ascending"))
                                        .clicked()
                                    {
                                        self.sort_desc = false;
                                    }
                                    if ui
                                        .selectable_label(self.sort_desc, tr("Descending"))
                                        .clicked()
                                    {
                                        self.sort_desc = true;
                                    }
                                });
                            }
                        });
                    });
                });
            });
    }
}
