// One JSON file per message the iPhone app sends or reads, as `wb_net::Msg`
// writes it. The iPhone app keeps its own copy of these messages
// (ios/WorkboardKit Protocol.swift); its ProtocolGoldenTests.swift reads every
// file here, decodes it and encodes it again, and must get the same JSON.
// So a field added, renamed or dropped on either side fails one of the two.
//
// to_server/ is what the app sends, to_app/ what the server sends.
//
// After changing a message on purpose: WB_GOLDEN_WRITE=1 cargo test -p wb-net
// --test protocol, review the diff, and make the Swift side match.

use serde_json::{json, Value};
use std::path::PathBuf;
use wb_net::{
    Backup, ChannelMode, CommandInfo, HookInfo, Msg, Person, PushPrefs, Role, ScheduledMsg,
};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/protocol")
}

fn data() -> Value {
    json!({"channels": [{"id": 301, "name": "general"}], "next_id": 7})
}

fn scheduled() -> ScheduledMsg {
    ScheduledMsg {
        id: 1763000000123,
        channel: 301,
        parent: Some(5001),
        text: "Standup in 5 @Sam".into(),
        at: 1763003600,
        mentions: vec![3],
    }
}

fn hook() -> HookInfo {
    HookInfo {
        token: "hk_abc".into(),
        kind: "outgoing".into(),
        channel: 301,
        name: "Deploy bot".into(),
        url: "https://example.com/hook".into(),
        trigger: "deploy".into(),
        creator: 2,
    }
}

fn command() -> CommandInfo {
    CommandInfo {
        name: "weather".into(),
        description: "Today's weather".into(),
        url: "https://example.com/weather".into(),
    }
}

fn hello() -> Msg {
    Msg::Hello {
        device: "dev-1".into(),
        tag: None,
        token: None,
        invite: None,
        claim: None,
        email: None,
        password: None,
        new_password: None,
        reset_code: None,
        link_code: None,
        app: None,
        sync: None,
        deltas: false,
        watch: false,
    }
}

/// What the app sends.
fn to_server() -> Vec<(&'static str, Msg)> {
    let with = |f: &dyn Fn(&mut Msg)| {
        let mut m = hello();
        f(&mut m);
        m
    };
    vec![
        (
            "hello_token",
            with(&|m| {
                if let Msg::Hello {
                    tag,
                    token,
                    app,
                    sync,
                    deltas,
                    ..
                } = m
                {
                    *tag = Some(5);
                    *token = Some("tok-123".into());
                    *app = Some("0.1.23".into());
                    *sync = Some(3);
                    *deltas = true;
                }
            }),
        ),
        (
            "hello_password",
            with(&|m| {
                if let Msg::Hello {
                    email, password, ..
                } = m
                {
                    *email = Some("sam@example.com".into());
                    *password = Some("secret-pass".into());
                }
            }),
        ),
        (
            "hello_invite",
            with(&|m| {
                if let Msg::Hello {
                    invite,
                    email,
                    new_password,
                    ..
                } = m
                {
                    *invite = Some("ABCD-EFGH-JKLM".into());
                    *email = Some("sam@example.com".into());
                    *new_password = Some("new-secret".into());
                }
            }),
        ),
        (
            "hello_reset",
            with(&|m| {
                if let Msg::Hello {
                    reset_code,
                    email,
                    new_password,
                    ..
                } = m
                {
                    *reset_code = Some("RSET-CODE-0001".into());
                    *email = Some("sam@example.com".into());
                    *new_password = Some("new-secret".into());
                }
            }),
        ),
        (
            "hello_link",
            with(&|m| {
                if let Msg::Hello { link_code, .. } = m {
                    *link_code = Some("K3P9QX".into());
                }
            }),
        ),
        ("push", Msg::Push { data: data() }),
        ("resync", Msg::Resync),
        (
            "set_password",
            Msg::SetPassword {
                email: "sam@example.com".into(),
                password: "new-secret".into(),
                old_password: Some("old-secret".into()),
            },
        ),
        (
            "set_password_first",
            Msg::SetPassword {
                email: "sam@example.com".into(),
                password: "new-secret".into(),
                old_password: None,
            },
        ),
        (
            "push_token",
            Msg::PushToken {
                token: "apns-token".into(),
                platform: "ios".into(),
                env: "production".into(),
                workspace: Some("macmini:8787".into()),
                prefs: Some(PushPrefs {
                    direct: true,
                    mentions: false,
                    replies: true,
                    channel_default: "all".into(),
                    channel_modes: vec![ChannelMode {
                        channel: 301,
                        mode: "nothing".into(),
                    }],
                    keywords: vec!["launch".into(), "deploy".into()],
                    muted_until: 1763009999,
                }),
            },
        ),
        (
            "push_token_stop",
            Msg::PushToken {
                token: String::new(),
                platform: "ios".into(),
                env: "development".into(),
                workspace: None,
                prefs: None,
            },
        ),
        ("call_join", Msg::CallJoin { channel: 301 }),
        (
            "typing",
            Msg::Typing {
                channel: 301,
                member: None,
                thread: Some(5001),
            },
        ),
        (
            "create_invite",
            Msg::CreateInvite {
                member: 2,
                role: Role::Admin,
            },
        ),
        (
            "set_role",
            Msg::SetRole {
                member: 2,
                role: Role::Owner,
            },
        ),
        ("revoke", Msg::Revoke { member: 2 }),
        ("access_list", Msg::AccessList),
        ("backup_list", Msg::BackupList),
        (
            "restore",
            Msg::Restore {
                name: "hourly-20261115-03.json".into(),
            },
        ),
        ("schedule", Msg::Schedule { item: scheduled() }),
        ("unschedule", Msg::Unschedule { id: 1763000000123 }),
        ("scheduled_list", Msg::ScheduledList),
        (
            "mine",
            Msg::Mine {
                data: json!({"read": {"301": 5001}}),
            },
        ),
        ("integrations", Msg::Integrations),
        ("hook_add", Msg::HookAdd { hook: hook() }),
        (
            "hook_remove",
            Msg::HookRemove {
                token: "hk_abc".into(),
            },
        ),
        ("command_add", Msg::CommandAdd { command: command() }),
        (
            "command_remove",
            Msg::CommandRemove {
                name: "weather".into(),
            },
        ),
        (
            "command",
            Msg::Command {
                channel: 301,
                parent: Some(5001),
                name: "weather".into(),
                text: "Seoul".into(),
            },
        ),
        ("ping", Msg::Ping),
        ("pong", Msg::Pong),
    ]
}

/// What the server sends that the app reads.
fn to_app() -> Vec<(&'static str, Msg)> {
    vec![
        (
            "welcome",
            Msg::Welcome {
                tag: 5,
                snapshot: Some(data()),
                rev: 12,
                member: 2,
                role: Role::Admin,
                token: Some("tok-123".into()),
                email: Some("sam@example.com".into()),
                hidden: vec![9001],
                deltas: true,
            },
        ),
        (
            "welcome_empty",
            Msg::Welcome {
                tag: 1,
                snapshot: None,
                rev: 0,
                member: 1,
                role: Role::Member,
                token: None,
                email: None,
                hidden: vec![],
                deltas: false,
            },
        ),
        (
            "update",
            Msg::Update {
                data: data(),
                rev: 13,
                hidden: vec![9001],
            },
        ),
        (
            "delta",
            Msg::Delta {
                data: data(),
                rev: 14,
                base: 13,
                hidden: vec![],
            },
        ),
        (
            "account",
            Msg::Account {
                email: "sam@example.com".into(),
            },
        ),
        (
            "call_token",
            Msg::CallToken {
                channel: 301,
                url: "wss://lk.example.com".into(),
                token: "jwt".into(),
            },
        ),
        (
            "typing",
            Msg::Typing {
                channel: 301,
                member: Some(3),
                thread: None,
            },
        ),
        (
            "invite",
            Msg::Invite {
                member: 2,
                code: "ABCD-EFGH-JKLM".into(),
                emailed: Some("sam@example.com".into()),
                mail_error: None,
            },
        ),
        (
            "invite_mail_failed",
            Msg::Invite {
                member: 2,
                code: "ABCD-EFGH-JKLM".into(),
                emailed: None,
                mail_error: Some("no mail server".into()),
            },
        ),
        (
            "access",
            Msg::Access {
                people: vec![
                    Person {
                        member: 1,
                        role: Role::Owner,
                        devices: 2,
                        invite: None,
                        account: true,
                    },
                    Person {
                        member: 2,
                        role: Role::Member,
                        devices: 0,
                        invite: Some("ABCD-EFGH-JKLM".into()),
                        account: false,
                    },
                ],
                role: Role::Owner,
                answer: Some(true),
            },
        ),
        (
            "backups",
            Msg::Backups {
                backups: vec![Backup {
                    name: "hourly-20261115-03.json".into(),
                    bytes: 123456,
                }],
            },
        ),
        (
            "restored",
            Msg::Restored {
                name: "hourly-20261115-03.json".into(),
            },
        ),
        ("presence", Msg::Presence { online: vec![1, 3] }),
        (
            "scheduled",
            Msg::Scheduled {
                items: vec![scheduled()],
            },
        ),
        ("mine", Msg::Mine { data: json!({}) }),
        (
            "integration_list",
            Msg::IntegrationList {
                hooks: vec![hook()],
                commands: vec![command()],
            },
        ),
        ("ping", Msg::Ping),
        ("pong", Msg::Pong),
        ("error", Msg::error("Say hello first.", None)),
        (
            "error_auth",
            Msg::error("That code isn't right.", Some(wb_net::CODE_AUTH)),
        ),
        (
            "error_call",
            Msg::call_denied("Huddles aren't set up.", 301),
        ),
        (
            "error_put_back",
            Msg::Error {
                message: "Some changes were turned down.".into(),
                code: Some(wb_net::CODE_DENIED.into()),
                channel: None,
                put_back: true,
                gone: false,
            },
        ),
        ("error_gone", Msg::gone()),
    ]
}

fn check(folder: &str, msgs: Vec<(&'static str, Msg)>) -> Vec<String> {
    let dir = dir().join(folder);
    let write = std::env::var("WB_GOLDEN_WRITE").is_ok_and(|v| v == "1");
    if write {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
    }
    let mut problems = vec![];
    let mut names = vec![];
    for (name, msg) in msgs {
        let file = format!("{name}.json");
        let text = serde_json::to_string_pretty(&msg).unwrap() + "\n";
        // what the server writes must read back as the same message
        assert_eq!(Msg::parse(&text).as_ref(), Some(&msg), "{folder}/{file}");
        let path = dir.join(&file);
        if write {
            std::fs::write(&path, &text).unwrap();
        }
        match std::fs::read_to_string(&path) {
            // (a Windows checkout may turn the files' line ends into \r\n)
            Ok(saved) if saved.replace("\r\n", "\n") == text => {}
            Ok(_) => problems.push(format!("{folder}/{file} differs; now:\n{text}")),
            Err(_) => problems.push(format!("{folder}/{file} is missing")),
        }
        names.push(file);
    }
    for entry in std::fs::read_dir(&dir).unwrap() {
        let file = entry.unwrap().file_name().to_string_lossy().to_string();
        if !names.contains(&file) {
            problems.push(format!("{folder}/{file} is no longer written"));
        }
    }
    problems
}

#[test]
fn messages_the_iphone_app_uses_match_the_saved_files() {
    let mut problems = check("to_server", to_server());
    problems.extend(check("to_app", to_app()));
    assert!(
        problems.is_empty(),
        "the wire format changed (the iPhone app reads these files too); \
         to accept: WB_GOLDEN_WRITE=1 cargo test -p wb-net --test protocol\n{}",
        problems.join("\n")
    );
}
