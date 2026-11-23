// The Settings page: one full page with a section list on the left (Admin,
// Features, Personal), as in the original. Each section shows the same
// controls the matching dialog has, so behaviour stays in one place.

use super::*;

/// The sections of the Settings page, in the order the left list shows
/// them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsTab {
    #[default]
    General,
    People,
    Spaces,
    Trash,
    Apps,
    CustomFields,
    TagManager,
    Templates,
    Automations,
    WorkTogether,
    MySettings,
    Notifications,
    Themes,
    Language,
    KeyboardShortcuts,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 15] = [
        SettingsTab::General,
        SettingsTab::People,
        SettingsTab::Spaces,
        SettingsTab::Trash,
        SettingsTab::Apps,
        SettingsTab::CustomFields,
        SettingsTab::TagManager,
        SettingsTab::Templates,
        SettingsTab::Automations,
        SettingsTab::WorkTogether,
        SettingsTab::MySettings,
        SettingsTab::Notifications,
        SettingsTab::Themes,
        SettingsTab::Language,
        SettingsTab::KeyboardShortcuts,
    ];

    /// Its name in the list (English; shown translated, and the name tests
    /// and screen readers use after "Settings ").
    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::People => "People",
            SettingsTab::Spaces => "Spaces",
            SettingsTab::Trash => "Trash",
            SettingsTab::Apps => "Apps",
            SettingsTab::CustomFields => "Custom fields",
            SettingsTab::TagManager => "Tag manager",
            SettingsTab::Templates => "Templates",
            SettingsTab::Automations => "Automations",
            SettingsTab::WorkTogether => "Work together",
            SettingsTab::MySettings => "My settings",
            SettingsTab::Notifications => "Notifications",
            SettingsTab::Themes => "Themes",
            SettingsTab::Language => "Language",
            SettingsTab::KeyboardShortcuts => "Keyboard shortcuts",
        }
    }

    /// The heading it is listed under.
    fn group(self) -> &'static str {
        use SettingsTab::*;
        match self {
            General | People | Spaces | Trash => "Admin",
            Apps | CustomFields | TagManager | Templates | Automations | WorkTogether => "Features",
            MySettings | Notifications | Themes | Language | KeyboardShortcuts => "Personal",
        }
    }

    pub fn from_label(label: &str) -> Option<SettingsTab> {
        Self::ALL.into_iter().find(|s| s.label() == label)
    }

    /// Changes the whole workspace for everyone: on a team server only its
    /// owner and admins see it (the server keeps it too), and General (the
    /// workspace's name and colour) only its owner.
    fn admin_only(self) -> bool {
        use SettingsTab::*;
        matches!(
            self,
            General | Apps | CustomFields | TagManager | Automations
        )
    }
}

impl App {
    /// Whether I see `tab` in Settings (see [`SettingsTab::admin_only`]).
    fn may_open_settings(&self, tab: SettingsTab) -> bool {
        match tab {
            SettingsTab::General => self.has_owner_rights(),
            _ => !tab.admin_only() || self.may_manage(),
        }
    }

    pub(super) fn open_settings(&mut self, label: &str) {
        let tab = SettingsTab::from_label(label).unwrap_or_default();
        self.settings_tab = if self.may_open_settings(tab) {
            tab
        } else {
            SettingsTab::MySettings
        };
        self.settings_state = None;
        // a photo picked last time and not saved doesn't come back
        self.profile_photo = None;
        self.section = Section::Settings;
    }

    fn settings_modal_for(&self, label: &str) -> Option<Modal> {
        Some(match label {
            "General" => Modal::WorkspaceSettings {
                name: self.data.workspace.clone(),
            },
            "Trash" => Modal::Trash,
            "Apps" => Modal::ClickApps,
            "Custom fields" => Modal::custom_fields(),
            "Tag manager" => Modal::TagManager {
                from: String::new(),
                to: String::new(),
            },
            "Templates" => self.template_center(super::templates::Category::All, None),
            "Automations" => Modal::automations(None),
            "Work together" => self.together_modal(),
            "My settings" => self.profile_modal(),
            "Notifications" => Modal::Notifications,
            "Themes" => Modal::Themes,
            "Keyboard shortcuts" => Modal::Shortcuts,
            _ => return None,
        })
    }

    pub(super) fn settings_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        let full = ui.available_rect_before_wrap();
        let body = if self.sidebar_over_page() {
            // a narrow window: the section list is the sidebar drawer
            // (the home screen's next step in a compact one), the section
            // has the width
            self.side_panel(ui, egui::Margin::same(10), false, |app, ui| {
                app.settings_nav(ui)
            });
            if self.page_covered() {
                return;
            }
            full
        } else {
            let nav_w = 230.0;
            let nav = Rect::from_min_max(full.min, pos2(full.left() + nav_w, full.bottom()));
            ui.painter().rect_filled(nav, 0.0, t.sidebar);
            ui.painter()
                .vline(nav.right(), nav.y_range(), Stroke::new(1.0_f32, t.border));
            let mut nui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(nav.shrink(10.0))
                    .layout(Layout::top_down(Align::Min)),
            );
            self.settings_nav(&mut nui);
            Rect::from_min_max(pos2(full.left() + nav_w, full.top()), full.max)
        };
        if !self.may_open_settings(self.settings_tab) {
            // a member (or an admin, for General) lands on their own settings instead
            self.settings_tab = SettingsTab::MySettings;
            self.settings_state = None;
        }
        let label = self.settings_tab.label();
        // someone's profile, opened from their name in People: at the
        // right, the whole height of the section
        let mut body = body;
        if label == "People" && self.people_profile.is_some() {
            let mut pui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(body)
                    .layout(Layout::top_down(Align::Min)),
            );
            self.people_profile_panel(&mut pui);
            if self.people_profile.is_some() {
                // the panel moved what's left of the row to its left
                body.max.x = pui.available_rect_before_wrap().right();
            }
        }
        let pad = if self.tier == crate::app::shell::Tier::Compact {
            16.0
        } else {
            32.0
        };
        let mut bui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body.shrink2(vec2(pad, 20.0)))
                .layout(Layout::top_down(Align::Min)),
        );
        bui.label(RichText::new(tr(label)).size(22.0).strong());
        bui.add_space(12.0);
        match label {
            "People" => {
                self.people_page(&mut bui);
                return;
            }
            "Spaces" => {
                self.spaces_settings(&mut bui);
                return;
            }
            "Language" => {
                self.language_settings(&mut bui);
                return;
            }
            _ => {}
        }
        egui::ScrollArea::vertical()
            .id_salt("settings_body")
            .auto_shrink([false, false])
            .show(&mut bui, |ui| {
                // at most 640 wide, but never wider than a small window leaves
                ui.set_max_width(ui.available_width().min(640.0));
                let mut state = self
                    .settings_state
                    .take()
                    .or_else(|| self.settings_modal_for(label));
                if let Some(m) = state.as_mut() {
                    // this page scrolls already: a section that scrolls in its
                    // own dialog draws its rows straight in here
                    let nr = ui.ctx().cumulative_pass_nr();
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::new(IN_SETTINGS_PAGE), nr));
                    let done = self.modal_body(ui, &t, m);
                    if done {
                        // saved: start the section fresh
                        state = None;
                    }
                }
                self.settings_state = state;
            });
    }

    /// The Settings page's left list: back to the previous page, then its
    /// sections by group.
    fn settings_nav(&mut self, nui: &mut Ui) {
        let t = self.t();
        if nui
            .add(ghost(egui::Button::new(
                RichText::new(tr("\u{2190}  Back to Workspace")).color(t.text2),
            )))
            .clicked()
        {
            // back to the page Settings was opened from, like the Back arrow
            self.go_back();
            if self.section == Section::Settings {
                // nothing before Settings in the history
                self.section = Section::Home;
            }
        }
        nui.add_space(6.0);
        let ws = self.data.workspace.clone();
        nui.label(RichText::new(ws).strong());
        // a short window can't fit every section: the list scrolls
        egui::ScrollArea::vertical()
            .id_salt("settings_nav")
            .auto_shrink([false, false])
            .show(nui, |nui| {
                let mut last_group = "";
                for tab in SettingsTab::ALL {
                    let (group, label) = (tab.group(), tab.label());
                    // sections that change the workspace for everyone are the owner's and admins'
                    if !self.may_open_settings(tab) {
                        continue;
                    }
                    if group != last_group {
                        last_group = group;
                        nui.add_space(8.0);
                        nui.label(RichText::new(tr(group)).small().color(t.text3));
                    }
                    let sel = self.settings_tab == tab;
                    let w = nui.available_width();
                    let (r, resp) = nui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                    name_it(&resp, &format!("Settings {}", label));
                    if sel {
                        nui.painter().rect_filled(r, 6.0, t.selected_bg);
                    } else if resp.hovered() {
                        nui.painter().rect_filled(r, 6.0, t.hover);
                    }
                    paint_fit(
                        nui,
                        r.left_center() + vec2(10.0, 0.0),
                        Align2::LEFT_CENTER,
                        tr(label),
                        FontId::proportional(13.5),
                        if sel { t.selected_text } else { t.text },
                        w - 14.0,
                    );
                    if resp.clicked() && !sel {
                        self.settings_tab = tab;
                        self.settings_state = None;
                    }
                }
            });
    }

    /// True while `modal_body` draws a section of this page (not a dialog).
    pub(super) fn in_settings_page(ui: &Ui) -> bool {
        let nr = ui.ctx().cumulative_pass_nr();
        ui.ctx()
            .data(|d| d.get_temp::<u64>(egui::Id::new(IN_SETTINGS_PAGE)))
            == Some(nr)
    }

    /// The interface language: the system's, English or Korean.
    fn language_settings(&mut self, ui: &mut Ui) {
        use crate::i18n::Choice;
        ui.label(RichText::new(tr("The language of menus, buttons and messages on this computer. Names and text people write stay as they are.")).color(self.t().text2));
        ui.add_space(10.0);
        // each language is named in itself, so it can be found whatever is on
        for (c, label) in [
            (Choice::System, tr("Same as the system")),
            (Choice::En, "English"),
            (Choice::Ko, "\u{d55c}\u{ad6d}\u{c5b4}"),
        ] {
            let r = radio(ui, self.lang_choice == c, label);
            name_it(&r, &format!("Language {}", crate::i18n::choice_key(c)));
            if r.clicked() && self.lang_choice != c {
                self.lang_choice = c;
                crate::i18n::set_lang(crate::i18n::resolve(c));
                // what is kept worked out (the Inbox rows) holds words in
                // the language that was shown ("Someone")
                self.data_changed();
                if self.persist {
                    Data::set_language(c);
                }
            }
        }
    }

    /// Spaces table: open, rename, colour, statuses, delete.
    fn spaces_settings(&mut self, ui: &mut Ui) {
        let t = self.t();
        // every space that is not archived, hidden ones included
        let spaces: Vec<(u64, String, [u8; 3], usize, usize)> = self
            .open_spaces()
            .iter()
            .map(|s| {
                // archived Lists and tasks are counted under Archived below
                let lists: Vec<u64> = self
                    .data
                    .lists_in(Location::Space(s.id))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|l| !self.data.list_archived(*l))
                    .collect();
                let tasks = self
                    .data
                    .tasks
                    .iter()
                    .filter(|t| lists.contains(&t.list_id) && !t.archived)
                    .count();
                (s.id, s.name.clone(), s.color, lists.len(), tasks)
            })
            .collect();
        ui.horizontal(|ui| {
            ui.label(RichText::new(trf!("{} Spaces", spaces.len())).color(t.text2));
            // beside the count, near the table (at the page's right edge it
            // was far from the narrow table it adds to)
            ui.add_space(12.0);
            if primary_button(ui, &t, tr("+ New Space")).clicked() {
                self.modal = Some(Modal::NewSpace {
                    name: String::new(),
                    color: STATUS_PALETTE[1 + (self.data.spaces.len() * 3) % 13],
                });
            }
        });
        ui.add_space(8.0);
        if spaces.is_empty() {
            ui.label(
                RichText::new(tr(
                    "No spaces yet. A Space holds your Lists and their tasks: make one with + New Space.",
                ))
                .color(t.text3),
            );
        }
        let gap = if self.tier == crate::app::shell::Tier::Compact {
            10.0
        } else {
            24.0
        };
        egui::Grid::new("spaces_table")
            .num_columns(5)
            .striped(true)
            .spacing(vec2(gap, 10.0))
            .show(ui, |ui| {
                // no header over an empty table
                if spaces.is_empty() {
                    return;
                }
                for h in ["Name", "Lists", "Tasks", "", ""] {
                    ui.label(RichText::new(tr(h)).small().color(t.text3));
                }
                ui.end_row();
                for (id, name, color, nl, nt) in spaces {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        ui.painter().rect_filled(r, 4.0, rgb(color));
                        if ui.link(&name).clicked() {
                            self.actions.push(Action::Go(Location::Space(id)));
                        }
                    });
                    ui.label(nl.to_string());
                    ui.label(nt.to_string());
                    let r = super::icons::square_button(
                        ui,
                        "\u{22EF}",
                        crate::widgets::CONTROL_H,
                        &format!("Space options {}", name),
                    );
                    click_menu(&r, |ui| self.space_menu(ui, id, true));
                    let hidden = self
                        .data
                        .spaces
                        .iter()
                        .find(|s| s.id == id)
                        .is_some_and(|s| s.hidden);
                    if hidden {
                        let b = ui.button(tr("Show in sidebar"));
                        name_it(&b, &format!("Show {}", name));
                        if b.clicked() {
                            if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == id) {
                                s.hidden = false;
                            }
                            self.dirty = true;
                        }
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
        // archived spaces and lists, with Restore
        let arch_spaces: Vec<(u64, String)> = self
            .data
            .spaces
            .iter()
            .filter(|s| s.archived && s.private_owner.is_none())
            .map(|s| (s.id, s.name.clone()))
            .collect();
        // a List in a Folder shows its Folder too, as Move tasks does
        let arch_lists: Vec<(u64, String)> = self
            .data
            .list_places()
            .filter(|p| !p.space.archived && p.list.archived)
            .map(|p| (p.list.id, p.path()))
            .collect();
        let arch_folders: Vec<(u64, String)> = self
            .data
            .spaces
            .iter()
            .filter(|s| !s.archived)
            .flat_map(|s| {
                s.folders
                    .iter()
                    .filter(|f| f.archived)
                    .map(move |f| (f.id, format!("{} / {}", s.name, f.name)))
            })
            .collect();
        ui.add_space(18.0);
        ui.label(RichText::new(tr("Archived")).strong());
        if arch_spaces.is_empty() && arch_lists.is_empty() && arch_folders.is_empty() {
            ui.label(RichText::new(tr("Nothing is archived. Archived Spaces and Lists show here so you can restore them.")).color(t.text3));
        }
        egui::Grid::new("archived_table")
            .num_columns(3)
            .striped(true)
            .spacing(vec2(24.0, 8.0))
            .show(ui, |ui| {
                for (id, name) in arch_spaces {
                    ui.label(RichText::new(tr("Space")).small().color(t.text3));
                    ui.label(&name);
                    let b = ui.button(tr("Restore"));
                    name_it(&b, &format!("Restore {}", name));
                    if b.clicked() {
                        self.set_space_archived(id, false);
                    }
                    ui.end_row();
                }
                for (id, name) in arch_folders {
                    ui.label(RichText::new(tr("Folder")).small().color(t.text3));
                    ui.label(&name);
                    let b = ui.button(tr("Restore"));
                    name_it(&b, &format!("Restore {}", name));
                    if b.clicked() {
                        self.set_folder_archived(id, false);
                    }
                    ui.end_row();
                }
                for (id, name) in arch_lists {
                    ui.label(RichText::new(tr("List")).small().color(t.text3));
                    ui.label(&name);
                    let b = ui.button(tr("Restore"));
                    name_it(&b, &format!("Restore {}", name));
                    if b.clicked() {
                        self.set_list_archived(id, false);
                    }
                    ui.end_row();
                }
            });
    }
}

const IN_SETTINGS_PAGE: &str = "__wb_in_settings_page";
