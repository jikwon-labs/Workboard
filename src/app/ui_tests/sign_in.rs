// Click-through tests for the sign-in screen, passwords, logging out and
// switching person.

use super::*;

fn sign_in_screen(h: &mut Ui1, addr: &str) {
    h.app.login = Some(crate::app::login::LoginScreen::start(addr));
    h.settle();
}

fn login_page(h: &Ui1) -> Option<crate::app::login::LoginPage> {
    h.app.login.as_ref().map(|l| l.page)
}

#[test]
fn switch_person_on_a_team_server_does_not_become_someone_else() {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    let my_name = h.app.data.member(me).unwrap().name.clone();
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .name
        .clone();
    let count = |h: &Ui1| h.tree.iter().filter(|(n, _)| *n == other).count();
    let before = count(&h);
    // on this computer only: pick anyone
    h.click("Profile menu");
    h.click("Switch person");
    assert!(count(&h) > before);
    h.key(egui::Key::Escape);
    // on a server the person is who signed in
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-switch".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.click("Profile menu");
    assert!(
        h.has("Trash") && !h.has("Switch person"),
        "not in the menu on a team server"
    );
    h.key(egui::Key::Escape);
    // the dialog, if it ever opens there, still switches no one
    h.app.modal = Some(Modal::SwitchUser);
    h.settle();
    assert!(h.has(&format!("This computer is signed in to the team's server as {}. Each person uses Workboard on their own computer with their own sign-in.", my_name)));
    assert_eq!(count(&h), before, "no other member to switch to");
    h.click("Work together");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
    assert_eq!(h.app.data.me, me);
}

#[test]
fn the_first_start_shows_the_sign_in_screen_with_working_alone_as_a_choice() {
    use crate::app::login::LoginPage;
    assert!(
        Data::fresh().extra.welcome,
        "a first start shows the sign-in screen"
    );
    let old: Data = serde_json::from_str(
        &serde_json::to_string(&Data::seed())
            .unwrap()
            .replace(r#","welcome":false"#, ""),
    )
    .unwrap();
    assert!(!old.extra.welcome, "files from before don't");
    let mut h = Ui1::new();
    sign_in_screen(&mut h, "");
    // the screen fills the window, starting with the server's address:
    // none of the app shows, and it doesn't close
    assert_eq!(login_page(&h), Some(LoginPage::Server));
    assert!(
        h.has("Sign in to your team")
            && h.has("Server address entry")
            && h.has("Continue")
            && h.has("Set up a server for my team")
            && h.has(alone_button())
            && !h.has("Email entry")
    );
    assert!(!h.has("Workspace menu") && !h.has("Profile menu"));
    h.key(egui::Key::Escape);
    h.key_with(egui::Key::N, egui::Modifiers::COMMAND);
    assert!(h.app.login.is_some() && h.app.quick_add.is_none());

    // no address: it says so and stays
    h.click("Continue");
    assert_eq!(login_page(&h), Some(LoginPage::Server));
    assert!(!h.app.sync_rt.form.note.is_empty());
    // then email and password on that server
    h.click("Server address entry");
    h.type_text("127.0.0.1:1");
    h.click("Continue");
    assert_eq!(login_page(&h), Some(LoginPage::SignIn));
    assert!(h.app.sync_rt.form.note.is_empty());
    assert!(
        h.has("Sign in with email")
            && h.has("Change")
            && h.has("Email entry")
            && h.has("Password entry")
            && h.has("Forgot password?")
    );
    h.click("Email entry");
    h.type_text("me@team.io");
    h.click("Password entry");
    h.type_text("secret-pass");
    h.settle();
    h.click("Sign in");
    // first it asks the server which workspaces that account is in
    let j = h.app.sync_rt.listing.as_ref().expect("looking up");
    assert_eq!(j.addr, "127.0.0.1:1");
    assert_eq!(
        j.how,
        together::How::Account {
            email: "me@team.io".into(),
            password: "secret-pass".into()
        }
    );
    assert!(h.has("Looking up your workspaces..."));
    assert!(
        !h.has(alone_button()) && !h.has("Set up a server for my team"),
        "the other ways wait while it connects"
    );
    h.click("Cancel");
    assert!(h.app.sync_rt.listing.is_none() && h.app.login.is_some());

    // an invite code instead, choosing a password; the email carries over
    h.click("Join with an invite code");
    assert_eq!(login_page(&h), Some(LoginPage::Invite));
    assert!(h.has("Invite code entry") && h.has("Choose a password entry"));
    assert_eq!(h.app.sync_rt.form.mode, together::Mode::Invite);
    assert_eq!(h.app.sync_rt.form.email, "me@team.io");
    h.click("Sign in with email");
    assert_eq!(login_page(&h), Some(LoginPage::SignIn));

    // another server
    h.click("Change");
    assert_eq!(login_page(&h), Some(LoginPage::Server));
    h.click("Continue");
    assert_eq!(login_page(&h), Some(LoginPage::SignIn), "the address stays");

    // a new server: its address, the owner's email and password
    h.click("Set up a server for my team");
    assert_eq!(login_page(&h), Some(LoginPage::NewServer));
    assert!(
        h.has("Set up your team's server")
            && h.has("Server address entry")
            && h.has("Repeat password entry")
            && !h.has("Invite code entry")
    );
    assert_eq!(
        h.app.sync_rt.form.email, "me@team.io",
        "the email carries over"
    );
    h.click("Back to sign in");
    assert_eq!(login_page(&h), Some(LoginPage::SignIn));

    // alone on this Mac: just a name, then the app
    h.click(alone_button());
    assert!(h.has("What should we call you?") && h.has("Your name entry"));
    let me = h.app.data.me;
    h.app.login.as_mut().unwrap().name = "Jiwon".into();
    h.settle();
    h.click("Start");
    assert!(h.app.login.is_none());
    assert!(
        !h.app.data.extra.welcome,
        "no sign-in screen at the next start"
    );
    assert_eq!(h.app.data.member(me).unwrap().name, "Jiwon");
    assert!(h.has("Workspace menu"));
}

#[test]
fn the_sign_in_screen_starts_at_the_email_when_the_server_is_known() {
    let mut h = Ui1::new();
    sign_in_screen(&mut h, "wb.example.com");
    assert_eq!(login_page(&h), Some(crate::app::login::LoginPage::SignIn));
    assert!(h.has("Email entry") && h.has("Change"));
}

#[test]
fn an_invite_link_opens_the_join_form_filled_in() {
    let mut h = Ui1::new();
    // working on this computer only: the team's workspace opens beside it
    h.app
        .open_join_link("workboard://join?server=macmini&code=ABCD-EFGH-JKMN");
    assert!(matches!(&h.app.modal, Some(Modal::JoinTeam { addr }) if addr == "macmini"));
    h.app.modal = None;
    // on the sign-in screen (a first start): its invite step, filled in
    sign_in_screen(&mut h, "");
    h.app.open_join_link("workboard://join?server=mac-mini.tail1.ts.net%3A8787&code=ABCD-EFGH-JKMN&email=bo%40team.io");
    h.settle();
    assert!(
        matches!(&h.app.login, Some(l) if l.addr == "mac-mini.tail1.ts.net:8787" && l.page == crate::app::login::LoginPage::Invite)
    );
    assert!(h.app.modal.is_none());
    assert_eq!(h.app.sync_rt.form.mode, together::Mode::Invite);
    assert_eq!(
        (
            h.app.sync_rt.form.code.as_str(),
            h.app.sync_rt.form.email.as_str()
        ),
        ("ABCD-EFGH-JKMN", "bo@team.io")
    );
    assert!(h.has("Invite code entry") && h.has("Choose a password entry"));

    // a whole invite mail pasted into the code field fills in the server too
    h.app.login = Some(crate::app::login::LoginScreen::new(
        crate::app::login::LoginPage::Invite,
        "",
    ));
    h.app.sync_rt.form = together::Form {
        mode: together::Mode::Invite,
        ..Default::default()
    };
    h.settle();
    h.click("Invite code entry");
    h.type_text("Open this: workboard://join?server=macmini&code=WXYZ-2345-6789 thanks");
    h.settle();
    h.settle();
    assert_eq!(h.app.sync_rt.form.code, "WXYZ-2345-6789");
    assert!(matches!(&h.app.login, Some(l) if l.addr == "macmini"));
    h.app.login = None;

    // already on a server: the other team opens beside it, this one stays
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.toast = None;
    h.app
        .open_join_link("workboard://join?server=other&code=ABCD-EFGH-JKMN");
    assert!(matches!(&h.app.modal, Some(Modal::JoinTeam { addr }) if addr == "other"));
    assert_eq!(h.app.sync_rt.form.code, "ABCD-EFGH-JKMN");
    assert!(h.app.toast.is_none(), "no telling them to stop sharing");

    // this workspace's server let it go: the code signs it back in there
    h.app.sync_rt.net_status = Some(wb_net::Status::Refused("removed".into()));
    h.app
        .open_join_link("workboard://join?server=127.0.0.1:1&code=ABCD-EFGH-JKMN");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
}

#[test]
fn joining_a_server_that_does_not_answer_offers_to_update_the_app() {
    let mut h = Ui1::new();
    let addr = "https://wb.example.com/w/ab12";
    h.app.login = Some(crate::app::login::LoginScreen::new(
        crate::app::login::LoginPage::Invite,
        addr,
    ));
    h.settle();
    assert!(!h.has("Update Workboard"), "nothing to fix before joining");
    // what an app older than the server sees with a workspace's invite link
    h.app.sync_rt.joining = Some(together::Joining {
        addr: addr.into(),
        device: "dev1".into(),
        how: together::How::Invite {
            code: "ABCD-EFGH-JKMN".into(),
            email: String::new(),
            password: "a long password".into(),
        },
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Reconnecting(
        "The server didn't answer like Workboard".into(),
    ));
    h.settle();
    assert!(h.has("Something answered at this address, but not a Workboard server. Check the address and port. Trying again every few seconds."));
    assert!(h.has("An older Workboard may not work with this server."));
    h.click("Update Workboard");
    h.settle();
    if crate::updater::own::enabled() {
        // Windows: the app's own updater says how it goes under the button
        assert!(h.app.sync_rt.form.app_updates);
        assert!(h.app.sync_rt.form.update_note.is_empty());
    } else {
        // tests don't run inside Workboard.app, so it says why it can't update
        let note = h.app.sync_rt.form.update_note.clone();
        assert!(note.contains(env!("CARGO_PKG_VERSION")), "{}", note);
        assert!(h.has(&note), "shown under the button");
    }
    assert!(h.app.sync_rt.joining.is_some(), "still trying to join");
}

#[test]
fn log_out_asks_first_then_deletes_the_team_copy_and_shows_the_sign_in_screen() {
    let mut h = Ui1::new();
    // working on this computer only: there is nothing to log out of
    h.click("Profile menu");
    assert!(h.has("Switch person") && !h.has("Log out"));
    h.key(egui::Key::Escape);

    let addr = "127.0.0.1:1";
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev-out".into(),
        tag: 5,
        server: Some(addr.into()),
    });
    together::save_token(addr, "dev-out", "token-1");
    h.app.sync_rt.account = Some("me@team.io".into());
    let team_task = h.app.data.tasks[0].name.clone();
    h.click("Profile menu");
    h.click("Log out");
    assert!(matches!(h.app.modal, Some(Modal::LogOut)));
    assert!(h.has("Log out of 127.0.0.1:1?"));
    // not connected: it says what would be lost
    assert!(h.has("This computer isn't connected to the server right now. Changes you made while it was offline haven't gone up and will be lost."));
    h.click("Cancel");
    assert!(h.app.modal.is_none() && h.app.data.extra.shared.is_some());

    h.click("Profile menu");
    h.click("Log out");
    h.click("Log out");
    assert!(h.app.data.extra.shared.is_none());
    assert!(
        !h.app.data.tasks.iter().any(|t| t.name == team_task),
        "this computer's copy of the team workspace is gone"
    );
    assert!(together::load_token(addr, "dev-out").is_none());
    assert!(
        h.app.data.extra.welcome,
        "the sign-in screen again at the next start"
    );
    assert!(matches!(&h.app.login, Some(l) if l.addr == addr));
    assert_eq!(
        h.app.sync_rt.form.email, "me@team.io",
        "ready to sign in again"
    );
    assert!(h.has("Sign in with email") && h.has("Email entry") && !h.has("Profile menu"));
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Logged out")
    );
}

#[test]
fn signed_in_on_a_server_you_set_a_password_or_change_it() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert!(h.has("Your account") && h.has("Set a password to sign in on your other computers and phones without a new invite code."));
    h.click("Password entry");
    h.type_text("long-enough");
    h.click("Repeat password entry");
    h.type_text("long-enougH");
    h.settle();
    h.click("Save password");
    assert!(h.has("The two passwords aren't the same."));
    // with an account: change it (needs the current one)
    h.app.sync_rt.account = Some("me@team.io".into());
    h.app.sync_rt.acct = together::Form::default();
    h.settle();
    assert!(h.has("You sign in with me@team.io on your other computers and phones."));
    h.click("Change password");
    assert!(h.has("Current password entry") && h.has("New password entry"));
}

#[test]
fn password_boxes_show_on_the_eye_and_take_english_only() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    // every password box has its own eye
    assert_eq!(
        h.tree.iter().filter(|(n, _)| n == "Show password").count(),
        2
    );
    h.click("Password entry");
    h.type_text("ab");
    // Korean typing: neither the word being put together nor the result
    // goes in, and a note says to switch
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Preedit("ㅎ".into()))]);
    h.frame(vec![egui::Event::Ime(egui::ImeEvent::Commit("한".into()))]);
    h.type_text("가c1!");
    h.frame(vec![egui::Event::Paste("d 한e".into())]);
    h.settle();
    assert_eq!(h.app.sync_rt.acct.password, "abc1!d e");
    assert!(h.has("Passwords take English letters, numbers and symbols. Switch to English input."));
    // English again: the note goes
    h.type_text("x");
    h.settle();
    assert!(!h.has("Passwords take English letters, numbers and symbols. Switch to English input."));
    // the eye beside this box shows the text and hides it again, typing
    // carries on
    let entry = h
        .tree
        .iter()
        .find(|(n, _)| n == "Password entry")
        .unwrap()
        .1;
    let eye = h
        .tree
        .iter()
        .find(|(n, r)| n == "Show password" && entry.y_range().contains(r.center().y))
        .expect("the eye sits inside its box")
        .1;
    h.click_at(eye.center());
    assert!(h.has("Hide password") && !h.painted_text("abc1!d ex").is_empty());
    h.type_text("y");
    h.settle();
    assert_eq!(h.app.sync_rt.acct.password, "abc1!d exy");
    h.click("Hide password");
    assert!(h.painted_text("abc1!d exy").is_empty());
    assert_eq!(
        h.tree.iter().filter(|(n, _)| n == "Show password").count(),
        2
    );
}

#[test]
fn the_ipad_keyboard_leaves_the_name_and_start_in_view() {
    // an iPad mini on its side; its keyboard covers the bottom 390 points
    let mut h = Ui1::sized(1133.0, 744.0);
    sign_in_screen(&mut h, "");
    h.click(alone_button());
    let above = 744.0 - 390.0;
    assert!(h.rect("Start").bottom() > above, "under the keyboard-to-be");
    h.click("Your name entry");
    crate::os::fake_keyboard(390.0);
    for _ in 0..10 {
        h.frame(vec![]);
    }
    let name = h.rect("Your name entry");
    let start = h.rect("Start");
    crate::os::fake_keyboard(0.0);
    assert!(
        name.bottom() <= above,
        "the name shows over the keyboard: {name:?}"
    );
    assert!(start.bottom() <= above, "so does Start: {start:?}");
    h.frame(vec![]);
}

#[test]
fn the_ipad_keyboard_lifts_a_dialog_and_its_button_still_works() {
    let mut h = Ui1::sized(1133.0, 744.0);
    let before = h.app.data.tasks.len();
    h.click("Add Task");
    h.type_text("Over the keyboard");
    crate::os::fake_keyboard(390.0);
    h.settle();
    // the button, right of the heading with the same words
    let button = h
        .tree
        .iter()
        .filter(|(n, _)| n == "Create Task")
        .map(|(_, r)| *r)
        .max_by(|a, b| a.left().total_cmp(&b.left()))
        .unwrap();
    let ok = button.bottom() <= 744.0 - 390.0;
    if ok {
        h.click_side("Create Task", true);
    }
    crate::os::fake_keyboard(0.0);
    assert!(ok, "the button shows over the keyboard: {button:?}");
    assert_eq!(h.app.data.tasks.len(), before + 1);
    assert!(h
        .app
        .data
        .tasks
        .iter()
        .any(|t| t.name == "Over the keyboard"));
}

#[test]
fn the_ipad_keyboard_leaves_the_message_box_in_view() {
    let mut h = Ui1::sized(1133.0, 744.0);
    h.click("general");
    h.click("Message composer");
    crate::os::fake_keyboard(390.0);
    h.settle();
    let above = 744.0 - 390.0;
    // the channel's list on the right is taller than what is left: it
    // scrolls instead of pushing the box down
    let send = h.rect("Send");
    let box_rect = h.rect("Message composer");
    crate::os::fake_keyboard(0.0);
    assert!(send.bottom() <= above, "Send over the keyboard: {send:?}");
    assert!(box_rect.bottom() <= above, "{box_rect:?}");
    h.type_text("over the keyboard");
    h.click("Send");
    assert!(h.has("over the keyboard"));
}
