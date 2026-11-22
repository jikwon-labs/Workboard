// Behaviour tests for working together: two computers on one shared folder
// or team server, merging changes, joining a team, and what older files
// and servers still do.

use super::*;

/// A change isn't written the moment it's made: the workspace is handed to
/// the background writer once changes stop for a second (or five seconds
/// after the first one, for steady typing), as compact text. Quitting saves
/// at once, and a copy still waiting never lands on top of a newer save.
/// A synced workspace's file leaves out this computer's fingerprints when
/// they are just what its content gives, and opening it works them out
/// again: the same bookkeeping comes back. Fingerprints a change hasn't
/// been recorded against stay in the file, so that change is still found
/// after opening it.
#[test]
fn a_saved_workspace_leaves_out_fingerprints_it_can_work_out_again() {
    let _home = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home-hashes");
    std::fs::create_dir_all(&home).unwrap();
    super::point_home(&home);
    let mut a = app();
    a.persist = true;
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "d".into(),
        tag: 1,
        server: Some("127.0.0.1:1".into()),
    });
    a.sync_note_change();
    assert!(!a.data.extra.sync.hashes.is_empty());
    a.save_now();
    let text = std::fs::read_to_string(Data::path()).unwrap();
    assert!(!text.contains("\"field_hashes\""), "left out");
    assert!(!text.contains("\"hashes\""));
    assert!(text.contains("\"rehash\":true"));
    let opened = Data::load();
    assert_eq!(
        opened.extra.sync, a.data.extra.sync,
        "the same bookkeeping, worked out again"
    );

    // a change not recorded yet: the old fingerprints stay and still show it
    let t = a.data.tasks[0].id;
    a.data.tasks[0].name = "Renamed before it was recorded".into();
    a.save_now();
    let text = std::fs::read_to_string(Data::path()).unwrap();
    assert!(text.contains("\"hashes\""), "kept");
    assert!(!text.contains("\"rehash\""));
    let mut b = App::build(None, Some(Data::load()));
    let before = b.data.extra.sync.stamps.get(&t).copied();
    b.sync_note_change();
    assert_ne!(
        b.data.extra.sync.stamps.get(&t).copied(),
        before,
        "the change is still recorded"
    );
}

#[test]
fn two_computers_work_in_one_shared_folder() {
    let dir = temp_home("share");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();

    // someone else joins from their own computer
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.join_shared(&folder).unwrap();
    assert_eq!(
        b.data.tasks.len(),
        a.data.tasks.len(),
        "the joiner gets the team's tasks"
    );
    assert!(
        b.data.member(b.data.me).is_none(),
        "the joiner still has to pick who they are"
    );
    b.become_member(None, "Bora", "bora@team.kr");
    let bora = b.data.me;

    // both work at the same time
    let ch = channel_id(&b, "general");
    b.apply_chat(vec![ChatAction::Send {
        ch,
        text: "hi from Bora".into(),
        parent: None,
        files: vec![],
    }]);
    b.sync_note_change();
    b.write_shared();
    let list = a.data.first_list().unwrap();
    a.apply(Action::Add {
        list,
        name: "Made by A".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    a.sync_note_change();
    a.write_shared();

    assert!(a.sync_pull(), "A gets B's changes");
    assert!(
        a.data.member(bora).is_some(),
        "the new member shows up for A"
    );

    let msgs = &a
        .data
        .channels
        .iter()
        .find(|c| c.id == ch)
        .unwrap()
        .messages;
    assert_eq!(msgs.last().unwrap().text, "hi from Bora");
    assert!(b.sync_pull(), "B gets A's task");
    assert!(b.data.tasks.iter().any(|t| t.name == "Made by A"));

    // a delete on one computer reaches the other and doesn't come back
    let tid = b
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Made by A")
        .unwrap()
        .id;
    b.apply(Action::DeleteTask(tid));
    b.sync_note_change();
    b.write_shared();
    a.sync_pull();
    assert!(a.task(tid).is_none(), "deleted for everyone");
    a.sync_note_change();
    a.write_shared();
    b.sync_pull();
    assert!(b.task(tid).is_none(), "and it stays deleted");

    // ids made on the two computers never clash
    assert_ne!(a.data.new_id() % 1024, b.data.new_id() % 1024);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn joining_another_team_keeps_the_workspace_already_here() {
    let _home = HOME_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let home = temp_home("home-join");
    std::fs::create_dir_all(&home).unwrap();
    super::point_home(&home);

    // someone else's team, with an invite code for Sam
    let theirs = TestServer::start();
    let their_addr = format!("127.0.0.1:{}", theirs.port);
    let mut a = app();
    share_on(&mut a, &theirs);
    let code = invite_code_for_sam(&mut a, &theirs);

    // Sam's own workspace, already on Sam's own server
    let mine = TestServer::start();
    let my_addr = format!("127.0.0.1:{}", mine.port);
    let mut b = App::build(None, Some(blank_as("Mine", 1, "Sam")));
    b.persist = true;
    b.data.save();
    b.connect_server(&my_addr, "").unwrap();
    net_until(&mut b, "B's workspace on its server", |b| {
        b.data.extra.shared.is_some()
    });
    net_until(&mut b, "B's people list", |b| !b.sync_rt.people.is_empty());
    let my_tasks = b.data.tasks.len();

    // an empty server doesn't take this workspace away from its own
    let empty = TestServer::start();
    b.connect_server(&format!("127.0.0.1:{}", empty.port), "")
        .unwrap();
    net_until(&mut b, "told the server has no team", |b| {
        b.toast
            .as_ref()
            .is_some_and(|t| t.0.contains("no team yet"))
    });
    assert_eq!(
        b.data.extra.shared.as_ref().unwrap().server.as_deref(),
        Some(my_addr.as_str())
    );

    // join their team from the workspace menu with the code
    b.open_join_team(String::new());
    b.connect_with(
        &their_addr,
        together::How::Invite {
            code: code.clone(),
            email: String::new(),
            password: String::new(),
        },
    )
    .unwrap();
    net_until(&mut b, "B joined their team", |b| {
        b.data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.as_deref() == Some(their_addr.as_str()))
    });
    assert_eq!(b.data.me, 3, "the code says who they are there");
    assert_eq!(b.data.tasks.len(), a.data.tasks.len());
    assert!(
        b.sync_rt.people.is_empty(),
        "the people of Sam's own server don't come along"
    );
    assert_ne!(b.ws_file, "data.json", "their team is a new workspace");

    // Sam's own workspace is still there, on Sam's server, as it was
    let list = Data::workspaces();
    assert!(
        list.iter().any(|w| w.file == "data.json"),
        "{:?}",
        list.iter().map(|w| &w.file).collect::<Vec<_>>()
    );
    assert!(list.iter().any(|w| w.file == b.ws_file));
    let kept = Data::load();
    assert_eq!(kept.workspace, "Mine");
    assert_eq!(kept.tasks.len(), my_tasks);
    assert_eq!(
        kept.extra.shared.as_ref().unwrap().server.as_deref(),
        Some(my_addr.as_str())
    );
    // and switching back connects to it again as its owner
    b.switch_workspace("data.json");
    assert_eq!(b.data.workspace, "Mine");
    net_until(&mut b, "back on Sam's server", |b| {
        b.sync_rt.net_status == Some(wb_net::Status::Connected)
    });
    assert_eq!(b.sync_rt.role, Some(wb_net::Role::Owner));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn two_computers_work_through_a_server() {
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);

    // the first app to connect brings its workspace to the empty server
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A shared on the server", |a| {
        a.data.extra.shared.is_some()
    });
    let cfg = a.data.extra.shared.clone().unwrap();
    assert_eq!(cfg.server.as_deref(), Some(addr.as_str()));
    assert!(cfg.folder.is_empty());
    // wait until the server has saved A's copy, so B finds it there
    net_until_saved(&mut a, &server.home);

    // A is the owner and makes an invite code for Sam (already a member)
    assert_eq!(a.sync_rt.role, Some(wb_net::Role::Owner));
    net_until(&mut a, "A sees the people list", |a| {
        a.sync_rt.people.iter().any(|p| p.member == 3)
    });
    let code = invite_code(&mut a, 3, wb_net::Role::Member);

    // without a code nobody gets in
    let mut x = App::build(None, Some(blank_as("Nope", 1, "X")));
    x.connect_server(&addr, "").unwrap();
    net_until(&mut x, "refused", |x| {
        matches!(x.sync_rt.net_status, Some(wb_net::Status::Refused(_)))
    });
    assert!(x.data.extra.shared.is_none(), "nothing changes for them");

    // Sam joins from their own computer with the code
    let mut b = join_with(&addr, &code.to_lowercase());
    assert_eq!(
        b.data.tasks.len(),
        a.data.tasks.len(),
        "the joiner gets the team's tasks"
    );
    assert_eq!(b.data.me, 3, "the code says who they are");
    assert!(
        !matches!(b.modal, Some(Modal::WhoAmI { .. })),
        "no picking who you are on a server"
    );
    assert_eq!(b.sync_rt.role, Some(wb_net::Role::Member));
    assert_ne!(
        b.data.extra.shared.as_ref().unwrap().tag,
        cfg.tag,
        "the server hands out different id tags"
    );
    let cfg_b = b.data.extra.shared.clone().unwrap();
    let token = together::load_token(&addr, &cfg_b.device).expect("the token is kept");
    assert!(
        !serde_json::to_string(&b.data).unwrap().contains(&token),
        "but not in data.json"
    );
    let bora = b.data.me;

    // chat from B reaches A
    let ch = channel_id(&b, "general");
    b.apply_chat(vec![ChatAction::Send {
        ch,
        text: "hi through the server".into(),
        parent: None,
        files: vec![],
    }]);
    b.sync_note_change();
    b.write_shared();
    net_until(&mut a, "B's message at A", |a| {
        a.data
            .channels
            .iter()
            .find(|c| c.id == ch)
            .is_some_and(|c| {
                c.messages
                    .last()
                    .is_some_and(|m| m.text == "hi through the server")
            })
    });
    assert!(
        a.data.member(bora).is_some(),
        "the new member shows up for A"
    );

    // B starts writing the next one: A sees "Sam ... is typing..." in #general
    b.typing_now(ch, None);
    let general = |a: &App| a.data.channels.iter().find(|c| c.id == ch).unwrap().clone();
    net_until(&mut a, "B typing at A", |a| {
        a.typing_in(&general(a)) == vec![bora]
    });
    let name = a.data.member(bora).unwrap().shown().to_string();
    assert_eq!(
        a.typing_line(&general(&a)),
        Some(format!("{} is typing...", name))
    );

    // then in the thread of that message: only the thread shows it
    let tid = general(&a).messages.last().unwrap().id;
    b.sync_rt.typed = None;
    b.typing_now(ch, Some(tid));
    net_until(&mut a, "B typing in the thread at A", |a| {
        a.typing_where(&general(a), Some(tid)) == vec![bora]
    });
    assert!(a.typing_in(&general(&a)).is_empty(), "not in the channel");

    // a task from A reaches B
    let list = a.data.first_list().unwrap();
    a.apply(Action::Add {
        list,
        name: "Made by A".into(),
        status: None,
        priority: 0,
        assignees: vec![],
        due: None,
        parent: None,
        open: false,
    });
    a.sync_note_change();
    a.write_shared();
    net_until(&mut b, "A's task at B", |b| {
        b.data.tasks.iter().any(|t| t.name == "Made by A")
    });

    // the owner removes Sam's access: Sam is signed out and can sign in again with a new code
    a.access_request(wb_net::Msg::Revoke { member: 3 });
    net_until(&mut b, "B signed out", |b| {
        matches!(b.sync_rt.net_status, Some(wb_net::Status::Refused(_)))
    });
    let code = invite_code(&mut a, 3, wb_net::Role::Member);
    b.sign_in_again(&code);
    net_until(&mut b, "B back in", |b| {
        b.sync_rt.net_status == Some(wb_net::Status::Connected)
    });

    // stopping sharing ends the connection and forgets the token
    let dev = a.data.extra.shared.as_ref().unwrap().device.clone();
    a.stop_sharing();
    assert!(a.sync_rt.net.is_none() && a.data.extra.shared.is_none());
    assert!(together::load_token(&addr, &dev).is_none());
}

#[test]
fn a_bad_server_address_is_explained() {
    let mut a = app();
    assert!(a.connect_server("", "").is_err());
    assert!(a.connect_server("https://example:web", "").is_err());
    assert!(a.sync_rt.joining.is_none());
}

#[test]
fn old_shared_settings_still_load() {
    let old = r#"{"folder":"/tmp/x","device":"abc","tag":7}"#;
    let cfg: crate::sync::SharedCfg = serde_json::from_str(old).unwrap();
    assert_eq!(cfg.server, None);
    assert_eq!(cfg.folder, "/tmp/x");
    assert!(
        !serde_json::to_string(&cfg).unwrap().contains("server"),
        "folder settings are saved as before"
    );
}

#[test]
fn team_settings_and_moved_lists_reach_everyone_once() {
    let dir = temp_home("move");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.data.extra.accent = "pink".into();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.data.extra.accent = "teal".into();
    b.join_shared(&folder).unwrap();
    assert_eq!(
        b.data.extra.accent, "teal",
        "the joiner keeps their own theme"
    );
    assert_eq!(b.data.workspace, a.data.workspace);
    b.become_member(None, "Bora", "bora@team.kr");
    b.sync_note_change();
    b.write_shared();
    a.sync_pull();

    // A adds a second space, moves a list there and renames the workspace
    let sid = a.data.new_id();
    a.data.spaces.push(Space {
        id: sid,
        name: "Other".into(),
        statuses: default_statuses(),
        ..Default::default()
    });
    a.dirty = true;
    a.sync_note_change();
    a.write_shared();
    b.sync_pull();
    let list = a.data.first_list().unwrap();
    a.move_list(list, sid, None);
    a.data.workspace = "Renamed".into();
    a.data.extra.apps.priorities = false;
    a.sync_note_change();
    a.write_shared();
    // B, not having seen that yet, writes its stale copy
    b.sync_note_change();
    b.write_shared();
    b.sync_pull();
    a.sync_pull();
    for d in [&a.data, &b.data] {
        let homes = d
            .spaces
            .iter()
            .flat_map(|s| {
                s.lists
                    .iter()
                    .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
            })
            .filter(|l| l.id == list)
            .count();
        assert_eq!(homes, 1, "the moved list is in one place");
        assert!(d
            .spaces
            .iter()
            .find(|s| s.id == sid)
            .unwrap()
            .lists
            .iter()
            .any(|l| l.id == list));
        assert_eq!(d.workspace, "Renamed");
        assert!(!d.extra.apps.priorities);
    }
    assert_eq!(b.data.extra.accent, "teal", "theme stays personal");

    // a copy this app can't read is reported once, not on every update
    let mut bad = serde_json::to_value(&b.data).unwrap();
    // a newer app changed what a setting looks like
    bad["workspace"] = serde_json::json!({"name": "Renamed"});
    bad["extra"]["sync"]["values"]["workspace"] = serde_json::json!(i64::MAX / 2);
    std::fs::write(
        dir.join(format!("zz{}", crate::sync::FILE_SUFFIX)),
        bad.to_string(),
    )
    .unwrap();
    a.toast = None;
    assert!(!a.sync_pull());
    assert!(a
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("newer version")));
    a.toast = None;
    a.sync_pull();
    assert!(a.toast.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quitting_sends_my_stopped_timer_to_the_team() {
    let dir = temp_home("quit");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.join_shared(&folder).unwrap();
    b.become_member(None, "Bora", "");
    let tid = a.data.tasks[0].id;
    let entries = a.data.tasks[0].time_entries.len();
    a.data.tasks[0].timer_started = Some(now_ts() - 60);
    a.data.tasks[0].timer_by = Some(a.data.me);
    a.sync_note_change();
    a.write_shared();
    b.sync_pull();
    assert!(b.task(tid).unwrap().timer_started.is_some());
    // quitting right away, before the next frame would have written it
    a.finish_before_quit();
    assert!(a.task(tid).unwrap().timer_started.is_none());
    b.sync_pull();
    let t = b.task(tid).unwrap();
    assert!(
        t.timer_started.is_none(),
        "the stopped timer reached the team"
    );
    assert_eq!(t.time_entries.len(), entries + 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_not_recorded_before_a_merge_still_reaches_the_team() {
    let dir = temp_home("unrecorded");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.join_shared(&folder).unwrap();
    b.become_member(None, "Bora", "");
    b.sync_note_change();
    b.write_shared();
    // A changes a task, and the team's copy is merged in before that change was recorded
    let tid = a.data.tasks[0].id;
    a.data.tasks[0].name = "Renamed just before".into();
    a.sync_pull();
    a.sync_note_change();
    a.write_shared();
    b.sync_pull();
    assert_eq!(b.task(tid).unwrap().name, "Renamed just before");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn joining_a_folder_does_not_add_you_to_private_channels() {
    let mut a = app();
    let private = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel && c.name != "general")
        .unwrap()
        .id;
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == private)
        .unwrap()
        .private = true;
    a.become_member(None, "New", "");
    let me = a.data.me;
    let ch = |id: u64| a.data.channels.iter().find(|c| c.id == id).unwrap();
    assert!(!ch(private).members.contains(&me));
    assert!(a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap()
        .members
        .contains(&me));
    assert!(a
        .data
        .channels
        .iter()
        .filter(|c| c.kind == ChannelKind::Direct)
        .all(|c| !c.members.contains(&me)));
}

#[test]
fn fields_added_later_read_the_same_to_sync_when_left_unset() {
    let fp = |extra: serde_json::Value| {
        crate::sync::fingerprints(&serde_json::json!({ "extra": extra }))
    };
    let dash = Dashboard {
        id: 1,
        name: "D".into(),
        ..Default::default()
    };
    let form = Form {
        id: 2,
        name: "F".into(),
        list_id: 3,
        description: String::new(),
        fields: vec![],
        owner: 0,
        ..Default::default()
    };
    let wb = Whiteboard {
        id: 4,
        name: "W".into(),
        strokes: vec![],
        notes: vec![],
        owner: 0,
        ..Default::default()
    };
    let new = fp(serde_json::json!({"dashboards": [dash], "forms": [form], "whiteboards": [wb]}));
    // as an app from before these fields wrote them
    let old = fp(
        serde_json::json!({"dashboards": [{"id": 1, "name": "D", "space": null}], "forms": [{"id": 2, "name": "F", "list_id": 3, "description": "", "fields": []}], "whiteboards": [{"id": 4, "name": "W", "strokes": [], "notes": []}]}),
    );
    assert_eq!(new, old);
    let e = serde_json::to_value(TimeEntry {
        start: 1,
        end: 2,
        member: 0,
    })
    .unwrap();
    assert_eq!(e, serde_json::json!({"start": 1, "end": 2}));

    // fingerprints saved while these fields were always written don't make
    // everything look edited after updating
    let dir = temp_home("shape");
    let mut a = app();
    a.data.extra.dashboards.push(Dashboard {
        id: 7001,
        name: "Old".into(),
        ..Default::default()
    });
    a.data.tasks[0].time_entries.push(TimeEntry {
        start: 1,
        end: 2,
        member: 0,
    });
    a.share_here(dir.to_str().unwrap()).unwrap();
    let v = serde_json::to_value(&a.data).unwrap();
    a.data.extra.sync.hashes = crate::sync::fingerprints(&together::legacy_shape(&v));
    a.data.extra.sync.v = 1;
    let stamps = a.data.extra.sync.stamps.clone();
    a.sync_note_change();
    assert_eq!(a.data.extra.sync.stamps, stamps, "nothing counts as edited");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn on_a_server_what_i_may_no_longer_see_leaves_my_copy_without_deleting_it() {
    let mut a = app();
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "d".into(),
        tag: 1,
        server: Some("127.0.0.1:1".into()),
    });
    let me = a.data.me;
    let ch = a
        .data
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel && c.name != "general")
        .unwrap()
        .id;
    // a DM between two other people, kept from an older version
    a.data.channels.push(Channel {
        id: 7300,
        kind: ChannelKind::Direct,
        members: vec![2, 3],
        ..Default::default()
    });
    a.sync_note_change();
    // I write in the channel just as I'm taken out of it
    a.data
        .channels
        .iter_mut()
        .find(|c| c.id == ch)
        .unwrap()
        .messages
        .push(Message {
            goal: None,
            id: 7301,
            author: me,
            text: "my last words".into(),
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
        });
    let mut team = serde_json::to_value(&a.data).unwrap();
    crate::sync::hide(&mut team, &[ch, 7300]);
    a.merge_in(&[team], &[ch]);
    assert!(!a.data.channels.iter().any(|c| c.id == ch || c.id == 7300));
    assert!(
        a.data.extra.notepad.contains("my last words"),
        "what I wrote is kept: {:?}",
        a.data.extra.notepad
    );
    assert!(a.toast.is_some());
    a.sync_note_change();
    let m = &a.data.extra.sync;
    assert!(
        !m.tombs.contains_key(&ch) && !m.tombs.contains_key(&7300) && !m.tombs.contains_key(&7301),
        "nothing deleted for the team"
    );
}

#[test]
fn seen_ids_keep_the_newest_and_cover_from_the_oldest_of_them() {
    let few = SeenIds::of((1..=5).map(|i| (i, i as i64 * 10)));
    assert_eq!(few.ids, vec![1, 2, 3, 4, 5]);
    assert!(
        few.is_new(99, -1_000_000),
        "with few messages everything is covered"
    );
    let many = SeenIds::of((1..=400u64).map(|i| (i, i as i64)));
    assert_eq!(many.ids.len(), SeenIds::MAX);
    assert_eq!(many.floor, 101);
    assert!(many.has(400) && !many.has(100));
    assert!(
        many.is_new(999, 150) && !many.is_new(999, 50),
        "only inside what it covers"
    );
}

#[test]
fn joining_a_shared_workspace_forgets_history() {
    let dir = temp_home("join-history");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.apply(Action::Go(Location::Inbox));
    b.track_history();
    b.apply(Action::Go(Location::Home));
    b.track_history();
    assert!(!b.nav.back.is_empty());
    b.join_shared(&folder).unwrap();
    assert!(
        b.nav.back.is_empty() && b.nav.fwd.is_empty(),
        "Back doesn't lead into the old workspace"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn files_from_before_ids_were_kept_still_load_and_new_fields_stay_out_when_empty() {
    // an activity line and a chat message as an older app writes them
    let act: Activity =
        serde_json::from_str(r#"{"ts":5,"author":2,"text":"assigned to Me","comment":false}"#)
            .unwrap();
    assert_eq!((act.target, act.mentions.is_empty()), (None, true));
    let msg: Message =
        serde_json::from_str(r#"{"id":9,"author":2,"text":"@Me hi","ts":5}"#).unwrap();
    assert!(msg.mentions.is_empty());
    // empty ones are not written, so files only change where ids are known
    let out = serde_json::to_value(&act).unwrap();
    assert!(out.get("target").is_none() && out.get("mentions").is_none());
    assert!(serde_json::to_value(&msg)
        .unwrap()
        .get("mentions")
        .is_none());
    // and they round-trip when set
    let a2 = Activity {
        target: Some(4),
        mentions: vec![2, 3],
        ..act.clone()
    };
    let back: Activity = serde_json::from_value(serde_json::to_value(&a2).unwrap()).unwrap();
    assert_eq!((back.target, back.mentions), (Some(4), vec![2, 3]));
    // a whole workspace saved by an older app reads, and its inbox still works by name
    let mut old = serde_json::to_value(Data::seed()).unwrap();
    let t0 = &mut old["tasks"][0];
    t0["assignees"] = serde_json::json!([]);
    t0["activity"] =
        serde_json::json!([{"ts": 5, "author": 2, "text": "assigned to Me", "comment": false}]);
    let d: Data = serde_json::from_value(old).unwrap();
    let a = App::build(None, Some(d));
    assert!(a
        .inbox_items()
        .iter()
        .any(|i| i.assigned && i.text == "assigned to Me"));
}

#[test]
fn whiteboard_strokes_and_notes_get_the_ids_every_app_works_out() {
    let mut d = Data::seed();
    d.extra.whiteboards = vec![Whiteboard {
        id: 4,
        name: "W".into(),
        strokes: vec![WbStroke {
            id: 0,
            color: [0, 0, 0],
            width: 2.0,
            points: vec![[0.1, 1.25], [1.1, 2.0]],
        }],
        notes: vec![Sticky {
            id: 0,
            pos: [1.0, 2.0],
            text: "old".into(),
            color: [1, 2, 3],
            ..Default::default()
        }],
        owner: 0,
        ..Default::default()
    }];
    // what a copy from before ids looks like to wb-sync
    let mut v = serde_json::to_value(&d).unwrap();
    crate::sync::fill_board_ids(&mut v);
    d.fix_activity_ids();
    let w = &d.extra.whiteboards[0];
    assert!(w.strokes[0].id >= 1 << 62 && w.notes[0].id >= 1 << 62);
    assert_eq!(
        v["extra"]["whiteboards"][0]["strokes"][0]["id"],
        w.strokes[0].id
    );
    assert_eq!(
        v["extra"]["whiteboards"][0]["notes"][0]["id"],
        w.notes[0].id
    );
    // and the ids are kept from then on
    let back: Data = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
    assert_eq!(back.extra.whiteboards[0].strokes[0].id, w.strokes[0].id);
}

#[test]
fn a_shared_folder_copy_from_an_older_app_is_pointed_out_once() {
    let dir = temp_home("old-app");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    // our own file says which version wrote it
    let mine = crate::sync::read_json(&dir.join(format!(
        "{}{}",
        a.data.extra.shared.as_ref().unwrap().device,
        crate::sync::FILE_SUFFIX
    )))
    .unwrap();
    assert_eq!(
        crate::sync::app_version_of(&mine),
        Some(together::APP_VERSION)
    );
    a.toast = None;
    a.sync_pull();
    assert!(a.toast.is_none(), "nobody else here yet");
    // a teammate's copy written by an app from before versions were written
    let mut old = serde_json::to_value(&a.data).unwrap();
    old["extra"]["sync"]["v"] = serde_json::json!(2);
    std::fs::write(
        dir.join(format!("old{}", crate::sync::FILE_SUFFIX)),
        old.to_string(),
    )
    .unwrap();
    a.sync_pull();
    let t = a.toast.take().map(|t| t.0).unwrap_or_default();
    assert!(t.contains("older version of Workboard"), "{}", t);
    a.sync_pull();
    assert!(a.toast.is_none(), "said once");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bookmarks_from_before_sharing_stay_personal_and_new_ones_are_shared() {
    // a file saved before bookmarks were shared
    let mut old = serde_json::to_value(Data::seed()).unwrap();
    old["extra"]["bookmarks"] =
        serde_json::json!([{"id": 5, "title": "My bank", "url": "https://bank.example"}]);
    old["extra"]
        .as_object_mut()
        .unwrap()
        .remove("bookmarks_shared");
    let mut d: Data = serde_json::from_value(old).unwrap();
    assert!(d.keep_old_bookmarks_personal());
    assert!(!d.keep_old_bookmarks_personal(), "only once");
    assert!(d.extra.bookmarks[0].personal);
    d.extra.bookmarks.push(Bookmark {
        id: 6,
        title: "Team wiki".into(),
        url: "https://wiki.example".into(),
        personal: false,
    });
    let shared = crate::sync::shareable(&serde_json::to_value(&d).unwrap());
    let titles: Vec<&str> = shared["extra"]["bookmarks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["title"].as_str().unwrap())
        .collect();
    assert_eq!(
        titles,
        vec!["Team wiki"],
        "only the new one goes to the team"
    );
    // both still show here, and a new workspace shares from the start
    let back: Data = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
    assert_eq!(back.extra.bookmarks.len(), 2);
    assert!(Data::seed().extra.bookmarks_shared);
}

/// Every sentence the team server can send an app (refusals, sign-in and
/// account errors) and the ones its client adds read in Korean, numbers
/// included. Texts only the server's command line prints are left out.
#[test]
fn server_refusals_are_shown_in_the_interface_language() {
    use crate::app::together::server_text;
    const CLI_ONLY: &[&str] = &[
        "Invite code for member {} as {}: {}  (works once, for {} days)",
        "No member with id {} in the workspace (see `workboard-server people`).",
        "Restored {}. Everyone's app gets it now (the copy from before is in {}/before-restore-*.json).",
        "This address takes Workboard's WebSocket connection.",
        // `workboard-server invite-email`: only the local admin request sees these
        "Type a valid ASCII email address (for example, member@example.test).",
        "Couldn't save invitation: {e}. No new code was issued.",
        "Set up a workspace before inviting teammates.",
        "Workspace metadata is malformed; no invitation was created.",
        "Workspace sync metadata is malformed; no invitation was created.",
        "Workspace has no members list.",
        "Several members have that email. Resolve the duplicate identities first.",
        "That email already has a sign-in account. Use sign-in or Forgot password instead.",
        "That member has an invalid id.",
        "Several members share that id. Resolve the duplicate identities first.",
        "That member's role is malformed.",
        "Only unregistered plain Members can be invited by email; privileged identities are protected.",
        "That member has already signed in. Use sign-in or account recovery instead.",
        "Invite code for member {} as Member: {}  (works once, for {} days)",
    ];
    let sentences = |src: &str| -> Vec<String> {
        let src = src.split("#[cfg(test)]\nmod tests").next().unwrap();
        let mut out = vec![];
        // each `"` followed by a capital letter, up to the next `"`
        let starts = src.match_indices('"').map(|(i, _)| i + 1);
        for part in starts.filter_map(|i| src[i..].split('"').next()) {
            let long = part.chars().count() > 10;
            if long
                && part.starts_with(|c: char| c.is_ascii_uppercase())
                && part.ends_with(['.', '?', '!', ')'])
                && !part.contains('\\')
                && !CLI_ONLY.contains(&part)
            {
                out.push(part.to_string());
            }
        }
        out
    };
    let mut all = sentences(include_str!("../../../server/src/lib.rs"));
    for src in [
        include_str!("../../../crates/wb-net/src/protocol.rs"),
        include_str!("../../../crates/wb-net/src/addr.rs"),
        include_str!("../../../crates/wb-net/src/client.rs"),
        include_str!("../../../crates/wb-net/src/http.rs"),
    ] {
        all.extend(sentences(src));
    }
    assert!(all.len() > 30, "found the texts: {}", all.len());
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let shown: Vec<(String, String)> = all
        .iter()
        .map(|s| {
            let filled = s.replace("{}", "5");
            (filled.clone(), server_text(&filled))
        })
        .collect();
    crate::i18n::set_lang(crate::i18n::Lang::En);
    let english: Vec<&String> = shown
        .iter()
        .filter(|(en, ko)| en == ko || !ko.chars().any(|c| ('\u{ac00}'..='\u{d7a3}').contains(&c)))
        .map(|(en, _)| en)
        .collect();
    assert!(english.is_empty(), "no Korean for: {:#?}", english);
    crate::i18n::set_lang(crate::i18n::Lang::Ko);
    let tries =
        server_text("Too many wrong tries. Wait 15 minutes and try again, or use Forgot password.");
    crate::i18n::set_lang(crate::i18n::Lang::En);
    assert!(tries.contains("15분"), "{}", tries);
}

#[test]
fn a_server_that_cannot_be_reached_is_said_plainly() {
    use crate::app::together::{net_problem, net_status_text, reach_text, FAILS_BEFORE_CANT_REACH};
    use crate::i18n::{set_lang, Lang};
    let refused = "Can't reach 127.0.0.1:9: Connection refused (os error 61)";
    let lost = wb_net::Status::Reconnecting(refused.into());
    set_lang(Lang::Ko);
    let said = [
        reach_text(refused),
        reach_text("Can't reach macmini:8787: Operation timed out (os error 60)"),
        reach_text("Can't find macmini: failed to lookup address information"),
        reach_text("IO error: Connection reset by peer (os error 54)"),
        reach_text("The server didn't answer like Workboard: HTTP error: 404 Not Found"),
        reach_text("The server stopped answering."),
        reach_text("The secure connection to wb.example.com failed: invalid peer certificate: UnknownIssuer"),
    ];
    let first = net_status_text(true, Some(&lost), 1);
    let later = net_status_text(true, Some(&lost), FAILS_BEFORE_CANT_REACH);
    let why = net_problem(Some(&lost)).unwrap();
    set_lang(Lang::En);
    for s in &said {
        assert!(
            s.chars().any(|c| ('\u{ac00}'..='\u{d7a3}').contains(&c))
                && !s.contains("os error")
                && !s.contains("refused")
                && !s.contains("lookup"),
            "plain Korean: {:?}",
            s
        );
    }
    assert!(said[0].contains("127.0.0.1:9") && said[0].contains("workboard-server"));
    assert!(said[1].contains("macmini:8787") && said[2].contains("macmini"));
    assert!(said[6].contains("wb.example.com") && !said[6].contains("UnknownIssuer"));
    // after a few failed tries it no longer says it is about to reconnect
    assert_eq!(first, "다시 연결하는 중\u{2026}");
    assert_eq!(later, "서버에 연결할 수 없음");
    assert!(why.starts_with(&said[0]), "{}", why);
    assert_eq!(
        net_status_text(true, Some(&wb_net::Status::Connected), 9),
        "Connected"
    );
}

#[test]
fn a_long_folder_path_is_cut_from_the_front_at_folder_names() {
    use crate::app::together::short_path;
    assert_eq!(short_path("/Users/me/Dropbox", 48), "/Users/me/Dropbox");
    let p = "/Users/someone/Library/Mobile Documents/com~apple~CloudDocs/team-shared/_shared";
    let s = short_path(p, 42);
    assert_eq!(s, "\u{2026}/com~apple~CloudDocs/team-shared/_shared");
    assert!(s.chars().count() <= 42);
    // one folder name longer than the room is kept whole
    assert_eq!(
        short_path("/a/a-very-long-folder-name-that-will-not-fit", 10),
        "\u{2026}/a-very-long-folder-name-that-will-not-fit"
    );
    // a Windows path keeps its backslashes
    assert_eq!(
        short_path(
            r"C:\Users\someone\OneDrive\Documents\team-shared\_shared",
            32
        ),
        "\u{2026}\\Documents\\team-shared\\_shared"
    );
}

/// Through a real server: Sam can only view a space Ann shares with him.
/// An edit his app makes there anyway is put back and he's told why; when
/// Ann takes the space away it leaves his copy, and nothing is deleted.
#[test]
fn an_edit_in_a_view_only_space_comes_back_with_a_word() {
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A sees the people list", |a| {
        a.sync_rt.people.iter().any(|p| p.member == 3)
    });
    let space = a
        .data
        .spaces
        .iter()
        .find(|s| s.private_owner.is_none() && !s.archived)
        .unwrap()
        .id;
    let task = a
        .data
        .tasks
        .iter()
        .find(|t| a.data.space_of_list(t.list_id).map(|s| s.id) == Some(space))
        .unwrap()
        .clone();
    a.data
        .spaces
        .iter_mut()
        .find(|s| s.id == space)
        .unwrap()
        .sharing = Some(SpaceSharing {
        everyone: false,
        people: vec![SpaceGrant {
            member: 3,
            level: "view".into(),
        }],
    });
    a.sync_note_change();
    a.write_shared();
    let code = invite_code(&mut a, 3, wb_net::Role::Member);
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.connect_server(&addr, &code).unwrap();
    net_until(&mut b, "Sam has the space, to view", |b| {
        b.data.extra.shared.is_some()
            && b.task(task.id).is_some()
            && b.space_level(space) == crate::sync::Level::View
    });
    // his app changes the task anyway (an older app, say)
    b.data
        .tasks
        .iter_mut()
        .find(|t| t.id == task.id)
        .unwrap()
        .name = "Sam was here".into();
    b.sync_note_change();
    b.write_shared();
    net_until(&mut b, "put back, with a word", |b| {
        b.task(task.id).is_some_and(|t| t.name == task.name)
            && b.toast.as_ref().is_some_and(|t| {
                t.0 == "Some changes were put back because that space is shared with you at a level that doesn't allow them."
            })
    });
    assert_eq!(a.task(task.id).unwrap().name, task.name);
    // taken away: it leaves Sam's copy without a tombstone
    a.data
        .spaces
        .iter_mut()
        .find(|s| s.id == space)
        .unwrap()
        .sharing
        .as_mut()
        .unwrap()
        .people
        .clear();
    a.sync_note_change();
    a.write_shared();
    net_until(&mut b, "the space leaves Sam's copy", |b| {
        b.task(task.id).is_none() && !b.data.spaces.iter().any(|s| s.id == space)
    });
    assert!(!b.data.extra.sync.tombs.contains_key(&task.id));
    b.sync_note_change();
    b.write_shared();
    net_until(&mut a, "A keeps it all", |a| a.task(task.id).is_some());
}

#[test]
fn just_made_an_admin_my_copy_keeps_a_space_the_server_sent() {
    let mut a = app();
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "d".into(),
        tag: 1,
        server: Some("127.0.0.1:1".into()),
    });
    // the server made me an admin, and the copy came before the word did
    a.sync_rt.role = Some(wb_net::Role::Member);
    let mut team = a.data.clone();
    let mut plans = team.spaces[0].clone();
    plans.id = 7400;
    plans.name = "Plans".into();
    plans.lists.clear();
    plans.folders.clear();
    plans.private_owner = None;
    plans.sharing = Some(SpaceSharing {
        everyone: false,
        people: vec![SpaceGrant {
            member: 999,
            level: "view".into(),
        }],
    });
    team.spaces.push(plans);
    // a DM between two other people still leaves, whatever my role
    team.channels.push(Channel {
        id: 7401,
        kind: ChannelKind::Direct,
        members: vec![2, 3],
        ..Default::default()
    });
    a.merge_in(&[serde_json::to_value(&team).unwrap()], &[]);
    assert!(
        a.data.spaces.iter().any(|s| s.id == 7400),
        "the server's word on what I may see stands"
    );
    assert!(!a.data.channels.iter().any(|c| c.id == 7401));
}

/// Teammates' changes that arrive in one frame are merged in together (one
/// merge over the workspace instead of one each), with the same result as
/// taking them one by one: every change counts, a later change to the same
/// task wins, and the last hidden list is the one that holds. Anything else
/// the server said in between is taken in its place.
#[test]
fn changes_arriving_together_are_merged_in_once() {
    let shared = || crate::sync::SharedCfg {
        folder: String::new(),
        device: "d".into(),
        tag: 1,
        server: Some("127.0.0.1:1".into()),
    };
    let mut a = app();
    a.data.extra.shared = Some(shared());
    a.sync_note_change();
    // a teammate with the same copy, and what the server already has
    let mut b = App::build(None, Some(a.data.clone()));
    b.data.extra.shared = Some(shared());
    let mut known = crate::sync::Summary::of(&crate::sync::shareable(
        &serde_json::to_value(&b.data).unwrap(),
    ));
    let (t0, t1) = (a.data.tasks[0].id, a.data.tasks[1].id);
    let mut change = |b: &mut App, edit: &dyn Fn(&mut Data)| {
        // a later change has a later clock
        std::thread::sleep(std::time::Duration::from_millis(3));
        edit(&mut b.data);
        b.sync_note_change();
        let d = crate::sync::delta(
            &known,
            &crate::sync::shareable(&serde_json::to_value(&b.data).unwrap()),
        )
        .expect("something changed");
        known.absorb(&d);
        wb_net::Event::Update {
            data: d,
            rev: 0,
            hidden: vec![],
        }
    };
    let first = change(&mut b, &|d| d.tasks[0].name = "Renamed once".into());
    let second = change(&mut b, &|d| d.tasks[1].name = "Other task renamed".into());
    let third = change(&mut b, &|d| d.tasks[0].name = "Renamed twice".into());

    let merges = a.data_gen.get();
    assert!(a.take_net_events(vec![first, second, third]));
    assert_eq!(a.data_gen.get(), merges + 1, "one merge for all three");
    assert_eq!(a.task(t0).unwrap().name, "Renamed twice");
    assert_eq!(a.task(t1).unwrap().name, "Other task renamed");

    // something else in between: the changes before it go in first
    let fourth = change(&mut b, &|d| d.tasks[1].name = "Fourth".into());
    let fifth = change(&mut b, &|d| d.tasks[1].name = "Fifth".into());
    let merges = a.data_gen.get();
    a.take_net_events(vec![fourth, wb_net::Event::Presence(vec![2]), fifth]);
    assert_eq!(a.data_gen.get(), merges + 2);
    assert_eq!(a.sync_rt.online, Some(vec![2]));
    assert_eq!(a.task(t1).unwrap().name, "Fifth");
}

#[test]
fn an_older_file_notes_changes_before_the_first_sync_field_by_field() {
    let mut a = app();
    a.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "d".into(),
        tag: 1,
        server: Some("127.0.0.1:1".into()),
    });
    let tid = a.data.tasks[0].id;
    a.data.tasks[0].attachments = vec![Attachment {
        file: "x".into(),
        name: "x.png".into(),
        ..Default::default()
    }];
    a.sync_note_change();
    // saved by an app from before field fingerprints were kept
    let m = &mut a.data.extra.sync;
    m.field_hashes.clear();
    m.fields.clear();
    m.v = 2;
    let team = serde_json::to_value(&a.data).unwrap();
    // opened again: unchanged items get their field fingerprints
    let mut b = App::build(None, Some(a.data.clone()));
    assert!(b.data.extra.sync.field_hashes.contains_key(&tid));
    // the last attachment removed before the first sync
    b.data
        .tasks
        .iter_mut()
        .find(|t| t.id == tid)
        .unwrap()
        .attachments
        .clear();
    b.sync_note_change();
    b.merge_in(&[team], &[]);
    let t = b.data.tasks.iter().find(|t| t.id == tid).unwrap();
    assert!(t.attachments.is_empty(), "it stays removed");
}

#[test]
fn updates_come_from_a_team_server_even_while_a_local_workspace_is_open() {
    let srv = |s: &str| Some(s.to_string());
    // the open workspace's own server comes first
    assert_eq!(
        together::pick_update_server(srv("https://a"), "https://b", [srv("https://c")]),
        srv("https://a")
    );
    // a workspace kept only on this computer: the server last signed in to
    assert_eq!(
        together::pick_update_server(None, "https://b", [srv("https://c")]),
        srv("https://b")
    );
    // never signed in on this version: any other workspace on a server
    assert_eq!(
        together::pick_update_server(None, "", [None, srv(""), srv("https://c")]),
        srv("https://c")
    );
    assert_eq!(together::pick_update_server(srv(" "), "", [None]), None);
}

/// My phone, signed in as me with the code my Mac shows: a bare
/// connection that keeps the last `mine` it heard.
struct Phone {
    client: wb_net::Client,
    mine: Option<serde_json::Value>,
}

impl Phone {
    fn pump(&mut self) {
        for e in self.client.events() {
            if let wb_net::Event::Mine(m) = e {
                self.mine = Some(m);
            }
        }
    }

    fn until(&mut self, what: &str, cond: impl Fn(&crate::sync::mine::MineView) -> bool) {
        let held = wait_until(
            std::time::Duration::from_secs(15),
            std::time::Duration::from_millis(20),
            || {
                self.pump();
                self.mine
                    .as_ref()
                    .is_some_and(|m| cond(&crate::sync::mine::resolve(m)))
            },
        );
        assert!(held, "timed out waiting for: {} ({:?})", what, self.mine);
    }
}

/// What I read, clear from the Inbox or choose for notifications on my Mac
/// shows on my phone, and what I do on the phone shows on the Mac: the
/// conversation no longer counts as unread there, nor in the Dock's number.
#[test]
fn my_reading_state_goes_between_my_mac_and_my_phone() {
    use crate::sync::mine::collect;
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "the server keeps my state", |a| a.sync_rt.mine_ok);

    a.ask_phone_link();
    net_until(&mut a, "a code for my phone", |a| a.sync_rt.phone.is_some());
    let code = a.sync_rt.phone.as_ref().unwrap().code.clone();
    let mut phone = Phone {
        client: wb_net::Client::start(
            &addr,
            wb_net::SignIn {
                device: "phone1".into(),
                link_code: Some(code),
                app: Some("test".into()),
                sync: Some(crate::sync::META_V),
                ..Default::default()
            },
            std::sync::Arc::new(|| {}),
        ),
        mine: None,
    };
    phone.until("the phone hears what the server keeps", |_| true);

    // on the Mac: clear an Inbox row, read a conversation, turn off DMs
    let ch = a.data.channels[0].id;
    a.data.extra.inbox_cleared.push("chat:1:2".into());
    let read = a.data.channels[0].last_read.max(now_ts()) + 10;
    a.data.channels[0].last_read = read;
    a.data.extra.notify.direct = false;
    a.send_mine();
    phone.until("the phone gets the Mac's state", |v| {
        v.inbox_cleared.contains("chat:1:2")
            && v.read.get(&ch) == Some(&read)
            && v.settings["notify"]["direct"] == serde_json::json!(false)
    });

    // someone writes; the Mac has seen everything before it
    let me = a.data.me;
    let other = a.data.members.iter().find(|m| m.id != me).unwrap().id;
    let ts = read + 5;
    a.data.channels[0]
        .messages
        .push(crate::app::slack::new_message(
            777_001,
            other,
            "new".into(),
            ts,
        ));
    let older: Vec<(u64, i64)> = a.data.channels[0]
        .messages
        .iter()
        .filter(|m| m.id != 777_001)
        .map(|m| (m.id, m.ts))
        .collect();
    a.data
        .extra
        .chat_seen
        .insert(ch, SeenIds::of(older.into_iter()));
    let c = a.data.channels[0].clone();
    assert_eq!(a.unread_here(&c), 1);

    // on the phone: read it, move the row back, save one for later, mute
    let known = phone.mine.clone().unwrap();
    let mut v = crate::sync::mine::resolve(&known);
    v.read.insert(ch, ts);
    v.inbox_cleared.clear();
    v.inbox_later.insert("t:9:9".into());
    v.replies_read.insert("chat:1:3".into());
    v.settings
        .insert("muted_until".into(), serde_json::json!(now_ts() + 3600));
    let later = chrono::Utc::now().timestamp_millis() + 1000;
    phone.client.send(wb_net::Msg::Mine {
        data: collect(&v, &known, later),
    });
    net_until(&mut a, "the Mac takes the phone's state", |a| {
        a.data.extra.inbox_later.contains(&"t:9:9".to_string())
    });
    assert!(a.data.extra.inbox_cleared.is_empty(), "the row is back");
    assert!(a.data.extra.replies_read.contains(&"chat:1:3".to_string()));
    assert!(a.data.extra.muted_until > now_ts());
    assert!(!a.data.extra.notify.direct, "the Mac's own choice stays");
    let c = a.data.channels[0].clone();
    assert_eq!(c.last_read, ts);
    assert_eq!(a.unread_here(&c), 0, "read on the phone: read here too");

    // nothing new on the Mac: nothing more goes up
    let before = a.data.extra.mine_sync.clone();
    a.send_mine();
    assert_eq!(a.data.extra.mine_sync, before);
}

/// Two computers on one goal through a server: what one records reaches
/// the other with its history and milestone, edits to different parts of
/// it both stay (a rename and a tick, or two different targets, with both
/// history lines), and a goal one of them deletes stays deleted even if
/// the other was changing it.
#[test]
fn a_goal_is_worked_on_from_two_computers() {
    use crate::app::more::GoalAct;
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = app();
    share_on(&mut a, &server);
    let code = invite_code_for_sam(&mut a, &server);
    let mut b = join_with(&addr, &code.to_lowercase());
    let (ann, sam) = (a.data.me, b.data.me);
    let push = |x: &mut App| {
        x.sync_note_change();
        x.write_shared();
    };
    // both send, then both take in what the other sent
    let settle = |a: &mut App, b: &mut App| {
        for _ in 0..60 {
            a.net_tick(None);
            b.net_tick(None);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    };
    let gid = a.add_goal(Goal {
        name: "Shared goal".into(),
        owner: ann,
        due: Some(today() + chrono::Duration::days(30)),
        targets: vec![
            GoalTarget {
                id: 0,
                name: "Signups".into(),
                kind: TargetKind::Number {
                    start: 0.0,
                    target: 10.0,
                    current: 0.0,
                    unit: String::new(),
                },
            },
            GoalTarget {
                id: 0,
                name: "Review".into(),
                kind: TargetKind::TrueFalse { done: false },
            },
        ],
        ..Default::default()
    });
    push(&mut a);
    let g_of = |x: &App| x.data.extra.goals.iter().find(|g| g.id == gid).cloned();
    net_until(&mut b, "the goal at B", |b| g_of(b).is_some());

    // Ann records 6 of 10 (30%): Sam sees it, her line and the 25% milestone
    a.goal_apply(GoalAct::SetNumber {
        g: gid,
        i: 0,
        to: 6.0,
    });
    push(&mut a);
    net_until(&mut b, "6 at B", |b| {
        matches!(g_of(b).unwrap().targets[0].kind,
            TargetKind::Number { current, .. } if current == 6.0)
    });
    let g = g_of(&b).unwrap();
    assert!(g
        .log
        .iter()
        .any(|e| e.who == ann && matches!(e.what, GoalChange::Number { to, .. } if to == 6.0)));
    assert!(g
        .log
        .iter()
        .any(|e| e.who == ann && e.what == GoalChange::Milestone(25)));
    // Sam's app takes it as it is: no second milestone of its own
    b.goal_watch(now_ts());
    assert_eq!(g_of(&b).unwrap().log.len(), g.log.len());

    // at once: Ann renames it, Sam ticks the review (80%): both stay
    for g in a.data.extra.goals.iter_mut().filter(|g| g.id == gid) {
        g.name = "Renamed by Ann".into();
    }
    b.goal_apply(GoalAct::ToggleDone { g: gid, i: 1 });
    push(&mut a);
    push(&mut b);
    settle(&mut a, &mut b);
    for x in [&a, &b] {
        let g = g_of(x).unwrap();
        assert_eq!(g.name, "Renamed by Ann");
        assert!(matches!(
            g.targets[1].kind,
            TargetKind::TrueFalse { done: true }
        ));
        assert_eq!(g.reached, 75);
        let m75 = g
            .log
            .iter()
            .filter(|e| e.what == GoalChange::Milestone(75))
            .count();
        assert_eq!(m75, 1, "one 75% milestone, Sam's: {:?}", g.log);
        assert!(g
            .log
            .iter()
            .any(|e| e.who == sam && e.what == GoalChange::Milestone(75)));
    }

    // at once, each on another target: both changes and both lines stay
    a.goal_apply(GoalAct::SetNumber {
        g: gid,
        i: 0,
        to: 8.0,
    });
    b.goal_apply(GoalAct::ToggleDone { g: gid, i: 1 });
    push(&mut a);
    push(&mut b);
    settle(&mut a, &mut b);
    for x in [&a, &b] {
        let g = g_of(x).unwrap();
        assert!(
            matches!(g.targets[0].kind, TargetKind::Number { current, .. } if current == 8.0),
            "Ann's number"
        );
        assert!(
            matches!(g.targets[1].kind, TargetKind::TrueFalse { done: false }),
            "Sam's untick"
        );
        assert!(g
            .log
            .iter()
            .any(|e| e.who == ann && matches!(e.what, GoalChange::Number { to, .. } if to == 8.0)));
        assert!(g
            .log
            .iter()
            .any(|e| e.who == sam && matches!(e.what, GoalChange::Done { done: false, .. })));
    }
    // one goal on both (lines written in the same second may sit in
    // either order)
    let same = |x: &App| {
        let mut g = g_of(x).unwrap();
        g.log.sort_by_key(|e| (e.ts, e.id));
        serde_json::to_value(&g).unwrap()
    };
    assert_eq!(same(&a), same(&b), "one goal on both");

    // Ann deletes it while Sam records: it stays deleted on both
    a.remove_item("goal", gid);
    b.goal_apply(GoalAct::SetNumber {
        g: gid,
        i: 0,
        to: 2.0,
    });
    push(&mut a);
    push(&mut b);
    settle(&mut a, &mut b);
    assert!(g_of(&a).is_none() && g_of(&b).is_none());
}

/// Posts go between computers through the server as they were written:
/// title, type, text and who they mention, from the Posts page and from a
/// channel's Post box. A post in a private channel reaches only its
/// members. A post written while the server is away goes once it is back,
/// once.
#[test]
fn posts_reach_the_team_through_a_server() {
    let home = workboard_server::testing::temp_home("posts-server");
    std::fs::create_dir_all(&home).unwrap();
    let server = TestServer::start_at(&home, 0);
    let port = server.port;
    let addr = format!("127.0.0.1:{}", port);
    let mut a = app();
    // #website-redesign is private to Me and Alex
    let web = a
        .data
        .channels
        .iter_mut()
        .find(|c| c.name == "website-redesign")
        .unwrap();
    web.private = true;
    web.members = vec![1, 2];
    let web = web.id;
    let gen = channel_id(&a, "general");
    share_on(&mut a, &server);
    let code = invite_code_for_sam(&mut a, &server);
    let mut b = join_with(&addr, &code.to_lowercase());
    assert_eq!(b.data.me, 3);
    assert!(
        !b.data.channels.iter().any(|c| c.id == web),
        "Sam doesn't get the private channel"
    );

    // from the Posts page (Me): a typed title, an Idea, a mention
    let p1 = a.add_post(
        gen,
        "Office move".into(),
        "Idea",
        "@Sam Lee boxes go Monday".into(),
        vec![],
        &[],
    );
    let p2 = a.add_post(web, "Secret".into(), "Update", "x".into(), vec![], &[]);
    a.sync_note_change();
    a.write_shared();
    net_until(&mut b, "A's post at B", |b| {
        b.data
            .channels
            .iter()
            .any(|c| c.messages.iter().any(|m| m.id == p1))
    });
    let m = b
        .data
        .channels
        .iter()
        .flat_map(|c| c.messages.iter())
        .find(|m| m.id == p1)
        .unwrap()
        .clone();
    assert_eq!(m.post_title.as_deref(), Some("Office move"));
    assert_eq!(m.post_type, "Idea");
    assert_eq!(m.text, "@Sam Lee boxes go Monday");
    assert_eq!(m.mentions, vec![3]);
    assert!(
        !b.data
            .channels
            .iter()
            .any(|c| c.messages.iter().any(|m| m.id == p2)),
        "the private channel's post stays with its members"
    );

    // from Sam's channel box (Post tab)
    b.apply_chat(vec![ChatAction::SendPost {
        ch: gen,
        title: "Lunch".into(),
        text: "Pizza at noon".into(),
        files: vec![],
    }]);
    b.sync_note_change();
    b.write_shared();
    net_until(&mut a, "B's post at A", |a| {
        a.data.channels.iter().any(|c| {
            c.messages
                .iter()
                .any(|m| m.post_title.as_deref() == Some("Lunch"))
        })
    });
    let m = a
        .data
        .channels
        .iter()
        .flat_map(|c| c.messages.iter())
        .find(|m| m.post_title.as_deref() == Some("Lunch"))
        .unwrap();
    assert_eq!(m.post_type, "Announcement");
    assert_eq!(m.author, 3);

    // the server goes away; Me posts; it comes back
    drop(server);
    net_until(&mut a, "A sees the server gone", |a| {
        matches!(a.sync_rt.net_status, Some(wb_net::Status::Reconnecting(_)))
    });
    let p3 = a.add_post(gen, "Offline".into(), "Question", "?".into(), vec![], &[]);
    a.sync_note_change();
    a.write_shared();
    let _server = TestServer::start_at(&home, port);
    net_until(&mut b, "the offline post at B", |b| {
        b.data
            .channels
            .iter()
            .any(|c| c.messages.iter().any(|m| m.id == p3))
    });
    for _ in 0..20 {
        a.net_tick(None);
        b.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let copies = |x: &App| {
        x.data
            .channels
            .iter()
            .flat_map(|c| c.messages.iter())
            .filter(|m| m.id == p3)
            .count()
    };
    assert_eq!((copies(&a), copies(&b)), (1, 1), "once on each computer");
}

/// A change made on a frame isn't stamped then (stamping fingerprints the
/// whole workspace, too slow for every key press): it is stamped when the
/// copy goes out, as made at the last change, not at the moment it went
/// out. So a teammate's edit made after it, and taken in before ours went
/// out, still wins over it, and one made before it still loses.
#[test]
fn a_change_is_stamped_when_it_goes_out_as_made_then() {
    let dir = temp_home("share-late-stamp");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.join_shared(&folder).unwrap();
    b.become_member(None, "Bora", "bora@team.kr");
    b.sync_note_change();
    b.write_shared();
    a.sync_pull();
    let t = a.data.tasks[0].id;
    let wait = || std::thread::sleep(std::time::Duration::from_millis(5));

    // A renames the task: noted, not stamped yet
    let before = a.data.extra.sync.stamps.get(&t).copied();
    a.data.tasks[0].name = "A's name".into();
    a.sync_mark_change();
    let at = a.sync_rt.unstamped.expect("noted");
    assert_eq!(a.data.extra.sync.stamps.get(&t).copied(), before);
    wait();
    // B renames it after that, and A takes B's copy in first
    b.sync_pull();
    b.data.tasks.iter_mut().find(|x| x.id == t).unwrap().name = "B's later name".into();
    b.sync_note_change();
    b.write_shared();
    wait();
    a.sync_pull();
    assert_eq!(a.sync_rt.unstamped, None, "stamped before the merge");
    assert_eq!(
        a.task(t).unwrap().name,
        "B's later name",
        "the later edit wins, though A's was stamped after it"
    );

    // an edit stamped as it goes out carries the time it was made
    a.data.tasks[0].name = "A again".into();
    a.sync_mark_change();
    let at2 = a.sync_rt.unstamped.unwrap();
    assert!(at2 > at);
    wait();
    a.write_shared();
    let id0 = a.data.tasks[0].id;
    assert_eq!(a.data.extra.sync.stamps.get(&id0).copied(), Some(at2));
    // B's older edit of it (made before A's, sent after) loses
    b.sync_pull();
    assert_eq!(b.task(id0).unwrap().name, "A again");
}

/// Taking in the team's copy when nothing in it is new keeps this copy as
/// it is (nothing to read again, nothing redrawn), but its bookkeeping
/// still learns what the team's copy knew; and when something is new, the
/// fingerprints come out just as working them all out again would give.
#[test]
fn taking_in_a_copy_with_nothing_new_keeps_this_one() {
    let dir = temp_home("share-nothing-new");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.join_shared(&folder).unwrap();
    b.become_member(None, "Bora", "bora@team.kr");
    b.sync_note_change();
    b.write_shared();
    assert!(a.sync_pull());
    let gen = a.data_gen.get();
    assert!(!a.merge_in(
        &[crate::sync::shareable(
            &serde_json::to_value(&b.data).unwrap()
        )],
        &[]
    ));
    assert_eq!(a.data_gen.get(), gen, "nothing to redraw");

    // something new: fingerprints as if all worked out again
    let ch = channel_id(&b, "general");
    b.apply_chat(vec![ChatAction::Send {
        ch,
        text: "new".into(),
        parent: None,
        files: vec![],
    }]);
    b.sync_note_change();
    b.write_shared();
    assert!(a.sync_pull());
    assert_ne!(a.data_gen.get(), gen);
    let mut v = serde_json::to_value(&a.data).unwrap();
    v["extra"]["sync"] = serde_json::json!({});
    assert_eq!(a.data.extra.sync.hashes, crate::sync::fingerprints(&v));
    assert_eq!(a.data.extra.sync.field_hashes, crate::sync::part_hashes(&v));
    assert_eq!(a.data.extra.sync.parents, crate::sync::parents_of(&v));
}

/// Passes bytes between apps and the server, counting what the server
/// sends, and can cut every connection through it at once (the computer
/// sleeping, the network going) while the server and the app keep running.
struct Cutter {
    port: u16,
    open: std::sync::Arc<std::sync::Mutex<Vec<std::net::TcpStream>>>,
    from_server: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl Cutter {
    fn new(to: u16) -> Cutter {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let open: std::sync::Arc<std::sync::Mutex<Vec<std::net::TcpStream>>> = Default::default();
        let from_server: std::sync::Arc<std::sync::atomic::AtomicUsize> = Default::default();
        let (list, count) = (open.clone(), from_server.clone());
        std::thread::spawn(move || {
            for app in listener.incoming() {
                let Ok(app) = app else { return };
                let Ok(server) = std::net::TcpStream::connect(("127.0.0.1", to)) else {
                    continue;
                };
                let mut l = list.lock().unwrap();
                l.push(app.try_clone().unwrap());
                l.push(server.try_clone().unwrap());
                for (mut from, mut to, counted) in [
                    (app.try_clone().unwrap(), server.try_clone().unwrap(), None),
                    (server, app, Some(count.clone())),
                ] {
                    std::thread::spawn(move || {
                        let mut buf = [0u8; 16 * 1024];
                        while let Ok(n) = from.read(&mut buf) {
                            if n == 0 || to.write_all(&buf[..n]).is_err() {
                                break;
                            }
                            if let Some(c) = &counted {
                                c.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                        let _ = to.shutdown(std::net::Shutdown::Both);
                    });
                }
            }
        });
        Cutter {
            port,
            open,
            from_server,
        }
    }

    fn cut(&self) {
        for s in self.open.lock().unwrap().drain(..) {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }

    /// Bytes the server sent since the last call.
    fn heard(&self) -> usize {
        self.from_server
            .swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

/// Back after the network dropped (the app kept running): only what
/// changed while away comes, not the whole workspace again, and it isn't
/// announced; what I wrote while away still reaches the team.
#[test]
fn a_reconnect_brings_only_what_changed_while_away() {
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let mut a = app();
    // a long history, so the whole workspace is easy to tell from a part
    let g = channel_id(&a, "general");
    let me = a.data.me;
    let gi = a.data.channels.iter().position(|c| c.id == g).unwrap();
    for i in 0..2000u64 {
        a.data.channels[gi]
            .messages
            .push(crate::app::slack::new_message(
                800_000 + i,
                me,
                format!("an old message, number {i}, from long ago"),
                1_000 + i as i64,
            ));
    }
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A shared on the server", |a| {
        a.data.extra.shared.is_some()
    });
    let code = invite_code_for_sam(&mut a, &server);
    let cutter = Cutter::new(server.port);
    let mut b = join_with(&format!("127.0.0.1:{}", cutter.port), &code);
    b.data.extra.notify.set_channel_mode(g, "all");
    let has = |b: &App, text: &str| {
        b.data
            .channels
            .iter()
            .any(|c| c.id == g && c.messages.iter().any(|m| m.text == text))
    };
    net_until(&mut b, "B has the history", |b| {
        has(b, "an old message, number 1999, from long ago")
    });
    let whole = cutter.heard();
    assert!(whole > 100_000, "the whole workspace came: {whole}");
    b.notify_scan();
    b.notify_rt.sent.clear();

    cutter.cut();
    net_until(&mut b, "B lost the server", |b| {
        matches!(b.sync_rt.net_status, Some(wb_net::Status::Reconnecting(_)))
    });
    let say = |a: &mut App, text: &str| {
        a.apply_chat(vec![ChatAction::Send {
            ch: g,
            text: text.into(),
            parent: None,
            files: vec![],
        }]);
        a.sync_note_change();
        a.write_shared();
    };
    for i in 0..3 {
        say(&mut a, &format!("while B was away {i}"));
    }
    say(&mut b, "B wrote this offline");
    cutter.heard();
    net_until(&mut b, "what A wrote while B was away", |b| {
        has(b, "while B was away 2")
    });
    net_until(&mut a, "what B wrote offline", |a| {
        has(a, "B wrote this offline")
    });
    let part = cutter.heard();
    assert!(
        part < whole / 5,
        "only what changed came: {part} of {whole}"
    );
    b.notify_scan();
    assert!(
        b.notify_rt.sent.is_empty(),
        "not announced: {:?}",
        b.notify_rt.sent
    );
    // once back, new messages are announced again
    say(&mut a, "back now?");
    net_until(&mut b, "the new message", |b| has(b, "back now?"));
    b.notify_scan();
    assert_eq!(b.notify_rt.sent.len(), 1, "{:?}", b.notify_rt.sent);
}
