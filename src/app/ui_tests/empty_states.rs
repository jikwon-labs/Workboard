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
    let mut ui = Ui1::sized_with(w, h, data);
    ui.settle();
    ui
}

impl Ui1 {
    /// Every text painted in the last frame.
    fn texts(&self) -> Vec<String> {
        drawn_texts(&self.shapes)
    }

    /// Clicks the lowest control with this name (a dialog's button, not
    /// its title).
    fn click_lowest(&mut self, name: &str) {
        let r = self
            .tree
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .max_by(|a, b| a.center().y.total_cmp(&b.center().y))
            .unwrap_or_else(|| panic!("nothing named {:?}", name));
        self.click_at(r.center());
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
    assert_eq!(h.count_named("Create team"), 1, "one create button");
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

fn go_home(h: &mut Ui1) {
    h.app.actions.push(Action::Go(Location::Home));
    h.app.apply_actions();
    h.settle();
}

#[test]
fn home_my_work_with_nothing_says_so_and_adds_a_task() {
    let mut h = with(solo(), 1440.0, 900.0);
    go_home(&mut h);
    assert!(h.has("Nothing assigned to you yet."));
    assert!(
        !h.texts().iter().any(|x| x == "Unscheduled"),
        "no rows of 0"
    );
    h.click("+ Add a task for me");
    let qa = h.app.quick_add.as_ref().expect("the quick add opens");
    assert_eq!(qa.assignees, vec![1]);
    // with no list there is nowhere to add it: say where to start
    let mut h = with(no_space(), 1440.0, 900.0);
    go_home(&mut h);
    assert!(h.has("Tasks live in Lists: make one in the Lists card first."));
    assert!(!h.has("+ Add a task for me"));
    // anything of mine brings the rows back
    let mut h = Ui1::new();
    go_home(&mut h);
    assert!(!h.has("Nothing assigned to you yet."));
    assert!(h.texts().iter().any(|x| x == "Unscheduled"));
}

#[test]
fn home_shows_due_reminders_and_tomorrows_meeting() {
    let mut d = no_space();
    let now = now_ts();
    for (id, text, due, done) in [
        (5000, "File taxes", now - 2 * 86400, false),
        (5001, "Dentist", now - 86400, false),
        (5002, "Mom's birthday", now + 2 * 86400, false),
        (5003, "Old one", now - 86400, true),
    ] {
        d.extra.reminders.push(Reminder {
            id,
            text: text.into(),
            due_ts: due,
            done,
            owner: 1,
            alerted: true,
            ..Default::default()
        });
    }
    d.extra.meetings.push(Meeting {
        id: 6000,
        title: "Weekly sync".into(),
        date: today() + Duration::days(1),
        time: "10:00".into(),
        attendees: vec![1],
        agenda: String::new(),
        notes: String::new(),
    });
    d.extra.meetings.push(Meeting {
        id: 6001,
        title: "Next month review".into(),
        date: today() + Duration::days(30),
        time: "10:00".into(),
        attendees: vec![1],
        agenda: String::new(),
        notes: String::new(),
    });
    let mut h = with(d, 1440.0, 900.0);
    go_home(&mut h);
    assert!(!h.has("Nothing assigned to you yet."));
    let texts = h.texts();
    for x in [
        "Reminders",
        "File taxes",
        "Dentist",
        "Meetings",
        "Weekly sync",
    ] {
        assert!(texts.iter().any(|t| t == x), "{} on Home: {:?}", x, texts);
    }
    for x in ["Mom's birthday", "Old one", "Next month review"] {
        assert!(!texts.iter().any(|t| t == x), "{} is not due", x);
    }
    assert!(texts.iter().any(|t| t == "Tomorrow 10:00"));
    h.click("Weekly sync");
    assert_eq!(h.app.section, Section::Meetings);
    assert_eq!(h.app.meeting_open, Some(6000));
}

#[test]
fn empty_channel_alone_offers_invite_instead_of_add_people() {
    let mut h = with(solo(), 1440.0, 900.0);
    h.click("general");
    assert_eq!(h.app.section, Section::Chat);
    assert!(!h.has("+  Add People"), "nobody else to add");
    h.click("+  Invite people");
    assert!(matches!(h.app.modal, Some(Modal::Invite { .. })));
    // with teammates it adds them to the channel as before
    let mut h = Ui1::new();
    for c in &mut h.app.data.channels {
        c.messages.clear();
    }
    h.click("general");
    h.click("+  Add People");
    assert!(matches!(h.app.modal, Some(Modal::ChannelSettings(_))));
}

#[test]
fn task_comments_alone_do_not_suggest_mentions_or_assigning() {
    let d = solo();
    let list = d.spaces[0].lists[0].id;
    let mut h = with(d, 1440.0, 900.0);
    h.app.apply(Action::Add {
        list,
        name: "Write report".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: true,
    });
    h.app.apply_actions();
    h.settle();
    assert!(h.app.open_task.is_some());
    assert!(h
        .texts()
        .iter()
        .any(|x| x == "Keep notes and updates on this task here."));
    assert!(!h.has("@  Mention a teammate"));
    assert!(!h.has("\u{1F464}  Assign this task"));
    assert!(h.texts().iter().any(|x| x == "Write a comment"));
    // with teammates they are there
    let mut h = Ui1::new();
    h.open_clean_task();
    assert!(h.has("@  Mention a teammate"));
    assert!(h
        .texts()
        .iter()
        .any(|x| x == "Write a comment, @mention someone"));
}

#[test]
fn inbox_alone_with_due_work_points_to_today_and_overdue() {
    let d = solo();
    let list = d.spaces[0].lists[0].id;
    let mut h = with(d, 1440.0, 900.0);
    h.click("Inbox");
    assert!(h.has("Looking to collaborate?"), "nothing due: the invite");
    h.app.apply(Action::Add {
        list,
        name: "Send invoice".into(),
        status: None,
        priority: 0,
        assignees: vec![1],
        due: Some(today() - Duration::days(1)),
        parent: None,
        open: false,
    });
    h.settle();
    assert!(h.has("Nothing from other people"));
    assert!(h.has("Tasks due today or overdue: 1"));
    assert!(h.has("Invite people"), "inviting is still there");
    h.click("Open Today & Overdue");
    assert_eq!(h.app.loc, Location::TodayOverdue);
}

#[test]
fn welcome_on_this_mac_asks_for_a_name_without_a_team() {
    let mut h = with(Data::fresh(), 1440.0, 900.0);
    h.app.login = Some(crate::app::login::LoginScreen::start(""));
    h.settle();
    h.click(alone_button());
    assert!(h.has("What should we call you?"));
    assert!(!h.has("What should your team call you?"));
}

#[test]
fn timesheet_options_with_no_tasks_say_why() {
    let mut h = with(solo(), 1440.0, 900.0);
    h.app.section = Section::Timesheets;
    h.settle();
    h.click("All assigned tasks");
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("No open tasks are assigned to you")
    );
    h.click("Track time");
    assert!(h.app.modal.is_none(), "no empty task picker");
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Make a task first: time is tracked on tasks")
    );
    // and the Create Task window opens, so it says where
    assert!(
        h.app
            .quick_add
            .as_ref()
            .is_some_and(|q| q.sheet_week.is_some()),
        "a task made here goes on the sheet"
    );
}

#[test]
fn timesheet_says_what_it_is_for_and_how_to_fill_it() {
    const WHAT: &str = "Write down how long you worked on each task, day by day. Time from a task's timer is filled in for you.";
    const HOW: &str =
        "Add a task you worked on below, then type how long you spent under each day.";
    let says = |h: &Ui1, s: &str| h.texts().iter().any(|x| x == s);
    // no tasks at all: the start card says what a timesheet is
    let mut h = with(solo(), 1440.0, 900.0);
    h.app.section = Section::Timesheets;
    h.settle();
    assert!(says(&h, WHAT), "{:?}", h.texts());
    // an empty grid says how to fill it in
    h.click("Individual tasks");
    assert!(says(&h, HOW), "{:?}", h.texts());
    // nothing to add: the menu says why instead of opening as a bare dot,
    // and a task can be made right there
    h.click("+ Add task");
    assert!(says(&h, "No tasks yet: time is tracked on tasks"));
    h.click("+ New task");
    let qa = h
        .app
        .quick_add
        .as_ref()
        .expect("the Create Task window opens");
    assert_eq!(qa.assignees, vec![h.app.data.me], "a task of mine");
    h.type_text("Write the report");
    h.key(egui::Key::Enter);
    assert!(h.app.quick_add.is_none());
    assert!(
        h.has("Remove row Write the report"),
        "the new task is on this week's sheet"
    );
    assert!(!says(&h, HOW));

    // with tasks: once a row is in, the hint goes
    let mut h = Ui1::new();
    let td = today();
    let ws = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
    for t in &mut h.app.data.tasks {
        t.time_entries.clear();
    }
    h.app.section = Section::Timesheets;
    h.app.hub_filter = 0;
    h.app.sheet_week = ws;
    h.app.sheet_entries = false;
    h.settle();
    h.click("Individual tasks");
    assert!(says(&h, HOW));
    h.click("+ Add task");
    h.click("Renew passport");
    assert!(h.has("Remove row Renew passport"));
    assert!(!says(&h, HOW), "the hint goes once a row is in");
    // every task on the sheet: the menu says so
    let all: Vec<u64> = h.app.data.tasks.iter().map(|t| t.id).collect();
    h.app.add_sheet_rows(ws, all);
    h.settle();
    h.click("+ Add task");
    assert!(says(&h, "Every task is already on this week's timesheet"));
}

#[test]
fn dark_mode_tag_chips_are_readable() {
    let card = crate::theme::theme(true).card;
    for c in STATUS_PALETTE.iter().skip(1) {
        let c = rgb(*c);
        // the chip's tint (hovered, the stronger one) over a dark card
        let mix = |x: u8, y: u8| (x as f32 * 0.28 + y as f32 * 0.72) as u8;
        let bg = Color32::from_rgb(
            mix(c.r(), card.r()),
            mix(c.g(), card.g()),
            mix(c.b(), card.b()),
        );
        let text = crate::widgets::chip_text_on_dark(c);
        assert!(contrast(text, bg) >= 4.5, "{:?} on {:?}", text, bg);
    }
    // and the chips really use it in the dark theme
    let mut h = Ui1::new();
    h.app.data.dark = true;
    h.app.open_settings("Tag manager");
    h.settle();
    let tag = h.app.data.tasks.iter().flat_map(|t| t.tags.clone()).next();
    let tag = tag.expect("the example data has tags");
    let painted = h.painted_text(&tag);
    assert!(!painted.is_empty(), "{} chip", tag);
    let want = crate::widgets::chip_text_on_dark(tag_color(&tag));
    assert!(painted.iter().all(|p| p.1 == want), "{:?}", painted);
}

impl Ui1 {
    /// Every shape painted in the last frame, nested ones included.
    fn all_shapes(&self) -> Vec<egui::Shape> {
        fn walk(shape: &egui::Shape, out: &mut Vec<egui::Shape>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                s => out.push(s.clone()),
            }
        }
        let mut out = vec![];
        for s in &self.shapes {
            walk(&s.shape, &mut out);
        }
        out
    }
}

#[test]
fn dark_mode_dialogs_and_theme_swatches_stand_out() {
    let dark = crate::theme::theme(true);
    let mut h = Ui1::new();
    h.app.data.dark = true;
    h.app.data.extra.auto_theme = false;
    h.app.modal = Some(Modal::Themes);
    h.settle();
    let shapes = h.all_shapes();
    // the dialog's backdrop is the theme's overlay
    let screen = Rect::from_min_size(pos2(0.0, 0.0), h.size);
    assert!(shapes.iter().any(|s| matches!(s,
        egui::Shape::Rect(r) if r.rect == screen && r.fill == dark.overlay)));
    // every theme colour swatch, "black" too, has an outline
    let rings = shapes
        .iter()
        .filter(|s| {
            matches!(s,
            egui::Shape::Circle(c) if c.radius == 13.0 && c.stroke.color == dark.text3)
        })
        .count();
    assert_eq!(rings, crate::theme::ACCENTS.len());
}

#[test]
fn a_new_workspace_shows_its_colour_chosen() {
    let mut d = Data::fresh();
    d.extra.welcome = false;
    let start = d.extra.workspace_color;
    let mut h = with(d, 1440.0, 900.0);
    let text = h.app.t().text;
    h.app.modal = Some(Modal::WorkspaceSettings {
        name: "My Workspace".into(),
    });
    h.settle();
    let chosen = |h: &Ui1| {
        h.all_shapes()
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r)
                    if r.stroke.width == 2.0
                        && r.stroke.color == text
                        && r.rect.width() == 22.0 =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .count()
    };
    assert_eq!(chosen(&h), 1, "the colour it has is marked");
    // a palette colour picked later is marked in its place
    h.app.data.extra.workspace_color = STATUS_PALETTE[3];
    h.settle();
    assert_eq!(chosen(&h), 1);
    assert_ne!(start, STATUS_PALETTE[3]);
}

#[test]
fn mark_all_as_read_is_off_with_nothing_unread() {
    let mut h = with(solo(), 1440.0, 900.0);
    h.app.actions.push(Action::Go(Location::Replies));
    h.app.apply_actions();
    h.settle();
    let before = h.app.data.extra.replies_seen;
    h.click("Mark all as read");
    assert_eq!(h.app.data.extra.replies_seen, before, "nothing to mark");
    // a reply from someone else turns it on
    let mut h = Ui1::new();
    let other = h.app.data.members[1].id;
    let ts = now_ts();
    h.app.data.extra.replies_seen = ts - 100;
    h.app.data.tasks[0].activity.push(Activity {
        ts: ts - 10,
        author: other,
        text: "Looks good".into(),
        comment: true,
        ..Default::default()
    });
    h.app.actions.push(Action::Go(Location::Replies));
    h.app.apply_actions();
    h.settle();
    h.click("Mark all as read");
    assert!(h.app.data.extra.replies_seen >= ts);
}

#[test]
fn new_dashboard_name_field_is_labelled_and_create_waits_for_a_name() {
    let mut h = Ui1::new();
    let n = h.app.data.extra.dashboards.len();
    h.app.modal = Some(Modal::NewDashboard {
        name: String::new(),
    });
    h.settle();
    assert!(h.texts().iter().any(|x| x == "Dashboard name"));
    assert!(h
        .texts()
        .iter()
        .any(|x| x == "For example: Weekly overview"));
    h.click_lowest("Create Dashboard");
    assert_eq!(
        h.app.data.extra.dashboards.len(),
        n,
        "no name, nothing made"
    );
    assert!(matches!(h.app.modal, Some(Modal::NewDashboard { .. })));
    h.click("Dashboard name entry");
    h.type_text("Weekly");
    h.click_lowest("Create Dashboard");
    assert_eq!(h.app.data.extra.dashboards.len(), n + 1);
    assert_eq!(h.app.data.extra.dashboards.last().unwrap().name, "Weekly");
}
