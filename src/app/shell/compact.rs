// The compact window (under 600 wide, narrow.rs): one column like the iPhone
// app. The Home sidebar, laid over the whole page, is the home screen; every
// page opened from it gets a back button at the top left (and Cmd+[) that
// goes one step up: a List to its Folder or Space, a Space to the home
// screen, a Doc to the Docs list, a hub's page to the hub's own sidebar.

use super::*;

/// Where the back button goes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Up {
    /// the home screen: the Home sidebar over the page
    Home,
    /// the page's own sidebar (a hub's, Settings' sections)
    Sheet,
    Loc(Location),
    /// close the Doc, whiteboard, form or dashboard open in a hub
    CloseItem,
}

impl App {
    /// The home screen is up: the Home sidebar over the whole window.
    pub(crate) fn compact_at_home(&self) -> bool {
        self.tier == Tier::Compact && self.sheet.open && self.sheet.seen == Some(true)
    }

    /// One step up from here, and what the back button calls it; None on
    /// the home screen.
    fn compact_up(&self) -> Option<(Up, String)> {
        if self.sheet.open {
            return match self.sheet.seen {
                Some(true) => None,
                _ => Some((Up::Home, tr("Home").to_string())),
            };
        }
        let section_name = |s: Section| -> &'static str {
            match s {
                Section::Docs => "Docs",
                Section::Whiteboards => "Whiteboards",
                Section::Forms => "Forms",
                Section::Dashboards => "Dashboards",
                Section::Settings => "Settings",
                Section::Teams => "Teams",
                _ => "Home",
            }
        };
        if self.section == Section::Home {
            let d = &self.data;
            return Some(match self.loc {
                Location::List(l) => {
                    let folder = d
                        .spaces
                        .iter()
                        .flat_map(|s| s.folders.iter())
                        .find(|f| f.lists.iter().any(|x| x.id == l));
                    match (folder, d.space_of_list(l)) {
                        (Some(f), _) => (Up::Loc(Location::Folder(f.id)), f.name.clone()),
                        (None, Some(s)) => (Up::Loc(Location::Space(s.id)), s.name.clone()),
                        _ => (Up::Home, tr("Home").to_string()),
                    }
                }
                Location::Folder(f) => {
                    match d
                        .spaces
                        .iter()
                        .find(|s| s.folders.iter().any(|x| x.id == f))
                    {
                        Some(s) => (Up::Loc(Location::Space(s.id)), s.name.clone()),
                        None => (Up::Home, tr("Home").to_string()),
                    }
                }
                _ => (Up::Home, tr("Home").to_string()),
            });
        }
        if self.nav_spot().item.is_some() && self.section != Section::Chat {
            return Some((Up::CloseItem, tr(section_name(self.section)).to_string()));
        }
        if self.sheet.seen == Some(false) {
            return Some((Up::Sheet, tr(section_name(self.section)).to_string()));
        }
        Some((Up::Home, tr("Home").to_string()))
    }

    /// The back button, Cmd+[ and the mouse's back button in a compact
    /// window; false on the home screen (nothing to go back to).
    pub(crate) fn compact_back(&mut self) -> bool {
        let Some((up, _)) = self.compact_up() else {
            return false;
        };
        match up {
            Up::Home => {
                // the home screen covers the page: Home's own, which shows
                // nothing a hidden page shouldn't (a channel would be read)
                self.section = Section::Home;
                self.open_task = None;
                self.open_sheet_as(true);
            }
            Up::Sheet => self.open_sheet_as(false),
            Up::Loc(l) => self.actions.push(Action::Go(l)),
            Up::CloseItem => match self.section {
                Section::Docs => self.doc_open = None,
                Section::Whiteboards => self.sweep.wb.open = None,
                Section::Forms => self.form.open = None,
                Section::Dashboards => self.dash_open = None,
                _ => {}
            },
        }
        true
    }

    /// In a compact window, the back button's step (true even on the home
    /// screen, where there is none); elsewhere false.
    pub(crate) fn compact_back_or_not(&mut self) -> bool {
        if self.tier != Tier::Compact {
            return false;
        }
        self.compact_back();
        true
    }

    /// The top bar of a compact window, after the window buttons at `left`:
    /// the workspace on the home screen, the back button elsewhere; the
    /// search button at the right (the avatar comes after it, drawn by the
    /// caller from the right edge).
    pub(super) fn compact_bar(&mut self, ui: &mut Ui, full: Rect, left: f32) {
        let t = self.t();
        // the search button, left of the avatar (44 wide, at the right edge)
        let sr = Rect::from_center_size(
            pos2(full.right() - 44.0 - 4.0 - 15.0, full.center().y),
            vec2(30.0, 28.0),
        );
        let sresp = ui.interact(sr, ui.id().with("search_btn"), Sense::click());
        name_it(&sresp, "Search");
        if sresp.hovered() {
            ui.painter().rect_filled(sr, 6.0, t.hover);
        }
        super::super::icons::line_icon_scaled(
            ui.painter(),
            Icon::Search,
            sr.center(),
            t.text2,
            0.9,
        );
        if sresp
            .on_hover_text(format!(
                "{}  {}",
                tr("Search"),
                crate::os::keys("\u{2318}J")
            ))
            .clicked()
        {
            self.palette = Some(String::new());
        }
        let room = sr.left() - 8.0 - left;
        match self.compact_up() {
            None => {
                self.workspace_pill(ui, pos2(left, full.center().y), room - 58.0);
            }
            Some((_, name)) => {
                // "‹ Parent", the parent's name shortened to the room
                let font = FontId::proportional(14.0);
                let name = fit_text(ui.painter(), &name, &font, (room - 30.0).min(200.0));
                let g = ui.painter().layout_no_wrap(name, font, t.primary);
                let br = Rect::from_min_size(
                    pos2(left, full.center().y - 14.0),
                    vec2(g.size().x + 26.0, 28.0),
                );
                let resp = ui.interact(br, ui.id().with("compact_back"), Sense::click());
                name_it(&resp, "Back");
                if resp.hovered() {
                    ui.painter().rect_filled(br, 6.0, t.hover);
                }
                // the chevron close after the window buttons, as on the iPhone
                let c = br.left_center() + vec2(8.0, 0.0);
                ui.painter().line(
                    vec![c + vec2(3.5, -6.0), c + vec2(-2.5, 0.0), c + vec2(3.5, 6.0)],
                    Stroke::new(2.0_f32, t.primary),
                );
                ui.painter().galley(
                    pos2(br.left() + 20.0, br.center().y - g.size().y / 2.0),
                    g,
                    t.primary,
                );
                let keys = crate::os::keys("\u{2318}[");
                if resp
                    .on_hover_text(format!("{}  {}", tr("Go back"), keys))
                    .clicked()
                {
                    self.compact_back();
                }
            }
        }
    }

    /// The home screen's first rows: the apps the rail holds (the rail
    /// isn't shown in a compact window).
    pub(super) fn compact_apps(&mut self, ui: &mut Ui, tc: Color32) {
        for label in self.rail_order() {
            let icon = rail_icon(&label);
            let r = self.side_row_as(
                ui,
                0.0,
                false,
                |p, c| line_icon(p, icon, c, tc),
                &format!("App {}", label),
                tr(&label),
                None,
                None,
                true,
            );
            if r.clicked() {
                if label == "Home" {
                    self.actions.push(Action::Go(Location::Home));
                } else {
                    self.open_app(&label);
                }
                // picked, even when it was already the page under it
                self.sheet.close();
            }
        }
    }
}
