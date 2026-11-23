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
                    id: 0,
                    name: "Beta testers".into(),
                    kind: TargetKind::Number {
                        start: 0.0,
                        target: 50.0,
                        current: 38.0,
                        unit: "people".into(),
                    },
                },
                GoalTarget {
                    id: 0,
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
                id: 0,
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
    assert_eq!(h.app.goal.list_open, Some(a), "the arrow shows the targets");

    h.click("Goal row Beta launch");
    assert_eq!(h.app.goal.open, Some(a));
    assert!(h.has("Plus 10 Beta testers") && h.has("Done Store review"));
    assert!(
        h.has("Progress bar") && h.has("Time bar"),
        "progress against time"
    );
    h.click("Back to goals");
    assert_eq!(h.app.goal.open, None);

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
    assert_eq!(h.app.goal.open, Some(a));
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
        h.app.goal.banner.map(|b| (b.0, b.1)),
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
    assert_eq!(h.app.goal.banner, None);

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
    assert_eq!(h.app.goal.banner.map(|b| (b.0, b.1)), Some((a, 75)));
    assert_eq!(milestones(&h), [75]);
    assert_eq!(
        h.rect("Done Store review").size(),
        button.size(),
        "Done and Mark done are the same size"
    );
    // undone: back to 38%, the 75% milestone goes from the banner and the
    // history, and the milestones show it as still to reach
    h.click("Done Store review");
    assert_eq!(h.app.goal.banner, None);
    assert!(milestones(&h).is_empty(), "{:?}", goal(&h, a).log);
    assert_eq!(goal(&h, a).reached, 25);
    // done again: the milestone comes back once
    h.click("Done Store review");
    assert_eq!(milestones(&h), [75]);
    assert!(h.has("Close milestone"));
    // it floats a few seconds, then goes by itself
    let (g, m, _) = h.app.goal.banner.unwrap();
    h.app.goal.banner = Some((g, m, now_ts() - 60));
    h.settle();
    assert_eq!(h.app.goal.banner, None);
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
        id: 0,
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
    assert_eq!(h.app.goal.open, Some(g.id), "the new goal's page opens");
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
        id: 0,
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
    let general = h.channel_id("general");
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
    assert_eq!(h.app.goal.open, Some(b));
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
    assert_eq!(h.app.goal.open, Some(b));
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
        id: 0,
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
            id: 0,
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
    h.click("Target value entry");
    h.type_text("inf");
    h.click("Start value entry");
    h.type_text("-1e999");
    h.click("Create goal");
    let g = h.app.data.extra.goals.last().unwrap().clone();
    let TargetKind::Number { start, target, .. } = g.targets[0].kind else {
        panic!("a number target")
    };
    assert!(start.is_finite() && target.is_finite(), "{start} {target}");
    assert!(reads_back(&h).is_ok(), "{:?}", reads_back(&h).err());
}

#[test]
fn a_number_typed_with_commas_is_recorded() {
    let (mut h, a, _) = two_goals();
    h.click("Record Beta testers");
    h.click("Value now");
    // the way the card itself writes numbers: "7,200 / 10,000"
    h.type_text("1,500");
    h.click("Save value");
    assert_eq!(current_of(&h, a, 0), 1500.0);
}

#[test]
fn setting_the_number_in_edit_goes_into_the_history_and_can_achieve_the_goal() {
    let (mut h, _, _) = two_goals();
    let id = add_goal_with(&mut h, "Signups", vec![number("People", 0.0, 10.0, 4.0)]);
    h.click("Goal Signups");
    h.click("Edit People");
    h.click("Number now");
    h.type_text("10");
    h.key(egui::Key::Enter);
    h.settle();
    assert_eq!(current_of(&h, id, 0), 10.0);
    let me = h.app.data.me;
    assert!(
        goal(&h, id).log.iter().any(|e| e.who == me
            && matches!(e.what, GoalChange::Number { from, to, .. } if from == 4.0 && to == 10.0)),
        "one line in the history, from 4 to 10: {:?}",
        goal(&h, id).log
    );
    assert!(
        matches!(h.app.modal, Some(Modal::GoalAchieved { id: g }) if g == id),
        "Goal achieved opens"
    );
}

#[test]
fn dragging_the_number_in_edit_records_one_line_and_escape_drops_a_typed_one() {
    let (mut h, _, _) = two_goals();
    let id = add_goal_with(&mut h, "Signups", vec![number("People", 0.0, 100.0, 4.0)]);
    h.click("Goal Signups");
    h.click("Edit People");
    let lines = |h: &Ui1| {
        goal(h, id)
            .log
            .iter()
            .filter(|e| matches!(e.what, GoalChange::Number { .. }))
            .count()
    };
    let at = h.find("Number now").expect("the box for the number now");
    h.drag_by(at, at + vec2(30.0, 0.0));
    let after = current_of(&h, id, 0);
    assert!(after > 4.0, "dragged up: {after}");
    assert_eq!(lines(&h), 1, "one line for the whole drag");

    h.click("Number now");
    h.type_text("77");
    h.key(egui::Key::Escape);
    h.settle();
    assert_eq!(current_of(&h, id, 0), after, "Escape keeps what it was");
    assert_eq!(lines(&h), 1);
}

#[test]
fn enter_in_record_saves_and_escape_drops_it() {
    let (mut h, a, _) = two_goals();
    h.click("Record Beta testers");
    h.click("Value now");
    h.type_text("41");
    h.key(egui::Key::Enter);
    h.settle();
    assert_eq!(current_of(&h, a, 0), 41.0, "Enter saves");
    assert!(!h.has("Save value"), "and closes the popup");

    h.click("Record Beta testers");
    h.click("Value now");
    h.type_text("12");
    h.key(egui::Key::Escape);
    h.settle();
    assert_eq!(current_of(&h, a, 0), 41.0, "Escape keeps what it was");
    assert!(!h.has("Save value"));
}

#[test]
fn history_by_someone_who_left_names_someone() {
    let (mut h, _, _) = two_goals();
    let id = add_goal_with(&mut h, "Signups", vec![number("People", 0.0, 10.0, 4.0)]);
    let gone = 987_654_321;
    assert!(h.app.data.member(gone).is_none());
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == id) {
        g.record(GoalEvent {
            id: 0,
            ts: now_ts(),
            who: gone,
            what: GoalChange::Created,
            pct: 0,
        });
    }
    h.click("Goal Signups");
    assert!(
        !h.painted_text("Someone created the goal").is_empty(),
        "not \" created the goal\""
    );
    assert!(h.painted_text(" created the goal").is_empty());
}

#[test]
fn sharing_leaves_out_people_who_left_the_team() {
    let (mut h, _, b) = two_goals();
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == b) {
        g.record(GoalEvent {
            id: 0,
            ts: now_ts(),
            who: 987_654_321,
            what: GoalChange::Created,
            pct: 0,
        });
    }
    h.click("Done Lease signed");
    h.click("Share from achieved");
    assert!(
        !h.has("Mention the people involved"),
        "nobody to mention, not \"Mention the people involved ()\""
    );
}

#[test]
fn every_new_goal_button_starts_in_the_next_colour() {
    let (mut h, _, _) = two_goals();
    // two goals already: the third colour, as the empty page's button and
    // Goal achieved's "Set the next goal" give
    let next = [0x10, 0x90, 0xe0];
    h.click("New goal");
    let Some(Modal::NewGoal(d)) = &h.app.modal else {
        panic!("New goal opens")
    };
    assert_eq!(d.color, next, "+ New goal on the Goals page");
    h.click("Cancel new goal");
    h.click("Done Lease signed");
    h.key(egui::Key::Escape);
    h.settle();
    h.click("Achieved Office move");
    h.click("Set the next goal");
    let Some(Modal::NewGoal(d)) = &h.app.modal else {
        panic!("New goal opens")
    };
    assert_eq!(
        d.color, next,
        "Set the next goal on an achieved goal's page"
    );
}

#[test]
fn the_controls_on_a_card_work_without_opening_it() {
    let (mut h, a, _) = two_goals();
    h.click("Done Store review");
    assert_eq!(h.app.goal.open, None, "ticking doesn't open the goal");
    assert!(matches!(
        goal(&h, a).targets[1].kind,
        TargetKind::TrueFalse { done: true }
    ));
    h.click("Record Beta testers");
    assert_eq!(h.app.goal.open, None, "+ Record doesn't open the goal");
    assert!(h.has("Save value"), "its popup does");
    // the popup lies over the card: pressing in it stays in it
    h.click("Value now");
    h.type_text("40");
    h.click("Save value");
    assert_eq!(h.app.goal.open, None);
    assert_eq!(current_of(&h, a, 0), 40.0);
    // the rest of the card opens it
    h.click("Goal Beta launch");
    assert_eq!(h.app.goal.open, Some(a));
}

#[test]
fn a_rows_arrow_shows_its_targets_without_opening_it() {
    let (mut h, a, _) = two_goals();
    h.click("Show as list");
    h.click("Show targets of Beta launch");
    assert_eq!(h.app.goal.open, None);
    assert_eq!(h.app.goal.list_open, Some(a));
    assert!(!h.painted_text("38 / 50 people").is_empty(), "its targets");
    h.click("Show targets of Beta launch");
    assert_eq!(h.app.goal.list_open, None, "the arrow folds them again");
    assert!(h.painted_text("38 / 50 people").is_empty());
}

#[test]
fn deleting_a_goal_closes_its_page_and_unlinks_its_chat_card() {
    let (mut h, _, b) = two_goals();
    h.click("Done Lease signed");
    h.click("Share from achieved");
    h.click("Send goal to chat");
    h.click("Achieved Office move");
    h.click("Goal settings");
    h.click("Delete goal");
    // the confirm dialog's own Delete button (the lowest of the "Delete"s)
    let at = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Delete" && r.height() < 40.0)
        .map(|(_, r)| r.center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    h.click_at(at);
    assert!(h.app.modal.is_none());
    assert!(h.app.data.extra.goals.iter().all(|g| g.id != b), "gone");
    assert_eq!(h.app.goal.open, None, "back on the Goals page");
    assert!(h.has("Goal Beta launch"));
    assert!(h.app.data.extra.trash.iter().any(|x| x.kind == "Goal"));

    // its card in the channel stays, without the way to open it
    let general = h.channel_id("general");
    h.app.section = Section::Chat;
    h.app.chat.open = Some(general);
    h.settle();
    assert!(!h.painted_text("Office move").is_empty(), "the card");
    assert!(!h.has("Open goal Office move"));

    // a notice about it opens the Goals page, not an empty one
    h.app.open_target(&format!("goal:{}", b));
    h.settle();
    assert_eq!(h.app.section, Section::Goals);
    assert!(h.has("Goal Beta launch"));
    assert_eq!(h.app.goal.open, None);

    // Trash brings it back as it was, achieved, with its history
    let j = h
        .app
        .data
        .extra
        .trash
        .iter()
        .position(|x| x.kind == "Goal")
        .unwrap();
    h.app.restore_trash(j);
    h.settle();
    let g = goal(&h, b);
    assert_eq!(g.done_on, Some(today()));
    assert!(g.log.iter().any(|e| e.what == GoalChange::Milestone(100)));
    assert!(h.has("Achieved Office move"));
}

#[test]
fn a_goal_or_target_name_cant_be_left_empty() {
    let (mut h, a, _) = two_goals();
    h.click("Goal Beta launch");
    h.click("Goal settings");
    // the box, under its label of the same name
    let at = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Goal name")
        .map(|(_, r)| r.center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    h.click_at(at);
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.settle();
    assert_eq!(goal(&h, a).name, "Beta launch", "a blank name isn't kept");
    h.type_text("Beta 2");
    assert_eq!(goal(&h, a).name, "Beta 2", "a new one is");
    h.click("Close goal settings");
    assert!(h.has("Beta 2"));

    h.click("Target name Beta testers");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.click("Goal overview");
    assert_eq!(goal(&h, a).targets[0].name, "Beta testers");
    assert!(h.has("Target name Beta testers"), "the box shows it again");
}

/// No two of these texts overlap, and all are inside `area`.
fn assert_apart(texts: &[(String, Rect)], area: Rect, what: &str) {
    for (i, (a, ra)) in texts.iter().enumerate() {
        assert!(
            area.expand(0.5).contains_rect(*ra),
            "{what}: {a:?} {ra:?} runs out of {area:?}"
        );
        for (b, rb) in &texts[i + 1..] {
            assert!(
                !ra.shrink(0.5).intersects(rb.shrink(0.5)),
                "{what}: {a:?} {ra:?} overlaps {b:?} {rb:?}"
            );
        }
    }
}

#[test]
fn a_long_owner_name_fits_in_a_card_and_a_row() {
    // two cards to a row at their narrowest
    let mut h = Ui1::sized(990.0, 900.0);
    h.app.data = two_goals().0.app.data.clone();
    let me = h.app.data.me;
    for m in h.app.data.members.iter_mut().filter(|m| m.id == me) {
        m.name = "김서연 (Seoyeon Kim, Product Design Lead)".into();
        m.display_name.clear();
    }
    h.click("Rail More");
    h.click("Goals");
    h.settle();
    let card = h.rect("Goal Beta launch");
    assert!(card.width() < 500.0, "two to a row: {card:?}");
    let band = Rect::from_min_max(pos2(card.left(), card.bottom() - 50.0), card.max);
    assert_apart(
        &h.texts_in(band, Seen::Touching),
        band.shrink2(vec2(19.0, 0.0)),
        "card footer",
    );

    h.click("Show as list");
    let row = h.rect("Goal row Beta launch");
    assert_apart(&h.texts_in(row, Seen::Touching), row, "row");
}

#[test]
fn a_goals_page_fits_the_narrowest_window_with_big_numbers() {
    let mut h = Ui1::sized(900.0, 900.0);
    h.app.data = two_goals().0.app.data.clone();
    let id = add_goal_with(
        &mut h,
        "Revenue",
        vec![
            GoalTarget {
                id: 0,
                name: "New contracts".into(),
                kind: TargetKind::Number {
                    start: 0.0,
                    target: 10_000_000_000.0,
                    current: 4_200_000_000.0,
                    unit: "won".into(),
                },
            },
            number("Most of all", -1e15, 1e15, 999_999_999_999_999.0),
        ],
    );
    h.hover(pos2(450.0, 450.0));
    h.app.section = Section::Goals;
    h.app.goal.open = Some(id);
    h.settle();
    let window = Rect::from_min_size(Pos2::ZERO, vec2(900.0, 900.0));
    for name in [
        "Remove target New contracts",
        "Remove target Most of all",
        "Add target",
        "Goal settings",
        "Edit New contracts",
    ] {
        let r = h.rect(name);
        assert!(
            window.shrink(8.0).contains_rect(r),
            "{name} {r:?} is cut off by the window's edge"
        );
    }
    // the boxes end where the row on top does, at the page's edge
    let edge = h.rect("Goal settings").right();
    let overview = h.rect("Goal overview");
    assert!(overview.right() <= edge + 0.5, "{overview:?} past {edge}");
    // (below the window's own top bar)
    let page = h.texts_in(window.expand(2000.0), Seen::Touching);
    let past: Vec<_> = page
        .iter()
        .filter(|(_, r)| r.top() > 60.0 && r.right() > edge + 0.5)
        .collect();
    assert!(past.is_empty(), "past the page's edge: {past:?}");
}

#[test]
fn the_goals_page_fits_the_narrowest_window() {
    let mut h = Ui1::sized(900.0, 900.0);
    let mut data = two_goals().0.app.data.clone();
    // a week of history: "Up 12 points this week" beside the title
    let week = today() - Duration::days(8);
    for g in &mut data.extra.goals {
        g.days = vec![(week, 26)];
    }
    h.app.data = data;
    h.hover(pos2(450.0, 450.0));
    h.app.section = Section::Goals;
    h.settle();
    let edge = h.rect("New goal").right();
    let boxes: Vec<Rect> = SUMMARY.iter().map(|n| h.rect(n)).collect();
    for b in &boxes {
        assert!(
            (b.width() - boxes[0].width()).abs() < 1.0,
            "one width: {boxes:?}"
        );
        assert!(b.right() <= edge + 0.5, "{b:?} past the page's edge {edge}");
    }
    let card = h.rect("Goal Beta launch");
    assert!(card.right() <= edge + 0.5, "{card:?} past {edge}");
    let window = Rect::from_min_size(Pos2::ZERO, vec2(900.0, 900.0));
    let past: Vec<_> = h
        .texts_in(window.expand(2000.0), Seen::Touching)
        .into_iter()
        .filter(|(_, r)| r.top() > 60.0 && r.right() > edge + 0.5)
        .collect();
    assert!(past.is_empty(), "past the page's edge: {past:?}");
    for b in &boxes {
        assert_apart(
            &h.texts_in(*b, Seen::Touching),
            b.shrink(1.0),
            "summary box",
        );
    }
    // 38 - 26 and 0 - 26 points: down 7 on average, beside the big number
    assert!(!h.painted_text("Down 7 points this week").is_empty());
    for n in ["On track 0", "Behind 0", "At risk 2"] {
        assert!(!h.painted_text(n).is_empty(), "{n} in full");
    }
}

#[test]
fn a_goal_without_targets_says_so_in_the_list_too() {
    let (mut h, _, _) = two_goals();
    add_goal_with(&mut h, "Blank", vec![]);
    assert!(!h.painted_text("No targets yet").is_empty(), "on its card");
    h.click("Show as list");
    assert!(h.has("Goal row Blank"));
    assert!(
        h.painted_text("0 of 0 targets done").is_empty(),
        "not \"0 of 0 targets done\""
    );
    assert!(!h.painted_text("No targets yet").is_empty(), "in its row");
}

#[test]
fn changing_a_targets_range_or_its_tasks_counts_as_mine() {
    let (mut h, _, _) = two_goals();
    let me = h.app.data.me;
    let id = add_goal_with(&mut h, "Signups", vec![number("People", 0.0, 100.0, 40.0)]);
    // its history has begun, as it has for any goal the app has seen
    h.app.goal_watch(now_ts());
    h.click("Goal Signups");
    // 40 of 100 is 40%; aiming for 40 makes it done
    h.click("Edit People");
    h.click("Number target");
    h.type_text("40");
    h.key(egui::Key::Enter);
    h.settle();
    let g = goal(&h, id).clone();
    assert!(matches!(g.targets[0].kind, TargetKind::Number { target, .. } if target == 40.0));
    assert!(
        matches!(h.app.modal, Some(Modal::GoalAchieved { id: x }) if x == id),
        "Goal achieved opens, as for any change made on the page"
    );
    assert!(
        g.log
            .iter()
            .any(|e| e.what == GoalChange::Milestone(100) && e.who == me),
        "the milestone is mine, not the app's: {:?}",
        g.log
    );
    h.app.notify_clock(now_ts());
    assert!(
        h.app
            .notify_rt
            .sent
            .iter()
            .all(|n| !n.target.contains(&id.to_string())),
        "nobody tells me what I just did: {:?}",
        h.app.notify_rt.sent
    );
}

#[test]
fn picking_tasks_that_finish_a_goal_opens_goal_achieved() {
    let (mut h, _, _) = two_goals();
    let me = h.app.data.me;
    let task = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.parent.is_none() && !t.archived)
        .expect("a task")
        .clone();
    // closed already
    h.app.toggle_done(task.id);
    for a in std::mem::take(&mut h.app.actions) {
        h.app.apply(a);
    }
    assert!(h.app.data.is_closed(h.app.task(task.id).unwrap()));
    let id = add_goal_with(
        &mut h,
        "Ship it",
        vec![GoalTarget {
            id: 0,
            name: "Launch tasks".into(),
            kind: TargetKind::Tasks { ids: vec![] },
        }],
    );
    h.app.goal_watch(now_ts());
    h.click("Goal Ship it");
    h.click("Pick tasks for Launch tasks");
    h.click(&task.name);
    h.settle();
    assert!(
        matches!(h.app.modal, Some(Modal::GoalAchieved { id: x }) if x == id),
        "Goal achieved opens"
    );
    assert!(goal(&h, id)
        .log
        .iter()
        .any(|e| e.what == GoalChange::Milestone(100) && e.who == me));
}

#[test]
fn goal_history_and_daily_progress_keep_their_limits() {
    let (mut h, a, _) = two_goals();
    h.app.goal_watch(now_ts());
    let gi = h
        .app
        .data
        .extra
        .goals
        .iter()
        .position(|g| g.id == a)
        .unwrap();
    // 150 recordings: the newest 100 lines stay, oldest first
    for n in 0..150 {
        h.app.goal_apply(crate::app::more::GoalAct::SetNumber {
            g: a,
            i: 0,
            to: (n % 2) as f64 + 1.0,
        });
    }
    let log = &h.app.data.extra.goals[gi].log;
    assert_eq!(log.len(), crate::model::GOAL_LOG_KEEP);
    assert!(log.windows(2).all(|w| w[0].ts <= w[1].ts), "oldest first");
    // a change on each of 80 days: the last 60 days stay
    let start = now_ts() - 90 * 86_400;
    for d in 0..80 {
        let pct = (d % 2) as f64 * 10.0;
        if let TargetKind::Number { current, .. } = &mut h.app.data.extra.goals[gi].targets[0].kind
        {
            *current = pct;
        }
        h.app.goal_settle(gi, 0, start + d * 86_400);
    }
    let days = &h.app.data.extra.goals[gi].days;
    assert_eq!(days.len(), crate::model::GOAL_DAYS_KEEP);
    assert!(days.windows(2).all(|w| w[0].0 < w[1].0), "one entry a day");
}

#[test]
fn passing_several_milestones_at_once_notes_the_highest() {
    let (mut h, _, _) = two_goals();
    let me = h.app.data.me;
    let id = add_goal_with(&mut h, "Signups", vec![number("People", 0.0, 100.0, 0.0)]);
    h.app.goal_watch(now_ts());
    h.click("Goal Signups");
    h.app.goal_apply(crate::app::more::GoalAct::SetNumber {
        g: id,
        i: 0,
        to: 80.0,
    });
    h.settle();
    let g = goal(&h, id).clone();
    let ms: Vec<u8> = g
        .log
        .iter()
        .filter_map(|e| match e.what {
            GoalChange::Milestone(m) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(ms, [75], "one line for the highest");
    assert_eq!(g.reached, 75);
    assert_eq!(h.app.goal.banner.map(|b| (b.0, b.1)), Some((id, 75)));
    // the page shows the ones below as reached too, without a day
    assert!(!h.painted_text("Reached").is_empty());
    // back to 30%: 75 leaves, 25 stays; a later 100% opens Goal achieved
    h.app.goal_apply(crate::app::more::GoalAct::SetNumber {
        g: id,
        i: 0,
        to: 30.0,
    });
    assert_eq!(goal(&h, id).reached, 25);
    assert!(goal(&h, id)
        .log
        .iter()
        .all(|e| !matches!(e.what, GoalChange::Milestone(_))));
    h.app.goal_apply(crate::app::more::GoalAct::SetNumber {
        g: id,
        i: 0,
        to: 100.0,
    });
    assert!(matches!(h.app.modal, Some(Modal::GoalAchieved { id: x }) if x == id));
    let g = goal(&h, id).clone();
    assert_eq!(g.done_on, Some(today()));
    assert!(g
        .log
        .iter()
        .any(|e| e.what == GoalChange::Milestone(100) && e.who == me));
}

#[test]
fn a_goal_from_before_history_starts_it_quietly() {
    let (mut h, _, b) = two_goals();
    // achieved before this version kept history
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == b) {
        g.targets[0].kind = TargetKind::TrueFalse { done: true };
        g.log.clear();
    }
    h.app.goal_watch(now_ts());
    let g = goal(&h, b).clone();
    assert_eq!(g.log.len(), 1);
    assert_eq!(g.log[0].what, GoalChange::Tracked);
    assert_eq!(g.reached, 100, "taken as it is");
    h.app.notify_clock(now_ts());
    assert!(
        h.app
            .notify_rt
            .sent
            .iter()
            .all(|n| !n.title.contains("Office move")),
        "not announced: {:?}",
        h.app.notify_rt.sent
    );
    // the next look gives it an achieved day, so it counts this year
    h.app.goal_watch(now_ts());
    assert_eq!(goal(&h, b).done_on, Some(today()));
}

#[test]
fn korean_typed_in_a_goal_and_a_target_name_is_kept() {
    use egui::{Event::Ime, ImeEvent::*};
    let (mut h, a, _) = two_goals();
    h.click("Goal Beta launch");
    // a target's name: end of it, then a syllable put together
    h.click("Target name Beta testers");
    h.key(egui::Key::End);
    h.frame(vec![Ime(Enabled), Ime(Preedit("ㅎ".into()))]);
    h.frame(vec![Ime(Preedit("하".into()))]);
    h.frame(vec![Ime(Preedit("한".into()))]);
    h.frame(vec![
        Ime(Preedit("한".into())),
        Ime(Disabled),
        Ime(Commit("한".into())),
        Ime(Disabled),
    ]);
    h.settle();
    assert_eq!(goal(&h, a).targets[0].name, "Beta testers한");

    // the goal's name in its settings, from empty
    h.click("Goal settings");
    let at = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Goal name")
        .map(|(_, r)| r.center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    h.click_at(at);
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.frame(vec![Ime(Enabled), Ime(Preedit("ㅁ".into()))]);
    h.frame(vec![Ime(Preedit("목".into()))]);
    h.frame(vec![
        Ime(Preedit("목".into())),
        Ime(Disabled),
        Ime(Commit("목".into())),
        Ime(Disabled),
    ]);
    h.frame(vec![Ime(Enabled), Ime(Preedit("ㅍ".into()))]);
    h.frame(vec![Ime(Preedit("표".into()))]);
    h.frame(vec![
        Ime(Preedit("표".into())),
        Ime(Disabled),
        Ime(Commit("표".into())),
        Ime(Disabled),
    ]);
    h.settle();
    assert_eq!(goal(&h, a).name, "목표");
}

#[test]
fn search_finds_a_goal_and_opens_it() {
    let (mut h, a, _) = two_goals();
    let rows = h.app.search_rows("beta lau", 0);
    assert!(rows
        .iter()
        .any(|r| matches!(r.4, crate::app::more::Hit::Goal(x) if x == a)));
    h.app.section = Section::Chat;
    h.settle();
    h.click("Search");
    h.type_text("Beta lau");
    h.settle();
    h.click("Beta launch");
    assert_eq!(h.app.section, Section::Goals);
    assert_eq!(h.app.goal.open, Some(a));
    assert!(h.has("Goal overview"), "its own page");
}

/// Targets with long names added in New goal start new rows in the dialog
/// instead of stretching it wider, and each one's × can be pressed.
#[test]
fn long_target_chips_wrap_inside_the_dialog() {
    let (mut h, _, _) = two_goals();
    h.click("New goal");
    h.click("Another target");
    h.click("Another target");
    h.click("Another target");
    let names = [
        "Ship the onboarding redesign to every customer",
        "Short",
        "아주 긴 한국어 목표 대상 이름이 여기에 들어갑니다 정말 길게",
    ];
    if let Some(Modal::NewGoal(d)) = &mut h.app.modal {
        for (tg, n) in d.more.iter_mut().zip(names) {
            tg.name = n.into();
        }
    }
    h.settle();
    let dialog = h.modal_rect();
    // the chips don't stretch the dialog past its widest (620 and its margin)
    assert!(
        dialog.width() <= 660.0,
        "the dialog is {} wide",
        dialog.width()
    );
    for n in names {
        let x = h.rect(&format!("Remove {}", n));
        assert!(x.right() <= dialog.right(), "{:?}'s × is at {:?}", n, x);
    }
    h.click(&format!("Remove {}", names[2]));
    if let Some(Modal::NewGoal(d)) = &h.app.modal {
        assert_eq!(d.more.len(), 2);
    }
}

/// A goal's header reads "< Goals / name" with "/" right after "Goals", as
/// a form's header does (a wide gap used to sit before the "/").
#[test]
fn goal_header_slash_follows_the_hub_name() {
    let (mut h, a, _) = two_goals();
    h.app.goal.open = Some(a);
    h.settle();
    let crumb = h.rect("Goals crumb");
    let hub = h
        .painted_text("Goals")
        .into_iter()
        .find(|(r, _)| crumb.contains(r.center()))
        .expect("\"Goals\" in its crumb")
        .0;
    let slash = h
        .painted_text("/")
        .into_iter()
        .find(|(r, _)| (r.center().y - hub.center().y).abs() < 10.0 && r.left() > hub.right())
        .expect("a / after it")
        .0;
    assert!(
        slash.left() - hub.right() <= 8.0,
        "{} between \"Goals\" and \"/\"",
        slash.left() - hub.right()
    );
    h.click("Goals crumb");
    assert!(h.app.goal.open.is_none());
}

#[test]
fn a_number_target_cant_start_where_it_ends() {
    let (mut h, _, _) = two_goals();
    let n = h.app.data.extra.goals.len();
    h.click("New goal");
    h.type_text("Zero to zero");
    // from 0 to 0: it can't be measured, so the goal can't be made yet
    h.click("Target value entry");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("0");
    h.key(egui::Key::Enter);
    h.settle();
    assert!(
        !h.painted_text("The start and target values are the same")
            .is_empty(),
        "it says why"
    );
    h.click("Create goal");
    h.click("Another target");
    assert_eq!(h.app.data.extra.goals.len(), n, "not made");
    let Some(Modal::NewGoal(d)) = &h.app.modal else {
        panic!("still open")
    };
    assert!(d.more.is_empty(), "nor set aside as another target");
    // a target to aim for: it can be made
    h.click("Target value entry");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("5");
    h.key(egui::Key::Enter);
    h.click("Create goal");
    assert_eq!(h.app.data.extra.goals.len(), n + 1);

    // on a goal's page: a target value equal to the start isn't taken
    let id = add_goal_with(&mut h, "Signups", vec![number("People", 10.0, 50.0, 20.0)]);
    h.app.goal.open = Some(id);
    h.settle();
    h.click("Edit People");
    h.click("Number target");
    h.type_text("10");
    h.key(egui::Key::Enter);
    h.settle();
    assert!(matches!(
        goal(&h, id).targets[0].kind,
        TargetKind::Number { target, .. } if target == 50.0
    ));
    assert!(!h
        .painted_text("The start and target values are the same")
        .is_empty());
}

#[test]
fn words_on_a_light_goal_colour_are_dark_enough_to_read() {
    let (mut h, _, b) = two_goals();
    // yellow, where white words hardly show
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == b) {
        g.color = [0xf8, 0xae, 0x00];
    }
    h.click("Done Lease signed");
    h.key(egui::Key::Escape);
    h.settle();
    h.click("Achieved Office move");
    let (_, c) = h.painted_text("Share to team chat")[0];
    assert_ne!(c, Color32::WHITE, "not white on yellow");
    let (_, c) = h.painted_text("Done")[0];
    assert_ne!(c, Color32::WHITE, "the Done button either");
    // a dark colour keeps white words
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == b) {
        g.color = [0x5f, 0x55, 0xee];
    }
    h.settle();
    let (_, c) = h.painted_text("Share to team chat")[0];
    assert_eq!(c, Color32::WHITE);
}

#[test]
fn a_goal_achieved_after_its_deadline_says_how_late() {
    let (mut h, _, b) = two_goals();
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == b) {
        g.due = Some(today() - Duration::days(2));
    }
    h.click("Done Lease signed");
    assert!(
        !h.painted_text("Done 2 days after the deadline").is_empty(),
        "Goal achieved says it"
    );
    h.key(egui::Key::Escape);
    h.settle();
    let late = format!("Achieved {}, 2 days late", today().text("%b %-d"));
    assert!(!h.painted_text(&late).is_empty(), "so does its shelf card");
    h.click("Achieved Office move");
    assert!(
        !h.painted_text("Goal achieved, 2 days after the deadline")
            .is_empty(),
        "and its page"
    );
}

#[test]
fn a_milestone_i_reach_by_closing_a_task_anywhere_is_mine() {
    let (mut h, _, _) = two_goals();
    let me = h.app.data.me;
    let open: Vec<Task> = h
        .app
        .data
        .tasks
        .iter()
        .filter(|t| t.parent.is_none() && !h.app.data.is_closed(t) && !t.archived)
        .take(2)
        .cloned()
        .collect();
    let id = add_goal_with(
        &mut h,
        "Ship it",
        vec![GoalTarget {
            id: 0,
            name: "Launch tasks".into(),
            kind: TargetKind::Tasks {
                ids: open.iter().map(|t| t.id).collect(),
            },
        }],
    );
    h.app.goal_watch(now_ts());
    let milestone = |h: &Ui1, m: u8| {
        goal(h, id)
            .log
            .iter()
            .find(|e| e.what == GoalChange::Milestone(m))
            .map(|e| e.who)
    };
    // I close one in a list, not on the goal's page: half done, by me
    h.app.toggle_done(open[0].id);
    h.settle();
    assert!(h.app.data.is_closed(h.app.task(open[0].id).unwrap()));
    assert_eq!(milestone(&h, 50), Some(me));
    h.app.notify_clock(now_ts());
    let about = format!("goal:{}", id);
    assert!(
        h.app.notify_rt.sent.iter().all(|n| n.target != about),
        "not told what I did: {:?}",
        h.app.notify_rt.sent
    );

    // a teammate closes the other: it comes in from the team, and I'm told
    let mut theirs = serde_json::to_value(&h.app.data).unwrap();
    let tasks = theirs["tasks"].as_array_mut().unwrap();
    let t = tasks
        .iter_mut()
        .find(|t| t["id"].as_u64() == Some(open[1].id))
        .unwrap();
    t["status"] = serde_json::json!("complete");
    theirs["extra"]["sync"]["stamps"][open[1].id.to_string()] = serde_json::json!(i64::MAX / 2);
    h.app.merge_in(&[theirs], &[]);
    assert!(h.app.data.is_closed(h.app.task(open[1].id).unwrap()));
    assert_eq!(milestone(&h, 100), Some(0), "noticed, not mine");
    h.app.notify_clock(now_ts());
    assert!(
        h.app
            .notify_rt
            .sent
            .iter()
            .any(|n| n.title == "Goal achieved: Ship it"),
        "{:?}",
        h.app.notify_rt.sent
    );
}

#[test]
fn targets_and_history_lines_get_ids_as_they_are_made() {
    let (mut h, a, _) = two_goals();
    let all_have = |h: &Ui1| {
        h.app
            .data
            .extra
            .goals
            .iter()
            .all(|g| g.targets.iter().all(|t| t.id != 0) && g.log.iter().all(|e| e.id != 0))
    };
    // goals from a file before ids: given theirs on the first look
    h.app.goal_watch(now_ts());
    assert!(all_have(&h));
    let before: Vec<u64> = goal(&h, a).targets.iter().map(|t| t.id).collect();
    // a target added and a number recorded on the page
    h.click("Goal Beta launch");
    h.click("Add target");
    h.click("Add Done or not target");
    h.click("Plus 1 Beta testers");
    assert!(all_have(&h), "{:?}", goal(&h, a));
    let g = goal(&h, a).clone();
    assert_eq!(g.targets.len(), 3);
    assert_eq!(
        &g.targets.iter().map(|t| t.id).collect::<Vec<_>>()[..2],
        &before[..],
        "kept"
    );
    let mut ids: Vec<u64> = g.targets.iter().map(|t| t.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "each its own");
    // and they are kept through saving and reading back
    let back: Data = serde_json::from_value(serde_json::to_value(&h.app.data).unwrap()).unwrap();
    let gb = back.extra.goals.iter().find(|x| x.id == a).unwrap();
    assert_eq!(
        gb.targets.iter().map(|t| t.id).collect::<Vec<_>>(),
        g.targets.iter().map(|t| t.id).collect::<Vec<_>>()
    );
    // a new goal from the dialog has them at once too
    h.click("Back to goals");
    h.click("New goal");
    h.type_text("Fresh");
    h.click("Create goal");
    assert!(all_have(&h));
}

#[test]
fn a_milestone_noted_on_two_computers_shows_once() {
    let (mut h, _, _) = two_goals();
    let id = add_goal_with(&mut h, "Signups", vec![number("People", 0.0, 10.0, 6.0)]);
    h.app.goal_watch(now_ts());
    // both computers noted 50% before seeing each other's line; merged,
    // both lines are there
    let t = now_ts();
    for g in h.app.data.extra.goals.iter_mut().filter(|g| g.id == id) {
        g.log
            .retain(|e| !matches!(e.what, GoalChange::Milestone(_)));
        for (ts, who, line) in [(t - 5, 2, 9_001), (t - 9, 3, 9_002)] {
            g.log.push(GoalEvent {
                id: line,
                ts,
                who,
                what: GoalChange::Milestone(50),
                pct: 60,
            });
        }
        g.reached = 50;
    }
    h.app.goal_watch(now_ts());
    let lines: Vec<(u64, u64)> = goal(&h, id)
        .log
        .iter()
        .filter(|e| e.what == GoalChange::Milestone(50))
        .map(|e| (e.id, e.who))
        .collect();
    assert_eq!(
        lines,
        [(9_002, 3)],
        "the first one stays, on every computer"
    );
}

/// The Goals page, its list and a goal's own page stay inside every window
/// size, in both languages, with long names: in a narrow window the ring
/// goes over the goal's details, the filters go under the title, a
/// target's value goes under its name, and the list scrolls sideways. A
/// long target name wraps instead of being cut.
#[test]
fn goal_pages_fit_every_window_size() {
    use super::hub_layout::{assert_fits, each_size, page};
    const LONG_GOAL: &str = "Win three hundred new customers and more repeat orders this half";
    const LONG_TARGET: &str = "Beta testers signed up online and offline together";
    each_size(|w, hh, at| {
        let mut h = Ui1::sized(w, hh);
        h.app.data = two_goals().0.app.data.clone();
        let a = h.app.data.extra.goals[0].id;
        h.app.data.extra.goals[0].name = LONG_GOAL.into();
        h.app.data.extra.goals[0].targets[0].name = LONG_TARGET.into();
        h.app.section = Section::Goals;
        h.settle();
        h.settle();
        assert_fits(&h, page(&h), &format!("{at} goals page"));
        for n in ["New goal", "Show as list", "Show Achieved"] {
            assert!(h.rect(n).right() <= w, "{at}: {n} {:?}", h.rect(n));
        }
        // the list's columns don't squeeze: the row keeps its width and
        // scrolls in a narrow window
        h.click("Show as list");
        let row = h.rect(&format!("Goal row {LONG_GOAL}"));
        assert!(row.left() >= 0.0 && row.width() >= 600.0, "{at}: {row:?}");
        let cut = h.cut_texts(Rect::from_min_max(pos2(0.0, 40.0), pos2(w, row.top())));
        assert!(cut.is_empty(), "{at}: above the list {cut:?}");
        h.click("Show as cards");
        // the goal's own page, top and bottom
        h.app.goal.open = Some(a);
        h.settle();
        assert_fits(&h, page(&h), &format!("{at} goal page"));
        h.scroll_into_view("Done Store review");
        assert_fits(&h, page(&h), &format!("{at} goal page, its targets"));
        for n in [
            "Add target",
            &format!("Remove target {LONG_TARGET}"),
            &format!("Edit {LONG_TARGET}"),
            "Done Store review",
        ] {
            let r = h.rect(n);
            assert!(r.right() <= w, "{at}: {n} {r:?}");
        }
    });
}

/// A goal's pace sentence sits beside its pill, wrapping in its own column
/// there, or on a line under the pill in a narrow window; it never wraps
/// round under the pill.
#[test]
fn the_pace_sentence_is_beside_or_under_its_pill() {
    use super::hub_layout::{each_size, page};
    each_size(|w, hh, at| {
        let mut h = Ui1::sized(w, hh);
        h.app.data = two_goals().0.app.data.clone();
        let a = h.app.data.extra.goals[0].id;
        h.app.section = Section::Goals;
        h.app.goal.open = Some(a);
        h.settle();
        let pill = h.rect("At risk");
        let pace: Vec<Rect> = h
            .texts_in(page(&h), Seen::Visible)
            .into_iter()
            .filter(|(s, _)| s.contains('%') && s.contains('\u{b7}'))
            .map(|(_, r)| r)
            .collect();
        assert!(!pace.is_empty(), "{at}: the pace sentence");
        for r in pace {
            assert!(
                r.left() >= pill.right() || r.top() >= pill.bottom() - 1.0,
                "{at}: the pace sentence {r:?} wraps under the pill {pill:?}"
            );
        }
    });
}

/// The Goals page before there are any goals fits every window size: the
/// picture gives up its sides, and the three steps go one under another in
/// a narrow window instead of running past its right edge.
#[test]
fn the_empty_goals_page_fits_every_window_size() {
    use super::hub_layout::{assert_fits, each_size, page};
    each_size(|w, hh, at| {
        let mut h = Ui1::sized(w, hh);
        h.app.data.extra.goals.clear();
        h.app.section = Section::Goals;
        h.settle();
        assert_fits(&h, page(&h), &format!("{at} top"));
        for _ in 0..30 {
            h.frame(vec![egui::Event::PointerMoved(pos2(w / 2.0, hh / 2.0))]);
            h.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -200.0),
                modifiers: Default::default(),
            }]);
        }
        h.settle();
        assert!(!h.painted_text(tr("Record progress")).is_empty(), "{at}");
        assert_fits(&h, page(&h), &format!("{at} bottom"));
    });
}
