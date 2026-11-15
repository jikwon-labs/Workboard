// Korean interface check: opens every page, view, settings section and
// dialog with Korean on, collects all the text drawn on screen and lists the
// pieces still in English. Text from the workspace itself (task names,
// people, messages) is expected to stay as typed and is skipped.

use super::*;
use crate::i18n::{set_lang, Lang};

fn texts(shapes: &[egui::epaint::ClippedShape], out: &mut Vec<String>) {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    for c in shapes {
        walk(&c.shape, out);
    }
}

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
    // needs a word of two or more Latin letters
    let mut run = 0;
    let mut word = false;
    for c in s.chars() {
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
    const KEEP: [&str; 9] = [
        "workboard",
        "english",
        "jitsi",
        "markdown",
        "enter",
        "esc",
        "shift+enter",
        "https://...",
        "tab",
    ];
    if KEEP.iter().any(|k| low == *k) {
        return false;
    }
    !data
        .iter()
        .any(|d| d.contains(&low) || (low.len() > 3 && low.contains(d.as_str()) && d.len() > 3))
}

fn screens(h: &Ui1) -> Vec<(String, Box<dyn Fn(&mut App)>)> {
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
            a.chat_open = ch;
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
        (
            "new workspace",
            Box::new(move || Modal::NewWorkspace {
                name: String::new(),
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
                private: false,
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
    // signing in: the first start, each sign-in tab, the account section
    use super::together::{Form, Mode};
    for step in 0..4u8 {
        v.push((
            format!("welcome {}", step),
            Box::new(move |a: &mut App| {
                a.sync_rt.form = Form::default();
                a.modal = Some(Modal::Welcome {
                    step,
                    name: String::new(),
                    addr: String::new(),
                });
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
        texts(&h.shapes, &mut shown);
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
