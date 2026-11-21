// Click-through tests for the Inbox page: moving rows between Primary,
// Other, Later and Cleared with the buttons a row shows under the pointer,
// the filter, and what the rows say.

use super::*;

/// The task the Inbox rows below are about: mine (I made it and I'm on it).
const CASE: &str = "Inbox case";

/// A window on the Inbox with only the rows `setup` adds: every line by
/// someone else on a task and every message are taken out first.
fn inbox(setup: impl FnOnce(&mut Data)) -> Ui1 {
    let mut h = Ui1::new();
    let me = h.app.data.me;
    for t in &mut h.app.data.tasks {
        t.activity.retain(|a| a.author == me);
    }
    for c in &mut h.app.data.channels {
        c.messages.clear();
    }
    setup(&mut h.app.data);
    h.app.data_changed();
    // the pointer is on the window, as it is for a person
    h.hover(pos2(700.0, 400.0));
    h.click("Inbox");
    h
}

/// Adds the task `CASE` (or `name`) with a comment by Alex `ago` seconds
/// ago; returns the comment's Inbox key.
fn comment_on(d: &mut Data, name: &str, text: &str, ago: i64) -> String {
    let me = d.me;
    let ts = now_ts() - ago;
    let list = d.tasks[0].list_id;
    let id = match d.tasks.iter().find(|t| t.name == name) {
        Some(t) => t.id,
        None => {
            let id = d.new_id();
            d.tasks.push(Task {
                id,
                list_id: list,
                name: name.into(),
                assignees: vec![me],
                activity: vec![Activity {
                    ts: ts - 100,
                    author: me,
                    text: "created this task".into(),
                    ..Default::default()
                }],
                ..d.tasks[0].clone()
            });
            id
        }
    };
    let t = d.tasks.iter_mut().find(|t| t.id == id).unwrap();
    t.activity.push(Activity {
        ts,
        author: 2,
        text: text.into(),
        comment: true,
        ..Default::default()
    });
    format!("{}:{}:2", id, ts)
}

/// Puts the pointer on the row named `name`, so it shows its buttons.
fn hover_row(h: &mut Ui1, name: &str) {
    let p = h
        .find(name)
        .unwrap_or_else(|| panic!("no Inbox row named {:?}", name));
    h.hover(p);
}

#[test]
fn later_on_a_cleared_row_moves_it_to_later_and_it_can_go_back() {
    let mut key = String::new();
    let mut h = inbox(|d| {
        key = comment_on(d, CASE, "Over to you", 60);
        d.extra.inbox_cleared.push(key.clone());
    });
    h.click("Inbox Cleared");
    hover_row(&mut h, CASE);
    h.click("Later");
    let x = &h.app.data.extra;
    assert!(!x.inbox_cleared.contains(&key), "no longer cleared");
    assert!(x.inbox_later.contains(&key), "saved for later");
    assert!(!h.has(CASE), "gone from Cleared");
    h.click("Inbox Later");
    hover_row(&mut h, CASE);
    // and from Later back to Primary, where it came from
    h.click("Primary");
    h.click("Inbox Primary");
    assert!(h.has(CASE));
    assert!(h.app.data.extra.inbox_later.is_empty());
    assert!(h.app.data.extra.inbox_cleared.is_empty());
}

/// Adds a message in #general by `author` that mentions me; returns its
/// Inbox key.
fn mention_in_general(d: &mut Data, author: u64, text: &str, ago: i64) -> String {
    let me = d.me;
    let id = d.new_id();
    let mut m = crate::app::slack::new_message(id, author, text.into(), now_ts() - ago);
    m.mentions = vec![me];
    let c = d
        .channels
        .iter_mut()
        .find(|c| c.name == "general")
        .expect("#general");
    c.messages.push(m);
    format!("chat:{}:{}", c.id, id)
}

#[test]
fn an_imported_row_offers_no_restore_it_cannot_do() {
    let mut key = String::new();
    let mut h = inbox(|d| {
        key = mention_in_general(d, 2, "@Me an old Slack mention", 30 * 86400);
        let c = d.channels.iter_mut().find(|c| c.name == "general").unwrap();
        c.messages.last_mut().unwrap().imported = Some("slack".into());
    });
    // old history starts out cleared
    h.click("Inbox Cleared");
    hover_row(&mut h, "#general");
    assert!(
        !h.has("Restore"),
        "imported history stays cleared, so Restore would do nothing"
    );
    // it can still be saved for later, and cleared again from there
    h.click("Later");
    assert!(h.app.data.extra.inbox_later.contains(&key));
    h.click("Inbox Later");
    hover_row(&mut h, "#general");
    h.click("Clear");
    h.click("Inbox Cleared");
    assert!(h.has("#general"));
}
