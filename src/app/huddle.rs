// Voice huddles inside the app (stage 3). When the workspace is on a server,
// the Huddle button joins the conversation's LiveKit room: the server hands
// out a token for it, wb_voice runs the call, and a bar shows who is in it,
// who is speaking, mute and leave. Without a server the Huddle button opens
// a browser call as before (see together.rs). Video (stage 4): the Video
// button sends the camera, and everyone's pictures show above the bar.
// Share screen offers the displays and windows to share (after the Screen
// Recording permission); a shared screen shows large, with its sharer's name,
// for everyone else. How large each picture is drawn goes to the call, which
// asks the server for the video layer that fits (wb_voice::quality).

use super::*;
use std::collections::HashMap;
use wb_voice::{
    screen_owner, Audio, Call, Event as VoiceEvent, Join, Person, Picture, ScreenTrouble, Share,
    Source, SourceKind, Video, ME,
};

/// Where screens to share come from: macOS, or a made-up list in tests.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum ScreenEnv {
    System,
    Fake { allowed: bool, sources: Vec<Source> },
}

impl ScreenEnv {
    fn allowed(&self) -> bool {
        match self {
            ScreenEnv::System => wb_voice::screen::allowed(),
            ScreenEnv::Fake { allowed, .. } => *allowed,
        }
    }

    /// Ask for the permission (macOS shows its request only the first time).
    fn ask(&self) -> bool {
        match self {
            ScreenEnv::System => wb_voice::screen::ask(),
            ScreenEnv::Fake { allowed, .. } => *allowed,
        }
    }

    fn sources(&self) -> Result<Vec<Source>, ScreenTrouble> {
        match self {
            ScreenEnv::System => wb_voice::screen::sources(),
            ScreenEnv::Fake { allowed: false, .. } => Err(ScreenTrouble::NotAllowed),
            ScreenEnv::Fake { sources, .. } => Ok(sources.clone()),
        }
    }

    fn open_settings(&self) {
        if *self == ScreenEnv::System {
            wb_voice::screen::open_settings();
        }
    }
}

fn test_screens() -> ScreenEnv {
    ScreenEnv::Fake {
        allowed: true,
        sources: vec![
            Source {
                kind: SourceKind::Display,
                id: 1,
                name: "Test display".into(),
                main: true,
            },
            Source {
                kind: SourceKind::Window,
                id: 7,
                name: "Notes: Plan".into(),
                main: false,
            },
        ],
    }
}

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
    /// The screen or window being shared from here.
    pub(super) screen: Option<Share>,
    /// The "Share your screen" chooser is open.
    pub(super) share_menu: bool,
    /// What the chooser offers (filled when it opens).
    pub(super) screen_sources: Vec<Source>,
    /// Why the chooser can't offer anything: no permission, or a message.
    pub(super) screen_problem: Option<ScreenTrouble>,
    pub(super) screen_env: ScreenEnv,
    /// Pressed "Open System Settings" (for tests).
    pub(super) opened_settings: bool,
    /// How wide (physical pixels) each picture was drawn, as last told to the call.
    pub(super) views: HashMap<String, f32>,
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
            audio: if cfg!(test) {
                Audio::Tone(440.0)
            } else {
                Audio::Platform
            },
            camera: false,
            // WORKBOARD_TEST_VIDEO=1: a colored test picture instead of the camera
            // (to check video on a computer without a camera)
            video: if cfg!(test) || std::env::var("WORKBOARD_TEST_VIDEO").is_ok_and(|v| v == "1") {
                Video::Pattern([200, 60, 60])
            } else {
                Video::Camera
            },
            pictures: HashMap::new(),
            textures: HashMap::new(),
            focus: None,
            screen: None,
            share_menu: false,
            screen_sources: vec![],
            screen_problem: None,
            screen_env: if cfg!(test) {
                test_screens()
            } else {
                ScreenEnv::System
            },
            opened_settings: false,
            views: HashMap::new(),
        }
    }
}

impl HuddleRt {
    /// Someone's video (or screen) stopped.
    pub(super) fn drop_picture(&mut self, id: &str) {
        self.pictures.remove(id);
        self.textures.remove(id);
    }
}

pub(super) fn huddle_channel(link: &str) -> Option<u64> {
    link.strip_prefix(HUDDLE_PREFIX)?.parse().ok()
}

impl App {
    /// In-app huddles need a signed-in server connection.
    pub(super) fn huddles_on_server(&self) -> bool {
        self.on_server() && self.sync_rt.net_status == Some(wb_net::Status::Connected)
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
        self.chat.open = Some(ch);
    }

    /// Tell everyone in the conversation that a huddle started.
    fn post_huddle_card(&mut self, ch: u64) {
        let id = self.data.new_id();
        let me = self.data.me;
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
            c.messages.push(Message {
                edited: None,
                imported: None,
                preview: None,
                forwarded: None,
                thread_of: None,
                bot: None,
                mentions: vec![],
                attachments: vec![],
                id,
                author: me,
                text: "Started a huddle".into(),
                ts: now_ts(),
                reactions: vec![],
                replies: vec![],
                saved: false,
                task: None,
                post_title: None,
                post_type: String::new(),
                pinned: false,
                call: Some(format!("{}{}", HUDDLE_PREFIX, ch)),
            });
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
            let has_server = self.on_server();
            self.toast(if has_server {
                    tr("Huddles go through the team's server, and it can't be reached right now. Try again once it's connected.")
                } else {
                    tr("Huddles need the team's server. Connect in Work together.")
                }
                );
            return;
        }
        let (audio, video) = (self.huddle.audio, self.huddle.video);
        let screen_env = self.huddle.screen_env.clone();
        self.huddle = HuddleRt {
            channel: Some(ch),
            waiting: true,
            asked: Some(std::time::Instant::now()),
            audio,
            video,
            screen_env,
            ..Default::default()
        };
        self.access_request(wb_net::Msg::CallJoin { channel: ch });
    }

    pub(super) fn leave_huddle(&mut self) {
        let (audio, video) = (self.huddle.audio, self.huddle.video);
        let screen_env = self.huddle.screen_env.clone();
        self.huddle = HuddleRt {
            audio,
            video,
            screen_env,
            ..Default::default()
        };
    }

    /// Open the "Share your screen" chooser: ask for the Screen Recording
    /// permission if needed, then list the displays and windows.
    pub(super) fn open_share_menu(&mut self) {
        self.huddle.share_menu = true;
        self.huddle.screen_problem = None;
        self.huddle.screen_sources.clear();
        let env = self.huddle.screen_env.clone();
        if !env.allowed() && !env.ask() {
            self.huddle.screen_problem = Some(ScreenTrouble::NotAllowed);
            return;
        }
        match env.sources() {
            Ok(list) => self.huddle.screen_sources = list,
            Err(e) => self.huddle.screen_problem = Some(e),
        }
    }

    /// Share a screen or window (or stop with None).
    pub(super) fn set_huddle_screen(&mut self, share: Option<Share>) {
        self.huddle.screen = share;
        self.huddle.share_menu = false;
        if let Some(c) = &self.huddle.call {
            c.share_screen(share);
        }
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

    /// A request was turned down without naming a conversation. Servers from
    /// before refusals named theirs answer a refused call_join this way, so
    /// while waiting for a huddle's token it's taken as that refusal, unless
    /// another request went out shortly before the call_join (the server
    /// answers in order, so the refusal could be that one's).
    pub(super) fn huddle_denied_unnamed(&mut self) {
        let Some(asked) = self.huddle.asked.filter(|_| self.huddle.waiting) else {
            return;
        };
        let other_first = self.sync_rt.other_asked.is_some_and(|t| {
            t <= asked && asked.duration_since(t) < std::time::Duration::from_secs(HUDDLE_JOIN_SECS)
        });
        if let Some(ch) = self.huddle.channel.filter(|_| !other_first) {
            self.huddle_denied(ch);
        }
    }

    /// Once per frame: start the call when the token is here, take in news.
    pub(super) fn huddle_tick(&mut self, ctx: Option<&egui::Context>) {
        if self.huddle.waiting
            && self
                .huddle
                .asked
                .is_some_and(|t| t.elapsed() >= std::time::Duration::from_secs(HUDDLE_JOIN_SECS))
        {
            self.leave_huddle();
            self.toast(tr(
                "The server didn't answer, so the huddle didn't start. Try again in a moment.",
            ));
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
            if self.huddle.screen.is_some() {
                call.share_screen(self.huddle.screen);
            }
            self.huddle.views.clear();
            self.huddle.call = Some(call);
        }
        let events = self
            .huddle
            .call
            .as_ref()
            .map(|c| c.events())
            .unwrap_or_default();
        for e in events {
            match e {
                VoiceEvent::Joined => self.huddle.joined = true,
                VoiceEvent::People(p) => self.huddle.people = p,
                VoiceEvent::Trouble(why) => {
                    // so far only the camera says this: turn it back off
                    self.set_huddle_camera(false);
                    self.toast(why);
                }
                VoiceEvent::ScreenEnded(why) => {
                    self.huddle.screen = None;
                    match why {
                        // stopped from macOS's own controls
                        None => {}
                        Some(ScreenTrouble::NotAllowed) => {
                            self.huddle.share_menu = true;
                            self.huddle.screen_problem = Some(ScreenTrouble::NotAllowed);
                        }
                        Some(ScreenTrouble::Gone) => {
                            self.toast(tr(
                                "The screen or window you shared is gone, so sharing stopped.",
                            ));
                        }
                        Some(ScreenTrouble::Failed(m)) => {
                            self.toast(trf!("Screen sharing stopped: {}", m));
                        }
                    }
                }
                VoiceEvent::Left(why) => {
                    if let Some(why) = why {
                        self.toast(why);
                    }
                    self.leave_huddle();
                }
            }
        }
        let pictures = self
            .huddle
            .call
            .as_ref()
            .map(|c| c.new_pictures())
            .unwrap_or_default();
        for (id, p) in pictures {
            match p {
                Some(p) => {
                    self.huddle.pictures.insert(id, (p, true));
                }
                None => self.huddle.drop_picture(&id),
            }
        }
        // the server connection went away or the workspace changed
        if self.huddle.channel.is_some() && !self.on_server() {
            self.leave_huddle();
        }
    }

    /// Everyone's video above the bar: shared screens large on top, then
    /// cameras, others first, then you.
    fn huddle_videos(&mut self, ctx: &egui::Context, t: &Theme) {
        if self.huddle.pictures.is_empty() {
            self.tell_view_sizes(HashMap::new());
            return;
        }
        for (id, (pic, fresh)) in self.huddle.pictures.iter_mut() {
            if !*fresh {
                continue;
            }
            *fresh = false;
            let img = egui::ColorImage::from_rgba_unmultiplied(
                [pic.width as usize, pic.height as usize],
                &pic.rgba,
            );
            match self.huddle.textures.get_mut(id) {
                Some(tex) => tex.set(img, egui::TextureOptions::LINEAR),
                None => {
                    let tex = ctx.load_texture(
                        format!("huddle-video-{}", id),
                        img,
                        egui::TextureOptions::LINEAR,
                    );
                    self.huddle.textures.insert(id.clone(), tex);
                }
            }
        }
        // only people still in the call: a picture can outlive its sender by a frame
        let in_call = |id: &str| self.huddle.people.iter().any(|p| p.identity == id);
        let mut ids: Vec<String> = self
            .huddle
            .textures
            .keys()
            .filter(|id| *id == ME || in_call(screen_owner(id).unwrap_or(id)))
            .cloned()
            .collect();
        if ids.is_empty() {
            self.huddle.focus = None;
            self.tell_view_sizes(HashMap::new());
            return;
        }
        ids.sort_by_key(|id| (id == ME, id.clone()));
        let name_of = |id: &str| -> String {
            if id == ME {
                return "You".to_string();
            }
            self.huddle
                .people
                .iter()
                .find(|p| p.identity == id)
                .map(|p| self.huddle_name(p))
                .unwrap_or_default()
        };
        let (screens, cameras): (Vec<(String, String)>, Vec<(String, String)>) = ids
            .iter()
            .map(|id| (id.clone(), name_of(screen_owner(id).unwrap_or(id))))
            .partition(|(id, _)| screen_owner(id).is_some());
        // a clicked video shows large and the rest small; otherwise tiles get
        // smaller when many people send video (and when a screen is shared)
        if self
            .huddle
            .focus
            .as_ref()
            .is_some_and(|f| !cameras.iter().any(|(id, _)| id == f))
        {
            self.huddle.focus = None;
        }
        let focus = self.huddle.focus.clone();
        let normal = if !screens.is_empty() {
            140.0
        } else if cameras.len() <= 2 {
            320.0
        } else if cameras.len() <= 4 {
            240.0
        } else {
            180.0
        };
        // a shared screen as large as fits above the bar
        let room = ctx.screen_rect();
        let screen_w = (room.width() - 80.0).clamp(240.0, 960.0);
        let screen_max_h = (room.height() - 210.0 - 200.0 - 60.0).max(140.0);
        let ppp = ctx.pixels_per_point();
        let mut drawn: HashMap<String, f32> = HashMap::new();
        let mut clicked: Option<String> = None;
        egui::Area::new(egui::Id::new("huddle_videos"))
            .order(egui::Order::Middle)
            .anchor(Align2::CENTER_BOTTOM, [0.0, -210.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t.card)
                    .stroke(Stroke::new(1.0_f32, t.border))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            for (id, name) in &screens {
                                let Some(tex) = self.huddle.textures.get(id) else {
                                    continue;
                                };
                                let size = tex.size_vec2();
                                let aspect = size.y / size.x.max(1.0);
                                let w = screen_w.min(screen_max_h / aspect.max(0.01));
                                let r = ui.add(
                                    egui::Image::new((tex.id(), vec2(w, w * aspect)))
                                        .corner_radius(8),
                                );
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
                                        .strong()
                                        .color(t.text2),
                                );
                                ui.add_space(6.0);
                            }
                            ui.horizontal(|ui| {
                                for (id, name) in &cameras {
                                    let Some(tex) = self.huddle.textures.get(id) else {
                                        continue;
                                    };
                                    let big = focus.as_ref() == Some(id);
                                    let w = match &focus {
                                        Some(_) if big => 560.0,
                                        Some(_) => 140.0,
                                        None => normal,
                                    };
                                    let size = tex.size_vec2();
                                    let h = w * size.y / size.x.max(1.0);
                                    drawn.insert(id.clone(), w * ppp);
                                    ui.vertical(|ui| {
                                        let r = ui.add(
                                            egui::Image::new((tex.id(), vec2(w, h)))
                                                .corner_radius(8)
                                                .sense(Sense::click()),
                                        );
                                        hover_bg(ui, &r, r.rect, 8.0);
                                        r.widget_info(|| {
                                            egui::WidgetInfo::labeled(
                                                egui::WidgetType::Image,
                                                true,
                                                format!("Video of {}", name),
                                            )
                                        });
                                        if r.on_hover_text(if big {
                                            tr("Click to make smaller")
                                        } else {
                                            tr("Click to make larger")
                                        })
                                        .clicked()
                                        {
                                            clicked = Some(id.clone());
                                        }
                                        ui.label(
                                            RichText::new(if id == ME {
                                                tr("You")
                                            } else {
                                                name.as_str()
                                            })
                                            .small()
                                            .color(t.text2),
                                        );
                                    });
                                }
                            });
                        });
                    });
            });
        if let Some(id) = clicked {
            self.huddle.focus = if self.huddle.focus.as_ref() == Some(&id) {
                None
            } else {
                Some(id)
            };
        }
        drawn.remove(ME);
        self.tell_view_sizes(drawn);
    }

    /// Tell the call how large each received picture is drawn (when that
    /// changes), so it asks for the video layer that fits.
    fn tell_view_sizes(&mut self, sizes: HashMap<String, f32>) {
        if sizes == self.huddle.views {
            return;
        }
        if let Some(c) = &self.huddle.call {
            c.set_view_sizes(sizes.clone());
        }
        self.huddle.views = sizes;
    }

    /// The "Share your screen" chooser above the bar: the displays and
    /// windows, or how to give the Screen Recording permission.
    fn share_menu(&mut self, ctx: &egui::Context, t: &Theme) {
        if !self.huddle.share_menu {
            return;
        }
        let mut pick: Option<Share> = None;
        let mut close = false;
        let mut settings = false;
        let mut again = false;
        egui::Area::new(egui::Id::new("huddle_share_menu"))
            .order(egui::Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, [0.0, -200.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t.card)
                    .stroke(Stroke::new(1.0_f32, t.border))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::same(14))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(40),
                    })
                    .show(ui, |ui| {
                        ui.set_width(380.0);
                        ui.label(
                            RichText::new(tr("Share your screen"))
                                .strong()
                                .color(t.text),
                        );
                        ui.add_space(6.0);
                        match &self.huddle.screen_problem {
                            Some(ScreenTrouble::NotAllowed) => {
                                ui.label(
                                    RichText::new(tr(
                                        "Workboard needs the Screen Recording permission to share your screen.",
                                    ))
                                    .color(t.text),
                                );
                                ui.label(
                                    RichText::new(tr(
                                        "Turn on Workboard in System Settings > Privacy & Security > Screen & System Audio Recording, then quit and reopen Workboard.",
                                    ))
                                    .small()
                                    .color(t.text2),
                                );
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    if ui.button(tr("Open System Settings")).clicked() {
                                        settings = true;
                                    }
                                    if ui.button(tr("Try again")).clicked() {
                                        again = true;
                                    }
                                });
                            }
                            Some(other) => {
                                let why = match other {
                                    ScreenTrouble::Failed(m) => m.clone(),
                                    _ => String::new(),
                                };
                                ui.label(
                                    RichText::new(trf!("Couldn't list the screens to share: {}", why))
                                        .color(t.text2),
                                );
                                if ui.button(tr("Try again")).clicked() {
                                    again = true;
                                }
                            }
                            None => {
                                let (displays, windows): (Vec<&Source>, Vec<&Source>) = self
                                    .huddle
                                    .screen_sources
                                    .iter()
                                    .partition(|s| s.kind == SourceKind::Display);
                                ui.label(RichText::new(tr("Screens")).small().color(t.text2));
                                for d in displays {
                                    let label = if d.main {
                                        trf!("{} (main)", d.name)
                                    } else {
                                        d.name.clone()
                                    };
                                    let r = ui.add(
                                        egui::Button::new(label)
                                            .min_size(vec2(352.0, 26.0))
                                            .corner_radius(6),
                                    );
                                    r.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::WidgetType::Button,
                                            true,
                                            format!("Share {}", d.name),
                                        )
                                    });
                                    if r.clicked() {
                                        pick = Some(d.share());
                                    }
                                }
                                ui.add_space(6.0);
                                ui.label(RichText::new(tr("Windows")).small().color(t.text2));
                                if windows.is_empty() {
                                    ui.label(
                                        RichText::new(tr("No windows to share."))
                                            .small()
                                            .color(t.text3),
                                    );
                                }
                                egui::ScrollArea::vertical()
                                    .max_height(220.0)
                                    .show(ui, |ui| {
                                        for w in windows {
                                            let r = ui.add(
                                                egui::Button::new(
                                                    RichText::new(&w.name).small(),
                                                )
                                                .truncate()
                                                .min_size(vec2(352.0, 24.0))
                                                .corner_radius(6),
                                            );
                                            r.widget_info(|| {
                                                egui::WidgetInfo::labeled(
                                                    egui::WidgetType::Button,
                                                    true,
                                                    format!("Share {}", w.name),
                                                )
                                            });
                                            if r.clicked() {
                                                pick = Some(w.share());
                                            }
                                        }
                                    });
                            }
                        }
                        ui.add_space(6.0);
                        if ui.button(tr("Cancel")).clicked() {
                            close = true;
                        }
                    });
            });
        if settings {
            self.huddle.opened_settings = true;
            self.huddle.screen_env.open_settings();
        }
        if again {
            self.open_share_menu();
        }
        if close {
            self.huddle.share_menu = false;
        }
        if pick.is_some() {
            self.set_huddle_screen(pick);
        }
    }

    fn huddle_name(&self, p: &Person) -> String {
        p.member()
            .and_then(|m| self.data.member(m))
            .map(|m| m.shown().to_string())
            .unwrap_or_else(|| p.name.clone())
    }

    /// The bar at the bottom while in a huddle (above the message box, so it
    /// never covers it).
    pub(super) fn huddle_bar(&mut self, ctx: &egui::Context) {
        let Some(ch) = self.huddle.channel else {
            return;
        };
        let t = self.t();
        let where_ = self
            .data
            .channels
            .iter()
            .find(|c| c.id == ch)
            .map(|c| {
                if c.kind == ChannelKind::Channel {
                    format!("#{}", c.name)
                } else {
                    self.channel_label(c)
                }
            })
            .unwrap_or_else(|| tr("a conversation").into());
        let green = Color32::from_rgb(0x00, 0x88, 0x44);
        let mut leave = false;
        let mut mute: Option<bool> = None;
        let mut camera: Option<bool> = None;
        let mut share_click = false;
        self.huddle_videos(ctx, &t);
        self.share_menu(ctx, &t);
        egui::Area::new(egui::Id::new("huddle_bar"))
            .order(egui::Order::Middle)
            .anchor(Align2::CENTER_BOTTOM, [0.0, -132.0])
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(t.card)
                    .stroke(Stroke::new(1.0_f32, t.border))
                    .corner_radius(12)
                    .inner_margin(egui::Margin::symmetric(14, 10))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(40),
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 13.0, green);
                            super::shell::line_icon(
                                ui.painter(),
                                "phone",
                                r.center(),
                                Color32::WHITE,
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(trf!("Huddle in {}", where_))
                                        .strong()
                                        .color(t.text),
                                );
                                let status = if self.huddle.joined {
                                    match self.huddle.people.len() {
                                        0 | 1 => tr("Only you so far").to_string(),
                                        n => trf!("{} people", n),
                                    }
                                } else {
                                    tr("Connecting\u{2026}").to_string()
                                };
                                ui.label(RichText::new(status).small().color(t.text2));
                                if self.huddle.screen.is_some() {
                                    ui.label(
                                        RichText::new(tr("Sharing your screen"))
                                            .small()
                                            .color(green),
                                    );
                                }
                            });
                            ui.add_space(10.0);
                            for p in self.huddle.people.clone() {
                                let name = self.huddle_name(&p);
                                let (r, resp) =
                                    ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                                let color = p
                                    .member()
                                    .and_then(|m| self.data.member(m))
                                    .map(|m| rgb(m.color))
                                    .unwrap_or(t.text3);
                                if p.speaking && !p.muted {
                                    ui.painter().circle_stroke(
                                        r.center(),
                                        15.0,
                                        Stroke::new(2.5_f32, green),
                                    );
                                }
                                ui.painter().circle_filled(r.center(), 12.0, color);
                                let initial = name
                                    .chars()
                                    .next()
                                    .map(|c| c.to_uppercase().to_string())
                                    .unwrap_or_default();
                                ui.painter().text(
                                    r.center(),
                                    Align2::CENTER_CENTER,
                                    initial,
                                    FontId::proportional(12.0),
                                    Color32::WHITE,
                                );
                                if p.muted {
                                    ui.painter().circle_filled(
                                        r.right_bottom() - vec2(5.0, 5.0),
                                        5.5,
                                        t.danger,
                                    );
                                    ui.painter().line_segment(
                                        [
                                            r.right_bottom() - vec2(8.0, 8.0),
                                            r.right_bottom() - vec2(2.0, 2.0),
                                        ],
                                        Stroke::new(1.5_f32, Color32::WHITE),
                                    );
                                }
                                let state = if p.muted {
                                    "muted"
                                } else if p.speaking {
                                    "speaking"
                                } else {
                                    "listening"
                                };
                                let label = if p.me {
                                    format!("{} (you), {}", name, state)
                                } else {
                                    format!("{}, {}", name, state)
                                };
                                resp.widget_info(|| {
                                    egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label)
                                });
                                let shown = if p.me {
                                    trf!("{} (you), {}", name, tr(state))
                                } else {
                                    format!("{}, {}", name, tr(state))
                                };
                                resp.on_hover_text(shown);
                            }
                            ui.add_space(10.0);
                            let muted = self.huddle.muted;
                            if ui
                                .add(
                                    egui::Button::new(if muted {
                                        tr("Unmute")
                                    } else {
                                        tr("Mute")
                                    })
                                    .min_size(vec2(72.0, 30.0))
                                    .corner_radius(7),
                                )
                                .clicked()
                            {
                                mute = Some(!muted);
                            }
                            let cam = self.huddle.camera;
                            if ui
                                .add(
                                    egui::Button::new(if cam {
                                        tr("Stop video")
                                    } else {
                                        tr("Video")
                                    })
                                    .min_size(vec2(72.0, 30.0))
                                    .corner_radius(7),
                                )
                                .clicked()
                            {
                                camera = Some(!cam);
                            }
                            let sharing = self.huddle.screen.is_some();
                            if ui
                                .add(
                                    egui::Button::new(if sharing {
                                        tr("Stop sharing screen")
                                    } else {
                                        tr("Share screen")
                                    })
                                    .min_size(vec2(72.0, 30.0))
                                    .corner_radius(7),
                                )
                                .clicked()
                            {
                                share_click = true;
                            }
                            if ui
                                .add(Tint(
                                    egui::Button::new(
                                        RichText::new(tr("Leave")).color(Color32::WHITE),
                                    )
                                    .fill(t.danger)
                                    .min_size(vec2(64.0, 30.0))
                                    .corner_radius(7),
                                ))
                                .clicked()
                            {
                                leave = true;
                            }
                        });
                    });
            });
        // the huddle floats over the page but under dialogs: put an open
        // dialog (and its dimmed background) back on top
        if let Some(m) = &self.modal {
            for id in [egui::Id::new("modal_dim"), super::menus::modal_window_id(m)] {
                ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
            }
        }
        if self.new_message_open {
            ctx.move_to_top(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("new_message_window"),
            ));
        }
        if let Some(m) = mute {
            self.set_huddle_muted(m);
        }
        if let Some(c) = camera {
            self.set_huddle_camera(c);
        }
        if share_click {
            if self.huddle.screen.is_some() {
                self.set_huddle_screen(None);
            } else if self.huddle.share_menu {
                self.huddle.share_menu = false;
            } else {
                self.open_share_menu();
            }
        }
        if leave {
            self.leave_huddle();
        }
    }
}
