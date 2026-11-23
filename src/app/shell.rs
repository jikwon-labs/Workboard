// The window shell measured from the person's running ClickUp: black icon
// rail, light top bar with back/forward and search, the Home sidebar (with
// Channels and Direct Messages; sidebar.rs, side_tree.rs) and the page header
// with coloured view tabs and pill toolbar (header.rs). The List and Board
// views are in listview.rs and board.rs. See MECHANICS.md.

use super::*;
use chrono::Datelike;
use egui::{Pos2, Response};

mod compact;
mod header;
mod narrow;
mod side_tree;
mod sidebar;

pub(crate) use narrow::{Sheet, Tier};

pub(super) use super::icons::line_icon;
use super::icons::paint_view_icon;

/// What is being dragged in the sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SideDrag {
    Space(u64),
    List(u64),
}

/// A red pill with a white count for what hasn't been looked at yet (Inbox,
/// replies, unread messages), placed by `align` at `at`. Over 99 reads "99+".
pub(super) fn paint_unread_badge(
    p: &egui::Painter,
    danger: Color32,
    at: Pos2,
    align: Align2,
    n: usize,
) -> Rect {
    let label = if n > 99 {
        "99+".to_string()
    } else {
        n.to_string()
    };
    let g = p.layout_no_wrap(label, FontId::proportional(11.0), Color32::WHITE);
    let h = 16.0;
    let r = align.anchor_size(at, vec2((g.size().x + 10.0).max(h), h));
    p.rect_filled(r, h / 2.0, danger);
    p.galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
    r
}

/// The red count on a rail icon: smaller than the sidebar's, with a ring in
/// the rail's color so it reads as cut out of the icon's background instead
/// of sitting on top of it.
fn paint_rail_badge(p: &egui::Painter, danger: Color32, rail: Color32, icon: Pos2, n: usize) {
    let label = if n > 99 {
        "99+".to_string()
    } else {
        n.to_string()
    };
    let g = p.layout_no_wrap(label, FontId::proportional(10.0), Color32::WHITE);
    let h = 14.0;
    let r = Rect::from_min_size(icon + vec2(6.0, -16.0), vec2((g.size().x + 8.0).max(h), h));
    p.rect_filled(r.expand(2.0), h / 2.0 + 2.0, rail);
    p.rect_filled(r, h / 2.0, danger);
    p.galley(r.center() - g.size() / 2.0, g, Color32::WHITE);
}

/// The distance from one rail item's top to the next one's.
const RAIL_STEP: f32 = 62.0;

/// A rail item being dragged: its place in the bar's order, the top it
/// floats at under the mouse, and the place it would take if let go.
#[derive(Clone, Copy)]
struct RailLift {
    from: usize,
    at: f32,
    to: usize,
}

/// The keys that open the rail's items, top to bottom.
const RAIL_KEYS: [egui::Key; 9] = [
    egui::Key::Num1,
    egui::Key::Num2,
    egui::Key::Num3,
    egui::Key::Num4,
    egui::Key::Num5,
    egui::Key::Num6,
    egui::Key::Num7,
    egui::Key::Num8,
    egui::Key::Num9,
];

/// The small number at the bottom right of a rail icon while Ctrl is held:
/// Ctrl plus that number opens the item.
fn paint_key_hint(p: &egui::Painter, ibox: Rect, n: usize) {
    let r = Rect::from_center_size(ibox.center() + vec2(8.0, 7.0), vec2(13.0, 13.0));
    p.rect_filled(r, 4.0, Color32::WHITE);
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        n.to_string(),
        FontId::proportional(10.0),
        Color32::from_gray(20),
    );
}

/// The line icon of an item the rail can hold.
pub(super) fn rail_icon(label: &str) -> Icon {
    match label {
        "Home" => Icon::Home,
        "Planner" => Icon::Planner,
        "AI" => Icon::Ai,
        "Teams" => Icon::Teams,
        "Chat" => Icon::Chat,
        "Docs" => Icon::Docs,
        "Dashboards" => Icon::Dashboards,
        "Whiteboards" => Icon::Whiteboards,
        "Forms" => Icon::Forms,
        "Goals" => Icon::Goals,
        "Timesheets" => Icon::Timesheets,
        "Clips" => Icon::Video,
        "Apps" => Icon::Apps,
        "Spaces" => Icon::Space,
        _ => Icon::Dots,
    }
}

/// A rail item's name beside the bar, level with its icon (`ibox`), once
/// the pointer has rested on it as long as egui waits for a tooltip, or at
/// once while moving on from another item's. egui puts a tooltip under its
/// item, which in the bar covered the next item.
fn rail_tip(resp: &Response, rail: Rect, ibox: Rect, text: &str) {
    let ctx = &resp.ctx;
    if !resp.hovered() || ctx.memory(|m| m.any_popup_open()) {
        return;
    }
    let style = ctx.style();
    let (delay, grace) = (
        style.interaction.tooltip_delay,
        style.interaction.tooltip_grace_time,
    );
    let (now, still, since_click) = ctx.input(|i| {
        (
            i.time,
            i.pointer.time_since_last_movement(),
            i.pointer.time_since_last_click(),
        )
    });
    // clicking an item and resting there shows nothing, as with egui's
    if since_click < still + 0.1 {
        return;
    }
    let last_id = egui::Id::new("rail_tip_shown");
    let last: Option<f64> = ctx.data(|d| d.get_temp(last_id));
    let recent = last.is_some_and(|t| now - t <= grace as f64 + 0.1);
    if still < delay && !recent {
        ctx.request_repaint_after_secs(delay - still);
        return;
    }
    ctx.data_mut(|d| d.insert_temp(last_id, now));
    egui::Area::new(resp.id.with("rail_tip"))
        .kind(egui::UiKind::Popup)
        .order(egui::Order::Tooltip)
        .pivot(Align2::LEFT_CENTER)
        .fixed_pos(pos2(rail.right() + 8.0, ibox.center().y))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.add(egui::Label::new(text).extend());
            });
        });
}

// moved to widgets.rs; kept here for code that names it `shell::fit_text`
pub(super) use crate::widgets::fit_text;

// moved to listview/columns.rs; kept here for tests that name it `shell::col_layout`
#[cfg(test)]
pub(super) use super::listview::col_layout;

impl App {
    // ---------------------------------------------------------------- history

    pub(super) fn track_history(&mut self) {
        let cur = self.nav_spot();
        match self.nav.last {
            // a page filling in its default item (Chat opening the first
            // channel) is not a move
            Some(last)
                if last != cur
                    && last.section == cur.section
                    && last.loc == cur.loc
                    && last.item.is_none() => {}
            Some(last) if last != cur => {
                if self.nav.skip {
                    self.nav.skip = false;
                } else {
                    self.nav.back.push(last);
                    self.nav.fwd.clear();
                }
            }
            _ => {}
        }
        self.nav.last = Some(cur);
    }

    pub(super) fn go_back(&mut self) {
        // pages deleted since are dropped from the history
        while let Some(prev) = self.nav.back.pop() {
            if !self.nav_spot_exists(&prev) {
                continue;
            }
            self.nav.fwd.push(self.nav_spot());
            self.show_nav_spot(prev);
            self.nav.skip = true;
            break;
        }
    }

    pub(super) fn go_forward(&mut self) {
        while let Some(next) = self.nav.fwd.pop() {
            if !self.nav_spot_exists(&next) {
                continue;
            }
            self.nav.back.push(self.nav_spot());
            self.show_nav_spot(next);
            self.nav.skip = true;
            break;
        }
    }

    // ---------------------------------------------------------------- top bar

    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        let t = self.t();
        egui::TopBottomPanel::top("top")
            .exact_height(40.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.app_bg)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ctx, |ui| {
                let full = ui.max_rect();
                // empty header space moves the window (there is no title bar)
                let drag = ui.interact(
                    full.expand(6.0),
                    ui.id().with("drag_header"),
                    Sense::click_and_drag(),
                );
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                }
                let mut left = if cfg!(target_os = "macos") { 74.0 } else { 4.0 };
                let compact = self.tier == Tier::Compact;
                if compact {
                    self.compact_bar(ui, full, full.left() + left);
                } else {
                    left += self.sheet_button(ui, pos2(full.left() + left, full.center().y));
                    // workspace picker (a narrow window keeps room for the search)
                    let max_name = if self.tier == Tier::Wide {
                        f32::INFINITY
                    } else {
                        140.0
                    };
                    let wr = self.workspace_pill(
                        ui,
                        pos2(full.left() + left, full.center().y),
                        max_name,
                    );
                    // planner shortcut
                    let cr = Rect::from_center_size(
                        pos2(wr.right() + 18.0, full.center().y),
                        vec2(28.0, 28.0),
                    );
                    let cresp = ui.interact(cr, ui.id().with("cal_btn"), Sense::click());
                    name_it(&cresp, "Planner shortcut");
                    if cresp.hovered() {
                        ui.painter().rect_filled(cr, 6.0, t.hover);
                    }
                    line_icon(ui.painter(), Icon::CalendarSmall, cr.center(), t.text2);
                    ui.painter().text(
                        cr.center() + vec2(0.0, 2.0),
                        Align2::CENTER_CENTER,
                        today().day().to_string(),
                        FontId::proportional(6.5),
                        t.text2,
                    );
                    if cresp.on_hover_text(tr("Planner")).clicked() {
                        self.section = Section::Planner;
                    }
                    // back / forward + search with AI Chats
                    let (sw, sx) = if self.tier == Tier::Wide {
                        (
                            287.0f32.min(full.width() * 0.25).max(220.0),
                            full.center().x + 40.0,
                        )
                    } else {
                        // narrow: between the planner button (and the arrows
                        // left of the search) and what sits at the right
                        let from = cr.right() + 12.0 + 52.0;
                        let mut reserve = 60.0 + 28.0 * self.data.extra.pinned_tools.len() as f32;
                        if self.server_trouble().is_some() {
                            reserve += 110.0;
                        }
                        if self.data.tasks.iter().any(|x| self.data.is_my_timer(x)) {
                            reserve += 150.0;
                        }
                        let to = full.right() - reserve;
                        let sw = (to - from).clamp(120.0, 220.0);
                        let lo = from + sw / 2.0;
                        (sw, (full.center().x + 40.0).min(to - sw / 2.0).max(lo))
                    };
                    // too narrow for the AI Chats chip inside it: AI is on the rail
                    let with_ai = sw >= 200.0;
                    let sr = Rect::from_center_size(pos2(sx, full.center().y), vec2(sw, 28.0));
                    for (i, (glyph, enabled, tip)) in [
                        ("\u{2190}", !self.nav.back.is_empty(), "Go back"),
                        ("\u{2192}", !self.nav.fwd.is_empty(), "Go forward"),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let ar = Rect::from_center_size(
                            pos2(sr.left() - 40.0 + i as f32 * 24.0, full.center().y),
                            vec2(24.0, 24.0),
                        );
                        let a = ui.interact(ar, ui.id().with(("nav", i)), Sense::click());
                        name_it(&a, tip);
                        if a.hovered() && enabled {
                            ui.painter().rect_filled(ar, 5.0, t.hover);
                        }
                        ui.painter().text(
                            ar.center(),
                            Align2::CENTER_CENTER,
                            glyph,
                            FontId::proportional(15.0),
                            if enabled {
                                t.text2
                            } else {
                                Color32::from_rgb(190, 190, 190)
                            },
                        );
                        if a.on_hover_text(tr(tip)).clicked() && enabled {
                            if i == 0 {
                                self.go_back();
                            } else {
                                self.go_forward();
                            }
                        }
                    }
                    let ai_r = if with_ai {
                        Rect::from_min_max(
                            pos2(sr.right() - 94.0, sr.top() + 4.0),
                            pos2(sr.right() - 4.0, sr.bottom() - 4.0),
                        )
                    } else {
                        Rect::from_min_max(sr.right_top(), sr.right_bottom())
                    };
                    let sresp = ui.interact(
                        Rect::from_min_max(sr.min, pos2(ai_r.left(), sr.bottom())),
                        ui.id().with("search_pill"),
                        Sense::click(),
                    );
                    name_it(&sresp, "Search");
                    ui.painter().rect_filled(
                        sr,
                        14.0,
                        if sresp.hovered() { t.hover } else { t.bg },
                    );
                    ui.painter().rect_stroke(
                        sr,
                        14.0,
                        Stroke::new(1.0_f32, t.border),
                        egui::StrokeKind::Inside,
                    );
                    super::icons::line_icon_scaled(
                        ui.painter(),
                        Icon::Search,
                        sr.left_center() + vec2(20.0, 0.0),
                        t.text2,
                        0.8,
                    );
                    let sg = ui.painter().layout_no_wrap(
                        tr("Search").into(),
                        FontId::proportional(14.0),
                        t.text3,
                    );
                    let sgw = sg.size().x;
                    ui.painter().galley(
                        pos2(sr.left() + 32.0, sr.center().y - sg.size().y / 2.0),
                        sg,
                        t.text3,
                    );
                    ui.painter().text(
                        pos2(sr.left() + 38.0 + sgw, sr.center().y),
                        Align2::LEFT_CENTER,
                        crate::os::keys("\u{2318}J"),
                        FontId::proportional(11.5),
                        Color32::from_rgb(170, 170, 170),
                    );
                    if sresp.clicked() {
                        self.palette = Some(String::new());
                    }
                    if with_ai {
                        let air = ui.interact(ai_r, ui.id().with("ai_chats"), Sense::click());
                        name_it(&air, "AI Chats");
                        ui.painter().rect_filled(
                            ai_r,
                            10.0,
                            if air.hovered() { t.hover } else { t.sidebar },
                        );
                        ui.painter().text(
                            ai_r.center() - vec2(8.0, 0.0),
                            Align2::CENTER_CENTER,
                            tr("AI Chats"),
                            FontId::proportional(13.0),
                            t.text2,
                        );
                        super::icons::line_icon_scaled(
                            ui.painter(),
                            Icon::Sparkle,
                            ai_r.right_center() - vec2(12.0, 0.0),
                            Color32::from_rgb(171, 74, 186),
                            0.75,
                        );
                        if air.clicked() {
                            self.section = Section::Ai;
                        }
                    }
                }
                // right part: pinned personal tools, avatar menu
                let mut right = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_max(
                            pos2(full.right() - 480.0, full.top()),
                            full.max,
                        ))
                        .layout(Layout::right_to_left(Align::Center)),
                );
                let ui = &mut right;
                ui.spacing_mut().item_spacing.x = 2.0;
                if let Some(m) = self.data.member(self.data.me).cloned() {
                    let (r, resp) = ui.allocate_exact_size(vec2(44.0, 30.0), Sense::click());
                    name_it(&resp, "Profile menu");
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 15.0, t.hover);
                    }
                    let c = r.left_center() + vec2(15.0, 0.0);
                    paint_avatar(ui.painter(), c, 12.0, &m, t.app_bg);
                    // orange: notifications muted; green / grey ring:
                    // connected to the team server or not; alone or in a
                    // shared folder there is no "online", so no dot
                    let muted = self.data.extra.muted_until > now_ts();
                    let dot = c + vec2(9.0, 9.0);
                    if muted {
                        ui.painter().circle_filled(dot, 4.0, t.app_bg);
                        ui.painter().circle_filled(dot, 2.8, crate::widgets::MUTED);
                    } else if self.on_server() {
                        crate::widgets::paint_presence(
                            ui.painter(),
                            dot,
                            2.8,
                            self.connected(),
                            t.app_bg,
                            t.text3,
                        );
                    }
                    if crate::updater::update_available() {
                        // a newer version is waiting: "Check for updates" in this menu
                        let d = c + vec2(9.0, -9.0);
                        ui.painter().circle_filled(d, 4.5, t.app_bg);
                        ui.painter().circle_filled(d, 3.5, t.danger);
                    }
                    let cv = r.right_center() - vec2(8.0, 1.0);
                    ui.painter().line_segment(
                        [cv + vec2(-3.0, -1.5), cv + vec2(0.0, 1.5)],
                        Stroke::new(1.2_f32, t.text2),
                    );
                    ui.painter().line_segment(
                        [cv + vec2(0.0, 1.5), cv + vec2(3.0, -1.5)],
                        Stroke::new(1.2_f32, t.text2),
                    );
                    if self.force_menu.as_deref() == Some("avatar") {
                        let pid = ui.id().with("avatar_menu");
                        ui.memory_mut(|m| m.open_popup(pid));
                    }
                    popup(ui, &resp, ui.id().with("avatar_menu"), 270.0, |ui| {
                        self.avatar_menu(ui)
                    });
                }
                // a team server that can't be reached (or won't let this
                // computer in): a quiet word here, Work together on click
                if let Some(word) = self.server_trouble().filter(|_| !compact) {
                    ui.add_space(4.0);
                    let g = ui.painter().layout_no_wrap(
                        word,
                        FontId::proportional(12.5),
                        super::together::WARN,
                    );
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(g.size().x + 26.0, 26.0), Sense::click());
                    name_it(&resp, "Server connection");
                    if resp.hovered() {
                        ui.painter().rect_filled(r, 13.0, t.hover);
                    }
                    ui.painter().circle_filled(
                        r.left_center() + vec2(11.0, 0.0),
                        3.5,
                        super::together::WARN,
                    );
                    ui.painter().galley(
                        pos2(r.left() + 19.0, r.center().y - g.size().y / 2.0),
                        g,
                        super::together::WARN,
                    );
                    let tip = super::together::net_problem(self.sync_rt.net_status.as_ref())
                        .unwrap_or_default();
                    if resp
                        .on_hover_text(format!("{}\n{}", tip, tr("Click for Work together.")))
                        .clicked()
                    {
                        self.modal = Some(self.together_modal());
                    }
                }
                ui.add_space(6.0);
                // a compact window has room for the avatar only
                let pinned = if compact {
                    vec![]
                } else {
                    self.data.extra.pinned_tools.clone()
                };
                for id in pinned.iter().rev() {
                    if let Some((tid, label, icon)) = super::menus::TOOLS.iter().find(|x| x.0 == id)
                    {
                        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                        name_it(&resp, label);
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, t.hover);
                        }
                        // TOOLS names each tool's line icon
                        line_icon(ui.painter(), *icon, r.center(), t.text2);
                        if resp.on_hover_text(tr(label)).clicked() {
                            self.run_tool(tid);
                        }
                    }
                }
                let now = now_ts();
                if let Some(task) = self
                    .data
                    .tasks
                    .iter()
                    .find(|x| self.data.is_my_timer(x))
                    .filter(|_| !compact)
                {
                    // room at the start for the stop square drawn over it
                    let txt = format!(
                        "     {}  {}",
                        fmt_duration(self.data.my_tracked_secs(task, now)),
                        truncate(&task.name, if self.tier == Tier::Wide { 22 } else { 10 })
                    );
                    let b = Tint(
                        egui::Button::new(RichText::new(txt).color(Color32::WHITE))
                            .fill(t.danger)
                            .corner_radius(14),
                    );
                    let id = task.id;
                    ui.add_space(8.0);
                    let br = ui.add(b);
                    super::icons::line_icon_scaled(
                        ui.painter(),
                        Icon::Stop,
                        br.rect.left_center() + vec2(15.0, 0.0),
                        Color32::WHITE,
                        0.6,
                    );
                    if br.on_hover_text(tr("Stop timer")).clicked() {
                        self.actions.push(Action::ToggleTimer(id));
                    }
                    ctx.request_repaint_after(std::time::Duration::from_secs(1));
                }
            });
    }

    /// The workspace's pill in the top bar, its left middle at `at`, the
    /// name shortened to `max_name` (infinite: whole), with its menu.
    pub(super) fn workspace_pill(&mut self, ui: &mut Ui, at: Pos2, max_name: f32) -> Rect {
        let t = self.t();
        let mut name = self.data.workspace.clone();
        if max_name.is_finite() {
            name = fit_text(ui.painter(), &name, &FontId::proportional(14.0), max_name);
        }
        let g = ui
            .painter()
            .layout_no_wrap(name, FontId::proportional(14.0), t.text);
        let wr = Rect::from_min_size(pos2(at.x, at.y - 14.0), vec2(g.size().x + 58.0, 28.0));
        let wresp = ui.interact(wr, ui.id().with("ws_pill"), Sense::click());
        name_it(&wresp, "Workspace menu");
        ui.painter()
            .rect_filled(wr, 7.0, if wresp.hovered() { t.hover } else { t.sidebar });
        let av = Rect::from_center_size(wr.left_center() + vec2(15.0, 0.0), vec2(18.0, 18.0));
        ui.painter()
            .rect_filled(av, 4.0, rgb(self.data.extra.workspace_color));
        let letter = self
            .data
            .workspace
            .chars()
            .next()
            .unwrap_or('W')
            .to_uppercase()
            .to_string();
        ui.painter().text(
            av.center(),
            Align2::CENTER_CENTER,
            letter,
            FontId::proportional(11.0),
            Color32::WHITE,
        );
        ui.painter().galley(
            pos2(wr.left() + 30.0, wr.center().y - g.size().y / 2.0),
            g,
            t.text,
        );
        let cv = wr.right_center() - vec2(14.0, 1.0);
        ui.painter().line_segment(
            [cv + vec2(-4.0, -2.0), cv + vec2(0.0, 2.0)],
            Stroke::new(1.3_f32, t.text2),
        );
        ui.painter().line_segment(
            [cv + vec2(0.0, 2.0), cv + vec2(4.0, -2.0)],
            Stroke::new(1.3_f32, t.text2),
        );
        if self.force_menu.as_deref() == Some("workspace") {
            let pid = ui.id().with("ws_menu");
            ui.memory_mut(|m| m.open_popup(pid));
        }
        popup(ui, &wresp, ui.id().with("ws_menu"), 300.0, |ui| {
            self.workspace_menu(ui)
        });
        wr
    }

    // ---------------------------------------------------------------- rail

    /// The red counts on the rail's Home and Chat: the Inbox badge, and the
    /// unread messages the sidebar shows. While Chat isn't pinned to the rail
    /// it lives under Home, so Home carries both. Nothing while notifications
    /// are paused, as in the sidebar.
    pub(super) fn rail_unread(&self, chat_pinned: bool) -> (usize, usize) {
        if self.data.extra.muted_until > now_ts() {
            return (0, 0);
        }
        let (inbox, chat) = (self.inbox_badge(), self.chat_unread_total());
        if chat_pinned {
            (inbox, chat)
        } else {
            (inbox + chat, 0)
        }
    }

    /// The rail's items from the top: Home and the pinned apps. Home is
    /// first unless it was dragged further down, the only time it is kept
    /// in `rail_items`.
    pub(super) fn rail_order(&self) -> Vec<String> {
        let pinned = &self.data.extra.rail_items;
        let mut v = Vec::with_capacity(pinned.len() + 1);
        if !pinned.iter().any(|r| r == "Home") {
            v.push("Home".to_string());
        }
        v.extend(pinned.iter().cloned());
        v
    }

    /// Moves the rail item at `from` to `to`, both places in `rail_order`.
    pub(super) fn move_rail_item(&mut self, from: usize, to: usize) {
        let mut order = self.rail_order();
        if from == to || from >= order.len() || to >= order.len() {
            return;
        }
        let item = order.remove(from);
        order.insert(to, item);
        if order.first().map(String::as_str) == Some("Home") {
            order.remove(0);
        }
        self.data.extra.rail_items = order;
        self.dirty = true;
    }

    /// Whether Ctrl+1..9 go to the rail's items now. Not while a dialog, the
    /// command palette or quick add is up, nor while a task is open: there
    /// Cmd+1..4 set its priority, and off the Mac Cmd is the Ctrl key.
    pub(super) fn rail_keys_on(&self) -> bool {
        self.modal.is_none()
            && self.palette.is_none()
            && self.quick_add.is_none()
            && self.open_task.is_none()
    }

    pub(super) fn rail(&mut self, ctx: &egui::Context) {
        // a compact window is one column: its home screen lists the apps
        if self.tier == Tier::Compact {
            return;
        }
        let t = self.t();
        egui::SidePanel::left("rail")
            .exact_width(58.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(t.app_bg)
                    .inner_margin(egui::Margin {
                        left: 6,
                        right: 0,
                        top: 0,
                        bottom: 6,
                    }),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let p = ui.painter().clone();
                if t.rail_top == t.rail_bottom {
                    p.rect_filled(rect, 8.0, t.rail_top);
                } else {
                    p.rect_filled(rect, 8.0, t.rail_bottom);
                    let mut mesh = egui::Mesh::default();
                    let m = rect.shrink2(vec2(0.0, 8.0));
                    mesh.colored_vertex(m.left_top(), t.rail_top);
                    mesh.colored_vertex(m.right_top(), t.rail_top);
                    mesh.colored_vertex(m.right_bottom(), t.rail_bottom);
                    mesh.colored_vertex(m.left_bottom(), t.rail_bottom);
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    p.rect_filled(
                        Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 16.0)),
                        8.0,
                        t.rail_top,
                    );
                    p.add(egui::Shape::mesh(mesh));
                }
                let cx = rect.center().x;
                let white = Color32::WHITE;
                // bottom group: Invite (every 56 px), only for whoever may
                // invite; Labs, Upgrade and Install removed at the person's
                // request. It stays at the bottom whatever the bar holds.
                let bottom: &[(&str, &str)] = if self.may_manage() {
                    &[("Invite", "")]
                } else {
                    &[]
                };
                let bottom_top = rect.bottom() - bottom.len() as f32 * 56.0 - 2.0;
                let items = self.rail_order();
                let section_of = |l: &str| -> Option<Section> {
                    match l {
                        "Home" => Some(Section::Home),
                        "Planner" => Some(Section::Planner),
                        "AI" => Some(Section::Ai),
                        "Teams" => Some(Section::Teams),
                        "Chat" => Some(Section::Chat),
                        "Docs" => Some(Section::Docs),
                        "Dashboards" => Some(Section::Dashboards),
                        "Whiteboards" => Some(Section::Whiteboards),
                        "Forms" => Some(Section::Forms),
                        "Goals" => Some(Section::Goals),
                        "Timesheets" => Some(Section::Timesheets),
                        _ => None,
                    }
                };
                let pinned_sections: Vec<Section> =
                    items.iter().filter_map(|l| section_of(l)).collect();
                // an app opened from More stays in the bar while it's open
                let temp = self.temp_app.and_then(|ts| {
                    let label = match ts {
                        Section::Docs => "Docs",
                        Section::Dashboards => "Dashboards",
                        Section::Whiteboards => "Whiteboards",
                        Section::Forms => "Forms",
                        Section::Goals => "Goals",
                        Section::Timesheets => "Timesheets",
                        _ => "",
                    };
                    (!label.is_empty() && !pinned_sections.contains(&ts)).then_some((ts, label))
                });
                // (label, the app opened from More)
                let mut rows: Vec<(String, Option<Section>)> =
                    items.into_iter().map(|l| (l, None)).collect();
                if let Some((ts, label)) = temp {
                    rows.push((label.to_string(), Some(ts)));
                }
                // every item, 62 px apart (60 tall), then More. When they
                // don't all fit above the bottom group, More stays just above
                // it and the items scroll with the wheel behind it; those
                // scrolled out of view are also in More's menu.
                let top = rect.top() + 4.0;
                let more_y = (top + rows.len() as f32 * RAIL_STEP).min(bottom_top - 62.0);
                let view = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), more_y));
                let scroll_key = ui.id().with("rail_scroll");
                let max_scroll = (rows.len() as f32 * RAIL_STEP - 2.0 - view.height()).max(0.0);
                let mut scroll = ui.data(|d| d.get_temp::<f32>(scroll_key)).unwrap_or(0.0);
                let any_drag = rows
                    .iter()
                    .any(|(l, _)| ctx.is_being_dragged(ui.id().with(("rail", l.as_str()))));
                if max_scroll > 0.0 && !any_drag && ui.rect_contains_pointer(view) {
                    let dy = ctx.input(|i| i.smooth_scroll_delta.y);
                    if dy != 0.0 {
                        scroll -= dy;
                        ctx.input_mut(|i| i.smooth_scroll_delta.y = 0.0);
                    }
                }
                let scroll = scroll.clamp(0.0, max_scroll);
                ui.data_mut(|d| d.insert_temp(scroll_key, scroll));
                let slot_y = |i: usize| top + i as f32 * RAIL_STEP - scroll;
                let hidden: Vec<String> = rows
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        slot_y(*i) < view.top() - 1.0 || slot_y(*i) + 60.0 > view.bottom() + 1.0
                    })
                    .map(|(_, (l, _))| l.clone())
                    .collect();
                let rail_id = ui.id();
                let item_id = |l: &str| rail_id.with(("rail", l));
                // an item dragged up or down the bar floats under the mouse
                // while the others move aside to leave its slot open; let go,
                // it takes that slot. The pinned items shown come first in
                // `rows`, in the bar's order; the app opened from More and
                // those hidden in More's menu stay where they are
                let pinned_shown = rows.iter().take_while(|(_, tmp)| tmp.is_none()).count();
                let grab_key = ui.id().with("rail_grab");
                let dragged = rows[..pinned_shown]
                    .iter()
                    .position(|(l, _)| ctx.is_being_dragged(item_id(l)));
                let dropped = rows[..pinned_shown]
                    .iter()
                    .position(|(l, _)| ctx.drag_stopped_id() == Some(item_id(l)));
                let lift = dragged.or(dropped).and_then(|from| {
                    // how far below the item's top it was picked up, kept
                    // for the whole drag so it doesn't jump to the mouse
                    let grab = match ui.data(|d| d.get_temp::<f32>(grab_key)) {
                        Some(g) => g,
                        None => {
                            let g = ctx.input(|i| i.pointer.press_origin())?.y - slot_y(from);
                            ui.data_mut(|d| d.insert_temp(grab_key, g));
                            g
                        }
                    };
                    let py = ctx.input(|i| i.pointer.latest_pos())?.y;
                    // within the items, and within the part of the bar in view
                    let lo = slot_y(0).max(view.top());
                    let hi = slot_y(pinned_shown - 1).min(view.bottom() - 60.0).max(lo);
                    let at = (py - grab).clamp(lo, hi);
                    let to =
                        (((at - slot_y(0)) / RAIL_STEP).round() as usize).min(pinned_shown - 1);
                    Some(RailLift { from, at, to })
                });
                if dragged.is_none() {
                    ui.data_mut(|d| d.remove::<f32>(grab_key));
                }
                let lift = match lift {
                    Some(l) if dropped.is_some() => {
                        self.move_rail_item(l.from, l.to);
                        // the same items, a new order among them
                        for ((label, _), now) in rows.iter_mut().zip(self.rail_order()) {
                            *label = now;
                        }
                        None
                    }
                    l => l,
                };
                // the items are drawn and clicked only in the part in view
                let full_clip = ui.clip_rect();
                ui.set_clip_rect(view.intersect(full_clip));
                let full_p = p.clone();
                let p = full_p.with_clip_rect(view.intersect(full_clip));
                // the order drawn: the dragged item taken out and put back
                // at the slot it would drop into
                let mut drawn: Vec<usize> = (0..rows.len()).collect();
                if let Some(l) = lift {
                    let ix = drawn.remove(l.from);
                    drawn.insert(l.to, ix);
                    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                    // the open slot: where the item lands when let go
                    let r = Rect::from_min_size(
                        pos2(rect.left() + 3.0, slot_y(l.to)),
                        vec2(rect.width() - 6.0, 60.0),
                    );
                    p.rect_filled(r, 10.0, Color32::from_white_alpha(18));
                    p.rect_stroke(
                        r,
                        10.0,
                        Stroke::new(1.0_f32, Color32::from_white_alpha(110)),
                        egui::StrokeKind::Inside,
                    );
                }
                // red counts on Home (the Inbox) and Chat (unread messages);
                // Chat counts on Home while it isn't in the bar of its own,
                // and on More while it's pinned but hidden there
                let (home_n, chat_n) = self.rail_unread(pinned_sections.contains(&Section::Chat));
                // Ctrl+1..9 open the bar's items from the top (Control on
                // the Mac, not Cmd); holding Ctrl shows each item's number at
                // the corner of its icon
                let keys_on = self.rail_keys_on();
                let hints = keys_on && ctx.input(|i| i.modifiers.ctrl);
                let mut slot = 0usize;
                let mut next_key = |p: &egui::Painter, ibox: Rect| -> bool {
                    if !keys_on || slot >= RAIL_KEYS.len() {
                        return false;
                    }
                    let key = RAIL_KEYS[slot];
                    slot += 1;
                    if hints {
                        paint_key_hint(p, ibox, slot);
                    }
                    ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, key))
                };
                for (slot, &ix) in drawn.iter().enumerate() {
                    let (label, temp) = &rows[ix];
                    let label = label.as_str();
                    let lifted = lift.filter(|l| l.from == ix);
                    // the others glide to their places while an item is
                    // dragged; otherwise the slot is kept in step so the next
                    // drag starts from where they are
                    let y_id = ui.id().with(("rail_y", label));
                    let y = match lifted {
                        Some(l) => {
                            ctx.animate_value_with_time(y_id, l.at, 0.0);
                            l.at
                        }
                        None if lift.is_some() => {
                            ctx.animate_value_with_time(y_id, slot_y(slot), 0.12)
                        }
                        None => {
                            ctx.animate_value_with_time(y_id, slot_y(slot), 0.0);
                            slot_y(slot)
                        }
                    };
                    let sec = section_of(label);
                    let active = match sec {
                        Some(Section::Home) => {
                            matches!(
                                self.section,
                                Section::Home | Section::Posts | Section::Meetings
                            ) || (self.section == Section::Chat
                                && !pinned_sections.contains(&Section::Chat))
                        }
                        Some(s) => s == self.section,
                        None => false,
                    };
                    let ibox = Rect::from_center_size(pos2(cx, y + 22.0), vec2(32.0, 32.0));
                    let hit =
                        Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                    let resp = match temp {
                        Some(_) => ui.interact(hit, ui.id().with("rail_temp"), Sense::click()),
                        None => ui.interact(hit, item_id(label), Sense::click_and_drag()),
                    };
                    name_it(&resp, &format!("Rail {}", label));
                    // the dragged item is drawn above the bar, on a raised
                    // tile of the bar's color
                    let p = if lifted.is_some() {
                        let p = ctx.layer_painter(egui::LayerId::new(
                            egui::Order::Tooltip,
                            resp.id.with("lift"),
                        ));
                        let tile = Rect::from_min_size(
                            pos2(rect.left() + 3.0, y),
                            vec2(rect.width() - 6.0, 60.0),
                        );
                        let shadow = egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 14,
                            spread: 0,
                            color: Color32::from_black_alpha(110),
                        };
                        p.add(shadow.as_shape(tile, 10.0));
                        p.rect_filled(tile, 10.0, t.rail_top.lerp_to_gamma(white, 0.16));
                        p
                    } else {
                        p.clone()
                    };
                    if active {
                        p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                    } else if resp.hovered() && lift.is_none() {
                        p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                    }
                    line_icon(&p, rail_icon(label), ibox.center(), white);
                    let pressed = next_key(&p, ibox);
                    let shown = tr(label);
                    let short = if temp.is_some() && shown.chars().count() > 8 {
                        format!("{}..", shown.chars().take(7).collect::<String>())
                    } else {
                        shown.to_string()
                    };
                    p.text(
                        pos2(cx, ibox.bottom() + 10.0),
                        Align2::CENTER_CENTER,
                        short,
                        FontId::proportional(11.0),
                        white,
                    );
                    let unread = match sec {
                        Some(Section::Home) => home_n,
                        Some(Section::Chat) => chat_n,
                        _ => 0,
                    };
                    if unread > 0 {
                        paint_rail_badge(&p, t.danger, t.rail_top, ibox.center(), unread);
                    }
                    if lift.is_none() {
                        rail_tip(&resp, rect, ibox, &shown);
                    }
                    if resp.clicked() || pressed {
                        match temp {
                            Some(ts) => self.section = *ts,
                            None => {
                                self.open_app(label);
                            }
                        }
                    }
                }
                ui.set_clip_rect(full_clip);
                let p = full_p;
                let y = more_y;
                // More
                let ibox = Rect::from_center_size(pos2(cx, y + 22.0), vec2(32.0, 32.0));
                let hit = Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                let resp = ui.interact(hit, ui.id().with("rail_more"), Sense::click());
                name_it(&resp, "Rail More");
                let more_active = (matches!(
                    self.section,
                    Section::Docs
                        | Section::Dashboards
                        | Section::Timesheets
                        | Section::Whiteboards
                        | Section::Forms
                        | Section::Goals
                ) && !pinned_sections.contains(&self.section)
                    && self.temp_app != Some(self.section))
                    || hidden.iter().any(|l| section_of(l) == Some(self.section));
                if more_active {
                    p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                } else if resp.hovered() {
                    p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                }
                line_icon(&p, Icon::More, ibox.center(), white);
                p.text(
                    pos2(cx, ibox.bottom() + 10.0),
                    Align2::CENTER_CENTER,
                    tr("More"),
                    FontId::proportional(11.0),
                    white,
                );
                if chat_n > 0 && hidden.iter().any(|l| l == "Chat") {
                    paint_rail_badge(&p, t.danger, t.rail_top, ibox.center(), chat_n);
                }
                rail_tip(&resp, rect, ibox, tr("More"));
                if self.force_menu.as_deref() == Some("more") {
                    let pid = ui.id().with("more_pop");
                    ui.memory_mut(|m| m.open_popup(pid));
                }
                popup_right(ui, &resp, ui.id().with("more_pop"), 220.0, |ui| {
                    self.rail_more_menu(ui, &hidden)
                });
                let mut by = bottom_top;
                for &(label, icon) in bottom {
                    let ibox = Rect::from_center_size(pos2(cx, by + 16.0), vec2(32.0, 32.0));
                    let hit = Rect::from_min_max(
                        pos2(rect.left(), by - 2.0),
                        pos2(rect.right(), by + 52.0),
                    );
                    let resp = ui.interact(hit, ui.id().with(("railb", label)), Sense::click());
                    name_it(&resp, &format!("Rail {}", label));
                    if resp.hovered() {
                        p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                    }
                    if label == "Invite" {
                        line_icon(
                            &p,
                            Icon::PersonSmall,
                            ibox.center() + vec2(-1.0, -1.0),
                            white,
                        );
                        super::icons::paint_plus(&p, ibox.center() + vec2(6.0, 4.0), 10.0, white);
                    } else if let Some(i) = Icon::named(&label.to_lowercase()) {
                        line_icon(&p, i, ibox.center(), white);
                    } else {
                        p.text(
                            ibox.center(),
                            Align2::CENTER_CENTER,
                            icon,
                            FontId::proportional(16.0),
                            white,
                        );
                    }
                    p.text(
                        pos2(cx, ibox.bottom() + 10.0),
                        Align2::CENTER_CENTER,
                        tr(label),
                        FontId::proportional(11.0),
                        white,
                    );
                    rail_tip(&resp, rect, ibox, tr(label));
                    if resp.clicked() {
                        self.modal = Some(Modal::Invite {
                            emails: String::new(),
                            role: "Member".into(),
                        });
                    }
                    by += 56.0;
                }
            });
    }
}
