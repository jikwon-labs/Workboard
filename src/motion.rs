//! Short movements that keep the app from looking still: hover fills that
//! fade, a switch knob that slides, a page or panel that rises into place
//! as it opens. egui draws each frame from the state of that moment, so
//! anything that should move between two states has to be eased here.
//!
//! Off in tests and `--screenshot` runs (they read the picture of one
//! frame), and when the system asks for reduced motion.

use egui::{Color32, Id, Rect, Response, Ui, Vec2};
use std::sync::atomic::{AtomicBool, Ordering};

static ON: AtomicBool = AtomicBool::new(!cfg!(test));

/// How long a hover fill takes to come and go.
pub const HOVER: f32 = 0.12;
/// How long a page, panel or popup takes to settle in.
pub const ENTER: f32 = 0.22;
/// How long a switch knob or a tick takes to move.
pub const TOGGLE: f32 = 0.16;

pub fn set_enabled(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

/// Fast at first, gentle at the end, like the system's own animations.
pub fn ease_out(t: f32) -> f32 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

/// How far the pointer has come onto `resp`, from 0 (away) to 1 (on it or
/// pressing it), eased in both directions.
pub fn hover(resp: &Response) -> f32 {
    let on = resp.enabled() && (resp.hovered() || resp.is_pointer_button_down_on());
    if !enabled() {
        return if on { 1.0 } else { 0.0 };
    }
    resp.ctx
        .animate_bool_with_time(resp.id.with("wb_hover"), on, HOVER)
}

/// `fill` as strong as the pointer is on `resp`: transparent away from it,
/// fading in and out. A press makes it half as strong again.
pub fn hover_fill(resp: &Response, fill: Color32) -> Option<Color32> {
    let k = hover(resp);
    if k <= 0.0 {
        return None;
    }
    let fill = if resp.is_pointer_button_down_on() {
        fill.gamma_multiply(1.5)
    } else {
        fill
    };
    Some(fill.gamma_multiply(k))
}

/// `on` as a number from 0 to 1 that slides over `secs` when it changes.
pub fn toggle(ctx: &egui::Context, id: Id, on: bool, secs: f32) -> f32 {
    if !enabled() {
        return if on { 1.0 } else { 0.0 };
    }
    ctx.animate_bool_with_time_and_easing(id, on, secs, egui::emath::easing::cubic_out)
}

/// From 0 to 1 (eased) over `secs` since what is shown under `id` came up:
/// since `key` changed, or since it is shown again after a pass without it
/// (a panel that opens). 1 in the app's first passes, so nothing moves as
/// the window comes up.
pub fn since_change(ctx: &egui::Context, id: Id, key: u64, secs: f32) -> f32 {
    if !enabled() {
        return 1.0;
    }
    let now = ctx.input(|i| i.time);
    let pass = ctx.cumulative_pass_nr();
    let start = ctx.data_mut(|d| {
        let start = match d.get_temp::<(u64, u64, f64)>(id) {
            Some((k, seen, start)) if k == key && seen + 1 >= pass => start,
            _ if pass < 3 => f64::NEG_INFINITY,
            _ => now,
        };
        d.insert_temp(id, (key, pass, start));
        start
    });
    let t = ((now - start) as f32 / secs).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    ease_out(t)
}

/// What `add` paints moves into place by `from` (it starts that far off)
/// and fades in, over `ENTER` after it comes up (`since_change`): a new
/// page, a panel that opens. Only the picture moves; clicks land where
/// things will settle.
pub fn enter<R>(ui: &mut Ui, id: Id, key: u64, from: Vec2, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = since_change(ui.ctx(), id, key, ENTER);
    let layer = ui.layer_id();
    let start = ui.ctx().graphics_mut(|g| g.entry(layer).next_idx().0);
    let out = add(ui);
    if t < 1.0 {
        move_shapes(ui.ctx(), layer, start, from * (1.0 - t), t);
    }
    out
}

/// Moves the shapes from `start` on in `layer` by `by` and fades them to
/// `opacity`.
fn move_shapes(ctx: &egui::Context, layer: egui::LayerId, start: usize, by: Vec2, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    ctx.graphics_mut(|g| {
        let list = g.entry(layer).shapes_mut();
        for clipped in list.iter_mut().skip(start) {
            if by != Vec2::ZERO {
                clipped.shape.translate(by);
            }
            if opacity < 1.0 {
                egui::epaint::shape_transform::adjust_colors(&mut clipped.shape, move |c| {
                    if *c != Color32::PLACEHOLDER {
                        *c = c.gamma_multiply(opacity);
                    }
                });
            }
        }
    });
}

/// A rectangle that glides to `target` over `secs` when it changes (eased):
/// the line under the chosen tab moving to the new one.
pub fn glide_rect(ctx: &egui::Context, id: Id, target: Rect, secs: f32) -> Rect {
    if !enabled() {
        return target;
    }
    let now = ctx.input(|i| i.time);
    // (where it set off from, where it is going, when it set off)
    let (from, to, start) = ctx.data_mut(|d| {
        let mut st =
            d.get_temp::<(Rect, Rect, f64)>(id)
                .unwrap_or((target, target, f64::NEG_INFINITY));
        if st.1 != target {
            let t = ease_out(((now - st.2) as f32 / secs).clamp(0.0, 1.0));
            let here = lerp_rect(st.0, st.1, t);
            st = (here, target, now);
        }
        d.insert_temp(id, st);
        st
    });
    let t = ((now - start) as f32 / secs).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    lerp_rect(from, to, ease_out(t))
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(a.min + (b.min - a.min) * t, a.max + (b.max - a.max) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_starts_fast_and_ends_at_one() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert!(ease_out(0.5) > 0.8);
        assert_eq!(ease_out(2.0), 1.0);
    }

    #[test]
    fn off_in_tests_so_frames_show_the_final_state() {
        assert!(!enabled());
        let ctx = egui::Context::default();
        assert_eq!(since_change(&ctx, Id::new("x"), 1, ENTER), 1.0);
        assert_eq!(since_change(&ctx, Id::new("x"), 2, ENTER), 1.0);
        assert_eq!(toggle(&ctx, Id::new("y"), true, TOGGLE), 1.0);
    }
}
