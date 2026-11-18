// Words on screen that have to fit what they describe: open tasks aren't
// "in progress", new items get names in the screen's language, rows show
// what they are and counts read right. Clicked through like ui_tests.

use super::*;
use crate::i18n::{set_lang, Lang};

/// Every text painted in the last frame.
fn painted(h: &Ui1) -> Vec<(String, Rect)> {
    let mut out = vec![];
    each_text(&h.shapes, |t, _| {
        out.push((t.galley.text().to_string(), t.visual_bounding_rect()))
    });
    out
}

fn shows(h: &Ui1, text: &str) -> bool {
    painted(h).iter().any(|(t, _)| t == text)
}

#[test]
fn open_tasks_and_active_people_are_not_called_in_progress_in_korean() {
    let mut h = Ui1::new();
    set_lang(Lang::Ko);
    h.app.section = Section::Dashboards;
    h.app.dash_open = Some(super::hubs::WORKSPACE_OVERVIEW);
    h.settle();
    // the stats card counts open tasks; "진행 중" is the status below it
    assert!(shows(&h, "열린 작업"), "the open count says 열린 작업");
    h.app.section = Section::People;
    h.settle();
    assert!(shows(&h, "활성"), "a joined member is 활성");
    assert!(!shows(&h, "진행"), "not 진행, the status kind's word");
    set_lang(Lang::En);
}

#[test]
fn task_picker_rows_show_only_the_task_name() {
    let mut h = Ui1::new();
    h.click_left("Task Design new homepage layout", 120.0);
    h.click("\u{2026}");
    h.click("Make subtask of...");
    assert!(shows(&h, "Plan Q1 campaign"));
    let drawn: Vec<String> = painted(&h)
        .into_iter()
        .map(|(t, _)| t)
        .filter(|t| {
            h.app
                .data
                .tasks
                .iter()
                .any(|x| *t == format!("Pick {}", x.name))
        })
        .collect();
    assert!(
        drawn.is_empty(),
        "the accessibility name isn't drawn: {:?}",
        drawn
    );
    // still found and clicked by that name
    h.click("Pick Plan Q1 campaign");
    assert!(h.app.modal.is_none());
}

#[test]
fn new_items_are_named_in_the_screen_language() {
    let mut h = Ui1::new();
    set_lang(Lang::Ko);
    h.app.apply(Action::NewSpace);
    let sp = h.app.data.spaces.last().unwrap();
    assert_eq!(sp.name, "새 스페이스");
    assert_eq!(sp.lists[0].name, "목록");
    let sid = sp.id;
    h.app.apply(Action::NewFolder(sid));
    let f = h.app.data.spaces.last().unwrap().folders.last().unwrap();
    assert_eq!(
        (f.name.as_str(), f.lists[0].name.as_str()),
        ("새 폴더", "목록")
    );
    h.app.new_form();
    let form = h.app.data.extra.forms.last().unwrap();
    assert_eq!(form.name, "새 양식");
    let qs: Vec<&str> = form.fields.iter().map(|f| f.label.as_str()).collect();
    assert_eq!(qs, ["제목", "자세한 내용"]);
    assert!(!form.description.is_ascii(), "{}", form.description);
    let owner = h.app.data.member(h.app.data.me).cloned().unwrap();
    let blank = Data::blank("혼자 쓰는 곳", owner.clone());
    assert_eq!(
        blank.spaces[0].name, "기본 스페이스",
        "no team in a one-person place"
    );
    set_lang(Lang::En);
    assert_eq!(Data::blank("Mine", owner).spaces[0].name, "Main space");
    // names already saved stay as they are
    assert!(h.app.data.spaces.iter().any(|s| s.name == "Team Space"));
}

#[test]
fn a_new_workspace_leaves_out_online_only_tools_but_saved_ones_keep_theirs() {
    let owner = Member {
        id: 1,
        name: "Me".into(),
        role: "Owner".into(),
        ..Default::default()
    };
    let d = Data::blank("Mine", owner.clone());
    assert_eq!(d.extra.pinned_tools, ["create_task"]);
    assert!(d.extra.hidden_nav.iter().any(|x| x == "Skills"));
    assert!(d.extra.hidden_nav.iter().any(|x| x == "Artifacts"));
    let mut h = Ui1::new();
    h.app = App::build(None, Some(d.clone()));
    h.settle();
    assert!(
        !h.has("Skills") && !h.has("Artifacts"),
        "not in the sidebar"
    );
    assert!(
        !h.has("Record a Clip") && !h.has("Talk to Text"),
        "not pinned"
    );
    // Customize still offers them
    h.app.modal = Some(Modal::CustomizeSidebar);
    h.settle();
    h.click("Skills");
    assert!(!h.app.data.extra.hidden_nav.iter().any(|x| x == "Skills"));
    // a file saved before keeps what it had; one from before the fields existed gets the old defaults
    let mut v = serde_json::to_value(&d).unwrap();
    v["extra"]["pinned_tools"] = serde_json::json!(["create_task", "clip", "stt"]);
    v["extra"]["hidden_nav"] = serde_json::json!([]);
    let saved: Data = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(saved.extra.pinned_tools, ["create_task", "clip", "stt"]);
    assert!(saved.extra.hidden_nav.is_empty());
    let o = v["extra"].as_object_mut().unwrap();
    o.remove("pinned_tools");
    o.remove("hidden_nav");
    let old: Data = serde_json::from_value(v).unwrap();
    assert_eq!(old.extra.pinned_tools, ["create_task", "clip", "stt"]);
    assert!(old.extra.hidden_nav.is_empty());
}

#[test]
fn add_channel_is_left_off_views_that_gather_tasks_and_share_names_them() {
    let mut h = Ui1::new();
    // the sidebar has its own "Add Channel" row, left of 320
    let on_page = |h: &Ui1| {
        h.tree
            .iter()
            .any(|(n, r)| n == "Add Channel" && r.left() >= 320.0)
    };
    for loc in [
        Location::TodayOverdue,
        Location::MyTasks,
        Location::Everything,
    ] {
        h.app.apply(Action::Go(loc));
        h.settle();
        assert!(!on_page(&h), "no Add Channel on {:?}", loc);
    }
    let list = h.app.data.first_list().unwrap();
    h.app.apply(Action::Go(Location::List(list)));
    h.settle();
    assert!(on_page(&h), "a List still has it");
    set_lang(Lang::Ko);
    h.app.apply(Action::Go(Location::TodayOverdue));
    h.app.modal = Some(Modal::Share {
        emails: String::new(),
    });
    h.settle();
    assert!(
        shows(
            &h,
            "워크스페이스 사람들과 공유합니다: \"오늘 및 기한 지남\""
        ),
        "{:?}",
        painted(&h)
            .iter()
            .filter(|(t, _)| t.contains("공유"))
            .collect::<Vec<_>>()
    );
    set_lang(Lang::En);
}

#[test]
fn the_general_channel_is_called_general_everywhere() {
    let mut h = Ui1::new();
    h.click("general");
    assert!(shows(&h, "# general"), "the header names the channel");
    set_lang(Lang::Ko);
    h.settle();
    assert!(
        painted(&h).iter().any(|(t, _)| t.starts_with("general")),
        "the composer writes to general, not 일반"
    );
    assert!(!painted(&h).iter().any(|(t, _)| t.contains("일반")));
    set_lang(Lang::En);
}

#[test]
fn one_of_something_is_not_plural() {
    let mut h = Ui1::new();
    let id = h.app.data.tasks[0].id;
    h.app.data.tasks.iter_mut().for_each(|t| t.tags.clear());
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .tags
        .push("solo-tag".into());
    h.app.open_settings("Tag manager");
    h.settle();
    assert!(shows(&h, "1 task"), "Tag manager says 1 task");
    assert!(!shows(&h, "1 tasks"));
}

#[test]
fn bulk_delete_mentions_subtasks_only_when_there_are_some() {
    let mut h = Ui1::new();
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Select Write copy for landing page");
    h.key(egui::Key::Backspace);
    assert!(h.app.confirm_bulk_delete);
    assert!(shows(
        &h,
        "You can undo right after, or restore it from Trash."
    ));
    assert!(!painted(&h).iter().any(|(t, _)| t.contains("subtasks")));
    h.key(egui::Key::Escape);
    h.click("Select Design new homepage layout");
    h.key(egui::Key::Backspace);
    assert!(shows(
        &h,
        "Their subtasks go too. You can undo right after, or restore them from Trash."
    ));
}

#[test]
fn unread_replies_show_a_count_beside_replies() {
    let mut h = Ui1::new();
    h.app.data.extra.replies_seen = now_ts();
    h.settle();
    let row = h.rect("Replies");
    let count_in_row = |h: &Ui1| {
        painted(h)
            .iter()
            .any(|(t, r)| t == "1" && row.contains(r.center()))
    };
    assert!(!count_in_row(&h), "nothing unread yet");
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != h.app.data.me)
        .unwrap()
        .id;
    h.app.data.tasks[0].activity.push(Activity {
        ts: now_ts() + 5,
        author: other,
        text: "Looks good".into(),
        comment: true,
        ..Default::default()
    });
    h.settle();
    assert_eq!(h.app.unread_replies(), 1);
    assert!(count_in_row(&h), "1 beside Replies");
    h.click("Replies");
    h.click("Mark all as read");
    h.app.data.extra.replies_seen = now_ts() + 10;
    h.settle();
    assert!(!count_in_row(&h));
}

#[test]
fn timesheet_cells_and_totals_use_hours_and_minutes() {
    assert_eq!(fmt_hm(40 * 60), "40m");
    assert_eq!(fmt_hm(2400 + 3600), "1h 40m");
    assert_eq!(fmt_hm(7200), "2h");
    assert_eq!(fmt_hm(0), "-");
    assert_eq!(
        super::taskfields::parse_estimate("1\u{c2dc}\u{ac04} 30\u{bd84}"),
        Some(90)
    );
    set_lang(Lang::Ko);
    assert_eq!(fmt_hm(2400 + 3600), "1시간 40분");
    set_lang(Lang::En);
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let ws = today() - Duration::days(today().weekday().num_days_from_monday() as i64);
    let start = ws
        .and_hms_opt(10, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .unwrap()
        .timestamp();
    let t = &mut h.app.data.tasks[0];
    t.assignees = vec![me];
    t.time_entries.push(TimeEntry {
        start,
        end: start + 40 * 60,
        member: me,
    });
    h.app.section = Section::Timesheets;
    h.settle();
    let n = painted(&h).iter().filter(|(t, _)| t == "40m").count();
    assert!(n >= 2, "the day's cell and the totals: {}", n);
    assert!(!painted(&h).iter().any(|(t, _)| t.contains("0.67")));
}

#[test]
fn inbox_tabs_without_anything_to_invite_for_use_their_own_icon() {
    let mut h = Ui1::new();
    h.app.apply(Action::Go(Location::Inbox));
    h.app.inbox_tab = 2;
    h.settle();
    assert!(shows(&h, "Nothing saved for later"));
    // the icons are drawn lines now; the people icon comes only with the
    // invite text, which this tab does not show
    assert!(!shows(&h, "Looking to collaborate?"));
}

#[test]
fn particles_follow_the_last_sound() {
    set_lang(Lang::Ko);
    let p = |s: String| crate::i18n::particles(&s);
    assert_eq!(
        p(crate::trf!("Signed in as {} ({})", "지원", "관리자")),
        "지원으로 로그인됨 (관리자)"
    );
    assert_eq!(
        p(crate::trf!("Joined as {}.", "me@team.io")),
        "me@team.io로 참가했습니다."
    );
    assert_eq!(
        p(crate::trf!("Joined as {}.", "Kim")),
        "Kim으로 참가했습니다."
    );
    // a Latin ending whose sound isn't plain is left as written
    assert_eq!(
        p(crate::trf!("Joined as {}.", "Alex")),
        "Alex(으)로 참가했습니다."
    );
    set_lang(Lang::En);
}

#[test]
fn settings_people_has_one_title() {
    let mut h = Ui1::new();
    h.app.open_settings("People");
    h.settle();
    // the left list has "People" too; the body has it once
    let body: Vec<_> = painted(&h)
        .into_iter()
        .filter(|(t, r)| t == "People" && r.left() > 300.0)
        .collect();
    assert_eq!(body.len(), 1, "{:?}", body);
}
