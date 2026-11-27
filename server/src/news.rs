// What a quit app is told besides new messages (push.rs tells those):
// reactions to my messages, tasks assigned to me, comments, mentions and
// changes on my tasks, answers to my forms, my goals passing a milestone
// through someone else, and people joining (for owners and admins). The
// same things, with the same words, as the Mac app's own notices while it
// runs (notify_candidates in src/app/notify.rs).
//
// Each thing has a key (`Atom::key`). After a merge (notify.rs `push`) the
// keys of the part of the team's copy the incoming change touched (`Scope`)
// are compared with what that part had before: a key that wasn't there is
// news. Only devices whose app said its language get these
// (`push::device_payload`), so older apps see no difference.

use serde_json::Value;
use std::collections::{HashMap, HashSet};
use wb_net::notice::{self, Lang};
use wb_sync::json::{array_at, text_field, u64_field};

use crate::push::{cut, Kind, Note, Why, Words, MAX_AGE_SECS};

/// The part of the team's copy a change can bring news in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scope {
    pub channels: HashSet<u64>,
    pub tasks: HashSet<u64>,
    pub members: bool,
    pub goals: bool,
}

/// What `copy` (a change an app pushed) can bring news to.
pub fn scope_of(copy: &Value) -> Scope {
    let ids = |path: &str| -> HashSet<u64> {
        wb_sync::json::get_path(copy, path)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|x| u64_field(x, "id"))
            .collect()
    };
    Scope {
        channels: ids("channels"),
        tasks: ids("tasks"),
        members: copy.get("members").is_some_and(Value::is_array),
        goals: wb_sync::json::get_path(copy, "extra.goals").is_some_and(Value::is_array),
    }
}

/// The keys of everything in `scope` that could be news.
pub fn keys(canon: &Value, scope: &Scope) -> HashSet<String> {
    let mut out = HashSet::new();
    walk(canon, scope, &mut |a| {
        out.insert(a.key());
    });
    out
}

/// One thing that may be news, before it is known who is told.
enum Atom<'a> {
    Reaction {
        channel: &'a Value,
        msg: &'a Value,
        thread: Option<u64>,
        emoji: String,
        user: u64,
    },
    Activity {
        task: &'a Value,
        a: &'a Value,
    },
    Member(&'a Value),
    Milestone {
        goal: &'a Value,
        m: u64,
        e: &'a Value,
    },
}

impl Atom<'_> {
    fn key(&self) -> String {
        match self {
            Atom::Reaction {
                channel,
                msg,
                emoji,
                user,
                ..
            } => format!(
                "react:{}:{}:{}:{}",
                u64_field(channel, "id").unwrap_or(0),
                u64_field(msg, "id").unwrap_or(0),
                emoji,
                user
            ),
            Atom::Activity { task, a } => format!(
                "task:{}:{}:{}:{}",
                u64_field(task, "id").unwrap_or(0),
                a.get("ts").and_then(Value::as_i64).unwrap_or(0),
                u64_field(a, "author").unwrap_or(0),
                text_field(a, "text")
            ),
            Atom::Member(m) => format!("member:{}", u64_field(m, "id").unwrap_or(0)),
            Atom::Milestone { goal, m, e } => format!(
                "goalm:{}:{}:{}",
                u64_field(goal, "id").unwrap_or(0),
                m,
                e.get("ts").and_then(Value::as_i64).unwrap_or(0)
            ),
        }
    }
}

fn walk<'a>(canon: &'a Value, scope: &Scope, f: &mut dyn FnMut(Atom<'a>)) {
    for ch in array_at(canon, "channels") {
        if !u64_field(ch, "id").is_some_and(|i| scope.channels.contains(&i)) {
            continue;
        }
        for m in array_at(ch, "messages") {
            let mid = u64_field(m, "id");
            let replies = array_at(m, "replies").iter().map(|r| (r, mid));
            for (msg, thread) in std::iter::once((m, None)).chain(replies) {
                for re in array_at(msg, "reactions") {
                    let emoji = text_field(re, "emoji");
                    for u in array_at(re, "users").iter().filter_map(Value::as_u64) {
                        f(Atom::Reaction {
                            channel: ch,
                            msg,
                            thread,
                            emoji: emoji.clone(),
                            user: u,
                        });
                    }
                }
            }
        }
    }
    for t in array_at(canon, "tasks") {
        if !u64_field(t, "id").is_some_and(|i| scope.tasks.contains(&i)) {
            continue;
        }
        for a in array_at(t, "activity") {
            f(Atom::Activity { task: t, a });
        }
    }
    if scope.members {
        for m in array_at(canon, "members") {
            if m.get("invited").and_then(Value::as_bool) != Some(true) {
                f(Atom::Member(m));
            }
        }
    }
    if scope.goals {
        for g in wb_sync::json::get_path(canon, "extra.goals")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            for e in array_at(g, "log") {
                if let Some(m) = e
                    .get("what")
                    .and_then(|w| w.get("Milestone"))
                    .and_then(Value::as_u64)
                {
                    f(Atom::Milestone { goal: g, m, e });
                }
            }
        }
    }
}

/// People in the team's copy: names as the app shows them, and who an
/// old entry names.
struct People {
    /// id -> (full name, shown name)
    names: HashMap<u64, (String, String)>,
}

impl People {
    fn of(canon: &Value) -> People {
        let names = array_at(canon, "members")
            .iter()
            .filter_map(|m| {
                let full = text_field(m, "name").trim().to_string();
                let d = text_field(m, "display_name").trim().to_string();
                let shown = if d.is_empty() { full.clone() } else { d };
                Some((u64_field(m, "id")?, (full, shown)))
            })
            .collect();
        People { names }
    }

    /// The name the app shows ("Someone" for a person who left).
    fn who(&self, id: u64) -> [String; 2] {
        match self
            .names
            .get(&id)
            .map(|n| n.1.clone())
            .filter(|n| !n.is_empty())
        {
            Some(n) => [n.clone(), n],
            None => [
                notice::tr(Lang::En, "Someone").into(),
                notice::tr(Lang::Ko, "Someone").into(),
            ],
        }
    }

    fn full(&self, id: u64) -> &str {
        self.names.get(&id).map(|n| n.0.as_str()).unwrap_or("")
    }

    /// (id, name) for each full and shown name, for mentions by text.
    fn named(&self) -> Vec<(u64, &str)> {
        self.names
            .iter()
            .flat_map(|(i, (f, d))| [(*i, f.as_str()), (*i, d.as_str())])
            .filter(|(_, n)| !n.is_empty())
            .collect()
    }
}

/// Words with a name in them, made in both languages: `who` holds the
/// person's name as each language writes it.
fn words(title: &str, title_args: [&[&str]; 2], body: &str, body_args: [&[&str]; 2]) -> Words {
    Words {
        title: [
            notice::trf(Lang::En, title, title_args[0]),
            notice::trf(Lang::Ko, title, title_args[1]),
        ],
        body: [
            notice::trf(Lang::En, body, body_args[0]),
            notice::trf(Lang::Ko, body, body_args[1]),
        ],
    }
}

/// Whether `member` gets a task's comments and changes: its assignees and
/// maker unless they turned that off, and the people who chose to watch
/// (`Task::watched_by`).
fn watches(task: &Value, member: u64) -> bool {
    let has = |k: &str| array_at(task, k).iter().any(|x| x.as_u64() == Some(member));
    if has("unwatched") {
        return false;
    }
    let made = array_at(task, "activity").iter().any(|a| {
        u64_field(a, "author") == Some(member)
            && a.get("comment").and_then(Value::as_bool) != Some(true)
            && {
                let t = text_field(a, "text");
                t == "created this task" || t.starts_with("duplicated this task from ")
            }
    });
    has("watchers") || has("assignees") || made
}

/// The form an answer is to, as the app finds it (`old_answer_form`): the
/// one the task says, else the only one of that name (in its List).
fn answer_form<'a>(forms: &'a [Value], task: &Value, name: &str) -> Option<&'a Value> {
    if let Some(id) = u64_field(task, "form") {
        return forms.iter().find(|f| u64_field(f, "id") == Some(id));
    }
    let named: Vec<&Value> = forms
        .iter()
        .filter(|f| text_field(f, "name") == name)
        .collect();
    if named.len() == 1 {
        return Some(named[0]);
    }
    let list = u64_field(task, "list_id");
    let in_list: Vec<&Value> = named
        .into_iter()
        .filter(|f| u64_field(f, "list_id") == list)
        .collect();
    (in_list.len() == 1).then(|| in_list[0])
}

/// The notes for what is in `scope` of `canon` and wasn't in `before`
/// (`keys` of the same part before the merge). `skip`: members who get
/// nothing (the one whose app made the change). `manages`: whether a
/// member is an owner or admin.
pub fn notes(
    canon: &Value,
    before: &HashSet<String>,
    scope: &Scope,
    skip: &[u64],
    manages: &dyn Fn(u64) -> bool,
    now_secs: i64,
) -> Vec<Note> {
    let people = People::of(canon);
    let forms = wb_sync::json::get_path(canon, "extra.forms")
        .and_then(Value::as_array)
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    let workspace = text_field(canon, "workspace");
    let mut out: Vec<Note> = vec![];
    // what each member may not see, looked up once someone is to be told
    let mut hidden: HashMap<u64, HashSet<u64>> = HashMap::new();
    let mut can_see = |member: u64, id: u64| {
        !hidden
            .entry(member)
            .or_insert_with(|| {
                wb_sync::hidden_from(canon, member, manages(member))
                    .into_iter()
                    .collect()
            })
            .contains(&id)
    };
    let fresh = |ts: i64| {
        let ts = if ts > 100_000_000_000 { ts / 1000 } else { ts };
        now_secs - ts <= MAX_AGE_SECS
    };
    let mut push = |member: u64, kind: Kind, channel: u64, target: String, w: Words| {
        if skip.contains(&member) {
            return;
        }
        out.push(Note {
            member,
            channel,
            id: 0,
            payload: Value::Null,
            why: Why::Channel,
            text: String::new(),
            kind,
            chat: None,
            words: Some(w),
            target,
        });
    };
    let mut atoms = vec![];
    walk(canon, scope, &mut |a| atoms.push(a));
    for atom in atoms {
        if before.contains(&atom.key()) {
            continue;
        }
        match atom {
            Atom::Reaction {
                channel,
                msg,
                thread,
                emoji,
                user,
            } => {
                let Some(author) = u64_field(msg, "author") else {
                    continue;
                };
                if author == user || wb_sync::closed_to(channel, author) {
                    continue;
                }
                let cid = u64_field(channel, "id").unwrap_or(0);
                let target = match thread {
                    Some(t) => format!("chat:{}:{}", cid, t),
                    None => format!("chat:{}", cid),
                };
                let who = people.who(user);
                let w = Words {
                    title: [
                        notice::trf(
                            Lang::En,
                            "{} reacted {} to your message",
                            &[&who[0], &emoji],
                        ),
                        notice::trf(
                            Lang::Ko,
                            "{} reacted {} to your message",
                            &[&who[1], &emoji],
                        ),
                    ],
                    body: [
                        crate::push::preview(msg, Lang::En),
                        crate::push::preview(msg, Lang::Ko),
                    ],
                };
                push(author, Kind::Reaction, cid, target, w);
            }
            Atom::Activity { task, a } => {
                let author = u64_field(a, "author").unwrap_or(0);
                let ts = a.get("ts").and_then(Value::as_i64).unwrap_or(0);
                if !fresh(ts) {
                    continue;
                }
                let tid = u64_field(task, "id").unwrap_or(0);
                let tname = text_field(task, "name");
                let text = text_field(a, "text");
                let comment = a.get("comment").and_then(Value::as_bool) == Some(true);
                let who = people.who(author);
                let target = format!("task:{}", tid);
                // an answer to a form: its maker
                if let Some(name) = text.strip_prefix("submitted the form ") {
                    if comment {
                        continue;
                    }
                    // the form the task says it answers (or the only one of
                    // that name); several and the List doesn't tell which:
                    // the makers of each of that name, as the app tells it
                    let owners: Vec<u64> = match answer_form(forms, task, name) {
                        Some(f) => u64_field(f, "owner").into_iter().collect(),
                        None => {
                            let mut o: Vec<u64> = forms
                                .iter()
                                .filter(|f| text_field(f, "name") == name)
                                .filter_map(|f| u64_field(f, "owner"))
                                .collect();
                            o.sort_unstable();
                            o.dedup();
                            o
                        }
                    };
                    let shown = |l: Lang| {
                        if name.trim().is_empty() {
                            notice::tr(l, "Untitled form").to_string()
                        } else {
                            name.to_string()
                        }
                    };
                    let (en, ko) = (shown(Lang::En), shown(Lang::Ko));
                    for owner in owners {
                        if owner == author || !can_see(owner, tid) {
                            continue;
                        }
                        let w = words(
                            "New answer to {}",
                            [&[&en], &[&ko]],
                            "{} answered: {}",
                            [&[&who[0], &tname], &[&who[1], &tname]],
                        );
                        push(owner, Kind::Form, 0, target.clone(), w);
                    }
                    continue;
                }
                let saved: Vec<u64> = array_at(a, "mentions")
                    .iter()
                    .filter_map(Value::as_u64)
                    .collect();
                let mentioned: Vec<u64> = if comment {
                    wb_sync::mentions::mentions_of(&saved, &text, &people.named())
                } else {
                    vec![]
                };
                // everyone it could be for: the people it names, and the
                // task's people
                let mut to: Vec<u64> = people.names.keys().copied().collect();
                to.sort_unstable();
                for m in to {
                    if m == author || !can_see(m, tid) {
                        continue;
                    }
                    let mine = watches(task, m);
                    let full = people.full(m);
                    let assigned = !comment
                        && match u64_field(a, "target") {
                            Some(who) => who == m && text.starts_with("assigned to "),
                            None => !full.is_empty() && text == format!("assigned to {}", full),
                        };
                    let body140 = cut(&text, 140);
                    let (kind, w) = if comment && mentioned.contains(&m) {
                        (
                            Kind::Mention,
                            words(
                                "{} mentioned you in {}",
                                [&[&who[0], &tname], &[&who[1], &tname]],
                                "{}",
                                [&[&body140], &[&body140]],
                            ),
                        )
                    } else if assigned {
                        (
                            Kind::Assigned,
                            words(
                                "{} assigned you a task",
                                [&[&who[0]], &[&who[1]]],
                                "{}",
                                [&[&tname], &[&tname]],
                            ),
                        )
                    } else if comment && mine {
                        (
                            Kind::Comment,
                            words(
                                "{} commented on {}",
                                [&[&who[0], &tname], &[&who[1], &tname]],
                                "{}",
                                [&[&body140], &[&body140]],
                            ),
                        )
                    } else if !comment && mine && {
                        text.starts_with("changed status")
                            || text.starts_with("changed due date")
                            || text.starts_with("set priority")
                            || (!full.is_empty() && text == format!("removed assignee {}", full))
                    } {
                        (
                            Kind::TaskChange,
                            Words {
                                title: [tname.clone(), tname.clone()],
                                body: [
                                    notice::history_line(Lang::En, &who[0], &text),
                                    notice::history_line(Lang::Ko, &who[1], &text),
                                ],
                            },
                        )
                    } else {
                        continue;
                    };
                    push(m, kind, 0, target.clone(), w);
                }
            }
            Atom::Member(m) => {
                let Some(id) = u64_field(m, "id") else {
                    continue;
                };
                for (&to, _) in people.names.iter() {
                    if to == id || !manages(to) {
                        continue;
                    }
                    push(
                        to,
                        Kind::Member,
                        0,
                        "people".into(),
                        joined_words(m, &workspace),
                    );
                }
            }
            Atom::Milestone { goal, m, e } => {
                let Some(owner) = u64_field(goal, "owner") else {
                    continue;
                };
                let ts = e.get("ts").and_then(Value::as_i64).unwrap_or(0);
                // my own progress shows on the goal's page; news, not history
                if u64_field(e, "who") == Some(owner) || !fresh(ts) {
                    continue;
                }
                let name = text_field(goal, "name");
                let pct = u64_field(e, "pct").unwrap_or(m).to_string();
                let w = if m >= 100 {
                    words(
                        "Goal achieved: {}",
                        [&[&name], &[&name]],
                        "All targets met",
                        [&[], &[]],
                    )
                } else {
                    let ms = m.to_string();
                    words(
                        "{} reached {}%",
                        [&[&name, &ms], &[&name, &ms]],
                        "{}% done",
                        [&[&pct], &[&pct]],
                    )
                };
                let gid = u64_field(goal, "id").unwrap_or(0);
                push(owner, Kind::Goal, 0, format!("goal:{}", gid), w);
            }
        }
    }
    out
}

/// "Kim joined Acme": what owners and admins are told when someone joins.
pub fn joined_words(m: &Value, workspace: &str) -> Words {
    let d = text_field(m, "display_name").trim().to_string();
    let name = if d.is_empty() {
        text_field(m, "name").trim().to_string()
    } else {
        d
    };
    words(
        "{} joined {}",
        [&[&name, workspace], &[&name, workspace]],
        "Say hello in a direct message",
        [&[], &[]],
    )
}

/// The notes for `member` having joined: their owners and admins are told.
pub fn joined_notes(canon: &Value, member: u64, manages: &dyn Fn(u64) -> bool) -> Vec<Note> {
    let Some(m) = array_at(canon, "members")
        .iter()
        .find(|m| u64_field(m, "id") == Some(member))
    else {
        return vec![];
    };
    let w = joined_words(m, &text_field(canon, "workspace"));
    array_at(canon, "members")
        .iter()
        .filter_map(|x| u64_field(x, "id"))
        .filter(|to| *to != member && manages(*to))
        .map(|to| Note {
            member: to,
            channel: 0,
            id: 0,
            payload: Value::Null,
            why: Why::Channel,
            text: String::new(),
            kind: Kind::Member,
            chat: None,
            words: Some(w.clone()),
            target: "people".into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::push::{batch_words, device_payload, notes_in};
    use serde_json::json;

    const NOW: i64 = 1_800_000_000;

    /// Alex (1, owner), Bo (2), Cy (3); a public channel 10 and a DM 20
    /// between Alex and Bo; task 100 assigned to Alex, made by Alex.
    fn team() -> Value {
        json!({
            "workspace": "Acme",
            "members": [
                {"id": 1, "name": "Alex Kim", "display_name": "Alex"},
                {"id": 2, "name": "Bo Lee"},
                {"id": 3, "name": "Cy Park"},
            ],
            "channels": [
                {"id": 10, "name": "general", "kind": "Channel", "members": [], "messages": [
                    {"id": 11, "author": 1, "text": "Shipped", "ts": NOW - 50, "reactions": []}
                ]},
                {"id": 20, "name": "", "kind": "Direct", "members": [1, 2], "messages": []}
            ],
            "spaces": [{"id": 50, "name": "Team", "lists": [{"id": 60, "name": "Work"}]}],
            "tasks": [
                {"id": 100, "list_id": 60, "name": "Launch", "assignees": [1], "watchers": [], "activity": [
                    {"ts": NOW - 3600, "author": 1, "text": "created this task", "comment": false}
                ]}
            ],
            "extra": {
                "forms": [{"id": 70, "name": "Signup", "owner": 1, "list_id": 60}],
                "goals": [{"id": 80, "name": "Q4", "owner": 1, "log": []}]
            }
        })
    }

    fn everything(lang: &str) -> wb_net::PushPrefs {
        wb_net::PushPrefs {
            huddles: Some(true),
            reactions: Some(true),
            assigned: Some(true),
            comments: Some(true),
            task_changes: Some(true),
            forms: Some(true),
            team: Some(true),
            goal_milestones: Some(true),
            lang: Some(lang.into()),
            ..Default::default()
        }
    }

    /// The (title, body, target) each note for `member` makes on a device
    /// asking for everything in `lang`.
    fn told(notes: &[Note], member: u64, lang: &str) -> Vec<(String, String, String)> {
        let p = everything(lang);
        notes
            .iter()
            .filter(|n| n.member == member)
            .filter_map(|n| device_payload(n, &p, NOW))
            .map(|(_, v)| {
                (
                    v["aps"]["alert"]["title"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    v["aps"]["alert"]["body"].as_str().unwrap_or("").to_string(),
                    v["target"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect()
    }

    fn owner_manages(m: u64) -> bool {
        m == 1
    }

    /// The notes `change` (applied to `team()`) makes, `by` being whose app sent it.
    fn after(change: impl FnOnce(&mut Value), scope: Scope, by: u64) -> Vec<Note> {
        let mut c = team();
        let before = keys(&c, &scope);
        change(&mut c);
        notes(&c, &before, &scope, &[by], &owner_manages, NOW)
    }

    fn task_scope() -> Scope {
        Scope {
            tasks: [100].into(),
            ..Default::default()
        }
    }

    fn add_activity(c: &mut Value, a: Value) {
        c["tasks"][0]["activity"].as_array_mut().unwrap().push(a);
    }

    #[test]
    fn task_news_reaches_who_the_app_would_tell_in_its_words() {
        // Bo assigns Alex, comments, mentions Cy, changes the status
        let notes = after(
            |c| {
                add_activity(
                    c,
                    json!({"ts": NOW - 10, "author": 2, "text": "assigned to Alex Kim", "comment": false, "target": 1}),
                );
                add_activity(
                    c,
                    json!({"ts": NOW - 9, "author": 2, "text": "Looks good", "comment": true}),
                );
                add_activity(
                    c,
                    json!({"ts": NOW - 8, "author": 2, "text": "@Cy Park can you check?", "comment": true, "mentions": [3]}),
                );
                add_activity(
                    c,
                    json!({"ts": NOW - 7, "author": 2, "text": "changed status from to do to complete", "comment": false}),
                );
            },
            task_scope(),
            2,
        );
        assert_eq!(
            told(&notes, 1, "ko"),
            vec![
                (
                    "Bo Lee님이 작업을 맡겼습니다".into(),
                    "Launch".into(),
                    "task:100".into()
                ),
                (
                    "Bo Lee님이 Launch에 댓글을 남겼습니다".into(),
                    "Looks good".into(),
                    "task:100".into()
                ),
                (
                    "Bo Lee님이 Launch에 댓글을 남겼습니다".into(),
                    "@Cy Park can you check?".into(),
                    "task:100".into()
                ),
                (
                    "Launch".into(),
                    "Bo Lee님이 상태를 할 일에서 완료로 변경했습니다".into(),
                    "task:100".into()
                ),
            ]
        );
        // Cy doesn't watch the task: only the mention
        assert_eq!(
            told(&notes, 3, "en"),
            vec![(
                "Bo Lee mentioned you in Launch".into(),
                "@Cy Park can you check?".into(),
                "task:100".into()
            )]
        );
        // Bo made the changes: nothing
        assert!(notes.iter().all(|n| n.member != 2));
        // the payload carries the task for the iPhone
        let p = device_payload(&notes[0], &everything("ko"), NOW).unwrap().1;
        assert_eq!(p["task"], json!(100));
        assert_eq!(p["kind"], json!("assigned"));
    }

    #[test]
    fn each_kind_follows_the_devices_choice_and_old_apps_get_none() {
        let notes = after(
            |c| {
                add_activity(
                    c,
                    json!({"ts": NOW - 9, "author": 2, "text": "Looks good", "comment": true}),
                )
            },
            task_scope(),
            2,
        );
        let n = notes.iter().find(|n| n.member == 1).unwrap();
        let mut p = everything("en");
        assert!(device_payload(n, &p, NOW).is_some());
        p.comments = Some(false);
        assert!(device_payload(n, &p, NOW).is_none());
        // paused
        let mut q = everything("en");
        q.muted_until = NOW + 60;
        assert!(device_payload(n, &q, NOW).is_none());
        // an app that never said its language: as before, messages only
        assert!(!crate::push::wanted(n, None, NOW));
        assert!(!crate::push::wanted(
            n,
            Some(&wb_net::PushPrefs::default()),
            NOW
        ));
    }

    #[test]
    fn old_hidden_and_already_known_things_are_not_news() {
        // a day and more ago
        let old = after(
            |c| {
                add_activity(
                    c,
                    json!({"ts": NOW - 2 * 86_400, "author": 2, "text": "Late", "comment": true}),
                )
            },
            task_scope(),
            2,
        );
        assert!(old.is_empty());
        // what was there before the change
        let mut c = team();
        add_activity(
            &mut c,
            json!({"ts": NOW - 9, "author": 2, "text": "Looks good", "comment": true}),
        );
        let before = keys(&c, &task_scope());
        assert!(notes(&c, &before, &task_scope(), &[2], &owner_manages, NOW).is_empty());
        // a task in a space shared with Bo alone isn't Cy's to hear of
        let hidden = after(
            |c| {
                c["spaces"][0]["sharing"] =
                    json!({"everyone": false, "people": [{"member": 2, "level": "full"}]});
                add_activity(
                    c,
                    json!({"ts": NOW - 8, "author": 2, "text": "@Cy Park look", "comment": true, "mentions": [3]}),
                );
            },
            task_scope(),
            2,
        );
        assert!(hidden.iter().all(|n| n.member != 3), "{:?}", hidden);
        assert!(
            hidden.iter().any(|n| n.member == 1),
            "an owner sees every space"
        );
    }

    #[test]
    fn taken_off_a_task_and_priority_changes_are_told() {
        let notes = after(
            |c| {
                add_activity(
                    c,
                    json!({"ts": NOW - 9, "author": 2, "text": "set priority to Urgent", "comment": false}),
                );
                add_activity(
                    c,
                    json!({"ts": NOW - 8, "author": 2, "text": "removed assignee Alex Kim", "comment": false}),
                );
                // not a change the app tells
                add_activity(
                    c,
                    json!({"ts": NOW - 7, "author": 2, "text": "added tag ui", "comment": false}),
                );
            },
            task_scope(),
            2,
        );
        let bodies: Vec<String> = told(&notes, 1, "ko").into_iter().map(|t| t.1).collect();
        assert_eq!(
            bodies,
            vec![
                "Bo Lee님이 우선순위를 긴급으로 설정했습니다",
                "Bo Lee님이 담당자 Alex Kim을 제거했습니다"
            ]
        );
    }

    #[test]
    fn form_answers_reactions_milestones_and_joins() {
        // an answer to Alex's form, sent by Cy's app
        let form = after(
            |c| {
                c["tasks"].as_array_mut().unwrap().push(json!({
                    "id": 101, "list_id": 60, "name": "Dana", "form": 70, "assignees": [], "activity": [
                        {"ts": NOW - 5, "author": 3, "text": "submitted the form Signup", "comment": false}
                    ]
                }))
            },
            Scope {
                tasks: [101].into(),
                ..Default::default()
            },
            3,
        );
        assert_eq!(
            told(&form, 1, "ko"),
            vec![(
                "Signup 새 응답".into(),
                "Cy Park님의 응답: Dana".into(),
                "task:101".into()
            )]
        );
        // Bo reacts to Alex's message
        let react = after(
            |c| {
                c["channels"][0]["messages"][0]["reactions"] =
                    json!([{"emoji": "🎉", "users": [2]}]);
            },
            Scope {
                channels: [10].into(),
                ..Default::default()
            },
            2,
        );
        assert_eq!(
            told(&react, 1, "ko"),
            vec![(
                "Bo Lee님이 내 메시지에 🎉 반응을 남겼습니다".into(),
                "Shipped".into(),
                "chat:10".into()
            )]
        );
        // Bo's work takes Alex's goal past 50%
        let goal = after(
            |c| {
                c["extra"]["goals"][0]["log"] =
                    json!([{"ts": NOW - 5, "who": 2, "what": {"Milestone": 50}, "pct": 52}]);
            },
            Scope {
                goals: true,
                ..Default::default()
            },
            2,
        );
        assert_eq!(
            told(&goal, 1, "ko"),
            vec![("Q4 50% 달성".into(), "52% 완료".into(), "goal:80".into())]
        );
        // a milestone Alex reached himself shows on the goal's page instead
        let own = after(
            |c| {
                c["extra"]["goals"][0]["log"] =
                    json!([{"ts": NOW - 5, "who": 1, "what": {"Milestone": 100}, "pct": 100}]);
            },
            Scope {
                goals: true,
                ..Default::default()
            },
            1,
        );
        assert!(own.is_empty());
        // Dee joins: the owner hears, the others don't
        let mut c = team();
        c["members"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 4, "name": "Dee"}));
        let joined = joined_notes(&c, 4, &owner_manages);
        assert_eq!(
            told(&joined, 1, "ko"),
            vec![(
                "Dee님이 Acme에 들어왔습니다".into(),
                "DM으로 인사해 보세요".into(),
                "people".into()
            )]
        );
        assert!(joined.iter().all(|n| n.member == 1));
    }

    #[test]
    fn messages_are_written_by_each_devices_choices_in_its_language() {
        let mut c = team();
        let before = crate::push::message_ids(&c);
        {
            let ch = c["channels"][0]["messages"].as_array_mut().unwrap();
            ch.push(json!({"id": 12, "author": 2, "text": "Deploy at 5", "ts": NOW - 5}));
            ch.push(json!({"id": 13, "author": 2, "text": "@Alex Kim ready?", "ts": NOW - 4, "mentions": [1]}));
            ch.push(json!({"id": 14, "author": 2, "text": "Started a huddle", "ts": NOW - 3, "call": "room"}));
            c["channels"][0]["messages"][0]["replies"] =
                json!([{"id": 15, "author": 2, "text": "**nice**", "ts": NOW - 2}]);
            c["channels"][1]["messages"] = json!([{"id": 21, "author": 2, "text": "", "ts": NOW - 1,
                "attachments": [{"kind": "Image"}]}]);
        }
        let notes = notes_in(&c, &before, None, &[2], NOW);
        let mut p = everything("ko");
        p.keywords = vec!["deploy".into()];
        let words: Vec<(String, String)> = notes
            .iter()
            .filter(|n| n.member == 1)
            .filter_map(|n| device_payload(n, &p, NOW))
            .map(|(_, v)| {
                (
                    v["aps"]["alert"]["title"].as_str().unwrap().to_string(),
                    v["aps"]["alert"]["body"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        assert_eq!(
            words,
            vec![
                // the reply is under the first message
                (
                    "Bo Lee님이 #general에서 답글을 남겼습니다".into(),
                    "**nice**".into()
                ),
                (
                    "Bo Lee님의 메시지에 \"deploy\" 가 있습니다 (# general)".into(),
                    "Deploy at 5".into()
                ),
                (
                    "Bo Lee님이 #general에서 나를 멘션했습니다".into(),
                    "@Alex Kim ready?".into()
                ),
                (
                    "Bo Lee님이 # general에서 허들을 시작했습니다".into(),
                    "대화에서 참여하세요".into()
                ),
                ("Bo Lee".into(), "사진을 보냈습니다".into()),
            ]
        );
        // a channel set to every message: "who · place"
        p.channel_default = "all".into();
        let deploy = notes.iter().find(|n| n.id == 12 && n.member == 1).unwrap();
        let first = device_payload(deploy, &p, NOW).unwrap().1;
        assert_eq!(
            first["aps"]["alert"]["title"],
            json!("Bo Lee님 · # general")
        );
        // a call over five minutes old no longer rings
        let call = notes.iter().find(|n| n.id == 14 && n.member == 1).unwrap();
        assert!(device_payload(call, &p, NOW + 600).is_none());
        // the same notes on an app that never said its language: as before
        let old: Vec<u64> = notes
            .iter()
            .filter(|n| n.member == 1 && crate::push::wanted(n, None, NOW))
            .map(|n| n.id)
            .collect();
        assert_eq!(old, vec![15, 13, 21]);
    }

    #[test]
    fn many_at_once_become_one_summary_but_calls_ring() {
        let p = |t: &str, k| {
            (
                k,
                json!({"aps": {"alert": {"title": t, "body": "x"}}, "target": "task:1"}),
            )
        };
        let out = batch_words(
            vec![
                p("a", Kind::Comment),
                p("b", Kind::Huddle),
                p("c", Kind::Assigned),
                p("d", Kind::Comment),
                p("e", Kind::Comment),
            ],
            Lang::Ko,
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["aps"]["alert"]["title"], json!("b"));
        assert_eq!(out[1]["aps"]["alert"]["title"], json!("새 알림 4개"));
        assert_eq!(out[1]["aps"]["alert"]["body"], json!("a: x\nc: x\nd: x"));
        assert_eq!(out[1]["target"], json!("inbox"));
    }
}
