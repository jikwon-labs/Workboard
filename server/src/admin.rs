// Invites, roles and access, and the admin commands left in
// admin-request.json by `workboard-server`.

use super::*;

impl Hub {
    /// The whole team's copy `member` gets, and what is hidden from them.
    /// (The copy is taken under the lock and written out after.)
    pub(crate) fn latest(&self, member: u64) -> Option<(String, Vec<u64>)> {
        let (canon, rev, manages) = {
            let g = self.state();
            (
                g.canon.clone()?,
                g.saved.rev,
                g.saved.role_of(member).manages(),
            )
        };
        Some(match copy_for(&canon, member, manages) {
            Some((data, hidden)) => (update_json(&data, rev, &hidden), left_out(&canon, &hidden)),
            None => (update_json(&canon, rev, &[]), vec![]),
        })
    }

    /// `answer`: whether it answers that connection's own request.
    pub(crate) fn access_for(&self, member: u64, answer: bool) -> Msg {
        let g = self.state();
        let role = g.saved.role_of(member);
        let people = if role.manages() {
            g.saved.people(g.canon.as_deref(), role)
        } else {
            vec![]
        };
        Msg::Access {
            people,
            role,
            answer: Some(answer),
        }
    }

    /// An access request from `member`.
    pub(crate) fn request(&self, member: u64, from: u64, msg: Msg) -> Reply {
        // backups: the owner only (a restore changes everyone's workspace)
        if matches!(msg, Msg::BackupList | Msg::Restore { .. }) {
            if self.state().saved.role_of(member) != Role::Owner {
                return Reply::Denied("Only the owner can see and restore backups.".into());
            }
            return match msg {
                Msg::Restore { name } => {
                    match self.backup_path(&name).and_then(|p| self.restore(&p)) {
                        Ok(()) => Reply::To(Msg::Restored { name }),
                        Err(e) => Reply::Denied(e),
                    }
                }
                _ => Reply::To(Msg::Backups {
                    backups: self.list_backups(),
                }),
            };
        }
        let mut g = self.state();
        let me = g.saved.role_of(member);
        if !me.manages() {
            return Reply::Denied("Only the owner or an admin can do that.".into());
        }
        let mut kicked: Vec<String> = vec![];
        let mut told = false;
        let done = match msg {
            // nothing changed: only the asker hears
            Msg::AccessList => {
                told = true;
                Ok(None)
            }
            Msg::CreateInvite { member: who, role } => {
                self.create_invite(&mut g, me, who, role).map(Some)
            }
            Msg::SetRole { member: who, role } => {
                told = true;
                self.set_role(&mut g, me, member, from, who, role)
                    .map(|()| None)
            }
            Msg::Revoke { member: who } => {
                told = true;
                self.revoke(&mut g, me, member, from, who).map(|k| {
                    kicked = k;
                    None
                })
            }
            _ => Err("Unknown request.".into()),
        };
        let reply = match done {
            Ok(r) => r,
            Err(why) => return Reply::Denied(why),
        };
        drop(g);
        if !kicked.is_empty() {
            let _ = self.kicks.send(Arc::new(kicked));
        }
        if !told {
            let _ = self.access.send(from);
        }
        match reply {
            Some(m) => Reply::To(m),
            None => Reply::To(self.access_for(member, true)),
        }
    }

    /// CreateInvite from someone whose role is `me`: a code that signs in
    /// as `who` with `role`.
    fn create_invite(&self, g: &mut Inner, me: Role, who: u64, role: Role) -> Result<Msg, String> {
        if role == Role::Owner {
            return Err("There is one owner. Invite them as an admin instead.".into());
        }
        if !member_exists(g.canon.as_deref(), who) {
            return Err(
                "That person isn't in the workspace yet. Add them with Invite first.".into(),
            );
        }
        // a code for the owner signs in as the owner: only the owner makes one
        if g.saved.role_of(who) == Role::Owner && me != Role::Owner {
            return Err("Only the owner can make a code for the owner.".into());
        }
        // a code signs in as that person: admins make them only for
        // members who haven't set up an account yet
        if me != Role::Owner && !g.saved.admin_may_invite(who) {
            return Err("Only the owner can make a code for an admin or for someone who has already signed in. They can sign in with their email and password, or use Forgot password.".into());
        }
        g.saved.drop_expired_invites(now_ms());
        let code = g.saved.make_invite(who, role);
        g.dirty = true;
        Ok(Msg::Invite {
            member: who,
            code,
            emailed: None,
            mail_error: None,
        })
    }

    /// SetRole from `member` (whose role is `me`) on connection `from`.
    fn set_role(
        &self,
        g: &mut Inner,
        me: Role,
        member: u64,
        from: u64,
        who: u64,
        role: Role,
    ) -> Result<(), String> {
        if role == Role::Owner || g.saved.role_of(who) == Role::Owner {
            return Err("The owner's role can't be changed here.".into());
        }
        // admins are the owner's choice: an admin changes only their own
        if me != Role::Owner && g.saved.role_of(who) == Role::Admin && who != member {
            return Err("Only the owner can change another admin's role.".into());
        }
        g.saved.set_role(who, role);
        // a code made earlier doesn't bring back a role taken away
        g.saved
            .invites
            .retain(|_, i| i.member != who || i.role <= role);
        g.dirty = true;
        // their apps hear the new role before the copy it lets them
        // see (an app goes by its role for what it may see)
        let _ = self.access.send(from);
        let relabeled = g
            .canon
            .as_mut()
            .map(Arc::make_mut)
            .is_some_and(|c| label_role(c, who, role));
        if relabeled {
            self.broadcast_canon(g, false);
        }
        Ok(())
    }

    /// Revoke from `member` (whose role is `me`) on connection `from`: the
    /// devices signed out.
    fn revoke(
        &self,
        g: &mut Inner,
        me: Role,
        member: u64,
        from: u64,
        who: u64,
    ) -> Result<Vec<String>, String> {
        if g.saved.role_of(who) == Role::Owner {
            return Err("The owner's access can't be removed.".into());
        }
        // it also deletes their account: only the owner removes an admin
        if me != Role::Owner && g.saved.role_of(who) == Role::Admin && who != member {
            return Err("Only the owner can remove another admin's access.".into());
        }
        let kicked: Vec<String> = g
            .saved
            .devices
            .iter()
            .filter(|(_, d)| d.member == who)
            .map(|(id, _)| id.clone())
            .collect();
        g.saved.devices.retain(|_, d| d.member != who);
        let devices = &g.saved.devices;
        let push: BTreeMap<String, push::PushToken> = g
            .saved
            .push
            .iter()
            .filter(|(d, _)| devices.contains_key(*d))
            .map(|(d, t)| (d.clone(), t.clone()))
            .collect();
        g.saved.push = push;
        g.saved.invites.retain(|_, i| i.member != who);
        // and the password that would let them back in
        g.saved.accounts.remove(&who);
        // removing access also takes back admin rights
        g.saved.set_role(who, Role::Member);
        g.dirty = true;
        let _ = self.access.send(from);
        if g.canon
            .as_mut()
            .map(Arc::make_mut)
            .is_some_and(|c| label_role(c, who, Role::Member))
        {
            self.broadcast_canon(g, false);
        }
        Ok(kicked)
    }
}

impl Hub {
    /// Create a plain member through the same canon and broadcast as app edits.
    pub(crate) fn invite_email(&self, email: &str) -> Result<Value, String> {
        let email = normalize_invite_email(email)?;
        let _persistence = self.persistence.locked();
        let mut g = self.state();
        let canon = g
            .canon
            .as_deref()
            .ok_or("Set up a workspace before inviting teammates.")?;
        if !copy_shape_ok(canon) {
            return Err("Workspace metadata is malformed; no invitation was created.".into());
        }
        let mut meta: wb_sync::SyncMeta = match canon.get("extra").and_then(|e| e.get("sync")) {
            None | Some(Value::Null) => Default::default(),
            Some(value) => serde_json::from_value(value.clone())
                .map_err(|_| "Workspace sync metadata is malformed; no invitation was created.")?,
        };
        let members = canon["members"]
            .as_array()
            .ok_or("Workspace has no members list.")?;
        let matches: Vec<&Value> = members
            .iter()
            .filter(|m| m["email"].as_str().is_some_and(|e| norm_email(e) == email))
            .collect();
        if matches.len() > 1 {
            return Err(
                "Several members have that email. Resolve the duplicate identities first.".into(),
            );
        }
        if g.saved.account_member(&email).is_some() {
            return Err(
                "That email already has a sign-in account. Use sign-in or Forgot password instead."
                    .into(),
            );
        }
        if let Some(member) = matches.first() {
            let id = member["id"]
                .as_u64()
                .filter(|id| *id != 0)
                .ok_or("That member has an invalid id.")?;
            if members
                .iter()
                .filter(|m| m["id"].as_u64() == Some(id))
                .count()
                != 1
            {
                return Err(
                    "Several members share that id. Resolve the duplicate identities first.".into(),
                );
            }
            let role = match member.get("role") {
                None | Some(Value::Null) => "",
                Some(value) => value.as_str().ok_or("That member's role is malformed.")?,
            };
            if !(role.is_empty() || role.eq_ignore_ascii_case("Member"))
                || g.saved.role_of(id) != Role::Member
                || g.saved
                    .invites
                    .values()
                    .any(|i| i.member == id && i.role != Role::Member)
            {
                return Err("Only unregistered plain Members can be invited by email; privileged identities are protected.".into());
            }
            if g.saved.accounts.contains_key(&id)
                || g.saved.devices.values().any(|d| d.member == id)
            {
                return Err(
                    "That member has already signed in. Use sign-in or account recovery instead."
                        .into(),
                );
            }
            let mut saved = g.saved.clone();
            let code = saved.make_invite(id, Role::Member);
            persist_email_invite(&self.home, canon, &saved)?;
            g.saved = saved;
            g.dirty = false;
            drop(g);
            let _ = self.access.send(EVERYONE);
            return Ok(json!({"ok": true, "member": id, "code": code,
                "message": format!("Invite code for member {} as Member: {}  (works once, for {} days)", id, code, INVITE_DAYS)}));
        }
        let id = fresh_member_id(canon, &g.saved, wb_sync::random_u64());
        let mut canon = canon.clone();
        wb_sync::touch_fields(&mut meta, id, &[], now_ms());
        canon["members"]
            .as_array_mut()
            .ok_or("Workspace has no members list.")?
            .push(json!({
                "id": id, "name": email.split('@').next().unwrap_or(&email),
                "email": email, "role": "Member", "invited": true,
                "color": [90, 150, 200], "status_text": ""
            }));
        if let Some(general) = canon["channels"].as_array_mut().and_then(|a| {
            a.iter_mut()
                .find(|c| c["kind"] == "Channel" && c["name"] == "general")
        }) {
            if let Some(members) = general["members"].as_array_mut() {
                members.push(json!(id));
                if let Some(channel) = general["id"].as_u64() {
                    wb_sync::touch_fields(&mut meta, channel, &["members"], now_ms());
                }
            }
        }
        wb_sync::json::set_meta(&mut canon, &meta);
        let mut saved = g.saved.clone();
        let code = saved.make_invite(id, Role::Member);
        // Persist the same revision broadcast_canon will publish, before any
        // identity/code becomes visible to sign-ins or connected apps.
        saved.rev += 1;
        persist_email_invite(&self.home, &canon, &saved)?;
        saved.rev = g.saved.rev;
        g.canon = Some(Arc::new(canon));
        g.saved = saved;
        self.broadcast_canon(&mut g, false);
        g.dirty = false;
        drop(g);
        let _ = self.access.send(EVERYONE);
        Ok(json!({"ok": true, "member": id, "code": code,
            "message": format!("Invite code for member {} as Member: {}  (works once, for {} days)", id, code, INVITE_DAYS)}))
    }

    /// Requests from the `workboard-server` admin commands.
    pub(crate) fn admin(&self, req: &Value) -> Value {
        if let Some(email) = req.get("invite_email") {
            if req.as_object().is_none_or(|o| o.len() != 1) {
                return json!({"ok": false, "message": "invite-email is Member only; role overrides and mixed requests are not allowed."});
            }
            let Some(email) = email.as_str() else {
                return json!({"ok": false, "message": "invite-email requires an email string."});
            };
            return self
                .invite_email(email)
                .unwrap_or_else(|e| json!({"ok": false, "message": e}));
        }
        if let Some(file) = req.get("restore").and_then(|f| f.as_str()) {
            return match self.restore(Path::new(file)) {
                Ok(()) => {
                    json!({"ok": true, "message": format!("Restored {}. Everyone's app gets it now (the copy from before is in {}/before-restore-*.json).", file, BACKUP_DIR)})
                }
                Err(e) => json!({"ok": false, "message": e}),
            };
        }
        if let Some(member) = req.get("invite").and_then(|m| m.as_u64()) {
            let role: Role = req
                .get("role")
                .and_then(|r| serde_json::from_value(r.clone()).ok())
                .unwrap_or_default();
            let mut g = self.state();
            if g.canon.is_some() && !member_exists(g.canon.as_deref(), member) {
                return json!({"ok": false, "message": format!("No member with id {} in the workspace (see `workboard-server people`).", member)});
            }
            let code = g.saved.make_invite(member, role);
            g.dirty = true;
            drop(g);
            let _ = self.access.send(EVERYONE);
            return json!({"ok": true, "message": format!("Invite code for member {} as {}: {}  (works once, for {} days)", member, role.label(), code, INVITE_DAYS), "code": code});
        }
        json!({"ok": false, "message": "unknown request"})
    }

    /// Pick up an admin request left in the server folder.
    pub(crate) fn admin_file(&self) {
        let Some(req) = take_request(&self.home.join(REQUEST_FILE)) else {
            return;
        };
        let res = self.admin(&req);
        self.flush();
        let _ = write_atomic(&self.home.join(RESULT_FILE), &res.to_string());
    }
}

/// Take the request left at `path`, if any. It is moved aside first, so a
/// command that withdraws it (by removing `path`) knows the server never saw it.
pub(crate) fn take_request(path: &Path) -> Option<Value> {
    let mut name = path.file_name()?.to_os_string();
    name.push(".taken");
    let taken = path.with_file_name(name);
    std::fs::rename(path, &taken).ok()?;
    let req = read_json(&taken).ok().flatten();
    let _ = std::fs::remove_file(&taken);
    req
}

/// Lock `RUNNING_FILE` in the server folder `root` for as long as the
/// returned file is kept. None if it can't (another server holds it, or the
/// folder isn't writable); the server runs anyway.
pub(crate) fn hold_running(root: &Path) -> Option<std::fs::File> {
    std::fs::create_dir_all(root).ok()?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(root.join(RUNNING_FILE))
        .ok()?;
    file.try_lock().ok()?;
    Some(file)
}

/// Whether a server is running on `home`: one of the server's own folders,
/// the first workspace's or another's under `workspaces/`.
pub(crate) fn server_running(home: &Path) -> bool {
    let root = match home.parent() {
        Some(up) if up.file_name() == Some(registry::WORKSPACES_DIR.as_ref()) => {
            up.parent().unwrap_or(home)
        }
        _ => home,
    };
    let Ok(file) = std::fs::File::open(root.join(RUNNING_FILE)) else {
        return false;
    };
    matches!(
        file.try_lock_shared(),
        Err(std::fs::TryLockError::WouldBlock)
    )
}

/// How much longer a command waits for a server that is running but hasn't
/// answered yet (busy with backups, or a slow disk).
const BUSY_WAIT: Duration = Duration::from_secs(60);

/// Leave a request for the running server (or the next start) and wait for
/// its answer. None when no server is running (it is done at the next
/// start); an error when one is running but doesn't answer in time.
pub fn admin_request(home: &Path, req: &Value, wait: Duration) -> std::io::Result<Option<Value>> {
    request_files(home, (REQUEST_FILE, RESULT_FILE), req, wait)
}

/// Leave a request for the running server's list of workspaces (undelete)
/// and wait for its answer, like `admin_request`.
pub fn workspaces_request(
    root: &Path,
    req: &Value,
    wait: Duration,
) -> std::io::Result<Option<Value>> {
    request_files(
        root,
        (
            registry::WORKSPACES_REQUEST_FILE,
            registry::WORKSPACES_RESULT_FILE,
        ),
        req,
        wait,
    )
}

pub(crate) fn request_files(
    home: &Path,
    files: (&str, &str),
    req: &Value,
    wait: Duration,
) -> std::io::Result<Option<Value>> {
    request_files_within(home, files, req, wait, BUSY_WAIT)
}

/// `request_files`, waiting `busy` more when the server is running but slow.
/// Then the request is withdrawn if the server hasn't taken it yet, and either
/// way it is an error: the server is running, so "do it when it starts"
/// doesn't apply.
pub(crate) fn request_files_within(
    home: &Path,
    (request, result): (&str, &str),
    req: &Value,
    wait: Duration,
    busy: Duration,
) -> std::io::Result<Option<Value>> {
    std::fs::create_dir_all(home)?;
    let _ = std::fs::remove_file(home.join(result));
    write_atomic(&home.join(request), &req.to_string())?;
    let mut until = std::time::Instant::now() + wait;
    let mut waited_more = false;
    loop {
        if let Ok(Some(v)) = read_json(&home.join(result)) {
            let _ = std::fs::remove_file(home.join(result));
            return Ok(Some(v));
        }
        if std::time::Instant::now() >= until {
            if !server_running(home) {
                // left for the next start
                return Ok(None);
            }
            if !waited_more {
                waited_more = true;
                until = std::time::Instant::now() + busy;
                continue;
            }
            let secs = (wait + busy).as_secs();
            return Err(std::io::Error::other(
                if std::fs::remove_file(home.join(request)).is_ok() {
                    format!("The server is running but didn't take the request in {secs} seconds, so nothing was done. Try again.")
                } else {
                    format!(
                        "The server is running but didn't answer in {secs} seconds. It is still on it; the answer will be in {}.",
                        home.join(result).display()
                    )
                },
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
