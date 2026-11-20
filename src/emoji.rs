// Color emoji. egui draws every glyph in one color, so emoji came out black
// and white. Just before each pass ends, every text shape on screen is
// checked for emoji; their black-and-white glyphs are hidden and a color
// picture of the emoji is painted over the same spot. The picture comes from
// the system's emoji font (`os::render_emoji`) and is made once per emoji and
// size. Messages, reactions, the emoji picker, task names and text boxes all
// get it without each screen doing anything. Where the system has no color
// emoji to give (outside macOS for now) the old glyphs stay.

use egui::epaint::{Galley, Shape, TextShape};
use egui::{pos2, vec2, Color32, Context, Rect, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// Starts drawing color emoji in this context (once; later calls do nothing).
pub fn install(ctx: &Context) {
    let id = egui::Id::new("color-emoji-installed");
    if ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false) {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, true));
    ctx.on_end_pass("color_emoji", Arc::new(paint));
}

/// One emoji's picture: its texture and its size in pixels.
type Picture = Option<(TextureHandle, [usize; 2])>;
/// Pictures by (emoji, picture size in pixels); `None` when the system has none.
type Cache = Arc<Mutex<HashMap<(String, u32), Picture>>>;

fn cache(ctx: &Context) -> Cache {
    let id = egui::Id::new("color-emoji-cache");
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(id).clone())
}

fn paint(ctx: &Context) {
    let ppp = ctx.pixels_per_point();
    // which emoji are on screen this pass, at which picture size
    let mut want = HashSet::new();
    ctx.graphics_mut(|g| {
        for list in g.all_lists_mut() {
            for cs in list.shapes_mut().iter() {
                visit(&cs.shape, &mut |ts| {
                    for hit in find(&ts.galley) {
                        want.insert((hit.text, bucket(hit.side * ppp)));
                    }
                });
            }
        }
    });
    if want.is_empty() {
        return;
    }
    let cache = cache(ctx);
    let missing: Vec<_> = {
        let c = cache.lock().unwrap();
        want.iter()
            .filter(|k| !c.contains_key(*k))
            .cloned()
            .collect()
    };
    for (text, px) in missing {
        let pic = crate::os::render_emoji(&text, px).map(|img| {
            let size = img.size;
            let tex = ctx.load_texture(format!("emoji {text} {px}"), img, TextureOptions::LINEAR);
            (tex, size)
        });
        cache.lock().unwrap().insert((text, px), pic);
    }
    let c = cache.lock().unwrap();
    ctx.graphics_mut(|g| {
        for list in g.all_lists_mut() {
            let shapes = list.shapes_mut();
            for cs in shapes.iter_mut() {
                rewrite(&mut cs.shape, ppp, &c);
            }
        }
    });
}

/// Calls `f` for every text shape in `shape`, also inside groups.
fn visit(shape: &Shape, f: &mut impl FnMut(&TextShape)) {
    match shape {
        Shape::Text(ts) if ts.angle == 0.0 && ts.opacity_factor > 0.0 => f(ts),
        Shape::Vec(v) => v.iter().for_each(|s| visit(s, f)),
        _ => {}
    }
}

/// Hides the emoji glyphs of every text shape in `shape` and puts the color
/// pictures over them.
fn rewrite(shape: &mut Shape, ppp: f32, cache: &HashMap<(String, u32), Picture>) {
    match shape {
        Shape::Vec(v) => v.iter_mut().for_each(|s| rewrite(s, ppp, cache)),
        Shape::Text(ts) if ts.angle == 0.0 && ts.opacity_factor > 0.0 => {
            let hits = find(&ts.galley);
            if hits.is_empty() {
                return;
            }
            let mut pictures = Vec::new();
            let mut hidden = Vec::new();
            for hit in hits {
                let Some(Some((tex, [w, h]))) =
                    cache.get(&(hit.text.clone(), bucket(hit.side * ppp)))
                else {
                    continue; // no color picture: the glyph stays
                };
                let alpha = text_alpha(ts, &hit);
                if alpha == 0 {
                    continue; // see-through text (spacers) stays see-through
                }
                // the picture keeps its shape and fits in a square of `side`
                let k = hit.side / (*w).max(*h) as f32;
                let size = vec2(*w as f32 * k, *h as f32 * k);
                let rect = Rect::from_center_size(hit.center, size).translate(ts.pos.to_vec2());
                let tint = Color32::WHITE
                    .gamma_multiply(alpha as f32 / 255.0 * ts.opacity_factor.min(1.0));
                pictures.push(Shape::image(
                    tex.id(),
                    rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    tint,
                ));
                hidden.push((hit.row, hit.glyphs));
            }
            if pictures.is_empty() {
                return;
            }
            hide_glyphs(Arc::make_mut(&mut ts.galley), &hidden);
            let text = std::mem::replace(shape, Shape::Noop);
            pictures.insert(0, text);
            *shape = Shape::Vec(pictures);
        }
        _ => {}
    }
}

/// How opaque the text of an emoji is (0 = not drawn at all).
fn text_alpha(ts: &TextShape, hit: &Hit) -> u8 {
    if let Some(c) = ts.override_text_color {
        return c.a();
    }
    let section = ts.galley.rows[hit.row].glyphs[hit.glyphs.start].section_index as usize;
    let c = ts.galley.job.sections[section].format.color;
    if c == Color32::PLACEHOLDER {
        ts.fallback_color.a()
    } else {
        c.a()
    }
}

/// Folds the four corners of each of these glyphs into one point, so the
/// black-and-white glyph is not drawn under the color picture.
fn hide_glyphs(galley: &mut Galley, hidden: &[(usize, std::ops::Range<usize>)]) {
    for (row_i, glyphs) in hidden {
        let row = &mut galley.rows[*row_i];
        let mut v = row.visuals.glyph_vertex_range.start;
        for (i, g) in row.glyphs.iter().enumerate() {
            if g.uv_rect.is_nothing() {
                continue; // a space: no corners
            }
            if glyphs.contains(&i) && v + 4 <= row.visuals.mesh.vertices.len() {
                let p = row.visuals.mesh.vertices[v].pos;
                for vx in &mut row.visuals.mesh.vertices[v..v + 4] {
                    vx.pos = p;
                }
            }
            v += 4;
        }
    }
}

/// The picture size (pixels) to make for an emoji shown `px` pixels wide:
/// a few fixed sizes, so a handful of pictures serve every text size.
fn bucket(px: f32) -> u32 {
    [32, 64, 128]
        .into_iter()
        .find(|&b| b as f32 >= px)
        .unwrap_or(256)
}

/// One emoji in a galley.
#[derive(Debug)]
struct Hit {
    /// The emoji's characters (one or more, e.g. a skin tone or a family).
    text: String,
    row: usize,
    /// Its glyphs in that row.
    glyphs: std::ops::Range<usize>,
    /// The middle of the square it is drawn in, and the square's side.
    center: egui::Pos2,
    side: f32,
}

/// Every emoji in a galley.
fn find(galley: &Galley) -> Vec<Hit> {
    let mut out = Vec::new();
    for (row_i, row) in galley.rows.iter().enumerate() {
        // quick skip: no emoji starts below U+00A9
        if !row.glyphs.iter().any(|g| g.chr as u32 >= 0xA9) {
            continue;
        }
        let chars: Vec<char> = row.glyphs.iter().map(|g| g.chr).collect();
        let mut i = 0;
        while i < chars.len() {
            let Some(n) = emoji_len(&chars[i..]) else {
                i += 1;
                continue;
            };
            let first = &row.glyphs[i];
            let last = &row.glyphs[i + n - 1];
            let size = galley.job.sections[first.section_index as usize]
                .format
                .font_id
                .size;
            let x0 = first.pos.x;
            let x1 = last.pos.x + last.advance_width;
            // the emoji glyph's own square (as wide as the text is tall),
            // a little inside it so the picture does not touch the letters
            // beside it; a flag or a family takes several glyphs' room
            let side = size.min(x1 - x0).max(size * 0.5) * 0.92;
            let cy = first.pos.y - first.font_ascent + first.font_height * 0.5;
            out.push(Hit {
                text: chars[i..i + n].iter().collect(),
                row: row_i,
                glyphs: i..i + n,
                center: pos2((x0 + x1) * 0.5, cy),
                side,
            });
            i += n;
        }
    }
    out
}

const VS16: char = '\u{FE0F}';
const VS15: char = '\u{FE0E}';
const ZWJ: char = '\u{200D}';
const KEYCAP: char = '\u{20E3}';

/// How many characters the emoji at the start of `s` takes, if one starts
/// there: a single emoji, one with a skin tone, a ZWJ sequence (a family), a
/// flag (two regional letters), a keycap or a tag sequence.
pub(crate) fn emoji_len(s: &[char]) -> Option<usize> {
    let c = *s.first()?;
    let at = |i: usize| s.get(i).copied();
    // keycap: 1, #, * with the enclosing keycap mark
    if c.is_ascii_digit() || c == '#' || c == '*' {
        return match (at(1), at(2)) {
            (Some(VS16), Some(KEYCAP)) => Some(3),
            (Some(KEYCAP), _) => Some(2),
            _ => None,
        };
    }
    if is_regional(c) {
        return Some(if at(1).is_some_and(is_regional) { 2 } else { 1 });
    }
    if at(1) == Some(VS15) {
        return None; // asked for as text
    }
    let starts = is_emoji_presentation(c)
        || ALWAYS_EMOJI.contains(&c)
        || (at(1) == Some(VS16) && c as u32 >= 0xA9);
    if !starts {
        return None;
    }
    let mut i = 1;
    loop {
        if at(i) == Some(VS16) {
            i += 1;
        }
        if at(i).is_some_and(|c| ('\u{1F3FB}'..='\u{1F3FF}').contains(&c)) {
            i += 1; // skin tone
        }
        while at(i).is_some_and(|c| ('\u{E0020}'..='\u{E007F}').contains(&c)) {
            i += 1; // tag sequence (the flags of England, Scotland, Wales)
        }
        if at(i) == Some(ZWJ) && at(i + 1).is_some_and(|c| c as u32 >= 0xA9) {
            i += 2;
            continue;
        }
        return Some(i);
    }
}

fn is_regional(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

/// Shown as text by default but always meant as emoji here: the hearts and
/// hands of the emoji picker (`chat::EMOJIS`), sent without U+FE0F.
const ALWAYS_EMOJI: [char; 2] = ['\u{2764}', '\u{270C}'];

/// Characters shown as emoji even without U+FE0F (Unicode's
/// Emoji_Presentation, emoji-data.txt 16.0).
fn is_emoji_presentation(c: char) -> bool {
    let c = c as u32;
    if c < 0x231A {
        return false;
    }
    EMOJI_PRESENTATION
        .binary_search_by(|&(a, b)| {
            if c < a {
                std::cmp::Ordering::Greater
            } else if c > b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

#[rustfmt::skip]
const EMOJI_PRESENTATION: &[(u32, u32)] = &[
    (0x231A, 0x231B), (0x23E9, 0x23EC), (0x23F0, 0x23F0), (0x23F3, 0x23F3),
    (0x25FD, 0x25FE), (0x2614, 0x2615), (0x2648, 0x2653), (0x267F, 0x267F),
    (0x2693, 0x2693), (0x26A1, 0x26A1), (0x26AA, 0x26AB), (0x26BD, 0x26BE),
    (0x26C4, 0x26C5), (0x26CE, 0x26CE), (0x26D4, 0x26D4), (0x26EA, 0x26EA),
    (0x26F2, 0x26F3), (0x26F5, 0x26F5), (0x26FA, 0x26FA), (0x26FD, 0x26FD),
    (0x2705, 0x2705), (0x270A, 0x270B), (0x2728, 0x2728), (0x274C, 0x274C),
    (0x274E, 0x274E), (0x2753, 0x2755), (0x2757, 0x2757), (0x2795, 0x2797),
    (0x27B0, 0x27B0), (0x27BF, 0x27BF), (0x2B1B, 0x2B1C), (0x2B50, 0x2B50),
    (0x2B55, 0x2B55), (0x1F004, 0x1F004), (0x1F0CF, 0x1F0CF), (0x1F18E, 0x1F18E),
    (0x1F191, 0x1F19A), (0x1F1E6, 0x1F1FF), (0x1F201, 0x1F201), (0x1F21A, 0x1F21A),
    (0x1F22F, 0x1F22F), (0x1F232, 0x1F236), (0x1F238, 0x1F23A), (0x1F250, 0x1F251),
    (0x1F300, 0x1F320), (0x1F32D, 0x1F335), (0x1F337, 0x1F37C), (0x1F37E, 0x1F393),
    (0x1F3A0, 0x1F3CA), (0x1F3CF, 0x1F3D3), (0x1F3E0, 0x1F3F0), (0x1F3F4, 0x1F3F4),
    (0x1F3F8, 0x1F43E), (0x1F440, 0x1F440), (0x1F442, 0x1F4FC), (0x1F4FF, 0x1F53D),
    (0x1F54B, 0x1F54E), (0x1F550, 0x1F567), (0x1F57A, 0x1F57A), (0x1F595, 0x1F596),
    (0x1F5A4, 0x1F5A4), (0x1F5FB, 0x1F64F), (0x1F680, 0x1F6C5), (0x1F6CC, 0x1F6CC),
    (0x1F6D0, 0x1F6D2), (0x1F6D5, 0x1F6D7), (0x1F6DC, 0x1F6DF), (0x1F6EB, 0x1F6EC),
    (0x1F6F4, 0x1F6FC), (0x1F7E0, 0x1F7EB), (0x1F7F0, 0x1F7F0), (0x1F90C, 0x1F93A),
    (0x1F93C, 0x1F945), (0x1F947, 0x1F9FF), (0x1FA70, 0x1FA7C), (0x1FA80, 0x1FA89),
    (0x1FA8F, 0x1FAC6), (0x1FACE, 0x1FADC), (0x1FADF, 0x1FAE9), (0x1FAF0, 0x1FAF8),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn len(s: &str) -> Option<usize> {
        emoji_len(&s.chars().collect::<Vec<_>>())
    }

    #[test]
    fn finds_whole_emoji_and_leaves_symbols_alone() {
        assert_eq!(len("\u{1F44D} ok"), Some(1), "thumbs up");
        assert_eq!(len("\u{1F44D}\u{1F3FD}"), Some(2), "with a skin tone");
        assert_eq!(len("\u{2764}\u{FE0F}"), Some(2), "red heart with U+FE0F");
        assert_eq!(len("\u{2764}"), Some(1), "the picker's heart");
        assert_eq!(
            len("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}"),
            Some(5),
            "a family"
        );
        assert_eq!(len("\u{1F1F0}\u{1F1F7}"), Some(2), "a flag");
        assert_eq!(len("1\u{FE0F}\u{20E3}"), Some(3), "a keycap");
        assert_eq!(len("1"), None, "a digit");
        assert_eq!(len("a"), None);
        assert_eq!(len("\u{D55C}"), None, "Hangul");
        // symbols the app draws as text: stars, checks, arrows, bullets
        for s in [
            "\u{2605}", "\u{2714}", "\u{2193}", "\u{2022}", "\u{25BE}", "\u{2611}",
        ] {
            assert_eq!(len(s), None, "{s:?} stays text");
        }
        assert_eq!(len("\u{2B50}\u{FE0E}"), None, "asked for as text");
        assert_eq!(
            len("\u{21A9}\u{FE0F}"),
            Some(2),
            "a symbol asked for as emoji"
        );
    }

    #[test]
    fn presentation_table_is_sorted() {
        for w in EMOJI_PRESENTATION.windows(2) {
            assert!(w[0].1 < w[1].0);
        }
    }
}
