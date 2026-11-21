// One task row in the List view: dragging it, the name and its tags, the
// property and custom-field columns, the "..." menu and the selection box.

use super::super::icons::{
    paint_calendar_icon, paint_comment_icon, paint_outline_flag, paint_person_icon,
};
use super::*;

impl App {
    fn list_row_h(&self) -> f32 {
        if self.data.extra.labs.iter().any(|l| l == "compact") {
            30.0
        } else {
            36.0
        }
    }

    /// A List row by task id. A row scrolled out of view keeps its place,
    /// its name and its spot for the arrow keys and dragging, but isn't
    /// drawn and its task isn't copied: a long list draws only what shows.
    pub(super) fn task_row_by_id(&mut self, ui: &mut Ui, id: u64, depth: usize) {
        let row_h = self.list_row_h();
        let w = ui.available_width();
        let next = Rect::from_min_size(ui.cursor().min, vec2(w, row_h));
        let busy =
            self.row.drag == Some(id) || self.row.rename.as_ref().is_some_and(|(r, _)| *r == id);
        if !ui.is_rect_visible(next) && !busy {
            let Some(task) = self.task(id) else {
                return;
            };
            let (name, parent) = (format!("Task {}", task.name), task.parent);
            let (rect, resp) = ui.allocate_exact_size(vec2(w, row_h), Sense::click_and_drag());
            name_it(&resp, &name);
            self.row.spots.push(super::listview::RowSpot {
                id,
                parent,
                rect,
                group: self.row.groups.len().saturating_sub(1),
            });
            return;
        }
        if let Some(task) = self.task(id).cloned() {
            self.task_row(ui, &task, depth);
        }
    }

    fn task_row(&mut self, ui: &mut Ui, task: &Task, depth: usize) {
        let t = self.t();
        let w = ui.available_width();
        let row_h = self.list_row_h();
        // drag a row to put it in a new place (Sort: Manual) or group
        let (rect, resp) = ui.allocate_exact_size(vec2(w, row_h), Sense::click_and_drag());
        name_it(&resp, &format!("Task {}", task.name));
        let id = ui.id().with(("row", task.id));
        let apps = self.data.extra.apps.clone();
        self.row.spots.push(super::listview::RowSpot {
            id: task.id,
            parent: task.parent,
            rect,
            group: self.row.groups.len().saturating_sub(1),
        });
        if resp.drag_started() && self.row.rename.is_none() {
            self.row.drag = Some(task.id);
        }
        // the highlight also covers the selection box drawn just left of the row
        let band = rect.with_min_x(rect.left() - 10.0);
        let band_hover = ui.rect_contains_pointer(band) && self.row.drag.is_none();
        if self.selected.contains(&task.id) {
            ui.painter().rect_filled(band, 0.0, t.selected_bg);
        } else if resp.hovered() || band_hover || self.row.drag == Some(task.id) {
            ui.painter().rect_filled(band, 0.0, t.hover);
        }
        if self.row.focus == Some(task.id) {
            ui.painter().rect_stroke(
                band.shrink(1.0),
                3.0,
                Stroke::new(1.5_f32, t.primary),
                egui::StrokeKind::Inside,
            );
        }
        let row_hover = band_hover || Self::quick_tags_open(ui, task.id);
        let renaming = self.row.rename.as_ref().is_some_and(|(r, _)| *r == task.id);
        ui.painter().hline(
            rect.left() + 28.0..=rect.right(),
            rect.bottom() - 0.5,
            Stroke::new(1.0_f32, t.border),
        );
        let cols = self.list_cols(rect);
        let st = self.data.status_of(task);
        let closed = matches!(st.kind, StatusKind::Closed | StatusKind::Done);
        let mut x = rect.left() + 14.0 + depth as f32 * 26.0;
        let nsub = self.subtasks(task.id).len();
        if nsub > 0 {
            let ar = Rect::from_center_size(pos2(x, rect.center().y), vec2(16.0, 16.0));
            let a = ui.interact(ar, id.with("exp"), Sense::click());
            let open = self.subtasks_open || self.expanded_tasks.contains(&task.id);
            if a.hovered() || resp.hovered() || open {
                paint_triangle(
                    ui.painter(),
                    ar.center(),
                    open,
                    if a.hovered() { t.text } else { t.text3 },
                );
            }
            if a.clicked() {
                if self.expanded_tasks.contains(&task.id) {
                    self.expanded_tasks.remove(&task.id);
                } else {
                    self.expanded_tasks.insert(task.id);
                }
            }
        }
        x += 14.0;
        let cr = Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(18.0, 18.0));
        let c = ui.interact(cr, id.with("st"), Sense::click());
        name_it(&c, &format!("Status of {}", task.name));
        paint_status_circle(
            ui.painter(),
            cr.center(),
            if c.hovered() { 7.5 } else { 7.0 },
            &st,
        );
        let statuses = self.data.statuses_for_list(task.list_id);
        let mut edited = task.clone();
        let mut changed = false;
        if let Some(true) = popup(ui, &c, id.with("stp"), 200.0, |ui| {
            status_menu(ui, &t, &statuses, &mut edited.status)
        }) {
            changed = true;
        }
        x += 26.0;
        let name_col = if closed { t.text3 } else { t.text };
        // the name takes the room up to the columns, minus the counts drawn
        // after it; cut by measured width, not by a guessed character count
        // (room for the quick action buttons while hovered)
        let quick_w = if row_hover && !renaming { 72.0 } else { 0.0 };
        let after = if nsub > 0 { 34.0 } else { 0.0 }
            + if task.checklist.is_empty() { 0.0 } else { 42.0 }
            + 12.0
            + quick_w;
        // tags follow the name: a name too long for the row leaves room for
        // the first tag and a "+N" for the rest (or just "+N" when narrow)
        let tag_font = FontId::proportional(11.5);
        let tags: Vec<String> = if apps.tags {
            task.tags
                .iter()
                .map(|tag| fit_text(ui.painter(), tag, &tag_font, 90.0))
                .collect()
        } else {
            vec![]
        };
        let measure = ui.painter().clone();
        let chip_w = |s: &str| {
            measure
                .layout_no_wrap(s.to_string(), tag_font.clone(), t.text)
                .size()
                .x
                + 12.0
        };
        let plus_w = chip_w(&format!("+{}", tags.len().max(1)));
        let room = cols.left - x - after;
        let tag_room = match tags.len() {
            0 => 0.0,
            1 => chip_w(&tags[0]) + 4.0,
            _ => chip_w(&tags[0]) + 4.0 + plus_w + 4.0,
        };
        let name_w = measure
            .layout_no_wrap(task.name.clone(), FontId::proportional(14.0), t.text)
            .size()
            .x;
        let tag_room = if tags.is_empty() || name_w <= room {
            // the whole name fits: tags take what is left after it
            0.0
        } else if room - tag_room >= 240.0 {
            tag_room
        } else {
            plus_w + 4.0
        };
        let max_name = (room - tag_room).max(60.0);
        if renaming {
            let nr = Rect::from_min_max(
                pos2(x - 4.0, rect.top() + 4.0),
                pos2((cols.left - 12.0).max(x + 120.0), rect.bottom() - 4.0),
            );
            self.row_rename_field(ui, nr, task);
            x = nr.right() + 10.0;
        } else {
            let name = fit_text(
                ui.painter(),
                &task.name,
                &FontId::proportional(14.0),
                max_name,
            );
            let galley = ui
                .painter()
                .layout_no_wrap(name, FontId::proportional(14.0), name_col);
            let gw = galley.size().x;
            ui.painter().galley(
                pos2(x, rect.center().y - galley.size().y / 2.0),
                galley,
                name_col,
            );
            if closed {
                ui.painter()
                    .hline(x..=x + gw, rect.center().y, Stroke::new(1.0_f32, t.text3));
            }
            x += gw + 10.0;
        }
        if nsub > 0 {
            super::icons::icon_count(
                ui.painter(),
                "subtasks",
                pos2(x, rect.center().y),
                &nsub.to_string(),
                t.text3,
            );
            x += 34.0;
        }
        if !task.checklist.is_empty() {
            let done = task.checklist.iter().filter(|c| c.done).count();
            super::icons::icon_count(
                ui.painter(),
                "task",
                pos2(x, rect.center().y),
                &format!("{}/{}", done, task.checklist.len()),
                t.text3,
            );
            x += 42.0;
        }
        if quick_w > 0.0 {
            self.row_quick_actions(ui, rect, x, cols.left - 8.0, task);
            x += quick_w;
        }
        // as many tags as fit, then "+N" for the rest (hover lists them all)
        let tag_end = cols.left - 8.0;
        let mut shown = 0;
        for (k, (tag, full)) in tags.iter().zip(&task.tags).enumerate() {
            let rest = tags.len() - k - 1;
            let w = chip_w(tag);
            let need = if rest > 0 { w + 4.0 + plus_w } else { w };
            if x + need > tag_end {
                break;
            }
            let col = tag_color(full);
            let g = ui
                .painter()
                .layout_no_wrap(tag.clone(), tag_font.clone(), col);
            let r = Rect::from_min_size(pos2(x, rect.center().y - 9.0), vec2(w, 18.0));
            ui.painter().rect_filled(r, 9.0, col.gamma_multiply(0.16));
            ui.painter().galley(r.center() - g.size() / 2.0, g, col);
            x = r.right() + 4.0;
            shown += 1;
        }
        if shown < tags.len() && x + plus_w <= tag_end {
            let more = format!("+{}", tags.len() - shown);
            let r = Rect::from_min_size(pos2(x, rect.center().y - 9.0), vec2(plus_w, 18.0));
            ui.painter().rect_filled(r, 9.0, t.hover);
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                more,
                tag_font.clone(),
                t.text2,
            );
            if ui.rect_contains_pointer(r) {
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), id.with("tags_tip"), |ui| {
                    ui.label(task.tags.join(", "))
                });
            }
        }
        let icon_col = Color32::from_rgb(160, 160, 160);
        // assignee
        let ar = ui.interact(cols.assignee, id.with("as"), Sense::click());
        let used = paint_avatars(
            &ui.painter_at(cols.assignee),
            cols.assignee.left_center(),
            11.0,
            &task.assignees,
            &self.data,
            if resp.hovered() { t.hover } else { t.bg },
        );
        if used == 0.0 {
            paint_person_icon(
                &ui.painter_at(cols.assignee),
                cols.assignee.left_center() + vec2(12.0, 0.0),
                icon_col,
            );
        }
        let data = &self.data;
        if let Some(true) = popup(ui, &ar, id.with("asp"), 220.0, |ui| {
            assignee_menu(ui, &t, data, &mut edited.assignees)
        }) {
            changed = true;
        }
        // due date
        let dr = ui.interact(cols.due, id.with("du"), Sense::click());
        hover_bg(ui, &dr, cols.due.expand2(vec2(4.0, -6.0)), 4.0);
        name_it(&dr, &format!("Due date of {}", task.name));
        match fmt_due(task) {
            Some(d) => {
                // a date with a time is long for the column: a size smaller
                let col = due_color(task, closed, &t);
                let mut g = ui
                    .painter()
                    .layout_no_wrap(d.clone(), FontId::proportional(13.5), col);
                if g.size().x > cols.due.width() - 4.0 {
                    g = ui
                        .painter()
                        .layout_no_wrap(d, FontId::proportional(11.5), col);
                }
                ui.painter_at(cols.due).galley(
                    pos2(cols.due.left(), cols.due.center().y - g.size().y / 2.0),
                    g,
                    col,
                );
            }
            None => paint_calendar_icon(
                &ui.painter_at(cols.due),
                cols.due.left_center() + vec2(10.0, 0.0),
                icon_col,
            ),
        }
        if task.recur.is_some() && cols.due.width() > 0.0 {
            super::icons::line_icon_scaled(
                &ui.painter_at(cols.due),
                "repeat",
                cols.due.right_center() - vec2(14.0, 0.0),
                t.text3,
                0.7,
            );
        }
        if let Some(true) = popup(ui, &dr, id.with("dup"), DATE_MENU_W, |ui| {
            dates_menu(ui, &t, id.with("dcal"), &mut edited)
        }) {
            changed = true;
        }
        // priority
        let pr = ui.interact(cols.priority, id.with("pr"), Sense::click());
        hover_bg(ui, &pr, cols.priority.expand2(vec2(4.0, -6.0)), 4.0);
        if !apps.priorities {
            // Priorities are turned off in Apps
        } else if task.priority > 0 {
            paint_flag(
                &ui.painter_at(cols.priority),
                cols.priority.left_center() + vec2(4.0, 0.0),
                priority_color(task.priority, &t),
                13.0,
            );
            ui.painter_at(cols.priority).text(
                cols.priority.left_center() + vec2(18.0, 0.0),
                Align2::LEFT_CENTER,
                tr(PRIORITIES[task.priority as usize].label),
                FontId::proportional(13.5),
                t.text,
            );
        } else {
            paint_outline_flag(
                &ui.painter_at(cols.priority),
                cols.priority.left_center() + vec2(8.0, 0.0),
                icon_col,
            );
        }
        if apps.priorities {
            if let Some(true) = popup(ui, &pr, id.with("prp"), 180.0, |ui| {
                priority_menu(ui, &t, &mut edited.priority)
            }) {
                changed = true;
            }
        }
        // status pill column
        let sr = ui.interact(cols.status, id.with("stcol"), Sense::click());
        hover_bg(ui, &sr, cols.status.expand2(vec2(4.0, -6.0)), 4.0);
        if cols.status.width() > 0.0 {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(cols.status)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            status_pill(&mut child, &st, Sense::hover());
        }
        if cols.created.width() > 0.0 {
            let created = local_date(task.created);
            ui.painter_at(cols.created).text(
                cols.created.left_center(),
                Align2::LEFT_CENTER,
                fmt_date(created),
                FontId::proportional(13.0),
                t.text2,
            );
        }
        for (name, r) in &cols.extra {
            let txt = self.prop_text(task, name);
            if txt.is_empty() {
                continue;
            }
            let p = ui.painter_at(r.shrink2(vec2(0.0, 2.0)));
            if *name == "Tags" {
                let mut x = r.left();
                for tag in &task.tags {
                    let g =
                        p.layout_no_wrap(tag.clone(), FontId::proportional(11.0), tag_color(tag));
                    let chip = Rect::from_min_size(
                        pos2(x, r.center().y - 9.0),
                        vec2(g.size().x + 12.0, 18.0),
                    );
                    p.rect_filled(chip, 4.0, tag_color(tag).gamma_multiply(0.16));
                    p.galley(chip.center() - g.size() / 2.0, g, tag_color(tag));
                    x = chip.right() + 4.0;
                }
            } else {
                let short = if *name == "Latest comment" {
                    truncate_chars(&txt, 26)
                } else {
                    txt
                };
                p.text(
                    r.left_center(),
                    Align2::LEFT_CENTER,
                    short,
                    FontId::proportional(13.0),
                    t.text2,
                );
            }
        }
        if let Some(true) = popup(ui, &sr, id.with("stcolp"), 200.0, |ui| {
            status_menu(ui, &t, &statuses, &mut edited.status)
        }) {
            changed = true;
        }
        // comments
        let comments = task.activity.iter().filter(|a| a.comment).count();
        paint_comment_icon(
            &ui.painter_at(cols.comments),
            cols.comments.left_center() + vec2(10.0, 0.0),
            icon_col,
        );
        if comments > 0 {
            ui.painter_at(cols.comments).text(
                cols.comments.left_center() + vec2(22.0, 0.0),
                Align2::LEFT_CENTER,
                comments.to_string(),
                FontId::proportional(12.5),
                t.text2,
            );
        }
        // custom field cells, edited in place by the field's kind
        for (fid, r) in &cols.fields {
            if self.field_cell(ui, *r, id.with(("cf", *fid)), task, *fid, &mut edited) {
                changed = true;
            }
        }
        let tid = task.id;
        // "..." at the end of the row, and the selection box at the start
        let dots =
            Rect::from_center_size(pos2(rect.right() - 18.0, rect.center().y), vec2(22.0, 22.0));
        let dr = ui.interact(dots, id.with("dots"), Sense::click());
        name_it(&dr, &format!("Task options {}", task.name));
        if resp.hovered() || dr.hovered() {
            if dr.hovered() {
                ui.painter().rect_filled(dots, 5.0, t.border);
            }
            ui.painter().text(
                dots.center(),
                Align2::CENTER_CENTER,
                "\u{22EF}",
                FontId::proportional(14.0),
                t.text2,
            );
        }
        click_menu(&dr, |ui| self.task_menu(ui, tid));
        let ticked = self.row_checkbox(ui, rect, tid, resp.hovered() || band_hover);
        resp.context_menu(|ui| self.task_menu(ui, tid));
        if resp.clicked() && ui.input(|i| i.modifiers.command || i.modifiers.shift) {
            // Cmd click toggles the row; Shift click selects the range from
            // the last clicked row
            let shift = ui.input(|i| i.modifiers.shift);
            self.select_click(tid, shift);
            return;
        }
        if ticked {
            return;
        }
        if changed {
            self.actions.push(Action::Replace(edited));
        } else if resp.clicked() {
            self.actions.push(Action::Open(task.id));
        }
    }
}
