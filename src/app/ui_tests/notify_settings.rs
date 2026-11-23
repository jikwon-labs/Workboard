// The Notifications settings, clicked through: rows that turn a setting on
// and off as a whole, the preview that follows the delivery switches, the
// due-notice time, the minutes before a meeting and keywords as chips.

use super::*;

fn open_dialog() -> Ui1 {
    let mut h = Ui1::new();
    h.click("Inbox");
    h.click("Notification settings");
    assert!(matches!(h.app.modal, Some(Modal::Notifications)));
    h
}

fn keywords(h: &Ui1) -> Vec<String> {
    h.app.data.extra.notify.keyword_list()
}

#[test]
fn a_whole_row_turns_its_setting_on_and_off() {
    let mut h = open_dialog();
    assert!(h.app.data.extra.notify.comments);
    h.click("Comments on my tasks");
    assert!(!h.app.data.extra.notify.comments);
    h.click("Comments on my tasks");
    assert!(
        h.app.data.extra.notify.comments,
        "a second click turns it back on"
    );
    h.click("Notification sound");
    assert!(!h.app.data.extra.notify.sound);
    h.click("The team server connection drops and comes back");
    assert!(!h.app.data.extra.notify.connection);
    assert!(
        matches!(h.app.modal, Some(Modal::Notifications)),
        "the dialog stays open"
    );
}

/// "Notify me while Workboard is quit": on at first, and off sends an empty
/// token to the open workspace's server, so it stops pushing to this Mac.
#[test]
#[cfg_attr(not(target_os = "macos"), ignore = "the row shows on the Mac only")]
fn notifying_while_quit_turns_off_and_tells_the_server() {
    let mut h = open_dialog();
    assert!(!h.app.push_rt.off, "on at first");
    h.click("Notify me while Workboard is quit");
    assert!(h.app.push_rt.off);
    let token = "ab".repeat(32);
    let off = h.app.push_wanted(&token);
    assert!(
        matches!(&off, wb_net::Msg::PushToken { token, .. } if token.is_empty()),
        "turned off: an empty token, {:?}",
        off
    );
    h.click("Notify me while Workboard is quit");
    assert!(!h.app.push_rt.off, "a second click turns it back on");
    let on = h.app.push_wanted(&token);
    assert!(
        matches!(&on, wb_net::Msg::PushToken { token: t, prefs: Some(_), .. } if *t == token),
        "on: the token with this computer's choices, {:?}",
        on
    );
    assert!(
        matches!(h.app.modal, Some(Modal::Notifications)),
        "the dialog stays open"
    );
}

#[test]
fn the_preview_follows_the_delivery_switches() {
    let mut h = open_dialog();
    assert!(
        !h.painted_text("just now").is_empty(),
        "a banner as it comes"
    );
    let counts = h.painted_text("3").len();
    assert!(counts > 0, "the Dock icon's count");
    h.click("Desktop notifications");
    assert!(!h.app.data.extra.notify.desktop);
    assert!(
        !h.painted_text("Off").is_empty(),
        "the banner says banners are off"
    );
    h.click(if cfg!(windows) {
        "Show the unread count on the taskbar icon"
    } else {
        "Show the unread count on the Dock icon"
    });
    assert!(!h.app.data.extra.notify.dock_badge);
    assert_eq!(
        h.painted_text("3").len(),
        counts - 1,
        "no count without the badge"
    );
}

#[test]
fn the_due_notice_time_is_picked_from_a_menu_of_half_hours() {
    let mut h = open_dialog();
    assert_eq!(h.app.data.extra.notify.due_time, "09:00");
    h.click("Due notice time");
    assert!(h.has("9:30 AM"), "the menu opens near the chosen time");
    h.click("7:30 AM");
    assert_eq!(h.app.data.extra.notify.due_time, "07:30");
    assert!(
        h.app.data.extra.notify.due,
        "picking a time doesn't turn the row off"
    );
    h.settle();
    assert!(!h.has("8:00 AM"), "the menu closes");
}

#[test]
fn meeting_minutes_step_by_five_and_wait_while_meetings_are_off() {
    let mut h = open_dialog();
    let minutes = |h: &Ui1| h.app.data.extra.notify.meeting_minutes;
    assert_eq!(minutes(&h), 10);
    h.click("More minutes before a meeting");
    assert_eq!(minutes(&h), 15);
    h.click("Fewer minutes before a meeting");
    h.click("Fewer minutes before a meeting");
    assert_eq!(minutes(&h), 5);
    h.click("Fewer minutes before a meeting");
    assert_eq!(minutes(&h), 1, "down to a minute");
    h.click("More minutes before a meeting");
    assert_eq!(minutes(&h), 5, "back onto the steps of five");
    assert!(
        h.app.data.extra.notify.meetings,
        "the steps don't turn the row off"
    );
    h.app.data.extra.notify.meetings = false;
    h.settle();
    h.click("More minutes before a meeting");
    assert_eq!(minutes(&h), 5, "off: the minutes can't be changed");
}

#[test]
fn keywords_are_added_as_chips_and_taken_out_with_their_x() {
    let mut h = open_dialog();
    h.click("Notification keywords");
    h.type_text("deploy, 배포");
    assert_eq!(
        keywords(&h),
        vec!["deploy"],
        "the comma ends the first word"
    );
    h.key(egui::Key::Enter);
    assert_eq!(keywords(&h), vec!["deploy", "배포"]);
    // Enter leaves the field ready for the next word
    h.type_text("launch");
    h.key(egui::Key::Enter);
    assert_eq!(keywords(&h), vec!["deploy", "배포", "launch"]);
    h.click("Remove keyword 배포");
    assert_eq!(keywords(&h), vec!["deploy", "launch"]);
    h.click("Notification keywords");
    h.type_text("Deploy");
    h.key(egui::Key::Enter);
    assert_eq!(
        keywords(&h),
        vec!["deploy", "launch"],
        "a word already there isn't added twice"
    );
}

#[test]
fn the_settings_page_has_the_same_rows_and_a_link_to_the_system_settings() {
    let mut h = Ui1::new();
    h.app.open_settings("Notifications");
    h.settle();
    h.click("Direct messages");
    assert!(!h.app.data.extra.notify.direct);
    assert!(h.has("New members join"), "the owner sees the admins' row");
    let link = if cfg!(windows) {
        "Open Windows Settings"
    } else {
        "Open System Settings"
    };
    // a test app has nothing to save, so nothing opens
    h.click(link);
    assert!(matches!(h.app.section, Section::Settings));
}
