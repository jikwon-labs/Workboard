// Telling iPhones about news through Apple's push service.

use super::*;

impl Hub {
    /// Merge the copy `member` sent from `device` into the team's copy; tell
    /// everyone when it changed. True when some of its changes were turned
    /// down (and put back), to tell its sender.
    pub(crate) fn push(self: &Arc<Self>, member: u64, device: &str, data: Value) -> bool {
        if !copy_shape_ok(&data) {
            eprintln!(
                "ignored a copy from member {} that isn't a workspace",
                member
            );
            return false;
        }
        let mut data = data;
        wb_sync::make_shareable(&mut data);
        // no change time from the future (see clamp_future)
        spaces::clamp_future(&mut data, now_ms() + FUTURE_SKEW_MS);
        // only conversations this copy has can get new messages from it
        let chans = push::channel_ids(&data);
        let mut g = self.state();
        let members_before = member_ids(g.canon.as_ref());
        // the messages already there, to tell which ones are new (only when
        // someone could be notified)
        let before = match &g.canon {
            Some(c) if self.pusher.is_some() && !g.saved.push.is_empty() => {
                Some(push::message_ids_in(c, Some(&chans)))
            }
            _ => None,
        };
        let fix = match &g.canon {
            Some(c) => guard_push(c, &mut data, member, &g.saved),
            None => spaces::PutBack::default(),
        };
        let claimer = g.saved.devices.contains_key(device);
        let changed = match g.canon.as_mut() {
            // the workspace comes from the computer that claimed the server
            // (a claim taken over after claim_wait no longer counts)
            None if !claimer => false,
            None => {
                // the owner's first copy brings the workspace
                let mut data = data;
                let owners: Vec<u64> = g
                    .saved
                    .roles
                    .iter()
                    .filter(|(_, r)| **r == Role::Owner)
                    .map(|(m, _)| *m)
                    .collect();
                for m in owners {
                    label_role(&mut data, m, Role::Owner);
                }
                g.canon = Some(data);
                true
            }
            Some(c) => {
                let written = spaces::written_marks(c);
                let changed = wb_sync::merge(c, &[data]);
                if changed {
                    // the merge's fingerprints are this server's own
                    wb_sync::make_shareable(c);
                }
                // what was turned down goes back to its sender with everyone's update
                spaces::put_back(c, &fix);
                if changed {
                    let now = now_ms();
                    let after = spaces::written_marks(c);
                    let graves = &mut g.saved.graves;
                    for (id, mark) in written {
                        if !after.contains_key(&id) {
                            graves.insert(id, (mark, now));
                        }
                    }
                    graves.retain(|_, (_, t)| now - *t < GRAVE_DAYS * 86_400_000);
                }
                changed || !fix.is_empty()
            }
        };
        if changed {
            if let (Some(before), Some(canon)) = (&before, &g.canon) {
                let notes = push::notes_in(canon, before, Some(&chans), &[member], now_ms() / 1000);
                self.notify(&g.saved, notes);
            }
            self.broadcast_canon(&mut g, false);
            // someone added or removed people: owners and admins see the new list
            if member_ids(g.canon.as_ref()) != members_before {
                drop(g);
                let _ = self.access.send(EVERYONE);
            }
        }
        fix.refused
    }

    /// Send `notes` to the devices of their members that have a push token
    /// and aren't connected, MAX_PER_DEVICE each at most, on another thread.
    pub(crate) fn notify(self: &Arc<Self>, saved: &Saved, notes: Vec<push::Note>) {
        let Some(sender) = self.pusher.clone() else {
            return;
        };
        if notes.is_empty() {
            return;
        }
        let online = self.online.locked().clone();
        let mut jobs: Vec<(String, push::PushToken, Value)> = vec![];
        for (device, tok) in &saved.push {
            let Some(d) = saved.devices.get(device) else {
                continue;
            };
            if online.get(device).is_some_and(|n| *n > 0) {
                continue;
            }
            let mine: Vec<&push::Note> = notes.iter().filter(|n| n.member == d.member).collect();
            for p in push::batch(&mine) {
                jobs.push((device.clone(), tok.clone(), p));
            }
        }
        if jobs.is_empty() {
            return;
        }
        let hub = self.clone();
        std::thread::spawn(move || {
            for (device, tok, payload) in jobs {
                match sender.send(&tok.env, &tok.token, &payload) {
                    push::Outcome::Sent => {}
                    push::Outcome::Gone => {
                        // the app was removed (or the token was never good): forget it
                        let mut g = hub.state();
                        if g.saved
                            .push
                            .get(&device)
                            .is_some_and(|t| t.token == tok.token)
                        {
                            g.saved.push.remove(&device);
                            g.dirty = true;
                        }
                    }
                    push::Outcome::Failed(why) => {
                        eprintln!("couldn't send a notification to device {}: {}", device, why)
                    }
                }
            }
        });
    }

    /// An iPhone's push token (`push_token`).
    pub(crate) fn set_push_token(&self, device: &str, token: &str, platform: &str, env: &str) {
        let mut g = self.state();
        if g.saved.set_push_token(device, token, platform, env) {
            g.dirty = true;
        }
    }
}
