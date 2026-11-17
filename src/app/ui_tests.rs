// Click-through tests: the whole interface runs in a 1440x900 window and
// receives real pointer moves, presses, releases and typed text at the
// on-screen position of each control (found by its name), then the result
// is checked. This is the same input path a mouse click takes.

use super::*;
use egui::Pos2;

mod empty_states;

struct Ui1 {
    ctx: egui::Context,
    app: App,
    time: f64,
    tree: Vec<(String, egui::Rect)>,
    size: egui::Vec2,
    shapes: Vec<egui::epaint::ClippedShape>,
    /// files dropped on the window in the next frame
    dropped: Vec<egui::DroppedFile>,
    /// the mouse cursor the last frame asked for
    cursor: egui::CursorIcon,
}

impl Ui1 {
    fn new() -> Self {
        Self::sized(1440.0, 900.0)
    }

    // the smallest window the app allows is 900x560
    fn sized(w: f32, h: f32) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut h = Ui1 {
            ctx,
            app: App::build(None, Some(Data::seed())),
            time: 0.0,
            tree: vec![],
            size: vec2(w, h),
            shapes: vec![],
            dropped: vec![],
            cursor: egui::CursorIcon::Default,
        };
        // no notices on the wall clock: after 09:00 the seed's tasks due
        // today put a toast over the bottom of the window, so a test would
        // pass or fail by the time of day (notify_tests move the clock)
        h.app.notify_rt.last_clock = i64::MAX;
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.size)),
            time: Some(self.time),
            events,
            dropped_files: std::mem::take(&mut self.dropped),
            ..Default::default()
        };
        let app = &mut self.app;
        let out = self.ctx.run(input, |ctx| app.ui_frame(ctx));
        self.shapes = out.shapes;
        self.cursor = out.platform_output.cursor_icon;
        if let Some(update) = out.platform_output.accesskit_update {
            self.tree.clear();
            for (_, node) in update.nodes {
                let Some(b) = node.bounds() else { continue };
                let rect = Rect::from_min_max(
                    pos2(b.x0 as f32, b.y0 as f32),
                    pos2(b.x1 as f32, b.y1 as f32),
                );
                for name in [node.label(), node.placeholder(), node.value()]
                    .into_iter()
                    .flatten()
                {
                    self.tree.push((name.to_string(), rect));
                }
            }
        }
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    fn find(&self, name: &str) -> Option<Pos2> {
        self.tree
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r.center())
    }

    fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    // scroll with the mouse wheel over a control until it is on screen
    fn scroll_into_view(&mut self, name: &str) {
        for _ in 0..40 {
            let Some(p) = self.find(name) else { return };
            let bottom = self.size.y - 20.0;
            if p.y > 40.0 && p.y < bottom {
                return;
            }
            let at = pos2(p.x, self.size.y / 2.0);
            let dy = if p.y >= bottom { -120.0 } else { 120.0 };
            self.frame(vec![egui::Event::PointerMoved(at)]);
            self.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, dy),
                modifiers: Default::default(),
            }]);
            self.settle();
        }
    }

    fn click(&mut self, name: &str) {
        if !self.has(name) && self.app.modal.is_some() {
            // maybe further down a dialog that scrolls
            self.scroll_dialog_to(name);
        }
        self.scroll_into_view(name);
        let pos = self.find(name).unwrap_or_else(|| {
            let mut names: Vec<&String> = self.tree.iter().map(|(n, _)| n).collect();
            names.sort();
            names.dedup();
            panic!(
                "nothing named {:?} on screen; visible names: {:?}",
                name, names
            )
        });
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        self.settle();
    }

    fn click_at(&mut self, pos: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        self.settle();
    }

    // click near the left end of a wide control (a task row's name)
    fn click_left(&mut self, name: &str, dx: f32) {
        let r = self
            .tree
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("nothing named {:?}", name));
        self.click_at(pos2(r.left() + dx, r.center().y));
    }

    fn right_click(&mut self, name: &str, dx: f32) {
        self.scroll_into_view(name);
        let r = self
            .tree
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("nothing named {:?}", name));
        let pos = pos2(r.left() + dx, r.center().y);
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        self.settle();
    }

    // press on one control, move the mouse in small steps to another, release
    fn drag(&mut self, from: &str, to: &str) {
        let a = self
            .find(from)
            .unwrap_or_else(|| panic!("nothing named {:?}", from));
        let b = self
            .find(to)
            .unwrap_or_else(|| panic!("nothing named {:?}", to));
        self.frame(vec![egui::Event::PointerMoved(a)]);
        self.frame(vec![egui::Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        for i in 1..=12 {
            let p = a + (b - a) * (i as f32 / 12.0);
            self.frame(vec![egui::Event::PointerMoved(p)]);
        }
        self.frame(vec![egui::Event::PointerButton {
            pos: b,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]);
        self.settle();
    }

    // click the left-most (or right-most) control with this name, for names
    // shown twice (sidebar and page)
    fn click_side(&mut self, name: &str, rightmost: bool) {
        let pick = |h: &Self| {
            let mut hits: Vec<Rect> = h
                .tree
                .iter()
                .filter(|(n, _)| n == name)
                .map(|(_, r)| *r)
                .collect();
            hits.sort_by(|a, b| a.center().x.total_cmp(&b.center().x));
            if rightmost { hits.last() } else { hits.first() }
                .copied()
                .unwrap_or_else(|| panic!("nothing named {:?}", name))
        };
        for _ in 0..30 {
            let r = pick(self);
            if r.center().y > 40.0 && r.center().y < 880.0 {
                break;
            }
            let dy = if r.center().y >= 880.0 { -120.0 } else { 120.0 };
            self.frame(vec![egui::Event::PointerMoved(pos2(r.center().x, 450.0))]);
            self.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, dy),
                modifiers: Default::default(),
            }]);
            self.settle();
        }
        let r = pick(self);
        self.click_at(r.center());
    }

    fn hover(&mut self, pos: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.settle();
    }

    /// Whether the soft hover fill (`widgets::hover_fill`) is painted under `pos`.
    fn hover_fill_at(&self, pos: Pos2) -> bool {
        let fill = crate::widgets::hover_fill_for(self.ctx.style().visuals.dark_mode);
        self.shapes.iter().any(|c| match &c.shape {
            egui::Shape::Rect(r) => r.fill == fill && r.rect.contains(pos),
            _ => false,
        })
    }

    fn type_text(&mut self, text: &str) {
        self.frame(vec![egui::Event::Text(text.to_string())]);
        self.frame(vec![]);
    }

    fn key(&mut self, key: egui::Key) {
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Default::default(),
        }]);
        self.settle();
    }
}

#[test]
fn click_sidebar_inbox_and_my_tasks() {
    let mut h = Ui1::new();
    h.click("Inbox");
    assert_eq!(h.app.loc, Location::Inbox);
    h.click("My Tasks");
    assert_eq!(h.app.loc, Location::MyTasks);
    h.click("Today & Overdue");
    assert_eq!(h.app.loc, Location::TodayOverdue);
    h.click("Personal List");
    let pl = h
        .app
        .personal_list_id()
        .expect("personal list created by the click");
    assert_eq!(h.app.loc, Location::List(pl));
}

#[test]
fn click_create_menu_task_and_type_it() {
    let mut h = Ui1::new();
    let n = h.app.data.tasks.len();
    h.click("Create");
    h.click("Task");
    assert!(h.app.quick_add.is_some(), "the Create Task window opens");
    h.type_text("Clicked into existence");
    h.key(egui::Key::Enter);
    assert_eq!(h.app.data.tasks.len(), n + 1);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.name == "Clicked into existence"));
}

#[test]
fn click_add_task_row_in_list() {
    let mut h = Ui1::new();
    h.click("Add Task in TO DO");
    h.type_text("Inline task");
    h.key(egui::Key::Enter);
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Inline task")
        .expect("added from the list row");
    assert_eq!(t.status, "to do");
}

#[test]
fn click_status_circle_and_pick_status() {
    let mut h = Ui1::new();
    h.click("Status of Set up analytics dashboard");
    h.click("IN PROGRESS");
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Set up analytics dashboard")
        .unwrap();
    assert_eq!(t.status, "in progress");
}

#[test]
fn click_task_row_opens_it_then_complete_and_timer() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.expect("task window opens");
    h.click("Start timer");
    assert!(
        h.app.task(id).unwrap().timer_started.is_some(),
        "the play button starts the timer"
    );
    h.click("Mark complete");
    assert_eq!(
        h.app.task(id).unwrap().status,
        "complete",
        "the check button completes the task"
    );
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none(), "Esc closes the task");
}

#[test]
fn click_invite_in_rail_and_send() {
    let mut h = Ui1::new();
    let n = h.app.data.members.len();
    h.click("Rail Invite");
    assert!(matches!(h.app.modal, Some(Modal::Invite { .. })));
    h.type_text("lee.jiwoo@team.kr");
    h.click("Send invite");
    assert_eq!(h.app.data.members.len(), n + 1);
    assert_eq!(
        h.app.data.members.last().unwrap().email,
        "lee.jiwoo@team.kr"
    );
    assert!(h.app.modal.is_none(), "the dialog closes after sending");
}

#[test]
fn a_member_on_a_server_sees_no_invite_or_workspace_settings() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.settle();
    assert!(!h.has("Rail Invite"), "no Invite in the rail");
    // People lists everyone, but without Invite people
    h.app.open_settings("People");
    h.settle();
    assert!(h.has("Settings People") && !h.has("Invite people"));
    // no General (workspace name and colour); asking for it opens my settings
    assert!(!h.has("Settings General"));
    h.app.open_settings("General");
    h.settle();
    assert!(h.has("Full name") && !h.has("Workspace name"));
    // nor what changes the workspace for everyone; Templates stays, to use them
    for s in ["Apps", "Custom fields", "Tag manager", "Automations"] {
        assert!(!h.has(&format!("Settings {}", s)), "no {}", s);
    }
    h.click("Settings Templates");
    assert!(!h.has("New task template"));
    // reached anyway, those dialogs only say who can change them
    h.app.section = Section::Home;
    h.app.modal = Some(Modal::ClickApps);
    h.settle();
    assert!(h.has("Only the owner or an admin of this workspace can change this."));
    assert!(!h.has("Priorities"));
    h.click("Close");
    // teams can be looked at, not made
    h.app.data.extra.teams.push(Team {
        id: 777,
        name: "Design".into(),
        members: vec![h.app.data.me],
        color: [1, 2, 3],
    });
    h.app.section = Section::Teams;
    h.settle();
    assert!(!h.has("Create team"));
    h.app.team_open = Some(777);
    h.app.hub_filter = 9;
    h.settle();
    assert!(!h.has("Delete team"));
    h.app.section = Section::Home;
    // Share keeps Copy link but has no invite field
    h.app.modal = Some(Modal::Share {
        emails: String::new(),
    });
    h.settle();
    assert!(h.has("Copy link") && !h.has("Invite by email"));
    // Import and export keeps CSV but not Import from Slack (it brings people in)
    h.app.modal = Some(Modal::Import {
        path: String::new(),
        result: String::new(),
    });
    h.settle();
    assert!(h.has("Export current tasks to CSV") && !h.has("Import from Slack"));
    h.app
        .open_slack_export(std::path::Path::new("/nonexistent.zip"));
    assert!(matches!(h.app.modal, Some(Modal::Import { .. })));
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Only the owner or an admin of this workspace can import from Slack.")
    );
    // the invite dialog reached anyway (a link) only says who can invite
    h.app.modal = Some(Modal::Invite {
        emails: String::new(),
        role: "Member".into(),
    });
    h.settle();
    assert!(h.has("Only the owner or an admin of this workspace can invite people."));
    assert!(!h.has("Send invite"));
    let n = h.app.data.members.len();
    h.click("Close");
    assert!(h.app.modal.is_none() && h.app.data.members.len() == n);
    // an admin gets all of it back
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.settle();
    assert!(h.has("Rail Invite"));
    h.click("Rail Invite");
    assert!(h.has("Send invite"));
    h.key(egui::Key::Escape);
    h.app.open_settings("General");
    h.settle();
    assert!(h.has("Settings General") && h.has("Workspace name"));
    assert!(h.has("Settings Automations") && h.has("Settings Custom fields"));
    h.app.section = Section::Teams;
    h.settle();
    assert!(h.has("Delete team"));
    // before the server is reached, the role in People decides
    h.app.section = Section::Home;
    h.app.sync_rt.role = None;
    let me = h.app.data.me;
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == me)
        .unwrap()
        .role = "Member".into();
    h.settle();
    assert!(!h.has("Rail Invite"));
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == me)
        .unwrap()
        .role = "Owner".into();
    h.settle();
    assert!(h.has("Rail Invite"));
}

#[test]
fn click_view_tabs() {
    let mut h = Ui1::new();
    for (name, v) in [
        ("View Board", ViewKind::Board),
        ("View Calendar", ViewKind::Calendar),
        ("View Gantt", ViewKind::Gantt),
        ("View Timeline", ViewKind::Timeline),
        ("View Table", ViewKind::Table),
        ("View List", ViewKind::List),
    ] {
        h.click(name);
        assert!(h.app.view == v, "{} selects the view", name);
    }
}

#[test]
fn click_channel_and_send_message() {
    let mut h = Ui1::new();
    h.click("general");
    assert_eq!(h.app.section, Section::Chat);
    let cid = h.app.chat_open.unwrap();
    let n = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    h.click("Message composer");
    h.type_text("hello from a click test");
    h.key(egui::Key::Enter);
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(ch.messages.len(), n + 1);
    assert_eq!(ch.messages.last().unwrap().text, "hello from a click test");
}

#[test]
fn click_more_docs_new_doc() {
    let mut h = Ui1::new();
    let n = h.app.data.docs.len();
    h.click("Rail More");
    h.click("Docs");
    assert_eq!(h.app.section, Section::Docs);
    h.click("New Doc");
    assert_eq!(h.app.data.docs.len(), n + 1);
    assert!(h.app.doc_open.is_some(), "the new doc opens");
}

#[test]
fn click_workspace_menu_people_and_profile_themes() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("People");
    assert_eq!(h.app.section, Section::People);
    h.click("Profile menu");
    h.click("Themes");
    assert!(matches!(h.app.modal, Some(Modal::Themes)));
    h.click("Dark");
    assert!(h.app.data.dark, "dark theme switched on");
}

#[test]
fn click_search_and_open_result() {
    let mut h = Ui1::new();
    h.click("Search");
    assert!(h.app.palette.is_some());
    h.type_text("passport");
    h.key(egui::Key::Enter);
    let id = h.app.open_task.expect("Enter opens the first match");
    assert_eq!(h.app.task(id).unwrap().name, "Renew passport");
}

#[test]
fn click_back_and_forward() {
    let mut h = Ui1::new();
    let start = h.app.loc;
    h.click("Inbox");
    h.click("Replies");
    h.click("Go back");
    assert_eq!(h.app.loc, Location::Inbox);
    h.click("Go back");
    assert_eq!(h.app.loc, start);
    h.click("Go forward");
    assert_eq!(h.app.loc, Location::Inbox);
}

#[test]
fn click_whiteboard_template() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Whiteboards");
    assert_eq!(h.app.section, Section::Whiteboards);
    h.click("Action Plan");
    let w = h
        .app
        .data
        .extra
        .whiteboards
        .last()
        .expect("a whiteboard was made");
    assert_eq!(w.name, "Action Plan");
    assert!(w.notes.len() >= 4, "the template's sticky notes are there");
}

#[test]
fn click_form_template_and_submit() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Forms");
    h.click("IT Requests");
    let fid = h.app.form_open.expect("form opens");
    assert_eq!(
        h.app
            .data
            .extra
            .forms
            .iter()
            .find(|f| f.id == fid)
            .unwrap()
            .fields
            .len(),
        3
    );
    h.click("Fill out");
    let n = h.app.data.tasks.len();
    h.click("Submit");
    assert_eq!(h.app.data.tasks.len(), n + 1, "submitting creates a task");
}

#[test]
fn click_inbox_item_later_and_clear() {
    let mut h = Ui1::new();
    h.click("Inbox");
    // hover the first inbox item to reveal its buttons
    let row = h
        .find("Inbox Primary")
        .map(|p| pos2(700.0, p.y + 120.0))
        .unwrap();
    h.hover(row);
    h.click("Later");
    assert_eq!(h.app.data.extra.inbox_later.len(), 1);
    h.click("Inbox Later");
    h.hover(row);
    h.click("Clear");
    assert_eq!(h.app.data.extra.inbox_cleared.len(), 1);
}

#[test]
fn click_docs_template_and_timesheet_options() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Timesheets");
    assert_eq!(h.app.section, Section::Timesheets);
    h.click("All timesheets");
    assert_eq!(h.app.hub_filter, 1);
    h.click("My timesheet");
    assert_eq!(h.app.hub_filter, 0);
}

#[test]
fn drag_board_card_to_another_column() {
    let mut h = Ui1::new();
    h.click("View Board");
    h.drag(
        "Card Write copy for landing page",
        "Card Fix navigation on mobile",
    );
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Write copy for landing page")
        .unwrap();
    assert_eq!(
        t.status, "in progress",
        "dropping a card in another column moves it"
    );
}

#[test]
fn right_click_delete_then_restore_from_trash() {
    let mut h = Ui1::new();
    h.right_click("Task Collect feedback from beta users", 120.0);
    h.click("Delete");
    assert!(!h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.name == "Collect feedback from beta users"));
    h.click("Profile menu");
    h.click("Trash");
    assert!(matches!(h.app.modal, Some(Modal::Trash)));
    h.click("Restore");
    assert!(
        h.app
            .data
            .tasks
            .iter()
            .any(|t| t.name == "Collect feedback from beta users"),
        "restored from the trash"
    );
    h.key(egui::Key::Escape);
    assert!(h.app.modal.is_none());
}

#[test]
fn click_goals_dashboards_teams_create() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Goals");
    let g = h.app.data.extra.goals.len();
    h.click("Set a Goal");
    assert_eq!(h.app.data.extra.goals.len(), g + 1);

    h.click("Rail More");
    h.click("Dashboards");
    let d = h.app.data.extra.dashboards.len();
    h.click("Simple Dashboard");
    assert_eq!(h.app.data.extra.dashboards.len(), d + 1);

    h.click("Rail Teams");
    assert_eq!(h.app.section, Section::Teams);
    let n = h.app.data.extra.teams.len();
    h.click("Create team");
    assert_eq!(h.app.data.extra.teams.len(), n + 1);
}

#[test]
fn click_settings_dialogs_open_and_close() {
    let mut h = Ui1::new();
    for item in [
        "Apps",
        "Templates",
        "Custom fields",
        "Automations",
        "Tag manager",
    ] {
        h.click("Workspace menu");
        h.click(item);
        assert!(h.app.modal.is_some(), "{} opens a dialog", item);
        h.key(egui::Key::Escape);
        assert!(h.app.modal.is_none(), "Esc closes {}", item);
    }
    for item in [
        "Notifications",
        "Themes",
        "Labs",
        "Keyboard shortcuts",
        "Help",
    ] {
        h.click("Profile menu");
        h.click(item);
        assert!(h.app.modal.is_some(), "{} opens a dialog", item);
        h.key(egui::Key::Escape);
        assert!(h.app.modal.is_none(), "Esc closes {}", item);
    }
}

#[test]
fn check_for_updates_explains_why_it_cannot_run_outside_the_installed_app() {
    let mut h = Ui1::new();
    h.click("Profile menu");
    h.click("Check for updates");
    // tests don't run inside Workboard.app, so there is no Sparkle to ask
    match &h.app.modal {
        Some(Modal::Unavailable { title, body }) => {
            assert_eq!(title, "Check for updates");
            assert!(body.contains(env!("CARGO_PKG_VERSION")), "{}", body);
        }
        other => panic!("expected an explanation, got {:?}", other.is_some()),
    }
    h.key(egui::Key::Escape);
    assert!(h.app.modal.is_none());
}

/// Red dots (the theme's danger color) painted inside the control named `name`.
fn red_dots_in(h: &Ui1, name: &str) -> usize {
    let danger = h.app.t().danger;
    let Some((_, rect)) = h.tree.iter().rev().find(|(n, _)| n == name) else {
        return 0;
    };
    fn count(s: &egui::Shape, rect: egui::Rect, danger: Color32) -> usize {
        match s {
            egui::Shape::Circle(c) if c.fill == danger && rect.contains(c.center) => 1,
            egui::Shape::Vec(v) => v.iter().map(|s| count(s, rect, danger)).sum(),
            _ => 0,
        }
    }
    h.shapes
        .iter()
        .map(|c| count(&c.shape, *rect, danger))
        .sum()
}

/// Filled circles of `color` painted inside the control named `name`.
fn dots_in(h: &Ui1, name: &str, color: Color32) -> usize {
    let Some((_, rect)) = h.tree.iter().rev().find(|(n, _)| n == name) else {
        return 0;
    };
    fn count(s: &egui::Shape, rect: egui::Rect, color: Color32) -> usize {
        match s {
            egui::Shape::Circle(c) if c.fill == color && rect.contains(c.center) => 1,
            egui::Shape::Vec(v) => v.iter().map(|s| count(s, rect, color)).sum(),
            _ => 0,
        }
    }
    h.shapes.iter().map(|c| count(&c.shape, *rect, color)).sum()
}

/// Grey presence rings ("not online") painted inside the control named `name`.
fn rings_in(h: &Ui1, name: &str) -> usize {
    let grey = h.app.t().text3;
    let Some((_, rect)) = h.tree.iter().rev().find(|(n, _)| n == name) else {
        return 0;
    };
    fn count(s: &egui::Shape, rect: egui::Rect, grey: Color32) -> usize {
        match s {
            egui::Shape::Circle(c)
                if c.stroke.color == grey && c.radius < 4.0 && rect.contains(c.center) =>
            {
                1
            }
            egui::Shape::Vec(v) => v.iter().map(|s| count(s, rect, grey)).sum(),
            _ => 0,
        }
    }
    h.shapes.iter().map(|c| count(&c.shape, *rect, grey)).sum()
}

#[test]
fn green_dots_show_who_is_really_connected() {
    use crate::widgets::ONLINE;
    let mut h = Ui1::new();
    let me = h.app.data.me;
    // a DM with someone else, open in the sidebar
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone();
    let has_dm = h
        .app
        .data
        .channels
        .iter()
        .any(|c| c.kind == crate::model::ChannelKind::Direct && c.members.contains(&other.id));
    if !has_dm {
        let mut c = h.app.data.channels[0].clone();
        c.id = 987_654;
        c.name = String::new();
        c.kind = crate::model::ChannelKind::Direct;
        c.members = vec![me, other.id];
        c.messages.clear();
        h.app.data.channels.push(c);
    }
    h.app.section = Section::Chat;
    h.settle();
    let row = other.name.clone();
    assert!(h.has(&row), "the DM is listed");
    // alone: no "online" at all, so no dots
    assert_eq!(dots_in(&h, "Profile menu", ONLINE), 0);
    assert_eq!(dots_in(&h, &row, ONLINE) + rings_in(&h, &row), 0);
    h.click("Profile menu");
    assert!(!h.has("Online"), "no claim of being online");
    h.key(egui::Key::Escape);

    // on a server, connected; the server says only I am online
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.online = Some(vec![me]);
    h.settle();
    assert_eq!(dots_in(&h, "Profile menu", ONLINE), 1, "I am connected");
    assert_eq!(dots_in(&h, &row, ONLINE), 0, "they aren't");
    assert_eq!(rings_in(&h, &row), 1, "a grey ring says so");
    h.click("Profile menu");
    assert!(h.has("Online"));
    h.key(egui::Key::Escape);

    // they open Workboard
    h.app.sync_rt.online = Some(vec![me, other.id]);
    h.settle();
    assert_eq!(dots_in(&h, &row, ONLINE), 1);
    assert_eq!(rings_in(&h, &row), 0);

    // the connection drops: who is online isn't known, and I am offline
    h.app.sync_rt.net_status = Some(wb_net::Status::Reconnecting("gone".into()));
    h.settle();
    assert_eq!(dots_in(&h, "Profile menu", ONLINE), 0);
    assert_eq!(rings_in(&h, "Profile menu"), 1);
    assert_eq!(dots_in(&h, &row, ONLINE) + rings_in(&h, &row), 0);
    h.click("Profile menu");
    assert!(h.has("Offline") && !h.has("Online"));
    h.key(egui::Key::Escape);
}

#[test]
fn a_waiting_update_puts_a_red_dot_on_the_profile_and_check_for_updates() {
    let mut h = Ui1::new();
    // nothing waiting: no dots
    crate::updater::pretend_update_available(false);
    h.settle();
    assert_eq!(red_dots_in(&h, "Profile menu"), 0);
    h.click("Profile menu");
    assert_eq!(red_dots_in(&h, "Check for updates"), 0);
    h.key(egui::Key::Escape);

    // the server has a newer version
    crate::updater::pretend_update_available(true);
    h.settle();
    assert_eq!(red_dots_in(&h, "Profile menu"), 1, "the profile shows it");
    h.click("Profile menu");
    assert_eq!(
        red_dots_in(&h, "Check for updates"),
        1,
        "the menu row says where to go"
    );
    assert_eq!(red_dots_in(&h, "Settings"), 0, "only that row");
    crate::updater::pretend_update_available(false);
}

#[test]
fn an_online_only_tool_says_workboard_has_no_such_service() {
    let mut h = Ui1::new();
    h.click("Profile menu");
    h.click("Talk to Text");
    // the team server exists now, so the note must not promise that setting
    // one up makes AI, video or voice processing work
    match &h.app.modal {
        Some(Modal::Unavailable { title, body }) => {
            assert_eq!(title, "Talk to Text");
            assert!(
                body.starts_with("Talk to Text needs an online service"),
                "{}",
                body
            );
            assert!(body.contains("Workboard doesn't have"), "{}", body);
            assert!(!body.contains("server"), "{}", body);
        }
        other => panic!("expected an explanation, got {:?}", other.is_some()),
    }
    h.key(egui::Key::Escape);
    assert!(h.app.modal.is_none());
}

#[test]
fn click_message_replies_opens_thread() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("1 reply");
    assert!(h.app.thread_open.is_some(), "the thread panel opens");
}

#[test]
fn click_every_rail_more_app() {
    let mut h = Ui1::new();
    for (item, sec) in [
        ("Docs", Section::Docs),
        ("Dashboards", Section::Dashboards),
        ("Whiteboards", Section::Whiteboards),
        ("Forms", Section::Forms),
        ("Goals", Section::Goals),
        ("Timesheets", Section::Timesheets),
    ] {
        h.click("Rail More");
        h.click(item);
        assert_eq!(h.app.section, sec, "{} opens", item);
    }
    for item in ["Rail Home", "Rail Planner", "Rail AI", "Rail Teams"] {
        h.click(item);
    }
    assert_eq!(h.app.section, Section::Teams);
}

#[test]
fn click_thread_reply_and_hover_reaction() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat_open.unwrap();
    h.click("1 reply");
    let mid = h.app.thread_open.unwrap();
    h.click("Reply composer");
    h.type_text("on it");
    h.key(egui::Key::Enter);
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let m = ch.messages.iter().find(|m| m.id == mid).unwrap();
    assert_eq!(
        m.replies.last().unwrap().text,
        "on it",
        "reply posted in the thread"
    );
    let before = m.reactions.iter().map(|r| r.users.len()).sum::<usize>();

    // hover the last message to show its toolbar, then react with a party emoji
    let p = h
        .find("@Me can you check the beta feedback list when you have a minute?")
        .unwrap();
    h.hover(p);
    h.click("\u{1F389}");
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let last = ch
        .messages
        .iter()
        .find(|m| m.text.starts_with("@Me"))
        .unwrap();
    assert!(
        last.reactions.iter().any(|r| r.emoji == "\u{1F389}"),
        "reaction added from the hover toolbar"
    );
    let _ = before;
}

#[test]
fn click_huddle_starts_a_call_in_general() {
    let mut h = Ui1::new();
    h.click("Huddle");
    assert!(matches!(h.app.modal, Some(Modal::Call { .. })));
    h.click("Start call");
    assert!(h.app.modal.is_none());
    assert_eq!(h.app.section, Section::Chat);
    let ch = h.app.chat_open.unwrap();
    let m = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == ch)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone();
    assert!(m.call.is_some(), "a call message with a Join button");
    assert!(h.has("Join call"));
}

#[test]
fn click_work_together_opens_sharing() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Work together");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
}

#[test]
fn work_together_in_a_shared_folder_says_how_teammates_join_and_keeps_the_path_on_one_line() {
    let mut h = Ui1::new();
    // no sync folder found on this computer: nothing "above" to pick
    h.click("Workspace menu");
    h.click("Work together");
    let hint = |h: &Ui1, s: &str| !h.painted_text(s).is_empty();
    if crate::sync::suggested_folders().is_empty() {
        assert!(hint(&h, "Paste a folder path"));
        assert!(!hint(&h, "Paste a folder path, or pick one above"));
    } else {
        assert!(h.has("Found on this computer"));
        assert!(hint(&h, "Paste a folder path, or pick one above"));
    }
    h.key(egui::Key::Escape);
    let dir = std::env::temp_dir()
        .join(format!("wb-ui-{:x}", crate::sync::random_u64()))
        .join("audit-states-shared-folder")
        .join("team-workspace-folder")
        .join("_shared");
    std::fs::create_dir_all(&dir).unwrap();
    let full = dir.to_str().unwrap().to_string();
    h.app.share_here(&full).unwrap();
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    // the long path is cut from the front at folder names, on one line
    let short = super::together::short_path(&full, 48);
    assert!(short.starts_with('\u{2026}') && short.ends_with("/team-workspace-folder/_shared"));
    assert!(h.has(&short), "{}", short);
    let at = h.painted_text(&short);
    assert_eq!(at.len(), 1, "one line: {:?}", at);
    assert!(at[0].0.height() < 24.0, "{:?}", at);
    // counts computers, and says what teammates do on theirs
    assert!(h.has("No other computer is working in this folder yet. Share the folder with your teammates in your sync app. On their computer they open Work together, paste the folder's path and choose Join this workspace."));
    assert!(h.has("Tasks, chat and docs show up for everyone a few seconds after their sync app copies the folder. Calls open in the browser from a link posted in chat."));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap().parent().unwrap());
}

#[test]
fn work_together_fits_a_small_window_and_scrolls() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.click("Workspace menu");
    h.click("Work together");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
    // the title and the close button stay on screen
    let close = h.rect("Close");
    assert!(close.top() >= 0.0 && close.bottom() <= 560.0, "{:?}", close);
    // the bottom of the dialog is reached by scrolling inside it
    h.scroll_into_view("Connect");
    let connect = h.rect("Connect");
    assert!(connect.bottom() <= 560.0, "{:?}", connect);
    assert!(h.rect("Close").top() >= 0.0, "the title stays put");
}

#[test]
fn a_workspace_on_a_server_can_still_join_another_team() {
    // this workspace is already on a team server (its own team)
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.settle();
    h.click("Workspace menu");
    h.click("Join a team");
    assert!(matches!(&h.app.modal, Some(Modal::JoinTeam { .. })));
    assert!(h.has("Join a team") && h.has("Server address") && h.has("Email entry"));
    assert!(h.has("The team's workspace opens as a new workspace. The ones you already have stay in the workspace menu, as they are."));
    assert!(!h.has("Back"), "no first-start choices to go back to");
    h.click("Server address");
    h.type_text("127.0.0.1:2");
    h.click("Invite code");
    h.click("Invite code entry");
    h.type_text("abcd-efgh-jkmn");
    h.click("Choose a password entry");
    h.type_text("pw-12345678");
    h.click("Repeat password entry");
    h.type_text("pw-12345678");
    h.settle();
    h.click("Connect");
    let j = h.app.sync_rt.joining.as_ref().expect("connecting");
    assert_eq!(j.addr, "127.0.0.1:2");
    assert!(matches!(&j.how, together::How::Invite { code, .. } if code == "abcd-efgh-jkmn"));
    assert_eq!(
        h.app
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.as_deref()),
        Some("127.0.0.1:1"),
        "this workspace stays on its server while joining"
    );
    // giving up goes back to this workspace's own server
    h.click("Cancel");
    assert!(h.app.sync_rt.joining.is_none());
    assert_eq!(
        h.app
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.as_deref()),
        Some("127.0.0.1:1")
    );
}

#[test]
fn work_together_connects_to_a_server_and_shows_the_status() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Work together");
    assert!(h.has("Or connect to your team's server"));
    h.click("Server address");
    h.type_text("127.0.0.1:1");
    h.settle();
    // email and password first; an invite code on its own tab
    assert!(h.has("Email entry") && h.has("Password entry") && h.has("Forgot password?"));
    h.click("Invite code");
    h.click("Invite code entry");
    h.type_text("abcd-efgh-jkmn");
    h.click("Choose a password entry");
    h.type_text("pw-123");
    h.click("Repeat password entry");
    h.type_text("pw-123");
    h.settle();
    h.click("Connect");
    assert!(h.app.sync_rt.joining.is_none(), "too short");
    assert!(h.has("Choose a password of at least 8 characters."));
    h.click("Choose a password entry");
    h.type_text("45678");
    h.click("Repeat password entry");
    h.type_text("45678");
    h.settle();
    h.click("Connect");
    let j = h.app.sync_rt.joining.as_ref().expect("connecting");
    assert_eq!(j.addr, "127.0.0.1:1");
    assert_eq!(
        j.how,
        together::How::Invite {
            code: "abcd-efgh-jkmn".into(),
            email: String::new(),
            password: "pw-12345678".into()
        }
    );
    assert!(h.has("Cancel"));
    h.click("Cancel");
    assert!(h.app.sync_rt.joining.is_none());
    assert!(h.has("Connect"));

    // a workspace on a server shows where and how the connection is doing
    let tag = 5;
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert!(h.has("This workspace is on a server"));
    assert!(h.has("127.0.0.1:1"));
    assert!(h.has("Status: Disconnected"));
    // nothing listens on port 1, so the app keeps trying
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !matches!(
        h.app.sync_rt.net_status,
        Some(wb_net::Status::Reconnecting(_))
    ) && std::time::Instant::now() < until
    {
        h.app.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.settle();
    assert!(
        h.has("Status: Reconnecting\u{2026}"),
        "shows that it is reconnecting"
    );
    // why, in plain words, without the system's error text
    assert!(
        h.has("Nothing answers at 127.0.0.1:1. Check that workboard-server is running there. Trying again every few seconds."),
        "says why"
    );
    assert!(!h.tree.iter().any(|(n, _)| n.contains("os error")));
    let status = h.painted_text("Status: Reconnecting\u{2026}");
    assert!(
        !status.is_empty() && status.iter().all(|(_, c)| *c == super::together::WARN),
        "in the warning colour: {:?}",
        status
    );
    // still failing after a few tries: it says it can't reach the server
    h.app.sync_rt.fails = super::together::FAILS_BEFORE_CANT_REACH;
    h.settle();
    assert!(h.has("Status: Can't reach the server"));
    // owners and admins see everyone's access
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    let me = h.app.data.me;
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone();
    h.app.sync_rt.people = vec![
        wb_net::Person {
            member: me,
            role: wb_net::Role::Owner,
            devices: 1,
            invite: None,
            account: true,
        },
        wb_net::Person {
            member: other.id,
            role: wb_net::Role::Member,
            devices: 0,
            invite: Some("ABCD-EFGH-JKMN".into()),
            account: false,
        },
    ];
    h.settle();
    let my_name = h.app.data.member(me).unwrap().name.clone();
    assert!(h.has(&format!("Signed in as {} (Owner)", my_name)));
    assert!(h.has("People"));
    assert!(h.has("Make admin") && h.has("Remove access") && h.has("New code"));
    // row buttons leave room around the text, all the same height
    let heights: Vec<f32> = ["Make admin", "Remove access", "New code"]
        .iter()
        .map(|n| h.tree.iter().find(|(t, _)| t == n).unwrap().1.height())
        .collect();
    assert!(
        heights
            .iter()
            .all(|&ht| ht >= 26.0 && (ht - heights[0]).abs() < 0.5),
        "row button heights {:?}",
        heights
    );
    assert!(h.has(&format!("Code for {}: ABCD-EFGH-JKMN", other.name)));
    h.click("Copy");
    // the owner can go back to a backup, with a second click to confirm
    assert!(h.has("Backups") && h.has("Show backups"));
    h.app.sync_rt.backups = Some(vec![
        wb_net::Backup {
            name: "hourly-20261115-03.json".into(),
            bytes: 2048,
        },
        wb_net::Backup {
            name: "daily-20261115.json".into(),
            bytes: 2048,
        },
    ]);
    h.settle();
    assert!(h.has("2026-11-15 03:00 (hourly)") && h.has("2026-11-15 (daily)"));
    h.click("Restore");
    assert!(h.app.sync_rt.restore_confirm.is_some(), "asks first");
    assert!(h.has("Yes, restore"));
    h.click("Cancel");
    assert!(h.app.sync_rt.restore_confirm.is_none() && !h.has("Yes, restore"));
    // an admin can't make a code for the owner
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.app.sync_rt.people[0].member = other.id;
    h.app.sync_rt.people[1] = wb_net::Person {
        member: me,
        role: wb_net::Role::Admin,
        devices: 1,
        invite: None,
        account: false,
    };
    h.settle();
    assert!(
        !h.has("Invite code") && !h.has("New code"),
        "no code button on the owner's row for an admin"
    );
    // a member sees no People list
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.settle();
    assert!(!h.has("People") && !h.has("Make admin"));
    // access removed: sign in again with email and password, or a new code
    h.app.sync_rt.form = together::Form::default();
    h.app.sync_rt.net_status = Some(wb_net::Status::Refused(
        "Your access to this server was removed.".into(),
    ));
    h.settle();
    assert!(h.has("Sign in"));
    h.click("Email entry");
    h.type_text("me@team.io");
    h.click("Password entry");
    h.type_text("secret-pass");
    h.settle();
    h.click("Sign in");
    assert_eq!(
        h.app.sync_rt.resign,
        Some(together::How::Account {
            email: "me@team.io".into(),
            password: "secret-pass".into()
        })
    );
    h.app.sync_rt.net_status = Some(wb_net::Status::Refused(
        "That email and password don't match an account on this server.".into(),
    ));
    h.settle();
    h.click("Invite code");
    h.click("Invite code entry");
    h.type_text("NEWC-ODEX-XXXX");
    h.click("Choose a password entry");
    h.type_text("new-secret-1");
    h.click("Repeat password entry");
    h.type_text("new-secret-1");
    h.settle();
    h.click("Sign in");
    assert!(
        matches!(&h.app.sync_rt.resign, Some(together::How::Invite { code, .. }) if code == "NEWC-ODEX-XXXX")
    );
    h.click("Reconnect now");
    assert!(h.app.sync_rt.net.is_none(), "starts over on the next frame");
    h.click("Stop sharing");
    assert!(h.app.data.extra.shared.is_none());
}

impl Ui1 {
    // where the open dialog is on screen
    fn modal_rect(&self) -> Rect {
        self.try_modal_rect().expect("a dialog is drawn")
    }

    fn try_modal_rect(&self) -> Option<Rect> {
        let m = self.app.modal.as_ref()?;
        self.ctx
            .memory(|mem| mem.area_rect(super::menus::modal_window_id(m)))
    }

    // turn the mouse wheel over the open dialog until a control in it shows
    fn scroll_dialog_to(&mut self, name: &str) {
        for _ in 0..30 {
            if self.has(name) {
                break;
            }
            let Some(r) = self.try_modal_rect() else {
                return;
            };
            let at = r.center();
            self.frame(vec![egui::Event::PointerMoved(at)]);
            self.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -120.0),
                modifiers: Default::default(),
            }]);
            self.settle();
        }
    }
}

// a server workspace whose server can't be reached, waited on until the
// app has tried and failed (nothing listens on port 1)
fn server_down(h: &mut Ui1) {
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-down".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !matches!(
        h.app.sync_rt.net_status,
        Some(wb_net::Status::Reconnecting(_))
    ) && std::time::Instant::now() < until
    {
        // tests don't save, so the frame doesn't run the connection
        h.app.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.settle();
}

#[test]
fn the_top_bar_says_when_the_team_server_cannot_be_reached() {
    let mut h = Ui1::new();
    // working alone: nothing about a server
    assert!(!h.has("Server connection"));
    server_down(&mut h);
    assert!(h.has("Server connection"), "a word beside the profile menu");
    assert!(h
        .painted_text("Reconnecting\u{2026}")
        .iter()
        .any(|(r, _)| r.top() < 40.0));
    h.app.sync_rt.fails = super::together::FAILS_BEFORE_CANT_REACH;
    h.settle();
    assert!(h
        .painted_text("Not connected")
        .iter()
        .any(|(r, _)| r.top() < 40.0));
    // it sits left of the profile menu, inside the top bar
    let word = h.rect("Server connection");
    let profile = h.rect("Profile menu");
    assert!(word.right() <= profile.left() + 1.0 && word.bottom() <= 40.0);
    h.click("Server connection");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
    assert!(h.has("This workspace is on a server"));
    h.key(egui::Key::Escape);
    // signed out by the server: says so too
    h.app.sync_rt.net_status = Some(wb_net::Status::Refused(
        "Your access to this server was removed.".into(),
    ));
    h.settle();
    assert!(h
        .painted_text("Not signed in")
        .iter()
        .any(|(r, _)| r.top() < 40.0));
    // connected: out of the way
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.settle();
    assert!(!h.has("Server connection"));
}

#[test]
fn work_together_on_a_server_that_is_down_leads_with_working_offline_and_can_change_the_address() {
    let mut h = Ui1::new();
    super::together::save_token("127.0.0.1:1", "dev-down", "tok-down");
    server_down(&mut h);
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    // not "everyone sees it in a second or two" while nobody can
    assert!(!h.has("Tasks, chat and docs show up for everyone within a second or two."));
    assert!(h.has("You can keep working here. Your changes are kept on this computer and go up when the server is back."));
    // the server moved: type its new address and connect there
    h.click("Change address");
    h.click("Server address");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("127.0.0.1:2");
    h.click("Save and reconnect");
    let cfg = h.app.data.extra.shared.clone().unwrap();
    assert_eq!(cfg.server.as_deref(), Some("127.0.0.1:2"));
    assert!(
        h.app.sync_rt.net.is_none(),
        "connects again on the next frame"
    );
    assert_eq!(
        super::together::load_token("127.0.0.1:2", "dev-down").as_deref(),
        Some("tok-down"),
        "the sign-in goes along"
    );
    assert!(super::together::load_token("127.0.0.1:1", "dev-down").is_none());
    assert!(h.has("127.0.0.1:2") && !h.has("Save and reconnect"));
    // a wrong address says so and changes nothing
    h.click("Change address");
    h.click("Server address");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("https://team.example:web");
    h.click("Save and reconnect");
    assert_eq!(
        h.app.data.extra.shared.as_ref().unwrap().server.as_deref(),
        Some("127.0.0.1:2")
    );
    assert!(h.has("That port isn't a number."));
    h.click("Cancel");
    assert!(!h.has("Save and reconnect"));
    // connected: the usual words, and no address to change
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.settle();
    assert!(h.has("Tasks, chat and docs show up for everyone within a second or two."));
    assert!(!h.has("Change address"));
}

#[test]
fn people_says_why_roles_are_missing_offline_and_how_invited_people_join() {
    let mut h = Ui1::new();
    h.app.section = Section::People;
    h.settle();
    // nobody invited: no note about invited people
    let invited_note = |h: &Ui1| h.tree.iter().any(|(n, _)| n.starts_with("Invited people "));
    assert!(!invited_note(&h));
    h.app
        .invite_people(&["dana@example.com".to_string()], "Member");
    h.settle();
    assert!(h.has("Invited people can join once you share this workspace (Work together). Until then they can use this computer through Switch person in the profile menu."));
    assert!(h.has("Remove") && !h.has("Changing roles and removing people need the team's server. They come back once it's connected."));
    // on a server that can't be reached yet: the owner's controls are gone, and it says why
    server_down(&mut h);
    h.app.section = Section::People;
    h.settle();
    assert!(!h.has("Remove"));
    assert!(h.has("Changing roles and removing people need the team's server. They come back once it's connected."));
    assert!(h.has("Invited people sign in with the invite code the server emails them. Owners and admins also find the codes in Work together > People while connected."));
    // signed in as the owner: the controls are back, the note goes
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.settle();
    assert!(h.has("Remove"));
    assert!(!h.has("Changing roles and removing people need the team's server. They come back once it's connected."));
    // a shared folder: invited people join from the folder
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: "/tmp/wb-none".into(),
        device: "dev-f".into(),
        tag: 1,
        server: None,
    });
    h.settle();
    assert!(h.has("Invited people join from the shared folder: on their computer they open Work together, paste the folder's path, choose Join this workspace and pick their name."));
}

#[test]
fn an_invited_person_has_cancel_invite_instead_of_remove() {
    let mut h = Ui1::new();
    h.app.section = Section::People;
    h.settle();
    let before = h.app.data.members.len();
    h.app
        .invite_people(&["dana@example.com".to_string()], "Member");
    h.settle();
    let dana = h.app.data.members.iter().find(|m| m.invited).unwrap().id;
    assert!(h.has("Cancel invite"));
    h.click("Cancel invite");
    assert!(h.has(
        "Cancel the invite for Dana? Their invite code stops working and they leave the list."
    ));
    // the dialog's button (its title says the same, above it) takes them out
    let m = h.modal_rect();
    let at = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Cancel invite" && m.contains(r.center()))
        .map(|(_, r)| r.center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    h.click_at(at);
    assert!(h.app.modal.is_none());
    assert!(h.app.data.members.iter().all(|m| m.id != dana));
    assert_eq!(h.app.data.members.len(), before);
    assert!(!h.has("Cancel invite"));
    // people who joined keep the Remove button
    assert!(h.has("Remove"));
}

#[test]
fn a_member_on_the_server_is_told_who_manages_people() {
    let mut h = Ui1::new();
    h.app
        .invite_people(&["dana@example.com".to_string()], "Member");
    server_down(&mut h);
    h.app.section = Section::People;
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.settle();
    assert!(!h.has("Remove") && !h.has("Cancel invite"));
    assert!(h.has("Only the owner or an admin can change roles, cancel invites and remove people."));
    // the owner sees the buttons and no such note
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.settle();
    assert!(h.has("Remove") && h.has("Cancel invite"));
    assert!(
        !h.has("Only the owner or an admin can change roles, cancel invites and remove people.")
    );
}

#[test]
fn inviting_while_the_server_is_down_says_the_codes_wait_and_the_dialog_stays_narrow() {
    let window = |h: &Ui1| h.modal_rect();
    let bookmark = || Modal::Bookmark {
        title: String::new(),
        url: String::new(),
    };
    // a small dialog's width when nothing wide was open before
    let mut fresh = Ui1::new();
    fresh.app.modal = Some(bookmark());
    fresh.settle();
    let small = window(&fresh).width();

    let mut h = Ui1::new();
    server_down(&mut h);
    h.app.modal = Some(Modal::Invite {
        emails: String::new(),
        role: "Member".into(),
    });
    h.settle();
    // no "Work together > People" while that isn't there; the link names the place
    assert!(h.has("The team's server can't be reached right now. The invites are kept, and their invite codes are asked for once it's back."));
    assert!(h.has("Work together") && !h.has("Sharing"));
    let w = window(&h).width();
    assert!(
        w <= 640.0,
        "the note wraps instead of widening the dialog: {}",
        w
    );
    h.type_text("dana@example.com");
    h.click("Send invite");
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Invited 1 people. Their invite codes are asked for once the server is back.")
    );
    assert!(h
        .app
        .data
        .members
        .iter()
        .any(|m| m.email == "dana@example.com" && m.invited));
    // the next dialog opens at its own size
    h.app.modal = Some(bookmark());
    h.settle();
    assert!(
        (window(&h).width() - small).abs() < 2.0,
        "{} vs {}",
        window(&h).width(),
        small
    );
}

#[test]
fn help_says_where_the_workspace_is_kept() {
    let mut h = Ui1::new();
    h.app.modal = Some(Modal::Help);
    h.settle();
    assert!(
        h.has("This is your own copy of the work manager. Everything is saved on this computer.")
    );
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: "/tmp/wb-help".into(),
        device: "dev-help".into(),
        tag: 1,
        server: None,
    });
    h.settle();
    assert!(h.has("This workspace is shared through a folder your sync app keeps in step. Your copy is saved on this computer and in that folder."));
    h.app.data.extra.shared.as_mut().unwrap().server = Some("127.0.0.1:1".into());
    h.settle();
    assert!(h.has("This workspace is on your team's server. A copy is kept on this computer, so you can keep working while the server can't be reached."));
    assert!(
        !h.has("This is your own copy of the work manager. Everything is saved on this computer.")
    );
}

#[test]
fn calls_say_why_there_is_no_huddle_when_the_server_is_down_and_when_nobody_else_is_there() {
    let alone_note = "You're the only one in this conversation, so nobody else gets the Join button. Invite people first.";
    let down_note = "The team's server can't be reached, so a huddle in the app isn't available. This call opens in the browser instead, and the others see its Join button once you're connected again.";
    let mut h = Ui1::new();
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    h.app.section = Section::Chat;
    h.app.chat_open = Some(general);
    // a team in #general, no server: the browser call, no extra words
    h.app.open_call_dialog();
    h.settle();
    assert!(matches!(h.app.modal, Some(Modal::Call { .. })));
    assert!(!h.has(alone_note) && !h.has(down_note));
    h.key(egui::Key::Escape);
    // the server is set up but down: says why it's a browser call
    server_down(&mut h);
    h.app.open_call_dialog();
    h.settle();
    assert!(matches!(h.app.modal, Some(Modal::Call { .. })));
    assert!(h.has(down_note));
    h.key(egui::Key::Escape);
    // joining someone's huddle: not "huddles need a server" when there is one
    h.app.join_huddle(general);
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Huddles go through the team's server, and it can't be reached right now. Try again once it's connected.")
    );
    // alone in the conversation: nobody else gets the Join button
    let mut solo = Ui1::new();
    let me = solo.app.data.me;
    solo.app.data.members.retain(|m| m.id == me);
    for c in &mut solo.app.data.channels {
        c.members.retain(|id| *id == me);
    }
    solo.app.section = Section::Chat;
    solo.app.chat_open = Some(general);
    solo.app.open_call_dialog();
    solo.settle();
    assert!(solo.has(alone_note));
}

#[test]
fn share_says_when_nobody_else_is_in_the_workspace_yet() {
    let note = "Only you are in this Workspace so far. Invite people by email below to work on it together.";
    let mut h = Ui1::new();
    h.app.modal = Some(Modal::Share {
        emails: String::new(),
    });
    h.settle();
    assert!(!h.has(note), "a team has people to share with");
    let mut solo = Ui1::new();
    let me = solo.app.data.me;
    solo.app.data.members.retain(|m| m.id == me);
    solo.app.modal = Some(Modal::Share {
        emails: String::new(),
    });
    solo.settle();
    assert!(solo.has(note));
}

#[test]
fn huddle_bar_shows_people_mute_and_leave() {
    let mut h = Ui1::new();
    let ch = h.app.data.channels[0].id;
    let me = h.app.data.me;
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone();
    let my_name = h.app.data.member(me).unwrap().name.clone();
    // a huddle message in chat has a Join button
    let id = h.app.data.new_id();
    h.app.data.channels[0].messages.push(Message {
        edited: None,
        imported: None,
        mentions: vec![],
        attachments: vec![],
        id,
        author: other.id,
        text: "Started a huddle".into(),
        ts: now_ts(),
        reactions: vec![],
        replies: vec![],
        saved: false,
        task: None,
        post_title: None,
        post_type: String::new(),
        pinned: false,
        call: Some(format!("{}{}", huddle::HUDDLE_PREFIX, ch)),
    });
    h.app.section = Section::Chat;
    h.app.chat_open = Some(ch);
    h.settle();
    h.click("Join huddle");
    assert!(
        h.app.huddle.channel.is_none(),
        "not on a server: no in-app huddle"
    );
    assert!(h.app.toast.as_ref().is_some_and(|t| t.0.contains("server")));

    // in a huddle (on a server): the bar shows who is there and who is speaking
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.huddle.channel = Some(ch);
    h.app.huddle.joined = true;
    h.app.huddle.people = vec![
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
            identity: format!("{}-dev2", other.id),
            name: String::new(),
            me: false,
            speaking: true,
            muted: false,
            heard: 0.3,
            camera: true,
            screen: false,
        },
    ];
    h.settle();
    assert!(h.has(&format!("{} (you), listening", my_name)));
    assert!(h.has(&format!("{}, speaking", other.name)));
    assert!(h.has("2 people"));
    assert!(
        !h.has("Join huddle"),
        "no Join button for the huddle you're in"
    );
    h.click("Mute");
    assert!(h.app.huddle.muted);
    assert!(h.has("Unmute"));
    // video: the other person's picture shows with their name, the Video button toggles
    let pic = wb_voice::Picture {
        width: 32,
        height: 18,
        rgba: vec![200u8; 32 * 18 * 4],
    };
    h.app
        .huddle
        .pictures
        .insert(format!("{}-dev2", other.id), (pic.clone(), true));
    h.settle();
    assert!(h.has(&format!("Video of {}", other.name)));
    // clicking a video makes it large, clicking again makes it small
    let small = h.rect(&format!("Video of {}", other.name));
    h.click(&format!("Video of {}", other.name));
    assert_eq!(
        h.app.huddle.focus.as_deref(),
        Some(format!("{}-dev2", other.id).as_str())
    );
    let large = h.rect(&format!("Video of {}", other.name));
    assert!(
        large.width() > small.width() * 1.5,
        "{:?} -> {:?}",
        small,
        large
    );
    h.click(&format!("Video of {}", other.name));
    assert!(h.app.huddle.focus.is_none());
    h.click("Video");
    assert!(h.app.huddle.camera);
    h.app
        .huddle
        .pictures
        .insert(wb_voice::ME.to_string(), (pic, true));
    h.settle();
    assert!(h.has("Video of You") && h.has("Stop video"));
    h.click("Stop video");
    assert!(!h.app.huddle.camera);
    h.click("Leave");
    assert!(h.app.huddle.channel.is_none());
    assert!(!h.has("Unmute"));
}

#[test]
fn rail_has_no_labs_or_upgrade_and_empty_chat_can_start_a_channel() {
    let mut h = Ui1::new();
    assert!(!h.has("Rail Labs") && !h.has("Rail Upgrade") && !h.has("Rail Install"));
    assert!(h.has("Rail Invite"));
    h.app.data.channels.clear();
    h.app.section = Section::Chat;
    h.app.chat_open = None;
    h.settle();
    h.click("Create a Channel");
    assert_eq!(h.app.data.channels.len(), 1);
    assert_eq!(h.app.data.channels[0].name, "general");
    assert_eq!(h.app.chat_open, Some(h.app.data.channels[0].id));
}

#[test]
fn sidebar_add_channel_takes_enter_and_lets_go_on_escape() {
    let mut h = Ui1::new();
    let n = h.app.data.channels.len();
    h.click_side("Add Channel", false);
    assert!(h.app.new_channel.is_some());
    h.type_text("dev team");
    h.key(egui::Key::Enter);
    assert_eq!(
        h.app.data.channels.len(),
        n + 1,
        "Enter creates the channel"
    );
    assert!(h.app.data.channels.iter().any(|c| c.name == "dev-team"));
    assert!(h.app.new_channel.is_none());
    // Escape leaves the field and typing goes elsewhere again
    h.app.section = Section::Home;
    h.settle();
    h.click_side("Add Channel", false);
    assert!(h.app.new_channel.is_some());
    h.key(egui::Key::Escape);
    assert!(h.app.new_channel.is_none(), "Escape closes the field");
    assert!(
        h.ctx.memory(|m| m.focused().is_none()),
        "nothing keeps the keyboard"
    );
}

#[test]
fn new_message_window_closes_on_escape_and_when_moving_away() {
    let mut h = Ui1::new();
    h.app.new_message_open = true;
    h.settle();
    h.key(egui::Key::Escape);
    assert!(!h.app.new_message_open);
    h.app.new_message_open = true;
    h.settle();
    h.click("Inbox");
    assert!(!h.app.new_message_open, "going to another page closes it");
}

#[test]
fn create_list_opens_a_dialog_and_lands_in_a_visible_space() {
    let mut h = Ui1::new();
    h.click("My Tasks");
    h.click("Personal List");
    h.click("Create");
    h.click("List");
    assert!(
        matches!(h.app.modal, Some(Modal::NewList { .. })),
        "Create List dialog"
    );
    h.type_text("Roadmap");
    h.key(egui::Key::Enter);
    let Location::List(id) = h.app.loc else {
        panic!("opened the new list")
    };
    assert_eq!(h.app.data.list_name(id), "Roadmap");
    assert!(h
        .app
        .data
        .space_of_list(id)
        .unwrap()
        .private_owner
        .is_none());
}

#[test]
fn right_click_task_move_to_another_list() {
    let mut h = Ui1::new();
    h.right_click("Task Set up analytics dashboard", 60.0);
    h.click("Move to...");
    assert!(matches!(h.app.modal, Some(Modal::MoveTasks { .. })));
    h.click("Team Space / Projects / Marketing");
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Set up analytics dashboard")
        .unwrap();
    assert_eq!(h.app.data.list_name(t.list_id), "Marketing");
}

#[test]
fn tick_two_rows_and_archive_them_from_the_bar() {
    let mut h = Ui1::new();
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Select Write copy for landing page");
    h.click("Select Set up analytics dashboard");
    assert_eq!(h.app.selected.len(), 2);
    assert!(h.has("2 tasks selected"));
    h.click("Archive");
    let archived = h.app.data.tasks.iter().filter(|t| t.archived).count();
    assert_eq!(archived, 2);
    assert!(h.app.selected.is_empty());
}

#[test]
fn row_dots_menu_duplicates_a_task() {
    let mut h = Ui1::new();
    let n = h.app.data.tasks.len();
    h.hover(h.find("Task Set up analytics dashboard").unwrap());
    h.click("Task options Set up analytics dashboard");
    h.click("Duplicate");
    assert_eq!(h.app.data.tasks.len(), n + 1);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.name == "Set up analytics dashboard (copy)"));
}

#[test]
fn single_key_t_opens_create_task_and_cmd_backslash_hides_the_sidebar() {
    let mut h = Ui1::new();
    h.key(egui::Key::T);
    assert!(h.app.quick_add.is_some());
    h.app.quick_add = None;
    h.settle();
    let cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Backslash,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: cmd,
    }]);
    h.settle();
    assert!(h.app.sidebar_collapsed);
}

#[test]
fn list_dots_menu_moves_a_list() {
    let mut h = Ui1::new();
    h.hover(h.find("Marketing").unwrap());
    h.click("List options Marketing");
    h.click("Move...");
    assert!(matches!(h.app.modal, Some(Modal::MoveList { .. })));
    h.click_side("Side Projects", true);
    let id = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.folders
                .iter()
                .flat_map(|f| f.lists.iter())
                .chain(s.lists.iter())
        })
        .find(|l| l.name == "Marketing")
        .unwrap()
        .id;
    assert_eq!(h.app.data.space_of_list(id).unwrap().name, "Side Projects");
}

#[test]
fn settings_is_a_full_page_with_sections() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Settings");
    assert_eq!(h.app.section, Section::Settings);
    h.click("Settings Spaces");
    assert!(h.has("Team Space"));
    h.click("Settings Themes");
    h.click("Auto");
    assert!(h.app.data.extra.auto_theme);
    h.click("\u{2190}  Back to Workspace");
    assert_eq!(h.app.section, Section::Home);
}

#[test]
fn search_shows_recent_items_and_finds_docs() {
    let mut h = Ui1::new();
    h.click("Task Set up analytics dashboard");
    h.key(egui::Key::Escape);
    h.app.palette = Some(String::new());
    h.settle();
    assert!(h.has("Recent"));
    h.click("Docs");
    // (results are named by their title; the doc glyph is drawn beside it)
    h.click_side("Project brief", true);
    assert_eq!(h.app.section, Section::Docs);
}

#[test]
fn doc_slash_command_and_sub_page() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    // the empty body has no visible name to click; put the cursor in it
    h.ctx
        .memory_mut(|m| m.request_focus(egui::Id::new(("doc_body", id))));
    h.settle();
    h.type_text("/");
    h.settle();
    h.click("Heading 1");
    h.type_text("Plan");
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == id).unwrap().body,
        "# Plan"
    );
    h.click("+  Add page");
    assert!(h.app.data.docs.iter().any(|d| d.parent == Some(id)));
}

// ---------------------------------------------------------------- round 2 bugs

fn pri(h: &Ui1, name: &str) -> u8 {
    h.app
        .data
        .tasks
        .iter()
        .find(|t| t.name == name)
        .unwrap()
        .priority
}

#[test]
fn bulk_priority_opening_the_menu_changes_nothing() {
    let mut h = Ui1::new();
    let a = "Write copy for landing page";
    let b = "Set up analytics dashboard";
    for t in h
        .app
        .data
        .tasks
        .iter_mut()
        .filter(|t| t.name == a || t.name == b)
    {
        t.priority = 2;
    }
    h.hover(h.find(&format!("Task {}", a)).unwrap());
    h.click(&format!("Select {}", a));
    h.click(&format!("Select {}", b));
    assert!(h.has("2 tasks selected"));
    h.click("Priority");
    assert!(h.has("Clear"), "the priority menu is open");
    h.settle();
    assert_eq!(
        (pri(&h, a), pri(&h, b)),
        (2, 2),
        "just opening the menu keeps the priority"
    );
    h.click("Urgent");
    assert_eq!((pri(&h, a), pri(&h, b)), (1, 1));
    h.click("Priority");
    h.click("Clear");
    assert_eq!((pri(&h, a), pri(&h, b)), (0, 0), "Clear still clears");
}

impl Ui1 {
    fn rect(&self, name: &str) -> Rect {
        self.tree
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("nothing named {:?}", name))
    }
}

#[test]
fn dots_menus_open_submenus_beside_the_parent() {
    let mut h = Ui1::new();
    // sidebar space "..." > Create new
    h.hover(h.find("Team Space").unwrap());
    h.click("Space options Team Space");
    let parent = h.rect("Create new");
    h.hover(parent.center());
    let sub = h.rect("Folder");
    assert!(
        sub.left() >= parent.right() - 2.0,
        "Create new opens to the right: parent {:?} sub {:?}",
        parent,
        sub
    );
    assert!(h.has("Task statuses"), "the parent menu stays open");
    h.key(egui::Key::Escape);

    // list row "..." > Status
    h.hover(h.find("Task Set up analytics dashboard").unwrap());
    h.click("Task options Set up analytics dashboard");
    let parent = h.rect("Status");
    h.hover(parent.center());
    let sub = h.rect("IN PROGRESS");
    assert!(
        sub.left() >= parent.right() - 2.0,
        "Status opens to the right: parent {:?} sub {:?}",
        parent,
        sub
    );
    h.click("IN PROGRESS");
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Set up analytics dashboard")
        .unwrap();
    assert_eq!(t.status, "in progress", "picking from the submenu works");
}

#[test]
fn profile_mute_flyout_does_not_cover_the_menu() {
    let mut h = Ui1::new();
    h.click("Profile menu");
    let row = h.rect("Mute notifications");
    h.hover(row.center());
    let item = h.rect("For 1 hour");
    let settings = h.rect("Settings");
    assert!(
        item.right() <= settings.left() || item.left() >= settings.right(),
        "flyout {:?} sits beside the menu row {:?}",
        item,
        settings
    );
    // walk the pointer from the row to the flyout; it must stay open
    let to = item.center();
    for i in 1..=8 {
        let p = row.center() + (to - row.center()) * (i as f32 / 8.0);
        h.frame(vec![egui::Event::PointerMoved(p)]);
    }
    h.settle();
    h.click("For 1 hour");
    assert!(
        h.app.data.extra.muted_until > now_ts(),
        "muted from the flyout"
    );
}

#[test]
fn date_picker_fits_its_content_and_shows_set_recurring() {
    let mut h = Ui1::new();
    let mut rows: Vec<(String, Rect)> = h
        .tree
        .iter()
        .filter(|(n, _)| n.starts_with("Due date of "))
        .cloned()
        .collect();
    rows.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    assert!(rows.len() >= 2);
    for (name, _) in [rows.first().unwrap().clone(), rows.last().unwrap().clone()] {
        h.click(&name);
        let today = h.rect("Today");
        let weekly = h.rect("Weekly");
        assert!(
            weekly.bottom() <= 900.0 && weekly.top() >= 0.0,
            "{}: Set Recurring is on screen ({:?})",
            name,
            weekly
        );
        assert!(
            weekly.top() - today.top() < 380.0,
            "{}: the menu is only as tall as its content ({:?} .. {:?})",
            name,
            today,
            weekly
        );
        h.key(egui::Key::Escape);
    }
}

#[test]
fn escape_closes_the_task_menu_first_then_the_task() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    assert!(h.app.open_task.is_some());
    h.click("\u{2026}");
    assert!(h.has("Duplicate"), "the task window menu is open");
    h.key(egui::Key::Escape);
    assert!(!h.has("Duplicate"), "Escape closes the menu");
    assert!(h.app.open_task.is_some(), "and leaves the task open");
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none(), "a second Escape closes the task");

    // same for a row "..." menu with ticked rows: the selection stays
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Select Write copy for landing page");
    h.click("Task options Write copy for landing page");
    assert_eq!(h.app.selected.len(), 1);
    assert!(h.has("Add subtask"), "the row menu is open");
    h.key(egui::Key::Escape);
    assert_eq!(
        h.app.selected.len(),
        1,
        "Escape on the menu keeps the selection"
    );
}

#[test]
fn chat_drafts_stay_with_their_channel() {
    let mut h = Ui1::new();
    h.click("general");
    let a = h.app.chat_open.unwrap();
    h.click("Message composer");
    h.type_text("hello");
    // move to another channel, write and send there
    let b = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id != a)
        .expect("seed has two channels")
        .clone();
    h.click_side(&b.name, false);
    assert_eq!(h.app.chat_open, Some(b.id));
    assert_eq!(h.app.chat_text, "", "the other channel starts empty");
    h.click("Message composer");
    h.type_text("first message");
    h.key(egui::Key::Enter);
    let sent = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == b.id)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .text
        .clone();
    assert_eq!(sent, "first message");
    // back to the first channel: the unsent text is still there
    h.click_side("general", false);
    assert_eq!(h.app.chat_text, "hello");
}

#[test]
fn new_channel_field_comes_into_view_in_a_long_sidebar() {
    // (+ Create > Channel now opens the Create Channel window; the sidebar's
    // "Add Channel" row keeps the field at the end of the channel list)
    for via_create in [false] {
        let mut h = Ui1::new();
        // a long sidebar: many lists in the first space
        let base = h
            .app
            .data
            .spaces
            .iter()
            .flat_map(|s| s.lists.iter())
            .next()
            .cloned()
            .or_else(|| {
                h.app
                    .data
                    .spaces
                    .iter()
                    .flat_map(|s| s.folders.iter().flat_map(|f| f.lists.iter()))
                    .next()
                    .cloned()
            })
            .unwrap();
        for i in 0..30 {
            let mut l = base.clone();
            l.id = h.app.data.new_id();
            l.name = format!("Extra list {}", i);
            h.app.data.spaces[0].lists.push(l);
        }
        h.settle();
        if via_create {
            h.click("Create");
            h.click("Channel");
        } else {
            // the sidebar's own "Add Channel" row, scrolled to with the wheel
            h.click_side("Add Channel", false);
        }
        assert!(h.app.new_channel.is_some());
        for _ in 0..30 {
            h.frame(vec![]); // let the scroll animation finish
        }
        let r = h
            .ctx
            .read_response(egui::Id::new("new_channel_input"))
            .expect("the field is drawn")
            .rect;
        assert!(
            r.top() >= 40.0 && r.bottom() <= 880.0,
            "via_create={}: the field is on screen ({:?})",
            via_create,
            r
        );
        h.type_text("design");
        h.key(egui::Key::Enter);
        assert!(h.app.data.channels.iter().any(|c| c.name == "design"));
    }
}

#[test]
fn search_hides_empty_group_headings() {
    let mut h = Ui1::new();
    assert!(
        !h.app
            .data
            .tasks
            .iter()
            .any(|t| t.name.to_lowercase().contains("gen")),
        "seed check"
    );
    // distinct places on screen with this name
    let count = |h: &Ui1, n: &str| {
        let mut r: Vec<String> = h
            .tree
            .iter()
            .filter(|(x, _)| x == n)
            .map(|(_, r)| format!("{:?}", r))
            .collect();
        r.sort();
        r.dedup();
        r.len()
    };
    h.click("Search");
    h.type_text("a"); // matches tasks and locations
    let tasks_with = count(&h, "Tasks");
    assert!(h.has("Locations"));
    h.app.palette = Some(String::new());
    h.settle();
    h.type_text("gen"); // only the #general channel
    assert_eq!(h.app.palette.as_deref(), Some("gen"));
    assert!(h.has("Channels"));
    assert!(!h.has("Locations"), "no empty Locations heading");
    assert_eq!(
        count(&h, "Tasks"),
        tasks_with - 1,
        "no empty Tasks heading (the Tasks tab stays)"
    );
}

#[test]
fn my_docs_badge_counts_docs_not_sub_pages() {
    let mut h = Ui1::new();
    let before = h.app.my_docs_count();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    h.click("+  Add page");
    assert!(
        h.app.data.docs.iter().any(|d| d.parent == Some(id)),
        "a sub page was made"
    );
    assert_eq!(
        h.app.my_docs_count(),
        before + 1,
        "the sub page is not counted"
    );
}

#[test]
fn shortcut_symbols_have_glyphs() {
    if !std::path::Path::new("/System/Library/Fonts/SFNS.ttf").exists() {
        return; // the system font is only on macOS
    }
    let ctx = egui::Context::default();
    super::setup_fonts(&ctx);
    let _ = ctx.run(egui::RawInput::default(), |_| {});
    for c in ['\u{2325}', '\u{2318}', '\u{21E7}'] {
        for f in [FontId::monospace(13.0), FontId::proportional(13.0)] {
            assert!(
                ctx.fonts(|x| x.has_glyph(&f, c)),
                "{:?} has a glyph in {:?}",
                c,
                f
            );
        }
    }
}

// ---------------------------------------------------------------- round 2 parity

#[test]
fn create_task_window_has_description_status_and_tags() {
    let mut h = Ui1::new();
    h.click("Create");
    h.click("Task");
    assert!(h.has("Close"), "header with a close button");
    assert!(h.has("Task Name"));
    h.type_text("Full task");
    h.click("Add description");
    h.type_text("Some details");
    h.click("Task status");
    h.click("IN PROGRESS");
    h.click("Tags");
    h.type_text("urgent-fix");
    h.key(egui::Key::Enter);
    h.key(egui::Key::Escape);
    assert!(
        h.app.quick_add.is_some(),
        "Escape closes the tag popup first"
    );
    h.click_side("Create Task", true); // the button, right of the heading
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Full task")
        .expect("created");
    assert_eq!(t.description, "Some details");
    assert_eq!(t.status, "in progress");
    assert_eq!(t.tags, vec!["urgent-fix".to_string()]);
    assert!(h.app.quick_add.is_none());
}

#[test]
fn create_task_window_closes_from_the_header_x() {
    let mut h = Ui1::new();
    h.click("Create");
    h.click("Task");
    h.click("Close");
    assert!(h.app.quick_add.is_none());
}

#[test]
fn doc_edit_mode_draws_formatting_while_typing() {
    use super::docs::{doc_layout_job, line_kinds, LineKind};
    let text = "# Title\n- item\n- [x] done\n> quote\n**bold** text";
    let kinds: Vec<LineKind> = line_kinds(text).into_iter().map(|k| k.0).collect();
    assert_eq!(
        kinds,
        vec![
            LineKind::Heading(1),
            LineKind::Bullet,
            LineKind::Check(true),
            LineKind::Quote,
            LineKind::Plain
        ]
    );
    let t = theme::theme_accent(false, "");
    let job = doc_layout_job(text, &t, 600.0);
    assert_eq!(job.text, text, "the stored Markdown is laid out unchanged");
    let fmt_at = |needle: &str| {
        let b = job.text.find(needle).unwrap();
        job.sections
            .iter()
            .find(|s| s.byte_range.contains(&b))
            .unwrap()
            .format
            .clone()
    };
    assert_eq!(
        fmt_at("Title").font_id.size,
        28.0,
        "heading is big while editing"
    );
    assert!(fmt_at("quote").italics);
    assert!(
        fmt_at("done").strikethrough.width > 0.0,
        "a ticked item is struck through"
    );
    assert_eq!(
        fmt_at("- item").color,
        Color32::TRANSPARENT,
        "the bullet marker is replaced by a painted dot"
    );
}

#[test]
fn doc_checkbox_toggles_and_new_page_opens_in_edit() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    assert!(h.has("Start writing"), "an empty page offers how to start");
    h.click("Table");
    assert!(h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .body
        .starts_with("| Column 1"));
    let mut body = "- [ ] a\n- [x] b".to_string();
    super::docs::toggle_check_at(&mut body, 0);
    super::docs::toggle_check_at(&mut body, 8);
    assert_eq!(body, "- [x] a\n- [ ] b");
    // View on this page, then a new page: it opens ready to write
    h.click("View");
    assert!(h.app.doc_preview);
    h.click("+  Add page");
    assert!(!h.app.doc_preview, "a new page opens in Edit");
}

#[test]
fn doc_empty_page_add_subpage() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    h.click("Subpage");
    assert!(h.app.data.docs.iter().any(|d| d.parent == Some(id)));
    assert_ne!(h.app.doc_open, Some(id), "the sub page opens");
}

#[test]
fn new_message_window_searches_people_and_channels() {
    let mut h = Ui1::new();
    h.click("Create");
    h.click("Message");
    assert!(h.has("People or Channels"));
    assert!(h.has("Recents") && h.has("People") && h.has("Channels"));
    // Channels tab, search, pick: goes to the channel
    h.click("Channels");
    h.click("People or Channels");
    h.type_text("website");
    assert!(!h.has("# general"), "search narrows the list");
    h.click("# website-redesign");
    let ch = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat_open, Some(ch));
    assert!(!h.app.new_message_open);
    // reopened, the old search is gone; the avatar row opens a DM
    h.app.new_message_open = true;
    h.settle();
    assert!(h.has("# general"));
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != h.app.data.me)
        .unwrap()
        .clone();
    h.click(&format!("Message {}", other.name));
    let open = h.app.chat_open.unwrap();
    let c = h.app.data.channels.iter().find(|c| c.id == open).unwrap();
    assert_eq!(c.kind, ChannelKind::Direct);
    assert!(c.members.contains(&other.id));
}

#[test]
fn create_channel_window_with_private_and_list() {
    let mut h = Ui1::new();
    h.click("Create");
    h.click("Channel");
    assert!(
        matches!(h.app.modal, Some(Modal::NewChannel { .. })),
        "Create Channel window"
    );
    h.type_text("Design Team");
    h.click_side("Make Private", true); // the switch, right of its label
    h.click_side("Add a List", true);
    h.click_side("Create", true);
    let c = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "design-team")
        .expect("channel made")
        .clone();
    assert!(c.private);
    let lid = c.list.expect("a List tied to the channel");
    assert_eq!(h.app.data.list_name(lid), "Design Team");
    assert_eq!(h.app.chat_open, Some(c.id));
    assert!(h.app.modal.is_none());
    // the channel's List tab opens that List
    h.click("List");
    h.app.apply_actions();
    h.settle();
    assert_eq!(h.app.loc, Location::List(lid));
}

#[test]
fn create_channel_needs_a_name_and_old_files_still_load() {
    let mut h = Ui1::new();
    let n = h.app.data.channels.len();
    h.app.modal = Some(Modal::NewChannel {
        name: String::new(),
        private: false,
        add_list: false,
    });
    h.settle();
    h.click_side("Create", true);
    assert_eq!(h.app.data.channels.len(), n, "no channel without a name");
    let old = r#"{"id":1,"name":"x","kind":"Channel","topic":"","members":[],"messages":[]}"#;
    let c: Channel = serde_json::from_str(old).unwrap();
    assert!(!c.private && c.list.is_none());
}

#[test]
fn task_window_subtask_and_checklist_rows_open_their_fields() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.unwrap();
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .checklist
        .clear();
    h.settle();
    assert!(
        !h.has("Subtask name"),
        "no subtask field until the row is clicked"
    );
    h.click("+  Add subtask");
    h.type_text("Child one");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    h.settle();
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.parent == Some(id) && t.name == "Child one"));
    assert!(!h.has("Checklist item"));
    h.click("+  Create checklist");
    h.type_text("Step A");
    h.key(egui::Key::Enter);
    h.settle();
    assert_eq!(h.app.task(id).unwrap().checklist.len(), 1);
    assert!(h.app.open_task.is_some());
}

#[test]
fn task_window_comment_post_tabs_tools_and_search() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.unwrap();
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .activity
        .retain(|a| !a.comment);
    h.settle();
    // empty: the two shortcut buttons
    assert!(h.has("@  Mention a teammate") && h.has("Assign this task"));
    h.click("@  Mention a teammate");
    assert!(
        h.ctx.memory(|m| m.any_popup_open()),
        "the people list opens"
    );
    h.click("Alex Kim");
    assert_eq!(h.app.comment_text, "@Alex Kim ");
    h.type_text("please check");
    h.click("Emoji");
    h.click("\u{1F44D}");
    h.click("Send Comment");
    let t = h.app.task(id).unwrap().clone();
    let c = t.activity.iter().rev().find(|a| a.comment).unwrap();
    assert_eq!(c.text, "@Alex Kim please check\u{1F44D}");
    assert_eq!(c.mentions, vec![2], "the mention keeps Alex Kim's id");
    assert!(!c.post);
    // Post tab
    h.click("Post");
    h.click("Comment box");
    h.type_text("Release notes");
    h.click("Send Post");
    assert!(h
        .app
        .task(id)
        .unwrap()
        .activity
        .iter()
        .any(|a| a.post && a.text == "Release notes"));
    // search narrows the comments
    h.click("Search comments button");
    h.type_text("release");
    assert!(h.has("Release notes"));
    assert!(
        !h.has("@Alex Kim please check\u{1F44D}"),
        "the other comment is hidden"
    );
}

#[test]
fn task_window_assign_this_task_button() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.unwrap();
    {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.activity.retain(|a| !a.comment);
        t.assignees.clear();
    }
    h.settle();
    h.click("Assign this task");
    h.click_side("Sam Lee", true);
    h.settle();
    assert!(h.app.task(id).unwrap().assignees.contains(&3));
}

#[test]
fn fields_panel_searches_and_adds_property_columns() {
    let mut h = Ui1::new();
    assert!(!h.app.col_on("Created by"), "properties start hidden");
    h.click("Columns");
    assert!(h.app.fields_open);
    assert!(h.has("Shown") && h.has("Properties"));
    h.click("Search fields");
    h.type_text("created");
    assert!(h.has("Field Created by") && h.has("Field Date created"));
    assert!(!h.has("Field Tags"), "search narrows the list");
    h.click("Field Created by");
    assert!(h.app.col_on("Created by"));
    h.key(egui::Key::Escape);
    assert!(!h.app.fields_open, "Escape closes the panel");
    // the column is laid out, with a value ("Me" made the seed tasks)
    let cols = super::shell::col_layout(
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 30.0)),
        &h.app.hidden_cols,
        true,
    );
    assert_eq!(
        cols.extra.iter().map(|c| c.0).collect::<Vec<_>>(),
        vec!["Created by"]
    );
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Write copy for landing page")
        .unwrap();
    assert_eq!(
        h.app.prop_text(t, "Created by"),
        h.app.data.member(t.activity[0].author).unwrap().name
    );
}

#[test]
fn closing_a_task_records_date_done_and_closed() {
    let mut h = Ui1::new();
    let mut t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Write copy for landing page")
        .unwrap()
        .clone();
    assert!(t.closed_at.is_none());
    t.status = "complete".into();
    h.app.actions.push(Action::Replace(t.clone()));
    h.app.apply_actions();
    let after = h.app.task(t.id).unwrap().clone();
    assert!(after.closed_at.is_some() && after.done_at.is_some());
    assert!(!h.app.prop_text(&after, "Date closed").is_empty());
    t.status = "to do".into();
    h.app.actions.push(Action::Replace(t.clone()));
    h.app.apply_actions();
    assert!(
        h.app.task(t.id).unwrap().closed_at.is_none(),
        "reopening clears it"
    );
    // old data without the fields still loads
    let mut v = serde_json::to_value(h.app.task(t.id).unwrap()).unwrap();
    v.as_object_mut().unwrap().remove("closed_at");
    v.as_object_mut().unwrap().remove("done_at");
    let back: Task = serde_json::from_value(v).unwrap();
    assert!(back.closed_at.is_none());
}

#[test]
fn filter_builder_rows_are_joined_with_and() {
    let mut h = Ui1::new();
    h.app.loc = Location::Everything;
    h.settle();
    h.click("Filter");
    assert!(h.app.filter_open);
    h.click("+ Add filter");
    h.click("Filter 1 field");
    h.click("Priority");
    h.click("Filter 1 values");
    h.click("Urgent");
    h.key(egui::Key::Escape); // closes the drop-down, not the panel
    assert!(
        h.app.filter_open,
        "the panel stays while a drop-down closes"
    );
    assert_eq!(h.app.filter.rows.len(), 1);
    assert!(h.app.filtered().iter().all(|t| t.priority == 1));
    let urgent = h.app.filtered().len();
    // a second row: Status is not "to do"
    h.click("+ Add filter");
    h.click("Filter 2 field");
    h.click("Status");
    h.click("Filter 2 condition");
    h.click("is not");
    h.click("Filter 2 values");
    h.click("TO DO");
    h.key(egui::Key::Escape);
    assert_eq!(h.app.filter.count(), 2);
    assert!(h
        .app
        .filtered()
        .iter()
        .all(|t| t.priority == 1 && t.status != "to do"));
    assert!(h.app.filtered().len() <= urgent);
    h.click("Remove filter 2");
    assert_eq!(h.app.filter.rows.len(), 1);
    h.click("Clear all");
    assert_eq!(h.app.filter.count(), 0);
    h.key(egui::Key::Escape);
    assert!(!h.app.filter_open);
}

#[test]
fn filter_groups_with_their_own_or_and_the_badge_counts_them() {
    use parity::Join;
    let mut h = Ui1::new();
    h.app.loc = Location::Everything;
    h.settle();
    h.click("Filter");
    // Priority is Urgent
    h.click("+ Add filter");
    h.click("Filter 1 field");
    h.click("Priority");
    h.click("Filter 1 values");
    h.click("Urgent");
    h.key(egui::Key::Escape);
    // or a group: Status is TO DO or Priority is High
    h.click("+ Add group");
    assert_eq!(h.app.filter.groups.len(), 1);
    h.click("Group 1 filter 1 field");
    h.click("Status");
    h.click("Group 1 filter 1 values");
    h.click("TO DO");
    h.key(egui::Key::Escape);
    h.click("Add filter to group 1");
    h.click("Group 1 filter 2 field");
    h.click("Priority");
    h.click("Group 1 filter 2 values");
    h.click("High");
    h.key(egui::Key::Escape);
    assert!(h.app.filter_open, "the panel stays open");
    assert_eq!(h.app.filter.groups[0].rows.len(), 2);
    // And inside the group: TO DO and High
    let and_inside = h.app.filtered().len();
    h.click("Group 1 join");
    assert_eq!(h.app.filter.groups[0].join, Join::Or);
    h.click("Filter join");
    assert_eq!(h.app.filter.join, Join::Or);
    let shown: Vec<(u8, String)> = h
        .app
        .filtered()
        .iter()
        .map(|t| (t.priority, t.status.clone()))
        .collect();
    assert!(shown.len() > and_inside, "Or lets more through");
    assert!(shown
        .iter()
        .all(|(p, s)| *p == 1 || s == "to do" || *p == 2));
    assert!(shown.iter().any(|(p, _)| *p != 1), "not only Urgent");
    // the badge counts every condition, the ones in the group too
    assert_eq!(h.app.filter.count(), 3);
    assert!(h.has("Filter \u{00B7} 3"));
    // removing a row in the group, then the group
    h.click("Remove group 1 filter 2");
    assert_eq!(h.app.filter.count(), 2);
    h.click("Remove group 1");
    assert!(h.app.filter.groups.is_empty());
    assert_eq!(h.app.filter.count(), 1);
    assert!(h.has("Filter \u{00B7} 1"));
}

#[test]
fn save_a_filter_then_bring_it_back_rename_and_delete_it() {
    use parity::{FilterField, FilterGroup, FilterRow, Join};
    let mut h = Ui1::new();
    h.app.loc = Location::Everything;
    h.settle();
    h.click("Filter");
    // nothing to save yet
    h.click("Saved filter name");
    h.type_text("Urgent or to do");
    h.click("Save filter");
    assert!(h.app.data.extra.saved_filters.is_empty(), "no conditions");
    // Urgent, or (TO DO or High)
    let row = |field, v: &str| FilterRow {
        field: Some(field),
        not: false,
        values: vec![v.to_string()],
    };
    h.app.filter.rows = vec![row(FilterField::Priority, "1")];
    h.app.filter.join = Join::Or;
    h.app.filter.groups = vec![FilterGroup {
        join: Join::Or,
        rows: vec![
            row(FilterField::Status, "to do"),
            row(FilterField::Priority, "2"),
        ],
    }];
    h.settle();
    let shown = h.app.filtered().len();
    h.click("Save filter");
    assert_eq!(h.app.data.extra.saved_filters.len(), 1);
    let saved = h.app.data.extra.saved_filters[0].clone();
    assert_eq!(saved.name, "Urgent or to do");
    assert_eq!(saved.groups[0].rows.len(), 2);
    assert_eq!((saved.join, saved.groups[0].join), (Join::Or, Join::Or));
    assert!(h.has("Urgent or to do"));
    // clear, then bring it back by its name
    h.click("Clear all");
    assert_eq!(h.app.filter.count(), 0);
    h.click("Urgent or to do");
    assert_eq!(h.app.filter.count(), 3);
    assert_eq!(h.app.filter.groups, saved.groups);
    assert_eq!(h.app.filtered().len(), shown);
    assert!(h.has("Filter \u{00B7} 3"));
    // rename
    h.click("Rename saved filter Urgent or to do");
    h.type_text(" soon");
    h.click("Done renaming");
    assert_eq!(
        h.app.data.extra.saved_filters[0].name,
        "Urgent or to do soon"
    );
    assert!(h.has("Urgent or to do soon"));
    // changing the builder leaves the saved filter as it was
    h.click("Remove group 1");
    assert_eq!(h.app.filter.count(), 1);
    assert_eq!(h.app.data.extra.saved_filters[0].groups.len(), 1);
    // delete
    h.click("Delete saved filter Urgent or to do soon");
    assert!(h.app.data.extra.saved_filters.is_empty());
    assert!(!h.has("Urgent or to do soon"));
}

#[test]
fn space_overview_customize_and_lists_card_columns() {
    let mut h = Ui1::new();
    let sid = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.private_owner.is_none())
        .unwrap()
        .id;
    h.app.actions.push(Action::Go(Location::Space(sid)));
    h.app.apply_actions();
    h.app.view = ViewKind::Overview;
    h.settle();
    let lid = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.id == sid)
        .unwrap()
        .lists[0]
        .id;
    let lname = h.app.data.list_name(lid);
    assert!(h.has(&format!("Overview list {}", lname)));
    // Priority and Owner cells edit the List
    h.click(&format!("Priority of {}", lname));
    h.click("Urgent");
    assert_eq!(h.app.data.list_item(lid).unwrap().priority, 1);
    h.click(&format!("Owner of {}", lname));
    h.click_side("Sam Lee", true);
    assert_eq!(h.app.data.list_item(lid).unwrap().owner, Some(3));
    assert!(
        matches!(h.app.loc, Location::Space(_)),
        "editing a cell does not open the List"
    );
    // Customize turns the Docs card off; Add card brings it back
    h.click("Customize");
    h.click("Card Docs");
    assert!(h.app.data.extra.hidden_cards.contains(&"Docs".to_string()));
    h.key(egui::Key::Escape);
    assert!(!h.has("Add a Doc"));
    h.click("+ Add card");
    h.click("Add card Docs");
    assert!(h.app.data.extra.hidden_cards.is_empty());
    // New List opens the Create List window
    h.click("+ New List");
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })));
}

#[test]
fn personal_list_looks_like_the_original_and_my_tasks_stays_open() {
    let mut h = Ui1::new();
    // the three My Tasks items show without clicking My Tasks
    assert!(h.has("Assigned to me") && h.has("Today & Overdue") && h.has("Personal List"));
    assert!(h.app.group_by == GroupBy::Status);
    h.click("Personal List");
    let pl = h.app.personal_list_id().unwrap();
    assert_eq!(h.app.loc, Location::List(pl));
    assert!(h.app.group_by == GroupBy::None, "Group: None");
    assert!(h.app.col_on("Date created"));
    assert!(h.has("View Board") && h.has("View List"));
    assert!(
        !h.has("View Calendar") && !h.has("View Gantt") && !h.has("View Table"),
        "only Board and List"
    );
    assert!(
        !h.has("Add Channel")
            || h.tree
                .iter()
                .filter(|(n, _)| n == "Add Channel")
                .all(|(_, r)| r.left() < 320.0),
        "no Add Channel button on the page"
    );
    // empty: an inline Add Task row
    h.click("Add Task in ALL TASKS");
    h.type_text("Buy milk");
    h.key(egui::Key::Enter);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.list_id == pl && t.name == "Buy milk"));
    // leaving brings the earlier settings back
    h.click("Assigned to me");
    assert!(h.app.group_by == GroupBy::Status);
    assert!(!h.app.col_on("Date created"));
    assert!(h.has("Personal List"), "still open after moving away");
}

#[test]
fn list_plus_opens_a_create_menu() {
    let mut h = Ui1::new();
    h.click("Create in Marketing");
    assert!(
        h.app.quick_add.is_none(),
        "the + opens a menu, not the Create Task window"
    );
    for item in [
        "Create Task",
        "Create List",
        "Create Doc",
        "Create Dashboard",
        "Create Whiteboard",
        "Create Form",
    ] {
        assert!(h.has(item), "{} in the menu", item);
    }
    h.click("Create Task");
    assert!(h.app.quick_add.is_some());
    let mk = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .find(|l| l.name == "Marketing")
        .unwrap()
        .id;
    assert_eq!(
        h.app.quick_add.as_ref().unwrap().list,
        mk,
        "the task goes to that List"
    );
}

#[test]
fn list_menu_info_archive_and_restore_from_settings() {
    let mut h = Ui1::new();
    let mk = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .find(|l| l.name == "Marketing")
        .unwrap()
        .id;
    h.click("List options Marketing");
    h.click("List Info");
    assert!(matches!(h.app.modal, Some(Modal::ListInfo { .. })));
    h.click("List description");
    h.type_text("Campaign work");
    assert_eq!(
        h.app.data.list_item(mk).unwrap().description,
        "Campaign work"
    );
    h.key(egui::Key::Escape);
    // Create new is a side menu with the same items as "+"
    h.click("List options Marketing");
    h.click("Create new");
    assert!(h.has("Create Doc"));
    h.key(egui::Key::Escape);
    h.key(egui::Key::Escape);
    // Archive: gone from the sidebar and from All Tasks
    h.click("List options Marketing");
    h.click("Archive");
    assert!(h.app.data.list_item(mk).unwrap().archived);
    assert!(
        !h.has("List options Marketing"),
        "an archived List leaves the sidebar"
    );
    h.app.loc = Location::Everything;
    assert!(h.app.filtered().iter().all(|t| t.list_id != mk));
    // Settings > Spaces lists it with Restore
    h.app.section = Section::Settings;
    h.settle();
    h.click("Settings Spaces");
    h.click("Restore Team Space / Projects / Marketing");
    assert!(!h.app.data.list_item(mk).unwrap().archived);
}

#[test]
fn space_menu_duplicate_hide_archive_and_icon() {
    let mut h = Ui1::new();
    let sid = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.name == "Side Projects")
        .unwrap()
        .id;
    let n = h.app.data.spaces.len();
    // Color & Icon
    h.click("Space options Side Projects");
    h.click("Color & Icon");
    h.click("Space icon \u{2605}");
    assert_eq!(
        h.app.data.spaces.iter().find(|s| s.id == sid).unwrap().icon,
        "\u{2605}"
    );
    // Duplicate copies lists and tasks
    h.click("Space options Side Projects");
    h.click("Duplicate");
    h.app.apply_actions();
    assert_eq!(h.app.data.spaces.len(), n + 1);
    let copy = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.name == "Side Projects (copy)")
        .unwrap()
        .clone();
    let orig_lists = h.app.data.lists_in(Location::Space(sid)).unwrap().len();
    let copy_lists = h.app.data.lists_in(Location::Space(copy.id)).unwrap();
    assert_eq!(copy_lists.len(), orig_lists);
    assert!(
        h.app
            .data
            .tasks
            .iter()
            .any(|t| copy_lists.contains(&t.list_id)),
        "tasks come along"
    );
    h.settle();
    // Hide Space: off the sidebar, back from Settings
    h.click("Space options Side Projects (copy)");
    h.click("Hide Space");
    assert!(!h.has("Space options Side Projects (copy)"));
    h.app.section = Section::Settings;
    h.settle();
    h.click("Settings Spaces");
    h.click("Show Side Projects (copy)");
    assert!(
        !h.app
            .data
            .spaces
            .iter()
            .find(|s| s.id == copy.id)
            .unwrap()
            .hidden
    );
    // Archive, then Restore from Settings
    h.click("\u{2190}  Back to Workspace");
    h.click("Space options Side Projects (copy)");
    h.click("Archive");
    assert!(
        h.app
            .data
            .spaces
            .iter()
            .find(|s| s.id == copy.id)
            .unwrap()
            .archived
    );
    h.app.section = Section::Settings;
    h.settle();
    h.click("Settings Spaces");
    h.click("Restore Side Projects (copy)");
    assert!(
        !h.app
            .data
            .spaces
            .iter()
            .find(|s| s.id == copy.id)
            .unwrap()
            .archived
    );
}

#[test]
fn old_space_and_list_files_still_load() {
    let s: Space = serde_json::from_str(r#"{"id":1,"name":"S","color":[1,2,3],"statuses":[],"folders":[],"lists":[{"id":2,"name":"L"}]}"#).unwrap();
    assert!(!s.archived && !s.hidden && s.icon.is_empty());
    assert!(!s.lists[0].archived && s.lists[0].owner.is_none());
}

#[test]
fn escape_in_the_subtask_field_keeps_the_task_open() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    h.click("+  Add subtask");
    h.type_text("Child");
    h.key(egui::Key::Enter);
    h.key(egui::Key::Escape);
    assert!(
        h.app.open_task.is_some(),
        "Escape leaves the field, not the task"
    );
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none(), "a second Escape closes the task");
}

#[test]
fn property_columns_never_squeeze_the_name_away() {
    let mut hidden = HashSet::new();
    hidden.insert("Date created".to_string());
    let row = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 30.0));
    let cols = super::shell::col_layout(row, &hidden, true);
    assert!(
        cols.extra.len() < parity::PROPERTIES.len(),
        "not every column fits in 1100px"
    );
    assert!(
        cols.left >= 300.0,
        "the name keeps at least 300px ({})",
        cols.left
    );
}

// ---------------------------------------------------------------- round 3

#[test]
fn open_toolbar_search_does_not_steal_the_comment_box() {
    let mut h = Ui1::new();
    h.click("Search tasks");
    assert!(h.app.show_search);
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.expect("task window opens");
    h.click("Comment box");
    h.type_text("zzleak");
    h.settle();
    assert_eq!(
        h.app.comment_text, "zzleak",
        "typing lands in the comment box"
    );
    assert!(h.app.search.is_empty(), "the toolbar search stays empty");
    h.key(egui::Key::Escape);
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none() || h.app.open_task == Some(id));
    assert!(
        h.app.search.is_empty(),
        "the list is not filtered by the comment text"
    );
}

impl Ui1 {
    // the one control with this name inside a screen region
    fn rect_in(&self, name: &str, area: Rect) -> Rect {
        self.tree
            .iter()
            .rev()
            .find(|(n, r)| n == name && area.contains(r.center()))
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("nothing named {:?} in {:?}", name, area))
    }
}

#[test]
fn narrow_window_list_header_toolbar_and_columns_do_not_overlap() {
    let mut h = Ui1::sized(900.0, 560.0);
    let page = Rect::from_min_max(pos2(320.0, 0.0), pos2(900.0, 560.0));
    // columns: the name keeps its room, the property cells start after it
    let row = h.rect("Task Write copy for landing page");
    let due = h.rect("Due date of Write copy for landing page");
    let cols = super::shell::col_layout(row, &h.app.hidden_cols, true);
    assert!(
        cols.left - row.left() >= 220.0,
        "name column {} px wide",
        cols.left - row.left()
    );
    assert!(
        due.left() >= cols.left && cols.assignee.left() >= cols.left,
        "cells sit right of the name: due {:?}, left {}",
        due,
        cols.left
    );
    for r in [
        cols.assignee,
        cols.due,
        cols.priority,
        cols.status,
        cols.comments,
        cols.created,
    ] {
        assert!(
            r.width() == 0.0 || r.left() >= row.left() + 220.0,
            "a shown column {:?} covers the name",
            r
        );
    }
    // header: the breadcrumb (and its star) end before the buttons on the right
    let star = h.rect_in("\u{2605}", page).union(
        h.tree
            .iter()
            .find(|(n, _)| n == "\u{2606}")
            .map(|(_, r)| *r)
            .unwrap_or(Rect::NOTHING),
    );
    let crumb = h.rect_in("Website Redesign", page);
    let huddle = h.rect("Huddle");
    assert!(
        crumb.right() <= huddle.left() && star.right() <= huddle.left(),
        "crumb {:?} star {:?} huddle {:?}",
        crumb,
        star,
        huddle
    );
    // toolbar: left chips end before the right ones start, with search closed and open
    for open in [false, true] {
        if open {
            h.click("Search tasks");
        }
        let cols_chip = h.rect("Columns");
        let sort = h.rect("Sort: Manual");
        assert!(
            cols_chip.right() <= sort.left(),
            "search open {}: Columns {:?} Sort {:?}",
            open,
            cols_chip,
            sort
        );
        if open {
            let field = h.rect("Search tasks...");
            assert!(
                field.left() >= cols_chip.right() && field.right() <= h.rect("Add Task").left(),
                "the search field fits: {:?}",
                field
            );
        }
    }
    // the wide window still shows every label in full
    let w = Ui1::new();
    assert!(w.rect("Columns").width() > 60.0 && w.has("Team Space") && w.has("Projects"));
}

#[test]
fn switch_person_on_a_team_server_does_not_become_someone_else() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let my_name = h.app.data.member(me).unwrap().name.clone();
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .name
        .clone();
    let count = |h: &Ui1| h.tree.iter().filter(|(n, _)| *n == other).count();
    let before = count(&h);
    // on this computer only: pick anyone
    h.click("Profile menu");
    h.click("Switch person");
    assert!(count(&h) > before);
    h.key(egui::Key::Escape);
    // on a server the person is who signed in
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-switch".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.click("Profile menu");
    assert!(
        h.has("Trash") && !h.has("Switch person"),
        "not in the menu on a team server"
    );
    h.key(egui::Key::Escape);
    // the dialog, if it ever opens there, still switches no one
    h.app.modal = Some(Modal::SwitchUser);
    h.settle();
    assert!(h.has(&format!("This computer is signed in to the team's server as {}. Each person uses Workboard on their own computer with their own sign-in.", my_name)));
    assert_eq!(count(&h), before, "no other member to switch to");
    h.click("Work together");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
    assert_eq!(h.app.data.me, me);
}

#[test]
fn short_window_profile_menu_scrolls_to_trash_and_switch_person() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.click("Profile menu");
    h.settle();
    let menu_top = h.rect("Settings").top();
    // the menu does not run off the window: its bottom row is reached by
    // scrolling inside the menu
    for name in ["Trash", "Switch person"] {
        h.scroll_into_view(name);
        let r = h.rect(name);
        assert!(
            r.top() >= menu_top - 400.0 && r.bottom() <= 560.0,
            "{} is on screen after scrolling the menu: {:?}",
            name,
            r
        );
    }
    h.click("Switch person");
    assert!(
        matches!(h.app.modal, Some(Modal::SwitchUser)),
        "the bottom row of the menu works"
    );
    // in the tall window the menu still works as before
    h.key(egui::Key::Escape);
    let mut w = Ui1::new();
    w.click("Profile menu");
    w.click("Trash");
    assert!(matches!(w.app.modal, Some(Modal::Trash)));
}

#[test]
fn sidebar_search_filters_spaces_folders_and_lists() {
    let mut h = Ui1::new();
    let side = Rect::from_min_max(pos2(0.0, 0.0), pos2(320.0, 5000.0));
    let in_side = |h: &Ui1, name: &str| {
        h.tree
            .iter()
            .any(|(n, r)| n == name && side.contains(r.center()))
    };
    let spaces: Vec<Space> = h
        .app
        .data
        .spaces
        .iter()
        .filter(|s| s.private_owner.is_none() && !s.archived && !s.hidden)
        .cloned()
        .collect();
    assert!(spaces.len() >= 2);
    for s in &spaces {
        assert!(in_side(&h, &s.name), "{} shows before searching", s.name);
    }
    h.click("Search your sidebar");
    h.type_text("zzz");
    h.settle();
    for s in &spaces {
        assert!(
            !in_side(&h, &s.name),
            "no match: space {} is hidden",
            s.name
        );
        for f in &s.folders {
            assert!(
                !in_side(&h, &f.name),
                "no match: folder {} is hidden",
                f.name
            );
        }
        for l in &s.lists {
            assert!(!in_side(&h, &l.name), "no match: list {} is hidden", l.name);
        }
    }
    // part of one list's name: that list, its folder and space stay; other rows go
    let (space, folder, list) = spaces
        .iter()
        .find_map(|s| {
            s.folders
                .iter()
                .find_map(|f| f.lists.first().map(|l| (s.clone(), f.clone(), l.clone())))
        })
        .expect("a list in a folder");
    let q: String = list.name.to_lowercase().chars().take(5).collect();
    h.app.side_search = Some(q.clone());
    h.settle();
    assert!(
        in_side(&h, &space.name) && in_side(&h, &folder.name) && in_side(&h, &list.name),
        "{:?}: the list, its folder and space show",
        q
    );
    for s in spaces.iter().filter(|s| s.id != space.id) {
        let inside = s.name.to_lowercase().contains(&q)
            || s.folders.iter().any(|f| {
                f.name.to_lowercase().contains(&q)
                    || f.lists.iter().any(|l| l.name.to_lowercase().contains(&q))
            })
            || s.lists.iter().any(|l| l.name.to_lowercase().contains(&q))
            || h.app
                .data
                .docs
                .iter()
                .any(|d| d.space == Some(s.id) && d.title.to_lowercase().contains(&q));
        assert_eq!(
            in_side(&h, &s.name),
            inside,
            "space {} shows only with a match",
            s.name
        );
    }
    for l in space
        .lists
        .iter()
        .chain(folder.lists.iter())
        .filter(|l| !l.name.to_lowercase().contains(&q))
    {
        assert!(
            !in_side(&h, &l.name) || h.app.data.favorites.contains(&Location::List(l.id)),
            "list {} without a match is hidden",
            l.name
        );
    }
    // an empty query shows everything again
    h.app.side_search = Some(String::new());
    h.settle();
    for s in &spaces {
        assert!(in_side(&h, &s.name));
    }
}

#[test]
fn unread_filter_still_lists_unread_chats_while_muted() {
    let mut h = Ui1::new();
    let side = Rect::from_min_max(pos2(0.0, 0.0), pos2(320.0, 5000.0));
    let me = h.app.data.me;
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| {
            c.kind == ChannelKind::Direct
                && c.messages
                    .iter()
                    .any(|m| m.ts > c.last_read && m.author != me)
        })
        .expect("a DM with an unread message")
        .clone();
    let other = dm
        .members
        .iter()
        .find(|m| **m != me)
        .and_then(|m| h.app.data.member(*m))
        .unwrap()
        .name
        .clone();
    h.click("Profile menu");
    h.hover(h.rect("Mute notifications").center());
    h.click("For 4 hours");
    assert!(h.app.data.extra.muted_until > now_ts());
    h.click("Filter your sidebar");
    h.click("Sidebar filter Unread");
    assert_eq!(h.app.side_filter, Some("unread"));
    assert!(
        h.tree
            .iter()
            .any(|(n, r)| *n == other && side.contains(r.center())),
        "the unread DM with {} is listed while muted",
        other
    );
    // a DM without unread messages is not
    let read = h.app.data.channels.iter().find(|c| {
        c.kind == ChannelKind::Direct
            && !c
                .messages
                .iter()
                .any(|m| m.ts > c.last_read && m.author != me)
    });
    if let Some(c) = read {
        if let Some(n) = c
            .members
            .iter()
            .find(|m| **m != me)
            .and_then(|m| h.app.data.member(*m))
            .map(|m| m.name.clone())
        {
            assert!(
                !h.tree
                    .iter()
                    .any(|(x, r)| *x == n && side.contains(r.center())),
                "read DM {} is filtered out",
                n
            );
        }
    }
}

impl Ui1 {
    // every painted text with this exact content: (position, color)
    fn painted_text(&self, text: &str) -> Vec<(Rect, Color32)> {
        fn walk(shape: &egui::Shape, text: &str, out: &mut Vec<(Rect, Color32)>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, text, out)),
                egui::Shape::Text(ts) if ts.galley.text() == text => {
                    let c = ts
                        .galley
                        .job
                        .sections
                        .first()
                        .map(|s| s.format.color)
                        .unwrap_or(ts.fallback_color);
                    let c = if c == Color32::PLACEHOLDER {
                        ts.fallback_color
                    } else {
                        c
                    };
                    out.push((
                        ts.visual_bounding_rect(),
                        ts.override_text_color.unwrap_or(c),
                    ));
                }
                _ => {}
            }
        }
        let mut out = vec![];
        for s in &self.shapes {
            walk(&s.shape, text, &mut out);
        }
        out
    }

    fn circle_fill_at(&self, p: Pos2) -> Option<Color32> {
        self.shapes.iter().rev().find_map(|s| match &s.shape {
            egui::Shape::Circle(c) if c.center.distance(p) < 3.0 && c.radius > 5.0 => Some(c.fill),
            _ => None,
        })
    }
}

// WCAG contrast ratio of two colors
fn contrast(a: Color32, b: Color32) -> f32 {
    let lum = |c: Color32| {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn add_column_plus_and_reply_link_read_in_both_themes() {
    for dark in [false, true] {
        let mut h = Ui1::new();
        h.app.data.dark = dark;
        h.settle();
        let t = h.app.t();
        // the "+" at the right end of the list column header
        let plus = h
            .painted_text("+")
            .into_iter()
            .find(|(r, _)| h.circle_fill_at(r.center()).is_some())
            .unwrap_or_else(|| panic!("dark {}: no + on a circle", dark));
        let fill = h.circle_fill_at(plus.0.center()).unwrap();
        assert_ne!(
            plus.1, fill,
            "dark {}: + is not drawn in the circle's color",
            dark
        );
        assert!(
            contrast(plus.1, fill) >= 4.5,
            "dark {}: + on circle contrast {:.1}",
            dark,
            contrast(plus.1, fill)
        );
        // the "1 reply" link under a threaded message in #general
        h.click("general");
        let reply = h.painted_text("1 reply");
        assert!(!reply.is_empty(), "dark {}: a thread link is shown", dark);
        for (_, c) in reply {
            assert!(
                contrast(c, t.bg) >= 4.5,
                "dark {}: 1 reply contrast {:.1}",
                dark,
                contrast(c, t.bg)
            );
        }
    }
}

#[test]
fn narrow_task_window_fields_stay_left_of_the_comments() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.click_left("Task Write copy for landing page", 60.0);
    assert!(h.app.open_task.is_some());
    let panel = h.rect("Comments").left() - 12.0;
    for name in [
        "Time entries",
        "Time estimate value",
        "Track time",
        "Priority",
        "Assignees",
        "Time estimate",
    ] {
        let r = h.rect(name);
        assert!(
            r.right() <= panel,
            "{:?} {:?} runs into the comments column (from x {})",
            name,
            r,
            panel
        );
    }
    // the wide window keeps two fields per row
    let mut w = Ui1::new();
    w.click_left("Task Write copy for landing page", 120.0);
    let (st, asg) = (w.rect("Status"), w.rect("Assignees"));
    assert!(
        (st.center().y - asg.center().y).abs() < 4.0,
        "Status and Assignees share a row: {:?} {:?}",
        st,
        asg
    );
}

#[test]
fn sidebar_search_hides_the_favorites_heading_without_a_match() {
    let mut h = Ui1::new();
    let side = Rect::from_min_max(pos2(0.0, 0.0), pos2(320.0, 5000.0));
    let heading = |h: &Ui1| {
        h.painted_text("Favorites")
            .iter()
            .any(|(r, _)| side.contains(r.center()))
    };
    if h.app.data.favorites.is_empty() {
        h.app
            .data
            .favorites
            .push(Location::List(h.app.data.spaces[0].folders[0].lists[0].id));
        h.settle();
    }
    assert!(heading(&h), "Favorites shows before searching");
    let fav = h.app.data.location_name(h.app.data.favorites[0]);
    h.app.side_search = Some("zzz".into());
    h.settle();
    assert!(!heading(&h), "no favorite matches: no heading");
    h.app.side_search = Some(fav.to_lowercase());
    h.settle();
    assert!(heading(&h), "a matching favorite keeps the heading");
}

#[test]
fn task_names_use_the_room_up_to_the_columns() {
    let mut h = Ui1::sized(900.0, 560.0);
    let row = h.rect("Task Write copy for landing page");
    let cols = super::shell::col_layout(row, &h.app.hidden_cols, true);
    assert!(
        h.painted_text("Write copy for landing page")
            .iter()
            .any(|(r, _)| row.contains(r.center())),
        "the whole name fits at 900 px"
    );
    // a long name is cut with an ellipsis just before the first column
    let long = "Prepare the quarterly marketing report with charts for every channel and region";
    let id = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Write copy for landing page")
        .unwrap()
        .id;
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .name = long.into();
    h.settle();
    let row = h.rect(&format!("Task {}", long));
    let mut found = None;
    for s in &h.shapes {
        if let egui::Shape::Text(ts) = &s.shape {
            let txt = ts.galley.text();
            if txt.ends_with('\u{2026}')
                && long.starts_with(txt.trim_end_matches('\u{2026}'))
                && row.contains(ts.visual_bounding_rect().center())
            {
                found = Some(ts.visual_bounding_rect());
            }
        }
    }
    let r = found.expect("the long name is shown cut with an ellipsis");
    assert!(
        r.right() <= cols.left,
        "the cut name {:?} ends before the columns at {}",
        r,
        cols.left
    );
    assert!(
        r.right() >= cols.left - 60.0,
        "the cut name {:?} uses the room up to the columns at {}",
        r,
        cols.left
    );
}

#[test]
fn dark_theme_post_chip_and_ai_hint_read() {
    let mut h = Ui1::new();
    h.app.data.dark = true;
    h.settle();
    let t = h.app.t();
    let bgs = [t.bg, t.card, t.column_bg];
    let best = |c: Color32| bgs.iter().map(|b| contrast(c, *b)).fold(0.0f32, f32::max);
    // a post in the task activity carries a "Post" chip
    h.click_left("Task Write copy for landing page", 120.0);
    h.click("Post");
    h.click("Comment box");
    h.type_text("Shipped");
    h.click("Send Post");
    let chips: Vec<_> = h.painted_text("Post");
    assert!(!chips.is_empty());
    for (r, c) in chips {
        // the comment box's Post button is the theme colour, which is light
        // with dark text in the black theme: that label reads on its fill
        let on = if c == t.on_primary {
            contrast(c, t.primary)
        } else {
            best(c)
        };
        assert!(on >= 3.0, "Post at {:?} contrast {:.1}", r, on);
    }
    h.key(egui::Key::Escape);
    h.key(egui::Key::Escape);
    // the AI page hint
    h.click("Rail AI");
    let hint = h.painted_text("Ask anything... (available with the server)");
    assert!(!hint.is_empty(), "the hint is shown");
    for (_, c) in hint {
        assert!(best(c) >= 3.0, "AI hint contrast {:.1}", best(c));
    }
}

#[test]
fn narrow_task_window_has_no_blank_rows_when_fields_are_off() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.app.data.extra.apps.priorities = false;
    h.app.data.extra.apps.time_tracking = false;
    h.settle();
    h.click_left("Task Write copy for landing page", 60.0);
    let dates = h.rect("Dates");
    let est = h.rect("Time estimate");
    let tags = h.rect("Tags");
    let step = h.rect("Assignees").center().y - h.rect("Status").center().y;
    assert!(
        est.center().y - dates.center().y < step * 1.5,
        "no gap after Dates: {:?} {:?} (row step {})",
        dates,
        est,
        step
    );
    assert!(
        tags.center().y - est.center().y < step * 1.5,
        "no gap after Time estimate: {:?} {:?}",
        est,
        tags
    );
}

/// A picture file for attachment tests.
fn test_picture(name: &str, w: u32, h: u32) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wb-ui-pic-{:x}", crate::sync::random_u64()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut img = image::RgbaImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([(x * 255 / w) as u8, (y * 255 / h) as u8, 160, 255]);
    }
    let p = dir.join(name);
    img.save(&p).unwrap();
    p
}

#[test]
fn a_picture_can_be_sent_while_the_server_is_down_and_goes_up_later() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-ui-offline-pic".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    super::together::save_token("127.0.0.1:9", "dev-ui-offline-pic", "tok");
    h.click("general");
    let cid = h.app.chat_open.unwrap();
    h.dropped = vec![egui::DroppedFile {
        path: Some(test_picture("offline.png", 60, 40)),
        ..Default::default()
    }];
    h.frame(vec![]);
    // nothing listens on port 9: the upload fails, but the message may go
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !matches!(
        h.app
            .chat_files
            .first()
            .and_then(|p| p.transfer(&h.app.files)),
        Some(super::files::Transfer::Failed(_))
    ) && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let Some(super::files::Transfer::Failed(why)) = h.app.chat_files[0].transfer(&h.app.files)
    else {
        panic!("the upload failed");
    };
    // said without the system's English words
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let shown = App::error_text(&why);
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert!(
        !shown.contains("os error") && !shown.contains("refused"),
        "{}",
        shown
    );
    let (ok, files) = h.app.pending_ready(false);
    assert!(ok && files.len() == 1, "the message can go");
    h.click("Message composer");
    h.type_text("from the train");
    h.key(egui::Key::Enter);
    let m = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone();
    super::together::forget_token("127.0.0.1:9", "dev-ui-offline-pic");
    assert_eq!(m.text, "from the train");
    assert_eq!(m.attachments.len(), 1);
    assert!(
        h.app.files.is_waiting(&m.attachments[0].file),
        "the picture waits to go up, also after a restart"
    );
}

#[test]
fn drop_a_picture_send_it_and_open_it_large() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat_open.unwrap();
    let n = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    // dragging over the window says where to drop
    let pic = test_picture("beach.png", 600, 400);
    h.frame(vec![]);
    // dropped from Finder: it waits in the message box until it's ready
    h.dropped = vec![egui::DroppedFile {
        path: Some(pic.clone()),
        ..Default::default()
    }];
    h.frame(vec![]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !(h.app.pending_ready(false).0 && !h.app.pending_ready(false).1.is_empty())
        && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.settle();
    assert!(h.has("Attachment beach.png"), "a tile in the message box");
    // a picture alone can be sent: Enter in the empty box
    h.click("Message composer");
    h.key(egui::Key::Enter);
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert_eq!(ch.messages.len(), n + 1, "sent");
    let m = ch.messages.last().unwrap().clone();
    assert_eq!(m.text, "");
    assert_eq!(m.attachments.len(), 1);
    assert_eq!(
        (
            m.attachments[0].w,
            m.attachments[0].h,
            m.attachments[0].kind
        ),
        (600, 400, MediaKind::Image)
    );
    assert!(h.app.chat_files.is_empty(), "the box is empty again");
    assert!(!h.has("Attachment beach.png"));
    // it shows in the conversation; clicking opens it large, Esc closes
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h
        .app
        .files
        .texture(&m.attachments[0].file, super::files::THUMB)
        .is_none()
        && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        h.app
            .files
            .texture(&m.attachments[0].file, super::files::THUMB)
            .is_some(),
        "the picture was decoded for the screen"
    );
    h.click("Picture beach.png");
    assert!(h.app.viewer.is_some());
    assert!(h.has("Large beach.png") && h.has("Show in Finder") && h.has("Save to Downloads"));
    h.key(egui::Key::Escape);
    assert!(h.app.viewer.is_none());
    // a file that isn't a picture or video is refused in the box
    let txt = pic.with_file_name("notes.txt");
    std::fs::write(&txt, "hi").unwrap();
    h.dropped = vec![egui::DroppedFile {
        path: Some(txt),
        ..Default::default()
    }];
    h.frame(vec![]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.app.chat_files.first().is_some_and(|p| p.error.is_none())
        && std::time::Instant::now() < until
    {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(h.app.chat_files[0]
        .error
        .as_deref()
        .unwrap_or("")
        .contains("isn't a picture or a video"));
    assert!(
        !h.app.pending_ready(false).0,
        "can't send with a refused file"
    );
    h.click("Remove notes.txt");
    assert!(h.app.chat_files.is_empty());
}

// send a message in #general and show its hover toolbar
fn general_with_my_message(text: &str) -> (Ui1, u64) {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat_open.unwrap();
    h.click("Message composer");
    h.type_text(text);
    h.key(egui::Key::Enter);
    let p = h.find(text).unwrap();
    h.hover(p);
    (h, cid)
}

fn general_msg<'a>(h: &'a Ui1, cid: u64, start: &str) -> Option<&'a Message> {
    h.app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .iter()
        .find(|m| m.text.starts_with(start))
}

#[test]
fn click_more_actions_edit_my_message() {
    let (mut h, cid) = general_with_my_message("first draft");
    h.click("More actions");
    h.click("Edit message");
    assert!(
        h.app.chat_edit.is_some(),
        "the message turns into a text box"
    );
    h.click("Edit message");
    for _ in 0.."first draft".len() {
        h.key(egui::Key::Backspace);
    }
    h.type_text("final words");
    h.key(egui::Key::Enter);
    assert!(h.app.chat_edit.is_none());
    let m = general_msg(&h, cid, "final words").expect("text changed");
    assert!(m.edited.is_some(), "marked as edited");
    assert!(h.has("(edited)"));
}

#[test]
fn escape_cancels_an_edit_and_up_arrow_starts_one() {
    let (mut h, cid) = general_with_my_message("keep me");
    h.click("Message composer");
    h.key(egui::Key::ArrowUp);
    let mid = general_msg(&h, cid, "keep me").unwrap().id;
    assert_eq!(
        h.app.chat_edit.as_ref().map(|e| e.1),
        Some(mid),
        "Up in an empty box edits my last message"
    );
    h.click("Edit message");
    h.type_text(" changed");
    h.key(egui::Key::Escape);
    assert!(h.app.chat_edit.is_none());
    let m = general_msg(&h, cid, "keep me").unwrap();
    assert_eq!(m.text, "keep me", "Escape leaves the text as it was");
    assert!(m.edited.is_none());
}

#[test]
fn click_more_actions_delete_needs_two_clicks() {
    let (mut h, cid) = general_with_my_message("oops wrong channel");
    h.click("More actions");
    h.click("Delete message");
    assert!(
        general_msg(&h, cid, "oops").is_some(),
        "one click only asks"
    );
    h.click("Click again to delete");
    assert!(
        general_msg(&h, cid, "oops").is_none(),
        "the second click deletes"
    );
}

#[test]
fn others_messages_cannot_be_edited_or_deleted() {
    let mut h = Ui1::new();
    h.click("general");
    let p = h
        .find("Reminder: newsletter draft is due Thursday.")
        .unwrap();
    h.hover(p);
    h.click("More actions");
    assert!(h.has("Copy text") && h.has("Mark unread"));
    assert!(!h.has("Edit message") && !h.has("Delete message"));
}

#[test]
fn click_more_reactions_picks_any_emoji() {
    let (mut h, cid) = general_with_my_message("ship it");
    h.click("More reactions");
    h.click("\u{1F680}");
    let m = general_msg(&h, cid, "ship it").unwrap();
    assert!(m
        .reactions
        .iter()
        .any(|r| r.emoji == "\u{1F680}" && r.users == vec![h.app.data.me]));
}

#[test]
fn click_mark_unread_keeps_the_channel_unread() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat_open.unwrap();
    let text = "Reminder: newsletter draft is due Thursday.";
    let p = h.find(text).unwrap();
    h.hover(p);
    h.click("More actions");
    h.click("Mark unread");
    h.settle();
    let ts = general_msg(&h, cid, "Reminder:").unwrap().ts;
    let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert!(
        c.last_read < ts,
        "still unread while the channel stays open"
    );
    // leaving and coming back reads it
    h.click("website-redesign");
    h.click("general");
    let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert!(c.last_read >= ts);
}

#[test]
fn click_remind_me_about_a_message() {
    let mut h = Ui1::new();
    h.click("general");
    let p = h
        .find("Reminder: newsletter draft is due Thursday.")
        .unwrap();
    h.hover(p);
    h.click("More actions");
    h.click("Remind me in 1 hour");
    let r = h
        .app
        .data
        .extra
        .reminders
        .last()
        .expect("a reminder is made");
    assert!(
        r.text
            .starts_with("Message from Sam Lee: Reminder: newsletter"),
        "{}",
        r.text
    );
    assert!((r.due_ts - now_ts() - 3600).abs() < 10);
}

#[test]
fn composer_mention_and_emoji_pickers() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Mention someone");
    h.click("Alex Kim");
    assert_eq!(h.app.chat_text, "@Alex Kim ");
    h.click("Emoji");
    h.click("\u{1F525}");
    assert_eq!(h.app.chat_text, "@Alex Kim \u{1F525}");
    // typing "@" opens the list by itself and the pick replaces the "@"
    h.type_text(" @");
    assert!(h.has("Jordan Park"), "typing @ offers people");
    // (the right-most "Jordan Park" is the list's row, not a name in the feed)
    h.click_side("Jordan Park", true);
    assert_eq!(h.app.chat_text, "@Alex Kim \u{1F525} @Jordan Park ");
}

// ---- task comments and activity (sf-comments)

impl Ui1 {
    fn cmd_enter(&mut self) {
        let m = egui::Modifiers::COMMAND;
        self.frame(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: m,
        }]);
        self.frame(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: m,
        }]);
        self.settle();
    }

    // open "Write copy for landing page" with no comments; returns its id
    fn open_clean_task(&mut self) -> u64 {
        self.click_left("Task Write copy for landing page", 120.0);
        let id = self.app.open_task.unwrap();
        self.app
            .data
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .activity
            .retain(|a| !a.comment);
        self.settle();
        id
    }

    fn comments(&self, id: u64) -> Vec<Activity> {
        self.app
            .task(id)
            .unwrap()
            .activity
            .iter()
            .filter(|a| a.comment)
            .cloned()
            .collect()
    }
}

#[test]
fn comment_send_with_cmd_enter_then_react_edit_reply_delete() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    h.click("Comment box");
    h.type_text("First thoughts");
    h.cmd_enter();
    let c = h.comments(id);
    assert_eq!(c.len(), 1, "Cmd+Enter sends");
    assert_eq!(c[0].text, "First thoughts");
    assert_ne!(c[0].id, 0);
    assert!(h.app.comment_text.is_empty());
    // react
    h.click("Add reaction");
    h.click("\u{1F44D}");
    assert_eq!(h.comments(id)[0].reactions[0].users, vec![1]);
    // the reaction chip takes it back
    h.click("\u{1F44D} 1");
    assert!(h.comments(id)[0].reactions.is_empty());
    // edit my comment
    h.click("Comment actions");
    h.click("Edit");
    h.type_text(" (more)");
    h.click("Save comment");
    let c = h.comments(id);
    assert_eq!(c[0].text, "First thoughts (more)");
    assert!(c[0].edited.is_some());
    assert!(h.has("(edited)"));
    // reply
    h.click("Comment actions");
    h.click("Reply");
    h.type_text("A reply");
    h.click("Send reply");
    let c = h.comments(id);
    assert_eq!(c.len(), 2);
    assert_eq!(c[1].parent, Some(c[0].id));
    assert!(h.has("A reply"));
    // deleting the comment takes its replies with it
    h.click_side("Comment actions", false);
    h.click("Delete comment");
    assert!(h.comments(id).is_empty());
}

#[test]
fn comment_assign_and_resolve_shows_in_assigned_comments() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    h.click("Comment box");
    h.type_text("Please review");
    h.click("Send Comment");
    h.click("Comment actions");
    h.click("Assign comment");
    h.click("Me");
    let c = h.comments(id);
    assert_eq!(c[0].assignee, Some(1));
    assert!(h.has("Assigned to Me"));
    // it is on the Assigned Comments page until resolved there
    h.app.open_task = None;
    h.app.loc = Location::Assigned;
    h.settle();
    assert!(h.has("Resolve comment on Write copy for landing page"));
    h.click("Resolve comment on Write copy for landing page");
    assert!(h.comments(id)[0].resolved);
    assert!(h.app.open_task.is_none(), "ticking does not open the task");
    assert!(
        !h.has("Resolve comment on Write copy for landing page"),
        "resolved comments drop out"
    );
    // and it can be reopened from the task
    h.app.apply(Action::Open(id));
    h.settle();
    h.click("Reopen comment");
    assert!(!h.comments(id)[0].resolved);
}

#[test]
fn comment_checklist_ticks_and_links_open() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    let other = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.id != id && t.parent.is_none())
        .unwrap()
        .id;
    let link = format!("workboard://task/{}", other);
    {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.activity.push(Activity {
            ts: now_ts(),
            author: 2,
            text: format!("To do:\n\u{2610} buy milk\nsee {}", link),
            comment: true,
            id: 77_001,
            ..Default::default()
        });
    }
    h.settle();
    h.click("Tick buy milk");
    assert!(h.comments(id)[0].text.contains("\u{2611} buy milk"));
    assert!(h.comments(id)[0].edited.is_none(), "ticking is no edit");
    h.click(&link);
    assert_eq!(h.app.open_task, Some(other));
}

#[test]
fn typing_at_opens_the_people_list() {
    let mut h = Ui1::new();
    h.open_clean_task();
    h.click("Comment box");
    h.type_text("hey @");
    h.settle();
    assert!(
        h.ctx.memory(|m| m.any_popup_open()),
        "the people list opens"
    );
    let alex = h.tree.iter().filter(|(n, _)| n == "Alex Kim").count();
    h.type_text("Sa");
    h.settle();
    assert!(
        h.ctx.memory(|m| m.any_popup_open()),
        "still open while typing a name"
    );
    let alex_after = h.tree.iter().filter(|(n, _)| n == "Alex Kim").count();
    assert!(
        h.has("Sam Lee") && alex_after < alex,
        "narrowed by what is typed"
    );
    // (the sidebar has a "Sam Lee" too, below the window)
    let pick = h
        .tree
        .iter()
        .find(|(n, r)| n == "Sam Lee" && r.center().y < 900.0)
        .map(|(_, r)| r.center())
        .unwrap();
    h.click_at(pick);
    assert_eq!(h.app.comment_text, "hey @Sam Lee ");
}

#[test]
fn mentions_stand_out_and_comments_only_hides_activity() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.activity.push(Activity {
            ts: now_ts(),
            author: 1,
            text: "ping @Alex Kim now".into(),
            comment: true,
            id: 77_002,
            ..Default::default()
        });
    }
    h.settle();
    // the mention is its own piece of text
    assert!(h.has("@Alex Kim"));
    let line = h
        .app
        .task(id)
        .unwrap()
        .activity
        .iter()
        .find(|a| !a.comment)
        .map(|a| a.text.clone())
        .unwrap();
    let shown = format!("\u{2022} Me {}", line);
    assert!(h.has(&shown), "activity lines show by default");
    h.click("Comments only");
    assert!(!h.has(&shown), "hidden by Comments only");
    assert!(h.has("@Alex Kim"));
}

// ---------------------------------------------------------------- docs slice (A1-A16)

impl Ui1 {
    fn key_mods(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }]);
        self.settle();
    }

    /// A fresh doc open in Edit with the cursor in its body.
    fn new_doc_focused(&mut self) -> u64 {
        self.app.new_doc(None);
        self.settle();
        let id = self.app.doc_open.unwrap();
        self.ctx
            .memory_mut(|m| m.request_focus(egui::Id::new(("doc_body", id))));
        self.settle();
        id
    }

    fn doc_body(&self, id: u64) -> String {
        self.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .body
            .clone()
    }

    fn open_docs_list(&mut self) {
        self.app.section = Section::Docs;
        self.app.doc_open = None;
        self.app.hub_filter = 0;
        self.settle();
    }
}

#[test]
fn docs_pages_panel_menu_duplicates_reorders_moves_and_deletes() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let root = h.app.doc_open.unwrap();
    let a = h.app.add_page(root);
    let b = h.app.add_page(root);
    h.app.doc_open = Some(root);
    h.settle();
    // duplicate "Page 1": a copy sits under the same parent
    h.right_click("Page Page 1", 40.0);
    h.click("Duplicate");
    let copy = h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.title == "Page 1 (copy)")
        .expect("a copy is made");
    assert_eq!(copy.parent, Some(root));
    // move "Page 2" up past the copy and "Page 1"
    let order = |h: &Ui1| {
        h.app
            .data
            .docs
            .iter()
            .filter(|d| d.parent == Some(root))
            .map(|d| d.id)
            .collect::<Vec<_>>()
    };
    let before = order(&h);
    h.right_click("Page Page 2", 40.0);
    h.click("Move up");
    let after = order(&h);
    assert_ne!(before, after);
    assert_eq!(
        after.iter().position(|x| *x == b).unwrap() + 1,
        before.iter().position(|x| *x == b).unwrap()
    );
    // move "Page 2" under "Page 1"
    h.right_click("Page Page 2", 40.0);
    let mt = h.rect("Move to");
    h.hover(mt.center());
    h.click_side("Page 1", true);
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == b).unwrap().parent,
        Some(a)
    );
    // deleting "Page 1" takes its sub page with it, as one Trash item
    let trash = h.app.data.extra.trash.len();
    h.right_click("Page Page 1", 40.0);
    h.click("Delete");
    assert!(!h.app.data.docs.iter().any(|d| d.id == a || d.id == b));
    assert_eq!(h.app.data.extra.trash.len(), trash + 1);
}

#[test]
fn docs_delete_keeps_sub_pages_and_restore_brings_them_back() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let root = h.app.doc_open.unwrap();
    let a = h.app.add_page(root);
    let aa = h.app.add_page(a);
    h.app.doc_open = Some(root);
    h.settle();
    h.click("Delete");
    assert!(
        !h.app
            .data
            .docs
            .iter()
            .any(|d| [root, a, aa].contains(&d.id)),
        "the whole doc is gone"
    );
    let item = h.app.data.extra.trash.last().unwrap();
    assert_eq!(item.kind, "Doc");
    let i = h.app.data.extra.trash.len() - 1;
    h.app.restore_trash(i);
    let parent = |h: &Ui1, id: u64| {
        h.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.parent)
    };
    assert_eq!(parent(&h, root), Some(None));
    assert_eq!(
        parent(&h, a),
        Some(Some(root)),
        "sub pages come back under their doc"
    );
    assert_eq!(parent(&h, aa), Some(Some(a)));
    // an old single-page Trash item still restores
    let old = Doc {
        id: 999_001,
        space: None,
        owner: 1,
        private: false,
        archived: false,
        meeting_note: false,
        wiki: None,
        views: vec![],
        parent: Some(424242),
        title: "Old".into(),
        body: String::new(),
        created: 0,
        updated: 0,
        tags: vec![],
    };
    h.app.data.extra.trash.push(TrashItem {
        id: 999_002,
        kind: "Doc".into(),
        title: "Old".into(),
        deleted: 0,
        payload: serde_json::to_string(&old).unwrap(),
    });
    let i = h.app.data.extra.trash.len() - 1;
    h.app.restore_trash(i);
    assert_eq!(
        parent(&h, 999_001),
        Some(None),
        "a page whose parent is gone becomes a doc"
    );
}

#[test]
fn docs_header_copy_link_duplicate_move_and_meeting_note() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let id = h.app.doc_open.unwrap();
    h.app.add_page(id);
    h.app.doc_open = Some(id);
    h.settle();
    // move to another Space from the location
    h.click("Doc location");
    h.click("Side Projects");
    let side = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.name == "Side Projects")
        .unwrap()
        .id;
    assert!(
        h.app
            .data
            .docs
            .iter()
            .filter(|d| d.id == id || d.parent == Some(id))
            .all(|d| d.space == Some(side)),
        "all pages move"
    );
    // mark as a meeting note by hand
    h.click("Doc options");
    h.click("Mark as Meeting Note");
    assert!(
        h.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .meeting_note
    );
    h.click("Copy link");
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Link copied")
    );
    let n = h.app.data.docs.len();
    h.click("Duplicate");
    assert_eq!(
        h.app.data.docs.len(),
        n + 2,
        "the copy has its sub page too"
    );
    let copy = h.app.doc_open.unwrap();
    assert_ne!(copy, id);
    assert!(h.app.data.docs.iter().any(|d| d.parent == Some(copy)));
}

#[test]
fn docs_list_right_click_search_and_tags() {
    let mut h = Ui1::new();
    h.app.search = "keep".into();
    h.open_docs_list();
    h.app.search.clear();
    h.settle();
    // the Docs search box is its own, not the task list's filter
    h.click("Search Docs");
    h.type_text("zzz");
    assert_eq!(h.app.docs_search, "zzz");
    assert!(h.app.search.is_empty());
    assert!(!h.has("Doc Project brief"));
    h.app.docs_search.clear();
    h.settle();
    // right click: favorite, meeting note, rename
    h.right_click("Doc Project brief", 60.0);
    h.click("Add to Favorites");
    let id = h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.title == "Project brief")
        .unwrap()
        .id;
    assert!(h.app.data.extra.starred.contains(&format!("doc:{}", id)));
    h.right_click("Doc Project brief", 60.0);
    h.click("Mark as Meeting Note");
    assert!(
        h.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .meeting_note
    );
    h.right_click("Doc Project brief", 60.0);
    h.click("Rename doc");
    // the field starts with the whole name selected
    h.type_text("Project brief v2");
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == id).unwrap().title,
        "Project brief v2"
    );
    h.key(egui::Key::Escape);
    // tags: add one from the doc, then filter the list by it
    h.app.doc_open = Some(id);
    h.settle();
    h.click("Add tag");
    h.type_text("spec");
    h.key(egui::Key::Enter);
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == id).unwrap().tags,
        vec!["spec".to_string()]
    );
    h.open_docs_list();
    h.click("Docs tag filter");
    h.click("spec");
    assert_eq!(h.app.docs_tag.as_deref(), Some("spec"));
    assert!(h.has("Doc Project brief v2"));
    h.app.docs_tag = Some("other".into());
    h.settle();
    assert!(
        !h.has("Doc Project brief v2"),
        "a doc without the tag is filtered out"
    );
}

#[test]
fn docs_archived_leave_sidebar_and_sidebar_rows_have_a_menu() {
    let mut h = Ui1::new();
    h.app.section = Section::Home;
    h.settle();
    assert!(h.has("Project brief"));
    h.right_click("Project brief", 60.0);
    assert!(
        h.has("Mark as Meeting Note"),
        "the sidebar doc row has the doc menu"
    );
    h.key(egui::Key::Escape);
    for d in h
        .app
        .data
        .docs
        .iter_mut()
        .filter(|d| d.title == "Project brief")
    {
        d.archived = true;
    }
    h.settle();
    assert!(
        !h.has("Project brief"),
        "an archived doc leaves the sidebar"
    );
}

#[test]
fn docs_editor_keys_bold_enter_tab_and_slash_filter() {
    let mut h = Ui1::new();
    let cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    let id = h.new_doc_focused();
    // Cmd+B wraps (here: inserts) bold markers
    h.key_mods(egui::Key::B, cmd);
    h.type_text("x");
    assert_eq!(h.doc_body(id), "**x**");

    let id = h.new_doc_focused();
    h.type_text("- a");
    h.key(egui::Key::Enter);
    assert_eq!(h.doc_body(id), "- a\n- ", "Enter carries the bullet on");
    h.type_text("b");
    h.key(egui::Key::Tab);
    assert_eq!(h.doc_body(id), "- a\n  - b", "Tab nests the item");
    h.key(egui::Key::Enter);
    h.key(egui::Key::Enter);
    assert_eq!(
        h.doc_body(id),
        "- a\n  - b\n",
        "Enter on an empty item ends the list"
    );

    // "/" then a filter word and the arrow keys
    let id = h.new_doc_focused();
    h.type_text("/");
    h.type_text("head");
    assert!(
        h.has("Heading 3") && !h.has("Quote"),
        "the menu is filtered"
    );
    h.key(egui::Key::ArrowDown);
    h.key(egui::Key::Enter);
    assert_eq!(h.doc_body(id), "## ");
}

#[test]
fn docs_view_mode_lists_links_and_contents() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let id = h.app.doc_open.unwrap();
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .body =
        "# One\n* star item\n3. third\n- [X] done\n## Two\nsee [site](https://example.com)".into();
    h.app.doc_preview = true;
    h.settle();
    assert!(h.has("  \u{2022}  "), "a * bullet is drawn");
    assert!(h.has("  3.  "), "a numbered item keeps its number");
    assert!(h.has("site"), "the link text shows, not the Markdown");
    assert!(!h.has("[site](https://example.com)"));
    assert!(
        h.has("Contents One") && h.has("Contents Two"),
        "headings are listed in On this page"
    );
    h.click("Contents Two");
    // the meeting notes template uses real checklist items
    h.open_docs_list();
    h.click("Template Meeting Notes");
    let d = h
        .app
        .data
        .docs
        .iter()
        .find(|d| Some(d.id) == h.app.doc_open)
        .unwrap();
    assert!(
        d.meeting_note && d.body.ends_with("Action items\n- [ ] "),
        "{:?}",
        d.body
    );
    assert!(super::docs::line_kinds(&d.body)
        .iter()
        .any(|k| matches!(k.0, super::docs::LineKind::Check(false))));
}

#[test]
fn docs_editor_text_helpers() {
    use super::docs::{
        apply_link, continue_list, doc_headings, indent_line, inline_spans, line_kinds, LineKind,
        Span,
    };
    let mut s = "1. a\n".to_string();
    assert_eq!(continue_list(&mut s, 5), Some(8));
    assert_eq!(s, "1. a\n2. ");
    let mut s = "  - [x] a\n".to_string();
    continue_list(&mut s, 10);
    assert_eq!(s, "  - [x] a\n  - [ ] ");
    let mut s = "plain\n".to_string();
    assert_eq!(continue_list(&mut s, 6), None);
    let mut s = "a\n  b".to_string();
    assert_eq!(indent_line(&mut s, 3, false), 2);
    assert_eq!(s, "a\nb");
    let mut s = "see it".to_string();
    let c = apply_link(&mut s, 4, 6);
    assert_eq!(s, "see [it](https://)");
    assert_eq!(c, 17);
    let kinds: Vec<LineKind> = line_kinds("  - a\n    1. b\n  - [ ] c")
        .into_iter()
        .map(|k| k.0)
        .collect();
    assert_eq!(
        kinds,
        vec![LineKind::Bullet, LineKind::Numbered, LineKind::Check(false)]
    );
    let runs = inline_spans("a `c` ~~s~~ [t](u) https://x.y.");
    let kinds: Vec<Span> = runs
        .iter()
        .filter(|r| r.0 != Span::Marker && r.0 != Span::Text)
        .map(|r| r.0)
        .collect();
    assert_eq!(kinds, vec![Span::Code, Span::Strike, Span::Link, Span::Url]);
    assert_eq!(
        runs.iter().map(|r| r.1).collect::<String>(),
        "a `c` ~~s~~ [t](u) https://x.y.",
        "runs cover the line"
    );
    assert_eq!(
        runs.iter().find(|r| r.0 == Span::Url).unwrap().1,
        "https://x.y"
    );
    assert_eq!(
        doc_headings("# A\ntext\n### B"),
        vec![(1, "A".to_string(), 0), (3, "B".to_string(), 2)]
    );
}

// ---------------------------------------------------------------- list view slice (sf-listview)

impl Ui1 {
    // one frame with modifier keys held (egui reads them from the raw input)
    fn frame_held(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.size)),
            time: Some(self.time),
            events,
            modifiers,
            ..Default::default()
        };
        let app = &mut self.app;
        let _ = self.ctx.run(input, |ctx| app.ui_frame(ctx));
    }

    fn click_with(&mut self, name: &str, modifiers: egui::Modifiers, dx: Option<f32>) {
        self.scroll_into_view(name);
        let r = self.rect(name);
        let pos = dx
            .map(|d| pos2(r.left() + d, r.center().y))
            .unwrap_or(r.center());
        self.frame_held(vec![egui::Event::PointerMoved(pos)], modifiers);
        self.frame_held(
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
            modifiers,
        );
        self.frame_held(
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
            modifiers,
        );
        self.settle();
    }

    fn key_with(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }]);
        self.settle();
    }

    fn task_id(&self, name: &str) -> u64 {
        self.app
            .data
            .tasks
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id)
            .unwrap_or_else(|| panic!("no task {:?}", name))
    }
}

#[test]
fn drag_a_row_into_a_new_place() {
    let mut h = Ui1::new();
    let row_y = |h: &Ui1, n: &str| h.rect(&format!("Task {}", n)).center().y;
    assert!(
        row_y(&h, "Collect feedback from beta users") > row_y(&h, "Write copy for landing page")
    );
    h.drag(
        "Task Collect feedback from beta users",
        "Task Write copy for landing page",
    );
    assert!(h.app.open_task.is_none(), "a drag does not open the task");
    assert!(
        row_y(&h, "Collect feedback from beta users") < row_y(&h, "Write copy for landing page"),
        "the dragged row now comes first"
    );
    // dropped into another status group, it takes that status
    h.drag(
        "Task Set up analytics dashboard",
        "Task Fix navigation on mobile",
    );
    let t = h.app.task(h.task_id("Set up analytics dashboard")).unwrap();
    assert_eq!(t.status, "in progress");
}

#[test]
fn shift_click_selects_a_range_and_cmd_a_selects_all() {
    let mut h = Ui1::new();
    h.hover(h.find("Task Design new homepage layout").unwrap());
    h.click("Select Design new homepage layout");
    h.click_with(
        "Task Set up analytics dashboard",
        egui::Modifiers::SHIFT,
        Some(60.0),
    );
    // to do: Write copy, Set up analytics, Collect feedback; in progress: Design, Fix navigation
    assert_eq!(h.app.selected.len(), 3, "{:?}", h.app.selected);
    assert!(!h
        .app
        .selected
        .contains(&h.task_id("Write copy for landing page")));
    assert!(h
        .app
        .selected
        .contains(&h.task_id("Collect feedback from beta users")));
    h.key(egui::Key::Escape);
    assert!(h.app.selected.is_empty());
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    assert_eq!(h.app.selected.len(), 5, "every row shown");
}

#[test]
fn delete_key_asks_then_undo_brings_tasks_back() {
    let mut h = Ui1::new();
    let n = h.app.data.tasks.len();
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Select Write copy for landing page");
    h.click("Select Set up analytics dashboard");
    h.key(egui::Key::Backspace);
    assert!(h.app.confirm_bulk_delete, "Delete asks first");
    assert_eq!(h.app.data.tasks.len(), n);
    h.click("Confirm delete");
    assert_eq!(h.app.data.tasks.len(), n - 2);
    assert!(h.has("2 tasks deleted"));
    h.click("Undo delete");
    assert_eq!(h.app.data.tasks.len(), n);
    assert!(h.app.data.extra.trash.is_empty());
    // the row menu deletes at once, with the same Undo
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Task options Write copy for landing page");
    h.click("Delete");
    assert_eq!(h.app.data.tasks.len(), n - 1);
    h.click("Undo delete");
    assert_eq!(h.app.data.tasks.len(), n);
}

#[test]
fn group_menu_selects_and_collapses() {
    let mut h = Ui1::new();
    h.hover(h.find("Group options TO DO").unwrap());
    h.click("Group options TO DO");
    h.click("Select all in group");
    assert_eq!(h.app.selected.len(), 3);
    h.key(egui::Key::Escape);
    h.hover(h.find("Group options TO DO").unwrap());
    h.click("Group options TO DO");
    h.click("Collapse all groups");
    assert!(
        !h.has("Task Write copy for landing page") && !h.has("Task Design new homepage layout")
    );
    h.hover(h.find("Group options TO DO").unwrap());
    h.click("Group options TO DO");
    h.click("Expand all groups");
    assert!(h.has("Task Write copy for landing page"));
}

#[test]
fn row_quick_actions_rename_subtask_and_tag() {
    let mut h = Ui1::new();
    h.hover(h.find("Task Write copy for landing page").unwrap());
    h.click("Rename Write copy for landing page");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("Landing page copy");
    h.key(egui::Key::Enter);
    let id = h.task_id("Landing page copy");
    assert!(h.app.open_task.is_none());
    h.hover(h.find("Task Landing page copy").unwrap());
    h.click("Add subtask Landing page copy");
    h.type_text("Headline");
    h.key(egui::Key::Enter);
    let sub = h.app.task(h.task_id("Headline")).unwrap();
    assert_eq!(sub.parent, Some(id));
    h.key(egui::Key::Escape);
    h.hover(h.find("Task Landing page copy").unwrap());
    h.click("Add tag Landing page copy");
    h.click("design");
    assert!(h.app.task(id).unwrap().tags.contains(&"design".to_string()));
    // the row menu's Rename also renames in place
    h.key(egui::Key::Escape);
    h.right_click("Task Set up analytics dashboard", 60.0);
    h.click("Rename");
    assert!(h.app.row_rename.is_some() && h.app.rename.is_none());
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("Analytics");
    h.key(egui::Key::Enter);
    h.task_id("Analytics");
}

#[test]
fn arrow_keys_and_enter_open_a_row() {
    let mut h = Ui1::new();
    h.key(egui::Key::ArrowDown);
    h.key(egui::Key::ArrowDown);
    h.key(egui::Key::Enter);
    // the second row: TO DO comes first
    assert_eq!(
        h.app.open_task,
        Some(h.task_id("Set up analytics dashboard"))
    );
}

#[test]
fn bulk_bar_tags_unassign_start_date_and_unarchive() {
    let mut h = Ui1::new();
    let a = "Write copy for landing page";
    let b = "Design new homepage layout";
    h.hover(h.find(&format!("Task {}", a)).unwrap());
    h.click(&format!("Select {}", a));
    h.click(&format!("Select {}", b));
    h.click("Tags");
    h.type_text("launch");
    h.key(egui::Key::Enter);
    for n in [a, b] {
        assert!(
            h.app
                .task(h.task_id(n))
                .unwrap()
                .tags
                .contains(&"launch".to_string()),
            "{}",
            n
        );
    }
    h.key(egui::Key::Escape);
    h.click("Assignees");
    h.click("Unassign all");
    for n in [a, b] {
        assert!(h.app.task(h.task_id(n)).unwrap().assignees.is_empty());
    }
    h.click("Start date");
    h.click("Today");
    for n in [a, b] {
        assert_eq!(h.app.task(h.task_id(n)).unwrap().start, Some(today()));
    }
    h.click("Archive");
    h.app.filter.archived = true;
    h.settle();
    h.hover(h.find(&format!("Task {}", a)).unwrap());
    h.click(&format!("Select {}", a));
    h.click(&format!("Select {}", b));
    h.click("Unarchive");
    assert!(!h.app.task(h.task_id(a)).unwrap().archived);
}

// ---------------------------------------------------------------- task window fields (sf-taskfields)

fn open_design_task(h: &mut Ui1) -> u64 {
    h.click_left("Task Design new homepage layout", 120.0);
    h.app.open_task.expect("the task window opens")
}

#[test]
fn task_type_pill_switches_to_milestone() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("Task type");
    h.click("Milestone");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().milestone);
    h.click("Task type");
    h.click_side("Task", true);
    h.app.apply_actions();
    assert!(!h.app.task(id).unwrap().milestone);
}

#[test]
fn time_estimate_takes_hours_and_minutes_as_text() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("Time estimate value");
    h.type_text("2h 30m");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    assert_eq!(h.app.task(id).unwrap().estimate_min, Some(150));
    assert!(h.has("2h 30m"), "shown back as text");
}

#[test]
fn time_entries_add_by_hand_and_remove() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .time_entries
        .clear();
    h.settle();
    h.click("Time entries");
    h.click("Time to add");
    h.type_text("1h 30m");
    h.click("Add time");
    h.app.apply_actions();
    let t = h.app.task(id).unwrap();
    assert_eq!(t.time_entries.len(), 1);
    assert_eq!(t.time_entries[0].end - t.time_entries[0].start, 5400);
    h.click("Remove entry 1");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().time_entries.is_empty());
}

#[test]
fn tag_field_suggests_tags_in_use() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("Add tag");
    h.type_text("bu");
    assert!(h.has("Tag bug"), "the tag used on another task is offered");
    assert!(!h.has("Tag content"), "narrowed by what is typed");
    h.click("Tag bug");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().tags.contains(&"bug".to_string()));
}

#[test]
fn watch_toggle_and_watchers_menu() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    let me = h.app.data.me;
    h.click("Watch task");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().watchers.contains(&me));
    assert!(h.has("Stop watching"));
    h.click("Watchers");
    h.click("Watcher Sam Lee");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().watchers.contains(&3));
    h.key(egui::Key::Escape);
    h.click("Stop watching");
    h.app.apply_actions();
    assert_eq!(h.app.task(id).unwrap().watchers, vec![3]);
}

#[test]
fn checklist_items_rename_reorder_assign_and_become_subtasks() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    // rename by clicking the text
    h.click("Sketch three options");
    assert!(h.has("Checklist item name"));
    h.type_text(" now");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    assert_eq!(
        h.app.task(id).unwrap().checklist[1].text,
        "Sketch three options now"
    );
    // on hover: move up
    let p = h.find("Sketch three options now").unwrap();
    h.hover(p);
    h.click("Move up Sketch three options now");
    h.app.apply_actions();
    assert_eq!(
        h.app.task(id).unwrap().checklist[0].text,
        "Sketch three options now"
    );
    // assign
    let p = h.find("Share with the team").unwrap();
    h.hover(p);
    h.click("Assign item Share with the team");
    h.click_side("Sam Lee", true);
    h.app.apply_actions();
    assert_eq!(h.app.task(id).unwrap().checklist[2].assignee, Some(3));
    // turn into a subtask (it keeps the assignee)
    let p = h.find("Share with the team").unwrap();
    h.hover(p);
    h.click("Make subtask Share with the team");
    h.app.apply_actions();
    let t = h.app.task(id).unwrap();
    assert_eq!(t.checklist.len(), 2);
    let sub = h
        .app
        .data
        .tasks
        .iter()
        .find(|x| x.parent == Some(id) && x.name == "Share with the team")
        .expect("a subtask");
    assert_eq!(sub.assignees, vec![3]);
}

#[test]
fn subtask_rows_set_due_make_top_level_and_delete() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    let sub = |h: &Ui1, n: &str| {
        h.app
            .data
            .tasks
            .iter()
            .find(|t| t.name == n)
            .map(|t| (t.id, t.parent, t.due))
    };
    h.click("Subtask due Pick hero image");
    h.click("Today");
    h.app.apply_actions();
    assert_eq!(sub(&h, "Pick hero image").unwrap().2, Some(today()));
    let p = h.find("Subtask due Wireframe hero section").unwrap();
    h.hover(p);
    h.click("Make top-level Wireframe hero section");
    h.app.apply_actions();
    assert_eq!(sub(&h, "Wireframe hero section").unwrap().1, None);
    let p = h.find("Subtask due Pick hero image").unwrap();
    h.hover(p);
    h.click("Delete subtask Pick hero image");
    h.app.apply_actions();
    assert!(sub(&h, "Pick hero image").is_none());
    assert!(h.app.subtasks(id).is_empty());
}

#[test]
fn list_due_date_takes_a_time_removes_it_and_clearing_the_date_drops_it() {
    let mut h = Ui1::new();
    let name = "Write copy for landing page";
    let cell = format!("Due date of {}", name);
    // Add time: 9:00 on the date (today when there was none)
    h.click(&cell);
    h.click("Add time");
    h.app.apply_actions();
    let t = task_named(&h, name);
    assert!(t.due.is_some());
    assert_eq!(t.due_at(), chrono::NaiveTime::from_hms_opt(9, 0, 0));
    // type another time and press Enter
    h.click("Time of day");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("2:30 pm");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    let t = task_named(&h, name).clone();
    assert_eq!(t.due_at(), chrono::NaiveTime::from_hms_opt(14, 30, 0));
    // shown next to the date in the row, and in the history
    h.key(egui::Key::Escape);
    let shown = fmt_due(&t).unwrap();
    assert!(shown.ends_with("2:30 PM"), "{}", shown);
    assert!(
        !h.painted_text(&shown).is_empty(),
        "the row shows {:?}",
        shown
    );
    let line = &t.activity.last().unwrap().text;
    assert!(
        line.starts_with("changed due date to ") && line.ends_with(" at 2:30 PM"),
        "{}",
        line
    );
    // Remove time keeps the date
    h.click(&cell);
    h.click("Remove time");
    h.app.apply_actions();
    let t = task_named(&h, name);
    assert!(t.due.is_some());
    assert_eq!(t.due_at(), None);
    assert_eq!(t.due_time, Some(None), "a removed time is written as null");
    // with a time again, clearing the date drops the time too
    h.click("Add time");
    h.app.apply_actions();
    assert!(task_named(&h, name).due_at().is_some());
    h.click("Clear");
    h.app.apply_actions();
    let t = task_named(&h, name);
    assert_eq!((t.due, t.due_time), (None, Some(None)));
    // a new date afterwards starts without a time
    h.click(&cell);
    h.click("Tomorrow");
    h.app.apply_actions();
    let t = task_named(&h, name);
    assert_eq!(t.due, Some(today() + Duration::days(1)));
    assert_eq!(t.due_at(), None);
}

#[test]
fn subtask_due_date_takes_a_time() {
    let mut h = Ui1::new();
    open_design_task(&mut h);
    h.click("Subtask due Pick hero image");
    h.click("Add time");
    h.app.apply_actions();
    let t = task_named(&h, "Pick hero image");
    assert_eq!(t.due, Some(today()), "a time on no date sets today");
    assert_eq!(t.due_at(), chrono::NaiveTime::from_hms_opt(9, 0, 0));
    h.click("Remove time");
    h.app.apply_actions();
    assert_eq!(task_named(&h, "Pick hero image").due_at(), None);
}

#[test]
fn task_window_picker_sets_parent_and_adds_existing_subtask() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("\u{2026}");
    h.click("Make subtask of...");
    assert!(matches!(
        h.app.modal,
        Some(Modal::PickTask {
            mode: TaskPick::Parent,
            ..
        })
    ));
    assert!(
        !h.has("Pick Wireframe hero section"),
        "its own subtasks can't be its parent"
    );
    h.click("Search tasks to pick");
    h.type_text("q1");
    h.click("Pick Plan Q1 campaign");
    let q1 = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Plan Q1 campaign")
        .unwrap()
        .clone();
    let t = h.app.task(id).unwrap();
    assert_eq!(t.parent, Some(q1.id));
    assert_eq!(
        t.list_id, q1.list_id,
        "a subtask moves into its parent's List"
    );
    assert!(h.app.modal.is_none());
    // the reverse: put an existing task under this one
    h.click("+  Add existing task");
    h.click("Search tasks to pick");
    h.type_text("passport");
    h.click("Pick Renew passport");
    let p = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Renew passport")
        .unwrap();
    assert_eq!(p.parent, Some(id));
}

#[test]
fn relationships_waiting_on_blocking_and_linked() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.click("+  Add relationship");
    h.click("Waiting on");
    h.click("Search tasks to pick");
    h.type_text("analytics");
    h.click("Pick Set up analytics dashboard");
    let other = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Set up analytics dashboard")
        .unwrap()
        .id;
    assert_eq!(h.app.task(id).unwrap().waiting_on, vec![other]);
    assert!(h.has("Waiting on Set up analytics dashboard"));
    // the other task shows it as Blocking
    assert_eq!(h.app.relations(other).1, vec![id]);
    h.click("+  Add relationship");
    h.click("Link a task");
    h.click("Search tasks to pick");
    h.type_text("newsletter");
    h.click("Pick Newsletter draft");
    assert!(h.has("Link a task Newsletter draft"));
    h.click("Remove link Set up analytics dashboard");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().waiting_on.is_empty());
    // a row opens the other task
    h.click("Link a task Newsletter draft");
    h.app.apply_actions();
    assert_ne!(h.app.open_task, Some(id));
}

#[test]
fn description_keeps_markdown_and_ticks_checkboxes() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .description = "- [ ] step".into();
    h.settle();
    // the "- [ ] " marker is hidden and a box is painted instead
    let r = h.rect("Add description");
    h.click_at(pos2(r.left() + 10.0, r.top() + 10.0));
    h.app.apply_actions();
    assert_eq!(h.app.task(id).unwrap().description, "- [x] step");
}

#[test]
fn pictures_dropped_on_a_task_attach_and_can_be_removed() {
    let mut h = Ui1::new();
    let id = open_design_task(&mut h);
    let pic = test_picture("mockup.png", 300, 200);
    h.dropped = vec![egui::DroppedFile {
        path: Some(pic),
        ..Default::default()
    }];
    h.frame(vec![]);
    assert!(h.app.chat_files.is_empty(), "not taken by the chat box");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.app.task(id).unwrap().attachments.is_empty() && std::time::Instant::now() < until {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let t = h.app.task(id).unwrap();
    assert_eq!(t.attachments.len(), 1);
    assert_eq!(
        (t.attachments[0].name.as_str(), t.attachments[0].w),
        ("mockup.png", 300)
    );
    assert!(h.app.task_files.is_empty());
    h.settle();
    assert!(h.has("Attachment mockup.png"));
    h.click("Remove attachment mockup.png");
    h.app.apply_actions();
    assert!(h.app.task(id).unwrap().attachments.is_empty());
}

// ---------------------------------------------------------------- views and sidebar slice
// (board, calendar, gantt, table and the sidebar's spaces, folders, lists, favorites)

fn task_named<'a>(h: &'a Ui1, name: &str) -> &'a Task {
    h.app
        .data
        .tasks
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("no task {:?}", name))
}

fn rect_of(h: &Ui1, name: &str) -> Rect {
    h.tree
        .iter()
        .rev()
        .find(|(n, _)| n == name)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("nothing named {:?}", name))
}

fn drag_by(h: &mut Ui1, from: Pos2, to: Pos2) {
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    for i in 1..=12 {
        h.frame(vec![egui::Event::PointerMoved(
            from + (to - from) * (i as f32 / 12.0),
        )]);
    }
    h.frame(vec![egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    h.settle();
}

#[test]
fn board_card_boxes_open_the_pickers_and_the_card_opens() {
    let mut h = Ui1::new();
    h.click("View Board");
    h.click("Card priority Set up analytics dashboard");
    h.click("Urgent");
    assert_eq!(task_named(&h, "Set up analytics dashboard").priority, 1);
    assert!(
        h.app.open_task.is_none(),
        "the box changes the field, it doesn't open the task"
    );
    h.click("Card assignees Set up analytics dashboard");
    h.click_side("Sam Lee", true);
    assert_eq!(
        task_named(&h, "Set up analytics dashboard").assignees,
        vec![3]
    );
    h.key(egui::Key::Escape);
    h.click("Card due date Collect feedback from beta users");
    h.click("Today");
    assert_eq!(
        task_named(&h, "Collect feedback from beta users").due,
        Some(today())
    );
    h.key(egui::Key::Escape);
    h.click("Card Write copy for landing page");
    assert_eq!(
        h.app.open_task,
        Some(task_named(&h, "Write copy for landing page").id)
    );
}

#[test]
fn board_card_options_button_opens_the_task_menu() {
    let mut h = Ui1::new();
    h.click("View Board");
    let n = h.app.data.tasks.len();
    h.click("Card options Set up analytics dashboard");
    h.click("Duplicate");
    assert!(
        h.app.data.tasks.len() > n,
        "Duplicate from the card's ... menu"
    );
}

#[test]
fn board_add_task_asks_for_a_name_first() {
    let mut h = Ui1::new();
    h.click("View Board");
    let n = h.app.data.tasks.len();
    h.click("Add Task in TO DO");
    assert_eq!(
        h.app.data.tasks.len(),
        n,
        "no Untitled task is made by the click"
    );
    h.type_text("Board task");
    h.key(egui::Key::Enter);
    assert_eq!(task_named(&h, "Board task").status, "to do");
    // Esc on an empty field leaves nothing behind
    h.key(egui::Key::Escape);
    assert_eq!(h.app.data.tasks.len(), n + 1);
    assert!(h.app.open_task.is_none());
}

#[test]
fn board_columns_collapse_and_expand() {
    let mut h = Ui1::new();
    h.click("View Board");
    h.click("Collapse group TO DO");
    assert!(
        !h.has("Card Set up analytics dashboard"),
        "the column's cards are hidden"
    );
    h.click("Expand group TO DO");
    assert!(h.has("Card Set up analytics dashboard"));
}

#[test]
fn board_card_dropped_on_a_sidebar_list_moves_the_task() {
    // tall enough for the whole sidebar
    let mut h = Ui1::sized(1440.0, 1200.0);
    h.click("View Board");
    let personal = h.app.data.spaces[0].lists[0].id;
    h.drag("Card Set up analytics dashboard", "Personal");
    assert_eq!(
        task_named(&h, "Set up analytics dashboard").list_id,
        personal
    );
}

#[test]
fn calendar_more_opens_the_days_tasks_and_spans_show_every_day() {
    let mut h = Ui1::new();
    let td = today();
    let base = task_named(&h, "Set up analytics dashboard").clone();
    for k in 0..8 {
        let mut x = base.clone();
        x.id = 9000 + k;
        x.name = format!("Busy day {}", k);
        x.start = None;
        x.due = Some(td);
        h.app.data.tasks.push(x);
    }
    let mut span = base.clone();
    span.id = 9100;
    span.name = "Three day task".into();
    span.start = Some(td);
    span.due = Some(td + Duration::days(2));
    h.app.data.tasks.push(span);
    h.click("View Calendar");
    for d in 0..3 {
        assert!(
            h.has(&format!(
                "Calendar Three day task {}",
                td + Duration::days(d)
            )),
            "drawn on day {}",
            d
        );
    }
    h.click(&format!("More tasks on {}", td));
    h.click("Busy day 7");
    assert_eq!(h.app.open_task, Some(9007));
}

#[test]
fn calendar_shows_due_times_and_orders_a_day_by_them() {
    let mut h = Ui1::new();
    let td = today();
    let base = task_named(&h, "Set up analytics dashboard").clone();
    // only these on the calendar, so none is hidden under "+N more"
    for x in h.app.data.tasks.iter_mut() {
        x.start = None;
        x.due = None;
    }
    for (k, name, at) in [
        (0, "Late call", Some(15)),
        (1, "Early call", Some(9)),
        (2, "All day", None),
    ] {
        let mut x = base.clone();
        x.id = 9200 + k;
        x.name = name.into();
        x.start = None;
        x.due = Some(td);
        x.set_due_time(at.and_then(|hr| chrono::NaiveTime::from_hms_opt(hr, 0, 0)));
        h.app.data.tasks.push(x);
    }
    h.click("View Calendar");
    let y = |h: &Ui1, n: &str| rect_of(h, &format!("Calendar {} {}", n, td)).center().y;
    assert!(
        y(&h, "All day") < y(&h, "Early call"),
        "a day without a time first"
    );
    assert!(y(&h, "Early call") < y(&h, "Late call"), "then by time");
    assert!(!h.painted_text("9:00 AM").is_empty());
    assert!(!h.painted_text("3:00 PM").is_empty());
}

#[test]
fn calendar_unscheduled_items_have_the_task_menu() {
    let mut h = Ui1::new();
    h.click("View Calendar");
    let n = h.app.data.tasks.len();
    h.right_click("Unscheduled Collect feedback from beta users", 20.0);
    h.click("Duplicate");
    assert_eq!(h.app.data.tasks.len(), n + 1);
}

#[test]
fn gantt_bar_ends_resize_and_empty_rows_schedule() {
    let mut h = Ui1::new();
    h.click("View Gantt");
    let before = task_named(&h, "Write copy for landing page").clone();
    let r = rect_of(&h, "End of Write copy for landing page");
    // a day is 30 px wide at the Day scale
    drag_by(&mut h, r.center(), r.center() + vec2(90.0, 0.0));
    let after = task_named(&h, "Write copy for landing page").clone();
    assert_eq!(after.due, before.due.map(|d| d + Duration::days(3)));
    assert_eq!(after.start, before.start, "only the end moved");
    let r = rect_of(&h, "Start of Write copy for landing page");
    drag_by(&mut h, r.center(), r.center() - vec2(60.0, 0.0));
    assert_eq!(
        task_named(&h, "Write copy for landing page").start,
        before.start.map(|d| d - Duration::days(2))
    );
    // a task without dates gets them from a click in its row
    let r = rect_of(&h, "Schedule Collect feedback from beta users");
    // (the row runs under the names column too; click where it shows)
    h.click_at(pos2(1000.0, r.center().y));
    assert!(task_named(&h, "Collect feedback from beta users")
        .due
        .is_some());
}

#[test]
fn gantt_shows_subtasks_when_expanded() {
    let mut h = Ui1::new();
    h.click("View Gantt");
    assert!(!h.has("Schedule Wireframe hero section"));
    h.click("Subtasks of Design new homepage layout");
    assert!(
        h.has("Schedule Wireframe hero section"),
        "the subtask gets its own row"
    );
}

fn count_named(h: &Ui1, name: &str) -> usize {
    h.tree.iter().filter(|(n, _)| n == name).count()
}

#[test]
fn timeline_lanes_switch_grouping() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    assert!(h.app.view == ViewKind::Timeline);
    // by assignee: the task Me and Alex share has a bar in both lanes
    for lane in [
        "Lane ME",
        "Lane ALEX KIM",
        "Lane SAM LEE",
        "Lane UNASSIGNED",
    ] {
        assert!(h.has(lane), "{} is shown", lane);
    }
    assert_eq!(count_named(&h, "Bar Design new homepage layout"), 2);
    // a task with only a due date gets a one-day bar in the Unassigned lane
    let bar = rect_of(&h, "Bar Set up analytics dashboard");
    assert!(bar.width() < 30.0, "one day wide: {}", bar.width());
    let lane = rect_of(&h, "Lane UNASSIGNED");
    assert!(bar.center().y > lane.top() && bar.center().y < lane.bottom());
    h.click("Lanes by Status");
    assert_eq!(h.app.timeline.by, GroupBy::Status);
    assert!(h.has("Lane IN PROGRESS") && h.has("Lane TO DO"));
    assert!(!h.has("Lane ALEX KIM"));
    assert_eq!(count_named(&h, "Bar Design new homepage layout"), 1);
    h.click("Lanes by Priority");
    assert_eq!(h.app.timeline.by, GroupBy::Priority);
    assert!(h.has("Lane URGENT"));
    h.click("Lanes by List");
    assert_eq!(h.app.timeline.by, GroupBy::List);
    assert!(h.has("Lane WEBSITE REDESIGN"));
    h.click("Lanes by Assignee");
    assert!(h.has("Lane UNASSIGNED"));
}

#[test]
fn timeline_bar_opens_and_drags_dates() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    let id = task_named(&h, "Write copy for landing page").id;
    h.click("Bar Write copy for landing page");
    assert_eq!(h.app.open_task, Some(id), "a click opens the task");
    h.app.open_task = None;
    h.settle();
    let before = task_named(&h, "Write copy for landing page").clone();
    // a day is 30 px wide at the Day scale, as in the Gantt
    let r = rect_of(&h, "Bar Write copy for landing page");
    drag_by(&mut h, r.center(), r.center() + vec2(60.0, 0.0));
    let moved = task_named(&h, "Write copy for landing page").clone();
    assert_eq!(moved.start, before.start.map(|d| d + Duration::days(2)));
    assert_eq!(moved.due, before.due.map(|d| d + Duration::days(2)));
    assert!(h.app.open_task.is_none(), "a drag doesn't open the task");
    let r = rect_of(&h, "End of Write copy for landing page");
    drag_by(&mut h, r.center(), r.center() + vec2(90.0, 0.0));
    let after = task_named(&h, "Write copy for landing page").clone();
    assert_eq!(after.due, moved.due.map(|d| d + Duration::days(3)));
    assert_eq!(after.start, moved.start, "only the end moved");
}

#[test]
fn timeline_keeps_a_name_beside_a_bar_off_the_next_bar() {
    let mut h = Ui1::new();
    // two one-day tasks on neighbouring days in the Unassigned lane: the
    // first bar is too narrow for its name, which is written to its right
    let a = task_named(&h, "Write copy for landing page").id;
    let b = task_named(&h, "Set up analytics dashboard").id;
    let today = today();
    for (n, id) in [a, b].into_iter().enumerate() {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.assignees.clear();
        t.start = None;
        t.due = Some(today + Duration::days(n as i64));
    }
    h.click("View Timeline");
    let first = rect_of(&h, "Bar Write copy for landing page");
    let second = rect_of(&h, "Bar Set up analytics dashboard");
    assert!(first.width() < 60.0, "one day wide: {}", first.width());
    assert!(
        second.left() - first.right() < 60.0,
        "the bars sit on neighbouring days"
    );
    assert!(
        (first.center().y - second.center().y).abs() > 20.0,
        "the second bar gets its own row: {:?} {:?}",
        first,
        second
    );
    let lane = rect_of(&h, "Lane UNASSIGNED");
    for r in [first, second] {
        assert!(r.center().y > lane.top() && r.center().y < lane.bottom());
    }
}

#[test]
fn timeline_tray_lists_tasks_without_dates() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    assert!(!h.has("Undated Collect feedback from beta users"));
    h.click("Tasks without dates");
    let id = task_named(&h, "Collect feedback from beta users").id;
    h.click("Undated Collect feedback from beta users");
    assert_eq!(h.app.open_task, Some(id));
}

#[test]
fn timeline_follows_the_filter_and_zoom() {
    let mut h = Ui1::new();
    h.click("View Timeline");
    assert!(h.has("Bar Design new homepage layout"));
    h.app.search = "copy".into();
    h.settle();
    assert!(h.has("Bar Write copy for landing page"));
    assert!(!h.has("Bar Design new homepage layout"), "filtered out");
    h.app.search.clear();
    h.app.me_mode = true;
    h.settle();
    assert!(h.has("Bar Fix navigation on mobile"));
    assert!(!h.has("Bar Write copy for landing page"), "Me mode");
    h.app.me_mode = false;
    h.click("Month");
    assert_eq!(h.app.timeline.zoom, 2);
    h.click("Today");
    let r = rect_of(&h, "Bar Write copy for landing page");
    assert!(r.left() > 0.0 && r.right() < 1440.0, "today is in view");
    assert!(r.width() < 30.0, "narrow at the Month scale");
}

#[test]
fn table_sorts_renames_and_chooses_columns() {
    // wide enough for every column and the "+" after them
    let mut h = Ui1::sized(1900.0, 900.0);
    h.click("View Table");
    h.click("Sort by Name");
    let y = |h: &Ui1, n: &str| rect_of(h, &format!("Rename {}", n)).center().y;
    assert!(
        y(&h, "Collect feedback from beta users") < y(&h, "Write copy for landing page"),
        "A to Z"
    );
    h.click("Sort by Name");
    assert!(
        y(&h, "Collect feedback from beta users") > y(&h, "Write copy for landing page"),
        "Z to A"
    );
    h.click("Rename Collect feedback from beta users");
    h.frame(vec![egui::Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    h.type_text("Pick colours");
    h.key(egui::Key::Enter);
    assert!(h.app.data.tasks.iter().any(|t| t.name == "Pick colours"));
    assert!(h.app.open_task.is_none(), "renaming doesn't open the task");
    // custom fields get columns; any column but # and Name can be hidden
    h.app.data.extra.fields.push(CustomField {
        id: 777,
        name: "Budget".into(),
        kind: "number".into(),
        options: vec![],
    });
    h.settle();
    assert!(h.has("Sort by Budget"));
    h.click("Table columns");
    h.click("Column Tags");
    assert!(!h.has("Sort by Tags"));
    h.click("Column Budget");
    assert!(!h.has("Sort by Budget"));
}

#[test]
fn folder_row_menu_duplicate_archive_and_move() {
    let mut h = Ui1::new();
    h.click("Folder options Projects");
    h.click("Duplicate");
    assert_eq!(h.app.data.spaces[0].folders.len(), 2);
    let copy = h.app.data.spaces[0].folders[1].id;
    assert_eq!(
        h.app.data.spaces[0].folders[1].lists.len(),
        2,
        "the lists are copied"
    );
    h.key(egui::Key::Escape);
    h.click("Folder options Projects (copy)");
    h.click("Archive");
    assert!(h.app.data.spaces[0].folders[1].archived);
    assert!(
        !h.has("Projects (copy)"),
        "archived folders leave the sidebar"
    );
    let lid = h.app.data.spaces[0].folders[1].lists[0].id;
    assert!(h.app.data.list_archived(lid));
    h.app.set_folder_archived(copy, false);
    let side = h.app.data.spaces[1].id;
    h.app.move_folder(copy, side);
    assert!(h.app.data.spaces[1].folders.iter().any(|f| f.id == copy));
    assert_eq!(h.app.data.spaces[0].folders.len(), 1);
}

#[test]
fn list_color_from_the_list_menu() {
    let mut h = Ui1::new();
    h.click("List options Personal");
    h.click("Color");
    let c = STATUS_PALETTE[2];
    h.click(&format!("List color {:02x}{:02x}{:02x}", c[0], c[1], c[2]));
    assert_eq!(h.app.data.spaces[0].lists[0].color, Some(c));
}

#[test]
fn favorite_a_task_and_open_it_from_the_sidebar() {
    let mut h = Ui1::new();
    h.click("Task options Write copy for landing page");
    h.click("Add to Favorites");
    let id = task_named(&h, "Write copy for landing page").id;
    assert!(h.app.data.extra.starred.contains(&format!("task:{}", id)));
    h.click_side("Write copy for landing page", false);
    assert_eq!(h.app.open_task, Some(id));
}

#[test]
fn sidebar_lists_reorder_by_drag() {
    let mut h = Ui1::sized(1440.0, 1200.0);
    let personal = h.app.data.spaces[0].lists[0].id;
    h.drag("Personal", "Marketing");
    let f = &h.app.data.spaces[0].folders[0];
    assert_eq!(
        f.lists.iter().map(|l| l.id).collect::<Vec<_>>()[1],
        personal,
        "Personal lands before Marketing"
    );
    assert!(h.app.data.spaces[0].lists.is_empty());
}

// ---------------------------------------------------------------- apps sweep (home, planner, reminders, apps, search)

fn cmd_key(h: &mut Ui1, key: egui::Key) {
    let cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    h.frame(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: cmd,
    }]);
    h.frame(vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: cmd,
    }]);
    h.settle();
}

#[test]
fn home_my_work_row_ticks_done() {
    let mut h = Ui1::new();
    h.app.actions.push(Action::Go(Location::Home));
    h.settle();
    h.click("Complete Renew passport");
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Renew passport")
        .unwrap();
    assert!(h.app.data.is_closed(t));
    // the Done section starts folded
    h.app.home_collapsed.remove("Done");
    h.settle();
    h.click("Reopen Renew passport");
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Renew passport")
        .unwrap();
    assert!(!h.app.data.is_closed(t));
}

#[test]
fn planner_day_adds_a_task_for_me() {
    let mut h = Ui1::new();
    h.app.section = Section::Planner;
    h.settle();
    let day = h.app.planner_week + Duration::days(3);
    h.click(&format!("Add task on {}", day.format("%a %-d")));
    h.type_text("Planned from the day");
    h.key(egui::Key::Enter);
    let t = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Planned from the day")
        .expect("task added");
    assert_eq!(t.due, Some(day));
    assert_eq!(t.assignees, vec![h.app.data.me]);
}

#[test]
fn reminders_snooze_edit_delete_and_show_done() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    h.app.data.extra.reminders.push(Reminder {
        id: 501,
        text: "Call the bank".into(),
        due_ts: now_ts() + 600,
        done: false,
        owner: me,
        ..Default::default()
    });
    h.app.data.extra.reminders.push(Reminder {
        id: 502,
        text: "Old one".into(),
        due_ts: now_ts() - 600,
        done: true,
        owner: me,
        ..Default::default()
    });
    h.click("Inbox");
    h.click("Snooze Call the bank");
    h.click("Tomorrow 9 AM");
    let r = h
        .app
        .data
        .extra
        .reminders
        .iter()
        .find(|r| r.id == 501)
        .unwrap();
    assert!(r.due_ts > now_ts() + 3600);
    h.click("Edit Call the bank");
    h.click("Reminder text");
    h.type_text(" back");
    h.click("Save");
    assert!(h
        .app
        .data
        .extra
        .reminders
        .iter()
        .any(|r| r.text == "Call the bank back"));
    assert!(!h.has("Old one"));
    h.click("Show done reminders");
    assert!(h.has("Old one"));
    h.click("Done Old one");
    assert!(
        !h.app
            .data
            .extra
            .reminders
            .iter()
            .find(|r| r.id == 502)
            .unwrap()
            .done,
        "a done reminder can be ticked back"
    );
    h.click("Delete Call the bank back");
    assert!(!h.app.data.extra.reminders.iter().any(|r| r.id == 501));
}

#[test]
fn inbox_later_item_goes_back_to_primary() {
    let mut h = Ui1::new();
    let tid = h.app.data.tasks[1].id;
    let ts = now_ts() - 30;
    h.app.data.tasks[1].activity.push(Activity {
        ts,
        author: 2,
        text: "Over to you".into(),
        comment: true,
        post: false,
        ..Default::default()
    });
    let key = format!("{}:{}:2", tid, ts);
    h.app.data.extra.inbox_later.push(key.clone());
    h.click("Inbox");
    h.click("Inbox Later");
    let p = h
        .find("Write copy for landing page")
        .expect("the item is in Later");
    h.hover(p);
    h.click("Primary");
    assert!(!h.app.data.extra.inbox_later.contains(&key));
}

#[test]
fn replies_show_chat_replies_and_mark_one_read() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.name == "general")
        .unwrap();
    let mut m = c.messages[0].clone();
    m.id = 9001;
    m.author = me;
    m.text = "Who has the keys?".into();
    let mut r = m.clone();
    r.id = 9002;
    r.author = 3;
    r.text = "I do".into();
    r.ts = now_ts();
    r.replies = vec![];
    m.replies = vec![r];
    c.messages.push(m);
    h.app.data.extra.replies_seen = now_ts() - 100;
    let ch = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    h.click("Replies");
    assert!(h.has("Mark read I do"));
    h.click("Mark read I do");
    assert!(h
        .app
        .data
        .extra
        .replies_read
        .contains(&format!("chat:{}:9002", ch)));
    assert!(!h.has("Mark read I do"));
}

#[test]
fn whiteboard_notes_delete_undo_and_clear_asks() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    wb.notes.push(Sticky {
        id: 0,
        pos: [40.0, 40.0],
        text: "One".into(),
        color: [0xf8, 0xae, 0x00],
    });
    wb.strokes.push(WbStroke {
        id: 0,
        color: [0, 0, 0],
        width: 2.0,
        points: vec![[1.0, 1.0], [5.0, 5.0]],
    });
    h.settle();
    h.click("Note colour 1");
    h.settle();
    let board = |h: &Ui1| {
        h.app
            .data
            .extra
            .whiteboards
            .iter()
            .find(|w| w.id == wid)
            .cloned()
            .unwrap()
    };
    h.key(egui::Key::Escape);
    h.click("Delete note 0");
    assert!(board(&h).notes.is_empty());
    h.click("Undo");
    assert_eq!(board(&h).notes.len(), 1, "undo brings the note back");
    h.click("Clear");
    assert_eq!(board(&h).strokes.len(), 1, "Clear asks first");
    h.click("Yes");
    let b = board(&h);
    assert!(
        b.strokes.is_empty() && b.notes.is_empty(),
        "{:?} {:?} {:?}",
        b.strokes.len(),
        b.notes.len(),
        h.app.sweep.confirm
    );
    h.click("Delete");
    assert!(
        matches!(
            h.app.modal,
            Some(Modal::ConfirmRemove {
                kind: "whiteboard",
                ..
            })
        ),
        "Delete asks first: {:?}",
        h.app.modal.is_some()
    );
    // the confirm dialog's own Delete button (the lowest of the "Delete"s)
    let at = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Delete" && r.height() < 40.0)
        .map(|(_, r)| r.center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    h.click_at(at);
    assert!(h.app.data.extra.whiteboards.iter().all(|w| w.id != wid));
    assert!(h
        .app
        .data
        .extra
        .trash
        .iter()
        .any(|x| x.kind == "Whiteboard"));
}

#[test]
fn form_questions_reorder_require_and_block_bad_submit() {
    let mut h = Ui1::new();
    h.app.new_form();
    h.settle();
    let fid = h.app.form_open.unwrap();
    let form = |h: &Ui1| {
        h.app
            .data
            .extra
            .forms
            .iter()
            .find(|f| f.id == fid)
            .cloned()
            .unwrap()
    };
    h.click("Move question 2 up");
    assert_eq!(form(&h).fields[0].label, "Details");
    h.click("Required 1");
    assert!(form(&h).fields[0].required);
    h.app.form_mode_fill = true;
    h.settle();
    let n = h.app.data.tasks.len();
    h.click("Submit");
    assert_eq!(
        h.app.data.tasks.len(),
        n,
        "a required question stops the submit"
    );
    assert!(h
        .app
        .sweep
        .form_error
        .as_deref()
        .unwrap_or("")
        .contains("Details"));
    h.click("Answer 1");
    h.type_text("Needs a desk");
    h.click("Submit");
    assert_eq!(h.app.data.tasks.len(), n + 1);
    // answers don't follow you to another form
    h.app.form_fill.insert(0, "left over".into());
    h.app.new_form();
    h.settle();
    assert!(!h.app.form_fill.values().any(|v| v == "left over"));
}

#[test]
fn dashboard_cards_move_rename_remove_and_add() {
    let mut h = Ui1::new();
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Team board");
    h.settle();
    let did = h.app.dash_open.unwrap();
    let cards = |h: &Ui1| {
        h.app
            .data
            .extra
            .dashboards
            .iter()
            .find(|d| d.id == did)
            .unwrap()
            .cards
            .clone()
    };
    h.click("Remove card Tasks by priority");
    assert!(!cards(&h).unwrap().iter().any(|c| c.kind == "priority"));
    h.click("Move card up Workload by assignee");
    assert_eq!(cards(&h).unwrap()[1].kind, "workload");
    h.click("Rename card Task totals");
    h.type_text(" now");
    h.key(egui::Key::Enter);
    assert_eq!(cards(&h).unwrap()[0].title, "Task totals now");
    h.click("+ Add card");
    h.click("Add card Tasks by priority");
    assert_eq!(cards(&h).unwrap().last().unwrap().kind, "priority");
    // the dashboard can be renamed and starred
    h.click(&format!("Star dash:{}", did));
    assert!(h.app.data.extra.starred.contains(&format!("dash:{}", did)));
}

#[test]
fn search_link_arrows_and_enter() {
    let mut h = Ui1::new();
    let tid = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Newsletter draft")
        .unwrap()
        .id;
    h.app.palette = Some(format!("workboard://task/{}", tid));
    h.settle();
    h.key(egui::Key::Enter);
    assert_eq!(h.app.open_task, Some(tid));
    h.app.open_task = None;
    // arrows move the highlight; Enter opens the highlighted result
    h.app.palette = Some("Projects".into());
    h.settle();
    h.key(egui::Key::ArrowDown);
    let rows = h.app.search_rows("Projects", 0);
    assert_eq!(h.app.sweep.search_sel, 1);
    h.key(egui::Key::Enter);
    assert!(h.app.palette.is_none());
    assert_eq!(
        h.app.sweep.search_recent.first().map(String::as_str),
        Some("Projects")
    );
    if let super::more::Hit::Go(l) = rows[1].4 {
        assert_eq!(h.app.loc, l);
    }
}

#[test]
fn cmd_brackets_and_comma_shortcuts() {
    let mut h = Ui1::new();
    let start = h.app.loc;
    h.click("Inbox");
    cmd_key(&mut h, egui::Key::OpenBracket);
    assert_eq!(h.app.loc, start, "Cmd+[ goes back");
    cmd_key(&mut h, egui::Key::CloseBracket);
    assert_eq!(h.app.loc, Location::Inbox, "Cmd+] goes forward");
    cmd_key(&mut h, egui::Key::Comma);
    assert_eq!(h.app.section, Section::Settings);
}

#[test]
fn trash_empty_asks_then_clears() {
    let mut h = Ui1::new();
    let lid = h.app.data.spaces[1].lists[0].id;
    h.app.do_delete(Target::List(lid));
    h.app.modal = Some(Modal::Trash);
    h.settle();
    h.click("Empty trash");
    assert!(!h.app.data.extra.trash.is_empty());
    h.click("Yes");
    assert!(h.app.data.extra.trash.is_empty());
}

#[test]
fn invited_people_are_marked_in_channel_and_team_member_lists() {
    let mut h = Ui1::new();
    h.app
        .invite_people(&["dana.reed@example.com".to_string()], "Member");
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    // in #general already, for when they join: marked as not here yet
    h.app.modal = Some(Modal::ChannelSettings(general));
    h.settle();
    assert!(h.has("Dana Reed (invited)"));
    h.key(egui::Key::Escape);
    h.app.section = Section::Teams;
    h.settle();
    h.click_side("Create team", true);
    assert!(h.has("Dana Reed (invited)"));
}

#[test]
fn team_color_and_my_teams() {
    let mut h = Ui1::new();
    h.app.section = Section::Teams;
    h.settle();
    h.click_side("Create team", true);
    let id = h.app.team_open.unwrap();
    h.click("Team color 3");
    assert_eq!(
        h.app
            .data
            .extra
            .teams
            .iter()
            .find(|t| t.id == id)
            .unwrap()
            .color,
        STATUS_PALETTE[3]
    );
    h.app.data.extra.teams.push(Team {
        id: 7,
        name: "Not mine".into(),
        members: vec![2],
        color: [1, 1, 1],
    });
    h.click("My Teams");
    assert_eq!(h.app.hub_filter, 4);
    assert!(h.has("Team 1") && !h.has("Not mine"));
}

#[test]
fn timesheet_entry_edits_and_week_picker() {
    let mut h = Ui1::new();
    let td = today();
    let ws = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
    let start = td
        .and_hms_opt(10, 0, 0)
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
        .position(|t| t.name == "Renew passport")
        .unwrap();
    h.app.data.tasks[i].time_entries = vec![TimeEntry {
        start,
        end: start + 1800,
        member: 1,
    }];
    h.app.section = Section::Timesheets;
    h.app.sheet_week = ws;
    h.app.sheet_entries = true;
    h.settle();
    h.click("Edit entry Renew passport");
    h.app.sweep.entry_edit.as_mut().unwrap().hours = 2.0;
    h.click("Save");
    let e = &h.app.data.tasks[i].time_entries[0];
    assert_eq!(e.end - e.start, 7200);
    h.app.sheet_entries = false;
    h.settle();
    h.click("Pick a week");
    assert!(
        h.has("Today") || h.has("Tomorrow"),
        "the week picker opens a calendar"
    );
}
#[path = "i18n_tests.rs"]
mod i18n_tests;
#[path = "layout_tests.rs"]
mod layout_tests;
#[path = "template_tests.rs"]
mod template_tests;
#[path = "wording_tests.rs"]
mod wording_tests;

fn sign_in_screen(h: &mut Ui1, addr: &str) {
    h.app.login = Some(super::login::LoginScreen::start(addr));
    h.settle();
}

fn login_page(h: &Ui1) -> Option<super::login::LoginPage> {
    h.app.login.as_ref().map(|l| l.page)
}

#[test]
fn the_first_start_shows_the_sign_in_screen_with_working_alone_as_a_choice() {
    use super::login::LoginPage;
    assert!(
        Data::fresh().extra.welcome,
        "a first start shows the sign-in screen"
    );
    let old: Data = serde_json::from_str(
        &serde_json::to_string(&Data::seed())
            .unwrap()
            .replace(r#","welcome":false"#, ""),
    )
    .unwrap();
    assert!(!old.extra.welcome, "files from before don't");
    let mut h = Ui1::new();
    sign_in_screen(&mut h, "");
    // the screen fills the window, starting with the server's address:
    // none of the app shows, and it doesn't close
    assert_eq!(login_page(&h), Some(LoginPage::Server));
    assert!(
        h.has("Sign in to your team")
            && h.has("Server address entry")
            && h.has("Continue")
            && h.has("Set up a server for my team")
            && h.has("Use it on this Mac without an account")
            && !h.has("Email entry")
    );
    assert!(!h.has("Workspace menu") && !h.has("Profile menu"));
    h.key(egui::Key::Escape);
    h.key_with(egui::Key::N, egui::Modifiers::COMMAND);
    assert!(h.app.login.is_some() && h.app.quick_add.is_none());

    // no address: it says so and stays
    h.click("Continue");
    assert_eq!(login_page(&h), Some(LoginPage::Server));
    assert!(!h.app.sync_rt.form.note.is_empty());
    // then email and password on that server
    h.click("Server address entry");
    h.type_text("127.0.0.1:1");
    h.click("Continue");
    assert_eq!(login_page(&h), Some(LoginPage::SignIn));
    assert!(h.app.sync_rt.form.note.is_empty());
    assert!(
        h.has("Sign in with email")
            && h.has("Change")
            && h.has("Email entry")
            && h.has("Password entry")
            && h.has("Forgot password?")
    );
    h.click("Email entry");
    h.type_text("me@team.io");
    h.click("Password entry");
    h.type_text("secret-pass");
    h.settle();
    h.click("Sign in");
    // first it asks the server which workspaces that account is in
    let j = h.app.sync_rt.listing.as_ref().expect("looking up");
    assert_eq!(j.addr, "127.0.0.1:1");
    assert_eq!(
        j.how,
        together::How::Account {
            email: "me@team.io".into(),
            password: "secret-pass".into()
        }
    );
    assert!(h.has("Looking up your workspaces..."));
    assert!(
        !h.has("Use it on this Mac without an account") && !h.has("Set up a server for my team"),
        "the other ways wait while it connects"
    );
    h.click("Cancel");
    assert!(h.app.sync_rt.listing.is_none() && h.app.login.is_some());

    // an invite code instead, choosing a password; the email carries over
    h.click("Join with an invite code");
    assert_eq!(login_page(&h), Some(LoginPage::Invite));
    assert!(h.has("Invite code entry") && h.has("Choose a password entry"));
    assert_eq!(h.app.sync_rt.form.mode, together::Mode::Invite);
    assert_eq!(h.app.sync_rt.form.email, "me@team.io");
    h.click("Sign in with email");
    assert_eq!(login_page(&h), Some(LoginPage::SignIn));

    // another server
    h.click("Change");
    assert_eq!(login_page(&h), Some(LoginPage::Server));
    h.click("Continue");
    assert_eq!(login_page(&h), Some(LoginPage::SignIn), "the address stays");

    // a new server: its address, the owner's email and password
    h.click("Set up a server for my team");
    assert_eq!(login_page(&h), Some(LoginPage::NewServer));
    assert!(
        h.has("Set up your team's server")
            && h.has("Server address entry")
            && h.has("Repeat password entry")
            && !h.has("Invite code entry")
    );
    assert_eq!(
        h.app.sync_rt.form.email, "me@team.io",
        "the email carries over"
    );
    h.click("Back to sign in");
    assert_eq!(login_page(&h), Some(LoginPage::SignIn));

    // alone on this Mac: just a name, then the app
    h.click("Use it on this Mac without an account");
    assert!(h.has("What should we call you?") && h.has("Your name entry"));
    let me = h.app.data.me;
    h.app.login.as_mut().unwrap().name = "Jiwon".into();
    h.settle();
    h.click("Start");
    assert!(h.app.login.is_none());
    assert!(
        !h.app.data.extra.welcome,
        "no sign-in screen at the next start"
    );
    assert_eq!(h.app.data.member(me).unwrap().name, "Jiwon");
    assert!(h.has("Workspace menu"));
}

#[test]
fn the_sign_in_screen_starts_at_the_email_when_the_server_is_known() {
    let mut h = Ui1::new();
    sign_in_screen(&mut h, "wb.example.com");
    assert_eq!(login_page(&h), Some(super::login::LoginPage::SignIn));
    assert!(h.has("Email entry") && h.has("Change"));
}

#[test]
fn an_invite_link_opens_the_join_form_filled_in() {
    let mut h = Ui1::new();
    // working on this computer only: the team's workspace opens beside it
    h.app
        .open_join_link("workboard://join?server=macmini&code=ABCD-EFGH-JKMN");
    assert!(matches!(&h.app.modal, Some(Modal::JoinTeam { addr }) if addr == "macmini"));
    h.app.modal = None;
    // on the sign-in screen (a first start): its invite step, filled in
    sign_in_screen(&mut h, "");
    h.app.open_join_link("workboard://join?server=mac-mini.tail1.ts.net%3A8787&code=ABCD-EFGH-JKMN&email=bo%40team.io");
    h.settle();
    assert!(
        matches!(&h.app.login, Some(l) if l.addr == "mac-mini.tail1.ts.net:8787" && l.page == super::login::LoginPage::Invite)
    );
    assert!(h.app.modal.is_none());
    assert_eq!(h.app.sync_rt.form.mode, together::Mode::Invite);
    assert_eq!(
        (
            h.app.sync_rt.form.code.as_str(),
            h.app.sync_rt.form.email.as_str()
        ),
        ("ABCD-EFGH-JKMN", "bo@team.io")
    );
    assert!(h.has("Invite code entry") && h.has("Choose a password entry"));

    // a whole invite mail pasted into the code field fills in the server too
    h.app.login = Some(super::login::LoginScreen::new(
        super::login::LoginPage::Invite,
        "",
    ));
    h.app.sync_rt.form = together::Form {
        mode: together::Mode::Invite,
        ..Default::default()
    };
    h.settle();
    h.click("Invite code entry");
    h.type_text("Open this: workboard://join?server=macmini&code=WXYZ-2345-6789 thanks");
    h.settle();
    h.settle();
    assert_eq!(h.app.sync_rt.form.code, "WXYZ-2345-6789");
    assert!(matches!(&h.app.login, Some(l) if l.addr == "macmini"));
    h.app.login = None;

    // already on a server: the other team opens beside it, this one stays
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.toast = None;
    h.app
        .open_join_link("workboard://join?server=other&code=ABCD-EFGH-JKMN");
    assert!(matches!(&h.app.modal, Some(Modal::JoinTeam { addr }) if addr == "other"));
    assert_eq!(h.app.sync_rt.form.code, "ABCD-EFGH-JKMN");
    assert!(h.app.toast.is_none(), "no telling them to stop sharing");

    // this workspace's server let it go: the code signs it back in there
    h.app.sync_rt.net_status = Some(wb_net::Status::Refused("removed".into()));
    h.app
        .open_join_link("workboard://join?server=127.0.0.1:1&code=ABCD-EFGH-JKMN");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
}

#[test]
fn log_out_asks_first_then_deletes_the_team_copy_and_shows_the_sign_in_screen() {
    let mut h = Ui1::new();
    // working on this computer only: there is nothing to log out of
    h.click("Profile menu");
    assert!(h.has("Switch person") && !h.has("Log out"));
    h.key(egui::Key::Escape);

    let addr = "127.0.0.1:1";
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-out".into(),
        tag: 5,
        server: Some(addr.into()),
    });
    together::save_token(addr, "dev-out", "token-1");
    h.app.sync_rt.account = Some("me@team.io".into());
    let team_task = h.app.data.tasks[0].name.clone();
    h.click("Profile menu");
    h.click("Log out");
    assert!(matches!(h.app.modal, Some(Modal::LogOut)));
    assert!(h.has("Log out of 127.0.0.1:1?"));
    // not connected: it says what would be lost
    assert!(h.has("This computer isn't connected to the server right now. Changes you made while it was offline haven't gone up and will be lost."));
    h.click("Cancel");
    assert!(h.app.modal.is_none() && h.app.data.extra.shared.is_some());

    h.click("Profile menu");
    h.click("Log out");
    h.click("Log out");
    assert!(h.app.data.extra.shared.is_none());
    assert!(
        !h.app.data.tasks.iter().any(|t| t.name == team_task),
        "this computer's copy of the team workspace is gone"
    );
    assert!(together::load_token(addr, "dev-out").is_none());
    assert!(
        h.app.data.extra.welcome,
        "the sign-in screen again at the next start"
    );
    assert!(matches!(&h.app.login, Some(l) if l.addr == addr));
    assert_eq!(
        h.app.sync_rt.form.email, "me@team.io",
        "ready to sign in again"
    );
    assert!(h.has("Sign in with email") && h.has("Email entry") && !h.has("Profile menu"));
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Logged out")
    );
}

#[test]
fn signed_in_on_a_server_you_set_a_password_or_change_it() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert!(h.has("Your account") && h.has("Set a password to sign in on your other computers and phones without a new invite code."));
    h.click("Password entry");
    h.type_text("long-enough");
    h.click("Repeat password entry");
    h.type_text("long-enougH");
    h.settle();
    h.click("Save password");
    assert!(h.has("The two passwords aren't the same."));
    // with an account: change it (needs the current one)
    h.app.sync_rt.account = Some("me@team.io".into());
    h.app.sync_rt.acct = together::Form::default();
    h.settle();
    assert!(h.has("You sign in with me@team.io on your other computers and phones."));
    h.click("Change password");
    assert!(h.has("Current password entry") && h.has("New password entry"));
}

#[test]
fn password_boxes_show_on_the_eye_and_take_english_only() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    // every password box has its own eye
    assert_eq!(
        h.tree.iter().filter(|(n, _)| n == "Show password").count(),
        2
    );
    h.click("Password entry");
    h.type_text("ab");
    // Korean typing: neither the word being put together nor the result
    // goes in, and a note says to switch
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit("ㅎ".into()))]);
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
    h.type_text("가c1!");
    h.frame(vec![egui::Event::Paste("d 한e".into())]);
    h.settle();
    assert_eq!(h.app.sync_rt.acct.password, "abc1!d e");
    assert!(h.has("Passwords take English letters, numbers and symbols. Switch to English input."));
    // English again: the note goes
    h.type_text("x");
    h.settle();
    assert!(!h.has("Passwords take English letters, numbers and symbols. Switch to English input."));
    // the eye beside this box shows the text and hides it again, typing
    // carries on
    let entry = h
        .tree
        .iter()
        .find(|(n, _)| n == "Password entry")
        .unwrap()
        .1;
    let eye = h
        .tree
        .iter()
        .find(|(n, r)| n == "Show password" && entry.y_range().contains(r.center().y))
        .expect("the eye sits inside its box")
        .1;
    h.click_at(eye.center());
    assert!(h.has("Hide password") && !h.painted_text("abc1!d ex").is_empty());
    h.type_text("y");
    h.settle();
    assert_eq!(h.app.sync_rt.acct.password, "abc1!d exy");
    h.click("Hide password");
    assert!(h.painted_text("abc1!d exy").is_empty());
    assert_eq!(
        h.tree.iter().filter(|(n, _)| n == "Show password").count(),
        2
    );
}

#[test]
fn enter_in_the_middle_sends_without_a_line_break_and_shift_enter_keeps_one() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat_open.unwrap();
    h.click("Message composer");
    h.type_text("helloworld");
    for _ in 0..5 {
        h.key(egui::Key::ArrowLeft);
    }
    h.key(egui::Key::Enter);
    let last = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .text
        .clone();
    assert_eq!(last, "helloworld", "the cursor's spot gets no line break");
    // Shift+Enter is still a new line
    h.click("Message composer");
    h.type_text("one");
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: shift,
    }]);
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: shift,
    }]);
    h.type_text("two");
    assert_eq!(h.app.chat_text, "one\ntwo");
}

#[test]
fn others_never_see_a_private_channel_even_when_it_was_left_open() {
    let mut h = Ui1::new();
    let secret = h.app.create_channel("hush", true, false);
    h.settle();
    assert!(h.has("hush"));
    // Sam signs in on this computer with the private channel still open
    h.app.data.me = 3;
    h.app.section = Section::Chat;
    h.app.chat_open = Some(secret);
    h.settle();
    assert!(!h.has("hush"), "not in the sidebar");
    assert_ne!(
        h.app.chat_open,
        Some(secret),
        "the chat moved to a channel Sam may see"
    );
    // and my DM with Alex isn't Sam's either
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .clone();
    assert!(!h.app.listed(&dm));
}

#[test]
fn a_message_from_a_clock_ahead_is_read_once() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat_open.unwrap();
    let ahead = now_ts() + 3600;
    let id = h.app.data.new_id();
    let c = h
        .app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap();
    let mut m = c.messages[0].clone();
    m.id = id;
    m.author = 2;
    m.ts = ahead;
    c.messages.push(m);
    h.settle();
    let c = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    assert!(c.last_read >= ahead, "read up to the newest message");
    assert_eq!(c.messages.iter().filter(|m| m.ts > c.last_read).count(), 0);
}

#[test]
fn delete_channel_asks_first_and_close_conversation_only_hides() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    h.click("Delete channel");
    assert!(
        h.app.data.channels.iter().any(|c| c.id == cid),
        "one click doesn't delete"
    );
    h.click("Cancel");
    assert!(h.app.data.channels.iter().any(|c| c.id == cid));
    h.click("Delete channel");
    h.click("Delete");
    assert!(
        !h.app.data.channels.iter().any(|c| c.id == cid),
        "deleted after saying yes"
    );
    // a DM is only closed for me
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    h.app.modal = Some(Modal::ChannelSettings(dm));
    h.settle();
    h.click("Close conversation");
    assert!(h.app.data.channels.iter().any(|c| c.id == dm));
    assert!(!h.has("Alex Kim"));
}

#[test]
fn mentions_of_names_with_spaces_are_marked_whole() {
    let h = Ui1::new();
    let job = h
        .app
        .mention_text("hi @Alex Kim and @Sam Lee, also @someone", 14.0);
    let t = h.app.t();
    let marked: Vec<&str> = job
        .sections
        .iter()
        .filter(|s| s.format.background == t.selected_bg)
        .map(|s| &job.text[s.byte_range.clone()])
        .collect();
    assert_eq!(marked, vec!["@Alex Kim", "@Sam Lee", "@someone"]);
}

#[test]
fn cmd_brackets_go_back_and_forward() {
    let mut h = Ui1::new();
    let start = h.app.loc;
    h.click("Inbox");
    let cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    let press = |h: &mut Ui1, key| {
        h.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: cmd,
        }]);
        h.settle();
    };
    press(&mut h, egui::Key::OpenBracket);
    assert_eq!(h.app.loc, start);
    press(&mut h, egui::Key::CloseBracket);
    assert_eq!(h.app.loc, Location::Inbox);
}

#[test]
fn option_t_while_typing_and_enter_in_search() {
    let mut h = Ui1::new();
    h.app.palette = Some(String::new());
    h.settle();
    // Option+T types into the search box, it doesn't open Create Task
    let alt = egui::Modifiers {
        alt: true,
        ..Default::default()
    };
    h.frame(vec![egui::Event::Key {
        key: egui::Key::T,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: alt,
    }]);
    h.settle();
    assert!(h.app.quick_add.is_none());
    // a pasted task link opens the task on Enter
    let tid = h.app.data.tasks[0].id;
    h.app.palette = Some(format!("workboard://task/{}", tid));
    h.settle();
    h.key(egui::Key::Enter);
    h.settle();
    assert!(h.app.palette.is_none());
    assert_eq!(h.app.open_task, Some(tid));
    // Enter on a search that only finds a channel opens the channel
    h.app.open_task = None;
    let ch = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app.palette = Some("website-redesign".into());
    h.settle();
    h.key(egui::Key::Enter);
    h.settle();
    assert_eq!((h.app.section, h.app.chat_open), (Section::Chat, Some(ch)));
}

#[test]
fn search_leaves_out_other_peoples_personal_space_and_archived_lists() {
    let mut h = Ui1::new();
    let other = h.app.data.spaces.len() as u64 + 90_000;
    h.app.data.spaces.push(Space {
        id: other,
        name: "Someone Private".into(),
        private_owner: Some(2),
        lists: vec![ListItem {
            id: other + 1,
            name: "Their Secret List".into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    if let Some(l) = h.app.data.spaces[0].lists.first_mut() {
        l.name = "Old Archived List".into();
        l.archived = true;
    }
    h.app.palette = Some(String::new());
    h.settle();
    // the palette names lists by their path
    assert!(h.has("Team Space / Projects / Marketing"));
    assert!(
        !h.has("Someone Private")
            && !h.has("Someone Private / Their Secret List")
            && !h.has("Team Space / Old Archived List")
    );
}

#[test]
fn a_deleted_channel_goes_to_trash_and_comes_back_with_its_messages() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    let messages = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .len();
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    h.click("Delete channel");
    h.click("Delete");
    assert!(!h.app.data.channels.iter().any(|c| c.id == cid));
    assert!(h
        .app
        .data
        .extra
        .trash
        .iter()
        .any(|t| t.kind == "Channel" && t.title == "#website-redesign"));
    h.app.modal = Some(Modal::Trash);
    h.settle();
    h.click("Restore");
    let back = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .expect("restored");
    assert_eq!(back.messages.len(), messages);
}

#[test]
fn someone_elses_channel_can_be_joined_but_not_deleted_or_its_people_changed() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .creator = 2;
    // a plain member (an owner or admin may manage anyone's channel)
    let me = h.app.data.me;
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == me)
        .unwrap()
        .role = "Member".into();
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    // Alex Kim (2) is in it and Sam Lee (3) isn't: neither checkbox changes
    h.click("Sam Lee");
    h.click("Alex Kim");
    assert_eq!(
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .members,
        vec![1, 2]
    );
    h.click("Delete channel");
    assert!(!h.has("Delete"), "the delete button is off");
    assert!(h.app.data.channels.iter().any(|c| c.id == cid));
}

#[test]
fn saved_messages_from_every_channel_open_from_the_sidebar() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages[0]
        .saved = true;
    h.settle();
    h.click("Saved messages");
    assert!(matches!(h.app.modal, Some(Modal::SavedMessages)));
    h.click("Saved message");
    assert_eq!((h.app.section, h.app.chat_open), (Section::Chat, Some(cid)));
    assert!(h.app.modal.is_none());
}

#[test]
fn add_channel_doesnt_open_someone_elses_private_channel() {
    let mut h = Ui1::new();
    let list = h
        .app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .find(|l| l.name == "Marketing")
        .unwrap()
        .id;
    let secret = h.app.data.new_id();
    h.app.data.channels.push(Channel {
        id: secret,
        name: "marketing".into(),
        kind: ChannelKind::Channel,
        members: vec![2],
        private: true,
        ..Default::default()
    });
    h.app.apply(Action::Go(Location::List(list)));
    h.settle();
    h.click("Add Channel");
    let open = h.app.chat_open.unwrap();
    assert_ne!(open, secret);
    assert!(h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == open)
        .is_some_and(|c| c.name == "marketing" && !c.private));
}

#[test]
fn timesheet_drawing_a_long_day_keeps_its_entries() {
    let mut h = Ui1::new();
    let td = today();
    let ws = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
    let start = td
        .and_hms_opt(0, 0, 0)
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
        .position(|t| t.name == "Renew passport")
        .unwrap();
    // 30 hours on one day (e.g. typed on another computer): more than a cell's 24h
    let long = vec![TimeEntry {
        start,
        end: start + 30 * 3600,
        member: 1,
    }];
    h.app.data.tasks[i].time_entries = long.clone();
    h.app.section = Section::Timesheets;
    h.app.hub_filter = 0;
    h.app.sheet_week = ws;
    h.app.sheet_entries = false;
    h.app.dirty = false;
    h.settle();
    assert!(h.has("Remove row Renew passport"), "the grid is drawn");
    let e = &h.app.data.tasks[i].time_entries;
    assert_eq!(
        (e.len(), e[0].start, e[0].end),
        (1, long[0].start, long[0].end),
        "drawing the grid changed nothing"
    );
    assert!(!h.app.dirty);
    // no Approvals tab: approving weeks isn't built
    assert!(h.has("All timesheets") && !h.has("Approvals"));
}

#[test]
fn timesheet_rows_added_by_hand_stay_in_their_week() {
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
    h.app.sheet_force = true;
    h.settle();
    h.click("+ Add task");
    h.click("Renew passport");
    assert!(h.has("Remove row Renew passport"));
    h.click("Next period");
    assert_eq!(h.app.sheet_week, ws + Duration::days(7));
    assert!(!h.has("Remove row Renew passport"), "not in the next week");
    h.click("Previous period");
    assert!(h.has("Remove row Renew passport"));
    h.click("Remove row Renew passport");
    assert!(!h.has("Remove row Renew passport"), "the x takes it off");
}

#[test]
fn ownerless_old_timer_can_be_stopped_from_the_task() {
    let mut h = Ui1::new();
    let id = h.task_id("Write copy for landing page");
    let started = now_ts() - 600;
    let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
    t.timer_started = Some(started);
    t.timer_by = None;
    h.app.apply(Action::Open(id));
    h.settle();
    // it isn't mine, so my play button starts my own timer beside it
    h.click("Start timer");
    let t = task_named(&h, "Write copy for landing page");
    assert_eq!(t.timer_started, Some(started));
    assert!(h.app.data.is_my_timer(t));
    h.click("Stop (started by an older app)");
    let t = task_named(&h, "Write copy for landing page");
    assert!(
        t.time_entries
            .iter()
            .any(|e| e.start == started && e.member == 0),
        "its time is kept"
    );
    assert!(h.app.data.is_my_timer(t), "my timer keeps running");
    assert!(!h.has("Stop (started by an older app)"));
}

#[test]
fn my_subtask_shows_once_on_my_tasks() {
    let mut h = Ui1::new();
    let list = h.app.data.first_list().unwrap();
    let me = h.app.data.me;
    h.app.apply(Action::Add {
        list,
        name: "Parent of mine".into(),
        status: None,
        priority: 0,
        assignees: vec![me],
        due: None,
        parent: None,
        open: false,
    });
    let parent = h.task_id("Parent of mine");
    h.app.apply(Action::Add {
        list,
        name: "Sub of mine".into(),
        status: None,
        priority: 0,
        assignees: vec![me],
        due: None,
        parent: Some(parent),
        open: false,
    });
    h.app.loc = Location::MyTasks;
    h.app.subtasks_open = true;
    h.settle();
    h.scroll_into_view("Task Sub of mine");
    assert_eq!(
        h.tree
            .iter()
            .filter(|(n, _)| n == "Task Sub of mine")
            .count(),
        1
    );
}

#[test]
fn whiteboard_undo_puts_a_moved_note_back() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    wb.notes.push(Sticky {
        id: 0,
        pos: [60.0, 60.0],
        text: "One".into(),
        color: [0xf8, 0xae, 0x00],
    });
    h.app.wb_tool = 2;
    h.settle();
    // press near the note's bottom edge and drag it
    let x = rect_of(&h, "Delete note 0");
    let a = pos2(x.center().x - 60.0, x.center().y + 100.0);
    h.frame(vec![egui::Event::PointerMoved(a)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: a,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    for k in 1..=10 {
        h.frame(vec![egui::Event::PointerMoved(
            a + vec2(10.0 * k as f32, 8.0 * k as f32),
        )]);
    }
    h.frame(vec![egui::Event::PointerButton {
        pos: a + vec2(100.0, 80.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    h.settle();
    let pos = |h: &Ui1| {
        h.app
            .data
            .extra
            .whiteboards
            .iter()
            .find(|w| w.id == wid)
            .unwrap()
            .notes[0]
            .pos
    };
    assert_ne!(pos(&h), [60.0, 60.0], "the note moved");
    h.click("Undo");
    assert_eq!(pos(&h), [60.0, 60.0], "undo puts it back");
}

#[test]
fn header_automate_opens_for_the_current_list_and_manage_people_opens_settings() {
    let mut h = Ui1::new();
    let lid = h.app.data.first_list().unwrap();
    h.app.loc = Location::List(lid);
    h.settle();
    h.click("Automate");
    assert!(
        matches!(h.app.modal, Some(Modal::Automations { scope: Some(Location::List(l)), .. }) if l == lid),
        "new rules run in this list"
    );
    h.app.modal = None;
    h.app.section = Section::Teams;
    h.app.hub_filter = 3;
    h.settle();
    h.click("Manage people");
    assert_eq!(h.app.section, Section::Settings);
    assert_eq!(
        super::settings::SETTINGS[h.app.settings_tab as usize].1,
        "People"
    );
}

#[test]
fn a_link_from_macos_opens_in_the_frame_it_arrives() {
    let mut h = Ui1::new();
    let id = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| h.app.task_visible(t))
        .map(|t| t.id)
        .unwrap();
    assert_eq!(h.app.open_task, None);
    // macOS hands the link over between frames (no click or key follows)
    crate::links::arrived(format!("workboard://task/{}", id));
    h.frame(vec![]);
    assert_eq!(
        h.app.open_task,
        Some(id),
        "the link opened without waiting for more input"
    );
    assert!(
        h.ctx.has_requested_repaint(),
        "the app draws again to show it"
    );
}

#[test]
fn sidebar_leaves_out_other_peoples_private_docs() {
    let mut h = Ui1::new();
    let sid = h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.title == "Project brief")
        .and_then(|d| d.space)
        .unwrap();
    for s in &mut h.app.data.spaces {
        s.expanded = s.id == sid;
    }
    h.settle();
    assert!(h.has("Project brief"), "the space's doc is in the sidebar");
    let me = h.app.data.me;
    let d = h
        .app
        .data
        .docs
        .iter_mut()
        .find(|d| d.title == "Project brief")
        .unwrap();
    d.private = true;
    d.owner = me + 100;
    h.settle();
    assert!(
        !h.has("Project brief"),
        "someone else's Private doc stays out of the space tree"
    );
    // and out of the sidebar search
    h.app.side_search = Some("brief".into());
    h.settle();
    assert!(!h.has("Project brief"));
    // my own Private doc stays
    h.app.side_search = None;
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.title == "Project brief")
        .unwrap()
        .owner = me;
    h.settle();
    assert!(h.has("Project brief"));
}

#[test]
fn mention_picked_from_the_list_keeps_that_person_when_names_are_shared() {
    let mut h = Ui1::new();
    h.app.data.members.push(Member {
        id: 9,
        name: "Alex Kim".into(),
        color: [1, 2, 3],
        ..Default::default()
    });
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.unwrap();
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .activity
        .retain(|a| !a.comment);
    h.settle();
    h.click("@  Mention a teammate");
    // the second "Alex Kim" in the list (the one just added) is the lower row
    // (the list's rows are 200 points wide)
    let rows: Vec<Pos2> = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Alex Kim" && r.width() == 200.0)
        .map(|(_, r)| r.center())
        .collect();
    assert!(rows.len() >= 2, "both people are in the list: {:?}", rows);
    let lowest = rows
        .iter()
        .copied()
        .fold(rows[0], |a, b| if b.y > a.y { b } else { a });
    h.click_at(lowest);
    assert_eq!(h.app.comment_text, "@Alex Kim ");
    h.type_text("this one");
    h.click("Send Comment");
    let c = h
        .app
        .task(id)
        .unwrap()
        .activity
        .iter()
        .rev()
        .find(|a| a.comment)
        .unwrap()
        .clone();
    assert_eq!(c.text, "@Alex Kim this one");
    assert_eq!(
        c.mentions,
        vec![9],
        "only the person picked, not both Alex Kims"
    );
}

#[test]
fn click_notification_settings_turn_kinds_off_and_set_channels() {
    let mut h = Ui1::new();
    h.click("Profile menu");
    h.click("Notifications");
    assert!(matches!(h.app.modal, Some(Modal::Notifications)));
    assert!(h.app.data.extra.notify.direct);
    h.click("Direct messages");
    assert!(!h.app.data.extra.notify.direct);
    // rows past the dialog's scroll area are laid out but clipped: bring
    // this one up before clicking it
    let over = h.find("Direct messages").unwrap();
    h.frame(vec![egui::Event::PointerMoved(over)]);
    h.frame(vec![egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -120.0),
        modifiers: Default::default(),
    }]);
    h.settle();
    h.click("Reactions to my messages");
    assert!(h.app.data.extra.notify.reactions);
    // the dialog scrolls: wheel down over it to reach the channel choices
    let over = h.find("Direct messages").unwrap();
    for _ in 0..6 {
        h.frame(vec![egui::Event::PointerMoved(over)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -120.0),
            modifiers: Default::default(),
        }]);
    }
    h.settle();
    h.click("Every new message");
    assert_eq!(h.app.data.extra.notify.channel_default, "all");
    h.click("Send a test notification");
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|t| t.0.starts_with("Sent a test notification")));
}

#[test]
fn click_the_channel_bell_to_choose_what_it_notifies() {
    let mut h = Ui1::new();
    let g = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    h.app.section = Section::Chat;
    h.app.chat_open = Some(g);
    h.settle();
    h.click("Channel notifications");
    h.click("Every new message");
    assert_eq!(h.app.data.extra.notify.channel_mode(g), "all");
    h.click("Channel notifications");
    h.click("Nothing");
    assert_eq!(h.app.data.extra.notify.channel_mode(g), "nothing");
}

#[test]
fn click_a_notification_toast_to_open_the_conversation() {
    let mut h = Ui1::new();
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .id;
    // a teammate's message arrives, as a merge would bring it
    let id = h.app.data.new_id();
    let m: Message = serde_json::from_value(
        serde_json::json!({"id": id, "author": 2, "text": "Got a minute?", "ts": now_ts()}),
    )
    .unwrap();
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == dm)
        .unwrap()
        .messages
        .push(m);
    h.app.notify_rt.pending = true;
    h.settle();
    let toast = h.app.toast.clone().expect("a toast for the new message").0;
    assert!(toast.contains("Got a minute?"), "{}", toast);
    assert_ne!(h.app.section, Section::Chat);
    h.click(&format!("Open {}", toast));
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat_open, Some(dm));
}

// ---- wikis, doc links and the Docs hub list (r3-work)

fn doc_id(h: &Ui1, title: &str) -> u64 {
    h.app
        .data
        .docs
        .iter()
        .find(|d| d.title == title)
        .map(|d| d.id)
        .expect("doc exists")
}

#[test]
fn docs_marked_as_wiki_show_in_popular_wikis_and_the_wikis_tab() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let a = h.app.doc_open.unwrap();
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == a)
        .unwrap()
        .title = "Handbook".into();
    h.app.new_doc(None);
    let b = h.app.doc_open.unwrap();
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == b)
        .unwrap()
        .title = "Onboarding".into();
    h.settle();
    // opening a doc counts one view, not one per frame
    let views = |h: &Ui1, id: u64| {
        h.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .views_since(0)
    };
    assert_eq!(views(&h, b), 1);
    h.settle();
    assert_eq!(views(&h, b), 1);
    // mark both as Wikis from the doc menu
    h.click("Doc options");
    h.click("Mark as Wiki");
    assert!(h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.id == b)
        .unwrap()
        .is_wiki());
    assert!(h.has("Wiki badge"), "the doc header shows the Wiki badge");
    h.app.doc_open = Some(a);
    h.settle();
    h.click("Doc options");
    h.click("Mark as Wiki");
    // open the handbook twice more: it becomes the most popular
    for _ in 0..2 {
        h.app.doc_open = None;
        h.settle();
        h.app.doc_open = Some(a);
        h.settle();
    }
    assert_eq!(views(&h, a), 3);
    let order: Vec<u64> = h.app.popular_wikis(now_ts()).iter().map(|w| w.0).collect();
    assert_eq!(order, vec![a, b]);
    assert!(h.has("Popular Wiki Handbook") && h.has("Popular Wiki Onboarding"));
    // the card opens a wiki
    h.click("Popular Wiki Onboarding");
    assert_eq!(h.app.doc_open, Some(b));
    // the Wikis tab lists only wikis
    h.open_docs_list();
    h.click("Wikis");
    assert!(h.has("Doc Handbook") && h.has("Doc Onboarding"));
    assert!(!h.has("Doc Project brief"));
    // unmarking takes it out (and writes false, never back to missing)
    h.right_click("Doc Onboarding", 60.0);
    h.click("Unmark as Wiki");
    let d = h.app.data.docs.iter().find(|d| d.id == b).unwrap();
    assert_eq!(d.wiki, Some(false));
    h.settle();
    assert!(!h.has("Doc Onboarding") && !h.has("Popular Wiki Onboarding"));
}

#[test]
fn doc_views_and_wiki_flag_round_trip_and_old_docs_still_load() {
    let old: Doc =
        serde_json::from_str(r#"{"id":5,"title":"T","body":"","created":0,"updated":0}"#).unwrap();
    assert!(!old.is_wiki() && old.views.is_empty());
    let out = serde_json::to_value(&old).unwrap();
    assert!(out.get("wiki").is_none() && out.get("views").is_none());
    let mut a = App::build(None, Some(Data::seed()));
    let id = a.data.docs[0].id;
    a.count_doc_view(id, 1000);
    a.count_doc_view(id, 1000 + DOC_VIEW_DAYS * 86_400 + 5);
    let d = a.data.docs[0].clone();
    // the first open fell out of the 30 days
    assert_eq!(d.views.len(), 1);
    assert_eq!(d.views[0].at.len(), 1);
    let back: Doc = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
    assert_eq!(back.views, d.views);
    // a duplicate starts unread, so no view entry id is used twice
    let copy = a.duplicate_doc(id).unwrap();
    assert!(a
        .data
        .docs
        .iter()
        .find(|d| d.id == copy)
        .unwrap()
        .views
        .is_empty());
}

#[test]
fn doc_copy_link_opens_the_doc_from_a_comment_chat_or_search() {
    let mut h = Ui1::new();
    let id = doc_id(&h, "Project brief");
    let url = format!("workboard://doc/{}", id);
    assert_eq!(
        super::nav::parse_link(&url),
        Some(super::nav::Link::Doc(id))
    );
    // comments read links with the same parser
    assert_eq!(
        super::comments::parse_link(&format!("{}, see", url)).map(|x| x.0),
        Some(super::nav::Link::Doc(id))
    );
    assert!(h.app.open_link(&url));
    h.settle();
    assert_eq!((h.app.section, h.app.doc_open), (Section::Docs, Some(id)));
    // a doc in someone else's Personal space is not opened
    h.app.section = Section::Home;
    h.app.doc_open = None;
    let sid = h.app.data.spaces[0].id;
    h.app.data.spaces[0].private_owner = Some(h.app.data.me + 1000);
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .space = Some(sid);
    assert!(!h.app.open_link(&url));
    assert_eq!(h.app.doc_open, None);
    // search finds the doc from its pasted link
    h.app.data.spaces[0].private_owner = None;
    let rows = h.app.search_rows(&url, 0);
    assert!(rows
        .iter()
        .any(|r| matches!(r.4, super::more::Hit::Doc(x) if x == id)));
}

#[test]
fn docs_hub_leaves_out_docs_in_archived_or_other_peoples_spaces() {
    let mut h = Ui1::new();
    let id = doc_id(&h, "Project brief");
    h.open_docs_list();
    assert!(h.has("Doc Project brief"));
    let sid = h.app.data.spaces[0].id;
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .space = Some(sid);
    h.app.data.spaces[0].archived = true;
    h.settle();
    assert!(!h.has("Doc Project brief"), "archived space");
    h.app.data.spaces[0].archived = false;
    h.app.data.spaces[0].private_owner = Some(h.app.data.me + 1000);
    h.settle();
    assert!(!h.has("Doc Project brief"), "someone else's Personal space");
    h.app.data.spaces[0].private_owner = None;
    h.settle();
    assert!(h.has("Doc Project brief"));
}

// ---- the status editor: renames reach automations once done, and closing
// never leaves tasks on a missing status (r3-work)

#[test]
fn status_editor_retargets_automations_only_when_a_rename_is_done() {
    let mut h = Ui1::new();
    let sid = h.app.data.spaces[0].id;
    h.app.data.spaces[0]
        .statuses
        .insert(1, Status::new("triage", [1, 2, 3], StatusKind::Custom));
    h.app.data.spaces[1]
        .statuses
        .insert(1, Status::new("review", [1, 2, 3], StatusKind::Custom));
    let list = h.app.data.lists_in(Location::Space(sid)).unwrap()[0];
    let tid = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.list_id == list)
        .unwrap()
        .id;
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == tid)
        .unwrap()
        .status = "triage".into();
    let rule = |id, s: &str| Automation {
        id,
        enabled: true,
        trigger_status: s.into(),
        action: "priority".into(),
        value: "1".into(),
        list: None,
    };
    h.app.data.extra.automations = vec![rule(1, "review"), rule(2, "triage")];
    h.app.edit_statuses = Some(sid);
    h.settle();
    let status = |h: &Ui1| h.app.task(tid).unwrap().status.clone();
    let triggers = |h: &Ui1| {
        h.app
            .data
            .extra
            .automations
            .iter()
            .map(|a| a.trigger_status.clone())
            .collect::<Vec<_>>()
    };
    // type a name through the other space's "review": tasks follow live
    h.click("triage");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("review");
    assert_eq!(status(&h), "review");
    assert_eq!(
        triggers(&h),
        ["review", "triage"],
        "rules wait for the rename"
    );
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("reviewed");
    assert_eq!(status(&h), "reviewed");
    h.key(egui::Key::Enter);
    assert_eq!(
        triggers(&h),
        ["review", "reviewed"],
        "the other space's rule stays; this status's rule follows to the final name"
    );
}

#[test]
fn closing_the_status_editor_puts_back_an_empty_or_taken_name() {
    let mut h = Ui1::new();
    let sid = h.app.data.spaces[0].id;
    let list = h.app.data.lists_in(Location::Space(sid)).unwrap()[0];
    let tid = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.list_id == list)
        .unwrap()
        .id;
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == tid)
        .unwrap()
        .status = "in progress".into();
    h.app.edit_statuses = Some(sid);
    h.settle();
    // "in progress" typed into "to do": taken, so its tasks wait
    h.click("in progress");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("to do");
    assert_eq!(h.app.task(tid).unwrap().status, "in progress");
    h.click("Close window");
    assert!(h.app.edit_statuses.is_none());
    let names: Vec<String> = h.app.data.spaces[0]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert!(names.contains(&"in progress".to_string()), "{:?}", names);
    let st = h.app.task(tid).unwrap().status.clone();
    assert!(names.contains(&st), "the task's status exists: {}", st);
    assert!(h.app.toast.is_some());
    // an empty name goes back too
    h.app.edit_statuses = Some(sid);
    h.settle();
    h.click("in progress");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.click("Close window");
    assert!(h.app.data.spaces[0]
        .statuses
        .iter()
        .any(|s| s.name == "in progress"));
    assert_eq!(h.app.task(tid).unwrap().status, "in progress");
}

// ---- whiteboard pan and zoom (r3-work)

fn drag_with(h: &mut Ui1, from: Pos2, to: Pos2, button: egui::PointerButton) {
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: from,
        button,
        pressed: true,
        modifiers: Default::default(),
    }]);
    for i in 1..=12 {
        h.frame(vec![egui::Event::PointerMoved(
            from + (to - from) * (i as f32 / 12.0),
        )]);
    }
    h.frame(vec![egui::Event::PointerButton {
        pos: to,
        button,
        pressed: false,
        modifiers: Default::default(),
    }]);
    h.settle();
}

#[test]
fn whiteboard_pans_and_zooms_and_draws_in_board_coordinates() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let board = |h: &Ui1| {
        h.app
            .data
            .extra
            .whiteboards
            .iter()
            .find(|w| w.id == wid)
            .cloned()
            .unwrap()
    };
    // a note far beyond the window, like a big template's
    h.app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap()
        .notes
        .push(Sticky {
            id: 0,
            pos: [2400.0, 1600.0],
            text: "Far".into(),
            color: [0xf8, 0xae, 0x00],
        });
    h.settle();
    assert!(!h.has("Delete note 0"), "the far note is off screen");
    let canvas = rect_of(&h, "Whiteboard canvas");
    let view = |h: &Ui1| h.app.sweep.wb_view.get(&wid).copied().unwrap_or_default();
    // two-finger scroll moves the view until the note shows
    h.frame(vec![egui::Event::PointerMoved(canvas.center())]);
    for _ in 0..40 {
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(-7.0, -5.0) * 10.0,
            modifiers: Default::default(),
        }]);
        h.settle();
        if h.has("Delete note 0") {
            break;
        }
    }
    assert!(h.has("Delete note 0"), "scrolling reaches the far note");
    assert!(view(&h).pan.x < -1000.0);
    // Reset view comes back
    h.click("Reset view");
    assert_eq!(view(&h), super::more::WbView::default());
    assert!(!h.has("Delete note 0"));
    // the buttons zoom around the canvas centre
    h.click("Zoom in");
    assert!((view(&h).zoom - 1.25).abs() < 1e-3);
    h.click("Zoom out");
    assert!((view(&h).zoom - 1.0).abs() < 1e-3);
    // Cmd+scroll zooms at the pointer
    let at = canvas.center() + vec2(100.0, 50.0);
    h.frame(vec![egui::Event::PointerMoved(at)]);
    let before = view(&h).to_board(canvas.min, at);
    h.frame(vec![egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, 5.0),
        modifiers: egui::Modifiers::COMMAND,
    }]);
    h.settle();
    let v = view(&h);
    assert!(v.zoom > 1.0, "zoomed in: {}", v.zoom);
    let after = v.to_board(canvas.min, at);
    assert!((before[0] - after[0]).abs() < 0.5 && (before[1] - after[1]).abs() < 0.5);
    // a pen stroke is stored in board coordinates
    h.app.wb_tool = 0;
    let from = canvas.center();
    let to = from + vec2(80.0, 40.0);
    drag_by(&mut h, from, to);
    let s = board(&h).strokes.last().cloned().expect("a stroke");
    let want = v.to_board(canvas.min, from);
    assert!((s.points[0][0] - want[0]).abs() < 2.0 && (s.points[0][1] - want[1]).abs() < 2.0);
    // the middle button pans and doesn't draw
    let n = board(&h).strokes.len();
    let pan = view(&h).pan;
    drag_with(
        &mut h,
        from,
        from + vec2(60.0, 30.0),
        egui::PointerButton::Middle,
    );
    assert_eq!(board(&h).strokes.len(), n);
    assert!((view(&h).pan - pan - vec2(60.0, 30.0)).length() < 2.0);
    // Space + drag pans too
    let pan = view(&h).pan;
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    drag_with(
        &mut h,
        from,
        from - vec2(50.0, 20.0),
        egui::PointerButton::Primary,
    );
    h.frame(vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Default::default(),
    }]);
    h.settle();
    assert_eq!(board(&h).strokes.len(), n, "Space + drag doesn't draw");
    assert!((view(&h).pan - pan + vec2(50.0, 20.0)).length() < 2.0);
}

#[test]
fn whiteboard_view_keeps_the_anchor_point_when_zooming() {
    use super::more::WbView;
    let o = pos2(100.0, 50.0);
    let mut v = WbView::default();
    let anchor = pos2(400.0, 300.0);
    let b = v.to_board(o, anchor);
    v.zoom_at(o, anchor, 2.0);
    assert_eq!(v.zoom, 2.0);
    let b2 = v.to_board(o, anchor);
    assert!((b[0] - b2[0]).abs() < 1e-3 && (b[1] - b2[1]).abs() < 1e-3);
    v.zoom_at(o, anchor, 100.0);
    assert_eq!(v.zoom, WbView::MAX_ZOOM);
    let p = v.to_screen(o, [10.0, 20.0]);
    let back = v.to_board(o, p);
    assert!((back[0] - 10.0).abs() < 1e-3 && (back[1] - 20.0).abs() < 1e-3);
}

// ---- custom field columns in the List view (r3-work)

#[test]
fn list_view_shows_custom_field_columns_edits_them_in_place_and_remembers_them() {
    let mut h = Ui1::new();
    h.app.data.extra.apps.custom_fields = true;
    let field = |id, name: &str, kind: &str, options: &[&str]| CustomField {
        id,
        name: name.into(),
        kind: kind.into(),
        options: options.iter().map(|s| s.to_string()).collect(),
    };
    h.app.data.extra.fields = vec![
        field(9001, "Billable", "checkbox", &[]),
        field(9002, "Size", "dropdown", &["S", "M", "L"]),
    ];
    let name = "Write copy for landing page";
    let tid = task_named(&h, name).id;
    let list = task_named(&h, name).list_id;
    h.app.apply(Action::Go(Location::List(list)));
    h.app.view = ViewKind::List;
    h.settle();
    // make room for two field columns next to the name
    h.app.hidden_cols.insert("Comments".into());
    h.app.hidden_cols.insert("Assignee".into());
    h.settle();
    let custom = |h: &Ui1, f: u64| h.app.task(tid).unwrap().custom.get(&f).cloned();
    // a checkbox ticks in place, without opening the task
    h.click(&format!("Billable of {}", name));
    assert_eq!(custom(&h, 9001).as_deref(), Some("true"));
    assert!(h.app.open_task.is_none());
    // a dropdown opens its options
    h.click(&format!("Size of {}", name));
    h.click("M");
    assert_eq!(custom(&h, 9002).as_deref(), Some("M"));
    // the Columns panel turns a field column off; the list remembers it
    h.click("Columns");
    assert!(h.has("Custom fields"));
    h.click("Field Billable");
    assert!(h.app.hidden_cols.contains("field:9001"));
    h.key(egui::Key::Escape);
    assert!(!h.has(&format!("Billable of {}", name)));
    let other = list_of_other(&h, list);
    h.app.apply(Action::Go(Location::List(other)));
    h.settle();
    h.app.hidden_cols.remove("field:9001");
    h.settle();
    h.app.apply(Action::Go(Location::List(list)));
    h.settle();
    assert!(
        h.app.hidden_cols.contains("field:9001"),
        "the first list still hides it"
    );
    // text and number fields open a box; a number takes only numbers
    h.app.data.extra.fields = vec![
        field(9003, "Notes", "text", &[]),
        field(9004, "Points", "number", &[]),
    ];
    h.settle();
    h.click(&format!("Notes of {}", name));
    h.type_text("hello");
    assert_eq!(custom(&h, 9003).as_deref(), Some("hello"));
    h.key(egui::Key::Enter);
    h.click(&format!("Points of {}", name));
    h.type_text("abc");
    assert_eq!(custom(&h, 9004), None);
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("5");
    assert_eq!(custom(&h, 9004).as_deref(), Some("5"));
    // with the Custom Fields ClickApp off the columns go away
    h.key(egui::Key::Enter);
    h.app.data.extra.apps.custom_fields = false;
    h.settle();
    assert!(!h.has(&format!("Notes of {}", name)));
}

fn list_of_other(h: &Ui1, not: u64) -> u64 {
    h.app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .map(|l| l.id)
                .chain(s.folders.iter().flat_map(|f| f.lists.iter().map(|l| l.id)))
        })
        .find(|l| *l != not)
        .unwrap()
}

#[test]
fn a_shared_folder_says_private_items_are_readable_from_its_files() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Work together");
    assert!(h.has(together::FOLDER_PRIVACY), "said before sharing");
    let dir = std::env::temp_dir().join(format!("wb-folder-note-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: dir.to_string_lossy().into(),
        device: "dev1".into(),
        tag: 5,
        server: None,
    });
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert!(h.has("This workspace is shared"));
    assert!(h.has(together::FOLDER_PRIVACY), "and while sharing");
    // on a server, other people's copies leave private items out: no note
    h.app.data.extra.shared.as_mut().unwrap().server = Some("127.0.0.1:1".into());
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert!(!h.has(together::FOLDER_PRIVACY));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn comment_links_to_private_or_archived_places_do_not_open() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    let me = h.app.data.me;
    let task_list = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.id == id)
        .unwrap()
        .list_id;
    // a list in another space, made someone else's Personal space
    let (sid, lid) = h
        .app
        .data
        .spaces
        .iter()
        .filter(|s| s.lists.iter().all(|l| l.id != task_list))
        .find_map(|s| s.lists.first().map(|l| (s.id, l.id)))
        .unwrap();
    h.app
        .data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .private_owner = Some(me + 100);
    let link = format!("workboard://list/{}", lid);
    let folder_link = "workboard://folder/987654".to_string();
    {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.activity.push(Activity {
            ts: now_ts(),
            author: 2,
            text: format!("see {}\nand {}", link, folder_link),
            comment: true,
            id: 77_002,
            ..Default::default()
        });
    }
    h.settle();
    let loc = h.app.loc;
    h.click(&link);
    assert_eq!(h.app.open_task, Some(id), "the task stays open");
    assert_eq!(h.app.loc, loc);
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("archived or private")));
    // a folder link is a link too, and one this workspace doesn't have says so
    h.app.toast = None;
    h.click(&folder_link);
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("can't be opened")));
    // my own Personal space opens like a link in chat: task window closed
    h.app
        .data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .private_owner = Some(me);
    h.click(&link);
    assert_eq!(h.app.open_task, None);
    assert_eq!(h.app.loc, Location::List(lid));
}

#[test]
fn an_owner_without_a_server_manages_someone_elses_channel() {
    let mut h = Ui1::new();
    let cid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "website-redesign")
        .unwrap()
        .id;
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .creator = 2;
    h.app.modal = Some(Modal::ChannelSettings(cid));
    h.settle();
    // the Sam Lee checkbox in the dialog, not a name on the page behind it
    let dialog = h.modal_rect();
    let sam = h.rect_in("Sam Lee", dialog);
    h.click_at(sam.center());
    assert!(h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .members
        .contains(&3));
}

#[test]
fn an_admin_sees_code_buttons_only_for_members_without_a_password() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    let me = h.app.data.me;
    let others: Vec<u64> = h
        .app
        .data
        .members
        .iter()
        .map(|m| m.id)
        .filter(|id| *id != me)
        .take(3)
        .collect();
    let person = |member, role, account| wb_net::Person {
        member,
        role,
        devices: 0,
        invite: None,
        account,
    };
    h.app.sync_rt.people = vec![
        person(me, wb_net::Role::Admin, true),
        person(others[0], wb_net::Role::Admin, false),
        person(others[1], wb_net::Role::Member, true),
        person(others[2], wb_net::Role::Member, false),
    ];
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    let codes = |h: &Ui1| h.tree.iter().filter(|(n, _)| n == "Invite code").count();
    assert_eq!(codes(&h), 1, "only the member without a password");
    // the owner makes codes for everyone else
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.app.sync_rt.people[0].role = wb_net::Role::Owner;
    h.settle();
    assert_eq!(codes(&h), 3);
}

#[test]
fn a_reply_notice_from_a_duplicate_dm_copy_opens_its_thread() {
    let mut h = Ui1::new();
    let dm = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Direct && c.members == vec![1, 2])
        .unwrap()
        .clone();
    // the shown copy has a message; a second copy (made on another
    // computer at the same time) holds a thread with a reply
    let first = h.app.data.new_id();
    let mut primary = dm.clone();
    primary.messages.push(
        serde_json::from_value(
            serde_json::json!({"id": first, "author": 1, "text": "hello", "ts": now_ts() - 100}),
        )
        .unwrap(),
    );
    let pid = primary.id;
    *h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == pid)
        .unwrap() = primary;
    let dup_id = h.app.data.new_id();
    let parent = h.app.data.new_id();
    let reply = h.app.data.new_id();
    let mut dup = dm.clone();
    dup.id = dup_id;
    dup.messages = vec![serde_json::from_value(serde_json::json!({
        "id": parent, "author": 2, "text": "question in the other copy", "ts": now_ts() - 50,
        "replies": [{"id": reply, "author": 2, "text": "the answer", "ts": now_ts() - 10}]
    }))
    .unwrap()];
    h.app.data.channels.push(dup);
    h.app.open_target(&format!("chat:{}:{}", dup_id, parent));
    h.settle();
    assert_eq!(h.app.chat_open, Some(pid), "the shown copy opens");
    assert_eq!(
        h.app.thread_open,
        Some(parent),
        "with the thread still open"
    );
    assert!(h.has("Thread"), "the thread panel is on screen");
}

#[test]
fn a_task_attachment_that_cannot_go_up_says_why_and_can_be_tried_again() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-ui-task-fail".into(),
        tag: 1,
        server: Some("127.0.0.1:9".into()),
    });
    super::together::save_token("127.0.0.1:9", "dev-ui-task-fail", "tok");
    let id = open_design_task(&mut h);
    h.dropped = vec![egui::DroppedFile {
        path: Some(test_picture("refused.png", 40, 30)),
        ..Default::default()
    }];
    h.frame(vec![]);
    // the server can't be reached: not "Adding..." for ever, but why, and
    // a way to try again
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !h.has("Try again refused.png") && std::time::Instant::now() < until {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(h.has("Try again refused.png"));
    assert!(h.app.task(id).unwrap().attachments.is_empty());
    let sha = h.app.task_files[0].1.att.clone().unwrap().file;
    assert!(h.app.files.is_waiting(&sha), "also tried again by itself");
    h.click("Try again refused.png");
    assert_eq!(h.app.task_files.len(), 1);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !h.has("Dismiss refused.png") && std::time::Instant::now() < until {
        h.frame(vec![]);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.click("Dismiss refused.png");
    super::together::forget_token("127.0.0.1:9", "dev-ui-task-fail");
    assert!(h.app.task_files.is_empty());
    assert!(
        !h.app.files.is_waiting(&sha),
        "and no longer waits to go up"
    );
}

#[test]
fn import_from_slack_in_an_empty_channel_brings_the_export_in() {
    let mut h = Ui1::new();
    h.app
        .apply_chat(vec![super::chat::ChatAction::NewChannel("fresh".into())]);
    h.app.section = Section::Chat;
    h.settle();
    assert!(!h.has("\u{2699}  Integrate"), "no button that does nothing");
    // a Slack export zip, "chosen" in the file picker
    let root = std::env::temp_dir().join(format!("wb-ui-slack-{:x}", crate::sync::random_u64()));
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(
        ex.join("users.json"),
        r#"[{"id":"U1","name":"alex","real_name":"Alex Kim","profile":{}}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":["U1"]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","user":"U1","text":"first sketch","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    assert!(std::process::Command::new("/usr/bin/ditto")
        .args(["-c", "-k"])
        .arg(&ex)
        .arg(&zip)
        .status()
        .unwrap()
        .success());
    super::slack::PICKED.with(|p| *p.borrow_mut() = Some(zip));
    h.click("Import from Slack");
    assert!(matches!(h.app.modal, Some(Modal::SlackImport(_))));
    h.click("Import");
    let design = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "design")
        .expect("the channel came in");
    assert_eq!(design.messages[0].text, "first sketch");
    assert_eq!(design.messages[0].ts, 1_704_189_600);
    h.click("OK");
    assert!(h.app.modal.is_none());
}

#[test]
fn huddle_shares_a_screen_and_shows_others_screens_large() {
    let mut h = Ui1::new();
    let ch = h.app.data.channels[0].id;
    let me = h.app.data.me;
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    let person = |id: u64, dev: &str, me: bool, screen: bool| wb_voice::Person {
        identity: format!("{}-{}", id, dev),
        name: String::new(),
        me,
        speaking: false,
        muted: false,
        heard: 0.0,
        camera: true,
        screen,
    };
    h.app.huddle.channel = Some(ch);
    h.app.huddle.joined = true;
    h.app.huddle.people = vec![person(me, "dev1", true, false)];
    h.settle();

    // Share screen offers the displays and the windows
    h.click("Share screen");
    assert!(h.app.huddle.share_menu);
    assert!(h.has("Share Test display") && h.has("Share Notes: Plan"));
    h.click("Share Notes: Plan");
    assert_eq!(h.app.huddle.screen, Some(wb_voice::Share::Window(7)));
    assert!(!h.app.huddle.share_menu);
    assert!(h.has("Stop sharing screen") && h.has("Sharing your screen"));
    h.click("Stop sharing screen");
    assert!(h.app.huddle.screen.is_none());
    h.click("Share screen");
    h.click("Share Test display");
    assert_eq!(h.app.huddle.screen, Some(wb_voice::Share::Display(1)));
    h.click("Stop sharing screen");

    // without the Screen Recording permission: how to turn it on
    h.app.huddle.screen_env = huddle::ScreenEnv::Fake {
        allowed: false,
        sources: vec![],
    };
    h.click("Share screen");
    assert!(h.has("Workboard needs the Screen Recording permission to share your screen."));
    assert!(!h.has("Share Test display"));
    h.click("Open System Settings");
    assert!(h.app.huddle.opened_settings);
    h.click("Cancel");
    assert!(!h.app.huddle.share_menu);
    // capture refused once sharing started: the same guidance comes up
    h.app.huddle.screen = Some(wb_voice::Share::Display(1));
    h.app.huddle.screen_problem = Some(wb_voice::ScreenTrouble::NotAllowed);
    h.app.huddle.share_menu = true;
    h.settle();
    assert!(h.has("Open System Settings"));
    h.click("Cancel");

    // someone else shares: their screen shows large with their name, their
    // camera small, and each picture asks for the layer that fits its size
    let them = person(other.id, "dev2", false, true);
    h.app.huddle.people.push(them.clone());
    let pic = |w: u32, hh: u32| wb_voice::Picture {
        width: w,
        height: hh,
        rgba: vec![120u8; (w * hh * 4) as usize],
    };
    let screen = wb_voice::screen_key(&them.identity);
    h.app
        .huddle
        .pictures
        .insert(screen.clone(), (pic(192, 108), true));
    h.app
        .huddle
        .pictures
        .insert(them.identity.clone(), (pic(64, 36), true));
    h.settle();
    assert!(h.has(&format!("Screen of {}", other.name)));
    assert!(h.has(&format!("{}'s screen", other.name)));
    let big = h.rect(&format!("Screen of {}", other.name));
    let small = h.rect(&format!("Video of {}", other.name));
    assert!(big.width() > small.width() * 3.0, "{:?} {:?}", big, small);
    let views = h.app.huddle.views.clone();
    assert_eq!(views.len(), 2, "{:?}", views);
    assert_eq!(
        wb_voice::quality::layer_for(views[&them.identity]),
        wb_voice::quality::VideoQuality::Low
    );
    assert!(wb_voice::quality::layer_for(views[&screen]) != wb_voice::quality::VideoQuality::Low);
    // the screen share ends: back to normal-sized video
    h.app.huddle.drop_picture(&screen);
    h.settle();
    assert!(!h.has(&format!("Screen of {}", other.name)));
    assert!(h.rect(&format!("Video of {}", other.name)).width() > small.width() * 1.5);
    assert_eq!(h.app.huddle.views.len(), 1);
}

#[test]
fn home_lists_card_says_when_there_are_no_lists_and_offers_one() {
    let mut h = Ui1::new();
    // a workspace without Spaces or Lists, as a first start has
    h.app.data.spaces.retain(|s| s.private_owner.is_some());
    h.app.section = Section::Home;
    h.app.loc = Location::Home;
    h.settle();
    assert!(h.has("No Lists yet. Lists keep a Space's tasks together."));
    h.click("Home new list");
    assert!(
        matches!(h.app.modal, Some(Modal::NewSpace { .. })),
        "no Space yet: make one first"
    );
    h.key(egui::Key::Escape);
    // with a Space, the button makes a List in it
    let mut h = Ui1::new();
    for s in h.app.data.spaces.iter_mut() {
        s.lists.clear();
        s.folders.clear();
    }
    h.app.section = Section::Home;
    h.app.loc = Location::Home;
    h.settle();
    h.click("Home new list");
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })));
}

#[test]
fn inbox_with_reminders_waiting_is_not_all_caught_up() {
    let mut h = Ui1::new();
    h.click("Inbox");
    h.app.data.extra.inbox_cleared = h.app.inbox_items().into_iter().map(|i| i.key).collect();
    h.settle();
    assert!(h.has("You're all caught up") || h.has("Looking to collaborate?"));
    let id = h.app.data.new_id();
    h.app.data.extra.reminders.push(Reminder {
        id,
        text: "Call the bank".into(),
        due_ts: now_ts() - 60,
        owner: h.app.data.me,
        ..Default::default()
    });
    h.settle();
    assert!(h.has("Call the bank"));
    assert!(!h.has("You're all caught up"), "a reminder is waiting");
    assert!(h.has("No new comments, mentions or assignments."));
}

// ---------------------------------------------------------------- hover feedback

#[test]
fn frameless_buttons_show_the_hand_and_a_hover_fill() {
    let mut h = Ui1::new();
    let g = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    h.app.section = Section::Chat;
    h.app.chat_open = Some(g);
    h.settle();
    // the channel header's bell, like Ask and Integrate next to it
    let bell = h.find("Channel notifications").unwrap();
    h.hover(pos2(bell.x, bell.y + 200.0));
    assert!(
        !h.hover_fill_at(bell),
        "no fill while the pointer is elsewhere"
    );
    h.hover(bell);
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand);
    assert!(h.hover_fill_at(bell), "a fill appears under the pointer");
}

#[test]
fn reaction_bar_buttons_show_which_one_is_under_the_pointer() {
    let (mut h, _) = general_with_my_message("hover the bar");
    let thumbs = h.find("\u{1F44D}").unwrap();
    h.hover(thumbs);
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand);
    assert!(h.hover_fill_at(thumbs));
    // only the button under the pointer is lit
    let other = h.find("More reactions").unwrap();
    assert!(!h.hover_fill_at(other));
}

#[test]
fn painted_controls_get_the_hand_but_fields_and_backdrops_do_not() {
    let mut h = Ui1::new();
    h.app.section = Section::Teams;
    h.settle();
    h.click_side("Create team", true);
    // a colour swatch is painted by hand, not an egui button
    let sw = h.find("Team color 3").unwrap();
    h.hover(sw);
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand);

    // a text field keeps its I-beam
    let mut h = Ui1::new();
    h.click("general");
    let field = h.find("Message composer").unwrap();
    h.hover(field);
    assert_eq!(h.cursor, egui::CursorIcon::Text);

    // the dim backdrop behind an open task is clickable (it closes the task)
    // but is not a control
    let mut h = Ui1::new();
    h.open_clean_task();
    h.hover(pos2(6.0, 450.0));
    assert!(h.app.open_task.is_some());
    assert_eq!(h.cursor, egui::CursorIcon::Default);
}

#[test]
fn filled_buttons_tint_on_hover() {
    let mut h = Ui1::new();
    h.click("general");
    h.click("Message composer");
    h.type_text("ready to send");
    // the channel composer's send arrow turns solid once there is text
    let send = h.find("Send").unwrap();
    h.hover(pos2(send.x, send.y + 200.0));
    assert!(!h.hover_fill_at(send));
    h.hover(send);
    assert_eq!(h.cursor, egui::CursorIcon::PointingHand);
    assert!(
        h.hover_fill_at(send),
        "a tint goes over the button's own fill"
    );
}

// ---- spaces shared with chosen people (sharing.rs)

fn sharing_of(h: &Ui1, sid: u64) -> Option<(bool, Vec<(u64, String)>)> {
    let s = h.app.data.spaces.iter().find(|s| s.id == sid)?;
    s.sharing.as_ref().map(|x| {
        (
            x.everyone,
            x.people
                .iter()
                .map(|p| (p.member, p.level.clone()))
                .collect(),
        )
    })
}

#[test]
fn share_dialog_chooses_people_and_their_levels() {
    let mut h = Ui1::new();
    let sid = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.name == "Side Projects")
        .unwrap()
        .id;
    h.click("Space options Side Projects");
    h.click("Share & permissions");
    assert!(
        h.has("Sharing is enforced on a team server only. Alone or in a shared folder, everyone with the workspace can see and change every space."),
        "without a server the dialog says nothing is enforced"
    );
    h.click("Share with chosen people");
    h.click("Add person");
    h.click("Add Sam Lee");
    h.click("Sam Lee View");
    h.click("Save");
    assert_eq!(
        sharing_of(&h, sid),
        Some((false, vec![(1, "full".into()), (3, "view".into())])),
        "whoever closes the space keeps Full in it"
    );
    // back to everyone: the people stay for next time
    h.click("Space options Side Projects");
    h.click("Share & permissions");
    h.click("Remove Sam Lee");
    h.click("Share with everyone");
    h.click("Save");
    assert_eq!(sharing_of(&h, sid), Some((true, vec![(1, "full".into())])));
    // a space never shared stays without the field when saved as everyone's
    let other = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.id != sid && s.private_owner.is_none())
        .unwrap()
        .clone();
    h.app.open_sharing(other.id);
    h.settle();
    h.click("Save");
    assert_eq!(sharing_of(&h, other.id), None);
}

#[test]
fn a_space_shared_to_view_or_comment_reads_only() {
    let mut h = Ui1::new();
    // on a team server, as a plain member
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    let task = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Write copy for landing page")
        .unwrap()
        .clone();
    let space = h.app.data.space_of_list(task.list_id).unwrap().clone();
    let share = |h: &mut Ui1, level: &str| {
        let s = h
            .app
            .data
            .spaces
            .iter_mut()
            .find(|s| s.id == space.id)
            .unwrap();
        s.sharing = Some(SpaceSharing {
            everyone: false,
            people: vec![SpaceGrant {
                member: 1,
                level: level.into(),
            }],
        });
        h.settle();
    };
    share(&mut h, "view");
    // the list says so, and the space menu leaves out what would change it
    h.app.loc = Location::List(task.list_id);
    h.settle();
    assert!(h.has("View only"));
    h.click(&format!("Space options {}", space.name));
    assert!(h.has("You can read this space.") && !h.has("Rename") && !h.has("Delete"));
    h.key(egui::Key::Escape);
    // the task window: fields and the comment box are off
    h.click_left("Task Write copy for landing page", 120.0);
    assert_eq!(h.app.open_task, Some(task.id));
    h.click("Comment box");
    h.type_text("hello");
    assert!(h.app.comment_text.is_empty(), "the comment box is off");
    // an edit that gets through some other way is turned down
    let mut renamed = task.clone();
    renamed.name = "Renamed".into();
    h.app.actions.push(Action::Replace(renamed.clone()));
    h.app.apply_actions();
    assert_eq!(h.app.task(task.id).unwrap().name, task.name);
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("You can only view this space.")
    );
    h.app.actions.push(Action::DeleteTask(task.id));
    h.app.apply_actions();
    assert!(h.app.task(task.id).is_some(), "not deleted");
    // Comment: the box works and comments go in, nothing else does
    share(&mut h, "comment");
    assert!(h.has("Can comment"));
    h.click("Comment box");
    h.type_text("looks good");
    h.click("Send Comment");
    assert!(h
        .app
        .task(task.id)
        .unwrap()
        .activity
        .iter()
        .any(|a| a.comment && a.text == "looks good" && a.author == 1));
    h.app.actions.push(Action::Replace(renamed));
    h.app.apply_actions();
    assert_eq!(h.app.task(task.id).unwrap().name, task.name);
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("You can only view and comment in this space.")
    );
    // an admin has Full everywhere
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.settle();
    assert!(!h.has("Can comment"));
}

// ---------------------------------------------------------------- control sizes

/// A text field, the buttons and drop-downs beside it are one height, and
/// sit on one line (the fields used to be 20 pt next to 28 pt buttons).
#[test]
fn fields_buttons_and_drop_downs_in_a_dialog_are_one_height() {
    let mut h = Ui1::new();
    h.app.modal = Some(Modal::SetStatus {
        text: String::new(),
    });
    h.settle();
    // a text field reports the box its text sits in: add the field's
    // 5 pt top and bottom margin for its frame
    let framed = |r: Rect| r.expand2(vec2(0.0, 5.0));
    let field = framed(h.rect("Status text"));
    let save = h.rect("Save");
    let clear = h.rect("Clear status");
    assert!(
        field.height() >= save.height() - 0.5,
        "status field {} vs Save {}",
        field.height(),
        save.height()
    );
    assert!(
        (save.height() - clear.height()).abs() < 0.5,
        "Save and Clear status"
    );

    h.app.modal = Some(Modal::Reminder {
        text: String::new(),
        date: Some(today()),
        time: "09:00".into(),
    });
    h.settle();
    let day = h.rect("Reminder date");
    let time = framed(h.rect("Reminder time"));
    assert!(
        (day.height() - time.height()).abs() < 0.5,
        "Today and the time"
    );
    assert!((day.center().y - time.center().y).abs() < 0.5);

    h.app.modal = Some(Modal::CustomFields {
        name: String::new(),
        kind: "text".into(),
        opts: String::new(),
    });
    h.settle();
    let name = framed(h.rect("Field name"));
    let kind = h.rect("Field type");
    assert!(
        (name.height() - kind.height()).abs() < 0.5,
        "field and drop-down"
    );
    assert!(
        (name.top() - kind.top()).abs() < 0.5,
        "the drop-down sits on the field's line: {} vs {}",
        name.top(),
        kind.top()
    );
}

/// Check boxes are small squares, filled when on, outlined when off (egui's
/// own took the widgets' 6 pt corners and came out as circles).
#[test]
fn check_boxes_are_squares_that_fill_when_ticked() {
    let mut h = Ui1::new();
    h.app.modal = Some(Modal::CustomizeNav);
    h.settle();
    fn boxes(s: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match s {
            egui::Shape::Rect(r)
                if (r.rect.width() - 15.0).abs() < 0.1 && (r.rect.height() - 15.0).abs() < 0.1 =>
            {
                out.push(r.clone())
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| boxes(s, out)),
            _ => {}
        }
    }
    let mut found = vec![];
    for c in &h.shapes {
        boxes(&c.shape, &mut found);
    }
    assert!(found.len() >= 4, "a box per app: {}", found.len());
    for b in &found {
        assert!(b.corner_radius.nw <= 4, "square corners, not a circle");
    }
    let filled = found
        .iter()
        .filter(|b| {
            b.fill != h.app.t().bg && b.fill != Color32::TRANSPARENT && b.stroke.width == 0.0
        })
        .count();
    let outlined = found.iter().filter(|b| b.stroke.width > 0.0).count();
    assert!(
        filled > 0 && outlined > 0,
        "ticked boxes filled ({}), the others outlined ({})",
        filled,
        outlined
    );
}

#[test]
fn the_slack_import_dialog_shows_who_goes_where_and_lets_me_change_it() {
    let mut h = Ui1::new();
    h.app.section = Section::Chat;
    h.settle();
    let root = std::env::temp_dir().join(format!("wb-ui-slack2-{:x}", crate::sync::random_u64()));
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(
        ex.join("users.json"),
        r#"[{"id":"U1","name":"alex","real_name":"Alex Kim","profile":{}}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":["U1"]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","user":"U1","text":"first sketch","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    assert!(std::process::Command::new("/usr/bin/ditto")
        .args(["-c", "-k"])
        .arg(&ex)
        .arg(&zip)
        .status()
        .unwrap()
        .success());
    h.app.open_slack_export(&zip);
    h.settle();
    // the Slack Alex Kim is proposed as the member Alex Kim
    assert!(h.has("Whose messages go where:"));
    assert!(h.has("Alex Kim (@alex)"));
    h.click("Messages from Alex Kim");
    h.click("Import under my name");
    match &h.app.modal {
        Some(Modal::SlackImport(p)) => {
            assert_eq!(p.assign.get("U1"), Some(&super::slack::Assign::Me))
        }
        _ => panic!("the dialog stays open"),
    }
    h.click("Import");
    let me = h.app.data.me;
    let design = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "design")
        .unwrap();
    assert_eq!(design.messages[0].author, me);
    assert_eq!(design.messages[0].text, "Alex Kim: first sketch");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_chat_mention_picked_from_the_list_means_that_person_when_two_share_a_name() {
    let mut h = Ui1::new();
    let sam = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Sam Lee")
        .unwrap()
        .id;
    let sam2 = h.app.data.new_id();
    h.app.data.members.push(Member {
        id: sam2,
        name: "Sam Lee".into(),
        color: [1, 2, 3],
        email: "sam2@example.com".into(),
        role: "Member".into(),
        invited: false,
        status_text: String::new(),
        slack: vec![],
        slack_mine: vec![],
    });
    h.click("general");
    let before: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Sam Lee")
        .map(|(_, r)| *r)
        .collect();
    h.click("Mention someone");
    // the list's rows: the second Sam Lee is the lower one
    let mut rows: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Sam Lee" && !before.contains(r))
        .map(|(_, r)| *r)
        .collect();
    rows.sort_by(|a, b| a.center().y.total_cmp(&b.center().y));
    rows.dedup();
    assert_eq!(rows.len(), 2, "both are offered");
    h.click_at(rows[1].center());
    assert_eq!(h.app.chat_text, "@Sam Lee ");
    h.type_text("can you look?");
    h.key(egui::Key::Enter);
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap();
    let m = general.messages.last().unwrap();
    assert_eq!(m.text, "@Sam Lee can you look?");
    assert_eq!(m.mentions, vec![sam2], "the one picked, not both");
    assert!(!m.mentions.contains(&sam));
    assert!(h.app.chat_picked.is_empty(), "used up by the message");
    // typed by hand, without the list: both (it can't tell which)
    h.type_text("@Sam Lee again");
    h.key(egui::Key::Enter);
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap();
    let mut both = general.messages.last().unwrap().mentions.clone();
    both.sort();
    let mut want = vec![sam, sam2];
    want.sort();
    assert_eq!(both, want);
}

/// A workspace with no List a task could go in: no Spaces at all, or
/// (with `keep_space`) a Space whose Lists are gone.
fn no_list_workspace(keep_space: bool) -> Ui1 {
    let mut h = Ui1::new();
    if keep_space {
        for s in h.app.data.spaces.iter_mut() {
            s.lists.clear();
            s.folders.clear();
        }
    } else {
        h.app.data.spaces.retain(|s| s.private_owner.is_some());
        for s in h.app.data.spaces.iter_mut() {
            s.lists.clear();
            s.folders.clear();
        }
    }
    assert_eq!(h.app.data.first_list(), None);
    h.settle();
    h
}

#[test]
fn add_task_without_a_list_says_why_and_offers_one() {
    // the header "Add Task" on My Tasks: no Space yet, so New Space opens
    let mut h = no_list_workspace(false);
    h.click("My Tasks");
    h.click("Add Task");
    assert!(h.app.quick_add.is_none());
    let toast = h.app.toast.clone().expect("a toast says why").0;
    assert!(toast.contains("Make a Space first"), "{}", toast);
    assert!(matches!(h.app.modal, Some(Modal::NewSpace { .. })));
    // with a Space, the New List dialog opens instead (Everything, + Create > Task)
    let mut h = no_list_workspace(true);
    h.click("All Tasks");
    h.click("Create");
    h.click("Task");
    assert!(h.app.quick_add.is_none());
    assert!(h.app.toast.clone().unwrap().0.contains("Make one first"));
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })));
}

#[test]
fn inline_add_without_a_list_keeps_the_name() {
    let mut h = no_list_workspace(true);
    h.click("Today & Overdue");
    let row = h
        .tree
        .iter()
        .map(|(n, _)| n.clone())
        .find(|n| n.starts_with("Add Task in "))
        .expect("an inline add row");
    h.click(&row);
    h.type_text("Call the printer");
    h.key(egui::Key::Enter);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .all(|t| t.name != "Call the printer"));
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })));
    assert_eq!(
        h.app.adding.as_ref().map(|a| a.1.as_str()),
        Some("Call the printer")
    );
}

#[test]
fn planner_add_without_a_list_keeps_the_name_and_says_why() {
    let mut h = no_list_workspace(false);
    h.app.section = Section::Planner;
    h.settle();
    let day = h.app.planner_week + Duration::days(2);
    h.click(&format!("Add task on {}", day.format("%a %-d")));
    h.type_text("Plan the trip");
    h.key(egui::Key::Enter);
    assert!(h.app.data.tasks.iter().all(|t| t.name != "Plan the trip"));
    assert_eq!(
        h.app.sweep.planner_add,
        Some((day, "Plan the trip".to_string()))
    );
    assert!(h.app.toast.is_some());
    assert!(matches!(h.app.modal, Some(Modal::NewSpace { .. })));
}

/// The seed with every List and task archived (the audit's "archived" state).
fn archived_workspace() -> Ui1 {
    let mut h = Ui1::new();
    for s in h.app.data.spaces.iter_mut() {
        for l in s.lists.iter_mut() {
            l.archived = true;
        }
        for f in s.folders.iter_mut() {
            for l in f.lists.iter_mut() {
                l.archived = true;
            }
        }
    }
    for t in h.app.data.tasks.iter_mut() {
        t.archived = true;
    }
    h.app.section = Section::Home;
    h.app.loc = Location::Home;
    h.settle();
    h
}

fn list_id(h: &Ui1, name: &str) -> u64 {
    h.app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .find(|l| l.name == name)
        .map(|l| l.id)
        .unwrap()
}

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
fn create_list_or_channel_list_without_a_space_says_so() {
    let mut h = no_list_workspace(false);
    h.click("Create");
    h.click("List");
    assert!(
        matches!(h.app.modal, Some(Modal::NewSpace { .. })),
        "no Space to pick: New Space instead of a List dialog with nothing to pick"
    );
    assert!(h
        .app
        .toast
        .clone()
        .unwrap()
        .0
        .contains("Make a Space first"));
    // the channel's "Add a List" switch is off and says why
    h.app.modal = Some(Modal::NewChannel {
        name: String::new(),
        private: false,
        add_list: true,
    });
    h.settle();
    assert!(h.has("Needs a Space to put the List in. Make one from + Create > Space."));
    h.click("Add a List");
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewChannel {
            add_list: false,
            ..
        })
    ));
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
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                _ => {}
            }
        }
        let mut out = vec![];
        for s in &h.shapes {
            walk(&s.shape, &mut out);
        }
        out
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

#[test]
fn task_picker_with_nothing_typed_does_not_say_no_match() {
    let mut h = archived_workspace();
    let tid = h.app.data.tasks[0].id;
    h.app.data.tasks[0].archived = false;
    h.app.modal = Some(Modal::PickTask {
        task: tid,
        mode: TaskPick::Linked,
        query: String::new(),
    });
    h.settle();
    assert!(!h.has("No tasks match"));
    assert!(h.has("No other tasks to pick. Archived tasks are not offered here."));
}

/// What the app names a whiteboard, form, team or workspace when it makes
/// one is saved in the language of the person who made it.
#[test]
fn things_the_app_names_are_named_in_korean() {
    let mut h = Ui1::new();
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    h.app.new_whiteboard();
    h.app.new_form();
    h.app.new_team();
    let fresh = Data::fresh();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let d = &h.app.data.extra;
    assert_eq!(d.whiteboards.last().unwrap().name, "화이트보드 1");
    let f = d.forms.last().unwrap();
    assert_eq!(f.name, "새 양식");
    assert_eq!(f.fields[0].label, "제목");
    assert!(d.teams.last().unwrap().name.starts_with("팀 "));
    assert_eq!(fresh.workspace, "내 워크스페이스");
    assert_eq!(fresh.spaces[0].name, "기본 스페이스");
    assert_eq!(fresh.spaces[0].lists[0].name, "목록");
    // and in English, as before
    h.app.new_whiteboard();
    assert_eq!(
        h.app.data.extra.whiteboards.last().unwrap().name,
        "Whiteboard 2"
    );
}

/// A call message is saved in English for every device and shown in the
/// reader's language; a member who joined is "활성", not the task status.
#[test]
fn call_messages_and_member_state_read_in_korean() {
    let mut h = Ui1::new();
    let ch = h.app.data.channels[0].id;
    h.app.start_call(None, ch, "https://meet.example/x");
    let m = h.app.data.channels[0].messages.last().unwrap().clone();
    assert_eq!(m.text, "Started a call", "saved as before");
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    assert_eq!(super::chat::shown_text(&m), "통화를 시작했습니다");
    assert_eq!(crate::i18n::tr_ctx("Active (member)", "Active"), "활성");
    assert_eq!(
        crate::i18n::tr("Active"),
        "진행",
        "the task status keeps its word"
    );
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert_eq!(super::chat::shown_text(&m), "Started a call");
}

/// The invite dialog's long note wraps inside the dialog instead of
/// stretching it past its widest.
#[test]
fn invite_note_wraps_inside_the_dialog() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    // connected as the owner: the longest note
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.app.modal = Some(Modal::Invite {
        emails: String::new(),
        role: "Member".into(),
    });
    h.settle();
    let close = h.rect("Close");
    let note = h
        .tree
        .iter()
        .rev()
        .find(|(n, _)| n.starts_with("This workspace is on a server"))
        .map(|(_, r)| *r)
        .expect("the note");
    assert!(note.width() <= 620.0, "note {} wide", note.width());
    assert!(note.right() <= close.right() + 1.0, "inside the dialog");
}

// ---- whiteboard ids, undo and note focus (r4-ui)

/// A whiteboard on a team (a server that isn't connected), open on screen,
/// with three strokes and two notes the team already knows.
fn shared_board(h: &mut Ui1) -> u64 {
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let ids: Vec<u64> = (0..5).map(|_| h.app.data.new_id()).collect();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    for (k, id) in ids[..3].iter().enumerate() {
        wb.strokes.push(WbStroke {
            id: *id,
            color: [0, 0, 0],
            width: 2.0,
            points: vec![
                [10.0, 10.0 + k as f32 * 20.0],
                [60.0, 10.0 + k as f32 * 20.0],
            ],
        });
    }
    for (k, id) in ids[3..].iter().enumerate() {
        wb.notes.push(Sticky {
            id: *id,
            pos: [300.0 + k as f32 * 200.0, 600.0],
            text: format!("Known {}", k),
            color: [0xf8, 0xae, 0x00],
        });
    }
    h.app.dirty = true;
    h.settle();
    h.app.sync_note_change();
    wid
}

fn board_of(h: &Ui1, wid: u64) -> Whiteboard {
    h.app
        .data
        .extra
        .whiteboards
        .iter()
        .find(|w| w.id == wid)
        .cloned()
        .unwrap()
}

/// A teammate's copy of `data` with one change of theirs: `change` runs on
/// it and the change is stamped a moment later.
fn teammate_copy(data: &Data, change: impl FnOnce(&mut Data)) -> serde_json::Value {
    let mut d = data.clone();
    change(&mut d);
    let mut v = serde_json::to_value(&d).unwrap();
    let mut meta = d.extra.sync.clone();
    crate::sync::stamp(&v, &mut meta, chrono::Utc::now().timestamp_millis() + 5);
    v["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    v
}

#[test]
fn a_teammate_change_arriving_while_i_draw_keeps_every_stroke() {
    let mut h = Ui1::new();
    let wid = shared_board(&mut h);
    let known: Vec<u64> = board_of(&h, wid).strokes.iter().map(|s| s.id).collect();
    let before = h.app.data.clone();
    // I start a stroke and keep the button down
    h.app.wb_tool = 0;
    let canvas = rect_of(&h, "Whiteboard canvas");
    let from = canvas.center();
    h.frame(vec![egui::Event::PointerMoved(from)]);
    h.frame(vec![egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    for i in 1..=6 {
        h.frame(vec![egui::Event::PointerMoved(
            from + vec2(10.0 * i as f32, 4.0 * i as f32),
        )]);
    }
    assert!(
        board_of(&h, wid).strokes.iter().all(|s| s.id != 0),
        "the stroke being drawn has an id already"
    );
    // a teammate's new note merges in mid-stroke
    let note_id = 1 << 40;
    let team = teammate_copy(&before, |d| {
        let w = d
            .extra
            .whiteboards
            .iter_mut()
            .find(|w| w.id == wid)
            .unwrap();
        w.notes.push(Sticky {
            id: note_id,
            pos: [600.0, 40.0],
            text: "Theirs".into(),
            color: [8, 128, 234],
        });
    });
    h.app.merge_in(&[team], &[]);
    // the stroke goes on and ends
    let last = from + vec2(120.0, 50.0);
    for i in 7..=12 {
        h.frame(vec![egui::Event::PointerMoved(
            from + vec2(10.0 * i as f32, 4.0 * i as f32),
        )]);
    }
    h.frame(vec![egui::Event::PointerButton {
        pos: last,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    h.settle();
    let b = board_of(&h, wid);
    for id in &known {
        assert!(
            b.strokes.iter().any(|s| s.id == *id),
            "known stroke {} is kept: {:?}",
            id,
            b.strokes.iter().map(|s| s.id).collect::<Vec<_>>()
        );
        assert!(
            !h.app.data.extra.sync.tombs.contains_key(id),
            "and not marked deleted"
        );
    }
    assert!(
        b.notes.iter().any(|n| n.id == note_id),
        "their note came in"
    );
    assert_eq!(b.strokes.len(), known.len() + 1);
    let mine = b.strokes.iter().find(|s| !known.contains(&s.id)).unwrap();
    assert!(mine.id != 0);
    assert!(
        mine.points.len() >= 12,
        "every point went into my stroke: {}",
        mine.points.len()
    );
    // and the next merge doesn't take them away either
    let team = teammate_copy(&h.app.data, |_| {});
    h.app.merge_in(&[team], &[]);
    assert_eq!(board_of(&h, wid).strokes.len(), known.len() + 1);
}

#[test]
fn whiteboard_undo_takes_back_my_own_last_action_not_a_teammates() {
    let mut h = Ui1::new();
    let wid = shared_board(&mut h);
    let known: Vec<u64> = board_of(&h, wid).strokes.iter().map(|s| s.id).collect();
    let canvas = rect_of(&h, "Whiteboard canvas");
    // I draw a stroke
    h.app.wb_tool = 0;
    drag_by(&mut h, canvas.center(), canvas.center() + vec2(60.0, 20.0));
    let mine = board_of(&h, wid)
        .strokes
        .iter()
        .find(|s| !known.contains(&s.id))
        .unwrap()
        .id;
    // I add a note
    h.app.wb_tool = 1;
    let at = canvas.left_top() + vec2(120.0, 420.0);
    h.click_at(at);
    let b = board_of(&h, wid);
    let my_note = b.notes.last().unwrap().id;
    assert!(my_note != 0);
    // a teammate adds a stroke and a note after mine (merged in at the end)
    let (their_stroke, their_note) = (1 << 41, (1 << 41) + 1);
    let team = teammate_copy(&h.app.data, |d| {
        let w = d
            .extra
            .whiteboards
            .iter_mut()
            .find(|w| w.id == wid)
            .unwrap();
        w.strokes.push(WbStroke {
            id: their_stroke,
            color: [8, 128, 234],
            width: 3.0,
            points: vec![[500.0, 500.0], [520.0, 540.0]],
        });
        w.notes.push(Sticky {
            id: their_note,
            pos: [700.0, 40.0],
            text: "Theirs".into(),
            color: [8, 128, 234],
        });
    });
    h.app.merge_in(&[team], &[]);
    h.settle();
    let b = board_of(&h, wid);
    assert_eq!(b.strokes.last().unwrap().id, their_stroke);
    assert_eq!(b.notes.last().unwrap().id, their_note);
    // Undo takes back my note, then my stroke; theirs stay
    h.click("Undo");
    let b = board_of(&h, wid);
    assert!(
        !b.notes.iter().any(|n| n.id == my_note),
        "my note is undone"
    );
    assert!(b.notes.iter().any(|n| n.id == their_note), "theirs stays");
    h.click("Undo");
    let b = board_of(&h, wid);
    assert!(
        !b.strokes.iter().any(|s| s.id == mine),
        "my stroke is undone"
    );
    assert!(
        b.strokes.iter().any(|s| s.id == their_stroke),
        "theirs stays"
    );
    assert_eq!(b.strokes.len(), known.len() + 1);

    // a note I moved, with a teammate's note merged in before it
    let first = b.notes[0].id;
    let at0 = b.notes[0].pos;
    h.app.wb_tool = 2;
    h.settle();
    let view = h.app.sweep.wb_view.get(&wid).copied().unwrap_or_default();
    let grab = view.to_screen(canvas.min, [at0[0] + 80.0, at0[1] + 4.0]);
    drag_by(&mut h, grab, grab + vec2(40.0, 30.0));
    let moved = board_of(&h, wid)
        .notes
        .iter()
        .find(|n| n.id == first)
        .unwrap()
        .pos;
    assert!(moved != at0, "the note moved");
    let front = (1 << 41) + 2;
    let mut team = teammate_copy(&h.app.data, |d| {
        let w = d
            .extra
            .whiteboards
            .iter_mut()
            .find(|w| w.id == wid)
            .unwrap();
        w.notes.insert(
            0,
            Sticky {
                id: front,
                pos: [900.0, 900.0],
                text: "Front".into(),
                color: [8, 128, 234],
            },
        );
    });
    // the merge keeps their order: the new note goes first
    let _ = &mut team;
    h.app.merge_in(&[team], &[]);
    h.settle();
    h.click("Undo");
    let b = board_of(&h, wid);
    assert_eq!(
        b.notes.iter().find(|n| n.id == first).unwrap().pos,
        at0,
        "my moved note goes back"
    );
    assert_eq!(
        b.notes.iter().find(|n| n.id == front).unwrap().pos,
        [900.0, 900.0],
        "theirs doesn't move"
    );

    // a stroke of mine a teammate already erased: Undo does nothing to the board
    h.app.wb_tool = 0;
    drag_by(
        &mut h,
        canvas.center() + vec2(0.0, 60.0),
        canvas.center() + vec2(60.0, 80.0),
    );
    let gone = board_of(&h, wid).strokes.last().unwrap().id;
    let team = teammate_copy(&h.app.data, |d| {
        let w = d
            .extra
            .whiteboards
            .iter_mut()
            .find(|w| w.id == wid)
            .unwrap();
        w.strokes.retain(|s| s.id != gone);
        let now = chrono::Utc::now().timestamp_millis() + 5;
        d.extra.sync.tombs.insert(gone, now);
    });
    h.app.merge_in(&[team], &[]);
    h.settle();
    let before = board_of(&h, wid);
    assert!(!before.strokes.iter().any(|s| s.id == gone));
    h.click("Undo");
    let after = board_of(&h, wid);
    assert_eq!(
        after.strokes.iter().map(|s| s.id).collect::<Vec<_>>(),
        before.strokes.iter().map(|s| s.id).collect::<Vec<_>>(),
        "nothing else is taken away"
    );
}

#[test]
fn typing_in_a_note_keeps_going_to_it_after_scrolling_the_board() {
    let mut h = Ui1::new();
    h.app.new_whiteboard();
    h.app.section = Section::Whiteboards;
    let wid = h.app.wb_open.unwrap();
    let wb = h
        .app
        .data
        .extra
        .whiteboards
        .iter_mut()
        .find(|w| w.id == wid)
        .unwrap();
    // one note at the top that scrolls away, two lower down
    for (k, (text, pos)) in [
        ("Top", [20.0, 0.0]),
        ("Mid", [20.0, 300.0]),
        ("Low", [240.0, 300.0]),
    ]
    .into_iter()
    .enumerate()
    {
        wb.notes.push(Sticky {
            id: 7000 + k as u64,
            pos,
            text: text.into(),
            color: [0xf8, 0xae, 0x00],
        });
    }
    h.settle();
    h.click("Low");
    h.key(egui::Key::End);
    h.type_text("A");
    let text = |h: &Ui1, id: u64| {
        board_of(h, wid)
            .notes
            .iter()
            .find(|n| n.id == id)
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(text(&h, 7002), "LowA");
    // scroll the board up so the top note leaves the screen
    let canvas = rect_of(&h, "Whiteboard canvas");
    h.frame(vec![egui::Event::PointerMoved(canvas.center())]);
    for _ in 0..4 {
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -60.0),
            modifiers: Default::default(),
        }]);
    }
    h.settle();
    assert!(
        h.app
            .sweep
            .wb_view
            .get(&wid)
            .is_some_and(|v| v.pan.y < -150.0),
        "the board scrolled"
    );
    assert!(!h.has("Top"), "the top note is off screen");
    h.type_text("B");
    assert_eq!(
        text(&h, 7002),
        "LowAB",
        "typing still goes to the same note"
    );
    assert_eq!(text(&h, 7001), "Mid");
    assert_eq!(text(&h, 7000), "Top");
}

// ---- the status editor finishes its edits however it goes away (r4-ui)

#[test]
fn switching_the_status_editor_to_another_space_or_quitting_finishes_the_edits() {
    let mut h = Ui1::new();
    let sid = h.app.data.spaces[0].id;
    let other = h.app.data.spaces[1].id;
    let list = h.app.data.lists_in(Location::Space(sid)).unwrap()[0];
    let other_list = h.app.data.lists_in(Location::Space(other)).unwrap()[0];
    let in_list = |h: &Ui1, l: u64| {
        h.app
            .data
            .tasks
            .iter()
            .filter(|t| t.list_id == l)
            .map(|t| t.id)
            .collect::<Vec<_>>()
    };
    let (a, b) = (in_list(&h, list)[0], in_list(&h, list)[1]);
    let c = in_list(&h, other_list)[0];
    for (t, st) in [(a, "in progress"), (b, "to do"), (c, "in progress")] {
        h.app
            .data
            .tasks
            .iter_mut()
            .find(|x| x.id == t)
            .unwrap()
            .status = st.into();
    }
    h.app.data.extra.automations = vec![Automation {
        id: 1,
        enabled: true,
        trigger_status: "to do".into(),
        action: "priority".into(),
        value: "1".into(),
        list: Some(list),
    }];
    let status_ok = |h: &Ui1, si: usize, t: u64| {
        let st = h.app.task(t).unwrap().status.clone();
        h.app.data.spaces[si].statuses.iter().any(|s| s.name == st)
    };
    // in the first space: "to do" renamed, "in progress" emptied
    h.app.apply(Action::EditStatuses(sid));
    h.settle();
    h.click("to do");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("backlog");
    h.click("in progress");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.settle();
    assert!(!status_ok(&h, 0, a), "mid-edit the name is empty");
    // the other space's "Edit statuses" while the editor is open
    h.app.toast = None;
    h.app.apply(Action::EditStatuses(other));
    h.settle();
    assert_eq!(h.app.edit_statuses, Some(other));
    assert!(status_ok(&h, 0, a), "the emptied name went back");
    assert!(status_ok(&h, 0, b));
    assert_eq!(h.app.task(b).unwrap().status, "backlog");
    assert_eq!(
        h.app.data.extra.automations[0].trigger_status, "backlog",
        "the rename reached the automation"
    );
    assert!(h.app.toast.is_some());
    // quitting with a name taken in the other space's editor
    h.click("in progress");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("complete");
    h.settle();
    assert!(
        !status_ok(&h, 1, c),
        "mid-edit the task waits on a name no status has"
    );
    h.app.finish_before_quit();
    assert!(h.app.edit_statuses.is_none());
    let names: Vec<String> = h.app.data.spaces[1]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert!(
        names.contains(&"in progress".to_string()),
        "the taken name went back: {:?}",
        names
    );
    assert!(status_ok(&h, 1, c));
    assert_eq!(
        names.iter().filter(|n| *n == "complete").count(),
        1,
        "no two statuses share a name"
    );
}

// ---- the Columns panel's "no room" counts custom field columns (r4-ui)

#[test]
fn columns_panel_marks_a_property_custom_fields_leave_no_room_for() {
    let mut h = Ui1::new();
    h.app.data.extra.apps.custom_fields = true;
    let name = "Write copy for landing page";
    let list = task_named(&h, name).list_id;
    h.app.apply(Action::Go(Location::List(list)));
    h.app.view = ViewKind::List;
    h.app.hidden_cols.remove("Created by");
    h.settle();
    let w = h.app.list_width;
    assert!(w > 0.0);
    let row = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, 30.0));
    let prio = h.app.data.extra.apps.priorities;
    // as many field columns as it takes to push "Created by" out
    let mut n = 0;
    while h
        .app
        .list_cols(row)
        .extra
        .iter()
        .any(|c| c.0 == "Created by")
    {
        n += 1;
        h.app.data.extra.fields.push(CustomField {
            id: 9100 + n,
            name: format!("F{}", n),
            kind: "text".into(),
            options: vec![],
        });
        assert!(n < 10);
    }
    assert!(
        super::shell::col_layout(row, &h.app.hidden_cols, prio)
            .extra
            .iter()
            .any(|c| c.0 == "Created by"),
        "without the fields it would fit"
    );
    h.settle();
    h.click("Columns");
    let created = rect_of(&h, "Created by");
    let marks: Vec<Rect> = h
        .tree
        .iter()
        .filter(|(n, _)| n == "no room")
        .map(|(_, r)| *r)
        .collect();
    assert!(
        marks
            .iter()
            .any(|r| (r.center().y - created.center().y).abs() < 6.0),
        "\"no room\" beside Created by: {:?} {:?}",
        created,
        marks
    );
}

/// A shared workspace with one 30-minute entry of mine at 10:00 today on
/// "Renew passport", the timesheet's entry list open. Returns the task's
/// index and the entry's start.
fn shared_timesheet(h: &mut Ui1) -> (usize, i64) {
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    let td = today();
    let ws = td - Duration::days(td.weekday().num_days_from_sunday() as i64);
    let start = td
        .and_hms_opt(10, 0, 0)
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
        .position(|t| t.name == "Renew passport")
        .unwrap();
    let me = h.app.data.me;
    h.app.data.tasks[i].time_entries = vec![TimeEntry {
        start,
        end: start + 1800,
        member: me,
    }];
    h.app.section = Section::Timesheets;
    h.app.sheet_week = ws;
    h.app.sheet_entries = true;
    h.app.dirty = true;
    h.settle();
    h.app.sync_note_change();
    (i, start)
}

#[test]
fn a_teammates_earlier_entry_merging_in_mid_edit_does_not_move_my_change() {
    let mut h = Ui1::new();
    let (i, early) = shared_timesheet(&mut h);
    // a second entry of mine at 13:00 the same day
    let late = early + 3 * 3600;
    h.app.data.tasks[i].time_entries.push(TimeEntry {
        start: late,
        end: late + 1800,
        member: 1,
    });
    h.app.dirty = true;
    h.settle();
    h.app.sync_note_change();
    let before = h.app.data.clone();
    // the newest entry is listed first: change the 13:00 one to 2 hours
    let top = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Edit entry Renew passport")
        .map(|(_, r)| *r)
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap();
    h.click_at(top.center());
    assert_eq!(h.app.sweep.entry_edit.as_ref().unwrap().start, late);
    h.app.sweep.entry_edit.as_mut().unwrap().hours = 2.0;
    h.settle();
    // a teammate's entry from earlier that day merges in: the team merge
    // sorts entries by start, so every entry of mine moves one place down
    let theirs = early - 3600;
    let team = teammate_copy(&before, |d| {
        d.tasks[i].time_entries.push(TimeEntry {
            start: theirs,
            end: theirs + 900,
            member: 2,
        });
    });
    h.app.merge_in(&[team], &[]);
    assert_eq!(
        h.app.data.tasks[i].time_entries[0].member, 2,
        "the merge put their entry first"
    );
    h.settle();
    h.click("Save");
    let es = &h.app.data.tasks[i].time_entries;
    assert_eq!(es.len(), 3);
    let t = es.iter().find(|e| e.member == 2).unwrap();
    assert_eq!(
        (t.start, t.end),
        (theirs, theirs + 900),
        "the teammate's entry is untouched"
    );
    let mut mine: Vec<(i64, i64)> = es
        .iter()
        .filter(|e| e.member == 1)
        .map(|e| (e.start, e.end - e.start))
        .collect();
    mine.sort();
    assert_eq!(
        mine,
        vec![(early, 1800), (late, 7200)],
        "the 13:00 entry got the change and the 10:00 one is kept"
    );
    assert!(h.app.sweep.entry_edit.is_none());
}

#[test]
fn an_entry_taken_away_elsewhere_mid_edit_closes_the_editor_with_a_note() {
    let mut h = Ui1::new();
    let (i, _) = shared_timesheet(&mut h);
    let before = h.app.data.clone();
    h.click("Edit entry Renew passport");
    assert!(h.has("Entry hours"));
    // my other device took the entry away
    let team = teammate_copy(&before, |d| d.tasks[i].time_entries.clear());
    h.app.merge_in(&[team], &[]);
    assert!(h.app.data.tasks[i].time_entries.is_empty());
    h.settle();
    assert!(h.app.sweep.entry_edit.is_none(), "the editor closed");
    assert!(!h.has("Entry hours"));
    let note = h
        .app
        .toast
        .as_ref()
        .map(|t| t.0.clone())
        .unwrap_or_default();
    assert!(note.contains("closed without saving"), "{}", note);
    assert!(h.app.data.tasks[i].time_entries.is_empty(), "nothing saved");
}

#[test]
fn the_slack_matching_picker_tells_members_of_one_name_apart() {
    let mut h = Ui1::new();
    let sam2 = h.app.data.new_id();
    h.app.data.members.push(Member {
        id: sam2,
        name: "Sam Lee".into(),
        color: [1, 2, 3],
        role: "Guest".into(),
        ..Default::default()
    });
    h.app.section = Section::Chat;
    h.settle();
    let root = std::env::temp_dir().join(format!("wb-ui-slack3-{:x}", crate::sync::random_u64()));
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(
        ex.join("users.json"),
        r#"[{"id":"U1","name":"sam","real_name":"Sam Lee","profile":{}}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":["U1"]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","user":"U1","text":"hello","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    assert!(std::process::Command::new("/usr/bin/ditto")
        .args(["-c", "-k"])
        .arg(&ex)
        .arg(&zip)
        .status()
        .unwrap()
        .success());
    h.app.open_slack_export(&zip);
    h.settle();
    h.click("Messages from Sam Lee");
    assert!(
        h.has("Sam Lee (sam@example.com)"),
        "the email next to the name"
    );
    assert!(h.has("Sam Lee (Guest)"), "no email: the role");
    h.click("Sam Lee (Guest)");
    match &h.app.modal {
        Some(Modal::SlackImport(p)) => {
            assert_eq!(
                p.assign.get("U1"),
                Some(&super::slack::Assign::Member(sam2))
            )
        }
        _ => panic!("the dialog stays open"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_thread_reply_keeps_the_same_named_person_picked_from_its_mention_list() {
    let mut h = Ui1::new();
    let sam = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Sam Lee")
        .unwrap()
        .id;
    let sam2 = h.app.data.new_id();
    h.app.data.members.push(Member {
        id: sam2,
        name: "Sam Lee".into(),
        color: [1, 2, 3],
        email: "sam2@example.com".into(),
        role: "Member".into(),
        ..Default::default()
    });
    h.click("general");
    let cid = h.app.chat_open.unwrap();
    h.click("1 reply");
    let mid = h.app.thread_open.unwrap();
    let before: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Sam Lee")
        .map(|(_, r)| *r)
        .collect();
    h.click("Mention someone in the reply");
    let mut rows: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Sam Lee" && !before.contains(r))
        .map(|(_, r)| *r)
        .collect();
    rows.sort_by(|a, b| a.center().y.total_cmp(&b.center().y));
    rows.dedup();
    assert_eq!(rows.len(), 2, "both are offered in the thread's list");
    h.click_at(rows[1].center());
    assert_eq!(h.app.thread_text, "@Sam Lee ");
    assert!(h.app.chat_text.is_empty(), "not into the channel's box");
    assert_eq!(h.app.thread_picked, vec![sam2]);
    h.type_text("see this");
    h.key(egui::Key::Enter);
    let reply = |h: &Ui1| {
        h.app
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .messages
            .iter()
            .find(|m| m.id == mid)
            .unwrap()
            .replies
            .last()
            .unwrap()
            .clone()
    };
    let r = reply(&h);
    assert_eq!(r.text, "@Sam Lee see this");
    assert_eq!(r.mentions, vec![sam2], "the one picked, not both");
    assert!(h.app.thread_picked.is_empty(), "used up by the reply");
    // typing "@" opens the same list
    h.click("Reply composer");
    h.type_text("@");
    let rows: Vec<egui::Rect> = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Sam Lee" && !before.contains(r))
        .map(|(_, r)| *r)
        .collect();
    assert!(rows.len() >= 2, "the @ list opens on typing @");
    let first = rows
        .iter()
        .min_by(|a, b| a.center().y.total_cmp(&b.center().y))
        .unwrap();
    h.click_at(first.center());
    assert_eq!(h.app.thread_text, "@Sam Lee ", "the typed @ is replaced");
    h.type_text("too");
    h.key(egui::Key::Enter);
    assert_eq!(reply(&h).mentions, vec![sam]);
}

/// Opens the first space's status editor and, mid-edit, renames "to do" to
/// "backlog" and empties "in progress". Returns (space, a task on "to do",
/// a task on "in progress").
fn status_edit_half_done(h: &mut Ui1) -> (u64, u64, u64) {
    let sid = h.app.data.spaces[0].id;
    let list = h.app.data.lists_in(Location::Space(sid)).unwrap()[0];
    let ids: Vec<u64> = h
        .app
        .data
        .tasks
        .iter()
        .filter(|t| t.list_id == list)
        .map(|t| t.id)
        .collect();
    let (a, b) = (ids[0], ids[1]);
    for (t, st) in [(a, "to do"), (b, "in progress")] {
        h.app
            .data
            .tasks
            .iter_mut()
            .find(|x| x.id == t)
            .unwrap()
            .status = st.into();
    }
    h.app.apply(Action::EditStatuses(sid));
    h.settle();
    h.click("to do");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("backlog");
    h.click("in progress");
    h.key_mods(egui::Key::A, egui::Modifiers::COMMAND);
    h.key(egui::Key::Backspace);
    h.settle();
    assert!(h.app.status_edit.is_some());
    (sid, a, b)
}

#[test]
fn switching_workspace_finishes_the_status_edits_first() {
    let mut h = Ui1::new();
    let (_, a, b) = status_edit_half_done(&mut h);
    // a workspace whose file can't be read: the switch goes no further, so
    // what the switch did first shows in this workspace
    let file = format!("workspace-r5-gone-{:x}.json", crate::sync::random_u64());
    h.app.switch_workspace(&file);
    assert!(h.app.edit_statuses.is_none(), "the editor closed");
    assert!(h.app.status_edit.is_none());
    assert_eq!(
        h.app.task(a).unwrap().status,
        "backlog",
        "the rename reached its tasks"
    );
    let names: Vec<String> = h.app.data.spaces[0]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert!(
        names.contains(&"in progress".to_string()),
        "the emptied name went back: {:?}",
        names
    );
    assert_eq!(h.app.task(b).unwrap().status, "in progress");
}

#[test]
fn joining_a_team_finishes_the_status_edits_and_leaves_the_teams_statuses_alone() {
    let mut h = Ui1::new();
    // the team's copy: the same spaces, with "to do" renamed to "queued"
    let mut team = App::build(None, Some(h.app.data.clone()));
    let st = team.data.spaces[0]
        .statuses
        .iter_mut()
        .find(|s| s.name == "to do")
        .unwrap();
    st.name = "queued".into();
    let folder = std::env::temp_dir().join(format!("wb-r5-join-{:x}", crate::sync::random_u64()));
    team.share_here(folder.to_str().unwrap()).unwrap();
    let team_names: Vec<String> = team.data.spaces[0]
        .statuses
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let (sid, _, _) = status_edit_half_done(&mut h);
    h.app.join_shared(folder.to_str().unwrap()).unwrap();
    assert!(h.app.edit_statuses.is_none(), "the editor closed");
    assert!(h.app.status_edit.is_none(), "and its bookkeeping is gone");
    h.settle();
    // closing anything status-related later puts no old names on the team's
    h.app.finish_status_edit();
    let sp = h.app.data.spaces.iter().find(|s| s.id == sid).unwrap();
    let names: Vec<String> = sp.statuses.iter().map(|s| s.name.clone()).collect();
    assert_eq!(names, team_names, "the team's statuses as they were");
    let _ = std::fs::remove_dir_all(&folder);
}

/// The profile menu, opened at the right end of the top bar, keeps a margin
/// from the window's edge instead of touching it.
#[test]
fn profile_menu_keeps_off_the_window_edge() {
    let mut h = Ui1::new();
    h.click("Profile menu");
    h.settle();
    let r = h.rect("Set status");
    // 8 pt of margin outside the menu's own frame (about 7 pt)
    assert!(
        r.right() <= h.size.x - 14.0,
        "Set status ends at {}",
        r.right()
    );
}

/// Choosing where a Slack person goes in the import dialog sticks: opened
/// again, the dialog starts from that choice (my name, then another member),
/// not from the match it proposed the first time.
#[test]
fn the_slack_import_dialog_remembers_the_last_choice_for_each_person() {
    let mut h = Ui1::new();
    h.app.section = Section::Chat;
    h.settle();
    let root = std::env::temp_dir().join(format!("wb-ui-slack3-{:x}", crate::sync::random_u64()));
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(
        ex.join("users.json"),
        r#"[{"id":"U1","name":"alex","real_name":"Alex Kim","profile":{}}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":["U1"]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","user":"U1","text":"first sketch","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    assert!(std::process::Command::new("/usr/bin/ditto")
        .args(["-c", "-k"])
        .arg(&ex)
        .arg(&zip)
        .status()
        .unwrap()
        .success());
    let chosen = |h: &Ui1| match &h.app.modal {
        Some(Modal::SlackImport(p)) => p.assign.get("U1").copied(),
        _ => panic!("the dialog is open"),
    };
    // proposed as the member Alex Kim (by name); I take them under my name
    h.app.open_slack_export(&zip);
    h.settle();
    let alex = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Alex Kim")
        .unwrap()
        .id;
    assert_eq!(chosen(&h), Some(super::slack::Assign::Member(alex)));
    h.click("Messages from Alex Kim");
    h.click("Import under my name");
    h.click("Import");
    // opened again: my name, as chosen
    h.app.open_slack_export(&zip);
    h.settle();
    assert_eq!(chosen(&h), Some(super::slack::Assign::Me));
    // now Sam Lee instead; the next time it's Sam Lee
    h.click("Messages from Alex Kim");
    h.click("Sam Lee (sam@example.com)");
    let sam = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Sam Lee")
        .unwrap()
        .id;
    assert_eq!(chosen(&h), Some(super::slack::Assign::Member(sam)));
    h.click("Import");
    h.app.open_slack_export(&zip);
    h.settle();
    assert_eq!(chosen(&h), Some(super::slack::Assign::Member(sam)));
    let _ = std::fs::remove_dir_all(&root);
}

/// Radio buttons that are off have a ring that shows on the page (egui's
/// own was border grey on the page colour and vanished in the dark theme).
#[test]
fn radio_buttons_that_are_off_have_a_visible_ring() {
    let mut h = Ui1::new();
    h.app.data.dark = true;
    // Settings > Language: three choices at the top of the page
    h.app.open_settings("Language");
    h.settle();
    fn rings(s: &egui::Shape, out: &mut Vec<egui::epaint::CircleShape>) {
        match s {
            // grey rings of the radio size (status icons are coloured)
            egui::Shape::Circle(c)
                if (c.radius - 6.9).abs() < 0.2
                    && c.stroke.color.r() == c.stroke.color.g()
                    && c.stroke.color.g() == c.stroke.color.b() =>
            {
                out.push(*c)
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| rings(s, out)),
            _ => {}
        }
    }
    let mut found = vec![];
    for c in &h.shapes {
        rings(&c.shape, &mut found);
    }
    assert!(found.len() >= 2, "the choices that are off draw rings");
    let page = h.app.t().bg;
    for r in &found {
        assert!(r.stroke.width >= 1.0);
        let d = (r.stroke.color.r() as i32 - page.r() as i32).abs();
        assert!(
            d >= 50,
            "the ring stands out from the page: {:?}",
            r.stroke.color
        );
    }
}

#[test]
fn black_theme_primary_button_reads_as_enabled_in_dark_mode() {
    let mut h = Ui1::new();
    h.app.data.dark = true;
    h.app.data.extra.accent = "black".into();
    h.click("Rail More");
    h.click("Goals");
    h.settle();
    let t = h.app.t();
    let (r, text) = h.painted_text("Set a Goal")[0];
    // the button's own fill under its label
    fn fills(s: &egui::Shape, p: Pos2, out: &mut Vec<Color32>) {
        match s {
            egui::Shape::Rect(r) if r.rect.contains(p) && r.rect.width() < 300.0 => {
                out.push(r.fill)
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| fills(s, p, out)),
            _ => {}
        }
    }
    let mut found = vec![];
    for c in &h.shapes {
        fills(&c.shape, r.center(), &mut found);
    }
    assert!(
        found.contains(&t.primary),
        "the button is filled: {found:?}"
    );
    // a mid-grey fill with white text looked like a disabled button: the
    // fill now stands out from the page and the label reads on it
    assert!(
        contrast(t.primary, t.bg) >= 3.0,
        "the button stands out from the page"
    );
    assert!(
        contrast(text, t.primary) >= 4.5,
        "the label reads: {text:?}"
    );
    // the other colours keep white text on their own fill
    h.app.data.extra.accent = "purple".into();
    let t = h.app.t();
    assert_eq!(t.on_primary, Color32::WHITE);
    assert!(contrast(t.on_primary, t.primary) >= 4.5);
}

/// Points HOME at a new folder for a test that saves workspace files; the
/// guard keeps the other such tests waiting until it's done.
fn own_home(tag: &str) -> std::sync::MutexGuard<'static, ()> {
    let guard = super::tests::HOME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let home =
        std::env::temp_dir().join(format!("wb-home-{}-{:x}", tag, crate::sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    guard
}

#[test]
fn new_workspace_on_this_computer_is_a_name_and_apps() {
    let _home = own_home("wizard");
    let mut h = Ui1::new();
    let before = h.app.ws_file.clone();
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace { step: 0, .. })
    ));
    assert!(h.has("Step 1 of 2"));
    // step 1: the name
    h.type_text("Side Biz");
    h.click("Next");
    assert!(h.has("Step 2 of 2"));
    // step 2: the apps on the left bar, the default ones already chosen
    h.click("Chat");
    h.click("AI");
    // nobody to invite to a workspace only here: no invite step, one button
    assert!(!h.has("Invite people to your workspace"));
    assert!(!h.has("Skip for now"));
    assert!(!h.has("Next"));
    h.click("Create");
    h.settle();
    assert!(h.app.modal.is_none(), "the dialog closes when it's made");
    assert_ne!(h.app.ws_file, before, "the new workspace is open");
    assert_eq!(h.app.data.workspace, "Side Biz");
    assert_eq!(
        h.app.data.extra.rail_items,
        vec!["Planner", "Teams", "Chat"],
        "the chosen apps, in the bar's order"
    );
    assert_eq!(h.app.data.members.len(), 1, "only me in the new one");
    let me = h.app.data.member(h.app.data.me).unwrap();
    assert_eq!(me.role, "Owner");
    // the workspace menu lists both
    let list = Data::workspaces();
    assert!(list.iter().any(|w| w.file == before));
    assert!(list
        .iter()
        .any(|w| w.name == "Side Biz" && w.file == h.app.ws_file));
}

#[test]
fn new_workspace_needs_a_name_and_goes_back() {
    let _home = own_home("wizard-skip");
    let mut h = Ui1::new();
    let members_before = h.app.data.members.len();
    h.click("Workspace menu");
    h.click("Create Workspace");
    // no name yet: Next does nothing
    h.click("Next");
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace { step: 0, .. })
    ));
    h.type_text("Acme");
    // Enter in the name moves on, like Next
    h.key(egui::Key::Enter);
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace { step: 1, .. })
    ));
    // back a step: what was typed is still there
    h.click("Back");
    match &h.app.modal {
        Some(Modal::NewWorkspace { step, name, .. }) => {
            assert_eq!(*step, 0);
            assert_eq!(name, "Acme");
        }
        _ => panic!("the dialog is still open"),
    }
    h.click("Next");
    h.click("Create");
    h.settle();
    assert!(h.app.modal.is_none());
    assert_eq!(h.app.data.workspace, "Acme");
    assert_eq!(h.app.data.members.len(), 1, "only me in the new one");
    assert!(members_before > 1, "the seed had others");
    assert_eq!(
        h.app.data.extra.rail_items,
        Extra::default().rail_items,
        "untouched apps stay the default"
    );
}

#[test]
fn a_new_workspace_can_go_on_the_team_server_or_stay_here() {
    let _home = own_home("wizard-server");
    let mut h = Ui1::new();
    // this workspace is on a team server, signed in with an account
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 1,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.account = Some("me@team.io".into());
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    together::save_token("127.0.0.1:1", "dev1", "tok");
    let before = h.app.ws_file.clone();

    // on the server is the choice it starts with
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(h.has("On the team server (127.0.0.1:1)"));
    assert!(h.has("On this computer only"));
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace {
            on_server: true,
            ..
        })
    ));
    h.type_text("Side Biz");
    h.click("Next");
    h.click("Next");
    assert!(h.has("The server emails each of them an invite code once the workspace is up."));
    h.type_text("helper@team.io");
    h.click("Create and invite");
    h.settle();
    assert!(h.app.modal.is_none());
    assert!(
        h.app.making_ws.is_some(),
        "asked the server to make it; it opens when the server answers"
    );
    assert_eq!(h.app.ws_file, before, "nothing opens before that");
    h.app.making_ws = None;

    // or only on this computer, as before
    h.click("Workspace menu");
    h.click("Create Workspace");
    h.click("On this computer only");
    assert!(matches!(
        h.app.modal,
        Some(Modal::NewWorkspace {
            on_server: false,
            ..
        })
    ));
    h.type_text("Notes");
    h.click("Next");
    assert!(h.has("Step 2 of 2"), "no invite step when it stays here");
    assert!(!h.has("Invite people to your workspace"));
    h.click("Create");
    h.settle();
    assert!(h.app.making_ws.is_none());
    assert_ne!(h.app.ws_file, before);
    assert_eq!(h.app.data.workspace, "Notes");
    assert!(h.app.data.extra.shared.is_none());
}

#[test]
fn a_workspace_on_this_computer_only_has_no_server_choice() {
    let _home = own_home("wizard-local");
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Create Workspace");
    assert!(!h.has("On this computer only"));
    assert!(!h.has("Where"));
}
