// Click-through tests for Docs: the editor, pages and sub-pages, the Docs
// list and hub, wikis and doc links.

use super::*;

fn doc_id(h: &Ui1, title: &str) -> u64 {
    h.app
        .data
        .docs
        .iter()
        .find(|d| d.title == title)
        .map(|d| d.id)
        .expect("doc exists")
}

#[test]
fn click_more_docs_new_doc() {
    let mut h = Ui1::new();
    let n = h.app.data.docs.len();
    h.click("Rail More");
    h.click("Docs");
    assert_eq!(h.app.section, Section::Docs);
    h.click("New Doc");
    assert_eq!(h.app.data.docs.len(), n + 1);
    assert!(h.app.doc_open.is_some(), "the new doc opens");
}

#[test]
fn click_docs_template_and_timesheet_options() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Timesheets");
    assert_eq!(h.app.section, Section::Timesheets);
    h.click("All timesheets");
    assert_eq!(h.app.hub_filter, 1);
    h.click("My timesheet");
    assert_eq!(h.app.hub_filter, 0);
}

#[test]
fn doc_slash_command_and_sub_page() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    // the empty body has no visible name to click; put the cursor in it
    h.ctx
        .memory_mut(|m| m.request_focus(egui::Id::new(("doc_body", id))));
    h.settle();
    h.type_text("/");
    h.settle();
    h.click("Heading 1");
    h.type_text("Plan");
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == id).unwrap().body,
        "# Plan"
    );
    h.click("+  Add page");
    assert!(h.app.data.docs.iter().any(|d| d.parent == Some(id)));
}

#[test]
fn my_docs_badge_counts_docs_not_sub_pages() {
    let mut h = Ui1::new();
    let before = h.app.my_docs_count();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    h.click("+  Add page");
    assert!(
        h.app.data.docs.iter().any(|d| d.parent == Some(id)),
        "a sub page was made"
    );
    assert_eq!(
        h.app.my_docs_count(),
        before + 1,
        "the sub page is not counted"
    );
}

#[test]
fn doc_edit_mode_draws_formatting_while_typing() {
    use crate::app::docs::{doc_layout_job, line_kinds, LineKind};
    let text = "# Title\n- item\n- [x] done\n> quote\n**bold** text";
    let kinds: Vec<LineKind> = line_kinds(text).into_iter().map(|k| k.0).collect();
    assert_eq!(
        kinds,
        vec![
            LineKind::Heading(1),
            LineKind::Bullet,
            LineKind::Check(true),
            LineKind::Quote,
            LineKind::Plain
        ]
    );
    let t = theme::theme_accent(false, "");
    let job = doc_layout_job(text, &t, 600.0);
    assert_eq!(job.text, text, "the stored Markdown is laid out unchanged");
    let fmt_at = |needle: &str| {
        let b = job.text.find(needle).unwrap();
        job.sections
            .iter()
            .find(|s| s.byte_range.contains(&b))
            .unwrap()
            .format
            .clone()
    };
    assert_eq!(
        fmt_at("Title").font_id.size,
        28.0,
        "heading is big while editing"
    );
    assert!(fmt_at("quote").italics);
    assert!(
        fmt_at("done").strikethrough.width > 0.0,
        "a ticked item is struck through"
    );
    assert_eq!(
        fmt_at("- item").color,
        Color32::TRANSPARENT,
        "the bullet marker is replaced by a painted dot"
    );
}

#[test]
fn doc_edit_mode_hides_markers_off_the_cursor_line() {
    use crate::app::docs::{cursor_lines, doc_layout_job_live};
    let text = "# Title\n**bold** and [site](https://a.b)\n---\n# ";
    let t = theme::theme_accent(false, "");
    let fmt = |job: &egui::text::LayoutJob, b: usize| {
        job.sections
            .iter()
            .find(|s| s.byte_range.contains(&b))
            .unwrap()
            .format
            .clone()
    };
    // the cursor is on the first line: its "# " shows, the rest are gone
    let job = doc_layout_job_live(text, &t, 600.0, cursor_lines(text, 3, 3));
    assert_eq!(job.text, text, "the stored Markdown is laid out unchanged");
    assert_eq!(
        fmt(&job, 0).font_id.size,
        28.0,
        "the cursor's heading marker shows"
    );
    let bold = text.find("**").unwrap();
    assert!(
        fmt(&job, bold).font_id.size < 1.0,
        "bold markers off the cursor line shrink away"
    );
    let url = text.find("https").unwrap();
    assert!(
        fmt(&job, url).font_id.size < 1.0,
        "a link's address is hidden"
    );
    let site = text.find("site").unwrap();
    assert_eq!(fmt(&job, site).font_id.size, 16.0, "the link text stays");
    let rule = text.find("---").unwrap();
    assert_eq!(
        fmt(&job, rule).color,
        Color32::TRANSPARENT,
        "a divider is only the painted line"
    );
    let empty_heading = text.rfind("# ").unwrap();
    assert!(
        fmt(&job, empty_heading).font_id.size > 1.0,
        "an empty heading keeps its marker so the line keeps its height"
    );
    // move the cursor to the second line: now its markers show
    let second = text.find("**").unwrap() + 3;
    let job = doc_layout_job_live(text, &t, 600.0, cursor_lines(text, second, second));
    assert!(fmt(&job, 0).font_id.size < 1.0, "the heading marker hides");
    assert!(
        fmt(&job, bold).font_id.size > 1.0,
        "the cursor line's markers come back"
    );
    assert!(fmt(&job, url).font_id.size > 1.0);
    // a selection over two lines shows both
    assert_eq!(cursor_lines(text, 2, second), 0..2);
}

#[test]
fn doc_typing_formatting_keeps_the_text() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    h.ctx
        .memory_mut(|m| m.request_focus(egui::Id::new(("doc_body", id))));
    h.settle();
    h.type_text("# Plan");
    h.click("B");
    h.type_text("x");
    h.settle();
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == id).unwrap().body,
        "# Plan**x**"
    );
}

#[test]
fn doc_checkbox_toggles_and_new_page_opens_in_edit() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    assert!(h.has("Start writing"), "an empty page offers how to start");
    h.click("Table");
    assert!(h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .body
        .starts_with("| Column 1"));
    let mut body = "- [ ] a\n- [x] b".to_string();
    crate::app::docs::toggle_check_at(&mut body, 0);
    crate::app::docs::toggle_check_at(&mut body, 8);
    assert_eq!(body, "- [x] a\n- [ ] b");
    // View on this page, then a new page: it opens ready to write
    h.click("View");
    assert!(h.app.doc_preview);
    h.click("+  Add page");
    assert!(!h.app.doc_preview, "a new page opens in Edit");
}

#[test]
fn doc_empty_page_add_subpage() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    h.settle();
    let id = h.app.doc_open.unwrap();
    h.click("Subpage");
    assert!(h.app.data.docs.iter().any(|d| d.parent == Some(id)));
    assert_ne!(h.app.doc_open, Some(id), "the sub page opens");
}

#[test]
fn docs_pages_panel_menu_duplicates_reorders_moves_and_deletes() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let root = h.app.doc_open.unwrap();
    let a = h.app.add_page(root);
    let b = h.app.add_page(root);
    h.app.doc_open = Some(root);
    h.settle();
    // duplicate "Page 1": a copy sits under the same parent
    h.right_click("Page Page 1", 40.0);
    h.click("Duplicate");
    let copy = h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.title == "Page 1 (copy)")
        .expect("a copy is made");
    assert_eq!(copy.parent, Some(root));
    // move "Page 2" up past the copy and "Page 1"
    let order = |h: &Ui1| {
        h.app
            .data
            .docs
            .iter()
            .filter(|d| d.parent == Some(root))
            .map(|d| d.id)
            .collect::<Vec<_>>()
    };
    let before = order(&h);
    h.right_click("Page Page 2", 40.0);
    h.click("Move up");
    let after = order(&h);
    assert_ne!(before, after);
    assert_eq!(
        after.iter().position(|x| *x == b).unwrap() + 1,
        before.iter().position(|x| *x == b).unwrap()
    );
    // move "Page 2" under "Page 1"
    h.right_click("Page Page 2", 40.0);
    let mt = h.rect("Move to");
    h.hover(mt.center());
    h.click_side("Page 1", true);
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == b).unwrap().parent,
        Some(a)
    );
    // deleting "Page 1" takes its sub page with it, as one Trash item
    let trash = h.app.data.extra.trash.len();
    h.right_click("Page Page 1", 40.0);
    h.click("Delete");
    assert!(!h.app.data.docs.iter().any(|d| d.id == a || d.id == b));
    assert_eq!(h.app.data.extra.trash.len(), trash + 1);
}

#[test]
fn docs_delete_keeps_sub_pages_and_restore_brings_them_back() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let root = h.app.doc_open.unwrap();
    let a = h.app.add_page(root);
    let aa = h.app.add_page(a);
    h.app.doc_open = Some(root);
    h.settle();
    h.click("Delete");
    assert!(
        !h.app
            .data
            .docs
            .iter()
            .any(|d| [root, a, aa].contains(&d.id)),
        "the whole doc is gone"
    );
    let item = h.app.data.extra.trash.last().unwrap();
    assert_eq!(item.kind, "Doc");
    let i = h.app.data.extra.trash.len() - 1;
    h.app.restore_trash(i);
    let parent = |h: &Ui1, id: u64| {
        h.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.parent)
    };
    assert_eq!(parent(&h, root), Some(None));
    assert_eq!(
        parent(&h, a),
        Some(Some(root)),
        "sub pages come back under their doc"
    );
    assert_eq!(parent(&h, aa), Some(Some(a)));
    // an old single-page Trash item still restores
    let old = Doc {
        id: 999_001,
        space: None,
        owner: 1,
        private: false,
        archived: false,
        meeting_note: false,
        wiki: None,
        views: vec![],
        parent: Some(424242),
        title: "Old".into(),
        body: String::new(),
        created: 0,
        updated: 0,
        tags: vec![],
    };
    h.app.data.extra.trash.push(TrashItem {
        id: 999_002,
        kind: "Doc".into(),
        title: "Old".into(),
        deleted: 0,
        payload: serde_json::to_string(&old).unwrap(),
    });
    let i = h.app.data.extra.trash.len() - 1;
    h.app.restore_trash(i);
    assert_eq!(
        parent(&h, 999_001),
        Some(None),
        "a page whose parent is gone becomes a doc"
    );
}

#[test]
fn docs_header_copy_link_duplicate_move_and_meeting_note() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let id = h.app.doc_open.unwrap();
    h.app.add_page(id);
    h.app.doc_open = Some(id);
    h.settle();
    // move to another Space from the location
    h.click("Doc location");
    h.click("Side Projects");
    let side = h
        .app
        .data
        .spaces
        .iter()
        .find(|s| s.name == "Side Projects")
        .unwrap()
        .id;
    assert!(
        h.app
            .data
            .docs
            .iter()
            .filter(|d| d.id == id || d.parent == Some(id))
            .all(|d| d.space == Some(side)),
        "all pages move"
    );
    // mark as a meeting note by hand
    h.click("Doc options");
    h.click("Mark as Meeting Note");
    assert!(
        h.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .meeting_note
    );
    h.click("Copy link");
    assert_eq!(
        h.app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Link copied")
    );
    let n = h.app.data.docs.len();
    h.click("Duplicate");
    assert_eq!(
        h.app.data.docs.len(),
        n + 2,
        "the copy has its sub page too"
    );
    let copy = h.app.doc_open.unwrap();
    assert_ne!(copy, id);
    assert!(h.app.data.docs.iter().any(|d| d.parent == Some(copy)));
}

#[test]
fn docs_list_right_click_search_and_tags() {
    let mut h = Ui1::new();
    h.app.search = "keep".into();
    h.open_docs_list();
    h.app.search.clear();
    h.settle();
    // the Docs search box is its own, not the task list's filter
    h.click("Search Docs");
    h.type_text("zzz");
    assert_eq!(h.app.docs.search, "zzz");
    assert!(h.app.search.is_empty());
    assert!(!h.has("Doc Project brief"));
    h.app.docs.search.clear();
    h.settle();
    // right click: favorite, meeting note, rename
    h.right_click("Doc Project brief", 60.0);
    h.click("Add to Favorites");
    let id = h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.title == "Project brief")
        .unwrap()
        .id;
    assert!(h.app.data.extra.starred.contains(&format!("doc:{}", id)));
    h.right_click("Doc Project brief", 60.0);
    h.click("Mark as Meeting Note");
    assert!(
        h.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .meeting_note
    );
    h.right_click("Doc Project brief", 60.0);
    h.click("Rename doc");
    // the field starts with the whole name selected
    h.type_text("Project brief v2");
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == id).unwrap().title,
        "Project brief v2"
    );
    h.key(egui::Key::Escape);
    // tags: add one from the doc, then filter the list by it
    h.app.doc_open = Some(id);
    h.settle();
    h.click("Add tag");
    h.type_text("spec");
    h.key(egui::Key::Enter);
    assert_eq!(
        h.app.data.docs.iter().find(|d| d.id == id).unwrap().tags,
        vec!["spec".to_string()]
    );
    h.open_docs_list();
    h.click("Docs tag filter");
    h.click("spec");
    assert_eq!(h.app.docs.tag.as_deref(), Some("spec"));
    assert!(h.has("Doc Project brief v2"));
    h.app.docs.tag = Some("other".into());
    h.settle();
    assert!(
        !h.has("Doc Project brief v2"),
        "a doc without the tag is filtered out"
    );
}

#[test]
fn docs_archived_leave_sidebar_and_sidebar_rows_have_a_menu() {
    let mut h = Ui1::new();
    h.app.section = Section::Home;
    h.settle();
    assert!(h.has("Project brief"));
    h.right_click("Project brief", 60.0);
    assert!(
        h.has("Mark as Meeting Note"),
        "the sidebar doc row has the doc menu"
    );
    h.key(egui::Key::Escape);
    for d in h
        .app
        .data
        .docs
        .iter_mut()
        .filter(|d| d.title == "Project brief")
    {
        d.archived = true;
    }
    h.settle();
    assert!(
        !h.has("Project brief"),
        "an archived doc leaves the sidebar"
    );
}

#[test]
fn docs_editor_keys_bold_enter_tab_and_slash_filter() {
    let mut h = Ui1::new();
    let cmd = egui::Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    };
    let id = h.new_doc_focused();
    // Cmd+B wraps (here: inserts) bold markers
    h.key_with(egui::Key::B, cmd);
    h.type_text("x");
    assert_eq!(h.doc_body(id), "**x**");

    let id = h.new_doc_focused();
    h.type_text("- a");
    h.key(egui::Key::Enter);
    assert_eq!(h.doc_body(id), "- a\n- ", "Enter carries the bullet on");
    h.type_text("b");
    h.key(egui::Key::Tab);
    assert_eq!(h.doc_body(id), "- a\n  - b", "Tab nests the item");
    h.key(egui::Key::Enter);
    h.key(egui::Key::Enter);
    assert_eq!(
        h.doc_body(id),
        "- a\n  - b\n",
        "Enter on an empty item ends the list"
    );

    // "/" then a filter word and the arrow keys
    let id = h.new_doc_focused();
    h.type_text("/");
    h.type_text("head");
    assert!(
        h.has("Heading 3") && !h.has("Quote"),
        "the menu is filtered"
    );
    h.key(egui::Key::ArrowDown);
    h.key(egui::Key::Enter);
    assert_eq!(h.doc_body(id), "## ");
}

#[test]
fn docs_view_mode_lists_links_and_contents() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let id = h.app.doc_open.unwrap();
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .body =
        "# One\n* star item\n3. third\n- [X] done\n## Two\nsee [site](https://example.com)".into();
    h.app.doc_preview = true;
    h.settle();
    assert!(h.has("  \u{2022}  "), "a * bullet is drawn");
    assert!(h.has("  3.  "), "a numbered item keeps its number");
    assert!(h.has("site"), "the link text shows, not the Markdown");
    assert!(!h.has("[site](https://example.com)"));
    assert!(
        h.has("Contents One") && h.has("Contents Two"),
        "headings are listed in On this page"
    );
    h.click("Contents Two");
    // the meeting notes template uses real checklist items
    h.open_docs_list();
    h.click("Template Meeting Notes");
    let d = h
        .app
        .data
        .docs
        .iter()
        .find(|d| Some(d.id) == h.app.doc_open)
        .unwrap();
    assert!(
        d.meeting_note && d.body.ends_with("Action items\n- [ ] "),
        "{:?}",
        d.body
    );
    assert!(crate::app::docs::line_kinds(&d.body)
        .iter()
        .any(|k| matches!(k.0, crate::app::docs::LineKind::Check(false))));
}

#[test]
fn docs_editor_text_helpers() {
    use crate::app::docs::{
        apply_link, continue_list, doc_headings, indent_line, inline_spans, line_kinds, LineKind,
        Span,
    };
    let mut s = "1. a\n".to_string();
    assert_eq!(continue_list(&mut s, 5), Some(8));
    assert_eq!(s, "1. a\n2. ");
    let mut s = "  - [x] a\n".to_string();
    continue_list(&mut s, 10);
    assert_eq!(s, "  - [x] a\n  - [ ] ");
    let mut s = "plain\n".to_string();
    assert_eq!(continue_list(&mut s, 6), None);
    let mut s = "a\n  b".to_string();
    assert_eq!(indent_line(&mut s, 3, false), 2);
    assert_eq!(s, "a\nb");
    let mut s = "see it".to_string();
    let c = apply_link(&mut s, 4, 6);
    assert_eq!(s, "see [it](https://)");
    assert_eq!(c, 17);
    let kinds: Vec<LineKind> = line_kinds("  - a\n    1. b\n  - [ ] c")
        .into_iter()
        .map(|k| k.0)
        .collect();
    assert_eq!(
        kinds,
        vec![LineKind::Bullet, LineKind::Numbered, LineKind::Check(false)]
    );
    let runs = inline_spans("a `c` ~~s~~ [t](u) https://x.y.");
    let kinds: Vec<Span> = runs
        .iter()
        .filter(|r| r.0 != Span::Marker && r.0 != Span::Text)
        .map(|r| r.0)
        .collect();
    assert_eq!(kinds, vec![Span::Code, Span::Strike, Span::Link, Span::Url]);
    assert_eq!(
        runs.iter().map(|r| r.1).collect::<String>(),
        "a `c` ~~s~~ [t](u) https://x.y.",
        "runs cover the line"
    );
    assert_eq!(
        runs.iter().find(|r| r.0 == Span::Url).unwrap().1,
        "https://x.y"
    );
    assert_eq!(
        doc_headings("# A\ntext\n### B"),
        vec![(1, "A".to_string(), 0), (3, "B".to_string(), 2)]
    );
}

#[test]
fn sidebar_leaves_out_other_peoples_private_docs() {
    let mut h = Ui1::new();
    let sid = h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.title == "Project brief")
        .and_then(|d| d.space)
        .unwrap();
    for s in &mut h.app.data.spaces {
        s.expanded = s.id == sid;
    }
    h.settle();
    assert!(h.has("Project brief"), "the space's doc is in the sidebar");
    let me = h.app.data.me;
    let d = h
        .app
        .data
        .docs
        .iter_mut()
        .find(|d| d.title == "Project brief")
        .unwrap();
    d.private = true;
    d.owner = me + 100;
    h.settle();
    assert!(
        !h.has("Project brief"),
        "someone else's Private doc stays out of the space tree"
    );
    // and out of the sidebar search
    h.app.side.search = Some("brief".into());
    h.settle();
    assert!(!h.has("Project brief"));
    // my own Private doc stays
    h.app.side.search = None;
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.title == "Project brief")
        .unwrap()
        .owner = me;
    h.settle();
    assert!(h.has("Project brief"));
}

#[test]
fn docs_marked_as_wiki_show_in_popular_wikis_and_the_wikis_tab() {
    let mut h = Ui1::new();
    h.app.new_doc(None);
    let a = h.app.doc_open.unwrap();
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == a)
        .unwrap()
        .title = "Handbook".into();
    h.app.new_doc(None);
    let b = h.app.doc_open.unwrap();
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == b)
        .unwrap()
        .title = "Onboarding".into();
    h.settle();
    // opening a doc counts one view, not one per frame
    let views = |h: &Ui1, id: u64| {
        h.app
            .data
            .docs
            .iter()
            .find(|d| d.id == id)
            .unwrap()
            .views_since(0)
    };
    assert_eq!(views(&h, b), 1);
    h.settle();
    assert_eq!(views(&h, b), 1);
    // mark both as Wikis from the doc menu
    h.click("Doc options");
    h.click("Mark as Wiki");
    assert!(h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.id == b)
        .unwrap()
        .is_wiki());
    assert!(h.has("Wiki badge"), "the doc header shows the Wiki badge");
    h.app.doc_open = Some(a);
    h.settle();
    h.click("Doc options");
    h.click("Mark as Wiki");
    // open the handbook twice more: it becomes the most popular
    for _ in 0..2 {
        h.app.doc_open = None;
        h.settle();
        h.app.doc_open = Some(a);
        h.settle();
    }
    assert_eq!(views(&h, a), 3);
    let order: Vec<u64> = h.app.popular_wikis(now_ts()).iter().map(|w| w.0).collect();
    assert_eq!(order, vec![a, b]);
    assert!(h.has("Popular Wiki Handbook") && h.has("Popular Wiki Onboarding"));
    // the card opens a wiki
    h.click("Popular Wiki Onboarding");
    assert_eq!(h.app.doc_open, Some(b));
    // the Wikis tab lists only wikis
    h.open_docs_list();
    h.click("Wikis");
    assert!(h.has("Doc Handbook") && h.has("Doc Onboarding"));
    assert!(!h.has("Doc Project brief"));
    // unmarking takes it out (and writes false, never back to missing)
    h.right_click("Doc Onboarding", 60.0);
    h.click("Unmark as Wiki");
    let d = h.app.data.docs.iter().find(|d| d.id == b).unwrap();
    assert_eq!(d.wiki, Some(false));
    h.settle();
    assert!(!h.has("Doc Onboarding") && !h.has("Popular Wiki Onboarding"));
}

#[test]
fn doc_views_and_wiki_flag_round_trip_and_old_docs_still_load() {
    let old: Doc =
        serde_json::from_str(r#"{"id":5,"title":"T","body":"","created":0,"updated":0}"#).unwrap();
    assert!(!old.is_wiki() && old.views.is_empty());
    let out = serde_json::to_value(&old).unwrap();
    assert!(out.get("wiki").is_none() && out.get("views").is_none());
    let mut a = App::build(None, Some(Data::seed()));
    let id = a.data.docs[0].id;
    a.count_doc_view(id, 1000);
    a.count_doc_view(id, 1000 + DOC_VIEW_DAYS * 86_400 + 5);
    let d = a.data.docs[0].clone();
    // the first open fell out of the 30 days
    assert_eq!(d.views.len(), 1);
    assert_eq!(d.views[0].at.len(), 1);
    let back: Doc = serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
    assert_eq!(back.views, d.views);
    // a duplicate starts unread, so no view entry id is used twice
    let copy = a.duplicate_doc(id).unwrap();
    assert!(a
        .data
        .docs
        .iter()
        .find(|d| d.id == copy)
        .unwrap()
        .views
        .is_empty());
}

#[test]
fn doc_copy_link_opens_the_doc_from_a_comment_chat_or_search() {
    let mut h = Ui1::new();
    let id = doc_id(&h, "Project brief");
    let url = format!("workboard://doc/{}", id);
    assert_eq!(
        crate::app::nav::parse_link(&url),
        Some(crate::app::nav::Link::Doc(id))
    );
    // comments read links with the same parser
    assert_eq!(
        crate::app::comments::parse_link(&format!("{}, see", url)).map(|x| x.0),
        Some(crate::app::nav::Link::Doc(id))
    );
    assert!(h.app.open_link(&url));
    h.settle();
    assert_eq!((h.app.section, h.app.doc_open), (Section::Docs, Some(id)));
    // a doc in someone else's Personal space is not opened
    h.app.section = Section::Home;
    h.app.doc_open = None;
    let sid = h.app.data.spaces[0].id;
    h.app.data.spaces[0].private_owner = Some(h.app.data.me + 1000);
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .space = Some(sid);
    assert!(!h.app.open_link(&url));
    assert_eq!(h.app.doc_open, None);
    // search finds the doc from its pasted link
    h.app.data.spaces[0].private_owner = None;
    let rows = h.app.search_rows(&url, 0);
    assert!(rows
        .iter()
        .any(|r| matches!(r.4, crate::app::more::Hit::Doc(x) if x == id)));
}

#[test]
fn docs_hub_leaves_out_docs_in_archived_or_other_peoples_spaces() {
    let mut h = Ui1::new();
    let id = doc_id(&h, "Project brief");
    h.open_docs_list();
    assert!(h.has("Doc Project brief"));
    let sid = h.app.data.spaces[0].id;
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == id)
        .unwrap()
        .space = Some(sid);
    h.app.data.spaces[0].archived = true;
    h.settle();
    assert!(!h.has("Doc Project brief"), "archived space");
    h.app.data.spaces[0].archived = false;
    h.app.data.spaces[0].private_owner = Some(h.app.data.me + 1000);
    h.settle();
    assert!(!h.has("Doc Project brief"), "someone else's Personal space");
    h.app.data.spaces[0].private_owner = None;
    h.settle();
    assert!(h.has("Doc Project brief"));
}

#[test]
fn sidebar_width_is_shared_by_chat_docs_and_dashboards() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Docs");
    let before = h.rect("Create in Docs");
    // the panel's edge sits just right of the Create button (8 px margin)
    let edge = pos2(before.right() + 8.0, before.center().y + 200.0);
    h.drag_by(edge, edge + vec2(100.0, 0.0));
    let after = h.rect("Create in Docs");
    assert!(
        (after.right() - before.right() - 100.0).abs() < 4.0,
        "the sidebar got wider: {:?} -> {:?}",
        before,
        after
    );
    h.click("Rail More");
    h.click("Dashboards");
    assert_eq!(h.app.section, Section::Dashboards);
    let dash = h.rect("Create in Dashboards");
    assert!(
        (dash.right() - after.right()).abs() < 1.0,
        "Dashboards uses the same width: {:?} vs {:?}",
        dash,
        after
    );
    // narrowing the Chat sidebar narrows the hubs' sidebar too
    h.app.section = Section::Chat;
    h.settle();
    let edge = pos2(after.right() + 8.0, after.center().y + 200.0);
    h.drag_by(edge, edge - vec2(150.0, 0.0));
    h.click("Rail More");
    h.click("Docs");
    let docs = h.rect("Create in Docs");
    assert!(
        (after.right() - docs.right() - 150.0).abs() < 4.0,
        "Docs follows the width set in Chat: {:?} -> {:?}",
        after,
        docs
    );
}

#[test]
fn meetings_list_can_be_dragged_wider() {
    let mut h = Ui1::new();
    h.app.section = Section::Meetings;
    h.settle();
    let before = h.rect("+ Meeting");
    let edge = pos2(before.right() + 10.0, before.center().y + 200.0);
    h.drag_by(edge, edge + vec2(80.0, 0.0));
    let after = h.rect("+ Meeting");
    assert!(
        (after.right() - before.right() - 80.0).abs() < 4.0,
        "the list got wider: {:?} -> {:?}",
        before,
        after
    );
}
