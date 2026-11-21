// Behaviour tests for filters, sorting, grouping, view settings and search.

use super::*;

#[test]
fn filters_search_me_mode_and_assignee() {
    let mut a = app();
    a.loc = Location::Everything;
    a.prefs.show_closed = true;
    a.search = "homepage".into();
    let r: Vec<String> = a.filtered().iter().map(|t| t.name.clone()).collect();
    assert!(!r.is_empty() && r.iter().all(|n| n.to_lowercase().contains("homepage")));
    a.search.clear();
    a.me_mode = true;
    assert!(a
        .filtered()
        .iter()
        .all(|t| t.assignees.contains(&a.data.me)));
    a.me_mode = false;
    a.assignee_filter = Some(3);
    assert!(a.filtered().iter().all(|t| t.assignees.contains(&3)));
}

#[test]
fn today_and_overdue_filter() {
    let mut a = app();
    a.loc = Location::TodayOverdue;
    let r = a.filtered();
    assert!(!r.is_empty());
    assert!(r
        .iter()
        .all(|t| t.due.map(|d| d <= today()).unwrap_or(false) && t.assignees.contains(&a.data.me)));
}

#[test]
fn due_date_groups_follow_the_rule() {
    let td = today();
    assert_eq!(due_group_key(None, false), "null");
    assert_eq!(due_group_key(Some(td), true), "done");
    assert_eq!(due_group_key(Some(td), false), "0");
    assert_eq!(due_group_key(Some(td - Duration::days(30)), false), "-1");
    assert_eq!(due_group_key(Some(td + Duration::days(3)), false), "3");
    assert_eq!(due_group_key(Some(td + Duration::days(40)), false), "+7");
}

#[test]
fn archived_tasks_leave_views_until_the_archived_filter() {
    let mut a = app();
    let list = list_ids(&a)[0];
    a.loc = Location::List(list);
    let id = add_task(&mut a, list, "Old work");
    a.set_archived(&[id], true);
    assert!(!a.filtered().iter().any(|t| t.id == id));
    a.prefs.filter.archived = true;
    assert!(a.filtered().iter().any(|t| t.id == id));
    a.prefs.filter.archived = false;
    a.set_archived(&[id], false);
    assert!(a.filtered().iter().any(|t| t.id == id));
}

#[test]
fn filter_conditions_status_priority_due_and_sort_direction() {
    let mut a = app();
    a.loc = Location::Everything;
    let all = a.filtered().len();
    a.prefs.filter.priorities = vec![1];
    assert!(a.filtered().iter().all(|t| t.priority == 1));
    a.prefs.filter = Default::default();
    a.prefs.filter.statuses = vec!["in progress".into()];
    assert!(!a.filtered().is_empty());
    assert!(a.filtered().iter().all(|t| t.status == "in progress"));
    a.prefs.filter = Default::default();
    a.prefs.filter.due = 1;
    assert!(a
        .filtered()
        .iter()
        .all(|t| t.due.map_or(false, |d| d < today())));
    a.prefs.filter.due = 5;
    assert!(a.filtered().iter().all(|t| t.due.is_none()));
    a.prefs.filter = Default::default();
    assert_eq!(a.filtered().len(), all);
    a.prefs.sort_by = SortBy::Name;
    let asc: Vec<u64> = a.filtered().iter().map(|t| t.id).collect();
    a.prefs.sort_desc = true;
    let mut desc: Vec<u64> = a.filtered().iter().map(|t| t.id).collect();
    desc.reverse();
    assert_eq!(asc, desc);
}

fn names(a: &App) -> Vec<String> {
    a.filtered().iter().map(|t| t.name.clone()).collect()
}

#[test]
fn manual_order_moves_a_task_among_its_siblings() {
    let mut a = app();
    let web = a
        .data
        .tasks
        .iter()
        .find(|t| t.name == "Write copy for landing page")
        .unwrap()
        .list_id;
    a.loc = Location::List(web);
    a.prefs.show_closed = true;
    let before = names(&a);
    let (drag, target) = (
        tid(&a, "Collect feedback from beta users"),
        tid(&a, "Write copy for landing page"),
    );
    a.reorder_task(drag, target, false);
    let after = names(&a);
    let pos = |v: &[String], n: &str| v.iter().position(|x| x == n).unwrap();
    assert!(
        pos(&after, "Collect feedback from beta users") + 1
            == pos(&after, "Write copy for landing page"),
        "{:?}",
        after
    );
    assert_eq!(after.len(), before.len());
    // dropped after a task
    a.reorder_task(drag, tid(&a, "Choose color palette"), true);
    let v = names(&a);
    assert!(pos(&v, "Collect feedback from beta users") > pos(&v, "Choose color palette"));
    // subtasks move only under their own parent
    let (s1, s2) = (
        tid(&a, "Wireframe hero section"),
        tid(&a, "Pick hero image"),
    );
    let parent = a.task(s1).unwrap().parent.unwrap();
    a.reorder_task(s2, s1, false);
    let subs: Vec<u64> = a.subtasks(parent).iter().map(|t| t.id).collect();
    assert_eq!(subs, vec![s2, s1]);
    let top = tid(&a, "Write copy for landing page");
    let order = a.task(s1).unwrap().order;
    a.reorder_task(s1, top, false);
    assert_eq!(a.task(s1).unwrap().parent, Some(parent));
    assert_eq!(
        a.task(s1).unwrap().order,
        order,
        "a subtask is not moved among top-level tasks"
    );
}

#[test]
fn new_sorts_and_groups() {
    let mut a = app();
    a.loc = Location::Everything;
    a.prefs.show_closed = true;
    a.prefs.sort_by = SortBy::Status;
    let sts: Vec<StatusKind> = a
        .filtered()
        .iter()
        .map(|t| a.data.status_of(t).kind)
        .collect();
    let rank = |k: &StatusKind| match k {
        StatusKind::Open => 0,
        StatusKind::Custom => 1,
        StatusKind::Done => 2,
        StatusKind::Closed => 3,
    };
    assert!(
        sts.windows(2).all(|w| rank(&w[0]) <= rank(&w[1])),
        "open statuses first"
    );
    a.prefs.sort_by = SortBy::StartDate;
    let v = a.filtered();
    let firsts: Vec<bool> = v.iter().map(|t| t.start.is_none()).collect();
    assert!(
        firsts.windows(2).all(|w| !w[0] || w[1]),
        "no start date last"
    );
    a.prefs.sort_by = SortBy::Assignee;
    let v = a.filtered();
    assert!(v.last().unwrap().assignees.is_empty() || v.iter().all(|t| !t.assignees.is_empty()));
    let id = tid(&a, "Newsletter draft");
    a.data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .estimate_min = Some(5);
    a.prefs.sort_by = SortBy::Estimate;
    assert_eq!(a.filtered()[0].id, id);
    a.data
        .tasks
        .iter_mut()
        .find(|t| t.id == id)
        .unwrap()
        .updated = now_ts() + 100;
    a.prefs.sort_by = SortBy::Updated;
    assert_eq!(a.filtered()[0].id, id, "most recently updated first");
    // Group: Tags (a task under each of its tags) and Group: List
    a.prefs.sort_by = SortBy::Manual;
    a.prefs.group_by = GroupBy::Tags;
    let tasks = a.filtered();
    let groups = a.groups(&tasks, false);
    let design = groups
        .iter()
        .find(|g| g.key == "t:design")
        .expect("a design group");
    assert_eq!(design.tasks.len(), 2);
    assert!(groups.iter().any(|g| g.label == "NO TAGS"));
    a.prefs.group_by = GroupBy::List;
    let tasks = a.filtered();
    let groups = a.groups(&tasks, false);
    assert!(groups.iter().any(|g| g.label == "MARKETING"));
    // adding from a tag group gives the task that tag; from a list group, that list
    a.prefs.group_by = GroupBy::Tags;
    let tasks = a.filtered();
    let g = a
        .groups(&tasks, false)
        .into_iter()
        .find(|g| g.key == "t:bug")
        .unwrap();
    a.add_from_group(&g, "Tagged".into(), false);
    a.apply_actions();
    assert_eq!(
        a.task(tid(&a, "Tagged")).unwrap().tags,
        vec!["bug".to_string()]
    );
    a.prefs.group_by = GroupBy::List;
    let tasks = a.filtered();
    let g = a
        .groups(&tasks, false)
        .into_iter()
        .find(|g| g.label == "MARKETING")
        .unwrap();
    a.add_from_group(&g, "In marketing".into(), false);
    a.apply_actions();
    assert_eq!(
        a.data
            .list_name(a.task(tid(&a, "In marketing")).unwrap().list_id),
        "Marketing"
    );
}

#[test]
fn new_filter_fields() {
    use parity::{FilterField, FilterRow};
    let mut a = app();
    a.loc = Location::Everything;
    let row = |f: FilterField, v: &str| FilterRow {
        field: Some(f),
        not: false,
        values: vec![v.to_string()],
    };
    a.prefs.filter.rows = vec![row(FilterField::HasSubtasks, "yes")];
    assert_eq!(names(&a), vec!["Design new homepage layout".to_string()]);
    a.prefs.filter.rows = vec![row(FilterField::StartDate, "5")];
    assert!(!a.filtered().is_empty() && a.filtered().iter().all(|t| t.start.is_none()));
    a.prefs.filter.rows = vec![row(FilterField::CreatedBy, "2")];
    assert!(
        a.filtered().is_empty(),
        "the seed tasks were all made by Me"
    );
    a.prefs.filter.rows = vec![row(FilterField::CreatedBy, "1")];
    assert!(!a.filtered().is_empty());
    a.prefs.filter.rows = vec![row(FilterField::Created, "1")];
    assert!(a.filtered().is_empty(), "nothing was made today");
    a.prefs.filter.rows = vec![row(FilterField::Created, "3")];
    assert!(!a.filtered().is_empty());
    a.prefs.filter.rows = vec![row(FilterField::Updated, "1")];
    assert!(!a.filtered().is_empty());
}

#[test]
fn week_start_follows_the_labs_option_and_keeps_unknown_labs() {
    let mut d = Data::seed();
    d.extra.labs = vec!["from_a_newer_app".into()];
    let day = |m, n| NaiveDate::from_ymd_opt(2026, m, n).unwrap();
    // Sunday 22 Nov 2026, Wednesday 25 Nov 2026
    assert_eq!(d.week_start(day(11, 22)), day(11, 22));
    assert_eq!(d.week_start(day(11, 25)), day(11, 22));
    d.set_lab(Lab::WeekMonday, true);
    assert!(d.lab(Lab::WeekMonday));
    assert_eq!(
        d.week_start(day(11, 22)),
        day(11, 16),
        "a Sunday ends the week"
    );
    assert_eq!(d.week_start(day(11, 25)), day(11, 23));
    assert_eq!(sunday_week_start(day(11, 25)), day(11, 22), "fixed Sunday");
    d.set_lab(Lab::WeekMonday, false);
    assert_eq!(d.extra.labs, vec!["from_a_newer_app".to_string()]);
}

#[test]
fn start_and_due_filters_agree_on_this_week() {
    use parity::{FilterField, FilterRow};
    let mut a = app();
    let list = list_ids(&a)[0];
    let id = add_task(&mut a, list, "Dated");
    let row = |f: FilterField, v: &str| FilterRow {
        field: Some(f),
        not: false,
        values: vec![v.to_string()],
    };
    let td = crate::model::today();
    for monday in [false, true] {
        a.data.extra.labs.retain(|l| l != "week_monday");
        if monday {
            a.data.extra.labs.push("week_monday".into());
        }
        for back in -7..14 {
            let d = td + chrono::Duration::days(back);
            let t = a.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
            t.start = Some(d);
            t.due = Some(d);
            let t = a.task(id).unwrap();
            for which in ["3", "4"] {
                assert_eq!(
                    row(FilterField::StartDate, which).matches(t, &a.data),
                    row(FilterField::DueDate, which).matches(t, &a.data),
                    "{d} with week_monday={monday}, choice {which}"
                );
            }
        }
    }
}

/// Four tasks with known priority and tags, and which of them a filter keeps.
fn filter_fixture() -> (App, impl Fn(&App, &parity::TaskFilter) -> Vec<&'static str>) {
    let mut a = app();
    let list = list_ids(&a)[0];
    for (name, priority, tag) in [
        ("A", 1, true),
        ("B", 1, false),
        ("C", 3, true),
        ("D", 3, false),
    ] {
        let id = add_task(&mut a, list, name);
        let t = a.data.tasks.iter_mut().find(|t| t.id == id).unwrap();
        t.priority = priority;
        t.tags = if tag { vec!["x".into()] } else { vec![] };
    }
    let hits = |a: &App, f: &parity::TaskFilter| {
        ["A", "B", "C", "D"]
            .into_iter()
            .filter(|n| f.matches(a.task(tid(a, n)).unwrap(), &a.data))
            .collect()
    };
    (a, hits)
}

fn urgent() -> parity::FilterRow {
    parity::FilterRow {
        field: Some(parity::FilterField::Priority),
        not: false,
        values: vec!["1".into()],
    }
}

fn tagged_x() -> parity::FilterRow {
    parity::FilterRow {
        field: Some(parity::FilterField::Tags),
        not: false,
        values: vec!["x".into()],
    }
}

#[test]
fn filter_rows_combine_with_and_or_or() {
    use parity::{Join, TaskFilter};
    let (a, hits) = filter_fixture();
    let mut f = TaskFilter {
        rows: vec![urgent(), tagged_x()],
        ..Default::default()
    };
    assert_eq!(hits(&a, &f), vec!["A"], "And: both");
    f.join = Join::Or;
    assert_eq!(hits(&a, &f), vec!["A", "B", "C"], "Or: either");
    // is not
    f.rows[1].not = true;
    assert_eq!(hits(&a, &f), vec!["A", "B", "D"], "urgent or not tagged");
    f.join = Join::And;
    assert_eq!(hits(&a, &f), vec!["B"], "urgent and not tagged");
    // an unfinished row doesn't count, with Or as well as And
    f.rows = vec![urgent(), parity::FilterRow::default()];
    assert_eq!(hits(&a, &f), vec!["A", "B"]);
    f.join = Join::Or;
    assert_eq!(
        hits(&a, &f),
        vec!["A", "B"],
        "an empty row doesn't match everything"
    );
    assert_eq!(f.count(), 1);
}

#[test]
fn filter_groups_nest_one_level() {
    use parity::{FilterGroup, Join, TaskFilter};
    let (a, hits) = filter_fixture();
    // tagged x, and (urgent or not tagged): only A
    let mut f = TaskFilter {
        rows: vec![tagged_x()],
        groups: vec![FilterGroup {
            join: Join::Or,
            rows: vec![
                urgent(),
                parity::FilterRow {
                    not: true,
                    ..tagged_x()
                },
            ],
        }],
        ..Default::default()
    };
    assert_eq!(hits(&a, &f), vec!["A"]);
    assert_eq!(f.count(), 3, "every row counts, the ones in groups too");
    // top level Or: tagged x, or (urgent or not tagged) = everything
    f.join = Join::Or;
    assert_eq!(hits(&a, &f), vec!["A", "B", "C", "D"]);
    // a group with And inside, top level Or: (urgent and tagged) or (not urgent and not tagged)
    f.rows.clear();
    f.groups = vec![
        FilterGroup {
            join: Join::And,
            rows: vec![urgent(), tagged_x()],
        },
        FilterGroup {
            join: Join::And,
            rows: vec![
                parity::FilterRow {
                    not: true,
                    ..urgent()
                },
                parity::FilterRow {
                    not: true,
                    ..tagged_x()
                },
            ],
        },
    ];
    assert_eq!(hits(&a, &f), vec!["A", "D"]);
    f.join = Join::And;
    assert!(hits(&a, &f).is_empty(), "nothing is both");
}

#[test]
fn empty_filter_groups_are_left_out() {
    use parity::{FilterGroup, Join, TaskFilter};
    let (a, hits) = filter_fixture();
    // only empty groups: nothing is filtered, and the badge shows nothing
    let mut f = TaskFilter {
        join: Join::Or,
        groups: vec![
            FilterGroup::default(),
            FilterGroup {
                join: Join::Or,
                rows: vec![parity::FilterRow::default()],
            },
        ],
        ..Default::default()
    };
    assert_eq!(hits(&a, &f), vec!["A", "B", "C", "D"]);
    assert_eq!(f.count(), 0);
    // with Or at the top, an empty group doesn't let everything through
    f.rows = vec![urgent()];
    assert_eq!(hits(&a, &f), vec!["A", "B"]);
    f.join = Join::And;
    assert_eq!(hits(&a, &f), vec!["A", "B"]);
    assert_eq!(f.count(), 1);
}

#[test]
fn view_settings_are_written_with_the_names_they_always_had() {
    use crate::app::listview::ViewPrefs;
    let mut p = ViewPrefs::new();
    p.hidden_cols = ["Tags", "Assignee"].map(String::from).into();
    p.filter.rows = vec![urgent()];
    p.filter.statuses = vec!["open".into()];
    let v = serde_json::to_value(&p).unwrap();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "archived",
            "filter_rows",
            "group_by",
            "hidden_cols",
            "show_closed",
            "sort_by",
            "sort_desc",
            "subtasks_open"
        ]
    );
    assert_eq!(v["hidden_cols"], serde_json::json!(["Assignee", "Tags"]));
    let back: ViewPrefs = serde_json::from_value(v).unwrap();
    assert_eq!(back.filter.rows, p.filter.rows);
    assert!(
        back.filter.statuses.is_empty(),
        "the fixed fields aren't saved"
    );
}

#[test]
fn view_settings_from_before_filter_groups_still_load() {
    use parity::{FilterGroup, Join};
    let p: crate::app::listview::ViewPrefs = serde_json::from_value(serde_json::json!({
        "group_by": "Status", "sort_by": "Manual",
        "filter_rows": [{"field": "Priority", "not": false, "values": ["1"]}]
    }))
    .unwrap();
    assert_eq!(p.filter.rows.len(), 1);
    assert!(p.filter.groups.is_empty() && p.filter.join == Join::And);
    // groups are remembered per location like the rows
    let mut a = app();
    a.prefs.filter.join = Join::Or;
    a.prefs.filter.groups = vec![FilterGroup {
        join: Join::Or,
        rows: vec![urgent(), tagged_x()],
    }];
    let v = serde_json::to_value(a.prefs.clone()).unwrap();
    let back: crate::app::listview::ViewPrefs = serde_json::from_value(v).unwrap();
    assert_eq!(back.filter.join, Join::Or);
    assert_eq!(back.filter.groups, a.prefs.filter.groups);
}

#[test]
fn saved_filters_keep_the_nested_shape_and_old_files_still_load() {
    use parity::{FilterGroup, Join};
    let mut a = app();
    a.prefs.filter.rows = vec![urgent()];
    a.prefs.filter.join = Join::Or;
    a.prefs.filter.groups = vec![FilterGroup {
        join: Join::Or,
        rows: vec![tagged_x()],
    }];
    a.save_filter("  Hot  ");
    assert_eq!(a.data.extra.saved_filters.len(), 1);
    let saved = a.data.extra.saved_filters[0].clone();
    assert_eq!(saved.name, "Hot");
    assert!(saved.is_shown(&a.prefs.filter));
    // saving under the same name updates it
    a.prefs.filter.join = Join::And;
    a.save_filter("Hot");
    assert_eq!(a.data.extra.saved_filters.len(), 1);
    assert_eq!(a.data.extra.saved_filters[0].id, saved.id);
    assert_eq!(a.data.extra.saved_filters[0].join, Join::And);
    // written to the workspace file and read back with the groups
    let v = serde_json::to_value(&a.data).unwrap();
    let back: Data = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(back.extra.saved_filters, a.data.extra.saved_filters);
    assert_eq!(
        v["extra"]["saved_filters"][0]["groups"][0]["join"],
        serde_json::json!("Or")
    );
    // bringing it back after the builder was cleared
    a.prefs.filter = Default::default();
    back.extra.saved_filters[0].apply_to(&mut a.prefs.filter);
    assert_eq!(a.prefs.filter.groups.len(), 1);
    assert_eq!(a.prefs.filter.count(), 2);
    // a file from before saved filters and groups still loads
    let mut old = v.clone();
    old["extra"]
        .as_object_mut()
        .unwrap()
        .remove("saved_filters");
    let d: Data = serde_json::from_value(old).unwrap();
    assert!(d.extra.saved_filters.is_empty());
    // nothing about the saved filters reaches the copy teammates get
    assert!(crate::sync::shareable(&v)["extra"]
        .get("saved_filters")
        .is_none());
}

#[test]
fn search_finds_subtasks() {
    let mut a = app();
    a.loc = Location::Everything;
    a.prefs.show_closed = true;
    a.search = "hero".into();
    let v = names(&a);
    assert!(
        v.contains(&"Pick hero image".to_string())
            && v.contains(&"Wireframe hero section".to_string()),
        "{:?}",
        v
    );
    a.search.clear();
    assert!(
        !names(&a).contains(&"Pick hero image".to_string()),
        "subtasks stay under their parent otherwise"
    );
}

#[test]
fn view_settings_are_kept_per_location() {
    let mut a = app();
    let lists = list_ids(&a);
    let (l1, l2) = (lists[0], lists[1]);
    a.loc = Location::List(l1);
    a.sync_view_prefs();
    a.prefs.group_by = GroupBy::Priority;
    a.prefs.sort_by = SortBy::Name;
    a.sync_view_prefs();
    a.loc = Location::List(l2);
    a.sync_view_prefs();
    assert!(
        a.prefs.group_by == GroupBy::Priority,
        "a list never changed keeps what was showing"
    );
    a.prefs.group_by = GroupBy::Assignee;
    a.prefs.show_closed = true;
    a.sync_view_prefs();
    a.loc = Location::List(l1);
    a.sync_view_prefs();
    assert!(
        a.prefs.group_by == GroupBy::Priority
            && a.prefs.sort_by == SortBy::Name
            && !a.prefs.show_closed
    );
    a.loc = Location::List(l2);
    a.sync_view_prefs();
    assert!(a.prefs.group_by == GroupBy::Assignee && a.prefs.show_closed);
    // and they survive a round trip through the saved file format
    let s = serde_json::to_string(&a.view_prefs).unwrap();
    let back: HashMap<String, listview::ViewPrefs> = serde_json::from_str(&s).unwrap();
    assert_eq!(back, a.view_prefs);
}

#[test]
fn search_reads_copied_links_and_finds_apps_and_people() {
    use crate::app::more::Hit;
    let mut a = app();
    let tid = a.data.tasks[2].id;
    let rows = a.search_rows(&format!("workboard://task/{}", tid), 0);
    assert_eq!(rows[0].4, Hit::Task(tid));
    let lid = a.data.first_list().unwrap();
    assert_eq!(
        a.search_rows(&format!("workboard://list/{}", lid), 0)[0].4,
        Hit::Go(Location::List(lid))
    );
    let sid = a.data.spaces[1].id;
    assert_eq!(
        a.search_rows(&format!("workboard://space/{}/", sid), 0)[0].4,
        Hit::Go(Location::Space(sid))
    );
    a.new_dashboard("Quarterly numbers");
    let did = a.dash_open.unwrap();
    assert!(a
        .search_rows("quarterly", 0)
        .iter()
        .any(|r| r.4 == Hit::Dashboard(did)));
    assert!(a
        .search_rows("alex", 0)
        .iter()
        .any(|r| r.4 == Hit::Person(2)));
    // a message found by its text opens the channel at it
    assert!(a
        .search_rows("newsletter draft is due", 5)
        .iter()
        .any(|r| matches!(r.4, Hit::Message(..))));
}

#[test]
fn search_leaves_out_archived_lists_and_other_peoples_private_docs() {
    let mut a = app();
    let me = a.data.me;
    let lists = list_ids(&a);
    add_task(&mut a, lists[0], "Zebra crossing");
    let found = |a: &App, q: &str| {
        a.search_rows(q, 0)
            .iter()
            .filter(|r| r.0 != "Recent searches")
            .map(|r| r.2.clone())
            .collect::<Vec<_>>()
    };
    assert!(found(&a, "zebra").iter().any(|t| t == "Zebra crossing"));
    a.data.list_item_mut(lists[0]).unwrap().archived = true;
    assert!(
        !found(&a, "zebra").iter().any(|t| t == "Zebra crossing"),
        "a task in an archived list stays out"
    );
    let now = now_ts();
    let space = a
        .data
        .spaces
        .iter()
        .find(|s| s.private_owner.is_none() && !s.archived)
        .map(|s| s.id);
    for (id, owner, private, title) in [
        (9101, me + 100, true, "Quokka secret"),
        (9102, me, true, "Quokka mine"),
        (9103, me + 100, false, "Quokka shared"),
    ] {
        a.data.docs.push(Doc {
            id,
            space,
            owner,
            private,
            archived: false,
            meeting_note: false,
            wiki: None,
            views: vec![],
            parent: None,
            title: title.into(),
            body: String::new(),
            created: now,
            updated: now,
            tags: vec![],
        });
    }
    let docs = found(&a, "quokka");
    assert!(!docs.iter().any(|t| t == "Quokka secret"));
    assert!(docs.iter().any(|t| t == "Quokka mine") && docs.iter().any(|t| t == "Quokka shared"));
    let d = a.data.docs.iter().find(|d| d.id == 9101).unwrap();
    assert!(!a.doc_shown(d));
}

#[test]
fn search_leaves_out_other_peoples_private_dashboards_and_personal_forms() {
    use crate::app::more::Hit;
    let mut a = app();
    let me = a.data.me;
    let lists = list_ids(&a);
    let hits =
        |a: &App, q: &str| -> Vec<Hit> { a.search_rows(q, 6).into_iter().map(|r| r.4).collect() };
    a.new_dashboard("Salary plans");
    let did = a.dash_open.unwrap();
    fn dash(a: &mut App, did: u64) -> &mut Dashboard {
        a.data
            .extra
            .dashboards
            .iter_mut()
            .find(|d| d.id == did)
            .unwrap()
    }
    dash(&mut a, did).private = true;
    assert!(
        hits(&a, "salary").contains(&Hit::Dashboard(did)),
        "my own private one shows"
    );
    dash(&mut a, did).owner = me + 100;
    assert!(
        !hits(&a, "salary").contains(&Hit::Dashboard(did)),
        "someone else's stays out"
    );
    // older dashboards without a maker are everyone's
    dash(&mut a, did).owner = 0;
    assert!(hits(&a, "salary").contains(&Hit::Dashboard(did)));
    // a public dashboard in someone else's Personal space stays out too
    let sid = a.data.space_of_list(lists[0]).unwrap().id;
    let d = dash(&mut a, did);
    d.private = false;
    d.owner = me + 100;
    d.space = Some(sid);
    a.data.extra.forms.push(Form {
        id: 777_001,
        name: "Secret survey".into(),
        list_id: lists[0],
        description: String::new(),
        fields: vec![],
        owner: me + 100,
        ..Default::default()
    });
    assert!(hits(&a, "salary").contains(&Hit::Dashboard(did)));
    assert!(hits(&a, "secret survey").contains(&Hit::Form(777_001)));
    a.data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .private_owner = Some(me + 100);
    assert!(!hits(&a, "salary").contains(&Hit::Dashboard(did)));
    assert!(!hits(&a, "secret survey").contains(&Hit::Form(777_001)));
    let d = a
        .data
        .extra
        .dashboards
        .iter()
        .find(|d| d.id == did)
        .unwrap()
        .clone();
    let f = a.data.extra.forms.last().unwrap().clone();
    assert!(!a.dash_shown(&d) && !a.form_shown(&f));
    // my own Personal space: both show
    a.data
        .spaces
        .iter_mut()
        .find(|s| s.id == sid)
        .unwrap()
        .private_owner = Some(me);
    assert!(hits(&a, "salary").contains(&Hit::Dashboard(did)));
    assert!(hits(&a, "secret survey").contains(&Hit::Form(777_001)));
}
