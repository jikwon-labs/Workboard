// Signing in: the server field, the progress of joining, the sign-in and
// invite forms with their password fields, and the account section.

use super::*;

/// What a password may hold: English letters, digits, symbols and spaces.
/// Typed in Korean mode the same keys give other letters, and the password
/// then won't match the one typed in English on another computer or phone.
pub(super) fn password_char(c: char) -> bool {
    (' '..='~').contains(&c)
}

/// A password box: dots by default, an eye button inside its right edge
/// shows what was typed, and only `password_char`s go in. Anything else
/// (Korean input, pasted text in another script) is left out and a note
/// under the box asks for English input.
pub(super) fn secret_edit(
    ui: &mut Ui,
    t: &Theme,
    text: &mut String,
    label: &str,
    hint: &str,
    big: bool,
) -> egui::Response {
    let id = ui.make_persistent_id(("password field", label));
    let shown_id = id.with("shown");
    let note_id = id.with("not english");
    let mut shown = ui.data(|d| d.get_temp::<bool>(shown_id)).unwrap_or(false);
    let mut note = ui.data(|d| d.get_temp::<bool>(note_id)).unwrap_or(false);
    if ui.memory(|m| m.has_focus(id)) {
        ui.input_mut(|i| {
            i.events.retain_mut(|e| {
                let s = match e {
                    egui::Event::Text(s) | egui::Event::Paste(s) => s,
                    egui::Event::Ime(egui::ImeEvent::Commit(s)) => s,
                    // a word still being put together in another script:
                    // nothing of it goes in
                    egui::Event::Ime(egui::ImeEvent::Preedit(s)) => {
                        if !s.is_empty() {
                            note = true;
                        }
                        return false;
                    }
                    _ => return true,
                };
                let kept: String = s.chars().filter(|c| password_char(*c)).collect();
                if kept.len() != s.len() {
                    note = true;
                } else if !kept.is_empty() {
                    note = false;
                }
                *s = kept;
                !s.is_empty()
            })
        });
    }
    let (eye_w, margin) = if big {
        (
            40.0,
            egui::Margin {
                left: 14,
                right: 40,
                top: 12,
                bottom: 12,
            },
        )
    } else {
        (
            30.0,
            egui::Margin {
                left: 8,
                right: 30,
                top: 5,
                bottom: 5,
            },
        )
    };
    let mut te = field(text)
        .id(id)
        .hint_text(tr(hint))
        .password(!shown)
        .margin(margin)
        .desired_width(f32::INFINITY);
    if big {
        te = te
            .font(FontId::proportional(17.0))
            .min_size(vec2(0.0, super::login::FIELD_H));
    }
    let r = ui.add(te);
    let name = format!("{} entry", label);
    r.widget_info(move || egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name));
    // the eye, inside the box's right edge (the response holds the box
    // without its margin)
    let outer = r.rect + margin;
    let er = egui::Rect::from_min_max(
        egui::pos2(outer.right() - eye_w, outer.top()),
        outer.right_bottom(),
    );
    let eye = ui.interact(er, id.with("eye"), Sense::click());
    let what = if shown {
        "Hide password"
    } else {
        "Show password"
    };
    name_it(&eye, what);
    let color = if eye.hovered() { t.text } else { t.text2 };
    super::icons::line_icon(
        ui.painter(),
        if shown { "eye_off" } else { "eye" },
        er.center(),
        color,
    );
    let eye = eye
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tr(what));
    // pressing the eye takes the focus off the box (egui drops it on any
    // press elsewhere); give it straight back so typing carries on
    let was_focused = ui.data(|d| d.get_temp::<bool>(id.with("focused")).unwrap_or(false));
    if was_focused && (eye.is_pointer_button_down_on() || eye.clicked()) {
        r.request_focus();
    }
    if eye.clicked() {
        shown = !shown;
        r.request_focus();
    }
    let focused = ui.memory(|m| m.has_focus(id));
    ui.data_mut(|d| d.insert_temp(id.with("focused"), focused));
    text.retain(password_char);
    if note && !focused {
        note = false;
    }
    if note {
        ui.label(
            RichText::new(tr(
                "Passwords take English letters, numbers and symbols. Switch to English input.",
            ))
            .size(12.0)
            .color(t.danger),
        );
    }
    ui.data_mut(|d| {
        d.insert_temp(shown_id, shown);
        d.insert_temp(note_id, note);
    });
    r
}

impl App {
    /// Server address, sign-in fields, and Connect (or the connection's
    /// progress and Cancel while it tries).
    pub(crate) fn server_form(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        addr: &mut String,
        modes: &[Mode],
        button: &str,
    ) {
        ui.label(RichText::new(tr("Server")).color(t.text2));
        let r = ui.add(
            field(addr)
                .hint_text("https://wb.example.com")
                .desired_width(f32::INFINITY),
        );
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Server address")
        });
        if self.join_progress(ui, t) {
            return;
        }
        ui.add_space(6.0);
        if let Some(how) = self.login_form(ui, t, addr, modes, button, false) {
            if let Err(e) = self.connect_with(addr, how) {
                self.sync_rt.form.note = e;
            }
        }
    }

    /// While connecting from a sign-in form: how it goes, and Cancel. True
    /// while it connects; a refusal goes back to the form with the reason.
    pub(crate) fn join_progress(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        if self.sync_rt.listing.is_some() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("Looking up your workspaces...")).color(t.text2));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        self.sync_rt.listing = None;
                    }
                });
            });
            return self.sync_rt.listing.is_some();
        }
        if let Some((list, how)) = self.sync_rt.choices.clone() {
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr("Choose a workspace"))
                    .size(15.0)
                    .strong()
                    .color(t.text),
            );
            ui.label(
                RichText::new(tr("Your account is in these workspaces on this server. You can open the others later the same way."))
                    .small()
                    .color(t.text2),
            );
            ui.add_space(6.0);
            for (addr, name) in &list {
                let label = if name.trim().is_empty() {
                    addr.clone()
                } else {
                    name.clone()
                };
                let r = ui.add(
                    egui::Button::new(RichText::new(label).size(15.0))
                        .min_size(vec2(ui.available_width(), 36.0)),
                );
                if r.clicked() {
                    self.open_or_join(addr, how.clone());
                }
            }
            ui.add_space(6.0);
            if ui.button(tr("Cancel")).clicked() {
                self.sync_rt.choices = None;
            }
            return self.sync_rt.choices.is_some() || self.sync_rt.joining.is_some();
        }
        if self.sync_rt.joining.is_some() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let status = net_status_text(
                    self.sync_rt.net.is_some(),
                    self.sync_rt.net_status.as_ref(),
                    self.sync_rt.fails,
                );
                ui.label(RichText::new(status).color(t.text2));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(tr("Cancel")).clicked() {
                        self.sync_rt.joining = None;
                        self.sync_rt.net = None;
                        self.sync_rt.net_status = None;
                    }
                });
            });
            if let Some(why) = net_problem(self.sync_rt.net_status.as_ref())
                .filter(|_| matches!(self.sync_rt.net_status, Some(NetStatus::Reconnecting(_))))
            {
                ui.label(RichText::new(why).small().color(WARN));
                // an app older than the server can't talk to it (an invite
                // link with /w/<id> needs 0.1.13 or newer), so let it update
                // from that same server right here
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(tr("An older Workboard may not work with this server."))
                            .small()
                            .color(t.text2),
                    );
                    if ui.button(tr("Update Workboard")).clicked() {
                        if let Some(j) = &self.sync_rt.joining {
                            crate::updater::set_server(&j.addr);
                        }
                        // Sparkle shows its own window; only explain when it can't run
                        self.sync_rt.form.update_note = String::new();
                        self.sync_rt.form.app_updates = false;
                        match crate::updater::check_now() {
                            Ok(crate::updater::Shown::BySparkle) => {}
                            // Windows: how it goes shows here
                            Ok(crate::updater::Shown::ByApp) => {
                                self.sync_rt.form.app_updates = true
                            }
                            Err(why) => self.sync_rt.form.update_note = why.explain(),
                        }
                    }
                });
                if self.sync_rt.form.app_updates {
                    ui.add_space(4.0);
                    crate::updater::status_ui(ui, t, false);
                }
                if !self.sync_rt.form.update_note.is_empty() {
                    ui.add(
                        egui::Label::new(
                            RichText::new(self.sync_rt.form.update_note.clone())
                                .small()
                                .color(t.text2),
                        )
                        .wrap(),
                    );
                }
            }
            // refused: say why and let them fix what they typed
            if let Some(NetStatus::Refused(why) | NetStatus::Gone(why)) =
                self.sync_rt.net_status.clone()
            {
                self.sync_rt.joining = None;
                self.sync_rt.net = None;
                self.sync_rt.net_status = None;
                self.sync_rt.form.note = why;
            }
            return self.sync_rt.joining.is_some();
        }
        false
    }

    /// The sign-in fields for one of `modes` (with tabs when there are
    /// several, and "Forgot password?" under the password). The way to sign
    /// in when `button` is clicked and everything needed is there. `big`:
    /// the sign-in screen's look (bold names, tall fields, a button as wide
    /// as the fields, "Forgot password?" beside the password's name).
    pub(crate) fn login_form(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        addr: &mut String,
        modes: &[Mode],
        button: &str,
        big: bool,
    ) -> Option<How> {
        let mut form = std::mem::take(&mut self.sync_rt.form);
        if !modes.contains(&form.mode)
            && !(form.mode == Mode::Forgot && modes.contains(&Mode::Account))
        {
            form.mode = modes[0];
        }
        if modes.len() > 1 {
            ui.horizontal(|ui| {
                for m in modes {
                    let label = match m {
                        Mode::Account => "Email and password",
                        Mode::Invite => "Invite code",
                        Mode::Forgot => "Forgot password",
                        Mode::NewServer => "New server",
                    };
                    let on = form.mode == *m || (form.mode == Mode::Forgot && *m == Mode::Account);
                    if ui.selectable_label(on, tr(label)).clicked() && form.mode != *m {
                        // the email carries over; passwords and codes don't
                        form = Form {
                            mode: *m,
                            email: std::mem::take(&mut form.email),
                            ..Default::default()
                        };
                    }
                }
            });
            ui.add_space(4.0);
        }
        let ctx = ui.ctx().clone();
        let name_text = |label: &str| {
            if big {
                RichText::new(tr(label))
                    .font(super::bold_font(&ctx, 15.0))
                    .color(t.text)
            } else {
                RichText::new(tr(label)).color(t.text2)
            }
        };
        let edit = |ui: &mut Ui, text: &mut String, label: &str, hint: &str, secret: bool| {
            if secret {
                secret_edit(ui, t, text, label, hint, big);
                return;
            }
            let mut te = field(text)
                .hint_text(tr(hint))
                .password(secret)
                .desired_width(f32::INFINITY);
            if big {
                te = te
                    .font(FontId::proportional(17.0))
                    .margin(egui::Margin::symmetric(14, 12))
                    .min_size(vec2(0.0, super::login::FIELD_H));
            }
            let r = ui.add(te);
            let name = format!("{} entry", label);
            r.widget_info(move || {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name)
            });
        };
        let field = |ui: &mut Ui, text: &mut String, label: &str, hint: &str, secret: bool| {
            // a gap above each name, so a name groups with its own field
            ui.add_space(if big { 12.0 } else { 4.0 });
            ui.label(name_text(label));
            if big {
                ui.add_space(2.0);
            }
            edit(ui, text, label, hint, secret);
        };
        let mut go = false;
        let mut how = None;
        match form.mode {
            Mode::Account => {
                field(ui, &mut form.email, "Email", "you@company.com", false);
                let forgot = if big {
                    ui.add_space(12.0);
                    let forgot = ui
                        .horizontal(|ui| {
                            ui.label(name_text("Password"));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.link(tr("Forgot password?")).clicked()
                            })
                            .inner
                        })
                        .inner;
                    ui.add_space(2.0);
                    edit(ui, &mut form.password, "Password", "", true);
                    forgot
                } else {
                    field(ui, &mut form.password, "Password", "", true);
                    ui.link(tr("Forgot password?")).clicked()
                };
                if forgot {
                    form.mode = Mode::Forgot;
                    form.sent = false;
                    form.note.clear();
                }
            }
            Mode::Invite => {
                field(
                    ui,
                    &mut form.code,
                    "Invite code",
                    "XXXX-XXXX-XXXX, or paste the invite link",
                    false,
                );
                // a pasted link (or a whole invite mail) fills in everything it has
                if form.code.contains("workboard://") {
                    let j = wb_net::parse_join(&form.code);
                    form.code = j.code;
                    if let Some(e) = j.email {
                        form.email = e;
                    }
                    if let Some(s) = j.server {
                        *addr = s;
                    }
                }
                field(
                    ui,
                    &mut form.email,
                    "Email",
                    "the email you were invited with",
                    false,
                );
                field(
                    ui,
                    &mut form.password,
                    "Choose a password",
                    &trf!("at least {} characters", wb_net::MIN_PASSWORD),
                    true,
                );
                field(
                    ui,
                    &mut form.repeat,
                    "Repeat password",
                    "the same password again",
                    true,
                );
                ui.label(RichText::new(tr("You sign in with this email and password on your other computers and phones.")).small().color(t.text3));
            }
            Mode::Forgot => {
                field(ui, &mut form.email, "Email", "you@company.com", false);
                if let Some(rx) = &self.sync_rt.forgot {
                    match rx.try_recv() {
                        Ok(Ok(true)) => {
                            form.sent = true;
                            form.note = trf!("If {} has an account on this server, a reset code is on its way. It works for 30 minutes.", form.email.trim());
                            self.sync_rt.forgot = None;
                        }
                        Ok(Ok(false)) => {
                            form.note = tr("This server can't send email. Ask the owner or an admin for a new invite code instead.").into();
                            self.sync_rt.forgot = None;
                        }
                        Ok(Err(e)) => {
                            form.note = e;
                            self.sync_rt.forgot = None;
                        }
                        Err(_) => {
                            ui.label(RichText::new(tr("Asking the server\u{2026}")).color(t.text2));
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_millis(200));
                        }
                    }
                }
                if self.sync_rt.forgot.is_none()
                    && ui
                        .button(tr(if form.sent {
                            "Email me another code"
                        } else {
                            "Email me a code"
                        }))
                        .clicked()
                {
                    if wb_net::server_url(addr).is_err() {
                        form.note = tr("Type the server's address first.").into();
                    } else if !form.email.contains('@') {
                        form.note = tr("Type the email you sign in with.").into();
                    } else {
                        let (tx, rx) = std::sync::mpsc::channel();
                        let (a, e, ctx) = (addr.clone(), form.email.clone(), ui.ctx().clone());
                        std::thread::spawn(move || {
                            let _ = tx.send(wb_net::forgot_password(&a, &e));
                            ctx.request_repaint();
                        });
                        self.sync_rt.forgot = Some(rx);
                        form.note.clear();
                    }
                }
                if form.sent {
                    field(
                        ui,
                        &mut form.code,
                        "Reset code",
                        "XXXX-XXXX-XXXX from the mail",
                        false,
                    );
                    field(
                        ui,
                        &mut form.password,
                        "New password",
                        &trf!("at least {} characters", wb_net::MIN_PASSWORD),
                        true,
                    );
                    field(
                        ui,
                        &mut form.repeat,
                        "Repeat password",
                        "the same password again",
                        true,
                    );
                }
                if ui.link(tr("Back to sign in")).clicked() {
                    form.mode = Mode::Account;
                    form.note.clear();
                }
            }
            Mode::NewServer => {
                ui.label(RichText::new(tr("This workspace goes up to the empty server and you become its owner. Choose how you sign in on your other computers and phones.")).small().color(t.text3));
                field(ui, &mut form.email, "Email", "you@company.com", false);
                field(
                    ui,
                    &mut form.password,
                    "Choose a password",
                    &trf!("at least {} characters", wb_net::MIN_PASSWORD),
                    true,
                );
                field(
                    ui,
                    &mut form.repeat,
                    "Repeat password",
                    "the same password again",
                    true,
                );
            }
        }
        if !form.note.is_empty() {
            ui.add_space(if big { 8.0 } else { 0.0 });
            ui.label(RichText::new(server_text(&form.note)).color(t.danger));
        }
        ui.add_space(if big { 20.0 } else { 6.0 });
        let shown = form.mode != Mode::Forgot || form.sent;
        let label = tr(if form.mode == Mode::Forgot {
            "Set password and sign in"
        } else {
            button
        });
        if shown && big {
            go = super::login::big_button(ui, t, label).clicked();
        } else if shown {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    go = primary_button(ui, t, label).clicked();
                });
            });
        }
        if go {
            let email_ok = form.email.trim().contains('@');
            let problem = match form.mode {
                Mode::Account if !email_ok => Some(tr("Type your email.").to_string()),
                Mode::Account if form.password.is_empty() => {
                    Some(tr("Type your password.").to_string())
                }
                Mode::Invite if form.code.trim().is_empty() => {
                    Some(tr("Type your invite code.").to_string())
                }
                Mode::Invite if !form.email.trim().is_empty() && !email_ok => {
                    Some(tr("That email doesn't look right.").to_string())
                }
                Mode::Forgot if form.code.trim().is_empty() => {
                    Some(tr("Type the code from the mail.").to_string())
                }
                Mode::NewServer if !email_ok => Some(tr("Type your email.").to_string()),
                Mode::Invite | Mode::Forgot | Mode::NewServer => {
                    new_password_problem(&form.password, &form.repeat)
                }
                _ => None,
            };
            match problem {
                Some(p) => form.note = p,
                None => {
                    form.note.clear();
                    how = Some(match form.mode {
                        Mode::Account => How::Account {
                            email: form.email.clone(),
                            password: form.password.clone(),
                        },
                        Mode::Invite => How::Invite {
                            code: form.code.clone(),
                            email: form.email.clone(),
                            password: form.password.clone(),
                        },
                        Mode::Forgot => How::Reset {
                            email: form.email.clone(),
                            code: form.code.clone(),
                            password: form.password.clone(),
                        },
                        Mode::NewServer => How::Claim {
                            email: form.email.clone(),
                            password: form.password.clone(),
                        },
                    });
                }
            }
        }
        self.sync_rt.form = form;
        how
    }

    /// Signed in on a server: the email you sign in with elsewhere, setting
    /// up a password or changing it.
    pub(super) fn account_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("Your account")).strong());
        let mut form = std::mem::take(&mut self.sync_rt.acct);
        let field = |ui: &mut Ui, text: &mut String, label: &str, secret: bool| {
            ui.horizontal(|ui| {
                // names start at one left edge (add_sized centred them), and
                // the field takes the rest of the row
                ui.allocate_ui_with_layout(
                    vec2(130.0, CONTROL_H),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_min_width(130.0);
                        ui.label(RichText::new(tr(label)).color(t.text2));
                    },
                );
                if secret {
                    secret_edit(ui, t, text, label, "", false);
                    return;
                }
                let r = ui.add(field(text).desired_width(f32::INFINITY));
                let name = format!("{} entry", label);
                r.widget_info(move || {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, &name)
                });
            });
        };
        let mut send = None;
        match self.sync_rt.account.clone() {
            Some(email) => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(crate::i18n::particles(&trf!(
                            "You sign in with {} on your other computers and phones.",
                            email
                        )))
                        .color(t.text2),
                    );
                    if !form.changing && row_button(ui, tr("Change password")).clicked() {
                        form.changing = true;
                        form.note.clear();
                    }
                });
                if form.changing {
                    field(ui, &mut form.old, "Current password", true);
                    field(ui, &mut form.password, "New password", true);
                    field(ui, &mut form.repeat, "Repeat password", true);
                    ui.horizontal(|ui| {
                        if primary_button(ui, t, tr("Save password")).clicked() {
                            match new_password_problem(&form.password, &form.repeat) {
                                Some(p) => form.note = p,
                                None => {
                                    send = Some(NetMsg::SetPassword {
                                        email: email.clone(),
                                        password: form.password.clone(),
                                        old_password: Some(form.old.clone()),
                                    })
                                }
                            }
                        }
                        if ui.button(tr("Cancel")).clicked() {
                            form = Form::default();
                        }
                    });
                }
            }
            None => {
                ui.label(RichText::new(tr("Set a password to sign in on your other computers and phones without a new invite code.")).color(t.text2));
                if form.email.is_empty() {
                    form.email = self
                        .data
                        .member(self.data.me)
                        .map(|m| m.email.clone())
                        .unwrap_or_default();
                }
                field(ui, &mut form.email, "Email", false);
                field(ui, &mut form.password, "Password", true);
                field(ui, &mut form.repeat, "Repeat password", true);
                if primary_button(ui, t, tr("Save password")).clicked() {
                    if !form.email.trim().contains('@') {
                        form.note = tr("Type your email.").into();
                    } else {
                        match new_password_problem(&form.password, &form.repeat) {
                            Some(p) => form.note = p,
                            None => {
                                send = Some(NetMsg::SetPassword {
                                    email: form.email.trim().to_string(),
                                    password: form.password.clone(),
                                    old_password: None,
                                })
                            }
                        }
                    }
                }
            }
        }
        if !form.note.is_empty() {
            ui.label(RichText::new(server_text(&form.note)).color(t.danger));
        }
        self.sync_rt.acct = form;
        if let Some(m) = send {
            self.access_request(m);
        }
    }

    /// Joining a team while this computer already has a workspace (the
    /// workspace menu's "Join a team", or an invite link): the server
    /// address and sign-in or invite code. The team's workspace opens as a
    /// new one and the others stay as they are.
    pub(crate) fn join_team_body(&mut self, ui: &mut Ui, t: &Theme, addr: &mut String) -> bool {
        ui.label(RichText::new(tr("Type your team's server address. Then sign in with your email and password, or use the invite code you were sent.")).color(t.text2));
        ui.label(
            RichText::new(tr("The team's workspace opens as a new workspace. The ones you already have stay in the workspace menu, as they are."))
                .small()
                .color(t.text3),
        );
        ui.add_space(4.0);
        self.server_form(ui, t, addr, &[Mode::Account, Mode::Invite], "Connect");
        false
    }
}
