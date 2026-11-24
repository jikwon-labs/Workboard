// Voice messages: the microphone button in a message box records one
// (media::Recorder, AVAudioRecorder on macOS), Stop puts it in the box like
// a picked file, where it can be played or removed before it is sent. In a
// message it shows as a card with a play button, how far it has played and
// how long it is (media::Player, the video player, plays sounds too).
// Where nothing records (`os::records_audio`) the button isn't shown; where
// nothing plays (`os::plays_audio`) a voice message is a file card that
// saves the file, as in apps from before voice messages.

use super::files::Transfer;
use super::icons::{icon_button, icon_button_ex, line_icon};
use super::*;
use crate::media;
use egui::Response;
use std::cell::RefCell;
use std::path::Path;

#[derive(Default)]
pub(super) struct VoiceRt {
    /// Recording now: in the thread's box (true) or the channel's, and the
    /// recorder.
    rec: Option<(bool, media::Recorder)>,
    /// The sound playing or paused: its file (sha256) and its player.
    player: RefCell<Option<(String, media::Player)>>,
    /// A sound asked to play that isn't fetched yet: it starts once it is.
    wanted: RefCell<Option<String>>,
}

/// Where recordings wait until they are in the cache (files.rs copies them
/// there). Old ones are cleared when a recording starts.
fn recordings_dir() -> PathBuf {
    std::env::temp_dir().join("workboard-voice")
}

fn clear_old_recordings(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for e in rd.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|m| now.duration_since(m).ok())
            .is_some_and(|d| d.as_secs() > 24 * 3600);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The microphone button in a message box (red while recording there).
pub(super) fn voice_tool(ui: &mut Ui, t: &Theme, recording: bool) -> Response {
    let (name, tip) = if recording {
        ("Stop recording", "Stop recording")
    } else {
        ("Record a voice message", "Record a voice message")
    };
    let b = icon_button_ex(
        ui,
        t,
        Icon::Mic,
        "",
        name,
        recording,
        0.0,
        t.text,
        if recording { t.danger } else { t.text2 },
    );
    b.on_hover_text(tr(tip))
}

impl App {
    /// Recording in this box now.
    pub(super) fn recording_in(&self, thread: bool) -> bool {
        self.voice.rec.as_ref().is_some_and(|r| r.0 == thread)
    }

    /// The microphone button was pressed in a box: start recording there,
    /// or stop and put the recording in the box.
    pub(super) fn voice_pressed(&mut self, thread: bool) {
        if self.recording_in(thread) {
            self.stop_recording(true);
            return;
        }
        // one recording at a time: one going in the other box is kept
        if self.voice.rec.is_some() {
            self.stop_recording(true);
        }
        match media::mic_access() {
            media::MicAccess::Granted => {}
            media::MicAccess::Asking => {
                self.toast(tr("Allow the microphone, then press record again."));
                return;
            }
            media::MicAccess::Denied => {
                self.toast(tr(
                    "Workboard can't use the microphone. Turn it on in System Settings > Privacy & Security > Microphone.",
                ));
                return;
            }
        }
        self.pause_sound();
        let dir = recordings_dir();
        clear_old_recordings(&dir);
        match media::Recorder::start(&dir) {
            Ok(r) => self.voice.rec = Some((thread, r)),
            Err(e) => self.toast(tr(&e)),
        }
    }

    /// Stop recording: into its box (`keep`) or thrown away.
    pub(super) fn stop_recording(&mut self, keep: bool) {
        let Some((thread, r)) = self.voice.rec.take() else {
            return;
        };
        if !keep {
            r.cancel();
            return;
        }
        // a slip of the button: nothing worth sending
        if r.secs() < 0.5 {
            r.cancel();
            self.toast(tr("Too short to send"));
            return;
        }
        match r.stop() {
            Some(path) => {
                self.add_media(thread, Some(path), None);
            }
            None => self.toast(tr("The recording couldn't be saved.")),
        }
    }

    /// Over a message box while it records: the time so far, how loud it
    /// is, and buttons to throw it away or stop. Stops by itself at
    /// `media::VOICE_MAX_SECS`.
    pub(super) fn recording_bar(&mut self, ui: &mut Ui, thread: bool) {
        let Some((_, r)) = self.voice.rec.as_ref().filter(|r| r.0 == thread) else {
            return;
        };
        let secs = r.secs();
        if secs >= media::VOICE_MAX_SECS {
            self.stop_recording(true);
            self.toast(tr("Voice messages can be up to 5 minutes long."));
            return;
        }
        let level = r.level();
        let t = self.t();
        let mut stop = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (dot, resp) = ui.allocate_exact_size(vec2(12.0, 20.0), Sense::hover());
            name_it(&resp, "Recording");
            // the dot breathes while it records
            let phase = (ui.input(|i| i.time) * 3.0).sin() as f32 * 0.5 + 0.5;
            ui.painter().circle_filled(
                dot.center(),
                4.0 + phase,
                t.danger.gamma_multiply(0.6 + 0.4 * phase),
            );
            ui.label(
                RichText::new(format!(
                    "{} {} / {}",
                    tr("Recording"),
                    media::clock(secs),
                    media::clock(media::VOICE_MAX_SECS)
                ))
                .color(t.text)
                .monospace(),
            );
            // how loud: a bar that follows the voice
            let (bar, _) = ui.allocate_exact_size(vec2(60.0, 6.0), Sense::hover());
            ui.painter().rect_filled(bar, 3.0, t.hover);
            let mut lit = bar;
            lit.set_width(bar.width() * level.clamp(0.05, 1.0));
            ui.painter().rect_filled(lit, 3.0, t.danger);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                stop =
                    icon_button(ui, &t, Icon::Stop, tr("Stop"), "Stop recording", true).clicked();
                cancel = icon_button(ui, &t, Icon::Trash, "", "Discard recording", false)
                    .on_hover_text(tr("Discard recording"))
                    .clicked();
            });
        });
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        if stop {
            self.stop_recording(true);
        } else if cancel {
            self.stop_recording(false);
        }
    }

    /// Whether this sound is the one playing now.
    pub(super) fn sound_playing(&self, sha: &str) -> bool {
        self.voice
            .player
            .borrow()
            .as_ref()
            .is_some_and(|(s, p)| s == sha && p.playing())
    }

    /// Play a sound, or pause it when it's the one playing. One that isn't
    /// here yet is fetched and plays once it is.
    pub(super) fn toggle_sound(&self, a: &Attachment) {
        let mut player = self.voice.player.borrow_mut();
        if let Some((s, p)) = player.as_mut() {
            if *s == a.file {
                if p.playing() {
                    p.pause();
                } else {
                    p.play();
                }
                return;
            }
            p.pause();
        }
        *player = None;
        *self.voice.wanted.borrow_mut() = Some(a.file.clone());
        drop(player);
        self.start_wanted(a);
    }

    /// Start the sound asked for once its file is here.
    fn start_wanted(&self, a: &Attachment) {
        if self.voice.wanted.borrow().as_deref() != Some(a.file.as_str()) {
            return;
        }
        let Some(path) = self.files.local(&a.file).and(self.files.playable(a)) else {
            self.files.fetch(a);
            return;
        };
        *self.voice.wanted.borrow_mut() = None;
        match media::Player::new(&path, 2, 2) {
            Some(mut p) => {
                p.play();
                *self.voice.player.borrow_mut() = Some((a.file.clone(), p));
            }
            None => eprintln!("workboard: couldn't play {}", a.name),
        }
    }

    /// Pause whatever sound is playing (a recording is about to start).
    pub(super) fn pause_sound(&self) {
        if let Some((_, p)) = self.voice.player.borrow_mut().as_mut() {
            p.pause();
        }
    }

    /// How far the sound has played (seconds) and its length, when it's
    /// the one in the player.
    fn sound_progress(&self, a: &Attachment) -> Option<(f32, f32)> {
        let player = self.voice.player.borrow();
        let (s, p) = player.as_ref()?;
        if *s != a.file {
            return None;
        }
        let dur = if p.duration() > 0.0 {
            p.duration()
        } else {
            a.secs
        };
        Some((p.position().min(dur), dur))
    }

    fn seek_sound(&self, a: &Attachment, frac: f32) {
        if let Some((s, p)) = self.voice.player.borrow_mut().as_mut() {
            if *s == a.file {
                let dur = if p.duration() > 0.0 {
                    p.duration()
                } else {
                    a.secs
                };
                p.seek(dur * frac.clamp(0.0, 1.0));
            }
        }
    }

    /// A voice message in a message: play / pause, how far it has played
    /// (click the bar to jump there) and its length.
    pub(super) fn audio_card(&self, ui: &mut Ui, a: &Attachment, avail: f32) {
        let t = self.t();
        self.start_wanted(a);
        let size = vec2(280.0_f32.min(avail).max(180.0), 48.0);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        if !ui.is_rect_visible(rect) {
            return;
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 24.0, t.card);
        painter.rect_stroke(
            rect,
            24.0,
            Stroke::new(1.0_f32, t.border),
            egui::StrokeKind::Inside,
        );
        let playing = self.sound_playing(&a.file);
        let loading = self.voice.wanted.borrow().as_deref() == Some(a.file.as_str());
        let btn = Rect::from_center_size(rect.left_center() + vec2(26.0, 0.0), vec2(34.0, 34.0));
        let b = ui.interact(btn, ui.id().with(("play_sound", &a.file)), Sense::click());
        let name = if playing { "Pause" } else { "Play" };
        b.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                format!("{} {}", name, a.name),
            )
        });
        painter.circle_filled(
            btn.center(),
            17.0,
            if b.hovered() {
                t.primary.gamma_multiply(0.85)
            } else {
                t.primary
            },
        );
        if playing {
            // two bars
            for dx in [-3.5, 3.5] {
                painter.rect_filled(
                    Rect::from_center_size(btn.center() + vec2(dx, 0.0), vec2(3.5, 12.0)),
                    1.0,
                    t.on_primary,
                );
            }
        } else {
            line_icon(
                &painter,
                Icon::Play,
                btn.center() + vec2(1.0, 0.0),
                t.on_primary,
            );
        }
        if b.clicked() {
            self.toggle_sound(a);
        }
        b.on_hover_text(tr(if playing { "Pause" } else { "Play" }));
        // the bar and the time under it
        let left = btn.right() + 10.0;
        let bar = Rect::from_min_max(
            pos2(left, rect.center().y - 8.0),
            pos2(rect.right() - 16.0, rect.center().y - 4.0),
        );
        let (pos, dur) = self.sound_progress(a).unwrap_or((0.0, a.secs));
        let frac = if dur > 0.0 { pos / dur } else { 0.0 };
        let hit = ui.interact(
            bar.expand2(vec2(0.0, 6.0)),
            ui.id().with(("seek_sound", &a.file)),
            Sense::click(),
        );
        name_it(&hit, &format!("Progress of {}", a.name));
        if hit.clicked() {
            if let Some(p) = hit.interact_pointer_pos() {
                self.seek_sound(a, (p.x - bar.left()) / bar.width());
            }
        }
        painter.rect_filled(bar, 2.0, t.hover);
        let mut done = bar;
        done.set_width(bar.width() * frac.clamp(0.0, 1.0));
        painter.rect_filled(done, 2.0, t.primary);
        let state = match self.files.state(&a.file) {
            Some(Transfer::Down(f)) if loading => trf!("Loading {}%", (f * 100.0) as u32),
            Some(Transfer::Failed(e)) if loading => App::error_text(&e),
            _ if loading => tr("Loading...").to_string(),
            _ if pos > 0.0 => format!("{} / {}", media::clock(pos), media::clock(dur)),
            _ => media::clock(dur),
        };
        painter.text(
            pos2(left, rect.center().y + 9.0),
            Align2::LEFT_CENTER,
            state,
            FontId::proportional(11.5),
            t.text3,
        );
        if playing || loading {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// The player holds this sound (tests).
    #[cfg(test)]
    pub(super) fn voice_player_has(&self, sha: &str) -> bool {
        self.voice
            .player
            .borrow()
            .as_ref()
            .is_some_and(|(s, _)| s == sha)
    }
}
