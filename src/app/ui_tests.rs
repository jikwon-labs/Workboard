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

    // click the left-most (or right-most) control with this name, for names
    // shown twice (sidebar and page)
    fn click_side(&mut self, name: &str, rightmost: bool) {
        let pick = |h: &Self| {
            let mut hits: Vec<Rect> = h.tree.iter().filter(|(n, _)| n == name).map(|(_, r)| *r).collect();
            hits.sort_by(|a, b| a.center().x.total_cmp(&b.center().x));
            if rightmost { hits.last() } else { hits.first() }.copied().unwrap_or_else(|| panic!("nothing named {:?}", name))
        };
        for _ in 0..30 {
            let r = pick(self);
            if r.center().y > 40.0 && r.center().y < 880.0 {
                break;
            }
            let dy = if r.center().y >= 880.0 { -120.0 } else { 120.0 };
            self.frame(vec![egui::Event::PointerMoved(pos2(r.center().x, 450.0))]);
            self.frame(vec![egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, dy), modifiers: Default::default() }]);
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

#[test]
fn sidebar_add_channel_takes_enter_and_lets_go_on_escape() {
    let mut h = Ui1::new();
    let n = h.app.data.channels.len();
    h.click_side("Add Channel", false);
    assert!(h.app.new_channel.is_some());
    h.type_text("dev team");
    h.key(egui::Key::Enter);
    assert_eq!(h.app.data.channels.len(), n + 1, "Enter creates the channel");
    assert!(h.app.data.channels.iter().any(|c| c.name == "dev-team"));
    assert!(h.app.new_channel.is_none());
    // Escape leaves the field and typing goes elsewhere again
    h.app.section = Section::Home;
    h.settle();
    h.click_side("Add Channel", false);
    assert!(h.app.new_channel.is_some());
    h.key(egui::Key::Escape);
    assert!(h.app.new_channel.is_none(), "Escape closes the field");
    assert!(h.ctx.memory(|m| m.focused().is_none()), "nothing keeps the keyboard");
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
    assert!(matches!(h.app.modal, Some(Modal::NewList { .. })), "Create List dialog");
    h.type_text("Roadmap");
    h.key(egui::Key::Enter);
    let Location::List(id) = h.app.loc else { panic!("opened the new list") };
    assert_eq!(h.app.data.list_name(id), "Roadmap");
    assert!(h.app.data.space_of_list(id).unwrap().private_owner.is_none());
}

#[test]
fn right_click_task_move_to_another_list() {
    let mut h = Ui1::new();
    h.right_click("Task Set up analytics dashboard", 60.0);
    h.click("Move to...");
    assert!(matches!(h.app.modal, Some(Modal::MoveTasks { .. })));
    h.click("Team Space / Projects / Marketing");
    let t = h.app.data.tasks.iter().find(|t| t.name == "Set up analytics dashboard").unwrap();
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
    assert!(h.app.data.tasks.iter().any(|t| t.name == "Set up analytics dashboard (copy)"));
}

#[test]
fn single_key_t_opens_create_task_and_cmd_backslash_hides_the_sidebar() {
    let mut h = Ui1::new();
    h.key(egui::Key::T);
    assert!(h.app.quick_add.is_some());
    h.app.quick_add = None;
    h.settle();
    let cmd = egui::Modifiers { command: true, mac_cmd: true, ..Default::default() };
    h.frame(vec![egui::Event::Key { key: egui::Key::Backslash, physical_key: None, pressed: true, repeat: false, modifiers: cmd }]);
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
    let id = h.app.data.spaces.iter().flat_map(|s| s.folders.iter().flat_map(|f| f.lists.iter()).chain(s.lists.iter())).find(|l| l.name == "Marketing").unwrap().id;
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
    h.click("\u{1F4C4}  Project brief");
    assert_eq!(h.app.section, Section::Docs);
}

#[test]
fn doc_slash_command_and_sub_page() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    // the empty body has no visible name to click; put the cursor in it
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new(("doc_body", id))));
    h.settle();
    h.type_text("/");
    h.settle();
    h.click("Heading 1");
    h.type_text("Plan");
    assert_eq!(h.app.data.docs.iter().find(|d| d.id == id).unwrap().body, "# Plan");
    h.click("+  Add page");
    assert!(h.app.data.docs.iter().any(|d| d.parent == Some(id)));
}

// ---------------------------------------------------------------- round 2 bugs

fn pri(h: &Ui1, name: &str) -> u8 {
    h.app.data.tasks.iter().find(|t| t.name == name).unwrap().priority
}

#[test]
fn bulk_priority_opening_the_menu_changes_nothing() {
    let mut h = Ui1::new();
    let a = "Write copy for landing page";
    let b = "Set up analytics dashboard";
    for t in h.app.data.tasks.iter_mut().filter(|t| t.name == a || t.name == b) {
        t.priority = 2;
    }
    h.hover(h.find(&format!("Task {}", a)).unwrap());
    h.click(&format!("Select {}", a));
    h.click(&format!("Select {}", b));
    assert!(h.has("2 tasks selected"));
    h.click("Priority");
    assert!(h.has("Clear"), "the priority menu is open");
    h.settle();
    assert_eq!((pri(&h, a), pri(&h, b)), (2, 2), "just opening the menu keeps the priority");
    h.click("Urgent");
    assert_eq!((pri(&h, a), pri(&h, b)), (1, 1));
    h.click("Priority");
    h.click("Clear");
    assert_eq!((pri(&h, a), pri(&h, b)), (0, 0), "Clear still clears");
}

impl Ui1 {
    fn rect(&self, name: &str) -> Rect {
        self.tree.iter().rev().find(|(n, _)| n == name).map(|(_, r)| *r).unwrap_or_else(|| panic!("nothing named {:?}", name))
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
    assert!(sub.left() >= parent.right() - 2.0, "Create new opens to the right: parent {:?} sub {:?}", parent, sub);
    assert!(h.has("Task statuses"), "the parent menu stays open");
    h.key(egui::Key::Escape);

    // list row "..." > Status
    h.hover(h.find("Task Set up analytics dashboard").unwrap());
    h.click("Task options Set up analytics dashboard");
    let parent = h.rect("Status");
    h.hover(parent.center());
    let sub = h.rect("IN PROGRESS");
    assert!(sub.left() >= parent.right() - 2.0, "Status opens to the right: parent {:?} sub {:?}", parent, sub);
    h.click("IN PROGRESS");
    let t = h.app.data.tasks.iter().find(|t| t.name == "Set up analytics dashboard").unwrap();
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
    assert!(item.right() <= settings.left() || item.left() >= settings.right(), "flyout {:?} sits beside the menu row {:?}", item, settings);
    // walk the pointer from the row to the flyout; it must stay open
    let to = item.center();
    for i in 1..=8 {
        let p = row.center() + (to - row.center()) * (i as f32 / 8.0);
        h.frame(vec![egui::Event::PointerMoved(p)]);
    }
    h.settle();
    h.click("For 1 hour");
    assert!(h.app.data.extra.muted_until > now_ts(), "muted from the flyout");
}

#[test]
fn date_picker_fits_its_content_and_shows_set_recurring() {
    let mut h = Ui1::new();
    let mut rows: Vec<(String, Rect)> = h.tree.iter().filter(|(n, _)| n.starts_with("Due date of ")).cloned().collect();
    rows.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    assert!(rows.len() >= 2);
    for (name, _) in [rows.first().unwrap().clone(), rows.last().unwrap().clone()] {
        h.click(&name);
        let today = h.rect("Today");
        let weekly = h.rect("Weekly");
        assert!(weekly.bottom() <= 900.0 && weekly.top() >= 0.0, "{}: Set Recurring is on screen ({:?})", name, weekly);
        assert!(weekly.top() - today.top() < 380.0, "{}: the menu is only as tall as its content ({:?} .. {:?})", name, today, weekly);
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
    assert_eq!(h.app.selected.len(), 1, "Escape on the menu keeps the selection");
}

#[test]
fn chat_drafts_stay_with_their_channel() {
    let mut h = Ui1::new();
    h.click("general");
    let a = h.app.chat_open.unwrap();
    h.click("Message composer");
    h.type_text("hello");
    // move to another channel, write and send there
    let b = h.app.data.channels.iter().find(|c| c.id != a).expect("seed has two channels").clone();
    h.click_side(&b.name, false);
    assert_eq!(h.app.chat_open, Some(b.id));
    assert_eq!(h.app.chat_text, "", "the other channel starts empty");
    h.click("Message composer");
    h.type_text("first message");
    h.key(egui::Key::Enter);
    let sent = h.app.data.channels.iter().find(|c| c.id == b.id).unwrap().messages.last().unwrap().text.clone();
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
        let base = h.app.data.spaces.iter().flat_map(|s| s.lists.iter()).next().cloned().or_else(|| h.app.data.spaces.iter().flat_map(|s| s.folders.iter().flat_map(|f| f.lists.iter())).next().cloned()).unwrap();
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
        let r = h.ctx.read_response(egui::Id::new("new_channel_input")).expect("the field is drawn").rect;
        assert!(r.top() >= 40.0 && r.bottom() <= 880.0, "via_create={}: the field is on screen ({:?})", via_create, r);
        h.type_text("design");
        h.key(egui::Key::Enter);
        assert!(h.app.data.channels.iter().any(|c| c.name == "design"));
    }
}

#[test]
fn search_hides_empty_group_headings() {
    let mut h = Ui1::new();
    assert!(!h.app.data.tasks.iter().any(|t| t.name.to_lowercase().contains("gen")), "seed check");
    // distinct places on screen with this name
    let count = |h: &Ui1, n: &str| {
        let mut r: Vec<String> = h.tree.iter().filter(|(x, _)| x == n).map(|(_, r)| format!("{:?}", r)).collect();
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
    assert_eq!(count(&h, "Tasks"), tasks_with - 1, "no empty Tasks heading (the Tasks tab stays)");
}

#[test]
fn my_docs_badge_counts_docs_not_sub_pages() {
    let mut h = Ui1::new();
    let before = h.app.my_docs_count();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    h.click("+  Add page");
    assert!(h.app.data.docs.iter().any(|d| d.parent == Some(id)), "a sub page was made");
    assert_eq!(h.app.my_docs_count(), before + 1, "the sub page is not counted");
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
            assert!(ctx.fonts(|x| x.has_glyph(&f, c)), "{:?} has a glyph in {:?}", c, f);
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
    assert!(h.has("Task Name or type '/' for commands"));
    h.type_text("Full task");
    h.click("Add description");
    h.type_text("Some details");
    h.click("Task status");
    h.click("IN PROGRESS");
    h.click("\u{1F3F7} Tags");
    h.type_text("urgent-fix");
    h.key(egui::Key::Enter);
    h.key(egui::Key::Escape);
    assert!(h.app.quick_add.is_some(), "Escape closes the tag popup first");
    h.click_side("Create Task", true); // the button, right of the heading
    let t = h.app.data.tasks.iter().find(|t| t.name == "Full task").expect("created");
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
    assert_eq!(kinds, vec![LineKind::Heading(1), LineKind::Bullet, LineKind::Check(true), LineKind::Quote, LineKind::Plain]);
    let t = theme::theme_accent(false, "");
    let job = doc_layout_job(text, &t, 600.0);
    assert_eq!(job.text, text, "the stored Markdown is laid out unchanged");
    let fmt_at = |needle: &str| {
        let b = job.text.find(needle).unwrap();
        job.sections.iter().find(|s| s.byte_range.contains(&b)).unwrap().format.clone()
    };
    assert_eq!(fmt_at("Title").font_id.size, 28.0, "heading is big while editing");
    assert!(fmt_at("quote").italics);
    assert!(fmt_at("done").strikethrough.width > 0.0, "a ticked item is struck through");
    assert_eq!(fmt_at("- item").color, Color32::TRANSPARENT, "the bullet marker is replaced by a painted dot");
}

#[test]
fn doc_checkbox_toggles_and_new_page_opens_in_edit() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    assert!(h.has("Start writing"), "an empty page offers how to start");
    h.click("\u{25A6}  Table");
    assert!(h.app.data.docs.iter().find(|d| d.id == id).unwrap().body.starts_with("| Column 1"));
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
    let ch = h.app.data.channels.iter().find(|c| c.name == "website-redesign").unwrap().id;
    assert_eq!(h.app.section, Section::Chat);
    assert_eq!(h.app.chat_open, Some(ch));
    assert!(!h.app.new_message_open);
    // reopened, the old search is gone; the avatar row opens a DM
    h.app.new_message_open = true;
    h.settle();
    assert!(h.has("# general"));
    let other = h.app.data.members.iter().find(|m| m.id != h.app.data.me).unwrap().clone();
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
    assert!(matches!(h.app.modal, Some(Modal::NewChannel { .. })), "Create Channel window");
    h.type_text("Design Team");
    h.click_side("Make Private", true); // the switch, right of its label
    h.click_side("Add a List", true);
    h.click_side("Create", true);
    let c = h.app.data.channels.iter().find(|c| c.name == "design-team").expect("channel made").clone();
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
    h.app.modal = Some(Modal::NewChannel { name: String::new(), private: false, add_list: false });
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
    h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap().checklist.clear();
    h.settle();
    assert!(!h.has("Subtask name"), "no subtask field until the row is clicked");
    h.click("+  Add subtask");
    h.type_text("Child one");
    h.key(egui::Key::Enter);
    h.app.apply_actions();
    h.settle();
    assert!(h.app.data.tasks.iter().any(|t| t.parent == Some(id) && t.name == "Child one"));
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
    h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap().activity.retain(|a| !a.comment);
    h.settle();
    // empty: the two shortcut buttons
    assert!(h.has("@  Mention a teammate") && h.has("\u{1F464}  Assign this task"));
    h.click("@  Mention a teammate");
    assert!(h.ctx.memory(|m| m.any_popup_open()), "the people list opens");
    h.click("Alex Kim");
    assert_eq!(h.app.comment_text, "@Alex Kim ");
    h.type_text("please check");
    h.click("Emoji");
    h.click("\u{1F44D}");
    h.click("Send Comment");
    let t = h.app.task(id).unwrap().clone();
    let c = t.activity.iter().rev().find(|a| a.comment).unwrap();
    assert_eq!(c.text, "@Alex Kim please check\u{1F44D}");
    assert!(!c.post);
    // Post tab
    h.click("Post");
    h.click("Comment box");
    h.type_text("Release notes");
    h.click("Send Post");
    assert!(h.app.task(id).unwrap().activity.iter().any(|a| a.post && a.text == "Release notes"));
    // search narrows the comments
    h.click("Search comments button");
    h.type_text("release");
    assert!(h.has("Release notes"));
    assert!(!h.has("@Alex Kim please check\u{1F44D}"), "the other comment is hidden");
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
    let cols = super::shell::col_layout(Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 30.0)), &h.app.hidden_cols, true);
    assert_eq!(cols.extra.iter().map(|c| c.0).collect::<Vec<_>>(), vec!["Created by"]);
    let t = h.app.data.tasks.iter().find(|t| t.name == "Write copy for landing page").unwrap();
    assert_eq!(h.app.prop_text(t, "Created by"), h.app.data.member(t.activity[0].author).unwrap().name);
}

#[test]
fn closing_a_task_records_date_done_and_closed() {
    let mut h = Ui1::new();
    let mut t = h.app.data.tasks.iter().find(|t| t.name == "Write copy for landing page").unwrap().clone();
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
    assert!(h.app.task(t.id).unwrap().closed_at.is_none(), "reopening clears it");
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
    assert!(h.app.filter_open, "the panel stays while a drop-down closes");
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
    assert!(h.app.filtered().iter().all(|t| t.priority == 1 && t.status != "to do"));
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
    let sid = h.app.data.spaces.iter().find(|s| s.private_owner.is_none()).unwrap().id;
    h.app.actions.push(Action::Go(Location::Space(sid)));
    h.app.apply_actions();
    h.app.view = ViewKind::Overview;
    h.settle();
    let lid = h.app.data.spaces.iter().find(|s| s.id == sid).unwrap().lists[0].id;
    let lname = h.app.data.list_name(lid);
    assert!(h.has(&format!("Overview list {}", lname)));
    // Priority and Owner cells edit the List
    h.click(&format!("Priority of {}", lname));
    h.click("Urgent");
    assert_eq!(h.app.data.list_item(lid).unwrap().priority, 1);
    h.click(&format!("Owner of {}", lname));
    h.click_side("Sam Lee", true);
    assert_eq!(h.app.data.list_item(lid).unwrap().owner, Some(3));
    assert!(matches!(h.app.loc, Location::Space(_)), "editing a cell does not open the List");
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
    assert!(!h.has("View Calendar") && !h.has("View Gantt") && !h.has("View Table"), "only Board and List");
    assert!(!h.has("Add Channel") || h.tree.iter().filter(|(n, _)| n == "Add Channel").all(|(_, r)| r.left() < 320.0), "no Add Channel button on the page");
    // empty: an inline Add Task row
    h.click("Add Task in ALL TASKS");
    h.type_text("Buy milk");
    h.key(egui::Key::Enter);
    assert!(h.app.data.tasks.iter().any(|t| t.list_id == pl && t.name == "Buy milk"));
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
    assert!(h.app.quick_add.is_none(), "the + opens a menu, not the Create Task window");
    for item in ["Create Task", "Create List", "Create Doc", "Create Dashboard", "Create Whiteboard", "Create Form"] {
        assert!(h.has(item), "{} in the menu", item);
    }
    h.click("Create Task");
    assert!(h.app.quick_add.is_some());
    let mk = h.app.data.spaces.iter().flat_map(|s| s.lists.iter().chain(s.folders.iter().flat_map(|f| f.lists.iter()))).find(|l| l.name == "Marketing").unwrap().id;
    assert_eq!(h.app.quick_add.as_ref().unwrap().list, mk, "the task goes to that List");
}

#[test]
fn list_menu_info_archive_and_restore_from_settings() {
    let mut h = Ui1::new();
    let mk = h.app.data.spaces.iter().flat_map(|s| s.lists.iter().chain(s.folders.iter().flat_map(|f| f.lists.iter()))).find(|l| l.name == "Marketing").unwrap().id;
    h.click("List options Marketing");
    h.click("List Info");
    assert!(matches!(h.app.modal, Some(Modal::ListInfo { .. })));
    h.click("List description");
    h.type_text("Campaign work");
    assert_eq!(h.app.data.list_item(mk).unwrap().description, "Campaign work");
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
    assert!(!h.has("List options Marketing"), "an archived List leaves the sidebar");
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
    let sid = h.app.data.spaces.iter().find(|s| s.name == "Side Projects").unwrap().id;
    let n = h.app.data.spaces.len();
    // Color & Icon
    h.click("Space options Side Projects");
    h.click("Color & Icon");
    h.click("Space icon \u{2605}");
    assert_eq!(h.app.data.spaces.iter().find(|s| s.id == sid).unwrap().icon, "\u{2605}");
    // Duplicate copies lists and tasks
    h.click("Space options Side Projects");
    h.click("Duplicate");
    h.app.apply_actions();
    assert_eq!(h.app.data.spaces.len(), n + 1);
    let copy = h.app.data.spaces.iter().find(|s| s.name == "Side Projects (copy)").unwrap().clone();
    let orig_lists = h.app.data.lists_in(Location::Space(sid)).unwrap().len();
    let copy_lists = h.app.data.lists_in(Location::Space(copy.id)).unwrap();
    assert_eq!(copy_lists.len(), orig_lists);
    assert!(h.app.data.tasks.iter().any(|t| copy_lists.contains(&t.list_id)), "tasks come along");
    h.settle();
    // Hide Space: off the sidebar, back from Settings
    h.click("Space options Side Projects (copy)");
    h.click("Hide Space");
    assert!(!h.has("Space options Side Projects (copy)"));
    h.app.section = Section::Settings;
    h.settle();
    h.click("Settings Spaces");
    h.click("Show Side Projects (copy)");
    assert!(!h.app.data.spaces.iter().find(|s| s.id == copy.id).unwrap().hidden);
    // Archive, then Restore from Settings
    h.click("\u{2190}  Back to Workspace");
    h.click("Space options Side Projects (copy)");
    h.click("Archive");
    assert!(h.app.data.spaces.iter().find(|s| s.id == copy.id).unwrap().archived);
    h.app.section = Section::Settings;
    h.settle();
    h.click("Settings Spaces");
    h.click("Restore Side Projects (copy)");
    assert!(!h.app.data.spaces.iter().find(|s| s.id == copy.id).unwrap().archived);
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
    assert!(h.app.open_task.is_some(), "Escape leaves the field, not the task");
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none(), "a second Escape closes the task");
}

#[test]
fn property_columns_never_squeeze_the_name_away() {
    let mut hidden = HashSet::new();
    hidden.insert("Date created".to_string());
    let row = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 30.0));
    let cols = super::shell::col_layout(row, &hidden, true);
    assert!(cols.extra.len() < parity::PROPERTIES.len(), "not every column fits in 1100px");
    assert!(cols.left >= 300.0, "the name keeps at least 300px ({})", cols.left);
}
