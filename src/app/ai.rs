// The AI tools: summing up a conversation or a thread, working on writing
// in a doc page or a task description, suggesting subtasks, and the AI
// page, where people ask about the workspace. The team server asks the AI
// (server/src/ai.rs) from what this person may see; the app sends a request
// and shows the answer as its pieces come. The tools show in workspaces on
// a team server that says it has an AI (`ai_ready`).
//
// A request is a job, kept while a dialog or the AI page shows it. A job
// nothing shows any more is stopped on the server (`ai_stop`) and
// forgotten; one still going when the connection drops fails, as its answer
// can't come any more.

use super::icons::{icon_button, icon_button_ex, line_icon, line_icon_scaled};
use super::*;
use std::collections::BTreeMap;
use wb_net::{AiAsk, AiTurn, Msg};

/// Earlier questions and answers a question takes along (as many as the
/// server reads).
use wb_net::AI_MAX_HISTORY as MAX_HISTORY;
/// Said when a request can't go: no connection, or the server has no AI.
pub(super) const NOT_READY: &str =
    "Not connected to a team server with AI. Try again once it's back.";
/// Said when the connection broke while an answer was coming.
pub(super) const BROKE: &str =
    "The connection to the server broke before the answer came. Try again.";
/// Said when a result can't be put back because its text changed meanwhile.
pub(super) const CHANGED: &str =
    "The text changed while the AI was working, so the answer wasn't put in. Copy it instead.";

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AiState {
    Going,
    Done,
    Failed(String),
}

/// One request and its answer so far.
pub(super) struct AiJob {
    /// what was asked (asked again by "Try again")
    pub ask: AiAsk,
    pub text: String,
    pub state: AiState,
}

/// A question on the AI page and the job answering it.
pub(super) struct AiQuestion {
    pub question: String,
    pub job: u64,
}

#[derive(Default)]
pub(super) struct AiRt {
    /// the server's model, from its ai_ready until the connection drops
    pub model: Option<String>,
    /// the server address that last said it has an AI: its tools stay
    /// shown there while a dropped connection comes back
    offered_at: Option<String>,
    next: u64,
    pub jobs: BTreeMap<u64, AiJob>,
    /// the AI page's conversation, oldest first
    pub chat: Vec<AiQuestion>,
    /// the question being typed there
    pub draft: String,
    /// a task window or dialog was open at the end of the last frame: an
    /// Escape now is for it, not for leaving the AI page
    window_was_open: bool,
    /// the workspace (its file and server) all this belongs to: another
    /// workspace never sees the conversation, nor sends it to its server
    owner: Option<(String, Option<String>)>,
}

/// What a result dialog does with its answer.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum AiUse {
    /// a summary to read and copy
    Read,
    /// writing for doc page `doc`: the part asked about (a character
    /// range, or the whole page) and what it said then
    Doc {
        doc: u64,
        range: Option<(usize, usize)>,
        was: String,
        how: String,
    },
    /// a task's description, and what it said then
    Description { task: u64, was: String, how: String },
    /// subtasks for a task: the suggestions read from the answer, and
    /// which are ticked
    Subtasks {
        task: u64,
        names: Vec<String>,
        picked: Vec<bool>,
    },
}

/// The AI result dialog (`Modal::Ai`).
pub(super) struct AiDialog {
    pub job: u64,
    pub title: String,
    pub use_: AiUse,
}

/// What each writing action is called, by `AiAsk::Write`'s `how`.
pub(super) const WRITE_ACTIONS: [(&str, &str); 8] = [
    ("improve", "Improve writing"),
    ("fix", "Fix spelling and grammar"),
    ("shorter", "Make shorter"),
    ("longer", "Make longer"),
    ("summarize", "Summarize"),
    ("korean", "Translate to Korean"),
    ("english", "Translate to English"),
    ("continue", "Continue writing"),
];

/// The name of a writing action, untranslated.
fn action_label(how: &str) -> &'static str {
    if how == "draft" {
        return "Write a first draft";
    }
    WRITE_ACTIONS
        .iter()
        .find(|(h, _)| *h == how)
        .map(|(_, l)| *l)
        .unwrap_or("AI")
}

/// Where in `body` (characters) the text a request was made on sits now:
/// at `range` when it is still there, else where it moved to, when it is
/// on the page once.
/// The whole of `body` when the request was on the whole of it and it
/// hasn't changed. None when it's gone (changed meanwhile).
pub(super) fn find_again(
    body: &str,
    range: Option<(usize, usize)>,
    was: &str,
) -> Option<(usize, usize)> {
    let chars: Vec<char> = body.chars().collect();
    match range {
        None => (body == was).then_some((0, chars.len())),
        Some((a, b)) => {
            let n = was.chars().count();
            if b <= chars.len() && a <= b && chars[a..b].iter().collect::<String>() == was {
                return Some((a, b));
            }
            if was.is_empty() {
                return None;
            }
            // moved: wherever it is now, when that is only one place (with
            // the same words twice, which one was meant can't be told)
            let mut found = body.match_indices(was).map(|(i, _)| i);
            let at = found.next()?;
            if found.next().is_some() {
                return None;
            }
            let c = body[..at].chars().count();
            Some((c, c + n))
        }
    }
}

/// `body` with `new` in place of characters `a..b` (`replace`) or after
/// them on a paragraph of its own.
pub(super) fn splice(body: &str, at: (usize, usize), new: &str, replace: bool) -> String {
    splice_at(body, at, new, replace).0
}

/// `splice`, and where (characters) the new text ends in the result.
pub(super) fn splice_at(
    body: &str,
    (a, b): (usize, usize),
    new: &str,
    replace: bool,
) -> (String, usize) {
    let at = |c: usize| {
        body.char_indices()
            .nth(c)
            .map(|(i, _)| i)
            .unwrap_or(body.len())
    };
    let (start, end) = (at(a), at(b));
    let new = new.trim_matches('\n');
    let n = new.chars().count();
    if replace {
        return (format!("{}{}{}", &body[..start], new, &body[end..]), a + n);
    }
    let before = &body[..end];
    let gap = if before.is_empty() || before.ends_with("\n\n") {
        ""
    } else if before.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    let after = &body[end..];
    let tail = if after.is_empty() || after.starts_with("\n\n") {
        ""
    } else if after.starts_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    (
        format!("{}{}{}{}{}", before, gap, new, tail, after),
        b + gap.chars().count() + n,
    )
}

/// The subtasks in an answer: {"subtasks": ["...", ...]}, each once, not
/// empty, at most 20. None when it can't be read.
pub(super) fn subtasks_in(answer: &str) -> Option<Vec<String>> {
    let s = answer.trim();
    let s = s
        .strip_prefix("```json")
        .or_else(|| s.strip_prefix("```"))
        .unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s).trim();
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    let mut out: Vec<String> = vec![];
    for x in v.get("subtasks")?.as_array()? {
        let name = x.as_str()?.split_whitespace().collect::<Vec<_>>().join(" ");
        if !name.is_empty() && !out.contains(&name) && out.len() < 20 {
            out.push(name);
        }
    }
    Some(out)
}

/// An answer as chat formatting shows it: headings become bold lines, and
/// the markers the AI cites things with become links to them (named by
/// `name`; a marker for something not here is left out).
pub(super) fn linked(answer: &str, name: &dyn Fn(&str, u64, u64) -> Option<String>) -> String {
    let mut out = String::with_capacity(answer.len());
    for (i, line) in answer.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let t = line.trim_start();
        let heading = t.starts_with('#') && t.trim_start_matches('#').starts_with(' ');
        let indent = &line[..line.len() - t.len()];
        let line = if heading {
            format!("**{}**", t.trim_start_matches('#').trim())
        } else if let Some(rest) = t.strip_prefix("- [ ] ") {
            format!("{}\u{2610} {}", indent, rest)
        } else if let Some(rest) = t
            .strip_prefix("- [x] ")
            .or_else(|| t.strip_prefix("- [X] "))
        {
            format!("{}\u{2611} {}", indent, rest)
        } else {
            line.to_string()
        };
        let mut rest = line.as_str();
        while let Some(at) = rest.find('[') {
            out.push_str(&rest[..at]);
            let tail = &rest[at..];
            let Some(end) = tail.find(']') else {
                out.push_str(tail);
                rest = "";
                break;
            };
            let inner = &tail[1..end];
            let mut parts = inner.split(':');
            let kind = parts.next().unwrap_or("");
            let a = parts.next().and_then(|x| x.trim().parse::<u64>().ok());
            let b = parts.next().and_then(|x| x.trim().parse::<u64>().ok());
            let marker = match (kind, a, b, parts.next()) {
                ("task" | "doc", Some(a), None, None) => Some((kind, a, 0)),
                ("msg", Some(a), Some(b), None) => Some((kind, a, b)),
                _ => None,
            };
            // a link written as [text](url) is the answer's own
            let own_link = tail[end + 1..].starts_with('(');
            match marker {
                Some((kind, a, b)) if !own_link => {
                    if let Some(label) = name(kind, a, b) {
                        let url = match kind {
                            "msg" => format!("workboard://ai-msg/{}/{}", a, b),
                            k => format!("workboard://{}/{}", k, a),
                        };
                        // the name written just before its marker becomes
                        // the link, rather than showing twice
                        // (on this line only: a name ending the line above stays)
                        let kept = out.trim_end_matches([' ', '\t']).len();
                        let named = [
                            ("**", "**"),
                            ("\u{201C}", "\u{201D}"),
                            ("\"", "\""),
                            ("'", "'"),
                            ("`", "`"),
                            ("", ""),
                        ]
                        .iter()
                        .map(|(l, r)| (l.is_empty(), format!("{}{}{}", l, label, r)))
                        .find(|(bare, w)| {
                            let start = kept.saturating_sub(w.len());
                            !label.trim().is_empty()
                                && out[..kept].ends_with(w.as_str())
                                // a bare name starts a word ("GraphAPI" isn't "API")
                                && (!bare
                                    || !out[..start]
                                        .chars()
                                        .next_back()
                                        .is_some_and(|c| c.is_alphanumeric() || c == '_'))
                        })
                        .map(|(_, w)| w);
                        let gap = if let Some(w) = named {
                            out.truncate(kept - w.len());
                            ""
                        } else if out.ends_with(|c: char| !c.is_whitespace()) && !out.is_empty() {
                            " "
                        } else {
                            ""
                        };
                        let label: String = label
                            .chars()
                            .map(|c| match c {
                                '[' | ']' => ' ',
                                c => c,
                            })
                            .collect();
                        out.push_str(&format!("{}[{}]({})", gap, label.trim(), url));
                    } else {
                        // drop the space before a marker that is left out
                        while out.ends_with(' ') {
                            out.pop();
                        }
                    }
                }
                _ => out.push_str(&tail[..=end]),
            }
            rest = &tail[end + 1..];
        }
        out.push_str(rest);
    }
    out
}

impl App {
    // ------------------------------------------------------------ jobs

    /// Whether this workspace's team server has the AI tools (shown while
    /// a dropped connection comes back).
    pub(super) fn ai_offered(&self) -> bool {
        let here = self
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.clone());
        here.is_some() && self.ai.offered_at == here
    }

    /// Whether a request can go now.
    pub(super) fn ai_reachable(&self) -> bool {
        self.ai.model.is_some() && self.connected() && self.sync_rt.net.is_some()
    }

    /// Sends request `ask`; the job's id. It fails at once when it can't go.
    pub(super) fn ai_ask(&mut self, ask: AiAsk) -> u64 {
        self.ai.next += 1;
        let id = self.ai.next;
        let lang = if crate::i18n::is_ko() { "ko" } else { "en" };
        let state = match &self.sync_rt.net {
            Some(net) if self.ai_reachable() => {
                net.send(Msg::Ai {
                    id,
                    ask: ask.clone(),
                    lang: lang.into(),
                });
                AiState::Going
            }
            _ => AiState::Failed(tr(NOT_READY).into()),
        };
        self.ai.jobs.insert(
            id,
            AiJob {
                ask,
                text: String::new(),
                state,
            },
        );
        id
    }

    /// Asks `ask` and shows the answer in a dialog titled `title`.
    pub(super) fn ai_dialog(&mut self, ask: AiAsk, title: String, use_: AiUse) {
        let job = self.ai_ask(ask);
        self.modal = Some(Modal::Ai(Box::new(AiDialog { job, title, use_ })));
    }

    /// The server said it has an AI.
    pub(super) fn ai_ready_now(&mut self, model: String) {
        self.ai.model = Some(model);
        self.ai.offered_at = self
            .data
            .extra
            .shared
            .as_ref()
            .and_then(|c| c.server.clone());
    }

    /// A new connection is in: whether it has an AI, its ai_ready says.
    pub(super) fn ai_welcomed(&mut self) {
        self.ai.model = None;
        self.ai.offered_at = None;
        self.ai_fail_going();
    }

    /// The connection dropped: answers on their way won't come.
    pub(super) fn ai_lost(&mut self) {
        self.ai.model = None;
        self.ai_fail_going();
    }

    fn ai_fail_going(&mut self) {
        for j in self.ai.jobs.values_mut() {
            if j.state == AiState::Going {
                j.state = AiState::Failed(tr(BROKE).into());
            }
        }
    }

    pub(super) fn ai_text_came(&mut self, id: u64, text: &str) {
        if let Some(j) = self.ai.jobs.get_mut(&id) {
            if j.state == AiState::Going {
                j.text.push_str(text);
            }
        }
    }

    pub(super) fn ai_done_came(&mut self, id: u64, error: Option<String>) {
        if let Some(j) = self.ai.jobs.get_mut(&id) {
            if j.state == AiState::Going {
                j.state = match error {
                    Some(e) => AiState::Failed(together::server_text(&e)),
                    None => AiState::Done,
                };
            }
        }
    }

    /// Once a frame, after the dialogs: jobs nothing shows any more are
    /// stopped and forgotten, and going ones fail when the connection is gone.
    /// Forgets every request and the AI page's conversation (another
    /// workspace opened); answers still coming are stopped.
    pub(super) fn ai_forget(&mut self) {
        if let Some(net) = &self.sync_rt.net {
            for (id, j) in &self.ai.jobs {
                if j.state == AiState::Going {
                    net.send(Msg::AiStop { id: *id });
                }
            }
        }
        // what the connection said stays with the connection (welcome and
        // a dropped connection set it), and ids keep going up so a late
        // answer to an old request never lands on a new one
        let fresh = AiRt {
            model: self.ai.model.take(),
            offered_at: self.ai.offered_at.take(),
            next: self.ai.next,
            ..Default::default()
        };
        self.ai = fresh;
    }

    pub(super) fn ai_tick(&mut self) {
        let here = (
            self.ws_file.clone(),
            self.data
                .extra
                .shared
                .as_ref()
                .and_then(|c| c.server.clone()),
        );
        if self.ai.owner.as_ref().is_some_and(|o| *o != here) {
            self.ai_forget();
            if matches!(self.modal, Some(Modal::Ai(_))) {
                self.modal = None;
            }
        }
        self.ai.owner = Some(here);
        self.ai.window_was_open = self.open_task.is_some() || self.modal.is_some();
        if !self.ai.jobs.is_empty() && !(self.connected() && self.sync_rt.net.is_some()) {
            self.ai.model = None;
            self.ai_fail_going();
        }
        let mut shown: HashSet<u64> = self.ai.chat.iter().map(|q| q.job).collect();
        if let Some(Modal::Ai(d)) = &self.modal {
            shown.insert(d.job);
        }
        let gone: Vec<u64> = self
            .ai
            .jobs
            .keys()
            .filter(|id| !shown.contains(id))
            .copied()
            .collect();
        for id in gone {
            if let Some(j) = self.ai.jobs.remove(&id) {
                if j.state == AiState::Going {
                    if let Some(net) = &self.sync_rt.net {
                        net.send(Msg::AiStop { id });
                    }
                }
            }
        }
    }

    // ------------------------------------------------------ opening them

    /// "Summarize" in a conversation's header.
    pub(super) fn ai_summarize_channel(&mut self, cid: u64, title: &str) {
        self.ai_dialog(
            AiAsk::SummarizeChannel { channel: cid },
            trf!("Summary of {}", title),
            AiUse::Read,
        );
    }

    /// "Summarize thread" in a thread's header: the thread under message
    /// `tid`, in whichever copy of the conversation holds it.
    pub(super) fn ai_summarize_thread(&mut self, tid: u64) {
        let Some(ch) = self
            .data
            .channels
            .iter()
            .find(|c| c.messages.iter().any(|m| m.id == tid))
            .map(|c| c.id)
        else {
            return;
        };
        self.ai_dialog(
            AiAsk::SummarizeThread {
                channel: ch,
                message: tid,
            },
            tr("Thread summary").into(),
            AiUse::Read,
        );
    }

    /// The AI menu of a doc page's formatting bar: what to do with the
    /// selected text, or with the whole page when nothing is selected.
    pub(super) fn doc_ai_menu(&mut self, ui: &mut Ui, idx: usize, body_id: egui::Id) {
        let t = self.t();
        let d = &self.data.docs[idx];
        let (doc, title, body) = (d.id, d.title.clone(), d.body.clone());
        let n = body.chars().count();
        let cur: usize = ui
            .data(|m| m.get_temp(body_id.with("cursor")))
            .unwrap_or(n)
            .min(n);
        let sel: usize = ui
            .data(|m| m.get_temp(body_id.with("sel")))
            .unwrap_or(cur)
            .min(n);
        let (a, b) = (cur.min(sel), cur.max(sel));
        let picked = a < b && body.chars().skip(a).take(b - a).any(|c| !c.is_whitespace());
        let (range, was) = if picked {
            (
                Some((a, b)),
                body.chars().skip(a).take(b - a).collect::<String>(),
            )
        } else {
            (None, body.clone())
        };
        ui.label(
            RichText::new(if picked {
                tr("On the selected text")
            } else {
                tr("On the whole page")
            })
            .small()
            .color(t.text3),
        );
        let mut chosen: Option<&str> = None;
        if was.trim().is_empty() {
            if title.trim().is_empty() {
                ui.label(
                    RichText::new(tr("Write something or give the page a title first."))
                        .color(t.text2),
                );
            } else if ai_row(ui, &t, "draft") {
                chosen = Some("draft");
            }
        } else {
            for (how, _) in WRITE_ACTIONS {
                if ai_row(ui, &t, how) {
                    chosen = Some(how);
                }
            }
        }
        if let Some(how) = chosen {
            close_popup(ui);
            self.ai_dialog(
                AiAsk::Write {
                    how: how.into(),
                    text: was.clone(),
                    title,
                },
                tr(action_label(how)).into(),
                AiUse::Doc {
                    doc,
                    range,
                    was,
                    how: how.into(),
                },
            );
        }
    }

    /// The AI menu by a task's description.
    pub(super) fn description_ai_menu(&mut self, ui: &mut Ui, task: &Task) {
        let t = self.t();
        let mut chosen: Option<&str> = None;
        if task.description.trim().is_empty() {
            if ai_row(ui, &t, "draft") {
                chosen = Some("draft");
            }
        } else {
            for (how, _) in WRITE_ACTIONS {
                if how != "continue" && how != "summarize" && ai_row(ui, &t, how) {
                    chosen = Some(how);
                }
            }
        }
        if let Some(how) = chosen {
            close_popup(ui);
            self.ai_dialog(
                AiAsk::Write {
                    how: how.into(),
                    text: task.description.clone(),
                    title: task.name.clone(),
                },
                tr(action_label(how)).into(),
                AiUse::Description {
                    task: task.id,
                    was: task.description.clone(),
                    how: how.into(),
                },
            );
        }
    }

    /// "Suggest subtasks" by a task's Subtasks.
    pub(super) fn ai_suggest_subtasks(&mut self, task: &Task) {
        self.ai_dialog(
            AiAsk::Subtasks {
                name: task.name.clone(),
                description: task.description.clone(),
            },
            tr("Suggested subtasks").into(),
            AiUse::Subtasks {
                task: task.id,
                names: vec![],
                picked: vec![],
            },
        );
    }

    // ------------------------------------------------------- the dialog

    /// The body of the AI result dialog; true closes it.
    pub(super) fn ai_dialog_body(&mut self, ui: &mut Ui, t: &Theme, d: &mut AiDialog) -> bool {
        let mut close = false;
        let (state, text) = match self.ai.jobs.get(&d.job) {
            Some(j) => (j.state.clone(), j.text.clone()),
            None => (AiState::Failed(tr(BROKE).into()), String::new()),
        };
        let max_h = (ui.ctx().screen_rect().height() * 0.55).max(160.0);
        ui.set_min_width(520.0);
        match &d.use_ {
            AiUse::Doc { range, .. } => {
                ui.label(
                    RichText::new(if range.is_some() {
                        tr("On the selected text")
                    } else {
                        tr("On the whole page")
                    })
                    .small()
                    .color(t.text3),
                );
            }
            AiUse::Description { .. } => {
                ui.label(
                    RichText::new(tr("On the description"))
                        .small()
                        .color(t.text3),
                );
            }
            _ => {}
        }
        // subtasks: read from the whole answer once it's done
        if let (AiUse::Subtasks { names, picked, .. }, AiState::Done) = (&mut d.use_, &state) {
            if names.is_empty() {
                if let Some(found) = subtasks_in(&text) {
                    *picked = vec![true; found.len()];
                    *names = found;
                }
            }
        }
        let unreadable = matches!(&d.use_, AiUse::Subtasks { names, .. } if names.is_empty())
            && state == AiState::Done;
        egui::Frame::new()
            .fill(t.column_bg)
            .corner_radius(8)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .id_salt(("ai_answer", d.job))
                    .max_height(max_h)
                    .auto_shrink([false, true])
                    .stick_to_bottom(state == AiState::Going)
                    .show(ui, |ui| {
                        let waiting = state == AiState::Going
                            && (text.is_empty() || matches!(d.use_, AiUse::Subtasks { .. }));
                        if waiting {
                            thinking(ui, t);
                        } else if let AiUse::Subtasks { names, picked, .. } = &mut d.use_ {
                            for (i, name) in names.iter().enumerate() {
                                let r = check(ui, &mut picked[i], name.as_str());
                                name_it(&r, &format!("Subtask suggestion {}", name));
                            }
                        } else if text.is_empty() {
                            // nothing came (it failed before the answer began)
                        } else {
                            // formatted as the page or description shows it
                            self.ai_rich(ui, &text);
                        }
                        if unreadable {
                            ui.label(
                                RichText::new(tr("The AI's answer couldn't be read. Try again."))
                                    .color(crate::widgets::WARN_ORANGE),
                            );
                        }
                        if let AiState::Failed(why) = &state {
                            ui.label(
                                RichText::new(why.as_str()).color(crate::widgets::WARN_ORANGE),
                            );
                        }
                    });
            });
        ui.add_space(10.0);
        let mut again = false;
        // put the answer in: replacing what it was asked on, or below it
        let mut put: Option<bool> = None;
        ui.horizontal(|ui| {
            match (&mut d.use_, &state) {
                (_, AiState::Going) => {
                    if secondary_button(ui, tr("Stop")).clicked() {
                        close = true;
                    }
                }
                (AiUse::Read, AiState::Done) => {
                    if primary_button(ui, t, tr("Copy")).clicked() {
                        ui.ctx().copy_text(text.clone());
                        self.toast(tr("Copied"));
                    }
                    if secondary_button(ui, tr("Close")).clicked() {
                        close = true;
                    }
                }
                (
                    AiUse::Subtasks {
                        task,
                        names,
                        picked,
                    },
                    AiState::Done,
                ) if !names.is_empty() => {
                    let n = picked.iter().filter(|p| **p).count();
                    let add = ui.add_enabled(
                        n > 0,
                        Tint(primary_btn(
                            t,
                            &if n == 1 {
                                tr("Add 1 subtask").to_string()
                            } else {
                                trf!("Add {} subtasks", n)
                            },
                        )),
                    );
                    name_it(&add, "Add subtasks");
                    if add.clicked() {
                        let task = *task;
                        let chosen: Vec<String> = names
                            .iter()
                            .zip(picked.iter())
                            .filter(|(_, p)| **p)
                            .map(|(n, _)| n.clone())
                            .collect();
                        if self.ai_add_subtasks(task, chosen) {
                            close = true;
                        }
                    }
                    if secondary_button(ui, tr("Try again")).clicked() {
                        again = true;
                    }
                }
                (AiUse::Doc { .. } | AiUse::Description { .. }, AiState::Done)
                    if !text.trim().is_empty() =>
                {
                    let (how, was) = match &d.use_ {
                        AiUse::Doc { how, was, .. } | AiUse::Description { how, was, .. } => {
                            (how.clone(), was.clone())
                        }
                        _ => unreachable!(),
                    };
                    // adding below fits new text; replacing fits a new version
                    let adds = matches!(how.as_str(), "continue" | "summarize");
                    let empty = was.trim().is_empty();
                    let (first, first_name) = if empty {
                        (tr("Use this"), "Use this")
                    } else if adds {
                        (tr("Insert below"), "Insert below")
                    } else {
                        (tr("Replace"), "Replace")
                    };
                    let b = primary_button(ui, t, first);
                    name_it(&b, first_name);
                    if b.clicked() {
                        put = Some(empty || !adds);
                    }
                    if !empty {
                        let (second, second_name) = if adds {
                            (tr("Replace"), "Replace")
                        } else {
                            (tr("Insert below"), "Insert below")
                        };
                        let b = secondary_button(ui, second);
                        name_it(&b, second_name);
                        if b.clicked() {
                            put = Some(adds);
                        }
                    }
                    if secondary_button(ui, tr("Copy")).clicked() {
                        ui.ctx().copy_text(text.clone());
                        self.toast(tr("Copied"));
                    }
                    if secondary_button(ui, tr("Try again")).clicked() {
                        again = true;
                    }
                }
                _ => {
                    // failed, or nothing came
                    if primary_button(ui, t, tr("Try again")).clicked() {
                        again = true;
                    }
                    if secondary_button(ui, tr("Close")).clicked() {
                        close = true;
                    }
                }
            }
        });
        if let Some(replace) = put {
            if let Some(end) = self.ai_put_in(&d.use_, &text, replace) {
                close = true;
                // the page's cursor goes to the end of the new text, and
                // nothing is selected there any more
                if let AiUse::Doc { doc, .. } = &d.use_ {
                    let id = egui::Id::new(("doc_body", *doc));
                    super::docs::set_cursor(ui.ctx(), id, end);
                    ui.data_mut(|m| {
                        m.insert_temp(id.with("cursor"), end);
                        m.insert_temp(id.with("sel"), end);
                    });
                }
            }
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(tr(
                "AI answers can be wrong. Check them before you use them.",
            ))
            .small()
            .color(t.text3),
        );
        if again {
            if let Some(ask) = self.ai.jobs.get(&d.job).map(|j| j.ask.clone()) {
                d.job = self.ai_ask(ask);
                if let AiUse::Subtasks { names, picked, .. } = &mut d.use_ {
                    names.clear();
                    picked.clear();
                }
            }
        }
        close
    }

    /// Puts a writing answer in the page or description it was asked on:
    /// in place of what it was asked on (`replace`) or after it. Where the
    /// new text ends; None (said with a toast) when that text changed
    /// meanwhile.
    fn ai_put_in(&mut self, use_: &AiUse, answer: &str, replace: bool) -> Option<usize> {
        match use_ {
            AiUse::Doc {
                doc, range, was, ..
            } => {
                let Some(d) = self.data.docs.iter_mut().find(|d| d.id == *doc) else {
                    self.toast(tr("That page isn't there any more."));
                    return None;
                };
                let Some(at) = find_again(&d.body, *range, was) else {
                    self.toast(tr(CHANGED));
                    return None;
                };
                let (body, end) = splice_at(&d.body, at, answer, replace);
                d.body = body;
                d.updated = now_ts();
                self.dirty = true;
                Some(end)
            }
            AiUse::Description { task, was, .. } => {
                let Some(mut t) = self.data.tasks.iter().find(|t| t.id == *task).cloned() else {
                    self.toast(tr("That task isn't there any more."));
                    return None;
                };
                let Some(at) = find_again(&t.description, None, was) else {
                    self.toast(tr(CHANGED));
                    return None;
                };
                let (description, end) = splice_at(&t.description, at, answer, replace);
                t.description = description;
                self.actions.push(Action::Replace(t));
                Some(end)
            }
            _ => None,
        }
    }

    /// Adds the chosen suggestions as subtasks of `task`; false when the
    /// task is gone.
    fn ai_add_subtasks(&mut self, task: u64, names: Vec<String>) -> bool {
        let Some(list) = self
            .data
            .tasks
            .iter()
            .find(|t| t.id == task)
            .map(|t| t.list_id)
        else {
            self.toast(tr("That task isn't there any more."));
            return false;
        };
        let n = names.len();
        for name in names {
            self.actions.push(Action::Add {
                list,
                name,
                status: None,
                priority: DEFAULT_PRIORITY,
                assignees: vec![],
                due: None,
                parent: Some(task),
                open: false,
            });
        }
        self.toast(if n == 1 {
            tr("Added 1 subtask").to_string()
        } else {
            trf!("Added {} subtasks", n)
        });
        true
    }

    // ------------------------------------------------------ answers

    /// An answer with chat formatting, its citations as links; a click on
    /// one opens what it points to.
    pub(super) fn ai_rich(&mut self, ui: &mut Ui, answer: &str) {
        let data = &self.data;
        let name = |kind: &str, a: u64, b: u64| -> Option<String> {
            match kind {
                "task" => data
                    .tasks
                    .iter()
                    .find(|t| t.id == a && !t.archived)
                    .map(|t| t.name.clone()),
                "doc" => data
                    .docs
                    .iter()
                    .find(|d| d.id == a && !d.archived)
                    .map(|d| {
                        if d.title.trim().is_empty() {
                            tr("Untitled").to_string()
                        } else {
                            d.title.clone()
                        }
                    }),
                "msg" => {
                    let c = data.channels.iter().find(|c| c.id == a)?;
                    let here = c
                        .messages
                        .iter()
                        .any(|m| m.id == b || m.replies.iter().any(|r| r.id == b));
                    here.then(|| match c.kind {
                        ChannelKind::Channel => format!("#{}", c.name),
                        ChannelKind::Direct => tr("Direct message").to_string(),
                    })
                }
                _ => None,
            }
        };
        let src = linked(answer, &name);
        if let Some(url) = self.message_text(ui, &src, 14.5) {
            self.ai_open_link(&url);
        }
    }

    /// Opens a link from an answer: a message in its conversation (in its
    /// thread when it is a reply), or what a workboard:// link points to.
    fn ai_open_link(&mut self, url: &str) {
        if let Some(rest) = url.strip_prefix("workboard://ai-msg/") {
            let mut p = rest.split('/').filter_map(|x| x.parse::<u64>().ok());
            if let (Some(ch), Some(id)) = (p.next(), p.next()) {
                let thread = self
                    .data
                    .channels
                    .iter()
                    .find(|c| c.id == ch)
                    .and_then(|c| {
                        c.messages
                            .iter()
                            .find(|m| m.replies.iter().any(|r| r.id == id))
                            .map(|m| m.id)
                    });
                self.open_chat_at(ch, thread);
            }
            return;
        }
        self.open_link(url);
    }

    // ------------------------------------------------------- the AI page

    /// Asks `question` on the AI page, after the answered questions before it.
    fn ai_ask_question(&mut self, question: String) {
        let mut history: Vec<AiTurn> = self
            .ai
            .chat
            .iter()
            .filter_map(|q| {
                let j = self.ai.jobs.get(&q.job)?;
                (j.state == AiState::Done && !j.text.trim().is_empty()).then(|| AiTurn {
                    question: q.question.clone(),
                    answer: j.text.clone(),
                })
            })
            .collect();
        // the server reads only the newest few
        let n = history.len();
        history.drain(..n.saturating_sub(MAX_HISTORY));
        let job = self.ai_ask(AiAsk::Question {
            question: question.clone(),
            history,
        });
        self.ai.chat.push(AiQuestion { question, job });
    }

    /// Whether the newest question is still being answered.
    fn ai_answering(&self) -> bool {
        self.ai.chat.last().is_some_and(|q| {
            self.ai
                .jobs
                .get(&q.job)
                .is_some_and(|j| j.state == AiState::Going)
        })
    }

    /// The AI page: questions about the workspace and their answers. Before
    /// the first question it greets with the box in the middle and a few
    /// questions to start from; then the conversation fills the page and the
    /// box stays at the bottom.
    pub(super) fn ai_page(&mut self, ui: &mut Ui) {
        let t = self.t();
        let offered = self.ai_offered();
        let mut close = false;
        let mut ask: Option<String> = None;
        // a failed question asked again takes its place
        let mut retry: Option<u64> = None;
        // ---- header
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 24,
                right: 16,
                top: 12,
                bottom: 10,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(30.0);
                    ai_badge(ui, 24.0);
                    ui.add_space(2.0);
                    ui.label(RichText::new(tr("AI")).size(17.0).strong().color(t.text));
                    if let Some(m) = self.ai.model.clone().filter(|_| offered) {
                        ui.add_space(4.0);
                        egui::Frame::new()
                            .fill(t.hover)
                            .corner_radius(9)
                            .inner_margin(egui::Margin::symmetric(8, 2))
                            .show(ui, |ui| {
                                ui.label(RichText::new(m).size(11.5).color(t.text2));
                            });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        close = icon_button(ui, &t, "close", "", "Close AI", false)
                            .on_hover_text(format!("{} (Esc)", tr("Close")))
                            .clicked();
                        if !self.ai.chat.is_empty()
                            && icon_button(ui, &t, "edit", tr("New chat"), "New chat", true)
                                .clicked()
                        {
                            self.ai.chat.clear();
                        }
                    });
                });
            });
        let line = ui.available_rect_before_wrap();
        ui.painter()
            .hline(line.x_range(), line.top(), Stroke::new(1.0_f32, t.border));
        // ---- body: a centred column
        let body = ui.available_rect_before_wrap();
        ui.allocate_rect(body, Sense::hover());
        let col_w = (body.width() - 48.0).clamp(300.0, 720.0);
        let col_x = body.center().x - col_w / 2.0;
        if self.ai.chat.is_empty() {
            let mut cui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_max(
                        pos2(col_x, body.top()),
                        pos2(col_x + col_w, body.bottom()),
                    ))
                    .layout(Layout::top_down(Align::Center)),
            );
            let ui = &mut cui;
            // about a third of the way down, by last frame's height
            let group_id = egui::Id::new("ai_welcome_h");
            let group_h: f32 = ui.data(|d| d.get_temp(group_id)).unwrap_or(420.0);
            ui.add_space(((body.height() - group_h) * 0.38).max(24.0));
            let top = ui.cursor().top();
            ai_badge(ui, 56.0);
            ui.add_space(18.0);
            ui.label(
                RichText::new(if offered {
                    tr("What can I help with?")
                } else {
                    tr("AI for this workspace")
                })
                .size(26.0)
                .strong()
                .color(t.text),
            );
            ui.add_space(8.0);
            let note = self.ai_note(offered);
            ui.add(
                egui::Label::new(RichText::new(note).size(14.5).color(t.text2))
                    .wrap()
                    .halign(Align::Center),
            );
            if !self.on_server() {
                ui.add_space(12.0);
                let b = icon_button_ex(
                    ui,
                    &t,
                    "teams",
                    tr("Work together"),
                    "Work together",
                    true,
                    0.0,
                    t.text,
                    t.text2,
                );
                if b.clicked() {
                    self.modal = Some(self.together_modal());
                }
            }
            ui.add_space(24.0);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                if let Some(q) = self.ai_composer(ui, &t, offered) {
                    ask = Some(q);
                }
                if offered {
                    ui.add_space(14.0);
                    if let Some(q) = ai_suggestions(ui, &t) {
                        ask = Some(q);
                    }
                }
            });
            let h = ui.cursor().top() - top;
            ui.data_mut(|d| d.insert_temp(group_id, h));
        } else {
            // the conversation, scrolled anywhere on the page, above the box
            let foot_h = 150.0;
            let list = Rect::from_min_max(
                body.min + vec2(0.0, 4.0),
                pos2(
                    body.right(),
                    (body.bottom() - foot_h).max(body.top() + 60.0),
                ),
            );
            let mut lui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(list)
                    .layout(Layout::top_down(Align::Min)),
            );
            egui::ScrollArea::vertical()
                .id_salt("ai_chat")
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(&mut lui, |ui| {
                    ui.add_space(20.0);
                    let turns: Vec<(String, u64)> = self
                        .ai
                        .chat
                        .iter()
                        .map(|q| (q.question.clone(), q.job))
                        .collect();
                    let last = turns.len().saturating_sub(1);
                    for (i, (q, job)) in turns.into_iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add_space((col_x - list.left()).max(0.0));
                            ui.vertical(|ui| {
                                ui.set_width(col_w);
                                if let Some(again) = self.ai_turn(ui, &t, &q, job, i == last) {
                                    ask = Some(again);
                                    retry = Some(job);
                                }
                            });
                        });
                        ui.add_space(22.0);
                    }
                });
            let mut fui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(Rect::from_min_max(
                        pos2(col_x, list.bottom() + 8.0),
                        pos2(col_x + col_w, body.bottom()),
                    ))
                    .layout(Layout::top_down(Align::Min)),
            );
            if let Some(q) = self.ai_composer(&mut fui, &t, offered) {
                ask = Some(q);
            }
            if let Some(m) = self.ai.model.clone().filter(|_| offered) {
                fui.add_space(6.0);
                fui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(trf!(
                            "Answers come from {} and can be wrong. They use only what you can see.",
                            m
                        ))
                        .size(11.5)
                        .color(t.text3),
                    );
                });
            }
        }
        if let Some(q) = ask {
            if offered && !self.ai_answering() {
                if let Some(job) = retry {
                    self.ai.chat.retain(|x| x.job != job);
                }
                self.ai_ask_question(q);
            }
        }
        close |= self.modal.is_none()
            && !self.ai.window_was_open
            && self.open_task.is_none()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
            && !ui.memory(|m| m.has_focus(egui::Id::new(AI_BOX)));
        if close {
            self.section = Section::Home;
        }
    }

    /// What the AI page says under its title, by how things stand.
    fn ai_note(&self, offered: bool) -> &'static str {
        if !self.on_server() {
            tr("AI answers questions about this workspace, sums up conversations and helps with writing. It works in workspaces on a team server whose owner has turned it on.")
        } else if offered {
            tr("Ask about the tasks, docs and conversations you can see in this workspace.")
        } else if self.connected() {
            tr("AI isn't set up on this team server yet. The server's owner can turn it on with an Anthropic API key in ai.json on the server.")
        } else {
            tr("Connecting to the team server…")
        }
    }

    /// The question box: a rounded card with the text and a send button.
    /// The question to ask when it is sent (Enter, or the button).
    fn ai_composer(&mut self, ui: &mut Ui, t: &Theme, offered: bool) -> Option<String> {
        let busy = self.ai_answering();
        let box_id = egui::Id::new(AI_BOX);
        if offered {
            self.focus_on_open(ui.ctx(), "ai", 0, box_id);
        }
        let focused = ui.memory(|m| m.has_focus(box_id));
        let can = offered && !busy && !self.ai.draft.trim().is_empty();
        let enter = offered && !busy && super::chat::take_enter(ui, box_id);
        let mut send = false;
        egui::Frame::new()
            .fill(t.card)
            .stroke(if focused {
                Stroke::new(1.5_f32, AI_VIOLET)
            } else {
                Stroke::new(1.0_f32, t.border)
            })
            .corner_radius(16)
            .inner_margin(egui::Margin {
                left: 14,
                right: 10,
                top: 12,
                bottom: 10,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let r = ui.add(
                    egui::TextEdit::multiline(&mut self.ai.draft)
                        .id(box_id)
                        .frame(false)
                        .interactive(offered)
                        .hint_text(
                            RichText::new(tr("Ask anything about this workspace…")).color(t.text3),
                        )
                        .font(FontId::proportional(15.0))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                name_field(&r, "Ask AI");
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(tr("Enter to send, Shift+Enter for a new line"))
                            .size(11.5)
                            .color(t.text3),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let (r, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
                        name_it(&resp, "Ask the AI");
                        let fill = if can {
                            if resp.hovered() {
                                AI_PINK
                            } else {
                                AI_VIOLET
                            }
                        } else {
                            t.border
                        };
                        ui.painter().circle_filled(r.center(), 16.0, fill);
                        line_icon(
                            ui.painter(),
                            "send",
                            r.center(),
                            if can { Color32::WHITE } else { t.text3 },
                        );
                        if can {
                            resp.clone().on_hover_text(tr("Ask"));
                        }
                        send = resp.clicked();
                        if busy {
                            ui.add_space(4.0);
                            ui.add(egui::Spinner::new().size(14.0));
                        }
                    });
                });
            });
        if (send || enter) && can {
            let q = self.ai.draft.trim().to_string();
            self.ai.draft.clear();
            return Some(q);
        }
        None
    }

    /// One question and its answer on the AI page; a question to ask again
    /// when its "Try again" is clicked.
    fn ai_turn(&mut self, ui: &mut Ui, t: &Theme, q: &str, job: u64, last: bool) -> Option<String> {
        let mut again = None;
        let w = ui.available_width();
        // the question, in a bubble on the right
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            egui::Frame::new()
                .fill(t.accent_bg)
                .corner_radius(egui::CornerRadius {
                    nw: 16,
                    ne: 16,
                    sw: 16,
                    se: 4,
                })
                .inner_margin(egui::Margin::symmetric(14, 9))
                .show(ui, |ui| {
                    ui.set_max_width((w * 0.75).min(560.0));
                    ui.add(egui::Label::new(RichText::new(q).size(14.5).color(t.text)).wrap());
                });
        });
        ui.add_space(14.0);
        let (state, text) = match self.ai.jobs.get(&job) {
            Some(j) => (j.state.clone(), j.text.clone()),
            None => (AiState::Failed(tr(BROKE).into()), String::new()),
        };
        // the answer, beside the AI's badge
        ui.horizontal_top(|ui| {
            ai_badge(ui, 26.0);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.set_width((w - 40.0).max(100.0));
                ui.add_space(3.0);
                if state == AiState::Going && text.is_empty() {
                    thinking(ui, t);
                } else if !text.is_empty() {
                    self.ai_rich(ui, &text);
                }
                match &state {
                    AiState::Failed(why) => {
                        ui.add_space(4.0);
                        ui.label(RichText::new(why.as_str()).color(crate::widgets::WARN_ORANGE));
                        if last && self.ai_offered() {
                            ui.add_space(4.0);
                            if row_button(ui, tr("Try again")).clicked() {
                                again = Some(q.to_string());
                            }
                        }
                    }
                    AiState::Done => {
                        ui.add_space(4.0);
                        let b = icon_button_ex(
                            ui,
                            t,
                            "docs",
                            tr("Copy"),
                            &format!("Copy answer {}", job),
                            false,
                            0.0,
                            t.text3,
                            t.text3,
                        );
                        if b.clicked() {
                            ui.ctx().copy_text(text.clone());
                            self.toast(tr("Copied"));
                        }
                    }
                    AiState::Going => {}
                }
            });
        });
        again
    }
}

/// The AI page's question box (its id).
pub(super) const AI_BOX: &str = "ai_question_box";

/// The AI's own colours, a violet running into a pink.
pub(super) const AI_VIOLET: Color32 = Color32::from_rgb(112, 82, 236);
pub(super) const AI_PINK: Color32 = Color32::from_rgb(214, 70, 158);

fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// A rounded square filled with the AI's colours (top left to bottom
/// right), edges smoothed.
fn gradient_square(p: &egui::Painter, r: Rect, rad: f32) {
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0);
    let corners = [
        (r.right_top() + vec2(-rad, rad), -90.0_f32),
        (r.right_bottom() + vec2(-rad, -rad), 0.0),
        (r.left_bottom() + vec2(rad, -rad), 90.0),
        (r.left_top() + vec2(rad, rad), 180.0),
    ];
    let mut pts = vec![];
    for (c, start) in corners {
        for i in 0..=8 {
            let a = (start + 90.0 * i as f32 / 8.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    let col = |q: egui::Pos2| {
        mix(
            AI_VIOLET,
            AI_PINK,
            ((q.x - r.left()) + (q.y - r.top())) / (r.width() + r.height()),
        )
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.center(), col(r.center()));
    for q in &pts {
        mesh.colored_vertex(*q, col(*q));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(egui::Shape::mesh(mesh));
    // a thin line of the same colours over the edge smooths it
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        p.line_segment([a, b], Stroke::new(1.0_f32, col(a + (b - a) / 2.0)));
    }
}

/// The AI's badge: its colours with a white sparkle.
fn ai_badge(ui: &mut Ui, size: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    gradient_square(ui.painter(), r, size * 0.3);
    line_icon_scaled(
        ui.painter(),
        "sparkle",
        r.center(),
        Color32::WHITE,
        (size / 17.0).max(0.8),
    );
}

/// Questions to start from, as cards two to a row; the one clicked.
fn ai_suggestions(ui: &mut Ui, t: &Theme) -> Option<String> {
    const SUGGESTIONS: [(&str, &str); 4] = [
        ("calendar", "What's due this week?"),
        ("chat", "What did I miss in chat lately?"),
        ("task", "Summarize my open tasks"),
        ("alarm", "Which tasks are overdue?"),
    ];
    let mut out = None;
    let gap = 10.0;
    let w = ((ui.available_width() - gap) / 2.0).floor();
    for row in SUGGESTIONS.chunks(2) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (icon, s) in row {
                let (r, resp) = ui.allocate_exact_size(vec2(w, 56.0), Sense::click());
                name_it(&resp, s);
                let hover = resp.hovered();
                ui.painter()
                    .rect_filled(r, 12.0, if hover { t.hover } else { t.card });
                ui.painter().rect_stroke(
                    r,
                    12.0,
                    Stroke::new(1.0_f32, if hover { AI_VIOLET } else { t.border }),
                    egui::StrokeKind::Inside,
                );
                let ic = pos2(r.left() + 28.0, r.center().y);
                ui.painter().circle_filled(ic, 15.0, t.accent_bg);
                line_icon(ui.painter(), icon, ic, AI_VIOLET);
                let g = ui.painter().layout(
                    tr(s).to_string(),
                    FontId::proportional(14.0),
                    t.text,
                    w - 64.0,
                );
                ui.painter().galley(
                    pos2(r.left() + 52.0, r.center().y - g.size().y / 2.0),
                    g,
                    t.text,
                );
                if hover {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    out = Some(tr(s).to_string());
                }
            }
        });
        ui.add_space(gap - ui.spacing().item_spacing.y);
    }
    out
}

/// Three dots that pulse while an answer hasn't started.
fn thinking(ui: &mut Ui, t: &Theme) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(30.0, 18.0), Sense::hover());
        let now = ui.input(|i| i.time) as f32;
        for k in 0..3 {
            let phase = (now * 4.0 - k as f32 * 0.7).sin() * 0.5 + 0.5;
            let c = pos2(r.left() + 5.0 + k as f32 * 10.0, r.center().y - phase * 2.0);
            ui.painter()
                .circle_filled(c, 3.2, mix(t.border, AI_VIOLET, 0.35 + phase * 0.65));
        }
        ui.label(RichText::new(tr("Thinking…")).color(t.text2));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    });
}

/// A writing action in an AI menu; true when clicked.
fn ai_row(ui: &mut Ui, t: &Theme, how: &str) -> bool {
    let label = action_label(how);
    menu_row_named(ui, t, &format!("AI {}", label), tr(label), false, |p, c| {
        super::icons::line_icon(p, "sparkle", c, AI_VIOLET);
    })
    .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_goes_back_where_its_text_still_is() {
        let body = "One.\nTwo words here.\nThree.";
        // still in place
        assert_eq!(
            find_again(body, Some((5, 19)), "Two words here"),
            Some((5, 19))
        );
        // moved: something was typed above it
        let moved = format!("Zero.\n{}", body);
        assert_eq!(
            find_again(&moved, Some((5, 19)), "Two words here"),
            Some((11, 25))
        );
        // gone
        assert_eq!(
            find_again("One.\nThree.", Some((5, 19)), "Two words here"),
            None
        );
        // the same words twice: still in place is fine; moved, which one
        // was meant can't be told
        let twice = "TODO: write\nmiddle\nTODO: write";
        assert_eq!(
            find_again(twice, Some((19, 30)), "TODO: write"),
            Some((19, 30))
        );
        let moved = format!("New line on top\n{}", twice);
        assert_eq!(find_again(&moved, Some((19, 30)), "TODO: write"), None);
        assert_eq!(find_again("abc", Some((1, 1)), ""), Some((1, 1)));
        // the whole page: only while it is unchanged
        assert_eq!(find_again(body, None, body), Some((0, 27)));
        assert_eq!(find_again("changed", None, body), None);
        // Korean counts by characters, not bytes
        assert_eq!(find_again("가나다라", Some((1, 3)), "나다"), Some((1, 3)));
        assert_eq!(find_again("가나다라", Some((0, 2)), "다라"), Some((2, 4)));
    }

    #[test]
    fn answers_replace_the_text_or_go_below_it() {
        assert_eq!(splice("a b c", (2, 3), "B", true), "a B c");
        assert_eq!(splice("가나다", (1, 2), "\nX\n", true), "가X다");
        // below: a paragraph of its own, a blank line on both sides
        assert_eq!(
            splice("Intro\nBody", (0, 5), "Summary", false),
            "Intro\n\nSummary\n\nBody"
        );
        assert_eq!(
            splice("Intro\n\nBody", (0, 5), "Summary", false),
            "Intro\n\nSummary\n\nBody"
        );
        assert_eq!(splice("a\nb\nc", (2, 3), "B2", false), "a\nb\n\nB2\n\nc");
        assert_eq!(splice("Text", (0, 4), "More", false), "Text\n\nMore");
        assert_eq!(splice("Text\n", (0, 5), "More", false), "Text\n\nMore");
        assert_eq!(splice("", (0, 0), "Draft", true), "Draft");
        assert_eq!(splice("", (0, 0), "Draft", false), "Draft");
    }

    #[test]
    fn subtasks_are_read_from_the_answer() {
        assert_eq!(
            subtasks_in(r#"{"subtasks": ["Book a room", " Send   invites ", "", "Book a room"]}"#),
            Some(vec!["Book a room".to_string(), "Send invites".to_string()])
        );
        assert_eq!(
            subtasks_in("```json\n{\"subtasks\": [\"A\"]}\n```"),
            Some(vec!["A".to_string()])
        );
        assert_eq!(subtasks_in("Sure! Here are some"), None);
        assert_eq!(subtasks_in(r#"{"subtasks": "A"}"#), None);
        let many: Vec<String> = (0..30).map(|i| format!("\"T{}\"", i)).collect();
        assert_eq!(
            subtasks_in(&format!("{{\"subtasks\": [{}]}}", many.join(",")))
                .unwrap()
                .len(),
            20
        );
    }

    #[test]
    fn citations_become_links_and_headings_bold_lines() {
        let name = |kind: &str, a: u64, b: u64| -> Option<String> {
            match (kind, a, b) {
                ("task", 1, _) => Some("Design [v2]".into()),
                ("doc", 2, _) => Some("Plan".into()),
                ("msg", 3, 4) => Some("#general".into()),
                _ => None,
            }
        };
        assert_eq!(
            linked("## Status\n- Bora does it [task:1] (see [doc:2]).", &name),
            "**Status**\n- Bora does it [Design  v2](workboard://task/1) (see [Plan](workboard://doc/2))."
        );
        assert_eq!(
            linked("Said in [msg:3:4].", &name),
            "Said in [#general](workboard://ai-msg/3/4)."
        );
        // the name written before its marker is the link, not twice
        assert_eq!(
            linked("- **Plan** [doc:2] is done; \u{201C}Plan\u{201D} [doc:2]; Plan[doc:2]", &name),
            "- [Plan](workboard://doc/2) is done; [Plan](workboard://doc/2); [Plan](workboard://doc/2)"
        );
        assert_eq!(
            linked("Design [v2] [task:1]", &name),
            "[Design  v2](workboard://task/1)"
        );
        // a name ending the line above stays there
        assert_eq!(
            linked("## Plan\n[doc:2] is done", &name),
            "**Plan**\n[Plan](workboard://doc/2) is done"
        );
        // only a whole word is the name
        assert_eq!(
            linked("MyPlan [doc:2]", &name),
            "MyPlan [Plan](workboard://doc/2)"
        );
        // something not here: left out, with the space before it
        assert_eq!(linked("Gone [task:9].", &name), "Gone.");
        // not markers: left as they are
        assert_eq!(
            linked("[x] done, [task:abc], [link](https://a.b)", &name),
            "[x] done, [task:abc], [link](https://a.b)"
        );
        assert_eq!(linked("a [ b", &name), "a [ b");
        assert_eq!(linked("#hashtag stays", &name), "#hashtag stays");
        assert_eq!(
            linked("- [ ] Book\n  - [x] Room", &name),
            "\u{2610} Book\n  \u{2611} Room"
        );
    }
}
