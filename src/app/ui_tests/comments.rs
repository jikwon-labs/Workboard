// Click-through tests for task comments and activity: posting, reacting,
// assigning, mentions and links in comments.

use super::*;

#[test]
fn task_window_comment_post_tabs_tools_and_search() {
    let mut h = Ui1::new();
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.unwrap();
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .activity
        .retain(|a| !a.comment);
    h.settle();
    // empty: the two shortcut buttons
    assert!(h.has("@  Mention a teammate") && h.has("Assign this task"));
    h.click("@  Mention a teammate");
    assert!(
        h.ctx.memory(|m| m.any_popup_open()),
        "the people list opens"
    );
    h.click("Alex Kim");
    assert_eq!(h.app.comment.text, "@Alex Kim ");
    h.type_text("please check");
    h.click("Emoji");
    h.click("\u{1F44D}");
    h.click("Send Comment");
    let t = h.app.task(id).unwrap().clone();
    let c = t.activity.iter().rev().find(|a| a.comment).unwrap();
    assert_eq!(c.text, "@Alex Kim please check\u{1F44D}");
    assert_eq!(c.mentions, vec![2], "the mention keeps Alex Kim's id");
    assert!(!c.post);
    // Post tab
    h.click("Post");
    h.click("Comment box");
    h.type_text("Release notes");
    h.click("Send Post");
    assert!(h
        .app
        .task(id)
        .unwrap()
        .activity
        .iter()
        .any(|a| a.post && a.text == "Release notes"));
    // search narrows the comments
    h.click("Search comments button");
    h.type_text("release");
    assert!(h.has("Release notes"));
    assert!(
        !h.has("@Alex Kim please check\u{1F44D}"),
        "the other comment is hidden"
    );
}

#[test]
fn open_toolbar_search_does_not_steal_the_comment_box() {
    let mut h = Ui1::new();
    h.click("Search tasks");
    assert!(h.app.show_search);
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.expect("task window opens");
    h.click("Comment box");
    h.type_text("zzleak");
    h.settle();
    assert_eq!(
        h.app.comment.text, "zzleak",
        "typing lands in the comment box"
    );
    assert!(h.app.search.is_empty(), "the toolbar search stays empty");
    h.key(egui::Key::Escape);
    h.key(egui::Key::Escape);
    assert!(h.app.open_task.is_none() || h.app.open_task == Some(id));
    assert!(
        h.app.search.is_empty(),
        "the list is not filtered by the comment text"
    );
}

#[test]
fn comment_send_with_cmd_enter_then_react_edit_reply_delete() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    h.click("Comment box");
    h.type_text("First thoughts");
    h.cmd_enter();
    let c = h.comments(id);
    assert_eq!(c.len(), 1, "Cmd+Enter sends");
    assert_eq!(c[0].text, "First thoughts");
    assert_ne!(c[0].id, 0);
    assert!(h.app.comment.text.is_empty());
    // react
    h.click("Add reaction");
    h.click("\u{1F44D}");
    assert_eq!(h.comments(id)[0].reactions[0].users, vec![1]);
    // the reaction chip takes it back
    h.click("\u{1F44D} 1");
    assert!(h.comments(id)[0].reactions.is_empty());
    // edit my comment
    h.click("Comment actions");
    h.click("Edit");
    h.type_text(" (more)");
    h.click("Save comment");
    let c = h.comments(id);
    assert_eq!(c[0].text, "First thoughts (more)");
    assert!(c[0].edited.is_some());
    assert!(h.has("(edited)"));
    // reply
    h.click("Comment actions");
    h.click("Reply");
    h.type_text("A reply");
    h.click("Send reply");
    let c = h.comments(id);
    assert_eq!(c.len(), 2);
    assert_eq!(c[1].parent, Some(c[0].id));
    assert!(h.has("A reply"));
    // deleting the comment takes its replies with it
    h.click_side("Comment actions", false);
    h.click("Delete comment");
    assert!(h.comments(id).is_empty());
}

#[test]
fn comment_assign_and_resolve_shows_in_assigned_comments() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    h.click("Comment box");
    h.type_text("Please review");
    h.click("Send Comment");
    h.click("Comment actions");
    h.click("Assign comment");
    h.click("Me");
    let c = h.comments(id);
    assert_eq!(c[0].assignee, Some(1));
    assert!(h.has("Assigned to Me"));
    // it is on the Assigned Comments page until resolved there
    h.app.open_task = None;
    h.app.loc = Location::Assigned;
    h.settle();
    assert!(h.has("Resolve comment on Write copy for landing page"));
    h.click("Resolve comment on Write copy for landing page");
    assert!(h.comments(id)[0].resolved);
    assert!(h.app.open_task.is_none(), "ticking does not open the task");
    assert!(
        !h.has("Resolve comment on Write copy for landing page"),
        "resolved comments drop out"
    );
    // and it can be reopened from the task
    h.app.apply(Action::Open(id));
    h.settle();
    h.click("Reopen comment");
    assert!(!h.comments(id)[0].resolved);
}

#[test]
fn comment_checklist_ticks_and_links_open() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    let other = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.id != id && t.parent.is_none())
        .unwrap()
        .id;
    let link = format!("workboard://task/{}", other);
    {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.activity.push(Activity {
            ts: now_ts(),
            author: 2,
            text: format!("To do:\n\u{2610} buy milk\nsee {}", link),
            comment: true,
            id: 77_001,
            ..Default::default()
        });
    }
    h.settle();
    h.click("Tick buy milk");
    assert!(h.comments(id)[0].text.contains("\u{2611} buy milk"));
    assert!(h.comments(id)[0].edited.is_none(), "ticking is no edit");
    h.click(&link);
    assert_eq!(h.app.open_task, Some(other));
}

#[test]
fn typing_at_opens_the_people_list() {
    let mut h = Ui1::new();
    h.open_clean_task();
    h.click("Comment box");
    h.type_text("hey @");
    h.settle();
    assert!(
        h.ctx.memory(|m| m.any_popup_open()),
        "the people list opens"
    );
    let alex = h.tree.iter().filter(|(n, _)| n == "Alex Kim").count();
    h.type_text("Sa");
    h.settle();
    assert!(
        h.ctx.memory(|m| m.any_popup_open()),
        "still open while typing a name"
    );
    let alex_after = h.tree.iter().filter(|(n, _)| n == "Alex Kim").count();
    assert!(
        h.has("Sam Lee") && alex_after < alex,
        "narrowed by what is typed"
    );
    // (the sidebar has a "Sam Lee" too, below the window)
    let pick = h
        .tree
        .iter()
        .find(|(n, r)| n == "Sam Lee" && r.center().y < 900.0)
        .map(|(_, r)| r.center())
        .unwrap();
    h.click_at(pick);
    assert_eq!(h.app.comment.text, "hey @Sam Lee ");
}

#[test]
fn mentions_stand_out_and_comments_only_hides_activity() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.activity.push(Activity {
            ts: now_ts(),
            author: 1,
            text: "ping @Alex Kim now".into(),
            comment: true,
            id: 77_002,
            ..Default::default()
        });
    }
    h.settle();
    // the mention is its own piece of text
    assert!(h.has("@Alex Kim"));
    let line = h
        .app
        .task(id)
        .unwrap()
        .activity
        .iter()
        .find(|a| !a.comment)
        .map(|a| a.text.clone())
        .unwrap();
    let shown = format!("\u{2022} Me {}", line);
    assert!(h.has(&shown), "activity lines show by default");
    h.click("Comments only");
    assert!(!h.has(&shown), "hidden by Comments only");
    assert!(h.has("@Alex Kim"));
}

#[test]
fn mention_picked_from_the_list_keeps_that_person_when_names_are_shared() {
    let mut h = Ui1::new();
    h.app.data.members.push(Member {
        id: 9,
        name: "Alex Kim".into(),
        color: [1, 2, 3],
        ..Default::default()
    });
    h.click_left("Task Write copy for landing page", 120.0);
    let id = h.app.open_task.unwrap();
    h.app
        .data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .activity
        .retain(|a| !a.comment);
    h.settle();
    h.click("@  Mention a teammate");
    // the second "Alex Kim" in the list (the one just added) is the lower row
    // (the list's rows are 200 points wide)
    let rows: Vec<Pos2> = h
        .tree
        .iter()
        .filter(|(n, r)| n == "Alex Kim" && r.width() == 200.0)
        .map(|(_, r)| r.center())
        .collect();
    assert!(rows.len() >= 2, "both people are in the list: {:?}", rows);
    let lowest = rows
        .iter()
        .copied()
        .fold(rows[0], |a, b| if b.y > a.y { b } else { a });
    h.click_at(lowest);
    assert_eq!(h.app.comment.text, "@Alex Kim ");
    h.type_text("this one");
    h.click("Send Comment");
    let c = h
        .app
        .task(id)
        .unwrap()
        .activity
        .iter()
        .rev()
        .find(|a| a.comment)
        .unwrap()
        .clone();
    assert_eq!(c.text, "@Alex Kim this one");
    assert_eq!(
        c.mentions,
        vec![9],
        "only the person picked, not both Alex Kims"
    );
}

#[test]
fn comment_links_to_private_or_archived_places_do_not_open() {
    let mut h = Ui1::new();
    let id = h.open_clean_task();
    let me = h.app.data.me;
    let task_list = h
        .app
        .data
        .tasks
        .iter()
        .find(|t| t.id == id)
        .unwrap()
        .list_id;
    // a list in another space, made someone else's Personal space
    let (sid, lid) = h
        .app
        .data
        .spaces
        .iter()
        .filter(|s| s.lists.iter().all(|l| l.id != task_list))
        .find_map(|s| s.lists.first().map(|l| (s.id, l.id)))
        .unwrap();
    h.app
        .data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .private_owner = Some(me + 100);
    let link = format!("workboard://list/{}", lid);
    let folder_link = "workboard://folder/987654".to_string();
    {
        let t = h.app.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.activity.push(Activity {
            ts: now_ts(),
            author: 2,
            text: format!("see {}\nand {}", link, folder_link),
            comment: true,
            id: 77_002,
            ..Default::default()
        });
    }
    h.settle();
    let loc = h.app.loc;
    h.click(&link);
    assert_eq!(h.app.open_task, Some(id), "the task stays open");
    assert_eq!(h.app.loc, loc);
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("archived or private")));
    // a folder link is a link too, and one this workspace doesn't have says so
    h.app.toast = None;
    h.click(&folder_link);
    assert!(h
        .app
        .toast
        .as_ref()
        .is_some_and(|(t, _)| t.contains("can't be opened")));
    // my own Personal space opens like a link in chat: task window closed
    h.app
        .data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .private_owner = Some(me);
    h.click(&link);
    assert_eq!(h.app.open_task, None);
    assert_eq!(h.app.loc, Location::List(lid));
}
