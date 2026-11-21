// Dialogs for how the app looks and behaves for me: themes, shortcuts,
// help, Labs, ClickApps and customizing the nav and sidebar (notifications
// are in notify_page.rs).

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
        if check(ui, &mut show, tr("Show the recent tasks row")).changed() {
            self.data.extra.hide_tray = !show;
            self.dirty = true;
        }
        // what the row is, right under the box's words
        ui.add_space(-ui.spacing().item_spacing.y);
        ui.horizontal(|ui| {
            ui.add_space(21.0);
            ui.add(
                egui::Label::new(
                    RichText::new(tr(
                        "Tasks you open line up along the bottom of the window, so you can open them again in one click.",
                    ))
                    .small()
                    .color(t.text3),
                )
                .wrap(),
            );
        });
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
                    ("\u{2303}1 \u{2013} \u{2303}9", "Go to a left bar item"),
                    ("\u{2318}/\u{21E7} click", "Select several tasks"),
                    ("/", "Commands in a Doc"),
                    // Cmd+Left / Cmd+Right do it too (parity.rs), like Safari
                    (
                        "\u{2318}[  /  \u{2318}]  /  \u{2318}\u{2190}  /  \u{2318}\u{2192}",
                        "Back / forward",
                    ),
                    ("\u{2318}Z", "Undo on a whiteboard"),
                    ("\u{2318}\u{21E7}Z", "Redo on a whiteboard"),
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
