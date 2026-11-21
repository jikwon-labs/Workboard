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
    assert_eq!(h.app.goal_banner, Some((a, 50)), "the 50% banner shows");
    assert!(h.has("Close milestone"));
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
fn a_goals_settings_change_its_name_owner_and_colour() {
    let (mut h, a, _) = two_goals();
    h.click("Goal Beta launch");
    h.click("Goal color [16, 144, 224]");
    assert_eq!(goal(&h, a).color, [0x10, 0x90, 0xe0]);
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
    h.click("Open goal Office move");
    assert_eq!(h.app.section, Section::Goals);
    assert_eq!(h.app.goal_open, Some(b));
    assert!(
        h.has("Share to team chat"),
        "the achieved goal's page offers sharing too"
    );
}
