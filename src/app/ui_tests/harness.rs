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
    pub(super) size: egui::Vec2,
    pub(super) shapes: Vec<egui::epaint::ClippedShape>,
    /// files dropped on the window in the next frame
    pub(super) dropped: Vec<egui::DroppedFile>,
    /// the mouse cursor the last frame asked for
    pub(super) cursor: egui::CursorIcon,
    /// whether the window is in front (false: another app is)
    pub(super) focused: bool,
}

impl Ui1 {
    pub(super) fn new() -> Self {
        Self::sized(1440.0, 900.0)
    }

    // the smallest window the app allows is 900x560
    pub(super) fn sized(w: f32, h: f32) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut h = Ui1 {
            ctx,
            app: App::build(None, Some(Data::seed())),
            time: 0.0,
            tree: vec![],
            size: vec2(w, h),
            shapes: vec![],
            dropped: vec![],
            cursor: egui::CursorIcon::Default,
            focused: true,
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
            focused: self.focused,
            ..Default::default()
        };
        // as eframe reports it, read by the app through viewport().focused
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .focused = Some(self.focused);
        // as eframe's raw_input_hook does
        self.app.fix_input(&self.ctx, &mut input);
        let app = &mut self.app;
        let out = self.ctx.run(input, |ctx| app.ui_frame(ctx));
        self.shapes = out.shapes;
        self.cursor = out.platform_output.cursor_icon;
        if let Some(update) = out.platform_output.accesskit_update {
            self.tree.clear();
            for (_, node) in update.nodes {
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
                }
            }
        }
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
        if !self.has(name) && self.app.modal.is_some() {
            // maybe further down a dialog that scrolls
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

    // turn the mouse wheel over the open dialog until a control in it shows
    pub(super) fn scroll_dialog_to(&mut self, name: &str) {
        for _ in 0..30 {
            if self.has(name) {
                break;
            }
            let Some(r) = self.try_modal_rect() else {
                return;
            };
            let at = r.center();
            self.frame(vec![egui::Event::PointerMoved(at)]);
            self.frame(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, -120.0),
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
        self.app.hub_filter = 0;
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
    let danger = h.app.t().danger;
    let Some((_, rect)) = h.tree.iter().rev().find(|(n, _)| n == name) else {
        return 0;
    };
    fn count(s: &egui::Shape, rect: egui::Rect, danger: Color32) -> usize {
        match s {
            egui::Shape::Circle(c) if c.fill == danger && rect.contains(c.center) => 1,
            egui::Shape::Vec(v) => v.iter().map(|s| count(s, rect, danger)).sum(),
            _ => 0,
        }
    }
    h.shapes
        .iter()
        .map(|c| count(&c.shape, *rect, danger))
        .sum()
}

/// The number on the red unread pill painted inside the control named
/// `name`, if there is one; the number is painted white on the theme's danger.
pub(super) fn red_badge_in(h: &Ui1, name: &str) -> Option<String> {
    let danger = h.app.t().danger;
    let (_, area) = h.tree.iter().rev().find(|(n, _)| n == name)?;
    fn pills(s: &egui::Shape, area: Rect, danger: Color32, out: &mut Vec<Rect>) {
        match s {
            egui::Shape::Rect(r) if r.fill == danger && area.contains(r.rect.center()) => {
                out.push(r.rect)
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| pills(s, area, danger, out)),
            _ => {}
        }
    }
    let mut found = vec![];
    for c in &h.shapes {
        pills(&c.shape, *area, danger, &mut found);
    }
    let pill = *found.first()?;
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
    fn count(s: &egui::Shape, rect: egui::Rect, color: Color32) -> usize {
        match s {
            egui::Shape::Circle(c) if c.fill == color && rect.contains(c.center) => 1,
            egui::Shape::Vec(v) => v.iter().map(|s| count(s, rect, color)).sum(),
            _ => 0,
        }
    }
    h.shapes.iter().map(|c| count(&c.shape, rect, color)).sum()
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
    fn count(s: &egui::Shape, rect: egui::Rect, grey: Color32) -> usize {
        match s {
            egui::Shape::Circle(c)
                if c.stroke.color == grey && c.radius < 4.0 && rect.contains(c.center) =>
            {
                1
            }
            egui::Shape::Vec(v) => v.iter().map(|s| count(s, rect, grey)).sum(),
            _ => 0,
        }
    }
    h.shapes.iter().map(|c| count(&c.shape, rect, grey)).sum()
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

/// Calls `f` with every text painted in the last frame (also the ones inside
/// shape lists), in paint order, with the clip rect it was painted in.
pub(super) fn each_text(
    shapes: &[egui::epaint::ClippedShape],
    mut f: impl FnMut(&egui::epaint::TextShape, Rect),
) {
    fn walk(shape: &egui::Shape, clip: Rect, f: &mut dyn FnMut(&egui::epaint::TextShape, Rect)) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, f)),
            egui::Shape::Text(ts) => f(ts, clip),
            _ => {}
        }
    }
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut f);
    }
}

/// Every text painted in the last frame.
pub(super) fn drawn_texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    let mut out = vec![];
    each_text(shapes, |ts, _| out.push(ts.galley.text().to_string()));
    out
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
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !matches!(
        h.app.sync_rt.net_status,
        Some(wb_net::Status::Reconnecting(_))
    ) && std::time::Instant::now() < until
    {
        // tests don't save, so the frame doesn't run the connection
        h.app.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.settle();
}

// send a message in #general and show its hover toolbar
pub(super) fn general_with_my_message(text: &str) -> (Ui1, u64) {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    h.click("Message composer");
    h.type_text(text);
    h.key(egui::Key::Enter);
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
