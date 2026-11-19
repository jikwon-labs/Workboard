// Edit profile: my full name, display name, title and photo (and, as
// before, my email and avatar colour).
//
// Others see the display name before the full name (`Member::shown`). The
// photo is cropped to its middle square, shrunk to PHOTO_PX and goes through
// the same file cache and server as a chat picture; the member keeps it as an
// attachment, so the cache cleanup, sharing and the server's file cleanup
// count it as in use. Every frame the app asks for the members' photos and
// puts the ones decoded into egui's temp data, where `paint_avatar` finds
// them; until then (and without a photo) avatars show initials.

use super::*;
use crate::media;
use crate::widgets::{AvatarTextures, AVATAR_TEXTURES};
use std::path::Path;

/// Side of the square photo kept, in pixels.
pub(super) const PHOTO_PX: u32 = 512;
/// Size the photos are decoded at for avatars.
pub(super) const AVATAR_PX: u32 = 256;
/// Side of the photo in the Edit profile dialog.
const PREVIEW: f32 = 168.0;

/// The picture at `path` cut to its middle square and shrunk to PHOTO_PX
/// (when larger), as PNG.
pub(super) fn square_png(path: &Path) -> Result<Vec<u8>, String> {
    let img = match media::thumbnail(path, MediaKind::Image, PHOTO_PX * 4) {
        Some(px) => {
            // the decoder gives premultiplied pixels; PNG keeps straight ones
            let mut raw = px.px;
            for c in raw.chunks_exact_mut(4) {
                let a = c[3] as u32;
                if a > 0 && a < 255 {
                    for v in &mut c[..3] {
                        *v = ((*v as u32 * 255 + a / 2) / a).min(255) as u8;
                    }
                }
            }
            image::RgbaImage::from_raw(px.w, px.h, raw)
        }
        None => image::open(path).ok().map(|i| i.to_rgba8()),
    }
    .ok_or_else(|| tr("That file isn't a picture this app can open.").to_string())?;
    let (w, h) = img.dimensions();
    let side = w.min(h);
    if side == 0 {
        return Err(tr("That file isn't a picture this app can open.").into());
    }
    let sq = image::imageops::crop_imm(&img, (w - side) / 2, (h - side) / 2, side, side).to_image();
    let out = if side > PHOTO_PX {
        image::imageops::resize(
            &sq,
            PHOTO_PX,
            PHOTO_PX,
            image::imageops::FilterType::Triangle,
        )
    } else {
        sq
    };
    let mut png = vec![];
    image::DynamicImage::ImageRgba8(out)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(png)
}

impl Modal {
    /// The Edit profile dialog, filled in from `m`.
    pub(super) fn profile(m: &Member) -> Modal {
        Modal::Profile {
            name: m.name.clone(),
            display: m.display_name.clone(),
            title: m.title.clone(),
            email: m.email.clone(),
            photo: m.photo.clone(),
        }
    }
}

impl App {
    /// Open Edit profile for me.
    pub(super) fn open_profile(&mut self) {
        let m = self.data.member(self.data.me).cloned().unwrap_or_default();
        self.profile_photo = None;
        self.modal = Some(Modal::profile(&m));
    }

    /// Upload photo: choose a picture and start getting it ready.
    pub(super) fn pick_profile_photo(&mut self) {
        if let Some(p) = media::pick_picture() {
            self.use_profile_photo(&p);
        }
    }

    /// Start getting the picture at `path` ready as my new photo: cropped
    /// and shrunk here, then checked, cached and sent like a chat picture.
    pub(super) fn use_profile_photo(&mut self, path: &Path) {
        match square_png(path) {
            Ok(png) => {
                let remote = self.media_remote();
                self.profile_photo = Some(self.files.add(None, Some(png), &remote));
            }
            Err(e) => self.toast(e),
        }
    }

    /// A photo got ready (or didn't): it goes up to the server, and is tried
    /// again later (and after a restart) while it can't.
    pub(super) fn profile_photo_prepared(
        &mut self,
        local: u64,
        result: &Result<Attachment, String>,
        remote: &files::Remote,
    ) {
        let Some(p) = self.profile_photo.as_mut().filter(|p| p.local == local) else {
            return;
        };
        match result {
            Ok(a) => {
                let mut a = a.clone();
                a.name = "Profile photo.png".into();
                self.files.upload(&a, remote);
                self.files.upload_soon(&a, remote);
                p.att = Some(a);
            }
            Err(e) => {
                p.error = Some(e.clone());
                self.toast(App::error_text(e));
            }
        }
    }

    /// Before the files tick: ask for every member's photo (and the one
    /// being chosen), decoded at avatar size.
    pub(super) fn want_profile_photos(&self) {
        let chosen = self.profile_photo.as_ref().and_then(|p| p.att.as_ref());
        for a in self
            .data
            .members
            .iter()
            .filter_map(|m| m.photo.as_ref())
            .chain(chosen)
        {
            self.files.want(a, AVATAR_PX);
        }
    }

    /// After the files tick: the photos decoded so far, for `paint_avatar`.
    pub(super) fn publish_profile_photos(&self, ctx: &egui::Context) {
        let chosen = self.profile_photo.as_ref().and_then(|p| p.att.as_ref());
        let map: std::collections::HashMap<String, egui::TextureId> = self
            .data
            .members
            .iter()
            .filter_map(|m| m.photo.as_ref())
            .chain(chosen)
            .filter_map(|a| {
                self.files
                    .texture(&a.file, AVATAR_PX)
                    .map(|t| (a.file.clone(), t.id()))
            })
            .collect();
        let map: AvatarTextures = std::sync::Arc::new(map);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(AVATAR_TEXTURES), map));
    }

    /// The Edit profile dialog. Returns true when it should close.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn profile_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        name: &mut String,
        display: &mut String,
        title: &mut String,
        email: &mut String,
        photo: &mut Option<Attachment>,
    ) -> bool {
        let mut close = false;
        let me = self.data.me;
        // a picture chosen here takes the photo's place once it's ready
        if let Some(a) = self.profile_photo.as_ref().and_then(|p| p.att.clone()) {
            if photo.as_ref().map(|p| &p.file) != Some(&a.file) {
                *photo = Some(a);
            }
        }
        let getting_ready = self
            .profile_photo
            .as_ref()
            .is_some_and(|p| p.att.is_none() && p.error.is_none());
        let help = |ui: &mut Ui, s: &str| {
            ui.label(RichText::new(s).small().color(t.text3));
        };
        let heading = |ui: &mut Ui, s: &str| {
            ui.label(RichText::new(s).strong().color(t.text));
        };
        ui.horizontal_top(|ui| {
            let left = (ui.available_width() - PREVIEW - 20.0).max(220.0);
            ui.vertical(|ui| {
                ui.set_width(left);
                heading(ui, tr("Full name"));
                let r = ui.add(field(name).desired_width(f32::INFINITY));
                name_field(&r, "Profile full name");
                ui.add_space(10.0);
                heading(ui, tr("Display name"));
                let r = ui.add(
                    field(display)
                        .hint_text(tr("Display name"))
                        .desired_width(f32::INFINITY),
                );
                name_field(&r, "Profile display name");
                help(
                    ui,
                    tr("This could be your first name or a nickname: what you'd like people to call you. Others see it before your full name."),
                );
                ui.add_space(10.0);
                let job = crate::i18n::tr_ctx("Title (job)", "Title");
                heading(ui, job);
                let r = ui.add(field(title).hint_text(job).desired_width(f32::INFINITY));
                name_field(&r, "Profile title");
                help(
                    ui,
                    &crate::i18n::particles(&trf!(
                        "Let people know what you do at {}.",
                        self.data.workspace
                    )),
                );
            });
            ui.add_space(20.0);
            ui.vertical(|ui| {
                ui.set_width(PREVIEW);
                heading(ui, tr("Profile photo"));
                let (r, resp) = ui.allocate_exact_size(vec2(PREVIEW, PREVIEW), Sense::hover());
                name_it(&resp, "Profile photo");
                let tex = photo
                    .as_ref()
                    .and_then(|a| self.files.texture(&a.file, AVATAR_PX))
                    .map(|t| t.id());
                let p = ui.painter();
                match tex {
                    Some(id) => {
                        p.image(
                            id,
                            r,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    None => {
                        let color = self.data.member(me).map(|m| m.color).unwrap_or([90, 150, 200]);
                        p.rect_filled(r, 6.0, rgb(color));
                        let mut who = self.data.member(me).cloned().unwrap_or_default();
                        who.name = name.clone();
                        who.display_name = display.clone();
                        p.text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            who.initials(),
                            FontId::proportional(PREVIEW * 0.36),
                            Color32::WHITE,
                        );
                    }
                }
                if getting_ready {
                    p.rect_filled(r, 6.0, Color32::from_black_alpha(90));
                    egui::Spinner::new().paint_at(ui, Rect::from_center_size(r.center(), vec2(22.0, 22.0)));
                }
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        vec2(PREVIEW, CONTROL_H),
                        egui::Button::new(tr("Upload photo")).corner_radius(6),
                    )
                    .clicked()
                {
                    self.pick_profile_photo();
                }
                if photo.is_some()
                    && ui
                        .add_sized(
                            vec2(PREVIEW, CONTROL_H),
                            ghost(egui::Button::new(
                                RichText::new(tr("Remove photo")).color(t.text2),
                            )),
                        )
                        .clicked()
                {
                    *photo = None;
                    self.profile_photo = None;
                }
            });
        });
        ui.add_space(12.0);
        heading(ui, tr("Email"));
        let r = ui.add(field(email).desired_width(f32::INFINITY));
        name_field(&r, "Profile email");
        ui.add_space(10.0);
        heading(ui, tr("Avatar color"));
        ui.label(
            RichText::new(tr("Shown when there's no photo."))
                .small()
                .color(t.text3),
        );
        ui.horizontal_wrapped(|ui| {
            let cur = self.data.member(me).map(|m| m.color);
            for (i, c) in STATUS_PALETTE.iter().skip(1).enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                name_it(&resp, &format!("Avatar color {}", i + 1));
                ui.painter().circle_filled(r.center(), 9.0, rgb(*c));
                hover_ring(ui, &resp, r.shrink(1.0), 10.0);
                if cur == Some(*c) {
                    ui.painter()
                        .circle_stroke(r.center(), 11.0, Stroke::new(1.5_f32, t.text));
                }
                if resp.clicked() {
                    if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me) {
                        m.color = *c;
                        self.dirty = true;
                    }
                }
            }
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let save = ui.add_enabled(
                    !name.trim().is_empty() && !getting_ready,
                    Tint(primary_btn(t, tr("Save changes"))),
                );
                if save.clicked() {
                    if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me) {
                        m.name = name.trim().to_string();
                        m.display_name = display.trim().to_string();
                        m.title = title.trim().to_string();
                        m.email = email.trim().to_string();
                        m.photo = photo.clone();
                    }
                    self.profile_photo = None;
                    self.dirty = true;
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    self.profile_photo = None;
                    close = true;
                }
            });
        });
        close
    }
}
