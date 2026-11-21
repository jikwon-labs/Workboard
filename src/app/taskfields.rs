// The task window's smaller fields: task type, the time estimate picker,
// time entries, tag suggestions, watchers, checklist rows, relationships
// (waiting on / blocking / linked), the parent picker, the formatted
// description and attachments.

use super::docs::{line_kinds, toggle_check_at, LineKind};
use super::files::Transfer;
use super::*;
use crate::media;

/// What the task picker dialog sets on the task it was opened for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskPick {
    /// the task becomes a subtask of the picked one
    Parent,
    /// the picked task becomes a subtask of this one
    Child,
    WaitingOn,
    Blocking,
    Linked,
}

impl TaskPick {
    pub fn title(self) -> &'static str {
        match self {
            TaskPick::Parent => "Make subtask of",
            TaskPick::Child => "Add existing task as subtask",
            TaskPick::WaitingOn => "Waiting on",
            TaskPick::Blocking => "Blocking",
            TaskPick::Linked => "Link a task",
        }
    }
}

/// Minutes from "2h 30m", "1.5h", "45m", "1d" (8 hours), "2:30" or a bare
/// number (hours, as in the original). None when it can't be read.
pub fn parse_estimate(s: &str) -> Option<u32> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }
    if let Some((h, m)) = s.split_once(':') {
        let h: u32 = h.trim().parse().ok()?;
        let m: u32 = m.trim().parse().ok()?;
        return Some(h * 60 + m);
    }
    let mut total = 0.0f64;
    let mut num = String::new();
    let mut units = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            continue;
        }
        if c.is_whitespace() {
            continue;
        }
        if !c.is_alphabetic() {
            return None;
        }
        let mut unit = String::from(c);
        while let Some(&n) = chars.peek() {
            if !n.is_alphabetic() {
                break;
            }
            unit.push(n);
            chars.next();
        }
        let v: f64 = num.parse().ok()?;
        num.clear();
        let mult = match unit.as_str() {
            "h" | "hr" | "hrs" | "hour" | "hours" | "\u{c2dc}\u{ac04}" => 60.0,
            "m" | "min" | "mins" | "minute" | "minutes" | "\u{bd84}" => 1.0,
            "d" | "day" | "days" => 480.0,
            "w" | "wk" | "week" | "weeks" => 2400.0,
            _ => return None,
        };
        total += v * mult;
        units += 1;
    }
    if !num.is_empty() {
        let v: f64 = num.parse().ok()?;
        // "2h 30": a trailing bare number after a unit is minutes
        total += v * if units > 0 { 1.0 } else { 60.0 };
    }
    (total >= 0.0 && total < u32::MAX as f64).then(|| total.round() as u32)
}

/// "2h 30m", "2h", "45m".
pub fn fmt_estimate(min: u32) -> String {
    match (min / 60, min % 60) {
        (0, m) => format!("{}m", m),
        (h, 0) => format!("{}h", h),
        (h, m) => format!("{}h {}m", h, m),
    }
}

/// The "Task" pill above the title: a menu to switch between Task and
/// Milestone.
pub(super) fn task_type_pill(ui: &mut Ui, t: &Theme, id: egui::Id, milestone: &mut bool) {
    let label = if *milestone { "Milestone" } else { "Task" };
    let g = ui
        .painter()
        .layout_no_wrap(tr(label).into(), FontId::proportional(13.5), t.text);
    let (pill_r, resp) = ui.allocate_exact_size(vec2(g.size().x + 44.0, 26.0), Sense::click());
    name_it(&resp, "Task type");
    if resp.hovered() {
        ui.painter().rect_filled(pill_r, 7.0, t.hover);
    }
    ui.painter().rect_stroke(
        pill_r,
        7.0,
        Stroke::new(1.0_f32, t.border),
        egui::StrokeKind::Inside,
    );
    let c = pill_r.left_center() + vec2(13.0, 0.0);
    if *milestone {
        paint_diamond(ui.painter(), c, 5.5, t.text2);
    } else {
        ui.painter().circle_filled(c, 5.0, t.text2);
    }
    ui.painter().galley(
        pos2(pill_r.left() + 24.0, pill_r.center().y - g.size().y / 2.0),
        g,
        t.text,
    );
    paint_triangle(
        ui.painter(),
        pill_r.right_center() - vec2(10.0, 0.0),
        true,
        t.text2,
    );
    let resp = resp.on_hover_text(tr("Task type"));
    popup(ui, &resp, id.with("type"), 180.0, |ui| {
        ui.label(RichText::new(tr("Task type")).small().color(t.text3));
        let tc = t.text2;
        if menu_row(ui, t, tr("Task"), !*milestone, |p, c| {
            p.circle_filled(c, 5.0, tc);
        })
        .clicked()
        {
            *milestone = false;
            close_popup(ui);
        }
        if menu_row(ui, t, tr("Milestone"), *milestone, |p, c| {
            paint_diamond(p, c, 5.5, tc)
        })
        .clicked()
        {
            *milestone = true;
            close_popup(ui);
        }
    });
}

fn paint_diamond(p: &egui::Painter, c: egui::Pos2, r: f32, color: Color32) {
    let pts = vec![
        c + vec2(0.0, -r),
        c + vec2(r, 0.0),
        c + vec2(0.0, r),
        c + vec2(-r, 0.0),
    ];
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// Quick picks in the time estimate menu, in minutes (a day is 8 hours).
pub const ESTIMATE_PRESETS: [u32; 6] = [15, 30, 60, 120, 240, 480];

/// The time estimate: shows "2h 30m" (or Empty) and opens a menu with quick
/// picks and hour and minute steppers. It used to be free text, which took
/// any digits ("213123123") and dropped what it could not read without a
/// word; a picker only offers times that make sense.
pub(super) fn estimate_field(
    ui: &mut Ui,
    t: &Theme,
    id: egui::Id,
    width: f32,
    est: &mut Option<u32>,
) {
    let (r, resp) = ui.allocate_exact_size(vec2(width, 26.0), Sense::click());
    name_it(&resp, "Time estimate value");
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    let (text, color) = match *est {
        Some(m) => (fmt_hm(m as i64 * 60), t.text),
        None => (tr("Empty").to_string(), t.text3),
    };
    ui.painter().text(
        r.left_center() + vec2(6.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0),
        color,
    );
    popup(ui, &resp, id.with("est"), 220.0, |ui| {
        estimate_menu(ui, t, est)
    });
}

/// The time estimate menu: quick picks, then hours and minutes to set any
/// other time, and Clear.
fn estimate_menu(ui: &mut Ui, t: &Theme, est: &mut Option<u32>) {
    ui.label(RichText::new(tr("Time estimate")).small().color(t.text3));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for m in ESTIMATE_PRESETS {
            let r = ui.selectable_label(*est == Some(m), fmt_hm(m as i64 * 60));
            name_it(&r, &format!("Estimate {}", fmt_estimate(m)));
            if r.clicked() {
                *est = Some(m);
                close_popup(ui);
            }
        }
    });
    ui.add_space(6.0);
    let cur = est.unwrap_or(0);
    let mut total = cur;
    hours_minutes(ui, &mut total, 999, "Estimate hours", "Estimate minutes");
    if total != cur {
        *est = (total > 0).then_some(total);
    }
    if est.is_some() {
        ui.add_space(6.0);
        let r = ui.add(ghost(egui::Button::new(
            RichText::new(tr("Clear")).color(t.text2),
        )));
        name_it(&r, "Clear estimate");
        if r.clicked() {
            *est = None;
            close_popup(ui);
        }
    }
}

/// An hour stepper (up to `max_h`) and a minute stepper (0 to 59) for a
/// time in minutes. Each can be dragged or clicked and typed; what is typed
/// is held to the range.
fn hours_minutes(ui: &mut Ui, total: &mut u32, max_h: u32, h_name: &str, m_name: &str) {
    let (mut h, mut m) = ((*total / 60).min(max_h), *total % 60);
    ui.horizontal(|ui| {
        let r = ui.add(egui::DragValue::new(&mut h).range(0..=max_h).speed(0.1));
        name_it(&r, h_name);
        ui.label(tr("hours"));
        ui.add_space(6.0);
        let r = ui.add(egui::DragValue::new(&mut m).range(0..=59).speed(0.2));
        name_it(&r, m_name);
        ui.label(tr("minutes"));
    });
    *total = h * 60 + m;
}

/// Quick picks for time added by hand, in minutes.
pub const ADD_TIME_PRESETS: [u32; 5] = [15, 30, 60, 120, 240];

/// The time entries popup under Track time: the entries (newest first)
/// with a remove button, and a row to add time by hand.
pub(super) fn time_entries_menu(ui: &mut Ui, t: &Theme, id: egui::Id, task: &mut Task, me: u64) {
    ui.label(RichText::new(tr("Time entries")).small().color(t.text3));
    if task.time_entries.is_empty() {
        ui.label(RichText::new(tr("No time tracked yet")).color(t.text3));
    }
    let mut remove = None;
    for (i, e) in task.time_entries.iter().enumerate().rev() {
        ui.horizontal(|ui| {
            let day = local(e.start)
                .map(|d| d.format("%b %-d").to_string())
                .unwrap_or_default();
            ui.label(RichText::new(day).color(t.text2));
            ui.label(fmt_duration(e.end - e.start));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let r = ui.add(ghost(egui::Button::new(
                    RichText::new("\u{00D7}").size(10.0).color(t.text3),
                )));
                name_it(&r, &format!("Remove entry {}", i + 1));
                if r.on_hover_text(tr("Remove this entry")).clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        task.time_entries.remove(i);
    }
    ui.separator();
    // picked, not typed, as the estimate: quick picks or hours and minutes
    // (up to a day), then Today or Yesterday
    let min_id = id.with("add_time");
    let day_id = id.with("add_day");
    let mut min: u32 = ui.data(|d| d.get_temp(min_id)).unwrap_or(0);
    let mut yesterday: bool = ui.data(|d| d.get_temp(day_id)).unwrap_or(false);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for m in ADD_TIME_PRESETS {
            let r = ui.selectable_label(min == m, fmt_hm(m as i64 * 60));
            name_it(&r, &format!("Add {}", fmt_estimate(m)));
            if r.clicked() {
                min = m;
            }
        }
    });
    hours_minutes(ui, &mut min, 24, "Hours to add", "Minutes to add");
    min = min.min(24 * 60);
    let mut add = false;
    ui.horizontal(|ui| {
        if ui.selectable_label(!yesterday, tr("Today")).clicked() {
            yesterday = false;
        }
        if ui.selectable_label(yesterday, tr("Yesterday")).clicked() {
            yesterday = true;
        }
        let r = ui.add_enabled_ui(min > 0, |ui| primary_button(ui, t, tr("Add time")));
        if r.inner.clicked() {
            add = true;
        }
    });
    if add && min > 0 {
        let end = now_ts() - if yesterday { 86400 } else { 0 };
        task.time_entries.push(TimeEntry {
            start: end - min as i64 * 60,
            end,
            member: me,
        });
        min = 0;
    }
    ui.data_mut(|d| {
        d.insert_temp(min_id, min);
        d.insert_temp(day_id, yesterday);
    });
}

impl App {
    /// Every tag used on a task, sorted.
    /// The tag field: typing offers tags already in use; Enter adds what is
    /// typed.
    pub(super) fn tag_field(&mut self, ui: &mut Ui, t: &Theme, id: egui::Id, task: &mut Task) {
        let r = ui.add(
            field(&mut self.tag_text)
                .hint_text(tr("Add tag"))
                .desired_width(100.0),
        );
        name_field(&r, "Add tag");
        let pid = id.with("tag_suggest");
        if r.gained_focus() || (r.has_focus() && r.changed()) {
            ui.memory_mut(|m| m.open_popup(pid));
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let tag = self.tag_text.trim().to_lowercase();
            if !tag.is_empty() && !task.tags.contains(&tag) {
                task.tags.push(tag);
            }
            self.tag_text.clear();
        }
        let q = self.tag_text.trim().to_lowercase();
        let sugg: Vec<String> = self
            .all_tags()
            .into_iter()
            .filter(|g| !task.tags.contains(g) && (q.is_empty() || g.contains(&q)))
            .take(8)
            .collect();
        if sugg.is_empty() {
            if ui.memory(|m| m.is_popup_open(pid)) {
                ui.memory_mut(|m| m.close_popup());
            }
            return;
        }
        egui::popup_below_widget(
            ui,
            pid,
            &r,
            egui::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(180.0);
                ui.label(RichText::new(tr("Tags in use")).small().color(t.text3));
                for g in &sugg {
                    let c = tag_color(g);
                    if menu_row_named(ui, t, &format!("Tag {}", g), g, false, |p, at| {
                        p.circle_filled(at, 5.0, c);
                    })
                    .clicked()
                    {
                        task.tags.push(g.clone());
                        self.tag_text.clear();
                        close_popup(ui);
                    }
                }
            },
        );
    }

    /// Header: the avatars of who gets the task's updates (click to choose)
    /// and a Watch toggle for me. Assignees and the maker get them until
    /// they turn them off (`Task::watched_by`).
    pub(super) fn watchers_header(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        id: egui::Id,
        task: &mut Task,
    ) {
        let me = self.data.me;
        let on = task.watched_by(me);
        let b = super::icons::icon_button_ex(
            ui,
            t,
            "eye",
            tr(if on { "Watching" } else { "Watch" }),
            if on { "Stop watching" } else { "Watch task" },
            false,
            0.0,
            if on { t.text } else { t.text2 },
            t.text2,
        );
        if b.on_hover_text(tr(if on {
            "Stop getting updates"
        } else {
            "Get updates on this task"
        }))
        .clicked()
        {
            task.set_watching(me, !on);
        }
        let watching: Vec<u64> = self
            .data
            .members
            .iter()
            .filter(|m| !m.invited && task.watched_by(m.id))
            .map(|m| m.id)
            .collect();
        let n = watching.len().min(4) as f32;
        let w = if n == 0.0 { 22.0 } else { 14.0 * n + 12.0 };
        let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
        name_it(&resp, "Watchers");
        if resp.hovered() {
            ui.painter().rect_filled(r, 6.0, t.hover);
        }
        if watching.is_empty() {
            // a person, not a second eye beside the Watch button's
            super::icons::line_icon_scaled(ui.painter(), "person", r.center(), t.text3, 0.8);
        } else {
            let shown: Vec<u64> = watching.iter().take(4).copied().collect();
            paint_avatars(
                ui.painter(),
                r.left_center() + vec2(2.0, 0.0),
                9.0,
                &shown,
                &self.data,
                t.card,
            );
        }
        let resp = resp.on_hover_text(trf!("{} watching", watching.len()));
        let data = &self.data;
        popup(ui, &resp, id.with("watchers"), 220.0, |ui| {
            ui.label(RichText::new(tr("Watchers")).small().color(t.text3));
            for m in data.members.iter().filter(|m| !m.invited) {
                let sel = task.watched_by(m.id);
                let ring = t.card;
                let r = menu_row_named(
                    ui,
                    t,
                    &format!("Watcher {}", m.shown()),
                    m.shown(),
                    sel,
                    |p, c| paint_avatar(p, c, 9.0, m, ring),
                );
                if sel {
                    super::icons::line_icon_scaled(
                        ui.painter(),
                        "check",
                        r.rect.right_center() - vec2(16.0, 0.0),
                        t.selected_text,
                        0.8,
                    );
                }
                if r.clicked() {
                    task.set_watching(m.id, !sel);
                }
            }
        });
    }

    /// The description, with Doc formatting shown while editing (headings,
    /// bullets, checkboxes, bold, code).
    pub(super) fn description_field(&mut self, ui: &mut Ui, t: &Theme, task: &mut Task) {
        let tc = t.clone();
        let mut layouter = |ui: &Ui, s: &str, wrap: f32| {
            ui.fonts(|f| f.layout_job(super::docs::doc_layout_job(s, &tc, wrap)))
        };
        let out = egui::TextEdit::multiline(&mut task.description)
            .hint_text(tr("Add description"))
            .frame(false)
            .font(FontId::proportional(15.0))
            .desired_rows(4)
            .desired_width(f32::INFINITY)
            .layouter(&mut layouter)
            .show(ui);
        name_field(&out.response, "Add description");
        // painted marks over the hidden Markdown markers, as in Docs
        let click = if out.response.clicked() {
            ui.input(|i| i.pointer.interact_pos())
        } else {
            None
        };
        let mut tick = None;
        for (kind, start, _) in line_kinds(&task.description) {
            let row = out
                .galley
                .pos_from_cursor(&out.galley.from_ccursor(egui::text::CCursor::new(start)));
            let at = out.galley_pos + row.min.to_vec2();
            let mid = at.y + row.height() / 2.0;
            let p = ui.painter();
            match kind {
                LineKind::Bullet => {
                    p.circle_filled(pos2(at.x + 5.0, mid), 2.6, t.text);
                }
                LineKind::Check(done) => {
                    let bx = Rect::from_min_size(pos2(at.x + 2.0, mid - 7.5), vec2(15.0, 15.0));
                    if done {
                        p.rect_filled(bx, 3.0, t.primary);
                        p.line_segment(
                            [
                                bx.left_center() + vec2(3.5, 0.0),
                                bx.center_bottom() + vec2(-1.0, -4.0),
                            ],
                            Stroke::new(1.8_f32, t.on_primary),
                        );
                        p.line_segment(
                            [
                                bx.center_bottom() + vec2(-1.0, -4.0),
                                bx.right_top() + vec2(-3.5, 4.0),
                            ],
                            Stroke::new(1.8_f32, t.on_primary),
                        );
                    } else {
                        p.rect_stroke(
                            bx,
                            3.0,
                            Stroke::new(1.3_f32, t.text3),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if click.is_some_and(|c| bx.expand(2.0).contains(c)) {
                        tick = Some(start);
                    }
                }
                LineKind::Quote => {
                    p.rect_filled(
                        Rect::from_min_size(
                            pos2(at.x + 1.0, at.y + 1.0),
                            vec2(3.0, row.height() - 2.0),
                        ),
                        1.0,
                        t.border,
                    );
                }
                LineKind::Divider => {
                    p.hline(
                        at.x..=out.response.rect.right(),
                        mid,
                        Stroke::new(1.0_f32, t.border),
                    );
                }
                _ => {}
            }
        }
        if let Some(start) = tick {
            toggle_check_at(&mut task.description, start);
        }
    }

    /// Checklist rows: tick, click the text to rename, and on hover assign,
    /// move up or down, turn into a subtask or remove.
    pub(super) fn checklist_rows(&mut self, ui: &mut Ui, t: &Theme, id: egui::Id, task: &mut Task) {
        let tid = task.id;
        let edit_id = id.with(("ck_edit", tid));
        let mut editing: Option<usize> = ui.data(|d| d.get_temp(edit_id)).unwrap_or(None);
        let members: Vec<Member> = self
            .data
            .members
            .iter()
            .filter(|m| !m.invited)
            .cloned()
            .collect();
        let n = task.checklist.len();
        let (mut remove, mut swap, mut to_sub) = (None, None, None);
        for i in 0..n {
            let w = ui.available_width();
            let (r, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
            let hover = ui.rect_contains_pointer(r);
            if hover {
                ui.painter().rect_filled(r, 6.0, t.hover);
            }
            let mut row = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(r)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            let item = &mut task.checklist[i];
            let label = item.text.clone();
            let cb = check(&mut row, &mut item.done, "");
            name_it(&cb, &format!("Done {}", label));
            if editing == Some(i) {
                let te =
                    row.add(field(&mut item.text).desired_width((r.width() - 190.0).max(80.0)));
                name_field(&te, "Checklist item name");
                focus_once(&row, &te);
                if te.lost_focus() {
                    editing = None;
                    if item.text.trim().is_empty() {
                        remove = Some(i);
                    }
                }
            } else {
                let mut txt = RichText::new(&item.text);
                if item.done {
                    txt = txt.strikethrough().color(t.text3);
                }
                // the hover fill keeps room around the name, as on a button
                // (a fill the size of the text sat right against its letters)
                let slot = row.painter().add(egui::Shape::Noop);
                let l = row.add(
                    egui::Label::new(txt)
                        .sense(Sense::click())
                        .selectable(false)
                        .truncate(),
                );
                if l.hovered() {
                    let fill = crate::widgets::hover_fill(&row);
                    let fill = if l.is_pointer_button_down_on() {
                        fill.gamma_multiply(1.5)
                    } else {
                        fill
                    };
                    let rect = l.rect.expand2(vec2(6.0, 3.0)).intersect(r);
                    crate::widgets::paint_hover_slot(&row, slot, rect, fill);
                }
                if l.on_hover_text(tr("Click to rename")).clicked() {
                    editing = Some(i);
                }
            }
            row.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let small = |s: &str| {
                    ghost(egui::Button::new(
                        RichText::new(s).size(11.0).color(t.text2),
                    ))
                };
                // the assignee shows always once set; the button only on hover
                let pid = id.with(("ck_as", tid, i));
                if item.assignee.is_some() || hover || ui.memory(|m| m.is_popup_open(pid)) {
                    let (ar, aresp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    hover_bg(ui, &aresp, ar, 11.0);
                    name_it(&aresp, &format!("Assign item {}", label));
                    match item
                        .assignee
                        .and_then(|a| members.iter().find(|m| m.id == a))
                    {
                        Some(m) => paint_avatar(ui.painter(), ar.center(), 9.0, m, t.card),
                        None => {
                            super::icons::line_icon_scaled(
                                ui.painter(),
                                "person",
                                ar.center(),
                                t.text3,
                                0.8,
                            );
                        }
                    }
                    let aresp = aresp.on_hover_text(tr("Assign"));
                    popup(ui, &aresp, pid, 200.0, |ui| {
                        for m in &members {
                            let sel = item.assignee == Some(m.id);
                            let ring = t.card;
                            if menu_row(ui, t, m.shown(), sel, |p, c| {
                                paint_avatar(p, c, 9.0, m, ring)
                            })
                            .clicked()
                            {
                                item.assignee = if sel { None } else { Some(m.id) };
                                close_popup(ui);
                            }
                        }
                    });
                }
                if !hover {
                    return;
                }
                let x = ui.add(small("\u{00D7}"));
                name_it(&x, &format!("Remove item {}", label));
                if x.on_hover_text(tr("Remove")).clicked() {
                    remove = Some(i);
                }
                let s = ui.add(small("\u{21B3}"));
                name_it(&s, &format!("Make subtask {}", label));
                if s.on_hover_text(tr("Turn into a subtask")).clicked() {
                    to_sub = Some(i);
                }
                if i + 1 < n {
                    let d = ui.add(small("\u{2193}"));
                    name_it(&d, &format!("Move down {}", label));
                    if d.on_hover_text(tr("Move down")).clicked() {
                        swap = Some((i, i + 1));
                    }
                }
                if i > 0 {
                    let u = ui.add(small("\u{2191}"));
                    name_it(&u, &format!("Move up {}", label));
                    if u.on_hover_text(tr("Move up")).clicked() {
                        swap = Some((i - 1, i));
                    }
                }
            });
        }
        if let Some((a, b)) = swap {
            task.checklist.swap(a, b);
            editing = None;
        }
        if let Some(i) = to_sub {
            let item = task.checklist.remove(i);
            self.actions.push(Action::Add {
                list: task.list_id,
                name: item.text,
                status: None,
                priority: DEFAULT_PRIORITY,
                assignees: item.assignee.into_iter().collect(),
                due: None,
                parent: Some(tid),
                open: false,
            });
            editing = None;
        } else if let Some(i) = remove {
            task.checklist.remove(i);
            editing = None;
        }
        ui.data_mut(|d| d.insert_temp(edit_id, editing));
    }

    /// Tasks this one waits on, tasks waiting on it, and linked tasks (both
    /// directions); missing tasks are left out.
    pub(super) fn relations(&self, tid: u64) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
        let Some(task) = self.task(tid) else {
            return Default::default();
        };
        let exists = |i: &u64| self.task(*i).is_some();
        let waiting: Vec<u64> = task.waiting_on.iter().copied().filter(exists).collect();
        let blocking: Vec<u64> = self
            .data
            .tasks
            .iter()
            .filter(|x| x.waiting_on.contains(&tid))
            .map(|x| x.id)
            .collect();
        let mut linked: Vec<u64> = task.linked.iter().copied().filter(exists).collect();
        for x in self.data.tasks.iter().filter(|x| x.linked.contains(&tid)) {
            if !linked.contains(&x.id) {
                linked.push(x.id);
            }
        }
        (waiting, blocking, linked)
    }

    /// The task and every subtask under it.
    fn with_descendants(&self, tid: u64) -> Vec<u64> {
        let mut v = vec![tid];
        let mut i = 0;
        while i < v.len() {
            let k = v[i];
            v.extend(
                self.data
                    .tasks
                    .iter()
                    .filter(|t| t.parent == Some(k))
                    .map(|t| t.id),
            );
            i += 1;
        }
        v
    }

    /// Tasks the picker offers for `mode`.
    pub(super) fn pick_candidates(&self, tid: u64, mode: TaskPick) -> Vec<u64> {
        let Some(task) = self.task(tid) else {
            return vec![];
        };
        let (waiting, blocking, linked) = self.relations(tid);
        let below = self.with_descendants(tid);
        let mut above = vec![];
        let mut p = task.parent;
        while let Some(x) = p {
            if above.contains(&x) {
                break;
            }
            above.push(x);
            p = self.task(x).and_then(|t| t.parent);
        }
        self.data
            .tasks
            .iter()
            .filter(|x| x.id != tid && !x.archived)
            .filter(|x| match mode {
                TaskPick::Parent => !below.contains(&x.id) && task.parent != Some(x.id),
                TaskPick::Child => !above.contains(&x.id) && x.parent != Some(tid),
                TaskPick::WaitingOn => !waiting.contains(&x.id),
                TaskPick::Blocking => !blocking.contains(&x.id),
                TaskPick::Linked => !linked.contains(&x.id),
            })
            .map(|x| x.id)
            .collect()
    }

    /// Sets what the picker picked.
    pub(super) fn set_relation(&mut self, tid: u64, mode: TaskPick, other: u64) {
        if tid == other || self.task(tid).is_none() || self.task(other).is_none() {
            return;
        }
        let me = self.data.me;
        let name = |a: &App, i: u64| a.task(i).map(|t| t.name.clone()).unwrap_or_default();
        let (on, target, text) = match mode {
            TaskPick::Parent => (
                tid,
                other,
                format!("made this a subtask of {}", name(self, other)),
            ),
            TaskPick::Child => (
                other,
                tid,
                format!("made this a subtask of {}", name(self, tid)),
            ),
            TaskPick::WaitingOn => (tid, other, format!("is waiting on {}", name(self, other))),
            TaskPick::Blocking => (other, tid, format!("is waiting on {}", name(self, tid))),
            TaskPick::Linked => (tid, other, format!("linked {}", name(self, other))),
        };
        let target_list = self.task(target).map(|t| t.list_id).unwrap_or(0);
        let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == on) else {
            return;
        };
        match mode {
            TaskPick::Parent | TaskPick::Child => {
                t.parent = Some(target);
                // a subtask lives in its parent's List
                t.list_id = target_list;
            }
            TaskPick::WaitingOn | TaskPick::Blocking => {
                if !t.waiting_on.contains(&target) {
                    t.waiting_on.push(target);
                }
            }
            TaskPick::Linked => {
                if !t.linked.contains(&target) {
                    t.linked.push(target);
                }
            }
        }
        t.log(me, text);
        t.updated = now_ts();
        self.dirty = true;
    }

    /// A subtask becomes a task of its own again.
    pub(super) fn make_top_level(&mut self, tid: u64) {
        let me = self.data.me;
        if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == tid) {
            if t.parent.take().is_some() {
                t.log(me, "made this a top-level task".into());
                t.updated = now_ts();
                self.dirty = true;
            }
        }
    }

    /// Takes away a relationship of either direction.
    pub(super) fn remove_relation(&mut self, tid: u64, mode: TaskPick, other: u64) {
        for t in self.data.tasks.iter_mut() {
            let before = (t.waiting_on.len(), t.linked.len());
            match mode {
                TaskPick::WaitingOn if t.id == tid => t.waiting_on.retain(|x| *x != other),
                TaskPick::Blocking if t.id == other => t.waiting_on.retain(|x| *x != tid),
                TaskPick::Linked if t.id == tid => t.linked.retain(|x| *x != other),
                TaskPick::Linked if t.id == other => t.linked.retain(|x| *x != tid),
                _ => {}
            }
            if before != (t.waiting_on.len(), t.linked.len()) {
                t.updated = now_ts();
                self.dirty = true;
            }
        }
    }

    /// The section under Subtasks: Waiting on, Blocking and Linked tasks.
    pub(super) fn relationships_section(&mut self, ui: &mut Ui, t: &Theme, id: egui::Id, tid: u64) {
        let (waiting, blocking, linked) = self.relations(tid);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Relationships")).strong());
            let n = waiting.len() + blocking.len() + linked.len();
            if n > 0 {
                ui.label(RichText::new(n.to_string()).color(t.text3));
            }
        });
        let mut drop: Option<(TaskPick, u64)> = None;
        for (mode, ids) in [
            (TaskPick::WaitingOn, &waiting),
            (TaskPick::Blocking, &blocking),
            (TaskPick::Linked, &linked),
        ] {
            if ids.is_empty() {
                continue;
            }
            let (head, col, kind) = match mode {
                TaskPick::WaitingOn => ("Waiting on", WARN_ORANGE, "hourglass"),
                TaskPick::Blocking => ("Blocking", t.danger, "warning"),
                _ => ("Linked", t.text2, "link"),
            };
            super::icons::icon_label(ui, kind, RichText::new(tr(head)).small().color(col), col);
            for oid in ids.iter() {
                let Some(o) = self.task(*oid).cloned() else {
                    continue;
                };
                let st = self.data.status_of(&o);
                let w = ui.available_width();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                let hover = ui.rect_contains_pointer(r);
                if hover {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                paint_status_circle(ui.painter(), r.left_center() + vec2(14.0, 0.0), 7.0, &st);
                let closed = matches!(st.kind, StatusKind::Closed | StatusKind::Done);
                ui.painter()
                    .with_clip_rect(r.shrink2(vec2(30.0, 0.0)))
                    .text(
                        r.left_center() + vec2(32.0, 0.0),
                        Align2::LEFT_CENTER,
                        &o.name,
                        FontId::proportional(13.0),
                        if closed { t.text3 } else { t.text },
                    );
                name_it(&resp, &format!("{} {}", mode.title(), o.name));
                let x = ui.put(
                    Rect::from_center_size(r.right_center() - vec2(14.0, 0.0), vec2(20.0, 20.0)),
                    ghost(
                        egui::Button::new(RichText::new("\u{00D7}").size(10.0).color(if hover {
                            t.text2
                        } else {
                            Color32::TRANSPARENT
                        }))
                        .small(),
                    ),
                );
                name_it(&x, &format!("Remove link {}", o.name));
                if x.on_hover_text(tr("Remove")).clicked() {
                    drop = Some((mode, *oid));
                } else if resp.clicked() {
                    self.actions.push(Action::Open(*oid));
                }
            }
        }
        if let Some((mode, oid)) = drop {
            self.remove_relation(tid, mode, oid);
        }
        let r = add_line(ui, t, tr("+  Add relationship"));
        popup(ui, &r, id.with(("rel_add", tid)), 200.0, |ui| {
            for mode in [TaskPick::WaitingOn, TaskPick::Blocking, TaskPick::Linked] {
                if menu_row_plain(ui, t, tr(mode.title()), false).clicked() {
                    self.modal = Some(Modal::PickTask {
                        task: tid,
                        mode,
                        query: String::new(),
                    });
                    close_popup(ui);
                }
            }
        });
    }

    /// The picker dialog: search tasks, click one to set the relation.
    /// It lists at most `PICK_ROWS` tasks and says how many more match.
    pub(super) fn pick_task_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        tid: u64,
        mode: TaskPick,
        query: &mut String,
    ) -> bool {
        let mut close = false;
        let name = self.task(tid).map(|x| x.name.clone()).unwrap_or_default();
        let prompt = match mode {
            TaskPick::Parent => trf!("Choose the task \"{}\" goes under.", name),
            TaskPick::Child => trf!("Choose a task to put under \"{}\".", name),
            TaskPick::WaitingOn => trf!("Choose a task \"{}\" waits on.", name),
            TaskPick::Blocking => trf!("Choose a task \"{}\" blocks.", name),
            TaskPick::Linked => trf!("Choose a task to link to \"{}\".", name),
        };
        ui.label(RichText::new(prompt).color(t.text2));
        let r = ui.add(
            field(query)
                .hint_text(tr("Search tasks..."))
                .desired_width(f32::INFINITY),
        );
        name_field(&r, "Search tasks to pick");
        focus_once(ui, &r);
        ui.add_space(4.0);
        let q = query.trim().to_lowercase();
        let ids = self.pick_candidates(tid, mode);
        let mut picked = None;
        let more = egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                let mut shown = 0;
                let mut more = 0;
                for oid in ids {
                    let Some(o) = self.task(oid) else { continue };
                    if !q.is_empty() && !o.name.to_lowercase().contains(&q) {
                        continue;
                    }
                    shown += 1;
                    // the first 200; the rest are counted and found by searching
                    if shown > PICK_ROWS {
                        more += 1;
                        continue;
                    }
                    let st = self.data.status_of(o);
                    if menu_row_named(
                        ui,
                        t,
                        &format!("Pick {}", o.name),
                        &o.name,
                        false,
                        |p, c| paint_status_circle(p, c, 6.0, &st),
                    )
                    .clicked()
                    {
                        picked = Some(oid);
                    }
                }
                if shown == 0 {
                    // nothing typed yet: say why there is nothing, not "no match"
                    let text = if !q.is_empty() {
                        tr("No tasks match")
                    } else if self
                        .data
                        .tasks
                        .iter()
                        .any(|x| x.id != tid && !self.data.task_live(x))
                    {
                        tr("No other tasks to pick. Archived tasks are not offered here.")
                    } else {
                        tr("No other tasks to pick yet.")
                    };
                    ui.label(RichText::new(text).color(t.text3));
                }
                more
            })
            .inner;
        if more > 0 {
            ui.add_space(4.0);
            ui.label(
                RichText::new(trf!("{} more not shown. Search to find them.", more))
                    .small()
                    .color(t.text3),
            );
        }
        if let Some(oid) = picked {
            self.set_relation(tid, mode, oid);
            close = true;
        }
        close
    }

    /// Start getting a picture or video ready for a task.
    pub(super) fn add_task_file(&mut self, tid: u64, path: PathBuf) {
        let remote = self.media_remote();
        let p = self.files.add(Some(path), None, &remote);
        self.task_files.push((tid, p));
    }

    /// Read back the task attachments that were on their way in this
    /// workspace when the app last quit (once per workspace), and send them
    /// on: they join their task when they're up.
    pub(super) fn restore_task_files(&mut self, remote: &files::Remote) {
        if self.task_files_saved.is_some() {
            return;
        }
        let mut saved = vec![];
        for e in read_task_files(self.files.dir()) {
            if e.ws != self.ws_file
                || self.task_files.iter().any(|(t, p)| {
                    *t == e.task && p.att.as_ref().is_some_and(|a| a.file == e.att.file)
                })
            {
                continue;
            }
            // only files the cache still has
            if files::parts(&e.att)
                .iter()
                .any(|p| self.files.local(&p.file).is_none())
            {
                continue;
            }
            self.files.upload(&e.att, remote);
            self.files.upload_soon(&e.att, remote);
            saved.push((e.task, e.att.file.clone()));
            let p = self.files.pending_for(e.att);
            self.task_files.push((e.task, p));
        }
        self.task_files_saved = Some(saved);
    }

    /// Write down the task attachments on their way when they changed
    /// (only ones copied into the cache; this workspace's part of the file).
    pub(super) fn save_task_files(&mut self) {
        let Some(saved) = &self.task_files_saved else {
            return;
        };
        let now: Vec<(u64, Attachment)> = self
            .task_files
            .iter()
            .filter_map(|(t, p)| p.att.clone().map(|a| (*t, a)))
            .collect();
        let keys: Vec<(u64, String)> = now.iter().map(|(t, a)| (*t, a.file.clone())).collect();
        if *saved == keys {
            return;
        }
        let dir = self.files.dir().to_path_buf();
        let mut all: Vec<TaskFileEntry> = read_task_files(&dir)
            .into_iter()
            .filter(|e| e.ws != self.ws_file)
            .collect();
        all.extend(now.into_iter().map(|(task, att)| TaskFileEntry {
            ws: self.ws_file.clone(),
            task,
            att,
        }));
        write_task_files(&dir, &all);
        self.task_files_saved = Some(keys);
    }

    /// Once a frame: task attachments that are ready join their task.
    pub(super) fn attach_ready_task_files(&mut self) {
        let files = &self.files;
        let mut done = vec![];
        self.task_files.retain(|(tid, p)| {
            if p.ready(files) {
                if let Some(a) = p.att.clone() {
                    done.push((*tid, a));
                }
                false
            } else {
                true
            }
        });
        let me = self.data.me;
        for (tid, a) in done {
            if let Some(t) = self.data.tasks.iter_mut().find(|t| t.id == tid) {
                if !t.attachments.iter().any(|x| x.file == a.file) {
                    t.log(me, format!("attached {}", a.name));
                    t.attachments.push(a);
                    t.updated = now_ts();
                    self.dirty = true;
                }
            }
        }
    }

    /// The Attachments section: tiles (click to open), files on their way,
    /// and an Attach button. Files can also be dropped on the window.
    pub(super) fn attachments_section(&mut self, ui: &mut Ui, t: &Theme, task: &mut Task) {
        let tid = task.id;
        let mut attach = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Attachments")).strong());
            if !task.attachments.is_empty() {
                ui.label(RichText::new(task.attachments.len().to_string()).color(t.text3));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let b = super::icons::icon_button_ex(
                    ui,
                    t,
                    "attach",
                    tr("Attach"),
                    "Attach file",
                    false,
                    0.0,
                    t.text2,
                    t.text2,
                );
                if b.on_hover_text(tr("Attach files (or drop them here)"))
                    .clicked()
                {
                    attach = true;
                }
            });
        });
        if attach {
            for f in media::pick_files() {
                self.add_task_file(tid, f);
            }
        }
        let mut remove = None;
        let mut open = None;
        if !task.attachments.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for (i, a) in task.attachments.iter().enumerate() {
                    let size = vec2(120.0, 90.0);
                    let (rect, resp) =
                        ui.allocate_exact_size(size + vec2(0.0, 18.0), Sense::click());
                    name_it(&resp, &format!("Attachment {}", a.name));
                    let pic = Rect::from_min_size(rect.min, size);
                    // a video with a still shows the still (the video is
                    // fetched when it's opened)
                    let shown = files::shown(a);
                    self.files.want(&shown, files::THUMB);
                    match self.files.texture(&shown.file, files::THUMB) {
                        _ if a.kind == MediaKind::File => {
                            files::paint_file_tile(ui.painter(), pic, &a.name, t);
                        }
                        Some(tex) => {
                            let uv = files::cover_uv(tex.size_vec2(), size);
                            ui.painter().add(
                                egui::epaint::RectShape::filled(pic, 8.0, Color32::WHITE)
                                    .with_texture(tex.id(), uv),
                            );
                        }
                        None => {
                            ui.painter().rect_filled(pic, 8.0, t.hover);
                            let kind = if a.kind == MediaKind::Video {
                                "play"
                            } else {
                                "shapes"
                            };
                            super::icons::line_icon_scaled(
                                ui.painter(),
                                kind,
                                pic.center(),
                                t.text3,
                                1.4,
                            );
                        }
                    }
                    // a picture or video stays as it is under the pointer
                    if a.kind == MediaKind::File {
                        hover_bg(ui, &resp, pic, 8.0);
                    }
                    ui.painter().with_clip_rect(rect).text(
                        pos2(rect.left(), pic.bottom() + 9.0),
                        Align2::LEFT_CENTER,
                        &a.name,
                        FontId::proportional(11.5),
                        t.text2,
                    );
                    let hover = ui.rect_contains_pointer(rect);
                    let x = ui.put(
                        Rect::from_center_size(
                            pic.right_top() + vec2(-12.0, 12.0),
                            vec2(20.0, 20.0),
                        ),
                        egui::Button::new(RichText::new("\u{00D7}").size(10.0).color(if hover {
                            Color32::WHITE
                        } else {
                            Color32::TRANSPARENT
                        }))
                        .fill(if hover {
                            Color32::from_black_alpha(150)
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(10)
                        .small(),
                    );
                    name_it(&x, &format!("Remove attachment {}", a.name));
                    if x.on_hover_text(tr("Remove")).clicked() {
                        remove = Some(i);
                    } else if resp.clicked() {
                        open = Some(a.clone());
                    }
                }
            });
        }
        if let Some(i) = remove {
            task.attachments.remove(i);
        }
        if let Some(a) = open.as_ref().filter(|a| !files::opens_as_media(a)) {
            // a plain file is saved to Downloads, never opened from here (it
            // may be a program, even one marked as a picture)
            self.save_file(a);
        } else if let Some(a) = open {
            // the picture or video opens in the system's own viewer
            if let Some(p) = self.files.playable(&a) {
                if !cfg!(test) {
                    crate::os::open_path(p);
                }
            } else {
                self.toast(trf!("{} is still downloading", a.name));
            }
        }
        // files on their way: getting ready, going up, or why not (with
        // Try again; the backlog also tries again by itself, waiting longer
        // each time, and sends the ones waiting for a sign-in once there is one)
        let mut dismiss = None;
        let mut retry = None;
        for (i, (ftid, p)) in self.task_files.iter().enumerate() {
            if *ftid != tid {
                continue;
            }
            let state = p.transfer(&self.files);
            ui.horizontal(|ui| match &state {
                Some(Transfer::Failed(e)) => {
                    ui.label(
                        RichText::new(format!("{}: {}", p.name, App::error_text(e)))
                            .small()
                            .color(if e == files::NOT_SIGNED_IN {
                                t.text2
                            } else {
                                t.danger
                            }),
                    );
                    if p.error.is_none() && e != files::NOT_SIGNED_IN {
                        let r = ui.add(ghost(egui::Button::new(
                            RichText::new(tr("Try again")).small(),
                        )));
                        name_it(&r, &format!("Try again {}", p.name));
                        if r.clicked() {
                            retry = p.att.clone();
                        }
                    }
                    let x = ui.add(ghost(egui::Button::new(
                        RichText::new("\u{00D7}").size(10.0).color(t.text3),
                    )));
                    name_it(&x, &format!("Dismiss {}", p.name));
                    if x.clicked() {
                        dismiss = Some(i);
                    }
                }
                Some(Transfer::Up(f)) => {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.label(
                        RichText::new(trf!("Uploading {} {}%", p.name, (f * 100.0) as u32))
                            .small()
                            .color(t.text2),
                    );
                }
                _ => {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.label(
                        RichText::new(trf!("Adding {}...", p.name))
                            .small()
                            .color(t.text2),
                    );
                }
            });
        }
        if let Some(a) = retry {
            let remote = self.media_remote();
            for part in files::parts(&a) {
                self.files.retry(&part.file);
            }
            self.files.upload(&a, &remote);
            self.files.upload_soon(&a, &remote);
        }
        if let Some(i) = dismiss {
            let (_, p) = self.task_files.remove(i);
            if let Some(a) = p.att {
                if !self.file_in_use(&a.file) {
                    self.files.forget(&a);
                }
            }
        }
    }
}

/// In the cache folder: task attachments on their way, for every workspace
/// on this computer (each person's own; never shared).
pub(super) const TASK_FILES: &str = ".task-files.json";

/// One task attachment on its way, as written down.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub(super) struct TaskFileEntry {
    /// the workspace's file name (`App::ws_file`)
    pub ws: String,
    pub task: u64,
    pub att: Attachment,
}

pub(super) fn read_task_files(dir: &std::path::Path) -> Vec<TaskFileEntry> {
    std::fs::read_to_string(dir.join(TASK_FILES))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_task_files(dir: &std::path::Path, list: &[TaskFileEntry]) {
    let path = dir.join(TASK_FILES);
    if list.is_empty() {
        let _ = std::fs::remove_file(path);
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    if let Ok(s) = serde_json::to_string(list) {
        let tmp = dir.join(format!(".task-files-{:x}", wb_sync::random_u64()));
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

/// How many tasks the task picker lists before "N more not shown".
pub(super) const PICK_ROWS: usize = 200;
