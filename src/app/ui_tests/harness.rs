// The harness for the click-through tests: a window driven with real
// input events (Ui1) and the helpers the tests share.

use super::*;
use egui::Pos2;

/// The red pill at the bottom of the sidebar while unread chats are below.
pub(super) const UNREAD_PILL: &str = "Unread messages \u{2193}";
const UNREAD_PILL_KO: &str = "안 읽은 메시지 \u{2193}";

pub(super) struct Ui1 {
    pub(super) ctx: egui::Context,
    pub(super) app: App,
    pub(super) time: f64,
    pub(super) tree: Vec<(String, egui::Rect)>,
    /// the accessibility node of each name in `tree`, for actions a screen
    /// reader or keyboard activation sends (`accessibility_click`)
    pub(super) nodes: Vec<(String, egui::accesskit::NodeId)>,
    pub(super) size: egui::Vec2,
    pub(super) shapes: Vec<egui::epaint::ClippedShape>,
    /// files dropped on the window in the next frame
    pub(super) dropped: Vec<egui::DroppedFile>,
    /// files being dragged over the window, every frame until cleared
    pub(super) hovering: Vec<egui::HoveredFile>,
    /// the mouse cursor the last frame asked for
    pub(super) cursor: egui::CursorIcon,
    /// whether the window is in front (false: another app is)
    pub(super) focused: bool,
    /// what the frames since the last `close_window` asked of the main
    /// window (Close, CancelClose...)
    pub(super) commands: Vec<egui::ViewportCommand>,
    /// the window's close button is pressed in the next frame
    pub(super) close_next: bool,
}

impl Ui1 {
    pub(super) fn new() -> Self {
        Self::sized(1440.0, 900.0)
    }

    // The smallest window that still has the sidebar beside the page is
    // 960x560 (the "smallest window" of the layout tests; it was 900x560
    // before narrower windows got the drawer and one-column layouts, see
    // shell/narrow.rs). The app allows 360x520 (narrow.rs tests those).
    pub(super) fn sized(w: f32, h: f32) -> Self {
        Self::sized_with(w, h, Data::seed())
    }

    /// A window of the usual size showing `data` instead of the seed.
    pub(super) fn with_data(data: Data) -> Self {
        Self::sized_with(1440.0, 900.0, data)
    }

    /// A window of this size showing `data`. Tests that need an app other
    /// than the seed's start here, so it keeps the harness's clock pin.
    pub(super) fn sized_with(w: f32, h: f32, data: Data) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut h = Ui1 {
            ctx,
            app: App::build(None, Some(data)),
            time: 0.0,
            tree: vec![],
            nodes: vec![],
            size: vec2(w, h),
            shapes: vec![],
            dropped: vec![],
            hovering: vec![],
            cursor: egui::CursorIcon::Default,
            focused: true,
            commands: vec![],
            close_next: false,
        };
        // no notices on the wall clock: after 09:00 the seed's tasks due
        // today put a toast over the bottom of the window, so a test would
        // pass or fail by the time of day (notify_tests move the clock)
        h.app.notify_rt.last_clock = i64::MAX;
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }

    pub(super) fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.05;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.size)),
            time: Some(self.time),
            events,
            dropped_files: std::mem::take(&mut self.dropped),
            hovered_files: self.hovering.clone(),
            focused: self.focused,
            ..Default::default()
        };
        // as eframe reports it, read by the app through viewport().focused
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .focused = Some(self.focused);
        if std::mem::take(&mut self.close_next) {
            input
                .viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .events
                .push(egui::ViewportEvent::Close);
        }
        // as eframe's raw_input_hook does
        self.app.fix_input(&self.ctx, &mut input);
        let app = &mut self.app;
        let out = self.ctx.run(input, |ctx| app.ui_frame(ctx));
        self.shapes = out.shapes;
        self.cursor = out.platform_output.cursor_icon;
        if let Some(v) = out.viewport_output.get(&egui::ViewportId::ROOT) {
            self.commands.extend(v.commands.iter().cloned());
        }
        if let Some(update) = out.platform_output.accesskit_update {
            // a screen reader (or any app reading the window) panics on a
            // focus that has no node: accesskit_consumer's "Focused id ...
            // is not in the node list"
            assert!(
                update.tree.as_ref().is_some_and(|t| t.root == update.focus)
                    || update.nodes.iter().any(|(id, _)| *id == update.focus),
                "the focused widget has no accessibility node"
            );
            self.tree.clear();
            self.nodes.clear();
            for (id, node) in update.nodes {
                let Some(b) = node.bounds() else { continue };
                let rect = Rect::from_min_max(
                    pos2(b.x0 as f32, b.y0 as f32),
                    pos2(b.x1 as f32, b.y1 as f32),
                );
                for name in [node.label(), node.placeholder(), node.value()]
                    .into_iter()
                    .flatten()
                {
                    self.tree.push((name.to_string(), rect));
                    self.nodes.push((name.to_string(), id));
                }
            }
        }
    }

    /// Draws frames until `done` holds; fails the test after 20 seconds.
    pub(super) fn until(&mut self, what: &str, done: impl Fn(&Ui1) -> bool) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !done(self) {
            assert!(
                std::time::Instant::now() < end,
                "timed out waiting for: {}",
                what
            );
            self.frame(vec![]);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Draws frames, taking in what the server says, until `cond` holds,
    /// then settles; fails the test after 20 seconds.
    pub(super) fn net_until(&mut self, what: &str, cond: impl Fn(&Ui1) -> bool) {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while std::time::Instant::now() < end {
            self.app.net_tick(None);
            self.frame(vec![]);
            if cond(self) {
                self.settle();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for: {} (status {:?}, toast {:?})",
            what, self.app.sync_rt.net_status, self.app.toast
        );
    }

    /// Press the main window's close button (Alt+F4, Cmd+W): one frame.
    pub(super) fn close_window(&mut self) {
        self.commands.clear();
        self.close_next = true;
        self.frame(vec![]);
    }

    pub(super) fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(vec![]);
        }
    }

    pub(super) fn find(&self, name: &str) -> Option<Pos2> {
        self.tree
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r.center())
    }

    /// Whether the sidebar's "Unread messages" pill covers the point.
    fn under_pill(&self, p: Pos2) -> bool {
        self.tree
            .iter()
            .any(|(n, r)| (n == UNREAD_PILL || n == UNREAD_PILL_KO) && r.expand(4.0).contains(p))
    }

    pub(super) fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    // scroll with the mouse wheel over a control until it is on screen
    pub(super) fn scroll_into_view(&mut self, name: &str) {
        for _ in 0..40 {
            let Some(p) = self.find(name) else { return };
            let bottom = self.size.y - 20.0;
            let covered = name != UNREAD_PILL && name != UNREAD_PILL_KO && self.under_pill(p);
            if p.y > 40.0 && p.y < bottom && !covered {
                return;
            }
            let at = pos2(p.x, self.size.y / 2.0);
            let dy = if p.y >= bottom || covered {
                -120.0
            } else {
                120.0
            };
            self.frame(vec![egui::Event::PointerMoved(at)]);
            self.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, dy),
                modifiers: Default::default(),
            }]);
            self.settle();
        }
    }

    pub(super) fn click(&mut self, name: &str) {
        if (!self.has(name) || self.clipped_in_dialog(name)) && self.app.modal.is_some() {
            // maybe further down (or up) a dialog that scrolls
            self.scroll_dialog_to(name);
        }
        self.scroll_into_view(name);
        let pos = self.find(name).unwrap_or_else(|| {
            let mut names: Vec<&String> = self.tree.iter().map(|(n, _)| n).collect();
            names.sort();
            names.dedup();
            panic!(
                "nothing named {:?} on screen; visible names: {:?}",
                name, names
            )
        });
        self.click_at(pos);
    }

    /// Activates the control named `name` the way a screen reader or the
    /// keyboard does (an accessibility Click action), wherever it is, also
    /// scrolled out of sight.
    pub(super) fn accessibility_click(&mut self, name: &str) {
        let target = self
            .nodes
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, id)| *id)
            .unwrap_or_else(|| panic!("no accessibility node named {:?}", name));
        self.frame(vec![egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::Click,
                target,
                data: None,
            },
        )]);
        self.settle();
    }

    pub(super) fn click_at(&mut self, pos: Pos2) {
        self.click_button(pos, egui::PointerButton::Primary);
    }

    // move to a point, press and release one mouse button there
    pub(super) fn click_button(&mut self, pos: Pos2, button: egui::PointerButton) {
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button,
            pressed: true,
            modifiers: Default::default(),
        }]);
        self.frame(vec![egui::Event::PointerButton {
            pos,
            button,
            pressed: false,
            modifiers: Default::default(),
        }]);
        self.settle();
    }

    // click near the left end of a wide control (a task row's name)
    pub(super) fn click_left(&mut self, name: &str, dx: f32) {
        let r = self.rect(name);
        self.click_at(pos2(r.left() + dx, r.center().y));
    }

    pub(super) fn right_click(&mut self, name: &str, dx: f32) {
        self.scroll_into_view(name);
        let r = self.rect(name);
        let pos = pos2(r.left() + dx, r.center().y);
        self.click_button(pos, egui::PointerButton::Secondary);
    }

    // press on one control, move the mouse in small steps to another, release
    pub(super) fn drag(&mut self, from: &str, to: &str) {
        let a = self
            .find(from)
            .unwrap_or_else(|| panic!("nothing named {:?}", from));
        let b = self
            .find(to)
            .unwrap_or_else(|| panic!("nothing named {:?}", to));
        self.drag_by(a, b);
    }

    // press at one point, move the mouse in 12 steps to another, release
    pub(super) fn drag_by(&mut self, from: Pos2, to: Pos2) {
        self.drag_with(from, to, egui::PointerButton::Primary);
    }

    // drag_by with any mouse button
    pub(super) fn drag_with(&mut self, from: Pos2, to: Pos2, button: egui::PointerButton) {
        self.frame(vec![egui::Event::PointerMoved(from)]);
        self.frame(vec![egui::Event::PointerButton {
            pos: from,
            button,
            pressed: true,
            modifiers: Default::default(),
        }]);
        for i in 1..=12 {
            self.frame(vec![egui::Event::PointerMoved(
                from + (to - from) * (i as f32 / 12.0),
            )]);
        }
        self.frame(vec![egui::Event::PointerButton {
            pos: to,
            button,
            pressed: false,
            modifiers: Default::default(),
        }]);
        self.settle();
    }

    // click the left-most (or right-most) control with this name, for names
    // shown twice (sidebar and page)
    pub(super) fn click_side(&mut self, name: &str, rightmost: bool) {
        let pick = |h: &Self| {
            let mut hits: Vec<Rect> = h
                .tree
                .iter()
                .filter(|(n, _)| n == name)
                .map(|(_, r)| *r)
                .collect();
            hits.sort_by(|a, b| a.center().x.total_cmp(&b.center().x));
            if rightmost { hits.last() } else { hits.first() }
                .copied()
                .unwrap_or_else(|| panic!("nothing named {:?}", name))
        };
        for _ in 0..30 {
            let r = pick(self);
            let covered = self.under_pill(r.center());
            if r.center().y > 40.0 && r.center().y < 880.0 && !covered {
                break;
            }
            let dy = if r.center().y >= 880.0 || covered {
                -120.0
            } else {
                120.0
            };
            self.frame(vec![egui::Event::PointerMoved(pos2(r.center().x, 450.0))]);
            self.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, dy),
                modifiers: Default::default(),
            }]);
            self.settle();
        }
        let r = pick(self);
        self.click_at(r.center());
    }

    pub(super) fn hover(&mut self, pos: Pos2) {
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.settle();
    }

    /// Whether the soft hover fill (`widgets::hover_fill`) is painted under `pos`.
    pub(super) fn hover_fill_at(&self, pos: Pos2) -> bool {
        let fill = crate::widgets::hover_fill_for(self.ctx.style().visuals.dark_mode);
        self.shapes.iter().any(|c| match &c.shape {
            egui::Shape::Rect(r) => r.fill == fill && r.rect.contains(pos),
            _ => false,
        })
    }

    pub(super) fn type_text(&mut self, text: &str) {
        self.frame(vec![egui::Event::Text(text.to_string())]);
        self.frame(vec![]);
    }

    pub(super) fn key(&mut self, key: egui::Key) {
        self.key_with(key, Default::default());
    }

    // press and release a key with these modifiers held
    pub(super) fn key_with(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }]);
        self.settle();
    }

    // where the open dialog is on screen
    pub(super) fn modal_rect(&self) -> Rect {
        self.try_modal_rect().expect("a dialog is drawn")
    }

    pub(super) fn try_modal_rect(&self) -> Option<Rect> {
        let m = self.app.modal.as_ref()?;
        self.ctx
            .memory(|mem| mem.area_rect(super::menus::modal_window_id(m)))
    }

    /// Whether a control of the open dialog is laid out but scrolled out of
    /// it, wholly or in part: in the dialog's column, reaching above or below
    /// what it shows (its title row and its margins stay put, so the top 56
    /// and bottom 16 points don't count).
    fn clipped_in_dialog(&self, name: &str) -> bool {
        let Some(r) = self.try_modal_rect() else {
            return false;
        };
        let Some(c) = self
            .tree
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, c)| *c)
        else {
            return false;
        };
        let x = c.center().x;
        x > r.left()
            && x < r.right()
            && (c.top() < r.top() + 56.0 || c.bottom() > r.bottom() - 16.0)
    }

    // turn the mouse wheel over the open dialog until a control in it shows
    pub(super) fn scroll_dialog_to(&mut self, name: &str) {
        for _ in 0..30 {
            if self.has(name) && !self.clipped_in_dialog(name) {
                break;
            }
            let Some(r) = self.try_modal_rect() else {
                return;
            };
            // a notch at a time, more when the control is far off
            let dy = self.find(name).map_or(-120.0, |p| {
                let far = (r.center().y - p.y).clamp(-480.0, 480.0);
                if far.abs() < 120.0 {
                    120.0_f32.copysign(far)
                } else {
                    far
                }
            });
            let at = r.center();
            self.frame(vec![egui::Event::PointerMoved(at)]);
            self.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, dy),
                modifiers: Default::default(),
            }]);
            self.settle();
        }
    }

    pub(super) fn rect(&self, name: &str) -> Rect {
        self.tree
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("nothing named {:?}", name))
    }

    // the one control with this name inside a screen region
    pub(super) fn rect_in(&self, name: &str, area: Rect) -> Rect {
        self.tree
            .iter()
            .rev()
            .find(|(n, r)| n == name && area.contains(r.center()))
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("nothing named {:?} in {:?}", name, area))
    }

    /// The control with this name in the open date menu: the one nearest
    /// today's day cell. A task row can show the same word (a seed task
    /// starts on this week's Monday, so on Mondays its row says "Today").
    pub(super) fn date_menu_rect(&self, name: &str) -> Rect {
        let day = self
            .rect(&format!("Day {}", crate::model::today()))
            .center();
        self.tree
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .min_by(|a, b| {
                a.center()
                    .distance(day)
                    .total_cmp(&b.center().distance(day))
            })
            .unwrap_or_else(|| panic!("nothing named {:?}", name))
    }

    // every painted text with this exact content: (position, color)
    pub(super) fn painted_text(&self, text: &str) -> Vec<(Rect, Color32)> {
        let mut out = vec![];
        each_text(&self.shapes, |ts, _| {
            if ts.galley.text() != text {
                return;
            }
            let c = ts
                .galley
                .job
                .sections
                .first()
                .map(|s| s.format.color)
                .unwrap_or(ts.fallback_color);
            let c = if c == Color32::PLACEHOLDER {
                ts.fallback_color
            } else {
                c
            };
            out.push((
                ts.visual_bounding_rect(),
                ts.override_text_color.unwrap_or(c),
            ));
        });
        out
    }

    /// The "+" drawn as two crossing lines (`paint_plus`) inside `area`:
    /// the box the lines span and their colour.
    pub(super) fn plus_in(&self, area: Rect) -> Option<(Rect, Color32)> {
        let (mut across, mut down) = (None, None);
        for s in &self.shapes {
            if let egui::Shape::LineSegment {
                points: [a, b],
                stroke,
            } = &s.shape
            {
                if !(area.contains(*a) && area.contains(*b)) {
                    continue;
                }
                if (a.y - b.y).abs() < 0.01 {
                    across = Some((*a, *b, stroke.color));
                } else if (a.x - b.x).abs() < 0.01 {
                    down = Some((*a, *b));
                }
            }
        }
        let ((a, b, color), (c, d)) = (across?, down?);
        let cross = Rect::from_points(&[a, b, c, d]);
        // the lines cross at their middles
        (cross.center().distance(a.lerp(b, 0.5)) < 0.01).then_some((cross, color))
    }

    /// Panics unless the control named `name` shows a "+" in its middle.
    pub(super) fn assert_plus_centered(&self, name: &str) {
        let r = self.rect(name);
        let (cross, _) = self
            .plus_in(r)
            .unwrap_or_else(|| panic!("{}: no + drawn in it", name));
        let off = cross.center() - r.center();
        assert!(
            off.length() < 0.5,
            "{}: + is {:?} off its middle",
            name,
            off
        );
    }

    pub(super) fn circle_fill_at(&self, p: Pos2) -> Option<Color32> {
        self.shapes.iter().rev().find_map(|s| match &s.shape {
            egui::Shape::Circle(c) if c.center.distance(p) < 3.0 && c.radius > 5.0 => Some(c.fill),
            _ => None,
        })
    }

    /// Clicks the open chat's message box, types `text` and presses Enter.
    pub(super) fn send_message(&mut self, text: &str) {
        self.click("Message composer");
        self.type_text(text);
        self.key(egui::Key::Enter);
    }

    pub(super) fn cmd_enter(&mut self) {
        self.key_with(egui::Key::Enter, egui::Modifiers::COMMAND);
    }

    // open "Write copy for landing page" with no comments; returns its id
    pub(super) fn open_clean_task(&mut self) -> u64 {
        self.click_left("Task Write copy for landing page", 120.0);
        let id = self.app.open_task.unwrap();
        self.app
            .data
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .activity
            .retain(|a| !a.comment);
        self.settle();
        id
    }

    pub(super) fn comments(&self, id: u64) -> Vec<Activity> {
        self.app
            .task(id)
            .unwrap()
            .activity
            .iter()
            .filter(|a| a.comment)
            .cloned()
            .collect()
    }

    /// A fresh doc open in Edit with the cursor in its body.
    pub(super) fn new_doc_focused(&mut self) -> u64 {
        self.app.new_doc(None);
        self.settle();
        let id = self.app.doc_open.unwrap();
        self.ctx
            .memory_mut(|m| m.request_focus(egui::Id::new(("doc_body", id))));
        self.settle();
        id
    }

    pub(super) fn doc_body(&self, id: u64) -> String {
        self.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .body
            .clone()
    }

    pub(super) fn open_docs_list(&mut self) {
        self.app.section = Section::Docs;
        self.app.doc_open = None;
        self.app.hub.docs = DocFilter::All;
        self.settle();
    }

    // one frame with modifier keys held (egui reads them from the raw input)
    pub(super) fn frame_held(&mut self, events: Vec<egui::Event>, modifiers: egui::Modifiers) {
        self.time += 0.05;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), self.size)),
            time: Some(self.time),
            events,
            modifiers,
            ..Default::default()
        };
        self.app.fix_input(&self.ctx, &mut input);
        let app = &mut self.app;
        // keep what was drawn, for checks of what shows while a key is held
        self.shapes = self.ctx.run(input, |ctx| app.ui_frame(ctx)).shapes;
    }

    pub(super) fn click_with(&mut self, name: &str, modifiers: egui::Modifiers, dx: Option<f32>) {
        self.scroll_into_view(name);
        let r = self.rect(name);
        let pos = dx
            .map(|d| pos2(r.left() + d, r.center().y))
            .unwrap_or(r.center());
        self.frame_held(vec![egui::Event::PointerMoved(pos)], modifiers);
        self.frame_held(
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            }],
            modifiers,
        );
        self.frame_held(
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            }],
            modifiers,
        );
        self.settle();
    }

    /// Every text painted in the last frame.
    pub(super) fn texts(&self) -> Vec<String> {
        let mut out = vec![];
        each_text(&self.shapes, |ts, _| out.push(ts.galley.text().to_string()));
        out
    }

    /// The texts painted in `area` in the last frame, picked as `seen` says:
    /// (what it says, where).
    pub(super) fn texts_in(&self, area: Rect, seen: Seen) -> Vec<(String, Rect)> {
        let mut out = vec![];
        each_text(&self.shapes, |ts, clip| {
            let text = ts.galley.text();
            let r = ts.visual_bounding_rect();
            match seen {
                Seen::All => out.push((text.to_string(), r)),
                Seen::Touching => {
                    if area.intersects(r) && !text.trim().is_empty() {
                        out.push((text.to_string(), r));
                    }
                }
                Seen::Visible => {
                    let r = r.intersect(clip);
                    let text = text.trim();
                    if !text.is_empty() && r.is_positive() && area.contains(r.center()) {
                        out.push((text.to_string(), r));
                    }
                }
            }
        });
        out
    }

    /// Painted texts inside `area` that their clip rect cuts on the left or
    /// right (cut mid-glyph instead of shortened with an ellipsis).
    pub(super) fn cut_texts(&self, area: Rect) -> Vec<String> {
        let mut out = vec![];
        each_text(&self.shapes, |ts, clip| {
            let r = ts.visual_bounding_rect();
            let text = ts.galley.text().trim();
            // where it shows decides whether it is in the area
            if text.is_empty() || !area.contains(r.intersect(clip).center()) {
                return;
            }
            // scrolled out of view is fine; partly in view is cut
            if r.center().y < clip.top()
                || r.center().y > clip.bottom()
                || r.right() <= clip.left()
                || r.left() >= clip.right()
            {
                return;
            }
            if r.right() > clip.right() + 1.0 || r.left() < clip.left() - 1.0 {
                out.push(text.to_string());
            }
        });
        out
    }

    /// Controls on screen (vertically) that reach past the window's right
    /// edge.
    pub(super) fn past_right_edge(&self) -> Vec<String> {
        let w = self.size.x;
        let h = self.size.y;
        let mut out: Vec<String> = self
            .tree
            .iter()
            .filter(|(_, r)| r.top() >= 0.0 && r.bottom() <= h && r.right() > w + 1.0)
            .map(|(n, _)| n.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Clicks the lowest control with this name (a dialog's button, not
    /// its title).
    pub(super) fn click_lowest(&mut self, name: &str) {
        let r = self
            .tree
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, r)| *r)
            .max_by(|a, b| a.center().y.total_cmp(&b.center().y))
            .unwrap_or_else(|| panic!("nothing named {:?}", name));
        self.click_at(r.center());
    }

    /// What `pick` takes from the shapes painted in the last frame (nested
    /// ones included), in paint order.
    pub(super) fn shapes_where<T>(
        &self,
        mut pick: impl FnMut(&egui::Shape) -> Option<T>,
    ) -> Vec<T> {
        let mut out = vec![];
        each_shape(&self.shapes, |s, _| out.extend(pick(s)));
        out
    }

    /// The id of the channel (not a direct message) named `name`.
    pub(super) fn channel_id(&self, name: &str) -> u64 {
        crate::app::tests::channel_id(&self.app, name)
    }

    pub(super) fn count_named(&self, name: &str) -> usize {
        self.tree.iter().filter(|(n, _)| n == name).count()
    }

    pub(super) fn task_id(&self, name: &str) -> u64 {
        self.app
            .data
            .tasks
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id)
            .unwrap_or_else(|| panic!("no task {:?}", name))
    }
}

/// Red dots (the theme's danger color) painted inside the control named `name`.
pub(super) fn red_dots_in(h: &Ui1, name: &str) -> usize {
    dots_in(h, name, h.app.t().danger)
}

/// The number on the red unread pill painted inside the control named
/// `name`, if there is one; the number is painted white on the theme's danger.
pub(super) fn red_badge_in(h: &Ui1, name: &str) -> Option<String> {
    let danger = h.app.t().danger;
    let (_, area) = h.tree.iter().rev().find(|(n, _)| n == name)?;
    let pill = *h
        .shapes_where(|s| match s {
            egui::Shape::Rect(r) if r.fill == danger && area.contains(r.rect.center()) => {
                Some(r.rect)
            }
            _ => None,
        })
        .first()?;
    let mut text = None;
    each_text(&h.shapes, |ts, _| {
        let c = ts
            .galley
            .job
            .sections
            .first()
            .map_or(ts.fallback_color, |s| s.format.color);
        if pill.contains(ts.visual_bounding_rect().center()) && c == Color32::WHITE {
            text = Some(ts.galley.text().to_string());
        }
    });
    text
}

/// Filled circles of `color` painted inside the control named `name`.
pub(super) fn dots_in(h: &Ui1, name: &str, color: Color32) -> usize {
    let Some((_, rect)) = h.tree.iter().rev().find(|(n, _)| n == name) else {
        return 0;
    };
    dots_in_rect(h, *rect, color)
}

/// Filled dots of `color` painted inside `rect`.
pub(super) fn dots_in_rect(h: &Ui1, rect: egui::Rect, color: Color32) -> usize {
    h.shapes_where(|s| match s {
        egui::Shape::Circle(c) if c.fill == color && rect.contains(c.center) => Some(()),
        _ => None,
    })
    .len()
}

/// Grey presence rings ("not online") painted inside the control named `name`.
pub(super) fn rings_in(h: &Ui1, name: &str) -> usize {
    let Some((_, rect)) = h.tree.iter().rev().find(|(n, _)| n == name) else {
        return 0;
    };
    rings_in_rect(h, *rect)
}

/// Grey presence rings painted inside `rect`.
pub(super) fn rings_in_rect(h: &Ui1, rect: egui::Rect) -> usize {
    let grey = h.app.t().text3;
    h.shapes_where(|s| match s {
        egui::Shape::Circle(c)
            if c.stroke.color == grey && c.radius < 4.0 && rect.contains(c.center) =>
        {
            Some(())
        }
        _ => None,
    })
    .len()
}

// WCAG contrast ratio of two colors
pub(super) fn contrast(a: Color32, b: Color32) -> f32 {
    let lum = |c: Color32| {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

pub(super) fn task_named<'a>(h: &'a Ui1, name: &str) -> &'a Task {
    h.app
        .data
        .tasks
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("no task {:?}", name))
}

pub(super) fn cmd_key(h: &mut Ui1, key: egui::Key) {
    let cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    h.key_with(key, cmd);
}

pub(super) fn list_id(h: &Ui1, name: &str) -> u64 {
    h.app
        .data
        .spaces
        .iter()
        .flat_map(|s| {
            s.lists
                .iter()
                .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
        })
        .find(|l| l.name == name)
        .map(|l| l.id)
        .unwrap()
}

/// Calls `f` with every shape painted (the ones inside shape lists too, not
/// the lists themselves), in paint order, with the clip rect it was painted in.
pub(in crate::app) fn each_shape(
    shapes: &[egui::epaint::ClippedShape],
    mut f: impl FnMut(&egui::Shape, Rect),
) {
    fn walk(shape: &egui::Shape, clip: Rect, f: &mut dyn FnMut(&egui::Shape, Rect)) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, f)),
            s => f(s, clip),
        }
    }
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut f);
    }
}

/// Calls `f` with every text painted in the last frame (also the ones inside
/// shape lists), in paint order, with the clip rect it was painted in.
pub(super) fn each_text(
    shapes: &[egui::epaint::ClippedShape],
    mut f: impl FnMut(&egui::epaint::TextShape, Rect),
) {
    each_shape(shapes, |s, clip| {
        if let egui::Shape::Text(ts) = s {
            f(ts, clip)
        }
    });
}

/// Which painted texts `Ui1::texts_in` takes, and how.
pub(super) enum Seen {
    /// Every text, blank ones too, as painted with its whole rect, wherever
    /// it is (the area isn't looked at).
    All,
    /// Texts that aren't blank whose whole rect touches the area, as painted.
    Touching,
    /// Texts that aren't blank, trimmed, with the part their clip rect shows,
    /// whose shown middle is in the area.
    Visible,
}

// a server workspace whose server can't be reached, waited on until the
// app has tried and failed (nothing listens on port 1)
pub(super) fn server_down(h: &mut Ui1) {
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-down".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    // tests don't save, so the frame doesn't run the connection
    crate::app::tests::net_until(&mut h.app, "the app trying again", |a| {
        matches!(a.sync_rt.net_status, Some(wb_net::Status::Reconnecting(_)))
    });
    h.settle();
}

// send a message in #general and show its hover toolbar
pub(super) fn general_with_my_message(text: &str) -> (Ui1, u64) {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.send_message(text);
    let p = h.find(text).unwrap();
    h.hover(p);
    (h, cid)
}

pub(super) fn open_design_task(h: &mut Ui1) -> u64 {
    h.click_left("Task Design new homepage layout", 120.0);
    h.app.open_task.expect("the task window opens")
}

/// The seed with every List and task archived (the audit's "archived" state).
pub(super) fn archived_workspace() -> Ui1 {
    let mut h = Ui1::new();
    for s in h.app.data.spaces.iter_mut() {
        for l in s.lists.iter_mut() {
            l.archived = true;
        }
        for f in s.folders.iter_mut() {
            for l in f.lists.iter_mut() {
                l.archived = true;
            }
        }
    }
    for t in h.app.data.tasks.iter_mut() {
        t.archived = true;
    }
    h.app.section = Section::Home;
    h.app.loc = Location::Home;
    h.settle();
    h
}

/// A teammate's copy of `data` with one change of theirs: `change` runs on
/// it and the change is stamped a moment later.
pub(super) fn teammate_change(data: &Data, change: impl FnOnce(&mut Data)) -> serde_json::Value {
    let mut d = data.clone();
    change(&mut d);
    let mut v = serde_json::to_value(&d).unwrap();
    let mut meta = d.extra.sync.clone();
    crate::sync::stamp(&v, &mut meta, chrono::Utc::now().timestamp_millis() + 5);
    v["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    v
}

/// The sign-in screen's way in without an account: it names the computer.
pub(super) fn alone_button() -> &'static str {
    if cfg!(windows) {
        "Use it on this PC without an account"
    } else {
        "Use it on this Mac without an account"
    }
}
