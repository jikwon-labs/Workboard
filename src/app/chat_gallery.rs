// A conversation's files gathered in one place (the channel panel's Files):
// its pictures and videos as a grid, its other files as a list, newest
// first, thread replies' files too.

use super::chat::ChatAction;
use super::*;

/// One sent file: the conversation copy holding its message, the message
/// and which of its attachments.
pub(super) struct SentFile {
    pub ch: u64,
    pub msg: u64,
    pub index: usize,
    pub att: Attachment,
    pub author: String,
    pub ts: i64,
}

impl App {
    /// The files sent in `msgs` (each with the conversation copy it's in)
    /// and in their threads, newest first; `keep` picks the messages.
    pub(super) fn sent_files<'a>(
        &self,
        msgs: impl Iterator<Item = (u64, &'a Message)>,
        keep: impl Fn(&Message) -> bool,
    ) -> Vec<SentFile> {
        let mut out = vec![];
        for (ch, m) in msgs {
            for x in std::iter::once(m)
                .chain(m.replies.iter())
                .filter(|x| keep(x))
            {
                let author = match &x.bot {
                    Some(b) => b.clone(),
                    None => self.who(x.author),
                };
                for (index, a) in x.attachments.iter().enumerate() {
                    out.push(SentFile {
                        ch,
                        msg: x.id,
                        index,
                        att: a.clone(),
                        author: author.clone(),
                        ts: x.ts,
                    });
                }
            }
        }
        out.sort_by_key(|f| (std::cmp::Reverse(f.ts), f.msg, f.index));
        out
    }

    /// The Files view in place of the messages: "Pictures and videos" or
    /// "Other files", picked over it.
    pub(super) fn files_gallery(
        &mut self,
        ui: &mut Ui,
        cid: u64,
        files: &[SentFile],
        acts: &mut Vec<ChatAction>,
    ) {
        let t = self.t();
        if files.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(tr("No files in this conversation yet")).color(t.text3));
            });
            return;
        }
        let (media, other): (Vec<&SentFile>, Vec<&SentFile>) =
            files.iter().partition(|f| f.att.kind != MediaKind::File);
        let tab_id = egui::Id::new(("chat_gallery_tab", cid));
        // where there are only other files, those
        let mut others = ui
            .data(|d| d.get_temp::<bool>(tab_id))
            .unwrap_or(media.is_empty());
        ui.horizontal(|ui| {
            for (o, label, n) in [
                (false, "Pictures and videos", media.len()),
                (true, "Other files", other.len()),
            ] {
                let text = format!("{}  {}", tr(label), n);
                if ui.selectable_label(others == o, text).clicked() {
                    others = o;
                }
            }
        });
        ui.data_mut(|d| d.insert_temp(tab_id, others));
        ui.add_space(8.0);
        let mut go: Option<(u64, u64)> = None;
        // what each one is and where it was sent, and the way to it
        let about = |ui: &mut Ui, f: &SentFile, go: &mut Option<(u64, u64)>| {
            ui.label(
                RichText::new(format!("{} \u{2022} {}", f.author, fmt_ago(f.ts)))
                    .small()
                    .color(t.text3),
            );
            if ui
                .link(RichText::new(tr("Show in conversation")).small())
                .clicked()
            {
                *go = Some((f.ch, f.msg));
            }
        };
        if !others {
            if media.is_empty() {
                ui.label(RichText::new(tr("No files in this conversation yet")).color(t.text3));
            }
            let side = 132.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for f in &media {
                    let resp = self.media_tile(ui, &f.att, vec2(side, side));
                    if resp.clicked() {
                        acts.push(ChatAction::OpenMedia {
                            ch: f.ch,
                            msg: f.msg,
                            index: f.index,
                        });
                    }
                    let resp = resp.on_hover_text(format!(
                        "{}\n{} \u{2022} {}",
                        f.att.name,
                        f.author,
                        fmt_ago(f.ts)
                    ));
                    resp.context_menu(|ui| {
                        if ui.button(tr("Show in conversation")).clicked() {
                            go = Some((f.ch, f.msg));
                            ui.close_menu();
                        }
                    });
                }
            });
        } else {
            if other.is_empty() {
                ui.label(RichText::new(tr("No files in this conversation yet")).color(t.text3));
            }
            let avail = ui.available_width();
            for f in &other {
                ui.horizontal(|ui| {
                    if self.file_card(ui, &f.att, avail.min(320.0)) {
                        acts.push(ChatAction::OpenMedia {
                            ch: f.ch,
                            msg: f.msg,
                            index: f.index,
                        });
                    }
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.add_space(6.0);
                        about(ui, f, &mut go);
                    });
                });
                ui.add_space(6.0);
            }
        }
        if let Some((ch, msg)) = go {
            self.show_message(ch, msg);
        }
    }
}
