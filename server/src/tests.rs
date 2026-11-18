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
                g.canon.as_ref() == Some(&canon),
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

fn test_hub(home: &Path, canon: Option<Value>, saved: Saved) -> Hub {
    Hub {
        home: home.to_path_buf(),
        root: home.to_path_buf(),
        id: None,
        persistence: Mutex::new(()),
        inner: Mutex::new(Inner {
            canon,
            saved,
            dirty: false,
            resets: Default::default(),
            mailed: Default::default(),
            fails: Default::default(),
            summary: None,
        }),
        updates: broadcast::channel(64).0,
        kicks: broadcast::channel(16).0,
        access: broadcast::channel(16).0,
        stop: watch::channel(false).1,
        connections: AtomicUsize::new(0),
        hashing: AtomicUsize::new(0),
        max_file: MAX_FILE,
        claim_wait: Duration::from_secs(CLAIM_WAIT_SECS),
        pusher: None,
        online: Mutex::new(Default::default()),
        members_online: Mutex::new(Default::default()),
        presence: watch::channel(vec![]).0,
        closed: watch::channel(false).0,
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
        resets: Default::default(),
        mailed: Default::default(),
        fails: Default::default(),
        summary: None,
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
        g.saved.people(g.canon.as_ref(), Role::Admin)
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
