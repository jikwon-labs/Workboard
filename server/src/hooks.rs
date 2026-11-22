// Webhooks and slash commands, set up by owners and admins (wb_net::HookInfo,
// wb_net::CommandInfo) and kept in server.json:
//
//   - an incoming webhook posts in its channel what is sent to the server's
//     /hooks/<token> (or /w/<id>/hooks/<token>) address: a JSON body
//     {"text": "...", "username": "..."} or a form with payload=<that JSON>
//     (as Slack's incoming webhooks take it). The message is its setter's,
//     shown under the webhook's name (`bot`).
//   - an outgoing webhook sends each new message of its channel (starting
//     with its trigger word, when it has one) to its URL as JSON; an answer
//     {"text": "..."} is posted back in the channel under the webhook's name.
//   - a slash command: "/name text" in an app's message box comes here
//     (`command`); the text goes to the command's URL as JSON and an answer
//     {"text": "..."} is posted in the conversation under "/name".
//
// Messages a webhook or command posted (`bot`) never go out to outgoing
// webhooks again, so two of them can't answer each other forever. Posting
// goes through the same merge an app's push does (permissions, iPhone
// pushes and everyone's update included). Requests out go through the
// system's curl (`Curl`), or what the tests put in its place.

use super::*;
use serde::{Deserialize, Serialize};
use wb_net::{CommandInfo, HookInfo};

/// The longest text a webhook or command may post (characters).
const MAX_TEXT: usize = 40_000;
/// At most this many webhooks and commands in one workspace.
const MAX_HOOKS: usize = 100;

/// Sends a JSON body to a URL and gives back what it answered.
pub trait Poster: Send + Sync + std::fmt::Debug {
    fn post(&self, url: &str, body: &Value) -> Result<String, String>;
}

/// `curl`, at most 10 seconds and 1 MB of answer.
#[derive(Debug)]
pub struct Curl;

impl Poster for Curl {
    fn post(&self, url: &str, body: &Value) -> Result<String, String> {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err("not a web address".into());
        }
        let args = [
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "--max-filesize",
            "1000000",
            "--proto",
            "=http,https",
            "-H",
            "Content-Type: application/json",
            "--data-binary",
            "@-",
            "--",
            url,
        ];
        let out = crate::curl::run("curl", &args, body.to_string().as_bytes())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

/// A slash command as kept: with who added it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Slash {
    #[serde(flatten)]
    pub info: CommandInfo,
    pub creator: u64,
}

/// The text an answer asks to post: {"text": "..."} (else nothing).
pub fn answer_text(body: &str) -> Option<(String, Option<String>)> {
    let v: Value = serde_json::from_str(body.trim()).ok()?;
    let t = v.get("text")?.as_str()?.trim().to_string();
    let who = v
        .get("username")
        .and_then(|u| u.as_str())
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    (!t.is_empty()).then_some((t, who))
}

/// What an incoming webhook was sent: JSON, or a form's payload= field.
pub fn incoming_text(body: &[u8]) -> Option<(String, Option<String>)> {
    let s = String::from_utf8_lossy(body);
    if let Some(a) = answer_text(&s) {
        return Some(a);
    }
    // payload=<url-encoded JSON>
    let p = s.split('&').find_map(|kv| kv.strip_prefix("payload="))?;
    answer_text(&wb_net::pct_decode(p))
}

fn token() -> String {
    random_bytes::<18>()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// A command's name as kept: lowercase letters, digits, - and _, without the slash.
pub fn command_name(name: &str) -> Option<String> {
    let n = name.trim().trim_start_matches('/').to_lowercase();
    (!n.is_empty()
        && n.len() <= 32
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
    .then_some(n)
}

impl Hub {
    /// The webhooks (for owners and admins) and slash commands `member` gets.
    pub(crate) fn integrations(&self, member: u64) -> (Vec<HookInfo>, Vec<CommandInfo>) {
        let g = self.state();
        let manages = g.saved.role_of(member).manages();
        let hooks = if manages {
            g.saved.hooks.clone()
        } else {
            vec![]
        };
        let commands = g
            .saved
            .commands
            .iter()
            .map(|c| CommandInfo {
                url: if manages {
                    c.info.url.clone()
                } else {
                    String::new()
                },
                ..c.info.clone()
            })
            .collect();
        (hooks, commands)
    }

    fn manager(&self, member: u64) -> Result<(), String> {
        if self.state().saved.role_of(member).manages() {
            Ok(())
        } else {
            Err("Only an owner or admin can set up webhooks and commands.".into())
        }
    }

    pub(crate) fn add_hook(&self, member: u64, mut hook: HookInfo) -> Result<(), String> {
        self.manager(member)?;
        if hook.kind != "in" && hook.kind != "out" {
            return Err("A webhook is incoming or outgoing.".into());
        }
        if hook.kind == "out"
            && !(hook.url.starts_with("https://") || hook.url.starts_with("http://"))
        {
            return Err("An outgoing webhook needs a web address (https://...).".into());
        }
        let mut g = self.state();
        let exists = g.canon.as_deref().is_some_and(|c| {
            wb_sync::json::array_at(c, "channels")
                .iter()
                .any(|x| wb_sync::json::u64_field(x, "id") == Some(hook.channel))
        });
        if !exists {
            return Err("That channel isn't there.".into());
        }
        if g.saved.hooks.len() >= MAX_HOOKS {
            return Err("There are too many webhooks.".into());
        }
        hook.token = token();
        hook.creator = member;
        hook.name = hook.name.trim().chars().take(60).collect();
        if hook.name.is_empty() {
            hook.name = "Webhook".into();
        }
        g.saved.hooks.push(hook);
        g.dirty = true;
        Ok(())
    }

    pub(crate) fn remove_hook(&self, member: u64, token: &str) -> Result<(), String> {
        self.manager(member)?;
        let mut g = self.state();
        g.saved.hooks.retain(|h| h.token != token);
        g.dirty = true;
        Ok(())
    }

    pub(crate) fn add_command(&self, member: u64, c: CommandInfo) -> Result<(), String> {
        self.manager(member)?;
        let name = command_name(&c.name)
            .ok_or("A command's name is letters, digits, - and _ (\"deploy\").")?;
        if !(c.url.starts_with("https://") || c.url.starts_with("http://")) {
            return Err("A command needs a web address (https://...).".into());
        }
        let mut g = self.state();
        if g.saved.commands.len() >= MAX_HOOKS {
            return Err("There are too many commands.".into());
        }
        g.saved.commands.retain(|x| x.info.name != name);
        g.saved.commands.push(Slash {
            info: CommandInfo {
                name,
                description: c.description.trim().chars().take(120).collect(),
                url: c.url.trim().to_string(),
            },
            creator: member,
        });
        g.dirty = true;
        Ok(())
    }

    pub(crate) fn remove_command(&self, member: u64, name: &str) -> Result<(), String> {
        self.manager(member)?;
        let mut g = self.state();
        g.saved.commands.retain(|x| x.info.name != name);
        g.dirty = true;
        Ok(())
    }

    /// Post `text` in `channel` (thread `parent`) as `member`'s, under
    /// `bot`'s name. False when it couldn't be.
    pub(crate) fn post_as_bot(
        self: &Arc<Self>,
        member: u64,
        channel: u64,
        parent: Option<u64>,
        text: &str,
        bot: &str,
    ) -> bool {
        let text: String = text.trim().chars().take(MAX_TEXT).collect();
        if text.is_empty() {
            return false;
        }
        let id = wb_sync::random_u64() >> 12;
        let now = now_ms() / 1000;
        let w = schedule::Waiting {
            member,
            item: wb_net::ScheduledMsg {
                id,
                channel,
                parent,
                text,
                at: now,
                mentions: vec![],
            },
        };
        let copy = {
            let g = self.state();
            g.canon.as_deref().and_then(|c| {
                let mut c = schedule::with_message(c, &w, now)?;
                set_bot(&mut c, channel, parent, id, bot);
                Some(c)
            })
        };
        match copy {
            Some(c) => {
                self.push(member, "hook", c);
                true
            }
            None => false,
        }
    }

    /// POST /hooks/<token>: an incoming webhook's message.
    pub(crate) fn hook_in(self: &Arc<Self>, token: &str, body: &[u8]) -> Result<(), (u16, String)> {
        let hook = {
            let g = self.state();
            g.saved
                .hooks
                .iter()
                .find(|h| h.kind == "in" && same(&h.token, token))
                .cloned()
        };
        let Some(h) = hook else {
            return Err((404, "no such webhook".into()));
        };
        let Some((text, who)) = incoming_text(body) else {
            return Err((400, "send JSON like {\"text\": \"Hello\"}".into()));
        };
        let name = who.unwrap_or(h.name.clone());
        if self.post_as_bot(h.creator, h.channel, None, &text, &name) {
            Ok(())
        } else {
            Err((410, "its channel is gone".into()))
        }
    }

    /// After a merge: send the new messages of channels with outgoing
    /// webhooks to them (on the job threads, in order for each webhook).
    pub(crate) fn hooks_out(self: &Arc<Self>, new: Vec<(u64, Value)>) {
        let (hooks, canon) = {
            let g = self.state();
            if !g.saved.hooks.iter().any(|h| h.kind == "out") {
                return;
            }
            (g.saved.hooks.clone(), g.canon.clone())
        };
        let Some(canon) = canon else { return };
        for (ch, m) in new {
            // what a webhook or command posted doesn't go out again
            if m.get("bot").is_some_and(|b| !b.is_null()) {
                continue;
            }
            let text = wb_sync::json::text_field(&m, "text");
            for h in hooks.iter().filter(|h| h.kind == "out" && h.channel == ch) {
                if !h.trigger.is_empty() && !text.trim_start().starts_with(&h.trigger) {
                    continue;
                }
                let author = wb_sync::json::u64_field(&m, "author").unwrap_or(0);
                let body = json!({
                    "token": h.token,
                    "channel_id": ch,
                    "channel_name": channel_name(&canon, ch),
                    "user_id": author,
                    "user_name": crate::canon::shown_name(Some(&canon), author),
                    "text": text,
                    "trigger_word": h.trigger,
                    "message_id": m.get("id"),
                    "timestamp": m.get("ts"),
                });
                let hub = self.clone();
                let h = h.clone();
                let poster = self.poster.clone();
                let key = format!("hook:{}", h.token);
                jobs::run(&key, move || match poster.post(&h.url, &body) {
                    Ok(answer) => {
                        if let Some((t, who)) = answer_text(&answer) {
                            hub.post_as_bot(
                                h.creator,
                                h.channel,
                                None,
                                &t,
                                &who.unwrap_or(h.name.clone()),
                            );
                        }
                    }
                    Err(why) => eprintln!("outgoing webhook {} to {}: {}", h.name, h.url, why),
                });
            }
        }
    }

    /// "/name text" from `member` in `channel` (thread `parent`).
    pub(crate) fn run_command(
        self: &Arc<Self>,
        member: u64,
        channel: u64,
        parent: Option<u64>,
        name: &str,
        text: &str,
    ) -> Result<(), String> {
        let (cmd, canon) = {
            let g = self.state();
            let canon = g.canon.clone().ok_or("The workspace isn't set up yet.")?;
            let cmd = g
                .saved
                .commands
                .iter()
                .find(|c| Some(c.info.name.clone()) == command_name(name))
                .cloned()
                .ok_or_else(|| format!("There's no /{} command.", name.trim_start_matches('/')))?;
            (cmd, canon)
        };
        let open = wb_sync::json::array_at(&canon, "channels")
            .iter()
            .find(|c| wb_sync::json::u64_field(c, "id") == Some(channel))
            .is_some_and(|c| !wb_sync::closed_to(c, member));
        if !open {
            return Err("That conversation isn't there for you.".into());
        }
        let body = json!({
            "command": format!("/{}", cmd.info.name),
            "text": text,
            "user_id": member,
            "user_name": crate::canon::shown_name(Some(&canon), member),
            "channel_id": channel,
            "channel_name": channel_name(&canon, channel),
        });
        let hub = self.clone();
        let poster = self.poster.clone();
        let key = format!("command:{}", cmd.info.name);
        jobs::run(&key, move || match poster.post(&cmd.info.url, &body) {
            Ok(answer) => {
                if let Some((t, who)) = answer_text(&answer) {
                    let name = who.unwrap_or(format!("/{}", cmd.info.name));
                    hub.post_as_bot(member, channel, parent, &t, &name);
                }
            }
            Err(why) => eprintln!("command /{} at {}: {}", cmd.info.name, cmd.info.url, why),
        });
        Ok(())
    }
}

/// Mark message `id` (in `channel`, thread `parent`) as posted by `bot`.
fn set_bot(c: &mut Value, channel: u64, parent: Option<u64>, id: u64, bot: &str) {
    let Some(ch) = c
        .get_mut("channels")
        .and_then(|a| a.as_array_mut())
        .and_then(|a| {
            a.iter_mut()
                .find(|x| x.get("id").and_then(|i| i.as_u64()) == Some(channel))
        })
    else {
        return;
    };
    let Some(msgs) = ch.get_mut("messages").and_then(|m| m.as_array_mut()) else {
        return;
    };
    let list = match parent {
        None => Some(msgs),
        Some(p) => msgs
            .iter_mut()
            .find(|x| x.get("id").and_then(|i| i.as_u64()) == Some(p))
            .and_then(|pm| pm.get_mut("replies"))
            .and_then(|r| r.as_array_mut()),
    };
    if let Some(m) = list.and_then(|l| {
        l.iter_mut()
            .find(|x| x.get("id").and_then(|i| i.as_u64()) == Some(id))
    }) {
        m["bot"] = json!(bot.chars().take(60).collect::<String>());
    }
}

fn channel_name(canon: &Value, ch: u64) -> String {
    wb_sync::json::array_at(canon, "channels")
        .iter()
        .find(|c| wb_sync::json::u64_field(c, "id") == Some(ch))
        .map(|c| wb_sync::json::text_field(c, "name"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_forms_and_names() {
        assert_eq!(
            answer_text(r#"{"text": " hi "}"#),
            Some(("hi".into(), None))
        );
        assert_eq!(
            answer_text(r#"{"text": "hi", "username": "CI"}"#),
            Some(("hi".into(), Some("CI".into())))
        );
        assert_eq!(answer_text(r#"{"text": ""}"#), None);
        assert_eq!(answer_text("ok"), None);
        assert_eq!(
            incoming_text(b"payload=%7B%22text%22%3A%22Build+passed%22%7D"),
            Some(("Build passed".into(), None))
        );
        assert_eq!(command_name("/Deploy"), Some("deploy".into()));
        assert_eq!(command_name("rm -rf"), None);
        assert_eq!(command_name(""), None);
    }
}
