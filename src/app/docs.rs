// The Doc editor body: a pages panel (pages and sub pages), a formatting
// bar, "/" commands and a rendered view. Doc text is kept as Markdown so
// Export and the shared-folder copy stay plain text.

use super::parity::close_any;
use super::*;

/// "/" commands and formatting choices: (label, line prefix, wrap).
const BLOCKS: [(&str, &str, &str); 13] = [
    ("Heading 1", "# ", ""),
    ("Heading 2", "## ", ""),
    ("Heading 3", "### ", ""),
    ("Bulleted list", "- ", ""),
    ("Numbered list", "1. ", ""),
    ("Checklist", "- [ ] ", ""),
    ("Quote", "> ", ""),
    ("Code block", "```\n", "\n```"),
    ("Divider", "---\n", ""),
    ("Bold", "", "**"),
    ("Italic", "", "*"),
    ("Strikethrough", "", "~~"),
    ("Inline code", "", "`"),
];

/// What the "/" menu offers, filtered by the word typed after the "/".
const SLASH: [&str; 11] = [
    "Heading 1",
    "Heading 2",
    "Heading 3",
    "Bulleted list",
    "Numbered list",
    "Checklist",
    "Quote",
    "Code block",
    "Divider",
    "Table",
    "Sub page",
];

/// Byte offset of the `ci`-th character.
fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

/// Applies a block or wrap at the cursor (character index); returns the new cursor.
pub(super) fn apply_block(body: &mut String, cursor: usize, sel_end: usize, label: &str) -> usize {
    let Some((_, prefix, wrap)) = BLOCKS.iter().find(|b| b.0 == label) else {
        return cursor;
    };
    let (a, b) = (cursor.min(sel_end), cursor.max(sel_end));
    if prefix.is_empty() {
        // wrap the selection (or insert a pair) with ** or *
        let ba = byte_at(body, a);
        let bb = byte_at(body, b);
        body.insert_str(bb, wrap);
        body.insert_str(ba, wrap);
        return b + wrap.chars().count() * if a == b { 1 } else { 2 };
    }
    let ba = byte_at(body, a);
    let line_start = body[..ba].rfind('\n').map(|i| i + 1).unwrap_or(0);
    // replace an existing block prefix on this line
    let line = &body[line_start..];
    let mut strip = 0;
    for (_, p, _) in BLOCKS.iter() {
        if !p.is_empty() && !p.ends_with('\n') && line.starts_with(p) && p.len() > strip {
            strip = p.len();
        }
    }
    body.replace_range(line_start..line_start + strip, "");
    body.insert_str(line_start, prefix);
    // block prefixes are ASCII, so bytes and characters agree
    let c = (a + prefix.chars().count()).saturating_sub(strip);
    if !wrap.is_empty() {
        let at = byte_at(body, c);
        body.insert_str(at, wrap);
    }
    c
}

impl App {
    /// The top page of the Doc a page belongs to.
    pub(super) fn doc_root(&self, id: u64) -> u64 {
        let mut cur = id;
        for _ in 0..32 {
            match self
                .data
                .docs
                .iter()
                .find(|d| d.id == cur)
                .and_then(|d| d.parent)
            {
                Some(p) if self.data.docs.iter().any(|d| d.id == p) => cur = p,
                _ => break,
            }
        }
        cur
    }

    pub(super) fn add_page(&mut self, parent: u64) -> u64 {
        let id = self.data.new_id();
        let now = now_ts();
        let (space, private) = self
            .data
            .docs
            .iter()
            .find(|d| d.id == parent)
            .map(|d| (d.space, d.private))
            .unwrap_or((None, false));
        let n = self
            .data
            .docs
            .iter()
            .filter(|d| d.parent == Some(parent))
            .count()
            + 1;
        self.data.docs.push(Doc {
            id,
            space,
            owner: self.data.me,
            private,
            archived: false,
            meeting_note: false,
            wiki: None,
            views: vec![],
            parent: Some(parent),
            title: format!("Page {}", n),
            body: String::new(),
            created: now,
            updated: now,
            tags: vec![],
        });
        self.doc_open = Some(id);
        // a new page is for writing: open it in Edit, not the last page's View
        self.doc_preview = false;
        self.dirty = true;
        id
    }

    fn page_rows(&mut self, ui: &mut Ui, parent: u64, depth: usize) {
        let t = self.t();
        let kids: Vec<(u64, String)> = self
            .data
            .docs
            .iter()
            .filter(|d| d.parent == Some(parent))
            .map(|d| (d.id, d.title.clone()))
            .collect();
        for (id, title) in kids {
            let sel = self.doc_open == Some(id);
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
            name_it(&resp, &format!("Page {}", title));
            if sel {
                ui.painter().rect_filled(r, 6.0, t.selected_bg);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, 6.0, t.hover);
            }
            let x = r.left() + 10.0 + depth as f32 * 14.0;
            ui.painter().with_clip_rect(r.shrink(2.0)).text(
                pos2(x, r.center().y),
                Align2::LEFT_CENTER,
                if title.is_empty() {
                    tr("Untitled")
                } else {
                    &title
                },
                FontId::proportional(13.0),
                if sel { t.selected_text } else { t.text },
            );
            let plus = Rect::from_center_size(r.right_center() - vec2(12.0, 0.0), vec2(18.0, 18.0));
            let pr = ui.interact(plus, ui.id().with(("subpage", id)), Sense::click());
            if resp.hovered() || pr.hovered() {
                ui.painter().text(
                    plus.center(),
                    Align2::CENTER_CENTER,
                    "+",
                    FontId::proportional(14.0),
                    t.text2,
                );
            }
            if pr.on_hover_text(tr("Add sub page")).clicked() {
                self.add_page(id);
            } else if resp.clicked() {
                self.doc_open = Some(id);
            }
            resp.context_menu(|ui| self.page_menu(ui, id));
            if !self.data.docs.iter().any(|d| d.id == id) {
                // deleted from its menu just now
                return;
            }
            self.page_rows(ui, id, depth + 1);
        }
    }

    /// Pages panel, formatting bar and the editable / rendered body.
    pub(super) fn doc_body(&mut self, ui: &mut Ui, idx: usize) {
        let t = self.t();
        let id = self.data.docs[idx].id;
        let root = self.doc_root(id);
        let full = ui.available_rect_before_wrap();
        let pages_w = 220.0;
        let pages = Rect::from_min_max(full.min, pos2(full.left() + pages_w, full.bottom()));
        let main = Rect::from_min_max(pos2(full.left() + pages_w, full.top()), full.max);
        ui.painter().rect_filled(pages, 0.0, t.sidebar);
        ui.painter()
            .vline(pages.right(), pages.y_range(), Stroke::new(1.0, t.border));
        let mut pui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(pages.shrink(8.0))
                .layout(Layout::top_down(Align::Min)),
        );
        pui.horizontal(|ui| {
            ui.label(RichText::new(tr("Pages")).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("+").on_hover_text(tr("Add page")).clicked() {
                    self.add_page(root);
                }
            });
        });
        pui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("doc_pages")
            .auto_shrink([false, false])
            .show(&mut pui, |ui| {
                let title = self
                    .data
                    .docs
                    .iter()
                    .find(|d| d.id == root)
                    .map(|d| d.title.clone())
                    .unwrap_or_default();
                let sel = self.doc_open == Some(root);
                let w = ui.available_width();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                if sel {
                    ui.painter().rect_filled(r, 6.0, t.selected_bg);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 6.0, t.hover);
                }
                ui.painter().with_clip_rect(r.shrink(2.0)).text(
                    r.left_center() + vec2(10.0, 0.0),
                    Align2::LEFT_CENTER,
                    format!(
                        "\u{1F4C4} {}",
                        if title.is_empty() {
                            tr("Untitled")
                        } else {
                            &title
                        }
                    ),
                    FontId::proportional(13.0),
                    if sel { t.selected_text } else { t.text },
                );
                name_it(&resp, &format!("Page {}", title));
                if resp.clicked() {
                    self.doc_open = Some(root);
                }
                resp.context_menu(|ui| self.doc_menu(ui, root, false));
                if !self.data.docs.iter().any(|d| d.id == root) {
                    return;
                }
                self.page_rows(ui, root, 1);
                ui.add_space(6.0);
                if ui
                    .add(ghost(egui::Button::new(
                        RichText::new(tr("+  Add page")).color(t.text2),
                    )))
                    .clicked()
                {
                    self.add_page(root);
                }
                // table of contents: the open page's headings
                let open = self.doc_open.unwrap_or(root);
                let heads: Vec<(u8, String, usize)> = self
                    .data
                    .docs
                    .iter()
                    .find(|d| d.id == open)
                    .map(|d| doc_headings(&d.body))
                    .unwrap_or_default();
                if !heads.is_empty() {
                    ui.add_space(12.0);
                    ui.label(RichText::new(tr("On this page")).small().color(t.text3));
                    for (level, text, line) in heads {
                        ui.horizontal(|ui| {
                            ui.add_space((level as f32 - 1.0) * 12.0);
                            let r = ui.add(ghost(egui::Button::new(
                                RichText::new(&text).size(12.5).color(t.text2),
                            )));
                            name_it(&r, &format!("Contents {}", text));
                            if r.clicked() {
                                ui.data_mut(|m| {
                                    m.insert_temp(egui::Id::new(("doc_jump", open)), line)
                                });
                            }
                        });
                    }
                }
            });
        // the open page may have been deleted from a menu above
        if !self
            .doc_open
            .is_some_and(|d| self.data.docs.iter().any(|x| x.id == d))
        {
            return;
        }
        // the page may have changed above
        let Some(idx) = self
            .doc_open
            .and_then(|d| self.data.docs.iter().position(|x| x.id == d))
            .or(Some(idx))
        else {
            return;
        };
        let mut mui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(main)
                .layout(Layout::top_down(Align::Min)),
        );
        let ui = &mut mui;
        // in a space shared with me to view or comment, the page only reads
        let level = self.doc_level(self.data.docs[idx].id);
        if level < crate::sync::Level::Edit {
            self.doc_preview = true;
        }
        // formatting bar
        let body_id = egui::Id::new(("doc_body", self.data.docs[idx].id));
        let mut pick: Option<&'static str> = None;
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(16, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.selectable_label(!self.doc_preview, tr("Edit")).clicked() {
                        self.doc_preview = false;
                    }
                    if ui.selectable_label(self.doc_preview, tr("View")).clicked() {
                        self.doc_preview = true;
                    }
                    self.level_note(ui, &t, level);
                    ui.separator();
                    if !self.doc_preview {
                        for (short, label) in [
                            ("H1", "Heading 1"),
                            ("H2", "Heading 2"),
                            ("H3", "Heading 3"),
                            ("B", "Bold"),
                            ("I", "Italic"),
                            ("S", "Strikethrough"),
                            ("`", "Inline code"),
                            ("\u{1F517}", "Link"),
                            ("\u{2022}", "Bulleted list"),
                            ("1.", "Numbered list"),
                            ("\u{2611}", "Checklist"),
                            ("\u{201C}", "Quote"),
                            ("</>", "Code block"),
                            ("\u{2014}", "Divider"),
                        ] {
                            let txt = match short {
                                "B" => RichText::new(short).strong(),
                                "I" => RichText::new(short).italics(),
                                "S" => RichText::new(short).strikethrough(),
                                _ => RichText::new(short),
                            };
                            if ui
                                .add(ghost(egui::Button::new(txt).min_size(vec2(26.0, 24.0))))
                                .on_hover_text(tr(label))
                                .clicked()
                            {
                                pick = Some(label);
                            }
                        }
                        ui.separator();
                        let lr = ui.add(ghost(egui::Button::new(tr("Link task"))));
                        let tasks: Vec<(u64, String)> = self
                            .data
                            .tasks
                            .iter()
                            .filter(|t| t.parent.is_none() && !t.archived)
                            .map(|t| (t.id, t.name.clone()))
                            .collect();
                        popup(ui, &lr, ui.id().with("link_task"), 260.0, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(260.0)
                                .show(ui, |ui| {
                                    for (tid, name) in tasks {
                                        if ui.button(&name).clicked() {
                                            let d = &mut self.data.docs[idx];
                                            let cur: usize = ui
                                                .data(|m| m.get_temp(body_id.with("cursor")))
                                                .unwrap_or(d.body.chars().count());
                                            let at = byte_at(&d.body, cur);
                                            d.body.insert_str(at, &format!("[task:{}]", tid));
                                            d.updated = now_ts();
                                            self.dirty = true;
                                            close_popup(ui);
                                        }
                                    }
                                });
                        });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let d = &self.data.docs[idx];
                        let words = d.body.split_whitespace().count();
                        let author = self
                            .data
                            .member(d.owner)
                            .map(|m| m.name.clone())
                            .unwrap_or_default();
                        ui.label(
                            RichText::new(trf!("{} \u{00B7} {} words", author, words))
                                .small()
                                .color(t.text3),
                        );
                    });
                });
            });
        let r = ui.available_rect_before_wrap();
        ui.painter()
            .hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
        let focused = ui.memory(|m| m.has_focus(body_id));
        let arm_id = body_id.with("slash_at");
        let slash_sel_id = body_id.with("slash_sel");
        let mut slash_enter = false;
        if focused && !self.doc_preview {
            let cmd = |k| egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, k);
            let (b, i, e) = ui.input_mut(|inp| {
                (
                    inp.consume_shortcut(&cmd(egui::Key::B)),
                    inp.consume_shortcut(&cmd(egui::Key::I)),
                    inp.consume_shortcut(&cmd(egui::Key::E)),
                )
            });
            if b {
                pick = Some("Bold");
            } else if i {
                pick = Some("Italic");
            } else if e {
                pick = Some("Inline code");
            }
            if ui.data(|m| m.get_temp::<usize>(arm_id)).is_some() {
                // the "/" menu takes the arrow keys and Enter while it is open
                let (up, down, enter) = ui.input_mut(|inp| {
                    (
                        inp.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                        inp.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                        inp.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                    )
                });
                let sel: usize = ui.data(|m| m.get_temp(slash_sel_id)).unwrap_or(0);
                let sel = if down {
                    sel + 1
                } else if up {
                    sel.saturating_sub(1)
                } else {
                    sel
                };
                ui.data_mut(|m| m.insert_temp(slash_sel_id, sel));
                slash_enter = enter;
            }
            // Tab and Shift+Tab indent and outdent the line (the field keeps
            // focus on Tab, see lock_focus below)
            let (shift_tab, tab) = ui.input_mut(|inp| {
                (
                    inp.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab),
                    inp.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
                )
            });
            if tab || shift_tab {
                let d = &mut self.data.docs[idx];
                let n = d.body.chars().count();
                let cur: usize = ui
                    .data(|m| m.get_temp(body_id.with("cursor")))
                    .unwrap_or(n)
                    .min(n);
                let c = indent_line(&mut d.body, cur, tab);
                d.updated = now_ts();
                self.dirty = true;
                set_cursor(ui.ctx(), body_id, c);
            }
        }
        if let Some(label) = pick {
            let d = &mut self.data.docs[idx];
            let n = d.body.chars().count();
            let cur: usize = ui
                .data(|m| m.get_temp(body_id.with("cursor")))
                .unwrap_or(n)
                .min(n);
            let sel_end: usize = ui
                .data(|m| m.get_temp(body_id.with("sel")))
                .unwrap_or(cur)
                .min(n);
            let c = if label == "Link" {
                apply_link(&mut d.body, cur, sel_end)
            } else {
                apply_block(&mut d.body, cur, sel_end, label)
            };
            d.updated = now_ts();
            self.dirty = true;
            set_cursor(ui.ctx(), body_id, c);
        }
        // a heading picked in "On this page": scroll to its line
        let jump_id = egui::Id::new(("doc_jump", self.data.docs[idx].id));
        let jump: Option<usize> = ui.data_mut(|m| m.remove_temp(jump_id));
        egui::ScrollArea::vertical()
            .id_salt(("doc_scroll", self.data.docs[idx].id))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(60, 24))
                    .show(ui, |ui| {
                        let d = &mut self.data.docs[idx];
                        let r1 = ui.add(
                            egui::TextEdit::singleline(&mut d.title)
                                .font(FontId::proportional(32.0))
                                .frame(false)
                                .hint_text(tr("Untitled"))
                                .desired_width(f32::INFINITY),
                        );
                        if r1.changed() {
                            d.updated = now_ts();
                            self.dirty = true;
                        }
                        if self.data.docs[idx].parent.is_none() {
                            self.doc_tags_row(ui, idx);
                        }
                        ui.add_space(10.0);
                        if self.doc_preview {
                            self.render_markdown(ui, idx, jump);
                            return;
                        }
                        let d = &mut self.data.docs[idx];
                        let empty = d.body.is_empty();
                        let mut layouter = |ui: &Ui, s: &str, wrap: f32| {
                            ui.fonts(|f| f.layout_job(doc_layout_job(s, &t, wrap)))
                        };
                        let out = egui::TextEdit::multiline(&mut d.body)
                            .id(body_id)
                            .frame(false)
                            .lock_focus(true)
                            .hint_text(tr("Start writing, or type '/' for commands"))
                            .font(FontId::proportional(16.0))
                            .desired_width(f32::INFINITY)
                            .desired_rows(if empty { 3 } else { 24 })
                            .layouter(&mut layouter)
                            .show(ui);
                        name_field(&out.response, "Doc body");
                        if out.response.changed() {
                            d.updated = now_ts();
                            self.dirty = true;
                        }
                        // painted marks over the hidden Markdown markers: bullets,
                        // checkboxes (click to tick), quote bars and dividers
                        let click = if out.response.clicked() {
                            ui.input(|i| i.pointer.interact_pos())
                        } else {
                            None
                        };
                        let mut tick: Option<usize> = None;
                        for ((kind, start, _), line) in
                            line_kinds(&d.body).into_iter().zip(d.body.split('\n'))
                        {
                            // list items may be indented; the mark sits at the marker
                            let ind = if matches!(
                                kind,
                                LineKind::Bullet | LineKind::Check(_) | LineKind::Numbered
                            ) {
                                indent_of(line)
                            } else {
                                0
                            };
                            let at = out.galley_pos
                                + out
                                    .galley
                                    .pos_from_cursor(
                                        &out.galley
                                            .from_ccursor(egui::text::CCursor::new(start + ind)),
                                    )
                                    .min
                                    .to_vec2();
                            let row = out.galley.pos_from_cursor(
                                &out.galley.from_ccursor(egui::text::CCursor::new(start)),
                            );
                            let h = row.height();
                            let mid = at.y + h / 2.0;
                            let p = ui.painter();
                            match kind {
                                LineKind::Bullet => {
                                    p.circle_filled(pos2(at.x + 5.0, mid), 2.6, t.text);
                                }
                                LineKind::Check(done) => {
                                    let bx = Rect::from_min_size(
                                        pos2(at.x + 2.0, mid - 7.5),
                                        vec2(15.0, 15.0),
                                    );
                                    if done {
                                        p.rect_filled(bx, 3.0, t.primary);
                                        p.line_segment(
                                            [
                                                bx.left_center() + vec2(3.5, 0.0),
                                                bx.center_bottom() + vec2(-1.0, -4.0),
                                            ],
                                            Stroke::new(1.8, Color32::WHITE),
                                        );
                                        p.line_segment(
                                            [
                                                bx.center_bottom() + vec2(-1.0, -4.0),
                                                bx.right_top() + vec2(-3.5, 4.0),
                                            ],
                                            Stroke::new(1.8, Color32::WHITE),
                                        );
                                    } else {
                                        p.rect_stroke(
                                            bx,
                                            3.0,
                                            Stroke::new(1.3, t.text3),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    if click.is_some_and(|c| bx.expand(2.0).contains(c)) {
                                        tick = Some(start);
                                    }
                                }
                                LineKind::Quote => {
                                    p.rect_filled(
                                        Rect::from_min_size(
                                            pos2(at.x + 1.0, at.y + 1.0),
                                            vec2(3.0, h - 2.0),
                                        ),
                                        1.0,
                                        t.border,
                                    );
                                }
                                LineKind::Divider => {
                                    p.hline(
                                        at.x..=out.response.rect.right(),
                                        mid,
                                        Stroke::new(1.0, t.border),
                                    );
                                }
                                _ => {}
                            }
                        }
                        if let Some(line) = jump {
                            if let Some((_, start, _)) = line_kinds(&d.body).get(line) {
                                let row = out.galley.pos_from_cursor(
                                    &out.galley.from_ccursor(egui::text::CCursor::new(*start)),
                                );
                                ui.scroll_to_rect(
                                    row.translate(out.galley_pos.to_vec2()).expand(4.0),
                                    Some(Align::TOP),
                                );
                            }
                        }
                        if let Some(start) = tick {
                            toggle_check_at(&mut d.body, start);
                            d.updated = now_ts();
                            self.dirty = true;
                        }
                        // Enter on a list item starts the next item (or ends an empty one)
                        let mut cursor_now = out
                            .cursor_range
                            .map(|cr| (cr.primary.ccursor.index, cr.secondary.ccursor.index));
                        if out.response.changed()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)
                        {
                            if let Some((a, _)) = cursor_now.filter(|(a, b)| a == b) {
                                if let Some(c) = continue_list(&mut d.body, a) {
                                    set_cursor(ui.ctx(), body_id, c);
                                    cursor_now = Some((c, c));
                                }
                            }
                        }
                        // a new, empty page offers how to start, as in the original
                        if empty {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add(ghost(egui::Button::new(
                                        RichText::new(tr("Start writing")).color(t.text2),
                                    )))
                                    .clicked()
                                {
                                    set_cursor(ui.ctx(), body_id, 0);
                                }
                            });
                            ui.add_space(6.0);
                            ui.label(RichText::new(tr("Add new")).small().color(t.text3));
                            let mut add_table = false;
                            let mut add_sub = false;
                            ui.horizontal(|ui| {
                                if ui.button(tr("\u{25A6}  Table")).clicked() {
                                    add_table = true;
                                }
                                if ui.button(tr("\u{1F4C4}  Subpage")).clicked() {
                                    add_sub = true;
                                }
                            });
                            if add_table {
                                let d = &mut self.data.docs[idx];
                                d.body.push_str(TABLE_TEMPLATE);
                                d.updated = now_ts();
                                self.dirty = true;
                            }
                            if add_sub {
                                let pid = self.data.docs[idx].id;
                                self.add_page(pid);
                                return;
                            }
                        }
                        let d = &mut self.data.docs[idx];
                        if let Some((a, b)) = cursor_now {
                            ui.data_mut(|m| {
                                m.insert_temp(body_id.with("cursor"), a);
                                m.insert_temp(body_id.with("sel"), b);
                            });
                            // "/" typed at the start of a line opens the command menu; it
                            // stays open while a filter word is typed after it, and while
                            // the menu is clicked (the click takes focus away)
                            let chars: Vec<char> = d.body.chars().collect();
                            let slash_here = a > 0
                                && a == b
                                && chars.get(a - 1) == Some(&'/')
                                && (a == 1 || chars.get(a - 2) == Some(&'\n'));
                            if out.response.has_focus() {
                                let prev: Option<usize> = ui.data(|m| m.get_temp(arm_id));
                                let keep = prev.filter(|s| {
                                    a == b
                                        && *s > 0
                                        && a >= *s
                                        && a <= chars.len()
                                        && chars.get(*s - 1) == Some(&'/')
                                        && a - *s <= 24
                                        && !chars[*s..a].contains(&'\n')
                                });
                                let next = if slash_here { Some(a) } else { keep };
                                ui.data_mut(|m| match next {
                                    Some(s) => {
                                        if prev != Some(s) && slash_here {
                                            m.insert_temp(slash_sel_id, 0usize);
                                        }
                                        m.insert_temp(arm_id, s);
                                    }
                                    None => {
                                        m.remove::<usize>(arm_id);
                                        m.remove::<usize>(slash_sel_id);
                                    }
                                });
                            }
                        }
                        let d = &mut self.data.docs[idx];
                        let chars: Vec<char> = d.body.chars().collect();
                        let armed: Option<usize> = ui
                            .data(|m| m.get_temp(arm_id))
                            .filter(|a: &usize| *a > 0 && chars.get(*a - 1) == Some(&'/'));
                        if let Some(a) = armed {
                            let cur: usize = ui
                                .data(|m| m.get_temp(body_id.with("cursor")))
                                .unwrap_or(a)
                                .clamp(a, chars.len());
                            let query: String = chars[a..cur]
                                .iter()
                                .collect::<String>()
                                .trim()
                                .to_lowercase();
                            let items: Vec<&'static str> = SLASH
                                .iter()
                                .copied()
                                .filter(|l| l.to_lowercase().contains(&query))
                                .collect();
                            let sel: usize = ui
                                .data(|m| m.get_temp(slash_sel_id))
                                .unwrap_or(0)
                                .min(items.len().saturating_sub(1));
                            let ccur = egui::text::CCursor::new(a);
                            let pos = out.galley_pos
                                + out
                                    .galley
                                    .pos_from_cursor(&out.galley.from_ccursor(ccur))
                                    .left_bottom()
                                    .to_vec2()
                                + vec2(0.0, 4.0);
                            let mut chosen: Option<&'static str> = if slash_enter {
                                items.get(sel).copied()
                            } else {
                                None
                            };
                            let area = egui::Area::new(body_id.with("slash"))
                                .order(egui::Order::Foreground)
                                .fixed_pos(pos)
                                .show(ui.ctx(), |ui| {
                                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.set_width(200.0);
                                        ui.label(
                                            RichText::new(tr("Basic blocks"))
                                                .small()
                                                .color(t.text3),
                                        );
                                        if items.is_empty() {
                                            ui.label(
                                                RichText::new(tr("No matching blocks"))
                                                    .color(t.text3),
                                            );
                                        }
                                        for (i, label) in items.iter().enumerate() {
                                            if ui
                                                .add(
                                                    egui::Button::new(tr(label))
                                                        .frame(i == sel)
                                                        .selected(i == sel)
                                                        .min_size(vec2(190.0, 22.0)),
                                                )
                                                .clicked()
                                            {
                                                chosen = Some(label);
                                            }
                                        }
                                    });
                                });
                            let pressed_elsewhere = ui.input(|i| i.pointer.any_pressed())
                                && !ui.ctx().pointer_hover_pos().map_or(false, |p| {
                                    area.response.rect.contains(p) || out.response.rect.contains(p)
                                });
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) || pressed_elsewhere {
                                ui.data_mut(|m| m.remove::<usize>(arm_id));
                            }
                            if slash_enter && items.is_empty() {
                                // nothing to pick: Enter is a plain line break again
                                let at = byte_at(&d.body, cur);
                                d.body.insert(at, '\n');
                                ui.data_mut(|m| m.remove::<usize>(arm_id));
                                set_cursor(ui.ctx(), body_id, cur + 1);
                            }
                            if let Some(label) = chosen {
                                // drop the "/" and the filter word, then apply the block
                                let from = byte_at(&d.body, a - 1);
                                let to = byte_at(&d.body, cur);
                                d.body.replace_range(from..to, "");
                                d.updated = now_ts();
                                self.dirty = true;
                                ui.data_mut(|m| m.remove::<usize>(arm_id));
                                match label {
                                    "Table" => {
                                        d.body.insert_str(from, TABLE_TEMPLATE);
                                        set_cursor(ui.ctx(), body_id, a - 1 + 2);
                                    }
                                    "Sub page" => {
                                        let pid = d.id;
                                        self.add_page(pid);
                                        return;
                                    }
                                    _ => {
                                        let c = apply_block(&mut d.body, a - 1, a - 1, label);
                                        set_cursor(ui.ctx(), body_id, c);
                                    }
                                }
                            }
                        }
                        // sub pages listed under the text, as in the original
                        let kids: Vec<(u64, String)> = self
                            .data
                            .docs
                            .iter()
                            .filter(|x| x.parent == Some(self.data.docs[idx].id))
                            .map(|x| (x.id, x.title.clone()))
                            .collect();
                        if !kids.is_empty() {
                            ui.add_space(16.0);
                            ui.label(RichText::new(tr("Sub pages")).small().color(t.text3));
                            for (kid, title) in kids {
                                if ui.link(format!("\u{1F4C4} {}", title)).clicked() {
                                    self.doc_open = Some(kid);
                                }
                            }
                        }
                    });
            });
    }

    /// Draws the Doc's Markdown: headings, lists (indented too), checklists
    /// (clickable), quotes, code, dividers, inline styles, links and task
    /// links. `jump` is a line to scroll to (from "On this page").
    fn render_markdown(&mut self, ui: &mut Ui, idx: usize, jump: Option<usize>) {
        let t = self.t();
        let body = self.data.docs[idx].body.clone();
        let mut toggle: Option<usize> = None;
        let mut old_toggle: Option<usize> = None;
        let mut open_task: Option<u64> = None;
        let kinds = line_kinds(&body);
        for (ln, line) in body.split('\n').enumerate() {
            let (kind, start, pre) = kinds[ln];
            let top = ui.cursor().top();
            match kind {
                LineKind::Fence => continue,
                LineKind::Code => {
                    egui::Frame::new()
                        .fill(t.column_bg)
                        .inner_margin(egui::Margin::symmetric(8, 2))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(line).monospace());
                        });
                }
                LineKind::Divider => {
                    ui.separator();
                }
                LineKind::Table => {
                    let cells: Vec<&str> = line
                        .trim()
                        .trim_matches('|')
                        .split('|')
                        .map(|c| c.trim())
                        .collect();
                    if cells
                        .iter()
                        .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
                    {
                        continue; // the | --- | row under the header
                    }
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for c in cells {
                            let (r, _) = ui.allocate_exact_size(vec2(160.0, 30.0), Sense::hover());
                            ui.painter().rect_stroke(
                                r,
                                0.0,
                                Stroke::new(1.0, t.border),
                                egui::StrokeKind::Inside,
                            );
                            ui.painter().with_clip_rect(r.shrink(2.0)).text(
                                r.left_center() + vec2(8.0, 0.0),
                                Align2::LEFT_CENTER,
                                c,
                                FontId::proportional(14.0),
                                t.text,
                            );
                        }
                    });
                }
                _ if line.trim().is_empty() => {
                    ui.add_space(8.0);
                }
                _ => {
                    let (size, strong) = match kind {
                        LineKind::Heading(1) => (28.0, true),
                        LineKind::Heading(2) => (22.0, true),
                        LineKind::Heading(3) => (18.0, true),
                        _ => (16.0, false),
                    };
                    let text = &line[byte_at(line, pre)..];
                    ui.horizontal_wrapped(|ui| {
                        // runs of text sit right next to each other
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let mut rest = text;
                        match kind {
                            LineKind::Check(done) => {
                                ui.add_space(indent_of(line) as f32 * 8.0);
                                let mut on = done;
                                if ui.checkbox(&mut on, "").clicked() {
                                    toggle = Some(start);
                                }
                                ui.add_space(4.0);
                            }
                            LineKind::Bullet => {
                                ui.add_space(indent_of(line) as f32 * 8.0);
                                ui.label(RichText::new("  \u{2022}  ").size(size));
                            }
                            LineKind::Numbered => {
                                ui.add_space(indent_of(line) as f32 * 8.0);
                                let num: String = line
                                    .trim_start()
                                    .chars()
                                    .take_while(|c| c.is_ascii_digit())
                                    .collect();
                                ui.label(RichText::new(format!("  {}.  ", num)).size(size));
                            }
                            LineKind::Quote => {
                                let (bar, _) =
                                    ui.allocate_exact_size(vec2(3.0, size + 4.0), Sense::hover());
                                ui.painter().rect_filled(bar, 1.0, t.border);
                                ui.add_space(8.0);
                            }
                            _ => {
                                // checklist lines from older Meeting Notes ("[ ] " with no dash)
                                for (mark, on) in [("[ ] ", false), ("[x] ", true), ("[X] ", true)]
                                {
                                    if let Some(r) = rest.strip_prefix(mark) {
                                        let mut on = on;
                                        if ui.checkbox(&mut on, "").clicked() {
                                            old_toggle = Some(ln);
                                        }
                                        ui.add_space(4.0);
                                        rest = r;
                                        break;
                                    }
                                }
                            }
                        }
                        // inline: [task:ID], then styles and links
                        let mut s = rest;
                        while !s.is_empty() {
                            if let Some(i) = s.find("[task:") {
                                if let Some(j) = s[i..].find(']') {
                                    inline(ui, &s[..i], size, strong);
                                    let id: u64 = s[i + 6..i + j].parse().unwrap_or(0);
                                    let name = self
                                        .task(id)
                                        .map(|t| t.name.clone())
                                        .unwrap_or_else(|| tr("missing task").into());
                                    if ui
                                        .link(
                                            RichText::new(format!("\u{2611} {}", name)).size(size),
                                        )
                                        .clicked()
                                    {
                                        open_task = Some(id);
                                    }
                                    s = &s[i + j + 1..];
                                    continue;
                                }
                            }
                            inline(ui, s, size, strong);
                            break;
                        }
                    });
                }
            }
            if jump == Some(ln) {
                let r = Rect::from_min_max(
                    pos2(ui.min_rect().left(), top),
                    pos2(ui.min_rect().right(), ui.cursor().top().max(top + 20.0)),
                );
                ui.scroll_to_rect(r, Some(Align::TOP));
            }
        }
        if let Some(start) = toggle {
            let d = &mut self.data.docs[idx];
            toggle_check_at(&mut d.body, start);
            d.updated = now_ts();
            self.dirty = true;
        }
        if let Some(ln) = old_toggle {
            let d = &mut self.data.docs[idx];
            let lines: Vec<String> = d
                .body
                .split('\n')
                .enumerate()
                .map(|(i, l)| {
                    if i != ln {
                        l.to_string()
                    } else if l.starts_with("[ ] ") {
                        l.replacen("[ ] ", "[x] ", 1)
                    } else {
                        format!("[ ] {}", &l[4..])
                    }
                })
                .collect();
            d.body = lines.join("\n");
            d.updated = now_ts();
            self.dirty = true;
        }
        if let Some(id) = open_task {
            if self.task(id).is_some() {
                self.actions.push(Action::Open(id));
            }
        }
    }
}

/// Moves the text cursor of a text field and gives it focus.
fn set_cursor(ctx: &egui::Context, id: egui::Id, c: usize) {
    let mut st = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
        egui::text::CCursor::new(c),
    )));
    st.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Text with its inline styles and clickable links (read view).
fn inline(ui: &mut Ui, s: &str, size: f32, strong: bool) {
    let runs = inline_spans(s);
    for (i, (span, text)) in runs.iter().enumerate() {
        let mut r = RichText::new(*text).size(size);
        if strong {
            r = r.strong();
        }
        match span {
            Span::Marker => {}
            Span::Text => {
                ui.label(r);
            }
            Span::Bold => {
                ui.label(r.strong());
            }
            Span::Italic => {
                ui.label(r.italics());
            }
            Span::Strike => {
                ui.label(r.strikethrough());
            }
            Span::Code => {
                ui.label(r.code());
            }
            Span::Link => {
                // the url is in the marker run after the text: "](url)"
                let url = runs
                    .get(i + 1)
                    .map(|m| m.1)
                    .filter(|m| m.len() > 3)
                    .map(|m| &m[2..m.len() - 1])
                    .unwrap_or("");
                ui.hyperlink_to(r, url);
            }
            Span::Url => {
                ui.hyperlink_to(r, *text);
            }
        }
    }
}

/// How a run of a line's inline text is drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Span {
    Text,
    Bold,
    Italic,
    Strike,
    Code,
    /// the text of a [text](url) link; the marker run after it holds the url
    Link,
    /// a bare web address
    Url,
    /// Markdown punctuation: drawn faint while editing, hidden when reading
    Marker,
}

/// Splits a line into inline runs: **bold**, *italic*, ~~strike~~, `code`,
/// [text](url) links and bare http(s) addresses. The runs cover the line
/// exactly, so the edit view can lay out the stored text unchanged.
pub(super) fn inline_spans(s: &str) -> Vec<(Span, &str)> {
    let mut out: Vec<(Span, &str)> = vec![];
    let mut plain_from = 0;
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        let mut hit: Option<(usize, Vec<(Span, &str)>)> = None;
        for (mk, span) in [
            ("`", Span::Code),
            ("**", Span::Bold),
            ("~~", Span::Strike),
            ("*", Span::Italic),
        ] {
            if let Some(after) = rest.strip_prefix(mk) {
                if let Some(j) = after.find(mk) {
                    if j > 0 {
                        let n = mk.len();
                        hit = Some((
                            2 * n + j,
                            vec![
                                (Span::Marker, &rest[..n]),
                                (span, &rest[n..n + j]),
                                (Span::Marker, &rest[n + j..2 * n + j]),
                            ],
                        ));
                    }
                }
                break;
            }
        }
        if hit.is_none() && rest.starts_with('[') {
            if let Some(close) = rest.find("](") {
                if close > 1 && !rest[1..close].contains(['[', ']']) {
                    if let Some(end) = rest[close + 2..].find(')') {
                        let url = &rest[close + 2..close + 2 + end];
                        if !url.is_empty() && !url.contains(char::is_whitespace) {
                            hit = Some((
                                close + 3 + end,
                                vec![
                                    (Span::Marker, &rest[..1]),
                                    (Span::Link, &rest[1..close]),
                                    (Span::Marker, &rest[close..close + 3 + end]),
                                ],
                            ));
                        }
                    }
                }
            }
        }
        if hit.is_none()
            && (rest.starts_with("http://") || rest.starts_with("https://"))
            && (i == 0 || s[..i].ends_with(char::is_whitespace))
        {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let url = rest[..end].trim_end_matches(['.', ',', ')', ';', ':', '!', '?']);
            hit = Some((url.len(), vec![(Span::Url, url)]));
        }
        match hit {
            Some((n, runs)) => {
                if i > plain_from {
                    out.push((Span::Text, &s[plain_from..i]));
                }
                out.extend(runs);
                i += n;
                plain_from = i;
            }
            None => i += rest.chars().next().map_or(1, |c| c.len_utf8()),
        }
    }
    if s.len() > plain_from {
        out.push((Span::Text, &s[plain_from..]));
    }
    out
}

/// What a line of a Doc is, judged by its Markdown prefix.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum LineKind {
    Plain,
    Heading(u8),
    Bullet,
    Numbered,
    Check(bool),
    Quote,
    Code,
    Fence,
    Divider,
    Table,
}

/// Each line of `text` with its kind, its first character's index and the
/// length (in characters) of its Markdown prefix.
pub(super) fn line_kinds(text: &str) -> Vec<(LineKind, usize, usize)> {
    let mut out = vec![];
    let mut in_code = false;
    let mut start = 0usize;
    for line in text.split('\n') {
        let n = line.chars().count();
        let (kind, pre) = if line.trim_start().starts_with("```") {
            in_code = !in_code;
            (LineKind::Fence, n)
        } else if in_code {
            (LineKind::Code, 0)
        } else if line.trim() == "---" {
            (LineKind::Divider, n)
        } else if line.starts_with("### ") {
            (LineKind::Heading(3), 4)
        } else if line.starts_with("## ") {
            (LineKind::Heading(2), 3)
        } else if line.starts_with("# ") {
            (LineKind::Heading(1), 2)
        } else if line.starts_with("> ") {
            (LineKind::Quote, 2)
        } else if line.trim_start().starts_with('|') {
            (LineKind::Table, 0)
        } else {
            // list items may be indented (Tab) to nest them
            let ind = indent_of(line);
            let l = &line[ind..];
            let digits = l.chars().take_while(|c| c.is_ascii_digit()).count();
            if l.starts_with("- [ ] ") {
                (LineKind::Check(false), ind + 6)
            } else if l.starts_with("- [x] ") || l.starts_with("- [X] ") {
                (LineKind::Check(true), ind + 6)
            } else if l.starts_with("- ") || l.starts_with("* ") {
                (LineKind::Bullet, ind + 2)
            } else if digits > 0 && l[digits..].starts_with(". ") {
                (LineKind::Numbered, ind + digits + 2)
            } else {
                (LineKind::Plain, 0)
            }
        };
        out.push((kind, start, pre));
        start += n + 1;
    }
    out
}

/// Lays out Doc text with its formatting shown while editing: big headings,
/// grey quotes, struck-through done items, code in monospace, **bold** and
/// *italic*. Markdown markers stay in the text (the file format is
/// unchanged) but are drawn faint, or hidden where a painted mark replaces
/// them (bullets, checkboxes, quote bars).
pub(super) fn doc_layout_job(text: &str, t: &Theme, wrap: f32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let kinds = line_kinds(text);
    let lines: Vec<&str> = text.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        let (kind, _, pre) = kinds[i];
        let size = match kind {
            LineKind::Heading(1) => 28.0,
            LineKind::Heading(2) => 22.0,
            LineKind::Heading(3) => 18.0,
            _ => 16.0,
        };
        let base = TextFormat {
            font_id: FontId::proportional(size),
            color: match kind {
                LineKind::Quote => t.text2,
                LineKind::Check(true) => t.text3,
                LineKind::Heading(_) => t.text,
                _ => t.text,
            },
            italics: kind == LineKind::Quote,
            strikethrough: if kind == LineKind::Check(true) {
                Stroke::new(1.0, t.text3)
            } else {
                Stroke::NONE
            },
            ..Default::default()
        };
        let faint = TextFormat {
            font_id: FontId::proportional(size),
            color: t.text3.gamma_multiply(0.6),
            ..Default::default()
        };
        let hidden = TextFormat {
            font_id: FontId::proportional(size),
            color: Color32::TRANSPARENT,
            ..Default::default()
        };
        let mono = TextFormat {
            font_id: FontId::monospace(14.0),
            color: t.text,
            background: t.column_bg,
            ..Default::default()
        };
        let pb = byte_at(line, pre);
        match kind {
            LineKind::Code | LineKind::Table => job.append(line, 0.0, mono),
            LineKind::Fence => job.append(
                line,
                0.0,
                TextFormat {
                    font_id: FontId::monospace(12.0),
                    color: t.text3,
                    ..Default::default()
                },
            ),
            LineKind::Divider => job.append(line, 0.0, faint),
            _ => {
                let marker = match kind {
                    LineKind::Bullet | LineKind::Check(_) | LineKind::Quote => hidden,
                    LineKind::Numbered => base.clone(),
                    _ => faint.clone(),
                };
                job.append(&line[..pb], 0.0, marker);
                append_inline(&mut job, &line[pb..], &base, &faint, t);
            }
        }
        if i + 1 < lines.len() {
            job.append("\n", 0.0, base_plain(t));
        }
    }
    job
}

fn base_plain(t: &Theme) -> egui::text::TextFormat {
    egui::text::TextFormat {
        font_id: FontId::proportional(16.0),
        color: t.text,
        ..Default::default()
    }
}

/// Appends a line's text with its inline styles shown and the Markdown
/// punctuation drawn faint.
fn append_inline(
    job: &mut egui::text::LayoutJob,
    s: &str,
    base: &egui::text::TextFormat,
    faint: &egui::text::TextFormat,
    t: &Theme,
) {
    let link = Color32::from_rgb(16, 144, 224);
    for (span, text) in inline_spans(s) {
        let f = match span {
            Span::Text => base.clone(),
            Span::Marker => faint.clone(),
            Span::Bold => egui::text::TextFormat {
                color: if t.dark {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                },
                ..base.clone()
            },
            Span::Italic => egui::text::TextFormat {
                italics: true,
                ..base.clone()
            },
            Span::Strike => egui::text::TextFormat {
                strikethrough: Stroke::new(1.0_f32, base.color),
                ..base.clone()
            },
            Span::Code => egui::text::TextFormat {
                font_id: FontId::monospace(base.font_id.size * 0.88),
                background: t.column_bg,
                ..base.clone()
            },
            Span::Link | Span::Url => egui::text::TextFormat {
                color: link,
                underline: Stroke::new(1.0_f32, link),
                ..base.clone()
            },
        };
        job.append(text, 0.0, f);
    }
}

/// An empty three-column table, inserted by "Add new > Table".
pub(super) const TABLE_TEMPLATE: &str =
    "| Column 1 | Column 2 | Column 3 |\n| --- | --- | --- |\n|  |  |  |";

/// Ticks or unticks the checklist item on the line starting at character `start`.
pub(super) fn toggle_check_at(body: &mut String, start: usize) {
    let b = byte_at(body, start);
    let line_end = body[b..].find('\n').map(|i| b + i).unwrap_or(body.len());
    let line = &body[b..line_end];
    let ind = indent_of(line);
    let (pad, line) = line.split_at(ind);
    let new = if line.starts_with("- [ ] ") {
        format!("{}{}", pad, line.replacen("- [ ] ", "- [x] ", 1))
    } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
        format!("{}- [ ] {}", pad, &line[6..])
    } else {
        return;
    };
    body.replace_range(b..line_end, &new);
}

/// Leading spaces of a line (list nesting).
pub(super) fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// Indents (or outdents) the line holding character `cursor` by two
/// spaces; returns the new cursor.
pub(super) fn indent_line(body: &mut String, cursor: usize, indent: bool) -> usize {
    let b = byte_at(body, cursor);
    let ls = body[..b].rfind('\n').map(|i| i + 1).unwrap_or(0);
    if indent {
        body.insert_str(ls, "  ");
        return cursor + 2;
    }
    let n = indent_of(&body[ls..]).min(2);
    body.replace_range(ls..ls + n, "");
    cursor.saturating_sub(n).max(body[..ls].chars().count())
}

/// After Enter put a line break just before character `cursor`: a list
/// item's marker is carried onto the new line (numbers count up), and an
/// empty item ends the list instead. Returns the new cursor when it acted.
pub(super) fn continue_list(body: &mut String, cursor: usize) -> Option<usize> {
    let bc = byte_at(body, cursor);
    if bc == 0 || body.as_bytes()[bc - 1] != b'\n' {
        return None;
    }
    let nl = bc - 1;
    let ps = body[..nl].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let prev = &body[ps..nl];
    let (kind, _, pre) = line_kinds(prev).first().copied()?;
    let ind = indent_of(prev);
    let marker = match kind {
        LineKind::Check(_) => "- [ ] ".to_string(),
        LineKind::Bullet => prev[ind..ind + 2].to_string(),
        LineKind::Quote => "> ".to_string(),
        LineKind::Numbered => {
            let n: u64 = prev[ind..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .ok()?;
            format!("{}. ", n + 1)
        }
        _ => return None,
    };
    if prev[byte_at(prev, pre)..].trim().is_empty() {
        // Enter on an empty item: drop its marker and stay on that line
        body.replace_range(ps..bc, "");
        return Some(body[..ps].chars().count());
    }
    let ins = if kind == LineKind::Quote {
        marker
    } else {
        format!("{}{}", " ".repeat(ind), marker)
    };
    body.insert_str(bc, &ins);
    Some(cursor + ins.chars().count())
}

/// Wraps the selection as a Markdown link, "[text](https://)", with the
/// cursor where the address goes (or inside the brackets when nothing was
/// selected). Returns the new cursor.
pub(super) fn apply_link(body: &mut String, cursor: usize, sel_end: usize) -> usize {
    let (a, b) = (cursor.min(sel_end), cursor.max(sel_end));
    let (ba, bb) = (byte_at(body, a), byte_at(body, b));
    if a == b {
        body.insert_str(ba, "[](https://)");
        return a + 1;
    }
    body.insert_str(bb, "](https://)");
    body.insert(ba, '[');
    b + 1 + 2 + "https://".len()
}

/// The page's headings for "On this page": (level, text, line index).
pub(super) fn doc_headings(body: &str) -> Vec<(u8, String, usize)> {
    line_kinds(body)
        .into_iter()
        .zip(body.split('\n'))
        .enumerate()
        .filter_map(|(i, ((kind, _, pre), line))| match kind {
            LineKind::Heading(n) if !line[pre..].trim().is_empty() => {
                Some((n, line[pre..].trim().to_string(), i))
            }
            _ => None,
        })
        .collect()
}

// ------------------------------------------------------------ doc actions
// Shared by the Docs list, the doc header, the pages panel and the sidebar.

impl App {
    /// A page and every page under it, the page first.
    pub(super) fn doc_subtree(&self, id: u64) -> Vec<u64> {
        let mut out = vec![id];
        let mut i = 0;
        while i < out.len() {
            let cur = out[i];
            for d in self.data.docs.iter().filter(|d| d.parent == Some(cur)) {
                if !out.contains(&d.id) {
                    out.push(d.id);
                }
            }
            i += 1;
        }
        out
    }

    /// Copies a page with all its sub pages, right after the original;
    /// returns the copy's id.
    pub(super) fn duplicate_doc(&mut self, id: u64) -> Option<u64> {
        let ids = self.doc_subtree(id);
        let olds: Vec<Doc> = ids
            .iter()
            .filter_map(|i| self.data.docs.iter().find(|d| d.id == *i).cloned())
            .collect();
        if olds.is_empty() {
            return None;
        }
        let now = now_ts();
        let mut map: HashMap<u64, u64> = HashMap::new();
        for d in &olds {
            map.insert(d.id, self.data.new_id());
        }
        let copies: Vec<Doc> = olds
            .into_iter()
            .map(|mut d| {
                if d.id == id {
                    d.title = format!("{} (copy)", d.title);
                } else {
                    d.parent = d.parent.and_then(|p| map.get(&p).copied());
                }
                d.id = map[&d.id];
                d.owner = self.data.me;
                d.created = now;
                d.updated = now;
                // a copy starts unread (and the view entries' ids are the original's)
                d.views.clear();
                d
            })
            .collect();
        let at = self
            .data
            .docs
            .iter()
            .position(|d| d.id == id)
            .map_or(self.data.docs.len(), |p| p + 1);
        let new_id = map[&id];
        self.data.docs.splice(at..at, copies);
        self.dirty = true;
        Some(new_id)
    }

    /// Moves a page and its sub pages to Trash as one item, so restoring
    /// brings the whole tree back.
    pub(super) fn trash_doc(&mut self, id: u64) {
        let ids = self.doc_subtree(id);
        let pages: Vec<Doc> = ids
            .iter()
            .filter_map(|i| self.data.docs.iter().find(|d| d.id == *i).cloned())
            .collect();
        let Some(root) = pages.first().cloned() else {
            return;
        };
        let tid = self.data.new_id();
        self.data.extra.trash.push(TrashItem {
            id: tid,
            kind: "Doc".into(),
            title: root.title.clone(),
            deleted: now_ts(),
            payload: serde_json::to_string(&pages).unwrap_or_default(),
        });
        self.data.docs.retain(|d| !ids.contains(&d.id));
        if self.doc_open.is_some_and(|o| ids.contains(&o)) {
            self.doc_open = root.parent;
        }
        self.dirty = true;
    }

    /// Puts a whole doc (all its pages) in another Space, or the Workspace.
    pub(super) fn move_doc_to_space(&mut self, id: u64, space: Option<u64>) {
        let ids = self.doc_subtree(self.doc_root(id));
        for d in self.data.docs.iter_mut().filter(|d| ids.contains(&d.id)) {
            d.space = space;
            d.updated = now_ts();
        }
        self.dirty = true;
    }

    /// Hangs a page under another page of the same doc.
    pub(super) fn move_page(&mut self, id: u64, parent: u64) {
        if self.doc_subtree(id).contains(&parent) {
            return;
        }
        if let Some(d) = self.data.docs.iter_mut().find(|d| d.id == id) {
            d.parent = Some(parent);
            d.updated = now_ts();
            self.dirty = true;
        }
    }

    /// Swaps a page with the sibling above (`up`) or below it.
    pub(super) fn shift_page(&mut self, id: u64, up: bool) {
        let Some(parent) = self.data.docs.iter().find(|d| d.id == id).map(|d| d.parent) else {
            return;
        };
        let sibs: Vec<usize> = self
            .data
            .docs
            .iter()
            .enumerate()
            .filter(|(_, d)| d.parent == parent)
            .map(|(i, _)| i)
            .collect();
        let Some(k) = sibs.iter().position(|&i| self.data.docs[i].id == id) else {
            return;
        };
        let other = if up {
            k.checked_sub(1)
        } else {
            Some(k + 1).filter(|j| *j < sibs.len())
        };
        if let Some(j) = other {
            self.data.docs.swap(sibs[k], sibs[j]);
            self.dirty = true;
        }
    }

    /// Every tag used on any doc, sorted.
    pub(super) fn doc_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .data
            .docs
            .iter()
            .flat_map(|d| d.tags.iter().cloned())
            .collect();
        tags.sort_by_key(|g| g.to_lowercase());
        tags.dedup();
        tags
    }

    /// The doc's menu (right click in the Docs list, the sidebar and the
    /// pages panel; the header's "..." button). `rename` adds a title field.
    pub(super) fn doc_menu(&mut self, ui: &mut Ui, id: u64, rename: bool) {
        let Some(d) = self.data.docs.iter().find(|d| d.id == id).cloned() else {
            return;
        };
        ui.set_min_width(210.0);
        if rename {
            let mut title = d.title.clone();
            let r = ui.add(
                egui::TextEdit::singleline(&mut title)
                    .hint_text(tr("Doc name"))
                    .desired_width(190.0),
            );
            name_field(&r, "Rename doc");
            if r.changed() {
                if let Some(x) = self.data.docs.iter_mut().find(|x| x.id == id) {
                    x.title = title;
                    x.updated = now_ts();
                }
                self.dirty = true;
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                close_any(ui);
            }
            ui.separator();
        }
        if ui.button(tr("Open")).clicked() {
            self.doc_open = Some(id);
            self.section = Section::Docs;
            close_any(ui);
        }
        let key = format!("doc:{}", id);
        let starred = self.data.extra.starred.contains(&key);
        if ui
            .button(tr(if starred {
                "Remove from Favorites"
            } else {
                "Add to Favorites"
            }))
            .clicked()
        {
            if starred {
                self.data.extra.starred.retain(|s| s != &key);
            } else {
                self.data.extra.starred.push(key);
            }
            self.dirty = true;
            close_any(ui);
        }
        if ui.button(tr("Copy link")).clicked() {
            ui.ctx().copy_text(format!("workboard://doc/{}", id));
            self.toast = Some((tr("Link copied").into(), now_ts()));
            close_any(ui);
        }
        if ui.button(tr("Duplicate")).clicked() {
            if let Some(n) = self.duplicate_doc(id) {
                self.toast = Some((tr("Doc duplicated").into(), now_ts()));
                if self.doc_open.is_some() {
                    self.doc_open = Some(n);
                }
            }
            close_any(ui);
        }
        if ui.button(tr("Save as template")).clicked() {
            if self.save_doc_as_template(id).is_some() {
                self.toast = Some((tr("Saved as a template").into(), now_ts()));
            }
            close_any(ui);
        }
        ui.menu_button(tr("Move to Space"), |ui| {
            let mut spaces: Vec<(Option<u64>, String)> =
                vec![(None, tr("Workspace (no Space)").into())];
            spaces.extend(
                self.open_spaces()
                    .iter()
                    .map(|s| (Some(s.id), s.name.clone())),
            );
            for (sid, name) in spaces {
                if ui
                    .add(egui::Button::new(name).selected(d.space == sid))
                    .clicked()
                {
                    self.move_doc_to_space(id, sid);
                    close_any(ui);
                }
            }
        });
        if ui
            .button(tr(if d.meeting_note {
                "Unmark as Meeting Note"
            } else {
                "Mark as Meeting Note"
            }))
            .clicked()
        {
            if let Some(x) = self.data.docs.iter_mut().find(|x| x.id == id) {
                x.meeting_note = !x.meeting_note;
            }
            self.dirty = true;
            close_any(ui);
        }
        let wr = ui.button(tr(if d.is_wiki() {
            "Unmark as Wiki"
        } else {
            "Mark as Wiki"
        }));
        name_it(
            &wr,
            if d.is_wiki() {
                "Unmark as Wiki"
            } else {
                "Mark as Wiki"
            },
        );
        if wr.clicked() {
            if let Some(x) = self.data.docs.iter_mut().find(|x| x.id == id) {
                x.wiki = Some(!x.is_wiki());
            }
            self.dirty = true;
            close_any(ui);
        }
        ui.separator();
        if ui
            .button(tr(if d.archived { "Unarchive" } else { "Archive" }))
            .clicked()
        {
            self.toggle_archive(id);
            close_any(ui);
        }
        if ui
            .button(RichText::new(tr("Delete")).color(self.t().danger))
            .clicked()
        {
            self.trash_doc(id);
            close_any(ui);
        }
    }

    /// A sub page's menu in the pages panel.
    fn page_menu(&mut self, ui: &mut Ui, id: u64) {
        ui.set_min_width(200.0);
        if ui.button(tr("Open")).clicked() {
            self.doc_open = Some(id);
            close_any(ui);
        }
        if ui.button(tr("Add sub page")).clicked() {
            self.add_page(id);
            close_any(ui);
        }
        if ui.button(tr("Duplicate")).clicked() {
            if let Some(n) = self.duplicate_doc(id) {
                self.doc_open = Some(n);
            }
            close_any(ui);
        }
        ui.separator();
        if ui.button(tr("Move up")).clicked() {
            self.shift_page(id, true);
            close_any(ui);
        }
        if ui.button(tr("Move down")).clicked() {
            self.shift_page(id, false);
            close_any(ui);
        }
        ui.menu_button(tr("Move to"), |ui| {
            // any page of this doc that is not this page or under it
            let root = self.doc_root(id);
            let skip = self.doc_subtree(id);
            let cur = self
                .data
                .docs
                .iter()
                .find(|d| d.id == id)
                .and_then(|d| d.parent);
            let targets: Vec<(u64, String)> = self
                .doc_subtree(root)
                .into_iter()
                .filter(|p| !skip.contains(p))
                .filter_map(|p| {
                    self.data.docs.iter().find(|d| d.id == p).map(|d| {
                        (
                            p,
                            if d.title.is_empty() {
                                tr("Untitled").to_string()
                            } else {
                                d.title.clone()
                            },
                        )
                    })
                })
                .collect();
            for (p, title) in targets {
                if ui
                    .add(egui::Button::new(title).selected(cur == Some(p)))
                    .clicked()
                {
                    self.move_page(id, p);
                    close_any(ui);
                }
            }
        });
        ui.separator();
        if ui
            .button(RichText::new(tr("Delete")).color(self.t().danger))
            .clicked()
        {
            self.trash_doc(id);
            close_any(ui);
        }
    }

    /// The doc's tags under its title: chips (click to remove) and "+ Tag".
    fn doc_tags_row(&mut self, ui: &mut Ui, idx: usize) {
        let t = self.t();
        let id = self.data.docs[idx].id;
        let tags = self.data.docs[idx].tags.clone();
        let mut remove: Option<String> = None;
        let mut add: Option<String> = None;
        ui.horizontal_wrapped(|ui| {
            for g in &tags {
                let r = chip(ui, &format!("{}  \u{00D7}", g), tag_color(g))
                    .on_hover_text(tr("Remove tag"));
                name_it(&r, &format!("Remove tag {}", g));
                if r.clicked() {
                    remove = Some(g.clone());
                }
            }
            let r = ui.add(ghost(egui::Button::new(
                RichText::new(tr("+ Tag")).small().color(t.text3),
            )));
            name_it(&r, "Add tag");
            let known: Vec<String> = self
                .doc_tags()
                .into_iter()
                .filter(|g| !tags.contains(g))
                .collect();
            popup(ui, &r, ui.id().with(("doc_tag_add", id)), 200.0, |ui| {
                let nid = egui::Id::new(("doc_tag_new", id));
                let mut s: String = ui.data(|m| m.get_temp(nid)).unwrap_or_default();
                let te = ui.add(
                    egui::TextEdit::singleline(&mut s)
                        .hint_text(tr("New tag"))
                        .desired_width(180.0),
                );
                name_field(&te, "New tag");
                focus_once(ui, &te);
                if te.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && !s.trim().is_empty()
                {
                    add = Some(s.trim().to_string());
                    s.clear();
                    close_popup(ui);
                }
                ui.data_mut(|m| m.insert_temp(nid, s));
                for g in known {
                    if ui
                        .add(ghost(egui::Button::new(
                            RichText::new(&g).color(tag_color(&g)),
                        )))
                        .clicked()
                    {
                        add = Some(g);
                        close_popup(ui);
                    }
                }
            });
        });
        if let Some(g) = remove {
            self.data.docs[idx].tags.retain(|x| x != &g);
            self.dirty = true;
        }
        if let Some(g) = add {
            let d = &mut self.data.docs[idx];
            if !d.tags.contains(&g) {
                d.tags.push(g);
                d.updated = now_ts();
                self.dirty = true;
            }
        }
    }
}

impl App {
    /// Puts a doc's pages back from Trash. A page whose parent is gone now
    /// becomes a doc of its own, so it still shows in the Docs list.
    pub(super) fn restore_doc_pages(&mut self, pages: Vec<Doc>) {
        let ids: Vec<u64> = pages.iter().map(|d| d.id).collect();
        for mut d in pages {
            if d.parent
                .is_some_and(|p| !ids.contains(&p) && !self.data.docs.iter().any(|x| x.id == p))
            {
                d.parent = None;
            }
            if !self.data.docs.iter().any(|x| x.id == d.id) {
                self.data.docs.push(d);
            }
        }
        self.dirty = true;
    }
}
