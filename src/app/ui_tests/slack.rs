// Click-through tests for importing a Slack export.

use super::*;

#[test]
fn import_from_slack_in_an_empty_channel_brings_the_export_in() {
    let mut h = Ui1::new();
    h.app
        .apply_chat(vec![crate::app::chat::ChatAction::NewChannel(
            "fresh".into(),
        )]);
    h.app.section = Section::Chat;
    h.settle();
    assert!(!h.has("\u{2699}  Integrate"), "no button that does nothing");
    // a Slack export zip, "chosen" in the file picker
    let root = temp_home("ui-slack");
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(
        ex.join("users.json"),
        r#"[{"id":"U1","name":"alex","real_name":"Alex Kim","profile":{}}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":["U1"]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","user":"U1","text":"first sketch","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    crate::app::slack::zip_folder(&ex, &zip, false);
    crate::app::slack::PICKED.with(|p| *p.borrow_mut() = Some(zip));
    h.click("Import from Slack");
    h.app.finish_slack_load();
    h.settle();
    assert!(matches!(h.app.modal, Some(Modal::SlackImport(_))));
    h.click("Import");
    let design = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "design")
        .expect("the channel came in");
    assert_eq!(design.messages[0].text, "first sketch");
    assert_eq!(design.messages[0].ts, 1_704_189_600);
    h.click("OK");
    assert!(h.app.modal.is_none());
}

#[test]
fn the_slack_import_dialog_shows_who_goes_where_and_lets_me_change_it() {
    let mut h = Ui1::new();
    h.app.section = Section::Chat;
    h.settle();
    let root = temp_home("ui-slack2");
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(
        ex.join("users.json"),
        r#"[{"id":"U1","name":"alex","real_name":"Alex Kim","profile":{}}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":["U1"]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","user":"U1","text":"first sketch","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    crate::app::slack::zip_folder(&ex, &zip, false);
    h.app.open_slack_export(&zip);
    h.app.finish_slack_load();
    h.settle();
    // the Slack Alex Kim is proposed as the member Alex Kim
    assert!(h.has("Whose messages go where:"));
    assert!(h.has("Alex Kim (@alex)"));
    h.click("Messages from Alex Kim");
    h.click("Import under my name");
    match &h.app.modal {
        Some(Modal::SlackImport(p)) => {
            assert_eq!(p.assign.get("U1"), Some(&crate::app::slack::Assign::Me))
        }
        _ => panic!("the dialog stays open"),
    }
    h.click("Import");
    let me = h.app.data.me;
    let design = h
        .app
        .data
        .channels
        .iter()
        .find(|c| c.name == "design")
        .unwrap();
    assert_eq!(design.messages[0].author, me);
    assert_eq!(design.messages[0].text, "Alex Kim: first sketch");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_slack_matching_picker_tells_members_of_one_name_apart() {
    let mut h = Ui1::new();
    let sam2 = h.app.data.new_id();
    h.app.data.members.push(Member {
        id: sam2,
        name: "Sam Lee".into(),
        color: [1, 2, 3],
        role: "Guest".into(),
        ..Default::default()
    });
    h.app.section = Section::Chat;
    h.settle();
    let root = temp_home("ui-slack3");
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(
        ex.join("users.json"),
        r#"[{"id":"U1","name":"sam","real_name":"Sam Lee","profile":{}}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":["U1"]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","user":"U1","text":"hello","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    crate::app::slack::zip_folder(&ex, &zip, false);
    h.app.open_slack_export(&zip);
    h.app.finish_slack_load();
    h.settle();
    h.click("Messages from Sam Lee");
    assert!(
        h.has("Sam Lee (sam@example.com)"),
        "the email next to the name"
    );
    assert!(h.has("Sam Lee (Guest)"), "no email: the role");
    h.click("Sam Lee (Guest)");
    match &h.app.modal {
        Some(Modal::SlackImport(p)) => {
            assert_eq!(
                p.assign.get("U1"),
                Some(&crate::app::slack::Assign::Member(sam2))
            )
        }
        _ => panic!("the dialog stays open"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// Choosing where a Slack person goes in the import dialog sticks: opened
/// again, the dialog starts from that choice (my name, then another member),
/// not from the match it proposed the first time.
#[test]
fn the_slack_import_dialog_remembers_the_last_choice_for_each_person() {
    let mut h = Ui1::new();
    h.app.section = Section::Chat;
    h.settle();
    let root = temp_home("ui-slack3");
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(
        ex.join("users.json"),
        r#"[{"id":"U1","name":"alex","real_name":"Alex Kim","profile":{}}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":["U1"]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","user":"U1","text":"first sketch","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    crate::app::slack::zip_folder(&ex, &zip, false);
    let chosen = |h: &Ui1| match &h.app.modal {
        Some(Modal::SlackImport(p)) => p.assign.get("U1").copied(),
        _ => panic!("the dialog is open"),
    };
    // proposed as the member Alex Kim (by name); I take them under my name
    h.app.open_slack_export(&zip);
    h.app.finish_slack_load();
    h.settle();
    let alex = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Alex Kim")
        .unwrap()
        .id;
    assert_eq!(chosen(&h), Some(crate::app::slack::Assign::Member(alex)));
    h.click("Messages from Alex Kim");
    h.click("Import under my name");
    h.click("Import");
    // opened again: my name, as chosen
    h.app.open_slack_export(&zip);
    h.app.finish_slack_load();
    h.settle();
    assert_eq!(chosen(&h), Some(crate::app::slack::Assign::Me));
    // now Sam Lee instead; the next time it's Sam Lee
    h.click("Messages from Alex Kim");
    h.click("Sam Lee (sam@example.com)");
    let sam = h
        .app
        .data
        .members
        .iter()
        .find(|m| m.name == "Sam Lee")
        .unwrap()
        .id;
    assert_eq!(chosen(&h), Some(crate::app::slack::Assign::Member(sam)));
    h.click("Import");
    h.app.open_slack_export(&zip);
    h.app.finish_slack_load();
    h.settle();
    assert_eq!(chosen(&h), Some(crate::app::slack::Assign::Member(sam)));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_slack_export_is_read_while_the_window_keeps_drawing() {
    let mut h = Ui1::new();
    h.app
        .apply_chat(vec![crate::app::chat::ChatAction::NewChannel(
            "fresh".into(),
        )]);
    h.app.section = Section::Chat;
    h.settle();
    let root = temp_home("ui-slack-thread");
    let ex = root.join("export");
    std::fs::create_dir_all(ex.join("design")).unwrap();
    std::fs::write(ex.join("users.json"), "[]").unwrap();
    std::fs::write(
        ex.join("channels.json"),
        r#"[{"id":"C1","name":"design","members":[]}]"#,
    )
    .unwrap();
    std::fs::write(
        ex.join("design/2024-01-02.json"),
        r#"[{"type":"message","text":"first sketch","ts":"1704189600.000100"}]"#,
    )
    .unwrap();
    let zip = root.join("slack.zip");
    crate::app::slack::zip_folder(&ex, &zip, false);
    crate::app::slack::PICKED.with(|p| *p.borrow_mut() = Some(zip));
    h.click("Import from Slack");
    // the export is read on its own thread; frames alone open the dialog
    let started = std::time::Instant::now();
    while !matches!(h.app.modal, Some(Modal::SlackImport(_))) {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "the import dialog opens once the export is read"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.settle();
    }
    // the "Reading" toast goes once the dialog is up
    assert!(!h.has("Reading the Slack export..."));
    assert!(h.has("Import"));
}
