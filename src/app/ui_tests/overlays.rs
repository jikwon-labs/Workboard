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
