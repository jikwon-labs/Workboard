// Click-through tests for asking before the app quits. The question itself
// is the system's own alert (`os::ask_quit`), which a test can't draw; its
// answer is handed in here as the alert would.

use super::*;

/// What the open alert answers.
fn answer(h: &mut Ui1, quit: bool) {
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(quit).unwrap();
    h.app.quit.wanted = false;
    h.app.quit.reply = Some(rx);
    h.settle();
}

#[test]
fn closing_the_window_asks_before_quitting() {
    use egui::ViewportCommand::{CancelClose, Close};
    let mut h = Ui1::new();
    // the close button: the window stays and the app asks
    h.close_window();
    assert!(h.commands.contains(&CancelClose), "{:?}", h.commands);
    assert!(h.app.quit.wanted);

    // Cancel keeps the app open
    answer(&mut h, false);
    assert!(h.app.quit.reply.is_none());
    assert!(!h.commands.contains(&Close));

    // Quit closes the window, and that close is let through
    h.close_window();
    assert!(h.app.quit.wanted);
    answer(&mut h, true);
    assert!(h.commands.contains(&Close), "{:?}", h.commands);
    h.close_window();
    assert!(!h.commands.contains(&CancelClose), "{:?}", h.commands);
    assert!(!h.app.quit.wanted);
}

#[test]
fn closing_again_while_the_alert_is_open_asks_once() {
    let mut h = Ui1::new();
    h.close_window();
    // the alert is open, no answer yet
    let (_tx, rx) = std::sync::mpsc::channel();
    h.app.quit.wanted = false;
    h.app.quit.reply = Some(rx);
    h.close_window();
    assert!(
        h.commands.contains(&egui::ViewportCommand::CancelClose),
        "{:?}",
        h.commands
    );
    assert!(!h.app.quit.wanted);
    assert!(h.app.quit.reply.is_some());
}
