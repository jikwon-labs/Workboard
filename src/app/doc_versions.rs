// Version history on the Mac: noticing when the open doc is being edited
// here, so a version is kept when the editing stops (`doc_versions_tick`)
// or a teammate's change replaces it (`doc_versions_after_merge`), and the
// Version history dialog that lists the versions, shows one read-only and
// restores it. The rules for what is kept are in model/doc_versions.rs.

use super::*;

/// The open doc, as this app last saw it (`App::doc_session`).
pub(super) struct DocSession {
    /// The workspace file it belongs to: opening another workspace drops it.
    ws: String,
    doc: u64,
    /// Its text when last looked at: before editing, what an edit started
    /// from; while editing, the latest text.
    title: String,
    body: String,
    /// While I'm editing it: when I last changed it.
    last: Option<i64>,
}

/// The Version history dialog's state (`Modal::DocVersions`).
pub(super) struct DocVersionsUi {
    pub doc: u64,
    /// The version picked in the list (the newest at first).
    pub pick: Option<u64>,
}

impl App {
    /// Once a frame, after the screen is drawn: notice the open doc being
    /// changed here (it no longer reads what it did) and keep the text it
    /// started from; keep the new text as a version once nothing changed
    /// for `DOC_VERSION_GAP`, or another doc is opened.
    pub(super) fn doc_versions_tick(&mut self, ctx: Option<&egui::Context>) {
        let now = now_ts();
        let open = self
            .doc_being_edited()
            .filter(|d| self.data.docs.iter().any(|x| x.id == *d))
            .filter(|d| self.doc_level(*d) >= crate::sync::Level::Edit);
        if let Some(s) = self.doc_session.take() {
            if s.ws != self.ws_file {
                // another workspace: its doc isn't this one's
            } else if Some(s.doc) != open {
                self.finish_doc_session(&s);
            } else {
                self.doc_session = Some(s);
            }
        }
        let Some(doc) = open else {
            return;
        };
        let Some(d) = self.data.docs.iter().find(|d| d.id == doc) else {
            return;
        };
        let Some(s) = self.doc_session.as_mut() else {
            self.doc_session = Some(DocSession {
                ws: self.ws_file.clone(),
                doc,
                title: d.title.clone(),
                body: d.body.clone(),
                last: None,
            });
            return;
        };
        if !d.reads(&s.title, &s.body) {
            let started = s.last.is_none();
            let (title, body) = (
                std::mem::replace(&mut s.title, d.title.clone()),
                std::mem::replace(&mut s.body, d.body.clone()),
            );
            s.last = Some(now);
            if started && self.data.keep_doc_start(doc, &title, &body, now) {
                self.doc_dirty = true;
            }
        } else if let Some(last) = s.last {
            if now - last >= DOC_VERSION_GAP {
                s.last = None;
                if self.data.keep_doc_version(doc, self.data.me, last) {
                    self.doc_dirty = true;
                }
            } else if let Some(ctx) = ctx {
                // nothing else may wake the app when the editing stops
                let wait = (last + DOC_VERSION_GAP - now).max(1) as u64;
                ctx.request_repaint_after(std::time::Duration::from_secs(wait));
            }
        }
    }

    /// The editing of a doc is over (another doc opened, the app quitting):
    /// what was written is kept as a version.
    fn finish_doc_session(&mut self, s: &DocSession) {
        if let Some(last) = s.last {
            if self.data.keep_doc_version(s.doc, self.data.me, last) {
                self.doc_dirty = true;
            }
        }
    }

    /// Before quitting: the doc being edited keeps its version now.
    pub(super) fn finish_doc_editing(&mut self) {
        if let Some(s) = self.doc_session.take() {
            if s.ws == self.ws_file {
                self.finish_doc_session(&s);
            }
        }
    }

    /// The text of the doc being edited here, before a merge.
    pub(super) fn doc_versions_before_merge(&self) -> Option<(u64, String, String)> {
        let s = self.doc_session.as_ref().filter(|s| s.ws == self.ws_file)?;
        s.last?;
        let d = self.data.docs.iter().find(|d| d.id == s.doc)?;
        Some((d.id, d.title.clone(), d.body.clone()))
    }

    /// After a merge: when teammates' changes replaced the text of the doc
    /// being edited here (`before`), what was written is kept as a version
    /// first, so it can be brought back; the doc then counts as not being
    /// edited, and what came in is where the next edit starts from.
    pub(super) fn doc_versions_after_merge(&mut self, before: Option<(u64, String, String)>) {
        let ws = &self.ws_file;
        let Some(s) = self.doc_session.as_mut().filter(|s| &s.ws == ws) else {
            return;
        };
        let Some(d) = self.data.docs.iter().find(|d| d.id == s.doc) else {
            return;
        };
        if d.reads(&s.title, &s.body) {
            return;
        }
        let editing = s.last.take();
        s.title = d.title.clone();
        s.body = d.body.clone();
        let (Some(last), Some((doc, title, body))) = (editing, before) else {
            return;
        };
        if doc == s.doc
            && self
                .data
                .keep_doc_text(doc, self.data.me, last, &title, &body)
        {
            self.sync_mark_change();
            self.save_soon();
        }
    }

    pub(super) fn open_doc_versions(&mut self, doc: u64) {
        // what is being written now shows in the list too
        self.finish_doc_editing();
        if self.doc_dirty {
            self.doc_dirty = false;
            self.dirty = true;
        }
        let pick = self
            .data
            .docs
            .iter()
            .find(|d| d.id == doc)
            .and_then(|d| d.latest_version())
            .map(|v| v.id);
        self.modal = Some(Modal::DocVersions(DocVersionsUi { doc, pick }));
    }

    /// Restore this version, as `restore_doc_version`; the doc being edited
    /// keeps what was written first.
    pub(super) fn restore_version(&mut self, doc: u64, version: u64) -> bool {
        self.finish_doc_editing();
        if !self
            .data
            .restore_doc_version(doc, version, self.data.me, now_ts())
        {
            return false;
        }
        self.dirty = true;
        true
    }

    /// The Version history dialog. Returns true when it should close.
    pub(super) fn doc_versions_body(
        &mut self,
        ui: &mut Ui,
        t: &Theme,
        st: &mut DocVersionsUi,
    ) -> bool {
        let Some(d) = self.data.docs.iter().find(|d| d.id == st.doc) else {
            return true;
        };
        let versions: Vec<DocVersion> = d.versions_newest().into_iter().cloned().collect();
        let (cur_title, cur_body) = (d.title.clone(), d.body.clone());
        let may_edit = self.doc_level(st.doc) >= crate::sync::Level::Edit;
        ui.label(
            RichText::new(tr(
                "Earlier texts of this doc, kept when someone stops editing it.",
            ))
            .color(t.text2),
        );
        ui.add_space(8.0);
        if versions.is_empty() {
            ui.label(
                RichText::new(tr(
                    "No versions yet. One is kept a few minutes after the doc is changed.",
                ))
                .color(t.text3),
            );
            return false;
        }
        if !versions.iter().any(|v| Some(v.id) == st.pick) {
            st.pick = versions.first().map(|v| v.id);
        }
        let h = (ui.ctx().screen_rect().height() - 260.0).clamp(220.0, 560.0);
        let stacked = ui.available_width() < 600.0;
        let mut restore = None;
        let list = |app: &App, ui: &mut Ui, st: &mut DocVersionsUi| {
            let w = ui.available_width();
            for v in &versions {
                if version_row(ui, t, w, app, v, st.pick == Some(v.id)).clicked() {
                    st.pick = Some(v.id);
                }
            }
        };
        let mut detail = |app: &App, ui: &mut Ui, st: &DocVersionsUi| {
            let Some(v) = versions.iter().find(|v| Some(v.id) == st.pick) else {
                return;
            };
            let same = v.title == cur_title && v.body == cur_body;
            ui.horizontal(|ui| {
                ui.label(RichText::new(when_text(v.at)).strong());
                ui.label(RichText::new(app.version_author(v)).color(t.text3));
            });
            ui.add_space(6.0);
            if same {
                ui.label(RichText::new(tr("This is the doc's text now.")).color(t.text3));
            } else if may_edit {
                let r = primary_button(ui, t, tr("Restore this version"));
                name_it(&r, "Restore this version");
                if r.clicked() {
                    restore = Some(v.id);
                }
            }
            ui.add_space(8.0);
            ui.separator();
            let title = if v.title.trim().is_empty() {
                tr("Untitled").to_string()
            } else {
                v.title.clone()
            };
            ui.label(RichText::new(title).size(20.0).strong());
            ui.add_space(6.0);
            let job = super::docs::doc_layout_job(&v.body, t, ui.available_width());
            let r = ui.label(job);
            name_it(&r, "Version text");
        };
        if stacked {
            egui::ScrollArea::vertical()
                .id_salt("dv_list")
                .max_height(h * 0.4)
                .auto_shrink([false, true])
                .show(ui, |ui| list(self, ui, st));
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("dv_detail")
                .max_height(h * 0.6)
                .auto_shrink([false, true])
                .show(ui, |ui| detail(self, ui, st));
        } else {
            let left_w = 230.0;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(left_w, h), Layout::top_down(Align::Min), |ui| {
                    ui.set_min_size(vec2(left_w, h));
                    egui::ScrollArea::vertical()
                        .id_salt("dv_list")
                        .max_height(h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| list(self, ui, st));
                });
                ui.separator();
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("dv_detail")
                        .max_height(h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| detail(self, ui, st));
                });
            });
        }
        if let Some(v) = restore {
            if self.restore_version(st.doc, v) {
                self.toast(tr("Version restored"));
                st.pick = self
                    .data
                    .docs
                    .iter()
                    .find(|d| d.id == st.doc)
                    .and_then(|d| d.latest_version())
                    .map(|v| v.id);
            }
        }
        false
    }

    /// Who wrote a version, as the list shows it.
    fn version_author(&self, v: &DocVersion) -> String {
        if v.by == 0 {
            return tr("Earlier edits").into();
        }
        let name = self
            .data
            .member(v.by)
            .map(|m| m.shown().to_string())
            .unwrap_or_else(|| tr("Someone").into());
        match v.restored {
            Some(at) => trf!("{} restored {}", name, when_text(at)),
            None => name,
        }
    }
}

/// When a version was written ("Today 2:30 PM", "Nov 3 9:05 AM").
fn when_text(at: i64) -> String {
    crate::timefmt::local(at)
        .map(|x| fmt_when(x.date_naive(), Some(x.time())))
        .unwrap_or_default()
}

/// One row of the versions list: when, and who.
fn version_row(
    ui: &mut Ui,
    t: &Theme,
    w: f32,
    app: &App,
    v: &DocVersion,
    selected: bool,
) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 44.0), Sense::click());
    name_it(&resp, &format!("Doc version {}", v.id));
    if selected {
        ui.painter().rect_filled(r, 8.0, t.selected_bg);
    } else {
        hover_bg(ui, &resp, r, 8.0);
    }
    let text_w = (r.width() - 20.0).max(0.0);
    let (f1, f2) = (FontId::proportional(13.5), FontId::proportional(11.5));
    let top = shell::fit_text(ui.painter(), &when_text(v.at), &f1, text_w);
    let sub = shell::fit_text(ui.painter(), &app.version_author(v), &f2, text_w);
    let clip = ui.painter().with_clip_rect(r.shrink(2.0));
    clip.text(
        r.left_center() + vec2(10.0, -8.0),
        Align2::LEFT_CENTER,
        top,
        f1,
        if selected { t.selected_text } else { t.text },
    );
    clip.text(
        r.left_center() + vec2(10.0, 9.0),
        Align2::LEFT_CENTER,
        sub,
        f2,
        t.text3,
    );
    resp
}

#[cfg(test)]
impl App {
    /// Tests: the doc being edited was last changed `secs` earlier, as if
    /// that much time had gone by since.
    pub(super) fn age_doc_edit(&mut self, secs: i64) {
        if let Some(l) = self.doc_session.as_mut().and_then(|s| s.last.as_mut()) {
            *l -= secs;
        }
    }

    pub(super) fn doc_editing(&self) -> bool {
        self.doc_session.as_ref().is_some_and(|s| s.last.is_some())
    }
}
