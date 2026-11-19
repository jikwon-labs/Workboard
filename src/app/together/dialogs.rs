// The Work together dialog and its sections (backups, people, account),
// choosing who you are, and the call dialog.

use super::*;

// ------------------------------------------------------------ dialogs

impl App {
    /// The owner: go back to an earlier copy of the team's workspace.
    fn backups_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("Backups")).strong());
        ui.label(RichText::new(tr("Go back to an earlier copy of the team's workspace. Deleted items come back; anything made since stays. The server keeps a copy from just before.")).small().color(t.text3));
        let Some(list) = self.sync_rt.backups.clone() else {
            if ui.button(tr("Show backups")).clicked() {
                self.access_request(NetMsg::BackupList);
            }
            return;
        };
        if list.is_empty() {
            ui.label(RichText::new(tr("No backups yet. The first one is made within a minute of the server having a workspace.")).color(t.text2));
            return;
        }
        let mut req: Option<NetMsg> = None;
        let mut confirm = self.sync_rt.restore_confirm.clone();
        egui::ScrollArea::vertical()
            .id_salt("backups")
            .max_height(160.0)
            .show(ui, |ui| {
                for b in list.iter().take(80) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(b.label()).color(t.text));
                        ui.label(
                            RichText::new(format!("{} KB", b.bytes.div_ceil(1024)))
                                .small()
                                .color(t.text3),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if confirm.as_deref() == Some(b.name.as_str()) {
                                if row_button(ui, tr("Cancel")).clicked() {
                                    confirm = None;
                                }
                                if row_button(ui, RichText::new(tr("Yes, restore")).color(t.danger))
                                    .clicked()
                                {
                                    req = Some(NetMsg::Restore {
                                        name: b.name.clone(),
                                    });
                                }
                            } else if row_button(ui, tr("Restore"))
                                .on_hover_text(trf!("Go back to {}", b.label()))
                                .clicked()
                            {
                                confirm = Some(b.name.clone());
                            }
                        });
                    });
                }
            });
        self.sync_rt.restore_confirm = confirm;
        if let Some(m) = req {
            self.access_request(m);
        }
    }

    /// Whether I'm an owner or admin by the role in People (on a server,
    /// a copy of the server's; [`Self::may_manage`] asks the server first).
    pub(crate) fn i_manage(&self) -> bool {
        self.data.member(self.data.me).is_some_and(Member::manages)
    }

    /// A member's name, or "Someone" for one not (or no longer) here.
    pub(crate) fn who(&self, id: u64) -> String {
        self.data
            .member(id)
            .map(|m| m.shown().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| tr("Someone").into())
    }

    /// Whether I may bring new people into the workspace and change its
    /// name and colour. On a team server
    /// only its owner and admins: what the server last said, or before it
    /// is reached the role in People (which follows the server's). Alone or
    /// in a shared folder anyone may, as the People page there lets anyone
    /// manage people.
    pub(crate) fn may_manage(&self) -> bool {
        let server = self.on_server();
        if !server {
            return true;
        }
        match self.sync_rt.role {
            Some(r) => r.manages(),
            None => self.i_manage(),
        }
    }

    /// Owners and admins: everyone's access, invite codes, roles.
    fn people_section(&mut self, ui: &mut Ui, t: &Theme) {
        ui.label(RichText::new(tr("People")).strong());
        ui.label(RichText::new(tr("Give each person their invite code (it works once, for 7 days). Add people first with Invite.")).small().color(t.text3));
        let me = self.data.me;
        let my_role = self.sync_rt.role.unwrap_or_default();
        let people = self.sync_rt.people.clone();
        let mut req: Option<NetMsg> = None;
        egui::ScrollArea::vertical()
            .id_salt("people")
            .max_height(220.0)
            .show(ui, |ui| {
                for p in &people {
                    let Some(m) = self.data.member(p.member) else {
                        continue;
                    };
                    let name = m.shown().to_string();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&name).color(t.text));
                        ui.label(RichText::new(tr(p.role.label())).small().color(t.text2));
                        ui.label(
                            RichText::new(match p.devices {
                                0 => tr("not signed in").to_string(),
                                1 => tr("1 computer").to_string(),
                                n => trf!("{} computers", n),
                            })
                            .small()
                            .color(t.text3),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            // another admin is the owner's to change (as the server)
                            let others_admin = p.role == Role::Admin && my_role != Role::Owner;
                            if p.role != Role::Owner && p.member != me && !others_admin {
                                if row_button(
                                    ui,
                                    RichText::new(tr("Remove access")).color(t.danger),
                                )
                                .on_hover_text(trf!("Sign out {}'s computers", name))
                                .clicked()
                                {
                                    req = Some(NetMsg::Revoke { member: p.member });
                                }
                                if my_role.manages() {
                                    let (label, role) = if p.role == Role::Admin {
                                        ("Make member", Role::Member)
                                    } else {
                                        ("Make admin", Role::Admin)
                                    };
                                    if row_button(ui, tr(label)).clicked() {
                                        req = Some(NetMsg::SetRole {
                                            member: p.member,
                                            role,
                                        });
                                    }
                                }
                            }
                            // a code signs in as its person: the owner makes any, an
                            // admin only for members without a password (as the server)
                            let may_manage = p.member != me
                                && (my_role == Role::Owner
                                    || (p.role == Role::Member && !p.account));
                            if may_manage
                                && row_button(
                                    ui,
                                    if p.invite.is_some() {
                                        tr("New code")
                                    } else {
                                        tr("Invite code")
                                    },
                                )
                                .clicked()
                            {
                                req = Some(NetMsg::CreateInvite {
                                    member: p.member,
                                    role: Role::Member,
                                });
                            }
                        });
                    });
                    if let Some(code) = &p.invite {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(trf!("Code for {}: {}", name, code))
                                    .monospace()
                                    .color(t.text),
                            );
                            if row_button(ui, tr("Copy")).clicked() {
                                ui.ctx().copy_text(code.clone());
                                self.toast(tr("Invite code copied"));
                            }
                        });
                    }
                }
            });
        if let Some(m) = req {
            self.access_request(m);
        }
    }

    /// The Work together dialog, filled in with what this workspace uses.
    pub(in crate::app) fn together_modal(&self) -> Modal {
        let cfg = self.data.extra.shared.as_ref();
        Modal::Together {
            path: cfg
                .filter(|c| c.server.is_none())
                .map(|c| c.folder.clone())
                .unwrap_or_default(),
            addr: cfg.and_then(|c| c.server.clone()).unwrap_or_default(),
            code: String::new(),
            result: String::new(),
        }
    }

    pub(crate) fn together_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        path: &mut String,
        addr: &mut String,
        code: &mut String,
        result: &mut String,
    ) -> bool {
        let mut close = false;
        if let Some(SharedCfg {
            server: Some(server),
            ..
        }) = self.data.extra.shared.clone()
        {
            ui.label(
                RichText::new(tr("This workspace is on a server"))
                    .size(15.0)
                    .strong(),
            );
            ui.label(RichText::new(&server).color(t.text2));
            let connected = self.sync_rt.net_status == Some(NetStatus::Connected);
            let status = net_status_text(
                self.sync_rt.net.is_some(),
                self.sync_rt.net_status.as_ref(),
                self.sync_rt.fails,
            );
            let trouble = matches!(
                self.sync_rt.net_status,
                Some(NetStatus::Reconnecting(_) | NetStatus::Refused(_) | NetStatus::Gone(_))
            );
            ui.label(
                RichText::new(trf!("Status: {}", status)).color(if connected {
                    t.text
                } else if trouble {
                    WARN
                } else {
                    t.text2
                }),
            );
            if let Some(why) = net_problem(self.sync_rt.net_status.as_ref()) {
                ui.label(RichText::new(why).small().color(t.text2));
            }
            if let Some(role) = self.sync_rt.role.filter(|_| connected) {
                let name = self
                    .data
                    .member(self.data.me)
                    .map(|m| m.shown().to_string())
                    .unwrap_or_default();
                ui.label(
                    RichText::new(crate::i18n::particles(&trf!(
                        "Signed in as {} ({})",
                        name,
                        tr(role.label())
                    )))
                    .color(t.text2),
                );
            }
            let gone = matches!(self.sync_rt.net_status, Some(NetStatus::Gone(_)));
            if gone {
                ui.add_space(6.0);
                ui.label(RichText::new(tr("What's here stays on this computer until you remove it. Nothing you change here reaches anyone.")).color(t.text2));
                ui.add_space(4.0);
                if ui
                    .add(Tint(
                        egui::Button::new(
                            RichText::new(tr("Remove from this computer")).color(Color32::WHITE),
                        )
                        .fill(t.danger),
                    ))
                    .clicked()
                {
                    self.remove_gone_workspace();
                    return true;
                }
            }
            if matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Sign this computer in again with your email and password, or with a new invite code from the owner or an admin.")).color(t.text2));
                let mut at = server.clone();
                if let Some(how) = self.login_form(
                    ui,
                    t,
                    &mut at,
                    &[Mode::Account, Mode::Invite],
                    "Sign in",
                    false,
                ) {
                    self.sign_in_with(how);
                }
            }
            ui.add_space(8.0);
            let how = if connected {
                tr("Tasks, chat and docs show up for everyone within a second or two.")
            } else if gone {
                tr("The server no longer has this workspace.")
            } else if matches!(self.sync_rt.net_status, Some(NetStatus::Refused(_))) {
                tr("You can keep working here. Your changes are kept on this computer and go up once it's signed in again.")
            } else {
                tr("You can keep working here. Your changes are kept on this computer and go up when the server is back.")
            };
            ui.label(RichText::new(how).small().color(t.text3));
            if !connected && self.sync_rt.editing_addr {
                ui.add_space(8.0);
                ui.label(RichText::new(tr("New server address")).color(t.text2));
                let r = ui.add(
                    egui::TextEdit::singleline(addr)
                        .hint_text("https://wb.example.com")
                        .desired_width(f32::INFINITY),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Server address")
                });
                ui.horizontal(|ui| {
                    if primary_button(ui, t, tr("Save and reconnect")).clicked() {
                        match self.change_server(addr) {
                            Ok(()) => result.clear(),
                            Err(e) => *result = server_text(&e),
                        }
                    }
                    if ui.button(tr("Cancel")).clicked() {
                        self.sync_rt.editing_addr = false;
                        *addr = server.clone();
                        result.clear();
                    }
                });
            }
            if connected {
                ui.add_space(10.0);
                self.account_section(ui, t);
            }
            if connected && self.sync_rt.role.is_some_and(|r| r.manages()) {
                ui.add_space(10.0);
                self.people_section(ui, t);
                if self.sync_rt.role == Some(Role::Owner) {
                    ui.add_space(10.0);
                    self.backups_section(ui, t);
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Reconnect now")).clicked() {
                    self.sync_rt.net = None;
                    self.sync_rt.net_status = None;
                }
                // the server moved or the address was mistyped
                if !connected
                    && !self.sync_rt.editing_addr
                    && ui
                        .button(tr("Change address"))
                        .on_hover_text(tr(
                            "Connect this workspace to the server at another address",
                        ))
                        .clicked()
                {
                    self.sync_rt.editing_addr = true;
                    *addr = server.clone();
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new(tr("Stop sharing")).color(t.danger))
                        .on_hover_text(tr("Keep working on this computer only; your copy stays"))
                        .clicked()
                    {
                        self.stop_sharing();
                        close = true;
                    }
                });
            });
        } else if let Some(cfg) = self.data.extra.shared.clone() {
            ui.label(
                RichText::new(tr("This workspace is shared"))
                    .size(15.0)
                    .strong(),
            );
            ui.add(
                egui::Label::new(RichText::new(short_path(&cfg.folder, 48)).color(t.text2))
                    .wrap_mode(egui::TextWrapMode::Truncate),
            )
            .on_hover_text(&cfg.folder);
            // computers, not people: the people list counts everyone added
            let n = sync::other_files(Path::new(&cfg.folder), &cfg.device).len();
            ui.label(RichText::new(match n {
                0 => tr("No other computer is working in this folder yet. Share the folder with your teammates in your sync app. On their computer they open Work together, paste the folder's path and choose Join this workspace.").to_string(),
                1 => tr("1 other computer is working in it.").to_string(),
                n => trf!("{} other computers are working in it.", n),
            }).color(t.text2));
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Tasks, chat and docs show up for everyone a few seconds after their sync app copies the folder. Calls open in the browser from a link posted in chat.")).small().color(t.text3));
            ui.label(RichText::new(tr(FOLDER_PRIVACY)).small().color(t.text2));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Check for changes now")).clicked() {
                    let changed = self.sync_pull();
                    *result = if changed {
                        tr("Got the latest changes.").into()
                    } else {
                        tr("Everything is up to date.").into()
                    };
                }
                if ui.button(tr("Show folder")).clicked() {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(format!("file://{}", cfg.folder)));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new(tr("Stop sharing")).color(t.danger))
                        .on_hover_text(tr("Keep working on this computer only; your copy stays"))
                        .clicked()
                    {
                        self.stop_sharing();
                        close = true;
                    }
                });
            });
        } else {
            ui.label(tr(
                "Work on this workspace with other people, without a server.",
            ));
            ui.label(RichText::new(tr("Pick a folder that a sync app shares with your teammates (iCloud Drive, Dropbox, Google Drive, OneDrive or Syncthing). Everyone opens Workboard on their own computer and points it at the same folder.")).color(t.text2));
            ui.label(RichText::new(tr(FOLDER_PRIVACY)).small().color(t.text2));
            ui.add_space(8.0);
            let sugg = sync::suggested_folders();
            if !sugg.is_empty() {
                ui.label(
                    RichText::new(tr("Found on this computer"))
                        .small()
                        .color(t.text3),
                );
                ui.horizontal_wrapped(|ui| {
                    for (name, p) in &sugg {
                        if ui.button(name).on_hover_text(p).clicked() {
                            *path = p.clone();
                        }
                    }
                });
            }
            ui.label(RichText::new(tr("Folder")).color(t.text2));
            ui.add(
                field(path)
                    .hint_text(if sugg.is_empty() {
                        tr("Paste a folder path")
                    } else {
                        tr("Paste a folder path, or pick one above")
                    })
                    .desired_width(f32::INFINITY),
            );
            let n = folder_people(path);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if n > 0 {
                    ui.label(RichText::new(if n == 1 { tr("A shared workspace is already here (1 person).").to_string() } else { trf!("A shared workspace is already here ({} people).", n) }).color(t.text2));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, t, tr("Join this workspace")).clicked() {
                            match self.join_shared(path) {
                                Ok(()) => {
                                    self.modal = Some(Modal::WhoAmI { name: String::new(), email: String::new() });
                                }
                                Err(e) => *result = e,
                            }
                        }
                    });
                } else {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, t, tr("Share this workspace here")).clicked() {
                            match self.share_here(path) {
                                Ok(()) => {
                                    self.toast(tr("Shared. Now share the folder with your teammates in your sync app."));
                                    close = true;
                                }
                                Err(e) => *result = e,
                            }
                        }
                    });
                }
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new(tr("Or connect to your team's server")).strong());
            ui.label(RichText::new(tr("A Workboard server (for example https://wb.example.com). Sign in with your email and password, or join with the invite code you were given. Setting up a new, empty server? This workspace goes up and you become its owner.")).color(t.text2));
            let _ = code;
            self.server_form(
                ui,
                t,
                addr,
                &[Mode::Account, Mode::Invite, Mode::NewServer],
                "Connect",
            );
        }
        if !result.is_empty() {
            ui.label(RichText::new(result.as_str()).color(t.text2));
        }
        close
    }

    pub(crate) fn who_am_i_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        name: &mut String,
        email: &mut String,
    ) -> bool {
        let mut close = false;
        ui.label(
            RichText::new(tr(
                "Pick yourself so your messages and tasks show your name.",
            ))
            .color(t.text2),
        );
        ui.add_space(6.0);
        let members: Vec<Member> = self.data.members.clone();
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for m in &members {
                    let label = if m.invited {
                        trf!("{}   {}  (invited)", m.shown(), m.email)
                    } else {
                        format!("{}   {}", m.shown(), m.email)
                    };
                    if ui.add(ghost(egui::Button::new(label))).clicked() {
                        self.become_member(Some(m.id), "", "");
                        close = true;
                    }
                }
            });
        ui.separator();
        ui.label(RichText::new(tr("I'm new here")).color(t.text2));
        ui.add(
            field(name)
                .hint_text(tr("Your name"))
                .desired_width(f32::INFINITY),
        );
        ui.add(
            field(email)
                .hint_text(tr("Email"))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(6.0);
        if primary_button(ui, t, tr("Join as new member")).clicked() && !name.trim().is_empty() {
            self.become_member(None, name, email);
            close = true;
        }
        close
    }

    pub(crate) fn call_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        ch: &mut u64,
        link: &mut String,
    ) -> bool {
        let mut close = false;
        // why it's a browser call rather than a huddle here
        if self.on_server() && self.sync_rt.net_status != Some(NetStatus::Connected) {
            ui.label(RichText::new(tr("The team's server can't be reached, so a huddle in the app isn't available. This call opens in the browser instead, and the others see its Join button once you're connected again.")).color(WARN));
            ui.add_space(6.0);
        }
        ui.label(RichText::new(tr("The call opens in your browser on Jitsi Meet (free). The person who starts it may be asked to sign in with Google, GitHub or Facebook; everyone else joins without an account. Everyone in the conversation gets a Join button.")).color(t.text2));
        // nobody else to get the Join button
        let me = self.data.me;
        let alone = self
            .data
            .channels
            .iter()
            .find(|c| c.id == *ch)
            .is_some_and(|c| {
                !c.members
                    .iter()
                    .any(|id| *id != me && self.data.member(*id).is_some_and(|m| !m.invited))
            });
        if alone {
            ui.label(RichText::new(tr("You're the only one in this conversation, so nobody else gets the Join button. Invite people first.")).small().color(t.text3));
        }
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Call in")).color(t.text2));
        let chans: Vec<(u64, String)> = self
            .data
            .channels
            .iter()
            .filter(|c| self.listed(c))
            .map(|c| {
                (
                    c.id,
                    if c.kind == ChannelKind::Channel {
                        format!("# {}", c.name)
                    } else {
                        self.channel_label(c)
                    },
                )
            })
            .collect();
        let cur = chans
            .iter()
            .find(|c| c.0 == *ch)
            .map(|c| c.1.clone())
            .unwrap_or_else(|| tr("Choose").into());
        // as wide as the link field below it
        egui::ComboBox::from_id_salt("call_in")
            .selected_text(cur)
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (id, n) in &chans {
                    ui.selectable_value(ch, *id, n);
                }
            });
        ui.add_space(6.0);
        ui.label(
            RichText::new(tr("Or use your own meeting link (Zoom, Google Meet...)"))
                .small()
                .color(t.text3),
        );
        ui.add(
            field(link)
                .hint_text(tr("https://..."))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary_button(ui, t, tr("Start call")).clicked()
                    && chans.iter().any(|c| c.0 == *ch)
                {
                    let ctx = ui.ctx().clone();
                    self.start_call(Some(&ctx), *ch, link);
                    close = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    close = true;
                }
            });
        });
        close
    }

    /// The call to open from a Huddle / call button: the open conversation, else #general.
    pub(crate) fn open_call_dialog(&mut self) {
        let ch = if self.section == Section::Chat {
            self.chat.open
        } else {
            None
        }
        .or_else(|| {
            self.data
                .channels
                .iter()
                .find(|c| c.kind == ChannelKind::Channel)
                .map(|c| c.id)
        })
        .unwrap_or(0);
        // on the team's server: a voice huddle right here
        if self.huddles_on_server() && ch != 0 {
            self.start_huddle(ch);
            return;
        }
        self.modal = Some(Modal::Call {
            ch,
            link: String::new(),
        });
    }
}
