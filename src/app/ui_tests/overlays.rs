// Dialogs, menus and floating windows in small windows: each stays inside
// the window, its title row and buttons can be reached, and what it holds
// isn't cut or drawn over something else.

use super::*;

/// The smallest window with the sidebar beside the page, a medium one and
/// two phone-width ones.
const SMALL: [(f32, f32); 4] = [
    (960.0, 560.0),
    (700.0, 640.0),
    (460.0, 640.0),
    (360.0, 640.0),
];

fn window(h: &Ui1) -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2())
}

/// The dialogs the screens list doesn't open, the tall ones among them.
fn more_dialogs() -> Vec<(&'static str, fn(&mut App) -> Modal)> {
    vec![
        ("edit profile", |a| {
            a.open_profile();
            a.modal.take().unwrap()
        }),
        ("new goal", |a| {
            a.open_new_goal(None);
            a.modal.take().unwrap()
        }),
        ("new goal sample", |a| {
            a.open_new_goal(Some(0));
            a.modal.take().unwrap()
        }),
        ("shortcuts", |_| Modal::Shortcuts),
        ("notifications", |_| Modal::Notifications),
        ("themes", |_| Modal::Themes),
        ("click apps", |_| Modal::ClickApps),
        ("custom fields", |_| Modal::custom_fields()),
        ("workflows", |_| Modal::Workflows {
            form: Default::default(),
        }),
        ("who am i", |_| Modal::WhoAmI {
            name: String::new(),
            email: String::new(),
        }),
        ("join team", |_| Modal::JoinTeam {
            addr: String::new(),
        }),
        ("delete workspace", |_| Modal::DeleteWorkspace {
            typed: String::new(),
        }),
    ]
}

/// The open dialog is wholly inside the window: its title row with the
/// close button at the top, its body (which scrolls when it is taller)
/// below.
fn dialog_inside(h: &Ui1, what: &str) -> Option<String> {
    let r = h.try_modal_rect()?;
    if !window(h).expand(1.0).contains_rect(r) {
        return Some(format!(
            "{} at {:?}: dialog {:?} outside the window",
            what, h.size, r
        ));
    }
    let close = h
        .tree
        .iter()
        .filter(|(n, c)| n == "Close" && r.contains_rect(*c))
        .map(|(_, c)| *c)
        .min_by(|a, b| a.top().total_cmp(&b.top()));
    match close {
        Some(c) if c.top() < r.top() + 40.0 => None,
        _ => Some(format!(
            "{} at {:?}: no close button at the dialog's top",
            what, h.size
        )),
    }
}

#[test]
fn dialogs_stay_inside_small_windows() {
    let mut bad = vec![];
    for (w, hh) in SMALL {
        // every dialog the screens list opens
        let names: Vec<String> = {
            let h = Ui1::new();
            super::i18n_tests::screens(&h)
                .into_iter()
                .map(|(n, _)| n)
                .filter(|n| n.starts_with("modal "))
                .collect()
        };
        for name in &names {
            let mut h = Ui1::new();
            super::layout_tests::heavy(&mut h);
            h.settle();
            h.size = vec2(w, hh);
            let go = super::i18n_tests::screens(&h)
                .into_iter()
                .find(|(n, _)| n == name)
                .map(|(_, go)| go)
                .unwrap();
            h.app.modal = None;
            go(&mut h.app);
            h.settle();
            bad.extend(dialog_inside(&h, name));
        }
        for (name, open) in more_dialogs() {
            let mut h = Ui1::sized(w, hh);
            super::layout_tests::heavy(&mut h);
            h.settle();
            let m = open(&mut h.app);
            h.app.modal = Some(m);
            h.settle();
            assert!(h.try_modal_rect().is_some(), "{} opens", name);
            bad.extend(dialog_inside(&h, name));
        }
    }
    assert!(bad.is_empty(), "{:#?}", bad);
}

#[test]
fn tall_dialogs_scroll_to_their_buttons() {
    for (w, hh) in SMALL {
        // New goal: taller than a 560 window; its buttons below the fold
        let mut h = Ui1::sized(w, hh);
        h.app.open_new_goal(None);
        h.settle();
        dialog_inside(&h, "new goal").map(|e| panic!("{}", e));
        h.scroll_dialog_to("Create goal");
        let r = h.rect("Create goal");
        assert!(
            window(&h).contains_rect(r) && h.modal_rect().contains_rect(r),
            "at {}x{}: Create goal {:?} out of reach",
            w,
            hh,
            r
        );
        // its unit box keeps its label on one line
        let unit = h.painted_text("Unit");
        assert!(!unit.is_empty(), "at {}: no Unit label", w);
        assert!(
            unit[0].0.height() < 20.0,
            "at {}: Unit wraps: {:?}",
            w,
            unit[0].0
        );
        let past = h.past_right_edge();
        assert!(past.is_empty(), "at {}: past the right edge: {:?}", w, past);
        h.click("Cancel new goal");
        assert!(h.app.modal.is_none());

        // Edit profile: the picture over the fields in a narrow window
        let mut h = Ui1::sized(w, hh);
        h.app.open_profile();
        h.settle();
        dialog_inside(&h, "edit profile").map(|e| panic!("{}", e));
        h.scroll_dialog_to("Save changes");
        let r = h.rect("Save changes");
        assert!(
            window(&h).contains_rect(r) && h.modal_rect().contains_rect(r),
            "at {}x{}: Save changes {:?} out of reach",
            w,
            hh,
            r
        );
    }
}

/// The control named `name` nearest `x` (a menu's item, not the page's).
fn nearest(h: &Ui1, name: &str, x: f32) -> Option<Rect> {
    h.tree
        .iter()
        .filter(|(n, _)| n == name)
        .map(|(_, r)| *r)
        .min_by(|a, b| {
            (a.center().x - x)
                .abs()
                .total_cmp(&(b.center().x - x).abs())
        })
}

/// Turns the mouse wheel over `at` until the item `name` of the menu near
/// `at` is wholly in the window; where it is then.
fn wheel_to(h: &mut Ui1, at: Pos2, name: &str) -> Option<Rect> {
    for _ in 0..12 {
        if let Some(r) = nearest(h, name, at.x) {
            if window(h).contains_rect(r) {
                return Some(r);
            }
        }
        h.frame(vec![egui::Event::PointerMoved(at)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -120.0),
            modifiers: Default::default(),
        }]);
        h.settle();
    }
    None
}

#[test]
fn long_task_menus_scroll_to_their_last_items() {
    // a task's menu is taller than a 960x560 window: the items under
    // "Remind me" (Start timer, Save as template, Archive, Delete) ran off
    // the bottom
    for how in ["right click", "row button", "card button"] {
        let mut h = Ui1::sized(960.0, 560.0);
        let name = match how {
            "right click" => {
                h.right_click("Task Set up analytics dashboard", 60.0);
                "Set up analytics dashboard"
            }
            "row button" => {
                h.hover(h.find("Task Set up analytics dashboard").unwrap());
                h.click("Task options Set up analytics dashboard");
                "Set up analytics dashboard"
            }
            _ => {
                h.app.view = ViewKind::Board;
                h.settle();
                h.click("Card options Design new homepage layout");
                "Design new homepage layout"
            }
        };
        let tid = h.task_id(name);
        assert!(h.has("Remind me"), "{}: the menu opens", how);
        // (its first items show wherever it opened)
        let x = h.rect("Remind me").center().x;
        let at = nearest(&h, "Open", x).unwrap().center();
        let delete = wheel_to(&mut h, at, "Delete");
        assert!(
            delete.is_some(),
            "{}: Delete never comes into the window",
            how
        );
        for item in ["Start timer", "Archive"] {
            assert!(h.has(item), "{}: {} in the menu", how, item);
        }
        h.click_at(delete.unwrap().center());
        assert!(
            !h.app.data.tasks.iter().any(|t| t.id == tid && !t.archived) || h.app.modal.is_some(),
            "{}: Delete deletes the task",
            how
        );
    }
}

/// Where the area shown in the last frame that holds `p` is (the topmost
/// one other than `not`).
fn area_at(h: &Ui1, p: Pos2, not: egui::Id) -> Option<Rect> {
    h.ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| l.order == egui::Order::Foreground && l.id != not)
            .filter_map(|l| m.area_rect(l.id))
            .filter(|r| r.contains(p))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
    })
}

#[test]
fn bulk_bar_keeps_to_the_page_and_its_popups_clear_it() {
    let bar_id = egui::Id::new("bulk_bar");
    for (w, hh) in SMALL {
        let mut h = Ui1::sized(w, hh);
        let list = h.app.data.first_list().unwrap();
        h.app.section = Section::Home;
        h.app.loc = Location::List(list);
        h.app.view = ViewKind::List;
        let tid = h.task_id("Set up analytics dashboard");
        h.app.selected.insert(tid);
        h.settle();
        let bar = h
            .ctx
            .memory(|m| m.area_rect(bar_id))
            .expect("the bar shows");
        // inside the window, its ends not cut
        assert!(
            window(&h).contains_rect(bar),
            "at {}: bar {:?} wider than the window",
            w,
            bar
        );
        for name in ["Clear selection", "Archive", "Delete"] {
            let r = h.rect_in(name, bar);
            assert!(
                window(&h).contains_rect(r),
                "at {}: {} {:?} cut",
                w,
                name,
                r
            );
        }
        let label = h.painted_text("1 task selected");
        assert!(
            label.iter().any(|(r, _)| window(&h).contains_rect(*r)),
            "at {}: the count is cut",
            w
        );
        // beside the sidebar, it stays off the sidebar
        if w >= 960.0 {
            let side = h.rect("Create");
            assert!(
                bar.left() > side.right(),
                "at {}: bar {:?} over the sidebar {:?}",
                w,
                bar,
                side
            );
        }
        // a popup opened from it is above it, not under its top edge
        for (button, item) in [("Status", "IN PROGRESS"), ("Priority", "Urgent")] {
            let b = h.rect_in(button, bar);
            h.click_at(b.center());
            let shown = h.painted_text(item);
            assert!(!shown.is_empty(), "at {}: {} opens", w, button);
            let menu = shown
                .iter()
                .find_map(|(r, _)| area_at(&h, r.center(), bar_id))
                .unwrap_or_else(|| panic!("at {}: no {} popup", w, button));
            assert!(
                menu.bottom() <= bar.top() + 0.5,
                "at {}: the {} popup {:?} runs under the bar {:?}",
                w,
                button,
                menu,
                bar
            );
            h.key(egui::Key::Escape);
            h.app.selected.insert(tid);
            h.settle();
        }
    }
}

/// general's message with replies (the seed's thread).
fn thread_parent(a: &App) -> (u64, u64) {
    let c = a
        .data
        .channels
        .iter()
        .find(|c| c.name == "general")
        .unwrap();
    let m = c.messages.iter().find(|m| !m.replies.is_empty()).unwrap();
    (c.id, m.id)
}

#[test]
fn floating_windows_and_small_dialogs_stay_inside_small_windows() {
    // the thread window ran past the bottom of a 960x560 window (its
    // "Also send to #general" cut) and, like the notepad and the status
    // editor, a little past the right edge of a 360 one
    let setups: Vec<(&str, fn(&mut App))> = vec![
        ("thread window", |a| {
            let (g, m) = thread_parent(a);
            a.section = Section::Chat;
            a.chat.open = Some(g);
            a.chat.thread_open = Some(m);
            a.chat.thread_window = Some(g);
        }),
        ("notepad", |a| a.notepad_open = true),
        ("status editor", |a| {
            a.edit_statuses = Some(a.data.spaces[0].id)
        }),
        ("new message", |a| a.new_message_open = true),
        ("bulk delete", |a| {
            let t = a.data.tasks[0].id;
            a.selected = [t].into_iter().collect();
            a.confirm_bulk_delete = true;
        }),
        ("rename", |a| {
            let l = a.data.first_list().unwrap();
            a.rename = Some((Target::List(l), "Website Redesign".into()));
        }),
        ("delete space", |a| {
            a.confirm_delete = Some(Target::Space(a.data.spaces[0].id))
        }),
        ("create task", |a| a.open_quick_add(None)),
    ];
    let mut bad = vec![];
    for (w, hh) in SMALL {
        for (name, open) in &setups {
            let mut h = Ui1::sized(w, hh);
            super::layout_tests::heavy(&mut h);
            h.settle();
            open(&mut h.app);
            h.settle();
            h.settle();
            let screen = window(&h);
            let areas: Vec<(egui::LayerId, Rect)> = h.ctx.memory(|m| {
                m.areas()
                    .visible_layer_ids()
                    .into_iter()
                    .filter(|l| matches!(l.order, egui::Order::Middle | egui::Order::Foreground))
                    .filter_map(|l| m.area_rect(l.id).map(|r| (l, r)))
                    .collect()
            });
            let mut seen = false;
            for (l, r) in areas {
                // the dimmed page behind a dialog fills the window
                if r == screen || r.width() < 1.0 {
                    continue;
                }
                seen = true;
                if !screen.expand(0.5).contains_rect(r) {
                    bad.push(format!("{} at {}x{}: {:?} {:?}", name, w, hh, l.id, r));
                }
            }
            assert!(seen, "{} at {}: nothing floating shown", name, w);
        }
    }
    assert!(bad.is_empty(), "{:#?}", bad);
}

#[test]
fn a_popup_closes_when_its_button_scrolls_away() {
    // the dashboard's "+ Add card" menu stayed at the window's top after
    // the page scrolled its button out of sight
    let mut h = Ui1::sized(960.0, 560.0);
    h.app.section = Section::Dashboards;
    h.app.new_dashboard("Workspace overview");
    h.settle();
    h.click("+ Add card");
    let item = "Add card Overdue tasks";
    assert!(h.has(item), "the menu opens");
    // wheel over the page, away from the menu
    let at = pos2(h.size.x - 60.0, h.size.y - 60.0);
    for _ in 0..10 {
        h.frame(vec![egui::Event::PointerMoved(at)]);
        h.frame(vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -200.0),
            modifiers: Default::default(),
        }]);
        h.settle();
        if !h.has("+ Add card") {
            break;
        }
    }
    let shown = h
        .tree
        .iter()
        .any(|(n, r)| n == "+ Add card" && r.bottom() > 40.0 && r.top() < h.size.y);
    assert!(!shown, "the page scrolled the button away");
    assert!(!h.has(item), "the menu went with its button");
}

#[test]
fn recent_tray_names_stay_inside_their_chips() {
    // a long name of wide letters (Korean ones in the app's fonts) was
    // cut by a count of letters, so its "..." ran up against the next
    // chip's status circle
    let mut h = Ui1::sized(960.0, 560.0);
    let names = [
        "WWW MMM WWW MMM WWW MMM WWW MMM WWW MMM",
        "MMMM WWWW MMMM WWWW MMMM WWWW MMMM WWWW MMMM",
        "A very long task name that goes on and on to see how it fits",
    ];
    let mut ids = vec![];
    for (i, n) in names.iter().enumerate() {
        let id = h.app.data.tasks[i].id;
        h.app.data.tasks[i].name = n.to_string();
        ids.push(id);
    }
    h.app.recent = ids;
    h.settle();
    let strip = Rect::from_min_max(pos2(0.0, h.size.y - 40.0), h.size.to_pos2());
    let mut shown: Vec<(String, Rect)> = h
        .texts_in(strip, Seen::Visible)
        .into_iter()
        .filter(|(t, _)| t.chars().count() > 3)
        .collect();
    shown.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
    assert!(shown.len() >= 3, "the chips show: {:?}", shown);
    for pair in shown.windows(2) {
        // the next chip's status circle sits 15 points before its name
        assert!(
            pair[0].1.right() + 15.0 <= pair[1].1.left(),
            "{:?} runs into the next chip {:?}",
            pair[0],
            pair[1]
        );
    }
}
