// Looking things up in a workspace: members, Lists and their paths,
// statuses, locations, and my running timer.

use super::*;

impl Data {
    pub fn member(&self, id: u64) -> Option<&Member> {
        self.members.iter().find(|m| m.id == id)
    }

    /// Who a text @mentions (see `mentioned_ids`).
    pub fn mentions_in(&self, text: &str) -> Vec<u64> {
        mentioned_ids(text, &self.members, &[])
    }

    pub fn space_of_list(&self, list_id: u64) -> Option<&Space> {
        self.spaces.iter().find(|s| {
            s.lists.iter().any(|l| l.id == list_id)
                || s.folders
                    .iter()
                    .any(|f| f.lists.iter().any(|l| l.id == list_id))
        })
    }

    /// True when the list, or the folder or space it sits in, is archived.
    pub fn list_archived(&self, id: u64) -> bool {
        self.list_item(id).is_some_and(|l| l.archived)
            || self.space_of_list(id).is_some_and(|s| {
                s.archived
                    || s.folders
                        .iter()
                        .any(|f| f.archived && f.lists.iter().any(|l| l.id == id))
            })
    }

    /// True when the Space, Folder or List, or what it sits in, is archived.
    pub fn location_archived(&self, loc: Location) -> bool {
        match loc {
            Location::Space(id) => self.spaces.iter().any(|s| s.id == id && s.archived),
            Location::Folder(id) => self.spaces.iter().any(|s| {
                s.folders
                    .iter()
                    .any(|f| f.id == id && (f.archived || s.archived))
            }),
            Location::List(id) => self.list_archived(id),
            _ => false,
        }
    }

    /// Not archived, and not in an archived list or space.
    pub fn task_live(&self, t: &Task) -> bool {
        !t.archived && !self.list_archived(t.list_id)
    }

    /// Only one person in the workspace: everything unowned is theirs.
    pub fn solo(&self) -> bool {
        self.members.len() <= 1
    }

    /// The task's timer runs and it is mine. A timer from before timers
    /// recorded who started it counts as mine only when I'm alone in the
    /// workspace, so it is never stopped behind a teammate's back.
    pub fn is_my_timer(&self, t: &Task) -> bool {
        self.my_timer_start(t).is_some()
    }

    /// When my timer on the task started, if one runs.
    pub fn my_timer_start(&self, t: &Task) -> Option<i64> {
        let legacy = t.timer_started.filter(|_| match t.timer_by {
            Some(m) => m == self.me,
            None => self.solo(),
        });
        legacy.or_else(|| {
            t.timers
                .iter()
                .find(|r| r.member == self.me)
                .map(|r| r.start)
        })
    }

    /// My time on a task: my time entries plus my running timer (not my
    /// teammates' time, which `Task::tracked_secs` adds in).
    pub fn my_tracked_secs(&self, t: &Task, now: i64) -> i64 {
        let solo = self.solo();
        let done: i64 = t
            .time_entries
            .iter()
            .filter(|e| {
                let owner = t.entry_owner(e);
                owner == self.me || (owner == 0 && solo)
            })
            .map(|e| e.end - e.start)
            .sum();
        done + self.my_timer_start(t).map(|st| now - st).unwrap_or(0)
    }

    /// A timer from an older app that didn't record who started it, in a
    /// workspace with more people: nobody's to stop, so it is offered to
    /// stop by hand.
    pub fn ownerless_timer(&self, t: &Task) -> bool {
        t.timer_started.is_some() && t.timer_by.is_none() && !self.solo()
    }

    /// Starts my timer on a task (other people's timers keep running).
    pub fn start_my_timer(&mut self, task_id: u64, now: i64) {
        let me = self.me;
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == task_id) else {
            return;
        };
        if t.timer_started.is_none() {
            t.timer_started = Some(now);
            t.timer_by = Some(me);
        } else if !t.timers.iter().any(|r| r.member == me) {
            t.timers.push(RunningTimer {
                member: me,
                start: now,
            });
        }
    }

    /// Stops my timer on one task, keeping the time as my entry.
    pub fn stop_my_timer(&mut self, task_id: u64, now: i64) {
        let me = self.me;
        let Some(i) = self.tasks.iter().position(|t| t.id == task_id) else {
            return;
        };
        let legacy_mine = self.tasks[i].timer_started.is_some()
            && match self.tasks[i].timer_by {
                Some(m) => m == me,
                None => self.solo(),
            };
        let t = &mut self.tasks[i];
        if legacy_mine {
            if let Some(st) = t.timer_started.take() {
                t.time_entries.push(TimeEntry {
                    start: st,
                    end: now,
                    member: me,
                });
            }
            t.timer_by = None;
            // the next person's timer moves into the field older apps read
            if !t.timers.is_empty() {
                let r = t.timers.remove(0);
                t.timer_started = Some(r.start);
                t.timer_by = Some(r.member);
            }
        } else if let Some(k) = t.timers.iter().position(|r| r.member == me) {
            let r = t.timers.remove(k);
            t.time_entries.push(TimeEntry {
                start: r.start,
                end: now,
                member: me,
            });
        }
    }

    /// Stops a timer an older app started without an owner, keeping the
    /// time without an owner too.
    pub fn stop_ownerless_timer(&mut self, task_id: u64, now: i64) {
        let Some(t) = self.tasks.iter_mut().find(|t| t.id == task_id) else {
            return;
        };
        if t.timer_by.is_none() {
            if let Some(st) = t.timer_started.take() {
                t.time_entries.push(TimeEntry {
                    start: st,
                    end: now,
                    member: 0,
                });
            }
            if !t.timers.is_empty() {
                let r = t.timers.remove(0);
                t.timer_started = Some(r.start);
                t.timer_by = Some(r.member);
            }
        }
    }

    /// A time entry counts as mine (old entries: see `Task::entry_owner`;
    /// with nobody to give it to, it is mine when I'm alone).
    pub fn is_my_entry(&self, t: &Task, e: &TimeEntry) -> bool {
        match t.entry_owner(e) {
            0 => self.solo(),
            o => o == self.me,
        }
    }

    /// Stops every timer of mine, keeping the time as my entries.
    pub fn stop_my_timers(&mut self, now: i64) {
        let mine: Vec<u64> = self
            .tasks
            .iter()
            .filter(|t| self.is_my_timer(t))
            .map(|t| t.id)
            .collect();
        for id in mine {
            self.stop_my_timer(id, now);
        }
    }

    /// The letter or symbol drawn on a space's square.
    pub fn space_glyph(s: &Space) -> String {
        if s.icon.is_empty() {
            s.name
                .chars()
                .next()
                .unwrap_or('S')
                .to_uppercase()
                .to_string()
        } else {
            s.icon.clone()
        }
    }

    /// The list with this id, wherever it sits (space or folder).
    pub fn list_item(&self, id: u64) -> Option<&ListItem> {
        self.spaces
            .iter()
            .flat_map(|s| {
                s.lists
                    .iter()
                    .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
            })
            .find(|l| l.id == id)
    }

    pub fn list_item_mut(&mut self, id: u64) -> Option<&mut ListItem> {
        self.spaces
            .iter_mut()
            .flat_map(|s| {
                s.lists
                    .iter_mut()
                    .chain(s.folders.iter_mut().flat_map(|f| f.lists.iter_mut()))
            })
            .find(|l| l.id == id)
    }

    pub fn list_name(&self, list_id: u64) -> String {
        for s in &self.spaces {
            for l in &s.lists {
                if l.id == list_id {
                    return l.name.clone();
                }
            }
            for f in &s.folders {
                for l in &f.lists {
                    if l.id == list_id {
                        return l.name.clone();
                    }
                }
            }
        }
        "List".into()
    }

    /// Every List with where it sits: its Space and its Folder (None for a
    /// List right in the Space), each Space's own Lists before its Folders'.
    pub fn list_places(&self) -> impl Iterator<Item = ListPlace<'_>> {
        self.spaces.iter().flat_map(|s| {
            let own = s.lists.iter().map(move |l| ListPlace {
                space: s,
                folder: None,
                list: l,
            });
            let in_folders = s.folders.iter().flat_map(move |f| {
                f.lists.iter().map(move |l| ListPlace {
                    space: s,
                    folder: Some(f),
                    list: l,
                })
            });
            own.chain(in_folders)
        })
    }

    /// Breadcrumb names for a list: space / folder / list.
    pub fn list_path(&self, list_id: u64) -> Vec<String> {
        for s in &self.spaces {
            for l in &s.lists {
                if l.id == list_id {
                    return vec![s.name.clone(), l.name.clone()];
                }
            }
            for f in &s.folders {
                for l in &f.lists {
                    if l.id == list_id {
                        return vec![s.name.clone(), f.name.clone(), l.name.clone()];
                    }
                }
            }
        }
        vec![]
    }

    pub fn statuses_for_list(&self, list_id: u64) -> Vec<Status> {
        self.statuses_ref(list_id).to_vec()
    }

    /// The statuses of a list's space (the default ones outside any), without
    /// copying them: views ask for them for every task on every frame.
    pub fn statuses_ref(&self, list_id: u64) -> &[Status] {
        match self.space_of_list(list_id) {
            Some(s) => &s.statuses,
            None => default_statuses_ref(),
        }
    }

    pub fn status_of(&self, t: &Task) -> Status {
        self.status_ref(t).clone()
    }

    /// The task's status as set up in its space (its first status when the
    /// name isn't there any more), without copying it.
    pub fn status_ref(&self, t: &Task) -> &Status {
        let sts = self.statuses_ref(t.list_id);
        sts.iter()
            .find(|s| s.name == t.status)
            .or(sts.first())
            .unwrap_or(&default_statuses_ref()[0])
    }

    pub fn is_closed(&self, t: &Task) -> bool {
        matches!(
            self.status_ref(t).kind,
            StatusKind::Closed | StatusKind::Done
        )
    }

    /// Lists covered by a location (empty for cross-workspace locations).
    pub fn lists_in(&self, loc: Location) -> Option<Vec<u64>> {
        match loc {
            Location::List(id) => Some(vec![id]),
            Location::Folder(fid) => {
                for s in &self.spaces {
                    for f in &s.folders {
                        if f.id == fid {
                            return Some(f.lists.iter().map(|l| l.id).collect());
                        }
                    }
                }
                Some(vec![])
            }
            Location::Space(sid) => {
                let s = self.spaces.iter().find(|s| s.id == sid)?;
                let mut v: Vec<u64> = s.lists.iter().map(|l| l.id).collect();
                for f in &s.folders {
                    v.extend(f.lists.iter().map(|l| l.id));
                }
                Some(v)
            }
            _ => None,
        }
    }

    pub fn location_name(&self, loc: Location) -> String {
        match loc {
            Location::Home => "Home".into(),
            Location::Inbox => "Inbox".into(),
            Location::Replies => "Replies".into(),
            Location::Assigned => "Assigned Comments".into(),
            Location::TodayOverdue => "Today & Overdue".into(),
            Location::MyTasks => "My Tasks".into(),
            Location::Everything => "Everything".into(),
            Location::Space(id) => self
                .spaces
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            Location::Folder(id) => self
                .spaces
                .iter()
                .flat_map(|s| s.folders.iter())
                .find(|f| f.id == id)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            Location::List(id) => self.list_name(id),
        }
    }

    pub fn location_exists(&self, loc: Location) -> bool {
        match loc {
            Location::Space(id) => self.spaces.iter().any(|s| s.id == id),
            Location::Folder(id) => self
                .spaces
                .iter()
                .any(|s| s.folders.iter().any(|f| f.id == id)),
            Location::List(id) => self.space_of_list(id).is_some(),
            _ => true,
        }
    }

    /// The first list new tasks can go into: skips archived spaces and lists
    /// and other people's private spaces.
    pub fn first_list(&self) -> Option<u64> {
        for s in &self.spaces {
            if s.archived || s.private_owner.is_some_and(|o| o != self.me) {
                continue;
            }
            for f in &s.folders {
                if let Some(l) = f.lists.iter().find(|l| !l.archived) {
                    return Some(l.id);
                }
            }
            if let Some(l) = s.lists.iter().find(|l| !l.archived) {
                return Some(l.id);
            }
        }
        None
    }
}

/// A List with the Space and Folder it sits in (`Data::list_places`).
#[derive(Clone, Copy)]
pub struct ListPlace<'a> {
    pub space: &'a Space,
    pub folder: Option<&'a Folder>,
    pub list: &'a ListItem,
}

impl ListPlace<'_> {
    /// "Space / Folder / List", or "Space / List" outside a Folder.
    pub fn path(&self) -> String {
        match self.folder {
            Some(f) => format!("{} / {} / {}", self.space.name, f.name, self.list.name),
            None => format!("{} / {}", self.space.name, self.list.name),
        }
    }

    /// The List, its Folder or its Space is archived.
    pub fn archived(&self) -> bool {
        self.list.archived || self.folder.is_some_and(|f| f.archived) || self.space.archived
    }

    /// In someone else's Personal space (hidden from me).
    pub fn others_private(&self, me: u64) -> bool {
        self.space.private_owner.is_some_and(|o| o != me)
    }
}
