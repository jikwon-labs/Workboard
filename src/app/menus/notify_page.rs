// The Notifications settings, a Settings section and a dialog from the
// Inbox: how notices arrive (with a preview of a banner and the Dock icon),
// then what is notified, in cards by subject. Each row is one of the
// person's NotifySettings; the row turns it on and off as a whole.

use super::*;
use crate::app::icons::{line_icon_scaled, paint_minus, paint_plus};
use chrono::NaiveTime;
use egui::Pos2;

/// Where a row's text starts: after the 16 px margin, the icon tile and a gap.
const TEXT_X: f32 = 16.0 + ICON_TILE + 12.0;
const ICON_TILE: f32 = 30.0;
/// The space right of a row's switch.
const ROW_RIGHT: f32 = 14.0;
const CARD_RADIUS: u8 = 12;

/// The colours this page adds to the theme's.
struct Tones {
    /// behind the preview at the top of the first card
    well: Color32,
    /// the preview banner
    banner: Color32,
    /// the square behind each row's icon
    tile: Color32,
    /// the channel choices: their track and the chosen one
    seg: Color32,
    seg_on: Color32,
}

fn tones(t: &Theme) -> Tones {
    if t.dark {
        Tones {
            well: t.column_bg,
            banner: Color32::from_gray(47),
            tile: t.hover,
            seg: t.app_bg,
            seg_on: Color32::from_gray(58),
        }
    } else {
        Tones {
            well: Color32::from_gray(246),
            banner: Color32::WHITE,
            tile: Color32::from_gray(242),
            seg: Color32::from_gray(240),
            seg_on: Color32::WHITE,
        }
    }
}

/// A row's optional parts besides its name and switch.
#[derive(Default)]
struct RowOpts {
    /// a line under the name, for what the name doesn't say
    desc: Option<&'static str>,
    /// a small label after the name ("Admins only")
    badge: Option<&'static str>,
    /// room kept left of the switch for controls the row adds
    extra_w: f32,
}

impl App {
    /// The Notifications dialog. Returns true when it should close.
    pub(super) fn notifications_body(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        // in Settings the page scrolls; in the dialog the list does.
        // (A list scrolling inside the scrolling page cut off its last button.)
        // the dialog shows as much as a short window allows, up to 620:
        // the preview card alone is a third of that
        let (max_h, min_h) = if App::in_settings_page(ui) {
            (f32::INFINITY, 0.0)
        } else {
            let h = (ui.ctx().screen_rect().height() - 200.0).max(240.0);
            (h, h.min(620.0))
        };
        egui::ScrollArea::vertical()
            .id_salt("notify_settings")
            .max_height(max_h)
            .min_scrolled_height(min_h)
            .auto_shrink([false, true])
            .show(ui, |ui| self.notification_cards(ui, t));
        false
    }

    fn notification_cards(&mut self, ui: &mut Ui, t: &Theme) {
        let persist = self.persist;
        let manages = self.i_manage();
        let tn = tones(t);
        let (mut ch, mut test, mut open_settings) = (false, false, false);
        let mut while_quit = !self.push_rt.off;
        let while_quit_was = while_quit;
        let n = &mut self.data.extra.notify;
        ui.spacing_mut().item_spacing.y = 0.0;

        section_title(ui, t, "How you're notified");
        card(ui, t, |ui| {
            preview(ui, t, &tn, n);
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Bell,
                "Desktop notifications",
                &mut n.desktop,
                false,
                RowOpts {
                    desc: Some("When off, notifications show only inside Workboard"),
                    ..Default::default()
                },
                |_, _| false,
            );
            // the team server's pushes through Apple (push.rs): the Mac only
            if cfg!(target_os = "macos") {
                setting_row(
                    ui,
                    t,
                    &tn,
                    Icon::Moon,
                    "Notify me while Workboard is quit",
                    &mut while_quit,
                    true,
                    RowOpts {
                        desc: Some("The team server sends banners even when the app isn't running"),
                        ..Default::default()
                    },
                    |_, _| false,
                );
            }
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Volume,
                "Notification sound",
                &mut n.sound,
                true,
                RowOpts::default(),
                |_, _| false,
            );
            let badge = crate::os::mac_or_pc(
                "Show the unread count on the Dock icon",
                "Show the unread count on the taskbar icon",
            );
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Badge,
                badge,
                &mut n.dock_badge,
                true,
                RowOpts::default(),
                |_, _| false,
            );
            let (o, s) = delivery_footer(ui, t);
            (open_settings, test) = (o, s);
        });

        section_gap(ui);
        section_title(ui, t, "Inbox");
        caption(ui, t, "Notified and kept in your Inbox");
        card(ui, t, |ui| {
            let rows: [(Icon, &str, &mut bool); 5] = [
                (Icon::Chat, "Comments on my tasks", &mut n.comments),
                (Icon::At, "Mentions", &mut n.mentions),
                (Icon::Person, "Tasks assigned to me", &mut n.assigned),
                (Icon::Alarm, "Reminders", &mut n.reminders),
                (Icon::Inbox, "Unread chat count", &mut n.chat),
            ];
            for (i, (icon, title, on)) in rows.into_iter().enumerate() {
                ch |= setting_row(
                    ui,
                    t,
                    &tn,
                    icon,
                    title,
                    on,
                    i > 0,
                    RowOpts::default(),
                    |_, _| false,
                );
            }
        });

        section_gap(ui);
        divider_label(ui, t, "Notified only");
        section_gap(ui);
        section_title(ui, t, "Chat");
        card(ui, t, |ui| {
            ch |= channel_choice(ui, t, &tn, n);
            let rows: [(Icon, &str, &mut bool); 4] = [
                (Icon::PersonComment, "Direct messages", &mut n.direct),
                (Icon::Reply, "Replies in threads I'm in", &mut n.replies),
                (Icon::Smile, "Reactions to my messages", &mut n.reactions),
                (Icon::Phone, "Huddles and calls", &mut n.huddles),
            ];
            for (icon, title, on) in rows {
                ch |= setting_row(
                    ui,
                    t,
                    &tn,
                    icon,
                    title,
                    on,
                    true,
                    RowOpts::default(),
                    |_, _| false,
                );
            }
            ch |= keywords(ui, t, &tn, n);
        });

        section_gap(ui);
        section_title(ui, t, "Tasks and schedule");
        card(ui, t, |ui| {
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Edit,
                "Changes to my tasks",
                &mut n.task_changes,
                false,
                RowOpts {
                    desc: Some("Status, due date, priority"),
                    ..Default::default()
                },
                |_, _| false,
            );
            let due_time = &mut n.due_time;
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Calendar,
                "Tasks due today",
                &mut n.due,
                true,
                RowOpts {
                    extra_w: 190.0,
                    ..Default::default()
                },
                |ui, _| due_time_pick(ui, t, due_time),
            );
            let minutes = &mut n.meeting_minutes;
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Video,
                "Meetings",
                &mut n.meetings,
                true,
                RowOpts {
                    extra_w: 150.0,
                    ..Default::default()
                },
                |ui, _| minutes_stepper(ui, t, minutes),
            );
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Goals,
                "Goal deadlines",
                &mut n.goals,
                true,
                RowOpts {
                    desc: Some("The day before and the day of"),
                    ..Default::default()
                },
                |_, _| false,
            );
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Flag,
                "Goal milestones",
                &mut n.goal_milestones,
                true,
                RowOpts {
                    desc: Some("When my goals pass 25, 50, 75 and 100%"),
                    ..Default::default()
                },
                |_, _| false,
            );
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Warning,
                "Goals falling behind",
                &mut n.goal_risk,
                true,
                RowOpts {
                    desc: Some("Once, when a goal is far behind its time"),
                    ..Default::default()
                },
                |_, _| false,
            );
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Forms,
                "Answers to my forms",
                &mut n.forms,
                true,
                RowOpts::default(),
                |_, _| false,
            );
        });

        section_gap(ui);
        section_title(ui, t, "Workspace");
        card(ui, t, |ui| {
            if manages {
                ch |= setting_row(
                    ui,
                    t,
                    &tn,
                    Icon::PersonPlus,
                    "New members join",
                    &mut n.team,
                    false,
                    RowOpts {
                        badge: Some("Admins only"),
                        ..Default::default()
                    },
                    |_, _| false,
                );
            }
            ch |= setting_row(
                ui,
                t,
                &tn,
                Icon::Wifi,
                "The team server connection drops and comes back",
                &mut n.connection,
                manages,
                RowOpts::default(),
                |_, _| false,
            );
        });
        ui.add_space(8.0);

        let sound = n.sound;
        if while_quit != while_quit_was {
            self.set_push_while_quit(while_quit);
        }
        if open_settings && persist {
            crate::os::open_notification_settings();
        }
        if test {
            if persist {
                crate::notifier::post(
                    "Workboard",
                    &tr("Notifications are working."),
                    "inbox",
                    sound,
                );
            }
            self.toast(crate::os::mac_or_pc(
                tr("Sent a test notification. If no banner shows, allow Workboard in System Settings > Notifications."),
                tr("Sent a test notification. If no banner shows, turn on notifications for Workboard in Windows Settings > System > Notifications."),
            ));
        }
        if ch {
            self.dirty = true;
        }
    }
}

// ------------------------------------------------------------- page parts

fn section_title(ui: &mut Ui, t: &Theme, text: &str) {
    paint_line(ui, t, text, 13.0, 18.0);
    ui.add_space(8.0);
}

/// One line of grey text across the page, from 2 points in, `h` tall.
/// (Painted: a label in a row would be as tall as a button.)
fn paint_line(ui: &mut Ui, t: &Theme, text: &str, size: f32, h: f32) {
    let w = ui.available_width();
    let g = ui.painter().layout(
        tr(text).to_string(),
        FontId::proportional(size),
        t.text2,
        w - 2.0,
    );
    let (rect, _) = ui.allocate_exact_size(vec2(w, h.max(g.size().y)), Sense::hover());
    ui.painter().galley(
        pos2(rect.left() + 2.0, rect.center().y - g.size().y / 2.0),
        g,
        t.text2,
    );
}

fn section_gap(ui: &mut Ui) {
    ui.add_space(24.0);
}

/// A line under a section's title that says what its card's rows share.
fn caption(ui: &mut Ui, t: &Theme, text: &str) {
    ui.add_space(-6.0);
    paint_line(ui, t, text, 12.5, 18.0);
    ui.add_space(8.0);
}

/// A label with a rule after it, between the cards kept in the Inbox and
/// the ones that only notify.
fn divider_label(ui: &mut Ui, t: &Theme, text: &str) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
    let r = ui.painter().text(
        rect.left_center() + vec2(2.0, 0.0),
        Align2::LEFT_CENTER,
        tr(text),
        FontId::proportional(12.5),
        t.text2,
    );
    if r.right() + 12.0 < rect.right() {
        ui.painter().hline(
            (r.right() + 12.0)..=rect.right(),
            rect.center().y,
            Stroke::new(1.0_f32, t.border),
        );
    }
}

/// A bordered card the width of the page; `add` draws its rows edge to edge.
fn card<R>(ui: &mut Ui, t: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(t.card)
        .stroke(Stroke::new(1.0_f32, t.border))
        .corner_radius(CARD_RADIUS)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
        .inner
}

/// The rule above every row but a card's first: from the text to the edge.
fn row_rule(ui: &Ui, t: &Theme, rect: Rect) {
    ui.painter().hline(
        (rect.left() + TEXT_X)..=rect.right(),
        rect.top() + 0.5,
        Stroke::new(1.0_f32, t.border),
    );
}

fn icon_tile(ui: &Ui, tn: &Tones, t: &Theme, icon: Icon, c: Pos2) {
    let tile = Rect::from_center_size(c, vec2(ICON_TILE, ICON_TILE));
    ui.painter().rect_filled(tile, 8.0, tn.tile);
    line_icon(ui.painter(), icon, c, t.text2);
}

/// One setting: an icon tile, the name (and a line under it when it says
/// more than the name), what `extra` adds, and a switch. Clicking the row
/// anywhere but on `extra`'s controls turns the setting on or off; those
/// controls are only usable while it is on. `title` is English: the row
/// shows it translated and is named by it. Returns true when it changed.
#[allow(clippy::too_many_arguments)]
fn setting_row(
    ui: &mut Ui,
    t: &Theme,
    tn: &Tones,
    icon: Icon,
    title: &str,
    on: &mut bool,
    rule: bool,
    opts: RowOpts,
    extra: impl FnOnce(&mut Ui, bool) -> bool,
) -> bool {
    let w = ui.available_width();
    let right = ROW_RIGHT
        + SWITCH_SIZE.x
        + if opts.extra_w > 0.0 {
            opts.extra_w + 10.0
        } else {
            0.0
        };
    let badge = opts.badge.map(|b| {
        ui.painter()
            .layout_no_wrap(tr(b).to_string(), FontId::proportional(11.0), t.text2)
    });
    let badge_w = badge.as_ref().map_or(0.0, |g| g.size().x + 18.0);
    let wrap = (w - TEXT_X - right - 8.0 - badge_w).max(60.0);
    let name = ui.painter().layout(
        tr(title).to_string(),
        FontId::proportional(14.0),
        t.text,
        wrap,
    );
    let desc = opts.desc.map(|d| {
        ui.painter().layout(
            tr(d).to_string(),
            FontId::proportional(12.5),
            t.text2,
            wrap + badge_w,
        )
    });
    let text_h = name.size().y + desc.as_ref().map_or(0.0, |g| 2.0 + g.size().y);
    let h = (text_h + 24.0).max(52.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let mut changed = false;
    if resp.clicked() {
        *on = !*on;
        changed = true;
    }
    let state = *on;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, state, title));
    if rule {
        row_rule(ui, t, rect);
    }
    icon_tile(
        ui,
        tn,
        t,
        icon,
        pos2(rect.left() + 16.0 + ICON_TILE / 2.0, rect.center().y),
    );
    let top = rect.center().y - text_h / 2.0;
    let name_w = name.size().x;
    let name_h = name.size().y;
    ui.painter()
        .galley(pos2(rect.left() + TEXT_X, top), name, t.text);
    if let Some(g) = badge {
        let at = pos2(rect.left() + TEXT_X + name_w + 8.0, top + name_h / 2.0);
        let r = Rect::from_min_size(at - vec2(0.0, 9.0), vec2(g.size().x + 12.0, 18.0));
        ui.painter().rect_filled(r, 4.0, tn.tile);
        ui.painter().galley(r.center() - g.size() / 2.0, g, t.text2);
    }
    if let Some(g) = desc {
        ui.painter()
            .galley(pos2(rect.left() + TEXT_X, top + name_h + 2.0), g, t.text2);
    }
    let sw = Rect::from_min_size(
        pos2(
            rect.right() - ROW_RIGHT - SWITCH_SIZE.x,
            rect.center().y - SWITCH_SIZE.y / 2.0,
        ),
        SWITCH_SIZE,
    );
    paint_switch(ui, t, &resp, sw, state);
    if opts.extra_w > 0.0 {
        let room = Rect::from_min_max(
            pos2(sw.left() - 10.0 - opts.extra_w, rect.top()),
            pos2(sw.left() - 10.0, rect.bottom()),
        );
        let mut sub = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(room)
                .layout(Layout::right_to_left(Align::Center)),
        );
        sub.add_enabled_ui(state, |ui| changed |= extra(ui, state));
    }
    changed
}

// ---------------------------------------------------------------- preview

/// The app's icon, for the preview banner and the Dock tile.
fn app_icon(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("notify_app_icon");
    if let Some(tex) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(tex);
    }
    let img = image::load_from_memory_with_format(
        include_bytes!("../../../Resources/AppIcon-256.png"),
        image::ImageFormat::Png,
    )
    .ok()?
    .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let tex = ctx.load_texture(
        "notify app icon",
        egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    Some(tex)
}

fn paint_app_icon(ui: &Ui, rect: Rect, fade: f32) {
    let tint = Color32::WHITE.gamma_multiply(fade);
    match app_icon(ui.ctx()) {
        Some(tex) => {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            ui.painter().image(tex.id(), rect, uv, tint);
        }
        None => {
            ui.painter().rect_filled(
                rect.shrink(rect.width() * 0.1),
                rect.width() * 0.2,
                Color32::from_gray(32).gamma_multiply(fade),
            );
        }
    }
}

/// What a notice looks like with the switches below: a banner (faded when
/// banners are off, with a speaker while sounds are on) and the Dock icon
/// with its count (while the count is on). It follows the switches as they
/// change.
fn preview(ui: &mut Ui, t: &Theme, tn: &Tones, n: &NotifySettings) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 112.0), Sense::hover());
    let p = ui.painter();
    let r = CARD_RADIUS - 1;
    p.rect_filled(
        rect.shrink(1.0),
        egui::CornerRadius {
            nw: r,
            ne: r,
            sw: 0,
            se: 0,
        },
        tn.well,
    );
    p.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0_f32, t.border),
    );

    // the Dock (or taskbar) icon at the right
    let dock = Rect::from_center_size(
        pos2(rect.right() - 52.0, rect.center().y - 9.0),
        vec2(48.0, 48.0),
    );
    paint_app_icon(ui, dock, 1.0);
    if n.dock_badge {
        let c = dock.right_top() + vec2(-6.0, 6.0);
        p.circle_filled(c, 10.0, Color32::from_rgb(0xe5, 0x48, 0x4d));
        p.text(
            c,
            Align2::CENTER_CENTER,
            "3",
            FontId::proportional(11.5),
            Color32::WHITE,
        );
    }
    let place = crate::os::mac_or_pc("Dock", "Taskbar");
    p.text(
        pos2(dock.center().x, dock.bottom() + 9.0),
        Align2::CENTER_CENTER,
        tr(place),
        FontId::proportional(11.5),
        t.text2,
    );

    // the banner, as wide as there is room for, up to 380
    let fade: f32 = if n.desktop { 1.0 } else { 0.4 };
    let right = dock.left() - 28.0;
    let left = rect.left() + 20.0;
    let b = Rect::from_min_max(
        pos2(left, rect.center().y - 32.0),
        pos2(right.min(left + 380.0), rect.center().y + 32.0),
    );
    if b.width() < 160.0 {
        return;
    }
    let shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(if t.dark { 90 } else { 22 }),
    };
    if n.desktop {
        p.add(shadow.as_shape(b, 12));
    }
    p.rect(
        b,
        12.0,
        tn.banner.gamma_multiply(fade.max(0.7)),
        Stroke::new(1.0_f32, t.border.gamma_multiply(fade)),
        egui::StrokeKind::Inside,
    );
    let icon = Rect::from_min_size(b.left_top() + vec2(10.0, 13.0), vec2(38.0, 38.0));
    paint_app_icon(ui, icon, fade);
    let x = icon.right() + 10.0;
    let text_w = b.right() - 12.0 - x;
    let dim = |c: Color32| c.gamma_multiply(fade);
    let head = p.text(
        pos2(x, b.top() + 16.0),
        Align2::LEFT_CENTER,
        "Workboard",
        FontId::proportional(12.0),
        dim(t.text),
    );
    if n.sound && n.desktop {
        line_icon_scaled(
            p,
            Icon::Volume,
            pos2(head.right() + 10.0, head.center().y),
            t.text2,
            0.7,
        );
    }
    let when = if n.desktop { "just now" } else { "Off" };
    p.text(
        pos2(b.right() - 12.0, b.top() + 16.0),
        Align2::RIGHT_CENTER,
        tr(when),
        FontId::proportional(11.5),
        t.text2,
    );
    paint_fit(
        ui,
        pos2(x, b.top() + 33.0),
        Align2::LEFT_CENTER,
        &tr("#design"),
        FontId::proportional(13.5),
        dim(t.text),
        text_w,
    );
    paint_fit(
        ui,
        pos2(x, b.top() + 50.0),
        Align2::LEFT_CENTER,
        &tr("Minji: Take a look at the new mockup"),
        FontId::proportional(12.5),
        dim(t.text2),
        text_w,
    );
}

/// The bottom of the first card: where to turn banners on when none show,
/// a link there under it, and the test button at the right. Returns (open
/// the settings, send a test).
fn delivery_footer(ui: &mut Ui, t: &Theme) -> (bool, bool) {
    let (help, open) = crate::os::mac_or_pc(
        (
            "If notifications don't show, turn them on for Workboard in System Settings.",
            "Open System Settings",
        ),
        (
            "If notifications don't show, turn them on for Workboard in Windows Settings.",
            "Open Windows Settings",
        ),
    );
    let w = ui.available_width();
    let test_label = tr("Send a test notification");
    let button_w = ui
        .painter()
        .layout_no_wrap(test_label.to_string(), FontId::proportional(14.0), t.text)
        .size()
        .x
        + 48.0;
    let text_w = (w - 16.0 - ROW_RIGHT - button_w - 16.0).max(120.0);
    let help_g = ui.painter().layout(
        tr(help).to_string(),
        FontId::proportional(12.5),
        t.text2,
        text_w,
    );
    let link_g =
        ui.painter()
            .layout_no_wrap(tr(open).to_string(), FontId::proportional(12.5), t.text);
    let h = (12.0 + help_g.size().y + 4.0 + link_g.size().y + 12.0).max(56.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0_f32, t.border),
    );
    let x = rect.left() + 16.0;
    let top = rect.center().y - (help_g.size().y + 4.0 + link_g.size().y) / 2.0;
    let help_h = help_g.size().y;
    ui.painter().galley(pos2(x, top), help_g, t.text2);
    let link_size = link_g.size();
    let link_rect = Rect::from_min_size(pos2(x, top + help_h + 4.0), link_size + vec2(14.0, 0.0));
    let link = ui
        .interact(
            link_rect,
            ui.id().with("open_notification_settings"),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    name_it(&link, open);
    let color = if link.hovered() { t.text2 } else { t.text };
    ui.painter().galley(link_rect.min, link_g, color);
    ui.painter().hline(
        link_rect.left()..=link_rect.left() + link_size.x,
        link_rect.top() + link_size.y + 0.5,
        Stroke::new(1.0_f32, color),
    );
    line_icon_scaled(
        ui.painter(),
        Icon::External,
        pos2(link_rect.right() - 5.0, link_rect.center().y),
        color,
        0.6,
    );
    let room = Rect::from_min_max(
        pos2(rect.right() - ROW_RIGHT - button_w, rect.top()),
        pos2(rect.right() - ROW_RIGHT, rect.bottom()),
    );
    let mut sub = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(room)
            .layout(Layout::right_to_left(Align::Center)),
    );
    let test = icon_button(
        &mut sub,
        t,
        Icon::Send,
        &test_label,
        "Send a test notification",
        true,
    )
    .clicked();
    (link.clicked(), test)
}

// ---------------------------------------------------------- chat controls

/// Channel messages: the row's name, where a channel is set apart, and the
/// three choices side by side. Choosing one makes channels set that way on
/// their own follow it again.
fn channel_choice(ui: &mut Ui, t: &Theme, tn: &Tones, n: &mut NotifySettings) -> bool {
    let w = ui.available_width();
    let wrap = w - TEXT_X - ROW_RIGHT;
    let name = ui.painter().layout(
        tr("Channel messages").to_string(),
        FontId::proportional(14.0),
        t.text,
        wrap,
    );
    let desc = ui.painter().layout(
        tr("Set a channel apart with the bell at its top").to_string(),
        FontId::proportional(12.5),
        t.text2,
        wrap,
    );
    let seg_h = 32.0;
    let h = 14.0 + name.size().y + 2.0 + desc.size().y + 10.0 + seg_h + 14.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    icon_tile(
        ui,
        tn,
        t,
        Icon::Hash,
        pos2(
            rect.left() + 16.0 + ICON_TILE / 2.0,
            rect.top() + 14.0 + ICON_TILE / 2.0,
        ),
    );
    let x = rect.left() + TEXT_X;
    let mut y = rect.top() + 14.0;
    let name_h = name.size().y;
    ui.painter().galley(pos2(x, y), name, t.text);
    y += name_h + 2.0;
    let desc_h = desc.size().y;
    ui.painter().galley(pos2(x, y), desc, t.text2);
    y += desc_h + 10.0;
    let track = Rect::from_min_max(pos2(x, y), pos2(rect.right() - ROW_RIGHT, y + seg_h));
    ui.painter().rect_filled(track, 9.0, tn.seg);
    let modes = [
        ("all", "Every new message"),
        ("mentions", "Only @mentions and my threads"),
        ("nothing", "Nothing"),
    ];
    let seg_w = (track.width() - 6.0) / modes.len() as f32;
    let mut changed = false;
    for (i, (mode, label)) in modes.into_iter().enumerate() {
        let r = Rect::from_min_size(
            track.left_top() + vec2(3.0 + seg_w * i as f32, 3.0),
            vec2(seg_w, seg_h - 6.0),
        );
        let resp = ui.interact(r, ui.id().with(("channel_mode", mode)), Sense::click());
        let chosen = n.channel_default == mode;
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, chosen, label)
        });
        if chosen {
            if !t.dark {
                ui.painter().add(
                    egui::epaint::Shadow {
                        offset: [0, 1],
                        blur: 3,
                        spread: 0,
                        color: Color32::from_black_alpha(30),
                    }
                    .as_shape(r, 7),
                );
            }
            ui.painter().rect_filled(r, 7.0, tn.seg_on);
        } else {
            hover_bg(ui, &resp, r, 7.0);
        }
        paint_fit(
            ui,
            r.center(),
            Align2::CENTER_CENTER,
            &tr(label),
            FontId::proportional(13.0),
            if chosen { t.text } else { t.text2 },
            r.width() - 12.0,
        );
        if resp.clicked() && !chosen {
            n.channel_default = mode.into();
            // channels set the same way as the new default no longer stand apart
            n.channel_modes.retain(|(_, m)| m != mode);
            changed = true;
        }
    }
    changed
}

/// Keywords: each a chip that can be taken out, and a field to add more
/// (Enter, a comma or leaving the field adds what was typed).
fn keywords(ui: &mut Ui, t: &Theme, tn: &Tones, n: &mut NotifySettings) -> bool {
    let w = ui.available_width();
    let top = ui.cursor().top();
    let left = ui.max_rect().left();
    row_rule(ui, t, Rect::from_min_size(pos2(left, top), vec2(w, 1.0)));
    icon_tile(
        ui,
        tn,
        t,
        Icon::Tag,
        pos2(left + 16.0 + ICON_TILE / 2.0, top + 14.0 + ICON_TILE / 2.0),
    );
    let mut changed = false;
    let mut list = n.keyword_list();
    let buf_id = egui::Id::new("notify_new_keyword");
    let mut buf: String = ui.data(|d| d.get_temp(buf_id)).unwrap_or_default();
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: TEXT_X as i8,
            right: ROW_RIGHT as i8,
            top: 14,
            bottom: 14,
        })
        .show(ui, |ui| {
            ui.set_width(w - TEXT_X - ROW_RIGHT);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(tr("Keywords")).size(14.0).color(t.text));
            ui.label(
                RichText::new(tr(
                    "Notified like a mention when a channel message has one of these words",
                ))
                .size(12.5)
                .color(t.text2),
            );
            ui.add_space(8.0);
            egui::Frame::new()
                .stroke(Stroke::new(1.0_f32, t.border))
                .corner_radius(8)
                .inner_margin(egui::Margin::same(5))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        let mut remove = None;
                        for k in &list {
                            if keyword_chip(ui, t, tn, k) {
                                remove = Some(k.clone());
                            }
                        }
                        if let Some(k) = remove {
                            list.retain(|x| *x != k);
                            changed = true;
                        }
                        let field_w = ui.available_width().clamp(120.0, 220.0);
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut buf)
                                .id(egui::Id::new("notify_keyword_field"))
                                .frame(false)
                                .hint_text(tr("Add keyword"))
                                .desired_width(field_w)
                                .min_size(vec2(0.0, 26.0))
                                .vertical_align(Align::Center),
                        );
                        name_it(&resp, "Notification keywords");
                        let entered = resp.lost_focus();
                        // a comma ends a word as it is typed; Enter or
                        // leaving the field ends the last one
                        let done = if entered {
                            std::mem::take(&mut buf)
                        } else if let Some(i) = buf.rfind(',') {
                            let head = buf[..i].to_string();
                            buf = buf[i + 1..].trim_start().to_string();
                            head
                        } else {
                            String::new()
                        };
                        for word in done.split(',').map(str::trim).filter(|w| !w.is_empty()) {
                            if !list.iter().any(|k| k.eq_ignore_ascii_case(word)) {
                                list.push(word.to_string());
                                changed = true;
                            }
                        }
                        if entered && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            resp.request_focus();
                        }
                    });
                });
        });
    ui.data_mut(|d| d.insert_temp(buf_id, buf));
    if changed {
        n.keywords = list.join(", ");
    }
    changed
}

/// A keyword with an x that takes it out. Returns true when the x is clicked.
fn keyword_chip(ui: &mut Ui, t: &Theme, tn: &Tones, word: &str) -> bool {
    let g = ui
        .painter()
        .layout_no_wrap(word.to_string(), FontId::proportional(13.0), t.text);
    let size = vec2(10.0 + g.size().x + 4.0 + 20.0 + 3.0, 26.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, 13.0, tn.tile);
    ui.painter().galley(
        pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0),
        g,
        t.text,
    );
    let x = Rect::from_center_size(pos2(rect.right() - 13.0, rect.center().y), vec2(20.0, 20.0));
    let resp = ui.interact(x, ui.id().with(("remove_keyword", word)), Sense::click());
    name_it(&resp, &format!("Remove keyword {}", word));
    hover_bg(ui, &resp, x, 10.0);
    let s = Stroke::new(1.4_f32, t.text2);
    let c = x.center();
    ui.painter()
        .line_segment([c + vec2(-3.5, -3.5), c + vec2(3.5, 3.5)], s);
    ui.painter()
        .line_segment([c + vec2(-3.5, 3.5), c + vec2(3.5, -3.5)], s);
    resp.clicked()
}

// --------------------------------------------------------- task controls

/// The time of day the due notice comes, picked from a menu of half hours
/// (a time set another way stays in the menu).
fn due_time_pick(ui: &mut Ui, t: &Theme, due_time: &mut String) -> bool {
    let current = parse_time_of_day(due_time);
    let shown = current.map_or_else(|| due_time.clone(), fmt_time);
    let text = crate::trf!("Every day at {}", shown);
    let g = ui
        .painter()
        .layout_no_wrap(text, FontId::proportional(13.0), t.text);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 36.0, CONTROL_H), Sense::click());
    name_it(&resp, "Due notice time");
    let fade = if resp.enabled() { 1.0 } else { 0.45 };
    ui.painter().rect_stroke(
        rect,
        7.0,
        Stroke::new(1.0_f32, t.border),
        egui::StrokeKind::Inside,
    );
    hover_bg(ui, &resp, rect, 7.0);
    ui.painter().galley(
        pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0),
        g,
        t.text.gamma_multiply(fade),
    );
    line_icon_scaled(
        ui.painter(),
        Icon::ChevronDown,
        pos2(rect.right() - 13.0, rect.center().y),
        t.text2.gamma_multiply(fade),
        0.7,
    );
    let mut changed = false;
    let seen_id = resp.id.with("menu_seen");
    click_menu(&resp, |ui| {
        // the first pass of this opening: show the chosen time, not midnight
        let nr = ui.ctx().cumulative_pass_nr();
        let last: Option<u64> = ui.data(|d| d.get_temp(seen_id));
        let first = last.is_none_or(|l| l + 1 < nr);
        ui.data_mut(|d| d.insert_temp(seen_id, nr));
        egui::ScrollArea::vertical()
            .max_height(260.0)
            .show(ui, |ui| {
                let mut slots: Vec<NaiveTime> = (0..48)
                    .filter_map(|i| NaiveTime::from_hms_opt(i / 2, (i % 2) * 30, 0))
                    .collect();
                if let Some(c) = current {
                    if !slots.contains(&c) {
                        slots.push(c);
                        slots.sort();
                    }
                }
                for at in slots {
                    let chosen = current == Some(at);
                    let row = menu_row_plain(ui, t, &fmt_time(at), chosen);
                    if chosen && first {
                        row.scroll_to_me(Some(Align::Center));
                    }
                    if row.clicked() {
                        *due_time = at.format("%H:%M").to_string();
                        changed = true;
                        ui.close_menu();
                    }
                }
            });
    });
    changed
}

/// Minutes before a meeting: minus, the number, plus, in steps of five
/// (1 to 120).
fn minutes_stepper(ui: &mut Ui, t: &Theme, minutes: &mut u32) -> bool {
    let label = crate::trf!("{} min before", *minutes);
    let g = ui
        .painter()
        .layout_no_wrap(label, FontId::proportional(13.0), t.text);
    let side = 28.0;
    let (rect, _) = ui.allocate_exact_size(
        vec2(side * 2.0 + g.size().x + 16.0, CONTROL_H),
        Sense::hover(),
    );
    let enabled = ui.is_enabled();
    let fade = if enabled { 1.0 } else { 0.45 };
    ui.painter().rect_stroke(
        rect,
        7.0,
        Stroke::new(1.0_f32, t.border),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        rect.center() - g.size() / 2.0,
        g,
        t.text.gamma_multiply(fade),
    );
    let less = Rect::from_min_size(rect.min, vec2(side, rect.height()));
    let more = Rect::from_min_size(
        pos2(rect.right() - side, rect.top()),
        vec2(side, rect.height()),
    );
    let mut changed = false;
    for (r, minus, name) in [
        (less, true, "Fewer minutes before a meeting"),
        (more, false, "More minutes before a meeting"),
    ] {
        let resp = ui.interact(r, ui.id().with(name), Sense::click());
        name_it(&resp, name);
        hover_bg(ui, &resp, r.shrink(2.0), 6.0);
        let color = t.text2.gamma_multiply(fade);
        if minus {
            paint_minus(ui.painter(), r.center(), 13.0, color);
        } else {
            paint_plus(ui.painter(), r.center(), 13.0, color);
        }
        if resp.clicked() {
            let m = *minutes;
            let next = if minus {
                if m > 5 {
                    (m - 1) / 5 * 5
                } else {
                    1
                }
            } else {
                (m / 5 + 1) * 5
            };
            let next = next.clamp(1, 120);
            if next != m {
                *minutes = next;
                changed = true;
            }
        }
    }
    changed
}
