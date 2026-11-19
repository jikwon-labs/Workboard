// The window shell measured from the person's running ClickUp: black icon
// rail, light top bar with back/forward and search, the Home sidebar (with
// Channels and Direct Messages; sidebar.rs, side_tree.rs) and the page header
// with coloured view tabs and pill toolbar (header.rs). The List and Board
// views are in listview.rs and board.rs. See MECHANICS.md.

use super::*;
use chrono::Datelike;
use egui::{Pos2, Response};

mod header;
mod side_tree;
mod sidebar;

pub(super) use super::icons::line_icon;
use super::icons::{paint_calendar_icon, paint_checklist_icon, paint_person_icon, paint_view_icon};

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
                // workspace picker
                let name = self.data.workspace.clone();
                let g = ui
                    .painter()
                    .layout_no_wrap(name, FontId::proportional(14.0), t.text);
                let left = if cfg!(target_os = "macos") { 74.0 } else { 4.0 };
                let wr = Rect::from_min_size(
                    pos2(full.left() + left, full.center().y - 14.0),
                    vec2(g.size().x + 58.0, 28.0),
                );
                let wresp = ui.interact(wr, ui.id().with("ws_pill"), Sense::click());
                name_it(&wresp, "Workspace menu");
                ui.painter().rect_filled(
                    wr,
                    7.0,
                    if wresp.hovered() { t.hover } else { t.sidebar },
                );
                let av =
                    Rect::from_center_size(wr.left_center() + vec2(15.0, 0.0), vec2(18.0, 18.0));
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
                paint_calendar_icon(ui.painter(), cr.center(), t.text2);
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
                let sw = 287.0f32.min(full.width() * 0.25).max(220.0);
                let sr = Rect::from_center_size(full.center() + vec2(40.0, 0.0), vec2(sw, 28.0));
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
                let ai_r = Rect::from_min_max(
                    pos2(sr.right() - 94.0, sr.top() + 4.0),
                    pos2(sr.right() - 4.0, sr.bottom() - 4.0),
                );
                let sresp = ui.interact(
                    Rect::from_min_max(sr.min, pos2(ai_r.left(), sr.bottom())),
                    ui.id().with("search_pill"),
                    Sense::click(),
                );
                name_it(&sresp, "Search");
                ui.painter()
                    .rect_filled(sr, 14.0, if sresp.hovered() { t.hover } else { t.bg });
                ui.painter().rect_stroke(
                    sr,
                    14.0,
                    Stroke::new(1.0_f32, t.border),
                    egui::StrokeKind::Inside,
                );
                super::icons::line_icon_scaled(
                    ui.painter(),
                    "search",
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
                    "sparkle",
                    ai_r.right_center() - vec2(12.0, 0.0),
                    Color32::from_rgb(171, 74, 186),
                    0.75,
                );
                if air.clicked() {
                    self.section = Section::Ai;
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
                if let Some(word) = self.server_trouble() {
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
                let pinned = self.data.extra.pinned_tools.clone();
                for id in pinned.iter().rev() {
                    if let Some((tid, label, icon)) = super::menus::TOOLS.iter().find(|x| x.0 == id)
                    {
                        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
                        name_it(&resp, label);
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, t.hover);
                        }
                        // TOOLS names each tool's line icon
                        line_icon(ui.painter(), icon, r.center(), t.text2);
                        if resp.on_hover_text(tr(label)).clicked() {
                            self.run_tool(tid);
                        }
                    }
                }
                let now = now_ts();
                if let Some(task) = self.data.tasks.iter().find(|x| self.data.is_my_timer(x)) {
                    // room at the start for the stop square drawn over it
                    let txt = format!(
                        "     {}  {}",
                        fmt_duration(self.data.my_tracked_secs(task, now)),
                        truncate(&task.name, 22)
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
                        "stop",
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

    pub(super) fn rail(&mut self, ctx: &egui::Context) {
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
                let mut items: Vec<String> = vec!["Home".into()];
                items.extend(self.data.extra.rail_items.iter().cloned());
                let mut y = rect.top() + 4.0;
                // the line icon of each item the rail can hold
                let icon_of = |l: &str| -> &'static str {
                    match l {
                        "Home" => "home",
                        "Planner" => "planner",
                        "AI" => "ai",
                        "Teams" => "teams",
                        "Chat" => "chat",
                        "Docs" => "docs",
                        "Dashboards" => "dashboards",
                        "Whiteboards" => "whiteboards",
                        "Forms" => "forms",
                        "Goals" => "goals",
                        "Timesheets" => "timesheets",
                        "Clips" => "video",
                        "Apps" => "apps",
                        "Spaces" => "space",
                        _ => "dots",
                    }
                };
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
                // red counts on Home (the Inbox) and Chat (unread messages);
                // Chat counts on Home while it isn't in the bar of its own
                let (home_n, chat_n) = self.rail_unread(pinned_sections.contains(&Section::Chat));
                for label in &items {
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
                    let resp =
                        ui.interact(hit, ui.id().with(("rail", label.as_str())), Sense::click());
                    name_it(&resp, &format!("Rail {}", label));
                    if active {
                        p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                    } else if resp.hovered() {
                        p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                    }
                    line_icon(&p, icon_of(label), ibox.center(), white);
                    p.text(
                        pos2(cx, ibox.bottom() + 10.0),
                        Align2::CENTER_CENTER,
                        tr(label),
                        FontId::proportional(11.0),
                        white,
                    );
                    let unread = match sec {
                        Some(Section::Home) => home_n,
                        Some(Section::Chat) => chat_n,
                        _ => 0,
                    };
                    if unread > 0 {
                        paint_unread_badge(
                            &p,
                            t.danger,
                            ibox.center() + vec2(4.0, -16.0),
                            Align2::LEFT_TOP,
                            unread,
                        );
                    }
                    if resp.on_hover_text(tr(label)).clicked() {
                        self.open_app(label);
                        if label == "Home" {
                            self.section = Section::Home;
                        }
                    }
                    y += 62.0;
                }
                // an app opened from More stays in the bar while it's open
                if let Some(ts) = self.temp_app {
                    let label = match ts {
                        Section::Docs => "Docs",
                        Section::Dashboards => "Dashboards",
                        Section::Whiteboards => "Whiteboards",
                        Section::Forms => "Forms",
                        Section::Goals => "Goals",
                        Section::Timesheets => "Timesheets",
                        _ => "",
                    };
                    if !label.is_empty() && !pinned_sections.contains(&ts) {
                        let ibox = Rect::from_center_size(pos2(cx, y + 22.0), vec2(32.0, 32.0));
                        let hit =
                            Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                        let resp = ui.interact(hit, ui.id().with("rail_temp"), Sense::click());
                        let active = self.section == ts;
                        if active {
                            p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                        } else if resp.hovered() {
                            p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                        }
                        line_icon(&p, icon_of(label), ibox.center(), white);
                        let shown = tr(label);
                        let short = if shown.chars().count() > 8 {
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
                        if resp.on_hover_text(tr(label)).clicked() {
                            self.section = ts;
                        }
                        y += 62.0;
                    }
                }
                // More
                let ibox = Rect::from_center_size(pos2(cx, y + 22.0), vec2(32.0, 32.0));
                let hit = Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + 60.0));
                let resp = ui.interact(hit, ui.id().with("rail_more"), Sense::click());
                name_it(&resp, "Rail More");
                let more_active = matches!(
                    self.section,
                    Section::Docs
                        | Section::Dashboards
                        | Section::Timesheets
                        | Section::Whiteboards
                        | Section::Forms
                        | Section::Goals
                ) && !pinned_sections.contains(&self.section)
                    && self.temp_app != Some(self.section);
                if more_active {
                    p.circle_filled(ibox.center(), 17.0, Color32::from_white_alpha(46));
                } else if resp.hovered() {
                    p.rect_filled(ibox, 8.0, Color32::from_white_alpha(30));
                }
                line_icon(&p, "more", ibox.center(), white);
                p.text(
                    pos2(cx, ibox.bottom() + 10.0),
                    Align2::CENTER_CENTER,
                    tr("More"),
                    FontId::proportional(11.0),
                    white,
                );
                if self.force_menu.as_deref() == Some("more") {
                    let pid = ui.id().with("more_pop");
                    ui.memory_mut(|m| m.open_popup(pid));
                }
                popup(ui, &resp, ui.id().with("more_pop"), 220.0, |ui| {
                    self.rail_more_menu(ui)
                });
                // bottom group: Invite (every 56 px), only for whoever may
                // invite; Labs, Upgrade and Install removed at the person's request
                let bottom: &[(&str, &str)] = if self.may_manage() {
                    &[("Invite", "")]
                } else {
                    &[]
                };
                let mut by = rect.bottom() - bottom.len() as f32 * 56.0 - 2.0;
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
                        paint_person_icon(&p, ibox.center() + vec2(-1.0, -1.0), white);
                        super::icons::paint_plus(&p, ibox.center() + vec2(6.0, 4.0), 10.0, white);
                    } else if !line_icon(&p, &label.to_lowercase(), ibox.center(), white) {
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
                    if resp.on_hover_text(tr(label)).clicked() {
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
