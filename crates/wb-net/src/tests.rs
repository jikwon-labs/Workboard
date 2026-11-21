use crate::client::{queue, send_failed, MAX_QUEUED};
use crate::*;
use serde_json::Value;

/// Form-encoded text (an incoming webhook's payload=) and query values
/// come back as sent; a stray % is kept.
#[test]
fn pct_decode_undoes_encoding_and_keeps_bad_escapes() {
    assert_eq!(pct_decode(&pct_encode("a b+c/한")), "a b+c/한");
    assert_eq!(pct_decode("Build+passed"), "Build passed");
    assert_eq!(pct_decode("100%zz%4"), "100%zz%4");
    assert_eq!(pct_decode("%7B%22x%22%7D"), "{\"x\"}");
}

/// AI requests and answers on the wire: the kind of request rides in
/// `kind`, empty extras stay out, and the answer's pieces and end carry
/// the app's own request number.
#[test]
fn ai_messages_on_the_wire() {
    let ask = Msg::Ai {
        id: 7,
        ask: AiAsk::SummarizeThread {
            channel: 3,
            message: 9,
        },
        lang: "ko".into(),
    };
    let v: Value = serde_json::from_str(&ask.to_json()).unwrap();
    assert_eq!(
        v,
        serde_json::json!({"type": "ai", "id": 7, "lang": "ko",
            "ask": {"kind": "summarize_thread", "channel": 3, "message": 9}})
    );
    assert_eq!(Msg::parse(&ask.to_json()), Some(ask));
    let q = Msg::parse(r#"{"type":"ai","id":1,"ask":{"kind":"question","question":"Who?"}}"#);
    assert_eq!(
        q,
        Some(Msg::Ai {
            id: 1,
            ask: AiAsk::Question {
                question: "Who?".into(),
                history: vec![]
            },
            lang: String::new()
        })
    );
    let w = Msg::Ai {
        id: 2,
        ask: AiAsk::Write {
            how: "fix".into(),
            text: "teh".into(),
            title: String::new(),
        },
        lang: String::new(),
    };
    assert!(!w.to_json().contains("title") && !w.to_json().contains("lang"));
    assert_eq!(
        Msg::AiDone { id: 2, error: None }.to_json(),
        r#"{"type":"ai_done","id":2}"#
    );
    assert_eq!(
        Msg::AiText {
            id: 2,
            text: "the".into()
        }
        .to_json(),
        r#"{"type":"ai_text","id":2,"text":"the"}"#
    );
    assert_eq!(
        Msg::parse(r#"{"type":"ai_ready","model":"m"}"#),
        Some(Msg::AiReady { model: "m".into() })
    );
}

#[test]
fn web_addresses_on_the_server() {
    assert_eq!(
        http_address("https://wb.example.com/w/ab12", "/hooks/t1").as_deref(),
        Some("https://wb.example.com/w/ab12/hooks/t1")
    );
    assert_eq!(
        http_address("macmini", "/hooks/t1").as_deref(),
        Some("http://macmini:8787/hooks/t1")
    );
}

#[test]
fn a_workspace_below_the_server_has_its_own_paths() {
    assert_eq!(
        server_url("macmini/w/ab12").unwrap().2,
        "ws://macmini:8787/w/ab12/ws"
    );
    assert_eq!(
        server_url("https://wb.example.com/w/ab12/").unwrap().2,
        "wss://wb.example.com/w/ab12/ws"
    );
    assert_eq!(
        server_url("ws://h:1/w/ab12/ws").unwrap().2,
        "ws://h:1/w/ab12/ws"
    );
    // the first workspace keeps the addresses it always had
    assert_eq!(server_url("http://h:1/ws").unwrap().2, "ws://h:1/ws");
    assert_eq!(workspace_prefix("https://wb.example.com"), "");
    assert_eq!(workspace_prefix("ws://h:1/ws"), "");
    assert_eq!(workspace_prefix("https://wb.example.com/w/ab12"), "/w/ab12");
    assert_eq!(workspace_prefix("macmini/w/ab12/ws"), "/w/ab12");
    assert_eq!(
        workspace_addr("https://wb.example.com/w/ab12", "cd34"),
        "https://wb.example.com/w/cd34"
    );
    assert_eq!(workspace_addr("macmini:9000/w/ab12", ""), "macmini:9000");
    assert_eq!(
        server_part("https://wb.example.com/w/ab12"),
        "https://wb.example.com"
    );
    // app updates come from the server, whichever workspace
    assert_eq!(
        updates_url("https://wb.example.com/w/ab12").unwrap(),
        "https://wb.example.com/updates/appcast.xml"
    );
    assert!(same_workspace("macmini", "ws://macmini:8787/ws"));
    assert!(same_workspace(
        "macmini/w/ab12",
        "ws://MACMINI:8787/w/ab12/ws"
    ));
    assert!(!same_workspace("macmini", "macmini/w/ab12"));
    assert!(!same_workspace("macmini/w/ab12", "macmini/w/cd34"));
}

#[test]
fn addresses_people_type_become_urls() {
    assert_eq!(server_url("macmini").unwrap().2, "ws://macmini:8787/ws");
    assert_eq!(
        updates_url("macmini").unwrap(),
        "http://macmini:8787/updates/appcast.xml"
    );
    assert_eq!(
        updates_url("ws://mac-mini.tail1.ts.net:9000/ws").unwrap(),
        "http://mac-mini.tail1.ts.net:9000/updates/appcast.xml"
    );
    assert_eq!(
        updates_url("[::1]:8787").unwrap(),
        "http://[::1]:8787/updates/appcast.xml"
    );
    assert!(updates_url("").is_err());
    assert_eq!(
        server_url(" 100.1.2.3:9000 ").unwrap(),
        ("100.1.2.3".into(), 9000, "ws://100.1.2.3:9000/ws".into())
    );
    assert_eq!(
        server_url("ws://mac-mini.tail1.ts.net/").unwrap().2,
        "ws://mac-mini.tail1.ts.net:8787/ws"
    );
    assert_eq!(server_url("http://h:1/ws").unwrap().2, "ws://h:1/ws");
    assert_eq!(server_url("[::1]:8787").unwrap().2, "ws://[::1]:8787/ws");
    assert!(server_url("").is_err());
    assert!(server_url("h:port").is_err() && server_url("https://h:port").is_err());

    // behind an HTTPS front (Cloudflare Tunnel): TLS on 443 unless said
    assert_eq!(
        server_url("https://wb.example.com").unwrap(),
        (
            "wb.example.com".into(),
            443,
            "wss://wb.example.com/ws".into()
        )
    );
    assert_eq!(
        server_url("WSS://wb.example.com/").unwrap().2,
        "wss://wb.example.com/ws"
    );
    assert_eq!(
        server_url("wss://wb.example.com:8443/ws").unwrap(),
        (
            "wb.example.com".into(),
            8443,
            "wss://wb.example.com:8443/ws".into()
        )
    );
    assert!(is_secure(&server_url("https://wb.example.com").unwrap().2));
    assert!(!is_secure(&server_url("macmini").unwrap().2));
    assert_eq!(
        updates_url("https://wb.example.com").unwrap(),
        "https://wb.example.com/updates/appcast.xml"
    );
    assert_eq!(
        updates_url("wss://wb.example.com:8443").unwrap(),
        "https://wb.example.com:8443/updates/appcast.xml"
    );
    assert!(server_url("h:port").is_err());
}

#[test]
fn a_call_identity_names_its_member() {
    assert_eq!(call_identity(42, "dev-1"), "42-dev-1");
    assert_eq!(identity_member(&call_identity(42, "dev-1")), Some(42));
    assert_eq!(identity_member("someone"), None);
}

#[test]
fn a_sign_in_becomes_its_hello_and_back_byte_for_byte() {
    let full = SignIn {
        device: "d1".into(),
        tag: Some(3),
        token: Some("t".into()),
        invite: Some("ABCD-EFGH-JKLM".into()),
        claim: Some(7),
        email: Some("a@b.co".into()),
        password: Some("pw".into()),
        new_password: Some("new pw".into()),
        reset_code: Some("R".into()),
        link_code: Some("K7Q2MX".into()),
        app: Some("0.1.7".into()),
        sync: Some(4),
        full_copies: false,
        watch: false,
    };
    let by_hand = Msg::Hello {
        device: "d1".into(),
        tag: Some(3),
        token: Some("t".into()),
        invite: Some("ABCD-EFGH-JKLM".into()),
        claim: Some(7),
        email: Some("a@b.co".into()),
        password: Some("pw".into()),
        new_password: Some("new pw".into()),
        reset_code: Some("R".into()),
        link_code: Some("K7Q2MX".into()),
        app: Some("0.1.7".into()),
        sync: Some(4),
        deltas: true,
        watch: false,
    };
    assert_eq!(Msg::from(&full).to_json(), by_hand.to_json());
    assert_eq!(
        Msg::from(&full).to_json(),
        r#"{"type":"hello","device":"d1","tag":3,"token":"t","invite":"ABCD-EFGH-JKLM","claim":7,"email":"a@b.co","password":"pw","new_password":"new pw","reset_code":"R","link_code":"K7Q2MX","app":"0.1.7","sync":4,"deltas":true}"#
    );
    let back = SignIn::try_from(Msg::parse(&by_hand.to_json()).unwrap()).unwrap();
    assert_eq!(back, full);
    // whole copies (no deltas) and nothing else: only the device goes
    let bare = SignIn {
        device: "d2".into(),
        full_copies: true,
        ..Default::default()
    };
    assert_eq!(
        Msg::from(&bare).to_json(),
        r#"{"type":"hello","device":"d2"}"#
    );
    assert_eq!(
        SignIn::try_from(Msg::parse(r#"{"type":"hello","device":"d2"}"#).unwrap()),
        Ok(bare)
    );
    assert_eq!(SignIn::try_from(Msg::Ping), Err(Msg::Ping));
}

#[test]
fn messages_round_trip_as_tagged_json() {
    let hello = |tag| Msg::Hello {
        device: "d1".into(),
        tag,
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
    };
    assert_eq!(
        hello(Some(3)).to_json(),
        r#"{"type":"hello","device":"d1","tag":3}"#
    );
    // the app's version goes along; an older server ignores it
    let mut h = hello(None);
    if let Msg::Hello { app, sync, .. } = &mut h {
        *app = Some("0.1.7".into());
        *sync = Some(3);
    }
    assert_eq!(
        h.to_json(),
        r#"{"type":"hello","device":"d1","app":"0.1.7","sync":3}"#
    );
    assert_eq!(
        Msg::parse(r#"{"type":"hello","device":"d1"}"#),
        Some(hello(None))
    );
    // an older server's welcome (no email) still reads
    assert!(matches!(
        Msg::parse(
            r#"{"type":"welcome","tag":1,"snapshot":null,"rev":0,"member":2,"role":"member"}"#
        ),
        Some(Msg::Welcome { email: None, .. })
    ));
    assert_eq!(Msg::parse(r#"{"type":"ping"}"#), Some(Msg::Ping));
    assert_eq!(
        Msg::parse(r#"{"type":"set_role","member":5,"role":"admin"}"#),
        Some(Msg::SetRole {
            member: 5,
            role: Role::Admin
        })
    );
    // what the iPhone app sends (ios/WorkboardKit Protocol.swift)
    let t = Msg::PushToken {
        token: "ab12".into(),
        platform: "ios".into(),
        env: "development".into(),
        workspace: None,
        prefs: None,
    };
    assert_eq!(
        Msg::parse(r#"{"type":"push_token","token":"ab12","platform":"ios","env":"development"}"#),
        Some(t.clone())
    );
    assert_eq!(Msg::parse(&t.to_json()), Some(t));
    // with the phone's name for the workspace (Protocol.swift sends it too)
    assert_eq!(
        Msg::parse(
            r#"{"type":"push_token","token":"ab12","platform":"ios","env":"production","workspace":"k1"}"#
        ),
        Some(Msg::PushToken {
            token: "ab12".into(),
            platform: "ios".into(),
            env: "production".into(),
            workspace: Some("k1".into()),
            prefs: None,
        })
    );
    // a watch connection: the hello says so, the server answers watching and notes
    let w = SignIn {
        device: "d3".into(),
        token: Some("t".into()),
        watch: true,
        full_copies: true,
        ..Default::default()
    };
    assert_eq!(
        Msg::from(&w).to_json(),
        r#"{"type":"hello","device":"d3","token":"t","watch":true}"#
    );
    assert_eq!(Msg::parse(r#"{"type":"watching"}"#), Some(Msg::Watching));
    assert_eq!(
        Msg::parse(r#"{"type":"note","title":"Ann","body":"hi","channel":20,"parent":11}"#),
        Some(Msg::Note {
            title: "Ann".into(),
            body: "hi".into(),
            channel: Some(20),
            parent: Some(11),
        })
    );
    // with the device's choices; missing ones take their defaults
    let Some(Msg::PushToken { prefs: Some(p), .. }) = Msg::parse(
        r#"{"type":"push_token","token":"ab12","prefs":{"channel_modes":[{"channel":7,"mode":"all"}],"keywords":["deploy"],"muted_until":5}}"#,
    ) else {
        panic!("prefs read")
    };
    assert!(p.direct && p.mentions && p.replies);
    assert_eq!(p.channel_mode(7), "all");
    assert_eq!(p.channel_mode(8), "mentions");
    assert_eq!(p.keywords, vec!["deploy"]);
    assert_eq!(p.muted_until, 5);
}

#[test]
fn invite_links_round_trip_and_read_from_pasted_mail() {
    let link = join_link(
        "mac-mini.tail1.ts.net:8787",
        "ABCD-EFGH-JKMN",
        Some("kim.minji+wb@company.com"),
    );
    assert_eq!(link, "workboard://join?server=mac-mini.tail1.ts.net%3A8787&code=ABCD-EFGH-JKMN&email=kim.minji%2Bwb%40company.com");
    let want = JoinLink {
        server: Some("mac-mini.tail1.ts.net:8787".into()),
        code: "ABCD-EFGH-JKMN".into(),
        email: Some("kim.minji+wb@company.com".into()),
    };
    assert_eq!(parse_join(&link), want);
    let mail = format!("Hi,\n\nOpen this: <{}>\nThanks", link);
    assert_eq!(parse_join(&mail), want);
    assert_eq!(
        parse_join("  abcd-efgh-jkmn "),
        JoinLink {
            code: "abcd-efgh-jkmn".into(),
            ..Default::default()
        }
    );
    assert_eq!(
        parse_join("workboard://join?code=X%2"),
        JoinLink {
            code: "X%2".into(),
            ..Default::default()
        }
    );
}

#[test]
fn backup_names_read_as_dates() {
    let b = |n: &str| {
        Backup {
            name: n.into(),
            bytes: 0,
        }
        .label()
    };
    assert_eq!(b("hourly-20261115-03.json"), "2026-11-15 03:00 (hourly)");
    assert_eq!(b("daily-20261115.json"), "2026-11-15 (daily)");
    assert_eq!(
        b("before-restore-20261115-031500.json"),
        "2026-11-15 03:15:00 (before a restore)"
    );
    assert_eq!(b("other.json"), "other.json");
}

#[test]
fn roles_order_from_member_to_owner() {
    assert!(Role::Owner.manages() && Role::Admin.manages() && !Role::Member.manages());
    assert!(Role::Owner > Role::Admin && Role::Admin > Role::Member);
}

#[test]
fn email_ok_ignores_surrounding_spaces_only() {
    assert!(email_ok("sam@example.com"));
    assert!(email_ok("  sam@example.com "));
    assert!(!email_ok("sam @example.com"));
    assert!(!email_ok("@example.com"));
    assert!(!email_ok("sam@example"));
    assert!(!email_ok("sam@.example.com"));
    assert!(!email_ok("sam@example.com."));
}

#[test]
fn a_put_back_refusal_reads_as_denied_to_older_apps() {
    let text = Msg::put_back("no").to_json();
    assert_eq!(
        text,
        r#"{"type":"error","message":"no","code":"denied","put_back":true}"#
    );
    // an app from before reads the same text as a plain refusal
    let old: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(old["code"], CODE_DENIED);
    assert_eq!(Msg::parse(&text), Some(Msg::put_back("no")));
}

#[test]
fn a_call_refusal_names_its_conversation_and_old_errors_still_read() {
    assert_eq!(
        Msg::call_denied("no", 5).to_json(),
        r#"{"type":"error","message":"no","code":"denied","channel":5}"#
    );
    assert_eq!(
        Msg::parse(r#"{"type":"error","message":"no","code":"denied"}"#),
        Some(Msg::error("no", Some(CODE_DENIED)))
    );
    assert_eq!(
        Msg::error("x", None).to_json(),
        r#"{"type":"error","message":"x"}"#
    );
}

#[test]
fn requests_wait_while_reconnecting_but_pings_and_pushes_do_not() {
    let mut q = vec![];
    queue(&mut q, Msg::CallJoin { channel: 1 });
    queue(&mut q, Msg::Ping);
    queue(&mut q, Msg::Push { data: Value::Null });
    queue(&mut q, Msg::BackupList);
    assert_eq!(q, vec![Msg::CallJoin { channel: 1 }, Msg::BackupList]);
    for _ in 0..MAX_QUEUED {
        queue(&mut q, Msg::AccessList);
    }
    assert_eq!(q.len(), MAX_QUEUED);
    assert_eq!(q[0], Msg::AccessList, "the oldest went first");
}

#[test]
fn a_failed_send_is_repeated_only_when_twice_does_no_harm() {
    let mut q = vec![Msg::AccessList];
    for m in [
        Msg::Revoke { member: 3 },
        Msg::SetRole {
            member: 3,
            role: Role::Admin,
        },
        Msg::CallJoin { channel: 1 },
        Msg::BackupList,
    ] {
        assert_eq!(send_failed(m.clone(), &mut q), None);
        assert_eq!(q[0], m, "goes again first thing after reconnecting");
    }
    let before = q.len();
    for m in [
        Msg::Restore {
            name: "daily-20261115.json".into(),
        },
        Msg::CreateInvite {
            member: 3,
            role: Role::Member,
        },
        Msg::SetPassword {
            email: "a@b.c".into(),
            password: "x".into(),
            old_password: None,
        },
    ] {
        assert_eq!(
            send_failed(m, &mut q),
            Some(Event::Denied(UNSURE_SEND.into()))
        );
    }
    assert_eq!(q.len(), before, "not sent again by itself");
    assert_eq!(send_failed(Msg::Ping, &mut q), None);
    assert_eq!(q.len(), before);
}
