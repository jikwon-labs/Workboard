// A layout sweep for manual QA: opens every screen the Korean check opens
// (pages, views, settings sections, dialogs) with long names in the data, in
// the default and the smallest window, in English and Korean, and lists
// overlapping texts, texts cut mid-glyph and controls past the right edge.
// It reports instead of failing, because a few hits are intended (a menu
// drawn over the page). Run it by hand:
// cargo test --bin workboard layout_sweep -- --ignored --nocapture

use super::i18n_tests::screens;
use super::layout_tests::{heavy, overlapping};
use super::*;
use crate::i18n::{set_lang, Lang};

const LONG_KO: &str =
    "아주 긴 한국어 작업 이름으로 화면마다 어떻게 줄여서 보여 주는지 확인하는 항목입니다";

#[test]
#[ignore = "manual QA report"]
fn layout_sweep() {
    let mut out = vec![];
    for (w, hh) in [(1440.0, 900.0), (900.0, 560.0)] {
        for lang in [Lang::En, Lang::Ko] {
            let mut h = Ui1::sized(w, hh);
            heavy(&mut h);
            let me = h.app.data.me;
            if let Some(mut x) = h.app.data.tasks.first().cloned() {
                x.id = 20_200;
                x.name = LONG_KO.into();
                x.assignees = vec![me];
                x.parent = None;
                h.app.data.tasks.push(x);
            }
            set_lang(lang);
            for (name, go) in screens(&h) {
                h.app.modal = None;
                h.app.login = None;
                go(&mut h.app);
                h.settle();
                let page = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
                let tag = format!("{}x{} {:?} [{}]", w, hh, lang, name);
                let modal = h.app.modal.is_some() || h.app.login.is_some();
                if !modal {
                    let mut over = overlapping(&h.texts_in(page, Seen::Visible));
                    over.sort();
                    over.dedup();
                    for (a, b) in over {
                        out.push(format!("{} overlap: {:?} / {:?}", tag, a, b));
                    }
                }
                let mut cut = h.cut_texts(page);
                cut.sort();
                cut.dedup();
                for c in cut {
                    out.push(format!("{} cut: {:?}", tag, c));
                }
                for p in h.past_right_edge() {
                    out.push(format!("{} past right: {:?}", tag, p));
                }
            }
            set_lang(Lang::En);
        }
    }
    let path = std::env::var("WB_SWEEP_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("workboard-layout-sweep.txt"));
    let _ = std::fs::write(&path, out.join("\n"));
    println!("{} hits, written to {}", out.len(), path.display());
}

// The same screens in narrow windows (the medium and compact layouts of
// shell/narrow.rs), one fresh app per screen so a panic only loses that
// screen. By hand, like layout_sweep:
// WB_SWEEP_SIZES=360x640,700x640 WB_SWEEP_OUT=/tmp/narrow.txt \
//   cargo test --bin workboard narrow_survey -- --ignored
#[test]
#[ignore = "manual QA report"]
fn narrow_survey() {
    let sizes: Vec<(f32, f32)> = std::env::var("WB_SWEEP_SIZES")
        .unwrap_or_else(|_| "460x640,700x640".into())
        .split(',')
        .filter_map(|s| {
            let (w, h) = s.trim().split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .collect();
    let mut out = vec![];
    std::panic::set_hook(Box::new(|_| {}));
    let names: Vec<String> = {
        let h = Ui1::new();
        screens(&h).into_iter().map(|(n, _)| n).collect()
    };
    for (w, hh) in sizes {
        for (i, name) in names.iter().enumerate() {
            let tag = format!("{}x{} [{}]", w, hh, name);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut lines = vec![];
                let mut h = Ui1::new();
                heavy(&mut h);

                h.settle();
                h.size = vec2(w, hh);
                let go = screens(&h).into_iter().nth(i).unwrap().1;
                h.app.modal = None;
                h.app.login = None;
                go(&mut h.app);
                h.settle();
                let mut page = Rect::from_min_max(pos2(0.0, 0.0), h.size.to_pos2());
                if h.app.open_task.is_some() {
                    // only the task window, not the page under it
                    if let Some(r) = h.ctx.memory(|m| m.area_rect(egui::Id::new("task_window"))) {
                        page = r;
                    }
                }
                let modal = h.app.modal.is_some() || h.app.login.is_some();
                if !modal {
                    let mut over = overlapping(&h.texts_in(page, Seen::Visible));
                    over.sort();
                    over.dedup();
                    for (a, b) in over {
                        lines.push(format!("{} overlap: {:?} / {:?}", tag, a, b));
                    }
                }
                if let Some(m) = h.try_modal_rect() {
                    if m.right() > w + 1.0 || m.left() < -1.0 {
                        lines.push(format!("{} dialog wider than window: {:?}", tag, m));
                    }
                }
                let mut cut = h.cut_texts(page);
                cut.sort();
                cut.dedup();
                for c in cut {
                    lines.push(format!("{} cut: {:?}", tag, c));
                }
                for p in h.past_right_edge() {
                    lines.push(format!("{} past right: {:?}", tag, p));
                }
                lines
            }));
            match r {
                Ok(l) => out.extend(l),
                Err(_) => out.push(format!("{} PANIC", tag)),
            }
        }
    }
    let _ = std::panic::take_hook();
    let path = std::env::var("WB_SWEEP_OUT").unwrap();
    let _ = std::fs::write(&path, out.join("\n"));
    println!("{} hits, written to {}", out.len(), path);
}
