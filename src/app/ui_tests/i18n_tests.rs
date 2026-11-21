// Korean interface check: opens every page, view, settings section and
// dialog with Korean on, collects all the text drawn on screen and lists the
// pieces still in English. Text from the workspace itself (task names,
// people, messages) is expected to stay as typed and is skipped.

use super::*;
use crate::i18n::{set_lang, Lang};

/// All strings in the workspace data (lower case), for telling data from interface.
fn data_strings(d: &Data) -> Vec<String> {
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => out.push(s.to_lowercase()),
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            serde_json::Value::Object(o) => o.iter().for_each(|(_, x)| walk(x, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    walk(&serde_json::to_value(d).unwrap(), &mut out);
    out
}

fn english_left(shown: &str, data: &[String]) -> bool {
    let s = shown.trim().trim_end_matches('\u{2026}').trim();
    if s.chars().any(|c| ('\u{ac00}'..='\u{d7a3}').contains(&c)) {
        return false;
    }
    // needs a word of two or more Latin letters; key names in shortcuts
    // ("Ctrl+J" outside the Mac) are the same in Korean
    let keyless = s
        .replace("Ctrl", "")
        .replace("Alt", "")
        .replace("Shift", "");
    let mut run = 0;
    let mut word = false;
    for c in keyless.chars() {
        if c.is_ascii_alphabetic() {
            run += 1;
            word |= run >= 2;
        } else {
            run = 0;
        }
    }
    if !word {
        return false;
    }
    // avatar initials made from people's names
    if s.len() <= 3 && s.chars().all(|c| c.is_ascii_uppercase()) {
        return false;
    }
    let low = s.to_lowercase();
    // names, key names and examples that are the same in Korean
    const KEEP: [&str; 15] = [
        "workboard",
        // the Mac's name for it in Korean too (the notification preview)
        "dock",
        "english",
        "jitsi",
        "markdown",
        "enter",
        "esc",
        "shift+enter",
        "https://...",
        "https://wb.example.com",
        "tab",
        // sync services found on the computer running the tests
        "icloud drive",
        "dropbox",
        "google drive",
        "onedrive",
    ];
    if KEEP.iter().any(|k| low == *k) {
        return false;
    }
    // the app's name with its version ("Workboard 0.1.9")
    if low
        .strip_prefix("workboard ")
        .is_some_and(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.'))
    {
        return false;
    }
    !data
        .iter()
        .any(|d| d.contains(&low) || (low.len() > 3 && low.contains(d.as_str()) && d.len() > 3))
}

pub(super) fn screens(h: &Ui1) -> Vec<(String, Box<dyn Fn(&mut App)>)> {
    let d = &h.app.data;
    let space = d.spaces.first().map(|s| s.id).unwrap_or(0);
    let folder = d
        .spaces
        .iter()
        .flat_map(|s| s.folders.iter())
        .next()
        .map(|f| f.id);
    let list = d.first_list().unwrap_or(0);
    let task = d.tasks.first().map(|t| t.id);
    let ch = d.channels.first().map(|c| c.id);
    let doc = d.docs.first().map(|x| x.id);
    let mut v: Vec<(String, Box<dyn Fn(&mut App)>)> = vec![];
    for (name, loc) in [
        ("home", Location::Home),
        ("inbox", Location::Inbox),
        ("replies", Location::Replies),
        ("assigned", Location::Assigned),
        ("today", Location::TodayOverdue),
        ("my tasks", Location::MyTasks),
        ("everything", Location::Everything),
        ("space", Location::Space(space)),
    ] {
        v.push((
            name.into(),
            Box::new(move |a: &mut App| {
                a.section = Section::Home;
                a.loc = loc;
            }),
        ));
    }
    if let Some(f) = folder {
        v.push((
            "folder".into(),
            Box::new(move |a: &mut App| {
                a.section = Section::Home;
                a.loc = Location::Folder(f);
            }),
        ));
    }
    for (vname, view) in [
        ("overview", ViewKind::Overview),
        ("list", ViewKind::List),
        ("board", ViewKind::Board),
        ("calendar", ViewKind::Calendar),
        ("gantt", ViewKind::Gantt),
        ("timeline", ViewKind::Timeline),
        ("table", ViewKind::Table),
    ] {
        v.push((
            format!("list {}", vname),
            Box::new(move |a: &mut App| {
                a.section = Section::Home;
                a.loc = Location::List(list);
                a.view = view;
            }),
        ));
    }
    v.push((
        "open task".into(),
        Box::new(move |a: &mut App| {
            a.section = Section::Home;
            a.loc = Location::List(list);
            a.view = ViewKind::List;
            a.open_task = task;
        }),
    ));
    for s in [
        Section::Chat,
        Section::Planner,
        Section::Teams,
        Section::Docs,
        Section::Dashboards,
        Section::Timesheets,
        Section::People,
        Section::Posts,
        Section::Meetings,
        Section::Goals,
        Section::Forms,
        Section::Whiteboards,
        Section::Ai,
    ] {
        v.push((
            format!("section {:?}", s),
            Box::new(move |a: &mut App| {
                a.open_task = None;
                a.section = s;
            }),
        ));
    }
    v.push((
        "chat channel".into(),
        Box::new(move |a: &mut App| {
            a.section = Section::Chat;
            a.chat.open = ch;
        }),
    ));
    v.push((
        "doc".into(),
        Box::new(move |a: &mut App| {
            a.section = Section::Docs;
            a.doc_open = doc;
        }),
    ));
    for i in 0..settings::SETTINGS.len() {
        v.push((
            format!("settings {}", settings::SETTINGS[i].1),
            Box::new(move |a: &mut App| {
                a.section = Section::Settings;
                a.settings_tab = i as u8;
                a.settings_state = None;
            }),
        ));
    }
    let modals: Vec<(&str, Box<dyn Fn() -> Modal>)> = vec![
        (
            "invite",
            Box::new(move || Modal::Invite {
                emails: String::new(),
                role: "Member".into(),
            }),
        ),
        ("new workspace", Box::new(move || Modal::new_workspace())),
        (
            "new workspace apps",
            Box::new(move || {
                let mut m = Modal::new_workspace();
                if let Modal::NewWorkspace { step, name, .. } = &mut m {
                    *step = 1;
                    *name = "Acme".into();
                }
                m
            }),
        ),
        (
            "new workspace invite",
            Box::new(move || {
                let mut m = Modal::new_workspace();
                if let Modal::NewWorkspace { step, name, .. } = &mut m {
                    *step = 2;
                    *name = "Acme".into();
                }
                m
            }),
        ),
        (
            "set status",
            Box::new(move || Modal::SetStatus {
                text: String::new(),
            }),
        ),
        ("help", Box::new(move || Modal::Help)),
        ("labs", Box::new(move || Modal::Labs)),
        (
            "import",
            Box::new(move || Modal::Import {
                path: String::new(),
                result: String::new(),
            }),
        ),
        (
            "share",
            Box::new(move || Modal::Share {
                emails: String::new(),
            }),
        ),
        ("customize nav", Box::new(move || Modal::CustomizeNav)),
        (
            "customize sidebar",
            Box::new(move || Modal::CustomizeSidebar),
        ),
        (
            "reminder",
            Box::new(move || Modal::Reminder {
                text: String::new(),
                date: Some(today()),
                time: "09:00".into(),
            }),
        ),
        (
            "bookmark",
            Box::new(move || Modal::Bookmark {
                title: String::new(),
                url: String::new(),
            }),
        ),
        ("switch user", Box::new(move || Modal::SwitchUser)),
        (
            "new dashboard",
            Box::new(move || Modal::NewDashboard {
                name: String::new(),
            }),
        ),
        (
            "together",
            Box::new(move || Modal::Together {
                path: String::new(),
                addr: String::new(),
                code: String::new(),
                result: String::new(),
            }),
        ),
        (
            "new list",
            Box::new(move || Modal::NewList {
                name: String::new(),
                space,
                folder: None,
            }),
        ),
        (
            "new space",
            Box::new(move || Modal::NewSpace {
                name: String::new(),
                color: [0x44, 0x66, 0xff],
            }),
        ),
        ("move list", Box::new(move || Modal::MoveList { list })),
        (
            "new channel",
            Box::new(move || Modal::NewChannel {
                name: String::new(),
                private: false,
                add_list: false,
            }),
        ),
        ("list info", Box::new(move || Modal::ListInfo { list })),
    ];
    for (name, m) in modals {
        v.push((
            format!("modal {}", name),
            Box::new(move |a: &mut App| {
                a.section = Section::Home;
                a.loc = Location::List(list);
                a.modal = Some(m());
            }),
        ));
    }
    if let Some(c) = ch {
        v.push((
            "modal call".into(),
            Box::new(move |a: &mut App| {
                a.modal = Some(Modal::Call {
                    ch: c,
                    link: String::new(),
                })
            }),
        ));
        v.push((
            "modal channel settings".into(),
            Box::new(move |a: &mut App| a.modal = Some(Modal::ChannelSettings(c))),
        ));
    }
    if let Some(t) = task {
        v.push((
            "modal track time".into(),
            Box::new(move |a: &mut App| a.modal = Some(Modal::TrackTime { task: t })),
        ));
        v.push((
            "modal move tasks".into(),
            Box::new(move |a: &mut App| {
                a.modal = Some(Modal::MoveTasks {
                    ids: vec![t],
                    query: String::new(),
                })
            }),
        ));
        for mode in [
            TaskPick::Parent,
            TaskPick::Child,
            TaskPick::WaitingOn,
            TaskPick::Blocking,
            TaskPick::Linked,
        ] {
            v.push((
                format!("modal pick task {:?}", mode),
                Box::new(move |a: &mut App| {
                    a.modal = Some(Modal::PickTask {
                        task: t,
                        mode,
                        query: String::new(),
                    })
                }),
            ));
        }
        v.push((
            "bulk delete".into(),
            Box::new(move |a: &mut App| {
                a.section = Section::Home;
                a.loc = Location::List(list);
                a.view = ViewKind::List;
                a.selected = [t].into_iter().collect();
                a.confirm_bulk_delete = true;
            }),
        ));
    }
    // the small apps, each with one item made the way the app makes it
    v.push((
        "modal trash".into(),
        Box::new(move |a: &mut App| {
            a.open_task = None;
            a.confirm_bulk_delete = false;
            a.selected.clear();
            let id = a.data.new_id();
            a.data.extra.trash.push(TrashItem {
                id,
                kind: "Doc".into(),
                title: String::new(),
                deleted: now_ts(),
                payload: String::new(),
            });
            a.modal = Some(Modal::Trash);
        }),
    ));
    v.push((
        "search".into(),
        Box::new(move |a: &mut App| a.palette = Some("e".into())),
    ));
    v.push((
        "whiteboard".into(),
        Box::new(move |a: &mut App| {
            a.palette = None;
            a.new_whiteboard();
        }),
    ));
    v.push(("form".into(), Box::new(move |a: &mut App| a.new_form())));
    // the form's other tabs, and the hub's table with a form in it
    for (name, tab) in [
        ("form fill out", super::more::FormTab::Fill),
        ("form answers", super::more::FormTab::Answers),
    ] {
        v.push((
            name.into(),
            Box::new(move |a: &mut App| {
                a.new_form();
                a.form_tab = tab;
            }),
        ));
    }
    v.push((
        "forms table".into(),
        Box::new(move |a: &mut App| {
            a.new_form();
            a.form_open = None;
        }),
    ));
    v.push((
        "dashboard".into(),
        Box::new(move |a: &mut App| {
            a.section = Section::Dashboards;
            a.new_dashboard("Workspace overview");
        }),
    ));
    v.push((
        "team".into(),
        Box::new(move |a: &mut App| {
            a.section = Section::Teams;
            a.new_team();
        }),
    ));
    // the Template Center with each kind of template picked, and a team one
    for key in [
        "project-tracker",
        "bug-report",
        "meeting-notes",
        "team",
        "new",
    ] {
        v.push((
            format!("template center {}", key),
            Box::new(move |a: &mut App| {
                use super::super::templates::{Category, Pick};
                a.section = Section::Home;
                a.loc = Location::List(list);
                let pick = match key {
                    "team" => {
                        let id = a.save_list_as_template(list).unwrap();
                        Pick::Team(id)
                    }
                    "new" => Pick::NewTask,
                    k => Pick::Builtin(k),
                };
                let mut m = a.template_center(Category::All, None);
                if let Modal::TemplateCenter(c) = &mut m {
                    c.pick = Some(pick);
                    c.new_space = false;
                }
                a.modal = Some(m);
            }),
        ));
    }
    for f in 1..=4u8 {
        v.push((
            format!("teams tab {}", f),
            Box::new(move |a: &mut App| {
                a.section = Section::Teams;
                a.team_open = None;
                a.hub_filter = f;
            }),
        ));
    }
    for tab in 0..4u8 {
        v.push((
            format!("inbox tab {}", tab),
            Box::new(move |a: &mut App| {
                a.section = Section::Home;
                a.loc = Location::Inbox;
                a.inbox_tab = tab;
            }),
        ));
    }
    v.push((
        "notepad".into(),
        Box::new(move |a: &mut App| a.notepad_open = true),
    ));
    v.push((
        "filter groups and saved filters".into(),
        Box::new(move |a: &mut App| {
            a.notepad_open = false;
            a.section = Section::Home;
            a.loc = Location::Everything;
            a.view = ViewKind::List;
            a.filter.rows = vec![parity::FilterRow::default(), parity::FilterRow::default()];
            a.filter.groups = vec![parity::FilterGroup {
                join: parity::Join::Or,
                rows: vec![parity::FilterRow::default(), parity::FilterRow::default()],
            }];
            a.save_filter("Mine");
            a.filter_open = true;
        }),
    ));
    // signing in: the sign-in screen's pages, each sign-in tab, the
    // account section, logging out
    use super::login::{LoginPage, LoginScreen};
    use super::together::{Form, Mode};
    for (name, page, mode) in [
        ("server", LoginPage::Server, Mode::Account),
        ("sign in", LoginPage::SignIn, Mode::Account),
        ("forgot", LoginPage::SignIn, Mode::Forgot),
        ("invite", LoginPage::Invite, Mode::Invite),
        ("new server", LoginPage::NewServer, Mode::NewServer),
        ("on this mac", LoginPage::OnThisMac, Mode::Account),
    ] {
        v.push((
            format!("login screen {}", name),
            Box::new(move |a: &mut App| {
                a.modal = None;
                a.sync_rt.form = Form {
                    mode,
                    ..Default::default()
                };
                a.login = Some(LoginScreen::new(page, "100.101.102.103"));
            }),
        ));
    }
    for (name, mode, sent) in [
        ("invite", Mode::Invite, false),
        ("new server", Mode::NewServer, false),
        ("forgot", Mode::Forgot, false),
        ("forgot sent", Mode::Forgot, true),
    ] {
        v.push((
            format!("sign in {}", name),
            Box::new(move |a: &mut App| {
                a.login = None;
                a.sync_rt.form = Form {
                    mode,
                    sent,
                    note: "x".into(),
                    ..Default::default()
                };
                a.modal = Some(a.together_modal());
            }),
        ));
    }
    for (name, account, changing) in [
        ("account none", None, false),
        ("account change", Some("me@team.io"), true),
    ] {
        v.push((
            format!("together {}", name),
            Box::new(move |a: &mut App| {
                a.data.extra.shared = Some(crate::sync::SharedCfg {
                    folder: String::new(),
                    device: "dev1".into(),
                    tag: 5,
                    server: Some("127.0.0.1:1".into()),
                });
                a.sync_rt.net_status = Some(wb_net::Status::Connected);
                a.sync_rt.role = Some(wb_net::Role::Member);
                a.sync_rt.account = account.map(|e| e.to_string());
                a.sync_rt.acct = Form {
                    changing,
                    ..Default::default()
                };
                a.modal = Some(a.together_modal());
            }),
        ));
    }
    v.push((
        "together signed out".into(),
        Box::new(move |a: &mut App| {
            a.data.extra.shared = Some(crate::sync::SharedCfg {
                folder: String::new(),
                device: "dev1".into(),
                tag: 5,
                server: Some("127.0.0.1:1".into()),
            });
            a.sync_rt.net_status = Some(wb_net::Status::Refused("x".into()));
            a.sync_rt.form = Form::default();
            a.modal = Some(a.together_modal());
        }),
    ));
    {
        // the Windows updater's window, in each state it can be in
        use crate::updater::own::{Problem, Release, Status};
        let r = Release {
            version: "99.0.0".into(),
            file: "Workboard-99.0.0-windows-x64.zip".into(),
            length: 10,
            sha256: "ab".repeat(32),
        };
        let states = [
            Status::Checking,
            Status::UpToDate,
            Status::Available(r.clone()),
            Status::Downloading {
                version: "99.0.0".into(),
                done: 5,
                total: 10,
            },
            Status::Failed(Problem::Unreachable),
            Status::Failed(Problem::NotOnServer),
            Status::Failed(Problem::BadFeed),
            Status::Failed(Problem::Damaged),
            Status::Failed(Problem::CantReplace("C:\\Workboard".into())),
            Status::Restarting,
        ];
        for st in states {
            v.push((
                format!("update {:?}", st),
                Box::new(move |a: &mut App| {
                    crate::updater::own::pretend_status(st.clone(), None);
                    a.modal = Some(Modal::Update);
                }),
            ));
        }
    }
    v.push((
        "log out".into(),
        Box::new(move |a: &mut App| {
            a.modal = Some(Modal::LogOut);
        }),
    ));
    v.push((
        "space sharing".into(),
        Box::new(move |a: &mut App| {
            a.data.extra.shared = None;
            a.modal = Some(Modal::SpaceSharing {
                space,
                draft: SpaceSharing {
                    everyone: false,
                    people: vec![SpaceGrant {
                        member: 2,
                        level: "comment".into(),
                    }],
                },
            });
        }),
    ));
    v.push((
        "view only space".into(),
        Box::new(move |a: &mut App| {
            a.data.extra.shared = Some(crate::sync::SharedCfg {
                folder: String::new(),
                device: "dev1".into(),
                tag: 5,
                server: Some("127.0.0.1:1".into()),
            });
            a.sync_rt.role = Some(wb_net::Role::Member);
            if let Some(s) = a.data.spaces.iter_mut().find(|s| s.id == space) {
                s.sharing = Some(SpaceSharing {
                    everyone: false,
                    people: vec![SpaceGrant {
                        member: a.data.me,
                        level: "view".into(),
                    }],
                });
            }
            a.section = Section::Home;
            a.loc = Location::Space(space);
            a.modal = None;
        }),
    ));
    v
}

/// Lists the English left on Korean screens. Run it to see them:
/// cargo test --bin workboard korean_screens -- --nocapture
#[test]
fn korean_screens_have_no_english_left() {
    let mut h = Ui1::new();
    set_lang(Lang::Ko);
    let mut left: Vec<String> = vec![];
    for (name, go) in screens(&h) {
        h.app.modal = None;
        go(&mut h.app);
        h.settle();
        // names of things a screen made (a new team, form...) are workspace data
        let data = data_strings(&h.app.data);
        let mut shown = vec![];
        shown.extend(h.texts());
        for s in shown {
            if english_left(&s, &data) {
                left.push(format!("[{}] {:?}", name, s));
            }
        }
    }
    set_lang(Lang::En);
    left.sort();
    left.dedup();
    let _ = std::fs::write(
        std::env::temp_dir().join("workboard-korean-left.txt"),
        left.join("\n"),
    );
    assert!(
        left.is_empty(),
        "{} English texts on Korean screens:\n{}",
        left.len(),
        left.join("\n")
    );
}

/// A Whiteboard made from a template on a Korean screen gets its name and
/// the words in its boxes in Korean, like the other things the app names.
#[test]
fn whiteboard_template_shapes_are_made_in_korean() {
    let mut h = Ui1::new();
    set_lang(Lang::Ko);
    h.click("Rail More");
    h.click("Whiteboards");
    h.click("Action Plan");
    set_lang(Lang::En);
    let w = h.app.data.extra.whiteboards.last().expect("a whiteboard");
    assert_eq!(w.name, "실행 계획");
    let words: Vec<&str> = w.shapes.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(
        words,
        ["목표", "1단계", "2단계", "3단계", "담당자 / 마감일"]
    );
}

/// The word before a filter's first condition reads "조건", not "위치" (the
/// place a new workspace is kept, whose English is also "Where").
#[test]
fn filter_lead_reads_as_a_condition_in_korean() {
    let mut h = Ui1::new();
    h.app.loc = Location::Everything;
    set_lang(Lang::Ko);
    h.settle();
    h.click("Filter");
    h.click("+ 필터 추가");
    h.settle();
    let mut shown = vec![];
    shown.extend(h.texts());
    set_lang(Lang::En);
    assert!(shown.iter().any(|s| s == "조건"), "{:?}", shown);
    assert!(!shown.iter().any(|s| s == "위치"));
}

/// A goal's new targets are named in the screen's language.
#[test]
fn goal_targets_are_named_in_korean() {
    let mut h = Ui1::new();
    set_lang(Lang::Ko);
    h.click("Rail More");
    h.click("Goals");
    h.click("New goal");
    h.type_text("출시");
    h.click("Create goal");
    h.click("Add target");
    h.click("Add Tasks target");
    set_lang(Lang::En);
    let g = h.app.data.extra.goals.last().expect("a goal");
    let names: Vec<&str> = g.targets.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["목표치", "작업 끝내기"]);
}

/// Goals in Korean: cards, rows with targets, a goal's page, New goal, Goal
/// achieved and the empty page show no English of their own.
#[test]
fn goal_screens_have_no_english_left() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let day = today();
    let task = h.app.data.tasks[0].id;
    let target = |name: &str, kind: TargetKind| GoalTarget {
        id: 0,
        name: name.into(),
        kind,
    };
    h.app.data.extra.goals = vec![
        Goal {
            id: 801,
            name: "Launch".into(),
            color: [0x5f, 0x55, 0xee],
            due: Some(day + chrono::Duration::days(20)),
            owner: me,
            start: Some(day - chrono::Duration::days(40)),
            targets: vec![
                target(
                    "Testers",
                    TargetKind::Number {
                        start: 0.0,
                        target: 50.0,
                        current: 38.0,
                        unit: "x".into(),
                    },
                ),
                target("Review", TargetKind::TrueFalse { done: false }),
                target("Work", TargetKind::Tasks { ids: vec![task] }),
            ],
            ..Default::default()
        },
        Goal {
            id: 802,
            name: "Move".into(),
            color: [0x00, 0x88, 0x44],
            owner: me,
            start: Some(day - chrono::Duration::days(9)),
            due: Some(day + chrono::Duration::days(3)),
            done_on: Some(day),
            targets: vec![target("Lease", TargetKind::TrueFalse { done: true })],
            ..Default::default()
        },
    ];
    let mut data = data_strings(&h.app.data);
    set_lang(Lang::Ko);
    h.click("Rail More");
    h.click("Goals");
    h.settle();
    let mut shown = vec![];
    shown.extend(h.texts());
    h.click("Show as list");
    h.click("Show targets of Launch");
    shown.extend(h.texts());
    h.click("Goal row Launch");
    h.settle();
    shown.extend(h.texts());
    // a milestone floating over the page, and the settings dialog
    h.app.goal_banner = Some((801, 50, now_ts()));
    h.settle();
    shown.extend(h.texts());
    h.app.modal = Some(Modal::GoalSettings { id: 801 });
    h.settle();
    shown.extend(h.texts());
    h.app.modal = Some(Modal::NewGoal(Box::new(super::more::GoalDraft::new(me))));
    h.settle();
    shown.extend(h.texts());
    h.app.modal = Some(Modal::GoalAchieved { id: 802 });
    h.settle();
    shown.extend(h.texts());
    h.app.open_goal_share(802);
    h.settle();
    shown.extend(h.texts());
    // the card in the conversation
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    h.app.modal = None;
    h.app.share_goal(general, 802, "", false);
    h.app.section = Section::Chat;
    h.app.chat.open = Some(general);
    h.settle();
    shown.extend(h.texts());
    h.app.section = Section::Goals;
    h.app.modal = None;
    h.app.goal_open = None;
    h.app.data.extra.goals.clear();
    h.settle();
    shown.extend(h.texts());
    set_lang(Lang::En);
    data.extend(data_strings(&h.app.data));
    let mut left: Vec<&String> = shown.iter().filter(|s| english_left(s, &data)).collect();
    left.sort();
    left.dedup();
    assert!(left.is_empty(), "English on the goal screens: {:?}", left);
}
