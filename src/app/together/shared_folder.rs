// Working in a shared folder: writing and pulling the shared copy, merging
// it in, joining or leaving a folder, and the baseline of what was synced.

use super::*;

/// A workspace as the app wrote it before fields left at their default were
/// left out of the file (time entries' `member`, forms', whiteboards' and
/// dashboards' `owner`, dashboards' `kind`), for `sync::carry_hashes`.
pub(crate) fn legacy_shape(v: &serde_json::Value) -> serde_json::Value {
    let mut v = v.clone();
    let fill = |items: Option<&mut serde_json::Value>, keys: &[(&str, serde_json::Value)]| {
        for it in items.and_then(|a| a.as_array_mut()).into_iter().flatten() {
            if let Some(o) = it.as_object_mut() {
                for (k, d) in keys {
                    o.entry(k.to_string()).or_insert_with(|| d.clone());
                }
            }
        }
    };
    for t in v
        .get_mut("tasks")
        .and_then(|a| a.as_array_mut())
        .into_iter()
        .flatten()
    {
        fill(t.get_mut("time_entries"), &[("member", 0.into())]);
    }
    if let Some(e) = v.get_mut("extra") {
        fill(e.get_mut("forms"), &[("owner", 0.into())]);
        fill(e.get_mut("whiteboards"), &[("owner", 0.into())]);
        fill(
            e.get_mut("dashboards"),
            &[("owner", 0.into()), ("kind", "".into())],
        );
    }
    v
}

/// After opening a workspace: a copy saved before field fingerprints were
/// kept gets them for every item unchanged since its last save, so changes
/// made before the next sync are noted field by field (as the iPhone does
/// when it opens an older copy). Without them a whole item counts as
/// changed, and a field emptied offline (the last attachment removed) comes
/// back from the team's copy.
pub(crate) fn fill_sync_baseline(data: &mut Data) {
    if data.extra.shared.is_none() || data.extra.sync.hashes.is_empty() {
        return;
    }
    let v = serde_json::to_value(&*data).unwrap_or_default();
    let meta = &mut data.extra.sync;
    if meta.v == 1 {
        // fingerprints from before fields at their default were left out
        // of the file: the new shape alone isn't an edit
        sync::carry_hashes(meta, &legacy_shape(&v), &v);
    }
    sync::fill_field_hashes(meta, &v);
    // a shared setting or Slack match never fingerprinted (every save since
    // shared maps came in fingerprints them all) is as it is now, so the
    // next change doesn't take it as set then, as the iPhone does
    sync::fill_value_hashes(meta, &v);
}

impl App {
    fn data_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.data).unwrap_or_default()
    }

    /// After a change: note what changed so teammates' copies merge correctly.
    pub(crate) fn sync_note_change(&mut self) {
        if self.data.extra.shared.is_none() {
            return;
        }
        self.stamp_now();
        self.sync_rt.pending = true;
    }

    /// Record what changed since the last save in the change bookkeeping.
    /// True when anything did.
    fn stamp_now(&mut self) -> bool {
        let v = self.data_value();
        let mut meta = std::mem::take(&mut self.data.extra.sync);
        let before = meta.clone();
        if meta.v == 1 {
            // fingerprints from before fields left at their default were
            // left out of the file: the new shape alone isn't an edit
            sync::carry_hashes(&mut meta, &legacy_shape(&v), &v);
        }
        sync::stamp(&v, &mut meta, chrono::Utc::now().timestamp_millis());
        let changed = meta != before;
        self.data.extra.sync = meta;
        changed
    }

    pub(crate) fn write_shared(&mut self) {
        let Some(cfg) = self.data.extra.shared.clone() else {
            return;
        };
        self.data.fix_activity_ids();
        let v = self.data_value();
        if cfg.server.is_some() {
            // sent when connected; after a reconnect the whole copy goes again anyway
            if let Some(net) = &self.sync_rt.net {
                net.push(sync::shareable(&v));
            }
            self.sync_rt.pending = false;
            return;
        }
        if sync::write_mine(Path::new(&cfg.folder), &cfg.device, &v, APP_VERSION).is_err() {
            self.toast(tr(
                "Couldn't write to the shared folder. Is it still there?",
            ));
        }
        self.sync_rt.pending = false;
    }

    pub(super) fn save_local(&self) {
        self.save_now();
    }

    /// Read teammates' copies and merge them in. Returns true when something new arrived.
    pub(crate) fn sync_pull(&mut self) -> bool {
        let Some(cfg) = self.data.extra.shared.clone() else {
            return false;
        };
        if cfg.server.is_some() {
            return self.net_tick(None);
        }
        let files = sync::other_files(Path::new(&cfg.folder), &cfg.device);
        self.sync_rt.seen = files.clone();
        let others: Vec<serde_json::Value> = files
            .iter()
            .filter_map(|(p, _)| sync::read_json(p))
            .collect();
        if !self.sync_rt.told_old_app {
            if let Some(v) = others.iter().find_map(|o| sync::older_app(o, APP_VERSION)) {
                self.sync_rt.told_old_app = true;
                self.toast(if v.is_empty() {
                        tr("Someone in this shared folder uses an older version of Workboard. Ask them to update, or their changes can undo yours.").to_string()
                    } else {
                        trf!("Someone in this shared folder uses Workboard {}, an older version. Ask them to update, or their changes can undo yours.", v)
                    });
            }
        }
        self.merge_in(&others, &[])
    }

    /// Merge the team's copies into ours. On a server, `hidden` (with what
    /// our own copy shows we may not see) is taken out of our copy without
    /// deleting it. Returns true when something new arrived.
    pub(crate) fn merge_in(&mut self, others: &[serde_json::Value], hidden: &[u64]) -> bool {
        if others.is_empty() {
            return false;
        }
        // changes not recorded yet (made just before quitting, say) are
        // recorded first, or the merge would take them as already known
        if self.data.extra.shared.is_some()
            && !self.data.extra.sync.hashes.is_empty()
            && self.stamp_now()
        {
            self.sync_rt.pending = true;
        }
        let me = self.data.me;
        let seen: Vec<u64> = self
            .data
            .channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Channel && (!c.private || c.members.contains(&me)))
            .map(|c| c.id)
            .collect();
        let slack_before: std::collections::BTreeSet<String> =
            self.data.extra.slack_map.keys().cloned().collect();
        let mut v = self.data_value();
        let mut changed = sync::merge(&mut v, others);
        let on_server = self.on_server();
        let gone = if on_server {
            // the server says what our role keeps from us (spaces shared
            // with chosen people); our own copy adds only what no role
            // opens (others' DMs, Personal spaces and private docs). Our
            // role here may be a step behind the server's: just made an
            // admin, a space that just came would go again.
            let mut ids = hidden.to_vec();
            ids.extend(sync::hidden_from(&v, me, true));
            sync::hide(&mut v, &ids)
        } else {
            vec![]
        };
        changed |= !gone.is_empty();
        match serde_json::from_value::<Data>(v) {
            Ok(d) => {
                self.data = d;
                self.data_changed();
                self.data.note_carried_slack(&slack_before);
                // fingerprints of what the app writes, which can differ a
                // little from what came in (fields an older app left out)
                let v = self.data_value();
                self.data.extra.sync.hashes = sync::fingerprints(&v);
                self.data.extra.sync.field_hashes = sync::part_hashes(&v);
            }
            Err(e) => {
                // most likely saved by a newer version of the app
                if !self.sync_rt.told_unreadable {
                    eprintln!("workboard: couldn't take in the team's changes: {}", e);
                    self.toast(tr("Your team's changes couldn't be read. Someone may be using a newer version of the app, so update Workboard."));
                    self.sync_rt.told_unreadable = true;
                }
                return false;
            }
        }
        for ch in gone.iter().filter(|c| {
            c.get("id")
                .and_then(|i| i.as_u64())
                .is_some_and(|i| seen.contains(&i))
        }) {
            self.left_channel(ch);
        }
        if changed {
            // what's new for me is announced on the next frame (notify.rs)
            self.notify_rt.pending = true;
            // teammates' changes come often: saved like my own edits, once
            // they settle and off the screen's thread
            self.save_soon();
            self.sync_rt.pending = true;
        }
        changed
    }

    /// A channel I could see was taken out of my copy because I'm no longer
    /// in it: say so, and keep what I wrote there in the last few minutes in
    /// my Notepad, since it may not have reached the team before I left.
    fn left_channel(&mut self, ch: &serde_json::Value) {
        let me = self.data.me;
        let name = ch
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or_default()
            .to_string();
        let since = now_ts() - 10 * 60;
        let mut mine: Vec<String> = vec![];
        for m in ch
            .get("messages")
            .and_then(|m| m.as_array())
            .into_iter()
            .flatten()
        {
            for x in std::iter::once(m).chain(
                m.get("replies")
                    .and_then(|r| r.as_array())
                    .into_iter()
                    .flatten(),
            ) {
                let author = x.get("author").and_then(|a| a.as_u64());
                let ts = x.get("ts").and_then(|t| t.as_i64()).unwrap_or(0);
                let text = x.get("text").and_then(|t| t.as_str()).unwrap_or_default();
                if author == Some(me) && ts >= since && !text.trim().is_empty() {
                    mine.push(text.to_string());
                }
            }
        }
        if mine.is_empty() {
            self.toast(trf!(
                "You're no longer in #{}, so it was taken off your list.",
                name
            ));
            return;
        }
        let notepad = &mut self.data.extra.notepad;
        if !notepad.is_empty() && !notepad.ends_with('\n') {
            notepad.push('\n');
        }
        notepad.push_str(&format!(
            "{}\n{}\n",
            trf!("Written in #{} before you were taken out of it:", name),
            mine.join("\n")
        ));
        self.toast(trf!("You're no longer in #{}. What you wrote there in the last few minutes is in your Notepad.", name));
    }

    /// Once per frame: write our copy when it changed, look for teammates' changes.
    pub(crate) fn sync_tick(&mut self, ctx: &egui::Context) {
        if self.sync_rt.listing.is_some() || self.sync_rt.open_local.is_some() {
            self.listing_tick();
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if self.sync_rt.joining.is_some() {
            self.net_tick(Some(ctx));
            return;
        }
        let Some(cfg) = self.data.extra.shared.clone() else {
            return;
        };
        let now = ctx.input(|i| i.time);
        if cfg.server.is_some() {
            self.net_tick(Some(ctx));
            if self.sync_rt.pending {
                if now - self.sync_rt.last_write >= WRITE_SECS {
                    self.sync_rt.last_write = now;
                    self.write_shared();
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(WRITE_SECS));
                }
            }
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(POLL_SECS));
        if self.sync_rt.pending && now - self.sync_rt.last_write >= WRITE_SECS {
            self.sync_rt.last_write = now;
            self.write_shared();
        }
        if now - self.sync_rt.last_poll >= POLL_SECS {
            self.sync_rt.last_poll = now;
            let cfg = self.data.extra.shared.clone().unwrap();
            let files = sync::other_files(Path::new(&cfg.folder), &cfg.device);
            if files != self.sync_rt.seen {
                self.sync_pull();
            }
        }
    }

    fn new_cfg(folder: &str) -> SharedCfg {
        // pick an id tag nobody in the folder uses yet
        let used: Vec<u16> = sync::other_files(Path::new(folder), "")
            .iter()
            .filter_map(|(p, _)| sync::read_json(p))
            .filter_map(|v| {
                v.get("extra")?
                    .get("shared")?
                    .get("tag")?
                    .as_u64()
                    .map(|t| t as u16)
            })
            .collect();
        let mut tag = (sync::random_u64() % 1024) as u16;
        while used.contains(&tag) {
            tag = (tag + 1) % 1024;
        }
        SharedCfg {
            folder: folder.to_string(),
            device: format!("{:08x}", sync::random_u64() as u32),
            tag,
            server: None,
        }
    }

    /// Share the open workspace in an (empty) folder.
    pub(crate) fn share_here(&mut self, folder: &str) -> Result<(), String> {
        let folder = folder.trim();
        if folder.is_empty() {
            return Err(tr("Choose a folder first.").into());
        }
        std::fs::create_dir_all(folder).map_err(|e| trf!("Couldn't use that folder: {}", e))?;
        self.data.extra.shared = Some(Self::new_cfg(folder));
        self.data.extra.sync = Default::default();
        self.sync_note_change();
        self.write_shared();
        self.save_local();
        self.sync_rt = SyncRt::default();
        self.share_cached_files();
        Ok(())
    }

    /// Join the workspace someone shared in a folder. It opens as a new
    /// workspace, so the one you had stays in the workspace menu.
    pub(crate) fn join_shared(&mut self, folder: &str) -> Result<(), String> {
        let folder = folder.trim();
        let files = sync::other_files(Path::new(folder), "");
        let newest = files
            .iter()
            .max_by_key(|(_, m)| *m)
            .ok_or(tr("No shared workspace in that folder yet."))?;
        let base =
            sync::read_json(&newest.0).ok_or(tr("The shared workspace couldn't be read."))?;
        self.join_from(base, Self::new_cfg(folder))?;
        self.sync_rt = SyncRt::default();
        self.sync_pull();
        self.after_join();
        Ok(())
    }

    /// Open the team's copy as a new workspace on this computer.
    pub(super) fn join_from(
        &mut self,
        base: serde_json::Value,
        cfg: SharedCfg,
    ) -> Result<(), String> {
        serde_json::from_value::<Data>(base.clone())
            .map_err(|_| tr("The shared workspace couldn't be read.").to_string())?;
        // the team's things with this person's own settings (theme, notifications,
        // pinned tools...) instead of the owner's
        let mut d: Data = serde_json::from_value(sync::with_team_things(&self.data_value(), &base))
            .map_err(|_| tr("The shared workspace couldn't be read.").to_string())?;
        // personal things that point into the old workspace start empty
        d.favorites.clear();
        d.extra.recent.clear();
        d.extra.notepad.clear();
        d.extra.reminders.clear();
        d.extra.trash.clear();
        d.extra.inbox_cleared.clear();
        d.extra.inbox_later.clear();
        d.extra.starred.clear();
        d.extra.closed_dms.clear();
        // Slack matches this computer carried belong to the workspace left;
        // the team's own carried ones (no stamp) count as carried here
        d.extra.slack_carried.clear();
        d.note_carried_slack(&Default::default());
        d.extra.shared = Some(cfg);
        d.extra.welcome = false;
        d.extra.sync.hashes.clear();
        d.extra.sync.field_hashes.clear();
        d.me = 0;
        // a status editor still open: its edits are done (and saved with the
        // workspace being left) like on closing it, and it doesn't stay open
        // on the joined workspace, where it would put its names on a space
        // of the same id
        self.finish_status_edit();
        // the workspace left was chosen past the sign-in screen
        self.data.extra.welcome = false;
        if self.persist {
            self.save_now();
            let mut list = Data::workspaces();
            if list.is_empty() {
                list.push(WsEntry {
                    name: self.data.workspace.clone(),
                    file: self.ws_file.clone(),
                });
            }
            // the same team workspace joined again (signed out, or signed
            // in once more): its copy here is replaced, not added to
            let again = d
                .extra
                .shared
                .as_ref()
                .and_then(|c| c.server.as_deref())
                .and_then(|a| local_workspace_on(a, ""))
                .filter(|f| list.iter().any(|e| &e.file == f));
            let file = match again {
                Some(f) => {
                    if let Some(e) = list.iter_mut().find(|e| e.file == f) {
                        e.name = d.workspace.clone();
                    }
                    f
                }
                None => {
                    let file = format!("workspace-{}.json", now_ts());
                    list.push(WsEntry {
                        name: d.workspace.clone(),
                        file: file.clone(),
                    });
                    file
                }
            };
            Data::save_workspaces(&list);
            Data::set_last_workspace(&file);
            self.ws_places.clear();
            self.ws_file = file;
        }
        self.data = d;
        self.data_changed();
        // a huddle in the workspace left ends there (its conversation isn't here)
        self.leave_huddle();
        Ok(())
    }

    pub(super) fn after_join(&mut self) {
        let v = self.data_value();
        self.data.extra.sync.hashes = sync::fingerprints(&v);
        self.data.extra.sync.field_hashes = sync::part_hashes(&v);
        // the team's settings and Slack matches as they came (the copy
        // teammates send leaves these out): a change to one, even the
        // first thing done here, is then noted as one (as the iPhone does)
        self.data.extra.sync.value_hashes = sync::value_hashes(&v);
        self.loc = self
            .data
            .first_list()
            .map(Location::List)
            .unwrap_or(Location::Home);
        self.section = Section::Home;
        self.open_task = None;
        self.chat.open = None;
        self.recent.clear();
        self.clear_history();
        // a workspace I just joined: what is in it already isn't news
        self.notify_rt.seeded = false;
        self.save_local();
    }

    pub(crate) fn stop_sharing(&mut self) {
        self.leave_huddle();
        self.ws_places.clear();
        if let Some(SharedCfg {
            server: Some(addr),
            device,
            ..
        }) = &self.data.extra.shared
        {
            forget_token(addr, device);
        }
        self.data.extra.shared = None;
        self.data.extra.sync = Default::default();
        self.sync_rt = SyncRt::default();
        self.dirty = true;
    }

    /// Who you are in a joined workspace: an existing (invited) member, or someone new.
    pub(crate) fn become_member(&mut self, id: Option<u64>, name: &str, email: &str) {
        let id = match id {
            Some(id) => id,
            None => {
                let id = self.data.new_id();
                let color = STATUS_PALETTE[1 + (id % 13) as usize];
                self.data.members.push(Member {
                    id,
                    name: name.trim().to_string(),
                    color,
                    email: email.trim().to_string(),
                    role: "Member".into(),
                    ..Default::default()
                });
                id
            }
        };
        if let Some(m) = self.data.members.iter_mut().find(|m| m.id == id) {
            m.invited = false;
        }
        self.data.me = id;
        for c in &mut self.data.channels {
            if c.kind == ChannelKind::Channel && !c.private && !c.members.contains(&id) {
                c.members.push(id);
            }
        }
        self.dirty = true;
    }
}
