// What the team hears when someone invited joins: a line in each default
// channel, and a hello in a direct message with each teammate.

use super::*;
use serde_json::Map;
use wb_sync::json::{array_at, id_of, meta_of};

/// What `join_news` posts in the default channels. Kept in English (like a
/// call's "Started a call"): the app shows it in the reader's language.
pub const JOINED_TEXT: &str = "Joined the workspace.";
/// What `join_news` posts in the direct messages.
pub const HELLO_TEXT: &str = "Hi! I just joined the workspace.";
/// The `event` of the messages `join_news` posts: the app translates their
/// text only then, never what someone typed.
pub const JOIN_EVENT: &str = "joined";

/// A channel someone joining is announced in: a public channel marked
/// `default_channel`, or #general until it is turned off (the app's
/// `Channel::is_default`).
pub fn is_default_channel(c: &Value) -> bool {
    let text = |k: &str| c.get(k).and_then(|v| v.as_str());
    text("kind") == Some("Channel")
        && c.get("private").and_then(|p| p.as_bool()) != Some(true)
        && match c.get("default_channel").and_then(|d| d.as_bool()) {
            Some(d) => d,
            None => text("name") == Some("general"),
        }
}

/// What `member` says on joining, as a copy from their app with only what
/// changed (`schedule::with_message`'s shape): `JOINED_TEXT` in every
/// default channel, and `HELLO_TEXT` in a direct message with each teammate
/// who has joined, made when the two have none. `now` in milliseconds;
/// `new_id` gives the ids of what is made. None when there's no one to tell.
pub fn join_news(
    canon: &Value,
    member: u64,
    now: i64,
    mut new_id: impl FnMut() -> u64,
) -> Option<Value> {
    let team = meta_of(canon);
    let mut meta = wb_sync::SyncMeta {
        v: team.v,
        ..Default::default()
    };
    // something new: its id, stamped as made now
    let mut made = |meta: &mut wb_sync::SyncMeta| -> u64 {
        let id = new_id();
        meta.stamps.insert(id, now);
        id
    };
    let say = |id: u64, text: &str| -> Value {
        json!({
            "id": id, "author": member, "text": text, "ts": now / 1000,
            "reactions": [], "replies": [], "saved": false, "task": null,
            "post_title": null, "post_type": "", "pinned": false, "call": null,
            "event": JOIN_EVENT,
        })
    };
    // a conversation that's there already: its own fields and bookkeeping
    // as the team has them, with only the new message inside
    let into = |c: &Value, msg: Value, meta: &mut wb_sync::SyncMeta| -> Option<Value> {
        let id = id_of(c)?;
        let mut o: Map<String, Value> = c
            .as_object()?
            .iter()
            .filter(|(k, _)| *k != "messages")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        o.insert("messages".into(), json!([msg]));
        if let Some(t) = team.stamps.get(&id) {
            meta.stamps.insert(id, *t);
        }
        if let Some(t) = team.moved.get(&id) {
            meta.moved.insert(id, *t);
        }
        if let Some(f) = team.fields.get(&id) {
            meta.fields.insert(id, f.clone());
        }
        Some(Value::Object(o))
    };
    let channels = array_at(canon, "channels");
    let mut parts = vec![];
    for c in channels.iter().filter(|c| is_default_channel(c)) {
        let msg = say(made(&mut meta), JOINED_TEXT);
        parts.extend(into(c, msg, &mut meta));
    }
    let members_of = |c: &Value| -> Vec<u64> {
        let mut v: Vec<u64> = array_at(c, "members")
            .iter()
            .filter_map(|m| m.as_u64())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let mut told = vec![member];
    for m in array_at(canon, "members") {
        let Some(other) = m.get("id").and_then(|i| i.as_u64()) else {
            continue;
        };
        // someone still invited hasn't joined to be told
        if other == 0 || told.contains(&other) || m.get("invited") == Some(&json!(true)) {
            continue;
        }
        told.push(other);
        let mut pair = vec![member, other];
        pair.sort_unstable();
        let dm = channels.iter().find(|c| {
            c.get("kind").and_then(|k| k.as_str()) == Some("Direct") && members_of(c) == pair
        });
        let msg = say(made(&mut meta), HELLO_TEXT);
        match dm {
            Some(c) => parts.extend(into(c, msg, &mut meta)),
            None => {
                let id = made(&mut meta);
                parts.push(json!({
                    "id": id, "name": "", "kind": "Direct", "topic": "",
                    "members": [member, other], "messages": [msg],
                }));
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    let m = serde_json::to_value(&meta).ok()?;
    let part = |k: &str| m.get(k).cloned().unwrap_or_else(|| json!({}));
    Some(json!({
        "channels": parts,
        "extra": {"sync": {
            "v": part("v"),
            "stamps": part("stamps"),
            "moved": part("moved"),
            "fields": part("fields"),
        }}
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team() -> Value {
        json!({
            "members": [
                {"id": 1, "name": "Ann"}, {"id": 2, "name": "Bo", "invited": true},
                {"id": 3, "name": "Cy"}, {"id": 4, "name": "Dee", "invited": true},
            ],
            "channels": [
                {"id": 10, "name": "general", "kind": "Channel", "topic": "", "members": [1, 3],
                 "messages": [{"id": 11, "author": 1, "text": "hi", "ts": 1}]},
                {"id": 20, "name": "random", "kind": "Channel", "topic": "", "members": [], "messages": []},
                {"id": 30, "name": "news", "kind": "Channel", "topic": "", "members": [], "messages": [], "default_channel": true},
                {"id": 40, "name": "secret", "kind": "Channel", "topic": "", "members": [1], "messages": [], "private": true, "default_channel": true},
                {"id": 50, "name": "", "kind": "Direct", "topic": "", "members": [3, 2], "messages": []},
                {"id": 60, "name": "", "kind": "Direct", "topic": "", "members": [1, 2, 3], "messages": []},
            ],
            "extra": {"sync": {"v": 5, "stamps": {"10": 7, "50": 8}, "fields": {"10": {"": 6, "topic": 7}}}}
        })
    }

    fn news(canon: &Value) -> Value {
        let mut next = 1000;
        join_news(canon, 2, 5_000_000, || {
            next += 1;
            next
        })
        .expect("someone to tell")
    }

    fn part(news: &Value, id: u64) -> Option<&Value> {
        array_at(news, "channels").iter().find(|c| c["id"] == id)
    }

    #[test]
    fn says_it_in_the_default_channels_only() {
        let n = news(&team());
        let general = part(&n, 10).expect("#general is a default channel");
        assert_eq!(general["name"], "general");
        assert_eq!(
            general["members"],
            json!([1, 3]),
            "its own fields as they are"
        );
        let msgs = general["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1, "only the new message, not the old ones");
        assert_eq!(msgs[0]["author"], 2);
        assert_eq!(msgs[0]["text"], JOINED_TEXT);
        assert_eq!(msgs[0]["event"], JOIN_EVENT);
        assert_eq!(msgs[0]["ts"], 5000);
        assert_eq!(part(&n, 30).unwrap()["messages"][0]["text"], JOINED_TEXT);
        assert!(part(&n, 20).is_none(), "a channel not marked default");
        assert!(part(&n, 40).is_none(), "a private channel, even marked");
        // #general turned off: not there
        let mut t = team();
        t["channels"][0]["default_channel"] = json!(false);
        assert!(part(&news(&t), 10).is_none());
    }

    #[test]
    fn says_hello_to_each_teammate_who_joined_in_a_direct_message() {
        let n = news(&team());
        // Cy and Bo have a DM: it gets the hello
        let dm = part(&n, 50).expect("their DM");
        assert_eq!(dm["messages"][0]["text"], HELLO_TEXT);
        assert_eq!(dm["messages"][0]["author"], 2);
        // a group DM with them both isn't theirs alone
        assert!(part(&n, 60).is_none());
        // Ann and Bo have none: one is made
        let made: Vec<&Value> = array_at(&n, "channels")
            .iter()
            .filter(|c| c["kind"] == "Direct" && c["id"] != 50)
            .collect();
        assert_eq!(made.len(), 1, "Dee is still invited: nobody to tell");
        assert_eq!(made[0]["members"], json!([2, 1]));
        assert_eq!(made[0]["messages"][0]["text"], HELLO_TEXT);
        assert_eq!(made[0]["messages"][0]["event"], JOIN_EVENT);
    }

    #[test]
    fn stamps_what_it_makes_and_keeps_the_team_stamps_of_the_rest() {
        let n = news(&team());
        let m = meta_of(&n);
        assert_eq!(m.v, 5);
        assert_eq!(m.stamps[&10], 7, "#general's own stamp as the team has it");
        assert_eq!(m.fields[&10]["topic"], 7);
        assert_eq!(m.stamps[&50], 8);
        let new: Vec<u64> = array_at(&n, "channels")
            .iter()
            .flat_map(|c| array_at(c, "messages"))
            .filter_map(id_of)
            .collect();
        assert_eq!(new.len(), 4, "#general, #news, DM with Cy, DM with Ann");
        for id in new {
            assert_eq!(m.stamps[&id], 5_000_000, "message {} stamped new", id);
        }
    }

    #[test]
    fn nothing_when_there_is_no_one_to_tell() {
        let alone = json!({
            "members": [{"id": 2, "name": "Bo", "invited": true}],
            "channels": [{"id": 20, "name": "random", "kind": "Channel", "topic": "", "members": [], "messages": []}],
        });
        assert!(join_news(&alone, 2, 1, || 9).is_none());
    }

    #[test]
    fn merging_it_posts_the_news_and_leaves_the_rest_alone() {
        let mut canon = team();
        let n = news(&canon);
        assert!(wb_sync::merge_unhashed(&mut canon, &[n]));
        let general = &array_at(&canon, "channels")[0];
        let texts: Vec<&str> = array_at(general, "messages")
            .iter()
            .filter_map(|m| m["text"].as_str())
            .collect();
        assert_eq!(texts, ["hi", JOINED_TEXT]);
        assert_eq!(general["members"], json!([1, 3]));
        let dms = array_at(&canon, "channels")
            .iter()
            .filter(|c| c["kind"] == "Direct")
            .count();
        assert_eq!(dms, 3, "the two there and the one made with Ann");
    }
}
