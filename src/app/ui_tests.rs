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
}

impl Ui1 {
    fn new() -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut h = Ui1 { ctx, app: App::build(None, Some(Data::seed())), time: 0.0, tree: vec![] };
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 900.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let out = self.ctx.run(input, |ctx| app.ui_frame(ctx));
        if let Some(update) = out.platform_output.accesskit_update {
            self.tree.clear();
            for (_, node) in update.nodes {
                let Some(b) = node.bounds() else { continue };
                let rect = Rect::from_min_max(pos2(b.x0 as f32, b.y0 as f32), pos2(b.x1 as f32, b.y1 as f32));
                for name in [node.label(), node.placeholder(), node.value()].into_iter().flatten() {
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
        self.tree.iter().rev().find(|(n, _)| n == name).map(|(_, r)| r.center())
    }

    fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    // scroll with the mouse wheel over a control until it is on screen
    fn scroll_into_view(&mut self, name: &str) {
        for _ in 0..40 {
            let Some(p) = self.find(name) else { return };
            if p.y > 40.0 && p.y < 880.0 {
                return;
            }
            let at = pos2(p.x, 450.0);
            let dy = if p.y >= 880.0 { -120.0 } else { 120.0 };
            self.frame(vec![egui::Event::PointerMoved(at)]);
            self.frame(vec![egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, dy), modifiers: Default::default() }]);
            self.settle();
        }
    }

    fn click(&mut self, name: &str) {
        self.scroll_into_view(name);
        let pos = self.find(name).unwrap_or_else(|| {
            let mut names: Vec<&String> = self.tree.iter().map(|(n, _)| n).collect();
            names.sort();
            names.dedup();
            panic!("nothing named {:?} on screen; visible names: {:?}", name, names)
        });
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        self.frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        self.settle();
    }

    fn click_at(&mut self, pos: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        self.frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        self.settle();
    }

    // click near the left end of a wide control (a task row's name)
    fn click_left(&mut self, name: &str, dx: f32) {
        let r = self.tree.iter().rev().find(|(n, _)| n == name).map(|(_, r)| *r).unwrap_or_else(|| panic!("nothing named {:?}", name));
        self.click_at(pos2(r.left() + dx, r.center().y));
    }

    fn right_click(&mut self, name: &str, dx: f32) {
        self.scroll_into_view(name);
        let r = self.tree.iter().rev().find(|(n, _)| n == name).map(|(_, r)| *r).unwrap_or_else(|| panic!("nothing named {:?}", name));
        let pos = pos2(r.left() + dx, r.center().y);
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed: true, modifiers: Default::default() }]);
        self.frame(vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed: false, modifiers: Default::default() }]);
        self.settle();
    }

    // press on one control, move the mouse in small steps to another, release
    fn drag(&mut self, from: &str, to: &str) {
        let a = self.find(from).unwrap_or_else(|| panic!("nothing named {:?}", from));
        let b = self.find(to).unwrap_or_else(|| panic!("nothing named {:?}", to));
        self.frame(vec![egui::Event::PointerMoved(a)]);
        self.frame(vec![egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }]);
        for i in 1..=12 {
            let p = a + (b - a) * (i as f32 / 12.0);
            self.frame(vec![egui::Event::PointerMoved(p)]);
        }
        self.frame(vec![egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }]);
        self.settle();
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
        self.frame(vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        self.frame(vec![egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Default::default() }]);
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
    let pl = h.app.personal_list_id().expect("personal list created by the click");
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
    assert!(h.app.data.tasks.iter().any(|t| t.name == "Clicked into existence"));
}

#[test]
fn click_add_task_row_in_list() {
    let mut h = Ui1::new();
    h.click("Add Task in TO DO");
    h.type_text("Inline task");
    h.key(egui::Key::Enter);
    let t = h.app.data.tasks.iter().find(|t| t.name == "Inline task").expect("added from the list row");
    assert_eq!(t.status, "to do");
}

#[test]
fn click_status_circle_and_pick_status() {
    let mut h = Ui1::new();
    h.click("Status of Set up analytics dashboard");
    h.click("IN PROGRESS");
    let t = h.app.data.tasks.iter().find(|t| t.name == "Set up analytics dashboard").unwrap();
    assert_eq!(t.status, "in progress");
}

#[test]
fn click_task_row_opens_it_then_complete_and_timer() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.expect("task window opens");
    h.click("\u{25B6}");
    assert!(h.app.task(id).unwrap().timer_started.is_some(), "the play button starts the timer");
    h.click("\u{2714}");
    assert_eq!(h.app.task(id).unwrap().status, "complete", "the check button completes the task");
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
    assert_eq!(h.app.data.members.last().unwrap().email, "lee.jiwoo@team.kr");
    assert!(h.app.modal.is_none(), "the dialog closes after sending");
}

#[test]
fn click_view_tabs() {
    let mut h = Ui1::new();
    for (name, v) in [("View Board", ViewKind::Board), ("View Calendar", ViewKind::Calendar), ("View Gantt", ViewKind::Gantt), ("View Table", ViewKind::Table), ("View List", ViewKind::List)] {
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
    let n = h.app.data.channels.iter().find(|c| c.id == cid).unwrap().messages.len();
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
    let w = h.app.data.extra.whiteboards.last().expect("a whiteboard was made");
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
    assert_eq!(h.app.data.extra.forms.iter().find(|f| f.id == fid).unwrap().fields.len(), 3);
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
    let row = h.find("Inbox Primary").map(|p| pos2(700.0, p.y + 120.0)).unwrap();
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
    h.drag("Card Write copy for landing page", "Card Fix navigation on mobile");
    let t = h.app.data.tasks.iter().find(|t| t.name == "Write copy for landing page").unwrap();
    assert_eq!(t.status, "in progress", "dropping a card in another column moves it");
}

#[test]
fn right_click_delete_then_restore_from_trash() {
    let mut h = Ui1::new();
    h.right_click("Task Collect feedback from beta users", 120.0);
    h.click("Delete");
    assert!(!h.app.data.tasks.iter().any(|t| t.name == "Collect feedback from beta users"));
    h.click("Profile menu");
    h.click("Trash");
    assert!(matches!(h.app.modal, Some(Modal::Trash)));
    h.click("Restore");
    assert!(h.app.data.tasks.iter().any(|t| t.name == "Collect feedback from beta users"), "restored from the trash");
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
    for item in ["Apps", "Templates", "Custom Fields", "Automations", "Tag Manager"] {
        h.click("Workspace menu");
        h.click(item);
        assert!(h.app.modal.is_some(), "{} opens a dialog", item);
        h.key(egui::Key::Escape);
        assert!(h.app.modal.is_none(), "Esc closes {}", item);
    }
    for item in ["Notifications", "Themes", "Labs", "Keyboard shortcuts", "Help"] {
        h.click("Profile menu");
        h.click(item);
        assert!(h.app.modal.is_some(), "{} opens a dialog", item);
        h.key(egui::Key::Escape);
        assert!(h.app.modal.is_none(), "Esc closes {}", item);
    }
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
    for (item, sec) in [("Docs", Section::Docs), ("Dashboards", Section::Dashboards), ("Whiteboards", Section::Whiteboards), ("Forms", Section::Forms), ("Goals", Section::Goals), ("Timesheets", Section::Timesheets)] {
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
    assert_eq!(m.replies.last().unwrap().text, "on it", "reply posted in the thread");
    let before = m.reactions.iter().map(|r| r.users.len()).sum::<usize>();

    // hover the last message to show its toolbar, then react with a party emoji
    let p = h.find("@Me can you check the beta feedback list when you have a minute?").unwrap();
    h.hover(p);
    h.click("\u{1F389}");
    let ch = h.app.data.channels.iter().find(|c| c.id == cid).unwrap();
    let last = ch.messages.iter().find(|m| m.text.starts_with("@Me")).unwrap();
    assert!(last.reactions.iter().any(|r| r.emoji == "\u{1F389}"), "reaction added from the hover toolbar");
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
    let m = h.app.data.channels.iter().find(|c| c.id == ch).unwrap().messages.last().unwrap().clone();
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
fn rail_has_no_labs_or_upgrade_and_empty_chat_can_start_a_channel() {
    let mut h = Ui1::new();
    assert!(!h.has("Rail Labs") && !h.has("Rail Upgrade"));
    assert!(h.has("Rail Install") && h.has("Rail Invite"));
    h.app.data.channels.clear();
    h.app.section = Section::Chat;
    h.app.chat_open = None;
    h.settle();
    h.click("Create a Channel");
    assert_eq!(h.app.data.channels.len(), 1);
    assert_eq!(h.app.data.channels[0].name, "general");
    assert_eq!(h.app.chat_open, Some(h.app.data.channels[0].id));
}
