// Click-through tests for screens with little or nothing in them: a new
// workspace, one person alone, no spaces, no tasks. Each screen should say
// what is going on and offer a next step that works. Also small details
// that made such screens read wrong (counts, reminders, dark mode).

use super::*;

/// A workspace with only me in it, one space and list, and no tasks.
fn solo() -> Data {
    Data::blank(
        "Solo",
        Member {
            id: 1,
            name: "Me".into(),
            color: [0x5f, 0x55, 0xee],
            role: "Owner".into(),
            ..Default::default()
        },
    )
}

/// The same with no space or list at all.
fn no_space() -> Data {
    let mut d = solo();
    d.spaces.clear();
    d
}

fn with(data: Data, w: f32, h: f32) -> Ui1 {
    let mut ui = Ui1::sized(w, h);
    ui.app = App::build(None, Some(data));
    ui.settle();
    ui
}

impl Ui1 {
    /// Every text painted in the last frame.
    fn texts(&self) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Text(ts) => out.push(ts.galley.text().to_string()),
                _ => {}
            }
        }
        let mut out = vec![];
        for s in &self.shapes {
            walk(&s.shape, &mut out);
        }
        out
    }

    fn count_named(&self, name: &str) -> usize {
        self.tree.iter().filter(|(n, _)| n == name).count()
    }
}

#[test]
fn docs_page_does_not_offer_the_task_csv_importer() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Docs");
    assert_eq!(h.app.section, Section::Docs);
    assert!(h.has("New Doc"));
    assert!(!h.has("\u{2B07}  Import"), "Import read tasks into a List");
}

#[test]
fn dashboard_status_card_shows_zeros_with_no_space() {
    let mut h = with(no_space(), 1440.0, 1400.0);
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Board");
    h.settle();
    let texts = h.texts();
    for st in ["TO DO", "IN PROGRESS", "COMPLETE"] {
        assert!(texts.iter().any(|x| x == st), "{} row: {:?}", st, texts);
    }
}

#[test]
fn open_tasks_per_person_agree_on_dashboard_and_teams() {
    let mut h = Ui1::sized(1440.0, 1400.0);
    // a subtask assigned to someone counts as their work on both pages
    let who = h.app.data.members[1].id;
    let parent = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.parent.is_none() && h.app.data.task_live(t))
        .unwrap()
        .clone();
    h.app.apply(Action::Add {
        list: parent.list_id,
        name: "Check the numbers".into(),
        status: None,
        priority: 0,
        assignees: vec![who],
        due: None,
        parent: Some(parent.id),
        open: false,
    });
    let d = &h.app.data;
    let mut expected: Vec<usize> = d
        .members
        .iter()
        .map(|m| {
            d.tasks
                .iter()
                .filter(|x| x.assignees.contains(&m.id) && !d.is_closed(x) && d.task_live(x))
                .count()
        })
        .collect();
    expected.sort();
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Board");
    h.settle();
    let mut dash: Vec<usize> = h
        .texts()
        .iter()
        .filter_map(|x| x.strip_suffix(" open")?.parse().ok())
        .collect();
    dash.sort();
    assert_eq!(dash, expected, "dashboard workload");
    h.app.section = Section::Teams;
    h.app.hub_filter = 3;
    h.settle();
    let mut teams: Vec<usize> = h
        .texts()
        .iter()
        .filter_map(|x| x.split_once(" open  \u{2022}")?.0.parse().ok())
        .collect();
    teams.sort();
    assert_eq!(teams, expected, "Teams > Analytics");
}

#[test]
fn workload_bars_say_what_they_compare_and_go_away_when_alone() {
    let caption = "Bars compare open tasks with the busiest person.";
    let mut h = Ui1::sized(1440.0, 1400.0);
    h.app.section = Section::Teams;
    h.app.hub_filter = 3;
    h.settle();
    assert!(h.has(caption), "a team sees what the bars mean");
    // alone, with one open task: no full bar beside "done 0"
    let d = solo();
    let list = d.spaces[0].lists[0].id;
    let mut h = with(d, 1440.0, 1400.0);
    h.app.apply(Action::Add {
        list,
        name: "Write report".into(),
        status: None,
        priority: 0,
        assignees: vec![1],
        due: None,
        parent: None,
        open: false,
    });
    h.app.section = Section::Teams;
    h.app.hub_filter = 3;
    h.settle();
    assert!(!h.has(caption));
    assert!(h.texts().iter().any(|x| x.starts_with("1 open  \u{2022}")));
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Board");
    h.settle();
    assert!(!h.has(caption));
    assert!(h.texts().iter().any(|x| x == "1 open"));
}

#[test]
fn settings_spaces_with_none_says_so_and_offers_new_space() {
    let mut h = with(no_space(), 1440.0, 900.0);
    h.app.open_settings("Spaces");
    h.settle();
    assert!(h.has(
        "No spaces yet. A Space holds your Lists and their tasks: make one with + New Space."
    ));
    assert!(!h.has("Lists"), "no table header over nothing");
    h.click("+ New Space");
    assert!(matches!(h.app.modal, Some(Modal::NewSpace { .. })));
}

#[test]
fn teams_page_alone_leads_with_inviting_people() {
    let mut h = with(solo(), 1440.0, 900.0);
    h.app.section = Section::Teams;
    h.app.hub_filter = 0;
    h.settle();
    assert!(h.has("Invite people to make teams"));
    assert_eq!(h.count_named("Create Team"), 1, "one create button");
    h.click("Invite people");
    assert!(matches!(h.app.modal, Some(Modal::Invite { .. })));
    // with other people in the workspace the usual page shows
    let mut h = Ui1::new();
    h.app.data.extra.teams.clear();
    h.app.section = Section::Teams;
    h.app.hub_filter = 0;
    h.settle();
    assert!(h.has("Manage teams and see their work"));
}

#[test]
fn snooze_never_moves_a_reminder_earlier() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    for (id, text, days) in [(601, "Pay rent", 3), (602, "Renew lease", 30)] {
        h.app.data.extra.reminders.push(Reminder {
            id,
            text: text.into(),
            due_ts: now_ts() + days * 86400,
            owner: me,
            ..Default::default()
        });
    }
    h.click("Inbox");
    h.click("Snooze Pay rent");
    assert!(!h.has("In 1 hour"), "earlier than it is due");
    assert!(!h.has("Tomorrow 9 AM"));
    h.click("Next week");
    let r = h.app.data.extra.reminders.iter().find(|r| r.id == 601);
    assert!(r.unwrap().due_ts > now_ts() + 3 * 86400);
    // nothing on offer is later than a month away
    assert!(!h.has("Snooze Renew lease"));
    assert!(h.has("Edit Renew lease"));
}
