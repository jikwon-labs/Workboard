// The sign-in screen and logging out.
//
// The sign-in screen fills the window in place of the app: on a first start,
// after logging out, and when an invite link is opened while this computer
// isn't on a team server. It signs in to the team's server (email and
// password, an invite code, or a password reset), sets up a new server, or
// lets the person use Workboard on this Mac alone, without an account.
//
// Logging out signs this computer out of the team server: the token is
// forgotten and this computer's copy of the team workspace is deleted (the
// team's copy stays on the server and comes back on the next sign-in), as
// other apps do, so the next person at this computer can't read it. The
// sign-in screen then shows, and stays at the next start until someone
// signs in or chooses to use the app alone (`Extra::welcome` on data.json).

use super::together::{Form, Mode};
use super::*;
use crate::sync::SharedCfg;

/// Which part of the sign-in screen shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoginPage {
    /// Sign in to the team's server (email and password, or an invite code).
    SignIn,
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
                egui::ScrollArea::vertical()
                    .id_salt("login_scroll")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        let w = COLUMN.min(ui.available_width() - 32.0).max(240.0);
                        ui.add_space((full.height() * 0.12).max(56.0));
                        ui.horizontal(|ui| {
                            ui.add_space(((ui.available_width() - w) / 2.0).max(16.0));
                            ui.vertical(|ui| {
                                ui.set_width(w);
                                done = self.login_column(ui, &t, &mut login);
                            });
                        });
                        ui.add_space(40.0);
                    });
            });
        // it stays until someone signs in (net_welcome takes it away) or
        // starts on this Mac
        if !done && self.login.is_none() {
            self.login = Some(login);
        }
    }

    /// The sign-in column. True when the screen is done with.
    fn login_column(&mut self, ui: &mut Ui, t: &Theme, login: &mut LoginScreen) -> bool {
        let mut done = false;
        ui.label(RichText::new("Workboard").size(28.0).strong().color(t.text));
        ui.label(
            RichText::new(tr("Tasks, chat and docs for you and your team."))
                .size(14.0)
                .color(t.text2),
        );
        ui.add_space(20.0);
        dialog_frame(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let joining = self.sync_rt.joining.is_some();
            match login.page {
                LoginPage::SignIn => {
                    ui.label(
                        RichText::new(tr("Sign in to your team"))
                            .size(18.0)
                            .color(t.text),
                    );
                    ui.label(RichText::new(tr("Type your team's server address. Then sign in with your email and password, or use the invite code you were sent.")).color(t.text2));
                    ui.add_space(8.0);
                    self.server_form(
                        ui,
                        t,
                        &mut login.addr,
                        &[Mode::Account, Mode::Invite],
                        "Sign in",
                    );
                }
                LoginPage::NewServer => {
                    ui.label(
                        RichText::new(tr("Set up your team's server"))
                            .size(18.0)
                            .color(t.text),
                    );
                    ui.label(RichText::new(tr("Type the address of your workboard-server (for example https://wb.example.com).")).color(t.text2));
                    ui.add_space(8.0);
                    self.server_form(ui, t, &mut login.addr, &[Mode::NewServer], "Set up server");
                    if !joining {
                        ui.add_space(6.0);
                        if ui.link(tr("Back to sign in")).clicked() {
                            login.page = LoginPage::SignIn;
                            self.sync_rt.form = Form::default();
                        }
                    }
                }
                LoginPage::OnThisMac => {
                    ui.label(
                        RichText::new(tr("Use it on this Mac"))
                            .size(18.0)
                            .color(t.text),
                    );
                    ui.label(RichText::new(tr("Start your own workspace without an account. You can share it with your team later.")).color(t.text2));
                    ui.add_space(8.0);
                    ui.label(RichText::new(tr("What should we call you?")).color(t.text2));
                    let r = ui.add(
                        field(&mut login.name)
                            .hint_text(tr("Your name"))
                            .desired_width(f32::INFINITY),
                    );
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::TextEdit,
                            true,
                            "Your name entry",
                        )
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.link(tr("Back to sign in")).clicked() {
                            login.page = LoginPage::SignIn;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if primary_button(ui, t, tr("Start")).clicked() {
                                self.start_on_this_mac(&login.name);
                                done = true;
                            }
                        });
                    });
                }
            }
        });
        if login.page == LoginPage::SignIn && self.sync_rt.joining.is_none() {
            ui.add_space(14.0);
            ui.label(RichText::new(tr("No team server yet?")).color(t.text2));
            ui.horizontal_wrapped(|ui| {
                if ui.link(tr("Set up a server for my team")).clicked() {
                    login.page = LoginPage::NewServer;
                    self.sync_rt.form = Form {
                        mode: Mode::NewServer,
                        email: std::mem::take(&mut self.sync_rt.form.email),
                        ..Default::default()
                    };
                }
                ui.label(RichText::new("\u{00B7}").color(t.text3));
                if ui
                    .link(tr("Use it on this Mac without an account"))
                    .clicked()
                {
                    login.page = LoginPage::OnThisMac;
                    if login.name.is_empty() {
                        login.name = self
                            .data
                            .member(self.data.me)
                            .map(|m| m.name.clone())
                            .unwrap_or_default();
                    }
                }
            });
        }
        done
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
        match &mut self.login {
            Some(l) => {
                l.page = page;
                if !addr.is_empty() {
                    l.addr = addr.to_string();
                }
            }
            None => self.login = Some(LoginScreen::new(page, addr)),
        }
        self.modal = None;
    }

    /// The workspace open is on a team server: Log out is offered.
    pub(super) fn on_team_server(&self) -> bool {
        self.data
            .extra
            .shared
            .as_ref()
            .is_some_and(|c| c.server.is_some())
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
        self.ws_file = "data.json".into();
        if self.persist {
            self.data.save();
        }
        self.ws_places.clear();
        self.loc = self
            .data
            .first_list()
            .map(Location::List)
            .unwrap_or(Location::Home);
        self.section = Section::Home;
        self.open_task = None;
        self.chat_open = None;
        self.thread_open = None;
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
        self.toast = Some((tr("Logged out").into(), now_ts()));
    }
}
