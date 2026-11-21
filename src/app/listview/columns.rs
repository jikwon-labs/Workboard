// The List view's columns: which property and custom-field columns fit the
// row width, and drawing a custom-field cell.

use super::*;

pub(crate) struct Cols {
    pub assignee: Rect,
    pub due: Rect,
    pub priority: Rect,
    pub status: Rect,
    pub comments: Rect,
    pub created: Rect,
    /// Extra property columns that are shown (Created by, Tags...).
    pub extra: Vec<(&'static str, Rect)>,
    /// Custom field columns that are shown, by field id.
    pub fields: Vec<(u64, Rect)>,
    /// Left edge of the leftmost shown column (the name ends before it).
    pub left: f32,
}

/// The List's `hidden_cols` key of a custom field column.
pub(crate) fn field_col_key(id: u64) -> String {
    format!("field:{}", id)
}

/// Width of a custom field column in the List.
pub(super) const FIELD_COL_W: f32 = 130.0;

#[cfg(test)]
/// Column rectangles from the right; hidden columns get an empty rect.
pub(crate) fn col_layout(row: Rect, hidden: &HashSet<String>, show_pr: bool) -> Cols {
    col_layout_with(row, hidden, show_pr, &[])
}

/// `col_layout` with the workspace's custom fields (by id): those not
/// hidden come right after the built-in columns, before the properties,
/// while the name keeps room to read.
pub(super) fn col_layout_with(
    row: Rect,
    hidden: &HashSet<String>,
    show_pr: bool,
    fields: &[u64],
) -> Cols {
    // In a narrow window the name keeps MIN_BUILTIN px; the built-in columns
    // that do not fit are dropped, least important first.
    const MIN_BUILTIN: f32 = 220.0;
    const BUILTIN: [(&str, f32); 6] = [
        ("Assignee", 100.0),
        ("Due date", 100.0),
        ("Priority", 100.0),
        ("Status", 130.0),
        ("Comments", 90.0),
        ("Date created", 110.0),
    ];
    let mut budget = row.width() - 40.0 - MIN_BUILTIN;
    let mut dropped: Vec<&str> = vec![];
    for (name, w) in BUILTIN {
        if hidden.contains(name) || (name == "Priority" && !show_pr) {
            continue;
        }
        if w <= budget {
            budget -= w;
        } else {
            budget = -1.0;
            dropped.push(name);
        }
    }
    let mut x = row.right() - 40.0;
    let mut take = |name: &str, w: f32| {
        let off =
            hidden.contains(name) || (name == "Priority" && !show_pr) || dropped.contains(&name);
        if off {
            return Rect::from_min_max(
                pos2(row.right(), row.top()),
                pos2(row.right(), row.bottom()),
            );
        }
        x -= w;
        Rect::from_min_max(pos2(x, row.top()), pos2(x + w, row.bottom()))
    };
    let comments = take("Comments", 90.0);
    let status = take("Status", 130.0);
    let priority = take("Priority", 100.0);
    let due = take("Due date", 100.0);
    let assignee = take("Assignee", 100.0);
    let created = take("Date created", 110.0);
    // extra property columns only while the name keeps room to read
    const MIN_NAME: f32 = 300.0;
    let mut field_cols = vec![];
    for id in fields {
        let w = FIELD_COL_W;
        if !hidden.contains(&field_col_key(*id)) && x - w >= row.left() + MIN_NAME {
            x -= w;
            field_cols.push((
                *id,
                Rect::from_min_max(pos2(x, row.top()), pos2(x + w, row.bottom())),
            ));
        }
    }
    let mut extra = vec![];
    for p in parity::PROPERTIES {
        let w = parity::prop_width(p);
        if !hidden.contains(p) && x - w >= row.left() + MIN_NAME {
            x -= w;
            extra.push((
                p,
                Rect::from_min_max(pos2(x, row.top()), pos2(x + w, row.bottom())),
            ));
        }
    }
    Cols {
        assignee,
        due,
        priority,
        status,
        comments,
        created,
        extra,
        fields: field_cols,
        left: x,
    }
}

impl App {
    /// The custom fields the List can show as columns (none while the
    /// Custom Fields ClickApp is off).
    pub(crate) fn list_field_ids(&self) -> Vec<u64> {
        if self.data.extra.apps.custom_fields {
            self.data.extra.fields.iter().map(|f| f.id).collect()
        } else {
            vec![]
        }
    }

    /// The List's columns in a row rectangle.
    pub(crate) fn list_cols(&self, row: Rect) -> Cols {
        col_layout_with(
            row,
            &self.hidden_cols,
            self.data.extra.apps.priorities,
            &self.list_field_ids(),
        )
    }

    /// A custom field's cell in a List row: shows the value and edits it in
    /// place (a checkbox ticks, a dropdown or date opens its picker, text and
    /// numbers open a small box). Returns whether `edited` changed.
    pub(super) fn field_cell(
        &mut self,
        ui: &mut Ui,
        r: Rect,
        id: egui::Id,
        task: &Task,
        fid: u64,
        edited: &mut Task,
    ) -> bool {
        let t = self.t();
        let Some(f) = self.data.extra.fields.iter().find(|f| f.id == fid).cloned() else {
            return false;
        };
        let v = task.custom.get(&fid).cloned().unwrap_or_default();
        let cell = r.shrink2(vec2(0.0, 4.0));
        let resp = ui.interact(cell, id, Sense::click());
        name_it(&resp, &format!("{} of {}", f.name, task.name));
        if resp.hovered() {
            ui.painter().rect_stroke(
                cell.shrink2(vec2(2.0, 0.0)),
                4.0,
                Stroke::new(1.0_f32, t.border),
                egui::StrokeKind::Inside,
            );
        }
        let p = ui.painter_at(cell);
        let at = cell.left_center() + vec2(6.0, 0.0);
        let font = FontId::proportional(13.0);
        let mut set: Option<String> = None;
        match f.kind.as_str() {
            "checkbox" => {
                let on = v == "true";
                let b = Rect::from_center_size(at + vec2(7.0, 0.0), vec2(14.0, 14.0));
                p.rect_stroke(
                    b,
                    3.0,
                    Stroke::new(1.2_f32, t.text3),
                    egui::StrokeKind::Inside,
                );
                if on {
                    p.rect_filled(b, 3.0, t.primary);
                    p.text(
                        b.center(),
                        Align2::CENTER_CENTER,
                        "\u{2713}",
                        FontId::proportional(11.0),
                        t.on_primary,
                    );
                }
                if resp.clicked() {
                    set = Some(if on { String::new() } else { "true".into() });
                }
            }
            "dropdown" => {
                if !v.is_empty() {
                    p.text(at, Align2::LEFT_CENTER, &v, font, t.text);
                }
                popup(ui, &resp, id.with("pop"), 180.0, |ui| {
                    if ui.selectable_label(v.is_empty(), tr("Empty")).clicked() {
                        set = Some(String::new());
                        close_popup(ui);
                    }
                    for o in &f.options {
                        if ui.selectable_label(v == *o, o).clicked() {
                            set = Some(o.clone());
                            close_popup(ui);
                        }
                    }
                });
            }
            "date" => {
                let mut d = NaiveDate::parse_from_str(&v, "%Y-%m-%d").ok();
                if let Some(day) = d {
                    p.text(at, Align2::LEFT_CENTER, fmt_date(day), font, t.text2);
                }
                if let Some(true) = popup(ui, &resp, id.with("pop"), DATE_MENU_W, |ui| {
                    date_menu(ui, &t, id.with("cal"), &mut d)
                }) {
                    set = Some(d.map(|d| d.to_string()).unwrap_or_default());
                }
            }
            kind => {
                if !v.is_empty() {
                    p.text(at, Align2::LEFT_CENTER, &v, font, t.text2);
                }
                let number = kind == "number";
                let tid = id.with("text");
                popup(ui, &resp, id.with("pop"), 200.0, |ui| {
                    let mut text: String = ui
                        .data(|d| d.get_temp::<String>(tid))
                        .unwrap_or_else(|| v.clone());
                    let r = ui.add(field(&mut text).hint_text(tr("Empty")).desired_width(180.0));
                    name_field(&r, &format!("Edit {}", f.name));
                    focus_once(ui, &r);
                    let ok =
                        !number || text.trim().is_empty() || text.trim().parse::<f64>().is_ok();
                    if number && !ok {
                        ui.label(RichText::new(tr("Enter a number")).small().color(t.danger));
                    }
                    if r.changed() && ok {
                        set = Some(text.trim().to_string());
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        close_popup(ui);
                        ui.data_mut(|d| d.remove::<String>(tid));
                    } else {
                        ui.data_mut(|d| d.insert_temp(tid, text));
                    }
                });
            }
        }
        match set {
            Some(new) if new != v => {
                if new.is_empty() {
                    edited.custom.remove(&fid);
                } else {
                    edited.custom.insert(fid, new);
                }
                true
            }
            _ => false,
        }
    }
}
