// Dialogs for how the app looks and behaves for me: themes, shortcuts,
// help, Labs, notifications, ClickApps and customizing the nav and sidebar.

use super::*;

impl App {
    /// The Themes dialog. Returns true when it should close.
    pub(super) fn themes_body(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        ui.label(tr("Appearance"));
        ui.horizontal(|ui| {
            let auto = self.data.extra.auto_theme;
            let sel = !auto && !self.data.dark;
            if icon_button_ex(
                ui,
                t,
                "sun",
                tr("Light"),
                "Light",
                sel,
                0.0,
                if sel { t.text } else { t.text2 },
                t.text2,
            )
            .clicked()
            {
                self.data.dark = false;
                self.data.extra.auto_theme = false;
                self.dirty = true;
            }
            let sel = !auto && self.data.dark;
            if icon_button_ex(
                ui,
                t,
                "moon",
                tr("Dark"),
                "Dark",
                sel,
                0.0,
                if sel { t.text } else { t.text2 },
                t.text2,
            )
            .clicked()
            {
                self.data.dark = true;
                self.data.extra.auto_theme = false;
                self.dirty = true;
            }
            if icon_button_ex(
                ui,
                t,
                "auto",
                tr("Auto"),
                "Auto",
                auto,
                0.0,
                if auto { t.text } else { t.text2 },
                t.text2,
            )
            .on_hover_text(tr("Follow your computer's appearance"))
            .clicked()
            {
                self.data.extra.auto_theme = true;
                self.dirty = true;
            }
        });
        ui.add_space(6.0);
        let mut show = !self.data.extra.hide_tray;
        if check(
            ui,
            &mut show,
            tr("Show recently opened tasks at the bottom"),
        )
        .changed()
        {
            self.data.extra.hide_tray = !show;
            self.dirty = true;
        }
        ui.add_space(6.0);
        ui.label(tr("Theme color"));
        ui.horizontal_wrapped(|ui| {
            for (name, c) in theme::ACCENTS {
                let sel = self.data.extra.accent == name;
                // ten swatches fit one row of the 640 pt settings page
                let (r, resp) = ui.allocate_exact_size(vec2(56.0, 52.0), Sense::click());
                hover_bg(ui, &resp, r, 8.0);
                ui.painter()
                    .circle_filled(r.center_top() + vec2(0.0, 16.0), 13.0, rgb(c));
                // "black" is the dark background's own colour: outline it
                if t.dark {
                    ui.painter().circle_stroke(
                        r.center_top() + vec2(0.0, 16.0),
                        13.0,
                        Stroke::new(1.0_f32, t.text3),
                    );
                }
                if sel {
                    ui.painter().circle_stroke(
                        r.center_top() + vec2(0.0, 16.0),
                        16.0,
                        Stroke::new(2.0_f32, t.text),
                    );
                }
                // the saved name is lower case ("black"); shown as a name ("Black")
                let label = name[..1].to_uppercase() + &name[1..];
                ui.painter().text(
                    r.center_bottom() - vec2(0.0, 8.0),
                    Align2::CENTER_CENTER,
                    tr(&label),
                    FontId::proportional(11.5),
                    t.text2,
                );
                if resp.clicked() {
                    self.data.extra.accent = name.to_string();
                    self.dirty = true;
                }
            }
        });
        false
    }

    /// The Shortcuts dialog. Returns true when it should close.
    pub(super) fn shortcuts_body(&mut self, ui: &mut Ui, _t: &Theme) -> bool {
        egui::Grid::new("shortcuts")
            .num_columns(2)
            .spacing(vec2(30.0, 8.0))
            .show(ui, |ui| {
                for (k, d) in [
                    ("\u{2318}J  /  \u{2318}K", "Search"),
                    // one row per action (Create task was listed twice)
                    ("\u{2318}N  /  \u{2325}T  /  T", "Create task"),
                    ("\u{2318}G", "New message"),
                    ("R", "Create reminder"),
                    ("N  /  P", "Open or close the notepad"),
                    ("\u{2318}\\", "Show or hide the sidebar"),
                    (
                        "\u{2318}1 \u{2013} \u{2318}4  /  \u{2318}0",
                        "Set or clear priority (open task)",
                    ),
                    ("\u{2318}/\u{21E7} click", "Select several tasks"),
                    ("/", "Commands in a Doc"),
                    ("\u{2318}[  /  \u{2318}]", "Back / forward"),
                    ("\u{2318},", "Settings"),
                    ("Enter", "Send a message or save a new task"),
                    ("Shift+Enter", "New line in a message"),
                    ("Esc", "Close a window or menu"),
                    ("Right-click", "Rename, delete, favorite, more"),
                ] {
                    // the system font: in the monospace font the Shift
                    // arrow came from a fallback and was drawn tiny
                    ui.label(RichText::new(crate::os::keys(tr(k))).strong());
                    ui.label(tr(d));
                    ui.end_row();
                }
            });
        false
    }

    /// The Help dialog. Returns true when it should close.
    pub(super) fn help_body(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        ui.label(match &self.data.extra.shared {
            None => tr("This is your own copy of the work manager. Everything is saved on this computer."),
            Some(c) if c.server.is_some() => tr("This workspace is on your team's server. A copy is kept on this computer, so you can keep working while the server can't be reached."),
            Some(_) => tr("This workspace is shared through a folder your sync app keeps in step. Your copy is saved on this computer and in that folder."),
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(crate::trf!("Saved in: {}", Data::dir().display()))
                .small()
                .color(t.text2),
        );
        ui.label(
            RichText::new(tr(
                "Tip: right-click spaces, lists and tasks for more options.",
            ))
            .small()
            .color(t.text2),
        );
        false
    }

    /// The Labs dialog. Returns true when it should close.
    pub(super) fn labs_body(&mut self, ui: &mut Ui, _t: &Theme) -> bool {
        ui.label(tr("Try features early. Turn them on or off at any time."));
        ui.add_space(6.0);
        for (id, label) in [
            ("compact", "Compact list rows"),
            ("week_monday", "Start the week on Monday in the Planner"),
            ("confetti", "Celebrate when a task is completed"),
        ] {
            let mut on = self.data.extra.labs.iter().any(|l| l == id);
            if check(ui, &mut on, tr(label)).changed() {
                if on {
                    self.data.extra.labs.push(id.to_string());
                } else {
                    self.data.extra.labs.retain(|l| l != id);
                }
                self.dirty = true;
            }
        }
        false
    }

    /// The Notifications dialog. Returns true when it should close.
    pub(super) fn notifications_body(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        // in Settings the page scrolls; in the dialog the list does.
        // (A list scrolling inside the scrolling page cut off its last button.)
        let max_h = if App::in_settings_page(ui) {
            f32::INFINITY
        } else {
            (ui.ctx().screen_rect().height() - 200.0).max(240.0)
        };
        egui::ScrollArea::vertical().id_salt("notify_settings").max_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
            let persist = self.persist;
            let manages = self.i_manage();
            let n = &mut self.data.extra.notify;
            let mut ch = false;
            ui.label(RichText::new(tr("Show in your Inbox when...")).strong());
            ch |= check(ui, &mut n.comments, tr("Someone comments on my tasks")).changed();
            ch |= check(ui, &mut n.mentions, tr("Someone @mentions me")).changed();
            ch |= check(ui, &mut n.assigned, tr("A task is assigned to me")).changed();
            ch |= check(ui, &mut n.reminders, tr("A reminder is due")).changed();
            ch |= check(ui, &mut n.chat, tr("New chat messages (unread counts)")).changed();
            ui.add_space(12.0);
            ui.label(RichText::new(tr("Desktop notifications")).strong());
            ch |= check(ui, &mut n.desktop, tr("Show banners when Workboard isn't in front")).changed();
            ch |= check(ui, &mut n.sound, tr("Play a sound")).changed();
            let badge = if cfg!(windows) { tr("Show a count on the taskbar icon") } else { tr("Show a count on the Dock icon") };
            ch |= check(ui, &mut n.dock_badge, badge).changed();
            ui.add_space(6.0);
            ui.label(RichText::new(tr("Mentions, assignments, comments and reminders follow the Inbox choices above. Also tell me about:")).small().color(t.text2));
            ch |= check(ui, &mut n.direct, tr("Direct messages")).changed();
            ch |= check(ui, &mut n.replies, tr("Replies in threads I'm in")).changed();
            ch |= check(ui, &mut n.huddles, tr("Huddles and calls")).changed();
            ch |= check(ui, &mut n.reactions, tr("Reactions to my messages")).changed();
            ch |= check(ui, &mut n.task_changes, tr("Changes to my tasks (status, due date, priority)")).changed();
            ui.horizontal(|ui| {
                ch |= check(ui, &mut n.due, tr("My tasks due today, each day at")).changed();
                let te = ui.add_enabled(n.due, field(&mut n.due_time).hint_text("09:00").desired_width(54.0));
                name_it(&te, "Due notice time");
                ch |= te.changed();
            });
            ui.horizontal(|ui| {
                ch |= check(ui, &mut n.meetings, tr("Meetings, this many minutes before:")).changed();
                let dv = ui.add_enabled(n.meetings, egui::DragValue::new(&mut n.meeting_minutes).range(1..=120));
                name_it(&dv, "Minutes before a meeting");
                ch |= dv.changed();
            });
            ch |= check(ui, &mut n.goals, tr("My goals due tomorrow or today")).changed();
            if manages {
                ch |= check(ui, &mut n.team, tr("Someone joins the workspace")).changed();
            }
            ch |= check(ui, &mut n.forms, tr("Answers to my forms")).changed();
            ch |= check(ui, &mut n.connection, tr("The team server connection drops and comes back")).changed();
            ui.add_space(12.0);
            ui.label(RichText::new(tr("Channel messages")).strong());
            for (mode, label) in [("all", "Every new message"), ("mentions", "Only @mentions and my threads"), ("nothing", "Nothing")] {
                if radio(ui, n.channel_default == mode, tr(label)).clicked() && n.channel_default != mode {
                    n.channel_default = mode.into();
                    // channels set the same way as the new default no longer stand apart
                    n.channel_modes.retain(|(_, m)| m != mode);
                    ch = true;
                }
            }
            ui.label(RichText::new(tr("Set one channel apart with the bell at the top of that channel. Direct messages follow \"Direct messages\" above.")).small().color(t.text2));
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Keywords")).strong());
            let kw = ui.add(field(&mut n.keywords).hint_text(tr("deploy, launch")).desired_width(300.0));
            name_it(&kw, "Notification keywords");
            ch |= kw.changed();
            ui.label(RichText::new(tr("Tell me when a message in a channel has one of these words, like a mention. Separate them with commas.")).small().color(t.text2));
            ui.add_space(10.0);
            let test = ui.button(tr("Send a test notification"));
            if test.clicked() {
                if persist {
                    crate::notifier::post("Workboard", &tr("Notifications are working."), "inbox", n.sound);
                }
                self.toast(if cfg!(windows) {
                    tr("Sent a test notification. If no banner shows, turn on notifications for Workboard in Windows Settings > System > Notifications.")
                } else {
                    tr("Sent a test notification. If no banner shows, allow Workboard in System Settings > Notifications.")
                });
            }
            if ch {
                self.dirty = true;
            }
        });
        false
    }

    /// The ClickApps dialog. Returns true when it should close.
    pub(super) fn click_apps_body(&mut self, ui: &mut Ui, _t: &Theme) -> bool {
        ui.label(tr("Turn features on or off for this Workspace."));
        ui.add_space(6.0);
        let a = &mut self.data.extra.apps;
        let mut ch = false;
        ch |= check(ui, &mut a.priorities, tr("Priorities")).changed();
        ch |= check(ui, &mut a.tags, tr("Tags")).changed();
        ch |= check(ui, &mut a.time_tracking, tr("Time Tracking")).changed();
        ch |= check(ui, &mut a.time_estimates, tr("Time Estimates")).changed();
        ch |= check(ui, &mut a.custom_fields, tr("Custom fields")).changed();
        ch |= check(ui, &mut a.multiple_assignees, tr("Multiple Assignees")).changed();
        if ch {
            self.dirty = true;
        }
        false
    }

    /// The CustomizeNav dialog. Returns true when it should close.
    pub(super) fn customize_nav_body(&mut self, ui: &mut Ui, _t: &Theme) -> bool {
        ui.label(tr("Choose the apps pinned to the left bar."));
        for label in RAIL_APPS {
            let mut on = self.data.extra.rail_items.iter().any(|r| r == label);
            if check(ui, &mut on, tr(label)).changed() {
                if on {
                    self.data.extra.rail_items.push(label.to_string());
                } else {
                    self.data.extra.rail_items.retain(|r| r != label);
                }
                self.dirty = true;
            }
        }
        false
    }

    /// The CustomizeSidebar dialog. Returns true when it should close.
    pub(super) fn customize_sidebar_body(&mut self, ui: &mut Ui, _t: &Theme) -> bool {
        ui.label(tr("Choose what shows in your sidebar."));
        for label in [
            "Inbox",
            "Replies",
            "Assigned Comments",
            "Posts",
            "Skills",
            "Artifacts",
            "Meetings",
            "My Tasks",
            "AI Chats",
            "Favorites",
            "Channels",
            "Direct Messages",
        ] {
            let mut on = !self.data.extra.hidden_nav.iter().any(|r| r == label);
            if check(ui, &mut on, tr(label)).changed() {
                if on {
                    self.data.extra.hidden_nav.retain(|r| r != label);
                } else {
                    self.data.extra.hidden_nav.push(label.to_string());
                }
                self.dirty = true;
            }
        }
        false
    }
}
