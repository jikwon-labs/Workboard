// Click-through tests for Goals: the page as cards and as rows, a goal's
// own page with the controls that move its targets, the New goal dialog,
// Goal achieved and the empty page's samples.

use super::*;
use chrono::Duration;

/// Two goals of mine: "Beta launch" (a number at 38 of 50 and a store
/// review not done, two thirds of its time gone) and "Office move" (one
/// done-or-not target). Returns the window on the Goals page and their ids.
fn two_goals() -> (Ui1, u64, u64) {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let day = today();
    let a = h.app.data.new_id();
    let b = h.app.data.new_id();
    h.app.data.extra.goals = vec![
        Goal {
            id: a,
            name: "Beta launch".into(),
            color: [0x5f, 0x55, 0xee],
            due: Some(day + Duration::days(20)),
            owner: me,
            start: Some(day - Duration::days(40)),
            targets: vec![
                GoalTarget {
                    name: "Beta testers".into(),
                    kind: TargetKind::Number {
                        start: 0.0,
                        target: 50.0,
                        current: 38.0,
                        unit: "people".into(),
                    },
                },
                GoalTarget {
                    name: "Store review".into(),
                    kind: TargetKind::TrueFalse { done: false },
                },
            ],
            ..Default::default()
        },
        Goal {
            id: b,
            name: "Office move".into(),
            color: [0x00, 0x88, 0x44],
            due: Some(day + Duration::days(10)),
            owner: me,
            start: Some(day - Duration::days(10)),
            targets: vec![GoalTarget {
                name: "Lease signed".into(),
                kind: TargetKind::TrueFalse { done: false },
            }],
            ..Default::default()
        },
    ];
    h.click("Rail More");
    h.click("Goals");
    h.settle();
    (h, a, b)
}

fn goal(h: &Ui1, id: u64) -> &Goal {
    h.app
        .data
        .extra
        .goals
        .iter()
        .find(|g| g.id == id)
        .expect("the goal")
}

#[test]
fn goals_show_as_cards_or_rows_and_open_their_own_page() {
    let (mut h, a, _) = two_goals();
    assert!(
        h.has("Goal Beta launch") && h.has("Goal Office move"),
        "cards"
    );
    assert!(
        h.has("Upcoming Office move"),
        "the sooner deadline is in Coming up"
    );

    h.click("Show as list");
    assert_eq!(h.app.data.extra.goals_view, "list");
    assert!(h.has("Goal row Beta launch"));
    h.click("Show targets of Beta launch");
    assert_eq!(h.app.goal_list_open, Some(a), "the arrow shows the targets");

    h.click("Goal row Beta launch");
    assert_eq!(h.app.goal_open, Some(a));
    assert!(h.has("Plus 10 Beta testers") && h.has("Done Store review"));
    assert!(
        h.has("Progress bar") && h.has("Time bar"),
        "progress against time"
    );
    h.click("Back to goals");
    assert_eq!(h.app.goal_open, None);

    h.click("Show as cards");
    assert_eq!(h.app.data.extra.goals_view, "");
    h.click("Show Achieved");
    assert!(!h.has("Goal Beta launch"), "only achieved goals");
    h.click("Show All");
    assert!(h.has("Goal Beta launch"));
}

#[test]
fn the_plan_mark_shows_once_some_time_has_gone() {
    let legend = "Where it should be by today, from its start and due dates";
    let (mut h, a, b) = two_goals();
    // Office move starts today: nothing is due yet, so no mark at 0%
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == b) {
        g.start = Some(today());
    }
    h.click("Show as list");
    assert!(
        h.has("Plan mark Beta launch"),
        "two thirds of its time gone"
    );
    assert!(
        !h.has("Plan mark Office move"),
        "a mark at the very start reads as part of the bar"
    );
    assert!(
        !h.painted_text(legend).is_empty(),
        "the legend for the mark"
    );

    // no bar with a mark: no legend either
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == a) {
        g.start = Some(today());
    }
    h.settle();
    assert!(!h.has("Plan mark Beta launch"));
    assert!(h.painted_text(legend).is_empty(), "nothing to explain");
}

#[test]
fn far_behind_goals_come_first_with_their_pace() {
    let (h, a, b) = two_goals();
    let day = today();
    // Beta launch: 38% done with 67% of its time gone
    let ga = goal(&h, a).clone();
    assert_eq!(ga.pace(h.app.goal_progress(&ga), day), Some(Pace::AtRisk));
    let gb = goal(&h, b).clone();
    assert_eq!(gb.pace(h.app.goal_progress(&gb), day), Some(Pace::AtRisk));
    // both at risk: the sooner deadline first
    let ra = h.rect("Goal Beta launch");
    let rb = h.rect("Goal Office move");
    assert!(
        (rb.top(), rb.left()) < (ra.top(), ra.left()),
        "Office move (due sooner) comes before Beta launch"
    );
}

#[test]
fn recording_a_number_moves_the_goal_and_shows_the_milestone() {
    let (mut h, a, _) = two_goals();
    h.click("Goal Beta launch");
    assert_eq!(h.app.goal_open, Some(a));
    let row = h.rect("Minus 1 Beta testers");
    h.click("Plus 10 Beta testers");
    h.click("Plus 1 Beta testers");
    h.click("Plus 1 Beta testers");
    let g = goal(&h, a).clone();
    let TargetKind::Number { current, .. } = g.targets[0].kind else {
        panic!("a number target")
    };
    assert_eq!(current, 50.0);
    // the number full and the review not done: halfway
    assert_eq!((h.app.goal_progress(&g) * 100.0).round() as u8, 50);
    assert_eq!(
        h.app.goal_banner.map(|b| (b.0, b.1)),
        Some((a, 50)),
        "the 50% banner shows"
    );
    assert!(h.has("Close milestone"));
    assert_eq!(
        h.rect("Minus 1 Beta testers"),
        row,
        "the banner floats over the page: nothing on it moves"
    );
    let me = h.app.data.me;
    assert!(g
        .log
        .iter()
        .any(|e| e.who == me && matches!(e.what, GoalChange::Number { to, .. } if to == 50.0)));
    assert!(
        g.log
            .iter()
            .any(|e| e.what == GoalChange::Milestone(50) && e.who == me),
        "the milestone is in the history: {:?}",
        g.log
    );
    h.click("Close milestone");
    assert_eq!(h.app.goal_banner, None);

    h.click("Minus 1 Beta testers");
    let TargetKind::Number { current, .. } = goal(&h, a).targets[0].kind else {
        panic!()
    };
    assert_eq!(current, 49.0);
}

#[test]
fn undoing_a_target_takes_back_the_milestone_it_passed() {
    let (mut h, a, _) = two_goals();
    h.click("Goal Beta launch");
    let milestones = |h: &Ui1| -> Vec<u8> {
        goal(h, a)
            .log
            .iter()
            .filter_map(|e| match e.what {
                GoalChange::Milestone(m) => Some(m),
                _ => None,
            })
            .collect()
    };
    let button = h.rect("Done Store review");
    // 38 of 50 people and the review done: 88%
    h.click("Done Store review");
    assert_eq!(h.app.goal_banner.map(|b| (b.0, b.1)), Some((a, 75)));
    assert_eq!(milestones(&h), [75]);
    assert_eq!(
        h.rect("Done Store review").size(),
        button.size(),
        "Done and Mark done are the same size"
    );
    // undone: back to 38%, the 75% milestone goes from the banner and the
    // history, and the milestones show it as still to reach
    h.click("Done Store review");
    assert_eq!(h.app.goal_banner, None);
    assert!(milestones(&h).is_empty(), "{:?}", goal(&h, a).log);
    assert_eq!(goal(&h, a).reached, 25);
    // done again: the milestone comes back once
    h.click("Done Store review");
    assert_eq!(milestones(&h), [75]);
    assert!(h.has("Close milestone"));
    // it floats a few seconds, then goes by itself
    let (g, m, _) = h.app.goal_banner.unwrap();
    h.app.goal_banner = Some((g, m, now_ts() - 60));
    h.settle();
    assert_eq!(h.app.goal_banner, None);
    assert!(!h.has("Close milestone"));
}

#[test]
fn a_targets_remove_button_is_round() {
    let (mut h, _, _) = two_goals();
    h.click("Goal Beta launch");
    let r = h.rect("Remove target Beta testers");
    assert_eq!(r.width(), r.height(), "{r:?}");
    h.hover(r.center());
    assert_eq!(
        h.circle_fill_at(r.center()),
        Some(crate::widgets::hover_fill_for(false)),
        "a round hover fill"
    );
    assert!(!h.hover_fill_at(r.center()), "not a rounded box");
}

#[test]
fn a_goal_reaching_100_opens_goal_achieved_and_moves_to_achieved() {
    let (mut h, _, b) = two_goals();
    h.click("Done Lease signed");
    assert!(
        matches!(h.app.modal, Some(Modal::GoalAchieved { id }) if id == b),
        "Goal achieved opens"
    );
    assert_eq!(goal(&h, b).done_on, Some(today()));
    assert!(h.has("Next goal from achieved"));
    h.key(egui::Key::Escape);
    h.settle();
    assert!(h.app.modal.is_none());
    assert!(h.has("Achieved Office move"), "it moved to Achieved");
    assert!(!h.has("Goal Office move"));

    // unticked again it is back in progress
    h.click("Achieved Office move");
    h.click("Done Lease signed");
    assert_eq!(goal(&h, b).done_on, None);
}

#[test]
fn goal_achieved_leads_to_the_next_goal() {
    let (mut h, _, _) = two_goals();
    h.click("Done Lease signed");
    h.click("Next goal from achieved");
    assert!(
        matches!(h.app.modal, Some(Modal::NewGoal(_))),
        "New goal takes its place"
    );
}

#[test]
fn ticking_a_task_on_a_goal_page_closes_it() {
    let (mut h, a, _) = two_goals();
    let task = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.parent.is_none() && !h.app.data.is_closed(t) && !t.archived)
        .expect("an open task")
        .clone();
    h.app.data.extra.goals[0].targets.push(GoalTarget {
        name: "Launch tasks".into(),
        kind: TargetKind::Tasks { ids: vec![task.id] },
    });
    h.click("Goal Beta launch");
    let name = format!("Tick {}", task.name);
    h.click(&name);
    assert!(
        h.app.data.is_closed(h.app.task(task.id).unwrap()),
        "the task is closed"
    );
    assert!(goal(&h, a)
        .log
        .iter()
        .any(|e| matches!(&e.what, GoalChange::Task { done: true, .. })));
}

#[test]
fn new_goal_dialog_creates_a_goal_with_its_targets() {
    let (mut h, _, _) = two_goals();
    let n = h.app.data.extra.goals.len();
    h.click("New goal");
    assert!(matches!(h.app.modal, Some(Modal::NewGoal(_))));
    h.type_text("Hire two designers");
    h.click("Measure by Done or not");
    h.click("Another target");
    let x = h.rect("Remove Target");
    assert_eq!(
        x.width(),
        x.height(),
        "the added target's remove button is round"
    );
    h.click("Measure by Number");
    h.click("Due end of month");
    h.click("Create goal");
    assert!(h.app.modal.is_none());
    assert_eq!(h.app.data.extra.goals.len(), n + 1);
    let g = h.app.data.extra.goals.last().unwrap().clone();
    assert_eq!(g.name, "Hire two designers");
    assert_eq!(g.targets.len(), 2);
    assert!(matches!(g.targets[0].kind, TargetKind::TrueFalse { .. }));
    assert!(matches!(g.targets[1].kind, TargetKind::Number { .. }));
    assert_eq!(g.start, Some(today()));
    assert!(g.due.is_some_and(|d| d >= today()));
    assert_eq!(
        g.log.first().map(|e| e.what.clone()),
        Some(GoalChange::Created)
    );
    assert_eq!(h.app.goal_open, Some(g.id), "the new goal's page opens");
}

#[test]
fn the_empty_goals_page_starts_from_a_sample() {
    let mut h = Ui1::new();
    h.app.data.extra.goals.clear();
    h.click("Rail More");
    h.click("Goals");
    assert!(h.has("New goal"));
    h.click("Sample Product launch");
    h.click("Create goal");
    let g = h.app.data.extra.goals.last().expect("a goal").clone();
    assert_eq!(g.name, "Product launch");
    let names: Vec<&str> = g.targets.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Launch tasks", "Store review passed"]);
}

#[test]
fn a_goals_settings_open_from_its_page_in_a_dialog() {
    let (mut h, a, _) = two_goals();
    h.click("Goal Beta launch");
    assert!(
        !h.has("Goal name") && !h.has("Delete goal"),
        "not on the page itself"
    );
    h.click("Goal settings");
    assert!(matches!(h.app.modal, Some(Modal::GoalSettings { id }) if id == a));
    h.click("Goal color [16, 144, 224]");
    assert_eq!(goal(&h, a).color, [0x10, 0x90, 0xe0]);
    h.click("Close goal settings");
    assert!(h.app.modal.is_none());
    h.click("Goal settings");
    h.click("Delete goal");
    assert!(matches!(
        h.app.modal,
        Some(Modal::ConfirmRemove { kind: "goal", .. })
    ));
}

#[test]
fn goal_milestone_and_risk_notices_can_be_turned_off() {
    let mut h = Ui1::new();
    h.click("Inbox");
    h.click("Notification settings");
    assert!(h.app.data.extra.notify.goal_milestones && h.app.data.extra.notify.goal_risk);
    h.click("Goal milestones");
    h.click("Goals falling behind");
    assert!(!h.app.data.extra.notify.goal_milestones);
    assert!(!h.app.data.extra.notify.goal_risk);
}

#[test]
fn an_achieved_goal_is_shared_to_chat_as_a_card_that_opens_it() {
    let (mut h, _, b) = two_goals();
    // someone else helped: they are mentioned
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != h.app.data.me)
        .expect("a teammate")
        .clone();
    h.app.data.extra.goals[1].record(GoalEvent {
        ts: now_ts(),
        who: other.id,
        what: GoalChange::Created,
        pct: 0,
    });
    h.click("Done Lease signed");
    h.click("Share from achieved");
    let Some(Modal::ShareGoal { channel, .. }) = &h.app.modal else {
        panic!("Share to team chat opens")
    };
    let channel = *channel;
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .expect("the general channel")
        .id;
    assert_eq!(channel, general, "it starts on the general channel");
    // the message's box reaches its 4 px margin past the text area named here
    let (to, msg) = (
        h.rect("Share to channel"),
        h.rect("Share message").expand(4.0),
    );
    assert!(
        (to.left() - msg.left()).abs() < 1.0 && (to.right() - msg.right()).abs() < 1.0,
        "Send to as wide as the message: {to:?} {msg:?}"
    );
    h.click("Send goal to chat");
    assert!(h.app.modal.is_none());
    let c = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == general)
        .unwrap();
    let m = c.messages.last().expect("a message");
    let card = m.goal.as_ref().expect("with the goal's card");
    assert_eq!(card.goal, b);
    assert_eq!((card.pct, card.targets, card.targets_done), (100, 1, 1));
    assert!(
        m.text.contains(&format!("@{}", other.shown())),
        "{}",
        m.text
    );
    assert!(m.mentions.contains(&other.id));

    // in the channel the card opens the goal
    h.app.section = Section::Chat;
    h.app.chat.open = Some(general);
    h.settle();
    let link = h.find("Open goal Office move").expect("the card's link");
    assert!(!h.hover_fill_at(link));
    h.hover(link);
    assert!(
        h.hover_fill_at(link),
        "the link shows it is under the pointer"
    );
    h.click("Open goal Office move");
    assert_eq!(h.app.section, Section::Goals);
    assert_eq!(h.app.goal_open, Some(b));
    assert!(
        h.has("Share to team chat"),
        "the achieved goal's page offers sharing too"
    );
}

const SUMMARY: [&str; 3] = [
    "Summary average progress",
    "Summary coming up",
    "Summary achieved this year",
];

/// Both goals achieved, nothing in progress.
fn all_achieved(h: &mut Ui1) {
    for g in &mut h.app.data.extra.goals {
        for tg in &mut g.targets {
            match &mut tg.kind {
                TargetKind::Number {
                    current, target, ..
                } => *current = *target,
                TargetKind::TrueFalse { done } => *done = true,
                TargetKind::Tasks { .. } => {}
            }
        }
    }
    // the first look starts their history, the next notes the day
    h.app.goal_watch(now_ts());
    h.app.goal_watch(now_ts());
    h.settle();
}

#[test]
fn the_goals_page_and_a_goals_page_line_up_and_centre_in_a_wide_window() {
    let (mut h, _, _) = two_goals();
    let first = h.rect(SUMMARY[0]);
    let last = h.rect(SUMMARY[2]);
    h.click("Goal Beta launch");
    let page = h.rect("Goal overview");
    let near = |a: f32, b: f32| (a - b).abs() < 0.5;
    assert!(near(page.top(), first.top()), "the boxes start on one line");
    assert!(near(page.left(), first.left()));
    assert!(near(page.right(), last.right()), "and end at one edge");
    assert_eq!(
        h.rect("Back to goals").center().y,
        {
            h.click("Back to goals");
            h.rect("New goal").center().y
        },
        "the rows on top are one height"
    );

    // 560 pt wider: the page moves right by half of that, a goal's page too
    let mut w = Ui1::sized(1440.0 + 560.0, 900.0);
    w.app.data.extra.goals = h.app.data.extra.goals.clone();
    w.click("Rail More");
    w.click("Goals");
    w.settle();
    let wide = w.rect(SUMMARY[0]);
    assert!(near(wide.left() - first.left(), 280.0), "in the middle");
    assert!(near(w.rect(SUMMARY[2]).right() - last.right(), 280.0));
    w.click("Goal Beta launch");
    assert!(near(w.rect("Goal overview").left(), wide.left()));
}

#[test]
fn the_three_boxes_on_top_are_one_height_and_never_empty() {
    let (mut h, _, _) = two_goals();
    let same = |h: &Ui1| {
        let r: Vec<Rect> = SUMMARY.iter().map(|n| h.rect(n)).collect();
        assert!(
            r.iter()
                .all(|x| x.top() == r[0].top() && x.height() == r[0].height()),
            "{:?}",
            r
        );
    };
    same(&h);
    // none achieved yet: the one closest to done
    assert!(!h.painted_text("Next up: Beta launch · 38%").is_empty());
    assert!(
        h.has("Achieved in Jan") && h.has("Achieved in Dec"),
        "the months"
    );

    h.click("Done Lease signed");
    h.key(egui::Key::Escape);
    h.settle();
    same(&h);
    let latest = format!("Latest: Office move, {}", today().text("%b %-d"));
    assert!(!h.painted_text(&latest).is_empty());
}

#[test]
fn a_card_says_not_done_and_centres_its_footer_under_the_line() {
    let (h, _, _) = two_goals();
    assert!(!h.painted_text("Not done").is_empty());
    assert!(h.painted_text("Not yet").is_empty());
    let card = h.rect("Goal Beta launch");
    let (text, _) = h.painted_text("0 of 2 targets done")[0];
    // the band under the line is 50 pt, the text in its middle
    assert!(
        (card.bottom() - 25.0 - text.center().y).abs() < 2.0,
        "{:?} {:?}",
        card,
        text
    );
}

#[test]
fn the_list_view_shows_achieved_goals_as_rows_too() {
    let (mut h, _, b) = two_goals();
    all_achieved(&mut h);
    assert!(h.has("Achieved Office move") && h.has("Achieved Beta launch"));

    // nothing in progress, yet the switch changes the page
    h.click("Show as list");
    assert!(h.has("Goal row Office move") && h.has("Goal row Beta launch"));
    assert!(!h.has("Achieved Office move"), "rows instead of cards");
    let when = format!("Achieved {}", today().text("%b %-d"));
    assert_eq!(h.painted_text(&when).len(), 2, "when, for the days left");
    h.click("Goal row Office move");
    assert_eq!(h.app.goal_open, Some(b));
    h.click("Back to goals");

    h.click("Show as cards");
    assert!(h.has("Achieved Office move"));
}

#[test]
fn share_and_set_the_next_goal_are_one_height() {
    let (mut h, _, _) = two_goals();
    h.click("Done Lease signed");
    assert_eq!(
        h.rect("Share from achieved").height(),
        h.rect("Next goal from achieved").height(),
        "in Goal achieved"
    );
    h.key(egui::Key::Escape);
    h.settle();
    h.click("Achieved Office move");
    assert_eq!(
        h.rect("Share to team chat").height(),
        h.rect("Set the next goal").height(),
        "on the goal's page"
    );
}

/// Adds a goal of mine due in 30 days that started 10 days ago with these
/// targets, and returns its id. The window stays where it is.
fn add_goal_with(h: &mut Ui1, name: &str, targets: Vec<GoalTarget>) -> u64 {
    let id = h.app.data.new_id();
    let day = today();
    h.app.data.extra.goals.push(Goal {
        id,
        name: name.into(),
        color: [0x44, 0x66, 0xff],
        due: Some(day + Duration::days(30)),
        owner: h.app.data.me,
        start: Some(day - Duration::days(10)),
        targets,
        ..Default::default()
    });
    h.settle();
    id
}

fn number(name: &str, start: f64, target: f64, current: f64) -> GoalTarget {
    GoalTarget {
        name: name.into(),
        kind: TargetKind::Number {
            start,
            target,
            current,
            unit: String::new(),
        },
    }
}

fn current_of(h: &Ui1, id: u64, i: usize) -> f64 {
    match goal(h, id).targets[i].kind {
        TargetKind::Number { current, .. } => current,
        _ => panic!("a number target"),
    }
}

#[test]
fn a_goal_just_short_of_100_is_not_achieved() {
    let (mut h, _, _) = two_goals();
    // 994 of 1,000: one more is 99.5%, which rounds to 100 but isn't done
    let id = add_goal_with(
        &mut h,
        "Signups",
        vec![number("People", 0.0, 1000.0, 994.0)],
    );
    h.click("Goal Signups");
    h.click("Plus 1 People");
    assert_eq!(current_of(&h, id, 0), 995.0);
    assert!(
        h.app.modal.is_none(),
        "Goal achieved doesn't open before every target is met"
    );
    let g = goal(&h, id).clone();
    assert_eq!(g.done_on, None, "no achieved day");
    assert!(
        !g.log.iter().any(|e| e.what == GoalChange::Milestone(100)),
        "no 100% milestone: {:?}",
        g.log
    );
    assert!(
        h.painted_text("100%").is_empty() && !h.painted_text("99%").is_empty(),
        "the ring says 99%"
    );
    h.click("Back to goals");
    assert!(h.has("Goal Signups"), "still in progress");
    assert!(!h.has("Achieved Signups"));
}

#[test]
fn ticking_the_last_task_on_a_goals_page_opens_goal_achieved() {
    let (mut h, _, _) = two_goals();
    let task = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.parent.is_none() && !h.app.data.is_closed(t) && !t.archived)
        .expect("an open task")
        .clone();
    let id = add_goal_with(
        &mut h,
        "Ship it",
        vec![GoalTarget {
            name: "Launch tasks".into(),
            kind: TargetKind::Tasks { ids: vec![task.id] },
        }],
    );
    h.click("Goal Ship it");
    h.click(&format!("Tick {}", task.name));
    assert!(h.app.data.is_closed(h.app.task(task.id).unwrap()));
    assert!(
        matches!(h.app.modal, Some(Modal::GoalAchieved { id: g }) if g == id),
        "Goal achieved opens as it does for a number or a tick: {:?}",
        h.app.modal.as_ref().map(|_| "another dialog")
    );
    assert_eq!(goal(&h, id).done_on, Some(today()));
}

/// The workspace as it is saved and read back, the way a restart or a
/// teammate's copy sees it.
fn reads_back(h: &Ui1) -> Result<Data, String> {
    let v = serde_json::to_value(&h.app.data).map_err(|e| e.to_string())?;
    serde_json::from_value::<Data>(v).map_err(|e| e.to_string())
}

#[test]
fn recording_inf_or_nan_keeps_the_workspace_readable() {
    let (mut h, a, _) = two_goals();
    for typed in ["inf", "NaN", "-inf", "1e999"] {
        h.click("Record Beta testers");
        h.click("Value now");
        h.type_text(typed);
        h.click("Save value");
        h.settle();
        let v = current_of(&h, a, 0);
        assert!(v.is_finite(), "{typed:?} gave {v}");
        assert!(
            reads_back(&h).is_ok(),
            "{typed:?}: {:?}",
            reads_back(&h).err()
        );
    }
}

#[test]
fn the_new_goal_dialog_keeps_its_numbers_finite() {
    let (mut h, _, _) = two_goals();
    h.click("New goal");
    h.type_text("Infinite");
    h.click("Target value");
    h.type_text("inf");
    h.click("Start value");
    h.type_text("-1e999");
    h.click("Create goal");
    let g = h.app.data.extra.goals.last().unwrap().clone();
    let TargetKind::Number { start, target, .. } = g.targets[0].kind else {
        panic!("a number target")
    };
    assert!(start.is_finite() && target.is_finite(), "{start} {target}");
    assert!(reads_back(&h).is_ok(), "{:?}", reads_back(&h).err());
}
