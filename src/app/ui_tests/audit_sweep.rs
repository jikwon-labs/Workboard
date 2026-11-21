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
