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

    /// How tall the row of buttons is.
    pub(in crate::app) fn buttons_h(&self) -> f32 {
        self.button.y
    }
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
        let _ = s;
        let state = if muted {
            "muted"
        } else if speaking {
            "speaking"
        } else {
            "listening"
        };
        let access = if tex.is_some() {
            format!("Video of {}", if me { "You" } else { tile.name.as_str() })
        } else if me {
            format!("{} (you), {}", tile.name, state)
        } else {
            format!("{}, {}", tile.name, state)
        };
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, &access));
        let tip = if me {
            trf!("{} (you), {}", tile.name, tr(state))
        } else {
            format!("{}, {}", tile.name, tr(state))
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

    /// The row of buttons under the stage: microphone, camera, screen, the
    /// conversation (`chat`: not in the small window, which shows it) and
    /// Leave, in the middle of `width`. What was pressed goes in `press`.
    pub(in crate::app) fn huddle_toolbar(
        &self,
        ui: &mut Ui,
        width: f32,
        s: Sizes,
        chat: bool,
        press: &mut HuddlePress,
    ) {
        let count = if chat { 4.0 } else { 3.0 };
        let row_w = count * s.button.x + s.leave_w + count * s.gap;
        let (row, _) = ui.allocate_exact_size(vec2(width, s.button.y), Sense::hover());
        let mut x = row.center().x - row_w.min(width) / 2.0;
        let mut next = |w: f32| {
            let r = Rect::from_min_size(pos2(x, row.top()), vec2(w, s.button.y));
            x += w + s.gap;
            r
        };
        let muted = self.huddle.muted;
        if tool_button(
            ui,
            next(s.button.x),
            !muted,
            |p, c, col, bg| {
                if muted {
                    paint_slashed(p, Icon::Mic, c, col, bg, 1.15)
                } else {
                    line_icon_scaled(p, Icon::Mic, c, col, 1.15)
                }
            },
            if muted { "Unmute" } else { "Mute" },
            if muted { tr("Unmute") } else { tr("Mute") },
        ) {
            press.mute = Some(!muted);
        }
        let cam = self.huddle.camera;
        if tool_button(
            ui,
            next(s.button.x),
            cam,
            |p, c, col, bg| {
                if cam {
                    line_icon_scaled(p, Icon::Video, c, col, 1.15)
                } else {
                    paint_slashed(p, Icon::Video, c, col, bg, 1.15)
                }
            },
            if cam { "Stop video" } else { "Video" },
            if cam { tr("Stop video") } else { tr("Video") },
        ) {
            press.camera = Some(!cam);
        }
        let sharing = self.huddle.screen.is_some();
        if tool_button(
            ui,
            next(s.button.x),
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
        if chat
            && tool_button(
                ui,
                next(s.button.x),
                false,
                |p, c, col, _| line_icon_scaled(p, Icon::Chat, c, col, 1.1),
                "Open the huddle's conversation",
                tr("Open the conversation"),
            )
        {
            press.open_chat = true;
        }
        let leave = next(s.leave_w);
        let r = ui.interact(leave, ui.id().with("huddle_leave"), Sense::click());
        let fill = if r.hovered() {
            Color32::from_rgb(0xd8, 0x4b, 0x70)
        } else {
            LEAVE_RED
        };
        ui.painter().rect_filled(leave, 9.0, fill);
        ui.painter().text(
            leave.center(),
            Align2::CENTER_CENTER,
            tr("Leave"),
            FontId::new(14.0, egui::FontFamily::Proportional),
            Color32::WHITE,
        );
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Leave"));
        if r.clicked() {
            press.leave = true;
        }
    }

    /// The width the row of buttons needs.
    pub(in crate::app) fn toolbar_width(s: Sizes, chat: bool) -> f32 {
        let count = if chat { 4.0 } else { 3.0 };
        count * s.button.x + s.leave_w + count * s.gap
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
    let (mut fill, icon) = if on {
        (BUTTON_ON, Color32::from_rgb(0x1d, 0x1c, 0x1d))
    } else {
        (BUTTON_OFF, Color32::WHITE)
    };
    if resp.hovered() {
        fill = if on {
            Color32::WHITE
        } else {
            Color32::from_rgb(0x4c, 0x4e, 0x54)
        };
    }
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
