// Pages in windows of their own: a doc, a whiteboard, a meeting, a task or
// a huddle's shared screens, moved out of the main window so they stay in
// sight beside a conversation (before, keeping up with a chat meant going
// back and forth between the page and the chat).
//
// Each is a separate window of the system, moved and sized on its own, with
// a bar on top whose "Back to main window" puts the page back where it came
// from; closing the window just closes it. While a page is in its window the
// main window doesn't show the same one: opening it there brings its window
// to the front instead. Where there is only one window (the click tests) it
// is a window inside the app's, as the thread's window is (chat.rs).

use super::icons::icon_button;
use super::*;
use egui::ViewportCommand;

/// What a window of its own shows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) enum Pop {
    Doc(u64),
    Board(u64),
    Meeting(u64),
    Task(u64),
    /// The shared screens of the huddle you are in.
    Screen,
}

/// One page's window.
pub(super) struct PopWin {
    pub what: Pop,
    /// Where it opens (the screen's points); kept as it was so the window
    /// isn't moved back there every frame.
    at: Option<egui::Pos2>,
    ime: super::ime::ImeFix,
    /// To be brought to the front in the next frame.
    raise: bool,
    /// The doc's Preview switch in this window.
    doc_preview: bool,
    /// The board's pick, drags and the like in this window.
    wb: more::WbMoment,
}

#[derive(Default)]
pub(super) struct PopUi {
    pub wins: Vec<PopWin>,
    /// The window being drawn now (None: the main window).
    pub inside: Option<Pop>,
    /// Asked this frame: move this page out to a window of its own.
    pub out: Option<Pop>,
    /// Asked from inside a window this frame: put its page back in the
    /// main window.
    pub back: bool,
}

/// What a window asked for while it was drawn.
#[derive(Default)]
struct PopOut {
    close: bool,
    back: bool,
}

impl App {
    /// The page `what` is in a window of its own.
    pub(super) fn popped(&self, what: Pop) -> bool {
        self.pop.wins.iter().any(|w| w.what == what)
    }

    /// Drawing inside one of the pages' windows now.
    pub(super) fn in_popout(&self) -> bool {
        self.pop.inside.is_some()
    }

    /// The "Open in new window" button of a page, at `ui`'s next place:
    /// asks for `what` to move out.
    pub(super) fn pop_out_button(&mut self, ui: &mut Ui, what: Pop) {
        if self.in_popout() {
            return;
        }
        let t = self.t();
        let b = icon_button(ui, &t, Icon::PopOut, "", "Open in new window", false)
            .on_hover_text(tr("Open in new window"));
        if b.clicked() {
            self.pop.out = Some(what);
        }
    }

    /// Before the main window is drawn: a page that is in its own window
    /// isn't shown there too; asking for it there brings its window up.
    pub(super) fn popout_guard(&mut self) {
        for w in &mut self.pop.wins {
            let shown_here = match w.what {
                Pop::Doc(id) => self.doc_open == Some(id),
                Pop::Board(id) => self.sweep.wb.open == Some(id),
                Pop::Meeting(id) => self.meeting_open == Some(id),
                Pop::Task(id) => self.open_task == Some(id),
                Pop::Screen => false,
            };
            if !shown_here {
                continue;
            }
            match w.what {
                Pop::Doc(_) => self.doc_open = None,
                Pop::Board(_) => self.sweep.wb.open = None,
                Pop::Meeting(_) => self.meeting_open = None,
                Pop::Task(_) => self.open_task = None,
                Pop::Screen => {}
            }
            w.raise = true;
        }
    }

    /// Moves the page `what` out of the main window into one of its own
    /// (or brings its window up when it has one already).
    pub(super) fn pop_out(&mut self, what: Pop, ctx: &egui::Context) {
        if let Some(w) = self.pop.wins.iter_mut().find(|w| w.what == what) {
            w.raise = true;
            return;
        }
        // a little in from the main window's top left
        let at = ctx
            .input(|i| i.viewport().inner_rect)
            .map(|r| r.min + vec2(60.0, 50.0));
        let mut w = PopWin {
            what,
            at,
            ime: Default::default(),
            raise: false,
            doc_preview: false,
            wb: Default::default(),
        };
        match what {
            Pop::Doc(id) => {
                if self.doc_open == Some(id) {
                    w.doc_preview = self.doc_preview;
                    self.doc_open = None;
                    self.doc_preview = false;
                }
            }
            Pop::Board(id) => {
                if self.sweep.wb.open == Some(id) {
                    // its pick and the rest go with it
                    self.sweep.wb.swap_moment(&mut w.wb);
                } else {
                    w.wb.open = Some(id);
                }
            }
            Pop::Meeting(id) => {
                if self.meeting_open == Some(id) {
                    self.meeting_open = None;
                }
            }
            Pop::Task(id) => {
                if self.open_task == Some(id) {
                    self.open_task = None;
                }
            }
            Pop::Screen => {}
        }
        self.pop.wins.push(w);
    }

    /// Puts the page of a window back in the main window, as it was before
    /// it moved out, and brings the main window up.
    fn pop_back(&mut self, w: &mut PopWin, ctx: &egui::Context) {
        match w.what {
            Pop::Doc(id) => {
                self.section = Section::Docs;
                self.doc_open = Some(id);
                self.doc_preview = w.doc_preview;
            }
            Pop::Board(id) => {
                self.section = Section::Whiteboards;
                w.wb.open = Some(id);
                self.sweep.wb.swap_moment(&mut w.wb);
            }
            Pop::Meeting(id) => {
                self.section = Section::Meetings;
                self.meeting_open = Some(id);
            }
            Pop::Task(id) => self.open_task = Some(id),
            Pop::Screen => {}
        }
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, ViewportCommand::Focus);
    }

    /// The window's title, or None when its page is gone (deleted, taken
    /// out of reach, the huddle over): the window then closes.
    fn pop_title(&self, what: Pop) -> Option<String> {
        let or = |name: &str, none: &str| -> String {
            if name.trim().is_empty() {
                tr(none).to_string()
            } else {
                name.to_string()
            }
        };
        match what {
            Pop::Doc(id) => self
                .data
                .docs
                .iter()
                .find(|d| d.id == id)
                .map(|d| or(&d.title, "Untitled")),
            Pop::Board(id) => self
                .data
                .extra
                .whiteboards
                .iter()
                .find(|w| w.id == id)
                .map(|w| or(&w.name, "Untitled whiteboard")),
            Pop::Meeting(id) => self
                .data
                .extra
                .meetings
                .iter()
                .find(|m| m.id == id)
                .map(|m| or(&m.title, "Meeting")),
            Pop::Task(id) => self.task(id).map(|t| or(&t.name, "Task")),
            Pop::Screen => {
                let ch = self.huddle.channel?;
                self.huddle_has_others_screen()
                    .then(|| format!("{} \u{00B7} {}", tr("Shared screen"), self.huddle_place(ch)))
            }
        }
    }

    /// Once a frame: the pages' windows, and the page asked to move out.
    pub(super) fn popout_windows(&mut self, ctx: &egui::Context) {
        let wins = std::mem::take(&mut self.pop.wins);
        let mut keep: Vec<PopWin> = vec![];
        for mut w in wins {
            if self.popout_window(ctx, &mut w) && !keep.iter().any(|k| k.what == w.what) {
                keep.push(w);
            }
        }
        // asked from inside a window (a task opened from a doc's link and
        // moved out, say): opened from the next frame on
        let more = std::mem::take(&mut self.pop.wins);
        self.pop.wins = keep;
        for w in more {
            if !self.popped(w.what) {
                self.pop.wins.push(w);
            }
        }
        // the page was drawn in the main window this frame: its window
        // shows it from the next frame on
        if let Some(what) = self.pop.out.take() {
            self.pop_out(what, ctx);
            ctx.request_repaint();
        }
    }

    /// One page's window; false once it closed.
    fn popout_window(&mut self, ctx: &egui::Context, w: &mut PopWin) -> bool {
        let Some(title) = self.pop_title(w.what) else {
            return false;
        };
        let (min, size) = match w.what {
            Pop::Doc(_) => ([420.0, 360.0], [900.0, 760.0]),
            Pop::Board(_) => ([480.0, 360.0], [1040.0, 740.0]),
            Pop::Meeting(_) => ([380.0, 360.0], [720.0, 760.0]),
            Pop::Task(_) => ([480.0, 420.0], [960.0, 760.0]),
            Pop::Screen => ([360.0, 240.0], [1120.0, 720.0]),
        };
        let mut builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_min_inner_size(min)
            .with_inner_size(size);
        if let Some(at) = w.at {
            builder = builder.with_position(at);
        }
        let id = egui::ViewportId::from_hash_of(("popout", w.what));
        if std::mem::take(&mut w.raise) {
            ctx.send_viewport_cmd_to(id, ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd_to(id, ViewportCommand::Focus);
        }
        let t = self.t();
        let mut out = PopOut::default();
        let fill = if w.what == Pop::Screen {
            super::huddle::PANEL_BG
        } else {
            t.bg
        };
        ctx.show_viewport_immediate(id, builder, |ctx, class| {
            if class == egui::ViewportClass::Embedded {
                // one window only (the click tests): a window inside it
                let frame = egui::Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(1.0_f32, t.border))
                    .corner_radius(10);
                let room = ctx.screen_rect().shrink(8.0).size();
                let win = egui::Window::new(format!("popout {:?}", w.what))
                    .id(egui::Id::new(("popout_window", w.what)))
                    .title_bar(false)
                    .collapsible(false)
                    .default_size(vec2(size[0], size[1]).min(room))
                    .default_pos(ctx.screen_rect().min + vec2(40.0, 40.0))
                    .frame(frame);
                crate::widgets::window_inside(win, ctx, &frame).show(ctx, |ui| {
                    ui.set_min_size(vec2(size[0], size[1]).min(room - vec2(16.0, 16.0)));
                    self.popout_body(ui, w, &mut out);
                });
                return;
            }
            super::ime::fix_window_input(&mut w.ime, ctx);
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(fill))
                .show(ctx, |ui| self.popout_body(ui, w, &mut out));
            // pictures and videos dragged in from Finder attach to the task
            if let Pop::Task(tid) = w.what {
                for f in ctx.input(|i| {
                    i.raw
                        .dropped_files
                        .iter()
                        .filter_map(|f| f.path.clone())
                        .collect::<Vec<_>>()
                }) {
                    self.add_task_file(tid, f);
                }
            }
            out.close |= ctx.input(|i| i.viewport().close_requested());
        });
        if out.back {
            self.pop_back(w, ctx);
            return false;
        }
        !out.close
    }

    /// Inside a page's window: the bar on top and the page under it.
    fn popout_body(&mut self, ui: &mut Ui, w: &mut PopWin, out: &mut PopOut) {
        let t = self.t();
        let was_tier = self.tier;
        // laid out for this window's width, not the main one's
        self.tier = super::shell::Tier::of(ui.max_rect().width());
        self.pop.inside = Some(w.what);
        let what = w.what;
        ui.push_id(("popout", what), |ui| {
            let screen = what == Pop::Screen;
            egui::TopBottomPanel::top(egui::Id::new(("popout_bar", what)))
                .frame(
                    egui::Frame::new()
                        .fill(if screen {
                            super::huddle::PANEL_BG
                        } else {
                            t.sidebar
                        })
                        .inner_margin(egui::Margin::symmetric(10, 6)),
                )
                .show_separator_line(!screen)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        let kind = match what {
                            Pop::Doc(_) => "Doc",
                            Pop::Board(_) => "Whiteboard",
                            Pop::Meeting(_) => "Meeting",
                            Pop::Task(_) => "Task",
                            Pop::Screen => "Shared screen",
                        };
                        let fg = if screen {
                            Color32::from_gray(0xab)
                        } else {
                            t.text3
                        };
                        ui.label(RichText::new(tr(kind)).small().color(fg));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let b = ui
                                .add(ghost(egui::Button::new(
                                    RichText::new(tr("Back to main window")).color(fg),
                                )))
                                .on_hover_text(tr("Show it in the main window again"));
                            name_it(&b, "Back to main window");
                            if b.clicked() {
                                out.back = true;
                            }
                        });
                    });
                });
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(if screen {
                    super::huddle::PANEL_BG
                } else {
                    t.bg
                }))
                .show_inside(ui, |ui| match what {
                    Pop::Doc(id) => {
                        let main = (self.doc_open, self.doc_preview);
                        self.doc_open = Some(id);
                        self.doc_preview = w.doc_preview;
                        self.doc_editor(ui);
                        let now = self.doc_open;
                        w.doc_preview = self.doc_preview;
                        (self.doc_open, self.doc_preview) = main;
                        match now {
                            // another page of the doc picked in its Pages panel
                            Some(n) => w.what = Pop::Doc(n),
                            None => out.close = true,
                        }
                    }
                    Pop::Board(id) => {
                        w.wb.open = Some(id);
                        self.sweep.wb.swap_moment(&mut w.wb);
                        self.whiteboard_canvas(ui);
                        self.sweep.wb.swap_moment(&mut w.wb);
                        if w.wb.open != Some(id) {
                            out.close = true;
                        }
                    }
                    Pop::Meeting(id) => {
                        let main = self.meeting_open;
                        self.meeting_open = Some(id);
                        self.meeting_body(ui, false);
                        let now = self.meeting_open;
                        self.meeting_open = main;
                        if now != Some(id) {
                            out.close = true;
                        }
                    }
                    Pop::Task(id) => {
                        let main = self.open_task;
                        self.open_task = Some(id);
                        let closed = self.task_popout_body(ui, id);
                        let now = self.open_task;
                        self.open_task = main;
                        match now {
                            // previous / next, or a subtask opened from it
                            Some(n) if !closed => w.what = Pop::Task(n),
                            _ => out.close = true,
                        }
                    }
                    Pop::Screen => self.screen_window_body(ui),
                });
        });
        self.pop.inside = None;
        self.tier = was_tier;
        if std::mem::take(&mut self.pop.back) {
            out.back = true;
        }
    }

    /// The doc whose editing version history follows (doc_versions.rs):
    /// the main window's, else the one in a window of its own.
    pub(super) fn doc_being_edited(&self) -> Option<u64> {
        self.doc_open.or_else(|| {
            self.pop.wins.iter().find_map(|w| match w.what {
                Pop::Doc(id) => Some(id),
                _ => None,
            })
        })
    }
}
