// The Doc editor body: a pages panel (pages and sub pages), a formatting
// bar, "/" commands and a rendered view. Doc text is kept as Markdown so
// Export and the shared-folder copy stay plain text.

use super::*;

/// "/" commands and formatting choices: (label, line prefix, wrap).
const BLOCKS: [(&str, &str, &str); 11] = [
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
];

/// Byte offset of the `ci`-th character.
fn byte_at(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

/// Applies a block or wrap at the cursor (character index); returns the new cursor.
pub(super) fn apply_block(body: &mut String, cursor: usize, sel_end: usize, label: &str) -> usize {
    let Some((_, prefix, wrap)) = BLOCKS.iter().find(|b| b.0 == label) else { return cursor };
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
            match self.data.docs.iter().find(|d| d.id == cur).and_then(|d| d.parent) {
                Some(p) if self.data.docs.iter().any(|d| d.id == p) => cur = p,
                _ => break,
            }
        }
        cur
    }

    pub(super) fn add_page(&mut self, parent: u64) -> u64 {
        let id = self.data.new_id();
        let now = now_ts();
        let (space, private) = self.data.docs.iter().find(|d| d.id == parent).map(|d| (d.space, d.private)).unwrap_or((None, false));
        let n = self.data.docs.iter().filter(|d| d.parent == Some(parent)).count() + 1;
        self.data.docs.push(Doc { id, space, owner: self.data.me, private, archived: false, meeting_note: false, parent: Some(parent), title: format!("Page {}", n), body: String::new(), created: now, updated: now });
        self.doc_open = Some(id);
        // a new page is for writing: open it in Edit, not the last page's View
        self.doc_preview = false;
        self.dirty = true;
        id
    }

    fn page_rows(&mut self, ui: &mut Ui, parent: u64, depth: usize) {
        let t = self.t();
        let kids: Vec<(u64, String)> = self.data.docs.iter().filter(|d| d.parent == Some(parent)).map(|d| (d.id, d.title.clone())).collect();
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
            ui.painter().with_clip_rect(r.shrink(2.0)).text(pos2(x, r.center().y), Align2::LEFT_CENTER, if title.is_empty() { tr("Untitled") } else { &title }, FontId::proportional(13.0), if sel { t.selected_text } else { t.text });
            let plus = Rect::from_center_size(r.right_center() - vec2(12.0, 0.0), vec2(18.0, 18.0));
            let pr = ui.interact(plus, ui.id().with(("subpage", id)), Sense::click());
            if resp.hovered() || pr.hovered() {
                ui.painter().text(plus.center(), Align2::CENTER_CENTER, "+", FontId::proportional(14.0), t.text2);
            }
            if pr.on_hover_text(tr("Add sub page")).clicked() {
                self.add_page(id);
            } else if resp.clicked() {
                self.doc_open = Some(id);
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
        ui.painter().vline(pages.right(), pages.y_range(), Stroke::new(1.0, t.border));
        let mut pui = ui.new_child(egui::UiBuilder::new().max_rect(pages.shrink(8.0)).layout(Layout::top_down(Align::Min)));
        pui.horizontal(|ui| {
            ui.label(RichText::new(tr("Pages")).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("+").on_hover_text(tr("Add page")).clicked() {
                    self.add_page(root);
                }
            });
        });
        pui.add_space(4.0);
        egui::ScrollArea::vertical().id_salt("doc_pages").auto_shrink([false, false]).show(&mut pui, |ui| {
            let title = self.data.docs.iter().find(|d| d.id == root).map(|d| d.title.clone()).unwrap_or_default();
            let sel = self.doc_open == Some(root);
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
            if sel {
                ui.painter().rect_filled(r, 6.0, t.selected_bg);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, 6.0, t.hover);
            }
            ui.painter().with_clip_rect(r.shrink(2.0)).text(r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, format!("\u{1F4C4} {}", if title.is_empty() { tr("Untitled") } else { &title }), FontId::proportional(13.0), if sel { t.selected_text } else { t.text });
            if resp.clicked() {
                self.doc_open = Some(root);
            }
            self.page_rows(ui, root, 1);
            ui.add_space(6.0);
            if ui.add(egui::Button::new(RichText::new(tr("+  Add page")).color(t.text2)).frame(false)).clicked() {
                self.add_page(root);
            }
        });
        // the page may have changed above
        let Some(idx) = self.doc_open.and_then(|d| self.data.docs.iter().position(|x| x.id == d)).or(Some(idx)) else { return };
        let mut mui = ui.new_child(egui::UiBuilder::new().max_rect(main).layout(Layout::top_down(Align::Min)));
        let ui = &mut mui;
        // formatting bar
        let body_id = egui::Id::new(("doc_body", self.data.docs[idx].id));
        let mut pick: Option<&'static str> = None;
        egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(!self.doc_preview, tr("Edit")).clicked() {
                    self.doc_preview = false;
                }
                if ui.selectable_label(self.doc_preview, tr("View")).clicked() {
                    self.doc_preview = true;
                }
                ui.separator();
                if !self.doc_preview {
                    for (short, label) in [("H1", "Heading 1"), ("H2", "Heading 2"), ("H3", "Heading 3"), ("B", "Bold"), ("I", "Italic"), ("\u{2022}", "Bulleted list"), ("1.", "Numbered list"), ("\u{2611}", "Checklist"), ("\u{201C}", "Quote"), ("</>", "Code block"), ("\u{2014}", "Divider")] {
                        let txt = match short {
                            "B" => RichText::new(short).strong(),
                            "I" => RichText::new(short).italics(),
                            _ => RichText::new(short),
                        };
                        if ui.add(egui::Button::new(txt).frame(false).min_size(vec2(26.0, 24.0))).on_hover_text(tr(label)).clicked() {
                            pick = Some(label);
                        }
                    }
                    ui.separator();
                    let lr = ui.add(egui::Button::new(tr("Link task")).frame(false));
                    let tasks: Vec<(u64, String)> = self.data.tasks.iter().filter(|t| t.parent.is_none() && !t.archived).map(|t| (t.id, t.name.clone())).collect();
                    popup(ui, &lr, ui.id().with("link_task"), 260.0, |ui| {
                        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for (tid, name) in tasks {
                                if ui.button(&name).clicked() {
                                    let d = &mut self.data.docs[idx];
                                    let cur: usize = ui.data(|m| m.get_temp(body_id.with("cursor"))).unwrap_or(d.body.chars().count());
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
                    let author = self.data.member(d.owner).map(|m| m.name.clone()).unwrap_or_default();
                    ui.label(RichText::new(trf!("{} \u{00B7} {} words", author, words)).small().color(t.text3));
                });
            });
        });
        let r = ui.available_rect_before_wrap();
        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, t.border));
        if let Some(label) = pick {
            let d = &mut self.data.docs[idx];
            let n = d.body.chars().count();
            let cur: usize = ui.data(|m| m.get_temp(body_id.with("cursor"))).unwrap_or(n).min(n);
            let sel_end: usize = ui.data(|m| m.get_temp(body_id.with("sel"))).unwrap_or(cur).min(n);
            let c = apply_block(&mut d.body, cur, sel_end, label);
            d.updated = now_ts();
            self.dirty = true;
            set_cursor(ui.ctx(), body_id, c);
        }
        egui::ScrollArea::vertical().id_salt(("doc_scroll", self.data.docs[idx].id)).auto_shrink([false, false]).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(60, 24)).show(ui, |ui| {
                let d = &mut self.data.docs[idx];
                let r1 = ui.add(egui::TextEdit::singleline(&mut d.title).font(FontId::proportional(32.0)).frame(false).hint_text(tr("Untitled")).desired_width(f32::INFINITY));
                if r1.changed() {
                    d.updated = now_ts();
                    self.dirty = true;
                }
                ui.add_space(10.0);
                if self.doc_preview {
                    self.render_markdown(ui, idx);
                    return;
                }
                let d = &mut self.data.docs[idx];
                let empty = d.body.is_empty();
                let mut layouter = |ui: &Ui, s: &str, wrap: f32| ui.fonts(|f| f.layout_job(doc_layout_job(s, &t, wrap)));
                let out = egui::TextEdit::multiline(&mut d.body).id(body_id).frame(false).hint_text(tr("Start writing, or type '/' for commands")).font(FontId::proportional(16.0)).desired_width(f32::INFINITY).desired_rows(if empty { 3 } else { 24 }).layouter(&mut layouter).show(ui);
                name_field(&out.response, "Doc body");
                if out.response.changed() {
                    d.updated = now_ts();
                    self.dirty = true;
                }
                // painted marks over the hidden Markdown markers: bullets,
                // checkboxes (click to tick), quote bars and dividers
                let click = if out.response.clicked() { ui.input(|i| i.pointer.interact_pos()) } else { None };
                let mut tick: Option<usize> = None;
                for (kind, start, _) in line_kinds(&d.body) {
                    let at = out.galley_pos + out.galley.pos_from_cursor(&out.galley.from_ccursor(egui::text::CCursor::new(start))).min.to_vec2();
                    let row = out.galley.pos_from_cursor(&out.galley.from_ccursor(egui::text::CCursor::new(start)));
                    let h = row.height();
                    let mid = at.y + h / 2.0;
                    let p = ui.painter();
                    match kind {
                        LineKind::Bullet => {
                            p.circle_filled(pos2(at.x + 5.0, mid), 2.6, t.text);
                        }
                        LineKind::Check(done) => {
                            let bx = Rect::from_min_size(pos2(at.x + 2.0, mid - 7.5), vec2(15.0, 15.0));
                            if done {
                                p.rect_filled(bx, 3.0, t.primary);
                                p.line_segment([bx.left_center() + vec2(3.5, 0.0), bx.center_bottom() + vec2(-1.0, -4.0)], Stroke::new(1.8, Color32::WHITE));
                                p.line_segment([bx.center_bottom() + vec2(-1.0, -4.0), bx.right_top() + vec2(-3.5, 4.0)], Stroke::new(1.8, Color32::WHITE));
                            } else {
                                p.rect_stroke(bx, 3.0, Stroke::new(1.3, t.text3), egui::StrokeKind::Inside);
                            }
                            if click.is_some_and(|c| bx.expand(2.0).contains(c)) {
                                tick = Some(start);
                            }
                        }
                        LineKind::Quote => {
                            p.rect_filled(Rect::from_min_size(pos2(at.x + 1.0, at.y + 1.0), vec2(3.0, h - 2.0)), 1.0, t.border);
                        }
                        LineKind::Divider => {
                            p.hline(at.x..=out.response.rect.right(), mid, Stroke::new(1.0, t.border));
                        }
                        _ => {}
                    }
                }
                if let Some(start) = tick {
                    toggle_check_at(&mut d.body, start);
                    d.updated = now_ts();
                    self.dirty = true;
                }
                // a new, empty page offers how to start, as in the original
                if empty {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new(tr("Start writing")).color(t.text2)).frame(false)).clicked() {
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
                if let Some(cr) = out.cursor_range {
                    let (a, b) = (cr.primary.ccursor.index, cr.secondary.ccursor.index);
                    ui.data_mut(|m| {
                        m.insert_temp(body_id.with("cursor"), a);
                        m.insert_temp(body_id.with("sel"), b);
                    });
                    // "/" typed at the start of a line opens the command menu; it stays
                    // armed while the menu is clicked (the click takes focus away)
                    let chars: Vec<char> = d.body.chars().collect();
                    let slash_here = a > 0 && a == b && chars.get(a - 1) == Some(&'/') && (a == 1 || chars.get(a - 2) == Some(&'\n'));
                    let arm_id = body_id.with("slash_at");
                    if out.response.has_focus() {
                        ui.data_mut(|m| if slash_here { m.insert_temp(arm_id, a) } else { m.remove::<usize>(arm_id) });
                    }
                }
                let arm_id = body_id.with("slash_at");
                let d = &mut self.data.docs[idx];
                let chars: Vec<char> = d.body.chars().collect();
                let armed: Option<usize> = ui.data(|m| m.get_temp(arm_id)).filter(|a: &usize| *a > 0 && chars.get(*a - 1) == Some(&'/'));
                if let Some(a) = armed {
                    let ccur = egui::text::CCursor::new(a);
                    let pos = out.galley_pos + out.galley.pos_from_cursor(&out.galley.from_ccursor(ccur)).left_bottom().to_vec2() + vec2(0.0, 4.0);
                    let mut chosen: Option<&'static str> = None;
                    let area = egui::Area::new(body_id.with("slash")).order(egui::Order::Foreground).fixed_pos(pos).show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_width(200.0);
                            ui.label(RichText::new(tr("Basic blocks")).small().color(t.text3));
                            for (label, _, _) in BLOCKS.iter().take(9) {
                                if ui.add(egui::Button::new(tr(label)).frame(false).min_size(vec2(190.0, 22.0))).clicked() {
                                    chosen = Some(label);
                                }
                            }
                        });
                    });
                    let pressed_elsewhere = ui.input(|i| i.pointer.any_pressed()) && !ui.ctx().pointer_hover_pos().map_or(false, |p| area.response.rect.contains(p) || out.response.rect.contains(p));
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) || pressed_elsewhere {
                        ui.data_mut(|m| m.remove::<usize>(arm_id));
                    }
                    if let Some(label) = chosen {
                        // drop the "/" then apply the block, and put the cursor after it
                        let at = byte_at(&d.body, a - 1);
                        d.body.replace_range(at..at + 1, "");
                        let c = apply_block(&mut d.body, a - 1, a - 1, label);
                        d.updated = now_ts();
                        self.dirty = true;
                        ui.data_mut(|m| m.remove::<usize>(arm_id));
                        set_cursor(ui.ctx(), body_id, c);
                    }
                }
                // sub pages listed under the text, as in the original
                let kids: Vec<(u64, String)> = self.data.docs.iter().filter(|x| x.parent == Some(self.data.docs[idx].id)).map(|x| (x.id, x.title.clone())).collect();
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

    /// Draws the Doc's Markdown: headings, lists, checklists (clickable),
    /// quotes, code, dividers, **bold**, *italic* and task links.
    fn render_markdown(&mut self, ui: &mut Ui, idx: usize) {
        let t = self.t();
        let body = self.data.docs[idx].body.clone();
        let mut toggle: Option<usize> = None;
        let mut in_code = false;
        let mut open_task: Option<u64> = None;
        for (ln, line) in body.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                in_code = !in_code;
                continue;
            }
            if in_code {
                egui::Frame::new().fill(t.column_bg).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(line).monospace());
                });
                continue;
            }
            let (size, text, strong) = if let Some(r) = line.strip_prefix("### ") {
                (18.0, r, true)
            } else if let Some(r) = line.strip_prefix("## ") {
                (22.0, r, true)
            } else if let Some(r) = line.strip_prefix("# ") {
                (28.0, r, true)
            } else {
                (16.0, line, false)
            };
            if line.trim() == "---" {
                ui.separator();
                continue;
            }
            if line.trim_start().starts_with('|') {
                let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(|c| c.trim()).collect();
                if cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')) {
                    continue; // the | --- | row under the header
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for c in cells {
                        let (r, _) = ui.allocate_exact_size(vec2(160.0, 30.0), Sense::hover());
                        ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
                        ui.painter().with_clip_rect(r.shrink(2.0)).text(r.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER, c, FontId::proportional(14.0), t.text);
                    }
                });
                continue;
            }
            if line.trim().is_empty() {
                ui.add_space(8.0);
                continue;
            }
            ui.horizontal_wrapped(|ui| {
                // runs of text sit right next to each other
                ui.spacing_mut().item_spacing.x = 0.0;
                let mut rest = text;
                if let Some(r) = rest.strip_prefix("- [ ] ").or_else(|| rest.strip_prefix("[ ] ")) {
                    let mut on = false;
                    if ui.checkbox(&mut on, "").clicked() {
                        toggle = Some(ln);
                    }
                    ui.add_space(4.0);
                    rest = r;
                } else if let Some(r) = rest.strip_prefix("- [x] ").or_else(|| rest.strip_prefix("[x] ")) {
                    let mut on = true;
                    if ui.checkbox(&mut on, "").clicked() {
                        toggle = Some(ln);
                    }
                    ui.add_space(4.0);
                    rest = r;
                } else if let Some(r) = rest.strip_prefix("- ") {
                    ui.label(RichText::new("  \u{2022}  ").size(size));
                    rest = r;
                } else if let Some(r) = rest.strip_prefix("> ") {
                    let (bar, _) = ui.allocate_exact_size(vec2(3.0, size + 4.0), Sense::hover());
                    ui.painter().rect_filled(bar, 1.0, t.border);
                    rest = r;
                }
                // inline: [task:ID], **bold**, *italic*
                let mut s = rest;
                while !s.is_empty() {
                    if let Some(i) = s.find("[task:") {
                        if let Some(j) = s[i..].find(']') {
                            inline(ui, &s[..i], size, strong);
                            let id: u64 = s[i + 6..i + j].parse().unwrap_or(0);
                            let name = self.task(id).map(|t| t.name.clone()).unwrap_or_else(|| tr("missing task").into());

                            if ui.link(RichText::new(format!("\u{2611} {}", name)).size(size)).clicked() {
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
        if let Some(ln) = toggle {
            let d = &mut self.data.docs[idx];
            let lines: Vec<String> = d
                .body
                .lines()
                .enumerate()
                .map(|(i, l)| {
                    if i != ln {
                        l.to_string()
                    } else if l.contains("[ ] ") {
                        l.replacen("[ ] ", "[x] ", 1)
                    } else {
                        l.replacen("[x] ", "[ ] ", 1)
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
    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(c))));
    st.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

/// Text with **bold** and *italic* runs.
fn inline(ui: &mut Ui, s: &str, size: f32, strong: bool) {
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(i) = rest.find("**") {
            if let Some(j) = rest[i + 2..].find("**") {
                plain(ui, &rest[..i], size, strong, false);
                ui.label(RichText::new(&rest[i + 2..i + 2 + j]).size(size).strong());
                rest = &rest[i + 4 + j..];
                continue;
            }
        }
        if let Some(i) = rest.find('*') {
            if let Some(j) = rest[i + 1..].find('*') {
                if j > 0 {
                    plain(ui, &rest[..i], size, strong, false);
                    plain(ui, &rest[i + 1..i + 1 + j], size, strong, true);
                    rest = &rest[i + 2 + j..];
                    continue;
                }
            }
        }
        plain(ui, rest, size, strong, false);
        break;
    }
}

fn plain(ui: &mut Ui, s: &str, size: f32, strong: bool, italic: bool) {
    if s.is_empty() {
        return;
    }
    let mut r = RichText::new(s).size(size);
    if strong {
        r = r.strong();
    }
    if italic {
        r = r.italics();
    }
    ui.label(r);
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
        } else if line.starts_with("- [ ] ") {
            (LineKind::Check(false), 6)
        } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
            (LineKind::Check(true), 6)
        } else if line.starts_with("- ") || line.starts_with("* ") {
            (LineKind::Bullet, 2)
        } else if line.starts_with("> ") {
            (LineKind::Quote, 2)
        } else if line.trim_start().starts_with('|') {
            (LineKind::Table, 0)
        } else {
            let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 && line[digits..].starts_with(". ") {
                (LineKind::Numbered, digits + 2)
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
            strikethrough: if kind == LineKind::Check(true) { Stroke::new(1.0, t.text3) } else { Stroke::NONE },
            ..Default::default()
        };
        let faint = TextFormat { font_id: FontId::proportional(size), color: t.text3.gamma_multiply(0.6), ..Default::default() };
        let hidden = TextFormat { font_id: FontId::proportional(size), color: Color32::TRANSPARENT, ..Default::default() };
        let mono = TextFormat { font_id: FontId::monospace(14.0), color: t.text, background: t.column_bg, ..Default::default() };
        let pb = byte_at(line, pre);
        match kind {
            LineKind::Code | LineKind::Table => job.append(line, 0.0, mono),
            LineKind::Fence => job.append(line, 0.0, TextFormat { font_id: FontId::monospace(12.0), color: t.text3, ..Default::default() }),
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
    egui::text::TextFormat { font_id: FontId::proportional(16.0), color: t.text, ..Default::default() }
}

/// Appends a line's text with **bold** and *italic* runs formatted and the
/// asterisks drawn faint.
fn append_inline(job: &mut egui::text::LayoutJob, s: &str, base: &egui::text::TextFormat, faint: &egui::text::TextFormat, t: &Theme) {
    let strong = egui::text::TextFormat { color: if t.dark { Color32::WHITE } else { Color32::BLACK }, ..base.clone() };
    let italic = egui::text::TextFormat { italics: true, ..base.clone() };
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(i) = rest.find("**") {
            if let Some(j) = rest[i + 2..].find("**") {
                if j > 0 {
                    job.append(&rest[..i], 0.0, base.clone());
                    job.append("**", 0.0, faint.clone());
                    job.append(&rest[i + 2..i + 2 + j], 0.0, strong.clone());
                    job.append("**", 0.0, faint.clone());
                    rest = &rest[i + 4 + j..];
                    continue;
                }
            }
        }
        if let Some(i) = rest.find('*') {
            if let Some(j) = rest[i + 1..].find('*') {
                if j > 0 {
                    job.append(&rest[..i], 0.0, base.clone());
                    job.append("*", 0.0, faint.clone());
                    job.append(&rest[i + 1..i + 1 + j], 0.0, italic.clone());
                    job.append("*", 0.0, faint.clone());
                    rest = &rest[i + 2 + j..];
                    continue;
                }
            }
        }
        job.append(rest, 0.0, base.clone());
        break;
    }
}

/// An empty three-column table, inserted by "Add new > Table".
pub(super) const TABLE_TEMPLATE: &str = "| Column 1 | Column 2 | Column 3 |\n| --- | --- | --- |\n|  |  |  |";

/// Ticks or unticks the checklist item on the line starting at character `start`.
pub(super) fn toggle_check_at(body: &mut String, start: usize) {
    let b = byte_at(body, start);
    let line_end = body[b..].find('\n').map(|i| b + i).unwrap_or(body.len());
    let line = &body[b..line_end];
    let new = if line.starts_with("- [ ] ") { line.replacen("- [ ] ", "- [x] ", 1) } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") { format!("- [ ] {}", &line[6..]) } else { return };
    body.replace_range(b..line_end, &new);
}
