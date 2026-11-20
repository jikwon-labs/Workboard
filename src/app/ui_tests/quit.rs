// Click-through tests for asking before the app quits.

use super::*;

#[test]
fn closing_the_window_asks_before_quitting() {
    use egui::ViewportCommand::{CancelClose, Close};
    let mut h = Ui1::new();
    // the close button: the window stays and the app asks
    h.close_window();
    assert!(h.commands.contains(&CancelClose), "{:?}", h.commands);
    h.settle();
    assert!(matches!(h.app.modal, Some(Modal::Quit)));
    assert!(h.has("Quit Workboard? Messages and notifications stop coming to this computer until you open it again."));

    // Cancel keeps the app open
    h.click("Cancel");
    assert!(h.app.modal.is_none());
    assert!(!h.commands.contains(&Close));

    // Escape too
    h.close_window();
    h.settle();
    assert!(matches!(h.app.modal, Some(Modal::Quit)));
    h.key(egui::Key::Escape);
    assert!(h.app.modal.is_none());

    // Quit closes the window, and that close is let through
    h.close_window();
    h.settle();
    h.click("Quit");
    assert!(h.commands.contains(&Close), "{:?}", h.commands);
    h.close_window();
    assert!(!h.commands.contains(&CancelClose), "{:?}", h.commands);
    assert!(h.app.modal.is_none());
}

#[test]
fn enter_in_the_quit_dialog_quits() {
    let mut h = Ui1::new();
    h.close_window();
    h.settle();
    h.key(egui::Key::Enter);
    assert!(
        h.commands.contains(&egui::ViewportCommand::Close),
        "{:?}",
        h.commands
    );
}
