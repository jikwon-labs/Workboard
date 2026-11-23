use super::*;

#[test]
fn email_invite_commit_failure_keeps_previous_workspace() {
    let home = std::env::temp_dir().join(format!("wb-invite-commit-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    let workspace = home.join(WORKSPACE_FILE);
    let state = home.join(STATE_FILE);
    let before = b"{\"workspace\": \"unchanged\"}\n";
    std::fs::write(&workspace, before).unwrap();
    // Both temporary writes succeed, but installing server.json cannot.
    std::fs::create_dir(&state).unwrap();
    let result = persist_email_invite(&home, &json!({"workspace": "candidate"}), &Saved::default());
    assert!(result.is_err());
    assert!(
        std::fs::read(&workspace).unwrap() == before,
        "failed commit must preserve the original workspace bytes"
    );
    assert!(state.is_dir());
    assert!(!workspace.with_extension("json.tmp").exists());
    assert!(!state.with_extension("json.tmp").exists());
    // Also inject the same fault *after* preparation, at the commit seam:
    // cover rollback of an existing workspace and of a not-yet-saved one.
    for existing in [true, false] {
        if !existing {
            std::fs::remove_file(&workspace).unwrap();
        }
        std::fs::write(workspace.with_extension("json.tmp"), b"candidate").unwrap();
        std::fs::write(state.with_extension("json.tmp"), b"candidate state").unwrap();
        let before = existing.then_some(before.as_slice());
        assert!(commit_email_invite(&workspace, &state, before).is_err());
        if existing {
            assert!(std::fs::read(&workspace).unwrap() == before.unwrap());
        } else {
            assert!(
                !workspace.exists(),
                "rollback must restore original absence"
            );
        }
        assert!(state.is_dir());
    }
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn email_invite_staging_failure_leaves_live_state_and_notifications_untouched() {
    for file in [STATE_FILE, WORKSPACE_FILE] {
        for reinvite in [false, true] {
            let home =
                std::env::temp_dir().join(format!("wb-invite-live-{:x}", wb_sync::random_u64()));
            std::fs::create_dir_all(home.join(file).with_extension("json.tmp")).unwrap();
            let canon = json!({
                "members": [{"id": 1, "role": "Owner"}, {"id": 2, "role": "Member", "email": "pending@example.test"}],
                "channels": [{"id": 10, "kind": "Channel", "name": "general", "members": []}],
                "extra": {"sync": {}}
            });
            let mut saved = Saved::default();
            saved.roles.insert(1, Role::Owner);
            saved.make_invite(2, Role::Member);
            let before_saved = serde_json::to_value(&saved).unwrap();
            let hub = test_hub(&home, Some(canon.clone()), saved);
            // Existing unsaved work must remain dirty after a failed invite.
            hub.inner.lock().unwrap().dirty = true;
            let mut updates = hub.updates.subscribe();
            let mut access = hub.access.subscribe();
            let email = if reinvite {
                "pending@example.test"
            } else {
                "new@example.test"
            };
            let res = hub.admin(&json!({"invite_email": email}));
            assert_eq!(res["ok"], false);
            assert!(res.get("code").is_none());
            let g = hub.inner.lock().unwrap();
            assert!(
                g.canon.as_deref() == Some(&canon),
                "live canon must not change"
            );
            assert!(
                serde_json::to_value(&g.saved).unwrap() == before_saved,
                "live invites, roles, revision and credentials must not change"
            );
            assert!(g.dirty, "do not forget unrelated pending writes");
            assert!(
                updates.try_recv().is_err(),
                "no canon notification on failure"
            );
            assert!(
                access.try_recv().is_err(),
                "no access/code notification on failure"
            );
            drop(g);
            std::fs::remove_dir_all(home).unwrap();
        }
    }
}

#[test]
fn email_invite_id_allocation_skips_items_tombs_and_every_saved_identity() {
    let mut canon = json!({"members": [{"id": 1}], "tasks": [{"id": 2, "activity": [{"id": 3}]}], "extra": {"sync": {"tombs": {"4": 1}, "stamps": {"5": 1}}}});
    let mut saved = Saved::default();
    saved.roles.insert(6, Role::Owner);
    saved.accounts.insert(
        7,
        Account {
            email: "reserved@example.test".into(),
            salt: String::new(),
            hash: String::new(),
            rounds: 1,
            created: 1,
        },
    );
    saved.devices.insert(
        "test".into(),
        Device {
            tag: 1,
            member: 8,
            token: String::new(),
            created: 1,
        },
    );
    saved.make_invite(9, Role::Admin);
    assert_eq!(
        fresh_member_id(&canon, &saved, 0),
        10,
        "all occupied candidate ids must be skipped"
    );
    assert_eq!(fresh_member_id(&canon, &saved, 2), 10);
    canon["extra"]["sync"]["revived"] = json!({"10": 1});
    assert_eq!(fresh_member_id(&canon, &saved, 10), 11);
    let id = fresh_member_id(&canon, &saved, u64::MAX);
    assert!(
        id > 0 && id < (1u64 << 62),
        "ids stay compatible with signed clients"
    );
}

fn dt(s: &str) -> chrono::NaiveDateTime {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()
}

#[test]
fn the_updates_route_serves_only_plain_update_files() {
    assert_eq!(update_content_type("appcast.xml"), Some("application/xml"));
    assert_eq!(
        update_content_type("Workboard-0.1.2-arm64.zip"),
        Some("application/zip")
    );
    assert_eq!(
        update_content_type("Workboard12-11.delta"),
        Some("application/octet-stream")
    );
    for bad in [
        "",
        ".appcast.xml",
        "../server.json",
        "a/b.zip",
        "workspace.json",
        "x.zip ",
        "%2e%2e.zip",
        "appcast",
    ] {
        assert_eq!(update_content_type(bad), None, "{:?}", bad);
    }
}

#[test]
fn downloads_pick_the_newest_version_by_number() {
    let names = [
        "appcast.xml",
        "Workboard-0.1.9-arm64.zip",
        "Workboard-0.1.10-arm64.zip",
        "Workboard-0.1.2-arm64.zip",
        "Workboard-0.2.0-windows-x64.zip",
        "Workboard-0.1.11-windows-x64.zip",
        "Workboard9-10.delta",
        "Workboard-latest-arm64.zip",
        "Workboard-0.9..1-arm64.zip",
        "Workboard--arm64.zip",
    ];
    let (mac, _) = download_suffix("mac").unwrap();
    let (win, _) = download_suffix("windows").unwrap();
    assert_eq!(
        latest_download(names, mac).as_deref(),
        Some("Workboard-0.1.10-arm64.zip")
    );
    assert_eq!(
        latest_download(names, win).as_deref(),
        Some("Workboard-0.2.0-windows-x64.zip")
    );
    assert_eq!(latest_download(["appcast.xml"], win), None);
    assert_eq!(latest_download(Vec::<String>::new(), mac), None);
    assert_eq!(download_suffix("linux"), None);
}

#[test]
fn backups_keep_48_hourly_and_30_daily_copies() {
    let home = std::env::temp_dir().join(format!("wb-backup-{:x}", wb_sync::random_u64()));
    let start = dt("2026-11-01 00:00");
    // 40 days of hourly changes
    for h in 0..(40 * 24) {
        backup(
            &home,
            &format!("{{\"v\":{}}}", h),
            start + chrono::Duration::hours(h),
        )
        .unwrap();
    }
    let dir = home.join(BACKUP_DIR);
    let hourly = backup_files(&dir, "hourly");
    let daily = backup_files(&dir, "daily");
    assert_eq!(hourly.len(), KEEP_HOURLY);
    assert_eq!(daily.len(), KEEP_DAILY);
    assert!(
        hourly.last().unwrap().ends_with("hourly-20261210-23.json"),
        "{:?}",
        hourly.last()
    );
    assert!(
        daily.first().unwrap().ends_with("daily-20261111.json"),
        "{:?}",
        daily.first()
    );
    // nothing changed: no new copy next hour
    let n = backup(
        &home,
        "{\"v\":959}",
        start + chrono::Duration::hours(40 * 24),
    )
    .unwrap();
    assert!(
        n.iter().all(|p| !p.to_string_lossy().contains("hourly")),
        "{:?}",
        n
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_restored_copy_wins_and_brings_deleted_items_back() {
    let mut meta = wb_sync::SyncMeta::default();
    let old = json!({"tasks": [{"id": 1, "name": "old name"}, {"id": 2, "name": "deleted later"}], "extra": {}});
    wb_sync::stamp(&old, &mut meta, 1000);
    let mut backup = old.clone();
    backup["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    // later: task 1 renamed, task 2 deleted
    let mut now = json!({"tasks": [{"id": 1, "name": "new name"}, {"id": 3, "name": "made later"}], "extra": {}});
    wb_sync::stamp(&now, &mut meta, 2000);
    now["extra"]["sync"] = serde_json::to_value(&meta).unwrap();
    mark_restored(&mut backup, 3000);
    wb_sync::merge(&mut backup, &[now]);
    let names: Vec<&str> = backup["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["old name", "deleted later", "made later"]);
}

#[test]
fn call_tokens_are_signed_livekit_jwts() {
    use hmac::{KeyInit, Mac};
    let cfg = LiveKitCfg {
        url: "ws://x:7880".into(),
        key: "APIkey".into(),
        secret: "s3cret-s3cret-s3cret-s3cret-s3cret".into(),
    };
    let t = call_token(&cfg, "wb-10", "2-dev1", "Bora", 1_000_000);
    let parts: Vec<&str> = t.split('.').collect();
    assert_eq!(parts.len(), 3);
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(cfg.secret.as_bytes()).unwrap();
    mac.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
    assert_eq!(
        parts[2],
        b64url(&mac.finalize().into_bytes()),
        "signature checks out"
    );
    assert!(
        !parts
            .iter()
            .any(|p| p.contains('=') || p.contains('+') || p.contains('/')),
        "base64url without padding"
    );
    // decode the claims back
    let dec = |s: &str| -> Vec<u8> {
        const T: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut bits = 0u32;
        let mut n = 0;
        let mut out = vec![];
        for c in s.chars() {
            bits = bits << 6 | T.find(c).unwrap() as u32;
            n += 6;
            if n >= 8 {
                n -= 8;
                out.push((bits >> n) as u8);
            }
        }
        out
    };
    let claims: Value = serde_json::from_slice(&dec(parts[1])).unwrap();
    assert_eq!(claims["iss"], "APIkey");
    assert_eq!(claims["sub"], "2-dev1");
    assert_eq!(claims["name"], "Bora");
    assert_eq!(claims["video"]["room"], "wb-10");
    assert_eq!(claims["video"]["roomJoin"], true);
    assert_eq!(claims["exp"], 1_000_000 + CALL_TOKEN_SECS);
}

#[test]
fn passwords_hash_with_pbkdf2_and_check() {
    // PBKDF2-HMAC-SHA256 test vectors (RFC 7914 section 11 / common sets)
    assert_eq!(
        hex(&pbkdf2_sha256(b"password", b"salt", 1)),
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
    );
    assert_eq!(
        hex(&pbkdf2_sha256(b"password", b"salt", 4096)),
        "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
    );
    let a = Account::new(" Kim@Company.com ", "correct horse");
    assert_eq!(a.email, "Kim@Company.com");
    assert!(a.check("correct horse") && !a.check("correct horsE") && !a.check(""));
    assert_ne!(
        Account::new("k@c.co", "correct horse").salt,
        a.salt,
        "every hash has its own salt"
    );
    let mut s = Saved::default();
    s.accounts.insert(4, a);
    assert_eq!(s.account_member("kim@company.COM"), Some(4));
    assert!(s
        .account_problem(5, "kim@company.com", "long enough")
        .unwrap()
        .contains("already"));
    assert!(s
        .account_problem(4, "kim@company.com", "short")
        .unwrap()
        .contains("at least 8"));
    assert!(s
        .account_problem(4, "not an email", "long enough")
        .is_some());
    assert_eq!(s.account_problem(4, "kim@company.com", "long enough"), None);
}

#[test]
fn invite_codes_work_once_and_ignore_case_and_dashes() {
    let mut s = Saved::default();
    let code = s.make_invite(7, Role::Admin);
    assert_eq!(code.len(), 14);
    let typed = code.to_lowercase().replace('-', " ");
    let inv = s.take_invite(&typed).expect("works");
    assert_eq!((inv.member, inv.role), (7, Role::Admin));
    assert!(s.take_invite(&code).is_none(), "only once");
    s.invites.insert(
        "OLDC-ODEX-XXXX".into(),
        InviteRec {
            member: 8,
            role: Role::Member,
            created: now_ms() - 8 * 86_400_000,
        },
    );
    assert!(s.take_invite("OLDC-ODEX-XXXX").is_none(), "expired");
}

#[test]
fn a_stage_one_state_file_keeps_its_revision() {
    let home = std::env::temp_dir().join(format!("wb-state-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join(STATE_FILE),
        r#"{"rev": 9, "devices": {"aaaa": 3}}"#,
    )
    .unwrap();
    let s = load_saved(&home).unwrap();
    assert_eq!(s.rev, 9);
    assert!(s.devices.is_empty());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn expired_invite_codes_are_not_shown() {
    let mut s = Saved::default();
    s.invites.insert(
        "OLDC-ODEX-XXXX".into(),
        InviteRec {
            member: 8,
            role: Role::Member,
            created: now_ms() - 8 * 86_400_000,
        },
    );
    let fresh = s.make_invite(9, Role::Member);
    let canon = json!({"members": [{"id": 8}, {"id": 9}]});
    let people = s.people(Some(&canon), Role::Owner);
    assert_eq!(people.iter().find(|p| p.member == 8).unwrap().invite, None);
    assert_eq!(
        people.iter().find(|p| p.member == 9).unwrap().invite,
        Some(fresh)
    );
    assert_eq!(s.open_invite(8, now_ms()), None);
}

#[test]
fn odd_copies_never_panic() {
    for mut v in [
        json!("x"),
        json!([1]),
        json!({"members": [{"id": 1}], "extra": 3}),
        json!({"members": [{"id": 1}], "extra": {"sync": "s"}}),
        json!({"members": [{"id": 1}], "extra": {"sync": {"stamps": []}}}),
    ] {
        assert!(!copy_shape_ok(&v));
        assert!(!label_role(&mut v, 1, Role::Owner));
        mark_restored(&mut v, 1);
    }
    let mut ok = json!({"members": [{"id": 1}]});
    assert!(label_role(&mut ok, 1, Role::Owner));
    assert_eq!(ok["members"][0]["role"], "Owner");
}

#[test]
fn a_huddle_lets_in_whoever_may_see_its_conversation() {
    let home = std::env::temp_dir().join(format!("wb-callrule-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join(LIVEKIT_FILE),
        json!({"url": "ws://x:7880", "key": "k", "secret": "s3cret-s3cret-s3cret-s3cret"})
            .to_string(),
    )
    .unwrap();
    // Cy (3) is on no list: a public channel with a list, one without, a
    // private channel and a DM between Ann and Bo
    let canon = json!({"members": [{"id": 1, "name": "Ann"}, {"id": 2}, {"id": 3}], "channels": [
        {"id": 10, "kind": "Channel", "members": [1, 2]},
        {"id": 11, "kind": "Channel", "members": []},
        {"id": 12, "kind": "Channel", "private": true, "members": [1, 2]},
        {"id": 13, "kind": "Direct", "members": [1, 2]},
    ]});
    let hub = test_hub(&home, Some(canon), Saved::default());
    let gets_in = |member, ch| matches!(hub.call(member, "dev", ch), Ok(Msg::CallToken { .. }));
    assert!(gets_in(3, 10), "a public channel is everyone's");
    assert!(gets_in(3, 11));
    for ch in [12, 13] {
        assert!(!gets_in(3, ch), "not in private conversation {ch}");
        assert!(gets_in(1, ch), "Ann is in {ch}");
    }
    let _ = std::fs::remove_dir_all(&home);
}

fn test_hub(home: &Path, canon: Option<Value>, saved: Saved) -> Hub {
    Hub {
        home: home.to_path_buf(),
        root: home.to_path_buf(),
        id: None,
        persistence: Mutex::new(()),
        inner: Mutex::new(Inner {
            canon: canon.map(Arc::new),
            saved,
            dirty: false,
            mine_dirty: false,
            canon_due: None,
            resets: Default::default(),
            mailed: Default::default(),
            fails: Default::default(),
            links: Default::default(),
            link_fails: (0, 0),
            summary: None,
            backed_up: None,
            catchup: Default::default(),
        }),
        updates: broadcast::channel(64).0,
        kicks: broadcast::channel(16).0,
        notes: broadcast::channel(64).0,
        access: broadcast::channel(16).0,
        stop: watch::channel(false).1,
        connections: AtomicUsize::new(0),
        hashing: AtomicUsize::new(0),
        max_file: MAX_FILE,
        claim_wait: Duration::from_secs(CLAIM_WAIT_SECS),
        pusher: None,
        poster: Arc::new(hooks::Curl),
        ai: None,
        ai_allowance: Default::default(),
        online: Mutex::new(Default::default()),
        watching: Mutex::new(Default::default()),
        members_online: Mutex::new(Default::default()),
        presence: watch::channel(vec![]).0,
        typing: broadcast::channel(64).0,
        mines: broadcast::channel(64).0,
        closed: watch::channel(false).0,
        epoch: 1,
        seen: Mutex::new(Default::default()),
        read_pending: Mutex::new(Default::default()),
        read_quiet: Duration::from_secs(10),
    }
}

fn hello(device: &str) -> Hello {
    Hello {
        device: device.into(),
        ..Default::default()
    }
}

#[test]
fn wrong_tries_are_forgotten_and_never_pile_up() {
    let mut g = Inner {
        canon: None,
        saved: Saved::default(),
        dirty: false,
        mine_dirty: false,
        canon_due: None,
        resets: Default::default(),
        mailed: Default::default(),
        fails: Default::default(),
        links: Default::default(),
        link_fails: (0, 0),
        summary: None,
        backed_up: None,
        catchup: Default::default(),
    };
    let now = now_ms();
    // old tries no longer count and go
    g.fails
        .insert("old@x.io".into(), (3, now - LOCK_MINUTES * 60_000 - 1));
    for _ in 0..MAX_TRIES {
        g.failed("locked@x.io");
    }
    g.prune_fails(now);
    assert!(!g.fails.contains_key("old@x.io"));
    assert!(g.locked("locked@x.io").is_some());
    // a real account one try short of locked
    g.saved
        .accounts
        .insert(2, Account::new("bo@team.io", "bo-secret-22"));
    for _ in 0..MAX_TRIES - 1 {
        g.failed("bo@team.io");
    }
    // many different emails: the map stays bounded and the locked one stays locked
    for i in 0..(MAX_FAIL_ENTRIES + 500) {
        g.failed(&format!("guess{}@x.io", i));
    }
    assert_eq!(
        g.fails.get("bo@team.io").map(|f| f.0),
        Some(MAX_TRIES - 1),
        "made-up emails don't push a real account's tries out"
    );
    assert!(g.fails.len() <= MAX_FAIL_ENTRIES, "{}", g.fails.len());
    assert!(
        g.locked("locked@x.io").is_some(),
        "flooding doesn't unlock an email"
    );
    assert!(
        g.fails
            .contains_key(&format!("guess{}@x.io", MAX_FAIL_ENTRIES + 499)),
        "the newest tries are kept"
    );
}

#[test]
fn password_checks_dont_hold_up_everyone_else() {
    let home = std::env::temp_dir().join(format!("wb-lock-{:x}", wb_sync::random_u64()));
    let mut saved = Saved::default();
    saved
        .accounts
        .insert(2, Account::new("bo@team.io", "bo-secret-22"));
    let hub = Arc::new(test_hub(
        &home,
        Some(json!({"members": [{"id": 1}, {"id": 2}]})),
        saved,
    ));
    let t = std::time::Instant::now();
    let _ = Account::new("x@x.x", "timing it");
    let one_hash = t.elapsed();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // people trying passwords all the time (different emails, so none is locked)
    let workers: Vec<_> = (0..4)
        .map(|w| {
            let (hub, stop) = (hub.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut i = 0;
                while !stop.load(Ordering::Relaxed) {
                    let h = Hello {
                        email: Some(format!("try{}-{}@team.io", w, i)),
                        password: Some("nope-nope".into()),
                        ..hello("d")
                    };
                    assert!(hub.sign_in(&h).is_err());
                    i += 1;
                }
            })
        })
        .collect();
    // meanwhile sync takes the lock: it doesn't wait for their hashing
    let mut waits = vec![];
    let until = std::time::Instant::now() + Duration::from_millis(600).max(one_hash * 20);
    while std::time::Instant::now() < until {
        let t = std::time::Instant::now();
        drop(hub.inner.lock().unwrap());
        waits.push(t.elapsed());
        std::thread::sleep(Duration::from_millis(2));
    }
    stop.store(true, Ordering::Relaxed);
    for w in workers {
        w.join().unwrap();
    }
    waits.sort();
    let median = waits[waits.len() / 2];
    assert!(
        median < one_hash / 4,
        "median wait {:?}, one hash {:?}",
        median,
        one_hash
    );
    // and the right password still signs in, the wrong one doesn't
    assert_eq!(
        hub.sign_in(&Hello {
            email: Some("Bo@team.io".into()),
            password: Some("bo-secret-22".into()),
            ..hello("phone")
        })
        .map(|r| r.0),
        Ok(2)
    );
    assert!(hub
        .sign_in(&Hello {
            email: Some("bo@team.io".into()),
            password: Some("bo-secret-23".into()),
            ..hello("phone")
        })
        .is_err());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn tries_made_at_once_still_stop_at_the_limit() {
    let home = std::env::temp_dir().join(format!("wb-limit-{:x}", wb_sync::random_u64()));
    let mut saved = Saved::default();
    saved
        .accounts
        .insert(2, Account::new("bo@team.io", "bo-secret-22"));
    let hub = Arc::new(test_hub(
        &home,
        Some(json!({"members": [{"id": 2}]})),
        saved,
    ));
    let guesses: Vec<_> = (0..20)
        .map(|i| {
            let hub = hub.clone();
            std::thread::spawn(move || {
                hub.sign_in(&Hello {
                    email: Some("bo@team.io".into()),
                    password: Some(format!("guess-{}", i)),
                    ..hello("d")
                })
                .unwrap_err()
            })
        })
        .collect();
    let answers: Vec<String> = guesses.into_iter().map(|g| g.join().unwrap()).collect();
    // (some are turned away while the server is busy hashing: not checked)
    let checked = answers
        .iter()
        .filter(|a| !a.contains("Too many") && !a.contains("busy"))
        .count();
    assert!(
        checked <= MAX_TRIES as usize,
        "{} guesses were checked",
        checked
    );
    // more wrong ones one at a time, in case many were turned away
    for i in 0..MAX_TRIES {
        let _ = hub.sign_in(&Hello {
            email: Some("bo@team.io".into()),
            password: Some(format!("later-{}", i)),
            ..hello("d")
        });
    }
    let right = Hello {
        email: Some("bo@team.io".into()),
        password: Some("bo-secret-22".into()),
        ..hello("d")
    };
    assert!(hub.sign_in(&right).unwrap_err().contains("Too many"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_server_that_had_a_workspace_is_never_claimed_again() {
    let home = std::env::temp_dir().join(format!("wb-claimed-{:x}", wb_sync::random_u64()));
    let claim = |device: &str| Hello {
        claim: Some(9),
        ..hello(device)
    };
    // workspace.json is gone but server.json says there was one (rev > 0)
    let mut saved = Saved {
        rev: 12,
        ..Default::default()
    };
    saved.devices.insert(
        "owner-mac".into(),
        Device {
            tag: 1,
            member: 1,
            token: hash_token("t"),
            created: now_ms() - 3_600_000,
        },
    );
    saved.roles.insert(1, Role::Owner);
    let hub = test_hub(&home, None, saved);
    for device in ["stranger", "owner-mac"] {
        assert!(
            hub.sign_in(&claim(device)).unwrap_err().contains("restore"),
            "{}",
            device
        );
    }
    assert_eq!(
        hub.inner.lock().unwrap().saved.role_of(1),
        Role::Owner,
        "nothing was cleared"
    );
    // even with nobody's computer left on it
    hub.inner.lock().unwrap().saved.devices.clear();
    assert!(hub.sign_in(&claim("stranger")).is_err());
    // the owner's way back: a restore brings the workspace, then the token works
    hub.inner.lock().unwrap().saved.devices.insert(
        "owner-mac".into(),
        Device {
            tag: 1,
            member: 1,
            token: hash_token("t"),
            created: 0,
        },
    );
    assert_eq!(
        hub.sign_in(&Hello {
            token: Some("t".into()),
            ..hello("owner-mac")
        })
        .map(|r| r.0),
        Ok(1)
    );

    // a claim that never brought its workspace (rev 0) gives way after claim_wait
    let mut saved = Saved::default();
    saved.devices.insert(
        "quit-app".into(),
        Device {
            tag: 1,
            member: 1,
            token: hash_token("t"),
            created: now_ms() - (CLAIM_WAIT_SECS as i64 + 1) * 1000,
        },
    );
    saved.roles.insert(1, Role::Owner);
    let hub = test_hub(&home, None, saved);
    assert_eq!(
        hub.sign_in(&claim("stranger")).map(|r| (r.0, r.1)),
        Ok((9, Role::Owner))
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn admins_make_codes_only_for_new_members_and_codes_never_replace_a_password() {
    let home = std::env::temp_dir().join(format!("wb-codes-{:x}", wb_sync::random_u64()));
    let canon = json!({"members": [{"id": 1}, {"id": 2}, {"id": 3}, {"id": 4, "email": "di@team.io"}, {"id": 5}]});
    let mut saved = Saved::default();
    saved.set_role(1, Role::Owner);
    saved.set_role(2, Role::Admin);
    saved.set_role(5, Role::Admin);
    saved
        .accounts
        .insert(3, Account::new("cy@team.io", "cy-secret-33"));
    let hub = test_hub(&home, Some(canon), saved);
    let ask = |from: u64, who: u64| match hub.request(
        from,
        EVERYONE,
        Msg::CreateInvite {
            member: who,
            role: Role::Member,
        },
    ) {
        Reply::To(Msg::Invite { code, .. }) => Ok(code),
        Reply::To(_) => Err("other answer".to_string()),
        Reply::Denied(why) => Err(why),
    };
    // admin 2: not for the owner, another admin, or someone with a password
    assert!(ask(2, 1).unwrap_err().contains("Only the owner"));
    assert!(ask(2, 5).unwrap_err().contains("Only the owner"));
    assert!(ask(2, 3).unwrap_err().contains("Only the owner"));
    // a new member without an account: yes
    let code4 = ask(2, 4).unwrap();
    // the owner may make any of them
    let code3 = ask(1, 3).unwrap();
    assert!(ask(1, 5).is_ok());
    // admins don't see the codes they couldn't make
    let people = {
        let g = hub.inner.lock().unwrap();
        g.saved.people(g.canon.as_deref(), Role::Admin)
    };
    let shown = |m: u64| {
        people
            .iter()
            .find(|p| p.member == m)
            .unwrap()
            .invite
            .is_some()
    };
    assert!(shown(4) && !shown(3) && !shown(5));

    // redeeming a code for someone with a password signs in but keeps the password
    let join = |code: &str, pw: &str| Hello {
        invite: Some(code.into()),
        new_password: Some(pw.into()),
        ..hello("new-mac")
    };
    assert_eq!(
        hub.sign_in(&join(&code3, "taken-over-1")).map(|r| r.0),
        Ok(3)
    );
    let cy = |pw: &str| Hello {
        email: Some("cy@team.io".into()),
        password: Some(pw.into()),
        ..hello("phone")
    };
    assert!(hub.sign_in(&cy("taken-over-1")).is_err());
    assert_eq!(hub.sign_in(&cy("cy-secret-33")).map(|r| r.0), Ok(3));
    // a new member's code still sets up their account
    assert_eq!(
        hub.sign_in(&join(&code4, "di-secret-44")).map(|r| r.0),
        Ok(4)
    );
    assert_eq!(
        hub.inner.lock().unwrap().saved.accounts[&4].email,
        "di@team.io"
    );

    // once someone has come in, an admin can't make a code that signs in
    // as them, even without a password or after taking their access away
    // (which deletes the password)
    assert!(ask(2, 4).unwrap_err().contains("Only the owner"));
    assert!(matches!(
        hub.request(2, EVERYONE, Msg::Revoke { member: 4 }),
        Reply::To(_)
    ));
    assert!(ask(2, 4).unwrap_err().contains("Only the owner"));
    assert!(ask(1, 4).is_ok());
    // an admin code made earlier doesn't bring back a role taken away
    let admin_code = match hub.request(
        1,
        EVERYONE,
        Msg::CreateInvite {
            member: 4,
            role: Role::Admin,
        },
    ) {
        Reply::To(Msg::Invite { code, .. }) => code,
        _ => panic!("the owner makes an admin code"),
    };
    hub.request(
        1,
        EVERYONE,
        Msg::SetRole {
            member: 4,
            role: Role::Member,
        },
    );
    assert!(hub
        .sign_in(&Hello {
            invite: Some(admin_code),
            ..hello("later")
        })
        .unwrap_err()
        .contains("invite code doesn't work"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn files_nothing_names_go_after_a_while_but_not_while_a_backup_names_them() {
    let home = std::env::temp_dir().join(format!("wb-gc-{:x}", wb_sync::random_u64()));
    let dir = home.join(FILES_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(home.join(BACKUP_DIR)).unwrap();
    let name = |c: char| std::iter::repeat(c).take(64).collect::<String>();
    let now = std::time::SystemTime::now();
    let old = now - Duration::from_secs((FILE_KEEP_DAYS + 1) * 86_400);
    let put = |n: &str, when: std::time::SystemTime| {
        std::fs::write(dir.join(n), b"x").unwrap();
        std::fs::write(dir.join(format!("{}.type", n)), "image/png").unwrap();
        std::fs::File::options()
            .write(true)
            .open(dir.join(n))
            .unwrap()
            .set_modified(when)
            .unwrap();
    };
    let (in_canon, in_backup, orphan, recent) = (name('a'), name('b'), name('c'), name('d'));
    for n in [&in_canon, &in_backup, &orphan] {
        put(n, old);
    }
    put(&recent, now);
    std::fs::write(dir.join(".tmp-0011"), b"part").unwrap();
    std::fs::File::options()
        .write(true)
        .open(dir.join(".tmp-0011"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    let canon = json!({"channels": [{"messages": [{"attachments": [{"file": in_canon, "name": "a.png"}]}]}]}).to_string();
    std::fs::write(
        home.join(BACKUP_DIR).join("daily-20261101.json"),
        json!({"tasks": [{"attachments": [{"file": in_backup}]}]}).to_string(),
    )
    .unwrap();

    // a server that has been cleaning up already (none of these was named lately)
    std::fs::write(dir.join(files::NAMED_FILE), "{}").unwrap();
    // no team copy: nothing goes
    assert!(collect_garbage(&home, None, now).is_empty());
    assert_eq!(
        collect_garbage(&home, Some(&canon), now),
        vec![orphan.clone()]
    );
    assert!(!dir.join(&orphan).exists() && !dir.join(format!("{}.type", orphan)).exists());
    assert!(
        !dir.join(".tmp-0011").exists(),
        "an old broken upload goes too"
    );
    for n in [&in_canon, &in_backup, &recent] {
        assert!(
            dir.join(n).exists() && dir.join(format!("{}.type", n)).exists(),
            "{} stays",
            &n[..4]
        );
    }
    // once the backup that named it is gone, it goes too, FILE_KEEP_DAYS
    // after it was last named
    std::fs::remove_file(home.join(BACKUP_DIR).join("daily-20261101.json")).unwrap();
    assert!(collect_garbage(&home, Some(&canon), now).is_empty());
    let later = now + Duration::from_secs(FILE_KEEP_DAYS * 86_400);
    let mut gone = collect_garbage(&home, Some(&canon), later);
    gone.sort();
    // (by then the recent upload nothing named is old enough too)
    assert_eq!(gone, vec![in_backup.clone(), recent.clone()]);
    assert!(dir.join(&in_canon).exists());
    // a backup that can't be read: nothing is deleted
    put(&orphan, old);
    std::fs::create_dir_all(home.join(BACKUP_DIR).join("odd.json")).unwrap();
    assert!(collect_garbage(&home, Some(&canon), later).is_empty());
    assert!(dir.join(&orphan).exists());
    let _ = std::fs::remove_dir_all(&home);
}

/// A message deleted to someone's Trash (which the server never sees)
/// keeps its file on the server for FILE_KEEP_DAYS after the team's copy
/// last named it, however old the file is; and a server's first clean
/// up deletes nothing it hasn't watched for that long.
#[test]
fn a_file_stays_a_while_after_its_message_is_deleted() {
    let home = std::env::temp_dir().join(format!("wb-gc2-{:x}", wb_sync::random_u64()));
    let dir = home.join(FILES_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    let name = |c: char| std::iter::repeat(c).take(64).collect::<String>();
    let now = std::time::SystemTime::now();
    let day = Duration::from_secs(86_400);
    let old = now - day * (FILE_KEEP_DAYS as u32 + 100);
    let (pic, orphan) = (name('e'), name('f'));
    for n in [&pic, &orphan] {
        std::fs::write(dir.join(n), b"x").unwrap();
        std::fs::File::options()
            .write(true)
            .open(dir.join(n))
            .unwrap()
            .set_modified(old)
            .unwrap();
    }
    let with = json!({"channels": [{"messages": [{"attachments": [{"file": pic}]}]}]}).to_string();
    let without = json!({"channels": [{"messages": []}]}).to_string();
    // the first clean up only starts watching
    assert!(collect_garbage(&home, Some(&with), now).is_empty());
    assert!(dir.join(files::NAMED_FILE).is_file());
    // the message is deleted: the file stays while it was named lately
    let soon = now + day * 3;
    assert!(collect_garbage(&home, Some(&without), soon).is_empty());
    assert!(dir.join(&pic).exists());
    let mut late = collect_garbage(&home, Some(&without), now + day * FILE_KEEP_DAYS as u32);
    late.sort();
    assert_eq!(late, vec![pic.clone(), orphan.clone()]);
    let record = std::fs::read_to_string(dir.join(files::NAMED_FILE)).unwrap();
    assert!(!record.contains(&pic), "forgotten once gone: {}", record);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn only_a_wrong_current_password_counts_as_a_wrong_try() {
    let home = std::env::temp_dir().join(format!("wb-setpw-{:x}", wb_sync::random_u64()));
    let mut saved = Saved::default();
    saved
        .accounts
        .insert(2, Account::new("bo@team.io", "bo-secret-22"));
    saved
        .accounts
        .insert(3, Account::new("cy@team.io", "cy-secret-33"));
    saved.add_device("mac", 2, None);
    let hub = test_hub(
        &home,
        Some(json!({"members": [{"id": 2}, {"id": 3}]})),
        saved,
    );
    let tries = |hub: &Hub| {
        hub.inner
            .lock()
            .unwrap()
            .fails
            .get("bo@team.io")
            .map(|f| f.0)
    };
    // too short, someone else's email: said, not counted
    for _ in 0..MAX_TRIES + 2 {
        assert!(hub
            .set_password(2, "mac", "bo@team.io", "short", Some("bo-secret-22"))
            .unwrap_err()
            .contains("at least"));
        assert!(hub
            .set_password(
                2,
                "mac",
                "cy@team.io",
                "long-enough-1",
                Some("bo-secret-22")
            )
            .is_err());
    }
    assert_eq!(tries(&hub), None);
    // a wrong current password counts
    assert!(hub
        .set_password(2, "mac", "bo@team.io", "long-enough-1", Some("wrong"))
        .unwrap_err()
        .contains("current password"));
    assert_eq!(tries(&hub), Some(1));
    // the right one changes it and forgets the wrong tries
    assert!(hub
        .set_password(
            2,
            "mac",
            "bo@team.io",
            "long-enough-1",
            Some("bo-secret-22")
        )
        .is_ok());
    assert_eq!(tries(&hub), None);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn reset_codes_guessed_at_once_still_stop_at_the_limit() {
    let home = std::env::temp_dir().join(format!("wb-reset-{:x}", wb_sync::random_u64()));
    let mut saved = Saved::default();
    saved
        .accounts
        .insert(2, Account::new("bo@team.io", "bo-secret-22"));
    let hub = Arc::new(test_hub(
        &home,
        Some(json!({"members": [{"id": 2}]})),
        saved,
    ));
    hub.inner
        .lock()
        .unwrap()
        .resets
        .insert("bo@team.io".into(), ("ABCD-EFGH-JKMN".into(), now_ms()));
    let guess = |code: String, pw: &str| Hello {
        email: Some("bo@team.io".into()),
        reset_code: Some(code),
        new_password: Some(pw.into()),
        ..hello("d")
    };
    // a password that won't do isn't a wrong try
    for _ in 0..MAX_TRIES + 2 {
        assert!(hub
            .sign_in(&guess("ZZZZ-ZZZZ-ZZZZ".into(), "short"))
            .unwrap_err()
            .contains("at least"));
    }
    assert!(hub.inner.lock().unwrap().fails.is_empty());
    let guesses: Vec<_> = (0..20)
        .map(|i| {
            let hub = hub.clone();
            let g = guess(format!("WXYZ-WXYZ-{:04}", i), "long-enough-1");
            std::thread::spawn(move || hub.sign_in(&g).unwrap_err())
        })
        .collect();
    let answers: Vec<String> = guesses.into_iter().map(|g| g.join().unwrap()).collect();
    // (some are turned away while the server is busy hashing: not checked)
    let checked = answers
        .iter()
        .filter(|a| !a.contains("Too many") && !a.contains("busy"))
        .count();
    assert!(
        checked <= MAX_TRIES as usize,
        "{} guesses were checked",
        checked
    );
    for i in 0..MAX_TRIES {
        let _ = hub.sign_in(&guess(format!("WXYZ-LATE-{:04}", i), "long-enough-1"));
    }
    assert!(hub
        .sign_in(&guess("ABCD-EFGH-JKMN".into(), "long-enough-1"))
        .unwrap_err()
        .contains("Too many"));
    // wrong passwords don't stop a reset, nor wrong codes signing in
    assert!(hub.inner.lock().unwrap().locked("bo@team.io").is_none());
    let _ = std::fs::remove_dir_all(&home);
}

#[cfg(unix)]
#[test]
fn saved_files_are_for_this_user_only() {
    use std::os::unix::fs::PermissionsExt;
    let home = std::env::temp_dir().join(format!("wb-mode-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    let path = home.join(WORKSPACE_FILE);
    // an old tmp file left readable by everyone
    std::fs::write(path.with_extension("json.tmp"), "old").unwrap();
    std::fs::set_permissions(
        path.with_extension("json.tmp"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    write_atomic(&path, "{}").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    assert!(!path.with_extension("json.tmp").exists());
    // replacing it keeps it whole and private, and leaves no temp file
    write_atomic(&path, b"{\"a\":1}").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":1}");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert!(!path.with_extension("json.tmp").exists());
    // the files folder's dot-file stages next to itself the same way
    assert_eq!(
        tmp_path(&home.join(files::NAMED_FILE)),
        home.join(".named.json.tmp")
    );
    let _ = std::fs::remove_dir_all(&home);
}

// Unix file modes; Windows keeps a user's files private through the folder
#[cfg(unix)]
#[test]
fn an_email_invite_saves_both_files_for_this_user_only() {
    use std::os::unix::fs::PermissionsExt;
    let home = std::env::temp_dir().join(format!("wb-invmode-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    persist_email_invite(&home, &json!({"members": []}), &Saved::default()).unwrap();
    for file in [WORKSPACE_FILE, STATE_FILE] {
        let path = home.join(file);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{file}");
        assert!(!path.with_extension("json.tmp").exists(), "{file}");
    }
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_request_is_signed_in_only_with_the_devices_own_token() {
    use axum::http::{HeaderMap, HeaderValue};
    let mut saved = Saved::default();
    saved.devices.insert(
        "mac-1".into(),
        Device {
            tag: 1,
            member: 2,
            token: hash_token("right"),
            created: 0,
        },
    );
    let auth = |v: &str| {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, HeaderValue::from_str(v).unwrap());
        h
    };
    assert_eq!(
        bearer(&auth("Bearer mac-1:right")),
        Some(("mac-1", "right"))
    );
    assert_eq!(bearer(&auth("Bearer mac-1")), None);
    assert_eq!(bearer(&auth("Basic mac-1:right")), None);
    assert_eq!(bearer(&HeaderMap::new()), None);
    assert!(saved.device_with("mac-1", "right").is_some());
    assert!(saved.device_with("mac-1", "wrong").is_none());
    assert!(saved.device_with("mac-1", "").is_none());
    assert!(saved.device_with("mac-2", "right").is_none());
}

#[test]
fn a_new_password_signs_out_the_persons_other_devices() {
    let home = std::env::temp_dir().join(format!("wb-pwout-{:x}", wb_sync::random_u64()));
    let mut saved = Saved::default();
    saved
        .accounts
        .insert(2, Account::new("bo@team.io", "bo-secret-22"));
    let hub = test_hub(
        &home,
        Some(json!({"members": [{"id": 2}, {"id": 3}]})),
        saved,
    );
    let mut kicks = hub.kicks.subscribe();
    let devices = |hub: &Hub| -> Vec<String> {
        let g = hub.inner.lock().unwrap();
        g.saved.devices.keys().cloned().collect()
    };
    {
        let mut g = hub.inner.lock().unwrap();
        for (d, m) in [("mac", 2), ("stolen", 2), ("cy-mac", 3)] {
            g.saved.add_device(d, m, None);
        }
    }
    // Bo changes the password on the Mac: the other device of Bo's is out
    assert!(hub
        .set_password(2, "mac", "bo@team.io", "bo-secret-23", Some("bo-secret-22"))
        .is_ok());
    assert_eq!(devices(&hub), ["cy-mac", "mac"]);
    assert_eq!(*kicks.try_recv().unwrap(), ["stolen".to_string()]);
    // a reset from a new phone signs out the Mac too
    hub.inner
        .lock()
        .unwrap()
        .resets
        .insert("bo@team.io".into(), ("ABCD-EFGH-JKMN".into(), now_ms()));
    assert!(hub
        .sign_in(&Hello {
            email: Some("bo@team.io".into()),
            reset_code: Some("ABCD-EFGH-JKMN".into()),
            new_password: Some("bo-secret-24".into()),
            ..hello("phone")
        })
        .is_ok());
    assert_eq!(devices(&hub), ["cy-mac", "phone"]);
    assert_eq!(*kicks.try_recv().unwrap(), ["mac".to_string()]);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn setting_a_password_ends_codes_made_for_that_person() {
    let home = std::env::temp_dir().join(format!("wb-pwcode-{:x}", wb_sync::random_u64()));
    let mut saved = Saved::default();
    saved.add_device("mac", 2, None);
    let hub = test_hub(
        &home,
        Some(json!({"members": [{"id": 2}, {"id": 3}]})),
        saved,
    );
    let (bo, cy) = {
        let mut g = hub.inner.lock().unwrap();
        (
            g.saved.make_invite(2, Role::Member),
            g.saved.make_invite(3, Role::Member),
        )
    };
    // Bo signs in with an admin's code on one computer and sets a password
    assert!(hub
        .set_password(2, "mac", "bo@team.io", "bo-secret-22", None)
        .is_ok());
    {
        let g = hub.inner.lock().unwrap();
        assert!(
            !g.saved.invites.contains_key(&bo),
            "Bo's code no longer works"
        );
        assert!(g.saved.invites.contains_key(&cy), "Cy's still does");
    }
    assert!(hub
        .sign_in(&Hello {
            invite: Some(bo.clone()),
            ..hello("other")
        })
        .unwrap_err()
        .contains("invite code doesn't work"));
    // the same after changing it, or a reset
    let again = hub.inner.lock().unwrap().saved.make_invite(2, Role::Member);
    assert!(hub
        .set_password(2, "mac", "bo@team.io", "bo-secret-23", Some("bo-secret-22"))
        .is_ok());
    assert!(!hub.inner.lock().unwrap().saved.invites.contains_key(&again));
    let third = {
        let mut g = hub.inner.lock().unwrap();
        g.resets
            .insert("bo@team.io".into(), ("ABCD-EFGH-JKMN".into(), now_ms()));
        g.saved.make_invite(2, Role::Member)
    };
    assert!(hub
        .sign_in(&Hello {
            email: Some("bo@team.io".into()),
            reset_code: Some("ABCD-EFGH-JKMN".into()),
            new_password: Some("bo-secret-24".into()),
            ..hello("phone")
        })
        .is_ok());
    assert!(!hub.inner.lock().unwrap().saved.invites.contains_key(&third));
    let _ = std::fs::remove_dir_all(&home);
}

/// A record of what was named lately that got cut short (it was being
/// written when the power went) deletes nothing: every file's clock
/// starts again, and the record is whole again afterwards.
#[test]
fn a_broken_named_record_deletes_nothing_and_starts_the_clock_again() {
    let home = std::env::temp_dir().join(format!("wb-gc3-{:x}", wb_sync::random_u64()));
    let dir = home.join(FILES_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    let name = |c: char| std::iter::repeat(c).take(64).collect::<String>();
    let now = std::time::SystemTime::now();
    let day = Duration::from_secs(86_400);
    let old = now - day * (FILE_KEEP_DAYS as u32 + 100);
    let trashed = name('a');
    std::fs::write(dir.join(&trashed), b"x").unwrap();
    std::fs::File::options()
        .write(true)
        .open(dir.join(&trashed))
        .unwrap()
        .set_modified(old)
        .unwrap();
    let without = json!({"channels": [{"messages": []}]}).to_string();
    for broken in ["", "{\"aaaa", "\0\0\0\0"] {
        std::fs::write(dir.join(files::NAMED_FILE), broken).unwrap();
        assert!(
            collect_garbage(&home, Some(&without), now).is_empty(),
            "nothing goes with a record of {:?}",
            broken
        );
        assert!(dir.join(&trashed).exists());
        let record = std::fs::read_to_string(dir.join(files::NAMED_FILE)).unwrap();
        let map: BTreeMap<String, i64> = serde_json::from_str(&record).unwrap();
        assert!(map.contains_key(&trashed), "rebuilt: {}", record);
        assert!(!dir.join(format!("{}.tmp", files::NAMED_FILE)).exists());
    }
    // the clock started at `now`: still here a while later, gone after
    // FILE_KEEP_DAYS
    assert!(collect_garbage(&home, Some(&without), now + day * 10).is_empty());
    assert_eq!(
        collect_garbage(&home, Some(&without), now + day * FILE_KEEP_DAYS as u32),
        vec![trashed.clone()]
    );
    let _ = std::fs::remove_dir_all(&home);
}

/// Changes to the team's copy come several a second while people work, and
/// each write is the whole copy: it's written once they stop for a moment
/// (or a while after the first one, while they go on). The first copy and
/// stopping write at once, and so do other changes (people, devices).
#[test]
fn the_team_copy_is_written_once_changes_settle() {
    use std::time::Instant;
    let home = std::env::temp_dir().join(format!("wb-settle-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    let path = home.join(WORKSPACE_FILE);
    let hub = test_hub(&home, Some(json!({"workspace": "one"})), Saved::default());
    let change = |name: &str| {
        let mut g = hub.inner.lock().unwrap();
        g.canon.as_mut().map(Arc::make_mut).unwrap()["workspace"] = json!(name);
        hub.broadcast_canon(&mut g, false);
    };
    let on_disk = || {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .map(|v| v["workspace"].as_str().unwrap_or("").to_string())
    };

    change("one");
    hub.save_tick();
    assert_eq!(on_disk().as_deref(), Some("one"), "the first copy at once");

    change("two");
    hub.save_tick();
    assert_eq!(on_disk().as_deref(), Some("one"), "not on every change");
    let quiet = Instant::now() - SAVE_QUIET - Duration::from_millis(10);
    hub.inner.lock().unwrap().canon_due = Some((quiet, quiet));
    hub.save_tick();
    assert_eq!(on_disk().as_deref(), Some("two"), "once they stop");

    // changes that go on are still written after a while
    change("three");
    let first = Instant::now() - SAVE_AT_MOST - Duration::from_millis(10);
    hub.inner.lock().unwrap().canon_due = Some((first, Instant::now()));
    hub.save_tick();
    assert_eq!(on_disk().as_deref(), Some("three"));

    // a change to people or devices goes at once, without the copy
    change("four");
    hub.inner.lock().unwrap().saved.roles.insert(7, Role::Admin);
    hub.inner.lock().unwrap().dirty = true;
    hub.save_tick();
    assert_eq!(on_disk().as_deref(), Some("three"));
    assert_eq!(load_saved(&home).unwrap().role_of(7), Role::Admin);

    // stopping writes what is left, and the revision with it
    change("five");
    hub.save_tick();
    assert_eq!(on_disk().as_deref(), Some("three"));
    hub.flush();
    assert_eq!(on_disk().as_deref(), Some("five"));
    assert_eq!(
        load_saved(&home).unwrap().rev,
        hub.inner.lock().unwrap().saved.rev
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_command_tells_a_busy_server_from_one_that_is_off() {
    let root = std::env::temp_dir().join(format!("wb-running-{:x}", wb_sync::random_u64()));
    let other = root.join(registry::WORKSPACES_DIR).join("w2");
    std::fs::create_dir_all(&other).unwrap();
    let ask = |home: &Path| {
        admin::request_files_within(
            home,
            (REQUEST_FILE, RESULT_FILE),
            &json!({"people": true}),
            Duration::from_millis(200),
            Duration::from_millis(200),
        )
    };
    // off: the request waits for the next start
    assert!(!admin::server_running(&root));
    assert!(ask(&root).unwrap().is_none());
    assert!(root.join(REQUEST_FILE).exists());
    std::fs::remove_file(root.join(REQUEST_FILE)).unwrap();

    // running but not taking it: an error, and the request is withdrawn
    let running = admin::hold_running(&root).expect("lock");
    assert!(admin::server_running(&root));
    assert!(
        admin::server_running(&other),
        "a workspace under the same server"
    );
    let err = ask(&root).unwrap_err().to_string();
    assert!(err.contains("nothing was done"), "{err}");
    assert!(!root.join(REQUEST_FILE).exists());
    let err = ask(&other).unwrap_err().to_string();
    assert!(err.contains("nothing was done"), "{err}");
    assert!(!other.join(REQUEST_FILE).exists());

    // once taken, a withdrawal can't happen: it says where the answer goes
    let taker = {
        let root = root.clone();
        std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(5);
            while std::time::Instant::now() < until {
                if admin::take_request(&root.join(REQUEST_FILE)).is_some() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            false
        })
    };
    let err = ask(&root).unwrap_err().to_string();
    assert!(taker.join().unwrap());
    assert!(err.contains("still on it"), "{err}");
    assert!(err.contains(RESULT_FILE), "{err}");

    drop(running);
    assert!(!admin::server_running(&root));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_code_from_a_signed_in_computer_signs_a_phone_in_once() {
    let home = std::env::temp_dir().join(format!("wb-link-{:x}", wb_sync::random_u64()));
    let mut saved = Saved::default();
    saved
        .accounts
        .insert(2, Account::new("bo@team.io", "bo-secret-22"));
    let hub = test_hub(
        &home,
        Some(json!({"members": [{"id": 2}, {"id": 3}]})),
        saved,
    );
    {
        let mut g = hub.inner.lock().unwrap();
        for (d, m) in [("mac", 2), ("cy-mac", 3)] {
            g.saved.add_device(d, m, None);
        }
    }
    let code = |hub: &Hub, member: u64, device: &str| match hub.new_link(member, device) {
        Msg::LinkCode { code, seconds } => {
            assert_eq!(seconds, LINK_SECS as u64);
            code
        }
        m => panic!("not a link code: {:?}", m),
    };
    let phone = |device: &str, code: &str| Hello {
        link_code: Some(code.into()),
        ..hello(device)
    };
    // six capital letters and digits, without look-alikes
    let c = code(&hub, 2, "mac");
    assert_eq!(c.len(), 6);
    assert!(c
        .chars()
        .all(|ch| "ABCDEFGHJKMNPQRSTUVWXYZ23456789".contains(ch)));
    // typed in lower case with a space: the phone is in as Bo, with a token
    let typed = format!("{} {}", &c[..3], &c[3..]).to_lowercase();
    let (member, _, _, token) = hub.sign_in(&phone("phone", &typed)).unwrap();
    assert_eq!(member, 2);
    assert!(token.is_some());
    assert!(hub.device_is("phone", 2));
    // it works once
    assert!(hub
        .sign_in(&phone("phone-2", &c))
        .unwrap_err()
        .contains("doesn't work"));
    // a new code from the same computer ends the one before
    let old = code(&hub, 2, "mac");
    let new = code(&hub, 2, "mac");
    assert!(hub.sign_in(&phone("p3", &old)).is_err());
    // another person's code stays
    let cy = code(&hub, 3, "cy-mac");
    assert_eq!(hub.sign_in(&phone("p4", &new)).map(|r| r.0), Ok(2));
    assert_eq!(hub.sign_in(&phone("p5", &cy)).map(|r| r.0), Ok(3));
    // a code from a computer that was signed out since (password changed
    // elsewhere, access removed) no longer works
    let late = code(&hub, 2, "mac");
    assert!(hub
        .set_password(
            2,
            "phone",
            "bo@team.io",
            "bo-secret-23",
            Some("bo-secret-22")
        )
        .is_ok());
    assert!(hub.sign_in(&phone("p6", &late)).is_err());
    assert!(!hub.device_is("p6", 2));
    // an expired code doesn't work
    let cy2 = code(&hub, 3, "cy-mac");
    hub.inner.lock().unwrap().links.get_mut(&cy2).unwrap().made -= LINK_SECS * 1000;
    assert!(hub.sign_in(&phone("p7", &cy2)).is_err());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn guessing_link_codes_stops_after_a_few_tries_a_minute() {
    let home = std::env::temp_dir().join(format!("wb-linkguess-{:x}", wb_sync::random_u64()));
    let hub = test_hub(
        &home,
        Some(json!({"members": [{"id": 2}]})),
        Saved::default(),
    );
    hub.inner.lock().unwrap().saved.add_device("mac", 2, None);
    let real = match hub.new_link(2, "mac") {
        Msg::LinkCode { code, .. } => code,
        m => panic!("not a link code: {:?}", m),
    };
    let try_code = |c: &str| {
        hub.sign_in(&Hello {
            link_code: Some(c.into()),
            ..hello("guesser")
        })
    };
    for _ in 0..LINK_TRIES_A_MINUTE {
        assert!(try_code("ZZZZZZ").unwrap_err().contains("doesn't work"));
    }
    // even the right code waits until the minute is over
    assert!(try_code(&real).unwrap_err().contains("Too many"));
    hub.inner.lock().unwrap().link_fails.1 -= 60_000;
    assert_eq!(try_code(&real).map(|r| r.0), Ok(2));
    let _ = std::fs::remove_dir_all(&home);
}

/// People who may see the same get the same broadcast, made once per
/// revision: a whole copy or a change, without what is hidden from them,
/// the same as made for each on its own.
#[test]
fn a_broadcast_is_made_once_for_people_who_see_the_same() {
    let canon = json!({"members": [{"id": 1}, {"id": 2}, {"id": 3}, {"id": 4}], "channels": [
        {"id": 10, "kind": "Channel", "members": []},
        {"id": 12, "kind": "Channel", "private": true, "members": [1, 4]},
    ]});
    let delta = json!({"channels": [{"id": 12, "kind": "Channel", "private": true, "members": [1, 4], "name": "x"}]});
    let c = Canon::new(
        Arc::new(canon.clone()),
        5,
        Some(delta.clone()),
        &Saved::default(),
    );
    // 2 and 3 aren't in channel 12; 1 and 4 are
    let (two, gone) = c.update_for(2);
    assert_eq!(c.update_for(3), (two.clone(), gone.clone()));
    let (data, hidden) = copy_for(&canon, 2, false).unwrap();
    let alone = Msg::Update {
        data,
        rev: 5,
        hidden: hidden.clone(),
    }
    .to_json();
    assert_eq!(two, alone);
    assert_eq!(gone, left_out(&canon, &hidden));
    assert_eq!(c.made.locked().len(), 1);
    // nothing hidden: the whole copy, not kept with the others
    assert_eq!(c.update_for(1).0, c.whole_json());
    assert_eq!(c.update_for(4).0, c.whole_json());
    assert_eq!(c.made.locked().len(), 1);
    // the change: one for 2 and 3, another for 1 and 4
    let d2 = c.delta_for(2, &[]).unwrap();
    assert_eq!(c.delta_for(3, &[]).unwrap(), d2);
    assert!(!d2.0.contains("\"name\":\"x\""), "{}", d2.0);
    let d1 = c.delta_for(1, &[]).unwrap();
    assert_eq!(c.delta_for(4, &[]).unwrap(), d1);
    assert!(d1.0.contains("\"name\":\"x\""), "{}", d1.0);
    assert_eq!(c.made.locked().len(), 3);
    // one whose last copy left channel 12 out gets it back whole: its own
    let back = c.delta_for(4, &[12]).unwrap();
    assert_eq!(c.made.locked().len(), 4);
    let fresh = Canon::new(Arc::new(canon.clone()), 5, Some(delta), &Saved::default());
    assert_eq!(fresh.delta_for(4, &[12]).unwrap(), back);
}

#[test]
fn waiting_for_a_busy_workspace_leaves_the_other_connections_running() {
    let home = std::env::temp_dir().join(format!("wb-busy-lock-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    let hub = Arc::new(test_hub(&home, None, Saved::default()));
    // one worker thread: a task stuck on the lock would stall every other one
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_time()
        .build()
        .unwrap();
    // a long merge elsewhere holds the lock
    let held = hub.clone();
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    let merge = std::thread::spawn(move || {
        let _g = held.inner.locked();
        locked_tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(600));
    });
    locked_rx.recv().unwrap();
    let ran_while_waiting = rt.block_on(async {
        let waiter = hub.clone();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let waiting = tokio::spawn(async move {
            started_tx.send(()).unwrap();
            let _g = waiter.state();
        });
        started_rx.await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        // a ping on another connection is answered meanwhile
        let other = tokio::spawn(async { std::time::Instant::now() });
        let answered = other.await.unwrap();
        waiting.await.unwrap();
        answered
    });
    let released = std::time::Instant::now();
    merge.join().unwrap();
    assert!(
        released.duration_since(ran_while_waiting) > Duration::from_millis(200),
        "the other task ran only after the lock was let go"
    );
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn messages_written_from_a_borrowed_copy_are_the_same_as_wb_nets() {
    let data = json!({"workspace": "Team", "channels": [{"id": 10, "messages": [{"id": 11, "text": "hi"}]}]});
    for hidden in [vec![], vec![12, 13]] {
        let msg = Msg::Update {
            data: data.clone(),
            rev: 7,
            hidden: hidden.clone(),
        };
        assert_eq!(update_json(&data, 7, &hidden), msg.to_json());
    }
    for (snapshot, token, email, hidden) in [
        (None, None, None, vec![]),
        (
            Some(data.clone()),
            Some("tok".to_string()),
            Some("a@b.c".to_string()),
            vec![12],
        ),
        (Some(data.clone()), None, None, vec![]),
    ] {
        let msg = Msg::Welcome {
            tag: 3,
            snapshot: snapshot.clone(),
            rev: 9,
            member: 4,
            role: Role::Admin,
            token: token.clone(),
            email: email.clone(),
            hidden: hidden.clone(),
            deltas: true,
            epoch: Some(77),
            since: snapshot.is_none().then_some(5),
            part: snapshot.is_none().then(|| json!({"tasks": []})),
        };
        let part = json!({"tasks": []});
        let ours = welcome_json(
            3,
            snapshot.as_ref(),
            9,
            4,
            Role::Admin,
            token,
            email,
            &hidden,
            77,
            snapshot.is_none().then_some((5, &part)),
        );
        assert_eq!(ours, msg.to_json());
        assert_eq!(Msg::parse(&ours), Some(msg));
    }
}

#[test]
fn what_is_left_out_is_every_id_inside_the_hidden_items() {
    let canon = json!({
        "channels": [
            {"id": 10, "messages": [{"id": 11, "replies": [{"id": 12}, {"text": "no id"}]}]},
            {"id": 20, "messages": [{"id": 21}]}
        ],
        "spaces": [{"id": 30, "folders": [{"id": 31, "lists": [{"id": 32}]}], "lists": [{"id": 33}],
                    "statuses": [{"name": "open"}]}],
        "tasks": [{"id": 40, "activity": [{"id": 41}], "checklist": []}],
        "docs": [{"id": 50}]
    });
    for hidden in [
        vec![10],
        vec![30, 40],
        vec![10, 20, 30, 40, 50],
        vec![99],
        vec![],
    ] {
        assert_eq!(
            left_out(&canon, &hidden),
            wb_sync::ids_inside(&canon, &hidden),
            "{:?}",
            hidden
        );
    }
}

#[test]
fn reading_state_is_written_on_its_own_and_moved_out_of_an_older_server_json() {
    let home = std::env::temp_dir().join(format!("wb-mine-file-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join(WORKSPACE_FILE), r#"{"workspace": "one"}"#).unwrap();
    // an older server kept reading state in server.json, pretty printed
    let old_mine = json!({"read": {"10": {"ts": 5, "at": 100}}});
    std::fs::write(
        home.join(STATE_FILE),
        serde_json::to_string_pretty(&json!({"rev": 4, "mine": {"1": old_mine}})).unwrap(),
    )
    .unwrap();
    let (_stop, stop) = watch::channel(false);
    let hub = Hub::open(
        home.clone(),
        home.clone(),
        None,
        &Options::default(),
        None,
        stop,
    )
    .unwrap();
    assert_eq!(*hub.state().saved.mine[&1], old_mine);
    hub.save_tick();
    let state = std::fs::read_to_string(home.join(STATE_FILE)).unwrap();
    assert!(!state.contains("\"read\""), "moved out: {state}");
    assert!(!state.contains('\n'), "written compact: {state}");
    let file: Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(MINE_FILE)).unwrap()).unwrap();
    assert_eq!(file, json!({"1": old_mine}));
    assert_eq!(load_saved(&home).unwrap().rev, 4);

    // someone opens a conversation: mine.json alone
    let before = std::fs::metadata(home.join(STATE_FILE))
        .unwrap()
        .modified()
        .unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let newer = json!({"read": {"10": {"ts": 9, "at": 200}}});
    assert_eq!(hub.take_mine(1, &newer, false), None);
    hub.save_tick();
    assert_eq!(
        std::fs::metadata(home.join(STATE_FILE))
            .unwrap()
            .modified()
            .unwrap(),
        before,
        "server.json isn't written again"
    );
    assert_eq!(
        *load_saved(&home).unwrap().mine[&1],
        *hub.state().saved.mine[&1]
    );
    assert!(!std::fs::read_to_string(home.join(WORKSPACE_FILE))
        .unwrap()
        .contains("\"read\""));

    // back on an older server for a while, which wrote it in server.json
    // again: both are merged, the newest entry winning
    let mut both =
        serde_json::from_str::<Value>(&std::fs::read_to_string(home.join(STATE_FILE)).unwrap())
            .unwrap();
    both["mine"] = json!({"1": {"read": {"10": {"ts": 7, "at": 150}, "11": {"ts": 2, "at": 300}}}, "2": {"x": 1}});
    std::fs::write(home.join(STATE_FILE), both.to_string()).unwrap();
    let loaded = load_saved(&home).unwrap();
    assert_eq!(
        loaded.mine[&1]["read"]["10"],
        json!({"ts": 9, "at": 200}),
        "the newer from mine.json"
    );
    assert_eq!(
        loaded.mine[&1]["read"]["11"],
        json!({"ts": 2, "at": 300}),
        "only in server.json"
    );
    assert_eq!(*loaded.mine[&2], json!({"x": 1}));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_backup_check_with_nothing_new_reads_and_writes_nothing() {
    let home = std::env::temp_dir().join(format!("wb-backup-skip-{:x}", wb_sync::random_u64()));
    std::fs::create_dir_all(&home).unwrap();
    let hub = test_hub(&home, Some(json!({"workspace": "one"})), Saved::default());
    hub.backup_now();
    let dir = home.join(BACKUP_DIR);
    assert_eq!(backup_files(&dir, "hourly").len(), 1);
    assert_eq!(backup_files(&dir, "daily").len(), 1);
    assert!(hub.state().backed_up.is_some());
    // the files are gone, but nothing changed: the next check doesn't look
    for f in backup_files(&dir, "hourly")
        .into_iter()
        .chain(backup_files(&dir, "daily"))
    {
        std::fs::remove_file(f).unwrap();
    }
    hub.backup_now();
    assert!(backup_files(&dir, "hourly").is_empty());
    // a change: the next check writes it
    {
        let mut g = hub.state();
        g.canon.as_mut().map(Arc::make_mut).unwrap()["workspace"] = json!("two");
        hub.broadcast_canon(&mut g, false);
    }
    hub.backup_now();
    let hourly = backup_files(&dir, "hourly");
    assert_eq!(hourly.len(), 1);
    assert!(std::fs::read_to_string(&hourly[0]).unwrap().contains("two"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn marks_of_what_a_delete_takes_away_are_the_same_as_all_marks() {
    let canon = json!({
        "channels": [
            {"id": 10, "messages": [
                {"id": 11, "author": 1, "text": "hi", "replies": [{"id": 12, "author": 2, "text": "yo"}]},
                {"id": 13, "author": 2, "text": "there"}
            ]},
            {"id": 20, "messages": [{"id": 21, "author": 1, "text": "x"}]}
        ],
        "tasks": [
            {"id": 30, "activity": [{"id": 31, "comment": true, "author": 1, "text": "c"}, {"id": 32, "text": "moved"}]},
            {"id": 40, "activity": [{"id": 41, "comment": true, "author": 2, "text": "d"}]}
        ]
    });
    let all = spaces::written_marks(&canon);
    let within = |ids: &[u64]| spaces::written_marks_within(&canon, &ids.iter().copied().collect());
    let pick = |ids: &[u64]| -> std::collections::HashMap<u64, u64> {
        ids.iter().map(|i| (*i, all[i])).collect()
    };
    assert_eq!(within(&[]), pick(&[]));
    assert_eq!(within(&[13, 41]), pick(&[13, 41]));
    // a message takes its thread along, a channel or task everything in it
    assert_eq!(within(&[11]), pick(&[11, 12]));
    assert_eq!(within(&[10]), pick(&[11, 12, 13]));
    assert_eq!(within(&[30, 20]), pick(&[31, 21]));
    assert_eq!(within(&[32, 99]), pick(&[]));
    let ids: Vec<u64> = all.keys().copied().collect();
    let mut want: Vec<u64> = spaces::written_ids(&canon).into_iter().collect();
    want.sort();
    let mut ids = ids;
    ids.sort();
    assert_eq!(ids, want);
}

#[test]
fn only_changes_to_what_hiding_goes_by_need_the_trial_merge() {
    let canon = json!({
        "spaces": [
            {"id": 1, "name": "Team", "lists": [{"id": 2, "name": "L"}]},
            {"id": 3, "name": "Mine", "private_owner": 9, "lists": []}
        ],
        "docs": [{"id": 5, "title": "Doc", "body": "a", "private": false, "owner": 7, "parent": null}],
        "tasks": [{"id": 6, "name": "T", "list_id": 2}],
        "channels": [{"id": 8, "kind": "Channel", "members": []}]
    });
    let team = wb_sync::fingerprints(&canon);
    let may = |data: Value| spaces::may_hide(&canon, &data, &team);
    // typing in a doc, renaming a space or task: no
    assert!(!may(
        json!({"docs": [{"id": 5, "title": "Doc", "body": "ab", "owner": 7}]})
    ));
    assert!(!may(
        json!({"spaces": [{"id": 1, "name": "Renamed", "lists": [{"id": 2, "name": "L"}]}]})
    ));
    assert!(!may(
        json!({"spaces": [{"id": 1, "name": "Team"}], "tasks": [{"id": 6, "name": "T2", "list_id": 2}]})
    ));
    // a new list in a space, a new private doc: nothing the team has
    assert!(!may(
        json!({"spaces": [{"id": 1, "lists": [{"id": 2}, {"id": 40}]}]})
    ));
    assert!(!may(
        json!({"docs": [{"id": 41, "private": true, "owner": 9}]})
    ));
    // what hiding goes by: yes
    assert!(may(
        json!({"docs": [{"id": 5, "private": true, "owner": 7}]})
    ));
    assert!(may(json!({"docs": [{"id": 5, "owner": 7, "parent": 41}]})));
    assert!(may(json!({"spaces": [{"id": 1, "private_owner": 9}]})));
    assert!(
        may(json!({"spaces": [{"id": 3, "private_owner": 9, "lists": [{"id": 2}]}]})),
        "a team list moved into a Personal space"
    );
    assert!(
        may(json!({"spaces": [{"id": 50, "private_owner": 9, "lists": [{"id": 2}]}]})),
        "into a new one"
    );
    assert!(may(
        json!({"docs": [], "tasks": [{"id": 6, "list_id": 99}]})
    ));
    assert!(may(
        json!({"docs": [], "channels": [{"id": 8, "kind": "Channel", "private": true, "members": [9]}]})
    ));
}

#[test]
fn a_posted_message_merges_as_if_the_whole_copy_came_with_it() {
    let mut canon = json!({
        "workspace": "Team",
        "members": [{"id": 1, "name": "Ann"}],
        "channels": [
            {"id": 10, "kind": "Channel", "name": "general", "topic": "", "members": [], "messages": [
                {"id": 11, "author": 1, "text": "hi", "ts": 5, "replies": [{"id": 12, "author": 1, "text": "yo", "ts": 6}]},
                {"id": 13, "author": 1, "text": "no thread yet", "ts": 7}
            ]},
            {"id": 20, "kind": "Channel", "name": "other", "members": [], "messages": [{"id": 21, "text": "x"}]}
        ],
        "tasks": [{"id": 30, "name": "T"}],
        "extra": {"sync": {}}
    });
    let mut meta = wb_sync::json::meta_of(&canon);
    wb_sync::stamp(&canon, &mut meta, 1_000);
    wb_sync::touch_fields(&mut meta, 10, &["topic"], 2_000);
    wb_sync::json::set_meta(&mut canon, &meta);
    wb_sync::make_shareable(&mut canon);
    let now = 1_795_000_000;
    for parent in [None, Some(11), Some(13)] {
        let w = schedule::Waiting {
            member: 1,
            item: wb_net::ScheduledMsg {
                id: 99,
                channel: 10,
                parent,
                text: "later".into(),
                at: now,
                mentions: vec![3],
            },
        };
        let part = schedule::with_message(&canon, &w, now).unwrap();
        assert!(
            !part.to_string().contains("\"other\""),
            "only its conversation: {part}"
        );
        let stamp = part["extra"]["sync"]["stamps"]["99"].as_i64().unwrap();
        // what posting it used to merge: the whole copy with it added
        let mut whole = canon.clone();
        let msg = part["channels"][0]["messages"][0].clone();
        let list = &mut whole["channels"][0]["messages"];
        match parent {
            None => list.as_array_mut().unwrap().push(msg),
            Some(p) => {
                let at = list
                    .as_array()
                    .unwrap()
                    .iter()
                    .position(|m| m["id"] == p)
                    .unwrap();
                let reply = msg["replies"].as_array().unwrap().last().unwrap().clone();
                if !list[at]["replies"].is_array() {
                    list[at]["replies"] = json!([]);
                }
                list[at]["replies"].as_array_mut().unwrap().push(reply);
            }
        }
        let mut m = wb_sync::json::meta_of(&whole);
        m.stamps.insert(99, stamp);
        wb_sync::json::set_meta(&mut whole, &m);
        let (mut a, mut b) = (canon.clone(), canon.clone());
        assert!(wb_sync::merge(&mut a, &[whole]));
        assert!(wb_sync::merge(&mut b, &[part]));
        wb_sync::make_shareable(&mut a);
        wb_sync::make_shareable(&mut b);
        assert_eq!(a, b, "parent {parent:?}");
        assert!(b.to_string().contains("later"));
        // posted twice: nothing the second time
        assert!(schedule::with_message(&b, &w, now).is_none());
    }
}

/// What changed after a revision can be told for the revisions kept, not
/// for older ones, and not across one that went whole (a restore).
#[test]
fn catching_up_reaches_back_only_as_far_as_the_kept_revisions() {
    let changes = |id: u64| {
        let mut s = wb_sync::Summary::of(&json!({}));
        wb_sync::delta_advance(&mut s, &json!({"tasks": [{"id": id}]})).1
    };
    let mut c = Catchup {
        from: Some(10),
        ..Default::default()
    };
    // nothing yet: up to date at 10, nothing known before
    assert!(c.since(10, 10).is_some_and(|ch| ch.is_empty()));
    assert!(c.since(9, 10).is_none());
    for rev in 11..=13 {
        c.note(rev, Some(changes(rev)));
    }
    assert_eq!(c.since(10, 13).unwrap().len(), 3);
    assert_eq!(c.since(12, 13).unwrap().len(), 1);
    assert!(c.since(13, 13).unwrap().is_empty());
    // a revision the server never had
    assert!(c.since(14, 13).is_none());
    // a restore: nothing from before it
    c.note(14, None);
    assert!(c.since(13, 14).is_none());
    assert!(c.since(14, 14).unwrap().is_empty());
    // only so many are kept: the oldest go
    for rev in 15..15 + CATCHUP_REVS as u64 + 2 {
        c.note(rev, Some(changes(rev)));
    }
    let newest = 14 + CATCHUP_REVS as u64 + 2;
    assert!(c.since(14, newest).is_none());
    assert!(c.since(16, newest).is_some());
    assert_eq!(c.revs.len(), CATCHUP_REVS);
}
