// Voice huddles inside the app (stage 3). When the workspace is on a server,
// the Huddle button joins the conversation's LiveKit room: the server hands
// out a token for it, wb_voice runs the call, and a card like a call window
// (huddle/panel.rs) shows everyone's tiles, who is speaking, mute and leave. Without a server the Huddle button opens
// a browser call as before (see together.rs). Video (stage 4): the Video
// button sends the camera, and everyone's pictures show above the bar.
// Share screen offers the displays and windows to share (after the Screen
// Recording permission); a shared screen shows large, with its sharer's name,
// for everyone else. How large each picture is drawn goes to the call, which
// asks the server for the video layer that fits (wb_voice::quality).

use super::*;
use std::collections::HashMap;

mod panel;
pub(super) use panel::{panel_frame, Sizes};
use wb_voice::{
    screen_key, screen_owner, Audio, Call, Event as VoiceEvent, Join, Person, Picture,
    ScreenTrouble, Share, Source, SourceKind, Video, ME,
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
    pub(super) pictures: HashMap<String, (std::sync::Arc<Picture>, bool)>,
    pub(super) textures: HashMap<String, egui::TextureHandle>,
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
    /// Where the bar goes over the conversation's message box, set by Chat
    /// when it draws one: the pass it was drawn in, and the box's left and
    /// right and the top of its Message / Post / Ask tabs.
    pub(super) dock: Option<(u64, Rect)>,
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
            dock: None,
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
                goal: None,
                event: None,
                poll: None,
                votes: Default::default(),
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

    /// Puts the pictures that came in since the last frame on the screen's
    /// textures (the main window's and the small window's share them).
    pub(super) fn huddle_textures(&mut self, ctx: &egui::Context) {
        for (id, (pic, fresh)) in self.huddle.pictures.iter_mut() {
            if !*fresh {
                continue;
            }
            *fresh = false;
            // every received picture is opaque (alpha 255), where
            // premultiplied and not are the same, and this skips the divide
            let img = egui::ColorImage::from_rgba_premultiplied(
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
    }

    /// The pictures to show, as (picture, whose name) for the shared screens
    /// and for the cameras; None when there are none. Only people still in
    /// the call: a picture can outlive its sender by a frame.
    pub(super) fn huddle_shown_videos(
        &self,
    ) -> Option<(Vec<(String, String)>, Vec<(String, String)>)> {
        let in_call = |id: &str| self.huddle.people.iter().any(|p| p.identity == id);
        let mut ids: Vec<String> = self
            .huddle
            .textures
            .keys()
            .filter(|id| {
                let who = screen_owner(id).unwrap_or(id);
                who == ME || in_call(who)
            })
            .cloned()
            .collect();
        if ids.is_empty() {
            return None;
        }
        // yours last: your camera after the others', your screen after theirs
        ids.sort_by_key(|id| (screen_owner(id).unwrap_or(id) == ME, id.clone()));
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
        Some((screens, cameras))
    }

    /// The words under a shared screen (and its accessibility name).
    pub(super) fn screen_label(id: &str, name: &str) -> String {
        if screen_owner(id) == Some(ME) {
            tr("Your screen").to_string()
        } else {
            trf!("{}'s screen", name)
        }
    }

    /// Tell the call how large each received picture is drawn (when that
    /// changes), so it asks for the video layer that fits.
    pub(super) fn tell_view_sizes(&mut self, sizes: HashMap<String, f32>) {
        if sizes == self.huddle.views {
            return;
        }
        if let Some(c) = &self.huddle.call {
            c.set_view_sizes(sizes.clone());
        }
        self.huddle.views = sizes;
    }

    /// The "Share your screen" chooser above the bar: the displays and
    /// windows, or how to give the Screen Recording permission. `width`:
    /// how wide it is (narrower in the small window, see mini.rs, and in a
    /// narrow window). `bottom`: where its bottom goes (the huddle bar's
    /// top), else 200 over the window's bottom.
    pub(super) fn share_menu(
        &mut self,
        ctx: &egui::Context,
        t: &Theme,
        width: f32,
        bottom: Option<egui::Pos2>,
    ) {
        if !self.huddle.share_menu {
            return;
        }
        let mut pick: Option<Share> = None;
        let mut close = false;
        let mut settings = false;
        let mut again = false;
        // inside the window with its margins (it started left of a narrow
        // window, the start of every line cut)
        let screen = ctx.screen_rect();
        let width = width.min(screen.width() - 28.0 - 16.0).max(160.0);
        let area =
            egui::Area::new(egui::Id::new("huddle_share_menu")).order(egui::Order::Foreground);
        let area = match bottom {
            Some(at) => area.pivot(Align2::CENTER_BOTTOM).fixed_pos(at),
            None => area.anchor(Align2::CENTER_BOTTOM, [0.0, -200.0]),
        };
        area.show(ctx, |ui| {
                card_frame(&t)
                    .fill(t.card)
                    .inner_margin(egui::Margin::same(14))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(40),
                    })
                    .show(ui, |ui| {
                        ui.set_width(width);
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
                                ui.horizontal_wrapped(|ui| {
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
                                            .min_size(vec2(width - 28.0, 26.0))
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
                                                .min_size(vec2(width - 28.0, 24.0))
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

    /// Where the huddle is, as the bar says it ("#general", a person's name).
    pub(super) fn huddle_place(&self, ch: u64) -> String {
        self.data
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
            .unwrap_or_else(|| tr("a conversation").into())
    }

    /// Who is in the huddle so far, or that it is still connecting.
    pub(super) fn huddle_status(&self) -> String {
        if self.huddle.joined {
            match self.huddle.people.len() {
                0 | 1 => tr("Only you so far").to_string(),
                n => trf!("{} people", n),
            }
        } else {
            tr("Connecting\u{2026}").to_string()
        }
    }

    /// Does what was pressed among the huddle's buttons.
    pub(super) fn huddle_pressed(&mut self, press: HuddlePress) {
        if let Some(m) = press.mute {
            self.set_huddle_muted(m);
        }
        if let Some(c) = press.camera {
            self.set_huddle_camera(c);
        }
        if press.share {
            if self.huddle.screen.is_some() {
                self.set_huddle_screen(None);
            } else if self.huddle.share_menu {
                self.huddle.share_menu = false;
            } else {
                self.open_share_menu();
            }
        }
        if press.open_chat {
            if let Some(ch) = self.huddle.channel {
                self.section = Section::Chat;
                self.chat.open = Some(ch);
            }
        }
        if press.leave {
            self.leave_huddle();
        }
    }

    /// The huddle over the page while in one, as a call window shows it:
    /// its name, the stage with everyone's tiles and the row of buttons
    /// (panel.rs). Over Chat's message box and as wide as it at most (it
    /// covered the top of the box's tabs, and the sidebar beside a narrow
    /// page); elsewhere near the window's bottom, inside the window. Not
    /// while the huddle shows in the small window.
    pub(super) fn huddle_bar(&mut self, ctx: &egui::Context) {
        let Some(ch) = self.huddle.channel else {
            return;
        };
        if self.mini_has_huddle() {
            return;
        }
        let t = self.t();
        let mut press = HuddlePress::default();
        let mut small = false;
        let screen = ctx.screen_rect();
        let (at, max_w) = match self.huddle.dock {
            Some((pass, r)) if pass == ctx.cumulative_pass_nr() => {
                (pos2(r.center().x, r.top() - 6.0), r.width())
            }
            _ => (
                pos2(screen.center().x, screen.bottom() - 132.0),
                screen.width() - 16.0,
            ),
        };
        // wider while a screen or a picture shows large
        let large = self.huddle.focus.is_some()
            || self
                .huddle_shown_videos()
                .is_some_and(|(screens, _)| !screens.is_empty());
        let margin = 10.0;
        let width = (if large { 960.0 } else { 600.0_f32 }).min(max_w);
        let inner_w = width - 2.0 * margin;
        // the conversation's button only where there is room for it, and
        // smaller buttons in a very narrow window
        let mut s = Sizes::MAIN;
        if inner_w < Self::toolbar_width(s, false) {
            s = Sizes::SMALL;
        }
        let chat = inner_w >= Self::toolbar_width(s, true);
        // the screen chooser goes above the card: the stage gives it room
        let menu_id = egui::Id::new("huddle_share_menu");
        let menu_h = if self.huddle.share_menu {
            ctx.memory(|m| m.area_rect(menu_id))
                .map(|r| r.height())
                .unwrap_or(300.0)
                + 8.0
        } else {
            0.0
        };
        let room = at.y - screen.top() - 8.0 - menu_h;
        // a short window with the chooser open: no stage, only the title
        // and the buttons (the chooser covered them)
        let stage_max = (room - 2.0 * margin - 26.0 - s.buttons_h() - 2.0 * s.gap).min(620.0);
        let stage = stage_max >= s.strip + 2.0 * s.pad;
        self.huddle_textures(ctx);
        let bar = egui::Area::new(egui::Id::new("huddle_bar"))
            .order(egui::Order::Middle)
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(at)
            .show(ctx, |ui| {
                panel_frame(14, margin as i8)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(70),
                    })
                    .show(ui, |ui| {
                        ui.set_width(inner_w);
                        ui.spacing_mut().item_spacing.y = s.gap;
                        self.huddle_header(ui, ch, &mut small);
                        if stage {
                            self.huddle_stage(ui, inner_w, stage_max, s);
                        } else {
                            self.tell_view_sizes(HashMap::new());
                        }
                        self.huddle_toolbar(ui, inner_w, s, chat, &mut press);
                    });
            });
        // the screen chooser above the card
        let top = pos2(at.x, bar.response.rect.top() - 8.0);
        self.share_menu(ctx, &t, 380.0, Some(top));
        // the huddle floats over the page but under dialogs: put an open
        // dialog (and its dimmed background) back on top
        if let Some(m) = &self.modal {
            let id = super::menus::modal_window_id(m);
            for id in [id.with("dim"), id] {
                ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
            }
        }
        if self.new_message_open {
            let id = egui::Id::new("new_message_window");
            for id in [id.with("dim"), id] {
                ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
            }
        }
        self.huddle_pressed(press);
        if small && self.huddle.channel == Some(ch) {
            self.open_mini(ch);
        }
    }
}

/// What was pressed among a huddle's buttons this frame.
#[derive(Default)]
pub(super) struct HuddlePress {
    mute: Option<bool>,
    camera: Option<bool>,
    share: bool,
    leave: bool,
    open_chat: bool,
}
