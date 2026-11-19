// Click-through tests for people and teams: inviting, roles, working
// together on a server or shared folder, presence and sharing spaces.

use super::*;

fn sharing_of(h: &Ui1, sid: u64) -> Option<(bool, Vec<(u64, String)>)> {
    let s = h.app.data.spaces.iter().find(|s| s.id == sid)?;
    s.sharing.as_ref().map(|x| {
        (
            x.everyone,
            x.people
                .iter()
                .map(|p| (p.member, p.level.clone()))
                .collect(),
        )
    })
}

#[test]
fn click_invite_in_rail_and_send() {
    let mut h = Ui1::new();
    let n = h.app.data.members.len();
    h.click("Rail Invite");
    assert!(matches!(h.app.modal, Some(Modal::Invite { .. })));
    h.type_text("lee.jiwoo@team.kr");
    h.click("Send invite");
    assert_eq!(h.app.data.members.len(), n + 1);
    assert_eq!(
        h.app.data.members.last().unwrap().email,
        "lee.jiwoo@team.kr"
    );
    assert!(h.app.modal.is_none(), "the dialog closes after sending");
}

#[test]
fn a_member_on_a_server_sees_no_invite_or_workspace_settings() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.settle();
    assert!(!h.has("Rail Invite"), "no Invite in the rail");
    // People lists everyone, but without Invite people
    h.app.open_settings("People");
    h.settle();
    assert!(h.has("Settings People") && !h.has("Invite people"));
    // no General (workspace name and colour); asking for it opens my settings
    assert!(!h.has("Settings General"));
    h.app.open_settings("General");
    h.settle();
    assert!(h.has("Full name") && !h.has("Workspace name"));
    // nor what changes the workspace for everyone; Templates stays, to use them
    for s in ["Apps", "Custom fields", "Tag manager", "Automations"] {
        assert!(!h.has(&format!("Settings {}", s)), "no {}", s);
    }
    h.click("Settings Templates");
    assert!(!h.has("New task template"));
    // reached anyway, those dialogs only say who can change them
    h.app.section = Section::Home;
    h.app.modal = Some(Modal::ClickApps);
    h.settle();
    assert!(h.has("Only the owner or an admin of this workspace can change this."));
    assert!(!h.has("Priorities"));
    h.click("Close");
    // teams can be looked at, not made
    h.app.data.extra.teams.push(Team {
        id: 777,
        name: "Design".into(),
        members: vec![h.app.data.me],
        color: [1, 2, 3],
    });
    h.app.section = Section::Teams;
    h.settle();
    assert!(!h.has("Create team"));
    h.app.team_open = Some(777);
    h.app.hub_filter = 9;
    h.settle();
    assert!(!h.has("Delete team"));
    h.app.section = Section::Home;
    // Share keeps Copy link but has no invite field
    h.app.modal = Some(Modal::Share {
        emails: String::new(),
    });
    h.settle();
    assert!(h.has("Copy link") && !h.has("Invite by email"));
    // Import and export keeps CSV but not Import from Slack (it brings people in)
    h.app.modal = Some(Modal::Import {
        path: String::new(),
        result: String::new(),
    });
    h.settle();
    assert!(h.has("Export current tasks to CSV") && !h.has("Import from Slack"));
    h.app
        .open_slack_export(std::path::Path::new("/nonexistent.zip"));
    assert!(matches!(h.app.modal, Some(Modal::Import { .. })));
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Only the owner or an admin of this workspace can import from Slack.")
    );
    // the invite dialog reached anyway (a link) only says who can invite
    h.app.modal = Some(Modal::Invite {
        emails: String::new(),
        role: "Member".into(),
    });
    h.settle();
    assert!(h.has("Only the owner or an admin of this workspace can invite people."));
    assert!(!h.has("Send invite"));
    let n = h.app.data.members.len();
    h.click("Close");
    assert!(h.app.modal.is_none() && h.app.data.members.len() == n);
    // an admin gets all of it back
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.settle();
    assert!(h.has("Rail Invite"));
    h.click("Rail Invite");
    assert!(h.has("Send invite"));
    h.key(egui::Key::Escape);
    h.app.open_settings("General");
    h.settle();
    assert!(h.has("Settings General") && h.has("Workspace name"));
    assert!(h.has("Settings Automations") && h.has("Settings Custom fields"));
    h.app.section = Section::Teams;
    h.settle();
    assert!(h.has("Delete team"));
    // before the server is reached, the role in People decides
    h.app.section = Section::Home;
    h.app.sync_rt.role = None;
    let me = h.app.data.me;
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == me)
        .unwrap()
        .role = "Member".into();
    h.settle();
    assert!(!h.has("Rail Invite"));
    h.app
        .data
        .members
        .iter_mut()
        .find(|m| m.id == me)
        .unwrap()
        .role = "Owner".into();
    h.settle();
    assert!(h.has("Rail Invite"));
}

#[test]
fn green_dots_show_who_is_really_connected() {
    use crate::widgets::ONLINE;
    let mut h = Ui1::new();
    let me = h.app.data.me;
    // a DM with someone else, open in the sidebar
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone();
    let has_dm = h
        .app
        .data
        .channels
        .iter()
        .any(|c| c.kind == crate::model::ChannelKind::Direct && c.members.contains(&other.id));
    if !has_dm {
        let mut c = h.app.data.channels[0].clone();
        c.id = 987_654;
        c.name = String::new();
        c.kind = crate::model::ChannelKind::Direct;
        c.members = vec![me, other.id];
        c.messages.clear();
        h.app.data.channels.push(c);
    }
    h.app.section = Section::Chat;
    h.settle();
    let row = other.name.clone();
    assert!(h.has(&row), "the DM is listed");
    // alone: no "online" at all, so no dots
    assert_eq!(dots_in(&h, "Profile menu", ONLINE), 0);
    assert_eq!(dots_in(&h, &row, ONLINE) + rings_in(&h, &row), 0);
    h.click("Profile menu");
    assert!(!h.has("Online"), "no claim of being online");
    h.key(egui::Key::Escape);

    // on a server, connected; the server says only I am online
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.online = Some(vec![me]);
    h.settle();
    assert_eq!(dots_in(&h, "Profile menu", ONLINE), 1, "I am connected");
    assert_eq!(dots_in(&h, &row, ONLINE), 0, "they aren't");
    assert_eq!(rings_in(&h, &row), 1, "a grey ring says so");
    h.click("Profile menu");
    assert!(h.has("Online"));
    h.key(egui::Key::Escape);

    // they open Workboard
    h.app.sync_rt.online = Some(vec![me, other.id]);
    h.settle();
    assert_eq!(dots_in(&h, &row, ONLINE), 1);
    assert_eq!(rings_in(&h, &row), 0);

    // the connection drops: who is online isn't known, and I am offline
    h.app.sync_rt.net_status = Some(wb_net::Status::Reconnecting("gone".into()));
    h.settle();
    assert_eq!(dots_in(&h, "Profile menu", ONLINE), 0);
    assert_eq!(rings_in(&h, "Profile menu"), 1);
    assert_eq!(dots_in(&h, &row, ONLINE) + rings_in(&h, &row), 0);
    h.click("Profile menu");
    assert!(h.has("Offline") && !h.has("Online"));
    h.key(egui::Key::Escape);
}

#[test]
fn click_work_together_opens_sharing() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Work together");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
}

#[test]
fn work_together_in_a_shared_folder_says_how_teammates_join_and_keeps_the_path_on_one_line() {
    let mut h = Ui1::new();
    // no sync folder found on this computer: nothing "above" to pick
    h.click("Workspace menu");
    h.click("Work together");
    let hint = |h: &Ui1, s: &str| !h.painted_text(s).is_empty();
    if crate::sync::suggested_folders().is_empty() {
        assert!(hint(&h, "Paste a folder path"));
        assert!(!hint(&h, "Paste a folder path, or pick one above"));
    } else {
        assert!(h.has("Found on this computer"));
        assert!(hint(&h, "Paste a folder path, or pick one above"));
    }
    h.key(egui::Key::Escape);
    let dir = temp_home("ui")
        .join("audit-states-shared-folder")
        .join("team-workspace-folder")
        .join("_shared");
    std::fs::create_dir_all(&dir).unwrap();
    let full = dir.to_str().unwrap().to_string();
    h.app.share_here(&full).unwrap();
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    // the long path is cut from the front at folder names, on one line
    let short = crate::app::together::short_path(&full, 48);
    let sep = std::path::MAIN_SEPARATOR;
    assert!(
        short.starts_with('\u{2026}')
            && short.ends_with(&format!("{sep}team-workspace-folder{sep}_shared")),
        "{}",
        short
    );
    assert!(h.has(&short), "{}", short);
    let at = h.painted_text(&short);
    assert_eq!(at.len(), 1, "one line: {:?}", at);
    assert!(at[0].0.height() < 24.0, "{:?}", at);
    // counts computers, and says what teammates do on theirs
    assert!(h.has("No other computer is working in this folder yet. Share the folder with your teammates in your sync app. On their computer they open Work together, paste the folder's path and choose Join this workspace."));
    assert!(h.has("Tasks, chat and docs show up for everyone a few seconds after their sync app copies the folder. Calls open in the browser from a link posted in chat."));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap().parent().unwrap());
}

#[test]
fn work_together_fits_a_small_window_and_scrolls() {
    let mut h = Ui1::sized(900.0, 560.0);
    h.click("Workspace menu");
    h.click("Work together");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
    // the title and the close button stay on screen
    let close = h.rect("Close");
    assert!(close.top() >= 0.0 && close.bottom() <= 560.0, "{:?}", close);
    // the bottom of the dialog is reached by scrolling inside it
    h.scroll_into_view("Connect");
    let connect = h.rect("Connect");
    assert!(connect.bottom() <= 560.0, "{:?}", connect);
    assert!(h.rect("Close").top() >= 0.0, "the title stays put");
}

#[test]
fn a_workspace_on_a_server_can_still_join_another_team() {
    // this workspace is already on a team server (its own team)
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.settle();
    h.click("Workspace menu");
    h.click("Join a team");
    assert!(matches!(&h.app.modal, Some(Modal::JoinTeam { .. })));
    assert!(h.has("Join a team") && h.has("Server address") && h.has("Email entry"));
    assert!(h.has("The team's workspace opens as a new workspace. The ones you already have stay in the workspace menu, as they are."));
    assert!(!h.has("Back"), "no first-start choices to go back to");
    h.click("Server address");
    h.type_text("127.0.0.1:2");
    h.click("Invite code");
    h.click("Invite code entry");
    h.type_text("abcd-efgh-jkmn");
    h.click("Choose a password entry");
    h.type_text("pw-12345678");
    h.click("Repeat password entry");
    h.type_text("pw-12345678");
    h.settle();
    h.click("Connect");
    let j = h.app.sync_rt.joining.as_ref().expect("connecting");
    assert_eq!(j.addr, "127.0.0.1:2");
    assert!(matches!(&j.how, together::How::Invite { code, .. } if code == "abcd-efgh-jkmn"));
    assert_eq!(
        h.app
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.as_deref()),
        Some("127.0.0.1:1"),
        "this workspace stays on its server while joining"
    );
    // giving up goes back to this workspace's own server
    h.click("Cancel");
    assert!(h.app.sync_rt.joining.is_none());
    assert_eq!(
        h.app
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.as_deref()),
        Some("127.0.0.1:1")
    );
}

#[test]
fn work_together_connects_to_a_server_and_shows_the_status() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Work together");
    assert!(h.has("Or connect to your team's server"));
    h.click("Server address");
    h.type_text("127.0.0.1:1");
    h.settle();
    // email and password first; an invite code on its own tab
    assert!(h.has("Email entry") && h.has("Password entry") && h.has("Forgot password?"));
    h.click("Invite code");
    h.click("Invite code entry");
    h.type_text("abcd-efgh-jkmn");
    h.click("Choose a password entry");
    h.type_text("pw-123");
    h.click("Repeat password entry");
    h.type_text("pw-123");
    h.settle();
    h.click("Connect");
    assert!(h.app.sync_rt.joining.is_none(), "too short");
    assert!(h.has("Choose a password of at least 8 characters."));
    h.click("Choose a password entry");
    h.type_text("45678");
    h.click("Repeat password entry");
    h.type_text("45678");
    h.settle();
    h.click("Connect");
    let j = h.app.sync_rt.joining.as_ref().expect("connecting");
    assert_eq!(j.addr, "127.0.0.1:1");
    assert_eq!(
        j.how,
        together::How::Invite {
            code: "abcd-efgh-jkmn".into(),
            email: String::new(),
            password: "pw-12345678".into()
        }
    );
    assert!(h.has("Cancel"));
    h.click("Cancel");
    assert!(h.app.sync_rt.joining.is_none());
    assert!(h.has("Connect"));

    // a workspace on a server shows where and how the connection is doing
    let tag = 5;
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert!(h.has("This workspace is on a server"));
    assert!(h.has("127.0.0.1:1"));
    assert!(h.has("Status: Disconnected"));
    // nothing listens on port 1, so the app keeps trying
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !matches!(
        h.app.sync_rt.net_status,
        Some(wb_net::Status::Reconnecting(_))
    ) && std::time::Instant::now() < until
    {
        h.app.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    h.settle();
    assert!(
        h.has("Status: Reconnecting\u{2026}"),
        "shows that it is reconnecting"
    );
    // why, in plain words, without the system's error text
    assert!(
        h.has("Nothing answers at 127.0.0.1:1. Check that workboard-server is running there. Trying again every few seconds."),
        "says why"
    );
    assert!(!h.tree.iter().any(|(n, _)| n.contains("os error")));
    let status = h.painted_text("Status: Reconnecting\u{2026}");
    assert!(
        !status.is_empty() && status.iter().all(|(_, c)| *c == crate::app::together::WARN),
        "in the warning colour: {:?}",
        status
    );
    // still failing after a few tries: it says it can't reach the server
    h.app.sync_rt.fails = crate::app::together::FAILS_BEFORE_CANT_REACH;
    h.settle();
    assert!(h.has("Status: Can't reach the server"));
    // owners and admins see everyone's access
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    let me = h.app.data.me;
    let other = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.id != me)
        .unwrap()
        .clone();
    h.app.sync_rt.people = vec![
        wb_net::Person {
            member: me,
            role: wb_net::Role::Owner,
            devices: 1,
            invite: None,
            account: true,
        },
        wb_net::Person {
            member: other.id,
            role: wb_net::Role::Member,
            devices: 0,
            invite: Some("ABCD-EFGH-JKMN".into()),
            account: false,
        },
    ];
    h.settle();
    let my_name = h.app.data.member(me).unwrap().name.clone();
    assert!(h.has(&format!("Signed in as {} (Owner)", my_name)));
    assert!(h.has("People"));
    assert!(h.has("Make admin") && h.has("Remove access") && h.has("New code"));
    // row buttons leave room around the text, all the same height
    let heights: Vec<f32> = ["Make admin", "Remove access", "New code"]
        .iter()
        .map(|n| h.tree.iter().find(|(t, _)| t == n).unwrap().1.height())
        .collect();
    assert!(
        heights
            .iter()
            .all(|&ht| ht >= 26.0 && (ht - heights[0]).abs() < 0.5),
        "row button heights {:?}",
        heights
    );
    assert!(h.has(&format!("Code for {}: ABCD-EFGH-JKMN", other.name)));
    h.click("Copy");
    // the owner can go back to a backup, with a second click to confirm
    assert!(h.has("Backups") && h.has("Show backups"));
    h.app.sync_rt.backups = Some(vec![
        wb_net::Backup {
            name: "hourly-20261115-03.json".into(),
            bytes: 2048,
        },
        wb_net::Backup {
            name: "daily-20261115.json".into(),
            bytes: 2048,
        },
    ]);
    h.settle();
    assert!(h.has("2026-11-15 03:00 (hourly)") && h.has("2026-11-15 (daily)"));
    h.click("Restore");
    assert!(h.app.sync_rt.restore_confirm.is_some(), "asks first");
    assert!(h.has("Yes, restore"));
    h.click("Cancel");
    assert!(h.app.sync_rt.restore_confirm.is_none() && !h.has("Yes, restore"));
    // an admin can't make a code for the owner
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.app.sync_rt.people[0].member = other.id;
    h.app.sync_rt.people[1] = wb_net::Person {
        member: me,
        role: wb_net::Role::Admin,
        devices: 1,
        invite: None,
        account: false,
    };
    h.settle();
    assert!(
        !h.has("Invite code") && !h.has("New code"),
        "no code button on the owner's row for an admin"
    );
    // a member sees no People list
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.settle();
    assert!(!h.has("People") && !h.has("Make admin"));
    // access removed: sign in again with email and password, or a new code
    h.app.sync_rt.form = together::Form::default();
    h.app.sync_rt.net_status = Some(wb_net::Status::Refused(
        "Your access to this server was removed.".into(),
    ));
    h.settle();
    assert!(h.has("Sign in"));
    h.click("Email entry");
    h.type_text("me@team.io");
    h.click("Password entry");
    h.type_text("secret-pass");
    h.settle();
    h.click("Sign in");
    assert_eq!(
        h.app.sync_rt.resign,
        Some(together::How::Account {
            email: "me@team.io".into(),
            password: "secret-pass".into()
        })
    );
    h.app.sync_rt.net_status = Some(wb_net::Status::Refused(
        "That email and password don't match an account on this server.".into(),
    ));
    h.settle();
    h.click("Invite code");
    h.click("Invite code entry");
    h.type_text("NEWC-ODEX-XXXX");
    assert!(
        !h.painted_text("the same password again").is_empty(),
        "the repeat box says what goes in it"
    );
    h.click("Choose a password entry");
    h.type_text("new-secret-1");
    h.click("Repeat password entry");
    h.type_text("new-secret-1");
    h.settle();
    h.click("Sign in");
    assert!(
        matches!(&h.app.sync_rt.resign, Some(together::How::Invite { code, .. }) if code == "NEWC-ODEX-XXXX")
    );
    h.click("Reconnect now");
    assert!(h.app.sync_rt.net.is_none(), "starts over on the next frame");
    h.click("Stop sharing");
    assert!(h.app.data.extra.shared.is_none());
}

#[test]
fn the_top_bar_says_when_the_team_server_cannot_be_reached() {
    let mut h = Ui1::new();
    // working alone: nothing about a server
    assert!(!h.has("Server connection"));
    server_down(&mut h);
    assert!(h.has("Server connection"), "a word beside the profile menu");
    assert!(h
        .painted_text("Reconnecting\u{2026}")
        .iter()
        .any(|(r, _)| r.top() < 40.0));
    h.app.sync_rt.fails = crate::app::together::FAILS_BEFORE_CANT_REACH;
    h.settle();
    assert!(h
        .painted_text("Not connected")
        .iter()
        .any(|(r, _)| r.top() < 40.0));
    // it sits left of the profile menu, inside the top bar
    let word = h.rect("Server connection");
    let profile = h.rect("Profile menu");
    assert!(word.right() <= profile.left() + 1.0 && word.bottom() <= 40.0);
    h.click("Server connection");
    assert!(matches!(h.app.modal, Some(Modal::Together { .. })));
    assert!(h.has("This workspace is on a server"));
    h.key(egui::Key::Escape);
    // signed out by the server: says so too
    h.app.sync_rt.net_status = Some(wb_net::Status::Refused(
        "Your access to this server was removed.".into(),
    ));
    h.settle();
    assert!(h
        .painted_text("Not signed in")
        .iter()
        .any(|(r, _)| r.top() < 40.0));
    // connected: out of the way
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.settle();
    assert!(!h.has("Server connection"));
}

#[test]
fn work_together_on_a_server_that_is_down_leads_with_working_offline_and_can_change_the_address() {
    let mut h = Ui1::new();
    crate::app::together::save_token("127.0.0.1:1", "dev-down", "tok-down");
    server_down(&mut h);
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    // not "everyone sees it in a second or two" while nobody can
    assert!(!h.has("Tasks, chat and docs show up for everyone within a second or two."));
    assert!(h.has("You can keep working here. Your changes are kept on this computer and go up when the server is back."));
    // the server moved: type its new address and connect there
    h.click("Change address");
    h.click("Server address");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("127.0.0.1:2");
    h.click("Save and reconnect");
    let cfg = h.app.data.extra.shared.clone().unwrap();
    assert_eq!(cfg.server.as_deref(), Some("127.0.0.1:2"));
    assert!(
        h.app.sync_rt.net.is_none(),
        "connects again on the next frame"
    );
    assert_eq!(
        crate::app::together::load_token("127.0.0.1:2", "dev-down").as_deref(),
        Some("tok-down"),
        "the sign-in goes along"
    );
    assert!(crate::app::together::load_token("127.0.0.1:1", "dev-down").is_none());
    assert!(h.has("127.0.0.1:2") && !h.has("Save and reconnect"));
    // a wrong address says so and changes nothing
    h.click("Change address");
    h.click("Server address");
    h.key_with(egui::Key::A, egui::Modifiers::COMMAND);
    h.type_text("https://team.example:web");
    h.click("Save and reconnect");
    assert_eq!(
        h.app.data.extra.shared.as_ref().unwrap().server.as_deref(),
        Some("127.0.0.1:2")
    );
    assert!(h.has("That port isn't a number."));
    h.click("Cancel");
    assert!(!h.has("Save and reconnect"));
    // connected: the usual words, and no address to change
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.settle();
    assert!(h.has("Tasks, chat and docs show up for everyone within a second or two."));
    assert!(!h.has("Change address"));
}

#[test]
fn people_says_why_roles_are_missing_offline_and_how_invited_people_join() {
    let mut h = Ui1::new();
    h.app.section = Section::People;
    h.settle();
    // nobody invited: no note about invited people
    let invited_note = |h: &Ui1| h.tree.iter().any(|(n, _)| n.starts_with("Invited people "));
    assert!(!invited_note(&h));
    h.app
        .invite_people(&["dana@example.com".to_string()], "Member");
    h.settle();
    assert!(h.has("Invited people can join once you share this workspace (Work together). Until then they can use this computer through Switch person in the profile menu."));
    assert!(h.has("Remove") && !h.has("Changing roles and removing people need the team's server. They come back once it's connected."));
    // on a server that can't be reached yet: the owner's controls are gone, and it says why
    server_down(&mut h);
    h.app.section = Section::People;
    h.settle();
    assert!(!h.has("Remove"));
    assert!(h.has("Changing roles and removing people need the team's server. They come back once it's connected."));
    assert!(h.has("Invited people sign in with the invite code the server emails them. Owners and admins also find the codes in Work together > People while connected."));
    // signed in as the owner: the controls are back, the note goes
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.settle();
    assert!(h.has("Remove"));
    assert!(!h.has("Changing roles and removing people need the team's server. They come back once it's connected."));
    // a shared folder: invited people join from the folder
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: "/tmp/wb-none".into(),
        device: "dev-f".into(),
        tag: 1,
        server: None,
    });
    h.settle();
    assert!(h.has("Invited people join from the shared folder: on their computer they open Work together, paste the folder's path, choose Join this workspace and pick their name."));
}

#[test]
fn an_invited_person_has_cancel_invite_instead_of_remove() {
    let mut h = Ui1::new();
    h.app.section = Section::People;
    h.settle();
    let before = h.app.data.members.len();
    h.app
        .invite_people(&["dana@example.com".to_string()], "Member");
    h.settle();
    let dana = h.app.data.members.iter().find(|m| m.invited).unwrap().id;
    assert!(h.has("Cancel invite"));
    h.click("Cancel invite");
    assert!(h.has(
        "Cancel the invite for Dana? Their invite code stops working and they leave the list."
    ));
    // the dialog's button (its title says the same, above it) takes them out
    let m = h.modal_rect();
    let at = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Cancel invite" && m.contains(r.center()))
        .map(|(_, r)| r.center())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    h.click_at(at);
    assert!(h.app.modal.is_none());
    assert!(h.app.data.members.iter().all(|m| m.id != dana));
    assert_eq!(h.app.data.members.len(), before);
    assert!(!h.has("Cancel invite"));
    // people who joined keep the Remove button
    assert!(h.has("Remove"));
}

#[test]
fn an_admin_has_no_controls_for_another_admin_but_the_owner_does() {
    let mut h = Ui1::new();
    server_down(&mut h);
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    let me = h.app.data.me;
    let others: Vec<u64> = h
        .app
        .data
        .members
        .iter()
        .filter(|m| m.id != me && m.role != "Owner")
        .map(|m| m.id)
        .take(2)
        .collect();
    for (id, role) in [(me, "Admin"), (others[0], "Admin"), (others[1], "Member")] {
        h.app
            .data
            .members
            .iter_mut()
            .find(|m| m.id == id)
            .unwrap()
            .role = role.into();
    }
    let count = |h: &Ui1, name: &str| h.tree.iter().filter(|(n, _)| n == name).count();
    // People page: the owner may remove both, an admin only the member
    h.app.section = Section::People;
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.settle();
    let owner_removes = count(&h, "Remove");
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.settle();
    assert_eq!(count(&h, "Remove"), owner_removes - 1);
    // Work together > People: no Make member or Remove access on the other admin's row
    let person = |member, role| wb_net::Person {
        member,
        role,
        devices: 1,
        invite: None,
        account: true,
    };
    h.app.sync_rt.people = vec![
        person(me, wb_net::Role::Admin),
        person(others[0], wb_net::Role::Admin),
        person(others[1], wb_net::Role::Member),
    ];
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert_eq!(count(&h, "Make member"), 0);
    assert_eq!(
        (count(&h, "Make admin"), count(&h, "Remove access")),
        (1, 1)
    );
    // the member's row still works
    h.click("Make admin");
    assert!(h.app.sync_rt.other_asked.is_some(), "the request went out");
    // the owner has them for the admin too
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.app.sync_rt.people[0].role = wb_net::Role::Owner;
    h.settle();
    assert_eq!(count(&h, "Make member"), 1);
    assert_eq!(count(&h, "Remove access"), 2);
}

#[test]
fn a_member_on_the_server_is_told_who_manages_people() {
    let mut h = Ui1::new();
    h.app
        .invite_people(&["dana@example.com".to_string()], "Member");
    server_down(&mut h);
    h.app.section = Section::People;
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.settle();
    assert!(!h.has("Remove") && !h.has("Cancel invite"));
    assert!(h.has("Only the owner or an admin can change roles, cancel invites and remove people."));
    // the owner sees the buttons and no such note
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.settle();
    assert!(h.has("Remove") && h.has("Cancel invite"));
    assert!(
        !h.has("Only the owner or an admin can change roles, cancel invites and remove people.")
    );
}

#[test]
fn inviting_while_the_server_is_down_says_the_codes_wait_and_the_dialog_stays_narrow() {
    let window = |h: &Ui1| h.modal_rect();
    let bookmark = || Modal::Bookmark {
        title: String::new(),
        url: String::new(),
    };
    // a small dialog's width when nothing wide was open before
    let mut fresh = Ui1::new();
    fresh.app.modal = Some(bookmark());
    fresh.settle();
    let small = window(&fresh).width();

    let mut h = Ui1::new();
    server_down(&mut h);
    h.app.modal = Some(Modal::Invite {
        emails: String::new(),
        role: "Member".into(),
    });
    h.settle();
    // no "Work together > People" while that isn't there; the link names the place
    assert!(h.has("The team's server can't be reached right now. The invites are kept, and their invite codes are asked for once it's back."));
    assert!(h.has("Work together") && !h.has("Sharing"));
    let w = window(&h).width();
    assert!(
        w <= 640.0,
        "the note wraps instead of widening the dialog: {}",
        w
    );
    h.type_text("dana@example.com");
    h.click("Send invite");
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Invited 1 people. Their invite codes are asked for once the server is back.")
    );
    assert!(h
        .app
        .data
        .members
        .iter()
        .any(|m| m.email == "dana@example.com" && m.invited));
    // the next dialog opens at its own size
    h.app.modal = Some(bookmark());
    h.settle();
    assert!(
        (window(&h).width() - small).abs() < 2.0,
        "{} vs {}",
        window(&h).width(),
        small
    );
}

#[test]
fn share_says_when_nobody_else_is_in_the_workspace_yet() {
    let note = "Only you are in this Workspace so far. Invite people by email below to work on it together.";
    let mut h = Ui1::new();
    h.app.modal = Some(Modal::Share {
        emails: String::new(),
    });
    h.settle();
    assert!(!h.has(note), "a team has people to share with");
    let mut solo = Ui1::new();
    let me = solo.app.data.me;
    solo.app.data.members.retain(|m| m.id == me);
    solo.app.modal = Some(Modal::Share {
        emails: String::new(),
    });
    solo.settle();
    assert!(solo.has(note));
}

#[test]
fn invited_people_are_marked_in_channel_and_team_member_lists() {
    let mut h = Ui1::new();
    h.app
        .invite_people(&["dana.reed@example.com".to_string()], "Member");
    let general = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    // in #general already, for when they join: marked as not here yet
    h.app.modal = Some(Modal::ChannelSettings(general));
    h.settle();
    assert!(h.has("Dana Reed (invited)"));
    h.key(egui::Key::Escape);
    h.app.section = Section::Teams;
    h.settle();
    h.click_side("Create team", true);
    assert!(h.has("Dana Reed (invited)"));
}

#[test]
fn team_color_and_my_teams() {
    let mut h = Ui1::new();
    h.app.section = Section::Teams;
    h.settle();
    h.click_side("Create team", true);
    let id = h.app.team_open.unwrap();
    h.click("Team color 3");
    assert_eq!(
        h.app
            .data
            .extra
            .teams
            .iter()
            .find(|t| t.id == id)
            .unwrap()
            .color,
        STATUS_PALETTE[3]
    );
    h.app.data.extra.teams.push(Team {
        id: 7,
        name: "Not mine".into(),
        members: vec![2],
        color: [1, 1, 1],
    });
    h.click("My Teams");
    assert_eq!(h.app.hub_filter, 4);
    assert!(h.has("Team 1") && !h.has("Not mine"));
}

#[test]
fn a_shared_folder_says_private_items_are_readable_from_its_files() {
    let mut h = Ui1::new();
    h.click("Workspace menu");
    h.click("Work together");
    assert!(h.has(together::FOLDER_PRIVACY), "said before sharing");
    let dir = std::env::temp_dir().join(format!("wb-folder-note-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: dir.to_string_lossy().into(),
        device: "dev1".into(),
        tag: 5,
        server: None,
    });
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert!(h.has("This workspace is shared"));
    assert!(h.has(together::FOLDER_PRIVACY), "and while sharing");
    // on a server, other people's copies leave private items out: no note
    h.app.data.extra.shared.as_mut().unwrap().server = Some("127.0.0.1:1".into());
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    assert!(!h.has(together::FOLDER_PRIVACY));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_admin_sees_code_buttons_only_for_members_without_a_password() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    let me = h.app.data.me;
    let others: Vec<u64> = h
        .app
        .data
        .members
        .iter()
        .map(|m| m.id)
        .filter(|id| *id != me)
        .take(3)
        .collect();
    let person = |member, role, account| wb_net::Person {
        member,
        role,
        devices: 0,
        invite: None,
        account,
    };
    h.app.sync_rt.people = vec![
        person(me, wb_net::Role::Admin, true),
        person(others[0], wb_net::Role::Admin, false),
        person(others[1], wb_net::Role::Member, true),
        person(others[2], wb_net::Role::Member, false),
    ];
    h.app.modal = Some(h.app.together_modal());
    h.settle();
    let codes = |h: &Ui1| h.tree.iter().filter(|(n, _)| n == "Invite code").count();
    assert_eq!(codes(&h), 1, "only the member without a password");
    // the owner makes codes for everyone else
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.app.sync_rt.people[0].role = wb_net::Role::Owner;
    h.settle();
    assert_eq!(codes(&h), 3);
}

#[test]
fn share_dialog_chooses_people_and_their_levels() {
    let mut h = Ui1::new();
    let sid = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.name == "Side Projects")
        .unwrap()
        .id;
    h.click("Space options Side Projects");
    h.click("Share & permissions");
    assert!(
        h.has("Sharing is enforced on a team server only. Alone or in a shared folder, everyone with the workspace can see and change every space."),
        "without a server the dialog says nothing is enforced"
    );
    h.click("Share with chosen people");
    h.click("Add person");
    h.click("Add Sam Lee");
    h.click("Sam Lee View");
    h.click("Save");
    assert_eq!(
        sharing_of(&h, sid),
        Some((false, vec![(1, "full".into()), (3, "view".into())])),
        "whoever closes the space keeps Full in it"
    );
    // back to everyone: the people stay for next time
    h.click("Space options Side Projects");
    h.click("Share & permissions");
    h.click("Remove Sam Lee");
    h.click("Share with everyone");
    h.click("Save");
    assert_eq!(sharing_of(&h, sid), Some((true, vec![(1, "full".into())])));
    // a space never shared stays without the field when saved as everyone's
    let other = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.id != sid && s.private_owner.is_none())
        .unwrap()
        .clone();
    h.app.open_sharing(other.id);
    h.settle();
    h.click("Save");
    assert_eq!(sharing_of(&h, other.id), None);
}

#[test]
fn a_space_shared_to_view_or_comment_reads_only() {
    let mut h = Ui1::new();
    // on a team server, as a plain member
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    let task = task_named(&h, "Write copy for landing page").clone();
    let space = h.app.data.space_of_list(task.list_id).unwrap().clone();
    let share = |h: &mut Ui1, level: &str| {
        let s = h
            .app
            .data
            .spaces
            .iter_mut()
            .find(|s| s.id == space.id)
            .unwrap();
        s.sharing = Some(SpaceSharing {
            everyone: false,
            people: vec![SpaceGrant {
                member: 1,
                level: level.into(),
            }],
        });
        h.settle();
    };
    share(&mut h, "view");
    // the list says so, and the space menu leaves out what would change it
    h.app.loc = Location::List(task.list_id);
    h.settle();
    assert!(h.has("View only"));
    h.click(&format!("Space options {}", space.name));
    assert!(h.has("You can read this space.") && !h.has("Rename") && !h.has("Delete"));
    h.key(egui::Key::Escape);
    // the task window: fields and the comment box are off
    h.click_left("Task Write copy for landing page", 120.0);
    assert_eq!(h.app.open_task, Some(task.id));
    h.click("Comment box");
    h.type_text("hello");
    assert!(h.app.comment.text.is_empty(), "the comment box is off");
    // an edit that gets through some other way is turned down
    let mut renamed = task.clone();
    renamed.name = "Renamed".into();
    h.app.actions.push(Action::Replace(renamed.clone()));
    h.app.apply_actions();
    assert_eq!(h.app.task(task.id).unwrap().name, task.name);
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("You can only view this space.")
    );
    h.app.actions.push(Action::DeleteTask(task.id));
    h.app.apply_actions();
    assert!(h.app.task(task.id).is_some(), "not deleted");
    // Comment: the box works and comments go in, nothing else does
    share(&mut h, "comment");
    assert!(h.has("Can comment"));
    h.click("Comment box");
    h.type_text("looks good");
    h.click("Send Comment");
    assert!(h
        .app
        .task(task.id)
        .unwrap()
        .activity
        .iter()
        .any(|a| a.comment && a.text == "looks good" && a.author == 1));
    h.app.actions.push(Action::Replace(renamed));
    h.app.apply_actions();
    assert_eq!(h.app.task(task.id).unwrap().name, task.name);
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("You can only view and comment in this space.")
    );
    // an admin has Full everywhere
    h.app.sync_rt.role = Some(wb_net::Role::Admin);
    h.settle();
    assert!(!h.has("Can comment"));
}

/// The invite dialog's long note wraps inside the dialog instead of
/// stretching it past its widest.
#[test]
fn invite_note_wraps_inside_the_dialog() {
    let mut h = Ui1::new();
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    // connected as the owner: the longest note
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Owner);
    h.app.modal = Some(Modal::Invite {
        emails: String::new(),
        role: "Member".into(),
    });
    h.settle();
    let close = h.rect("Close");
    let note = h
        .tree
        .iter()
        .rev()
        .find(|(n, _)| n.starts_with("This workspace is on a server"))
        .map(|(_, r)| *r)
        .expect("the note");
    assert!(note.width() <= 620.0, "note {} wide", note.width());
    assert!(note.right() <= close.right() + 1.0, "inside the dialog");
}
