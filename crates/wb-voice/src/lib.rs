// Voice huddles: one LiveKit room per conversation. A call runs on its own
// thread with its own async runtime, so the window never waits for it; the
// app sends commands (mute, leave) and reads events (who is in the call, who
// is speaking) through channels, like the sync connection in wb_net.
//
// With `Audio::Platform` the computer's microphone is sent and everyone
// else is played on its speakers (WebRTC's own audio device handling, with
// echo cancellation). `Audio::Tone` sends a steady tone instead and measures
// how loud the others arrive, without touching any audio device; tests use it.
//
// Video (stage 4) goes into the same room: `set_camera(true)` publishes the
// camera (or a test pattern) and everyone's pictures arrive as RGBA through
// `new_pictures` (see video.rs). `share_screen` publishes a display or a
// window as a screen-share track (screen.rs); it arrives under
// `screen_key(identity)`, and a copy of your own under `screen_key(ME)`.
//
// Quality adapts (quality.rs): video goes out in simulcast layers, the app
// tells `set_view_sizes` how large each picture is drawn and the call asks the
// server for the layer that fits, dynacast stops layers nobody watches, and a
// poor connection makes this computer send smaller, slower video.

pub mod quality;
pub mod screen;
pub mod video;
pub use screen::{ScreenTrouble, Share, Source, SourceKind};
pub use video::{start_preview, Feed, Picture, Video};

use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::track::VideoQuality;
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_source::{AudioSourceOptions, RtcAudioSource};
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use livekit::webrtc::prelude::RtcVideoSource;
use livekit::webrtc::video_source::native::NativeVideoSource;
use livekit::webrtc::video_source::VideoResolution;
use livekit::webrtc::video_stream::native::NativeVideoStream;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Where to join: from workboard-server's call token answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Join {
    pub url: String,
    pub token: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Audio {
    /// The microphone and speakers.
    Platform,
    /// A tone at this frequency (Hz) instead of the microphone; nothing played.
    Tone(f32),
}

/// Someone in the call.
#[derive(Clone, Debug, PartialEq)]
pub struct Person {
    /// LiveKit identity: "<member id>-<device>" (`wb_net::call_identity`).
    pub identity: String,
    pub name: String,
    pub me: bool,
    pub speaking: bool,
    pub muted: bool,
    /// How loud their audio arrives here, 0..1 (only measured with Audio::Tone).
    pub heard: f32,
    /// Sending video.
    pub camera: bool,
    /// Sharing their screen.
    pub screen: bool,
    /// Their hand is raised.
    pub hand: bool,
}

impl Person {
    /// The workspace member this person is.
    pub fn member(&self) -> Option<u64> {
        wb_net::identity_member(&self.identity)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Joined,
    People(Vec<Person>),
    /// The call ended for this computer; holds why when it wasn't asked for.
    Left(Option<String>),
    /// Something went wrong with the camera that doesn't end the call.
    Trouble(String),
    /// Screen sharing stopped without being asked to: why, or None when it
    /// was stopped from macOS's own controls.
    ScreenEnded(Option<ScreenTrouble>),
    /// Someone (ME for you) sent a reaction: an emoji to show over their tile.
    Reaction {
        identity: String,
        emoji: String,
    },
    /// The microphones and speakers there are (after joining, and again
    /// when asked with `list_devices`).
    Devices {
        mics: Vec<Device>,
        speakers: Vec<Device>,
    },
    /// A microphone, speaker or camera couldn't be switched to.
    DeviceTrouble(String),
}

/// A microphone or speaker: what to ask for it by, and its name.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
}

enum Cmd {
    Mute(bool),
    Camera(bool),
    Screen(Option<Share>),
    Views(HashMap<String, f32>),
    Hand(bool),
    React(String),
    ListDevices,
    Mic(String),
    Speaker(String),
    /// The camera to use, by name (None: the first).
    CameraDevice(Option<String>),
    Leave,
}

/// The data topic for hands and reactions, and what goes in it:
/// "hand:1", "hand:0" or "react:<emoji>".
const TOPIC: &str = "workboard-huddle";

/// What a huddle message says.
#[derive(Debug, PartialEq)]
enum Signal {
    Hand(bool),
    React(String),
}

impl Signal {
    fn parse(payload: &[u8]) -> Option<Signal> {
        let text = std::str::from_utf8(payload).ok()?;
        match text.split_once(':')? {
            ("hand", "1") => Some(Signal::Hand(true)),
            ("hand", "0") => Some(Signal::Hand(false)),
            // a reaction is one emoji, a few characters at most
            ("react", e) if !e.is_empty() && e.chars().count() <= 8 => {
                Some(Signal::React(e.to_string()))
            }
            _ => None,
        }
    }

    fn payload(&self) -> Vec<u8> {
        match self {
            Signal::Hand(up) => format!("hand:{}", if *up { 1 } else { 0 }),
            Signal::React(e) => format!("react:{}", e),
        }
        .into_bytes()
    }
}

/// Send a huddle message to everyone (or to the people in `to`).
async fn signal(room: &Room, s: Signal, to: Vec<ParticipantIdentity>) {
    let _ = room
        .local_participant()
        .publish_data(DataPacket {
            payload: s.payload(),
            topic: Some(TOPIC.to_string()),
            reliable: true,
            destination_identities: to,
        })
        .await;
}

/// The cameras on this computer, by name (a made-up one with test pictures).
pub fn cameras(video: Video) -> Vec<String> {
    match video {
        Video::Pattern(_) => vec!["Test camera".to_string()],
        Video::Camera => video::camera_names(),
    }
}

/// Whose picture: a participant identity, or ME for your own camera (and
/// `screen_key(ME)` for your own shared screen).
pub const ME: &str = "";

const SCREEN_SUFFIX: &str = "#screen";

/// The picture key of someone's shared screen (identities never hold '#').
pub fn screen_key(identity: &str) -> String {
    format!("{}{}", identity, SCREEN_SUFFIX)
}

/// Whose screen a picture key is, if it is a shared screen.
pub fn screen_owner(key: &str) -> Option<&str> {
    key.strip_suffix(SCREEN_SUFFIX)
}

/// The picture key for a participant's video track of this source.
fn picture_key(identity: &str, source: TrackSource) -> String {
    if source == TrackSource::Screenshare {
        screen_key(identity)
    } else {
        identity.to_string()
    }
}

/// The latest picture of everyone sending video, with a counter that moves
/// when a new one arrives; a missing entry means no video. Shared, so
/// handing one to the window copies no pixels.
type Pictures = Arc<Mutex<HashMap<String, (u64, Arc<Picture>)>>>;

pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// A call in progress. Dropping it leaves the call.
pub struct Call {
    tx: tokio::sync::mpsc::UnboundedSender<Cmd>,
    rx: Receiver<Event>,
    pictures: Pictures,
    seen: Mutex<HashMap<String, u64>>,
}

impl Call {
    /// Join a call; `video` is where pictures come from once the camera is turned on.
    pub fn start(join: Join, audio: Audio, video: Video, wake: Wake) -> Call {
        let (tx, crx) = tokio::sync::mpsc::unbounded_channel();
        let (etx, rx) = mpsc::channel();
        let pictures: Pictures = Default::default();
        let pics = pictures.clone();
        std::thread::Builder::new()
            .name("workboard-voice".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = etx.send(Event::Left(Some(e.to_string())));
                        wake();
                        return;
                    }
                };
                let why = rt.block_on(run(join, audio, video, crx, &etx, &wake, pics));
                let _ = etx.send(Event::Left(why));
                wake();
            })
            .expect("start voice thread");
        Call {
            tx,
            rx,
            pictures,
            seen: Default::default(),
        }
    }

    pub fn set_camera(&self, on: bool) {
        let _ = self.tx.send(Cmd::Camera(on));
    }

    /// Share a screen or window (replacing one already shared), or stop with None.
    pub fn share_screen(&self, share: Option<Share>) {
        let _ = self.tx.send(Cmd::Screen(share));
    }

    /// How wide (physical pixels) each picture is drawn, by picture key;
    /// pictures not listed get the smallest layer.
    pub fn set_view_sizes(&self, sizes: HashMap<String, f32>) {
        let _ = self.tx.send(Cmd::Views(sizes));
    }

    /// Pictures that changed since the last call: (identity or ME, picture),
    /// with None for someone whose video stopped. The window is woken (the
    /// `wake` given to start) when one arrives.
    pub fn new_pictures(&self) -> Vec<(String, Option<Arc<Picture>>)> {
        let pics = self.pictures.lock().unwrap();
        let mut seen = self.seen.lock().unwrap();
        let mut out = vec![];
        for (id, (n, p)) in pics.iter() {
            if seen.get(id) != Some(n) {
                seen.insert(id.clone(), *n);
                out.push((id.clone(), Some(p.clone())));
            }
        }
        let gone: Vec<String> = seen
            .keys()
            .filter(|k| !pics.contains_key(*k))
            .cloned()
            .collect();
        for id in gone {
            seen.remove(&id);
            out.push((id, None));
        }
        out
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.tx.send(Cmd::Mute(muted));
    }

    /// Raise or lower your hand.
    pub fn raise_hand(&self, up: bool) {
        let _ = self.tx.send(Cmd::Hand(up));
    }

    /// Send a reaction (an emoji) to everyone.
    pub fn react(&self, emoji: &str) {
        let _ = self.tx.send(Cmd::React(emoji.to_string()));
    }

    /// Ask for the microphones and speakers again (an Event::Devices comes).
    pub fn list_devices(&self) {
        let _ = self.tx.send(Cmd::ListDevices);
    }

    /// Use this microphone (a Device id).
    pub fn use_mic(&self, id: &str) {
        let _ = self.tx.send(Cmd::Mic(id.to_string()));
    }

    /// Play the call on this speaker (a Device id).
    pub fn use_speaker(&self, id: &str) {
        let _ = self.tx.send(Cmd::Speaker(id.to_string()));
    }

    /// Use this camera (by name, from `cameras`); None for the first.
    pub fn use_camera(&self, name: Option<String>) {
        let _ = self.tx.send(Cmd::CameraDevice(name));
    }

    pub fn leave(&self) {
        let _ = self.tx.send(Cmd::Leave);
    }

    /// Everything that happened since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

impl Drop for Call {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Leave);
    }
}

const RATE: u32 = 48_000;
const FRAME: u32 = RATE / 100; // 10 ms

/// Send a steady tone, 10 ms at a time.
async fn play_tone(source: NativeAudioSource, hz: f32) {
    let mut phase = 0f32;
    let step = 2.0 * std::f32::consts::PI * hz / RATE as f32;
    let mut tick = tokio::time::interval(Duration::from_millis(10));
    loop {
        tick.tick().await;
        let data: Vec<i16> = (0..FRAME)
            .map(|_| {
                phase = (phase + step) % (2.0 * std::f32::consts::PI);
                (phase.sin() * 8000.0) as i16
            })
            .collect();
        let frame = AudioFrame {
            data: data.into(),
            sample_rate: RATE,
            num_channels: 1,
            samples_per_channel: FRAME,
        };
        if source.capture_frame(&frame).await.is_err() {
            return;
        }
    }
}

/// Measure how loud a remote track arrives (root mean square, 0..1).
async fn measure(
    track: RemoteAudioTrack,
    identity: String,
    levels: Arc<Mutex<HashMap<String, f32>>>,
) {
    let mut stream = NativeAudioStream::new(track.rtc_track(), RATE as i32, 1);
    while let Some(frame) = stream.next().await {
        let n = frame.data.len().max(1) as f32;
        let rms = (frame
            .data
            .iter()
            .map(|s| (*s as f32 / 32768.0).powi(2))
            .sum::<f32>()
            / n)
            .sqrt();
        let mut l = levels.lock().unwrap();
        let e = l.entry(identity.clone()).or_insert(0.0);
        *e = *e * 0.8 + rms * 0.2;
    }
}

fn put_picture(pictures: &Pictures, id: &str, p: Picture) {
    let mut g = pictures.lock().unwrap();
    let n = g.get(id).map_or(1, |(n, _)| n + 1);
    g.insert(id.to_string(), (n, Arc::new(p)));
}

/// Who is being watched: a flag the watcher checks under the pictures lock,
/// and its task.
type Watchers = HashMap<String, (Arc<AtomicBool>, tokio::task::JoinHandle<()>)>;

/// Stop watching someone and drop their picture. The flag is cleared under the
/// same lock the watcher takes to put a picture, so a frame that was being
/// decoded at that moment can't bring the picture back.
fn unwatch(watchers: &mut Watchers, pictures: &Pictures, id: &str) {
    let mut g = pictures.lock().unwrap();
    if let Some((alive, task)) = watchers.remove(id) {
        alive.store(false, Ordering::SeqCst);
        task.abort();
    }
    g.remove(id);
}

/// Turn received frames into pictures (at most `fps` a second, waking the
/// window for each), until the stream ends or `alive` is cleared.
async fn watch(
    track: RemoteVideoTrack,
    identity: String,
    pictures: Pictures,
    alive: Arc<AtomicBool>,
    fps: u64,
    wake: Wake,
) {
    let mut stream = NativeVideoStream::new(track.rtc_track());
    let gap = Duration::from_millis(1000 / fps);
    let mut last = std::time::Instant::now() - gap;
    while let Some(frame) = stream.next().await {
        if last.elapsed() < gap {
            continue;
        }
        last = std::time::Instant::now();
        let pic = video::i420_to_picture(&frame.buffer.to_i420());
        let mut g = pictures.lock().unwrap();
        if !alive.load(Ordering::SeqCst) {
            return;
        }
        let n = g.get(&identity).map_or(1, |(n, _)| n + 1);
        g.insert(identity.clone(), (n, Arc::new(pic)));
        drop(g);
        // the window draws it now, not whenever something else redraws
        wake();
    }
    let mut g = pictures.lock().unwrap();
    if alive.load(Ordering::SeqCst) {
        g.remove(&identity);
        drop(g);
        wake();
    }
}

/// Ask the server for the layer of each received video that fits how large
/// it is drawn here (only when that changes).
fn ask_layers(
    room: &Room,
    views: &HashMap<String, f32>,
    asked: &mut HashMap<String, VideoQuality>,
) {
    for p in room.remote_participants().into_values() {
        let id = p.identity().to_string();
        for publication in p.track_publications().into_values() {
            if publication.kind() != TrackKind::Video
                || !publication.is_subscribed()
                || !publication.simulcasted()
            {
                continue;
            }
            let key = picture_key(&id, publication.source());
            let want = quality::layer_for(views.get(&key).copied().unwrap_or(0.0));
            if asked.get(&key) != Some(&want) {
                publication.set_video_quality(want);
                asked.insert(key, want);
            }
        }
    }
}

fn people(
    room: &Room,
    levels: &HashMap<String, f32>,
    speaking: &[String],
    camera_on: bool,
    screen_on: bool,
    hands: &HashMap<String, bool>,
    my_hand: bool,
) -> Vec<Person> {
    let me = room.local_participant();
    let mut out = vec![Person {
        identity: me.identity().to_string(),
        name: me.name(),
        me: true,
        speaking: speaking.contains(&me.identity().to_string()),
        muted: me
            .track_publications()
            .values()
            .filter(|p| p.source() == TrackSource::Microphone)
            .all(|p| p.is_muted()),
        heard: 0.0,
        camera: camera_on,
        screen: screen_on,
        hand: my_hand,
    }];
    let mut others: Vec<RemoteParticipant> = room.remote_participants().into_values().collect();
    others.sort_by_key(|p| p.identity().to_string());
    for p in others {
        let id = p.identity().to_string();
        let pubs = p.track_publications();
        let mics: Vec<_> = pubs
            .values()
            .filter(|t| t.source() == TrackSource::Microphone)
            .collect();
        out.push(Person {
            camera: pubs
                .values()
                .any(|t| t.source() == TrackSource::Camera && !t.is_muted()),
            screen: pubs
                .values()
                .any(|t| t.source() == TrackSource::Screenshare && !t.is_muted()),
            speaking: speaking.contains(&id),
            muted: mics.is_empty() || mics.iter().all(|t| t.is_muted()),
            heard: levels.get(&id).copied().unwrap_or(0.0),
            hand: hands.get(&id).copied().unwrap_or(false),
            name: p.name(),
            identity: id,
            me: false,
        });
    }
    out
}

/// The microphones and speakers (made-up ones when a tone stands in for
/// the microphone).
fn devices(platform: Option<&PlatformAudio>) -> Event {
    match platform {
        Some(p) => Event::Devices {
            mics: p
                .recording_devices()
                .map(|d| Device {
                    id: d.id.as_str().to_string(),
                    name: d.name,
                })
                .collect(),
            speakers: p
                .playout_devices()
                .map(|d| Device {
                    id: d.id.as_str().to_string(),
                    name: d.name,
                })
                .collect(),
        },
        None => Event::Devices {
            mics: vec![Device {
                id: "test-mic".into(),
                name: "Test microphone".into(),
            }],
            speakers: vec![Device {
                id: "test-speakers".into(),
                name: "Test speakers".into(),
            }],
        },
    }
}

/// Start feeding the camera `name` (None: the first) into `source`: your
/// own picture goes to the pictures (ME), a camera problem to the app.
fn start_camera(
    video: Video,
    name: Option<String>,
    source: NativeVideoSource,
    limits: Arc<video::Limits>,
    pictures: &Pictures,
    etx: &Sender<Event>,
    wake: &Wake,
) -> video::Feed {
    let pics = pictures.clone();
    let (etx2, wake2, wake3) = (etx.clone(), wake.clone(), wake.clone());
    // (pictures come at the sending rate: a wake each)
    video::start_feed(
        video,
        name,
        source,
        limits,
        move |pic| {
            match pic {
                Some(pic) => put_picture(&pics, ME, pic),
                None => {
                    pics.lock().unwrap().remove(ME);
                }
            }
            wake3();
        },
        move |why| {
            let _ = etx2.send(Event::Trouble(why));
            wake2();
        },
    )
}

async fn run(
    join: Join,
    audio: Audio,
    video: Video,
    mut crx: tokio::sync::mpsc::UnboundedReceiver<Cmd>,
    etx: &Sender<Event>,
    wake: &Wake,
    pictures: Pictures,
) -> Option<String> {
    let emit = |e: Event| {
        let ok = etx.send(e).is_ok();
        wake();
        ok
    };
    // the microphone (and speakers) only for real calls
    let platform = match audio {
        Audio::Platform => match PlatformAudio::new() {
            Ok(p) => Some(p),
            Err(e) => return Some(format!("Couldn't use the microphone: {}", e)),
        },
        Audio::Tone(_) => None,
    };
    // adaptive stream: layers follow what is shown here (set_view_sizes);
    // dynacast: layers no one watches aren't encoded at all
    let mut options = RoomOptions::default();
    options.adaptive_stream = true;
    options.dynacast = true;
    let (room, mut events) = match Room::connect(&join.url, &join.token, options).await {
        Ok(x) => x,
        Err(e) => return Some(format!("Couldn't join the call: {}", e)),
    };
    let source = match (&platform, audio) {
        (Some(p), _) => p.rtc_source(),
        (None, Audio::Tone(hz)) => {
            let src = NativeAudioSource::new(AudioSourceOptions::default(), RATE, 1, 100);
            tokio::spawn(play_tone(src.clone(), hz));
            RtcAudioSource::Native(src)
        }
        (None, Audio::Platform) => unreachable!(),
    };
    let track = LocalAudioTrack::create_audio_track("microphone", source);
    let publication = match room
        .local_participant()
        .publish_track(
            LocalTrack::Audio(track),
            TrackPublishOptions {
                source: TrackSource::Microphone,
                ..Default::default()
            },
        )
        .await
    {
        Ok(p) => p,
        Err(e) => {
            let _ = room.close().await;
            return Some(format!("Couldn't send your audio: {}", e));
        }
    };
    emit(Event::Joined);
    emit(devices(platform.as_ref()));
    // raised hands (yours, and the others' from their messages)
    let mut my_hand = false;
    let mut hands: HashMap<String, bool> = HashMap::new();
    // the camera picked (None: the first)
    let mut camera_name: Option<String> = None;
    let levels: Arc<Mutex<HashMap<String, f32>>> = Default::default();
    let mut speaking: Vec<String> = vec![];
    // the camera while it's on: what's published and what feeds it
    let mut camera: Option<(LocalTrackPublication, video::Feed, NativeVideoSource)> = None;
    let mut screen: Option<(LocalTrackPublication, video::Feed)> = None;
    // how large and often to send, following this computer's connection
    let camera_limits = video::Limits::new(quality::Kind::Camera);
    let screen_limits = video::Limits::new(quality::Kind::Screen);
    // how wide each picture is drawn here, and the layer asked for per track
    let mut views: HashMap<String, f32> = HashMap::new();
    let mut asked: HashMap<String, VideoQuality> = HashMap::new();
    let mut watchers: Watchers = HashMap::new();
    let mut last_people: Vec<Person> = vec![];
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    let why = loop {
        tokio::select! {
            ev = events.recv() => match ev {
                None => break Some("The call ended.".to_string()),
                Some(RoomEvent::Disconnected { reason }) => break Some(format!("Disconnected from the call ({:?}).", reason)),
                Some(RoomEvent::ActiveSpeakersChanged { speakers }) => {
                    speaking = speakers.iter().map(|p| p.identity().to_string()).collect();
                }
                Some(RoomEvent::TrackSubscribed { track: RemoteTrack::Audio(t), participant, .. }) => {
                    if platform.is_none() {
                        tokio::spawn(measure(t, participant.identity().to_string(), levels.clone()));
                    }
                }
                Some(RoomEvent::TrackSubscribed { track: RemoteTrack::Video(t), publication, participant }) => {
                    let source = publication.source();
                    let key = picture_key(&participant.identity().to_string(), source);
                    unwatch(&mut watchers, &pictures, &key);
                    let alive = Arc::new(AtomicBool::new(true));
                    let fps = if source == TrackSource::Screenshare { quality::SCREEN_FPS as u64 } else { video::FPS };
                    let task = tokio::spawn(watch(t, key.clone(), pictures.clone(), alive.clone(), fps, wake.clone()));
                    watchers.insert(key.clone(), (alive, task));
                    asked.remove(&key);
                    ask_layers(&room, &views, &mut asked);
                }
                Some(RoomEvent::TrackUnsubscribed { track: RemoteTrack::Video(_), publication, participant }) | Some(RoomEvent::TrackUnpublished { publication, participant }) => {
                    let source = publication.source();
                    if !participant.track_publications().values().any(|t| t.source() == source && t.is_subscribed()) {
                        let key = picture_key(&participant.identity().to_string(), source);
                        unwatch(&mut watchers, &pictures, &key);
                        asked.remove(&key);
                    }
                }
                Some(RoomEvent::DataReceived { payload, topic: Some(topic), participant: Some(p), .. }) if topic == TOPIC => {
                    let id = p.identity().to_string();
                    match Signal::parse(&payload) {
                        Some(Signal::Hand(up)) => {
                            hands.insert(id, up);
                        }
                        Some(Signal::React(emoji)) => {
                            emit(Event::Reaction { identity: id, emoji });
                        }
                        None => {}
                    }
                }
                // someone new hears from you that your hand is up
                Some(RoomEvent::ParticipantConnected(p)) => {
                    if my_hand {
                        signal(&room, Signal::Hand(true), vec![p.identity()]).await;
                    }
                }
                Some(RoomEvent::ParticipantDisconnected(p)) => {
                    let id = p.identity().to_string();
                    hands.remove(&id);
                    unwatch(&mut watchers, &pictures, &id);
                    unwatch(&mut watchers, &pictures, &screen_key(&id));
                }
                Some(RoomEvent::ConnectionQualityChanged { quality: q, participant: Participant::Local(_) }) => {
                    let net = quality::Network::from(q);
                    camera_limits.set(quality::limits(quality::Kind::Camera, net));
                    screen_limits.set(quality::limits(quality::Kind::Screen, net));
                }
                Some(_) => {}
            },
            // what the app asks, as soon as it asks
            cmd = crx.recv() => match cmd {
                Some(Cmd::Mute(true)) => publication.mute(),
                Some(Cmd::Mute(false)) => publication.unmute(),
                Some(Cmd::Camera(true)) if camera.is_none() => {
                    let source = NativeVideoSource::new(VideoResolution { width: quality::CAMERA_WIDTH, height: quality::CAMERA_WIDTH * 9 / 16 }, false);
                    let track = LocalVideoTrack::create_video_track("camera", RtcVideoSource::Native(source.clone()));
                    match room.local_participant().publish_track(LocalTrack::Video(track), quality::camera_options()).await {
                        Ok(p) => {
                            let feed = start_camera(video, camera_name.clone(), source.clone(), camera_limits.clone(), &pictures, etx, wake);
                            camera = Some((p, feed, source));
                        }
                        Err(e) => {
                            emit(Event::Trouble(format!("Couldn't send your video: {}", e)));
                        }
                    }
                }
                Some(Cmd::Camera(false)) => {
                    if let Some((p, feed, _)) = camera.take() {
                        drop(feed);
                        let _ = room.local_participant().unpublish_track(&p.sid()).await;
                        pictures.lock().unwrap().remove(ME);
                    }
                }
                Some(Cmd::Camera(_)) => {}
                // another camera: the same track, fed from it
                Some(Cmd::CameraDevice(name)) => {
                    camera_name = name;
                    if let Some((_, feed, source)) = camera.as_mut() {
                        let old = std::mem::replace(feed, start_camera(video, camera_name.clone(), source.clone(), camera_limits.clone(), &pictures, etx, wake));
                        drop(old);
                    }
                }
                Some(Cmd::Hand(up)) => {
                    my_hand = up;
                    signal(&room, Signal::Hand(up), vec![]).await;
                }
                Some(Cmd::React(emoji)) => {
                    signal(&room, Signal::React(emoji.clone()), vec![]).await;
                    emit(Event::Reaction { identity: ME.to_string(), emoji });
                }
                Some(Cmd::ListDevices) => {
                    emit(devices(platform.as_ref()));
                }
                Some(Cmd::Mic(id)) => {
                    if let Some(p) = &platform {
                        if let Err(e) = p.switch_recording_device(&RecordingDeviceId::from_unchecked_guid(&id)) {
                            emit(Event::DeviceTrouble(format!("Couldn't switch the microphone: {}", e)));
                        }
                    }
                }
                Some(Cmd::Speaker(id)) => {
                    if let Some(p) = &platform {
                        if let Err(e) = p.switch_playout_device(&PlayoutDeviceId::from_unchecked_guid(&id)) {
                            emit(Event::DeviceTrouble(format!("Couldn't switch the speaker: {}", e)));
                        }
                    }
                }
                Some(Cmd::Screen(share)) => {
                    if let Some((p, feed)) = screen.take() {
                        drop(feed);
                        let _ = room.local_participant().unpublish_track(&p.sid()).await;
                    }
                    // test pictures stand in for the screen too
                    let share = match (share, video) {
                        (Some(Share::Pattern(c)), _) => Some(Share::Pattern(c)),
                        (Some(_), Video::Pattern(c)) => Some(Share::Pattern([255 - c[0], 255 - c[1], 255 - c[2]])),
                        (s, _) => s,
                    };
                    if let Some(share) = share {
                        let (w, h) = (quality::SCREEN_WIDTH, quality::SCREEN_WIDTH * 9 / 16);
                        let source = NativeVideoSource::new(VideoResolution { width: w, height: h }, true);
                        let track = LocalVideoTrack::create_video_track("screen", RtcVideoSource::Native(source.clone()));
                        match room.local_participant().publish_track(LocalTrack::Video(track), quality::screen_options()).await {
                            Ok(p) => {
                                let pics = pictures.clone();
                                let (etx2, wake2, wake3) = (etx.clone(), wake.clone(), wake.clone());
                                // your own screen, as the others see it
                                let mine = screen_key(ME);
                                let feed = screen::start_screen(share, source, screen_limits.clone(), move |pic| {
                                    match pic {
                                        Some(pic) => put_picture(&pics, &mine, pic),
                                        None => {
                                            pics.lock().unwrap().remove(&mine);
                                        }
                                    }
                                    wake3();
                                }, move |why| {
                                    let _ = etx2.send(Event::ScreenEnded(why));
                                    wake2();
                                });
                                screen = Some((p, feed));
                            }
                            Err(e) => {
                                emit(Event::ScreenEnded(Some(ScreenTrouble::Failed(format!("Couldn't send your screen: {}", e)))));
                            }
                        }
                    }
                }
                Some(Cmd::Views(v)) => {
                    views = v;
                    ask_layers(&room, &views, &mut asked);
                }
                Some(Cmd::Leave) | None => {
                    let _ = room.close().await;
                    return None;
                }
            },
            _ = tick.tick() => {
                // a screen share that ended by itself: take it down
                if screen.as_ref().is_some_and(|(_, f)| f.stopped()) {
                    if let Some((p, _)) = screen.take() {
                        let _ = room.local_participant().unpublish_track(&p.sid()).await;
                    }
                }
                let now = people(&room, &levels.lock().unwrap(), &speaking, camera.is_some(), screen.is_some(), &hands, my_hand);
                // levels drift a little all the time: compare them coarsely
                let coarse = |v: &[Person]| v.iter().map(|p| (p.identity.clone(), p.speaking, p.muted, p.camera, p.screen, p.hand, (p.heard * 20.0) as i32)).collect::<Vec<_>>();
                if coarse(&now) != coarse(&last_people) {
                    last_people = now.clone();
                    if !emit(Event::People(now)) {
                        let _ = room.close().await;
                        return None;
                    }
                }
            }
        }
    };
    let _ = room.close().await;
    drop(platform);
    why
}
