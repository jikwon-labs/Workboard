// List view helpers: view settings kept per location, dragging rows into a
// manual order, range selection and the selection keys, the group header
// menu, and deleting with an Undo.

use super::*;
use parity::{FilterGroup, FilterRow, Join};

/// The view settings remembered for one location (List, Folder, Space...).
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ViewPrefs {
    pub group_by: GroupBy,
    pub sort_by: SortBy,
    #[serde(default)]
    pub sort_desc: bool,
    #[serde(default)]
    pub show_closed: bool,
    #[serde(default)]
    pub subtasks_open: bool,
    #[serde(default)]
    pub hidden_cols: Vec<String>,
    #[serde(default)]
    pub filter_rows: Vec<FilterRow>,
    #[serde(default, skip_serializing_if = "Join::is_and")]
    pub filter_join: Join,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter_groups: Vec<FilterGroup>,
    #[serde(default)]
    pub archived: bool,
}

/// A row drawn this frame, for dropping a dragged row and for range
/// selection and arrow keys (they use the order rows were drawn in).
pub(super) struct RowSpot {
    pub id: u64,
    pub parent: Option<u64>,
    pub rect: Rect,
    /// Index into `App::row_groups`.
    pub group: usize,
}

fn prefs_path() -> std::path::PathBuf {
    Data::dir().join("view-settings.json")
}

/// View settings saved on this computer (they are personal, so they stay
/// out of the shared workspace file).
pub(super) fn load_prefs() -> HashMap<String, ViewPrefs> {
    std::fs::read_to_string(prefs_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

impl App {
    // ---------------------------------------------------------------- view settings

    fn prefs_key(&self) -> String {
        format!("{}|{:?}", self.ws_file, self.loc)
    }

    pub(super) fn current_prefs(&self) -> ViewPrefs {
        let mut hidden: Vec<String> = self.hidden_cols.iter().cloned().collect();
        hidden.sort();
        ViewPrefs {
            group_by: self.group_by,
            sort_by: self.sort_by,
            sort_desc: self.sort_desc,
            show_closed: self.show_closed,
            subtasks_open: self.subtasks_open,
            hidden_cols: hidden,
            filter_rows: self.filter.rows.clone(),
            filter_join: self.filter.join,
            filter_groups: self.filter.groups.clone(),
            archived: self.filter.archived,
        }
    }

    fn apply_prefs(&mut self, p: &ViewPrefs) {
        self.group_by = p.group_by;
        self.sort_by = p.sort_by;
        self.sort_desc = p.sort_desc;
        self.show_closed = p.show_closed;
        self.subtasks_open = p.subtasks_open;
        self.hidden_cols = p.hidden_cols.iter().cloned().collect();
        self.filter.rows = p.filter_rows.clone();
        self.filter.join = p.filter_join;
        self.filter.groups = p.filter_groups.clone();
        self.filter.archived = p.archived;
    }

    /// Called once a frame: entering a location brings back the settings
    /// last used there (a location never changed keeps what was showing),
    /// and a change made here is remembered for it.
    pub(super) fn sync_view_prefs(&mut self) {
        // the Personal List sets its own defaults (personal_list_defaults)
        if self.section != Section::Home || self.on_personal_list() {
            self.view_prefs_at = None;
            return;
        }
        let key = self.prefs_key();
        if self.view_prefs_at.as_ref().map(|(k, _)| k) != Some(&key) {
            if let Some(p) = self.view_prefs.get(&key).cloned() {
                self.apply_prefs(&p);
            }
            self.view_prefs_at = Some((key, self.current_prefs()));
            return;
        }
        let cur = self.current_prefs();
        if self
            .view_prefs_at
            .as_ref()
            .is_some_and(|(_, seen)| *seen != cur)
        {
            self.view_prefs.insert(key.clone(), cur.clone());
            self.view_prefs_at = Some((key, cur));
            if self.persist {
                if let Ok(s) = serde_json::to_string_pretty(&self.view_prefs) {
                    let _ = std::fs::create_dir_all(Data::dir());
                    let _ = std::fs::write(prefs_path(), s);
                }
            }
        }
    }

    // ---------------------------------------------------------------- manual order

    /// Puts `dragged` just before or after `target` among tasks with the same
    /// parent, by handing the moved block the `order` values it already
    /// used (tasks elsewhere keep their place relative to them).
    pub(super) fn reorder_task(&mut self, dragged: u64, target: u64, after: bool) {
        let (Some(d), Some(tg)) = (self.task(dragged), self.task(target)) else {
            return;
        };
        // only same-parent moves: a subtask stays under its parent
        if dragged == target || d.parent != tg.parent {
            return;
        }
        let parent = tg.parent;
        // top-level rows: the tasks of the lists shown here
        let lists = if parent.is_none() {
            self.data.lists_in(self.loc)
        } else {
            None
        };
        let mut seq: Vec<(u64, i64)> = self
            .data
            .tasks
            .iter()
            .filter(|t| {
                t.parent == parent
                    && lists
                        .as_ref()
                        .map_or(true, |l| l.contains(&t.list_id) || t.id == dragged)
            })
            .map(|t| (t.id, t.order))
            .collect();
        seq.sort_by_key(|x| x.1);
        let mut slots: Vec<i64> = seq.iter().map(|x| x.1).collect();
        let mut ids: Vec<u64> = seq
            .iter()
            .map(|x| x.0)
            .filter(|id| *id != dragged)
            .collect();
        let Some(ti) = ids.iter().position(|id| *id == target) else {
            return;
        };
        ids.insert(if after { ti + 1 } else { ti }, dragged);
        // keep the values strictly increasing so the new order is unambiguous
        for i in 1..slots.len() {
            if slots[i] <= slots[i - 1] {
                slots[i] = slots[i - 1] + 1;
            }
        }
        let now = now_ts();
        for (id, ord) in ids.iter().zip(slots) {
            if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == *id) {
                if t.order != ord {
                    t.order = ord;
                    t.updated = now;
                    self.dirty = true;
                }
            }
        }
    }

    /// Drops a dragged row on the row under the pointer: top-level rows
    /// change order (and take the group's value when dropped into another
    /// group); subtasks move only within their parent.
    pub(super) fn drop_row(&mut self, dragged: u64, spot: usize, after: bool) {
        let Some(s) = self.row.spots.get(spot) else {
            return;
        };
        let (target, target_parent, gi) = (s.id, s.parent, s.group);
        let Some(parent) = self.task(dragged).map(|t| t.parent) else {
            return;
        };
        if parent != target_parent {
            return;
        }
        let manual = self.sort_by == SortBy::Manual;
        if manual {
            self.reorder_task(dragged, target, after);
        }
        let group = self.row.groups.get(gi).cloned();
        let other_group =
            parent.is_none() && group.as_ref().is_some_and(|g| !g.tasks.contains(&dragged));
        if let (true, Some(g)) = (other_group, group) {
            // cloned after the reorder so the new order goes along
            if let Some(mut e) = self.task(dragged).cloned() {
                self.move_to_group(&mut e, &g);
                self.actions.push(Action::Replace(e));
            }
        } else if !manual {
            self.toast(tr("Sort by Manual to drag tasks into your own order"));
        }
    }

    /// While a row is dragged: the drop line and the moving name; on release,
    /// the drop. Call after all rows are drawn, inside the scroll area.
    pub(super) fn finish_row_drag(&mut self, ui: &mut Ui) {
        let Some(d) = self.row.drag else { return };
        let t = self.t();
        let pos = ui.ctx().pointer_latest_pos();
        let released = ui.input(|i| !i.pointer.primary_down());
        let hit = pos.and_then(|p| {
            let i = self
                .row
                .spots
                .iter()
                .position(|s| p.y >= s.rect.top() && p.y < s.rect.bottom())?;
            Some((i, p.y > self.row.spots[i].rect.center().y))
        });
        if let (Some((i, after)), Some(p)) = (hit, pos) {
            let r = self.row.spots[i].rect;
            let y = if after { r.bottom() } else { r.top() };
            let x0 = r.left()
                + 28.0
                + if self.row.spots[i].parent.is_some() {
                    26.0
                } else {
                    0.0
                };
            ui.painter()
                .hline(x0..=r.right(), y, Stroke::new(2.0_f32, t.primary));
            let name = self.task(d).map(|x| x.name.clone()).unwrap_or_default();
            let lp = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("row_drag_ghost"),
            ));
            let g = lp.layout_no_wrap(truncate(&name, 40), FontId::proportional(13.5), t.text);
            let br = Rect::from_min_size(p + vec2(12.0, -14.0), g.size() + vec2(20.0, 12.0));
            lp.rect_filled(br, 6.0, t.card);
            lp.rect_stroke(
                br,
                6.0,
                Stroke::new(1.0_f32, t.border),
                egui::StrokeKind::Inside,
            );
            lp.galley(br.min + vec2(10.0, 6.0), g, t.text);
        }
        if released {
            self.row.drag = None;
            if let Some((i, after)) = hit {
                if self.row.spots[i].id != d {
                    self.drop_row(d, i, after);
                }
            }
        } else {
            ui.ctx().request_repaint();
        }
    }

    // ---------------------------------------------------------------- selection

    /// A click on a row with Cmd (toggle) or Shift (everything from the last
    /// clicked row to this one).
    pub(super) fn select_click(&mut self, tid: u64, shift: bool) {
        if shift {
            if let Some(anchor) = self.select_anchor {
                let a = self.row.order.iter().position(|x| *x == anchor);
                let b = self.row.order.iter().position(|x| *x == tid);
                if let (Some(a), Some(b)) = (a, b) {
                    let (lo, hi) = (a.min(b), a.max(b));
                    for id in &self.row.order[lo..=hi] {
                        self.selected.insert(*id);
                    }
                    return;
                }
            }
        }
        if !self.selected.remove(&tid) {
            self.selected.insert(tid);
        }
        self.select_anchor = Some(tid);
    }

    /// Keys in the List view while no text field has focus: Cmd+A selects
    /// every row shown, Delete / Backspace deletes the selection (after
    /// asking), Up / Down move a highlight and Enter opens it.
    pub(super) fn list_keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.memory(|m| m.focused().is_some()) || ctx.wants_keyboard_input();
        if typing
            || self.modal.is_some()
            || self.quick_add.is_some()
            || self.palette.is_some()
            || self.rename.is_some()
            || self.open_task.is_some()
            || self.row.rename.is_some()
            || self.confirm_bulk_delete
            || self.menu_was_open
        {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::A)) {
            self.selected.extend(self.row.order.iter().copied());
            return;
        }
        let plain = ctx.input(|i| !i.modifiers.command && !i.modifiers.alt && !i.modifiers.ctrl);
        if !plain {
            return;
        }
        if !self.selected.is_empty()
            && ctx
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            self.confirm_bulk_delete = true;
            return;
        }
        let down = ctx.input(|i| i.key_pressed(egui::Key::ArrowDown));
        let up = ctx.input(|i| i.key_pressed(egui::Key::ArrowUp));
        if (down || up) && !self.row.order.is_empty() {
            let at = self
                .row
                .focus
                .and_then(|f| self.row.order.iter().position(|x| *x == f));
            let next = match at {
                None => {
                    if down {
                        0
                    } else {
                        self.row.order.len() - 1
                    }
                }
                Some(i) if down => (i + 1).min(self.row.order.len() - 1),
                Some(i) => i.saturating_sub(1),
            };
            self.row.focus = Some(self.row.order[next]);
        }
        if let Some(f) = self.row.focus.filter(|f| self.row.order.contains(f)) {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.actions.push(Action::Open(f));
            }
        }
    }

    // ---------------------------------------------------------------- delete and undo

    /// Asks before deleting the selected tasks (bulk bar Delete, or the
    /// Delete key with rows selected).
    pub(super) fn bulk_delete_confirm(&mut self, ctx: &egui::Context) {
        if !self.confirm_bulk_delete {
            return;
        }
        if self.selected.is_empty() {
            self.confirm_bulk_delete = false;
            return;
        }
        let t = self.t();
        let n = self.selected.len();
        let mut close = false;
        egui::Window::new("Delete tasks").title_bar(false).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.label(RichText::new(if n == 1 { tr("Delete 1 task?").to_string() } else { trf!("Delete {} tasks?", n) }).strong().size(16.0));
            ui.add_space(4.0);
            // only mention subtasks when some of these tasks have them
            let subs = self.data.tasks.iter().any(|x| x.parent.is_some_and(|p| self.selected.contains(&p)));
            let note = match (subs, n) {
                (true, _) => tr("Their subtasks go too. You can undo right after, or restore them from Trash."),
                (false, 1) => tr("You can undo right after, or restore it from Trash."),
                (false, _) => tr("You can undo right after, or restore them from Trash."),
            };
            ui.label(RichText::new(note).color(t.text2));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
                let del = ui.add(Tint(egui::Button::new(RichText::new(tr("Delete")).color(Color32::WHITE)).fill(t.danger)));
                name_it(&del, "Confirm delete");
                if del.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    for id in self.selected.drain() {
                        self.actions.push(Action::DeleteTask(id));
                    }
                    close = true;
                }
            });
        });
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.confirm_bulk_delete = false;
        }
    }

    /// "Deleted - Undo" at the bottom after tasks were deleted.
    pub(super) fn undo_bar(&mut self, ctx: &egui::Context) {
        let Some((trash, ts)) = self.undo_trash.clone() else {
            return;
        };
        if now_ts() - ts > 8 || trash.is_empty() {
            self.undo_trash = None;
            return;
        }
        let n = trash.len();
        let mut undo = false;
        egui::Area::new(egui::Id::new("undo_bar"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, [0.0, -24.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(32, 32, 32))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(16, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(if n == 1 {
                                    tr("Task deleted").to_string()
                                } else {
                                    trf!("{} tasks deleted", n)
                                })
                                .color(Color32::WHITE),
                            );
                            ui.add_space(8.0);
                            let b = ui.add(ghost(egui::Button::new(
                                RichText::new(tr("Undo"))
                                    .color(Color32::from_rgb(140, 170, 255))
                                    .strong(),
                            )));
                            name_it(&b, "Undo delete");
                            if b.clicked() {
                                undo = true;
                            }
                        });
                    });
            });
        if undo {
            for tid in trash.iter().rev() {
                if let Some(i) = self.data.extra.trash.iter().position(|x| x.id == *tid) {
                    self.restore_trash(i);
                }
            }
            self.undo_trash = None;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    // ---------------------------------------------------------------- group header

    /// The group header's menu ("..." or right click).
    pub(super) fn group_menu(&mut self, ui: &mut Ui, g: &Group) {
        ui.set_min_width(200.0);
        let collapsed = self.collapsed.contains(&g.key);
        if ui
            .button(tr(if collapsed {
                "Expand group"
            } else {
                "Collapse group"
            }))
            .clicked()
        {
            if collapsed {
                self.collapsed.remove(&g.key);
            } else {
                self.collapsed.insert(g.key.clone());
            }
            parity::close_any(ui);
        }
        if ui.button(tr("Collapse all groups")).clicked() {
            let tasks = self.filtered();
            let keys: Vec<String> = self
                .groups(&tasks, false)
                .into_iter()
                .map(|g| g.key)
                .collect();
            self.collapsed.extend(keys);
            parity::close_any(ui);
        }
        if ui.button(tr("Expand all groups")).clicked() {
            self.collapsed.clear();
            parity::close_any(ui);
        }
        ui.separator();
        if ui.button(tr("Select all in group")).clicked() {
            self.selected.extend(g.tasks.iter().copied());
            parity::close_any(ui);
        }
        if ui.button(tr("+ Add task")).clicked() {
            self.adding = Some((g.key.clone(), String::new()));
            self.collapsed.remove(&g.key);
            parity::close_any(ui);
        }
    }

    // ---------------------------------------------------------------- row quick actions

    /// Small buttons drawn after a row's name while it is hovered: add a
    /// subtask, rename, add a tag.
    pub(super) fn row_quick_actions(
        &mut self,
        ui: &mut Ui,
        row: Rect,
        x: f32,
        limit: f32,
        task: &Task,
    ) {
        let t = self.t();
        let id = Self::quick_id(ui, task.id);
        let mut x = x;
        let items: [(&str, &str); 3] = [
            ("+", "Add subtask"),
            ("\u{270E}", "Rename"),
            ("#", "Add tag"),
        ];
        for (glyph, what) in items {
            let r = Rect::from_min_size(pos2(x, row.center().y - 10.0), vec2(20.0, 20.0));
            if r.right() > limit {
                break;
            }
            let resp = ui.interact(r, id.with(what), Sense::click());
            name_it(&resp, &format!("{} {}", what, task.name));
            ui.painter().rect_stroke(
                r,
                4.0,
                Stroke::new(1.0_f32, if resp.hovered() { t.text3 } else { t.border }),
                egui::StrokeKind::Inside,
            );
            if resp.hovered() {
                ui.painter().rect_filled(r.shrink(1.0), 4.0, t.hover);
            }
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                glyph,
                FontId::proportional(12.0),
                t.text2,
            );
            let resp = resp.on_hover_text(tr(what));
            match what {
                "Add subtask" => {
                    if resp.clicked() {
                        self.expanded_tasks.insert(task.id);
                        self.sub_adding = Some((task.id, String::new()));
                    }
                }
                "Rename" => {
                    if resp.clicked() {
                        self.row.rename = Some((task.id, task.name.clone()));
                    }
                }
                _ => {
                    let mut tags = task.tags.clone();
                    if let Some(true) = popup(ui, &resp, id.with("tagp"), 200.0, |ui| {
                        tag_menu(ui, &t, &self.all_tags(), &mut tags)
                    }) {
                        let mut e = task.clone();
                        e.tags = tags;
                        self.actions.push(Action::Replace(e));
                    }
                }
            }
            x = r.right() + 4.0;
        }
    }

    fn quick_id(ui: &Ui, tid: u64) -> egui::Id {
        ui.id().with(("rowq", tid))
    }

    /// The row's tag menu is open (its buttons stay while the pointer is in it).
    pub(super) fn quick_tags_open(ui: &Ui, tid: u64) -> bool {
        ui.memory(|m| m.is_popup_open(Self::quick_id(ui, tid).with("tagp")))
    }

    pub(super) fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .data
            .tasks
            .iter()
            .flat_map(|t| t.tags.iter().cloned())
            .collect();
        tags.sort_by_key(|g| g.to_lowercase());
        tags.dedup();
        tags
    }

    /// The name field shown in place of a row's name while renaming it.
    pub(super) fn row_rename_field(&mut self, ui: &mut Ui, r: Rect, task: &Task) {
        let Some((_, text)) = self.row.rename.as_mut() else {
            return;
        };
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(r)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let resp = child.add(
            field(text)
                .desired_width(r.width())
                .font(FontId::proportional(14.0)),
        );
        name_field(&resp, "Rename task");
        focus_once(&child, &resp);
        let enter = child.input(|i| i.key_pressed(egui::Key::Enter));
        let esc = child.input(|i| i.key_pressed(egui::Key::Escape));
        if esc {
            self.row.rename = None;
        } else if resp.lost_focus() || enter {
            let name = text.trim().to_string();
            if !name.is_empty() && name != task.name {
                let mut e = task.clone();
                e.name = name;
                self.actions.push(Action::Replace(e));
            }
            self.row.rename = None;
        }
    }

    /// The inline "new subtask" field under a parent's subtasks.
    pub(super) fn sub_add_row(&mut self, ui: &mut Ui, parent: &Task) {
        let t = self.t();
        let Some((_, text)) = self.sub_adding.as_mut() else {
            return;
        };
        let mut submit = false;
        let mut cancel = false;
        let w = ui.available_width();
        ui.horizontal(|ui| {
            ui.add_space(14.0 + 26.0 + 40.0);
            let r = ui.add(
                field(text)
                    .hint_text(tr("Subtask name"))
                    .desired_width((w - 300.0).max(200.0)),
            );
            name_field(&r, "Subtask name");
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
            if ui.button(tr("Cancel")).clicked() {
                cancel = true;
            }
            if primary_button(ui, &t, tr("Save")).clicked() {
                submit = true;
            }
        });
        let name = text.trim().to_string();
        if submit && !name.is_empty() {
            self.actions.push(Action::Add {
                list: parent.list_id,
                name,
                status: None,
                priority: 0,
                assignees: vec![],
                due: None,
                parent: Some(parent.id),
                open: false,
            });
            self.sub_adding = Some((parent.id, String::new()));
        } else if cancel || submit {
            self.sub_adding = None;
        }
    }
}

/// Tag picker: existing tags to toggle, and a field for a new one.
/// Returns true when the tags changed.
pub(super) fn tag_menu(ui: &mut Ui, t: &Theme, all: &[String], tags: &mut Vec<String>) -> bool {
    let mut changed = false;
    let id = ui.id().with("new_tag");
    let mut text: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let r = ui.add(
        field(&mut text)
            .hint_text(tr("Search or add tags..."))
            .desired_width(f32::INFINITY),
    );
    name_field(&r, "Tag name");
    focus_once(ui, &r);
    let q = text.trim().to_lowercase();
    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !q.is_empty() {
        let tag = text.trim().to_string();
        if !tags.iter().any(|x| x.eq_ignore_ascii_case(&tag)) {
            tags.push(tag);
            changed = true;
        }
        text.clear();
    }
    for tag in all
        .iter()
        .filter(|g| q.is_empty() || g.to_lowercase().contains(&q))
    {
        let on = tags.contains(tag);
        let col = tag_color(tag);
        if menu_row(ui, t, tag, on, |p, c| {
            p.circle_filled(c, 4.0, col);
        })
        .clicked()
        {
            if on {
                tags.retain(|x| x != tag);
            } else {
                tags.push(tag.clone());
            }
            changed = true;
        }
    }
    ui.data_mut(|d| d.insert_temp(id, text));
    changed
}
