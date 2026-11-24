// Click-through tests for a doc's Version history: versions kept as the doc
// is edited, the list, a version shown read-only and Restore this version.

use super::*;

fn versions(h: &Ui1, id: u64) -> Vec<DocVersion> {
    let d = h.app.data.docs.iter().find(|d| d.id == id).unwrap();
    d.versions_newest().into_iter().cloned().collect()
}

#[test]
fn editing_twice_a_few_minutes_apart_lists_two_versions_and_restore_brings_one_back() {
    let mut h = Ui1::new();
    let id = h.new_doc_focused();
    h.type_text("First draft");
    assert!(h.app.doc_editing());
    assert!(versions(&h, id).is_empty(), "kept once the editing stops");
    // nothing typed for a while: the text is kept as a version
    h.app.age_doc_edit(3 * DOC_VERSION_GAP);
    h.settle();
    assert!(!h.app.doc_editing());
    let v = versions(&h, id);
    assert_eq!(v.len(), 1, "an empty new doc needs no version before it");
    assert_eq!(
        (v[0].body.as_str(), v[0].by),
        ("First draft", h.app.data.me)
    );

    // a few minutes later, more
    h.type_text(" and more");
    h.app.age_doc_edit(DOC_VERSION_GAP + 1);
    h.settle();
    let v = versions(&h, id);
    assert_eq!(
        v.iter().map(|v| v.body.as_str()).collect::<Vec<_>>(),
        vec!["First draft and more", "First draft"]
    );

    // the doc's menu opens the list; the older one shows read-only
    h.click("Doc options");
    h.click("Version history");
    assert!(h.has(&format!("Doc version {}", v[0].id)));
    assert!(h.has(&format!("Doc version {}", v[1].id)));
    assert!(
        h.texts()
            .iter()
            .any(|t| t.contains("This is the doc's text now")),
        "the newest is picked and reads as the doc"
    );
    assert!(!h.has("Restore this version"));
    h.click(&format!("Doc version {}", v[1].id));
    assert!(h.has("Version text"));
    assert!(h.texts().iter().any(|t| t == "First draft"));
    assert_eq!(
        h.doc_body(id),
        "First draft and more",
        "only shown, not changed"
    );

    h.click("Restore this version");
    assert_eq!(h.doc_body(id), "First draft");
    let after = versions(&h, id);
    assert_eq!(after.len(), 3, "the restore is a version too");
    assert_eq!(after[0].restored, Some(v[1].at));
    assert_eq!(after[0].body, "First draft");

    // and it can be undone the same way
    h.click(&format!("Doc version {}", v[0].id));
    h.click("Restore this version");
    assert_eq!(h.doc_body(id), "First draft and more");
    assert_eq!(versions(&h, id).len(), 4);
}

#[test]
fn a_doc_written_before_versions_keeps_its_text_when_first_edited() {
    let mut h = Ui1::new();
    let id = h.new_doc_focused();
    let d = h.app.data.docs.iter_mut().find(|d| d.id == id).unwrap();
    d.body = "Old text".into();
    d.updated = 1000;
    h.app.doc_session = None;
    h.settle();
    h.type_text("!");
    let v = versions(&h, id);
    assert_eq!(v.len(), 1, "the text before the edit, right away");
    assert_eq!((v[0].body.as_str(), v[0].by), ("Old text", 0));
    assert!(
        v[0].at > 1000,
        "kept as of the edit: a text older than versions are kept stays"
    );
    // opening another doc ends the editing: its version is kept
    h.app.doc_open = None;
    h.settle();
    let v = versions(&h, id);
    assert_eq!(v.len(), 2);
    assert!(v[0].body.contains('!'));
}

#[test]
fn a_view_only_doc_offers_no_restore() {
    let mut h = Ui1::new();
    let id = h.new_doc_focused();
    h.type_text("Text");
    h.app.age_doc_edit(DOC_VERSION_GAP);
    h.settle();
    h.type_text(" more");
    h.app.age_doc_edit(3 * DOC_VERSION_GAP);
    h.settle();
    let older = versions(&h, id)[1].id;
    // the doc goes in a space shared with me to view only, on a server
    let sid = h.app.data.spaces[0].id;
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.app.data.spaces[0].sharing = Some(SpaceSharing {
        everyone: false,
        people: vec![SpaceGrant {
            member: h.app.data.me,
            level: "view".into(),
        }],
    });
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .space = Some(sid);
    h.app.open_doc_versions(id);
    h.settle();
    h.click(&format!("Doc version {}", older));
    assert!(h.has("Version text"));
    assert!(!h.has("Restore this version"));
}
