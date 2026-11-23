// Take a photo: the camera beside + under the message box opens a window
// with the camera's live picture, and Take photo puts what it shows in the
// box like a file picked with +. It is kept as a BMP here and goes as a
// JPEG (files.rs converts it, as it does any BMP). The camera is on only
// while the window is open (wb_voice::start_preview); tests, and
// WORKBOARD_TEST_VIDEO=1 on a computer without a camera, get a colored
// test picture instead.

use super::*;
use std::sync::{Arc, Mutex};
use wb_voice::{Picture, Video};

/// How wide a photo is taken (narrower when the camera is).
const PHOTO_WIDTH: u32 = 1920;

/// What the camera thread hands the window.
#[derive(Default)]
struct Shared {
    /// The newest picture, and whether it still has to go to the screen.
    latest: Option<(Arc<Picture>, bool)>,
    /// Why the camera can't be used.
    trouble: Option<String>,
}

pub(super) struct PhotoRt {
    /// The camera; tests use a colored picture instead.
    pub(super) video: Video,
    feed: Option<wb_voice::Feed>,
    shared: Arc<Mutex<Shared>>,
    texture: Option<egui::TextureHandle>,
    /// The picture on the screen: what Take photo keeps.
    pub(super) shown: Option<Arc<Picture>>,
}

impl Default for PhotoRt {
    fn default() -> Self {
        PhotoRt {
            video: if cfg!(test) || std::env::var("WORKBOARD_TEST_VIDEO").is_ok_and(|v| v == "1") {
                Video::Pattern([60, 120, 200])
            } else {
                Video::Camera
            },
            feed: None,
            shared: Default::default(),
            texture: None,
            shown: None,
        }
    }
}

impl App {
    /// The camera window for the channel's (or the thread's) message box.
    pub(super) fn open_camera(&mut self, thread: bool) {
        self.modal = Some(Modal::Camera { thread });
    }

    /// Whether the camera is on (its window is open).
    pub(super) fn camera_on(&self) -> bool {
        self.photo.feed.is_some()
    }

    /// Turn the camera off and forget its pictures.
    pub(super) fn stop_camera(&mut self) {
        self.photo.feed = None;
        // a new one: a last picture from the old thread can't show next time
        self.photo.shared = Default::default();
        self.photo.texture = None;
        self.photo.shown = None;
    }

    fn start_camera(&mut self, ctx: &egui::Context) {
        let (pics, repaint) = (self.photo.shared.clone(), ctx.clone());
        let (problems, repaint2) = (self.photo.shared.clone(), ctx.clone());
        self.photo.feed = Some(wb_voice::start_preview(
            self.photo.video,
            PHOTO_WIDTH,
            move |p| {
                if let Some(p) = p {
                    pics.lock().unwrap().latest = Some((Arc::new(p), true));
                }
                repaint.request_repaint();
            },
            move |e| {
                problems.lock().unwrap().trouble = Some(e);
                repaint2.request_repaint();
            },
        ));
    }

    /// The camera window: its live picture, Take photo and Cancel. True
    /// when it should close.
    pub(super) fn camera_body(&mut self, ui: &mut Ui, t: &Theme, thread: bool) -> bool {
        if self.photo.feed.is_none() {
            self.start_camera(ui.ctx());
        }
        let (fresh, trouble) = {
            let mut s = self.photo.shared.lock().unwrap();
            let fresh = match s.latest.as_mut() {
                Some((p, new)) if *new => {
                    *new = false;
                    Some(p.clone())
                }
                _ => None,
            };
            (fresh, s.trouble.clone())
        };
        if let Some(pic) = fresh {
            // camera pictures are opaque: premultiplied or not is the same
            let img = egui::ColorImage::from_rgba_premultiplied(
                [pic.width as usize, pic.height as usize],
                &pic.rgba,
            );
            match self.photo.texture.as_mut() {
                Some(tex) => tex.set(img, egui::TextureOptions::LINEAR),
                None => {
                    let tex =
                        ui.ctx()
                            .load_texture("photo-camera", img, egui::TextureOptions::LINEAR);
                    self.photo.texture = Some(tex);
                }
            }
            self.photo.shown = Some(pic);
        }
        let stopped = self.photo.feed.as_ref().is_some_and(|f| f.stopped());
        // the picture across the window, in 16:9 like most computer cameras
        // (another shape fits inside, on black)
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::hover());
        name_it(&resp, "Camera preview");
        ui.painter().rect_filled(rect, 8.0, Color32::BLACK);
        let problem = match (&trouble, stopped) {
            // wb-voice's words: the usual ones are in the Korean table, any
            // other shows as the system put it
            (Some(e), _) => Some(tr(e).to_string()),
            (None, true) => Some(tr("The camera stopped.").to_string()),
            _ => None,
        };
        match (&self.photo.texture, &problem) {
            (Some(tex), None) => {
                let size = tex.size_vec2();
                let scale = (rect.width() / size.x).min(rect.height() / size.y);
                let at = Rect::from_center_size(rect.center(), size * scale);
                egui::Image::new((tex.id(), at.size()))
                    .corner_radius(8)
                    .paint_at(ui, at);
            }
            _ => {
                let words = problem.unwrap_or_else(|| tr("Starting the camera...").to_string());
                let galley = ui.painter().layout(
                    words,
                    FontId::proportional(14.0),
                    Color32::from_gray(220),
                    rect.width() - 48.0,
                );
                let at = rect.center() - galley.size() / 2.0;
                ui.painter().galley(at, galley, Color32::from_gray(220));
            }
        }
        let ready = self.photo.shown.is_some() && trouble.is_none() && !stopped;
        let (take, cancel) = dialog_footer(ui, t, tr("Take photo"), ready);
        if take {
            if let Some(pic) = self.photo.shown.clone() {
                let bmp = crate::media::bmp_from_rgba(pic.width, pic.height, &pic.rgba);
                let when = chrono::Local::now().format("%Y-%m-%d at %H.%M.%S");
                self.add_photo(thread, bmp, &format!("Photo {}.bmp", when));
            }
        }
        if take || cancel {
            self.stop_camera();
            return true;
        }
        false
    }
}
