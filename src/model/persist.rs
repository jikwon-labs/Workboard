// Finding, loading and saving workspaces, and fixing what older versions
// of the app wrote.

use super::*;

impl Data {
    /// Gives every activity entry without an id one: entries from files saved
    /// before activity had ids, and lines just logged with `..Default::default()`.
    /// The id comes from what the entry says (`wb_sync::activity_id`), so every
    /// computer works out the same one. Repeats of an id are dropped.
    pub fn fix_activity_ids(&mut self) {
        for t in &mut self.tasks {
            fix_task_activity_ids(t);
        }
        self.fix_board_ids();
        self.fix_goal_ids();
    }

    /// Gives goal targets and history lines without an id one, the same
    /// one every computer works out (`wb_sync::fill_goal_ids`), passing
    /// over ids deleted before. Nothing to do (the usual case) costs a look.
    pub fn fix_goal_ids(&mut self) {
        let goals = &self.extra.goals;
        if goals
            .iter()
            .all(|g| g.targets.iter().all(|t| t.id != 0) && g.log.iter().all(|e| e.id != 0))
        {
            return;
        }
        let mut v = serde_json::json!({"extra": {
            "goals": serde_json::to_value(goals).unwrap_or_default(),
            "sync": {"tombs": serde_json::to_value(&self.extra.sync.tombs).unwrap_or_default()},
        }});
        if wb_sync::fill_goal_ids(&mut v) {
            if let Ok(g) = serde_json::from_value(v["extra"]["goals"].take()) {
                self.extra.goals = g;
            }
        }
    }

    /// Gives whiteboard strokes and notes without an id one, the same one
    /// every computer works out (`wb_sync::fill_board_ids`).
    pub fn fix_board_ids(&mut self) {
        let boards = &mut self.extra.whiteboards;
        if boards
            .iter()
            .all(|w| w.strokes.iter().all(|s| s.id != 0) && w.notes.iter().all(|n| n.id != 0))
        {
            return;
        }
        let mut v = serde_json::json!({"extra": {"whiteboards": serde_json::to_value(&*boards).unwrap_or_default()}});
        if wb_sync::fill_board_ids(&mut v) {
            if let Ok(b) = serde_json::from_value(v["extra"]["whiteboards"].take()) {
                *boards = b;
            }
        }
    }

    pub fn dir() -> PathBuf {
        let mut p = Self::path();
        p.pop();
        p
    }

    pub fn workspaces() -> Vec<WsEntry> {
        let p = Self::dir().join("workspaces.json");
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// The workspace file open when the app last ran.
    pub fn last_workspace() -> Option<String> {
        let f = std::fs::read_to_string(Self::dir().join("last-workspace"))
            .ok()?
            .trim()
            .to_string();
        // only a plain file name in the data folder
        (!f.is_empty() && !f.contains('/') && !f.contains("..")).then_some(f)
    }

    /// The interface language picked on this computer.
    pub fn language() -> crate::i18n::Choice {
        let s = std::fs::read_to_string(Self::dir().join("language")).unwrap_or_default();
        crate::i18n::choice_from_key(&s)
    }

    pub fn set_language(c: crate::i18n::Choice) {
        let _ = std::fs::create_dir_all(Self::dir());
        let _ = std::fs::write(Self::dir().join("language"), crate::i18n::choice_key(c));
    }

    /// Whether this computer is notified while the app is quit (Settings >
    /// Notifications, src/app/push.rs): on unless turned off here.
    pub fn push_while_quit() -> bool {
        std::fs::read_to_string(Self::dir().join("push-while-quit"))
            .map_or(true, |s| s.trim() != "off")
    }

    pub fn set_push_while_quit(on: bool) {
        let _ = std::fs::create_dir_all(Self::dir());
        let _ = std::fs::write(
            Self::dir().join("push-while-quit"),
            if on { "on" } else { "off" },
        );
    }

    /// Where each workspace was left on this computer (the app keeps what
    /// goes in it): read back on the next start. Never sent to the team.
    pub fn workspace_spots<T: serde::de::DeserializeOwned>() -> std::collections::HashMap<String, T>
    {
        std::fs::read_to_string(Self::dir().join("workspace-spots.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_workspace_spots<T: serde::Serialize>(spots: &std::collections::HashMap<String, T>) {
        let _ = std::fs::create_dir_all(Self::dir());
        if let Ok(s) = serde_json::to_string(spots) {
            let _ = std::fs::write(Self::dir().join("workspace-spots.json"), s);
        }
    }

    pub fn set_last_workspace(file: &str) {
        let _ = std::fs::create_dir_all(Self::dir());
        let _ = std::fs::write(Self::dir().join("last-workspace"), file);
    }

    pub fn save_workspaces(list: &[WsEntry]) {
        let _ = std::fs::create_dir_all(Self::dir());
        if let Ok(s) = serde_json::to_string_pretty(list) {
            let _ = std::fs::write(Self::dir().join("workspaces.json"), s);
        }
    }

    /// Just opened: the sync fingerprints a file left out
    /// (`sync::leave_out_hashes`), from the content as it was saved, so
    /// first, before anything opening it changes.
    fn refill_hashes(&mut self) {
        if !self.extra.sync.rehash {
            return;
        }
        let v = serde_json::to_value(&*self).unwrap_or_default();
        crate::sync::refill_hashes(&mut self.extra.sync, &v);
    }

    pub fn load_file(file: &str) -> Option<Self> {
        let s = std::fs::read_to_string(Self::dir().join(file)).ok()?;
        let mut d: Option<Data> = serde_json::from_str(&s).ok();
        match d.as_mut() {
            Some(d) => {
                d.refill_hashes();
                d.keep_old_bookmarks_personal();
                d.keep_slack_under_me_personal();
            }
            None => Self::keep_unreadable(file, &s),
        }
        d
    }

    /// A file from before bookmarks were shared: the bookmarks in it are
    /// this person's own, so they stay on this computer instead of going to
    /// the whole team. True when that happened.
    pub fn keep_old_bookmarks_personal(&mut self) -> bool {
        if self.extra.bookmarks_shared {
            return false;
        }
        for b in &mut self.extra.bookmarks {
            b.personal = true;
        }
        self.extra.bookmarks_shared = true;
        true
    }

    /// A file from the version that kept "Import under my name" on my
    /// shared member (`Member::slack_mine`): the choices are copied to my
    /// personal `Extra::slack_under_me`. The member keeps them as they are:
    /// it is shared, and taking them off would leave nothing for my other
    /// computers to take over when they update. Only my own member's; a
    /// teammate's are theirs to take over on their computer. Ones I since
    /// pointed elsewhere (`Extra::slack_not_under_me`) aren't. True when
    /// any were copied.
    pub fn keep_slack_under_me_personal(&mut self) -> bool {
        let me = self.me;
        let Some(m) = self.members.iter().find(|m| m.id == me) else {
            return false;
        };
        let mut copied = false;
        for sid in &m.slack_mine {
            if !self.extra.slack_under_me.contains(sid)
                && !self.extra.slack_not_under_me.contains(sid)
            {
                self.extra.slack_under_me.push(sid.clone());
                copied = true;
            }
        }
        copied
    }

    /// The Slack people I bring in under my name: my personal choices, and
    /// any still on my member from an earlier version that I haven't
    /// pointed elsewhere since.
    pub fn slack_under_me(&self) -> Vec<String> {
        let mut out = self.extra.slack_under_me.clone();
        for sid in self
            .member(self.me)
            .map(|m| &m.slack_mine[..])
            .unwrap_or(&[])
        {
            if !out.contains(sid) && !self.extra.slack_not_under_me.contains(sid) {
                out.push(sid.clone());
            }
        }
        out
    }

    /// The member a Slack person goes to by an earlier decision: their
    /// entry in the shared `Extra::slack_map`, else (data from an older
    /// version that came in since the workspace opened) the first member
    /// whose `Member::slack` has them. Once they have an entry, or had one
    /// taken away, the older versions' lists no longer count for them (the
    /// lists stay as they were, for those versions, and may name a member
    /// a later decision moved them off): an entry for a member no longer
    /// here is no decision at all, and matching goes on by email or name.
    pub fn slack_member(&self, sid: &str) -> Option<u64> {
        let decided = self.extra.slack_map.contains_key(sid)
            || self
                .extra
                .sync
                .values
                .contains_key(&crate::sync::map_part(SLACK_MAP, sid));
        let legacy = || {
            self.members
                .iter()
                .find(|m| m.slack.iter().any(|x| x == sid))
                .map(|m| m.id)
        };
        if decided {
            if let Some(m) = self
                .extra
                .slack_map
                .get(sid)
                .copied()
                .filter(|m| self.member(*m).is_some())
            {
                return Some(m);
            }
            // only carried over from the older lists, never decided (no
            // stamp: a teammate's decision coming in has one): with that
            // member gone, another member they name still counts
            let stamped = self
                .extra
                .sync
                .values
                .contains_key(&crate::sync::map_part(SLACK_MAP, sid));
            if !stamped && self.extra.slack_carried.iter().any(|x| x == sid) {
                return legacy();
            }
            return None;
        }
        // an older version's lists; one could name a Slack person on two
        // members (`fold_legacy_slack` leaves those out of the map), and the
        // first member still here wins
        legacy()
    }

    /// After a merge or a join: a Slack match that came in with a stamp is
    /// somebody's decision, so it is no longer only carried here; one that
    /// came in new without a stamp was carried over from older lists on a
    /// teammate's computer (every decision is stamped as it is shared), so
    /// it counts as carried here too. `before`: the Slack ids this copy had
    /// before.
    pub fn note_carried_slack(&mut self, before: &std::collections::BTreeSet<String>) {
        let keys: Vec<String> = self.extra.slack_map.keys().cloned().collect();
        for sid in keys {
            let stamped = self
                .extra
                .sync
                .values
                .contains_key(&crate::sync::map_part(SLACK_MAP, &sid));
            if stamped {
                self.extra.slack_carried.retain(|x| *x != sid);
            } else if !before.contains(&sid) && !self.extra.slack_carried.contains(&sid) {
                self.extra.slack_carried.push(sid);
            }
        }
    }

    /// The Slack matches an older version kept on the members
    /// (`Member::slack`) go into the shared `Extra::slack_map` for Slack ids
    /// it has no entry for (and never had one taken away), noted as nobody's
    /// decision (`sync::carry_map_entry`): opening the workspace in the new
    /// version isn't an edit, and any real decision, here or on a teammate's
    /// computer, wins over them. An older version could leave one Slack id
    /// on two members; those stay out of the map (matching keeps reading the
    /// lists, first member still here) until someone decides. The
    /// members keep their lists, for older versions. True when any came in.
    pub fn fold_legacy_slack(&mut self) -> bool {
        let mut folded = false;
        // a Slack person an older version left on two members stays with
        // those lists (first member still here wins) until someone decides:
        // carrying one of them into the map would block the other if that
        // member were deleted
        let mut holders: std::collections::HashMap<&str, usize> = Default::default();
        for m in &self.members {
            for sid in &m.slack {
                *holders.entry(sid.as_str()).or_default() += 1;
            }
        }
        let ambiguous: std::collections::HashSet<String> = holders
            .into_iter()
            .filter(|(_, n)| *n > 1)
            .map(|(k, _)| k.to_string())
            .collect();
        for m in &self.members {
            for sid in &m.slack {
                if ambiguous.contains(sid) {
                    continue;
                }
                let decided = self
                    .extra
                    .sync
                    .values
                    .contains_key(&crate::sync::map_part(SLACK_MAP, sid));
                if self.extra.slack_map.contains_key(sid) || decided {
                    continue;
                }
                self.extra.slack_map.insert(sid.clone(), m.id);
                if !self.extra.slack_carried.contains(sid) {
                    self.extra.slack_carried.push(sid.clone());
                }
                crate::sync::carry_map_entry(
                    &mut self.extra.sync,
                    SLACK_MAP,
                    sid,
                    &serde_json::json!(m.id),
                );
                folded = true;
            }
        }
        folded
    }

    /// A saved workspace that couldn't be read is copied aside before
    /// anything can save over it; the app tells the person where it went.
    fn keep_unreadable(file: &str, text: &str) {
        let stem = file.strip_suffix(".json").unwrap_or(file);
        let p = Self::dir().join(format!("{}-unreadable-{}.json", stem, now_ts()));
        eprintln!(
            "workboard: couldn't read {}; a copy is kept at {}",
            file,
            p.display()
        );
        if std::fs::write(&p, text).is_ok() {
            *UNREADABLE.lock().unwrap_or_else(|e| e.into_inner()) = Some(p);
        }
    }

    /// The copy of a workspace file that couldn't be read, once.
    pub fn take_unreadable() -> Option<PathBuf> {
        UNREADABLE.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn save_file(&self, file: &str) {
        self.save_to(&Self::dir().join(file));
    }

    /// Where `save_file(file)` writes ("data.json" is the first workspace's
    /// own place, which `WORKBOARD_HOME` can move).
    pub fn file_path(file: &str) -> PathBuf {
        if file == "data.json" {
            Self::path()
        } else {
            Self::dir().join(file)
        }
    }

    /// Write now and wait for it. Writes still queued by `save_later` go
    /// first, so an older copy can't land on top of this one.
    fn save_to(&self, p: &Path) {
        writer::flush();
        writer::write(self, p);
    }

    /// Write on the background writer: the caller doesn't wait for the
    /// serializing or the disk. A newer copy for the same file replaces one
    /// still waiting.
    pub fn save_later(self, p: PathBuf) {
        writer::queue(p, self);
    }

    /// Wait until every write queued by `save_later` is on disk.
    pub fn finish_saving() {
        writer::flush();
    }

    /// A fresh workspace with one space and list, owned by `owner`.
    pub fn blank(name: &str, owner: Member) -> Self {
        let mut d = Data {
            workspace: name.to_string(),
            spaces: vec![],
            tasks: vec![],
            members: vec![owner.clone()],
            me: owner.id,
            next_id: 1000,
            favorites: vec![],
            dark: false,
            docs: vec![],
            channels: vec![],
            extra: Extra::default(),
        };
        let sid = d.new_id();
        let lid = d.new_id();
        // named in the language the screen is in; a new workspace has one
        // person, so the space isn't called a team's
        d.spaces.push(Space {
            id: sid,
            name: crate::i18n::tr("Main space").into(),
            color: [0x5f, 0x55, 0xee],
            statuses: default_statuses(),
            folders: vec![],
            lists: vec![ListItem {
                id: lid,
                name: crate::i18n::tr("List").into(),
                ..Default::default()
            }],
            expanded: true,
            private_owner: None,
            ..Default::default()
        });
        let cid = d.new_id();
        d.channels.push(Channel {
            id: cid,
            name: "general".into(),
            kind: ChannelKind::Channel,
            topic: String::new(),
            members: vec![owner.id],
            messages: vec![],
            last_read: now_ts(),
            ..Default::default()
        });
        d.extra.demo_removed = true;
        // a new workspace leaves out what needs an online service Workboard
        // doesn't have (Customize brings them back); files saved before keep
        // their own choices, and files without these fields keep the old
        // defaults (`Extra::default`)
        d.extra.pinned_tools = vec!["create_task".into()];
        d.extra.hidden_nav = vec!["Skills".into(), "Artifacts".into()];
        d
    }

    pub fn path() -> PathBuf {
        // a separate data folder (for trying things without touching real work)
        if let Ok(dir) = std::env::var("WORKBOARD_HOME") {
            if !dir.is_empty() {
                return PathBuf::from(dir).join("data.json");
            }
        }
        crate::os::app_data_dir().join("data.json")
    }

    pub fn load() -> Self {
        if let Ok(s) = std::fs::read_to_string(Self::path()) {
            if let Ok(mut d) = serde_json::from_str::<Data>(&s) {
                d.refill_hashes();
                d.keep_old_bookmarks_personal();
                d.keep_slack_under_me_personal();
                // move the earlier default colours to the measured ones
                for sp in &mut d.spaces {
                    for st in &mut sp.statuses {
                        if st.name == "in progress" && st.color == [0x5f, 0x55, 0xee] {
                            st.color = IN_PROGRESS_COLOR;
                        }
                        if st.name == "complete" && st.color == [0x00, 0x88, 0x44] {
                            st.color = COMPLETE_COLOR;
                        }
                    }
                }
                if !d.extra.demo_removed {
                    let _ = std::fs::write(
                        Self::dir().join(format!("data-before-cleanup-{}.json", now_ts())),
                        &s,
                    );
                    d.remove_demo();
                    d.save();
                }
                // every workspace has a #general channel to talk in
                if !d.channels.iter().any(|c| c.kind == ChannelKind::Channel) {
                    let id = d.new_id();
                    let members = d.members.iter().map(|m| m.id).collect();
                    d.channels.insert(
                        0,
                        Channel {
                            id,
                            name: "general".into(),
                            kind: ChannelKind::Channel,
                            topic: String::new(),
                            members,
                            messages: vec![],
                            last_read: now_ts(),
                            ..Default::default()
                        },
                    );
                }
                return d;
            }
            if !s.trim().is_empty() {
                Self::keep_unreadable("data.json", &s);
            }
        }
        Self::fresh()
    }

    /// The empty workspace a first start opens with.
    /// The login name with its first letter in capitals ("Me" when empty);
    /// works on any first character, not only ASCII.
    pub fn display_user(user: &str) -> String {
        let mut cs = user.chars();
        match cs.next() {
            None => "Me".to_string(),
            Some(c) => c.to_uppercase().collect::<String>() + cs.as_str(),
        }
    }

    pub fn fresh() -> Self {
        let user = std::env::var("USER").unwrap_or_default();
        let name = Self::display_user(&user);
        let owner = Member {
            id: 1,
            name,
            color: [0x5f, 0x55, 0xee],
            email: String::new(),
            role: "Owner".into(),
            ..Default::default()
        };
        let mut d = Self::blank(crate::i18n::tr("My Workspace"), owner);
        d.extra.welcome = true;
        d
    }

    pub fn save(&self) {
        self.save_to(&Self::path());
    }
}
