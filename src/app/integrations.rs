// Slash commands and webhooks on the Mac.
//
// A message starting with "/" is a command: the built-in ones work here
// (/shrug, /me, /topic, /mute, /unmute, /remind), the ones an owner or
// admin added go to the team server, which sends them to their address and
// posts the answer (server/src/hooks.rs). Typing "/" at the start of the
// box lists them.
//
// Integrations (from a channel's settings, owners and admins on a server):
// the channel's incoming webhooks (an address to post to) and outgoing
// webhooks (an address its messages go to), and the workspace's slash
// commands.

use super::*;
use wb_net::{CommandInfo, HookInfo};

/// The commands that work without a server: (name, what it does).
pub(super) const BUILT_IN: &[(&str, &str)] = &[
    (
        "shrug",
        "Add \u{00AF}\\_(\u{30C4})_/\u{00AF} to your message",
    ),
    ("me", "Say what you're doing, in italics"),
    ("topic", "Set this channel's topic"),
    ("mute", "Turn off notifications for this channel"),
    ("unmute", "Turn notifications for this channel back on"),
    ("remind", "Remind yourself: /remind call Sam in 30 minutes"),
    ("poll", "Ask the conversation a question with choices"),
];

/// Examples in the address and name fields (the same in every language).
const URL_EXAMPLE: &str = "https://...";
const COMMAND_EXAMPLE: &str = "/deploy";

/// What a "/" message turned out to be.
#[derive(Debug, PartialEq)]
pub(super) enum Slash {
    /// Not a command ("/usr/bin is full"): send it as it is.
    Plain,
    /// Done here (or sent to the server); nothing to post.
    Done,
    /// Post this instead.
    Post(String),
    /// No such command: the text goes back in the box.
    Unknown(String),
}

/// "in 30 minutes" / "in 2 hours" at the end of a /remind: (what, seconds).
pub(super) fn remind_parts(rest: &str) -> Option<(String, i64)> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let n = words.len();
    if n < 3 || !words[n - 3].eq_ignore_ascii_case("in") {
        return None;
    }
    let count: i64 = words[n - 2].parse().ok().filter(|c| *c > 0)?;
    let unit = words[n - 1].to_ascii_lowercase();
    let secs = match unit.trim_end_matches('s') {
        "minute" | "min" | "m" => 60,
        "hour" | "hr" | "h" => 3600,
        "day" | "d" => 86_400,
        _ => return None,
    };
    let what = words[..n - 3].join(" ");
    let what = what.strip_prefix("me ").unwrap_or(&what).trim().to_string();
    (!what.is_empty()).then_some((what, count * secs))
}

impl App {
    /// The server's slash commands (none until it answered, or without one).
    pub(super) fn server_commands(&self) -> Vec<CommandInfo> {
        self.sync_rt
            .integrations
            .as_ref()
            .map(|i| i.1.clone())
            .unwrap_or_default()
    }

    /// The commands the "/" list offers for what is typed: (name, description).
    pub(super) fn slash_choices(&self, typed: &str) -> Vec<(String, String)> {
        let Some(q) = typed.strip_prefix('/') else {
            return vec![];
        };
        if q.contains(char::is_whitespace) {
            return vec![];
        }
        let q = q.to_lowercase();
        BUILT_IN
            .iter()
            .map(|(n, d)| (n.to_string(), tr(d).to_string()))
            .chain(
                self.server_commands()
                    .into_iter()
                    .map(|c| (c.name, c.description)),
            )
            .filter(|(n, _)| n.starts_with(&q))
            .collect()
    }

    /// Run `text` (starting with "/") written in channel `ch` (thread `parent`).
    pub(super) fn slash(&mut self, ch: u64, parent: Option<u64>, text: &str) -> Slash {
        let Some(body) = text.strip_prefix('/') else {
            return Slash::Plain;
        };
        let (name, rest) = body
            .split_once(char::is_whitespace)
            .map_or((body, ""), |(n, r)| (n, r.trim()));
        if name.is_empty() || name.contains('/') {
            return Slash::Plain;
        }
        let lower = name.to_lowercase();
        match lower.as_str() {
            "shrug" => {
                let s = "\u{00AF}\\_(\u{30C4})_/\u{00AF}";
                return Slash::Post(if rest.is_empty() {
                    s.to_string()
                } else {
                    format!("{} {}", rest, s)
                });
            }
            "me" if !rest.is_empty() => return Slash::Post(format!("*{}*", rest)),
            "topic" => {
                if let Some(c) = self
                    .data
                    .channels
                    .iter_mut()
                    .find(|c| c.id == ch && c.kind == ChannelKind::Channel)
                {
                    c.topic = rest.to_string();
                    self.dirty = true;
                    self.toast(tr("Topic changed"));
                }
                return Slash::Done;
            }
            "poll" => {
                self.open_new_poll(ch, parent, rest);
                return Slash::Done;
            }
            "mute" => {
                self.data.extra.notify.set_channel_mode(ch, "nothing");
                self.dirty = true;
                self.toast(tr("Notifications for this channel are off"));
                return Slash::Done;
            }
            "unmute" => {
                let default = self.data.extra.notify.channel_default.clone();
                let back = if default == "nothing" {
                    "mentions".to_string()
                } else {
                    default
                };
                self.data.extra.notify.set_channel_mode(ch, &back);
                self.dirty = true;
                self.toast(tr("Notifications for this channel are on"));
                return Slash::Done;
            }
            "remind" => {
                let Some((what, secs)) = remind_parts(rest) else {
                    self.toast(tr("Try: /remind call Sam in 30 minutes"));
                    return Slash::Unknown(text.to_string());
                };
                let id = self.data.new_id();
                self.data.extra.reminders.push(Reminder {
                    id,
                    text: what,
                    due_ts: now_ts() + secs,
                    done: false,
                    owner: self.data.me,
                    ..Default::default()
                });
                self.dirty = true;
                self.toast(tr("Reminder created"));
                return Slash::Done;
            }
            _ => {}
        }
        if self.server_commands().iter().any(|c| c.name == lower) {
            if let Some(net) = &self.sync_rt.net {
                net.send(wb_net::Msg::Command {
                    channel: ch,
                    parent,
                    name: lower.clone(),
                    text: rest.to_string(),
                });
                self.toast(trf!("Sent /{}", lower));
                return Slash::Done;
            }
        }
        self.toast(trf!("There's no /{} command.", name));
        Slash::Unknown(text.to_string())
    }

    /// The address an incoming webhook takes posts at.
    pub(super) fn hook_address(&self, token: &str) -> String {
        self.data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.clone())
            .and_then(|a| wb_net::http_address(&a, &format!("/hooks/{}", token)))
            .unwrap_or_default()
    }

    fn ask(&self, m: wb_net::Msg) {
        if let Some(net) = &self.sync_rt.net {
            net.send(m);
        }
    }

    /// The Integrations window for channel `ch`. True when it should close.
    pub(super) fn integrations_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        ch: u64,
        form: &mut IntegrationForm,
    ) -> bool {
        if !self.on_server() || !self.i_manage() {
            ui.label(tr(
                "Webhooks and slash commands are set up by an owner or admin of a team server.",
            ));
            return false;
        }
        let (hooks, commands) = self.sync_rt.integrations.clone().unwrap_or_default();
        ui.label(RichText::new(tr("Incoming webhooks")).strong());
        ui.label(
            RichText::new(tr("Other services post to this channel by sending {\"text\": \"...\"} to the address."))
                .small()
                .color(t.text2),
        );
        for h in hooks.iter().filter(|h| h.channel == ch && h.kind == "in") {
            let addr = self.hook_address(&h.token);
            ui.horizontal(|ui| {
                ui.label(RichText::new(&h.name).strong());
                ui.label(RichText::new(&addr).small().monospace().color(t.text2));
                let c = ui.small_button(tr("Copy"));
                name_it(&c, &format!("Copy address {}", h.name));
                if c.clicked() {
                    ui.ctx().copy_text(addr.clone());
                    self.toast(tr("Copied"));
                }
                if ui.small_button(tr("Remove")).clicked() {
                    self.ask(wb_net::Msg::HookRemove {
                        token: h.token.clone(),
                    });
                }
            });
        }
        ui.horizontal(|ui| {
            let n = ui.add(
                field(&mut form.in_name)
                    .hint_text(tr("Name, like CI"))
                    .desired_width(180.0),
            );
            name_it(&n, "Incoming webhook name");
            let b = ui.button(tr("Add incoming webhook"));
            name_it(&b, "Add incoming webhook");
            if b.clicked() {
                self.ask(wb_net::Msg::HookAdd {
                    hook: HookInfo {
                        kind: "in".into(),
                        channel: ch,
                        name: std::mem::take(&mut form.in_name),
                        ..Default::default()
                    },
                });
            }
        });
        ui.add_space(10.0);
        ui.label(RichText::new(tr("Outgoing webhooks")).strong());
        ui.label(
            RichText::new(tr("Each new message here (starting with the trigger word, if there is one) is sent to the address; an answer {\"text\": \"...\"} is posted back."))
                .small()
                .color(t.text2),
        );
        for h in hooks.iter().filter(|h| h.channel == ch && h.kind == "out") {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&h.name).strong());
                ui.label(RichText::new(&h.url).small().color(t.text2));
                if !h.trigger.is_empty() {
                    ui.label(RichText::new(&h.trigger).small().monospace());
                }
                if ui.small_button(tr("Remove")).clicked() {
                    self.ask(wb_net::Msg::HookRemove {
                        token: h.token.clone(),
                    });
                }
            });
        }
        ui.horizontal(|ui| {
            let n = ui.add(
                field(&mut form.out_name)
                    .hint_text(tr("Name"))
                    .desired_width(100.0),
            );
            name_it(&n, "Outgoing webhook name");
            let u = ui.add(
                field(&mut form.out_url)
                    .hint_text(URL_EXAMPLE)
                    .desired_width(200.0),
            );
            name_it(&u, "Outgoing webhook address");
            let w = ui.add(
                field(&mut form.out_trigger)
                    .hint_text(tr("Trigger word"))
                    .desired_width(90.0),
            );
            name_it(&w, "Outgoing webhook trigger");
            let b = ui.button(tr("Add"));
            name_it(&b, "Add outgoing webhook");
            if b.clicked() {
                self.ask(wb_net::Msg::HookAdd {
                    hook: HookInfo {
                        kind: "out".into(),
                        channel: ch,
                        name: std::mem::take(&mut form.out_name),
                        url: std::mem::take(&mut form.out_url).trim().to_string(),
                        trigger: std::mem::take(&mut form.out_trigger).trim().to_string(),
                        ..Default::default()
                    },
                });
            }
        });
        ui.add_space(10.0);
        ui.label(RichText::new(tr("Slash commands (whole workspace)")).strong());
        ui.label(
            RichText::new(tr("\"/name text\" in any message box sends the text to the address; an answer {\"text\": \"...\"} is posted in the conversation."))
                .small()
                .color(t.text2),
        );
        for c in &commands {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("/{}", c.name)).strong().monospace());
                ui.label(RichText::new(&c.description).small().color(t.text2));
                ui.label(RichText::new(&c.url).small().color(t.text3));
                if ui.small_button(tr("Remove")).clicked() {
                    self.ask(wb_net::Msg::CommandRemove {
                        name: c.name.clone(),
                    });
                }
            });
        }
        ui.horizontal(|ui| {
            let n = ui.add(
                field(&mut form.cmd_name)
                    .hint_text(COMMAND_EXAMPLE)
                    .desired_width(90.0),
            );
            name_it(&n, "Command name");
            let u = ui.add(
                field(&mut form.cmd_url)
                    .hint_text(URL_EXAMPLE)
                    .desired_width(200.0),
            );
            name_it(&u, "Command address");
            let d = ui.add(
                field(&mut form.cmd_desc)
                    .hint_text(tr("What it does"))
                    .desired_width(120.0),
            );
            name_it(&d, "Command description");
            let b = ui.button(tr("Add"));
            name_it(&b, "Add command");
            if b.clicked() {
                self.ask(wb_net::Msg::CommandAdd {
                    command: CommandInfo {
                        name: std::mem::take(&mut form.cmd_name),
                        url: std::mem::take(&mut form.cmd_url).trim().to_string(),
                        description: std::mem::take(&mut form.cmd_desc),
                    },
                });
            }
        });
        false
    }
}

/// What is typed in the Integrations window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IntegrationForm {
    pub in_name: String,
    pub out_name: String,
    pub out_url: String,
    pub out_trigger: String,
    pub cmd_name: String,
    pub cmd_url: String,
    pub cmd_desc: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remind_reads_what_and_when() {
        assert_eq!(
            remind_parts("me to call Sam in 30 minutes"),
            Some(("to call Sam".into(), 1800))
        );
        assert_eq!(
            remind_parts("stand up in 2 h"),
            Some(("stand up".into(), 7200))
        );
        assert_eq!(remind_parts("in 2 hours"), None);
        assert_eq!(remind_parts("call Sam tomorrow"), None);
    }
}
