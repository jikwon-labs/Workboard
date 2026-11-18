// Features that bring the app closer to the original. Here: the column and
// property names, recurring tasks, recent items and single-key shortcuts.
// In parity/: create dialogs (dialogs.rs), moving / duplicating / archiving
// (entity_ops.rs), the task, List, Folder and Space menus (context_menus.rs),
// condition filters (filter_model.rs), the Columns and Filter panels
// (filter_panel.rs) and selecting several tasks at once (bulk.rs).

use super::*;
use egui::Pos2;

mod bulk;
mod context_menus;
mod dialogs;
mod entity_ops;
mod filter_model;
mod filter_panel;

pub use filter_model::*;

/// Space icons offered in Color & Icon ("" = the first letter of the name).
pub const SPACE_ICONS: [&str; 12] = [
    "", "\u{2605}", "\u{25C6}", "\u{25CF}", "\u{25B2}", "\u{2665}", "\u{2691}", "\u{2714}",
    "\u{2600}", "\u{2699}", "\u{263A}", "\u{25A0}",
];

pub const COLUMNS: [&str; 6] = [
    "Assignee",
    "Due date",
    "Priority",
    "Status",
    "Comments",
    "Date created",
];
/// Properties that can be shown as extra columns (hidden at first), drawn
/// from values every task already has.
pub const PROPERTIES: [&str; 10] = [
    "Created by",
    "Date closed",
    "Date done",
    "Date updated",
    "Start date",
    "Time estimate",
    "Time tracked",
    "Tags",
    "Latest comment",
    "Subtasks",
];

/// Width of an extra property column.
pub fn prop_width(name: &str) -> f32 {
    match name {
        "Latest comment" => 180.0,
        "Tags" => 140.0,
        "Created by" => 120.0,
        _ => 105.0,
    }
}

// the stored shapes: condition rows, groups and saved filters
pub use crate::filters::{FilterField, FilterGroup, FilterRow, Join, SavedFilter};

/// Closes whichever menu (popup or context menu) is showing.
pub fn close_any(ui: &mut Ui) {
    close_popup(ui);
    ui.close_menu();
}

impl App {
    // ---------------------------------------------------------------- recurring

    /// When a repeating task is closed it opens again with its dates moved on.
    pub(super) fn roll_recurring(&self, old: &Task, new: &mut Task) {
        let Some(r) = new.recur.clone() else { return };
        let closes = |t: &Task| {
            self.data.statuses_for_list(t.list_id).iter().any(|s| {
                s.name == t.status && matches!(s.kind, StatusKind::Closed | StatusKind::Done)
            })
        };
        if closes(old) || !closes(new) {
            return;
        }
        let first = self.data.statuses_for_list(new.list_id)[0].name.clone();
        let base = new.due.unwrap_or_else(today);
        let next = next_recur(base, &r);
        let shift = next - base;
        new.due = Some(next);
        if let Some(s) = new.start {
            new.start = Some(s + shift);
        }
        new.status = first;
        // it's open again: no Date done / Date closed until it next closes
        new.done_at = None;
        new.closed_at = None;
        new.activity.push(Activity {
            ts: now_ts(),
            author: self.data.me,
            text: format!(
                "completed a {} repeat; next due {}",
                r,
                crate::i18n::in_english(|| match new.due_at() {
                    // the time stays with the task
                    Some(at) => format!("{} at {}", fmt_date(next), fmt_time(at)),
                    None => fmt_date(next),
                })
            ),
            comment: false,
            ..Default::default()
        });
        for c in &mut new.checklist {
            c.done = false;
        }
    }

    // ---------------------------------------------------------------- recent

    /// Remembers what was opened (tasks, lists, docs) for the Recent card.
    pub(super) fn note_recent(&mut self) {
        let key = if let Some(id) = self.open_task {
            format!("task:{}", id)
        } else if self.section == Section::Docs && self.doc_open.is_some() {
            format!("doc:{}", self.doc_open.unwrap_or(0))
        } else if let (Section::Home, Location::List(id)) = (self.section, self.loc) {
            format!("list:{}", id)
        } else {
            return;
        };
        if self.data.extra.recent.first() == Some(&key) {
            return;
        }
        self.data.extra.recent.retain(|k| *k != key);
        self.data.extra.recent.insert(0, key);
        self.data.extra.recent.truncate(20);
        self.dirty = true;
    }

    /// Recent items that still exist (and sit inside `lists` when given).
    pub(super) fn recent_items(&self, lists: Option<&[u64]>) -> Vec<(String, u64, String, String)> {
        let mut out = vec![];
        for k in &self.data.extra.recent {
            let Some((kind, id)) = k.split_once(':') else {
                continue;
            };
            let Ok(id) = id.parse::<u64>() else { continue };
            match kind {
                "task" => {
                    if let Some(t) = self.task(id) {
                        if lists.map_or(true, |l| l.contains(&t.list_id)) {
                            out.push((
                                "task".into(),
                                id,
                                t.name.clone(),
                                self.data.list_name(t.list_id),
                            ));
                        }
                    }
                }
                "list" => {
                    if self.data.location_exists(Location::List(id))
                        && lists.map_or(true, |l| l.contains(&id))
                    {
                        let sp = self
                            .data
                            .space_of_list(id)
                            .map(|s| s.name.clone())
                            .unwrap_or_default();
                        out.push(("list".into(), id, self.data.list_name(id), sp));
                    }
                }
                "doc" => {
                    if let Some(d) = self.data.docs.iter().find(|d| d.id == id) {
                        let space_ok = match (lists, d.space) {
                            (None, _) => true,
                            (Some(l), Some(sp)) => self
                                .data
                                .lists_in(Location::Space(sp))
                                .map_or(false, |sl| sl.iter().any(|x| l.contains(x))),
                            _ => false,
                        };
                        if space_ok {
                            out.push(("doc".into(), id, d.title.clone(), tr("Doc").into()));
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    // ---------------------------------------------------------------- shortcuts

    /// Single-key shortcuts (only while no text field has focus):
    /// T new task, R reminder, N or P notepad, Cmd+\ sidebar, Cmd+[ / ] history.
    pub(super) fn single_key_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Backslash)) {
            self.sidebar_collapsed = !self.sidebar_collapsed;
        }
        // Cmd+, Settings
        if self.modal.is_none()
            && self.palette.is_none()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma))
        {
            self.open_settings("My settings");
        }
        let typing = ctx.memory(|m| m.focused().is_some()) || ctx.wants_keyboard_input();
        if typing
            || self.modal.is_some()
            || self.quick_add.is_some()
            || self.palette.is_some()
            || self.rename.is_some()
        {
            return;
        }
        // Cmd+[ / Cmd+]: back / forward, like the top bar arrows
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::OpenBracket)) {
            self.go_back();
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::CloseBracket)) {
            self.go_forward();
            return;
        }
        let plain = ctx.input(|i| !i.modifiers.command && !i.modifiers.alt && !i.modifiers.ctrl);
        if !plain {
            return;
        }
        if self.open_task.is_none() {
            if ctx.input(|i| i.key_pressed(egui::Key::T)) {
                self.open_quick_add(None);
            } else if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.modal = Some(Modal::reminder(String::new(), today()));
            } else if ctx.input(|i| i.key_pressed(egui::Key::N) || i.key_pressed(egui::Key::P)) {
                self.notepad_open = !self.notepad_open;
            }
        }
    }

    /// The tasks around the open one, in the order the view shows them.
    pub(super) fn neighbours(&self, tid: u64) -> (Option<u64>, Option<u64>) {
        let ids: Vec<u64> = self.filtered().iter().map(|t| t.id).collect();
        let Some(i) = ids.iter().position(|x| *x == tid) else {
            return (None, None);
        };
        (
            if i > 0 { Some(ids[i - 1]) } else { None },
            ids.get(i + 1).copied(),
        )
    }
}
