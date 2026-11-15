// Chat: Channels and Direct Messages in a list on the left (the original's
// sidebar sections "Channels" and "Direct Messages"), the message feed with
// reactions, saved messages and "Reply in thread", a thread panel on the
// right, and turning a message into a task. Everything stays on this computer.

use super::*;

const QUICK_REACTIONS: [&str; 4] = ["\u{1F44D}", "\u{2705}", "\u{1F440}", "\u{1F389}"];

/// The emoji grid behind "Add reaction" and the message box's emoji button.
pub(super) const EMOJIS: [&str; 40] = [
    "\u{1F44D}",
    "\u{1F44E}",
    "\u{2705}",
    "\u{1F440}",
    "\u{1F389}",
    "\u{2764}",
    "\u{1F525}",
    "\u{1F64F}",
    "\u{1F604}",
    "\u{1F602}",
    "\u{1F60D}",
    "\u{1F914}",
    "\u{1F62E}",
    "\u{1F622}",
    "\u{1F621}",
    "\u{1F60E}",
    "\u{1F44F}",
    "\u{1F64C}",
    "\u{1F4AA}",
    "\u{1F44B}",
    "\u{1F91D}",
    "\u{1F449}",
    "\u{270C}",
    "\u{1F44C}",
    "\u{1F680}",
    "\u{1F4AF}",
    "\u{2B50}",
    "\u{1F4A1}",
    "\u{1F4CC}",
    "\u{1F4C5}",
    "\u{23F0}",
    "\u{1F6A7}",
    "\u{2757}",
    "\u{2753}",
    "\u{274C}",
    "\u{2714}",
    "\u{1F3AF}",
    "\u{1F381}",
    "\u{2615}",
    "\u{1F355}",
];

/// A grid of emoji buttons; returns the one clicked.
fn emoji_grid(ui: &mut Ui) -> Option<&'static str> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        for e in EMOJIS {
            let b = ui.add(ghost(
                egui::Button::new(RichText::new(e).size(18.0)).min_size(vec2(28.0, 28.0)),
            ));
            if b.clicked() {
                picked = Some(e);
            }
        }
    });
    picked
}

pub(super) enum ChatAction {
    Open(u64),
    JoinHuddle(u64),
    React {
        ch: u64,
        msg: u64,
        emoji: String,
    },
    Thread(u64),
    CloseThread,
    Save {
        ch: u64,
        msg: u64,
    },
    Task {
        ch: u64,
        msg: u64,
    },
    Send {
        ch: u64,
        text: String,
        parent: Option<u64>,
        files: Vec<Attachment>,
    },
    NewChannel(String),
    Dm(u64),
    SendPost {
        ch: u64,
        title: String,
        text: String,
        files: Vec<Attachment>,
    },
    Pin {
        ch: u64,
        msg: u64,
    },
    /// Open a message's picture or video large.
    OpenMedia {
        ch: u64,
        msg: u64,
        index: usize,
    },
    /// Start changing one of my messages in place.
    Edit {
        ch: u64,
        msg: u64,
    },
    SaveEdit {
        ch: u64,
        msg: u64,
        text: String,
    },
    CancelEdit,
    Delete {
        ch: u64,
        msg: u64,
    },
    /// Show the channel as unread from this message on.
    MarkUnread {
        ch: u64,
        ts: i64,
    },
    /// Remind me about a message at this time (None: pick a time).
    Remind {
        ch: u64,
        msg: u64,
        at: Option<i64>,
    },
    /// The text went to the clipboard.
    Copied,
}

/// The newest message or reply in a channel (0 when it has none).
pub(super) fn newest_ts(c: &Channel) -> i64 {
    c.messages
        .iter()
        .flat_map(|m| std::iter::once(m.ts).chain(m.replies.iter().map(|r| r.ts)))
        .max()
        .unwrap_or(0)
}

/// Takes a press of Enter (without Shift, Cmd or Option) meant for the text
/// box `id` before the box sees it, so sending doesn't also put a line break
/// where the cursor is. True when there was one.
fn take_enter(ui: &Ui, id: egui::Id) -> bool {
    if !ui.memory(|m| m.has_focus(id)) {
        return false;
    }
    ui.input_mut(|i| {
        let before = i.events.len();
        i.events.retain(|e| !matches!(e, egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift && !modifiers.command && !modifiers.alt));
        i.events.len() != before
    })
}

/// Whether `text` mentions `name`: "@" and the whole name, not the start
/// of a longer one ("@Bo" doesn't mention Bob).
pub(super) fn mentions(text: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let at = format!("@{}", name);
    text.match_indices(&at)
        .any(|(i, _)| name_ends(&text[i + at.len()..]))
}

/// What follows a name in a mention: the end, a space or punctuation, not
/// more letters of a longer name.
fn name_ends(rest: &str) -> bool {
    rest.chars()
        .next()
        .map_or(true, |c| !(c.is_alphanumeric() || c == '_'))
}

/// The people in a direct message, sorted, each once.
fn dm_people(c: &Channel) -> Vec<u64> {
    let mut v = c.members.clone();
    v.sort_unstable();
    v.dedup();
    v
}

/// Which copy of a direct message shows: one with messages, then the older.
fn dm_rank(c: &Channel) -> (bool, u64) {
    (c.messages.is_empty(), c.id)
}

fn find_msg(msgs: &mut [Message], id: u64) -> Option<&mut Message> {
    for m in msgs.iter_mut() {
        if m.id == id {
            return Some(m);
        }
        if let Some(r) = m.replies.iter_mut().find(|r| r.id == id) {
            return Some(r);
        }
    }
    None
}

impl App {
    pub(super) fn channel_label(&self, c: &Channel) -> String {
        match c.kind {
            ChannelKind::Channel => format!("# {}", c.name),
            ChannelKind::Direct => c
                .members
                .iter()
                .find(|m| **m != self.data.me)
                .and_then(|m| self.data.member(*m))
                .map(|m| m.name.clone())
                .unwrap_or_else(|| tr("Just you").into()),
        }
    }

    /// Messages from others I haven't seen in a conversation (with its
    /// duplicate direct messages). Seen ids decide where there are some, so a
    /// message that arrives late or with its author's clock behind still
    /// counts; before any were kept, the time last read does.
    pub(super) fn unread(&self, c: &Channel) -> usize {
        self.conversation(c)
            .iter()
            .map(|d| self.unread_here(d))
            .sum()
    }

    fn unread_here(&self, c: &Channel) -> usize {
        let seen = self.data.extra.chat_seen.get(&c.id);
        c.messages
            .iter()
            .filter(|m| {
                m.author != self.data.me
                    && match seen {
                        Some(s) => !s.has(m.id) && (m.ts >= s.floor || m.ts > c.last_read),
                        None => m.ts > c.last_read,
                    }
            })
            .count()
    }

    /// Something in an open conversation still to be marked read.
    fn read_mark_due(&self, c: &Channel) -> bool {
        self.conversation(c).iter().any(|d| {
            self.unread_here(d) > 0
                || d.messages.iter().map(|m| m.ts).max().unwrap_or(0) > d.last_read
                || !self.data.extra.chat_seen.contains_key(&d.id)
        })
    }

    /// Mark a conversation (and its duplicate direct messages) read: up to
    /// the newest message even when its clock is ahead of ours, and every
    /// message in it seen.
    pub(super) fn mark_read(&mut self, cid: u64) {
        let Some(c) = self.data.channels.iter().find(|c| c.id == cid) else {
            return;
        };
        let ids: Vec<u64> = self.conversation(c).iter().map(|d| d.id).collect();
        let now = now_ts();
        for id in ids {
            if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == id) {
                let newest = c.messages.iter().map(|m| m.ts).max().unwrap_or(0);
                c.last_read = now.max(newest);
                self.data
                    .extra
                    .chat_seen
                    .insert(id, SeenIds::of(c.messages.iter().map(|m| (m.id, m.ts))));
            }
        }
    }

    /// A direct message's copies with the same people (see `dm_duplicate`),
    /// the one shown first; just `c` for anything else.
    pub(super) fn conversation<'a>(&'a self, c: &'a Channel) -> Vec<&'a Channel> {
        if c.kind != ChannelKind::Direct || self.dm_duplicate(c) {
            return vec![c];
        }
        let mine = dm_people(c);
        std::iter::once(c)
            .chain(
                self.data.channels.iter().filter(|d| {
                    d.id != c.id && d.kind == ChannelKind::Direct && dm_people(d) == mine
                }),
            )
            .collect()
    }

    /// The copy of a direct message that shows (itself for anything else).
    pub(super) fn dm_primary(&self, c: &Channel) -> u64 {
        if c.kind != ChannelKind::Direct {
            return c.id;
        }
        let mine = dm_people(c);
        self.data
            .channels
            .iter()
            .filter(|d| d.kind == ChannelKind::Direct && dm_people(d) == mine)
            .min_by_key(|d| dm_rank(d))
            .map_or(c.id, |d| d.id)
    }

    /// Which copy of a conversation holds a message or reply: `ch`, or one of
    /// its duplicate direct messages (shown together in `ch`).
    fn msg_home(&self, ch: u64, msg: u64) -> u64 {
        let Some(c) = self.data.channels.iter().find(|c| c.id == ch) else {
            return ch;
        };
        self.conversation(c)
            .iter()
            .find(|d| {
                d.messages
                    .iter()
                    .any(|m| m.id == msg || m.replies.iter().any(|r| r.id == msg))
            })
            .map_or(ch, |d| d.id)
    }

    /// Whether I may see a channel: a public channel is everyone's, a private
    /// one its members', a direct message the people in it.
    pub(super) fn can_see(&self, c: &Channel) -> bool {
        match c.kind {
            ChannelKind::Channel => !c.private || c.members.contains(&self.data.me),
            ChannelKind::Direct => c.members.contains(&self.data.me),
        }
    }

    /// Whether I may delete a channel and change who else is in it: whoever
    /// made it (any member for one made before that was kept), or an owner
    /// or admin of the team server.
    pub(super) fn may_manage_channel(&self, c: &Channel) -> bool {
        let me = self.data.me;
        self.sync_rt.role.is_some_and(|r| r.manages())
            || c.creator == me
            || (c.creator == 0 && c.members.contains(&me))
    }

    /// Whether a channel shows in my lists: one I may see and, for a direct
    /// message, not one I closed (until something newer arrives) or a second
    /// copy of a conversation with the same people.
    pub(super) fn listed(&self, c: &Channel) -> bool {
        if !self.can_see(c) {
            return false;
        }
        if c.kind != ChannelKind::Direct {
            return true;
        }
        if self.dm_duplicate(c) {
            return false;
        }
        if let Some((_, at)) = self
            .data
            .extra
            .closed_dms
            .iter()
            .find(|(id, _)| *id == c.id)
        {
            // back once a message comes that wasn't there when I closed it
            let all: Vec<(u64, i64)> = self
                .conversation(c)
                .iter()
                .flat_map(|d| {
                    d.messages
                        .iter()
                        .flat_map(|m| std::iter::once(m).chain(m.replies.iter()))
                })
                .map(|m| (m.id, m.ts))
                .collect();
            let fresh = match self.data.extra.closed_dm_seen.get(&c.id) {
                Some(s) => all
                    .iter()
                    .any(|(id, ts)| !s.has(*id) && (*ts >= s.floor || *ts > *at)),
                None => all.iter().any(|(_, ts)| *ts > *at),
            };
            if !fresh {
                return false;
            }
        }
        true
    }

    /// Two people who start a conversation with each other at the same moment
    /// make two. One shows, with the messages of both: one with messages
    /// before an empty one, else the older. Nothing is moved; new messages go
    /// to the one that shows.
    pub(super) fn dm_duplicate(&self, c: &Channel) -> bool {
        if c.kind != ChannelKind::Direct {
            return false;
        }
        let mine = dm_people(c);
        self.data.channels.iter().any(|d| {
            d.id != c.id
                && d.kind == ChannelKind::Direct
                && dm_people(d) == mine
                && dm_rank(d) < dm_rank(c)
        })
    }

    /// "Close conversation": the direct message leaves my sidebar until a
    /// new message comes in. Nobody else's copy changes.
    pub(super) fn close_dm(&mut self, cid: u64) {
        let Some(c) = self.data.channels.iter().find(|c| c.id == cid) else {
            return;
        };
        let at = now_ts().max(newest_ts(c));
        let seen = SeenIds::of(
            self.conversation(c)
                .iter()
                .flat_map(|d| {
                    d.messages
                        .iter()
                        .flat_map(|m| std::iter::once(m).chain(m.replies.iter()))
                })
                .map(|m| (m.id, m.ts)),
        );
        self.data.extra.closed_dms.retain(|(id, _)| *id != cid);
        self.data.extra.closed_dms.push((cid, at));
        self.data.extra.closed_dm_seen.insert(cid, seen);
        if self.chat_open == Some(cid) {
            self.chat_open = None;
            self.thread_open = None;
        }
        self.dirty = true;
    }

    /// Message text with mentions marked: "@" and a member's name (names
    /// may have spaces), else "@" and the word after it.
    pub(super) fn mention_text(&self, text: &str, size: f32) -> egui::text::LayoutJob {
        let t = self.t();
        let mut job = egui::text::LayoutJob::default();
        let plain = egui::TextFormat {
            font_id: FontId::proportional(size),
            color: t.text,
            ..Default::default()
        };
        let marked = egui::TextFormat {
            font_id: FontId::proportional(size),
            color: t.selected_text,
            background: t.selected_bg,
            ..Default::default()
        };
        let mut names: Vec<&str> = self
            .data
            .members
            .iter()
            .map(|m| m.name.as_str())
            .filter(|n| !n.is_empty())
            .collect();
        names.sort_by_key(|n| std::cmp::Reverse(n.len()));
        let (mut from, mut i) = (0, 0);
        while i < text.len() {
            let rest = &text[i..];
            if rest.starts_with('@') && (i == 0 || text[..i].ends_with(char::is_whitespace)) {
                let after = &rest[1..];
                let len = names
                    .iter()
                    .find(|n| after.starts_with(**n) && name_ends(&after[n.len()..]))
                    .map(|n| n.len())
                    .or_else(|| {
                        Some(after.find(char::is_whitespace).unwrap_or(after.len()))
                            .filter(|w| *w > 0)
                    });
                if let Some(len) = len {
                    if from < i {
                        job.append(&text[from..i], 0.0, plain.clone());
                    }
                    job.append(&text[i..i + 1 + len], 0.0, marked.clone());
                    i += 1 + len;
                    from = i;
                    continue;
                }
            }
            i += rest.chars().next().map_or(1, char::len_utf8);
        }
        if from < text.len() || text.is_empty() {
            job.append(&text[from..], 0.0, plain);
        }
        job.wrap.max_width = f32::INFINITY;
        job
    }

    pub(super) fn chat(&mut self, ui: &mut Ui) {
        let t = self.t();
        // nothing open, or (from a search, a saved place, a link) a channel I
        // may not see: the first one in my list instead
        if self.chat_open.map_or(true, |id| {
            self.data
                .channels
                .iter()
                .any(|c| c.id == id && !self.can_see(c))
        }) {
            self.chat_open = self
                .data
                .channels
                .iter()
                .find(|c| self.listed(c))
                .map(|c| c.id);
            self.thread_open = None;
        }
        let mut acts: Vec<ChatAction> = vec![];
        let Some(cid) = self.chat_open else {
            // empty state: no channel or DM yet
            let mut make = false;
            ui.vertical_centered(|ui| {
                ui.add_space((ui.available_height() * 0.28).max(40.0));
                let (r, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
                ui.painter().rect_filled(r, 14.0, t.accent_bg);
                super::shell::line_icon(ui.painter(), "chat", r.center(), t.accent_text);
                ui.add_space(14.0);
                ui.label(RichText::new(tr("Chat with your team")).size(22.0).color(t.text));
                ui.add_space(4.0);
                ui.label(RichText::new(tr("Channels keep conversations about a topic in one place. Direct messages are for one-on-one talks and calls.")).color(t.text2));
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    let w = 170.0 * 2.0 + 150.0 + 16.0;
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    let b = ui.add(Tint(egui::Button::new(RichText::new(tr("#  Create a Channel")).color(Color32::WHITE)).fill(t.primary).corner_radius(7).min_size(vec2(170.0, 34.0))));
                    name_it(&b, "Create a Channel");
                    if b.clicked() {
                        make = true;
                    }
                    if ui.add(egui::Button::new(tr("\u{1F464}  Message someone")).stroke(Stroke::new(1.0, t.border)).corner_radius(7).min_size(vec2(170.0, 34.0))).clicked() {
                        self.new_message_open = true;
                    }
                    if ui.add(egui::Button::new(tr("\u{1F465}  Work together")).stroke(Stroke::new(1.0, t.border)).corner_radius(7).min_size(vec2(150.0, 34.0))).on_hover_text(tr("Share this workspace so others can chat with you")).clicked() {
                        self.modal = Some(self.together_modal());
                    }
                });
            });
            if make {
                self.apply_chat(vec![ChatAction::NewChannel("general".into())]);
            }
            return;
        };
        let Some(mut channel) = self.data.channels.iter().find(|c| c.id == cid).cloned() else {
            self.chat_open = None;
            return;
        };
        // a second copy of a direct message opens as the one that shows
        let primary = self.dm_primary(&channel);
        if primary != cid {
            self.chat_open = Some(primary);
            self.thread_open = None;
            return;
        }
        // with the messages of its duplicates, in time order
        let others: Vec<Message> = self
            .conversation(&channel)
            .iter()
            .skip(1)
            .flat_map(|d| d.messages.iter().cloned())
            .collect();
        if !others.is_empty() {
            channel.messages.extend(others);
            channel.messages.sort_by_key(|m| m.ts);
        }
        if self.chat_keep_unread.is_some_and(|k| k != cid) {
            self.chat_keep_unread = None;
        }
        // a closed conversation that's opened again is back in the sidebar
        if self.data.extra.closed_dms.iter().any(|(id, _)| *id == cid) {
            self.data.extra.closed_dms.retain(|(id, _)| *id != cid);
            self.data.extra.closed_dm_seen.remove(&cid);
            self.dirty = true;
        }

        // ---- thread panel
        let mut thread_rect: Option<Rect> = None;
        if let Some(tid) = self.thread_open {
            if let Some(parent) = channel.messages.iter().find(|m| m.id == tid).cloned() {
                let panel = egui::SidePanel::right("thread")
                    .exact_width(360.0)
                    .resizable(false)
                    .frame(
                        egui::Frame::new()
                            .fill(t.column_bg)
                            .inner_margin(egui::Margin::symmetric(14, 10)),
                    )
                    .show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("Thread")).size(15.0).strong());
                            ui.label(RichText::new(self.channel_label(&channel)).color(t.text3));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(ghost(egui::Button::new("\u{2716}"))).clicked() {
                                    acts.push(ChatAction::CloseThread);
                                }
                            });
                        });
                        ui.separator();
                        // one row of tiles when pictures wait in the box
                        let composer_h = 96.0
                            + if self.thread_files.is_empty() {
                                0.0
                            } else {
                                76.0
                            };
                        egui::ScrollArea::vertical()
                            .id_salt("thread_scroll")
                            .max_height(ui.available_height() - composer_h)
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                self.message_row(ui, cid, &parent, false, &mut acts);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(if parent.replies.len() == 1 {
                                            tr("1 reply").to_string()
                                        } else {
                                            trf!("{} replies", parent.replies.len())
                                        })
                                        .small()
                                        .color(t.text3),
                                    );
                                    let r = ui.available_rect_before_wrap();
                                    ui.painter().hline(
                                        r.x_range(),
                                        r.center().y,
                                        Stroke::new(1.0, t.border),
                                    );
                                });
                                for r in &parent.replies {
                                    self.message_row(ui, cid, r, false, &mut acts);
                                }
                            });
                        if let Some((text, files)) = self.composer(ui, tr("Reply..."), true) {
                            acts.push(ChatAction::Send {
                                ch: cid,
                                text,
                                parent: Some(tid),
                                files,
                            });
                        }
                    });
                thread_rect = Some(panel.response.rect);
            }
        }
        // pictures and videos dragged in from Finder
        self.take_dropped(ui.ctx(), thread_rect);
        let dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());

        // ---- conversation (header with tabs, side panel, messages, composer)
        let ws_name = self.data.workspace.clone();
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 12, top: 8, bottom: 0 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(30.0);
                    let title = match channel.kind {
                        ChannelKind::Channel if channel.name == "general" => ws_name.clone(),
                        ChannelKind::Channel if channel.private => format!("\u{1F512} {}", channel.name),
                        ChannelKind::Channel => format!("# {}", channel.name),
                        ChannelKind::Direct => self.channel_label(&channel),
                    };
                    if channel.kind == ChannelKind::Direct {
                        if let Some(m) = channel.members.iter().find(|m| **m != self.data.me).or(channel.members.first()).and_then(|m| self.data.member(*m)) {
                            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                            paint_avatar(ui.painter(), r.center(), 10.0, m, t.bg);
                        }
                    }
                    ui.label(RichText::new(title).size(15.5).color(t.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let hb = |ui: &mut Ui, text: &str| ui.add(ghost(egui::Button::new(RichText::new(tr(text)).size(14.0).color(t.text2))));
                        if channel.kind == ChannelKind::Channel {
                            // how this channel notifies me (notify.rs)
                            let mode = self.data.extra.notify.channel_mode(cid).to_string();
                            let bell = ui.add(ghost(egui::Button::new(RichText::new(if mode == "nothing" { "\u{1F515}" } else { "\u{1F514}" }).size(14.0).color(t.text2))));
                            name_it(&bell, "Channel notifications");
                            let bell = bell.on_hover_text(tr("Notifications for this channel"));
                            popup(ui, &bell, ui.id().with(("chan_notify", cid)), 240.0, |ui| {
                                ui.label(RichText::new(tr("Notify me about")).small().color(t.text2));
                                for (m, label) in [("all", "Every new message"), ("mentions", "Only @mentions and my threads"), ("nothing", "Nothing")] {
                                    if ui.radio(mode == m, tr(label)).clicked() {
                                        self.data.extra.notify.set_channel_mode(cid, m);
                                        self.dirty = true;
                                        close_popup(ui);
                                    }
                                }
                            });
                        }
                        if hb(ui, "\u{1F4DD}  Ask").clicked() {
                            self.section = Section::Ai;
                        }
                        if hb(ui, "\u{2699}  Integrate").clicked() {
                            self.modal = Some(super::menus::unavailable("Integrations"));
                        }
                        if hb(ui, "\u{260E}").on_hover_text(tr("Start a SyncUp")).clicked() {
                            self.open_call_dialog();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    let g = ui.painter().layout_no_wrap(tr("Channel").into(), FontId::proportional(14.0), t.text);
                    let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 34.0, 34.0), Sense::hover());
                    let ic = Rect::from_center_size(pos2(r.left() + 13.0, r.center().y), vec2(15.0, 15.0));
                    ui.painter().rect_filled(ic, 3.0, Color32::from_rgb(0x5f, 0x55, 0xee));
                    ui.painter().text(ic.center(), Align2::CENTER_CENTER, "#", FontId::proportional(10.0), Color32::WHITE);
                    ui.painter().galley(pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0), g.clone(), t.text);
                    ui.painter().galley(pos2(r.left() + 26.3, r.center().y - g.size().y / 2.0), g, t.text);
                    ui.painter().rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 2.0), r.max), 1.0, t.text);
                    for (label, v) in [("List", ViewKind::List), ("Board", ViewKind::Board), ("Calendar", ViewKind::Calendar)] {
                        if ui.add(ghost(egui::Button::new(RichText::new(tr(label)).size(14.0).color(t.text2)).min_size(vec2(0.0, 30.0)))).clicked() {
                            self.view = v;
                            // a channel made with "Add a List" opens its own List
                            let to = channel.list.filter(|l| self.data.location_exists(Location::List(*l))).map(Location::List).unwrap_or(Location::Everything);
                            self.actions.push(Action::Go(to));
                        }
                    }
                    let vr = ui.add(ghost(egui::Button::new(RichText::new(tr("+  View")).size(14.0).color(t.text2)).min_size(vec2(0.0, 30.0))));
                    popup(ui, &vr, ui.id().with(("chan_view", cid)), 180.0, |ui| {
                        for (label, v) in [("List", ViewKind::List), ("Board", ViewKind::Board), ("Calendar", ViewKind::Calendar), ("Gantt", ViewKind::Gantt), ("Table", ViewKind::Table)] {
                            if ui.button(tr(label)).clicked() {
                                self.view = v;
                                self.actions.push(Action::Go(Location::Everything));
                                close_popup(ui);
                            }
                        }
                    });
                });
            });
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
            // right panel
            egui::SidePanel::right(egui::Id::new(("chan_panel", cid))).exact_width(230.0).resizable(false).show_separator_line(false).frame(egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 12, top: 12, bottom: 8 })).show_inside(ui, |ui| {
                egui::Frame::new().stroke(Stroke::new(1.0, t.border)).corner_radius(10).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let rows: [(&str, &str, u8); 7] = [("\u{1F464}", "Followers", 10), ("\u{1F50D}", "Find in Channel", 11), ("\u{21A9}", "Replies", 2), ("\u{1F4CC}", "Pinned Messages", 1), ("\u{1F516}", "Saved", 4), ("\u{2611}", "Turned into tasks", 3), ("\u{2699}", "Settings", 12)];
                    for (icon, label, code) in rows {
                        let w = ui.available_width();
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                        let sel = code < 10 && self.channel_panel == code;
                        if sel {
                            ui.painter().rect_filled(r, 6.0, t.row_selected);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, t.hover);
                        }
                        if code == 10 {
                            paint_avatars(ui.painter(), r.left_center() + vec2(4.0, 0.0), 8.0, &channel.members.iter().take(1).copied().collect::<Vec<_>>(), &self.data, t.bg);
                        } else {
                            ui.painter().text(r.left_center() + vec2(12.0, 0.0), Align2::CENTER_CENTER, icon, FontId::proportional(13.0), t.text2);
                        }
                        ui.painter().text(r.left_center() + vec2(28.0, 0.0), Align2::LEFT_CENTER, tr(label), FontId::proportional(14.0), t.text);
                        if code == 10 {
                            ui.painter().text(r.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, channel.members.len().to_string(), FontId::proportional(12.0), t.text3);
                        }
                        if resp.clicked() {
                            match code {
                                10 | 12 => self.modal = Some(Modal::ChannelSettings(cid)),
                                11 => self.channel_find = if self.channel_find.is_some() { None } else { Some(String::new()) },
                                c => self.channel_panel = if self.channel_panel == c { 0 } else { c },
                            }
                        }
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                if ui.add(egui::Button::new(RichText::new(tr("Add bookmarks, notes, and more")).size(13.5).color(t.text2)).corner_radius(14)).clicked() {
                    self.modal = Some(Modal::Bookmark { title: String::new(), url: String::new() });
                }
                if let Some(mut q) = self.channel_find.take() {
                    ui.add(egui::TextEdit::singleline(&mut q).hint_text(tr("\u{1F50D} Find in channel")).desired_width(200.0));
                    self.channel_find = Some(q);
                }
                if self.channel_panel != 0 {
                    let label = match self.channel_panel {
                        1 => tr("Pinned messages"),
                        2 => tr("Messages with replies"),
                        4 => tr("Saved messages"),
                        _ => tr("Messages turned into tasks"),
                    };
                    if ui.add(egui::Button::new(format!("{}  \u{2716}", label)).corner_radius(14)).clicked() {
                        self.channel_panel = 0;
                    }
                }
            });
            let composer_h = 140.0 + if self.chat_files.is_empty() { 0.0 } else { 76.0 };
            let q = self.channel_find.clone().unwrap_or_default().to_lowercase();
            let panel = self.channel_panel;
            let shown: Vec<Message> = channel
                .messages
                .iter()
                .filter(|m| q.is_empty() || m.text.to_lowercase().contains(&q) || m.post_title.as_ref().map(|t| t.to_lowercase().contains(&q)).unwrap_or(false))
                .filter(|m| match panel {
                    1 => m.pinned,
                    2 => !m.replies.is_empty(),
                    3 => m.task.is_some(),
                    4 => m.saved || m.replies.iter().any(|r| r.saved),
                    _ => true,
                })
                .cloned()
                .collect();
            egui::ScrollArea::vertical().id_salt(("chat_scroll", cid)).max_height(ui.available_height() - composer_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 10)).show(ui, |ui| {
                    if channel.messages.is_empty() {
                        // empty state, as in the original
                        ui.add_space(50.0);
                        let w = ui.available_width();
                        let cw = 400.0f32.min(w - 40.0);
                        let pad = ((w - cw) / 2.0).max(0.0);
                        let name = if channel.kind == ChannelKind::Channel { format!("#{}", if channel.name == "general" { tr("General").to_string() } else { channel.name.clone() }) } else { self.channel_label(&channel) };
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_width(cw);
                                ui.label(RichText::new(trf!("Chat in {}", name)).size(16.0).color(t.text));
                                ui.label(RichText::new(tr("Collaborate seamlessly across tasks and conversations. Start chatting with your team or connect tasks to stay on top of your work.")).color(t.text2));
                                ui.add_space(10.0);
                                if ui.add(egui::Button::new(tr("+  Add People")).min_size(vec2(cw, 32.0))).clicked() {
                                    self.modal = Some(Modal::ChannelSettings(cid));
                                }
                                if ui.add(egui::Button::new(tr("\u{2728}  Import from Slack")).min_size(vec2(cw, 32.0))).clicked() {
                                    self.modal = Some(super::menus::unavailable("Import from Slack"));
                                }
                                ui.add_space(8.0);
                                ui.separator();
                                for (bg, icon, title, sub) in [([0xb6, 0x60, 0xe0], "\u{2630}", "Track Tasks", "Manage tasks, bugs, people, and more"), ([0x10, 0x90, 0xe0], "\u{1F4C4}", "Add Doc", "Take notes or create detailed documents"), ([0x00, 0x88, 0x44], "\u{260E}", "Start SyncUp", "Jump on a voice call or video call")] {
                                    let (r, resp) = ui.allocate_exact_size(vec2(cw, 54.0), Sense::click());
                                    ui.painter().rect_filled(r, 8.0, rgb(bg).gamma_multiply(if resp.hovered() { 0.14 } else { 0.07 }));
                                    let ib = Rect::from_center_size(r.left_center() + vec2(26.0, 0.0), vec2(34.0, 34.0));
                                    ui.painter().rect_filled(ib, 8.0, Color32::WHITE);
                                    ui.painter().text(ib.center(), Align2::CENTER_CENTER, icon, FontId::proportional(15.0), rgb(bg));
                                    ui.painter().text(r.left_center() + vec2(54.0, -9.0), Align2::LEFT_CENTER, tr(title), FontId::proportional(14.5), t.text);
                                    ui.painter().text(r.left_center() + vec2(54.0, 10.0), Align2::LEFT_CENTER, tr(sub), FontId::proportional(12.5), t.text2);
                                    if resp.clicked() {
                                        match title {
                                            "Track Tasks" => self.actions.push(Action::Go(Location::Everything)),
                                            "Add Doc" => self.new_doc(None),
                                            _ => self.open_call_dialog(),
                                        }
                                    }
                                    ui.add_space(6.0);
                                }
                            });
                        });
                    }
                    let mut last_day: Option<NaiveDate> = None;
                    let mut prev: Option<(u64, i64)> = None;
                    for m in &shown {
                        let d = chrono::DateTime::from_timestamp(m.ts, 0).map(|x| x.with_timezone(&chrono::Local).date_naive()).unwrap_or_else(today);
                        if last_day != Some(d) {
                            last_day = Some(d);
                            ui.add_space(6.0);
                            let w = ui.available_width();
                            let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
                            ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.border));
                            let label = match (d - today()).num_days() {
                                0 => tr("Today").to_string(),
                                -1 => tr("Yesterday").to_string(),
                                _ => d.text("%A, %B %-d"),
                            };
                            let g = ui.painter().layout_no_wrap(label, FontId::proportional(11.5), t.text2);
                            let pill = Rect::from_center_size(r.center(), g.size() + vec2(20.0, 8.0));
                            ui.painter().rect_filled(pill, 11.0, t.bg);
                            ui.painter().rect_stroke(pill, 11.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                            ui.painter().galley(pill.center() - g.size() / 2.0, g, t.text2);
                            prev = None;
                        }
                        let compact = prev.map_or(false, |(a, ts)| a == m.author && m.ts - ts < 300) && m.post_title.is_none() && m.call.is_none();
                        self.message_row_c(ui, cid, m, true, compact, &mut acts);
                        prev = Some((m.author, m.ts));
                    }
                });
            });
            egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 12, top: 4, bottom: 10 }).show(ui, |ui| {
                let target = match channel.kind {
                    ChannelKind::Channel => if channel.name == "general" { tr("General").to_string() } else { channel.name.clone() },
                    ChannelKind::Direct => self.channel_label(&channel),
                };
                if let Some((text, title, files)) = self.channel_composer(ui, &target) {
                    match title {
                        Some(tt) => acts.push(ChatAction::SendPost { ch: cid, title: tt, text, files }),
                        None => acts.push(ChatAction::Send { ch: cid, text, parent: None, files }),
                    }
                }
            });
        });
        if dragging {
            let r = ui.max_rect();
            let p = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drop_hint"),
            ));
            p.rect_filled(r.shrink(8.0), 12.0, t.primary.gamma_multiply(0.12));
            p.rect_stroke(
                r.shrink(8.0),
                12.0,
                Stroke::new(2.0, t.primary),
                egui::StrokeKind::Inside,
            );
            p.text(
                r.center(),
                Align2::CENTER_CENTER,
                tr("Drop pictures or videos to send"),
                FontId::proportional(18.0),
                t.text,
            );
        }

        // mark as read (unless the person just marked it unread and is still here)
        if self.chat_keep_unread == Some(cid) {
            self.apply_chat(acts);
            return;
        }
        if self
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .is_some_and(|c| self.read_mark_due(c))
        {
            self.mark_read(cid);
            if self.persist {
                self.dirty = true;
            }
        }
        self.apply_chat(acts);
    }

    /// Channel message box with Message / Post / Ask tabs. Returns (text, post
    /// title, attachments).
    fn channel_composer(
        &mut self,
        ui: &mut Ui,
        target: &str,
    ) -> Option<(String, Option<String>, Vec<Attachment>)> {
        let t = self.t();
        let mut out = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (i, (icon, label)) in [
                ("\u{1F4AC}", "Message"),
                ("\u{1F4E2}", "Post"),
                ("\u{2728}", "Ask"),
            ]
            .iter()
            .enumerate()
            {
                let sel = self.composer_mode == i as u8;
                let b = Tint(
                    egui::Button::new(
                        RichText::new(format!("{} {}", icon, tr(label)))
                            .size(13.5)
                            .color(if sel { t.text } else { t.text2 }),
                    )
                    .fill(if sel { t.bg } else { Color32::TRANSPARENT })
                    .stroke(if sel {
                        Stroke::new(1.0, t.border)
                    } else {
                        Stroke::NONE
                    })
                    .corner_radius(6),
                );
                if ui.add(b).clicked() {
                    if i == 2 {
                        self.section = Section::Ai;
                    } else {
                        self.composer_mode = i as u8;
                    }
                }
            }
        });
        let text_id = egui::Id::new(super::files::CHAT_BOX);
        let bg_id = egui::Id::new("chat_composer_bg");
        let frame_resp = egui::Frame::new()
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .fill(t.card)
            .show(ui, |ui| {
                // a click anywhere on the box (not on its buttons) puts the cursor in the message
                let last: Option<Rect> = ui.ctx().data(|d| d.get_temp(bg_id));
                if let Some(r) = last {
                    let bg = ui.interact(r, bg_id, Sense::click());
                    if bg.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                    }
                    if bg.clicked() {
                        ui.memory_mut(|m| m.request_focus(text_id));
                    }
                }
                self.pending_strip(ui, false);
                if self.composer_mode == 1 {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.post_title)
                            .hint_text(tr("Title"))
                            .font(FontId::proportional(15.0))
                            .frame(false)
                            .desired_width(f32::INFINITY),
                    );
                }
                let hint = if self.composer_mode == 1 {
                    tr("Share an announcement...").to_string()
                } else {
                    trf!(
                        "Write to {}, press 'space' for AI, '/' for commands",
                        target
                    )
                };
                let enter = take_enter(ui, text_id);
                let r = ui.add(
                    egui::TextEdit::multiline(&mut self.chat_text)
                        .id(text_id)
                        .hint_text(hint)
                        .frame(false)
                        .desired_rows(1)
                        .desired_width(f32::INFINITY),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Message composer")
                });
                // typing "@" at the start of a word offers the people to mention
                let mention_popup = egui::Id::new("chat_mention_popup");
                if r.changed() && self.chat_text.ends_with('@') {
                    let before = self.chat_text[..self.chat_text.len() - 1].chars().last();
                    if before.map_or(true, char::is_whitespace) {
                        ui.memory_mut(|m| m.open_popup(mention_popup));
                    }
                }
                // Up in an empty box changes my last message here
                if r.has_focus()
                    && self.chat_text.is_empty()
                    && self.composer_mode == 0
                    && ui.input(|i| i.key_pressed(egui::Key::ArrowUp))
                {
                    let me = self.data.me;
                    let last = self
                        .chat_open
                        .and_then(|cid| self.data.channels.iter().find(|c| c.id == cid))
                        .and_then(|c| {
                            c.messages
                                .iter()
                                .rev()
                                .find(|m| {
                                    m.author == me && m.call.is_none() && m.post_title.is_none()
                                })
                                .map(|m| (c.id, m.id, m.text.clone()))
                        });
                    if last.is_some() {
                        self.chat_edit = last;
                    }
                }
                let mut insert: Option<String> = None;
                ui.horizontal(|ui| {
                    let tool = |ui: &mut Ui, g: &str, tip: &str| {
                        let b = ui
                            .add(ghost(egui::Button::new(RichText::new(g).color(t.text2))))
                            .on_hover_text(tr(tip));
                        name_it(&b, tip);
                        b
                    };
                    if tool(ui, "+", "Add pictures or videos").clicked() {
                        self.pick_media(false);
                    }
                    let at = tool(ui, "@", "Mention someone");
                    let me = self.data.me;
                    let people: Vec<Member> = self
                        .data
                        .members
                        .iter()
                        .filter(|m| !m.invited && m.id != me)
                        .cloned()
                        .collect();
                    popup(ui, &at, mention_popup, 210.0, |ui| {
                        if people.is_empty() {
                            ui.label(RichText::new(tr("No one else here yet")).color(t.text3));
                        }
                        for m in &people {
                            if menu_row(ui, &t, &m.name, false, |p, c| {
                                paint_avatar(p, c, 8.0, m, t.card)
                            })
                            .clicked()
                            {
                                insert = Some(format!("@{} ", m.name));
                                close_popup(ui);
                            }
                        }
                    });
                    let em = tool(ui, "\u{263A}", "Emoji");
                    popup(ui, &em, egui::Id::new("chat_emoji_popup"), 248.0, |ui| {
                        if let Some(e) = emoji_grid(ui) {
                            insert = Some(e.to_string());
                            close_popup(ui);
                        }
                    });
                    if tool(ui, "\u{2611}", "Create task").clicked() {
                        self.open_quick_add(None);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let label = if self.composer_mode == 1 {
                            tr("Post")
                        } else {
                            "\u{27A4}"
                        };
                        let (files_ok, files) = self.pending_ready(false);
                        let ready = files_ok
                            && (!self.chat_text.trim().is_empty()
                                || !files.is_empty()
                                || (self.composer_mode == 1 && !self.post_title.trim().is_empty()));
                        let b = Tint(
                            egui::Button::new(RichText::new(label).color(if ready {
                                Color32::WHITE
                            } else {
                                t.text3
                            }))
                            .fill(if ready { t.primary } else { t.hover })
                            .corner_radius(6)
                            .min_size(vec2(34.0, 28.0)),
                        );
                        if (ui.add(b).clicked() || enter) && ready {
                            let text = self.chat_text.trim().to_string();
                            let title = if self.composer_mode == 1 {
                                Some(if self.post_title.trim().is_empty() {
                                    "Update".to_string()
                                } else {
                                    self.post_title.trim().to_string()
                                })
                            } else {
                                None
                            };
                            out = Some((text, title, files));
                            self.chat_text.clear();
                            self.post_title.clear();
                            self.chat_files.clear();
                        } else if enter && files_ok && self.chat_files.is_empty() {
                            self.chat_text.clear();
                        }
                    });
                });
                if let Some(text) = insert {
                    // a picked @name replaces an "@" already typed
                    if text.starts_with('@') && self.chat_text.ends_with('@') {
                        self.chat_text.pop();
                    }
                    self.chat_text.push_str(&text);
                    ui.memory_mut(|m| m.request_focus(text_id));
                    let n = self.chat_text.chars().count();
                    let mut st = egui::TextEdit::load_state(ui.ctx(), text_id).unwrap_or_default();
                    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(n),
                    )));
                    st.store(ui.ctx(), text_id);
                }
            });
        ui.ctx()
            .data_mut(|d| d.insert_temp(bg_id, frame_resp.response.rect));
        out
    }

    /// The thread's message box; returns the text and attachments when Enter
    /// (without Shift) or Send is pressed.
    fn composer(
        &mut self,
        ui: &mut Ui,
        hint: &str,
        thread: bool,
    ) -> Option<(String, Vec<Attachment>)> {
        let t = self.t();
        let mut out = None;
        egui::Frame::new()
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(10)
            .inner_margin(egui::Margin::same(8))
            .fill(t.card)
            .show(ui, |ui| {
                self.pending_strip(ui, thread);
                let (files_ok, files) = self.pending_ready(thread);
                let has_files = !(if thread {
                    &self.thread_files
                } else {
                    &self.chat_files
                })
                .is_empty();
                let id = egui::Id::new(if thread {
                    super::files::THREAD_BOX
                } else {
                    super::files::CHAT_BOX
                });
                let text = if thread {
                    &mut self.thread_text
                } else {
                    &mut self.chat_text
                };
                let enter = take_enter(ui, id);
                let r = ui.add(
                    egui::TextEdit::multiline(text)
                        .id(id)
                        .hint_text(hint)
                        .frame(false)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                r.widget_info(move || {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        true,
                        if thread {
                            "Reply composer"
                        } else {
                            "Message composer"
                        },
                    )
                });
                let mut pick = false;
                ui.horizontal(|ui| {
                    let b = ui
                        .add(ghost(egui::Button::new(RichText::new("+").color(t.text2))))
                        .on_hover_text(tr("Add pictures or videos"));
                    b.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            true,
                            "Add pictures or videos to the reply",
                        )
                    });
                    pick = b.clicked();
                    ui.label(
                        RichText::new(tr("Enter to send, Shift+Enter for a new line"))
                            .small()
                            .color(t.text3),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let ready = files_ok && (!text.trim().is_empty() || !files.is_empty());
                        if (primary_button(ui, &t, tr("Send")).clicked() || enter) && ready {
                            out = Some((text.trim().to_string(), files));
                            text.clear();
                        } else if enter && !has_files {
                            text.clear();
                        }
                    });
                });
                if out.is_some() {
                    if thread {
                        self.thread_files.clear();
                    } else {
                        self.chat_files.clear();
                    }
                }
                if pick {
                    self.pick_media(thread);
                }
            });
        out
    }

    fn message_row(
        &mut self,
        ui: &mut Ui,
        cid: u64,
        m: &Message,
        top_level: bool,
        acts: &mut Vec<ChatAction>,
    ) {
        self.message_row_c(ui, cid, m, top_level, false, acts)
    }

    /// `compact`: a follow-up from the same person a moment later, shown
    /// without the avatar and name (as the original groups messages).
    fn message_row_c(
        &mut self,
        ui: &mut Ui,
        cid: u64,
        m: &Message,
        top_level: bool,
        compact: bool,
        acts: &mut Vec<ChatAction>,
    ) {
        let t = self.t();
        let me = self.data.me;
        let editing = matches!(&self.chat_edit, Some((c, id, _)) if *c == cid && *id == m.id);
        let w = ui.available_width();
        let start = ui.cursor().min;
        // the hover background goes underneath the message, not over it
        let bg = ui.painter().add(egui::Shape::Noop);
        let inner = ui.horizontal_top(|ui| {
            ui.set_width(w);
            let (ar, _) = ui.allocate_exact_size(
                vec2(36.0, if compact { 18.0 } else { 36.0 }),
                Sense::hover(),
            );
            if compact {
                if ui.rect_contains_pointer(
                    ar.expand2(vec2(0.0, 6.0))
                        .union(Rect::from_min_size(ar.min, vec2(w, 22.0))),
                ) {
                    let time = chrono::DateTime::from_timestamp(m.ts, 0)
                        .map(|x| x.with_timezone(&chrono::Local).text("%-I:%M"))
                        .unwrap_or_default();
                    ui.painter().text(
                        ar.center(),
                        Align2::CENTER_CENTER,
                        time,
                        FontId::proportional(10.5),
                        t.text3,
                    );
                }
            } else if let Some(au) = self.data.member(m.author) {
                paint_avatar(ui.painter(), ar.center(), 15.0, au, t.bg);
            }
            ui.vertical(|ui| {
                ui.set_width(w - 50.0);
                if !compact {
                    ui.horizontal(|ui| {
                        let name = self
                            .data
                            .member(m.author)
                            .map(|x| x.name.clone())
                            .unwrap_or_else(|| tr("Someone").into());
                        ui.label(RichText::new(name).strong());
                        let time = chrono::DateTime::from_timestamp(m.ts, 0)
                            .map(|x| x.with_timezone(&chrono::Local).text("%-I:%M %p"))
                            .unwrap_or_default();
                        ui.label(RichText::new(time).small().color(t.text3));
                        if m.edited.is_some() {
                            ui.label(RichText::new(tr("(edited)")).small().color(t.text3));
                        }
                        if m.saved {
                            ui.label(
                                RichText::new(tr("\u{1F516} Saved"))
                                    .small()
                                    .color(Color32::from_rgb(0xe1, 0x6b, 0x16)),
                            );
                        }
                        if m.pinned {
                            ui.label(RichText::new(tr("\u{1F4CC} Pinned")).small().color(t.text3));
                        }
                    });
                }
                if let Some(pt) = &m.post_title {
                    ui.label(
                        RichText::new(format!("\u{1F4E2} {}", pt))
                            .size(16.0)
                            .color(t.text),
                    );
                }
                if editing {
                    self.edit_box(ui, cid, m.id, acts);
                } else if !m.text.is_empty() {
                    let job = self.mention_text(&m.text, 14.0);
                    if let Some(link) = self.text_with_links(ui, job, &m.text) {
                        self.link_pending = Some(link);
                    }
                    if compact && m.edited.is_some() {
                        ui.label(RichText::new(tr("(edited)")).small().color(t.text3));
                    }
                }
                if let Some(index) = self.attachment_tiles(ui, m) {
                    acts.push(ChatAction::OpenMedia {
                        ch: cid,
                        msg: m.id,
                        index,
                    });
                }
                if let Some(ch) = m.call.as_deref().and_then(super::huddle::huddle_channel) {
                    let here = self.huddle.channel == Some(ch);
                    egui::Frame::new()
                        .fill(Color32::from_rgb(0x00, 0x88, 0x44).gamma_multiply(0.08))
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                                ui.painter().circle_filled(
                                    r.center(),
                                    15.0,
                                    Color32::from_rgb(0x00, 0x88, 0x44),
                                );
                                super::shell::line_icon(
                                    ui.painter(),
                                    "phone",
                                    r.center(),
                                    Color32::WHITE,
                                );
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(tr("Huddle")).strong());
                                    ui.label(
                                        RichText::new(if here {
                                            tr("You're in it")
                                        } else {
                                            tr("Voice call in Workboard")
                                        })
                                        .small()
                                        .color(t.text3),
                                    );
                                });
                                ui.add_space(16.0);
                                if !here {
                                    let join = ui.add(Tint(
                                        egui::Button::new(
                                            RichText::new(tr("Join")).color(Color32::WHITE),
                                        )
                                        .fill(Color32::from_rgb(0x00, 0x88, 0x44))
                                        .corner_radius(6)
                                        .min_size(vec2(64.0, 28.0)),
                                    ));
                                    name_it(&join, "Join huddle");
                                    if join.clicked() {
                                        acts.push(ChatAction::JoinHuddle(ch));
                                    }
                                }
                            });
                        });
                } else if let Some(link) = &m.call {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(0x00, 0x88, 0x44).gamma_multiply(0.08))
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
                                ui.painter().circle_filled(
                                    r.center(),
                                    15.0,
                                    Color32::from_rgb(0x00, 0x88, 0x44),
                                );
                                super::shell::line_icon(
                                    ui.painter(),
                                    "phone",
                                    r.center(),
                                    Color32::WHITE,
                                );
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(tr("Call")).strong());
                                    ui.label(
                                        RichText::new(truncate(link, 46)).small().color(t.text3),
                                    );
                                });
                                ui.add_space(16.0);
                                let join = ui.add(Tint(
                                    egui::Button::new(
                                        RichText::new(tr("Join")).color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(0x00, 0x88, 0x44))
                                    .corner_radius(6)
                                    .min_size(vec2(64.0, 28.0)),
                                ));
                                name_it(&join, "Join call");
                                if join.clicked() {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(link));
                                }
                                if ui.button(tr("Copy link")).clicked() {
                                    ui.ctx().copy_text(link.clone());
                                }
                            });
                        });
                }
                if let Some(tid) = m.task {
                    if let Some(task) = self.task(tid) {
                        let st = self.data.status_of(task);
                        let r = egui::Frame::new()
                            .stroke(Stroke::new(1.0, t.border))
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                    paint_status_circle(ui.painter(), r.center(), 5.5, &st);
                                    ui.label(RichText::new(&task.name).size(12.5));
                                    ui.label(
                                        RichText::new(self.data.list_name(task.list_id))
                                            .small()
                                            .color(t.text3),
                                    );
                                });
                            });
                        let card = ui.interact(
                            r.response.rect,
                            ui.id().with(("msgtask", m.id)),
                            Sense::click(),
                        );
                        hover_bg(ui, &card, card.rect, 8.0);
                        if card.clicked() {
                            // open the linked task through the main action queue
                            acts.push(ChatAction::Task {
                                ch: u64::MAX,
                                msg: tid,
                            });
                        }
                    }
                }
                if !m.reactions.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        for rc in &m.reactions {
                            let mine = rc.users.contains(&self.data.me);
                            let txt = format!("{} {}", rc.emoji, rc.users.len());
                            let b = Tint(
                                egui::Button::new(RichText::new(txt).size(12.0))
                                    .fill(if mine { t.selected_bg } else { t.hover })
                                    .stroke(Stroke::new(
                                        1.0,
                                        if mine { t.primary } else { t.border },
                                    ))
                                    .corner_radius(10),
                            );
                            let names: Vec<String> = rc
                                .users
                                .iter()
                                .filter_map(|u| self.data.member(*u))
                                .map(|x| x.name.clone())
                                .collect();
                            if ui.add(b).on_hover_text(names.join(", ")).clicked() {
                                acts.push(ChatAction::React {
                                    ch: cid,
                                    msg: m.id,
                                    emoji: rc.emoji.clone(),
                                });
                            }
                        }
                    });
                }
                if top_level && !m.replies.is_empty() {
                    let last = m.replies.last().map(|r| fmt_ago(r.ts)).unwrap_or_default();
                    ui.horizontal(|ui| {
                        paint_avatars(
                            ui.painter(),
                            ui.cursor().left_center() + vec2(0.0, 9.0),
                            8.0,
                            &m.replies.iter().map(|r| r.author).collect::<Vec<_>>(),
                            &self.data,
                            t.bg,
                        );
                        ui.add_space(30.0);
                        let label = if m.replies.len() == 1 {
                            "1 reply".to_string()
                        } else {
                            format!("{} replies", m.replies.len())
                        };
                        let shown = if m.replies.len() == 1 {
                            tr("1 reply").to_string()
                        } else {
                            trf!("{} replies", m.replies.len())
                        };
                        // a link color that reads on both themes (dark primary is a dim grey)
                        let (link, link_hover) = if t.dark {
                            (t.accent_text, t.text)
                        } else {
                            (t.primary, t.accent_text)
                        };
                        let g =
                            ui.painter()
                                .layout_no_wrap(shown, FontId::proportional(13.0), link);
                        let (lr, lk) =
                            ui.allocate_exact_size(g.size() + vec2(4.0, 4.0), Sense::click());
                        name_it(&lk, &label);
                        let col = if lk.hovered() { link_hover } else { link };
                        ui.painter().galley(lr.min + vec2(2.0, 2.0), g, col);
                        if lk.hovered() {
                            ui.painter().hline(
                                lr.x_range().shrink(2.0),
                                lr.bottom() - 1.0,
                                Stroke::new(1.0, col),
                            );
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if lk.clicked() {
                            acts.push(ChatAction::Thread(m.id));
                        }
                        ui.label(
                            RichText::new(trf!("Last reply {}", last))
                                .small()
                                .color(t.text3),
                        );
                    });
                }
            });
        });
        let rect = Rect::from_min_max(
            start,
            pos2(
                start.x + w,
                inner.response.rect.bottom() + if compact { 1.0 } else { 4.0 },
            ),
        );
        ui.add_space(if compact { 2.0 } else { 8.0 });
        // hover toolbar; it stays while one of its menus is open
        let more_id = egui::Id::new(("msg_more", cid, m.id, top_level));
        let emoji_id = egui::Id::new(("msg_emoji", cid, m.id, top_level));
        let menu_open = ui.memory(|mm| mm.is_popup_open(more_id) || mm.is_popup_open(emoji_id));
        if !editing && (ui.rect_contains_pointer(rect) || menu_open) {
            ui.painter().set(
                bg,
                egui::Shape::rect_filled(rect.expand2(vec2(6.0, 2.0)), 6.0, t.hover),
            );
            let n = QUICK_REACTIONS.len() + 2 + if top_level { 4 } else { 1 };
            let bw = 30.0;
            let bar = Rect::from_min_size(
                pos2(rect.right() - bw * n as f32 - 8.0, rect.top() - 10.0),
                vec2(bw * n as f32 + 4.0, 28.0),
            );
            ui.painter().rect_filled(bar, 7.0, t.card);
            ui.painter().rect_stroke(
                bar,
                7.0,
                Stroke::new(1.0, t.border),
                egui::StrokeKind::Inside,
            );
            let mut x = bar.left() + 2.0;
            let mut button = |ui: &mut Ui, label: &str, tip: &str| -> egui::Response {
                let r = Rect::from_min_size(pos2(x, bar.top() + 2.0), vec2(bw, 24.0));
                x += bw;
                let b = put_over(ui, r, ghost(egui::Button::new(label)));
                name_it(&b, tip);
                b.on_hover_text(tr(tip))
            };
            for e in QUICK_REACTIONS {
                let b = button(ui, e, "Add reaction");
                // (named by its emoji, so a test can pick one)
                name_it(&b, e);
                if b.clicked() {
                    acts.push(ChatAction::React {
                        ch: cid,
                        msg: m.id,
                        emoji: e.to_string(),
                    });
                }
            }
            let eb = button(ui, "\u{263A}", "More reactions");
            popup(ui, &eb, emoji_id, 248.0, |ui| {
                if let Some(e) = emoji_grid(ui) {
                    acts.push(ChatAction::React {
                        ch: cid,
                        msg: m.id,
                        emoji: e.to_string(),
                    });
                    close_popup(ui);
                }
            });
            if top_level {
                if button(ui, "\u{1F4AC}", "Reply in thread").clicked() {
                    acts.push(ChatAction::Thread(m.id));
                }
                if button(ui, "\u{2611}", "Create task from message").clicked() {
                    acts.push(ChatAction::Task { ch: cid, msg: m.id });
                }
            }
            if top_level
                && button(
                    ui,
                    "\u{1F4CC}",
                    if m.pinned { "Unpin" } else { "Pin message" },
                )
                .clicked()
            {
                acts.push(ChatAction::Pin { ch: cid, msg: m.id });
            }
            if button(
                ui,
                "\u{1F516}",
                if m.saved {
                    "Remove from saved"
                } else {
                    "Save for later"
                },
            )
            .clicked()
            {
                acts.push(ChatAction::Save { ch: cid, msg: m.id });
            }
            let mb = button(ui, "\u{22EF}", "More actions");
            if mb.clicked() {
                self.chat_delete = None;
            }
            let now = now_ts();
            let tomorrow = (today() + Duration::days(1))
                .and_hms_opt(9, 0, 0)
                .and_then(|x| x.and_local_timezone(chrono::Local).single())
                .map(|x| x.timestamp())
                .unwrap_or(now + 86400);
            popup(ui, &mb, more_id, 220.0, |ui| {
                let row = |ui: &mut Ui, label: &str| menu_row(ui, &t, tr(label), false, |_, _| {});
                if !m.text.is_empty() && row(ui, "Copy text").clicked() {
                    ui.ctx().copy_text(m.text.clone());
                    acts.push(ChatAction::Copied);
                    close_popup(ui);
                }
                if top_level && m.author != me && row(ui, "Mark unread").clicked() {
                    acts.push(ChatAction::MarkUnread { ch: cid, ts: m.ts });
                    close_popup(ui);
                }
                ui.separator();
                for (label, at) in [
                    ("Remind me in 1 hour", Some(now + 3600)),
                    ("Remind me in 3 hours", Some(now + 3 * 3600)),
                    ("Remind me tomorrow", Some(tomorrow)),
                    ("Remind me at...", None),
                ] {
                    if row(ui, label).clicked() {
                        acts.push(ChatAction::Remind {
                            ch: cid,
                            msg: m.id,
                            at,
                        });
                        close_popup(ui);
                    }
                }
                if m.author == me {
                    ui.separator();
                    if m.call.is_none() && row(ui, "Edit message").clicked() {
                        acts.push(ChatAction::Edit { ch: cid, msg: m.id });
                        close_popup(ui);
                    }
                    // a second click deletes, so one slip doesn't lose a message
                    let armed = self.chat_delete == Some(m.id);
                    if row(
                        ui,
                        if armed {
                            "Click again to delete"
                        } else {
                            "Delete message"
                        },
                    )
                    .clicked()
                    {
                        if armed {
                            acts.push(ChatAction::Delete { ch: cid, msg: m.id });
                            close_popup(ui);
                        } else {
                            self.chat_delete = Some(m.id);
                        }
                    }
                }
            });
        }
    }

    /// Changing one of my messages in place: Enter saves, Escape cancels.
    fn edit_box(&mut self, ui: &mut Ui, cid: u64, mid: u64, acts: &mut Vec<ChatAction>) {
        let t = self.t();
        let Some((_, _, text)) = self.chat_edit.as_mut() else {
            return;
        };
        let id = egui::Id::new(("chat_edit", cid, mid));
        let enter = take_enter(ui, id);
        let r = egui::Frame::new()
            .stroke(Stroke::new(1.0, t.primary))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .fill(t.card)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(text)
                        .id(id)
                        .frame(false)
                        .desired_rows(1)
                        .desired_width(f32::INFINITY),
                )
            })
            .inner;
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Edit message")
        });
        focus_once(ui, &r);
        let esc =
            (r.has_focus() || r.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Escape));
        let mut save = enter;
        let mut cancel = esc;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tr("Enter to save, Esc to cancel"))
                    .small()
                    .color(t.text3),
            );
            if ui.button(tr("Cancel")).clicked() {
                cancel = true;
            }
            if primary_button(ui, &t, tr("Save")).clicked() {
                save = true;
            }
        });
        if save {
            acts.push(ChatAction::SaveEdit {
                ch: cid,
                msg: mid,
                text: text.trim().to_string(),
            });
        } else if cancel {
            acts.push(ChatAction::CancelEdit);
        }
    }

    /// My saved messages from every conversation I can see, newest first;
    /// a click opens the conversation (and the thread, for a reply). True
    /// when one was opened.
    pub(super) fn saved_messages(&mut self, ui: &mut Ui) -> bool {
        let t = self.t();
        let mut found: Vec<(u64, String, Option<u64>, Message)> = vec![];
        for c in self.data.channels.iter().filter(|c| self.can_see(c)) {
            let label = self.channel_label(c);
            for m in &c.messages {
                if m.saved {
                    found.push((self.dm_primary(c), label.clone(), None, m.clone()));
                }
                for r in m.replies.iter().filter(|r| r.saved) {
                    found.push((self.dm_primary(c), label.clone(), Some(m.id), r.clone()));
                }
            }
        }
        found.sort_by_key(|x| std::cmp::Reverse(x.3.ts));
        if found.is_empty() {
            ui.label(RichText::new(tr("Messages you save for later show up here.")).color(t.text3));
            return false;
        }
        let mut open = None;
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .show(ui, |ui| {
                for (ch, label, parent, m) in &found {
                    let who = self
                        .data
                        .member(m.author)
                        .map(|x| x.name.clone())
                        .unwrap_or_else(|| tr("Someone").into());
                    let body = m
                        .post_title
                        .clone()
                        .filter(|x| !x.is_empty())
                        .unwrap_or_else(|| m.text.clone());
                    let body = if body.is_empty() && !m.attachments.is_empty() {
                        tr("Picture or video").to_string()
                    } else {
                        truncate(body.lines().next().unwrap_or(""), 80)
                    };
                    let r = egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(label).small().color(t.text3));
                                ui.label(RichText::new(fmt_ago(m.ts)).small().color(t.text3));
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(who).strong());
                                ui.label(body);
                            });
                        });
                    let resp = ui.interact(
                        r.response.rect,
                        ui.id().with(("saved_msg", m.id)),
                        Sense::click(),
                    );
                    name_it(&resp, "Saved message");
                    if resp.hovered() {
                        ui.painter().rect_stroke(
                            r.response.rect,
                            6.0,
                            Stroke::new(1.0, t.border),
                            egui::StrokeKind::Inside,
                        );
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        open = Some((*ch, *parent));
                    }
                }
            });
        let Some((ch, parent)) = open else {
            return false;
        };
        self.section = Section::Chat;
        self.chat_open = Some(ch);
        self.thread_open = parent;
        true
    }

    pub(super) fn apply_chat(&mut self, acts: Vec<ChatAction>) {
        let me = self.data.me;
        for a in acts {
            // a direct message shows its duplicates' messages too: act on the
            // copy that holds the message
            let a = match a {
                ChatAction::React { ch, msg, emoji } => ChatAction::React {
                    ch: self.msg_home(ch, msg),
                    msg,
                    emoji,
                },
                ChatAction::Pin { ch, msg } => ChatAction::Pin {
                    ch: self.msg_home(ch, msg),
                    msg,
                },
                ChatAction::Save { ch, msg } => ChatAction::Save {
                    ch: self.msg_home(ch, msg),
                    msg,
                },
                ChatAction::Task { ch, msg } if ch != u64::MAX => ChatAction::Task {
                    ch: self.msg_home(ch, msg),
                    msg,
                },
                ChatAction::SaveEdit { ch, msg, text } => ChatAction::SaveEdit {
                    ch: self.msg_home(ch, msg),
                    msg,
                    text,
                },
                ChatAction::Delete { ch, msg } => ChatAction::Delete {
                    ch: self.msg_home(ch, msg),
                    msg,
                },
                ChatAction::Remind { ch, msg, at } => ChatAction::Remind {
                    ch: self.msg_home(ch, msg),
                    msg,
                    at,
                },
                ChatAction::OpenMedia { ch, msg, index } => ChatAction::OpenMedia {
                    ch: self.msg_home(ch, msg),
                    msg,
                    index,
                },
                ChatAction::Send {
                    ch,
                    text,
                    parent: Some(pid),
                    files,
                } => ChatAction::Send {
                    ch: self.msg_home(ch, pid),
                    text,
                    parent: Some(pid),
                    files,
                },
                a => a,
            };
            match a {
                ChatAction::JoinHuddle(ch) => self.join_huddle(ch),
                ChatAction::Open(id) => {
                    self.chat_open = Some(id);
                    self.thread_open = None;
                }
                ChatAction::Thread(id) => self.thread_open = Some(id),
                ChatAction::CloseThread => self.thread_open = None,
                ChatAction::React { ch, msg, emoji } => {
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(m) = find_msg(&mut c.messages, msg) {
                            if let Some(i) = m.reactions.iter().position(|r| r.emoji == emoji) {
                                let r = &mut m.reactions[i];
                                if let Some(j) = r.users.iter().position(|u| *u == me) {
                                    r.users.remove(j);
                                } else {
                                    r.users.push(me);
                                }
                                if r.users.is_empty() {
                                    m.reactions.remove(i);
                                }
                            } else {
                                m.reactions.push(Reaction {
                                    emoji,
                                    users: vec![me],
                                });
                            }
                            self.dirty = true;
                        }
                    }
                }
                ChatAction::Pin { ch, msg } => {
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(m) = c.messages.iter_mut().find(|m| m.id == msg) {
                            m.pinned = !m.pinned;
                            self.dirty = true;
                        }
                    }
                }
                ChatAction::SendPost {
                    ch,
                    title,
                    text,
                    files,
                } => {
                    self.send_files(&files);
                    let id = self.data.new_id();
                    let mentions = self.data.mentions_in(&text);
                    let m = Message {
                        edited: None,
                        mentions,
                        attachments: files,
                        id,
                        author: me,
                        text,
                        ts: now_ts(),
                        reactions: vec![],
                        replies: vec![],
                        saved: false,
                        task: None,
                        post_title: Some(title),
                        post_type: "Announcement".into(),
                        pinned: false,
                        call: None,
                    };
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        c.messages.push(m);
                        c.last_read = c.last_read.max(now_ts());
                    }
                    self.dirty = true;
                }
                ChatAction::Save { ch, msg } => {
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(m) = find_msg(&mut c.messages, msg) {
                            m.saved = !m.saved;
                            self.dirty = true;
                        }
                    }
                }
                ChatAction::Task { ch, msg } => {
                    if ch == u64::MAX {
                        // msg holds a task id here: open it
                        self.apply(Action::Open(msg));
                        continue;
                    }
                    let text = self
                        .data
                        .channels
                        .iter()
                        .find(|c| c.id == ch)
                        .and_then(|c| c.messages.iter().find(|m| m.id == msg))
                        .map(|m| m.text.clone());
                    let Some(text) = text else { continue };
                    let Some(list) = self.data.first_list() else {
                        continue;
                    };
                    let name: String = text.lines().next().unwrap_or("").chars().take(80).collect();
                    self.apply(Action::Add {
                        list,
                        name,
                        status: None,
                        priority: 0,
                        assignees: vec![me],
                        due: None,
                        parent: None,
                        open: true,
                    });
                    let tid = self.data.next_id;
                    if let Some(task) = self.data.tasks.iter_mut().find(|x| x.id == tid) {
                        task.description = text.clone();
                    }
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(m) = c.messages.iter_mut().find(|m| m.id == msg) {
                            m.task = Some(tid);
                        }
                    }
                    self.dirty = true;
                }
                ChatAction::Edit { ch, msg } => {
                    // (kept under the conversation shown; saving finds its copy)
                    let home = self.msg_home(ch, msg);
                    let text = self
                        .data
                        .channels
                        .iter_mut()
                        .find(|c| c.id == home)
                        .and_then(|c| find_msg(&mut c.messages, msg))
                        .filter(|m| m.author == me)
                        .map(|m| m.text.clone());
                    if let Some(text) = text {
                        self.chat_edit = Some((ch, msg, text));
                    }
                    self.chat_delete = None;
                }
                ChatAction::SaveEdit { ch, msg, text } => {
                    self.chat_edit = None;
                    if text.is_empty() {
                        continue;
                    }
                    let members = self.data.members.clone();
                    if let Some(m) = self
                        .data
                        .channels
                        .iter_mut()
                        .find(|c| c.id == ch)
                        .and_then(|c| find_msg(&mut c.messages, msg))
                    {
                        if m.author == me && m.text != text {
                            m.mentions = crate::model::mentioned_ids(&text, &members, &m.mentions);
                            m.text = text;
                            m.edited = Some(now_ts());
                            self.dirty = true;
                        }
                    }
                }
                ChatAction::CancelEdit => self.chat_edit = None,
                ChatAction::Delete { ch, msg } => {
                    self.chat_delete = None;
                    if self.chat_edit.as_ref().is_some_and(|e| e.1 == msg) {
                        self.chat_edit = None;
                    }
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(i) = c
                            .messages
                            .iter()
                            .position(|m| m.id == msg && m.author == me)
                        {
                            // a message with a thread takes its replies with it
                            c.messages.remove(i);
                            if self.thread_open == Some(msg) {
                                self.thread_open = None;
                            }
                            self.dirty = true;
                        } else {
                            for p in c.messages.iter_mut() {
                                if let Some(i) =
                                    p.replies.iter().position(|r| r.id == msg && r.author == me)
                                {
                                    p.replies.remove(i);
                                    self.dirty = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                ChatAction::MarkUnread { ch, ts } => {
                    let ids: Vec<u64> = self
                        .data
                        .channels
                        .iter()
                        .find(|c| c.id == ch)
                        .map(|c| self.conversation(c).iter().map(|d| d.id).collect())
                        .unwrap_or_default();
                    for id in ids {
                        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == id) {
                            c.last_read = ts - 1;
                            // and the messages from then on no longer seen
                            if let Some(s) = self.data.extra.chat_seen.get_mut(&id) {
                                let later: HashSet<u64> = c
                                    .messages
                                    .iter()
                                    .filter(|m| m.ts >= ts)
                                    .map(|m| m.id)
                                    .collect();
                                s.ids.retain(|x| !later.contains(x));
                            }
                            self.chat_keep_unread = Some(ch);
                            if self.persist {
                                self.dirty = true;
                            }
                        }
                    }
                }
                ChatAction::Remind { ch, msg, at } => {
                    let found = self
                        .data
                        .channels
                        .iter_mut()
                        .find(|c| c.id == ch)
                        .and_then(|c| find_msg(&mut c.messages, msg))
                        .map(|m| (m.author, m.text.clone(), m.post_title.clone()));
                    let Some((author, text, title)) = found else {
                        continue;
                    };
                    let who = self
                        .data
                        .member(author)
                        .map(|x| x.name.clone())
                        .unwrap_or_else(|| "Someone".into());
                    let body = title.filter(|x| !x.is_empty()).unwrap_or(text);
                    let what = format!(
                        "Message from {}: {}",
                        who,
                        truncate(body.lines().next().unwrap_or(""), 60)
                    );
                    match at {
                        Some(ts) => {
                            let id = self.data.new_id();
                            self.data.extra.reminders.push(Reminder {
                                id,
                                text: what,
                                due_ts: ts,
                                done: false,
                                owner: me,
                                message: Some((ch, msg)),
                                ..Default::default()
                            });
                            self.dirty = true;
                            self.toast = Some((tr("Reminder created").into(), now_ts()));
                        }
                        None => {
                            self.modal = Some(Modal::Reminder {
                                text: what,
                                date: Some(today()),
                                time: "09:00".into(),
                            });
                            self.reminder_about = (None, Some((ch, msg)));
                        }
                    }
                }
                ChatAction::Copied => self.toast = Some((tr("Copied").into(), now_ts())),
                ChatAction::OpenMedia { ch, msg, index } => {
                    // a picture that failed to load tries again right away
                    let file = self
                        .data
                        .channels
                        .iter()
                        .find(|c| c.id == ch)
                        .and_then(|c| {
                            c.messages
                                .iter()
                                .flat_map(|m| std::iter::once(m).chain(m.replies.iter()))
                                .find(|m| m.id == msg)
                        })
                        .and_then(|m| m.attachments.get(index))
                        .map(|a| a.file.clone());
                    if let Some(f) = file.filter(|f| {
                        matches!(self.files.state(f), Some(super::files::Transfer::Failed(_)))
                    }) {
                        self.files.retry(&f);
                    }
                    self.viewer = Some(super::files::Viewer::new(ch, msg, index));
                }
                ChatAction::Send {
                    ch,
                    text,
                    parent,
                    files,
                } => {
                    if self.chat_keep_unread == Some(ch) {
                        self.chat_keep_unread = None;
                    }
                    self.send_files(&files);
                    let id = self.data.new_id();
                    let mentions = self.data.mentions_in(&text);
                    let m = Message {
                        edited: None,
                        mentions,
                        attachments: files,
                        id,
                        author: me,
                        text,
                        ts: now_ts(),
                        reactions: vec![],
                        replies: vec![],
                        saved: false,
                        task: None,
                        post_title: None,
                        post_type: String::new(),
                        pinned: false,
                        call: None,
                    };
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        match parent {
                            Some(pid) => {
                                if let Some(p) = c.messages.iter_mut().find(|x| x.id == pid) {
                                    p.replies.push(m);
                                }
                            }
                            None => c.messages.push(m),
                        }
                        c.last_read = c.last_read.max(now_ts());
                    }
                    self.dirty = true;
                }
                ChatAction::NewChannel(name) => {
                    let id = self.data.new_id();
                    let name = name.trim().to_lowercase().replace(' ', "-");
                    let members = self.data.members.iter().map(|m| m.id).collect();
                    self.data.channels.push(Channel {
                        id,
                        name,
                        kind: ChannelKind::Channel,
                        topic: String::new(),
                        members,
                        messages: vec![],
                        last_read: now_ts(),
                        creator: me,
                        ..Default::default()
                    });
                    self.chat_open = Some(id);
                    self.thread_open = None;
                    self.dirty = true;
                }
                ChatAction::Dm(uid) => {
                    let existing = self
                        .data
                        .channels
                        .iter()
                        .find(|c| {
                            c.kind == ChannelKind::Direct
                                && c.members.contains(&uid)
                                && c.members.contains(&me)
                                && !self.dm_duplicate(c)
                        })
                        .map(|c| c.id);
                    let id = match existing {
                        Some(id) => id,
                        None => {
                            let id = self.data.new_id();
                            self.data.channels.push(Channel {
                                id,
                                name: String::new(),
                                kind: ChannelKind::Direct,
                                topic: String::new(),
                                members: vec![me, uid],
                                messages: vec![],
                                last_read: now_ts(),
                                ..Default::default()
                            });
                            self.dirty = true;
                            id
                        }
                    };
                    self.chat_open = Some(id);
                    self.thread_open = None;
                }
            }
        }
    }
}
