// "Someone is typing..." in a conversation on a team server: the app tells
// the server every few seconds while its person writes in the message box,
// and shows, under the box, who else the server says is writing there (in a
// direct message also as moving dots by the other person's name in the
// sidebar, which a narrow window hides). The small window shows the line
// too.
// Typing in a thread's reply box shows only under that thread's box.
// Nothing is saved; it lasts a few seconds after the last word from the
// server, or until that person's message arrives.

use super::*;
use std::time::{Duration, Instant};

/// How often the app says its person is still typing.
pub(super) const TYPING_EVERY: Duration = Duration::from_secs(3);

/// How long someone shows as typing after the server last said so (a bit
/// more than TYPING_EVERY, so steady typing shows without a gap).
pub(super) const TYPING_SHOWS: Duration = Duration::from_secs(5);

/// Someone the server said is typing in a conversation.
#[derive(Clone, Debug)]
pub struct Typist {
    pub(super) channel: u64,
    pub(super) member: u64,
    /// the message whose thread they're answering; None in the conversation
    pub(super) thread: Option<u64>,
    pub(super) heard: Instant,
    /// their newest message in that conversation (or reply in that thread)
    /// when it was heard: a newer one means they sent what they were typing
    pub(super) last_msg: Option<u64>,
}

impl App {
    /// The id of `member`'s newest message in conversation `channel`, or
    /// newest reply in the thread of message `thread` there.
    fn newest_by(&self, channel: u64, member: u64, thread: Option<u64>) -> Option<u64> {
        let msgs = &self
            .data
            .channels
            .iter()
            .find(|c| c.id == channel)?
            .messages;
        let msgs = match thread {
            Some(tid) => &msgs.iter().find(|m| m.id == tid)?.replies,
            None => msgs,
        };
        msgs.iter().rev().find(|m| m.author == member).map(|m| m.id)
    }

    /// The server says `member` is typing in `channel` (in the thread of
    /// message `thread`).
    pub(super) fn heard_typing(&mut self, channel: u64, member: u64, thread: Option<u64>) {
        if member == self.data.me {
            return;
        }
        let last_msg = self.newest_by(channel, member, thread);
        let rt = &mut self.sync_rt;
        // one place at a time: typing in a thread ends typing in the channel
        rt.typing
            .retain(|t| !(t.channel == channel && t.member == member));
        rt.typing.push(Typist {
            channel,
            member,
            thread,
            heard: Instant::now(),
            last_msg,
        });
    }

    /// My person typed in the message box of conversation `channel` (or
    /// the reply box of message `thread`'s thread): tell the server, at most
    /// every TYPING_EVERY (only while connected; it isn't kept for later).
    pub(super) fn typing_now(&mut self, channel: u64, thread: Option<u64>) {
        if self.sync_rt.net_status != Some(wb_net::Status::Connected) {
            return;
        }
        let Some(net) = &self.sync_rt.net else {
            return;
        };
        let soon = self
            .sync_rt
            .typed
            .is_some_and(|(c, th, at)| c == channel && th == thread && at.elapsed() < TYPING_EVERY);
        if soon {
            return;
        }
        net.send(wb_net::Msg::Typing {
            channel,
            member: None,
            thread,
        });
        self.sync_rt.typed = Some((channel, thread, Instant::now()));
    }

    /// Who is typing in the conversation shown as `c` (with its duplicate
    /// direct messages), oldest first: heard lately, still in it, and not
    /// having sent a message since.
    pub(super) fn typing_in(&self, c: &Channel) -> Vec<u64> {
        self.typing_where(c, None)
    }

    /// The same for the thread of message `thread` in `c`, or for the
    /// conversation itself with None (typing in a thread isn't counted
    /// there).
    pub(super) fn typing_where(&self, c: &Channel, thread: Option<u64>) -> Vec<u64> {
        let copies: Vec<u64> = self.conversation(c).iter().map(|d| d.id).collect();
        let mut out = vec![];
        for t in &self.sync_rt.typing {
            if !copies.contains(&t.channel)
                || t.thread != thread
                || t.heard.elapsed() >= TYPING_SHOWS
                || self.newest_by(t.channel, t.member, t.thread) != t.last_msg
                || out.contains(&t.member)
            {
                continue;
            }
            out.push(t.member);
        }
        out
    }

    /// The line under the message box: "Bo is typing...", for two people
    /// both names, for more "Several people are typing...". None when
    /// nobody is.
    pub(super) fn typing_line(&self, c: &Channel) -> Option<String> {
        self.typing_line_in(c, None)
    }

    /// The same for the reply box of message `thread`'s thread (None: the
    /// conversation's own box).
    pub(super) fn typing_line_in(&self, c: &Channel, thread: Option<u64>) -> Option<String> {
        let name = |m: u64| {
            self.data
                .member(m)
                .map(|p| p.shown().to_string())
                .unwrap_or_else(|| tr("Someone").to_string())
        };
        match self.typing_where(c, thread).as_slice() {
            [] => None,
            [a] => Some(trf!("{} is typing...", name(*a))),
            [a, b] => Some(trf!("{} and {} are typing...", name(*a), name(*b))),
            _ => Some(tr("Several people are typing...").to_string()),
        }
    }

    /// Three dots going up and down in `r` while the other person of the
    /// direct message `c` writes: drawn after their name in the sidebar's
    /// Direct Messages (the line under the message box shows as well).
    pub(super) fn typing_dots(&self, ui: &mut Ui, c: &Channel, r: Rect) {
        let Some(line) = self.typing_line(c) else {
            return;
        };
        let t = self.t();
        let resp = ui.interact(r, ui.id().with(("typing_dots", c.id)), Sense::hover());
        let time = ui.input(|i| i.time) as f32;
        for k in 0..3 {
            let phase = time * 6.0 - k as f32 * 0.9;
            let lift = phase.sin().max(0.0) * 3.0;
            let p = pos2(r.left() + 6.0 + k as f32 * 7.0, r.center().y + 2.0 - lift);
            ui.painter().circle_filled(p, 2.2, t.text2);
        }
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &line));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }

    /// Draw the typing line (a row kept the same height whether or not
    /// someone is typing, so the message box doesn't jump), and redraw once
    /// it should go away. `thread` is the thread whose reply box it is under
    /// (None: the conversation's own box).
    pub(super) fn typing_row(&mut self, ui: &mut Ui, c: &Channel, thread: Option<u64>) {
        let line = self.typing_line_in(c, thread);
        self.draw_typing_row(ui, line);
    }

    /// The same under the small window's box, for conversation `cid`.
    pub(super) fn typing_row_for(&mut self, ui: &mut Ui, cid: u64) {
        let line = self
            .data
            .channels
            .iter()
            .find(|c| c.id == cid)
            .and_then(|c| self.typing_line_in(c, None));
        self.draw_typing_row(ui, line);
    }

    fn draw_typing_row(&mut self, ui: &mut Ui, line: Option<String>) {
        let t = self.t();
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 16.0), Sense::hover());
        if let Some(line) = line {
            ui.painter().text(
                r.left_center() + vec2(2.0, 0.0),
                Align2::LEFT_CENTER,
                &line,
                FontId::proportional(11.5),
                t.text2,
            );
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &line));
        }
        // forget the old ones; wake up when the next one runs out
        self.sync_rt
            .typing
            .retain(|t| t.heard.elapsed() < TYPING_SHOWS);
        if let Some(left) = self
            .sync_rt
            .typing
            .iter()
            .map(|t| TYPING_SHOWS.saturating_sub(t.heard.elapsed()))
            .min()
        {
            ui.ctx().request_repaint_after(left);
        }
    }
}
