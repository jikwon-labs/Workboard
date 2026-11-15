// Task comments in the task window: the comment cards with their checklist
// items, @mentions and workboard:// links, reactions, replies, editing,
// deleting, assigning and resolving, and the per-task unsent drafts.

use super::*;

/// Reactions offered on a comment.
const REACTIONS: [&str; 12] = [
    "\u{1F44D}",
    "\u{1F389}",
    "\u{2705}",
    "\u{1F440}",
    "\u{1F525}",
    "\u{1F64F}",
    "\u{1F604}",
    "\u{2764}",
    "\u{1F680}",
    "\u{1F914}",
    "\u{1F4AF}",
    "\u{1F44F}",
];

const UNCHECKED: &str = "\u{2610} ";
const CHECKED: &str = "\u{2611} ";

/// A piece of one comment line.
#[derive(Debug, PartialEq)]
pub(super) enum Seg<'a> {
    Text(&'a str),
    Mention(&'a str),
    Link(&'a str, LinkTo),
}

/// Where a workboard:// link points.
#[derive(Debug, PartialEq, Clone, Copy)]
pub(super) enum LinkTo {
    Task(u64),
    List(u64),
    Space(u64),
}

/// Reads `workboard://task/12` (or list/space) at the start of `s`; returns
/// the target and how many bytes the link takes.
pub(super) fn parse_link(s: &str) -> Option<(LinkTo, usize)> {
    let rest = s.strip_prefix("workboard://")?;
    for (kind, mk) in [
        ("task/", LinkTo::Task as fn(u64) -> LinkTo),
        ("list/", LinkTo::List),
        ("space/", LinkTo::Space),
    ] {
        if let Some(r) = rest.strip_prefix(kind) {
            let digits = r.bytes().take_while(|b| b.is_ascii_digit()).count();
            let n: u64 = r[..digits].parse().ok()?;
            return Some((mk(n), "workboard://".len() + kind.len() + digits));
        }
    }
    None
}

/// Splits a line into plain text, @mentions of the given people (longest
/// name first, so "@Alex Kim" wins over "@Alex") and workboard:// links.
pub(super) fn segments<'a>(line: &'a str, names: &[String]) -> Vec<Seg<'a>> {
    let mut out = vec![];
    let mut plain = 0;
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let mut hit: Option<(Seg<'a>, usize)> = None;
        if let Some((to, n)) = parse_link(rest) {
            hit = Some((Seg::Link(&line[i..i + n], to), n));
        } else if rest.starts_with('@') && (i == 0 || line[..i].ends_with(char::is_whitespace)) {
            let best = names
                .iter()
                .filter(|n| !n.is_empty() && rest[1..].starts_with(n.as_str()))
                .map(|n| n.len())
                .max();
            if let Some(n) = best {
                hit = Some((Seg::Mention(&line[i..i + 1 + n]), 1 + n));
            }
        }
        match hit {
            Some((seg, n)) => {
                if plain < i {
                    out.push(Seg::Text(&line[plain..i]));
                }
                out.push(seg);
                i += n;
                plain = i;
            }
            None => i += rest.chars().next().map_or(1, |c| c.len_utf8()),
        }
    }
    if plain < line.len() {
        out.push(Seg::Text(&line[plain..]));
    }
    out
}

/// Ticks or unticks the checklist item on line `line` of a comment.
pub(super) fn toggle_comment_check(text: &str, line: usize) -> String {
    text.split('\n')
        .enumerate()
        .map(|(i, l)| {
            if i != line {
                l.to_string()
            } else if let Some(r) = l.strip_prefix(UNCHECKED) {
                format!("{}{}", CHECKED, r)
            } else if let Some(r) = l.strip_prefix(CHECKED) {
                format!("{}{}", UNCHECKED, r)
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Adds my reaction, or takes it back.
pub(super) fn toggle_reaction(rs: &mut Vec<Reaction>, emoji: &str, me: u64) {
    if let Some(i) = rs.iter().position(|r| r.emoji == emoji) {
        let r = &mut rs[i];
        if let Some(p) = r.users.iter().position(|u| *u == me) {
            r.users.remove(p);
            if r.users.is_empty() {
                rs.remove(i);
            }
        } else {
            r.users.push(me);
        }
    } else {
        rs.push(Reaction {
            emoji: emoji.to_string(),
            users: vec![me],
        });
    }
}

/// What a click in a comment asked for; applied after the comments are drawn.
enum Act {
    Check { id: u64, line: usize },
    Link(LinkTo),
    React { id: u64, emoji: String },
    Reply(u64),
    SendReply { parent: u64, text: String },
    Edit(u64),
    SaveEdit { id: u64, text: String },
    CancelEdit,
    Delete(u64),
    Assign { id: u64, to: Option<u64> },
    Resolve { id: u64, done: bool },
}

impl App {
    /// Each task keeps its own unsent comment: put the box's text away for
    /// the task it was typed in and bring back what was typed for `id`.
    pub(super) fn swap_comment_draft(&mut self, id: u64) {
        if self.comment_for == Some(id) {
            return;
        }
        let text = std::mem::take(&mut self.comment_text);
        if let Some(prev) = self.comment_for {
            if text.trim().is_empty() {
                self.comment_drafts.remove(&prev);
            } else {
                self.comment_drafts.insert(prev, text);
            }
        }
        self.comment_text = self.comment_drafts.remove(&id).unwrap_or_default();
        self.comment_for = Some(id);
    }

    /// Goes where a workboard:// link points.
    pub(super) fn follow_link(&mut self, to: LinkTo) {
        match to {
            LinkTo::Task(id) => {
                if self.task(id).is_some() {
                    self.actions.push(Action::Open(id));
                } else {
                    self.toast = Some((tr("That task no longer exists").into(), now_ts()));
                }
            }
            LinkTo::List(id) if self.data.space_of_list(id).is_some() => {
                self.open_task = None;
                self.section = Section::Home;
                self.loc = Location::List(id);
                self.view = ViewKind::List;
            }
            LinkTo::Space(id) if self.data.spaces.iter().any(|s| s.id == id) => {
                self.open_task = None;
                self.section = Section::Home;
                self.loc = Location::Space(id);
            }
            _ => self.toast = Some((tr("That place no longer exists").into(), now_ts())),
        }
    }

    /// The task window's comments and activity lines, oldest first.
    /// `q` keeps only comments containing it; `only_comments` hides the
    /// activity lines.
    pub(super) fn comment_feed(
        &mut self,
        ui: &mut Ui,
        task: &mut Task,
        q: Option<&str>,
        only_comments: bool,
        win_id: egui::Id,
    ) {
        let t = self.t();
        let me = self.data.me;
        let edit_id = win_id.with("comment_edit");
        let reply_id = win_id.with("comment_reply");
        let mut editing: Option<(u64, String)> = ui.data(|d| d.get_temp(edit_id)).unwrap_or(None);
        let mut replying: Option<(u64, String)> = ui.data(|d| d.get_temp(reply_id)).unwrap_or(None);
        let mut names: Vec<String> = self.data.members.iter().map(|m| m.name.clone()).collect();
        names.sort_by_key(|n| std::cmp::Reverse(n.len()));
        let ids: HashSet<u64> = task
            .activity
            .iter()
            .filter(|a| a.comment)
            .map(|a| a.id)
            .collect();
        let is_reply = |a: &Activity| a.parent.is_some_and(|p| p != a.id && ids.contains(&p));
        let mut acts: Vec<Act> = vec![];
        let mut shown = 0;
        for a in task.activity.iter().filter(|a| !is_reply(a)) {
            if let Some(q) = q {
                if !a.comment || !a.text.to_lowercase().contains(q) {
                    continue;
                }
            }
            if !a.comment {
                if only_comments {
                    continue;
                }
                shown += 1;
                let who = self
                    .data
                    .member(a.author)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| tr("Someone").into());
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "\u{2022} {}",
                            crate::i18n::history_line(&who, &a.text)
                        ))
                        .small()
                        .color(t.text2),
                    );
                    ui.label(RichText::new(fmt_ago(a.ts)).small().color(t.text3));
                });
                continue;
            }
            shown += 1;
            self.comment_card(ui, &t, a, &names, &mut editing, &mut acts);
            let replies: Vec<&Activity> = task
                .activity
                .iter()
                .filter(|r| r.comment && r.parent == Some(a.id) && r.id != a.id)
                .collect();
            let reply_open = replying.as_ref().is_some_and(|(p, _)| *p == a.id);
            if !replies.is_empty() || reply_open {
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        for r in &replies {
                            self.comment_card(ui, &t, r, &names, &mut editing, &mut acts);
                        }
                        if let Some((_, text)) = replying.as_mut().filter(|(p, _)| *p == a.id) {
                            let send_key = ui.memory(|m| m.has_focus(reply_id))
                                && ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                                });
                            let r = ui.add(
                                egui::TextEdit::multiline(text)
                                    .id(reply_id)
                                    .hint_text(tr("Reply..."))
                                    .desired_rows(2)
                                    .desired_width(f32::INFINITY),
                            );
                            name_field(&r, "Reply box");
                            focus_once(ui, &r);
                            ui.horizontal(|ui| {
                                let send = primary_button(ui, &t, tr("Reply"));
                                name_it(&send, "Send reply");
                                if (send.clicked() || send_key) && !text.trim().is_empty() {
                                    acts.push(Act::SendReply {
                                        parent: a.id,
                                        text: text.trim().to_string(),
                                    });
                                }
                                if ui.button(tr("Cancel")).clicked() {
                                    acts.push(Act::Reply(0));
                                }
                            });
                        }
                    });
                });
                ui.add_space(6.0);
            }
        }
        if q.is_some() && shown == 0 {
            ui.label(RichText::new(tr("No comments match")).color(t.text3));
        }
        for act in acts {
            let find = |task: &mut Task, id: u64| task.activity.iter().position(|a| a.id == id);
            match act {
                Act::Check { id, line } => {
                    if let Some(i) = find(task, id) {
                        task.activity[i].text = toggle_comment_check(&task.activity[i].text, line);
                    }
                }
                Act::Link(to) => self.follow_link(to),
                Act::React { id, emoji } => {
                    if let Some(i) = find(task, id) {
                        toggle_reaction(&mut task.activity[i].reactions, &emoji, me);
                    }
                }
                Act::Reply(id) => replying = (id != 0).then(|| (id, String::new())),
                Act::SendReply { parent, text } => {
                    let id = self.data.new_id();
                    let mentions = self.data.mentions_in(&text);
                    task.activity.push(Activity {
                        ts: now_ts(),
                        author: me,
                        text,
                        comment: true,
                        id,
                        parent: Some(parent),
                        mentions,
                        ..Default::default()
                    });
                    replying = None;
                }
                Act::Edit(id) => {
                    editing = task
                        .activity
                        .iter()
                        .find(|a| a.id == id)
                        .map(|a| (id, a.text.clone()));
                }
                Act::SaveEdit { id, text } => {
                    if let Some(i) = find(task, id) {
                        if task.activity[i].text != text {
                            task.activity[i].mentions = crate::model::mentioned_ids(
                                &text,
                                &self.data.members,
                                &task.activity[i].mentions,
                            );
                            task.activity[i].text = text;
                            task.activity[i].edited = Some(now_ts());
                        }
                    }
                    editing = None;
                }
                Act::CancelEdit => editing = None,
                Act::Delete(id) => {
                    task.activity.retain(|a| a.id != id && a.parent != Some(id));
                    if editing.as_ref().is_some_and(|(e, _)| *e == id) {
                        editing = None;
                    }
                    self.toast = Some((tr("Comment deleted").into(), now_ts()));
                }
                Act::Assign { id, to } => {
                    if let Some(i) = find(task, id) {
                        task.activity[i].assignee = to;
                        task.activity[i].resolved = false;
                    }
                }
                Act::Resolve { id, done } => {
                    if let Some(i) = find(task, id) {
                        task.activity[i].resolved = done;
                    }
                }
            }
        }
        ui.data_mut(|d| {
            d.insert_temp(edit_id, editing);
            d.insert_temp(reply_id, replying);
        });
    }

    /// One comment (or reply) card.
    fn comment_card(
        &self,
        ui: &mut Ui,
        t: &Theme,
        a: &Activity,
        names: &[String],
        editing: &mut Option<(u64, String)>,
        acts: &mut Vec<Act>,
    ) {
        let me = self.data.me;
        let mine = a.author == me;
        let who = self
            .data
            .member(a.author)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| tr("Someone").into());
        let fill = if a.post { t.accent_bg } else { t.card };
        egui::Frame::new()
            .fill(fill)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(8)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    if let Some(m) = self.data.member(a.author) {
                        let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                        paint_avatar(ui.painter(), r.center(), 10.0, m, fill);
                    }
                    ui.label(RichText::new(&who).strong());
                    if a.post {
                        chip(
                            ui,
                            tr("Post"),
                            if t.dark { t.accent_text } else { t.primary },
                        );
                    }
                    ui.label(RichText::new(fmt_ago(a.ts)).small().color(t.text3));
                    if a.edited.is_some() {
                        ui.label(RichText::new(tr("(edited)")).small().color(t.text3));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let more = ui.add(
                            egui::Button::new(RichText::new("\u{22EF}").color(t.text2))
                                .frame(false),
                        );
                        name_it(&more, "Comment actions");
                        click_menu(&more, |ui| {
                            if a.parent.is_none() && ui.button(tr("Reply")).clicked() {
                                acts.push(Act::Reply(a.id));
                                ui.close_menu();
                            }
                            if mine && ui.button(tr("Edit")).clicked() {
                                acts.push(Act::Edit(a.id));
                                ui.close_menu();
                            }
                            ui.menu_button(tr("Assign comment"), |ui| {
                                for m in self.data.members.iter().filter(|m| !m.invited) {
                                    if ui
                                        .selectable_label(a.assignee == Some(m.id), &m.name)
                                        .clicked()
                                    {
                                        acts.push(Act::Assign {
                                            id: a.id,
                                            to: Some(m.id),
                                        });
                                        ui.close_menu();
                                    }
                                }
                                if a.assignee.is_some() && ui.button(tr("Unassign")).clicked() {
                                    acts.push(Act::Assign { id: a.id, to: None });
                                    ui.close_menu();
                                }
                            });
                            if ui
                                .button(tr(if a.resolved { "Reopen" } else { "Resolve" }))
                                .clicked()
                            {
                                acts.push(Act::Resolve {
                                    id: a.id,
                                    done: !a.resolved,
                                });
                                ui.close_menu();
                            }
                            if ui.button(tr("Copy text")).clicked() {
                                ui.ctx().copy_text(a.text.clone());
                                ui.close_menu();
                            }
                            if mine {
                                ui.separator();
                                if ui
                                    .button(RichText::new(tr("Delete comment")).color(t.danger))
                                    .clicked()
                                {
                                    acts.push(Act::Delete(a.id));
                                    ui.close_menu();
                                }
                            }
                        });
                        let react = ui.add(
                            egui::Button::new(RichText::new("\u{263A}").color(t.text2))
                                .frame(false),
                        );
                        name_it(&react, "Add reaction");
                        popup(ui, &react, ui.id().with(("react", a.id)), 180.0, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for e in REACTIONS {
                                    if ui
                                        .add(
                                            egui::Button::new(RichText::new(e).size(18.0))
                                                .frame(false),
                                        )
                                        .clicked()
                                    {
                                        acts.push(Act::React {
                                            id: a.id,
                                            emoji: e.to_string(),
                                        });
                                        close_popup(ui);
                                    }
                                }
                            });
                        });
                    });
                });
                // assigned: who, and the tick that resolves it
                if let Some(to) = a.assignee {
                    ui.horizontal(|ui| {
                        let name = self
                            .data
                            .member(to)
                            .map(|m| m.name.clone())
                            .unwrap_or_else(|| tr("someone").into());
                        let tick = ui.add(
                            egui::Button::new(if a.resolved { "\u{2611}" } else { "\u{2610}" })
                                .frame(false),
                        );
                        name_it(
                            &tick,
                            if a.resolved {
                                "Reopen comment"
                            } else {
                                "Resolve comment"
                            },
                        );
                        if tick.clicked() {
                            acts.push(Act::Resolve {
                                id: a.id,
                                done: !a.resolved,
                            });
                        }
                        let label = if a.resolved {
                            trf!("Resolved \u{00B7} assigned to {}", name)
                        } else {
                            trf!("Assigned to {}", name)
                        };
                        ui.label(RichText::new(label).small().color(if a.resolved {
                            t.text3
                        } else {
                            t.text2
                        }));
                    });
                } else if a.resolved {
                    ui.label(RichText::new(tr("Resolved")).small().color(t.text3));
                }
                match editing.as_mut().filter(|(id, _)| *id == a.id) {
                    Some((_, text)) => {
                        let edit_box = ui.id().with("comment_edit_box");
                        let save_key = ui.memory(|m| m.has_focus(edit_box))
                            && ui.input_mut(|i| {
                                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                            });
                        let r = ui.add(
                            egui::TextEdit::multiline(text)
                                .id(edit_box)
                                .desired_rows(2)
                                .desired_width(f32::INFINITY),
                        );
                        name_field(&r, "Edit comment box");
                        focus_once(ui, &r);
                        ui.horizontal(|ui| {
                            let save = primary_button(ui, t, tr("Save"));
                            name_it(&save, "Save comment");
                            if (save.clicked() || save_key) && !text.trim().is_empty() {
                                acts.push(Act::SaveEdit {
                                    id: a.id,
                                    text: text.trim().to_string(),
                                });
                            }
                            if ui.button(tr("Cancel")).clicked() {
                                acts.push(Act::CancelEdit);
                            }
                        });
                    }
                    None => self.comment_body(ui, t, a, names, acts),
                }
                if !a.reactions.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for rc in &a.reactions {
                            let on = rc.users.contains(&me);
                            let b = egui::Button::new(
                                RichText::new(format!("{} {}", rc.emoji, rc.users.len()))
                                    .size(12.0),
                            )
                            .fill(if on { t.selected_bg } else { t.hover })
                            .stroke(Stroke::new(1.0_f32, if on { t.primary } else { t.border }))
                            .corner_radius(10);
                            let who: Vec<String> = rc
                                .users
                                .iter()
                                .filter_map(|u| self.data.member(*u))
                                .map(|m| m.name.clone())
                                .collect();
                            if ui.add(b).on_hover_text(who.join(", ")).clicked() {
                                acts.push(Act::React {
                                    id: a.id,
                                    emoji: rc.emoji.clone(),
                                });
                            }
                        }
                    });
                }
            });
        ui.add_space(6.0);
    }

    /// A comment's text: checklist items tick, @mentions stand out, and
    /// workboard:// links open what they point to.
    fn comment_body(
        &self,
        ui: &mut Ui,
        t: &Theme,
        a: &Activity,
        names: &[String],
        acts: &mut Vec<Act>,
    ) {
        let (link, link_hover) = if t.dark {
            (t.accent_text, t.text)
        } else {
            (t.primary, t.accent_text)
        };
        for (li, line) in a.text.split('\n').enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let mut rest = line;
                let check = if let Some(r) = line.strip_prefix(UNCHECKED) {
                    Some((false, r))
                } else {
                    line.strip_prefix(CHECKED).map(|r| (true, r))
                };
                if let Some((done, r)) = check {
                    rest = r;
                    let b = ui.add(
                        egui::Button::new(if done { "\u{2611}" } else { "\u{2610}" }).frame(false),
                    );
                    name_it(&b, &format!("Tick {}", r.trim()));
                    if b.clicked() {
                        acts.push(Act::Check { id: a.id, line: li });
                    }
                    ui.add_space(4.0);
                }
                if rest.is_empty() {
                    ui.label(" ");
                }
                for seg in segments(rest, names) {
                    match seg {
                        Seg::Text(s) => {
                            let mut rt = RichText::new(s).color(t.text);
                            if check.is_some_and(|(done, _)| done) {
                                rt = rt.strikethrough().color(t.text3);
                            }
                            ui.add(egui::Label::new(rt).wrap());
                        }
                        Seg::Mention(s) => {
                            ui.label(
                                RichText::new(s)
                                    .color(if t.dark { t.accent_text } else { t.primary })
                                    .background_color(t.selected_bg)
                                    .strong(),
                            );
                        }
                        Seg::Link(s, to) => {
                            let r = ui.add(
                                egui::Label::new(RichText::new(s).color(link).underline())
                                    .sense(Sense::click()),
                            );
                            let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
                            name_it(&r, s);
                            if r.hovered() {
                                ui.painter().hline(
                                    r.rect.x_range(),
                                    r.rect.bottom(),
                                    Stroke::new(1.0_f32, link_hover),
                                );
                            }
                            if r.clicked() {
                                acts.push(Act::Link(to));
                            }
                        }
                    }
                }
            });
        }
    }
}
