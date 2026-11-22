// What server.json keeps: devices, invites, accounts, roles and the rest.

use super::*;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Device {
    pub tag: u16,
    pub member: u64,
    /// sha256 of the device token (the token itself only lives on the device)
    pub token: String,
    pub created: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InviteRec {
    pub member: u64,
    pub role: Role,
    pub created: i64,
}

/// A person's account: the email and password that sign them in anywhere.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub email: String,
    /// hex
    pub salt: String,
    /// hex PBKDF2-HMAC-SHA256 of the password
    pub hash: String,
    pub rounds: u32,
    pub created: i64,
}

impl Account {
    pub(crate) fn new(email: &str, password: &str) -> Account {
        let salt = random_bytes::<16>();
        Account {
            email: email.trim().to_string(),
            salt: hex(&salt),
            hash: hex(&pbkdf2_sha256(password.as_bytes(), &salt, PBKDF2_ROUNDS)),
            rounds: PBKDF2_ROUNDS,
            created: now_ms(),
        }
    }

    /// The same password hash under another email (hashing takes a
    /// moment, so it's done before taking the hub's lock).
    pub(crate) fn with_email(mut self, email: &str) -> Account {
        self.email = email.trim().to_string();
        self
    }

    pub(crate) fn check(&self, password: &str) -> bool {
        let salt: Vec<u8> = (0..self.salt.len() / 2)
            .filter_map(|i| u8::from_str_radix(&self.salt[2 * i..2 * i + 2], 16).ok())
            .collect();
        same(
            &hex(&pbkdf2_sha256(password.as_bytes(), &salt, self.rounds)),
            &self.hash,
        )
    }
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct Saved {
    pub rev: u64,
    /// device id -> device
    pub devices: BTreeMap<String, Device>,
    /// member -> role (members not listed are plain members)
    pub roles: BTreeMap<u64, Role>,
    /// code (as shown, XXXX-XXXX-XXXX) -> invite
    pub invites: BTreeMap<String, InviteRec>,
    /// member -> account
    pub accounts: BTreeMap<u64, Account>,
    /// device id -> its push token (iPhones; see push.rs)
    pub push: BTreeMap<String, push::PushToken>,
    /// A workspace made with POST /workspaces and not set up yet: the
    /// account of the person who made it, which becomes the owner's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<Account>,
    /// Members who have ever signed in here (kept after Revoke). A code
    /// signs in as its person, so admins may make one only for someone who
    /// never came in (see admin_may_invite).
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub joined: std::collections::BTreeSet<u64>,
    /// Deleted chat messages and task comments: id -> (what it said and
    /// who wrote it, see spaces::written_marks; when it went). Only one
    /// saying the same may come back under someone else's name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub graves: BTreeMap<u64, (u64, i64)>,
    /// Messages waiting to be posted (schedule.rs).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scheduled: Vec<crate::schedule::Waiting>,
    /// Webhooks owners and admins set up (hooks.rs).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<wb_net::HookInfo>,
    /// Slash commands owners and admins added (hooks.rs).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<crate::hooks::Slash>,
    /// member -> their own reading state and notification choices, kept in
    /// step between their devices (mine.rs, `wb_sync::mine`). Saved in
    /// mine.json, on its own: it changes whenever someone opens a
    /// conversation. Older servers kept it here, so it is still read from
    /// server.json (see `load_saved`).
    #[serde(skip_serializing, deserialize_with = "shared_values")]
    pub mine: BTreeMap<u64, Arc<Value>>,
}

/// Values read from JSON, each behind an Arc (serde's own needs its "rc" feature).
fn shared_values<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<u64, Arc<Value>>, D::Error> {
    let plain: BTreeMap<u64, Value> = Deserialize::deserialize(d)?;
    Ok(plain.into_iter().map(|(k, v)| (k, Arc::new(v))).collect())
}

impl Saved {
    /// Device `device`, if `token` is the one this server gave it. Compares
    /// in constant time so a wrong guess doesn't tell how close it was.
    pub(crate) fn device_with(&self, device: &str, token: &str) -> Option<&Device> {
        self.devices
            .get(device)
            .filter(|d| same(&d.token, &hash_token(token)))
    }

    pub fn role_of(&self, member: u64) -> Role {
        self.roles.get(&member).copied().unwrap_or_default()
    }

    pub(crate) fn set_role(&mut self, member: u64, role: Role) {
        if role == Role::Member {
            self.roles.remove(&member);
        } else {
            self.roles.insert(member, role);
        }
    }

    pub(crate) fn free_tag(&self, asked: Option<u16>) -> u16 {
        let used: Vec<u16> = self.devices.values().map(|d| d.tag).collect();
        let mut tag = asked
            .map(|t| t % 1024)
            .unwrap_or(u16::from_le_bytes(random_bytes::<2>()) % 1024);
        for _ in 0..1024 {
            if !used.contains(&tag) {
                break;
            }
            tag = (tag + 1) % 1024;
        }
        tag
    }

    /// Sign a device in as `member`; returns (tag, token).
    /// Take back `member`'s sign-ins on every device but `keep`, with those
    /// devices' push tokens. Returns the devices to kick.
    pub(crate) fn sign_out_others(&mut self, member: u64, keep: &str) -> Vec<String> {
        let gone: Vec<String> = self
            .devices
            .iter()
            .filter(|(id, d)| d.member == member && id.as_str() != keep)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &gone {
            self.devices.remove(id);
            self.push.remove(id);
        }
        gone
    }

    pub(crate) fn add_device(
        &mut self,
        device: &str,
        member: u64,
        asked_tag: Option<u16>,
    ) -> (u16, String) {
        self.joined.insert(member);
        let old = self.devices.remove(device);
        let tag = old
            .map(|d| d.tag)
            .unwrap_or_else(|| self.free_tag(asked_tag));
        let token = hex(&random_bytes::<32>());
        self.devices.insert(
            device.to_string(),
            Device {
                tag,
                member,
                token: hash_token(&token),
                created: now_ms(),
            },
        );
        (tag, token)
    }

    /// Keep (or with an empty token, forget) `device`'s push token. True when
    /// that changed anything.
    pub(crate) fn set_push_token(
        &mut self,
        device: &str,
        token: &str,
        platform: &str,
        env: &str,
        workspace: Option<&str>,
        prefs: Option<wb_net::PushPrefs>,
    ) -> bool {
        let token = token.trim().to_ascii_lowercase();
        let workspace = workspace
            .map(str::trim)
            .filter(|w| push::workspace_key_ok(w))
            .map(str::to_string);
        if token.is_empty() {
            return self.push.remove(device).is_some();
        }
        if !self.devices.contains_key(device)
            || !push::token_ok(&token)
            || !platform.trim().eq_ignore_ascii_case("ios")
        {
            return false;
        }
        let env = push::norm_env(env);
        if self.push.get(device).is_some_and(|t| {
            t.token == token && t.env == env && t.workspace == workspace && t.prefs == prefs
        }) {
            return false;
        }
        // one phone, one entry: a token that moved to a new device id (the app
        // was installed again) leaves the old one
        self.push.retain(|d, t| d == device || t.token != token);
        self.push.insert(
            device.to_string(),
            push::PushToken {
                token,
                platform: "ios".into(),
                env: env.into(),
                updated: now_ms(),
                workspace,
                prefs,
            },
        );
        true
    }

    pub(crate) fn drop_expired_invites(&mut self, now: i64) {
        self.invites.retain(|_, i| invite_alive(i, now));
    }

    /// The code `member` can still sign in with (expired ones don't count).
    pub fn open_invite(&self, member: u64, now: i64) -> Option<&String> {
        self.invites
            .iter()
            .find(|(_, i)| i.member == member && invite_alive(i, now))
            .map(|(c, _)| c)
    }

    /// A fresh one-time code for `member`, replacing any earlier one.
    pub fn make_invite(&mut self, member: u64, role: Role) -> String {
        self.invites.retain(|_, i| i.member != member);
        let code = new_code();
        self.invites.insert(
            code.clone(),
            InviteRec {
                member,
                role,
                created: now_ms(),
            },
        );
        code
    }

    pub(crate) fn find_invite(&mut self, typed: &str) -> Option<String> {
        self.drop_expired_invites(now_ms());
        let want = normalize_code(typed);
        self.invites
            .keys()
            .find(|k| normalize_code(k) == want)
            .cloned()
    }

    #[cfg(test)]
    pub(crate) fn take_invite(&mut self, typed: &str) -> Option<InviteRec> {
        let key = self.find_invite(typed)?;
        self.invites.remove(&key)
    }

    /// The member whose account has this email.
    pub fn account_member(&self, email: &str) -> Option<u64> {
        let want = norm_email(email);
        self.accounts
            .iter()
            .find(|(_, a)| norm_email(&a.email) == want)
            .map(|(m, _)| *m)
    }

    /// Why `member` can't have an account with `email`, if they can't.
    pub(crate) fn account_problem(
        &self,
        member: u64,
        email: &str,
        password: &str,
    ) -> Option<String> {
        if !email_ok(email) {
            return Some("Type a valid email address.".into());
        }
        if self.account_member(email).is_some_and(|m| m != member) {
            return Some("Someone else in this workspace already signs in with that email.".into());
        }
        password_problem(password)
    }

    /// Whether an admin (not only the owner) may make and see a code for
    /// `member`: a code signs in as that person, so only for plain members
    /// who have never come in (new people). Otherwise an admin could sign
    /// in as someone already here, with or without a password (Revoke
    /// deletes the password, so "has no account" alone isn't enough), and
    /// read their messages.
    pub(crate) fn admin_may_invite(&self, member: u64) -> bool {
        self.role_of(member) == Role::Member
            && !self.accounts.contains_key(&member)
            && !self.joined.contains(&member)
            && !self.devices.values().any(|d| d.member == member)
    }

    /// Everyone the workspace knows, with their access, as `viewer` may see
    /// it: a code signs in as its person, so admins see only the codes they
    /// may make (see admin_may_invite); the owner sees every code.
    pub(crate) fn people(&self, canon: Option<&Value>, viewer: Role) -> Vec<Person> {
        let mut ids: Vec<u64> = canon
            .and_then(|c| c.get("members"))
            .and_then(|m| m.as_array())
            .map(|a| a.iter().filter_map(|m| m.get("id")?.as_u64()).collect())
            .unwrap_or_default();
        for d in self.devices.values() {
            if !ids.contains(&d.member) {
                ids.push(d.member);
            }
        }
        ids.iter()
            .map(|&member| Person {
                member,
                role: self.role_of(member),
                devices: self.devices.values().filter(|d| d.member == member).count(),
                invite: self
                    .open_invite(member, now_ms())
                    .cloned()
                    .filter(|_| viewer == Role::Owner || self.admin_may_invite(member)),
                account: self.accounts.contains_key(&member),
            })
            .collect()
    }
}
