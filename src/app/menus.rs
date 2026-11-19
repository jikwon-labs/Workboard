// Menus and dialogs measured from the person's ClickUp: workspace menu,
// profile menu with personal tools, the sidebar Create menu, the rail More
// menu, team invite, people and roles, settings, themes, notifications,
// shortcuts, custom fields, automations, tag manager, templates, CSV import
// and export, channel settings, share, trash, reminders and the notepad.
// Online-only features (AI, Clips, Talk to Text, huddles) open a dialog that
// says they come with the shared server. The bigger dialogs' bodies live in
// menus/ (workspace, prefs, fields, items); modal_body picks one.

use super::icons::{icon_button, icon_button_ex, line_icon};
use super::*;
use egui::Response;

mod fields;
mod items;
mod prefs;
mod workspace;

pub(super) enum Modal {
    Invite {
        emails: String,
        role: String,
    },
    WorkspaceSettings {
        name: String,
    },
    /// A new workspace, set up a step at a time: its name, the apps on the
    /// left bar, then (on a team server) the people to invite.
    NewWorkspace {
        step: u8,
        name: String,
        apps: Vec<String>,
        emails: String,
        role: String,
        /// make it on the team server the open workspace is on (when there
        /// is one), else on this computer only
        on_server: bool,
    },
    /// Edit profile (`profile.rs`).
    Profile {
        name: String,
        display: String,
        title: String,
        email: String,
        photo: Option<Attachment>,
    },
    SetStatus {
        text: String,
    },
    Themes,
    Shortcuts,
    Help,
    Labs,
    Notifications,
    ClickApps,
    CustomFields {
        name: String,
        kind: String,
        opts: String,
    },
    /// `scope`: the List or Space it was opened from (new rules run only there).
    Automations {
        status: String,
        action: String,
        value: String,
        scope: Option<Location>,
    },
    TagManager {
        from: String,
        to: String,
    },
    /// Browse, preview and use templates; manage the team's.
    TemplateCenter(Box<super::templates::TemplateCenter>),
    Import {
        path: String,
        result: String,
    },
    ChannelSettings(u64),
    /// My saved messages from every conversation I can see.
    SavedMessages,
    Share {
        emails: String,
    },
    CustomizeNav,
    CustomizeSidebar,
    TrackTime {
        task: u64,
    },
    Reminder {
        text: String,
        date: Option<NaiveDate>,
        time: String,
    },
    Trash,
    Unavailable {
        title: String,
        body: String,
    },
    /// A Slack export read and waiting for Import (slack.rs).
    SlackImport(Box<super::slack::SlackPlan>),
    Bookmark {
        title: String,
        url: String,
    },
    SwitchUser,
    NewDashboard {
        name: String,
    },
    /// Share the workspace through a folder, or join one.
    /// `path`: the shared folder typed in; `addr`: the server address typed in.
    /// `code`: an invite code typed in.
    Together {
        path: String,
        addr: String,
        code: String,
        result: String,
    },
    WhoAmI {
        name: String,
        email: String,
    },
    Call {
        ch: u64,
        link: String,
    },
    NewList {
        name: String,
        space: u64,
        folder: Option<u64>,
    },
    NewSpace {
        name: String,
        color: [u8; 3],
    },
    MoveTasks {
        ids: Vec<u64>,
        query: String,
    },
    MoveList {
        list: u64,
    },
    /// Share & permissions for a space (sharing.rs); `draft` until Save.
    SpaceSharing {
        space: u64,
        draft: SpaceSharing,
    },
    NewChannel {
        name: String,
        private: bool,
        add_list: bool,
    },
    ListInfo {
        list: u64,
    },
    /// Pick a task to set as parent, subtask, dependency or link.
    PickTask {
        task: u64,
        mode: super::TaskPick,
        query: String,
    },
    /// "Delete this?" for a dashboard, whiteboard, form, goal, meeting, team
    /// or person (see `remove_item`).
    ConfirmRemove {
        kind: &'static str,
        id: u64,
        name: String,
    },
    /// Joining a team from the workspace menu or an invite link while this
    /// computer already has a workspace: the team's opens as a new one.
    /// `addr`: the server address typed in.
    JoinTeam {
        addr: String,
    },
    /// Asking before logging out of the team server (login.rs).
    LogOut,
    /// Asking before deleting the open workspace, its name typed in
    /// (delete_ws.rs).
    DeleteWorkspace {
        typed: String,
    },
    /// Check for updates on Windows, where the app updates itself
    /// (`updater::status_ui`; Sparkle has its own window on macOS). Last, so
    /// the other dialogs keep their window ids.
    Update,
}

pub const ROLES: [(&str, &str); 4] = [
    ("Admin", "Can manage people, settings and all items."),
    ("Member", "Can access all public items in your workspace."),
    ("Guest", "Can only access items shared with them."),
    ("Owner", "Owns the Workspace."),
];

/// Personal tools as listed in the original's profile menu.
pub const TOOLS: [(&str, &str, &str); 12] = [
    ("create_task", "Create task", "check_plus"),
    ("my_work", "My Work", "briefcase"),
    ("track_time", "Track Time", "clock"),
    ("notepad", "Notepad", "edit"),
    ("clip", "Record a Clip", "video"),
    ("stt", "Talk to Text", "mic"),
    ("reminder", "Create Reminder", "alarm"),
    ("doc", "Create Doc", "docs"),
    ("whiteboard", "Create Whiteboard", "whiteboards"),
    ("people", "View People", "teams"),
    ("dashboard", "Create Dashboard", "dashboards"),
    ("notetaker", "AI Notetaker", "notebook"),
];

pub fn unavailable(what: &str) -> Modal {
    Modal::Unavailable {
        title: what.to_string(),
        body: crate::trf!(
            "{} needs an online service for AI, video or voice processing, which Workboard doesn't have. Everything else in the app works, on your own or with your team.",
            tr(what)
        ),
    }
}

/// A menu row: a line icon (`icon` is its kind), the label and an optional
/// hint on the right.
fn menu_item(ui: &mut Ui, t: &Theme, icon: &str, label: &str, right: &str) -> Response {
    let w = ui.available_width().max(200.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 27.0), Sense::click());
    name_it(&resp, label);
    if resp.hovered() {
        ui.painter().rect_filled(r, 6.0, t.hover);
    }
    line_icon(
        ui.painter(),
        icon,
        r.left_center() + vec2(16.0, 0.0),
        t.text2,
    );
    ui.painter().text(
        r.left_center() + vec2(32.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        t.text,
    );
    if !right.is_empty() {
        ui.painter().text(
            r.right_center() - vec2(10.0, 0.0),
            Align2::RIGHT_CENTER,
            crate::os::keys(right),
            FontId::proportional(12.0),
            t.text3,
        );
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
    line_icon(ui.painter(), icon, r.left_top() + vec2(16.0, 14.0), t.text2);
    ui.painter().text(
        r.left_top() + vec2(32.0, 14.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        t.text,
    );
    ui.painter().text(
        r.left_top() + vec2(32.0, 32.0),
        Align2::LEFT_CENTER,
        sub,
        FontId::proportional(12.0),
        t.text2,
    );
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

/// The window a dialog of this kind is drawn in.
/// The apps that can be pinned to the left bar, in the order offered.
const RAIL_APPS: [&str; 10] = [
    "Planner",
    "AI",
    "Teams",
    "Chat",
    "Docs",
    "Dashboards",
    "Whiteboards",
    "Forms",
    "Goals",
    "Timesheets",
];

/// How many steps the new-workspace dialog has: inviting is the third, and
/// only on a team server; one only on this computer has nobody to invite
/// until it's shared from Work together.
fn new_ws_steps(on_server: bool) -> u8 {
    if on_server {
        3
    } else {
        2
    }
}

impl Modal {
    /// The new-workspace dialog at its first step, with the apps a new
    /// workspace pins by default already chosen.
    pub(super) fn new_workspace() -> Modal {
        Modal::NewWorkspace {
            step: 0,
            name: String::new(),
            apps: Extra::default().rail_items,
            emails: String::new(),
            role: "Member".into(),
            on_server: true,
        }
    }

    /// The Create Reminder dialog with `text` filled in, on `date` at 9:00.
    pub(super) fn reminder(text: String, date: NaiveDate) -> Modal {
        Modal::Reminder {
            text,
            date: Some(date),
            time: "09:00".into(),
        }
    }

    /// The Custom fields dialog with an empty new text field.
    pub(super) fn custom_fields() -> Modal {
        Modal::CustomFields {
            name: String::new(),
            kind: "text".into(),
            opts: String::new(),
        }
    }

    /// The Automations dialog with an empty new rule (when complete, comment);
    /// new rules run in `scope`, a List or Space, or the whole Workspace.
    pub(super) fn automations(scope: Option<Location>) -> Modal {
        Modal::Automations {
            status: "complete".into(),
            action: "comment".into(),
            value: String::new(),
            scope,
        }
    }
}

pub(super) fn modal_window_id(m: &Modal) -> egui::Id {
    egui::Id::new(("modal_window", std::mem::discriminant(m)))
}

impl App {
    // ---------------------------------------------------------------- menus

    pub(super) fn workspace_menu(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.set_width(300.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            ui.painter()
                .rect_filled(r, 8.0, rgb(self.data.extra.workspace_color));
            let letter = self
                .data
                .workspace
                .chars()
                .next()
                .unwrap_or('W')
                .to_uppercase()
                .to_string();
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                letter,
                FontId::proportional(18.0),
                Color32::WHITE,
            );
            ui.vertical(|ui| {
                ui.label(RichText::new(&self.data.workspace).size(16.0));
                let sub = match &self.data.extra.shared {
                    Some(c) if c.server.is_some() => {
                        tr("On your team's server \u{2022} every feature included")
                    }
                    Some(_) => tr("Shared with your team \u{2022} every feature included"),
                    None => tr("On this computer \u{2022} every feature included"),
                };
                ui.label(RichText::new(sub).small().color(t.text2));
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 8.0) / 2.0;
            if icon_button_ex(
                ui,
                &t,
                "settings",
                tr("Settings"),
                "Settings",
                true,
                w,
                t.text,
                t.text2,
            )
            .clicked()
            {
                self.open_settings("General");
                close_popup(ui);
            }
            if icon_button_ex(
                ui,
                &t,
                "teams",
                tr("People"),
                "People",
                true,
                w,
                t.text,
                t.text2,
            )
            .clicked()
            {
                self.section = Section::People;
                close_popup(ui);
            }
        });
        ui.separator();
        section_label(ui, &t, tr("Manage"));
        let items: [(&str, &str); 5] = [
            ("apps", "Apps"),
            ("templates", "Templates"),
            ("fields", "Custom fields"),
            ("robot", "Automations"),
            ("tag", "Tag manager"),
        ];
        let manages = self.may_manage();
        for (icon, label) in items {
            // only Templates (to use one) for a plain member on a server
            if !manages && label != "Templates" {
                continue;
            }
            if menu_item(ui, &t, icon, tr(label), "").clicked() {
                self.modal = Some(match label {
                    "Apps" => Modal::ClickApps,
                    "Templates" => self.template_center(super::templates::Category::All, None),
                    "Custom fields" => Modal::custom_fields(),
                    "Automations" => Modal::automations(None),
                    _ => Modal::TagManager {
                        from: String::new(),
                        to: String::new(),
                    },
                });
                close_popup(ui);
            }
        }
        let label = if self.data.extra.shared.is_some() {
            tr("Work together (shared)")
        } else {
            tr("Work together")
        };
        if menu_item(ui, &t, "teams", label, "").clicked() {
            self.modal = Some(self.together_modal());
            close_popup(ui);
        }
        let others: Vec<WsEntry> = Data::workspaces()
            .into_iter()
            .filter(|w| w.file != self.ws_file)
            .collect();
        if !others.is_empty() || self.ws_file != "data.json" {
            ui.separator();
            section_label(ui, &t, tr("Switch Workspace"));
            let mut all = others;
            // this computer's own workspace, unless it was deleted
            let own = !self.persist || Data::path().is_file();
            if own && self.ws_file != "data.json" && !all.iter().any(|w| w.file == "data.json") {
                all.insert(
                    0,
                    WsEntry {
                        name: "My Workspace".into(),
                        file: "data.json".into(),
                    },
                );
            }
            for w in all {
                // workspaces often share a name ("My Workspace"): say where each one lives
                let place = self.workspace_place(&w.file);
                if menu_item(ui, &t, "building", &w.name, tr(place)).clicked() {
                    self.switch_workspace(&w.file);
                    close_popup(ui);
                }
            }
        }
        ui.separator();
        if menu_item(ui, &t, "plus", tr("Create Workspace"), "").clicked() {
            self.modal = Some(Modal::new_workspace());
            close_popup(ui);
        }
        // someone else's team: it opens as another workspace here, and the
        // ones already on this computer (on a server or not) stay as they are
        if menu_item(ui, &t, "login", tr("Join a team"), "").clicked() {
            self.open_join_team(String::new());
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
        let path = if file == "data.json" {
            Data::path()
        } else {
            Data::dir().join(file)
        };
        let cfg: Option<crate::sync::SharedCfg> = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| serde_json::from_value(v.get("extra")?.get("shared")?.clone()).ok());
        let place = label(cfg.as_ref());
        self.ws_places.insert(file.to_string(), place);
        place
    }

    pub(super) fn switch_workspace(&mut self, file: &str) {
        // a status editor still open: its edits are done (and saved with
        // this workspace) like on closing it, and it doesn't stay open on
        // the next workspace
        self.finish_status_edit();
        if self.persist {
            self.save_now();
        }
        self.open_workspace(file);
    }

    /// Open workspace `file` in place of the one open, which isn't saved
    /// (switch_workspace saves it first). Nothing changes when it can't be read.
    pub(super) fn open_workspace(&mut self, file: &str) {
        self.ws_places.clear();
        let d = if file == "data.json" {
            Some(Data::load())
        } else {
            Data::load_file(file)
        };
        if let Some(p) = Data::take_unreadable() {
            self.toast(super::unreadable_note(&p));
        }
        if let Some(mut d) = d {
            d.fold_legacy_slack();
            super::together::fill_sync_baseline(&mut d);
            // task attachments on their way stay written down with the
            // workspace they belong to and come back with it
            self.save_task_files();
            self.task_files.clear();
            self.task_files_saved = None;
            self.data = d;
            self.data_changed();
            self.ws_file = file.to_string();
            if self.persist {
                Data::set_last_workspace(file);
            }
            self.loc = self
                .data
                .first_list()
                .map(Location::List)
                .unwrap_or(Location::Home);
            self.section = Section::Home;
            self.open_task = None;
            self.chat.open = None;
            self.recent.clear();
            self.clear_history();
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
                    ui.label(RichText::new(m.shown()).size(15.0));
                    if !m.title.trim().is_empty() {
                        ui.label(RichText::new(m.title.trim()).small().color(t.text2));
                    }
                    let muted = self.data.extra.muted_until > now_ts();
                    let st: String = if !m.status_text.is_empty() {
                        m.status_text.clone()
                    } else if muted {
                        tr("Notifications muted").into()
                    } else if self.connected() {
                        tr("Online").into()
                    } else if self.on_server() {
                        tr("Offline").into()
                    } else {
                        String::new()
                    };
                    if !st.is_empty() {
                        ui.label(RichText::new(st).small().color(t.text2));
                    }
                });
            });
        }
        // a button, drawn like the other framed buttons (in grey text it read
        // as an empty text field)
        let w = ui.available_width();
        if icon_button_ex(
            ui,
            &t,
            "smile",
            tr("Set status"),
            "Set status",
            true,
            w,
            t.text,
            t.text2,
        )
        .clicked()
        {
            let cur = self
                .data
                .member(me)
                .map(|m| m.status_text.clone())
                .unwrap_or_default();
            self.modal = Some(Modal::SetStatus { text: cur });
            close_popup(ui);
        }
        if menu_item(ui, &t, "person", tr("Edit profile"), "").clicked() {
            self.open_profile();
            close_popup(ui);
        }
        // a flyout beside the row (egui's sub menu drew over this menu): to the
        // right when it fits, otherwise to the left of the whole menu
        let mr = menu_item(ui, &t, "bell_off", tr("Mute notifications"), "\u{203A}");
        let fly = ui.id().with("mute_fly");
        let keep: bool = ui.data(|d| d.get_temp(fly)).unwrap_or(false);
        let fly_w: f32 = ui.data(|d| d.get_temp(fly.with("w"))).unwrap_or(170.0);
        let menu = ui
            .max_rect()
            .expand(egui::Frame::popup(ui.style()).total_margin().left);
        let on_right = menu.right() + 6.0 + fly_w <= ui.ctx().screen_rect().right();
        let fly_x = if on_right {
            menu.right() + 6.0
        } else {
            menu.left() - 6.0 - fly_w
        };
        let mut over = false;
        if mr.hovered() || keep {
            let area = egui::Area::new(fly)
                .order(egui::Order::Tooltip)
                .fixed_pos(pos2(fly_x, mr.rect.top() - 4.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_width(150.0);
                        for (label, secs) in [
                            ("For 1 hour", 3600),
                            ("For 4 hours", 4 * 3600),
                            ("Until tomorrow", 24 * 3600),
                            ("Unmute", 0),
                        ] {
                            let shown = if label == "Unmute" {
                                crate::i18n::tr_ctx("Unmute (notifications)", label)
                            } else {
                                tr(label)
                            };
                            if ui
                                .add(ghost(egui::Button::new(shown).min_size(vec2(140.0, 26.0))))
                                .clicked()
                            {
                                self.data.extra.muted_until =
                                    if secs == 0 { 0 } else { now_ts() + secs };
                                self.dirty = true;
                                self.toast(if secs == 0 {
                                    tr("Notifications on").into()
                                } else {
                                    crate::trf!("Notifications muted {}", tr(label).to_lowercase())
                                });
                                close_popup(ui);
                            }
                        }
                    });
                });
            let r = area.response.rect;
            ui.data_mut(|d| d.insert_temp(fly.with("w"), r.width()));
            over = r
                .expand(4.0)
                .contains(ui.ctx().pointer_hover_pos().unwrap_or_default());
        }
        // the strip the pointer crosses between the row and the flyout
        let (gx0, gx1) = if on_right {
            (mr.rect.right() - 10.0, fly_x + 12.0)
        } else {
            (fly_x + fly_w - 12.0, mr.rect.left() + 10.0)
        };
        let gap = ui.ctx().pointer_hover_pos().map_or(false, |p| {
            p.y >= mr.rect.top() - 4.0
                && p.y <= mr.rect.bottom() + 120.0
                && p.x >= gx0
                && p.x <= gx1
        });
        ui.data_mut(|d| d.insert_temp(fly, mr.hovered() || over || (keep && gap)));
        ui.separator();
        let rows: [(&str, &str); 8] = [
            ("settings", "Settings"),
            ("bell", "Notifications"),
            ("palette", "Themes"),
            ("sparkle", "Labs"),
            ("keyboard", "Keyboard shortcuts"),
            ("download", "Download app"),
            ("refresh", "Check for updates"),
            ("help", "Help"),
        ];
        for (icon, label) in rows {
            let resp = menu_item(ui, &t, icon, tr(label), "");
            if label == "Check for updates" && crate::updater::update_available() {
                // a newer version is waiting on the team's server
                ui.painter().circle_filled(
                    resp.rect.right_center() - vec2(14.0, 0.0),
                    4.0,
                    t.danger,
                );
            }
            if resp.clicked() {
                if label == "Settings" {
                    self.open_settings("My settings");
                    close_popup(ui);
                    continue;
                }
                if label == "Check for updates" {
                    // Sparkle shows its own window; only explain when it can't run
                    match crate::updater::check_now() {
                        Ok(crate::updater::Shown::BySparkle) => {}
                        Ok(crate::updater::Shown::ByApp) => self.modal = Some(Modal::Update),
                        Err(why) => {
                            self.modal = Some(Modal::Unavailable {
                                title: "Check for updates".into(),
                                body: why.explain(),
                            })
                        }
                    }
                    close_popup(ui);
                    continue;
                }
                self.modal = Some(match label {
                    "Settings" => {
                        let m = self.data.member(me).cloned().unwrap_or_default();
                        Modal::profile(&m)
                    }
                    "Notifications" => Modal::Notifications,
                    "Themes" => Modal::Themes,
                    "Labs" => Modal::Labs,
                    "Keyboard shortcuts" => Modal::Shortcuts,
                    "Download app" => Modal::Unavailable {
                        title: "Download".into(),
                        body: tr("You're already using the desktop app on this computer.").into(),
                    },
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
                // room for the 28 pt pin and the gap before it
                let w = ui.available_width() - 28.0 - ui.spacing().item_spacing.x;
                let (r, resp) = ui.allocate_exact_size(vec2(w, 27.0), Sense::click());
                name_it(&resp, label);
                if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                line_icon(
                    ui.painter(),
                    icon,
                    r.left_center() + vec2(16.0, 0.0),
                    t.text2,
                );
                ui.painter().text(
                    r.left_center() + vec2(32.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr(label),
                    FontId::proportional(14.0),
                    t.text,
                );
                if resp.clicked() {
                    self.run_tool(id);
                    close_popup(ui);
                }
                // the pin: dark when pinned, the faint text grey otherwise
                // (it was the border grey and could hardly be seen)
                let pin = icon_button_ex(
                    ui,
                    &t,
                    "pin",
                    "",
                    "Pin",
                    false,
                    0.0,
                    t.text,
                    if pinned { t.text } else { t.text3 },
                );
                if pin
                    .on_hover_text(if pinned {
                        tr("Unpin from the top bar")
                    } else {
                        tr("Pin to the top bar")
                    })
                    .clicked()
                {
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
        if menu_item(ui, &t, "trash", tr("Trash"), "").clicked() {
            self.modal = Some(Modal::Trash);
            close_popup(ui);
        }
        // it changes who uses the app on this computer without signing out;
        // on a team server the person is who signed in, so it isn't there
        if !self.on_team_server()
            && menu_item(ui, &t, "person", tr("Switch person"), "")
                .on_hover_text(tr("Switch to another person on this computer"))
                .clicked()
        {
            self.modal = Some(Modal::SwitchUser);
            close_popup(ui);
        }
        // signs this computer out of the team server (login.rs)
        if self.on_team_server()
            && menu_item(ui, &t, "logout", tr("Log out"), "")
                .on_hover_text(tr("Sign this computer out of the team server"))
                .clicked()
        {
            self.modal = Some(Modal::LogOut);
            close_popup(ui);
        }
    }

    pub(super) fn run_tool(&mut self, id: &str) {
        match id {
            "create_task" => self.open_quick_add(None),
            "my_work" => self.actions.push(Action::Go(Location::Home)),
            "track_time" => {
                let first = self
                    .data
                    .tasks
                    .iter()
                    .find(|x| x.assignees.contains(&self.data.me))
                    .or(self.data.tasks.first())
                    .map(|x| x.id)
                    .unwrap_or(0);
                self.modal = Some(Modal::TrackTime { task: first });
            }
            "notepad" => self.notepad_open = !self.notepad_open,
            "clip" => self.modal = Some(unavailable("Record a Clip")),
            "stt" => self.modal = Some(unavailable("Talk to Text")),
            "reminder" => self.modal = Some(Modal::reminder(String::new(), today())),
            "doc" => self.new_doc(None),
            "whiteboard" => self.new_whiteboard(),
            "people" => self.section = Section::People,
            "dashboard" => {
                self.modal = Some(Modal::NewDashboard {
                    name: tr("New Dashboard").into(),
                })
            }
            _ => self.modal = Some(unavailable("AI Notetaker")),
        }
    }

    pub(super) fn new_doc(&mut self, space: Option<u64>) {
        let id = self.data.new_id();
        let now = now_ts();
        self.data.docs.insert(
            0,
            Doc {
                id,
                space,
                owner: self.data.me,
                private: false,
                archived: false,
                meeting_note: false,
                wiki: None,
                views: vec![],
                parent: None,
                title: tr("Untitled").into(),
                body: String::new(),
                created: now,
                updated: now,
                tags: vec![],
            },
        );
        self.doc_open = Some(id);
        self.section = Section::Docs;
        self.dirty = true;
    }

    pub(super) fn new_whiteboard(&mut self) {
        let id = self.data.new_id();
        let n = self.data.extra.whiteboards.len() + 1;
        self.data.extra.whiteboards.push(Whiteboard {
            id,
            name: crate::trf!("Whiteboard {}", n),
            strokes: vec![],
            notes: vec![],
            owner: self.data.me,
        });
        self.wb_open = Some(id);
        self.section = Section::Whiteboards;
        self.dirty = true;
    }

    pub(super) fn new_form(&mut self) {
        let id = self.data.new_id();
        let list = self.default_list().unwrap_or(0);
        self.data.extra.forms.push(Form {
            id,
            name: tr("New Form").into(),
            list_id: list,
            description: tr("Fill out this form to create a task.").into(),
            fields: vec![
                FormField {
                    label: tr("Title").into(),
                    map: "name".into(),
                    ..Default::default()
                },
                FormField {
                    label: tr("Details").into(),
                    map: "description".into(),
                    ..Default::default()
                },
            ],
            owner: self.data.me,
        });
        self.form_open = Some(id);
        self.section = Section::Forms;
        self.dirty = true;
    }

    /// The sidebar "+ Create" menu, in the original's order.
    pub(super) fn create_menu(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.set_width(400.0);
        let r = ui.add(
            field(&mut self.create_text)
                .hint_text(tr("Describe anything to create"))
                .desired_width(f32::INFINITY),
        );
        if r.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter))
            && !self.create_text.trim().is_empty()
        {
            if let Some(list) = self.default_list() {
                self.actions.push(Action::Add {
                    list,
                    name: self.create_text.clone(),
                    status: None,
                    priority: 0,
                    assignees: vec![self.data.me],
                    due: None,
                    parent: None,
                    open: true,
                });
                self.create_text.clear();
            } else {
                // the text stays here for after the List is made
                self.need_a_list();
            }
            close_popup(ui);
        }
        ui.separator();
        section_label(ui, &t, tr("Create"));
        if menu_item(ui, &t, "task", tr("Task"), "\u{2325}T").clicked() {
            self.open_quick_add(None);
            close_popup(ui);
        }
        if menu_item(ui, &t, "send", tr("Message"), "\u{2318}G").clicked() {
            self.new_message_open = true;
            close_popup(ui);
        }
        ui.separator();
        let space = self
            .current_space()
            .or_else(|| self.data.spaces.first().map(|s| s.id));
        if menu_item2(
            ui,
            &t,
            "list",
            tr("List"),
            tr("Track tasks, projects, people & more"),
        )
        .clicked()
        {
            // no Space to put it in: say so and make the Space first
            if self.open_spaces().is_empty() {
                self.toast(tr(
                    "Lists live in a Space. Make a Space first: it comes with a List.",
                ));
            }
            self.offer_new_list(space);
            close_popup(ui);
        }
        if menu_item2(
            ui,
            &t,
            "hash",
            tr("Channel"),
            tr("Conversations on specific topics"),
        )
        .clicked()
        {
            self.modal = Some(Modal::NewChannel {
                name: String::new(),
                private: false,
                add_list: false,
            });
            close_popup(ui);
        }
        if menu_item2(
            ui,
            &t,
            "space",
            tr("Space"),
            tr("Organize work by team or department"),
        )
        .clicked()
        {
            let color = STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13];
            self.modal = Some(Modal::NewSpace {
                name: String::new(),
                color,
            });
            close_popup(ui);
        }
        ui.separator();
        if menu_item(ui, &t, "sparkle", tr("Create with AI"), "").clicked() {
            self.modal = Some(unavailable("Create with AI"));
            close_popup(ui);
        }
        if menu_item(ui, &t, "robot", tr("Super Agent"), tr("Hot")).clicked() {
            self.modal = Some(unavailable("Super Agent"));
            close_popup(ui);
        }
        ui.separator();
        for (icon, label) in [
            ("docs", "Doc"),
            ("heart", "Artifact"),
            ("forms", "Form"),
            ("dashboards", "Dashboard"),
            ("whiteboards", "Whiteboard"),
            ("video", "Clip"),
        ] {
            if menu_item(ui, &t, icon, tr(label), "").clicked() {
                match label {
                    "Doc" => self.new_doc(space),
                    "Form" => self.new_form(),
                    "Dashboard" => {
                        self.modal = Some(Modal::NewDashboard {
                            name: tr("New Dashboard").into(),
                        })
                    }
                    "Whiteboard" => self.new_whiteboard(),
                    "Artifact" => self.modal = Some(unavailable("Artifacts")),
                    _ => self.modal = Some(unavailable("Clips")),
                }
                close_popup(ui);
            }
        }
        ui.separator();
        if menu_item(ui, &t, "sidebar", tr("Customize your sidebar"), "").clicked() {
            self.modal = Some(Modal::CustomizeSidebar);
            close_popup(ui);
        }
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 8.0) / 2.0;
            if icon_button_ex(
                ui,
                &t,
                "download",
                tr("Import"),
                "Import",
                true,
                w,
                t.text,
                t.text2,
            )
            .clicked()
            {
                self.modal = Some(Modal::Import {
                    path: String::new(),
                    result: String::new(),
                });
                close_popup(ui);
            }
            if icon_button_ex(
                ui,
                &t,
                "templates",
                tr("Templates"),
                "Templates",
                true,
                w,
                t.text,
                t.text2,
            )
            .clicked()
            {
                self.open_templates(super::templates::Category::All, None);
                close_popup(ui);
            }
        });
    }

    pub(super) fn current_space(&self) -> Option<u64> {
        match self.loc {
            Location::Space(id) => Some(id),
            Location::List(id) => self.data.space_of_list(id).map(|s| s.id),
            Location::Folder(fid) => self
                .data
                .spaces
                .iter()
                .find(|s| s.folders.iter().any(|f| f.id == fid))
                .map(|s| s.id),
            _ => None,
        }
    }

    /// The rail's More menu: Spaces, Chat, Docs, Dashboards, Whiteboards,
    /// Forms, Clips, Goals, Timesheets, Apps, Customize navigation.
    pub(super) fn rail_more_menu(&mut self, ui: &mut Ui) {
        let t = self.t();
        ui.set_width(220.0);
        let items: [(&str, &str); 10] = [
            ("space", "Spaces"),
            ("chat", "Chat"),
            ("docs", "Docs"),
            ("dashboards", "Dashboards"),
            ("whiteboards", "Whiteboards"),
            ("forms", "Forms"),
            ("video", "Clips"),
            ("goals", "Goals"),
            ("timesheets", "Timesheets"),
            ("apps", "Apps"),
        ];
        for (icon, label) in items {
            let pinned = self.data.extra.rail_items.iter().any(|r| r == label);
            ui.horizontal(|ui| {
                let w = ui.available_width() - 28.0 - ui.spacing().item_spacing.x;
                let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                name_it(&resp, label);
                if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                line_icon(
                    ui.painter(),
                    icon,
                    r.left_center() + vec2(16.0, 0.0),
                    t.text2,
                );
                ui.painter().text(
                    r.left_center() + vec2(32.0, 0.0),
                    Align2::LEFT_CENTER,
                    tr(label),
                    FontId::proportional(14.0),
                    t.text,
                );
                if resp.clicked() {
                    self.open_app(label);
                    close_popup(ui);
                }
                if icon_button_ex(
                    ui,
                    &t,
                    "pin",
                    "",
                    "Pin",
                    false,
                    0.0,
                    t.text,
                    if pinned { t.text } else { t.text3 },
                )
                .on_hover_text(tr("Pin to the left bar"))
                .clicked()
                {
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
        if menu_item(ui, &t, "settings", tr("Customize navigation"), "").clicked() {
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
        if !matches!(self.modal, Some(Modal::Reminder { .. })) {
            self.reminder_about = (None, None);
        }
        let Some(mut modal) = self.modal.take() else {
            return;
        };
        let t = self.t();
        let mut open = true;
        let mut close = false;
        let title = match &modal {
            Modal::Invite { .. } => tr("Invite people").to_string(),
            Modal::WorkspaceSettings { .. } => tr("Workspace settings").into(),
            Modal::NewWorkspace { .. } => tr("Create Workspace").into(),
            Modal::Profile { .. } => tr("Edit profile").into(),
            Modal::SetStatus { .. } => tr("Set status").into(),
            Modal::Themes => tr("Themes").into(),
            Modal::Shortcuts => tr("Keyboard shortcuts").into(),
            Modal::Help => tr("Help").into(),
            Modal::Labs => tr("Labs").into(),
            Modal::Notifications => tr("Notifications").into(),
            Modal::ClickApps => tr("Apps").into(),
            Modal::CustomFields { .. } => tr("Custom fields").into(),
            Modal::Automations { .. } => tr("Automations").into(),
            Modal::TagManager { .. } => tr("Tag manager").into(),
            Modal::TemplateCenter(_) => tr("Template Center").into(),
            Modal::Import { .. } => tr("Import and export").into(),
            Modal::ChannelSettings(_) => tr("Channel settings").into(),
            Modal::SavedMessages => tr("Saved messages").into(),
            Modal::Share { .. } => tr("Share").into(),
            Modal::CustomizeNav => tr("Customize navigation").into(),
            Modal::CustomizeSidebar => tr("Customize your sidebar").into(),
            Modal::TrackTime { .. } => tr("Track Time").into(),
            Modal::Reminder { .. } => tr("Create Reminder").into(),
            Modal::Trash => tr("Trash").into(),
            Modal::Unavailable { title, .. } => tr(title).to_string(),
            Modal::Update => tr("Check for updates").into(),
            Modal::SlackImport(_) => tr("Import from Slack").into(),
            Modal::Bookmark { .. } => tr("Add bookmark").into(),
            Modal::SwitchUser => tr("Switch person").into(),
            Modal::NewDashboard { .. } => tr("Create Dashboard").into(),
            Modal::Together { .. } => tr("Work together").into(),
            Modal::WhoAmI { .. } => tr("Who are you?").into(),
            Modal::JoinTeam { .. } => tr("Join a team").into(),
            Modal::LogOut => tr("Log out").into(),
            Modal::DeleteWorkspace { .. } => tr("Delete workspace").into(),
            Modal::Call { .. } => tr("Start a call").into(),
            Modal::NewList { .. } => tr("Create List").into(),
            Modal::NewSpace { .. } => tr("Create a Space").into(),
            Modal::MoveTasks { .. } => tr("Move to").into(),
            Modal::MoveList { .. } => tr("Move List").into(),
            Modal::SpaceSharing { .. } => tr("Share & permissions").into(),
            Modal::NewChannel { .. } => tr("Create Channel").into(),
            Modal::ListInfo { .. } => tr("List Info").into(),
            Modal::PickTask { mode, .. } => tr(mode.title()).into(),
            Modal::ConfirmRemove { kind, .. } => if *kind == "person" {
                tr("Remove")
            } else if *kind == "invite" {
                tr("Cancel invite")
            } else {
                tr("Delete")
            }
            .into(),
        };
        // dim backdrop
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("modal_dim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let r = ui.allocate_rect(screen, Sense::click());
                crate::widgets::no_pointer(&r);
                // the dark theme needs its stronger overlay to set the dialog apart
                let dim = if t.dark {
                    t.overlay
                } else {
                    Color32::from_black_alpha(90)
                };
                ui.painter().rect_filled(screen, 0.0, dim);
                if r.clicked() {
                    close = true;
                }
            });
        // each kind of dialog sizes itself: a wide one doesn't leave the next wide
        let win_id = modal_window_id(&modal);
        // the Template Center has a list beside its preview
        let wide = matches!(modal, Modal::TemplateCenter(_));
        let (min_w, max_w) = if wide { (800.0, 820.0) } else { (460.0, 620.0) };
        egui::Window::new(RichText::new(&title).size(17.0))
            .id(win_id)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, -20.0])
            .min_width(min_w)
            .max_width(max_w)
            .frame(dialog_frame(&t))
            .show(ctx, |ui| {
                ui.set_min_width(min_w - 20.0);
                if dialog_header(ui, &t, &title) {
                    open = false;
                }
                ui.add_space(10.0);
                // the server and sign-in dialogs can be taller than a small
                // window: the title stays put and the rest scrolls (the
                // others are short, or scroll their own lists)
                if matches!(modal, Modal::Together { .. } | Modal::JoinTeam { .. }) {
                    let tall = (screen.height() - 140.0).max(200.0);
                    egui::ScrollArea::vertical()
                        .id_salt("modal_body")
                        .max_height(tall)
                        .min_scrolled_height(tall)
                        .show(ui, |ui| {
                            ui.set_min_width(min_w - 20.0);
                            close |= self.modal_body(ui, &t, &mut modal);
                        });
                } else {
                    close |= self.modal_body(ui, &t, &mut modal);
                }
            });
        ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, win_id));
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.menu_was_open {
            close = true;
        }
        if open && !close && self.modal.is_none() {
            self.modal = Some(modal);
        } else if matches!(modal, Modal::Profile { .. }) {
            // a photo chosen in Edit profile and not saved is dropped
            self.profile_photo = None;
        }
    }

    /// Returns true when the dialog should close.
    pub(super) fn modal_body(&mut self, ui: &mut Ui, t: &Theme, modal: &mut Modal) -> bool {
        let mut close = false;
        match modal {
            Modal::Together {
                path,
                addr,
                code,
                result,
            } => close = self.together_body(ui, t, path, addr, code, result),
            Modal::WhoAmI { name, email } => close = self.who_am_i_body(ui, t, name, email),
            Modal::JoinTeam { addr } => close = self.join_team_body(ui, t, addr),
            Modal::LogOut => close = self.log_out_body(ui, t),
            Modal::DeleteWorkspace { typed } => close = self.delete_workspace_body(ui, t, typed),
            Modal::Call { ch, link } => close = self.call_body(ui, t, ch, link),
            Modal::NewList {
                name,
                space,
                folder,
            } => close = self.new_list_body(ui, t, name, space, folder),
            Modal::NewSpace { name, color } => close = self.new_space_body(ui, t, name, color),
            Modal::NewChannel {
                name,
                private,
                add_list,
            } => close = self.new_channel_body(ui, t, name, private, add_list),
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
            Modal::SpaceSharing { space, draft } => {
                let s = *space;
                close = self.sharing_body(ui, t, s, draft);
            }
            Modal::PickTask { task, mode, query } => {
                let (tid, m) = (*task, *mode);
                close = self.pick_task_body(ui, t, tid, m, query);
            }
            Modal::Invite { .. }
            | Modal::ClickApps
            | Modal::CustomFields { .. }
            | Modal::Automations { .. }
            | Modal::TagManager { .. }
                if !self.may_manage() =>
            {
                // reached by a link or a stale screen: on a server these are
                // the owner's and admins' (the server keeps them too)
                ui.label(tr(if matches!(modal, Modal::Invite { .. }) {
                    "Only the owner or an admin of this workspace can invite people."
                } else {
                    "Only the owner or an admin of this workspace can change this."
                }));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(tr("Close")).clicked() {
                            close = true;
                        }
                    });
                });
            }
            Modal::Invite { emails, role } => close = self.invite_body(ui, t, emails, role),
            Modal::WorkspaceSettings { name } => close = self.workspace_settings_body(ui, t, name),
            Modal::NewWorkspace {
                step,
                name,
                apps,
                emails,
                role,
                on_server,
            } => close = self.new_workspace_body(ui, t, step, name, apps, emails, role, on_server),
            Modal::Profile {
                name,
                display,
                title,
                email,
                photo,
            } => close = self.profile_body(ui, t, name, display, title, email, photo),
            Modal::SetStatus { text } => close = self.set_status_body(ui, t, text),
            Modal::Themes => close = self.themes_body(ui, t),
            Modal::Shortcuts => close = self.shortcuts_body(ui, t),
            Modal::Help => close = self.help_body(ui, t),
            Modal::Labs => close = self.labs_body(ui, t),
            Modal::Notifications => close = self.notifications_body(ui, t),
            Modal::ClickApps => close = self.click_apps_body(ui, t),
            Modal::CustomFields { name, kind, opts } => {
                close = self.custom_fields_body(ui, t, name, kind, opts)
            }
            Modal::Automations {
                status,
                action,
                value,
                scope,
            } => close = self.automations_body(ui, t, status, action, value, scope),
            Modal::TagManager { from, to } => close = self.tag_manager_body(ui, t, from, to),
            Modal::TemplateCenter(c) => close = self.template_center_body(ui, t, c),
            Modal::Import { path, result } => close = self.import_body(ui, t, path, result),
            Modal::ChannelSettings(cid) => close = self.channel_settings_body(ui, t, cid),
            Modal::Share { emails } => close = self.share_body(ui, t, emails),
            Modal::CustomizeNav => close = self.customize_nav_body(ui, t),
            Modal::CustomizeSidebar => close = self.customize_sidebar_body(ui, t),
            Modal::TrackTime { task } => close = self.track_time_body(ui, t, task),
            Modal::Reminder { text, date, time } => {
                close = self.reminder_body(ui, t, text, date, time)
            }
            Modal::SavedMessages => {
                close = self.saved_messages(ui);
            }
            Modal::Trash => close = self.trash_body(ui, t),
            Modal::SlackImport(plan) => {
                if self.slack_modal(ui, t, plan) {
                    close = true;
                }
            }
            Modal::Unavailable { body, .. } => {
                ui.label(tr(body.as_str()));
                ui.add_space(8.0);
                if primary_button(ui, t, tr("OK")).clicked() {
                    close = true;
                }
            }
            Modal::Update => close = crate::updater::status_ui(ui, t, true),
            Modal::Bookmark { title, url } => close = self.bookmark_body(ui, t, title, url),
            Modal::SwitchUser if self.on_server() => close = self.switch_user_on_server_body(ui, t),
            Modal::SwitchUser => close = self.switch_user_body(ui, t),
            Modal::ConfirmRemove { kind, id, name } => {
                close = self.confirm_remove_body(ui, t, kind, *id, name)
            }
            Modal::NewDashboard { name } => close = self.new_dashboard_body(ui, t, name),
        }
        close
    }

    /// Adds invited people (skipping emails already in the Workspace); they join #general.
    /// Returns how many were added.
    /// Makes a workspace on this computer owned by me, opens it, pins
    /// `apps` to its left bar and invites `invite` there.
    pub(super) fn create_workspace(
        &mut self,
        name: &str,
        apps: &[String],
        invite: &[String],
        role: &str,
    ) {
        let mut list = Data::workspaces();
        if list.is_empty() {
            list.push(WsEntry {
                name: self.data.workspace.clone(),
                file: self.ws_file.clone(),
            });
        }
        let file = format!("workspace-{}.json", now_ts());
        let mut owner = self.data.member(self.data.me).cloned().unwrap_or_default();
        owner.id = 1;
        owner.role = "Owner".into();
        let mut d = Data::blank(name, owner);
        d.extra.rail_items = apps.to_vec();
        d.save_file(&file);
        list.push(WsEntry {
            name: name.to_string(),
            file: file.clone(),
        });
        Data::save_workspaces(&list);
        self.switch_workspace(&file);
        // the invites belong to the new one: none if it didn't open
        let invited = if self.ws_file == file {
            self.invite_people(invite, role)
        } else {
            0
        };
        if invited > 0 {
            self.toast(crate::trf!(
                "Invited {} people. Share the workspace (Work together) so they can join.",
                invited
            ));
        }
    }

    pub(super) fn invite_people(&mut self, emails: &[String], role: &str) -> usize {
        let mut added = 0;
        for e in emails {
            let e = e.trim();
            if !(e.contains('@') && e.contains('.'))
                || self
                    .data
                    .members
                    .iter()
                    .any(|m| m.email.eq_ignore_ascii_case(e))
            {
                continue;
            }
            let id = self.data.new_id();
            let name = e.split('@').next().unwrap_or(e).replace(['.', '_'], " ");
            let name = name
                .split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" ");
            let color = STATUS_PALETTE[1 + (id as usize % 13)];
            self.data.members.push(Member {
                id,
                name,
                color,
                email: e.to_string(),
                role: role.to_string(),
                invited: true,
                status_text: String::new(),
                slack: vec![],
                slack_mine: vec![],
                ..Default::default()
            });
            // on a server (or going up to one): ask for their invite code
            // (emailed by the server) once it has them
            let setting_up = self
                .sync_rt
                .joining
                .as_ref()
                .is_some_and(|j| matches!(j.how, super::together::How::Claim { .. }));
            if setting_up || self.on_server() {
                self.sync_rt.auto_invite.push((
                    id,
                    if role == "Admin" {
                        wb_net::Role::Admin
                    } else {
                        wb_net::Role::Member
                    },
                ));
            }
            if let Some(g) = self
                .data
                .channels
                .iter_mut()
                .find(|c| c.kind == ChannelKind::Channel && c.name == "general")
            {
                g.members.push(id);
            }
            added += 1;
        }
        if added > 0 {
            self.dirty = true;
        }
        added
    }

    /// The List or Space an automation runs in, by id.
    fn scope_name(&self, id: u64) -> String {
        match self.data.spaces.iter().find(|s| s.id == id) {
            Some(s) => s.name.clone(),
            None => self.data.list_name(id),
        }
    }

    pub(super) fn describe_action(&self, a: &Automation) -> String {
        match a.action.as_str() {
            "assign" => format!(
                "assign to {}",
                a.value
                    .parse::<u64>()
                    .ok()
                    .and_then(|id| self.data.member(id))
                    .map(|m| m.shown().to_string())
                    .unwrap_or_default()
            ),
            "priority" => format!(
                "set priority to {}",
                a.value
                    .parse::<usize>()
                    .ok()
                    .filter(|p| *p < 5)
                    .map(|p| PRIORITIES[p].label)
                    .unwrap_or("")
            ),
            "move" => format!(
                "move to {}",
                a.value
                    .parse::<u64>()
                    .ok()
                    .map(|id| self.data.list_name(id))
                    .unwrap_or_default()
            ),
            _ => format!("comment \"{}\"", a.value),
        }
    }

    /// `describe_action` in the interface language (the English one goes into the activity log).
    fn action_text(&self, a: &Automation) -> String {
        match a.action.as_str() {
            "assign" => crate::trf!(
                "assign to {}",
                a.value
                    .parse::<u64>()
                    .ok()
                    .and_then(|id| self.data.member(id))
                    .map(|m| m.shown().to_string())
                    .unwrap_or_default()
            ),
            "priority" => crate::trf!(
                "set priority to {}",
                a.value
                    .parse::<usize>()
                    .ok()
                    .filter(|p| *p < 5)
                    .map(|p| tr(PRIORITIES[p].label))
                    .unwrap_or("")
            ),
            "move" => crate::trf!(
                "move to {}",
                a.value
                    .parse::<u64>()
                    .ok()
                    .map(|id| self.data.list_name(id))
                    .unwrap_or_default()
            ),
            _ => crate::trf!("comment \"{}\"", a.value),
        }
    }

    pub(super) fn import_csv(&mut self, path: &str) -> String {
        let Ok(text) = std::fs::read_to_string(path) else {
            return crate::trf!("Couldn't read {}", path);
        };
        let Some(list) = self.default_list() else {
            return tr("Open a list first").into();
        };
        let statuses = self.data.statuses_for_list(list);
        let mut n = 0;
        for (i, cols) in parse_csv(&text).into_iter().enumerate() {
            if cols.is_empty() || cols[0].is_empty() {
                continue;
            }
            if i == 0 && cols[0].eq_ignore_ascii_case("name") {
                continue;
            }
            let status = cols
                .get(1)
                .and_then(|s| statuses.iter().find(|st| st.name.eq_ignore_ascii_case(s)))
                .map(|s| s.name.clone());
            let priority = cols
                .get(2)
                .and_then(|p| {
                    PRIORITIES
                        .iter()
                        .position(|d| d.label.eq_ignore_ascii_case(p))
                })
                .unwrap_or(0) as u8;
            let due = cols
                .get(3)
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
            let assignees = cols
                .get(4)
                .and_then(|a| {
                    self.data
                        .members
                        .iter()
                        .find(|m| m.any_name(|n| n.eq_ignore_ascii_case(a)))
                })
                .map(|m| vec![m.id])
                .unwrap_or_default();
            self.actions.push(Action::Add {
                list,
                name: cols[0].clone(),
                status,
                priority,
                assignees,
                due,
                parent: None,
                open: false,
            });
            n += 1;
        }
        crate::trf!("Imported {} tasks into {}", n, self.data.list_name(list))
    }

    /// The current tasks as CSV text (every field quoted, so names keep their
    /// commas, quotes and line breaks; `import_csv` reads it back).
    pub(super) fn csv_text(&self) -> String {
        let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut out = String::from("name,status,priority,due,assignee,list\n");
        for t in self.filtered() {
            let who = t
                .assignees
                .first()
                .and_then(|a| self.data.member(*a))
                .map(|m| m.shown().to_string())
                .unwrap_or_default();
            let row = [
                t.name.clone(),
                t.status.clone(),
                PRIORITIES[t.priority as usize].label.to_string(),
                t.due.map(|d| d.to_string()).unwrap_or_default(),
                who,
                self.data.list_name(t.list_id),
            ];
            out.push_str(&row.iter().map(|f| q(f)).collect::<Vec<_>>().join(","));
            out.push('\n');
        }
        out
    }

    pub(super) fn export_csv(&self) -> String {
        let out = self.csv_text();
        let path = crate::os::home_dir()
            .join("Downloads")
            .join(format!("tasks-{}.csv", today()));
        match std::fs::write(&path, out) {
            Ok(_) => crate::trf!("Saved {}", path.display()),
            Err(e) => crate::trf!("Couldn't save: {}", e),
        }
    }

    pub(super) fn restore_trash(&mut self, i: usize) {
        let item = self.data.extra.trash.remove(i);
        // the server may have cleaned up files only the trash still named
        // (Trash stays on this computer): put this computer's copies back
        self.reupload_files(&super::files::trash_attachments(&item.payload));
        match item.kind.as_str() {
            "Task" => {
                if let Ok(mut tasks) = serde_json::from_str::<Vec<Task>>(&item.payload) {
                    // the list it came from may be gone: bring the list back
                    // from Trash too, or else put the task in the first list
                    let mut gone: Vec<u64> = tasks
                        .iter()
                        .map(|t| t.list_id)
                        .filter(|l| !self.data.location_exists(Location::List(*l)))
                        .collect();
                    gone.sort_unstable();
                    gone.dedup();
                    let mut note = None;
                    for l in gone {
                        if self.data.location_exists(Location::List(l)) {
                            continue;
                        }
                        let in_trash = self.data.extra.trash.iter().position(|x| {
                            x.kind == "List"
                                && ListTrash::parse(&x.payload).is_some_and(|lt| lt.list.id == l)
                        });
                        if let Some(j) = in_trash {
                            let name = self.data.extra.trash[j].title.clone();
                            self.restore_trash(j);
                            if self.data.location_exists(Location::List(l)) {
                                note = Some(crate::trf!("Also restored its list \"{}\"", name));
                                continue;
                            }
                        }
                        let Some(to) = self.data.first_list() else {
                            self.data
                                .extra
                                .trash
                                .insert(i.min(self.data.extra.trash.len()), item);
                            self.toast(tr(
                                "There is no list to restore it into. Make a list first.",
                            ));
                            return;
                        };
                        for t in tasks.iter_mut().filter(|t| t.list_id == l) {
                            t.list_id = to;
                        }
                        if let Some(si) = self.data.spaces.iter().position(|s| {
                            s.lists
                                .iter()
                                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
                                .any(|x| x.id == to)
                        }) {
                            let moved: Vec<&mut Task> =
                                tasks.iter_mut().filter(|t| t.list_id == to).collect();
                            self.fit_statuses(moved, &[], si);
                        }
                        note = Some(crate::trf!(
                            "Its list is gone, so it went to {}",
                            self.data.list_name(to)
                        ));
                    }
                    self.data.tasks.extend(tasks);
                    if let Some(n) = note {
                        self.toast(n);
                    }
                }
            }
            "Doc" => {
                // a doc with its sub pages (older items hold a single page)
                let pages = serde_json::from_str::<Vec<Doc>>(&item.payload)
                    .or_else(|_| serde_json::from_str::<Doc>(&item.payload).map(|d| vec![d]))
                    .unwrap_or_default();
                self.restore_doc_pages(pages);
            }
            "List" => {
                if let Some(mut lt) = ListTrash::parse(&item.payload) {
                    // a List deleted while archived comes back to be seen, not
                    // archived again where only Settings > Spaces shows it
                    lt.list.archived = false;
                    let lid = lt.list.id;
                    // back into its folder, else its space, else the first space
                    let folder = lt.folder.and_then(|f| {
                        self.data
                            .spaces
                            .iter()
                            .position(|s| s.folders.iter().any(|x| x.id == f))
                            .map(|si| (si, f))
                    });
                    let space = self.data.spaces.iter().position(|s| s.id == lt.space);
                    let name = lt.list.name.clone();
                    let into;
                    if let Some((si, fid)) = folder {
                        if let Some(f) = self.data.spaces[si]
                            .folders
                            .iter_mut()
                            .find(|x| x.id == fid)
                        {
                            f.lists.push(lt.list);
                        }
                        into = si;
                    } else if let Some(si) = space.or_else(|| self.restore_space()) {
                        let home = self.data.spaces[si].name.clone();
                        self.data.spaces[si].lists.push(lt.list);
                        if lt.folder.is_some() || space.is_none() {
                            self.toast(crate::trf!(
                                "\"{}\" was restored to {} because its old place is gone",
                                name,
                                home
                            ));
                        }
                        into = si;
                    } else {
                        self.data
                            .extra
                            .trash
                            .insert(i.min(self.data.extra.trash.len()), item);
                        self.toast(tr(
                            "There is no space to restore it into. Make a space first.",
                        ));
                        return;
                    }
                    let mut tasks = lt.tasks;
                    if self.data.spaces[into].id != lt.space {
                        self.fit_statuses(tasks.iter_mut().collect(), &lt.statuses, into);
                    }
                    self.data.tasks.extend(tasks);
                    // its Folder or Space is archived: it is back, but hidden with them
                    if self.data.list_archived(lid) {
                        self.toast(crate::trf!(
                                "\"{}\" is back in an archived Folder or Space. Restore that from Settings > Spaces.",
                                name
                            ));
                    }
                }
            }
            "Space" => {
                if let Ok(st) = serde_json::from_str::<SpaceTrash>(&item.payload) {
                    if !self.data.spaces.iter().any(|s| s.id == st.space.id) {
                        self.data.spaces.push(st.space);
                    }
                    // its lists, while they are still in Trash
                    for tid in st.lists {
                        if let Some(j) = self.data.extra.trash.iter().position(|x| x.id == tid) {
                            self.restore_trash(j);
                        }
                    }
                }
            }
            "Folder" => {
                if let Ok(ft) = serde_json::from_str::<FolderTrash>(&item.payload) {
                    let space = self.data.spaces.iter().position(|s| s.id == ft.space);
                    let Some(si) = space.or_else(|| self.restore_space()) else {
                        self.data
                            .extra
                            .trash
                            .insert(i.min(self.data.extra.trash.len()), item);
                        self.toast(tr(
                            "There is no space to restore it into. Make a space first.",
                        ));
                        return;
                    };
                    if !self
                        .data
                        .spaces
                        .iter()
                        .any(|s| s.folders.iter().any(|f| f.id == ft.folder.id))
                    {
                        if space.is_none() {
                            self.toast(crate::trf!(
                                "\"{}\" was restored to {} because its old place is gone",
                                ft.folder.name,
                                self.data.spaces[si].name
                            ));
                        }
                        self.data.spaces[si].folders.push(ft.folder);
                    }
                    for tid in ft.lists {
                        if let Some(j) = self.data.extra.trash.iter().position(|x| x.id == tid) {
                            self.restore_trash(j);
                        }
                    }
                }
            }
            "Channel" => {
                // back for everyone, with its messages (unless it never left)
                if let Ok(c) = serde_json::from_str::<Channel>(&item.payload) {
                    if !self.data.channels.iter().any(|x| x.id == c.id) {
                        self.data.channels.push(c);
                    }
                }
            }
            "Whiteboard" => {
                if let Ok(w) = serde_json::from_str::<Whiteboard>(&item.payload) {
                    self.data.extra.whiteboards.push(w);
                }
            }
            "Form" => {
                if let Ok(f) = serde_json::from_str::<Form>(&item.payload) {
                    self.data.extra.forms.push(f);
                }
            }
            "Dashboard" => {
                if let Ok(d) = serde_json::from_str::<Dashboard>(&item.payload) {
                    self.data.extra.dashboards.push(d);
                }
            }
            "Goal" => {
                if let Ok(g) = serde_json::from_str::<Goal>(&item.payload) {
                    self.data.extra.goals.push(g);
                }
            }
            "Team" => {
                if let Ok(tm) = serde_json::from_str::<Team>(&item.payload) {
                    self.data.extra.teams.push(tm);
                }
            }
            _ => {}
        }
        self.dirty = true;
    }

    /// Where something restored from Trash goes when its own space is gone:
    /// the first team space in use, else my own Personal space, else any
    /// space that isn't someone else's.
    fn restore_space(&self) -> Option<usize> {
        let me = self.data.me;
        let sp = &self.data.spaces;
        sp.iter()
            .position(|s| !s.archived && s.private_owner.is_none())
            .or_else(|| {
                sp.iter()
                    .position(|s| !s.archived && s.private_owner == Some(me))
            })
            .or_else(|| {
                sp.iter()
                    .position(|s| s.private_owner.is_none_or(|o| o == me))
            })
    }

    /// Tasks coming back into another space (space `si`) take its status of
    /// the same name, else of the same kind (`from` holds the statuses they
    /// had), else its first one; each change is noted in the task's history.
    fn fit_statuses(&mut self, tasks: Vec<&mut Task>, from: &[Status], si: usize) {
        let Some(sp) = self.data.spaces.get(si) else {
            return;
        };
        let Some(first) = sp.statuses.first() else {
            return;
        };
        let me = self.data.me;
        for t in tasks {
            if sp.statuses.iter().any(|s| s.name == t.status) {
                continue;
            }
            let kind = from.iter().find(|s| s.name == t.status).map(|s| s.kind);
            let to = kind
                .and_then(|k| sp.statuses.iter().find(|s| s.kind == k))
                .unwrap_or(first);
            let old = std::mem::replace(&mut t.status, to.name.clone());
            Self::log(
                t,
                me,
                format!(
                    "changed status from {} to {}",
                    old.to_uppercase(),
                    to.name.to_uppercase()
                ),
            );
        }
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
        let at = ctx
            .data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("new_message_at"), || here));
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
                    let r = ui.add(
                        field(&mut q)
                            .hint_text(tr("People or Channels"))
                            .desired_width(f32::INFINITY),
                    );
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
                        for m in members.iter().filter(|m| !m.invited && m.any_name(&hit)) {
                            let label = if m.id == me {
                                crate::trf!("{} (You)", m.shown())
                            } else {
                                m.shown().to_string()
                            };
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(72.0, 64.0), Sense::click());
                            name_it(&resp, &format!("Message {}", m.shown()));
                            if resp.hovered() {
                                ui.painter().rect_filled(r, 8.0, t.hover);
                            }
                            paint_avatar(
                                ui.painter(),
                                r.center_top() + vec2(0.0, 22.0),
                                16.0,
                                m,
                                t.card,
                            );
                            let short = truncate(&label, 10);
                            ui.painter().text(
                                r.center_bottom() - vec2(0.0, 10.0),
                                Align2::CENTER_CENTER,
                                short,
                                FontId::proportional(11.0),
                                t.text2,
                            );
                            if resp.clicked() {
                                dm_with = Some(m.id);
                            }
                        }
                    });
                }
                if tab == 1 {
                    ui.add_space(4.0);
                    ui.label(RichText::new(tr("People")).small().color(t.text3));
                    for m in members.iter().filter(|m| m.any_name(&hit) || hit(&m.email)) {
                        let label = if m.id == me {
                            crate::trf!("{} \u{2014} You", m.shown())
                        } else {
                            m.shown().to_string()
                        };
                        let ring = t.card;
                        if menu_row(ui, &t, &label, false, |p, c| {
                            paint_avatar(p, c, 9.0, m, ring)
                        })
                        .clicked()
                        {
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
                        .filter(|c| {
                            (c.kind == ChannelKind::Channel || tab == 0)
                                && self.can_see(c)
                                && !self.dm_duplicate(c)
                        })
                        .map(|c| {
                            (
                                c.id,
                                self.channel_label(c),
                                c.messages.last().map(|m| m.ts).unwrap_or(c.last_read),
                            )
                        })
                        .filter(|c| hit(&c.1))
                        .collect();
                    if tab == 0 {
                        chans.sort_by(|a, b| b.2.cmp(&a.2));
                        chans.truncate(8);
                    }
                    ui.label(
                        RichText::new(if tab == 0 {
                            tr("Recents")
                        } else {
                            tr("Channels")
                        })
                        .small()
                        .color(t.text3),
                    );
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            for (id, label, _) in &chans {
                                if menu_row(ui, &t, label, false, |p, c| {
                                    p.text(
                                        c,
                                        Align2::CENTER_CENTER,
                                        if label.starts_with('#') { "#" } else { "@" },
                                        FontId::proportional(13.0),
                                        t.text2,
                                    );
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
            let existing = self
                .data
                .channels
                .iter()
                .find(|c| {
                    c.kind == ChannelKind::Direct
                        && ((uid == me && c.members == vec![me])
                            || (uid != me && c.members.contains(&uid) && c.members.contains(&me)))
                        && !self.dm_duplicate(c)
                })
                .map(|c| c.id);
            go = Some(existing.unwrap_or_else(|| {
                let id = self.data.new_id();
                let members = if uid == me { vec![me] } else { vec![me, uid] };
                self.data.channels.push(Channel {
                    id,
                    name: String::new(),
                    kind: ChannelKind::Direct,
                    topic: String::new(),
                    members,
                    messages: vec![],
                    last_read: now_ts(),
                    ..Default::default()
                });
                self.dirty = true;
                id
            }));
        }
        if let Some(id) = go {
            self.section = Section::Chat;
            self.chat.open = Some(id);
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
        egui::Window::new(tr("Notepad"))
            .id(egui::Id::new("Notepad"))
            .open(&mut open)
            .default_size([360.0, 420.0])
            // below the page header and toolbar it used to cover; no collapse
            // arrow (egui's own, larger than the app's arrows); it closes with x
            .default_pos([ctx.screen_rect().right() - 400.0, 170.0])
            .collapsible(false)
            .show(ctx, |ui| {
                // Write: the plain text; List: each line with its checkbox and a
                // button that turns it into a task ("- [ ] " lines are checklist items)
                let mode_id = egui::Id::new("notepad_list");
                let mut list: bool = ui.data(|d| d.get_temp(mode_id)).unwrap_or(false);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut list, false, tr("Write"));
                    ui.selectable_value(&mut list, true, tr("List"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(tr("+ Checklist item")).clicked() {
                            let n = &mut self.data.extra.notepad;
                            if !n.is_empty() && !n.ends_with('\n') {
                                n.push('\n');
                            }
                            n.push_str("- [ ] ");
                            list = false;
                            self.dirty = true;
                        }
                    });
                });
                ui.data_mut(|d| d.insert_temp(mode_id, list));
                if !list {
                    if ui
                        .add(
                            field_multi(&mut self.data.extra.notepad)
                                .hint_text(tr("Write anything. Only you can see this."))
                                .desired_width(f32::INFINITY)
                                .desired_rows(18),
                        )
                        .changed()
                    {
                        self.dirty = true;
                    }
                    return;
                }
                let lines: Vec<String> =
                    self.data.extra.notepad.lines().map(String::from).collect();
                let mut toggle = None;
                let mut make = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if lines.iter().all(|l| l.trim().is_empty()) {
                        ui.label(RichText::new(tr("Nothing written yet")).color(self.t().text3));
                    }
                    for (i, line) in lines.iter().enumerate() {
                        let (check, text) = notepad_line(line);
                        if text.is_empty() {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            if let Some(done) = check {
                                let mut on = done;
                                let cb = crate::widgets::check(ui, &mut on, "");
                                name_it(&cb, &format!("Check {}", text));
                                if cb.changed() {
                                    toggle = Some(i);
                                }
                            }
                            ui.label(if check == Some(true) {
                                RichText::new(text).strikethrough().color(self.t().text3)
                            } else {
                                RichText::new(text)
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let b = ui
                                    .add(ghost(egui::Button::new(
                                        RichText::new(format!("\u{2192} {}", tr("Task"))).small(),
                                    )))
                                    .on_hover_text(tr("Make a task from this line"));
                                name_it(&b, &format!("Make task {}", text));
                                if b.clicked() {
                                    make = Some(text.to_string());
                                }
                            });
                        });
                    }
                });
                if let Some(i) = toggle {
                    let mut v = lines.clone();
                    let l = &v[i];
                    v[i] = if let Some(rest) = l.trim_start().strip_prefix("- [ ] ") {
                        format!("- [x] {}", rest)
                    } else {
                        format!("- [ ] {}", notepad_line(l).1)
                    };
                    self.data.extra.notepad = v.join("\n");
                    self.dirty = true;
                }
                if let Some(name) = make {
                    if let Some(list) = self.default_list() {
                        let me = self.data.me;
                        self.actions.push(Action::Add {
                            list,
                            name,
                            status: None,
                            priority: 0,
                            assignees: vec![me],
                            due: None,
                            parent: None,
                            open: false,
                        });
                        self.toast(trf!("Task created in {}", self.data.list_name(list)));
                    } else {
                        self.need_a_list();
                    }
                }
            });
        if !open {
            self.notepad_open = false;
        }
    }

    /// A short message at the bottom of the window.
    pub(super) fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, ts)) = self.toast.clone() else {
            return;
        };
        if now_ts() - ts > 5 {
            self.toast = None;
            return;
        }
        // a notification's toast opens what it is about
        let target = self
            .notify_rt
            .toast_target
            .as_ref()
            .filter(|(text, _)| *text == msg)
            .map(|(_, target)| target.clone());
        let mut open = false;
        // The text keeps to one line while the window has room and wraps at
        // the window's width otherwise. Left to itself, the area offers only
        // its last frame's size, so a long message after a short one wrapped
        // every few letters.
        let font = egui::TextStyle::Body.resolve(&ctx.style());
        let one_line = ctx.fonts(|f| f.layout_no_wrap(msg.clone(), font, Color32::WHITE).size().x);
        let text_w = one_line.min((ctx.screen_rect().width() - 32.0 - 32.0).max(120.0));
        egui::Area::new(egui::Id::new("toast"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, [0.0, -24.0])
            .show(ctx, |ui| {
                let r = egui::Frame::new()
                    .fill(Color32::from_rgb(32, 32, 32))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.set_width(text_w.ceil() + 1.0);
                        ui.add(egui::Label::new(RichText::new(&msg).color(Color32::WHITE)).wrap());
                    });
                if target.is_some() {
                    let resp = ui
                        .interact(r.response.rect, ui.id().with("toast_open"), Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    hover_bg(ui, &resp, resp.rect, 10.0);
                    resp.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            true,
                            format!("Open {}", msg),
                        )
                    });
                    open = resp.clicked();
                }
            });
        if let (true, Some(target)) = (open, target) {
            self.toast = None;
            self.notify_rt.toast_target = None;
            self.open_target(&target);
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

/// A notepad line as (checklist state, text): "- [ ] a" is (Some(false), "a").
pub fn notepad_line(l: &str) -> (Option<bool>, &str) {
    let x = l.trim_start();
    if let Some(r) = x.strip_prefix("- [ ] ") {
        (Some(false), r.trim())
    } else if let Some(r) = x
        .strip_prefix("- [x] ")
        .or_else(|| x.strip_prefix("- [X] "))
    {
        (Some(true), r.trim())
    } else {
        (None, x.trim())
    }
}

/// Splits CSV text into rows of fields. A quoted field may hold commas, line
/// breaks and doubled quotes (""); unquoted fields are trimmed.
pub(super) fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = vec![];
    let mut row: Vec<String> = vec![];
    let mut field = String::new();
    let (mut quoted, mut was_quoted) = (false, false);
    let finish = |field: &mut String, was_quoted: &mut bool, row: &mut Vec<String>| {
        row.push(if *was_quoted {
            std::mem::take(field)
        } else {
            std::mem::take(field).trim().to_string()
        });
        *was_quoted = false;
    };
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if quoted {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    it.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            ',' => finish(&mut field, &mut was_quoted, &mut row),
            '\n' => {
                finish(&mut field, &mut was_quoted, &mut row);
                rows.push(std::mem::take(&mut row));
            }
            '\r' => {}
            '"' if !was_quoted && field.trim().is_empty() => {
                field.clear();
                quoted = true;
                was_quoted = true;
            }
            // anything after a closing quote, before the comma, is dropped
            _ if was_quoted => {}
            _ => field.push(c),
        }
    }
    if !field.is_empty() || was_quoted || !row.is_empty() {
        finish(&mut field, &mut was_quoted, &mut row);
        rows.push(row);
    }
    rows
}

/// The email addresses in what was typed, split at commas and spaces.
fn emails_in(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .map(|s| s.trim().to_string())
        .filter(|s| s.contains('@') && s.contains('.'))
        .collect()
}
