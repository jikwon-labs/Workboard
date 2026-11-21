// Workflows: automations that start from chat. "When a message is posted
// in #channel (with a word in it), then post a message in another channel,
// reply in its thread, or make a task from it." They are `Automation`s with
// `trigger` "message" (shared with the team like the others), set up in the
// Workflows window (from a channel's settings or the Automations window).
//
// The app of whoever posts the message runs them, once, right after
// sending (the iPhone app too: Edits.runWorkflows), so no two computers
// run the same one. A post is such a message too: its keyword may be in
// the title, and its {text} is the title when it has no text (a title
// someone wrote, not the default "Update" a post without one gets). What a
// workflow posts is marked as its own (`bot` "Workflow") and starts no
// workflow itself. Its text may say {user}, {text} and {channel}.

use super::*;

/// The name a workflow's messages go under.
pub(super) const WORKFLOW_BOT: &str = "Workflow";

/// What is typed in the Workflows window.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowForm {
    pub channel: Option<u64>,
    pub keyword: String,
    /// "post", "reply" or "task"
    pub action: String,
    pub target: Option<u64>,
    pub text: String,
    pub list: Option<u64>,
}

impl Default for WorkflowForm {
    fn default() -> Self {
        WorkflowForm {
            channel: None,
            keyword: String::new(),
            action: "post".into(),
            target: None,
            text: String::new(),
            list: None,
        }
    }
}

/// `text` with {user}, {text} and {channel} filled in.
pub(super) fn fill(text: &str, user: &str, said: &str, channel: &str) -> String {
    text.replace("{user}", user)
        .replace("{text}", said)
        .replace("{channel}", channel)
}

impl App {
    fn channel_hash(&self, id: u64) -> String {
        self.data
            .channels
            .iter()
            .find(|c| c.id == id)
            .map(|c| match c.kind {
                ChannelKind::Channel => format!("#{}", c.name),
                ChannelKind::Direct => self.channel_label(c),
            })
            .unwrap_or_default()
    }

    /// A workflow in words, for its row.
    pub(super) fn workflow_text(&self, a: &Automation) -> String {
        let when = match (&a.channel, a.keyword.trim()) {
            (Some(c), "") => trf!("When a message is posted in {}", self.channel_hash(*c)),
            (Some(c), k) => trf!(
                "When a message with \"{}\" is posted in {}",
                k,
                self.channel_hash(*c)
            ),
            _ => String::new(),
        };
        let then = match a.action.as_str() {
            "reply" => trf!("reply in its thread: {}", a.value),
            "task" => trf!(
                "make a task in {}",
                a.value
                    .parse::<u64>()
                    .ok()
                    .map(|id| self.data.list_name(id))
                    .unwrap_or_default()
            ),
            _ => trf!(
                "post in {}: {}",
                a.target.map(|t| self.channel_hash(t)).unwrap_or_default(),
                a.value
            ),
        };
        format!("{}, {}", when, then)
    }

    /// Run the workflows message `msg` (just posted by me in channel `ch`)
    /// starts.
    pub(super) fn run_workflows(&mut self, ch: u64, msg: u64) {
        let Some(m) = self
            .data
            .channels
            .iter()
            .find(|c| c.id == ch)
            .and_then(|c| c.messages.iter().find(|m| m.id == msg))
            .cloned()
        else {
            return;
        };
        // (what a workflow or webhook posted starts nothing)
        if m.bot.is_some() {
            return;
        }
        let rules: Vec<Automation> = self
            .data
            .extra
            .automations
            .iter()
            .filter(|a| a.enabled && a.trigger == "message" && a.channel == Some(ch))
            .filter(|a| {
                let k = a.keyword.trim().to_lowercase();
                k.is_empty()
                    || m.text.to_lowercase().contains(&k)
                    || super::chat::typed_post_title(&m)
                        .is_some_and(|t| t.to_lowercase().contains(&k))
            })
            .cloned()
            .collect();
        if rules.is_empty() {
            return;
        }
        let me = self.data.me;
        let user = self.who(me);
        let place = self.channel_hash(ch);
        // a post without text: the title someone wrote (not the default)
        let said = match super::chat::typed_post_title(&m) {
            Some(t) if m.text.trim().is_empty() => super::chatfmt::plain(t),
            _ => super::chatfmt::plain(&m.text),
        };
        for a in rules {
            let text = fill(&a.value, &user, &said, &place);
            let post = |app: &mut App, to: u64, parent: Option<u64>, text: String| {
                let id = app.data.new_id();
                let mentions = app.chat_mentions(&text, to, &[]);
                let w = Message {
                    id,
                    author: me,
                    text,
                    ts: now_ts(),
                    mentions,
                    bot: Some(WORKFLOW_BOT.into()),
                    ..serde_json::from_value(
                        serde_json::json!({"id": 0, "author": 0, "text": "", "ts": 0}),
                    )
                    .expect("an empty message")
                };
                if let Some(c) = app.data.channels.iter_mut().find(|c| c.id == to) {
                    match parent {
                        Some(p) => {
                            if let Some(pm) = c.messages.iter_mut().find(|x| x.id == p) {
                                pm.replies.push(w);
                            }
                        }
                        None => c.messages.push(w),
                    }
                }
            };
            match a.action.as_str() {
                "reply" => post(self, ch, Some(msg), text),
                "task" => {
                    if let Some(list) = a.value.parse::<u64>().ok() {
                        let name: String = said
                            .lines()
                            .next()
                            .unwrap_or("")
                            .chars()
                            .take(120)
                            .collect();
                        if !name.trim().is_empty() {
                            self.apply(Action::Add {
                                list,
                                name,
                                status: None,
                                priority: 0,
                                assignees: vec![me],
                                due: None,
                                parent: None,
                                open: false,
                            });
                        }
                    }
                }
                _ => {
                    if let Some(to) = a.target {
                        post(self, to, None, text);
                    }
                }
            }
        }
        self.dirty = true;
    }

    /// The Workflows window. True when it should close.
    pub(super) fn workflows_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        form: &mut WorkflowForm,
    ) -> bool {
        let rules: Vec<Automation> = self
            .data
            .extra
            .automations
            .iter()
            .filter(|a| a.trigger == "message")
            .cloned()
            .collect();
        let mut remove = None;
        for a in &rules {
            ui.horizontal(|ui| {
                let mut en = a.enabled;
                if check(ui, &mut en, "").changed() {
                    if let Some(r) = self
                        .data
                        .extra
                        .automations
                        .iter_mut()
                        .find(|x| x.id == a.id)
                    {
                        r.enabled = en;
                        self.dirty = true;
                    }
                }
                ui.label(self.workflow_text(a));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let d = ui.add(ghost(egui::Button::new(
                        RichText::new(tr("Delete")).color(t.danger),
                    )));
                    name_it(&d, &format!("Delete workflow {}", a.id));
                    if d.clicked() {
                        remove = Some(a.id);
                    }
                });
            });
        }
        if let Some(id) = remove {
            self.data.extra.automations.retain(|a| a.id != id);
            self.dirty = true;
        }
        if rules.is_empty() {
            ui.label(RichText::new(tr("No workflows yet.")).color(t.text3));
        }
        ui.separator();
        ui.label(RichText::new(tr("New workflow")).strong());
        let chans: Vec<(u64, String)> = self
            .data
            .channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Channel && self.can_see(c))
            .map(|c| (c.id, format!("#{}", c.name)))
            .collect();
        let pick = |ui: &mut Ui, salt: &str, v: &mut Option<u64>| {
            let cur = v
                .and_then(|id| chans.iter().find(|c| c.0 == id))
                .map(|c| c.1.clone())
                .unwrap_or_else(|| tr("choose").into());
            egui::ComboBox::from_id_salt(salt)
                .selected_text(cur)
                .show_ui(ui, |ui| {
                    for (id, name) in &chans {
                        ui.selectable_value(v, Some(*id), name);
                    }
                })
                .response
        };
        ui.horizontal(|ui| {
            ui.label(tr("When a message is posted in"));
            let r = pick(ui, "wf_channel", &mut form.channel);
            name_it(&r, "Workflow channel");
        });
        ui.horizontal(|ui| {
            ui.label(tr("with this word in it (optional)"));
            let k = ui.add(
                field(&mut form.keyword)
                    .hint_text(tr("any message"))
                    .desired_width(140.0),
            );
            name_it(&k, "Workflow keyword");
        });
        ui.horizontal(|ui| {
            ui.label(tr("then"));
            let r = egui::ComboBox::from_id_salt("wf_action")
                .selected_text(match form.action.as_str() {
                    "reply" => tr("reply in its thread"),
                    "task" => tr("make a task in"),
                    _ => tr("post a message in"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut form.action,
                        "post".to_string(),
                        tr("post a message in"),
                    );
                    ui.selectable_value(
                        &mut form.action,
                        "reply".to_string(),
                        tr("reply in its thread"),
                    );
                    ui.selectable_value(&mut form.action, "task".to_string(), tr("make a task in"));
                })
                .response;
            name_it(&r, "Workflow action");
            match form.action.as_str() {
                "post" => {
                    let r = pick(ui, "wf_target", &mut form.target);
                    name_it(&r, "Workflow target");
                }
                "task" => {
                    let cur = form
                        .list
                        .map(|id| self.data.list_name(id))
                        .unwrap_or_else(|| tr("choose").into());
                    egui::ComboBox::from_id_salt("wf_list")
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            for s in &self.data.spaces {
                                for l in s
                                    .lists
                                    .iter()
                                    .chain(s.folders.iter().flat_map(|f| f.lists.iter()))
                                {
                                    ui.selectable_value(
                                        &mut form.list,
                                        Some(l.id),
                                        format!("{} / {}", s.name, l.name),
                                    );
                                }
                            }
                        });
                }
                _ => {}
            }
        });
        if form.action != "task" {
            let x = ui.add(
                field(&mut form.text)
                    .hint_text(tr("Message, like: {user} said {text}"))
                    .desired_width(f32::INFINITY),
            );
            name_it(&x, "Workflow message");
        }
        let ready = form.channel.is_some()
            && match form.action.as_str() {
                "post" => form.target.is_some() && !form.text.trim().is_empty(),
                "reply" => !form.text.trim().is_empty(),
                _ => form.list.is_some(),
            };
        let add = ui.add_enabled(ready, egui::Button::new(tr("Add workflow")));
        name_it(&add, "Add workflow");
        if add.clicked() {
            let id = self.data.new_id();
            self.data.extra.automations.push(Automation {
                id,
                enabled: true,
                trigger: "message".into(),
                channel: form.channel,
                keyword: form.keyword.trim().to_string(),
                action: form.action.clone(),
                value: if form.action == "task" {
                    form.list.map(|l| l.to_string()).unwrap_or_default()
                } else {
                    form.text.trim().to_string()
                },
                target: if form.action == "post" {
                    form.target
                } else {
                    None
                },
                ..Default::default()
            });
            *form = WorkflowForm {
                channel: form.channel,
                ..Default::default()
            };
            self.dirty = true;
        }
        false
    }
}
