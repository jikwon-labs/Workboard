// The example workspace earlier versions started with (tests still build on
// it), and taking that example content out of a workspace.

use super::*;

impl Data {
    /// Take out the example content (people, spaces, tasks, docs, chat...)
    /// that earlier versions started with, keeping everything the person made.
    pub fn remove_demo(&mut self) {
        use serde_json::Value;
        use std::collections::HashSet;
        fn label(v: &Value) -> Option<String> {
            ["name", "title", "text"]
                .iter()
                .find_map(|k| v.get(*k).and_then(|x| x.as_str()).map(|x| x.to_string()))
        }
        fn collect(v: &Value, key: &str, out: &mut HashSet<(String, String)>) {
            match v {
                Value::Array(a) => {
                    for x in a {
                        if x.get("id").is_some() {
                            if let Some(l) = label(x) {
                                out.insert((key.to_string(), l));
                            }
                        }
                        collect(x, key, out);
                    }
                }
                Value::Object(o) => {
                    for (k, x) in o {
                        collect(x, k, out);
                    }
                }
                _ => {}
            }
        }
        // true when the item (or something inside it) must stay
        fn filter(
            v: &mut Value,
            key: &str,
            demo: &HashSet<(String, String)>,
            keep: &HashSet<u64>,
        ) -> bool {
            let mut stays = false;
            match v {
                Value::Array(a) => {
                    a.retain_mut(|x| {
                        let inner = filter(x, key, demo, keep);
                        let Some(id) = x.get("id").and_then(|i| i.as_u64()) else {
                            return true;
                        };
                        let is_demo = id < 1000
                            && label(x).map_or(false, |l| demo.contains(&(key.to_string(), l)));
                        let keep_it = !is_demo || inner || keep.contains(&id);
                        stays |= keep_it;
                        keep_it
                    });
                }
                Value::Object(o) => {
                    for (k, x) in o.iter_mut() {
                        if x.is_array() {
                            let had = x
                                .as_array()
                                .map_or(false, |a| a.iter().any(|i| i.get("id").is_some()));
                            let any = filter(x, k, demo, keep);
                            stays |= had && any;
                        }
                    }
                }
                _ => {}
            }
            stays
        }
        let mut seed = serde_json::to_value(Data::seed()).unwrap_or_default();
        if let Some(e) = seed.get_mut("extra").and_then(|e| e.as_object_mut()) {
            e.remove("sync");
        }
        let mut demo = HashSet::new();
        collect(&seed, "", &mut demo);
        let mut v = serde_json::to_value(&*self).unwrap_or_default();
        // lists still holding the person's own tasks, and the person themself, stay
        let mut keep: HashSet<u64> = HashSet::new();
        keep.insert(self.me);
        let demo_task =
            |t: &Task| t.id < 1000 && demo.contains(&("tasks".to_string(), t.name.clone()));
        for t in &self.tasks {
            if !demo_task(t) {
                keep.insert(t.list_id);
                for s in &self.spaces {
                    if s.lists.iter().any(|l| l.id == t.list_id) {
                        keep.insert(s.id);
                    }
                    for f in &s.folders {
                        if f.lists.iter().any(|l| l.id == t.list_id) {
                            keep.insert(f.id);
                            keep.insert(s.id);
                        }
                    }
                }
            }
        }
        for d in &self.docs {
            if !(d.id < 1000 && demo.contains(&("docs".to_string(), d.title.clone()))) {
                if let Some(sp) = d.space {
                    keep.insert(sp);
                }
            }
        }
        if let Some(o) = v.as_object_mut() {
            for (k, x) in o.iter_mut() {
                if k == "extra" {
                    if let Some(e) = x.as_object_mut() {
                        for (ek, ex) in e.iter_mut() {
                            if ek != "sync" && ex.is_array() {
                                filter(ex, ek, &demo, &keep);
                            }
                        }
                    }
                } else if x.is_array() {
                    filter(x, k, &demo, &keep);
                }
            }
        }
        if let Ok(mut d) = serde_json::from_value::<Data>(v) {
            let people: HashSet<u64> = d.members.iter().map(|m| m.id).collect();
            for t in &mut d.tasks {
                t.assignees.retain(|a| people.contains(a));
            }
            for c in &mut d.channels {
                c.members.retain(|m| people.contains(m));
            }
            for m in &mut d.members {
                if m.email.ends_with("@example.com") {
                    m.email.clear();
                }
            }
            let lists: HashSet<u64> = d
                .spaces
                .iter()
                .flat_map(|s| {
                    s.lists
                        .iter()
                        .map(|l| l.id)
                        .chain(s.folders.iter().flat_map(|f| f.lists.iter().map(|l| l.id)))
                })
                .collect();
            let spaces: HashSet<u64> = d.spaces.iter().map(|s| s.id).collect();
            let folders: HashSet<u64> = d
                .spaces
                .iter()
                .flat_map(|s| s.folders.iter().map(|f| f.id))
                .collect();
            d.favorites.retain(|f| match f {
                Location::List(id) => lists.contains(id),
                Location::Space(id) => spaces.contains(id),
                Location::Folder(id) => folders.contains(id),
                _ => true,
            });
            if d.spaces.is_empty() {
                let sid = d.new_id();
                let lid = d.new_id();
                d.spaces.push(Space {
                    id: sid,
                    name: "Team Space".into(),
                    color: [0x5f, 0x55, 0xee],
                    statuses: default_statuses(),
                    folders: vec![],
                    lists: vec![ListItem {
                        id: lid,
                        name: "List".into(),
                        ..Default::default()
                    }],
                    expanded: true,
                    private_owner: None,
                    ..Default::default()
                });
            }
            d.extra.demo_removed = true;
            *self = d;
        }
    }

    pub fn seed() -> Self {
        let mut d = Data {
            workspace: "My Workspace".into(),
            spaces: vec![],
            tasks: vec![],
            members: vec![
                Member {
                    id: 1,
                    name: "Me".into(),
                    color: [0x5f, 0x55, 0xee],
                    email: "me@example.com".into(),
                    role: "Owner".into(),
                    ..Default::default()
                },
                Member {
                    id: 2,
                    name: "Alex Kim".into(),
                    color: [0x10, 0x90, 0xe0],
                    email: "alex@example.com".into(),
                    role: "Admin".into(),
                    ..Default::default()
                },
                Member {
                    id: 3,
                    name: "Sam Lee".into(),
                    color: [0xe1, 0x6b, 0x16],
                    email: "sam@example.com".into(),
                    role: "Member".into(),
                    ..Default::default()
                },
                Member {
                    id: 4,
                    name: "Jordan Park".into(),
                    color: [0x00, 0x88, 0x44],
                    email: "jordan@example.com".into(),
                    role: "Member".into(),
                    ..Default::default()
                },
            ],
            me: 1,
            next_id: 100,
            favorites: vec![],
            dark: false,
            docs: vec![],
            channels: vec![],
            extra: Extra::default(),
        };
        let space = d.new_id();
        let folder = d.new_id();
        let l_web = d.new_id();
        let l_mkt = d.new_id();
        let l_personal = d.new_id();
        let space2 = d.new_id();
        let l_ideas = d.new_id();
        d.spaces.push(Space {
            id: space,
            name: "Team Space".into(),
            color: [0x5f, 0x55, 0xee],
            statuses: default_statuses(),
            folders: vec![Folder {
                id: folder,
                name: "Projects".into(),
                lists: vec![
                    ListItem {
                        id: l_web,
                        name: "Website Redesign".into(),
                        ..Default::default()
                    },
                    ListItem {
                        id: l_mkt,
                        name: "Marketing".into(),
                        ..Default::default()
                    },
                ],
                expanded: true,
                archived: false,
            }],
            lists: vec![ListItem {
                id: l_personal,
                name: "Personal".into(),
                ..Default::default()
            }],
            expanded: true,
            private_owner: None,
            ..Default::default()
        });
        d.spaces.push(Space {
            id: space2,
            name: "Side Projects".into(),
            color: [0xe1, 0x6b, 0x16],
            statuses: default_statuses(),
            folders: vec![],
            lists: vec![ListItem {
                id: l_ideas,
                name: "Ideas".into(),
                ..Default::default()
            }],
            expanded: false,
            private_owner: None,
            ..Default::default()
        });
        d.favorites.push(Location::List(l_web));

        let t = today();
        let wd = t.weekday().num_days_from_monday() as i64;
        let day = |n: i64| Some(t + Duration::days(n));
        let samples: Vec<(
            u64,
            &str,
            &str,
            u8,
            Vec<u64>,
            Option<NaiveDate>,
            Option<NaiveDate>,
            Vec<&str>,
        )> = vec![
            (
                l_web,
                "Design new homepage layout",
                "in progress",
                2,
                vec![1, 2],
                day(-wd),
                day(2),
                vec!["design"],
            ),
            (
                l_web,
                "Write copy for landing page",
                "to do",
                3,
                vec![3],
                day(1),
                day(4),
                vec!["content"],
            ),
            (
                l_web,
                "Set up analytics dashboard",
                "to do",
                4,
                vec![],
                None,
                day(9),
                vec![],
            ),
            (
                l_web,
                "Fix navigation on mobile",
                "in progress",
                1,
                vec![1],
                day(-3),
                day(-1),
                vec!["bug"],
            ),
            (
                l_web,
                "Collect feedback from beta users",
                "to do",
                0,
                vec![4],
                None,
                None,
                vec![],
            ),
            (
                l_web,
                "Choose color palette",
                "complete",
                3,
                vec![2],
                day(-8),
                day(-5),
                vec!["design"],
            ),
            (
                l_mkt,
                "Plan Q1 campaign",
                "in progress",
                2,
                vec![1, 3],
                day(-2),
                day(6),
                vec![],
            ),
            (
                l_mkt,
                "Newsletter draft",
                "to do",
                3,
                vec![3],
                None,
                day(3),
                vec!["content"],
            ),
            (
                l_mkt,
                "Social media calendar",
                "complete",
                4,
                vec![4],
                day(-10),
                day(-6),
                vec![],
            ),
            (
                l_personal,
                "Renew passport",
                "to do",
                2,
                vec![1],
                None,
                day(0),
                vec![],
            ),
            (
                l_personal,
                "Book dentist appointment",
                "to do",
                3,
                vec![1],
                None,
                day(12),
                vec![],
            ),
            (
                l_ideas,
                "Build a reading tracker",
                "to do",
                0,
                vec![1],
                None,
                None,
                vec![],
            ),
        ];
        let now = now_ts();
        for (i, (list, name, st, pr, asg, start, due, tags)) in samples.into_iter().enumerate() {
            let id = d.new_id();
            d.tasks.push(Task {
                id,
                list_id: list,
                parent: None,
                name: name.into(),
                description: String::new(),
                status: st.into(),
                priority: pr,
                assignees: asg,
                start,
                due,
                start_time: None,
                due_time: None,
                tags: tags.into_iter().map(String::from).collect(),
                checklist: vec![],
                activity: vec![Activity {
                    ts: now - 86400 * 3,
                    author: 1,
                    text: "created this task".into(),
                    comment: false,
                    ..Default::default()
                }],
                time_entries: vec![],
                timer_started: None,
                timer_by: None,
                timers: vec![],
                estimate_min: None,
                created: now - 86400 * 3 + i as i64,
                updated: now,
                order: i as i64,
                custom: Default::default(),
                recur: None,
                archived: false,
                done_at: None,
                closed_at: None,
                milestone: false,
                watchers: vec![],
                unwatched: None,
                waiting_on: vec![],
                linked: vec![],
                attachments: vec![],
                form: None,
            });
        }
        // Subtasks and a checklist on the first task.
        let first = d.tasks[0].id;
        for (n, name) in ["Wireframe hero section", "Pick hero image"]
            .iter()
            .enumerate()
        {
            let id = d.new_id();
            d.tasks.push(Task {
                id,
                list_id: l_web,
                parent: Some(first),
                name: (*name).into(),
                description: String::new(),
                status: if n == 0 {
                    "complete".into()
                } else {
                    "to do".into()
                },
                priority: 0,
                assignees: vec![2],
                start: None,
                due: None,
                start_time: None,
                due_time: None,
                tags: vec![],
                checklist: vec![],
                activity: vec![],
                time_entries: vec![],
                timer_started: None,
                timer_by: None,
                timers: vec![],
                estimate_min: None,
                created: now,
                updated: now,
                order: 100 + n as i64,
                custom: Default::default(),
                recur: None,
                archived: false,
                done_at: None,
                closed_at: None,
                milestone: false,
                watchers: vec![],
                unwatched: None,
                waiting_on: vec![],
                linked: vec![],
                attachments: vec![],
                form: None,
            });
        }
        d.tasks[0].description =
            "Refresh the homepage with a cleaner hero, clear call to action and a lighter grid."
                .into();
        d.tasks[0].checklist = vec![
            CheckItem {
                text: "Review competitors".into(),
                done: true,
                assignee: None,
            },
            CheckItem {
                text: "Sketch three options".into(),
                done: false,
                assignee: None,
            },
            CheckItem {
                text: "Share with the team".into(),
                done: false,
                assignee: None,
            },
        ];
        let now2 = now_ts();
        let doc_id = d.new_id();
        d.docs.push(Doc {
            id: doc_id,
            space: Some(space),
            owner: 1,
            private: false,
            archived: false,
            meeting_note: false,
            wiki: None,
            views: vec![],
            parent: None,
            title: "Project brief".into(),
            body: "Goals\n- Launch the new website\n- Grow the newsletter\n\nNotes\nKeep the design light and simple.".into(),
            created: now2 - 86400,
            updated: now2 - 3600,
            tags: vec![],
        });
        // chat
        let msg = |d: &mut Data, author: u64, text: &str, ago: i64| Message {
            goal: None,
            id: d.new_id(),
            author,
            text: text.into(),
            mentions: vec![],
            ts: now2 - ago,
            reactions: vec![],
            replies: vec![],
            saved: false,
            task: None,
            post_title: None,
            post_type: String::new(),
            pinned: false,
            call: None,
            attachments: vec![],
            edited: None,
            imported: None,
            preview: None,
            forwarded: None,
            thread_of: None,
            bot: None,
        };
        let c1 = d.new_id();
        let mut m1 = msg(
            &mut d,
            2,
            "Morning everyone! The new homepage mockups are in the Website Redesign list.",
            3 * 3600,
        );
        m1.reactions.push(Reaction {
            emoji: "\u{1F44D}".into(),
            users: vec![3, 4],
        });
        let r1 = msg(
            &mut d,
            1,
            "Looks great, I'll review them after lunch.",
            2 * 3600,
        );
        m1.replies.push(r1);
        let m2 = msg(
            &mut d,
            3,
            "Reminder: newsletter draft is due Thursday.",
            3600,
        );
        let m3 = msg(
            &mut d,
            4,
            "@Me can you check the beta feedback list when you have a minute?",
            1200,
        );
        d.channels.push(Channel {
            id: c1,
            name: "general".into(),
            kind: ChannelKind::Channel,
            topic: "Company-wide announcements and chat".into(),
            members: vec![1, 2, 3, 4],
            messages: vec![m1, m2, m3],
            last_read: now2 - 1800,
            ..Default::default()
        });
        let c2 = d.new_id();
        let m4 = msg(
            &mut d,
            2,
            "Should the hero image be a photo or an illustration?",
            5400,
        );
        d.channels.push(Channel {
            id: c2,
            name: "website-redesign".into(),
            kind: ChannelKind::Channel,
            topic: "Everything about the new site".into(),
            members: vec![1, 2],
            messages: vec![m4],
            last_read: now2,
            ..Default::default()
        });
        let c3 = d.new_id();
        let m5 = msg(
            &mut d,
            2,
            "Hey, do you have a minute to talk about the launch date?",
            600,
        );
        d.channels.push(Channel {
            id: c3,
            name: String::new(),
            kind: ChannelKind::Direct,
            topic: String::new(),
            members: vec![1, 2],
            messages: vec![m5],
            last_read: 0,
            ..Default::default()
        });
        let c4 = d.new_id();
        d.channels.push(Channel {
            id: c4,
            name: String::new(),
            kind: ChannelKind::Direct,
            topic: String::new(),
            members: vec![1, 3],
            messages: vec![],
            last_read: now2,
            ..Default::default()
        });
        d.tasks[0].time_entries.push(TimeEntry {
            start: now2 - 86400 - 5400,
            end: now2 - 86400,
            member: 0,
        });
        d.tasks[3].time_entries.push(TimeEntry {
            start: now2 - 7200,
            end: now2 - 4800,
            member: 0,
        });
        d.tasks[0].activity.push(Activity {
            ts: now - 3600 * 5,
            author: 2,
            text: "Looks great so far, let's review on Friday.".into(),
            comment: true,
            ..Default::default()
        });
        d
    }
}
