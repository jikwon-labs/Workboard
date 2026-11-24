// Click-through tests for voice messages: the microphone button in the
// message box, listening before sending, and playing one in a message.

use super::*;

fn set_secs(s: Option<f32>) {
    crate::media::REC_SECS.with(|c| c.set(s));
}

#[test]
#[cfg_attr(
    not(target_os = "macos"),
    ignore = "only macOS records voice messages (os::records_audio)"
)]
fn record_listen_and_send_a_voice_message() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    set_secs(Some(4.0));
    h.click("Record a voice message");
    assert!(h.app.recording_in(false));
    h.frame(vec![]);
    // the bar over the box: how long so far, Stop and the bin
    assert!(h.has("Recording") && h.has("Discard recording"));
    assert!(h.texts().iter().any(|t| t.contains("0:04 / 5:00")));
    h.click("Stop recording");
    set_secs(None);
    assert!(!h.app.recording_in(false));
    h.until("the recording ready in the message box", |h| {
        h.app.pending_ready(false).0 && !h.app.pending_ready(false).1.is_empty()
    });
    h.settle();
    let att = h.app.pending_ready(false).1[0].clone();
    assert_eq!(att.kind, MediaKind::Audio);
    // it can be listened to before it goes
    let play = format!("Play {}", att.name);
    assert!(h.has(&play), "a play button in the box");
    h.click(&play);
    h.until("it plays", |h| h.app.sound_playing(&att.file));
    h.click(&format!("Pause {}", att.name));
    assert!(!h.app.sound_playing(&att.file));
    // Enter in the empty box sends it alone
    h.click("Message composer");
    h.key(egui::Key::Enter);
    let m = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .clone();
    assert_eq!(m.attachments, vec![att.clone()]);
    h.settle();
    // in the conversation: a card with play, its bar and its length
    assert!(h.has(&play) && h.has(&format!("Progress of {}", att.name)));
    assert!(h.texts().iter().any(|t| t == "0:04"));
}

#[test]
#[cfg_attr(
    not(target_os = "macos"),
    ignore = "only macOS records voice messages (os::records_audio)"
)]
fn a_recording_can_be_thrown_away_and_a_slip_is_not_kept() {
    let mut h = Ui1::new();
    h.click("general");
    set_secs(Some(2.0));
    h.click("Record a voice message");
    h.frame(vec![]);
    h.click("Discard recording");
    assert!(!h.app.recording_in(false));
    assert!(h.app.chat.channel.files.is_empty(), "nothing in the box");
    // pressed and let go at once: too short, nothing kept
    set_secs(Some(0.2));
    h.click("Record a voice message");
    h.click("Stop recording");
    assert!(h.app.chat.channel.files.is_empty());
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|t| t.0 == "Too short to send"));
    // at five minutes it stops by itself and goes in the box
    set_secs(Some(301.0));
    h.click("Record a voice message");
    h.frame(vec![]);
    set_secs(None);
    assert!(!h.app.recording_in(false));
    assert_eq!(h.app.chat.channel.files.len(), 1);
}

#[test]
#[cfg_attr(
    not(target_os = "macos"),
    ignore = "only macOS records voice messages (os::records_audio)"
)]
fn the_thread_reply_box_records_too() {
    let mut h = Ui1::new();
    h.click("general");
    h.send_message("question");
    let cid = h.app.chat.open.unwrap();
    let mid = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .last()
        .unwrap()
        .id;
    h.app.show_thread(cid, Some(mid));
    h.frame(vec![]);
    set_secs(Some(2.0));
    h.click_side("Record a voice message", true);
    assert!(h.app.recording_in(true), "recording in the thread's box");
    h.click_side("Stop recording", true);
    set_secs(None);
    h.until("the recording in the reply box", |h| {
        !h.app.pending_ready(true).1.is_empty()
    });
    assert!(h.app.chat.channel.files.is_empty(), "not the channel's box");
}

#[test]
fn a_voice_message_plays_where_sounds_play_and_is_a_file_elsewhere() {
    let mut h = Ui1::new();
    h.click("general");
    let cid = h.app.chat.open.unwrap();
    let name = "Voice message 2026-11-24 10.00.00.m4a";
    let me = h.app.data.me;
    let mut m = crate::app::slack::new_message(h.app.data.new_id(), me, String::new(), now_ts());
    m.attachments = vec![Attachment {
        file: "ab".repeat(32),
        name: name.into(),
        kind: MediaKind::Audio,
        bytes: 24_000,
        secs: 12.0,
        ..Default::default()
    }];
    h.app
        .data
        .channels
        .iter_mut()
        .find(|c| c.id == cid)
        .unwrap()
        .messages
        .push(m);
    h.settle();
    if crate::os::plays_audio() {
        assert!(h.has(&format!("Play {}", name)));
        assert!(h.texts().iter().any(|t| t == "0:12"));
    } else {
        assert!(h.has(&format!("File {}", name)), "saved like any file");
    }
    // the record button only where it records
    assert_eq!(h.has("Record a voice message"), crate::os::records_audio());
}
