// Dialogs about the workspace and the people in it: invite, workspace
// settings, a new workspace, profile, status, share and switching user.

use super::*;

impl App {
    /// The Invite dialog. Returns true when it should close.
    pub(super) fn invite_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        emails: &mut String,
        role: &mut String,
    ) -> bool {
        let mut close = false;
        ui.label(RichText::new(tr("Email")).color(t.text2));
        let r = ui.add(
            field(emails)
                .hint_text(tr("Enter email, comma or space separated"))
                .desired_width(f32::INFINITY),
        );
        r.request_focus();
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Invite as")).color(t.text2));
        let desc = ROLES
            .iter()
            .find(|r| r.0 == role.as_str())
            .map(|r| r.1)
            .unwrap_or("");
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter().rect_filled(r, 8.0, t.hover);
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                "\u{1F464}",
                FontId::proportional(14.0),
                t.text2,
            );
            ui.vertical(|ui| {
                egui::ComboBox::from_id_salt("invite_role")
                    .selected_text(role_text(role))
                    .show_ui(ui, |ui| {
                        // no Guest: nothing limits what a guest can see yet
                        for (name, _) in ROLES.iter().take(2) {
                            ui.selectable_value(role, name.to_string(), role_text(name));
                        }
                    });
                ui.label(RichText::new(tr(desc)).small().color(t.text2));
            });
        });
        ui.add_space(8.0);
        let shared = self.data.extra.shared.is_some();
        let on_server = self.on_server();
        // codes are made by the server, by its owner and admins
        let connected = self.sync_rt.net_status == Some(wb_net::Status::Connected);
        let manages = self.sync_rt.role.is_some_and(|r| r.manages());
        let note = if on_server && !connected {
            tr("The team's server can't be reached right now. The invites are kept, and their invite codes are asked for once it's back.")
        } else if on_server && !manages {
            tr("This workspace is on a server. The owner or an admin gives each person their invite code.")
        } else if on_server {
            tr("This workspace is on a server: each person gets an invite code by email when the server has email set up. Otherwise give them their code from Work together > People.")
        } else if shared {
            tr("This workspace is shared, so invited people can join it from the shared folder.")
        } else {
            tr("To work with them, share this workspace in a folder first.")
        };
        // wrapped: in a plain row the long note pushed the dialog past its width
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(note).small().color(t.text2));
            if ui
                .link(RichText::new(tr("Work together")).small())
                .clicked()
            {
                self.modal = Some(self.together_modal());
            }
        });
        ui.add_space(4.0);
        ui.separator();
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let valid = emails_in(emails);
                let send = ui.add_enabled(!valid.is_empty(), Tint(primary_btn(t, tr("Send invite"))));
                if send.clicked() {
                    self.invite_people(&valid, role);
                    self.toast(if on_server && !connected { crate::trf!("Invited {} people. Their invite codes are asked for once the server is back.", valid.len()) } else if on_server && !manages { crate::trf!("Invited {} people. The owner or an admin gives them their invite codes.", valid.len()) } else if on_server { crate::trf!("Invited {} people. Their invite codes are on the way (Work together > People shows them too).", valid.len()) } else if self.data.extra.shared.is_some() { crate::trf!("Invited {} people. When they join the shared folder they pick their name.", valid.len()) } else { crate::trf!("Invited {} people. Share the workspace (Work together) so they can join.", valid.len()) });
                    self.dirty = true;
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// The WorkspaceSettings dialog. Returns true when it should close.
    pub(super) fn workspace_settings_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        name: &mut String,
    ) -> bool {
        let mut close = false;
        ui.label(tr("Workspace name"));
        ui.add(field(name).desired_width(f32::INFINITY));
        ui.add_space(6.0);
        ui.label(tr("Workspace color"));
        // the current colour first when it isn't one of these (the
        // default a new workspace starts with is not), so it shows chosen
        let current = self.data.extra.workspace_color;
        let mut colors: Vec<[u8; 3]> = STATUS_PALETTE[1..].to_vec();
        if !colors.contains(&current) {
            colors.insert(0, current);
        }
        ui.horizontal_wrapped(|ui| {
            for c in colors.iter() {
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                ui.painter().rect_filled(r.shrink(2.0), 5.0, rgb(*c));
                hover_ring(ui, &resp, r.shrink(1.0), 6.0);
                if self.data.extra.workspace_color == *c {
                    ui.painter().rect_stroke(
                        r,
                        6.0,
                        Stroke::new(2.0_f32, t.text),
                        egui::StrokeKind::Inside,
                    );
                }
                if resp.clicked() {
                    self.data.extra.workspace_color = *c;
                    self.dirty = true;
                }
            }
        });
        ui.add_space(6.0);
        // archived tasks are not in the count, but say how many there are
        let live = self
            .data
            .tasks
            .iter()
            .filter(|x| self.data.task_live(x))
            .count();
        let archived = self.data.tasks.len() - live;
        let mut counts = crate::trf!(
            "{} people \u{2022} {} spaces \u{2022} {} tasks",
            self.data.members.len(),
            self.open_spaces().len(),
            live
        );
        if archived > 0 {
            counts += &crate::trf!(" \u{2022} {} archived", archived);
        }
        ui.label(RichText::new(counts).color(t.text2));
        ui.add_space(8.0);
        if primary_button(ui, t, tr("Save")).clicked() && !name.trim().is_empty() {
            self.data.workspace = name.trim().to_string();
            let mut list = Data::workspaces();
            if let Some(w) = list.iter_mut().find(|w| w.file == self.ws_file) {
                w.name = self.data.workspace.clone();
                Data::save_workspaces(&list);
            }
            self.dirty = true;
            close = true;
        }
        if self.delete_workspace_row(ui, t) {
            self.modal = Some(Modal::DeleteWorkspace {
                typed: String::new(),
            });
            close = true;
        }
        close
    }

    /// The NewWorkspace dialog. Returns true when it should close.
    pub(super) fn new_workspace_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        step: &mut u8,
        name: &mut String,
        apps: &mut Vec<String>,
        emails: &mut String,
        role: &mut String,
        on_server: &mut bool,
    ) -> bool {
        let mut close = false;
        let server = self.server_for_new_workspace();
        let on_server_now = *on_server && server.is_some();
        let steps = new_ws_steps(on_server_now);
        *step = (*step).min(steps - 1);
        // where the setup is: one bar per step, filled up to this one
        ui.horizontal(|ui| {
            let gap = 6.0;
            ui.spacing_mut().item_spacing.x = gap;
            let n = steps as f32;
            let w = (ui.available_width() - gap * (n - 1.0)) / n;
            for i in 0..steps {
                let (r, _) = ui.allocate_exact_size(vec2(w, 4.0), Sense::hover());
                let c = if i <= *step { t.primary } else { t.border };
                ui.painter().rect_filled(r, 2.0, c);
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(crate::trf!("Step {} of {}", *step + 1, steps))
                .small()
                .color(t.text2),
        );
        ui.add_space(2.0);
        let mut enter = false;
        match *step {
            0 => {
                ui.label(RichText::new(tr("Name your workspace")).size(15.0).strong());
                ui.add_space(4.0);
                let r = ui.add(
                    field(name)
                        .hint_text(tr("e.g. Acme Inc."))
                        .desired_width(f32::INFINITY),
                );
                enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                r.request_focus();
                ui.label(
                    RichText::new(tr(
                        "You'll be its Owner. Switch between workspaces from the workspace menu.",
                    ))
                    .small()
                    .color(t.text2),
                );
                if let Some(addr) = &server {
                    ui.add_space(8.0);
                    ui.label(RichText::new(tr("Where")).color(t.text2));
                    let host = wb_net::server_part(addr);
                    if radio(ui, *on_server, trf!("On the team server ({})", host)).clicked() {
                        *on_server = true;
                    }
                    if radio(ui, !*on_server, tr("On this computer only")).clicked() {
                        *on_server = false;
                    }
                    ui.label(
                        RichText::new(tr(if *on_server {
                            "Your team signs in to it on the same server, with their own invite."
                        } else {
                            "You can share it from Work together later."
                        }))
                        .small()
                        .color(t.text2),
                    );
                }
            }
            1 => {
                ui.label(
                    RichText::new(tr("Which apps do you want on the left bar?"))
                        .size(15.0)
                        .strong(),
                );
                ui.label(
                    RichText::new(tr("You can change them later from Customize navigation."))
                        .small()
                        .color(t.text2),
                );
                ui.add_space(4.0);
                egui::Grid::new("new_ws_apps")
                    .num_columns(2)
                    .spacing([24.0, 6.0])
                    .show(ui, |ui| {
                        for (i, label) in RAIL_APPS.iter().enumerate() {
                            let mut on = apps.iter().any(|a| a == label);
                            if check(ui, &mut on, tr(label)).changed() {
                                // kept in the order the bar shows them
                                let mut chosen: Vec<String> = RAIL_APPS
                                    .iter()
                                    .filter(|a| {
                                        if *a == label {
                                            on
                                        } else {
                                            apps.iter().any(|x| x == *a)
                                        }
                                    })
                                    .map(|a| a.to_string())
                                    .collect();
                                std::mem::swap(apps, &mut chosen);
                            }
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
            }
            _ => {
                ui.label(
                    RichText::new(tr("Invite people to your workspace"))
                        .size(15.0)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(RichText::new(tr("Email")).color(t.text2));
                let r = ui.add(
                    field(emails)
                        .hint_text(tr("Enter email, comma or space separated"))
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Invite as")).color(t.text2));
                egui::ComboBox::from_id_salt("new_ws_role")
                    .selected_text(role_text(role))
                    .show_ui(ui, |ui| {
                        // no Guest: nothing limits what a guest can see yet
                        for (n, _) in ROLES.iter().take(2) {
                            ui.selectable_value(role, n.to_string(), role_text(n));
                        }
                    });
                ui.add_space(4.0);
                ui.label(
                    RichText::new(tr(
                        "The server emails each of them an invite code once the workspace is up.",
                    ))
                    .small()
                    .color(t.text2),
                );
            }
        }
        ui.add_space(8.0);
        ui.separator();
        let last = *step + 1 == steps;
        let mut finish: Option<Vec<String>> = None;
        ui.horizontal(|ui| {
            if *step > 0 && secondary_button(ui, tr("Back")).clicked() {
                *step -= 1;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if last && !on_server_now {
                    if primary_button(ui, t, tr("Create")).clicked() {
                        finish = Some(vec![]);
                    }
                } else if last {
                    let invite = emails_in(emails);
                    let label = if invite.is_empty() {
                        tr("Create Workspace")
                    } else {
                        tr("Create and invite")
                    };
                    if primary_button(ui, t, label).clicked() {
                        finish = Some(invite);
                    }
                    if ui.button(tr("Skip for now")).clicked() {
                        finish = Some(vec![]);
                    }
                } else {
                    let ready = *step > 0 || !name.trim().is_empty();
                    let next = ui.add_enabled(ready, Tint(primary_btn(t, tr("Next"))));
                    if ready && (next.clicked() || enter) {
                        *step += 1;
                    }
                }
            });
        });
        if let Some(invite) = finish {
            if on_server_now {
                let ctx = ui.ctx().clone();
                self.make_workspace_on_server(name.trim(), apps, &invite, role, Some(ctx));
            } else {
                self.create_workspace(name.trim(), apps, &invite, role);
            }
            close = true;
        }
        close
    }

    /// The SetStatus dialog. Returns true when it should close.
    pub(super) fn set_status_body(&mut self, ui: &mut Ui, t: &Theme, text: &mut String) -> bool {
        let mut close = false;
        let r = ui.add(
            field(text)
                .hint_text(tr("What's your status?"))
                .desired_width(f32::INFINITY),
        );
        name_field(&r, "Status text");
        ui.horizontal_wrapped(|ui| {
            // the emoji stays part of the status people see; the button
            // draws a line icon and the words
            for (kind, s) in [
                ("calendar", tr("\u{1F4C5} In a meeting")),
                ("bus", tr("\u{1F68C} Commuting")),
                ("frown", tr("\u{2639} Out sick")),
                ("plane", tr("\u{1F334} Vacationing")),
                ("home", tr("\u{1F3E0} Working remotely")),
            ] {
                let words = s.split_once(' ').map_or(s, |x| x.1);
                if icon_button(ui, t, kind, words, s, true).clicked() {
                    *text = s.to_string();
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if primary_button(ui, t, tr("Save")).clicked() {
                let me = self.data.me;
                if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me) {
                    m.status_text = text.trim().to_string();
                }
                self.dirty = true;
                close = true;
            }
            if ui.button(tr("Clear status")).clicked() {
                let me = self.data.me;
                if let Some(m) = self.data.members.iter_mut().find(|m| m.id == me) {
                    m.status_text.clear();
                }
                self.dirty = true;
                close = true;
            }
        });
        close
    }

    /// The Share dialog. Returns true when it should close.
    pub(super) fn share_body(&mut self, ui: &mut Ui, t: &Theme, emails: &mut String) -> bool {
        let mut close = false;
        // the built-in views' names are the app's words, the rest are names people chose
        let name = match self.loc {
            Location::Space(_) | Location::Folder(_) | Location::List(_) => {
                self.data.location_name(self.loc)
            }
            l => tr(&self.data.location_name(l)).to_string(),
        };
        ui.label(crate::trf!(
            "Share \"{}\" with people in your workspace.",
            name
        ));
        let may_manage = self.may_manage();
        if may_manage && !self.data.members.iter().any(|m| m.id != self.data.me) {
            ui.label(
                RichText::new(tr("Only you are in this Workspace so far. Invite people by email below to work on it together."))
                    .small()
                    .color(t.text3),
            );
        }
        ui.add_space(4.0);
        for m in self.data.members.clone() {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                paint_avatar(ui.painter(), r.center(), 10.0, &m, t.card);
                ui.label(m.shown());
                ui.label(
                    RichText::new(if m.invited {
                        tr("Invited")
                    } else {
                        role_text(&m.role)
                    })
                    .small()
                    .color(t.text3),
                );
            });
        }
        ui.separator();
        // bringing new people in is for the owner and admins on a server
        if may_manage {
            ui.label(tr("Invite by email"));
            ui.add(
                field(emails)
                    .hint_text(tr("name@company.com"))
                    .desired_width(f32::INFINITY),
            );
        }
        ui.horizontal(|ui| {
            if may_manage && primary_button(ui, t, tr("Invite")).clicked() && emails.contains('@') {
                self.modal = Some(Modal::Invite {
                    emails: emails.clone(),
                    role: "Member".into(),
                });
                close = true;
            }
            if icon_button(ui, t, "link", tr("Copy link"), "Copy link", true).clicked() {
                // the same kind of link the app opens (search, messages, macOS)
                self.copy_link(ui, super::nav::location_link(self.loc));
            }
        });
        close
    }

    /// The SwitchUser dialog. Returns true when it should close.
    pub(super) fn switch_user_on_server_body(&mut self, ui: &mut Ui, _t: &Theme) -> bool {
        // the server knows this computer as the person who signed in
        // on it: being someone else takes their sign-in
        let name = self
            .data
            .member(self.data.me)
            .map(|m| m.shown().to_string())
            .unwrap_or_default();
        ui.label(crate::trf!("This computer is signed in to the team's server as {}. Each person uses Workboard on their own computer with their own sign-in.", name));
        ui.add_space(6.0);
        if ui.link(tr("Work together")).clicked() {
            self.modal = Some(self.together_modal());
        }
        false
    }

    /// The SwitchUser dialog. Returns true when it should close.
    pub(super) fn switch_user_body(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        let mut close = false;
        ui.label(tr(
            "Who's using the app? Everyone on this computer shares the same workspace.",
        ));
        if self.data.solo() {
            // alone there's no one to switch to yet: say where people come from
            ui.label(
                RichText::new(tr(
                    "Only you are in this workspace. People you invite show up here.",
                ))
                .color(t.text3),
            );
        }
        ui.add_space(4.0);
        for m in self.data.members.clone() {
            let ring = t.card;
            let label = if m.id == self.data.me {
                crate::trf!("{} (current)", m.shown())
            } else if m.invited {
                crate::trf!("{} (invited)", m.shown())
            } else {
                m.shown().to_string()
            };
            if menu_row(ui, t, &label, m.id == self.data.me, |p, c| {
                paint_avatar(p, c, 9.0, &m, ring)
            })
            .clicked()
            {
                self.data.me = m.id;
                if let Some(mm) = self.data.members.iter_mut().find(|x| x.id == m.id) {
                    mm.invited = false;
                }
                self.dirty = true;
                close = true;
            }
        }
        close
    }
}
