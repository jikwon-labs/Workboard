// Click-through tests: the whole interface runs in a 1440x900 window and
// receives real pointer moves, presses, releases and typed text at the
// on-screen position of each control (found by its name), then the result
// is checked. This is the same input path a mouse click takes.

use super::*;
use egui::Pos2;

struct Ui1 {
    ctx: egui::Context,
    app: App,
    time: f64,
    tree: Vec<(String, egui::Rect)>,
    size: egui::Vec2,
    shapes: Vec<egui::epaint::ClippedShape>,
    /// files dropped on the window in the next frame
    dropped: Vec<egui::DroppedFile>,
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
        };
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
    h.click("\u{25B6}");
    assert!(
        h.app.task(id).unwrap().timer_started.is_some(),
        "the play button starts the timer"
    );
    h.click("\u{2714}");
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
    h.click("Send free invite");
    assert_eq!(h.app.data.members.len(), n + 1);
    assert_eq!(
        h.app.data.members.last().unwrap().email,
        "lee.jiwoo@team.kr"
    );
    assert!(h.app.modal.is_none(), "the dialog closes after sending");
}

#[test]
fn click_view_tabs() {
    let mut h = Ui1::new();
    for (name, v) in [
        ("View Board", ViewKind::Board),
        ("View Calendar", ViewKind::Calendar),
        ("View Gantt", ViewKind::Gantt),
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
    h.click("\u{1F465}  People");
    assert_eq!(h.app.section, Section::People);
    h.click("Profile menu");
    h.click("Themes");
    assert!(matches!(h.app.modal, Some(Modal::Themes)));
    h.click("\u{1F319} Dark");
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
    h.click("\u{1F441} Fill out");
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
    h.click("Create Team");
    assert_eq!(h.app.data.extra.teams.len(), n + 1);
}

#[test]
fn click_settings_dialogs_open_and_close() {
    let mut h = Ui1::new();
    for item in [
        "Apps",
        "Templates",
        "Custom Fields",
        "Automations",
        "Tag Manager",
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
        h.tree
            .iter()
            .any(|(n, _)| n.starts_with("Status: Reconnecting")),
        "shows that it is reconnecting"
    );
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
        },
        wb_voice::Person {
            identity: format!("{}-dev2", other.id),
            name: String::new(),
            me: false,
            speaking: true,
            muted: false,
            heard: 0.3,
            camera: true,
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
    h.click("\u{2699}  Settings");
    assert_eq!(h.app.section, Section::Settings);
    h.click("Settings Spaces");
    assert!(h.has("Team Space"));
    h.click("Settings Themes");
    h.click("\u{25D0} Auto");
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
    h.click("\u{1F3F7} Tags");
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
    h.click("\u{25A6}  Table");
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
    h.click("\u{1F4C4}  Subpage");
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
    assert!(h.has("@  Mention a teammate") && h.has("\u{1F464}  Assign this task"));
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
    h.click("\u{1F464}  Assign this task");
    h.click_side("Sam Lee", true);
    h.settle();
    assert!(h.app.task(id).unwrap().assignees.contains(&3));
}

#[test]
fn fields_panel_searches_and_adds_property_columns() {
    let mut h = Ui1::new();
    assert!(!h.app.col_on("Created by"), "properties start hidden");
    h.click("\u{25A5}  Columns");
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
    h.click("\u{2261}  Filter");
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
    h.click("\u{2699}  Customize");
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
    h.click("Restore Team Space / Marketing");
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
        let cols_chip = h.rect("\u{25A5}  Columns");
        let sort = h.rect("\u{21C5}  Sort: Manual");
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
    assert!(w.rect("\u{25A5}  Columns").width() > 60.0 && w.has("Team Space") && w.has("Projects"));
}

#[test]
fn short_window_profile_menu_scrolls_to_trash_and_log_out() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.click("Profile menu");
    h.settle();
    let menu_top = h.rect("Settings").top();
    // the menu does not run off the window: its bottom row is reached by
    // scrolling inside the menu
    for name in ["Trash", "Log out"] {
        h.scroll_into_view(name);
        let r = h.rect(name);
        assert!(
            r.top() >= menu_top - 400.0 && r.bottom() <= 560.0,
            "{} is on screen after scrolling the menu: {:?}",
            name,
            r
        );
    }
    h.click("Log out");
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
        "\u{23F1}  Track time",
        "\u{2691}  Priority",
        "\u{1F464}  Assignees",
        "\u{231B}  Time estimate",
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
    let (st, asg) = (w.rect("\u{25CE}  Status"), w.rect("\u{1F464}  Assignees"));
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
        assert!(best(c) >= 3.0, "Post at {:?} contrast {:.1}", r, best(c));
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
    let dates = h.rect("\u{1F4C5}  Dates");
    let est = h.rect("\u{231B}  Time estimate");
    let tags = h.rect("\u{1F3F7}  Tags");
    let step = h.rect("\u{1F464}  Assignees").center().y - h.rect("\u{25CE}  Status").center().y;
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
    h.click("Search tasks");
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
    h.click("Search tasks");
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
    h.click("Search tasks");
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
    h.click("Search tasks");
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
        pos: [40.0, 40.0],
        text: "One".into(),
        color: [0xf8, 0xae, 0x00],
    });
    wb.strokes.push(WbStroke {
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
fn team_color_and_my_teams() {
    let mut h = Ui1::new();
    h.app.section = Section::Teams;
    h.settle();
    h.click_side("Create Team", true);
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
    h.app.sweep.entry_edit.as_mut().unwrap().3 = 2.0;
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

#[test]
fn the_first_start_offers_working_alone_joining_or_a_new_server() {
    let mut h = Ui1::new();
    assert!(
        Data::fresh().extra.welcome,
        "a first start shows the welcome"
    );
    let old: Data = serde_json::from_str(
        &serde_json::to_string(&Data::seed())
            .unwrap()
            .replace(r#","welcome":false"#, ""),
    )
    .unwrap();
    assert!(!old.extra.welcome, "files from before don't");
    h.app.modal = Some(Modal::Welcome {
        step: 0,
        name: String::new(),
        addr: String::new(),
    });
    h.settle();
    assert!(
        h.has("Use it on this Mac")
            && h.has("Join my team")
            && h.has("Set up a server for my team")
    );

    // alone: just a name
    h.click("Use it on this Mac");
    assert!(h.has("Your name entry"));
    let me = h.app.data.me;
    h.app.modal = Some(Modal::Welcome {
        step: 1,
        name: "Jiwon".into(),
        addr: String::new(),
    });
    h.settle();
    h.click("Start");
    assert!(h.app.modal.is_none());
    assert_eq!(h.app.data.member(me).unwrap().name, "Jiwon");

    // join: server, then email and password or an invite code
    h.app.modal = Some(Modal::Welcome {
        step: 0,
        name: String::new(),
        addr: String::new(),
    });
    h.settle();
    h.click("Join my team");
    assert!(
        h.has("Join your team")
            && h.has("Server address")
            && h.has("Email entry")
            && h.has("Invite code")
    );
    h.click("Server address");
    h.type_text("127.0.0.1:1");
    h.click("Email entry");
    h.type_text("me@team.io");
    h.click("Password entry");
    h.type_text("secret-pass");
    h.settle();
    h.click("Connect");
    let j = h.app.sync_rt.joining.as_ref().expect("connecting");
    assert_eq!(
        j.how,
        together::How::Account {
            email: "me@team.io".into(),
            password: "secret-pass".into()
        }
    );
    assert!(h.has("Cancel"));
    h.click("Cancel");

    // a new server: the owner's email and password
    h.click("Back");
    h.click("Set up a server for my team");
    assert!(
        h.has("Set up your team's server")
            && h.has("Repeat password entry")
            && !h.has("Invite code entry")
    );
}

#[test]
fn an_invite_link_opens_the_join_form_filled_in() {
    let mut h = Ui1::new();
    h.app.open_join_link("workboard://join?server=mac-mini.tail1.ts.net%3A8787&code=ABCD-EFGH-JKMN&email=bo%40team.io");
    h.settle();
    assert!(
        matches!(&h.app.modal, Some(Modal::Welcome { step: 2, addr, .. }) if addr == "mac-mini.tail1.ts.net:8787")
    );
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
    h.app.modal = Some(Modal::Welcome {
        step: 2,
        name: String::new(),
        addr: String::new(),
    });
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
    assert!(matches!(&h.app.modal, Some(Modal::Welcome { addr, .. }) if addr == "macmini"));

    // already on a server: says so instead of switching
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app
        .open_join_link("workboard://join?server=other&code=ABCD-EFGH-JKMN");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|t| t.0.contains("already on 127.0.0.1:1")));
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
    h.click(">");
    assert_eq!(h.app.sheet_week, ws + Duration::days(7));
    assert!(!h.has("Remove row Renew passport"), "not in the next week");
    h.click("<");
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
    h.click("\u{25B6}");
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
    h.click("\u{1F464} Manage people");
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
