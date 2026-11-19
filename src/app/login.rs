// The sign-in screen and logging out.
//
// The sign-in screen fills the window in place of the app: on a first start,
// after logging out, and when an invite link is opened on it. One column in
// the middle, in steps: the team server's address, then email and password
// (or "Forgot password?"), or an invite code with a new password. Below them
// the other ways in: setting up a new server, or using Workboard on this Mac
// alone, without an account. The address is filled in from the last sign-in
// (`last-server`), and then it starts at the email step.
//
// Logging out signs this computer out of the team server: the token is
// forgotten and this computer's copy of the team workspace is deleted (the
// team's copy stays on the server and comes back on the next sign-in), as
// other apps do, so the next person at this computer can't read it. The
// sign-in screen then shows, and stays at the next start until someone
// signs in or chooses to use the app alone (`Extra::welcome` on data.json).

use super::together::{server_text, Form, Mode};
use super::*;
use crate::sync::SharedCfg;
use egui::Response;

/// Which step of the sign-in screen shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoginPage {
    /// The team server's address.
    Server,
    /// Email and password on that server (or a password reset).
    SignIn,
    /// An invite code, choosing a password.
    Invite,
    /// Set up an empty server and become its owner.
    NewServer,
    /// Use Workboard on this Mac only: just a name.
    OnThisMac,
}

/// The sign-in screen's state.
pub struct LoginScreen {
    pub page: LoginPage,
    /// The server address typed in.
    pub addr: String,
    /// The name typed in for using the app alone.
    pub name: String,
}

impl LoginScreen {
    pub fn new(page: LoginPage, addr: &str) -> Self {
        LoginScreen {
            page,
            addr: addr.to_string(),
            name: String::new(),
        }
    }

    /// Where the screen starts: the email step when the server is known.
    pub fn start(addr: &str) -> Self {
        let page = if addr.trim().is_empty() {
            LoginPage::Server
        } else {
            LoginPage::SignIn
        };
        Self::new(page, addr)
    }
}

/// The team server this computer last signed in to, so the sign-in screen
/// after logging out (or at the next start) has it filled in.
const LAST_SERVER: &str = "last-server";

pub(super) fn last_server() -> String {
    std::fs::read_to_string(Data::dir().join(LAST_SERVER))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub(super) fn set_last_server(addr: &str) {
    let _ = std::fs::create_dir_all(Data::dir());
    let _ = std::fs::write(Data::dir().join(LAST_SERVER), addr.trim());
}

/// Widest the sign-in column gets.
const COLUMN: f32 = 400.0;
/// Height of the screen's fields and buttons.
pub(super) const FIELD_H: f32 = 48.0;

/// The outline of the screen's fields and second buttons: darker than the
/// app's hairline, so a tall empty field still reads as one.
fn field_line(t: &Theme) -> Color32 {
    if t.dark {
        Color32::from_gray(90)
    } else {
        Color32::from_gray(185)
    }
}

/// The screen's main button: as wide as the column, in the accent.
pub(super) fn big_button(ui: &mut Ui, t: &Theme, text: &str) -> Response {
    wide_button(
        ui,
        text,
        17.0,
        (t.primary, t.primary_hover),
        Stroke::NONE,
        t.on_primary,
    )
}

/// A second way in, under the main one: outlined, as wide as the column.
fn outline_button(ui: &mut Ui, t: &Theme, text: &str) -> Response {
    wide_button(
        ui,
        text,
        16.0,
        (t.app_bg, t.hover),
        Stroke::new(1.0_f32, field_line(t)),
        t.text,
    )
}

/// A button as wide as the column with its words in the middle (egui's
/// own button puts them at the left when it is stretched).
fn wide_button(
    ui: &mut Ui,
    text: &str,
    size: f32,
    (fill, hover): (Color32, Color32),
    stroke: Stroke,
    color: Color32,
) -> Response {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, FIELD_H), Sense::click());
    name_it(&resp, text);
    let fill = if resp.hovered() { hover } else { fill };
    ui.painter()
        .rect(r, 6.0, fill, stroke, egui::StrokeKind::Inside);
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), super::bold_font(ui.ctx(), size), color);
    ui.painter().galley(r.center() - g.size() / 2.0, g, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A line across the column with "or" in the middle.
fn or_line(ui: &mut Ui, t: &Theme) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    let g = ui
        .painter()
        .layout_no_wrap(tr("or").to_string(), FontId::proportional(14.0), t.text2);
    let gap = g.size().x / 2.0 + 14.0;
    let y = r.center().y;
    let line = Stroke::new(1.0_f32, t.border);
    ui.painter().hline(r.left()..=r.center().x - gap, y, line);
    ui.painter().hline(r.center().x + gap..=r.right(), y, line);
    ui.painter().galley(r.center() - g.size() / 2.0, g, t.text2);
}

/// A label centred in the column, wrapping.
fn centred(ui: &mut Ui, text: RichText) {
    ui.vertical_centered(|ui| {
        ui.add(egui::Label::new(text).wrap());
    });
}

impl App {
    /// The sign-in screen, filling the window.
    pub(super) fn login_screen(&mut self, ctx: &egui::Context) {
        let Some(mut login) = self.login.take() else {
            return;
        };
        let t = self.t();
        let mut done = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(t.app_bg))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                // the top strip moves the window (there is no title bar)
                let strip = Rect::from_min_size(full.min, vec2(full.width(), 40.0));
                let drag = ui.interact(strip, ui.id().with("login_drag"), Sense::click_and_drag());
                if drag.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                // the screen's fields: taller, with a clear outline and the
                // app's corners
                let line = Stroke::new(1.0_f32, field_line(&t));
                let v = ui.visuals_mut();
                v.widgets.inactive.bg_stroke = line;
                v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, t.text3);
                v.widgets.inactive.corner_radius = 6.into();
                v.widgets.hovered.corner_radius = 6.into();
                v.widgets.active.corner_radius = 6.into();
                egui::ScrollArea::vertical()
                    .id_salt("login_scroll")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        self.login_header(ui, &t, &mut login);
                        let w = COLUMN.min(ui.available_width() - 32.0).max(240.0);
                        ui.add_space((full.height() * 0.06).max(32.0));
                        ui.horizontal(|ui| {
                            ui.add_space(((ui.available_width() - w) / 2.0).max(16.0));
                            ui.vertical(|ui| {
                                ui.set_width(w);
                                done = self.login_column(ui, &t, &mut login);
                                ui.add_space(40.0);
                                centred(
                                    ui,
                                    RichText::new(format!("Workboard {}", together::APP_VERSION))
                                        .size(13.0)
                                        .color(t.text3),
                                );
                            });
                        });
                        ui.add_space(32.0);
                    });
            });
        // it stays until someone signs in (net_welcome takes it away) or
        // starts on this Mac
        if !done && self.login.is_none() {
            self.login = Some(login);
        }
    }

    /// The name in the middle, and the other way in at the right.
    fn login_header(&mut self, ui: &mut Ui, t: &Theme, login: &mut LoginScreen) {
        let w = ui.available_width();
        let (hr, _) = ui.allocate_exact_size(vec2(w, 72.0), Sense::hover());
        // the mark and the name, centred together
        let name = ui.painter().layout_no_wrap(
            "Workboard".into(),
            super::bold_font(ui.ctx(), 24.0),
            t.text,
        );
        let mark = 30.0;
        let whole = mark + 10.0 + name.size().x;
        let left = hr.center().x - whole / 2.0;
        let cy = hr.top() + 40.0;
        let mr = Rect::from_min_size(pos2(left, cy - mark / 2.0), vec2(mark, mark));
        ui.painter().rect_filled(mr, 8.0, t.primary);
        let s = mark / 30.0;
        let at = |x: f32, y: f32| pos2(mr.left() + x * s, mr.top() + y * s);
        ui.painter().add(egui::Shape::line(
            vec![at(9.0, 15.5), at(13.0, 19.5), at(21.0, 10.5)],
            Stroke::new(2.6_f32, t.on_primary),
        ));
        ui.painter().galley(
            pos2(mr.right() + 10.0, cy - name.size().y / 2.0),
            name,
            t.text,
        );
        // the other way in, top right (not on a narrow window, where it
        // would run into the name)
        let (ask, act, to) = match login.page {
            LoginPage::Server | LoginPage::SignIn => (
                "No team server yet?",
                "Set up a server for my team",
                LoginPage::NewServer,
            ),
            LoginPage::Invite => (
                "Already have an account?",
                "Sign in with email",
                LoginPage::SignIn,
            ),
            LoginPage::NewServer | LoginPage::OnThisMac => {
                ("Already have a team server?", "Sign in", LoginPage::SignIn)
            }
        };
        if w < 760.0
            || self.sync_rt.joining.is_some()
            || self.sync_rt.listing.is_some()
            || self.sync_rt.choices.is_some()
        {
            return;
        }
        let rr = Rect::from_min_max(
            pos2(hr.right() - 300.0, hr.top() + 22.0),
            pos2(hr.right() - 32.0, hr.bottom()),
        );
        let bold = super::bold_font(ui.ctx(), 13.0);
        let clicked = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(rr)
                    .layout(Layout::top_down(Align::Max)),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new(tr(ask)).size(13.0).color(t.text2));
                    ui.link(RichText::new(tr(act)).font(bold)).clicked()
                },
            )
            .inner;
        if clicked {
            self.go_to(login, to);
        }
    }

    /// Show another step, with the form set for it (the email carries over).
    fn go_to(&mut self, login: &mut LoginScreen, page: LoginPage) {
        let mode = match page {
            LoginPage::Invite => Mode::Invite,
            LoginPage::NewServer => Mode::NewServer,
            _ => Mode::Account,
        };
        if page == LoginPage::SignIn && login.addr.trim().is_empty() {
            // no server yet: its address comes first
            login.page = LoginPage::Server;
        } else {
            login.page = page;
        }
        self.sync_rt.form = Form {
            mode,
            email: std::mem::take(&mut self.sync_rt.form.email),
            ..Default::default()
        };
        if page == LoginPage::OnThisMac && login.name.is_empty() {
            login.name = self
                .data
                .member(self.data.me)
                .map(|m| m.name.clone())
                .unwrap_or_default();
        }
    }

    /// The step's title, what it asks for and its button. True when the
    /// screen is done with.
    fn login_column(&mut self, ui: &mut Ui, t: &Theme, login: &mut LoginScreen) -> bool {
        let mut done = false;
        let (title, sub) = match login.page {
            LoginPage::Server => (
                "Sign in to your team",
                Some("First, type your team's server address."),
            ),
            LoginPage::SignIn => ("Sign in with email", None),
            LoginPage::Invite => (
                "Join with an invite code",
                Some("Type the code your team's admin gave you, and choose a password."),
            ),
            LoginPage::NewServer => (
                "Set up your team's server",
                Some("Type the address of your workboard-server (for example https://wb.example.com)."),
            ),
            LoginPage::OnThisMac => (
                if cfg!(windows) {
                    "Use it on this PC"
                } else {
                    "Use it on this Mac"
                },
                Some("Start your own workspace without an account. You can share it with your team later."),
            ),
        };
        centred(
            ui,
            RichText::new(tr(title))
                .font(super::bold_font(ui.ctx(), 40.0))
                .color(t.text),
        );
        if let Some(sub) = sub {
            ui.add_space(6.0);
            centred(ui, RichText::new(tr(sub)).size(17.0).color(t.text2));
        }
        if matches!(login.page, LoginPage::SignIn | LoginPage::Invite) {
            ui.add_space(10.0);
            self.server_chip(ui, t, login);
        }
        ui.add_space(28.0);
        let mut how = None;
        match login.page {
            LoginPage::Server => {
                self.server_field(ui, t, &mut login.addr);
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(tr("If you don't know it, ask your team's admin. The link in an invite mail fills it in."))
                            .size(13.0)
                            .color(t.text2),
                    )
                    .wrap(),
                );
                if !self.sync_rt.form.note.is_empty() {
                    ui.add_space(8.0);
                    ui.label(RichText::new(server_text(&self.sync_rt.form.note)).color(t.danger));
                }
                ui.add_space(20.0);
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                if big_button(ui, t, tr("Continue")).clicked() || enter {
                    match wb_net::server_url(&login.addr) {
                        Ok(_) => {
                            self.sync_rt.form.note.clear();
                            self.go_to(login, LoginPage::SignIn);
                        }
                        Err(e) => self.sync_rt.form.note = e,
                    }
                }
            }
            LoginPage::SignIn => {
                if !self.join_progress(ui, t) {
                    how =
                        self.login_form(ui, t, &mut login.addr, &[Mode::Account], "Sign in", true);
                    if self.sync_rt.form.mode == Mode::Account {
                        ui.add_space(20.0);
                        egui::Frame::new()
                            .fill(if t.dark {
                                Color32::from_gray(36)
                            } else {
                                Color32::from_gray(247)
                            })
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(16, 14))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(tr("New here? Click the link in the invite mail from your team's admin."))
                                            .size(14.0)
                                            .color(t.text),
                                    )
                                    .wrap(),
                                );
                                if ui.link(tr("Join with an invite code")).clicked() {
                                    self.go_to(login, LoginPage::Invite);
                                }
                            });
                    }
                }
            }
            LoginPage::Invite => {
                if !self.join_progress(ui, t) {
                    how = self.login_form(ui, t, &mut login.addr, &[Mode::Invite], "Join", true);
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        if ui.link(tr("Sign in with email")).clicked() {
                            self.go_to(login, LoginPage::SignIn);
                        }
                    });
                }
            }
            LoginPage::NewServer => {
                self.server_field(ui, t, &mut login.addr);
                if !self.join_progress(ui, t) {
                    how = self.login_form(
                        ui,
                        t,
                        &mut login.addr,
                        &[Mode::NewServer],
                        "Set up server",
                        true,
                    );
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        if ui.link(tr("Back to sign in")).clicked() {
                            self.go_to(login, LoginPage::SignIn);
                        }
                    });
                }
            }
            LoginPage::OnThisMac => {
                ui.label(
                    RichText::new(tr("What should we call you?"))
                        .font(super::bold_font(ui.ctx(), 15.0))
                        .color(t.text),
                );
                ui.add_space(2.0);
                let r = ui.add(
                    field(&mut login.name)
                        .hint_text(tr("Your name"))
                        .font(FontId::proportional(17.0))
                        .margin(egui::Margin::symmetric(14, 12))
                        .min_size(vec2(0.0, FIELD_H))
                        .desired_width(f32::INFINITY),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Your name entry")
                });
                ui.add_space(20.0);
                if big_button(ui, t, tr("Start")).clicked() {
                    self.start_on_this_mac(&login.name);
                    done = true;
                }
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    if ui.link(tr("Back to sign in")).clicked() {
                        self.go_to(login, LoginPage::SignIn);
                    }
                });
            }
        }
        if let Some(how) = how {
            if let Err(e) = self.connect_with(&login.addr, how) {
                self.sync_rt.form.note = e;
            }
        }
        // the way without an account, under the first two steps
        if matches!(login.page, LoginPage::Server | LoginPage::SignIn)
            && self.sync_rt.joining.is_none()
            && self.sync_rt.listing.is_none()
            && self.sync_rt.choices.is_none()
            && self.sync_rt.form.mode != Mode::Forgot
        {
            ui.add_space(28.0);
            or_line(ui, t);
            ui.add_space(28.0);
            let alone = if cfg!(windows) {
                tr("Use it on this PC without an account")
            } else {
                tr("Use it on this Mac without an account")
            };
            if outline_button(ui, t, alone).clicked() {
                self.go_to(login, LoginPage::OnThisMac);
            }
        }
        done
    }

    /// The server's address with its name above it.
    fn server_field(&mut self, ui: &mut Ui, t: &Theme, addr: &mut String) {
        ui.label(
            RichText::new(tr("Server address"))
                .font(super::bold_font(ui.ctx(), 15.0))
                .color(t.text),
        );
        ui.add_space(2.0);
        let r = ui.add(
            field(addr)
                .hint_text("https://wb.example.com")
                .font(FontId::proportional(17.0))
                .margin(egui::Margin::symmetric(14, 12))
                .min_size(vec2(0.0, FIELD_H))
                .desired_width(f32::INFINITY),
        );
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Server address entry")
        });
    }

    /// Which server it signs in to, as a pill, and Change to type another.
    fn server_chip(&mut self, ui: &mut Ui, t: &Theme, login: &mut LoginScreen) {
        let font = super::bold_font(ui.ctx(), 15.0);
        let g = ui
            .painter()
            .layout_no_wrap(login.addr.trim().to_string(), font, t.text);
        let change = tr("Change");
        let cg = ui.painter().layout_no_wrap(
            change.to_string(),
            FontId::proportional(15.0),
            t.accent_text,
        );
        let pill = vec2(g.size().x + 28.0, 30.0);
        let whole = pill.x + 10.0 + cg.size().x;
        let w = ui.available_width();
        let mut clicked = false;
        ui.horizontal(|ui| {
            ui.add_space(((w - whole) / 2.0).max(0.0));
            let (pr, _) = ui.allocate_exact_size(pill, Sense::hover());
            ui.painter().rect_filled(
                pr,
                15.0,
                if t.dark {
                    Color32::from_gray(48)
                } else {
                    Color32::from_gray(244)
                },
            );
            ui.painter().galley(pr.center() - g.size() / 2.0, g, t.text);
            ui.add_space(4.0);
            clicked = ui.link(change).clicked();
        });
        if clicked {
            login.page = LoginPage::Server;
            self.sync_rt.form.note.clear();
        }
    }

    /// "Use it on this Mac": the workspace open behind the sign-in screen,
    /// with the name typed in, and no sign-in screen at the next start.
    fn start_on_this_mac(&mut self, name: &str) {
        let me = self.data.me;
        if let Some(m) = self
            .data
            .members
            .iter_mut()
            .find(|m| m.id == me)
            .filter(|_| !name.trim().is_empty())
        {
            m.name = name.trim().to_string();
        }
        self.data.extra.welcome = false;
        self.sync_rt.form = Form::default();
        self.dirty = true;
    }

    /// Open the sign-in screen (an invite link, say), keeping what it shows
    /// if it is open already.
    pub(super) fn open_login(&mut self, page: LoginPage, addr: &str) {
        let l = self
            .login
            .get_or_insert_with(|| LoginScreen::new(page, addr));
        l.page = page;
        if !addr.is_empty() {
            l.addr = addr.to_string();
        }
        if l.addr.trim().is_empty() && matches!(page, LoginPage::SignIn | LoginPage::Invite) {
            // no server yet: its address comes first
            l.page = LoginPage::Server;
        }
        self.modal = None;
    }

    /// The workspace open is on a team server: Log out is offered.
    pub(super) fn on_team_server(&self) -> bool {
        self.on_server()
    }

    /// Asking before logging out: what goes, and what may be lost.
    pub(super) fn log_out_body(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        let mut close = false;
        let server = self
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.clone())
            .unwrap_or_default();
        ui.label(RichText::new(trf!("Log out of {}?", server)).color(t.text));
        ui.add_space(4.0);
        ui.label(RichText::new(tr("This computer's copy of the team workspace is deleted. Everything stays on the server and comes back when you sign in again.")).color(t.text2));
        if self.sync_rt.net_status != Some(wb_net::Status::Connected) {
            ui.add_space(4.0);
            ui.label(RichText::new(tr("This computer isn't connected to the server right now. Changes you made while it was offline haven't gone up and will be lost.")).color(t.danger));
        }
        let waiting = self.files.waiting().count();
        if waiting > 0 {
            ui.add_space(4.0);
            ui.label(
                RichText::new(trf!(
                    "Pictures and videos not on the server yet: {}. They will be lost.",
                    waiting
                ))
                .color(t.danger),
            );
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add(Tint(
                    egui::Button::new(RichText::new(tr("Log out")).color(Color32::WHITE))
                        .fill(t.danger),
                ))
                .clicked()
            {
                self.log_out();
                close = true;
            }
            if ui.button(tr("Cancel")).clicked() {
                close = true;
            }
        });
        close
    }

    /// Sign this computer out of the team server and delete its copy of the
    /// team workspace; the sign-in screen shows next (see the top of this file).
    pub(super) fn log_out(&mut self) {
        let Some(SharedCfg {
            server: Some(addr), ..
        }) = self.data.extra.shared.clone()
        else {
            return;
        };
        let email = self.sync_rt.account.clone().unwrap_or_default();
        // what isn't sent yet goes now if it can (the connection sends it
        // before it closes)
        self.finish_before_quit();
        self.stop_sharing();
        // the team's pictures waiting to go up belong to the copy deleted
        self.task_files.clear();
        self.task_files_saved = None;
        let file = self.ws_file.clone();
        let mut d = if self.persist {
            let gone = if file == "data.json" {
                Data::path()
            } else {
                Data::dir().join(&file)
            };
            // a save still waiting would put the deleted copy back
            self.save_due.set(None);
            Data::finish_saving();
            let _ = std::fs::remove_file(gone);
            let mut list = Data::workspaces();
            list.retain(|w| w.file != file);
            Data::save_workspaces(&list);
            Data::set_last_workspace("data.json");
            set_last_server(&addr);
            // this computer's own workspace, if it has one (an empty one
            // when the team's copy was in data.json)
            Data::load()
        } else {
            Data::fresh()
        };
        d.fix_activity_ids();
        d.fold_legacy_slack();
        super::together::fill_sync_baseline(&mut d);
        d.extra.welcome = true;
        self.data = d;
        self.data_changed();
        self.ws_file = "data.json".into();
        if self.persist {
            self.save_now();
        }
        self.ws_places.clear();
        self.loc = self
            .data
            .first_list()
            .map(Location::List)
            .unwrap_or(Location::Home);
        self.section = Section::Home;
        self.open_task = None;
        self.chat.open = None;
        self.chat.thread_open = None;
        self.recent.clear();
        self.clear_history();
        self.palette = None;
        self.quick_add = None;
        self.modal = None;
        self.notify_rt.seeded = false;
        self.sync_rt.form = Form {
            email,
            ..Default::default()
        };
        self.login = Some(LoginScreen::new(LoginPage::SignIn, &addr));
        self.toast(tr("Logged out"));
    }
}
