// Deleting a workspace.
//
// From Workspace settings, after typing the workspace's name. What goes
// depends on where the workspace lives:
//
// - only on this computer: its file is deleted;
// - in a shared folder: this computer's copy is deleted (the folder and the
//   others' copies stay);
// - on a team server: an owner deletes it there for everyone
//   (wb_net::delete_workspace). The server keeps it 30 days
//   (`workboard-server undelete`), but every app loses it at once: the ones
//   connected hear it (wb_net::Status::Gone) and show a way to remove their
//   copy. The server's first workspace can't be deleted.
//
// Afterwards another workspace on this computer opens, or the sign-in
// screen when none is left.

use super::together::{load_token, server_text};
use super::*;
use crate::sync::SharedCfg;

/// How a workspace can be deleted from here, and by whom.
#[derive(Clone, Debug, PartialEq)]
pub enum DeleteHow {
    /// Only on this computer: its file goes.
    Here,
    /// In a shared folder: this computer's copy goes.
    Folder,
    /// On a team server, as its owner: deleted there for everyone.
    Server(String),
}

/// A delete asked of the team server, waiting for its answer.
pub struct DeletingWs {
    rx: std::sync::mpsc::Receiver<Result<(), String>>,
    /// the workspace file it is for (it may no longer be open)
    file: String,
    name: String,
}

impl App {
    /// How the open workspace can be deleted, or why it can't (None and the
    /// reason, None and None when there's nothing to say: a plain member).
    pub(super) fn delete_how(&self) -> Result<DeleteHow, Option<&'static str>> {
        let Some(SharedCfg { server, .. }) = &self.data.extra.shared else {
            return Ok(DeleteHow::Here);
        };
        let Some(addr) = server else {
            return Ok(DeleteHow::Folder);
        };
        match self.sync_rt.role {
            Some(wb_net::Role::Owner) => {}
            Some(_) => return Err(None),
            None => return Err(Some("Connect to the team server to delete this workspace.")),
        }
        if !addr.contains("/w/") {
            return Err(Some(
                "This is the team server's first workspace. It can't be deleted.",
            ));
        }
        Ok(DeleteHow::Server(addr.clone()))
    }

    /// The bottom of Workspace settings: the way to delete it.
    pub(super) fn delete_workspace_row(&mut self, ui: &mut Ui, t: &Theme) -> bool {
        let how = self.delete_how();
        if how == Err(None) {
            return false;
        }
        ui.add_space(10.0);
        ui.separator();
        ui.label(RichText::new(tr("Delete workspace")).strong());
        match how {
            Err(why) => {
                if let Some(why) = why {
                    ui.label(RichText::new(tr(why)).small().color(t.text2));
                }
                false
            }
            Ok(how) => {
                let says = match how {
                    DeleteHow::Here => tr("Everything in it is deleted from this computer."),
                    DeleteHow::Folder => tr("This computer's copy is deleted. The shared folder and your teammates' copies stay."),
                    DeleteHow::Server(_) => tr("It is deleted from the team server for everyone."),
                };
                ui.label(RichText::new(says).small().color(t.text2));
                ui.add_space(4.0);
                ui.add(Tint(
                    egui::Button::new(
                        RichText::new(tr("Delete workspace...")).color(Color32::WHITE),
                    )
                    .fill(t.danger),
                ))
                .clicked()
            }
        }
    }

    /// Asking before deleting: what goes, and the workspace's name typed in.
    pub(super) fn delete_workspace_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        typed: &mut String,
    ) -> bool {
        let mut close = false;
        let name = self.data.workspace.clone();
        let Ok(how) = self.delete_how() else {
            ui.label(tr("This workspace can't be deleted from here."));
            return ui.button(tr("Close")).clicked();
        };
        let busy = self.deleting_ws.is_some();
        ui.label(RichText::new(trf!("Delete {}?", name)).color(t.text));
        ui.add_space(4.0);
        let says = match how {
            DeleteHow::Here => tr("Its tasks, chats, docs and everything else are deleted from this computer. This can't be undone."),
            DeleteHow::Folder => tr("This computer's copy is deleted and this computer stops syncing with the shared folder. The folder and your teammates' copies stay."),
            DeleteHow::Server(_) => tr("It is deleted from the team server for everyone: their apps lose it too. The server keeps a copy for 30 days, which whoever runs the server can bring back."),
        };
        ui.label(RichText::new(says).color(t.text2));
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Type the workspace's name to confirm.")).color(t.text2));
        let r = ui.add_enabled(
            !busy,
            field(typed)
                .hint_text(name.as_str())
                .desired_width(f32::INFINITY),
        );
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Workspace name to delete")
        });
        let ready = typed.trim() == name.trim() && !busy;
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let label = if busy {
                tr("Deleting...")
            } else {
                tr("Delete")
            };
            if ui
                .add_enabled(
                    ready,
                    Tint(
                        egui::Button::new(RichText::new(label).color(Color32::WHITE))
                            .fill(t.danger),
                    ),
                )
                .clicked()
            {
                match how {
                    DeleteHow::Here | DeleteHow::Folder => {
                        self.leave_deleted_workspace();
                        self.toast(trf!("Deleted {}", name));
                        close = true;
                    }
                    DeleteHow::Server(addr) => self.delete_on_server(&addr, ui.ctx().clone()),
                }
            }
            if ui
                .add_enabled(!busy, egui::Button::new(tr("Cancel")))
                .clicked()
            {
                close = true;
            }
        });
        close
    }

    /// Ask the team server to delete the open workspace; deleting_ws_tick
    /// takes the answer.
    pub(super) fn delete_on_server(&mut self, addr: &str, ctx: egui::Context) {
        let Some(cfg) = self.data.extra.shared.clone() else {
            return;
        };
        let Some(token) = load_token(addr, &cfg.device) else {
            self.toast(tr(
                "Sign in to the team server again, then delete the workspace.",
            ));
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let auth = wb_net::Auth {
            device: cfg.device,
            token,
        };
        let addr = addr.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(wb_net::delete_workspace(&addr, &auth));
            ctx.request_repaint();
        });
        self.deleting_ws = Some(DeletingWs {
            rx,
            file: self.ws_file.clone(),
            name: self.data.workspace.clone(),
        });
    }

    /// The server answered a delete_on_server.
    pub(super) fn deleting_ws_tick(&mut self) {
        let Some(d) = &self.deleting_ws else {
            return;
        };
        let answer = match d.rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err("The server had a problem. Try again.".into())
            }
        };
        let d = self.deleting_ws.take().expect("checked above");
        match answer {
            Err(e) => {
                self.toast(trf!("Couldn't delete the workspace: {}", server_text(&e)));
            }
            Ok(()) => {
                if self.ws_file == d.file {
                    self.leave_deleted_workspace();
                    self.modal = None;
                } else {
                    // another one opened meanwhile: only the file goes
                    self.remove_workspace_file(&d.file);
                }
                self.toast(trf!("Deleted {}", d.name));
            }
        }
    }

    /// The open workspace was deleted on its server by its owner (seen from
    /// another computer): remove this computer's copy too.
    pub(super) fn remove_gone_workspace(&mut self) {
        let name = self.data.workspace.clone();
        self.leave_deleted_workspace();
        self.modal = None;
        self.toast(trf!("Removed {} from this computer", name));
    }

    /// Delete a workspace file that isn't open, and its line in the list.
    fn remove_workspace_file(&mut self, file: &str) {
        self.ws_places.clear();
        if !self.persist {
            return;
        }
        let path = if file == "data.json" {
            Data::path()
        } else {
            Data::dir().join(file)
        };
        // a save still waiting would put the deleted copy back
        if file == self.ws_file {
            self.save_due.set(None);
        }
        Data::finish_saving();
        let _ = std::fs::remove_file(path);
        let mut list = Data::workspaces();
        list.retain(|w| w.file != file);
        Data::save_workspaces(&list);
    }

    /// Drop the open workspace from this computer (its file, sign-in and
    /// sync) and open another one, or the sign-in screen when none is left.
    pub(super) fn leave_deleted_workspace(&mut self) {
        let file = self.ws_file.clone();
        self.finish_status_edit();
        self.stop_sharing();
        // pictures waiting to go up belong to the copy deleted
        self.task_files.clear();
        self.task_files_saved = None;
        self.remove_workspace_file(&file);
        // the next one: the first left in the list, else this computer's own
        let next = if self.persist {
            Data::workspaces()
                .into_iter()
                .map(|w| w.file)
                .find(|f| *f != file && Data::dir().join(f).is_file())
                .or_else(|| {
                    (file != "data.json" && Data::path().is_file()).then(|| "data.json".to_string())
                })
        } else {
            None
        };
        match next {
            Some(f) => {
                self.open_workspace(&f);
                if self.ws_file != f {
                    // it couldn't be read: start over like with none left
                    self.start_over_after_delete();
                }
            }
            None => self.start_over_after_delete(),
        }
        self.dirty = false;
    }

    /// No workspace left on this computer: a new empty one in data.json and
    /// the sign-in screen, as on a first start.
    fn start_over_after_delete(&mut self) {
        let mut d = Data::fresh();
        d.fix_activity_ids();
        d.fold_legacy_slack();
        super::together::fill_sync_baseline(&mut d);
        d.extra.welcome = true;
        self.data = d;
        self.data_changed();
        self.ws_file = "data.json".into();
        if self.persist {
            self.save_now();
            Data::set_last_workspace("data.json");
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
        self.chat.thread_window = None;
        self.recent.clear();
        self.clear_history();
        self.palette = None;
        self.quick_add = None;
        self.modal = None;
        self.notify_rt.seeded = false;
        self.login = Some(super::login::LoginScreen::start(
            &super::login::last_server(),
        ));
    }
}
