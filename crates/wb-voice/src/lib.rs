// Voice huddles: one LiveKit room per conversation. A call runs on its own
// thread with its own async runtime, so the window never waits for it; the
// app sends commands (mute, leave) and reads events (who is in the call, who
// is speaking) through channels, like the sync connection in wb_net.
//
// With `Audio::Platform` the computer's microphone is sent and everyone
// else is played on its speakers (WebRTC's own audio device handling, with
// echo cancellation). `Audio::Tone` sends a steady tone instead and measures
// how loud the others arrive, without touching any audio device; tests use it.

use futures_util::StreamExt;
use livekit::options::TrackPublishOptions;
use livekit::prelude::*;
use livekit::webrtc::audio_frame::AudioFrame;
use livekit::webrtc::audio_source::native::NativeAudioSource;
use livekit::webrtc::audio_source::{AudioSourceOptions, RtcAudioSource};
use livekit::webrtc::audio_stream::native::NativeAudioStream;
use std::collections::HashMap;
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
    /// LiveKit identity: "<member id>-<device>" (see workboard-server).
    pub identity: String,
    pub name: String,
    pub me: bool,
    pub speaking: bool,
    pub muted: bool,
    /// How loud their audio arrives here, 0..1 (only measured with Audio::Tone).
    pub heard: f32,
}

impl Person {
    /// The workspace member this person is.
    pub fn member(&self) -> Option<u64> {
        self.identity.split('-').next()?.parse().ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Joined,
    People(Vec<Person>),
    /// The call ended for this computer; holds why when it wasn't asked for.
    Left(Option<String>),
}

enum Cmd {
    Mute(bool),
    Leave,
}

pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// A call in progress. Dropping it leaves the call.
pub struct Call {
    tx: Sender<Cmd>,
    rx: Receiver<Event>,
}

impl Call {
    pub fn start(join: Join, audio: Audio, wake: Wake) -> Call {
        let (tx, crx) = mpsc::channel();
        let (etx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("workboard-voice".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = etx.send(Event::Left(Some(e.to_string())));
                        wake();
                        return;
                    }
                };
                let why = rt.block_on(run(join, audio, crx, &etx, &wake));
                let _ = etx.send(Event::Left(why));
                wake();
            })
            .expect("start voice thread");
        Call { tx, rx }
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self.tx.send(Cmd::Mute(muted));
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
        let frame = AudioFrame { data: data.into(), sample_rate: RATE, num_channels: 1, samples_per_channel: FRAME };
        if source.capture_frame(&frame).await.is_err() {
            return;
        }
    }
}

/// Measure how loud a remote track arrives (root mean square, 0..1).
async fn measure(track: RemoteAudioTrack, identity: String, levels: Arc<Mutex<HashMap<String, f32>>>) {
    let mut stream = NativeAudioStream::new(track.rtc_track(), RATE as i32, 1);
    while let Some(frame) = stream.next().await {
        let n = frame.data.len().max(1) as f32;
        let rms = (frame.data.iter().map(|s| (*s as f32 / 32768.0).powi(2)).sum::<f32>() / n).sqrt();
        let mut l = levels.lock().unwrap();
        let e = l.entry(identity.clone()).or_insert(0.0);
        *e = *e * 0.8 + rms * 0.2;
    }
}

fn people(room: &Room, levels: &HashMap<String, f32>, speaking: &[String]) -> Vec<Person> {
    let me = room.local_participant();
    let mut out = vec![Person {
        identity: me.identity().to_string(),
        name: me.name(),
        me: true,
        speaking: speaking.contains(&me.identity().to_string()),
        muted: me.track_publications().values().all(|p| p.is_muted()),
        heard: 0.0,
    }];
    let mut others: Vec<RemoteParticipant> = room.remote_participants().into_values().collect();
    others.sort_by_key(|p| p.identity().to_string());
    for p in others {
        let id = p.identity().to_string();
        let pubs = p.track_publications();
        out.push(Person {
            speaking: speaking.contains(&id),
            muted: pubs.is_empty() || pubs.values().all(|t| t.is_muted()),
            heard: levels.get(&id).copied().unwrap_or(0.0),
            name: p.name(),
            identity: id,
            me: false,
        });
    }
    out
}

async fn run(join: Join, audio: Audio, crx: Receiver<Cmd>, etx: &Sender<Event>, wake: &Wake) -> Option<String> {
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
    let (room, mut events) = match Room::connect(&join.url, &join.token, RoomOptions::default()).await {
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
    let publication = match room.local_participant().publish_track(LocalTrack::Audio(track), TrackPublishOptions { source: TrackSource::Microphone, ..Default::default() }).await {
        Ok(p) => p,
        Err(e) => {
            let _ = room.close().await;
            return Some(format!("Couldn't send your audio: {}", e));
        }
    };
    emit(Event::Joined);
    let levels: Arc<Mutex<HashMap<String, f32>>> = Default::default();
    let mut speaking: Vec<String> = vec![];
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
                Some(_) => {}
            },
            _ = tick.tick() => {
                loop {
                    match crx.try_recv() {
                        Ok(Cmd::Mute(true)) => publication.mute(),
                        Ok(Cmd::Mute(false)) => publication.unmute(),
                        Ok(Cmd::Leave) | Err(mpsc::TryRecvError::Disconnected) => {
                            let _ = room.close().await;
                            return None;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                let now = people(&room, &levels.lock().unwrap(), &speaking);
                // levels drift a little all the time: compare them coarsely
                let coarse = |v: &[Person]| v.iter().map(|p| (p.identity.clone(), p.speaking, p.muted, (p.heard * 20.0) as i32)).collect::<Vec<_>>();
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
