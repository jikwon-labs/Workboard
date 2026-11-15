// Layout checks with long names, many items and the smallest window
// (900x560): everything stays reachable and nothing is cut mid-glyph or
// drawn over something else.

use super::*;

impl Ui1 {
    /// Painted texts inside `area` that their clip rect cuts on the left or
    /// right (cut mid-glyph instead of shortened with an ellipsis).
    fn cut_texts(&self, area: Rect) -> Vec<String> {
        fn walk(shape: &egui::Shape, clip: Rect, area: Rect, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, area, out)),
                egui::Shape::Text(ts) => {
                    let r = ts.visual_bounding_rect();
                    let text = ts.galley.text().trim();
                    if text.is_empty() || !area.contains(r.center()) {
                        return;
                    }
                    // scrolled out of view vertically is fine
                    if r.center().y < clip.top() || r.center().y > clip.bottom() {
                        return;
                    }
                    if r.right() > clip.right() + 1.0 || r.left() < clip.left() - 1.0 {
                        out.push(text.to_string());
                    }
                }
                _ => {}
            }
        }
        let mut out = vec![];
        for s in &self.shapes {
            walk(&s.shape, s.clip_rect, area, &mut out);
        }
        out
    }

    /// Controls on screen (vertically) that reach past the window's right
    /// edge.
    fn past_right_edge(&self) -> Vec<String> {
        let w = self.size.x;
        let h = self.size.y;
        let mut out: Vec<String> = self
            .tree
            .iter()
            .filter(|(_, r)| r.top() >= 0.0 && r.bottom() <= h && r.right() > w + 1.0)
            .map(|(n, _)| n.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Every painted text shape with its rect (inside its clip rect).
    fn texts_in(&self, area: Rect) -> Vec<(String, Rect)> {
        fn walk(shape: &egui::Shape, clip: Rect, area: Rect, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, area, out)),
                egui::Shape::Text(ts) => {
                    let r = ts.visual_bounding_rect().intersect(clip);
                    let text = ts.galley.text().trim();
                    if !text.is_empty() && r.is_positive() && area.contains(r.center()) {
                        out.push((text.to_string(), r));
                    }
                }
                _ => {}
            }
        }
        let mut out = vec![];
        for s in &self.shapes {
            walk(&s.shape, s.clip_rect, area, &mut out);
        }
        out
    }
}

/// Pairs of painted texts in `area` whose visible rects overlap.
fn overlapping(texts: &[(String, Rect)]) -> Vec<(String, String)> {
    let mut out = vec![];
    for (i, (a, ra)) in texts.iter().enumerate() {
        for (b, rb) in &texts[i + 1..] {
            let o = ra.intersect(*rb);
            if o.width() > 1.0 && o.height() > 1.0 {
                out.push((a.clone(), b.clone()));
            }
        }
    }
    out
}

#[test]
fn small_window_settings_reach_every_section_and_its_controls() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.app.open_settings("General");
    h.settle();
    for (_, label) in super::settings::SETTINGS {
        h.click(&format!("Settings {}", label));
        assert_eq!(
            super::settings::SETTINGS[h.app.settings_tab as usize].1,
            label,
            "the {} section opens",
            label
        );
        let past = h.past_right_edge();
        assert!(
            past.is_empty(),
            "{}: past the right edge: {:?}",
            label,
            past
        );
        let body = Rect::from_min_max(pos2(300.0, 40.0), h.size.to_pos2());
        let cut = h.cut_texts(body);
        assert!(cut.is_empty(), "{}: cut texts: {:?}", label, cut);
    }
}

#[test]
fn small_window_settings_people_columns_do_not_overlap() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.app.data.members.push(Member {
        id: 777,
        name: "Alexandria Montgomery-Featherstonehaugh".into(),
        color: [120, 80, 200],
        email: "alexandria.montgomery@example-company.com".into(),
        role: "Admin".into(),
        invited: true,
        status_text: String::new(),
    });
    h.app.open_settings("People");
    h.settle();
    let body = Rect::from_min_max(pos2(300.0, 40.0), h.size.to_pos2());
    let texts = h.texts_in(body);
    let over = overlapping(&texts);
    assert!(over.is_empty(), "overlapping texts: {:?}", over);
    // the role picker stays left of the status column
    let status = h.painted_text(tr("Invited"));
    assert!(!status.is_empty());
    for (n, r) in &h.tree {
        if n == tr("Admin") && (r.center().y - status[0].0.center().y).abs() < 20.0 {
            assert!(
                r.right() <= status[0].0.left(),
                "role picker {:?} covers the status {:?}",
                r,
                status[0].0
            );
        }
    }
    assert!(h.past_right_edge().is_empty());
}

const LONG_LIST: &str =
    "A very long list name that is wider than the sidebar and any column it shows in";
const LONG_TASK: &str =
    "A very long task name that goes on and on to see how every screen shortens it at the end";

/// The seed plus what the audit's heavy state had: a list with a long name,
/// 35 overdue tasks of mine and one long-named task on that list due today.
fn heavy(h: &mut Ui1) -> u64 {
    let me = h.app.data.me;
    let long = h.app.data.new_id();
    h.app.data.spaces[0].lists.push(ListItem {
        id: long,
        name: LONG_LIST.into(),
        color: None,
        start: None,
        end: None,
        priority: 0,
        owner: None,
        description: String::new(),
        archived: false,
    });
    let base = task_named(h, "Set up analytics dashboard").clone();
    let td = today();
    for k in 0..35 {
        let mut x = base.clone();
        x.id = 20_000 + k;
        x.name = format!("Overdue chore {}", k);
        x.assignees = vec![me];
        x.start = None;
        x.due = Some(td - Duration::days(1 + (k as i64 * 7) % 20));
        x.parent = None;
        h.app.data.tasks.push(x);
    }
    let mut x = base.clone();
    x.id = 20_100;
    x.name = LONG_TASK.into();
    x.list_id = long;
    x.assignees = vec![me];
    x.start = None;
    x.due = Some(td);
    x.parent = None;
    h.app.data.tasks.push(x);
    h.settle();
    long
}

fn planner_days_share_the_width(w: f32, hh: f32) {
    let mut h = Ui1::sized(w, hh);
    heavy(&mut h);
    h.app.section = Section::Planner;
    h.settle();
    let week = h.app.planner_week;
    let adds: Vec<Rect> = (0..7)
        .map(|i| {
            h.rect(&format!(
                "Add task on {}",
                (week + Duration::days(i)).format("%a %-d")
            ))
        })
        .collect();
    // every day is inside the window and they are evenly spaced
    for (i, r) in adds.iter().enumerate() {
        assert!(
            r.right() <= w,
            "{}x{}: day {} ends at {}",
            w,
            hh,
            i,
            r.right()
        );
    }
    let steps: Vec<f32> = adds.windows(2).map(|p| p[1].left() - p[0].left()).collect();
    for s in &steps {
        assert!(
            (s - steps[0]).abs() < 1.5,
            "{}x{}: day columns differ: {:?}",
            w,
            hh,
            steps
        );
    }
    // and they use the width: no wide empty strip on the right
    let last = adds[6].left() + steps[0];
    assert!(w - last < 60.0, "{}x{}: {} px left empty", w, hh, w - last);
    let page = Rect::from_min_max(pos2(60.0, 40.0), h.size.to_pos2());
    let cut = h.cut_texts(page);
    assert!(cut.is_empty(), "{}x{}: cut texts {:?}", w, hh, cut);
    // today's 36 tasks scroll, so the "+ Add task" under them is reachable
    let add_today = format!("Add task on {}", today().format("%a %-d"));
    if week <= today() && today() < week + Duration::days(7) {
        h.click(&add_today);
        assert!(
            h.app.sweep.planner_add.is_some(),
            "{}x{}: the add row under today's tasks is reachable",
            w,
            hh
        );
    }
}

#[test]
fn planner_days_share_the_width_and_today_scrolls() {
    planner_days_share_the_width(1440.0, 900.0);
    planner_days_share_the_width(900.0, 560.0);
}

#[test]
fn planner_without_unscheduled_tasks_says_so() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let td = today();
    for x in h.app.data.tasks.iter_mut() {
        if x.assignees.contains(&me) && x.due.is_none() {
            x.due = Some(td);
        }
    }
    h.app.section = Section::Planner;
    h.settle();
    assert!(!h
        .painted_text(tr(
            "Nothing to schedule. Every task of yours has a day, or drag one here to take its date off."
        ))
        .is_empty());
}
