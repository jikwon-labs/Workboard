// "Someone is typing..." in a conversation on a team server: the app tells
// the server every few seconds while its person writes in the message box,
// and shows, under the box, who else the server says is writing there.
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
    pub(super) heard: Instant,
    /// their newest message in that conversation when it was heard: a newer
    /// one means they sent what they were typing
    pub(super) last_msg: Option<u64>,
}

impl App {
    /// The id of `member`'s newest message in conversation `channel`.
    fn newest_by(&self, channel: u64, member: u64) -> Option<u64> {
        self.data
            .channels
            .iter()
            .find(|c| c.id == channel)?
            .messages
            .iter()
            .rev()
            .find(|m| m.author == member)
            .map(|m| m.id)
    }

    /// The server says `member` is typing in `channel`.
    pub(super) fn heard_typing(&mut self, channel: u64, member: u64) {
        if member == self.data.me {
            return;
        }
        let last_msg = self.newest_by(channel, member);
        let rt = &mut self.sync_rt;
        rt.typing
            .retain(|t| !(t.channel == channel && t.member == member));
        rt.typing.push(Typist {
            channel,
            member,
            heard: Instant::now(),
            last_msg,
        });
    }

    /// My person typed in the message box of conversation `channel`: tell
    /// the server, at most every TYPING_EVERY (only while connected; it
    /// isn't kept for later).
    pub(super) fn typing_now(&mut self, channel: u64) {
        if self.sync_rt.net_status != Some(wb_net::Status::Connected) {
            return;
        }
        let Some(net) = &self.sync_rt.net else {
            return;
        };
        let soon = self
            .sync_rt
            .typed
            .is_some_and(|(c, at)| c == channel && at.elapsed() < TYPING_EVERY);
        if soon {
            return;
        }
        net.send(wb_net::Msg::Typing {
            channel,
            member: None,
        });
        self.sync_rt.typed = Some((channel, Instant::now()));
    }

    /// Who is typing in the conversation shown as `c` (with its duplicate
    /// direct messages), oldest first: heard lately, still in it, and not
    /// having sent a message since.
    pub(super) fn typing_in(&self, c: &Channel) -> Vec<u64> {
        let copies: Vec<u64> = self.conversation(c).iter().map(|d| d.id).collect();
        let mut out = vec![];
        for t in &self.sync_rt.typing {
            if !copies.contains(&t.channel)
                || t.heard.elapsed() >= TYPING_SHOWS
                || self.newest_by(t.channel, t.member) != t.last_msg
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
        let name = |m: u64| {
            self.data
                .member(m)
                .map(|p| p.shown().to_string())
                .unwrap_or_else(|| tr("Someone").to_string())
        };
        match self.typing_in(c).as_slice() {
            [] => None,
            [a] => Some(trf!("{} is typing...", name(*a))),
            [a, b] => Some(trf!("{} and {} are typing...", name(*a), name(*b))),
            _ => Some(tr("Several people are typing...").to_string()),
        }
    }

    /// Draw the typing line (a row kept the same height whether or not
    /// someone is typing, so the message box doesn't jump), and redraw once
    /// it should go away.
    pub(super) fn typing_row(&mut self, ui: &mut Ui, c: &Channel) {
        let t = self.t();
        let line = self.typing_line(c);
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
