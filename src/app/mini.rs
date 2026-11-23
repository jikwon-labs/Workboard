// The small window: one conversation in a window of its own that stays
// above other apps, for keeping a chat or a huddle in sight while working
// in something else. Opened from the huddle bar or a conversation's header.
// When the huddle is in its conversation, the huddle's people, pictures and
// buttons move into it from the main window. Under them are the newest
// messages and a box to write in (text only: files go from the main window).

use super::huddle::HuddlePress;
use super::*;

/// The small window's message box.
pub(super) const MINI_BOX: &str = "mini_composer_text";
/// How large it first opens.
const MINI_W: f32 = 340.0;
const MINI_H: f32 = 520.0;
/// The newest messages it shows (older ones are in the main window).
const MINI_SHOWN: usize = 50;

#[derive(Default)]
pub(super) struct MiniUi {
    /// The conversation in the small window; None while it is closed.
    pub(super) channel: Option<u64>,
    /// What is being written in its box, and for which conversation (kept
    /// when it is closed and opened again on the same one).
    pub(super) text: String,
    pub(super) text_for: Option<u64>,
    /// Where it opens this time: where it was when last closed (None: the
    /// system's choice). Kept the same while it is open, so the window is
    /// never moved back while it is being dragged.
    open_at: Option<Rect>,
    /// Where it was when last closed.
    last_at: Option<Rect>,
    /// Where it is now (outer position, inner size).
    now_at: Option<Rect>,
    /// It is the window in front: its messages count as read.
    pub(super) focused: bool,
    /// It is in the Dock: nothing in it is seen.
    pub(super) minimized: bool,
    /// The messages go down to the newest on the next frame.
    to_bottom: bool,
    /// The input method fixes for its own input (ime.rs).
    ime: super::ime::ImeFix,
}

/// A message as the small window shows it.
struct MiniMsg {
    id: u64,
    who: String,
    time: String,
    title: Option<String>,
    text: String,
    replies: usize,
}

/// What happened in the small window this frame.
#[derive(Default)]
struct MiniOut {
    send: bool,
    typed: bool,
    open_main: bool,
    closed: bool,
    focused: bool,
    minimized: bool,
    at: Option<Rect>,
}

impl App {
    /// Opens conversation `ch` in the small window (in place of the one it
    /// shows, whose half-written words go).
    pub(super) fn open_mini(&mut self, ch: u64) {
        if self.mini.text_for != Some(ch) {
            self.mini.text.clear();
            self.mini.text_for = Some(ch);
        }
        if self.mini.channel.is_none() {
            self.mini.open_at = self.mini.last_at;
        }
        self.mini.channel = Some(ch);
        self.mini.to_bottom = true;
    }

    pub(super) fn close_mini(&mut self) {
        if self.mini.now_at.is_some() {
            self.mini.last_at = self.mini.now_at.take();
        }
        self.mini.channel = None;
        self.mini.focused = false;
        self.mini.minimized = false;
    }

    /// Closes the small window and forgets what was written in it: signed
    /// out, or another workspace.
    pub(super) fn forget_mini(&mut self) {
        self.close_mini();
        self.mini.text.clear();
        self.mini.text_for = None;
    }

    /// The huddle I'm in is in the small window's conversation (or a copy
    /// of that direct message), so it shows there and not in the main window.
    pub(super) fn mini_has_huddle(&self) -> bool {
        let (Some(m), Some(h)) = (self.mini.channel, self.huddle.channel) else {
            return false;
        };
        m == h
            || self
                .data
                .channels
                .iter()
                .find(|c| c.id == m)
                .is_some_and(|c| self.conversation(c).iter().any(|d| d.id == h))
    }

    /// The newest messages of conversation `c` and its copies, oldest first.
    fn mini_messages(&self, c: &Channel) -> Vec<MiniMsg> {
        let mut all: Vec<&Message> = self
            .conversation(c)
            .iter()
            .flat_map(|d| d.messages.iter())
            .collect();
        all.sort_by_key(|m| (m.ts, m.id));
        let skip = all.len().saturating_sub(MINI_SHOWN);
        all.into_iter()
            .skip(skip)
            .map(|m| {
                let mut text = super::chatfmt::plain(&super::chat::shown_text(m))
                    .trim()
                    .to_string();
                if !m.attachments.is_empty() {
                    let what = super::notify::sent_what(&m.attachments);
                    text = if text.is_empty() {
                        what.to_string()
                    } else {
                        format!("{}\n{}", text, what)
                    };
                }
                MiniMsg {
                    id: m.id,
                    who: m.bot.clone().unwrap_or_else(|| self.who(m.author)),
                    time: local(m.ts).map(|x| x.text("%-I:%M %p")).unwrap_or_default(),
                    title: m.post_title.clone().filter(|s| !s.trim().is_empty()),
                    text,
                    replies: m.replies.len(),
                }
            })
            .collect()
    }

    /// Once a frame: the small window, while it is open.
    pub(super) fn mini_window(&mut self, ctx: &egui::Context) {
        let Some(cid) = self.mini.channel else {
            self.mini.focused = false;
            return;
        };
        // its conversation deleted or out of reach (taken out of a private
        // channel): it closes
        let view = self
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .filter(|c| self.can_see(c))
            .map(|c| (self.channel_label(c), self.mini_messages(c)));
        let Some((label, msgs)) = view else {
            self.close_mini();
            return;
        };
        let huddle = self.mini_has_huddle();
        if huddle {
            self.huddle_textures(ctx);
        }
        let t = self.t();
        let mut builder = egui::ViewportBuilder::default()
            .with_title(label.clone())
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_min_inner_size([300.0, 360.0]);
        builder = match self.mini.open_at {
            Some(r) => builder.with_position(r.min).with_inner_size(r.size()),
            None => builder.with_inner_size([MINI_W, MINI_H]),
        };
        let mut press = HuddlePress::default();
        let mut out = MiniOut::default();
        let frame = egui::Frame::new()
            .fill(t.bg)
            .inner_margin(egui::Margin::symmetric(12, 10));
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("mini_window"),
            builder,
            |ctx, class| {
                if class == egui::ViewportClass::Embedded {
                    // one window only (the click tests): over the main
                    // window's top right
                    egui::Window::new("mini_window")
                        .title_bar(false)
                        .collapsible(false)
                        .resizable(false)
                        .fixed_size([MINI_W - 24.0, MINI_H - 20.0])
                        .default_pos(ctx.screen_rect().right_top() + vec2(-MINI_W - 40.0, 60.0))
                        .frame(
                            frame
                                .stroke(Stroke::new(1.0_f32, t.border))
                                .corner_radius(10),
                        )
                        .show(ctx, |ui| {
                            self.mini_body(
                                ui, &t, cid, &label, &msgs, huddle, &mut press, &mut out,
                            );
                        });
                    if huddle {
                        self.share_menu(ctx, &t, 380.0);
                    }
                    return;
                }
                super::ime::fix_window_input(&mut self.mini.ime, ctx);
                egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                    self.mini_body(ui, &t, cid, &label, &msgs, huddle, &mut press, &mut out);
                });
                if huddle {
                    let w = (ctx.screen_rect().width() - 40.0).clamp(240.0, 380.0);
                    self.share_menu(ctx, &t, w);
                }
                let (close, front, docked, outer, inner) = ctx.input(|i| {
                    let v = i.viewport();
                    (
                        v.close_requested(),
                        v.focused.unwrap_or(false),
                        v.minimized.unwrap_or(false),
                        v.outer_rect,
                        v.inner_rect,
                    )
                });
                out.closed |= close;
                out.focused = front;
                out.minimized = docked;
                if let (Some(o), Some(i)) = (outer, inner) {
                    out.at = Some(Rect::from_min_size(o.min, i.size()));
                }
            },
        );
        self.mini.focused = out.focused && !out.closed;
        self.mini.minimized = out.minimized;
        if out.at.is_some() {
            self.mini.now_at = out.at;
        }
        self.huddle_pressed(press);
        if out.typed {
            self.typing_now(cid, None);
        }
        if out.send {
            self.mini_send(cid);
        }
        if out.closed || out.open_main {
            self.close_mini();
            if out.open_main {
                self.open_chat_at(cid, None);
                ctx.send_viewport_cmd_to(
                    egui::ViewportId::ROOT,
                    egui::ViewportCommand::Minimized(false),
                );
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
            }
            return;
        }
        // read as it comes in while it is in front, or while the app is
        // (it stays in sight above the main window); not after "Mark unread"
        if (self.mini.focused || !self.notify_rt.away)
            && !self.mini.minimized
            && self.chat.keep_unread != Some(cid)
        {
            let due = self
                .data
                .channels
                .iter()
                .find(|c| c.id == cid)
                .is_some_and(|c| self.read_mark_due(c));
            if due {
                self.mark_read(cid);
                if self.persist {
                    self.dirty = true;
                    // this frame's saving and syncing are done already: the
                    // next frame takes it, at once
                    ctx.request_repaint();
                }
            }
        }
    }

    /// The small window's inside: the conversation's name and buttons, the
    /// huddle (when it is here), the messages and the box to write in.
    #[allow(clippy::too_many_arguments)]
    fn mini_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        cid: u64,
        label: &str,
        msgs: &[MiniMsg],
        huddle: bool,
        press: &mut HuddlePress,
        out: &mut MiniOut,
    ) {
        ui.horizontal(|ui| {
            let room = (ui.available_width() - 70.0).max(40.0);
            ui.scope(|ui| {
                ui.set_max_width(room);
                ui.add(
                    egui::Label::new(RichText::new(label).strong().size(15.0).color(t.text))
                        .truncate(),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let close =
                    super::icons::icon_button(ui, t, Icon::Close, "", "Close small window", false);
                if close.on_hover_text(tr("Close")).clicked() {
                    out.closed = true;
                }
                let main = super::icons::icon_button(
                    ui,
                    t,
                    Icon::External,
                    "",
                    "Open in the main window",
                    false,
                );
                if main.on_hover_text(tr("Open in the main window")).clicked() {
                    out.open_main = true;
                }
            });
        });
        if huddle {
            self.mini_huddle(ui, t, press);
        }
        ui.separator();
        // the box at the bottom, the messages above it in what is left
        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            // bottom up: the typing line goes under the box
            self.typing_row_for(ui, cid);
            self.mini_composer(ui, t, label, out);
            ui.add_space(6.0);
            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                let to_bottom = std::mem::take(&mut self.mini.to_bottom);
                egui::ScrollArea::vertical()
                    .id_salt(("mini_messages", cid))
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if msgs.is_empty() {
                            ui.label(RichText::new(tr("No messages yet")).color(t.text2));
                        }
                        for m in msgs {
                            ui.push_id(("mini_msg", m.id), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&m.who).strong().color(t.text));
                                    ui.label(RichText::new(&m.time).small().color(t.text2));
                                });
                                if let Some(title) = &m.title {
                                    ui.label(RichText::new(title).strong().color(t.text));
                                }
                                if !m.text.is_empty() {
                                    ui.add(
                                        egui::Label::new(RichText::new(&m.text).color(t.text))
                                            .wrap(),
                                    );
                                }
                                if m.replies > 0 {
                                    let n = if m.replies == 1 {
                                        tr("1 reply").to_string()
                                    } else {
                                        trf!("{} replies", m.replies)
                                    };
                                    ui.label(RichText::new(n).small().color(t.accent_text));
                                }
                                ui.add_space(8.0);
                            });
                        }
                        if to_bottom {
                            ui.scroll_to_cursor(Some(Align::BOTTOM));
                        }
                    });
            });
        });
    }

    /// The huddle in the small window: who is there, their pictures (a
    /// shared screen as wide as the window) and the buttons.
    fn mini_huddle(&mut self, ui: &mut Ui, t: &Theme, press: &mut HuddlePress) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.huddle_status()).small().color(t.text2));
            if self.huddle.screen.is_some() {
                ui.label(
                    RichText::new(tr("Sharing your screen"))
                        .small()
                        .color(super::huddle::HUDDLE_GREEN),
                );
            }
        });
        // the pictures and people scroll in at most about half the window,
        // so the buttons under them, the messages and the box always show
        // (a shared screen is tall)
        let room = (ui.available_height() * 0.45).max(60.0);
        egui::ScrollArea::vertical()
            .id_salt("mini_huddle")
            .max_height(room)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                self.mini_videos(ui, t);
                ui.horizontal_wrapped(|ui| {
                    for p in &self.huddle.people {
                        self.huddle_person(ui, t, p);
                    }
                });
            });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            self.huddle_buttons(ui, t, press);
        });
        ui.add_space(4.0);
    }

    /// The huddle's pictures, small: shared screens the window's width, the
    /// cameras in tiles under them. How large each is drawn goes to the
    /// call, as in the main window.
    fn mini_videos(&mut self, ui: &mut Ui, t: &Theme) {
        let Some((screens, cameras)) = self.huddle_shown_videos() else {
            self.huddle.focus = None;
            self.tell_view_sizes(std::collections::HashMap::new());
            return;
        };
        let ppp = ui.ctx().pixels_per_point();
        let full = ui.available_width().max(120.0);
        let mut drawn = std::collections::HashMap::new();
        for (id, name) in &screens {
            let Some(tex) = self.huddle.textures.get(id) else {
                continue;
            };
            let size = tex.size_vec2();
            let aspect = size.y / size.x.max(1.0);
            let w = full.min(220.0 / aspect.max(0.01));
            let r = ui.add(egui::Image::new((tex.id(), vec2(w, w * aspect))).corner_radius(6));
            r.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Image,
                    true,
                    format!("Screen of {}", name),
                )
            });
            drawn.insert(id.clone(), w * ppp);
            ui.label(
                RichText::new(trf!("{}'s screen", name))
                    .small()
                    .color(t.text2),
            );
        }
        let tile = ((full - 12.0) / 3.0).max(80.0);
        ui.horizontal_wrapped(|ui| {
            for (id, name) in &cameras {
                let Some(tex) = self.huddle.textures.get(id) else {
                    continue;
                };
                let size = tex.size_vec2();
                let h = tile * size.y / size.x.max(1.0);
                let r = ui.add(egui::Image::new((tex.id(), vec2(tile, h))).corner_radius(6));
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Image,
                        true,
                        format!("Video of {}", name),
                    )
                });
                let shown = if id == wb_voice::ME {
                    tr("You").to_string()
                } else {
                    name.clone()
                };
                r.on_hover_text(shown);
                drawn.insert(id.clone(), tile * ppp);
            }
        });
        // a big picture picked in the main window means nothing here
        self.huddle.focus = None;
        drawn.remove(wb_voice::ME);
        self.tell_view_sizes(drawn);
    }

    /// The box at the bottom of the small window: Enter sends, Shift+Enter
    /// starts a new line.
    fn mini_composer(&mut self, ui: &mut Ui, t: &Theme, label: &str, out: &mut MiniOut) {
        let id = egui::Id::new(MINI_BOX);
        ui.horizontal(|ui| {
            let enter = super::chat::take_enter(ui, id);
            let send_w = 34.0;
            let r = ui.add_sized(
                vec2((ui.available_width() - send_w - 6.0).max(60.0), 34.0),
                egui::TextEdit::multiline(&mut self.mini.text)
                    .id(id)
                    .hint_text(trf!("Message {}", label))
                    .desired_rows(1),
            );
            r.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::TextEdit,
                    true,
                    "Small window message box",
                )
            });
            let ready = !self.mini.text.trim().is_empty();
            if r.changed() && ready {
                out.typed = true;
            }
            let send = ui.add_enabled_ui(ready, |ui| {
                super::icons::icon_button(ui, t, Icon::Send, "", "Send from the small window", true)
            });
            let send = send.inner.on_hover_text(tr("Send"));
            if ready && (enter || send.clicked()) {
                out.send = true;
                if send.clicked() {
                    ui.memory_mut(|m| m.request_focus(id));
                }
            }
        });
    }

    /// Sends what is in the small window's box to conversation `cid`, as the
    /// main window's box would (commands and @mentions too), without
    /// touching what waits in the main window's box.
    fn mini_send(&mut self, cid: u64) {
        let text = self.mini.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.mini.text.clear();
        let main_box = std::mem::take(&mut self.chat.channel);
        self.send_chat(cid, text, None, vec![]);
        let used = std::mem::replace(&mut self.chat.channel, main_box);
        // a command it doesn't know comes back to the box
        if !used.text.is_empty() {
            self.mini.text = used.text;
        }
        self.mini.to_bottom = true;
    }
}
