// Building blocks of the sidebar: its rows, headings and search, and the
// tree of a Space with its Folders and Lists (dragging, archived notes).

use super::*;

impl App {
    pub(super) fn side_row(
        &self,
        ui: &mut Ui,
        indent: f32,
        selected: bool,
        lead: impl FnOnce(&egui::Painter, Pos2),
        label: &str,
        sub: Option<&str>,
        trailing: Option<String>,
        muted: bool,
    ) -> Response {
        self.side_row_as(
            ui, indent, selected, lead, label, label, sub, trailing, muted,
        )
    }

    /// A sidebar row with a fixed English name (named by it for the tests,
    /// drawn in the interface language). Names people chose use `side_row`.
    pub(super) fn side_row_tr(
        &self,
        ui: &mut Ui,
        indent: f32,
        selected: bool,
        lead: impl FnOnce(&egui::Painter, Pos2),
        label: &str,
        sub: Option<&str>,
        trailing: Option<String>,
        muted: bool,
    ) -> Response {
        self.side_row_as(
            ui,
            indent,
            selected,
            lead,
            label,
            tr(label),
            sub,
            trailing,
            muted,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn side_row_as(
        &self,
        ui: &mut Ui,
        indent: f32,
        selected: bool,
        lead: impl FnOnce(&egui::Painter, Pos2),
        label: &str,
        shown: &str,
        sub: Option<&str>,
        trailing: Option<String>,
        muted: bool,
    ) -> Response {
        let t = self.t();
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
        name_it(&resp, label);
        if selected {
            ui.painter().rect_filled(rect, 7.0, t.row_selected);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 7.0, t.hover);
        }
        let x = rect.left() + 8.0 + indent;
        lead(ui.painter(), pos2(x + 8.0, rect.center().y));
        let col = if muted && !selected { t.text2 } else { t.text };
        let clip = Rect::from_min_max(
            pos2(x + 24.0, rect.top()),
            pos2(rect.right() - 28.0, rect.bottom()),
        );
        // too long for the row: shortened with an ellipsis, whole on hover;
        // the grey note after the name keeps up to 40% of the room
        let sub_font = FontId::proportional(12.0);
        let sub_w = sub.map_or(0.0, |s| {
            let w = ui
                .painter()
                .layout_no_wrap(s.to_string(), sub_font.clone(), t.text3)
                .size()
                .x;
            (w + 4.0).min(clip.width() * 0.4)
        });
        let nr = paint_fit(
            ui,
            pos2(x + 24.0, rect.center().y),
            Align2::LEFT_CENTER,
            shown,
            FontId::proportional(14.0),
            col,
            clip.width() - sub_w,
        );
        if let Some(s) = sub {
            paint_fit(
                ui,
                pos2(nr.right() + 4.0, rect.center().y + 1.0),
                Align2::LEFT_CENTER,
                s,
                sub_font,
                t.text3,
                clip.right() - nr.right() - 4.0,
            );
        }
        if let Some(count) = trailing {
            ui.painter().text(
                rect.right_center() - vec2(12.0, 0.0),
                Align2::RIGHT_CENTER,
                count,
                FontId::proportional(12.0),
                t.text3,
            );
        }
        resp
    }

    pub(super) fn add_row(&self, ui: &mut Ui, label: &str) -> Response {
        let t = self.t();
        let tc = t.text3;
        self.side_row_tr(
            ui,
            0.0,
            false,
            |p, c| {
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    "+",
                    FontId::proportional(16.0),
                    tc,
                );
            },
            label,
            None,
            None,
            true,
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    pub(super) fn side_header(
        &self,
        ui: &mut Ui,
        title: &str,
        icons: &[(&str, &str)],
    ) -> Vec<Response> {
        let t = self.t();
        ui.add_space(10.0);
        let w = ui.available_width();
        let (r, hresp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
        ui.painter().text(
            r.left_center() + vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            tr(title),
            FontId::proportional(13.0),
            t.text2,
        );
        let hovered = hresp.hovered() || ui.rect_contains_pointer(r);
        let mut out = vec![];
        let mut x = r.right() - 13.0;
        for (i, (glyph, tip)) in icons.iter().enumerate().rev() {
            let br = Rect::from_center_size(pos2(x, r.center().y), vec2(22.0, 22.0));
            let resp = ui.interact(br, ui.id().with(("shdr", title, i)), Sense::click());
            let boxed = *glyph == "+" && i == icons.len() - 1;
            if boxed {
                ui.painter().rect_filled(
                    br,
                    5.0,
                    if resp.hovered() {
                        t.border
                    } else {
                        t.row_selected
                    },
                );
            } else if resp.hovered() {
                ui.painter().rect_filled(br, 5.0, t.row_selected);
            }
            // a line icon by its kind; the boxed "+" is drawn as lines so it
            // sits in the exact middle of its box (the "+" character sat low);
            // "⋯" stays a character
            if boxed {
                name_it(&resp, &format!("Sidebar {}", tip));
                super::super::icons::line_icon_scaled(
                    ui.painter(),
                    "plus",
                    br.center(),
                    t.text2,
                    0.75,
                );
            } else if (hovered || resp.hovered())
                && !line_icon(ui.painter(), glyph, br.center(), t.text2)
            {
                ui.painter().text(
                    br.center(),
                    Align2::CENTER_CENTER,
                    *glyph,
                    FontId::proportional(if *glyph == "+" { 15.0 } else { 13.0 }),
                    t.text2,
                );
            }
            out.push(resp.on_hover_text(tr(tip)));
            x -= 26.0;
        }
        out.reverse();
        out
    }

    pub(super) fn hidden(&self, item: &str) -> bool {
        self.data.extra.hidden_nav.iter().any(|h| h == item)
    }

    pub(super) fn matches_search(&self, label: &str) -> bool {
        match &self.side.search {
            Some(q) if !q.trim().is_empty() => {
                // built-in rows are found by the name shown (Korean) as well as the English one
                let q = q.trim().to_lowercase();
                label.to_lowercase().contains(&q) || tr(label).to_lowercase().contains(&q)
            }
            _ => true,
        }
    }

    fn side_searching(&self) -> bool {
        self.side
            .search
            .as_ref()
            .is_some_and(|q| !q.trim().is_empty())
    }

    /// While the sidebar is searched, a space stays when its name or the name
    /// of one of its folders, lists or docs matches.
    pub(super) fn space_matches_search(&self, s: &Space) -> bool {
        self.matches_search(&s.name)
            || s.folders.iter().any(|f| {
                self.matches_search(&f.name)
                    || f.lists
                        .iter()
                        .any(|l| !l.archived && self.matches_search(&l.name))
            })
            || s.lists
                .iter()
                .any(|l| !l.archived && self.matches_search(&l.name))
            || self.data.docs.iter().any(|d| {
                d.space == Some(s.id)
                    && d.parent.is_none()
                    && self.doc_shown(d)
                    && self.matches_search(&d.title)
            })
    }

    pub(super) fn space_rows(&mut self, ui: &mut Ui, s: &Space) {
        let t = self.t();
        let loc = Location::Space(s.id);
        let color = rgb(s.color);
        let letter = Data::space_glyph(s);
        // a search shows the matches inside collapsed spaces and folders too
        let searching = self.side_searching();
        let expanded = s.expanded || searching;
        let sel = self.section == Section::Home && self.loc == loc;
        let r = self.side_row(
            ui,
            0.0,
            sel,
            |p, c| {
                let rr = Rect::from_center_size(c, vec2(19.0, 19.0));
                p.rect_filled(rr, 5.0, color);
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    letter,
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
            },
            &s.name,
            None,
            None,
            false,
        );
        // add button on hover / expand toggle on the icon
        let plus =
            Rect::from_center_size(r.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
        let pr = ui.interact(plus, ui.id().with(("splus", s.id)), Sense::click());
        name_it(&pr, &format!("Create in {}", s.name));
        if r.hovered() || pr.hovered() || sel {
            ui.painter().text(
                plus.center(),
                Align2::CENTER_CENTER,
                "+",
                FontId::proportional(16.0),
                t.text2,
            );
        }
        let sid = s.id;
        popup(ui, &pr, ui.id().with(("splus_pop", s.id)), 220.0, |ui| {
            ui.label(RichText::new(tr("Create")).small().color(t.text3));
            for (icon, label) in [
                ("list", "List"),
                ("folder", "Folder"),
                ("docs", "Doc"),
                ("forms", "Form"),
                ("dashboards", "Dashboard"),
                ("whiteboards", "Whiteboard"),
                ("templates", "From a template"),
            ] {
                if menu_row_named(ui, &t, label, tr(label), false, |p, c| {
                    line_icon(p, icon, c, t.text2);
                })
                .clicked()
                {
                    match label {
                        "List" => self.open_new_list(Some(sid), None),
                        "From a template" => {
                            self.open_templates(super::templates::Category::Lists, Some(sid))
                        }
                        "Folder" => self.actions.push(Action::NewFolder(sid)),
                        "Doc" => self.new_doc(Some(sid)),
                        "Form" => self.new_form(),
                        "Dashboard" => {
                            self.modal = Some(Modal::NewDashboard {
                                name: tr("New Dashboard").into(),
                            })
                        }
                        _ => self.new_whiteboard(),
                    }
                    close_popup(ui);
                }
            }
        });
        let icon = Rect::from_center_size(r.rect.left_center() + vec2(16.0, 0.0), vec2(20.0, 20.0));
        let ir = ui.interact(icon, ui.id().with(("sicon", s.id)), Sense::click());
        if ir.hovered() {
            ui.painter().rect_filled(icon, 5.0, t.sidebar);
            paint_triangle(ui.painter(), icon.center(), expanded, t.text2);
        }
        if ir.clicked() {
            self.actions.push(Action::ToggleSpace(s.id));
        } else if r.clicked()
            && !pr.clicked()
            && !Rect::from_center_size(r.rect.right_center() - vec2(36.0, 0.0), vec2(20.0, 20.0))
                .contains(r.interact_pointer_pos().unwrap_or_default())
        {
            self.actions.push(Action::Go(loc));
            if !expanded {
                self.actions.push(Action::ToggleSpace(s.id));
            }
        }
        // "..." on hover opens the same menu as a right click
        let dots =
            Rect::from_center_size(r.rect.right_center() - vec2(36.0, 0.0), vec2(20.0, 20.0));
        let dr = ui.interact(dots, ui.id().with(("sdots", s.id)), Sense::click());
        name_it(&dr, &format!("Space options {}", s.name));
        if r.hovered() || dr.hovered() || pr.hovered() {
            if dr.hovered() {
                ui.painter().rect_filled(dots, 5.0, t.border);
            }
            ui.painter().text(
                dots.center(),
                Align2::CENTER_CENTER,
                "\u{22EF}",
                FontId::proportional(13.0),
                t.text2,
            );
        }
        click_menu(&dr, |ui| self.space_menu(ui, sid));
        r.context_menu(|ui| self.space_menu(ui, sid));
        // drag a space onto another to reorder; drop a list on it to move it here
        let r = r.interact(Sense::click_and_drag());
        if r.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), SideDrag::Space(sid));
        }
        if let Some(d) = self.side_drop(ui, r.rect) {
            match d {
                SideDrag::Space(other) => self.place_space_before(other, sid),
                SideDrag::List(l) => self.move_list(l, sid, None),
            }
        }
        if !expanded {
            return;
        }
        let tc = t.text2;
        for f in s.folders.iter().filter(|f| !f.archived) {
            if searching
                && !self.matches_search(&f.name)
                && !f
                    .lists
                    .iter()
                    .any(|l| !l.archived && self.matches_search(&l.name))
            {
                continue;
            }
            let floc = Location::Folder(f.id);
            let fexp = f.expanded || searching;
            let fsel = self.section == Section::Home && self.loc == floc;
            let fr = self.side_row(
                ui,
                14.0,
                fsel,
                |p, c| {
                    let rr = Rect::from_center_size(c + vec2(0.0, 0.5), vec2(14.0, 11.0));
                    p.rect_stroke(rr, 2.0, Stroke::new(1.2_f32, tc), egui::StrokeKind::Inside);
                    p.line_segment(
                        [
                            rr.left_top() + vec2(0.0, 2.5),
                            rr.left_top() + vec2(5.0, 2.5),
                        ],
                        Stroke::new(1.2_f32, tc),
                    );
                },
                &f.name,
                None,
                None,
                false,
            );
            let fid = f.id;
            // "..." and "+" on hover, as on a space or list row
            let dots =
                Rect::from_center_size(fr.rect.right_center() - vec2(36.0, 0.0), vec2(20.0, 20.0));
            let plus =
                Rect::from_center_size(fr.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
            let dr = ui.interact(dots, ui.id().with(("fdots", fid)), Sense::click());
            let pr = ui.interact(plus, ui.id().with(("fplus", fid)), Sense::click());
            name_it(&dr, &format!("Folder options {}", f.name));
            name_it(&pr, &format!("Create in {}", f.name));
            if fr.hovered() || dr.hovered() || pr.hovered() {
                for (rr, hov, glyph) in
                    [(dots, dr.hovered(), "\u{22EF}"), (plus, pr.hovered(), "+")]
                {
                    ui.painter()
                        .rect_filled(rr, 5.0, if hov { t.border } else { t.hover });
                    ui.painter().text(
                        rr.center(),
                        Align2::CENTER_CENTER,
                        glyph,
                        FontId::proportional(14.0),
                        t.text2,
                    );
                }
            }
            click_menu(&dr, |ui| self.folder_menu(ui, fid));
            click_menu(&pr, |ui| {
                ui.set_min_width(180.0);
                ui.label(RichText::new(tr("Create")).small().color(t.text3));
                if menu_row_named(ui, &t, "List", tr("List"), false, |p, c| {
                    line_icon(p, "list", c, t.text2);
                })
                .clicked()
                {
                    self.open_new_list(Some(sid), Some(fid));
                    ui.close_menu();
                }
                if menu_row_named(ui, &t, "Doc", tr("Doc"), false, |p, c| {
                    line_icon(p, "docs", c, t.text2);
                })
                .clicked()
                {
                    self.new_doc(Some(sid));
                    ui.close_menu();
                }
            });
            // the folder icon is the expand arrow (as on spaces); clicking the
            // rest of the row opens the folder and expands it
            let ficon =
                Rect::from_center_size(fr.rect.left_center() + vec2(30.0, 0.0), vec2(20.0, 20.0));
            let fir = ui.interact(ficon, ui.id().with(("ficon", f.id)), Sense::click());
            if fir.hovered() {
                ui.painter().rect_filled(ficon, 5.0, t.sidebar);
                paint_triangle(ui.painter(), ficon.center(), fexp, t.text2);
            }
            let on_button = fr
                .interact_pointer_pos()
                .is_some_and(|p| dots.contains(p) || plus.contains(p));
            if fir.clicked() {
                self.actions.push(Action::ToggleFolder(f.id));
            } else if fr.clicked() && !on_button {
                self.actions.push(Action::Go(floc));
                if !f.expanded {
                    self.actions.push(Action::ToggleFolder(f.id));
                }
            }
            fr.context_menu(|ui| self.folder_menu(ui, fid));
            // a list dropped on a folder moves into it
            let fr = fr.interact(Sense::click_and_drag());
            if let Some(SideDrag::List(l)) = self.side_drop(ui, fr.rect) {
                self.move_list(l, sid, Some(fid));
            }
            if fexp {
                let shown: Vec<&ListItem> = f
                    .lists
                    .iter()
                    .filter(|l| !l.archived && self.matches_search(&l.name))
                    .collect();
                let none_live = f.lists.iter().all(|l| l.archived);
                for l in shown {
                    self.list_row(ui, 30.0, l);
                }
                if !searching && none_live {
                    self.archived_lists_row(ui, 30.0, f.lists.len());
                }
            }
        }
        let shown: Vec<&ListItem> = s
            .lists
            .iter()
            .filter(|l| !l.archived && self.matches_search(&l.name))
            .collect();
        for l in shown {
            self.list_row(ui, 14.0, l);
        }
        // someone else's Private docs stay out (as in the Docs list)
        let docs: Vec<(u64, String)> = self
            .data
            .docs
            .iter()
            .filter(|d| d.space == Some(s.id) && d.parent.is_none() && self.doc_shown(d))
            .map(|d| (d.id, d.title.clone()))
            .collect();
        // everything in the Space archived: say so rather than look empty
        if !searching
            && docs.is_empty()
            && s.folders.iter().all(|f| f.archived)
            && s.lists.iter().all(|l| l.archived)
        {
            let n = s.lists.len() + s.folders.iter().map(|f| f.lists.len()).sum::<usize>();
            self.archived_lists_row(ui, 14.0, n);
        }
        for (id, title) in docs {
            if !self.matches_search(&title) {
                continue;
            }
            let sel = self.section == Section::Docs && self.doc_open == Some(id);
            let r = self.side_row(
                ui,
                14.0,
                sel,
                |p, c| {
                    let rr = Rect::from_center_size(c, vec2(12.0, 14.0));
                    p.rect_filled(rr, 2.0, Color32::from_rgb(16, 144, 224));
                    for k in 0..3 {
                        let y = rr.top() + 4.0 + k as f32 * 3.0;
                        p.line_segment(
                            [pos2(rr.left() + 3.0, y), pos2(rr.right() - 3.0, y)],
                            Stroke::new(1.0_f32, Color32::WHITE),
                        );
                    }
                },
                &title,
                None,
                None,
                false,
            );
            if r.clicked() {
                self.doc_open = Some(id);
                self.section = Section::Docs;
            }
            r.context_menu(|ui| self.doc_menu(ui, id, true));
        }
    }

    fn list_row(&mut self, ui: &mut Ui, indent: f32, l: &ListItem) {
        let t = self.t();
        let loc = Location::List(l.id);
        let n = self.count_open(l.id);
        let tc = t.text2;
        let sel = self.section == Section::Home && self.loc == loc;
        // the List's colour (Space Overview, or the list menu's Color) tints its icon
        let ic = l.color.map(rgb).unwrap_or(tc);
        let r = self.side_row(
            ui,
            indent,
            sel,
            |p, c| paint_checklist_icon(p, c, ic),
            &l.name,
            None,
            if n > 0 { Some(n.to_string()) } else { None },
            false,
        );
        let dots_hit = r
            .interact_pointer_pos()
            .map_or(false, |p| p.x > r.rect.right() - 48.0);
        if r.clicked() && !dots_hit {
            self.actions.push(Action::Go(loc));
        }
        let lid = l.id;
        let dots =
            Rect::from_center_size(r.rect.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0));
        let dr = ui.interact(dots, ui.id().with(("ldots", l.id)), Sense::click());
        name_it(&dr, &format!("List options {}", l.name));
        if r.hovered() || dr.hovered() {
            // the open-task count sits under the button; cover it
            ui.painter().rect_filled(
                dots.expand2(vec2(6.0, 0.0)),
                5.0,
                if dr.hovered() { t.border } else { t.hover },
            );
            ui.painter().text(
                dots.center(),
                Align2::CENTER_CENTER,
                "\u{22EF}",
                FontId::proportional(13.0),
                t.text2,
            );
        }
        click_menu(&dr, |ui| self.list_menu(ui, lid));
        // "+" next to it creates a task in this list
        let plus =
            Rect::from_center_size(r.rect.right_center() - vec2(36.0, 0.0), vec2(20.0, 20.0));
        let pr = ui.interact(plus, ui.id().with(("lplus", l.id)), Sense::click());
        name_it(&pr, &format!("Create in {}", l.name));
        if r.hovered() || pr.hovered() || dr.hovered() {
            ui.painter()
                .rect_filled(plus, 5.0, if pr.hovered() { t.border } else { t.hover });
            ui.painter().text(
                plus.center(),
                Align2::CENTER_CENTER,
                "+",
                FontId::proportional(15.0),
                t.text2,
            );
        }
        // the same "Create" menu as the original's list "+"
        click_menu(&pr, |ui| self.list_create_menu(ui, lid));
        r.context_menu(|ui| self.list_menu(ui, lid));
        // drag a list onto another list to put it there; drop a task card on
        // a list to move the task into it
        let r = r.interact(Sense::click_and_drag());
        if r.drag_started() {
            egui::DragAndDrop::set_payload(ui.ctx(), SideDrag::List(lid));
        }
        if let Some(SideDrag::List(other)) = self.side_drop(ui, r.rect) {
            self.place_list_before(other, lid);
        }
        let task_over = egui::DragAndDrop::has_payload_of_type::<u64>(ui.ctx())
            && ui.rect_contains_pointer(r.rect);
        if task_over {
            ui.painter().rect_stroke(
                r.rect,
                7.0,
                Stroke::new(1.5_f32, t.accent_text),
                egui::StrokeKind::Inside,
            );
            if ui.input(|i| i.pointer.any_released()) {
                if let Some(tid) = egui::DragAndDrop::take_payload::<u64>(ui.ctx()) {
                    self.move_tasks(&[*tid], lid);
                }
            }
        }
    }

    /// Under a Folder or Space that shows nothing because its Lists are
    /// archived: "N archived Lists", opening Settings > Spaces to restore them.
    fn archived_lists_row(&mut self, ui: &mut Ui, indent: f32, n: usize) {
        if n == 0 {
            return;
        }
        let text = if n == 1 {
            tr("1 archived List").to_string()
        } else {
            trf!("{} archived Lists", n)
        };
        let r = self.side_row(ui, indent, false, |_, _| {}, &text, None, None, true);
        if r.on_hover_text(tr("Restore them from Settings > Spaces"))
            .clicked()
        {
            self.open_settings("Spaces");
        }
    }

    /// Top-level tasks here hidden only because they are archived: (archived
    /// tasks the Archived filter would show, tasks in archived Lists, which
    /// come back with their List). Nothing while the Archived filter is on.
    pub(crate) fn archived_here(&mut self) -> (usize, usize) {
        if self.filter.archived {
            return (0, 0);
        }
        self.filter.archived = true;
        let own = self.filtered().len();
        self.filter.archived = false;
        if matches!(self.loc, Location::List(_)) {
            return (own, 0);
        }
        let lists = self.data.lists_in(self.loc);
        let me = self.data.me;
        let in_lists = self
            .data
            .tasks
            .iter()
            .filter(|t| t.parent.is_none() && self.data.list_archived(t.list_id))
            .filter(|t| match self.loc {
                Location::MyTasks => t.assignees.contains(&me),
                Location::TodayOverdue => {
                    t.assignees.contains(&me)
                        && t.due.is_some_and(|d| d <= today())
                        && !self.data.is_closed(t)
                }
                _ => lists.as_ref().is_none_or(|l| l.contains(&t.list_id)),
            })
            .filter(|t| {
                self.data
                    .space_of_list(t.list_id)
                    .and_then(|s| s.private_owner)
                    .is_none_or(|o| o == me)
            })
            .count();
        (own, in_lists)
    }

    /// Beside the title of an archived Space, Folder or List: says so, and
    /// restores it when it is archived itself (not only what it sits in).
    pub(super) fn archived_note(&mut self, ui: &mut Ui, t: &Theme) {
        if !self.data.location_archived(self.loc) {
            return;
        }
        let own = match self.loc {
            Location::Space(id) => self.data.spaces.iter().any(|s| s.id == id && s.archived),
            Location::Folder(id) => self
                .data
                .spaces
                .iter()
                .any(|s| s.folders.iter().any(|f| f.id == id && f.archived)),
            Location::List(id) => self.data.list_item(id).is_some_and(|l| l.archived),
            _ => false,
        };
        let hover = if own {
            "Archived: hidden from the sidebar and from other views."
        } else {
            "What this sits in is archived. Restore it from Settings > Spaces."
        };
        egui::Frame::new()
            .fill(t.hover)
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(6, 1))
            .show(ui, |ui| {
                ui.label(RichText::new(tr("Archived")).small().color(t.text2))
                    .on_hover_text(tr(hover));
            });
        if own {
            let b = ui.small_button(tr("Restore"));
            name_it(&b, "Restore archived here");
            if b.clicked() {
                match self.loc {
                    Location::Space(id) => {
                        self.set_space_archived(id, false);
                        let name = self.data.location_name(self.loc);
                        self.toast(trf!("Restored \"{}\".", name));
                    }
                    Location::Folder(id) => self.set_folder_archived(id, false),
                    Location::List(id) => self.set_list_archived(id, false),
                    _ => {}
                }
            }
        }
    }

    /// While a sidebar item is dragged over `rect`: marks the spot and, when
    /// the mouse is let go there, hands back what was dropped.
    fn side_drop(&self, ui: &Ui, rect: Rect) -> Option<SideDrag> {
        if !egui::DragAndDrop::has_payload_of_type::<SideDrag>(ui.ctx())
            || !ui.rect_contains_pointer(rect)
        {
            return None;
        }
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            Stroke::new(2.0_f32, self.t().accent_text),
        );
        if !ui.input(|i| i.pointer.any_released()) {
            return None;
        }
        egui::DragAndDrop::take_payload::<SideDrag>(ui.ctx()).map(|d| *d)
    }
}
