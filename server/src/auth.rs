// Signing in: invite codes, passwords, tokens, password resets and
// emailed invitations.

use super::*;

impl Hub {
    /// Check a hello. Ok: (member, role, tag, new token).
    ///
    /// Hashing a password takes a moment (PBKDF2), so it happens outside the
    /// hub's lock: what's needed is read under the lock, hashed without it,
    /// and the result committed under the lock again, after checking that
    /// nothing it relied on changed meanwhile.
    pub(crate) fn sign_in(&self, h: &Hello) -> Result<(u64, Role, u16, Option<String>), String> {
        let device = h.device.as_str();
        // a password-reset code from the email POST /auth/forgot sent
        if let Some(code) = given(&h.reset_code) {
            let email = given(&h.email).ok_or("Type the email you sign in with.")?;
            let pw = h.new_password.as_deref().unwrap_or("");
            // wrong codes are counted apart from wrong passwords: someone
            // locking an email with wrong passwords doesn't stop its owner
            // from resetting it
            let tries = reset_tries(email);
            let _slot = HashSlot::take(&self.hashing)?;
            {
                let mut g = self.inner.lock().unwrap();
                if let Some(why) = g.locked(&tries) {
                    return Err(why);
                }
                // a new password that won't do isn't a wrong try
                if let Some(why) = password_problem(pw) {
                    return Err(why);
                }
                // a wrong try until the code proves right (as signing in
                // with a password), so many at once can't get past MAX_TRIES
                g.failed(&tries);
            }
            let hashed = Account::new("", pw);
            let mut g = self.inner.lock().unwrap();
            let key = norm_email(email);
            let fresh = g.resets.get(&key).is_some_and(|(c, t)| {
                same(&normalize_code(c), &normalize_code(code))
                    && now_ms() - t < RESET_MINUTES * 60_000
            });
            let member = g.saved.account_member(email).filter(|_| fresh);
            let Some(member) = member else {
                return Err(format!("That reset code doesn't work. It may be used already or expired (codes last {} minutes). Ask for a new one.", RESET_MINUTES));
            };
            g.resets.remove(&key);
            g.fails.remove(&key);
            g.fails.remove(&tries);
            let kept = g.saved.accounts[&member].email.clone();
            g.saved.accounts.insert(member, hashed.with_email(&kept));
            // a new password: codes made for them earlier no longer sign in
            g.saved.invites.retain(|_, i| i.member != member);
            // and whoever knew the old one (or took a device) is signed out
            let gone = g.saved.sign_out_others(member, device);
            if !gone.is_empty() {
                let _ = self.kicks.send(Arc::new(gone));
            }
            return Ok(self.signed_in(g, device, member, h.tag));
        }
        if let Some(code) = given(&h.invite) {
            // a made-up code costs no hashing (checked again below)
            if self.inner.lock().unwrap().saved.find_invite(code).is_none() {
                return Err("That invite code doesn't work. It may be used already or expired (codes last 7 days). Ask for a new one.".into());
            }
            let _slot = match h.new_password {
                Some(_) => Some(HashSlot::take(&self.hashing)?),
                None => None,
            };
            // a password to choose, hashed before the lock (checked below)
            let hashed = h
                .new_password
                .as_deref()
                .filter(|pw| password_problem(pw).is_none())
                .map(|pw| Account::new("", pw));
            let mut g = self.inner.lock().unwrap();
            let Some(key) = g.saved.find_invite(code) else {
                return Err("That invite code doesn't work. It may be used already or expired (codes last 7 days). Ask for a new one.".into());
            };
            let member = g.saved.invites[&key].member;
            // choosing a password: check it all before the code is used up.
            // Someone who already has a password keeps it: a code signs this
            // computer in, it doesn't take over their account (they use their
            // password, or Forgot password, to change it).
            let account = match h.new_password.as_deref() {
                Some(_) if g.saved.accounts.contains_key(&member) => None,
                Some(pw) => {
                    let email = given(&h.email)
                        .map(|e| e.trim().to_string())
                        .unwrap_or_else(|| member_field(g.canon.as_ref(), member, "email"));
                    if email.is_empty() {
                        return Err("Type your email too: you sign in with it on your other computers and phones.".into());
                    }
                    if let Some(why) = g.saved.account_problem(member, &email, pw) {
                        return Err(why);
                    }
                    match hashed {
                        Some(a) => Some(a.with_email(&email)),
                        None => {
                            return Err(password_problem(pw)
                                .unwrap_or_else(|| "Choose another password.".into()))
                        }
                    }
                }
                None => None,
            };
            let inv = g.saved.invites.remove(&key).expect("found above");
            if let Some(a) = account {
                g.saved.accounts.insert(member, a);
            }
            if inv.role > g.saved.role_of(member) {
                g.saved.set_role(member, inv.role);
            }
            return Ok(self.signed_in(g, device, member, h.tag));
        }
        if let (Some(email), Some(pw)) = (given(&h.email), h.password.as_deref()) {
            let (g, member) = self.check_password(email, pw)?;
            return Ok(self.signed_in(g, device, member, h.tag));
        }
        if let Some(token) = &h.token {
            let g = self.inner.lock().unwrap();
            return match g.saved.device_with(device, token) {
                Some(d) => Ok((d.member, g.saved.role_of(d.member), d.tag, None)),
                _ => Err("This computer isn't signed in to this server anymore. Sign in with your email and password, or ask the owner or an admin for a new invite code.".into()),
            };
        }
        if let Some(member) = h.claim {
            let _slot = match h.new_password {
                Some(_) => Some(HashSlot::take(&self.hashing)?),
                None => None,
            };
            let hashed = h
                .new_password
                .as_deref()
                .filter(|pw| password_problem(pw).is_none())
                .map(|pw| Account::new("", pw));
            let mut g = self.inner.lock().unwrap();
            // a server that once had a workspace (its revision moved past 0)
            // but lost workspace.json is never up for grabs: its owner brings
            // the workspace back with `workboard-server restore`.
            if g.canon.is_none() && g.saved.rev > 0 {
                return Err("This server lost its workspace file. Ask its owner to restore a backup on the server (workboard-server restore).".into());
            }
            // only the very first claim wins: two people setting up the same
            // empty server at once must not end up with their workspaces merged.
            // A claim that never brought its workspace (the app quit right
            // after) can be made again from the same computer, and gives way
            // to anyone after claim_wait.
            if g.canon.is_none() && !g.saved.devices.is_empty() {
                let same_device =
                    g.saved.devices.len() == 1 && g.saved.devices.contains_key(device);
                let stale = g
                    .saved
                    .devices
                    .values()
                    .all(|d| now_ms() - d.created >= self.claim_wait.as_millis() as i64);
                if !same_device && !stale {
                    return Err("Someone else is setting up this server right now. Ask them for an invite code.".into());
                }
                let gone: Vec<String> = g
                    .saved
                    .devices
                    .keys()
                    .filter(|d| d.as_str() != device)
                    .cloned()
                    .collect();
                g.saved.devices.clear();
                g.saved.roles.clear();
                g.saved.invites.clear();
                g.saved.accounts.clear();
                if !gone.is_empty() {
                    let _ = self.kicks.send(Arc::new(gone));
                }
            }
            if g.canon.is_none() && g.saved.devices.is_empty() {
                // a workspace made with POST /workspaces is for the person
                // who made it: they sign in to it with the account they
                // already have on this server (registry.rs)
                if let Some(seed) = g.saved.seed.clone() {
                    let email = given(&h.email).unwrap_or("");
                    if !same(&norm_email(email), &norm_email(&seed.email)) {
                        return Err("This workspace is being set up by someone else.".into());
                    }
                    g.saved.seed = None;
                    if h.new_password.is_none() {
                        g.saved.accounts.insert(member, seed);
                    }
                }
                if let Some(pw) = h.new_password.as_deref() {
                    let email = given(&h.email).unwrap_or("").trim().to_string();
                    if let Some(why) = g.saved.account_problem(member, &email, pw) {
                        return Err(why);
                    }
                    let Some(a) = hashed else {
                        return Err(password_problem(pw)
                            .unwrap_or_else(|| "Choose another password.".into()));
                    };
                    g.saved.accounts.insert(member, a.with_email(&email));
                }
                g.saved.set_role(member, Role::Owner);
                return Ok(self.signed_in(g, device, member, h.tag));
            }
        }
        Err("This server already has a workspace. Sign in with your email and password, or ask its owner or an admin for an invite code.".into())
    }

    /// The member whose account `email` and `pw` are, counting a wrong try
    /// (and refusing while the email is locked). The lock is held on return
    /// so the account can't change before it is used.
    pub(crate) fn check_password(
        &self,
        email: &str,
        pw: &str,
    ) -> Result<(std::sync::MutexGuard<'_, Inner>, u64), String> {
        let _slot = HashSlot::take(&self.hashing)?;
        // the try counts as a wrong one until the password proves right,
        // so many tries at once can't get past MAX_TRIES while they hash
        let stored = {
            let mut g = self.inner.lock().unwrap();
            if let Some(why) = g.locked(email) {
                return Err(why);
            }
            g.failed(email);
            g.saved
                .account_member(email)
                .map(|m| (m, g.saved.accounts[&m].clone()))
        };
        let ok = match &stored {
            Some((_, a)) => a.check(pw),
            None => {
                // as slow as a real check, so the answer doesn't tell who has an account
                let _ = Account::new("x@x.x", pw);
                false
            }
        };
        let mut g = self.inner.lock().unwrap();
        // the account may have changed or gone meanwhile
        let member = stored
            .filter(|(m, a)| ok && g.saved.accounts.get(m) == Some(a))
            .map(|(m, _)| m);
        let Some(member) = member else {
            return Err("That email and password don't match an account on this server.".into());
        };
        g.fails.remove(&norm_email(email));
        Ok((g, member))
    }

    /// Give `device` a new token for `member`.
    pub(crate) fn signed_in(
        &self,
        mut g: std::sync::MutexGuard<'_, Inner>,
        device: &str,
        member: u64,
        tag: Option<u16>,
    ) -> (u64, Role, u16, Option<String>) {
        let (tag, token) = g.saved.add_device(device, member, tag);
        g.dirty = true;
        let role = g.saved.role_of(member);
        drop(g);
        let _ = self.access.send(EVERYONE);
        (member, role, tag, Some(token))
    }

    /// `device` is still signed in here as `member`.
    pub(crate) fn device_is(&self, device: &str, member: u64) -> bool {
        let g = self.inner.lock().unwrap();
        g.saved
            .devices
            .get(device)
            .is_some_and(|d| d.member == member)
    }

    /// Set up `member`'s account or change its password. The hashing happens
    /// outside the hub's lock (see sign_in).
    pub(crate) fn set_password(
        &self,
        member: u64,
        device: &str,
        email: &str,
        password: &str,
        old: Option<&str>,
    ) -> Result<Msg, String> {
        let existing = {
            let mut g = self.inner.lock().unwrap();
            // a device whose access was taken back (Revoke deletes the
            // password) can't set up a new one
            if !g
                .saved
                .devices
                .get(device)
                .is_some_and(|d| d.member == member)
            {
                return Err("Your access to this server was removed.".into());
            }
            let existing = g.saved.accounts.get(&member).cloned();
            if let Some(a) = &existing {
                if let Some(why) = g.locked(&a.email) {
                    return Err(why);
                }
            }
            // a new email or password that won't do isn't a wrong try
            if let Some(why) = g.saved.account_problem(member, email, password) {
                return Err(why);
            }
            if let Some(a) = &existing {
                // a wrong try until the current password proves right
                g.failed(&a.email);
            }
            existing
        };
        if let Some(a) = &existing {
            if !a.check(old.unwrap_or("")) {
                return Err("Your current password isn't right.".into());
            }
        }
        let fresh = Account::new(email, password);
        let mut g = self.inner.lock().unwrap();
        // the current password was right: that try doesn't count
        if let Some(a) = &existing {
            g.unfail(&a.email);
        }
        if g.saved.accounts.get(&member) != existing.as_ref() {
            return Err("Your account changed in the meantime. Try again.".into());
        }
        if let Some(why) = g.saved.account_problem(member, email, password) {
            return Err(why);
        }
        if let Some(a) = &existing {
            g.fails.remove(&norm_email(&a.email));
        }
        g.saved.accounts.insert(member, fresh);
        // a code made for them earlier no longer signs in as them: with a
        // password they sign in with it (or Forgot password)
        g.saved.invites.retain(|_, i| i.member != member);
        // a changed password signs out their other devices: whoever knew the
        // old one (or took a device) is out
        if existing.is_some() {
            let gone = g.saved.sign_out_others(member, device);
            if !gone.is_empty() {
                let _ = self.kicks.send(Arc::new(gone));
            }
        }
        g.dirty = true;
        drop(g);
        let _ = self.access.send(EVERYONE);
        Ok(Msg::Account {
            email: email.trim().to_string(),
        })
    }

    /// Email a password-reset code when `email` has an account (quietly
    /// does nothing otherwise). False when this server can't send email.
    pub(crate) fn forgot(&self, email: &str) -> bool {
        let mail_home = self.setup_home(MAIL_FILE).to_path_buf();
        let Some(cfg) = load_mail(&mail_home) else {
            return false;
        };
        if !email_ok(email) {
            return true;
        }
        let target = {
            let mut g = self.inner.lock().unwrap();
            let key = norm_email(email);
            let now = now_ms();
            // at most one mail a minute per address
            let recent = g.mailed.get(&key).is_some_and(|t| now - t < 60_000);
            match g.saved.account_member(email) {
                Some(m) if !recent => {
                    // a code still good is sent again, not replaced: asking
                    // for mails doesn't cancel the code its owner is typing
                    let code = match g.resets.get(&key) {
                        Some((c, t)) if now - t < RESET_MINUTES * 60_000 => c.clone(),
                        _ => {
                            let c = new_code();
                            g.resets.insert(key.clone(), (c.clone(), now));
                            c
                        }
                    };
                    g.mailed.retain(|_, t| now - *t < 60_000);
                    g.mailed.insert(key, now);
                    Some((g.saved.accounts[&m].email.clone(), code))
                }
                _ => None,
            }
        };
        if let Some((to, code)) = target {
            std::thread::spawn(move || {
                if let Err(e) = mail::send(
                    &mail_home,
                    &cfg,
                    &mail::reset_mail(&to, &code, RESET_MINUTES),
                ) {
                    eprintln!("couldn't email a reset code to {}: {}", to, e);
                }
            });
        }
        true
    }

    /// Email `who`'s new invite code, when mail is set up and they have an
    /// email. (emailed to, why it failed)
    pub(crate) fn email_invite(
        &self,
        from: u64,
        who: u64,
        code: &str,
        connected_to: Option<&str>,
    ) -> (Option<String>, Option<String>) {
        let mail_home = self.setup_home(MAIL_FILE);
        let Some(cfg) = load_mail(mail_home) else {
            return (None, None);
        };
        let (to, workspace, inviter) = {
            let g = self.inner.lock().unwrap();
            let c = g.canon.as_ref();
            (
                member_field(c, who, "email"),
                c.and_then(|c| c.get("workspace"))
                    .and_then(|w| w.as_str())
                    .unwrap_or("")
                    .to_string(),
                member_field(c, from, "name"),
            )
        };
        if !email_ok(&to) {
            return (None, None);
        }
        let server = mail::link_address(&cfg, connected_to)
            .map(|s| format!("{}{}", s.trim().trim_end_matches('/'), self.prefix()));
        let m = mail::invite_mail(
            &to,
            &workspace,
            &inviter,
            code,
            server.as_deref(),
            INVITE_DAYS,
        );
        match mail::send(mail_home, &cfg, &m) {
            Ok(()) => (Some(to), None),
            Err(e) => (None, Some(e)),
        }
    }

    /// The welcome message, and what is hidden from `member`.
    pub(crate) fn welcome(
        &self,
        tag: u16,
        member: u64,
        role: Role,
        token: Option<String>,
    ) -> (String, Vec<u64>) {
        let g = self.inner.lock().unwrap();
        let email = g.saved.accounts.get(&member).map(|a| a.email.clone());
        let manages = g.saved.role_of(member).manages();
        let (snapshot, hidden) = match g
            .canon
            .as_ref()
            .map(|c| copy_for(c, member, manages).unwrap_or_else(|| (c.clone(), vec![])))
        {
            Some((s, h)) => (Some(s), h),
            None => (None, vec![]),
        };
        let gone = g
            .canon
            .as_ref()
            .map(|c| left_out(c, &hidden))
            .unwrap_or_default();
        (
            Msg::Welcome {
                tag,
                snapshot,
                rev: g.saved.rev,
                member,
                role,
                token,
                email,
                hidden,
                deltas: true,
            }
            .to_json(),
            gone,
        )
    }
}
