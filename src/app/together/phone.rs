// Signing a phone in as this person: the server hands out a short one-time
// code (create_link_code), shown here as a QR code the iPhone app scans and
// as six letters and digits (Show code) to type when scanning doesn't work.
//
// The QR code holds workboard://link?server=<this workspace's address>&code=<code>,
// so the phone needs nothing else.

use super::*;
use std::time::{Duration, Instant};

/// A code from the server and when it stops working.
pub struct PhoneLink {
    pub(crate) code: String,
    pub(crate) until: Instant,
}

/// What the QR code holds for `server` and `code`.
pub(crate) fn link_url(server: &str, code: &str) -> String {
    format!(
        "workboard://link?server={}&code={}",
        query_escape(server),
        query_escape(code)
    )
}

/// Percent-encoding for a query value: everything but unreserved characters.
fn query_escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

/// The code as shown: two groups of three, easier to read out and type.
pub(crate) fn shown_code(code: &str) -> String {
    if code.len() == 6 && code.is_ascii() {
        format!("{} {}", &code[..3], &code[3..])
    } else {
        code.to_string()
    }
}

/// The QR code's side, in points. The space is kept from the start so
/// nothing below moves when the code arrives.
const QR_SIDE: f32 = 180.0;

/// No answer to create_link_code after this long: the server is older.
const NO_ANSWER: Duration = Duration::from_secs(10);

/// Paint `text` as a QR code `side` points wide: dark squares on white with
/// a quiet edge, whatever the theme, so phone cameras read it. Without
/// text (the code isn't here yet), an empty box of the same size.
fn qr_code(ui: &mut Ui, t: &Theme, text: Option<&str>, side: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Sign-in QR code")
    });
    let painter = ui.painter_at(rect);
    let code = text.and_then(|s| qrcode::QrCode::new(s.as_bytes()).ok());
    let Some(code) = code else {
        painter.rect_filled(rect, 6.0, t.column_bg);
        painter.rect_stroke(
            rect.shrink(0.5),
            6.0,
            egui::Stroke::new(1.0_f32, t.border),
            egui::StrokeKind::Inside,
        );
        return resp;
    };
    let n = code.width();
    // four modules of quiet zone on each side, as the standard asks
    let quiet = 4;
    let cell = side / (n + 2 * quiet) as f32;
    painter.rect_filled(rect, 6.0, Color32::WHITE);
    for (i, c) in code.to_colors().into_iter().enumerate() {
        if c == qrcode::Color::Dark {
            let (x, y) = ((i % n + quiet) as f32, (i / n + quiet) as f32);
            let min = rect.min + vec2(x * cell, y * cell);
            // a hair of overlap so no seams show between squares
            painter.rect_filled(
                Rect::from_min_size(min, vec2(cell + 0.5, cell + 0.5)),
                0.0,
                Color32::BLACK,
            );
        }
    }
    resp
}

impl App {
    /// Ask the server for a code that signs a phone in as me.
    pub(crate) fn ask_phone_link(&mut self) {
        self.sync_rt.phone_asked = Some(Instant::now());
        self.access_request(NetMsg::CreateLinkCode);
    }

    /// The server's code arrived.
    pub(super) fn got_phone_link(&mut self, code: String, seconds: u64) {
        self.sync_rt.phone_asked = None;
        self.sync_rt.phone = Some(PhoneLink {
            code,
            until: Instant::now() + Duration::from_secs(seconds),
        });
    }

    /// Work together, signed in on a server: a QR code for signing in the
    /// iPhone app as me, there as soon as the dialog opens and renewed when
    /// it runs out. "Show code" puts the same code under it to type.
    pub(super) fn phone_section(&mut self, ui: &mut Ui, t: &Theme, server: &str) {
        let left = self
            .sync_rt
            .phone
            .as_ref()
            .map(|p| p.until.saturating_duration_since(Instant::now()))
            .filter(|l| !l.is_zero());
        if left.is_none() && self.sync_rt.phone_asked.is_none() {
            self.ask_phone_link();
        }
        let code = left.and(self.sync_rt.phone.as_ref().map(|p| p.code.clone()));
        let no_answer = code.is_none()
            && self
                .sync_rt
                .phone_asked
                .is_some_and(|at| at.elapsed() >= NO_ANSWER);

        ui.label(RichText::new(tr("Sign in on your phone")).strong());
        ui.label(
            RichText::new(tr("Open Workboard on your iPhone and scan this QR code. If scanning doesn't work, press Show code and type it. No password needed."))
                .small()
                .color(t.text3),
        );
        ui.add_space(4.0);
        let url = code.as_deref().map(|c| link_url(server, c));
        // the QR code on the left, what goes with it on the right: the row
        // is as tall as the QR code whatever shows beside it, so neither the
        // code arriving nor Show code moves anything (the dialog is centred)
        ui.horizontal_top(|ui| {
            qr_code(ui, t, url.as_deref(), QR_SIDE);
            ui.add_space(12.0);
            ui.vertical(|ui| {
                let status = match left {
                    Some(left) => {
                        let secs = left.as_secs();
                        trf!(
                            "Works once, for {} more.",
                            format!("{}:{:02}", secs / 60, secs % 60)
                        )
                    }
                    None if no_answer => {
                        tr("This server can't sign in phones yet. Update workboard-server first.")
                            .into()
                    }
                    None => tr("Getting a code from the server…").into(),
                };
                ui.label(RichText::new(status).small().color(t.text3));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let shown = self.sync_rt.phone_code_shown;
                    let toggle = ui.add_enabled(
                        code.is_some(),
                        egui::Button::new(if shown {
                            tr("Hide code")
                        } else {
                            tr("Show code")
                        }),
                    );
                    if toggle.clicked() {
                        self.sync_rt.phone_code_shown = !shown;
                    }
                    if ui
                        .add_enabled(code.is_some(), egui::Button::new(tr("New QR code")))
                        .clicked()
                    {
                        self.ask_phone_link();
                    }
                });
                if let (true, Some(code)) = (self.sync_rt.phone_code_shown, &code) {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(shown_code(code))
                            .monospace()
                            .size(24.0)
                            .strong()
                            .color(t.text),
                    );
                    ui.label(
                        RichText::new(trf!("Server: {}", server))
                            .small()
                            .color(t.text2),
                    );
                }
            });
        });
        // the countdown, the code running out, and an answer that never came
        ui.ctx().request_repaint_after(Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_qr_link_carries_the_whole_address_and_the_code() {
        assert_eq!(
            link_url("https://wb.example.com/w/ab12", "K7Q2MX"),
            "workboard://link?server=https%3A%2F%2Fwb.example.com%2Fw%2Fab12&code=K7Q2MX"
        );
        assert_eq!(
            link_url("192.168.0.5:8080", "ABC234"),
            "workboard://link?server=192.168.0.5%3A8080&code=ABC234"
        );
        assert_eq!(shown_code("K7Q2MX"), "K7Q 2MX");
        assert_eq!(shown_code("odd"), "odd");
    }
}
