// The hub pages (Timesheets, Dashboards, Teams, Planner, Forms, Settings)
// in every window size and both languages, with long names: nothing is cut
// mid-letter, drawn over something else or past the window's right edge.

use super::layout_tests::overlapping;
use super::*;
use crate::i18n::{set_lang, Lang};

pub(super) const SIZES: [(f32, f32); 5] = [
    (1440.0, 900.0),
    (960.0, 560.0),
    (700.0, 640.0),
    (460.0, 640.0),
    (360.0, 640.0),
];

pub(super) const LONG_NAME: &str =
    "A task with a very long name that keeps going well past the room its row has for it";

/// Runs `f` in each window size and language, English again afterwards.
pub(super) fn each_size(mut f: impl FnMut(f32, f32, &str)) {
    for (w, hh) in SIZES {
        for lang in [Lang::En, Lang::Ko] {
            set_lang(lang);
            f(w, hh, &format!("{}x{} {:?}", w, hh, lang));
        }
    }
    set_lang(Lang::En);
}

/// The page under the top bar, right of the rail when there is one.
pub(super) fn page(h: &Ui1) -> Rect {
    let left = if h.size.x < 600.0 { 0.0 } else { 60.0 };
    Rect::from_min_max(pos2(left, 40.0), h.size.to_pos2())
}

/// Nothing in `area` is cut, drawn over another text or past the edge.
pub(super) fn assert_fits(h: &Ui1, area: Rect, at: &str) {
    let cut = h.cut_texts(area);
    assert!(cut.is_empty(), "{}: cut {:?}", at, cut);
    let over = overlapping(&h.texts_in(area, Seen::Visible));
    assert!(over.is_empty(), "{}: overlapping {:?}", at, over);
    let past = h.past_right_edge();
    assert!(past.is_empty(), "{}: past the right edge {:?}", at, past);
}

/// The Timesheet's time entries: a long task name is shortened with "…"
/// before its time, Edit and ×, which stay in the window; a narrow card
/// puts the date over the task.
#[test]
fn timesheet_entries_shorten_long_task_names() {
    each_size(|w, hh, at| {
        let mut h = Ui1::sized(w, hh);
        let mut x = task_named(&h, "Design new homepage layout").clone();
        x.id = 31_000;
        x.name = LONG_NAME.into();
        x.time_entries.clear();
        h.app.data.tasks.push(x);
        h.app.set_day_time(31_000, today(), 1.5);
        h.app.section = Section::Timesheets;
        h.app.sheet_entries = true;
        h.settle();
        assert_fits(&h, page(&h), at);
        let edit = h.rect(&format!("Edit entry {}", LONG_NAME));
        assert!(edit.right() <= w, "{}: {:?}", at, edit);
        assert!(!h.painted_text(&fmt_duration(5400)).is_empty(), "{}", at);
    });
}

const LONG_PERSON: &str = "Bartholomew Alexander Fitzgerald-Montgomery the Third";

/// A teammate with a long name, given `task` (cloned from a seed task).
fn long_person_with_task(h: &mut Ui1, task: &str) -> u64 {
    let pid = 9_100;
    let mut p = h.app.data.members[1].clone();
    p.id = pid;
    p.name = LONG_PERSON.into();
    p.display_name.clear();
    h.app.data.members.push(p);
    let mut x = task_named(h, "Design new homepage layout").clone();
    x.id = 31_100;
    x.name = task.into();
    x.assignees = vec![pid];
    x.parent = None;
    h.app.data.tasks.push(x);
    h.settle();
    pid
}

/// The dashboard's bar cards ("Workload by assignee" and the others): a
/// long name is shortened with "…" in its column before the bar, and
/// never runs over the bar or the number at the right.
#[test]
fn dashboard_bars_shorten_long_names() {
    each_size(|w, _, at| {
        // tall enough to show every card
        let mut h = Ui1::sized(w, 1800.0);
        long_person_with_task(&mut h, "Plan the offsite");
        h.app.section = Section::Dashboards;
        h.app.dash_open = Some(crate::app::hubs::WORKSPACE_OVERVIEW);
        h.settle();
        assert_fits(&h, page(&h), at);
        let label: Vec<(String, Rect)> = h
            .texts_in(page(&h), Seen::Visible)
            .into_iter()
            .filter(|(s, _)| s.starts_with("Bart"))
            .collect();
        assert!(!label.is_empty(), "{at}: the long name's row");
        for (s, r) in label {
            assert!(s.ends_with('\u{2026}'), "{at}: {s:?} in full");
            assert!(r.width() <= 120.0, "{at}: {s:?} {r:?} runs over the bar");
        }
    });
}

/// The goal card shared to chat: a long goal name is shortened before the
/// percent at the right instead of running under it.
#[test]
fn shared_goal_card_shortens_a_long_name_before_the_percent() {
    each_size(|w, hh, at| {
        let mut h = Ui1::sized(w, hh);
        let id = h.app.data.new_id();
        h.app.data.extra.goals.push(Goal {
            id,
            name: "Win three hundred new customers and more repeat orders this half".into(),
            color: [0x44, 0x66, 0xff],
            due: Some(today() + chrono::Duration::days(30)),
            owner: h.app.data.me,
            start: Some(today() - chrono::Duration::days(10)),
            ..Default::default()
        });
        h.app.open_goal_share(id);
        h.settle();
        let dialog = h.modal_rect();
        let texts = h.texts_in(dialog, Seen::Visible);
        let name = texts
            .iter()
            .find(|(s, r)| {
                s.starts_with("Win three") && !s.contains(" is done") && dialog.contains_rect(*r)
            })
            .unwrap_or_else(|| panic!("{at}: the card's name"))
            .1;
        let pct = texts
            .iter()
            .find(|(s, r)| s.ends_with('%') && (r.center().y - name.center().y).abs() < 20.0)
            .unwrap_or_else(|| panic!("{at}: the card's percent"))
            .1;
        assert!(
            name.right() <= pct.left(),
            "{at}: the name {name:?} runs under {pct:?}"
        );
        assert!(
            pct.right() <= dialog.right(),
            "{at}: {pct:?} outside {dialog:?}"
        );
    });
}

/// Teams' Analytics: a long task name on a person's card is shortened with
/// "…" before its due date, and a long name over the card is too.
#[test]
fn teams_analytics_cards_shorten_long_names() {
    each_size(|w, _, at| {
        // tall enough to show every card
        let mut h = Ui1::sized(w, 1800.0);
        long_person_with_task(&mut h, LONG_NAME);
        h.app.section = Section::Teams;
        h.app.team_open = None;
        h.app.hub.teams = TeamsPage::from_key(3);
        h.settle();
        assert_fits(&h, page(&h), at);
        assert!(
            h.texts_in(page(&h), Seen::Visible)
                .iter()
                .any(|(s, _)| s.starts_with("A task with")),
            "{at}: the long task's row"
        );
    });
}

/// Settings > Templates and the Template Center: the category tabs and
/// "+ New task template" share a row only when they fit; else the button
/// goes on a line under the tabs instead of over "Docs".
#[test]
fn template_tabs_and_new_task_template_do_not_overlap() {
    each_size(|w, hh, at| {
        for dialog in [false, true] {
            let mut h = Ui1::sized(w, hh);
            if dialog {
                h.app
                    .open_templates(crate::app::templates::Category::All, None);
            } else {
                h.app.section = Section::Settings;
                h.app.settings_tab = crate::app::settings::SettingsTab::Templates;
                h.app.settings_state = None;
            }
            h.settle();
            let at = format!("{at} {}", if dialog { "dialog" } else { "page" });
            let new = h.rect("New task template");
            assert!(new.right() <= w, "{at}: {new:?}");
            for cat in ["All templates", "Lists & Spaces", "Tasks", "Docs"] {
                let r = h.rect(&format!("Template category {cat}"));
                assert!(!r.intersects(new), "{at}: {cat} {r:?} under {new:?}");
            }
            if !dialog {
                let over = overlapping(&h.texts_in(page(&h), Seen::Visible));
                assert!(over.is_empty(), "{at}: overlapping {over:?}");
            }
        }
    });
}

/// Settings > Templates scrolls with the page: the templates run on to the
/// bottom of the window instead of stopping in a box of their own above
/// empty space with the last one cut.
#[test]
fn settings_templates_run_to_the_bottom_of_the_page() {
    for (w, hh) in [(960.0, 560.0), (1440.0, 900.0), (700.0, 640.0)] {
        let mut h = Ui1::sized(w, hh);
        h.app.section = Section::Settings;
        h.app.settings_tab = crate::app::settings::SettingsTab::Templates;
        h.app.settings_state = None;
        h.settle();
        // a template's name cut off at the bottom is cut by the page's
        // own edge, not by a box ending above it
        let names: Vec<String> = crate::app::templates::builtins()
            .iter()
            .map(|b| tr(b.en).to_string())
            .collect();
        let mut seen = 0;
        super::harness::each_text(&h.shapes, |ts, clip| {
            let r = ts.visual_bounding_rect();
            if !names.iter().any(|n| n == ts.galley.text()) || r.top() >= clip.bottom() {
                return;
            }
            seen += 1;
            if r.bottom() > clip.bottom() + 0.5 {
                assert!(
                    clip.bottom() >= hh - 40.0,
                    "{w}x{hh}: {:?} cut at {}, above the page's bottom",
                    ts.galley.text(),
                    clip.bottom()
                );
            }
        });
        assert!(seen > 0, "{w}x{hh}: the templates");
    }
}

/// Settings > Notifications: in a narrow window "Send a test notification"
/// goes under the sentence about turning banners on, instead of over it.
#[test]
fn notifications_test_button_stays_clear_of_its_sentence() {
    each_size(|w, hh, at| {
        let mut h = Ui1::sized(w, hh);
        h.app.section = Section::Settings;
        h.app.settings_tab = crate::app::settings::SettingsTab::Notifications;
        h.app.settings_state = None;
        h.settle();
        h.scroll_into_view("Send a test notification");
        let b = h.rect("Send a test notification");
        assert!(b.right() <= w, "{at}: {b:?}");
        let label = tr("Send a test notification");
        for (s, r) in h.texts_in(page(&h), Seen::Visible) {
            if s != label && s.len() > 3 {
                assert!(!r.intersects(b), "{at}: {s:?} {r:?} under the button {b:?}");
            }
        }
        assert_fits(&h, page(&h), at);
    });
}

/// The Planner in a narrow window: "To schedule" and the seven days go one
/// under another, each the page's width, so no day is cut at the window's
/// edge; the week's dates go under the buttons when they don't fit.
#[test]
fn narrow_planner_stacks_its_days() {
    each_size(|w, hh, at| {
        let mut h = Ui1::sized(w, hh);
        let mut x = task_named(&h, "Design new homepage layout").clone();
        x.id = 31_200;
        x.name = LONG_NAME.into();
        x.assignees = vec![h.app.data.me];
        x.due = Some(today());
        x.parent = None;
        h.app.data.tasks.push(x);
        h.app.section = Section::Planner;
        h.settle();
        let week = h.app.planner_week;
        for i in 0..7 {
            let name = format!(
                "Add task on {}",
                (week + chrono::Duration::days(i)).format("%a %-d")
            );
            h.scroll_into_view(&name);
            let r = h.rect(&name);
            assert!(r.right() <= w, "{at}: {name} {r:?}");
            assert_fits(&h, page(&h), &format!("{at} at {name}"));
        }
    });
}
