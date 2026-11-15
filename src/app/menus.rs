// Menus and dialogs measured from the person's ClickUp: workspace menu,
// profile menu with personal tools, the sidebar Create menu, the rail More
// menu, team invite, people and roles, settings, themes, notifications,
// shortcuts, custom fields, automations, tag manager, templates, CSV import
// and export, channel settings, share, trash, reminders and the notepad.
// Online-only features (AI, Clips, Talk to Text, huddles) open a dialog that
// says they come with the shared server.

use super::*;
use egui::Response;

pub enum Modal {
    Invite { emails: String, role: String },
    WorkspaceSettings { name: String },
    NewWorkspace { name: String },
    Profile { name: String, email: String },
    SetStatus { text: String },
    Themes,
    Shortcuts,
    Help,
    Labs,
    Notifications,
    ClickApps,
    CustomFields { name: String, kind: String, opts: String },
    Automations { status: String, action: String, value: String },
    TagManager { from: String, to: String },
    Templates { name: String, desc: String, checklist: String, priority: u8 },
    Import { path: String, result: String },
    ChannelSettings(u64),
    Share { emails: String },
    CustomizeNav,
    CustomizeSidebar,
    TrackTime { task: u64 },
    Reminder { text: String, date: Option<NaiveDate>, time: String },
    Trash,
    Unavailable { title: String, body: String },
    Bookmark { title: String, url: String },
    SwitchUser,
    NewDashboard { name: String },
    /// Share the workspace through a folder, or join one.
    /// `path`: the shared folder typed in; `addr`: the server address typed in.
    /// `code`: an invite code typed in.
    Together { path: String, addr: String, code: String, result: String },
    WhoAmI { name: String, email: String },
    Call { ch: u64, link: String },
    NewList { name: String, space: u64, folder: Option<u64>, private: bool },
    NewSpace { name: String, color: [u8; 3] },
    MoveTasks { ids: Vec<u64>, query: String },
    MoveList { list: u64 },
    NewChannel { name: String, private: bool, add_list: bool },
    ListInfo { list: u64 },
}

pub const ROLES: [(&str, &str); 4] = [
    ("Admin", "Can manage people, settings and all items."),
    ("Member", "Can access all public items in your Workspace."),
    ("Guest", "Can only access items shared with them."),
    ("Owner", "Owns the Workspace."),
];

/// Personal tools as listed in the original's profile menu.
pub const TOOLS: [(&str, &str, &str); 12] = [
    ("create_task", "Create task", "\u{2611}"),
    ("my_work", "My Work", "\u{1F4BC}"),
    ("track_time", "Track Time", "\u{23F1}"),
    ("notepad", "Notepad", "\u{1F4DD}"),
    ("clip", "Record a Clip", "\u{1F3A5}"),
    ("stt", "Talk to Text", "\u{1F3A4}"),
    ("reminder", "Create Reminder", "\u{23F0}"),
    ("doc", "Create Doc", "\u{1F4C4}"),
    ("whiteboard", "Create Whiteboard", "\u{1F3A8}"),
    ("people", "View People", "\u{1F465}"),
    ("dashboard", "Create Dashboard", "\u{1F4CA}"),
    ("notetaker", "AI Notetaker", "\u{1F4D3}"),
];

pub fn unavailable(what: &str) -> Modal {
    Modal::Unavailable {
        title: what.to_string(),
        body: crate::trf!(
            "{} needs an online service (AI, video or voice processing). It will work once the shared server is set up; everything else in this app already works on this computer.",
            tr(what)
        ),
    }
}

fn menu_item(ui: &mut Ui, t: &Theme, icon: &str, label: &str, right: &str) -> Response {
    let w = ui.available_width().max(200.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 27.0), Sense::click());
    name_it(&resp, label);
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    ui.painter().text(r.left_center() + vec2(14.0, 0.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
    ui.painter().text(r.left_center() + vec2(32.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), t.text);
    if !right.is_empty() {
        ui.painter().text(r.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, right, FontId::proportional(12.0), t.text3);
    }
    resp
}

fn menu_item2(ui: &mut Ui, t: &Theme, icon: &str, label: &str, sub: &str) -> Response {
    let w = ui.available_width().max(200.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 44.0), Sense::click());
    name_it(&resp, label);
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    ui.painter().text(r.left_top() + vec2(14.0, 14.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
    ui.painter().text(r.left_top() + vec2(32.0, 14.0), Align2::LEFT_CENTER, label, FontId::proportional(14.0), t.text);
    ui.painter().text(r.left_top() + vec2(32.0, 32.0), Align2::LEFT_CENTER, sub, FontId::proportional(12.0), t.text2);
    resp
}

/// A role ("Admin", "Member"...) for display; the stored value stays English.
pub(super) fn role_text(role: &str) -> &str {
    match role {
        "Admin" => crate::i18n::tr_ctx("Admin (role)", "Admin"),
        "Member" | "Guest" | "Owner" => tr(role),
        _ => role,
    }
}

fn section_label(ui: &mut Ui, t: &Theme, s: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(s).size(12.5).color(t.text3));
}

impl App {
    // ---------------------------------------------------------------- menus

    pub(super) fn workspace_menu(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.set_width(300.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            ui.painter().rect_filled(r, 8.0, rgb(self.data.extra.workspace_color));
            let letter = self.data.workspace.chars().next().unwrap_or('W').to_uppercase().to_string();
            ui.painter().text(r.center(), Align2::CENTER_CENTER, letter, FontId::proportional(18.0), Color32::WHITE);
            ui.vertical(|ui| {
                ui.label(RichText::new(&self.data.workspace).size(16.0));
                let sub = match &self.data.extra.shared {
                    Some(c) if c.server.is_some() => tr("On your team's server \u{2022} every feature included"),
                    Some(_) => tr("Shared with your team \u{2022} every feature included"),
                    None => tr("On this computer \u{2022} every feature included"),
                };
                ui.label(RichText::new(sub).small().color(t.text2));
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 8.0) / 2.0;
            if ui.add(egui::Button::new(tr("\u{2699}  Settings")).min_size(vec2(w, 28.0)).stroke(Stroke::new(1.0, t.border))).clicked() {
                self.open_settings("General");
                close_popup(ui);
            }
            if ui.add(egui::Button::new(tr("\u{1F465}  People")).min_size(vec2(w, 28.0)).stroke(Stroke::new(1.0, t.border))).clicked() {
                self.section = Section::People;
                close_popup(ui);
            }
        });
        ui.separator();
        section_label(ui, &t, tr("Manage"));
        let items: [(&str, &str); 5] = [("\u{25A6}", "Apps"), ("\u{25A4}", "Templates"), ("\u{1F4DD}", "Custom Fields"), ("\u{2699}", "Automations"), ("\u{1F3F7}", "Tag Manager")];
        for (icon, label) in items {
            if menu_item(ui, &t, icon, tr(label), "").clicked() {
                self.modal = Some(match label {
                    "Apps" => Modal::ClickApps,
                    "Templates" => Modal::Templates { name: String::new(), desc: String::new(), checklist: String::new(), priority: 0 },
                    "Custom Fields" => Modal::CustomFields { name: String::new(), kind: "text".into(), opts: String::new() },
                    "Automations" => Modal::Automations { status: "complete".into(), action: "comment".into(), value: String::new() },
                    _ => Modal::TagManager { from: String::new(), to: String::new() },
                });
                close_popup(ui);
            }
        }
        let label = if self.data.extra.shared.is_some() { tr("Work together (shared)") } else { tr("Work together") };
        if menu_item(ui, &t, "\u{1F465}", label, "").clicked() {
            self.modal = Some(self.together_modal());
            close_popup(ui);
        }
        let others: Vec<WsEntry> = Data::workspaces().into_iter().filter(|w| w.file != self.ws_file).collect();
        if !others.is_empty() || self.ws_file != "data.json" {
            ui.separator();
            section_label(ui, &t, tr("Switch Workspace"));
            let mut all = others;
            if self.ws_file != "data.json" && !all.iter().any(|w| w.file == "data.json") {
                all.insert(0, WsEntry { name: "My Workspace".into(), file: "data.json".into() });
            }
            for w in all {
                // workspaces often share a name ("My Workspace"): say where each one lives
                let place = self.workspace_place(&w.file);
                if menu_item(ui, &t, "\u{1F3E2}", &w.name, tr(place)).clicked() {
                    self.switch_workspace(&w.file);
                    close_popup(ui);
                }
            }
        }
        ui.add_space(4.0);
        if ui.add(egui::Button::new(tr("+  Create Workspace")).min_size(vec2(ui.available_width(), 30.0)).stroke(Stroke::new(1.0, t.border))).clicked() {
            self.modal = Some(Modal::NewWorkspace { name: String::new() });
            close_popup(ui);
        }
    }

    /// Where a workspace file lives, for the switch menu.
    pub(super) fn workspace_place(&mut self, file: &str) -> &'static str {
        fn label(cfg: Option<&crate::sync::SharedCfg>) -> &'static str {
            match cfg {
                Some(c) if c.server.is_some() => "Team server",
                Some(_) => "Shared folder",
                None => "This computer",
            }
        }
        if file == self.ws_file {
            return label(self.data.extra.shared.as_ref());
        }
        if let Some(p) = self.ws_places.get(file) {
            return p;
        }
        let path = if file == "data.json" { Data::path() } else { Data::dir().join(file) };
        let cfg: Option<crate::sync::SharedCfg> = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| serde_json::from_value(v.get("extra")?.get("shared")?.clone()).ok());
        let place = label(cfg.as_ref());
        self.ws_places.insert(file.to_string(), place);
        place
    }

    pub(super) fn switch_workspace(&mut self, file: &str) {
        self.ws_places.clear();
        if self.persist {
            self.data.save_file(&self.ws_file);
        }
        let d = if file == "data.json" { Some(Data::load()) } else { Data::load_file(file) };
        if let Some(d) = d {
            self.data = d;
            self.ws_file = file.to_string();
            if self.persist {
                Data::set_last_workspace(file);
            }
            self.loc = self.data.first_list().map(Location::List).unwrap_or(Location::Home);
            self.section = Section::Home;
            self.open_task = None;
            self.chat_open = None;
            self.recent.clear();
            self.sync_rt = Default::default();
            self.leave_huddle();
        }
    }

    pub(super) fn avatar_menu(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.set_width(270.0);
        let me = self.data.me;
        if let Some(m) = self.data.member(me).cloned() {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(38.0, 38.0), Sense::hover());
                paint_avatar(ui.painter(), r.center(), 16.0, &m, t.card);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&m.name).size(15.0));
                    let muted = self.data.extra.muted_until > now_ts();
                    let st = if !m.status_text.is_empty() { m.status_text.clone() } else if muted { tr("Notifications muted").into() } else { tr("Online").into() };
                    ui.label(RichText::new(st).small().color(t.text2));
                });
            });
        }
        if ui.add(egui::Button::new(RichText::new(tr("\u{263A}  Set status")).color(t.text2)).min_size(vec2(ui.available_width(), 28.0)).stroke(Stroke::new(1.0, t.border))).clicked() {
            let cur = self.data.member(me).map(|m| m.status_text.clone()).unwrap_or_default();
            self.modal = Some(Modal::SetStatus { text: cur });
            close_popup(ui);
        }
        // a flyout beside the row (egui's sub menu drew over this menu): to the
        // right when it fits, otherwise to the left of the whole menu
        let mr = menu_item(ui, &t, "\u{1F515}", tr("Mute notifications"), "\u{203A}");
        let fly = ui.id().with("mute_fly");
        let keep: bool = ui.data(|d| d.get_temp(fly)).unwrap_or(false);
        let fly_w: f32 = ui.data(|d| d.get_temp(fly.with("w"))).unwrap_or(170.0);
        let menu = ui.max_rect().expand(egui::Frame::popup(ui.style()).total_margin().left);
        let on_right = menu.right() + 6.0 + fly_w <= ui.ctx().screen_rect().right();
        let fly_x = if on_right { menu.right() + 6.0 } else { menu.left() - 6.0 - fly_w };
        let mut over = false;
        if mr.hovered() || keep {
            let area = egui::Area::new(fly).order(egui::Order::Tooltip).fixed_pos(pos2(fly_x, mr.rect.top() - 4.0)).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(150.0);
                    for (label, secs) in [("For 1 hour", 3600), ("For 4 hours", 4 * 3600), ("Until tomorrow", 24 * 3600), ("Unmute", 0)] {
                        let shown = if label == "Unmute" { crate::i18n::tr_ctx("Unmute (notifications)", label) } else { tr(label) };
                        if ui.add(egui::Button::new(shown).frame(false).min_size(vec2(140.0, 26.0))).clicked() {
                            self.data.extra.muted_until = if secs == 0 { 0 } else { now_ts() + secs };
                            self.dirty = true;
                            self.toast = Some((if secs == 0 { tr("Notifications on").into() } else { crate::trf!("Notifications muted {}", tr(label).to_lowercase()) }, now_ts()));
                            close_popup(ui);
                        }
                    }
                });
            });
            let r = area.response.rect;
            ui.data_mut(|d| d.insert_temp(fly.with("w"), r.width()));
            over = r.expand(4.0).contains(ui.ctx().pointer_hover_pos().unwrap_or_default());
        }
        // the strip the pointer crosses between the row and the flyout
        let (gx0, gx1) = if on_right { (mr.rect.right() - 10.0, fly_x + 12.0) } else { (fly_x + fly_w - 12.0, mr.rect.left() + 10.0) };
        let gap = ui.ctx().pointer_hover_pos().map_or(false, |p| p.y >= mr.rect.top() - 4.0 && p.y <= mr.rect.bottom() + 120.0 && p.x >= gx0 && p.x <= gx1);
        ui.data_mut(|d| d.insert_temp(fly, mr.hovered() || over || (keep && gap)));
        ui.separator();
        let rows: [(&str, &str); 8] = [("\u{2699}", "Settings"), ("\u{1F514}", "Notifications"), ("\u{1F3A8}", "Themes"), ("\u{2728}", "Labs"), ("\u{2318}", "Keyboard shortcuts"), ("\u{2B07}", "Download app"), ("\u{21BB}", "Check for updates"), ("\u{2753}", "Help")];
        for (icon, label) in rows {
            if menu_item(ui, &t, icon, tr(label), "").clicked() {
                if label == "Settings" {
                    self.open_settings("My Settings");
                    close_popup(ui);
                    continue;
                }
                if label == "Check for updates" {
                    // Sparkle shows its own window; only explain when it can't run
                    if let Err(why) = crate::updater::check_now() {
                        self.modal = Some(Modal::Unavailable { title: "Check for updates".into(), body: why.explain() });
                    }
                    close_popup(ui);
                    continue;
                }
                self.modal = Some(match label {
                    "Settings" => {
                        let m = self.data.member(me).cloned().unwrap_or_default();
                        Modal::Profile { name: m.name, email: m.email }
                    }
                    "Notifications" => Modal::Notifications,
                    "Themes" => Modal::Themes,
                    "Labs" => Modal::Labs,
                    "Keyboard shortcuts" => Modal::Shortcuts,
                    "Download app" => Modal::Unavailable { title: "Download".into(), body: tr("You're already using the desktop app on this computer.").into() },
                    _ => Modal::Help,
                });
                close_popup(ui);
            }
        }
        ui.separator();
        section_label(ui, &t, tr("Personal Tools"));
        for (id, label, icon) in TOOLS {
            ui.horizontal(|ui| {
                let pinned = self.data.extra.pinned_tools.iter().any(|p| p == id);
                let w = ui.available_width() - 30.0;
                let (r, resp) = ui.allocate_exact_size(vec2(w, 27.0), Sense::click());
                name_it(&resp, label);
                if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                ui.painter().text(r.left_center() + vec2(14.0, 0.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
                ui.painter().text(r.left_center() + vec2(32.0, 0.0), Align2::LEFT_CENTER, tr(label), FontId::proportional(14.0), t.text);
                if resp.clicked() {
                    self.run_tool(id);
                    close_popup(ui);
                }
                let pin = egui::Button::new(RichText::new("\u{1F4CC}").size(11.0).color(if pinned { t.text } else { t.border })).frame(false);
                if ui.add(pin).on_hover_text(if pinned { tr("Unpin from the top bar") } else { tr("Pin to the top bar") }).clicked() {
                    if pinned {
                        self.data.extra.pinned_tools.retain(|p| p != id);
                    } else {
                        self.data.extra.pinned_tools.push(id.to_string());
                    }
                    self.dirty = true;
                }
            });
        }
        ui.separator();
        if menu_item(ui, &t, "\u{1F5D1}", tr("Trash"), "").clicked() {
            self.modal = Some(Modal::Trash);
            close_popup(ui);
        }
        if menu_item(ui, &t, "\u{21AA}", tr("Log out"), "").on_hover_text(tr("Switch to another person on this computer")).clicked() {
            self.modal = Some(Modal::SwitchUser);
            close_popup(ui);
        }
    }

    pub(super) fn run_tool(&mut self, id: &str) {
        match id {
            "create_task" => self.open_quick_add(None),
            "my_work" => self.actions.push(Action::Go(Location::Home)),
            "track_time" => {
                let first = self.data.tasks.iter().find(|x| x.assignees.contains(&self.data.me)).or(self.data.tasks.first()).map(|x| x.id).unwrap_or(0);
                self.modal = Some(Modal::TrackTime { task: first });
            }
            "notepad" => self.notepad_open = !self.notepad_open,
            "clip" => self.modal = Some(unavailable("Record a Clip")),
            "stt" => self.modal = Some(unavailable("Talk to Text")),
            "reminder" => self.modal = Some(Modal::Reminder { text: String::new(), date: Some(today()), time: "09:00".into() }),
            "doc" => self.new_doc(None),
            "whiteboard" => self.new_whiteboard(),
            "people" => self.section = Section::People,
            "dashboard" => self.modal = Some(Modal::NewDashboard { name: "New Dashboard".into() }),
            _ => self.modal = Some(unavailable("AI Notetaker")),
        }
    }

    pub(super) fn new_doc(&mut self, space: Option<u64>) {
        let id = self.data.new_id();
        let now = now_ts();
        self.data.docs.insert(0, Doc { id, space, owner: self.data.me, private: false, archived: false, meeting_note: false, parent: None, title: "Untitled".into(), body: String::new(), created: now, updated: now });
        self.doc_open = Some(id);
        self.section = Section::Docs;
        self.dirty = true;
    }

    pub(super) fn new_whiteboard(&mut self) {
        let id = self.data.new_id();
        let n = self.data.extra.whiteboards.len() + 1;
        self.data.extra.whiteboards.push(Whiteboard { id, name: format!("Whiteboard {}", n), strokes: vec![], notes: vec![] });
        self.wb_open = Some(id);
        self.section = Section::Whiteboards;
        self.dirty = true;
    }

    pub(super) fn new_form(&mut self) {
        let id = self.data.new_id();
        let list = self.default_list().unwrap_or(0);
        self.data.extra.forms.push(Form {
            id,
            name: "New Form".into(),
            list_id: list,
            description: "Fill out this form to create a task.".into(),
            fields: vec![FormField { label: "Title".into(), map: "name".into() }, FormField { label: "Details".into(), map: "description".into() }],
        });
        self.form_open = Some(id);
        self.section = Section::Forms;
        self.dirty = true;
    }

    /// The sidebar "+ Create" menu, in the original's order.
    pub(super) fn create_menu(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.set_width(400.0);
        let r = ui.add(egui::TextEdit::singleline(&mut self.create_text).hint_text(tr("Describe anything to create")).desired_width(f32::INFINITY));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.create_text.trim().is_empty() {
            if let Some(list) = self.default_list() {
                self.actions.push(Action::Add { list, name: self.create_text.clone(), status: None, priority: 0, assignees: vec![self.data.me], due: None, parent: None, open: true });
            }
            self.create_text.clear();
            close_popup(ui);
        }
        ui.separator();
        section_label(ui, &t, tr("Create"));
        if menu_item(ui, &t, "\u{2611}", tr("Task"), "\u{2325}T").clicked() {
            self.open_quick_add(None);
            close_popup(ui);
        }
        if menu_item(ui, &t, "\u{27A4}", tr("Message"), "\u{2318}G").clicked() {
            self.new_message_open = true;
            close_popup(ui);
        }
        ui.separator();
        let space = self.current_space().or_else(|| self.data.spaces.first().map(|s| s.id));
        if menu_item2(ui, &t, "\u{2630}", tr("List"), tr("Track tasks, projects, people & more")).clicked() {
            self.open_new_list(space, None);
            close_popup(ui);
        }
        if menu_item2(ui, &t, "#", tr("Channel"), tr("Conversations on specific topics")).clicked() {
            self.modal = Some(Modal::NewChannel { name: String::new(), private: false, add_list: false });
            close_popup(ui);
        }
        if menu_item2(ui, &t, "\u{25CE}", tr("Space"), tr("Organize work by team or department")).clicked() {
            let color = STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13];
            self.modal = Some(Modal::NewSpace { name: String::new(), color });
            close_popup(ui);
        }
        ui.separator();
        if menu_item(ui, &t, "\u{2728}", tr("Create with AI"), "").clicked() {
            self.modal = Some(unavailable("Create with AI"));
            close_popup(ui);
        }
        if menu_item(ui, &t, "\u{2699}", tr("Super Agent"), tr("Hot")).clicked() {
            self.modal = Some(unavailable("Super Agent"));
            close_popup(ui);
        }
        ui.separator();
        for (icon, label) in [("\u{1F4C4}", "Doc"), ("\u{1F496}", "Artifact"), ("\u{1F4CB}", "Form"), ("\u{1F4CA}", "Dashboard"), ("\u{1F3A8}", "Whiteboard"), ("\u{1F3A5}", "Clip")] {
            if menu_item(ui, &t, icon, tr(label), "").clicked() {
                match label {
                    "Doc" => self.new_doc(space),
                    "Form" => self.new_form(),
                    "Dashboard" => self.modal = Some(Modal::NewDashboard { name: "New Dashboard".into() }),
                    "Whiteboard" => self.new_whiteboard(),
                    "Artifact" => self.modal = Some(unavailable("Artifacts")),
                    _ => self.modal = Some(unavailable("Clips")),
                }
                close_popup(ui);
            }
        }
        ui.separator();
        if menu_item(ui, &t, "\u{25A6}", tr("Customize your sidebar"), "").clicked() {
            self.modal = Some(Modal::CustomizeSidebar);
            close_popup(ui);
        }
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 8.0) / 2.0;
            if ui.add(egui::Button::new(tr("\u{2B07}  Import")).min_size(vec2(w, 30.0)).stroke(Stroke::new(1.0, t.border))).clicked() {
                self.modal = Some(Modal::Import { path: String::new(), result: String::new() });
                close_popup(ui);
            }
            if ui.add(egui::Button::new(tr("\u{2728}  Templates")).min_size(vec2(w, 30.0)).stroke(Stroke::new(1.0, t.border))).clicked() {
                self.modal = Some(Modal::Templates { name: String::new(), desc: String::new(), checklist: String::new(), priority: 0 });
                close_popup(ui);
            }
        });
    }

    pub(super) fn current_space(&self) -> Option<u64> {
        match self.loc {
            Location::Space(id) => Some(id),
            Location::List(id) => self.data.space_of_list(id).map(|s| s.id),
            Location::Folder(fid) => self.data.spaces.iter().find(|s| s.folders.iter().any(|f| f.id == fid)).map(|s| s.id),
            _ => None,
        }
    }

    /// The rail's More menu: Spaces, Chat, Docs, Dashboards, Whiteboards,
    /// Forms, Clips, Goals, Timesheets, Apps, Customize navigation.
    pub(super) fn rail_more_menu(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.set_width(220.0);
        let items: [(&str, &str); 10] = [
            ("\u{25CE}", "Spaces"),
            ("\u{1F4AC}", "Chat"),
            ("\u{1F4C4}", "Docs"),
            ("\u{1F4CA}", "Dashboards"),
            ("\u{1F3A8}", "Whiteboards"),
            ("\u{1F4CB}", "Forms"),
            ("\u{1F3A5}", "Clips"),
            ("\u{1F3AF}", "Goals"),
            ("\u{23F1}", "Timesheets"),
            ("\u{25A6}", "Apps"),
        ];
        for (icon, label) in items {
            let pinned = self.data.extra.rail_items.iter().any(|r| r == label);
            ui.horizontal(|ui| {
                let w = ui.available_width() - 28.0;
                let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                name_it(&resp, label);
                if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                ui.painter().text(r.left_center() + vec2(14.0, 0.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
                ui.painter().text(r.left_center() + vec2(32.0, 0.0), Align2::LEFT_CENTER, tr(label), FontId::proportional(14.0), t.text);
                if resp.clicked() {
                    self.open_app(label);
                    close_popup(ui);
                }
                if ui.add(egui::Button::new(RichText::new("\u{1F4CC}").size(11.0).color(if pinned { t.text } else { t.border })).frame(false)).on_hover_text(tr("Pin to the left bar")).clicked() {
                    if pinned {
                        self.data.extra.rail_items.retain(|r| r != label);
                    } else {
                        self.data.extra.rail_items.push(label.to_string());
                    }
                    self.dirty = true;
                }
            });
        }
        ui.separator();
        if menu_item(ui, &t, "\u{2699}", tr("Customize navigation"), "").clicked() {
            self.modal = Some(Modal::CustomizeNav);
            close_popup(ui);
        }
    }

    pub(super) fn open_app(&mut self, label: &str) {
        self.open_task = None;
        self.hub_filter = 0;
        let temp = match label {
            "Docs" => Some(Section::Docs),
            "Dashboards" => Some(Section::Dashboards),
            "Whiteboards" => Some(Section::Whiteboards),
            "Forms" => Some(Section::Forms),
            "Goals" => Some(Section::Goals),
            "Timesheets" => Some(Section::Timesheets),
            _ => None,
        };
        if temp.is_some() && !self.data.extra.rail_items.iter().any(|r| r == label) {
            self.temp_app = temp;
        }
        match label {
            "Spaces" => self.actions.push(Action::Go(Location::Everything)),
            "Chat" => {
                self.section = Section::Chat;
            }
            "Docs" => self.section = Section::Docs,
            "Dashboards" => self.section = Section::Dashboards,
            "Whiteboards" => self.section = Section::Whiteboards,
            "Forms" => self.section = Section::Forms,
            "Goals" => self.section = Section::Goals,
            "Timesheets" => self.section = Section::Timesheets,
            "Planner" => self.section = Section::Planner,
            "Teams" => self.section = Section::Teams,
            "Apps" => self.modal = Some(Modal::ClickApps),
            "Clips" => self.modal = Some(unavailable("Clips")),
            "AI" => self.section = Section::Ai,
            _ => {}
        }
    }

    // ---------------------------------------------------------------- dialogs

    pub(super) fn modals(&mut self, ctx: &egui::Context) {
        let Some(mut modal) = self.modal.take() else { return };
        let t = self.t();
        let mut open = true;
        let mut close = false;
        let title = match &modal {
            Modal::Invite { .. } => tr("Invite people for free").to_string(),
            Modal::WorkspaceSettings { .. } => tr("Workspace settings").into(),
            Modal::NewWorkspace { .. } => tr("Create Workspace").into(),
            Modal::Profile { .. } => tr("My Settings").into(),
            Modal::SetStatus { .. } => tr("Set status").into(),
            Modal::Themes => tr("Themes").into(),
            Modal::Shortcuts => tr("Keyboard shortcuts").into(),
            Modal::Help => tr("Help").into(),
            Modal::Labs => tr("Labs").into(),
            Modal::Notifications => tr("Notifications").into(),
            Modal::ClickApps => tr("Apps").into(),
            Modal::CustomFields { .. } => tr("Custom Fields").into(),
            Modal::Automations { .. } => tr("Automations").into(),
            Modal::TagManager { .. } => tr("Tag Manager").into(),
            Modal::Templates { .. } => tr("Templates").into(),
            Modal::Import { .. } => tr("Import and export").into(),
            Modal::ChannelSettings(_) => tr("Channel Settings").into(),
            Modal::Share { .. } => tr("Share").into(),
            Modal::CustomizeNav => tr("Customize navigation").into(),
            Modal::CustomizeSidebar => tr("Customize your sidebar").into(),
            Modal::TrackTime { .. } => tr("Track Time").into(),
            Modal::Reminder { .. } => tr("Create Reminder").into(),
            Modal::Trash => tr("Trash").into(),
            Modal::Unavailable { title, .. } => tr(title).to_string(),
            Modal::Bookmark { .. } => tr("Add Bookmark").into(),
            Modal::SwitchUser => tr("Switch person").into(),
            Modal::NewDashboard { .. } => tr("Create Dashboard").into(),
            Modal::Together { .. } => tr("Work together").into(),
            Modal::WhoAmI { .. } => tr("Who are you?").into(),
            Modal::Call { .. } => tr("Start a call").into(),
            Modal::NewList { .. } => tr("Create List").into(),
            Modal::NewSpace { .. } => tr("Create a Space").into(),
            Modal::MoveTasks { .. } => tr("Move to").into(),
            Modal::MoveList { .. } => tr("Move List").into(),
            Modal::NewChannel { .. } => tr("Create Channel").into(),
            Modal::ListInfo { .. } => tr("List Info").into(),
        };
        // dim backdrop
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("modal_dim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(screen, 0.0, Color32::from_black_alpha(90));
            if r.clicked() {
                close = true;
            }
        });
        let win_id = egui::Id::new("modal_window");
        egui::Window::new(RichText::new(&title).size(17.0))
            .id(win_id)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, -20.0])
            .min_width(460.0)
            .max_width(620.0)
            .frame(dialog_frame(&t))
            .show(ctx, |ui| {
                ui.set_min_width(440.0);
                if dialog_header(ui, &t, &title) {
                    open = false;
                }
                ui.add_space(10.0);
                close |= self.modal_body(ui, &t, &mut modal);
            });
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, win_id));
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.menu_was_open {
            close = true;
        }
        if open && !close && self.modal.is_none() {
            self.modal = Some(modal);
        }
    }

    /// Returns true when the dialog should close.
    pub(super) fn modal_body(&mut self, ui: &mut Ui, t: &Theme, modal: &mut Modal) -> bool {
        let mut close = false;
        match modal {
            Modal::Together { path, addr, code, result } => close = self.together_body(ui, t, path, addr, code, result),
            Modal::WhoAmI { name, email } => close = self.who_am_i_body(ui, t, name, email),
            Modal::Call { ch, link } => close = self.call_body(ui, t, ch, link),
            Modal::NewList { name, space, folder, .. } => close = self.new_list_body(ui, t, name, space, folder),
            Modal::NewSpace { name, color } => close = self.new_space_body(ui, t, name, color),
            Modal::NewChannel { name, private, add_list } => close = self.new_channel_body(ui, t, name, private, add_list),
            Modal::ListInfo { list } => {
                let l = *list;
                close = self.list_info_body(ui, t, l);
            }
            Modal::MoveTasks { ids, query } => {
                let ids = ids.clone();
                close = self.move_tasks_body(ui, t, &ids, query);
            }
            Modal::MoveList { list } => {
                let l = *list;
                close = self.move_list_body(ui, t, l);
            }
            Modal::Invite { emails, role } => {
                ui.label(RichText::new(tr("Email")).color(t.text2));
                let r = ui.add(egui::TextEdit::singleline(emails).hint_text(tr("Enter email, comma or space separated")).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(10, 8)));
                r.request_focus();
                ui.add_space(8.0);
                ui.label(RichText::new(tr("Invite as")).color(t.text2));
                let desc = ROLES.iter().find(|r| r.0 == role.as_str()).map(|r| r.1).unwrap_or("");
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                    ui.painter().rect_filled(r, 8.0, t.hover);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, "\u{1F464}", FontId::proportional(14.0), t.text2);
                    ui.vertical(|ui| {
                        egui::ComboBox::from_id_salt("invite_role").selected_text(role_text(role)).show_ui(ui, |ui| {
                            for (name, _) in ROLES.iter().take(3) {
                                ui.selectable_value(role, name.to_string(), role_text(name));
                            }
                        });
                        ui.label(RichText::new(tr(desc)).small().color(t.text2));
                    });
                });
                ui.add_space(8.0);
                let shared = self.data.extra.shared.is_some();
                let on_server = self.data.extra.shared.as_ref().is_some_and(|c| c.server.is_some());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if on_server { tr("This workspace is on a server: after inviting, give each person their invite code (Work together > People).") } else if shared { tr("This workspace is shared, so invited people can join it from the shared folder.") } else { tr("To work with them, share this workspace in a folder first.") }).small().color(t.text2));
                    if ui.link(if shared { tr("Sharing") } else { tr("Work together") }).clicked() {
                        self.modal = Some(self.together_modal());
                    }
                });
                ui.add_space(4.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let valid: Vec<String> = emails.split(|c: char| c == ',' || c.is_whitespace()).map(|s| s.trim().to_string()).filter(|s| s.contains('@') && s.contains('.')).collect();
                        let send = ui.add_enabled(!valid.is_empty(), egui::Button::new(RichText::new(tr("Send free invite")).color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(0.0, 32.0)));
                        if send.clicked() {
                            self.invite_people(&valid, role);
                            let on_server = self.data.extra.shared.as_ref().is_some_and(|c| c.server.is_some());
                            self.toast = Some((if on_server { crate::trf!("Invited {} people. Get their invite codes in Work together > People.", valid.len()) } else if self.data.extra.shared.is_some() { crate::trf!("Invited {} people. When they join the shared folder they pick their name.", valid.len()) } else { crate::trf!("Invited {} people. Share the workspace (Work together) so they can join.", valid.len()) }, now_ts()));
                            self.dirty = true;
                            close = true;
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Modal::WorkspaceSettings { name } => {
                ui.label(tr("Workspace name"));
                ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                ui.add_space(6.0);
                ui.label(tr("Workspace color"));
                ui.horizontal_wrapped(|ui| {
                    for c in STATUS_PALETTE.iter().skip(1) {
                        let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        ui.painter().rect_filled(r.shrink(2.0), 5.0, rgb(*c));
                        if self.data.extra.workspace_color == *c {
                            ui.painter().rect_stroke(r, 6.0, Stroke::new(2.0, t.text), egui::StrokeKind::Inside);
                        }
                        if resp.clicked() {
                            self.data.extra.workspace_color = *c;
                            self.dirty = true;
                        }
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new(crate::trf!("{} people \u{2022} {} spaces \u{2022} {} tasks", self.data.members.len(), self.data.spaces.len(), self.data.tasks.len())).color(t.text2));
                ui.add_space(8.0);
                if primary_button(ui, t, tr("Save")).clicked() && !name.trim().is_empty() {
                    self.data.workspace = name.trim().to_string();
                    let mut list = Data::workspaces();
                    if let Some(w) = list.iter_mut().find(|w| w.file == self.ws_file) {
                        w.name = self.data.workspace.clone();
                        Data::save_workspaces(&list);
                    }
                    self.dirty = true;
                    close = true;
                }
            }
            Modal::NewWorkspace { name } => {
                ui.label(tr("Name your Workspace"));
                let r = ui.add(egui::TextEdit::singleline(name).hint_text(tr("e.g. Acme Inc.")).desired_width(f32::INFINITY));
                r.request_focus();
                ui.label(RichText::new(tr("You'll be its Owner. Switch between workspaces from the workspace menu.")).small().color(t.text2));
                ui.add_space(8.0);
                if primary_button(ui, t, tr("Create Workspace")).clicked() && !name.trim().is_empty() {
                    let mut list = Data::workspaces();
                    if list.is_empty() {
                        list.push(WsEntry { name: self.data.workspace.clone(), file: self.ws_file.clone() });
                    }
                    let file = format!("workspace-{}.json", now_ts());
                    let mut owner = self.data.member(self.data.me).cloned().unwrap_or_default();
                    owner.id = 1;
                    owner.role = "Owner".into();
                    let d = Data::blank(name.trim(), owner);
                    d.save_file(&file);
                    list.push(WsEntry { name: name.trim().to_string(), file: file.clone() });
                    Data::save_workspaces(&list);
                    self.switch_workspace(&file);
                    close = true;
                }
            }
            Modal::Profile { name, email } => {
                ui.label(tr("Full name"));
                ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                ui.label(tr("Email"));
                ui.add(egui::TextEdit::singleline(email).desired_width(f32::INFINITY));
                ui.label(tr("Avatar color"));
                let me = self.data.me;
                ui.horizontal_wrapped(|ui| {
                    for c in STATUS_PALETTE.iter().skip(1) {
                        let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        ui.painter().circle_filled(r.center(), 9.0, rgb(*c));
                        if resp.clicked() {
                            if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me) {
                                m.color = *c;
                                self.dirty = true;
                            }
                        }
                    }
                });
                ui.add_space(8.0);
                if primary_button(ui, t, tr("Save changes")).clicked() && !name.trim().is_empty() {
                    if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me) {
                        m.name = name.trim().to_string();
                        m.email = email.trim().to_string();
                    }
                    self.dirty = true;
                    close = true;
                }
            }
            Modal::SetStatus { text } => {
                ui.add(egui::TextEdit::singleline(text).hint_text(tr("What's your status?")).desired_width(f32::INFINITY));
                ui.horizontal_wrapped(|ui| {
                    for s in [tr("\u{1F4C5} In a meeting"), tr("\u{1F68C} Commuting"), tr("\u{2639} Out sick"), tr("\u{1F334} Vacationing"), tr("\u{1F3E0} Working remotely")] {
                        if ui.button(s).clicked() {
                            *text = s.to_string();
                        }
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button(ui, t, tr("Save")).clicked() {
                        let me = self.data.me;
                        if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me) {
                            m.status_text = text.trim().to_string();
                        }
                        self.dirty = true;
                        close = true;
                    }
                    if ui.button(tr("Clear status")).clicked() {
                        let me = self.data.me;
                        if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me) {
                            m.status_text.clear();
                        }
                        self.dirty = true;
                        close = true;
                    }
                });
            }
            Modal::Themes => {
                ui.label(tr("Appearance"));
                ui.horizontal(|ui| {
                    let auto = self.data.extra.auto_theme;
                    if ui.selectable_label(!auto && !self.data.dark, tr("\u{2600} Light")).clicked() {
                        self.data.dark = false;
                        self.data.extra.auto_theme = false;
                        self.dirty = true;
                    }
                    if ui.selectable_label(!auto && self.data.dark, tr("\u{1F319} Dark")).clicked() {
                        self.data.dark = true;
                        self.data.extra.auto_theme = false;
                        self.dirty = true;
                    }
                    if ui.selectable_label(auto, tr("\u{25D0} Auto")).on_hover_text(tr("Follow your computer's appearance")).clicked() {
                        self.data.extra.auto_theme = true;
                        self.dirty = true;
                    }
                });
                ui.add_space(6.0);
                ui.label(tr("Theme color"));
                ui.horizontal_wrapped(|ui| {
                    for (name, c) in theme::ACCENTS {
                        let sel = self.data.extra.accent == name;
                        let (r, resp) = ui.allocate_exact_size(vec2(64.0, 52.0), Sense::click());
                        ui.painter().circle_filled(r.center_top() + vec2(0.0, 16.0), 13.0, rgb(c));
                        if sel {
                            ui.painter().circle_stroke(r.center_top() + vec2(0.0, 16.0), 16.0, Stroke::new(2.0, t.text));
                        }
                        ui.painter().text(r.center_bottom() - vec2(0.0, 8.0), Align2::CENTER_CENTER, tr(name), FontId::proportional(11.5), t.text2);
                        if resp.clicked() {
                            self.data.extra.accent = name.to_string();
                            self.dirty = true;
                        }
                    }
                });
            }
            Modal::Shortcuts => {
                egui::Grid::new("shortcuts").num_columns(2).spacing(vec2(30.0, 8.0)).show(ui, |ui| {
                    for (k, d) in [
                        ("\u{2318}J  /  \u{2318}K", "Search"),
                        ("\u{2318}N  /  \u{2325}T", "Create task"),
                        ("\u{2318}G", "New message"),
                        ("T", "Create task"),
                        ("R", "Create reminder"),
                        ("N  /  P", "Open or close the notepad"),
                        ("\u{2318}\\", "Show or hide the sidebar"),
                        ("\u{2318}1 \u{2013} \u{2318}4  /  \u{2318}0", "Set or clear priority (open task)"),
                        ("\u{2318}/\u{21E7} click", "Select several tasks"),
                        ("/", "Commands in a Doc"),
                        ("\u{2318}[  /  \u{2318}]", "Back / forward"),
                        ("Enter", "Send a message or save a new task"),
                        ("Shift+Enter", "New line in a message"),
                        ("Esc", "Close a window or menu"),
                        ("Right-click", "Rename, delete, favorite, more"),
                    ] {
                        ui.label(RichText::new(tr(k)).monospace());
                        ui.label(tr(d));
                        ui.end_row();
                    }
                });
            }
            Modal::Help => {
                ui.label(tr("This is your own copy of the work manager. Everything is saved on this computer."));
                ui.add_space(6.0);
                ui.label(RichText::new(crate::trf!("Saved in: {}", Data::dir().display())).small().color(t.text2));
                ui.label(RichText::new(tr("Tip: right-click spaces, lists and tasks for more options.")).small().color(t.text2));
            }
            Modal::Labs => {
                ui.label(tr("Try features early. Turn them on or off at any time."));
                ui.add_space(6.0);
                for (id, label) in [("compact", "Compact list rows"), ("week_monday", "Start the week on Monday in the Planner"), ("confetti", "Celebrate when a task is completed")] {
                    let mut on = self.data.extra.labs.iter().any(|l| l == id);
                    if ui.checkbox(&mut on, tr(label)).changed() {
                        if on {
                            self.data.extra.labs.push(id.to_string());
                        } else {
                            self.data.extra.labs.retain(|l| l != id);
                        }
                        self.dirty = true;
                    }
                }
            }
            Modal::Notifications => {
                ui.label(tr("Show in your Inbox when..."));
                let n = &mut self.data.extra.notify;
                let mut ch = false;
                ch |= ui.checkbox(&mut n.comments, tr("Someone comments on my tasks")).changed();
                ch |= ui.checkbox(&mut n.mentions, tr("Someone @mentions me")).changed();
                ch |= ui.checkbox(&mut n.assigned, tr("A task is assigned to me")).changed();
                ch |= ui.checkbox(&mut n.reminders, tr("A reminder is due")).changed();
                ch |= ui.checkbox(&mut n.chat, tr("New chat messages (unread counts)")).changed();
                if ch {
                    self.dirty = true;
                }
            }
            Modal::ClickApps => {
                ui.label(tr("Turn features on or off for this Workspace."));
                ui.add_space(6.0);
                let a = &mut self.data.extra.apps;
                let mut ch = false;
                ch |= ui.checkbox(&mut a.priorities, tr("Priorities")).changed();
                ch |= ui.checkbox(&mut a.tags, tr("Tags")).changed();
                ch |= ui.checkbox(&mut a.time_tracking, tr("Time Tracking")).changed();
                ch |= ui.checkbox(&mut a.time_estimates, tr("Time Estimates")).changed();
                ch |= ui.checkbox(&mut a.custom_fields, tr("Custom Fields")).changed();
                ch |= ui.checkbox(&mut a.multiple_assignees, tr("Multiple Assignees")).changed();
                if ch {
                    self.dirty = true;
                }
            }
            Modal::CustomFields { name, kind, opts } => {
                let mut remove = None;
                for (i, f) in self.data.extra.fields.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&f.name).strong());
                        let mut k = f.kind.clone();
                        if let Some(c) = k.get_mut(0..1) {
                            c.make_ascii_uppercase();
                        }
                        ui.label(RichText::new(tr(&k)).color(t.text2));
                        if !f.options.is_empty() {
                            ui.label(RichText::new(f.options.join(", ")).small().color(t.text3));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new(tr("Delete")).color(t.danger)).frame(false)).clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                }
                if let Some(i) = remove {
                    let id = self.data.extra.fields[i].id;
                    self.data.extra.fields.remove(i);
                    for task in &mut self.data.tasks {
                        task.custom.remove(&id);
                    }
                    self.dirty = true;
                }
                if self.data.extra.fields.is_empty() {
                    ui.label(RichText::new(tr("Add your own columns to tasks, like Budget, Client or Sprint. They show on every task and in the Table view.")).color(t.text2));
                }
                ui.add_space(4.0);
                ui.separator();
                ui.label(RichText::new(tr("New field")).strong());
                const KINDS: [(&str, &str, &str); 5] = [("text", "Text", "Any words"), ("number", "Number", "Amounts, counts, points"), ("dropdown", "Dropdown", "Pick one of your options"), ("checkbox", "Checkbox", "Done or not"), ("date", "Date", "A day on the calendar")];
                let kind_label = KINDS.iter().find(|k| k.0 == kind.as_str()).map(|k| k.1).unwrap_or("Text");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(name).hint_text(tr("Field name, e.g. Budget")).desired_width(240.0).margin(egui::Margin::symmetric(8, 6)));
                    egui::ComboBox::from_id_salt("cf_kind").selected_text(tr(kind_label)).width(150.0).show_ui(ui, |ui| {
                        for (k, l, d) in KINDS {
                            ui.selectable_value(kind, k.to_string(), tr(l)).on_hover_text(tr(d));
                        }
                    });
                });
                if let Some(d) = KINDS.iter().find(|k| k.0 == kind.as_str()).map(|k| k.2) {
                    ui.label(RichText::new(tr(d)).small().color(t.text3));
                }
                if kind == "dropdown" {
                    ui.add(egui::TextEdit::singleline(opts).hint_text(tr("Options, comma separated: Low, Medium, High")).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(8, 6)));
                }
                if primary_button(ui, t, tr("Add field")).clicked() && !name.trim().is_empty() {
                    let id = self.data.new_id();
                    let options = if kind == "dropdown" { opts.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect() } else { vec![] };
                    self.data.extra.fields.push(CustomField { id, name: name.trim().to_string(), kind: kind.clone(), options });
                    name.clear();
                    opts.clear();
                    self.dirty = true;
                }
            }
            Modal::Automations { status, action, value } => {
                let mut remove = None;
                let rules = self.data.extra.automations.clone();
                for (i, a) in rules.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let mut en = a.enabled;
                        if ui.checkbox(&mut en, "").changed() {
                            self.data.extra.automations[i].enabled = en;
                            self.dirty = true;
                        }
                        ui.label(crate::trf!("When status changes to {}, {}", crate::i18n::status_text(&a.trigger_status).to_uppercase(), self.action_text(a)));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new(tr("Delete")).color(t.danger)).frame(false)).clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                }
                if let Some(i) = remove {
                    self.data.extra.automations.remove(i);
                    self.dirty = true;
                }
                if rules.is_empty() {
                    ui.label(RichText::new(tr("No automations yet.")).color(t.text3));
                }
                ui.separator();
                ui.label(RichText::new(tr("New automation")).strong());
                let mut names: Vec<String> = vec![];
                for s in &self.data.spaces {
                    for st in &s.statuses {
                        if !names.contains(&st.name) {
                            names.push(st.name.clone());
                        }
                    }
                }
                ui.horizontal(|ui| {
                    ui.label(tr("When status changes to"));
                    egui::ComboBox::from_id_salt("au_status").selected_text(crate::i18n::status_text(status).to_uppercase()).show_ui(ui, |ui| {
                        for n in &names {
                            ui.selectable_value(status, n.clone(), crate::i18n::status_text(n).to_uppercase());
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(tr("then"));
                    egui::ComboBox::from_id_salt("au_action")
                        .selected_text(match action.as_str() {
                            "assign" => tr("assign to"),
                            "priority" => tr("set priority"),
                            "move" => tr("move to list"),
                            _ => tr("add a comment"),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(action, "assign".to_string(), tr("assign to"));
                            ui.selectable_value(action, "priority".to_string(), tr("set priority"));
                            ui.selectable_value(action, "comment".to_string(), tr("add a comment"));
                            ui.selectable_value(action, "move".to_string(), tr("move to list"));
                        });
                    match action.as_str() {
                        "assign" => {
                            let cur = value.parse::<u64>().ok().and_then(|id| self.data.member(id)).map(|m| m.name.clone()).unwrap_or_else(|| tr("choose").into());
                            egui::ComboBox::from_id_salt("au_val").selected_text(cur).show_ui(ui, |ui| {
                                for m in &self.data.members {
                                    ui.selectable_value(value, m.id.to_string(), &m.name);
                                }
                            });
                        }
                        "priority" => {
                            let cur = value.parse::<usize>().ok().filter(|p| *p < 5).map(|p| tr(PRIORITIES[p].label)).unwrap_or(tr("choose"));
                            egui::ComboBox::from_id_salt("au_val").selected_text(cur).show_ui(ui, |ui| {
                                for p in 0..5usize {
                                    ui.selectable_value(value, p.to_string(), tr(PRIORITIES[p].label));
                                }
                            });
                        }
                        "move" => {
                            let cur = value.parse::<u64>().ok().map(|id| self.data.list_name(id)).unwrap_or_else(|| tr("choose").into());
                            egui::ComboBox::from_id_salt("au_val").selected_text(cur).show_ui(ui, |ui| {
                                for s in &self.data.spaces {
                                    for l in s.lists.iter().chain(s.folders.iter().flat_map(|f| f.lists.iter())) {
                                        ui.selectable_value(value, l.id.to_string(), format!("{} / {}", s.name, l.name));
                                    }
                                }
                            });
                        }
                        _ => {
                            ui.add(egui::TextEdit::singleline(value).hint_text(tr("Comment text")).desired_width(200.0));
                        }
                    }
                });
                if primary_button(ui, t, tr("Add automation")).clicked() && !value.trim().is_empty() {
                    let id = self.data.new_id();
                    self.data.extra.automations.push(Automation { id, enabled: true, trigger_status: status.clone(), action: action.clone(), value: value.trim().to_string(), list: None });
                    value.clear();
                    self.dirty = true;
                }
            }
            Modal::TagManager { from, to } => {
                let mut counts: Vec<(String, usize)> = vec![];
                for task in &self.data.tasks {
                    for tag in &task.tags {
                        match counts.iter_mut().find(|c| &c.0 == tag) {
                            Some(c) => c.1 += 1,
                            None => counts.push((tag.clone(), 1)),
                        }
                    }
                }
                counts.sort();
                if counts.is_empty() {
                    ui.label(RichText::new(tr("No tags yet. Add tags from a task.")).color(t.text3));
                }
                let mut delete = None;
                for (tag, n) in &counts {
                    ui.horizontal(|ui| {
                        chip(ui, tag, tag_color(tag));
                        ui.label(RichText::new(crate::trf!("{} tasks", n)).color(t.text2));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.add(egui::Button::new(RichText::new(tr("Delete")).color(t.danger)).frame(false)).clicked() {
                                delete = Some(tag.clone());
                            }
                            if ui.add(egui::Button::new(tr("Rename")).frame(false)).clicked() {
                                *from = tag.clone();
                                *to = tag.clone();
                            }
                        });
                    });
                }
                if let Some(tag) = delete {
                    for task in &mut self.data.tasks {
                        task.tags.retain(|x| x != &tag);
                    }
                    self.dirty = true;
                }
                if !from.is_empty() {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(crate::trf!("Rename \"{}\" to", from));
                        ui.add(egui::TextEdit::singleline(to).desired_width(140.0));
                        if primary_button(ui, t, tr("Save")).clicked() && !to.trim().is_empty() {
                            let nt = to.trim().to_lowercase();
                            for task in &mut self.data.tasks {
                                for tag in task.tags.iter_mut() {
                                    if tag == from {
                                        *tag = nt.clone();
                                    }
                                }
                                task.tags.dedup();
                            }
                            from.clear();
                            self.dirty = true;
                        }
                    });
                }
            }
            Modal::Templates { name, desc, checklist, priority } => {
                let list = self.default_list();
                ui.label(RichText::new(tr("Start new tasks from a saved shape: name, description, checklist and priority.")).color(t.text2));
                ui.add_space(6.0);
                let mut remove = None;
                for (i, tp) in self.data.extra.templates.clone().iter().enumerate() {
                    egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&tp.name).strong());
                                let mut bits = vec![];
                                if !tp.checklist.is_empty() {
                                    bits.push(if tp.checklist.len() == 1 { tr("1 checklist item").to_string() } else { crate::trf!("{} checklist items", tp.checklist.len()) });
                                }
                                if tp.priority > 0 {
                                    bits.push(crate::trf!("{} priority", tr(PRIORITIES[tp.priority as usize].label)));
                                }
                                if !tp.description.is_empty() {
                                    bits.push(truncate(&tp.description, 40));
                                }
                                if !bits.is_empty() {
                                    ui.label(RichText::new(bits.join(" \u{2022} ")).small().color(t.text3));
                                }
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new(tr("Delete")).color(t.danger)).frame(false)).clicked() {
                                    remove = Some(i);
                                }
                                if primary_button(ui, t, tr("Use")).on_hover_text(tr("Create a task from this template")).clicked() {
                                    if let Some(l) = list {
                                        self.actions.push(Action::Add { list: l, name: tp.task_name.clone(), status: None, priority: tp.priority, assignees: vec![], due: None, parent: None, open: true });
                                        self.pending_template = Some(tp.clone());
                                        close = true;
                                    }
                                }
                            });
                        });
                    });
                    ui.add_space(4.0);
                }
                if let Some(i) = remove {
                    self.data.extra.templates.remove(i);
                    self.dirty = true;
                }
                if self.data.extra.templates.is_empty() {
                    ui.label(RichText::new(tr("No templates yet. Make one below, or open a task and choose \u{2026} > Save as template.")).color(t.text3));
                }
                ui.add_space(4.0);
                ui.separator();
                ui.label(RichText::new(tr("New template")).strong());
                ui.add(egui::TextEdit::singleline(name).hint_text(tr("Template name, e.g. Bug report")).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(8, 6)));
                ui.add(egui::TextEdit::multiline(desc).hint_text(tr("Description (optional)")).desired_rows(2).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(8, 6)));
                ui.add(egui::TextEdit::multiline(checklist).hint_text(tr("Checklist, one item per line (optional)")).desired_rows(3).desired_width(f32::INFINITY).margin(egui::Margin::symmetric(8, 6)));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Priority")).color(t.text2));
                    let cur = if *priority > 0 { tr(PRIORITIES[*priority as usize].label) } else { tr("None") };
                    egui::ComboBox::from_id_salt("tp_pr").selected_text(cur).show_ui(ui, |ui| {
                        ui.selectable_value(priority, 0, tr("None"));
                        for p in 1..PRIORITIES.len() {
                            ui.selectable_value(priority, p as u8, tr(PRIORITIES[p].label));
                        }
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let ok = !name.trim().is_empty();
                        let b = ui.add_enabled(ok, egui::Button::new(RichText::new(tr("Create template")).color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(0.0, 30.0)));
                        if b.clicked() {
                            let id = self.data.new_id();
                            let items: Vec<String> = checklist.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
                            self.data.extra.templates.push(TaskTemplate { id, name: name.trim().to_string(), task_name: name.trim().to_string(), description: desc.trim().to_string(), checklist: items, tags: vec![], priority: *priority });
                            name.clear();
                            desc.clear();
                            checklist.clear();
                            *priority = 0;
                            self.dirty = true;
                        }
                    });
                });
            }
            Modal::Import { path, result } => {
                ui.label(tr("Import tasks from a CSV file (columns: name, status, priority, due date YYYY-MM-DD, assignee name). They go into the current list."));
                ui.add(egui::TextEdit::singleline(path).hint_text(tr("/Users/you/Downloads/tasks.csv")).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if primary_button(ui, t, tr("Import")).clicked() {
                        *result = self.import_csv(path.trim());
                    }
                    if ui.button(tr("Export current tasks to CSV")).clicked() {
                        *result = self.export_csv();
                    }
                });
                if !result.is_empty() {
                    ui.label(RichText::new(result.clone()).color(t.text2));
                }
            }
            Modal::ChannelSettings(cid) => {
                let cid = *cid;
                let Some(ci) = self.data.channels.iter().position(|c| c.id == cid) else { return true };
                let is_channel = self.data.channels[ci].kind == ChannelKind::Channel;
                if is_channel {
                    ui.label(tr("Name"));
                    if ui.add(egui::TextEdit::singleline(&mut self.data.channels[ci].name).desired_width(f32::INFINITY)).changed() {
                        self.dirty = true;
                    }
                    ui.label(tr("Topic"));
                    if ui.add(egui::TextEdit::singleline(&mut self.data.channels[ci].topic).desired_width(f32::INFINITY)).changed() {
                        self.dirty = true;
                    }
                    ui.add_space(6.0);
                    ui.label(tr("Members"));
                    let members = self.data.members.clone();
                    ui.horizontal_wrapped(|ui| {
                        for m in &members {
                            let mut on = self.data.channels[ci].members.contains(&m.id);
                            if ui.checkbox(&mut on, &m.name).changed() {
                                let c = &mut self.data.channels[ci];
                                if on {
                                    c.members.push(m.id);
                                } else {
                                    c.members.retain(|x| *x != m.id);
                                }
                                self.dirty = true;
                            }
                        }
                    });
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Mark as read")).clicked() {
                        self.data.channels[ci].last_read = now_ts();
                        self.dirty = true;
                    }
                    if !is_channel {
                        // only hides it for me; it comes back with the next message
                        if ui.add(egui::Button::new(RichText::new(tr("Close conversation")).color(t.danger))).on_hover_text(tr("Hides it from your sidebar until a new message arrives. Nothing is deleted.")).clicked() {
                            self.close_dm(cid);
                            close = true;
                        }
                        return;
                    }
                    // deleting removes it for everyone: members only, and asked first
                    let member = self.data.channels[ci].members.contains(&self.data.me);
                    let ask = egui::Id::new(("delete_channel", cid));
                    let asking = ui.data(|d| d.get_temp::<bool>(ask).unwrap_or(false));
                    if !asking {
                        let b = ui.add_enabled(member, egui::Button::new(RichText::new(tr("Delete channel")).color(t.danger)));
                        let b = if member { b } else { b.on_disabled_hover_text(tr("Only members of this channel can delete it.")) };
                        if b.clicked() {
                            ui.data_mut(|d| d.insert_temp(ask, true));
                        }
                    }
                });
                let ask = egui::Id::new(("delete_channel", cid));
                if is_channel && ui.data(|d| d.get_temp::<bool>(ask).unwrap_or(false)) {
                    ui.add_space(6.0);
                    let name = self.data.channels[ci].name.clone();
                    ui.label(RichText::new(crate::trf!("Delete #{} for everyone? Its messages are removed for the whole team.", name)).color(t.text));
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new(tr("Delete")).color(Color32::WHITE)).fill(t.danger)).clicked() && self.data.channels[ci].members.contains(&self.data.me) {
                            self.data.channels.remove(ci);
                            if self.chat_open == Some(cid) {
                                self.chat_open = None;
                            }
                            self.dirty = true;
                            close = true;
                            ui.data_mut(|d| d.remove::<bool>(ask));
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            ui.data_mut(|d| d.remove::<bool>(ask));
                        }
                    });
                }
            }
            Modal::Share { emails } => {
                let name = self.data.location_name(self.loc);
                ui.label(crate::trf!("Share \"{}\" with people in your Workspace.", name));
                ui.add_space(4.0);
                for m in self.data.members.clone() {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                        paint_avatar(ui.painter(), r.center(), 10.0, &m, t.card);
                        ui.label(&m.name);
                        ui.label(RichText::new(if m.invited { tr("Invited") } else { role_text(&m.role) }).small().color(t.text3));
                    });
                }
                ui.separator();
                ui.label(tr("Invite by email"));
                ui.add(egui::TextEdit::singleline(emails).hint_text(tr("name@company.com")).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if primary_button(ui, t, tr("Invite")).clicked() && emails.contains('@') {
                        self.modal = Some(Modal::Invite { emails: emails.clone(), role: "Member".into() });
                        close = true;
                    }
                    if ui.button(tr("\u{1F517} Copy link")).clicked() {
                        ui.ctx().copy_text(format!("workboard://{}/{}", self.data.workspace.replace(' ', "-"), name.replace(' ', "-")));
                        self.toast = Some((tr("Link copied").into(), now_ts()));
                    }
                });
            }
            Modal::CustomizeNav => {
                ui.label(tr("Choose the apps pinned to the left bar."));
                for label in ["Planner", "AI", "Teams", "Chat", "Docs", "Dashboards", "Whiteboards", "Forms", "Goals", "Timesheets"] {
                    let mut on = self.data.extra.rail_items.iter().any(|r| r == label);
                    if ui.checkbox(&mut on, tr(label)).changed() {
                        if on {
                            self.data.extra.rail_items.push(label.to_string());
                        } else {
                            self.data.extra.rail_items.retain(|r| r != label);
                        }
                        self.dirty = true;
                    }
                }
            }
            Modal::CustomizeSidebar => {
                ui.label(tr("Choose what shows in your sidebar."));
                for label in ["Inbox", "Replies", "Assigned Comments", "Posts", "Skills", "Artifacts", "Meetings", "My Tasks", "AI Chats", "Favorites", "Channels", "Direct Messages"] {
                    let mut on = !self.data.extra.hidden_nav.iter().any(|r| r == label);
                    if ui.checkbox(&mut on, tr(label)).changed() {
                        if on {
                            self.data.extra.hidden_nav.retain(|r| r != label);
                        } else {
                            self.data.extra.hidden_nav.push(label.to_string());
                        }
                        self.dirty = true;
                    }
                }
            }
            Modal::TrackTime { task } => {
                let tasks: Vec<(u64, String)> = self.data.tasks.iter().map(|x| (x.id, x.name.clone())).collect();
                let cur = tasks.iter().find(|x| x.0 == *task).map(|x| x.1.clone()).unwrap_or_else(|| tr("Choose a task").into());
                egui::ComboBox::from_id_salt("tt_task").selected_text(cur).width(380.0).show_ui(ui, |ui| {
                    for (id, n) in &tasks {
                        ui.selectable_value(task, *id, n);
                    }
                });
                let now = now_ts();
                let running = self.data.tasks.iter().find(|x| x.timer_started.is_some()).map(|x| (x.id, x.name.clone(), x.tracked_secs(now)));
                if let Some((_, n, secs)) = &running {
                    ui.label(crate::trf!("Running: {} \u{2022} {}", n, fmt_duration(*secs)));
                    ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                }
                ui.horizontal(|ui| {
                    let is_running = running.as_ref().map(|r| r.0 == *task).unwrap_or(false);
                    if primary_button(ui, t, if is_running { tr("\u{23F9} Stop timer") } else { tr("\u{25B6} Start timer") }).clicked() && *task != 0 {
                        self.actions.push(Action::ToggleTimer(*task));
                    }
                    if ui.button(tr("Open Timesheets")).clicked() {
                        self.section = Section::Timesheets;
                        close = true;
                    }
                });
            }
            Modal::Reminder { text, date, time } => {
                let r = ui.add(egui::TextEdit::singleline(text).hint_text(tr("Remind me to...")).desired_width(f32::INFINITY));
                r.request_focus();
                ui.horizontal(|ui| {
                    let d_txt = date.map(fmt_date).unwrap_or_else(|| tr("\u{1F4C5} Date").into());
                    let r = ui.button(d_txt);
                    popup(ui, &r, ui.id().with("rem_date"), 380.0, |ui| date_menu(ui, t, ui.id().with("rem_cal"), date));
                    ui.add(egui::TextEdit::singleline(time).hint_text("09:00").desired_width(60.0));
                });
                if primary_button(ui, t, tr("Create Reminder")).clicked() && !text.trim().is_empty() {
                    let d = date.unwrap_or_else(today);
                    let (h, m) = time.split_once(':').map(|(h, m)| (h.trim().parse().unwrap_or(9), m.trim().parse().unwrap_or(0))).unwrap_or((9, 0));
                    let ts = d.and_hms_opt(h.min(23), m.min(59), 0).and_then(|x| x.and_local_timezone(chrono::Local).single()).map(|x| x.timestamp()).unwrap_or_else(now_ts);
                    let id = self.data.new_id();
                    self.data.extra.reminders.push(Reminder { id, text: text.trim().to_string(), due_ts: ts, done: false, owner: self.data.me });
                    self.dirty = true;
                    self.toast = Some((tr("Reminder created").into(), now_ts()));
                    close = true;
                }
            }
            Modal::Trash => {
                if self.data.extra.trash.is_empty() {
                    ui.label(RichText::new(tr("Trash is empty.")).color(t.text3));
                }
                let mut restore = None;
                let mut purge = None;
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for (i, item) in self.data.extra.trash.iter().enumerate().rev() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr(&item.kind)).small().color(t.text3));
                            ui.label(&item.title);
                            ui.label(RichText::new(fmt_ago(item.deleted)).small().color(t.text3));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new(tr("Delete forever")).color(t.danger)).frame(false)).clicked() {
                                    purge = Some(i);
                                }
                                if ui.button(tr("Restore")).clicked() {
                                    restore = Some(i);
                                }
                            });
                        });
                    }
                });
                if let Some(i) = restore {
                    self.restore_trash(i);
                }
                if let Some(i) = purge {
                    self.data.extra.trash.remove(i);
                    self.dirty = true;
                }
            }
            Modal::Unavailable { body, .. } => {
                ui.label(tr(body.as_str()));
                ui.add_space(8.0);
                if primary_button(ui, t, tr("OK")).clicked() {
                    close = true;
                }
            }
            Modal::Bookmark { title, url } => {
                ui.add(egui::TextEdit::singleline(title).hint_text(tr("Name")).desired_width(f32::INFINITY));
                ui.add(egui::TextEdit::singleline(url).hint_text(tr("https://...")).desired_width(f32::INFINITY));
                if primary_button(ui, t, tr("Add Bookmark")).clicked() && !url.trim().is_empty() {
                    let id = self.data.new_id();
                    let ttl = if title.trim().is_empty() { url.trim().to_string() } else { title.trim().to_string() };
                    self.data.extra.bookmarks.push(Bookmark { id, title: ttl, url: url.trim().to_string() });
                    self.dirty = true;
                    close = true;
                }
            }
            Modal::SwitchUser => {
                ui.label(tr("Who's using the app? Everyone on this computer shares the same Workspace."));
                ui.add_space(4.0);
                for m in self.data.members.clone() {
                    let ring = t.card;
                    let label = if m.id == self.data.me { crate::trf!("{} (current)", m.name) } else if m.invited { crate::trf!("{} (invited)", m.name) } else { m.name.clone() };
                    if menu_row(ui, t, &label, m.id == self.data.me, |p, c| paint_avatar(p, c, 9.0, &m, ring)).clicked() {
                        self.data.me = m.id;
                        if let Some(mm) = self.data.members.iter_mut().find(|x| x.id == m.id) {
                            mm.invited = false;
                        }
                        self.dirty = true;
                        close = true;
                    }
                }
            }
            Modal::NewDashboard { name } => {
                ui.add(egui::TextEdit::singleline(name).desired_width(f32::INFINITY));
                let spaces: Vec<(u64, String)> = self.data.spaces.iter().map(|s| (s.id, s.name.clone())).collect();
                ui.label(RichText::new(tr("Shows the whole Workspace; pick a Space later from the Dashboards page.")).small().color(t.text2));
                let _ = spaces;
                if primary_button(ui, t, tr("Create Dashboard")).clicked() && !name.trim().is_empty() {
                    let id = self.data.new_id();
                    self.data.extra.dashboards.push(Dashboard { id, name: name.trim().to_string(), space: None });
                    self.dash_open = Some(id);
                    self.section = Section::Dashboards;
                    self.dirty = true;
                    close = true;
                }
            }
        }
        close
    }

    /// Adds invited people (skipping emails already in the Workspace); they join #general.
    /// Returns how many were added.
    pub(super) fn invite_people(&mut self, emails: &[String], role: &str) -> usize {
        let mut added = 0;
        for e in emails {
            let e = e.trim();
            if !(e.contains('@') && e.contains('.')) || self.data.members.iter().any(|m| m.email.eq_ignore_ascii_case(e)) {
                continue;
            }
            let id = self.data.new_id();
            let name = e.split('@').next().unwrap_or(e).replace(['.', '_'], " ");
            let name = name
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ");
            let color = STATUS_PALETTE[1 + (id as usize % 13)];
            self.data.members.push(Member { id, name, color, email: e.to_string(), role: role.to_string(), invited: true, status_text: String::new() });
            if let Some(g) = self.data.channels.iter_mut().find(|c| c.kind == ChannelKind::Channel && c.name == "general") {
                g.members.push(id);
            }
            added += 1;
        }
        if added > 0 {
            self.dirty = true;
        }
        added
    }

    pub(super) fn describe_action(&self, a: &Automation) -> String {
        match a.action.as_str() {
            "assign" => format!("assign to {}", a.value.parse::<u64>().ok().and_then(|id| self.data.member(id)).map(|m| m.name.clone()).unwrap_or_default()),
            "priority" => format!("set priority to {}", a.value.parse::<usize>().ok().filter(|p| *p < 5).map(|p| PRIORITIES[p].label).unwrap_or("")),
            "move" => format!("move to {}", a.value.parse::<u64>().ok().map(|id| self.data.list_name(id)).unwrap_or_default()),
            _ => format!("comment \"{}\"", a.value),
        }
    }

    /// `describe_action` in the interface language (the English one goes into the activity log).
    fn action_text(&self, a: &Automation) -> String {
        match a.action.as_str() {
            "assign" => crate::trf!("assign to {}", a.value.parse::<u64>().ok().and_then(|id| self.data.member(id)).map(|m| m.name.clone()).unwrap_or_default()),
            "priority" => crate::trf!("set priority to {}", a.value.parse::<usize>().ok().filter(|p| *p < 5).map(|p| tr(PRIORITIES[p].label)).unwrap_or("")),
            "move" => crate::trf!("move to {}", a.value.parse::<u64>().ok().map(|id| self.data.list_name(id)).unwrap_or_default()),
            _ => crate::trf!("comment \"{}\"", a.value),
        }
    }

    pub(super) fn import_csv(&mut self, path: &str) -> String {
        let Ok(text) = std::fs::read_to_string(path) else { return crate::trf!("Couldn't read {}", path) };
        let Some(list) = self.default_list() else { return tr("Open a list first").into() };
        let statuses = self.data.statuses_for_list(list);
        let mut n = 0;
        for (i, line) in text.lines().enumerate() {
            let cols: Vec<String> = line.split(',').map(|c| c.trim().trim_matches('"').to_string()).collect();
            if cols.is_empty() || cols[0].is_empty() {
                continue;
            }
            if i == 0 && cols[0].eq_ignore_ascii_case("name") {
                continue;
            }
            let status = cols.get(1).and_then(|s| statuses.iter().find(|st| st.name.eq_ignore_ascii_case(s))).map(|s| s.name.clone());
            let priority = cols.get(2).and_then(|p| PRIORITIES.iter().position(|d| d.label.eq_ignore_ascii_case(p))).unwrap_or(0) as u8;
            let due = cols.get(3).and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
            let assignees = cols.get(4).and_then(|a| self.data.members.iter().find(|m| m.name.eq_ignore_ascii_case(a))).map(|m| vec![m.id]).unwrap_or_default();
            self.actions.push(Action::Add { list, name: cols[0].clone(), status, priority, assignees, due, parent: None, open: false });
            n += 1;
        }
        crate::trf!("Imported {} tasks into {}", n, self.data.list_name(list))
    }

    pub(super) fn export_csv(&self) -> String {
        let mut out = String::from("name,status,priority,due,assignee,list\n");
        for t in self.filtered() {
            let who = t.assignees.first().and_then(|a| self.data.member(*a)).map(|m| m.name.clone()).unwrap_or_default();
            out.push_str(&format!(
                "\"{}\",{},{},{},{},\"{}\"\n",
                t.name.replace('"', "'"),
                t.status,
                PRIORITIES[t.priority as usize].label,
                t.due.map(|d| d.to_string()).unwrap_or_default(),
                who,
                self.data.list_name(t.list_id)
            ));
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let path = PathBuf::from(home).join("Downloads").join(format!("tasks-{}.csv", today()));
        match std::fs::write(&path, out) {
            Ok(_) => crate::trf!("Saved {}", path.display()),
            Err(e) => crate::trf!("Couldn't save: {}", e),
        }
    }

    pub(super) fn restore_trash(&mut self, i: usize) {
        let item = self.data.extra.trash.remove(i);
        match item.kind.as_str() {
            "Task" => {
                if let Ok(tasks) = serde_json::from_str::<Vec<Task>>(&item.payload) {
                    self.data.tasks.extend(tasks);
                }
            }
            "Doc" => {
                if let Ok(d) = serde_json::from_str::<Doc>(&item.payload) {
                    self.data.docs.push(d);
                }
            }
            "List" => {
                if let Ok((sid, l, tasks)) = serde_json::from_str::<(u64, ListItem, Vec<Task>)>(&item.payload) {
                    let idx = self.data.spaces.iter().position(|s| s.id == sid).unwrap_or(0);
                    if let Some(s) = self.data.spaces.get_mut(idx) {
                        s.lists.push(l);
                    }
                    self.data.tasks.extend(tasks);
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    /// "New message" picker (sidebar + New message, Create > Message, Cmd+G).
    pub(super) fn new_message_window(&mut self, ctx: &egui::Context) {
        if !self.new_message_open {
            ctx.data_mut(|d| {
                d.remove::<(Section, Location)>(egui::Id::new("new_message_at"));
                d.remove::<String>(egui::Id::new("new_message_query"));
                d.remove::<u8>(egui::Id::new("new_message_tab"));
            });
            return;
        }
        // moving to another page closes it, as does Escape
        let here = (self.section, self.loc);
        let at = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("new_message_at"), || here));
        if at != here || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.new_message_open = false;
            ctx.data_mut(|d| d.remove::<(Section, Location)>(egui::Id::new("new_message_at")));
            return;
        }
        let t = self.t();
        let mut open = true;
        let me = self.data.me;
        let q_id = egui::Id::new("new_message_query");
        let tab_id = egui::Id::new("new_message_tab");
        let mut q: String = ctx.data(|d| d.get_temp(q_id)).unwrap_or_default();
        let mut tab: u8 = ctx.data(|d| d.get_temp(tab_id)).unwrap_or(0);
        let mut go: Option<u64> = None;
        let mut dm_with: Option<u64> = None;
        let win_id = egui::Id::new("new_message_window");
        egui::Window::new("New message dialog")
            .id(win_id)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 90.0])
            .min_width(620.0)
            .max_width(620.0)
            .frame(dialog_frame(&t))
            .show(ctx, |ui| {
                if dialog_header(ui, &t, tr("New message")) {
                    open = false;
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("To:")).color(t.text2));
                    let r = ui.add(egui::TextEdit::singleline(&mut q).hint_text(tr("People or Channels")).desired_width(f32::INFINITY));
                    name_field(&r, "People or Channels");
                    focus_once(ui, &r);
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    for (i, label) in ["Recents", "People", "Channels"].iter().enumerate() {
                        if ui.selectable_label(tab == i as u8, tr(label)).clicked() {
                            tab = i as u8;
                        }
                    }
                });
                ui.separator();
                let ql = q.trim().to_lowercase();
                let hit = |s: &str| ql.is_empty() || s.to_lowercase().contains(&ql);
                let members = self.data.members.clone();
                // people: an avatar row on Recents, a full list on People
                if tab != 2 {
                    ui.label(RichText::new(tr("Direct Messages")).small().color(t.text3));
                    ui.horizontal_wrapped(|ui| {
                        for m in members.iter().filter(|m| !m.invited && hit(&m.name)) {
                            let label = if m.id == me { crate::trf!("{} (You)", m.name) } else { m.name.clone() };
                            let (r, resp) = ui.allocate_exact_size(vec2(72.0, 64.0), Sense::click());
                            name_it(&resp, &format!("Message {}", m.name));
                            if resp.hovered() {
                                ui.painter().rect_filled(r, 8.0, t.hover);
                            }
                            paint_avatar(ui.painter(), r.center_top() + vec2(0.0, 22.0), 16.0, m, t.card);
                            let short = truncate(&label, 10);
                            ui.painter().text(r.center_bottom() - vec2(0.0, 10.0), Align2::CENTER_CENTER, short, FontId::proportional(11.0), t.text2);
                            if resp.clicked() {
                                dm_with = Some(m.id);
                            }
                        }
                    });
                }
                if tab == 1 {
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr("People")).small().color(t.text3));
                    for m in members.iter().filter(|m| hit(&m.name) || hit(&m.email)) {
                        let label = if m.id == me { crate::trf!("{} \u{2014} You", m.name) } else { m.name.clone() };
                        let ring = t.card;
                        if menu_row(ui, &t, &label, false, |p, c| paint_avatar(p, c, 9.0, m, ring)).clicked() {
                            dm_with = Some(m.id);
                        }
                    }
                }
                if tab != 1 {
                    ui.add_space(4.0);
                    let mut chans: Vec<(u64, String, i64)> = self
                        .data
                        .channels
                        .iter()
                        .filter(|c| (c.kind == ChannelKind::Channel || tab == 0) && self.can_see(c) && !self.dm_duplicate(c))
                        .map(|c| (c.id, self.channel_label(c), c.messages.last().map(|m| m.ts).unwrap_or(c.last_read)))
                        .filter(|c| hit(&c.1))
                        .collect();
                    if tab == 0 {
                        chans.sort_by(|a, b| b.2.cmp(&a.2));
                        chans.truncate(8);
                    }
                    ui.label(RichText::new(if tab == 0 { tr("Recents") } else { tr("Channels") }).small().color(t.text3));
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        for (id, label, _) in &chans {
                            if menu_row(ui, &t, label, false, |p, c| {
                                p.text(c, Align2::CENTER_CENTER, if label.starts_with('#') { "#" } else { "@" }, FontId::proportional(13.0), t.text2);
                            })
                            .clicked()
                            {
                                go = Some(*id);
                            }
                        }
                        if chans.is_empty() {
                            ui.label(RichText::new(tr("No matches")).color(t.text3));
                        }
                    });
                }
            });
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, win_id));
        if let Some(uid) = dm_with {
            let existing = self.data.channels.iter().find(|c| c.kind == ChannelKind::Direct && ((uid == me && c.members == vec![me]) || (uid != me && c.members.contains(&uid) && c.members.contains(&me))) && !self.dm_duplicate(c)).map(|c| c.id);
            go = Some(existing.unwrap_or_else(|| {
                let id = self.data.new_id();
                let members = if uid == me { vec![me] } else { vec![me, uid] };
                self.data.channels.push(Channel { id, name: String::new(), kind: ChannelKind::Direct, topic: String::new(), members, messages: vec![], last_read: now_ts(), ..Default::default() });
                self.dirty = true;
                id
            }));
        }
        if let Some(id) = go {
            self.section = Section::Chat;
            self.chat_open = Some(id);
            open = false;
        }
        if open {
            ctx.data_mut(|d| {
                d.insert_temp(q_id, q);
                d.insert_temp(tab_id, tab);
            });
        } else {
            self.new_message_open = false;
            ctx.data_mut(|d| {
                d.remove::<String>(q_id);
                d.remove::<u8>(tab_id);
            });
        }
    }

    /// Floating personal notepad.
    pub(super) fn notepad(&mut self, ctx: &egui::Context) {
        if !self.notepad_open {
            return;
        }
        let mut open = true;
        egui::Window::new(tr("Notepad")).id(egui::Id::new("Notepad")).open(&mut open).default_size([360.0, 420.0]).default_pos([ctx.screen_rect().right() - 400.0, 80.0]).show(ctx, |ui| {
            if ui.add(egui::TextEdit::multiline(&mut self.data.extra.notepad).hint_text(tr("Write anything. Only you can see this.")).desired_width(f32::INFINITY).desired_rows(18)).changed() {
                self.dirty = true;
            }
        });
        if !open {
            self.notepad_open = false;
        }
    }

    /// A short message at the bottom of the window.
    pub(super) fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, ts)) = self.toast.clone() else { return };
        if now_ts() - ts > 5 {
            self.toast = None;
            return;
        }
        let t = self.t();
        egui::Area::new(egui::Id::new("toast")).order(egui::Order::Foreground).anchor(Align2::CENTER_BOTTOM, [0.0, -24.0]).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgb(32, 32, 32)).corner_radius(8).inner_margin(egui::Margin::symmetric(16, 10)).show(ui, |ui| {
                ui.label(RichText::new(msg).color(Color32::WHITE));
            });
        });
        let _ = t;
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}
