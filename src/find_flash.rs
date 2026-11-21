// The words just searched for, lit up where a search result leads. After a
// result of the search palette is opened, every text on screen that holds
// the searched words shows those letters in orange for two seconds, fading
// back to their own colour at the end. As with color emoji (emoji.rs), the
// text shapes are rewritten just before each pass ends, so the task window,
// a doc, the chat, the sidebar and every other screen get it without doing
// anything themselves.

use crate::widgets::WARN_ORANGE;
use egui::epaint::{Galley, Shape, TextShape};
use egui::{Color32, Context};
use std::sync::Arc;

/// How long the words stay lit, and the end part of it they fade out over.
pub const SECS: f64 = 2.0;
const FADE: f64 = 0.5;

/// What is lit and since when (`InputState::time`).
#[derive(Clone)]
struct Lit {
    query: Vec<char>,
    since: f64,
}

fn key() -> egui::Id {
    egui::Id::new("find-flash")
}

/// Lights `query` wherever it shows on screen, from this pass for `SECS`.
/// An empty query lights nothing (and puts out what was lit).
pub fn start(ctx: &Context, query: &str) {
    let query = fold(query.trim());
    if query.is_empty() {
        ctx.data_mut(|d| d.remove::<Lit>(key()));
        return;
    }
    install(ctx);
    let since = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(key(), Lit { query, since }));
    ctx.request_repaint();
}

/// The words lit now, if any.
pub fn lit(ctx: &Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<Lit>(key()))
        .map(|l| l.query.into_iter().collect())
}

fn install(ctx: &Context) {
    let id = egui::Id::new("find-flash-installed");
    if ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, true));
    ctx.on_end_pass("find_flash", Arc::new(paint));
}

/// Letters compared without case, as the search does.
fn fold(s: &str) -> Vec<char> {
    s.chars().map(fold_char).collect()
}

fn fold_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn paint(ctx: &Context) {
    let Some(lit) = ctx.data(|d| d.get_temp::<Lit>(key())) else {
        return;
    };
    let age = ctx.input(|i| i.time) - lit.since;
    if !(0.0..SECS).contains(&age) {
        ctx.data_mut(|d| d.remove::<Lit>(key()));
        // one more pass draws the text in its own colour again
        ctx.request_repaint();
        return;
    }
    // full orange, then back to the text's own colour over the last moment
    let k = if age < SECS - FADE {
        1.0
    } else {
        ((SECS - age) / FADE) as f32
    };
    let needle: String = lit.query.iter().collect();
    ctx.graphics_mut(|g| {
        for list in g.all_lists_mut() {
            for cs in list.shapes_mut().iter_mut() {
                rewrite(&mut cs.shape, &lit.query, &needle, k);
            }
        }
    });
    ctx.request_repaint();
}

/// Colours the matches of `query` in every text shape in `shape`.
fn rewrite(shape: &mut Shape, query: &[char], needle: &str, k: f32) {
    match shape {
        Shape::Vec(v) => v.iter_mut().for_each(|s| rewrite(s, query, needle, k)),
        Shape::Text(ts) if ts.opacity_factor > 0.0 => {
            // quick skip: most texts on screen don't hold the words
            if !ts.galley.text().to_lowercase().contains(needle) {
                return;
            }
            let hits = find(&ts.galley, query);
            if !hits.is_empty() {
                light(ts, &hits, k);
            }
        }
        _ => {}
    }
}

/// The (row, glyph) of every letter of every match of `query` in a galley.
/// A match may run on over a wrapped line, not past the end of a line.
fn find(galley: &Galley, query: &[char]) -> Vec<(usize, usize)> {
    // every glyph in order; `None` stands for a line break
    let mut seq: Vec<Option<(usize, usize, char)>> = vec![];
    for (ri, row) in galley.rows.iter().enumerate() {
        for (gi, g) in row.glyphs.iter().enumerate() {
            seq.push(Some((ri, gi, fold_char(g.chr))));
        }
        if row.ends_with_newline {
            seq.push(None);
        }
    }
    let mut out = vec![];
    let n = query.len();
    let mut i = 0;
    while i + n <= seq.len() {
        let here = &seq[i..i + n];
        let same = here
            .iter()
            .zip(query)
            .all(|(s, q)| s.is_some_and(|(_, _, c)| c == *q));
        if same {
            out.extend(here.iter().flatten().map(|(r, g, _)| (*r, *g)));
            i += n;
        } else {
            i += 1;
        }
    }
    out
}

/// Turns these glyphs orange (`k` = 1) or partly back to their own colour.
fn light(ts: &mut TextShape, hits: &[(usize, usize)], k: f32) {
    let over = ts.override_text_color.take();
    let fallback = ts.fallback_color;
    let galley = Arc::make_mut(&mut ts.galley);
    for (ri, row) in galley.rows.iter_mut().enumerate() {
        let range = row.visuals.glyph_vertex_range.clone();
        // an override colours every glyph of the text: give the glyphs
        // that colour themselves, as it is dropped to colour the matches
        if let Some(c) = over {
            for v in &mut row.visuals.mesh.vertices[range.clone()] {
                v.color = c;
            }
        }
        if !hits.iter().any(|(r, _)| *r == ri) {
            continue;
        }
        let mut v = range.start;
        for (gi, g) in row.glyphs.iter().enumerate() {
            if g.uv_rect.is_nothing() {
                continue; // a space: no corners
            }
            if hits.contains(&(ri, gi)) && v + 4 <= range.end {
                for vx in &mut row.visuals.mesh.vertices[v..v + 4] {
                    let own = if vx.color == Color32::PLACEHOLDER {
                        fallback
                    } else {
                        vx.color
                    };
                    let orange = Color32::from_rgba_unmultiplied(
                        WARN_ORANGE.r(),
                        WARN_ORANGE.g(),
                        WARN_ORANGE.b(),
                        own.a(),
                    );
                    vx.color = own.lerp_to_gamma(orange, k);
                }
            }
            v += 4;
        }
    }
}
