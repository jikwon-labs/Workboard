// Click-through tests for screens with little or nothing in them: a new
// workspace, one person alone, no spaces, no tasks. Each screen should say
// what is going on and offer a next step that works.

use super::*;

/// A workspace with only me in it, one space and list, and no tasks.
fn solo() -> Data {
    Data::blank(
        "Solo",
        Member {
            id: 1,
            name: "Me".into(),
            color: [0x5f, 0x55, 0xee],
            role: "Owner".into(),
            ..Default::default()
        },
    )
}

/// The same with no space or list at all.
fn no_space() -> Data {
    let mut d = solo();
    d.spaces.clear();
    d
}

fn with(data: Data, w: f32, h: f32) -> Ui1 {
    let mut ui = Ui1::sized(w, h);
    ui.app = App::build(None, Some(data));
    ui.settle();
    ui
}

impl Ui1 {
    /// Every text painted in the last frame.
    fn texts(&self) -> Vec<String> {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Text(ts) => out.push(ts.galley.text().to_string()),
                _ => {}
            }
        }
        let mut out = vec![];
        for s in &self.shapes {
            walk(&s.shape, &mut out);
        }
        out
    }

    fn count_named(&self, name: &str) -> usize {
        self.tree.iter().filter(|(n, _)| n == name).count()
    }
}

#[test]
fn docs_page_does_not_offer_the_task_csv_importer() {
    let mut h = Ui1::new();
    h.click("Rail More");
    h.click("Docs");
    assert_eq!(h.app.section, Section::Docs);
    assert!(h.has("New Doc"));
    assert!(!h.has("\u{2B07}  Import"), "Import read tasks into a List");
}
