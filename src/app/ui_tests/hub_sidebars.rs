// Click tests for the hub sidebars (hubs/side.rs): Forms, Docs, Dashboards,
// Whiteboards and Teams all lay theirs out as Forms' (the name and «, a
// search, the filters with counts, a line, then a heading with + over every
// item), and the Meetings list does the same without the «. None of them
// has a filled "+ Create" button by its name.

use super::*;

/// Each hub from the rail's More, with the names its sidebar's search and +
/// carry.
const HUBS: [(&str, Section, &str, &str); 5] = [
    ("Forms", Section::Forms, "Find a form", "Add a form"),
    ("Docs", Section::Docs, "Find a doc", "Add a doc"),
    (
        "Dashboards",
        Section::Dashboards,
        "Find a dashboard",
        "Add a dashboard",
    ),
    (
        "Whiteboards",
        Section::Whiteboards,
        "Find a whiteboard",
        "Add a whiteboard",
    ),
    ("Teams", Section::Teams, "Find a team", "Add a team"),
];

fn open_hub(h: &mut Ui1, label: &str, section: Section) {
    if section == Section::Teams {
        // Teams sits in the rail itself
        h.click("Rail Teams");
    } else {
        h.click("Rail More");
        h.click(label);
    }
    assert_eq!(h.app.section, section);
}

/// Every hub sidebar has the same top: the « at the same place, the search
/// under it, and a + at the end of the heading over the items, lined up
/// with the « — and no "+ Create" button.
#[test]
fn every_hub_sidebar_is_laid_out_as_the_forms_one() {
    let mut h = Ui1::new();
    let mut first: Option<(Rect, Rect)> = None;
    for (label, section, find, add) in HUBS {
        open_hub(&mut h, label, section);
        assert!(!h.has(&format!("Create in {}", label)), "{}", label);
        assert!(!h.has(&format!("New in {}", label)), "{}", label);
        let close = h.rect("Close sidebar");
        let search = h.rect(find);
        let plus = h.rect(add);
        assert!(search.top() > close.bottom(), "{}: search under «", label);
        assert!(
            plus.top() > search.bottom(),
            "{}: + under the search",
            label
        );
        assert!(
            (plus.right() - close.right()).abs() < 1.0,
            "{}: the + lines up with the «: {:?} {:?}",
            label,
            plus,
            close
        );
        let side = crate::widgets::CONTROL_H;
        assert!(
            (plus.width() - side).abs() < 0.5 && (plus.height() - side).abs() < 0.5,
            "{}: the + is the icon button {:?}",
            label,
            plus
        );
        match first {
            None => first = Some((close, search)),
            Some((c, s)) => {
                assert_eq!(close, c, "{}: « where Forms has it", label);
                // the field's text area, inside the same frame
                assert!(
                    (search.top() - s.top()).abs() < 0.5,
                    "{}: search where Forms has it",
                    label
                );
            }
        }
    }
}

/// Each heading's + makes a new item of its hub, and the new one is in the
/// sidebar's list at once.
#[test]
fn the_heading_plus_makes_a_new_item_in_each_hub() {
    let mut h = Ui1::new();
    open_hub(&mut h, "Docs", Section::Docs);
    let docs = h.app.data.docs.len();
    h.click("Add a doc");
    assert_eq!(h.app.data.docs.len(), docs + 1);
    let id = h.app.doc_open.expect("the new doc is open");
    let title = h
        .app
        .data
        .docs
        .iter()
        .find(|d| d.id == id)
        .unwrap()
        .title
        .clone();
    let shown = if title.is_empty() {
        "Untitled".to_string()
    } else {
        title
    };
    assert!(
        h.has(&format!("Open doc {}", shown)),
        "listed in the sidebar"
    );

    open_hub(&mut h, "Dashboards", Section::Dashboards);
    h.click("Add a dashboard");
    assert!(matches!(h.app.modal, Some(Modal::NewDashboard { .. })));
    h.app.modal = None;
    h.app.new_dashboard("Sales");
    h.app.dash_open = None;
    h.settle();
    assert!(h.has("Open dashboard Sales"));

    open_hub(&mut h, "Teams", Section::Teams);
    let teams = h.app.data.extra.teams.len();
    h.click("Add a team");
    assert_eq!(h.app.data.extra.teams.len(), teams + 1);
    assert_eq!(h.app.hub_filter, 9, "the new team's page is open");
    let name = h.app.data.extra.teams.last().unwrap().name.clone();
    assert!(h.has(&format!("Open team {}", name)));
}

/// The search narrows the sidebar's list, and says so when nothing has
/// that name.
#[test]
fn the_sidebar_search_narrows_the_list() {
    let mut h = Ui1::new();
    for name in ["Roadmap", "Hiring plan"] {
        h.app.new_doc(None);
        let id = h.app.doc_open.unwrap();
        h.app
            .data
            .docs
            .iter_mut()
            .find(|d| d.id == id)
            .unwrap()
            .title = name.into();
    }
    h.open_docs_list();
    assert!(h.has("Open doc Roadmap") && h.has("Open doc Hiring plan"));
    h.click("Find a doc");
    h.type_text("road");
    assert!(h.has("Open doc Roadmap"));
    assert!(!h.has("Open doc Hiring plan"), "narrowed by the search");
    h.click("Open doc Roadmap");
    assert!(h.app.doc_open.is_some());
    h.open_docs_list();
    h.click("Find a doc");
    h.type_text("zzz");
    assert!(!h.painted_text("No doc has that name").is_empty());
    // each hub keeps its own search
    open_hub(&mut h, "Whiteboards", Section::Whiteboards);
    assert!(h.painted_text("No whiteboard has that name").is_empty());
}

/// Favorites is a filter row with a count on Docs, Dashboards and
/// Whiteboards, as on Forms (it was a section with a big empty card), and
/// the page lists only the starred ones.
#[test]
fn favorites_is_a_filter_on_docs_dashboards_and_whiteboards() {
    let mut h = Ui1::new();
    let brief = h.app.data.docs[0].id;
    h.app.new_doc(None);
    let other = h.app.doc_open.unwrap();
    h.app
        .data
        .docs
        .iter_mut()
        .find(|d| d.id == other)
        .unwrap()
        .title = "Roadmap".into();
    h.open_docs_list();
    assert!(h.has("Doc Project brief") && h.has("Doc Roadmap"));
    h.app.data.extra.starred.push(format!("doc:{}", brief));
    h.settle();
    h.click("Favorites");
    assert_eq!(h.app.hub_filter, 7);
    assert_eq!(h.app.docs_count(7), 1);
    assert!(h.has("Doc Project brief"));
    assert!(!h.has("Doc Roadmap"), "only the starred doc on the page");

    h.app.new_dashboard("Sales");
    h.app.new_dashboard("Costs");
    let sales = h.app.data.extra.dashboards[0].id;
    h.app.data.extra.starred.push(format!("dash:{}", sales));
    h.app.dash_open = None;
    open_hub(&mut h, "Dashboards", Section::Dashboards);
    h.click("Favorites");
    assert_eq!(h.app.hub_filter, 2);
    assert!(h.has("Sales") && !h.has("Costs"));

    h.app.new_whiteboard();
    h.app.new_whiteboard();
    let (a, b) = {
        let boards = &h.app.data.extra.whiteboards;
        (boards[0].id, boards[1].id)
    };
    h.app.data.extra.starred.push(format!("wb:{}", a));
    h.app.wb_open = None;
    h.app.sweep.wb_new = None;
    open_hub(&mut h, "Whiteboards", Section::Whiteboards);
    h.click("Favorites");
    assert_eq!(h.app.hub_filter, 2);
    let on_page = |h: &Ui1, id: u64| h.has(&format!("Star wb:{}", id));
    assert!(on_page(&h, a) && !on_page(&h, b), "only the starred board");
}

/// Teams are made by the owner and admins on a server: a member sees no +
/// (it used to show a framed + that did nothing).
#[test]
fn a_member_gets_no_plus_on_the_teams_sidebar() {
    let mut h = Ui1::new();
    open_hub(&mut h, "Teams", Section::Teams);
    assert!(h.has("Add a team"));
    h.app.data.extra.shared = Some(crate::sync::SharedCfg {
        folder: String::new(),
        device: "dev1".into(),
        tag: 5,
        server: Some("127.0.0.1:1".into()),
    });
    h.app.sync_rt.net_status = Some(wb_net::Status::Connected);
    h.app.sync_rt.role = Some(wb_net::Role::Member);
    h.settle();
    assert!(!h.has("Add a team"));
    assert!(h.has("Find a team") && h.has("All Teams"));
}

/// The Meetings list has the same pieces without the «: its name, a search,
/// and a heading whose + makes a meeting (it had a filled "+ Meeting").
#[test]
fn the_meetings_list_is_laid_out_as_the_hub_sidebars() {
    let mut h = Ui1::new();
    h.click("Meetings");
    assert_eq!(h.app.section, Section::Meetings);
    assert!(!h.has("+ Meeting"));
    assert!(!h.painted_text("No meetings yet").is_empty());
    h.click("Add a meeting");
    let id = h.app.meeting_open.expect("a meeting was made and opened");
    assert!(h.has("Open meeting New meeting"));
    h.app
        .data
        .extra
        .meetings
        .iter_mut()
        .find(|m| m.id == id)
        .unwrap()
        .title = "Weekly sync".into();
    h.click("Add a meeting");
    h.click("Find a meeting");
    h.type_text("weekly");
    assert!(h.has("Open meeting Weekly sync"));
    assert!(!h.has("Open meeting New meeting"));
}
