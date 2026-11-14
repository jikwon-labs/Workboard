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
