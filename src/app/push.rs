// Notifications while the app is quit (macOS).
//
// The team server notifies a Mac whose app isn't running through Apple's
// push service, as it does iPhones (server/src/push.rs). For that the app
// sends `push_token {token, platform: "macos"}`: on the open workspace's
// connection with this computer's choices for it (Settings > Notifications,
// turned into `wb_net::PushPrefs`), again after each welcome and whenever
// they change; and on each other team workspace's watch connection
// (watch.rs), without choices, so those notify the quit app too. While the
// app is connected the server sends nothing (it shows its notices itself),
// and what came while it was quit is a catch-up that isn't announced again
// (notify.rs).
//
// `workspace` in the token is the workspace file's name in hex, so a click
// on the banner opens that workspace: the notifier hands back
// "push:WS:CHANNEL[:THREAD]" and `open_pushed` turns it into a target.
//
// "Notify me while Workboard is quit" (Settings > Notifications, on unless
// turned off) is this computer's own (`Data::push_while_quit`): off, the
// app sends an empty token everywhere instead, and the servers forget this
// Mac's.
//
// The token comes from Apple only in the installed app, signed with its
// provisioning profile (scripts/bundle.sh). Run any other way there is none
// and nothing is sent.

use super::*;

/// While no token came yet, look again this often...
const WAIT_SECS: u64 = 2;
/// ...for this long after the start.
const WAIT_FOR_SECS: u64 = 120;

#[derive(Default)]
pub struct PushRt {
    started: Option<std::time::Instant>,
    /// what the open workspace's connection was last sent (cleared by each
    /// welcome)
    pub(super) sent: Option<String>,
    /// "Notify me while Workboard is quit" turned off
    pub(super) off: bool,
}

/// The `workspace` sent with the token: the file's name in hex (the server
/// keeps 1 to 64 letters and digits).
pub(super) fn ws_key(file: &str) -> Option<String> {
    let hex: String = file.bytes().map(|b| format!("{:02x}", b)).collect();
    (1..=64).contains(&hex.len()).then_some(hex)
}

/// The workspace file a `ws_key` stands for.
pub(super) fn ws_file(key: &str) -> Option<String> {
    if key.is_empty() || key.len() % 2 != 0 {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..key.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(key.get(i..i + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

/// What `settings` (and "Pause notifications" until `muted_until`) ask of
/// the server's pushes. Banners off: nothing.
pub(super) fn push_prefs(settings: &NotifySettings, muted_until: i64) -> wb_net::PushPrefs {
    let on = settings.desktop;
    wb_net::PushPrefs {
        direct: on && settings.direct,
        mentions: on && settings.mentions,
        replies: on && settings.replies,
        channel_default: if on {
            settings.channel_default.clone()
        } else {
            "nothing".into()
        },
        channel_modes: if on {
            settings
                .channel_modes
                .iter()
                .map(|(channel, mode)| wb_net::ChannelMode {
                    channel: *channel,
                    mode: mode.clone(),
                })
                .collect()
        } else {
            vec![]
        },
        keywords: if on && settings.mentions {
            settings.keyword_list()
        } else {
            vec![]
        },
        muted_until,
    }
}

/// The `push_token` message for workspace `file`.
pub(super) fn token_msg(token: &str, file: &str, prefs: Option<wb_net::PushPrefs>) -> wb_net::Msg {
    wb_net::Msg::PushToken {
        token: token.to_string(),
        platform: "macos".into(),
        // a Developer ID app's token is always a production one
        env: "production".into(),
        workspace: ws_key(file),
        prefs,
    }
}

impl App {
    /// Once a frame: ask Apple for the token, then keep the servers told.
    pub(super) fn push_tick(&mut self, ctx: Option<&egui::Context>) {
        if !cfg!(target_os = "macos") {
            return;
        }
        if self.push_rt.started.is_none() {
            self.push_rt.off = !Data::push_while_quit();
            crate::notifier::start_push();
            self.push_rt.started = Some(std::time::Instant::now());
        }
        let started = self.push_rt.started.unwrap_or_else(std::time::Instant::now);
        let Some(token) = crate::notifier::push_token() else {
            if started.elapsed().as_secs() < WAIT_FOR_SECS {
                if let Some(ctx) = ctx {
                    ctx.request_repaint_after(std::time::Duration::from_secs(WAIT_SECS));
                }
            }
            return;
        };
        self.push_to_servers(&token);
    }

    /// Send `token` where it wasn't yet: the open workspace's connection
    /// (with its choices) and the watch connections. Turned off: an empty
    /// token instead, which stops the pushes.
    pub(super) fn push_to_servers(&mut self, token: &str) {
        let off = self.push_rt.off;
        if let Some(net) = &self.sync_rt.net {
            let msg = self.push_wanted(token);
            let key = format!("{}|{}", self.ws_file, msg.to_json());
            if self.push_rt.sent.as_deref() != Some(key.as_str()) {
                net.send(msg);
                self.push_rt.sent = Some(key);
            }
        }
        self.watch_push(if off { "" } else { token });
    }

    /// The `push_token` the open workspace's server should have.
    pub(super) fn push_wanted(&self, token: &str) -> wb_net::Msg {
        if self.push_rt.off {
            token_msg("", &self.ws_file, None)
        } else {
            let prefs = push_prefs(&self.data.extra.notify, self.data.extra.muted_until);
            token_msg(token, &self.ws_file, Some(prefs))
        }
    }

    /// "Notify me while Workboard is quit" switched: kept on this computer,
    /// and the servers are told on the next frame.
    pub(super) fn set_push_while_quit(&mut self, on: bool) {
        self.push_rt.off = !on;
        if self.persist {
            Data::set_push_while_quit(on);
        }
    }

    /// A clicked pushed banner ("push:WS:CHANNEL[:THREAD]"): the target it
    /// opens.
    pub(super) fn open_pushed(&self, rest: &str) -> String {
        let (ws, chat) = rest.split_once(':').unwrap_or((rest, ""));
        let inner = if chat.is_empty() {
            "inbox".to_string()
        } else {
            format!("chat:{}", chat)
        };
        match ws_file(ws) {
            Some(file) if file != self.ws_file => format!("ws:{}:{}", file, inner),
            _ => inner,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_keys_go_both_ways() {
        for f in ["data.json", "workspace-1794847221.json"] {
            let k = ws_key(f).unwrap();
            assert!(k.chars().all(|c| c.is_ascii_hexdigit()) && k.len() <= 64);
            assert_eq!(ws_file(&k).as_deref(), Some(f));
        }
        assert_eq!(ws_key(&"x".repeat(40)), None);
        assert_eq!(ws_file("abc"), None);
        assert_eq!(ws_file("zz"), None);
    }

    #[test]
    fn banners_off_asks_for_nothing() {
        let mut s = NotifySettings::default();
        s.keywords = "deploy, 배포".into();
        s.channel_modes = vec![(7, "all".into())];
        let p = push_prefs(&s, 5);
        assert!(p.direct && p.mentions && p.replies);
        assert_eq!(p.keywords, vec!["deploy".to_string(), "배포".to_string()]);
        assert_eq!(p.channel_modes.len(), 1);
        assert_eq!(p.muted_until, 5);
        s.desktop = false;
        let p = push_prefs(&s, 0);
        assert!(!p.direct && !p.mentions && !p.replies);
        assert_eq!(p.channel_default, "nothing");
        assert!(p.channel_modes.is_empty() && p.keywords.is_empty());
    }
}
