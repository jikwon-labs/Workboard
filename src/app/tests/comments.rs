// Behaviour tests for task comments and activity: mentions, links,
// checklists, reactions, edits and ids that match on every computer.

use super::*;

#[test]
fn comment_lines_split_into_mentions_and_links() {
    let names = vec!["Alex Kim".to_string(), "Alex".to_string()];
    let segs = segments("hi @Alex Kim see workboard://task/42, ok", &names);
    assert_eq!(
        segs,
        vec![
            Seg::Text("hi "),
            Seg::Mention("@Alex Kim"),
            Seg::Text(" see "),
            Seg::Link("workboard://task/42", LinkTo::Task(42)),
            Seg::Text(", ok")
        ]
    );
    // an @ inside a word is no mention
    assert_eq!(segments("mail@Alex", &names), vec![Seg::Text("mail@Alex")]);
    assert_eq!(
        parse_link("workboard://list/7").map(|x| x.0),
        Some(LinkTo::Loc(Location::List(7)))
    );
    assert_eq!(
        parse_link("workboard://space/9 more").map(|x| x.0),
        Some(LinkTo::Loc(Location::Space(9)))
    );
    assert_eq!(parse_link("workboard://task/"), None);
}

#[test]
fn comment_checklist_items_tick_and_untick() {
    let text = "Todo:\n\u{2610} one\n\u{2610} two";
    let once = toggle_comment_check(text, 2);
    assert_eq!(once, "Todo:\n\u{2610} one\n\u{2611} two");
    assert_eq!(toggle_comment_check(&once, 2), text);
    // a line that is no checklist item stays as it is
    assert_eq!(toggle_comment_check(text, 0), text);
}

#[test]
fn comment_reactions_toggle_per_person() {
    let mut rs = vec![];
    toggle_reaction(&mut rs, "\u{1F44D}", 1);
    toggle_reaction(&mut rs, "\u{1F44D}", 2);
    assert_eq!(rs[0].users, vec![1, 2]);
    toggle_reaction(&mut rs, "\u{1F44D}", 1);
    toggle_reaction(&mut rs, "\u{1F44D}", 2);
    assert!(rs.is_empty());
}

#[test]
fn mention_word_reads_the_name_being_typed() {
    assert_eq!(mention_word("hi @"), Some(""));
    assert_eq!(mention_word("hi @Al"), Some("Al"));
    assert_eq!(mention_word("@Al"), Some("Al"));
    assert_eq!(mention_word("hi @Al and"), None);
    assert_eq!(mention_word("mail@x"), None);
}

#[test]
fn old_activity_gets_the_same_ids_on_every_computer() {
    // a file from before activity had ids
    let json = r#"{"ts": 5, "author": 1, "text": "hi", "comment": true}"#;
    let a: Activity = serde_json::from_str(json).unwrap();
    assert_eq!(a.id, 0);
    let mut d1 = Data::seed();
    let mut d2 = Data::seed();
    for d in [&mut d1, &mut d2] {
        let t = &mut d.tasks[0];
        t.activity = vec![
            a.clone(),
            a.clone(),
            Activity {
                ts: 6,
                author: 2,
                text: "x".into(),
                ..Default::default()
            },
        ];
        d.fix_activity_ids();
    }
    let ids1: Vec<u64> = d1.tasks[0].activity.iter().map(|a| a.id).collect();
    let ids2: Vec<u64> = d2.tasks[0].activity.iter().map(|a| a.id).collect();
    assert_eq!(ids1, ids2);
    assert!(ids1.iter().all(|i| *i != 0));
    // two identical old lines still get different ids
    assert_ne!(ids1[0], ids1[1]);
    // and the app works out the same ids wb-sync does
    let mut v = serde_json::json!({"tasks": [{"id": d1.tasks[0].id, "activity": [json_val(&a), json_val(&a)]}]});
    wb_sync::fill_activity_ids(&mut v);
    assert_eq!(v["tasks"][0]["activity"][1]["id"].as_u64(), Some(ids1[1]));
    // id 0 is never written, so an entry without an id never looks like id 0
    assert!(!serde_json::to_string(&a).unwrap().contains("\"id\""));
}

fn json_val(a: &Activity) -> serde_json::Value {
    serde_json::to_value(a).unwrap()
}

#[test]
fn edited_and_deleted_comments_carry_over_between_copies() {
    use wb_sync::{merge, stamp, SyncMeta};
    let mut base = Data::seed();
    base.tasks.truncate(1);
    base.tasks[0].activity = vec![
        Activity {
            ts: 1,
            author: 1,
            text: "first".into(),
            comment: true,
            id: 90_001,
            ..Default::default()
        },
        Activity {
            ts: 2,
            author: 2,
            text: "second".into(),
            comment: true,
            id: 90_002,
            ..Default::default()
        },
    ];
    let mut ma = SyncMeta::default();
    let mut mb = SyncMeta::default();
    let va = serde_json::to_value(&base).unwrap();
    stamp(&va, &mut ma, 1000);
    stamp(&va, &mut mb, 1000);
    // A edits the first comment, B deletes the second and adds a third
    let mut a = base.clone();
    a.tasks[0].activity[0].text = "first (fixed)".into();
    a.tasks[0].activity[0].edited = Some(3);
    let mut b = base.clone();
    b.tasks[0].activity.remove(1);
    b.tasks[0].activity.push(Activity {
        ts: 4,
        author: 3,
        text: "third".into(),
        comment: true,
        id: 90_003,
        ..Default::default()
    });
    let mut va = serde_json::to_value(&a).unwrap();
    stamp(&va, &mut ma, 2000);
    va["extra"]["sync"] = serde_json::to_value(&ma).unwrap();
    let mut vb = serde_json::to_value(&b).unwrap();
    stamp(&vb, &mut mb, 2000);
    vb["extra"]["sync"] = serde_json::to_value(&mb).unwrap();
    merge(&mut va, &[vb]);
    let merged: Data = serde_json::from_value(va).unwrap();
    let texts: Vec<&str> = merged.tasks[0]
        .activity
        .iter()
        .map(|a| a.text.as_str())
        .collect();
    assert_eq!(texts, vec!["first (fixed)", "third"]);
}

#[test]
fn typing_a_name_or_description_logs_one_line() {
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Plan");
    let n = a.task(id).unwrap().activity.len();
    // the name box sends a change on every key press
    for name in ["Plan t", "Plan tr", "Plan trip"] {
        let mut t = a.task(id).unwrap().clone();
        t.name = name.into();
        a.apply(Action::Replace(t));
    }
    let t = a.task(id).unwrap().clone();
    assert_eq!(t.activity.len(), n + 1);
    assert_eq!(
        t.activity.last().unwrap().text,
        "renamed this task to \"Plan trip\""
    );
    let mut t2 = t.clone();
    t2.description = "Notes".into();
    t2.tags.push("travel".into());
    t2.estimate_min = Some(90);
    a.apply(Action::Replace(t2));
    let texts: Vec<String> = a
        .task(id)
        .unwrap()
        .activity
        .iter()
        .map(|x| x.text.clone())
        .collect();
    assert!(texts.contains(&"updated the description".to_string()));
    assert!(texts.contains(&"added tag travel".to_string()));
    assert!(
        texts.contains(&"set the time estimate to 1h 30m".to_string()),
        "{:?}",
        texts
    );
}

#[test]
fn each_task_keeps_its_own_comment_draft() {
    let mut a = app();
    let (t1, t2) = (a.data.tasks[0].id, a.data.tasks[1].id);
    a.apply(Action::Open(t1));
    a.comment.text = "half a thought".into();
    a.apply(Action::Open(t2));
    assert!(a.comment.text.is_empty());
    a.comment.text = "other".into();
    a.apply(Action::Open(t1));
    assert_eq!(a.comment.text, "half a thought");
    a.apply(Action::Open(t2));
    assert_eq!(a.comment.text, "other");
}

#[test]
fn a_copy_from_before_comment_ids_merges_without_doubles() {
    let mut d = Data::seed();
    d.tasks.truncate(1);
    let tid = d.tasks[0].id;
    d.tasks[0].activity = vec![Activity {
        ts: 1,
        author: 1,
        text: "old".into(),
        comment: true,
        ..Default::default()
    }];
    // the other computer still runs an app without ids and added a comment
    let mut old = serde_json::to_value(&d).unwrap();
    old["tasks"][0]["activity"].as_array_mut().unwrap().push(
        serde_json::json!({"ts": 2, "author": 2, "text": "from the old app", "comment": true}),
    );
    d.fix_activity_ids();
    let mut mine = serde_json::to_value(&d).unwrap();
    wb_sync::merge(&mut mine, &[old]);
    let merged: Data = serde_json::from_value(mine).unwrap();
    let t = merged.tasks.iter().find(|t| t.id == tid).unwrap();
    let texts: Vec<&str> = t.activity.iter().map(|a| a.text.as_str()).collect();
    assert_eq!(texts, vec!["old", "from the old app"]);
    assert!(t.activity.iter().all(|a| a.id != 0));
}
