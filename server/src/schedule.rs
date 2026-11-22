// Scheduled messages ("Schedule message" in the apps): the server keeps
// each one in server.json until its time, then posts it as its writer's
// through the same merge an app's push goes through (permissions, pushes
// and everyone's update included), so it goes out even while the writer's
// computer is asleep.

use super::*;
use serde::{Deserialize, Serialize};
use wb_net::ScheduledMsg;

/// One member may keep this many at once.
pub(crate) const MAX_PER_MEMBER: usize = 200;
/// The longest text kept (characters).
const MAX_TEXT: usize = 40_000;
/// How far ahead a message may be scheduled (seconds).
const MAX_AHEAD: i64 = 400 * 86_400;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Waiting {
    pub member: u64,
    pub item: ScheduledMsg,
}

impl Hub {
    /// Keep `item` for `member` (replacing theirs with the same id).
    /// Returns their list, soonest first.
    pub(crate) fn schedule(
        &self,
        member: u64,
        item: ScheduledMsg,
    ) -> Result<Vec<ScheduledMsg>, String> {
        let now = now_ms() / 1000;
        let text = item.text.trim();
        if text.is_empty() || text.chars().count() > MAX_TEXT {
            return Err("A scheduled message needs some text (and not too much).".into());
        }
        if item.at < now - 300 || item.at > now + MAX_AHEAD {
            return Err("Pick a time from now to a year ahead.".into());
        }
        let mut g = self.state();
        let Some(canon) = g.canon.as_deref() else {
            return Err("The workspace isn't set up on this server yet.".into());
        };
        let ch = wb_sync::json::array_at(canon, "channels")
            .iter()
            .find(|c| wb_sync::json::u64_field(c, "id") == Some(item.channel));
        match ch {
            Some(c) if !wb_sync::closed_to(c, member) => {}
            _ => return Err("That conversation isn't there for you.".into()),
        }
        if g.saved
            .scheduled
            .iter()
            .any(|w| w.item.id == item.id && w.member != member)
        {
            return Err("That message can't be scheduled.".into());
        }
        let mine = g
            .saved
            .scheduled
            .iter()
            .filter(|w| w.member == member)
            .count();
        let replacing = g
            .saved
            .scheduled
            .iter()
            .any(|w| w.item.id == item.id && w.member == member);
        if mine >= MAX_PER_MEMBER && !replacing {
            return Err(format!(
                "You can schedule at most {} messages.",
                MAX_PER_MEMBER
            ));
        }
        g.saved.scheduled.retain(|w| w.item.id != item.id);
        g.saved.scheduled.push(Waiting { member, item });
        g.dirty = true;
        Ok(list_of(&g.saved.scheduled, member))
    }

    pub(crate) fn unschedule(&self, member: u64, id: u64) -> Vec<ScheduledMsg> {
        let mut g = self.state();
        let before = g.saved.scheduled.len();
        g.saved
            .scheduled
            .retain(|w| !(w.item.id == id && w.member == member));
        if g.saved.scheduled.len() != before {
            g.dirty = true;
        }
        list_of(&g.saved.scheduled, member)
    }

    pub(crate) fn scheduled_of(&self, member: u64) -> Vec<ScheduledMsg> {
        list_of(&self.state().saved.scheduled, member)
    }

    /// Once a second: post the messages whose time has come.
    pub(crate) fn post_due(self: &Arc<Self>) {
        let now = now_ms() / 1000;
        let due: Vec<Waiting> = {
            let mut g = self.state();
            if !g.saved.scheduled.iter().any(|w| w.item.at <= now) {
                return;
            }
            let (due, keep): (Vec<Waiting>, Vec<Waiting>) = std::mem::take(&mut g.saved.scheduled)
                .into_iter()
                .partition(|w| w.item.at <= now);
            g.saved.scheduled = keep;
            g.dirty = true;
            due
        };
        for w in due {
            let copy = {
                let g = self.state();
                g.canon.as_deref().and_then(|c| with_message(c, &w, now))
            };
            match copy {
                Some(c) => {
                    self.push(w.member, "scheduler", c);
                }
                None => eprintln!(
                    "dropped scheduled message {} of member {}: its conversation is gone",
                    w.item.id, w.member
                ),
            }
        }
    }
}

fn list_of(all: &[Waiting], member: u64) -> Vec<ScheduledMsg> {
    let mut v: Vec<ScheduledMsg> = all
        .iter()
        .filter(|w| w.member == member)
        .map(|w| w.item.clone())
        .collect();
    v.sort_by_key(|i| (i.at, i.id));
    v
}

/// `w`'s message posted (at `now`, seconds), as an app sends a new message:
/// only what changed (`wb_sync::delta`'s shape), its conversation (and the
/// message it answers) with their own fields and bookkeeping and only the
/// new message inside, stamped as new. Merging it is the same as merging
/// the whole copy with the message added, without copying the workspace.
/// None when its conversation (or thread) is gone or the message is there
/// already.
pub(crate) fn with_message(canon: &Value, w: &Waiting, now: i64) -> Option<Value> {
    let item = &w.item;
    let ch = wb_sync::json::array_at(canon, "channels")
        .iter()
        .find(|x| x.get("id").and_then(|i| i.as_u64()) == Some(item.channel))?;
    let messages = wb_sync::json::array_at(ch, "messages");
    let has = |list: &[Value]| {
        list.iter().any(|m| {
            wb_sync::json::id_of(m) == Some(item.id)
                || wb_sync::json::array_at(m, "replies")
                    .iter()
                    .any(|r| wb_sync::json::id_of(r) == Some(item.id))
        })
    };
    if has(messages) {
        return None;
    }
    let mut m = json!({
        "id": item.id,
        "author": w.member,
        "text": item.text,
        "ts": now,
        "reactions": [],
        "replies": [],
        "saved": false,
        "task": null,
        "post_title": null,
        "post_type": "",
        "pinned": false,
        "call": null,
    });
    if !item.mentions.is_empty() {
        m["mentions"] = json!(item.mentions);
    }
    // an item with its own fields and, of the items inside it, only `inner`
    // (`wb_sync::delta`'s `prune`)
    let shell = |x: &Value, key: &str, inner: Value| -> Value {
        let mut o = serde_json::Map::new();
        for (k, v) in x.as_object().into_iter().flatten() {
            if is_items(v) {
                let loose: Vec<Value> = v
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| wb_sync::json::id_of(e).is_none())
                    .cloned()
                    .collect();
                if !loose.is_empty() {
                    o.insert(k.clone(), Value::Array(loose));
                }
            } else {
                o.insert(k.clone(), v.clone());
            }
        }
        o.insert(key.into(), json!([inner]));
        Value::Object(o)
    };
    let (part, holders) = match item.parent {
        None => (shell(ch, "messages", m), vec![item.channel]),
        Some(p) => {
            let parent = messages
                .iter()
                .find(|x| x.get("id").and_then(|i| i.as_u64()) == Some(p))?;
            let parent = shell(parent, "replies", m);
            (shell(ch, "messages", parent), vec![item.channel, p])
        }
    };
    // the bookkeeping of the items it sits in, as the team's copy has it
    let sync = canon.get("extra").and_then(|e| e.get("sync"));
    let pick = |key: &str| -> serde_json::Map<String, Value> {
        holders
            .iter()
            .filter_map(|id| {
                let k = id.to_string();
                Some((k.clone(), sync?.get(key)?.get(&k)?.clone()))
            })
            .collect()
    };
    let mut stamps = pick("stamps");
    stamps.insert(item.id.to_string(), json!(now_ms()));
    Some(json!({
        "channels": [part],
        "extra": {"sync": {
            "v": sync.and_then(|s| s.get("v")).cloned().unwrap_or(json!(0)),
            "stamps": stamps,
            "moved": pick("moved"),
            "fields": pick("fields"),
        }}
    }))
}

/// A non-empty array of objects of which at least one has an id: items
/// (`wb_sync`'s `is_id_array`).
fn is_items(v: &Value) -> bool {
    v.as_array().is_some_and(|a| {
        !a.is_empty()
            && a.iter().all(|x| x.is_object())
            && a.iter().any(|x| wb_sync::json::id_of(x).is_some())
    })
}
