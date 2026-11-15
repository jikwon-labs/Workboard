// Voice huddles inside the app (stage 3). When the workspace is on a server,
// the Huddle button joins the conversation's LiveKit room: the server hands
// out a token for it, wb_voice runs the call, and a bar shows who is in it,
// who is speaking, mute and leave. Without a server the Huddle button opens
// a browser call as before (see together.rs). Video (stage 4): the Video
// button sends the camera, and everyone's pictures show above the bar.

use super::*;
use std::collections::HashMap;
use wb_voice::{Audio, Call, Event as VoiceEvent, Join, Person, Picture, Video, ME};

/// The `call` field of a chat message announcing an in-app huddle.
pub(super) const HUDDLE_PREFIX: &str = "workboard-huddle:";

/// How long to wait for the server's answer to joining a huddle.
pub(super) const HUDDLE_JOIN_SECS: u64 = 15;

pub struct HuddleRt {
    /// The conversation whose huddle this computer is in (or joining).
    pub(super) channel: Option<u64>,
    /// Asked the server for a token, waiting for it.
    pub(super) waiting: bool,
    /// When the token was asked for (gives up after HUDDLE_JOIN_SECS).
    pub(super) asked: Option<std::time::Instant>,
    /// Started here: post "Started a huddle" once the server lets us in.
    pub(super) announce: bool,
    pub(super) token: Option<Join>,
    pub(super) call: Option<Call>,
    pub(super) joined: bool,
    pub(super) muted: bool,
    pub(super) people: Vec<Person>,
    /// Microphone and speakers; tests send a tone instead.
    pub(super) audio: Audio,
    /// Sending video.
    pub(super) camera: bool,
    /// The camera; tests send a colored picture instead.
    pub(super) video: Video,
    /// Latest picture of everyone sending video (identity, or ME), and
    /// whether it still has to go to the screen.
    pub(super) pictures: HashMap<String, (Picture, bool)>,
    textures: HashMap<String, egui::TextureHandle>,
    /// The video shown large (clicked), if any.
    pub(super) focus: Option<String>,
}

impl Default for HuddleRt {
    fn default() -> Self {
        HuddleRt {
            channel: None,
            waiting: false,
            asked: None,
            announce: false,
            token: None,
            call: None,
            joined: false,
            muted: false,
            people: vec![],
            audio: if cfg!(test) { Audio::Tone(440.0) } else { Audio::Platform },
            camera: false,
            // WORKBOARD_TEST_VIDEO=1: a colored test picture instead of the camera
            // (to check video on a computer without a camera)
            video: if cfg!(test) || std::env::var("WORKBOARD_TEST_VIDEO").is_ok_and(|v| v == "1") { Video::Pattern([200, 60, 60]) } else { Video::Camera },
            pictures: HashMap::new(),
            textures: HashMap::new(),
            focus: None,
        }
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
        if !already && self.huddles_on_server() {
            // a conversation made a moment ago goes to the server first (the
            // copy is sent before the call request), so the server knows it
            self.sync_note_change();
            self.write_shared();
        }
        self.join_huddle(ch);
        if already || self.huddle.channel != Some(ch) {
            return;
        }
        // the "Started a huddle" card goes out when the token comes (huddle_token)
        self.huddle.announce = true;
        self.section = Section::Chat;
        self.chat_open = Some(ch);
    }

    /// Tell everyone in the conversation that a huddle started.
    fn post_huddle_card(&mut self, ch: u64) {
        let id = self.data.new_id();
        let me = self.data.me;
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
            c.messages.push(Message { edited: None, attachments: vec![], id, author: me, text: "Started a huddle".into(), ts: now_ts(), reactions: vec![], replies: vec![], saved: false, task: None, post_title: None, post_type: String::new(), pinned: false, call: Some(format!("{}{}", HUDDLE_PREFIX, ch)) });
            c.last_read = now_ts();
        }
        self.dirty = true;
    }

    /// Join the huddle of a conversation (leaving any other one).
    pub(super) fn join_huddle(&mut self, ch: u64) {
        if self.huddle.channel == Some(ch) {
            return;
        }
        if !self.huddles_on_server() {
            self.toast = Some((tr("Huddles need the team's server. Connect in Work together.").into(), now_ts()));
            return;
        }
        let (audio, video) = (self.huddle.audio, self.huddle.video);
        self.huddle = HuddleRt { channel: Some(ch), waiting: true, asked: Some(std::time::Instant::now()), audio, video, ..Default::default() };
        self.access_request(wb_net::Msg::CallJoin { channel: ch });
    }

    pub(super) fn leave_huddle(&mut self) {
        let (audio, video) = (self.huddle.audio, self.huddle.video);
        self.huddle = HuddleRt { audio, video, ..Default::default() };
    }

    pub(super) fn set_huddle_camera(&mut self, on: bool) {
        self.huddle.camera = on;
        if let Some(c) = &self.huddle.call {
            c.set_camera(on);
        }
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
            if std::mem::take(&mut self.huddle.announce) {
                self.post_huddle_card(channel);
            }
        }
    }

    /// The server said no to this conversation's huddle (not in it, voice
    /// not set up...).
    pub(super) fn huddle_denied(&mut self, channel: u64) {
        if self.huddle.waiting && self.huddle.channel == Some(channel) {
            self.leave_huddle();
        }
    }

    /// Once per frame: start the call when the token is here, take in news.
    pub(super) fn huddle_tick(&mut self, ctx: Option<&egui::Context>) {
        if self.huddle.waiting && self.huddle.asked.is_some_and(|t| t.elapsed() >= std::time::Duration::from_secs(HUDDLE_JOIN_SECS)) {
            self.leave_huddle();
            self.toast = Some((tr("The server didn't answer, so the huddle didn't start. Try again in a moment.").into(), now_ts()));
        } else if self.huddle.waiting {
            if let Some(c) = ctx {
                // come back to check even when nothing else happens
                c.request_repaint_after(std::time::Duration::from_secs(1));
            }
        }
        if let Some(join) = self.huddle.token.take() {
            let wake: wb_voice::Wake = match ctx {
                Some(c) => {
                    let c = c.clone();
                    std::sync::Arc::new(move || c.request_repaint())
                }
                None => std::sync::Arc::new(|| {}),
            };
            let call = Call::start(join, self.huddle.audio, self.huddle.video, wake);
            if self.huddle.muted {
                call.set_muted(true);
            }
            if self.huddle.camera {
                call.set_camera(true);
            }
            self.huddle.call = Some(call);
        }
        let events = self.huddle.call.as_ref().map(|c| c.events()).unwrap_or_default();
        for e in events {
            match e {
                VoiceEvent::Joined => self.huddle.joined = true,
                VoiceEvent::People(p) => self.huddle.people = p,
                VoiceEvent::Trouble(why) => {
                    // so far only the camera says this: turn it back off
                    self.set_huddle_camera(false);
                    self.toast = Some((why, now_ts()));
                }
                VoiceEvent::Left(why) => {
                    if let Some(why) = why {
                        self.toast = Some((why, now_ts()));
                    }
                    self.leave_huddle();
                }
            }
        }
        let pictures = self.huddle.call.as_ref().map(|c| c.new_pictures()).unwrap_or_default();
        for (id, p) in pictures {
            match p {
                Some(p) => {
                    self.huddle.pictures.insert(id, (p, true));
                }
                None => {
                    self.huddle.pictures.remove(&id);
                    self.huddle.textures.remove(&id);
                }
            }
        }
        // the server connection went away or the workspace changed
        if self.huddle.channel.is_some() && !self.data.extra.shared.as_ref().is_some_and(|c| c.server.is_some()) {
            self.leave_huddle();
        }
    }

    /// Everyone's video above the bar: others first, then you.
    fn huddle_videos(&mut self, ctx: &egui::Context, t: &Theme) {
        if self.huddle.pictures.is_empty() {
            return;
        }
        for (id, (pic, fresh)) in self.huddle.pictures.iter_mut() {
            if !*fresh {
                continue;
            }
            *fresh = false;
            let img = egui::ColorImage::from_rgba_unmultiplied([pic.width as usize, pic.height as usize], &pic.rgba);
            match self.huddle.textures.get_mut(id) {
                Some(tex) => tex.set(img, egui::TextureOptions::LINEAR),
                None => {
                    let tex = ctx.load_texture(format!("huddle-video-{}", id), img, egui::TextureOptions::LINEAR);
                    self.huddle.textures.insert(id.clone(), tex);
                }
            }
        }
        // only people still in the call: a picture can outlive its sender by a frame
        let mut ids: Vec<String> = self.huddle.textures.keys().filter(|id| *id == ME || self.huddle.people.iter().any(|p| &p.identity == *id)).cloned().collect();
        if ids.is_empty() {
            self.huddle.focus = None;
            return;
        }
        ids.sort_by_key(|id| (id == ME, id.clone()));
        let names: Vec<(String, String)> = ids
            .iter()
            .map(|id| {
                let name = if id == ME { "You".to_string() } else { self.huddle.people.iter().find(|p| &p.identity == id).map(|p| self.huddle_name(p)).unwrap_or_default() };
                (id.clone(), name)
            })
            .collect();
        // a clicked video shows large and the rest small; otherwise tiles get
        // smaller when many people send video
        if self.huddle.focus.as_ref().is_some_and(|f| !ids.contains(f)) {
            self.huddle.focus = None;
        }
        let focus = self.huddle.focus.clone();
        let normal = if ids.len() <= 2 { 320.0 } else if ids.len() <= 4 { 240.0 } else { 180.0 };
        let mut clicked: Option<String> = None;
        egui::Area::new(egui::Id::new("huddle_videos")).order(egui::Order::Middle).anchor(Align2::CENTER_BOTTOM, [0.0, -210.0]).show(ctx, |ui| {
            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (id, name) in &names {
                        let Some(tex) = self.huddle.textures.get(id) else { continue };
                        let big = focus.as_ref() == Some(id);
                        let w = match &focus {
                            Some(_) if big => 560.0,
                            Some(_) => 140.0,
                            None => normal,
                        };
                        let size = tex.size_vec2();
                        let h = w * size.y / size.x.max(1.0);
                        ui.vertical(|ui| {
                            let r = ui.add(egui::Image::new((tex.id(), vec2(w, h))).corner_radius(8).sense(Sense::click()));
                            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, format!("Video of {}", name)));
                            if r.on_hover_text(if big { tr("Click to make smaller") } else { tr("Click to make larger") }).clicked() {
                                clicked = Some(id.clone());
                            }
                            ui.label(RichText::new(if id == ME { tr("You") } else { name.as_str() }).small().color(t.text2));
                        });
                    }
                });
            });
        });
        if let Some(id) = clicked {
            self.huddle.focus = if self.huddle.focus.as_ref() == Some(&id) { None } else { Some(id) };
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
        let where_ = self.data.channels.iter().find(|c| c.id == ch).map(|c| if c.kind == ChannelKind::Channel { format!("#{}", c.name) } else { self.channel_label(c) }).unwrap_or_else(|| tr("a conversation").into());
        let green = Color32::from_rgb(0x00, 0x88, 0x44);
        let mut leave = false;
        let mut mute: Option<bool> = None;
        let mut camera: Option<bool> = None;
        self.huddle_videos(ctx, &t);
        egui::Area::new(egui::Id::new("huddle_bar")).order(egui::Order::Middle).anchor(Align2::CENTER_BOTTOM, [0.0, -132.0]).show(ctx, |ui| {
            egui::Frame::new().fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(12).inner_margin(egui::Margin::symmetric(14, 10)).shadow(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(40) }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 13.0, green);
                    super::shell::line_icon(ui.painter(), "phone", r.center(), Color32::WHITE);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(trf!("Huddle in {}", where_)).strong().color(t.text));
                        let status = if self.huddle.joined { match self.huddle.people.len() { 0 | 1 => tr("Only you so far").to_string(), n => trf!("{} people", n) } } else { tr("Connecting\u{2026}").to_string() };
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
                        let shown = if p.me { trf!("{} (you), {}", name, tr(state)) } else { format!("{}, {}", name, tr(state)) };
                        resp.on_hover_text(shown);
                    }
                    ui.add_space(10.0);
                    let muted = self.huddle.muted;
                    if ui.add(egui::Button::new(if muted { tr("Unmute") } else { tr("Mute") }).min_size(vec2(72.0, 30.0)).corner_radius(7)).clicked() {
                        mute = Some(!muted);
                    }
                    let cam = self.huddle.camera;
                    if ui.add(egui::Button::new(if cam { tr("Stop video") } else { tr("Video") }).min_size(vec2(72.0, 30.0)).corner_radius(7)).clicked() {
                        camera = Some(!cam);
                    }
                    if ui.add(egui::Button::new(RichText::new(tr("Leave")).color(Color32::WHITE)).fill(t.danger).min_size(vec2(64.0, 30.0)).corner_radius(7)).clicked() {
                        leave = true;
                    }
                });
            });
        });
        // the huddle floats over the page but under dialogs: put an open
        // dialog (and its dimmed background) back on top
        if self.modal.is_some() {
            for id in ["modal_dim", "modal_window"] {
                ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, egui::Id::new(id)));
            }
        }
        if self.new_message_open {
            ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, egui::Id::new("new_message_window")));
        }
        if let Some(m) = mute {
            self.set_huddle_muted(m);
        }
        if let Some(c) = camera {
            self.set_huddle_camera(c);
        }
        if leave {
            self.leave_huddle();
        }
    }
}
