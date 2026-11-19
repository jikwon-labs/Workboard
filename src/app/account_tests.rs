// Accounts through a real workboard-server: the owner chooses a password
// when setting up the server, signs in on a second computer with it, invites
// someone (the code goes out by mail) who joins with the code and a password.

use super::tests::{blank_as, net_until};
use super::together::How;
use super::*;
use workboard_server::testing::{temp_home, TestServer};

fn computer(name: &str) -> App {
    App::build(None, Some(blank_as(name, 1, "Someone")))
}

fn connected(a: &App) -> bool {
    a.sync_rt.net_status == Some(wb_net::Status::Connected)
}

#[test]
fn accounts_sign_in_anywhere_and_invites_go_out_by_mail() {
    let server = TestServer::with_mail_folder();
    let addr = format!("127.0.0.1:{}", server.port);

    // the owner sets up the empty server and chooses a password
    let mut a = computer("Ours");
    a.connect_with(
        &addr,
        How::Claim {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut a, "A on the server", |a| {
        a.data.extra.shared.is_some() && connected(a)
    });
    assert_eq!(a.sync_rt.account.as_deref(), Some("owner@team.io"));
    assert_eq!(
        a.data.member(a.data.me).unwrap().email,
        "owner@team.io",
        "the owner's email goes up with the workspace"
    );
    while !server.home.join(workboard_server::WORKSPACE_FILE).is_file() {
        a.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // the owner's second computer: email and password, no code
    let mut a2 = computer("Laptop");
    a2.connect_with(
        &addr,
        How::Account {
            email: "OWNER@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut a2, "A's laptop joins", |a| {
        a.data.extra.shared.is_some() && connected(a)
    });
    assert_eq!(
        (a2.data.me, a2.sync_rt.role),
        (a.data.me, Some(wb_net::Role::Owner))
    );
    assert_eq!(a2.data.tasks.len(), a.data.tasks.len());
    // a wrong password is refused and nothing changes
    let mut x = computer("Nope");
    x.connect_with(
        &addr,
        How::Account {
            email: "owner@team.io".into(),
            password: "guess-guess".into(),
        },
    )
    .unwrap();
    net_until(&mut x, "refused", |x| !x.sync_rt.form.note.is_empty());
    assert!(
        x.sync_rt.form.note.contains("don't match"),
        "{}",
        x.sync_rt.form.note
    );
    assert!(x.data.extra.shared.is_none() && x.sync_rt.joining.is_none());

    // inviting someone on the server asks for their code, which the server emails
    assert_eq!(a.invite_people(&["new.person@team.io".into()], "Member"), 1);
    let new_id = a
        .data
        .members
        .iter()
        .find(|m| m.email == "new.person@team.io")
        .unwrap()
        .id;
    a.sync_note_change();
    a.write_shared();
    net_until(&mut a, "the code is made and emailed", |a| {
        a.toast
            .as_ref()
            .is_some_and(|t| t.0.contains("Emailed") && t.0.contains("new.person@team.io"))
    });
    let code = a
        .sync_rt
        .people
        .iter()
        .find(|p| p.member == new_id)
        .and_then(|p| p.invite.clone())
        .expect("code in People");
    let mail = server.mails_to("new.person@team.io").pop().expect("a mail");
    assert!(
        mail.contains(&code) && mail.contains(&format!("{}/join?code={}", addr, code)),
        "{}",
        mail
    );

    // they join from the link's code and choose a password
    let mut c = computer("Theirs");
    c.connect_with(
        &addr,
        How::Invite {
            code: code.clone(),
            email: String::new(),
            password: "their-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut c, "C joins", |c| {
        c.data.extra.shared.is_some() && connected(c)
    });
    assert_eq!(c.data.me, new_id);
    assert_eq!(c.sync_rt.account.as_deref(), Some("new.person@team.io"));
    // and change it later
    c.access_request(wb_net::Msg::SetPassword {
        email: "new.person@team.io".into(),
        password: "their-pass-2".into(),
        old_password: Some("their-pass-1".into()),
    });
    net_until(&mut c, "password changed", |c| {
        c.toast
            .as_ref()
            .is_some_and(|t| t.0.starts_with("Password saved"))
    });

    // signed out by the owner: back in with email and password is no longer possible,
    // a new invite code with a new password is
    a.access_request(wb_net::Msg::Revoke { member: new_id });
    net_until(&mut c, "C signed out", |c| {
        matches!(c.sync_rt.net_status, Some(wb_net::Status::Refused(_)))
    });
    c.sign_in_with(How::Account {
        email: "new.person@team.io".into(),
        password: "their-pass-2".into(),
    });
    net_until(
        &mut c,
        "the account is gone",
        |c| matches!(&c.sync_rt.net_status, Some(wb_net::Status::Refused(why)) if why.contains("don't match")),
    );
    a.access_request(wb_net::Msg::CreateInvite {
        member: new_id,
        role: wb_net::Role::Member,
    });
    net_until(&mut a, "new code", |a| {
        a.sync_rt
            .people
            .iter()
            .any(|p| p.member == new_id && p.invite.as_deref().is_some_and(|i| i != code))
    });
    let code2 = a
        .sync_rt
        .people
        .iter()
        .find(|p| p.member == new_id)
        .and_then(|p| p.invite.clone())
        .unwrap();
    c.sign_in_with(How::Invite {
        code: code2,
        email: String::new(),
        password: "their-pass-3".into(),
    });
    net_until(&mut c, "C back in", connected);
    assert!(c.sync_rt.resign.is_none() && c.sync_rt.account.is_some());
}

#[test]
fn sign_in_fields_become_the_hello() {
    let s = How::Invite {
        code: " abcd-efgh-jkmn ".into(),
        email: "".into(),
        password: "p w".into(),
    }
    .sign_in("d1", 7);
    assert_eq!(
        (
            s.invite.as_deref(),
            s.email,
            s.new_password.as_deref(),
            s.claim
        ),
        (Some("abcd-efgh-jkmn"), None, Some("p w"), None)
    );
    let s = How::Claim {
        email: "".into(),
        password: "".into(),
    }
    .sign_in("d1", 7);
    assert_eq!(
        (s.claim, s.email, s.new_password),
        (Some(7), None, None),
        "a claim without an account, as before"
    );
    let s = How::Reset {
        email: "a@b.co".into(),
        code: "X".into(),
        password: "pw".into(),
    }
    .sign_in("d1", 7);
    assert_eq!(
        (s.reset_code.as_deref(), s.email.as_deref(), s.password),
        (Some("X"), Some("a@b.co"), None)
    );
    assert!(together::new_password_problem("1234567", "1234567").is_some());
    assert!(together::new_password_problem("12345678", "12345679").is_some());
    assert!(together::new_password_problem("12345678", "12345678").is_none());
}

#[test]
fn logging_out_sends_what_is_left_deletes_the_copy_and_signing_in_brings_it_back() {
    let server = TestServer::with_mail_folder();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = computer("Ours");
    a.connect_with(
        &addr,
        How::Claim {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut a, "A on the server", |a| {
        a.data.extra.shared.is_some() && connected(a)
    });
    while !server.home.join(workboard_server::WORKSPACE_FILE).is_file() {
        a.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // the owner's laptop signs in, writes something and logs out at once
    let mut b = computer("Laptop");
    b.connect_with(
        &addr,
        How::Account {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut b, "the laptop joins", |b| {
        b.data.extra.shared.is_some() && connected(b)
    });
    let device = b.data.extra.shared.as_ref().unwrap().device.clone();
    assert!(together::load_token(&addr, &device).is_some());
    let said = "sent just before logging out";
    let has = |d: &Data| {
        d.channels
            .iter()
            .any(|c| c.messages.iter().any(|m| m.text == said))
    };
    let ch = b
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .id;
    b.apply_chat(vec![super::chat::ChatAction::Send {
        ch,
        text: said.into(),
        parent: None,
        files: vec![],
    }]);
    b.log_out();
    assert!(b.data.extra.shared.is_none() && b.login.is_some());
    assert!(together::load_token(&addr, &device).is_none());
    assert!(!has(&b.data), "the laptop's copy is gone");
    // what was written last still reached the team
    net_until(&mut a, "the last message at A", |a| has(&a.data));

    // signing in again brings the team's workspace back
    b.connect_with(
        &addr,
        How::Account {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut b, "the laptop back in", |b| {
        b.data.extra.shared.is_some() && connected(b)
    });
    assert!(b.login.is_none() && !b.data.extra.welcome);
    assert!(has(&b.data));
}

/// A computer that keeps its workspace files (in a HOME of the test's own).
fn keeping_computer(name: &str) -> App {
    let mut a = computer(name);
    a.persist = true;
    a.data.save();
    a
}

#[test]
fn a_second_workspace_goes_on_the_same_server_and_opens_anywhere() {
    let _home = super::tests::HOME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home-several");
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let server = TestServer::with_mail_folder();
    let addr = format!("127.0.0.1:{}", server.port);

    // the owner sets up the server with an account
    let mut a = keeping_computer("Ours");
    a.connect_with(
        &addr,
        How::Claim {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut a, "A on the server", |a| {
        a.data.extra.shared.is_some() && connected(a)
    });
    assert_eq!(a.server_for_new_workspace(), Some(addr.clone()));
    let first = a.ws_file.clone();

    // a second workspace, on the same server, with someone to invite
    a.make_workspace_on_server(
        "Side project",
        &[],
        &["helper@team.io".to_string()],
        "Member",
        None,
    );
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while a.making_ws.is_some() && std::time::Instant::now() < end {
        a.making_ws_tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(a.making_ws.is_none(), "the server answered: {:?}", a.toast);
    net_until(&mut a, "the second workspace is up", |a| {
        a.data.workspace == "Side project" && connected(a)
    });
    let second = a.ws_file.clone();
    assert_ne!(second, first);
    let addr2 = a
        .data
        .extra
        .shared
        .as_ref()
        .and_then(|c| c.server.clone())
        .unwrap();
    assert!(addr2.starts_with(&format!("{}/w/", addr)), "{}", addr2);
    assert_eq!(a.sync_rt.role, Some(wb_net::Role::Owner));
    assert_eq!(a.sync_rt.account.as_deref(), Some("owner@team.io"));
    assert_eq!(a.workspace_place(&first), "Team server");
    assert_eq!(a.workspace_place(&second), "Team server");
    // the person invited gets their code by mail, with the second
    // workspace's address in the link
    let id = addr2.rsplit('/').next().unwrap().to_string();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while server.mails_to("helper@team.io").is_empty() && std::time::Instant::now() < end {
        a.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let mail = server
        .mails_to("helper@team.io")
        .pop()
        .expect("invite mail");
    assert!(mail.contains(&format!("/w/{}/join?code=", id)), "{}", mail);
    let folder = server.home.join("workspaces").join(&id);
    while !folder.join(workboard_server::WORKSPACE_FILE).is_file() {
        a.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // another computer signs in with the account: it chooses which one
    let mut b = keeping_computer("Laptop");
    b.connect_with(
        &addr,
        How::Account {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut b, "the choices", |b| b.sync_rt.choices.is_some());
    let (list, how) = b.sync_rt.choices.clone().unwrap();
    let mut names: Vec<&str> = list.iter().map(|(_, n)| n.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["Ours", "Side project"]);
    b.open_or_join(&addr2, how.clone());
    net_until(&mut b, "B joined the second", |b| {
        b.data.workspace == "Side project" && connected(b)
    });
    let b_second = b.ws_file.clone();
    let files = Data::workspaces().len();

    // the first one too, then the second again: that opens the copy here
    b.open_or_join(&addr, how.clone());
    net_until(&mut b, "B joined the first", |b| {
        b.data.workspace == "Ours" && connected(b)
    });
    assert_eq!(Data::workspaces().len(), files + 1);
    b.connect_with(
        &addr,
        How::Account {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut b, "the choices again", |b| b.sync_rt.choices.is_some());
    b.open_or_join(&addr2, how);
    net_until(&mut b, "the second opened", |b| {
        b.data.workspace == "Side project"
    });
    assert_eq!(b.ws_file, b_second, "the copy already here");
    assert_eq!(
        Data::workspaces().len(),
        files + 1,
        "no second copy of a workspace"
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_owner_deletes_a_second_workspace_and_every_computer_loses_it() {
    let _home = super::tests::HOME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home-delws");
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let server = TestServer::with_mail_folder();
    let addr = format!("127.0.0.1:{}", server.port);

    let mut a = keeping_computer("Ours");
    a.connect_with(
        &addr,
        How::Claim {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut a, "A on the server", |a| {
        a.data.extra.shared.is_some() && connected(a)
    });
    let first = a.ws_file.clone();
    // the server's first workspace can't be deleted
    assert_eq!(
        a.delete_how(),
        Err(Some(
            "This is the team server's first workspace. It can't be deleted."
        ))
    );
    a.make_workspace_on_server("Side project", &[], &[], "Member", None);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while a.making_ws.is_some() && std::time::Instant::now() < end {
        a.making_ws_tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    net_until(&mut a, "the second workspace is up", |a| {
        a.data.workspace == "Side project" && connected(a)
    });
    let second = a.ws_file.clone();
    let addr2 = a
        .data
        .extra
        .shared
        .as_ref()
        .and_then(|c| c.server.clone())
        .unwrap();
    let id = addr2.rsplit('/').next().unwrap().to_string();
    let folder = server.home.join("workspaces").join(&id);
    while !folder.join(workboard_server::WORKSPACE_FILE).is_file() {
        a.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    // another computer has it open
    let mut b = computer("Laptop");
    b.connect_with(
        &addr,
        How::Account {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut b, "the choices", |b| b.sync_rt.choices.is_some());
    let (_, how) = b.sync_rt.choices.clone().unwrap();
    b.open_or_join(&addr2, how);
    net_until(&mut b, "B has the second", |b| {
        b.data.workspace == "Side project" && connected(b)
    });

    // the owner deletes it
    assert_eq!(
        a.delete_how(),
        Ok(super::delete_ws::DeleteHow::Server(addr2.clone()))
    );
    a.delete_on_server(&addr2, egui::Context::default());
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while a.deleting_ws.is_some() && std::time::Instant::now() < end {
        a.deleting_ws_tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(a.deleting_ws.is_none());
    assert_eq!(
        a.toast.as_ref().map(|t| t.0.as_str()),
        Some("Deleted Side project")
    );
    // A is back on the first, and the second is gone from this computer
    assert_eq!(a.ws_file, first);
    assert_eq!(a.data.workspace, "Ours");
    assert!(!Data::workspaces().iter().any(|w| w.file == second));
    assert!(!Data::dir().join(&second).exists());
    net_until(&mut a, "A back on the first", connected);
    // nothing still queued for the background writer brings it back
    Data::finish_saving();
    assert!(!Data::dir().join(&second).exists());
    assert!(!folder.exists());
    assert_eq!(workboard_server::registry::deleted(&server.home).len(), 1);

    // B hears it and stops; it removes its copy
    net_until(&mut b, "B told it's gone", |b| {
        matches!(b.sync_rt.net_status, Some(wb_net::Status::Gone(_)))
    });
    b.remove_gone_workspace();
    assert!(b.data.extra.shared.is_none());
    assert_ne!(b.data.workspace, "Side project");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_message_in_a_workspace_not_open_is_announced_and_opens_it() {
    let _home = super::tests::HOME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home-watch");
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    let server = TestServer::with_mail_folder();
    let addr = format!("127.0.0.1:{}", server.port);

    // the owner sets up the server, and Sam joins the first workspace
    let mut a = keeping_computer("Ours");
    a.connect_with(
        &addr,
        How::Claim {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut a, "A on the server", |a| {
        a.data.extra.shared.is_some() && connected(a)
    });
    let first = a.ws_file.clone();
    let me = a.data.me;
    assert_eq!(a.invite_people(&["sam@team.io".into()], "Member"), 1);
    let sam = a
        .data
        .members
        .iter()
        .find(|m| m.email == "sam@team.io")
        .unwrap()
        .id;
    a.sync_note_change();
    a.write_shared();
    net_until(&mut a, "Sam's code", |a| {
        a.sync_rt
            .people
            .iter()
            .any(|p| p.member == sam && p.invite.is_some())
    });
    let code = a
        .sync_rt
        .people
        .iter()
        .find(|p| p.member == sam)
        .and_then(|p| p.invite.clone())
        .unwrap();
    let mut s = computer("Sam's");
    s.connect_with(
        &addr,
        How::Invite {
            code,
            email: String::new(),
            password: "sam-pass-1".into(),
        },
    )
    .unwrap();
    net_until(&mut s, "Sam joins", |s| {
        s.data.extra.shared.is_some() && connected(s)
    });

    // the owner opens a second workspace on the server: the first is watched
    a.make_workspace_on_server("Side project", &[], &[], "Member", None);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while a.making_ws.is_some() && std::time::Instant::now() < end {
        a.making_ws_tick();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    net_until(&mut a, "the second workspace is up", |a| {
        a.data.workspace == "Side project" && connected(a)
    });
    let second = a.ws_file.clone();
    let tick_until = |a: &mut App, what: &str, cond: &dyn Fn(&App) -> bool| {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            a.net_tick(None);
            a.watch_tick(None);
            if cond(a) {
                return;
            }
            assert!(
                std::time::Instant::now() < end,
                "timed out waiting for: {}",
                what
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    };
    tick_until(&mut a, "watching the first workspace", &|a| {
        a.watch_rt.listening(&first)
    });
    assert_eq!(a.watch_rt.files(), vec![first.clone()], "not the open one");

    // Sam sends the owner a direct message in the first workspace
    let dm = 9100;
    s.data.channels.push(Channel {
        id: dm,
        kind: ChannelKind::Direct,
        members: vec![me, sam],
        messages: vec![Message {
            id: 9101,
            author: sam,
            text: "got a minute?".into(),
            ts: now_ts(),
            reactions: vec![],
            replies: vec![],
            saved: false,
            task: None,
            post_title: None,
            post_type: String::new(),
            pinned: false,
            call: None,
            attachments: vec![],
            edited: None,
            imported: None,
            preview: None,
            forwarded: None,
            thread_of: None,
            bot: None,
            mentions: vec![],
        }],
        ..Default::default()
    });
    s.sync_note_change();
    s.write_shared();
    tick_until(&mut a, "the notice from the first workspace", &|a| {
        a.notify_rt
            .sent
            .iter()
            .any(|n| n.target.starts_with(&format!("ws:{}:", first)))
    });
    let note = a.notify_rt.sent.last().unwrap().clone();
    assert_eq!(note.title, "Ours \u{b7} Sam");
    assert_eq!(note.body, "got a minute?");
    assert_eq!(note.target, format!("ws:{}:chat:{}", first, dm));
    assert_eq!(a.data.workspace, "Side project", "nothing opened by itself");

    // a click opens the first workspace and, once its copy has it, the conversation
    a.open_target(&note.target);
    assert_eq!(a.ws_file, first);
    tick_until(&mut a, "the conversation open", &|a| {
        a.section == Section::Chat && a.chat.open == Some(dm)
    });
    // now the second one is watched instead
    tick_until(&mut a, "watching the second workspace", &|a| {
        a.watch_rt.listening(&second)
    });
    assert_eq!(a.watch_rt.files(), vec![second.clone()]);
    let _ = std::fs::remove_dir_all(&home);
}
