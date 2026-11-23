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

use super::icons::{icon_button, icon_button_ex};
use super::*;
use crate::media;
use crate::widgets::{AvatarTextures, AVATAR_TEXTURES};
use std::path::Path;

/// Side of the square photo kept, in pixels.
pub(super) const PHOTO_PX: u32 = 512;
/// Size the photos are decoded at for avatars.
pub(super) const AVATAR_PX: u32 = 256;
/// Side of the photo in the Edit profile dialog.
const PREVIEW: f32 = 128.0;
/// Width of the name and title fields to its right.
const FIELDS_W: f32 = 340.0;

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

/// Width of someone's profile at the right of Chat and the Posts page.
const PROFILE_W: f32 = 340.0;

impl App {
    /// The profile opened in Chat or on the Posts page (`ChatUi::profile`),
    /// where a thread shows (see `profile_side`).
    pub(super) fn profile_panel(&mut self, ui: &mut Ui) {
        if let Some(id) = self.chat.profile {
            if !self.profile_side(ui, id) {
                self.chat.profile = None;
            }
        }
    }

    /// The profile opened from a name in People (`App::people_profile`), at
    /// the right of the page.
    pub(super) fn people_profile_panel(&mut self, ui: &mut Ui) {
        if let Some(id) = self.people_profile {
            if !self.profile_side(ui, id) {
                self.people_profile = None;
            }
        }
    }

    /// Someone's profile at the right: photo or initials, name, title,
    /// whether they're online, status, email and role, and a button to
    /// message them (Edit profile for me). False when it should close: its
    /// close button, Escape, or they're no longer in the workspace.
    fn profile_side(&mut self, ui: &mut Ui, id: u64) -> bool {
        let Some(m) = self.data.member(id).cloned() else {
            // no longer in the workspace
            return false;
        };
        let t = self.t();
        let mut close = false;
        let frame = egui::Frame::new()
            .fill(t.column_bg)
            .inner_margin(egui::Margin::symmetric(14, 10));
        if self.tier == super::shell::Tier::Compact {
            // one column: the profile is laid over the page instead of
            // beside it (beside it, it squeezed the page to a sliver whose
            // every line was cut)
            let over = ui.max_rect();
            egui::Area::new(egui::Id::new("profile_over"))
                .order(egui::Order::Middle)
                .fixed_pos(over.min)
                .constrain(false)
                .movable(false)
                .show(ui.ctx(), |ui| {
                    ui.set_clip_rect(over);
                    let mut inner = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(over)
                            .layout(Layout::top_down(Align::Min)),
                    );
                    inner.painter().rect_filled(over, 0.0, t.column_bg);
                    // clicks on it stay off the page under it
                    ui.allocate_rect(over, Sense::click());
                    frame.show(&mut inner, |ui| {
                        ui.set_min_size(over.size() - vec2(28.0, 20.0));
                        self.profile_side_body(ui, &m, &mut close);
                    });
                });
        } else {
            // in a small window it narrows to leave the page 340, as the
            // thread does
            let avail = ui.available_width();
            let w = (avail - 340.0).clamp(240.0, PROFILE_W);
            egui::SidePanel::right("profile")
                .exact_width(w)
                .resizable(false)
                .frame(frame)
                .show_inside(ui, |ui| self.profile_side_body(ui, &m, &mut close));
        }
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape))
            && self.modal.is_none()
            && !self.menu_was_open
            && ui.memory(|mm| mm.focused().is_none());
        !(close || escape)
    }

    /// What `profile_side` shows of `m`; `close` is set when it should close.
    fn profile_side_body(&mut self, ui: &mut Ui, m: &Member, close: &mut bool) {
        let t = self.t();
        let me = self.data.me;
        let id = m.id;
        ui.horizontal(|ui| {
            ui.set_height(30.0);
            ui.label(RichText::new(tr("Profile")).size(15.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, &t, Icon::Close, "", "Close profile", false)
                    .on_hover_text(tr("Close"))
                    .clicked()
                {
                    *close = true;
                }
            });
        });
        ui.add_space(18.0);
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(112.0, 112.0), Sense::hover());
            paint_avatar(ui.painter(), r.center(), 52.0, &m, t.column_bg);
            ui.add_space(10.0);
            ui.label(RichText::new(m.shown()).size(20.0).strong().color(t.text));
            // the full name too when they go by a display name
            if m.shown() != m.name && !m.name.trim().is_empty() {
                ui.label(RichText::new(&m.name).color(t.text2));
            }
            if !m.title.trim().is_empty() {
                ui.label(RichText::new(m.title.trim()).color(t.text2));
            }
            if m.invited {
                ui.label(RichText::new(tr("Invited")).small().color(t.text3));
            } else if let Some(on) = self.is_online(id) {
                ui.horizontal(|ui| {
                    let label = tr(if on { "Online" } else { "Offline" });
                    let w = 14.0
                        + ui.fonts(|f| {
                            f.layout_no_wrap(label.to_string(), FontId::proportional(13.0), t.text2)
                                .size()
                                .x
                        });
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    let (d, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(
                        d.center(),
                        4.0,
                        if on {
                            Color32::from_rgb(0x2e, 0xa0, 0x43)
                        } else {
                            t.text3
                        },
                    );
                    ui.label(RichText::new(label).size(13.0).color(t.text2));
                });
            }
            if !m.status_text.trim().is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(m.status_text.trim()).color(t.text));
            }
        });
        ui.add_space(14.0);
        ui.separator();
        ui.add_space(6.0);
        for (label, value) in [
            ("Email", m.email.trim().to_string()),
            ("Role", super::menus::role_text(m.role.name()).to_string()),
        ] {
            if value.is_empty() {
                continue;
            }
            ui.label(RichText::new(tr(label)).small().color(t.text3));
            ui.add(egui::Label::new(RichText::new(value).color(t.text)).selectable(true));
            ui.add_space(8.0);
        }
        ui.add_space(6.0);
        let w = ui.available_width();
        if id == me {
            if icon_button_ex(
                ui,
                &t,
                Icon::Edit,
                tr("Edit profile"),
                "Edit profile",
                true,
                w,
                t.text,
                t.text2,
            )
            .clicked()
            {
                self.open_profile();
            }
        } else if !m.invited
            && icon_button_ex(
                ui,
                &t,
                Icon::Chat,
                tr("Message"),
                &format!("Message {}", m.shown()),
                true,
                w,
                t.text,
                t.text2,
            )
            .clicked()
        {
            // the conversation with them, given the room
            self.section = Section::Chat;
            self.apply_chat(vec![super::chat::ChatAction::Dm(id)]);
            *close = true;
        }
        if self.may_rename(&m) {
            ui.add_space(6.0);
            if icon_button_ex(
                ui,
                &t,
                Icon::Edit,
                tr("Change name"),
                &format!("Change name of {}", m.shown()),
                true,
                w,
                t.text,
                t.text2,
            )
            .clicked()
            {
                self.modal = Some(Modal::MemberName {
                    id,
                    name: m.name.clone(),
                    display: m.display_name.clone(),
                });
            }
        }
    }

    /// Edit profile (My settings) for me. Without an email of my own it
    /// starts with the one I sign in with to the team server.
    pub(super) fn profile_modal(&self) -> Modal {
        let mut m = self.data.member(self.data.me).cloned().unwrap_or_default();
        if m.email.trim().is_empty() {
            if let Some(a) = &self.sync_rt.account {
                m.email = a.clone();
            }
        }
        Modal::profile(&m)
    }

    /// Whether I may change `m`'s names for everyone: someone else, and on a
    /// team server I'm its owner or an admin (as the server checks: only
    /// they change anyone else's entry, and an admin not the owner's).
    /// Alone or in a shared folder anyone may.
    pub(super) fn may_rename(&self, m: &Member) -> bool {
        let me = self.data.me;
        if m.id == me || !self.may_manage() {
            return false;
        }
        if !self.on_server() || m.role != MemberRole::Owner {
            return true;
        }
        // the owner's: the owner only (the server's word, else People's)
        match self.sync_rt.role {
            Some(r) => r == wb_net::Role::Owner,
            None => self
                .data
                .member(me)
                .is_some_and(|x| x.role == MemberRole::Owner),
        }
    }

    /// Change name: someone else's full and display names, which everyone
    /// then sees them by (on the phone app too). Returns true when it should
    /// close.
    pub(super) fn member_name_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        id: u64,
        name: &mut String,
        display: &mut String,
    ) -> bool {
        let mut close = false;
        let heading = |ui: &mut Ui, s: &str| {
            ui.label(RichText::new(s).strong().color(t.text));
        };
        heading(ui, tr("Full name"));
        let r = ui.add(field(name).desired_width(f32::INFINITY));
        name_field(&r, "Member full name");
        ui.add_space(10.0);
        heading(ui, tr("Display name"));
        let r = ui.add(
            field(display)
                .hint_text(tr("Display name"))
                .desired_width(f32::INFINITY),
        );
        name_field(&r, "Member display name");
        ui.label(
            RichText::new(tr(
                "Everyone sees them by the display name, else the full name, on phones too.",
            ))
            .small()
            .color(t.text3),
        );
        ui.add_space(4.0);
        let ok = !name.trim().is_empty() && self.data.member(id).is_some();
        let (save, cancel) = dialog_footer(ui, t, tr("Save changes"), ok);
        if save {
            if let Some(m) = self.data.members.iter_mut().find(|m| m.id == id) {
                m.name = name.trim().to_string();
                m.display_name = display.trim().to_string();
                self.dirty = true;
            }
            close = true;
        }
        close || cancel
    }

    /// Open Edit profile for me.
    pub(super) fn open_profile(&mut self) {
        self.profile_photo = None;
        self.modal = Some(self.profile_modal());
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
        if !media::photo_file(path) {
            self.toast(tr("Choose a JPG, PNG or WebP picture."));
            return;
        }
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
        // too narrow for the photo beside the fields: above them
        // (a wide window keeps them side by side: the dialog grows to fit)
        let stack = self.tier != super::shell::Tier::Wide
            && ui.available_width() < PREVIEW + 20.0 + FIELDS_W;
        let fields_w = if stack {
            ui.available_width()
        } else {
            FIELDS_W
        };
        let row = if stack {
            Layout::top_down(Align::Min)
        } else {
            Layout::left_to_right(Align::Min)
        };
        ui.with_layout(row, |ui| {
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
                help(ui, tr("JPG, PNG or WebP only"));
                // its room is kept without a photo, so choosing one doesn't
                // push Email and the rest down
                let remove = vec2(PREVIEW, CONTROL_H);
                if photo.is_none() {
                    ui.allocate_space(remove);
                } else if ui
                    .add_sized(
                        remove,
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
            ui.add_space(20.0);
            // fixed: a width taken from what's left would grow the dialog
            // a little every frame (the spacing between the columns)
            ui.vertical(|ui| {
                ui.set_width(fields_w);
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
        let ok = !name.trim().is_empty() && !getting_ready;
        let (save, cancel) = dialog_footer(ui, t, tr("Save changes"), ok);
        if save {
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
        if cancel {
            self.profile_photo = None;
            close = true;
        }
        close
    }
}
