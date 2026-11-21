// Chat: Channels and Direct Messages in a list on the left (the original's
// sidebar sections "Channels" and "Direct Messages"), the message feed with
// reactions, saved messages and "Reply in thread", a thread panel on the
// right, and turning a message into a task. Everything stays on this computer.

use super::icons::{icon_button, icon_button_ex, line_icon};
use super::*;

pub(super) const QUICK_REACTIONS: [&str; 4] = ["\u{1F44D}", "\u{2705}", "\u{1F440}", "\u{1F389}"];

/// How many of a conversation's newest messages are drawn, and how many
/// more each "Show earlier messages" adds. Every drawn row is laid out on
/// every frame, so a channel with years of history draws only its end.
pub(super) const CHAT_PAGE: usize = 200;

/// The emoji grid behind "Add reaction" and the message box's emoji button.
/// Each must be in egui's built-in emoji fonts, or it draws as an empty box.
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
    "\u{1F615}",
    "\u{1F62E}",
    "\u{1F622}",
    "\u{1F621}",
    "\u{1F60E}",
    "\u{1F44F}",
    "\u{1F64C}",
    "\u{1F4AA}",
    "\u{1F44B}",
    "\u{1F44A}",
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
pub(super) fn emoji_grid(ui: &mut Ui) -> Option<&'static str> {
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

/// How long a message gone to stays lit up, in seconds.
const FLASH_SECS: f64 = 2.0;

/// The thread panel's width until it is dragged wider or narrower.
const THREAD_W: f32 = 360.0;

pub(super) enum ChatAction {
    JoinHuddle(u64),
    /// "Open goal" on a goal's card.
    OpenGoal(u64),
    /// Open the Forward window for a message.
    Forward {
        ch: u64,
        msg: u64,
    },
    /// The writer takes a link preview off their message.
    RemovePreview {
        ch: u64,
        msg: u64,
    },
    React {
        ch: u64,
        msg: u64,
        emoji: String,
    },
    Thread(u64),
    CloseThread,
    /// The open thread moves out to its own window (conversation, and where
    /// on the screen the window goes: None for wherever the system puts it).
    ThreadWindow {
        ch: u64,
        at: Option<Rect>,
    },
    /// The thread's window goes back to being the panel beside the conversation.
    DockThread,
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
    /// Open the direct message with this person (made if there is none).
    Dm(u64),
    /// Show this person's profile at the right.
    Profile(u64),
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
pub(super) fn take_enter(ui: &Ui, id: egui::Id) -> bool {
    if !ui.memory(|m| m.has_focus(id)) {
        return false;
    }
    ui.input_mut(|i| {
        let before = i.events.len();
        i.events.retain(|e| !matches!(e, egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift && !modifiers.command && !modifiers.alt));
        i.events.len() != before
    })
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

/// Link previews on their way: fetched on a thread of their own for each
/// message I send with a web address in it (linkpreview.rs), then put on
/// the message by `previews_tick`.
pub(super) struct Previews {
    tx: std::sync::mpsc::Sender<(u64, u64, LinkPreview)>,
    rx: std::sync::mpsc::Receiver<(u64, u64, LinkPreview)>,
    ctx: std::sync::Arc<std::sync::Mutex<Option<egui::Context>>>,
}

impl Default for Previews {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Previews {
            tx,
            rx,
            ctx: Default::default(),
        }
    }
}

impl App {
    /// Fetch the preview of the first web address of message `msg` (in
    /// channel `ch`), just sent.
    pub(super) fn fetch_preview(&self, ch: u64, msg: u64, text: &str) {
        let Some(url) = super::chatfmt::first_url(text) else {
            return;
        };
        let tx = self.previews.tx.clone();
        let ctx = self.previews.ctx.clone();
        let job = move || {
            if let Some(p) = crate::linkpreview::fetch(&url) {
                let _ = tx.send((ch, msg, p));
                if let Some(c) = ctx.lock().unwrap().as_ref() {
                    c.request_repaint();
                }
            }
        };
        // (the tests' fetch visits nothing and answers at once)
        if cfg!(test) {
            job();
        } else {
            std::thread::spawn(job);
        }
    }

    /// A link preview under a message: the site, the page's title (opens
    /// it) and description. True when its writer (`mine`) takes it away.
    pub(super) fn preview_card(&self, ui: &mut Ui, p: &LinkPreview, mine: bool) -> bool {
        let t = self.t();
        let mut remove = false;
        ui.add_space(4.0);
        let inner = egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 10,
                right: 8,
                top: 6,
                bottom: 6,
            })
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(460.0));
                ui.horizontal(|ui| {
                    if !p.site.is_empty() {
                        ui.label(RichText::new(&p.site).small().color(t.text3));
                    }
                    if mine {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let x = ui
                                .add(ghost(egui::Button::new(
                                    RichText::new("\u{00D7}").color(t.text3),
                                )))
                                .on_hover_text(tr("Remove preview"));
                            name_it(&x, "Remove preview");
                            remove = x.clicked();
                        });
                    }
                });
                if !p.title.is_empty() {
                    let title = ui.add(
                        egui::Label::new(
                            RichText::new(&p.title)
                                .font(crate::app::bold_font(ui.ctx(), 14.0))
                                .color(t.primary),
                        )
                        .sense(Sense::click())
                        .wrap(),
                    );
                    name_it(&title, &p.title);
                    if title.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if title.on_hover_text(&p.url).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(p.url.clone()));
                    }
                }
                if !p.description.is_empty() {
                    ui.add(egui::Label::new(RichText::new(&p.description).color(t.text2)).wrap());
                }
            });
        // the bar down its left side
        let r = inner.response.rect;
        ui.painter().rect_filled(
            Rect::from_min_size(r.min, vec2(3.0, r.height())),
            1.5,
            t.border,
        );
        remove
    }

    /// Put fetched previews on their messages (unless taken away meanwhile:
    /// a message edited to another address keeps none).
    pub(super) fn previews_tick(&mut self, ctx: &egui::Context) {
        *self.previews.ctx.lock().unwrap() = Some(ctx.clone());
        while let Ok((ch, id, p)) = self.previews.rx.try_recv() {
            if let Some(m) = self
                .data
                .channels
                .iter_mut()
                .find(|c| c.id == ch)
                .and_then(|c| find_msg(&mut c.messages, id))
            {
                if super::chatfmt::first_url(&m.text).as_deref() == Some(p.url.as_str()) {
                    m.preview = Some(p);
                    self.dirty = true;
                }
            }
        }
    }
}

/// The commands that start with what's typed after "/", under a message
/// box; the one clicked.
fn slash_list(ui: &mut Ui, t: &Theme, choices: &[(String, String)]) -> Option<String> {
    if choices.is_empty() {
        return None;
    }
    let mut out = None;
    egui::Frame::new()
        .fill(t.card)
        .stroke(Stroke::new(1.0_f32, t.border))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            for (name, about) in choices.iter().take(8) {
                let r = ui
                    .horizontal(|ui| {
                        let b = ui.add(
                            egui::Label::new(
                                RichText::new(format!("/{}", name)).monospace().strong(),
                            )
                            .sense(Sense::click()),
                        );
                        ui.label(RichText::new(about).small().color(t.text2));
                        b
                    })
                    .inner;
                name_it(&r, &format!("Command /{}", name));
                if r.clicked() {
                    out = Some(name.clone());
                }
            }
        });
    out
}

/// The text cursor of box `id` after the last character of `text`.
fn move_cursor_to_end(ctx: &egui::Context, id: egui::Id, text: &str) {
    let n = text.chars().count();
    let mut st = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
        egui::text::CCursor::new(n),
    )));
    st.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// The formatting marks the message boxes offer: (mark, name, key, with Shift).
const FORMATS: [(&str, &str, egui::Key, bool); 4] = [
    ("**", "Bold", egui::Key::B, false),
    ("*", "Italic", egui::Key::I, false),
    ("~~", "Strikethrough", egui::Key::X, true),
    ("`", "Code", egui::Key::C, true),
];

/// Puts `mark` around what is selected in the text box `id` (chatfmt::wrap),
/// keeping the same text selected.
fn format_selection(ctx: &egui::Context, id: egui::Id, text: &mut String, mark: &str) {
    let n = text.chars().count();
    let mut st = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    let (a, b) = st
        .cursor
        .char_range()
        .map(|r| (r.primary.index.min(n), r.secondary.index.min(n)))
        .unwrap_or((n, n));
    let (a, b) = super::chatfmt::wrap(text, a, b, mark);
    st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
        egui::text::CCursor::new(a),
        egui::text::CCursor::new(b),
    )));
    st.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Cmd+B, Cmd+I, Cmd+Shift+X and Cmd+Shift+C in the focused text box `id`.
fn format_keys(ui: &mut Ui, id: egui::Id, text: &mut String) {
    if !ui.memory(|m| m.has_focus(id)) {
        return;
    }
    for (mark, _, key, shift) in FORMATS {
        let mods = if shift {
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::COMMAND
        };
        if ui.input_mut(|i| i.consume_key(mods, key)) {
            format_selection(ui.ctx(), id, text, mark);
        }
    }
}

/// The B, I, S and </> buttons of a message box; `prefix` tells the
/// boxes apart for the tests ("" or "reply ").
fn format_buttons(ui: &mut Ui, t: &Theme, id: egui::Id, text: &mut String, prefix: &str) {
    for (mark, name, _, _) in FORMATS {
        let glyph = match name {
            "Bold" => RichText::new("B").strong(),
            "Italic" => RichText::new("I").italics(),
            "Strikethrough" => RichText::new("S").strikethrough(),
            _ => RichText::new("</>").monospace(),
        };
        let b = ui
            .add(ghost(egui::Button::new(glyph.color(t.text2))))
            .on_hover_text(tr(name));
        name_it(&b, &format!("{}{}", prefix, name));
        if b.clicked() {
            format_selection(ui.ctx(), id, text, mark);
        }
    }
}

/// The groups at the end of an @ list: "@channel" (everyone here), "@here"
/// (who is online) and each team by name. The text to put in the box for
/// the one clicked.
pub(super) fn group_mention_rows(ui: &mut Ui, t: &Theme, teams: &[String]) -> Option<String> {
    let mut out = None;
    ui.separator();
    for (word, tip) in [
        ("channel", "Everyone in this conversation"),
        ("here", "Everyone here who is online"),
    ] {
        let r = menu_row(ui, t, &format!("@{}", word), false, |p, c| {
            line_icon(p, "teams", c, t.text2);
        })
        .on_hover_text(tr(tip));
        if r.clicked() {
            out = Some(format!("@{} ", word));
        }
    }
    for name in teams {
        if menu_row(ui, t, &format!("@{}", name), false, |p, c| {
            line_icon(p, "teams", c, t.text2);
        })
        .on_hover_text(tr("Everyone in this team"))
        .clicked()
        {
            out = Some(format!("@{} ", name));
        }
    }
    out
}

/// A message that still says something with its text taken away: pictures,
/// a forwarded message or a post title. Emptying the text of any other
/// message deletes it (as Slack does).
fn stands_without_text(m: &Message) -> bool {
    !m.attachments.is_empty()
        || m.forwarded.is_some()
        || m.post_title.as_ref().is_some_and(|t| !t.trim().is_empty())
}

/// A message with pictures, videos or other files sent in it (the channel
/// panel's Files).
pub(super) fn has_files(m: &Message) -> bool {
    !m.attachments.is_empty()
}

/// A message with a web address in its text (the channel panel's Links).
pub(super) fn has_links(m: &Message) -> bool {
    m.preview.is_some() || super::chatfmt::first_url(&m.text).is_some()
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
    /// A post of mine in conversation `ch`, from the channel's box (Post) or
    /// the Posts page: who it mentions worked out as for any message there
    /// (`picked`: the people chosen from the box's @ list). The title is one
    /// line (text pasted into it may bring line breaks). Returns its id.
    pub(super) fn add_post(
        &mut self,
        ch: u64,
        title: String,
        kind: &str,
        text: String,
        files: Vec<Attachment>,
        picked: &[u64],
    ) -> u64 {
        let id = self.data.new_id();
        let mentions = self.chat_mentions(&text, ch, picked);
        let m = Message {
            goal: None,
            edited: None,
            imported: None,
            preview: None,
            forwarded: None,
            thread_of: None,
            bot: None,
            mentions,
            attachments: files,
            id,
            author: self.data.me,
            text,
            ts: now_ts(),
            reactions: vec![],
            replies: vec![],
            saved: false,
            task: None,
            post_title: Some(crate::widgets::one_line(title.trim())),
            post_type: kind.to_string(),
            pinned: false,
            call: None,
        };
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
            c.messages.push(m);
            c.last_read = c.last_read.max(now_ts());
        }
        self.dirty = true;
        // the channel's workflows start from a post as from a message
        self.run_workflows(ch, id);
        id
    }

    pub(super) fn channel_label(&self, c: &Channel) -> String {
        match c.kind {
            ChannelKind::Channel => format!("# {}", c.name),
            ChannelKind::Direct => c
                .members
                .iter()
                .find(|m| **m != self.data.me)
                .and_then(|m| self.data.member(*m))
                .map(|m| m.shown().to_string())
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

    pub(super) fn unread_here(&self, c: &Channel) -> usize {
        c.messages.iter().filter(|m| self.is_unread(c, m)).count()
    }

    /// Conversation `c` (or a copy of it) is open in Chat and read as
    /// messages come in, as the end of [`Self::chat`] marks it: not
    /// marked unread to stay so, the window in front.
    pub(super) fn reading_now(&self, c: &Channel) -> bool {
        let Some(open) = self.chat.open else {
            return false;
        };
        self.chat.shown
            && self.section == Section::Chat
            && self.chat.keep_unread != Some(open)
            && !self.notify_rt.away
            && self.conversation(c).iter().any(|d| d.id == open)
    }

    /// A message from someone else in channel `c` I haven't seen.
    pub(super) fn is_unread(&self, c: &Channel, m: &Message) -> bool {
        m.author != self.data.me
            // imported history isn't new to anyone
            && m.imported.is_none()
            && match self.data.extra.chat_seen.get(&c.id) {
                Some(s) => !s.has(m.id) && (m.ts >= s.floor || m.ts > c.last_read),
                None => m.ts > c.last_read,
            }
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

    /// Mark the replies in thread `parent` of channel `cid` read, as
    /// "Mark read" in Replies does (`replies_read`).
    pub(super) fn mark_thread_read(&mut self, cid: u64, parent: &Message) {
        let me = self.data.me;
        let mut added = false;
        for r in parent.replies.iter().filter(|r| r.author != me) {
            let k = format!("chat:{}:{}", cid, r.id);
            if !self.data.extra.replies_read.contains(&k) {
                self.data.extra.replies_read.push(k);
                added = true;
            }
        }
        if added && self.persist {
            self.dirty = true;
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
    /// or admin: of the team server on a server (it checks too), by the
    /// role in People alone or in a shared folder.
    pub(super) fn may_manage_channel(&self, c: &Channel) -> bool {
        let me = self.data.me;
        let server = self.on_server();
        let admin = if server {
            self.sync_rt.role.is_some_and(|r| r.manages())
        } else {
            self.i_manage()
        };
        admin || c.creator == me || (c.creator == 0 && c.members.contains(&me))
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
        if self.chat.open == Some(cid) {
            self.chat.open = None;
            self.leave_thread();
        }
        self.dirty = true;
    }

    /// The teams' names, for the @ lists.
    pub(super) fn team_names(&self) -> Vec<String> {
        self.data
            .extra
            .teams
            .iter()
            .map(|t| t.name.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect()
    }

    /// Everyone in a conversation: a public channel is everyone's, a private
    /// channel or a direct message its members'.
    pub(super) fn conversation_people(&self, c: &Channel) -> Vec<u64> {
        if c.kind == ChannelKind::Channel && !c.private {
            self.data
                .members
                .iter()
                .filter(|m| !m.invited)
                .map(|m| m.id)
                .collect()
        } else {
            c.members.clone()
        }
    }

    /// Who a message I write in channel `ch` mentions (wb_sync's
    /// `chat_mentions`): people by name, teams by name ("@Design"),
    /// everyone here for "@channel" / "@everyone", and those of them online
    /// now for "@here".
    pub(super) fn chat_mentions(&self, text: &str, ch: u64, picked: &[u64]) -> Vec<u64> {
        let people: Vec<(u64, &str)> = self
            .data
            .members
            .iter()
            .flat_map(|m| m.names().into_iter().map(move |n| (m.id, n)))
            .collect();
        let groups: Vec<(&str, &[u64])> = self
            .data
            .extra
            .teams
            .iter()
            .filter(|t| !t.name.trim().is_empty())
            .map(|t| (t.name.trim(), t.members.as_slice()))
            .collect();
        let conv = self
            .data
            .channels
            .iter()
            .find(|c| c.id == ch)
            .map(|c| self.conversation_people(c))
            .unwrap_or_default();
        let online = |id: u64| self.is_online(id) == Some(true);
        wb_sync::mentions::chat_mentions(
            text,
            &people,
            &groups,
            &conv,
            &online,
            self.data.me,
            picked,
        )
    }

    /// Where `text` @mentions someone (byte ranges): "@" and a member's or
    /// team's name, else "@" and the word after it (as `mention_text` marks).
    pub(super) fn mention_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        mention_spans(text, &self.mention_names())
    }

    /// The names an @ can stand for: everyone's names and the teams'.
    pub(super) fn mention_names(&self) -> Vec<String> {
        self.data
            .members
            .iter()
            .flat_map(|m| m.names())
            .map(str::to_string)
            .chain(self.team_names())
            .filter(|n| !n.is_empty())
            .collect()
    }

    /// A message's text as it is shown (chatfmt): its formatting, mentions
    /// marked and links clickable. A workboard:// link clicked comes back
    /// to be opened here; a web address opens in the browser.
    pub(super) fn message_text(&self, ui: &mut Ui, src: &str, size: f32) -> Option<String> {
        let t = self.t();
        let r = super::chatfmt::rich(src);
        let mentions = self.mention_ranges(&r.text);
        let mut links = r.links.clone();
        for l in super::nav::find_links(&r.text) {
            if !links.iter().any(|x| x.0 < l.1 && l.0 < x.1) {
                links.push(l);
            }
        }
        let mut cuts: Vec<usize> = vec![0, r.text.len()];
        for (a, b, _) in &r.runs {
            cuts.extend([*a, *b]);
        }
        for (a, b) in &mentions {
            cuts.extend([*a, *b]);
        }
        cuts.sort_unstable();
        cuts.dedup();
        let bold = crate::app::bold_font(ui.ctx(), size);
        let code_bg = if t.dark {
            Color32::from_white_alpha(18)
        } else {
            Color32::from_black_alpha(12)
        };
        let mut job = egui::text::LayoutJob::default();
        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a >= b {
                continue;
            }
            let look = r
                .runs
                .iter()
                .find(|(x, y, _)| *x <= a && b <= *y)
                .map(|x| x.2)
                .unwrap_or_default();
            let mention = mentions.iter().any(|(x, y)| *x <= a && b <= *y);
            let mut f = egui::TextFormat {
                font_id: if look.code || look.block {
                    FontId::monospace(size - 1.0)
                } else if look.bold {
                    bold.clone()
                } else {
                    FontId::proportional(size)
                },
                color: if look.quote { t.text2 } else { t.text },
                italics: look.italic,
                ..Default::default()
            };
            if look.strike {
                f.strikethrough = Stroke::new(1.0_f32, f.color);
            }
            if look.code || look.block {
                f.background = code_bg;
            }
            if mention && !look.code && !look.block {
                (f.color, f.background) = t.mention();
            }
            job.append(&r.text[a..b], 0.0, f);
        }
        if r.text.is_empty() {
            job.append("", 0.0, egui::TextFormat::default());
        }
        job.wrap.max_width = f32::INFINITY;
        if links.is_empty() {
            ui.add(egui::Label::new(job).wrap());
            return None;
        }
        // colour the links (char ranges of the shown text)
        let full = job.text.clone();
        let char_at = |byte: usize| full[..byte].chars().count();
        for s in job.sections.iter_mut() {
            let (a, b) = (char_at(s.byte_range.start), char_at(s.byte_range.end));
            if links.iter().any(|(ls, le, _)| a < *le && b > *ls) {
                s.format.color = t.primary;
                s.format.underline = Stroke::new(1.0_f32, t.primary);
            }
        }
        let (pos, galley, resp) = egui::Label::new(job)
            .wrap()
            .sense(Sense::click())
            .layout_in_ui(ui);
        ui.painter().galley(pos, galley.clone(), t.text);
        let at = |p: egui::Pos2| galley.cursor_from_pos(p - pos).ccursor.index;
        let over = resp
            .hover_pos()
            .map(at)
            .and_then(|c| links.iter().find(|(s, e, _)| c >= *s && c < *e).cloned());
        if let Some((_, _, url)) = &over {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            resp.clone().on_hover_text(url.as_str());
        }
        if resp.clicked() {
            let url = resp
                .interact_pointer_pos()
                .map(at)
                .and_then(|c| links.iter().find(|(s, e, _)| c >= *s && c < *e))
                .map(|l| l.2.clone())?;
            if url.starts_with("workboard://") {
                return Some(url);
            }
            ui.ctx().open_url(egui::OpenUrl::new_tab(url));
        }
        None
    }

    pub(super) fn chat(&mut self, ui: &mut Ui) {
        let t = self.t();
        // nothing open, or (from a search, a saved place, a link) a channel I
        // may not see: the first one in my list instead
        if self.chat.open.map_or(true, |id| {
            self.data
                .channels
                .iter()
                .any(|c| c.id == id && !self.can_see(c))
        }) {
            self.chat.open = self
                .data
                .channels
                .iter()
                .find(|c| self.listed(c))
                .map(|c| c.id);
            self.leave_thread();
        }
        let mut acts: Vec<ChatAction> = vec![];
        let Some(cid) = self.chat.open else {
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
                    let b = ui.add(Tint(primary_btn(&t, tr("#  Create a Channel")).min_size(vec2(170.0, CONTROL_H))));
                    name_it(&b, "Create a Channel");
                    if b.clicked() {
                        make = true;
                    }
                    if icon_button_ex(ui, &t, "send", tr("Message someone"), "Message someone", true, 170.0, t.text, t.text2).clicked() {
                        self.new_message_open = true;
                    }
                    if icon_button_ex(ui, &t, "teams", tr("Work together"), "Work together", true, 150.0, t.text, t.text2).on_hover_text(tr("Share this workspace so others can chat with you")).clicked() {
                        self.modal = Some(self.together_modal());
                    }
                });
            });
            if make {
                self.apply_chat(vec![ChatAction::NewChannel("general".into())]);
            }
            return;
        };
        let Some(ci) = self.data.channels.iter().position(|c| c.id == cid) else {
            self.chat.open = None;
            return;
        };
        // a second copy of a direct message opens as the one that shows
        let primary = self.dm_primary(&self.data.channels[ci]);
        if primary != cid {
            self.chat.open = Some(primary);
            // a thread opened in that copy (from a notification or the
            // Inbox) stays open: the shown copy has its messages too
            let keep = self.chat.thread_open.is_some_and(|tid| {
                self.conversation(&self.data.channels[ci])
                    .iter()
                    .any(|d| d.messages.iter().any(|m| m.id == tid))
            });
            if !keep {
                self.leave_thread();
            }
            return;
        }
        // the copies whose messages show here: this one and its duplicates
        let copies: Vec<usize> = self
            .conversation(&self.data.channels[ci])
            .iter()
            .filter_map(|d| self.data.channels.iter().position(|c| c.id == d.id))
            .collect();
        // where each message is, in time order (the messages themselves are
        // copied below only for the rows drawn: a long channel's history
        // isn't copied on every frame)
        let mut at: Vec<(usize, usize)> = copies
            .iter()
            .flat_map(|&c| (0..self.data.channels[c].messages.len()).map(move |m| (c, m)))
            .collect();
        at.sort_by_key(|&(c, m)| self.data.channels[c].messages[m].ts);
        let no_messages = at.is_empty();
        // ---- where I'd read up to (see ChatUi::read_line)
        let first_unread = at.iter().find_map(|&(c, m)| {
            let ch = &self.data.channels[c];
            let msg = &ch.messages[m];
            self.is_unread(ch, msg).then_some(msg.id)
        });
        let newest = at
            .last()
            .map(|&(c, m)| self.data.channels[c].messages[m].ts)
            .unwrap_or(0);
        let pass = ui.ctx().cumulative_pass_nr();
        let reopened = pass > self.chat.drawn_pass + 1;
        self.chat.drawn_pass = pass;
        match self.chat.read_line {
            Some((c, line)) if c == cid && !reopened => {
                // messages that come in while I'm scrolled up or away from
                // the window: the line goes above the first of them
                if line.is_none() && (!self.chat.at_bottom || self.notify_rt.away) {
                    self.chat.read_line = Some((cid, first_unread));
                } else if line.is_some()
                    && self.chat.at_bottom
                    && !self.notify_rt.away
                    && self.chat.keep_unread != Some(cid)
                    && self
                        .chat
                        .bottom_seen
                        .is_some_and(|(c, ts)| c == cid && newest > ts)
                {
                    // a message comes in while I'm reading along at the
                    // bottom: I've caught up, and a line left above the
                    // earlier ones would make them look still unread
                    self.chat.read_line = Some((cid, None));
                }
            }
            _ => {
                self.chat.read_line = Some((cid, first_unread));
                self.chat.bottom_seen = Some((cid, newest));
                self.chat.at_bottom = true;
            }
        }
        let line_at = self.chat.read_line.and_then(|(_, l)| l);
        // the channel's own settings, without its messages
        let messages = std::mem::take(&mut self.data.channels[ci].messages);
        let channel = self.data.channels[ci].clone();
        self.data.channels[ci].messages = messages;
        if self.chat.keep_unread.is_some_and(|k| k != cid) {
            self.chat.keep_unread = None;
        }
        // a closed conversation that's opened again is back in the sidebar
        if self.data.extra.closed_dms.iter().any(|(id, _)| *id == cid) {
            self.data.extra.closed_dms.retain(|(id, _)| *id != cid);
            self.data.extra.closed_dm_seen.remove(&cid);
            self.dirty = true;
        }

        // ---- a profile, in the thread's place
        self.profile_panel(ui);
        // ---- thread panel (not while the thread is in its own window)
        let mut thread_rect: Option<Rect> = None;
        if let Some(tid) = self
            .chat
            .thread_open
            .filter(|_| self.chat.thread_window.is_none() && self.chat.profile.is_none())
        {
            let found = at.iter().find_map(|&(c, m)| {
                let ch = self.data.channels.get(c)?;
                let m = ch.messages.get(m)?;
                (m.id == tid).then(|| (ch.id, m.clone()))
            });
            if let Some((pc, parent)) = found {
                // the replies on screen are read: they leave Replies' Unread
                // and the Inbox badge (not while the window is behind
                // another app, as for the channel)
                if !self.notify_rt.away {
                    self.mark_thread_read(pc, &parent);
                }
                // wider by dragging its left edge; the conversation keeps room.
                // In a small window it narrows to leave the conversation 340
                // (it was squeezed under the thread, drawn over its messages)
                let avail = ui.available_width();
                let max_w = (avail - 480.0).max(THREAD_W);
                let tight = avail - THREAD_W < 340.0;
                let narrow_w = (avail - 340.0).clamp(240.0, THREAD_W);
                let mut panel = egui::SidePanel::right("thread");
                panel = if tight {
                    panel.exact_width(narrow_w).resizable(false)
                } else {
                    panel
                        .default_width(THREAD_W)
                        .width_range(300.0..=max_w)
                        .resizable(true)
                };
                let panel = panel
                    .frame(
                        egui::Frame::new()
                            .fill(t.column_bg)
                            .inner_margin(egui::Margin::symmetric(14, 10)),
                    )
                    .show_inside(ui, |ui| {
                        self.thread_body(ui, cid, &channel, tid, &parent, false, &mut acts);
                    });
                thread_rect = Some(panel.response.rect);
                self.chat.thread_panel_pass = ui.ctx().cumulative_pass_nr();
                if self.escape_closes_thread(ui.ctx(), false) {
                    acts.push(ChatAction::CloseThread);
                }
            }
        }
        // pictures and videos dragged in from Finder
        self.take_dropped(ui.ctx(), thread_rect);
        let dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());

        // Cmd+F: the find box with the cursor in it, its words selected so
        // typing replaces them (not under a dialog or the task window)
        let find_id = egui::Id::new(("chan_find", cid));
        if self.modal.is_none()
            && self.palette.is_none()
            && self.open_task.is_none()
            && self.viewer.is_none()
            && self.quick_add.is_none()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F))
        {
            let n = self
                .channel_find
                .get_or_insert_with(String::new)
                .chars()
                .count();
            let mut st = egui::TextEdit::load_state(ui.ctx(), find_id).unwrap_or_default();
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(n),
            )));
            st.store(ui.ctx(), find_id);
            ui.memory_mut(|m| m.request_focus(find_id));
        }
        // ---- conversation (header with tabs, side panel, messages, composer)
        let conv = egui::CentralPanel::default().frame(egui::Frame::new().fill(t.bg)).show_inside(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 12, top: 8, bottom: 0 }).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_height(30.0);
                    let title = match channel.kind {
                        ChannelKind::Channel if channel.private => channel.name.clone(),
                        ChannelKind::Channel => format!("# {}", channel.name),
                        ChannelKind::Direct => self.channel_label(&channel),
                    };
                    if channel.kind == ChannelKind::Direct {
                        if let Some(m) = channel.members.iter().find(|m| **m != self.data.me).or(channel.members.first()).and_then(|m| self.data.member(*m)) {
                            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                            paint_avatar(ui.painter(), r.center(), 10.0, m, t.bg);
                        }
                    }
                    if channel.kind == ChannelKind::Channel && channel.private {
                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        line_icon(ui.painter(), "lock", r.center(), t.text2);
                    }
                    // shortened before the buttons on the right (it ran under them)
                    let buttons = if self.ai_offered() && !no_messages { 270.0 } else { 150.0 };
                    let room = (ui.available_width() - buttons).max(40.0);
                    ui.scope(|ui| {
                        ui.set_max_width(room);
                        ui.add(egui::Label::new(RichText::new(title.clone()).size(15.5).color(t.text)).truncate());
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if channel.kind == ChannelKind::Channel {
                            // how this channel notifies me (notify.rs)
                            let mode = self.data.extra.notify.channel_mode(cid).to_string();
                            let bell = icon_button(ui, &t, if mode == "nothing" { "bell_off" } else { "bell" }, "", "Channel notifications", false);
                            let bell = bell.on_hover_text(tr("Notifications for this channel"));
                            popup(ui, &bell, ui.id().with(("chan_notify", cid)), 240.0, |ui| {
                                ui.label(RichText::new(tr("Notify me about")).small().color(t.text2));
                                for (m, label) in [("all", "Every new message"), ("mentions", "Only @mentions and my threads"), ("nothing", "Nothing")] {
                                    if radio(ui, mode == m, tr(label)).clicked() {
                                        self.data.extra.notify.set_channel_mode(cid, m);
                                        self.dirty = true;
                                        close_popup(ui);
                                    }
                                }
                            });
                        }
                        if icon_button_ex(ui, &t, "edit", tr("Ask"), "Ask", false, 0.0, t.text2, t.text2).clicked() {
                            self.section = Section::Ai;
                        }
                        if self.ai_offered() && !no_messages {
                            let b = icon_button_ex(ui, &t, "sparkle", tr("Summarize"), "Summarize conversation", false, 0.0, t.text2, super::ai::AI_VIOLET)
                                .on_hover_text(tr("Sum up this conversation with AI"));
                            if b.clicked() {
                                let label = title.clone();
                                self.ai_summarize_channel(cid, &label);
                            }
                        }
                        if icon_button(ui, &t, "phone", "", "Start a SyncUp", false).on_hover_text(tr("Start a SyncUp")).clicked() {
                            self.open_call_dialog();
                        }
                    });
                });
                // wraps in a narrow conversation instead of widening it
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    // the tabs in my order: each can be dragged to another
                    // place, the others leave a line where it lands
                    let tabs = self.channel_tabs(cid);
                    let mut rects: Vec<Rect> = Vec::with_capacity(tabs.len());
                    let mut lifted: Option<(usize, bool)> = None; // (tab, let go this frame)
                    let mut remove: Option<usize> = None;
                    for (i, tab) in tabs.iter().enumerate() {
                        let chan = tab == "Channel";
                        let color = if chan { t.text } else { t.text2 };
                        let g = ui.painter().layout_no_wrap(tr(tab).into(), FontId::proportional(14.0), color);
                        let w = if chan { g.size().x + 34.0 } else { g.size().x + 8.0 };
                        let (r, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
                        let resp = ui.interact(r, ui.id().with(("chan_tab", cid, tab.as_str())), Sense::click_and_drag());
                        name_it(&resp, &format!("Tab {}", tab));
                        rects.push(r);
                        if resp.dragged() || resp.drag_stopped() {
                            lifted = Some((i, resp.drag_stopped()));
                        }
                        let dim = if resp.dragged() { 0.35 } else { 1.0 };
                        let p = ui.painter();
                        if chan {
                            let ic = Rect::from_center_size(pos2(r.left() + 13.0, r.center().y), vec2(15.0, 15.0));
                            p.rect_filled(ic, 3.0, Color32::from_rgb(0x5f, 0x55, 0xee).gamma_multiply(dim));
                            p.text(ic.center(), Align2::CENTER_CENTER, "#", FontId::proportional(10.0), Color32::WHITE.gamma_multiply(dim));
                            let at = pos2(r.left() + 26.0, r.center().y - g.size().y / 2.0);
                            p.galley(at, g.clone(), color.gamma_multiply(dim));
                            p.galley(at + vec2(0.3, 0.0), g, color.gamma_multiply(dim));
                            p.rect_filled(Rect::from_min_max(pos2(r.left(), r.bottom() - 2.0), r.max), 1.0, t.text.gamma_multiply(dim));
                        } else {
                            if resp.hovered() && !resp.dragged() {
                                p.rect_filled(r.shrink2(vec2(-2.0, 2.0)), 6.0, t.hover);
                            }
                            p.galley(r.center() - g.size() / 2.0, g, color.gamma_multiply(dim));
                        }
                        if resp.clicked() {
                            if let Some(v) = ViewKind::ALL.into_iter().find(|v| v.label() == tab) {
                                self.view = v;
                                // a channel made with "Add a List" opens its own List
                                let to = channel.list.filter(|l| self.data.location_exists(Location::List(*l))).map(Location::List).unwrap_or(Location::Everything);
                                self.actions.push(Action::Go(to));
                            }
                        }
                        if !chan {
                            resp.context_menu(|ui| {
                                if ui.button(tr("Remove")).clicked() {
                                    remove = Some(i);
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                    if let (Some((from, dropped)), Some(p)) = (lifted, ui.ctx().pointer_latest_pos()) {
                        let to = tab_drop_place(&rects, from, p);
                        if dropped {
                            let mut order = tabs.clone();
                            let tab = order.remove(from);
                            order.insert(to, tab);
                            self.set_channel_tabs(cid, order);
                        } else {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            // the line where it lands, halfway into the gap
                            if to != from {
                                let x = if to < from { rects[to].left() - 7.0 } else { rects[to].right() + 7.0 };
                                let row = rects[to];
                                ui.painter().line_segment([pos2(x, row.top() + 6.0), pos2(x, row.bottom() - 6.0)], Stroke::new(2.0_f32, t.primary));
                            }
                            // the tab follows the mouse
                            let fg = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, ui.id().with(("chan_tab_lift", cid))));
                            let g = fg.layout_no_wrap(tr(&tabs[from]).into(), FontId::proportional(14.0), t.text);
                            let r = Rect::from_center_size(p, g.size() + vec2(20.0, 12.0));
                            fg.rect(r, 6.0, t.bg, Stroke::new(1.0_f32, t.border), egui::StrokeKind::Inside);
                            fg.galley(r.center() - g.size() / 2.0, g, t.text);
                        }
                    }
                    if let Some(i) = remove {
                        let mut order = tabs.clone();
                        order.remove(i);
                        self.set_channel_tabs(cid, order);
                    }
                    // "+ View" adds a tab here; it doesn't leave the channel
                    let vr = ui.add(ghost(egui::Button::new(RichText::new(tr("+  View")).size(14.0).color(t.text2)).min_size(vec2(0.0, 30.0))));
                    popup(ui, &vr, ui.id().with(("chan_view", cid)), 180.0, |ui| {
                        for v in ViewKind::ALL.into_iter().filter(|v| *v != ViewKind::Overview) {
                            let have = tabs.iter().any(|x| x == v.label());
                            if ui.add_enabled(!have, egui::Button::new(tr(v.label()))).clicked() {
                                let mut order = tabs.clone();
                                order.push(v.label().to_string());
                                self.set_channel_tabs(cid, order);
                                close_popup(ui);
                            }
                        }
                    });
                });
            });
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0_f32, t.border));
            // right panel: narrower in a small window so the messages keep room;
            // where even that leaves them too little (a thread open beside a
            // small window) only its icons, named on hover: it was drawn
            // over the messages
            let icons_only = ui.available_width() < 560.0;
            let panel_w = if icons_only { 46.0 } else { (ui.available_width() * 0.3).clamp(170.0, 230.0) };
            egui::SidePanel::right(egui::Id::new(("chan_panel", cid))).exact_width(panel_w).resizable(false).show_separator_line(false).frame(egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 12, top: 12, bottom: 8 })).show_inside(ui, |ui| {
                egui::Frame::new().stroke(Stroke::new(1.0_f32, t.border)).corner_radius(10).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let rows: [(&str, &str, u8); 9] = [("person", "Followers", 10), ("search", "Find in channel", 11), ("reply", "Replies", 2), ("pin", "Pinned messages", 1), ("bookmark", "Saved", 4), ("attach", "Files", 5), ("link", "Links", 6), ("task", "Turned into tasks", 3), ("settings", "Settings", 12)];
                    for (icon, label, code) in rows {
                        let w = ui.available_width();
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                        let sel = code < 10 && self.channel_panel == code;
                        if sel {
                            ui.painter().rect_filled(r, 6.0, t.row_selected);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, 6.0, t.hover);
                        }
                        let at = if icons_only { r.center() - vec2(12.0, 0.0) } else { r.left_center() };
                        if code == 10 {
                            paint_avatars(ui.painter(), at + vec2(4.0, 0.0), 8.0, &channel.members.iter().take(1).copied().collect::<Vec<_>>(), &self.data, t.bg);
                        } else {
                            line_icon(ui.painter(), icon, at + vec2(12.0, 0.0), t.text2);
                        }
                        let resp = if icons_only {
                            name_it(&resp, label);
                            resp.on_hover_text(tr(label))
                        } else {
                            let count_w = if code == 10 { 24.0 } else { 0.0 };
                            paint_fit(ui, r.left_center() + vec2(28.0, 0.0), Align2::LEFT_CENTER, tr(label), FontId::proportional(14.0), t.text, w - 32.0 - count_w);
                            if code == 10 {
                                ui.painter().text(r.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, channel.members.len().to_string(), FontId::proportional(12.0), t.text3);
                            }
                            resp
                        };
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
            // the chips start under the Channel tab and clear the line above,
            // level with the box on the right
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                if ui.add(egui::Button::new(RichText::new(tr("Add bookmarks, notes, and more")).size(13.5).color(t.text2)).corner_radius(14)).clicked() {
                    self.modal = Some(Modal::Bookmark { title: String::new(), url: String::new() });
                }
                if let Some(mut q) = self.channel_find.take() {
                    let r = ui.add(field(&mut q).id(find_id).hint_text(tr("Find in channel")).desired_width(200.0));
                    // Escape in it closes it, and every message shows again
                    if !(r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape))) {
                        self.channel_find = Some(q);
                    }
                }
                if self.channel_panel != 0 {
                    let label = match self.channel_panel {
                        1 => tr("Pinned messages"),
                        2 => tr("Messages with replies"),
                        4 => tr("Saved messages"),
                        5 => tr("Messages with files"),
                        6 => tr("Messages with links"),
                        _ => tr("Messages turned into tasks"),
                    };
                    if ui.add(egui::Button::new(format!("{}  \u{00D7}", label)).corner_radius(14)).clicked() {
                        self.channel_panel = 0;
                    }
                }
            });
            // a gap and a hairline under the chips, so a message scrolled up
            // under them reads as cut off by the list, not covered by a chip
            ui.add_space(4.0);
            let line = ui.available_rect_before_wrap();
            ui.painter().hline(line.left() + 14.0..=line.right() - 14.0, line.top(), Stroke::new(0.5_f32, t.border));
            ui.add_space(2.0);
            let composer_h = (140.0_f32 + if self.chat.files.is_empty() { 0.0 } else { 76.0 }).max(composer_seen(ui, ("chat_composer_h", cid)));
            let q = self.channel_find.clone().unwrap_or_default().to_lowercase();
            let panel = self.channel_panel;
            let matching: Vec<&Message> = at
                .iter()
                .filter_map(|&(c, m)| self.data.channels.get(c)?.messages.get(m))
                .filter(|m| q.is_empty() || m.text.to_lowercase().contains(&q) || typed_post_title(m).is_some_and(|t| t.to_lowercase().contains(&q)))
                .filter(|m| match panel {
                    1 => m.pinned,
                    2 => !m.replies.is_empty(),
                    3 => m.task.is_some(),
                    4 => m.saved || m.replies.iter().any(|r| r.saved),
                    5 => has_files(m),
                    6 => has_links(m),
                    _ => true,
                })
                .collect();
            // the newest messages; earlier ones a page at a time on asking
            let mut limit = match self.chat.window {
                Some((c, n)) if c == cid => n,
                _ => CHAT_PAGE,
            };
            // a message to go to is shown, however far back it is
            if let Some(id) = self.chat.jump_to {
                match matching.iter().position(|m| m.id == id) {
                    Some(i) if matching.len() - i > limit => {
                        limit = matching.len() - i;
                        self.chat.window = Some((cid, limit));
                    }
                    Some(_) => {}
                    None => self.chat.jump_to = None,
                }
            }
            // (going to a message, the list isn't held at its bottom; the
            // light goes out on time even when its message isn't drawn)
            let now = ui.input(|i| i.time);
            if self.chat.flash.is_some_and(|(_, _, at)| now - at >= FLASH_SECS) {
                self.chat.flash = None;
            }
            let going = self.chat.jump_to.is_some() || self.chat.flash.is_some_and(|(c, _, _)| c == cid);
            let earlier = matching.len().saturating_sub(limit);
            let shown: Vec<Message> = matching[earlier..].iter().map(|m| (*m).clone()).collect();
            let list = egui::ScrollArea::vertical().id_salt(("chat_scroll", cid)).max_height(ui.available_height() - composer_h).auto_shrink([false, false]).stick_to_bottom(!going).show(ui, |ui| {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 10)).show(ui, |ui| {
                    if earlier > 0 {
                        ui.vertical_centered(|ui| {
                            let label = if earlier == 1 { tr("Show 1 earlier message").to_string() } else { trf!("Show {} earlier messages", earlier.min(CHAT_PAGE)) };
                            if ui.add(egui::Button::new(RichText::new(label).size(13.0).color(t.text2)).corner_radius(14)).clicked() {
                                self.chat.window = Some((cid, limit + CHAT_PAGE));
                            }
                        });
                        ui.add_space(6.0);
                    }
                    if no_messages {
                        // empty state, as in the original
                        ui.add_space(50.0);
                        let w = ui.available_width();
                        let cw = 400.0f32.min(w - 40.0);
                        let pad = ((w - cw) / 2.0).max(0.0);
                        let name = if channel.kind == ChannelKind::Channel { format!("#{}", channel.name) } else { self.channel_label(&channel) };
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_width(cw);
                                // a direct message is with someone, not in a place
                                let head = if channel.kind == ChannelKind::Channel { trf!("Chat in {}", name) } else { trf!("Chat with {}", name) };
                                ui.label(RichText::new(head).size(16.0).color(t.text));
                                ui.label(RichText::new(tr("Collaborate seamlessly across tasks and conversations. Start chatting with your team or connect tasks to stay on top of your work.")).color(t.text2));
                                ui.add_space(10.0);
                                // alone in the workspace there is nobody to add: invite instead
                                if self.data.solo() {
                                    if self.may_manage() && ui.add(egui::Button::new(tr("+  Invite people")).min_size(vec2(cw, CONTROL_H))).clicked() {
                                        self.modal = Some(Modal::Invite { emails: String::new(), role: "Member".into() });
                                    }
                                } else if ui.add(egui::Button::new(tr("+  Add People")).min_size(vec2(cw, CONTROL_H))).clicked() {
                                    self.modal = Some(Modal::ChannelSettings(cid));
                                }
                                // it brings people in: the owner's and admins' on a server
                                if self.may_manage() && icon_button_ex(ui, &t, "download", tr("Import from Slack"), "Import from Slack", true, cw, t.text, t.text2).clicked() {
                                    self.start_slack_import();
                                }
                                ui.add_space(8.0);
                                ui.separator();
                                for (bg, icon, title, sub) in [([0xb6, 0x60, 0xe0], "list", "Track Tasks", "Manage tasks, bugs, people, and more"), ([0x10, 0x90, 0xe0], "docs", "Add Doc", "Take notes or create detailed documents"), ([0x00, 0x88, 0x44], "phone", "Start SyncUp", "Jump on a voice call or video call")] {
                                    let (r, resp) = ui.allocate_exact_size(vec2(cw, 54.0), Sense::click());
                                    ui.painter().rect_filled(r, 8.0, rgb(bg).gamma_multiply(if resp.hovered() { 0.14 } else { 0.07 }));
                                    let ib = Rect::from_center_size(r.left_center() + vec2(26.0, 0.0), vec2(34.0, 34.0));
                                    ui.painter().rect_filled(ib, 8.0, t.card);
                                    line_icon(ui.painter(), icon, ib.center(), rgb(bg));
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
                        let d = local_date(m.ts);
                        if last_day != Some(d) {
                            last_day = Some(d);
                            ui.add_space(6.0);
                            let w = ui.available_width();
                            let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
                            ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0_f32, t.border));
                            let label = match (d - today()).num_days() {
                                0 => tr("Today").to_string(),
                                -1 => tr("Yesterday").to_string(),
                                _ => d.text("%A, %B %-d"),
                            };
                            let g = ui.painter().layout_no_wrap(label, FontId::proportional(11.5), t.text2);
                            let pill = Rect::from_center_size(r.center(), g.size() + vec2(20.0, 8.0));
                            ui.painter().rect_filled(pill, 11.0, t.bg);
                            ui.painter().rect_stroke(pill, 11.0, Stroke::new(1.0_f32, t.border), egui::StrokeKind::Inside);
                            ui.painter().galley(pill.center() - g.size() / 2.0, g, t.text2);
                            prev = None;
                        }
                        if line_at == Some(m.id) {
                            read_line(ui, &t);
                            prev = None;
                        }
                        let compact = prev.map_or(false, |(a, ts)| a == m.author && m.ts - ts < 300) && m.post_title.is_none() && m.call.is_none();
                        let top = ui.cursor().top();
                        let light = ui.painter().add(egui::Shape::Noop);
                        self.message_row_c(ui, cid, m, true, compact, &mut acts);
                        self.went_to(ui, cid, m.id, top, light);
                        prev = Some((m.author, m.ts));
                    }
                    // just sent: down to it, even from higher up the list
                    if std::mem::take(&mut self.chat.to_bottom) {
                        ui.scroll_to_cursor(Some(Align::BOTTOM));
                    }
                });
            });
            crate::widgets::redo_when_stuck_list_changes(ui.ctx(), &list);
            // scrolled up while messages came in: a quiet note over the
            // composer that takes the list down to them
            let at_bottom = list.state.offset.y + list.inner_rect.height() >= list.content_size.y - 24.0;
            self.chat.at_bottom = at_bottom;
            if at_bottom || self.chat.bottom_seen.is_none_or(|(c, _)| c != cid) {
                self.chat.bottom_seen = Some((cid, newest));
            } else {
                let since = self.chat.bottom_seen.map_or(newest, |(_, ts)| ts);
                let me = self.data.me;
                let below = at
                    .iter()
                    .map(|&(c, m)| &self.data.channels[c].messages[m])
                    .filter(|m| m.author != me && m.imported.is_none() && m.ts > since)
                    .count();
                if below > 0 && new_below(ui, &t, list.inner_rect, below) {
                    self.chat.to_bottom = true;
                }
            }
            let composer_top = ui.cursor().top();
            egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 12, top: 4, bottom: 2 }).show(ui, |ui| {
                let target = match channel.kind {
                    ChannelKind::Channel => channel.name.clone(),
                    ChannelKind::Direct => self.channel_label(&channel),
                };
                if let Some((text, title, files)) = self.channel_composer(ui, &target) {
                    match title {
                        Some(tt) => acts.push(ChatAction::SendPost { ch: cid, title: tt, text, files }),
                        None => acts.push(ChatAction::Send { ch: cid, text, parent: None, files }),
                    }
                }
                self.typing_row(ui, &channel, None);
            });
            composer_took(ui, ("chat_composer_h", cid), composer_top);
        });
        // only the part they will go to lights up: the thread beside the
        // conversation is a box of its own (a task open over the chat takes
        // them itself)
        if dragging && self.open_task.is_none() {
            // the window gets no mouse moves while files are dragged over it
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
            let at = super::files::drag_pointer(ui.ctx());
            match thread_rect.filter(|r| at.is_some_and(|p| r.contains(p))) {
                Some(r) => drop_hint(ui.ctx(), &t, r, tr("Drop files to reply")),
                None => drop_hint(ui.ctx(), &t, conv.response.rect, tr("Drop files to send")),
            }
        }

        // mark as read (unless the person just marked it unread and is still
        // here, or the window is behind another app: what comes in meanwhile
        // isn't seen yet and counts on the Dock until they come back)
        if self.chat.keep_unread == Some(cid) || self.notify_rt.away {
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

    /// Leaving a conversation closes its thread panel; a thread in its own
    /// window stays open.
    /// After drawing message `id` (from `top` down to the cursor): the one
    /// to go to comes into the middle of the list at once and is lit up
    /// (in the `light` shape under it), the light fading out over a moment.
    fn went_to(&mut self, ui: &mut Ui, cid: u64, id: u64, top: f32, light: egui::layers::ShapeIdx) {
        let lit = self.chat.flash.is_some_and(|(c, f, _)| c == cid && f == id);
        if self.chat.jump_to != Some(id) && !lit {
            return;
        }
        let row = Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top());
        let now = ui.input(|i| i.time);
        if self.chat.jump_to == Some(id) {
            self.chat.jump_to = None;
            self.chat.flash = Some((cid, id, now));
            ui.scroll_to_rect_animation(
                row,
                Some(Align::Center),
                egui::style::ScrollAnimation::none(),
            );
        }
        let Some((_, _, at)) = self.chat.flash else {
            return;
        };
        let left = 1.0 - ((now - at) / FLASH_SECS) as f32;
        if left <= 0.0 {
            self.chat.flash = None;
            return;
        }
        ui.painter().set(
            light,
            egui::Shape::rect_filled(
                row.expand2(vec2(6.0, 2.0)),
                6.0,
                WARN_ORANGE.gamma_multiply(0.25 * left),
            ),
        );
        ui.ctx().request_repaint();
    }

    pub(super) fn leave_thread(&mut self) {
        if self.chat.thread_window.is_none() {
            self.chat.thread_open = None;
        }
    }

    /// Escape closes the thread, unless something else takes the key first:
    /// a menu, a dialog, a picture being viewed, a message being changed, or
    /// a text field other than the message boxes (the thread's draft stays).
    /// `in_window`: the key went to the thread's own window, where only the
    /// thread's fields are.
    fn escape_closes_thread(&self, ctx: &egui::Context, in_window: bool) -> bool {
        let boxes = [
            egui::Id::new(super::files::CHAT_BOX),
            egui::Id::new(super::files::THREAD_BOX),
        ];
        let typing_elsewhere =
            !in_window && self.typing_was_on && !self.focus_was.is_some_and(|f| boxes.contains(&f));
        ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && !self.menu_was_open
            && !typing_elsewhere
            && self.chat.edit.is_none()
            && self.modal.is_none()
            && self.viewer.is_none()
            && self.open_task.is_none()
            && self.palette.is_none()
            && self.quick_add.is_none()
            && self.rename.is_none()
            && !self.new_message_open
    }

    /// Opens a thread of conversation `ch` (None: leaves the open one, as
    /// [`Self::leave_thread`]). With the thread window open it shows there.
    pub(super) fn show_thread(&mut self, ch: u64, thread: Option<u64>) {
        match thread {
            Some(tid) => {
                self.chat.thread_open = Some(tid);
                if self.chat.thread_window.is_some() {
                    self.chat.thread_window = Some(ch);
                }
            }
            None => self.leave_thread(),
        }
    }

    /// The thread's message in conversation `cid` or a copy of it: (the
    /// copy it is in, the message).
    fn thread_parent(&self, cid: u64, tid: u64) -> Option<(u64, Message)> {
        let c = self.data.channels.iter().find(|c| c.id == cid)?;
        self.conversation(c).iter().find_map(|d| {
            d.messages
                .iter()
                .find(|m| m.id == tid)
                .map(|m| (d.id, m.clone()))
        })
    }

    /// The thread: its header, the message and replies, the reply box. In
    /// the panel beside the conversation (`in_window` false) or its own window.
    #[allow(clippy::too_many_arguments)]
    fn thread_body(
        &mut self,
        ui: &mut Ui,
        cid: u64,
        channel: &Channel,
        tid: u64,
        parent: &Message,
        in_window: bool,
        acts: &mut Vec<ChatAction>,
    ) {
        let t = self.t();
        let row = ui.horizontal(|ui| {
            let title = ui.label(RichText::new(tr("Thread")).size(15.0).strong());
            let place = ui.label(RichText::new(self.channel_label(channel)).color(t.text3));
            let buttons = ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let close = icon_button(ui, &t, "close", "", "Close thread", false);
                if close.clicked() {
                    acts.push(ChatAction::CloseThread);
                }
                if in_window {
                    let back = icon_button(ui, &t, "sidebar", "", "Back to the sidebar", false)
                        .on_hover_text(tr("Back to the sidebar"));
                    if back.clicked() {
                        acts.push(ChatAction::DockThread);
                    }
                } else {
                    let out = icon_button(ui, &t, "pop_out", "", "Open in new window", false)
                        .on_hover_text(tr("Open in new window"));
                    if out.clicked() {
                        // the window opens where the panel is, as big
                        let at = ui
                            .ctx()
                            .input(|i| i.viewport().inner_rect)
                            .map(|w| ui.clip_rect().translate(w.min.to_vec2()));
                        acts.push(ChatAction::ThreadWindow { ch: cid, at });
                    }
                }
                if self.ai_offered() && !parent.replies.is_empty() {
                    let b = icon_button_ex(
                        ui,
                        &t,
                        "sparkle",
                        "",
                        "Summarize thread",
                        false,
                        0.0,
                        t.text2,
                        super::ai::AI_VIOLET,
                    )
                    .on_hover_text(tr("Sum up this thread with AI"));
                    if b.clicked() {
                        self.ai_summarize_thread(tid);
                    }
                }
            });
            title.rect.union(place.rect).left()..=buttons.response.rect.left()
        });
        if !in_window {
            // the header pulls the thread out into its own window
            let xs = row.inner;
            let grip =
                Rect::from_x_y_ranges(*xs.start()..=(*xs.end() - 4.0), row.response.rect.y_range());
            let id = ui.id().with("thread_grip");
            let drag = ui.interact(grip, id, Sense::drag());
            name_it(&drag, "Drag the thread out to a new window");
            if drag.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            if drag.drag_started() {
                if let Some(p) = drag.interact_pointer_pos() {
                    ui.data_mut(|d| d.insert_temp(id, p));
                }
            }
            let from: Option<egui::Pos2> = ui.data(|d| d.get_temp(id));
            let at = ui.ctx().input(|i| i.pointer.latest_pos());
            if drag.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                // what goes where the pointer is: the panel, faint
                if let (Some(p), Some(f)) = (at, from) {
                    let panel = ui.clip_rect();
                    let ghost_r = panel.translate(p - f);
                    let painter = ui
                        .ctx()
                        .layer_painter(egui::LayerId::new(egui::Order::Tooltip, id.with("ghost")));
                    painter.rect(
                        ghost_r,
                        8.0,
                        t.column_bg.gamma_multiply(0.75),
                        Stroke::new(1.0_f32, t.border),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            if drag.drag_stopped() {
                ui.data_mut(|d| d.remove::<egui::Pos2>(id));
                // dragged far enough (not a slip of the hand): the window
                // opens where the panel was dropped
                if let (Some(p), Some(f)) = (at.or(drag.interact_pointer_pos()), from) {
                    if (p - f).length() > 40.0 {
                        let rect = ui
                            .ctx()
                            .input(|i| i.viewport().inner_rect)
                            .map(|w| ui.clip_rect().translate(w.min.to_vec2() + (p - f)));
                        acts.push(ChatAction::ThreadWindow { ch: cid, at: rect });
                    }
                }
            }
        }
        ui.separator();
        // the box, the typing line under it and "Also send to",
        // all inside the window; one row of tiles when pictures
        // wait in the box
        let composer_h = (140.0_f32
            + if self.chat.thread_files.is_empty() {
                0.0
            } else {
                76.0
            })
        .max(composer_seen(ui, ("thread_composer_h", tid)));
        let list = egui::ScrollArea::vertical()
            .id_salt("thread_scroll")
            .max_height(ui.available_height() - composer_h)
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                self.message_row(ui, cid, parent, false, acts);
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
                    ui.painter()
                        .hline(r.x_range(), r.center().y, Stroke::new(1.0_f32, t.border));
                });
                for r in &parent.replies {
                    self.message_row(ui, cid, r, false, acts);
                }
                if std::mem::take(&mut self.chat.thread_to_bottom) {
                    ui.scroll_to_cursor(Some(Align::BOTTOM));
                }
            });
        crate::widgets::redo_when_stuck_list_changes(ui.ctx(), &list);
        let composer_top = ui.cursor().top();
        if let Some((text, files)) = self.composer(ui, tr("Reply..."), true) {
            acts.push(ChatAction::Send {
                ch: cid,
                text,
                parent: Some(tid),
                files,
            });
        } else if self.chat.thread_typed {
            // the others in the thread see that I'm typing
            self.typing_now(cid, Some(tid));
        }
        // who else is answering in this thread
        self.typing_row(ui, channel, Some(tid));
        // the reply also goes in the conversation, marked as one
        let place = match channel.kind {
            ChannelKind::Channel => format!("#{}", channel.name),
            ChannelKind::Direct => self.channel_label(channel),
        };
        let also = check(
            ui,
            &mut self.chat.also_channel,
            trf!("Also send to {}", place),
        );
        name_it(&also, "Also send to the channel");
        composer_took(ui, ("thread_composer_h", tid), composer_top);
    }

    /// The thread moved out to its own window: a separate window of the
    /// system, moved and sized on its own (inside the app's window where
    /// there is only one, as in the click tests).
    pub(super) fn thread_window(&mut self, ctx: &egui::Context) {
        let (Some(cid), Some(tid)) = (self.chat.thread_window, self.chat.thread_open) else {
            self.chat.thread_window = None;
            self.chat.thread_window_focused = false;
            return;
        };
        // its message deleted, the conversation gone or out of reach
        let seen = self
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .is_some_and(|c| self.can_see(c));
        let found = self.thread_parent(cid, tid).filter(|_| seen);
        let Some((pc, parent)) = found else {
            self.chat.thread_window = None;
            self.chat.thread_open = None;
            self.chat.thread_window_focused = false;
            return;
        };
        let messages = self
            .data
            .channels
            .iter_mut()
            .find(|c| c.id == cid)
            .map(|c| std::mem::take(&mut c.messages))
            .unwrap_or_default();
        let ci = self.data.channels.iter().position(|c| c.id == cid);
        let channel = ci
            .map(|i| self.data.channels[i].clone())
            .unwrap_or_default();
        if let Some(i) = ci {
            self.data.channels[i].messages = messages;
        }
        // just moved out of the panel: from the next frame on
        if self.chat.thread_panel_pass == ctx.cumulative_pass_nr() {
            ctx.request_repaint();
            return;
        }
        let t = self.t();
        let title = format!("{} \u{00B7} {}", tr("Thread"), self.channel_label(&channel));
        let mut builder = egui::ViewportBuilder::default()
            .with_title(title)
            .with_min_inner_size([320.0, 360.0]);
        builder = match self.chat.thread_window_at {
            Some(r) => builder.with_position(r.min).with_inner_size(r.size()),
            None => builder.with_inner_size([THREAD_W + 40.0, 640.0]),
        };
        let mut acts: Vec<ChatAction> = vec![];
        let mut closed = false;
        let mut focused = false;
        let frame = egui::Frame::new()
            .fill(t.column_bg)
            .inner_margin(egui::Margin::symmetric(14, 10));
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("thread_window"),
            builder,
            |ctx, class| {
                if class == egui::ViewportClass::Embedded {
                    egui::Window::new("thread_window")
                        .title_bar(false)
                        .collapsible(false)
                        .default_size([THREAD_W + 40.0, 600.0])
                        // over the right of the app's window, where the panel was
                        .default_pos(ctx.screen_rect().right_top() + vec2(-THREAD_W - 80.0, 60.0))
                        .frame(
                            frame
                                .stroke(Stroke::new(1.0_f32, t.border))
                                .corner_radius(10),
                        )
                        .show(ctx, |ui| {
                            ui.set_min_height(560.0);
                            self.thread_body(ui, cid, &channel, tid, &parent, true, &mut acts);
                        });
                    closed = self.escape_closes_thread(ctx, false);
                    return;
                }
                let win = egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                    self.thread_body(ui, cid, &channel, tid, &parent, true, &mut acts);
                });
                if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
                    drop_hint(ctx, &t, win.response.rect, tr("Drop files to reply"));
                }
                // pictures and videos dragged in from Finder go in its reply box
                let (dropped, close, front) = ctx.input(|i| {
                    (
                        i.raw
                            .dropped_files
                            .iter()
                            .filter_map(|f| f.path.clone())
                            .collect::<Vec<_>>(),
                        i.viewport().close_requested(),
                        i.viewport().focused.unwrap_or(false),
                    )
                });
                for f in dropped {
                    self.add_media(true, Some(f), None);
                }
                closed = close || (front && self.escape_closes_thread(ctx, true));
                focused = front;
            },
        );
        self.chat.thread_window_focused = focused;
        if focused || !self.notify_rt.away {
            self.mark_thread_read(pc, &parent);
        }
        if closed {
            acts.push(ChatAction::CloseThread);
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
                ("chat", "Message"),
                ("megaphone", "Post"),
                ("sparkle", "Ask"),
            ]
            .iter()
            .enumerate()
            {
                let sel = self.composer_mode == i as u8;
                // the chosen tab is outlined, the others plain
                let b = icon_button_ex(
                    ui,
                    &t,
                    icon,
                    tr(label),
                    label,
                    sel,
                    0.0,
                    if sel { t.text } else { t.text2 },
                    t.text2,
                );
                if b.clicked() {
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
            .stroke(Stroke::new(1.0_f32, t.border))
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
                    // the megaphone in the box says this goes out as a post, not a message
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (r, icon) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        line_icon(ui.painter(), "megaphone", r.center(), t.primary);
                        name_it(&icon, "Post mode");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.chat.post_title)
                                .hint_text(tr("Title"))
                                .font(FontId::proportional(15.0))
                                .frame(false)
                                .desired_width(f32::INFINITY),
                        );
                    });
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
                format_keys(ui, text_id, &mut self.chat.text);
                let names = self.mention_names();
                let mut lay = |ui: &Ui, s: &str, w: f32| mention_layout(ui, s, w, &names, &t);
                let r = capped(ui, text_id, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.chat.text)
                            .id(text_id)
                            .hint_text(hint)
                            .frame(false)
                            .desired_rows(1)
                            .desired_width(f32::INFINITY)
                            .layouter(&mut lay),
                    )
                });
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Message composer")
                });
                // the others in the conversation see that I'm typing
                if r.changed() && !self.chat.text.trim().is_empty() {
                    if let Some(cid) = self.chat.open {
                        self.typing_now(cid, None);
                    }
                }
                // "/" at the start lists the commands (integrations.rs)
                if let Some(pick) = slash_list(ui, &t, &self.slash_choices(&self.chat.text)) {
                    self.chat.text = format!("/{} ", pick);
                    move_cursor_to_end(ui.ctx(), text_id, &self.chat.text);
                }
                // typing "@" at the start of a word offers the people to mention
                let mention_popup = egui::Id::new("chat_mention_popup");
                if r.changed() && self.chat.text.ends_with('@') {
                    let before = self.chat.text[..self.chat.text.len() - 1].chars().last();
                    if before.map_or(true, char::is_whitespace) {
                        ui.memory_mut(|m| m.open_popup(mention_popup));
                    }
                }
                // Up in an empty box changes my last message here
                if r.has_focus()
                    && self.chat.text.is_empty()
                    && self.composer_mode == 0
                    && ui.input(|i| i.key_pressed(egui::Key::ArrowUp))
                {
                    let me = self.data.me;
                    let last = self
                        .chat
                        .open
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
                        self.chat.edit = last;
                    }
                }
                // a time picked from the clock (None: Custom time...)
                let mut schedule: Option<Option<i64>> = None;
                let mut insert: Option<String> = None;
                ui.horizontal(|ui| {
                    let tool = |ui: &mut Ui, g: &str, tip: &str| {
                        let plus = g == "+";
                        let text = if plus {
                            crate::app::icons::sign_text(false)
                        } else {
                            RichText::new(g).color(t.text2)
                        };
                        let b = ui.add(ghost(egui::Button::new(text)));
                        if plus {
                            crate::app::icons::paint_sign(ui, &b, false, Some(t.text2));
                        }
                        let b = b.on_hover_text(tr(tip));
                        name_it(&b, tip);
                        b
                    };
                    if tool(ui, "+", "Add files").clicked() {
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
                    let nobody = people.is_empty();
                    let people = mention_matches(
                        people,
                        trailing_mention(&self.chat.text).map_or("", |m| m.1),
                    );
                    let teams = self.team_names();
                    let mut picked = None;
                    popup(ui, &at, mention_popup, 210.0, |ui| {
                        if people.is_empty() {
                            let why = if nobody {
                                tr("No one else here yet")
                            } else {
                                tr("No one by that name")
                            };
                            ui.label(RichText::new(why).color(t.text3));
                        }
                        for m in &people {
                            if menu_row(ui, &t, m.shown(), false, |p, c| {
                                paint_avatar(p, c, 8.0, m, t.card)
                            })
                            .clicked()
                            {
                                insert = Some(format!("@{} ", m.shown()));
                                picked = Some(m.id);
                                close_popup(ui);
                            }
                        }
                        if let Some(g) = group_mention_rows(ui, &t, &teams) {
                            insert = Some(g);
                            close_popup(ui);
                        }
                    });
                    if let Some(id) = picked {
                        self.chat.picked.push(id);
                    }
                    let itool = |ui: &mut Ui, kind: &str, tip: &str| {
                        icon_button(ui, &t, kind, "", tip, false).on_hover_text(tr(tip))
                    };
                    let em = itool(ui, "smile", "Emoji");
                    popup(ui, &em, egui::Id::new("chat_emoji_popup"), 248.0, |ui| {
                        if let Some(e) = emoji_grid(ui) {
                            insert = Some(e.to_string());
                            close_popup(ui);
                        }
                    });
                    format_buttons(ui, &t, text_id, &mut self.chat.text, "");
                    if itool(ui, "task", "Create task").clicked() {
                        self.open_quick_add(None);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Post in words; a message sends with the paper plane drawn on it
                        let label = if self.composer_mode == 1 {
                            tr("Post")
                        } else {
                            ""
                        };
                        // pictures still going up, or still being copied,
                        // don't hold the message: they follow it
                        let (files_ok, files) = self.pending_ready(false);
                        let ready = files_ok
                            && (!self.chat.text.trim().is_empty()
                                || !self.chat.files.is_empty()
                                || (self.composer_mode == 1
                                    && !self.chat.post_title.trim().is_empty()));
                        let b = Tint(
                            egui::Button::new(RichText::new(label).color(if ready {
                                t.on_primary
                            } else {
                                t.text3
                            }))
                            .fill(if ready { t.primary } else { t.hover })
                            // round: a circle around the paper plane, a pill around "Post"
                            .corner_radius(15)
                            .min_size(if label.is_empty() {
                                vec2(30.0, 30.0)
                            } else {
                                vec2(34.0, 30.0)
                            }),
                        );
                        let sent = ui.add(b);
                        // the clock: send it later (schedule.rs), plain messages only
                        if self.composer_mode == 0 && !self.chat.text.trim().is_empty() {
                            let clock = icon_button(ui, &t, "clock", "", "Schedule message", false)
                                .on_hover_text(tr("Schedule message"));
                            popup(ui, &clock, egui::Id::new("schedule_popup"), 220.0, |ui| {
                                ui.label(RichText::new(tr("Send later")).small().color(t.text2));
                                for (label, at) in super::schedule::schedule_choices(now_ts()) {
                                    if menu_row_plain(ui, &t, tr(label), false).clicked() {
                                        schedule = Some(Some(at));
                                        close_popup(ui);
                                    }
                                }
                                if menu_row_plain(ui, &t, tr("Custom time..."), false).clicked() {
                                    schedule = Some(None);
                                    close_popup(ui);
                                }
                            });
                        }
                        if label.is_empty() {
                            name_it(&sent, "Send");
                            line_icon(
                                ui.painter(),
                                "send",
                                sent.rect.center(),
                                if ready { t.on_primary } else { t.text3 },
                            );
                        }
                        if (sent.clicked() || enter) && ready {
                            let text = self.chat.text.trim().to_string();
                            let title = if self.composer_mode == 1 {
                                Some(if self.chat.post_title.trim().is_empty() {
                                    DEFAULT_POST_TITLE.to_string()
                                } else {
                                    self.chat.post_title.trim().to_string()
                                })
                            } else {
                                None
                            };
                            out = Some((text, title, files));
                            self.chat.coming = self.pending_coming(false);
                            // after a post the box goes back to plain messages
                            self.composer_mode = 0;
                            self.chat.text.clear();
                            // the picks go with the message (see apply_chat)
                            self.chat.post_title.clear();
                            self.chat.files.clear();
                        } else if enter && files_ok && self.chat.files.is_empty() {
                            self.chat.text.clear();
                        }
                    });
                });
                match schedule {
                    Some(Some(at)) => {
                        if let Some(ch) = self.chat.open {
                            let text = self.chat.text.clone();
                            let picked = self.chat.picked.clone();
                            if self.schedule_message(ch, None, &text, &picked, at) {
                                self.chat.text.clear();
                                self.chat.picked.clear();
                            }
                        }
                    }
                    Some(None) => {
                        self.modal = Some(Modal::ScheduleAt {
                            date: Some(today()),
                            time: chrono::Local::now().format("%H:%M").to_string(),
                        });
                    }
                    None => {}
                }
                if let Some(text) = insert {
                    // a picked @name replaces the "@" (and what followed it) typed
                    if text.starts_with('@') {
                        if let Some((at, _)) = trailing_mention(&self.chat.text) {
                            self.chat.text.truncate(at);
                        }
                    }
                    self.chat.text.push_str(&text);
                    ui.memory_mut(|m| m.request_focus(text_id));
                    let n = self.chat.text.chars().count();
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
        let mut typed = false;
        egui::Frame::new()
            .stroke(Stroke::new(1.0_f32, t.border))
            .corner_radius(10)
            .inner_margin(egui::Margin::same(8))
            .fill(t.card)
            .show(ui, |ui| {
                self.pending_strip(ui, thread);
                let (files_ok, files) = self.pending_ready(thread);
                let has_files = !(if thread {
                    &self.chat.thread_files
                } else {
                    &self.chat.files
                })
                .is_empty();
                let id = egui::Id::new(if thread {
                    super::files::THREAD_BOX
                } else {
                    super::files::CHAT_BOX
                });
                // the people the @ list offers
                let me = self.data.me;
                let people: Vec<Member> = self
                    .data
                    .members
                    .iter()
                    .filter(|m| !m.invited && m.id != me)
                    .cloned()
                    .collect();
                let so_far = if thread {
                    &self.chat.thread_text
                } else {
                    &self.chat.text
                };
                let nobody = people.is_empty();
                let people = mention_matches(people, trailing_mention(so_far).map_or("", |m| m.1));
                let teams = self.team_names();
                let slash_names = self.slash_choices("/");
                let names = self.mention_names();
                let mut lay = |ui: &Ui, s: &str, w: f32| mention_layout(ui, s, w, &names, &t);
                let mut picked: Option<u64> = None;
                let mut insert: Option<String> = None;
                let text = if thread {
                    &mut self.chat.thread_text
                } else {
                    &mut self.chat.text
                };
                let enter = take_enter(ui, id);
                format_keys(ui, id, text);
                let r = capped(ui, id, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(text)
                            .id(id)
                            .hint_text(hint)
                            .frame(false)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .layouter(&mut lay),
                    )
                });
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
                // "/" at the start lists the commands, as in the channel's box
                let choices = {
                    let typed: &str = text;
                    let mut out = vec![];
                    if typed.starts_with('/') {
                        out = slash_names
                            .iter()
                            .filter(|(n, _)| {
                                typed[1..].chars().all(|c| !c.is_whitespace())
                                    && n.starts_with(&typed[1..].to_lowercase())
                            })
                            .cloned()
                            .collect();
                    }
                    out
                };
                if let Some(pick) = slash_list(ui, &t, &choices) {
                    *text = format!("/{} ", pick);
                    move_cursor_to_end(ui.ctx(), id, text);
                }
                typed = r.changed() && !text.trim().is_empty();
                // typing "@" at the start of a word offers the people to
                // mention, as in the channel's box
                let mention_popup = egui::Id::new(("reply_mention_popup", thread));
                if r.changed() && text.ends_with('@') {
                    let before = text[..text.len() - 1].chars().last();
                    if before.map_or(true, char::is_whitespace) {
                        ui.memory_mut(|m| m.open_popup(mention_popup));
                    }
                }
                let mut pick = false;
                ui.horizontal(|ui| {
                    let b = ui.add(ghost(egui::Button::new(crate::app::icons::sign_text(
                        false,
                    ))));
                    crate::app::icons::paint_sign(ui, &b, false, Some(t.text2));
                    let b = b.on_hover_text(tr("Add files"));
                    b.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            true,
                            "Add files to the reply",
                        )
                    });
                    pick = b.clicked();
                    let at = ui
                        .add(ghost(egui::Button::new(RichText::new("@").color(t.text2))))
                        .on_hover_text(tr("Mention someone"));
                    name_it(&at, "Mention someone in the reply");
                    popup(ui, &at, mention_popup, 210.0, |ui| {
                        if people.is_empty() {
                            let why = if nobody {
                                tr("No one else here yet")
                            } else {
                                tr("No one by that name")
                            };
                            ui.label(RichText::new(why).color(t.text3));
                        }
                        for m in &people {
                            if menu_row(ui, &t, m.shown(), false, |p, c| {
                                paint_avatar(p, c, 8.0, m, t.card)
                            })
                            .clicked()
                            {
                                insert = Some(format!("@{} ", m.shown()));
                                picked = Some(m.id);
                                close_popup(ui);
                            }
                        }
                        if let Some(g) = group_mention_rows(ui, &t, &teams) {
                            insert = Some(g);
                            close_popup(ui);
                        }
                    });
                    format_buttons(ui, &t, id, text, "reply ");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let ready = files_ok && (!text.trim().is_empty() || has_files);
                        if (primary_button(ui, &t, tr("Send")).clicked() || enter) && ready {
                            out = Some((text.trim().to_string(), files));
                            text.clear();
                        } else if enter && !has_files {
                            text.clear();
                        }
                    });
                });
                if let Some(new) = insert {
                    // a picked @name replaces the "@" (and what followed it) typed
                    if new.starts_with('@') {
                        if let Some((at, _)) = trailing_mention(text) {
                            text.truncate(at);
                        }
                    }
                    text.push_str(&new);
                    ui.memory_mut(|m| m.request_focus(id));
                    let n = text.chars().count();
                    let mut st = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
                    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(n),
                    )));
                    st.store(ui.ctx(), id);
                }
                if let Some(m) = picked {
                    // the picks go with the reply (see apply_chat)
                    if thread {
                        self.chat.thread_picked.push(m);
                    } else {
                        self.chat.picked.push(m);
                    }
                }
                if out.is_some() {
                    let coming = self.pending_coming(thread);
                    if thread {
                        self.chat.thread_coming = coming;
                        self.chat.thread_files.clear();
                    } else {
                        self.chat.coming = coming;
                        self.chat.files.clear();
                    }
                }
                if pick {
                    self.pick_media(thread);
                }
            });
        if thread {
            self.chat.thread_typed = typed;
        }
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
        let editing = matches!(&self.chat.edit, Some((c, id, _)) if *c == cid && *id == m.id);
        let w = ui.available_width();
        let start = ui.cursor().min;
        // the hover background goes underneath the message, not over it
        let bg = ui.painter().add(egui::Shape::Noop);
        let inner = ui.horizontal_top(|ui| {
            ui.set_width(w);
            // someone in the workspace (not a webhook's or workflow's
            // message): their avatar and name open their profile
            let person =
                (m.bot.is_none() && self.data.member(m.author).is_some()).then_some(m.author);
            let (ar, ar_resp) = ui.allocate_exact_size(
                vec2(36.0, if compact { 18.0 } else { 36.0 }),
                if !compact && person.is_some() {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            if compact {
                if ui.rect_contains_pointer(
                    ar.expand2(vec2(0.0, 6.0))
                        .union(Rect::from_min_size(ar.min, vec2(w, 22.0))),
                ) {
                    let time = local(m.ts).map(|x| x.text("%-I:%M")).unwrap_or_default();
                    ui.painter().text(
                        ar.center(),
                        Align2::CENTER_CENTER,
                        time,
                        FontId::proportional(10.5),
                        t.text3,
                    );
                }
            } else if let Some(au) = self.data.member(m.author) {
                paint_avatar(ui.painter(), ar.center(), 13.0, au, t.bg);
                if let Some(p) = person {
                    let ar_resp = ar_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    if ar_resp.clicked() {
                        acts.push(ChatAction::Profile(p));
                    }
                }
            }
            ui.vertical(|ui| {
                ui.set_width(w - 50.0);
                if !compact {
                    ui.horizontal(|ui| {
                        // a webhook's or command's message shows its name
                        let name = m.bot.clone().unwrap_or_else(|| self.who(m.author));
                        let label = egui::Label::new(RichText::new(name).strong());
                        if let Some(p) = person {
                            let r = ui
                                .add(label.sense(Sense::click()))
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if r.clicked() {
                                acts.push(ChatAction::Profile(p));
                            }
                        } else {
                            ui.add(label);
                        }
                        if m.bot.is_some() {
                            egui::Frame::new()
                                .fill(t.column_bg)
                                .corner_radius(3)
                                .inner_margin(egui::Margin::symmetric(4, 0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(tr("APP")).small().color(t.text2));
                                });
                        }
                        let time = local(m.ts).map(|x| x.text("%-I:%M %p")).unwrap_or_default();
                        ui.label(RichText::new(time).small().color(t.text3));
                        if m.edited.is_some() {
                            ui.label(RichText::new(tr("(edited)")).small().color(t.text3));
                        }
                        if m.saved {
                            let c = WARN_ORANGE;
                            small_icon(ui, "bookmark", c);
                            ui.label(RichText::new(tr("Saved")).small().color(c));
                        }
                        if m.pinned {
                            small_icon(ui, "pin", t.text3);
                            ui.label(RichText::new(tr("Pinned")).small().color(t.text3));
                        }
                    });
                }
                if let Some(pt) = shown_post_title(m) {
                    ui.horizontal_top(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 20.0), Sense::hover());
                        line_icon(ui.painter(), "megaphone", r.center(), t.text2);
                        // wrapped: a long one (a web address) widened the
                        // whole conversation past the window
                        ui.add(egui::Label::new(RichText::new(pt).size(16.0).color(t.text)).wrap());
                    });
                }
                if editing {
                    self.edit_box(ui, cid, m.id, acts);
                } else if !m.text.is_empty() {
                    let text = shown_text(m);
                    if let Some(link) = self.message_text(ui, &text, 14.0) {
                        self.link_pending = Some(link);
                    }
                    if compact && m.edited.is_some() {
                        ui.label(RichText::new(tr("(edited)")).small().color(t.text3));
                    }
                }
                if let Some(f) = &m.forwarded {
                    self.forwarded_card(ui, f);
                }
                if let Some(gc) = &m.goal {
                    if let Some(id) = self.goal_chat_card(ui, gc) {
                        acts.push(ChatAction::OpenGoal(id));
                    }
                }
                if let Some(tid) = m.thread_of {
                    let b = ui.add(
                        egui::Label::new(
                            RichText::new(tr("Replied in a thread · View thread"))
                                .small()
                                .color(t.primary),
                        )
                        .sense(Sense::click()),
                    );
                    name_it(&b, "View thread");
                    if b.clicked() {
                        acts.push(ChatAction::Thread(tid));
                    }
                }
                if let Some(p) = &m.preview {
                    if self.preview_card(ui, p, m.author == me) {
                        acts.push(ChatAction::RemovePreview { ch: cid, msg: m.id });
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
                        .stroke(Stroke::new(1.0_f32, t.border))
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
                        .stroke(Stroke::new(1.0_f32, t.border))
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
                            .stroke(Stroke::new(1.0_f32, t.border))
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
                                        1.0_f32,
                                        if mine { t.primary } else { t.border },
                                    ))
                                    .corner_radius(10),
                            );
                            let names: Vec<String> = rc
                                .users
                                .iter()
                                .filter_map(|u| self.data.member(*u))
                                .map(|x| x.shown().to_string())
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
                    // the people who replied, each once, in the order they joined
                    let mut people: Vec<u64> = vec![];
                    for r in &m.replies {
                        if !people.contains(&r.author) {
                            people.push(r.author);
                        }
                    }
                    ui.horizontal(|ui| {
                        // laid out like the other items so they share one centre line
                        let (ar, _) = ui.allocate_exact_size(
                            vec2(avatars_width(8.0, &people, &self.data), 18.0),
                            Sense::hover(),
                        );
                        paint_avatars(
                            ui.painter(),
                            ar.left_center(),
                            8.0,
                            &people,
                            &self.data,
                            t.bg,
                        );
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
                                Stroke::new(1.0_f32, col),
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
        // The toolbar reaches up over the row above, so the pointer on its way
        // to a button leaves this row and enters that one. The toolbar shown
        // last pass is remembered: while the pointer is on it, it stays and no
        // other row shows its own.
        let bar_key = egui::Id::new(("msg_bar", cid, m.id, top_level));
        let bar_mem = egui::Id::new("chat_msg_bar");
        let pass = ui.ctx().cumulative_pass_nr();
        let on_bar = ui
            .data(|d| d.get_temp::<(egui::Id, Rect, u64)>(bar_mem))
            .filter(|(_, r, p)| p + 1 >= pass && ui.rect_contains_pointer(*r))
            .map(|(k, _, _)| k);
        let hovered = match on_bar {
            Some(k) => k == bar_key,
            None => ui.rect_contains_pointer(rect),
        };
        if !editing && (hovered || menu_open) {
            ui.painter().set(
                bg,
                egui::Shape::rect_filled(rect.expand2(vec2(6.0, 2.0)), 6.0, t.hover),
            );
            let n = QUICK_REACTIONS.len() + 2 + if top_level { 4 } else { 1 };
            let bw = 30.0;
            // it floats over the row's top edge so it hides little of the
            // row; on a one-line row it used to sit right on the text
            let bar_top = (rect.top() - 20.0).max(ui.clip_rect().top() + 2.0);
            // at the row's right end, but inside the conversation that
            // shows: not past its left edge (a narrow one cut off the first
            // reactions) nor its right one
            let right = rect.right().min(ui.clip_rect().right() - 4.0);
            let bar_left = (right - bw * n as f32 - 8.0).max(ui.clip_rect().left() + 2.0);
            let bar = Rect::from_min_size(pos2(bar_left, bar_top), vec2(bw * n as f32 + 4.0, 28.0));
            ui.data_mut(|d| d.insert_temp(bar_mem, (bar_key, bar, pass)));
            ui.painter().rect_filled(bar, 7.0, t.card);
            ui.painter().rect_stroke(
                bar,
                7.0,
                Stroke::new(1.0_f32, t.border),
                egui::StrokeKind::Inside,
            );
            let mut x = bar.left() + 2.0;
            // a reaction shows its emoji; the other buttons a line icon (`label` is its kind)
            let mut button = |ui: &mut Ui, label: &str, tip: &str| -> egui::Response {
                let r = Rect::from_min_size(pos2(x, bar.top() + 2.0), vec2(bw, 24.0));
                x += bw;
                let icon = super::icons::KINDS.contains(&label);
                let b = put_over(
                    ui,
                    r,
                    ghost(egui::Button::new(if icon { "" } else { label })),
                );
                if icon {
                    line_icon(ui.painter(), label, r.center(), t.text2);
                }
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
            let eb = button(ui, "smile", "More reactions");
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
                if button(ui, "reply", "Reply in thread").clicked() {
                    acts.push(ChatAction::Thread(m.id));
                }
                if button(ui, "task", "Create task from message").clicked() {
                    acts.push(ChatAction::Task { ch: cid, msg: m.id });
                }
            }
            if top_level
                && button(ui, "pin", if m.pinned { "Unpin" } else { "Pin message" }).clicked()
            {
                acts.push(ChatAction::Pin { ch: cid, msg: m.id });
            }
            if button(
                ui,
                "bookmark",
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
            let mb = button(ui, "dots", "More actions");
            if mb.clicked() {
                self.chat.delete = None;
            }
            let now = now_ts();
            let tomorrow = local_ts(today() + Duration::days(1), nine_am()).unwrap_or(now + 86400);
            popup(ui, &mb, more_id, 220.0, |ui| {
                let row = |ui: &mut Ui, label: &str| menu_row_plain(ui, &t, tr(label), false);
                if !m.text.is_empty() && row(ui, "Copy text").clicked() {
                    ui.ctx().copy_text(m.text.clone());
                    acts.push(ChatAction::Copied);
                    close_popup(ui);
                }
                if m.call.is_none() && row(ui, "Forward message").clicked() {
                    acts.push(ChatAction::Forward { ch: cid, msg: m.id });
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
                    let armed = self.chat.delete == Some(m.id);
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
                            self.chat.delete = Some(m.id);
                        }
                    }
                }
            });
        }
    }

    /// Changing one of my messages in place: Enter saves, Escape cancels.
    /// Emptying the text deletes the message, unless it has pictures or
    /// another part that still says something.
    fn edit_box(&mut self, ui: &mut Ui, cid: u64, mid: u64, acts: &mut Vec<ChatAction>) {
        let t = self.t();
        let home = self.msg_home(cid, mid);
        let stands = self
            .data
            .channels
            .iter()
            .find(|c| c.id == home)
            .and_then(|c| {
                c.messages
                    .iter()
                    .chain(c.messages.iter().flat_map(|m| m.replies.iter()))
                    .find(|m| m.id == mid)
            })
            .is_some_and(stands_without_text);
        let names = self.mention_names();
        let mut lay = |ui: &Ui, s: &str, w: f32| mention_layout(ui, s, w, &names, &t);
        let Some((_, _, text)) = self.chat.edit.as_mut() else {
            return;
        };
        let id = egui::Id::new(("chat_edit", cid, mid));
        let enter = take_enter(ui, id);
        let r = egui::Frame::new()
            .stroke(Stroke::new(1.0_f32, t.primary))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .fill(t.card)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(text)
                        .id(id)
                        .frame(false)
                        .desired_rows(1)
                        .desired_width(f32::INFINITY)
                        .layouter(&mut lay),
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
        let deletes = text.trim().is_empty() && !stands;
        // emptied: the first Save only asks, a second one deletes
        if !deletes && self.chat.delete == Some(mid) {
            self.chat.delete = None;
        }
        let armed = deletes && self.chat.delete == Some(mid);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(tr(if armed {
                    "Press Enter again to delete, Esc to cancel"
                } else if deletes {
                    "Empty: saving deletes the message"
                } else {
                    "Enter to save, Esc to cancel"
                }))
                .small()
                .color(t.text3),
            );
            if ui.button(tr("Cancel")).clicked() {
                cancel = true;
            }
            let label = if armed {
                "Click again to delete"
            } else if deletes {
                "Delete message"
            } else {
                "Save"
            };
            if primary_button(ui, &t, tr(label)).clicked() {
                save = true;
            }
        });
        if save && armed {
            acts.push(ChatAction::Delete { ch: cid, msg: mid });
        } else if save && deletes {
            self.chat.delete = Some(mid);
        } else if save {
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
    /// The Forward window: pick a conversation, add a note, Forward. True
    /// when it should close.
    pub(super) fn forward_body(
        &mut self,
        ui: &mut Ui,
        ch: u64,
        msg: u64,
        to: &mut Option<u64>,
        note: &mut String,
    ) -> bool {
        let t = self.t();
        let Some(orig) = self
            .data
            .channels
            .iter()
            .find(|c| c.id == ch)
            .and_then(|c| {
                c.messages
                    .iter()
                    .chain(c.messages.iter().flat_map(|m| m.replies.iter()))
                    .find(|m| m.id == msg)
            })
            .cloned()
        else {
            return true;
        };
        // what it forwards
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(10, 6))
            .fill(t.column_bg)
            .corner_radius(6)
            .show(ui, |ui| {
                ui.label(RichText::new(self.who(orig.author)).strong());
                ui.label(truncate(&super::chatfmt::plain(&orig.text), 200));
            });
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Forward to")).small().color(t.text2));
        let places: Vec<(u64, String)> = self
            .data
            .channels
            .iter()
            .filter(|c| self.listed(c))
            .map(|c| {
                let name = match c.kind {
                    ChannelKind::Channel => format!("# {}", c.name),
                    ChannelKind::Direct => self.channel_label(c),
                };
                (c.id, name)
            })
            .collect();
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .id_salt("forward_places")
            .show(ui, |ui| {
                for (id, name) in &places {
                    if radio(ui, *to == Some(*id), name.as_str()).clicked() {
                        *to = Some(*id);
                    }
                }
            });
        ui.add_space(6.0);
        let n = ui.add(
            egui::TextEdit::multiline(note)
                .hint_text(tr("Add a message (optional)"))
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        name_it(&n, "Forward note");
        ui.add_space(8.0);
        let mut close = false;
        ui.horizontal(|ui| {
            let go = ui.add_enabled(to.is_some(), egui::Button::new(tr("Forward")));
            name_it(&go, "Forward");
            if go.clicked() {
                if let Some(dest) = *to {
                    self.forward(dest, &orig, ch, note.trim());
                    self.toast(tr("Forwarded"));
                    close = true;
                }
            }
            if ui.button(tr("Cancel")).clicked() {
                close = true;
            }
        });
        close
    }

    /// Post `orig` (of channel `from`) in channel `dest` with my `note`.
    pub(super) fn forward(&mut self, dest: u64, orig: &Message, from: u64, note: &str) {
        let me = self.data.me;
        let id = self.data.new_id();
        let mentions = self.chat_mentions(note, dest, &[]);
        // a forwarded forward carries the first message
        let carried = orig.forwarded.clone().unwrap_or(Forwarded {
            channel: from,
            message: orig.id,
            author: orig.author,
            ts: orig.ts,
            text: orig.text.clone(),
        });
        let m = Message {
            id,
            author: me,
            text: note.to_string(),
            ts: now_ts(),
            mentions,
            forwarded: Some(carried),
            ..serde_json::from_value(serde_json::json!({"id": 0, "author": 0, "text": "", "ts": 0}))
                .expect("an empty message")
        };
        if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == dest) {
            c.messages.push(m);
            c.last_read = c.last_read.max(now_ts());
            self.dirty = true;
        }
    }

    /// A forwarded message under a message's text: who wrote it, where and
    /// when, and what it said. A click opens where it came from.
    fn forwarded_card(&mut self, ui: &mut Ui, f: &Forwarded) {
        let t = self.t();
        let from = self
            .data
            .channels
            .iter()
            .find(|c| c.id == f.channel && self.can_see(c))
            .map(|c| (c.id, self.channel_label(c)));
        let inner = egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 10,
                right: 8,
                top: 4,
                bottom: 4,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(self.who(f.author)).strong());
                    let when = local(f.ts)
                        .map(|x| x.text("%b %-d, %-I:%M %p"))
                        .unwrap_or_default();
                    ui.label(RichText::new(when).small().color(t.text3));
                });
                if let Some(link) = self.message_text(ui, &f.text, 14.0) {
                    self.link_pending = Some(link);
                }
                if let Some((cid, label)) = &from {
                    let b = ui.add(
                        egui::Label::new(
                            RichText::new(trf!("Forwarded from {}", label))
                                .small()
                                .color(t.text3),
                        )
                        .sense(Sense::click()),
                    );
                    name_it(&b, "Forwarded from");
                    if b.clicked() {
                        self.chat.open = Some(*cid);
                        self.leave_thread();
                    }
                }
            });
        let r = inner.response.rect;
        ui.painter().rect_filled(
            Rect::from_min_size(r.min, vec2(3.0, r.height())),
            1.5,
            t.border,
        );
    }

    /// Drafts: every message I started and haven't sent; a click opens its
    /// conversation (or thread) with it in the box. True when it should close.
    pub(super) fn drafts_body(&mut self, ui: &mut Ui) -> bool {
        let t = self.t();
        let drafts: Vec<ChatDraft> = self
            .chat_drafts()
            .into_iter()
            .filter(|d| {
                self.data
                    .channels
                    .iter()
                    .any(|c| c.id == d.channel && self.can_see(c))
            })
            .collect();
        if drafts.is_empty() {
            ui.label(
                RichText::new(tr("Messages you start and don't send show up here.")).color(t.text3),
            );
            return false;
        }
        let mut open = None;
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .show(ui, |ui| {
                for d in &drafts {
                    let Some(c) = self.data.channels.iter().find(|c| c.id == d.channel) else {
                        continue;
                    };
                    let mut place = match c.kind {
                        ChannelKind::Channel => format!("#{}", c.name),
                        ChannelKind::Direct => self.channel_label(c),
                    };
                    if d.thread.is_some() {
                        place = trf!("Thread in {}", place);
                    }
                    let r = egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(6, 4))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(&place).small().color(t.text2));
                            ui.label(truncate(d.text.lines().next().unwrap_or(""), 80));
                        })
                        .response
                        .interact(Sense::click());
                    name_it(&r, &format!("Draft in {}", place));
                    if r.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if r.clicked() {
                        open = Some((d.channel, d.thread));
                    }
                }
            });
        if let Some((ch, thread)) = open {
            self.section = Section::Chat;
            self.chat.open = Some(ch);
            self.show_thread(ch, thread);
            return true;
        }
        false
    }

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
                    let who = self.who(m.author);
                    let body = shown_post_title(m)
                        .filter(|x| !x.is_empty())
                        .unwrap_or_else(|| m.text.clone());
                    let body = if body.is_empty() && !m.attachments.is_empty() {
                        if m.attachments.iter().any(|a| a.kind == MediaKind::File) {
                            tr("File").to_string()
                        } else {
                            tr("Picture or video").to_string()
                        }
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
                            Stroke::new(1.0_f32, t.border),
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
        self.chat.open = Some(ch);
        self.show_thread(ch, parent);
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
                ChatAction::Profile(id) => {
                    // in the thread's place
                    self.chat.profile = Some(id);
                    self.leave_thread();
                }
                ChatAction::Thread(id) => {
                    self.chat.profile = None;
                    // with the thread window open the thread shows in it
                    if let Some(ch) = self.chat.open {
                        self.show_thread(ch, Some(id));
                    } else {
                        self.chat.thread_open = Some(id);
                    }
                }
                ChatAction::CloseThread => {
                    self.chat.thread_open = None;
                    self.chat.thread_window = None;
                }
                ChatAction::ThreadWindow { ch, at } => {
                    if self.chat.thread_open.is_some() {
                        self.chat.thread_window = Some(ch);
                        self.chat.thread_window_at = at;
                    }
                }
                ChatAction::DockThread => {
                    // back beside its conversation
                    if let Some(ch) = self.chat.thread_window.take() {
                        self.section = Section::Chat;
                        self.chat.open = Some(ch);
                    }
                }
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
                ChatAction::OpenGoal(id) => {
                    self.section = Section::Goals;
                    self.goal_open = Some(id);
                }
                ChatAction::Forward { ch, msg } => {
                    self.modal = Some(Modal::Forward {
                        ch,
                        msg,
                        to: None,
                        note: String::new(),
                    });
                }
                ChatAction::RemovePreview { ch, msg } => {
                    if let Some(m) = self
                        .data
                        .channels
                        .iter_mut()
                        .find(|c| c.id == ch)
                        .and_then(|c| find_msg(&mut c.messages, msg))
                    {
                        if m.author == me && m.preview.take().is_some() {
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
                    self.chat.to_bottom = true;
                    self.send_files(&files);
                    // the channel's box: the people picked from its @ list
                    let picked = std::mem::take(&mut self.chat.picked);
                    let id = self.add_post(ch, title, "Announcement", text, files, &picked);
                    for p in std::mem::take(&mut self.chat.coming) {
                        self.chat.following.push((ch, vec![id], p));
                    }
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
                        priority: DEFAULT_PRIORITY,
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
                        self.chat.edit = Some((ch, msg, text));
                    }
                    self.chat.delete = None;
                }
                ChatAction::SaveEdit { ch, msg, text } => {
                    self.chat.edit = None;
                    let found = self
                        .data
                        .channels
                        .iter()
                        .find(|c| c.id == ch)
                        .and_then(|c| {
                            c.messages
                                .iter()
                                .chain(c.messages.iter().flat_map(|m| m.replies.iter()))
                                .find(|m| m.id == msg)
                        });
                    // no text left: only a message with pictures (or the like) keeps going
                    if text.is_empty() && !found.is_some_and(stands_without_text) {
                        continue;
                    }
                    let before = self
                        .data
                        .channels
                        .iter()
                        .find(|c| c.id == ch)
                        .and_then(|c| {
                            c.messages
                                .iter()
                                .chain(c.messages.iter().flat_map(|m| m.replies.iter()))
                                .find(|m| m.id == msg)
                        })
                        .map(|m| m.mentions.clone())
                        .unwrap_or_default();
                    let mentions = self.chat_mentions(&text, ch, &before);
                    let mut refetch: Option<String> = None;
                    if let Some(m) = self
                        .data
                        .channels
                        .iter_mut()
                        .find(|c| c.id == ch)
                        .and_then(|c| find_msg(&mut c.messages, msg))
                    {
                        if m.author == me && m.text != text {
                            m.mentions = mentions;
                            m.text = text;
                            m.edited = Some(now_ts());
                            self.dirty = true;
                            // a preview of an address no longer in it goes; a new one comes
                            let url = super::chatfmt::first_url(&m.text);
                            if m.preview.as_ref().map(|p| &p.url) != url.as_ref() {
                                m.preview = None;
                                refetch = url.map(|_| m.text.clone());
                            }
                        }
                    }
                    if let Some(text) = refetch {
                        self.fetch_preview(ch, msg, &text);
                    }
                }
                ChatAction::CancelEdit => {
                    self.chat.edit = None;
                    self.chat.delete = None;
                }
                ChatAction::Delete { ch, msg } => {
                    self.chat.delete = None;
                    if self.chat.edit.as_ref().is_some_and(|e| e.1 == msg) {
                        self.chat.edit = None;
                    }
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        if let Some(i) = c
                            .messages
                            .iter()
                            .position(|m| m.id == msg && m.author == me)
                        {
                            // a message with a thread takes its replies with it
                            c.messages.remove(i);
                            if self.chat.thread_open == Some(msg) {
                                self.chat.thread_open = None;
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
                    // the red line moves up to it
                    let from = ids
                        .iter()
                        .filter_map(|id| self.data.channels.iter().find(|c| c.id == *id))
                        .flat_map(|c| c.messages.iter())
                        .filter(|m| m.ts >= ts)
                        .min_by_key(|m| m.ts)
                        .map(|m| m.id);
                    if let Some(open) = self.chat.open {
                        self.chat.read_line = Some((open, from));
                    }
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
                            self.chat.keep_unread = Some(ch);
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
                        .map(|m| (m.author, m.text.clone(), shown_post_title(m)));
                    let Some((author, text, title)) = found else {
                        continue;
                    };
                    let who = self.who(author);
                    let body = title.filter(|x| !x.is_empty()).unwrap_or(text);
                    // the reminder is this person's own: in their language
                    let what = trf!(
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
                            self.toast(tr("Reminder created"));
                        }
                        None => {
                            self.modal = Some(Modal::reminder(what, today()));
                            self.reminder_about = (None, Some((ch, msg)));
                        }
                    }
                }
                ChatAction::Copied => self.toast(tr("Copied")),
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
                        .cloned();
                    // a plain file isn't opened large: it's saved to Downloads
                    if let Some(a) = file.as_ref().filter(|a| a.kind == MediaKind::File) {
                        self.save_file(a);
                    } else {
                        if let Some(f) = file.map(|a| a.file).filter(|f| {
                            matches!(self.files.state(f), Some(super::files::Transfer::Failed(_)))
                        }) {
                            self.files.retry(&f);
                        }
                        self.viewer = Some(super::files::Viewer::new(ch, msg, index));
                    }
                }
                ChatAction::Send {
                    ch,
                    text,
                    parent,
                    files,
                } => {
                    // files the box was sent with before they were ready
                    let coming = std::mem::take(if parent.is_none() {
                        &mut self.chat.coming
                    } else {
                        &mut self.chat.thread_coming
                    });
                    // "/name ...": a command (integrations.rs)
                    let mut text = text;
                    if text.starts_with('/') && files.is_empty() && coming.is_empty() {
                        use super::integrations::Slash;
                        match self.slash(ch, parent, &text) {
                            Slash::Plain => {}
                            Slash::Done => continue,
                            Slash::Post(t) => text = t,
                            Slash::Unknown(t) => {
                                if parent.is_none() {
                                    self.chat.text = t;
                                } else {
                                    self.chat.thread_text = t;
                                }
                                continue;
                            }
                        }
                    }
                    if self.chat.keep_unread == Some(ch) {
                        self.chat.keep_unread = None;
                    }
                    // nothing to show yet: it goes with the first file ready
                    // (files.rs follow_prepared), so the others never get an
                    // empty message
                    if text.trim().is_empty() && files.is_empty() && !coming.is_empty() {
                        let also_channel =
                            parent.is_some() && std::mem::take(&mut self.chat.also_channel);
                        self.chat.held.push(super::HeldMessage {
                            ch,
                            parent,
                            also_channel,
                            coming,
                        });
                        if parent.is_none() {
                            self.chat.picked.clear();
                        } else {
                            self.chat.thread_picked.clear();
                        }
                        continue;
                    }
                    if parent.is_none() {
                        self.chat.to_bottom = true;
                    } else {
                        self.chat.thread_to_bottom = true;
                    }
                    self.send_files(&files);
                    let id = self.data.new_id();
                    // the people picked from the @ list of the box it was
                    // written in: the channel's, or the thread's
                    let picked = if parent.is_none() {
                        std::mem::take(&mut self.chat.picked)
                    } else {
                        std::mem::take(&mut self.chat.thread_picked)
                    };
                    let mentions = self.chat_mentions(&text, ch, &picked);
                    let m = Message {
                        goal: None,
                        edited: None,
                        imported: None,
                        preview: None,
                        forwarded: None,
                        thread_of: None,
                        bot: None,
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
                    let sent = m.text.clone();
                    // "Also send to the channel": a copy there, marked with its thread
                    let copy = (parent.is_some() && std::mem::take(&mut self.chat.also_channel))
                        .then(|| Message {
                            id: self.data.new_id(),
                            thread_of: parent,
                            ..m.clone()
                        });
                    let ids: Vec<u64> = std::iter::once(id)
                        .chain(copy.as_ref().map(|c| c.id))
                        .collect();
                    for p in coming {
                        self.chat.following.push((ch, ids.clone(), p));
                    }
                    if let Some(c) = self.data.channels.iter_mut().find(|c| c.id == ch) {
                        match parent {
                            Some(pid) => {
                                if let Some(p) = c.messages.iter_mut().find(|x| x.id == pid) {
                                    p.replies.push(m);
                                }
                            }
                            None => c.messages.push(m),
                        }
                        c.messages.extend(copy);
                        c.last_read = c.last_read.max(now_ts());
                    }
                    self.dirty = true;
                    self.fetch_preview(ch, id, &sent);
                    // workflows this channel's new message starts (workflows.rs)
                    if parent.is_none() {
                        self.run_workflows(ch, id);
                    }
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
                    self.chat.open = Some(id);
                    self.leave_thread();
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
                    self.chat.open = Some(id);
                    self.leave_thread();
                }
            }
        }
    }
}

/// Calls and huddles post a message whose text is kept in English, so every
/// device and the iPhone app read the same thing; it is put in the reader's
/// language where it is shown. Anything else is what the person wrote.
/// The red "New" line above the first message I hadn't read.
fn read_line(ui: &mut Ui, t: &Theme) {
    ui.add_space(2.0);
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
    name_it(&resp, "Read up to here");
    let g = ui
        .painter()
        .layout_no_wrap(tr("New").into(), FontId::proportional(11.0), t.danger);
    // the word in the middle, the line on both sides of it
    let half = g.size().x / 2.0 + 8.0;
    let s = Stroke::new(1.0_f32, t.danger);
    ui.painter()
        .hline(r.left()..=r.center().x - half, r.center().y, s);
    ui.painter()
        .hline(r.center().x + half..=r.right(), r.center().y, s);
    ui.painter()
        .galley(r.center() - g.size() / 2.0, g, t.danger);
}

/// "N new messages" in grey at the foot of the message list `list`, for
/// messages that came in below where it's scrolled to. True when clicked.
fn new_below(ui: &mut Ui, t: &Theme, list: Rect, n: usize) -> bool {
    let label = if n == 1 {
        tr("1 new message").to_string()
    } else {
        trf!("{} new messages", n)
    };
    let g = ui
        .painter()
        .layout_no_wrap(label, FontId::proportional(12.5), t.text2);
    let size = vec2(g.size().x + 42.0, 28.0);
    let r = Rect::from_center_size(
        pos2(list.center().x, list.bottom() - 10.0 - size.y / 2.0),
        size,
    );
    let resp = ui
        .interact(r, ui.id().with("new_below"), Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    name_it(&resp, "New messages");
    let p = ui.painter();
    p.rect_filled(
        r.translate(vec2(0.0, 1.0)).expand(1.0),
        15.0,
        Color32::from_black_alpha(14),
    );
    p.rect_filled(r, 14.0, t.card);
    if resp.hovered() {
        p.rect_filled(r, 14.0, t.hover);
    }
    p.rect_stroke(
        r,
        14.0,
        Stroke::new(1.0_f32, t.border),
        egui::StrokeKind::Inside,
    );
    p.galley(
        pos2(r.left() + 14.0, r.center().y - g.size().y / 2.0),
        g,
        t.text2,
    );
    // a small arrow down
    let c = pos2(r.right() - 17.0, r.center().y);
    let s = Stroke::new(1.4_f32, t.text2);
    p.line_segment([c + vec2(0.0, -4.5), c + vec2(0.0, 4.0)], s);
    p.line_segment([c + vec2(-3.5, 0.5), c + vec2(0.0, 4.0)], s);
    p.line_segment([c + vec2(3.5, 0.5), c + vec2(0.0, 4.0)], s);
    resp.clicked()
}

/// The title the app gives a post sent without one. It is stored in English
/// like typed text, so it is put in the reader's language where it is shown.
pub(super) const DEFAULT_POST_TITLE: &str = "Update";

/// A post's title as someone wrote it: not blank and not the app's own
/// default title (which nobody wrote, so isn't searched or matched).
pub(super) fn typed_post_title(m: &Message) -> Option<&str> {
    m.post_title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != DEFAULT_POST_TITLE)
}

/// A post's title as shown: the app's own default title in the reader's
/// language, anything typed as it was typed. A post that came without a
/// title (the iPhone app used to send none with text only) shows the
/// default title too.
pub(super) fn shown_post_title(m: &Message) -> Option<String> {
    m.post_title.as_deref().map(|t| match t.trim() {
        "" | DEFAULT_POST_TITLE => tr(DEFAULT_POST_TITLE).to_string(),
        _ => t.to_string(),
    })
}

pub(super) fn shown_text(m: &Message) -> String {
    match m.text.as_str() {
        t @ ("Started a call" | "Started a huddle") if m.call.is_some() => tr(t).to_string(),
        t => t.to_string(),
    }
}

/// A 16 pt line icon in a row of text (before "Saved", "Pinned").
fn small_icon(ui: &mut Ui, kind: &str, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    line_icon(ui.painter(), kind, r.center(), color);
}

/// The tallest a message box grows; a longer message scrolls inside it, so
/// the box never pushes the conversation and its own buttons off the window.
pub(super) const COMPOSER_MAX_H: f32 = 200.0;

/// A message box that grows with its text up to `COMPOSER_MAX_H`, then scrolls.
fn capped<R>(ui: &mut Ui, id: egui::Id, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::ScrollArea::vertical()
        .id_salt(id.with("cap"))
        .max_height(COMPOSER_MAX_H)
        .auto_shrink([false, true])
        .show(ui, add)
        .inner
}

/// How tall the message box and what's under it were last frame, so the
/// list above leaves exactly that much room.
fn composer_seen(ui: &Ui, key: impl std::hash::Hash) -> f32 {
    ui.ctx()
        .data(|d| d.get_temp::<f32>(egui::Id::new(key)))
        .unwrap_or(0.0)
}

fn composer_took(ui: &Ui, key: impl std::hash::Hash, top: f32) {
    let id = egui::Id::new(key);
    let h = ui.cursor().top() - top + ui.spacing().item_spacing.y;
    let before = ui.ctx().data(|d| d.get_temp::<f32>(id));
    if before.map_or(true, |b| (b - h).abs() > 0.5) {
        ui.ctx().data_mut(|d| d.insert_temp(id, h));
        ui.ctx().request_repaint();
    }
}

/// The box over where files dragged in from Finder will go when let go.
fn drop_hint(ctx: &egui::Context, t: &Theme, r: Rect, label: &str) {
    let p = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("drop_hint"),
    ));
    p.rect_filled(r.shrink(8.0), 12.0, t.primary.gamma_multiply(0.12));
    p.rect_stroke(
        r.shrink(8.0),
        12.0,
        Stroke::new(2.0_f32, t.primary),
        egui::StrokeKind::Inside,
    );
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(18.0),
        t.text,
    );
}

/// Where the "@name" being typed at the end of `text` starts, and the
/// letters typed after its "@" (see `mention_word`): a name picked from the
/// @ list replaces them.
pub(super) fn trailing_mention(text: &str) -> Option<(usize, &str)> {
    super::mention_word(text).map(|w| (text.len() - w.len() - 1, w))
}

/// `people` narrowed to the names starting with `query`, as the task
/// comments' @ list does.
fn mention_matches(people: Vec<Member>, query: &str) -> Vec<Member> {
    let q = query.to_lowercase();
    people
        .into_iter()
        .filter(|m| m.any_name(|n| n.to_lowercase().starts_with(&q)))
        .collect()
}

/// Where the @mentions are in `text` (byte ranges, the @ included): a
/// known name, even one with spaces, else the word after the @.
pub(super) fn mention_spans(text: &str, names: &[String]) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if rest.starts_with('@') && (i == 0 || text[..i].ends_with(char::is_whitespace)) {
            let after = &rest[1..];
            let len =
                wb_sync::mentions::name_at(after, names.iter().map(String::as_str)).or_else(|| {
                    Some(after.find(char::is_whitespace).unwrap_or(after.len())).filter(|w| *w > 0)
                });
            if let Some(len) = len {
                out.push((i, i + 1 + len));
                i += 1 + len;
                continue;
            }
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    out
}

/// A text box's layouter: the text as typed, with each @mention in the
/// tint it gets once sent, so the writer sees who it reaches. Font and
/// colour are the box's own defaults.
pub(super) fn mention_layout(
    ui: &Ui,
    text: &str,
    wrap: f32,
    names: &[String],
    t: &Theme,
) -> std::sync::Arc<egui::Galley> {
    let font = egui::FontSelection::default().resolve(ui.style());
    let color = ui
        .visuals()
        .override_text_color
        .unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
    let plain = egui::TextFormat::simple(font, color);
    let (fg, bg) = t.mention();
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap;
    let mut at = 0;
    for (a, b) in mention_spans(text, names) {
        if at < a {
            job.append(&text[at..a], 0.0, plain.clone());
        }
        let tint = egui::TextFormat {
            color: fg,
            background: bg,
            ..plain.clone()
        };
        job.append(&text[a..b], 0.0, tint);
        at = b;
    }
    if at < text.len() || job.sections.is_empty() {
        job.append(&text[at..], 0.0, plain);
    }
    ui.fonts(|f| f.layout_job(job))
}

/// The tabs over a channel when I haven't changed them.
pub(super) const CHANNEL_TABS: [&str; 4] = ["Channel", "List", "Board", "Calendar"];

impl App {
    /// The tabs over this channel in my order: "Channel" and view names.
    pub(super) fn channel_tabs(&self, cid: u64) -> Vec<String> {
        match self.data.extra.channel_tabs.get(&cid) {
            Some(v) => v.clone(),
            None => CHANNEL_TABS.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Keeps a new order (or set) of this channel's tabs; the default one is
    /// not written down.
    pub(super) fn set_channel_tabs(&mut self, cid: u64, tabs: Vec<String>) {
        if tabs == CHANNEL_TABS {
            self.data.extra.channel_tabs.remove(&cid);
        } else {
            self.data.extra.channel_tabs.insert(cid, tabs);
        }
        self.dirty = true;
    }
}

/// Where the tab dragged from `from` lands with the mouse at `p`: its place
/// in the row once it is taken out and put back. The tabs can wrap onto more
/// rows; above or below them the mouse counts as on the nearest row.
pub(super) fn tab_drop_place(rects: &[Rect], from: usize, p: egui::Pos2) -> usize {
    let (Some(first), Some(last)) = (rects.first(), rects.last()) else {
        return from;
    };
    let y = p.y.clamp(first.top() + 1.0, last.bottom() - 1.0);
    rects
        .iter()
        .enumerate()
        .filter(|(i, r)| *i != from && (y > r.bottom() || (y >= r.top() && p.x > r.center().x)))
        .count()
}
