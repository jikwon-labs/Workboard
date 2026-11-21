// Click-through tests for archived Lists and tasks: they leave the sidebar,
// pickers and counts, and each screen says where they went and how to get
// them back.

use super::*;

#[test]
fn archived_favorite_leaves_the_sidebar_and_its_page_says_archived() {
    let h0 = Ui1::new();
    assert!(h0.has("Website Redesign"), "a favorite in the sidebar");
    let mut h = archived_workspace();
    let web = list_id(&h, "Website Redesign");
    assert!(h.app.data.favorites.contains(&Location::List(web)));
    assert!(!h.has("Website Redesign"), "archived: out of Favorites");
    // opened another way (a link), the page says it is archived and restores it
    h.app.actions.push(Action::Go(Location::List(web)));
    h.settle();
    assert!(h.has("Restore archived here"));
    h.click("Restore archived here");
    assert!(!h.app.data.list_archived(web));
    h.app.loc = Location::Home;
    h.settle();
    assert!(h.has("Website Redesign"), "back in Favorites");
}

#[test]
fn views_with_only_archived_tasks_say_so_and_show_the_way_back() {
    let mut h = archived_workspace();
    h.click("All Tasks");
    assert!(!h.has("No tasks here yet"));
    let n = h
        .app
        .data
        .tasks
        .iter()
        .filter(|t| t.parent.is_none())
        .count();
    assert!(h.has(&format!("All {} tasks here are archived", n)));
    // the tasks sit in archived Lists: they come back from Settings > Spaces
    assert!(!h.has("Show archived tasks"));
    h.click("See archived Lists");
    assert_eq!(h.app.section, Section::Settings);
    // a List that is not archived, with only archived tasks: the Archived filter
    let mut h = Ui1::new();
    let web = list_id(&h, "Website Redesign");
    for t in h.app.data.tasks.iter_mut().filter(|t| t.list_id == web) {
        t.archived = true;
    }
    h.app.actions.push(Action::Go(Location::List(web)));
    h.settle();
    h.click("Show archived tasks");
    assert!(h.app.filter.archived);
    assert!(h.has("Task Design new homepage layout"));
}

#[test]
fn home_lists_card_tells_archived_lists_from_none_and_empty_lists_have_no_bar() {
    let mut h = archived_workspace();
    assert!(!h.has("No Lists yet. Lists keep a Space's tasks together."));
    assert!(h.has("All 4 Lists are archived."));
    h.click("Home archived lists");
    assert_eq!(h.app.section, Section::Settings);
    // an empty List says so instead of "0/0" beside an empty bar
    let mut h = Ui1::new();
    h.app.data.tasks.clear();
    h.app.section = Section::Home;
    h.app.loc = Location::Home;
    h.settle();
    assert!(h.painted_text("0/0").is_empty());
    assert!(!h.painted_text("No tasks yet").is_empty());
}

#[test]
fn settings_counts_leave_out_archived_and_say_how_many() {
    let mut h = archived_workspace();
    h.app.open_settings("Spaces");
    h.settle();
    // nothing live is left to count; the archived List shows its Folder
    assert!(h.has("Team Space / Projects / Website Redesign"));
    let team = h
        .app
        .data
        .lists_in(Location::Space(h.app.data.spaces[0].id))
        .unwrap();
    let before = h
        .app
        .data
        .tasks
        .iter()
        .filter(|t| team.contains(&t.list_id))
        .count();
    assert!(before > 4);
    assert!(
        !h.has(&before.to_string()),
        "archived tasks are not counted in the Spaces table"
    );
    h.app.open_settings("General");
    h.settle();
    let n = h.app.data.tasks.len();
    let people = h.app.data.members.len();
    assert!(h.has(&format!(
        "{} people \u{2022} 2 spaces \u{2022} 0 tasks \u{2022} {} archived",
        people, n
    )));
}

#[test]
fn sidebar_says_when_a_folder_or_space_holds_only_archived_lists() {
    let mut h = archived_workspace();
    for s in h.app.data.spaces.iter_mut() {
        s.expanded = true;
    }
    h.settle();
    // the Projects folder (2 Lists) and Side Projects (1 List) look empty otherwise
    assert!(h.has("2 archived Lists"));
    assert!(h.has("1 archived List"));
    h.click("2 archived Lists");
    assert_eq!(h.app.section, Section::Settings);
    // nothing archived: no such rows
    let h = Ui1::new();
    assert!(!h.tree.iter().any(|(n, _)| n.contains("archived List")));
}

#[test]
fn time_on_archived_tasks_counts_on_the_dashboard_and_is_marked_in_timesheets() {
    let mut h = archived_workspace();
    // today at 00:01, so the entry is in this week whenever the test runs
    let start = today()
        .and_hms_opt(0, 1, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .single()
        .unwrap()
        .timestamp();
    let i = h
        .app
        .data
        .tasks
        .iter()
        .position(|t| t.parent.is_none())
        .unwrap();
    let name = h.app.data.tasks[i].name.clone();
    h.app.data.tasks[i].time_entries = vec![TimeEntry {
        start,
        end: start + 7200,
        member: h.app.data.me,
    }];
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Week");
    let did = h.app.dash_open.unwrap();
    let d = h
        .app
        .data
        .extra
        .dashboards
        .iter_mut()
        .find(|d| d.id == did)
        .unwrap();
    d.cards = Some(vec![DashCard {
        kind: "time".into(),
        title: String::new(),
    }]);
    h.settle();
    fn texts(h: &Ui1) -> Vec<String> {
        drawn_texts(&h.shapes)
    }
    // (the seed has some time of its own today too)
    assert!(
        texts(&h).iter().any(|x| x.starts_with("2h")),
        "the dashboard counts the time Timesheets shows"
    );
    h.app.section = Section::Timesheets;
    h.settle();
    assert!(!h.painted_text(&format!("{} (archived)", name)).is_empty());
}

#[test]
fn dashboard_time_this_week_follows_monday_weeks() {
    // "Time tracked this week" counts the same week as the "This week"
    // filter: from Monday with the Labs option (it used to start on Sunday)
    let mut h = Ui1::new();
    h.app.data.set_lab(Lab::WeekMonday, true);
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Week");
    let did = h.app.dash_open.unwrap();
    let d = h
        .app
        .data
        .extra
        .dashboards
        .iter_mut()
        .find(|d| d.id == did)
        .unwrap();
    d.cards = Some(vec![DashCard {
        kind: "time".into(),
        title: String::new(),
    }]);
    h.settle();
    let ws = h.app.data.week_start(today());
    assert_eq!(ws.weekday(), chrono::Weekday::Mon);
    let texts = drawn_texts(&h.shapes);
    let day = |d: NaiveDate| d.format("%a %-d").to_string();
    for i in 0..7 {
        let d = ws + Duration::days(i);
        assert!(texts.contains(&day(d)), "{} on the card", day(d));
    }
    let before = ws - Duration::days(1);
    assert!(
        !texts.contains(&day(before)),
        "{} is last week",
        day(before)
    );
}

#[test]
fn move_dialogs_leave_out_archived_lists_and_say_when_there_is_nowhere_else() {
    let mut h = archived_workspace();
    let ideas = list_id(&h, "Ideas");
    let web = list_id(&h, "Website Redesign");
    h.app.data.list_item_mut(ideas).unwrap().archived = false;
    let tid = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.list_id == web)
        .unwrap()
        .id;
    h.app.modal = Some(Modal::MoveTasks {
        ids: vec![tid],
        query: String::new(),
    });
    h.settle();
    assert!(h.has("Side Projects / Ideas"));
    assert!(
        !h.has("Team Space / Projects / Marketing"),
        "archived: not a place to move to"
    );
    // a task already in the only List there is: nowhere else to go, and it says so
    let tid = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.list_id == ideas)
        .unwrap()
        .id;
    h.app.modal = Some(Modal::MoveTasks {
        ids: vec![tid],
        query: String::new(),
    });
    h.settle();
    assert!(h.has("There is no other List to move to yet. Make one first."));
    // Move List with one Space and no Folder: the same
    let mut h = Ui1::new();
    let sid = h.app.data.spaces[0].id;
    h.app.data.spaces.retain(|s| s.id == sid);
    h.app.data.spaces[0].folders.clear();
    let l = h.app.data.spaces[0].lists[0].id;
    h.app.modal = Some(Modal::MoveList { list: l });
    h.settle();
    assert!(h.has("There is no other Space or Folder to move it to yet. Make one first."));
    // with somewhere else to go, no such line
    let mut h = Ui1::new();
    let l = list_id(&h, "Ideas");
    h.app.modal = Some(Modal::MoveList { list: l });
    h.settle();
    assert!(!h.has("There is no other Space or Folder to move it to yet. Make one first."));
}

#[test]
fn form_list_picker_marks_an_archived_list_already_picked() {
    let mut h = Ui1::new();
    let web = list_id(&h, "Website Redesign");
    h.app.new_form();
    let fid = h.app.form_open.unwrap();
    h.app
        .data
        .extra
        .forms
        .iter_mut()
        .find(|f| f.id == fid)
        .unwrap()
        .list_id = web;
    h.app.data.list_item_mut(web).unwrap().archived = true;
    h.settle();
    assert!(!h.painted_text("Website Redesign (archived)").is_empty());
}

#[test]
fn search_says_how_many_archived_matches_it_leaves_out() {
    let mut h = archived_workspace();
    h.app.palette = Some("Website".into());
    h.settle();
    assert!(!h.has("Team Space / Projects / Website Redesign"));
    // the List and the archived task with that word in its name
    let n = 1 + h
        .app
        .data
        .tasks
        .iter()
        .filter(|t| t.name.to_lowercase().contains("website"))
        .count();
    let text = if n == 1 {
        "1 archived match is not shown".to_string()
    } else {
        format!("{} archived matches are not shown", n)
    };
    assert!(h.has(&text), "{}", text);
    // nothing archived: no such line
    let mut h = Ui1::new();
    h.app.palette = Some("Website".into());
    h.settle();
    assert!(!h.tree.iter().any(|(n, _)| n.contains("archived match")));
}

#[test]
fn inbox_marks_comments_on_archived_tasks() {
    let mut h = Ui1::new();
    let tid = h.app.data.tasks[1].id;
    let name = h.app.data.tasks[1].name.clone();
    let ts = now_ts() - 30;
    h.app.data.tasks[1].activity.push(Activity {
        ts,
        author: 2,
        text: "Over to you".into(),
        comment: true,
        post: false,
        ..Default::default()
    });
    h.app
        .data
        .extra
        .inbox_later
        .push(format!("{}:{}:2", tid, ts));
    h.app.data.tasks[1].archived = true;
    h.click("Inbox");
    h.click("Inbox Later");
    assert!(!h.painted_text(&format!("{} (archived)", name)).is_empty());
}
