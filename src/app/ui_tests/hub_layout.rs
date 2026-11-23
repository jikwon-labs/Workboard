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
