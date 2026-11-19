// Behaviour tests for importing a Slack export: channels, threads and
// people, and how each person's match to a member is kept and shared
// between computers and versions.

use super::*;

/// A small Slack export: two people, #general with a thread and a file,
/// and #random. `Mia` is a member here by email, `ghost` isn't.
fn slack_export() -> std::path::PathBuf {
    let root = temp_home("slack").join("My Slack export");
    std::fs::create_dir_all(root.join("general")).unwrap();
    std::fs::create_dir_all(root.join("random")).unwrap();
    let w = |p: &str, v: serde_json::Value| std::fs::write(root.join(p), v.to_string()).unwrap();
    w(
        "users.json",
        serde_json::json!([
            {"id": "U1", "name": "mia", "real_name": "Mia Park", "profile": {"email": "MIA@example.com", "display_name": "mia"}},
            {"id": "U2", "name": "ghost", "real_name": "Casper Ghost", "profile": {"email": "casper@example.com"}}
        ]),
    );
    w(
        "channels.json",
        serde_json::json!([
            {"id": "C1", "name": "general", "members": ["U1", "U2"], "topic": {"value": "Everything"}},
            {"id": "C2", "name": "random", "members": ["U1"]}
        ]),
    );
    w(
        "general/2023-11-14.json",
        serde_json::json!([
            {"type": "message", "subtype": "channel_join", "user": "U2", "text": "<@U2> has joined the channel", "ts": "1699999000.000100"},
            {"type": "message", "user": "U1", "text": "Plan for <#C2|random> &amp; <https://example.com|the site>, cc <@U2>", "ts": "1700000000.000200", "thread_ts": "1700000000.000200"},
            {"type": "message", "user": "U2", "text": "on it", "ts": "1700000100.000300", "thread_ts": "1700000000.000200"},
            {"type": "message", "user": "U2", "text": "", "ts": "1700000200.000400", "files": [{"name": "spec.pdf", "permalink": "https://slack.example/spec"}]}
        ]),
    );
    w(
        "random/2023-11-15.json",
        serde_json::json!([{"type": "message", "user": "U1", "text": "hello random", "ts": "1700086400.000000"}]),
    );
    root.parent().unwrap().to_path_buf()
}

#[test]
fn slack_markup_becomes_plain_text() {
    use crate::app::slack::{plain_text, ts_secs};
    let names: HashMap<String, String> = [("U1".to_string(), "Mia Park".to_string())].into();
    assert_eq!(
        plain_text(
            "hi <@U1> see <#C9|dev> and <https://a.b|docs> or <https://c.d> &lt;3 <!here>",
            &names
        ),
        "hi @Mia Park see #dev and docs (https://a.b) or https://c.d <3 @here"
    );
    assert_eq!(
        plain_text("<mailto:x@y.z|x@y.z> <@U7>", &names),
        "x@y.z @U7"
    );
    assert_eq!(ts_secs("1700000000.000200"), 1_700_000_000);
}

#[test]
fn a_slack_export_comes_in_as_channels_with_threads_and_times() {
    let mut a = app();
    let mia = a.data.new_id();
    a.data.members.push(Member {
        id: mia,
        name: "Someone Else".into(),
        color: [1, 2, 3],
        email: "mia@example.com".into(),
        role: "Member".into(),
        invited: false,
        status_text: String::new(),
        slack: vec![],
        slack_mine: vec![],
        ..Default::default()
    });
    let before = a.data.members.len();
    let (users, channels) = crate::app::slack::read_export(&slack_export()).unwrap();
    let plan = crate::app::slack::plan(&a.data, users, channels);
    assert_eq!(plan.matched("U1"), Some(mia), "matched by email");
    assert_eq!(plan.unmatched(), vec!["U2".to_string()]);
    assert_eq!(plan.message_count(), 4, "the join notice isn't a message");
    let (chans, msgs) = a.import_slack(&plan);
    assert_eq!((chans, msgs), (2, 4));
    assert_eq!(a.data.members.len(), before + 1, "Casper is added");
    let casper = a
        .data
        .members
        .iter()
        .find(|m| m.name == "Casper Ghost")
        .unwrap();
    assert_eq!(casper.email, "casper@example.com");
    let casper = casper.id;
    let general = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general" && c.kind == ChannelKind::Channel)
        .unwrap()
        .clone();
    let plan_msg = general
        .messages
        .iter()
        .find(|m| m.ts == 1_700_000_000)
        .expect("the original time");
    assert_eq!(plan_msg.author, mia);
    assert_eq!(
        plan_msg.text,
        "Plan for #random & the site (https://example.com), cc @Casper Ghost"
    );
    assert_eq!(plan_msg.replies.len(), 1, "the reply is in its thread");
    assert_eq!(plan_msg.replies[0].author, casper);
    assert_eq!(plan_msg.replies[0].ts, 1_700_000_100);
    let file = general
        .messages
        .iter()
        .find(|m| m.ts == 1_700_000_200)
        .unwrap();
    assert_eq!(
        file.text, "[spec.pdf] https://slack.example/spec",
        "files as links"
    );
    let random = a.data.channels.iter().find(|c| c.name == "random").unwrap();
    assert_eq!(random.topic, "");
    assert!(random.members.contains(&mia) && random.members.contains(&a.data.me));
    assert_eq!(a.unread(random), 0, "old messages coming in aren't unread");
    // the same export again adds nothing
    let (users, channels) = crate::app::slack::read_export(&slack_export()).unwrap();
    let again = crate::app::slack::plan(&a.data, users, channels);
    assert!(again.unmatched().is_empty(), "Casper is a member now");
    assert_eq!(a.import_slack(&again), (0, 0));
}

#[test]
fn people_not_added_from_slack_come_in_under_my_name() {
    let mut a = app();
    let (users, channels) = crate::app::slack::read_export(&slack_export()).unwrap();
    let mut plan = crate::app::slack::plan(&a.data, users, channels);
    for id in plan.unmatched() {
        plan.assign.insert(id, crate::app::slack::Assign::Me);
    }
    let before = a.data.members.len();
    a.import_slack(&plan);
    assert_eq!(a.data.members.len(), before);
    let general = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap();
    let m = general
        .messages
        .iter()
        .find(|m| m.ts == 1_700_000_000)
        .unwrap();
    assert_eq!(m.author, a.data.me);
    assert!(m.text.starts_with("Mia Park: Plan for"), "{}", m.text);
}

#[cfg_attr(
    not(target_os = "macos"),
    ignore = "pictures and Slack zips need macOS until the Windows media work (src/media.rs)"
)]
#[test]
fn a_slack_export_zip_opens_and_anything_else_says_why() {
    let dir = slack_export();
    let zip = temp_home("slackzip").with_extension("zip");
    let ok = std::process::Command::new("/usr/bin/ditto")
        .args(["-c", "-k", "--keepParent"])
        .arg(dir.join("My Slack export"))
        .arg(&zip)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let mut a = app();
    a.open_slack_export(&zip);
    match &a.modal {
        Some(crate::app::menus::Modal::SlackImport(p)) => assert_eq!(p.channels.len(), 2),
        _ => panic!("the import dialog opens"),
    }
    a.modal = None;
    let junk = temp_home("junk").with_extension("zip");
    std::fs::write(&junk, b"not a zip").unwrap();
    a.open_slack_export(&junk);
    assert!(a.modal.is_none());
    assert!(a.toast.as_ref().unwrap().0.contains(".zip"));
}

/// Write a Slack export folder from (file, JSON) pairs and read it.
fn slack_from(
    files: &[(&str, serde_json::Value)],
) -> (
    Vec<crate::app::slack::SlackUser>,
    Vec<crate::app::slack::SlackChannel>,
) {
    let root = temp_home("slack2");
    for (p, v) in files {
        let path = root.join(p);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, v.to_string()).unwrap();
    }
    let out = crate::app::slack::read_export(&root).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    out
}

#[test]
fn a_private_slack_channel_never_pours_into_a_public_one_or_goes_to_everyone() {
    use serde_json::json;
    let mut a = app();
    a.apply_chat(vec![crate::app::chat::ChatAction::NewChannel(
        "design".into(),
    )]);
    let public = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "design")
        .unwrap()
        .id;
    let (users, channels) = slack_from(&[
        (
            "users.json",
            json!([{"id": "U9", "name": "zed", "real_name": "Zed Nobody", "profile": {}}]),
        ),
        (
            "channels.json",
            json!([{"id": "C1", "name": "design", "members": ["U9"]}]),
        ),
        (
            "groups.json",
            json!([{"id": "G1", "name": "design", "members": ["U9"]}]),
        ),
        (
            "design/2024-01-01.json",
            json!([{"type": "message", "user": "U9", "text": "secret plans", "ts": "1704100000.000100"}]),
        ),
    ]);
    let mut plan = crate::app::slack::plan(&a.data, users, channels);
    // Zed comes in under my name: no member to share the private one with
    plan.assign
        .insert("U9".into(), crate::app::slack::Assign::Me);
    let ((chans, _), alone) = a.import_slack_report(&plan);
    assert_eq!(chans, 2);
    assert_eq!(alone, vec!["design".to_string()], "and the dialog says so");
    let private: Vec<&Channel> = a
        .data
        .channels
        .iter()
        .filter(|c| c.kind == ChannelKind::Channel && c.private)
        .collect();
    assert_eq!(private.len(), 1, "a channel of its own");
    assert_eq!(private[0].name, "design-private", "named apart");
    assert_eq!(
        private[0].members,
        vec![a.data.me],
        "only me, not every member"
    );
    let pubc = a.data.channels.iter().find(|c| c.id == public).unwrap();
    assert!(!pubc.private);
    assert_eq!(pubc.messages.len(), 1, "the public one gets its own copy");
    // again: each lands in its own channel and nothing doubles
    let (users, channels) = slack_from(&[
        ("users.json", json!([])),
        ("groups.json", json!([{"id": "G1", "name": "design"}])),
        (
            "design/2024-01-01.json",
            json!([{"type": "message", "user": "U9", "text": "secret plans", "ts": "1704100000.000100"}]),
        ),
    ]);
    let plan = crate::app::slack::plan(&a.data, users, channels);
    assert_eq!(a.import_slack(&plan), (0, 0));
    assert_eq!(
        a.data
            .channels
            .iter()
            .filter(|c| c.name.starts_with("design"))
            .count(),
        2
    );
}

#[test]
fn imported_mentions_and_replies_are_not_news_in_anyones_inbox() {
    use serde_json::json;
    let mut a = app();
    let me = a.data.me;
    let my_name = a.data.member(me).unwrap().name.clone();
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == me)
        .unwrap()
        .email = "me@example.com".into();
    let sam = a
        .data
        .members
        .iter()
        .find(|m| m.name == "Sam Lee")
        .unwrap()
        .id;
    let badge = a.inbox_badge();
    let (users, channels) = slack_from(&[
        (
            "users.json",
            json!([
                {"id": "UME", "name": "me", "real_name": "Whoever", "profile": {"email": "me@example.com"}},
                {"id": "US", "name": "sam", "real_name": "Sam Lee", "profile": {}}
            ]),
        ),
        (
            "channels.json",
            json!([{"id": "C1", "name": "old-news", "members": ["UME", "US"]}]),
        ),
        (
            "old-news/2024-01-01.json",
            json!([
                {"type": "message", "user": "US", "text": "hey <@UME>", "ts": "1704100000.000100"},
                {"type": "message", "user": "UME", "text": "my thread", "ts": "1704100100.000100", "thread_ts": "1704100100.000100"},
                {"type": "message", "user": "US", "text": "a reply", "ts": "1704100200.000100", "thread_ts": "1704100100.000100"}
            ]),
        ),
    ]);
    let plan = crate::app::slack::plan(&a.data, users, channels);
    assert_eq!(plan.matched("UME"), Some(me));
    assert_eq!(plan.matched("US"), Some(sam));
    a.import_slack(&plan);
    let ch = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "old-news")
        .unwrap()
        .clone();
    assert!(ch.messages.iter().all(|m| m.imported.is_some()));
    let mention = ch.messages.iter().find(|m| m.author == sam).unwrap();
    assert_eq!(mention.text, format!("hey @{}", my_name));
    let rows: Vec<_> = a
        .inbox_items()
        .iter()
        .cloned()
        .filter(|i| matches!(i.src, crate::app::nav::InboxSrc::Chat { ch: c, .. } if c == ch.id))
        .collect();
    assert_eq!(rows.len(), 2, "the mention and the reply are in the Inbox");
    assert!(
        rows.iter().all(|r| r.tab == 3 && r.imported),
        "but already cleared"
    );
    assert_eq!(a.inbox_badge(), badge, "the badge doesn't move");
    a.notify_rt.seeded = true;
    let keys: Vec<String> = a
        .notify_candidates(&crate::model::NotifySettings::everything())
        .into_iter()
        .map(|(_, n)| n.key)
        .collect();
    assert!(
        rows.iter().all(|r| !keys.contains(&r.key)),
        "never announced"
    );
    // another member who had seen this channel long ago: not unread for them
    let mut seen = ch.clone();
    seen.last_read = 0;
    assert_eq!(a.unread_here(&seen), 0);
    // a real reply written now in my thread still is news
    let thread = ch.messages.iter().find(|m| m.author == me).unwrap().id;
    let c = a.data.channels.iter_mut().find(|c| c.id == ch.id).unwrap();
    let t = c.messages.iter_mut().find(|m| m.id == thread).unwrap();
    let mut live = t.replies[0].clone();
    live.id = 987_654;
    live.imported = None;
    live.ts = now_ts();
    t.replies.push(live);
    assert_eq!(a.inbox_badge(), badge + 1);
}

#[test]
fn slack_people_are_not_merged_by_name_and_same_second_messages_all_come_in() {
    use serde_json::json;
    let mut a = app();
    let alex = a
        .data
        .members
        .iter()
        .find(|m| m.name == "Alex Kim")
        .unwrap()
        .id;
    // two members called Sam Lee here
    let sam2 = a.data.new_id();
    a.data.members.push(Member {
        id: sam2,
        name: "Sam Lee".into(),
        color: [1, 2, 3],
        email: String::new(),
        role: "Member".into(),
        invited: false,
        status_text: String::new(),
        slack: vec![],
        slack_mine: vec![],
        ..Default::default()
    });
    let export = [
        (
            "users.json",
            json!([
                {"id": "UA1", "name": "alex", "real_name": "Alex Kim", "profile": {}},
                {"id": "UA2", "name": "alexk", "real_name": "Alex Kim", "profile": {}},
                {"id": "US", "name": "sam", "real_name": "Sam Lee", "profile": {}},
                {"id": "UJ", "name": "jordan", "real_name": "Jordan Park", "profile": {}}
            ]),
        ),
        (
            "channels.json",
            json!([{"id": "C1", "name": "busy", "members": ["UA1", "UA2", "US", "UJ"]}]),
        ),
        (
            "busy/2024-01-01.json",
            json!([
                {"type": "message", "user": "UJ", "text": "+1", "ts": "1704100000.000100"},
                {"type": "message", "user": "UJ", "text": "+1", "ts": "1704100000.000200"},
                {"type": "message", "user": "UA1", "text": "hi", "ts": "1704100001.000100"},
                {"type": "message", "user": "UA2", "text": "hi", "ts": "1704100002.000100"},
                {"type": "message", "user": "US", "text": "yo", "ts": "1704100003.000100"}
            ]),
        ),
    ];
    let (users, channels) = slack_from(&export);
    let mut plan = crate::app::slack::plan(&a.data, users, channels);
    assert_eq!(
        plan.matched("UA1"),
        None,
        "two Slack Alexes: neither matched"
    );
    assert_eq!(plan.matched("UA2"), None);
    assert_eq!(plan.matched("US"), None, "two Sams here: can't tell which");
    assert!(
        plan.matched("UJ").is_some(),
        "one name, one member: matched"
    );
    assert_eq!(plan.people, vec!["UJ", "UA1", "UA2", "US"]);
    assert!(plan.shared_members().is_empty());
    // someone points both Alexes at Alex: allowed, and said
    plan.assign
        .insert("UA1".into(), crate::app::slack::Assign::Member(alex));
    plan.assign
        .insert("UA2".into(), crate::app::slack::Assign::Member(alex));
    assert_eq!(
        plan.shared_members(),
        vec![(alex, vec!["UA1".to_string(), "UA2".to_string()])]
    );
    plan.assign
        .insert("US".into(), crate::app::slack::Assign::Me);
    let members = a.data.members.len();
    assert_eq!(a.import_slack(&plan), (1, 5), "both +1s in the same second");
    assert_eq!(a.data.members.len(), members, "nobody new");
    let busy = a.data.channels.iter().find(|c| c.name == "busy").unwrap();
    let me = a.data.me;
    assert!(busy
        .messages
        .iter()
        .any(|m| m.author == me && m.text == "Sam Lee: yo"));
    assert_eq!(busy.messages.iter().filter(|m| m.author == alex).count(), 2);
    // the same export again adds nothing
    let (users, channels) = slack_from(&export);
    let mut again = crate::app::slack::plan(&a.data, users, channels);
    again
        .assign
        .insert("UA1".into(), crate::app::slack::Assign::Member(alex));
    again
        .assign
        .insert("UA2".into(), crate::app::slack::Assign::Member(alex));
    again
        .assign
        .insert("US".into(), crate::app::slack::Assign::Me);
    assert_eq!(a.import_slack(&again), (0, 0));
}

#[test]
fn importing_the_same_slack_export_again_makes_no_new_members() {
    use serde_json::json;
    let mut a = app();
    // a second Sam Lee here, so Slack's Sam (no email) can't be matched
    let sam2 = a.data.new_id();
    a.data.members.push(Member {
        id: sam2,
        name: "Sam Lee".into(),
        color: [1, 2, 3],
        ..Default::default()
    });
    let export = [
        (
            "users.json",
            json!([
                {"id": "US", "name": "sam", "real_name": "Sam Lee", "profile": {}},
                {"id": "UZ", "name": "zed", "real_name": "Zed Nobody", "profile": {}},
                {"id": "UQ", "name": "quiet", "real_name": "Quinn Quiet", "profile": {}},
                {"id": "UO", "name": "old", "real_name": "Olga Old", "profile": {}}
            ]),
        ),
        (
            "channels.json",
            json!([{"id": "C1", "name": "again", "members": ["US", "UZ", "UQ", "UO"]}]),
        ),
        (
            "again/2024-01-01.json",
            json!([
                {"type": "message", "user": "US", "text": "first", "ts": "1704100000.000100"},
                {"type": "message", "user": "UZ", "text": "second", "ts": "1704100001.000100"},
                {"type": "message", "user": "UO", "text": "third", "ts": "1704100002.000100"}
            ]),
        ),
    ];
    let (users, channels) = slack_from(&export);
    let mut plan = crate::app::slack::plan(&a.data, users, channels);
    assert_eq!(plan.matched("US"), None);
    // Olga comes in under my name
    plan.assign
        .insert("UO".into(), crate::app::slack::Assign::Me);
    let members = a.data.members.len();
    assert_eq!(a.import_slack(&plan), (1, 3));
    assert_eq!(
        a.data.members.len(),
        members + 2,
        "Sam and Zed are new; Quinn wrote nothing and Olga is under my name"
    );
    let sam = *a
        .data
        .extra
        .slack_map
        .get("US")
        .expect("the new Sam is remembered for the Slack id");
    assert!(sam != sam2);
    assert_eq!(a.data.member(sam).unwrap().name, "Sam Lee");
    // Zed is renamed here before the next import
    let zed = a
        .data
        .members
        .iter_mut()
        .find(|m| m.name == "Zed Nobody")
        .unwrap();
    zed.name = "Z".into();
    let zed = zed.id;
    assert_eq!(slack_ids(&a.data, zed), ["UZ"]);
    let members = a.data.members.len();
    for round in 0..2 {
        let (users, channels) = slack_from(&export);
        let plan = crate::app::slack::plan(&a.data, users, channels);
        assert_eq!(plan.matched("US"), Some(sam), "round {}", round);
        assert_eq!(plan.matched("UZ"), Some(zed), "renamed, still found");
        let saved = serde_json::to_string(&a.data.members).unwrap();
        assert_eq!(a.import_slack(&plan), (0, 0), "round {}", round);
        assert_eq!(a.data.members.len(), members, "round {}: nobody new", round);
        assert_eq!(
            serde_json::to_string(&a.data.members).unwrap(),
            saved,
            "round {}: members unchanged",
            round
        );
        assert!(
            a.data.members.iter().all(|m| m.slack.is_empty()),
            "nothing is kept on the members any more"
        );
    }
    // a teammate's computer has the shared workspace: their import of the
    // same export finds the same members
    let mut b = App::build(None, Some(a.data.clone()));
    let (users, channels) = slack_from(&export);
    let plan = crate::app::slack::plan(&b.data, users, channels);
    assert_eq!(plan.matched("US"), Some(sam));
    assert_eq!(b.import_slack(&plan), (0, 0));
    assert_eq!(b.data.members.len(), members);
    // an import from before Slack ids were kept: nothing to add, so nobody
    // is made even though Sam can't be matched
    b.data.extra.slack_map.clear();
    let (users, channels) = slack_from(&export);
    let plan = crate::app::slack::plan(&b.data, users, channels);
    assert_eq!(plan.matched("US"), None, "three Sams now");
    assert_eq!(b.import_slack(&plan), (0, 0));
    assert_eq!(
        b.data.members.len(),
        members,
        "an empty import makes no one"
    );
}

#[test]
fn slack_member_choices_differ_for_members_of_one_name() {
    let m = |id: u64, name: &str, email: &str, role: &str| Member {
        id,
        name: name.into(),
        email: email.into(),
        role: role.into(),
        ..Default::default()
    };
    let list = crate::app::slack::member_choices(&[
        m(1, "Kim", "kim@a.com", "Member"),
        m(2, "Kim", "", "Admin"),
        m(10_003, "Kim", "", "Member"),
        m(20_004, "Kim", "", "Member"),
        m(5, "Lee", "", "Member"),
    ]);
    let labels: Vec<&str> = list.iter().map(|x| x.1.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Kim (kim@a.com)",
            "Kim (Admin)",
            "Kim (Member) #0003",
            "Kim (Member) #0004",
            "Lee"
        ]
    );
}

/// The member a Slack person was pointed at last is the one the next
/// import starts from: choosing another member moves the remembered Slack id
/// (it isn't left on both, where the first would win), and "under my name"
/// is remembered as my own choice even when the person matches a member by
/// email, without taking them off the member the team matched them to.
#[test]
fn a_changed_slack_match_sticks_on_the_next_import() {
    use crate::app::slack::Assign;
    use serde_json::json;
    let mut a = app();
    let (alex, sam) = (2, 3);
    // Alex's address: matched by email to Alex at first
    let export = [
        (
            "users.json",
            json!([{"id": "UA", "name": "al", "real_name": "Al", "profile": {"email": "alex@example.com"}}]),
        ),
        (
            "channels.json",
            json!([{"id": "C1", "name": "moves", "members": ["UA"]}]),
        ),
        (
            "moves/2024-01-01.json",
            json!([{"type": "message", "user": "UA", "text": "hello", "ts": "1704100000.000100"}]),
        ),
    ];
    let slack_of = |a: &App, m: u64| slack_ids(&a.data, m);
    let mine = |a: &App| a.data.extra.slack_under_me.clone();
    let fresh = |a: &App| {
        let (users, channels) = slack_from(&export);
        crate::app::slack::plan(&a.data, users, channels)
    };

    let plan = fresh(&a);
    assert_eq!(plan.matched("UA"), Some(alex));
    a.import_slack(&plan);
    assert_eq!(slack_of(&a, alex), ["UA"]);

    // pointed at Sam instead: remembered on Sam only
    let mut plan = fresh(&a);
    plan.assign.insert("UA".into(), Assign::Member(sam));
    a.import_slack(&plan);
    assert_eq!(slack_of(&a, sam), ["UA"]);
    assert!(slack_of(&a, alex).is_empty(), "taken off Alex");
    // and the import after that keeps Sam
    let plan = fresh(&a);
    assert_eq!(plan.matched("UA"), Some(sam));
    a.import_slack(&plan);
    assert_eq!(fresh(&a).matched("UA"), Some(sam));
    assert_eq!(a.data.extra.slack_map.get("UA"), Some(&sam));

    // under my name: Sam keeps the shared match, and my next import (which
    // could match Alex by email) starts from my name
    let mut plan = fresh(&a);
    plan.assign.insert("UA".into(), Assign::Me);
    a.import_slack(&plan);
    assert_eq!(slack_of(&a, sam), ["UA"], "the team's match stays");
    assert!(slack_of(&a, alex).is_empty());
    assert_eq!(mine(&a), ["UA"]);
    let plan = fresh(&a);
    assert_eq!(plan.assign.get("UA"), Some(&Assign::Me));
    assert_eq!(plan.matched("UA"), None);
    a.import_slack(&plan);
    assert_eq!(fresh(&a).assign.get("UA"), Some(&Assign::Me), "still mine");

    // a teammate's computer (the copy they get): my choice is mine, they
    // still go to Sam as the team decided
    let mut b = App::build(None, Some(teammate_copy(&a.data, sam)));
    assert!(b.data.extra.slack_under_me.is_empty());
    assert_eq!(fresh(&b).matched("UA"), Some(sam));
    b.data.me = 4;
    assert_eq!(fresh(&b).matched("UA"), Some(sam));

    // pointed back at Alex: my choice goes and the match moves to Alex
    let mut plan = fresh(&a);
    plan.assign.insert("UA".into(), Assign::Member(alex));
    a.import_slack(&plan);
    assert!(mine(&a).is_empty());
    assert_eq!(slack_of(&a, alex), ["UA"]);
    assert!(slack_of(&a, sam).is_empty());
    assert_eq!(fresh(&a).matched("UA"), Some(alex));

    // as a new member: nothing new comes in so nobody is made, and with
    // nobody to move them to, Alex keeps the match
    let mut plan = fresh(&a);
    plan.assign.insert("UA".into(), Assign::New);
    let members = a.data.members.len();
    a.import_slack(&plan);
    assert_eq!(a.data.members.len(), members);
    assert_eq!(slack_of(&a, alex), ["UA"]);
    assert!(mine(&a).is_empty());
    assert!(a.data.members.iter().all(|m| m.slack_mine.is_empty()));
}

/// The copy of a workspace a teammate gets (`shareable`), opened as them.
fn teammate_copy(d: &Data, me: u64) -> Data {
    let v = crate::sync::shareable(&serde_json::to_value(d).unwrap());
    let mut t: Data = serde_json::from_value(v).unwrap();
    t.me = me;
    t
}

/// A Slack export with the person UA (Alex's email address) and, in the
/// channel #moves, a message "msg N" by `user` for each (N, user).
fn slack_moves(
    msgs: &[(u32, &str)],
) -> (
    Vec<crate::app::slack::SlackUser>,
    Vec<crate::app::slack::SlackChannel>,
) {
    use serde_json::json;
    let day: Vec<serde_json::Value> = msgs
        .iter()
        .map(|(n, u)| {
            json!({"type": "message", "user": u, "text": format!("msg {}", n),
                   "ts": format!("{}.000100", 1704100000 + n)})
        })
        .collect();
    slack_from(&[
        (
            "users.json",
            json!([{"id": "UA", "name": "al", "real_name": "Al", "profile": {"email": "alex@example.com"}}]),
        ),
        (
            "channels.json",
            json!([{"id": "C1", "name": "moves", "members": ["UA"]}]),
        ),
        ("moves/2024-01-01.json", json!(day)),
    ])
}

/// Who message "msg N" came in as, and its text.
fn slack_msg(a: &App, n: u32) -> (u64, String) {
    let tail = format!("msg {}", n);
    a.data
        .channels
        .iter()
        .flat_map(|c| c.messages.iter())
        .find(|m| m.text.ends_with(&tail))
        .map(|m| (m.author, m.text.clone()))
        .unwrap_or_else(|| panic!("no {}", tail))
}

/// "Import under my name" is my own choice: a teammate's import, which
/// points the same Slack person somewhere else, doesn't erase it on my
/// computer, and the copy they get doesn't carry it.
#[test]
fn my_slack_under_my_name_choice_survives_a_teammates_import() {
    use crate::app::slack::{plan, Assign};
    let mut a = app();
    let (alex, sam, jordan) = (2, 3, 4);
    let msgs = vec![(1, "UA")];
    let (u, c) = slack_moves(&msgs);
    let mut p = plan(&a.data, u, c);
    assert_eq!(p.matched("UA"), Some(alex), "by email");
    p.assign.insert("UA".into(), Assign::Me);
    a.import_slack(&p);
    assert_eq!(a.data.extra.slack_under_me, ["UA"]);
    assert_eq!(slack_msg(&a, 1), (a.data.me, "Al: msg 1".into()));

    // Sam's computer: nothing of my choice; they point UA at Jordan
    let mut b = App::build(None, Some(teammate_copy(&a.data, sam)));
    assert!(b.data.extra.slack_under_me.is_empty());
    let (u, c) = slack_moves(&msgs);
    let mut p = plan(&b.data, u, c);
    assert_eq!(p.assign.get("UA"), Some(&Assign::Member(alex)));
    p.assign.insert("UA".into(), Assign::Member(jordan));
    b.import_slack(&p);
    assert_eq!(slack_ids(&b.data, jordan), ["UA"]);

    // their match comes over to my computer (sync): my choice is still mine
    a.data.extra.slack_map = b.data.extra.slack_map.clone();
    assert_eq!(a.data.extra.slack_under_me, ["UA"]);
    let (u, c) = slack_moves(&msgs);
    assert_eq!(plan(&a.data, u, c).assign.get("UA"), Some(&Assign::Me));
    // and theirs is Jordan
    let (u, c) = slack_moves(&msgs);
    assert_eq!(plan(&b.data, u, c).matched("UA"), Some(jordan));
}

/// Taking a Slack person under my name leaves the team's match alone: the
/// member they were matched to keeps them, so a teammate's import still
/// brings their messages in as that member.
#[test]
fn slack_under_my_name_keeps_the_teams_match() {
    use crate::app::slack::{plan, Assign};
    let mut a = app();
    let (sam, jordan) = (3, 4);
    let mut msgs = vec![(1, "UA")];
    let (u, c) = slack_moves(&msgs);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UA".into(), Assign::Member(sam));
    a.import_slack(&p);
    assert_eq!(slack_msg(&a, 1).0, sam);
    assert_eq!(slack_ids(&a.data, sam), ["UA"]);

    // I take their new message under my name
    msgs.push((2, "UA"));
    let (u, c) = slack_moves(&msgs);
    let mut p = plan(&a.data, u, c);
    assert_eq!(p.matched("UA"), Some(sam));
    p.assign.insert("UA".into(), Assign::Me);
    a.import_slack(&p);
    assert_eq!(slack_msg(&a, 2), (a.data.me, "Al: msg 2".into()));
    assert_eq!(slack_ids(&a.data, sam), ["UA"], "Sam keeps them");
    assert_eq!(a.data.extra.slack_under_me, ["UA"]);

    // Jordan's import of the next message: as Sam, as the team decided
    msgs.push((3, "UA"));
    let mut b = App::build(None, Some(teammate_copy(&a.data, jordan)));
    let (u, c) = slack_moves(&msgs);
    let p = plan(&b.data, u, c);
    assert_eq!(p.matched("UA"), Some(sam));
    b.import_slack(&p);
    assert_eq!(slack_msg(&b, 3).0, sam);
    assert_eq!(slack_ids(&b.data, sam), ["UA"]);
    // and mine still starts from my name
    let (u, c) = slack_moves(&msgs);
    assert_eq!(plan(&a.data, u, c).assign.get("UA"), Some(&Assign::Me));
}

/// An author an export's users.json leaves out (a deleted account, a partial
/// export) still goes to the member an earlier import matched them to, and
/// bringing them in some other way with nobody made for them doesn't take
/// that match away.
#[test]
fn a_slack_author_missing_from_users_keeps_their_match() {
    use crate::app::slack::{plan, Assign};
    let mut a = app();
    let sam = 3;
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == sam)
        .unwrap()
        .slack = vec!["UX".into()];
    let mut msgs = vec![(1, "UX")];
    let (u, c) = slack_moves(&msgs);
    assert!(!u.iter().any(|u| u.id == "UX"));
    let p = plan(&a.data, u, c);
    assert_eq!(p.matched("UX"), Some(sam));
    a.import_slack(&p);
    assert_eq!(slack_msg(&a, 1).0, sam);
    assert_eq!(a.data.member(sam).unwrap().slack, ["UX"]);

    // as a new member: nothing to make one from, so their message comes in
    // under my name and Sam keeps the match
    msgs.push((2, "UX"));
    let (u, c) = slack_moves(&msgs);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UX".into(), Assign::New);
    let members = a.data.members.len();
    a.import_slack(&p);
    assert_eq!(a.data.members.len(), members);
    assert_eq!(slack_msg(&a, 2).0, a.data.me);
    assert_eq!(a.data.member(sam).unwrap().slack, ["UX"]);
    assert!(a.data.extra.slack_under_me.is_empty());

    // without the earlier match: a new member by default, none made, and
    // no match for them appears anywhere
    let mut c2 = app();
    let (u, c) = slack_moves(&[(1, "UX")]);
    let p = plan(&c2.data, u, c);
    assert_eq!(p.assign.get("UX"), Some(&Assign::New));
    c2.import_slack(&p);
    assert!(c2
        .data
        .members
        .iter()
        .all(|m| !m.slack.contains(&"UX".to_string())));
}

/// Pointing a Slack person from one member at another moves the team's
/// match for everyone: a teammate's import follows it, and theirs moves it
/// for me in turn.
#[test]
fn a_reassigned_slack_person_sticks_for_everyone() {
    use crate::app::slack::{plan, Assign};
    let mut a = app();
    let (alex, sam, jordan) = (2, 3, 4);
    let mut msgs = vec![(1, "UA")];
    let (u, c) = slack_moves(&msgs);
    a.import_slack(&plan(&a.data, u, c));
    assert_eq!(slack_ids(&a.data, alex), ["UA"]);
    msgs.push((2, "UA"));
    let (u, c) = slack_moves(&msgs);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UA".into(), Assign::Member(sam));
    a.import_slack(&p);
    assert!(slack_ids(&a.data, alex).is_empty());
    assert_eq!(slack_ids(&a.data, sam), ["UA"]);

    // Jordan's computer: Sam, not Alex by email
    msgs.push((3, "UA"));
    let mut b = App::build(None, Some(teammate_copy(&a.data, jordan)));
    let (u, c) = slack_moves(&msgs);
    let p = plan(&b.data, u, c);
    assert_eq!(p.matched("UA"), Some(sam));
    b.import_slack(&p);
    assert_eq!(slack_msg(&b, 3).0, sam);

    // Jordan points them at himself; that comes back to me
    msgs.push((4, "UA"));
    let (u, c) = slack_moves(&msgs);
    let mut p = plan(&b.data, u, c);
    p.assign.insert("UA".into(), Assign::Member(jordan));
    b.import_slack(&p);
    a.data.extra.slack_map = b.data.extra.slack_map.clone();
    let (u, c) = slack_moves(&msgs);
    assert_eq!(plan(&a.data, u, c).matched("UA"), Some(jordan));
    assert_eq!(slack_ids(&a.data, jordan), ["UA"]);
    assert!(slack_ids(&a.data, alex).is_empty() && slack_ids(&a.data, sam).is_empty());
}

/// "Under my name" kept on my shared member by an earlier version is copied
/// to my own data (left out of the copy teammates get) when the file is
/// read, and stays on the member for my other computers; a teammate's list
/// stays for their computer to take over.
#[test]
fn slack_under_my_name_moves_off_the_shared_member() {
    use crate::app::slack::{plan, Assign};
    let mut d = Data::seed();
    let me = d.me;
    let mut v = serde_json::to_value(&d).unwrap();
    for m in v["members"].as_array_mut().unwrap() {
        match m["id"].as_u64() {
            Some(1) => m["slack_mine"] = serde_json::json!(["UA"]),
            Some(3) => m["slack_mine"] = serde_json::json!(["US"]),
            _ => {}
        }
    }
    d = serde_json::from_value(v).unwrap();
    assert_eq!(me, 1);
    // read even before it moves
    assert_eq!(d.slack_under_me(), ["UA"]);
    assert!(d.keep_slack_under_me_personal());
    assert!(!d.keep_slack_under_me_personal(), "only once");
    assert_eq!(d.extra.slack_under_me, ["UA"]);
    assert_eq!(d.slack_under_me(), ["UA"]);
    assert_eq!(
        d.member(me).unwrap().slack_mine,
        ["UA"],
        "left as written, for my other computers"
    );
    assert_eq!(d.member(3).unwrap().slack_mine, ["US"]);
    let saved = serde_json::to_value(&d).unwrap();
    assert_eq!(saved["extra"]["slack_under_me"], serde_json::json!(["UA"]));
    let shared = crate::sync::shareable(&saved);
    assert!(shared["extra"].get("slack_under_me").is_none());
    // nothing left over on a file that never had any
    let mut e = Data::seed().extra;
    e.slack_under_me.clear();
    assert!(!serde_json::to_string(&e)
        .unwrap()
        .contains("slack_under_me"));

    // still on my member when an import starts (a copy from an earlier
    // version came in): the import moves it, then my choice changes
    let mut a = app();
    a.data.members[0].slack_mine = vec!["UA".into()];
    let (u, c) = slack_moves(&[(1, "UA")]);
    let mut p = plan(&a.data, u, c);
    assert_eq!(p.assign.get("UA"), Some(&Assign::Me));
    p.assign.insert("UA".into(), Assign::Member(2));
    a.import_slack(&p);
    assert!(a.data.extra.slack_under_me.is_empty());
    assert_eq!(a.data.members[0].slack_mine, ["UA"], "nothing taken off");
    assert_eq!(a.data.extra.slack_not_under_me, ["UA"]);
    assert!(!a.data.slack_under_me().contains(&"UA".to_string()));
    assert!(!a.data.keep_slack_under_me_personal(), "not copied back");
    assert_eq!(slack_ids(&a.data, 2), ["UA"]);
    let (u, c) = slack_moves(&[(1, "UA")]);
    assert_eq!(plan(&a.data, u, c).matched("UA"), Some(2), "it sticks");
    // and under my name again
    let (u, c) = slack_moves(&[(1, "UA")]);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UA".into(), Assign::Me);
    a.import_slack(&p);
    assert!(a.data.extra.slack_not_under_me.is_empty());
    assert_eq!(a.data.slack_under_me(), ["UA"]);
    let shared = crate::sync::shareable(&serde_json::to_value(&a.data).unwrap());
    assert!(shared["extra"].get("slack_not_under_me").is_none());
}

/// The Slack ids the shared `Extra::slack_map` brings in as member `m`.
fn slack_ids(d: &Data, m: u64) -> Vec<String> {
    d.extra
        .slack_map
        .iter()
        .filter(|(_, x)| **x == m)
        .map(|(k, _)| k.clone())
        .collect()
}

/// A teammate's computer joining the workspace `folder` shares, as the
/// member `me`.
fn slack_joiner(folder: &str, me: u64) -> App {
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.join_shared(folder).unwrap();
    b.become_member(Some(me), "", "");
    b.sync_note_change();
    b.write_shared();
    b
}

/// A change made here reaches the shared folder.
fn slack_push(a: &mut App) {
    a.sync_note_change();
    a.write_shared();
}

/// Where the next import on `a` brings the Slack person `sid` in.
fn slack_next(a: &App, sid: &str) -> Option<crate::app::slack::Assign> {
    let (u, c) = slack_moves(&[(1, sid)]);
    crate::app::slack::plan(&a.data, u, c)
        .assign
        .get(sid)
        .copied()
}

/// Two people pointing one Slack person at different members at about the
/// same time: once the computers sync (a real merge of their copies), both
/// have the later choice, and the next import on either keeps it. A later
/// choice made after seeing that one moves it again for everyone.
#[test]
fn concurrent_slack_reassignments_end_with_the_later_one_everywhere() {
    use crate::app::slack::{plan, Assign};
    let dir = temp_home("slack-sync");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let (alex, sam, jordan) = (2, 3, 4);
    let mut b = slack_joiner(&folder, jordan);
    assert!(a.sync_pull());

    // matched to Alex by email, A points UA at Jordan; B, a moment later,
    // at Sam
    let (u, c) = slack_moves(&[(1, "UA")]);
    let mut p = plan(&a.data, u, c);
    assert_eq!(p.matched("UA"), Some(alex));
    p.assign.insert("UA".into(), Assign::Member(jordan));
    a.import_slack(&p);
    slack_push(&mut a);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let (u, c) = slack_moves(&[(1, "UA")]);
    let mut p = plan(&b.data, u, c);
    p.assign.insert("UA".into(), Assign::Member(sam));
    b.import_slack(&p);
    slack_push(&mut b);
    a.sync_pull();
    b.sync_pull();
    for (who, x) in [("A", &a), ("B", &b)] {
        assert_eq!(x.data.extra.slack_map.get("UA"), Some(&sam), "{}", who);
        assert_eq!(slack_next(x, "UA"), Some(Assign::Member(sam)), "{}", who);
    }
    // the next import on A keeps Sam, here and on B
    let (u, c) = slack_moves(&[(1, "UA"), (2, "UA")]);
    a.import_slack(&plan(&a.data, u, c));
    slack_push(&mut a);
    b.sync_pull();
    for x in [&a, &b] {
        assert_eq!(x.data.extra.slack_map.get("UA"), Some(&sam));
    }

    // the other way round: B first, A a moment later
    let (u, c) = slack_moves(&[(1, "UB")]);
    let mut p = plan(&b.data, u, c);
    p.assign.insert("UB".into(), Assign::Member(sam));
    b.import_slack(&p);
    slack_push(&mut b);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let (u, c) = slack_moves(&[(1, "UB")]);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UB".into(), Assign::Member(alex));
    a.import_slack(&p);
    slack_push(&mut a);
    b.sync_pull();
    a.sync_pull();
    for x in [&a, &b] {
        assert_eq!(x.data.extra.slack_map.get("UB"), Some(&alex));
        assert_eq!(x.data.extra.slack_map.get("UA"), Some(&sam));
    }

    // one after the other: B, having seen Sam, moves UA back to Alex
    let (u, c) = slack_moves(&[(1, "UA")]);
    let mut p = plan(&b.data, u, c);
    assert_eq!(p.matched("UA"), Some(sam));
    p.assign.insert("UA".into(), Assign::Member(alex));
    b.import_slack(&p);
    slack_push(&mut b);
    a.sync_pull();
    assert_eq!(slack_next(&a, "UA"), Some(Assign::Member(alex)));
    let (u, c) = slack_moves(&[(1, "UA"), (3, "UA")]);
    a.import_slack(&plan(&a.data, u, c));
    slack_push(&mut a);
    b.sync_pull();
    for x in [&a, &b] {
        assert_eq!(x.data.extra.slack_map.get("UA"), Some(&alex));
        assert!(
            x.data.members.iter().all(|m| m.slack.is_empty()),
            "nothing is written on the members"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two people pointing two different Slack people at one member at the
/// same time both keep theirs (one list per member used to keep only one).
/// B's import also brings UA (a member of the channel) along as matched by
/// email, a moment after A pointed UA at Jordan: only A decided, so A's
/// choice stays.
#[test]
fn different_slack_people_sent_to_one_member_at_once_are_both_kept() {
    use crate::app::slack::{plan, Assign};
    let dir = temp_home("slack-sync2");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let jordan = 4;
    let mut b = slack_joiner(&folder, 3);
    a.sync_pull();
    let (u, c) = slack_moves(&[(1, "UA")]);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UA".into(), Assign::Member(jordan));
    a.import_slack(&p);
    let (u, c) = slack_moves(&[(2, "UB")]);
    let mut p = plan(&b.data, u, c);
    p.assign.insert("UB".into(), Assign::Member(jordan));
    b.import_slack(&p);
    slack_push(&mut a);
    slack_push(&mut b);
    a.sync_pull();
    b.sync_pull();
    for (who, x) in [("A", &a), ("B", &b)] {
        assert_eq!(slack_ids(&x.data, jordan), ["UA", "UB"], "{}", who);
        assert_eq!(slack_next(x, "UA"), Some(Assign::Member(jordan)));
        assert_eq!(slack_next(x, "UB"), Some(Assign::Member(jordan)));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Under my name" an earlier version kept on my shared member: my first
/// computer to update takes it over and leaves the member as it was, so my
/// second computer, updating later, still finds it there.
#[test]
fn my_second_computer_updating_later_still_gets_under_my_name() {
    use crate::app::slack::{plan, Assign};
    let dir = temp_home("slack-sync3");
    let folder = dir.to_str().unwrap().to_string();
    // written by the earlier version, on both of my computers
    let mut a = app();
    let me = a.data.me;
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == me)
        .unwrap()
        .slack_mine = vec!["UA".into()];
    a.share_here(&folder).unwrap();
    let mut b = slack_joiner(&folder, me);
    assert_eq!(b.data.member(me).unwrap().slack_mine, ["UA"]);

    // my first computer updates and imports; its copy reaches the folder
    assert!(a.data.keep_slack_under_me_personal());
    let (u, c) = slack_moves(&[(1, "UA")]);
    let p = plan(&a.data, u, c);
    assert_eq!(p.assign.get("UA"), Some(&Assign::Me));
    a.import_slack(&p);
    slack_push(&mut a);
    b.sync_pull();
    // the second one updates later
    assert_eq!(b.data.member(me).unwrap().slack_mine, ["UA"], "still there");
    assert_eq!(slack_next(&b, "UA"), Some(Assign::Me));
    assert!(b.data.keep_slack_under_me_personal());
    assert_eq!(b.data.extra.slack_under_me, ["UA"]);
    assert_eq!(slack_next(&b, "UA"), Some(Assign::Me));
    // a teammate's computer isn't affected by it
    let c = slack_joiner(&folder, 3);
    assert!(c.data.extra.slack_under_me.is_empty());
    assert_eq!(slack_next(&c, "UA"), Some(Assign::Member(2)), "by email");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Slack matches an earlier version kept on the members go into the shared
/// map when the workspace opens in this version, without that counting as
/// an edit: a teammate's real choice wins over them, even one made before.
#[test]
fn slack_matches_from_an_earlier_version_move_into_the_map_without_an_edit() {
    use crate::app::slack::Assign;
    let dir = temp_home("slack-sync4");
    let folder = dir.to_str().unwrap().to_string();
    let (alex, sam, jordan) = (2, 3, 4);
    let mut a = app();
    // the earlier version left UA on two members (the old race) and UB on Jordan
    for (m, ids) in [(alex, vec!["UA"]), (sam, vec!["UA"]), (jordan, vec!["UB"])] {
        a.data.members.iter_mut().find(|x| x.id == m).unwrap().slack =
            ids.into_iter().map(String::from).collect();
    }
    a.share_here(&folder).unwrap();
    let mut b = slack_joiner(&folder, jordan);
    // B's teammate decides UA is Jordan before A's computer updates
    b.data.extra.slack_map.insert("UA".into(), jordan);
    slack_push(&mut b);
    std::thread::sleep(std::time::Duration::from_millis(5));

    // A's computer opens the workspace in this version
    let mut a = App::build(None, Some(a.data.clone()));
    assert_eq!(
        a.data.extra.slack_map,
        [("UB".to_string(), jordan)].into(),
        "UA, on two members, stays with the older lists until someone decides"
    );
    assert_eq!(
        a.data.slack_member("UA"),
        Some(alex),
        "the first member still wins"
    );
    let before = a.data.extra.sync.clone();
    slack_push(&mut a);
    let after = &a.data.extra.sync;
    assert_eq!(after.stamps, before.stamps, "no member counts as edited");
    assert_eq!(after.fields, before.fields);
    assert!(
        !after
            .values
            .keys()
            .any(|k| k.starts_with("extra.slack_map/")),
        "{:?}",
        after.values
    );
    assert!(
        !a.data.fold_legacy_slack(),
        "nothing more to move: {:?}",
        a.data.extra.slack_map
    );
    a.sync_pull();
    b.sync_pull();
    for x in [&a, &b] {
        assert_eq!(x.data.extra.slack_map.get("UA"), Some(&jordan));
        assert_eq!(x.data.extra.slack_map.get("UB"), Some(&jordan));
        assert_eq!(slack_next(x, "UA"), Some(Assign::Member(jordan)));
        assert_eq!(
            x.data.member(alex).unwrap().slack,
            ["UA"],
            "kept for older versions"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Slack matches set at about the same time on two computers connected to
/// a team server: the server merges them entry by entry like a shared
/// folder does, so both end with the later choice for one Slack person and
/// both people's entries for two.
#[test]
fn concurrent_slack_matches_go_through_a_server() {
    use crate::app::slack::{plan, Assign};
    let server = TestServer::start();
    let addr = format!("127.0.0.1:{}", server.port);
    let (alex, sam, jordan) = (2, 3, 4);
    let mut a = app();
    a.connect_server(&addr, "").unwrap();
    net_until(&mut a, "A shared on the server", |a| {
        a.data.extra.shared.is_some()
    });
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !server.home.join(workboard_server::WORKSPACE_FILE).is_file()
        && std::time::Instant::now() < until
    {
        a.net_tick(None);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    net_until(&mut a, "A sees the people list", |a| {
        a.sync_rt.people.iter().any(|p| p.member == sam)
    });
    a.access_request(wb_net::Msg::CreateInvite {
        member: sam,
        // Import from Slack is for owners and admins
        role: wb_net::Role::Admin,
    });
    net_until(&mut a, "invite code", |a| {
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
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.connect_server(&addr, &code).unwrap();
    net_until(&mut b, "B joined", |b| {
        b.data.extra.shared.is_some() && b.data.member(alex).is_some()
    });

    // A points UA at Jordan and UB at Jordan; B, a moment later, UA at Sam
    // and UC at Jordan
    let (u, c) = slack_moves(&[(1, "UA"), (2, "UB")]);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UA".into(), Assign::Member(jordan));
    p.assign.insert("UB".into(), Assign::Member(jordan));
    a.import_slack(&p);
    a.sync_note_change();
    a.write_shared();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let (u, c) = slack_moves(&[(1, "UA"), (3, "UC")]);
    let mut p = plan(&b.data, u, c);
    p.assign.insert("UA".into(), Assign::Member(sam));
    p.assign.insert("UC".into(), Assign::Member(jordan));
    b.import_slack(&p);
    b.sync_note_change();
    b.write_shared();
    let want: std::collections::BTreeMap<String, u64> = [
        ("UA".to_string(), sam),
        ("UB".to_string(), jordan),
        ("UC".to_string(), jordan),
    ]
    .into();
    net_until(&mut a, "A has both people's matches", |a| {
        a.data.extra.slack_map == want
    });
    net_until(&mut b, "B has both people's matches", |b| {
        b.data.extra.slack_map == want
    });
    assert_eq!(slack_next(&a, "UA"), Some(Assign::Member(sam)));
}

/// A Slack person an earlier version kept on Alex, moved to Sam in the
/// dialog: once Sam is gone the older list doesn't bring Alex back; the
/// next import matches them like anyone else (no email or name here, so a
/// new member).
#[test]
fn an_overturned_slack_match_from_an_earlier_version_stays_overturned() {
    use crate::app::slack::{plan, Assign};
    let mut a = app();
    let (alex, sam) = (2, 3);
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == alex)
        .unwrap()
        .slack = vec!["UX".into()];
    assert_eq!(slack_next(&a, "UX"), Some(Assign::Member(alex)));
    let (u, c) = slack_moves(&[(1, "UX")]);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UX".into(), Assign::Member(sam));
    a.import_slack(&p);
    assert_eq!(a.data.extra.slack_map.get("UX"), Some(&sam));
    assert_eq!(
        a.data.member(alex).unwrap().slack,
        ["UX"],
        "kept for older versions"
    );
    a.data.members.retain(|m| m.id != sam);
    assert_eq!(a.data.slack_member("UX"), None);
    assert_eq!(slack_next(&a, "UX"), Some(Assign::New));
    // an entry taken away (its stamp stays) doesn't bring Alex back either
    a.data.extra.slack_map.remove("UX");
    a.data
        .extra
        .sync
        .values
        .insert(crate::sync::map_part(crate::model::SLACK_MAP, "UX"), 5);
    assert_eq!(slack_next(&a, "UX"), Some(Assign::New));
    // a Slack person nothing was ever decided for still finds the older list
    a.data.extra.sync.values.clear();
    assert_eq!(slack_next(&a, "UX"), Some(Assign::Member(alex)));
}

/// A shared workspace whose bookkeeping has no fingerprint for a Slack
/// match it holds (saved by a version that didn't fingerprint the map):
/// opening it records the match as it is, so the first change made
/// afterwards isn't a new decision for it, and a teammate's later one
/// still wins.
#[test]
fn an_unfingerprinted_slack_match_is_not_an_edit_after_opening() {
    let dir = temp_home("slack-baseline");
    let folder = dir.to_str().unwrap().to_string();
    let alex = 2;
    let mut a = app();
    a.share_here(&folder).unwrap();
    a.data.extra.slack_map.insert("UA".into(), alex);
    let sync = &mut a.data.extra.sync;
    sync.value_hashes
        .retain(|p, _| crate::sync::SHARED_VALUES.contains(&p.as_str()));
    sync.values.insert("extra.slack_map/UA".into(), 100);
    let mut a = App::build(None, Some(a.data.clone()));
    a.data.workspace = "Renamed".into();
    slack_push(&mut a);
    let values = &a.data.extra.sync.values;
    assert_eq!(values.get("extra.slack_map/UA"), Some(&100), "{:?}", values);
    assert!(values.get("workspace").is_some_and(|s| *s > 100));
    let _ = std::fs::remove_dir_all(&dir);
}

/// An older version could leave one Slack id on two members. Opening the
/// workspace leaves that id out of the shared map (nobody decided it), so
/// when the first member is deleted the other one the older lists name
/// still counts, instead of a new member being made on the next import.
#[test]
fn a_slack_id_on_two_older_lists_falls_back_to_the_other_one() {
    use crate::app::slack::Assign;
    let mut a = app();
    let (alex, sam) = (2, 3);
    for id in [alex, sam] {
        a.data
            .members
            .iter_mut()
            .find(|m| m.id == id)
            .unwrap()
            .slack = vec!["UX".into()];
    }
    a.data.fold_legacy_slack();
    assert_eq!(a.data.extra.slack_map.get("UX"), None, "not carried");
    assert_eq!(slack_next(&a, "UX"), Some(Assign::Member(alex)));
    a.data.members.retain(|m| m.id != alex);
    assert_eq!(a.data.slack_member("UX"), Some(sam));
    assert_eq!(slack_next(&a, "UX"), Some(Assign::Member(sam)));
}

/// A Slack match decided before the workspace was shared stays a decision
/// after sharing (sharing starts the sync bookkeeping afresh): an overturned
/// match from an older version doesn't come back when its member is deleted.
#[test]
fn a_slack_decision_made_before_sharing_stays_one_after() {
    use crate::app::slack::{plan, Assign};
    let dir = temp_home("slack-share");
    let mut a = app();
    let (alex, sam) = (2, 3);
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == alex)
        .unwrap()
        .slack = vec!["UX".into()];
    let (u, c) = slack_moves(&[(1, "UX")]);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UX".into(), Assign::Member(sam));
    a.import_slack(&p);
    assert_eq!(a.data.extra.slack_map.get("UX"), Some(&sam));
    a.share_here(dir.to_str().unwrap()).unwrap();
    a.data.members.retain(|m| m.id != sam);
    assert_eq!(a.data.slack_member("UX"), None);
    assert_eq!(slack_next(&a, "UX"), Some(Assign::New));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A plain import that only accepts what matching found for a Slack id an
/// older version left on two members doesn't write it into the shared map
/// either, so deleting that member later still falls back to the other one.
#[test]
fn importing_a_slack_id_on_two_older_lists_keeps_it_out_of_the_map() {
    use crate::app::slack::{plan, Assign};
    let mut a = app();
    let (alex, sam) = (2, 3);
    for id in [alex, sam] {
        a.data
            .members
            .iter_mut()
            .find(|m| m.id == id)
            .unwrap()
            .slack = vec!["UX".into()];
    }
    let (u, c) = slack_moves(&[(1, "UX")]);
    let p = plan(&a.data, u, c);
    assert_eq!(p.assign.get("UX"), Some(&Assign::Member(alex)));
    a.import_slack(&p);
    assert_eq!(a.data.extra.slack_map.get("UX"), None);
    a.data.members.retain(|m| m.id != alex);
    assert_eq!(slack_next(&a, "UX"), Some(Assign::Member(sam)));
    // choosing a member in the dialog is a decision and goes in the map
    let (u, c) = slack_moves(&[(2, "UX")]);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UX".into(), Assign::Member(4));
    a.import_slack(&p);
    assert_eq!(a.data.extra.slack_map.get("UX"), Some(&4));
}

/// A Slack person whose shared match points at a deleted member: a plain
/// import that only accepts what matching finds for them isn't a decision,
/// so a teammate's real decision made at about the same time wins on both
/// computers after a real merge.
#[test]
fn a_plain_import_over_a_deleted_members_match_does_not_beat_a_teammates_choice() {
    use crate::app::slack::{plan, Assign};
    let dir = temp_home("slack-gone");
    let folder = dir.to_str().unwrap().to_string();
    let (alex, sam, jordan) = (2, 3, 4);
    let mut a = app();
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == sam)
        .unwrap()
        .slack = vec!["UA".into()];
    a.data.fold_legacy_slack();
    assert_eq!(a.data.extra.slack_map.get("UA"), Some(&sam));
    a.share_here(&folder).unwrap();
    let mut b = slack_joiner(&folder, jordan);
    a.data.members.retain(|m| m.id != sam);
    slack_push(&mut a);
    assert!(b.sync_pull());
    assert_eq!(slack_next(&b, "UA"), Some(Assign::Member(alex)), "by email");

    // A decides Jordan; B, a moment later and before syncing, imports and
    // keeps what matching found
    let (u, c) = slack_moves(&[(1, "UA")]);
    let mut p = plan(&a.data, u, c);
    p.assign.insert("UA".into(), Assign::Member(jordan));
    a.import_slack(&p);
    slack_push(&mut a);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let (u, c) = slack_moves(&[(1, "UA")]);
    b.import_slack(&plan(&b.data, u, c));
    slack_push(&mut b);
    a.sync_pull();
    b.sync_pull();
    for (who, x) in [("A", &a), ("B", &b)] {
        assert_eq!(x.data.extra.slack_map.get("UA"), Some(&jordan), "{}", who);
        assert_eq!(slack_next(x, "UA"), Some(Assign::Member(jordan)), "{}", who);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A match carried over from one older list, and a second older list for
/// the same Slack person arriving later from a teammate's older app: when
/// the first member is deleted, the second still counts.
#[test]
fn a_carried_slack_match_falls_back_to_an_older_list_that_came_later() {
    use crate::app::slack::Assign;
    let dir = temp_home("slack-late");
    let folder = dir.to_str().unwrap().to_string();
    let (alex, sam, jordan) = (2, 3, 4);
    let mut a = app();
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == alex)
        .unwrap()
        .slack = vec!["UX".into()];
    a.data.fold_legacy_slack();
    assert_eq!(a.data.extra.slack_carried, ["UX"]);
    a.share_here(&folder).unwrap();
    let mut b = slack_joiner(&folder, jordan);
    // an older app on B's side had put UX on Sam
    b.data
        .members
        .iter_mut()
        .find(|m| m.id == sam)
        .unwrap()
        .slack = vec!["UX".into()];
    slack_push(&mut b);
    assert!(a.sync_pull());
    a.data.members.retain(|m| m.id != alex);
    assert_eq!(a.data.slack_member("UX"), Some(sam));
    assert_eq!(slack_next(&a, "UX"), Some(Assign::Member(sam)));
    // my note is mine: it isn't in the copy teammates get
    let shared = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
        .collect::<String>();
    assert!(!shared.contains("slack_carried"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A Slack match I carried over, then a teammate's real decision for the
/// same person arriving by a real merge: that decision isn't "carried"
/// here any more, so when its member is deleted my computer doesn't fall
/// back to the older list against it (both computers agree).
#[test]
fn a_teammates_decision_merged_in_stops_my_carried_fallback() {
    use crate::app::slack::{plan, Assign};
    let dir = temp_home("slack-stale");
    let folder = dir.to_str().unwrap().to_string();
    let (alex, sam, jordan) = (2, 3, 4);
    let mut a = app();
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == alex)
        .unwrap()
        .slack = vec!["UX".into()];
    a.data.fold_legacy_slack();
    a.share_here(&folder).unwrap();
    let mut b = slack_joiner(&folder, jordan);
    let (u, c) = slack_moves(&[(1, "UX")]);
    let mut p = plan(&b.data, u, c);
    p.assign.insert("UX".into(), Assign::Member(sam));
    b.import_slack(&p);
    slack_push(&mut b);
    assert!(a.sync_pull());
    assert_eq!(a.data.extra.slack_map.get("UX"), Some(&sam));
    assert!(!a.data.extra.slack_carried.contains(&"UX".to_string()));
    b.data.members.retain(|m| m.id != sam);
    slack_push(&mut b);
    assert!(a.sync_pull());
    for (who, x) in [("A", &a), ("B", &b)] {
        assert_eq!(x.data.slack_member("UX"), None, "{}", who);
        assert_eq!(slack_next(x, "UX"), Some(Assign::New), "{}", who);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A carried match coming to a teammate's computer by merge (no stamp)
/// counts as carried there too: with its member deleted, both computers
/// fall back to the other older list.
#[test]
fn a_carried_slack_match_from_a_teammate_falls_back_on_my_computer_too() {
    use crate::app::slack::Assign;
    let dir = temp_home("slack-carried-b");
    let folder = dir.to_str().unwrap().to_string();
    let (alex, sam, jordan) = (2, 3, 4);
    let mut a = app();
    a.data
        .members
        .iter_mut()
        .find(|m| m.id == alex)
        .unwrap()
        .slack = vec!["UX".into()];
    a.data.fold_legacy_slack();
    a.share_here(&folder).unwrap();
    let mut b = slack_joiner(&folder, jordan);
    assert_eq!(
        b.data.extra.slack_carried,
        ["UX"],
        "joined: the team's carried match"
    );
    b.data
        .members
        .iter_mut()
        .find(|m| m.id == sam)
        .unwrap()
        .slack = vec!["UX".into()];
    b.data.members.retain(|m| m.id != alex);
    slack_push(&mut b);
    assert!(a.sync_pull());
    for (who, x) in [("A", &a), ("B", &b)] {
        assert_eq!(x.data.slack_member("UX"), Some(sam), "{}", who);
        assert_eq!(slack_next(x, "UX"), Some(Assign::Member(sam)), "{}", who);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Joining a team doesn't bring along Slack matches this computer carried
/// in the workspace it left.
#[test]
fn joining_a_team_forgets_my_old_carried_slack_matches() {
    let dir = temp_home("slack-join-carried");
    let folder = dir.to_str().unwrap().to_string();
    let mut a = app();
    a.share_here(&folder).unwrap();
    let mut b = App::build(None, Some(blank_as("Theirs", 1, "B")));
    b.data.extra.slack_carried = vec!["UOLD".into()];
    b.join_shared(&folder).unwrap();
    assert!(!b.data.extra.slack_carried.contains(&"UOLD".to_string()));
    let _ = std::fs::remove_dir_all(&dir);
}
