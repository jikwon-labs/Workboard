// How the window's width shapes the shell. Wide (960 and up) is the layout as
// designed: rail, sidebar, page. Medium (600 to 959) keeps the rail and lays
// the sidebar over the page as a drawer, opened from the button beside the
// window buttons (or Ctrl+Cmd+S, Cmd+\) and closed again once something in it
// is picked. Compact (under 600) is one column like the iPhone app: no rail,
// the sidebar is the home screen and a back button at the top left leaves
// each page. Every screen asks `self.tier` instead of measuring the window.

use super::*;
use crate::app::hubs::HubFilters;
use crate::app::nav::NavSpot;
use crate::app::settings::SettingsTab;

/// What the page shows: a pick in the sidebar sheet changes it (a list, a
/// hub's filter, a Settings section), and that closes the sheet.
type PageKey = (NavSpot, SettingsTab, HubFilters, Option<u64>, Option<u64>);

/// The window's width band. Read it from `App::tier`, set once a frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Tier {
    #[default]
    Wide,
    Medium,
    Compact,
}

impl Tier {
    /// Below this the window is one column.
    pub(crate) const MEDIUM_FROM: f32 = 600.0;
    /// From this the sidebar sits beside the page again.
    pub(crate) const WIDE_FROM: f32 = 960.0;

    pub(crate) fn of(width: f32) -> Tier {
        if width >= Self::WIDE_FROM {
            Tier::Wide
        } else if width >= Self::MEDIUM_FROM {
            Tier::Medium
        } else {
            Tier::Compact
        }
    }
}

/// The sidebar shown over the page because the window is too narrow for it
/// beside the page: a drawer in a medium window, the whole column in a
/// compact one (where it is the home screen).
#[derive(Default)]
pub(crate) struct Sheet {
    pub open: bool,
    /// What the page showed when it opened: picking something else closes it.
    pub at: Option<PageKey>,
    /// The page drew a sidebar in the last frame: Some(true) Home's (the
    /// home screen of a compact window), Some(false) its own (a hub's,
    /// Settings').
    pub seen: Option<bool>,
    seen_now: Option<bool>,
}

impl Sheet {
    pub(crate) fn close(&mut self) {
        self.open = false;
        self.at = None;
    }
}

/// How far the drawer's shade darkens the page under it.
const SHADE: f32 = 0.18;

impl App {
    /// Reads the window's width into `self.tier` (start of each frame) and
    /// closes the sheet once something in it has been picked.
    pub(crate) fn update_tier(&mut self, ctx: &egui::Context) {
        // beside a page moved out (the iPad's split screen): what is left
        let tier = Tier::of(ctx.screen_rect().width() - self.pop.split_w);
        self.sheet.seen = self.sheet.seen_now.take();
        if tier != self.tier {
            // back beside the page: the drawer has nothing to cover
            if tier == Tier::Wide {
                self.sheet.close();
            }
            self.tier = tier;
        }
        if self.sheet.open && self.sheet.at.is_some_and(|at| at != self.page_key()) {
            self.sheet.close();
        }
    }

    /// Keeps the window's size and place for the next start, once it has
    /// stood still half a second (not while full screen).
    pub(crate) fn keep_window_state(&mut self, ctx: &egui::Context) {
        use crate::winstate::WinState;
        let (inner, outer, full, now) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.inner_rect,
                v.outer_rect,
                v.fullscreen.unwrap_or(false),
                i.time,
            )
        });
        let (Some(inner), Some(outer)) = (inner, outer) else {
            return;
        };
        if full {
            return;
        }
        let st = WinState {
            w: inner.width(),
            h: inner.height(),
            x: outer.left(),
            y: outer.top(),
        };
        match self.win_seen {
            Some((seen, _)) if seen == st => {}
            _ => self.win_seen = Some((st, now)),
        }
        if let Some((seen, at)) = self.win_seen {
            if self.win_saved != Some(seen) {
                if now - at >= 0.5 {
                    crate::winstate::save(seen);
                    self.win_saved = Some(seen);
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(600));
                }
            }
        }
    }

    /// A compact window's sidebar is open over the whole page: the page
    /// isn't drawn.
    pub(crate) fn page_covered(&self) -> bool {
        self.tier == Tier::Compact && self.sheet.open
    }

    /// The sidebar doesn't fit beside the page.
    pub(crate) fn sidebar_over_page(&self) -> bool {
        self.tier != Tier::Wide
    }

    fn page_key(&self) -> PageKey {
        (
            self.nav_spot(),
            self.settings_tab,
            self.hub,
            self.team_open,
            self.meeting_open,
        )
    }

    pub(crate) fn open_sheet(&mut self) {
        self.sheet.open = true;
        self.sheet.at = Some(self.page_key());
    }

    /// Opens the sheet for a page about to draw Home's sidebar (`main`) or
    /// its own.
    pub(crate) fn open_sheet_as(&mut self, main: bool) {
        self.open_sheet();
        self.sheet.seen = Some(main);
    }

    /// Cmd+\, Ctrl+Cmd+S and the sidebar's own close buttons: beside the
    /// page that opens or closes the sidebar for good (the person's choice,
    /// kept); over it, the drawer opens or closes and the choice stays.
    pub(crate) fn toggle_sidebar(&mut self) {
        if self.sidebar_over_page() {
            if self.sheet.open {
                self.sheet.close();
            } else {
                self.open_sheet();
            }
        } else {
            self.sidebar_collapsed = !self.sidebar_collapsed;
        }
    }

    /// The sidebar's « (and the hubs' sidebars'): closes it.
    pub(crate) fn close_sidebar(&mut self) {
        if self.sidebar_over_page() {
            self.sheet.close();
        } else {
            self.sidebar_collapsed = true;
        }
    }

    /// The left sidebar of Home and the app hubs, with `body` inside: beside
    /// the page in a wide window (the "»" strip when closed), over it as a
    /// drawer otherwise (nothing while the drawer is closed).
    /// `main` is Home's sidebar (the compact window's home screen).
    pub(crate) fn side_panel(
        &mut self,
        ui: &mut Ui,
        margin: egui::Margin,
        main: bool,
        body: impl FnOnce(&mut Self, &mut Ui),
    ) {
        let t = self.t();
        self.sheet.seen_now = Some(main);
        if !self.sidebar_over_page() {
            if self.sidebar_collapsed {
                self.collapsed_sidebar(ui);
                return;
            }
            // one id for Home and the hubs, so one dragged width applies
            // everywhere
            egui::SidePanel::left("sidebar")
                .resizable(true)
                .default_width(255.0)
                .width_range(200.0..=400.0)
                .frame(side_panel_frame(&t, margin))
                .show_inside(ui, |ui| body(self, ui));
            return;
        }
        if !self.sheet.open {
            return;
        }
        let ctx = ui.ctx().clone();
        let card = ui.max_rect();
        let w = if self.tier == Tier::Compact {
            card.width()
        } else {
            // as wide as the person last dragged it beside the page
            egui::containers::panel::PanelState::load(&ctx, egui::Id::new("sidebar"))
                .map_or(255.0, |s| s.rect.width())
                .clamp(200.0, 400.0)
                .min(card.width() - 48.0)
        };
        let drawer = Rect::from_min_size(card.min, vec2(w, card.height()));
        // the home screen is where Escape would lead: it stays
        let home = main && self.tier == Tier::Compact;
        if !home
            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && !any_menu_was_open(&ctx)
            && self.modal.is_none()
        {
            self.sheet.close();
            return;
        }
        egui::Area::new(egui::Id::new("sidebar_sheet"))
            .order(egui::Order::Middle)
            .fixed_pos(card.min)
            .constrain(false)
            .movable(false)
            .show(&ctx, |ui| {
                ui.set_clip_rect(card);
                // the page under it, shaded: a click there closes the drawer
                let shade = ui.allocate_rect(card, Sense::click());
                if shade.clicked()
                    && !drawer.contains(shade.interact_pointer_pos().unwrap_or(card.max))
                {
                    self.sheet.close();
                }
                if w < card.width() {
                    ui.painter().rect_filled(
                        card,
                        8.0,
                        Color32::from_black_alpha((255.0 * SHADE) as u8),
                    );
                    // its edge lifted off the page
                    let edge = Rect::from_min_max(
                        drawer.right_top(),
                        drawer.right_bottom() + vec2(10.0, 0.0),
                    );
                    ui.painter()
                        .rect_filled(edge, 0.0, Color32::from_black_alpha(14));
                }
                let mut inner = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(drawer)
                        .layout(Layout::top_down(Align::Min)),
                );
                inner.painter().rect_filled(drawer, 8.0, t.sidebar);
                egui::SidePanel::left("sidebar_sheet_panel")
                    .exact_width(w)
                    .resizable(false)
                    .show_separator_line(w < card.width())
                    .frame(side_panel_frame(&t, margin))
                    .show_inside(&mut inner, |ui| body(self, ui));
            });
    }

    /// The button beside the window buttons that opens and closes the
    /// drawer (medium window only); `at` is its left edge. Returns its width.
    pub(super) fn sheet_button(&mut self, ui: &mut Ui, at: Pos2) -> f32 {
        if self.tier != Tier::Medium {
            return 0.0;
        }
        let t = self.t();
        let r = Rect::from_min_size(at - vec2(0.0, 14.0), vec2(30.0, 28.0));
        let resp = ui.interact(r, ui.id().with("sheet_btn"), Sense::click());
        name_it(&resp, "Toggle sidebar");
        if resp.hovered() || self.sheet.open {
            ui.painter().rect_filled(r, 6.0, t.hover);
        }
        // a window with its left part marked off
        let b = Rect::from_center_size(r.center(), vec2(16.0, 13.0));
        let s = Stroke::new(1.4_f32, t.text2);
        ui.painter()
            .rect_stroke(b, 3.0, s, egui::StrokeKind::Middle);
        ui.painter().line_segment(
            [
                pos2(b.left() + 5.5, b.top()),
                pos2(b.left() + 5.5, b.bottom()),
            ],
            s,
        );
        let tip = if self.sheet.open {
            tr("Close sidebar")
        } else {
            tr("Open sidebar")
        };
        let keys = if cfg!(target_os = "macos") {
            "\u{2303}\u{2318}S".to_string()
        } else {
            crate::os::keys("\u{2318}\\")
        };
        if resp.on_hover_text(format!("{}  {}", tip, keys)).clicked() {
            self.toggle_sidebar();
        }
        34.0
    }
}
