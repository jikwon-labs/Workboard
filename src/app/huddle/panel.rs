// The huddle as a call window shows it: a dark card with the huddle's name
// on top, a stage in the middle and a row of square buttons under it. On
// the stage everyone has a tile: their picture while they send video, else
// a square in their color with their initial. A tile is ringed in green
// while its person speaks and carries a crossed-out microphone while they
// are muted. A shared screen (or a video clicked to make it larger) fills
// the stage, with the tiles in a strip under it. The main window shows the
// whole card over the page; the small window (mini.rs) shows the stage and
// the buttons, smaller.

use super::*;
use crate::app::icons::line_icon_scaled;
use egui::{Pos2, Vec2};

/// The card is dark whatever the theme, as call windows are.
pub(in crate::app) const PANEL_BG: Color32 = Color32::from_rgb(0x1a, 0x1d, 0x21);
const STAGE_BG: Color32 = Color32::from_rgb(0x24, 0x3a, 0x30);
const TEXT: Color32 = Color32::from_rgb(0xf4, 0xf4, 0xf4);
const TEXT2: Color32 = Color32::from_rgb(0xab, 0xad, 0xb0);
/// A button that is off (camera off, not sharing) and one that is on
/// (microphone live, camera on, sharing).
const BUTTON_OFF: Color32 = Color32::from_rgb(0x3c, 0x3e, 0x43);
const BUTTON_ON: Color32 = Color32::from_rgb(0xe9, 0xe9, 0xea);
const LEAVE_RED: Color32 = Color32::from_rgb(0xc9, 0x3d, 0x62);
/// The raised hand.
pub(in crate::app) const HAND: &str = "\u{270B}";
/// How long a reaction shows over its sender's tile.
pub(in crate::app) const REACTION_SECS: f32 = 3.5;
/// The reactions to pick from.
pub(in crate::app) const REACTIONS: [&str; 8] = [
    "\u{1F44D}",
    "\u{1F44F}",
    "\u{1F602}",
    "\u{2764}\u{FE0F}",
    "\u{1F389}",
    "\u{1F62E}",
    "\u{1F64C}",
    "\u{1F44B}",
];
/// The ring around whoever speaks.
const SPEAKING: Color32 = Color32::from_rgb(0x2b, 0xc4, 0x7d);

/// How large the card's parts are: in the main window, or smaller in the
/// small window.
#[derive(Clone, Copy)]
pub(in crate::app) struct Sizes {
    pub(in crate::app) button: Vec2,
    pub(in crate::app) leave_w: f32,
    pub(in crate::app) gap: f32,
    pub(in crate::app) pad: f32,
    pub(in crate::app) tile_max: f32,
    pub(in crate::app) strip: f32,
    pub(in crate::app) radius: f32,
}

impl Sizes {
    pub(in crate::app) const MAIN: Sizes = Sizes {
        button: vec2(44.0, 40.0),
        leave_w: 84.0,
        gap: 8.0,
        pad: 14.0,
        tile_max: 170.0,
        strip: 72.0,
        radius: 10.0,
    };
    pub(in crate::app) const SMALL: Sizes = Sizes {
        button: vec2(38.0, 34.0),
        leave_w: 64.0,
        gap: 6.0,
        pad: 8.0,
        tile_max: 110.0,
        strip: 56.0,
        radius: 8.0,
    };
}

/// One person's tile on the stage.
struct Tile {
    /// Their picture's key (ME for yours).
    key: String,
    person: Option<Person>,
    name: String,
}

/// A frame for the card (the main window's huddle and the small window's).
pub(in crate::app) fn panel_frame(radius: u8, margin: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL_BG)
        .corner_radius(radius)
        .stroke(Stroke::new(1.0_f32, Color32::from_white_alpha(22)))
        .inner_margin(egui::Margin::same(margin))
}

impl App {
    /// Everyone in the huddle as tiles: the others first, you last. Your own
    /// picture gets a tile even before the call lists you.
    fn huddle_tiles(&self) -> Vec<Tile> {
        let mut people: Vec<&Person> = self.huddle.people.iter().collect();
        people.sort_by_key(|p| p.me);
        let mut tiles: Vec<Tile> = people
            .into_iter()
            .map(|p| Tile {
                key: if p.me {
                    ME.to_string()
                } else {
                    p.identity.clone()
                },
                person: Some(p.clone()),
                name: self.huddle_name(p),
            })
            .collect();
        if self.huddle.textures.contains_key(ME) && !tiles.iter().any(|t| t.key == ME) {
            tiles.push(Tile {
                key: ME.to_string(),
                person: None,
                name: "You".into(),
            });
        }
        tiles
    }

    /// The card's title row in the main window: the huddle's name, how many
    /// are in it, and the button that moves it to a small window.
    pub(in crate::app) fn huddle_header(&self, ui: &mut Ui, ch: u64, small: &mut bool) {
        let w = ui.available_width();
        let (row, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
        let p = ui.painter();
        let mut x = row.left() + 4.0;
        paint_headphones(p, pos2(x + 8.0, row.center().y), TEXT);
        x += 24.0;
        let title = p.layout_no_wrap(
            trf!("Huddle in {}", self.huddle_place(ch)),
            FontId::proportional(14.0),
            TEXT,
        );
        let status = p.layout_no_wrap(self.huddle_status(), FontId::proportional(12.0), TEXT2);
        // the button's room on the right; a long name is cut
        let room = (row.right() - 34.0 - x - status.size().x - 10.0).max(40.0);
        let title_w = title.size().x.min(room);
        let title_rect = Rect::from_min_size(
            pos2(x, row.center().y - title.size().y / 2.0),
            vec2(title_w, title.size().y),
        );
        p.with_clip_rect(title_rect)
            .galley(title_rect.min, title, TEXT);
        p.galley(
            pos2(x + title_w + 10.0, row.center().y - status.size().y / 2.0),
            status.clone(),
            TEXT2,
        );
        let label = ui.interact(title_rect, ui.id().with("huddle_title"), Sense::hover());
        label.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, self.huddle_status())
        });
        let b = Rect::from_center_size(pos2(row.right() - 14.0, row.center().y), vec2(28.0, 26.0));
        let r = ui.interact(b, ui.id().with("huddle_pop_out"), Sense::click());
        if r.hovered() {
            ui.painter()
                .rect_filled(b, 6.0, Color32::from_white_alpha(24));
        }
        super::super::icons::line_icon(ui.painter(), Icon::PopOut, b.center(), TEXT);
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Huddle in a small window")
        });
        if r.on_hover_text(tr("Open in a small window")).clicked() {
            *small = true;
        }
    }

    /// The stage: `width` wide and at most `max_h` tall. Tells the call how
    /// large each picture is drawn.
    pub(in crate::app) fn huddle_stage(&mut self, ui: &mut Ui, width: f32, max_h: f32, s: Sizes) {
        let tiles = self.huddle_tiles();
        let screens: Vec<(String, String)> = self
            .huddle_shown_videos()
            .map(|(screens, _)| screens)
            .unwrap_or_default();
        // a focus on a picture that went away (or no longer has a tile) ends
        if self.huddle.focus.as_ref().is_some_and(|f| {
            !self.huddle.textures.contains_key(f) || !tiles.iter().any(|t| &t.key == f)
        }) {
            self.huddle.focus = None;
        }
        let focus = self.huddle.focus.clone();
        let (stage, _) = ui.allocate_exact_size(
            vec2(width, self.stage_height(&tiles, &screens, width, max_h, s)),
            Sense::hover(),
        );
        ui.painter().rect_filled(stage, s.radius, STAGE_BG);
        let inner = stage.shrink(s.pad);
        let ppp = ui.ctx().pixels_per_point();
        let mut drawn: HashMap<String, f32> = HashMap::new();
        let mut clicked: Option<String> = None;
        let big: Vec<(String, String, bool)> = if !screens.is_empty() {
            screens
                .iter()
                .map(|(k, n)| (k.clone(), n.clone(), true))
                .collect()
        } else if let Some(f) = &focus {
            tiles
                .iter()
                .filter(|t| &t.key == f)
                .map(|t| (t.key.clone(), t.name.clone(), false))
                .collect()
        } else {
            vec![]
        };
        if big.is_empty() {
            // a grid of tiles as large as fits, in the middle
            let (cols, side) = grid(tiles.len(), inner.size(), s);
            let rows = tiles.len().div_ceil(cols.max(1));
            let total_h = rows as f32 * side + (rows.saturating_sub(1)) as f32 * s.gap;
            let mut y = inner.center().y - total_h / 2.0;
            for row in tiles.chunks(cols.max(1)) {
                let row_w = row.len() as f32 * side + (row.len() - 1) as f32 * s.gap;
                let mut x = inner.center().x - row_w / 2.0;
                for tile in row {
                    let r = Rect::from_min_size(pos2(x, y), vec2(side, side));
                    if self.huddle_tile(ui, tile, r, s, false) {
                        clicked = Some(tile.key.clone());
                    }
                    if self.huddle.textures.contains_key(&tile.key) {
                        drawn.insert(tile.key.clone(), side * ppp);
                    }
                    x += side + s.gap;
                }
                y += side + s.gap;
            }
        } else {
            // the large pictures on top, the other tiles in a strip under them
            let rest: Vec<&Tile> = tiles
                .iter()
                .filter(|t| focus.as_ref() != Some(&t.key) || !screens.is_empty())
                .collect();
            let strip_h = if rest.is_empty() {
                0.0
            } else {
                s.strip + s.gap
            };
            let label_h = if screens.is_empty() { 0.0 } else { 18.0 };
            let each_h = ((inner.height() - strip_h) / big.len() as f32 - label_h).max(40.0);
            let mut y = inner.top();
            for (key, name, screen) in &big {
                let Some(tex) = self.huddle.textures.get(key) else {
                    continue;
                };
                let size = tex.size_vec2();
                let aspect = size.y / size.x.max(1.0);
                let mine = *screen && screen_owner(key) == Some(ME);
                // your own screen smaller: it is only a check of what the
                // others see (and a shared screen that shows this window
                // repeats inside itself)
                let most = if mine {
                    inner.width().min(480.0)
                } else {
                    inner.width()
                };
                let w = most.min(each_h / aspect.max(0.01));
                let r =
                    Rect::from_min_size(pos2(inner.center().x - w / 2.0, y), vec2(w, w * aspect));
                drawn.insert(key.clone(), w * ppp);
                if *screen {
                    let img = ui.put(r, egui::Image::new((tex.id(), r.size())).corner_radius(8));
                    let access = if mine {
                        "Your screen".to_string()
                    } else {
                        format!("Screen of {}", name)
                    };
                    img.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Image, true, access.clone())
                    });
                    let label = Self::screen_label(key, name);
                    ui.painter().text(
                        pos2(inner.center().x, r.bottom() + 3.0),
                        Align2::CENTER_TOP,
                        &label,
                        FontId::proportional(12.0),
                        TEXT,
                    );
                    let lr = Rect::from_center_size(
                        pos2(inner.center().x, r.bottom() + 10.0),
                        vec2(w, 14.0),
                    );
                    ui.interact(lr, ui.id().with(("screen_label", key)), Sense::hover())
                        .widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label.clone())
                        });
                } else if let Some(tile) = tiles.iter().find(|t| &t.key == key) {
                    if self.huddle_tile(ui, tile, r, s, true) {
                        clicked = Some(key.clone());
                    }
                }
                y = r.bottom() + label_h + s.gap;
            }
            if !rest.is_empty() {
                let side = s.strip;
                let fit = ((inner.width() + s.gap) / (side + s.gap)).floor().max(1.0) as usize;
                let shown = rest.len().min(fit);
                let row_w = shown as f32 * side + (shown.saturating_sub(1)) as f32 * s.gap;
                let mut x = inner.center().x - row_w / 2.0;
                let y = inner.bottom() - side;
                for tile in rest.iter().take(shown) {
                    let r = Rect::from_min_size(pos2(x, y), vec2(side, side));
                    if self.huddle_tile(ui, tile, r, s, false) {
                        clicked = Some(tile.key.clone());
                    }
                    if self.huddle.textures.contains_key(&tile.key) {
                        drawn.insert(tile.key.clone(), side * ppp);
                    }
                    x += side + s.gap;
                }
            }
        }
        if self.huddle.screen.is_some() {
            self.sharing_pill(ui, stage, s);
        }
        if let Some(key) = clicked {
            self.huddle.focus = if self.huddle.focus.as_ref() == Some(&key) {
                None
            } else {
                Some(key)
            };
        }
        drawn.remove(ME);
        drawn.remove(&screen_key(ME));
        self.tell_view_sizes(drawn);
    }

    /// How tall the stage is: room around the tiles, or as much as there is
    /// for a large picture.
    fn stage_height(
        &self,
        tiles: &[Tile],
        screens: &[(String, String)],
        width: f32,
        max_h: f32,
        s: Sizes,
    ) -> f32 {
        let floor = s.strip + 2.0 * s.pad;
        if !screens.is_empty() || self.huddle.focus.is_some() {
            // the picture's own shape decides, up to what there is
            let key = screens
                .first()
                .map(|(k, _)| k.clone())
                .or(self.huddle.focus.clone())
                .unwrap_or_default();
            let aspect = self
                .huddle
                .textures
                .get(&key)
                .map(|t| t.size_vec2().y / t.size_vec2().x.max(1.0))
                .unwrap_or(0.5625);
            let pic_w = width - 2.0 * s.pad;
            let extra = if screens.is_empty() { 0.0 } else { 18.0 };
            let want = pic_w * aspect * screens.len().max(1) as f32
                + extra
                + s.strip
                + s.gap
                + 2.0 * s.pad;
            return want.min(max_h).max(floor);
        }
        let inner = vec2(width - 2.0 * s.pad, (max_h - 2.0 * s.pad).max(s.strip));
        let (cols, side) = grid(tiles.len(), inner, s);
        let rows = tiles.len().div_ceil(cols.max(1)).max(1);
        let tiles_h = rows as f32 * side + (rows - 1) as f32 * s.gap;
        // some stage above and below the tiles, as in a room
        (tiles_h + 2.0 * s.pad + side * 0.35).min(max_h).max(floor)
    }

    /// One tile at `r`: the person's picture (cut to the tile, or whole when
    /// `whole`) or their initial on their color, the green ring while they
    /// speak, the crossed-out microphone while muted and their name on
    /// hover. True when it was clicked (pictures only: that makes it larger).
    fn huddle_tile(&self, ui: &mut Ui, tile: &Tile, r: Rect, s: Sizes, whole: bool) -> bool {
        let radius = (r.width() * 0.06).clamp(4.0, 8.0);
        let id = ui.id().with(("huddle_tile", &tile.key));
        let tex = self.huddle.textures.get(&tile.key);
        let resp = ui.interact(
            r,
            id,
            if tex.is_some() {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        let p = ui.painter();
        let (speaking, muted) = tile
            .person
            .as_ref()
            .map(|p| (p.speaking && !p.muted, p.muted))
            .unwrap_or((false, self.huddle.muted));
        let me = tile.key == ME;
        let shown_name = if me && tile.person.is_none() {
            tr("You").to_string()
        } else {
            tile.name.clone()
        };
        match tex {
            Some(tex) => {
                let uv = if whole {
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
                } else {
                    cover_uv(tex.size_vec2(), r.size())
                };
                egui::Image::new((tex.id(), r.size()))
                    .uv(uv)
                    .corner_radius(radius)
                    .paint_at(ui, r);
            }
            None => {
                let color = tile
                    .person
                    .as_ref()
                    .and_then(|p| p.member())
                    .and_then(|m| self.data.member(m))
                    .map(|m| rgb(m.color))
                    .unwrap_or(Color32::from_gray(0x6b));
                p.rect_filled(r, radius, color);
                p.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    initial(&shown_name),
                    FontId::proportional((r.width() * 0.34).max(14.0)),
                    Color32::WHITE,
                );
            }
        }
        if speaking {
            p.rect_stroke(
                r,
                radius,
                Stroke::new(3.0_f32, SPEAKING),
                egui::StrokeKind::Inside,
            );
        }
        let small = r.width() < 90.0;
        let badge = if small { 20.0 } else { 26.0 };
        let mut x = r.left() + if small { 4.0 } else { 8.0 };
        let bottom = r.bottom() - if small { 4.0 } else { 8.0 };
        if muted {
            let b = Rect::from_min_size(pos2(x, bottom - badge), vec2(badge, badge));
            p.rect_filled(b, 6.0, Color32::from_black_alpha(150));
            paint_slashed(
                p,
                Icon::Mic,
                b.center(),
                Color32::WHITE,
                Color32::from_black_alpha(0),
                if small { 0.75 } else { 0.9 },
            );
            x = b.right() + 4.0;
        }
        if resp.hovered() && !small {
            let g = p.layout_no_wrap(
                shown_name.clone(),
                FontId::proportional(12.0),
                Color32::WHITE,
            );
            let w = g.size().x.min(r.right() - x - 8.0);
            if w > 12.0 {
                let chip = Rect::from_min_size(pos2(x, bottom - 22.0), vec2(w + 12.0, 22.0));
                p.rect_filled(chip, 6.0, Color32::from_black_alpha(160));
                p.with_clip_rect(chip.shrink2(vec2(6.0, 0.0))).galley(
                    pos2(chip.left() + 6.0, chip.center().y - g.size().y / 2.0),
                    g,
                    Color32::WHITE,
                );
            }
        }
        // a raised hand in the top left corner
        let hand = tile.person.as_ref().is_some_and(|p| p.hand) || (me && self.huddle.hand);
        if hand {
            let side = if small { 22.0 } else { 30.0 };
            let at = r.min + vec2(s.gap.min(6.0), s.gap.min(6.0));
            let b = Rect::from_min_size(at, vec2(side, side));
            p.rect_filled(b, 7.0, Color32::WHITE);
            p.text(
                b.center(),
                Align2::CENTER_CENTER,
                HAND,
                FontId::proportional(side * 0.6),
                Color32::WHITE,
            );
        }
        // reactions rise from the bottom of the tile and fade
        let now = std::time::Instant::now();
        for (n, (_, emoji, at)) in self
            .huddle
            .reactions
            .iter()
            .filter(|(who, _, _)| who == &tile.key)
            .enumerate()
        {
            let t = now.duration_since(*at).as_secs_f32() / REACTION_SECS;
            if t >= 1.0 {
                continue;
            }
            let size = (r.width() * 0.28).clamp(18.0, 40.0);
            // the first in the middle, more beside it
            let x = r.center().x + [0.0, -0.7, 0.7][n % 3] * size;
            let y = r.bottom() - size * 0.6 - t * r.height() * 0.6;
            let fade = if t > 0.7 { 1.0 - (t - 0.7) / 0.3 } else { 1.0 };
            p.text(
                pos2(x, y),
                Align2::CENTER_CENTER,
                emoji,
                FontId::proportional(size),
                Color32::WHITE.gamma_multiply(fade),
            );
        }
        let state = if muted {
            "muted"
        } else if speaking {
            "speaking"
        } else {
            "listening"
        };
        let raised = if hand { ", hand raised" } else { "" };
        let access = if tex.is_some() {
            format!(
                "Video of {}{}",
                if me { "You" } else { tile.name.as_str() },
                raised
            )
        } else if me {
            format!("{} (you), {}{}", tile.name, state, raised)
        } else {
            format!("{}, {}{}", tile.name, state, raised)
        };
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, &access));
        let tip = if me {
            trf!("{} (you), {}", tile.name, tr(state))
        } else {
            format!("{}, {}", tile.name, tr(state))
        };
        let tip = if hand {
            format!("{}, {}", tip, tr("hand raised"))
        } else {
            tip
        };
        let tip = match tex {
            Some(_) if self.huddle.focus.as_ref() == Some(&tile.key) => {
                format!("{}\n{}", tip, tr("Click to make smaller"))
            }
            Some(_) => format!("{}\n{}", tip, tr("Click to make larger")),
            None => tip,
        };
        resp.on_hover_text(tip).clicked()
    }

    /// "Sharing your screen" in the stage's top left corner.
    fn sharing_pill(&self, ui: &mut Ui, stage: Rect, s: Sizes) {
        let text = tr("Sharing your screen");
        let p = ui.painter();
        let g = p.layout_no_wrap(text.to_string(), FontId::proportional(12.0), Color32::WHITE);
        let w = (g.size().x + 30.0).min(stage.width() - 2.0 * s.gap);
        let pill = Rect::from_min_size(stage.min + vec2(s.gap, s.gap), vec2(w, 24.0));
        p.rect_filled(pill, 12.0, Color32::from_rgb(0x00, 0x7a, 0x4d));
        p.circle_filled(
            pos2(pill.left() + 12.0, pill.center().y),
            3.5,
            Color32::WHITE,
        );
        p.with_clip_rect(pill.shrink(2.0)).galley(
            pos2(pill.left() + 22.0, pill.center().y - g.size().y / 2.0),
            g,
            Color32::WHITE,
        );
        ui.interact(pill, ui.id().with("sharing_pill"), Sense::hover())
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    }

    /// The buttons under the stage: microphone and camera (each with an
    /// arrow that picks the device), screen, hand, reactions, the
    /// conversation (`chat`: not in the small window, which shows it) and
    /// Leave, in the middle of `width`, on more rows when they don't fit.
    /// What was pressed goes in `press`.
    pub(in crate::app) fn huddle_toolbar(
        &self,
        ui: &mut Ui,
        width: f32,
        s: Sizes,
        chat: bool,
        press: &mut HuddlePress,
    ) {
        let menu = self.huddle.menu;
        let mut anchor: Option<Rect> = None;
        for row in toolbar_rows(&items(chat), width, s) {
            let row_w: f32 =
                row.iter().map(|i| s.item_w(*i)).sum::<f32>() + (row.len() - 1) as f32 * s.gap;
            let (line, _) = ui.allocate_exact_size(vec2(width, s.button.y), Sense::hover());
            let mut x = line.center().x - row_w / 2.0;
            for item in row {
                let r = Rect::from_min_size(pos2(x, line.top()), vec2(s.item_w(item), s.button.y));
                x += r.width() + s.gap;
                self.toolbar_item(ui, item, r, s, press);
                let opens = match item {
                    Item::Mic => Some(HuddleMenu::Devices),
                    Item::Camera => Some(HuddleMenu::Cameras),
                    Item::React => Some(HuddleMenu::Reactions),
                    _ => None,
                };
                if opens.is_some() && opens == menu {
                    anchor = Some(r);
                }
            }
        }
        if let (Some(m), Some(r)) = (menu, anchor) {
            self.huddle_menu(ui.ctx(), m, r, press);
        }
    }

    /// How tall the buttons are for `width` (more rows when narrow).
    pub(in crate::app) fn toolbar_height(width: f32, s: Sizes, chat: bool) -> f32 {
        let rows = toolbar_rows(&items(chat), width, s).len().max(1) as f32;
        rows * s.button.y + (rows - 1.0) * s.gap
    }

    fn toolbar_item(&self, ui: &mut Ui, item: Item, r: Rect, s: Sizes, press: &mut HuddlePress) {
        let toggle = |m: HuddleMenu| {
            Some(if self.huddle.menu == Some(m) {
                None
            } else {
                Some(m)
            })
        };
        match item {
            Item::Mic => {
                let muted = self.huddle.muted;
                let (main, arrow) = split_button(
                    ui,
                    r,
                    s,
                    !muted,
                    self.huddle.menu == Some(HuddleMenu::Devices),
                    |p, c, col, bg| {
                        if muted {
                            paint_slashed(p, Icon::Mic, c, col, bg, 1.15)
                        } else {
                            line_icon_scaled(p, Icon::Mic, c, col, 1.15)
                        }
                    },
                    if muted { "Unmute" } else { "Mute" },
                    if muted { tr("Unmute") } else { tr("Mute") },
                    "Microphone and speaker",
                    tr("Microphone and speaker"),
                );
                if main {
                    press.mute = Some(!muted);
                }
                if arrow {
                    press.menu = toggle(HuddleMenu::Devices);
                }
            }
            Item::Camera => {
                let cam = self.huddle.camera;
                let (main, arrow) = split_button(
                    ui,
                    r,
                    s,
                    cam,
                    self.huddle.menu == Some(HuddleMenu::Cameras),
                    |p, c, col, bg| {
                        if cam {
                            line_icon_scaled(p, Icon::Video, c, col, 1.15)
                        } else {
                            paint_slashed(p, Icon::Video, c, col, bg, 1.15)
                        }
                    },
                    if cam { "Stop video" } else { "Video" },
                    if cam { tr("Stop video") } else { tr("Video") },
                    "Choose a camera",
                    tr("Choose a camera"),
                );
                if main {
                    press.camera = Some(!cam);
                }
                if arrow {
                    press.menu = toggle(HuddleMenu::Cameras);
                }
            }
            Item::Screen => {
                let sharing = self.huddle.screen.is_some();
                if tool_button(
                    ui,
                    r,
                    sharing,
                    |p, c, col, _| paint_screen(p, c, col),
                    if sharing {
                        "Stop sharing screen"
                    } else {
                        "Share screen"
                    },
                    if sharing {
                        tr("Stop sharing screen")
                    } else {
                        tr("Share screen")
                    },
                ) {
                    press.share = true;
                }
            }
            Item::Hand => {
                let up = self.huddle.hand;
                let size = s.button.y * 0.48;
                if tool_button(
                    ui,
                    r,
                    up,
                    |p, c, _, _| {
                        p.text(
                            c,
                            Align2::CENTER_CENTER,
                            HAND,
                            FontId::proportional(size),
                            Color32::WHITE,
                        );
                    },
                    if up { "Lower hand" } else { "Raise hand" },
                    if up {
                        tr("Lower hand")
                    } else {
                        tr("Raise hand")
                    },
                ) {
                    press.hand = Some(!up);
                }
            }
            Item::React => {
                let open = self.huddle.menu == Some(HuddleMenu::Reactions);
                if tool_button(
                    ui,
                    r,
                    open,
                    |p, c, col, _| line_icon_scaled(p, Icon::Smile, c, col, 1.15),
                    "Reactions",
                    tr("Reactions"),
                ) {
                    press.menu = toggle(HuddleMenu::Reactions);
                }
            }
            Item::Chat => {
                if tool_button(
                    ui,
                    r,
                    false,
                    |p, c, col, _| line_icon_scaled(p, Icon::Chat, c, col, 1.1),
                    "Open the huddle's conversation",
                    tr("Open the conversation"),
                ) {
                    press.open_chat = true;
                }
            }
            Item::Leave => {
                let resp = ui.interact(r, ui.id().with("huddle_leave"), Sense::click());
                let fill = if resp.hovered() {
                    Color32::from_rgb(0xd8, 0x4b, 0x70)
                } else {
                    LEAVE_RED
                };
                ui.painter().rect_filled(r, 9.0, fill);
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    tr("Leave"),
                    FontId::proportional(if s.button.y < 38.0 { 13.0 } else { 14.0 }),
                    Color32::WHITE,
                );
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Leave")
                });
                if resp.clicked() {
                    press.leave = true;
                }
            }
        }
    }

    /// The menu over the button at `at`: the microphones and speakers, the
    /// cameras, or the reactions. A click outside it (or Esc) closes it.
    fn huddle_menu(&self, ctx: &egui::Context, m: HuddleMenu, at: Rect, press: &mut HuddlePress) {
        let screen = ctx.screen_rect();
        // the reactions in one row; the device lists a little narrower
        let want = if m == HuddleMenu::Reactions {
            REACTIONS.len() as f32 * 38.0
        } else {
            260.0
        };
        let width = want.min(screen.width() - 32.0);
        let x = (at.center().x - width / 2.0)
            .min(screen.right() - width - 8.0)
            .max(screen.left() + 8.0);
        let shown = egui::Area::new(egui::Id::new("huddle_menu"))
            .order(egui::Order::Foreground)
            .pivot(Align2::LEFT_BOTTOM)
            .fixed_pos(pos2(x, at.top() - 6.0))
            .constrain(true)
            .show(ctx, |ui| {
                panel_frame(10, 8)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| match m {
                        HuddleMenu::Reactions => {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                                ui.set_max_width(width);
                                for e in REACTIONS {
                                    let (r, resp) =
                                        ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
                                    if resp.hovered() {
                                        ui.painter().rect_filled(
                                            r,
                                            8.0,
                                            Color32::from_white_alpha(28),
                                        );
                                    }
                                    ui.painter().text(
                                        r.center(),
                                        Align2::CENTER_CENTER,
                                        e,
                                        FontId::proportional(22.0),
                                        Color32::WHITE,
                                    );
                                    resp.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::WidgetType::Button,
                                            true,
                                            format!("React {}", e),
                                        )
                                    });
                                    if resp.clicked() {
                                        press.react = Some(e.to_string());
                                        press.menu = Some(None);
                                    }
                                }
                            });
                        }
                        HuddleMenu::Devices => {
                            ui.set_width(width - 16.0);
                            menu_heading(ui, tr("Microphone"));
                            let chosen = self.huddle.mic.clone().or(self
                                .huddle
                                .mics
                                .first()
                                .map(|d| d.id.clone()));
                            if self.huddle.mics.is_empty() {
                                menu_note(ui, tr("No microphones found"));
                            }
                            for d in &self.huddle.mics {
                                if menu_row(ui, &d.name, chosen.as_ref() == Some(&d.id)) {
                                    press.mic = Some(d.id.clone());
                                    press.menu = Some(None);
                                }
                            }
                            ui.add_space(4.0);
                            menu_heading(ui, tr("Speaker"));
                            let chosen = self.huddle.speaker.clone().or(self
                                .huddle
                                .speakers
                                .first()
                                .map(|d| d.id.clone()));
                            if self.huddle.speakers.is_empty() {
                                menu_note(ui, tr("No speakers found"));
                            }
                            for d in &self.huddle.speakers {
                                if menu_row(ui, &d.name, chosen.as_ref() == Some(&d.id)) {
                                    press.speaker = Some(d.id.clone());
                                    press.menu = Some(None);
                                }
                            }
                        }
                        HuddleMenu::Cameras => {
                            ui.set_width(width - 16.0);
                            menu_heading(ui, tr("Camera"));
                            let chosen = self.huddle.camera_pick.clone().or(self
                                .huddle
                                .cameras
                                .first()
                                .cloned());
                            if self.huddle.cameras.is_empty() {
                                menu_note(ui, tr("No cameras found"));
                            }
                            for c in &self.huddle.cameras {
                                if menu_row(ui, c, chosen.as_ref() == Some(c)) {
                                    press.camera_pick = Some(c.clone());
                                    press.menu = Some(None);
                                }
                            }
                        }
                    });
            });
        let area = shown.response.rect;
        let outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !area.contains(p) && !at.contains(p))
        });
        if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            press.menu.get_or_insert(None);
        }
    }
}

/// How many columns and how large the tiles are so `n` of them fit in
/// `room` as large as they can be.
fn grid(n: usize, room: Vec2, s: Sizes) -> (usize, f32) {
    let n = n.max(1);
    let mut best = (1, 0.0_f32);
    for cols in 1..=n {
        let rows = n.div_ceil(cols);
        let w = (room.x - (cols - 1) as f32 * s.gap) / cols as f32;
        let h = (room.y - (rows - 1) as f32 * s.gap) / rows as f32;
        let side = w.min(h).min(s.tile_max);
        // as large as can be; among equally large, in fewer rows (side by
        // side, as people sit in a call)
        if side > best.1 + 0.5 {
            best = (cols, side);
        } else if side >= best.1 - 0.5 {
            best.0 = cols;
        }
    }
    (best.0, best.1.max(24.0))
}

/// The part of a picture of `pic` size that covers a tile of `tile` size
/// (the middle, cut on the long side).
fn cover_uv(pic: Vec2, tile: Vec2) -> Rect {
    let pa = pic.x / pic.y.max(1.0);
    let ta = tile.x / tile.y.max(1.0);
    if pa > ta {
        let w = ta / pa;
        Rect::from_min_max(pos2(0.5 - w / 2.0, 0.0), pos2(0.5 + w / 2.0, 1.0))
    } else {
        let h = pa / ta;
        Rect::from_min_max(pos2(0.0, 0.5 - h / 2.0), pos2(1.0, 0.5 + h / 2.0))
    }
}

fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default()
}

/// One of the buttons under the stage.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Item {
    Mic,
    Camera,
    Screen,
    Hand,
    React,
    Chat,
    Leave,
}

fn items(chat: bool) -> Vec<Item> {
    let mut v = vec![
        Item::Mic,
        Item::Camera,
        Item::Screen,
        Item::Hand,
        Item::React,
    ];
    if chat {
        v.push(Item::Chat);
    }
    v.push(Item::Leave);
    v
}

impl Sizes {
    /// The arrow beside the microphone and the camera.
    fn arrow_w(&self) -> f32 {
        (self.button.x * 0.55).round()
    }

    fn item_w(&self, i: Item) -> f32 {
        match i {
            Item::Mic | Item::Camera => self.button.x + self.arrow_w(),
            Item::Leave => self.leave_w,
            _ => self.button.x,
        }
    }
}

/// The buttons in rows that fit `width`: one row when they all fit, else
/// split as evenly as can be.
fn toolbar_rows(items: &[Item], width: f32, s: Sizes) -> Vec<Vec<Item>> {
    let total = |v: &[Item]| -> f32 {
        v.iter().map(|i| s.item_w(*i)).sum::<f32>() + v.len().saturating_sub(1) as f32 * s.gap
    };
    for rows in 1..=items.len() {
        let per = items.len().div_ceil(rows);
        let split: Vec<Vec<Item>> = items.chunks(per).map(|c| c.to_vec()).collect();
        if split.iter().all(|r| total(r) <= width) {
            return split;
        }
    }
    items.iter().map(|i| vec![*i]).collect()
}

/// The microphone's or camera's button: the main part toggles it, the arrow
/// beside it opens its menu (`open`: shown open). (main clicked, arrow clicked)
#[allow(clippy::too_many_arguments)]
fn split_button(
    ui: &mut Ui,
    r: Rect,
    s: Sizes,
    on: bool,
    open: bool,
    paint: impl Fn(&egui::Painter, Pos2, Color32, Color32),
    access: &str,
    tip: &str,
    arrow_access: &str,
    arrow_tip: &str,
) -> (bool, bool) {
    let split = r.right() - s.arrow_w();
    let main_r = Rect::from_min_max(r.min, pos2(split, r.bottom()));
    let arrow_r = Rect::from_min_max(pos2(split, r.top()), r.max);
    let main = ui.interact(
        main_r,
        ui.id().with(("huddle_button", access)),
        Sense::click(),
    );
    let arrow = ui.interact(
        arrow_r,
        ui.id().with(("huddle_arrow", arrow_access)),
        Sense::click(),
    );
    let (fill, icon) = colors(on, false);
    ui.painter().rect_filled(r, 9.0, fill);
    // each half lights up on its own
    for (half, resp, corners) in [
        (
            main_r,
            &main,
            egui::CornerRadius {
                nw: 9,
                sw: 9,
                ne: 0,
                se: 0,
            },
        ),
        (
            arrow_r,
            &arrow,
            egui::CornerRadius {
                nw: 0,
                sw: 0,
                ne: 9,
                se: 9,
            },
        ),
    ] {
        if resp.hovered() || (open && half == arrow_r) {
            ui.painter().rect_filled(half, corners, colors(on, true).0);
        }
    }
    paint(ui.painter(), main_r.center() + vec2(2.0, 0.0), icon, fill);
    let line = icon.gamma_multiply(0.35);
    ui.painter().line_segment(
        [pos2(split, r.top() + 10.0), pos2(split, r.bottom() - 10.0)],
        Stroke::new(1.0_f32, line),
    );
    // the arrow points up: the menu opens above
    let c = arrow_r.center() - vec2(1.0, 0.0);
    let st = Stroke::new(1.6_f32, icon);
    ui.painter()
        .line_segment([c + vec2(-4.0, 2.0), c + vec2(0.0, -2.0)], st);
    ui.painter()
        .line_segment([c + vec2(0.0, -2.0), c + vec2(4.0, 2.0)], st);
    main.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, access));
    arrow.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, arrow_access));
    let m = main.on_hover_text(tip).clicked();
    let a = arrow.on_hover_text(arrow_tip).clicked();
    (m, a)
}

/// A button's fill and icon colors: light while on, dark while off, a
/// little lighter under the pointer.
fn colors(on: bool, hover: bool) -> (Color32, Color32) {
    match (on, hover) {
        (true, false) => (BUTTON_ON, Color32::from_rgb(0x1d, 0x1c, 0x1d)),
        (true, true) => (Color32::WHITE, Color32::from_rgb(0x1d, 0x1c, 0x1d)),
        (false, false) => (BUTTON_OFF, Color32::WHITE),
        (false, true) => (Color32::from_rgb(0x4c, 0x4e, 0x54), Color32::WHITE),
    }
}

fn menu_heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().strong().color(TEXT2));
}

fn menu_note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(TEXT2));
}

/// A choice in a huddle menu, checked when it is the one in use. True when
/// clicked.
fn menu_row(ui: &mut Ui, name: &str, chosen: bool) -> bool {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(r, 6.0, Color32::from_white_alpha(24));
    }
    if chosen {
        super::super::icons::line_icon(
            ui.painter(),
            Icon::Check,
            pos2(r.left() + 13.0, r.center().y),
            TEXT,
        );
    }
    let text = r.with_min_x(r.left() + 28.0).shrink2(vec2(0.0, 2.0));
    // a long name ends in "…" (all of it on hover)
    let mut job = egui::text::LayoutJob::simple_singleline(
        name.to_string(),
        FontId::proportional(13.0),
        TEXT,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(r.right() - 6.0 - text.left());
    let g = ui.fonts(|f| f.layout_job(job));
    ui.painter()
        .galley(pos2(text.left(), r.center().y - g.size().y / 2.0), g, TEXT);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, chosen, name)
    });
    resp.on_hover_text(name).clicked()
}

/// A square button of the row: light while `on`, dark while off. True when
/// clicked. `paint` draws its icon at a centre in a color, given the fill.
fn tool_button(
    ui: &mut Ui,
    r: Rect,
    on: bool,
    paint: impl Fn(&egui::Painter, Pos2, Color32, Color32),
    access: &str,
    tip: &str,
) -> bool {
    let resp = ui.interact(r, ui.id().with(("huddle_button", access)), Sense::click());
    let (fill, icon) = colors(on, resp.hovered());
    ui.painter().rect_filled(r, 9.0, fill);
    paint(ui.painter(), r.center(), icon, fill);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, access));
    resp.on_hover_text(tip).clicked()
}

/// An icon crossed out (microphone off, camera off): a line over it, with
/// a gap in `bg` so it reads on the icon.
fn paint_slashed(p: &egui::Painter, kind: Icon, c: Pos2, color: Color32, bg: Color32, scale: f32) {
    line_icon_scaled(p, kind, c, color, scale);
    let a = c + vec2(-7.0, -7.0) * scale;
    let b = c + vec2(7.0, 7.0) * scale;
    if bg.a() > 0 {
        p.line_segment(
            [a + vec2(1.6, -1.6), b + vec2(1.6, -1.6)],
            Stroke::new(2.0_f32, bg),
        );
    }
    p.line_segment([a, b], Stroke::new(1.6_f32, color));
}

/// A screen on its stand (share screen).
fn paint_screen(p: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.7_f32, color);
    p.rect_stroke(
        Rect::from_center_size(c + vec2(0.0, -2.0), vec2(18.0, 12.5)),
        2.0,
        s,
        egui::StrokeKind::Middle,
    );
    p.line_segment([c + vec2(0.0, 4.5), c + vec2(0.0, 7.5)], s);
    p.line_segment([c + vec2(-4.5, 8.0), c + vec2(4.5, 8.0)], s);
}

/// Headphones (the huddle's sign in its title).
fn paint_headphones(p: &egui::Painter, c: Pos2, color: Color32) {
    use std::f32::consts::PI;
    let s = Stroke::new(1.6_f32, color);
    let pts: Vec<Pos2> = (0..=16)
        .map(|i| {
            let a = PI + PI * i as f32 / 16.0;
            c + vec2(a.cos() * 6.5, a.sin() * 6.5 + 1.0)
        })
        .collect();
    p.add(egui::Shape::line(pts, s));
    for side in [-1.0_f32, 1.0] {
        let r = Rect::from_center_size(c + vec2(side * 5.5, 4.5), vec2(3.5, 6.5));
        p.rect_filled(r, 1.5, color);
    }
}
