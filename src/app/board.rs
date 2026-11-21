// The Board view: a column per group with its cards, dragging cards between
// columns, and adding a task at the bottom of a column.

use super::icons::{paint_calendar_icon, paint_outline_flag, paint_person_icon};
use super::*;
use egui::Response;

// ---------------------------------------------------------------- board view

impl App {
    pub(super) fn board_view(&mut self, ui: &mut Ui) {
        let t = self.t();
        // the cards by id: each looks its task up when it's drawn, so the
        // board isn't copied on every frame
        let groups = {
            let refs = self.filtered();
            self.groups(&refs, true)
        };
        // closed tasks are hidden by default: their columns say how many
        // instead of a plain 0
        let mut hidden_closed: HashMap<String, usize> = HashMap::new();
        if !self.show_closed {
            self.show_closed = true;
            for x in self.filtered() {
                if self.data.is_closed(x) {
                    *hidden_closed
                        .entry(self.data.status_ref(x).name.clone())
                        .or_default() += 1;
                }
            }
            self.show_closed = false;
        }
        let mut dropped: Option<(u64, usize)> = None;
        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for (gi, g) in groups.iter().enumerate() {
                        let open = g
                            .status
                            .as_ref()
                            .map(|s| s.kind == StatusKind::Open)
                            .unwrap_or(false);
                        let tint = if open || g.status.is_none() && g.color == t.text3 {
                            Color32::from_black_alpha(9)
                        } else {
                            g.color.gamma_multiply(0.07)
                        };
                        let accent = if open { t.text2 } else { g.color };
                        // a collapsed column is a narrow strip with the name running down it
                        let ckey = format!("board:{}", g.key);
                        let folded = self.collapsed.contains(&ckey);
                        let col_w = if folded { 44.0 } else { 252.0 };
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            let frame = egui::Frame::new()
                                .fill(tint)
                                .corner_radius(10)
                                .inner_margin(egui::Margin::same(8));
                            let (_, payload) = ui.dnd_drop_zone::<u64, ()>(frame, |ui| {
                                ui.set_width(col_w - 16.0);
                                if folded {
                                    let (r, resp) = ui.allocate_exact_size(
                                        vec2(
                                            col_w - 16.0,
                                            (ui.available_height() - 40.0).clamp(160.0, 420.0),
                                        ),
                                        Sense::click(),
                                    );
                                    name_it(&resp, &format!("Expand group {}", g.label));
                                    if resp.hovered() {
                                        ui.painter().rect_filled(
                                            r,
                                            6.0,
                                            Color32::from_black_alpha(8),
                                        );
                                    }
                                    let p = ui.painter();
                                    p.text(
                                        r.center_top() + vec2(0.0, 10.0),
                                        Align2::CENTER_CENTER,
                                        g.tasks.len().to_string(),
                                        FontId::proportional(12.5),
                                        accent,
                                    );
                                    let gal = p.layout_no_wrap(
                                        g.label.to_uppercase(),
                                        FontId::proportional(12.0),
                                        accent,
                                    );
                                    let at =
                                        pos2(r.center().x + gal.size().y / 2.0, r.top() + 24.0);
                                    p.add(
                                        egui::epaint::TextShape::new(at, gal, accent)
                                            .with_angle(std::f32::consts::FRAC_PI_2),
                                    );
                                    if resp.clicked() {
                                        self.collapsed.remove(&ckey);
                                    }
                                    return;
                                }
                                let hidden = g
                                    .status
                                    .as_ref()
                                    .filter(|_| g.tasks.is_empty())
                                    .and_then(|s| hidden_closed.get(&s.name))
                                    .copied()
                                    .unwrap_or(0);
                                ui.horizontal(|ui| {
                                    self.group_pill(ui, g);
                                    if hidden > 0 {
                                        ui.label(
                                            RichText::new(trf!("{} hidden", hidden)).color(t.text3),
                                        )
                                        .on_hover_text(tr("Closed tasks are hidden"));
                                    } else {
                                        ui.label(
                                            RichText::new(g.tasks.len().to_string()).color(accent),
                                        );
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let b = ui
                                            .add(ghost(egui::Button::new(
                                                RichText::new("\u{2039}").color(t.text3).size(15.0),
                                            )))
                                            .on_hover_text(tr("Collapse group"));
                                        name_it(&b, &format!("Collapse group {}", g.label));
                                        if b.clicked() {
                                            self.collapsed.insert(ckey.clone());
                                        }
                                    });
                                });
                                ui.add_space(4.0);
                                egui::ScrollArea::vertical()
                                    .id_salt(("col", gi))
                                    .auto_shrink([false, true])
                                    .max_height(ui.available_height() - 60.0)
                                    .show(ui, |ui| {
                                        for &tid in &g.tasks {
                                            let Some(name) =
                                                self.task(tid).map(|x| format!("Card {}", x.name))
                                            else {
                                                continue;
                                            };
                                            let resp = self.board_card_by_id(ui, tid, col_w - 16.0);
                                            name_it(&resp, &name);
                                            resp.context_menu(|ui| self.task_menu(ui, tid));
                                            if resp.clicked() {
                                                self.actions.push(Action::Open(tid));
                                            }
                                        }
                                        if hidden > 0 {
                                            let label = if hidden == 1 {
                                                tr("Show 1 closed task").to_string()
                                            } else {
                                                trf!("Show {} closed tasks", hidden)
                                            };
                                            let b = ui.add(ghost(egui::Button::new(
                                                RichText::new(label).color(t.text2),
                                            )));
                                            name_it(&b, &format!("Show closed in {}", g.label));
                                            if b.clicked() {
                                                self.show_closed = true;
                                            }
                                        }
                                        self.board_add_task(ui, g, col_w - 16.0, accent);
                                    });
                            });
                            if let Some(p) = payload {
                                dropped = Some((*p, gi));
                            }
                        });
                    }
                    if self.group_by == GroupBy::Status {
                        if let Some(sid) = self
                            .data
                            .lists_in(self.loc)
                            .and_then(|l| l.first().copied())
                            .and_then(|l| self.data.space_of_list(l))
                            .map(|s| s.id)
                        {
                            // level with the columns' heading row, not their top edge
                            let add = ui
                                .vertical(|ui| {
                                    ui.add_space(14.0);
                                    ui.add(ghost(egui::Button::new(
                                        RichText::new(tr("+  Add group")).color(t.text2).size(14.0),
                                    )))
                                })
                                .inner;
                            if add.on_hover_text(tr("Edit statuses")).clicked() {
                                self.actions.push(Action::EditStatuses(sid));
                            }
                        }
                    }
                });
            });
        if let Some((tid, gi)) = dropped {
            if let Some(mut task) = self.task(tid).cloned() {
                let g = &groups[gi];
                match self.group_by {
                    GroupBy::Status => {
                        if let Some(st) = &g.status {
                            let own = self.data.statuses_for_list(task.list_id);
                            if let Some(s) = own.iter().find(|s| s.name == st.name) {
                                task.status = s.name.clone();
                            }
                        }
                    }
                    GroupBy::Priority => task.priority = g.priority.unwrap_or(0),
                    GroupBy::Assignee => task.assignees = g.assignee.into_iter().collect(),
                    // the DONE column closes the task and keeps its date
                    GroupBy::DueDate if g.key == "d:done" => {
                        let own = self.data.statuses_for_list(task.list_id);
                        if let Some(s) = own
                            .iter()
                            .find(|s| s.kind == StatusKind::Done)
                            .or_else(|| own.iter().find(|s| s.kind == StatusKind::Closed))
                        {
                            task.status = s.name.clone();
                        }
                    }
                    GroupBy::DueDate => task.due = g.due,
                    GroupBy::None => {}
                    GroupBy::Tags | GroupBy::List => self.move_to_group(&mut task, g),
                }
                self.actions.push(Action::Replace(task));
            }
        }
    }

    /// "+ Add task" at the foot of a board column: a name field, as in the
    /// List (a task is only made once it has a name).
    fn board_add_task(&mut self, ui: &mut Ui, g: &Group, w: f32, accent: Color32) {
        let t = self.t();
        let key = format!("board:{}", g.key);
        let here = self.adding.as_ref().is_some_and(|(k, _)| *k == key);
        if !here {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click());
            name_it(&resp, &format!("Add Task in {}", g.label));
            if resp.hovered() {
                ui.painter()
                    .rect_filled(r, 6.0, Color32::from_black_alpha(8));
            }
            ui.painter().text(
                r.left_center() + vec2(10.0, 0.0),
                Align2::LEFT_CENTER,
                tr("+  Add task"),
                FontId::proportional(14.0),
                accent,
            );
            if resp.clicked() {
                self.adding = Some((key, String::new()));
            }
            return;
        }
        let mut text = self
            .adding
            .as_ref()
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        let (mut submit, mut cancel) = (false, false);
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(w - 18.0);
                let r = ui.add(
                    field(&mut text)
                        .hint_text(tr("Task Name"))
                        .desired_width(f32::INFINITY),
                );
                focus_once(ui, &r);
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                    r.request_focus();
                } else if r.lost_focus() && text.trim().is_empty() {
                    cancel = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel = true;
                }
                ui.horizontal(|ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        cancel = true;
                    }
                    if primary_button(ui, &t, tr("Save")).clicked() {
                        submit = true;
                    }
                });
            });
        if submit && !text.trim().is_empty() {
            // no List to put it in: the name stays for after one is made
            let added = self.add_from_group(g, text.trim().to_string(), false);
            self.adding = Some((key, if added { String::new() } else { text }));
        } else if cancel || submit {
            self.adding = None;
        } else {
            self.adding = Some((key, text));
        }
    }

    /// A board card that can be dragged to another column and clicked to
    /// open, with its assignee, date and priority boxes and "..." clickable
    /// on top. (egui's drag source covers what's inside it, so the card
    /// takes its drag and click under the boxes instead.)
    /// A Board card by task id. A card scrolled out of its column's view
    /// (its height known from when it was drawn) keeps its place and name
    /// but isn't drawn and its task isn't copied.
    fn board_card_by_id(&mut self, ui: &mut Ui, tid: u64, w: f32) -> Response {
        let id = egui::Id::new(("card", tid));
        let known: Option<f32> = ui.data(|d| d.get_temp(id.with("h")));
        if let Some(h) = known {
            let rect = Rect::from_min_size(ui.cursor().min, vec2(w, h));
            if !ui.is_rect_visible(rect) && !ui.ctx().is_being_dragged(id) {
                let resp = ui.interact(rect, id, Sense::click_and_drag());
                ui.allocate_rect(rect, Sense::hover());
                ui.add_space(6.0);
                return resp;
            }
        }
        match self.task(tid).cloned() {
            Some(task) => self.board_card(ui, &task, w),
            None => ui.allocate_response(egui::Vec2::ZERO, Sense::hover()),
        }
    }

    fn board_card(&mut self, ui: &mut Ui, task: &Task, w: f32) -> Response {
        let id = egui::Id::new(("card", task.id));
        let h_id = id.with("h");
        if ui.ctx().is_being_dragged(id) {
            egui::DragAndDrop::set_payload(ui.ctx(), task.id);
            let layer = egui::LayerId::new(egui::Order::Tooltip, id);
            let r = ui
                .scope_builder(egui::UiBuilder::new().layer_id(layer), |ui| {
                    self.card(ui, task, w, false)
                })
                .inner;
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                ui.ctx().transform_layer_shapes(
                    layer,
                    egui::emath::TSTransform::from_translation(p - r.center()),
                );
            }
            ui.add_space(6.0);
            return ui.interact(r, id, Sense::click_and_drag());
        }
        let h: f32 = ui.data(|d| d.get_temp(h_id)).unwrap_or(84.0);
        let rect = Rect::from_min_size(ui.cursor().min, vec2(w, h));
        let body = ui
            .interact(rect, id, Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab);
        let drawn = self.card(ui, task, w, true);
        if body.hovered() {
            // a darker outline says the card can be opened or picked up
            ui.painter().rect_stroke(
                drawn,
                8.0,
                Stroke::new(1.0_f32, hover_line(ui)),
                egui::StrokeKind::Inside,
            );
        }
        ui.add_space(6.0);
        if (drawn.height() - h).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(h_id, drawn.height()));
            ui.ctx().request_repaint();
        }
        body
    }

    /// Draws a card and returns where it went. `live` makes the boxes and
    /// the "..." clickable (not on the copy that follows the mouse).
    fn card(&mut self, ui: &mut Ui, task: &Task, w: f32, live: bool) -> Rect {
        let t = self.t();
        let closed = self.data.is_closed(task);
        let cid = egui::Id::new(("cardp", task.id));
        let mut edited = task.clone();
        let mut changed = false;
        let mut open = false;
        let frame = egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(11))
            .show(ui, |ui| {
                // text on a card must not take the card's click or drag
                ui.style_mut().interaction.selectable_labels = false;
                ui.set_width(w - 24.0);
                if !matches!(self.loc, Location::List(_)) {
                    ui.label(
                        RichText::new(self.data.list_name(task.list_id))
                            .size(11.5)
                            .color(t.text3),
                    );
                }
                let mut name = RichText::new(&task.name).size(14.0).color(t.text);
                if closed {
                    name = name.strikethrough().color(t.text3);
                }
                ui.add(egui::Label::new(name).wrap());
                if !task.tags.is_empty() && self.data.extra.apps.tags {
                    ui.horizontal_wrapped(|ui| {
                        for tag in &task.tags {
                            open |= chip(ui, tag, tag_color(tag)).clicked();
                        }
                    });
                }
                ui.add_space(2.0);
                let (r, _) = ui.allocate_exact_size(vec2(w - 24.0, 22.0), Sense::hover());
                let p = ui.painter().clone();
                let icon_col = Color32::from_rgb(150, 150, 150);
                let boxed = |x: f32| Rect::from_min_size(pos2(x, r.top()), vec2(22.0, 22.0));
                let mut x = r.left();
                let used = paint_avatars(
                    &p,
                    pos2(x, r.center().y),
                    10.0,
                    &task.assignees,
                    &self.data,
                    t.card,
                );
                let a_rect = if used > 0.0 {
                    let b = Rect::from_min_size(pos2(x - 2.0, r.top()), vec2(used + 4.0, 22.0));
                    x += used + 6.0;
                    b
                } else {
                    let b = boxed(x);
                    p.rect_stroke(
                        b,
                        6.0,
                        Stroke::new(1.0_f32, t.border),
                        egui::StrokeKind::Inside,
                    );
                    paint_person_icon(&p, b.center(), icon_col);
                    x += 26.0;
                    b
                };
                let d_rect = match fmt_due(task) {
                    Some(d) => {
                        let g = p.layout_no_wrap(
                            d,
                            FontId::proportional(12.5),
                            due_color(task, closed, &t),
                        );
                        let b =
                            Rect::from_min_size(pos2(x, r.top()), vec2(g.size().x + 26.0, 22.0));
                        p.rect_stroke(
                            b,
                            6.0,
                            Stroke::new(1.0_f32, t.border),
                            egui::StrokeKind::Inside,
                        );
                        paint_calendar_icon(&p, b.left_center() + vec2(11.0, 0.0), icon_col);
                        let gw = g.size().x;
                        p.galley(
                            pos2(b.left() + 20.0, b.center().y - g.size().y / 2.0),
                            g,
                            t.text2,
                        );
                        x += gw + 30.0;
                        b
                    }
                    None => {
                        let b = boxed(x);
                        p.rect_stroke(
                            b,
                            6.0,
                            Stroke::new(1.0_f32, t.border),
                            egui::StrokeKind::Inside,
                        );
                        paint_calendar_icon(&p, b.center(), icon_col);
                        x += 26.0;
                        b
                    }
                };
                let b = boxed(x);
                let show_pr = self.data.extra.apps.priorities;
                if show_pr {
                    p.rect_stroke(
                        b,
                        6.0,
                        Stroke::new(1.0_f32, t.border),
                        egui::StrokeKind::Inside,
                    );
                }
                if !show_pr {
                } else if task.priority > 0 {
                    paint_flag(
                        &p,
                        b.center() + vec2(-3.0, 0.0),
                        priority_color(task.priority, &t),
                        12.0,
                    );
                } else {
                    paint_outline_flag(&p, b.center(), icon_col);
                }
                let nsub = self.subtasks(task.id).len();
                if nsub > 0 {
                    super::icons::icon_count(
                        &p,
                        "subtasks",
                        pos2(b.right() + 8.0, r.center().y),
                        &nsub.to_string(),
                        t.text3,
                    );
                }
                if !live {
                    return;
                }
                // the same pickers as a List row's cells
                let ar = ui.interact(a_rect, cid.with("as"), Sense::click());
                hover_bg(ui, &ar, a_rect, 4.0);
                name_it(&ar, &format!("Card assignees {}", task.name));
                let data = &self.data;
                if let Some(true) = popup(ui, &ar, cid.with("asp"), 220.0, |ui| {
                    assignee_menu(ui, &t, data, &mut edited.assignees)
                }) {
                    changed = true;
                }
                let dr = ui.interact(d_rect, cid.with("du"), Sense::click());
                hover_bg(ui, &dr, d_rect, 4.0);
                name_it(&dr, &format!("Card due date {}", task.name));
                if let Some(true) = popup(ui, &dr, cid.with("dup"), DATE_MENU_W, |ui| {
                    dates_menu(ui, &t, cid.with("dcal"), &mut edited)
                }) {
                    changed = true;
                }
                if show_pr {
                    let pr = ui.interact(b, cid.with("pr"), Sense::click());
                    hover_bg(ui, &pr, b, 4.0);
                    name_it(&pr, &format!("Card priority {}", task.name));
                    if let Some(true) = popup(ui, &pr, cid.with("prp"), 180.0, |ui| {
                        priority_menu(ui, &t, &mut edited.priority)
                    }) {
                        changed = true;
                    }
                }
            });
        let rect = frame.response.rect;
        if live {
            // "..." in the corner on hover opens the task menu
            let dots =
                Rect::from_center_size(rect.right_top() + vec2(-16.0, 16.0), vec2(22.0, 22.0));
            let dr = ui.interact(dots, cid.with("dots"), Sense::click());
            name_it(&dr, &format!("Card options {}", task.name));
            if ui.rect_contains_pointer(rect) || dr.hovered() {
                ui.painter()
                    .rect_filled(dots, 5.0, if dr.hovered() { t.border } else { t.card });
                ui.painter().text(
                    dots.center(),
                    Align2::CENTER_CENTER,
                    "\u{22EF}",
                    FontId::proportional(14.0),
                    t.text2,
                );
            }
            let tid = task.id;
            click_menu(&dr, |ui| self.task_menu(ui, tid));
        }
        if changed {
            self.actions.push(Action::Replace(edited));
        } else if open {
            self.actions.push(Action::Open(task.id));
        }
        rect
    }
}
