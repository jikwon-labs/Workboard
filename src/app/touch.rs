// One finger on an iPad's screen, the way iPad apps take it. egui-winit
// turns the first finger into the mouse: put down is a press, moving is a
// drag. That made a swipe over the sidebar drag a row (rows can be dragged
// to move them) instead of scrolling, and a list couldn't be flung. Here,
// before egui sees the input (App::fix_input):
//
// - a finger that moves right away scrolls what is under it, and a quick
//   swipe keeps scrolling for a moment after the finger leaves;
// - a finger lifted without moving is a click (the press is held back until
//   then);
// - a finger held still for a moment, then moved, drags as before.
//
// Only on iPad (`enabled`); tests turn it on.

use egui::{Event, Modifiers, MouseWheelUnit, Pos2, RawInput, TouchId, TouchPhase, Vec2};

/// How far a finger moves (points) before it scrolls instead of clicking.
const MOVE_TO_SCROLL: f32 = 10.0;
/// How long a finger stays still (seconds) before it drags instead.
const HOLD_TO_DRAG: f64 = 0.35;
/// Below this speed (points a second) a let-go list stops.
const FLING_MIN: f32 = 60.0;

#[derive(Default)]
enum State {
    #[default]
    Idle,
    /// Down, not yet a scroll, a click or a drag; the press waits here.
    Pending {
        id: TouchId,
        start: Pos2,
        last: Pos2,
        at: f64,
        press: Option<Event>,
    },
    Scrolling {
        id: TouchId,
        last: Pos2,
        moves: Vec<(f64, Vec2)>,
    },
    /// Held, then moved: egui gets the finger as the mouse, as before.
    Dragging { id: TouchId },
    /// Let go while moving: the list goes on, slowing down.
    Flinging { speed: Vec2, at: f64 },
}

#[derive(Default)]
pub(super) struct TouchScroll {
    pub enabled: bool,
    state: State,
    /// The release of a finger that scrolled, which was never pressed.
    drop_release: bool,
}

impl TouchScroll {
    pub fn new() -> Self {
        TouchScroll {
            enabled: cfg!(target_os = "ios"),
            ..Default::default()
        }
    }

    /// Rewrites this frame's events. Returns in how many seconds it needs
    /// another frame (a held finger turning into a drag, a fling going on).
    pub fn fix(&mut self, raw: &mut RawInput) -> Option<f64> {
        if !self.enabled {
            return None;
        }
        let now = raw.time.unwrap_or(0.0);
        let mut out = Vec::with_capacity(raw.events.len() + 4);
        // what time alone does: a still finger becomes a drag, a fling moves
        match &mut self.state {
            State::Pending { id, at, press, .. } if now - *at >= HOLD_TO_DRAG => {
                out.extend(press.take());
                self.state = State::Dragging { id: *id };
            }
            State::Flinging { speed, at } => {
                let dt = (now - *at).clamp(0.0, 0.1) as f32;
                *at = now;
                scroll(&mut out, *speed * dt);
                *speed *= (-dt * 4.0).exp();
                if speed.length() < FLING_MIN {
                    out.push(Event::PointerGone);
                    self.state = State::Idle;
                }
            }
            _ => {}
        }
        for e in std::mem::take(&mut raw.events) {
            self.take(e, now, &mut out);
        }
        raw.events = out;
        match &self.state {
            State::Pending { at, .. } => Some((HOLD_TO_DRAG - (now - at)).max(0.0)),
            State::Flinging { .. } => Some(0.0),
            _ => None,
        }
    }

    fn take(&mut self, e: Event, now: f64, out: &mut Vec<Event>) {
        let touch = match &e {
            Event::Touch { id, phase, pos, .. } => Some((*id, *phase, *pos)),
            _ => None,
        };
        match (&mut self.state, touch) {
            // a new finger (or one that stops a fling)
            (State::Idle | State::Flinging { .. }, Some((id, TouchPhase::Start, pos))) => {
                self.state = State::Pending {
                    id,
                    start: pos,
                    last: pos,
                    at: now,
                    press: None,
                };
                self.drop_release = false;
                out.push(e);
            }
            (
                State::Pending {
                    id,
                    start,
                    last,
                    press,
                    ..
                },
                Some((tid, phase, pos)),
            ) if tid == *id => {
                match phase {
                    TouchPhase::Move if (pos - *start).length() > MOVE_TO_SCROLL => {
                        // moved away at once: a scroll, and no press at all
                        let d = pos - *last;
                        scroll(out, d);
                        self.state = State::Scrolling {
                            id: tid,
                            last: pos,
                            moves: vec![(now, d)],
                        };
                    }
                    TouchPhase::Move => *last = pos,
                    // lifted where it went down: the press, then (next) its release
                    TouchPhase::End => {
                        out.extend(press.take());
                        self.state = State::Idle;
                    }
                    _ => self.state = State::Idle,
                }
                out.push(e);
            }
            (State::Scrolling { id, last, moves }, Some((tid, phase, pos))) if tid == *id => {
                match phase {
                    TouchPhase::Move => {
                        let d = pos - *last;
                        scroll(out, d);
                        moves.push((now, d));
                        *last = pos;
                    }
                    TouchPhase::End => {
                        // the last tenth of a second sets the speed it goes on at
                        let recent: Vec<_> = moves.iter().filter(|(t, _)| now - t <= 0.1).collect();
                        let span =
                            recent.first().map_or(0.0, |(t, _)| now - t).max(1.0 / 60.0) as f32;
                        let speed = recent.iter().fold(Vec2::ZERO, |a, (_, d)| a + *d) / span;
                        self.drop_release = true;
                        self.state = if speed.length() >= FLING_MIN * 2.0 {
                            State::Flinging { speed, at: now }
                        } else {
                            State::Idle
                        };
                    }
                    _ => {
                        self.drop_release = true;
                        self.state = State::Idle;
                    }
                }
                out.push(e);
            }
            (State::Dragging { id }, Some((tid, TouchPhase::End | TouchPhase::Cancel, _)))
                if tid == *id =>
            {
                self.state = State::Idle;
                out.push(e);
            }
            // the press waits until it's known to be a click or a drag
            (State::Pending { press, .. }, None)
                if press.is_none()
                    && matches!(
                        e,
                        Event::PointerButton {
                            pressed: true,
                            button: egui::PointerButton::Primary,
                            ..
                        }
                    ) =>
            {
                *press = Some(e);
            }
            // while scrolling the pointer stays where the finger went down,
            // so the same list takes all of it
            (State::Scrolling { .. }, None) if matches!(e, Event::PointerMoved(_)) => {}
            (_, None)
                if self.drop_release
                    && matches!(e, Event::PointerButton { pressed: false, .. }) =>
            {
                self.drop_release = false;
            }
            // a fling needs the pointer over its list until it stops
            (State::Flinging { .. }, None) if matches!(e, Event::PointerGone) => {}
            _ => out.push(e),
        }
    }
}

/// A scroll by `d`, in steps egui applies at once (it spreads steps of 8
/// points and more over several frames, which lags behind a finger).
fn scroll(out: &mut Vec<Event>, d: Vec2) {
    let n = (d.length() / 7.5).ceil().max(1.0);
    if d == Vec2::ZERO {
        return;
    }
    for _ in 0..n as usize {
        out.push(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: d / n,
            modifiers: Modifiers::default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, PointerButton};

    fn touch(phase: TouchPhase, x: f32, y: f32) -> Event {
        Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: TouchId(7),
            phase,
            pos: pos2(x, y),
            force: None,
        }
    }

    fn button(pressed: bool, x: f32, y: f32) -> Event {
        Event::PointerButton {
            pos: pos2(x, y),
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::default(),
        }
    }

    /// What egui-winit sends for a finger going down, moving, coming up.
    fn down(x: f32, y: f32) -> Vec<Event> {
        vec![
            touch(TouchPhase::Start, x, y),
            Event::PointerMoved(pos2(x, y)),
            button(true, x, y),
        ]
    }
    fn moved(x: f32, y: f32) -> Vec<Event> {
        vec![
            touch(TouchPhase::Move, x, y),
            Event::PointerMoved(pos2(x, y)),
        ]
    }
    fn up(x: f32, y: f32) -> Vec<Event> {
        vec![
            touch(TouchPhase::End, x, y),
            button(false, x, y),
            Event::PointerGone,
        ]
    }

    fn frame(t: &mut TouchScroll, time: f64, events: Vec<Event>) -> Vec<Event> {
        let mut raw = RawInput {
            time: Some(time),
            events,
            ..Default::default()
        };
        t.fix(&mut raw);
        raw.events
    }

    fn on() -> TouchScroll {
        TouchScroll {
            enabled: true,
            ..Default::default()
        }
    }

    fn presses(events: &[Event]) -> usize {
        events
            .iter()
            .filter(|e| matches!(e, Event::PointerButton { pressed: true, .. }))
            .count()
    }

    fn scrolled(events: &[Event]) -> Vec2 {
        events
            .iter()
            .filter_map(|e| match e {
                Event::MouseWheel { delta, .. } => Some(*delta),
                _ => None,
            })
            .fold(Vec2::ZERO, |a, d| a + d)
    }

    #[test]
    fn a_tap_is_a_click() {
        let mut t = on();
        let a = frame(&mut t, 0.0, down(100.0, 100.0));
        assert_eq!(presses(&a), 0, "the press waits");
        let b = frame(&mut t, 0.1, up(101.0, 100.0));
        let kinds: Vec<_> = b
            .iter()
            .filter(|e| matches!(e, Event::PointerButton { .. }))
            .map(|e| matches!(e, Event::PointerButton { pressed: true, .. }))
            .collect();
        assert_eq!(kinds, vec![true, false], "a press, then its release");
    }

    #[test]
    fn a_swipe_scrolls_and_never_presses() {
        let mut t = on();
        let mut all = frame(&mut t, 0.0, down(100.0, 400.0));
        all.extend(frame(&mut t, 0.02, moved(100.0, 380.0)));
        all.extend(frame(&mut t, 0.04, moved(100.0, 300.0)));
        all.extend(frame(&mut t, 0.06, up(100.0, 300.0)));
        assert_eq!(presses(&all), 0);
        assert!(!all
            .iter()
            .any(|e| matches!(e, Event::PointerButton { pressed: false, .. })));
        let d = scrolled(&all);
        assert!(
            d.x.abs() < 0.01 && (d.y + 100.0).abs() < 0.01,
            "with the finger: {d:?}"
        );
        assert!(
            all.iter().all(|e| match e {
                Event::MouseWheel { delta, .. } => delta.length() < 8.0,
                _ => true,
            }),
            "in steps egui applies at once"
        );
        assert!(
            !all.iter()
                .any(|e| matches!(e, Event::PointerMoved(p) if p.y < 400.0)),
            "the pointer stays over the list"
        );
    }

    #[test]
    fn a_quick_swipe_goes_on_after_the_finger_and_stops() {
        let mut t = on();
        frame(&mut t, 0.0, down(100.0, 400.0));
        frame(&mut t, 0.02, moved(100.0, 360.0));
        frame(&mut t, 0.04, moved(100.0, 320.0));
        let at_up = frame(&mut t, 0.05, up(100.0, 320.0));
        assert!(!at_up.iter().any(|e| matches!(e, Event::PointerGone)));
        let mut later = Vec2::ZERO;
        let mut time = 0.05;
        let mut gone = false;
        for _ in 0..600 {
            time += 1.0 / 60.0;
            let ev = frame(&mut t, time, vec![]);
            later += scrolled(&ev);
            if ev.iter().any(|e| matches!(e, Event::PointerGone)) {
                gone = true;
                break;
            }
        }
        assert!(later.y < -100.0, "it went on: {later:?}");
        assert!(gone, "and stopped");
    }

    #[test]
    fn holding_still_then_moving_drags_as_before() {
        let mut t = on();
        frame(&mut t, 0.0, down(100.0, 100.0));
        let held = frame(&mut t, 0.4, vec![]);
        assert_eq!(presses(&held), 1, "the press goes through");
        let m = frame(&mut t, 0.5, moved(100.0, 200.0));
        assert!(m.iter().any(|e| matches!(e, Event::PointerMoved(_))));
        assert_eq!(scrolled(&m), Vec2::ZERO);
        let u = frame(&mut t, 0.6, up(100.0, 200.0));
        assert!(u
            .iter()
            .any(|e| matches!(e, Event::PointerButton { pressed: false, .. })));
    }

    #[test]
    fn off_it_changes_nothing() {
        let mut t = TouchScroll::default();
        let ev = down(1.0, 2.0);
        assert_eq!(frame(&mut t, 0.0, ev.clone()).len(), ev.len());
    }
}
