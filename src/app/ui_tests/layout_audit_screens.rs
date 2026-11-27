// The screens the layout audit opens beyond `screens()`: menus, detail
// pages, chat threads, the task window's parts, small dialogs drawn outside
// `Modal`, view states, hovers. Each one starts from a fresh audit window
// (layout_audit.rs) and may click; one that can't be opened is reported as
// "could not open".

use super::layout_audit::Go;
use super::*;
use crate::app::ai::{AiDialog, AiJob, AiQuestion, AiState, AiUse};
use crate::app::more::WbTool;
use chrono::Duration;

fn general(a: &App) -> u64 {
    a.data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .or_else(|| a.data.channels.first())
        .map(|c| c.id)
        .unwrap_or(0)
}

/// general's message with replies (the seed's thread), or its first one.
fn thread_parent(a: &App) -> u64 {
    let g = general(a);
    a.data
        .channels
        .iter()
        .find(|c| c.id == g)
        .and_then(|c| {
            c.messages
                .iter()
                .find(|m| !m.replies.is_empty())
                .or(c.messages.first())
        })
        .map(|m| m.id)
        .unwrap_or(0)
}

fn dm(a: &App, empty: bool) -> Option<u64> {
    a.data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.messages.is_empty() == empty)
        .map(|c| c.id)
}

fn on_server(a: &mut App) {
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
}

fn first_list(a: &App) -> u64 {
    a.data.first_list().unwrap_or(0)
}

fn first_task(a: &App) -> u64 {
    a.data.tasks.first().map(|t| t.id).unwrap_or(0)
}

/// A goal with a long Korean name, a number target and a done-or-not one.
fn add_goal(a: &mut App) -> u64 {
    let id = a.data.new_id();
    let day = today();
    let me = a.data.me;
    a.data.extra.goals.push(Goal {
        id,
        name: "2026년 하반기 신규 고객 300명 확보와 재구매율 높이기".into(),
        color: [0x5f, 0x55, 0xee],
        due: Some(day + Duration::days(20)),
        owner: me,
        start: Some(day - Duration::days(40)),
        targets: vec![
            GoalTarget {
                id: 0,
                name: "신규 가입한 베타 테스터 수 (온라인·오프라인 합산)".into(),
                kind: TargetKind::Number {
                    start: 0.0,
                    target: 300.0,
                    current: 138.0,
                    unit: "명".into(),
                },
            },
            GoalTarget {
                id: 0,
                name: "Store review".into(),
                kind: TargetKind::TrueFalse { done: false },
            },
        ],
        ..Default::default()
    });
    id
}

fn add_meeting(a: &mut App) -> u64 {
    let id = a.data.new_id();
    let mut people: Vec<u64> = a.data.members.iter().map(|m| m.id).collect();
    people.truncate(6);
    a.data.extra.meetings.push(Meeting {
        id,
        title: "주간 제품 기획 회의 – 출시 일정과 남은 작업 점검".into(),
        date: today(),
        time: "10:00".into(),
        attendees: people,
        agenda: "1. 지난주 결정 사항 확인\n2. 출시 일정\n3. Open questions".into(),
        notes: "Decide the launch date".into(),
    });
    id
}

fn add_post(a: &mut App) -> u64 {
    let g = general(a);
    a.add_post(
        g,
        "2026년 하반기 제품 출시 일정과 팀별 준비 사항 안내".into(),
        "Announcement",
        "모두 확인 부탁드립니다. Please read before Friday.".into(),
        vec![],
        &[],
    )
}

fn huddle(a: &mut App) {
    on_server(a);
    let me = a.data.me;
    let other = a
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .map(|m| m.id)
        .unwrap_or(2);
    a.huddle.channel = Some(general(a));
    a.huddle.joined = true;
    a.huddle.people = vec![
        wb_voice::Person {
            identity: format!("{}-dev1", me),
            name: String::new(),
            me: true,
            speaking: false,
            muted: false,
            heard: 0.0,
            camera: false,
            screen: false,
        },
        wb_voice::Person {
            identity: format!("{}-dev2", other),
            name: String::new(),
            me: false,
            speaking: true,
            muted: false,
            heard: 0.3,
            camera: true,
            screen: false,
        },
        wb_voice::Person {
            identity: "9001-dev3".into(),
            name: String::new(),
            me: false,
            speaking: false,
            muted: true,
            heard: 0.0,
            camera: false,
            screen: false,
        },
    ];
}

fn ai_on(a: &mut App) {
    on_server(a);
    a.ai_ready_now("m".into());
}

fn ai_job(a: &mut App, state: AiState) {
    a.ai.jobs.insert(
        900,
        AiJob {
            ask: wb_net::AiAsk::SummarizeChannel {
                channel: general(a),
            },
            text: "요약: 이번 주에는 홈페이지 시안과 뉴스레터 초안을 마무리했습니다. \
                   The beta feedback list still needs an owner."
                .into(),
            state,
        },
    );
}

/// The AI page with a kept chat shared with the team: mine, or a teammate's.
fn ai_shared_chat(a: &mut App, mine: bool) {
    ai_on(a);
    let by = if mine {
        a.data.me
    } else {
        a.data
            .members
            .iter()
            .find(|m| m.id != a.data.me)
            .unwrap()
            .id
    };
    a.data.extra.ai_chats.push(crate::model::AiChat {
        id: 902,
        title: "이번 주 general 채널에서 무슨 일이 있었는지 정리해 줘, 마감이 가까운 작업도 같이"
            .into(),
        by,
        created: 1,
        turns: vec![crate::model::AiChatTurn {
            id: 903,
            question: "이번 주 general 채널에서 무슨 일이 있었는지 정리해 줘".into(),
            answer: "요약: 홈페이지 시안과 뉴스레터 초안을 마무리했습니다.".into(),
            ts: 2,
        }],
        personal: false,
    });
    a.ai.open = Some(902);
    a.section = Section::Ai;
}

fn home_list(a: &mut App) {
    a.section = Section::Home;
    a.loc = Location::List(first_list(a));
    a.view = ViewKind::List;
}

pub(super) fn extra_screens(_h: &Ui1) -> Vec<(String, Go)> {
    let mut v: Vec<(String, Go)> = vec![];
    let mut x = |name: &str, go: Go| v.push((name.to_string(), go));
    let app = |f: fn(&mut App)| -> Go { Box::new(move |h: &mut Ui1| f(&mut h.app)) };

    // --- menus forced open
    for m in ["avatar", "workspace", "more", "create"] {
        x(
            &format!("menu {}", m),
            Box::new(move |h: &mut Ui1| {
                h.app.force_menu = Some(m.into());
                h.settle();
            }),
        );
    }
    x(
        "menu priority (task window)",
        Box::new(|h: &mut Ui1| {
            home_list(&mut h.app);
            h.app.open_task = Some(first_task(&h.app));
            h.app.force_menu = Some("priority".into());
            h.settle();
        }),
    );
    x(
        "menu avatar mute flyout (hover)",
        Box::new(|h: &mut Ui1| {
            h.app.force_menu = Some("avatar".into());
            h.settle();
            let p = h.find("Mute notifications").unwrap();
            h.hover(p);
            h.settle();
        }),
    );
    // --- menus opened by a click
    let clicks: Vec<(&str, Vec<&str>)> = vec![
        ("menu space options", vec!["Space options Team Space"]),
        ("menu create in space", vec!["Create in Team Space"]),
        ("menu folder options", vec!["Folder options Projects"]),
        ("menu list options", vec!["List options Website Redesign"]),
        ("menu create in list", vec!["Create in Website Redesign"]),
        ("menu spaces settings", vec!["Spaces settings"]),
        ("menu list header", vec!["List menu"]),
        ("menu group options", vec!["Group options TO DO"]),
        ("menu group by", vec!["Group: Status"]),
        ("menu sort", vec!["Sort: Manual"]),
        ("menu assignee filter", vec!["Assignee"]),
        ("menu add view", vec!["+  View"]),
    ];
    for (name, path) in clicks {
        x(
            name,
            Box::new(move |h: &mut Ui1| {
                home_list(&mut h.app);
                h.settle();
                // narrow windows keep the sidebar in a drawer
                if path[0].contains(" options ")
                    || path[0].starts_with("Create in")
                    || path[0] == "Spaces settings"
                {
                    if h.has("Toggle sidebar") {
                        h.click("Toggle sidebar");
                    } else if !h.has("Rail Home") {
                        // a compact window: the sidebar is its home screen
                        h.app.open_sheet();
                        h.settle();
                    }
                }
                for p in &path {
                    h.click(p)
                }
            }),
        );
    }
    x(
        "menu task row",
        Box::new(|h: &mut Ui1| {
            home_list(&mut h.app);
            h.settle();
            h.hover(h.find("Task Set up analytics dashboard").unwrap());
            h.click("Task options Set up analytics dashboard");
        }),
    );
    x(
        "menu board card",
        Box::new(|h: &mut Ui1| {
            home_list(&mut h.app);
            h.app.view = ViewKind::Board;
            h.settle();
            h.click("Card options Design new homepage layout");
        }),
    );
    x(
        "list columns panel",
        Box::new(|h: &mut Ui1| {
            home_list(&mut h.app);
            h.app.fields_open = true;
        }),
    );
    x(
        "list search and filters",
        Box::new(|h: &mut Ui1| {
            home_list(&mut h.app);
            h.app.show_search = true;
            h.app.search = "design".into();
            h.app.me_mode = true;
            h.app.assignee_filter = Some(2);
        }),
    );
    for bar in ["Status", "Due date", "Priority"] {
        x(
            &format!("bulk bar {}", bar),
            Box::new(move |h: &mut Ui1| {
                home_list(&mut h.app);
                let t = first_task(&h.app);
                h.app.selected = [t].into_iter().collect();
                h.settle();
                h.click(bar);
            }),
        );
    }

    // --- chat
    x(
        "chat dm",
        Box::new(|h: &mut Ui1| {
            h.app.section = Section::Chat;
            h.app.chat.open = dm(&h.app, false);
        }),
    );
    x(
        "chat dm empty",
        Box::new(|h: &mut Ui1| {
            h.app.section = Section::Chat;
            h.app.chat.open = dm(&h.app, true);
        }),
    );
    x(
        "chat long channel",
        Box::new(|h: &mut Ui1| {
            h.app.section = Section::Chat;
            h.app.chat.open = Some(9_002);
        }),
    );
    x(
        "chat no channels",
        app(|a| {
            a.data.channels.clear();
            a.section = Section::Chat;
            a.chat.open = None;
        }),
    );
    x(
        "chat thread panel",
        app(|a| {
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            a.chat.thread_open = Some(thread_parent(a));
        }),
    );
    x(
        "chat thread window",
        app(|a| {
            a.section = Section::Chat;
            let g = general(a);
            a.chat.open = Some(g);
            a.chat.thread_open = Some(thread_parent(a));
            a.chat.thread_window = Some(g);
        }),
    );
    x(
        "chat profile other",
        app(|a| {
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            a.chat.profile = Some(9_001);
        }),
    );
    x(
        "chat profile mine",
        app(|a| {
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            a.chat.profile = Some(a.data.me);
        }),
    );
    for (name, p) in [
        ("pinned", crate::app::chat::ChannelPanel::Pinned),
        ("replies", crate::app::chat::ChannelPanel::Replies),
        ("tasks", crate::app::chat::ChannelPanel::Tasks),
        ("saved", crate::app::chat::ChannelPanel::Saved),
        ("files", crate::app::chat::ChannelPanel::Files),
        ("links", crate::app::chat::ChannelPanel::Links),
    ] {
        x(
            &format!("chat panel {}", name),
            Box::new(move |h: &mut Ui1| {
                h.app.section = Section::Chat;
                h.app.chat.open = Some(general(&h.app));
                h.app.channel_panel = p;
            }),
        );
    }
    for (name, q) in [
        ("match", "homepage"),
        ("no match", "zzzz"),
        ("has file", "has:file"),
        ("from", "from:alex"),
    ] {
        x(
            &format!("chat find {}", name),
            Box::new(move |h: &mut Ui1| {
                h.app.section = Section::Chat;
                h.app.chat.open = Some(general(&h.app));
                h.app.channel_find = Some(q.into());
            }),
        );
    }
    x(
        "chat composer post mode",
        app(|a| {
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            a.composer_mode = crate::app::chat::ComposerMode::Post;
        }),
    );
    for b in [
        "Mention someone",
        "Emoji",
        "Schedule message",
        "Channel notifications",
    ] {
        x(
            &format!("chat popup {}", b),
            Box::new(move |h: &mut Ui1| {
                h.app.section = Section::Chat;
                h.app.chat.open = Some(general(&h.app));
                h.settle();
                if b == "Schedule message" {
                    h.click("Message composer");
                    h.type_text("내일 아침 회의 전에 확인 부탁드립니다");
                }
                h.click(b);
            }),
        );
    }
    x(
        "chat editing a message",
        app(|a| {
            a.section = Section::Chat;
            let g = general(a);
            a.chat.open = Some(g);
            a.chat.edit = Some((g, thread_parent(a), "고친 메시지 Edited text".into()));
        }),
    );
    x(
        "chat delete message confirm",
        app(|a| {
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            a.chat.delete = Some(thread_parent(a));
        }),
    );
    x(
        "chat message hover toolbar",
        Box::new(|h: &mut Ui1| {
            h.app.section = Section::Chat;
            h.app.chat.open = Some(general(&h.app));
            h.settle();
            let p = h
                .painted_text("Reminder: newsletter draft is due Thursday.")
                .first()
                .map(|(r, _)| r.center())
                .unwrap();
            h.hover(p);
            h.settle();
        }),
    );
    x(
        "chat message more actions",
        Box::new(|h: &mut Ui1| {
            h.app.section = Section::Chat;
            h.app.chat.open = Some(general(&h.app));
            h.settle();
            let p = h
                .painted_text("Reminder: newsletter draft is due Thursday.")
                .first()
                .map(|(r, _)| r.center())
                .unwrap();
            h.hover(p);
            h.settle();
            h.click("More actions");
        }),
    );
    x(
        "chat new channel field",
        app(|a| {
            a.section = Section::Chat;
            a.new_channel = Some(String::new());
        }),
    );
    x(
        "chat new message window",
        app(|a| a.new_message_open = true),
    );
    x(
        "huddle bar",
        app(|a| {
            huddle(a);
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
        }),
    );
    x(
        "huddle share chooser",
        app(|a| {
            huddle(a);
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            a.huddle.share_menu = true;
        }),
    );
    x(
        "huddle share no permission",
        app(|a| {
            huddle(a);
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            a.huddle.share_menu = true;
            a.huddle.screen_problem = Some(wb_voice::ScreenTrouble::NotAllowed);
        }),
    );
    x(
        "huddle sharing screen",
        app(|a| {
            huddle(a);
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            a.huddle.screen = Some(wb_voice::Share::Display(1));
        }),
    );
    x(
        "huddle video",
        app(|a| {
            huddle(a);
            a.section = Section::Chat;
            a.chat.open = Some(general(a));
            // the speaking person sends video: a picture in their tile
            let who = a.huddle.people[1].identity.clone();
            let (w, h) = (64u32, 36u32);
            let rgba = (0..w * h)
                .flat_map(|i| {
                    let (x, y) = (i % w, i / w);
                    [(60 + x * 2) as u8, (90 + y * 3) as u8, 140, 255]
                })
                .collect();
            let pic = wb_voice::Picture {
                width: w,
                height: h,
                rgba,
            };
            a.huddle
                .pictures
                .insert(who, (std::sync::Arc::new(pic), true));
        }),
    );
    x(
        "mini window",
        app(|a| {
            let g = general(a);
            a.open_mini(g);
        }),
    );
    x(
        "mini window with huddle",
        app(|a| {
            huddle(a);
            let g = general(a);
            a.open_mini(g);
        }),
    );
    x(
        "picture viewer",
        app(|a| {
            let g = general(a);
            let mut m =
                crate::app::slack::new_message(a.data.new_id(), a.data.me, String::new(), now_ts());
            m.attachments.push(Attachment {
                file: "no-such-file".into(),
                name: "아주-긴-파일-이름의-디자인-시안-최종-수정본.png".into(),
                kind: MediaKind::Image,
                bytes: 10,
                w: 400,
                h: 300,
                secs: 0.0,
                thumb: None,
            });
            let mid = m.id;
            if let Some(c) = a.data.channels.iter_mut().find(|c| c.id == g) {
                c.messages.push(m);
            }
            a.section = Section::Chat;
            a.chat.open = Some(g);
            a.viewer = Some(files::Viewer::new(g, mid, 0));
        }),
    );

    // --- detail pages
    x(
        "goal page",
        app(|a| {
            let id = add_goal(a);
            a.section = Section::Goals;
            a.goal.open = Some(id);
        }),
    );
    x(
        "goal milestone banner",
        app(|a| {
            let id = add_goal(a);
            a.section = Section::Goals;
            a.goal.open = Some(id);
            a.goal.banner = Some((id, 50, now_ts()));
        }),
    );
    x(
        "goals cards with a goal",
        app(|a| {
            add_goal(a);
            a.section = Section::Goals;
            a.goal.open = None;
        }),
    );
    x(
        "goals list view",
        app(|a| {
            let id = add_goal(a);
            a.section = Section::Goals;
            a.goal.open = None;
            a.data.extra.goals_view = "list".into();
            a.goal.list_open = Some(id);
        }),
    );
    x(
        "meeting page",
        app(|a| {
            let id = add_meeting(a);
            a.section = Section::Meetings;
            a.meeting_open = Some(id);
        }),
    );
    x(
        "meeting bad time",
        app(|a| {
            let id = add_meeting(a);
            if let Some(m) = a.data.extra.meetings.last_mut() {
                m.time = "abc".into();
            }
            a.section = Section::Meetings;
            a.meeting_open = Some(id);
        }),
    );
    x(
        "posts with a post",
        app(|a| {
            add_post(a);
            a.section = Section::Posts;
        }),
    );
    x(
        "whiteboard open",
        app(|a| {
            a.new_whiteboard();
            a.sweep.wb.new = None;
        }),
    );
    x(
        "whiteboard all templates",
        app(|a| {
            a.new_whiteboard();
            if let Some(n) = a.sweep.wb.new.as_mut() {
                n.all_templates = true;
            }
        }),
    );
    for (name, tool) in [
        ("sticky", WbTool::Sticky),
        ("shape", WbTool::Shape),
        ("line", WbTool::Line),
        ("select", WbTool::Select),
        ("eraser", WbTool::Eraser),
    ] {
        x(
            &format!("whiteboard tool {}", name),
            Box::new(move |h: &mut Ui1| {
                h.app.new_whiteboard();
                h.app.sweep.wb.new = None;
                h.app.sweep.wb.tool = tool;
            }),
        );
    }
    for t in ["Organizational Chart", "Action Plan"] {
        x(
            &format!("whiteboard template {}", t),
            Box::new(move |h: &mut Ui1| {
                h.app.new_whiteboard();
                h.app.sweep.wb.new = None;
                if let Some(w) = h.app.data.extra.whiteboards.last().map(|w| w.id) {
                    h.app.wb_fill_template(w, t);
                }
            }),
        );
    }
    x(
        "whiteboard library with a board",
        app(|a| {
            a.new_whiteboard();
            a.sweep.wb.new = None;
            a.sweep.wb.open = None;
        }),
    );
    x(
        "dashboards list with one",
        app(|a| {
            a.section = Section::Dashboards;
            a.new_dashboard("2026년 하반기 팀 전체 진행 현황 대시보드");
            a.dash_open = None;
        }),
    );
    x(
        "dashboard workspace overview",
        app(|a| {
            a.section = Section::Dashboards;
            a.dash_open = Some(hubs::WORKSPACE_OVERVIEW);
        }),
    );
    x(
        "dashboard add card popup",
        Box::new(|h: &mut Ui1| {
            h.app.section = Section::Dashboards;
            h.app.new_dashboard("Workspace overview");
            h.settle();
            h.click("+ Add card");
        }),
    );
    x(
        "teams all with a team",
        app(|a| {
            a.section = Section::Teams;
            a.new_team();
            a.team_open = None;
            a.hub.teams = TeamsPage::All;
        }),
    );
    x(
        "doc view mode",
        app(|a| {
            a.section = Section::Docs;
            a.doc_open = a.data.docs.get(1).map(|d| d.id);
            a.doc_preview = true;
        }),
    );
    x(
        "doc with sub pages",
        app(|a| {
            a.section = Section::Docs;
            let d = a.data.docs.get(1).map(|d| d.id).unwrap_or(0);
            a.add_page(d);
            a.add_page(d);
            a.doc_open = Some(d);
        }),
    );
    x(
        "doc empty offer",
        Box::new(|h: &mut Ui1| {
            h.new_doc_focused();
        }),
    );
    x(
        "doc slash menu",
        Box::new(|h: &mut Ui1| {
            h.new_doc_focused();
            h.type_text("/");
            h.settle();
        }),
    );
    x(
        "doc options menu",
        Box::new(|h: &mut Ui1| {
            h.new_doc_focused();
            h.type_text("회의록 Meeting notes");
            h.settle();
            h.click("Doc options");
        }),
    );
    for (name, f) in [
        ("mine", DocFilter::Mine),
        ("shared", DocFilter::Shared),
        ("private", DocFilter::Private),
        ("meetings", DocFilter::Meetings),
        ("wikis", DocFilter::Wikis),
        ("archived", DocFilter::Archived),
    ] {
        x(
            &format!("docs filter {}", name),
            Box::new(move |h: &mut Ui1| {
                h.app.section = Section::Docs;
                h.app.doc_open = None;
                h.app.hub.docs = f;
            }),
        );
    }

    // --- the task window's parts
    x(
        "task long korean",
        app(|a| {
            home_list(a);
            a.open_task = Some(20_200);
        }),
    );
    x(
        "task long english",
        app(|a| {
            home_list(a);
            a.open_task = Some(20_100);
        }),
    );
    x(
        "task subtask breadcrumb",
        app(|a| {
            home_list(a);
            a.open_task = a
                .data
                .tasks
                .iter()
                .find(|t| t.parent.is_some())
                .map(|t| t.id);
        }),
    );
    for b in ["Task actions", "Watchers", "Time entries", "Task type"] {
        x(
            &format!("task popup {}", b),
            Box::new(move |h: &mut Ui1| {
                home_list(&mut h.app);
                h.app.open_task = Some(first_task(&h.app));
                h.settle();
                h.click(b);
            }),
        );
    }
    x(
        "task comments tab narrow",
        Box::new(|h: &mut Ui1| {
            home_list(&mut h.app);
            h.app.open_task = Some(first_task(&h.app));
            h.settle();
            h.click("Task tab Comments");
        }),
    );
    x(
        "task milestone relations attachment",
        app(|a| {
            home_list(a);
            let t = first_task(a);
            let others: Vec<u64> = a.data.tasks.iter().skip(1).take(3).map(|t| t.id).collect();
            if let Some(task) = a.data.tasks.iter_mut().find(|x| x.id == t) {
                task.milestone = true;
                task.waiting_on = others.clone();
                task.linked = others;
                task.attachments.push(Attachment {
                    file: "no-such-file".into(),
                    name: "아주-긴-첨부-파일-이름-요구사항-정의서-v3-최종.pdf".into(),
                    kind: MediaKind::File,
                    bytes: 123_456,
                    w: 0,
                    h: 0,
                    secs: 0.0,
                    thumb: None,
                });
            }
            a.open_task = Some(t);
        }),
    );
    x(
        "task inline add subtask",
        app(|a| {
            home_list(a);
            let t = first_task(a);
            a.open_task = Some(t);
            a.sub_adding = Some((t, String::new()));
        }),
    );

    // --- small dialogs drawn outside Modal
    x(
        "rename popup",
        app(|a| {
            home_list(a);
            let l = first_list(a);
            a.rename = Some((Target::List(l), "Website Redesign".into()));
        }),
    );
    x(
        "delete space confirm",
        app(|a| {
            home_list(a);
            let s = a.data.spaces[0].id;
            a.confirm_delete = Some(Target::Space(s));
        }),
    );
    x(
        "delete task confirm",
        app(|a| {
            home_list(a);
            a.confirm_delete = Some(Target::Task(20_200));
        }),
    );
    x(
        "status editor",
        app(|a| {
            home_list(a);
            a.edit_statuses = Some(a.data.spaces[0].id);
        }),
    );
    x("create task window", app(|a| a.open_quick_add(None)));
    x("palette empty", app(|a| a.palette = Some(String::new())));
    x(
        "palette no results",
        app(|a| a.palette = Some("zzzz".into())),
    );
    x(
        "palette korean",
        app(|a| a.palette = Some("아주 긴".into())),
    );
    x(
        "toast",
        app(|a| a.toast = Some(("설정을 저장했습니다. Saved".into(), now_ts() + 30))),
    );
    x(
        "inline add task",
        app(|a| {
            home_list(a);
            a.adding = Some(("s:to do".into(), String::new()));
        }),
    );
    x(
        "subtasks expanded",
        app(|a| {
            home_list(a);
            let t = first_task(a);
            a.expanded_tasks.insert(t);
        }),
    );
    x(
        "row rename in place",
        app(|a| {
            home_list(a);
            a.row.rename = Some((20_200, LONG_KO_TASK_ROW.into()));
        }),
    );

    // --- views and sidebar states
    x(
        "calendar week",
        app(|a| {
            home_list(a);
            a.view = ViewKind::Calendar;
            a.cal.week = true;
        }),
    );
    x(
        "timeline tray",
        app(|a| {
            home_list(a);
            a.view = ViewKind::Timeline;
            a.timeline.tray = true;
        }),
    );
    for (name, g) in [
        ("priority", GroupBy::Priority),
        ("assignee", GroupBy::Assignee),
        ("due date", GroupBy::DueDate),
        ("tags", GroupBy::Tags),
        ("none", GroupBy::None),
    ] {
        x(
            &format!("list grouped by {}", name),
            Box::new(move |h: &mut Ui1| {
                home_list(&mut h.app);
                h.app.prefs.group_by = g;
            }),
        );
    }
    x(
        "archived list page",
        app(|a| {
            let l = first_list(a);
            a.set_list_archived(l, true);
            a.section = Section::Home;
            a.loc = Location::List(l);
        }),
    );
    x(
        "sidebar search",
        app(|a| a.side.search = Some("web".into())),
    );
    x(
        "sidebar filter unread",
        app(|a| a.side.filter = Some("unread".into())),
    );
    x("sidebar collapsed", app(|a| a.sidebar_collapsed = true));
    x(
        "sidebar sheet (narrow home screen)",
        app(|a| {
            a.section = Section::Home;
            a.open_sheet();
        }),
    );
    x(
        "temporary rail app",
        app(|a| {
            a.temp_app = Some(Section::Goals);
            a.section = Section::Goals;
        }),
    );
    x(
        "recent tray",
        app(|a| {
            a.recent = vec![first_task(a), 20_200, 20_100];
        }),
    );
    for f in 1..=3u8 {
        x(
            &format!("inbox filter {}", f),
            Box::new(move |h: &mut Ui1| {
                h.app.section = Section::Home;
                h.app.loc = Location::Inbox;
                h.app.inbox_filter = f.into();
            }),
        );
    }
    x(
        "planner add row",
        app(|a| {
            a.section = Section::Planner;
            a.sweep.planner_add = Some((today(), String::new()));
        }),
    );
    x(
        "timesheet entries",
        app(|a| {
            a.section = Section::Timesheets;
            a.sheet_entries = true;
        }),
    );
    x(
        "timesheets all",
        app(|a| {
            a.section = Section::Timesheets;
            a.hub.sheets = SheetsTab::All;
        }),
    );

    // --- AI (the user plans to add it later; still checked)
    x(
        "ai on",
        app(|a| {
            ai_on(a);
            a.section = Section::Ai;
        }),
    );
    x(
        "ai conversation",
        app(|a| {
            ai_on(a);
            ai_job(a, AiState::Done);
            a.ai.chat.push(AiQuestion {
                question: "이번 주 general 채널에서 무슨 일이 있었는지 정리해 줘".into(),
                job: 900,
                chat: 901,
            });
            a.ai.open = Some(901);
            a.section = Section::Ai;
        }),
    );
    x(
        "ai failed",
        app(|a| {
            ai_on(a);
            ai_job(a, AiState::Failed("연결이 끊겼습니다".into()));
            a.ai.chat.push(AiQuestion {
                question: "q".into(),
                job: 900,
                chat: 901,
            });
            a.ai.open = Some(901);
            a.section = Section::Ai;
        }),
    );
    // a kept chat of mine shared with the team, and a teammate's
    x("ai chat shared", app(|a| ai_shared_chat(a, true)));
    x("ai chat teammate", app(|a| ai_shared_chat(a, false)));
    x(
        "modal share ai chat",
        app(|a| {
            ai_on(a);
            a.modal = Some(Modal::ShareAiChat { id: 902 });
        }),
    );
    x(
        "modal ai summary",
        app(|a| {
            ai_on(a);
            ai_job(a, AiState::Done);
            a.modal = Some(Modal::Ai(Box::new(AiDialog {
                job: 900,
                title: "Summary".into(),
                use_: AiUse::Read,
            })));
        }),
    );

    // --- together states
    x(
        "together owner",
        app(|a| {
            on_server(a);
            a.sync_rt.net_status = Some(wb_net::Status::Connected);
            a.sync_rt.role = Some(wb_net::Role::Owner);
            a.sync_rt.account = Some("me@team.io".into());
            a.modal = Some(a.together_modal());
        }),
    );
    x(
        "together editing address",
        app(|a| {
            on_server(a);
            a.sync_rt.editing_addr = true;
            a.modal = Some(a.together_modal());
        }),
    );
    x(
        "together shared folder",
        app(|a| {
            a.data.extra.shared = Some(crate::sync::SharedCfg {
                folder: "/Users/me/Library/CloudStorage/아주-긴-공유-폴더-이름/Workboard".into(),
                device: "dev1".into(),
                tag: 5,
                server: None,
            });
            a.modal = Some(a.together_modal());
        }),
    );

    // --- Modal variants screens() does not open
    let list_modals: Vec<(&str, fn(&mut App) -> Modal)> = vec![
        ("edit profile", |a| {
            a.open_profile();
            a.modal.take().unwrap()
        }),
        ("custom fields", |_| Modal::custom_fields()),
        ("automations list", |a| {
            Modal::automations(Some(Location::List(first_list(a))))
        }),
        ("template center browsing", |a| {
            a.template_center(templates::Category::Lists, Some(a.data.spaces[0].id))
        }),
        ("click apps", |_| Modal::ClickApps),
        ("notifications", |_| Modal::Notifications),
        ("themes", |_| Modal::Themes),
        ("shortcuts", |_| Modal::Shortcuts),
        ("workspace settings", |a| Modal::WorkspaceSettings {
            name: a.data.workspace.clone(),
        }),
        ("invite member only", |a| {
            on_server(a);
            a.sync_rt.role = Some(wb_net::Role::Member);
            Modal::Invite {
                emails: String::new(),
                role: "Member".into(),
            }
        }),
        ("saved messages", |_| Modal::SavedMessages),
        ("drafts", |_| Modal::Drafts),
        ("scheduled", |_| Modal::Scheduled),
        ("schedule at", |_| Modal::ScheduleAt {
            date: Some(today()),
            time: "09:00".into(),
        }),
        ("forward", |a| Modal::Forward {
            ch: general(a),
            msg: thread_parent(a),
            to: None,
            note: String::new(),
        }),
        ("unavailable", |_| menus::unavailable("Clips")),
        ("who am i", |_| Modal::WhoAmI {
            name: String::new(),
            email: String::new(),
        }),
        ("join team", |_| Modal::JoinTeam {
            addr: String::new(),
        }),
        ("delete workspace", |_| Modal::DeleteWorkspace {
            typed: String::new(),
        }),
        ("delete workspace server", |a| {
            on_server(a);
            Modal::DeleteWorkspace {
                typed: String::new(),
            }
        }),
        ("workflows", |_| Modal::Workflows {
            form: Default::default(),
        }),
        ("integrations", |a| Modal::Integrations {
            channel: general(a),
            form: Default::default(),
        }),
        ("new goal", |a| {
            a.open_new_goal(None);
            a.modal.take().unwrap()
        }),
        ("new goal sample", |a| {
            a.open_new_goal(Some(0));
            a.modal.take().unwrap()
        }),
        ("goal achieved", |a| Modal::GoalAchieved { id: add_goal(a) }),
        ("share goal", |a| {
            let id = add_goal(a);
            a.open_goal_share(id);
            a.modal.take().unwrap()
        }),
        ("goal settings", |a| Modal::GoalSettings { id: add_goal(a) }),
        ("switch user server", |a| {
            on_server(a);
            Modal::SwitchUser
        }),
        ("confirm remove person", |_| Modal::ConfirmRemove {
            kind: "person".into(),
            id: 9_001,
            name: LONG_KO_PERSON_NAME.into(),
        }),
        ("confirm remove meeting", |a| Modal::ConfirmRemove {
            kind: "meeting".into(),
            id: add_meeting(a),
            name: "주간 제품 기획 회의 – 출시 일정과 남은 작업 점검".into(),
        }),
    ];
    for (name, m) in list_modals {
        x(
            &format!("modal {}", name),
            Box::new(move |h: &mut Ui1| {
                home_list(&mut h.app);
                let modal = m(&mut h.app);
                h.app.modal = Some(modal);
            }),
        );
    }

    // --- hovers
    x(
        "hover rail icon",
        Box::new(|h: &mut Ui1| {
            h.settle();
            let p = h.find("Rail Planner").unwrap();
            h.hover(p);
            h.settle();
            h.settle();
        }),
    );
    x(
        "hover sidebar list row",
        Box::new(|h: &mut Ui1| {
            h.settle();
            let r = h.rect("Website Redesign");
            h.hover(r.center());
            h.settle();
        }),
    );
    x(
        "hover task row",
        Box::new(|h: &mut Ui1| {
            home_list(&mut h.app);
            h.settle();
            h.hover(h.find("Task Set up analytics dashboard").unwrap());
            h.settle();
        }),
    );
    v
}

const LONG_KO_TASK_ROW: &str = super::layout_audit::LONG_KO_TASK;
const LONG_KO_PERSON_NAME: &str = "김수한무거북이와두루미 삼천갑자동방삭";
