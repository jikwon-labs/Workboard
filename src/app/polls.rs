// Polls in chat: "/poll" opens a window to write a question and 2 to 10
// choices (one or several may be picked, votes shown or anonymous), and
// Create poll posts it. The message's text is the question with its choices
// numbered, which is what apps from before polls show; `Message::poll` is
// what this app draws: a row per choice with its votes and a bar, picked
// with a click (again to take it back). Each person's votes merge on their
// own (`Message::votes`, wb-sync "votes/<member>"). Its author can close it.

use super::chat::ChatAction;
use super::icons::{icon_button, line_icon};
use super::*;
use crate::model::{poll_text, Poll, POLL_MAX_OPTIONS, POLL_MIN_OPTIONS};

/// The poll being written in the New poll window.
#[derive(Clone, Debug, Default)]
pub(super) struct PollDraft {
    pub ch: u64,
    pub parent: Option<u64>,
    pub question: String,
    pub options: Vec<String>,
    pub multi: bool,
    pub anonymous: bool,
}

impl PollDraft {
    pub(super) fn new(ch: u64, parent: Option<u64>, question: &str) -> PollDraft {
        PollDraft {
            ch,
            parent,
            question: question.trim().to_string(),
            options: vec![String::new(); POLL_MIN_OPTIONS],
            ..Default::default()
        }
    }

    /// The choices with words in them.
    fn filled(&self) -> Vec<String> {
        self.options
            .iter()
            .map(|o| o.trim().to_string())
            .filter(|o| !o.is_empty())
            .collect()
    }

    fn ready(&self) -> bool {
        !self.question.trim().is_empty() && self.filled().len() >= POLL_MIN_OPTIONS
    }
}

impl App {
    /// "/poll [question]" in conversation `ch` (thread `parent`).
    pub(super) fn open_new_poll(&mut self, ch: u64, parent: Option<u64>, question: &str) {
        self.modal = Some(Modal::NewPoll(PollDraft::new(ch, parent, question)));
    }

    /// The New poll window. Returns true when it should close.
    pub(super) fn new_poll_body(&mut self, ui: &mut Ui, t: &Theme, d: &mut PollDraft) -> bool {
        field_label(ui, t, tr("Question"));
        let r = ui.add(
            field(&mut d.question)
                .hint_text(tr("Ask something..."))
                .desired_width(f32::INFINITY),
        );
        name_it(&r, "Poll question");
        ui.add_space(8.0);
        field_label(ui, t, tr("Choices"));
        let mut remove = None;
        let many = d.options.len() > POLL_MIN_OPTIONS;
        for (i, o) in d.options.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let w = ui.available_width() - if many { 32.0 } else { 0.0 };
                let r = ui.add(
                    field(o)
                        .hint_text(trf!("Choice {}", i + 1))
                        .desired_width(w),
                );
                name_it(&r, &format!("Choice {}", i + 1));
                if many
                    && icon_button(
                        ui,
                        t,
                        Icon::Close,
                        "",
                        &format!("Remove choice {}", i + 1),
                        false,
                    )
                    .on_hover_text(tr("Remove choice"))
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.options.remove(i);
        }
        if d.options.len() < POLL_MAX_OPTIONS {
            let r = ui.add(
                egui::Label::new(RichText::new(tr("+ Add a choice")).color(t.primary))
                    .sense(Sense::click()),
            );
            name_it(&r, "Add a choice");
            if r.clicked() {
                d.options.push(String::new());
            }
        }
        ui.add_space(8.0);
        check(ui, &mut d.multi, tr("Allow more than one choice"));
        check(ui, &mut d.anonymous, tr("Anonymous votes"));
        let (create, cancel) = dialog_footer(ui, t, tr("Create poll"), d.ready());
        if create && d.ready() {
            self.post_poll(d.clone());
            return true;
        }
        cancel
    }

    /// Post the poll written in the window.
    pub(super) fn post_poll(&mut self, d: PollDraft) {
        let options = d.filled();
        let question = d.question.trim().to_string();
        let text = poll_text(&question, &options);
        self.chat.poll_to_send = Some(Poll {
            question,
            options,
            multi: d.multi,
            anonymous: d.anonymous,
            closed: None,
        });
        self.send_chat(d.ch, text, d.parent, vec![]);
        self.chat.poll_to_send = None;
    }

    /// Pick or unpick choice `option` of the poll in message `msg`.
    pub(super) fn vote(&mut self, ch: u64, msg: u64, option: u32) {
        let me = self.data.me;
        let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) else {
            return;
        };
        if let Some(m) = super::chat::find_msg(&mut c.messages, msg) {
            if m.toggle_vote(me, option) {
                self.dirty = true;
            }
        }
    }

    /// Its author closes the poll (or opens it again).
    pub(super) fn close_poll(&mut self, ch: u64, msg: u64, close: bool) {
        let me = self.data.me;
        let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) else {
            return;
        };
        let Some(m) = super::chat::find_msg(&mut c.messages, msg) else {
            return;
        };
        if m.author != me {
            return;
        }
        if let Some(p) = m.poll.as_mut() {
            p.closed = close.then(now_ts);
            self.dirty = true;
        }
    }

    /// A poll in a message: the question, a row per choice (my picks
    /// marked, votes and a bar), who voted when it isn't anonymous, and
    /// for its author Close poll.
    pub(super) fn poll_card(&self, ui: &mut Ui, cid: u64, m: &Message, acts: &mut Vec<ChatAction>) {
        let Some(p) = &m.poll else { return };
        let t = self.t();
        let me = self.data.me;
        let counts = m.poll_counts();
        let voters = m.votes.len();
        let most = counts.iter().copied().max().unwrap_or(0);
        let mine: &[u32] = m.votes.get(&me).map(|v| v.as_slice()).unwrap_or(&[]);
        let closed = p.closed.is_some();
        let w = ui.available_width().min(420.0);
        egui::Frame::new()
            .fill(t.card)
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(10)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(w - 24.0);
                ui.horizontal_top(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 20.0), Sense::hover());
                    line_icon(ui.painter(), Icon::Chart, r.center(), t.primary);
                    ui.add(egui::Label::new(RichText::new(&p.question).strong().size(15.0)).wrap());
                });
                ui.label(
                    RichText::new(if p.multi {
                        tr("Pick one or more")
                    } else {
                        tr("Pick one")
                    })
                    .small()
                    .color(t.text3),
                );
                ui.add_space(4.0);
                for (i, o) in p.options.iter().enumerate() {
                    let picked = mine.contains(&(i as u32));
                    let n = counts.get(i).copied().unwrap_or(0);
                    let row_w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(
                        vec2(row_w, 34.0),
                        if closed {
                            Sense::hover()
                        } else {
                            Sense::click()
                        },
                    );
                    let label = format!("Vote {}", o);
                    resp.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Checkbox,
                            !closed,
                            picked,
                            label.clone(),
                        )
                    });
                    if ui.is_rect_visible(rect) {
                        let painter = ui.painter_at(rect);
                        let fill = if resp.hovered() && !closed {
                            t.hover
                        } else {
                            t.bg
                        };
                        painter.rect_filled(rect, 6.0, fill);
                        // the bar: this choice's share of all votes
                        let total: usize = counts.iter().sum();
                        if total > 0 && n > 0 {
                            let mut bar = rect;
                            bar.set_width(rect.width() * n as f32 / total as f32);
                            let c = if n == most && closed {
                                t.primary.gamma_multiply(0.35)
                            } else {
                                t.primary.gamma_multiply(0.18)
                            };
                            painter.rect_filled(bar, 6.0, c);
                        }
                        painter.rect_stroke(
                            rect,
                            6.0,
                            Stroke::new(
                                if picked { 1.5_f32 } else { 1.0 },
                                if picked { t.primary } else { t.border },
                            ),
                            egui::StrokeKind::Inside,
                        );
                        // a round mark for one choice, a square for several
                        let mark = rect.left_center() + vec2(16.0, 0.0);
                        let stroke = Stroke::new(1.5_f32, if picked { t.primary } else { t.text3 });
                        if p.multi {
                            let r = Rect::from_center_size(mark, vec2(12.0, 12.0));
                            if picked {
                                painter.rect_filled(r, 3.0, t.primary);
                            } else {
                                painter.rect_stroke(r, 3.0, stroke, egui::StrokeKind::Inside);
                            }
                        } else if picked {
                            painter.circle_filled(mark, 6.0, t.primary);
                        } else {
                            painter.circle_stroke(mark, 6.0, stroke);
                        }
                        let count = n.to_string();
                        let room = ((rect.width() - 80.0) / 7.0).max(4.0) as usize;
                        painter.text(
                            mark + vec2(14.0, 0.0),
                            Align2::LEFT_CENTER,
                            crate::widgets::truncate(o, room),
                            FontId::proportional(14.0),
                            t.text,
                        );
                        painter.text(
                            rect.right_center() - vec2(12.0, 0.0),
                            Align2::RIGHT_CENTER,
                            count,
                            FontId::proportional(13.0),
                            t.text2,
                        );
                    }
                    // who picked it, on hover, when the votes are shown
                    let resp = if !p.anonymous && n > 0 {
                        let names: Vec<String> = m
                            .votes
                            .iter()
                            .filter(|(_, v)| v.contains(&(i as u32)))
                            .map(|(who, _)| self.who(*who))
                            .collect();
                        resp.on_hover_text(names.join(", "))
                    } else {
                        resp
                    };
                    if resp.clicked() && !closed {
                        acts.push(ChatAction::Vote {
                            ch: cid,
                            msg: m.id,
                            option: i as u32,
                        });
                    }
                    ui.add_space(2.0);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let mut status = if voters == 1 {
                        tr("1 person voted").to_string()
                    } else {
                        trf!("{} people voted", voters)
                    };
                    if p.anonymous {
                        status = format!("{} · {}", status, tr("Anonymous"));
                    }
                    if closed {
                        status = format!("{} · {}", status, tr("Poll closed"));
                    }
                    ui.label(RichText::new(status).small().color(t.text3));
                    if m.author == me {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (label, name) = if closed {
                                ("Reopen poll", "Reopen poll")
                            } else {
                                ("Close poll", "Close poll")
                            };
                            let b = ui.add(
                                egui::Label::new(RichText::new(tr(label)).small().color(t.primary))
                                    .sense(Sense::click()),
                            );
                            name_it(&b, name);
                            if b.clicked() {
                                acts.push(ChatAction::ClosePoll {
                                    ch: cid,
                                    msg: m.id,
                                    close: !closed,
                                });
                            }
                        });
                    }
                });
            });
    }
}
