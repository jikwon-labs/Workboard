// Accounts through a real workboard-server: the owner chooses a password
// when setting up the server, signs in on a second computer with it, invites
// someone (the code goes out by mail) who joins with the code and a password.

use super::together::How;
use super::*;

struct Server {
    port: u16,
    home: std::path::PathBuf,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    /// A server that writes its mails to outbox/ instead of sending them.
    fn start() -> Server {
        let home =
            std::env::temp_dir().join(format!("wb-accounts-{:x}", crate::sync::random_u64()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join(workboard_server::MAIL_FILE),
            r#"{"transport": "folder"}"#,
        )
        .unwrap();
        let h = home.clone();
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                port_tx.send(listener.local_addr().unwrap().port()).unwrap();
                workboard_server::serve(listener, h, async {
                    let _ = stop_rx.await;
                })
                .await
                .unwrap();
            });
        });
        let port = port_rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        Server {
            port,
            home,
            stop: Some(stop_tx),
            thread: Some(thread),
        }
    }

    fn mails_to(&self, to: &str) -> Vec<String> {
        let mut files: Vec<_> = std::fs::read_dir(self.home.join(workboard_server::OUTBOX_DIR))
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.sort();
        files
            .iter()
            .filter_map(|p| std::fs::read_to_string(p).ok())
            .filter(|t| t.starts_with(&format!("To: {}\n", to)))
            .collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            let _ = s.send(());
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn until(a: &mut App, what: &str, cond: impl Fn(&App) -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < end {
        a.net_tick(None);
        if cond(a) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!(
        "timed out waiting for: {} ({:?}, toast {:?})",
        what, a.sync_rt.net_status, a.toast
    );
}

fn computer(name: &str) -> App {
    App::build(
        None,
        Some(Data::blank(
            name,
            Member {
                id: 1,
                name: "Someone".into(),
                ..Default::default()
            },
        )),
    )
}

fn connected(a: &App) -> bool {
    a.sync_rt.net_status == Some(wb_net::Status::Connected)
}

#[test]
fn accounts_sign_in_anywhere_and_invites_go_out_by_mail() {
    let server = Server::start();
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
    until(&mut a, "A on the server", |a| {
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
    until(&mut a2, "A's laptop joins", |a| {
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
    until(&mut x, "refused", |x| {
        matches!(x.sync_rt.net_status, Some(wb_net::Status::Refused(_)))
    });
    assert!(x.data.extra.shared.is_none());

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
    until(&mut a, "the code is made and emailed", |a| {
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
    until(&mut c, "C joins", |c| {
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
    until(&mut c, "password changed", |c| {
        c.toast
            .as_ref()
            .is_some_and(|t| t.0.starts_with("Password saved"))
    });

    // signed out by the owner: back in with email and password is no longer possible,
    // a new invite code with a new password is
    a.access_request(wb_net::Msg::Revoke { member: new_id });
    until(&mut c, "C signed out", |c| {
        matches!(c.sync_rt.net_status, Some(wb_net::Status::Refused(_)))
    });
    c.sign_in_with(How::Account {
        email: "new.person@team.io".into(),
        password: "their-pass-2".into(),
    });
    until(
        &mut c,
        "the account is gone",
        |c| matches!(&c.sync_rt.net_status, Some(wb_net::Status::Refused(why)) if why.contains("don't match")),
    );
    a.access_request(wb_net::Msg::CreateInvite {
        member: new_id,
        role: wb_net::Role::Member,
    });
    until(&mut a, "new code", |a| {
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
    until(&mut c, "C back in", connected);
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
    let server = Server::start();
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
    until(&mut a, "A on the server", |a| {
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
    until(&mut b, "the laptop joins", |b| {
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
    until(&mut a, "the last message at A", |a| has(&a.data));

    // signing in again brings the team's workspace back
    b.connect_with(
        &addr,
        How::Account {
            email: "owner@team.io".into(),
            password: "owner-pass-1".into(),
        },
    )
    .unwrap();
    until(&mut b, "the laptop back in", |b| {
        b.data.extra.shared.is_some() && connected(b)
    });
    assert!(b.login.is_none() && !b.data.extra.welcome);
    assert!(has(&b.data));
}
