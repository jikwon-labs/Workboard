// Voice huddles inside the app (stage 3). When the workspace is on a server,
// the Huddle button joins the conversation's LiveKit room: the server hands
// out a token for it, wb_voice runs the call, and a bar shows who is in it,
// who is speaking, mute and leave. Without a server the Huddle button opens
// a browser call as before (see together.rs).

use super::*;
use wb_voice::{Audio, Call, Event as VoiceEvent, Join, Person};

/// The `call` field of a chat message announcing an in-app huddle.
pub(super) const HUDDLE_PREFIX: &str = "workboard-huddle:";

pub struct HuddleRt {
    /// The conversation whose huddle this computer is in (or joining).
    pub(super) channel: Option<u64>,
    /// Asked the server for a token, waiting for it.
    pub(super) waiting: bool,
    pub(super) token: Option<Join>,
    pub(super) call: Option<Call>,
    pub(super) joined: bool,
    pub(super) muted: bool,
    pub(super) people: Vec<Person>,
    /// Microphone and speakers; tests send a tone instead.
    pub(super) audio: Audio,
}

impl Default for HuddleRt {
    fn default() -> Self {
        HuddleRt { channel: None, waiting: false, token: None, call: None, joined: false, muted: false, people: vec![], audio: if cfg!(test) { Audio::Tone(440.0) } else { Audio::Platform } }
    }
}

pub(super) fn huddle_channel(link: &str) -> Option<u64> {
    link.strip_prefix(HUDDLE_PREFIX)?.parse().ok()
}

impl App {
    /// In-app huddles need a signed-in server connection.
    pub(super) fn huddles_on_server(&self) -> bool {
        self.data.extra.shared.as_ref().is_some_and(|c| c.server.is_some()) && self.sync_rt.net_status == Some(wb_net::Status::Connected)
    }

    /// Start a huddle in a conversation and tell everyone in it.
    pub(super) fn start_huddle(&mut self, ch: u64) {
        let already = self.huddle.channel == Some(ch);
        self.join_huddle(ch);
        if already || self.huddle.channel != Some(ch) {
            return;
        }
        let id = self.data.new_id();
        let me = self.data.me;
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
            c.messages.push(Message { id, author: me, text: "Started a huddle".into(), ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: Some(format!("{}{}", HUDDLE_PREFIX, ch)) });
            c.last_read = now_ts();
        }
        self.section = Section::Chat;
        self.chat_open = Some(ch);
        self.dirty = true;
    }

    /// Join the huddle of a conversation (leaving any other one).
    pub(super) fn join_huddle(&mut self, ch: u64) {
        if self.huddle.channel == Some(ch) {
            return;
        }
        if !self.huddles_on_server() {
            self.toast = Some(("Huddles need the team's server. Connect in Work together.".into(), now_ts()));
            return;
        }
        let audio = self.huddle.audio;
        self.huddle = HuddleRt { channel: Some(ch), waiting: true, audio, ..Default::default() };
        self.access_request(wb_net::Msg::CallJoin { channel: ch });
    }

    pub(super) fn leave_huddle(&mut self) {
        let audio = self.huddle.audio;
        self.huddle = HuddleRt { audio, ..Default::default() };
    }

    pub(super) fn set_huddle_muted(&mut self, muted: bool) {
        self.huddle.muted = muted;
        if let Some(c) = &self.huddle.call {
            c.set_muted(muted);
        }
    }

    /// The server answered with a token.
    pub(super) fn huddle_token(&mut self, channel: u64, url: String, token: String) {
        if self.huddle.channel == Some(channel) && self.huddle.waiting {
            self.huddle.waiting = false;
            self.huddle.token = Some(Join { url, token });
        }
    }

    /// The server said no (not in the conversation, voice not set up...).
    pub(super) fn huddle_denied(&mut self) {
        if self.huddle.waiting {
            self.leave_huddle();
        }
    }

    /// Once per frame: start the call when the token is here, take in news.
    pub(super) fn huddle_tick(&mut self, ctx: Option<&egui::Context>) {
        if let Some(join) = self.huddle.token.take() {
            let wake: wb_voice::Wake = match ctx {
                Some(c) => {
                    let c = c.clone();
                    std::sync::Arc::new(move || c.request_repaint())
                }
                None => std::sync::Arc::new(|| {}),
            };
            let call = Call::start(join, self.huddle.audio, wake);
            if self.huddle.muted {
                call.set_muted(true);
            }
            self.huddle.call = Some(call);
        }
        let events = self.huddle.call.as_ref().map(|c| c.events()).unwrap_or_default();
        for e in events {
            match e {
                VoiceEvent::Joined => self.huddle.joined = true,
                VoiceEvent::People(p) => self.huddle.people = p,
                VoiceEvent::Left(why) => {
                    if let Some(why) = why {
                        self.toast = Some((why, now_ts()));
                    }
                    self.leave_huddle();
                }
            }
        }
        // the server connection went away or the workspace changed
        if self.huddle.channel.is_some() && !self.data.extra.shared.as_ref().is_some_and(|c| c.server.is_some()) {
            self.leave_huddle();
        }
    }

    fn huddle_name(&self, p: &Person) -> String {
        p.member().and_then(|m| self.data.member(m)).map(|m| m.name.clone()).unwrap_or_else(|| p.name.clone())
    }

    /// The bar at the bottom while in a huddle (above the message box, so it
    /// never covers it).
    pub(super) fn huddle_bar(&mut self, ctx: &egui::Context) {
        let Some(ch) = self.huddle.channel else { return };
        let t = self.t();
        let where_ = self.data.channels.iter().find(|c| c.id == ch).map(|c| if c.kind == ChannelKind::Channel { format!("#{}", c.name) } else { self.channel_label(c) }).unwrap_or_else(|| "a conversation".into());
        let green = Color32::from_rgb(0x00, 0x88, 0x44);
        let mut leave = false;
        let mut mute: Option<bool> = None;
        egui::Area::new(egui::Id::new("huddle_bar")).order(egui::Order::Foreground).anchor(Align2::CENTER_BOTTOM, [0.0, -132.0]).show(ctx, |ui| {
            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::symmetric(14, 10)).shadow(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 13.0, green);
                    super::shell::line_icon(ui.painter(), "phone", r.center(), Color32::WHITE);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("Huddle in {}", where_)).strong().color(t.text));
                        let status = if self.huddle.joined { match self.huddle.people.len() { 0 | 1 => "Only you so far".to_string(), n => format!("{} people", n) } } else { "Connecting\u{2026}".to_string() };
                        ui.label(RichText::new(status).small().color(t.text2));
                    });
                    ui.add_space(10.0);
                    for p in self.huddle.people.clone() {
                        let name = self.huddle_name(&p);
                        let (r, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                        let color = p.member().and_then(|m| self.data.member(m)).map(|m| rgb(m.color)).unwrap_or(t.text3);
                        if p.speaking && !p.muted {
                            ui.painter().circle_stroke(r.center(), 15.0, Stroke::new(2.5, green));
                        }
                        ui.painter().circle_filled(r.center(), 12.0, color);
                        let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
                        ui.painter().text(r.center(), Align2::CENTER_CENTER, initial, FontId::proportional(12.0), Color32::WHITE);
                        if p.muted {
                            ui.painter().circle_filled(r.right_bottom() - vec2(5.0, 5.0), 5.5, t.danger);
                            ui.painter().line_segment([r.right_bottom() - vec2(8.0, 8.0), r.right_bottom() - vec2(2.0, 2.0)], Stroke::new(1.5, Color32::WHITE));
                        }
                        let state = if p.muted { "muted" } else if p.speaking { "speaking" } else { "listening" };
                        let label = if p.me { format!("{} (you), {}", name, state) } else { format!("{}, {}", name, state) };
                        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
                        resp.on_hover_text(label);
                    }
                    ui.add_space(10.0);
                    let muted = self.huddle.muted;
                    if ui.add(egui::Button::new(if muted { "Unmute" } else { "Mute" }).min_size(vec2(72.0, 30.0)).corner_radius(7)).clicked() {
                        mute = Some(!muted);
                    }
                    if ui.add(egui::Button::new(RichText::new("Leave").color(Color32::WHITE)).fill(t.danger).min_size(vec2(64.0, 30.0)).corner_radius(7)).clicked() {
                        leave = true;
                    }
                });
            });
        });
        if let Some(m) = mute {
            self.set_huddle_muted(m);
        }
        if leave {
            self.leave_huddle();
        }
    }
}
