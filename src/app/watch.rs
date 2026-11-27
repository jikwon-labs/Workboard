// Notifications from the other team workspaces on this computer.
//
// Only the open workspace is connected and synced. Each other workspace on a
// team server that this computer is signed in to gets a `wb_net::Watcher`: a
// small connection that hears of my new messages there, the ones an iPhone
// gets a push for (direct messages, mentions, replies in my threads), and
// nothing else. They are announced like the open workspace's notices
// (`deliver`: a toast in front, else a banner), with the workspace's name in
// the title. A click opens that workspace and, once its copy has the
// conversation, the conversation. Each one heard also counts towards the red
// number beside that workspace in the switch menu and on the workspace
// button (`ws_unread`), until that workspace is opened. The counts are kept
// while the app runs: what came in while it was quit isn't counted.
//
// The list of watchers is looked at again when the open workspace changes,
// when a sign-in token is saved or forgotten, and every LOOK_SECS (a
// workspace joined meanwhile). A watcher the server turned away (signed out
// there, workspace deleted) isn't started again until the tokens change.

use super::notify::{Kind, Note};
use super::*;

/// How often the list of workspaces is read again.
const LOOK_SECS: u64 = 30;
/// How long a click waits for the workspace's copy to have the conversation.
const OPEN_WAIT_SECS: u64 = 30;

struct Watching {
    addr: String,
    device: String,
    name: String,
    /// None once the server turned it away
    watcher: Option<wb_net::Watcher>,
    started: bool,
    /// the push token this watcher was handed last (push.rs; "" for off)
    pushed: Option<String>,
}

#[derive(Default)]
pub struct WatchRt {
    /// workspace file -> its watcher
    on: HashMap<String, Watching>,
    /// what the list was worked out for: (open workspace, token generation)
    looked: Option<(String, u64)>,
    looked_at: Option<std::time::Instant>,
    /// each workspace file's server and device (None: not on a server);
    /// files not open don't change, so this is read once per file
    cfgs: HashMap<String, Option<(String, String)>>,
    /// a click for another workspace: what to open once it's open there
    then_open: Option<(String, std::time::Instant)>,
    /// how many notes came in (makes keys unique)
    count: u64,
    /// workspace file -> notes heard from it since it was last open
    pub(super) unread: HashMap<String, usize>,
}

impl WatchRt {
    /// Whether the watcher for `file` is signed in and listening (tests).
    #[cfg(test)]
    pub(super) fn listening(&self, file: &str) -> bool {
        self.on.get(file).is_some_and(|w| w.started)
    }

    /// The workspace files being watched (tests).
    #[cfg(test)]
    pub(super) fn files(&self) -> Vec<String> {
        let mut f: Vec<String> = self.on.keys().cloned().collect();
        f.sort();
        f
    }
}

/// A workspace file's team server and device, from the file.
fn read_cfg(file: &str) -> Option<(String, String)> {
    let path = if file == "data.json" {
        Data::path()
    } else {
        Data::dir().join(file)
    };
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let cfg: crate::sync::SharedCfg =
        serde_json::from_value(v.get("extra")?.get("shared")?.clone()).ok()?;
    Some((cfg.server?, cfg.device))
}

/// What a click on a note from workspace `file` opens.
pub(super) fn target_in(file: &str, target: &str) -> String {
    format!("ws:{}:{}", file, target)
}

/// (workspace file, target there) of a target made by `target_in`.
pub(super) fn split_target(target: &str) -> Option<(&str, &str)> {
    let rest = target.strip_prefix("ws:")?;
    let (file, inner) = rest.split_once(':')?;
    (!file.is_empty()).then_some((file, inner))
}

impl App {
    /// Once a frame: keep a watcher on each other team workspace and
    /// announce what they heard.
    pub(super) fn watch_tick(&mut self, ctx: Option<&egui::Context>) {
        self.watch_open_pending();
        let key = (self.ws_file.clone(), super::together::token_gen());
        let due = self
            .watch_rt
            .looked_at
            .is_none_or(|t| t.elapsed().as_secs() >= LOOK_SECS);
        if self.watch_rt.looked.as_ref() != Some(&key) || due {
            self.watch_look(ctx, key);
        }
        let mut heard = vec![];
        for (file, w) in self.watch_rt.on.iter_mut() {
            let Some(watcher) = &w.watcher else {
                continue;
            };
            for e in watcher.events() {
                match e {
                    wb_net::Watch::Started => w.started = true,
                    wb_net::Watch::Note {
                        title,
                        body,
                        channel,
                        parent,
                    } => heard.push((file.clone(), w.name.clone(), title, body, channel, parent)),
                    wb_net::Watch::Refused(_) | wb_net::Watch::Gone(_) => {
                        w.started = false;
                        w.watcher = None;
                    }
                }
            }
        }
        for (file, name, title, body, channel, parent) in heard {
            self.watch_rt.count += 1;
            if file != self.ws_file {
                *self.watch_rt.unread.entry(file.clone()).or_default() += 1;
            }
            let target = match (channel, parent) {
                (Some(c), Some(p)) => format!("chat:{}:{}", c, p),
                (Some(c), None) => format!("chat:{}", c),
                _ => "inbox".to_string(),
            };
            let kind = if parent.is_some() {
                Kind::Reply
            } else if title.contains(" in #") {
                Kind::Mention
            } else {
                Kind::Direct
            };
            let title = if name.trim().is_empty() {
                title
            } else {
                format!("{} \u{b7} {}", name.trim(), title)
            };
            self.deliver(Note {
                key: format!("watch:{}:{}", file, self.watch_rt.count),
                kind,
                title,
                body,
                target: target_in(&file, &target),
            });
        }
    }

    /// Work out which workspaces to watch and start or stop watchers.
    fn watch_look(&mut self, ctx: Option<&egui::Context>, key: (String, u64)) {
        let tokens_changed = self.watch_rt.looked.as_ref().is_some_and(|k| k.1 != key.1);
        // the workspace just left may have changed (joined a server, say)
        if let Some((left, _)) = &self.watch_rt.looked {
            if *left != key.0 {
                self.watch_rt.cfgs.remove(left);
            }
        }
        self.watch_rt.looked = Some(key);
        self.watch_rt.looked_at = Some(std::time::Instant::now());
        let mut list = Data::workspaces();
        if !list.iter().any(|w| w.file == "data.json") && Data::path().is_file() {
            list.push(WsEntry {
                name: String::new(),
                file: "data.json".into(),
            });
        }
        let mut want: HashMap<String, (String, String, String)> = HashMap::new();
        for w in list {
            if w.file == self.ws_file {
                continue;
            }
            let cfg = self
                .watch_rt
                .cfgs
                .entry(w.file.clone())
                .or_insert_with(|| read_cfg(&w.file))
                .clone();
            if let Some((addr, device)) = cfg {
                want.insert(w.file, (addr, device, w.name));
            }
        }
        // a workspace opened, left the server or gone from this computer
        // has nothing waiting any more
        self.watch_rt
            .unread
            .retain(|file, _| want.contains_key(file));
        self.watch_rt.on.retain(|file, w| {
            want.get(file)
                .is_some_and(|(a, d, _)| *a == w.addr && *d == w.device)
                // turned away: tried again once the tokens change
                && (w.watcher.is_some() || !tokens_changed)
        });
        for (file, (addr, device, name)) in want {
            if let Some(w) = self.watch_rt.on.get_mut(&file) {
                w.name = name;
                continue;
            }
            let Some(token) = super::together::load_token(&addr, &device) else {
                continue;
            };
            let wake: wb_net::Wake = match ctx {
                Some(c) => {
                    let c = c.clone();
                    std::sync::Arc::new(move || c.request_repaint())
                }
                None => std::sync::Arc::new(|| {}),
            };
            let watcher = wb_net::Watcher::start(
                &addr,
                &device,
                &token,
                Some(super::together::APP_VERSION.to_string()),
                wake,
            );
            self.watch_rt.on.insert(
                file,
                Watching {
                    addr,
                    device,
                    name,
                    watcher: Some(watcher),
                    started: false,
                    pushed: None,
                },
            );
        }
    }

    /// Hand this Mac's push token ("": stop the pushes) to the watchers
    /// that don't have it yet.
    pub(super) fn watch_push(&mut self, token: &str) {
        for (file, w) in self.watch_rt.on.iter_mut() {
            if w.pushed.as_deref() == Some(token) {
                continue;
            }
            if let Some(watcher) = &w.watcher {
                watcher.set_push(super::push::token_msg(token, file, None));
                w.pushed = Some(token.to_string());
            }
        }
    }

    /// The red number for workspace `file`: what was heard from it since it
    /// was last open (pausing notifications leaves it, as on the rail).
    pub(super) fn ws_unread(&self, file: &str) -> usize {
        if file == self.ws_file {
            return 0;
        }
        self.watch_rt.unread.get(file).copied().unwrap_or(0)
    }

    /// The red number on the workspace button: all the other workspaces'.
    pub(super) fn ws_unread_total(&self) -> usize {
        self.watch_rt.unread.keys().map(|f| self.ws_unread(f)).sum()
    }

    /// Open a note's target in another workspace: switch to it, then open
    /// the conversation once its copy has it.
    pub(super) fn open_in_workspace(&mut self, file: &str, target: &str) {
        if file != self.ws_file {
            let known = file == "data.json" || Data::workspaces().iter().any(|w| w.file == file);
            if !known {
                return;
            }
            self.switch_workspace(file);
            if self.ws_file != file {
                return;
            }
        }
        self.watch_rt.then_open = Some((target.to_string(), std::time::Instant::now()));
        self.watch_open_pending();
    }

    /// The conversation a click asked for, once it's here.
    fn watch_open_pending(&mut self) {
        let Some((target, since)) = self.watch_rt.then_open.clone() else {
            return;
        };
        let ch = target
            .strip_prefix("chat:")
            .and_then(|r| r.split(':').next())
            .and_then(|c| c.parse::<u64>().ok());
        let here = ch.is_none_or(|c| self.data.channels.iter().any(|x| x.id == c));
        if here {
            self.watch_rt.then_open = None;
            self.open_target(&target);
        } else if since.elapsed().as_secs() >= OPEN_WAIT_SECS {
            self.watch_rt.then_open = None;
        }
    }
}
