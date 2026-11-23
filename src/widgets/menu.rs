// Popups and menus: opening and closing them, menu rows, and the status,
// priority and assignee pickers.

use super::*;

/// Opens a popup below `resp` (above it when it only fits there) when it is
/// clicked; shows `add` while open.
pub fn popup<R>(
    ui: &Ui,
    resp: &Response,
    id: egui::Id,
    width: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    close_when_scrolled_away(ui, resp, id);
    // a menu taller than the window scrolls inside itself instead of running
    // off the bottom (egui moves a shorter one up until it fits). Only menus
    // that were too tall last frame get the scroll area, so ordinary menus
    // are laid out exactly as before.
    let screen = ui.ctx().screen_rect();
    let tall_id = id.with("__tall");
    let tall: f32 = ui.data(|d| d.get_temp(tall_id)).unwrap_or(0.0);
    // a popup that does not fit below its button opens above it when there
    // is more room there; otherwise egui pushes it up over the button (an
    // emoji grid opened from the composer at the bottom covered the composer)
    let margin = egui::Frame::popup(ui.style()).total_margin().sum().y;
    let below = screen.bottom() - resp.rect.bottom() - 8.0;
    let above_room = resp.rect.top() - screen.top() - 8.0;
    let above = tall + margin > below && above_room > below;
    let room = if above {
        above_room - margin
    } else {
        (screen.bottom() - resp.rect.bottom() - 32.0).max(screen.height() * 0.6)
    };
    // egui only pushes a popup back inside the window, so near the right
    // edge it touched the edge: move it left to leave a margin (the width
    // is last frame's, or the minimum before it has been shown)
    let wide_id = id.with("__wide");
    let wide: f32 = ui.data(|d| d.get_temp(wide_id)).unwrap_or(width + 24.0);
    let mut at = resp.clone();
    // an open popup stays where it opened even when its button moves: a
    // click inside it (hiding a view tab) shortened the row and slid the
    // button and the popup left under the pointer. A new window width lets
    // it follow the button again.
    let pin_id = id.with("__pin");
    if ui.memory(|m| m.is_popup_open(id)) {
        let (x, w) = ui.data_mut(|d| {
            *d.get_temp_mut_or_insert_with(pin_id, || (at.rect.left(), screen.width()))
        });
        let x = if w == screen.width() {
            x
        } else {
            ui.data_mut(|d| d.insert_temp(pin_id, (at.rect.left(), screen.width())));
            at.rect.left()
        };
        at.rect = at.rect.translate(vec2(x - at.rect.left(), 0.0));
    } else {
        ui.data_mut(|d| d.remove::<(f32, f32)>(pin_id));
    }
    let over = at.rect.left() + wide + 8.0 - screen.right();
    if over > 0.0 {
        at.rect = at.rect.translate(vec2(-over, 0.0));
    }
    let resp = &at;
    let side = if above {
        egui::AboveOrBelow::Above
    } else {
        egui::AboveOrBelow::Below
    };
    egui::popup_above_or_below_widget(
        ui,
        id,
        resp,
        side,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| popup_body(ui, id, width, tall > room, room, add),
    )
}

/// A menu's items (a right-click menu, a "..." menu), in a scroll area as
/// high as the window when they were taller than it last frame: egui only
/// moves a menu up to keep it inside, so the last items of a taller one ran
/// off the bottom. (A menu grows only as its content did, so ordinary menus
/// are laid out without the scroll area, as before.)
pub fn menu_fits<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let screen = ui.ctx().screen_rect();
    let margin = egui::Frame::menu(ui.style()).total_margin().sum().y;
    let room = (screen.height() - 16.0 - margin).max(80.0);
    let tall_id = ui.id().with("__menu_tall");
    let tall: f32 = ui.data(|d| d.get_temp(tall_id)).unwrap_or(0.0);
    if tall > room {
        egui::ScrollArea::vertical()
            .id_salt("menu_fits")
            .max_height(room)
            .show(ui, |ui| {
                let r = add(ui);
                let h = ui.min_rect().height();
                ui.data_mut(|d| d.insert_temp(tall_id, h));
                r
            })
            .inner
    } else {
        let top = ui.min_rect().top();
        let r = add(ui);
        let h = ui.min_rect().bottom() - top;
        ui.data_mut(|d| d.insert_temp(tall_id, h));
        r
    }
}

/// A floating window kept inside the app's window with a margin: no larger
/// than the room there (`frame` is the window's frame, whose margins egui
/// leaves out of its own limit, so it reached past the edge), and moved
/// back inside when it would reach out.
pub fn window_inside<'a>(
    w: egui::Window<'a>,
    ctx: &egui::Context,
    frame: &egui::Frame,
) -> egui::Window<'a> {
    let room = ctx.screen_rect().shrink(8.0);
    w.constrain_to(room)
        .max_size(room.size() - frame.total_margin().sum())
}

/// Closes the open popup `id` once its button is scrolled out of sight:
/// egui kept it inside the window, so it stayed at the window's top or
/// bottom edge, away from anything it belonged to.
fn close_when_scrolled_away(ui: &Ui, resp: &Response, id: egui::Id) {
    if !ui.clip_rect().intersects(resp.rect) && ui.memory(|m| m.is_popup_open(id)) {
        ui.memory_mut(|m| m.close_popup());
    }
}

/// Opens a popup to the right of `resp` when it is clicked, its top level
/// with the button's and moved up as far as it must to stay in the window;
/// shows `add` while open. For the left bar: below its button a popup
/// covered the buttons under it, and slid down out of the window as apps
/// pinned to the bar pushed the button down.
pub fn popup_right<R>(
    ui: &Ui,
    resp: &Response,
    id: egui::Id,
    width: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    close_when_scrolled_away(ui, resp, id);
    let screen = ui.ctx().screen_rect();
    // last frame's height (none before it has been shown: egui keeps that
    // first frame inside the window)
    let tall: f32 = ui.data(|d| d.get_temp(id.with("__tall"))).unwrap_or(0.0);
    let margin = egui::Frame::popup(ui.style()).total_margin().sum().y;
    let room = screen.height() - 16.0 - margin;
    let top = resp
        .rect
        .top()
        .min(screen.bottom() - 8.0 - tall.min(room) - margin)
        .max(screen.top() + 8.0);
    // egui opens a popup below the rect it is given: an empty one at the
    // button's right edge. Clicks are still told apart by the button's own
    // rect, so clicking the button again closes it.
    let mut at = resp.clone();
    at.rect = Rect::from_min_size(pos2(resp.rect.right() + 6.0, top), egui::Vec2::ZERO);
    egui::popup_above_or_below_widget(
        ui,
        id,
        &at,
        egui::AboveOrBelow::Below,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| popup_body(ui, id, width, tall > room, room, add),
    )
}

/// The inside of a popup at least `width` wide: `add`, in a scroll area
/// `room` high when `scroll`. Remembers how tall and wide it was for the
/// next frame's placement.
fn popup_body<R>(
    ui: &mut Ui,
    id: egui::Id,
    width: f32,
    scroll: bool,
    room: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let tall_id = id.with("__tall");
    ui.set_min_width(width);
    let out = if scroll {
        egui::ScrollArea::vertical()
            .id_salt(id.with("scroll"))
            .max_height(room)
            .auto_shrink([true, true])
            .show(ui, |ui| {
                ui.set_min_width(width);
                let r = add(ui);
                let h = ui.min_rect().height();
                ui.data_mut(|d| d.insert_temp(tall_id, h));
                r
            })
            .inner
    } else {
        let top = ui.min_rect().top();
        let r = add(ui);
        let h = ui.min_rect().bottom() - top;
        ui.data_mut(|d| d.insert_temp(tall_id, h));
        r
    };
    // the frame's margin on both sides
    let w = ui.min_rect().width() + 2.0 * egui::Frame::popup(ui.style()).total_margin().left;
    ui.data_mut(|d| d.insert_temp(id.with("__wide"), w));
    out
}

/// Opens a real egui menu below `resp` when it is clicked, the same kind a
/// right click opens: rows draw as menu rows and `ui.menu_button` inside
/// opens a side submenu instead of a new dropdown over the parent.
///
/// Near the right edge of the window the menu's right edge lines up with
/// the button's, and a submenu opens to the left (patched egui). Before,
/// the menu was pushed far left of its button to leave room for a submenu.
pub fn click_menu(resp: &Response, add: impl FnOnce(&mut Ui)) {
    const MENU_W: f32 = 230.0;
    let ctx = &resp.ctx;
    let bar_id = resp.id.with("__click_menu");
    let mut st = egui::menu::BarState::load(ctx, bar_id);
    let open = st.as_ref().is_some_and(|r| r.id == resp.id);
    let pressed_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.primary_down())
        && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
            !resp.rect.contains(p)
                && !st
                    .as_ref()
                    .is_some_and(|r| r.menu_state.read().area_contains(p))
        });
    if open && (resp.clicked() || pressed_outside) {
        **st = None;
    } else if resp.clicked() {
        let screen = ctx.screen_rect();
        let margin = egui::Frame::menu(&ctx.style()).total_margin().left;
        let pos = resp.rect.left_bottom() + vec2(-margin, ctx.style().spacing.menu_spacing);
        let root = egui::menu::MenuRoot::new(pos, resp.id);
        // too close to the right edge to open rightwards: hang the menu from
        // the button's right edge instead (the frame's margin past it, as
        // the left side is)
        if pos.x + MENU_W > screen.right() - 8.0 {
            root.menu_state.write().right_edge = Some(resp.rect.right() + margin);
        }
        **st = Some(root);
    }
    // (MenuRootManager::show returns None while the menu stays open, so
    // read the state instead)
    let shown = st.as_ref().is_some_and(|r| r.id == resp.id);
    st.show(resp, add);
    st.store(ctx, bar_id);
    if shown {
        let nr = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(MENU_SHOWN), nr));
    }
}

const MENU_SHOWN: &str = "__wb_click_menu_shown";

/// True when a popup, a right-click menu or a `click_menu` was on screen in
/// the previous pass. Call it at the start of a pass: egui closes those on
/// Escape during the pass, so checking later sees them already gone.
pub fn any_menu_was_open(ctx: &egui::Context) -> bool {
    let nr = ctx.cumulative_pass_nr();
    let last: Option<u64> = ctx.data(|d| d.get_temp(egui::Id::new(MENU_SHOWN)));
    ctx.memory(|m| m.any_popup_open())
        || ctx.is_context_menu_open()
        || last.is_some_and(|l| l + 1 >= nr)
}

pub fn close_popup(ui: &Ui) {
    ui.memory_mut(|m| m.close_popup());
}

/// A plain full-width menu row with an optional painted lead.
pub fn menu_row(
    ui: &mut Ui,
    t: &Theme,
    label: &str,
    selected: bool,
    lead: impl FnOnce(&egui::Painter, Pos2),
) -> Response {
    menu_row_named(ui, t, label, label, selected, lead)
}

/// A `menu_row` showing `label` whose accessibility name is `name` (a row
/// that shows a task's name but is called "Pick <name>").
pub fn menu_row_named(
    ui: &mut Ui,
    t: &Theme,
    name: &str,
    label: &str,
    selected: bool,
    lead: impl FnOnce(&egui::Painter, Pos2),
) -> Response {
    let w = ui.available_width().max(160.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    name_it(&resp, name);
    if resp.hovered() || selected {
        ui.painter()
            .rect_filled(rect, 5.0, if selected { t.selected_bg } else { t.hover });
    }
    lead(ui.painter(), rect.left_center() + vec2(14.0, 0.0));
    ui.painter().text(
        rect.left_center() + vec2(30.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        t.text,
    );
    resp
}

/// A menu row with no icon: the text starts at the row's edge instead of
/// after an empty icon slot.
pub fn menu_row_plain(ui: &mut Ui, t: &Theme, label: &str, selected: bool) -> Response {
    let w = ui.available_width().max(160.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    name_it(&resp, label);
    if resp.hovered() || selected {
        ui.painter()
            .rect_filled(rect, 5.0, if selected { t.selected_bg } else { t.hover });
    }
    ui.painter().text(
        rect.left_center() + vec2(10.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        t.text,
    );
    resp
}

pub fn status_menu(ui: &mut Ui, t: &Theme, statuses: &[Status], current: &mut String) -> bool {
    let mut changed = false;
    ui.label(egui::RichText::new(tr("Statuses")).small().color(t.text3));
    for st in statuses {
        let sel = &st.name == current;
        let r = menu_row(
            ui,
            t,
            &crate::i18n::status_text(&st.name).to_uppercase(),
            sel,
            |p, c| paint_status_circle(p, c, 6.0, st),
        );
        if r.clicked() {
            if !sel {
                *current = st.name.clone();
                changed = true;
            }
            close_popup(ui);
        }
    }
    changed
}

pub fn priority_menu(ui: &mut Ui, t: &Theme, current: &mut u8) -> bool {
    match priority_pick(ui, t, *current) {
        Some(p) if p != *current => {
            *current = p;
            true
        }
        _ => false,
    }
}

/// The priority list; returns the row the user clicked this frame (0 for
/// Clear), even when it equals `current`. Bulk edit needs "Clear was
/// clicked" separately from "the value changed".
pub fn priority_pick(ui: &mut Ui, t: &Theme, current: u8) -> Option<u8> {
    let mut picked = None;
    ui.label(
        egui::RichText::new(tr("Task Priority"))
            .small()
            .color(t.text3),
    );
    for pr in [1u8, 2, 3, 4] {
        let col = priority_color(pr, t);
        let r = menu_row(
            ui,
            t,
            tr(PRIORITIES[pr as usize].label),
            current == pr,
            // 1pt low: the flag's cloth sits at the top of its pole, so a
            // flag centred on the row looked higher than the text beside it
            |p, c| paint_flag(p, c + vec2(-4.0, 1.0), col, 13.0),
        );
        if r.clicked() {
            picked = Some(pr);
            close_popup(ui);
        }
    }
    ui.separator();
    // a drawn cross starting where the flag poles stand, about as large as
    // a flag; the small "×" glyph sat to the right of the poles and lower
    let r = menu_row(ui, t, tr("Clear"), false, |p, c| {
        let mid = c + vec2(0.0, 0.5);
        let s = Stroke::new(1.5_f32, t.text3);
        p.line_segment([mid + vec2(-4.0, -4.0), mid + vec2(4.0, 4.0)], s);
        p.line_segment([mid + vec2(-4.0, 4.0), mid + vec2(4.0, -4.0)], s);
    });
    if r.clicked() {
        picked = Some(0);
        close_popup(ui);
    }
    picked
}

pub fn assignee_menu(ui: &mut Ui, t: &Theme, data: &Data, current: &mut Vec<u64>) -> bool {
    let mut changed = false;
    ui.label(egui::RichText::new(tr("People")).small().color(t.text3));
    for m in &data.members {
        let sel = current.contains(&m.id);
        let label = if m.id == data.me {
            crate::trf!("{} (you)", m.shown())
        } else {
            m.shown().to_string()
        };
        let ring = t.card;
        let r = menu_row(ui, t, &label, sel, |p, c| paint_avatar(p, c, 9.0, m, ring));
        if sel {
            // a drawn tick (the glyph came from the emoji font)
            let c = r.rect.right_center() - vec2(16.0, 0.0);
            ui.painter().add(egui::Shape::line(
                vec![
                    c + vec2(-4.5, 0.3),
                    c + vec2(-1.5, 3.3),
                    c + vec2(4.8, -3.2),
                ],
                Stroke::new(1.5_f32, t.selected_text),
            ));
        }
        if r.clicked() {
            if sel {
                current.retain(|x| *x != m.id);
            } else if data.extra.apps.multiple_assignees {
                current.push(m.id);
            } else {
                *current = vec![m.id];
                close_popup(ui);
            }
            changed = true;
        }
    }
    changed
}
