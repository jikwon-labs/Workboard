// Layout checks with long names, many items and the smallest window
// (900x560): everything stays reachable and nothing is cut mid-glyph or
// drawn over something else.

use super::*;

impl Ui1 {
    /// Painted texts inside `area` that their clip rect cuts on the left or
    /// right (cut mid-glyph instead of shortened with an ellipsis).
    pub(super) fn cut_texts(&self, area: Rect) -> Vec<String> {
        let mut out = vec![];
        each_text(&self.shapes, |ts, clip| {
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
        });
        out
    }

    /// Controls on screen (vertically) that reach past the window's right
    /// edge.
    pub(super) fn past_right_edge(&self) -> Vec<String> {
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
    pub(super) fn texts_in(&self, area: Rect) -> Vec<(String, Rect)> {
        let mut out = vec![];
        each_text(&self.shapes, |ts, clip| {
            let r = ts.visual_bounding_rect().intersect(clip);
            let text = ts.galley.text().trim();
            if !text.is_empty() && r.is_positive() && area.contains(r.center()) {
                out.push((text.to_string(), r));
            }
        });
        out
    }
}

/// Pairs of painted texts in `area` whose visible rects overlap.
pub(super) fn overlapping(texts: &[(String, Rect)]) -> Vec<(String, String)> {
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
        ..Default::default()
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
pub(super) fn heavy(h: &mut Ui1) -> u64 {
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
    let name_label = h.painted_text(tr("Name"))[0].0;
    let names = name_label.left() - 8.0;
    let timeline_left = names + 260.0;
    // the "Name" label sits in the middle of the 44-point date header
    let rows_top = name_label.center().y + 22.0;
    // the row says the bar is to the left and when it ended, inside the timeline
    let mark = h.painted_text(&format!("\u{2039} {}", fmt_date(ended)));
    assert_eq!(mark.len(), 1, "one marker for the bar out of sight");
    assert!(mark[0].0.left() >= timeline_left, "{:?}", mark[0].0);
    // the full long name is not drawn anywhere: the name column shortens it
    assert!(h.painted_text(LONG_TASK).is_empty());
    // the rows only: a day number in the header scrolled half out of the
    // timeline (which day depends on today) is not a task name
    let column = Rect::from_min_max(pos2(names, rows_top), pos2(timeline_left, h.size.y));
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
    // that number sits DAY_NUM.y under its cell's top
    let cell_bottom = below.center().y - DAY_NUM.y;
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

#[test]
fn search_keeps_a_message_of_several_lines_on_its_row() {
    let mut h = Ui1::new();
    let ch = h
        .app
        .data
        .channels
        .iter()
        .position(|c| c.name == "general")
        .unwrap();
    h.app.data.channels[ch]
        .messages
        .push(crate::app::slack::new_message(
            990_001,
            2,
            "Lunch plan\nPizza at noon\n\n  Bring cash".into(),
            1_900_000_000,
        ));
    h.app.palette = Some("pizza".into());
    h.settle();
    let title = "Lunch plan Pizza at noon Bring cash";
    assert!(h.has(title), "the message is found and named on one line");
    let row = h.rect(title);
    // everything painted around the row stays inside it, on one line
    let near = row.expand2(vec2(0.0, 30.0));
    let texts = h.texts_in(near);
    let ours: Vec<_> = texts
        .iter()
        .filter(|(t, _)| t.contains("Pizza") || t.contains("Lunch"))
        .collect();
    assert_eq!(ours.len(), 1, "{:?}", texts);
    let (t, r) = ours[0];
    assert!(!t.contains('\n'), "{:?}", t);
    assert!(
        r.top() >= row.top() - 1.0 && r.bottom() <= row.bottom() + 1.0,
        "{:?} at {:?} spills out of its row {:?}",
        t,
        r,
        row
    );
    let over = overlapping(&texts);
    assert!(over.is_empty(), "{:?}", over);
}

#[test]
fn search_no_results_has_room_above_and_below() {
    let mut h = Ui1::new();
    h.app.palette = Some("zzqx nothing matches".into());
    h.settle();
    let nr = h
        .painted_text(tr("No results"))
        .first()
        .expect("No results is shown")
        .0;
    // the tabs above it
    let tab = h
        .painted_text(tr("People"))
        .iter()
        .map(|(r, _)| *r)
        .filter(|r| r.bottom() < nr.top())
        .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
        .expect("the People tab");
    assert!(
        nr.top() - tab.bottom() >= 24.0,
        "tabs end at {}, No results starts at {}",
        tab.bottom(),
        nr.top()
    );
    // and it sits in the middle of the palette, not at its left edge
    let all = h
        .painted_text(tr("All"))
        .iter()
        .map(|(r, _)| *r)
        .find(|r| (r.center().y - tab.center().y).abs() < 4.0)
        .expect("the All tab");
    assert!(nr.left() > all.right() + 100.0, "{:?} after {:?}", nr, all);
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
    // the chip sits under the Channel tab with room below the tab line
    let chip = h.rect(tr("Add bookmarks, notes, and more"));
    let tab = h
        .painted_text(tr("Channel"))
        .into_iter()
        .map(|(r, _)| r)
        .filter(|r| r.bottom() < chip.top())
        .max_by(|a, b| a.bottom().total_cmp(&b.bottom()))
        .expect("the Channel tab");
    assert!(
        (chip.left() - (tab.left() - 26.0)).abs() <= 1.0,
        "chip {:?} lines up with the tab {:?}",
        chip,
        tab
    );
    assert!(
        chip.top() - tab.bottom() >= 18.0,
        "chip {:?} clears the tab line under {:?}",
        chip,
        tab
    );
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

// scroll bars (narrow, tall, painted bars) inside a strip of the window
fn scroll_bars_in(h: &Ui1, strip: Rect) -> usize {
    h.shapes
        .iter()
        .filter(|s| match &s.shape {
            egui::Shape::Rect(r) => {
                r.fill.a() > 0
                    && r.rect.width() > 0.5
                    && r.rect.width() <= 12.0
                    && r.rect.height() >= 40.0
                    && strip.contains_rect(r.rect)
            }
            _ => false,
        })
        .count()
}

fn wheel_down(h: &mut Ui1, at: Pos2, strip: Rect, what: &str) {
    for _ in 0..6 {
        h.frame(vec![egui::Event::PointerMoved(at)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -200.0),
            modifiers: Default::default(),
        }]);
        assert_eq!(
            scroll_bars_in(h, strip),
            0,
            "{}: no scroll bar while scrolling",
            what
        );
    }
    h.settle();
    assert_eq!(
        scroll_bars_in(h, strip),
        0,
        "{}: no scroll bar after scrolling",
        what
    );
}

#[test]
fn small_window_sidebar_scrolls_without_a_scroll_bar() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.settle();
    let side = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("sidebar"))
        .expect("the sidebar is open")
        .rect;
    // the "+ New message" row at the bottom of the sidebar list
    let last_row = |h: &Ui1| {
        h.painted_text(&tr("New message"))
            .into_iter()
            .map(|(r, _)| r)
            .filter(|r| side.x_range().contains(r.left()))
            .max_by(|a, b| a.top().total_cmp(&b.top()))
    };
    let start = last_row(&h).expect("the sidebar lists New message");
    assert!(
        start.bottom() > h.size.y - 20.0,
        "the sidebar overflows a small window: {:?}",
        start
    );
    // a scroll bar would sit along the sidebar's right edge
    let strip = side.with_min_x(side.right() - 30.0);
    assert_eq!(scroll_bars_in(&h, strip), 0, "no scroll bar at rest");
    wheel_down(&mut h, pos2(side.center().x, 300.0), strip, "sidebar");
    let end = last_row(&h).expect("New message is still listed");
    assert!(
        end.bottom() <= h.size.y,
        "the wheel brings the bottom row into view: {:?}",
        end
    );
}

#[test]
fn small_window_settings_scroll_without_a_scroll_bar() {
    let mut h = Ui1::sized(900.0, 560.0);
    // the longest section: taller than a small window
    h.app.open_settings("Notifications");
    h.settle();
    let bottom = |h: &Ui1| {
        h.tree
            .iter()
            .filter(|(_, r)| r.left() > 300.0)
            .map(|(_, r)| r.bottom())
            .fold(0.0_f32, f32::max)
    };
    let before = bottom(&h);
    assert!(before > h.size.y, "the section overflows: {}", before);
    // a scroll bar would sit near the window's right edge, inside the
    // section's margin
    let strip = Rect::from_min_max(pos2(h.size.x - 80.0, 40.0), h.size.to_pos2());
    assert_eq!(
        scroll_bars_in(&h, strip),
        0,
        "settings: no scroll bar at rest"
    );
    wheel_down(&mut h, pos2(600.0, 300.0), strip, "settings");
    let after = bottom(&h);
    assert!(
        after < before - 100.0,
        "the wheel scrolls the section: {} -> {}",
        before,
        after
    );
}

#[test]
fn toast_keeps_its_text_on_one_line_after_a_short_one() {
    // the toast's area remembers its last size and offered only that much
    // room, so a long message after a short one wrapped every few letters
    let mut h = Ui1::sized(900.0, 560.0);
    h.app.toast("Copied");
    h.settle();
    let msg = "Connected. This workspace is now on the server for your team.";
    h.app.toast(msg);
    h.settle();
    let painted = h.painted_text(msg);
    assert_eq!(painted.len(), 1, "the toast is drawn once");
    let r = painted[0].0;
    let font = egui::TextStyle::Body.resolve(&h.ctx.style());
    let line = h
        .ctx
        .fonts(|f| f.layout_no_wrap(msg.into(), font, Color32::WHITE).size());
    assert!(
        r.height() < line.y * 1.5,
        "the toast wrapped onto more lines: {r:?} for one line of {line:?}"
    );
}

/// The month in a date menu fills the menu beside the quick options: rows of
/// one height on both sides, the arrows square over the last two columns,
/// text inset inside a hovered row's fill, and Clear level with the last week.
#[test]
fn date_menu_fills_its_width_and_its_rows_line_up() {
    let mut h = Ui1::new();
    open_design_task(&mut h);
    h.click("Subtask due Pick hero image");
    let td = today();
    let first = NaiveDate::from_ymd_opt(td.year(), td.month(), 1).unwrap();
    let start = first - Duration::days(first.weekday().num_days_from_sunday() as i64);
    let day = |h: &Ui1, i: i64| h.rect(&format!("Day {}", start + Duration::days(i)));
    let (today_row, tomorrow) = (h.rect("Today"), h.rect("Tomorrow"));
    // the options and the month share the menu's width and their rows
    assert!(
        (day(&h, 6).right() - today_row.left() - DATE_MENU_W).abs() < 0.5,
        "the month ends at the menu's right edge ({:?} .. {:?})",
        today_row,
        day(&h, 6)
    );
    assert!((tomorrow.top() - today_row.bottom()).abs() < 0.5);
    assert!((today_row.height() - day(&h, 0).height()).abs() < 0.5);
    assert!(
        (h.rect("8 weeks").bottom() - day(&h, 35).top()).abs() < 0.5,
        "the options end where the last week starts"
    );
    // a hovered option's fill reaches past its text on the left
    h.hover(tomorrow.center());
    let fill = crate::widgets::hover_fill_for(false);
    let fill_rect = h
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            egui::Shape::Rect(r) if r.fill == fill && r.rect.contains(tomorrow.center()) => {
                Some(r.rect)
            }
            _ => None,
        })
        .expect("Tomorrow's hover fill");
    let text = h.painted_text("Tomorrow")[0].0;
    assert!(
        text.left() - fill_rect.left() >= 8.0,
        "room before the text inside the fill ({:?} in {:?})",
        text,
        fill_rect
    );
    // the arrows: squares over the last two columns, level with the month
    let (prev, next) = (h.rect("Previous month"), h.rect("Next month"));
    for r in [prev, next] {
        assert!((r.width() - r.height()).abs() < 0.5, "a square: {:?}", r);
    }
    assert!((prev.center().x - day(&h, 5).center().x).abs() < 0.5);
    assert!((next.center().x - day(&h, 6).center().x).abs() < 0.5);
    let title = h.painted_text(&first.format("%B %Y").to_string())[0].0;
    assert!(
        (title.center().y - next.center().y).abs() < 1.5,
        "the month's name {:?} and the arrows {:?} share a line",
        title,
        next
    );
    // with a date set, Clear takes the last row, level with the last week
    h.click("Today");
    h.app.apply_actions();
    h.click("Subtask due Pick hero image");
    let clear = h.rect("Clear");
    assert!((clear.top() - day(&h, 35).top()).abs() < 0.5, "{:?}", clear);
    assert!((clear.bottom() - day(&h, 41).bottom()).abs() < 0.5);
    // the arrows turn the month and a day of it can be picked
    h.click("Next month");
    let next_first = crate::widgets::add_months(first, 1);
    let pick = next_first + Duration::days(9);
    h.click(&format!("Day {}", pick));
    h.app.apply_actions();
    let sub = task_named(&h, "Pick hero image");
    assert_eq!(sub.due, Some(pick));
}

/// In the month view today's number stays in line with the other days (its
/// circle used to push it left) and its circle keeps clear of the day's top
/// line; the weekday names start where the numbers do.
#[test]
fn calendar_today_lines_up_with_the_other_days() {
    let mut h = Ui1::new();
    h.click("View Calendar");
    h.settle();
    let td = today();
    let danger = h.app.t().danger;
    let circles: Vec<egui::epaint::CircleShape> = h
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Circle(c) if c.fill == danger && c.radius > 5.0 => Some(*c),
            _ => None,
        })
        .collect();
    // today's number: white, inside a red circle
    let (num, circle) = h
        .painted_text(&td.day().to_string())
        .into_iter()
        .filter(|(_, c)| *c == Color32::WHITE)
        .find_map(|(r, _)| {
            circles
                .iter()
                .find(|c| c.center.distance(r.center()) < c.radius)
                .map(|c| (r, *c))
        })
        .expect("today's number in its circle");
    // a week before or after: the same column
    let other = if td.day() > 7 {
        td - Duration::days(7)
    } else {
        td + Duration::days(7)
    };
    let lefts: Vec<f32> = h
        .painted_text(&other.day().to_string())
        .into_iter()
        .map(|(r, _)| r.left())
        .collect();
    assert!(
        // (within a glyph's side bearing: "2" and "1" start apart)
        lefts.iter().any(|l| (l - num.left()).abs() < 1.5),
        "today's number at x {} is in line with {} ({:?})",
        num.left(),
        other,
        lefts
    );
    let cell_top = num.center().y - DAY_NUM.y;
    assert!(
        circle.center.y - circle.radius >= cell_top + 2.0,
        "the circle {:?} keeps off the day's top line at {}",
        circle,
        cell_top
    );
    // weekday names start at the numbers' x (Sunday's column)
    let first = NaiveDate::from_ymd_opt(td.year(), td.month(), 1).unwrap();
    let sunday = first - Duration::days(first.weekday().num_days_from_sunday() as i64);
    let sun = h.painted_text(tr("Sun"))[0].0;
    assert!(
        h.painted_text(&sunday.day().to_string())
            .iter()
            .any(|(r, _)| (r.left() - sun.left()).abs() < 1.5),
        "Sun at x {} starts where its days do",
        sun.left()
    );
}

/// The "To schedule" box ends on the same line as the days beside it.
#[test]
fn planner_columns_end_on_one_line() {
    let mut h = Ui1::new();
    h.app.section = Section::Planner;
    h.settle();
    // the tall box drawn around a point (the drop zones paint their own
    // frames, so they are found by where they are)
    let box_at = |h: &Ui1, p: Pos2| {
        h.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Rect(r) if r.rect.contains(p) && r.rect.height() > 200.0 => {
                    Some(r.rect)
                }
                _ => None,
            })
            .min_by(|a, b| a.area().total_cmp(&b.area()))
    };
    let label = h.painted_text(tr("To schedule"))[0].0;
    let left = box_at(&h, label.center()).expect("the To schedule box");
    let week = h.app.planner_week;
    for i in 0..7 {
        let d = week + Duration::days(i);
        let add = h.rect(&format!("Add task on {}", d.format("%a %-d")));
        let day = box_at(&h, add.center()).expect("the day's box");
        assert!(
            (day.bottom() - left.bottom()).abs() < 0.5,
            "{} ends at {}, To schedule at {}",
            d,
            day.bottom(),
            left.bottom()
        );
    }
}

/// In the smallest window the Dashboards templates go two to a row and a
/// dashboard's header wraps, so nothing runs past the right edge (the cards
/// below were widened with it) and every control can be pressed.
#[test]
fn small_window_dashboards_stay_inside() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.app.section = Section::Dashboards;
    h.settle();
    let page = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
    assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
    assert!(h.cut_texts(page).is_empty(), "{:?}", h.cut_texts(page));
    h.click("Simple Dashboard");
    assert!(h.app.dash_open.is_some());
    assert!(h.past_right_edge().is_empty(), "{:?}", h.past_right_edge());
    let add = h.rect("+ Add card");
    assert!(add.right() <= h.size.x);
    h.click("+ Dashboard");
    assert!(matches!(h.app.modal, Some(Modal::NewDashboard { .. })));
}

/// With many members the Org Chart's cards wrap onto more rows instead of
/// running past the window in one row.
#[test]
fn org_chart_cards_wrap_inside_the_window() {
    for (w, hh) in [(1440.0, 900.0), (900.0, 560.0)] {
        let mut h = Ui1::sized(w, hh);
        for k in 0..12u64 {
            h.app.data.members.push(Member {
                id: 900 + k,
                name: format!("Teammate {}", k),
                color: [120, 80, 200],
                role: "Member".into(),
                ..Default::default()
            });
        }
        h.app.section = Section::Teams;
        h.app.hub_filter = 2;
        h.settle();
        let mut right = 0.0f32;
        each_text(&h.shapes, |ts, _| {
            if ts.galley.text().starts_with("Teammate") {
                right = right.max(ts.visual_bounding_rect().right());
            }
        });
        assert!(right > 0.0);
        assert!(right <= w, "{}x{}: a card's name reaches {}", w, hh, right);
    }
}

/// The Timeline packs several bars into a row: with many of them out of
/// sight to the left, each row shows one "‹ date" at the edge, not one per
/// bar drawn over each other and over the names, and in the smallest window
/// "without dates" doesn't run over the lane choices.
#[test]
fn timeline_edge_marks_and_toolbar_do_not_overlap() {
    for (w, hh) in [(1440.0, 900.0), (900.0, 560.0)] {
        let mut h = Ui1::sized(w, hh);
        heavy(&mut h);
        let list = list_id(&h, "Website Redesign");
        h.app.loc = Location::List(list);
        h.app.view = ViewKind::Timeline;
        h.settle();
        let page = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
        let over = overlapping(&h.texts_in(page));
        assert!(over.is_empty(), "{}x{}: {:?}", w, hh, over);
    }
}

/// In the smallest window the view tabs (longer in English) used to run past
/// the edge with "+ View": they now keep only their icons when the words
/// don't fit, and every tab and "+ View" can be pressed.
#[test]
fn small_window_view_tabs_and_add_view_fit() {
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Ko] {
        let mut h = Ui1::sized(900.0, 560.0);
        crate::i18n::set_lang(lang);
        let list = h.app.data.first_list().unwrap();
        h.app.loc = Location::List(list);
        h.app.view = ViewKind::List;
        h.settle();
        for v in ["List", "Board", "Calendar", "Gantt", "Timeline", "Table"] {
            let r = h.rect(&format!("View {}", v));
            assert!(r.right() <= h.size.x - 8.0, "{:?} {}: {:?}", lang, v, r);
        }
        let plus = h.rect(tr("+  View"));
        assert!(
            plus.right() <= h.size.x - 8.0,
            "{:?}: + View at {:?}",
            lang,
            plus
        );
        h.click("View Timeline");
        assert!(h.app.view == ViewKind::Timeline);
        crate::i18n::set_lang(crate::i18n::Lang::En);
    }
}

/// The Timesheet fits the smallest window (the day columns narrow), and a
/// long task name is shortened with an ellipsis instead of cut mid-letter;
/// the calendar's "Unscheduled" cards shorten theirs the same way.
#[test]
fn timesheet_and_calendar_side_shorten_long_names() {
    for (w, hh) in [(1440.0, 900.0), (900.0, 560.0)] {
        let mut h = Ui1::sized(w, hh);
        heavy(&mut h);
        let id = h
            .app
            .data
            .tasks
            .iter()
            .find(|x| x.name == LONG_TASK)
            .unwrap()
            .id;
        h.app.set_day_time(id, today(), 1.5);
        h.app.section = Section::Timesheets;
        h.settle();
        let page = Rect::from_min_max(pos2(60.0, 40.0), h.size.to_pos2());
        assert!(
            h.cut_texts(page).is_empty(),
            "{}x{}: {:?}",
            w,
            hh,
            h.cut_texts(page)
        );
        assert!(
            h.past_right_edge().is_empty(),
            "{}x{}: {:?}",
            w,
            hh,
            h.past_right_edge()
        );
        // the calendar's side list: a task without dates, with a long name
        let mut x = h
            .app
            .data
            .tasks
            .iter()
            .find(|x| x.id == id)
            .unwrap()
            .clone();
        x.id = 31_000;
        x.due = None;
        x.start = None;
        x.time_entries.clear();
        h.app.data.tasks.push(x);
        h.app.section = Section::Home;
        h.app.loc = Location::List(list_id(&h, LONG_LIST));
        h.app.view = ViewKind::Calendar;
        h.settle();
        let side = h.rect(&format!("Unscheduled {}", LONG_TASK));
        assert!(
            h.cut_texts(side).is_empty(),
            "{}x{}: {:?}",
            w,
            hh,
            h.cut_texts(side)
        );
    }
}

/// An open Doc in the smallest window: the header's buttons don't run over
/// the path (Last updated, Copy link and Duplicate give way), and the
/// formatting bar wraps instead of running past the edge.
#[test]
fn small_window_doc_header_and_toolbar_fit() {
    for lang in [crate::i18n::Lang::En, crate::i18n::Lang::Ko] {
        let mut h = Ui1::sized(900.0, 560.0);
        crate::i18n::set_lang(lang);
        let doc = h.app.data.docs.first().map(|d| d.id).unwrap();
        h.app.section = Section::Docs;
        h.app.doc_open = Some(doc);
        h.settle();
        let top = Rect::from_min_max(pos2(320.0, 40.0), pos2(h.size.x, 140.0));
        let over = overlapping(&h.texts_in(top));
        assert!(over.is_empty(), "{:?}: {:?}", lang, over);
        for n in ["Link", "Checklist", "Doc options"] {
            let r = h.rect(n);
            assert!(r.right() <= h.size.x, "{:?} {}: {:?}", lang, n, r);
        }
        crate::i18n::set_lang(crate::i18n::Lang::En);
    }
}

/// A thread open beside a channel in the smallest window: the channel's box
/// on the right keeps only its icons (it was drawn over the messages), the
/// messages and the header's title don't run under anything, and the box's
/// rows still work.
#[test]
fn small_window_thread_beside_a_channel_does_not_overlap() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    assert!(h.app.chat.thread_open.is_some());
    h.size = vec2(900.0, 560.0);
    h.settle();
    assert!(h.has("Close thread"));
    let texts = h.texts_in(Rect::from_min_max(pos2(320.0, 40.0), pos2(900.0, 560.0)));
    let over = overlapping(&texts);
    assert!(over.is_empty(), "{:?}", over);
    // the hover toolbar over a message starts inside the conversation
    let msg = h
        .painted_text("Hello team, please check")
        .first()
        .map(|x| x.0.center())
        .or_else(|| {
            h.find("Morning everyone! The new homepage mockups are in the Website Redesign list.")
        })
        .expect("a message on screen");
    h.hover(msg);
    h.settle();
    let thumbs = h.rect("\u{1F44D}");
    assert!(thumbs.left() >= 320.0, "the first reaction at {:?}", thumbs);
    h.click("Pinned messages");
    assert_eq!(h.app.channel_panel, 1);
}
