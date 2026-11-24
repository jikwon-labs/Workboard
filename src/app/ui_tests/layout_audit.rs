// A layout audit for manual QA, closer to the real app than layout_sweep:
// the app's own fonts (Korean glyphs as the Mac draws them), two pixels per
// point, long Korean and English names in the data. It opens every screen
// of `screens()` plus the ones only reachable by a click (menus, detail
// pages, chat threads, the task window's parts, hovers, scrolled pages),
// in each window size and language, draws each one to a PNG and lists:
// - overlap: two texts drawn over each other
// - cut: a text cut mid-glyph by its clip rect
// - past right: a control reaching past the window's right edge
// - dialog too big: a dialog wider or taller than the window
// - covered: a text partly under a button, icon or avatar drawn after it
// - spills: a text running out of the small box (chip, button) under it
// - low contrast (dark mode): a text hard to see on what is behind it
// It reports instead of failing. Run it by hand:
// WB_AUDIT_OUT=/tmp/audit cargo test --release --bin workboard layout_audit -- --ignored --nocapture
// WB_AUDIT_SIZES=1440x900 WB_AUDIT_LANGS=ko WB_AUDIT_ONLY=chat,settings (substrings)
// WB_AUDIT_DARK=1 (dark mode only) WB_AUDIT_NOPNG=1 (no pictures)
// WB_AUDIT_EXACT=1 (WB_AUDIT_ONLY names whole screens) WB_AUDIT_MARKED_ONLY=1
// (pictures only of screens with hits) WB_AUDIT_TEXTS=1 (list every text, for
// measuring contrast on the pictures) WB_AUDIT_RUN=<tag> (own lists per run,
// so several runs can share one folder)

use super::i18n_tests::screens;
use super::layout_tests::heavy;
use super::*;
use crate::i18n::{set_lang, Lang};
use egui::epaint::{Primitive, Shape};
use std::path::{Path, PathBuf};

pub(super) const LONG_KO_TASK: &str =
    "아주 긴 한국어 작업 이름으로 화면마다 어떻게 줄여서 보여 주는지 확인하는 항목입니다";
const LONG_KO_PERSON: &str = "김수한무거북이와두루미 삼천갑자동방삭";
const LONG_KO_DOC: &str = "2026년 하반기 제품 출시 준비 회의록과 결정 사항 정리 문서";
const LONG_KO_CHANNEL: &str = "디자인팀-신규기능-출시-준비-논의방";
const LONG_KO_WORKSPACE: &str = "서울 본사 디자인·개발 통합 작업공간";

pub(super) type Go = Box<dyn Fn(&mut Ui1)>;

/// The audit's data: the seed, layout_tests' heavy state, and long Korean
/// names for a task, a person, a doc, a channel and the workspace.
pub(super) fn audit_window(w: f32, hh: f32) -> Ui1 {
    let mut h = Ui1::real(w, hh, Data::seed());
    heavy(&mut h);
    let d = &mut h.app.data;
    let me = d.me;
    d.workspace = LONG_KO_WORKSPACE.into();
    let pid = 9_001;
    let mut p = d.members[0].clone();
    p.id = pid;
    p.name = LONG_KO_PERSON.into();
    p.display_name = String::new();
    p.title = "프로덕트 디자이너 겸 사용자 연구 담당".into();
    p.email = "kim.suhanmu.geobugi@example-company.co.kr".into();
    p.role = "Member".into();
    d.members.push(p);
    if let Some(mut x) = d.tasks.first().cloned() {
        x.id = 20_200;
        x.name = LONG_KO_TASK.into();
        x.assignees = vec![me, pid];
        x.parent = None;
        d.tasks.push(x);
    }
    if let Some(doc) = d.docs.get_mut(1) {
        doc.title = LONG_KO_DOC.into();
    }
    if let Some(c) = d
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Channel)
        .cloned()
    {
        let mut c = c;
        c.id = 9_002;
        c.name = LONG_KO_CHANNEL.into();
        c.members.push(pid);
        d.channels.push(c);
    }
    h.settle();
    h
}

/// Every screen the audit opens: `screens()` and the extra ones.
pub(super) fn all_screens(h: &Ui1) -> Vec<(String, Go)> {
    let mut v: Vec<(String, Go)> = screens(h)
        .into_iter()
        .map(|(n, go)| {
            let g: Go = Box::new(move |h: &mut Ui1| go(&mut h.app));
            (n, g)
        })
        .collect();
    v.extend(super::layout_audit_screens::extra_screens(h));
    v
}

// ---------------------------------------------------------------- drawing

/// The window drawn as the GPU would, `ppp` pixels per point: premultiplied
/// RGBA, row by row.
pub(super) struct Canvas {
    pub(super) w: usize,
    pub(super) h: usize,
    px: Vec<[f32; 4]>,
}

fn edge(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn sample(t: &egui::ColorImage, u: f32, v: f32) -> [f32; 4] {
    let (tw, th) = (t.size[0], t.size[1]);
    if tw == 0 || th == 0 {
        return [1.0; 4];
    }
    let x = (u * tw as f32 - 0.5).clamp(0.0, (tw - 1) as f32);
    let y = (v * th as f32 - 0.5).clamp(0.0, (th - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(tw - 1), (y0 + 1).min(th - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let get = |x: usize, y: usize| t.pixels[y * tw + x].to_array();
    let (a, b, c, d) = (get(x0, y0), get(x1, y0), get(x0, y1), get(x1, y1));
    let mut out = [0.0; 4];
    for k in 0..4 {
        let top = a[k] as f32 * (1.0 - fx) + b[k] as f32 * fx;
        let bot = c[k] as f32 * (1.0 - fx) + d[k] as f32 * fx;
        out[k] = (top * (1.0 - fy) + bot * fy) / 255.0;
    }
    out
}

impl Canvas {
    pub(super) fn paint(h: &Ui1) -> Canvas {
        let ppp = h.ppp;
        let w = (h.size.x * ppp).round() as usize;
        let hh = (h.size.y * ppp).round() as usize;
        let bg = h.ctx.style().visuals.panel_fill.to_array();
        let bg = [
            bg[0] as f32 / 255.0,
            bg[1] as f32 / 255.0,
            bg[2] as f32 / 255.0,
            1.0,
        ];
        let mut c = Canvas {
            w,
            h: hh,
            px: vec![bg; w * hh],
        };
        let empty = Default::default();
        let textures = h.textures.as_ref().unwrap_or(&empty);
        let prims = h.ctx.tessellate(h.shapes.clone(), ppp);
        for p in prims {
            let Primitive::Mesh(m) = p.primitive else {
                continue;
            };
            let tex = textures.get(&m.texture_id);
            let cx0 = ((p.clip_rect.min.x * ppp).floor().max(0.0) as usize).min(w);
            let cy0 = ((p.clip_rect.min.y * ppp).floor().max(0.0) as usize).min(hh);
            let cx1 = ((p.clip_rect.max.x * ppp).ceil().max(0.0) as usize).min(w);
            let cy1 = ((p.clip_rect.max.y * ppp).ceil().max(0.0) as usize).min(hh);
            if cx0 >= cx1 || cy0 >= cy1 {
                continue;
            }
            for tri in m.indices.chunks_exact(3) {
                let v = [
                    &m.vertices[tri[0] as usize],
                    &m.vertices[tri[1] as usize],
                    &m.vertices[tri[2] as usize],
                ];
                let p0 = (v[0].pos.x * ppp, v[0].pos.y * ppp);
                let p1 = (v[1].pos.x * ppp, v[1].pos.y * ppp);
                let p2 = (v[2].pos.x * ppp, v[2].pos.y * ppp);
                let area = edge(p0, p1, p2);
                if area.abs() < 1e-6 {
                    continue;
                }
                let minx = p0.0.min(p1.0).min(p2.0).floor().max(cx0 as f32) as usize;
                let maxx = (p0.0.max(p1.0).max(p2.0).ceil().min(cx1 as f32)).max(0.0) as usize;
                let miny = p0.1.min(p1.1).min(p2.1).floor().max(cy0 as f32) as usize;
                let maxy = (p0.1.max(p1.1).max(p2.1).ceil().min(cy1 as f32)).max(0.0) as usize;
                let cols = [
                    v[0].color.to_array(),
                    v[1].color.to_array(),
                    v[2].color.to_array(),
                ];
                for y in miny..maxy {
                    for x in minx..maxx {
                        let pt = (x as f32 + 0.5, y as f32 + 0.5);
                        let w0 = edge(p1, p2, pt) / area;
                        let w1 = edge(p2, p0, pt) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }
                        let mut s = [0.0f32; 4];
                        for k in 0..4 {
                            s[k] = (cols[0][k] as f32 * w0
                                + cols[1][k] as f32 * w1
                                + cols[2][k] as f32 * w2)
                                / 255.0;
                        }
                        if let Some(t) = tex {
                            let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                            let vv = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                            let tc = sample(t, u, vv);
                            for k in 0..4 {
                                s[k] *= tc[k];
                            }
                        }
                        let d = &mut c.px[y * w + x];
                        let inv = 1.0 - s[3];
                        for k in 0..4 {
                            d[k] = s[k] + d[k] * inv;
                        }
                    }
                }
            }
        }
        c
    }

    /// The colour at a point (in points), as drawn.
    fn at(&self, p: Pos2, ppp: f32) -> Color32 {
        let x = ((p.x * ppp) as usize).min(self.w.saturating_sub(1));
        let y = ((p.y * ppp) as usize).min(self.h.saturating_sub(1));
        let q = self.px[y * self.w + x];
        let b = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgb(b(q[0]), b(q[1]), b(q[2]))
    }

    fn to_image(&self, marks: &[Rect], ppp: f32) -> image::RgbaImage {
        let b = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
        let mut img = image::RgbaImage::new(self.w as u32, self.h as u32);
        for (i, q) in self.px.iter().enumerate() {
            let (x, y) = ((i % self.w) as u32, (i / self.w) as u32);
            img.put_pixel(x, y, image::Rgba([b(q[0]), b(q[1]), b(q[2]), 255]));
        }
        let red = image::Rgba([235, 20, 20, 255]);
        for r in marks {
            let r = r.expand(2.0);
            let x0 = (r.min.x * ppp).max(0.0) as i64;
            let y0 = (r.min.y * ppp).max(0.0) as i64;
            let x1 = ((r.max.x * ppp) as i64).min(self.w as i64 - 1);
            let y1 = ((r.max.y * ppp) as i64).min(self.h as i64 - 1);
            for t in 0..3 {
                for x in x0..=x1 {
                    for y in [y0 + t, y1 - t] {
                        if x >= 0 && y >= 0 && x < self.w as i64 && y < self.h as i64 {
                            img.put_pixel(x as u32, y as u32, red);
                        }
                    }
                }
                for y in y0..=y1 {
                    for x in [x0 + t, x1 - t] {
                        if x >= 0 && y >= 0 && x < self.w as i64 && y < self.h as i64 {
                            img.put_pixel(x as u32, y as u32, red);
                        }
                    }
                }
            }
        }
        img
    }

    pub(super) fn save(&self, path: &Path, marks: &[Rect], ppp: f32) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = self.to_image(marks, ppp).save(path);
    }
}

// --------------------------------------------------------------- checking

/// One thing the audit found on a screen.
pub(super) struct Hit {
    pub(super) kind: &'static str,
    pub(super) what: String,
    pub(super) rects: Vec<Rect>,
}

enum Painted {
    Text {
        text: String,
        rect: Rect,
        color: Color32,
    },
    Fill {
        rect: Rect,
        big: bool,
        /// a picture (an emoji drawn in place of its glyph, a photo)
        picture: bool,
    },
}

fn opaque(c: Color32) -> bool {
    c.a() >= 230
}

/// What was painted, in paint order: texts (the part their clip shows)
/// and opaque fills (rects, circles, closed paths, pictures).
fn painted(h: &Ui1) -> Vec<Painted> {
    let mut out = vec![];
    let screen = Rect::from_min_size(Pos2::ZERO, h.size);
    each_shape(&h.shapes, |s, clip| {
        let clip = clip.intersect(screen);
        let fill = |r: Rect, picture: bool, out: &mut Vec<Painted>| {
            let r = r.intersect(clip);
            if r.is_positive() && r.width() > 1.0 && r.height() > 1.0 {
                let big = r.width() > 400.0 || r.height() > 160.0 || r.area() > 12_000.0;
                out.push(Painted::Fill {
                    rect: r,
                    big,
                    picture,
                });
            }
        };
        match s {
            Shape::Text(ts) => {
                let text = ts.galley.text().trim();
                let r = ts.visual_bounding_rect().intersect(clip);
                if text.is_empty() || !r.is_positive() {
                    return;
                }
                let color = ts.override_text_color.unwrap_or_else(|| {
                    ts.galley
                        .job
                        .sections
                        .first()
                        .map(|s| s.format.color)
                        .filter(|c| *c != Color32::PLACEHOLDER)
                        .unwrap_or(ts.fallback_color)
                });
                out.push(Painted::Text {
                    text: text.to_string(),
                    rect: r,
                    color,
                });
            }
            Shape::Rect(r) if opaque(r.fill) => fill(r.rect, false, &mut out),
            Shape::Circle(c) if opaque(c.fill) => fill(
                Rect::from_center_size(c.center, vec2(c.radius, c.radius) * 1.6),
                false,
                &mut out,
            ),
            Shape::Path(p) if p.closed && opaque(p.fill) => {
                fill(Rect::from_points(&p.points), false, &mut out)
            }
            Shape::Mesh(m)
                if m.texture_id != egui::TextureId::default()
                    || (!m.vertices.is_empty() && m.vertices.iter().all(|v| opaque(v.color))) =>
            {
                fill(
                    m.calc_bounds(),
                    m.texture_id != egui::TextureId::default(),
                    &mut out,
                )
            }
            _ => {}
        }
    });
    out
}

fn short(s: &str) -> String {
    let s = s.replace('\n', " ");
    if s.chars().count() > 60 {
        format!("{}…", s.chars().take(60).collect::<String>())
    } else {
        s
    }
}

fn luminance(c: Color32) -> f32 {
    let f = |u: u8| {
        let x = u as f32 / 255.0;
        if x <= 0.03928 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

fn ratio(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Text colour (premultiplied, maybe see-through) laid over `bg`.
fn over(c: Color32, bg: Color32) -> Color32 {
    let a = c.a() as f32 / 255.0;
    let m = |s: u8, d: u8| (s as f32 + d as f32 * (1.0 - a)).min(255.0) as u8;
    Color32::from_rgb(m(c.r(), bg.r()), m(c.g(), bg.g()), m(c.b(), bg.b()))
}

pub(super) fn check(h: &Ui1, canvas: Option<&Canvas>) -> Vec<Hit> {
    let mut hits = vec![];
    let screen = Rect::from_min_size(Pos2::ZERO, h.size);
    let items = painted(h);
    // texts behind a later big opaque fill (a dialog, a menu, a panel over
    // the page) are meant to be hidden
    let mut texts: Vec<(usize, &str, Rect, Color32)> = vec![];
    for (i, p) in items.iter().enumerate() {
        let Painted::Text { text, rect, color } = p else {
            continue;
        };
        let hidden = items[i + 1..].iter().any(
            |q| matches!(q, Painted::Fill { rect: f, big: true, .. } if f.contains(rect.center())),
        );
        if !hidden && screen.intersects(*rect) {
            texts.push((i, text, *rect, *color));
        }
    }
    // overlap
    let pills: Vec<Rect> = h
        .tree
        .iter()
        .filter(|(n, _)| n == UNREAD_PILL || n == UNREAD_PILL_KO)
        .map(|(_, r)| *r)
        .collect();
    for (k, (ia, a, ra, _)) in texts.iter().enumerate() {
        for (ib, b, rb, _) in &texts[k + 1..] {
            if a == b && ra.center().distance(rb.center()) < 2.0 {
                continue;
            }
            let o = ra.intersect(*rb);
            if !(o.width() > 1.0 && o.height() > 1.0) {
                continue;
            }
            // the floating "Unread messages" pill
            if pills
                .iter()
                .any(|p| p.contains(ra.center()) || p.contains(rb.center()))
            {
                continue;
            }
            // an opaque box drawn between the two (a menu, a dialog, a
            // button): the lower one is hidden there, which "covered"
            // reports when the box is small
            let (lo, hi) = (*ia.min(ib), *ia.max(ib));
            if items[lo + 1..hi]
                .iter()
                .any(|q| matches!(q, Painted::Fill { rect: f, .. } if f.contains(o.center())))
            {
                continue;
            }
            hits.push(Hit {
                kind: "overlap",
                what: format!("{:?} / {:?}", short(a), short(b)),
                rects: vec![*ra, *rb],
            });
        }
    }
    // covered by a later small fill, or spilling out of an earlier one
    for (i, t, r, _) in &texts {
        for (j, q) in items.iter().enumerate() {
            let Painted::Fill {
                rect: f,
                big: false,
                picture,
            } = q
            else {
                continue;
            };
            // an emoji's picture is drawn over the gap its text leaves
            if *picture && t.chars().any(|c| c as u32 >= 0x2190) {
                continue;
            }
            // dots (a doc's bullets) sit in a text's indent
            if f.width() < 6.0 || f.height() < 6.0 {
                continue;
            }
            // the sidebar's "Unread messages" pill floats over it on purpose
            if pills.iter().any(|p| p.contains_rect(f.shrink(1.0))) {
                continue;
            }
            let o = r.intersect(*f);
            if !(o.width() > 2.0 && o.height() > 2.0) {
                continue;
            }
            // a dialog or menu drawn between the two: they are on different
            // layers, and the one below is hidden
            let (lo, hi) = if j > *i { (*i, j) } else { (j, *i) };
            if items[lo + 1..hi].iter().any(
                |q| matches!(q, Painted::Fill { rect: b, big: true, .. } if b.contains(o.center())),
            ) {
                continue;
            }
            if j > *i {
                hits.push(Hit {
                    kind: "covered",
                    what: format!(
                        "{:?} under a box at {:.0},{:.0} {:.0}x{:.0}",
                        short(t),
                        f.min.x,
                        f.min.y,
                        f.width(),
                        f.height()
                    ),
                    rects: vec![*r, *f],
                });
            } else if !f.expand(1.0).contains_rect(*r)
                // a box mostly inside the text (a highlight, a caret) is fine
                && f.width() * f.height() > 0.5 * r.width() * r.height()
            {
                hits.push(Hit {
                    kind: "spills",
                    what: format!(
                        "{:?} out of a box at {:.0},{:.0} {:.0}x{:.0}",
                        short(t),
                        f.min.x,
                        f.min.y,
                        f.width(),
                        f.height()
                    ),
                    rects: vec![*r, *f],
                });
            }
        }
    }
    // cut mid-glyph
    let mut cut = h.cut_texts(screen);
    cut.sort();
    cut.dedup();
    for c in cut {
        let rects = texts
            .iter()
            .filter(|(_, t, _, _)| **t == *c.trim())
            .map(|(_, _, r, _)| *r)
            .collect();
        hits.push(Hit {
            kind: "cut",
            what: format!("{:?}", short(&c)),
            rects,
        });
    }
    // past the right edge
    for name in h.past_right_edge() {
        let rects = h
            .tree
            .iter()
            .filter(|(n, r)| *n == name && r.right() > h.size.x + 1.0)
            .map(|(_, r)| *r)
            .collect();
        hits.push(Hit {
            kind: "past right",
            what: format!("{:?}", short(&name)),
            rects,
        });
    }
    // a dialog bigger than the window
    if let Some(m) = h.try_modal_rect() {
        if m.right() > h.size.x + 1.0
            || m.left() < -1.0
            || m.bottom() > h.size.y + 1.0
            || m.top() < -1.0
        {
            hits.push(Hit {
                kind: "dialog too big",
                what: format!(
                    "dialog {:.0},{:.0} {:.0}x{:.0}",
                    m.min.x,
                    m.min.y,
                    m.width(),
                    m.height()
                ),
                rects: vec![m.intersect(screen)],
            });
        }
    }
    // every text shown, for measuring contrast on the saved pictures
    if std::env::var("WB_AUDIT_TEXTS").is_ok_and(|v| v == "1") {
        for (_, t, r, _) in &texts {
            hits.push(Hit {
                kind: "text",
                what: format!("{:?}", short(t)),
                rects: vec![*r],
            });
        }
    }
    // dark mode: text lost on its background
    if let (Some(c), true) = (canvas, h.app.data.dark) {
        for (_, t, r, col) in &texts {
            if r.width() < 3.0 || r.height() < 3.0 {
                continue;
            }
            // the corners of the text's box are mostly background
            let mut bgs: Vec<Color32> = [
                r.left_top(),
                r.right_top(),
                r.left_bottom(),
                r.right_bottom(),
            ]
            .iter()
            .map(|p| c.at(*p + (r.center() - *p) * 0.05, h.ppp))
            .collect();
            bgs.sort_by(|a, b| luminance(*a).total_cmp(&luminance(*b)));
            let bg = bgs[1];
            let fg = over(*col, bg);
            let k = ratio(fg, bg);
            if k < 1.8 {
                hits.push(Hit {
                    kind: "low contrast",
                    what: format!("{:?} contrast {:.2}", short(t), k),
                    rects: vec![*r],
                });
            }
        }
    }
    hits
}

// ----------------------------------------------------------------- runner

fn env_list(k: &str, default: &str) -> Vec<String> {
    std::env::var(k)
        .unwrap_or_else(|_| default.into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub(super) fn file_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').chars().take(60).collect()
}

#[test]
#[ignore = "manual QA report"]
fn layout_audit() {
    let out_dir = std::env::var("WB_AUDIT_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("workboard-layout-audit"));
    let dark = std::env::var("WB_AUDIT_DARK").is_ok_and(|v| v == "1");
    let pictures = !std::env::var("WB_AUDIT_NOPNG").is_ok_and(|v| v == "1");
    // only the marked copies of screens with hits, not every screen
    let texts_only = std::env::var("WB_AUDIT_TEXTS").is_ok_and(|v| v == "1");
    let marked_only = std::env::var("WB_AUDIT_MARKED_ONLY").is_ok_and(|v| v == "1");
    let sizes: Vec<(f32, f32)> = env_list(
        "WB_AUDIT_SIZES",
        if dark {
            "1440x900"
        } else {
            "1440x900,960x560,700x640,460x640,360x640"
        },
    )
    .iter()
    .filter_map(|s| {
        let (w, h) = s.split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    })
    .collect();
    let langs = env_list("WB_AUDIT_LANGS", "ko,en");
    // several runs can share one folder (one per size and language, in
    // parallel), each with its own list
    let run = std::env::var("WB_AUDIT_RUN")
        .map(|r| format!("-{}", r))
        .unwrap_or_else(|_| if dark { "-dark".into() } else { String::new() });
    let only = env_list("WB_AUDIT_ONLY", "");
    let names: Vec<String> = {
        let h = audit_window(1440.0, 900.0);
        // the same data for the real app (WORKBOARD_HOME=<dir with it as data.json>)
        let _ = std::fs::create_dir_all(&out_dir);
        if let Ok(j) = serde_json::to_string(&h.app.data) {
            let _ = std::fs::write(out_dir.join("audit-data.json"), j);
        }
        all_screens(&h).into_iter().map(|(n, _)| n).collect()
    };
    let _ = std::fs::create_dir_all(&out_dir);
    let _ = std::fs::write(out_dir.join("screens.txt"), names.join("\n"));
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut lines: Vec<String> = vec![];
    let mut opened = 0;
    let mut seen: Vec<String> = vec![];
    for (w, hh) in &sizes {
        for lang in &langs {
            let tag = format!("{}-{}x{}{}", lang, w, hh, if dark { "-dark" } else { "" });
            for (i, name) in names.iter().enumerate() {
                let exact = std::env::var("WB_AUDIT_EXACT").is_ok_and(|v| v == "1");
                if !only.is_empty()
                    && !only.iter().any(|o| {
                        if exact {
                            name == o
                        } else {
                            name.contains(o.as_str())
                        }
                    })
                {
                    continue;
                }
                let file = format!("{:03}-{}", i, file_name(name));
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut h = audit_window(*w, *hh);
                    h.app.data.dark = dark;
                    h.settle();
                    // opened in English (the clicks use English names), then
                    // shown in the language asked for
                    let go = all_screens(&h).into_iter().nth(i).unwrap().1;
                    go(&mut h);
                    h.settle();
                    set_lang(if lang == "ko" { Lang::Ko } else { Lang::En });
                    h.settle();
                    h.settle();
                    let shot = |h: &Ui1, file: &str| -> Vec<Hit> {
                        let canvas = (pictures || (dark && !texts_only)).then(|| Canvas::paint(h));
                        let hits = check(h, canvas.as_ref());
                        if let (Some(c), true) = (&canvas, pictures) {
                            let png = format!("{}.png", file);
                            if !marked_only {
                                c.save(&out_dir.join("all").join(&tag).join(&png), &[], h.ppp);
                            }
                            if !hits.is_empty() {
                                let marks: Vec<Rect> =
                                    hits.iter().flat_map(|x| x.rects.clone()).collect();
                                c.save(
                                    &out_dir.join("marked").join(&tag).join(&png),
                                    &marks,
                                    h.ppp,
                                );
                            }
                        }
                        hits
                    };
                    let mut out = vec![(file.clone(), shot(&h, &file))];
                    // and scrolled to the bottom, when that shows something else
                    let before = h.texts();
                    let at = h
                        .try_modal_rect()
                        .map(|m| m.center())
                        .unwrap_or(pos2(h.size.x * 0.62, h.size.y * 0.6));
                    h.frame(vec![egui::Event::PointerMoved(at)]);
                    for _ in 0..8 {
                        h.frame(vec![egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: vec2(0.0, -2000.0),
                            modifiers: Default::default(),
                        }]);
                    }
                    // move off so no hover stays
                    h.frame(vec![egui::Event::PointerGone]);
                    h.settle();
                    h.settle();
                    if h.texts() != before {
                        let f = format!("{}-down", file);
                        out.push((f.clone(), shot(&h, &f)));
                    }
                    set_lang(Lang::En);
                    out
                }));
                match r {
                    Ok(shots) => {
                        for (file, hits) in shots {
                            opened += 1;
                            let name = if file.ends_with("-down") {
                                format!("{} (scrolled down)", name)
                            } else {
                                name.clone()
                            };
                            seen.push(format!("{}\t{}\t{}", tag, file, name));
                            for x in hits {
                                let rects: Vec<String> = x
                                    .rects
                                    .iter()
                                    .map(|r| {
                                        format!(
                                            "{:.0},{:.0},{:.0},{:.0}",
                                            r.min.x, r.min.y, r.max.x, r.max.y
                                        )
                                    })
                                    .collect();
                                lines.push(format!(
                                    "{}\t{}\t{}\t{}\t{}\t{}",
                                    tag,
                                    file,
                                    name,
                                    x.kind,
                                    x.what,
                                    rects.join(";")
                                ));
                            }
                        }
                    }
                    Err(_) => {
                        set_lang(Lang::En);
                        lines.push(format!("{}\t{}\t{}\tcould not open\t\t", tag, file, name));
                    }
                }
            }
            let _ = std::fs::write(out_dir.join(format!("hits{}.tsv", run)), lines.join("\n"));
            println!("{} done ({} hits so far)", tag, lines.len());
        }
    }
    std::panic::set_hook(quiet);
    let _ = std::fs::write(out_dir.join(format!("opened{}.tsv", run)), seen.join("\n"));
    let _ = std::fs::write(out_dir.join(format!("hits{}.tsv", run)), lines.join("\n"));
    println!(
        "{} screens x conditions opened, {} hits, written to {}",
        opened,
        lines.len(),
        out_dir.display()
    );
}
