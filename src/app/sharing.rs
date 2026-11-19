// Sharing a space with chosen people (Share & permissions in a space's
// menu) and what the app does where someone may only view or comment: the
// space's menu shrinks, task and doc editing is off, edits are turned down
// with a word, and a small note says so. The team server is what enforces
// it (server/src/spaces.rs); this only keeps people from making edits it
// would put back. Alone or in a shared folder nothing is enforced and every
// space stays editable, which the dialog says.

use super::parity::close_any;
use super::*;
use crate::sync::Level;

/// A level as the dialog shows it.
pub(super) fn level_label(l: Level) -> &'static str {
    match l {
        Level::Full => "Full",
        Level::Edit => "Edit",
        Level::Comment => "Comment",
        Level::View => "View",
    }
}

fn level_help(l: Level) -> &'static str {
    match l {
        Level::Full => "Edit, and change the space and who it's shared with",
        Level::Edit => "Change lists, tasks and docs",
        Level::Comment => "Read, and comment on tasks",
        Level::View => "Read only",
    }
}

impl App {
    /// On a team server, the only place sharing is enforced.
    pub(super) fn sharing_enforced(&self) -> bool {
        self.on_server()
    }

    /// What I may do in space `sid` (Full unless a team server enforces sharing).
    pub(super) fn space_level(&self, sid: u64) -> Level {
        if !self.sharing_enforced() {
            return Level::Full;
        }
        let Some(s) = self.data.spaces.iter().find(|s| s.id == sid) else {
            return Level::Full;
        };
        let v = serde_json::json!({"private_owner": s.private_owner, "sharing": s.sharing});
        let manages = self.sync_rt.role.is_some_and(|r| r.manages());
        crate::sync::space_level(&v, self.data.me, manages).unwrap_or(Level::View)
    }

    pub(super) fn list_level(&self, list: u64) -> Level {
        self.data
            .space_of_list(list)
            .map_or(Level::Full, |s| self.space_level(s.id))
    }

    /// For a doc: the space of the page it sits under.
    pub(super) fn doc_level(&self, doc: u64) -> Level {
        let root = self.doc_root(doc);
        self.data
            .docs
            .iter()
            .find(|d| d.id == root)
            .and_then(|d| d.space)
            .map_or(Level::Full, |s| self.space_level(s))
    }

    pub(super) fn loc_level(&self, loc: Location) -> Level {
        match loc {
            Location::Space(s) => self.space_level(s),
            Location::Folder(f) => self
                .data
                .spaces
                .iter()
                .find(|s| s.folders.iter().any(|x| x.id == f))
                .map_or(Level::Full, |s| self.space_level(s.id)),
            Location::List(l) => self.list_level(l),
            _ => Level::Full,
        }
    }

    /// Only my own comments changed between two versions of a task (what
    /// Comment allows; its `updated` moves along).
    fn only_my_comments(&self, old: &Task, new: &Task) -> bool {
        let me = self.data.me;
        let bare = |t: &Task| {
            let mut t = t.clone();
            t.activity.clear();
            t.updated = 0;
            serde_json::to_value(&t).ok()
        };
        if bare(old) != bare(new) {
            return false;
        }
        let mine = |a: &Activity| a.comment && a.author == me;
        let same = |a: &Activity, b: &Activity| {
            serde_json::to_value(a).ok() == serde_json::to_value(b).ok()
        };
        for a in &new.activity {
            match old.activity.iter().find(|b| b.id == a.id && a.id != 0) {
                Some(b) if same(a, b) => {}
                Some(b) => {
                    if !(mine(a) && mine(b)) {
                        return false;
                    }
                }
                None => {
                    if !mine(a) {
                        return false;
                    }
                }
            }
        }
        old.activity
            .iter()
            .all(|b| mine(b) || new.activity.iter().any(|a| a.id == b.id))
    }

    /// Why an edit can't be made here (in a space shared with me at a lower
    /// level), or None. The edit is dropped and the reason shown.
    pub(super) fn refused_here(&self, a: &Action) -> Option<&'static str> {
        let view_only = "You can only view this space.";
        let comment_only = "You can only view and comment in this space.";
        let why = |l: Level| match l {
            Level::Comment => comment_only,
            _ => view_only,
        };
        let task_level = |id: u64| {
            self.task(id)
                .map_or(Level::Full, |t| self.list_level(t.list_id))
        };
        match a {
            Action::Replace(new) => {
                let old = self.task(new.id)?;
                let l = self
                    .list_level(old.list_id)
                    .min(self.list_level(new.list_id));
                match l {
                    Level::Full | Level::Edit => None,
                    Level::Comment if self.only_my_comments(old, new) => None,
                    l => Some(why(l)),
                }
            }
            Action::Add { list, .. } => {
                (self.list_level(*list) < Level::Edit).then(|| why(self.list_level(*list)))
            }
            Action::DeleteTask(id) | Action::ToggleTimer(id) | Action::StopOldTimer(id) => {
                let l = task_level(*id);
                (l < Level::Edit).then(|| why(l))
            }
            Action::NewFolder(s) | Action::NewList { space: s, .. } => {
                let l = self.space_level(*s);
                (l < Level::Edit).then(|| why(l))
            }
            Action::StartRename(tg) | Action::AskDelete(tg) => {
                let (l, need) = match tg {
                    Target::Space(s) => (self.space_level(*s), Level::Full),
                    Target::Folder(f) => (self.loc_level(Location::Folder(*f)), Level::Edit),
                    Target::List(l) => (self.list_level(*l), Level::Edit),
                    Target::Task(t) => (task_level(*t), Level::Edit),
                };
                (l < need).then(|| match l {
                    Level::Edit => "Only people with Full access can change the space itself.",
                    l => why(l),
                })
            }
            Action::EditStatuses(s) | Action::SetSpaceColor(s, _) => {
                let l = self.space_level(*s);
                (l < Level::Full).then(|| match l {
                    Level::Edit => "Only people with Full access can change the space itself.",
                    l => why(l),
                })
            }
            _ => None,
        }
    }

    /// The small note in a place I can only view or comment in.
    pub(super) fn level_note(&self, ui: &mut Ui, t: &Theme, level: Level) {
        let (text, hover) = match level {
            Level::View => (
                "View only",
                "This space is shared with you to read. Ask someone with Full access for more.",
            ),
            Level::Comment => (
                "Can comment",
                "This space is shared with you to read and comment on tasks.",
            ),
            _ => return,
        };
        egui::Frame::new()
            .fill(t.hover)
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(6, 1))
            .show(ui, |ui| {
                ui.label(RichText::new(tr(text)).small().color(t.text2))
                    .on_hover_text(tr(hover));
            });
    }

    /// The space menu for someone without Full access to the space.
    pub(super) fn limited_space_menu(&mut self, ui: &mut Ui, sid: u64, level: Level) {
        let t = self.t();
        ui.set_min_width(200.0);
        let loc = Location::Space(sid);
        let fav = if self.data.favorites.contains(&loc) {
            "Remove from Favorites"
        } else {
            "Add to Favorites"
        };
        if ui.button(tr(fav)).clicked() {
            self.actions.push(Action::ToggleFavorite(loc));
            close_any(ui);
        }
        if ui.button(tr("Copy link")).clicked() {
            self.copy_link(ui, nav::location_link(Location::Space(sid)));
            close_any(ui);
        }
        if level >= Level::Edit {
            ui.menu_button(tr("Create new"), |ui| {
                if ui.button(tr("List")).clicked() {
                    self.open_new_list(Some(sid), None);
                    close_any(ui);
                }
                if ui.button(tr("Folder")).clicked() {
                    self.actions.push(Action::NewFolder(sid));
                    close_any(ui);
                }
                if ui.button(tr("Doc")).clicked() {
                    self.new_doc(Some(sid));
                    close_any(ui);
                }
            });
        }
        if ui.button(tr("Share & permissions")).clicked() {
            self.open_sharing(sid);
            close_any(ui);
        }
        ui.separator();
        let note = match level {
            Level::Edit => {
                "You can edit in this space. Only people with Full access change the space itself."
            }
            Level::Comment => "You can read and comment on tasks in this space.",
            _ => "You can read this space.",
        };
        ui.label(RichText::new(tr(note)).small().color(t.text3));
    }

    pub(super) fn open_sharing(&mut self, sid: u64) {
        let draft = self
            .data
            .spaces
            .iter()
            .find(|s| s.id == sid)
            .and_then(|s| s.sharing.clone())
            .unwrap_or(SpaceSharing {
                everyone: true,
                people: vec![],
            });
        self.modal = Some(Modal::SpaceSharing { space: sid, draft });
    }

    /// Share & permissions: everyone, or chosen people with a level each.
    /// Returns true when the dialog should close.
    pub(super) fn sharing_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        sid: u64,
        draft: &mut SpaceSharing,
    ) -> bool {
        let Some(space) = self.data.spaces.iter().find(|s| s.id == sid).cloned() else {
            return true;
        };
        let me = self.data.me;
        let may = self.space_level(sid) >= Level::Full;
        ui.label(RichText::new(&space.name).strong().size(15.0));
        ui.add_space(6.0);
        let mut close = false;
        ui.add_enabled_ui(may, |ui| {
            let r = radio(ui, draft.everyone, tr("Everyone in the workspace"));
            name_it(&r, "Share with everyone");
            if r.clicked() {
                draft.everyone = true;
            }
            let r = radio(ui, !draft.everyone, tr("Only the people chosen below"));
            name_it(&r, "Share with chosen people");
            if r.clicked() && draft.everyone {
                draft.everyone = false;
                // whoever closes the space keeps it (owners and admins always do)
                let manages = self.sync_rt.role.is_some_and(|r| r.manages());
                if !manages && !draft.people.iter().any(|p| p.member == me) {
                    draft.people.push(SpaceGrant {
                        member: me,
                        level: Level::Full.as_str().into(),
                    });
                }
            }
            if draft.everyone {
                return;
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(tr("Owners and admins always have full access."))
                    .small()
                    .color(t.text3),
            );
            ui.add_space(4.0);
            let mut drop: Option<u64> = None;
            for p in draft.people.iter_mut() {
                let name = self
                    .data
                    .member(p.member)
                    .map(|m| m.shown().to_string())
                    .unwrap_or_else(|| p.member.to_string());
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [130.0, 20.0],
                        egui::Label::new(RichText::new(&name)).truncate(),
                    );
                    let now = Level::parse(&p.level);
                    for l in Level::ALL {
                        let r = ui
                            .selectable_label(now == l, tr(level_label(l)))
                            .on_hover_text(tr(level_help(l)));
                        name_it(&r, &format!("{} {}", name, level_label(l)));
                        if r.clicked() {
                            p.level = l.as_str().into();
                        }
                    }
                    let r = ui
                        .add(ghost(egui::Button::new("\u{00D7}")))
                        .on_hover_text(tr("Remove"));
                    name_it(&r, &format!("Remove {}", name));
                    if r.clicked() {
                        drop = Some(p.member);
                    }
                });
            }
            if let Some(m) = drop {
                draft.people.retain(|p| p.member != m);
            }
            let others: Vec<(u64, String)> = self
                .data
                .members
                .iter()
                .filter(|m| !draft.people.iter().any(|p| p.member == m.id))
                .map(|m| (m.id, m.shown().to_string()))
                .collect();
            if !others.is_empty() {
                ui.menu_button(tr("Add person"), |ui| {
                    for (id, name) in others {
                        let r = ui.button(&name);
                        name_it(&r, &format!("Add {}", name));
                        if r.clicked() {
                            draft.people.push(SpaceGrant {
                                member: id,
                                level: Level::Edit.as_str().into(),
                            });
                            close_any(ui);
                        }
                    }
                });
            }
        });
        if !may {
            ui.add_space(6.0);
            ui.label(
                RichText::new(tr(
                    "Only people with Full access, owners and admins can change who this space is shared with.",
                ))
                .small()
                .color(t.text3),
            );
        }
        if !self.sharing_enforced() {
            ui.add_space(6.0);
            ui.label(
                RichText::new(tr(
                    "Sharing is enforced on a team server only. Alone or in a shared folder, everyone with the workspace can see and change every space.",
                ))
                .small()
                .color(t.text3),
            );
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if may && primary_button(ui, t, tr("Save")).clicked() {
                let unchanged = space.sharing.is_none() && draft.everyone;
                if !unchanged {
                    if let Some(s) = self.data.spaces.iter_mut().find(|s| s.id == sid) {
                        s.sharing = Some(draft.clone());
                        self.dirty = true;
                    }
                }
                close = true;
            }
            if ui
                .button(tr(if may { "Cancel" } else { "Close" }))
                .clicked()
            {
                close = true;
            }
        });
        close
    }
}
