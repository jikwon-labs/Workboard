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
                    // where it shows decides whether it is in the area
                    if text.is_empty() || !area.contains(r.intersect(clip).center()) {
                        return;
                    }
                    // scrolled out of view is fine; partly in view is cut
                    if r.center().y < clip.top()
                        || r.center().y > clip.bottom()
                        || r.right() <= clip.left()
                        || r.left() >= clip.right()
                    {
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
            // the same text drawn twice a hair apart is a bold label
            if a == b && ra.center().distance(rb.center()) < 2.0 {
                continue;
            }
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
        // top and bottom of the section: nothing past the body's right
        // margin, no text cut off
        for pass in 0..2 {
            if pass == 1 {
                for _ in 0..8 {
                    h.frame(vec![egui::Event::PointerMoved(pos2(600.0, 300.0))]);
                    h.frame(vec![egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, -400.0),
                        modifiers: Default::default(),
                    }]);
                }
                h.settle();
            }
            let edge = h.size.x - 24.0;
            let past: Vec<&String> = h
                .tree
                .iter()
                .filter(|(_, r)| {
                    r.left() > 300.0
                        && r.top() >= 40.0
                        && r.bottom() <= h.size.y
                        && r.right() > edge
                })
                .map(|(n, _)| n)
                .collect();
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
        slack: vec![],
        slack_mine: vec![],
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

fn home_fits(w: f32, hh: f32) {
    let mut h = Ui1::sized(w, hh);
    heavy(&mut h);
    h.app.actions.push(Action::Go(Location::Home));
    h.settle();
    let page = Rect::from_min_max(pos2(60.0, 40.0), h.size.to_pos2());
    let over = overlapping(&h.texts_in(page));
    assert!(over.is_empty(), "{}x{}: overlapping {:?}", w, hh, over);
    let cut = h.cut_texts(page);
    assert!(cut.is_empty(), "{}x{}: cut {:?}", w, hh, cut);
    // the Lists card is not cut on the right: its long list is inside
    h.scroll_into_view("Home show all Next");
    let past = h.past_right_edge();
    assert!(
        past.is_empty(),
        "{}x{}: past the right edge {:?}",
        w,
        hh,
        past
    );
}

#[test]
fn home_cards_fit_and_long_names_do_not_overlap() {
    home_fits(1440.0, 900.0);
    home_fits(900.0, 560.0);
}

#[test]
fn home_overdue_is_by_date_and_capped_with_show_all() {
    let mut h = Ui1::new();
    heavy(&mut h);
    h.app.actions.push(Action::Go(Location::Home));
    h.settle();
    let me = h.app.data.me;
    let mut overdue: Vec<&Task> = h
        .app
        .data
        .tasks
        .iter()
        .filter(|x| x.assignees.contains(&me) && x.is_overdue() && !h.app.data.is_closed(x))
        .collect();
    overdue.sort_by_key(|x| x.due_moment());
    let n = overdue.len();
    assert!(n > HOME_ROWS);
    let first = overdue[0].name.clone();
    let hidden = overdue[HOME_ROWS].name.clone();
    // the oldest first, the rest behind "Show all"
    let y = |h: &Ui1, name: &str| h.rect(&format!("Complete {}", name)).center().y;
    assert!(y(&h, &first) < y(&h, &overdue[1].name.clone()));
    assert!(!h.has(&format!("Complete {}", hidden)));
    assert!(!h.painted_text(&trf!("Show all {}", n)).is_empty());
    h.click("Home show all Overdue");
    assert!(h.has(&format!("Complete {}", hidden)));
    h.click("Home show all Overdue");
    assert!(!h.has(&format!("Complete {}", hidden)));
}

#[test]
fn gantt_marks_bars_out_of_sight_and_shortens_names() {
    let mut h = Ui1::new();
    let td = today();
    let base = task_named(&h, "Write copy for landing page").clone();
    let sc = super::timebar::DayScale::new(0, td);
    // ended a week before the first day drawn
    let ended = sc.start - Duration::days(7);
    let mut x = base.clone();
    x.id = 30_000;
    x.name = LONG_TASK.into();
    x.start = None;
    x.due = Some(ended);
    h.app.data.tasks.push(x);
    let mut late = base.clone();
    late.id = 30_001;
    late.name = "Late one".into();
    late.start = None;
    late.due = Some(td - Duration::days(1));
    h.app.data.tasks.push(late);
    h.click("View Gantt");
    let names = h.painted_text(tr("Name"))[0].0.left() - 8.0;
    let timeline_left = names + 260.0;
    // the row says the bar is to the left and when it ended, inside the timeline
    let mark = h.painted_text(&format!("\u{2039} {}", fmt_date(ended)));
    assert_eq!(mark.len(), 1, "one marker for the bar out of sight");
    assert!(mark[0].0.left() >= timeline_left, "{:?}", mark[0].0);
    // the full long name is not drawn anywhere: the name column shortens it
    assert!(h.painted_text(LONG_TASK).is_empty());
    let column = Rect::from_min_max(pos2(names, 0.0), pos2(timeline_left, h.size.y));
    let cut = h.cut_texts(column);
    assert!(cut.is_empty(), "cut names {:?}", cut);
    // an overdue bar is red
    let bar = h.rect("Bar Late one");
    let t = h.app.t();
    let red = h.shapes.iter().any(|s| match &s.shape {
        egui::Shape::Rect(r) => {
            r.rect.center().distance(bar.center()) < 2.0
                && (r.fill == t.danger || r.fill == t.danger.gamma_multiply(0.85))
        }
        _ => false,
    });
    assert!(red, "the overdue bar is drawn red");
}

#[test]
fn gantt_without_tasks_says_how_to_start() {
    let mut h = Ui1::new();
    let list = task_named(&h, "Write copy for landing page").list_id;
    h.app.data.tasks.retain(|x| x.list_id != list);
    h.click("View Gantt");
    assert!(!h
        .painted_text(tr(
            "No tasks here yet. Add one, then click or drag across the days in its row to give it dates."
        ))
        .is_empty());
}

/// Monday of the current week.
fn this_monday() -> NaiveDate {
    let td = today();
    td - Duration::days(td.weekday().num_days_from_monday() as i64)
}

#[test]
fn calendar_multi_day_bar_keeps_one_line() {
    let mut h = Ui1::new();
    let base = task_named(&h, "Write copy for landing page").clone();
    for x in h.app.data.tasks.iter_mut() {
        x.start = None;
        x.due = None;
    }
    let mon = this_monday();
    // Mon-Wed, and a longer one that starts on Tuesday: it used to take the
    // top line from Tuesday on and push the first bar down
    for (id, name, s, e) in [
        (31_000, "Three days", 0, 2),
        (31_001, "Five days from Tuesday", 1, 5),
        (31_002, "Single Tuesday", 1, 1),
    ] {
        let mut x = base.clone();
        x.id = id;
        x.name = name.into();
        x.start = Some(mon + Duration::days(s));
        x.due = Some(mon + Duration::days(e));
        x.parent = None;
        h.app.data.tasks.push(x);
    }
    h.click("View Calendar");
    h.app.cal_week = true;
    // the week of that Monday (weeks start on Sunday)
    h.ctx
        .data_mut(|d| d.insert_temp(egui::Id::new("cal_week_anchor"), mon));
    h.settle();
    let y = |h: &Ui1, d: i64| {
        h.rect(&format!("Calendar Three days {}", mon + Duration::days(d)))
            .center()
            .y
    };
    assert!((y(&h, 0) - y(&h, 1)).abs() < 1.0, "same line on day 2");
    assert!((y(&h, 0) - y(&h, 2)).abs() < 1.0, "same line on day 3");
}

#[test]
fn calendar_more_line_fits_inside_the_day() {
    let mut h = Ui1::new();
    let base = task_named(&h, "Write copy for landing page").clone();
    let td = today();
    // a day in the month's second row, so the third row is below it
    let day = NaiveDate::from_ymd_opt(td.year(), td.month(), 1).unwrap() + Duration::days(7);
    for k in 0..14 {
        let mut x = base.clone();
        x.id = 32_000 + k;
        x.name = format!("Busy {}", k);
        x.start = None;
        x.due = Some(day);
        x.parent = None;
        h.app.data.tasks.push(x);
    }
    h.click("View Calendar");
    let more = h.rect(&format!("More tasks on {}", day));
    // the next week's day number, right under this day
    let next = (day + Duration::days(7)).day().to_string();
    let below = h
        .painted_text(&next)
        .into_iter()
        .map(|(r, _)| r)
        .find(|r| {
            r.left() >= more.left() - 8.0 && r.left() < more.left() + 30.0 && r.top() > more.top()
        })
        .expect("the next week's day number");
    // that number sits 12 px under its cell's top
    let cell_bottom = below.center().y - 12.0;
    assert!(
        more.bottom() <= cell_bottom + 0.5,
        "the more line {:?} ends inside the day (bottom {})",
        more,
        cell_bottom
    );
}

#[test]
fn board_closed_column_says_how_many_are_hidden() {
    let mut h = Ui1::new();
    h.click("View Board");
    assert!(!h.app.show_closed);
    // the seed's list has closed tasks; count them in its done status
    let list = task_named(&h, "Write copy for landing page").list_id;
    let closed: Vec<&Task> = h
        .app
        .data
        .tasks
        .iter()
        .filter(|x| {
            x.list_id == list && x.parent.is_none() && !x.archived && h.app.data.is_closed(x)
        })
        .collect();
    assert!(!closed.is_empty(), "the seed has a closed task");
    let st = h.app.data.status_of(closed[0]).name.clone();
    let n = closed
        .iter()
        .filter(|x| h.app.data.status_of(x).name == st)
        .count();
    let first = closed[0].name.clone();
    assert!(
        !h.painted_text(&trf!("{} hidden", n)).is_empty(),
        "{} hidden",
        n
    );
    let label = crate::i18n::status_text(&st).to_uppercase();
    h.click(&format!("Show closed in {}", label));
    assert!(h.app.show_closed);
    assert!(h.has(&format!("Card {}", first)));
}

#[test]
fn task_picker_says_how_many_it_does_not_list() {
    let mut h = Ui1::new();
    let base = task_named(&h, "Write copy for landing page").clone();
    for k in 0..250 {
        let mut x = base.clone();
        x.id = 40_000 + k;
        x.name = format!("Filler {}", k);
        x.parent = None;
        h.app.data.tasks.push(x);
    }
    let tid = base.id;
    let total = h.app.pick_candidates(tid, TaskPick::Linked).len();
    assert!(total > super::super::taskfields::PICK_ROWS);
    h.app.modal = Some(Modal::PickTask {
        task: tid,
        mode: TaskPick::Linked,
        query: String::new(),
    });
    h.settle();
    let more = total - super::super::taskfields::PICK_ROWS;
    assert!(
        !h.painted_text(&trf!("{} more not shown. Search to find them.", more))
            .is_empty(),
        "{} more",
        more
    );
}

#[test]
fn list_rows_show_tags_that_fit_and_count_the_rest() {
    let mut h = Ui1::new();
    let tags: Vec<String> = vec![
        "release-blocker-for-launch".into(),
        "bug".into(),
        "design".into(),
    ];
    let base = task_named(&h, "Write copy for landing page").clone();
    let mut short = base.clone();
    short.id = 50_000;
    short.name = "Short".into();
    short.tags = tags.clone();
    short.parent = None;
    h.app.data.tasks.push(short);
    let mut long = base.clone();
    long.id = 50_001;
    long.name = LONG_TASK.into();
    long.tags = tags.clone();
    long.parent = None;
    h.app.data.tasks.push(long);
    h.settle();
    let row = |h: &Ui1, name: &str| h.rect(&format!("Task {}", name));
    let in_row = |h: &Ui1, name: &str, text: &str| {
        let r = row(h, name);
        h.painted_text(text)
            .iter()
            .any(|(t, _)| r.contains(t.center()))
    };
    // room enough: all three
    assert!(in_row(&h, "Short", "bug") && in_row(&h, "Short", "design"));
    // a long name keeps room for the first tag and a count of the others
    assert!(in_row(&h, LONG_TASK, "+2") || in_row(&h, LONG_TASK, "+3"));
    let page = Rect::from_min_max(pos2(320.0, 40.0), h.size.to_pos2());
    let over = overlapping(&h.texts_in(page));
    assert!(over.is_empty(), "overlapping {:?}", over);
}

#[test]
fn dashboard_cards_line_up_and_overdue_is_capped() {
    let mut h = Ui1::new();
    heavy(&mut h);
    // a long name among the overdue ones
    if let Some(x) = h.app.data.tasks.iter_mut().find(|x| x.id == 20_000) {
        x.name = LONG_TASK.into();
    }
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Big board");
    h.settle();
    // the right column ends where the full-width cards end
    let full = h.rect("Remove card Overdue tasks").right();
    let right = h.rect("Remove card Tasks by priority").right();
    assert!(
        (full - right).abs() < 1.0,
        "full {} vs right column {}",
        full,
        right
    );
    h.scroll_into_view("Remove card Overdue tasks");
    let me = h.app.data.me;
    let n = h
        .app
        .data
        .tasks
        .iter()
        .filter(|x| {
            x.parent.is_none()
                && x.is_overdue()
                && !h.app.data.is_closed(x)
                && h.app.data.task_live(x)
        })
        .count();
    assert!(n > 10, "{} overdue for {}", n, me);
    assert!(
        h.painted_text(LONG_TASK).is_empty(),
        "the long name is shortened"
    );
    for _ in 0..10 {
        if !h.painted_text(&trf!("+{} more", n - 10)).is_empty() {
            break;
        }
        let at = pos2(800.0, 450.0);
        h.frame(vec![egui::Event::PointerMoved(at)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -300.0),
            modifiers: Default::default(),
        }]);
        h.settle();
    }
    assert!(!h.painted_text(&trf!("+{} more", n - 10)).is_empty());
}

#[test]
fn team_page_counts_open_tasks_it_does_not_list() {
    let mut h = Ui1::new();
    heavy(&mut h);
    h.app.section = Section::Teams;
    h.settle();
    h.click_side("Create team", true);
    let id = h.app.team_open.unwrap();
    let me = h.app.data.me;
    let team = h
        .app
        .data
        .extra
        .teams
        .iter_mut()
        .find(|x| x.id == id)
        .unwrap();
    if !team.members.contains(&me) {
        team.members.push(me);
    }
    h.settle();
    let open = h
        .app
        .data
        .tasks
        .iter()
        .filter(|x| {
            x.assignees.contains(&me) && !h.app.data.is_closed(x) && h.app.data.task_live(x)
        })
        .count();
    assert!(open > 6);
    let more = format!("      {}", trf!("+{} more", open - 6));
    assert!(!h.painted_text(&more).is_empty(), "{:?}", more);
}

#[test]
fn org_chart_cards_in_a_row_line_up() {
    let mut h = Ui1::new();
    h.app.section = Section::Teams;
    h.app.hub_filter = 2;
    h.settle();
    let sam = h.painted_text("Sam Lee");
    let jordan = h.painted_text("Jordan Park");
    assert!(!sam.is_empty() && !jordan.is_empty());
    assert!(
        (sam[0].0.bottom() - jordan[0].0.bottom()).abs() < 1.0,
        "Sam {:?} and Jordan {:?} on one line",
        sam[0].0,
        jordan[0].0
    );
}

#[test]
fn sidebar_shortens_long_names_with_an_ellipsis() {
    let mut h = Ui1::sized(1440.0, 1200.0);
    heavy(&mut h);
    h.app.data.workspace = "A workspace whose name is far too long for the sidebar row".into();
    h.settle();
    let side = h.rect("Team Space");
    let area = Rect::from_min_max(pos2(side.left(), 40.0), pos2(side.right(), h.size.y));
    let cut = h.cut_texts(area);
    assert!(cut.is_empty(), "cut in the sidebar: {:?}", cut);
    assert!(h.has(LONG_LIST), "the long list is in the sidebar");
    assert!(
        h.painted_text(LONG_LIST).is_empty(),
        "not drawn past the row"
    );
    let r = h.rect(LONG_LIST);
    assert!(
        h.texts_in(r).iter().any(|(t, _)| t.ends_with('\u{2026}')
            && LONG_LIST.starts_with(t.trim_end_matches('\u{2026}'))),
        "shortened with an ellipsis"
    );
}

#[test]
fn table_fits_its_columns_and_shortens_names_and_tags() {
    let mut h = Ui1::new();
    let base = task_named(&h, "Write copy for landing page").clone();
    let mut long = base.clone();
    long.id = 60_000;
    long.name = LONG_TASK.into();
    long.tags = vec![
        "release-blocker-for-launch".into(),
        "bug".into(),
        "design".into(),
    ];
    long.parent = None;
    h.app.data.tasks.push(long);
    h.click("View Table");
    let created = h.rect("Sort by Date created");
    assert!(
        created.right() <= h.size.x - 6.0,
        "Created {:?} fits the window",
        created
    );
    let page = Rect::from_min_max(pos2(320.0, 120.0), h.size.to_pos2());
    let cut = h.cut_texts(page);
    assert!(cut.is_empty(), "cut in the table: {:?}", cut);
    let row_y = h.rect(&format!("Rename {}", LONG_TASK)).center().y;
    assert!(
        h.painted_text("+2")
            .iter()
            .chain(h.painted_text("+1").iter())
            .chain(h.painted_text("+3").iter())
            .any(|(r, _)| (r.center().y - row_y).abs() < 10.0),
        "the tags that don't fit are counted"
    );
}

#[test]
fn search_shortens_a_long_path_before_its_kind() {
    let mut h = Ui1::new();
    heavy(&mut h);
    h.app.palette = Some("very long list".into());
    h.settle();
    let path = format!("Team Space / {}", LONG_LIST);
    assert!(h.has(&path), "the long list is found");
    let row = h.rect(&path);
    let texts = h.texts_in(row);
    assert!(
        texts
            .iter()
            .any(|(t, _)| t.ends_with('\u{2026}')
                && path.starts_with(t.trim_end_matches('\u{2026}'))),
        "the path is shortened with an ellipsis: {:?}",
        texts
    );
    let over = overlapping(&texts);
    assert!(over.is_empty(), "{:?}", over);
    let cut = h.cut_texts(row);
    assert!(cut.is_empty(), "{:?}", cut);
    // a gap between the path and the grey kind after it
    let title = texts
        .iter()
        .find(|(t, _)| t.ends_with('\u{2026}'))
        .unwrap()
        .1;
    let kind = texts
        .iter()
        .find(|(t, _)| t == tr("List"))
        .expect("the kind")
        .1;
    assert!(
        kind.left() - title.right() >= 8.0,
        "{:?} then {:?}",
        title,
        kind
    );
}

fn chip_clear(h: &Ui1) {
    let chip = h.rect(tr("Add bookmarks, notes, and more"));
    let page = Rect::from_min_max(pos2(chip.left() - 20.0, chip.top()), h.size.to_pos2());
    // the messages are cut a little below the chip, not right at its edge
    let below = chip.expand2(vec2(0.0, 4.0));
    for (t, r) in h.texts_in(page) {
        if t != tr("Add bookmarks, notes, and more") && r.intersects(below) {
            panic!("{:?} at {:?} under the chip {:?}", t, r, chip);
        }
    }
}

#[test]
fn small_window_chat_chip_stays_above_the_messages() {
    let mut h = Ui1::sized(900.0, 560.0);
    // enough messages to scroll
    let ch = h
        .app
        .data
        .channels
        .iter()
        .position(|c| c.name == "general")
        .unwrap();
    let first = h.app.data.channels[ch].messages[0].clone();
    for k in 0..12 {
        let mut m = first.clone();
        m.id = 70_000 + k;
        m.text = format!("Message number {} with a few words in it", k);
        m.replies.clear();
        h.app.data.channels[ch].messages.push(m);
    }
    h.click("general");
    // at the newest message, then a little way up, then at the top
    chip_clear(&h);
    for _ in 0..12 {
        let at = pos2(500.0, 250.0);
        h.frame(vec![egui::Event::PointerMoved(at)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, 7.0),
            modifiers: Default::default(),
        }]);
        h.settle();
        chip_clear(&h);
    }
    // the right panel gives the messages room in a small window
    let followers = h.painted_text(tr("Followers"))[0].0;
    assert!(
        h.size.x - followers.left() < 220.0,
        "the panel is narrower: {:?}",
        followers
    );
}

#[test]
fn small_window_doc_template_cards_fit_and_shorten() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.open_docs_list();
    let wiki = h.rect("Template Wiki");
    assert!(wiki.right() <= h.size.x, "the third card {:?} fits", wiki);
    let page = Rect::from_min_max(pos2(60.0, 40.0), h.size.to_pos2());
    let cut = h.cut_texts(page);
    assert!(cut.is_empty(), "cut: {:?}", cut);
}
